//! The fourteenth level's tiger guards, lava hazards and jump-switch room,
//! and its help-screen fuse, on your own install, against the original's
//! runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does but without the warp's own frame, see
//! [`Sim::place_between_ticks`], at the point the runs name, a little
//! above the floor, so that he lands there and his idle wait starts from
//! the landing), waits
//! until the level clock reaches the time the original's run started at,
//! and starts the shared random stream from the run's state, once. A
//! tiger awake before that time is put in step with the run there
//! ([`InStep`]). The routes:
//!
//! - tiger-stand: on the floor of the tiger guard placed at (2563, 3171),
//!   250 px left of it: Claw faces right and stands while the tiger runs
//!   at him, flips back, lunges and slashes;
//! - tiger-dyn: the same with a tap of Dynamite on row 25: the stick's
//!   blast finds the tiger lunging, and it blocks, unhurt;
//! - hand: left of the lava hand at (20787, 3184): Right on rows 20 to 35
//!   walks Claw into its box, and he stands while it throws its bursts of
//!   three lava balls and the fire shooter beside it fires;
//! - geyser-lake: the shore left of the lava lake with the fountains: Right
//!   from row 15, a jump off the edge on row 79 into the lava, the death
//!   and the respawn (a measuring run: the route's `# checkpoint 22783,2330`
//!   line puts the respawn point over the lake at time zero), after which
//!   the delayed fountain holds its last column through its delay;
//! - arena-c18: the jump-switch room at the warp's target: Claw stands
//!   while the room's unbounded tigers and Chameleons pace onto the
//!   switches and leap;
//! - dyn-fuse-shaft-c18: by the shaft under the checkpoint at (13202,
//!   2208): a jump left into the shaft, a stick of dynamite six rows in,
//!   the help screen for 4300 ms before row 43, whose time burns the fuse,
//!   the blast's mid-air hit, and Right and Jump on the tick after the
//!   mid-air stand.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched tigers, lizards, fountains and the hand
//! are compared too at the 18 ms step: their position, their state (lined
//! up by the numbers the run gives them, [`reference_state`]), their
//! health (the enemies'), whether they are out of every collision and
//! whether they are hidden.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace, or the reference runs (or a run's
//! `obj-log.csv`, or a watched object in it) are missing; `CLAW_L14_ONLY`
//! names one store to play alone, and `CLAW_L14_TRACE` a folder to write
//! the replay's traces to and `CLAW_L14_DUMP=FROM..TO` prints the watched
//! objects of both on those rows. Nothing from an install or a reference store is
//! kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::input::{self, ScriptTuning};
use claw_sim::logic::enemy::{self, EnemyState, chameleon, tiger_guard};
use claw_sim::logic::lava::{ball, geyser, hand};
use claw_sim::object::{ObjectId, draw_flags, flags};
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The Dynamite button.
const DYNAMITE: u32 = 0x100;

/// One stretch of a route: from `from` to `to` (inclusive) `held` is down,
/// pressed on the stretch's first row.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    held: u32,
}

const fn span(from: usize, to: usize, held: u32) -> Span {
    Span { from, to, held }
}

/// A watched object: its logic, its serial number in the original's runs
/// and where it is placed.
#[derive(Debug, Clone, Copy)]
struct Watched {
    logic: &'static str,
    serial: &'static str,
    at: (i32, i32),
}

const fn tiger(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: tiger_guard::NAMES[0],
        serial,
        at,
    }
}

const fn lizard(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: chameleon::NAMES[0],
        serial,
        at,
    }
}

/// A route of the stores: its name, where Claw is put down (the runs'
/// point, above the floor he lands on), how many rows it has, its stretches, the clocks it was
/// run at, the respawn point it sets at time zero, its help screens (the
/// row before which play comes back, and the screen's time), the objects
/// it watches and the tigers it puts in step at time zero.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
    checkpoint: Option<(i32, i32)>,
    helps: &'static [(usize, u32)],
    watch: &'static [Watched],
    in_step: &'static [InStep],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The game time at a route's time zero, by clock.
const T0_MS: [(&str, u64); 2] = [("c18", 828), ("c60", 817)];

/// The tiger guard on the floor right of the level's start.
const FLOOR_TIGER: Watched = tiger("880", (2563, 3171));
/// The lava hand by the fire shooter, and the shooter.
const HAND: Watched = Watched {
    logic: hand::NAMES[0],
    serial: "3216",
    at: (20787, 3184),
};
/// The first two fountains of the lava lake: no delay, and 500 ms.
const GEYSERS: [Watched; 2] = [
    Watched {
        logic: geyser::NAMES[0],
        serial: "664",
        at: (22852, 2421),
    },
    Watched {
        logic: geyser::NAMES[0],
        serial: "665",
        at: (22983, 2422),
    },
];
/// The jump-switch room's six tigers and two Chameleons.
const ARENA: [Watched; 8] = [
    tiger("732", (16392, 4389)),
    tiger("733", (15667, 4260)),
    tiger("734", (16646, 4260)),
    tiger("736", (15931, 4389)),
    tiger("737", (15402, 4132)),
    tiger("738", (16920, 4132)),
    lizard("739", (15718, 4283)),
    lizard("740", (16588, 4283)),
];

/// The floor tiger at time zero: landing from a flip away from Claw,
/// resting after its first strike, whose combo of four slashes became a
/// run since Claw was not within 100 px.
const FLOOR_TIGER_IN_STEP: [InStep; 2] = [
    InStep {
        clock: "c18",
        at: FLOOR_TIGER.at,
        state: EnemyState::BackFlip,
        x: 2634,
        frac: 0.1,
        frac_y: 0.324,
        frame: 850,
        vx: tiger_guard::FLIP_SPEED,
        rest: (36, 1632),
        target: 2630,
        slashes: 4,
    },
    InStep {
        clock: "c60",
        at: FLOOR_TIGER.at,
        state: EnemyState::BackFlip,
        x: 2630,
        frac: 0.06,
        frac_y: 0.0,
        frame: 850,
        vx: tiger_guard::FLIP_SPEED,
        rest: (34, 1632),
        target: 2630,
        slashes: 4,
    },
];

/// The white tiger guard on the floor right of the shaft.
const SHAFT_TIGER: Watched = tiger("883", (13020, 2602));

/// The shaft tiger at time zero: running at Claw, resting after its first
/// strike, whose combo of four slashes became a run.
const SHAFT_TIGER_IN_STEP: [InStep; 1] = [InStep {
    clock: "c18",
    at: SHAFT_TIGER.at,
    state: EnemyState::FastAdvance,
    x: 13071,
    frac: 0.08,
    frac_y: 0.0,
    frame: 2,
    vx: -(tiger_guard::SPEED as f64),
    rest: (108, 1586),
    target: 13107,
    slashes: 4,
}];

const ROUTES: [Route; 6] = [
    Route {
        store: "tiger-stand",
        start: (2380, 3150),
        rows: 700,
        spans: &[span(15, 16, RIGHT)],
        clocks: &["c18", "c60"],
        checkpoint: None,
        helps: &[],
        watch: &[FLOOR_TIGER],
        in_step: &FLOOR_TIGER_IN_STEP,
    },
    Route {
        store: "tiger-dyn",
        start: (2380, 3150),
        rows: 500,
        spans: &[span(15, 16, RIGHT), span(25, 26, DYNAMITE)],
        clocks: &["c18", "c60"],
        checkpoint: None,
        helps: &[],
        watch: &[FLOOR_TIGER],
        in_step: &FLOOR_TIGER_IN_STEP,
    },
    Route {
        store: "hand",
        start: (20290, 2990),
        rows: 600,
        spans: &[span(20, 35, RIGHT)],
        clocks: &["c18", "c60"],
        checkpoint: None,
        helps: &[],
        watch: &[HAND],
        in_step: &[],
    },
    Route {
        store: "geyser-lake",
        start: (22250, 2200),
        rows: 700,
        spans: &[span(15, 119, RIGHT), span(79, 96, JUMP)],
        clocks: &["c18", "c60"],
        checkpoint: Some((22783, 2330)),
        helps: &[],
        watch: &GEYSERS,
        in_step: &[],
    },
    Route {
        store: "arena-c18",
        start: (16170, 4480),
        rows: 900,
        spans: &[],
        clocks: &["c18"],
        checkpoint: None,
        helps: &[],
        watch: &ARENA,
        in_step: &[],
    },
    Route {
        store: "dyn-fuse-shaft-c18",
        start: (12860, 2600),
        rows: 300,
        spans: &[
            span(15, 16, LEFT),
            span(25, 44, JUMP),
            span(25, 54, LEFT),
            span(31, 32, DYNAMITE),
            span(74, 93, JUMP),
            span(74, 153, RIGHT),
        ],
        clocks: &["c18"],
        checkpoint: None,
        helps: &[(43, 4300)],
        watch: &[SHAFT_TIGER],
        in_step: &SHAFT_TIGER_IN_STEP,
    },
];

/// The stores whose runs set something up that plain play does not (each
/// says so in its meta).
const DIAGNOSTIC: [&str; 1] = ["geyser-lake"];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. The hand's route, the
/// geyser lake's and the jump-switch room follow their stores exactly at
/// the 18 ms step, the random stream and every watched object included,
/// and the geyser lake's at 60 frames per second too. The rest:
///
/// - tiger-stand and tiger-dyn, 18 ms: Claw's whole trace (the random
///   stream, his health from the tiger's slashes and lunges) and the
///   tiger's state and health follow the runs; the tiger's x is one pixel
///   off on one row of a walk late in each run (rows 552 and 539): the
///   original keeps its position as one number that adds 3.24 px a tick
///   and lands a hair below a whole pixel (2521.99999...), where this
///   build lands on it.
/// - dyn-fuse-shaft-c18: the return from the help screen runs every
///   object's logic once more, outside any step, with the screen's 4300
///   ms (see `Sim::pass_help_return`): the white tiger by the shaft, out
///   of Claw's reach since the jump, sees its 500 ms out-of-range wait end
///   there and goes idle on row 43, as in the original. The tiger still
///   parts from the run at 2592 ms (its x), the random stream at 2628 ms
///   and Claw at 3258 ms: not looked into here.
/// - At 60 frames per second the replay plays 18 ms rows: the original's
///   timers and draws run on its shorter frames, so the tigers' decisions
///   come out otherwise (the lunge that meets Claw on tiger-stand and
///   tiger-dyn comes at another moment) and the fire shooter by the hand
///   fires on its 18 ms schedule (its shots meet Claw at 2430 and 4626
///   ms, against the run's 2400 and 4533).
const KNOWN: &[Known] = &[
    ("tiger-stand", "c18", &[("880_x", 9954)]),
    ("tiger-dyn", "c18", &[("880_x", 9720)]),
    (
        "dyn-fuse-shaft-c18",
        "c18",
        &[
            ("rng", 2628),
            ("883_x", 2592),
            ("883_state", 2610),
            ("cam_x", 3276),
            ("883_out", 2610),
            ("state_id", 3258),
            ("health", 3258),
            ("x", 3276),
        ],
    ),
    (
        "tiger-stand",
        "c60",
        &[
            ("state_id", 2283),
            ("health", 2283),
            ("x", 2333),
            ("cam_x", 2333),
        ],
    ),
    (
        "tiger-dyn",
        "c60",
        &[
            ("state_id", 1967),
            ("health", 1967),
            ("x", 2017),
            ("cam_x", 2017),
        ],
    ),
    (
        "hand",
        "c60",
        &[
            ("state_id", 2767),
            ("health", 4533),
            ("x", 4567),
            ("cam_x", 4567),
        ],
    ),
];

fn known(store: &str, clock: &str) -> Vec<(&'static str, u64)> {
    KNOWN
        .iter()
        .filter(|(s, c, _)| *s == store && *c == clock)
        .flat_map(|(_, _, k)| k.iter().copied())
        .collect()
}

fn route_named(store: &str) -> &'static Route {
    ROUTES
        .iter()
        .find(|r| r.store == store)
        .unwrap_or_else(|| panic!("no route {store}"))
}

fn route_ticks(r: &Route) -> Vec<TickInput> {
    let mut held = vec![0u32; r.rows];
    let mut pressed = vec![0u32; r.rows];
    for s in r.spans {
        for h in &mut held[s.from..=s.to] {
            *h |= s.held;
        }
        pressed[s.from] |= s.held;
    }
    held.iter()
        .zip(&pressed)
        .map(|(&h, &p)| TickInput {
            held: Buttons(h),
            pressed: Buttons(p),
        })
        .collect()
}

/// A store's own copy of a route: (held, pressed) by row, its directives'
/// lines and its whole text.
type StoreRoute = (Vec<(u32, u32)>, Vec<String>, String);

/// The rows of a store's own copy of a route, with its directives' lines
/// and the route's whole text.
fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut directives = Vec::new();
    for l in text.lines() {
        if l.starts_with('@') {
            directives.push(l.to_string());
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, directives, text))
}

/// A value of a store's `meta.toml`: `key` in the table `[section]`, or
/// at the top when `section` is empty.
fn meta_value(meta: &str, section: &str, key: &str) -> Option<String> {
    let mut current = String::new();
    for line in meta.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = name.to_string();
        } else if current == section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn level14() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(14)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn reference_root() -> Option<PathBuf> {
    match std::env::var_os("CLAW_REFERENCE") {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => {
            eprintln!("skipped: CLAW_REFERENCE is not set");
            None
        }
    }
}

/// The folder of the reference runs: `CLAW_REFERENCE_RUNS`, or `rt` next
/// to the reference store. With `CLAW_REFERENCE` set it must be there.
fn runs_root(reference: &Path) -> PathBuf {
    let dir = match std::env::var_os("CLAW_REFERENCE_RUNS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => reference
            .parent()
            .expect("the reference store has a parent folder")
            .join("rt"),
    };
    assert!(
        dir.is_dir(),
        "the reference runs are not at {} (set CLAW_REFERENCE_RUNS)",
        dir.display()
    );
    dir
}

/// The first run of `clock` a store's meta names.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let line = meta_value(meta, clock, "run_ids")?;
    Some(
        line.trim_start_matches('[')
            .split('"')
            .find(|p| !p.is_empty())?
            .to_string(),
    )
}

fn config(clock: &str, seed: u32) -> SimConfig {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    SimConfig {
        tuning: tuning.tuning(),
        seed,
        ..SimConfig::default()
    }
}

fn t0_ms(clock: &str) -> u64 {
    T0_MS.iter().find(|(c, _)| *c == clock).map_or(0, |t| t.1)
}

/// The game at a route's time zero (see the module documentation), with
/// the route's respawn point set there and the objects it puts in step
/// with the original's run there ([`InStep`]).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
    let mut sim = Sim::new(level.clone(), config(clock, seed));
    sim.step(TickInput::default());
    sim.place_between_ticks(r.start.0, r.start.1);
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    if let Some(p) = r.checkpoint {
        sim.set_checkpoint(Some(p));
    }
    for s in r.in_step.iter().filter(|s| s.clock == clock) {
        put_in_step(&mut sim, s);
    }
    sim
}

/// A tiger guard the original's run shows at the route's time zero in a
/// state its life before time zero led to: awake from the level's first
/// ticks, it decided on draws of the random stream before time zero,
/// which the runs do not show, so this build's tiger takes the run's
/// state at time zero instead: the one placed at `at`, in `state` (a back
/// flip whose aim `target` it has passed, ending on the route's first
/// step, or a run at Claw), at `x` and the fraction `frac` (and its
/// height, its exact height `frac_y` px below it), showing `frame`, moving
/// at `vx` px/s, facing left, alerted, its strike rest started at `rest.0`
/// ms for `rest.1` ms and its last combo's slashes `slashes`.
///
/// A standing enemy's exact height sinks by each step's gravity (0.324 px
/// in the first 18 ms step) and goes back onto its floor pixel once it has
/// left it, so it runs through a cycle of a few steps the runs do not
/// show; where in that cycle the tiger is at time zero shows only in how
/// high a jump off the floor takes it. On tiger-dyn the floor tiger jumps
/// back from the stick on row 31, and only one step into the cycle at
/// time zero (0.324 px below its pixel) rises as the run's does.
/// tiger-stand, which shares the set-up, and the shaft's tiger follow
/// their runs at any point of the cycle.
#[derive(Debug, Clone, Copy)]
struct InStep {
    clock: &'static str,
    at: (i32, i32),
    state: EnemyState,
    x: i32,
    frac: f64,
    frac_y: f64,
    frame: i32,
    vx: f64,
    rest: (u64, u64),
    target: i32,
    slashes: u32,
}

/// Puts a tiger in step with the original's run at time zero (see
/// [`InStep`]).
fn put_in_step(sim: &mut Sim, s: &InStep) {
    let id = placed(sim, tiger_guard::NAMES[0], s.at);
    sim.with_world(|w| {
        let now = w.now_ms;
        enemy::set_state(w, id, s.state);
        let flip = s.state == EnemyState::BackFlip;
        let anim = if flip {
            tiger_guard::BACK_FLIP_ANIM
        } else {
            tiger_guard::RUN_ANIM
        };
        w.set_anim(id, anim);
        enemy::set_facing(w, id, false);
        let y = w.objects.get(id).map_or(0, |o| o.y);
        if let Some(o) = w.objects.get_mut(id) {
            o.x = s.x;
            o.frame = s.frame;
            if flip {
                o.flags |= flags::NO_COLLIDE;
            }
        }
        if let Some(d) = enemy::data_mut(w, id) {
            d.alerted = true;
            d.busy = false;
            d.carry = (s.frac, s.frac_y);
            d.moved_to = Some((s.x, y));
            d.vx = s.vx;
        }
        if let Some(t) = tiger_guard::tiger_data_mut(w, id) {
            t.strike_rest = enemy::Timer::new(s.rest.0, s.rest.1);
            t.target = s.target;
            t.flip_right = s.vx > 0.0;
            t.flip = enemy::Timer::new(now, tiger_guard::FLIP_MS);
            t.slashes = s.slashes;
            t.attacking = false;
            t.reaction = enemy::Timer::default();
        }
    });
}

/// A store's time zero and random state for `clock`.
fn time_zero(dir: &Path, clock: &str) -> Option<(u64, u32)> {
    let meta = std::fs::read_to_string(dir.join("meta.toml")).ok()?;
    let t0 = meta_value(&meta, clock, "t0_ms")?.parse().ok()?;
    let seed = meta_value(&meta, clock, "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))?
        .parse()
        .ok()?;
    Some((t0, seed))
}

/// The state a run's number for a watched object's state stands for, in
/// this build's numbers: a tiger's or a lizard's: idle, strike, dead,
/// the leap, pause, home, the hop back, advance, run, pace, the two hit
/// reactions, the block, the knife combo, the two fades, the back flip,
/// the lunge and the front flip; a fountain's waiting and erupting; a
/// hand's waiting and throwing.
fn reference_state(logic: &str, n: i32) -> Option<i32> {
    if logic == geyser::NAMES[0] {
        return match n {
            2069 => Some(geyser::state::IDLE),
            2102 => Some(geyser::state::ERUPTING),
            _ => None,
        };
    }
    if logic == hand::NAMES[0] {
        return match n {
            2069 => Some(hand::state::IDLE),
            2103 => Some(hand::state::THROWING),
            _ => None,
        };
    }
    if logic == ball::NAMES[0] {
        return match n {
            2001 => Some(ball::state::FALLING),
            2002 => Some(ball::state::EXPLODING),
            _ => None,
        };
    }
    let s = match n {
        2000 => EnemyState::Idle,
        2003 => EnemyState::Strike,
        2004 => EnemyState::Dead,
        2005 => EnemyState::Jump,
        2011 => EnemyState::Pause,
        2018 => EnemyState::Home,
        2020 => EnemyState::JumpBack,
        2021 => EnemyState::Advance,
        2022 => EnemyState::FastAdvance,
        2023 => EnemyState::Pace,
        2024 => EnemyState::HitLow,
        2025 => EnemyState::HitHigh,
        2035 => EnemyState::Block,
        2042 => EnemyState::KnifeAttack,
        2104 => EnemyState::PhaseOut,
        2105 => EnemyState::PhaseIn,
        2106 => EnemyState::BackFlip,
        2107 => EnemyState::LungeAttack,
        2108 => EnemyState::FrontFlip,
        _ => return None,
    };
    Some(s as i32)
}

/// Whether the runs log a health for objects of `logic` (the hazards have
/// none).
fn logic_has_health(logic: &str) -> bool {
    logic == tiger_guard::NAMES[0] || logic == chameleon::NAMES[0]
}

/// A watched object at one step of a run or of the replay: position,
/// state (the run's own numbers lined up through [`reference_state`]),
/// health, out of every collision, hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjAt {
    x: i32,
    y: i32,
    state: Option<i32>,
    health: i32,
    out: bool,
    hidden: bool,
}

/// The steps of the watched objects in a run's `obj-log.csv`, by object:
/// (step, the object there).
fn reference_objects(path: &Path, watch: &[Watched]) -> Option<Vec<Vec<(usize, ObjAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, fl, df, hp) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("flags")?,
        col("draw_flags")?,
        col("health")?,
    );
    let mut out = vec![Vec::new(); watch.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = watch
            .iter()
            .position(|w| f.get(serial) == Some(&w.serial) && f.get(st).is_some())
        else {
            continue;
        };
        let logic = watch[i].logic;
        out[i].push((
            f[tick].parse().ok()?,
            ObjAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(logic, int(f[st])?),
                health: if logic_has_health(logic) {
                    int(f[hp]).unwrap_or(0)
                } else {
                    0
                },
                out: hex(f[fl])? & flags::NO_COLLIDE != 0,
                hidden: hex(f[df])? & draw_flags::HIDDEN != 0,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's object leaves the
/// run's, by column (`<serial>_x`, `_y`, `_state`, `_health`, `_out`,
/// `_hidden`); a step on which the replay has no such object counts for
/// all of them.
fn compare_object(
    serial: &str,
    played: &[Option<ObjAt>],
    reference: &[(usize, ObjAt)],
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: &str, k: usize| {
        let name = format!("{serial}_{name}");
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in reference {
        let Some(got) = played.get(k) else { continue };
        let Some(got) = got else {
            for name in ["x", "y", "state", "health", "out", "hidden"] {
                note(name, k);
            }
            continue;
        };
        if got.x != want.x {
            note("x", k);
        }
        if got.y != want.y {
            note("y", k);
        }
        if want.state.is_none() || got.state != want.state {
            note("state", k);
        }
        if got.health != want.health {
            note("health", k);
        }
        if got.out != want.out {
            note("out", k);
        }
        if got.hidden != want.hidden {
            note("hidden", k);
        }
    }
    firsts
}

/// The object of logic `logic` placed at `at`.
fn placed(sim: &Sim, logic: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} placed at {at:?}"))
}

fn obj_at(sim: &Sim, id: ObjectId) -> Option<ObjAt> {
    sim.objects().get(id).map(|o| ObjAt {
        x: o.x,
        y: o.y,
        state: Some(o.state),
        health: if logic_has_health(&o.logic) {
            o.health
        } else {
            0
        },
        out: o.flags & flags::NO_COLLIDE != 0,
        hidden: o.draw_flags & draw_flags::HIDDEN != 0,
    })
}

/// Plays one row of route `r` (with the help screen that returns before
/// it).
fn step_row(sim: &mut Sim, r: &Route, ticks: &[TickInput], k: usize) {
    for &(_, ms) in r.helps.iter().filter(|(at, _)| *at == k) {
        // The return from the help screen: its update pass, and the idle
        // wait starts again.
        sim.pass_menu_time(ms);
        sim.restart_idle_wait();
    }
    sim.step(ticks.get(k).copied().unwrap_or_default());
}

/// One play of a route: its trace, the reference trace, and the watched
/// objects on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    objects: Vec<Vec<Option<ObjAt>>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level14").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    assert_eq!(t0, t0_ms(clock), "{}: time zero", r.store);
    assert_eq!(seed, SEED, "{}: random state", r.store);
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let ids: Vec<ObjectId> = r
        .watch
        .iter()
        .map(|w| placed(&sim, w.logic, w.at))
        .collect();
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut objects = vec![Vec::new(); ids.len()];
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        step_row(&mut sim, r, &ticks, k);
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            objects[i].push(obj_at(&sim, id));
        }
        k += 1;
    }
    Played {
        rows,
        reference,
        objects,
    }
}

/// Compares a play of route `r` at `clock` with the reference trace at
/// the clock's tolerances (exact for c18, feel for c60), and at c18 its
/// watched objects with the run's; returns the divergences not known.
fn new_divergences(root: &Path, r: &Route, clock: &str, p: &Played) -> Vec<String> {
    let store = r.store;
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(&p.rows, &p.reference, &tol);
    print!("{}", trace::report(&p.rows, &p.reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != p.reference.len() {
        out.push(format!(
            "{store} {clock}: {} of {} rows compared",
            cmp.rows_compared,
            p.reference.len()
        ));
    }
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.to_string(), t)))
        .collect();
    if clock == "c18" && !r.watch.is_empty() {
        let runs = runs_root(root);
        let meta = std::fs::read_to_string(root.join("level14").join(store).join("meta.toml"))
            .expect("meta");
        let id = run_id(&meta, clock).unwrap_or_else(|| panic!("{store}: no run id"));
        let log = runs.join(&id).join("obj-log.csv");
        let reference = reference_objects(&log, r.watch)
            .unwrap_or_else(|| panic!("{store}: {} lacks a watched object", log.display()));
        for (i, w) in r.watch.iter().enumerate() {
            firsts.extend(compare_object(w.serial, &p.objects[i], &reference[i]));
        }
        if let Some((from, to)) = dump_range() {
            for (i, w) in r.watch.iter().enumerate() {
                for &(k, want) in reference[i].iter().filter(|(k, _)| (from..=to).contains(k)) {
                    let got = p.objects[i].get(k).copied().flatten();
                    let mark = if got == Some(want) { ' ' } else { '>' };
                    println!(
                        "{mark}{} {k} want {want:?}\n  {} {k} got  {got:?}",
                        w.serial, w.serial
                    );
                }
            }
        }
    }
    let list = known(store, clock);
    for (column, first) in firsts {
        println!("first {store} {clock} {column} {first}");
        match list.iter().find(|(name, _)| *name == column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{store} {clock}: {column} at {first} ms")),
        }
    }
    out
}

/// The rows `CLAW_L14_DUMP=FROM..TO` asks to print the watched objects
/// of both on.
fn dump_range() -> Option<(usize, usize)> {
    let v = std::env::var("CLAW_L14_DUMP").ok()?;
    let (a, b) = v.split_once("..")?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level14").join(r.store);
        let (rows, directives, text) =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        // The respawn point its runs set at time zero.
        assert_eq!(
            input::script_checkpoint(&text),
            Ok(r.checkpoint),
            "{}: checkpoint",
            r.store
        );
        assert_eq!(input::script_health(&text), Ok(None), "{}", r.store);
        // Its help screens come back before the rows the route lists; its
        // pauses change nothing in the game.
        let helps: Vec<(usize, u32)> = directives
            .iter()
            .filter_map(|d| {
                let rest = d.strip_prefix('@')?;
                let mut f = rest.split_whitespace();
                let at: usize = f.next()?.parse().ok()?;
                (f.next()? == "menu" && f.next()? == "help")
                    .then(|| Some((at, f.next()?.parse().ok()?)))
                    .flatten()
            })
            .collect();
        assert_eq!(helps, r.helps, "{}: help screens", r.store);
        for d in &directives {
            assert!(
                d.contains(" menu help ") || d.contains(" pause "),
                "{}: {d}",
                r.store
            );
        }
        // Each clock the route lists has its meta, its start and its
        // trace; only the listed stores are diagnostic.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        assert_eq!(
            meta_value(&meta, "", "diagnostic").as_deref() == Some("true"),
            DIAGNOSTIC.contains(&r.store),
            "{}",
            r.store
        );
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let teleport = meta_value(&meta, clock, "teleport")
                .unwrap_or_else(|| panic!("{} {clock}: no start", r.store));
            let point: Vec<i32> = teleport.split(',').map(|v| v.parse().unwrap()).collect();
            assert_eq!(point, [r.start.0, r.start.1], "{} {clock}: start", r.store);
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_level14_object_routes_follow_the_reference_traces() {
    let Some(level) = level14() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L14_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level14/{} {clock}", r.store);
            let p = play(&level, &root, r, clock);
            if let Some(out) = std::env::var_os("CLAW_L14_TRACE") {
                let out = PathBuf::from(out).join(format!("{}-{clock}.csv", r.store));
                std::fs::write(&out, trace::to_csv(&p.rows)).expect("the trace is written");
            }
            failures.extend(new_divergences(&root, r, clock, &p));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw, his state and health, his lives,
/// the row's events, and the watched object.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    lives: i32,
    died: bool,
    obj: Option<ObjAt>,
    frame: i32,
    attack: Option<claw_sim::object::Rect>,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching the object of
/// logic `logic` placed at `at`.
fn watch(level: &SimLevel, store: &str, rows: usize, logic: &str, at: (i32, i32)) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, t0_ms("c18"), SEED);
    let id = placed(&sim, logic, at);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        step_row(&mut sim, r, &ticks, k);
        let c = sim.player().expect("Claw");
        let o = sim.objects().get(id);
        looks.push(Look {
            x: c.x,
            y: c.y,
            state: c.state,
            health: sim.claw().stats.health,
            lives: sim.claw().stats.lives,
            died: sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::ClawDied { .. })),
            obj: obj_at(&sim, id),
            frame: o.map_or(0, |o| o.frame),
            attack: o.map(|o| o.attack_rect).filter(|r| r.is_set()),
        });
    }
    looks
}

/// Rows `from..=to` of `rows`, with their numbers.
fn rows_in(rows: &[Look], from: usize, to: usize) -> impl Iterator<Item = (usize, &Look)> {
    rows[from..=to]
        .iter()
        .enumerate()
        .map(move |(i, l)| (from + i, l))
}

fn state_of(l: &Look) -> i32 {
    l.obj.and_then(|o| o.state).unwrap_or(-1)
}

/// The rows on which the watched object enters `state`.
fn entries(rows: &[Look], state: i32) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| state_of(&rows[k]) == state && (k == 0 || state_of(&rows[k - 1]) != state))
        .collect()
}

/// The rows on which Claw loses health, with the loss.
fn hurts(rows: &[Look]) -> Vec<(usize, i32)> {
    (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .map(|k| (k, rows[k - 1].health - rows[k].health))
        .collect()
}

#[test]
fn install_level14_the_tiger_lunges_on_the_original_schedule() {
    let Some(level) = level14() else { return };
    let rows = watch(
        &level,
        "tiger-stand",
        700,
        FLOOR_TIGER.logic,
        FLOOR_TIGER.at,
    );
    let lunge = EnemyState::LungeAttack as i32;
    let starts = entries(&rows, lunge);
    assert!(starts.contains(&209), "{starts:?}");
    let s = 209;
    // The crouch: still, on its crouch picture, S to S + 27.
    for (k, l) in rows_in(&rows, s, s + 27) {
        assert_eq!(l.frame, tiger_guard::CROUCH_FRAME, "row {k}");
        assert_eq!(l.obj.unwrap().x, rows[s].obj.unwrap().x, "row {k}");
    }
    // The dash from S + 28, the claws out S + 35 to S + 46, Home on S + 47.
    assert!(rows[s + 28].obj.unwrap().x < rows[s + 27].obj.unwrap().x);
    for (k, l) in rows_in(&rows, s + 35, s + 46) {
        assert_eq!(l.frame, tiger_guard::LUNGE_FRAME, "row {k}");
        assert_eq!(state_of(l), lunge, "row {k}");
    }
    assert_eq!(state_of(&rows[s + 47]), EnemyState::Home as i32);
    let moved = rows[s].obj.unwrap().x - rows[s + 46].obj.unwrap().x;
    assert!((100..=104).contains(&moved), "{moved}");
}

#[test]
fn install_level14_a_blast_on_a_lunging_tiger_makes_it_block_unhurt() {
    let Some(level) = level14() else { return };
    let rows = watch(&level, "tiger-dyn", 200, FLOOR_TIGER.logic, FLOOR_TIGER.at);
    let block = EnemyState::Block as i32;
    assert_eq!(entries(&rows, block), [125]);
    assert_eq!(state_of(&rows[124]), EnemyState::LungeAttack as i32);
    for (k, l) in rows_in(&rows, 125, 138) {
        assert_eq!(state_of(l), block, "row {k}");
    }
    assert_eq!(state_of(&rows[139]), EnemyState::Home as i32);
    assert!(
        rows.iter()
            .all(|l| l.obj.unwrap().health == tiger_guard::HEALTH)
    );
    // Its lunge's claws cost Claw 20 before the stick.
    assert!(hurts(&rows).contains(&(119, tiger_guard::LUNGE_DAMAGE)));
}

#[test]
fn install_level14_the_hand_throws_its_bursts_and_rests() {
    let Some(level) = level14() else { return };
    let rows = watch(&level, "hand", 600, HAND.logic, HAND.at);
    let throwing = hand::state::THROWING;
    // Claw's box reaches the hand's on row 139: it throws until 343, rests
    // 28 ticks and throws again from 371.
    assert_eq!(entries(&rows, throwing)[..2], [139, 371]);
    assert!(rows[139].obj.unwrap().out);
    assert_eq!(entries(&rows, hand::state::IDLE)[..2], [0, 343]);
    assert!(!rows[343].obj.unwrap().out);
    // Its balls leave on the tenth picture of each rise: T + 46, + 114,
    // + 182.
    let tens: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].frame == hand::THROW_FRAME && rows[k - 1].frame != hand::THROW_FRAME)
        .collect();
    assert_eq!(tens[..3], [185, 253, 321]);
}

#[test]
fn install_level14_a_delayed_geyser_holds_its_column_after_a_lost_life() {
    let Some(level) = level14() else { return };
    let [first, delayed] = GEYSERS;
    let a = watch(&level, "geyser-lake", 400, first.logic, first.at);
    let b = watch(&level, "geyser-lake", 400, delayed.logic, delayed.at);
    let erupting = geyser::state::ERUPTING;
    // Built on 61, the first erupts on 62, its column live from 63.
    assert_eq!(entries(&a, geyser::state::IDLE)[0], 61);
    assert_eq!(entries(&a, erupting)[0], 62);
    assert_eq!(a[62].attack, None);
    assert_eq!(a[63].attack, geyser::column(1));
    // The delayed one: built on 93, erupting on 121.
    assert_eq!(entries(&b, geyser::state::IDLE)[0], 93);
    assert_eq!(entries(&b, erupting)[0], 121);
    // Claw dies in the lava; on the lives step (306) the fountains reset:
    // the delayed one waits from 307 to 333 showing its last picture with
    // that picture's column live, and erupts again on 334.
    let lost = (1..b.len()).find(|&k| b[k].lives < b[k - 1].lives).unwrap();
    assert_eq!(lost, 306);
    let held = b[lost].attack;
    assert_eq!(held, geyser::column(3));
    for (k, l) in rows_in(&b, lost + 1, lost + 27) {
        assert_eq!(state_of(l), geyser::state::IDLE, "row {k}");
        assert_eq!(l.attack, held, "row {k}");
    }
    assert_eq!(state_of(&b[lost + 28]), erupting);
}

#[test]
fn install_level14_the_tiger_slashes_on_the_original_schedule() {
    let Some(level) = level14() else { return };
    let rows = watch(
        &level,
        "tiger-stand",
        700,
        FLOOR_TIGER.logic,
        FLOOR_TIGER.at,
    );
    let knife = EnemyState::KnifeAttack as i32;
    // The slash after the front flip from row 509: entered on 532, its
    // first picture S + 1 to S + 4, the cutting picture (its box live)
    // S + 5 to S + 10, the last S + 11 to S + 16, Home on S + 17.
    assert_eq!(entries(&rows, EnemyState::FrontFlip as i32)[0], 509);
    let s = 532;
    assert_eq!(entries(&rows, knife)[0], s);
    for (k, l) in rows_in(&rows, s + 1, s + 4) {
        assert_eq!(l.frame, 201, "row {k}");
    }
    for (k, l) in rows_in(&rows, s + 5, s + 10) {
        assert_eq!(l.frame, tiger_guard::KNIFE_FRAME, "row {k}");
    }
    for (k, l) in rows_in(&rows, s + 11, s + 16) {
        assert_eq!(l.frame, 203, "row {k}");
        assert_eq!(state_of(l), knife);
    }
    assert_eq!(state_of(&rows[s + 17]), EnemyState::Home as i32);
    // The slash costs Claw 15 on its first live tick.
    assert!(hurts(&rows).contains(&(s + 5, tiger_guard::KNIFE_DAMAGE)));
    // Through the front flip, nothing reaches the tiger.
    for (k, l) in rows_in(&rows, 509, s - 1) {
        assert!(l.obj.unwrap().out, "row {k}");
    }
}

/// A lava ball on one row: where it is, its state, and whether its blast
/// can hurt (in pair collision, not kept from hurting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BallAt {
    x: i32,
    y: i32,
    state: i32,
    live: bool,
}

#[test]
fn install_level14_a_lava_ball_flies_rests_and_bursts_at_its_fuse() {
    let Some(level) = level14() else { return };
    let r = route_named("hand");
    let mut sim = at_time_zero(&level, "c18", r, t0_ms("c18"), SEED);
    let ticks = route_ticks(r);
    let mut first: Option<ObjectId> = None;
    let mut rows: Vec<Option<BallAt>> = Vec::new();
    for k in 0..330 {
        step_row(&mut sim, r, &ticks, k);
        if first.is_none() {
            first = sim
                .objects()
                .all()
                .find(|(_, o)| &*o.logic == ball::NAMES[0])
                .map(|(id, _)| id);
        }
        rows.push(first.and_then(|id| sim.objects().get(id)).map(|o| BallAt {
            x: o.x,
            y: o.y,
            state: o.state,
            live: o.flags & flags::HARMLESS == 0,
        }));
    }
    // Made on 185 at the hand, it moves from 186: 7 px across and 13 up on
    // its first step, towards the side Claw stood on (left).
    let made = rows.iter().position(Option::is_some).unwrap();
    assert_eq!(made, 185);
    assert_eq!(rows[185].map(|b| (b.x, b.y)), Some(HAND.at));
    assert_eq!(rows[186].map(|b| (b.x, b.y)), Some((20780, 3171)));
    // It lands in the pit 51 ticks after it was made and rests there.
    let rest = rows[236].unwrap();
    assert_eq!(rows[240].map(|b| (b.x, b.y)), Some((rest.x, rest.y)));
    assert_eq!(rest.y, 3376);
    // Its fuse: 1000 ms, the burst on 241, live on that tick and on the
    // ninth tick of the burst's own (250, its animation's mark), kept from
    // hurting on the others, for 73 ticks.
    let bursts: Vec<usize> = (made..rows.len())
        .filter(|&k| rows[k].is_some_and(|b| b.state == ball::state::EXPLODING))
        .collect();
    assert_eq!(bursts.first(), Some(&241));
    assert_eq!(bursts.len() as u64, ball::BLAST_TICKS);
    let live: Vec<usize> = bursts
        .iter()
        .copied()
        .filter(|&k| rows[k].unwrap().live)
        .collect();
    assert_eq!(live, [241, 250]);
    assert!(rows[241 + ball::BLAST_TICKS as usize].is_none(), "gone");
}
