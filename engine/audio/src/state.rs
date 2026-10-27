//! The mixing state that lives on the output side, and the plain-data
//! commands that reach it.
//!
//! Everything here runs inside the output callback (or the silent output's
//! loop): it reads commands from a bounded lock-free queue and renders, and it
//! never allocates, locks or blocks. Objects whose last owner could be this
//! side are handed back to the game side to drop:
//!
//! - a replaced music source or movie soundtrack goes back through the
//!   return queue;
//! - sound banks: the game side keeps every bank it sent until this side
//!   reports, through the retire floor, that no playing voice still uses it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use rtrb::{Consumer, Producer};

use crate::bank::{SoundBank, SoundId};
use crate::mixer::{Mixer, MusicSource, PlayParams};
use crate::settings::MixSettings;

/// Stereo frames rendered per inner step on the output side.
pub(crate) const SCRATCH_FRAMES: usize = 1024;

/// A request from the game side.
pub(crate) enum Command {
    Play {
        epoch: u64,
        id: SoundId,
        params: PlayParams,
    },
    Stop(u32),
    StopAll,
    PauseGame,
    ResumeGame,
    HoldMusic(bool),
    Settings(MixSettings),
    Bank {
        epoch: u64,
        bank: Arc<SoundBank>,
    },
    Music(Option<Box<dyn MusicSource>>),
    Movie(Option<Box<dyn MusicSource>>),
}

/// Something the output side gives back for the game side to drop.
pub(crate) enum Garbage {
    Music(#[allow(dead_code)] Box<dyn MusicSource>),
    Movie(#[allow(dead_code)] Box<dyn MusicSource>),
}

/// Counters and flags shared between the game side, the output host thread
/// and the callback. Plain atomics only.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    /// The device reported an error that ends the stream.
    pub lost: AtomicBool,
    /// A device stream is running (otherwise the silent output is).
    pub on_device: AtomicBool,
    /// The host thread could not get its state back and stopped.
    pub dead: AtomicBool,
    /// Banks with an epoch below this are no longer used by any voice.
    pub retire_floor: AtomicU64,
    pub device_rate: AtomicU32,
    pub callbacks: AtomicU64,
    pub last_buffer_frames: AtomicU64,
    pub max_callback_ns: AtomicU64,
    pub total_callback_ns: AtomicU64,
    pub commands: AtomicU64,
    pub voices_started: AtomicU64,
    pub active_voices: AtomicU32,
    /// Buffer underruns or overruns the device reported and recovered from.
    pub xruns: AtomicU64,
}

/// The output side's state: the mixer, the queue ends and the current bank.
pub(crate) struct AudioState {
    pub mixer: Mixer,
    commands: Consumer<Command>,
    garbage: Producer<Garbage>,
    bank: Option<Arc<SoundBank>>,
    epoch: u64,
    swap_seq: u64,
    pub shared: Arc<Shared>,
    scratch: Box<[f32]>,
}

impl AudioState {
    pub fn new(
        mixer: Mixer,
        commands: Consumer<Command>,
        garbage: Producer<Garbage>,
        shared: Arc<Shared>,
    ) -> AudioState {
        AudioState {
            mixer,
            commands,
            garbage,
            bank: None,
            epoch: 0,
            swap_seq: 0,
            shared,
            scratch: vec![0.0; SCRATCH_FRAMES * 2].into_boxed_slice(),
        }
    }

    /// Applies every queued command.
    pub fn process_commands(&mut self) {
        let mut n = 0u64;
        while let Ok(cmd) = self.commands.pop() {
            n += 1;
            match cmd {
                Command::Play { epoch, id, params } => {
                    if epoch != self.epoch {
                        continue;
                    }
                    if let Some(bank) = self.bank.as_deref()
                        && self.mixer.play_sound(id, params, bank).is_some()
                    {
                        self.shared.voices_started.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Command::Stop(handle) => {
                    self.mixer.stop(handle);
                }
                Command::StopAll => self.mixer.stop_all(),
                Command::PauseGame => self.mixer.pause_game(),
                Command::ResumeGame => self.mixer.resume_game(),
                Command::HoldMusic(on) => self.mixer.hold_music(on),
                Command::Settings(s) => self.mixer.apply_settings(s),
                Command::Bank { epoch, bank } => {
                    // The previous bank's last owner is the game side, which
                    // keeps it until the retire floor passes it.
                    self.bank = Some(bank);
                    self.epoch = epoch;
                    self.swap_seq = self.mixer.next_seq();
                }
                Command::Music(music) => {
                    if let Some(old) = self.mixer.set_music(music) {
                        // When the return queue is full the source is dropped
                        // here; the queue is drained on every game-side call.
                        let _ = self.garbage.push(Garbage::Music(old));
                    }
                }
                Command::Movie(movie) => {
                    if let Some(old) = self.mixer.set_movie(movie) {
                        let _ = self.garbage.push(Garbage::Movie(old));
                    }
                }
            }
        }
        if n > 0 {
            self.shared.commands.fetch_add(n, Ordering::Relaxed);
        }
    }

    /// Publishes the retire floor and the voice count after a render.
    fn publish(&self) {
        let clear = self
            .mixer
            .oldest_active_seq()
            .is_none_or(|s| s >= self.swap_seq);
        if clear {
            self.shared
                .retire_floor
                .store(self.epoch, Ordering::Release);
        }
        self.shared
            .active_voices
            .store(self.mixer.active_voices() as u32, Ordering::Relaxed);
    }

    /// Processes commands and fills a device buffer of `channels` interleaved
    /// channels. Stereo goes to the first two channels and further channels
    /// get silence; a mono device gets the average of left and right.
    pub fn fill<T>(&mut self, data: &mut [T], channels: usize)
    where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        let start = Instant::now();
        self.process_commands();
        let channels = channels.max(1);
        let frames = data.len() / channels;
        for block in data.chunks_mut(SCRATCH_FRAMES * channels) {
            let n = block.len() / channels;
            let mix = &mut self.scratch[..n * 2];
            self.mixer.render(mix);
            for (frame, lr) in block.chunks_exact_mut(channels).zip(mix.chunks_exact(2)) {
                if channels == 1 {
                    frame[0] = T::from_sample((lr[0] + lr[1]) * 0.5);
                } else {
                    frame[0] = T::from_sample(lr[0]);
                    frame[1] = T::from_sample(lr[1]);
                    for s in &mut frame[2..] {
                        *s = T::EQUILIBRIUM;
                    }
                }
            }
            for s in block.chunks_exact_mut(channels).into_remainder() {
                *s = T::EQUILIBRIUM;
            }
        }
        self.publish();
        let ns = start.elapsed().as_nanos() as u64;
        let sh = &self.shared;
        sh.callbacks.fetch_add(1, Ordering::Relaxed);
        sh.last_buffer_frames
            .store(frames as u64, Ordering::Relaxed);
        sh.total_callback_ns.fetch_add(ns, Ordering::Relaxed);
        sh.max_callback_ns.fetch_max(ns, Ordering::Relaxed);
    }

    /// Processes commands and renders `frames` frames into nothing, for the
    /// silent output.
    pub fn run_silent(&mut self, frames: u64) {
        self.process_commands();
        let mut left = frames;
        while left > 0 {
            let n = left.min(SCRATCH_FRAMES as u64) as usize;
            self.mixer.render(&mut self.scratch[..n * 2]);
            left -= n as u64;
        }
        self.publish();
    }
}
