//! One level being played: the simulation, the fixed-step frame driver and
//! the input feeding it, without any window or GPU.
//!
//! The window, the headless replay and the benchmarks all run the game
//! through a [`Session`], so they share one frame loop:
//!
//! 1. key events are stamped with the simulation time at which they
//!    happened ([`Session::stamp`]) and queued;
//! 2. each frame adds its duration to the 18 ms accumulator and runs the
//!    whole ticks that fit, each one taking the input of its own 18 ms span
//!    (or the next row of a replayed script);
//! 3. the frame is drawn between the previous and the current tick.
//!
//! Because each key event lands in the tick whose span contains it, the
//! same events give the same ticks at any display rate.
//!
//! Sound cues and game events belong to the tick that made them, and a
//! frame can run several ticks, so the session gathers them from every
//! tick it runs; [`Session::drain_outputs`] hands them over. Replays and
//! benchmarks never drain them; the queues are bounded, and what does not
//! fit is counted and dropped.

use std::time::{Duration, Instant};

use claw_frame::step::{MAX_FRAME_US, TICK_US};
use claw_frame::{AudioCue, Buttons, FrameDriver, GameEvent, InterpolatedFrame, TickInput};
use claw_sim::trace::TraceRow;
use claw_sim::{InputLatch, InputScript, Sim};

/// What one frame did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Ticks run.
    pub ticks: u32,
    /// Interpolation factor after the ticks, in [0, 1).
    pub alpha: f64,
}

/// Cost of one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickSample {
    /// Time spent in the simulation step, ms.
    pub ms: f64,
    /// Objects in the live list after the tick (Claw and the camera
    /// included).
    pub live: u32,
}

/// Called before each tick with the index of the tick about to run.
pub type BeforeTick = Box<dyn FnMut(u64, &mut Sim)>;

/// Most sound cues, and separately most game events, kept between two
/// calls of [`Session::drain_outputs`].
pub const OUTPUT_LIMIT: usize = 4096;

/// Sound cues and game events of the ticks run since the last drain.
#[derive(Debug, Default)]
struct Outputs {
    audio: Vec<AudioCue>,
    events: Vec<GameEvent>,
    dropped: u64,
}

impl Outputs {
    fn add(&mut self, audio: &[AudioCue], events: &[GameEvent]) {
        let room = OUTPUT_LIMIT - self.audio.len();
        self.audio.extend(audio.iter().take(room).cloned());
        self.dropped += audio.len().saturating_sub(room) as u64;
        let room = OUTPUT_LIMIT - self.events.len();
        self.events.extend(events.iter().take(room).copied());
        self.dropped += events.len().saturating_sub(room) as u64;
    }
}

/// A level in play.
pub struct Session {
    sim: Sim,
    driver: FrameDriver,
    latch: InputLatch,
    ticks: u64,
    last_stamp: u64,
    paused: bool,
    script: Option<InputScript>,
    looped: bool,
    past_script: u64,
    trace: Option<Vec<TraceRow>>,
    samples: Option<Vec<TickSample>>,
    before_tick: Option<BeforeTick>,
    outputs: Outputs,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("ticks", &self.ticks)
            .field("replay", &self.script.as_ref().map(|s| s.ticks.len()))
            .field("tracing", &self.trace.is_some())
            .finish()
    }
}

impl Session {
    /// Starts playing `sim` from its current state.
    pub fn new(sim: Sim) -> Session {
        let first = sim.snapshot();
        Session {
            sim,
            driver: FrameDriver::new(first),
            latch: InputLatch::new(),
            ticks: 0,
            last_stamp: 0,
            paused: false,
            script: None,
            looped: false,
            past_script: 0,
            trace: None,
            samples: None,
            before_tick: None,
            outputs: Outputs::default(),
        }
    }

    /// Plays `script` from the next tick: one row per tick, then the
    /// keyboard again.
    pub fn with_replay(mut self, script: InputScript) -> Session {
        self.script = Some(script);
        self.looped = false;
        self
    }

    /// Plays `script` from the next tick over and over, for benchmarks
    /// that run for a time rather than a number of ticks.
    pub fn with_looped_replay(mut self, script: InputScript) -> Session {
        self.script = Some(script);
        self.looped = true;
        self
    }

    /// Ticks that took keyboard input because a replayed script had no row
    /// left for them; always 0 for a looped script.
    pub fn ticks_past_script(&self) -> u64 {
        self.past_script
    }

    /// Records one trace row per tick from now on.
    pub fn record_trace(&mut self) {
        self.trace.get_or_insert_with(Vec::new);
    }

    /// The trace rows recorded so far.
    pub fn trace(&self) -> &[TraceRow] {
        self.trace.as_deref().unwrap_or(&[])
    }

    /// Measures every tick from now on, dropping earlier measurements.
    pub fn time_ticks(&mut self) {
        self.samples = Some(Vec::new());
    }

    /// The tick measurements so far.
    pub fn tick_samples(&self) -> &[TickSample] {
        self.samples.as_deref().unwrap_or(&[])
    }

    /// Runs `hook` before every tick (tools and tests).
    pub fn set_before_tick(&mut self, hook: BeforeTick) {
        self.before_tick = Some(hook);
    }

    /// The simulation.
    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// The sound cues and game events of every tick run since the last
    /// call, in order.
    pub fn drain_outputs(&mut self) -> (Vec<AudioCue>, Vec<GameEvent>) {
        (
            std::mem::take(&mut self.outputs.audio),
            std::mem::take(&mut self.outputs.events),
        )
    }

    /// Cues and events dropped so far because more than
    /// [`OUTPUT_LIMIT`] waited to be drained.
    pub fn dropped_outputs(&self) -> u64 {
        self.outputs.dropped
    }

    /// Ticks run by this session.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// True while a replayed script still has rows left.
    pub fn replaying(&self) -> bool {
        self.script.as_ref().is_some_and(|s| {
            if self.looped {
                !s.ticks.is_empty()
            } else {
                (self.ticks as usize) < s.ticks.len()
            }
        })
    }

    /// The simulation clock at the last frame, in microseconds: the ticks
    /// run plus the time waiting in the accumulator.
    pub fn now_us(&self) -> u64 {
        self.ticks * TICK_US + (self.driver.alpha() * TICK_US as f64).round() as u64
    }

    /// The simulation time of an event that happened `since_frame` after the
    /// last frame (no later than the longest frame the accumulator takes;
    /// no time passes while paused). Stamps never go backwards.
    pub fn stamp(&mut self, since_frame: Duration) -> u64 {
        let extra = if self.is_paused() {
            0
        } else {
            u64::try_from(since_frame.as_micros())
                .unwrap_or(u64::MAX)
                .min(MAX_FRAME_US)
        };
        let at = (self.now_us() + extra).max(self.last_stamp);
        self.last_stamp = at;
        at
    }

    /// A key event at simulation time `at_us`: `buttons` went down or up.
    pub fn key(&mut self, at_us: u64, buttons: Buttons, down: bool) {
        let at = at_us.max(self.last_stamp);
        self.last_stamp = at;
        self.latch.event(at, buttons, down);
    }

    /// Releases every held button at `at_us`, for example when the window
    /// loses focus.
    pub fn release_all(&mut self, at_us: u64) {
        let down = self.latch.down();
        if !down.is_empty() {
            self.key(at_us, down, false);
        }
    }

    /// Adds one frame of `dt` and runs the ticks that fit.
    pub fn frame(&mut self, dt: Duration) -> Frame {
        let Session {
            sim,
            driver,
            latch,
            ticks,
            script,
            looped,
            past_script,
            trace,
            samples,
            before_tick,
            outputs,
            ..
        } = self;
        let report = driver.frame(dt, || {
            let n = *ticks;
            let live = latch.take(n);
            let scripted = script.as_ref().and_then(|s| {
                let row = match s.ticks.len() {
                    len if *looped && len > 0 => (n % len as u64) as usize,
                    _ => n as usize,
                };
                s.ticks.get(row)
            });
            if script.is_some() && scripted.is_none() {
                *past_script += 1;
            }
            let input: TickInput = scripted.copied().unwrap_or(live);
            if let Some(hook) = before_tick {
                hook(n, sim);
            }
            let snapshot = if let Some(samples) = samples {
                let t = Instant::now();
                let snapshot = sim.step(input);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let live = u32::try_from(sim.objects().listed().len()).unwrap_or(u32::MAX);
                samples.push(TickSample { ms, live });
                snapshot
            } else {
                sim.step(input)
            };
            outputs.add(&snapshot.audio, &snapshot.events);
            if let Some(rows) = trace {
                let source = if scripted.is_some() {
                    "script"
                } else {
                    "keyboard"
                };
                rows.push(TraceRow::capture(sim, n, source));
            }
            *ticks += 1;
            snapshot
        });
        Frame {
            ticks: report.ticks,
            alpha: report.alpha,
        }
    }

    /// The frame to draw: interpolated between the last two ticks, or the
    /// latest tick alone.
    pub fn interpolated(&self, interpolate: bool) -> InterpolatedFrame<'_> {
        if interpolate {
            self.driver.interpolated()
        } else {
            self.driver.latest()
        }
    }

    /// Stops the clock: frames run no ticks until [`Session::resume`].
    pub fn pause(&mut self) {
        self.driver.step_mut().pause();
        self.paused = true;
    }

    /// Restarts the clock.
    pub fn resume(&mut self) {
        self.driver.step_mut().resume();
        self.paused = false;
    }

    /// True while paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_frame::SimulatedRefresh;
    use claw_sim::{SimConfig, SimLevel, TileMap};

    fn level() -> SimLevel {
        let mut rows = vec![".".repeat(120); 15];
        rows.push("#".repeat(120));
        rows.push("#".repeat(120));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
    }

    const RIGHT: u32 = 0x200_0000;
    const LEFT: u32 = 0x100_0000;
    const JUMP: u32 = 0x1;

    fn script() -> InputScript {
        let mut held = vec![0u32; 20];
        held.extend([RIGHT; 80]);
        held.extend([RIGHT | JUMP; 12]);
        held.extend([RIGHT; 40]);
        held.extend([0; 10]);
        held.extend([JUMP; 1]);
        held.extend([0; 30]);
        held.extend([LEFT | JUMP; 30]);
        held.extend([0; 40]);
        InputScript::from_held(&held)
    }

    /// Runs `script` as key events on a `hz` display, with optional
    /// jitter, and returns the trace as CSV. Each event happens a little
    /// after its tick begins and is handled while the frame that contains
    /// it is under way, stamped with the moment it happened.
    fn keyboard_run(hz: f64, jitter: Option<u64>, script: &InputScript) -> String {
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.record_trace();
        let events = script.events();
        let mut next = 0;
        let mut refresh = SimulatedRefresh::new(hz);
        if let Some(seed) = jitter {
            refresh = refresh.with_jitter(0.25, seed);
        }
        let mut clock_us = 0u64;
        while session.ticks() < script.ticks.len() as u64 {
            let dt = refresh.next().unwrap();
            let end = clock_us + dt.as_micros() as u64;
            while let Some(&(at, b, down)) = events.get(next) {
                let happened = at + 1_000;
                if happened > end {
                    break;
                }
                let stamp = session.stamp(Duration::from_micros(happened - clock_us));
                assert_eq!(stamp, happened);
                session.key(stamp, b, down);
                next += 1;
            }
            session.frame(dt);
            clock_us = end;
        }
        let rows = &session.trace()[..script.ticks.len()];
        claw_sim::trace::to_csv(rows)
    }

    #[test]
    fn keyboard_traces_do_not_depend_on_the_display_rate() {
        let s = script();
        let base = keyboard_run(60.0, None, &s);
        for (hz, jitter) in [
            (144.0, None),
            (240.0, None),
            (60.0, Some(3)),
            (144.0, Some(4)),
            (240.0, Some(5)),
            (360.0, None),
        ] {
            assert_eq!(keyboard_run(hz, jitter, &s), base, "{hz} Hz {jitter:?}");
        }
        // The same as feeding the script tick by tick, apart from the
        // input source column.
        let mut sim = Sim::new(level(), SimConfig::default());
        let rows: Vec<TraceRow> = s
            .ticks
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                sim.step(t);
                TraceRow::capture(&sim, i as u64, "keyboard")
            })
            .collect();
        assert_eq!(claw_sim::trace::to_csv(&rows), base);
    }

    #[test]
    fn replays_do_not_depend_on_the_display_rate() {
        let run = |hz: f64| {
            let mut session =
                Session::new(Sim::new(level(), SimConfig::default())).with_replay(script());
            session.record_trace();
            let mut refresh = SimulatedRefresh::new(hz).with_jitter(0.2, hz as u64);
            while session.replaying() {
                session.frame(refresh.next().unwrap());
            }
            assert!(session.trace().iter().all(|r| r.source == "script"));
            claw_sim::trace::to_csv(&session.trace()[..script().ticks.len()])
        };
        let base = run(60.0);
        assert_eq!(run(144.0), base);
        assert_eq!(run(240.0), base);
    }

    #[test]
    fn a_looped_replay_never_runs_out() {
        let s = script();
        let len = s.ticks.len() as u64;
        let mut once = Session::new(Sim::new(level(), SimConfig::default())).with_replay(s.clone());
        let mut looped =
            Session::new(Sim::new(level(), SimConfig::default())).with_looped_replay(s);
        looped.record_trace();
        for _ in 0..(3 * len) {
            once.frame(Duration::from_millis(18));
            looped.frame(Duration::from_millis(18));
        }
        assert_eq!(looped.ticks(), 3 * len);
        assert!(looped.replaying());
        assert_eq!(looped.ticks_past_script(), 0);
        assert!(looped.trace().iter().all(|r| r.source == "script"));
        let held = |i: usize| looped.trace()[i].held;
        assert_eq!(held(5), held(5 + len as usize));
        assert!(!once.replaying());
        assert_eq!(once.ticks_past_script(), 2 * len);
    }

    #[test]
    fn a_tap_shorter_than_a_frame_is_one_press() {
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.record_trace();
        for _ in 0..30 {
            session.frame(Duration::from_micros(16_667));
        }
        let at = session.stamp(Duration::from_millis(2));
        session.key(at, Buttons::JUMP, true);
        let at = session.stamp(Duration::from_millis(3));
        session.key(at, Buttons::JUMP, false);
        for _ in 0..30 {
            session.frame(Duration::from_micros(16_667));
        }
        let presses: Vec<&TraceRow> = session
            .trace()
            .iter()
            .filter(|r| r.pressed & JUMP != 0)
            .collect();
        assert_eq!(presses.len(), 1);
        assert_eq!(presses[0].held & JUMP, JUMP);
        assert!(session.trace().iter().any(|r| r.state_id == 4), "jumped");
    }

    #[test]
    fn pause_stops_ticks_and_the_clock() {
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.frame(Duration::from_millis(40));
        let t = session.now_us();
        session.pause();
        assert!(session.is_paused());
        assert_eq!(session.frame(Duration::from_millis(500)).ticks, 0);
        assert_eq!(session.stamp(Duration::from_millis(30)), t);
        session.resume();
        assert_eq!(session.frame(Duration::from_millis(18)).ticks, 1);
    }

    #[test]
    fn tick_samples_count_live_objects() {
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.time_ticks();
        session.frame(Duration::from_millis(90));
        let s = session.tick_samples();
        assert_eq!(s.len(), 4);
        assert!(s.iter().all(|t| t.live == 2 && t.ms >= 0.0));
    }

    /// A level whose standing animation plays a sound on every tick.
    fn noisy_level() -> SimLevel {
        let mut l = level();
        let mut stand = claw_sim::anim::Animation::looping("CLAW_STAND", &[(11, 1)]);
        stand.frame_sounds = vec![vec![Some("TICK".into())]];
        l.animations
            .insert(stand.key.clone(), std::sync::Arc::new(stand));
        l
    }

    #[test]
    fn frames_of_several_ticks_lose_no_cues() {
        let mut session = Session::new(Sim::new(noisy_level(), SimConfig::default()));
        // The level start's own cue goes out with the first tick.
        let first = session.frame(Duration::from_millis(40)).ticks as usize;
        assert!(first >= 1);
        assert_eq!(session.drain_outputs().0.len(), first + 1);
        let ticks = session.frame(Duration::from_millis(90)).ticks as usize;
        assert!(ticks >= 4, "{ticks}");
        let (audio, events) = session.drain_outputs();
        assert_eq!(audio.len(), ticks);
        assert!(events.is_empty());
        assert!(session.drain_outputs().0.is_empty());
        // Undrained cues are bounded.
        for _ in 0..(OUTPUT_LIMIT + 100) {
            session.frame(Duration::from_millis(18));
        }
        assert_eq!(session.drain_outputs().0.len(), OUTPUT_LIMIT);
        assert_eq!(session.dropped_outputs(), 100);
    }

    #[test]
    fn hooks_run_before_each_tick() {
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.record_trace();
        session.set_before_tick(Box::new(|n, sim: &mut Sim| {
            if n == 3 {
                sim.teleport_player(1500, 600);
            }
        }));
        session.frame(Duration::from_millis(90));
        let xs: Vec<i32> = session.trace().iter().map(|r| r.x).collect();
        assert_eq!(xs[3], 1500);
        assert_ne!(xs[2], 1500);
    }
}
