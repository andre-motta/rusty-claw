//! The sixth level's goo vents and ground blowers on your own install,
//! against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does: the vents and blowers around him wake and run their first
//! call then, so their cycles start on the original's tick), lets him
//! settle, waits until the level clock reaches the time the original's run
//! started at, and starts the shared random stream from the run's state,
//! once. It is then played tick by tick and compared with the reference
//! trace of the same route: at the 18 ms step with the original's speeds
//! every column exactly (Claw's position and state, health, score, the
//! camera, the random stream and the events), at 60 frames per second
//! with the default tuning within the feel tolerances. The routes:
//!
//! - beside the vent at (19706, 6062): standing 16 px left of it, in its
//!   splash (hit, pushed 27 px away, hit again 48 ticks later, then out of
//!   reach), and standing 146 px left of it, out of reach, while it
//!   erupts every 205 ticks;
//! - on the blower at (31534, 5429): standing on its nozzle (thrown every
//!   85 ticks), Jump held throughout (the same), walking across it, Jump
//!   taps on every phase of its loop, one tap on the throw tick, one a
//!   tick before it and one coming down through the nozzle box; standing
//!   on the block 344 px left of it, off screen, where it hisses every
//!   loop;
//! - on the 400 px blower at (23408, 4790): standing on its nozzle,
//!   thrown every 78 ticks.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; when a store
//! holds its own copy of a route, the two must agree. Needs `CLAW_INSTALL`,
//! and `CLAW_REFERENCE` for the comparisons; each test skips itself when
//! what it needs is unset. Nothing from an install or a reference store is
//! kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::{goo_vent, ground_blower};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::state;
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

/// A route of the stores: its name, where Claw is put down, the clocks it
/// was run at, how many rows it has and its stretches (rows not covered
/// hold nothing). Routes whose stretches repeat give them as a pattern
/// instead ([`Route::every`]).
struct Route {
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    /// The original's game time at the route's time zero at the 18 ms
    /// step, ms: when Claw had settled at his place.
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    /// A stretch repeated: its first row, its length, the keys and the
    /// distance between two repeats, alternating between the keys listed.
    every: Option<(usize, usize, &'static [u32], usize)>,
}

const fn route(
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
) -> Route {
    Route {
        store,
        start,
        clocks,
        t0_ms,
        rows,
        spans,
        every: None,
    }
}

const C18: &[&str] = &["c18"];
const BOTH: &[&str] = &["c18", "c60"];

/// The vent of the routes and Claw's two places beside it.
const VENT: (i32, i32) = (19706, 6062);
const IN_SPLASH: (i32, i32) = (19690, 6002);
const OUT_OF_REACH: (i32, i32) = (19560, 6002);
/// The blower of the routes, Claw's place on its nozzle, 44 px left of it
/// and on the block 344 px left of it.
const BLOWER: (i32, i32) = (31534, 5429);
const ON_NOZZLE: (i32, i32) = (31534, 5455);
const LEFT_OF_NOZZLE: (i32, i32) = (31490, 5455);
const ON_THE_BLOCK: (i32, i32) = (31190, 5327);
/// The 400 px blower and Claw's place on its nozzle.
const BLOWER_400: (i32, i32) = (23408, 4790);
const ON_NOZZLE_400: (i32, i32) = (23408, 4815);

const ROUTES: [Route; 11] = [
    route("vent-stand", IN_SPLASH, BOTH, 1260, 900, &[]),
    route("vent-watch", OUT_OF_REACH, BOTH, 828, 900, &[]),
    route("blower-stand", ON_NOZZLE, BOTH, 1422, 1000, &[]),
    route(
        "blower-hold-jump",
        ON_NOZZLE,
        C18,
        1422,
        700,
        &[span(15, 699, JUMP, true)],
    ),
    Route {
        store: "blower-walk",
        start: LEFT_OF_NOZZLE,
        clocks: BOTH,
        t0_ms: 828,
        rows: 1000,
        spans: &[],
        every: Some((20, 20, &[RIGHT, LEFT], 20)),
    },
    Route {
        store: "blower-jump",
        start: ON_NOZZLE,
        clocks: BOTH,
        t0_ms: 1422,
        rows: 1000,
        spans: &[],
        every: Some((20, 2, &[JUMP], 37)),
    },
    route(
        "blower-jump-same",
        ON_NOZZLE,
        C18,
        1422,
        300,
        &[span(93, 94, JUMP, true)],
    ),
    route(
        "blower-jump-early",
        ON_NOZZLE,
        C18,
        1422,
        300,
        &[span(92, 93, JUMP, true)],
    ),
    route(
        "blower-jump-down",
        ON_NOZZLE,
        C18,
        1422,
        300,
        &[span(77, 78, JUMP, true)],
    ),
    route("blower-offscreen", ON_THE_BLOCK, C18, 828, 700, &[]),
    route("blower-400", ON_NOZZLE_400, BOTH, 1332, 450, &[]),
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
/// - c18, vent-watch, 16200 ms (rng): Claw has stood still for 17 s and
///   his idle animation reaches the picture on which he says one of his
///   idle lines, which draws from the random stream. The idle line is not
///   built (his idle animation plays, silent). The vent never reads the
///   stream.
/// - c60 (the default tuning played tick by tick): the vents and the
///   blowers count 18 ms ticks. A vent's picture lasts 12 ticks (216 ms)
///   where the original at 60 frames per second shows it for 12 frames
///   (200 ms), so a pass takes 3690 ms against 3417 ms and the splash
///   reaches Claw at other times (vent-stand). A blower's loop is 57 ticks
///   (1026 ms) against 58 frames (967 ms) and it sleeps 28 ticks of a
///   throw against 32 frames, so its throws come at other times; and a
///   thrown Claw rises 25 px in his first tick where the original's 16 and
///   17 ms frames give 22 and 24 px, so he is never left inside the
///   nozzle box for the second test (the original's 472 px throw).
const KNOWN: &[Known] = &[
    ("vent-watch", "c18", &[("rng", 16200)]),
    (
        "vent-stand",
        "c60",
        &[
            ("state_id", 333),
            ("health", 333),
            ("x", 450),
            ("cam_x", 450),
        ],
    ),
    (
        "blower-stand",
        "c60",
        &[
            ("y", 134),
            ("cam_y", 134),
            ("state_id", 1617),
            ("physics_type", 1617),
        ],
    ),
    (
        "blower-walk",
        "c60",
        &[
            ("state_id", 1150),
            ("physics_type", 1150),
            ("y", 1167),
            ("cam_y", 1167),
            ("score", 1700),
            ("x", 3100),
            ("cam_x", 3117),
        ],
    ),
    (
        "blower-jump",
        "c60",
        &[
            ("y", 134),
            ("cam_y", 134),
            ("state_id", 1617),
            ("physics_type", 1617),
        ],
    ),
    (
        "blower-400",
        "c60",
        &[
            ("y", 67),
            ("cam_y", 67),
            ("state_id", 1400),
            ("physics_type", 1400),
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
    let mut spans: Vec<Span> = r.spans.to_vec();
    if let Some((first, len, keys, gap)) = r.every {
        let mut from = first;
        let mut i = 0;
        while from < r.rows {
            spans.push(span(
                from,
                (from + len - 1).min(r.rows - 1),
                keys[i % keys.len()],
                true,
            ));
            from += gap;
            i += 1;
        }
    }
    for s in &spans {
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

fn level6() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(6)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

fn reference_root() -> Option<PathBuf> {
    let root = std::env::var_os("CLAW_REFERENCE").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_REFERENCE is not set");
    }
    root
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
/// the store's first trace runs; returns its rows and that trace, or
/// `None` when the store lacks the trace.
fn play(
    level: &SimLevel,
    root: &Path,
    r: &Route,
    clock: &str,
) -> Option<(Vec<TraceRow>, Vec<TraceRow>)> {
    let dir = root.join("level6").join(r.store);
    let text = std::fs::read_to_string(dir.join(format!("trace-{clock}-r1.csv"))).ok()?;
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) = time_zero(&dir, clock)?;
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
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
    Some((rows, reference))
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
        let Some(rows) = store_route(&root.join("level6").join(r.store)) else {
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
fn install_level6_vent_and_blower_routes_follow_the_reference_traces() {
    let Some(level) = level6() else { return };
    let Some(root) = reference_root() else { return };
    let mut failures = Vec::new();
    for r in &ROUTES {
        for &clock in r.clocks {
            println!("== level6/{} {clock}", r.store);
            let Some((rows, reference)) = play(&level, &root, r, clock) else {
                println!("skipped: no reference trace");
                continue;
            };
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    /// Claw's position, state and health.
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    /// Where his jump started and where it peaks.
    jump: (i32, i32),
    /// The watched object's picture and box, and whether it is live.
    frame: i32,
    attack_rect: Rect,
    live: bool,
    /// Hisses cued in the row.
    hisses: usize,
    /// The camera's centre.
    cam: (i32, i32),
}

/// Plays `rows` rows of route `store` at the 18 ms step from its time
/// zero, watching the object running `logic` placed at `at`.
fn watch(level: &SimLevel, store: &str, logic: &str, at: (i32, i32), rows: usize) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r.start, r.t0_ms, SEED);
    let id = find(&sim, logic, at);
    let ticks = route_ticks(r);
    (0..rows)
        .map(|k| {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            let c = sim.player().expect("Claw");
            let o = sim.objects().get(id).expect("the watched object");
            Look {
                x: c.x,
                y: c.y,
                state: c.state,
                health: sim.claw().stats.health,
                jump: (sim.claw().jump_start_y, sim.claw().jump_peak_y),
                frame: o.frame,
                attack_rect: o.attack_rect,
                live: o.is_listed(),
                hisses: cues(&sim, ground_blower::HISS),
                cam: sim.camera().current,
            }
        })
        .collect()
}

fn find(sim: &Sim, logic: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && (o.x, o.y) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} at {at:?}"))
}

fn cues(sim: &Sim, key: &str) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == key))
        .count()
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// Spans (first, last) of the rows on which `f` holds.
fn spans(rows: &[Look], f: impl Fn(&Look) -> bool + Copy) -> Vec<(usize, usize)> {
    starts(rows, f)
        .into_iter()
        .map(|a| {
            let b = (a..rows.len()).take_while(|&k| f(&rows[k])).last().unwrap();
            (a, b)
        })
        .collect()
}

/// The rows on which Claw is thrown: a jump starts whose peak is the
/// throw's height above him, on a row on which the blower shows its first
/// picture.
fn throws(rows: &[Look], height: i32) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| {
            let (start, peak) = rows[k].jump;
            rows[k].state == state::JUMP
                && rows[k].frame == ground_blower::THROW_FRAME
                && start == rows[k].y
                && peak == start - height
                && (rows[k - 1].state != state::JUMP || rows[k - 1].jump != rows[k].jump)
        })
        .collect()
}

/// The apex of the throw on row `k`: the first row at it and Claw's y
/// there.
fn apex(rows: &[Look], k: usize) -> (usize, i32) {
    let end = (k + 1..rows.len())
        .find(|&j| rows[j].y > rows[j - 1].y)
        .unwrap_or(rows.len());
    let j = (k..end).min_by_key(|&j| (rows[j].y, j)).unwrap();
    (j, rows[j].y)
}

#[test]
fn install_level6_the_vent_erupts_every_205_ticks_with_its_box_a_tick_late() {
    let Some(level) = level6() else { return };
    let rows = watch(&level, "vent-watch", "GooVent", VENT, 1000);
    // The vent woke with Claw's arrival between the first two ticks, so
    // its passes start on the original's rows: the first picture again
    // on rows 147 + 205n, for 25 rows (the last picture of one pass and
    // the first of the next).
    let ones = spans(&rows, |l| l.frame == 1);
    assert_eq!(
        ones,
        [(147, 171), (352, 376), (557, 581), (762, 786), (967, 991)]
    );
    // Dangerous from the row after the fourth picture appears, 132 rows.
    let danger = spans(&rows, |l| {
        goo_vent::attack_rect(l.frame).is_some() && l.attack_rect.is_set()
    });
    assert_eq!(
        danger[1..],
        [(197, 328), (402, 533), (607, 738), (812, 943)]
    );
    assert_eq!(danger[0], (0, 123), "mid-pass at time zero");
    // Every row's box is the one of the picture shown at its start: the
    // picture of the row before.
    for k in 1..rows.len() {
        let want = goo_vent::attack_rect(rows[k - 1].frame)
            .expect("pictures 1 to 9")
            .unwrap_or(Rect::UNSET);
        assert_eq!(rows[k].attack_rect, want, "row {k}");
    }
    // The widest box, 12 rows a pass.
    let widest = spans(&rows, |l| l.attack_rect == goo_vent::ATTACK_RECTS[5]);
    assert_eq!(widest[1], (257, 268));
    // Out of reach, Claw is never hurt and never moves.
    assert!(rows.iter().all(|l| l.health == 100 && l.x == 19560));
}

#[test]
fn install_level6_the_vent_takes_one_health_and_pushes_claw_27_px_away() {
    let Some(level) = level6() else { return };
    let rows = watch(&level, "vent-stand", "GooVent", VENT, 900);
    // Hit once before time zero (pushed from 19690 to 19663), then on rows
    // 16 and 64, 48 ticks apart while he is still in the splash.
    assert_eq!((rows[0].x, rows[0].health), (19663, 99));
    let hits = starts(&rows, |l| l.state == state::HIT);
    assert_eq!(hits, [16, 64]);
    let health: Vec<i32> = hits.iter().map(|&k| rows[k].health).collect();
    assert_eq!(health, [98, 97]);
    for &k in &hits {
        // 22 rows in the hit state, pushed 3 px a row away from the vent,
        // 27 px in all.
        let span = (k..rows.len())
            .take_while(|&j| rows[j].state == state::HIT)
            .count();
        assert_eq!(span, 22, "row {k}");
        assert_eq!(rows[k - 1].x - rows[k + 21].x, 27, "row {k}");
    }
    // At 19609 his box no longer meets the widest splash: unhurt to the
    // end.
    assert!(rows[86..].iter().all(|l| l.x == 19609 && l.health == 97));
}

#[test]
fn install_level6_a_claw_standing_on_a_blower_is_thrown_every_85_ticks() {
    let Some(level) = level6() else { return };
    let rows = watch(&level, "blower-stand", "GroundBlower", BLOWER, 1000);
    let thrown = throws(&rows, 450);
    let want: Vec<usize> = (0..12).map(|n| 8 + 85 * n).collect();
    assert_eq!(thrown, want);
    assert_eq!(thrown, starts(&rows, |l| l.state == state::JUMP));
    for &k in &thrown {
        // From standing at 5458 on the blower's first picture; up 25 px on
        // the next row, 450 px up 29 rows after the throw, straight up.
        assert_eq!(rows[k].frame, ground_blower::THROW_FRAME, "row {k}");
        assert_eq!(rows[k].y, 5458);
        assert_eq!(rows[k + 1].y, 5458 - 25);
        assert_eq!(apex(&rows, k), (k + 29, 5458 - 450), "row {k}");
        assert!(rows[k..].iter().take(60).all(|l| l.x == 31534));
    }
    // The blower falls asleep while the camera follows Claw up, so its
    // loop pauses: it leaves the live list after its call in row 26 of
    // each throw and wakes for row 55, missing 28 calls (57 calls and 28
    // missed make the 85 rows between two throws).
    let asleep = spans(&rows, |l| !l.live);
    let want: Vec<(usize, usize)> = (0..11).map(|n| (26 + 85 * n, 54 + 85 * n)).collect();
    assert_eq!(asleep[..11], want);
    // The hiss once a loop, six rows after the first throw row.
    let hisses: Vec<usize> = (0..rows.len()).filter(|&k| rows[k].hisses > 0).collect();
    let want: Vec<usize> = (0..12).map(|n| 14 + 85 * n).collect();
    assert_eq!(hisses, want);
    assert!(rows.iter().all(|l| l.hisses <= 1));
}

#[test]
fn install_level6_holding_jump_on_a_blower_changes_nothing() {
    let Some(level) = level6() else { return };
    let stand = watch(&level, "blower-stand", "GroundBlower", BLOWER, 700);
    let hold = watch(&level, "blower-hold-jump", "GroundBlower", BLOWER, 700);
    let path = |rows: &[Look]| -> Vec<(i32, i32, i32)> {
        rows.iter().map(|l| (l.x, l.y, l.state)).collect()
    };
    assert_eq!(path(&hold), path(&stand));
}

#[test]
fn install_level6_the_400_px_blower_throws_every_78_ticks() {
    let Some(level) = level6() else { return };
    let rows = watch(&level, "blower-400", "GroundBlower", BLOWER_400, 450);
    let thrown = throws(&rows, 400);
    let want: Vec<usize> = (0..6).map(|n| 5 + 78 * n).collect();
    assert_eq!(thrown, want);
    for &k in &thrown {
        assert_eq!(rows[k].y, 4818);
        assert_eq!(rows[k + 1].y, 4818 - 24, "row {k}");
        assert_eq!(apex(&rows, k), (k + 28, 4818 - 400), "row {k}");
    }
    let asleep = spans(&rows, |l| !l.live);
    let want: Vec<(usize, usize)> = (0..5).map(|n| (25 + 78 * n, 46 + 78 * n)).collect();
    // It misses 21 calls of each throw: 57 and 21 make 78.
    assert_eq!(asleep[..5], want);
}

#[test]
fn install_level6_a_blower_throws_from_a_walk_a_hop_or_a_fall() {
    let Some(level) = level6() else { return };
    // Walking across the nozzle: thrown out of the walk, either way.
    let rows = watch(&level, "blower-walk", "GroundBlower", BLOWER, 1000);
    let thrown = throws(&rows, 450);
    assert_eq!(thrown[..5], [68, 153, 241, 326, 411]);
    let walks: Vec<i32> = thrown[..5].iter().map(|&k| rows[k - 1].state).collect();
    assert_eq!(
        walks,
        [
            state::MOVE_RIGHT,
            state::MOVE_RIGHT,
            state::MOVE_LEFT,
            state::MOVE_LEFT,
            state::MOVE_LEFT
        ]
    );
    // A hop on the throw row is replaced by the throw, from where the hop
    // took him; a hop a row earlier and a fall through the nozzle box are
    // thrown from their height too, 450 px above it.
    for (store, from, before) in [
        ("blower-jump-same", 5450, state::STAND),
        ("blower-jump-early", 5443, state::JUMP),
        ("blower-jump-down", 5445, state::FALL),
    ] {
        let rows = watch(&level, store, "GroundBlower", BLOWER, 300);
        assert_eq!(rows[93].frame, ground_blower::THROW_FRAME, "{store}");
        assert_eq!(
            (rows[92].state, rows[93].state, rows[93].y),
            (before, state::JUMP, from),
            "{store}"
        );
        assert_eq!(throws(&rows, 450)[..2], [8, 93], "{store}");
        assert_eq!(apex(&rows, 93), (122, from - 450), "{store}");
    }
}

#[test]
fn install_level6_a_live_blower_hisses_off_screen() {
    let Some(level) = level6() else { return };
    let rows = watch(&level, "blower-offscreen", "GroundBlower", BLOWER, 812);
    // Claw on the block left of it: the blower is live but off screen,
    // never throws him, and hisses once a loop, every 57 rows.
    assert!(rows.iter().all(|l| l.live && l.state == state::STAND));
    assert!(rows.iter().all(|l| BLOWER.0 - l.cam.0 > 320));
    let hisses: Vec<usize> = (0..rows.len()).filter(|&k| rows[k].hisses > 0).collect();
    let want: Vec<usize> = (0..14).map(|n| 17 + 57 * n).collect();
    assert_eq!(hisses, want);
    let ones = starts(&rows, |l| l.frame == ground_blower::THROW_FRAME);
    let want: Vec<usize> = (0..15).map(|n| 11 + 57 * n).collect();
    assert_eq!(ones, want);
}

#[test]
fn install_level6_every_vent_and_blower_sets_up_from_its_record() {
    let Some(level) = level6() else { return };
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: claw_sim::ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 2);
    let (mut vents, mut heights) = (0, std::collections::BTreeMap::new());
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "GooVent" => {
                vents += 1;
                assert_eq!(o.state, goo_vent::state::IDLE);
                assert_eq!(o.type_flags, goo_vent::TYPE);
                assert_eq!(o.image_set, goo_vent::IMAGE_SET);
                let d = goo_vent::data_of(o).expect("vent data");
                assert_eq!(d.wait_ms, 0);
            }
            "GroundBlower" => {
                assert_eq!(o.state, ground_blower::state::IDLE);
                assert_eq!(o.z, 5000);
                let d = ground_blower::data_of(o).expect("blower data");
                assert_eq!(d.nozzle, Some(Rect::new(-16, 66, 21, 89)));
                *heights.entry(d.height).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    assert_eq!(vents, 12);
    assert_eq!(
        heights.into_iter().collect::<Vec<_>>(),
        [(400, 2), (450, 12)]
    );
}
