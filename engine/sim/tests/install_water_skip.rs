//! The twelfth level's water skip on your own install, against the
//! original's runs of the same routes.
//!
//! The pool left of the water's-edge ledge kills Claw the moment his
//! centre pixel's row under his feet is one of its Death rows, but under
//! it lies a hidden solid floor, the floor of the passage under the water
//! that leads to the boss. Nothing in the game opens it on purpose; the
//! skip comes out of rules every level shares:
//!
//! - a stick of dynamite goes off at once when play comes back from a
//!   help screen that, with the fuse already burnt, was up long enough
//!   (the help screen's time reaches the fuse);
//! - its blast hurts Claw in the air (15), holds him in place for 22
//!   ticks, and on the 22nd he stands in mid air; a jump on the next tick
//!   carries him on;
//! - a blast hurts him again only once his 850 ms of immunity are over;
//! - the liquid death reads the one pixel at his centre column;
//! - a landing needs one of its samples (X + 4 or X + 14) over a floor.
//!
//! From the ledge at (2900, 3607) two of Claw's own sticks, each set off
//! by a help screen, carry him west over the pool with two jumps from mid
//! air; with Left released over its left end he falls straight down past
//! the Death rows (his centre column left of the pool's first column, x
//! 2560) onto the hidden floor at y 3799 and walks the passage. The
//! window is x 2546 to 2559: further left every landing sample misses the
//! floor and he falls through the open column; from x 2560 the pool kills
//! him.
//!
//! The routes (the original's runs of the same, at the 18 ms step and at
//! 60 frames per second):
//!
//! - skip-water-skip-c18 and skip-water-skip: the whole skip, Left
//!   released at x 2558, then Right along the passage;
//! - skip-window-2546-c18, -2542-c18, -2562-c18: Left released at x
//!   2546 (lands and teeters at the floor's end), 2542 (falls through)
//!   and 2562 (dies in the pool);
//! - skip-window-2545: at 60 frames per second, released at x 2545 (falls
//!   through).
//!
//! Each route puts Claw down at (2900, 3567) after the level's first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does); he settles on the ledge at y 3607. That point is inside
//! the trigger of the three stalactites over the ledge, so they let go
//! while he settles and have crumbled by the route's time zero, as in the
//! original's runs. The route waits until the level clock reaches the
//! time the original's run started at and starts the shared random stream
//! from the run's state, once.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, ammunition, the
//! camera, the random stream and the events), at 60 frames per second with
//! the default tuning within the feel tolerances. When the reference run
//! also lists the objects near Claw per step (`obj-log.csv` beside the
//! run, in the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next
//! to the reference store), the three stalactites are compared too at the
//! 18 ms step: their position, state, whether they are out of every
//! collision and whether they are hidden.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them, its help screens included.
//! Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each
//! test skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but a listed store lacks its route, meta or
//! trace (or, with the reference runs there, a run's `obj-log.csv` or a
//! stalactite in it); `CLAW_SKIP_ONLY` names one store to play alone, and
//! `CLAW_SKIP_TRACE` a folder to write the replay's traces to. Nothing
//! from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::stalactite;
use claw_sim::object::{ObjectId, draw_flags, flags};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The dynamite button.
const DYNAMITE: u32 = Buttons::DYNAMITE.0;

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

/// A route of the stores: its name, how many rows it has, its stretches,
/// its help screens (before row, wall time ms) and the clock it was run
/// at.
struct Route {
    store: &'static str,
    rows: usize,
    spans: &'static [Span],
    helps: &'static [(usize, u32, usize)],
    clock: &'static str,
}

/// Where every route puts Claw down; he settles 40 px lower, on the ledge.
const START: (i32, i32) = (2900, 3567);
/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The original's game time at the routes' time zero at the 18 ms step.
const T0_MS: u64 = 828;

/// The first part of every route at the 18 ms step: Left turns Claw round
/// (rows 20, 21), a stick thrown standing (30, 31), Left to the ledge's
/// end and the jump from it (54); the first help screen sets the stick off
/// under him (69), its blast hits him on row 77, he stands in mid air on
/// row 99 and jumps on 100; a stick thrown in the air (118) and the second
/// help screen (120) hit him on 128; he stands on 150 and jumps on 151.
const SKIP_HELPS_C18: [(usize, u32, usize); 2] = [(69, 3200, 2), (120, 4350, 2)];
/// The same at 60 frames per second, with its own rows.
const SKIP_HELPS_C60: [(usize, u32, usize); 2] = [(69, 3200, 2), (118, 4350, 2)];

/// The skip at the 18 ms step with Left released after row `last_left`.
const fn skip_c18(last_left: usize) -> [Span; 7] {
    [
        span(20, 21, LEFT),
        span(30, 31, DYNAMITE),
        span(52, last_left, LEFT),
        span(54, 71, JUMP),
        span(100, 117, JUMP),
        span(118, 119, DYNAMITE),
        span(151, 168, JUMP),
    ]
}

const SKIP_C18: [Span; 8] = with_walk(skip_c18(179), 899);
const WINDOW_2546_C18: [Span; 8] = with_walk(skip_c18(182), 299);
const WINDOW_2542_C18: [Span; 8] = with_walk(skip_c18(183), 299);
const WINDOW_2562_C18: [Span; 8] = with_walk(skip_c18(178), 299);

/// The skip at 60 frames per second with Left released after row
/// `last_left` and the last jump held to row `last_jump`.
const fn skip_c60(last_left: usize, last_jump: usize) -> [Span; 7] {
    [
        span(20, 21, LEFT),
        span(30, 31, DYNAMITE),
        span(52, last_left, LEFT),
        span(54, 71, JUMP),
        span(98, 115, JUMP),
        span(116, 117, DYNAMITE),
        span(147, last_jump, JUMP),
    ]
}

const SKIP_C60: [Span; 8] = with_walk(skip_c60(162, 164), 899);
const WINDOW_2545: [Span; 8] = with_walk(skip_c60(165, 163), 299);

/// `spans` and Right from row 215 to row `last`: the walk along the
/// passage.
const fn with_walk(spans: [Span; 7], last: usize) -> [Span; 8] {
    [
        spans[0],
        spans[1],
        spans[2],
        spans[3],
        spans[4],
        spans[5],
        spans[6],
        span(215, last, RIGHT),
    ]
}

const ROUTES: [Route; 6] = [
    Route {
        store: "skip-water-skip-c18",
        rows: 900,
        spans: &SKIP_C18,
        helps: &SKIP_HELPS_C18,
        clock: "c18",
    },
    Route {
        store: "skip-water-skip",
        rows: 900,
        spans: &SKIP_C60,
        helps: &SKIP_HELPS_C60,
        clock: "c60",
    },
    Route {
        store: "skip-window-2546-c18",
        rows: 300,
        spans: &WINDOW_2546_C18,
        helps: &SKIP_HELPS_C18,
        clock: "c18",
    },
    Route {
        store: "skip-window-2542-c18",
        rows: 300,
        spans: &WINDOW_2542_C18,
        helps: &SKIP_HELPS_C18,
        clock: "c18",
    },
    Route {
        store: "skip-window-2562-c18",
        rows: 300,
        spans: &WINDOW_2562_C18,
        helps: &SKIP_HELPS_C18,
        clock: "c18",
    },
    Route {
        store: "skip-window-2545",
        rows: 300,
        spans: &WINDOW_2545,
        helps: &SKIP_HELPS_C60,
        clock: "c60",
    },
];

/// The three stalactites over the ledge (their serial numbers in the
/// original's runs and where they are placed).
const LEDGE_STALACTITES: [(&str, (i32, i32)); 3] = [
    ("2487", (3135, 3422)),
    ("2486", (3197, 3423)),
    ("2485", (3257, 3423)),
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store; a column not listed must stay within it, and one
/// listed must not leave it earlier. At the 18 ms step every route
/// follows its store exactly in Claw's position, state, health and
/// ammunition, the stalactites included, to the end; what differs:
///
/// - At the 18 ms step the stores of the entry window follow to their
///   end, the random stream and the camera included: the return from a
///   help screen runs every object's logic once more, outside any step,
///   and the screen's own frames (two more for each of the two keys
///   pressed on it) count down the 33 ms pulse, as in the original (see
///   `Sim::pass_help_return`). `skip-water-skip-c18`, which goes on past
///   the hidden floor, keeps a difference in the random stream from
///   14184 ms, not looked into here.
/// - At 60 frames per second (skip-water-skip, skip-window-2545) the
///   stores were found for the original's frames of 16 and 17 ms, and the
///   game's 60 fps tuning plays steps of 18 ms. The first blast's attack
///   frame comes 8 steps after the return from help either way, so here it
///   hits Claw one step later and 27 px lower (y 3706 against 3679), and
///   the hit holds him 22 steps either way, 396 ms here against 367 ms
///   there: the jump the route presses on row 98, the first step after the
///   original's stand, comes one step before his stand here and does
///   nothing, and he drops into the pool. The skip itself works at this
///   tuning with its own rows, and the same window
///   ([`install_water_skip_works_at_both_tunings_and_the_release_decides_the_ending`]).
const KNOWN: &[Known] = &[
    ("skip-water-skip-c18", &[("rng", 14184)]),
    (
        "skip-water-skip",
        &[
            ("x", 1416),
            ("y", 1400),
            ("state_id", 1766),
            ("physics_type", 1766),
            ("cam_x", 1433),
            ("cam_y", 1400),
            ("health", 1933),
            ("score", 5716),
            ("lives", 4966),
            ("dynamite", 2100),
            ("events", 4932),
        ],
    ),
    (
        "skip-window-2545",
        &[
            ("x", 1416),
            ("y", 1400),
            ("state_id", 1766),
            ("physics_type", 1766),
            ("cam_x", 1433),
            ("cam_y", 1400),
            ("health", 1933),
            ("lives", 4966),
            ("dynamite", 2100),
            ("events", 4932),
        ],
    ),
];

fn known(store: &str) -> Vec<(&'static str, u64)> {
    KNOWN
        .iter()
        .filter(|(s, _)| *s == store)
        .flat_map(|(_, k)| k.iter().copied())
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

/// The rows of a store's own copy of a route, (held, pressed) by row, and
/// its help screens, (row, wall ms, keys pressed on it).
type StoreRoute = (Vec<(u32, u32)>, Vec<(usize, u32, usize)>);

fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut helps = Vec::new();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix('@') {
            let f: Vec<&str> = rest.split_whitespace().collect();
            assert_eq!(
                (f.get(1), f.get(2)),
                (Some(&"menu"), Some(&"help")),
                "{l}: only help screens are expected"
            );
            let keys = f
                .iter()
                .find_map(|w| w.strip_prefix("keys="))
                .map_or(0, |k| k.split(',').filter(|k| !k.is_empty()).count());
            helps.push((f[0].parse().ok()?, f.get(3)?.parse().ok()?, keys));
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, helps))
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

fn level12() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(12)).unwrap();
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

/// The folder of the reference runs, if there is one.
fn runs_root(reference: &Path) -> Option<PathBuf> {
    let dir = match std::env::var_os("CLAW_REFERENCE_RUNS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => reference.parent()?.join("rt"),
    };
    dir.is_dir().then_some(dir)
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

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, t0_ms: u64, seed: u32) -> Sim {
    put_down_at(level, clock, START, t0_ms, seed)
}

/// The game with Claw put down at `start` after the level's first tick,
/// at time `t0_ms` with the random state `seed` (see the module
/// documentation).
fn put_down_at(level: &SimLevel, clock: &str, start: (i32, i32), t0_ms: u64, seed: u32) -> Sim {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(start));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
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

/// The state a run's number for a stalactite's state stands for, in this
/// build's numbers: it hangs (2069), waits its delay (2072), falls
/// (2066), crumbles (2073) or is gone (2074).
fn reference_state(n: i32) -> Option<i32> {
    Some(match n {
        2069 => stalactite::state::IDLE,
        2072 => stalactite::state::DELAY,
        2066 => stalactite::state::FALLING,
        2073 => stalactite::state::DEAD,
        2074 => stalactite::state::RESTART,
        _ => return None,
    })
}

/// A stalactite at one step of a run or of the replay: position, state
/// (the run's own numbers lined up through [`reference_state`]), out of
/// every collision, hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjAt {
    x: i32,
    y: i32,
    state: Option<i32>,
    out: bool,
    hidden: bool,
}

/// The steps of the three stalactites in a run's `obj-log.csv`, by
/// stalactite: (step, the stalactite there).
fn reference_objects(path: &Path) -> Option<Vec<Vec<(usize, ObjAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, fl, df) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("flags")?,
        col("draw_flags")?,
    );
    let mut out = vec![Vec::new(); LEDGE_STALACTITES.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = LEDGE_STALACTITES
            .iter()
            .position(|w| f.get(serial) == Some(&w.0))
        else {
            continue;
        };
        out[i].push((
            f[tick].parse().ok()?,
            ObjAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(int(f[st])?),
                out: hex(f[fl])? & flags::NO_COLLIDE != 0,
                hidden: hex(f[df])? & draw_flags::HIDDEN != 0,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's stalactite leaves
/// the run's, by column (`<serial>_x`, `_y`, `_state`, `_out`,
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
            for name in ["x", "y", "state", "out", "hidden"] {
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
        if got.out != want.out {
            note("out", k);
        }
        if got.hidden != want.hidden {
            note("hidden", k);
        }
    }
    firsts
}

/// The stalactite placed at `at`.
fn placed(sim: &Sim, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == stalactite::NAMES[0])
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no stalactite placed at {at:?}"))
}

fn obj_at(sim: &Sim, id: ObjectId) -> Option<ObjAt> {
    sim.objects().get(id).map(|o| ObjAt {
        x: o.x,
        y: o.y,
        state: Some(o.state),
        out: o.flags & flags::NO_COLLIDE != 0,
        hidden: o.draw_flags & draw_flags::HIDDEN != 0,
    })
}

/// Plays `rows` rows of route `r` from `sim` (at its time zero), with its
/// help screens, and returns the trace and the stalactites on each row.
fn play_rows(mut sim: Sim, r: &Route, rows: usize) -> (Vec<TraceRow>, Vec<Vec<Option<ObjAt>>>) {
    let ids: Vec<ObjectId> = LEDGE_STALACTITES
        .iter()
        .map(|&(_, at)| placed(&sim, at))
        .collect();
    let ticks = route_ticks(r);
    let mut trace = Vec::with_capacity(rows);
    let mut objects = vec![Vec::with_capacity(rows); ids.len()];
    for k in 0..rows {
        for &(_, ms, keys) in r.helps.iter().filter(|(at, ..)| *at == k) {
            // The return from the help screen: its update pass, and the
            // idle wait starts again.
            sim.pass_help_return(ms, keys);
            sim.restart_idle_wait();
        }
        sim.step(ticks.get(k).copied().unwrap_or_default());
        trace.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            objects[i].push(obj_at(&sim, id));
        }
    }
    (trace, objects)
}

/// One play of a route: its trace, the reference trace, and the
/// stalactites on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    objects: Vec<Vec<Option<ObjAt>>>,
}

/// Plays route `r` from its store's time zero for as long as the store's
/// first trace runs. A store without the trace or its meta fails the
/// test.
fn play(level: &SimLevel, root: &Path, r: &Route) -> Played {
    let dir = root.join("level12").join(r.store);
    let path = dir.join(format!("trace-{}-r1.csv", r.clock));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, r.clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if r.clock == "c18" {
        assert_eq!(t0, T0_MS, "{}: time zero", r.store);
    }
    assert_eq!(seed, SEED, "{}: random state", r.store);
    let sim = at_time_zero(level, r.clock, t0, seed);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let rows = end_ms.div_ceil(TICK) as usize;
    let (rows, objects) = play_rows(sim, r, rows);
    Played {
        rows,
        reference,
        objects,
    }
}

/// Compares a play of route `r` with the reference trace at the clock's
/// tolerances (exact for c18, feel for c60), and at c18 its stalactites
/// with the run's (when the reference runs are there); returns the
/// divergences not known.
fn new_divergences(root: &Path, r: &Route, p: &Played) -> Vec<String> {
    let store = r.store;
    let tol = if r.clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(&p.rows, &p.reference, &tol);
    print!("{}", trace::report(&p.rows, &p.reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != p.reference.len() {
        out.push(format!(
            "{store}: {} of {} rows compared",
            cmp.rows_compared,
            p.reference.len()
        ));
    }
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.to_string(), t)))
        .collect();
    if r.clock == "c18"
        && let Some(runs) = runs_root(root)
    {
        let meta = std::fs::read_to_string(root.join("level12").join(store).join("meta.toml"))
            .expect("meta");
        let id = run_id(&meta, r.clock).unwrap_or_else(|| panic!("{store}: no run id"));
        let log = runs.join(&id).join("obj-log.csv");
        let reference = reference_objects(&log)
            .unwrap_or_else(|| panic!("{store}: {} lacks a stalactite", log.display()));
        for (i, (serial, _)) in LEDGE_STALACTITES.iter().enumerate() {
            firsts.extend(compare_object(serial, &p.objects[i], &reference[i]));
        }
    }
    let list = known(store);
    for (column, first) in firsts {
        println!("first {store} {column} {first}");
        match list.iter().find(|(name, _)| *name == column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{store}: {column} at {first} ms")),
        }
    }
    out
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level12").join(r.store);
        let (rows, helps) =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        assert_eq!(helps, r.helps, "{}: help screens", r.store);
        // The store has the route's clock, its meta, its start and its
        // trace, and is plain play.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        assert_eq!(meta_value(&meta, "", "diagnostic"), None, "{}", r.store);
        assert_eq!(
            meta_value(&meta, "", "clocks").as_deref(),
            Some(format!("[\"{}\"]", r.clock).as_str()),
            "{}: clocks",
            r.store
        );
        assert!(
            meta_value(&meta, r.clock, "t0_ms").is_some(),
            "{}: no time zero",
            r.store
        );
        let teleport = meta_value(&meta, r.clock, "teleport")
            .unwrap_or_else(|| panic!("{}: no start", r.store));
        assert_eq!(
            teleport,
            format!("{},{}", START.0, START.1),
            "{}: start",
            r.store
        );
        let trace = dir.join(format!("trace-{}-r1.csv", r.clock));
        assert!(trace.is_file(), "{}", trace.display());
    }
}

#[test]
fn install_water_skip_routes_follow_the_reference_traces() {
    let Some(level) = level12() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_SKIP_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        println!("== level12/{} {}", r.store, r.clock);
        let p = play(&level, &root, r);
        if let Some(out) = std::env::var_os("CLAW_SKIP_TRACE") {
            let out = PathBuf::from(out).join(format!("{}-{}.csv", r.store, r.clock));
            std::fs::write(&out, trace::to_csv(&p.rows)).expect("the trace is written");
        }
        failures.extend(new_divergences(&root, r, &p));
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows of Claw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
}

fn look(sim: &Sim) -> Look {
    let c = sim.player().expect("Claw");
    Look {
        x: c.x,
        y: c.y,
        state: c.state,
        health: sim.stats().health,
    }
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there.
fn watch(level: &SimLevel, store: &str, rows: usize) -> Vec<Look> {
    let r = route_named(store);
    assert_eq!(r.clock, "c18", "{store}");
    let (trace, _) = play_rows(at_time_zero(level, r.clock, T0_MS, SEED), r, rows);
    trace
        .iter()
        .map(|t| Look {
            x: t.x,
            y: t.y,
            state: t.state_id,
            health: t.health,
        })
        .collect()
}

#[test]
fn install_water_skip_the_ledge_stalactites_are_down_before_time_zero() {
    let Some(level) = level12() else { return };
    let sim = at_time_zero(&level, "c18", T0_MS, SEED);
    // Claw has settled on the ledge, and the three stalactites he was put
    // down under have fallen and are crumbling.
    let c = sim.player().expect("Claw");
    assert_eq!((c.x, c.y), (2900, 3607));
    for &(_, at) in &LEDGE_STALACTITES {
        let s = sim.objects().get(placed(&sim, at)).unwrap();
        assert_eq!(s.state, stalactite::state::DEAD, "{at:?}");
        assert_eq!(s.y, 3624, "{at:?}");
    }
}

#[test]
fn install_water_skip_two_blasts_carry_claw_over_the_pool_onto_the_hidden_floor() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "skip-water-skip-c18", 900);
    // Each blast hits him in the air and holds him 22 rows, the stand on
    // the 22nd, the jump on the next.
    let hits: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect();
    assert_eq!(hits, [77, 128]);
    assert_eq!((rows[77].x, rows[77].y), (2786, 3696));
    assert_eq!((rows[128].x, rows[128].y), (2672, 3639));
    for &h in &hits {
        assert!((h..h + 22).all(|k| rows[k].state == state::HIT), "{h}");
        assert_eq!(rows[h + 22].state, state::STAND, "{h}");
        assert_eq!(rows[h + 23].state, state::JUMP, "{h}");
    }
    // Released at x 2558 he drops straight past the pool's rows and lands
    // on the hidden floor with 70 health, then walks the whole passage
    // under the pool unharmed.
    assert_eq!((rows[179].x, rows[179].y), (2558, 3708));
    assert!((180..185).all(|k| rows[k].x == 2558));
    assert_eq!(
        (rows[185].x, rows[185].y, rows[185].state),
        (2558, 3799, state::STAND)
    );
    assert_eq!(rows[185].health, 70);
    assert!(rows[185..].iter().all(|l| l.y == 3799 && l.health == 70));
    assert_eq!(rows[300].x, 2913);
    assert!(rows[899].x > 6000, "{}", rows[899].x);
}

#[test]
fn install_water_skip_the_entry_window_is_x_2546_to_2559() {
    let Some(level) = level12() else { return };
    // Released at 2546 he lands at the floor's end and teeters there.
    let rows = watch(&level, "skip-window-2546-c18", 200);
    assert_eq!((rows[185].x, rows[185].y), (2546, 3799));
    assert_eq!(rows[186].state, state::ON_EDGE);
    assert_eq!(rows[199].health, 70);
    // At 2542 every landing sample misses the floor: he falls through.
    let rows = watch(&level, "skip-window-2542-c18", 200);
    assert_eq!(rows[184].x, 2542);
    assert_eq!(rows[190].y, 3900);
    assert!(rows.iter().all(|l| l.state != state::DEATH));
    // At 2562 his centre column meets the pool: he dies in it.
    let rows = watch(&level, "skip-window-2562-c18", 200);
    assert_eq!(rows[179].x, 2562);
    assert_eq!((rows[180].state, rows[180].y), (state::DEATH, 3723));
}

/// The movement tuning a clock's routes are played with, for messages.
fn tuning_name(clock: &str) -> &'static str {
    if clock == "c18" {
        "the original's speeds"
    } else {
        "the 60 fps tuning"
    }
}

#[test]
fn install_water_skip_a_straight_drop_lands_on_the_hidden_floor_only_from_x_2546_to_2559() {
    let Some(level) = level12() else { return };
    for clock in ["c18", "c60"] {
        for x in 2536..2570 {
            // Put down in the air over the pool's left end, Claw falls
            // straight down.
            let mut sim = at_time_zero(&level, clock, T0_MS, SEED);
            sim.teleport_player(x, 3650);
            let mut end = look(&sim);
            for _ in 0..60 {
                sim.step(TickInput::default());
                end = look(&sim);
                if end.state == state::DEATH || end.y > 3900 {
                    break;
                }
            }
            let at = format!("{} at x {x}", tuning_name(clock));
            assert_eq!(end.x, x, "{at}");
            match x {
                // Neither landing sample (X + 4, X + 14) is over the floor.
                ..2546 => assert!(end.y > 3900 && end.state == state::FALL, "{at}: {end:?}"),
                // The centre column is left of the pool's first column.
                2546..2560 => {
                    assert_eq!(end.y, 3799, "{at}");
                    assert!(
                        matches!(end.state, state::STAND | state::ON_EDGE),
                        "{at}: {end:?}"
                    );
                }
                // The centre column meets the pool's rows on the way down.
                _ => assert_eq!(end.state, state::DEATH, "{at}: {end:?}"),
            }
        }
    }
}

/// How a skip played by [`skip_by_states`] ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// Standing on the hidden floor.
    Floor,
    /// Dead in the pool.
    Pool,
    /// Fallen past the floor.
    Through,
}

/// Plays the skip at `clock` from the routes' time zero the way its route
/// does, but with the two jumps from mid air pressed on the tick after
/// each stand and the second stick thrown on the third tick of the fall
/// that follows the first of them, its help screen two ticks later (found
/// from Claw's state, not by row, so that the plan holds at either
/// tuning); Left is let go from the first tick after one that ends with
/// Claw at `release` or further left. Returns the x he was let go at, the
/// rows on which he was hurt and how the skip ends.
fn skip_by_states(level: &SimLevel, clock: &str, release: i32) -> (i32, Vec<usize>, Ending) {
    let mut sim = at_time_zero(level, clock, T0_MS, SEED);
    let mut hits = Vec::new();
    let mut jumps: Vec<usize> = Vec::new();
    let mut helps: Vec<(usize, u32, usize)> = vec![(69, 3200, 2)];
    let mut throws: Vec<usize> = vec![30];
    let mut stood: Option<usize> = None;
    let mut released: Option<i32> = None;
    let mut last = look(&sim);
    for k in 0..400usize {
        if let Some(&(_, ms, keys)) = helps.iter().find(|(at, ..)| *at == k) {
            sim.pass_help_return(ms, keys);
            sim.restart_idle_wait();
        }
        if stood.take().is_some() {
            jumps.push(k);
        }
        if jumps.len() == 2 && released.is_none() && last.x <= release {
            released = Some(last.x);
        }
        let mut held = 0u32;
        let mut pressed = 0u32;
        let mut down = |mask: u32, from: usize, to: usize| {
            if (from..=to).contains(&k) {
                held |= mask;
                if k == from {
                    pressed |= mask;
                }
            }
        };
        down(LEFT, 20, 21);
        if released.is_none() {
            down(LEFT, 52, usize::MAX);
        }
        down(JUMP, 54, 71);
        for &t in &throws {
            down(DYNAMITE, t, t + 1);
        }
        for &j in &jumps {
            down(JUMP, j, j + 17);
        }
        sim.step(TickInput {
            held: Buttons(held),
            pressed: Buttons(pressed),
        });
        let now = look(&sim);
        if now.health < last.health {
            hits.push(k);
        }
        if now.state == state::STAND && last.state == state::HIT {
            stood = Some(k);
        }
        if jumps.len() == 1
            && throws.len() == 1
            && now.state == state::FALL
            && last.state == state::JUMP
        {
            throws.push(k + 3);
            helps.push((k + 5, 4350, 2));
        }
        last = now;
        let ending = if now.state == state::DEATH {
            Ending::Pool
        } else if now.y > 3900 {
            Ending::Through
        } else if now.y == 3799 && matches!(now.state, state::STAND | state::ON_EDGE) {
            Ending::Floor
        } else {
            continue;
        };
        return (released.unwrap_or(now.x), hits, ending);
    }
    panic!("the skip never ended ({} at {release})", tuning_name(clock));
}

#[test]
fn install_water_skip_works_at_both_tunings_and_the_release_decides_the_ending() {
    let Some(level) = level12() else { return };
    // At the original's speeds the plan plays the store's own rows.
    let (x, hits, ending) = skip_by_states(&level, "c18", 2559);
    assert_eq!((x, ending), (2558, Ending::Floor));
    assert_eq!(hits, [77, 128]);
    // At either tuning, Left let go anywhere over the pool's left end: the
    // two blasts hit him, and where he is let go decides.
    for clock in ["c18", "c60"] {
        let mut seen = Vec::new();
        for release in 2536..2572 {
            let (x, hits, ending) = skip_by_states(&level, clock, release);
            let at = format!("{} let go at x {x}", tuning_name(clock));
            assert_eq!(hits.len(), 2 + usize::from(ending == Ending::Pool), "{at}");
            let want = match x {
                ..2546 => Ending::Through,
                2546..2560 => Ending::Floor,
                _ => Ending::Pool,
            };
            assert_eq!(ending, want, "{at}");
            seen.push(ending);
        }
        for e in [Ending::Through, Ending::Floor, Ending::Pool] {
            assert!(seen.contains(&e), "{}: {e:?}", tuning_name(clock));
        }
    }
}

#[test]
fn install_water_skip_a_jump_from_the_hidden_floor_back_into_the_pool_kills_claw() {
    let Some(level) = level12() else { return };
    // On the hidden floor at the passage's far end, under the boss room's
    // pool, where nothing is above him (put down at y 3779, he settles at
    // 3799): a straight jump on row 20, held. He rises through the pool's
    // rows unharmed to y 3654 (row 39) and dies in them on the way down,
    // at y 3717 (row 52), as in the original's run of the same.
    let mut sim = put_down_at(&level, "c18", (37781, 3779), T0_MS, SEED);
    assert_eq!((look(&sim).x, look(&sim).y), (37781, 3799));
    let mut rows = Vec::new();
    for k in 0..60 {
        let held = if (20..=55).contains(&k) { JUMP } else { 0 };
        sim.step(TickInput {
            held: Buttons(held),
            pressed: Buttons(if k == 20 { held } else { 0 }),
        });
        rows.push(look(&sim));
    }
    let ys: Vec<i32> = rows[20..=39].iter().map(|l| l.y).collect();
    assert_eq!(
        ys,
        [
            3791, 3784, 3777, 3771, 3764, 3757, 3749, 3742, 3734, 3726, 3717, 3708, 3699, 3690,
            3681, 3673, 3667, 3661, 3655, 3654
        ]
    );
    assert!(rows[..52].iter().all(|l| l.health == 100));
    assert_eq!(rows[39].state, state::FALL);
    assert_eq!((rows[51].y, rows[51].state), (3708, state::FALL));
    assert_eq!((rows[52].y, rows[52].state), (3717, state::DEATH));
}
