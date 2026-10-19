//! The eleventh level's floor clip on your own install, against the
//! original's runs of the same short routes.
//!
//! Each route puts Claw down on the floor of the first fish pool (x 4067
//! to 4450) after the level's first tick, the way the original's runs put
//! him there (between two ticks, as a warp does), waits until the level
//! clock reaches the time the original's run started at, and starts the
//! shared random stream from the run's state, once. The Mercat by the
//! start room's opening is put in step there ([`put_in_step`]). The
//! routes:
//!
//! - Left on rows 15 to 39 walks Claw to the pool's left wall, and Down
//!   and Left from row 40 to row 649 keep him crouched there. The fish's
//!   touches knock him back and forth; the one on row 438 slides him,
//!   still crouched, under the ledge left of the wall, where the floor is
//!   higher than his feet. When the hit ends he stands up inside the
//!   ledge and the floor, falls through them and lands in the hall below.
//! - The same with a stick of dynamite thrown left first: Left on rows 15
//!   to 27, Dynamite on rows 30 to 37, Left on rows 56 to 65 and Down and
//!   Left from row 66. The blast between Claw and the wall pushes him
//!   away from it, and he never slides under the ledge.
//!
//! A Claw put down with his feet inside a floor falls through it the same
//! way, as the original's runs settle after their teleports
//! ([`settle`]).
//!
//! Each route is played tick by tick and compared with the reference
//! trace of the same route: at the 18 ms step with the original's speeds
//! every column exactly (Claw's position and state, health, score, the
//! camera, the random stream and the events), at 60 frames per second
//! with the default tuning within the feel tolerances. When the reference
//! run also lists the objects near Claw per step (`obj-log.csv` beside the
//! run, in the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next
//! to the reference store), the pool's two fish are compared too, their
//! position, state and health exactly at the 18 ms step. Divergences that
//! are known, with their causes, are listed in [`KNOWN`]; any other fails
//! the test. The routes are written below; each store's own copy of its
//! route must agree with them. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons: each test skips itself when what it needs is
//! unset, and fails when `CLAW_REFERENCE` is set but a listed store lacks
//! its route, meta or trace (or, with the reference runs there, a run's
//! `obj-log.csv` or a watched fish in it). Nothing from an install or a
//! reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{EnemyState, fish, mercat};
use claw_sim::logic::{self, enemy};
use claw_sim::object::{ObjectId, engine_state};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The dynamite key.
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

/// A route of the stores: its name, the original's game time at its time
/// zero at the 18 ms step, how many rows it has, its stretches and the
/// clocks it was run at.
struct Route {
    store: &'static str,
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
}

/// Where both routes put Claw down: above the first pool's floor.
const START: (i32, i32) = (4140, 1851);
/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// The Mercat by the start room's opening.
const START_MERCAT: (i32, i32) = (3935, 1764);
/// The pool's two fish: their serial numbers in the original's runs and
/// where they swim at time zero.
const FISH: [(&str, (i32, i32)); 2] = [("3107", (4103, 1871)), ("3106", (4266, 1871))];

const ROUTES: [Route; 2] = [
    Route {
        store: "clip-clip-crouch",
        t0_ms: 828,
        rows: 700,
        spans: &[span(15, 39, LEFT), span(40, 649, LEFT | DOWN)],
        clocks: &["c18", "c60"],
    },
    Route {
        store: "clip-clip-dynamite",
        t0_ms: 828,
        rows: 700,
        spans: &[
            span(15, 27, LEFT),
            span(30, 37, DYNAMITE),
            span(56, 65, LEFT),
            span(66, 649, LEFT | DOWN),
        ],
        clocks: &["c18"],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. Both routes follow their
/// stores exactly at the 18 ms step, the fish included. What the others
/// come from:
///
/// - c60, clip-clip-crouch: the route's rows are the original's frames at
///   60 frames per second, played here one per 18 ms tick at the default
///   tuning, so the first hit's end and the walk to the wall come at other
///   times and Claw crouches 6 px further from the wall (4105, the
///   original 4099), within the feel tolerance. The fish then meet him at
///   other places: the third touch (1818 ms) comes from his left and
///   knocks him right, where the original's (1800 ms) knocked him under
///   the ledge. He goes under it on the fifth touch instead (8010 ms),
///   stands up inside it and falls through to the same ledge in the hall
///   (3992, 2511; the original's 3989, 2511), and his later fall and death
///   on the hall's planks follow later too.
const KNOWN: &[Known] = &[(
    "clip-clip-crouch",
    "c60",
    &[
        ("x", 1817),
        ("cam_x", 1817),
        ("physics_type", 1833),
        ("state_id", 2183),
        ("y", 2267),
        ("cam_y", 2267),
        ("score", 3017),
        ("health", 4433),
        ("lives", 7433),
        ("events", 7433),
    ],
)];

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
        // A stretch adding a key to one already held presses only the new
        // key.
        let before = if s.from > 0 { held[s.from - 1] } else { 0 };
        pressed[s.from] |= s.held & !before;
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

/// The object of logic `name` placed at `at`.
fn placed(sim: &Sim, name: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == name)
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {name} placed at {at:?}"))
}

/// The object of logic `name` nearest (x, y).
fn nearest(sim: &Sim, name: &str, (x, y): (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == name)
        .min_by_key(|(_, o)| (o.x - x).abs() + (o.y - y).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {name} near {x},{y}"))
}

/// Puts the Mercat by the start room's opening in step with the
/// original's runs at time zero: built in the level's first tick and idle
/// since, Claw out of his range boxes, his idle pose's wait started in
/// that tick.
fn put_in_step(sim: &mut Sim) {
    let id = placed(sim, mercat::NAMES[0], START_MERCAT);
    sim.with_world(|w| {
        if w.objects
            .get(id)
            .is_some_and(|o| o.state == engine_state::INIT)
        {
            logic::tick(w, id);
            if let Some(d) = enemy::data_mut(w, id) {
                d.last_move_ms = Some(0);
            }
        }
        enemy::set_state(w, id, EnemyState::Idle);
        if let Some(d) = enemy::data_mut(w, id) {
            d.out_of_range = true;
        }
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.wait = enemy::Timer::new(0, 1000);
            m.idle = enemy::Timer::new(0, mercat::IDLE_MS);
        }
    });
}

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, t0_ms: u64, seed: u32) -> Sim {
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
    sim.with_world(|w| w.claw.special.warp_to = Some(START));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    put_in_step(&mut sim);
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

/// A fish at one step of a run or of the replay: position, state (the
/// run's own numbers lined up through [`reference_state`]) and health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FishAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
    health: i32,
}

/// The state a reference run's number stands for, for a fish.
fn reference_state(id: i32) -> Option<EnemyState> {
    Some(match id {
        2004 => EnemyState::Dead,
        2069 => EnemyState::Swim,
        2065 => EnemyState::Snip,
        2078 => EnemyState::JumpStrike,
        2079 => EnemyState::OutOfWater,
        _ => return None,
    })
}

/// The steps of the pool's fish in a run's `obj-log.csv`, by fish: (step,
/// the fish there).
fn reference_fish(path: &Path) -> Option<Vec<Vec<(usize, FishAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, health) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("health")?,
    );
    let mut out = vec![Vec::new(); FISH.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = FISH.iter().position(|w| f.get(serial) == Some(&w.0)) else {
            continue;
        };
        out[i].push((
            f[tick].parse().ok()?,
            FishAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(int(f[st])?),
                health: int(f[health])?,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's fish leaves the
/// run's, by column (`<serial>_x`, `_y`, `_state`, `_health`); a step on
/// which the replay has no such fish counts for all four.
fn compare_fish(
    serial: &str,
    played: &[Option<FishAt>],
    reference: &[(usize, FishAt)],
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
            for name in ["x", "y", "state", "health"] {
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
    }
    firsts
}

/// One play of a route: its trace, the reference trace, and the fish on
/// each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    fish: Vec<Vec<Option<FishAt>>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level11").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, t0, seed);
    let ids: Vec<ObjectId> = FISH
        .iter()
        .map(|w| nearest(&sim, fish::NAMES[0], w.1))
        .collect();
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut fish = vec![Vec::new(); ids.len()];
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            fish[i].push(sim.objects().get(id).map(|o| FishAt {
                x: o.x,
                y: o.y,
                state: EnemyState::from_id(o.state),
                health: o.health,
            }));
        }
        k += 1;
    }
    Played {
        rows,
        reference,
        fish,
    }
}

/// Compares a play of route `r` at `clock` with the reference trace at
/// the clock's tolerances (exact for c18, feel for c60), and at c18 its
/// fish with the run's (`fish`, when the run lists them); returns the
/// divergences not known.
fn new_divergences(
    r: &Route,
    clock: &str,
    played: &Played,
    fish: Option<&[Vec<(usize, FishAt)>]>,
) -> Vec<String> {
    let store = r.store;
    let (rows, reference) = (&played.rows, &played.reference);
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
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| Some((c.column.clone(), c.first_t_ms?)))
        .collect();
    if let Some(f) = fish {
        for (i, w) in FISH.iter().enumerate() {
            firsts.extend(compare_fish(w.0, &played.fish[i], &f[i]));
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
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        let teleport = meta_value(&meta, "c18", "teleport").expect("a teleport");
        assert_eq!(teleport, format!("{},{}", START.0, START.1), "{}", r.store);
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
fn install_floor_clip_routes_follow_the_reference_traces() {
    let Some(level) = level11() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in &ROUTES {
        let dir = root.join("level11").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for &clock in r.clocks {
            println!("== level11/{} {clock}", r.store);
            let played = play(&level, &root, r, clock);
            let fish = runs
                .as_ref()
                .filter(|_| clock == "c18")
                .and_then(|d| Some(d.join(run_id(&meta, clock)?).join("obj-log.csv")))
                .and_then(|p| reference_fish(&p));
            if clock == "c18" && fish.is_none() {
                if runs.is_some() {
                    failures.push(format!("{}: the run's object log lacks a fish", r.store));
                } else {
                    println!("the runs' object logs are not there: the fish are not compared");
                }
            }
            failures.extend(new_divergences(r, clock, &played, fish.as_deref()));
        }
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
    physics_type: i32,
    health: i32,
    lives: i32,
}

/// Plays `rows` rows of `store`'s route at `clock` from its time zero
/// with the original's random state there.
fn watch(level: &SimLevel, store: &str, clock: &str, rows: usize) -> Vec<Look> {
    let r = route_named(store);
    let t0 = if clock == "c18" { r.t0_ms } else { 817 };
    let mut sim = at_time_zero(level, clock, t0, SEED);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        looks.push(Look {
            x: c.x,
            y: c.y,
            state: c.state,
            physics_type: c.physics_type,
            health: sim.claw().stats.health,
            lives: sim.claw().stats.lives,
        });
    }
    looks
}

/// The rows on which Claw's health drops.
fn hits_on_claw(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect()
}

/// The first row from `from` on which `f` holds.
fn first_from(rows: &[Look], from: usize, f: impl Fn(&Look) -> bool) -> Option<usize> {
    (from..rows.len()).find(|&k| f(&rows[k]))
}

#[test]
fn install_floor_clip_a_crouched_claw_knocked_under_the_ledge_falls_through_it() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "clip-clip-crouch", "c18", 520);
    // Crouched at the wall from row 40 on, knocked back and forth by the
    // fish; the touch on row 438 at x 4084 pushes him left at 200 px/s.
    assert_eq!((rows[437].x, rows[437].state), (4084, state::CROUCH));
    assert_eq!(rows[438].state, state::HIT);
    assert_eq!(rows[438].health, 40);
    // 3 px a row under the ledge, still crouched: nothing under him from
    // row 444 at 4066, stopped at 4057 on row 447.
    let xs: Vec<i32> = rows[439..=447].iter().map(|l| l.x).collect();
    assert_eq!(xs, [4081, 4078, 4075, 4072, 4069, 4066, 4063, 4060, 4057]);
    assert_eq!(rows[443].physics_type, ptype::STAND);
    assert!(
        rows[444..460]
            .iter()
            .all(|l| l.physics_type == ptype::FALL && l.y == 1871 && l.state == state::HIT)
    );
    // The hit ends on row 460: he stands up inside the ledge and the
    // floor, then falls through the floor and the slab under it, passing
    // y 2047 on row 483.
    assert_eq!((rows[460].x, rows[460].state), (4057, state::STAND));
    assert!(
        rows[461..512]
            .iter()
            .all(|l| l.state == state::FALL && l.physics_type == ptype::FALL)
    );
    let past = rows.iter().position(|l| l.y > 2047);
    assert_eq!(past, Some(483));
    // He lands in the hall under the pool on row 512, health 40.
    assert_eq!((rows[512].x, rows[512].y), (4001, 2511));
    assert_eq!(rows[512].physics_type, ptype::WALK);
    assert_eq!(rows[512].health, 40);
}

#[test]
fn install_floor_clip_the_same_fall_at_60_frames_per_second() {
    // The original at 60 frames per second: touched on frame 107 at 4072,
    // nothing under him from 109, up on 129, down in the hall on 184.
    if let Some(root) = reference_root() {
        let path = root.join("level11/clip-clip-crouch/trace-c60-r1.csv");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let rows = trace::parse(&text).unwrap();
        let at = |k: usize| (rows[k].x, rows[k].y, rows[k].state_id, rows[k].physics_type);
        assert_eq!(at(106), (4072, 1871, state::CROUCH, ptype::DUCK));
        assert_eq!(at(107).2, state::HIT);
        assert_eq!(at(108).3, ptype::STAND);
        assert_eq!(at(109).3, ptype::FALL);
        assert_eq!(at(128).2, state::HIT);
        assert_eq!(at(129), (4048, 1871, state::STAND, ptype::STAND));
        assert_eq!(at(183).2, state::FALL);
        assert_eq!((at(184).0, at(184).1), (3989, 2511));
    }
    // The replay at the default tuning: the walk to the wall ends 6 px
    // further from it (see [`KNOWN`]), and the clip comes on the fifth
    // touch instead, on the same ledge.
    let Some(level) = level11() else { return };
    let rows = watch(&level, "clip-clip-crouch", "c60", 600);
    assert_eq!((rows[443].x, rows[443].state), (4074, state::CROUCH));
    assert_eq!(rows[444].state, state::HIT);
    assert_eq!(
        first_from(&rows, 440, |l| l.physics_type == ptype::FALL),
        Some(447)
    );
    assert_eq!(
        first_from(&rows, 445, |l| l.state == state::STAND),
        Some(466)
    );
    assert_eq!(rows[466].x, 4050);
    let landed = first_from(&rows, 467, |l| l.state != state::FALL);
    assert_eq!(landed, Some(517));
    assert_eq!((rows[517].x, rows[517].y), (3992, 2511));
}

#[test]
fn install_floor_clip_a_blast_pushes_claw_away_from_the_wall() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "clip-clip-dynamite", "c18", 812);
    // The blast on row 130 knocks him right, away from the wall; he never
    // goes under the ledge, and the fish's touches end his life on row
    // 790.
    assert!(hits_on_claw(&rows).contains(&130));
    assert!(rows[131].x > rows[129].x);
    assert!(rows.iter().all(|l| l.y < 1900), "no fall through the floor");
    let lost = (1..rows.len()).find(|&k| rows[k].lives < rows[k - 1].lives || rows[k].health <= 0);
    assert_eq!(lost, Some(790));
}

/// Plays the level from Claw put down at `at` after its first tick,
/// standing still, until he stands: where he stands then.
fn settle(level: &SimLevel, at: (i32, i32)) -> (i32, i32) {
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        seed: SEED,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(at));
    for _ in 0..200 {
        sim.step(TickInput::default());
        let c = sim.player().expect("Claw");
        if c.state == state::STAND {
            return (c.x, c.y);
        }
    }
    panic!("Claw put down at {at:?} never stands");
}

#[test]
fn install_floor_clip_a_claw_put_down_in_a_floor_falls_through_it() {
    let Some(level) = level11() else { return };
    // Put down 1 px below his standing height on the floor at x 4800 (he
    // stands on it at 1359), his feet inside it, he falls through the 584
    // px of Solid under it and on to the floor whose top is 2444; put down
    // 3 px below it on the floor by the laser at x 8780 (2767), he falls
    // to the one at 3340. Put down 20 px above the same floors, he stands
    // on them.
    assert_eq!(settle(&level, (4800, 1360)), (4800, 2383));
    assert_eq!(settle(&level, (8780, 2770)), (8780, 3279));
    assert_eq!(settle(&level, (4800, 1339)), (4800, 1359));
    assert_eq!(settle(&level, (8780, 2747)), (8780, 2767));
}
