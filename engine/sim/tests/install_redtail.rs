//! The thirteenth level's boss room on your own install: RedTail, his
//! stakes and his wind, played against the original's runs of the same
//! routes, and the fight's main outcomes checked on their own.
//!
//! Each route puts Claw down after the level's first tick, the way the
//! original's runs put him there (between two ticks, as a warp does but
//! without the warp's own frame), at the height he settles at, left of the
//! boss room's warp; waits until the level clock reaches the time the
//! original's run started at; and starts the shared random stream from the
//! run's state, once. Then, by route:
//!
//! - the normal entry: Right from row 15 walks Claw into the warp; he
//!   stands on the sand where it puts him through the introduction and
//!   about 20 s of the fight (the flare gun, runs, slides and kicks, the
//!   wind and the stakes, two deaths and two respawns);
//! - the intro skip: the same walk and Jump on Claw's first row in the
//!   arena (30 at the original's speeds, 27 at the default ones), which
//!   leaves RedTail waiting for the fight for good; and Jump one row late
//!   (31), which runs the introduction;
//! - the ledge: the skip, a full jump onto the left peg (row 150) and a
//!   jump right onto the ledge (rows 185 on): Claw stands 71 px left of
//!   the waiting RedTail;
//! - the kill (a measuring run: RedTail's health is 5 from the first
//!   arena step, the route's `# boss_health 5` line): the ledge route,
//!   one sword blow on row 225, his death jump, his death on the sand,
//!   the gem, and the walk and jump that collect it;
//! - the wind: the normal entry, then from the wind's first push row a
//!   walk to the right into it, or a full jump.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of its clock: at the original's speeds (c18) every column exactly, the
//! random stream and the boss columns included; at the default speeds
//! played tick by tick (c60) within the feel tolerances. When the reference
//! runs also list RedTail per step (`boss.csv`) and the objects near Claw
//! (`obj-log.csv`, beside each run, in the folder named by
//! `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference store),
//! RedTail's position, state and health and the wind's state are compared
//! exactly at c18, their states lined up by the numbers the runs give them
//! ([`reference_state`]).
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below ([`route`]);
//! each store's own copy of its route must agree with them. Needs
//! `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each test
//! skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but a listed store lacks its route, meta or
//! trace, or the reference runs (or a run's `boss.csv` or `obj-log.csv`)
//! are missing; `CLAW_L13B_ONLY` names one store to play alone,
//! `CLAW_L13B_TRACE` a folder to write the replay's traces to and
//! `CLAW_L13B_DUMP=FROM..TO` prints RedTail and Claw of both on those
//! rows. Nothing from an install or a reference store is kept in the
//! repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::boss::{debris, flare, wind};
use claw_sim::logic::enemy::{EnemyState, redtail};
use claw_sim::logic::pickup;
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state as claw_state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The attack button.
const ATTACK: u32 = 0x2;

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The game time at a route's time zero, by clock.
const T0_MS: [(&str, u64); 2] = [("c18", 828), ("c60", 817)];

/// Where the routes put Claw: left of the warp into the boss room, at the
/// height he settles at.
const WARP_WALK: (i32, i32) = (32900, 2327);
/// RedTail, where he is placed.
const REDTAIL_AT: (i32, i32) = (37431, 1886);
/// The wind, where it is placed.
const WIND_AT: (i32, i32) = (37509, 2009);

/// The stores, with their clocks.
const STORES: [(&str, &[&str]); 12] = [
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-intro-skip-c18", &["c18"]),
    ("boss-intro-skip", &["c60"]),
    ("boss-intro-skip-late-c18", &["c18"]),
    ("boss-skip-ledge-c18", &["c18"]),
    ("boss-skip-ledge", &["c60"]),
    ("boss-kill-ledge-c18", &["c18"]),
    ("boss-kill-ledge", &["c60"]),
    ("boss-wind-walk-c18", &["c18"]),
    ("boss-wind-walk", &["c60"]),
    ("boss-wind-jump-c18", &["c18"]),
    ("boss-wind-jump", &["c60"]),
];

// ----------------------------------------------------------------------
// The routes

/// A route: how many rows, the keys held per row, RedTail's health from
/// the first arena step for a measuring route, and whether it ends the
/// level.
#[derive(Debug, Clone, Default)]
struct Route {
    held: Vec<u32>,
    boss_health: Option<i32>,
    completes: bool,
}

impl Route {
    fn new(rows: usize) -> Route {
        Route {
            held: vec![0; rows],
            ..Route::default()
        }
    }

    /// Holds `mask` on rows `from..=to`.
    fn hold(mut self, mask: u32, from: usize, to: usize) -> Route {
        let to = to.min(self.held.len() - 1);
        for h in &mut self.held[from..=to] {
            *h |= mask;
        }
        self
    }

    /// Held and pressed per row: a key is pressed on the first row it is
    /// held.
    fn rows(&self) -> Vec<(u32, u32)> {
        let mut prev = 0;
        self.held
            .iter()
            .map(|&h| {
                let p = h & !prev;
                prev = h;
                (h, p)
            })
            .collect()
    }

    fn ticks(&self) -> Vec<TickInput> {
        self.rows()
            .into_iter()
            .map(|(h, p)| TickInput {
                held: Buttons(h),
                pressed: Buttons(p),
            })
            .collect()
    }
}

/// The skip and the climb onto the ledge, Jump on the first arena row
/// `first` and Right released on row `right_to`, in `rows` rows.
fn skip_ledge(rows: usize, first: usize, skip_right: usize, right_to: usize) -> Route {
    Route::new(rows)
        .hold(RIGHT, 15, skip_right)
        .hold(JUMP, first, first + 21)
        .hold(JUMP, 150, 163)
        .hold(JUMP, 185, 198)
        .hold(RIGHT, 185, right_to)
}

/// The route of `store`.
fn route(store: &str) -> Route {
    match store {
        "boss-intro-normal" => Route::new(1600).hold(RIGHT, 15, 59),
        "boss-intro-skip-c18" => Route::new(1800).hold(RIGHT, 15, 49).hold(JUMP, 30, 51),
        "boss-intro-skip" => Route::new(1900).hold(RIGHT, 15, 46).hold(JUMP, 27, 48),
        "boss-intro-skip-late-c18" => Route::new(800).hold(RIGHT, 15, 49).hold(JUMP, 31, 52),
        "boss-skip-ledge-c18" => skip_ledge(2000, 30, 49, 209),
        "boss-skip-ledge" => skip_ledge(2100, 27, 46, 202),
        "boss-kill-ledge-c18" => Route {
            boss_health: Some(5),
            completes: true,
            ..skip_ledge(700, 30, 49, 209)
                .hold(ATTACK, 225, 226)
                .hold(RIGHT, 392, 570)
                .hold(JUMP, 569, 578)
        },
        "boss-kill-ledge" => Route {
            boss_health: Some(5),
            completes: true,
            ..skip_ledge(700, 27, 46, 202)
                .hold(ATTACK, 225, 226)
                .hold(RIGHT, 393, 564)
                .hold(JUMP, 573, 581)
        },
        "boss-wind-walk-c18" => Route::new(1400).hold(RIGHT, 15, 59).hold(RIGHT, 1229, 1309),
        "boss-wind-walk" => Route::new(1500).hold(RIGHT, 15, 59).hold(RIGHT, 1347, 1429),
        "boss-wind-jump-c18" => Route::new(1400).hold(RIGHT, 15, 59).hold(JUMP, 1229, 1242),
        "boss-wind-jump" => Route::new(1500).hold(RIGHT, 15, 59).hold(JUMP, 1347, 1360),
        _ => panic!("no route {store}"),
    }
}

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The normal entry at the default speeds (c60): the walk into the warp
/// ends a pixel off the original's 60 frames per second walk (the
/// checkpoint at 37180 against 37181, then the camera), and the
/// introduction's delays count 18 ms ticks against frames of 16 and 17 ms,
/// so the fight starts 256 ms later here; RedTail's decisions then fall
/// on other ticks and other draws, and the fight goes its own way (Claw is
/// hit and dies at other times).
const NORMAL_ENTRY_C60: &[(&str, u64)] = &[
    ("events", 500),
    ("cam_y", 883),
    ("cam_x", 1017),
    ("boss_fight", 11300),
    ("state_id", 11317),
    ("health", 13367),
    ("x", 13400),
    ("physics_type", 19383),
    ("y", 20183),
    ("lives", 21067),
];

/// The climb onto the ledge at the default speeds (c60): Claw's full jump
/// onto the peg, played in 18 ms ticks, runs a few pixels off the
/// original's 60 frames per second arc near its top (10 px at most), and
/// the camera with him.
const CLIMB_C60: &[(&str, u64)] = &[("y", 3067), ("cam_y", 3067)];

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. At the original's speeds
/// (c18) every store follows the original exactly. At the default speeds
/// (c60) see [`NORMAL_ENTRY_C60`] and [`CLIMB_C60`]; on the kill route the
/// death jump's aim comes from another draw (the random stream is drawn at
/// other ticks at that speed), so he lands 18 px left of the original's
/// landing, the gem rises from there and the camera that follows it
/// leaves the original's (7367 ms).
const KNOWN: &[Known] = &[
    ("boss-intro-normal", "c60", NORMAL_ENTRY_C60),
    ("boss-wind-walk", "c60", NORMAL_ENTRY_C60),
    ("boss-wind-jump", "c60", NORMAL_ENTRY_C60),
    ("boss-skip-ledge", "c60", CLIMB_C60),
    (
        "boss-kill-ledge",
        "c60",
        &[("y", 3067), ("cam_y", 3067), ("cam_x", 7367)],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

// ----------------------------------------------------------------------
// The reference runs

/// The state a run's number for RedTail's state stands for, in this
/// build's numbers.
fn reference_state(n: i32) -> Option<i32> {
    let s = match n {
        2004 => EnemyState::Dead,
        2018 => EnemyState::Home,
        2022 => EnemyState::FastAdvance,
        2024 => EnemyState::HitLow,
        2025 => EnemyState::HitHigh,
        2028 => EnemyState::Startup,
        2040 => EnemyState::JumpUp,
        2041 => EnemyState::JumpDown,
        2042 => EnemyState::KnifeAttack,
        2043 => EnemyState::KickAttack,
        2044 => EnemyState::SlideAttack,
        2045 => EnemyState::GunAttack,
        2046 => EnemyState::WindAttack,
        _ => return None,
    };
    Some(s as i32)
}

/// The wind's state for a run's number: waiting or blowing.
fn reference_wind_state(n: i32) -> Option<i32> {
    match n {
        2069 => Some(wind::state::IDLE),
        2110 => Some(wind::state::BLOW),
        _ => None,
    }
}

/// One object at one step: position, state (in this build's numbers) and
/// health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct At {
    x: i32,
    y: i32,
    state: Option<i32>,
    health: i32,
}

/// RedTail by step in a run's `boss.csv`: `None` on the steps without him.
fn reference_redtail(path: &Path) -> Option<Vec<(usize, Option<At>)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, st, x, y, health) = (
        col("tick")?,
        col("boss_state")?,
        col("boss_x")?,
        col("boss_y")?,
        col("boss_health")?,
    );
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let k: usize = f.get(tick)?.parse().ok()?;
        let at = (|| {
            let n: i32 = f.get(st)?.parse().ok()?;
            if n < 0 {
                return None;
            }
            Some(At {
                x: f.get(x)?.parse().ok()?,
                y: f.get(y)?.parse().ok()?,
                state: reference_state(n),
                health: f.get(health)?.parse().ok()?,
            })
        })();
        out.push((k, at));
    }
    Some(out)
}

/// The wind by step in a run's `obj-log.csv`.
fn reference_wind(path: &Path) -> Option<Vec<(usize, At)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, logic, st, x, y) = (
        col("tick")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
    );
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        if f.get(logic) != Some(&wind::NAMES[0]) {
            continue;
        }
        out.push((
            f.get(tick)?.parse().ok()?,
            At {
                x: f.get(x)?.parse().ok()?,
                y: f.get(y)?.parse().ok()?,
                state: reference_wind_state(f.get(st)?.parse().ok()?),
                health: 0,
            },
        ));
    }
    (!out.is_empty()).then_some(out)
}

// ----------------------------------------------------------------------
// Playing

fn level13() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(13)).unwrap();
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

/// The folder of the reference runs: `CLAW_REFERENCE_RUNS`, or `rt` next
/// to the reference store. With `CLAW_REFERENCE` set it must be there.
fn runs_root(reference: &Path) -> PathBuf {
    let dir = match std::env::var_os("CLAW_REFERENCE_RUNS") {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => reference
            .parent()
            .expect("the reference store has a parent folder")
            .join("rt"),
    };
    assert!(
        dir.is_dir(),
        "the reference runs are not at {}",
        dir.display()
    );
    dir
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

fn config(clock: &str, r: &Route) -> SimConfig {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    SimConfig {
        tuning: tuning.tuning(),
        seed: SEED,
        boss_health: r.boss_health,
        ..SimConfig::default()
    }
}

fn t0_ms(clock: &str) -> u64 {
    T0_MS.iter().find(|(c, _)| *c == clock).map_or(0, |p| p.1)
}

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route) -> Sim {
    let mut sim = Sim::new(level.clone(), config(clock, r));
    sim.step(TickInput::default());
    sim.place_between_ticks(WARP_WALK.0, WARP_WALK.1);
    while sim.tick() * TICK < t0_ms(clock) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(SEED));
    sim
}

/// The object of `logic` placed at `at`, once the level has it.
fn placed(sim: &Sim, logic: &str, at: (i32, i32)) -> Option<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
}

/// A watched object of the replay while it is in the update pass (the
/// original's runs list only those) and built.
fn look(sim: &Sim, id: Option<ObjectId>) -> Option<At> {
    let o = sim.objects().get(id?)?;
    if !o.is_listed() || o.state == claw_sim::object::engine_state::INIT {
        return None;
    }
    Some(At {
        x: o.x,
        y: o.y,
        state: Some(o.state),
        health: o.health,
    })
}

/// One play of a route.
struct Played {
    rows: Vec<TraceRow>,
    /// RedTail and the wind per row.
    room: Vec<[Option<At>; 2]>,
    /// The flying gem per row.
    gem: Vec<Option<(i32, i32)>>,
    completed: bool,
}

/// Plays `r` at `clock` from its time zero for `ticks` ticks.
fn play(level: &SimLevel, r: &Route, clock: &str, ticks: usize) -> Played {
    let mut sim = at_time_zero(level, clock, r);
    let input = r.ticks();
    let mut ids = [None, None];
    let mut rows = Vec::new();
    let mut room = Vec::new();
    let mut gem = Vec::new();
    let mut completed = false;
    for k in 0..ticks {
        sim.step(input.get(k).copied().unwrap_or_default());
        completed |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        if ids[0].is_none() {
            ids[0] = placed(&sim, redtail::NAMES[0], REDTAIL_AT);
        }
        if ids[1].is_none() {
            ids[1] = placed(&sim, wind::NAMES[0], WIND_AT);
        }
        room.push([look(&sim, ids[0]), look(&sim, ids[1])]);
        gem.push(
            sim.objects()
                .all()
                .find(|(_, o)| &*o.logic == pickup::END_GEM)
                .map(|(_, o)| (o.x, o.y)),
        );
    }
    Played {
        rows,
        room,
        gem,
        completed,
    }
}

/// The step on which a measuring run set RedTail's health (its
/// `meta.txt` says so): its `boss.csv` shows the new health there while
/// its trace still shows his own, as this build's play does.
fn health_set_step(run: &Path) -> Option<usize> {
    let meta = std::fs::read_to_string(run.join("meta.txt")).ok()?;
    meta.lines()
        .find_map(|l| l.strip_prefix("boss_health_set_step="))
        .and_then(|v| v.trim().parse().ok())
}

/// The first step (as a reference time, ms) at which RedTail's x, y,
/// state or health, or the wind's state, leaves the run's; RedTail's
/// health is not compared on the step `set_at`, on which a measuring run
/// set it.
fn compare_room(
    played: &Played,
    redtail_ref: &[(usize, Option<At>)],
    wind_ref: &[(usize, At)],
    set_at: Option<usize>,
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: &str, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name.to_string(), (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in redtail_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        match (want, row[0]) {
            (None, None) => {}
            (Some(want), Some(got)) => {
                if got.x != want.x {
                    note("redtail_x", k);
                }
                if got.y != want.y {
                    note("redtail_y", k);
                }
                if want.state.is_none() || got.state != want.state {
                    note("redtail_state", k);
                }
                if got.health != want.health && set_at != Some(k) {
                    note("redtail_health", k);
                }
            }
            _ => {
                for c in ["redtail_x", "redtail_y", "redtail_state", "redtail_health"] {
                    note(c, k);
                }
            }
        }
    }
    for &(k, want) in wind_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        let same = row[1].is_some_and(|got| want.state.is_some() && got.state == want.state);
        if !same {
            note("wind_state", k);
        }
    }
    firsts
}

/// With `CLAW_L13B_DUMP=FROM..TO`, prints Claw and RedTail of the play
/// and of the run on those rows, for looking into a divergence.
fn dump(played: &Played, reference: &[TraceRow], redtail_ref: &[(usize, Option<At>)]) {
    let Some((a, b)) = std::env::var("CLAW_L13B_DUMP").ok().and_then(|v| {
        let (a, b) = v.split_once("..")?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }) else {
        return;
    };
    for k in a..b.min(played.room.len()) {
        let ours = &played.rows[k];
        let theirs = reference.get(k);
        println!(
            "row {k}: claw ours {} {} s{} h{} rng {:?} | ref {:?}",
            ours.x,
            ours.y,
            ours.state_id,
            ours.health,
            ours.rng,
            theirs.map(|t| (t.x, t.y, t.state_id, t.health, t.rng))
        );
        let rt = redtail_ref.iter().find(|r| r.0 == k).and_then(|r| r.1);
        println!(
            "  redtail ours {:?} ref {:?} | wind {:?} | gem {:?} | cam {} {} ref {:?}",
            played.room[k][0],
            rt,
            played.room[k][1],
            played.gem[k],
            ours.cam_x,
            ours.cam_y,
            theirs.map(|t| (t.cam_x, t.cam_y))
        );
    }
}

/// The divergences of a play of `store` at `clock` not in [`known`].
fn new_divergences(
    store: &str,
    clock: &str,
    played: &Played,
    reference: &[TraceRow],
    room: Vec<(String, u64)>,
) -> Vec<String> {
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(&played.rows, reference, &tol);
    print!("{}", trace::report(&played.rows, reference, &cmp, 3));
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
    firsts.extend(room);
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

/// A store's own copy of its route: (held, pressed) by row, and its boss
/// health line.
type StoreRoute = (Vec<(u32, u32)>, Option<i32>);

/// The store's own copy of its route (see [`StoreRoute`]).
fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    for l in text.lines() {
        if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    let health = claw_sim::input::script_boss_health(&text).ok()?;
    Some((rows, health))
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for (store, clocks) in STORES {
        let dir = root.join("level13").join(store);
        let (rows, health) =
            store_route(&dir).unwrap_or_else(|| panic!("{store}: no route in the store"));
        let r = route(store);
        let ours = r.rows();
        assert_eq!(ours.len(), rows.len(), "{store}: rows");
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{store}: first different row");
        assert_eq!(r.boss_health, health, "{store}: the boss health line");
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        assert_eq!(
            meta_value(&meta, "", "diagnostic").as_deref() == Some("true"),
            r.boss_health.is_some(),
            "{store}: only the kill routes measure"
        );
        assert_eq!(
            meta_value(&meta, "", "completed").as_deref() == Some("yes"),
            r.completes,
            "{store}: the level's end"
        );
        for &clock in clocks {
            let teleport = meta_value(&meta, clock, "teleport")
                .unwrap_or_else(|| panic!("{store}: no start for {clock}"));
            let x: i32 = teleport.split(',').next().unwrap().parse().unwrap();
            assert_eq!(x, WARP_WALK.0, "{store}: start");
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_redtail_routes_follow_the_reference_traces() {
    let Some(level) = level13() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L13B_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for (store, clocks) in STORES {
        if only.as_deref().is_some_and(|o| o != store) {
            continue;
        }
        let dir = root.join("level13").join(store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        let r = route(store);
        for &clock in clocks {
            println!("== level13/{store} {clock}");
            let t0: u64 = meta_value(&meta, clock, "t0_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{store}: no time zero for {clock}"));
            assert_eq!(t0, t0_ms(clock), "{store}: time zero");
            let seed: u32 = meta_value(&meta, clock, "rng_at_t0")
                .or_else(|| meta_value(&meta, "", "rng_at_t0"))
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{store}: no random state"));
            assert_eq!(seed, SEED, "{store}: random state");
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let reference = trace::parse(&text).unwrap();
            let played = play(&level, &r, clock, reference.len());
            if let Some(out) = std::env::var_os("CLAW_L13B_TRACE") {
                let out = PathBuf::from(out).join(format!("{store}-{clock}.csv"));
                std::fs::write(&out, trace::to_csv(&played.rows)).expect("the trace is written");
            }
            let mut room = Vec::new();
            if clock == "c18" {
                let id = run_id(&meta, clock)
                    .unwrap_or_else(|| panic!("{store}: no run named in its meta"));
                let run = runs.join(id);
                let boss = reference_redtail(&run.join("boss.csv"))
                    .unwrap_or_else(|| panic!("{store}: the run has no boss.csv"));
                let wind = reference_wind(&run.join("obj-log.csv"))
                    .unwrap_or_else(|| panic!("{store}: the run's obj-log.csv has no wind"));
                let set_at = health_set_step(&run);
                assert_eq!(
                    set_at.is_some(),
                    r.boss_health.is_some(),
                    "{store}: the run's health set-up"
                );
                room = compare_room(&played, &boss, &wind, set_at);
                dump(&played, &reference, &boss);
            } else {
                dump(&played, &reference, &[]);
            }
            if r.completes != played.completed {
                failures.push(format!(
                    "{store} {clock}: the level's end {}",
                    if r.completes {
                        "does not come"
                    } else {
                        "comes"
                    }
                ));
            }
            failures.extend(new_divergences(store, clock, &played, &reference, room));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ----------------------------------------------------------------------
// The fight's outcomes, on this build's own plays of the routes

/// What one row of a play shows.
#[derive(Debug, Clone, PartialEq)]
struct Look {
    claw: (i32, i32, i32),
    claw_health: i32,
    push: f64,
    fight: u8,
    redtail: Option<At>,
    /// RedTail's attack box, relative to him.
    redtail_box: Option<Rect>,
    hits: Option<u32>,
    pending: Option<u8>,
    link: redtail::RedTailLink,
    wind: Option<i32>,
    /// The flares: where each is and its state.
    flares: Vec<(i32, i32, i32)>,
    /// The pieces of debris there are.
    debris: usize,
    /// The gem: where it is, and whether it still flies.
    gem: Option<(i32, i32, bool)>,
    ended: bool,
}

/// Plays `rows` rows of `store`'s route at `clock` from its time zero,
/// with the original's random state there.
fn watch(level: &SimLevel, store: &str, clock: &str, rows: usize) -> Vec<Look> {
    let r = route(store);
    let mut sim = at_time_zero(level, clock, &r);
    let input = r.ticks();
    let mut out = Vec::new();
    let mut ids = [None, None];
    for k in 0..rows {
        sim.step(input.get(k).copied().unwrap_or_default());
        if ids[0].is_none() {
            ids[0] = placed(&sim, redtail::NAMES[0], REDTAIL_AT);
        }
        if ids[1].is_none() {
            ids[1] = placed(&sim, wind::NAMES[0], WIND_AT);
        }
        let c = sim.player().expect("Claw");
        let gem = sim
            .objects()
            .all()
            .find(|(_, o)| &*o.logic == pickup::END_GEM || o.logic.contains("EndOfLevel"))
            .map(|(_, o)| (o.x, o.y, &*o.logic == pickup::END_GEM));
        let own = ids[0].and_then(|id| match &sim.objects().get(id)?.data {
            claw_sim::logic::ObjectData::RedTail(d) => Some((d.hits, d.pending)),
            _ => None,
        });
        let flares = sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == flare::NAMES[0] && o.is_listed())
            .map(|(_, o)| (o.x, o.y, o.state))
            .collect();
        let debris = sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == debris::NAMES[0])
            .count();
        out.push(Look {
            claw: (c.x, c.y, c.state),
            claw_health: sim.claw().stats.health,
            push: sim.claw().conveyor_force,
            fight: sim.boss().fight(),
            redtail: look(&sim, ids[0]),
            redtail_box: ids[0]
                .and_then(|id| sim.objects().get(id))
                .map(|o| o.attack_rect),
            hits: own.map(|o| o.0),
            pending: own.map(|o| o.1),
            link: *sim.redtail_link(),
            wind: look(&sim, ids[1]).and_then(|a| a.state),
            flares,
            debris,
            gem,
            ended: sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
        });
    }
    out
}

fn st(l: &Look) -> Option<EnemyState> {
    l.redtail
        .and_then(|a| a.state)
        .and_then(EnemyState::from_id)
}

/// The rows on which RedTail's state turns to `s`.
fn starts(rows: &[Look], s: EnemyState) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| st(&rows[k]) == Some(s) && (k == 0 || st(&rows[k - 1]) != Some(s)))
        .collect()
}

/// The rows on which Claw's health drops, with what he lost.
fn hurts(rows: &[Look]) -> Vec<(usize, i32)> {
    (1..rows.len())
        .filter(|&k| rows[k].claw_health < rows[k - 1].claw_health)
        .map(|k| (k, rows[k - 1].claw_health - rows[k].claw_health))
        .collect()
}

#[test]
fn install_redtail_the_staging_ends_with_his_first_decision_28_steps_after_the_thaw() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "boss-intro-normal", "c18", 760);
    // The first arena step N: built on the ledge, waiting, the count at 1.
    let n = rows
        .iter()
        .position(|l| l.redtail.is_some())
        .expect("built");
    assert_eq!(n, 30);
    assert_eq!(st(&rows[n]), Some(EnemyState::Startup));
    assert_eq!(rows[n].hits, Some(redtail::HIT_COUNTER_START));
    assert_eq!(rows[n].redtail.map(|a| (a.x, a.y)), Some((37431, 1889)));
    assert!(!rows[n].link.wind);
    // The 613-step script: the flag 1 on N + 1, 2 on N + 614; he and Claw
    // start on N + 615, the count at 0.
    assert_eq!(rows.iter().position(|l| l.fight == 1), Some(n + 1));
    assert_eq!(rows.iter().position(|l| l.fight == 2), Some(n + 614));
    assert_eq!(st(&rows[n + 614]), Some(EnemyState::Startup));
    assert_eq!(st(&rows[n + 615]), Some(EnemyState::Home));
    assert_eq!(rows[n + 615].claw.2, claw_state::STAND);
    assert_eq!(rows[n + 614].claw.2, claw_state::FREEZE);
    assert_eq!(rows[n + 615].hits, Some(0));
    // The reset put him a little above the ledge: he falls back onto it.
    assert_eq!(rows[n + 615].redtail.map(|a| a.y), Some(1885));
    assert_eq!(rows[n + 619].redtail.map(|a| a.y), Some(1889));
    // His first decision 28 steps after the thaw: the jump down for the
    // gun, landing 27 steps later; the gun on the next step.
    assert_eq!(starts(&rows, EnemyState::JumpDown), vec![n + 615 + 28]);
    assert_eq!(rows[673].pending, Some(2));
    assert_eq!(rows[700].redtail.map(|a| (a.x, a.y)), Some((37526, 2086)));
    assert_eq!(starts(&rows, EnemyState::GunAttack), vec![701]);
}

#[test]
fn install_redtail_his_gun_slide_kick_and_body_hurt_a_standing_claw() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "boss-intro-normal", "c18", 1180);
    // The gun: 35 steps; the flare leaves on its 14th, 66 px in front of
    // him and 9 above, and flies 9 px a step until it bursts on Claw.
    let gun = starts(&rows, EnemyState::GunAttack);
    assert_eq!(gun[..2], [701, 792]);
    assert_eq!(st(&rows[735]), Some(EnemyState::GunAttack));
    assert_eq!(st(&rows[736]), Some(EnemyState::Home));
    assert!(rows[713].flares.is_empty());
    assert_eq!(rows[714].flares, vec![(37460, 2077, flare::state::FLYING)]);
    assert_eq!(rows[715].flares[0].0, 37451);
    assert_eq!(rows[742].flares[0].0, 37208);
    assert_eq!(rows[743].flares[0].2, flare::state::EXPLODING);
    // The hits Claw takes: two flares (15), the slide and his body (10),
    // the stakes the flares knocked him into (20).
    let h = hurts(&rows);
    assert_eq!(h[0], (743, 15));
    assert_eq!(h[1], (837, 15));
    assert!(h.contains(&(933, 10)), "the slide: {h:?}");
    assert!(h.contains(&(1029, 10)), "his body: {h:?}");
    assert_eq!(h.last(), Some(&(1077, 10)));
    assert_eq!(rows[1077].claw.2, claw_state::DEATH_FALL);
    // The slide: 5 px a step, its box from its ninth step.
    let slide = starts(&rows, EnemyState::SlideAttack);
    assert_eq!(slide[0], 916);
    let x = |k: usize| rows[k].redtail.map(|a| a.x).unwrap();
    assert_eq!(x(917) - x(918), 5);
    let mirrored_or_not = |k: usize| {
        rows[k]
            .redtail_box
            .is_some_and(|r| r == redtail::SLIDE_RECT || r == redtail::SLIDE_RECT.mirrored())
    };
    assert!(!mirrored_or_not(923));
    assert!(mirrored_or_not(924));
    // Ended against the wall two steps after he stopped; entered beside
    // Claw it lasts one step.
    assert_eq!(st(&rows[939]), Some(EnemyState::SlideAttack));
    assert_eq!(st(&rows[940]), Some(EnemyState::Home));
    assert!(slide.contains(&968));
    assert_eq!(st(&rows[969]), Some(EnemyState::Home));
    // The kick: 19 steps.
    let kick = starts(&rows, EnemyState::KickAttack);
    assert_eq!(kick[0], 997);
    assert_eq!(st(&rows[1015]), Some(EnemyState::KickAttack));
    assert_eq!(st(&rows[1016]), Some(EnemyState::Home));
    // Claw's respawn puts him back on the ledge at full health.
    assert_eq!(st(&rows[1172]), Some(EnemyState::Home));
    assert_eq!(rows[1172].redtail.map(|a| (a.x, a.y)), Some((37431, 1886)));
    assert_eq!(rows[1172].claw.2, claw_state::STAND);
}

#[test]
fn install_redtail_his_wind_pushes_claw_into_the_stakes() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "boss-intro-normal", "c18", 1520);
    // He raises the flag on S; the wind blows from S, pushes from S + 1
    // and Claw moves with it from S + 2.
    let s = starts(&rows, EnemyState::WindAttack)[0];
    assert_eq!(s, 1227);
    assert!(rows[s].link.wind && !rows[s - 1].link.wind);
    assert_eq!(rows[s].wind, Some(wind::state::BLOW));
    assert_eq!(rows[s].push, 0.0);
    assert_eq!(rows[s + 1].push, wind::PUSH);
    assert_eq!(rows[s + 1].claw.0, 37180);
    for k in s + 2..s + 8 {
        assert_eq!(rows[k - 1].claw.0 - rows[k].claw.0, 5, "row {k}");
    }
    // A piece of debris every 6 steps from S + 1.
    for k in s + 1..s + 120 {
        let made = rows[k].debris > rows[k - 1].debris;
        assert_eq!(made, (k - s - 1).is_multiple_of(6), "row {k}");
    }
    // The stakes: 20 every 48 steps while the wind holds him there.
    let stakes: Vec<usize> = hurts(&rows)
        .into_iter()
        .filter(|&(k, d)| k > s && d == 20)
        .map(|(k, _)| k)
        .collect();
    assert_eq!(stakes, vec![1236, 1284, 1332, 1380, 1428]);
    // 278 steps of wind; its end lowers the flag.
    assert_eq!(rows[s + 277].wind, Some(wind::state::BLOW));
    assert_eq!(rows[s + 278].wind, Some(wind::state::IDLE));
    assert!(rows[s + 277].link.wind && !rows[s + 278].link.wind);
    // His next decision 122 steps after the wind's end.
    assert_eq!(st(&rows[s + 18]), Some(EnemyState::Home));
    assert_eq!(starts(&rows, EnemyState::JumpDown).last(), Some(&1367));
}

#[test]
fn install_redtail_walking_or_jumping_into_the_wind() {
    let Some(level) = level13() else { return };
    // Walking right from S + 2: speed -75, losing 1 or 2 px a step until
    // the stakes catch him; then he is knocked away and held, not pushed.
    let rows = watch(&level, "boss-wind-walk-c18", "c18", 1300);
    for k in 1230..1250 {
        let d = rows[k - 1].claw.0 - rows[k].claw.0;
        assert!((1..=2).contains(&d), "row {k}: {d}");
    }
    assert_eq!(hurts(&rows).iter().find(|h| h.0 > 1227), Some(&(1250, 20)));
    // A full jump on S + 2: 5 or 6 px a step left in the air, the stakes
    // meet him there.
    let rows = watch(&level, "boss-wind-jump-c18", "c18", 1300);
    for k in 1230..1235 {
        let d = rows[k - 1].claw.0 - rows[k].claw.0;
        assert!((5..=6).contains(&d), "row {k}: {d}");
    }
    assert_eq!(hurts(&rows).iter().find(|h| h.0 > 1227), Some(&(1235, 20)));
    assert_eq!(rows[1235].claw.2, claw_state::HIT);
}

#[test]
fn install_redtail_the_intro_skip_leaves_him_waiting_with_his_count_at_one() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "boss-intro-skip-c18", "c18", 1800);
    assert!(rows.iter().all(|l| l.fight == 0));
    for l in &rows[30..] {
        assert_eq!(st(l), Some(EnemyState::Startup));
        assert_eq!(
            l.redtail.map(|a| (a.x, a.y, a.health)),
            Some((37431, 1889, 100))
        );
        assert_eq!(l.hits, Some(1));
        assert!(!l.link.wind);
    }
    // The skip jump lands him on the sand.
    assert_eq!(
        (rows[70].claw.0, rows[70].claw.2),
        (37258, claw_state::STAND)
    );
    // Beside him on the ledge he still waits.
    let ledge = watch(&level, "boss-skip-ledge-c18", "c18", 2000);
    assert_eq!((ledge[218].claw.0, ledge[218].claw.1), (37360, 1884));
    assert!(
        ledge[30..]
            .iter()
            .all(|l| st(l) == Some(EnemyState::Startup))
    );
    assert!(ledge.iter().all(|l| l.claw_health == 100));
    // One row late the introduction runs.
    let late = watch(&level, "boss-intro-skip-late-c18", "c18", 700);
    assert_eq!(late.iter().position(|l| l.fight == 2), Some(644));
}

#[test]
fn install_redtail_a_kill_on_the_ledge_jumps_down_dies_and_leaves_the_gem() {
    let Some(level) = level13() else { return };
    let rows = watch(&level, "boss-kill-ledge-c18", "c18", 700);
    // The measuring route's health from the step after the first arena
    // step.
    assert_eq!(rows[30].redtail.map(|a| a.health), Some(100));
    assert_eq!(rows[31].redtail.map(|a| a.health), Some(5));
    // The blow: counted (1 to 2), his health to 0, Die's jump down.
    assert_eq!((rows[228].hits, rows[229].hits), (Some(1), Some(2)));
    assert_eq!(rows[229].redtail.map(|a| a.health), Some(0));
    assert_eq!(starts(&rows, EnemyState::JumpDown), vec![229]);
    assert_eq!(rows[229].pending, Some(6));
    // Dead on the landing (26 steps of jump), the gem 136 steps later.
    assert_eq!(starts(&rows, EnemyState::Dead), vec![255]);
    assert_eq!(rows[255].redtail.map(|a| (a.x, a.y)), Some((37451, 2086)));
    let gem = rows.iter().position(|l| l.gem.is_some()).expect("the gem");
    assert_eq!(gem, 255 + 136);
    assert_eq!(rows[gem].gem, Some((37451, 2086, true)));
    // Claw held where he stood for the gem's flight of 147 steps.
    for l in &rows[gem..gem + 147] {
        assert_eq!(l.claw, (37360, 1884, BOSS_END));
    }
    assert_eq!(rows[gem + 147].claw.2, claw_state::STAND);
    assert_eq!(
        rows[gem + 147].gem.map(|g| (g.0, g.1, g.2)),
        Some((37509, 1750, false))
    );
    // He stays dead; the walk and the jump take the gem: the level ends.
    assert!(rows[gem..].iter().all(|l| st(l) == Some(EnemyState::Dead)));
    assert_eq!(rows.iter().position(|l| l.ended), Some(578));
}
