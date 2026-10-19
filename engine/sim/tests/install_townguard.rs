//! The fifth level's town guards and jump switches on your own install,
//! against the original's runs of the same short routes.
//!
//! Each route starts the level with Claw at a fixed point of it, as the
//! original's runs put him there once the level has loaded (see
//! [`as_after_a_teleport`]), and leaves him there until the store's time
//! zero (828 ms of game time at the original speeds, 817 ms at the
//! default ones, so the frame pulse and the level's timed sounds are in
//! the same phase), then starts the shared random stream from the store's
//! state at time zero. It is then played tick by tick and compared with
//! the reference trace of the same route: Claw's position, state and
//! health exactly at the original speeds (c18), within the feel
//! tolerances at the default ones (c60). The routes:
//!
//! - `tg1-engage`: Claw stands 141 px right of a first guard while it
//!   idles, engages him and strikes him again and again;
//! - `tg1-duck`: the same with Claw crouching (the low strike);
//! - `tg1-air`: the same with Claw jumping into the guard's strike: hit
//!   in the air, he hovers and stands there before falling;
//! - `tg1-swing`: Claw turns to a first guard and swings at it every 15
//!   ticks: on the fifth level it neither hops back nor parries, flinches
//!   and dies on the second blow;
//! - `tg2-swing`: Claw swings at an idle second guard every 15 ticks: it
//!   hops back, parries, flinches and dies;
//! - `tg2-lift`: Claw lifts a second guard, and throws it;
//! - `js-2402`, `js-7280`: guards pacing over jump switches are launched
//!   through the switches' targets and land on the floors below.
//!
//! When the reference runs also list the guards per step (`obj-log.csv`
//! beside each run, in the folder named by `CLAW_REFERENCE_RUNS`, by
//! default `rt` next to the reference store), the watched guard's
//! position, state and health are compared too.
//!
//! The routes are written below; when a store holds its own copy of a
//! route, the two must agree. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons; each test skips itself when what it needs is
//! unset; `CLAW_L5_ONLY` names one store to play alone. Nothing from an
//! install or a reference store is kept in the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::activation::Centres;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, ObjectId, engine_state};
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::{Buttons, GameLogic, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;
const LIFT: u32 = 0x20;

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
/// (inclusive), after `before`.
#[derive(Debug, Clone, Copy)]
struct Taps {
    first: usize,
    last: usize,
    every: usize,
    held: u32,
}

/// A route of the stores: its name, where Claw is put, how many rows it
/// has, its stretches, its repeated taps, the clocks it is compared at,
/// and the record of the guard it watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    taps: Option<Taps>,
    clocks: &'static [&'static str],
    guard: i32,
}

/// The first guard's room: Claw 141 px right of `TownGuard1` 6386.
const TG1_ROOM: (i32, i32) = (5600, 3000);
/// The second guard's room: Claw 85 px right of `TownGuard2` 6447.
const TG2_ROOM: (i32, i32) = (7790, 3250);

const ROUTES: [Route; 8] = [
    Route {
        store: "tg1-engage",
        start: TG1_ROOM,
        rows: 1400,
        spans: &[],
        taps: None,
        clocks: &["c18", "c60"],
        guard: 6386,
    },
    Route {
        store: "tg1-duck",
        start: TG1_ROOM,
        rows: 1400,
        spans: &[span(15, 1399, DOWN)],
        taps: None,
        clocks: &["c18", "c60"],
        guard: 6386,
    },
    Route {
        store: "tg1-air",
        start: TG1_ROOM,
        rows: 400,
        spans: &[span(200, 207, JUMP)],
        taps: None,
        clocks: &["c18"],
        guard: 6386,
    },
    Route {
        store: "tg1-swing",
        start: TG1_ROOM,
        rows: 900,
        spans: &[span(150, 151, LEFT)],
        taps: Some(Taps {
            first: 160,
            last: 850,
            every: 15,
            held: ATTACK,
        }),
        clocks: &["c18"],
        guard: 6386,
    },
    Route {
        store: "tg2-swing",
        start: TG2_ROOM,
        rows: 700,
        spans: &[span(15, 16, LEFT)],
        taps: Some(Taps {
            first: 30,
            last: 585,
            every: 15,
            held: ATTACK,
        }),
        clocks: &["c18", "c60"],
        guard: 6447,
    },
    Route {
        store: "tg2-lift",
        start: TG2_ROOM,
        rows: 500,
        spans: &[span(15, 16, LEFT), span(30, 31, LIFT), span(70, 71, LIFT)],
        taps: None,
        clocks: &["c18"],
        guard: 6447,
    },
    Route {
        store: "js-2402",
        start: (2300, 2800),
        rows: 600,
        spans: &[],
        taps: None,
        clocks: &["c18", "c60"],
        guard: 8778,
    },
    Route {
        store: "js-7280",
        start: (7000, 3250),
        rows: 1100,
        spans: &[],
        taps: None,
        clocks: &["c18", "c60"],
        guard: 6408,
    },
];

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

fn meta_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    meta.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"'))
}

/// A value of the meta's section for `clock`.
fn clock_value<'a>(meta: &'a str, clock: &str, key: &str) -> Option<&'a str> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let section = section.split("\n[").next()?;
    meta_value(section, key)
}

/// The first run named in the meta's section for `clock`.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let line = section
        .lines()
        .find(|l| l.trim_start().starts_with("run_ids"))?;
    Some(line.split('"').nth(1)?.to_string())
}

/// A guard on one tick: its position and state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Guard {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
}

fn find_record(sim: &Sim, record: i32) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| o.id == record && o.record.is_some())
        .map(|(id, _)| id)
}

fn guard_of(sim: &Sim, id: Option<ObjectId>) -> Option<Guard> {
    let o = sim.objects().get(id?)?;
    Some(Guard {
        x: o.x,
        y: o.y,
        state: EnemyState::from_id(o.state),
    })
}

/// The product state a reference run's state name stands for. The run
/// names a state with a short prefix and an upper-case word; only the word
/// after the first `_` is read.
fn state_named(name: &str) -> Option<EnemyState> {
    let word = name.split_once('_').map_or(name, |(_, w)| w);
    Some(match word {
        "IDLE" => EnemyState::Idle,
        "IDLE2" => EnemyState::Idle2,
        "HOME" => EnemyState::Home,
        "STRIKE" => EnemyState::Strike,
        "PACING" => EnemyState::Pace,
        "HITLOW" => EnemyState::HitLow,
        "HITHIGH" => EnemyState::HitHigh,
        "DEAD" => EnemyState::Dead,
        "PAUSE" => EnemyState::Pause,
        "FALLING" => EnemyState::Falling,
        "ADVANCE" => EnemyState::Advance,
        "FASTADVANCE" => EnemyState::FastAdvance,
        "JUMP" => EnemyState::Jump,
        "JUMPBACK" => EnemyState::JumpBack,
        "JUMPATCLAW" => EnemyState::JumpAtClaw,
        "MOVEBACK" => EnemyState::MoveBack,
        "MOVEAHEAD" => EnemyState::MoveAhead,
        "BLOCK" => EnemyState::Block,
        "LIFTED" => EnemyState::Lifted,
        "FLAILING" => EnemyState::Flailing,
        "THROWN" => EnemyState::Thrown,
        "BOUNCE" => EnemyState::Bounce,
        "DROPPED" => EnemyState::Dropped,
        _ => return None,
    })
}

/// The watched guard per step of a reference run, from its object log:
/// the guard of `logic` standing nearest `at` on the first step.
fn reference_guard(run: &Path, logic: &str, at: (i32, i32)) -> Option<Vec<Option<Guard>>> {
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
    let rows: Vec<Vec<&str>> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|c| c.get(name) == Some(&logic))
        .collect();
    let num = |c: &Vec<&str>, i: usize| c.get(i).and_then(|v| v.parse::<i32>().ok());
    let first_tick = rows.iter().filter_map(|c| num(c, tick)).min()?;
    let watched = rows
        .iter()
        .filter(|c| num(c, tick) == Some(first_tick))
        .min_by_key(|c| {
            let dx = num(c, x).unwrap_or(i32::MAX / 2) - at.0;
            let dy = num(c, y).unwrap_or(i32::MAX / 2) - at.1;
            i64::from(dx).pow(2) + i64::from(dy).pow(2)
        })?
        .get(serial)?
        .to_string();
    let mut by_tick: BTreeMap<usize, Guard> = BTreeMap::new();
    for c in rows
        .iter()
        .filter(|c| c.get(serial) == Some(&watched.as_str()))
    {
        let (Some(t), Some(s), Some(gx), Some(gy)) =
            (num(c, tick), num(c, state), num(c, x), num(c, y))
        else {
            continue;
        };
        by_tick.insert(
            t as usize,
            Guard {
                x: gx,
                y: gy,
                state: names.get(&s).and_then(|n| state_named(n)),
            },
        );
    }
    let last = *by_tick.keys().max()?;
    Some((0..=last).map(|t| by_tick.get(&t).copied()).collect())
}

/// One route played in the game.
struct Played {
    rows: Vec<TraceRow>,
    guard: Vec<Option<Guard>>,
    logic: String,
    guard_at_t0: Option<(i32, i32)>,
}

/// The level with Claw starting at `at`: he is put on the floor below
/// it, and the camera starts there.
fn level_at(level: &SimLevel, at: (i32, i32)) -> SimLevel {
    let mut level = level.clone();
    level.start = at;
    level
}

/// In the original's runs the level loads with Claw at its own start and
/// he is then put at the route's start, so only the enemies around the
/// level's start are set up with the level, and they have already seen
/// Claw out of their range by then; the guards and seagulls around the
/// route's start are built on the first tick, as any enemy that wakes up.
/// The game is started at the route's start ([`level_at`]), which sets
/// all of them up with the level: the ones outside the level start's
/// window are built again on the first tick from where they were placed
/// (a built enemy's exact height starts at its placement point, above the
/// floor it is dropped onto), and the others are told that Claw was out
/// of range.
fn as_after_a_teleport(sim: &mut Sim, level_start: (i32, i32)) {
    let cfg = sim.activation().config().clone();
    let c = Centres::new(&cfg, sim.level().plane(), level_start);
    sim.with_world(|w| {
        let ids: Vec<(ObjectId, bool)> = w
            .objects
            .all()
            .filter(|(_, o)| {
                matches!(
                    o.kind,
                    LogicKind::Game(
                        GameLogic::TownGuard1 | GameLogic::TownGuard2 | GameLogic::Seagull
                    )
                ) && o.state != engine_state::INIT
            })
            .map(|(id, o)| {
                let (hx, hy) = cfg.half_extents(o.activation_class(), cfg.view);
                let (x, y) = o.grid_point;
                let near_start = (c.main.0 - hx..=c.main.0 + hx).contains(&x)
                    && (c.main.1 - hy..=c.main.1 + hy).contains(&y);
                (id, near_start)
            })
            .collect();
        for (id, near_start) in ids {
            if near_start {
                if let Some(d) = enemy::data_mut(w, id) {
                    d.out_of_range = true;
                }
            } else if let Some(o) = w.objects.get_mut(id) {
                o.state = engine_state::INIT;
                o.data = ObjectData::None;
                (o.x, o.y) = (o.editor.0, o.editor.1);
            }
        }
    });
}

/// Claw left at his start until `t0_ms` of game time has passed, then the
/// random stream set to `seed`: the next tick is the route's time zero.
fn start_at(sim: &mut Sim, level_start: (i32, i32), t0_ms: u64, seed: u32) {
    as_after_a_teleport(sim, level_start);
    for _ in 0..t0_ms.div_ceil(TICK) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
}

/// Plays route `r` at `clock` for `rows` ticks from its time zero. With
/// `resync` (the reference trace at c18), each tick starts from the
/// original's random state at the end of the row before, so a family of
/// the level that draws differently (or is not built yet) shows on the
/// random stream's own column on its ticks without moving every later
/// draw of the level.
fn play(
    level: &SimLevel,
    r: &Route,
    (seed, t0_ms): (u32, u64),
    clock: &str,
    rows: usize,
    resync: Option<&[TraceRow]>,
) -> Played {
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
    let mut sim = Sim::new(level_at(level, r.start), cfg);
    start_at(&mut sim, level.start, t0_ms, seed);
    let ticks = route_ticks(r);
    let watched = find_record(&sim, r.guard);
    let logic = watched
        .and_then(|id| sim.objects().get(id))
        .map_or(String::new(), |o| o.logic.to_string());
    let mut out = Played {
        rows: Vec::with_capacity(rows),
        guard: Vec::with_capacity(rows),
        logic,
        guard_at_t0: None,
    };
    for k in 0..rows {
        if let Some(state) = resync.and_then(|t| t.get(k.checked_sub(1)?)?.rng) {
            sim.with_world(|w| *w.rng = Rng::new(state));
        }
        sim.step(ticks.get(k).copied().unwrap_or_default());
        out.rows.push(TraceRow::capture(&sim, k as u64, "script"));
        let g = guard_of(&sim, watched);
        if k == 0 {
            out.guard_at_t0 = g.map(|g| (g.x, g.y));
        }
        out.guard.push(g);
    }
    out
}

/// The first step on which the watched guard's x, y and state leave the
/// reference run's, by column (exact).
fn compare_guard(played: &[Option<Guard>], reference: &[Option<Guard>]) -> BTreeMap<String, u64> {
    let mut first = BTreeMap::new();
    for (k, (mine, theirs)) in played.iter().zip(reference).enumerate() {
        let t = (k as u64 + 1) * TICK;
        let (Some(m), Some(r)) = (mine, theirs) else {
            continue;
        };
        if m.x != r.x {
            first.entry("guard_x".to_string()).or_insert(t);
        }
        if m.y != r.y {
            first.entry("guard_y".to_string()).or_insert(t);
        }
        if r.state.is_some() && m.state != r.state {
            first.entry("guard_state".to_string()).or_insert(t);
        }
    }
    first
}

/// The spans of the watched guard's states, as "Name@step".
fn spans(guard: &[Option<Guard>]) -> String {
    let mut out = Vec::new();
    let mut last = None;
    for (k, g) in guard.iter().enumerate() {
        let s = g.and_then(|g| g.state);
        if s != last {
            out.push(format!("{}@{k}", s.map_or("-", |s| s.name())));
            last = s;
        }
    }
    out.join(" ")
}

/// The first divergence of every column of one comparison, by column.
fn trace_firsts(cmp: &Comparison) -> BTreeMap<String, u64> {
    cmp.per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.clone(), t)))
        .collect()
}

/// A store, a clock, and the first divergence (ms) of each column known
/// there; a column not listed must stay within its tolerance (exact at
/// c18), and one listed must not leave it earlier. What they come from:
///
/// - c18: the guards' idle routines, their engaging, every strike high
///   and low, the hop back, the parry, the flinches and deaths, the lift
///   and throw of a second guard, the switch launches and the strike that
///   meets Claw in the air follow the original step for step, and so does
///   Claw. Each tick starts from the original's random state (see
///   [`play`]), and the random stream's own column is only reported: the
///   seagulls of the guards' rooms are not built in this build, so the
///   original draws more on the frame pulse ticks.
/// - c18, a standing enemy keeps a fraction of a pixel in its height: a
///   guard killed while standing flies off from it as in the original
///   (`tg2-swing`). The switch guard by the level's start, set up with the
///   level, carries 0.648 px from the level's load in the original, a
///   fraction the game's does not carry yet, so its arc from the switch
///   is a pixel off from 3150 ms, it lands a tick sooner and reaches Claw
///   a tick sooner (`js-2402`, Claw from 5688 ms).
/// - c60 (the default tuning played tick by tick): the game's 18 ms ticks
///   against the original's frames of 16 and 17 ms put the guards' idle
///   routines, strikes and hops a few frames apart, so the first strike
///   that reaches Claw comes earlier or later (2367 ms, 4150 ms, 4717 ms,
///   7667 ms); after a second guard's death its coin flies elsewhere, so
///   Claw collects it in the game only (`tg2-swing`, score, 3417 ms).
///   These follow the random stream, which at c60 is not set from the
///   original's on every tick: with the seagulls built they are only
///   reported.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[
    (
        "js-2402",
        "c18",
        &[
            ("guard_y", 3150),
            ("guard_x", 3762),
            ("guard_state", 3870),
            ("state_id", 5688),
            ("health", 5688),
            ("x", 5706),
        ],
    ),
    (
        "tg1-engage",
        "c60",
        &[
            ("health", 2367),
            ("state_id", 2367),
            ("x", 2417),
            ("cam_x", 2417),
            ("physics_type", 15133),
            ("y", 15483),
            ("cam_y", 15483),
        ],
    ),
    (
        "tg1-duck",
        "c60",
        &[
            ("cam_y", 2033),
            ("state_id", 4150),
            ("health", 4150),
            ("physics_type", 4150),
            ("x", 4200),
            ("cam_x", 4200),
        ],
    ),
    ("tg2-swing", "c60", &[("score", 3417)]),
    (
        "js-2402",
        "c60",
        &[
            ("health", 7667),
            ("state_id", 7667),
            ("x", 7717),
            ("cam_x", 7717),
        ],
    ),
    (
        "js-7280",
        "c60",
        &[
            ("health", 4717),
            ("state_id", 4717),
            ("x", 4767),
            ("cam_x", 4767),
            ("physics_type", 15400),
            ("y", 16217),
            ("events", 17067),
            ("lives", 17067),
            ("cam_y", 17083),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

/// Columns compared exactly at c18: Claw's position, state and health;
/// the others are reported.
const GATED_C18: [&str; 4] = ["x", "y", "state_id", "health"];

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
        assert_eq!(ours, rows, "{}", r.store);
    }
}

#[test]
fn install_level5_guard_and_switch_routes_follow_the_reference_traces() {
    let Some(level) = level5() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    let only = std::env::var("CLAW_L5_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level5").join(r.store);
        let Ok(meta) = std::fs::read_to_string(dir.join("meta.toml")) else {
            println!("== level5/{}: no store, skipped", r.store);
            continue;
        };
        let seed: u32 = meta_value(&meta, "rng_at_t0").unwrap().parse().unwrap();
        for &clock in r.clocks {
            let Ok(text) = std::fs::read_to_string(dir.join(format!("trace-{clock}-r1.csv")))
            else {
                println!("== level5/{} {clock}: no trace, skipped", r.store);
                continue;
            };
            let reference = trace::parse(&text).unwrap();
            let t0_ms: u64 = clock_value(&meta, clock, "t0_ms").unwrap().parse().unwrap();
            let rows = (reference.last().map_or(0, |row| row.t_ms) / TICK) as usize;
            let resync = (clock == "c18").then_some(reference.as_slice());
            let played = play(
                &level,
                r,
                (seed, t0_ms),
                clock,
                rows.max(reference.len()),
                resync,
            );
            let tol = if clock == "c18" {
                Tolerances::exact()
            } else {
                Tolerances::feel()
            };
            let cmp = trace::compare(&played.rows, &reference, &tol);
            println!("== level5/{} {clock}", r.store);
            print!("{}", trace::report(&played.rows, &reference, &cmp, 3));
            println!("game guard {}: {}", r.guard, spans(&played.guard));
            let mut firsts = trace_firsts(&cmp);
            if clock == "c18"
                && let (Some(runs), Some(at)) = (runs.as_ref(), played.guard_at_t0)
                && let Some(id) = run_id(&meta, clock)
                && let Some(theirs) = reference_guard(&runs.join(&id), &played.logic, at)
            {
                println!("original guard: {}", spans(&theirs));
                let g = compare_guard(&played.guard, &theirs);
                println!("guard divergences: {g:?}");
                if let Some(&t) = g.values().min() {
                    let k = (t / TICK) as usize - 1;
                    let from = k.saturating_sub(2);
                    for (j, original) in theirs.iter().enumerate().skip(from).take(5) {
                        println!(
                            "  step {j}: game {:?} original {original:?}",
                            played.guard.get(j).copied().flatten(),
                        );
                    }
                }
                firsts.extend(g);
            }
            let gated = GATED_C18;
            let allowed = known(r.store, clock);
            // At c60 the times follow the random stream, which the
            // seagulls draw from once they are built.
            let c60_gated = !logic::built(GameLogic::Seagull);
            for (col, t) in &firsts {
                let listed = allowed.iter().find(|(c, _)| c == col).map(|(_, t)| *t);
                let gate = if clock == "c18" {
                    gated.contains(&col.as_str()) || col.starts_with("guard")
                } else {
                    c60_gated
                };
                match listed {
                    Some(at) if *t >= at => {}
                    _ if !gate => println!("  (reported) {col} first leaves at {t} ms"),
                    _ => failures.push(format!(
                        "level5/{} {clock}: {col} first leaves at {t} ms (known: {listed:?})",
                        r.store
                    )),
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
