//! The audio engine: the game-side handle, the output host thread and the
//! silent fallback output.
//!
//! The game side never waits on audio. Requests go through a bounded
//! lock-free queue to the output side, which is either a device stream (the
//! mixer runs inside its callback) or, when audio is off, no device opens or
//! the device fails, a silent output that runs the same mixer in real time on
//! the host thread. Each fallback to the silent output logs one line.
//! [`AudioEngine::poll_device`] asks the host to try the device again, at
//! most every 2 s after a loss and less often while it keeps failing.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::bank::SoundBank;
use crate::device;
use crate::mixer::{Mixer, MusicSource, PlayParams};
use crate::settings::{AudioSettings, Channel, MixSettings, PlayRequest};
use crate::state::{AudioState, Command, Garbage, Shared};

/// Capacity of the command queue.
const QUEUE: usize = 1024;
/// Capacity of the return queue.
const GARBAGE_QUEUE: usize = 16;
/// Commands held on the game side while the queue is full; plays beyond this
/// are dropped and counted.
const PENDING_MAX: usize = 4096;
/// Rate of the silent output.
const NULL_RATE: u32 = 48000;
/// Silent output render interval.
const NULL_TICK: Duration = Duration::from_millis(10);
/// How often the host checks a running stream for a device error.
const LOST_CHECK: Duration = Duration::from_millis(50);
/// First retry delay after a device loss, and the longest one.
const RETRY_MIN: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(30);

/// Where the engine's sound currently goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// An output device.
    Device,
    /// Nowhere: the mixer runs in real time and its output is discarded.
    Null,
}

/// Engine counters, for diagnostics and the device smoke test.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AudioStats {
    /// Device sample rate, 0 before a device opened.
    pub device_rate: u32,
    /// Device callbacks so far.
    pub callbacks: u64,
    /// Frames in the last device buffer.
    pub buffer_frames: u64,
    /// Longest callback in microseconds.
    pub max_callback_us: f64,
    /// Mean callback in microseconds.
    pub mean_callback_us: f64,
    /// Commands the output side applied.
    pub commands: u64,
    /// Voices started.
    pub voices_started: u64,
    /// Voices playing after the last render.
    pub active_voices: u32,
    /// Requests dropped because the queue stayed full.
    pub dropped: u64,
    /// Buffer underruns or overruns the device reported; the stream recovers
    /// from these by itself and keeps playing.
    pub xruns: u64,
}

enum Ctl {
    Reopen,
    /// Drop the device stream and go on with the silent output.
    Close,
    Shutdown,
}

struct Host {
    ctl: Sender<Ctl>,
    thread: Option<JoinHandle<()>>,
}

/// The audio engine. Construction never fails; see the module documentation.
pub struct AudioEngine {
    settings: AudioSettings,
    use_device: bool,
    commands: Option<Producer<Command>>,
    garbage: Option<Consumer<Garbage>>,
    pending: VecDeque<Command>,
    bank: Option<Arc<SoundBank>>,
    epoch: u64,
    retired: Vec<(u64, Arc<SoundBank>)>,
    game_paused: bool,
    shared: Arc<Shared>,
    host: Option<Host>,
    last_attempt: Instant,
    retry: Duration,
    dropped: u64,
}

impl fmt::Debug for AudioEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioEngine")
            .field("settings", &self.settings)
            .field("output", &self.output())
            .field("bank_sounds", &self.bank.as_ref().map(|b| b.len()))
            .field("game_paused", &self.game_paused)
            .finish()
    }
}

impl AudioEngine {
    /// Starts the engine. With audio enabled it opens the default output
    /// device on its own thread; otherwise, or when that fails, sound goes to
    /// the silent output.
    pub fn new(settings: AudioSettings) -> AudioEngine {
        AudioEngine::start(settings, true)
    }

    /// Starts the engine on the silent output only; it never opens a device.
    /// For headless runs and tests.
    pub fn null(settings: AudioSettings) -> AudioEngine {
        AudioEngine::start(settings, false)
    }

    fn start(settings: AudioSettings, use_device: bool) -> AudioEngine {
        if !settings.enabled {
            log("audio is off; sound is silent");
        }
        let mut engine = AudioEngine {
            settings,
            use_device,
            commands: None,
            garbage: None,
            pending: VecDeque::new(),
            bank: None,
            epoch: 0,
            retired: Vec::new(),
            game_paused: false,
            shared: Arc::new(Shared::default()),
            host: None,
            last_attempt: Instant::now(),
            retry: RETRY_MIN,
            dropped: 0,
        };
        engine.spawn_host();
        engine
    }

    /// Starts a host thread with fresh queues and resends the current bank,
    /// settings and pause state. A music source given earlier is not restored.
    fn spawn_host(&mut self) {
        let (mut tx, rx) = RingBuffer::new(QUEUE);
        let (gtx, grx) = RingBuffer::new(GARBAGE_QUEUE);
        let shared = Arc::new(Shared::default());
        let mut mixer = Mixer::new(NULL_RATE);
        mixer.set_settings(&self.settings);
        if self.game_paused {
            mixer.pause_game();
        }
        if let Some(bank) = &self.bank {
            let _ = tx.push(Command::Bank {
                epoch: self.epoch,
                bank: Arc::clone(bank),
            });
        }
        let state = AudioState::new(mixer, rx, gtx, Arc::clone(&shared));
        let (ctl_tx, ctl_rx) = mpsc::channel();
        let open_now = self.use_device && self.settings.enabled;
        let spawned = std::thread::Builder::new()
            .name("claw-audio".into())
            .spawn(move || host_main(state, ctl_rx, open_now));
        self.shared = shared;
        self.commands = Some(tx);
        self.garbage = Some(grx);
        self.pending.clear();
        self.last_attempt = Instant::now();
        match spawned {
            Ok(thread) => {
                self.host = Some(Host {
                    ctl: ctl_tx,
                    thread: Some(thread),
                })
            }
            Err(e) => {
                log(&format!(
                    "cannot start the audio thread ({e}); sound is off"
                ));
                self.host = None;
                self.commands = None;
            }
        }
    }

    /// Where sound goes right now.
    pub fn output(&self) -> Output {
        if self.shared.on_device.load(Ordering::Acquire) {
            Output::Device
        } else {
            Output::Null
        }
    }

    /// Current counters.
    pub fn stats(&self) -> AudioStats {
        let s = &self.shared;
        let callbacks = s.callbacks.load(Ordering::Relaxed);
        let total = s.total_callback_ns.load(Ordering::Relaxed);
        AudioStats {
            device_rate: s.device_rate.load(Ordering::Relaxed),
            callbacks,
            buffer_frames: s.last_buffer_frames.load(Ordering::Relaxed),
            max_callback_us: s.max_callback_ns.load(Ordering::Relaxed) as f64 / 1000.0,
            mean_callback_us: if callbacks == 0 {
                0.0
            } else {
                total as f64 / callbacks as f64 / 1000.0
            },
            commands: s.commands.load(Ordering::Relaxed),
            voices_started: s.voices_started.load(Ordering::Relaxed),
            active_voices: s.active_voices.load(Ordering::Relaxed),
            dropped: self.dropped,
            xruns: s.xruns.load(Ordering::Relaxed),
        }
    }

    /// The settings in force.
    pub fn settings(&self) -> &AudioSettings {
        &self.settings
    }

    /// Replaces the sound bank. Voices of the previous bank play on.
    pub fn set_bank(&mut self, bank: SoundBank) {
        self.housekeeping();
        let bank = Arc::new(bank);
        self.epoch += 1;
        if let Some(old) = self.bank.replace(Arc::clone(&bank)) {
            self.retired.push((self.epoch - 1, old));
        }
        self.send(Command::Bank {
            epoch: self.epoch,
            bank,
        });
    }

    /// The current sound bank.
    pub fn bank(&self) -> Option<&SoundBank> {
        self.bank.as_deref()
    }

    /// Plays a sound from the current bank. Unknown names are ignored.
    pub fn play(&mut self, request: &PlayRequest) {
        if !self.settings.enabled {
            return;
        }
        let Some(id) = self.bank.as_ref().and_then(|b| b.id(&request.key)) else {
            return;
        };
        self.send(Command::Play {
            epoch: self.epoch,
            id,
            params: PlayParams::from(request),
        });
    }

    /// Plays an interface sound at the configured volume, centred.
    pub fn play_ui(&mut self, key: &str) {
        self.play(&PlayRequest::new(key, Channel::Interface));
    }

    /// Stops every voice started with `handle`.
    pub fn stop(&mut self, handle: u32) {
        self.send(Command::Stop(handle));
    }

    /// Stops every voice. Music is not affected.
    pub fn stop_all(&mut self) {
        self.send(Command::StopAll);
    }

    /// Pauses the game's sounds: one-shot game sounds stop, loops and the
    /// music hold where they are, interface sounds keep playing (see
    /// [`Mixer::pause_game`]).
    pub fn pause_game(&mut self) {
        self.game_paused = true;
        self.send(Command::PauseGame);
    }

    /// Resumes the loops and the music where they stopped.
    pub fn resume_game(&mut self) {
        self.game_paused = false;
        self.send(Command::ResumeGame);
    }

    /// Applies new settings. Turning audio off closes the output device and
    /// goes on with the silent output; turning it on lets the next
    /// [`AudioEngine::poll_device`] open the device again.
    pub fn set_settings(&mut self, settings: AudioSettings) {
        let mix = MixSettings::from(&settings);
        let was_enabled = self.settings.enabled;
        self.settings = settings;
        if self.settings.enabled && !was_enabled {
            self.retry = RETRY_MIN;
            self.last_attempt = Instant::now()
                .checked_sub(RETRY_MIN)
                .unwrap_or(self.last_attempt);
        }
        self.send(Command::Settings(mix));
        if was_enabled
            && !self.settings.enabled
            && let Some(host) = &self.host
        {
            log("audio is off; sound is silent");
            let _ = host.ctl.send(Ctl::Close);
        }
    }

    /// Sets or clears the music source, mixed at the music volume.
    pub fn set_music(&mut self, music: Option<Box<dyn MusicSource>>) {
        self.send(Command::Music(music));
    }

    /// Housekeeping to call once per frame: drops what the output side gave
    /// back, and asks for the device again after a loss (at most every 2 s,
    /// backing off to 30 s while it keeps failing).
    pub fn poll_device(&mut self) {
        self.housekeeping();
        if self.shared.dead.load(Ordering::Acquire) {
            if self.last_attempt.elapsed() >= RETRY_MIN {
                self.join_host();
                self.spawn_host();
            }
            return;
        }
        if !self.use_device || !self.settings.enabled {
            return;
        }
        if self.output() == Output::Device {
            self.retry = RETRY_MIN;
            return;
        }
        if self.last_attempt.elapsed() < self.retry {
            return;
        }
        if let Some(host) = &self.host
            && host.ctl.send(Ctl::Reopen).is_ok()
        {
            self.last_attempt = Instant::now();
            self.retry = (self.retry * 2).min(RETRY_MAX);
        }
    }

    /// Reports a device failure as the device's error callback would, for the
    /// device smoke test.
    #[doc(hidden)]
    pub fn simulate_device_loss(&self) {
        self.shared.lost.store(true, Ordering::Release);
    }

    /// Drops returned objects, retires unused banks and flushes held
    /// commands.
    fn housekeeping(&mut self) {
        if let Some(g) = self.garbage.as_mut() {
            while let Ok(item) = g.pop() {
                drop(item);
            }
        }
        let floor = self.shared.retire_floor.load(Ordering::Acquire);
        self.retired.retain(|(epoch, _)| *epoch >= floor);
        self.flush();
    }

    fn flush(&mut self) {
        let Some(tx) = self.commands.as_mut() else {
            self.pending.clear();
            return;
        };
        while let Some(cmd) = self.pending.pop_front() {
            if let Err(rtrb::PushError::Full(cmd)) = tx.push(cmd) {
                self.pending.push_front(cmd);
                break;
            }
        }
    }

    fn send(&mut self, cmd: Command) {
        if self.commands.is_none() {
            return;
        }
        self.flush();
        if self.pending.is_empty()
            && let Some(tx) = self.commands.as_mut()
        {
            match tx.push(cmd) {
                Ok(()) => return,
                Err(rtrb::PushError::Full(cmd)) => return self.hold(cmd),
            }
        }
        self.hold(cmd);
    }

    fn hold(&mut self, cmd: Command) {
        if self.pending.len() >= PENDING_MAX && matches!(cmd, Command::Play { .. }) {
            self.dropped += 1;
        } else {
            self.pending.push_back(cmd);
        }
    }

    fn join_host(&mut self) {
        if let Some(mut host) = self.host.take() {
            let _ = host.ctl.send(Ctl::Shutdown);
            if let Some(t) = host.thread.take() {
                let _ = t.join();
            }
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.join_host();
        self.housekeeping();
    }
}

fn log(line: &str) {
    eprintln!("claw-audio: {line}");
}

/// The host thread: owns the output side and switches it between the device
/// and the silent output.
fn host_main(state: AudioState, ctl: Receiver<Ctl>, open_now: bool) {
    let mut host = HostThread {
        shared: Arc::clone(&state.shared),
        state: Some(Box::new(state)),
        stream: None,
        null_start: Instant::now(),
        null_frames: 0,
        logged_null: false,
    };
    if open_now {
        host.try_open();
    } else {
        host.enter_null(None);
    }
    loop {
        if host.stream.is_some() {
            match ctl.recv_timeout(LOST_CHECK) {
                Ok(Ctl::Reopen) => {
                    if !host.close() {
                        return;
                    }
                    host.try_open();
                }
                Ok(Ctl::Close) => {
                    if !host.close() {
                        return;
                    }
                    host.enter_null(None);
                }
                Ok(Ctl::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                    host.close();
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if host.shared.lost.load(Ordering::Acquire) {
                        if !host.close() {
                            return;
                        }
                        host.enter_null(Some("the output device stopped"));
                    }
                }
            }
        } else if host.state.is_some() {
            host.run_null();
            match ctl.recv_timeout(NULL_TICK) {
                Ok(Ctl::Reopen) => host.try_open(),
                Ok(Ctl::Close) => {}
                Ok(Ctl::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
        } else {
            // The state was lost with a stream; the game side starts a new
            // host.
            return;
        }
    }
}

struct HostThread {
    shared: Arc<Shared>,
    /// The output side while it is not inside a stream.
    state: Option<Box<AudioState>>,
    stream: Option<device::Opened>,
    null_start: Instant,
    null_frames: u64,
    logged_null: bool,
}

impl HostThread {
    fn try_open(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        self.shared.lost.store(false, Ordering::Release);
        match device::open(*state) {
            Ok(opened) => {
                self.stream = Some(opened);
                self.shared.on_device.store(true, Ordering::Release);
                if self.logged_null {
                    log("output device opened; sound is back");
                    self.logged_null = false;
                }
            }
            Err((Some(state), reason)) => {
                self.state = Some(state);
                self.enter_null(Some(&reason));
            }
            Err((None, _)) => self.shared.dead.store(true, Ordering::Release),
        }
    }

    /// Stops the stream and takes the state back. False when it did not come
    /// back, in which case the host is marked dead.
    fn close(&mut self) -> bool {
        let Some(opened) = self.stream.take() else {
            return self.state.is_some();
        };
        self.shared.on_device.store(false, Ordering::Release);
        let device::Opened { stream, back, .. } = opened;
        drop(stream);
        match back.recv_timeout(device::RETURN_TIMEOUT) {
            Ok(state) => {
                self.state = Some(Box::new(state));
                true
            }
            Err(_) => {
                self.shared.dead.store(true, Ordering::Release);
                false
            }
        }
    }

    /// Switches to the silent output, logging the reason once.
    fn enter_null(&mut self, reason: Option<&str>) {
        self.shared.on_device.store(false, Ordering::Release);
        if let Some(state) = self.state.as_mut() {
            state.mixer.set_out_rate(NULL_RATE);
        }
        self.null_start = Instant::now();
        self.null_frames = 0;
        if let Some(reason) = reason
            && !self.logged_null
        {
            log(&format!("{reason}; sound continues silently"));
            self.logged_null = true;
        }
    }

    /// Renders the frames due since the silent output started, catching up at
    /// most 200 ms.
    fn run_null(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let rate = u64::from(NULL_RATE);
        let due = (self.null_start.elapsed().as_secs_f64() * rate as f64) as u64;
        let mut todo = due.saturating_sub(self.null_frames);
        if todo > rate / 5 {
            todo = rate / 5;
            self.null_frames = due - todo;
        }
        state.run_silent(todo);
        self.null_frames += todo;
    }
}
