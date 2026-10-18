//! The fifth level's seagulls on your own install, against the
//! original's runs of the same short routes.
//!
//! Each route puts Claw on the floor under the seagull that patrols from
//! x 6462 to 6736 at height 2662, lets him settle, waits until the level
//! clock reaches the time the original's run started at, and starts the
//! shared random stream from the run's state. It is then played tick by
//! tick and compared with the reference trace of the same route: at the
//! 18 ms step with the original's speeds, every column exactly (Claw's
//! position and state, health, score, the camera, the random stream and
//! the events); at 60 frames per second with the default tuning, within
//! the feel tolerances. The routes:
//!
//! - standing still: the bird dives just after turning at its right limit,
//!   its beak knocks Claw back 27 px for 10 health 26 ticks into the dive,
//!   and it dives again later;
//! - jumping into the first dive: the beak meets Claw in the air;
//! - the same, then Jump pressed again on the tick after he is put back
//!   on his feet in the air.
//!
//! Divergences that are known, with their causes, are listed in
//! [`KNOWN`]; any other fails the test.
//!
//! The routes are written below; when a store holds its own copy of a
//! route, the two must agree. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons; each test skips itself when what it needs is unset.
//! Nothing from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{EnemyState, seagull};
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// One stretch of a route: from `from` to `to` (inclusive) `held` is down,
/// pressed on the stretch's first row when `press` is set.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    held: u32,
    press: bool,
}

const fn span(from: usize, to: usize, held: u32, press: bool) -> Span {
    Span {
        from,
        to,
        held,
        press,
    }
}

/// A route of the stores: its name, the clocks it was run at, how many
/// rows it has and its stretches (rows not covered hold nothing).
struct Route {
    store: &'static str,
    clocks: &'static [&'static str],
    rows: usize,
    spans: &'static [Span],
}

/// Where Claw is put down: under the bird, 143 px below its patrol line
/// once he stands.
const START: (i32, i32) = (6490, 2780);

/// Jump held on rows 36 to 43: the beak meets Claw in the air.
const JUMP_INTO_DIVE: Span = span(36, 43, JUMP, true);

const ROUTES: [Route; 3] = [
    Route {
        store: "gull-dive",
        clocks: &["c18", "c60"],
        rows: 850,
        spans: &[],
    },
    Route {
        store: "gull-air",
        clocks: &["c18", "c60"],
        rows: 300,
        spans: &[JUMP_INTO_DIVE],
    },
    Route {
        store: "gull-air-jump",
        clocks: &["c18"],
        rows: 300,
        spans: &[JUMP_INTO_DIVE, span(68, 75, JUMP, true)],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, 36 ms (rng): the town guards are not built yet. In the original
///   three of them are awake around Claw and each makes the enemies'
///   per-tick roll on every frame pulse tick (seven draws a pulse tick
///   with the seagulls' four), so the random stream runs ahead of the
///   game's from the first pulse tick. Nothing the seagulls do reads it:
///   their roll only has to be below 1000.
/// - c18, 5922 ms (gull-dive): Claw, knocked off the bird's floor by its
///   beaks, lands beside a first town guard, whose strike costs him 10
///   health in the original; here the guard stands still.
/// - c60 (the default tuning played tick by tick): Claw's hit reaction
///   lasts 22 calls, 396 ms in 18 ms ticks against 22 frames of 16 and
///   17 ms, so he stands (1167 ms) and then falls (the y columns) a tick
///   later than the original. The beak's hits themselves come on the
///   original's frames.
const KNOWN: &[Known] = &[
    (
        "gull-dive",
        "c18",
        &[
            ("rng", 36),
            ("state_id", 5922),
            ("health", 5922),
            ("x", 5940),
            ("cam_x", 5940),
        ],
    ),
    ("gull-air", "c18", &[("rng", 36)]),
    ("gull-air-jump", "c18", &[("rng", 36)]),
    (
        "gull-dive",
        "c60",
        &[("state_id", 1167), ("y", 4733), ("cam_y", 4733)],
    ),
    (
        "gull-air",
        "c60",
        &[
            ("state_id", 1167),
            ("physics_type", 1167),
            ("y", 1350),
            ("cam_y", 1350),
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
        if s.press {
            pressed[s.from] |= s.held;
        }
    }
    held.iter()
        .zip(&pressed)
        .map(|(&h, &p)| TickInput {
            held: Buttons(h),
            pressed: Buttons(p),
        })
        .collect()
}

/// The rows of a store's own route file: (held, pressed) by row.
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

fn level5() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(5)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn reference_root() -> Option<PathBuf> {
    let root = std::env::var_os("CLAW_REFERENCE").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_REFERENCE is not set");
    }
    root
}

/// The tuning a clock's runs are compared with: the original's speeds at
/// the 18 ms step, the default tuning at 60 frames per second.
fn tuning(clock: &str) -> ScriptTuning {
    if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    }
}

/// The game at a route's time zero: after the level's first tick Claw is
/// put down at `start` the way the original's runs put him there, between
/// two ticks as a warp does (the camera jumps to him, and the objects
/// around him wake and are built before the next tick, so the seagulls
/// fly from that tick on); he settles, the game idles until the level
/// clock is `t0_ms`, and the random stream starts from `seed`.
fn at_time_zero(level: &SimLevel, clock: &str, start: (i32, i32), t0_ms: u64, seed: u32) -> Sim {
    let cfg = SimConfig {
        tuning: tuning(clock).tuning(),
        seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(start));
    let mut still = 0;
    let mut last = None;
    for _ in 0..40 {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
    assert!(still >= 10, "Claw settles at {start:?}");
    assert!(sim.tick() * TICK <= t0_ms, "settled before time zero");
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
/// the store's first trace runs; returns the game, its rows and that
/// trace, or `None` when the store lacks the trace.
fn play(
    level: &SimLevel,
    root: &Path,
    r: &Route,
    clock: &str,
) -> Option<(Sim, Vec<TraceRow>, Vec<TraceRow>)> {
    let dir = root.join("level5").join(r.store);
    let text = std::fs::read_to_string(dir.join(format!("trace-{clock}-r1.csv"))).ok()?;
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) = time_zero(&dir, clock)?;
    let mut sim = at_time_zero(level, clock, START, t0, seed);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut k = 0u64;
    while k * TICK < end_ms {
        sim.step(ticks.get(k as usize).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k, "script"));
        k += 1;
    }
    Some((sim, rows, reference))
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
        let Some(rows) = store_route(&root.join("level5").join(r.store)) else {
            println!("{}: no route in the store", r.store);
            continue;
        };
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
fn install_level5_seagull_routes_follow_the_reference_traces() {
    let Some(level) = level5() else { return };
    let Some(root) = reference_root() else { return };
    let mut failures = Vec::new();
    for r in &ROUTES {
        for &clock in r.clocks {
            println!("== level5/{} {clock}", r.store);
            let Some((_, rows, reference)) = play(&level, &root, r, clock) else {
                println!("skipped: no reference trace");
                continue;
            };
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

fn bird(sim: &Sim, record: i32) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == "Seagull" && o.id == record)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("Seagull {record}"))
}

/// The seagull's state, position and velocity after each row of the
/// standing route at the 18 ms step.
#[test]
fn install_level5_the_test_rooms_seagull_dives_on_the_originals_ticks() {
    let Some(level) = level5() else { return };
    let mut sim = at_time_zero(&level, "c18", START, 828, 111_567);
    let id = bird(&sim, 6404);
    let ticks = route_ticks(&ROUTES[0]);
    let mut rows = Vec::new();
    let mut dive_cues = Vec::new();
    for (k, t) in ticks.iter().enumerate().take(260) {
        sim.step(*t);
        let o = sim.objects().get(id).expect("the bird");
        let d = claw_sim::logic::enemy::data_of(o).expect("built");
        rows.push((
            EnemyState::from_id(o.state),
            o.x,
            o.y,
            d.vx,
            d.vy,
            o.attack_rect,
        ));
        if sim.last_audio().iter().any(|c| {
            matches!(c, claw_sim::claw_frame::AudioCue::Play(s) if s.key == seagull::DIVE_SOUND)
        }) {
            dive_cues.push(k);
        }
    }
    let changes: Vec<(usize, EnemyState)> = (0..rows.len())
        .filter(|&k| k == 0 || rows[k].0 != rows[k - 1].0)
        .map(|k| (k, rows[k].0.expect("an enemy state")))
        .collect();
    println!("{changes:?}");
    // The first dive on row 19, just after the turn at 6736; it reaches
    // the left limit after 36 rows and climbs back for 11.
    assert_eq!(
        &changes[..5],
        [
            (0, EnemyState::Idle),
            (19, EnemyState::Strike),
            (55, EnemyState::Withdraw),
            (66, EnemyState::Idle),
            (193, EnemyState::Strike),
        ]
    );
    assert_eq!(dive_cues[..2], [19, 193]);
    let (_, x, y, vx, vy, beak) = rows[19];
    assert_eq!((x, y), (6726, 2665));
    assert!(
        (vx - -421.8).abs() < 0.05 && (vy - 178.2).abs() < 0.05,
        "{vx} {vy}"
    );
    assert_eq!(beak, seagull::BEAK, "facing left");
    // The withdraw starts on the limit, below Claw's head, and ends just
    // above the patrol height, put back on it on the next row.
    assert_eq!((rows[55].1, rows[55].2), (6462, 2766));
    assert_eq!(rows[55].5, claw_sim::object::Rect::UNSET);
    assert_eq!(rows[65].2, 2658);
    assert_eq!((rows[66].1, rows[66].2), (6464, 2662));
    assert_eq!(rows[66].3, 120.0);
}
