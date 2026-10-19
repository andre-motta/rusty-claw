//! The eighth level's boss room on your own install: Gabriel, his cannon,
//! its lever and button, played against the original's runs of the same
//! routes, and the fight's main outcomes checked on their own.
//!
//! The routes start on the floor right of the door into his room: Claw is
//! put at (31700, 5200) on the level's first tick and left there until the
//! route's time zero (828 ms of game time at the original speeds, 817 ms
//! at the default ones, as in the reference runs), then the random stream
//! is set to the store's state at time zero, once, and a route that asks
//! for it gives Claw Catnip for its time. Left held from row 15 walks him
//! into the door. Then, by route:
//!
//! - the normal entry: Claw stands through the whole introduction and the
//!   fight after it, or swings his sword every 12 rows and fires every 48
//!   from row 764;
//! - the skip: Right and Jump on the first row in the room (Claw lands on
//!   the dock), one row late (the introduction runs), then a walk back
//!   into the stager's box, or a life ended at row 300;
//! - with Catnip, the skip lands Claw on the deck beside Gabriel: he
//!   stands, swings his sword, lifts Gabriel and throws him onto the dock,
//!   or (the reference kill) lifts him again on the dock and throws him
//!   into the water, then jumps for the gem;
//! - the button: the climb over the three pegs and an air swipe at the
//!   button, after a skip (then back into the stager's box and sword
//!   swings) or after the introduction (sword swings; a life ended at row
//!   1100; or, as the original's diagnostic run does, his count of cannon
//!   hits set to four after step 1000 so that the next cannon hit starts
//!   his death acts, then the gem jump).
//!
//! The routes are written below ([`ROUTES`]); each store's own copy of
//! its route must agree with them. Each route is played tick by tick and
//! compared with the reference trace of each of its clocks: at the
//! original speeds (c18) every column exactly, the random stream included;
//! at the default speeds played tick by tick (c60) within the feel
//! tolerances. When the reference runs also list Gabriel's position,
//! state and health per step (`boss.csv` beside each run, in the folder
//! named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the reference
//! store), his are compared too: exactly at c18; positions within 8 px and
//! states and health within two ticks at c60.
//!
//! The pirates Gabriel calls in run his attack through the pirate's own
//! boss-fight hooks; a pirate of his that appears without being sent on
//! his attack fails the test.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE`
//! for the comparisons: each test skips itself when what it needs is
//! unset, and fails when `CLAW_REFERENCE` is set but a listed store lacks
//! its route, meta or trace (or, with the reference runs there, a run's
//! `boss.csv`); `CLAW_L8_ONLY` names one store to play alone. Nothing from
//! an install or a reference store is kept in the repository.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::activation::Centres;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::cannon_switch::{self, cannon};
use claw_sim::logic::enemy::{EnemyState, gabriel};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{ObjectData, pickup};
use claw_sim::object::{LogicKind, ObjectId, flags};
use claw_sim::player::death::{CATNIP_GLITTER_LOGIC, powerup};
use claw_sim::player::powerup_aid::AidPowerup;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Comparison, Tolerances, TraceRow};
use claw_sim::{Buttons, GameLogic, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Where every route starts: on the floor right of the door into his room.
const START: (i32, i32) = (31700, 5200);
/// The random state of every store at its time zero.
const SEED: u32 = 111_567;
/// The game time at a route's time zero, by clock.
const T0_MS: [(&str, u64); 2] = [("c18", 828), ("c60", 817)];

const ATTACK: u32 = 0x2;
const FIRE: u32 = 0x4;
const LIFT: u32 = 0x20;

/// Gabriel at his place on his deck.
const HIM_AT: (i32, i32) = (43223, 5065);

// ----------------------------------------------------------------------
// The routes

/// One stretch of a route: from `from` to `to` (inclusive) `key` is held,
/// pressed on the stretch's first row.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    key: u32,
}

const fn span(from: usize, to: usize, key: u32) -> Span {
    Span { from, to, key }
}

/// A tap repeated: its first row, how many rows it is held, the key, the
/// rows from one tap to the next and the first row no tap starts on.
#[derive(Debug, Clone, Copy)]
struct Every {
    first: usize,
    len: usize,
    key: u32,
    gap: usize,
    until: usize,
}

const fn every(first: usize, len: usize, key: u32, gap: usize, until: usize) -> Every {
    Every {
        first,
        len,
        key,
        gap,
        until,
    }
}

/// A route of the stores.
struct Route {
    store: &'static str,
    /// The clocks it was run at.
    clocks: &'static [&'static str],
    rows: usize,
    spans: &'static [Span],
    every: &'static [Every],
    /// Catnip given at time zero, ms.
    catnip_ms: Option<u32>,
    /// The row at which Claw's life is ended.
    end_life: Option<usize>,
    /// After this row his count of cannon hits is set to this number (the
    /// original's diagnostic run).
    hits_after: Option<(usize, i32)>,
}

const ROUTES: [Route; 22] = [
    Route {
        store: "boss-button-normal-c18",
        clocks: &["c18"],
        rows: 2400,
        spans: &[
            span(15, 21, LEFT),
            span(760, 826, LEFT),
            span(866, 881, JUMP),
            span(911, 926, JUMP),
            span(913, 918, RIGHT),
            span(956, 971, JUMP),
            span(958, 967, LEFT),
            span(996, 1011, JUMP),
            span(996, 1015, RIGHT),
            span(1012, 1013, ATTACK),
        ],
        every: &[every(1056, 2, ATTACK, 12, 2377)],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-button-normal-end-life-c18",
        clocks: &["c18"],
        rows: 2900,
        spans: &[
            span(15, 21, LEFT),
            span(760, 826, LEFT),
            span(866, 881, JUMP),
            span(911, 926, JUMP),
            span(913, 918, RIGHT),
            span(956, 971, JUMP),
            span(958, 967, LEFT),
            span(996, 1011, JUMP),
            span(996, 1015, RIGHT),
            span(1012, 1013, ATTACK),
        ],
        every: &[every(1056, 2, ATTACK, 12, 2869)],
        catnip_ms: None,
        end_life: Some(1100),
        hits_after: None,
    },
    Route {
        store: "boss-button-normal-kill-c18",
        clocks: &["c18"],
        rows: 2700,
        spans: &[
            span(15, 21, LEFT),
            span(760, 826, LEFT),
            span(866, 881, JUMP),
            span(911, 926, JUMP),
            span(913, 918, RIGHT),
            span(956, 971, JUMP),
            span(958, 967, LEFT),
            span(996, 1011, JUMP),
            span(996, 1015, RIGHT),
            span(1012, 1013, ATTACK),
            span(2462, 2507, RIGHT),
            span(2512, 2529, JUMP),
        ],
        every: &[every(1056, 2, ATTACK, 12, 2437)],
        catnip_ms: None,
        end_life: None,
        hits_after: Some((1000, 4)),
    },
    Route {
        store: "boss-button-skip",
        clocks: &["c60"],
        rows: 1700,
        spans: &[
            span(15, 20, LEFT),
            span(21, 40, JUMP),
            span(21, 60, RIGHT),
            span(70, 89, JUMP),
            span(70, 140, LEFT),
            span(200, 215, JUMP),
            span(245, 260, JUMP),
            span(247, 252, RIGHT),
            span(290, 305, JUMP),
            span(292, 301, LEFT),
            span(330, 345, JUMP),
            span(330, 349, RIGHT),
            span(346, 347, ATTACK),
        ],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-button-skip-c18",
        clocks: &["c18"],
        rows: 1700,
        spans: &[
            span(15, 21, LEFT),
            span(22, 41, JUMP),
            span(22, 61, RIGHT),
            span(70, 89, JUMP),
            span(70, 155, LEFT),
            span(200, 215, JUMP),
            span(245, 260, JUMP),
            span(247, 252, RIGHT),
            span(290, 305, JUMP),
            span(292, 301, LEFT),
            span(330, 345, JUMP),
            span(330, 349, RIGHT),
            span(346, 347, ATTACK),
        ],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-button-skip-return",
        clocks: &["c60"],
        rows: 2600,
        spans: &[
            span(15, 20, LEFT),
            span(21, 40, JUMP),
            span(21, 60, RIGHT),
            span(70, 89, JUMP),
            span(70, 140, LEFT),
            span(200, 215, JUMP),
            span(245, 260, JUMP),
            span(247, 252, RIGHT),
            span(290, 305, JUMP),
            span(292, 301, LEFT),
            span(330, 345, JUMP),
            span(330, 349, RIGHT),
            span(346, 347, ATTACK),
            span(400, 444, RIGHT),
        ],
        every: &[every(1150, 2, ATTACK, 12, 2579)],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-button-skip-return-c18",
        clocks: &["c18"],
        rows: 2600,
        spans: &[
            span(15, 21, LEFT),
            span(22, 41, JUMP),
            span(22, 61, RIGHT),
            span(70, 89, JUMP),
            span(70, 155, LEFT),
            span(200, 215, JUMP),
            span(245, 260, JUMP),
            span(247, 252, RIGHT),
            span(290, 305, JUMP),
            span(292, 301, LEFT),
            span(330, 345, JUMP),
            span(330, 349, RIGHT),
            span(346, 347, ATTACK),
            span(400, 444, RIGHT),
        ],
        every: &[every(1150, 2, ATTACK, 12, 2579)],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-normal",
        clocks: &["c18", "c60"],
        rows: 2100,
        spans: &[span(15, 21, LEFT)],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip",
        clocks: &["c60"],
        rows: 1900,
        spans: &[span(15, 20, LEFT), span(21, 40, JUMP), span(21, 60, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-c18",
        clocks: &["c18"],
        rows: 1900,
        spans: &[span(15, 21, LEFT), span(22, 41, JUMP), span(22, 61, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-catnip",
        clocks: &["c18", "c60"],
        rows: 1200,
        spans: &[span(15, 19, LEFT), span(20, 59, JUMP), span(20, 59, RIGHT)],
        every: &[],
        catnip_ms: Some(53000),
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-catnip-fight",
        clocks: &["c18", "c60"],
        rows: 1200,
        spans: &[span(15, 19, LEFT), span(20, 59, JUMP), span(20, 59, RIGHT)],
        every: &[every(76, 2, ATTACK, 12, 1181)],
        catnip_ms: Some(53000),
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-end-life",
        clocks: &["c60"],
        rows: 1900,
        spans: &[span(15, 20, LEFT), span(21, 40, JUMP), span(21, 60, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: Some(300),
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-end-life-c18",
        clocks: &["c18"],
        rows: 1900,
        spans: &[span(15, 21, LEFT), span(22, 41, JUMP), span(22, 61, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: Some(300),
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-late",
        clocks: &["c60"],
        rows: 1900,
        spans: &[span(15, 21, LEFT), span(22, 41, JUMP), span(22, 61, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-late-c18",
        clocks: &["c18"],
        rows: 1900,
        spans: &[span(15, 22, LEFT), span(23, 42, JUMP), span(23, 62, RIGHT)],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-return",
        clocks: &["c60"],
        rows: 1900,
        spans: &[
            span(15, 20, LEFT),
            span(21, 40, JUMP),
            span(21, 60, RIGHT),
            span(91, 120, LEFT),
        ],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-intro-skip-return-c18",
        clocks: &["c18"],
        rows: 1900,
        spans: &[
            span(15, 21, LEFT),
            span(22, 41, JUMP),
            span(22, 61, RIGHT),
            span(92, 121, LEFT),
        ],
        every: &[],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-lift-drown",
        clocks: &["c60"],
        rows: 1150,
        spans: &[
            span(15, 19, LEFT),
            span(20, 59, JUMP),
            span(20, 59, RIGHT),
            span(76, 77, LIFT),
            span(110, 115, LEFT),
            span(112, 113, LIFT),
            span(140, 169, JUMP),
            span(140, 209, LEFT),
            span(214, 215, RIGHT),
            span(300, 316, RIGHT),
            span(322, 323, LIFT),
            span(328, 329, LIFT),
            span(334, 335, LIFT),
            span(340, 341, LIFT),
            span(346, 347, LIFT),
            span(380, 383, LEFT),
            span(386, 585, LIFT),
            span(902, 913, RIGHT),
            span(916, 935, JUMP),
        ],
        every: &[],
        catnip_ms: Some(53000),
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-lift-drown-c18",
        clocks: &["c18"],
        rows: 1100,
        spans: &[
            span(15, 19, LEFT),
            span(20, 59, JUMP),
            span(20, 59, RIGHT),
            span(76, 77, LIFT),
            span(110, 115, LEFT),
            span(112, 113, LIFT),
            span(140, 169, JUMP),
            span(140, 209, LEFT),
            span(214, 215, RIGHT),
            span(300, 303, RIGHT),
            span(306, 307, LIFT),
            span(312, 313, LIFT),
            span(346, 349, LEFT),
            span(352, 551, LIFT),
            span(856, 877, RIGHT),
            span(880, 899, JUMP),
        ],
        every: &[],
        catnip_ms: Some(53000),
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-lift-throw",
        clocks: &["c18", "c60"],
        rows: 1400,
        spans: &[
            span(15, 19, LEFT),
            span(20, 59, JUMP),
            span(20, 59, RIGHT),
            span(76, 77, LIFT),
            span(110, 115, LEFT),
            span(112, 113, LIFT),
        ],
        every: &[],
        catnip_ms: Some(53000),
        end_life: None,
        hits_after: None,
    },
    Route {
        store: "boss-normal-fight",
        clocks: &["c18", "c60"],
        rows: 2100,
        spans: &[span(15, 21, LEFT)],
        every: &[
            every(764, 2, ATTACK, 12, 2085),
            every(770, 2, FIRE, 48, 2067),
        ],
        catnip_ms: None,
        end_life: None,
        hits_after: None,
    },
];

fn route_named(store: &str) -> &'static Route {
    ROUTES
        .iter()
        .find(|r| r.store == store)
        .unwrap_or_else(|| panic!("no route {store}"))
}

/// The route's input, row by row.
fn route_ticks(r: &Route) -> Vec<TickInput> {
    let mut held = vec![0u32; r.rows];
    let mut pressed = vec![0u32; r.rows];
    let mut spans: Vec<Span> = r.spans.to_vec();
    for e in r.every {
        let mut from = e.first;
        while from < e.until.min(r.rows) {
            spans.push(span(from, (from + e.len - 1).min(r.rows - 1), e.key));
            from += e.gap;
        }
    }
    for s in &spans {
        for h in &mut held[s.from..=s.to] {
            *h |= s.key;
        }
        pressed[s.from] |= s.key;
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

// ----------------------------------------------------------------------
// The stores

fn level8() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(8)).unwrap();
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

/// The first run named in the meta's table for `clock`.
fn run_id(meta: &str, clock: &str) -> Option<String> {
    let ids = meta_value(meta, clock, "run_ids")?;
    Some(ids.split('"').nth(1).unwrap_or(&ids).to_string())
}

// ----------------------------------------------------------------------
// Playing a route

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

/// Claw put at `at` on the level's first tick and left to settle until
/// `t0_ms` of game time has passed (as in the reference runs), then the
/// random stream set to `seed`: the next tick is the route's time zero.
///
/// With `catnip_ms`, Claw has Catnip for that time from the route's time
/// zero, given between the two ticks as a pickup gives it: the glitter
/// sheds its first sparkles in the route's second tick, as in the
/// reference runs. The replay gives it in the last tick before time zero,
/// then puts the time back and holds the glitter for one tick.
fn start_at(sim: &mut Sim, at: (i32, i32), t0_ms: u64, seed: u32, catnip_ms: Option<u32>) {
    sim.teleport_player(at.0, at.1);
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
/// tick in the room, that this build does not do yet: the door runs one
/// more call of each object he leaves behind outside the room's window,
/// with the frame pulse of the tick just played (their draws of that call
/// come before the room's first tick), and they sleep from then on; the
/// effects already running (the Catnip glitter) have no call then. The
/// replay makes those calls and holds those objects and effects for the
/// room's first tick when Claw is about to go through the door.
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
                return true;
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
            let effect = w
                .objects
                .get(id)
                .is_some_and(|o| o.kind == LogicKind::Game(GameLogic::Effect));
            if !effect {
                claw_sim::logic::tick(w, id);
            }
            if let Some(o) = w.objects.get_mut(id) {
                o.frame_delay = 1;
            }
        }
    });
}

/// Gabriel on one tick: x, y, state and health.
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
        .find(|(_, o)| gabriel::NAMES.contains(&&*o.logic))
        .map(|(id, _)| id)
}

fn him(sim: &Sim) -> Option<Him> {
    let o = sim.objects().get(find_him(sim)?)?;
    Some(Him {
        x: o.x,
        y: o.y,
        state: EnemyState::from_id(o.state)?,
        health: o.health,
    })
}

fn his_data(sim: &Sim) -> Option<&gabriel::GabrielData> {
    match &sim.objects().get(find_him(sim)?)?.data {
        ObjectData::Gabriel(d) => Some(d),
        _ => None,
    }
}

/// The pirates he called in that he still counts.
fn his_pirates(sim: &Sim) -> Vec<ObjectId> {
    his_data(sim).map_or_else(Vec::new, |d| d.summoned.clone())
}

/// The state a reference run's state number stands for.
fn state_numbered(id: i32) -> Option<EnemyState> {
    gabriel::REFERENCE_STATE_IDS
        .iter()
        .find(|(_, n)| *n == id)
        .map(|&(s, _)| s)
}

/// Gabriel per step of a reference run: (t_ms, him), from its `boss.csv`.
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

/// One route played in the game.
struct Played {
    rows: Vec<TraceRow>,
    him: Vec<Option<Him>>,
    /// The tick his first pirate appeared without being sent on his
    /// attack, if one did (the pirate's boss-fight hooks failed).
    hooks_idle_from: Option<usize>,
}

/// Plays route `r` at `clock` from its time zero for `ticks` ticks.
fn play(
    level: &SimLevel,
    r: &Route,
    (seed, t0_ms): (u32, u64),
    clock: &str,
    ticks: usize,
) -> Played {
    let input = route_ticks(r);
    let mut sim = Sim::new(level.clone(), config(clock, seed));
    start_at(&mut sim, START, t0_ms, seed, r.catnip_ms);
    let mut rows = Vec::new();
    let mut hims = Vec::new();
    let mut known_pirates = Vec::new();
    let mut hooks_idle_from = None;
    for tick in 0..ticks {
        if r.end_life == Some(tick) {
            sim.end_life();
        }
        sim.step(input.get(tick).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, tick as u64, "script"));
        hims.push(him(&sim));
        for p in his_pirates(&sim) {
            if known_pirates.contains(&p) {
                continue;
            }
            known_pirates.push(p);
            if !sent_on_his_attack(&sim, p) && hooks_idle_from.is_none() {
                hooks_idle_from = Some(tick);
            }
        }
        if let Some((after, hits)) = r.hits_after
            && tick == after
        {
            set_cannon_hits(&mut sim, hits);
        }
        through_the_door(&mut sim);
    }
    Played {
        rows,
        him: hims,
        hooks_idle_from,
    }
}

/// Sets his count of cannon hits (the original's diagnostic run does).
fn set_cannon_hits(sim: &mut Sim, hits: i32) {
    let id = find_him(sim).expect("Gabriel");
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id)
            && let ObjectData::Gabriel(d) = &mut o.data
        {
            d.cannon_hits = hits;
        }
    });
}

/// The first reference time (ms) Gabriel's x, y, state and health leave
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

// ----------------------------------------------------------------------
// What is known

/// A store, a clock, and the first divergence (ms) of each column known
/// there; a column not listed must stay within its tolerance, and one
/// listed must not leave it earlier. Claw standing still for 17 s (after
/// the skip, after the button, on the deck with Catnip, after a throw)
/// follows the original to the draw of his idle line as his idle
/// animation ends. What the entries come from:
///
/// - Claw's falling death in the water a pixel off (5244 against 5245):
///   the original's fall reads its vertical step twice per tick; the rule
///   comes with the third level's route work, merged later. After it, his
///   respawn: the original runs its two update passes with no frame time
///   and on the death point's objects first, so the camera, the animations
///   (his idle pose two ticks shorter here) and the draws differ from
///   there; those rules come with the same work.
/// - A life ended through the in-game menu: the original's frame pulse
///   countdown stands still in that tick (the menu takes the frame), so
///   every pulse draw comes a tick later from there on; the replay plays
///   no time for the menu.
/// - A cannonball striking Gabriel while he is off screen (Claw at the
///   button's foot): the original shows no hit burst for an enemy it did
///   not draw and makes no draw for its size; this build still shows it.
///   The rule comes with the third level's route work, merged later.
/// - The cannon death (the diagnostic run): the gem rising out of him
///   moves in the tick it is made in this build (it is listed after
///   Claw), where the original's first call only leads the camera; the
///   camera and Claw's release follow a tick early, and so does the draw
///   of the gem's glitter, which makes its first call as the gem becomes
///   the end-of-level pickup (44226 ms).
/// - c60 (the default tuning played tick by tick): the walk into the door
///   ends a pixel short of the original's 60 frames per second walk (the
///   checkpoint event), the introduction's delays count 18 ms ticks against
///   frames of 16 and 17 ms (the fight and his first idle start about a
///   quarter of a second apart), the skip's arc differs by a few pixels,
///   and with Catnip Claw reaches the room a tick before the route's
///   jump, so the stager fires and the introduction runs: those routes
///   part from the first ticks in the room.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

const KNOWN: &[Known] = &[
    // The first cannon hit strikes him off screen (the hit burst).
    (
        "boss-button-normal-c18",
        "c18",
        &[
            ("rng", 29736),
            ("his_state", 38052),
            ("health", 39366),
            ("state_id", 39366),
            ("cam_x", 39384),
            ("x", 39384),
        ],
    ),
    // The menu tick, then the death fall and the respawn.
    (
        "boss-button-normal-end-life-c18",
        "c18",
        &[
            ("rng", 19818),
            ("y", 20700),
            ("cam_x", 21510),
            ("his_state", 23886),
            ("boss_health", 25542),
            ("his_health", 25542),
            ("health", 29988),
            ("state_id", 29988),
            ("x", 30006),
            ("physics_type", 39816),
            ("events", 41508),
            ("lives", 41508),
        ],
    ),
    // The gem out of him: the camera, then his gem's first call and its
    // glitter's draw a tick early.
    (
        "boss-button-normal-kill-c18",
        "c18",
        &[
            ("cam_y", 42192),
            ("cam_x", 42606),
            ("rng", 44226),
            ("state_id", 44226),
        ],
    ),
    // The skip's arc at the default speeds.
    (
        "boss-button-skip",
        "c60",
        &[
            ("physics_type", 1083),
            ("state_id", 1083),
            ("cam_y", 6833),
            ("y", 6833),
        ],
    ),
    // The skip's arc, then the late introduction's delays.
    (
        "boss-button-skip-return",
        "c60",
        &[
            ("physics_type", 1083),
            ("state_id", 1083),
            ("cam_y", 6833),
            ("y", 6833),
            ("events", 7776),
            ("cam_x", 8000),
            ("boss_fight", 20750),
            ("his_state", 20767),
            ("health", 24817),
            ("x", 24833),
            ("boss_health", 30300),
            ("his_health", 30300),
            ("lives", 40367),
        ],
    ),
    // The death fall and the respawn.
    (
        "boss-button-skip-return-c18",
        "c18",
        &[("y", 47970), ("cam_x", 48780)],
    ),
    // The death fall and the respawn.
    (
        "boss-intro-normal",
        "c18",
        &[
            ("y", 35100),
            ("cam_x", 35910),
            ("his_state", 38412),
            ("rng", 38412),
        ],
    ),
    // The walk's last pixel and the introduction's delays.
    (
        "boss-intro-normal",
        "c60",
        &[
            ("events", 383),
            ("boss_fight", 13350),
            ("his_state", 13367),
            ("state_id", 13367),
            ("health", 16917),
            ("cam_x", 16950),
            ("x", 16950),
            ("physics_type", 36633),
            ("y", 37450),
            ("lives", 38317),
        ],
    ),
    // The skip's arc at the default speeds.
    (
        "boss-intro-skip",
        "c60",
        &[("physics_type", 1083), ("state_id", 1083)],
    ),
    // Claw with Catnip in the room a tick early: the introduction runs.
    (
        "boss-intro-skip-catnip",
        "c60",
        &[
            ("present", 350),
            ("events", 360),
            ("physics_type", 367),
            ("state_id", 367),
            ("x", 367),
            ("boss_fight", 400),
            ("y", 400),
            ("cam_x", 567),
            ("cam_y", 600),
            ("health", 900),
            ("his_state", 13633),
        ],
    ),
    // Claw with Catnip in the room a tick early: the introduction runs.
    (
        "boss-intro-skip-catnip-fight",
        "c60",
        &[
            ("present", 350),
            ("events", 360),
            ("physics_type", 367),
            ("state_id", 367),
            ("x", 367),
            ("boss_fight", 400),
            ("y", 400),
            ("cam_x", 567),
            ("cam_y", 600),
            ("health", 900),
            ("his_state", 1483),
        ],
    ),
    // The skip's arc, then the respawn.
    (
        "boss-intro-skip-end-life",
        "c60",
        &[
            ("physics_type", 1083),
            ("state_id", 1083),
            ("y", 6716),
            ("his_state", 13100),
        ],
    ),
    // The menu tick, then the death fall and the respawn.
    (
        "boss-intro-skip-end-life-c18",
        "c18",
        &[("rng", 5436), ("y", 6300), ("his_state", 9522)],
    ),
    // The walk's last pixel and the introduction's delays.
    (
        "boss-intro-skip-late",
        "c60",
        &[
            ("events", 383),
            ("boss_fight", 13350),
            ("his_state", 13367),
            ("state_id", 13367),
            ("health", 16917),
            ("cam_x", 16950),
            ("x", 16950),
        ],
    ),
    // The death fall and the respawn.
    (
        "boss-intro-skip-late-c18",
        "c18",
        &[
            ("y", 32652),
            ("cam_x", 33462),
            ("his_state", 35838),
            ("rng", 35838),
        ],
    ),
    // The skip's arc, then the late introduction's delays.
    (
        "boss-intro-skip-return",
        "c60",
        &[
            ("physics_type", 1083),
            ("state_id", 1083),
            ("events", 1817),
            ("cam_x", 12283),
            ("boss_fight", 14783),
            ("his_state", 14800),
            ("health", 20083),
            ("x", 20150),
            ("y", 33633),
            ("lives", 34517),
        ],
    ),
    // Claw with Catnip in the room a tick early: the introduction runs.
    (
        "boss-lift-drown",
        "c60",
        &[
            ("present", 350),
            ("events", 360),
            ("physics_type", 367),
            ("state_id", 367),
            ("x", 367),
            ("boss_fight", 400),
            ("y", 400),
            ("cam_x", 567),
            ("cam_y", 600),
            ("health", 900),
            ("his_state", 1383),
            ("his_x", 1383),
            ("his_y", 1383),
            ("boss_health", 4033),
            ("his_health", 4033),
            ("held", 16500),
        ],
    ),
    // Claw with Catnip in the room a tick early: the introduction runs.
    (
        "boss-lift-throw",
        "c60",
        &[
            ("present", 350),
            ("events", 360),
            ("physics_type", 367),
            ("state_id", 367),
            ("x", 367),
            ("boss_fight", 400),
            ("y", 400),
            ("cam_x", 567),
            ("cam_y", 600),
            ("health", 900),
            ("his_state", 1383),
            ("his_x", 1383),
            ("his_y", 1383),
            ("boss_health", 4033),
            ("his_health", 4033),
        ],
    ),
    // The death fall and the respawn.
    (
        "boss-normal-fight",
        "c18",
        &[("y", 38682), ("cam_x", 39492)],
    ),
    // The walk's last pixel and the introduction's delays.
    (
        "boss-normal-fight",
        "c60",
        &[
            ("events", 383),
            ("boss_fight", 13350),
            ("his_state", 13367),
            ("state_id", 13367),
            ("pistol", 17517),
            ("health", 18400),
            ("cam_x", 18467),
            ("x", 18467),
            ("physics_type", 31767),
            ("y", 32567),
            ("lives", 33450),
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
fn install_level8_boss_room_against_the_reference_traces() {
    let Some(level) = level8() else { return };
    let Some(root) = reference_root() else { return };
    let runs = runs_root(&root);
    if runs.is_none() {
        println!("no reference runs beside the store: Gabriel's own columns are not compared");
    }
    let only = std::env::var("CLAW_L8_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level8").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("level8/{}: meta: {e}", r.store));
        let copy =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let mine: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        let first_apart = copy
            .iter()
            .zip(&mine)
            .position(|(a, b)| a != b)
            .or((copy.len() != mine.len()).then_some(copy.len().min(mine.len())));
        assert_eq!(
            first_apart, None,
            "{}: the store's route parts at row",
            r.store
        );
        for &clock in r.clocks {
            let path = dir.join(format!("trace-{clock}-r1.csv"));
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let reference = trace::parse(&text).unwrap();
            let t0_ms: u64 = meta_value(&meta, clock, "t0_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{} {clock}: no time zero in its meta", r.store));
            let expected = T0_MS.iter().find(|(c, _)| *c == clock).map(|&(_, t)| t);
            assert_eq!(Some(t0_ms), expected, "{} {clock}: time zero", r.store);
            let seed: u32 = meta_value(&meta, clock, "rng_at_t0")
                .or_else(|| meta_value(&meta, "", "rng_at_t0"))
                .and_then(|v| v.parse().ok())
                .unwrap_or(SEED);
            let ticks = reference.len().max(r.rows);
            let played = play(&level, r, (seed, t0_ms), clock, ticks);
            let tol = if clock == "c18" {
                Tolerances::exact()
            } else {
                Tolerances::feel()
            };
            assert_eq!(
                played.hooks_idle_from, None,
                "level8/{} {clock}: a pirate of his was not sent on his attack",
                r.store
            );
            let until = played.rows.len();
            let reference_part: Vec<TraceRow> = reference.iter().take(until).cloned().collect();
            let cmp = trace::compare(&played.rows[..until], &reference_part, &tol);
            println!("== level8/{} {clock}", r.store);
            print!(
                "{}",
                trace::report(&played.rows[..until], &reference_part, &cmp, 3)
            );
            assert!(cmp.rows_compared > 0);
            let mut firsts = trace_firsts(&cmp);
            if let Some(runs) = &runs {
                let id = run_id(&meta, clock)
                    .unwrap_or_else(|| panic!("{} {clock}: no run in its meta", r.store));
                let path = runs.join(id).join("boss.csv");
                let his = reference_him(&path)
                    .unwrap_or_else(|| panic!("{}: no Gabriel per step", path.display()));
                let limit = (until as u64) * TICK;
                let his: Vec<(u64, Option<Him>)> =
                    his.into_iter().filter(|(t, _)| *t <= limit).collect();
                for (k, v) in compare_him(&played, &his, clock) {
                    firsts.insert(k.to_string(), v);
                }
            }
            let list = known(r.store, clock);
            for (column, first) in &firsts {
                println!("first {} {clock} {column} {first}", r.store);
                match list.iter().find(|(name, _)| name == column) {
                    Some(&(_, at)) if *first >= at => {}
                    _ => failures.push(format!("{} {clock}: {column} at {first} ms", r.store)),
                }
            }
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ----------------------------------------------------------------------
// The fight's outcomes on their own

/// Plays route `store` at the original speeds from its time zero, calling
/// `each` after every tick with the tick's number; returns the game.
fn run_route(
    level: &SimLevel,
    store: &str,
    ticks: usize,
    mut each: impl FnMut(usize, &Sim),
) -> Sim {
    let r = route_named(store);
    let input = route_ticks(r);
    let mut sim = Sim::new(level.clone(), config("c18", SEED));
    start_at(&mut sim, START, T0_MS[0].1, SEED, r.catnip_ms);
    for tick in 0..ticks {
        if r.end_life == Some(tick) {
            sim.end_life();
        }
        sim.step(input.get(tick).copied().unwrap_or_default());
        each(tick, &sim);
        if let Some((after, hits)) = r.hits_after
            && tick == after
        {
            set_cannon_hits(&mut sim, hits);
        }
        through_the_door(&mut sim);
    }
    sim
}

/// Each change of Gabriel's state: (tick, state).
fn his_states(states: &mut Vec<(usize, EnemyState)>, tick: usize, sim: &Sim) {
    if let Some(h) = him(sim)
        && states.last().is_none_or(|&(_, s)| s != h.state)
    {
        states.push((tick, h.state));
    }
}

/// True when the pirate's boss-fight hooks sent pirate `p`, which he has
/// just called in, on his attack (see the module documentation).
fn sent_on_his_attack(sim: &Sim, p: ObjectId) -> bool {
    sim.objects()
        .get(p)
        .is_some_and(|o| o.state == EnemyState::GabrielAttack as i32)
}

#[test]
fn install_level8_the_introduction_and_his_first_attacks() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let mut first_room_tick = None;
    let mut thaw = None;
    let mut pirate_track = Vec::new();
    let mut hooks_live = false;
    let sim = run_route(&level, "boss-intro-normal", 1000, |tick, sim| {
        his_states(&mut states, tick, sim);
        if first_room_tick.is_none() && sim.player().is_some_and(|c| c.x > 40000) {
            first_room_tick = Some(tick);
        }
        if thaw.is_none()
            && sim.boss().fight() == fight::ON
            && sim.player().unwrap().state == state::STAND
        {
            thaw = Some(tick);
        }
        if let Some(&p) = his_pirates(sim).first()
            && let Some(o) = sim.objects().get(p)
        {
            if pirate_track.is_empty() {
                hooks_live = sent_on_his_attack(sim, p);
            }
            pirate_track.push((tick, o.x, o.y));
        }
    });
    println!("his states: {states:?}");
    // In the room on tick 22, standing again on 758 as he goes idle.
    assert_eq!(first_room_tick, Some(22));
    assert_eq!(thaw, Some(758));
    // He is there, asleep, from Claw's first tick in the room.
    assert_eq!(states[0], (22, EnemyState::Startup));
    assert!(states.contains(&(758, EnemyState::Idle)), "{states:?}");
    // His second idle pose, 135 ticks, then the pirate.
    assert!(
        states.contains(&(893, EnemyState::ThugAttack)),
        "{states:?}"
    );
    assert!(states.contains(&(906, EnemyState::Idle)), "{states:?}");
    assert_eq!(him(&sim).map(|h| (h.x, h.y)), Some(HIM_AT));
    // The pirate appears 150 px right of him and 10 px up, on his deck,
    // runs left and leaps down onto the dock at x 42899.
    let made = pirate_track.first().copied().expect("a pirate");
    assert_eq!(made, (906, 43373, 5038));
    assert!(hooks_live, "his pirate is sent on his attack");
    let landed = pirate_track
        .iter()
        .find(|&&(_, _, y)| y == 5211)
        .copied()
        .expect("the pirate lands");
    assert_eq!(landed, (985, 42899, 5211));
    let top = pirate_track.iter().map(|&(_, _, y)| y).min().unwrap();
    assert_eq!(top, 4831);
}

#[test]
fn install_level8_the_skip_leaves_him_asleep() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let mut fights = Vec::new();
    let sim = run_route(&level, "boss-intro-skip-c18", 1900, |tick, sim| {
        his_states(&mut states, tick, sim);
        if fights.last() != Some(&sim.boss().fight()) {
            fights.push(sim.boss().fight());
        }
    });
    // Asleep in his start-up pose for the whole run; no introduction.
    assert_eq!(states, vec![(22, EnemyState::Startup)]);
    assert_eq!(fights, vec![fight::NONE]);
    assert_eq!(him(&sim).map(|h| h.health), Some(100));
    // The props were set up before Claw's first tick in the room: the
    // cannon waits barrel down, the switch waits.
    let props: Vec<(String, i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| {
            cannon::NAMES.contains(&&*o.logic) || cannon_switch::NAMES.contains(&&*o.logic)
        })
        .map(|(_, o)| (o.logic.to_string(), o.state, o.frame))
        .collect();
    println!("props: {props:?}");
    assert!(props.contains(&(
        cannon::NAMES[0].to_string(),
        EnemyState::Home as i32,
        cannon::REST_FRAME
    )));
    assert!(props.contains(&(
        cannon_switch::NAMES[0].to_string(),
        cannon_switch::state::IDLE,
        1
    )));
}

#[test]
fn install_level8_a_walk_back_into_the_stager_runs_the_introduction() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    run_route(&level, "boss-intro-skip-return-c18", 1000, |tick, sim| {
        his_states(&mut states, tick, sim);
    });
    // The stager fires on tick 104; he wakes 736 ticks later.
    assert_eq!(states[0], (22, EnemyState::Startup));
    assert_eq!(states[1], (840, EnemyState::Idle), "{states:?}");
}

#[test]
fn install_level8_claws_respawn_far_away_wakes_him() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let sim = run_route(&level, "boss-intro-skip-end-life-c18", 700, |tick, sim| {
        his_states(&mut states, tick, sim);
    });
    // Claw comes back at the level's start, far outside all his boxes:
    // he goes idle on the tick of the respawn and fights on alone.
    println!("his states: {states:?}");
    assert_eq!(states[0], (22, EnemyState::Startup));
    assert_eq!(states[1], (394, EnemyState::Idle));
    assert!(states.len() >= 3, "he attacks: {states:?}");
    let c = sim.player().unwrap();
    assert_eq!((c.x, c.y), (1046, 5532));
}

#[test]
fn install_level8_the_lift_and_the_drown() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let mut gone = None;
    let mut end = None;
    let mut gem_from = None;
    run_route(&level, "boss-lift-drown-c18", 1100, |tick, sim| {
        his_states(&mut states, tick, sim);
        if gone.is_none() && find_him(sim).is_none() && tick > 100 {
            gone = Some(tick);
        }
        if gem_from.is_none()
            && let Some((_, o)) = sim
                .objects()
                .all()
                .find(|(_, o)| &*o.logic == pickup::END_GEM)
        {
            gem_from = Some((tick, o.x, o.y));
        }
        if end.is_none()
            && sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            end = Some(tick);
        }
    });
    println!("his states: {states:?}");
    // Lifted from his start-up pose, thrown onto the dock, up again in a
    // pause, lifted again 1500 ms after the landing, thrown into the
    // water: the death band takes him and the gem rises from there. With
    // Catnip Claw is in the room on tick 20.
    assert_eq!(states[0], (20, EnemyState::Startup));
    assert!(states.contains(&(76, EnemyState::Lifted)), "{states:?}");
    assert!(states.contains(&(114, EnemyState::Thrown)), "{states:?}");
    assert!(states.contains(&(164, EnemyState::Bounce)), "{states:?}");
    assert!(states.contains(&(312, EnemyState::Lifted)), "{states:?}");
    assert!(states.contains(&(520, EnemyState::Thrown)), "{states:?}");
    assert_eq!(gone, Some(591));
    assert_eq!(gem_from, Some((591, 42241, 5762)));
    assert_eq!(end, Some(893));
}

#[test]
fn install_level8_after_a_throw_he_stays_in_his_strike() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let mut lifted_frames = Vec::new();
    run_route(&level, "boss-lift-throw", 1400, |tick, sim| {
        his_states(&mut states, tick, sim);
        if let Some(id) = find_him(sim)
            && (76..400).contains(&tick)
        {
            lifted_frames.push(sim.objects().get(id).unwrap().frame);
        }
    });
    println!("his states: {states:?}");
    // He keeps the picture he had when lifted, all the way.
    assert!(lifted_frames.iter().all(|&f| f == lifted_frames[0]));
    // Down on the dock on 164, a pause, then Home and a pause six times
    // more (each Home draws whether he strikes), then his strike for good.
    let pauses = states
        .iter()
        .filter(|&&(t, s)| t > 164 && s == EnemyState::Pause)
        .count();
    assert_eq!(pauses, 7, "{states:?}");
    assert_eq!(states.last().copied(), Some((382, EnemyState::Strike)));
}

#[test]
fn install_level8_the_button_raises_his_cannon() {
    let Some(level) = level8() else { return };
    let mut cannon_states = Vec::new();
    let mut pressed = None;
    run_route(&level, "boss-button-skip-c18", 1700, |tick, sim| {
        if let Some((_, o)) = sim
            .objects()
            .all()
            .find(|(_, o)| cannon::NAMES.contains(&&*o.logic))
            && cannon_states.last().is_none_or(|&(_, s, _)| s != o.state)
        {
            cannon_states.push((tick, o.state, o.frame));
        }
        if pressed.is_none() && his_data(sim).is_some_and(|d| d.link.button) {
            pressed = Some(tick);
        }
    });
    println!("cannon: {cannon_states:?}");
    // Struck on 349: the cannon turns up from 350 and is home again,
    // barrel up, on 389; asleep, Gabriel gives no order.
    assert_eq!(pressed, Some(349));
    assert!(cannon_states.contains(&(350, EnemyState::Rotate as i32, 1)));
    let last = cannon_states.last().copied().unwrap();
    assert_eq!(last, (389, EnemyState::Home as i32, cannon::RAISED_FRAME));
}

#[test]
fn install_level8_five_cannon_hits_and_his_death_acts() {
    let Some(level) = level8() else { return };
    let mut states = Vec::new();
    let mut healths = Vec::new();
    let mut end = None;
    let mut released = None;
    let mut hooks_live = None;
    let sim = run_route(&level, "boss-button-normal-kill-c18", 2700, |tick, sim| {
        his_states(&mut states, tick, sim);
        if hooks_live.is_none()
            && let Some(&p) = his_pirates(sim).first()
        {
            hooks_live = Some(sent_on_his_attack(sim, p));
        }
        if let Some(h) = him(sim)
            && healths.last().is_none_or(|&(_, v)| v != h.health)
        {
            healths.push((tick, h.health));
        }
        if released.is_none() && tick > 2341 && sim.player().unwrap().state == state::STAND {
            released = Some(tick);
        }
        if end.is_none()
            && sim
                .last_events()
                .iter()
                .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            end = Some(tick);
        }
    });
    println!("his states: {states:?}");
    println!("his health: {healths:?}");
    // The pirates he calls in draw from the stream: without their
    // boss-fight hooks his attacks would come at other times.
    assert_eq!(hooks_live, Some(true), "his pirates are sent on his attack");
    // The cannon's upward ball strikes him on 1651 and, his fifth, starts
    // his death acts 19 ticks later; they last 224, 32, 122 and 293
    // ticks, the second landing him on the dock.
    assert!(
        states.contains(&(1651, EnemyState::CannonHit)),
        "{states:?}"
    );
    assert!(states.contains(&(1670, EnemyState::DeadAct1)), "{states:?}");
    assert!(states.contains(&(1894, EnemyState::DeadAct2)), "{states:?}");
    assert!(states.contains(&(1926, EnemyState::DeadAct3)), "{states:?}");
    assert!(states.contains(&(2048, EnemyState::DeadAct4)), "{states:?}");
    assert!(healths.contains(&(1670, 0)), "{healths:?}");
    assert_eq!(sim.boss().defeated(), find_him(&sim));
    assert_eq!(him(&sim).map(|h| (h.x, h.y)), Some((43008, 5238)));
    // Gone? Never: he stays on the dock, his empty bar on screen.
    assert!(find_him(&sim).is_some());
    // The gem lands and lets Claw go 116 ticks after it rose (115 here,
    // see the known divergences of the store); his jump ends the level.
    println!("Claw free again on {released:?}, the level ends on {end:?}");
    assert_eq!(released, Some(2456));
    assert_eq!(end, Some(2528));
}

#[test]
fn install_level8_the_reset_keeps_his_cannon_firing_upwards() {
    let Some(level) = level8() else { return };
    let mut respawn = None;
    let mut fired_after = Vec::new();
    let mut hits = Vec::new();
    let mut frames_after_first = Vec::new();
    run_route(
        &level,
        "boss-button-normal-end-life-c18",
        2900,
        |tick, sim| {
            let c = sim.player().unwrap();
            if respawn.is_none() && tick > 1100 && c.state == state::STAND && c.x == 43058 {
                respawn = Some(tick);
            }
            let Some(r) = respawn else { return };
            if let Some((_, o)) = sim
                .objects()
                .all()
                .find(|(_, o)| cannon::NAMES.contains(&&*o.logic))
            {
                if tick == r {
                    // Reset: barrel down on screen.
                    assert_eq!(o.frame, cannon::REST_FRAME);
                }
                if (o.state == EnemyState::Fire as i32 || o.state == EnemyState::VertFiring as i32)
                    && fired_after.last() != Some(&o.state)
                {
                    fired_after.push(o.state);
                }
                if !fired_after.is_empty() {
                    frames_after_first.push((o.state, o.frame));
                }
            }
            if let Some(h) = him(sim)
                && h.state == EnemyState::CannonHit
                && hits.last().is_none_or(|&t: &usize| tick > t + 50)
            {
                hits.push(tick);
            }
        },
    );
    println!("respawn {respawn:?}, shots after it {fired_after:?}, hits {hits:?}");
    assert!(respawn.is_some());
    // Every shot after the reset goes up and strikes him; the cannon
    // never turns down again.
    assert!(!fired_after.is_empty());
    assert!(
        fired_after
            .iter()
            .all(|&s| s == EnemyState::VertFiring as i32)
    );
    assert!(!hits.is_empty());
    assert!(
        frames_after_first
            .iter()
            .all(|&(s, _)| s != EnemyState::Rotate as i32)
    );
}
