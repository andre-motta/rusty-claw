//! The fourteenth level's boss room on your own install: Omar, his ring
//! and his energy balls, played against the original's runs of the same
//! routes, and the fight's main outcomes checked on their own.
//!
//! Each route puts Claw down after the level's first tick, the way the
//! original's runs put him there (between two ticks, as a warp does, a
//! little above the floor he lands on); waits until the level clock
//! reaches the time the original's run started at; and starts the shared
//! random stream from the run's state, once. Then, by route:
//!
//! - intro-normal: left of the boss room's warp, Right from row 15 walks
//!   Claw into it; the warp drops him falling onto the ladder top, he
//!   lands in the trigger's box three steps later and stands through the
//!   whole introduction and the start of the fight;
//! - skip: the same walk with Jump pressed on Claw's first row in the
//!   room (34 at the original's speeds, 31 at the default ones): a falling
//!   Claw cannot jump, so the introduction runs all the same;
//! - end-life: the same walk, and the End Life three steps after the
//!   trigger (two at the default speeds): Claw dies at once, respawns on
//!   the ladder top with full health and one life fewer long before the
//!   introduction is over, and plays on while it runs; end-life-h90 is the
//!   same from a health of 90 (a measuring run: the route's `# health 90`
//!   line);
//! - fight (a measuring run of the whole fight): Claw starts on the
//!   chamber's floor right of the pillar with the ice sword, 300 health,
//!   and Omar with 50; ice shots through the ring's gap, a teleport left
//!   of the pillar with the fire sword, fire shots until the first phase
//!   ends, teleports into the hall above, a sword blow, Omar's energy
//!   balls, his health set to 25, the killing blow, his death and the
//!   gem, and a teleport onto the gem's rest point that takes it;
//! - kill-dead: the fight to the killing blow with the respawn point in
//!   the chamber, and the End Life while Omar falls dead: no gem comes.
//!
//! Each route is played tick by tick and compared with the reference
//! trace of its clock: at the original's speeds (c18) every column
//! exactly, the random stream and the boss columns included; at the
//! default speeds played tick by tick (c60) within the feel tolerances.
//! At c18, Omar is compared too with the run's list of him per step
//! (`boss.csv`: his position, his state lined up by the numbers the runs
//! give them, [`reference_state`], and his health), and the pieces of his
//! ring and his energy balls with the run's list of the objects near Claw
//! (`obj-log.csv`): every piece and every ball the run lists on a step is
//! where one of this build's is. Both lists lie beside each run, in the
//! folder named by `CLAW_REFERENCE_RUNS` (by default `rt` next to the
//! reference store).
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below ([`ROUTES`]);
//! each store's own copy of its route must agree with them. Needs
//! `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each test
//! skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but a listed store lacks its route, meta or
//! trace, or the reference runs (or a run's `boss.csv` or `obj-log.csv`)
//! are missing. `CLAW_L14B_ONLY` names one store to play alone,
//! `CLAW_L14B_TRACE` a folder to write the replay's traces to and
//! `CLAW_L14B_DUMP=FROM..TO` prints Claw and Omar of both on those rows.
//! Nothing from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::input::{self, ScriptTuning};
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::enemy::omar::{self, bullet, shield};
use claw_sim::logic::lava::mouth;
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{ObjectData, pickup};
use claw_sim::object::ObjectId;
use claw_sim::object::flags;
use claw_sim::player::powerup_aid::AidPowerup;
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

/// Where the warp routes put Claw: left of the boss room's warp.
const WARP_WALK: (i32, i32) = (31050, 2290);
/// Where the fight routes put Claw: on the chamber's floor right of the
/// pillar.
const CHAMBER: (i32, i32) = (41960, 2440);
/// Omar, where he is placed.
const OMAR_AT: (i32, i32) = (42241, 2468);

/// The ice sword and the fire sword.
const ICE_SWORD: u32 = 0x26BA;
const FIRE_SWORD: u32 = 0x26B8;

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

/// The set-up of a measuring fight route, at one clock's steps: the
/// powerup given at time zero; Omar's health after the first step; the
/// powerup given after a step; Claw's teleports after steps; Omar's
/// health set after a step.
#[derive(Debug, Clone, Copy)]
struct FightSetUp {
    powerup: (u32, u32),
    boss_health: i32,
    powerup_at: (usize, u32, u32),
    teleports: &'static [(usize, (i32, i32))],
    boss_health_at: (usize, i32),
}

/// A route of the stores: its name, where Claw is put down, how many rows
/// it has, its stretches, the clock it was run at, the row before which
/// the End Life comes, Claw's health at time zero, the respawn point set
/// at time zero, the fight's set-up, and whether it ends the level.
#[derive(Debug, Clone, Copy)]
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    clocks: &'static [&'static str],
    end_life: Option<usize>,
    health: Option<i32>,
    checkpoint: Option<(i32, i32)>,
    fight: Option<FightSetUp>,
    completes: bool,
}

const WALK: [Span; 1] = [span(15, 59, RIGHT)];

const fn walk(store: &'static str, clocks: &'static [&'static str]) -> Route {
    Route {
        store,
        start: WARP_WALK,
        rows: 1000,
        spans: &WALK,
        clocks,
        end_life: None,
        health: None,
        checkpoint: None,
        fight: None,
        completes: false,
    }
}

/// The fight's keys: a face-right tap, the ice shots every 20 rows, a
/// step left, the fire shots every 31 rows, a face-right tap in the hall,
/// the blow, a walk right into Omar, a face-left tap and the killing
/// blow.
const FIGHT_SPANS: [Span; 30] = [
    span(15, 16, RIGHT),
    span(30, 31, ATTACK),
    span(50, 51, ATTACK),
    span(70, 71, ATTACK),
    span(90, 91, ATTACK),
    span(110, 111, ATTACK),
    span(130, 131, ATTACK),
    span(150, 151, ATTACK),
    span(170, 171, ATTACK),
    span(200, 201, LEFT),
    span(206, 207, ATTACK),
    span(237, 238, ATTACK),
    span(268, 269, ATTACK),
    span(299, 300, ATTACK),
    span(330, 331, ATTACK),
    span(361, 362, ATTACK),
    span(392, 393, ATTACK),
    span(423, 424, ATTACK),
    span(520, 521, RIGHT),
    span(560, 561, ATTACK),
    span(575, 585, RIGHT),
    span(1105, 1106, LEFT),
    span(1110, 1111, ATTACK),
    // Padding: no more keys (kept so the list has a fixed length).
    span(0, 0, 0),
    span(0, 0, 0),
    span(0, 0, 0),
    span(0, 0, 0),
    span(0, 0, 0),
    span(0, 0, 0),
    span(0, 0, 0),
];

const FIGHT_C18: FightSetUp = FightSetUp {
    powerup: (ICE_SWORD, 120_000),
    boss_health: 50,
    powerup_at: (184, FIRE_SWORD, 5500),
    teleports: &[
        (184, (41725, 2440)),
        (500, (42275, 1040)),
        (600, (41700, 1040)),
        (1100, (41380, 1040)),
        (1450, (41825, 940)),
    ],
    boss_health_at: (1100, 25),
};

const FIGHT_C60: FightSetUp = FightSetUp {
    powerup: (ICE_SWORD, 120_000),
    boss_health: 50,
    powerup_at: (199, FIRE_SWORD, 5500),
    teleports: &[
        (199, (41725, 2440)),
        (540, (42275, 1040)),
        (648, (41700, 1040)),
        (1188, (41380, 1040)),
        (1566, (41825, 940)),
    ],
    boss_health_at: (1188, 25),
};

const fn fight(
    store: &'static str,
    clocks: &'static [&'static str],
    set_up: FightSetUp,
    end_life: Option<usize>,
    checkpoint: Option<(i32, i32)>,
    completes: bool,
) -> Route {
    Route {
        store,
        start: CHAMBER,
        rows: 1700,
        spans: &FIGHT_SPANS,
        clocks,
        end_life,
        health: Some(300),
        checkpoint,
        fight: Some(set_up),
        completes,
    }
}

/// The routes of the stores.
const ROUTES: [Route; 10] = [
    walk("boss-intro-normal", &["c18", "c60"]),
    Route {
        spans: &[span(15, 59, RIGHT), span(34, 47, JUMP)],
        rows: 800,
        ..walk("boss-skip-c18", &["c18"])
    },
    Route {
        spans: &[span(15, 59, RIGHT), span(31, 44, JUMP)],
        rows: 800,
        ..walk("boss-skip", &["c60"])
    },
    Route {
        end_life: Some(40),
        ..walk("boss-end-life-c18", &["c18"])
    },
    Route {
        end_life: Some(36),
        ..walk("boss-end-life", &["c60"])
    },
    Route {
        end_life: Some(40),
        health: Some(90),
        ..walk("boss-end-life-h90-c18", &["c18"])
    },
    Route {
        end_life: Some(36),
        health: Some(90),
        ..walk("boss-end-life-h90", &["c60"])
    },
    fight("boss-fight-c18", &["c18"], FIGHT_C18, None, None, true),
    fight("boss-fight", &["c60"], FIGHT_C60, None, None, true),
    fight(
        "boss-kill-dead-c18",
        &["c18"],
        FIGHT_C18,
        Some(1215),
        Some(CHAMBER),
        false,
    ),
];

/// True for the row of route `r` that follows one of the fight's
/// teleports. The teleports are the original's runs' own way of moving
/// Claw between two steps (a part of the measuring set-up, not of the
/// game): unlike a warp door, they run the objects around him twice more
/// with no game time, so Omar makes his frame pulse roll twice there. The
/// replay moves Claw between the ticks as a warp does and, after that row,
/// takes the run's random stream at the original's speeds (c18; at the
/// default speeds the stream is not compared).
fn after_teleport(r: &Route, k: usize) -> bool {
    r.fight
        .is_some_and(|f| k > 0 && f.teleports.iter().any(|t| t.0 == k - 1))
}

/// Half the size of the window an energy ball stays in around the camera,
/// px: Omar's family wakes and sleeps with the enemies' window.
const BALL_WINDOW: (i32, i32) = (800, 600);

/// The measuring routes (the stores say so in their meta).
const DIAGNOSTIC: [&str; 5] = [
    "boss-end-life-h90-c18",
    "boss-end-life-h90",
    "boss-fight-c18",
    "boss-fight",
    "boss-kill-dead-c18",
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The warp at the original's speeds (c18): before Claw's first tick at
/// the warp's target the original runs every object still awake around
/// the door once more with no game time, and puts those by the door to
/// sleep, so the tiger guard by the door makes one draw of its idle there
/// and none of its frame pulse roll on Claw's first tick in the room. That
/// rule comes with the sixth level's chain of rules, merged later; until
/// then the random stream is one draw apart from the room's first tick
/// (630 ms), and the screen shake of the banner's landing, which draws
/// from it, shakes the camera differently. Nothing else of the room
/// follows the stream before the fight's second phase.
const WARP_C18: &[(&str, u64)] = &[("rng", 630), ("cam_x", 9468), ("cam_y", 9450)];

/// The warp at the original's speeds with the End Life: [`WARP_C18`]'s
/// cause; and the original's frame pulse countdown runs on through the
/// frames the in-game menu takes, which the replay does not play (every
/// pulse draw comes a tick later from the End Life on, 738 ms); the
/// banner's shake shakes the camera differently from 9126 ms.
const END_LIFE_C18: &[(&str, u64)] = &[("rng", 630), ("cam_x", 9126), ("cam_y", 9126)];

/// The warp walk at the default speeds (c60, the default tuning played
/// tick by tick against the original's frames of 16 and 17 ms): the walk
/// into the warp ends a pixel off the original's 60 frames per second
/// walk (the checkpoint at 41842 against 41841, then the camera), and the
/// introduction's delays count 18 ms ticks against frames, so the fight
/// starts about a quarter of a second apart.
const WARP_C60: &[(&str, u64)] = &[
    ("events", 617),
    ("cam_x", 1100),
    ("cam_y", 1117),
    ("boss_fight", 11467),
    ("state_id", 11483),
];

/// The End Life at the default speeds: [`WARP_C60`]'s causes, the warp
/// reached two frames earlier, and Claw's death fall timed in 18 ms ticks
/// against frames (93 ticks against 100 frames to the lives step), so he
/// falls down the shaft at another pace and respawns earlier.
const END_LIFE_C60: &[(&str, u64)] = &[
    ("events", 617),
    ("cam_x", 1099),
    ("cam_y", 1233),
    ("y", 1949),
    ("boss_fight", 11466),
];

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. The diagnostic fight
/// (`boss-fight-c18`) follows the original exactly at c18.
///
/// - Life ended in Omar's death fall (c18): the original's frame pulse
///   countdown runs on through the frames the in-game menu takes, so
///   every pulse draw comes a tick later from the End Life on; the replay
///   plays no time for the menu. Nothing decides on those draws there.
/// - The fight at the default speeds (c60): Claw's flinch from a lava
///   hand's ball, timed in 18 ms ticks, ends a frame apart (667 ms); the
///   first teleport comes with Left pressed on the next row, which the
///   original's teleport lets pass without steering his fall and this
///   build's warp move lets steer it (3350 ms); the fight then plays on
///   other ticks (his moves, casts and balls, the hands' throws) and ends
///   the level a little apart.
const KNOWN: &[Known] = &[
    ("boss-intro-normal", "c18", WARP_C18),
    ("boss-skip-c18", "c18", WARP_C18),
    ("boss-end-life-c18", "c18", END_LIFE_C18),
    ("boss-end-life-h90-c18", "c18", END_LIFE_C18),
    ("boss-kill-dead-c18", "c18", &[("rng", 21906)]),
    ("boss-intro-normal", "c60", WARP_C60),
    ("boss-skip", "c60", WARP_C60),
    ("boss-end-life", "c60", END_LIFE_C60),
    ("boss-end-life-h90", "c60", END_LIFE_C60),
    (
        "boss-fight",
        "c60",
        &[
            ("state_id", 667),
            ("x", 3350),
            ("y", 3350),
            ("physics_type", 3350),
            ("cam_x", 3350),
            ("cam_y", 3350),
            ("health", 4533),
            ("boss_health", 19833),
            ("events", 26483),
        ],
    ),
];

/// Stores whose level's end is known not to come at a clock, with the
/// cause:
///
/// - the fight at the default speeds: the random stream is not the run's
///   at this clock (the run's first frame already drew), so Omar's casts
///   come on other ticks; one of his energy balls finds Claw as the
///   measuring route's killing blow comes, and the blow does not land.
const UNFINISHED: &[(&str, &str)] = &[("boss-fight", "c60")];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
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
    for s in r.spans.iter().filter(|s| s.held != 0) {
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

// ----------------------------------------------------------------------
// The reference runs

/// The state a run's number for Omar's state stands for, in this build's
/// numbers.
fn reference_state(n: i32) -> Option<i32> {
    let s = match n {
        2000 => EnemyState::Idle,
        2004 => EnemyState::Dead,
        2018 => EnemyState::Home,
        2024 => EnemyState::HitLow,
        2025 => EnemyState::HitHigh,
        2028 => EnemyState::Startup,
        2036 => EnemyState::BlockLow,
        2052 => EnemyState::Wait,
        2053 => EnemyState::MagicAttack,
        2054 => EnemyState::MoveToPosition,
        _ => return None,
    };
    Some(s as i32)
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

/// Omar by step in a run's `boss.csv`: `None` on the steps without him.
fn reference_omar(path: &Path) -> Option<Vec<(usize, Option<At>)>> {
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

/// The places of the objects of `logic` by step in a run's
/// `obj-log.csv`: (step, x, y) for each one listed.
fn reference_places(path: &Path, logic: &str) -> Option<Vec<(usize, i32, i32)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, name, x, y) = (col("tick")?, col("logic_name")?, col("x")?, col("y")?);
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        if f.get(name) != Some(&logic) {
            continue;
        }
        out.push((
            f.get(tick)?.parse().ok()?,
            f.get(x)?.parse().ok()?,
            f.get(y)?.parse().ok()?,
        ));
    }
    Some(out)
}

// ----------------------------------------------------------------------
// Playing

fn level14() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(14)).unwrap();
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
        "the reference runs are not at {} (set CLAW_REFERENCE_RUNS)",
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

fn config(clock: &str) -> SimConfig {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    SimConfig {
        tuning: tuning.tuning(),
        seed: SEED,
        ..SimConfig::default()
    }
}

fn t0_ms(clock: &str) -> u64 {
    T0_MS.iter().find(|(c, _)| *c == clock).map_or(0, |p| p.1)
}

/// The game at a route's time zero (see the module documentation), with
/// Claw's health, the respawn point and the powerup the route gives
/// there.
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route) -> Sim {
    let mut sim = Sim::new(level.clone(), config(clock));
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(r.start));
    while sim.tick() * TICK < t0_ms(clock) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(SEED));
    if let Some(h) = r.health {
        sim.set_health(h);
    }
    if let Some(p) = r.checkpoint {
        sim.set_checkpoint(Some(p));
    }
    if let Some(f) = r.fight {
        sim.give_powerup(AidPowerup {
            kind: f.powerup.0,
            ms: f.powerup.1,
        });
    }
    sim
}

/// Omar's object, once the level has built him.
fn omar_id(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == omar::NAMES[0])
        .find(|(_, o)| (o.editor.0, o.editor.1) == OMAR_AT)
        .map(|(id, _)| id)
}

/// Omar while he is in the update pass (the original's runs list only
/// those) and built.
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

/// The places of this build's objects of `logic` in the update pass.
fn places(sim: &Sim, logic: &str) -> Vec<(i32, i32)> {
    let mut v: Vec<(i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic && o.is_listed())
        .map(|(_, o)| (o.x, o.y))
        .collect();
    v.sort_unstable();
    v
}

/// Plays one row of route `r`: the End Life that comes before it, then
/// the row.
fn step_row(sim: &mut Sim, r: &Route, ticks: &[TickInput], k: usize) {
    if r.end_life == Some(k) {
        sim.end_life();
    }
    sim.step(ticks.get(k).copied().unwrap_or_default());
}

/// The fight's set-up the original's runs made after row `k` of route
/// `r` (after the row is logged).
fn set_up_after(sim: &mut Sim, r: &Route, k: usize) {
    let Some(f) = r.fight else { return };
    if k == 0 {
        set_omar_health(sim, f.boss_health);
    }
    if k == f.powerup_at.0 {
        sim.give_powerup(AidPowerup {
            kind: f.powerup_at.1,
            ms: f.powerup_at.2,
        });
    }
    if k == f.boss_health_at.0 {
        set_omar_health(sim, f.boss_health_at.1);
    }
    if let Some(&(_, to)) = f.teleports.iter().find(|t| t.0 == k) {
        // Held for the gem, Claw is only moved: a warp's move would also
        // drop the camera's scripted point, which the runs' teleport
        // keeps.
        if sim.player().is_some_and(|c| c.state == BOSS_END) {
            sim.teleport_player(to.0, to.1);
        } else {
            sim.with_world(|w| w.claw.special.warp_to = Some(to));
        }
    }
}

/// Sets the current boss's health, between two ticks.
fn set_omar_health(sim: &mut Sim, health: i32) {
    sim.with_world(|w| {
        if let Some(id) = w.current_boss()
            && let Some(o) = w.objects.get_mut(id)
        {
            o.health = health;
        }
    });
}

/// One play of a route.
struct Played {
    rows: Vec<TraceRow>,
    /// Omar per row.
    omar: Vec<Option<At>>,
    /// The pieces of his ring per row.
    pieces: Vec<Vec<(i32, i32)>>,
    /// His energy balls per row.
    balls: Vec<Vec<(i32, i32)>>,
    completed: bool,
}

/// Plays `r` at `clock` from its time zero for `rows` rows. With the
/// `reference` rows of the original's run, the random stream is put in
/// step with the run's after each row that follows one of the fight's
/// teleports (see [`after_teleport`]).
fn play(
    level: &SimLevel,
    r: &Route,
    clock: &str,
    rows: usize,
    reference: Option<&[TraceRow]>,
) -> Played {
    let mut sim = at_time_zero(level, clock, r);
    let ticks = route_ticks(r);
    let mut out = Played {
        rows: Vec::new(),
        omar: Vec::new(),
        pieces: Vec::new(),
        balls: Vec::new(),
        completed: false,
    };
    let mut id = None;
    for k in 0..rows {
        step_row(&mut sim, r, &ticks, k);
        if clock == "c18"
            && after_teleport(r, k)
            && let Some(state) = reference.and_then(|rows| rows.get(k)?.rng)
        {
            sim.with_world(|w| *w.rng = Rng::new(state));
        }
        out.completed |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
        out.rows.push(TraceRow::capture(&sim, k as u64, "script"));
        if id.is_none() {
            id = omar_id(&sim);
        }
        out.omar.push(look(&sim, id));
        out.pieces.push(places(&sim, shield::NAMES[0]));
        out.balls.push(places(&sim, bullet::NAMES[0]));
        set_up_after(&mut sim, r, k);
    }
    out
}

/// The step on which a measuring run set Omar's health (its `meta.txt`
/// says so): its `boss.csv` shows the new health there while its trace
/// still shows his own, as this build's play does.
fn health_set_steps(run: &Path) -> Vec<usize> {
    let Ok(meta) = std::fs::read_to_string(run.join("meta.txt")) else {
        return Vec::new();
    };
    meta.lines()
        .filter_map(|l| {
            if let Some(v) = l.strip_prefix("boss_health_set_step=") {
                return v.trim().parse().ok();
            }
            let v = l.strip_prefix("boss_health_at_")?.split_once('=')?.1;
            v.split_once('@')?.1.trim().parse().ok()
        })
        .collect()
}

/// The first step (as a reference time, ms) at which Omar's x, y, state or
/// health leaves the run's (his health is not compared on the steps
/// `set_at`, on which a measuring run set it), and at which a piece of his
/// ring or one of his balls the run lists is not where one of this
/// build's is.
fn compare_room(
    played: &Played,
    reference: &[TraceRow],
    omar_ref: &[(usize, Option<At>)],
    pieces_ref: &[(usize, i32, i32)],
    balls_ref: &[(usize, i32, i32)],
    set_at: &[usize],
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: &str, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name.to_string(), (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in omar_ref {
        let Some(got) = played.omar.get(k) else {
            continue;
        };
        match (want, *got) {
            (None, None) => {}
            (Some(want), Some(got)) => {
                if got.x != want.x {
                    note("omar_x", k);
                }
                if got.y != want.y {
                    note("omar_y", k);
                }
                if want.state.is_none() || got.state != want.state {
                    note("omar_state", k);
                }
                if got.health != want.health && !set_at.contains(&k) {
                    note("omar_health", k);
                }
            }
            _ => {
                for c in ["omar_x", "omar_y", "omar_state", "omar_health"] {
                    note(c, k);
                }
            }
        }
    }
    for &(k, x, y) in pieces_ref {
        if played.pieces.get(k).is_some_and(|p| !p.contains(&(x, y))) {
            note("ring", k);
        }
    }
    for &(k, x, y) in balls_ref {
        // The runs list the objects of a step before its end puts away
        // those that left their window: a ball that flew out of it is
        // listed once more there, gone here.
        let left = reference.get(k).is_some_and(|row| {
            (x - row.cam_x).abs() > BALL_WINDOW.0 || (y - row.cam_y).abs() > BALL_WINDOW.1
        });
        if left {
            continue;
        }
        if played.balls.get(k).is_some_and(|p| !p.contains(&(x, y))) {
            note("ball", k);
        }
    }
    firsts
}

/// With `CLAW_L14B_DUMP=FROM..TO`, prints Claw and Omar of the play and
/// of the run on those rows, for looking into a divergence.
fn dump(played: &Played, reference: &[TraceRow], omar_ref: &[(usize, Option<At>)]) {
    let Some((a, b)) = std::env::var("CLAW_L14B_DUMP").ok().and_then(|v| {
        let (a, b) = v.split_once("..")?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }) else {
        return;
    };
    for k in a..b.min(played.rows.len()) {
        let ours = &played.rows[k];
        let theirs = reference.get(k);
        println!(
            "row {k}: claw ours {} {} s{} p{} h{} l{} rng {:?} cam {} {} | ref {:?}",
            ours.x,
            ours.y,
            ours.state_id,
            ours.physics_type,
            ours.health,
            ours.lives,
            ours.rng,
            ours.cam_x,
            ours.cam_y,
            theirs.map(|t| (
                t.x,
                t.y,
                t.state_id,
                t.physics_type,
                t.health,
                t.lives,
                t.rng,
                t.cam_x,
                t.cam_y
            ))
        );
        println!(
            "  ring ours {:?} balls {:?}",
            played.pieces[k], played.balls[k]
        );
        let rt = omar_ref.iter().find(|r| r.0 == k).and_then(|r| r.1);
        println!(
            "  omar ours {:?} ref {:?} | fight {:?} {:?} ref {:?} {:?}",
            played.omar[k],
            rt,
            ours.boss_fight,
            ours.boss_health,
            theirs.and_then(|t| t.boss_fight),
            theirs.and_then(|t| t.boss_health),
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

/// A store's own copy of its route: (held, pressed) by row, its
/// directives' lines and its whole text.
type StoreRoute = (Vec<(u32, u32)>, Vec<String>, String);

fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut directives = Vec::new();
    for l in text.lines() {
        if l.starts_with('@') {
            directives.push(l.to_string());
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, directives, text))
}

/// The value of a route's `# key VALUE` comment line.
fn comment_value(text: &str, key: &str) -> Option<String> {
    text.lines().rev().find_map(|l| {
        let rest = l.trim().strip_prefix('#')?.trim();
        let v = rest.strip_prefix(key)?;
        v.starts_with(' ').then(|| v.trim().to_string())
    })
}

/// A route's fight set-up as its comment lines give it.
fn store_set_up(text: &str) -> Option<FightSetUp> {
    let num = |s: &str| -> Option<u32> {
        let s = s.trim();
        match s.strip_prefix("0x") {
            Some(h) => u32::from_str_radix(h, 16).ok(),
            None => s.parse().ok(),
        }
    };
    let pair = |s: &str| -> Option<(u32, u32)> {
        let (a, b) = s.split_once(',')?;
        Some((num(a)?, num(b)?))
    };
    let powerup = pair(&comment_value(text, "give_powerup")?)?;
    let boss_health = comment_value(text, "boss_health")?.parse().ok()?;
    let (step, rest) = comment_value(text, "give_powerup_at")?
        .split_once(':')
        .map(|(a, b)| (a.to_string(), b.to_string()))?;
    let at = pair(&rest)?;
    let (hstep, h) = comment_value(text, "boss_health_at")?
        .split_once(':')
        .map(|(a, b)| (a.parse().ok(), b.parse().ok()))?;
    let teleports: Vec<(usize, (i32, i32))> = comment_value(text, "teleport_at")?
        .split(';')
        .filter_map(|t| {
            let (k, p) = t.split_once(':')?;
            let (x, y) = p.split_once(',')?;
            Some((k.parse().ok()?, (x.parse().ok()?, y.parse().ok()?)))
        })
        .collect();
    let teleports: &'static [(usize, (i32, i32))] = Box::leak(teleports.into_boxed_slice());
    Some(FightSetUp {
        powerup,
        boss_health,
        powerup_at: (step.parse().ok()?, at.0, at.1),
        teleports,
        boss_health_at: (hstep?, h?),
    })
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level14").join(r.store);
        let (rows, directives, text) =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        let end_life: Vec<usize> = directives
            .iter()
            .map(|d| {
                let rest = d.strip_prefix('@').expect("a directive");
                let (at, what) = rest.split_once(' ').expect("a directive");
                assert_eq!(what.trim(), "command end_life", "{}: {d}", r.store);
                at.parse().expect("a row")
            })
            .collect();
        assert_eq!(
            end_life,
            r.end_life.into_iter().collect::<Vec<_>>(),
            "{}",
            r.store
        );
        assert_eq!(input::script_health(&text), Ok(r.health), "{}", r.store);
        assert_eq!(
            input::script_checkpoint(&text),
            Ok(r.checkpoint),
            "{}: checkpoint",
            r.store
        );
        let set_up = store_set_up(&text);
        match (set_up, r.fight) {
            (None, None) => {}
            (Some(s), Some(f)) => {
                assert_eq!(s.powerup, f.powerup, "{}", r.store);
                assert_eq!(s.boss_health, f.boss_health, "{}", r.store);
                assert_eq!(s.powerup_at, f.powerup_at, "{}", r.store);
                assert_eq!(s.teleports, f.teleports, "{}", r.store);
                assert_eq!(s.boss_health_at, f.boss_health_at, "{}", r.store);
            }
            _ => panic!("{}: the fight's set-up", r.store),
        }
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        assert_eq!(
            meta_value(&meta, "", "diagnostic").as_deref() == Some("true"),
            DIAGNOSTIC.contains(&r.store),
            "{}",
            r.store
        );
        assert_eq!(
            meta_value(&meta, "", "completed").as_deref() == Some("yes"),
            r.completes,
            "{}: the level's end",
            r.store
        );
        for &clock in r.clocks {
            let teleport = meta_value(&meta, clock, "teleport")
                .unwrap_or_else(|| panic!("{} {clock}: no start", r.store));
            let point: Vec<i32> = teleport.split(',').map(|v| v.parse().unwrap()).collect();
            assert_eq!(point, [r.start.0, r.start.1], "{} {clock}: start", r.store);
            let t0: u64 = meta_value(&meta, clock, "t0_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{} {clock}: no time zero", r.store));
            assert_eq!(t0, t0_ms(clock), "{} {clock}: time zero", r.store);
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_omar_routes_follow_the_reference_traces() {
    let Some(level) = level14() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L14B_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in &ROUTES {
        let store = r.store;
        if only.as_deref().is_some_and(|o| o != store) {
            continue;
        }
        let dir = root.join("level14").join(store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        for &clock in r.clocks {
            println!("== level14/{store} {clock}");
            let seed: u32 = meta_value(&meta, clock, "rng_at_t0")
                .or_else(|| meta_value(&meta, "", "rng_at_t0"))
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{store}: no random state"));
            assert_eq!(seed, SEED, "{store}: random state");
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let reference = trace::parse(&text).unwrap();
            let played = play(&level, r, clock, reference.len(), Some(&reference));
            if let Some(out) = std::env::var_os("CLAW_L14B_TRACE") {
                let out = PathBuf::from(out).join(format!("{store}-{clock}.csv"));
                std::fs::write(&out, trace::to_csv(&played.rows)).expect("the trace is written");
            }
            let mut room = Vec::new();
            if clock == "c18" {
                let id = run_id(&meta, clock)
                    .unwrap_or_else(|| panic!("{store}: no run named in its meta"));
                let run = runs.join(id);
                let omar_ref = reference_omar(&run.join("boss.csv"))
                    .unwrap_or_else(|| panic!("{store}: the run has no boss.csv"));
                let log = run.join("obj-log.csv");
                let pieces = reference_places(&log, shield::NAMES[0])
                    .unwrap_or_else(|| panic!("{store}: the run has no obj-log.csv"));
                let balls = reference_places(&log, bullet::NAMES[0])
                    .unwrap_or_else(|| panic!("{store}: the run has no obj-log.csv"));
                assert!(!pieces.is_empty(), "{store}: the run lists no ring");
                let set_at = health_set_steps(&run);
                assert_eq!(
                    set_at.is_empty(),
                    r.fight.is_none(),
                    "{store}: the run's health set-up"
                );
                room = compare_room(&played, &reference, &omar_ref, &pieces, &balls, &set_at);
                dump(&played, &reference, &omar_ref);
            } else {
                dump(&played, &reference, &[]);
            }
            let unfinished = UNFINISHED.contains(&(store, clock));
            if r.completes && unfinished {
                assert!(
                    !played.completed,
                    "{store} {clock}: now finishes, drop it from UNFINISHED"
                );
            } else if r.completes != played.completed {
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
    lives: i32,
    fight: u8,
    meter: Option<(i32, i32)>,
    camera_override: Option<(i32, i32)>,
    checkpoint: Option<(i32, i32)>,
    omar: Option<At>,
    phase: u8,
    ice: bool,
    gem_given: bool,
    safe: bool,
    out: bool,
    pieces: Vec<(i32, i32)>,
    balls: Vec<(i32, i32)>,
    gem: Option<(i32, i32)>,
    /// The delayed lava mouth of the corridor: its state and attack box.
    mouth: Option<(i32, claw_sim::object::Rect)>,
    ended: bool,
}

/// The corridor's lava mouth that waits 500 ms before it drips.
const DELAYED_MOUTH: (i32, i32) = (42144, 1903);

/// Plays `rows` rows of `store`'s route at `clock` from its time zero,
/// with the original's random state there. A fight route takes the run's
/// random stream after its teleports (see [`after_teleport`]), so it needs
/// `CLAW_REFERENCE` (`None` without it).
fn watch(level: &SimLevel, store: &str, clock: &str, rows: usize) -> Option<Vec<Look>> {
    let r = route_named(store);
    let reference = match r.fight {
        Some(_) => {
            let root = reference_root()?;
            let path = root
                .join("level14")
                .join(store)
                .join(format!("trace-{clock}-r1.csv"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            Some(trace::parse(&text).unwrap())
        }
        None => None,
    };
    let mut sim = at_time_zero(level, clock, r);
    let ticks = route_ticks(r);
    let mut out = Vec::new();
    let mut id = None;
    let mut checkpoint = None;
    for k in 0..rows {
        step_row(&mut sim, r, &ticks, k);
        if clock == "c18"
            && after_teleport(r, k)
            && let Some(state) = reference.as_deref().and_then(|rows| rows.get(k)?.rng)
        {
            sim.with_world(|w| *w.rng = Rng::new(state));
        }
        if id.is_none() {
            id = omar_id(&sim);
        }
        for e in sim.last_events() {
            if let GameEvent::Checkpoint { x, y } = e {
                checkpoint = Some((*x, *y));
            }
        }
        let c = sim.player().expect("Claw");
        let d = id.and_then(|i| match &sim.objects().get(i)?.data {
            ObjectData::Omar(d) => Some(d.clone()),
            _ => None,
        });
        let o = id.and_then(|i| sim.objects().get(i));
        let mouth = sim
            .objects()
            .all()
            .find(|(_, o)| {
                &*o.logic == mouth::NAMES[0] && (o.editor.0, o.editor.1) == DELAYED_MOUTH
            })
            .filter(|(_, o)| o.is_listed())
            .map(|(_, o)| (o.state, o.attack_rect));
        out.push(Look {
            claw: (c.x, c.y, c.state),
            claw_health: sim.claw().stats.health,
            lives: sim.claw().stats.lives,
            fight: sim.boss().fight(),
            meter: sim.boss().meter().map(|m| (m.value, m.max)),
            camera_override: sim.camera_override(),
            checkpoint,
            omar: look(&sim, id),
            phase: d.as_ref().map_or(1, |d| d.phase),
            ice: d.as_ref().is_some_and(|d| d.ice),
            gem_given: d.as_ref().is_some_and(|d| d.gem_given),
            safe: o.is_some_and(|o| o.flags & flags::SAFE != 0),
            out: o.is_some_and(|o| o.flags & flags::NO_COLLIDE != 0),
            pieces: places(&sim, shield::NAMES[0]),
            balls: places(&sim, bullet::NAMES[0]),
            gem: sim
                .objects()
                .all()
                .find(|(_, o)| &*o.logic == pickup::END_GEM)
                .map(|(_, o)| (o.x, o.y)),
            mouth,
            ended: sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
        });
        set_up_after(&mut sim, r, k);
    }
    Some(out)
}

fn st(l: &Look) -> Option<EnemyState> {
    l.omar.and_then(|a| a.state).and_then(EnemyState::from_id)
}

/// The rows on which Omar's state turns to `s`.
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

/// The first row on which the fight flag is `v`.
fn flag_at(rows: &[Look], v: u8) -> Option<usize> {
    rows.iter().position(|l| l.fight == v)
}

/// The ring's pieces of row `k` stand where the level clock puts them
/// around Omar at `at`: the previous row's turn, at the clock's `ms`.
fn ring_on_the_clock(rows: &[Look], k: usize, clock_ms: u64, at: (i32, i32)) {
    let mut want: Vec<(i32, i32)> = (0..omar::RING_PIECES)
        .map(|i| omar::ring_point(i, clock_ms, at))
        .collect();
    want.sort_unstable();
    assert_eq!(rows[k].pieces, want, "row {k}");
}

/// The level clock, ms, at the end of route row `k` at the 18 ms step:
/// the route's time zero is at 828 ms.
fn clock_after(k: usize) -> u64 {
    t0_ms("c18") + 18 * (k as u64 + 1)
}

#[test]
fn install_omar_the_warp_drops_claw_falling_into_the_616_step_introduction() {
    let Some(level) = level14() else { return };
    let Some(rows) = watch(&level, "boss-intro-normal", "c18", 700) else {
        return;
    };
    // N = 34: Claw falls from the warp's point for three ticks, lands in
    // the trigger's box on T = N + 3, which makes his place the checkpoint.
    let n = 34;
    assert_eq!(rows[n].claw, (41828, 1954, claw_state::FALL));
    for l in &rows[n..n + 3] {
        assert_eq!(l.claw.2, claw_state::FALL);
        assert_eq!(l.fight, fight::NONE);
    }
    let t = n + 3;
    assert_eq!((rows[t].claw.0, rows[t].claw.1), (41840, 1959));
    assert_eq!(rows[t].checkpoint, Some((41840, 1959)));
    assert_eq!(rows[t - 1].checkpoint, None);
    // The introduction: flag 1 and Claw frozen from T + 1, the health bar
    // full, the camera on Omar until T + 150; flag 2 on T + 617, Claw free
    // on T + 618.
    assert_eq!(flag_at(&rows, fight::STAGING), Some(t + 1));
    assert_eq!(rows[t + 1].claw.2, claw_state::FREEZE);
    assert_eq!(rows[t + 1].meter, Some((100, 100)));
    assert_eq!(rows[t + 1].camera_override, Some(omar::HOME));
    assert_eq!(rows[t + 149].camera_override, Some(omar::HOME));
    assert_eq!(rows[t + 150].camera_override, None);
    assert_eq!(flag_at(&rows, fight::ON), Some(t + 617));
    assert_eq!(rows[t + 617].claw.2, claw_state::FREEZE);
    assert_eq!(rows[t + 618].claw.2, claw_state::STAND);
    // Omar idles at his place through it all, his ring on the level clock.
    for (k, l) in rows.iter().enumerate().skip(n) {
        assert_eq!(st(l), Some(EnemyState::Idle), "row {k}");
        assert_eq!(
            l.omar.map(|a| (a.x, a.y, a.health)),
            Some((42250, 2475, 100))
        );
        assert_eq!(l.pieces.len(), omar::RING_PIECES);
    }
    for k in [n + 1, n + 2, 100, 400, 699] {
        ring_on_the_clock(&rows, k, clock_after(k - 1), omar::HOME);
    }
}

#[test]
fn install_omar_a_jump_on_the_first_rows_in_the_room_skips_nothing() {
    let Some(level) = level14() else { return };
    let Some(normal) = watch(&level, "boss-intro-normal", "c18", 700) else {
        return;
    };
    let Some(skip) = watch(&level, "boss-skip-c18", "c18", 700) else {
        return;
    };
    // Jump pressed on Claw's first row in the room: he falls all the same,
    // lands on the same tick and the introduction runs on the same ticks.
    for k in 34..40 {
        assert_eq!(skip[k].claw, normal[k].claw, "row {k}");
    }
    assert_eq!(skip[37].checkpoint, Some((41840, 1959)));
    assert_eq!(flag_at(&skip, fight::STAGING), Some(38));
    assert_eq!(flag_at(&skip, fight::ON), Some(654));
    assert_eq!(skip[655].claw.2, claw_state::STAND);
}

#[test]
fn install_omar_the_end_life_in_the_freeze_brings_claw_back_free_at_full_health() {
    let Some(level) = level14() else { return };
    for (store, health) in [("boss-end-life-c18", 100), ("boss-end-life-h90-c18", 90)] {
        let Some(rows) = watch(&level, store, "c18", 700) else {
            return;
        };
        let t = 37;
        assert_eq!(rows[t + 2].claw.2, claw_state::FREEZE, "{store}");
        assert_eq!(rows[t + 2].claw_health, health, "{store}");
        // The End Life on T + 3: dead at once, from the freeze.
        assert_eq!(rows[t + 3].claw.2, claw_state::DEATH_FALL, "{store}");
        assert_eq!(rows[t + 3].claw_health, 0, "{store}");
        // The lives step 93 ticks later, the respawn on the next: on the
        // ladder top with full health, free, the camera his again.
        assert_eq!(rows[t + 95].lives, 7, "{store}");
        assert_eq!(rows[t + 96].lives, 6, "{store}");
        assert_eq!(
            rows[t + 97].claw,
            (41840, 1959, claw_state::STAND),
            "{store}"
        );
        assert_eq!(rows[t + 97].claw_health, 100, "{store}");
        assert_eq!(rows[t + 96].camera_override, Some(omar::HOME), "{store}");
        assert_eq!(rows[t + 97].camera_override, None, "{store}");
        // The introduction keeps its ticks: flag 2 on T + 617; nothing
        // freezes Claw again.
        assert_eq!(flag_at(&rows, fight::ON), Some(t + 617), "{store}");
        assert!(
            rows[t + 97..]
                .iter()
                .all(|l| l.claw.2 != claw_state::FREEZE),
            "{store}"
        );
        // Omar is reset in the respawn: idle at his place, full health, his
        // ring of fire on the level clock without a jump.
        for k in [t + 96, t + 97, t + 98] {
            assert_eq!(st(&rows[k]), Some(EnemyState::Idle), "{store} {k}");
            assert_eq!(
                rows[k].omar.map(|a| (a.x, a.y, a.health)),
                Some((42250, 2475, 100))
            );
            assert!(!rows[k].ice);
            ring_on_the_clock(&rows, k, clock_after(k - 1), omar::HOME);
        }
        // The corridor's delayed mouth, put to sleep mid-drip as the camera
        // panned to Omar, wakes with Claw: it waits its 500 ms again
        // holding the drop box it showed when it went to sleep.
        let slept = (t..t + 97)
            .find(|&k| rows[k].mouth.is_none())
            .expect("the mouth sleeps under the pan");
        let held = rows[slept - 1].mouth.expect("awake before").1;
        assert!(held.is_set(), "{store}: a drop's box");
        for (k, l) in rows.iter().enumerate().take(t + 124).skip(t + 97) {
            assert_eq!(l.mouth, Some((mouth::state::IDLE, held)), "{store} {k}");
        }
        assert_eq!(
            rows[t + 124].mouth.map(|m| m.0),
            Some(mouth::state::FALLING)
        );
    }
}

#[test]
fn install_omar_the_first_phase_takes_four_good_shots_through_the_gap() {
    let Some(level) = level14() else { return };
    let Some(rows) = watch(&level, "boss-fight-c18", "c18", 700) else {
        return;
    };
    // The measuring set-up: his health 50 from the first row.
    assert_eq!(rows[1].omar.map(|a| a.health), Some(50));
    // The first ice shot through the fire ring's gap: 25 off, the ring of
    // ice, his move to the left ledge, which ends 112 ticks later.
    assert_eq!(starts(&rows, EnemyState::MoveToPosition)[0], 69);
    assert_eq!(rows[69].omar.map(|a| a.health), Some(25));
    assert!(rows[69].ice && rows[69].safe);
    assert_eq!(rows[180].omar.map(|a| (a.x, a.y)), Some(omar::HOME));
    assert_eq!(rows[181].omar.map(|a| (a.x, a.y)), Some(omar::LEDGE_LEFT));
    assert_eq!(st(&rows[181]), Some(EnemyState::Home));
    assert!(!rows[181].safe);
    // Ice shots that meet a piece end there: his health stays.
    assert!(
        rows[69..181]
            .iter()
            .all(|l| l.omar.map(|a| a.health) == Some(25))
    );
    // The fire shot that overlaps a piece and him on row 444 still hits
    // him: the ring is gone, his health full again, and he moves to the
    // hall, where he appears 112 ticks later.
    assert_eq!(rows[443].pieces.len(), 9);
    assert_eq!(rows[444].phase, 0);
    assert!(rows[445].pieces.is_empty());
    assert_eq!(rows[444].omar.map(|a| a.health), Some(100));
    assert_eq!(rows[444].meter, None, "no introduction: no bar");
    assert_eq!(rows[555].omar.map(|a| (a.x, a.y)), Some(omar::LEDGE_LEFT));
    assert_eq!(rows[556].omar.map(|a| (a.x, a.y)), Some(omar::HALL));
    // The lava hands' balls cost Claw 10 on the chamber's floor.
    let h = hurts(&rows[..500]);
    assert!(h.iter().all(|&(_, d)| d == 10), "{h:?}");
}

#[test]
fn install_omar_the_hall_phase_his_blows_balls_touch_death_and_gem() {
    let Some(level) = level14() else { return };
    let Some(rows) = watch(&level, "boss-fight-c18", "c18", 1500) else {
        return;
    };
    // The blow on row 564: 25 off, his move to the hall's left end.
    assert_eq!(rows[563].omar.map(|a| a.health), Some(100));
    assert_eq!(rows[564].omar.map(|a| a.health), Some(75));
    assert!(rows[564].safe);
    // His touch costs Claw 30 while nothing can hit him.
    let h = hurts(&rows);
    assert!(h.contains(&(580, 30)), "{h:?}");
    assert!(rows[580].safe);
    assert_eq!(rows[676].omar.map(|a| (a.x, a.y)), Some(omar::HALL_LEFT));
    // His casts: 60 ticks each, the ball on the 32nd, 65 px across and 20
    // up from him, 5.4 px a tick; one costs Claw 15 on row 767.
    let casts = starts(&rows, EnemyState::MagicAttack);
    assert_eq!(&casts[..4], &[685, 841, 904, 1015]);
    for &c in &casts[..4] {
        assert_eq!(st(&rows[c + 59]), Some(EnemyState::MagicAttack));
        assert_eq!(st(&rows[c + 60]), Some(EnemyState::Home));
    }
    assert!(rows[716].balls.is_empty());
    assert_eq!(rows[717].balls, vec![(41390, 1047)]);
    assert_eq!(rows[766].balls, vec![(41654, 1047)]);
    assert!(h.contains(&(767, 15)), "{h:?}");
    // The killing blow on row 1115 (his health set to 25 after row 1100):
    // dead where it found him, out of every collision; the gem 176 ticks
    // later from there, Claw held while it flies; the level's end.
    assert_eq!(starts(&rows, EnemyState::Dead), vec![1115]);
    assert_eq!(
        rows[1115].omar.map(|a| (a.x, a.y, a.health)),
        Some((41325, 1067, 0))
    );
    assert!(rows[1115].out);
    assert!(rows[1290].gem.is_none());
    assert_eq!(rows[1291].gem, Some(omar::HALL_LEFT));
    assert_eq!(st(&rows[1291]), Some(EnemyState::Wait));
    assert!(rows[1291].gem_given);
    for l in &rows[1291..1471] {
        assert_eq!(l.claw.2, BOSS_END);
    }
    assert_eq!(rows.iter().position(|l| l.ended), Some(1471));
}

#[test]
fn install_omar_claw_dead_at_his_falls_end_leaves_no_gem_for_good() {
    let Some(level) = level14() else { return };
    let Some(rows) = watch(&level, "boss-kill-dead-c18", "c18", 1700) else {
        return;
    };
    assert_eq!(starts(&rows, EnemyState::Dead), vec![1115]);
    assert_eq!(rows[1215].claw.2, claw_state::DEATH_FALL);
    // His fall ends on 1291 with Claw dead: no gem.
    assert!(!rows[1290].gem_given && rows[1291].gem_given);
    assert!(rows.iter().all(|l| l.gem.is_none()));
    // Claw's respawn in the chamber (1309) starts the first phase over with
    // a new ring and full health, out of every collision for good.
    assert_eq!(rows[1308].lives, 6);
    assert_eq!(rows[1309].claw.2, claw_state::STAND);
    assert_eq!(st(&rows[1309]), Some(EnemyState::Idle));
    assert_eq!(
        rows[1309].omar.map(|a| (a.x, a.y, a.health)),
        Some((42250, 2475, 100))
    );
    assert_eq!((rows[1309].phase, rows[1309].pieces.len()), (1, 9));
    assert!(rows[1309..].iter().all(|l| l.out && !l.safe));
    assert!(rows.iter().all(|l| !l.ended));
}
