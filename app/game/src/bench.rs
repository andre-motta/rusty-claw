//! Benchmarks: Claw on a fixed route through a level while every frame is
//! measured.
//!
//! The route ([`combat_route`]) starts Claw just before the stretch of the
//! level where the most enemies stand together, found from the level's own
//! records, and walks him through it: he runs on, strikes, fires the pistol
//! and jumps, so enemies fight back, shots and bursts fly and pickups drop
//! while frames are measured. A level with no enemies gets the plain
//! movement route ([`route`]: run right with jumps, run back, crouch and
//! look up) from its start. Two ways to run it:
//!
//! - in a window ([`crate::window`]): real presentation at the display's
//!   refresh (or at the frame-rate cap with vsync off), which also counts
//!   frames that missed their refresh;
//! - headless ([`run_headless`]): offscreen at `--res` with a simulated
//!   display clock, waiting for the GPU after each frame, which also
//!   measures GPU time. Frames are paced to the simulated display rate, so
//!   the run draws as many frames as that display would; `--uncapped`
//!   draws as fast as the machine allows, for throughput runs. Either way
//!   the work of a frame is timed around the frame alone, never the wait.
//!
//! The route is played once, not looped (each pass would meet a different
//! level); it is long enough for a run paced at the display rate. The
//! report records its length and the ticks run past its end
//! (`route.ticks_past_route`, normally 0), and the verdict line says when
//! the route ran out, as an unpaced run on a fast machine can. Every frame
//! is drawn as in play, with the status bar, messages and screen
//! transitions, and their cost is part of the measured work. The headless
//! run also plays the game's sounds and music through a silent output (the
//! mixer and the synthesizer run as with a device), and that work is part
//! of each frame too.
//!
//! The report is one JSON object. Times are milliseconds, each with `p50`,
//! `p99` and `max`:
//!
//! - `frame_work_ms`: CPU work of a frame: input, ticks, interpolation,
//!   scene building and command submission (not the wait for a free
//!   swapchain image);
//! - `tick_frame_work_ms`: the same for frames that ran at least one tick;
//! - `tick_ms`: one simulation tick alone, with `live_objects` the number
//!   of objects in the live list (Claw and the camera included);
//! - `deck_proxy_ms`: `tick_frame_work_ms.p99` times [`DECK_FACTOR`], an
//!   estimate of the same work on a handheld-class CPU;
//! - window runs: `acquire_ms` (waiting for a swapchain image),
//!   `present_interval_ms` and `missed_vblanks` (presents more than 1.5
//!   refresh periods apart) after a one-second warm-up; when the desktop
//!   cannot hold a window of the requested size, every frame is also drawn
//!   offscreen at that size and counted in the frame work
//!   (`offscreen_copy`); `missed_vblank_detail` describes the frame of each
//!   missed refresh (see [`PresentDetail`]), to tell a slow frame of the
//!   game from a stall outside it;
//! - headless runs: `gpu_ms` and `frame_ms` (work plus GPU), and `audio`:
//!   the silent output, the sounds and music tracks loaded and the cues
//!   played;
//! - `combat`: where the route starts, the enemies killed and the most
//!   shots and enemies live at once.
//!
//! `budget` checks the targets: frame work p99 under [`FRAME_BUDGET_MS`]
//! (a 144 Hz refresh), the handheld estimate under [`DECK_BUDGET_MS`] (half
//! a 60 Hz refresh) and no missed refresh. Missed refreshes can only be
//! measured in a window with vsync on; elsewhere they are reported as not
//! measurable, never as passed. [`Report::summary`] is the one-line PASS
//! or FAIL verdict naming each budget.

use std::fmt::Write as _;
use std::time::Instant;

use anyhow::{Context, Result};
use claw_frame::{Buttons, FramePacer, SimulatedRefresh};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::{FrameInput, Gpu, Offscreen, Renderer, SceneBuilder};
use claw_sim::logic::enemy;
use claw_sim::object::LogicKind;
use claw_sim::tiles::{GROUND, SOLID};
use claw_sim::{GameLogic, InputScript, Sim, SimLevel};

use crate::audio::{AudioBridge, ViewInfo, load_soundfont};
use crate::load::{load_graphics, load_level, new_sim, preload};
use crate::session::{Session, TickSample};
use crate::settings::Settings;
use crate::window::StatusLayer;

/// Per-frame CPU time on a handheld-class CPU relative to the machine the
/// benchmark runs on (a planning factor, not a measurement).
pub const DECK_FACTOR: f64 = 3.0;

/// Frame work budget: one refresh at 144 Hz, ms.
pub const FRAME_BUDGET_MS: f64 = 6.9;

/// Budget of the handheld estimate: half a 60 Hz refresh, ms.
pub const DECK_BUDGET_MS: f64 = 8.3;

/// Seconds at the start of a window run left out of the present
/// statistics (window mapping and the first swapchain images).
pub const WARMUP_S: f64 = 1.0;

/// A present more than this many refresh periods after the previous one
/// missed a refresh.
pub const MISS_FACTOR: f64 = 1.5;

/// The benchmark route as a script of at least `seconds`.
pub fn route(seconds: f64) -> InputScript {
    let (left, right, up, down, jump) = (
        Buttons::LEFT.0,
        Buttons::RIGHT.0,
        Buttons::UP.0,
        Buttons::DOWN.0,
        Buttons::JUMP.0,
    );
    // One cycle, 18 ms per entry: (mask, ticks).
    let mut cycle: Vec<(u32, usize)> = Vec::new();
    for dir in [right, left] {
        for _ in 0..4 {
            cycle.push((dir, 40));
            cycle.push((dir | jump, 25));
        }
        cycle.push((0, 20));
    }
    cycle.extend([(down, 60), (0, 20), (up, 60), (0, 20), (jump, 1), (0, 40)]);
    let per_cycle: usize = cycle.iter().map(|&(_, n)| n).sum();
    let want = (seconds.max(0.0) * 1000.0 / 18.0).ceil() as usize + 1;
    let mut held = Vec::with_capacity(want + per_cycle);
    while held.len() < want {
        for &(mask, n) in &cycle {
            held.extend(std::iter::repeat_n(mask, n));
        }
    }
    InputScript::from_held(&held)
}

/// The benchmark route of a level: the input Claw plays and the moves
/// that start its segments.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchRoute {
    /// The input, one row per tick.
    pub script: InputScript,
    /// Claw is moved to (x, y) before the tick of that index: the first
    /// move puts him at the start of the stretch, later ones start a fight
    /// he could not walk to.
    pub teleports: Vec<(u64, i32, i32)>,
    /// What the route is: `combat` or `movement`.
    pub kind: &'static str,
}

impl BenchRoute {
    /// Where the route starts, if it moves Claw first.
    pub fn start(&self) -> Option<(i32, i32)> {
        self.teleports
            .first()
            .filter(|t| t.0 == 0)
            .map(|&(_, x, y)| (x, y))
    }

    /// A hook for [`Session::set_before_tick`] that makes the route's
    /// moves.
    pub fn mover(&self) -> crate::session::BeforeTick {
        let moves = self.teleports.clone();
        let mut next = 0;
        Box::new(move |tick, sim: &mut Sim| {
            while let Some(&(at, x, y)) = moves.get(next) {
                if at > tick {
                    break;
                }
                if at == tick {
                    sim.teleport_player(x, y);
                }
                next += 1;
            }
        })
    }
}

/// Width of the stretch of a level the combat route starts at, px.
pub const COMBAT_STRETCH: i32 = 2400;

/// How far above or below the first enemy of a stretch the others may
/// stand to count, px: about the same floor.
pub const COMBAT_FLOOR_SPREAD: i32 = 120;

/// Ticks Claw tries to reach and beat one enemy (each time).
const FIGHT_TICKS: usize = 250;

/// The officers, soldiers and rats of `level` (x, y, z), sorted by x.
fn enemies_of(level: &SimLevel) -> Vec<(i32, i32, i32)> {
    let mut enemies: Vec<(i32, i32, i32)> = level
        .objects
        .iter()
        .filter(|o| matches!(o.logic.as_str(), "Officer" | "Soldier" | "Rat"))
        .map(|o| (o.record[5], o.record[6], o.record[7]))
        .collect();
    enemies.sort_unstable();
    enemies
}

/// The combat route of the game `sim` (as it stands before the run), long
/// enough for `seconds` of play at the game's own pace.
///
/// It is planned before the run by playing a copy of the game. Unless
/// `here` (start where Claw stands), Claw is first moved to the floor
/// 150 px before the first enemy of the stretch of
/// [`COMBAT_STRETCH`] px with the most officers, soldiers and rats on about
/// the same floor (within [`COMBAT_FLOOR_SPREAD`] px of the first), then
/// takes on the enemies from there on, in order: he walks to each (jumping
/// when a step stops him) and strikes it, crouching against a rat; every
/// third one he shoots with the pistol from where he stands. An enemy he
/// has not beaten within a few seconds is fought again after a move next to
/// it, then given up. When the enemies run out, the movement [`route`]
/// fills the rest. The input and the moves are recorded, so the run plays
/// the same game. A level with no enemies gets the plain movement route
/// from its start.
pub fn combat_route(sim: &Sim, seconds: f64, here: bool) -> BenchRoute {
    let level = sim.level();
    let enemies = enemies_of(level);
    let mut stretches: Vec<(usize, i32, i32)> = enemies
        .iter()
        .map(|&(x, y, _)| {
            let n = enemies
                .iter()
                .filter(|&&(ex, ey, _)| {
                    ex >= x && ex - x < COMBAT_STRETCH && (ey - y).abs() <= COMBAT_FLOOR_SPREAD
                })
                .count();
            (n, x, y)
        })
        .collect();
    stretches.sort_by_key(|&(n, x, _)| (std::cmp::Reverse(n), x));
    let start = if here {
        sim.player().map(|c| (c.x, c.y))
    } else {
        stretches.into_iter().find_map(|(_, x, y)| {
            let x = x - 150;
            floor_below(level, x, y - 100, 250).map(|floor| (x, floor - 64))
        })
    };
    let ticks = (seconds.max(0.0) * 1000.0 / 18.0).ceil() as usize + 1;
    let Some(start) = start else {
        return BenchRoute {
            script: route(seconds),
            teleports: Vec::new(),
            kind: "movement",
        };
    };
    let mut plan = Plan {
        sim: sim.clone(),
        held: Vec::with_capacity(ticks),
        teleports: Vec::new(),
        prev: 0,
    };
    if !here {
        plan.teleport(start.0, start.1);
    }
    let foes: Vec<(i32, i32, i32)> = enemies
        .into_iter()
        .filter(|e| e.0 >= start.0 - 100)
        .collect();
    for (i, &foe) in foes.iter().cycle().enumerate() {
        if plan.held.len() >= ticks || i >= foes.len() * 2 {
            break;
        }
        plan.fight(foe, i % 3 == 2, ticks);
    }
    // Enemies left behind or done: the movement route for the rest.
    let rest = route(seconds);
    let mut k = 0;
    while plan.held.len() < ticks {
        let held = rest.ticks.get(k).map_or(0, |t| t.held.0);
        plan.step(held);
        k += 1;
    }
    BenchRoute {
        script: InputScript::from_held(&plan.held),
        teleports: plan.teleports,
        kind: "combat",
    }
}

/// A combat route being planned on a copy of the game.
struct Plan {
    sim: Sim,
    held: Vec<u32>,
    teleports: Vec<(u64, i32, i32)>,
    prev: u32,
}

impl Plan {
    fn step(&mut self, held: u32) {
        let pressed = held & !self.prev;
        self.prev = held;
        self.held.push(held);
        self.sim.step(claw_frame::TickInput {
            held: Buttons(held),
            pressed: Buttons(pressed),
        });
    }

    fn teleport(&mut self, x: i32, y: i32) {
        self.teleports.push((self.held.len() as u64, x, y));
        self.sim.teleport_player(x, y);
    }

    /// The enemy placed at `placed`: its position, whether it is a rat,
    /// and whether it is still to be beaten.
    fn foe(&self, placed: (i32, i32, i32)) -> Option<(i32, i32, bool)> {
        self.sim
            .objects()
            .all()
            .find(|(_, o)| {
                o.editor == placed
                    && matches!(
                        o.kind,
                        LogicKind::Game(GameLogic::Officer | GameLogic::Soldier | GameLogic::Rat)
                    )
                    && enemy::data_of(o).is_none_or(|d| !d.dead)
            })
            .map(|(_, o)| (o.x, o.y, &*o.logic == "Rat"))
    }

    /// Walks to the enemy placed at `placed` and fights it (with the pistol
    /// when `shoot`). When it stands on another floor, or Claw makes no
    /// headway, he is moved next to it once (onto its floor).
    fn fight(&mut self, placed: (i32, i32, i32), shoot: bool, ticks: usize) {
        for round in 0..2 {
            if round == 1 {
                let Some((x, y, _)) = self.foe(placed) else {
                    return;
                };
                let side = if self.sim.player().is_some_and(|c| c.x < x) {
                    -70
                } else {
                    70
                };
                let Some(floor) = floor_below(self.sim.level(), x + side, y - 60, 200) else {
                    return;
                };
                self.teleport(x + side, floor - 64);
            }
            let (mut last_x, mut stuck, mut jump, mut since) = (i32::MIN, 0, 0, 10);
            let mut blows = 0;
            for _ in 0..FIGHT_TICKS {
                if self.held.len() >= ticks {
                    return;
                }
                let Some((x, y, rat)) = self.foe(placed) else {
                    return;
                };
                let Some(c) = self.sim.player() else { return };
                let (dx, dy) = (x - c.x, y - c.y);
                // On another floor, or no headway: move next to it, or
                // after that give it up.
                if dy.abs() > 40 || stuck > 60 || blows > 12 {
                    break;
                }
                let toward = if dx > 0 {
                    Buttons::RIGHT.0
                } else {
                    Buttons::LEFT.0
                };
                let facing = self.sim.claw().facing_right == (dx > 0);
                let busy = self.sim.claw().attacking;
                let reach = if shoot { 300 } else { 55 };
                stuck = if c.x == last_x { stuck + 1 } else { 0 };
                last_x = c.x;
                let low = if rat { Buttons::DOWN.0 } else { 0 };
                let held = if (dx.abs() > reach || !facing) && !busy {
                    if stuck > 12 && jump == 0 {
                        jump = 30;
                    }
                    since = 10;
                    toward
                } else if !busy && since > 2 {
                    stuck = 0;
                    since = 0;
                    blows += 1;
                    low | if shoot {
                        Buttons::PROJECTILE.0
                    } else {
                        Buttons::ATTACK.0
                    }
                } else {
                    stuck = 0;
                    since += 1;
                    low
                };
                let held = if jump > 0 {
                    jump -= 1;
                    held | if jump > 8 { Buttons::JUMP.0 } else { 0 }
                } else {
                    held
                };
                self.step(held);
            }
        }
    }
}

/// The floor under (x, y): the lowest clear row above ground or solid
/// tiles, searching `depth` px down.
fn floor_below(level: &SimLevel, x: i32, y: i32, depth: i32) -> Option<i32> {
    let map = &level.map;
    (y..y + depth)
        .find(|&yy| matches!(map.attr(x, yy + 1), SOLID | GROUND) && map.attr(x, yy) != SOLID)
}

/// What the combat route met: enemies killed and the most shots and enemies
/// live at once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Combat {
    /// Enemies dead by the end of the run.
    pub enemies_killed: usize,
    /// Most enemies live in one tick.
    pub enemies_live_max: usize,
    /// Most shots and thrown objects live in one tick.
    pub shots_live_max: usize,
}

impl Combat {
    /// Notes what is live in `sim` now.
    pub fn look(&mut self, sim: &Sim) {
        let (mut enemies, mut shots) = (0, 0);
        for id in sim.objects().listed() {
            let Some(o) = sim.objects().get(id) else {
                continue;
            };
            match o.kind {
                LogicKind::Game(GameLogic::Officer | GameLogic::Soldier | GameLogic::Rat)
                    if enemy::data_of(o).is_some_and(|d| !d.dead) =>
                {
                    enemies += 1;
                }
                LogicKind::Game(GameLogic::Projectile) => shots += 1,
                _ => {}
            }
        }
        self.enemies_live_max = self.enemies_live_max.max(enemies);
        self.shots_live_max = self.shots_live_max.max(shots);
        self.enemies_killed = sim
            .objects()
            .all()
            .filter(|(_, o)| {
                matches!(
                    o.kind,
                    LogicKind::Game(GameLogic::Officer | GameLogic::Soldier | GameLogic::Rat)
                ) && enemy::data_of(o).is_some_and(|d| d.dead)
            })
            .count()
            .max(self.enemies_killed);
    }
}

/// The audio of a headless run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioRun {
    /// Sounds decoded into the bank.
    pub sounds: usize,
    /// Music tracks loaded (0 without a SoundFont).
    pub music_tracks: usize,
    /// Cues played.
    pub cues: usize,
}

/// p50, p99 and max of a sample.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    /// Median.
    pub p50: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Largest value.
    pub max: f64,
    /// Number of values.
    pub n: usize,
}

impl Summary {
    /// Summarizes `values` (nearest-rank percentiles).
    pub fn of(values: &[f64]) -> Summary {
        let mut v = values.to_vec();
        v.sort_by(f64::total_cmp);
        let at = |q: f64| {
            if v.is_empty() {
                return 0.0;
            }
            let rank = (q * v.len() as f64).ceil() as usize;
            v[rank.clamp(1, v.len()) - 1]
        };
        Summary {
            p50: at(0.5),
            p99: at(0.99),
            max: v.last().copied().unwrap_or(0.0),
            n: v.len(),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4}, \"n\": {}}}",
            self.p50, self.p99, self.max, self.n
        )
    }
}

/// What happened in the frame of one present of a window run, to tell why
/// a refresh was missed.
///
/// The time between two presents is `gap_ms` (from the previous present to
/// the start of this frame's work: event handling and any pacing wait),
/// then `work_ms`, `acquire_ms` and `present_call_ms`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PresentDetail {
    /// From the previous present to the start of this frame's work, ms.
    pub gap_ms: f64,
    /// CPU work of the frame, ms.
    pub work_ms: f64,
    /// Waiting for a swapchain image, ms.
    pub acquire_ms: f64,
    /// The present call itself, ms.
    pub present_call_ms: f64,
    /// Ticks the frame ran.
    pub ticks: u32,
    /// Sprite sets read and decoded while this frame was drawn.
    pub sets_loaded: usize,
    /// Images added to the image store while this frame was drawn.
    pub images_loaded: usize,
    /// Window events since the previous present that reallocate GPU or
    /// drawing resources: `resize`, `view rebuilt`, `surface reconfigured`,
    /// `frame skipped`.
    pub events: Vec<&'static str>,
}

impl PresentDetail {
    /// The most likely reason a present came late, for a display period of
    /// `period_ms`.
    pub fn likely_cause(&self, period_ms: f64) -> &'static str {
        if self.sets_loaded > 0 || self.images_loaded > 0 {
            "sprite loading in the game"
        } else if !self.events.is_empty() {
            "window change in the game"
        } else if self.work_ms > 0.5 * period_ms {
            "frame work in the game"
        } else if self.acquire_ms + self.present_call_ms >= self.gap_ms {
            "waiting on the display (compositor or driver)"
        } else {
            "between frames (compositor frame callback or event loop)"
        }
    }

    fn json(&self, at_s: f64, interval_ms: f64, period_ms: f64) -> String {
        format!(
            "{{\"at_s\": {at_s:.3}, \"interval_ms\": {interval_ms:.3}, \"gap_ms\": {:.3}, \
             \"work_ms\": {:.3}, \"acquire_ms\": {:.3}, \"present_call_ms\": {:.3}, \
             \"ticks\": {}, \"sets_loaded\": {}, \"images_loaded\": {}, \"events\": [{}], \
             \"likely_cause\": \"{}\"}}",
            self.gap_ms,
            self.work_ms,
            self.acquire_ms,
            self.present_call_ms,
            self.ticks,
            self.sets_loaded,
            self.images_loaded,
            self.events
                .iter()
                .map(|e| format!("\"{e}\""))
                .collect::<Vec<_>>()
                .join(", "),
            self.likely_cause(period_ms),
        )
    }
}

/// Raw measurements of a run.
#[derive(Debug, Clone, Default)]
pub struct Samples {
    /// CPU work per frame, ms.
    pub work_ms: Vec<f64>,
    /// CPU work of frames that ran a tick, ms.
    pub tick_work_ms: Vec<f64>,
    /// Waiting for a swapchain image, ms (window runs).
    pub acquire_ms: Vec<f64>,
    /// GPU time after submission, ms (headless runs).
    pub gpu_ms: Vec<f64>,
    /// Intervals between presents after the warm-up, ms (window runs).
    pub present_ms: Vec<f64>,
    /// When each interval ended, s after the warm-up (window runs).
    pub present_at_s: Vec<f64>,
    /// The frame that ended each interval (window runs).
    pub present_detail: Vec<PresentDetail>,
    /// Window size changes after the warm-up (window runs).
    pub resizes: usize,
    /// Frames drawn.
    pub frames: usize,
}

impl Samples {
    /// Records one frame's work.
    pub fn frame(&mut self, work_ms: f64, ticks: u32) {
        self.frames += 1;
        self.work_ms.push(work_ms);
        if ticks > 0 {
            self.tick_work_ms.push(work_ms);
        }
    }
}

/// The outcome of one budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Measured and inside the budget.
    Pass,
    /// Measured and over the budget.
    Fail,
    /// Not measurable in this kind of run.
    NotMeasurable,
}

/// One budget of a benchmark and its outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetCheck {
    /// Budget name.
    pub name: &'static str,
    /// Outcome.
    pub verdict: Verdict,
    /// Measured value against the budget, or why it was not measured.
    pub text: String,
}

/// A benchmark result.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// "window" or "headless".
    pub mode: &'static str,
    /// Level name.
    pub level: String,
    /// Requested output size.
    pub res: (u32, u32),
    /// Output size actually drawn.
    pub output: (u32, u32),
    /// Window runs: size of the offscreen copy drawn with every frame when
    /// the window could not get the requested size.
    pub offscreen_copy: Option<(u32, u32)>,
    /// Presentation view in game pixels.
    pub presentation: (u32, u32),
    /// Whole scale of the composition.
    pub scale: u32,
    /// GPU and driver.
    pub adapter: String,
    /// Presentation mode of a window run.
    pub present_mode: String,
    /// What paced the frames: the display refresh, a frame-rate cap, the
    /// simulated display, or nothing.
    pub pacing: String,
    /// Display refresh rate (reported by the monitor, or simulated).
    pub display_hz: f64,
    /// Where `display_hz` came from.
    pub display_hz_source: &'static str,
    /// Length of the measured run, s.
    pub seconds: f64,
    /// Ticks run.
    pub ticks: u64,
    /// Activation mode.
    pub activation: &'static str,
    /// Images loaded when the run started.
    pub images: usize,
    /// Sprite sets loaded before the first frame.
    pub preloaded_sets: usize,
    /// Images loaded during the run (should be 0).
    pub images_loaded_during_run: usize,
    /// Load time up to the first drawn frame, ms.
    pub load_ms: f64,
    /// What the route is: `combat` or `movement`.
    pub route_kind: &'static str,
    /// Where the route starts, `None` for the level start.
    pub route_start: Option<(i32, i32)>,
    /// Length of the route, in ticks.
    pub route_ticks: usize,
    /// Ticks run after the route ran out (should be 0; an unpaced run can
    /// outrun it).
    pub ticks_past_route: u64,
    /// What the route met.
    pub combat: Combat,
    /// The audio of a headless run.
    pub audio: Option<AudioRun>,
    /// Frame measurements.
    pub samples: Samples,
    /// Tick measurements.
    pub ticks_measured: Vec<TickSample>,
}

impl Report {
    /// Frame work summary.
    pub fn frame_work(&self) -> Summary {
        Summary::of(&self.samples.work_ms)
    }

    /// Work of frames that ran a tick.
    pub fn tick_frame_work(&self) -> Summary {
        Summary::of(&self.samples.tick_work_ms)
    }

    /// The handheld estimate, ms.
    pub fn deck_proxy_ms(&self) -> f64 {
        self.tick_frame_work().p99 * DECK_FACTOR
    }

    /// Tick cost summary.
    pub fn tick_cost(&self) -> Summary {
        let v: Vec<f64> = self.ticks_measured.iter().map(|t| t.ms).collect();
        Summary::of(&v)
    }

    /// Mean and largest live-object count per tick.
    pub fn live_objects(&self) -> (f64, u32) {
        let n = self.ticks_measured.len().max(1) as f64;
        let sum: u64 = self.ticks_measured.iter().map(|t| u64::from(t.live)).sum();
        let max = self
            .ticks_measured
            .iter()
            .map(|t| t.live)
            .max()
            .unwrap_or(0);
        (sum as f64 / n, max)
    }

    /// Presents more than [`MISS_FACTOR`] refresh periods apart (window
    /// runs).
    pub fn missed_vblanks(&self) -> usize {
        self.missed_vblank_indices().len()
    }

    fn missed_vblank_indices(&self) -> Vec<usize> {
        if self.display_hz <= 0.0 {
            return Vec::new();
        }
        let period = 1000.0 / self.display_hz;
        self.samples
            .present_ms
            .iter()
            .enumerate()
            .filter(|&(_, &d)| d > MISS_FACTOR * period)
            .map(|(i, _)| i)
            .collect()
    }

    /// When each missed refresh happened, s after the warm-up.
    pub fn missed_vblank_times(&self) -> Vec<f64> {
        self.missed_vblank_indices()
            .into_iter()
            .map(|i| self.samples.present_at_s.get(i).copied().unwrap_or(-1.0))
            .collect()
    }

    /// Why missed refreshes cannot be measured in this run, or `None` when
    /// they can: only a window run waiting for a display of known rate
    /// shows them.
    pub fn missed_vblanks_unmeasurable(&self) -> Option<&'static str> {
        if self.mode != "window" {
            Some("offscreen")
        } else if !matches!(self.present_mode.as_str(), "Fifo" | "FifoRelaxed") {
            Some("without vsync")
        } else if self.display_hz <= 0.0 {
            Some("at an unknown display rate")
        } else {
            None
        }
    }

    /// Each budget with its outcome.
    pub fn checks(&self) -> Vec<BudgetCheck> {
        let verdict = |ok: bool| if ok { Verdict::Pass } else { Verdict::Fail };
        let mut out = vec![
            BudgetCheck {
                name: "frame work p99",
                verdict: verdict(self.work_within_budget()),
                text: format!(
                    "{:.3} ms (under {FRAME_BUDGET_MS} ms)",
                    self.frame_work().p99
                ),
            },
            BudgetCheck {
                name: "deck proxy",
                verdict: verdict(self.deck_within_budget()),
                text: format!("{:.3} ms (under {DECK_BUDGET_MS} ms)", self.deck_proxy_ms()),
            },
        ];
        out.push(match self.missed_vblanks_unmeasurable() {
            Some(why) => BudgetCheck {
                name: "missed vblanks",
                verdict: Verdict::NotMeasurable,
                text: format!("not measurable {why}"),
            },
            None => {
                let times = self.missed_vblank_times();
                let mut text = format!("{} at {:.2} Hz (want 0)", times.len(), self.display_hz);
                if !times.is_empty() {
                    let shown: Vec<String> =
                        times.iter().take(8).map(|t| format!("{t:.2}")).collect();
                    let more = if times.len() > 8 { ", ..." } else { "" };
                    let _ = write!(text, " at {}{more} s", shown.join(", "));
                }
                BudgetCheck {
                    name: "missed vblanks",
                    verdict: verdict(times.is_empty()),
                    text,
                }
            }
        });
        out
    }

    /// True when no measured budget failed.
    pub fn passed(&self) -> bool {
        self.checks().iter().all(|c| c.verdict != Verdict::Fail)
    }

    /// One line: PASS or FAIL, then each budget with its outcome.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .checks()
            .iter()
            .map(|c| {
                let tag = match c.verdict {
                    Verdict::Pass => " ok",
                    Verdict::Fail => " FAIL",
                    Verdict::NotMeasurable => "",
                };
                format!("{} {}{tag}", c.name, c.text)
            })
            .collect();
        let mut line = format!(
            "bench {}: {}",
            if self.passed() { "PASS" } else { "FAIL" },
            parts.join("; ")
        );
        if self.ticks_past_route > 0 {
            let _ = write!(
                line,
                "; the route ran out after {} ticks, {} ticks ran without it",
                self.route_ticks, self.ticks_past_route
            );
        }
        line
    }

    /// Frames per second over the run.
    pub fn fps(&self) -> f64 {
        if self.seconds > 0.0 {
            self.samples.frames as f64 / self.seconds
        } else {
            0.0
        }
    }

    /// Frame work p99 inside the frame budget.
    pub fn work_within_budget(&self) -> bool {
        self.frame_work().p99 < FRAME_BUDGET_MS
    }

    /// The handheld estimate inside its budget.
    pub fn deck_within_budget(&self) -> bool {
        self.deck_proxy_ms() < DECK_BUDGET_MS
    }

    /// The report as one line of JSON.
    pub fn to_json(&self) -> String {
        let (live_mean, live_max) = self.live_objects();
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\"mode\": \"{}\", \"level\": {}, \"res\": \"{}x{}\", \"output\": \"{}x{}\", \
             \"presentation\": \"{}x{}\", \"scale\": {}, \"adapter\": {}, ",
            self.mode,
            json_string(&self.level),
            self.res.0,
            self.res.1,
            self.output.0,
            self.output.1,
            self.presentation.0,
            self.presentation.1,
            self.scale,
            json_string(&self.adapter),
        );
        if let Some((w, h)) = self.offscreen_copy {
            let _ = write!(s, "\"offscreen_copy\": \"{w}x{h}\", ");
        }
        if !self.present_mode.is_empty() {
            let _ = write!(s, "\"present_mode\": \"{}\", ", self.present_mode);
        }
        let start = match self.route_start {
            Some((x, y)) => format!("[{x}, {y}]"),
            None => "null".to_string(),
        };
        let _ = write!(
            s,
            "\"pacing\": {}, \"route\": {{\"kind\": \"{}\", \"start\": {start}, \
             \"ticks\": {}, \"ticks_past_route\": {}, \"ran_out\": {}}}, \
             \"combat\": {{\"enemies_killed\": {}, \"enemies_live_max\": {}, \
             \"shots_live_max\": {}}}, ",
            json_string(&self.pacing),
            self.route_kind,
            self.route_ticks,
            self.ticks_past_route,
            self.ticks_past_route > 0,
            self.combat.enemies_killed,
            self.combat.enemies_live_max,
            self.combat.shots_live_max,
        );
        if let Some(a) = &self.audio {
            let _ = write!(
                s,
                "\"audio\": {{\"output\": \"silent\", \"sounds\": {}, \"music_tracks\": {}, \
                 \"cues\": {}}}, ",
                a.sounds, a.music_tracks, a.cues
            );
        }
        let _ = write!(
            s,
            "\"display_hz\": {:.2}, \"display_hz_source\": \"{}\", \"activation\": \"{}\", \
             \"seconds\": {:.2}, \"frames\": {}, \"fps\": {:.1}, \"ticks\": {}, \
             \"images\": {}, \"preloaded_sets\": {}, \"images_loaded_during_run\": {}, \
             \"load_ms\": {:.1}, \"frame_work_ms\": {}, \"tick_frame_work_ms\": {}, \
             \"tick_ms\": {}, \"live_objects\": {{\"mean\": {:.1}, \"max\": {}}}, \
             \"deck_proxy_ms\": {:.4}, ",
            self.display_hz,
            self.display_hz_source,
            self.activation,
            self.seconds,
            self.samples.frames,
            self.fps(),
            self.ticks,
            self.images,
            self.preloaded_sets,
            self.images_loaded_during_run,
            self.load_ms,
            self.frame_work().json(),
            self.tick_frame_work().json(),
            self.tick_cost().json(),
            live_mean,
            live_max,
            self.deck_proxy_ms(),
        );
        if self.mode == "window" {
            let period = if self.display_hz > 0.0 {
                1000.0 / self.display_hz
            } else {
                0.0
            };
            let detail: Vec<String> = self
                .missed_vblank_indices()
                .into_iter()
                .filter_map(|i| {
                    let d = self.samples.present_detail.get(i)?;
                    let at = self.samples.present_at_s.get(i).copied().unwrap_or(-1.0);
                    Some(d.json(at, self.samples.present_ms[i], period))
                })
                .collect();
            let _ = write!(
                s,
                "\"acquire_ms\": {}, \"present_interval_ms\": {}, \"missed_vblanks\": {}, \
                 \"missed_vblank_at_s\": [{}], \"missed_vblank_detail\": [{}], \
                 \"resizes_during_run\": {}, ",
                Summary::of(&self.samples.acquire_ms).json(),
                Summary::of(&self.samples.present_ms).json(),
                self.missed_vblanks(),
                self.missed_vblank_times()
                    .iter()
                    .map(|t| format!("{t:.2}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                detail.join(", "),
                self.samples.resizes,
            );
        } else {
            let total: Vec<f64> = self
                .samples
                .work_ms
                .iter()
                .zip(&self.samples.gpu_ms)
                .map(|(w, g)| w + g)
                .collect();
            let _ = write!(
                s,
                "\"gpu_ms\": {}, \"frame_ms\": {}, ",
                Summary::of(&self.samples.gpu_ms).json(),
                Summary::of(&total).json(),
            );
        }
        let _ = write!(
            s,
            "\"budget\": {{\"frame_work_p99_under_ms\": {FRAME_BUDGET_MS}, \"frame_work_ok\": {}, \
             \"deck_proxy_under_ms\": {DECK_BUDGET_MS}, \"deck_proxy_ok\": {}",
            self.work_within_budget(),
            self.deck_within_budget(),
        );
        match self.missed_vblanks_unmeasurable() {
            None => {
                let _ = write!(s, ", \"missed_vblanks_ok\": {}", self.missed_vblanks() == 0);
            }
            Some(why) => {
                let _ = write!(
                    s,
                    ", \"missed_vblanks_ok\": null, \"missed_vblanks_not_measurable\": \"{why}\""
                );
            }
        }
        let _ = write!(s, ", \"pass\": {}", self.passed());
        s.push_str("}}");
        s
    }
}

/// A JSON string literal.
pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Runs the headless benchmark: loads the level, draws offscreen at
/// `settings.res` for `seconds` of wall time with a simulated `hz` display
/// clock, plays the sounds and music through a silent output, and returns
/// the report. Frames are paced to `hz` unless `settings.fps_cap` is `None`
/// (a throughput run). A start point in the settings replaces the route's.
pub fn run_headless(settings: &Settings, seconds: f64, hz: f64) -> Result<Report> {
    let started = Instant::now();
    let bundle = load_level(&settings.install, &settings.level)?;
    let mut graphics = load_graphics(&bundle, settings.pack.as_deref())?;
    let size = settings.res;
    let view = settings.view(size);
    let sim = new_sim(&bundle, settings.sim_config(size), settings.start);
    // The route covers the run at the game's pace, with a quarter to spare;
    // from the given start point when there is one.
    let bench_route = combat_route(&sim, seconds * 1.25 + 2.0, settings.start.is_some());
    let start = settings.start.or(bench_route.start());
    let mut audio_settings = settings.audio_settings();
    audio_settings.enabled = true;
    let font = load_soundfont(&audio_settings);
    let mut audio = AudioBridge::silent_with_music(audio_settings, font);
    let bank = audio.load_level(&bundle.store, &bundle.level.registries);
    let mut audio_run = AudioRun {
        sounds: bank.decoded,
        music_tracks: bank.music_tracks,
        cues: 0,
    };
    let activation = sim.config().activation.mode.name();
    let preloaded = preload(&mut graphics, &bundle.level, &sim.snapshot());
    let mut status = StatusLayer::new(&mut graphics, &bundle);
    let gpu = Gpu::headless().context("opening the GPU")?;
    let max = gpu.max_target();
    if size.0 > max || size.1 > max {
        anyhow::bail!(
            "--res {}x{} is larger than this GPU's largest image ({max} pixels a side)",
            size.0,
            size.1
        );
    }
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&graphics);
    let mut builder = SceneBuilder::new(&graphics, view, max);
    let presentation = builder.composition().presentation;
    let output = Offscreen::new(&gpu, size);
    let route_ticks = bench_route.script.ticks.len();
    let mut session = Session::new(sim).with_replay(bench_route.script.clone());
    session.set_before_tick(bench_route.mover());
    let mut combat = Combat::default();
    {
        status.update(session.sim(), 0);
        let frame = session.interpolated(false);
        let mut input = FrameInput::from_frame(&frame, &view);
        status.apply(&mut input, session.sim(), presentation);
        let scene = builder.build(&mut graphics, &input);
        renderer.render(&graphics, &scene, &output.view, COLOR_FORMAT);
        renderer.wait_idle();
    }
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    let images = graphics.images.records.len();
    session.time_ticks();

    let mut refresh = SimulatedRefresh::new(hz);
    let mut pacer = match settings.fps_cap {
        Some(_) => FramePacer::new(hz),
        None => FramePacer::uncapped(),
    };
    let pacing = match pacer.hz() {
        Some(hz) => format!("simulated {hz:.0} Hz display"),
        None => "none (throughput run)".to_string(),
    };
    let mut samples = Samples::default();
    let run = Instant::now();
    while run.elapsed().as_secs_f64() < seconds {
        // The wait comes before the frame's clock starts: it is never
        // counted as work.
        pacer.wait();
        let t0 = Instant::now();
        let dt = refresh.next().unwrap_or_default();
        let f = session.frame(dt);
        let (cues, events) = session.drain_outputs();
        let r = session.sim().view().rect;
        audio.apply(
            &cues,
            &ViewInfo {
                left: r.left,
                top: r.top,
                width: (r.right - r.left + 1).max(0) as u32,
                height: (r.bottom - r.top + 1).max(0) as u32,
            },
        );
        audio.events(&events);
        audio_run.cues += cues.len();
        status.events(&events);
        status.update(session.sim(), f.ticks);
        let frame = session.interpolated(settings.interpolation);
        let mut input = FrameInput::from_frame(&frame, &view);
        status.apply(&mut input, session.sim(), presentation);
        let scene = builder.build(&mut graphics, &input);
        renderer.render(&graphics, &scene, &output.view, COLOR_FORMAT);
        let work = t0.elapsed().as_secs_f64() * 1000.0;
        renderer.wait_idle();
        let all = t0.elapsed().as_secs_f64() * 1000.0;
        samples.frame(work, f.ticks);
        samples.gpu_ms.push(all - work);
        // Outside the measured work.
        if f.ticks > 0 {
            combat.look(session.sim());
        }
    }
    let seconds = run.elapsed().as_secs_f64();
    let comp = *builder.composition();
    Ok(Report {
        mode: "headless",
        level: bundle.name.clone(),
        res: size,
        output: size,
        offscreen_copy: None,
        presentation: comp.presentation,
        scale: comp.scale,
        adapter: gpu.describe(),
        present_mode: String::new(),
        pacing,
        display_hz: hz,
        display_hz_source: "simulated",
        seconds,
        ticks: session.ticks(),
        activation,
        images,
        preloaded_sets: preloaded,
        images_loaded_during_run: graphics.images.records.len() - images,
        load_ms,
        route_kind: bench_route.kind,
        route_start: start,
        route_ticks,
        ticks_past_route: session.ticks_past_script(),
        combat,
        audio: Some(audio_run),
        samples,
        ticks_measured: session.tick_samples().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use claw_sim::level::ObjectSpec;
    use claw_sim::{SimConfig, TileMap};

    /// A long flat floor (feet rest on y = 959) with officers at `xs`.
    fn arena(xs: &[i32]) -> SimLevel {
        let mut rows = vec![".".repeat(200); 15];
        rows.push("#".repeat(200));
        rows.push("#".repeat(200));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 880));
        for &x in xs {
            level.objects.push(ObjectSpec::at("Officer", x, 910, 4000));
        }
        level
    }

    #[test]
    fn the_combat_route_starts_before_the_crowd_and_is_played_back_the_same() {
        let level = arena(&[900, 1300, 1500, 1700, 6000]);
        let sim = Sim::new(level, SimConfig::default());
        let r = combat_route(&sim, 6.0, false);
        assert_eq!(r.kind, "combat");
        assert!(r.script.ticks.len() as f64 * 18.0 >= 6000.0);
        // The stretch with the most officers starts at the first one.
        assert_eq!(r.start(), Some((750, 959 - 64)));
        assert_eq!(
            r,
            combat_route(&sim, 6.0, false),
            "planned the same each time"
        );
        // Played back through a session with its moves, the game is the
        // one stepped tick by tick.
        let mut session = Session::new(sim.clone()).with_replay(r.script.clone());
        session.set_before_tick(r.mover());
        while session.replaying() {
            session.frame(std::time::Duration::from_millis(17));
        }
        let mut direct = sim.clone();
        for (k, t) in r.script.ticks.iter().enumerate() {
            for &(at, x, y) in &r.teleports {
                if at == k as u64 {
                    direct.teleport_player(x, y);
                }
            }
            direct.step(*t);
        }
        let at = |s: &Sim| s.player().map(|o| (o.x, o.y, o.state));
        assert_eq!(at(session.sim()), at(&direct));
        assert!(r.teleports.len() >= 2, "Claw was moved to the officers");
        let mut combat = Combat::default();
        combat.look(session.sim());
        assert!(combat.enemies_live_max >= 1, "{combat:?}");
        // From where Claw stands, no move first.
        let here = combat_route(&sim, 6.0, true);
        assert_eq!(here.start(), None);
        assert_eq!(here.kind, "combat");
    }

    #[test]
    fn a_level_without_enemies_gets_the_movement_route() {
        let sim = Sim::new(arena(&[]), SimConfig::default());
        let r = combat_route(&sim, 3.0, false);
        assert_eq!(r.kind, "movement");
        assert!(r.teleports.is_empty());
        assert_eq!(r.script, route(3.0));
    }

    #[test]
    fn the_verdict_says_when_the_route_ran_out() {
        let mut r = passing_window_run();
        assert!(!r.summary().contains("ran out"));
        r.ticks_past_route = 40;
        assert!(r.passed(), "running out is no budget");
        assert!(
            r.summary()
                .contains("the route ran out after 50 ticks, 40 ticks ran without it"),
            "{}",
            r.summary()
        );
        assert!(r.to_json().contains("\"ran_out\": true"));
    }

    #[test]
    fn route_covers_the_run_and_moves_both_ways() {
        let s = route(10.0);
        assert!(s.ticks.len() as f64 * 18.0 >= 10_000.0);
        let any = |b: Buttons| s.ticks.iter().any(|t| t.held.contains(b));
        assert!(any(Buttons::LEFT) && any(Buttons::RIGHT) && any(Buttons::JUMP));
        assert!(any(Buttons::UP) && any(Buttons::DOWN));
        assert_eq!(route(10.0), s);
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let v: Vec<f64> = (1..=200).map(f64::from).collect();
        let s = Summary::of(&v);
        assert_eq!((s.p50, s.p99, s.max, s.n), (100.0, 198.0, 200.0, 200));
        assert_eq!(Summary::of(&[]).max, 0.0);
    }

    #[test]
    fn report_checks_the_budgets() {
        let mut r = Report {
            mode: "window",
            display_hz: 100.0,
            seconds: 2.0,
            ..Report::default()
        };
        for i in 0..200 {
            r.samples.frame(1.0 + f64::from(i) / 100.0, (i % 2) as u32);
            r.samples.present_ms.push(10.0);
        }
        assert!(r.work_within_budget());
        assert!((r.deck_proxy_ms() - 2.97 * 3.0).abs() < 1e-9);
        assert!(!r.deck_within_budget());
        assert_eq!(r.missed_vblanks(), 0);
        r.samples.present_ms.push(20.0);
        assert_eq!(r.missed_vblanks(), 1);
        assert_eq!(r.fps(), 100.0);
        let json = r.to_json();
        assert!(json.contains("\"missed_vblanks\": 1"), "{json}");
        assert!(json.contains("\"deck_proxy_ok\": false"), "{json}");
        assert!(json.starts_with('{') && json.ends_with('}'));
    }

    fn passing_window_run() -> Report {
        let mut r = Report {
            mode: "window",
            present_mode: "Fifo".into(),
            display_hz: 100.0,
            seconds: 1.0,
            route_ticks: 50,
            ..Report::default()
        };
        for i in 0..100 {
            r.samples.frame(0.5, (i % 2) as u32);
            r.samples.present_ms.push(10.0);
            r.samples.present_at_s.push(f64::from(i) / 100.0);
            r.samples.present_detail.push(PresentDetail::default());
        }
        r
    }

    #[test]
    fn the_verdict_names_every_budget() {
        let r = passing_window_run();
        assert!(r.passed());
        let line = r.summary();
        assert!(line.starts_with("bench PASS: "), "{line}");
        for name in ["frame work p99", "deck proxy", "missed vblanks 0"] {
            assert!(line.contains(name), "{line}");
        }
        assert!(!line.contains('\n'));
        assert!(r.to_json().contains("\"pass\": true"));
    }

    #[test]
    fn a_missed_refresh_fails_the_run_and_is_described() {
        let mut r = passing_window_run();
        r.samples.present_ms[40] = 25.0;
        r.samples.present_detail[40] = PresentDetail {
            gap_ms: 0.2,
            work_ms: 0.3,
            acquire_ms: 24.0,
            present_call_ms: 0.1,
            ticks: 1,
            ..PresentDetail::default()
        };
        r.samples.present_ms[70] = 30.0;
        r.samples.present_detail[70] = PresentDetail {
            sets_loaded: 1,
            ..PresentDetail::default()
        };
        assert!(!r.passed());
        let line = r.summary();
        assert!(line.starts_with("bench FAIL: "), "{line}");
        assert!(line.contains("missed vblanks 2 at 100.00 Hz (want 0) at 0.40, 0.70 s FAIL"));
        let json = r.to_json();
        assert!(json.contains("\"missed_vblanks_ok\": false"), "{json}");
        assert!(json.contains("\"pass\": false"), "{json}");
        assert!(
            json.contains("\"likely_cause\": \"waiting on the display"),
            "{json}"
        );
        assert!(json.contains("\"likely_cause\": \"sprite loading in the game\""));
        // A slow frame of work fails its own budget too.
        let mut r = passing_window_run();
        r.samples.work_ms[3] = 50.0;
        r.samples.work_ms[4] = 50.0;
        assert!(!r.passed());
        assert!(
            r.summary()
                .contains("frame work p99 50.000 ms (under 6.9 ms) FAIL")
        );
    }

    #[test]
    fn missed_refreshes_are_not_measurable_offscreen_or_without_vsync() {
        let mut r = passing_window_run();
        r.samples.present_ms[10] = 40.0;
        r.mode = "headless";
        assert!(r.passed());
        assert!(
            r.summary()
                .contains("missed vblanks not measurable offscreen"),
            "{}",
            r.summary()
        );
        let json = r.to_json();
        assert!(json.contains("\"missed_vblanks_ok\": null"), "{json}");
        r.mode = "window";
        r.present_mode = "Immediate".into();
        assert!(r.summary().contains("not measurable without vsync"));
        assert!(r.passed());
        r.present_mode = "Fifo".into();
        assert!(!r.passed());
    }

    #[test]
    fn likely_causes() {
        let d = |f: fn(&mut PresentDetail)| {
            let mut d = PresentDetail {
                gap_ms: 1.0,
                work_ms: 0.5,
                acquire_ms: 0.2,
                ..PresentDetail::default()
            };
            f(&mut d);
            d.likely_cause(7.0)
        };
        assert_eq!(
            d(|_| {}),
            "between frames (compositor frame callback or event loop)"
        );
        assert_eq!(
            d(|d| d.acquire_ms = 12.0),
            "waiting on the display (compositor or driver)"
        );
        assert_eq!(d(|d| d.work_ms = 9.0), "frame work in the game");
        assert_eq!(d(|d| d.events.push("resize")), "window change in the game");
        assert_eq!(d(|d| d.images_loaded = 3), "sprite loading in the game");
    }
}
