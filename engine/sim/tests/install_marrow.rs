//! The tenth level's boss room on your own install: Marrow, his parrot and
//! the two slabs of the trap floor, played against the original's runs of
//! the same routes, and the fight's main outcomes checked on their own.
//!
//! Every route starts on the floor of the niche left of the door into his
//! room: Claw is put at (32340, 7740) on the level's first tick and left
//! there until the route's time zero (828 ms of game time at the
//! original's speeds, 817 ms at the default ones, as in the reference
//! runs), then the random stream is set to the store's state at time zero,
//! once. Right held from row 15 walks him into the door, which puts him
//! inside the stager's box; Claw's first tick in the room is row 27 at the
//! original's speeds and row 25 at the default ones. Then, by route:
//!
//! - the normal entry: Claw stands through the introduction and the fight;
//!   or swings his sword on the parrot's first three swoops (it perches and
//!   the slabs close), walks the bridge, jumps onto Marrow's ledge and
//!   swings every 12 rows (Marrow's shot, 25 damage, the whistle, the
//!   carry); or the same and then a step down onto the right slab, which
//!   the carry opens under him; or one pistol shot at the parrot's first
//!   swoop;
//! - the bridge trick: the normal fight to five hits on Marrow (and, at
//!   the original's speeds, a sixth blow he blocks), then Left and a jump
//!   off his ledge as he whistles, so that Claw is in the air on Marrow's
//!   half when the parrot starts to carry him (to the left ledge) and lands
//!   on the left slab as it opens; a second jump off the slab onto the left
//!   ledge, the fight there (crouched stabs at the original's speeds,
//!   standing swings at the default ones) while the parrot never perches
//!   again, and the jump into the gem;
//! - the skip: Right and Jump on Claw's first row in the room (he lands on
//!   the left ledge outside the stager's box), one row late (the
//!   introduction runs), then a walk back into the stager's box;
//! - the reference crossing after the skip: a stick of dynamite thrown in
//!   the air, a jump off the ledge's edge into the blast over the pit, the
//!   jump out of the blast's hit onto Marrow's ledge, sword swings until he
//!   dies and the jump into the gem over the pit; the same with the
//!   reference run's pauses over the hit (pausing changes nothing); and,
//!   with Claw's respawn point moved to the left ledge at time zero (a
//!   diagnostic run: no checkpoint of the level gives him that point), the
//!   crossing, four hits, a life ended at row 300, the crossing again and
//!   swings until Marrow dies.
//!
//! The routes are written below ([`routes`]); each store's own copy of its
//! route must agree with them. Each route is played tick by tick and
//! compared with the reference trace of its clock: at the original's
//! speeds (c18) every column exactly, the random stream and the boss
//! columns included; at the default speeds played tick by tick (c60)
//! within the feel tolerances. When the reference runs also list Marrow
//! per step (`boss.csv`) and the objects near Claw (`obj-log.csv`, beside
//! each run, in the folder named by `CLAW_REFERENCE_RUNS`, by default `rt`
//! next to the reference store), Marrow's position, state and health and
//! the parrot's and both slabs' positions and states are compared exactly
//! at c18, their states lined up by the numbers the runs give them
//! ([`REFERENCE_STATES`]).
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons: each test skips itself when what it needs is
//! unset, and fails when `CLAW_REFERENCE` is set but a listed store lacks
//! its route, meta or trace (or, with the reference runs there, a run's
//! `boss.csv` or `obj-log.csv`); `CLAW_L10_ONLY` names one store to play
//! alone. Nothing from an install or a reference store is kept in the
//! repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{EnemyState, marrow, parrot};
use claw_sim::logic::marrow_floor;
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;
const PISTOL: u32 = 0x4;
const DYNAMITE: u32 = 0x100;

/// Where every route starts: the floor of the niche left of the door.
const START: (i32, i32) = (32340, 7740);
/// The random state of every store at its time zero.
const SEED: u32 = 111_567;
/// The game time at a route's time zero, by clock.
const T0_MS: [(&str, u64); 2] = [("c18", 828), ("c60", 817)];
/// The respawn point of the diagnostic route, on the left ledge.
const DIAGNOSTIC_CHECKPOINT: (i32, i32) = (40600, 7245);

/// Right held from this row walks Claw into the door.
const WALK: usize = 15;

/// Claw's first row in the room, by clock.
fn arrive(clock: &str) -> usize {
    if clock == "c18" { 27 } else { 25 }
}

// ----------------------------------------------------------------------
// The routes

/// A route: the keys held per row, and the lines that take effect at a
/// row's boundary (a pause or a menu, which change nothing in the game,
/// and a life ended).
#[derive(Debug, Clone, Default)]
struct Route {
    held: Vec<u32>,
    directives: Vec<(usize, String)>,
    /// Claw's respawn point set at time zero.
    checkpoint: Option<(i32, i32)>,
}

impl Route {
    fn new(rows: usize) -> Route {
        Route {
            held: vec![0; rows],
            ..Route::default()
        }
    }

    /// Holds `mask` on rows `from..to`.
    fn hold(mut self, mask: u32, from: usize, to: usize) -> Route {
        let to = to.min(self.held.len());
        for h in self.held.iter_mut().take(to).skip(from) {
            *h |= mask;
        }
        self
    }

    /// Holds `mask` two rows long every `gap` rows from `from` to `to`.
    fn taps(mut self, mask: u32, from: usize, to: usize, gap: usize) -> Route {
        for k in (from..to).step_by(gap) {
            self = self.hold(mask, k, k + 2);
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
                let row = (h, h & !prev);
                prev = h;
                row
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

    /// Plays the route's in-game menu and End Life due before row `k`, in
    /// the route's order: the menu's frames move the frame pulse
    /// ([`Sim::pass_ingame_menu`]), and the End Life ends the life.
    fn play_holds(&self, sim: &mut Sim, k: usize) {
        for (row, d) in &self.directives {
            if *row != k {
                continue;
            }
            if let Some(ms) = d.strip_prefix("menu ingame ") {
                sim.pass_ingame_menu(ms.parse().expect("a menu's time"));
            } else if d == "command end_life" {
                sim.end_life();
            }
        }
    }
}

/// The normal entry: Right from row 15 to the first row in the room.
fn normal(rows: usize) -> Route {
    Route::new(rows).hold(RIGHT, WALK, 27)
}

/// The skip, `late` rows after Claw's first row in the room at `clock`.
fn skip(clock: &str, late: usize, rows: usize) -> Route {
    let j = arrive(clock) + late;
    Route::new(rows)
        .hold(RIGHT, WALK, j + 20)
        .hold(JUMP, j, j + 22)
}

/// The skip and the dynamite crossing onto Marrow's ledge at `clock`.
fn cross(r: Route, clock: &str) -> Route {
    let a = arrive(clock);
    let (thr, right_after, j2, hj) = if clock == "c18" {
        (53, 13, 88, 155)
    } else {
        (45, 8, 78, 144)
    };
    let r = if clock == "c18" {
        r.hold(RIGHT, WALK, thr)
            .hold(RIGHT | DYNAMITE, thr, thr + 10)
            .hold(RIGHT, thr + 10, thr + right_after)
    } else {
        r.hold(RIGHT, WALK, thr + right_after)
            .hold(DYNAMITE, thr, thr + 10)
    };
    r.hold(JUMP, a, thr)
        .hold(RIGHT, j2 - 2, j2 + 53)
        .hold(JUMP, j2, j2 + 53)
        .hold(RIGHT | JUMP, hj, hj + 20)
        .hold(RIGHT, hj + 20, hj + 40)
}

/// The normal fight at `clock`: three swings on the parrot's swoops, the
/// bridge, the jump onto Marrow's ledge.
fn fight_start(clock: &str, rows: usize) -> Route {
    let (swings, spans): (&[usize], &[(u32, usize, usize)]) = if clock == "c18" {
        (
            &[1030, 1238, 1452],
            &[(RIGHT, 1580, 1720), (JUMP, 1690, 1705)],
        )
    } else {
        (
            &[1029, 1246, 1457],
            &[(RIGHT, 1583, 1723), (JUMP, 1693, 1708), (RIGHT, 1737, 1745)],
        )
    };
    let mut r = normal(rows);
    for &s in swings {
        r = r.hold(ATTACK, s, s + 2);
    }
    for &(m, a, b) in spans {
        r = r.hold(m, a, b);
    }
    r
}

/// The reference run's pauses over the blast's hit: (row, wall ms, held),
/// already moved onto this route's rows.
const PAUSES: [(usize, u32, u32); 18] = [
    (132, 417, 0),
    (133, 300, 0),
    (135, 300, 0),
    (136, 283, 0),
    (138, 283, 0),
    (139, 283, 0),
    (140, 300, 0),
    (141, 300, 0),
    (142, 283, 0),
    (143, 267, 0),
    (145, 333, 0),
    (147, 267, 0),
    (148, 300, 0),
    (149, 267, 0),
    (150, 300, 0),
    (151, 267, 0),
    (153, 267, 0),
    (155, 583, RIGHT | JUMP),
];

/// The route of `store`.
fn route(store: &str) -> Route {
    match store {
        "boss-intro-normal" => normal(2400),
        "boss-normal-fight-c18" => fight_start("c18", 2700).taps(ATTACK, 1725, 2690, 12),
        "boss-normal-fight" => fight_start("c60", 2800).taps(ATTACK, 1749, 2790, 12),
        "boss-normal-ride-c18" => fight_start("c18", 2300)
            .taps(ATTACK, 1725, 1870, 12)
            .hold(LEFT, 1878, 1888),
        "boss-parrot-pistol-c18" => normal(1300).hold(PISTOL, 1014, 1016),
        "boss-intro-skip-c18" => skip("c18", 0, 2000),
        "boss-intro-skip" => skip("c60", 0, 2000),
        "boss-intro-skip-late-c18" => skip("c18", 1, 1100),
        "boss-intro-skip-late" => skip("c60", 1, 1100),
        "boss-intro-skip-return-c18" => skip("c18", 0, 2000).hold(LEFT, 27 + 80, 27 + 96),
        "boss-intro-skip-return" => skip("c60", 0, 2000).hold(LEFT, 25 + 80, 25 + 96),
        "boss-skip-cross-kill-c18" => cross(Route::new(1000), "c18")
            .hold(RIGHT, 200, 206)
            .taps(ATTACK, 210, 680, 12)
            .hold(LEFT, 830, 880)
            .hold(JUMP, 840, 865),
        "boss-skip-cross-kill" => cross(Route::new(1300), "c60")
            .taps(ATTACK, 200, 1030, 12)
            .hold(LEFT, 1092, 1146)
            .hold(JUMP, 1106, 1131),
        "boss-skip-cross-pause-c18" => {
            let mut r = route("boss-skip-cross-kill-c18");
            r.directives = PAUSES
                .iter()
                .map(|&(k, ms, h)| (k, format!("pause {ms} held={h:#x}")))
                .collect();
            r
        }
        "boss-skip-death-fight-c18" => {
            let thr = 400 + 26;
            let mut r = cross(Route::new(2700), "c18")
                .hold(RIGHT, 200, 206)
                .taps(ATTACK, 210, 290, 12)
                .hold(JUMP, 400, thr)
                .hold(RIGHT | DYNAMITE, thr, thr + 10)
                .hold(RIGHT, thr + 10, thr + 13)
                .hold(RIGHT, thr + 35, thr + 35 + 53)
                .hold(JUMP, thr + 35, thr + 35 + 53)
                .hold(RIGHT | JUMP, 528, 548)
                .hold(RIGHT, 548, 568)
                .hold(RIGHT, 573, 579)
                .taps(ATTACK, 583, 1090, 12)
                .hold(RIGHT, 1100, 1106)
                .taps(ATTACK, 1110, 2680, 12);
            r.directives = vec![
                (300, "menu ingame 500".to_string()),
                (300, "command end_life".to_string()),
            ];
            r.checkpoint = Some(DIAGNOSTIC_CHECKPOINT);
            r
        }
        "boss-bridge-trick-c18" => fight_start("c18", 4400)
            .taps(ATTACK, 1725, 1858, 12)
            .hold(LEFT, 1870, 1960)
            .hold(JUMP, 1870, 1890)
            .hold(JUMP, 1918, 1938)
            .hold(RIGHT, 1965, 1967)
            .hold(DOWN, 1970, 4060)
            .taps(ATTACK, 1975, 4055, 12)
            .hold(RIGHT, 4195, 4255)
            .hold(JUMP, 4210, 4235),
        "boss-bridge-trick" => fight_start("c60", 3600)
            .taps(ATTACK, 1749, 1930, 12)
            .hold(LEFT, 1960, 2043)
            .hold(JUMP, 1960, 1980)
            .hold(JUMP, 2008, 2028)
            .hold(RIGHT, 2048, 2050)
            .taps(ATTACK, 2058, 3285, 12)
            .hold(RIGHT, 3310, 3370)
            .hold(JUMP, 3330, 3355),
        other => panic!("no route {other}"),
    }
}

/// The stores and the clocks they were run at.
const STORES: [(&str, &[&str]); 17] = [
    ("boss-intro-normal", &["c18", "c60"]),
    ("boss-normal-fight-c18", &["c18"]),
    ("boss-normal-fight", &["c60"]),
    ("boss-normal-ride-c18", &["c18"]),
    ("boss-parrot-pistol-c18", &["c18"]),
    ("boss-intro-skip-c18", &["c18"]),
    ("boss-intro-skip", &["c60"]),
    ("boss-intro-skip-late-c18", &["c18"]),
    ("boss-intro-skip-late", &["c60"]),
    ("boss-intro-skip-return-c18", &["c18"]),
    ("boss-intro-skip-return", &["c60"]),
    ("boss-skip-cross-kill-c18", &["c18"]),
    ("boss-skip-cross-kill", &["c60"]),
    ("boss-skip-cross-pause-c18", &["c18"]),
    ("boss-skip-death-fight-c18", &["c18"]),
    ("boss-bridge-trick-c18", &["c18"]),
    ("boss-bridge-trick", &["c60"]),
];

// ----------------------------------------------------------------------
// Known divergences

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, the ride (38268 ms) and the diagnostic run (7362 ms): after
///   Claw's death the parrot climbs from its perch again, and at one row of
///   the climb its height lands a hair under a whole pixel in the
///   original, which counts each step's game time in seconds as a sum of
///   double-precision differences, and on it in this build, which counts
///   whole 18 ms steps: the bird is drawn one pixel higher there.
/// - c18, the bridge trick, Marrow's hops (40698 ms): standing, Marrow's
///   exact height creeps a part of a pixel below his whole pixel and back
///   ([`marrow::MarrowData::stand_fraction`]); after the parrot drops him
///   on the left ledge the original's creep runs on a phase this build
///   does not follow yet, so some of his dodge hops there start a part of
///   a pixel higher and are drawn one pixel higher through their arc
///   (every state and his position across the same).
/// - c18, the bridge trick, the parrot's late swoop (66258 ms): the same
///   rounding of the step time as the ride's climb above, on one row
///   of a swoop's set-up across.
/// - c18, the diagnostic run, after the life ended from the in-game menu
///   at row 300 (whose frames keep the frame pulse going, as in the
///   original), Marrow's height (28746 ms), not traced yet; the bridge
///   trick below parts on the same column.
/// - c60 (the default tuning played tick by tick): the walk into the door
///   ends a pixel off the original's 60 frames per second walk (the
///   checkpoint event), the camera's follow differs by a few pixels, the
///   introduction's delays count 18 ms ticks against frames of 16 and
///   17 ms (the fight starts about a quarter of a second apart), and the
///   parrot's swoops then reach Claw at other times. A door's warp runs
///   the awake objects once more with no game time, and in the normal
///   fight and the bridge trick the draws of that pass move the swoops
///   the replay's 18 ms ticks play: the first one reaches Claw at 23033
///   ms (at 18 ms with the original's speeds the same routes follow the
///   runs exactly, the warp's pass included). The crossing's blast
///   hits Claw 35 px lower than at 60 frames per second: his jump out of it
///   meets the right ledge's wall and he falls onto the spikes, so that
///   route does not reach Marrow at this clock. The bridge trick at this
///   clock plays the normal fight's divergences: Claw dies in the swoops
///   before the bridge closes, so the trick is not played.
const KNOWN: &[Known] = &[
    ("boss-normal-ride-c18", "c18", &[("parrot_y", 38268)]),
    (
        "boss-bridge-trick-c18",
        "c18",
        &[("marrow_y", 40698), ("parrot_x", 66258)],
    ),
    ("boss-skip-death-fight-c18", "c18", &[("marrow_y", 28746)]),
    (
        "boss-intro-normal",
        "c60",
        &[
            ("events", 467),
            ("cam_x", 833),
            ("boss_fight", 13683),
            ("state_id", 13700),
            ("health", 18783),
            ("x", 18833),
        ],
    ),
    (
        "boss-normal-fight",
        "c60",
        &[
            ("events", 467),
            ("cam_x", 833),
            ("boss_fight", 13683),
            ("state_id", 13700),
            ("health", 23033),
            ("x", 23083),
            ("physics_type", 29117),
            ("y", 29200),
            ("cam_y", 29200),
            ("boss_health", 32217),
            ("lives", 32600),
        ],
    ),
    (
        "boss-bridge-trick",
        "c60",
        &[
            ("events", 467),
            ("cam_x", 833),
            ("boss_fight", 13683),
            ("state_id", 13700),
            ("health", 23033),
            ("x", 23083),
            ("physics_type", 29117),
            ("y", 29200),
            ("cam_y", 29200),
            ("boss_health", 32217),
            ("lives", 32600),
        ],
    ),
    (
        "boss-intro-skip-late",
        "c60",
        &[
            ("events", 467),
            ("cam_x", 833),
            ("boss_fight", 13683),
            ("state_id", 13700),
            ("health", 18783),
            ("x", 18833),
        ],
    ),
    (
        "boss-intro-skip-return",
        "c60",
        &[
            ("events", 2083),
            ("cam_x", 2417),
            ("boss_fight", 15300),
            ("state_id", 15317),
            ("health", 18700),
            ("x", 18750),
        ],
    ),
    (
        "boss-skip-cross-kill",
        "c60",
        &[
            ("y", 2233),
            ("cam_y", 2233),
            ("cam_x", 2317),
            ("state_id", 2583),
            ("physics_type", 2583),
            ("x", 2833),
            ("health", 2850),
            ("boss_health", 4350),
            ("events", 5850),
            ("lives", 5883),
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

/// The numbers the reference runs give the states of Marrow and his
/// parrot.
const REFERENCE_STATES: [(i32, EnemyState); 26] = [
    (2000, EnemyState::Idle),
    (2001, EnemyState::Falling),
    (2003, EnemyState::Strike),
    (2004, EnemyState::Dead),
    (2005, EnemyState::Jump),
    (2011, EnemyState::Pause),
    (2013, EnemyState::MoveBack),
    (2014, EnemyState::MoveAhead),
    (2015, EnemyState::BlockHigh),
    (2016, EnemyState::BlockLow),
    (2018, EnemyState::Home),
    (2020, EnemyState::JumpBack),
    (2021, EnemyState::Advance),
    (2022, EnemyState::FastAdvance),
    (2023, EnemyState::Pace),
    (2024, EnemyState::HitLow),
    (2025, EnemyState::HitHigh),
    (2027, EnemyState::Fire),
    (2028, EnemyState::Startup),
    (2059, EnemyState::GenericHit),
    (2111, EnemyState::FlyAbout),
    (2112, EnemyState::StrikeSetup),
    (2113, EnemyState::FlyToIdle),
    (2114, EnemyState::FlyToMarrow),
    (2115, EnemyState::CarryMarrow),
    (2116, EnemyState::WaitForParrot),
];

/// The numbers the reference runs give a slab's states.
const REFERENCE_FLOOR_STATES: [(i32, i32); 3] = [
    (2069, marrow_floor::state::IDLE),
    (2117, marrow_floor::state::CLOSING),
    (2118, marrow_floor::state::OPENING),
];

fn reference_state(id: i32) -> Option<i32> {
    REFERENCE_STATES
        .iter()
        .find(|(n, _)| *n == id)
        .map(|&(_, s)| s as i32)
}

fn reference_floor_state(id: i32) -> Option<i32> {
    REFERENCE_FLOOR_STATES
        .iter()
        .find(|(n, _)| *n == id)
        .map(|&(_, s)| s)
}

/// One object at one step: position, state (in this build's numbers)
/// and health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct At {
    x: i32,
    y: i32,
    state: Option<i32>,
    health: i32,
}

/// Which objects of the room are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Marrow,
    Parrot,
    /// A slab, by its record's x.
    Slab(i32),
}

impl Who {
    fn name(self) -> String {
        match self {
            Who::Marrow => "marrow".into(),
            Who::Parrot => "parrot".into(),
            Who::Slab(x) => format!("slab{x}"),
        }
    }
}

const WATCHED: [Who; 4] = [Who::Marrow, Who::Parrot, Who::Slab(40714), Who::Slab(40911)];

/// Marrow by step in a run's `boss.csv`: `None` on the steps without him.
fn reference_marrow(path: &Path) -> Option<Vec<(usize, Option<At>)>> {
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
            Some(At {
                x: f.get(x)?.parse().ok()?,
                y: f.get(y)?.parse().ok()?,
                state: reference_state(f.get(st)?.parse().ok()?),
                health: f.get(health)?.parse().ok()?,
            })
        })();
        out.push((k, at));
    }
    Some(out)
}

/// The parrot and the slabs by step in a run's `obj-log.csv`.
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
    let mut slabs: Vec<(String, i32)> = Vec::new();
    let mut out = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let name = *f.get(logic)?;
        let who = match name {
            "Parrot" => Who::Parrot,
            "MarrowFloor" => {
                let s = f.get(serial)?.to_string();
                let x0 = match slabs.iter().find(|(id, _)| *id == s) {
                    Some(&(_, x0)) => x0,
                    None => {
                        let x0: i32 = f.get(x)?.parse().ok()?;
                        slabs.push((s, x0));
                        x0
                    }
                };
                Who::Slab(x0)
            }
            _ => continue,
        };
        let raw: i32 = f.get(st)?.parse().ok()?;
        let state = match who {
            Who::Slab(_) => reference_floor_state(raw),
            _ => reference_state(raw),
        };
        out.push((
            f.get(tick)?.parse().ok()?,
            who,
            At {
                x: f.get(x)?.parse().ok()?,
                y: f.get(y)?.parse().ok()?,
                state,
                health: 0,
            },
        ));
    }
    Some(out)
}

// ----------------------------------------------------------------------
// Playing

fn level10() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(10)).unwrap();
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

fn config(clock: &str, seed: u32) -> SimConfig {
    let tuning = if clock == "c18" {
        ScriptTuning::Original
    } else {
        ScriptTuning::Default
    };
    SimConfig {
        tuning: tuning.tuning(),
        seed,
        ..SimConfig::default()
    }
}

fn t0_ms(clock: &str) -> u64 {
    T0_MS.iter().find(|(c, _)| *c == clock).map_or(0, |p| p.1)
}

/// The game at a route's time zero: Claw put at [`START`] on the level's
/// first tick and left there until `t0_ms`, then the random stream set to
/// `seed` and the route's respawn point, if it sets one.
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
    let mut sim = Sim::new(level.clone(), config(clock, seed));
    sim.teleport_player(START.0, START.1);
    for _ in 0..t0_ms.div_ceil(TICK) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| {
        *w.rng = Rng::new(seed);
        make_claw_speech_helper(w);
    });
    if let Some(p) = r.checkpoint {
        sim.set_checkpoint(Some(p));
    }
    sim
}

/// The original makes Claw's speech helper (the object his lines play
/// through) as the level loads, at his depth plus one, so it is listed
/// right after him; this build makes it on his first line. The reference
/// runs' helper is there from their first step: it is made here at time
/// zero, through the shortest of Claw's lines the level has, which plays
/// then. Listed right after Claw, it lets the update pass reach a stick of
/// dynamite he throws in the tick he throws it (the stick goes in after
/// the helper), as in the original.
fn make_claw_speech_helper(w: &mut claw_sim::logic::World) {
    let line = w
        .level
        .sounds
        .iter()
        .filter(|(k, ms)| k.starts_with("CLAW_") && ms.is_some())
        .min_by_key(|(_, ms)| **ms)
        .map(|(k, _)| k.clone())
        .expect("the level has lines of Claw's");
    assert!(claw_sim::logic::sound::claw_say(w, &line, true));
}

fn find(sim: &Sim, who: Who) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| match who {
            Who::Marrow => &*o.logic == "Marrow",
            Who::Parrot => &*o.logic == "Parrot",
            Who::Slab(x) => &*o.logic == "MarrowFloor" && o.editor.0 == x,
        })
        .map(|(id, _)| id)
}

/// One play of a route.
struct Played {
    rows: Vec<TraceRow>,
    /// The watched objects per row, in [`WATCHED`] order.
    room: Vec<[Option<At>; 4]>,
    completed: bool,
}

fn look(sim: &Sim, id: Option<ObjectId>) -> Option<At> {
    let o = sim.objects().get(id?)?;
    let state = if &*o.logic == "MarrowFloor" {
        (o.state != marrow_floor::state::SET_UP).then_some(o.state)
    } else {
        EnemyState::from_id(o.state).map(|s| s as i32)
    };
    Some(At {
        x: o.x,
        y: o.y,
        state,
        health: o.health,
    })
}

/// Plays `r` at `clock` from its time zero for `ticks` ticks.
fn play(level: &SimLevel, r: &Route, clock: &str, seed: u32, ticks: usize) -> Played {
    let mut sim = at_time_zero(level, clock, r, t0_ms(clock), seed);
    let ids = WATCHED.map(|who| find(&sim, who));
    let input = r.ticks();
    let mut rows = Vec::new();
    let mut room = Vec::new();
    let mut completed = false;
    for k in 0..ticks {
        r.play_holds(&mut sim, k);
        sim.step(input.get(k).copied().unwrap_or_default());
        completed |= sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }));
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        room.push(ids.map(|id| look(&sim, id)));
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
    marrow_ref: &[(usize, Option<At>)],
    room_ref: &[(usize, Who, At)],
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: String, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in marrow_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        let got = row[0].filter(|a| a.state.is_some());
        match (want, got) {
            (None, None) => {}
            (Some(want), Some(got)) => {
                for (col, same) in [
                    ("x", got.x == want.x),
                    ("y", got.y == want.y),
                    ("state", want.state.is_some() && got.state == want.state),
                    ("health", got.health == want.health),
                ] {
                    if !same {
                        note(format!("marrow_{col}"), k);
                    }
                }
            }
            (Some(_), None) | (None, Some(_)) => {
                for col in ["x", "y", "state", "health"] {
                    note(format!("marrow_{col}"), k);
                }
            }
        }
    }
    for &(k, who, want) in room_ref {
        let Some(row) = played.room.get(k) else {
            continue;
        };
        let i = WATCHED.iter().position(|w| *w == who).expect("watched");
        let name = who.name();
        let Some(got) = row[i] else {
            for col in ["x", "y", "state"] {
                note(format!("{name}_{col}"), k);
            }
            continue;
        };
        if got.x != want.x {
            note(format!("{name}_x"), k);
        }
        if got.y != want.y {
            note(format!("{name}_y"), k);
        }
        if want.state.is_none() || got.state != want.state {
            note(format!("{name}_state"), k);
        }
    }
    firsts
}

/// With `CLAW_L10_DUMP=FROM..TO`, prints the watched objects of the play
/// and of the run on those rows, for looking into a divergence.
fn dump(played: &Played, marrow_ref: &[(usize, Option<At>)], room_ref: &[(usize, Who, At)]) {
    let Some((a, b)) = std::env::var("CLAW_L10_DUMP").ok().and_then(|v| {
        let (a, b) = v.split_once("..")?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }) else {
        return;
    };
    for k in a..b.min(played.room.len()) {
        let claw = &played.rows[k];
        println!(
            "row {k}: claw {} {} s{} h{}",
            claw.x, claw.y, claw.state_id, claw.health
        );
        let theirs = marrow_ref.iter().find(|r| r.0 == k).and_then(|r| r.1);
        println!("  marrow ours {:?} ref {:?}", played.room[k][0], theirs);
        for (i, who) in WATCHED.iter().enumerate().skip(1) {
            let theirs = room_ref
                .iter()
                .find(|r| r.0 == k && r.1 == *who)
                .map(|r| r.2);
            println!(
                "  {} ours {:?} ref {:?}",
                who.name(),
                played.room[k][i],
                theirs
            );
        }
    }
}

/// The divergences of a play of `store` at `clock` not in [`KNOWN`].
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

/// The store's own copy of its route: (held, pressed) by row, and the
/// lines that take effect at a row's boundary.
#[allow(clippy::type_complexity)]
fn store_route(dir: &Path) -> Option<(Vec<(u32, u32)>, Vec<(usize, String)>)> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut directives = Vec::new();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix('@') {
            let (k, d) = rest.split_once(' ')?;
            directives.push((k.parse().ok()?, d.trim().to_string()));
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, directives))
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for (store, _) in STORES {
        let dir = root.join("level10").join(store);
        let (rows, directives) =
            store_route(&dir).unwrap_or_else(|| panic!("{store}: no route in the store"));
        let r = route(store);
        let ours = r.rows();
        assert_eq!(ours.len(), rows.len(), "{store}: rows");
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{store}: first different row");
        assert_eq!(r.directives, directives, "{store}: directives");
        let meta = std::fs::read_to_string(dir.join("meta.toml")).expect("meta");
        assert_eq!(
            meta_value(&meta, "", "diagnostic").as_deref() == Some("true"),
            r.checkpoint.is_some(),
            "{store}: only the diagnostic route moves the respawn point"
        );
    }
}

#[test]
fn install_marrow_routes_follow_the_reference_traces() {
    let Some(level) = level10() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L10_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for (store, clocks) in STORES {
        if only.as_deref().is_some_and(|o| o != store) {
            continue;
        }
        let dir = root.join("level10").join(store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{store}: meta: {e}"));
        let r = route(store);
        for &clock in clocks {
            println!("== level10/{store} {clock}");
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
            let ticks = reference.len();
            let played = play(&level, &r, clock, seed, ticks);
            if let Some(out) = std::env::var_os("CLAW_L10_TRACE") {
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
                        let boss = reference_marrow(&run.join("boss.csv"))
                            .unwrap_or_else(|| panic!("{store}: the run has no boss.csv"));
                        let objs = reference_room(&run.join("obj-log.csv"))
                            .unwrap_or_else(|| panic!("{store}: the run has no obj-log.csv"));
                        room = compare_room(&played, &boss, &objs);
                        dump(&played, &boss, &objs);
                    }
                    None => println!("the reference runs are not there: the room is not compared"),
                }
            }
            // The crossing and the bridge trick at the original's speeds
            // reach the level's end, as in the original (at 60 frames per
            // second, see `KNOWN`).
            let ends = store.starts_with("boss-skip-cross") || store == "boss-bridge-trick-c18";
            if clock == "c18" && ends && !played.completed {
                failures.push(format!("{store} {clock}: the level does not end"));
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
    fight: u8,
    marrow: Option<At>,
    marrow_safe: bool,
    marrow_z: i32,
    link: Option<marrow::MarrowLink>,
    parrot: Option<At>,
    slabs: [Option<At>; 2],
    bullets: Vec<(ObjectId, i32, i32)>,
    pistol_shots: usize,
    /// The gem out of Marrow: where it is, and whether it is still rising.
    gem: Option<(i32, i32, bool)>,
    ended: bool,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero, with the original's random state there.
fn watch(level: &SimLevel, store: &str, rows: usize) -> Vec<Look> {
    let r = route(store);
    let mut sim = at_time_zero(level, "c18", &r, t0_ms("c18"), SEED);
    let ids = WATCHED.map(|who| find(&sim, who));
    let input = r.ticks();
    let mut out = Vec::new();
    let mut gem_id = None;
    for k in 0..rows {
        r.play_holds(&mut sim, k);
        sim.step(input.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        let m = ids[0].and_then(|id| sim.objects().get(id));
        let objs = sim.objects();
        out.push(Look {
            claw: (c.x, c.y, c.state),
            claw_health: sim.claw().stats.health,
            fight: sim.boss().fight(),
            marrow: look(&sim, ids[0]).filter(|a| a.state.is_some()),
            marrow_safe: m.is_some_and(|o| o.flags & claw_sim::object::flags::SAFE != 0),
            marrow_z: m.map_or(0, |o| o.z),
            link: sim.marrow_link().copied(),
            parrot: look(&sim, ids[1]),
            slabs: [look(&sim, ids[2]), look(&sim, ids[3])],
            bullets: objs
                .all()
                .filter(|(_, o)| &*o.logic == "Bullet" && o.image_set == marrow::BULLET_IMAGE_SET)
                .map(|(id, o)| (id, o.x, o.y))
                .collect(),
            pistol_shots: objs
                .all()
                .filter(|(_, o)| {
                    o.type_flags == claw_sim::object::types::PISTOL_SHOT
                        && o.flags & claw_sim::object::flags::DESTROY == 0
                })
                .count(),
            gem: {
                if gem_id.is_none() {
                    gem_id = objs
                        .all()
                        .find(|(_, o)| &*o.logic == claw_sim::logic::pickup::END_GEM)
                        .map(|(id, _)| id);
                }
                gem_id
                    .and_then(|id| objs.get(id))
                    .map(|o| (o.x, o.y, &*o.logic == claw_sim::logic::pickup::END_GEM))
            },
            ended: sim.level_ended(),
        });
    }
    out
}

fn st(l: &Look) -> Option<EnemyState> {
    l.marrow.and_then(|m| m.state).and_then(EnemyState::from_id)
}

fn parrot_st(l: &Look) -> Option<EnemyState> {
    l.parrot.and_then(|p| p.state).and_then(EnemyState::from_id)
}

/// The first rows (from `from`) on which `f` starts to hold.
fn starts(rows: &[Look], from: usize, f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (from.max(1)..rows.len())
        .filter(|&k| f(&rows[k]) && !f(&rows[k - 1]))
        .collect()
}

fn first(rows: &[Look], from: usize, f: impl Fn(&Look) -> bool) -> usize {
    starts(rows, from, f)[0]
}

/// The rows on which Marrow's health drops, with the health after.
fn marrow_hits(rows: &[Look]) -> Vec<(usize, i32)> {
    (1..rows.len())
        .filter_map(|k| {
            let (a, b) = (rows[k - 1].marrow?, rows[k].marrow?);
            (b.health < a.health).then_some((k, b.health))
        })
        .collect()
}

const ARRIVE_C18: usize = 27;

#[test]
fn install_marrow_the_room_is_built_with_the_door_and_staged() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-intro-normal", 1100);
    let a = &rows[ARRIVE_C18];
    // On Claw's first row in the room: Marrow idles where he was placed,
    // sorted after Claw; the slabs are opening at their record's x; the
    // phase health reads 0; the parrot is 6 px above its perch.
    assert_eq!(rows[ARRIVE_C18 - 1].link, None, "nothing is built before");
    assert_eq!(st(a), Some(EnemyState::Idle));
    assert_eq!(a.marrow.map(|m| (m.x, m.y)), Some((41063, 7253)));
    assert_eq!(a.marrow_z, marrow::SORTED_Z);
    let link = a.link.expect("the Marrow link");
    assert!(link.open_request && !link.parrot_call && !link.floors_open);
    assert_eq!(link.phase_health, 0);
    let opening = Some(marrow_floor::state::OPENING);
    assert_eq!(
        a.slabs.map(|s| s.map(|s| (s.x, s.state))),
        [Some((40714, opening)), Some((40911, opening))]
    );
    assert_eq!(a.parrot.map(|p| (p.x, p.y)), Some((41049, 7135)));
    // The slabs step from the next row: the right one is open 32 rows
    // after the first row in the room, the left one 36.
    let open = |i: usize, x: i32| {
        first(&rows, ARRIVE_C18, |l| {
            l.slabs[i].is_some_and(|s| s.x == x && s.state == Some(marrow_floor::state::IDLE))
        })
    };
    assert_eq!(
        open(1, 41071) - ARRIVE_C18,
        marrow_floor::OPEN_TICKS.1 as usize
    );
    assert_eq!(
        open(0, 40534) - ARRIVE_C18,
        marrow_floor::OPEN_TICKS.0 as usize
    );
    // The introduction: flag 1 on the next row, 2 on the 748th, Claw
    // stands again on the 749th.
    assert_eq!(first(&rows, 0, |l| l.fight == 1), ARRIVE_C18 + 1);
    assert_eq!(
        first(&rows, 0, |l| l.fight == 2),
        ARRIVE_C18 + marrow::FIGHT_TICKS as usize
    );
    let thaw = first(&rows, ARRIVE_C18 + 2, |l| l.claw.2 != 5009);
    assert_eq!(thaw, ARRIVE_C18 + marrow::THAW_TICKS as usize);
    // He paces at the thaw, off screen, and never goes Home: Claw at the
    // door is out of his sight.
    assert_eq!(st(&rows[thaw]), Some(EnemyState::Pace));
    assert!(rows.iter().all(|l| st(l) != Some(EnemyState::Home)));
    // The parrot patrols, and dives only once the fight is on: its first
    // dive's set-up 28 rows, its swoop 31, and Claw at the door is hit for
    // 10 on the swoop's 28th row.
    let setups = starts(&rows, 0, |l| parrot_st(l) == Some(EnemyState::StrikeSetup));
    assert_eq!(setups[0], 985);
    assert!(setups[0] > ARRIVE_C18 + marrow::FIGHT_TICKS as usize);
    let strike = first(&rows, 985, |l| parrot_st(l) == Some(EnemyState::Strike));
    assert_eq!(strike - setups[0], parrot::STRIKE_SETUP_TICKS as usize);
    let climb = first(&rows, strike, |l| {
        parrot_st(l) == Some(EnemyState::FlyAbout)
    });
    assert_eq!(climb - strike, parrot::STRIKE_TICKS as usize);
    let hit = first(&rows, strike, |l| l.claw_health < 100);
    assert_eq!((hit, rows[hit].claw_health), (1040, 90));
}

#[test]
fn install_marrow_the_normal_fight_closes_the_bridge_and_the_parrot_carries_him() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-normal-fight-c18", 2100);
    // Three sword hits on the parrot's swoops send it home: perched on
    // 1534 (the open request clears there), the slabs close 33 and 37 rows
    // later, and the phase health is his health, 100.
    let perch = first(&rows, 0, |l| parrot_st(l) == Some(EnemyState::Idle));
    assert_eq!(perch, 1534);
    assert!(!rows[perch].link.expect("link").open_request);
    let closed = |i: usize, x: i32| {
        first(&rows, perch, |l| {
            l.slabs[i].is_some_and(|s| s.x == x && s.state == Some(marrow_floor::state::IDLE))
        })
    };
    assert_eq!(closed(1, 40911), perch + 33);
    assert_eq!(closed(0, 40714), perch + 37);
    assert_eq!(
        rows[perch + 37]
            .link
            .map(|l| (l.phase_health, l.floors_open)),
        Some((100, false))
    );
    // Claw on his ledge: Home on 1693, the shot from 1694; its bullet on
    // the 19th row 65 px ahead and 24 up, 600 px/s; Home on the 34th.
    assert_eq!(
        first(&rows, 1600, |l| st(l) == Some(EnemyState::Home)),
        1693
    );
    let fire = first(&rows, 1600, |l| st(l) == Some(EnemyState::Fire));
    assert_eq!(fire, 1694);
    let shot = first(&rows, fire, |l| !l.bullets.is_empty());
    assert_eq!(shot - fire, marrow::FIRE_TICKS.0 as usize);
    let (_, bx, by) = rows[shot].bullets[0];
    assert_eq!((bx, by), (40930, 7229));
    let (_, bx2, _) = rows[shot + 5].bullets[0];
    assert!(
        (53..=55).contains(&(bx - bx2)),
        "about 10.8 px a row: {}",
        bx - bx2
    );
    assert_eq!(
        first(&rows, fire, |l| st(l) != Some(EnemyState::Fire)) - fire,
        marrow::FIRE_TICKS.1 as usize
    );
    // His touch costs the jumping Claw 10.
    assert_eq!(first(&rows, 1690, |l| l.claw_health < 100), 1702);
    // Five hits of 5, then the blow 25 below the phase health is blocked;
    // the whistle on 1875 (grab offset 58 on the centre side), Safe; the
    // call 16 rows later; the carry from 1901, the slabs opening a row
    // later; the drop on the left ledge on 2020, the phase health 75.
    assert_eq!(
        marrow_hits(&rows),
        [(1742, 95), (1766, 90), (1791, 85), (1815, 80), (1839, 75)]
    );
    assert_eq!(
        first(&rows, 1840, |l| st(l) == Some(EnemyState::BlockLow)),
        1863
    );
    let wait = first(&rows, 0, |l| st(l) == Some(EnemyState::WaitForParrot));
    assert_eq!(wait, 1875);
    assert!(rows[wait].marrow_safe);
    let call = first(&rows, wait, |l| l.link.is_some_and(|k| k.parrot_call));
    assert_eq!(call - wait, marrow::CALL_TICK as usize);
    let carry = first(&rows, call, |l| {
        parrot_st(l) == Some(EnemyState::CarryMarrow)
    });
    assert_eq!(carry, 1901);
    assert!(rows[carry].link.expect("link").open_request);
    assert_eq!(
        first(&rows, carry, |l| {
            l.slabs[1].is_some_and(|s| s.state == Some(marrow_floor::state::OPENING))
        }),
        carry + 1
    );
    let drop = first(&rows, carry, |l| st(l) == Some(EnemyState::Home));
    assert_eq!(drop, wait + 145);
    assert_eq!(drop - carry, parrot::CARRY_TICKS as usize);
    let l = &rows[drop];
    assert_eq!(l.marrow.map(|m| (m.x, m.y)), Some((40612, 7253)));
    assert!(!l.marrow_safe);
    assert_eq!(
        l.link.map(|k| (k.phase_health, k.parrot_call)),
        Some((75, false))
    );
}

#[test]
fn install_marrow_a_claw_on_the_opening_slab_is_carried_into_the_wall() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-normal-ride-c18", 2200);
    // Claw stands on the right slab when the carry opens it: carried at
    // 5 px a row from 1903, held by the ledge wall at 40959, and falling on
    // the slab's 31st step, into the spikes.
    assert_eq!(rows[1895].claw.0, 40939);
    assert_eq!(rows[1932].claw.0, marrow_floor::RIGHT_RIDER_STOP_X);
    let fall = first(&rows, 1903, |l| l.claw.2 != claw_sim::player::state::STAND);
    assert_eq!(fall, 1933);
    assert_eq!(rows[fall].slabs[1].map(|s| s.x), Some(41066));
    assert!(rows[2000].claw_health <= 0, "he dies on the spikes");
    // His respawn puts Marrow back at full health with the slabs snapped
    // shut and opening again, in the respawn's row.
    let r = first(&rows, 2000, |l| l.claw_health == 100);
    assert_eq!(r, 2117);
    assert_eq!(rows[r].marrow.map(|m| (m.x, m.health)), Some((41063, 100)));
    assert_eq!(rows[r].link.map(|k| k.phase_health), Some(100));
    assert_eq!(
        rows[r].slabs.map(|s| s.map(|s| s.x)),
        [Some(40709), Some(40916)]
    );
}

#[test]
fn install_marrow_a_pistol_shot_counts_as_a_hit_on_the_parrot_and_flies_on() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-parrot-pistol-c18", 1350);
    let hit = first(&rows, 1000, |l| {
        parrot_st(l) == Some(EnemyState::GenericHit)
    });
    assert_eq!(hit, 1031);
    assert!(rows[hit].pistol_shots > 0, "the shot is not used up");
    // It flies on to Marrow, who blocks it and is woken: he fires across
    // the open pit.
    let block = first(&rows, hit, |l| st(l) == Some(EnemyState::BlockLow));
    assert_eq!(block, 1059);
    assert_eq!(rows[block].pistol_shots, 0);
    assert_eq!(
        first(&rows, block, |l| st(l) == Some(EnemyState::Fire)),
        1071
    );
}

#[test]
fn install_marrow_after_the_skip_he_only_idles_and_paces_and_the_pit_opens() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-intro-skip-c18", 2000);
    assert!(rows.iter().all(|l| l.fight == 0), "the flag stays 0");
    for l in &rows[ARRIVE_C18..] {
        assert!(
            matches!(st(l), Some(EnemyState::Idle | EnemyState::Pace)),
            "{:?}",
            st(l)
        );
        assert_eq!(parrot_st(l), Some(EnemyState::FlyAbout));
        assert!(l.bullets.is_empty());
        assert_eq!(l.marrow.map(|m| m.health), Some(100));
        assert_eq!(l.link.map(|k| k.phase_health), Some(0));
    }
    let open = Some(marrow_floor::state::IDLE);
    assert_eq!(
        rows[ARRIVE_C18 + 40]
            .slabs
            .map(|s| s.map(|s| (s.x, s.state))),
        [Some((40534, open)), Some((41071, open))]
    );
    assert_eq!(rows[67].claw.0, 40566, "landed on the left ledge");
    // Walking back into the stager's box runs the introduction late.
    let back = watch(&level, "boss-intro-skip-return-c18", 900);
    assert_eq!(first(&back, 0, |l| l.fight == 1), 117);
    assert_eq!(first(&back, 0, |l| l.fight == 2), 864);
}

#[test]
fn install_marrow_the_reference_crossing_kills_the_passive_marrow() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-skip-cross-kill-c18", 877);
    // The jump out of the blast's hit lands Claw on Marrow's ledge.
    assert!(rows[188].claw.0 > 40960 && rows[188].claw.1 == 7245);
    // Twenty sword hits of 5 kill him; no shot, no whistle, no block of
    // the health rule (the phase health stays 0).
    let hits = marrow_hits(&rows);
    assert_eq!(hits.len(), 20);
    assert!(hits.windows(2).all(|p| p[0].1 - p[1].1 == 5));
    assert_eq!((hits[0].0, hits[19]), (216, (672, 0)));
    assert!(rows.iter().all(|l| l.bullets.is_empty()));
    assert!(
        rows.iter()
            .all(|l| st(l) != Some(EnemyState::WaitForParrot))
    );
    assert!(rows.iter().all(|l| l.fight == 0));
    // Dead on 672, the corpse removed 52 rows later (the link ends there);
    // the gem rises 148 rows to its point over the pit and ends the level
    // when Claw jumps into it.
    assert_eq!(first(&rows, 600, |l| st(l) == Some(EnemyState::Dead)), 672);
    assert_eq!(
        first(&rows, 672, |l| l.link.is_none()),
        672 + marrow::DEAD_TICKS as usize
    );
    let landed = first(&rows, 672, |l| l.gem.is_some_and(|g| !g.2));
    assert_eq!(landed - 672, marrow::GEM_RISE_TICKS as usize);
    assert_eq!(rows[landed].gem.map(|g| (g.0, g.1)), Some((40798, 7171)));
    assert!(rows.last().expect("rows").ended, "the level ends");
    // Pausing changes nothing: the route with the reference run's pauses
    // over the blast's hit plays the same rows.
    let paused = watch(&level, "boss-skip-cross-pause-c18", 877);
    let first_diff = rows.iter().zip(&paused).position(|(a, b)| a != b);
    assert_eq!(first_diff, None);
}

#[test]
fn install_marrow_after_a_death_the_skipped_fight_blocks_whistles_and_drops_back() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-skip-death-fight-c18", 2700);
    assert!(rows.iter().all(|l| l.fight == 0), "the flag stays 0");
    // Four hits, a life ended at row 300, and the respawn on the left
    // ledge: Marrow back to 100, the slabs snapped shut and opening in the
    // respawn's row (open 31 and 35 rows later), the phase health 100.
    assert_eq!(
        marrow_hits(&rows)[..4]
            .iter()
            .map(|h| h.1)
            .collect::<Vec<_>>(),
        [95, 90, 85, 80]
    );
    let r = first(&rows, 300, |l| {
        l.claw_health == 100 && l.claw.0 == DIAGNOSTIC_CHECKPOINT.0
    });
    assert_eq!(r, 394);
    assert_eq!(rows[r].marrow.map(|m| m.health), Some(100));
    assert_eq!(rows[r].link.map(|k| k.phase_health), Some(100));
    assert_eq!(
        rows[r].slabs.map(|s| s.map(|s| s.x)),
        [Some(40709), Some(40916)]
    );
    let open = |i: usize, x: i32| {
        first(&rows, r, |l| {
            l.slabs[i].is_some_and(|s| s.x == x && s.state == Some(marrow_floor::state::IDLE))
        })
    };
    assert_eq!(open(1, 41071), r + 31);
    assert_eq!(open(0, 40534), r + 35);
    // From then on every 25 damage costs a blocked blow and a whistle the
    // parrot never answers: Safe for 5000 ms, the call 16 rows in, the
    // drop 278 rows in with the phase health his health, Safe no more
    // from there, or, when a blow of Claw's meets him then, once he has
    // blocked it; four such cycles still end in his death.
    let waits = starts(&rows, r, |l| st(l) == Some(EnemyState::WaitForParrot));
    assert!(waits.len() >= 3, "{waits:?}");
    for &w in &waits {
        let h = rows[w].marrow.expect("Marrow").health;
        assert_eq!(h % 25, 0, "a whistle at {h}");
        assert!(rows[w].marrow_safe);
        let call = first(&rows, w, |l| l.link.is_some_and(|k| k.parrot_call));
        assert_eq!(call - w, marrow::CALL_TICK as usize);
        let drop = first(&rows, w, |l| l.link.is_some_and(|k| k.phase_health == h));
        assert_eq!(drop - w, (marrow::WAIT_FOR_PARROT_MS as usize).div_ceil(18));
        let open = first(&rows, w, |l| !l.marrow_safe);
        assert!(open >= drop, "safe through the wait");
        assert!(
            rows[drop..open]
                .iter()
                .all(|l| matches!(st(l), Some(EnemyState::BlockHigh | EnemyState::BlockLow))),
            "a block keeps him safe past the drop"
        );
        assert_eq!(
            rows[drop].link.map(|k| (k.phase_health, k.parrot_call)),
            Some((h, false))
        );
    }
    assert!(
        rows[r..]
            .iter()
            .all(|l| matches!(parrot_st(l), Some(EnemyState::FlyAbout)))
    );
    assert!(
        rows.iter().any(|l| st(l) == Some(EnemyState::Dead)),
        "he dies"
    );
}

#[test]
fn install_marrow_the_bridge_trick_strands_him_on_claws_ledge() {
    let Some(level) = level10() else { return };
    let rows = watch(&level, "boss-bridge-trick-c18", 4244);
    // Five hits and a blocked sixth blow; Claw jumps off Marrow's ledge
    // as he whistles, so Marrow runs at him through the whistle and the
    // parrot takes hold of him late: the carry starts on 1908 with Claw in
    // the air on Marrow's half, and goes to the left ledge.
    let wait = first(&rows, 0, |l| st(l) == Some(EnemyState::WaitForParrot));
    assert_eq!(wait, 1875);
    let call = first(&rows, wait, |l| l.link.is_some_and(|k| k.parrot_call));
    assert!(rows[call].marrow.expect("Marrow").x < rows[wait].marrow.expect("Marrow").x);
    let carry = first(&rows, call, |l| {
        parrot_st(l) == Some(EnemyState::CarryMarrow)
    });
    assert_eq!(carry, 1908);
    let (cx, cy, cs) = rows[carry].claw;
    assert!(cx >= marrow::ROOM_CENTRE_X, "on Marrow's half: {cx}");
    assert_ne!(cs, claw_sim::player::state::STAND, "in the air at {cy}");
    // He lands on the left slab, which carries him left, and jumps onto
    // the left ledge before it is under the rock.
    let landed = first(&rows, carry, |l| l.claw.1 == 7312);
    assert!(landed < 1918, "{landed}");
    assert!(rows[landed + 3].claw.0 < rows[landed].claw.0);
    assert!(rows[1960].claw.0 < 40626 && rows[1960].claw.1 == 7245);
    // The drop beside him: (40615, 7253) on 2022, the phase health 75.
    let drop = first(&rows, carry, |l| st(l) == Some(EnemyState::Home));
    assert_eq!(drop, 2022);
    assert_eq!(rows[drop].marrow.map(|m| (m.x, m.y)), Some((40615, 7253)));
    assert_eq!(rows[drop].link.map(|k| k.phase_health), Some(75));
    // From then on the slabs stay open and the parrot never perches; his
    // whistles go unanswered and each ends in the 278-tick timeout with
    // the phase health his health.
    let open = Some(marrow_floor::state::IDLE);
    for l in &rows[1945..] {
        if l.link.is_none() {
            break;
        }
        assert_eq!(
            l.slabs.map(|s| s.map(|s| (s.x, s.state))),
            [Some((40534, open)), Some((41071, open))]
        );
        assert!(!matches!(
            parrot_st(l),
            Some(EnemyState::Idle | EnemyState::FlyToIdle | EnemyState::FlyToMarrow)
        ));
    }
    let waits = starts(&rows, drop, |l| st(l) == Some(EnemyState::WaitForParrot));
    assert_eq!(waits, [2569, 3289]);
    for (&w, health) in waits.iter().zip([49, 23]) {
        let end = first(&rows, w, |l| !l.marrow_safe);
        assert_eq!(end - w, 278);
        assert_eq!(rows[end].link.map(|k| k.phase_health), Some(health));
    }
    // He dies on the left ledge on 4045; Claw jumps into the gem.
    assert_eq!(
        first(&rows, drop, |l| st(l) == Some(EnemyState::Dead)),
        4045
    );
    assert!(rows[4045].marrow.expect("Marrow").x < 40626);
    assert!(rows.last().expect("rows").ended, "the level ends");
}
