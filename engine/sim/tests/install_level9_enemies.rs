//! The ninth level's hook-handed pirate and PegLeg on your own install,
//! against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), waits until the level clock reaches the time the
//! original's run started at, and starts the shared random stream from the
//! run's state, once. Every enemy engagement starts after that time: Claw
//! is put down out of every enemy's reach and walks in. The routes:
//!
//! - beside the hook-handed pirate whose patrol is 2750..3073 on the plank
//!   at 6416: Claw put down on the pillar top at x 3200 walks left onto
//!   the plank at x 3118, then stands (the hold, the swing, the walk in and
//!   the stabs), walks into the pirate while he holds the hook, fires his
//!   pistol every 40 ticks or twice into the hold, swings his sword every
//!   15 ticks, or crouches and stabs every 20 ticks;
//! - beside PegLeg, whose patrol is 1598..1855 on the floor at 6992: Claw
//!   put down on the rock at x 2000 walks left onto that floor at x 1898,
//!   then stands (shots and rolls), ducks (the low shot), fires every 60
//!   ticks (forward rolls), fires once and then swings every 12 ticks,
//!   stabs crouching, or lifts and throws him; and beside the PegLeg whose
//!   patrol is 21504..21761, Claw fires twice (forward rolls to the left).
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched enemy's position and state are compared
//! too, exactly at the 18 ms step, its states lined up by the numbers the
//! run gives them ([`REFERENCE_STATES`]). Divergences that are known, with
//! their causes, are listed in [`KNOWN`]; any other fails the test. The
//! routes are written below; each store's own copy of its route must agree
//! with them. Needs `CLAW_INSTALL`, and `CLAW_REFERENCE` for the
//! comparisons: each test skips itself when what it needs is unset, and
//! fails when `CLAW_REFERENCE` is set but a listed store lacks its route,
//! meta or trace; `CLAW_L9_ONLY` names one store to play alone. Nothing
//! from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::{self, EnemyState, hook_pirate, pegleg};
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;
const LIFT: u32 = 0x20;
const PISTOL: u32 = 0x40;

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

/// Which enemy a route watches: its logic name and its x on the run's
/// first step.
#[derive(Debug, Clone, Copy)]
enum Watch {
    HookPirate(i32),
    PegLeg(i32),
}

impl Watch {
    fn logic(self) -> &'static str {
        match self {
            Watch::HookPirate(_) => hook_pirate::NAMES[0],
            Watch::PegLeg(_) => "PegLeg",
        }
    }

    fn near(self) -> i32 {
        match self {
            Watch::HookPirate(x) | Watch::PegLeg(x) => x,
        }
    }
}

/// A route of the stores: its name, where Claw is put down, how many rows
/// it has, its stretches, its repeated stretch and the enemy it watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    /// The original's game time at the route's time zero at the 18 ms
    /// step, ms: when Claw had settled at his place.
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
    watch: Watch,
}

/// The original's game time at the time zero of the routes Claw walks in
/// on, at the 18 ms step, ms.
const T0_MS: u64 = 828;
/// The clocks the routes were run at.
const CLOCKS: [&str; 2] = ["c18", "c60"];
/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

/// Where the pirate routes put Claw down, and the walk onto the plank.
const HOOK_START: (i32, i32) = (3200, 6200);
const HOOK_WALK: Span = span(20, 39, LEFT);
/// The pirate of the pirate routes.
const HOOK: Watch = Watch::HookPirate(2847);
/// Where the PegLeg routes put Claw down, and the walk onto his floor.
const PEG_START: (i32, i32) = (2000, 6800);
const PEG_WALK: Span = span(20, 44, LEFT);
/// The PegLeg of the PegLeg routes.
const PEG: Watch = Watch::PegLeg(1629);

const fn route(
    store: &'static str,
    start: (i32, i32),
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
    watch: Watch,
) -> Route {
    Route {
        store,
        start,
        t0_ms,
        rows,
        spans,
        every,
        watch,
    }
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

const ROUTES: [Route; 13] = [
    route(
        "hook-engage",
        HOOK_START,
        T0_MS,
        1500,
        &[HOOK_WALK],
        None,
        HOOK,
    ),
    route(
        "hook-touch",
        HOOK_START,
        T0_MS,
        500,
        &[HOOK_WALK, span(60, 199, LEFT)],
        None,
        HOOK,
    ),
    route(
        "hook-pistol",
        HOOK_START,
        T0_MS,
        600,
        &[HOOK_WALK],
        every(200, 2, PISTOL, 40, 521),
        HOOK,
    ),
    route(
        "hook-pistol-hold",
        HOOK_START,
        T0_MS,
        500,
        &[HOOK_WALK, span(70, 71, PISTOL), span(110, 111, PISTOL)],
        None,
        HOOK,
    ),
    route(
        "hook-swing",
        HOOK_START,
        T0_MS,
        800,
        &[HOOK_WALK, span(320, 329, LEFT)],
        every(340, 2, ATTACK, 15, 686),
        HOOK,
    ),
    route(
        "hook-duck",
        HOOK_START,
        T0_MS,
        800,
        &[HOOK_WALK, span(320, 329, LEFT), span(335, 799, DOWN)],
        every(340, 2, ATTACK, 20, 681),
        HOOK,
    ),
    route("peg-engage", PEG_START, T0_MS, 1300, &[PEG_WALK], None, PEG),
    route(
        "peg-duck",
        PEG_START,
        T0_MS,
        900,
        &[PEG_WALK, span(60, 899, DOWN)],
        None,
        PEG,
    ),
    route(
        "peg-pistol",
        PEG_START,
        T0_MS,
        900,
        &[PEG_WALK],
        every(120, 2, PISTOL, 60, 841),
        PEG,
    ),
    route(
        "peg-left",
        (21540, 4900),
        1224,
        520,
        &[span(170, 171, PISTOL), span(330, 331, PISTOL)],
        None,
        Watch::PegLeg(21600),
    ),
    route(
        "peg-swing",
        PEG_START,
        T0_MS,
        600,
        &[PEG_WALK, span(120, 121, PISTOL)],
        every(175, 2, ATTACK, 12, 560),
        PEG,
    ),
    route(
        "peg-duck-stab",
        PEG_START,
        T0_MS,
        600,
        &[
            PEG_WALK,
            span(120, 121, PISTOL),
            span(172, 184, DOWN),
            span(176, 177, ATTACK),
            span(195, 196, ATTACK),
        ],
        every(230, 2, ATTACK, 20, 551),
        PEG,
    ),
    route(
        "peg-lift",
        PEG_START,
        T0_MS,
        900,
        &[
            PEG_WALK,
            span(120, 121, PISTOL),
            span(160, 161, LIFT),
            span(164, 165, LIFT),
            span(168, 169, LIFT),
            span(220, 221, LIFT),
            span(260, 261, LIFT),
        ],
        None,
        PEG,
    ),
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, hook-engage, 18000 ms (rng): Claw has stood still for 17 s and
///   his idle animation reaches the picture on which he says one of his
///   idle lines, which draws from the random stream. The idle line is not
///   built (his idle animation plays, silent). The pirate stabs on in both
///   games.
/// - c18, hook-pistol (4392 ms) and hook-pistol-hold (2052 ms), pistol:
///   Claw's second shot leaves a row early. Pressed again while his first
///   shot's pose still holds, it rewinds the shot's animation, and the
///   original's shot then leaves four rows after the press, the replay's
///   three (a shot from a fresh pose leaves nine rows after its press in
///   both). The third level's rules do not cover it: the one about Claw's
///   shots, that a shot waits one logic call before it moves, delays its
///   flight, not its leaving, and was already in. The shot reaches the
///   pirate a row early, so his flinch or his death and their draws come
///   a row early too (rng, enemy_state, enemy_x, enemy_y).
/// - c18, peg-engage (13014 ms) and peg-pistol (3726 ms), enemy_x: PegLeg
///   walks at 1.08 px and rolls at 4.5 px a tick; where his exact position
///   lands on a whole pixel, the original's movement step leaves it a
///   part of a pixel short (1836.9999999999998 against 1837), so the run
///   lists him one pixel back on that row. Nothing he decides reads it.
/// - c18, peg-swing (5454 ms, enemy_y) and peg-duck-stab (4248 ms,
///   enemy_y; 4266 ms, enemy_x): the killed PegLeg's body flies off on the
///   same arc, but a pixel lower on some rows: the shared flight of a
///   killed enemy starts from his whole-pixel height, the original's from
///   a part of a pixel below it. The body touches nothing on its way.
/// - c18, peg-lift (4140 ms, enemy_y): thrown, PegLeg lands with his
///   carried box reaching into the rock beside him; on the original his
///   first bounce meets the rock on the way up and turns over 7 px up, the
///   replay's shared bounce rises 17 px. His landings, his pause and his
///   firing spell come at other times from there (enemy_x, enemy_state,
///   rng, and Claw's health, state and position once a shot reaches him).
/// - c60 (the default tuning played tick by tick), every route: the
///   original at 60 frames per second makes the enemies' draws once a
///   frame, so their decisions differ from the replay's from their first
///   draws on (at 60 frames per second no run shows a hook hold, and
///   PegLeg rolls before he walks); their blows and shots reach Claw at
///   other times, and his health, state and position follow. On the
///   pistol routes Claw's second shot leaves a row early as at the 18 ms
///   step.
const KNOWN: &[Known] = &[
    ("hook-engage", "c18", &[("rng", 18000)]),
    (
        "hook-pistol",
        "c18",
        &[
            ("pistol", 4392),
            ("rng", 4554),
            ("enemy_state", 4554),
            ("enemy_x", 4572),
            ("enemy_y", 4572),
        ],
    ),
    (
        "hook-pistol-hold",
        "c18",
        &[
            ("pistol", 2052),
            ("rng", 2268),
            ("enemy_state", 2268),
            ("enemy_x", 2286),
        ],
    ),
    ("peg-engage", "c18", &[("enemy_x", 13014)]),
    ("peg-pistol", "c18", &[("enemy_x", 3726)]),
    ("peg-swing", "c18", &[("enemy_y", 5454)]),
    (
        "peg-duck-stab",
        "c18",
        &[("enemy_y", 4248), ("enemy_x", 4266)],
    ),
    (
        "peg-lift",
        "c18",
        &[
            ("enemy_y", 4140),
            ("enemy_x", 4212),
            ("enemy_state", 4284),
            ("rng", 4806),
            ("state_id", 5400),
            ("health", 5400),
            ("x", 5418),
            ("cam_x", 5418),
        ],
    ),
    (
        "hook-touch",
        "c60",
        &[
            ("state_id", 1583),
            ("physics_type", 1583),
            ("health", 1583),
            ("x", 1633),
            ("cam_x", 1633),
            ("y", 3200),
            ("cam_y", 3200),
        ],
    ),
    ("hook-pistol", "c60", &[("pistol", 4417)]),
    ("hook-pistol-hold", "c60", &[("pistol", 2083)]),
    (
        "hook-swing",
        "c60",
        &[
            ("state_id", 6167),
            ("health", 6167),
            ("x", 6250),
            ("cam_x", 6250),
        ],
    ),
    (
        "hook-duck",
        "c60",
        &[
            ("state_id", 6167),
            ("physics_type", 6167),
            ("health", 6167),
            ("x", 6250),
            ("cam_x", 6250),
            ("cam_y", 8483),
        ],
    ),
    (
        "peg-engage",
        "c60",
        &[
            ("state_id", 2333),
            ("health", 3917),
            ("physics_type", 25033),
        ],
    ),
    (
        "peg-duck",
        "c60",
        &[
            ("state_id", 2317),
            ("physics_type", 2333),
            ("cam_y", 3367),
            ("health", 3767),
        ],
    ),
    (
        "peg-pistol",
        "c60",
        &[
            ("state_id", 2333),
            ("health", 4183),
            ("x", 4233),
            ("cam_x", 4233),
            ("pistol", 4483),
        ],
    ),
    (
        "peg-left",
        "c60",
        &[
            ("state_id", 4350),
            ("health", 4350),
            ("x", 4400),
            ("cam_x", 4400),
        ],
    ),
    ("peg-swing", "c60", &[("state_id", 2333), ("health", 3917)]),
    (
        "peg-duck-stab",
        "c60",
        &[("state_id", 2333), ("health", 3917)],
    ),
    ("peg-lift", "c60", &[("state_id", 2333), ("health", 3917)]),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

/// The numbers the reference runs give the enemies' states, where they
/// differ from the states' own numbers.
const REFERENCE_STATES: [(i32, EnemyState); 26] = [
    (2000, EnemyState::Idle),
    (2001, EnemyState::Falling),
    (2003, EnemyState::Strike),
    (2004, EnemyState::Dead),
    (2005, EnemyState::Jump),
    (2008, EnemyState::Bounce),
    (2009, EnemyState::Lifted),
    (2010, EnemyState::Thrown),
    (2011, EnemyState::Pause),
    (2012, EnemyState::Dropped),
    (2013, EnemyState::MoveBack),
    (2014, EnemyState::MoveAhead),
    (2015, EnemyState::BlockHigh),
    (2016, EnemyState::BlockLow),
    (2017, EnemyState::Flailing),
    (2018, EnemyState::Home),
    (2019, EnemyState::JumpAtClaw),
    (2021, EnemyState::Advance),
    (2022, EnemyState::FastAdvance),
    (2023, EnemyState::Pace),
    (2024, EnemyState::HitLow),
    (2025, EnemyState::HitHigh),
    (2027, EnemyState::Fire),
    (2035, EnemyState::Block),
    (2075, EnemyState::HoldHook),
    (2076, EnemyState::RollForward),
];

/// The run's number of the backward roll.
const REFERENCE_ROLL_BACKWARD: i32 = 2077;

/// The state a reference run's number stands for.
fn reference_state(id: i32) -> Option<EnemyState> {
    if id == REFERENCE_ROLL_BACKWARD {
        return Some(EnemyState::RollBackward);
    }
    REFERENCE_STATES
        .iter()
        .find(|(n, _)| *n == id)
        .map(|&(_, s)| s)
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

fn level9() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(9)).unwrap();
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

/// The game at a route's time zero (see the module documentation): on
/// the routes where an enemy engages before that time, the enemy is kept
/// idle at its place until then and put in what the original's runs show
/// at that time ([`align`]).
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
    let early = early_engagement(r.store, clock);
    sim.with_world(|w| w.claw.special.warp_to = Some(r.start));
    let mut id = None;
    let mut knocked = false;
    while sim.tick() * TICK < t0_ms {
        if let (Some(id), Some(e)) = (id, early) {
            park(&mut sim, id, e);
            // Where the shot knocked Claw, once he has landed where he
            // was put down and taken what lies there.
            if !knocked && sim.tick() * TICK >= t0_ms / 2 {
                knocked = true;
                sim.with_world(|w| w.claw.special.warp_to = Some(e.claw));
            }
        }
        sim.step(TickInput::default());
        if early.is_some() && id.is_none() {
            id = Some(find(&sim, r.watch));
        }
    }
    if let (Some(id), Some(e)) = (id, early) {
        align(&mut sim, id, e);
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// What the original's runs show at a route's time zero when its enemy
/// engaged Claw before it (from draws of the original's random stream,
/// which the replay does not have): the PegLeg of `peg-left` shot Claw
/// once (10 health off, knocked 27 px left), rolled back to his bound and
/// walks at Claw again.
#[derive(Debug, Clone, Copy)]
struct EarlyEngagement {
    /// Where Claw stands (where he is put down a second time, after he
    /// landed where the route puts him and took the treasure there).
    claw: (i32, i32),
    /// Claw's health.
    health: i32,
    /// The enemy's exact position across, before the route's first row
    /// (the run lists it after that row's step of 1.08 px).
    x: f64,
    /// Its walk, px/s.
    vx: f64,
    /// When its gun started cooling, and when its last roll started, game
    /// ms.
    fired_ms: u64,
    rolled_ms: u64,
    /// Where its last roll aimed.
    target: i32,
}

fn early_engagement(store: &str, clock: &str) -> Option<EarlyEngagement> {
    if store != "peg-left" {
        return None;
    }
    let (x, fired_ms, rolled_ms) = if clock == "c18" {
        (21745.88 + 1.08, 486, 720)
    } else {
        (21747.02 + 1.08, 450, 667)
    };
    Some(EarlyEngagement {
        claw: (21513, 4947),
        health: 90,
        x,
        vx: -enemy::states::ADVANCE_SPEED,
        fired_ms,
        rolled_ms,
        target: 21847,
    })
}

/// Keeps the enemy idle at its place before time zero.
fn park(sim: &mut Sim, id: ObjectId, e: EarlyEngagement) {
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.x = e.x.trunc() as i32;
        }
        if enemy::data(w, id).is_some() {
            enemy::set_state(w, id, EnemyState::Idle);
        }
        if let Some(d) = enemy::data_mut(w, id) {
            d.busy = false;
            d.vx = 0.0;
            d.deferred = None;
        }
    });
}

/// Puts the enemy and Claw in what the original's runs show at time zero
/// ([`EarlyEngagement`]): the PegLeg walks at Claw, alerted, its gun
/// cooling since its shot. Nothing reaches Claw for the next 139 rows, so
/// what is left of his time safe from enemies after the shot does not
/// matter.
fn align(sim: &mut Sim, id: ObjectId, e: EarlyEngagement) {
    sim.with_world(|w| {
        w.claw.stats.health = e.health;
        let (x, y) = {
            let o = w.objects.get_mut(id).expect("the enemy");
            o.x = e.x.trunc() as i32;
            (o.x, o.y)
        };
        enemy::set_facing(w, id, e.vx > 0.0);
        enemy::set_state(w, id, EnemyState::Advance);
        enemy::set_anim(w, id, pegleg::CLASS.anims.advance);
        if let Some(d) = enemy::data_mut(w, id) {
            d.carry = (e.x.fract(), 0.0);
            d.moved_to = Some((x, y));
            d.busy = false;
            d.vx = e.vx;
            d.deferred = None;
            d.alerted = true;
        }
        if let Some(ObjectData::PegLeg(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            p.fire_cooldown = enemy::Timer::new(e.fired_ms, u64::from(pegleg::FIRE_COOLDOWN_MS));
            p.roll_timer = enemy::Timer::new(e.rolled_ms, u64::from(pegleg::ROLL_MS));
            p.target = e.target;
        }
    });
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

/// The watched enemy at one step of a run or of the replay: position and
/// state (the run's own numbers lined up through [`REFERENCE_STATES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnemyAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
}

/// The watched enemy's steps in a run's `obj-log.csv`: the enemy of its
/// family nearest its x on the run's first step, by step.
fn reference_enemy(path: &Path, watch: Watch) -> Option<Vec<(usize, EnemyAt)>> {
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
    let rows: Vec<Vec<&str>> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|f| f.get(logic) == Some(&watch.logic()))
        .collect();
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let ours = rows
        .iter()
        .filter(|f| f[tick] == "0")
        .min_by_key(|f| (int(f[x]).unwrap_or(0) - watch.near()).abs())?[serial];
    rows.iter()
        .filter(|f| f[serial] == ours)
        .map(|f| {
            Some((
                f[tick].parse().ok()?,
                EnemyAt {
                    x: int(f[x])?,
                    y: int(f[y])?,
                    state: reference_state(int(f[st])?),
                },
            ))
        })
        .collect()
}

/// The first reference time (ms) at which the replay's enemy leaves the
/// run's, by column (enemy_x, enemy_y, enemy_state); a step on which one
/// of them has no enemy counts for all three.
fn compare_enemy(
    played: &[Option<EnemyAt>],
    reference: &[(usize, EnemyAt)],
) -> Vec<(&'static str, u64)> {
    let mut firsts: Vec<(&'static str, u64)> = Vec::new();
    let mut note = |name: &'static str, k: usize| {
        if !firsts.iter().any(|(n, _)| *n == name) {
            firsts.push((name, (k as u64 + 1) * TICK));
        }
    };
    for &(k, want) in reference {
        let Some(got) = played.get(k) else { continue };
        let Some(got) = got else {
            for name in ["enemy_x", "enemy_y", "enemy_state"] {
                note(name, k);
            }
            continue;
        };
        if got.x != want.x {
            note("enemy_x", k);
        }
        if got.y != want.y {
            note("enemy_y", k);
        }
        if want.state.is_none() || got.state != want.state {
            note("enemy_state", k);
        }
    }
    firsts
}

fn find(sim: &Sim, watch: Watch) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == watch.logic())
        .min_by_key(|(_, o)| (o.x - watch.near()).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {} near {}", watch.logic(), watch.near()))
}

/// One play of a route: its trace, the reference trace, and the watched
/// enemy on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    enemy: Vec<Option<EnemyAt>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level9").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
    }
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let id = find(&sim, r.watch);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut enemy = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        enemy.push(sim.objects().get(id).map(|o| EnemyAt {
            x: o.x,
            y: o.y,
            state: EnemyState::from_id(o.state),
        }));
        k += 1;
    }
    Played {
        rows,
        reference,
        enemy,
    }
}

/// Compares a play of `store` at `clock` with the reference trace at the
/// clock's tolerances (exact for c18, feel for c60), and at c18 its enemy
/// with the run's (`enemy`, when the run lists it); returns the
/// divergences not in [`KNOWN`].
fn new_divergences(
    store: &str,
    clock: &str,
    played: &Played,
    enemy: Option<&[(usize, EnemyAt)]>,
) -> Vec<String> {
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
    let mut firsts: Vec<(&str, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| Some((c.column.as_str(), c.first_t_ms?)))
        .collect();
    if let Some(e) = enemy {
        firsts.extend(compare_enemy(&played.enemy, e));
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
        let dir = root.join("level9").join(r.store);
        let rows =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
    }
}

#[test]
fn install_level9_pirate_and_pegleg_routes_follow_the_reference_traces() {
    let Some(level) = level9() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L9_ONLY").ok();
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        let dir = root.join("level9").join(r.store);
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        for clock in CLOCKS {
            println!("== level9/{} {clock}", r.store);
            let played = play(&level, &root, r, clock);
            let enemy = runs
                .as_ref()
                .filter(|_| clock == "c18")
                .and_then(|d| Some(d.join(run_id(&meta, clock)?).join("obj-log.csv")))
                .and_then(|p| reference_enemy(&p, r.watch));
            if clock == "c18" && enemy.is_none() {
                if runs.is_some() {
                    failures.push(format!("{}: the run's object log has no enemy", r.store));
                } else {
                    println!("the runs' object logs are not there: the enemy is not compared");
                }
            }
            failures.extend(new_divergences(r.store, clock, &played, enemy.as_deref()));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw and the watched enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    /// Claw's position, state and health.
    x: i32,
    state: i32,
    health: i32,
    /// The enemy's position, state and health.
    ex: i32,
    es: Option<EnemyState>,
    ehealth: i32,
    /// The straight shots in the level (PegLeg's bullets among them).
    bullets: usize,
    /// Claw's pistol shots in the level.
    pistol_shots: usize,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching its enemy.
fn watch(level: &SimLevel, store: &str, rows: usize) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, r.t0_ms, SEED);
    let id = find(&sim, r.watch);
    let ticks = route_ticks(r);
    (0..rows)
        .map(|k| {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            let c = sim.player().expect("Claw");
            let e = sim.objects().get(id);
            let count = |logic: &str| {
                sim.objects()
                    .all()
                    .filter(|(_, o)| &*o.logic == logic)
                    .count()
            };
            Look {
                x: c.x,
                state: c.state,
                health: sim.claw().stats.health,
                ex: e.map_or(0, |o| o.x),
                es: e.and_then(|o| EnemyState::from_id(o.state)),
                ehealth: e.map_or(0, |o| o.health),
                bullets: count(pegleg::bullet::LOGIC),
                pistol_shots: count("PistolBullet"),
            }
        })
        .collect()
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

/// The rows on which a new straight shot appears.
fn new_bullets(rows: &[Look]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&k| rows[k].bullets > rows[k - 1].bullets)
        .collect()
}

#[test]
fn install_level9_the_pirate_holds_the_hook_then_swings_and_stabs() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-engage", 700);
    // Claw lands on the plank on row 54; the pirate decides on the next
    // row and holds the hook for 112 rows, then goes Home.
    assert_eq!(spans(&rows, EnemyState::HoldHook), [(55, 166)]);
    assert_eq!(rows[167].es, Some(EnemyState::Home));
    // Then the swing at the air, 263 px from Claw: 19 rows, Home on S+19.
    let strikes = spans(&rows, EnemyState::Strike);
    assert_eq!(strikes[0], (168, 186));
    assert_eq!(rows[187].es, Some(EnemyState::Home));
    // He runs, walks and stops 99 px from Claw, then stabs every 56 rows,
    // each stab over on S+21.
    assert_eq!(rows[314].es, Some(EnemyState::Home));
    assert_eq!((rows[314].ex, rows[314].x), (3019, 3118));
    let stabs: Vec<usize> = strikes[1..].iter().map(|s| s.0).collect();
    assert_eq!(stabs[0], 315);
    assert!(stabs.windows(2).all(|p| p[1] - p[0] == 56), "{stabs:?}");
    assert!(strikes[1..].iter().all(|s| s.1 - s.0 == 20), "{strikes:?}");
    // Neither reaches a standing Claw facing him.
    assert!(hits_on_claw(&rows).is_empty());
}

#[test]
fn install_level9_the_held_hook_hurts_on_every_touch_claws_protection_allows() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-touch", 300);
    let hold = spans(&rows, EnemyState::HoldHook)[0];
    assert_eq!(hold, (55, 166));
    // Walking into the hold: 10 on H+57 and again 48 rows later, the
    // length of Claw's protection.
    let in_hold: Vec<usize> = hits_on_claw(&rows)
        .into_iter()
        .filter(|&k| k <= hold.1)
        .collect();
    assert_eq!(in_hold, [112, 160]);
    assert_eq!((rows[112].health, rows[160].health), (90, 80));
    assert!(
        rows[112..=160]
            .iter()
            .all(|l| l.es == Some(EnemyState::HoldHook))
    );
}

#[test]
fn install_level9_the_pirate_parries_every_standing_sword_stab() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-swing", 800);
    let parries = spans(&rows, EnemyState::Block);
    assert_eq!(parries.len(), 12, "{parries:?}");
    assert_eq!((parries[0].0, parries[11].1), (361, 701));
    assert!(parries.iter().all(|p| p.1 - p.0 + 1 == 11), "{parries:?}");
    assert!(rows.iter().all(|l| l.ehealth == 10));
}

#[test]
fn install_level9_crouching_stabs_land_and_the_pirate_strikes_back() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-duck", 800);
    // Each stab costs him 2 and he swings back on the same row, with no
    // flinch drawn and no knockback; the fifth kills him.
    let landed: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].ehealth < rows[k - 1].ehealth)
        .collect();
    assert_eq!(landed, [366, 406, 446, 486, 546]);
    for &k in &landed[..4] {
        assert_eq!(rows[k].es, Some(EnemyState::Strike), "row {k}");
        assert_eq!(rows[k].ex, 3019);
    }
    assert_eq!(starts(&rows, |l| l.es == Some(EnemyState::Dead)), [546]);
    assert!(spans(&rows, EnemyState::HitLow).is_empty());
}

#[test]
fn install_level9_pistol_shots_hurt_the_pirate_outside_his_hold() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-pistol", 300);
    // The first shot flinches him for 19 rows and costs 8; the second
    // kills him (a row early here, see KNOWN).
    assert_eq!(spans(&rows, EnemyState::HitLow)[0], (219, 237));
    assert_eq!(rows[219].ehealth, 2);
    let dead = starts(&rows, |l| l.es == Some(EnemyState::Dead));
    assert_eq!(dead.len(), 1);
    assert!((252..=253).contains(&dead[0]), "{dead:?}");
}

#[test]
fn install_level9_the_held_hook_parries_a_pistol_shot_that_flies_on() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "hook-pistol-hold", 160);
    // The shot during the hold is parried (11 rows) and ends the hold;
    // the shot flies on past him, unhurt.
    assert_eq!(spans(&rows, EnemyState::HoldHook)[0], (55, 94));
    assert_eq!(spans(&rows, EnemyState::Block), [(95, 105)]);
    assert_eq!(rows[105].ehealth, 10);
    assert!(rows[95..=105].iter().all(|l| l.pistol_shots == 1));
    // The next shot, out of the hold, flinches him (a row early here, see
    // KNOWN).
    let hit = spans(&rows, EnemyState::HitLow)[0];
    assert!((125..=126).contains(&hit.0), "{hit:?}");
    assert_eq!(hit.1 - hit.0 + 1, 19);
}

#[test]
fn install_level9_pegleg_shoots_rolls_and_shoots_again() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-engage", 400);
    // He rolls back 150 px in 34 rows, fires on S+19 (the bullet leaves)
    // and leaves the shot on S+31.
    assert_eq!(spans(&rows, EnemyState::RollBackward)[0], (173, 206));
    let fire = spans(&rows, EnemyState::Fire)[0];
    assert_eq!(fire, (207, 237));
    assert!(new_bullets(&rows).contains(&(fire.0 + 19)));
    // The forward roll from Home towards Claw on his right starts with a
    // step of 4 px.
    let roll = spans(&rows, EnemyState::RollForward)[0];
    assert_eq!(roll.0, 239);
    assert_eq!(rows[239].ex - rows[238].ex, 4);
    // His bullet costs a standing Claw 10.
    assert_eq!(rows[hits_on_claw(&rows)[0]].health, 90);
}

#[test]
fn install_level9_pegleg_shoots_low_at_a_ducking_claw() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-duck", 300);
    let fire = spans(&rows, EnemyState::Fire)[0];
    assert_eq!(fire.0, 206);
    assert_eq!(hits_on_claw(&rows)[0], 250);
    assert_eq!(rows[250].health, 90);
}

#[test]
fn install_level9_a_pistol_shot_rolls_pegleg_at_claw() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-pistol", 200);
    // Rolling, he stops on his patrol bound 43 px short of Claw and fires.
    assert_eq!(spans(&rows, EnemyState::RollForward)[0], (138, 170));
    assert_eq!(rows[170].ex, 1855);
    assert_eq!(rows[171].es, Some(EnemyState::Fire));
    assert!(rows.iter().all(|l| l.ehealth == 7));
}

#[test]
fn install_level9_pegleg_rolls_stop_on_his_bound() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-left", 400);
    // Rolling back from Claw on his left, he stops exactly on his right
    // bound, 21761.
    let back = spans(&rows, EnemyState::RollBackward);
    assert_eq!(back[0], (78, 99));
    assert_eq!(rows[99].ex, 21761);
    // The shot on row 170 rolls him forward from Home on that bound: the
    // entry row moves him right, and the bound holds him.
    let fwd = spans(&rows, EnemyState::RollForward)[0];
    assert_eq!(fwd.0, 132);
    assert_eq!(rows[132].ex, rows[131].ex);
    assert!(rows[133].ex < rows[132].ex);
}

#[test]
fn install_level9_pegleg_flinches_from_a_stab_and_dies_from_a_later_one() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-swing", 400);
    assert_eq!(spans(&rows, EnemyState::HitHigh)[0], (180, 198));
    assert_eq!(rows[180].ehealth, 2);
    assert_eq!(starts(&rows, |l| l.es == Some(EnemyState::Dead)), [301]);
}

#[test]
fn install_level9_a_crouching_stab_opens_pegleg_up() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-duck-stab", 300);
    // The crouching stab flinches him low for 17 rows; the swing on row
    // 195 meets him flinching and only sends him Home; inside the window
    // the next swing always lands: 2 + 5 = 7.
    assert_eq!(spans(&rows, EnemyState::HitLow)[0], (182, 198));
    assert_eq!(rows[199].es, Some(EnemyState::Home));
    assert_eq!(starts(&rows, |l| l.es == Some(EnemyState::Dead)), [234]);
}

#[test]
fn install_level9_pegleg_is_lifted_mid_roll_and_thrown_unhurt() {
    let Some(level) = level9() else { return };
    let rows = watch(&level, "peg-lift", 700);
    // The presses on rows 160 and 164: the second lifts him out of his
    // forward roll.
    assert_eq!(spans(&rows, EnemyState::RollForward)[0], (138, 163));
    assert_eq!(starts(&rows, |l| l.es == Some(EnemyState::Lifted)), [164]);
    assert_eq!(spans(&rows, EnemyState::Thrown)[0], (222, 228));
    // After the landings he pauses 28 rows, gets up, taunts and fires six
    // shots 31 rows apart, never losing health.
    let pause = spans(&rows, EnemyState::Pause)[0];
    assert_eq!(pause.1 - pause.0 + 1, 28);
    let fire = spans(&rows, EnemyState::Fire)[0];
    let shots: Vec<usize> = new_bullets(&rows)
        .into_iter()
        .filter(|&k| k > fire.0 && k <= fire.1)
        .collect();
    assert_eq!(shots.len(), 6, "{shots:?}");
    assert!(shots.windows(2).all(|p| p[1] - p[0] == 31), "{shots:?}");
    assert_eq!(shots[0], fire.0 + 19);
    assert!(rows.iter().all(|l| l.ehealth == 7));
}
