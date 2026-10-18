//! The seventh level's red-tail pirates and bear sailors on your own
//! install, against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does: the enemies around him wake and settle then), waits until
//! the level clock reaches the time the original's run started at, and
//! starts the shared random stream from the run's state, once; on the
//! bear routes it also puts the bear in the pause the original's runs
//! show it in at that time (see [`align`]). It is then
//! played tick by tick and compared with the reference trace of the same
//! route: at the 18 ms step with the original's speeds every column
//! exactly (Claw's position and state, health, score, the camera, the
//! random stream and the events), at 60 frames per second with the default
//! tuning within the feel tolerances. The routes:
//!
//! - beside the red-tail pirate whose patrol is 1865..2117 on the floor at
//!   7320: Claw put down at x 1700, out of its reach, walks right to 1820
//!   and then stands (its engage, strikes and cadence), crouches (its low
//!   strike), swings his sword every 15 ticks (its hops back, its flinch
//!   and its death) or fires his pistol every 30 ticks;
//! - beside the bear sailor whose patrol is 3586..3712 on the floor at
//!   7448: Claw put down at x 3780 stands (its stop 99 px away, its blows
//!   and pauses), walks left to 3724 and stands (full hugs), mashes Up and
//!   Down on every tick (out on the tenth tick of the hug), Up on every
//!   other tick (out on the nineteenth), or presses nine times (held to
//!   the end); and put down 6 px from the bear (its touch, then the grab).
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them. Needs `CLAW_INSTALL`, and
//! `CLAW_REFERENCE` for the comparisons: each test skips itself when what
//! it needs is unset, and fails when `CLAW_REFERENCE` is set but a listed
//! store lacks its route, meta or trace; `CLAW_L7_ONLY` names one store
//! to play alone. Nothing from an install or a reference store is kept in
//! the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::{self, EnemyState, bear_sailor, redtail_pirate};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;
const PISTOL: u32 = 0x40;

/// One stretch of a route: from `from` to `to` (inclusive) `held` is down,
/// pressed on the stretch's first row when `press` is set.
#[derive(Debug, Clone, Copy)]
struct Span {
    from: usize,
    to: usize,
    held: u32,
    press: bool,
}

const fn span(from: usize, to: usize, held: u32, press: bool) -> Span {
    Span {
        from,
        to,
        held,
        press,
    }
}

/// A stretch repeated: its first row, its length, the keys (alternating
/// between those listed), the distance between two repeats and the first
/// row no repeat starts on.
#[derive(Debug, Clone, Copy)]
struct Every {
    first: usize,
    len: usize,
    keys: &'static [u32],
    gap: usize,
    until: usize,
}

/// A route of the stores: its name, where Claw is put down, the clocks it
/// was run at, how many rows it has, its stretches and its repeated
/// stretch.
struct Route {
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    /// The original's game time at the route's time zero at the 18 ms
    /// step, ms: when Claw had settled at his place.
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
}

const C18: &[&str] = &["c18"];
const BOTH: &[&str] = &["c18", "c60"];

/// Where the pirate routes put Claw down, and the walk to x 1820.
const PIRATE_START: (i32, i32) = (1700, 7150);
const WALK_RIGHT: Span = span(20, 49, RIGHT, true);
/// Where the bear routes put Claw down, and the walk to x 3724.
const BEAR_START: (i32, i32) = (3780, 7300);
const WALK_LEFT: Span = span(20, 33, LEFT, true);
/// Claw put down 6 px right of the bear.
const BEAR_CLOSE_START: (i32, i32) = (3655, 7300);

/// The pirate of the routes: its place and its patrol.
const PIRATE_AT: i32 = 1924;
/// The bear of the routes.
const BEAR_AT: i32 = 3681;

const fn route(
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    t0_ms: u64,
    rows: usize,
    spans: &'static [Span],
    every: Option<Every>,
) -> Route {
    Route {
        store,
        start,
        clocks,
        t0_ms,
        rows,
        spans,
        every,
    }
}

const ROUTES: [Route; 10] = [
    route(
        "rtp-engage",
        PIRATE_START,
        BOTH,
        828,
        1000,
        &[WALK_RIGHT],
        None,
    ),
    route(
        "rtp-duck",
        PIRATE_START,
        BOTH,
        828,
        1000,
        &[WALK_RIGHT, span(52, 999, DOWN, true)],
        None,
    ),
    route(
        "rtp-swing",
        PIRATE_START,
        BOTH,
        828,
        900,
        &[WALK_RIGHT],
        Some(Every {
            first: 60,
            len: 2,
            keys: &[ATTACK],
            gap: 15,
            until: 857,
        }),
    ),
    route(
        "rtp-pistol",
        PIRATE_START,
        C18,
        828,
        900,
        &[WALK_RIGHT],
        Some(Every {
            first: 60,
            len: 2,
            keys: &[PISTOL],
            gap: 30,
            until: 842,
        }),
    ),
    route("bear-engage", BEAR_START, BOTH, 828, 1000, &[], None),
    route("bear-close", BEAR_CLOSE_START, C18, 990, 500, &[], None),
    route("bear-hug", BEAR_START, BOTH, 828, 700, &[WALK_LEFT], None),
    route(
        "bear-mash1",
        BEAR_START,
        C18,
        828,
        700,
        &[WALK_LEFT],
        Some(Every {
            first: 34,
            len: 1,
            keys: &[UP],
            gap: 2,
            until: 399,
        }),
    ),
    route(
        "bear-mash2",
        BEAR_START,
        BOTH,
        828,
        700,
        &[WALK_LEFT],
        Some(Every {
            first: 34,
            len: 1,
            keys: &[UP, DOWN],
            gap: 1,
            until: 400,
        }),
    ),
    route(
        "bear-nine",
        BEAR_START,
        C18,
        828,
        700,
        &[WALK_LEFT],
        Some(Every {
            first: 58,
            len: 1,
            keys: &[UP],
            gap: 2,
            until: 75,
        }),
    ),
];

fn route_named(store: &str) -> &'static Route {
    ROUTES
        .iter()
        .find(|r| r.store == store)
        .unwrap_or_else(|| panic!("no route {store}"))
}

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, rtp-pistol, 1422 ms (rng): Claw's pistol shot flies from its
///   first logic call in the replay, from its second in the original
///   (the rule comes with the third level's route work, merged later), so
///   it meets the pirate as it lands from its hop, a row before the
///   original's: the hit's draws come a row early. The second shot
///   leaves a row early too (1692 ms, pistol): pressed again while the
///   first shot's pose still holds, it rewinds the shot's animation, and
///   the original waits one row more before it leaves.
/// - c18, bear-engage, 5832 ms (cam_x, cam_y): after the bear's blows
///   knock Claw into the water, he comes back at the checkpoint; the
///   original runs the two update passes of the respawn with no frame
///   time, so its camera does not take the step the replay's takes (18
///   px apart; the rule comes with the third level's route work). The
///   camera's place changes what is on screen, the stream follows two
///   rows later (5868 ms, rng), and the bear's next blows come at other
///   times (7308 ms).
/// - c60 (the default tuning played tick by tick), the pirate routes: the
///   original at 60 frames per second makes the pirate's draws once a
///   frame, so from its first steps in and back its moves and strikes
///   differ from the replay's; its blows reach Claw at other times
///   (rtp-engage 6083 ms, rtp-duck 3417 ms). Crouching, the camera pans
///   down 4 px a step, 60 steps a second in the original and 55.6 in the
///   replay, and is more than 8 px behind by 2700 ms (rtp-duck, cam_y). The pirate killed by the swings drops its coin with other
///   random speeds, and Claw picks it up at another time (rtp-swing, 6433
///   ms, score).
/// - c60, the bear routes, from the first frame: before time zero the
///   original's bear, its pause drawn from the original's stream at 60
///   frames per second, came forward and its blow knocked Claw 27 px
///   (health 90 at time zero); the replay's bear is put in the pause the
///   18 ms runs show. Its later grabs, pauses and blows follow from there;
///   a full hug also takes 30 health at 60 frames per second against 28
///   at the 18 ms step (a squeeze every 100 ms over 3000 ms).
const KNOWN: &[Known] = &[
    ("rtp-pistol", "c18", &[("rng", 1422), ("pistol", 1692)]),
    (
        "bear-engage",
        "c18",
        &[
            ("cam_x", 5832),
            ("cam_y", 5832),
            ("rng", 5868),
            ("state_id", 7308),
            ("health", 7308),
            ("x", 7326),
        ],
    ),
    (
        "rtp-engage",
        "c60",
        &[
            ("state_id", 6083),
            ("health", 6617),
            ("x", 6700),
            ("cam_x", 6700),
        ],
    ),
    (
        "rtp-duck",
        "c60",
        &[
            ("cam_y", 2700),
            ("state_id", 3417),
            ("physics_type", 3417),
            ("health", 3417),
            ("x", 3500),
            ("cam_x", 3500),
        ],
    ),
    ("rtp-swing", "c60", &[("score", 6433)]),
    (
        "bear-engage",
        "c60",
        &[
            ("x", 16),
            ("cam_x", 16),
            ("health", 16),
            ("state_id", 983),
            ("physics_type", 1333),
            ("y", 1650),
            ("cam_y", 1650),
            ("lives", 5083),
            ("events", 5083),
        ],
    ),
    (
        "bear-hug",
        "c60",
        &[
            ("x", 16),
            ("cam_x", 16),
            ("health", 16),
            ("state_id", 700),
            ("physics_type", 700),
            ("y", 13216),
            ("lives", 14083),
            ("events", 14083),
        ],
    ),
    (
        "bear-mash2",
        "c60",
        &[
            ("x", 16),
            ("cam_x", 16),
            ("health", 16),
            ("state_id", 700),
            ("physics_type", 700),
        ],
    ),
];

fn known(store: &str, clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(s, c, _)| *s == store && *c == clock)
        .map_or(&[], |(_, _, k)| k)
}

fn route_ticks(r: &Route) -> Vec<TickInput> {
    let mut held = vec![0u32; r.rows];
    let mut pressed = vec![0u32; r.rows];
    let mut spans: Vec<Span> = r.spans.to_vec();
    if let Some(e) = r.every {
        let mut from = e.first;
        let mut i = 0;
        while from < e.until.min(r.rows) {
            spans.push(span(
                from,
                (from + e.len - 1).min(r.rows - 1),
                e.keys[i % e.keys.len()],
                true,
            ));
            from += e.gap;
            i += 1;
        }
    }
    for s in &spans {
        for h in &mut held[s.from..=s.to] {
            *h |= s.held;
        }
        if s.press {
            pressed[s.from] |= s.held;
        }
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

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;

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
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some(start));
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// Rows the bear of the bear routes still pauses for at time zero in the
/// original's runs: the pause it is in was drawn before time zero.
const BEAR_PAUSE_ROWS: u64 = 11;

/// Puts the bear of a bear route in the pause the original's runs show at
/// time zero, ending on row [`BEAR_PAUSE_ROWS`] (back Home then). Its
/// length was drawn before time zero, from the original's random stream,
/// which the replay does not have; the stream is the original's from time
/// zero on.
fn align(sim: &mut Sim, r: &Route) {
    if !r.store.starts_with("bear-") {
        return;
    }
    let id = find(sim, "BearSailor", BEAR_AT);
    sim.with_world(|w| {
        let now = w.now_ms;
        if let Some(d) = enemy::data_mut(w, id) {
            d.pause = enemy::Timer::new(now, BEAR_PAUSE_ROWS * TICK);
            d.prev = EnemyState::Home;
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.state = EnemyState::Pause as i32;
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

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs; returns its rows and that trace. A store
/// without the trace or its meta fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> (Vec<TraceRow>, Vec<TraceRow>) {
    let dir = root.join("level7").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, r.t0_ms, "{}: time zero", r.store);
    }
    let mut sim = at_time_zero(level, clock, r.start, t0, seed);
    align(&mut sim, r);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        k += 1;
    }
    (rows, reference)
}

/// Compares `rows` with `reference` at the clock's tolerances (exact for
/// c18, feel for c60) and returns the divergences not in [`KNOWN`].
fn new_divergences(
    store: &str,
    clock: &str,
    rows: &[TraceRow],
    reference: &[TraceRow],
) -> Vec<String> {
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
    let list = known(store, clock);
    for c in &cmp.per_column {
        let Some(first) = c.first_t_ms else { continue };
        println!("first {store} {clock} {} {first}", c.column);
        match list.iter().find(|(name, _)| *name == c.column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{store} {clock}: {} at {first} ms", c.column)),
        }
    }
    out
}

#[test]
fn the_routes_match_the_stores_copies() {
    let Some(root) = reference_root() else { return };
    for r in &ROUTES {
        let dir = root.join("level7").join(r.store);
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
fn install_level7_pirate_and_bear_routes_follow_the_reference_traces() {
    let Some(level) = level7() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L7_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level7/{} {clock}", r.store);
            let (rows, reference) = play(&level, &root, r, clock);
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw and the watched enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    /// Claw's position, state, physics type, health and picture.
    x: i32,
    y: i32,
    state: i32,
    physics: i32,
    health: i32,
    frame: i32,
    /// The enemy's position, state, health, picture and attack box.
    ex: i32,
    ey: i32,
    es: Option<EnemyState>,
    ehealth: i32,
    eframe: i32,
    eattack: Rect,
    /// The pirate's hop chance.
    chance: i32,
}

/// Plays `rows` rows of route `store` at the 18 ms step from its time
/// zero, watching the enemy running `logic` nearest to x `near`.
fn watch(level: &SimLevel, store: &str, logic: &str, near: i32, rows: usize) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r.start, r.t0_ms, SEED);
    align(&mut sim, r);
    let id = find(&sim, logic, near);
    let ticks = route_ticks(r);
    (0..rows)
        .map(|k| {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            let c = sim.player().expect("Claw");
            let e = sim.objects().get(id);
            let chance = match e.map(|o| &o.data) {
                Some(ObjectData::RedTailPirate(p)) => p.jump_back_chance,
                _ => -1,
            };
            Look {
                x: c.x,
                y: c.y,
                state: c.state,
                physics: c.physics_type,
                health: sim.claw().stats.health,
                frame: c.frame,
                ex: e.map_or(0, |o| o.x),
                ey: e.map_or(0, |o| o.y),
                es: e.and_then(|o| EnemyState::from_id(o.state)),
                ehealth: e.map_or(0, |o| o.health),
                eframe: e.map_or(0, |o| o.frame),
                eattack: e.map_or(Rect::UNSET, |o| o.attack_rect),
                chance,
            }
        })
        .collect()
}

fn find(sim: &Sim, logic: &str, near: i32) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .min_by_key(|(_, o)| (o.x - near).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} near {near}"))
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// The rows on which the enemy enters state `s`.
fn entries(rows: &[Look], s: EnemyState) -> Vec<usize> {
    starts(rows, |l| l.es == Some(s))
}

/// The spans (first, last) of the rows on which `f` holds.
fn spans(rows: &[Look], f: impl Fn(&Look) -> bool + Copy) -> Vec<(usize, usize)> {
    starts(rows, f)
        .into_iter()
        .map(|a| {
            let b = (a..rows.len()).take_while(|&k| f(&rows[k])).last().unwrap();
            (a, b)
        })
        .collect()
}

/// The rows on which a strike that started on row `s` had its box live
/// (the attack box is the strike's).
fn live_rows(rows: &[Look], s: usize, strike: Rect) -> Vec<usize> {
    (s..rows.len())
        .take_while(|&k| rows[k].es == Some(EnemyState::Strike))
        .filter(|&k| rows[k].eattack == strike || rows[k].eattack == strike.mirrored())
        .map(|k| k - s)
        .collect()
}

/// How long each strike that starts on one of `starts` lasts, in rows,
/// until the enemy is Home again.
fn strike_lengths(rows: &[Look], starts: &[usize]) -> Vec<usize> {
    starts
        .iter()
        .map(|&s| {
            (s..rows.len())
                .take_while(|&k| rows[k].es == Some(EnemyState::Strike))
                .count()
        })
        .collect()
}

#[test]
fn install_level7_the_pirate_engages_at_once_and_strikes_on_its_cadence() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "rtp-engage", "RedTailPirate", PIRATE_AT, 400);
    // Idle until Claw walks into its boxes; its first Home step (36)
    // decides at once: a high strike at the air, Claw 100 to 200 px away.
    assert_eq!(entries(&rows, EnemyState::Home)[0], 36);
    assert!(rows[..36].iter().all(|l| l.es == Some(EnemyState::Idle)));
    let strikes = entries(&rows, EnemyState::Strike);
    assert_eq!(strikes, [37, 150, 169, 281, 300, 319, 338, 357]);
    // Live on its 7th row, Home on its 18th.
    for &s in &strikes {
        assert_eq!(
            live_rows(&rows, s, redtail_pirate::HIGH_STRIKE),
            [7],
            "strike {s}"
        );
    }
    assert!(strike_lengths(&rows, &strikes).iter().all(|&n| n == 18));
    // Close in, every 19 rows; after the strike from further out (37), the
    // next comes 113 rows later, its 2000 ms rest over.
    assert_eq!(strikes[1] - strikes[0], 113);
    assert!(strikes[3..].windows(2).all(|w| w[1] - w[0] == 19));
    // Each landed strike costs 10 and knocks Claw 27 px back.
    let hits = starts(&rows, |l| l.state == state::HIT);
    assert_eq!(hits, [157, 288, 345]);
    assert_eq!(rows[159].health, 90);
    assert_eq!(rows[156].x - rows[180].x, 27);
    // It walks at 120 px/s (2.16 px a row), steps in and back at 150.
    let walk = spans(&rows, |l| l.es == Some(EnemyState::Advance));
    let (a, b) = walk[1];
    assert_eq!((a, b), (118, 148));
    assert_eq!(rows[a].ex - rows[b].ex, 65);
    let back = spans(&rows, |l| l.es == Some(EnemyState::MoveBack))[0];
    assert_eq!(back, (76, 116));
    assert_eq!(rows[back.1].ex - rows[back.0].ex, 108);
    // Knocked out of its widened bounds, Claw is out of range: it paces.
    assert_eq!(entries(&rows, EnemyState::Pace), [376]);
}

#[test]
fn install_level7_the_pirate_strikes_low_at_a_crouching_claw() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "rtp-duck", "RedTailPirate", PIRATE_AT, 400);
    let strikes = entries(&rows, EnemyState::Strike);
    assert_eq!(strikes, [37, 150, 165, 279, 295, 314, 329, 346, 365]);
    // At a crouching Claw: live on the 7th row with the low box, Home on
    // the 14th; a strike started while Claw reels from a hit swings high.
    let low = [150, 279, 314, 329, 365];
    for &s in &low {
        assert_eq!(live_rows(&rows, s, redtail_pirate::LOW_STRIKE), [7], "{s}");
    }
    assert_eq!(strike_lengths(&rows, &low), [14; 5]);
    assert_eq!(strike_lengths(&rows, &[165, 295, 346]), [18; 3]);
    // Close in, every 15 rows.
    assert_eq!(strikes[6] - strikes[5], 15);
    assert_eq!(starts(&rows, |l| l.state == state::HIT), [157, 286, 336]);
}

#[test]
fn install_level7_the_pirate_hops_back_three_times_then_takes_the_hits() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "rtp-swing", "RedTailPirate", PIRATE_AT, 340);
    // A hop on the row each swing starts (60, then the swings that find it
    // in range), 18 rows, landing 96 or 97 px from where its first row
    // took it.
    let hops = spans(&rows, |l| l.es == Some(EnemyState::JumpBack));
    assert_eq!(hops, [(60, 77), (124, 141), (215, 232)]);
    for &(a, b) in &hops {
        let dx = rows[b + 1].ex - rows[a].ex;
        assert!((96..=97).contains(&dx), "hop {a}: {dx}");
    }
    // The chance: 90, then 60, 30, 0 after each hop.
    assert_eq!(rows[59].chance, 90);
    assert_eq!(
        [rows[60].chance, rows[124].chance, rows[215].chance],
        [60, 30, 0]
    );
    // It never parries here: the fourth swing in range flinches it (19
    // rows), the next kills it.
    assert_eq!(
        spans(&rows, |l| l.es == Some(EnemyState::HitLow)),
        [(186, 204)]
    );
    assert_eq!(rows[186].ehealth, 5);
    assert_eq!(entries(&rows, EnemyState::Dead), [276]);
    assert!(rows.iter().all(|l| l.es != Some(EnemyState::Block)));
}

#[test]
fn install_level7_the_pirate_hops_from_a_shot_too() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "rtp-pistol", "RedTailPirate", PIRATE_AT, 200);
    // The shot fired on row 60 starts the hop on that row; it meets the
    // pirate as it lands and the next shot kills it (8 a shot, 10 health).
    assert_eq!(entries(&rows, EnemyState::JumpBack), [60]);
    assert_eq!(rows[60].chance, 60);
    let flinch = entries(&rows, EnemyState::HitLow)[0];
    assert!((78..=79).contains(&flinch), "{flinch}");
    assert_eq!(rows[flinch].ehealth, 2);
    let dead = entries(&rows, EnemyState::Dead)[0];
    assert!((102..=103).contains(&dead), "{dead}");
}

#[test]
fn install_level7_the_bear_stops_99_px_away_and_strikes_or_pauses() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "bear-engage", "BearSailor", BEAR_AT, 1000);
    // 99 px from a standing Claw it does not come closer: pauses and
    // blows.
    assert_eq!((rows[35].x - rows[35].ex), 99);
    let strikes = entries(&rows, EnemyState::Strike);
    assert_eq!(strikes[..3], [36, 86, 389]);
    // Live on its 16th row (Claw hit on that row), Home on its 35th.
    for &s in &strikes[..2] {
        assert_eq!(live_rows(&rows, s, bear_sailor::STRIKE), [16], "{s}");
        assert_eq!(rows[s + 16].state, state::HIT, "{s}");
    }
    assert!(
        strike_lengths(&rows, &strikes[..3])
            .iter()
            .all(|&n| n == 35)
    );
    // Pauses of 20 to 33 rows between its blows.
    let pauses = spans(&rows, |l| l.es == Some(EnemyState::Pause));
    for &(a, b) in &pauses[1..pauses.len() - 1] {
        assert!((20..=33).contains(&(b - a + 1)), "pause {a}..{b}");
    }
    // Two blows knock Claw over the edge into the water; he comes back at
    // the checkpoint.
    assert_eq!(entries(&rows, EnemyState::Advance)[0], 72);
    assert_eq!(starts(&rows, |l| l.state == state::DEATH)[0], 154);
    assert_eq!(starts(&rows, |l| l.health == 100)[1], 323);
    assert!(rows.iter().all(|l| l.state != state::BEARHUG));
}

#[test]
fn install_level7_a_full_hug_squeezes_28_and_lets_go_after_167_rows() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "bear-hug", "BearSailor", BEAR_AT, 812);
    // The grab on row 36 (E), Claw 43 px from the bear; the hug from
    // E + 19, Claw put 30 px in front of the bear at its height.
    let e = entries(&rows, EnemyState::StartHug);
    assert_eq!(e, [36, 254, 469, 681]);
    assert_eq!((rows[35].x, rows[35].ex), (3724, BEAR_AT));
    assert_eq!(rows[36].state, state::BEARHUG);
    assert_eq!(entries(&rows, EnemyState::Hug), [55, 273, 488, 700]);
    assert_eq!((rows[55].x, rows[55].y), (BEAR_AT + 30, 7386));
    assert_eq!(rows[54].x, 3724, "held where grabbed until then");
    // A squeeze on j = 1, 7, .. 163: 28 health; let go on j = 167.
    let squeezes: Vec<usize> = (55..222)
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .map(|k| k - 55)
        .collect();
    let want: Vec<usize> = (0..28).map(|n| 1 + 6 * n).collect();
    assert_eq!(squeezes, want);
    assert_eq!(rows[221].health, 72);
    // Released on E + 186: standing at the bear's height, falling for two
    // rows, standing again on the floor 1 px lower.
    assert_eq!(rows[221].state, state::BEARHUG);
    let after: Vec<(i32, i32, i32)> = rows[222..226]
        .iter()
        .map(|l| (l.state, l.physics, l.y))
        .collect();
    assert_eq!(
        after,
        [
            (state::STAND, 1, 7386),
            (state::FALL, 4, 7386),
            (state::FALL, 4, 7386),
            (state::STAND, 1, 7387)
        ]
    );
    // Claw's pictures from j = 1: 452 x 8, 451 x 7, 450 x 9, 451 x 7.
    let frames: Vec<i32> = rows[56..87].iter().map(|l| l.frame).collect();
    let mut want = vec![452; 8];
    want.extend([451; 7]);
    want.extend([450; 9]);
    want.extend([451; 7]);
    assert_eq!(frames, want);
    // The fourth hug empties his health on j = 91: he dies on that row.
    assert_eq!(rows[790].health, 1);
    assert_eq!((rows[791].health, rows[791].state), (0, state::DEATH_FALL));
    assert_eq!(rows[791].es, Some(EnemyState::Home));
}

#[test]
fn install_level7_mashing_frees_claw_on_the_tenth_tick_with_a_press() {
    let Some(level) = level7() else { return };
    // A press on every row: out on j = 10 after 2 squeezes; the grab's
    // presses do not count.
    let rows = watch(&level, "bear-mash2", "BearSailor", BEAR_AT, 200);
    assert_eq!(
        spans(&rows, |l| l.state == state::BEARHUG)[..2],
        [(36, 64), (88, 116)]
    );
    assert_eq!(rows[64].health, 98);
    // A press every other row: out on j = 19 after 3 squeezes.
    let rows = watch(&level, "bear-mash1", "BearSailor", BEAR_AT, 200);
    assert_eq!(
        spans(&rows, |l| l.state == state::BEARHUG)[..2],
        [(36, 73), (104, 141)]
    );
    assert_eq!(rows[73].health, 97);
    // Nine presses never free him: held to the end of the hug.
    let rows = watch(&level, "bear-nine", "BearSailor", BEAR_AT, 300);
    assert_eq!(spans(&rows, |l| l.state == state::BEARHUG)[0], (36, 221));
    assert_eq!(rows[221].health, 72);
}

#[test]
fn install_level7_the_bear_grabs_a_claw_it_knocked_back() {
    let Some(level) = level7() else { return };
    let rows = watch(&level, "bear-close", "BearSailor", BEAR_AT, 300);
    // Put down 6 px from the bear, Claw is hit by its touch (before time
    // zero and again on row 3), knocked 27 px, then grabbed on row 12, 33
    // px from it, and held 30 px from it.
    assert_eq!(rows[0].health, 90);
    assert_eq!(starts(&rows, |l| l.state == state::HIT)[0], 3);
    assert_eq!(entries(&rows, EnemyState::StartHug)[0], 12);
    assert_eq!(rows[12].x - rows[12].ex, 33);
    assert_eq!(rows[31].x - rows[31].ex, 30);
    // Right after the hug its blow, live on its 16th row, reaches him.
    assert_eq!(entries(&rows, EnemyState::Strike)[0], 199);
    assert_eq!(rows[215].state, state::HIT);
}
