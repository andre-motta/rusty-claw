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
//!
//! A recorded input script starts with comment lines naming the behaviour
//! switches the game was played with: the mid-air hit jump fix
//! ([`behaviour_comment`]), then how keys count across a pause and whether
//! help screen time reaches the dynamite fuse ([`switch_comments`]). A
//! replay reads them back ([`script_behaviour`], [`script_switches`]) so
//! the recording plays the same.
//!
//! # Pauses, menus and commands
//!
//! Besides the input of each tick, a game holds what happened between two
//! ticks: a pause, a menu screen over play and the in-game menu's end of a
//! life. The session keeps them as the directives of an input script (see
//! [`claw_sim::input`]), each at the boundary before the tick it precedes:
//!
//! - played live, [`Session::open_span`] and [`Session::close_span`] mark a
//!   pause or a menu (the wall time it lasted is counted from the frames
//!   that ran meanwhile, and [`Session::menu_key`] notes the menu inputs),
//!   and [`Session::end_life`] gives up Claw's life at the next tick;
//! - replayed ([`Session::with_replay`], or [`Session::with_directives`]
//!   with live input), a pause or menu directive holds play at its tick
//!   boundary for its wall time, as the pause did when it was recorded, and
//!   a command takes effect before its tick. Holding takes frame time only:
//!   the ticks and their input are the same at any display rate.
//!
//! [`Session::directives`] lists every directive given or played so far;
//! the recorder writes them into the recording. A pause or menu changes
//! nothing in the game itself: play stops and goes on where it stopped,
//! with the input the ticks after it were recorded with. The one exception
//! is the help screen: with the fuse clock at its original default
//! ([`claw_sim::tuning::FuseClock`]) the time it was up reaches the
//! dynamite fuse at the boundary after it ([`Sim::pass_menu_time`]), live
//! and replayed alike.
//!
//! # Going on after a pause or a screen
//!
//! Played live, play stops at the tick boundary: the ticks run so far stay
//! and the time waiting in the accumulator is dropped. When it goes on
//! ([`Session::resume`] with a span open), exactly one tick runs at once,
//! in the next frame whatever its length, and real time counts from an
//! empty accumulator after it, so pausing and resuming steps the game one
//! tick at a time and never depends on how long the pause lasted (the
//! frame stepper's [`claw_frame::step::FixedStep::step_once`]). The input
//! of that tick follows the session's [`PauseInput`]:
//!
//! - original (the default): nothing is released when play stops, and the
//!   key events while it is stopped are not taken one by one. After a
//!   pause, the keys down at the resume are held and those not held on the
//!   last tick before the pause are pressed; after a screen, the keys down
//!   are held and none is pressed, so a key held on counts as pressed only
//!   once released and pressed again. The window tells the session which
//!   keys are down ([`Session::sync_keys`]), since a screen takes the keys
//!   while it is up.
//! - release all: every key is released when play stops (the caller does
//!   it, see [`crate::pause::Pauses::apply`]) and the key events while it
//!   is stopped count in the first tick.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use claw_frame::step::{MAX_FRAME_US, TICK_US};
use claw_frame::{AudioCue, Buttons, FrameDriver, GameEvent, InterpolatedFrame, TickInput};
use claw_sim::input::{MenuScreen, ScriptCommand};
use claw_sim::trace::TraceRow;
use claw_sim::tuning::FuseClock;
use claw_sim::{Directive, DirectiveKind, InputLatch, InputScript, ScriptHeader, Sim};

use crate::input::Recorder;
use crate::pause::PauseInput;

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

/// A stop of play between two ticks that a recording keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// The pause key.
    Pause,
    /// A menu screen over play.
    Menu(MenuScreen),
}

/// A pause or menu being played live: where its directive sits in the log,
/// and what it gathered so far.
#[derive(Debug, Clone)]
struct LiveSpan {
    index: usize,
    span: Span,
    wall_us: u64,
    keys: Vec<String>,
}

impl LiveSpan {
    fn wall_ms(&self) -> u32 {
        u32::try_from(self.wall_us.div_ceil(1000)).unwrap_or(u32::MAX)
    }

    fn kind(&self) -> DirectiveKind {
        let wall_ms = self.wall_ms();
        match self.span {
            Span::Pause => DirectiveKind::Pause {
                wall_ms,
                held: None,
            },
            Span::Menu(screen) => DirectiveKind::Menu {
                screen,
                wall_ms,
                keys: self.keys.clone(),
            },
        }
    }
}

/// The directives a session plays and the ones it has played or been
/// given.
#[derive(Debug, Default)]
struct Directives {
    /// To play, in tick order.
    plan: Vec<Directive>,
    /// The next one of `plan`.
    next: usize,
    /// Played or given so far, in order.
    log: Vec<Directive>,
}

impl Directives {
    /// Gives the commands of `plan` due before tick `tick`, up to the next
    /// pause or menu, which it leaves for [`Session::frame`] to hold at its
    /// boundary before the tick can run.
    fn commands_before(&mut self, tick: u64, sim: &mut Sim) {
        while let Some(d) = self.plan.get(self.next)
            && d.tick as u64 <= tick
            && matches!(d.kind, DirectiveKind::Command(_))
        {
            let d = d.clone();
            self.next += 1;
            if let DirectiveKind::Command(c) = d.kind {
                apply_command(sim, c);
            }
            self.log.push(d);
        }
    }

    /// The tick before which the next pause or menu of `plan` holds play.
    fn next_hold(&self) -> Option<u64> {
        self.plan[self.next.min(self.plan.len())..]
            .iter()
            .find(|d| !matches!(d.kind, DirectiveKind::Command(_)))
            .map(|d| d.tick as u64)
    }
}

/// The help screen time a pause or menu directive passes to the dynamite
/// fuse when play goes on after it (see [`Sim::pass_menu_time`]).
fn menu_time(kind: &DirectiveKind) -> Option<u32> {
    match kind {
        DirectiveKind::Menu {
            screen: MenuScreen::Help,
            wall_ms,
            ..
        } => Some(*wall_ms),
        _ => None,
    }
}

/// Carries out a command directive.
fn apply_command(sim: &mut Sim, c: ScriptCommand) {
    match c {
        ScriptCommand::EndLife => sim.end_life(),
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
    header: ScriptHeader,
    directives: Directives,
    live_span: Option<LiveSpan>,
    /// A replayed pause or menu in progress: the wall time left, µs.
    hold_us: Option<u64>,
    /// The help screen time the replayed hold in progress passes to the
    /// fuse when it ends.
    hold_menu_ms: Option<u32>,
    /// How keys count across a pause or a screen.
    pause_input: PauseInput,
    /// Buttons down right now, as the key events and
    /// [`Session::sync_keys`] say.
    down: Buttons,
    /// The tick after a resume waits for the next frame. It can stay set
    /// across a pause: when [`Session::pause`] cannot run that tick (a
    /// replayed pause or menu holds play at the boundary), the tick still
    /// waits, and the frame after the next resume runs it.
    stepping: bool,
    /// The input of that tick, when the resume decided it.
    resume_input: Option<TickInput>,
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
            header: ScriptHeader::default(),
            directives: Directives::default(),
            live_span: None,
            hold_us: None,
            hold_menu_ms: None,
            pause_input: PauseInput::default(),
            down: Buttons::NONE,
            stepping: false,
            resume_input: None,
        }
    }

    /// Plays with `rule` for keys across a pause or a screen (the original
    /// rule by default).
    pub fn with_pause_input(mut self, rule: PauseInput) -> Session {
        self.pause_input = rule;
        self
    }

    /// How keys count across a pause or a screen.
    pub fn pause_input(&self) -> PauseInput {
        self.pause_input
    }

    /// The buttons the input devices hold down now, for the tick that runs
    /// when play goes on (see the [module documentation](self)); the key
    /// events given since say the same unless a screen took the keys.
    pub fn sync_keys(&mut self, down: Buttons) {
        self.down = down;
    }

    /// Buttons down right now, as far as the session knows.
    pub fn keys_down(&self) -> Buttons {
        self.down
    }

    /// Plays `script` from the next tick: one row per tick, then the
    /// keyboard again, with its pauses, menus and commands at their tick
    /// boundaries (see the [module documentation](self)).
    pub fn with_replay(mut self, script: InputScript) -> Session {
        self.header = script.header;
        self.directives.plan = script.directives.clone();
        self.directives.next = 0;
        self.script = Some(script);
        self.looped = false;
        self
    }

    /// Plays the pauses, menus and commands of `directives` (in tick
    /// order) at their tick boundaries, with the input of each tick taken
    /// live: the same game as [`Session::with_replay`] when the live input
    /// is the script's.
    pub fn with_directives(mut self, directives: Vec<Directive>) -> Session {
        self.directives.plan = directives;
        self.directives.next = 0;
        self
    }

    /// Plays `script` from the next tick over and over, for benchmarks
    /// that run for a time rather than a number of ticks. Its directives
    /// are not played.
    pub fn with_looped_replay(mut self, script: InputScript) -> Session {
        self.script = Some(script);
        self.looped = true;
        self
    }

    /// The header keys of the replayed script (empty without one).
    pub fn header(&self) -> ScriptHeader {
        self.header
    }

    /// Every pause, menu and command played or given so far, in order,
    /// each at the tick it took effect before; a pause or menu still open
    /// counts with the wall time it has lasted so far.
    pub fn directives(&self) -> Vec<Directive> {
        let mut out = self.directives.log.clone();
        if let Some(s) = &self.live_span
            && let Some(d) = out.get_mut(s.index)
        {
            d.kind = s.kind();
        }
        out
    }

    /// Gives up Claw's life at the start of the next tick (the in-game
    /// menu's end life; see [`Sim::end_life`]), noted as a command.
    pub fn end_life(&mut self) {
        self.sim.end_life();
        self.directives.log.push(Directive {
            tick: self.ticks as usize,
            kind: DirectiveKind::Command(ScriptCommand::EndLife),
        });
    }

    /// A pause or a menu screen stops play from now on, until
    /// [`Session::close_span`]: noted at the boundary before the next tick,
    /// with the wall time of the frames that run until it closes. It only
    /// notes the stop; the caller stops the clock ([`Session::pause`]). A
    /// span still open is closed first.
    pub fn open_span(&mut self, span: Span) {
        self.close_span();
        let index = self.directives.log.len();
        let live = LiveSpan {
            index,
            span,
            wall_us: 0,
            keys: Vec::new(),
        };
        self.directives.log.push(Directive {
            tick: self.ticks as usize,
            kind: live.kind(),
        });
        self.live_span = Some(live);
    }

    /// A menu input while a menu span is open, noted by name: lowercase
    /// letters, digits and underscores (anything else is left out).
    pub fn menu_key(&mut self, name: &str) {
        let Some(s) = self.live_span.as_mut() else {
            return;
        };
        let ok = !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if ok && matches!(s.span, Span::Menu(_)) {
            s.keys.push(name.to_string());
        }
    }

    /// The pause or menu opened by [`Session::open_span`] ends.
    pub fn close_span(&mut self) {
        if let Some(s) = self.live_span.take()
            && let Some(d) = self.directives.log.get_mut(s.index)
        {
            d.kind = s.kind();
        }
    }

    /// True while a replayed pause or menu holds play.
    pub fn holding(&self) -> bool {
        self.hold_us.is_some()
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
        if self.stepping {
            // The tick after a resume has its input already and counts as
            // run: time goes on after it.
            return (self.ticks + 1) * TICK_US;
        }
        self.ticks * TICK_US + (self.driver.alpha() * TICK_US as f64).round() as u64
    }

    /// The simulation time of an event that happened `since_frame` after the
    /// last frame (no later than the longest frame the accumulator takes;
    /// no time passes while paused, nor while a replayed pause or menu
    /// holds play). Stamps never go backwards.
    pub fn stamp(&mut self, since_frame: Duration) -> u64 {
        let since = u64::try_from(since_frame.as_micros()).unwrap_or(u64::MAX);
        let at = if self.is_paused() {
            self.now_us()
        } else {
            self.sim_time_after(since.min(MAX_FRAME_US))
        };
        let at = at.max(self.last_stamp);
        self.last_stamp = at;
        at
    }

    /// The simulation time `wall_us` of frame time after the last frame:
    /// the time a replayed pause or menu holds play (the one in progress
    /// and those due on the way) does not count.
    fn sim_time_after(&self, wall_us: u64) -> u64 {
        let mut now = self.now_us();
        let mut left = wall_us.saturating_sub(self.hold_us.unwrap_or(0));
        let d = &self.directives;
        for next in &d.plan[d.next.min(d.plan.len())..] {
            let wall = match next.kind {
                DirectiveKind::Pause { wall_ms, .. } | DirectiveKind::Menu { wall_ms, .. } => {
                    u64::from(wall_ms) * 1000
                }
                DirectiveKind::Command(_) => continue,
            };
            let gap = (next.tick as u64 * TICK_US).saturating_sub(now);
            if left <= gap {
                break;
            }
            now += gap;
            left -= gap;
            left = left.saturating_sub(wall);
        }
        now + left
    }

    /// A key event at simulation time `at_us`: `buttons` went down or up.
    pub fn key(&mut self, at_us: u64, buttons: Buttons, down: bool) {
        let at = at_us.max(self.last_stamp);
        self.last_stamp = at;
        self.latch.event(at, buttons, down);
        if down {
            self.down |= buttons;
        } else {
            self.down = Buttons(self.down.0 & !buttons.0);
        }
    }

    /// Releases every held button at `at_us`, for example when the window
    /// loses focus.
    pub fn release_all(&mut self, at_us: u64) {
        let down = self.latch.down();
        if !down.is_empty() {
            self.key(at_us, down, false);
        }
    }

    /// Adds one frame of `dt` and runs the ticks that fit, holding play at
    /// the boundary of a replayed pause or menu for its wall time.
    pub fn frame(&mut self, dt: Duration) -> Frame {
        let mut left = u64::try_from(dt.as_micros()).unwrap_or(u64::MAX);
        if let Some(s) = self.live_span.as_mut() {
            s.wall_us = s.wall_us.saturating_add(left);
        }
        if self.paused {
            return self.run(left);
        }
        let mut ticks = 0;
        loop {
            if let Some(hold) = self.hold_us.as_mut() {
                let take = left.min(*hold);
                *hold -= take;
                left -= take;
                if *hold > 0 {
                    break;
                }
                self.hold_us = None;
                if let Some(ms) = self.hold_menu_ms.take() {
                    self.sim.pass_menu_time(ms);
                }
            }
            if self.start_due() {
                continue;
            }
            // Run no further than the next pause or menu's boundary.
            let feed = match self.directives.next_hold() {
                Some(at) => {
                    let room = at.saturating_sub(self.ticks) * TICK_US;
                    let room = room.saturating_sub(self.driver.step_mut().accumulator_us());
                    left.min(room.max(1))
                }
                None => left,
            };
            ticks += self.run(feed).ticks;
            left -= feed;
            if left == 0 {
                break;
            }
        }
        Frame {
            ticks,
            alpha: self.driver.alpha(),
        }
    }

    /// Plays the directives due at the boundary before the next tick:
    /// commands take effect, and a pause or menu starts holding play (true).
    fn start_due(&mut self) -> bool {
        let d = &mut self.directives;
        while let Some(next) = d.plan.get(d.next)
            && next.tick as u64 <= self.ticks
        {
            let next = next.clone();
            d.next += 1;
            let hold = match next.kind {
                DirectiveKind::Command(c) => {
                    apply_command(&mut self.sim, c);
                    None
                }
                DirectiveKind::Pause { wall_ms, .. } | DirectiveKind::Menu { wall_ms, .. } => {
                    Some(u64::from(wall_ms) * 1000)
                }
            };
            let menu_ms = menu_time(&next.kind);
            d.log.push(next);
            if let Some(us) = hold {
                self.hold_us = Some(us);
                self.hold_menu_ms = menu_ms;
                return true;
            }
        }
        false
    }

    /// Adds `us` microseconds to the frame driver and runs the ticks that
    /// fit.
    fn run(&mut self, us: u64) -> Frame {
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
            directives,
            resume_input,
            ..
        } = self;
        let report = driver.frame(Duration::from_micros(us), || {
            let n = *ticks;
            directives.commands_before(n, sim);
            let live = match resume_input.take() {
                Some(input) => input,
                None => latch.take(n),
            };
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
        if report.ticks > 0 {
            self.stepping = false;
        }
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

    /// Stops the clock: frames run no ticks until [`Session::resume`]. A
    /// tick a resume left for the next frame runs first, as a frame of no
    /// time runs it, so every resume runs its tick however soon play stops
    /// again. A replayed pause or menu due at that boundary, or still
    /// holding play, comes first as in any frame: the tick then waits for
    /// it, so a replay watched with pauses plays the same game as one
    /// played without them.
    pub fn pause(&mut self) {
        if self.paused {
            return;
        }
        if self.stepping {
            self.frame(Duration::ZERO);
        }
        self.driver.step_mut().pause();
        self.paused = true;
    }

    /// Restarts the clock. After a pause or a screen (a span still open,
    /// see [`Session::open_span`]) play goes on as the [module
    /// documentation](self) says: the help screen's time reaches the fuse,
    /// the next frame runs exactly one tick at once with the input of
    /// [`Session::pause_input`]'s rule, and real time counts from an empty
    /// accumulator after it. Without a span (the window was in the
    /// background) the clock just goes on.
    pub fn resume(&mut self) {
        if !self.paused {
            return;
        }
        self.driver.step_mut().resume();
        self.paused = false;
        let Some(span) = self.live_span.as_ref() else {
            return;
        };
        if let Span::Menu(MenuScreen::Help) = span.span {
            self.sim.pass_menu_time(span.wall_ms());
        }
        if self.pause_input == PauseInput::Original {
            let held = self.down;
            let pressed = match span.span {
                // Pressed: down now and not held on the last tick before
                // the pause.
                Span::Pause => Buttons(held.0 & !self.sim.input().held.0),
                // After a screen every key down counts as pressed already.
                Span::Menu(_) => Buttons::NONE,
            };
            self.resume_input = Some(TickInput { held, pressed });
            // The key events while stopped are not taken one by one: the
            // latch goes on from the keys down now.
            self.latch = InputLatch::new();
            if !held.is_empty() {
                self.latch.event(0, held, true);
                self.latch.take(0);
            }
        }
        self.driver.step_mut().step_once();
        self.stepping = true;
    }

    /// True while paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }
}

/// The key of the mid-air hit jump fix in an input script's comment line,
/// as in the config file.
pub const BEHAVIOUR_KEY: &str = "mid_air_hit_fix";

/// The comment line of an input script that records the mid-air hit jump
/// fix it was played with: `# mid_air_hit_fix = on` (or `off`).
pub fn behaviour_comment(fix: bool) -> String {
    format!("# {BEHAVIOUR_KEY} = {}", if fix { "on" } else { "off" })
}

/// The mid-air hit jump fix that script `text` says it was played with,
/// from its `# mid_air_hit_fix = on|off` comment line (the last one counts);
/// `None` when it has none.
pub fn script_behaviour(text: &str) -> Option<bool> {
    text.lines().rev().find_map(|l| {
        let (k, v) = l.trim().strip_prefix('#')?.split_once('=')?;
        if k.trim() != BEHAVIOUR_KEY {
            return None;
        }
        crate::settings::parse_on_off(v).ok()
    })
}

/// `script` as CSV, led by the comment line of the mid-air hit jump fix
/// `fix`.
pub fn recording_csv(script: &InputScript, fix: bool) -> String {
    format!("{}\n{}", behaviour_comment(fix), script.to_csv())
}

/// The key of the pause input switch in a recording's comment lines, as in
/// the config file.
pub const PAUSE_INPUT_KEY: &str = "pause_input";

/// The key of the fuse clock switch in a recording's comment lines, as in
/// the config file.
pub const FUSE_CLOCK_KEY: &str = "fuse_clock";

/// The behaviour switches of the pause and screens a recording was played
/// with, from its comment lines; `None` for a line it does not have.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScriptSwitches {
    /// How keys counted across a pause or a screen.
    pub pause_input: Option<PauseInput>,
    /// Whether help screen time reached the dynamite fuse.
    pub fuse_clock: Option<FuseClock>,
}

/// The comment lines of the pause and screen switches of a recording:
/// `# pause_input = original` and `# fuse_clock = original` (or the other
/// values).
pub fn switch_comments(pause_input: PauseInput, fuse_clock: FuseClock) -> String {
    format!(
        "# {PAUSE_INPUT_KEY} = {}\n# {FUSE_CLOCK_KEY} = {}",
        pause_input.name(),
        fuse_clock.name()
    )
}

/// The switches script `text` says it was played with (the last line of
/// each counts); lines with a value this game does not know are left out.
pub fn script_switches(text: &str) -> ScriptSwitches {
    let mut out = ScriptSwitches::default();
    for l in text.lines() {
        let Some((k, v)) = l.trim().strip_prefix('#').and_then(|r| r.split_once('=')) else {
            continue;
        };
        let v = v.trim().trim_matches('"');
        match k.trim() {
            PAUSE_INPUT_KEY => {
                if let Some(p) = PauseInput::from_name(v) {
                    out.pause_input = Some(p);
                }
            }
            FUSE_CLOCK_KEY => {
                if let Some(f) = FuseClock::from_name(v) {
                    out.fuse_clock = Some(f);
                }
            }
            _ => {}
        }
    }
    out
}

/// `script` as CSV, led by the comment lines of every behaviour switch:
/// the mid-air hit jump fix `fix` first (see [`recording_csv`]), then the
/// pause and screen switches (see [`switch_comments`]).
pub fn recording_text(
    script: &InputScript,
    fix: bool,
    pause_input: PauseInput,
    fuse_clock: FuseClock,
) -> String {
    format!(
        "{}\n{}\n{}",
        behaviour_comment(fix),
        switch_comments(pause_input, fuse_clock),
        script.to_csv()
    )
}

/// Writes what `recorder` kept of `session` to `path` as an input script,
/// led by the comment lines of the behaviour switches the session plays
/// with (see [`recording_text`]); returns the ticks written.
pub fn save_recording(recorder: &Recorder, session: &Session, path: &Path) -> Result<usize> {
    let script = recorder.script(session);
    let cfg = session.sim().config();
    let text = recording_text(
        &script,
        cfg.tuning.behaviour.fix_double_jump,
        session.pause_input(),
        cfg.fuse_clock,
    );
    std::fs::write(path, text)
        .with_context(|| format!("writing the recording {}", path.display()))?;
    Ok(script.ticks.len())
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
    fn commands_stop_at_the_next_pause_or_menu() {
        let mut sim = Sim::new(level(), SimConfig::default());
        let at = |tick, kind| Directive { tick, kind };
        let mut d = Directives {
            plan: vec![
                at(2, DirectiveKind::Command(ScriptCommand::EndLife)),
                at(
                    3,
                    DirectiveKind::Pause {
                        wall_ms: 100,
                        held: None,
                    },
                ),
                at(3, DirectiveKind::Command(ScriptCommand::EndLife)),
            ],
            ..Directives::default()
        };
        d.commands_before(5, &mut sim);
        assert_eq!(d.next, 1, "the pause is left for the frame to hold");
        assert_eq!(d.log.len(), 1);
        assert_eq!(d.next_hold(), Some(3));
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
