//! The sixth level's boss room on your own install: the boss room's
//! routes played against the reference traces of the same routes, in the
//! same trace format, and Wolvington's defeat, the gem it releases and the
//! jump that collects it.
//!
//! The routes: the jump into the door with the whole introduction, then
//! Claw standing, crouching, fighting, lifting him, or ending a life in
//! the middle of the fight; the jump-in entry that skips the introduction
//! with Jump held 2, 12, 13, 16 or 20 ticks, with and without Catnip, one
//! tick late, straight up, and then the fight, the lift and the walk back
//! into the stager's box after it.
//!
//! Each route starts under the door into his room: Claw is put there on
//! the level's first tick and left there until the route's time zero (828
//! ms of game time at the original speeds, 817 ms at the default ones, as
//! in the reference runs, so the frame pulse and the level's timed sounds
//! are in the same phase), then the random stream is set to the store's
//! state at time zero, and a route that asks for it gives Claw Catnip for
//! its time. The game then plays the route tick by tick and its trace is
//! compared with each reference trace of the route's clocks: tick for
//! tick, the random stream included, at the original speeds (c18); within
//! the feel tolerances at the default speeds (c60). On every respawn of
//! Claw at c18 he must be back on his floor, at most 16 px left of his
//! place, on that same step: the respawn's update passes still move him
//! here, where in the original they take no game time.
//!
//! When the reference runs also list Wolvington's position, state and
//! health per step (`boss.csv` beside each run, in the folder named by
//! `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference store), his
//! are compared too: exactly at c18, positions within 8 px and states and
//! health within two ticks at c60.
//!
//! Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparison; each test
//! skips itself when what it needs is unset; `CLAW_L6_ONLY` names one store
//! to play alone. Nothing from an install or a reference store is kept in
//! the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, GameEvent};
use claw_sim::activation::Centres;
use claw_sim::input::{DirectiveKind, ScriptCommand, ScriptTuning};
use claw_sim::logic::enemy::{EnemyState, wolvington};
use claw_sim::logic::pickup;
use claw_sim::logic::world_boss::{AMULET_SOUND, fight};
use claw_sim::object::{LogicKind, ObjectId, flags};
use claw_sim::player::death::{CATNIP_GLITTER_LOGIC, powerup};
use claw_sim::player::powerup_aid::AidPowerup;
use claw_sim::player::special::BOSS_END;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::{Buttons, GameLogic, InputScript, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Where every route starts: under the door into his room.
const START: (i32, i32) = (30685, 4175);
/// Wolvington on his floor at his place.
const HIM_AT: (i32, i32) = (34960, 4497);
/// His gem's target, from his level record.
const GEM_TARGET: (i32, i32) = (34909, 4330);
/// The random state of every store at its time zero.
const SEED: u32 = 111_567;

const ATTACK: u32 = 0x2;

/// Random draws the original's teleport makes before the objects around
/// its target wake: its screen transition draws once per row it plays.
/// The door's timed sounds (seagulls, a carriage, a church bell) wake
/// right after and draw their first timers, which last past the route's
/// time zero, so the replay makes the same draws at the teleport.
const TELEPORT_DRAWS: u32 = 10;

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

/// Claw put at `at` on the level's first tick and left to settle until
/// `t0_ms` of game time has passed since the level started (as in the
/// reference run), then the random stream set to `seed`: the next tick is
/// the route's time zero.
///
/// With `catnip_ms`, Claw has Catnip for that time from the route's time
/// zero, given between the two ticks as a pickup gives it (its tune, its
/// line and its glitter): the glitter sheds its first sparkles in the
/// route's second tick, as in the reference runs. The replay gives it in
/// the last tick before time zero, then puts the time back and holds the
/// glitter for one tick.
fn start_at(sim: &mut Sim, at: (i32, i32), t0_ms: u64, seed: u32, catnip_ms: Option<u32>) {
    sim.teleport_player(at.0, at.1);
    sim.with_world(|w| {
        for _ in 0..TELEPORT_DRAWS {
            w.rng.next_u15();
        }
    });
    let pre = t0_ms.div_ceil(TICK);
    for k in 0..pre {
        if k + 1 == pre
            && let Some(ms) = catnip_ms
        {
            sim.give_powerup(AidPowerup {
                kind: powerup::CATNIP,
                ms,
            });
        }
        sim.step(TickInput::default());
    }
    sim.with_world(|w| {
        *w.rng = Rng::new(seed);
        if let Some(ms) = catnip_ms {
            w.claw.stats.powerup_ms = ms;
            let glitter: Vec<ObjectId> = w
                .objects
                .all()
                .filter(|(_, o)| &*o.logic == CATNIP_GLITTER_LOGIC)
                .map(|(id, _)| id)
                .collect();
            for id in glitter {
                if let Some(o) = w.objects.get_mut(id) {
                    o.frame_delay = 1;
                }
            }
        }
    });
}

/// What the original does as Claw goes through the door, before his first
/// tick in the room, that this build does not do yet: the objects he left
/// behind outside their windows go to sleep at once (this build puts them
/// to sleep after that first tick, so the guards by the door would draw
/// their tick's rolls in it). The replay holds those for one call when
/// Claw is about to go through the door. The effects already running (the
/// Catnip glitter) have no call in the door's passes, which the build does
/// itself: the shedding glitter waits out its time delay in those passes,
/// which take no game time, so they leave it alone, and it is not held.
fn through_the_door(sim: &mut Sim) {
    let Some(target) = sim.claw().special.warp_to else {
        return;
    };
    let cfg = sim.activation().config().clone();
    let c = Centres::new(&cfg, sim.level().plane(), target);
    let held: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.is_listed())
        .filter(|(_, o)| {
            if o.kind == LogicKind::Game(GameLogic::Effect) {
                return false;
            }
            if o.flags & flags::WORLD == 0 || o.flags & flags::ALWAYS_ACTIVE != 0 {
                return false;
            }
            let (hx, hy) = cfg.half_extents(o.activation_class(), cfg.view);
            !((c.main.0 - hx..=c.main.0 + hx).contains(&o.x)
                && (c.main.1 - hy..=c.main.1 + hy).contains(&o.y))
        })
        .map(|(id, _)| id)
        .collect();
    sim.with_world(|w| {
        for id in held {
            if let Some(o) = w.objects.get_mut(id) {
                o.frame_delay = 1;
            }
        }
    });
}

/// A value of the meta's section for `clock`.
fn clock_value<'a>(meta: &'a str, clock: &str, key: &str) -> Option<&'a str> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let section = section.split("\n[").next()?;
    meta_value(section, key)
}

fn meta_value<'a>(meta: &'a str, key: &str) -> Option<&'a str> {
    meta.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"'))
}

/// The first run named in the meta's section for `clock`.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let section = meta.split(&format!("\n[{clock}]")).nth(1)?;
    let line = section
        .lines()
        .find(|l| l.trim_start().starts_with("run_ids"))?;
    let first = line.split('"').nth(1)?;
    Some(first.to_string())
}

/// The Catnip a route gives Claw at its time zero, ms (its `catnip_ms`
/// header line).
fn route_catnip(route: &str) -> Option<u32> {
    route
        .lines()
        .filter_map(|l| l.strip_prefix('#'))
        .filter_map(|l| l.trim().strip_prefix("catnip_ms"))
        .find_map(|v| v.trim().parse().ok())
}

/// Wolvington on one tick: x, y, state and health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Him {
    x: i32,
    y: i32,
    state: EnemyState,
    health: i32,
}

fn find_him(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| wolvington::NAMES.contains(&&*o.logic))
        .map(|(id, _)| id)
}

fn him(sim: &Sim) -> Option<Him> {
    let id = find_him(sim)?;
    let o = sim.objects().get(id)?;
    Some(Him {
        x: o.x,
        y: o.y,
        state: EnemyState::from_id(o.state)?,
        health: o.health,
    })
}

/// The product state a reference run's state number stands for. The runs
/// number his shared states their own way; his own states are the ones in
/// [`wolvington::REFERENCE_STATE_IDS`].
fn state_numbered(id: i32) -> Option<EnemyState> {
    if let Some(&(state, _)) = wolvington::REFERENCE_STATE_IDS
        .iter()
        .find(|(_, n)| *n == id)
    {
        return Some(state);
    }
    Some(match id {
        2000 => EnemyState::Idle,
        2004 => EnemyState::Dead,
        2008 => EnemyState::Bounce,
        2009 => EnemyState::Lifted,
        2010 => EnemyState::Thrown,
        2011 => EnemyState::Pause,
        2013 => EnemyState::MoveBack,
        2017 => EnemyState::Flailing,
        2020 => EnemyState::JumpBack,
        2022 => EnemyState::FastAdvance,
        2023 => EnemyState::Pace,
        2024 => EnemyState::HitLow,
        2025 => EnemyState::HitHigh,
        _ => return None,
    })
}

/// Wolvington per step of a reference run: (t_ms, him), from its
/// `boss.csv`.
fn reference_him(path: &Path) -> Option<Vec<(u64, Option<Him>)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let (t, name, x, y, health) = (
        col("t_ms")?,
        col("boss_state")?,
        col("boss_x")?,
        col("boss_y")?,
        col("boss_health")?,
    );
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        let t_ms: u64 = cells.get(t)?.parse().ok()?;
        let him = (|| {
            Some(Him {
                x: cells.get(x)?.parse().ok()?,
                y: cells.get(y)?.parse().ok()?,
                state: state_numbered(cells.get(name)?.parse().ok()?)?,
                health: cells.get(health)?.parse().ok()?,
            })
        })();
        rows.push((t_ms, him));
    }
    Some(rows)
}

/// One route of the store played in the game.
struct Played {
    rows: Vec<TraceRow>,
    him: Vec<Option<Him>>,
}

/// Plays the store's route at `clock` for as long as `until_ms`, from
/// [`START`] with the store's random state at time zero.
fn play(
    level: &SimLevel,
    route: &str,
    (seed, t0_ms): (u32, u64),
    clock: &str,
    until_ms: u64,
) -> Played {
    let script = InputScript::parse(route).unwrap();
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
    start_at(&mut sim, START, t0_ms, seed, route_catnip(route));
    let ticks = script.ticks.len().max(until_ms.div_ceil(TICK) as usize);
    let mut directives = script.directives().iter().peekable();
    let mut rows = Vec::new();
    let mut hims = Vec::new();
    for tick in 0..ticks {
        while let Some(d) = directives.next_if(|d| d.tick <= tick) {
            if let DirectiveKind::Command(ScriptCommand::EndLife) = d.kind {
                sim.end_life();
            }
        }
        sim.step(script.ticks.get(tick).copied().unwrap_or_default());
        through_the_door(&mut sim);
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
        hims.push(him(&sim));
    }
    Played { rows, him: hims }
}

/// The first reference time (ms) Wolvington's x, y, state and health leave
/// the reference run's, each by column: exact for c18; for c60 positions
/// within 8 px and states and health within two ticks of the reference.
fn compare_him(
    played: &Played,
    reference: &[(u64, Option<Him>)],
    clock: &str,
) -> BTreeMap<&'static str, u64> {
    let exact = clock == "c18";
    let at = |t: u64| -> Option<Option<Him>> {
        let i = (t.saturating_sub(1) / TICK) as usize;
        played.him.get(i).copied()
    };
    let near = |t: u64, f: &dyn Fn(Option<Him>) -> bool| -> bool {
        if exact {
            return at(t).is_some_and(f);
        }
        (0..=4).any(|k| {
            let s = t.saturating_sub(2 * TICK) + TICK * k;
            at(s).is_some_and(f)
        })
    };
    let mut first: BTreeMap<&'static str, u64> = BTreeMap::new();
    for &(t, r) in reference {
        let Some(mine) = at(t) else { break };
        let pos_ok = |a: i32, b: i32| if exact { a == b } else { (a - b).abs() <= 8 };
        let checks: [(&'static str, bool); 5] = match (r, mine) {
            (None, None) => [("present", true); 5],
            (Some(r), Some(m)) => [
                ("present", true),
                (
                    "his_x",
                    pos_ok(r.x, m.x)
                        || (!exact && near(t, &|h| h.is_some_and(|h| pos_ok(h.x, r.x)))),
                ),
                (
                    "his_y",
                    pos_ok(r.y, m.y)
                        || (!exact && near(t, &|h| h.is_some_and(|h| pos_ok(h.y, r.y)))),
                ),
                (
                    "his_state",
                    near(t, &|h| h.is_some_and(|h| h.state == r.state)),
                ),
                (
                    "his_health",
                    near(t, &|h| h.is_some_and(|h| h.health == r.health)),
                ),
            ],
            _ => [("present", false); 5],
        };
        for (name, ok) in checks {
            if !ok {
                first.entry(name).or_insert(t);
            }
        }
    }
    first
}

/// The first divergence of every column of one comparison, by column.
fn trace_firsts(cmp: &Comparison) -> BTreeMap<String, u64> {
    cmp.per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.clone(), t)))
        .collect()
}

/// The boss room's reference routes with the clocks each is compared at:
/// every route at the original speeds, and the introduction, the skip,
/// the skip with one more tick of Jump, the skip with Catnip and the lift
/// at the default speeds too.
const STORES: &[(&str, &[&str])] = &[
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-intro-normal-duck", &["c18"]),
    ("boss-intro-normal-fight", &["c18"]),
    ("boss-intro-normal-lift", &["c18", "c60"]),
    ("boss-intro-normal-end-life", &["c18"]),
    ("boss-intro-skip", &["c18", "c60"]),
    ("boss-intro-skip-tap", &["c18"]),
    ("boss-intro-skip-j13", &["c18", "c60"]),
    ("boss-intro-skip-j16", &["c18"]),
    ("boss-intro-skip-full", &["c18"]),
    ("boss-intro-skip-catnip", &["c18", "c60"]),
    ("boss-intro-skip-catnip-full", &["c18"]),
    ("boss-intro-skip-catnip-j7", &["c18"]),
    ("boss-intro-skip-catnip-tap", &["c18"]),
    ("boss-intro-skip-late", &["c18"]),
    ("boss-intro-skip-up", &["c18"]),
    ("boss-intro-skip-return", &["c18"]),
    ("boss-intro-skip-fight", &["c18"]),
    ("boss-intro-skip-lift", &["c18"]),
];

/// A store, a clock, and the first divergence (ms) of each column known
/// there; a column not listed must stay within its tolerance, and one
/// listed must not leave it earlier. What they come from:
///
/// - c18, every route: the door, the whole introduction, his fight
///   (walks, runs, sprints, uppercuts, thrusts, volleys and their shots,
///   hops back, flinches), lifting and throwing him, the jump-in skip and
///   the 40 px band that decides whether he wakes (asleep with Jump held 2
///   or 12 ticks and with Catnip held 2 or 7, awake for 13, 16 and 20 and
///   with Catnip for 12 and 20), the late trigger, the walk back into the
///   stager's box and the fight and the lift after a skip follow the
///   original step for step, the random stream included; so does Claw
///   standing still for 17 s after a skip that leaves him asleep, to the
///   draw of Claw's idle line when his idle animation ends (18018 ms).
/// - c18 end of life: the in-game menu opened to end the life puts the
///   original's frame pulse a tick later from there on (18018 ms; the
///   replay plays no time for the menu), Claw's death fall is a pixel off
///   from 18900 ms, and on the tick after the respawn Wolvington stands a
///   pixel left of the original's (19710 ms: 34957 against 34958, where
///   he is put back; the respawn's passes no longer move him, but this
///   build's tick still does, by a pixel).
/// - c18 Catnip held 20 ticks: he wakes in the air, fights, and kills
///   Claw; Claw's death fall, his respawn and Wolvington's place after it
///   follow the original's, but the fight flag of the introduction Claw
///   runs again by walking into the stager's box reaches 2 two ticks
///   before the original's (23094 ms; not traced yet).
/// - c60 (the default tuning played tick by tick): the camera pans to
///   him at 7 px per 18 ms tick against 6 px per frame (850 ms), and the
///   introduction's delays count 18 ms ticks against frames of 16 and 17
///   ms, so the flag reaches 2 and he wakes about 250 ms later (12617
///   ms); the jump-in's arc is 9 to 12 px lower (667 ms), so Jump held 13
///   ticks leaves him asleep at this tuning (950 ms) while 16 and 20 and
///   Catnip with 12 wake him as in the original, and Claw comes back into
///   his band, and so he comes Home, a few ticks earlier (1033 ms).
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[
    (
        "boss-intro-normal-end-life",
        "c18",
        &[
            ("rng", 18018),
            ("y", 18900),
            ("cam_x", 19710),
            ("his_state", 19710),
            ("his_x", 19710),
            ("health", 21600),
            ("state_id", 21600),
            ("x", 21618),
        ],
    ),
    (
        "boss-intro-skip-catnip-full",
        "c18",
        &[("boss_fight", 23094)],
    ),
    (
        "boss-intro-normal",
        "c60",
        &[
            ("cam_x", 850),
            ("boss_fight", 12617),
            ("his_state", 12633),
            ("state_id", 12633),
            ("his_x", 12700),
            ("health", 13500),
            ("x", 13550),
        ],
    ),
    (
        "boss-intro-normal-lift",
        "c60",
        &[
            ("cam_x", 850),
            ("boss_fight", 12617),
            ("his_state", 12633),
            ("state_id", 12633),
            ("his_x", 12700),
            ("health", 13500),
            ("x", 13550),
            ("his_y", 13983),
            ("physics_type", 15000),
            ("boss_health", 16183),
            ("his_health", 16183),
        ],
    ),
    ("boss-intro-skip", "c60", &[("y", 667), ("cam_y", 667)]),
    (
        "boss-intro-skip-j13",
        "c60",
        &[
            ("y", 683),
            ("cam_y", 683),
            ("his_state", 950),
            ("his_x", 1000),
            ("health", 1417),
            ("state_id", 1417),
            ("cam_x", 1450),
            ("x", 1450),
            ("events", 9400),
            ("boss_fight", 9417),
        ],
    ),
    (
        "boss-intro-skip-catnip",
        "c60",
        &[
            ("y", 700),
            ("cam_y", 700),
            ("his_state", 1033),
            ("physics_type", 1083),
            ("state_id", 1083),
            ("his_x", 1283),
            ("health", 1467),
            ("cam_x", 1500),
            ("x", 1500),
            ("events", 9432),
            ("boss_fight", 9483),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

#[test]
fn install_level6_boss_room_against_the_reference_traces() {
    let Some(level) = level6() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    let only = std::env::var("CLAW_L6_ONLY").ok();
    let mut failures = Vec::new();
    for &(store, clocks) in STORES {
        if only.as_deref().is_some_and(|o| o != store) {
            continue;
        }
        let dir = root.join("level6").join(store);
        let Ok(meta) = std::fs::read_to_string(dir.join("meta.toml")) else {
            println!("== level6/{store}: no store, skipped");
            continue;
        };
        let seed: u32 = meta_value(&meta, "rng_at_t0").map_or(SEED, |v| v.parse().unwrap());
        let route = std::fs::read_to_string(dir.join("route.csv")).unwrap();
        for &clock in clocks {
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let Ok(text) = std::fs::read_to_string(&path) else {
                println!("== level6/{store} {clock}: no trace, skipped");
                continue;
            };
            let reference = trace::parse(&text).unwrap();
            let until = reference.last().map_or(0, |r| r.t_ms);
            let t0_ms: u64 = clock_value(&meta, clock, "t0_ms").unwrap().parse().unwrap();
            let played = play(&level, &route, (seed, t0_ms), clock, until);
            let tol = if clock == "c18" {
                Tolerances::exact()
            } else {
                Tolerances::feel()
            };
            let cmp = trace::compare(&played.rows, &reference, &tol);
            // Claw's respawns: on that very step he is back on his floor
            // with full health, at his place or within the step he takes
            // from it in that step (up to a sprint's 9 px in the original);
            // where exactly is compared with the reference runs below.
            if clock == "c18" {
                for w in reference.windows(2) {
                    if w[0].health > 0 || w[1].health <= 0 {
                        continue;
                    }
                    let mine = played.him.get(w[1].tick as usize).copied().flatten();
                    println!("respawn on step {}: Wolvington {mine:?}", w[1].tick);
                    let mine = mine.expect("Wolvington");
                    assert_eq!((mine.y, mine.health), (HIM_AT.1, 100));
                    assert!((HIM_AT.0 - 16..=HIM_AT.0).contains(&mine.x), "{mine:?}");
                }
            }
            println!("== level6/{store} {clock}");
            print!("{}", trace::report(&played.rows, &reference, &cmp, 3));
            assert!(cmp.rows_compared > 0);
            let mut firsts = trace_firsts(&cmp);
            if let Some(his) = runs
                .as_ref()
                .and_then(|r| Some(r.join(run_id(&meta, clock)?).join("boss.csv")))
                .and_then(|p| reference_him(&p))
            {
                for (k, v) in compare_him(&played, &his, clock) {
                    firsts.insert(k.to_string(), v);
                }
            }
            let list = known(store, clock);
            for (column, first) in &firsts {
                println!("first {store} {clock} {column} {first}");
                match list.iter().find(|(name, _)| name == column) {
                    Some(&(_, at)) if *first >= at => {}
                    _ => failures.push(format!("{store} {clock}: {column} at {first} ms")),
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ----------------------------------------------------------------------
// The fight to the gem

fn input(held: u32, pressed: u32) -> TickInput {
    TickInput {
        held: Buttons(held),
        pressed: Buttons(pressed),
    }
}

/// Into his room through the door, and the introduction, at the original
/// speeds; the game is left at the thaw.
fn after_the_introduction(level: SimLevel) -> Sim {
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    start_at(&mut sim, START, 828, SEED, None);
    for t in 0..720u64 {
        let i = match t {
            15 => input(JUMP, JUMP),
            16 => input(JUMP, 0),
            _ => TickInput::default(),
        };
        sim.step(i);
    }
    assert_eq!(sim.boss().fight(), fight::ON);
    sim
}

/// The scripted player of the fight, decided each tick from what is on
/// screen: walk at Wolvington, turn to him and punch every eighth tick
/// once within 60 px (a punch never makes him hop back).
fn fight_input(sim: &Sim, him: ObjectId, k: u64) -> TickInput {
    let c = sim.player().unwrap();
    let toward = |dx: i32| if dx > 0 { RIGHT } else { LEFT };
    match sim
        .objects()
        .get(him)
        .and_then(|o| EnemyState::from_id(o.state).map(|s| (o.x, s)))
    {
        Some((x, s)) if s != EnemyState::Dead && c.state != BOSS_END => {
            let dx = x - c.x;
            if dx.abs() > 60 {
                input(toward(dx), 0)
            } else if (dx > 0) != sim.claw().facing_right {
                input(toward(dx), toward(dx))
            } else if k.is_multiple_of(8) {
                input(ATTACK, ATTACK)
            } else {
                TickInput::default()
            }
        }
        _ => TickInput::default(),
    }
}

/// Fights Wolvington to his death with Claw kept on his feet; then waits
/// for the gem to land. Returns his health after each change.
fn fight_to_the_gem(sim: &mut Sim) -> Vec<i32> {
    let him = sim.boss().current().expect("Wolvington");
    let mut healths = vec![100];
    let mut bar = vec![100];
    let mut k = 0u64;
    loop {
        assert!(k < 40_000, "he falls");
        let i = fight_input(sim, him, k);
        sim.step(i);
        sim.with_world(|w| {
            if w.claw.stats.health < 50 {
                w.claw.stats.health = 100;
            }
        });
        let o = sim.objects().get(him).unwrap().clone();
        if healths.last() != Some(&o.health) {
            healths.push(o.health);
        }
        if let Some(m) = sim.hud().boss_meter {
            assert_eq!(m.max, wolvington::METER_MAX);
            if bar.last() != Some(&m.value) {
                bar.push(m.value);
            }
        }
        if EnemyState::from_id(o.state) == Some(EnemyState::Dead) {
            break;
        }
        k += 1;
    }
    // The bar showed every health he had while he lived.
    assert_eq!(bar[..healths.len() - 1], healths[..healths.len() - 1]);
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    assert_eq!(sim.boss().defeated(), Some(him));
    assert!(
        sim.last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == AMULET_SOUND))
    );
    let gem = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_GEM)
        .map(|(id, _)| id)
        .expect("the gem");
    let mut n = 0;
    while &*sim.objects().get(gem).unwrap().logic != pickup::END_OF_LEVEL_LOGIC {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 400, "the gem lands");
    }
    // It lands at its target, or a pixel or two from it by where he fell
    // (see the next test for the original's own fall).
    let o = sim.objects().get(gem).unwrap();
    println!("the gem landed at {:?}", (o.x, o.y));
    assert!(
        (o.x - GEM_TARGET.0).abs() <= 2 && (o.y - GEM_TARGET.1).abs() <= 2,
        "at its target: {:?}",
        (o.x, o.y)
    );
    assert_eq!(sim.player().unwrap().state, state::STAND);
    healths
}

/// Puts Claw on the floor at `x` and lets him settle there.
fn stand_at(sim: &mut Sim, x: i32) {
    let y = sim.player().unwrap().y;
    sim.teleport_player(x, y);
    for _ in 0..20 {
        sim.step(TickInput::default());
    }
    let c = sim.player().unwrap();
    assert_eq!((c.x, c.state), (x, state::STAND));
}

/// A jump from the floor with Jump held `hold` ticks: whether it ends the
/// level, and how high Claw rose.
fn gem_jump(sim: &mut Sim, hold: u64) -> (bool, i32) {
    let floor_y = sim.player().unwrap().y;
    let mut top = floor_y;
    for k in 0..120u64 {
        let i = match k {
            0 => input(JUMP, JUMP),
            k if k < hold => input(JUMP, 0),
            _ => TickInput::default(),
        };
        sim.step(i);
        top = top.min(sim.player().unwrap().y);
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            return (true, floor_y - top);
        }
    }
    (false, floor_y - top)
}

#[test]
fn install_level6_his_defeat_releases_the_gem() {
    let Some(level) = level6() else { return };
    let mut sim = after_the_introduction(level);
    let healths = fight_to_the_gem(&mut sim);
    println!("his health: {healths:?}");
    assert!(healths.len() >= 5, "several blows");
    // Every accepted blow costs him 5.
    for w in healths.windows(2) {
        assert!(w[0] - w[1] == 5 || w[1] <= 0, "{healths:?}");
    }
}

/// Where the original's run that ends the fight saw him fall: the gem
/// starts there.
const FELL_AT: (i32, i32) = (34847, 4497);

/// The room after the introduction with the gem out of where the
/// original saw him fall, landed: Claw stands at `x` on the floor.
fn gem_landed(level: SimLevel, x: i32) -> Sim {
    let mut sim = after_the_introduction(level);
    let z = sim
        .boss()
        .current()
        .map_or(4010, |id| sim.objects().get(id).unwrap().z);
    let gem =
        sim.with_world(|w| pickup::spawn_end_gem(w, (FELL_AT.0, FELL_AT.1, z + 1), GEM_TARGET));
    let mut n = 0;
    while &*sim.objects().get(gem).unwrap().logic != pickup::END_OF_LEVEL_LOGIC {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 400, "the gem lands");
    }
    // On its target, the pickup on the 106th tick after it rose, as in
    // the original.
    assert_eq!(n, 106);
    let o = sim.objects().get(gem).unwrap();
    assert_eq!((o.x, o.y), GEM_TARGET);
    if let Some(id) = sim.boss().current() {
        sim.with_world(|w| {
            w.objects.remove(id);
        });
    }
    stand_at(&mut sim, x);
    sim
}

#[test]
fn install_level6_a_jump_held_11_ticks_collects_the_gem_and_8_does_not() {
    let Some(level) = level6() else { return };
    // Jump held 8 ticks peaks below the gem; 11 ticks collects it, from
    // where the original's run jumped.
    let mut short = gem_landed(level.clone(), 34885);
    let (done, rise) = gem_jump(&mut short, 8);
    println!("Jump held 8 ticks: {rise} px");
    assert!(!done, "8 ticks of Jump do not reach the gem");
    assert_eq!(rise, 84);
    let mut held = gem_landed(level, 34885);
    let (done, rise) = gem_jump(&mut held, 11);
    println!("Jump held 11 ticks: the level ended {rise} px up");
    assert!(done, "11 ticks of Jump collect the gem");
}
