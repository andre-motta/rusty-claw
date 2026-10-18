//! The ninth level's conveyor belts, saw blades, skull cannons and dart
//! traps on your own install, against the original's runs of the same
//! short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does: the objects around him wake and settle then), waits until
//! the level clock reaches the time the original's run started at, and
//! starts the shared random stream from the run's state, once. It is then
//! played tick by tick and compared with the reference trace of the same
//! route: at the 18 ms step with the original's speeds every column
//! exactly (Claw's position and state, health, score, the camera, the
//! random stream and the events), at 60 frames per second with the default
//! tuning within the feel tolerances. The routes:
//!
//! - the 150 px/s belt on the floor at 7504 (x 4784 to 5214): Claw put
//!   down left of it walks onto it and stands (the push from the second
//!   tick, the drift and its one-tick stalls, the carry off its right
//!   end), walks right along it, or is put down right of it and walks left
//!   across it;
//! - the -150 px/s belt on the floor at 7248 (x 9776 to 10462): Claw
//!   stands and is carried to its left end over a drop, where he teeters,
//!   or jumps in place, against it and with it;
//! - the -500 px/s belt at y 7385: Claw is carried into the warp at x
//!   24224 and stands at its target without drifting;
//! - the saws at x 3358 and 3555: Claw stands between them, on the first,
//!   or jumps down to them from the pillar where they sleep;
//! - the crossfire shaft of skull cannons: Claw stands, or ducks, on its
//!   middle ledge;
//! - the dart trap at (1558, 7212): Claw walks into its rectangle and
//!   stands.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace; `CLAW_L9_ONLY` names one store
//! to play alone. Nothing from an install or a reference store is kept in
//! the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::{ObjectData, conveyor_belt, dart_trap, saw_blade};
use claw_sim::object::{ObjectId, flags};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

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

/// A route of the stores: its name, where Claw is put down, the original's
/// game time at the route's time zero at the 18 ms step (ms), how many rows
/// it has and its stretches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
}

const fn route(
    store: &'static str,
    start: (i32, i32),
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
) -> Route {
    Route {
        store,
        start,
        t0_ms,
        rows,
        spans,
    }
}

/// Where the routes of the 150 belt put Claw down, left and right of it.
const BELT_LEFT: (i32, i32) = (4720, 7400);
const BELT_RIGHT: (i32, i32) = (5262, 7400);
/// Where the routes of the -150 belt put Claw down, on it.
const BELT_BACK: (i32, i32) = (10300, 7150);
/// The middle ledge of the crossfire shaft.
const SHAFT: (i32, i32) = (7885, 7050);

const ROUTES: [Route; 12] = [
    route("belt-stand", BELT_LEFT, 828, 300, &[span(20, 26, RIGHT)]),
    route("belt-with", BELT_LEFT, 828, 200, &[span(20, 109, RIGHT)]),
    route("belt-against", BELT_RIGHT, 828, 240, &[span(20, 221, LEFT)]),
    route("belt-teeter", BELT_BACK, 828, 300, &[]),
    route(
        "belt-jump",
        BELT_BACK,
        828,
        330,
        &[
            span(40, 59, JUMP),
            span(120, 139, JUMP),
            span(120, 159, RIGHT),
            span(220, 239, JUMP),
            span(220, 259, LEFT),
        ],
    ),
    route("belt-warp", (24470, 7300), 828, 150, &[]),
    route("saw-stand", (3456, 6650), 828, 1000, &[]),
    route("saw-hit", (3358, 6650), 1170, 600, &[]),
    route(
        "saw-wake",
        (3200, 6200),
        828,
        800,
        &[span(20, 49, RIGHT), span(22, 31, JUMP)],
    ),
    route("skull-shaft", SHAFT, 1224, 600, &[]),
    route("skull-duck", SHAFT, 1224, 600, &[span(15, 599, DOWN)]),
    route(
        "dart-right",
        (1570, 7200),
        828,
        1000,
        &[span(20, 39, RIGHT)],
    ),
];

fn route_named(store: &str) -> &'static Route {
    ROUTES
        .iter()
        .find(|r| r.store == store)
        .unwrap_or_else(|| panic!("no route {store}"))
}

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, every store (rng): the hook-handed pirates and the PegLegs
///   awake around these routes draw from the shared random stream on
///   every tick in the original, from time zero (or, in the crossfire
///   shaft, from when one wakes below: 6336 ms standing, 1854 ms with the
///   camera lowered by the crouch); their own item builds them, and
///   without them the replay's stream stays where time zero put it. No
///   object of this item draws, and every other column is exact.
/// - c60 (the default tuning played tick by tick), the belts: a belt's
///   push moves Claw by the whole pixels of each step's share, so the
///   step length sets the drift: 2 or 3 px a frame at 60 frames per
///   second (about 157 px/s on the 150 belt), 3 px every 18 ms (166.7
///   px/s) in the replay; and the original at 60 frames per second put
///   Claw down 5 px further along the -150 belt before time zero. The
///   standing drift is 12 px ahead by 3083 ms (belt-stand), the drift to
///   the edge reaches it 131 ms early (belt-teeter, 2767 ms) and picks up
///   the treasure on the belt earlier (1500 ms), and the jumps rise and
///   fall from other places (belt-jump). Walking with the belt Claw is 5
///   px further at the end of the walk and goes over the end, where the
///   original stopped on its edge (belt-with, 1983 ms), and falls to the
///   floor below; walking against it he gains on the original and walks
///   off its left end at 3474 ms (belt-against). Carried into the warp,
///   the original at 60 frames per second, put down 10 px nearer it before
///   time zero, is at the target on its third frame (50 ms), the replay
///   on its fifth tick (90 ms) (belt-warp).
/// - c60, the saws: a saw's rise counts animation calls, 31 frames (517
///   ms) at 60 frames per second and 31 ticks (558 ms) in the replay, so
///   its blow lands 45 ms earlier and Claw's hurt pose ends earlier
///   (saw-hit 700 ms, saw-wake 3083 ms); the jump down to the saws peaks
///   9 px higher at 60 frames per second (saw-wake 667 ms).
/// - c60, the skull cannons: knocked off the ledge, Claw falls faster at
///   60 frames per second (27 px lower by 6300 ms) and catches the rope
///   in the shaft at 6450 ms, which the replay's fall passes; crouching,
///   the camera pans down 4 px a step, 60 steps a second in the original
///   and 55.6 in the replay, and is 16 px behind by 2016 ms.
/// - c60, the dart trap: its puff counts animation calls, 31 frames (517
///   ms) at 60 frames per second and 31 ticks (558 ms) in the replay, and
///   its wait counts milliseconds, so it fires every 91 frames (1517 ms)
///   against every 87 ticks (1566 ms); the walk in with the default
///   tuning also ends 4 px shorter. Each arrow reaches Claw later in the
///   replay, the first 69 ms later (1083 ms).
const KNOWN: &[Known] = &[
    ("belt-stand", "c18", &[("rng", 18)]),
    ("belt-with", "c18", &[("rng", 18)]),
    ("belt-against", "c18", &[("rng", 18)]),
    ("belt-teeter", "c18", &[("rng", 18)]),
    ("belt-jump", "c18", &[("rng", 18)]),
    ("belt-warp", "c18", &[("rng", 90)]),
    ("saw-stand", "c18", &[("rng", 18)]),
    ("saw-hit", "c18", &[("rng", 18)]),
    ("saw-wake", "c18", &[("rng", 18)]),
    ("skull-shaft", "c18", &[("rng", 6336)]),
    ("skull-duck", "c18", &[("rng", 1854)]),
    ("dart-right", "c18", &[("rng", 18)]),
    ("belt-stand", "c60", &[("x", 3083), ("cam_x", 3083)]),
    (
        "belt-with",
        "c60",
        &[
            ("x", 1983),
            ("state_id", 1983),
            ("physics_type", 1983),
            ("cam_x", 1983),
            ("y", 2067),
            ("cam_y", 2067),
            ("health", 3367),
        ],
    ),
    (
        "belt-against",
        "c60",
        &[
            ("x", 817),
            ("cam_x", 817),
            ("state_id", 3500),
            ("physics_type", 3500),
            ("y", 3567),
            ("cam_y", 3567),
        ],
    ),
    (
        "belt-teeter",
        "c60",
        &[
            ("x", 833),
            ("cam_x", 833),
            ("score", 1500),
            ("state_id", 2767),
        ],
    ),
    (
        "belt-jump",
        "c60",
        &[
            ("x", 867),
            ("cam_x", 867),
            ("score", 1767),
            ("y", 4883),
            ("cam_y", 4883),
        ],
    ),
    (
        "belt-warp",
        "c60",
        &[("x", 50), ("y", 50), ("cam_x", 50), ("cam_y", 50)],
    ),
    ("saw-hit", "c60", &[("state_id", 700)]),
    (
        "saw-wake",
        "c60",
        &[("y", 667), ("cam_y", 667), ("state_id", 3083)],
    ),
    (
        "skull-shaft",
        "c60",
        &[
            ("y", 6300),
            ("cam_y", 6300),
            ("x", 6450),
            ("state_id", 6450),
            ("physics_type", 6450),
            ("cam_x", 6466),
            ("score", 8700),
            ("health", 8866),
        ],
    ),
    ("skull-duck", "c60", &[("cam_y", 2016), ("y", 6300)]),
    (
        "dart-right",
        "c60",
        &[
            ("state_id", 1083),
            ("health", 1083),
            ("x", 1117),
            ("cam_x", 1117),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
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

/// The rows of a store's own copy of a route: (held, pressed) by row.
fn store_route(dir: &Path) -> Option<Vec<(u32, u32)>> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    Some(
        text.lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
            .map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (hex(f[1]), hex(f[2]))
            })
            .collect(),
    )
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

fn level9() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(9)).unwrap();
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

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, start: (i32, i32), t0_ms: u64, seed: u32) -> Sim {
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

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs; returns its rows and that trace. A store
/// without the trace or its meta fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> (Vec<TraceRow>, Vec<TraceRow>) {
    let dir = root.join("level9").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, r.start, t0, seed);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        k += 1;
    }
    (rows, reference)
}

/// Compares `rows` with `reference` at the clock's tolerances (exact for
/// c18, feel for c60) and returns the divergences not in [`KNOWN`].
fn new_divergences(
    store: &str,
    clock: &str,
    rows: &[TraceRow],
    reference: &[TraceRow],
) -> Vec<String> {
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(rows, reference, &tol);
    print!("{}", trace::report(rows, reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != reference.len() {
        out.push(format!(
            "{store} {clock}: {} of {} rows compared",
            cmp.rows_compared,
            reference.len()
        ));
    }
    let list = known(store, clock);
    for c in &cmp.per_column {
        let Some(first) = c.first_t_ms else { continue };
        println!("first {store} {clock} {} {first}", c.column);
        match list.iter().find(|(name, _)| *name == c.column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{store} {clock}: {} at {first} ms", c.column)),
        }
    }
    out
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level9").join(r.store);
        let rows =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
    }
}

#[test]
fn install_level9_object_routes_follow_the_reference_traces() {
    let Some(level) = level9() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L9_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for clock in ["c18", "c60"] {
            println!("== level9/{} {clock}", r.store);
            let (rows, reference) = play(&level, &root, r, clock);
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------- the objects of the runs

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

/// The logic names the replay gives this item's objects.
const LOGICS: [&str; 8] = [
    "ConveyorBelt",
    "SawBlade",
    "SawBlade2",
    "SawBlade3",
    "SkullCannon",
    "TProjectile",
    "CannonBall",
    "Bullet",
];

/// The replay's name for a run row's logic name. The runs name every
/// object as the level file does, except the skull cannon, whose spelling
/// differs; any name starting with "skullcan" (any case) is the cannon.
fn run_logic(name: &str) -> Option<&'static str> {
    if name.to_ascii_lowercase().starts_with("skullcan") {
        return Some("SkullCannon");
    }
    LOGICS.iter().copied().find(|n| *n == name)
}

/// What one object shows on one step: its position, state, picture and
/// whether it is shut off (a saw's harmless flag, a trap's shut-off
/// flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjAt {
    x: i32,
    y: i32,
    state: i32,
    frame: i32,
    off: bool,
}

/// The state a run's number stands for in the replay, for an object
/// running `logic` (the replay's name).
fn replay_state(logic: &str, n: i32) -> i32 {
    match (logic, n) {
        ("ConveyorBelt", 2069) => conveyor_belt::state::IDLE,
        ("ConveyorBelt", 2071) => conveyor_belt::state::MOVE,
        ("SkullCannon", 2018) => EnemyState::Home as i32,
        ("SkullCannon", 2027) => EnemyState::Fire as i32,
        ("TProjectile", 2000) => dart_trap::state::IDLE,
        ("TProjectile", 2027) => dart_trap::state::FIRING,
        ("TProjectile", 2052) => dart_trap::state::WAIT,
        // A flying shot: its state is not compared.
        ("CannonBall" | "Bullet", _) => 0,
        _ => n,
    }
}

/// One object's steps in a run, by step.
struct RunObject {
    logic: &'static str,
    first: (usize, i32, i32),
    steps: Vec<(usize, ObjAt)>,
}

/// One step of a run's `obj-log.csv`: the belt pieces in list order (by
/// position) and every object of this item's families, by serial.
struct RunLog {
    objects: Vec<RunObject>,
    belt_order: Vec<Vec<(i32, i32)>>,
}

fn read_run_log(path: &Path) -> Option<RunLog> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, pos, serial, logic, st, x, y, fl, frame) = (
        col("tick")?,
        col("list_pos")?,
        col("serial")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("flags")?,
        col("frame")?,
    );
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut objects: Vec<(String, RunObject)> = Vec::new();
    let mut belts: Vec<Vec<(i32, (i32, i32))>> = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let Some(ours) = run_logic(f[logic]) else {
            continue;
        };
        let k: usize = f[tick].parse().ok()?;
        let flags = hex(f[fl])?;
        let at = ObjAt {
            x: int(f[x])?,
            y: int(f[y])?,
            state: replay_state(ours, int(f[st])?),
            frame: int(f[frame])?,
            off: match ours {
                "TProjectile" => flags & 0x1 != 0,
                n if n.starts_with("SawBlade") => flags & 0x80 != 0,
                _ => false,
            },
        };
        if ours == "ConveyorBelt" {
            if belts.len() <= k {
                belts.resize(k + 1, Vec::new());
            }
            belts[k].push((int(f[pos])?, (at.x, at.y)));
        }
        match objects.iter_mut().find(|(s, _)| s == f[serial]) {
            Some((_, o)) => o.steps.push((k, at)),
            None => objects.push((
                f[serial].to_string(),
                RunObject {
                    logic: ours,
                    first: (k, at.x, at.y),
                    steps: vec![(k, at)],
                },
            )),
        }
    }
    let belt_order = belts
        .into_iter()
        .map(|mut v| {
            v.sort_unstable();
            v.into_iter().map(|(_, p)| p).collect()
        })
        .collect();
    Some(RunLog {
        objects: objects.into_iter().map(|(_, o)| o).collect(),
        belt_order,
    })
}

/// What the replay shows of object `o`.
fn obj_at(o: &claw_sim::object::Object) -> ObjAt {
    let logic: &str = &o.logic;
    ObjAt {
        x: o.x,
        y: o.y,
        state: if matches!(logic, "CannonBall" | "Bullet") {
            0
        } else {
            o.state
        },
        frame: o.frame,
        off: match logic {
            "TProjectile" => o.flags & flags::NO_COLLIDE != 0,
            n if n.starts_with("SawBlade") => o.flags & flags::HARMLESS != 0,
            _ => false,
        },
    }
}

/// Where each of the item's objects was first seen in a replay: its id,
/// its logic (the replay's name) and its first step and place.
type Firsts = Vec<(ObjectId, &'static str, (usize, i32, i32))>;

/// One step of the replay: the item's objects (id and what they show),
/// the belt pieces in list order and Claw's conveyor force.
struct ReplayStep {
    objects: Vec<(ObjectId, ObjAt)>,
    belt_order: Vec<(i32, i32)>,
    force: i32,
}

/// Plays route `r` at the 18 ms step from its time zero for `rows` rows,
/// recording each step and where each of the item's objects was first
/// seen (step, place).
///
/// The runs list an object on every step it took part in: an object the
/// step's leave pass put to sleep is still listed on that step. So an
/// object counts on a step when it is live at its end or was live at the
/// end of the step before.
fn replay_objects(level: &SimLevel, r: &Route, rows: usize) -> (Vec<ReplayStep>, Firsts) {
    let mut sim = at_time_zero(level, "c18", r.start, r.t0_ms, SEED);
    let ticks = route_ticks(r);
    let mut firsts: Firsts = Vec::new();
    let mut steps = Vec::new();
    let mut live_before: Vec<ObjectId> = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let listed = sim.objects().listed();
        let mut objects = Vec::new();
        let mut belt_order = Vec::new();
        let mut live = Vec::new();
        let ids = listed.iter().copied().chain(
            live_before
                .iter()
                .copied()
                .filter(|id| !listed.contains(id)),
        );
        for id in ids {
            let Some(o) = sim.objects().get(id) else {
                continue;
            };
            let Some(&ours) = LOGICS.iter().find(|n| **n == &*o.logic) else {
                continue;
            };
            if o.is_listed() {
                live.push(id);
                if ours == "ConveyorBelt" {
                    belt_order.push((o.x, o.y));
                }
            }
            if !firsts.iter().any(|(i, _, _)| *i == id) {
                firsts.push((id, ours, (k, o.x, o.y)));
            }
            objects.push((id, obj_at(o)));
        }
        live_before = live;
        steps.push(ReplayStep {
            objects,
            belt_order,
            force: sim.claw().conveyor_force as i32,
        });
    }
    (steps, firsts)
}

/// The replay's object for a run's: a placed object by its logic and
/// place, a shot by its logic, its first step and its place then.
fn match_object(
    run: &RunObject,
    firsts: &[(ObjectId, &'static str, (usize, i32, i32))],
) -> Option<ObjectId> {
    let shot = matches!(run.logic, "CannonBall" | "Bullet");
    firsts
        .iter()
        .find(|(_, logic, (k, x, y))| {
            *logic == run.logic
                && (*x, *y) == (run.first.1, run.first.2)
                && (!shot || *k == run.first.0)
        })
        .map(|(id, _, _)| *id)
}

/// A store and the first step at which each object column already
/// differs from its run, known; a column not listed must not differ.
type KnownObjects = (&'static str, &'static [(&'static str, usize)]);

const KNOWN_OBJECTS: &[KnownObjects] = &[];

fn known_objects(store: &str) -> &'static [(&'static str, usize)] {
    KNOWN_OBJECTS
        .iter()
        .find(|(s, _)| *s == store)
        .map_or(&[], |(_, k)| k)
}

/// The first step of each column in which the replay leaves the run:
/// `<logic>_x`, `_y`, `_state`, `_frame`, `_off` and `_missing` per
/// family, `belt_order` and `force`.
fn compare_objects(
    log: &RunLog,
    forces: Option<&[(usize, i32)]>,
    steps: &[ReplayStep],
    firsts: &[(ObjectId, &'static str, (usize, i32, i32))],
) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut note = |name: String, k: usize| {
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, k));
        }
    };
    for run in &log.objects {
        let id = match_object(run, firsts);
        let last = run.steps.last().map_or(0, |s| s.0);
        for &(k, want) in &run.steps {
            let Some(step) = steps.get(k) else { continue };
            let got = id.and_then(|id| step.objects.iter().find(|(i, _)| *i == id));
            // A shot removed by the leave pass of the step it is last listed
            // on in the run: gone from the replay at the end of that step.
            let removed_now = k == last
                && k > 0
                && id.is_some_and(|id| steps[k - 1].objects.iter().any(|(i, _)| *i == id));
            if got.is_none() && removed_now {
                continue;
            }
            let Some(&(_, got)) = got else {
                note(format!("{}_missing", run.logic), k);
                continue;
            };
            for (name, same) in [
                ("x", got.x == want.x),
                ("y", got.y == want.y),
                ("state", got.state == want.state),
                ("frame", got.frame == want.frame),
                ("off", got.off == want.off),
            ] {
                if !same {
                    note(format!("{}_{name}", run.logic), k);
                }
            }
        }
    }
    // The order of the pieces live on both sides (a piece the step put to
    // sleep has left the replay's list).
    for (k, want) in log.belt_order.iter().enumerate() {
        let Some(step) = steps.get(k) else { continue };
        let got: Vec<(i32, i32)> = step
            .belt_order
            .iter()
            .copied()
            .filter(|p| want.contains(p))
            .collect();
        let want: Vec<(i32, i32)> = want.iter().copied().filter(|p| got.contains(p)).collect();
        if got != want {
            note("belt_order".to_string(), k);
        }
    }
    for &(k, want) in forces.unwrap_or_default() {
        if steps.get(k).is_some_and(|s| s.force != want) {
            note("force".to_string(), k);
        }
    }
    out.sort_by_key(|(_, k)| *k);
    out
}

/// Claw's conveyor force per step of a run's `pd-log.csv`. The log holds
/// one probed column besides Claw's usual ones (its name starts with
/// "pd_"): Claw's conveyor force column.
fn read_forces(path: &Path) -> Option<Vec<(usize, i32)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let mut probed = (0..head.len()).filter(|&i| head[i].starts_with("pd_"));
    let (tick, force) = (head.iter().position(|h| *h == "tick")?, probed.next()?);
    if probed.next().is_some() {
        return None;
    }
    lines
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Some((f[tick].parse().ok()?, f[force].trim().parse().ok()?))
        })
        .collect()
}

#[test]
fn install_level9_objects_follow_the_reference_runs() {
    let Some(level) = level9() else { return };
    let Some(root) = reference_root() else { return };
    let Some(runs) = runs_root(&root) else {
        eprintln!("skipped: no reference runs beside the store (CLAW_REFERENCE_RUNS)");
        return;
    };
    let only = std::env::var("CLAW_L9_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level9").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        let id = run_id(&meta, "c18").unwrap_or_else(|| panic!("{}: no run id", r.store));
        let path = runs.join(&id).join("obj-log.csv");
        let Some(log) = read_run_log(&path) else {
            eprintln!("{}: no object log at {}", r.store, path.display());
            continue;
        };
        let forces = read_forces(&runs.join(&id).join("pd-log.csv"));
        let rows = log.belt_order.len().max(
            log.objects
                .iter()
                .filter_map(|o| o.steps.last().map(|s| s.0 + 1))
                .max()
                .unwrap_or(0),
        );
        let (steps, firsts) = replay_objects(&level, r, rows);
        let diffs = compare_objects(&log, forces.as_deref(), &steps, &firsts);
        let list = known_objects(r.store);
        for (name, k) in diffs {
            println!("first {} objects {name} {k}", r.store);
            match list.iter().find(|(n, _)| *n == name) {
                Some(&(_, at)) if k >= at => {}
                _ => failures.push(format!("{}: {name} at step {k}", r.store)),
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one step of a route shows: Claw, his conveyor force, the sounds
/// started, and the watched objects.
#[derive(Debug, Clone)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    physics: i32,
    health: i32,
    force: i32,
    sounds: Vec<String>,
    objects: Vec<Option<ObjAt>>,
}

/// The object running `logic` placed nearest (x, y).
fn find(sim: &Sim, logic: &str, (x, y): (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .min_by_key(|(_, o)| (o.x - x).abs() + (o.y - y).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} near ({x}, {y})"))
}

/// Plays `rows` rows of route `store` at the 18 ms step from its time
/// zero, watching the objects running the logics given nearest the places
/// given.
fn watch(level: &SimLevel, store: &str, rows: usize, watched: &[(&str, (i32, i32))]) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r.start, r.t0_ms, SEED);
    let ids: Vec<ObjectId> = watched.iter().map(|&(l, p)| find(&sim, l, p)).collect();
    let ticks = route_ticks(r);
    (0..rows)
        .map(|k| {
            let snap = sim.step(ticks.get(k).copied().unwrap_or_default());
            let c = sim.player().expect("Claw");
            let sounds = snap
                .audio
                .iter()
                .filter_map(|a| match a {
                    claw_frame::AudioCue::Play(s) => Some(s.key.clone()),
                    _ => None,
                })
                .collect();
            Look {
                x: c.x,
                y: c.y,
                state: c.state,
                physics: c.physics_type,
                health: sim.claw().stats.health,
                force: sim.claw().conveyor_force as i32,
                sounds,
                objects: ids
                    .iter()
                    .map(|&id| sim.objects().get(id).map(obj_at))
                    .collect(),
            }
        })
        .collect()
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// The rows on which the push drops to 0 for a single row while Claw
/// stays on the belt (the force is the same on both sides).
fn stalls(rows: &[Look]) -> Vec<usize> {
    (1..rows.len() - 1)
        .filter(|&k| {
            rows[k].force == 0 && rows[k - 1].force != 0 && rows[k + 1].force == rows[k - 1].force
        })
        .collect()
}

/// The rows on which watched object `i` enters state `s`.
fn entries(rows: &[Look], i: usize, s: i32) -> Vec<usize> {
    starts(rows, |l| l.objects[i].is_some_and(|o| o.state == s))
}

/// Each row of `rise` with the first row of `sink` after it.
fn halves(rise: &[usize], sink: &[usize]) -> Vec<(usize, usize)> {
    rise.iter()
        .filter_map(|&e| sink.iter().find(|&&f| f > e).map(|&f| (e, f)))
        .collect()
}

/// The rows on which `key` starts.
fn sound_rows(rows: &[Look], key: &str) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| rows[k].sounds.iter().any(|s| s == key))
        .collect()
}

#[test]
fn install_level9_a_belt_pushes_from_the_second_tick_and_stalls_on_the_list_order() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "belt-stand", 300, &[]);
    // Claw's update first ends inside the left end's box on row 26; the
    // piece notices him on 27 and pushes from 28.
    assert_eq!(rows[26].x, 4748);
    assert_eq!(starts(&rows, |l| l.force != 0)[0], 28);
    assert_eq!(rows[28].force, 150);
    // Standing, he drifts 3 px a row.
    assert!((29..52).all(|k| rows[k].x - rows[k - 1].x == 3));
    // One-row stalls where the piece he leaves comes later in the list:
    // x 4823, 4943 and 5072.
    let st = stalls(&rows);
    assert_eq!(st, [53, 94, 138]);
    assert_eq!(
        st.iter().map(|&k| rows[k].x).collect::<Vec<_>>(),
        [4823, 4943, 5072]
    );
    // Carried off the right end onto the ground beyond: no push from 192.
    assert_eq!((rows[191].x, rows[191].force), (5231, 150));
    assert_eq!(rows[192].force, 0);
    assert!(rows[192..].iter().all(|l| l.force == 0));
    // Walking with it, the same stalls come sooner.
    let rows = watch(&level, "belt-with", 200, &[]);
    assert_eq!(stalls(&rows), [38, 54, 72]);
}

#[test]
fn install_level9_a_belt_stalls_where_the_pieces_woke_in_another_order() {
    let Some(level) = level9() else { return };
    // Put down right of the belt, the pieces wake in another order: walking
    // left across it he stalls at x 5163, 5099 and 4970, and drifting back
    // right at x 4822, 4879, 4945 and 5071.
    let rows = watch(&level, "belt-against", 352, &[]);
    let st = stalls(&rows);
    assert_eq!(st, [62, 98, 144, 246, 266, 289, 332]);
    assert_eq!(
        st.iter().map(|&k| rows[k].x).collect::<Vec<_>>(),
        [5163, 5099, 4970, 4822, 4879, 4945, 5071]
    );
}

#[test]
fn install_level9_a_belt_carries_claw_to_the_edge_where_he_teeters() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "belt-teeter", 412, &[]);
    // Every crossing of the drift left leaves a later-listed piece.
    assert_eq!(stalls(&rows), [34, 57, 79, 101]);
    // At the left end, over the drop, he teeters from row 151 for good,
    // the push kept and no drift.
    let edge = starts(&rows, |l| l.state == state::ON_EDGE);
    assert_eq!(edge, [151]);
    assert!(
        rows[151..]
            .iter()
            .all(|l| l.state == state::ON_EDGE && l.x == 9757 && l.force == -150)
    );
}

#[test]
fn install_level9_a_jump_keeps_the_push_while_it_rises() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "belt-jump", 442, &[]);
    // The jump in place on row 40 rises with the push (3 or 4 px a row
    // left), the fall from row 59 drops it on the row after the apex.
    let rise = 40..59;
    assert!(
        rise.clone()
            .all(|k| rows[k].physics == 3 && rows[k].force == -150)
    );
    assert!(
        rise.clone()
            .all(|k| (3..=4).contains(&(rows[k - 1].x - rows[k].x)))
    );
    assert_eq!(rows[59].physics, 4);
    assert_eq!(rows[60].force, 0);
    assert!((61..79).all(|k| rows[k].x == rows[60].x));
    // Landed on row 79, pushed again from row 80.
    assert_eq!(starts(&rows, |l| l.physics == 1)[1], 79);
    assert_eq!((rows[79].force, rows[80].force), (0, -150));
    // Against it (Right held) the rise gains 1 or 2 px a row, with it
    // (Left held) 7 or 8.
    assert!((121..139).all(|k| (1..=2).contains(&(rows[k].x - rows[k - 1].x))));
    assert!((221..239).all(|k| (7..=8).contains(&(rows[k - 1].x - rows[k].x))));
}

#[test]
fn install_level9_a_warp_from_a_belt_leaves_no_push() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "belt-warp", 262, &[]);
    // The -500 belt pushes at the 300 px/s limit, 5 px a row, into the
    // warp; at the target on row 4 the push is gone and he never drifts.
    assert!((0..4).all(|k| rows[k].force == -300));
    assert_eq!(rows[1].x - rows[2].x, 5);
    assert_eq!((rows[4].x, rows[4].y, rows[4].force), (2175, 5885, 0));
    assert!(rows[4..].iter().all(|l| l.x == 2175 && l.force == 0));
    assert_eq!(rows[261].y, 5997);
}

/// The saws of the saw routes: `SawBlade` and `SawBlade2`.
const SAWS: [(&str, (i32, i32)); 2] = [("SawBlade", (3358, 6716)), ("SawBlade2", (3555, 6716))];

#[test]
fn install_level9_saws_hurt_from_the_end_of_their_rise_to_the_end_of_the_up_half() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "saw-stand", 1112, &SAWS);
    let rise = entries(&rows, 0, saw_blade::state::RISE_START);
    assert_eq!(rise, [120, 287, 453, 620, 787, 953]);
    // The second saw 750 ms later.
    assert_eq!(entries(&rows, 1, saw_blade::state::RISE_START)[0], 162);
    let sink = entries(&rows, 0, saw_blade::state::DROP_START);
    let up = halves(&rise, &sink);
    assert_eq!(up.len(), 6);
    for (e, f) in up {
        // Rising from E + 1, dangerous from E + 32 to F (83 or 84 rows
        // later), harmless from F + 1, down again on F + 29.
        let s = |k: usize| rows[k].objects[0].expect("the saw");
        assert_eq!(s(e + 1).state, saw_blade::state::RISING);
        assert!(s(e + 31).off && !s(e + 32).off, "rise {e}");
        assert!((83..=84).contains(&(f - e)), "rise {e}: sink {f}");
        assert!(!s(f).off && s(f + 1).off, "sink {f}");
        assert_eq!(s(f + 28).state, saw_blade::state::DROPPING);
        assert_eq!(s(f + 29).state, saw_blade::state::DOWN);
        // Its pictures: 1 to 10 from E + 2, the spin's 10 and 11, then 12
        // to 20 from F + 2.
        let frames: Vec<i32> = (e + 2..e + 32).step_by(3).map(|k| s(k).frame).collect();
        assert_eq!(frames, (1..=10).collect::<Vec<_>>());
        assert!((e + 33..f).all(|k| matches!(s(k).frame, 10 | 11)));
        assert_eq!(s(f + 2).frame, 12);
        assert_eq!(s(f + 28).frame, 20);
    }
    // Between the two saws Claw is never hurt; the saws make no sound.
    assert!(rows.iter().all(|l| l.health == 100));
    let saw_sounds = [saw_blade::RISE_SOUND, saw_blade::SINK_SOUND];
    assert!(
        rows.iter()
            .all(|l| l.sounds.iter().all(|s| !saw_sounds.contains(&s.as_str())))
    );
}

#[test]
fn install_level9_a_saw_woken_in_its_up_half_rises_at_once() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "saw-wake", 912, &SAWS);
    // Both wake on row 64 as Claw drops beside them; the second, inside its
    // up half, starts rising on the next row and is dangerous for two rows
    // only; the first waits for its clock.
    let first_seen = |i: usize| {
        rows.iter()
            .position(|l| l.objects[i].is_some_and(|o| o.state != 0))
    };
    assert_eq!((first_seen(0), first_seen(1)), (Some(64), Some(64)));
    assert_eq!(entries(&rows, 1, saw_blade::state::RISE_START)[0], 65);
    let up: Vec<usize> = (0..rows.len())
        .filter(|&k| rows[k].objects[1].is_some_and(|o| o.state != 0 && !o.off))
        .take_while(|&k| k < 120)
        .collect();
    assert_eq!(up, [97, 98]);
    assert_eq!(entries(&rows, 0, saw_blade::state::RISE_START)[0], 120);
    // Standing in its strike box, Claw loses 10 when it is up (row 152).
    assert_eq!(starts(&rows, |l| l.health == 90), [152]);
}

#[test]
fn install_level9_a_saw_hurts_twice_at_most_in_one_up_half() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "saw-hit", 712, &SAWS[..1]);
    // Put down on the saw while it was up, Claw lost 10 before time zero;
    // his protection over, the same up half takes 10 more on row 16 and
    // knocks him out of its reach for good.
    assert_eq!(rows[0].health, 90);
    let hits: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect();
    assert_eq!(hits, [16]);
    assert_eq!(rows[15].health - rows[16].health, 10);
    let sink = entries(&rows, 0, saw_blade::state::DROP_START);
    assert!(sink[0] > 16);
    assert!(rows[16].objects[0].is_some_and(|o| !o.off));
    assert_eq!(rows[711].x, 3304);
}

/// The crossfire shaft's cannons: 326, 328 and 330 firing right, 324, 322
/// and 320 firing left.
const SHAFT_CANNONS: [(&str, (i32, i32)); 6] = [
    ("SkullCannon", (7490, 6693)),
    ("SkullCannon", (7490, 7078)),
    ("SkullCannon", (7490, 7461)),
    ("SkullCannon", (8252, 6375)),
    ("SkullCannon", (8251, 6948)),
    ("SkullCannon", (8250, 7332)),
];

#[test]
fn install_level9_skull_cannons_fire_every_167_ticks_from_their_right() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "skull-shaft", 712, &SHAFT_CANNONS);
    let fire = EnemyState::Fire as i32;
    // The five cannons in reach woke together and fire together, 167 rows
    // apart, as long as Claw, knocked off the ledge by the second ball and
    // fallen to the shaft's floor, keeps them in reach; the top one on the
    // left (324) sleeps out of reach.
    let shots = entries(&rows, 1, fire);
    assert_eq!(shots, [100, 267]);
    for i in [0, 1, 2, 4, 5] {
        let e = entries(&rows, i, fire);
        assert_eq!(e[..2], shots, "cannon {i}");
        assert!(
            e.windows(2).all(|w| w[1] - w[0] == 167),
            "cannon {i}: {e:?}"
        );
    }
    assert!(entries(&rows, 3, fire).is_empty());
    // Each shot: its two sounds on C + 7 and C + 19, Home on C + 43 with
    // the last picture of the shot, image 1 again from the next row.
    let c = shots[0];
    let home = EnemyState::Home as i32;
    let s = |k: usize| rows[k].objects[1].expect("the cannon");
    assert_eq!(s(c + 42).state, fire);
    assert_eq!((s(c + 43).state, s(c + 43).frame), (home, 7));
    assert_eq!(s(c + 44).frame, 1);
    assert!(sound_rows(&rows, "LEVEL_CANONSH1").contains(&(c + 7)));
    assert!(sound_rows(&rows, "LEVEL_FLYUP").contains(&(c + 19)));
    // A ball reaches Claw on the ledge: 10 health.
    assert_eq!(starts(&rows, |l| l.health == 85), [138]);
    // Ducking does not help.
    let rows = watch(&level, "skull-duck", 712, &[]);
    assert!(
        rows[30..]
            .iter()
            .all(|l| l.state == state::CROUCH || l.health < 95)
    );
    assert_eq!(starts(&rows, |l| l.health == 85), [139]);
}

#[test]
fn install_level9_skull_cannon_balls_leave_50_px_right_whichever_way_the_cannon_faces() {
    let Some(level) = level9() else { return };
    let r = route_named("skull-shaft");
    let mut sim = at_time_zero(&level, "c18", r.start, r.t0_ms, SEED);
    let cannons: Vec<ObjectId> = SHAFT_CANNONS
        .iter()
        .map(|&(l, p)| find(&sim, l, p))
        .collect();
    let mut seen: Vec<(usize, ObjectId, (i32, i32), i32)> = Vec::new();
    // The balls fired before time zero are not followed.
    let mut before: Vec<ObjectId> = Vec::new();
    for &c in &cannons {
        if let Some(ObjectData::SkullCannon(d)) = sim.objects().get(c).map(|o| &o.data)
            && let Some(b) = d.last_ball
        {
            before.push(b);
        }
    }
    for k in 0..300 {
        sim.step(TickInput::default());
        for (i, &c) in cannons.iter().enumerate() {
            let Some(ObjectData::SkullCannon(d)) = sim.objects().get(c).map(|o| &o.data) else {
                continue;
            };
            if let Some(b) = d.last_ball
                && !seen.iter().any(|s| s.1 == b)
                && !before.contains(&b)
                && let Some(o) = sim.objects().get(b)
            {
                seen.push((i, b, (o.x, o.y), k));
                assert_eq!(o.frame, 1);
            }
        }
    }
    // Every ball starts at the cannon's x + 50 and y + 10.
    for &(i, _, at, _) in &seen {
        let (x, y) = SHAFT_CANNONS[i].1;
        assert_eq!(at, (x + 50, y + 10), "cannon {i}");
    }
    // A mirrored cannon's ball flies left through it, 9 px a row, showing
    // image 1 throughout.
    let &(_, ball, (x0, _), _) = seen.iter().find(|s| s.0 == 4).expect("a ball of 322");
    assert_eq!(x0, 8301);
    let mut xs = vec![x0];
    for _ in 0..10 {
        sim.step(TickInput::default());
        if let Some(o) = sim.objects().get(ball) {
            assert_eq!(o.frame, 1);
            xs.push(o.x);
        }
    }
    assert!(
        xs.windows(2).all(|w| (8..=9).contains(&(w[0] - w[1]))),
        "{xs:?}"
    );
}

#[test]
fn install_level9_a_dart_trap_puffs_shoots_and_waits() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "dart-right", 1112, &[("TProjectile", (1558, 7212))]);
    let firing = entries(&rows, 0, dart_trap::state::FIRING);
    let t = firing[0];
    assert_eq!(t, 29);
    // Every 87 rows while Claw stays in its rectangle.
    assert!(firing.windows(2).all(|w| w[1] - w[0] == 87), "{firing:?}");
    let s = |k: usize| rows[k].objects[0].expect("the trap");
    // No picture on T, then pictures 1 to 5, 6 rows each; none again from
    // T + 31, when it waits.
    assert_eq!(s(t).frame, 0);
    let frames: Vec<i32> = (t + 1..t + 31).map(|k| s(k).frame).collect();
    let want: Vec<i32> = (1..=5).flat_map(|f| [f; 6]).collect();
    assert_eq!(frames, want);
    assert_eq!(
        (s(t + 31).frame, s(t + 31).state),
        (0, dart_trap::state::WAIT)
    );
    assert!(rows.iter().enumerate().all(|(k, l)| {
        let o = l.objects[0].expect("the trap");
        o.state == dart_trap::state::FIRING && k > t || o.frame == 0
    }));
    // Its blow on T + 19, of every puff.
    let blows = sound_rows(&rows, dart_trap::SOUND);
    assert_eq!(blows, firing.iter().map(|&f| f + 19).collect::<Vec<_>>());
    // Each arrow costs 5 and knocks Claw 27 px.
    let hits: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect();
    assert_eq!(hits[0], 62);
    assert!(
        hits.iter()
            .all(|&k| rows[k - 1].health - rows[k].health == 5)
    );
    assert_eq!(rows[hits[1] - 1].x - rows[hits[0] - 1].x, 27);
}

#[test]
fn install_level9_dart_arrows_leave_from_the_trap_at_9_px_a_tick() {
    let Some(level) = level9() else { return };
    let r = route_named("dart-right");
    let mut sim = at_time_zero(&level, "c18", r.start, r.t0_ms, SEED);
    let trap = find(&sim, "TProjectile", (1558, 7212));
    let ticks = route_ticks(r);
    let mut arrow: Option<(usize, ObjectId)> = None;
    let mut xs = Vec::new();
    for k in 0..120 {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        if arrow.is_none()
            && let Some(ObjectData::DartTrap(d)) = sim.objects().get(trap).map(|o| &o.data)
            && let Some(a) = d.last_arrow
        {
            arrow = Some((k, a));
        }
        if let Some((_, a)) = arrow
            && let Some(o) = sim.objects().get(a)
        {
            xs.push(o.x);
            assert_eq!((o.y, o.frame), (7212, 1));
            assert_eq!(o.damage, dart_trap::arrow::DAMAGE);
        }
    }
    // Shot on T + 31 from the trap, flying right 9 px a row.
    assert_eq!(arrow.map(|a| a.0), Some(29 + 31));
    assert_eq!(xs[0], 1558);
    assert!(xs.windows(2).all(|w| w[1] - w[0] == 9), "{xs:?}");
}
