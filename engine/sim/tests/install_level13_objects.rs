//! The thirteenth level's Chameleons, water rock and the two dynamite
//! throws of the speedrun on your own install, against the original's
//! runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), at the height he settles at there, waits until the level
//! clock reaches the time the original's run started at, and starts the
//! shared random stream from the run's state, once. The routes:
//!
//! - on the ledge at 1650, 132 px left of the Chameleon placed at (1782,
//!   1900) with no patrol limits: Claw stands; the lizard, carried toward
//!   the ledge's end by its first phase before the route starts, is
//!   carried off it by its second, falls and stays hidden;
//! - on the floor at 2600, 173 px right of the Chameleon placed at (2427,
//!   2416) with no patrol limits: Claw stands; its tongue lashes empty air
//!   and its phases leave it where it is, each moving its left limit;
//! - on the platform at 3600 left of the water rock at (3744, 2444): Right
//!   on rows 15 to 44 walks Claw off the platform onto the rock, which
//!   throws him 600 px up, again and again;
//! - right of the bear's block at 4250, facing right: Dynamite held on rows
//!   20 to 54 throws a stick down onto the floor of the Chameleon and the
//!   pirate below, whose blast kills both; Right from row 150 walks Claw
//!   off into the water;
//! - at the lip of the waterfall shaft at 26000, facing right: Dynamite
//!   held on rows 20 to 84 throws a stick down the shaft, whose blast kills
//!   the Chameleon and the pirate at its foot; Right from row 110 takes
//!   Claw down the shaft and into the water.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched lizards, rock and pirates are compared
//! too at the 18 ms step: their position, their state (lined up by the
//! numbers the run gives them, [`reference_state`]), their health,
//! whether they are out of every collision and whether they are hidden.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace, or the reference runs (or a run's
//! `obj-log.csv`, or a watched object in it) are missing; `CLAW_L13_ONLY`
//! names one store to play alone, and `CLAW_L13_TRACE` a folder to write
//! the replay's traces to. Nothing from an install or a reference store is
//! kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, chameleon, redtail_pirate};
use claw_sim::logic::springboard;
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

const fn lizard(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: chameleon::NAMES[0],
        serial,
        at,
    }
}

const fn pirate(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: redtail_pirate::NAMES[0],
        serial,
        at,
    }
}

/// A Chameleon that the original's run shows hidden in a phase at the
/// route's time zero (it phased while Claw stood still before it): the
/// one placed at `at`, put at `x` with its left patrol limit `left`,
/// facing right when `right`, its hidden wait ending on route row
/// `visible.0` at the 18 ms step and on frame `visible.1` at 60 frames per
/// second (the first row on which the run shows it drawn).
#[derive(Debug, Clone, Copy)]
struct Phased {
    at: (i32, i32),
    x: i32,
    left: i32,
    right: bool,
    visible: (u64, u64),
}

/// A route of the stores: its name, where Claw is put down (at the height
/// he settles at), how many rows it has, its stretches, the clocks it was
/// run at, the Chameleons put in step at time zero and the objects it
/// watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
    phased: &'static [Phased],
    watch: &'static [Watched],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The original's game time at the routes' time zero at the 18 ms step.
const T0_MS: u64 = 828;

/// The unbounded Chameleon on the ledge left of the level's start.
const LEDGE_LIZARD: Watched = lizard("60", (1782, 1900));
/// The unbounded Chameleon on the floor right of it.
const FLOOR_LIZARD: Watched = lizard("31", (2427, 2416));
/// The water rock.
const ROCK: Watched = Watched {
    logic: springboard::WATER_ROCK_NAME,
    serial: "2473",
    at: (3744, 2444),
};
/// The Chameleon on the ledge over the water rock.
const ROCK_LIZARD: Watched = lizard("32", (3545, 2089));
/// The Chameleon and the pirate below the bear's block.
const THROW_A: [Watched; 2] = [lizard("121", (4409, 2169)), pirate("123", (4487, 2123))];
/// The Chameleon and the pirate at the foot of the waterfall shaft.
const THROW_B: [Watched; 2] = [lizard("144", (26171, 3629)), pirate("152", (26021, 3595))];

/// The ledge lizard at time zero: its first phase, made while Claw stood
/// still before the route's time zero, took it 238 px left toward the
/// ledge's end (its target far left, the floor ending under it).
const LEDGE_PHASED: Phased = Phased {
    at: LEDGE_LIZARD.at,
    x: 1544,
    left: -2_147_483_647,
    right: false,
    visible: (15, 14),
};
/// The floor lizard at time zero: its first phase left it where it was,
/// with the target written into its left limit.
const FLOOR_PHASED: Phased = Phased {
    at: FLOOR_LIZARD.at,
    x: 2413,
    left: -2_147_483_646,
    right: true,
    visible: (59, 63),
};
/// The lizard over the water rock at time zero, in its range.
const ROCK_PHASED: Phased = Phased {
    at: ROCK_LIZARD.at,
    x: 3508,
    left: 3456,
    right: true,
    visible: (15, 14),
};

const ROUTES: [Route; 5] = [
    Route {
        store: "cham-unbounded-a",
        start: (1650, 1879),
        rows: 500,
        spans: &[],
        clocks: &["c18", "c60"],
        phased: &[LEDGE_PHASED],
        watch: &[LEDGE_LIZARD],
    },
    Route {
        store: "cham-unbounded-b",
        start: (2600, 2391),
        rows: 600,
        spans: &[],
        clocks: &["c18", "c60"],
        phased: &[FLOOR_PHASED],
        watch: &[FLOOR_LIZARD],
    },
    Route {
        store: "rock44",
        start: (3600, 2071),
        rows: 300,
        spans: &[span(15, 44, RIGHT)],
        clocks: &["c18", "c60"],
        phased: &[ROCK_PHASED],
        watch: &[ROCK, ROCK_LIZARD],
    },
    Route {
        store: "dyn-throw-a",
        start: (4250, 1879),
        rows: 450,
        spans: &[
            span(15, 16, RIGHT),
            span(20, 54, DYNAMITE),
            span(150, 449, RIGHT),
        ],
        clocks: &["c18", "c60"],
        phased: &[],
        watch: &THROW_A,
    },
    Route {
        store: "dyn-throw-b",
        start: (26000, 2903),
        rows: 450,
        spans: &[
            span(15, 16, RIGHT),
            span(20, 84, DYNAMITE),
            span(110, 449, RIGHT),
        ],
        clocks: &["c18", "c60"],
        phased: &[],
        watch: &THROW_B,
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. The floor lizard's route,
/// the water rock's and the second throw's follow their stores exactly at
/// the 18 ms step, the random stream included, the floor lizard's at 60
/// frames per second too. The rest:
///
/// - cham-unbounded-a, 18 ms: the ledge lizard leaves its ledge on row 137
///   as in the original's run (70 px left, then it falls), but the
///   original's lizard leaves it with the small upward speed it kept while
///   standing (about 36 px/s; an enemy that stands keeps its speed up or
///   down in the original, which this build zeroes), so it rises a pixel
///   first and lands later: its height differs from row 138 on, it leaves
///   Claw's range boxes (40 px up and down) a tick earlier here, so its
///   out-of-range idle and that idle's draws come a tick earlier and the
///   random stream follows from row 143; with other draws its idle paces
///   it off its place later on (row 405).
/// - dyn-throw-a, 18 ms: the stick leaves Claw on row 61 as in the
///   original's run, but the original runs its first update only on row
///   62 (it is put into the update list right behind Claw, whose update
///   made it, and the update pass skips an object put there; this build
///   runs it in the same pass), so its fuse runs out and it blows up a
///   tick later there, one more bounce lower (4485, 2167 against 4483,
///   2158): the random stream differs from the blast (row 132) on, and the
///   Chameleon and the pirate, killed by the blast with the same damage
///   (to -14 and -5), die a tick earlier here (row 140 against 141), with
///   the camera's jolt and their coins following.
/// - At 60 frames per second the replay plays 18 ms rows and does not
///   follow the original's random stream: Claw's height differs by up to
///   10 px on rows of the falls onto and off the water rock (the rows meet
///   the fall at other moments), and whatever random draws decide comes
///   out otherwise: the ledge lizard's tongue meets Claw on another call
///   (cham-unbounded-a), the lizard over the water rock lashes him in the
///   air on other throws (rock44), and the coins the killed pirates drop
///   fly elsewhere, so Claw walks through other coins and into the water
///   at another moment (dyn-throw-a, dyn-throw-b).
const KNOWN: &[Known] = &[
    (
        "cham-unbounded-a",
        "c18",
        &[
            ("60_y", 2502),
            ("rng", 2592),
            ("60_state", 2592),
            ("60_x", 7308),
        ],
    ),
    (
        "cham-unbounded-a",
        "c60",
        &[
            ("state_id", 1117),
            ("health", 1733),
            ("x", 1767),
            ("cam_x", 1767),
        ],
    ),
    (
        "rock44",
        "c60",
        &[
            ("y", 983),
            ("cam_y", 983),
            ("state_id", 2800),
            ("physics_type", 2800),
            ("health", 5267),
            ("x", 7217),
            ("cam_x", 7217),
            ("score", 7283),
        ],
    ),
    (
        "dyn-throw-a",
        "c18",
        &[
            ("rng", 2394),
            ("cam_x", 2538),
            ("cam_y", 2538),
            ("121_state", 2538),
            ("121_health", 2538),
            ("123_state", 2538),
            ("123_health", 2538),
            ("121_x", 2556),
            ("121_y", 2556),
            ("123_x", 2556),
            ("123_y", 2556),
            ("score", 3402),
            ("123_out", 3546),
            ("123_hidden", 3546),
        ],
    ),
    (
        "dyn-throw-a",
        "c60",
        &[
            ("score", 3433),
            ("y", 4367),
            ("cam_y", 4367),
            ("x", 8100),
            ("cam_x", 8100),
        ],
    ),
    (
        "dyn-throw-b",
        "c60",
        &[
            ("score", 2550),
            ("y", 2783),
            ("cam_y", 2783),
            ("x", 3150),
            ("cam_x", 3150),
            ("lives", 6650),
            ("state_id", 6667),
            ("physics_type", 6667),
            ("health", 6667),
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

/// The rows of a store's own copy of a route, (held, pressed) by row.
fn store_route(dir: &Path) -> Option<Vec<(u32, u32)>> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    for l in text.lines() {
        assert!(!l.starts_with('@'), "{l}: no directive is expected");
        if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some(rows)
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

fn level13() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(13)).unwrap();
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

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
    let mut sim = Sim::new(level.clone(), config(clock, seed));
    sim.step(TickInput::default());
    sim.place_between_ticks(r.start.0, r.start.1);
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
        put_in_step(&mut sim, r, clock, t0_ms);
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// Puts the Chameleons of `r` in step with the original's run at time
/// zero (see [`Phased`]), on every tick before time zero once they are
/// built: hidden in the phase at their place and limit, out of pair
/// collision, showing the phase picture with the fade at 0, their hidden
/// wait ending as the run shows.
fn put_in_step(sim: &mut Sim, r: &Route, clock: &str, t0_ms: u64) {
    for p in r.phased {
        let id = placed(sim, chameleon::NAMES[0], p.at);
        if sim
            .objects()
            .get(id)
            .is_none_or(|o| EnemyState::from_id(o.state).is_none())
        {
            continue;
        }
        let visible_ms = if clock == "c18" {
            t0_ms + p.visible.0 * TICK
        } else {
            t0_ms + (p.visible.1 * 50).div_ceil(3)
        };
        sim.with_world(|w| {
            enemy::set_state(w, id, EnemyState::PhaseIn);
            enemy::set_facing(w, id, p.right);
            if let Some(o) = w.objects.get_mut(id) {
                o.x = p.x;
                o.frame = chameleon::PHASE_FRAME;
                o.flags |= flags::NO_COLLIDE;
                o.draw_flags |= draw_flags::HIDDEN;
            }
            if let Some(d) = enemy::data_mut(w, id) {
                d.bounds.0 = p.left;
                d.vx = 0.0;
            }
            if let Some(c) = chameleon::chameleon_data_mut(w, id) {
                c.amount = 0;
                c.hidden = enemy::Timer::new(0, visible_ms);
            }
        });
        sim.with_world(|w| w.set_chameleon_phase(true, 0));
    }
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
/// this build's numbers: a water rock's states are the same numbers; a
/// lizard's or a pirate's: idle, strike, dead, pause, home, advance, run,
/// pace, the two hit reactions, the hop back and the two fades.
fn reference_state(logic: &str, n: i32) -> Option<i32> {
    if logic == springboard::WATER_ROCK_NAME {
        return Some(n);
    }
    let s = match n {
        2000 => EnemyState::Idle,
        2003 => EnemyState::Strike,
        2004 => EnemyState::Dead,
        2011 => EnemyState::Pause,
        2018 => EnemyState::Home,
        2020 => EnemyState::JumpBack,
        2021 => EnemyState::Advance,
        2022 => EnemyState::FastAdvance,
        2023 => EnemyState::Pace,
        2024 => EnemyState::HitLow,
        2025 => EnemyState::HitHigh,
        2104 => EnemyState::PhaseOut,
        2105 => EnemyState::PhaseIn,
        _ => return None,
    };
    Some(s as i32)
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
        out[i].push((
            f[tick].parse().ok()?,
            ObjAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(watch[i].logic, int(f[st])?),
                health: int(f[hp]).unwrap_or(0),
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

/// Whether the runs log a health for objects of `logic` (the water rock
/// has none).
fn logic_has_health(logic: &str) -> bool {
    logic != springboard::WATER_ROCK_NAME
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
    let dir = root.join("level13").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, T0_MS, "{}: time zero", r.store);
    }
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
        sim.step(ticks.get(k).copied().unwrap_or_default());
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
    if clock == "c18" {
        let runs = runs_root(root);
        let meta = std::fs::read_to_string(root.join("level13").join(store).join("meta.toml"))
            .expect("meta");
        let id = run_id(&meta, clock).unwrap_or_else(|| panic!("{store}: no run id"));
        let log = runs.join(&id).join("obj-log.csv");
        let reference = reference_objects(&log, r.watch)
            .unwrap_or_else(|| panic!("{store}: {} lacks a watched object", log.display()));
        for (i, w) in r.watch.iter().enumerate() {
            firsts.extend(compare_object(w.serial, &p.objects[i], &reference[i]));
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

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level13").join(r.store);
        let rows =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        // Each clock the route lists has its meta, its start and its
        // trace; none of these stores is a diagnostic one.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        assert_eq!(meta_value(&meta, "", "diagnostic"), None, "{}", r.store);
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let teleport = meta_value(&meta, clock, "teleport")
                .unwrap_or_else(|| panic!("{} {clock}: no start", r.store));
            let x: i32 = teleport.split(',').next().unwrap().parse().unwrap();
            assert_eq!(x, r.start.0, "{} {clock}: start", r.store);
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_level13_object_routes_follow_the_reference_traces() {
    let Some(level) = level13() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L13_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level13/{} {clock}", r.store);
            let p = play(&level, &root, r, clock);
            if let Some(out) = std::env::var_os("CLAW_L13_TRACE") {
                let out = PathBuf::from(out).join(format!("{}-{clock}.csv", r.store));
                std::fs::write(&out, trace::to_csv(&p.rows)).expect("the trace is written");
            }
            failures.extend(new_divergences(&root, r, clock, &p));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw, his state and health, and the
/// watched object with its patrol limits.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    obj: Option<ObjAt>,
    bounds: (i32, i32),
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching the object of
/// logic `logic` placed at `at`.
fn watch(level: &SimLevel, store: &str, rows: usize, logic: &str, at: (i32, i32)) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, T0_MS, SEED);
    let id = placed(&sim, logic, at);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        let bounds = sim
            .objects()
            .get(id)
            .and_then(enemy::data_of)
            .map_or(enemy::UNBOUNDED, |d| d.bounds);
        looks.push(Look {
            x: c.x,
            y: c.y,
            state: c.state,
            health: sim.claw().stats.health,
            obj: obj_at(&sim, id),
            bounds,
        });
    }
    looks
}

fn state_of(l: &Look) -> i32 {
    l.obj.and_then(|o| o.state).unwrap_or(-1)
}

/// The rows on which the watched object enters `state`.
fn entries(rows: &[Look], state: EnemyState) -> Vec<usize> {
    let s = state as i32;
    (0..rows.len())
        .filter(|&k| state_of(&rows[k]) == s && (k == 0 || state_of(&rows[k - 1]) != s))
        .collect()
}

#[test]
fn install_level13_the_floor_lizard_phases_on_calls_and_stays_in_place() {
    let Some(level) = level13() else { return };
    let rows = watch(
        &level,
        "cham-unbounded-b",
        600,
        chameleon::NAMES[0],
        FLOOR_LIZARD.at,
    );
    // Hidden from time zero until row 59, it fades in over 26 calls and
    // goes Home on the 26th (row 84), touchable again.
    assert!(rows[58].obj.unwrap().hidden && !rows[59].obj.unwrap().hidden);
    assert_eq!(state_of(&rows[83]), EnemyState::PhaseIn as i32);
    assert_eq!(state_of(&rows[84]), EnemyState::Home as i32);
    assert!(rows[83].obj.unwrap().out && !rows[84].obj.unwrap().out);
    // Its tongue: 16 calls from row 85, lashing empty air 173 px away.
    let strikes = entries(&rows, EnemyState::Strike);
    assert_eq!(strikes[0], 85);
    assert_eq!(state_of(&rows[100]), EnemyState::Strike as i32);
    assert_eq!(state_of(&rows[101]), EnemyState::Home as i32);
    assert!(rows.iter().all(|l| l.health == 100));
    // Its phases: 26 fade-out calls from the decision's next row, the jump
    // on the 27th; it never moves, and each jump writes its target into
    // its left limit, from -2147483646 up a few pixels at a time.
    let outs = entries(&rows, EnemyState::PhaseOut);
    let ins = entries(&rows, EnemyState::PhaseIn);
    assert_eq!(outs[0], 157);
    assert_eq!(
        ins.iter().filter(|&&k| k > 0).copied().collect::<Vec<_>>()[..3],
        [183, 318, 510]
    );
    for (&o, &i) in outs.iter().zip(ins.iter().filter(|&&k| k > 0)) {
        assert_eq!(i - o, 26, "a fade out from row {o}");
    }
    assert!(rows.iter().all(|l| l.obj.unwrap().x == 2413));
    assert_eq!(rows[182].bounds.0, -2_147_483_646);
    assert_eq!(rows[183].bounds.0, -2_147_483_645);
    assert_eq!(rows[318].bounds.0, -2_147_483_643);
    assert_eq!(rows[510].bounds.0, -2_147_483_642);
    // The hidden wait in whole steps: 494 ms after row 183 it is drawn
    // again on row 211 (ceil(494 / 18) = 28 calls), Home on row 236.
    assert!(rows[210].obj.unwrap().hidden && !rows[211].obj.unwrap().hidden);
    assert_eq!(state_of(&rows[235]), EnemyState::PhaseIn as i32);
    assert_eq!(state_of(&rows[236]), EnemyState::Home as i32);
}

#[test]
fn install_level13_the_ledge_lizard_is_carried_off_its_ledge_and_stays_hidden() {
    let Some(level) = level13() else { return };
    let rows = watch(
        &level,
        "cham-unbounded-a",
        500,
        chameleon::NAMES[0],
        LEDGE_LIZARD.at,
    );
    // The second phase fades out on rows 111 to 136; the jump toward its
    // far-left target carries it 70 px left, past its ledge's end at 1528,
    // and it falls.
    assert_eq!(entries(&rows, EnemyState::PhaseOut), [111]);
    assert_eq!(rows[136].obj.unwrap().x, 1544);
    let fell = rows[137].obj.unwrap();
    assert_eq!(
        (fell.x, fell.state),
        (1474, Some(EnemyState::PhaseIn as i32))
    );
    assert!(fell.hidden && fell.out);
    // It lands on the floor below at 2022 and, Claw out of its range, the
    // out-of-range idle ends the phase: hidden and untouchable to the end.
    let landed = (137..rows.len())
        .find(|&k| rows[k].obj.unwrap().y == 2022)
        .expect("landed");
    assert!(landed < 160, "{landed}");
    let idle = entries(&rows, EnemyState::Idle);
    assert!(idle.iter().any(|&k| (137..160).contains(&k)), "{idle:?}");
    for l in &rows[160..] {
        let o = l.obj.unwrap();
        assert!(o.hidden && o.out, "{o:?}");
    }
}

#[test]
fn install_level13_the_water_rock_throws_claw_600_px_up() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "rock44", 300, springboard::WATER_ROCK_NAME, ROCK.at);
    let rock = |k: usize| rows[k].obj.unwrap().state.unwrap();
    // Claw lands on its surface (standing at 2473, 61 px above the
    // water's Death row) on row 59 (L); the rock sees him on L + 1.
    assert_eq!((rows[59].y, rows[58].y), (2473, 2460));
    assert_eq!(rock(59), springboard::state::IDLE);
    assert_eq!(rock(60), springboard::state::CLAW_SPRING);
    // The throw on L + 21, to an apex 600 px above where he stood (L + 54).
    assert_eq!(rows[79].y, 2473);
    assert!(rows[80].y < 2473);
    let apex = (80..140).min_by_key(|&k| rows[k].y).unwrap();
    assert_eq!((apex, rows[apex].y), (113, 1873));
    // Idle again on T + 47, and the next landing (row 158) throws again.
    assert_eq!(rock(106), springboard::state::CLAW_SPRING);
    assert_eq!(rock(107), springboard::state::IDLE);
    assert_eq!(rock(159), springboard::state::CLAW_SPRING);
    assert!(rows[179].y < 2473 && rows[178].y == 2473);
}

#[test]
fn install_level13_a_stick_down_the_waterfall_shaft_kills_the_two_below() {
    let Some(level) = level13() else { return };
    let lizard = watch(&level, "dyn-throw-b", 200, THROW_B[0].logic, THROW_B[0].at);
    let pirate = watch(&level, "dyn-throw-b", 200, THROW_B[1].logic, THROW_B[1].at);
    // The blast on row 161 deals 15 to each: both are thrown away dead on
    // row 170; neither is hit before it, so the pirate's parry of the
    // thirteenth level never sees the stick.
    for (rows, health) in [(&lizard, -14), (&pirate, -5)] {
        let dead = entries(rows, EnemyState::Dead);
        assert_eq!(dead, [170]);
        assert_eq!(rows[170].obj.unwrap().health, health);
        // Woken as Claw comes down the shaft, each is whole up to the blast.
        assert!(
            rows[150..170]
                .iter()
                .all(|l| l.obj.unwrap().health == health + 15)
        );
    }
    assert!(entries(&pirate, EnemyState::Block).is_empty());
}

#[test]
fn install_level13_a_stick_from_the_bears_block_kills_the_two_below() {
    let Some(level) = level13() else { return };
    let lizard = watch(&level, "dyn-throw-a", 200, THROW_A[0].logic, THROW_A[0].at);
    let pirate = watch(&level, "dyn-throw-a", 200, THROW_A[1].logic, THROW_A[1].at);
    // The same two kills with the same damage, a tick earlier than in the
    // original's run (see [`KNOWN`]).
    for (rows, health) in [(&lizard, -14), (&pirate, -5)] {
        assert_eq!(entries(rows, EnemyState::Dead), [140]);
        assert_eq!(rows[140].obj.unwrap().health, health);
    }
    // Claw, on the step above, is never hurt.
    assert!(lizard.iter().all(|l| l.health == 100));
}
