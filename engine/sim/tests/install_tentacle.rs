//! The twelfth level's tentacles on your own install, against the
//! original's runs of the passage under the water that leads into
//! Aquatis's room.
//!
//! Each route puts Claw down on the hidden floor of the passage at 36000,
//! after the level's first tick, the way the original's runs put him there
//! (between two ticks, as a warp does but without the warp's own frame),
//! at the height he settles at (3799), waits until the level clock reaches
//! the time the original's run started at, and starts the shared random
//! stream from the run's state, once. Right held from row 15 walks him
//! along the passage under the three tentacles: the first two strike at
//! him and miss (he is far below their reach), and the third grabs him on
//! row 309, lifts him out of the water on row 327 and squeezes him; then,
//! by route:
//!
//! - the walk: no other key; the squeeze takes one health every 12 rows
//!   (14 in all) and lets him go on row 494, when its 3000 ms are over;
//!   he falls onto the toggle peg beside Aquatis and walks on into him;
//! - the mash: Attack pressed every fourth row from row 330; the tenth
//!   press, on row 366, frees him after 4 points of damage.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route at the 18 ms step with the original's speeds, every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events). When the reference run also lists
//! the objects near Claw per step (`obj-log.csv` beside the run, in the
//! folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the three tentacles are compared too: their position,
//! their state (lined up by the numbers the run gives them,
//! [`reference_state`]) and their depth, which decides their order against
//! Claw in each update pass.
//!
//! # While Aquatis is not built
//!
//! The tentacles act only while Aquatis is current, and he draws from the
//! shared random stream on every frame pulse tick from the tick after he
//! is built, so the tentacles' own draws come out right only with his.
//! Until his family is built ([`aquatis::BUILT`]), each play stands in for
//! those two things and nothing else ([`StandIn`]): once his object is
//! woken it is named in the Aquatis link as current and alive, and from
//! the next frame pulse tick on, one draw is made at the end of each
//! pulse tick, where the original's runs show his (after every other draw
//! of the tick). His body is not there: the contact hit Aquatis gives
//! Claw at the end of each route (row 528 of the walk, row 400 of the
//! mash) and everything after it are not compared ([`UNTIL_AQUATIS`]),
//! and the boss health column, which shows his health from his first
//! tick, is a known divergence. Once he is built, the stand-in, the cut
//! and that entry all go and the whole of each route is compared.
//!
//! Divergences that are known, with their causes, are listed in [`known`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace (or, with the reference runs
//! there, a run's `obj-log.csv` or a tentacle in it); `CLAW_L12_ONLY`
//! names one store to play alone, and `CLAW_L12_TRACE` a folder to write
//! the replay's traces to. Nothing from an install or a reference store is
//! kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{EnemyState, aquatis, tentacle};
use claw_sim::object::ObjectId;
use claw_sim::player::state as claw_state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The attack button.
const ATTACK: u32 = 0x2;

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

/// A route of the stores: its name, where Claw is put down (at the height
/// he settles at), how many rows it has and its stretches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The original's game time at the routes' time zero at the 18 ms step.
const T0_MS: u64 = 828;
/// Where the routes put Claw down: the hidden floor of the passage.
const START: (i32, i32) = (36000, 3799);

/// The three tentacles, by their serial numbers in the original's runs
/// and where they are placed, in the order they are built.
const TENTACLES: [(&str, (i32, i32)); 3] = [
    ("1691", (37397, 3608)),
    ("1692", (37523, 3612)),
    ("1693", (37648, 3614)),
];

/// Aquatis, where he is placed.
const AQUATIS_AT: (i32, i32) = (37912, 3600);

const ROUTES: [Route; 2] = [
    Route {
        store: "boss-passage-walk-c18",
        start: START,
        rows: 600,
        spans: &[span(15, 599, RIGHT)],
    },
    Route {
        store: "boss-passage-mash-c18",
        start: START,
        rows: 600,
        spans: &[
            span(15, 599, RIGHT),
            span(330, 331, ATTACK),
            span(334, 335, ATTACK),
            span(338, 339, ATTACK),
            span(342, 343, ATTACK),
            span(346, 347, ATTACK),
            span(350, 351, ATTACK),
            span(354, 355, ATTACK),
            span(358, 359, ATTACK),
            span(362, 363, ATTACK),
            span(366, 367, ATTACK),
        ],
    },
];

/// While Aquatis is not built: the first row of each store that is not
/// compared, the row on which his body's contact hit lands on Claw (see
/// the module documentation).
const UNTIL_AQUATIS: [(&str, usize); 2] = [
    ("boss-passage-walk-c18", 528),
    ("boss-passage-mash-c18", 400),
];

/// The first row of `store` not compared: none once Aquatis is built.
fn compared_until(store: &str) -> Option<usize> {
    if aquatis::BUILT {
        return None;
    }
    UNTIL_AQUATIS
        .iter()
        .find(|(s, _)| *s == store)
        .map(|&(_, row)| row)
}

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store; a column not listed must stay within it, and one
/// listed must not leave it earlier. Every route follows its store
/// exactly, the random stream included. While Aquatis is not built, the
/// boss health column, which shows his health from his first tick (row
/// 154), is empty in the replay.
fn known(store: &str) -> Vec<(&'static str, u64)> {
    let _ = store;
    if aquatis::BUILT {
        Vec::new()
    } else {
        vec![("boss_health", 155 * TICK)]
    }
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

/// The rows of a store's own copy of a route, (held, pressed) by row; a
/// directive line fails the test (these routes have none).
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

/// The object of logic `logic` placed at `at`.
fn placed(sim: &Sim, logic: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} placed at {at:?}"))
}

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, r: &Route) -> Sim {
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        seed: SEED,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    sim.step(TickInput::default());
    sim.place_between_ticks(r.start.0, r.start.1);
    while sim.tick() * TICK < T0_MS {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(SEED));
    sim
}

/// What of Aquatis the tentacles and the random stream see, while his
/// family is not built (see the module documentation): his object named
/// as current and alive from the end of the tick it is woken, and one
/// draw at the end of each frame pulse tick after that one.
struct StandIn {
    aquatis: ObjectId,
    woken: Option<usize>,
}

impl StandIn {
    fn new(sim: &Sim) -> StandIn {
        StandIn {
            aquatis: placed(sim, aquatis::NAMES[0], AQUATIS_AT),
            woken: None,
        }
    }

    /// After row `k` has been played.
    fn after(&mut self, sim: &mut Sim, k: usize) {
        if aquatis::BUILT {
            return;
        }
        match self.woken {
            None => {
                if sim
                    .objects()
                    .get(self.aquatis)
                    .is_some_and(|o| o.is_listed())
                {
                    self.woken = Some(k);
                    let id = self.aquatis;
                    sim.with_world(|w| {
                        let link = w.aquatis_link_mut();
                        link.aquatis = Some(id);
                        link.alive = true;
                    });
                }
            }
            Some(_) if sim.pulse() => {
                sim.with_world(|w| {
                    w.rng.below(1001);
                });
            }
            Some(_) => {}
        }
    }
}

/// The state a run's number for a tentacle's state stands for, in this
/// build's numbers.
fn reference_state(n: i32) -> Option<EnemyState> {
    Some(match n {
        2000 => EnemyState::Idle,
        2003 => EnemyState::Strike,
        2004 => EnemyState::Dead,
        2093 => EnemyState::SuperFrenzy,
        2097 => EnemyState::Respawn,
        2098 => EnemyState::StartGrab,
        2099 => EnemyState::Squeeze,
        _ => return None,
    })
}

/// A tentacle at one step of a run or of the replay: position, state (the
/// run's own numbers lined up through [`reference_state`]) and depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TentAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
    z: i32,
}

/// The steps of the tentacles in a run's `obj-log.csv`, by tentacle:
/// (step, the tentacle there).
fn reference_tentacles(path: &Path) -> Option<Vec<Vec<(usize, TentAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, z) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("z")?,
    );
    let mut out = vec![Vec::new(); TENTACLES.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = TENTACLES.iter().position(|t| f.get(serial) == Some(&t.0)) else {
            continue;
        };
        out[i].push((
            f[tick].parse().ok()?,
            TentAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(int(f[st])?),
                z: int(f[z])?,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's tentacle leaves the
/// run's, by column (`<serial>_x`, `_y`, `_state`, `_z`), over the rows
/// before `until`; a step on which the replay has no such tentacle in the
/// update pass (asleep, or not built yet) counts for all of them.
fn compare_tentacle(
    serial: &str,
    played: &[Option<TentAt>],
    reference: &[(usize, TentAt)],
    until: usize,
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: &str, k: usize| {
        let name = format!("{serial}_{name}");
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    let mut compared = 0;
    for &(k, want) in reference.iter().filter(|(k, _)| *k < until) {
        let Some(got) = played.get(k) else { continue };
        compared += 1;
        let Some(got) = got else {
            for name in ["x", "y", "state", "z"] {
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
        if got.z != want.z {
            note("z", k);
        }
    }
    println!("{serial}: {compared} steps compared");
    assert!(compared > 0, "{serial}: no step compared");
    firsts
}

/// A tentacle of the replay, while it is in the update pass (the original's
/// runs list only those).
fn tent_at(sim: &Sim, id: ObjectId) -> Option<TentAt> {
    sim.objects()
        .get(id)
        .filter(|o| o.is_listed())
        .filter(|o| o.state != claw_sim::object::engine_state::INIT)
        .map(|o| TentAt {
            x: o.x,
            y: o.y,
            state: EnemyState::from_id(o.state),
            z: o.z,
        })
}

/// One play of a route: its trace, the reference trace, the tentacles on
/// each row, and Claw's health and state on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    tentacles: Vec<Vec<Option<TentAt>>>,
}

/// Plays route `r` from its store's time zero for as long as the store's
/// first trace runs. A store without the trace or its meta fails the
/// test.
fn play(level: &SimLevel, root: &Path, r: &Route) -> Played {
    let dir = root.join("level12").join(r.store);
    let path = dir.join("trace-c18-r1.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let meta = std::fs::read_to_string(dir.join("meta.toml"))
        .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
    let t0: u64 = meta_value(&meta, "c18", "t0_ms")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    let seed: u32 = meta_value(&meta, "c18", "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{}: no random state in its meta", r.store));
    assert_eq!(t0, T0_MS, "{}: time zero", r.store);
    assert_eq!(seed, SEED, "{}: random state", r.store);
    let mut sim = at_time_zero(level, r);
    let ids: Vec<ObjectId> = TENTACLES
        .iter()
        .map(|t| placed(&sim, tentacle::NAMES[0], t.1))
        .collect();
    let mut stand_in = StandIn::new(&sim);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut tentacles = vec![Vec::new(); ids.len()];
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        stand_in.after(&mut sim, k);
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            tentacles[i].push(tent_at(&sim, id));
        }
        k += 1;
    }
    Played {
        rows,
        reference,
        tentacles,
    }
}

/// Compares a play of route `r` with the reference trace exactly, and its
/// tentacles with the run's (when the reference runs are there), over the
/// rows compared; returns the divergences not known.
fn new_divergences(root: &Path, r: &Route, p: &Played) -> Vec<String> {
    let store = r.store;
    let until = compared_until(store).unwrap_or(usize::MAX);
    if until != usize::MAX {
        println!("{store}: rows from {until} not compared until Aquatis is built");
    }
    let n = until.min(p.reference.len()).min(p.rows.len());
    let reference = &p.reference[..until.min(p.reference.len())];
    let rows = &p.rows[..n];
    let cmp = trace::compare(rows, reference, &Tolerances::exact());
    print!("{}", trace::report(rows, reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != reference.len() {
        out.push(format!(
            "{store}: {} of {} rows compared",
            cmp.rows_compared,
            reference.len()
        ));
    }
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.to_string(), t)))
        .collect();
    if let Some(runs) = runs_root(root) {
        let meta = std::fs::read_to_string(root.join("level12").join(store).join("meta.toml"))
            .expect("meta");
        let id = run_id(&meta, "c18").unwrap_or_else(|| panic!("{store}: no run id"));
        let log = runs.join(&id).join("obj-log.csv");
        let reference = reference_tentacles(&log)
            .unwrap_or_else(|| panic!("{store}: {} lacks a tentacle", log.display()));
        for (i, t) in TENTACLES.iter().enumerate() {
            firsts.extend(compare_tentacle(t.0, &p.tentacles[i], &reference[i], until));
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
        assert_eq!(meta_value(&meta, "", "diagnostic"), None, "{}", r.store);
        assert!(meta_value(&meta, "c18", "t0_ms").is_some(), "{}", r.store);
        let teleport =
            meta_value(&meta, "c18", "teleport").unwrap_or_else(|| panic!("{}: no start", r.store));
        let x: i32 = teleport.split(',').next().unwrap().parse().unwrap();
        assert_eq!(x, r.start.0, "{}: start", r.store);
        let trace = dir.join("trace-c18-r1.csv");
        assert!(trace.is_file(), "{}", trace.display());
    }
}

#[test]
fn install_tentacle_routes_follow_the_reference_traces() {
    let Some(level) = level12() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L12_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        println!("== level12/{} c18", r.store);
        let p = play(&level, &root, r);
        if let Some(out) = std::env::var_os("CLAW_L12_TRACE") {
            let out = PathBuf::from(out).join(format!("{}-c18.csv", r.store));
            std::fs::write(&out, trace::to_csv(&p.rows)).expect("the trace is written");
        }
        failures.extend(new_divergences(&root, r, &p));
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw and the third tentacle.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    health: i32,
    third: Option<TentAt>,
}

/// Plays `rows` rows of route `r` from its time zero with the original's
/// random state there (and the stand-in for Aquatis while he is not
/// built).
fn watch(level: &SimLevel, r: &Route, rows: usize) -> Vec<Look> {
    let mut sim = at_time_zero(level, r);
    let third = placed(&sim, tentacle::NAMES[0], TENTACLES[2].1);
    let mut stand_in = StandIn::new(&sim);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        stand_in.after(&mut sim, k);
        let c = sim.player().expect("Claw");
        looks.push(Look {
            x: c.x,
            y: c.y,
            state: c.state,
            health: sim.claw().stats.health,
            third: tent_at(&sim, third),
        });
    }
    looks
}

fn third_state(l: &Look) -> Option<EnemyState> {
    l.third.and_then(|t| t.state)
}

#[test]
fn install_tentacle_the_third_grabs_lifts_and_squeezes_claw_for_3000_ms() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, &ROUTES[0], 520);
    // The grab on row 309: Claw held where he walked, under the water.
    let grab = rows
        .iter()
        .position(|l| third_state(l) == Some(EnemyState::StartGrab))
        .expect("grabbed");
    assert_eq!(grab, 309);
    assert_eq!(rows[grab].state, claw_state::BEARHUG);
    assert_eq!((rows[grab].x, rows[grab].y), (rows[grab - 1].x, 3799));
    // 18 rows of the grab, then he is put at (x + 50, y - 70) and squeezed.
    assert_eq!(third_state(&rows[grab + 17]), Some(EnemyState::StartGrab));
    assert_eq!(third_state(&rows[grab + 18]), Some(EnemyState::Squeeze));
    assert_eq!(
        (rows[grab + 18].x, rows[grab + 18].y),
        (37648 + 50, 3614 - 70)
    );
    // One health on the squeeze's first row and every 12 rows after, 14
    // in all; let go on row 494, 167 rows after the squeeze started.
    let hits: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect();
    let squeezed: Vec<usize> = hits.iter().copied().filter(|&k| k < 500).collect();
    assert_eq!(squeezed, (0..14).map(|i| 328 + 12 * i).collect::<Vec<_>>());
    assert!(
        squeezed
            .iter()
            .all(|&k| rows[k - 1].health - rows[k].health == 1)
    );
    assert_eq!(rows[493].state, claw_state::BEARHUG);
    assert_ne!(rows[494].state, claw_state::BEARHUG);
    assert_eq!(third_state(&rows[494]), Some(EnemyState::Idle));
    assert_eq!(rows[494].health, 86);
}

#[test]
fn install_tentacle_ten_presses_free_claw_on_the_tenth() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, &ROUTES[1], 400);
    // Pressed on rows 330, 334, ... 366: held to the ninth, let go on the
    // tenth after 4 points of damage.
    assert_eq!(rows[365].state, claw_state::BEARHUG);
    assert_eq!(third_state(&rows[365]), Some(EnemyState::Squeeze));
    assert_ne!(rows[366].state, claw_state::BEARHUG);
    assert_eq!(third_state(&rows[366]), Some(EnemyState::Idle));
    assert_eq!(rows[366].health, 96);
}

#[test]
fn install_tentacle_its_depth_puts_the_grabber_before_claw_only_to_place_him() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, &ROUTES[0], 340);
    let z = |k: usize| rows[k].third.map(|t| t.z);
    // Behind Claw (so before him in the update pass) while he walks the
    // passage below it, the placement included; in front of him (after
    // him) from the placement on.
    assert_eq!(z(326), Some(enemy_depth_behind()));
    assert_eq!(z(327), Some(enemy_depth_in_front()));
    assert_eq!(z(339), Some(enemy_depth_in_front()));
}

fn enemy_depth_behind() -> i32 {
    claw_sim::logic::enemy::motion::DEPTH_BEHIND
}

fn enemy_depth_in_front() -> i32 {
    claw_sim::logic::enemy::motion::DEPTH_IN_FRONT
}
