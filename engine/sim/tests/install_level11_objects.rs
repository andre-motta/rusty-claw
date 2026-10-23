//! The eleventh level's lasers, break planks, hidden belt and start room
//! on your own install, against the original's runs of the same short
//! routes.
//!
//! Each route but the last puts Claw down at a fixed point of the level
//! after its first tick, the way the original's runs put him there
//! (between two ticks, as a warp does), waits until the level clock
//! reaches the time the original's run started at, and starts the shared
//! random stream from the run's state, once. The routes:
//!
//! - beside the laser at (8675, 2719): Claw put down at 8780, out of its
//!   beam, stands (the laser's cycle); or put down in the beam at 8675 and
//!   stands (its two hits);
//! - left of the break plank at (5215, 1494): Claw put down at 5150 holds
//!   Right on rows 20 to 59, over its three pieces, which give way under
//!   him one after the other;
//! - on the ledge under the start room, left of its pillar: Claw put down
//!   at 3424 jumps on rows 30 to 59 into the pocket that holds the map
//!   piece, which ends the level;
//! - from the level's start, with the run's random state there: Right on
//!   rows 15 to 103 walks Claw into the opening of the start room's
//!   floor, and Left from row 104 while he falls past the ledge and its
//!   hidden belt, which he cannot reach.
//!
//! Each route is played tick by tick and compared with the reference
//! trace of the same route: at the 18 ms step with the original's speeds
//! every column exactly (Claw's position and state, health, score, the
//! camera, the random stream and the events), at 60 frames per second
//! with the default tuning within the feel tolerances. Divergences that
//! are known, with their causes, are listed in [`KNOWN`]; any other fails
//! the test. The routes are written below; each store's own copy of its
//! route must agree with them. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons: each test skips itself when what it needs is
//! unset, and fails when `CLAW_REFERENCE` is set but a listed store lacks
//! its route, meta or trace; `CLAW_L11_ONLY` names one store to play
//! alone. Nothing from an install or a reference store is kept in the
//! repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, GameEvent};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, siren};
use claw_sim::logic::{break_plank, laser};
use claw_sim::object::ObjectId;
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

/// A route of the stores: its name, where Claw is put down (none: the
/// level's start), the original's game time at its time zero at the 18 ms
/// step, its random state there, how many rows it has, its stretches and
/// the clocks it was run at.
struct Route {
    store: &'static str,
    start: Option<(i32, i32)>,
    t0_ms: u64,
    seed: u32,
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
}

/// The original's random state at the teleport routes' time zero.
const SEED: u32 = 111_567;
/// The original's random state at the level's start.
const START_SEED: u32 = 1_980_168_385;

/// The laser of the laser routes.
const LASER: (i32, i32) = (8675, 2719);
/// The break plank of the plank route.
const PLANK: (i32, i32) = (5215, 1494);

const ROUTES: [Route; 5] = [
    Route {
        store: "laser-cycle",
        start: Some((8780, 2747)),
        t0_ms: 828,
        seed: SEED,
        rows: 900,
        spans: &[],
        clocks: &["c18", "c60"],
    },
    Route {
        store: "laser-stand",
        start: Some((8675, 2747)),
        t0_ms: 828,
        seed: SEED,
        rows: 900,
        spans: &[],
        clocks: &["c18", "c60"],
    },
    Route {
        store: "plank-walk",
        start: Some((5150, 1339)),
        t0_ms: 828,
        seed: SEED,
        rows: 300,
        spans: &[span(20, 59, RIGHT)],
        clocks: &["c18"],
    },
    Route {
        store: "belt-left",
        start: Some((3424, 1467)),
        t0_ms: 828,
        seed: SEED,
        rows: 200,
        spans: &[span(30, 59, JUMP)],
        clocks: &["c18"],
    },
    Route {
        store: "start-drop",
        start: None,
        t0_ms: 0,
        seed: START_SEED,
        rows: 300,
        spans: &[span(15, 103, RIGHT), span(104, 249, LEFT)],
        clocks: &["c18"],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. None: every route follows
/// its store exactly at the 18 ms step and within the feel tolerances at
/// 60 frames per second, the Siren and the Mercat beside the laser, the
/// Mercat by the start room's opening and the fish of the first pool
/// included. What the original's runs did before their time zero, which
/// the replay does not play the same way, is put in step there instead
/// ([`put_siren_in_step`]). The laser beside the put-down point and the
/// Mercat by the start room's opening need nothing: the original's runs
/// put Claw down with the game's own teleport, which runs every awake
/// object's logic once more at the game time of the tick it comes in, and
/// the replay's warp does the same in its second pass, waking the laser
/// there and giving the Mercat his first idle pose there.
const KNOWN: &[Known] = &[];

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

fn level11() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(11)).unwrap();
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

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
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
    let Some(start) = r.start else {
        return sim;
    };
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(start));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    if r.store.starts_with("laser-") {
        put_siren_in_step(&mut sim, clock);
    }

    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// Where the Siren beside the laser stands.
const SIREN: (i32, i32) = (8559, 2783);

/// The Siren's wait at home at time zero, by clock: when it started, on
/// the replay's clock, and how long it is. The original built her in the
/// tick it put Claw down in, at the replay's game time 0 (10 at 60 frames
/// per second), and drew a wait of 1327 ms from the random stream before
/// time zero, which the replay does not have.
fn siren_wait_at_time_zero(clock: &str) -> enemy::Timer {
    if clock == "c18" {
        enemy::Timer::new(0, 1327)
    } else {
        enemy::Timer::new(10, 1327)
    }
}

/// Puts the Siren beside the laser where the original's run shows her at
/// time zero: at home, her wait started when she was built (see
/// [`siren_wait_at_time_zero`]), and not counting Claw out of her range
/// (the replay builds her a tick later, before she is drawn).
fn put_siren_in_step(sim: &mut Sim, clock: &str) {
    let id = find(sim, siren::NAMES[0], SIREN);
    sim.with_world(|w| {
        let o = w.objects.get(id).expect("the Siren");
        assert_eq!((o.x, o.y), SIREN, "the Siren stands where the run shows");
        assert_eq!(enemy::state(w, id), Some(EnemyState::Home));
        if let Some(s) = siren::siren_mut(w, id) {
            s.wait = siren_wait_at_time_zero(clock);
        }
        if let Some(d) = enemy::data_mut(w, id) {
            d.out_of_range = false;
            d.miss_since = None;
            d.deferred = None;
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

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs, and returns the replay's trace and the
/// reference trace. A store without the trace or its meta fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> (Vec<TraceRow>, Vec<TraceRow>) {
    let dir = root.join("level11").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
        assert_eq!(seed, r.seed, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, r, t0, seed);
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

/// Compares a play of `store` at `clock` with the reference trace at the
/// clock's tolerances (exact for c18, feel for c60); returns the
/// divergences not in [`KNOWN`].
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
        let dir = root.join("level11").join(r.store);
        let rows =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        // Each clock the route lists has its meta and its trace.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_level11_object_routes_follow_the_reference_traces() {
    let Some(level) = level11() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L11_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level11/{} {clock}", r.store);
            let (rows, reference) = play(&level, &root, r, clock);
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw, and the watched object's box and
/// picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    armed: bool,
    frame: i32,
    complete: bool,
    /// The laser's zap played on the row: 1 or 2, 0 for none.
    zap: u8,
}

/// The object of logic `name` nearest (x, y).
fn find(sim: &Sim, name: &str, (x, y): (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == name)
        .min_by_key(|(_, o)| (o.x - x).abs() + (o.y - y).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {name} near {x},{y}"))
}

/// Plays `rows` rows of `store`'s route at `clock` from its time zero
/// with the original's random state there, watching the object of logic
/// `name` nearest `near`.
fn watch(
    level: &SimLevel,
    store: &str,
    clock: &str,
    rows: usize,
    name: &str,
    near: (i32, i32),
) -> Vec<Look> {
    let r = route_named(store);
    let t0 = if clock == "c18" { r.t0_ms } else { 817 };
    let mut sim = at_time_zero(level, clock, r, t0, r.seed);
    let id = find(&sim, name, near);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        let o = sim.objects().get(id);
        looks.push(Look {
            x: c.x,
            y: c.y,
            state: c.state,
            health: sim.claw().stats.health,
            armed: o.is_some_and(|o| o.attack_rect.is_set()),
            frame: o.map_or(0, |o| o.frame),
            complete: sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
            zap: sim
                .last_audio()
                .iter()
                .find_map(|a| match a {
                    AudioCue::Play(s) if s.key == laser::ZAP1 => Some(1),
                    AudioCue::Play(s) if s.key == laser::ZAP2 => Some(2),
                    _ => None,
                })
                .unwrap_or(0),
        });
    }
    looks
}

/// The spans (first, last) of the rows on which `f` holds.
fn spans_of(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < rows.len() {
        if f(&rows[k]) {
            let a = k;
            while k + 1 < rows.len() && f(&rows[k + 1]) {
                k += 1;
            }
            out.push((a, k));
        }
        k += 1;
    }
    out
}

/// The rows on which Claw's health drops.
fn hits_on_claw(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect()
}

#[test]
fn install_level11_the_laser_fires_every_159_ticks_for_75() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "laser-cycle", "c18", 1012, "Laser", LASER);
    let armed = spans_of(&rows, |l| l.armed);
    // Its first live call came in the tick Claw was put down: armed from
    // row 38 for 75 rows, every 159 rows (the last pass runs past the
    // store's end).
    let mut want: Vec<(usize, usize)> = (0..6).map(|n| (38 + 159 * n, 112 + 159 * n)).collect();
    want.push((992, 1011));
    assert_eq!(armed, want);
    assert_eq!(laser::period_ticks(1500), 159);
    // Image 1 on the pass's first row, then one picture every 3 rows, the
    // 25th from row 110, and image 1 again on the finishing row.
    assert_eq!(rows[37].frame, 1);
    for k in 0..25 {
        assert_eq!(rows[38 + 3 * k].frame, 1 + k as i32, "row {}", 38 + 3 * k);
    }
    assert_eq!(rows[113].frame, 1);
    // The zaps come every 6 rows while the arc is drawn, from the pass's
    // 13th row, the first zap first, though Claw is not in view of them.
    let zaps: Vec<(usize, u8)> = (0..300)
        .filter(|&k| rows[k].zap != 0)
        .map(|k| (k, rows[k].zap))
        .collect();
    let want: Vec<(usize, u8)> = (0..9)
        .map(|i| (50 + 6 * i, if i % 2 == 0 { 1 } else { 2 }))
        .chain((0..9).map(|i| (209 + 6 * i, if i % 2 == 0 { 1 } else { 2 })))
        .collect();
    assert_eq!(zaps, want);
    // Claw out of the beam is never hurt.
    assert!(hits_on_claw(&rows).is_empty());
}

#[test]
fn install_level11_a_claw_in_the_beam_is_hit_twice() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "laser-stand", "c18", 400, "Laser", LASER);
    // Hit for 1 on the first armed row and 48 rows later, each knock 27
    // px; the second carries him out of reach for good.
    assert_eq!(hits_on_claw(&rows), [38, 86]);
    assert_eq!((rows[38].health, rows[86].health), (99, 98));
    assert_eq!(rows[37].x, 8675);
    assert_eq!(rows[85].x, 8648);
    assert_eq!(rows[133].x, 8621);
    assert_eq!(rows.last().unwrap().health, 98);
}

#[test]
fn install_level11_the_planks_give_way_one_after_the_other() {
    let Some(level) = level11() else { return };
    let r = route_named("plank-walk");
    let mut sim = at_time_zero(&level, "c18", r, r.t0_ms, r.seed);
    let ticks = route_ticks(r);
    // The row's three pieces, left to right.
    let mut pieces: Vec<(i32, ObjectId)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == break_plank::PIECE)
        .filter(|(_, o)| (o.y - PLANK.1).abs() < 8 && (o.x - PLANK.0).abs() < 200)
        .map(|(id, o)| (o.x, id))
        .collect();
    pieces.sort();
    assert_eq!(pieces.len(), 3, "{pieces:?}");
    let mut waits = vec![None; 3];
    let mut crumbles = vec![None; 3];
    let mut fall = None;
    for k in 0..200 {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        for (i, &(_, id)) in pieces.iter().enumerate() {
            let s = sim.objects().get(id).map_or(-1, |o| o.state);
            if s == break_plank::state::WAIT {
                waits[i].get_or_insert(k);
            }
            if s == break_plank::state::CRUMBLE {
                crumbles[i].get_or_insert(k);
            }
        }
        let c = sim.player().expect("Claw");
        if fall.is_none() && k > 60 && c.physics_type == claw_sim::physics::ptype::FALL {
            fall = Some(k);
        }
    }
    // The first piece waits from row 33, each crumbles 14 rows after it
    // started waiting and starts the next; Claw, stopped over the last
    // two, falls on row 101.
    assert_eq!(waits, [Some(33), Some(47), Some(61)]);
    assert_eq!(crumbles, [Some(47), Some(61), Some(75)]);
    assert_eq!(fall, Some(101));
}

#[test]
fn install_level11_the_map_piece_is_one_jump_from_the_ledge() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "belt-left", "c18", 60, "ConveyorBelt", (3613, 1486));
    let done = rows.iter().position(|l| l.complete);
    assert_eq!(done, Some(41), "the level ends 11 rows after the jump");
    assert_eq!(rows[41].y, 1396);
}

#[test]
fn install_level11_the_start_room_cannot_reach_the_ledge() {
    let Some(level) = level11() else { return };
    let rows = watch(
        &level,
        "start-drop",
        "c18",
        200,
        "ConveyorBelt",
        (3613, 1486),
    );
    // He falls past the ledge's height (1487) right of its end at 3639,
    // held by the shaft's wall at 3713, and lands on the ground below.
    let passing = rows
        .windows(2)
        .find(|p| p[0].y < 1487 && p[1].y >= 1487)
        .expect("he passes the ledge's height");
    assert!(passing[1].x >= 3697, "{passing:?}");
    assert_eq!((rows[138].x, rows[138].y), (3651, 1743));
    assert!(rows.iter().all(|l| !l.complete));
}
