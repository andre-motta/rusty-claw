//! The seventh level's hermit crabs, crab nests and crab bombs on your own
//! install, against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), lets him settle, waits until the level clock reaches the
//! time the original's run started at, and starts the shared random
//! stream from the run's state, once. It is then played tick by tick and
//! compared with the reference trace of the same route: at the 18 ms step
//! with the original's speeds Claw's position, state and health exactly
//! (the other columns are reported), at 60 frames per second with the
//! default tuning within the feel tolerances. The routes:
//!
//! - `nest-hatch`: Claw walks into the default box of the nest at
//!   (15468, 7390) and stands there: it hatches three crabs, the first a
//!   bomber, whose snips and bombs reach him;
//! - `nest-duck`: the same, then he walks on to x 15504 and crouches,
//!   and the bomber's first bomb passes over him, bounces and goes off on
//!   its fuse;
//! - `crab-fight`: Claw stands between the two crabs of the patrol at
//!   10629..10806 and they fight him;
//! - `crab-swing`: the same with a sword swing every 15 rows;
//! - `crab-nudge`: Claw stands 43 px left of a crab hiding in its shell
//!   and swings at it every 15 rows: each swing knocks it aside;
//! - `crab-clamp`: Claw walks right far left of the crab whose record
//!   lies 4249 px left of its patrol: it is built on its limit, 10629,
//!   and goes to sleep at once.
//!
//! A crab starts with a second range box and a side the original game
//! does not fix (they differ from crab to crab and from run to run); each
//! store's crabs get the values its runs show ([`leftovers`]) as soon as
//! they are built.
//!
//! When the reference runs also list the objects per step (`obj-log.csv`
//! beside each run, in the folder named by `CLAW_REFERENCE_RUNS`, by
//! default `rt` next to the reference store), the crabs' positions and
//! states are compared too, and reported.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; the store's
//! copy of each must agree. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for
//! the comparisons; each test skips itself when what it needs is unset,
//! and with `CLAW_REFERENCE` set a listed store without its trace or its
//! meta fails. `CLAW_L7_ONLY` names one store to play alone. Nothing from
//! an install or a reference store is kept in the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::crab::{bomb, nest};
use claw_sim::logic::enemy::{self, EnemyState, crab, redtail_pirate};
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

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

/// A two-row tap of `held` every `every` rows from `first` to `last`
/// (inclusive).
#[derive(Debug, Clone, Copy)]
struct Taps {
    first: usize,
    last: usize,
    every: usize,
    held: u32,
}

/// A route of the stores: its name, where Claw is put, how many rows it
/// has, its stretches and taps, and the clocks it is compared at.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    taps: Option<Taps>,
    clocks: &'static [&'static str],
    /// The original's game time at the route's time zero at the 18 ms
    /// step, ms.
    t0_ms: u64,
}

const BOTH: &[&str] = &["c18", "c60"];
const C18: &[&str] = &["c18"];

/// Left of the nest at (15468, 7390).
const BY_THE_NEST: (i32, i32) = (15380, 7300);
/// Between the two crabs of the patrol 10629..10806.
const BETWEEN_CRABS: (i32, i32) = (10700, 7300);
/// 43 px left of the crab at 10653.
const LEFT_OF_CRAB: (i32, i32) = (10610, 7300);
/// 825 px left of the record of the crab placed outside its patrol.
const FAR_LEFT: (i32, i32) = (5555, 7400);

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// The nest of the nest routes.
const NEST: (i32, i32) = (15468, 7390);
/// The record of the crab placed outside its patrol, and where it is
/// built.
const CLAMPED_CRAB: i32 = 4720;
const CLAMPED_AT: (i32, i32) = (10629, 7424);

const SWINGS: Taps = Taps {
    first: 30,
    last: 645,
    every: 15,
    held: ATTACK,
};

const ROUTES: [Route; 6] = [
    Route {
        store: "nest-hatch",
        start: BY_THE_NEST,
        rows: 1100,
        spans: &[span(20, 29, RIGHT)],
        taps: None,
        clocks: BOTH,
        t0_ms: 828,
    },
    Route {
        store: "nest-duck",
        start: BY_THE_NEST,
        rows: 1100,
        spans: &[
            span(20, 29, RIGHT),
            span(40, 60, RIGHT),
            span(64, 1099, DOWN),
        ],
        taps: None,
        clocks: BOTH,
        t0_ms: 828,
    },
    Route {
        store: "crab-fight",
        start: BETWEEN_CRABS,
        rows: 1000,
        spans: &[],
        taps: None,
        clocks: BOTH,
        t0_ms: 1152,
    },
    Route {
        store: "crab-swing",
        start: BETWEEN_CRABS,
        rows: 700,
        spans: &[],
        taps: Some(SWINGS),
        clocks: C18,
        t0_ms: 1152,
    },
    Route {
        store: "crab-nudge",
        start: LEFT_OF_CRAB,
        rows: 300,
        spans: &[],
        taps: Some(Taps {
            last: 255,
            ..SWINGS
        }),
        clocks: C18,
        t0_ms: 828,
    },
    Route {
        store: "crab-clamp",
        start: FAR_LEFT,
        rows: 300,
        spans: &[span(20, 29, RIGHT)],
        taps: None,
        clocks: C18,
        t0_ms: 828,
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
    let mut spans: Vec<Span> = r.spans.to_vec();
    if let Some(t) = r.taps {
        spans.extend(
            (t.first..=t.last)
                .step_by(t.every)
                .map(|k| span(k, k + 1, t.held)),
        );
    }
    for s in spans {
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
/// them; a crab not listed starts with the plain default.
fn leftovers(store: &str, clock: &str) -> Vec<(Which, crab::Leftovers)> {
    let l = |side, box1| crab::Leftovers { side, box1 };
    let nest = |second| {
        vec![
            (Which::Hatched(0), l(382, (382, 382))),
            (Which::Hatched(1), second),
            (Which::Hatched(2), l(0, (0, 0))),
        ]
    };
    match (store, clock) {
        ("nest-hatch" | "nest-duck", "c18") => nest(l(0, (0, 0))),
        ("nest-hatch" | "nest-duck", _) => nest(l(382, (382, 382))),
        ("crab-nudge", _) => vec![(Which::Record(1192), l(3, (578, 432)))],
        ("crab-clamp", _) => vec![(Which::Record(CLAMPED_CRAB), l(382, (382, 382)))],
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
    let hatched: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter_map(|(_, o)| crab::nest::data_of(o))
        .flat_map(|d| d.hatchlings.clone())
        .collect();
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

/// A store's time zero and random state for `clock`, and the run its
/// first trace came from.
fn time_zero(dir: &Path, clock: &str) -> (u64, u32, Option<String>) {
    let meta = std::fs::read_to_string(dir.join("meta.toml"))
        .unwrap_or_else(|e| panic!("{}: no meta: {e}", dir.display()));
    let t0 = meta_value(&meta, clock, "t0_ms")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{}: no t0_ms for {clock}", dir.display()));
    let seed = meta_value(&meta, clock, "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{}: no rng_at_t0", dir.display()));
    let run =
        meta_value(&meta, clock, "run_ids").and_then(|v| v.split('"').nth(1).map(str::to_string));
    (t0, seed, run)
}

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
    let pirate = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "RedTailPirate")
        .min_by_key(|(_, o)| (o.x - PIRATE_BY_THE_START_X).abs())
        .map(|(id, _)| id);
    sim.step(TickInput::default());
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
        w.claw.special.warp_to = Some(start);
        for _ in 0..level_start_draws() {
            w.rng.next_u15();
        }
    });
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// A crab on one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Crab {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
    frame: i32,
    vx: f64,
    vy: f64,
    bomber: bool,
}

/// A bomb on one tick: its position and whether it is exploding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bomb {
    x: i32,
    y: i32,
    exploding: bool,
}

/// The crab state a run's state name stands for.
fn state_named(name: &str) -> Option<EnemyState> {
    let word = name.split_once('_').map_or(name, |(_, w)| w);
    Some(match word {
        "HOME" => EnemyState::Regroup,
        "HIDE" => EnemyState::Hide,
        "DEFENSE" => EnemyState::Defense,
        "ATTACK" => EnemyState::Attack,
        "MOVINGATTACK" => EnemyState::MovingAttack,
        "STRIKE" => EnemyState::Snip,
        "FALLING" => EnemyState::Tumble,
        "NUDGE" => EnemyState::Nudge,
        "LAUNCH" => EnemyState::Launch,
        "MOVEBACK" => EnemyState::MoveBack,
        "DEAD" => EnemyState::Dead,
        _ => return None,
    })
}

/// A crab of a reference run on one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TheirCrab {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
}

/// The crabs of a reference run per step, in their order of first
/// appearance (by list position within a step); `None` when not listed
/// on that step.
fn reference_crabs(run: &Path, rows: usize) -> Option<Vec<Vec<Option<TheirCrab>>>> {
    let names: BTreeMap<i32, String> = std::fs::read_to_string(run.join("states.csv"))
        .ok()?
        .lines()
        .skip(1)
        .filter_map(|l| {
            let (id, name) = l.split_once(',')?;
            Some((id.parse().ok()?, name.to_string()))
        })
        .collect();
    let text = std::fs::read_to_string(run.join("obj-log.csv")).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (tick, serial, name, state, x, y) = (
        col("tick")?,
        col("serial")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
    );
    let mut order: Vec<String> = Vec::new();
    let mut out: Vec<Vec<Option<TheirCrab>>> = Vec::new();
    for l in lines {
        let c: Vec<&str> = l.split(',').collect();
        if c.get(name) != Some(&"HermitCrab") {
            continue;
        }
        let num = |i: usize| c.get(i).and_then(|v| v.parse::<i32>().ok());
        let (Some(t), Some(s)) = (num(tick), c.get(serial)) else {
            continue;
        };
        let n = match order.iter().position(|o| o == s) {
            Some(n) => n,
            None => {
                order.push(s.to_string());
                out.push(vec![None; rows]);
                order.len() - 1
            }
        };
        if let Some(slot) = out[n].get_mut(t as usize) {
            *slot = Some(TheirCrab {
                x: num(x)?,
                y: num(y)?,
                state: num(state)
                    .and_then(|s| names.get(&s))
                    .and_then(|n| state_named(n)),
            });
        }
    }
    Some(out)
}

/// One route played in the game.
struct Played {
    rows: Vec<TraceRow>,
    /// The crabs per step, in their order of first appearance (by x
    /// among those already built, by hatching order among a nest's).
    crabs: Vec<Vec<Option<Crab>>>,
    /// The bombs per step, in throwing order.
    bombs: Vec<Vec<Option<Bomb>>>,
    /// The watched nest per step: its state and picture.
    nest: Vec<Option<(i32, i32)>>,
    /// The sounds started on each step.
    cues: Vec<Vec<String>>,
}

fn find_nest(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == "CrabNest" && (o.x, o.y) == NEST)
        .map(|(id, _)| id)
}

fn hatched(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter_map(|(_, o)| nest::data_of(o))
        .flat_map(|d| d.hatchlings.clone())
        .collect()
}

fn crab_look(sim: &Sim, id: ObjectId) -> Option<Crab> {
    let o = sim.objects().get(id).filter(|o| o.is_listed())?;
    let d = enemy::data_of(o)?;
    Some(Crab {
        x: o.x,
        y: o.y,
        state: EnemyState::from_id(o.state),
        frame: o.frame,
        vx: d.vx,
        vy: d.vy,
        bomber: crab::fields_of(o).is_some_and(|f| f.bomber),
    })
}

/// Plays route `r` from its store's time zero at `clock` for `rows` rows.
fn play(level: &SimLevel, r: &Route, clock: &str, (t0, seed): (u64, u32), rows: usize) -> Played {
    let mut sim = at_time_zero(level, clock, r.start, t0, seed);
    let list = leftovers(r.store, clock);
    let mut done = Vec::new();
    // Crabs built before time zero already ran: they keep their start.
    apply_leftovers(&mut sim, &list, &mut done);
    let ticks = route_ticks(r);
    let nest_id = find_nest(&sim);
    let mut order: Vec<ObjectId> = Vec::new();
    let mut bombs: Vec<ObjectId> = Vec::new();
    let mut out = Played {
        rows: Vec::with_capacity(rows),
        crabs: Vec::new(),
        bombs: Vec::new(),
        nest: Vec::with_capacity(rows),
        cues: Vec::with_capacity(rows),
    };
    let newcomers = |sim: &Sim,
                     order: &mut Vec<ObjectId>,
                     crabs: &mut Vec<Vec<Option<Crab>>>,
                     k: usize| {
        let hatched = hatched(sim);
        let mut new: Vec<(usize, i32, ObjectId)> = sim
            .objects()
            .all()
            .filter(|(id, o)| crab::fields_of(o).is_some() && o.is_listed() && !order.contains(id))
            .map(|(id, o)| {
                let n = hatched.iter().position(|&h| h == id).unwrap_or(usize::MAX);
                (n, o.x, id)
            })
            .collect();
        new.sort();
        for (_, _, id) in new {
            order.push(id);
            crabs.push(vec![None; k]);
        }
    };
    newcomers(&sim, &mut order, &mut out.crabs, 0);
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        apply_leftovers(&mut sim, &list, &mut done);
        out.rows.push(TraceRow::capture(&sim, k as u64, "script"));
        newcomers(&sim, &mut order, &mut out.crabs, k);
        for (n, &id) in order.iter().enumerate() {
            out.crabs[n].push(crab_look(&sim, id));
        }
        for (id, o) in sim.objects().all() {
            if &*o.logic == "CrabBomb" && !bombs.contains(&id) {
                bombs.push(id);
                out.bombs.push(vec![None; k]);
            }
        }
        for (n, &id) in bombs.iter().enumerate() {
            let b = sim
                .objects()
                .get(id)
                .filter(|o| o.is_listed())
                .map(|o| Bomb {
                    x: o.x,
                    y: o.y,
                    exploding: o.state == bomb::state::EXPLODING,
                });
            out.bombs[n].push(b);
        }
        out.nest.push(
            nest_id
                .and_then(|id| sim.objects().get(id))
                .map(|o| (o.state, o.frame)),
        );
        out.cues.push(
            sim.last_audio()
                .iter()
                .filter_map(|c| match c {
                    AudioCue::Play(s) => Some(s.key.clone()),
                    _ => None,
                })
                .collect(),
        );
    }
    out
}

/// Plays `rows` rows of a route at the 18 ms step from its time zero.
fn played_c18(level: &SimLevel, store: &str, rows: usize) -> Played {
    let r = route_named(store);
    play(level, r, "c18", (r.t0_ms, SEED), rows)
}

/// The spans of a crab's states, as "Name@step".
fn spans<T>(crab: &[Option<T>], state: impl Fn(&T) -> Option<EnemyState>) -> String {
    let mut out = Vec::new();
    let mut last: Option<Option<EnemyState>> = None;
    for (k, c) in crab.iter().enumerate() {
        let s = c.as_ref().and_then(&state);
        if last != Some(s) {
            out.push(format!("{}@{k}", s.map_or("-", |s| s.name())));
            last = Some(s);
        }
    }
    out.join(" ")
}

/// The first step (as reference ms) on which each crab's position, state
/// or presence leaves the reference run's, by column.
fn compare_crabs(
    ours: &[Vec<Option<Crab>>],
    theirs: &[Vec<Option<TheirCrab>>],
) -> BTreeMap<String, u64> {
    let mut first = BTreeMap::new();
    for n in 0..ours.len().max(theirs.len()) {
        let none = Vec::new();
        let (a, b) = (
            ours.get(n).unwrap_or(&none),
            theirs.get(n).map_or(&[][..], |v| &v[..]),
        );
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
/// - c18, the rooms of the patrol 10629..10806 (`crab-fight`,
///   `crab-swing`, `crab-nudge`): Claw, the crabs and the random stream
///   follow the original step for step, except that a crab killed while
///   standing flies off 1 px lower on some steps: the original keeps a
///   fraction of a pixel in the height of a standing enemy that this game
///   does not (`crab-swing` from 954 ms, `crab-nudge` from 1224 ms and
///   4194 ms).
/// - c18, `crab-clamp`: this game's leave pass runs after the update pass
///   of the same tick, so the crab moved onto its limit by its own first
///   call is asleep at the end of that step (504 ms), where the original
///   still lists it on that step and lets it go on the next.
/// - c18, `crab-clamp`: the random stream around Claw's place differs
///   from the original's from the first step; the crab's construction and
///   clamp draw nothing and match.
/// - c18, `nest-hatch` and `nest-duck`: the nest hatches on the
///   original's step and its crabs fly, land and choose as the original's
///   do; the bomber's first bomb kills two of them standing in
///   `nest-hatch`, and their bodies fly off 1 px lower on some steps, as
///   in the patrol's rooms (from 2070 ms). In `nest-duck` the random
///   stream leaves the original's at 6894 ms and the crabs' choices with
///   it (not traced yet).
/// - c60 (the default tuning played tick by tick): the crabs count 18 ms
///   ticks where the original at 60 frames per second counts frames of
///   16 and 17 ms, so their crouches, snips and hides end at other times
///   and their choices draw other numbers; their snips reach Claw at other
///   times. In `nest-duck` they wear him down until he loses a life at
///   17 s, where the original's do not.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[
    ("nest-hatch", "c18", &[("crab0_y", 2070), ("crab1_y", 2070)]),
    (
        "nest-hatch",
        "c60",
        &[
            ("state_id", 950),
            ("health", 950),
            ("x", 983),
            ("cam_x", 983),
        ],
    ),
    (
        "nest-duck",
        "c18",
        &[
            ("rng", 6894),
            ("crab0_state", 7470),
            ("crab2_state", 7470),
            ("crab2_x", 7470),
            ("crab1_state", 8064),
            ("crab1_x", 8064),
            ("crab0_x", 12132),
            ("crab0_y", 12132),
            ("crab0_listed", 13068),
            ("state_id", 8802),
            ("health", 8802),
            ("x", 8820),
        ],
    ),
    (
        "nest-duck",
        "c60",
        &[
            ("x", 800),
            ("state_id", 800),
            ("physics_type", 800),
            ("health", 800),
            ("cam_x", 800),
            ("cam_y", 3500),
            ("y", 16283),
            ("events", 17136),
            ("lives", 17167),
        ],
    ),
    (
        "crab-fight",
        "c60",
        &[
            ("state_id", 700),
            ("health", 1217),
            ("x", 1267),
            ("cam_x", 1267),
            ("physics_type", 14450),
            ("y", 14767),
            ("cam_y", 14767),
        ],
    ),
    ("crab-swing", "c18", &[("crab1_y", 954)]),
    ("crab-nudge", "c18", &[("crab0_y", 1224), ("crab1_y", 4194)]),
    ("crab-clamp", "c18", &[("rng", 18), ("crab0_listed", 504)]),
];

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
        let path = root.join("level7").join(r.store).join("route.csv");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
        let rows: Vec<(u32, u32)> = text
            .lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
            .map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (hex(f[1]), hex(f[2]))
            })
            .collect();
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours, rows, "{}", r.store);
    }
}

#[test]
fn install_level7_crab_routes_follow_the_reference_traces() {
    let Some(level) = level7() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in selected() {
        for &clock in r.clocks {
            println!("== level7/{} {clock}", r.store);
            let dir = root.join("level7").join(r.store);
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let reference = trace::parse(&text).unwrap();
            let (t0, seed, run) = time_zero(&dir, clock);
            if clock == "c18" {
                assert_eq!((t0, seed), (r.t0_ms, SEED), "{}: time zero", r.store);
            }
            let played = play(&level, r, clock, (t0, seed), reference.len());
            let tol = if clock == "c18" {
                Tolerances::exact()
            } else {
                Tolerances::feel()
            };
            let cmp = trace::compare(&played.rows, &reference, &tol);
            print!("{}", trace::report(&played.rows, &reference, &cmp, 3));
            if cmp.rows_compared != reference.len() {
                failures.push(format!(
                    "{} {clock}: {} of {} rows compared",
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
                println!("crab {n}: {}", spans(c, |c| c.state));
            }
            let theirs = runs
                .as_ref()
                .zip(run.as_ref())
                .and_then(|(d, id)| reference_crabs(&d.join(id), reference.len()));
            if let Some(theirs) = &theirs {
                for (n, c) in theirs.iter().enumerate() {
                    println!("their crab {n}: {}", spans(c, |c| c.state));
                }
                if clock == "c18" {
                    firsts.extend(compare_crabs(&played.crabs, theirs));
                }
            }
            let list = known(r.store, clock);
            for (column, &first) in &firsts {
                println!("first {} {clock} {column} {first}", r.store);
                let gated = clock != "c18" || gated_c18(column);
                if !gated {
                    continue;
                }
                match list.iter().find(|(name, _)| name == column) {
                    Some(&(_, at)) if first >= at => {}
                    _ => failures.push(format!("{} {clock}: {column} at {first} ms", r.store)),
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the pins

/// The rows on which `f` holds and did not on the row before.
fn starts<T>(rows: &[T], f: impl Fn(&T) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

fn cue_rows(p: &Played, key: &str) -> Vec<usize> {
    (0..p.cues.len())
        .filter(|&k| p.cues[k].iter().any(|c| c == key))
        .collect()
}

#[test]
fn install_level7_the_nest_hatches_on_the_originals_steps() {
    let Some(level) = level7() else { return };
    let p = played_c18(&level, "nest-hatch", 120);
    // Claw walks into its box on row 30: the hatch; its second picture,
    // the boink and the three crabs on the fourth hatch row; quiet for
    // good from the sixteenth.
    let nest: Vec<i32> = p.nest.iter().map(|n| n.expect("the nest").0).collect();
    assert_eq!(starts(&nest, |&s| s == nest::state::HATCH), [30]);
    assert_eq!(starts(&nest, |&s| s == nest::state::IDLE), [0, 46]);
    assert_eq!(p.nest[34].map(|n| n.1), Some(nest::SPAWN_FRAME));
    assert_eq!(cue_rows(&p, nest::HATCH_SOUND), [34]);
    assert_eq!(p.crabs.len(), 3);
    let first: Vec<Crab> = p.crabs.iter().map(|c| c[34].expect("hatched")).collect();
    for c in &first {
        assert!(p.crabs.iter().all(|c| c[33].is_none()));
        assert_eq!(
            (c.x, c.y, c.state),
            (NEST.0, 7424, Some(EnemyState::Tumble))
        );
        assert!((-400.0..=-200.0).contains(&c.vy), "{c:?}");
    }
    // The fan: -75 and -25 px/s, the last crab a drawn speed.
    assert_eq!((first[0].vx, first[1].vx), (-75.0, -25.0));
    assert!((-200.0..=200.0).contains(&first[2].vx));
    // Each lands on the nest's floor and regroups there.
    for c in &p.crabs {
        let landed = c
            .iter()
            .flatten()
            .find(|c| c.state == Some(EnemyState::Regroup))
            .expect("lands");
        assert_eq!(landed.y, 7424);
    }
}

#[test]
fn install_level7_a_swing_knocks_a_hiding_crab_aside_37_px() {
    let Some(level) = level7() else { return };
    let p = played_c18(&level, "crab-nudge", 300);
    let c = &p.crabs[0];
    let knocked: Vec<usize> = (0..c.len())
        .filter(|&k| c[k].is_some_and(|c| c.state == Some(EnemyState::Nudge)))
        .collect();
    // The swing live on rows 36 to 42 knocks it on each of them: one
    // 5.4 px step a row, from 10653 to 10690, then it regroups and
    // crouches; the boink once, on the 200 ms timer.
    assert_eq!(knocked[..7], [36, 37, 38, 39, 40, 41, 42]);
    assert_eq!(c[36].unwrap().x, 10653);
    assert_eq!(
        c[43].map(|c| (c.x, c.state)),
        Some((10690, Some(EnemyState::Regroup)))
    );
    assert_eq!(cue_rows(&p, crab::SHELL_SOUND), [36]);
    // Out of its shell, charging, the next swing kills it at once.
    let dead = starts(c, |c| c.is_some_and(|c| c.state == Some(EnemyState::Dead)));
    assert_eq!(dead, [66]);
    assert_eq!(c[65].unwrap().state, Some(EnemyState::MovingAttack));
    assert!(p.cues[66].iter().any(|k| k == crab::DEATH_SOUND));
}

#[test]
fn install_level7_the_crab_placed_beside_its_patrol_is_built_on_its_limit() {
    let Some(level) = level7() else { return };
    let r = route_named("crab-clamp");
    let mut sim = at_time_zero(&level, "c18", r.start, r.t0_ms, SEED);
    let ticks = route_ticks(r);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.id == CLAMPED_CRAB && o.record.is_some())
        .map(|(id, _)| id)
        .expect("the record");
    let mut built = None;
    for (k, t) in ticks.iter().enumerate().take(60) {
        sim.step(*t);
        let o = sim.objects().get(id).expect("the crab");
        if built.is_none() && crab::fields_of(o).is_some() {
            built = Some(k);
            // Built when the view nears its record, already on its limit,
            // on the floor there, regrouping.
            assert_eq!((o.x, o.y), CLAMPED_AT);
            assert_eq!(EnemyState::from_id(o.state), Some(EnemyState::Regroup));
        }
        if built.is_some() {
            assert!(!o.is_listed(), "asleep at {k}");
            assert_eq!((o.x, o.y), CLAMPED_AT);
        }
    }
    assert_eq!(built, Some(27));
}

#[test]
fn install_level7_bombs_leave_on_the_third_launch_tick_and_blow_for_16_ticks() {
    let Some(level) = level7() else { return };
    let mut throws = 0;
    for store in ["nest-hatch", "nest-duck"] {
        let p = played_c18(&level, store, 1100);
        let launches: Vec<usize> = p
            .crabs
            .iter()
            .flat_map(|c| {
                starts(c, |c| {
                    c.is_some_and(|c| c.state == Some(EnemyState::Launch))
                })
            })
            .collect();
        for b in &p.bombs {
            let first = b.iter().position(Option::is_some).expect("thrown");
            let last = b.iter().rposition(Option::is_some).unwrap();
            throws += 1;
            // Thrown on its crab's third launch row, with the nest
            // bomber's throw sound (the snip).
            assert!(launches.contains(&(first - 3)), "{store}: bomb at {first}");
            assert!(p.cues[first].iter().any(|k| k == crab::SNIP_SOUND));
            // It goes off on Claw or on its fuse, 56 rows after the
            // throw at the latest, and blows for 16 rows.
            let boom = b
                .iter()
                .position(|b| b.is_some_and(|b| b.exploding))
                .unwrap();
            assert!(
                boom - first <= 56,
                "{store}: bomb at {first} goes off at {boom}"
            );
            assert_eq!(last - boom + 1, 16, "{store}: bomb at {first}");
            assert!(
                b[boom..=last]
                    .iter()
                    .all(|b| b.is_some_and(|b| b.exploding))
            );
            assert!(p.cues[boom + 1].iter().any(|k| k == "GAME_BOMBEXP"));
        }
    }
    assert!(throws >= 2);
}
