//! Lifting a hiding hermit crab on the seventh level of your own install,
//! against the original's runs of the same short routes.
//!
//! Each route puts Claw down at the store's start point after the level's
//! first tick, between two ticks as a warp does but without the warp's own
//! frame, lets him settle, waits until the level clock reaches the store's
//! time zero and starts the shared random stream from the store's state
//! there, once (the store's time zero rule, `teleport`; a store with
//! another rule fails). It is then played tick by tick and compared with
//! every reference trace of the clock: at the 18 ms step with the
//! original's speeds Claw's position, state and health exactly (the other
//! columns are reported), at 60 frames per second with the default tuning
//! within the feel tolerances. The routes:
//!
//! - `crab-lift`: Claw lands 77 px left of the crab at 12337, hiding in
//!   its shell, walks up to it, lifts it, carries it left and throws it:
//!   it flies over the water's edge, falls into the water and is gone;
//! - `crab-lift-edge`: the same walk, and Lift pressed on the step on
//!   which the crab's hide ends (its regroup step): it is still lifted;
//! - `crab-lift-late`: Lift pressed one step later: it is not, and the
//!   crab crouches and snips at Claw;
//! - `crab-lift-pirate`: Claw walks into the nest by the level's red-tail
//!   pirates, then on to the third crab it hatched, hiding, lifts it and
//!   throws it at the pirate walking at him: it flies over the pirate,
//!   lands behind him and regroups, and the pirate is not hurt.
//!
//! A crab starts with a second range box and a side the original game
//! does not fix (they differ from crab to crab and from run to run); each
//! store's crabs get the values its runs show ([`leftovers`]) as soon as
//! they are built.
//!
//! When the reference runs also list the objects per step (`obj-log.csv`
//! beside each run, in the folder named by `CLAW_REFERENCE_RUNS`, by
//! default `rt` next to the reference store), the crabs' positions,
//! states and lift flags are compared too, exactly at the 18 ms step, and
//! the pins below are checked on the original's crab as well as on the
//! game's.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; the store's
//! copy of each must agree. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons; each test skips itself when what it needs is
//! unset, and with `CLAW_REFERENCE` set a listed store without its meta,
//! its route or one of its traces fails. `CLAW_L7_ONLY` names one store
//! to play alone. Nothing from an install or a reference store is kept in
//! the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, crab, redtail_pirate};
use claw_sim::object::{Object, ObjectId, flags, special, types};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const LIFT: u32 = Buttons::LIFT.0;

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

/// A route of the stores: its name, where Claw is put, how many rows it
/// has, its stretches, and the clocks it is compared at.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
}

/// The original's game time at every route's time zero at the 18 ms
/// step, ms, and its random state there: the pins below play from them
/// when no reference store is at hand, and the stores must agree.
const T0_MS: u64 = 828;
const SEED: u32 = 111_567;

const BOTH: &[&str] = &["c18", "c60"];
const C18: &[&str] = &["c18"];

/// 77 px left of the crab at 12337.
const LEFT_OF_CRAB: (i32, i32) = (12260, 7300);
/// Left of the nest at (15468, 7390).
const BY_THE_NEST: (i32, i32) = (15380, 7300);

/// The walk up to the crab at 12337.
const WALK: Span = span(20, 27, RIGHT);
/// The lift of `crab-lift`, the carry to the left and the throw.
const LIFT_ROW: usize = 40;
const CARRY: Span = span(75, 104, LEFT);
const THROW_ROW: usize = 115;
/// The step on which the crab's second hide ends in `crab-lift-edge` and
/// `crab-lift-late` (its regroup step).
const HIDE_END_ROW: usize = 204;
/// The walk into the nest's box, the walk on to its third crab, the lift
/// and the throw of `crab-lift-pirate`.
const NEST_WALK: Span = span(20, 29, RIGHT);
const PIRATE_WALK: Span = span(135, 156, RIGHT);
const PIRATE_LIFT_ROW: usize = 159;
const PIRATE_THROW_ROW: usize = 190;

/// A two-row tap of the lift key on `row`.
const fn lift_tap(row: usize) -> Span {
    span(row, row + 1, LIFT)
}

const ROUTES: [Route; 4] = [
    Route {
        store: "crab-lift",
        start: LEFT_OF_CRAB,
        rows: 300,
        spans: &[WALK, lift_tap(LIFT_ROW), CARRY, lift_tap(THROW_ROW)],
        clocks: BOTH,
    },
    Route {
        store: "crab-lift-edge",
        start: LEFT_OF_CRAB,
        rows: 400,
        spans: &[WALK, lift_tap(HIDE_END_ROW)],
        clocks: C18,
    },
    Route {
        store: "crab-lift-late",
        start: LEFT_OF_CRAB,
        rows: 400,
        spans: &[WALK, lift_tap(HIDE_END_ROW + 1)],
        clocks: C18,
    },
    Route {
        store: "crab-lift-pirate",
        start: BY_THE_NEST,
        rows: 600,
        spans: &[
            NEST_WALK,
            PIRATE_WALK,
            lift_tap(PIRATE_LIFT_ROW),
            lift_tap(PIRATE_THROW_ROW),
        ],
        clocks: C18,
    },
];

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

/// Which crab of a store: a placed one by its record, or the n-th a nest
/// hatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Record(i32),
    Hatched(usize),
}

/// The leftovers of each crab of a store at a clock, as its runs show
/// them; a crab not listed starts with the plain default. The nest of
/// `crab-lift-pirate` is the nest of the hatching routes, whose crabs
/// start as they do there.
fn leftovers(store: &str, clock: &str) -> Vec<(Which, crab::Leftovers)> {
    let l = |side, box1| crab::Leftovers {
        side,
        box1,
        stationary: false,
    };
    match (store, clock) {
        ("crab-lift-pirate", "c18") => vec![
            (Which::Hatched(0), l(382, (382, 382))),
            (Which::Hatched(1), l(0, (0, 0))),
            (Which::Hatched(2), l(0, (0, 0))),
        ],
        _ => vec![],
    }
}

/// Gives the crabs built since the last call their leftovers.
fn apply_leftovers(sim: &mut Sim, list: &[(Which, crab::Leftovers)], done: &mut Vec<ObjectId>) {
    let crabs: Vec<(ObjectId, i32)> = sim
        .objects()
        .all()
        .filter(|(id, o)| crab::fields_of(o).is_some() && !done.contains(id))
        .map(|(id, o)| (id, o.id))
        .collect();
    if crabs.is_empty() {
        return;
    }
    let hatched = hatched(sim);
    for (id, record) in crabs {
        done.push(id);
        let which = match hatched.iter().position(|&h| h == id) {
            Some(n) => Which::Hatched(n),
            None => Which::Record(record),
        };
        if let Some(&(_, l)) = list.iter().find(|(w, _)| *w == which) {
            sim.with_world(|w| crab::set_leftovers(w, id, l));
        }
    }
}

fn hatched(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter_map(|(_, o)| crab::nest::data_of(o))
        .flat_map(|d| d.hatchlings.clone())
        .collect()
}

fn level7() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(7)).unwrap();
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

/// The stores to play: all, or the one `CLAW_L7_ONLY` names.
fn selected() -> Vec<&'static Route> {
    let only = std::env::var("CLAW_L7_ONLY").ok();
    ROUTES
        .iter()
        .filter(|r| only.as_deref().is_none_or(|o| o == r.store))
        .collect()
}

/// A store's folder.
fn store_dir(root: &Path, store: &str) -> PathBuf {
    root.join("level7").join(store)
}

// ------------------------------------------------------------ the store

/// A value of a store's `meta.toml`: `key` in the table `[section]`, or
/// at the top when `section` is empty; quotes are kept.
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
            return Some(v.trim().to_string());
        }
    }
    None
}

/// The strings of a list value (`["a", "b"]`).
fn strings(v: &str) -> Vec<String> {
    v.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// What a store's meta says about one clock's runs.
#[derive(Debug)]
struct Meta {
    /// Where Claw is put.
    start: (i32, i32),
    /// The game time at the route's time zero, ms.
    t0_ms: u64,
    /// The random state there.
    seed: u32,
    /// The clock's traces, with the run each came from.
    traces: Vec<(String, Option<String>)>,
}

/// The time zero rule this test plays: Claw put down at the start point
/// after the first tick, left to settle until the time zero.
const TELEPORT_RULE: &str = "teleport";

/// Reads a store's meta for `clock`; fails, naming what is missing, when
/// the meta or one of the values the comparison needs is.
fn meta(dir: &Path, store: &str, clock: &str) -> Meta {
    let path = dir.join("meta.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{store}: the reference store's meta.toml is missing ({}: {e}); record the store, \
             or point CLAW_REFERENCE at a folder that has it",
            path.display()
        )
    });
    let get = |key: &str| {
        meta_value(&text, clock, key)
            .or_else(|| meta_value(&text, "", key))
            .map(|v| v.trim_matches('"').to_string())
    };
    let need = |key: &str| {
        get(key).unwrap_or_else(|| panic!("{store}: meta.toml has no {key} for {clock}"))
    };
    let rule = need("t0_rule");
    assert_eq!(
        rule, TELEPORT_RULE,
        "{store}: the time zero rule {rule:?} of {clock} is not one this test plays \
         (it plays {TELEPORT_RULE:?})"
    );
    let teleport = need("teleport");
    let start = teleport
        .split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
        .unwrap_or_else(|| panic!("{store}: teleport {teleport:?} is not X,Y"));
    let t0 = need("t0_ms");
    let t0_ms = t0
        .parse()
        .unwrap_or_else(|_| panic!("{store}: t0_ms {t0:?} of {clock} is not a whole number"));
    let seed = need("rng_at_t0");
    let seed = seed
        .parse()
        .unwrap_or_else(|_| panic!("{store}: rng_at_t0 {seed:?} is not a whole number"));
    let names = meta_value(&text, clock, "traces")
        .map(|v| strings(&v))
        .unwrap_or_default();
    assert!(
        !names.is_empty(),
        "{store}: meta.toml lists no {clock} traces"
    );
    let runs = meta_value(&text, clock, "run_ids")
        .map(|v| strings(&v))
        .unwrap_or_default();
    let traces = names
        .into_iter()
        .enumerate()
        .map(|(n, name)| (name, runs.get(n).cloned()))
        .collect();
    Meta {
        start,
        t0_ms,
        seed,
        traces,
    }
}

/// One cell of a route row as a hexadecimal mask, with a message naming
/// the row and the column when it is missing or not a number.
fn mask(store: &str, line_no: usize, line: &str, cells: &[&str], n: usize, what: &str) -> u32 {
    let cell = cells.get(n).map(|c| c.trim()).unwrap_or_else(|| {
        panic!("{store}: route.csv line {line_no} ({line:?}) has no {what} cell")
    });
    let digits = cell
        .strip_prefix("0x")
        .or_else(|| cell.strip_prefix("0X"))
        .unwrap_or(cell);
    u32::from_str_radix(digits, 16).unwrap_or_else(|_| {
        panic!(
            "{store}: route.csv line {line_no} has {what} {cell:?}, which is not a hexadecimal mask"
        )
    })
}

/// A store's copy of its route: held and pressed per row, in row order.
/// Fails with a clear message when the copy is missing, a row's cells
/// are malformed or its rows are not numbered 0, 1, 2 and so on.
fn store_route(dir: &Path, store: &str) -> Vec<(u32, u32)> {
    let path = dir.join("route.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{store}: the reference store's route.csv is missing ({}: {e})",
            path.display()
        )
    });
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line_no = n + 1;
        if !line.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let cells: Vec<&str> = line.split(',').collect();
        let row: usize = cells[0].trim().parse().unwrap_or_else(|_| {
            panic!(
                "{store}: route.csv line {line_no} has row {:?}, which is not a whole number",
                cells[0]
            )
        });
        assert_eq!(
            row,
            rows.len(),
            "{store}: route.csv line {line_no} is row {row}, expected row {}",
            rows.len()
        );
        let held = mask(store, line_no, line, &cells, 1, "held");
        let pressed = mask(store, line_no, line, &cells, 2, "pressed");
        rows.push((held, pressed));
    }
    assert!(!rows.is_empty(), "{store}: route.csv has no rows");
    rows
}

/// A store's trace; fails when it is missing or unreadable.
fn store_trace(dir: &Path, store: &str, name: &str) -> Vec<TraceRow> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{store}: the trace {name} its meta lists is missing ({}: {e})",
            path.display()
        )
    });
    trace::parse(&text).unwrap_or_else(|e| panic!("{store}: trace {name}: {e}"))
}

// ------------------------------------------------------------ playing

/// Draws the original makes at the level's start, before the routes'
/// warp, that this game does not make from the same stream: the level's
/// timed sounds take their first timers from it (this game draws them
/// while the level loads, before its time zero), and the red-tail pirate
/// by the level's start goes idle and idles twice (four draws). Built,
/// the pirate makes two of them itself on the first step (its first go
/// idle comes while the level loads, before the stream starts), so two
/// are added; not built, all four.
fn level_start_draws() -> u32 {
    const TIMED_SOUNDS: u32 = 3;
    const PIRATE_BY_THE_START: u32 = 4;
    const PIRATE_OWN_DRAWS: u32 = 2;
    TIMED_SOUNDS
        + if redtail_pirate::BUILT {
            PIRATE_BY_THE_START - PIRATE_OWN_DRAWS
        } else {
            PIRATE_BY_THE_START
        }
}

/// Where the red-tail pirate by the level's start stands.
const PIRATE_BY_THE_START_X: i32 = 1924;

/// The game at a route's time zero by the store's rule (see the module
/// documentation).
fn at_time_zero(level: &SimLevel, clock: &str, m: &Meta) -> Sim {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        seed: m.seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    let pirate = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "RedTailPirate")
        .min_by_key(|(_, o)| (o.x - PIRATE_BY_THE_START_X).abs())
        .map(|(id, _)| id);
    sim.step(TickInput::default());
    let start = m.start;
    sim.with_world(|w| {
        // The original's runs put Claw down before the leave pass of the
        // step, so the pirate by the level's start is asleep before the
        // next step and never runs after the warp; this game's warp comes
        // after that pass, so the pirate is put to sleep here.
        if redtail_pirate::BUILT
            && let Some(id) = pirate
        {
            w.objects.unlink(id);
        }
        for _ in 0..level_start_draws() {
            w.rng.next_u15();
        }
    });
    sim.place_between_ticks(start.0, start.1);
    while sim.tick() * TICK < m.t0_ms {
        sim.step(TickInput::default());
    }
    let seed = m.seed;
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// A crab (or the pirate) on one step, as the game or a reference run
/// shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
    /// Kept active everywhere.
    always_active: bool,
    /// Out of reach of hits.
    safe: bool,
    /// A thrown object rather than an enemy.
    thrown: bool,
    /// Liftable.
    liftable: bool,
    vx: f64,
    vy: f64,
    health: i32,
}

impl Look {
    fn of(o: &Object) -> Look {
        let d = enemy::data_of(o);
        Look {
            x: o.x,
            y: o.y,
            state: EnemyState::from_id(o.state),
            always_active: o.flags & flags::ALWAYS_ACTIVE != 0,
            safe: o.flags & flags::SAFE != 0,
            thrown: o.type_flags == types::THROWN,
            liftable: o.special_flags & special::LIFTABLE != 0,
            vx: d.map_or(0.0, |d| d.vx),
            vy: d.map_or(0.0, |d| d.vy),
            health: o.health,
        }
    }
}

/// One route played in the game.
struct Played {
    rows: Vec<TraceRow>,
    /// The crabs per step, in their order of first appearance (by x
    /// among those already built, by hatching order among a nest's).
    crabs: Vec<Vec<Option<Look>>>,
    /// The red-tail pirates per step, by x when first listed.
    pirates: Vec<Vec<Option<Look>>>,
    /// What Claw carries per step.
    lifted: Vec<Option<ObjectId>>,
    /// The crabs' ids, in the order of `crabs`.
    crab_ids: Vec<ObjectId>,
    /// Whether Claw faces right per step.
    facing_right: Vec<bool>,
}

/// Adds the objects `f` accepts that are listed and not yet in `order`,
/// by the key `key` gives.
fn newcomers(
    sim: &Sim,
    order: &mut Vec<ObjectId>,
    looks: &mut Vec<Vec<Option<Look>>>,
    k: usize,
    f: impl Fn(&Object) -> bool,
    key: impl Fn(ObjectId, &Object) -> (usize, i32),
) {
    let mut new: Vec<((usize, i32), ObjectId)> = sim
        .objects()
        .all()
        .filter(|(id, o)| f(o) && o.is_listed() && !order.contains(id))
        .map(|(id, o)| (key(id, o), id))
        .collect();
    new.sort();
    for (_, id) in new {
        order.push(id);
        looks.push(vec![None; k]);
    }
}

fn is_crab(o: &Object) -> bool {
    crab::fields_of(o).is_some()
}

fn is_pirate(o: &Object) -> bool {
    &*o.logic == "RedTailPirate" && enemy::data_of(o).is_some()
}

fn look(sim: &Sim, id: ObjectId) -> Option<Look> {
    sim.objects()
        .get(id)
        .filter(|o| o.is_listed())
        .map(Look::of)
}

/// Plays route `r` from its store's time zero at `clock` for `rows` rows.
fn play(level: &SimLevel, r: &Route, clock: &str, m: &Meta, rows: usize) -> Played {
    let mut sim = at_time_zero(level, clock, m);
    let list = leftovers(r.store, clock);
    let mut done = Vec::new();
    // Crabs built before time zero already ran: they keep their start.
    apply_leftovers(&mut sim, &list, &mut done);
    let ticks = route_ticks(r);
    let mut crabs: Vec<ObjectId> = Vec::new();
    let mut pirates: Vec<ObjectId> = Vec::new();
    let mut out = Played {
        rows: Vec::with_capacity(rows),
        crabs: Vec::new(),
        pirates: Vec::new(),
        lifted: Vec::with_capacity(rows),
        crab_ids: Vec::new(),
        facing_right: Vec::with_capacity(rows),
    };
    let crab_key = |sim: &Sim| {
        let hatched = hatched(sim);
        move |id: ObjectId, o: &Object| {
            let n = hatched.iter().position(|&h| h == id).unwrap_or(usize::MAX);
            (n, o.x)
        }
    };
    let by_x = |_: ObjectId, o: &Object| (0, o.x);
    newcomers(&sim, &mut crabs, &mut out.crabs, 0, is_crab, crab_key(&sim));
    newcomers(&sim, &mut pirates, &mut out.pirates, 0, is_pirate, by_x);
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        apply_leftovers(&mut sim, &list, &mut done);
        out.rows.push(TraceRow::capture(&sim, k as u64, "script"));
        newcomers(&sim, &mut crabs, &mut out.crabs, k, is_crab, crab_key(&sim));
        newcomers(&sim, &mut pirates, &mut out.pirates, k, is_pirate, by_x);
        for (n, &id) in crabs.iter().enumerate() {
            out.crabs[n].push(look(&sim, id));
        }
        for (n, &id) in pirates.iter().enumerate() {
            out.pirates[n].push(look(&sim, id));
        }
        out.lifted.push(sim.claw().special.lifted);
        out.facing_right.push(sim.claw().facing_right);
    }
    out.crab_ids = crabs;
    out
}

// ------------------------------------------------------------ the runs

/// The enemy state a reference run's state number stands for, among
/// those the crabs and the pirates go through here; `None` for another.
fn state_numbered(n: i32) -> Option<EnemyState> {
    Some(match n {
        2000 => EnemyState::Idle,
        2003 => EnemyState::Strike,
        2004 => EnemyState::Dead,
        2009 => EnemyState::Lifted,
        2010 => EnemyState::Thrown,
        2012 => EnemyState::Dropped,
        2013 => EnemyState::MoveBack,
        2017 => EnemyState::Flailing,
        2018 => EnemyState::Home,
        2021 => EnemyState::Advance,
        2022 => EnemyState::FastAdvance,
        2060 => EnemyState::Regroup,
        2061 => EnemyState::Hide,
        2062 => EnemyState::Defense,
        2063 => EnemyState::Attack,
        2064 => EnemyState::MovingAttack,
        2065 => EnemyState::Snip,
        2066 => EnemyState::Tumble,
        2067 => EnemyState::Nudge,
        2068 => EnemyState::Launch,
        2073 => EnemyState::Dead,
        _ => return None,
    })
}

/// The objects of logic `logic` in a reference run per step, in their
/// order of first appearance (by list position within a step); `None`
/// when not listed on that step. `None` when the run has no object list.
fn reference_objects(run: &Path, logic: &str, rows: usize) -> Option<Vec<Vec<Option<Look>>>> {
    let text = std::fs::read_to_string(run.join("obj-log.csv")).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("{}: obj-log.csv has no {name} column", run.display()))
    };
    let (tick, serial, name, st, x, y) = (
        col("tick"),
        col("serial"),
        col("logic_name"),
        col("state"),
        col("x"),
        col("y"),
    );
    let (fl, ty, sp, vx, vy, health) = (
        col("flags"),
        col("type_flags"),
        col("special_flags"),
        col("g_vx"),
        col("g_vy"),
        col("health"),
    );
    let mut order: Vec<String> = Vec::new();
    let mut out: Vec<Vec<Option<Look>>> = Vec::new();
    for (n, l) in lines.enumerate() {
        let c: Vec<&str> = l.split(',').collect();
        if c.get(name) != Some(&logic) {
            continue;
        }
        let bad = |what: &str| -> ! {
            panic!(
                "{}: obj-log.csv line {} has a malformed {what} cell: {l:?}",
                run.display(),
                n + 2
            )
        };
        let num = |i: usize, what: &str| -> i32 {
            c.get(i)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or_else(|| bad(what))
        };
        let hex = |i: usize, what: &str| -> u32 {
            c.get(i)
                .map(|v| v.trim().trim_start_matches("0x"))
                .and_then(|v| u32::from_str_radix(v, 16).ok())
                .unwrap_or_else(|| bad(what))
        };
        let real =
            |i: usize| -> f64 { c.get(i).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0) };
        let t = num(tick, "tick");
        let s = c.get(serial).copied().unwrap_or_else(|| bad("serial"));
        let k = match order.iter().position(|o| o == s) {
            Some(k) => k,
            None => {
                order.push(s.to_string());
                out.push(vec![None; rows]);
                order.len() - 1
            }
        };
        let f = hex(fl, "flags");
        let look = Look {
            x: num(x, "x"),
            y: num(y, "y"),
            state: state_numbered(num(st, "state")),
            always_active: f & flags::ALWAYS_ACTIVE != 0,
            safe: f & flags::SAFE != 0,
            thrown: hex(ty, "type_flags") == types::THROWN,
            liftable: hex(sp, "special_flags") & special::LIFTABLE != 0,
            vx: real(vx),
            vy: real(vy),
            health: num(health, "health"),
        };
        if let Some(slot) = usize::try_from(t).ok().and_then(|t| out[k].get_mut(t)) {
            *slot = Some(look);
        }
    }
    Some(out)
}

/// The crabs and the pirates of a reference run.
struct TheirRun {
    crabs: Vec<Vec<Option<Look>>>,
    pirates: Vec<Vec<Option<Look>>>,
}

fn their_run(runs: Option<&PathBuf>, run: Option<&String>, rows: usize) -> Option<TheirRun> {
    let dir = runs?.join(run?);
    Some(TheirRun {
        crabs: reference_objects(&dir, "HermitCrab", rows)?,
        pirates: reference_objects(&dir, "RedTailPirate", rows)?,
    })
}

/// The spans of a crab's states, as "Name@step".
fn spans(looks: &[Option<Look>]) -> String {
    let mut out = Vec::new();
    let mut last: Option<Option<EnemyState>> = None;
    for (k, c) in looks.iter().enumerate() {
        let s = c.and_then(|c| c.state);
        if last != Some(s) {
            out.push(format!("{}@{k}", s.map_or("-", |s| s.name())));
            last = Some(s);
        }
    }
    out.join(" ")
}

/// The first step (as reference ms) on which each crab's position,
/// state, lift flags or presence leaves the reference run's, by column.
fn compare_crabs(
    ours: &[Vec<Option<Look>>],
    theirs: &[Vec<Option<Look>>],
) -> BTreeMap<String, u64> {
    let mut first = BTreeMap::new();
    for n in 0..ours.len().max(theirs.len()) {
        let none = Vec::new();
        let (a, b) = (ours.get(n).unwrap_or(&none), theirs.get(n).unwrap_or(&none));
        for k in 0..a.len().max(b.len()) {
            let t = (k as u64 + 1) * TICK;
            let (m, r) = (a.get(k).copied().flatten(), b.get(k).copied().flatten());
            let mut note = |c: &str| {
                first.entry(format!("crab{n}_{c}")).or_insert(t);
            };
            match (m, r) {
                (Some(m), Some(r)) => {
                    if m.x != r.x {
                        note("x");
                    }
                    if m.y != r.y {
                        note("y");
                    }
                    if m.state != r.state {
                        note("state");
                    }
                    if (m.always_active, m.safe, m.thrown) != (r.always_active, r.safe, r.thrown) {
                        note("flags");
                    }
                    if m.liftable != r.liftable {
                        note("liftable");
                    }
                }
                (None, None) => {}
                _ => note("listed"),
            }
        }
    }
    first
}

/// A store, a clock, and the first divergence (ms) of each column known
/// there; a gated column not listed must stay within its tolerance (exact
/// at c18), and one listed must not leave it earlier. What they come from:
///
/// - c18, `crab-lift-pirate`: the bomber's bomb kills the nest's first
///   two crabs on the original's step, standing, and their bodies fly
///   off 1 px lower on some steps: the original keeps a fraction of a
///   pixel in the height of a standing enemy that this game does not
///   (from 2070 ms).
/// - c18, `crab-lift-pirate`: the thrown crab comes back down through its
///   release height on a whole pixel, which this game's sum of the
///   flight's steps reaches exactly and the original's falls a hair
///   short of, so it shows 1 px lower here on that one step (3960 ms);
///   it lands on the original's step and place.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[(
    "crab-lift-pirate",
    "c18",
    &[("crab0_y", 2070), ("crab1_y", 2070), ("crab2_y", 3960)],
)];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

/// Columns gated at c18: Claw's position, state and health, and the
/// crabs' (when the reference runs list them); the others are reported.
fn gated_c18(column: &str) -> bool {
    matches!(column, "x" | "y" | "state_id" | "health") || column.starts_with("crab")
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let rows = store_route(&store_dir(&root, r.store), r.store);
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        for (k, (a, b)) in ours.iter().zip(&rows).enumerate() {
            assert_eq!(
                a, b,
                "{}: row {k}: held and pressed {a:#x?} here, {b:#x?} in the store's copy",
                r.store
            );
        }
    }
}

#[test]
fn the_stores_start_where_the_routes_do() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        for &clock in r.clocks {
            let m = meta(&store_dir(&root, r.store), r.store, clock);
            assert_eq!(m.start, r.start, "{} {clock}: start", r.store);
            if clock == "c18" {
                assert_eq!((m.t0_ms, m.seed), (T0_MS, SEED), "{}: time zero", r.store);
            }
        }
    }
}

#[test]
fn install_level7_crab_lift_routes_follow_the_reference_traces() {
    let Some(level) = level7() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    if runs.is_none() {
        println!("no reference run folder: the crabs' paths are not compared");
    }
    let mut failures = Vec::new();
    for r in selected() {
        let dir = store_dir(&root, r.store);
        store_route(&dir, r.store);
        for &clock in r.clocks {
            let m = meta(&dir, r.store, clock);
            for (name, run) in &m.traces {
                println!("== level7/{} {clock} {name}", r.store);
                let reference = store_trace(&dir, r.store, name);
                let played = play(&level, r, clock, &m, reference.len());
                let tol = if clock == "c18" {
                    Tolerances::exact()
                } else {
                    Tolerances::feel()
                };
                let cmp = trace::compare(&played.rows, &reference, &tol);
                print!("{}", trace::report(&played.rows, &reference, &cmp, 3));
                if cmp.rows_compared != reference.len() {
                    failures.push(format!(
                        "{} {clock} {name}: {} of {} rows compared",
                        r.store,
                        cmp.rows_compared,
                        reference.len()
                    ));
                }
                let mut firsts: BTreeMap<String, u64> = cmp
                    .per_column
                    .iter()
                    .filter_map(|c| c.first_t_ms.map(|t| (c.column.clone(), t)))
                    .collect();
                for (n, c) in played.crabs.iter().enumerate() {
                    println!("crab {n}: {}", spans(c));
                }
                if let Some(theirs) = their_run(runs.as_ref(), run.as_ref(), reference.len()) {
                    for (n, c) in theirs.crabs.iter().enumerate() {
                        println!("their crab {n}: {}", spans(c));
                    }
                    if clock == "c18" {
                        firsts.extend(compare_crabs(&played.crabs, &theirs.crabs));
                    }
                }
                let list = known(r.store, clock);
                for (column, &first) in &firsts {
                    println!("first {} {clock} {column} {first}", r.store);
                    let gated = clock != "c18" || gated_c18(column);
                    if !gated {
                        continue;
                    }
                    match list.iter().find(|(c, _)| c == column) {
                        Some(&(_, at)) if first >= at => {}
                        _ => failures.push(format!(
                            "{} {clock} {name}: {column} at {first} ms",
                            r.store
                        )),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the pins

/// One run of a route seen from the pins: Claw's position and state per
/// step, the lifted crab's and the thrown-at pirate's looks.
struct View {
    who: String,
    claw: Vec<(i32, i32, i32)>,
    crab: Vec<Option<Look>>,
    pirates: Vec<Vec<Option<Look>>>,
}

impl View {
    /// The crab's offset from Claw on step `k`.
    fn offset(&self, k: usize) -> (i32, i32) {
        let c = self.crab[k].unwrap_or_else(|| panic!("{}: no crab on step {k}", self.who));
        (c.x - self.claw[k].0, c.y - self.claw[k].1)
    }

    fn crab_state(&self, k: usize) -> Option<EnemyState> {
        self.crab.get(k).copied().flatten().and_then(|c| c.state)
    }

    /// The first step at or after `from` on which the crab is in `s`.
    fn crab_enters(&self, s: EnemyState, from: usize) -> Option<usize> {
        (from..self.crab.len()).find(|&k| self.crab_state(k) == Some(s))
    }

    /// The first step on which Claw is in state `s`.
    fn claw_enters(&self, s: i32) -> Option<usize> {
        self.claw.iter().position(|c| c.2 == s)
    }
}

/// The index of the crab that is ever lifted in `crabs`, if any.
fn lifted_crab(crabs: &[Vec<Option<Look>>]) -> Option<usize> {
    crabs.iter().position(|c| {
        c.iter()
            .flatten()
            .any(|c| c.state == Some(EnemyState::Lifted))
    })
}

/// The views of a route at `clock`: the game's, and the original's when
/// the reference store and its runs are at hand. The game plays from the
/// store's time zero when there is a store, else from [`T0_MS`].
fn views(level: &SimLevel, store: &str, clock: &str, rows: usize) -> Vec<View> {
    let r = route_named(store);
    let root = std::env::var_os("CLAW_REFERENCE")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    let m = match &root {
        Some(root) => meta(&store_dir(root, store), store, clock),
        None => Meta {
            start: r.start,
            t0_ms: T0_MS,
            seed: SEED,
            traces: Vec::new(),
        },
    };
    let p = play(level, r, clock, &m, rows);
    let n = lifted_crab(&p.crabs).unwrap_or(0);
    let mut out = vec![View {
        who: format!("{store} {clock}, this game"),
        claw: p.rows.iter().map(|t| (t.x, t.y, t.state_id)).collect(),
        crab: p.crabs.get(n).cloned().unwrap_or_else(|| vec![None; rows]),
        pirates: p.pirates,
    }];
    let Some(root) = root else { return out };
    let runs = runs_root(&root);
    let dir = store_dir(&root, store);
    let Some((name, run)) = m.traces.first() else {
        return out;
    };
    let reference = store_trace(&dir, store, name);
    let rows = reference.len();
    if let Some(theirs) = their_run(runs.as_ref(), run.as_ref(), rows) {
        let n = lifted_crab(&theirs.crabs).unwrap_or(0);
        out.push(View {
            who: format!("{store} {clock}, the original"),
            claw: reference.iter().map(|t| (t.x, t.y, t.state_id)).collect(),
            crab: theirs
                .crabs
                .get(n)
                .cloned()
                .unwrap_or_else(|| vec![None; rows]),
            pirates: theirs.pirates,
        });
    }
    out
}

/// The first row at or after `from` on which the route's lift key is up.
fn key_up(r: &Route, from: usize) -> usize {
    let ticks = route_ticks(r);
    (from..ticks.len())
        .find(|&k| ticks[k].held.0 & LIFT == 0)
        .expect("the key comes up")
}

/// Thrown crabs leave at 200 px/s forwards and 600 px/s upwards.
const THROW_SPEED: (f64, f64) = (200.0, 600.0);
/// The gravity a thrown crab falls under, px/s^2.
const THROWN_GRAVITY: f64 = 2000.0;

#[test]
fn install_level7_a_lifted_crab_goes_beside_claw_then_over_his_head_and_flies_into_the_water() {
    let Some(level) = level7() else { return };
    let r = route_named("crab-lift");
    for clock in ["c18", "c60"] {
        for v in views(&level, "crab-lift", clock, 400) {
            let who = &v.who;
            // Lifted on the press step, beside him: 10 px behind (he
            // faces right) and 15 px up.
            let lift = v.claw_enters(state::LIFT).expect("Claw lifts");
            assert_eq!(v.crab_state(lift), Some(EnemyState::Lifted), "{who}");
            assert_eq!(v.offset(lift), (-10, -15), "{who}");
            // Over his head 14 steps later (15 frames at 60 frames per
            // second), 10 px behind and 69 px up.
            let over = v
                .crab_enters(EnemyState::Flailing, lift)
                .expect("over his head");
            let steps = if who.contains("c60") && who.contains("original") {
                15
            } else {
                14
            };
            assert_eq!(over - lift, steps, "{who}");
            assert_eq!(v.offset(over), (-10, -69), "{who}");
            // Carried to the left: the offset mirrors with his facing.
            let throw = v.claw_enters(state::THROW).expect("Claw throws");
            assert!(v.claw[throw].0 < v.claw[lift].0, "{who}: carried left");
            assert_eq!(v.offset(throw - 1), (10, -69), "{who}");
            // Released on the step on which the key comes up (the 18 ms
            // rows), at 200 px/s forwards (left) and 600 px/s up, one
            // step of 2000 px/s^2 already gone.
            let thrown = v.crab_enters(EnemyState::Thrown, throw).expect("thrown");
            let c = v.crab[thrown].unwrap();
            if clock == "c18" {
                assert_eq!((lift, throw), (LIFT_ROW, THROW_ROW), "{who}");
                assert_eq!(thrown, key_up(r, THROW_ROW), "{who}");
                // The movement step's clock counts the step in seconds as
                // a double, so the speed after it is equal to within a
                // rounding.
                let dv = THROWN_GRAVITY * TICK as f64 / 1000.0;
                assert!(
                    (c.vx + THROW_SPEED.0).abs() < 1e-9 && (c.vy + THROW_SPEED.1 - dv).abs() < 1e-9,
                    "{who}: {c:?}"
                );
                let next = v.crab[thrown + 1].unwrap();
                assert!((next.vy - c.vy - dv).abs() < 1e-9, "{who}: {next:?}");
            }
            assert!(c.thrown && c.safe && c.always_active, "{who}: {c:?}");
            // It never lands: it falls into the water and is gone.
            let gone = (thrown..v.crab.len())
                .find(|&k| v.crab[k].is_none())
                .unwrap_or_else(|| panic!("{who}: the crab is never gone"));
            assert!(
                (thrown..gone).all(|k| v.crab_state(k) == Some(EnemyState::Thrown)),
                "{who}"
            );
            assert!(v.crab[gone..].iter().all(Option::is_none), "{who}");
            let last = v.crab[gone - 1].unwrap();
            assert!(last.y > v.claw[gone - 1].1 + 300, "{who}: {last:?}");
            if clock == "c18" {
                assert_eq!(gone, 173, "{who}");
            }
        }
    }
}

#[test]
fn install_level7_the_window_ends_with_the_crabs_regroup_step() {
    let Some(level) = level7() else { return };
    for v in views(&level, "crab-lift-edge", "c18", 400) {
        let who = &v.who;
        // Lift on the regroup step that ends the hide: Claw comes first
        // in the step and still finds the crab hiding.
        assert_eq!(
            v.crab_state(HIDE_END_ROW - 1),
            Some(EnemyState::Hide),
            "{who}"
        );
        assert_eq!(v.claw_enters(state::LIFT), Some(HIDE_END_ROW), "{who}");
        assert_eq!(
            v.crab_state(HIDE_END_ROW),
            Some(EnemyState::Lifted),
            "{who}"
        );
        assert_eq!(v.offset(HIDE_END_ROW), (-10, -15), "{who}");
        assert_eq!(
            v.crab_enters(EnemyState::Flailing, HIDE_END_ROW),
            Some(HIDE_END_ROW + 14),
            "{who}"
        );
    }
    for v in views(&level, "crab-lift-late", "c18", 400) {
        let who = &v.who;
        // One step later the crab has regrouped: no lift, and it
        // crouches and snips at him.
        assert_eq!(
            v.crab_state(HIDE_END_ROW),
            Some(EnemyState::Regroup),
            "{who}"
        );
        assert_eq!(
            v.crab_state(HIDE_END_ROW + 1),
            Some(EnemyState::Defense),
            "{who}"
        );
        assert!(!v.crab[HIDE_END_ROW + 1].unwrap().liftable, "{who}");
        assert_eq!(v.claw_enters(state::LIFT), None, "{who}");
        assert_eq!(v.crab_enters(EnemyState::Lifted, 0), None, "{who}");
        assert!(
            v.crab_enters(EnemyState::Snip, HIDE_END_ROW).is_some(),
            "{who}"
        );
    }
}

#[test]
fn install_level7_a_crab_thrown_at_the_pirate_flies_over_him_and_lands_behind_him() {
    let Some(level) = level7() else { return };
    let r = route_named("crab-lift-pirate");
    for v in views(&level, "crab-lift-pirate", "c18", 712) {
        let who = &v.who;
        // Lifted from the nest's third crab, thrown with a press 31 rows
        // later, released as the key comes up.
        assert_eq!(v.claw_enters(state::LIFT), Some(PIRATE_LIFT_ROW), "{who}");
        assert_eq!(
            v.crab_state(PIRATE_LIFT_ROW),
            Some(EnemyState::Lifted),
            "{who}"
        );
        assert_eq!(v.offset(PIRATE_LIFT_ROW), (-10, -15), "{who}");
        assert_eq!(PIRATE_THROW_ROW - PIRATE_LIFT_ROW, 31);
        assert_eq!(v.claw_enters(state::THROW), Some(PIRATE_THROW_ROW), "{who}");
        let thrown = v
            .crab_enters(EnemyState::Thrown, PIRATE_THROW_ROW)
            .expect("thrown");
        assert_eq!(thrown, key_up(r, PIRATE_THROW_ROW), "{who}");
        // The pirate walking at Claw: the one nearest him at the release.
        let claw_x = v.claw[thrown].0;
        let pirate = v
            .pirates
            .iter()
            .filter(|p| p[thrown].is_some())
            .min_by_key(|p| (p[thrown].unwrap().x - claw_x).abs())
            .expect("the pirate");
        let health = pirate[PIRATE_LIFT_ROW].expect("listed at the lift").health;
        // It lands, a while later, behind him as seen from Claw.
        let landed = v
            .crab_enters(EnemyState::Regroup, thrown)
            .expect("it lands");
        assert!(
            (thrown..landed).all(|k| v.crab_state(k) == Some(EnemyState::Thrown)),
            "{who}"
        );
        let (c, p) = (v.crab[landed].unwrap(), pirate[landed].unwrap());
        let side = (p.x - v.claw[landed].0).signum();
        assert_eq!(side, 1, "{who}: the pirate is in front of Claw");
        assert!(
            (c.x - p.x) * side > 0,
            "{who}: {c:?} lands in front of {p:?}"
        );
        // On the way it passes over his head: where it crosses him it is
        // more than his height above him.
        let over = (thrown..landed)
            .find(|&k| pirate[k].is_some_and(|p| v.crab[k].is_some_and(|c| c.x >= p.x)))
            .expect("it passes him");
        let (c, p) = (v.crab[over].unwrap(), pirate[over].unwrap());
        assert!(c.y < p.y - 50, "{who}: {c:?} over {p:?}");
        // He is not hurt.
        assert!(
            pirate[PIRATE_LIFT_ROW..]
                .iter()
                .flatten()
                .all(|p| p.health == health),
            "{who}"
        );
        // Landed, the crab is an enemy again, no longer kept active
        // everywhere or out of reach, not liftable, and free: it crouches
        // on its next step.
        let c = v.crab[landed].unwrap();
        assert!(
            !c.thrown && !c.safe && !c.always_active && !c.liftable,
            "{who}: {c:?}"
        );
        assert_eq!(v.crab_state(landed + 1), Some(EnemyState::Defense), "{who}");
        if !who.contains("original") {
            assert_eq!((thrown, landed), (192, 230), "{who}");
        }
    }
}
