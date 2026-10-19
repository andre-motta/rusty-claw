//! The eleventh level's Mercat and Fish on your own install, against the
//! original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), waits until the level clock reaches the time the
//! original's run started at, and starts the shared random stream from the
//! run's state, once. The enemies near him were set going before that time
//! with the original's own draws, which the replay does not have: at time
//! zero each Mercat near him is put in the state and given the timers the
//! original's run shows ([`Align`]); the fish need nothing, they swim and
//! leap there as in the original's runs. The routes:
//!
//! - beside the Mercat on the floor at 4567: Claw put down 233 px to his
//!   right stands (his bolts, every one or two seconds, until Claw's last
//!   life of the run is gone and he is back at his checkpoint), or 60 px
//!   to his right stands (the stab chosen and replaced by the bolt, whose
//!   explosion kills him);
//! - beside the Mercat on the floor at 10268: Claw put down 120 px to his
//!   left swings his sword every 15 rows from row 20 (the hop back, then
//!   bolts from his new place), or 115 px to his left swings from row 80
//!   (busy after the bolt that replaced his stab, so no hop follows);
//! - in the wading pool at 5980 to 6504: Claw stands at 6240, above two
//!   fish's lines (their leaps) and level with the third's (its dashes);
//! - in the pool at 10206 to 10659: Claw stands at 10300 (the dashes);
//! - on the bank right of the pool at 13854 to 14498: Claw stands at 14540
//!   (the leaps at the pit's wall, which bounce back into the pool).
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched Mercats and fish are compared too, their
//! position, state and health exactly at the 18 ms step, their states lined
//! up by the numbers the run gives them ([`reference_state`]).
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`]
//! (and, while a neighbour of a route is not built yet, in [`UNBUILT`]);
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace (or, with the reference runs
//! there, a run's `obj-log.csv` or a watched enemy in it);
//! `CLAW_L11_ONLY` names one store to play alone. Nothing from an install
//! or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, fish, mercat, siren};
use claw_sim::logic::projectile::trident;
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, engine_state};
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

/// A stretch repeated: its first row, its length, its keys, the distance
/// between two repeats and the first row no repeat starts on.
#[derive(Debug, Clone, Copy)]
struct Every {
    first: usize,
    len: usize,
    held: u32,
    gap: usize,
    until: usize,
}

const fn every(first: usize, len: usize, held: u32, gap: usize, until: usize) -> Option<Every> {
    Some(Every {
        first,
        len,
        held,
        gap,
        until,
    })
}

/// A Mercat at a route's time zero as the original's run shows him: the
/// one placed at `at`, in `state`, his wait started on level tick
/// `wait.0` for `wait.1` ms, and his idle time started on level tick
/// `idle` (level tick 1 is the level's first).
#[derive(Debug, Clone, Copy)]
struct Align {
    at: (i32, i32),
    state: EnemyState,
    wait: (u64, u64),
    idle: u64,
}

const fn align(at: (i32, i32), state: EnemyState, wait: (u64, u64), idle: u64) -> Align {
    Align {
        at,
        state,
        wait,
        idle,
    }
}

/// The two families watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Mercat,
    Fish,
}

impl Family {
    fn logic(self) -> &'static str {
        match self {
            Family::Mercat => mercat::NAMES[0],
            Family::Fish => fish::NAMES[0],
        }
    }
}

/// A watched enemy: its family, its serial number in the original's runs
/// and where it is placed.
#[derive(Debug, Clone, Copy)]
struct Watched {
    family: Family,
    serial: &'static str,
    at: (i32, i32),
}

const fn mercat_at(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        family: Family::Mercat,
        serial,
        at,
    }
}

const fn fish_at(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        family: Family::Fish,
        serial,
        at,
    }
}

/// A route of the stores: its name, where Claw is put down, the
/// original's game time at its time zero at the 18 ms step, how many rows
/// it has, its stretches, its repeated stretch, the clocks it was run at,
/// the Mercats put in step at time zero and the enemies it watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
    clocks: &'static [&'static str],
    align: &'static [Align],
    watch: &'static [Watched],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// Mercat 5989, on the floor at 4567, and the Mercat below the start room.
const MERCAT_4567: (i32, i32) = (4567, 1376);
const MERCAT_3935: (i32, i32) = (3935, 1764);
/// Mercat 7463, on the floor at 10268, and his two neighbours.
const MERCAT_10268: (i32, i32) = (10268, 2836);
const MERCAT_9797: (i32, i32) = (9797, 2588);
const MERCAT_9884: (i32, i32) = (9884, 3169);
/// The Mercats on either side of the pool at 5980 to 6504.
const MERCAT_5896: (i32, i32) = (5896, 1369);
const MERCAT_6783: (i32, i32) = (6783, 1373);
/// The Mercat above the pool at 13854 to 14498.
const MERCAT_14491: (i32, i32) = (14491, 1315);

/// The three Mercats near Mercat 7463, as the runs of both his routes show
/// them at time zero.
const ALIGN_10268: [Align; 3] = [
    align(MERCAT_10268, EnemyState::Home, (2, 1535), 1),
    align(MERCAT_9797, EnemyState::Idle, (2, 1000), 2),
    align(MERCAT_9884, EnemyState::Idle, (2, 1000), 2),
];
const WATCH_10268: [Watched; 3] = [
    mercat_at("3195", MERCAT_10268),
    mercat_at("3196", MERCAT_9797),
    mercat_at("3197", MERCAT_9884),
];

const ROUTES: [Route; 7] = [
    Route {
        store: "merc-bolt",
        start: (4800, 1339),
        t0_ms: 828,
        rows: 700,
        spans: &[],
        every: None,
        clocks: &["c18", "c60"],
        align: &[align(MERCAT_4567, EnemyState::Home, (2, 1026), 1)],
        watch: &[mercat_at("733", MERCAT_4567)],
    },
    Route {
        store: "merc-close",
        start: (4627, 1339),
        t0_ms: 828,
        rows: 400,
        spans: &[],
        every: None,
        clocks: &["c18", "c60"],
        align: &[
            align(MERCAT_3935, EnemyState::Idle, (1, 1000), 1),
            align(MERCAT_4567, EnemyState::Home, (2, 1187), 1),
        ],
        watch: &[mercat_at("733", MERCAT_4567), mercat_at("726", MERCAT_3935)],
    },
    Route {
        store: "merc-hop",
        start: (10148, 2811),
        t0_ms: 828,
        rows: 500,
        spans: &[],
        every: every(20, 2, ATTACK, 15, 400),
        clocks: &["c18", "c60"],
        align: &ALIGN_10268,
        watch: &WATCH_10268,
    },
    Route {
        store: "merc-busy",
        start: (10153, 2811),
        t0_ms: 828,
        rows: 400,
        spans: &[],
        every: every(80, 2, ATTACK, 15, 300),
        clocks: &["c18"],
        align: &ALIGN_10268,
        watch: &WATCH_10268,
    },
    Route {
        store: "fish-leap",
        start: (6240, 1723),
        t0_ms: 828,
        rows: 600,
        spans: &[],
        every: None,
        clocks: &["c18", "c60"],
        align: &[
            align(MERCAT_5896, EnemyState::Idle, (2, 1000), 2),
            align(MERCAT_6783, EnemyState::Idle, (2, 1000), 2),
        ],
        watch: &[
            fish_at("3119", (6379, 1742)),
            fish_at("3120", (6111, 1744)),
            fish_at("3121", (6245, 1745)),
            mercat_at("734", MERCAT_5896),
            mercat_at("735", MERCAT_6783),
        ],
    },
    Route {
        store: "fish-dash",
        start: (10300, 2363),
        t0_ms: 828,
        rows: 700,
        spans: &[],
        every: None,
        clocks: &["c18", "c60"],
        align: &[
            align(MERCAT_9797, EnemyState::Idle, (2, 1000), 2),
            align(MERCAT_3935, EnemyState::Idle, (1, 1000), 1),
        ],
        watch: &[
            fish_at("3058", (10290, 2382)),
            fish_at("3059", (10422, 2382)),
            fish_at("3060", (10584, 2383)),
        ],
    },
    Route {
        store: "fish-bank",
        start: (14540, 1659),
        t0_ms: 828,
        rows: 700,
        spans: &[],
        every: None,
        clocks: &["c18"],
        align: &[align(MERCAT_14491, EnemyState::Idle, (2, 1000), 2)],
        watch: &[
            fish_at("2837", (14370, 1871)),
            fish_at("2838", (14111, 1871)),
            fish_at("2839", (13917, 1870)),
            mercat_at("2840", MERCAT_14491),
        ],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, merc-close, 1476 ms (`733_y`): the Mercat killed by his own
///   bolt's explosion flies off on the same arc, but a pixel lower on one
///   row: the shared flight of a killed enemy starts from his whole-pixel
///   height, the original's from a part of a pixel below it. The body
///   touches nothing on its way.
/// - c60 (the default tuning played tick by tick): the first bolt and the
///   first hop come within the tolerances, but the original at 60 frames
///   per second makes the enemies' draws once a frame, so the waits after
///   them (1000 ms and a draw) differ from the replay's, the later bolts
///   reach Claw at other times, and his health, state and position follow
///   (on merc-bolt and merc-hop he loses his life before the end; on
///   fish-dash the fish's touches come at other times and his last fall
///   with them).
const KNOWN: &[Known] = &[
    ("merc-close", "c18", &[("733_y", 1476)]),
    (
        "merc-bolt",
        "c60",
        &[
            ("state_id", 2650),
            ("health", 2650),
            ("x", 2683),
            ("cam_x", 2683),
            ("score", 4367),
            ("physics_type", 7700),
            ("y", 8517),
            ("lives", 9367),
            ("events", 9367),
        ],
    ),
    (
        "merc-hop",
        "c60",
        &[
            ("state_id", 2350),
            ("health", 2350),
            ("x", 2383),
            ("cam_x", 2383),
            ("score", 4483),
            ("lives", 6033),
            ("physics_type", 9617),
            ("y", 10433),
        ],
    ),
    ("fish-dash", "c60", &[("y", 11383)]),
];

/// Which unbuilt family a route meets besides the enemies it is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Neighbour {
    /// A Siren within reach of the pool.
    Siren,
}

impl Neighbour {
    /// True while it is not built.
    fn unbuilt(self) -> bool {
        match self {
            Neighbour::Siren => !siren::BUILT,
        }
    }
}

/// The divergences an unbuilt neighbour causes, per store and clock, as
/// [`KNOWN`]: in the original's runs the Siren draws from the shared
/// random stream (her roll on every third tick, her decisions' draws), so
/// the random state leaves the original's on the first row, and the later
/// draws of the fish and the Mercats nearby come from another stream: a
/// Mercat's first idle pose or pacing start, and a fish's leap, come on
/// other rows.
const UNBUILT: &[(Neighbour, Known)] = &[
    (
        Neighbour::Siren,
        (
            "fish-leap",
            "c18",
            &[
                ("rng", 18),
                ("735_x", 1242),
                ("735_state", 1242),
                ("734_x", 1278),
                ("734_state", 1278),
                ("3120_x", 9684),
                ("3120_y", 9684),
                ("3120_state", 9684),
            ],
        ),
    ),
    (
        Neighbour::Siren,
        (
            "fish-bank",
            "c18",
            &[
                ("rng", 18),
                ("2840_x", 1278),
                ("2840_state", 1278),
                ("2838_x", 8604),
                ("2838_y", 8604),
                ("2838_state", 8604),
            ],
        ),
    ),
];

fn known(store: &str, clock: &str) -> Vec<(&'static str, u64)> {
    let mut out: Vec<(&'static str, u64)> = KNOWN
        .iter()
        .filter(|(s, c, _)| *s == store && *c == clock)
        .flat_map(|(_, _, k)| k.iter().copied())
        .collect();
    for (n, (s, c, k)) in UNBUILT {
        if n.unbuilt() && *s == store && *c == clock {
            out.extend(k.iter().copied());
        }
    }
    out
}

/// The state a reference run's number stands for, for an enemy of
/// `family`.
fn reference_state(family: Family, id: i32) -> Option<EnemyState> {
    Some(match (family, id) {
        (_, 2004) => EnemyState::Dead,
        (Family::Fish, 2069) => EnemyState::Swim,
        (Family::Fish, 2065) => EnemyState::Snip,
        (Family::Fish, 2078) => EnemyState::JumpStrike,
        (Family::Fish, 2079) => EnemyState::OutOfWater,
        (Family::Mercat, 2000) => EnemyState::Idle,
        (Family::Mercat, 2003) => EnemyState::Strike,
        (Family::Mercat, 2008) => EnemyState::Bounce,
        (Family::Mercat, 2009) => EnemyState::Lifted,
        (Family::Mercat, 2010) => EnemyState::Thrown,
        (Family::Mercat, 2011) => EnemyState::Pause,
        (Family::Mercat, 2012) => EnemyState::Dropped,
        (Family::Mercat, 2017) => EnemyState::Flailing,
        (Family::Mercat, 2018) => EnemyState::Home,
        (Family::Mercat, 2019) => EnemyState::JumpAtClaw,
        (Family::Mercat, 2020) => EnemyState::JumpBack,
        (Family::Mercat, 2021) => EnemyState::Advance,
        (Family::Mercat, 2022) => EnemyState::FastAdvance,
        (Family::Mercat, 2023) => EnemyState::Pace,
        (Family::Mercat, 2024) => EnemyState::HitLow,
        (Family::Mercat, 2025) => EnemyState::HitHigh,
        (Family::Mercat, 2027) => EnemyState::Fire,
        (Family::Mercat, 2035) => EnemyState::Block,
        _ => return None,
    })
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
    let mut spans: Vec<Span> = r.spans.to_vec();
    if let Some(e) = r.every {
        let mut from = e.first;
        while from < e.until.min(r.rows) {
            spans.push(span(from, (from + e.len - 1).min(r.rows - 1), e.held));
            from += e.gap;
        }
    }
    for s in &spans {
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

fn level11() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(11)).unwrap();
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

/// The enemy of `family` placed at `at`.
fn placed(sim: &Sim, family: Family, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == family.logic())
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {} placed at {at:?}", family.logic()))
}

/// The game time the timers read during level tick `tick` (the first is
/// 1): the time at its start.
fn tick_start(tick: u64) -> u64 {
    (tick - 1) * TICK
}

/// Puts the Mercats of `r` in step with the original's run at time zero
/// (see [`Align`]).
fn put_in_step(sim: &mut Sim, r: &Route) {
    for a in r.align {
        let id = placed(sim, Family::Mercat, a.at);
        sim.with_world(|w| {
            if w.objects
                .get(id)
                .is_some_and(|o| o.state == engine_state::INIT)
            {
                // Built in the original's first tick and asleep since, his
                // last move made then; the replay has not woken him yet.
                logic::tick(w, id);
                if let Some(d) = enemy::data_mut(w, id) {
                    d.last_move_ms = Some(tick_start(a.idle));
                }
            }
            enemy::set_state(w, id, a.state);
            if a.state == EnemyState::Idle
                && let Some(d) = enemy::data_mut(w, id)
            {
                // Idle with Claw out of his range boxes since his first
                // tick: he went idle for it then.
                d.out_of_range = true;
            }
            if let Some(m) = mercat::mercat_data_mut(w, id) {
                m.wait = enemy::Timer::new(tick_start(a.wait.0), a.wait.1);
                m.idle = enemy::Timer::new(tick_start(a.idle), mercat::IDLE_MS);
            }
        });
    }
}

/// The game at a route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, clock: &str, r: &Route, t0_ms: u64, seed: u32) -> Sim {
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
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(r.start));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    put_in_step(&mut sim, r);
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// A store's time zero and random state for `clock`.
fn time_zero(dir: &Path, clock: &str) -> Option<(u64, u32)> {
    let meta = std::fs::read_to_string(dir.join("meta.toml")).ok()?;
    let t0 = meta_value(&meta, clock, "t0_ms")?.parse().ok()?;
    let seed = meta_value(&meta, clock, "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))?
        .parse()
        .ok()?;
    Some((t0, seed))
}

/// A watched enemy at one step of a run or of the replay: position, state
/// (the run's own numbers lined up through [`reference_state`]) and
/// health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnemyAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
    health: i32,
}

/// The steps of the watched enemies in a run's `obj-log.csv`, by enemy:
/// (step, the enemy there).
fn reference_enemies(path: &Path, watch: &[Watched]) -> Option<Vec<Vec<(usize, EnemyAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, health) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("health")?,
    );
    let mut out = vec![Vec::new(); watch.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = watch.iter().position(|w| f.get(serial) == Some(&w.serial)) else {
            continue;
        };
        out[i].push((
            f[tick].parse().ok()?,
            EnemyAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(watch[i].family, int(f[st])?),
                health: int(f[health])?,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's enemy leaves the
/// run's, by column (`<serial>_x`, `_y`, `_state`, `_health`); a step on
/// which the replay has no such enemy counts for all four.
fn compare_enemy(
    serial: &str,
    played: &[Option<EnemyAt>],
    reference: &[(usize, EnemyAt)],
) -> Vec<(String, u64)> {
    let mut firsts: Vec<(String, u64)> = Vec::new();
    let mut note = |name: &str, k: usize| {
        let name = format!("{serial}_{name}");
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in reference {
        let Some(got) = played.get(k) else { continue };
        let Some(got) = got else {
            for name in ["x", "y", "state", "health"] {
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
        if got.health != want.health {
            note("health", k);
        }
    }
    firsts
}

/// One play of a route: its trace, the reference trace, and the watched
/// enemies on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    enemies: Vec<Vec<Option<EnemyAt>>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level11").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let ids: Vec<ObjectId> = r
        .watch
        .iter()
        .map(|w| placed(&sim, w.family, w.at))
        .collect();
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut enemies = vec![Vec::new(); ids.len()];
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            enemies[i].push(sim.objects().get(id).map(|o| EnemyAt {
                x: o.x,
                y: o.y,
                state: EnemyState::from_id(o.state),
                health: o.health,
            }));
        }
        k += 1;
    }
    Played {
        rows,
        reference,
        enemies,
    }
}

/// Compares a play of route `r` at `clock` with the reference trace at
/// the clock's tolerances (exact for c18, feel for c60), and at c18 its
/// watched enemies with the run's (`enemies`, when the run lists them);
/// returns the divergences not known.
fn new_divergences(
    r: &Route,
    clock: &str,
    played: &Played,
    enemies: Option<&[Vec<(usize, EnemyAt)>]>,
) -> Vec<String> {
    let store = r.store;
    let (rows, reference) = (&played.rows, &played.reference);
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(rows, reference, &tol);
    print!("{}", trace::report(rows, reference, &cmp, 3));
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
    if let Some(e) = enemies {
        for (i, w) in r.watch.iter().enumerate() {
            firsts.extend(compare_enemy(w.serial, &played.enemies[i], &e[i]));
        }
    }
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

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level11").join(r.store);
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
        let teleport = meta_value(&meta, "c18", "teleport").expect("a teleport");
        assert_eq!(
            teleport,
            format!("{},{}", r.start.0, r.start.1),
            "{}: start",
            r.store
        );
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_level11_mercat_and_fish_routes_follow_the_reference_traces() {
    let Some(level) = level11() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L11_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level11").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for &clock in r.clocks {
            println!("== level11/{} {clock}", r.store);
            let played = play(&level, &root, r, clock);
            let enemies = runs
                .as_ref()
                .filter(|_| clock == "c18")
                .and_then(|d| Some(d.join(run_id(&meta, clock)?).join("obj-log.csv")))
                .and_then(|p| reference_enemies(&p, r.watch));
            if clock == "c18" && enemies.is_none() {
                if runs.is_some() {
                    failures.push(format!(
                        "{}: the run's object log lacks a watched enemy",
                        r.store
                    ));
                } else {
                    println!("the runs' object logs are not there: the enemies are not compared");
                }
            }
            failures.extend(new_divergences(r, clock, &played, enemies.as_deref()));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw, the first watched enemy, and the
/// bolts in the level.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    x: i32,
    health: i32,
    ex: i32,
    ey: i32,
    es: Option<EnemyState>,
    ehealth: i32,
    busy: bool,
    hop_chance: i32,
    bolts: Vec<(ObjectId, i32, i32, bool)>,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching the enemy placed
/// at `at` and the bolts.
fn watch(level: &SimLevel, store: &str, rows: usize, family: Family, at: (i32, i32)) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, r.t0_ms, SEED);
    let id = placed(&sim, family, at);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        let e = sim.objects().get(id);
        let m = e.and_then(|o| match &o.data {
            ObjectData::Mercat(m) => Some(m),
            _ => None,
        });
        looks.push(Look {
            x: c.x,
            health: sim.claw().stats.health,
            ex: e.map_or(0, |o| o.x),
            ey: e.map_or(0, |o| o.y),
            es: e.and_then(|o| EnemyState::from_id(o.state)),
            ehealth: e.map_or(0, |o| o.health),
            busy: e.and_then(enemy::data_of).is_some_and(|d| d.busy),
            hop_chance: m.map_or(0, |m| m.hop_chance),
            bolts: sim
                .objects()
                .all()
                .filter(|(_, o)| o.is_listed() && &*o.logic == trident::LOGIC)
                .map(|(b, o)| {
                    (
                        b,
                        o.x,
                        o.y,
                        o.type_flags == claw_sim::object::types::LEVEL_SHOT,
                    )
                })
                .collect(),
        });
    }
    looks
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// The spans (first, last) of the rows on which the enemy is in `s`.
fn spans(rows: &[Look], s: EnemyState) -> Vec<(usize, usize)> {
    let f = |l: &Look| l.es == Some(s);
    starts(rows, f)
        .into_iter()
        .map(|a| {
            let b = (a..rows.len()).take_while(|&k| f(&rows[k])).last().unwrap();
            (a, b)
        })
        .collect()
}

/// The rows on which Claw's health drops.
fn hits_on_claw(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect()
}

/// The new bolts of each row: (row, the bolt as it first shows).
fn new_bolts(rows: &[Look]) -> Vec<(usize, (i32, i32))> {
    let mut out = Vec::new();
    for k in 0..rows.len() {
        for &(id, x, y, _) in &rows[k].bolts {
            if k == 0 || !rows[k - 1].bolts.iter().any(|b| b.0 == id) {
                out.push((k, (x, y)));
            }
        }
    }
    out
}

#[test]
fn install_level11_mercat_throws_a_bolt_ten_ticks_after_he_decides() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "merc-bolt", 520, Family::Mercat, MERCAT_4567);
    // Claw in sight 233 px to his right: every decision is a throw, ten
    // rows long, the bolt leaving on the row after it from 28 px in front
    // of him at his height; he never walks.
    let fires = spans(&rows, EnemyState::Fire);
    assert_eq!(
        fires,
        [
            (12, 21),
            (110, 119),
            (208, 217),
            (277, 286),
            (391, 400),
            (466, 475)
        ]
    );
    let bolts = new_bolts(&rows);
    let thrown: Vec<usize> = bolts.iter().map(|b| b.0).collect();
    assert_eq!(thrown, [22, 120, 218, 287, 401, 476]);
    assert!(bolts.iter().all(|b| b.1 == (4595, 1369)), "{bolts:?}");
    assert!(rows.iter().all(|l| l.ex == 4567 && l.ey == 1369));
    // 7 or 8 px a row; the first meets Claw 21 rows later and bursts there
    // for 34 rows.
    let first = rows[22].bolts[0].0;
    let xs: Vec<i32> = rows[22..26]
        .iter()
        .map(|l| l.bolts.iter().find(|b| b.0 == first).unwrap().1)
        .collect();
    assert_eq!(xs, [4595, 4602, 4609, 4616]);
    let bursting: Vec<usize> = (0..rows.len())
        .filter(|&k| rows[k].bolts.iter().any(|b| b.0 == first && b.3))
        .collect();
    assert_eq!((bursting[0], *bursting.last().unwrap()), (43, 76));
    assert_eq!(bursting.len(), trident::EXPLOSION_TICKS as usize);
    assert_eq!(rows[43].bolts[0].1, 4746);
    // Each bolt costs Claw 20.
    assert_eq!(hits_on_claw(&rows)[..5], [43, 145, 247, 320, 437]);
    assert_eq!(rows[43].health, 80);
}

#[test]
fn install_level11_mercat_blows_himself_up_in_front_of_claw() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "merc-close", 60, Family::Mercat, MERCAT_4567);
    // Claw 60 px in front of him: the stab is chosen (busy from then on)
    // and replaced by the throw on the same row; no stab runs.
    assert_eq!(spans(&rows, EnemyState::Fire), [(21, 30)]);
    assert!(spans(&rows, EnemyState::Strike).is_empty());
    assert!(!rows[20].busy && rows[21].busy);
    // The bolt meets Claw at once and bursts where it leaves; its
    // explosion kills him on the same row: 9 health less 20.
    assert_eq!(new_bolts(&rows), [(31, (4595, 1369))]);
    assert!(rows[31].bolts[0].3, "bursting from its first row");
    assert_eq!(hits_on_claw(&rows), [31]);
    assert_eq!(rows[31].health, 80);
    assert_eq!(spans(&rows, EnemyState::Dead)[0].0, 31);
    assert_eq!((rows[30].ehealth, rows[31].ehealth), (9, -11));
}

#[test]
fn install_level11_mercat_hops_back_from_a_swing_while_free() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "merc-hop", 600, Family::Mercat, MERCAT_10268);
    // The first swing, 120 px from him, makes him hop back on the same
    // row: 18 rows in the air, landing 96 px back, free again.
    assert_eq!(spans(&rows, EnemyState::JumpBack), [(20, 37)]);
    assert_eq!((rows[19].ex, rows[37].ex), (10268, 10364));
    assert_eq!((rows[19].hop_chance, rows[20].hop_chance), (90, 60));
    assert!(rows[20].busy && !rows[38].busy);
    // From his new place Claw is out of the hop's reach and he throws.
    assert_eq!(
        starts(&rows, |l| l.es == Some(EnemyState::Fire)),
        [98, 211, 293, 397, 487]
    );
    // Claw out of his boxes at a decision: the chance is back at 90.
    assert_eq!((rows[486].hop_chance, rows[487].hop_chance), (60, 90));
}

#[test]
fn install_level11_mercat_stays_busy_after_a_bolt_over_his_stab() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "merc-busy", 512, Family::Mercat, MERCAT_10268);
    // The stab chosen and replaced by the throw on row 41 leaves him busy
    // for good: the swings from row 80 still draw and lower the chance,
    // 500 ms apart, but he never hops and never moves.
    assert!(!rows[40].busy);
    assert!(rows[41..].iter().all(|l| l.busy));
    assert!(spans(&rows, EnemyState::JumpBack).is_empty());
    assert!(rows.iter().all(|l| l.ex == 10268));
    let chance: Vec<(usize, i32)> = (1..rows.len())
        .filter(|&k| rows[k].hop_chance != rows[k - 1].hop_chance)
        .map(|k| (k, rows[k].hop_chance))
        .collect();
    assert_eq!(chance, [(80, 60), (113, 30), (430, 90)]);
    assert_eq!(
        starts(&rows, |l| l.es == Some(EnemyState::Fire)),
        [41, 110, 209, 323, 430]
    );
}

#[test]
fn install_level11_the_fish_leap_at_claw_above_their_line() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "fish-leap", 712, Family::Fish, (6111, 1744));
    // Swimming towards Claw above its line: 44 rows of leap, peaking 155
    // px up, back on its line on the 45th, 260 px on.
    let leaps = spans(&rows, EnemyState::JumpStrike);
    assert_eq!(leaps[0], (136, 179));
    assert!(
        leaps
            .iter()
            .all(|s| s.1 - s.0 + 1 == fish::LEAP_TICKS as usize)
    );
    assert_eq!(rows[135].ey, 1744);
    let top = rows[136..180].iter().map(|l| l.ey).min().unwrap();
    assert_eq!(top, 1589);
    let at_top: Vec<usize> = (136..180).filter(|&k| rows[k].ey == top).collect();
    assert_eq!(at_top, [156, 157, 158]);
    assert_eq!((rows[180].es, rows[180].ey), (Some(EnemyState::Swim), 1744));
    assert_eq!(rows[180].ex - rows[135].ex, 260);
    // The fish level with Claw dashes: 31 rows at 3 px a row, to his x.
    let dash = watch(&level, "fish-leap", 260, Family::Fish, (6379, 1742));
    assert_eq!(spans(&dash, EnemyState::Snip), [(183, 213)]);
    assert_eq!((dash[182].ex, dash[213].ex), (6141, 6242));
}

#[test]
fn install_level11_the_fish_dash_along_their_line() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "fish-dash", 540, Family::Fish, (10422, 2382));
    let dashes = spans(&rows, EnemyState::Snip);
    assert_eq!(dashes, [(377, 407), (494, 524)]);
    assert_eq!(dashes[0].1 - dashes[0].0 + 1, fish::DASH_TICKS as usize);
    assert!(
        rows.iter()
            .all(|l| l.ey == 2382 || l.es != Some(EnemyState::Snip))
    );
    // Its touches cost Claw 10 each.
    let hits = hits_on_claw(&rows);
    assert!(
        hits.iter()
            .all(|&k| rows[k - 1].health - rows[k].health == 10)
    );
}

#[test]
fn install_level11_a_fish_leaping_at_the_pits_wall_falls_back_in() {
    let Some(level) = level11() else { return };
    let rows = watch(&level, "fish-bank", 700, Family::Fish, (13917, 1870));
    // Claw on the bank: the fish leaps at him near the pit's right end,
    // meets the wall three rows in, comes back at half its speed across
    // and falls back into its pool on the 45th row; Claw is never touched.
    assert_eq!(spans(&rows, EnemyState::JumpStrike), [(357, 400)]);
    let xs: Vec<i32> = rows[357..362].iter().map(|l| l.ex).collect();
    assert_eq!(xs, [14447, 14453, 14459, 14459, 14456]);
    assert_eq!(
        (rows[401].es, rows[401].ex),
        (Some(EnemyState::Swim), 14340)
    );
    assert!(spans(&rows, EnemyState::OutOfWater).is_empty());
    assert!(hits_on_claw(&rows).is_empty());
}
