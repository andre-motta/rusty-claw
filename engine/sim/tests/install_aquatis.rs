//! The twelfth level's boss room on your own install: Aquatis, his crack,
//! his dynamite helper and his five stalactites, played against the
//! original's runs of the same routes, and the fight's main outcomes
//! checked on their own.
//!
//! Each route puts Claw down after the level's first tick, the way the
//! original's runs put him there (between two ticks, as a warp does), at
//! the height he settles at, waits until the level clock reaches the time
//! the original's run started at, and starts the shared random stream
//! from the run's state, once. Then, by route:
//!
//! - the normal entry: Right from row 15 walks Claw from under the boss
//!   room's door into it; he stands through the introduction and after it
//!   (Aquatis stays still: his crack sleeps far from Claw);
//! - the intro skip: the same walk and Jump on Claw's first row in the
//!   room (31 at the original's speeds, 27 at the default ones), which
//!   leaves Aquatis waiting for the fight for good; the arrival jump takes
//!   the stick of dynamite the helper put down, and in the original's run
//!   at the default speeds the first two tentacles grab him in turn (not
//!   reached here, see [`KNOWN`]); and Jump one row late,
//!   which runs the introduction;
//! - the lift: from the hidden floor right of the tentacles, Jump into
//!   Aquatis's body at the jump's peak, Lift on the row after the stand
//!   that follows his contact hit: he is lifted, dropped at once and dies
//!   in the water; with a help screen 12 rows later the gem turns into the
//!   level's end on Claw on the return (at the original's speeds; at the
//!   default ones the lift fails here, see [`KNOWN`]); without it the
//!   gem flies off and Claw dies in the water;
//! - the crack: a full-charge throw from the ledge's end lands the stick
//!   on the block right of the water, its blast cracks the wall, the quake
//!   ends with the first stalactite on Aquatis, who reels and thrashes; a
//!   tentacle's frenzy knocks Claw into the water and the respawn resets
//!   the fight;
//! - the fight pocket (a diagnostic run: Claw carries 9 sticks from the
//!   route's time zero, given here by the start lines of the level's
//!   start): five rounds of a throw onto the crack from the pocket over
//!   the block, five stalactites, his death and the gem.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of its clock: at the original's speeds (c18) every column exactly, the
//! random stream and the boss columns included; at the default speeds
//! played tick by tick (c60) within the feel tolerances. When the reference
//! runs also list Aquatis per step (`boss.csv`) and the objects near Claw
//! (`obj-log.csv`, beside each run, in the folder named by
//! `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference store),
//! Aquatis's position, state and health and the crack's state, the
//! stalactites' and the tentacles' positions and states are compared
//! exactly at c18, their states lined up by the numbers the runs give
//! them ([`reference_state`]).
//!
//! Divergences that are known, with their causes, are listed in [`known`];
//! any other fails the test. The routes are written below ([`route`]);
//! each store's own copy of its route must agree with them, its help
//! screens included. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the
//! comparisons: each test skips itself when what it needs is unset, and
//! fails when `CLAW_REFERENCE` is set but a listed store lacks its route,
//! meta or trace (or, with the reference runs there, a run's `boss.csv` or
//! `obj-log.csv`); `CLAW_L12_ONLY` names one store to play alone,
//! `CLAW_L12_TRACE` a folder to write the replay's traces to and
//! `CLAW_L12_DUMP=FROM..TO` prints the watched objects of both on those
//! rows. Nothing from an install or a reference store is kept in the
//! repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::aquatis::{self, crack, dynamite};
use claw_sim::logic::enemy::{EnemyState, tentacle};
use claw_sim::logic::pickup;
use claw_sim::object::ObjectId;
use claw_sim::player::progress::parse_start_lines;
use claw_sim::player::state as claw_state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, StartState, TickInput};
use common::*;

/// The lift button.
const LIFT: u32 = 0x20;
/// The dynamite button.
const DYNAMITE: u32 = 0x100;

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The game time at a route's time zero, by clock.
const T0_MS: [(&str, u64); 2] = [("c18", 828), ("c60", 817)];

/// Aquatis, where he is placed.
const AQUATIS_AT: (i32, i32) = (37912, 3600);
/// The crack, where it is placed.
const CRACK_AT: (i32, i32) = (38050, 3520);
/// The five stalactites over Aquatis, by their record's x, in the order of
/// their records.
const STALACTITES: [i32; 5] = [37858, 37896, 37936, 37972, 38009];
/// The three tentacles, by their record's x.
const TENTACLES: [i32; 3] = [37397, 37523, 37648];

/// The start lines of the fight-pocket route: Claw carries 9 sticks of
/// dynamite from the level's start.
const POCKET_START: &str = "# start_score = 0\n# start_health = 100\n# start_lives = 7\n\
                            # start_pistol = 10\n# start_magic = 5\n# start_dynamite = 9\n\
                            # start_weapon = pistol\n\
                            # start_treasure_game = 0, 0, 0, 0, 0, 0, 0, 0, 0\n";

/// The stores, with their clocks.
const STORES: [(&str, &[&str]); 9] = [
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-intro-skip-c18", &["c18"]),
    ("boss-intro-skip", &["c60"]),
    ("boss-intro-skip-late-c18", &["c18"]),
    ("boss-lift-help-c18", &["c18"]),
    ("boss-lift-help", &["c60"]),
    ("boss-lift-drop-c18", &["c18"]),
    ("boss-crack-throw-c18", &["c18"]),
    ("boss-fight-pocket-c18", &["c18"]),
];

// ----------------------------------------------------------------------
// The routes

/// A route: where Claw is put down (at the height he settles at), the
/// keys held per row, its help screens (row, ms, keys pressed on it),
/// whether it ends the
/// level, and whether it starts from the pocket's start lines.
#[derive(Debug, Clone, Default)]
struct Route {
    start: (i32, i32),
    held: Vec<u32>,
    helps: Vec<(usize, u32, usize)>,
    completes: bool,
    pocket: bool,
}

impl Route {
    fn new(start: (i32, i32), rows: usize) -> Route {
        Route {
            start,
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

/// Where the intro routes put Claw: under the boss room's door.
const DOOR_WALK: (i32, i32) = (36300, 2071);

/// The route of `store`.
fn route(store: &str) -> Route {
    match store {
        "boss-intro-normal" => Route::new(DOOR_WALK, 1500).hold(RIGHT, 15, 59),
        "boss-intro-skip-c18" => Route::new(DOOR_WALK, 1500)
            .hold(RIGHT, 15, 50)
            .hold(JUMP, 31, 52),
        "boss-intro-skip" => Route::new(DOOR_WALK, 1600)
            .hold(RIGHT, 15, 46)
            .hold(JUMP, 27, 48),
        "boss-intro-skip-late-c18" => Route::new(DOOR_WALK, 1000)
            .hold(RIGHT, 15, 51)
            .hold(JUMP, 32, 53),
        "boss-lift-help-c18" => Route {
            helps: vec![(106, 7750, 1)],
            completes: true,
            ..lift_c18(300)
        },
        "boss-lift-drop-c18" => lift_c18(450),
        "boss-lift-help" => Route {
            helps: vec![(97, 7750, 1)],
            completes: true,
            ..Route::new((37656, 3799), 300)
                .hold(RIGHT, 15, 51)
                .hold(JUMP, 32, 51)
                .hold(LIFT, 73, 74)
        },
        "boss-crack-throw-c18" => Route::new((37680, 3607), 800).hold(DYNAMITE, 20, 139),
        "boss-fight-pocket-c18" => {
            let mut r = Route::new((38208, 3523), 3000).hold(LEFT, 15, 16);
            for k in [20, 580, 1140, 1700, 2260] {
                r = r.hold(DYNAMITE, k, k + 1);
            }
            Route { pocket: true, ..r }
        }
        _ => panic!("no route {store}"),
    }
}

/// The lift at the original's speeds: from the hidden floor right of the
/// tentacles, Right, Jump on row 39, Lift on row 82.
fn lift_c18(rows: usize) -> Route {
    Route::new((37652, 3799), rows)
        .hold(RIGHT, 15, 65)
        .hold(JUMP, 39, 71)
        .hold(LIFT, 82, 83)
}

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// What the walk through the door leaves behind at the original's speeds:
/// the random stream from Claw's first row in the room, the camera's
/// shake under the falling banner (its jolts draw from the stream) and
/// the crack's last row in the update pass as the camera leaves Aquatis.
const DOOR_WALK_C18: &[(&str, u64)] = &[
    ("rng", 576),
    ("crack_x", 5724),
    ("crack_y", 5724),
    ("crack_state", 5724),
    ("cam_y", 11160),
    ("cam_x", 11178),
];

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, the walk through the door (576 ms): the Siren and the Mercat
///   near the door, left behind by the warp, run their whole call in
///   Claw's first tick in the room in this build (two frame pulse draws
///   and the Siren's idle draw, after the room's first calls); in the
///   original they go to sleep before it, the Siren's idle draw coming
///   first and alone. From there the stream runs two draws ahead: the
///   banner's shake moves the camera on other jolts (11160 ms) and, after
///   the skip, the tentacles strike and grab at other times (1692 ms on).
///   In the original the crack, woken while the camera visits Aquatis, is
///   listed for one more row as the camera leaves him (5724 ms); it runs
///   no logic then.
/// - c18, the crack throw, after Claw's respawn (14724 ms): Aquatis goes
///   idle twice in the respawn (his reset, then Claw out of his range);
///   the original makes the second before the Mercat near the level's
///   start draws, this build after it, so his idle pose comes from
///   another draw (the second one, 54 rows, against the first, 61).
/// - c18, the fight pocket (2196 ms): the first blast's camera shake moves
///   the original's window over the first tentacle for one row, which
///   this build's window, on the camera's unshaken point, does not do; the
///   tentacle, built before time zero, runs one call that draws and
///   changes nothing.
/// - c60 (the default tuning played tick by tick): the walk into the door
///   ends a few pixels off the original's 60 frames per second walk (the
///   camera), the introduction's delays count 18 ms ticks against frames
///   of 16 and 17 ms (the fight starts a sixth of a second apart); the
///   skip's jump, pressed on the row the original reads on its first
///   frame in the room, comes on the last tick before the room here, so
///   Claw lands in the stager's box and the introduction runs (no grab
///   follows); the jump out of the hidden floor meets Aquatis's body
///   8 px lower than at 60 frames per second, so the stand after his
///   contact hit fails the lift's 20 px feet test and Claw falls into the
///   water (the level does not end).
const KNOWN: &[Known] = &[
    ("boss-intro-normal", "c18", DOOR_WALK_C18),
    ("boss-intro-skip-late-c18", "c18", DOOR_WALK_C18),
    (
        "boss-intro-skip-c18",
        "c18",
        &[
            ("rng", 576),
            ("tent37397_state", 1692),
            ("state_id", 6462),
            ("physics_type", 6462),
            ("x", 6786),
            ("y", 6786),
            ("cam_x", 6786),
            ("cam_y", 6786),
            ("health", 6804),
            ("tent37523_state", 6804),
            ("tent37648_state", 8748),
        ],
    ),
    ("boss-crack-throw-c18", "c18", &[("rng", 14724)]),
    (
        "boss-fight-pocket-c18",
        "c18",
        &[
            ("tent37397_x", 2196),
            ("tent37397_y", 2196),
            ("tent37397_state", 2196),
        ],
    ),
    (
        "boss-intro-normal",
        "c60",
        &[("cam_x", 1017), ("boss_fight", 13500), ("state_id", 13517)],
    ),
    (
        "boss-intro-skip",
        "c60",
        &[
            ("state_id", 517),
            ("physics_type", 517),
            ("events", 522),
            ("y", 533),
            ("x", 550),
            ("cam_y", 550),
            ("boss_fight", 567),
            ("cam_x", 633),
            ("dynamite", 800),
            ("health", 7933),
        ],
    ),
    (
        "boss-lift-help",
        "c60",
        &[
            ("state_id", 1517),
            ("y", 1550),
            ("cam_x", 1550),
            ("cam_y", 1550),
            ("health", 1567),
            ("physics_type", 1749),
            ("events", 1749),
        ],
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

/// The state a run's number for an object's state stands for, in this
/// build's numbers: Aquatis's and the tentacles' as enemy states, the
/// crack's and the stalactites' as their own.
fn reference_state(who: Who, n: i32) -> Option<i32> {
    let enemy = |s: EnemyState| Some(s as i32);
    match who {
        Who::Aquatis | Who::Tentacle(_) => match n {
            2000 => enemy(EnemyState::Idle),
            2003 => enemy(EnemyState::Strike),
            2004 => enemy(EnemyState::Dead),
            2009 => enemy(EnemyState::Lifted),
            2012 => enemy(EnemyState::Dropped),
            2028 => enemy(EnemyState::Startup),
            2087 => enemy(EnemyState::DeadAct1),
            2091 => enemy(EnemyState::HighPunch),
            2092 => enemy(EnemyState::LowPunch),
            2093 => enemy(EnemyState::SuperFrenzy),
            2094 => enemy(EnemyState::StalactiteHit),
            2095 => enemy(EnemyState::Scared),
            2097 => enemy(EnemyState::Respawn),
            2098 => enemy(EnemyState::StartGrab),
            2099 => enemy(EnemyState::Squeeze),
            _ => None,
        },
        Who::Crack => match n {
            2000 => Some(crack::state::IDLE),
            2096 => Some(crack::state::DELAY),
            _ => None,
        },
        Who::Stalactite(_) => match n {
            2069 => Some(aquatis::stalactite::state::IDLE),
            2066 => Some(aquatis::stalactite::state::FALLING),
            2073 => Some(aquatis::stalactite::state::DEAD),
            2100 => Some(aquatis::stalactite::state::WAITING),
            _ => None,
        },
    }
}

/// The objects of the room compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Aquatis,
    Crack,
    /// A stalactite over Aquatis, by its record's x.
    Stalactite(i32),
    /// A tentacle, by its record's x.
    Tentacle(i32),
}

impl Who {
    fn name(self) -> String {
        match self {
            Who::Aquatis => "aquatis".into(),
            Who::Crack => "crack".into(),
            Who::Stalactite(x) => format!("stal{x}"),
            Who::Tentacle(x) => format!("tent{x}"),
        }
    }

    fn logic(self) -> &'static str {
        match self {
            Who::Aquatis => aquatis::NAMES[0],
            Who::Crack => crack::NAMES[0],
            Who::Stalactite(_) => aquatis::stalactite::NAMES[0],
            Who::Tentacle(_) => tentacle::NAMES[0],
        }
    }

    /// Whether its health is compared.
    fn health(self) -> bool {
        self == Who::Aquatis
    }
}

fn watched() -> Vec<Who> {
    let mut v = vec![Who::Aquatis, Who::Crack];
    v.extend(STALACTITES.map(Who::Stalactite));
    v.extend(TENTACLES.map(Who::Tentacle));
    v
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

/// Aquatis by step in a run's `boss.csv`: `None` on the steps without him.
fn reference_aquatis(path: &Path) -> Option<Vec<(usize, Option<At>)>> {
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
                state: reference_state(Who::Aquatis, n),
                health: f.get(health)?.parse().ok()?,
            })
        })();
        out.push((k, at));
    }
    Some(out)
}

/// The crack, the stalactites and the tentacles by step in a run's
/// `obj-log.csv`.
fn reference_room(path: &Path) -> Option<Vec<(usize, Who, At)>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, logic, st, x, y) = (
        col("tick")?,
        col("serial")?,
        col("logic_name")?,
        col("state")?,
        col("x")?,
        col("y")?,
    );
    // Each object is told apart by where it was when first logged.
    let mut first: Vec<(String, i32)> = Vec::new();
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let name = *f.get(logic)?;
        let s = f.get(serial)?.to_string();
        let x0 = match first.iter().find(|(id, _)| *id == s) {
            Some(&(_, x0)) => x0,
            None => {
                let x0: i32 = f.get(x)?.parse().ok()?;
                first.push((s, x0));
                x0
            }
        };
        let who = match name {
            "AquatisCrack" => Who::Crack,
            "AquatisStalactite" => Who::Stalactite(x0),
            "Tentacle" => Who::Tentacle(x0),
            _ => continue,
        };
        let raw: i32 = f.get(st)?.parse().ok()?;
        out.push((
            f.get(tick)?.parse().ok()?,
            who,
            At {
                x: f.get(x)?.parse().ok()?,
                y: f.get(y)?.parse().ok()?,
                state: reference_state(who, raw),
                health: 0,
            },
        ));
    }
    Some(out)
}

// ----------------------------------------------------------------------
// Playing

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

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route) -> Sim {
    let mut sim = if r.pocket {
        let carry = parse_start_lines(POCKET_START)
            .expect("the start lines read")
            .expect("start lines");
        Sim::with_start(
            level.clone(),
            config(clock),
            StartState {
                carry: Some(carry),
                save: None,
            },
        )
    } else {
        Sim::new(level.clone(), config(clock))
    };
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(r.start));
    while sim.tick() * TICK < t0_ms(clock) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(SEED));
    sim
}

/// The object of `who`, once the level has it.
fn find(sim: &Sim, who: Who) -> Option<ObjectId> {
    let at = |x: i32| -> Option<ObjectId> {
        sim.objects()
            .all()
            .filter(|(_, o)| &*o.logic == who.logic())
            .find(|(_, o)| o.editor.0 == x)
            .map(|(id, _)| id)
    };
    match who {
        Who::Aquatis => at(AQUATIS_AT.0),
        Who::Crack => at(CRACK_AT.0),
        Who::Stalactite(x) | Who::Tentacle(x) => at(x),
    }
}

/// A watched object of the replay while it is in the update pass (the
/// original's runs list only those) and built.
fn look(sim: &Sim, id: Option<ObjectId>) -> Option<At> {
    let o = sim.objects().get(id?)?;
    if !o.is_listed() || o.state == claw_sim::object::engine_state::INIT {
        return None;
    }
    let own = matches!(&*o.logic, "AquatisCrack" | "AquatisStalactite");
    let state = (own || EnemyState::from_id(o.state).is_some()).then_some(o.state);
    Some(At {
        x: o.x,
        y: o.y,
        state,
        health: o.health,
    })
}

/// One play of a route.
struct Played {
    rows: Vec<TraceRow>,
    /// The watched objects per row, in [`watched`] order.
    room: Vec<Vec<Option<At>>>,
    completed: bool,
}

/// Plays `r` at `clock` from its time zero for `ticks` ticks.
fn play(level: &SimLevel, r: &Route, clock: &str, ticks: usize) -> Played {
    let mut sim = at_time_zero(level, clock, r);
    let who = watched();
    let mut ids: Vec<Option<ObjectId>> = who.iter().map(|&w| find(&sim, w)).collect();
    let input = r.ticks();
    let mut rows = Vec::new();
    let mut room = Vec::new();
    let mut completed = false;
    for k in 0..ticks {
        for &(_, ms, keys) in r.helps.iter().filter(|(at, ..)| *at == k) {
            sim.pass_help_return(ms, keys);
        }
        sim.step(input.get(k).copied().unwrap_or_default());
        completed |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &w) in who.iter().enumerate() {
            if ids[i].is_none() {
                ids[i] = find(&sim, w);
            }
        }
        room.push(ids.iter().map(|&id| look(&sim, id)).collect());
    }
    Played {
        rows,
        room,
        completed,
    }
}

/// The first step (as a reference time, ms) at which each watched
/// object's x, y, state or health leaves the run's.
fn compare_room(
    played: &Played,
    aquatis_ref: &[(usize, Option<At>)],
    room_ref: &[(usize, Who, At)],
) -> Vec<(String, u64)> {
    let who = watched();
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: String, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    let mut check = |w: Who, k: usize, want: Option<At>, got: Option<At>| {
        let name = w.name();
        match (want, got) {
            (None, None) => {}
            (Some(want), Some(got)) => {
                for (col, same) in [
                    ("x", got.x == want.x),
                    ("y", got.y == want.y),
                    ("state", want.state.is_some() && got.state == want.state),
                    ("health", !w.health() || got.health == want.health),
                ] {
                    if !same {
                        note(format!("{name}_{col}"), k);
                    }
                }
            }
            _ => {
                for col in ["x", "y", "state"] {
                    note(format!("{name}_{col}"), k);
                }
            }
        }
    };
    for &(k, want) in aquatis_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        check(Who::Aquatis, k, want, row[0]);
    }
    for &(k, w, want) in room_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        let Some(i) = who.iter().position(|x| *x == w) else {
            continue;
        };
        check(w, k, Some(want), row[i]);
    }
    firsts
}

/// With `CLAW_L12_DUMP=FROM..TO`, prints the watched objects of the play
/// and of the run on those rows, for looking into a divergence.
fn dump(played: &Played, aquatis_ref: &[(usize, Option<At>)], room_ref: &[(usize, Who, At)]) {
    let Some((a, b)) = std::env::var("CLAW_L12_DUMP").ok().and_then(|v| {
        let (a, b) = v.split_once("..")?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }) else {
        return;
    };
    let who = watched();
    for k in a..b.min(played.room.len()) {
        let claw = &played.rows[k];
        println!(
            "row {k}: claw {} {} s{} h{} rng {:?}",
            claw.x, claw.y, claw.state_id, claw.health, claw.rng
        );
        let theirs = aquatis_ref.iter().find(|r| r.0 == k).and_then(|r| r.1);
        println!("  aquatis ours {:?} ref {:?}", played.room[k][0], theirs);
        for (i, w) in who.iter().enumerate().skip(1) {
            let theirs = room_ref.iter().find(|r| r.0 == k && r.1 == *w).map(|r| r.2);
            if theirs.is_some() || played.room[k][i].is_some() {
                println!(
                    "  {} ours {:?} ref {:?}",
                    w.name(),
                    played.room[k][i],
                    theirs
                );
            }
        }
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

/// The store's own copy of its route: (held, pressed) by row, and its help
/// screens (row, ms, keys pressed on it).
#[allow(clippy::type_complexity)]
fn store_route(dir: &Path) -> Option<(Vec<(u32, u32)>, Vec<(usize, u32, usize)>)> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut helps = Vec::new();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix('@') {
            let f: Vec<&str> = rest.split_whitespace().collect();
            assert_eq!(
                (f.get(1), f.get(2)),
                (Some(&"menu"), Some(&"help")),
                "{l}: only help screens are expected"
            );
            let keys = f
                .iter()
                .find_map(|w| w.strip_prefix("keys="))
                .map_or(0, |k| k.split(',').filter(|k| !k.is_empty()).count());
            helps.push((f[0].parse().ok()?, f.get(3)?.parse().ok()?, keys));
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, helps))
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for (store, clocks) in STORES {
        let dir = root.join("level12").join(store);
        let (rows, helps) =
            store_route(&dir).unwrap_or_else(|| panic!("{store}: no route in the store"));
        let r = route(store);
        let ours = r.rows();
        assert_eq!(ours.len(), rows.len(), "{store}: rows");
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{store}: first different row");
        assert_eq!(r.helps, helps, "{store}: help screens");
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        assert_eq!(
            meta_value(&meta, "", "diagnostic").as_deref() == Some("true"),
            r.pocket,
            "{store}: only the fight pocket starts with more sticks"
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
            assert_eq!(x, r.start.0, "{store}: start");
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_aquatis_routes_follow_the_reference_traces() {
    let Some(level) = level12() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L12_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for (store, clocks) in STORES {
        if only.as_deref().is_some_and(|o| o != store) {
            continue;
        }
        let dir = root.join("level12").join(store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        let r = route(store);
        for &clock in clocks {
            println!("== level12/{store} {clock}");
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
            if let Some(out) = std::env::var_os("CLAW_L12_TRACE") {
                let out = PathBuf::from(out).join(format!("{store}-{clock}.csv"));
                std::fs::write(&out, trace::to_csv(&played.rows)).expect("the trace is written");
            }
            let mut room = Vec::new();
            if clock == "c18" {
                match &runs {
                    Some(runs) => {
                        let id = run_id(&meta, clock)
                            .unwrap_or_else(|| panic!("{store}: no run named in its meta"));
                        let run = runs.join(id);
                        let boss = reference_aquatis(&run.join("boss.csv"))
                            .unwrap_or_else(|| panic!("{store}: the run has no boss.csv"));
                        let objs = reference_room(&run.join("obj-log.csv"))
                            .unwrap_or_else(|| panic!("{store}: the run has no obj-log.csv"));
                        room = compare_room(&played, &boss, &objs);
                        dump(&played, &boss, &objs);
                    }
                    None => println!("the reference runs are not there: the room is not compared"),
                }
            }
            // The help screen after the lift ends the level at the
            // original's speeds; at the default ones the lift fails (see
            // `KNOWN`).
            let ends = r.completes && clock == "c18";
            if ends != played.completed {
                failures.push(format!(
                    "{store} {clock}: the level's end {}",
                    if ends { "does not come" } else { "comes" }
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
    dynamite: i32,
    fight: u8,
    aquatis: Option<At>,
    link: aquatis::AquatisLink,
    crack: Option<i32>,
    stalactites: Vec<Option<At>>,
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
    let mut ids: Vec<Option<ObjectId>> = watched().iter().map(|&w| find(&sim, w)).collect();
    for k in 0..rows {
        for &(_, ms, keys) in r.helps.iter().filter(|(at, ..)| *at == k) {
            sim.pass_help_return(ms, keys);
        }
        sim.step(input.get(k).copied().unwrap_or_default());
        for (i, &w) in watched().iter().enumerate() {
            if ids[i].is_none() {
                ids[i] = find(&sim, w);
            }
        }
        let c = sim.player().expect("Claw");
        let gem = sim
            .objects()
            .all()
            .find(|(_, o)| &*o.logic == pickup::END_GEM || o.logic.contains("EndOfLevel"))
            .map(|(_, o)| (o.x, o.y, &*o.logic == pickup::END_GEM));
        out.push(Look {
            claw: (c.x, c.y, c.state),
            claw_health: sim.claw().stats.health,
            dynamite: sim.claw().stats.dynamite,
            fight: sim.boss().fight(),
            aquatis: look(&sim, ids[0]),
            link: *sim.aquatis_link(),
            crack: look(&sim, ids[1]).and_then(|a| a.state),
            stalactites: ids[2..7].iter().map(|&id| look(&sim, id)).collect(),
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
    l.aquatis
        .and_then(|a| a.state)
        .and_then(EnemyState::from_id)
}

/// The rows on which Aquatis's state turns to `s`.
fn starts(rows: &[Look], s: EnemyState) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| st(&rows[k]) == Some(s) && (k == 0 || st(&rows[k - 1]) != Some(s)))
        .collect()
}

#[test]
fn install_aquatis_the_staging_freezes_him_until_the_crack_wakes() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "boss-intro-normal", "c18", 1500);
    // Built on Claw's first row in the room, waiting for the fight.
    let first = rows
        .iter()
        .position(|l| l.aquatis.is_some())
        .expect("built");
    assert_eq!(first, 31);
    assert_eq!(st(&rows[31]), Some(EnemyState::Startup));
    assert!(rows[31].link.refill_outstanding && rows[31].link.alive);
    // The fight flag on N + 736, the reset and Claw standing on N + 737.
    let fight = rows.iter().position(|l| l.fight == 2).expect("the fight");
    assert_eq!(fight, 31 + 736);
    assert_eq!(st(&rows[fight]), Some(EnemyState::Startup));
    assert_eq!(st(&rows[fight + 1]), Some(EnemyState::Idle));
    assert_eq!(rows[fight + 1].claw.2, claw_state::STAND);
    assert_eq!(rows[fight + 1].link.reset_bits, aquatis::RESET_BITS);
    // Every helper but the sleeping crack clears its bit; he stays still.
    for l in &rows[fight + 2..] {
        assert_eq!(l.link.reset_bits, aquatis::CRACK_BIT);
        assert_eq!(st(l), Some(EnemyState::Idle));
    }
    assert!(!rows[fight + 1].link.refill_outstanding);
}

#[test]
fn install_aquatis_the_intro_skip_leaves_him_waiting_at_full_health() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "boss-intro-skip-c18", "c18", 1500);
    assert!(rows.iter().all(|l| l.fight == 0));
    for l in &rows[31..] {
        assert_eq!(st(l), Some(EnemyState::Startup));
        assert_eq!(l.aquatis.map(|a| a.health), Some(aquatis::HEALTH));
        assert_eq!(l.link.reset_bits, 0);
    }
    // The arrival jump takes the helper's stick on row 49.
    assert_eq!(rows[48].dynamite, 3);
    assert_eq!(rows[49].dynamite, 4);
    assert!(rows[48].link.refill_outstanding);
    assert!(!rows[50].link.refill_outstanding);
    // One row late, the introduction runs.
    let late = watch(&level, "boss-intro-skip-late-c18", "c18", 800);
    assert_eq!(late.iter().position(|l| l.fight == 2), Some(31 + 736));
}

#[test]
fn install_aquatis_the_lift_drops_him_into_the_water_and_the_help_ends_the_level() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "boss-lift-drop-c18", "c18", 450);
    // His contact hit at the jump's peak, the stand in mid air, the lift.
    assert_eq!(rows[59].claw_health, 90);
    assert_eq!(rows[59].claw.1, 3654);
    assert_eq!(rows[81].claw.2, claw_state::STAND);
    assert_eq!(st(&rows[82]), Some(EnemyState::Lifted));
    assert_eq!(starts(&rows, EnemyState::Dropped), vec![83]);
    let dropped = rows[83].aquatis.expect("him");
    assert_eq!((dropped.x, dropped.y), (37856, 3639));
    // He dies in the water 12 rows later; the gem rises from there with
    // Claw held in mid air.
    assert!(rows[93].aquatis.is_some());
    let gem = rows[94].gem.expect("the gem");
    assert!(gem.2, "flying");
    assert_eq!(rows[94].claw.2, claw_sim::player::special::BOSS_END);
    // Without help it lands far away and Claw dies in the water.
    let landed = rows.iter().position(|l| l.gem.is_some_and(|g| !g.2));
    assert_eq!(landed, Some(236));
    assert!(rows[240].claw.2 != claw_sim::player::special::BOSS_END);
    assert!(rows.iter().all(|l| !l.ended));
    // With the help screen the gem is on Claw on the return row.
    let help = watch(&level, "boss-lift-help-c18", "c18", 107);
    assert!(help[106].ended, "the level ends");
    assert!(help[..106].iter().all(|l| !l.ended));
}

#[test]
fn install_aquatis_five_stalactites_kill_him_and_release_the_gem() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "boss-fight-pocket-c18", "c18", 2900);
    assert_eq!(rows[0].dynamite, 9);
    // Each round: the crack's quake, the stalactite in order, his reel.
    let hits = starts(&rows, EnemyState::StalactiteHit);
    assert_eq!(hits, vec![362, 921, 1481, 2042]);
    let quakes: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].link.quake && !rows[k - 1].link.quake)
        .collect();
    assert_eq!(quakes, vec![120, 680, 1240, 1800, 2360]);
    for (h, health) in hits.iter().zip([80, 60, 40, 20]) {
        assert_eq!(rows[*h].aquatis.map(|a| a.health), Some(health));
    }
    // The reel lasts 54 rows, then the frenzy.
    assert_eq!(st(&rows[415]), Some(EnemyState::StalactiteHit));
    assert_eq!(st(&rows[416]), Some(EnemyState::SuperFrenzy));
    assert!(rows[416].link.frenzy);
    // From the second round on he is idle when the quake starts and cowers
    // from the next row until the stalactite hits him.
    assert_eq!(
        starts(&rows, EnemyState::Scared),
        vec![681, 1241, 1801, 2361]
    );
    assert_eq!(st(&rows[920]), Some(EnemyState::Scared));
    // The fifth: his death, 112 rows, then the gem flies 140 rows.
    assert_eq!(starts(&rows, EnemyState::DeadAct1), vec![2602]);
    assert_eq!(rows[2602].aquatis.map(|a| a.health), Some(0));
    assert!(rows[2713].link.alive);
    assert!(!rows[2714].link.alive);
    assert!(rows[2714].gem.is_some_and(|g| g.2));
    assert_eq!(rows[2714].claw.2, claw_sim::player::special::BOSS_END);
    let landed = rows.iter().position(|l| l.gem.is_some_and(|g| !g.2));
    assert_eq!(landed, Some(2714 + aquatis::GEM_FLIGHT_TICKS as usize));
}

#[test]
fn install_aquatis_a_full_charge_throw_from_the_ledge_cracks_the_wall() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "boss-crack-throw-c18", "c18", 800);
    let quake = (1..rows.len()).find(|&k| rows[k].link.quake && !rows[k - 1].link.quake);
    assert_eq!(quake, Some(225));
    assert_eq!(rows[225].crack, Some(crack::state::DELAY));
    // The first stalactite in order (x 37936) hits him while he waits.
    assert_eq!(starts(&rows, EnemyState::StalactiteHit), vec![467]);
    assert_eq!(st(&rows[466]), Some(EnemyState::Startup));
    let third = rows[467].stalactites[2].expect("the stalactite at 37936");
    assert_eq!(third.y, 3480);
    // Claw's respawn resets the fight: full health again.
    let respawn = (1..rows.len())
        .find(|&k| rows[k].aquatis.map(|a| a.health) == Some(100) && k > 467)
        .expect("the reset");
    assert_eq!(respawn, 763);
    assert_eq!(rows[respawn].link.reset_bits, 0);
    // The dynamite helper put nothing more down: its first stick is there.
    assert!(rows[respawn - 1].link.refill_outstanding);
    let _ = dynamite::STICK_AT;
}
