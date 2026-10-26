//! The fifth level's breaking planks and statues on your own install,
//! against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), lets him settle, waits until the level clock reaches the
//! time the original's run started at, and starts the shared random
//! stream from the run's state. It is then played tick by tick and
//! compared with the reference trace of the same route: at the 18 ms step
//! with the original's speeds every column exactly (Claw's position and
//! state, health, score, the camera, the random stream and the events), at
//! 60 frames per second with the default tuning within the feel
//! tolerances. The routes:
//!
//! - on the 500 ms row at (18727, 4633): Claw standing at either edge of
//!   a piece's trigger window (x - 23 and x + 18 start it, x - 24 and
//!   x + 19 do not) and in the 22 px between two windows; walking from
//!   that gap into the next window, which crumbles under him, so that he
//!   falls through the three stacked rows; walking onto the row from the
//!   checkpoint, falling through, ending a life and walking onto the
//!   restored row again;
//! - on the 750 ms row at (26215, 6104): the same walk, falling into the
//!   death tiles below;
//! - beside the first pair of statues: one swing at each.
//!
//! The planks' and the statues' own schedules are checked against the
//! original's on the same routes: the trigger tick, the wait, the crumble's
//! second picture, the floor going and the hiding; the row restored on
//! the tick after the lives step; the break's pictures and states, the
//! fan of treasure and the draw order.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; when a store
//! holds its own copy of a route, the two must agree. Needs `CLAW_INSTALL`,
//! and `CLAW_REFERENCE` for the comparisons; each test skips itself when
//! what it needs is unset. Nothing from an install or a reference store is
//! kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::break_plank::{self, state as plank_state};
use claw_sim::logic::statue::{self, state as statue_state};
use claw_sim::logic::{ObjectData, pickup};
use claw_sim::object::{ObjectId, draw_flags};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

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

/// How long the in-game menu is up before its End Life ends Claw's
/// life, ms, in the stores' routes (`menu ingame 500` and then
/// `command end_life` at the same row): its frames move the frame pulse
/// ([`Sim::pass_ingame_menu`], [`Sim::end_life`]).
const END_LIFE_MENU_MS: u32 = 500;

/// A route of the stores: its name, where Claw is put down, the clocks it
/// was run at, how many rows it has, its stretches (rows not covered hold
/// nothing) and the row before which Claw ends a life from the in-game
/// menu, if any.
struct Route {
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    rows: usize,
    spans: &'static [Span],
    end_life: Option<usize>,
}

const fn route(
    store: &'static str,
    start: (i32, i32),
    clocks: &'static [&'static str],
    rows: usize,
    spans: &'static [Span],
) -> Route {
    Route {
        store,
        start,
        clocks,
        rows,
        spans,
        end_life: None,
    }
}

const C18: &[&str] = &["c18"];
const BOTH: &[&str] = &["c18", "c60"];

/// Right held for four rows from row 15: 16 px, from a gap into the next
/// window.
const STEP_RIGHT: [Span; 1] = [span(15, 18, RIGHT, true)];

const ROUTES: [Route; 11] = [
    route("plank-gap", (18820, 4450), C18, 95, &[]),
    route("plank-edge-18809", (18809, 4450), C18, 80, &[]),
    route("plank-edge-18810", (18810, 4450), C18, 80, &[]),
    route("plank-edge-18831", (18831, 4450), C18, 80, &[]),
    route("plank-edge-18832", (18832, 4450), C18, 80, &[]),
    route("plank-walk", (18820, 4450), BOTH, 700, &STEP_RIGHT),
    route("plank-750", (26244, 5950), BOTH, 700, &STEP_RIGHT),
    Route {
        store: "plank-reset",
        start: (18481, 4400),
        clocks: C18,
        rows: 800,
        spans: &[span(15, 94, RIGHT, true), span(520, 599, RIGHT, true)],
        end_life: Some(400),
    },
    route(
        "statue-front",
        (2305, 2800),
        BOTH,
        300,
        &[span(15, 16, LEFT, true), span(22, 23, ATTACK, true)],
    ),
    route(
        "statue-behind",
        (2010, 2800),
        BOTH,
        300,
        &[span(16, 17, ATTACK, true)],
    ),
    route("statue-draw", (2215, 2800), &[], 400, &[]),
];

/// The attack key.
const ATTACK: u32 = 0x2;

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. What they come from:
///
/// - c18, 36 ms (rng): the town guards are not built yet. In the original
///   those awake around Claw each make the enemies' per-tick roll on every
///   frame pulse tick, so the random stream runs ahead of the game's from
///   the first pulse tick. The planks never read it. The statues' routes
///   follow the original on every column at c18: the objects awake where
///   Claw was put down from run once more in the warp and go to sleep
///   before his first tick there, so the statue's fan and its break sound
///   draw the original's values in the original's order (eight speeds,
///   then the sound).
/// - c18, the town guards again: Claw falls past (plank-edge-18832) a
///   town guard on a lower row, or walks into one (plank-reset, after the
///   restore), whose touch or strike costs him 10 health in the original;
///   here the guards stand still. On plank-walk Claw lands beside one and
///   follows the original to the end, the random stream included.
/// - c18, plank-edge-18809, 918 ms: Claw stands balancing on the edge of
///   the second piece's cell when it goes; in the original he stands for a
///   tick before he falls, here he falls at once (his own states, not the
///   planks': the floor goes on the original's tick).
/// - c18, plank-edge-18809, 3186 ms (health): the same early fall happens
///   again at the next row's edge, so Claw falls two ticks ahead of the
///   original and meets the town guard below two ticks earlier, at the
///   same height; from 3222 ms his health matches again.
/// - c18, plank-reset, 12132 ms: after the life ended from the in-game
///   menu (whose frames keep the frame pulse going, as in the original)
///   and the restore, Claw's state parts from the original's, not traced
///   yet.
/// - c60 (the default tuning played tick by tick): the planks count 18 ms
///   ticks, so a 500 ms wait is 28 ticks (504 ms) against 30 frames
///   (500 ms) and the 26 crumble calls to the floor's end take 468 ms
///   against 26 frames (433 ms): Claw falls through each row later than
///   in the original, and lands, falls into the death tiles (plank-750)
///   and meets the guards later.
const KNOWN: &[Known] = &[
    ("plank-gap", "c18", &[("rng", 36)]),
    (
        "plank-edge-18809",
        "c18",
        &[
            ("rng", 36),
            ("state_id", 918),
            ("physics_type", 918),
            ("y", 954),
            ("cam_y", 954),
            ("health", 3186),
        ],
    ),
    ("plank-edge-18810", "c18", &[("rng", 36)]),
    ("plank-edge-18831", "c18", &[("rng", 36)]),
    (
        "plank-edge-18832",
        "c18",
        &[
            ("rng", 36),
            ("state_id", 2178),
            ("health", 2178),
            ("y", 2196),
            ("cam_y", 2196),
            ("physics_type", 2232),
            ("x", 3078),
            ("cam_x", 3078),
        ],
    ),
    (
        "plank-walk",
        "c60",
        &[
            ("y", 1583),
            ("cam_y", 1583),
            ("state_id", 2633),
            ("physics_type", 2633),
            ("health", 3000),
            ("x", 3917),
            ("cam_x", 3917),
        ],
    ),
    ("plank-750", "c18", &[("rng", 36)]),
    (
        "plank-750",
        "c60",
        &[
            ("y", 1800),
            ("cam_y", 1800),
            ("lives", 4817),
            ("x", 4833),
            ("cam_x", 4833),
            ("state_id", 4833),
            ("physics_type", 4833),
            ("health", 4833),
        ],
    ),
    (
        "plank-reset",
        "c18",
        &[
            ("state_id", 12132),
            ("physics_type", 12132),
            ("y", 12168),
            ("cam_y", 12168),
            ("rng", 12366),
        ],
    ),
    ("statue-front", "c60", &[("score", 1633)]),
    ("statue-behind", "c60", &[("score", 1433)]),
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
    for s in r.spans {
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

/// A store's own copy of a route.
struct StoreRoute {
    /// (held, pressed) by row.
    rows: Vec<(u32, u32)>,
    /// The rows before which Claw ends a life.
    end_life: Vec<usize>,
    /// The in-game menus before the rows: (row, ms).
    menus: Vec<(usize, u32)>,
}

/// The route file of a store.
fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    let rows = text
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (hex(f[1]), hex(f[2]))
        })
        .collect();
    let end_life = text
        .lines()
        .filter(|l| l.ends_with("command end_life"))
        .filter_map(|l| l.trim_start_matches('@').split(' ').next()?.parse().ok())
        .collect();
    let menus = text
        .lines()
        .filter_map(|l| {
            let (row, rest) = l.strip_prefix('@')?.split_once(' ')?;
            let ms = rest.strip_prefix("menu ingame ")?;
            Some((row.parse().ok()?, ms.trim().parse().ok()?))
        })
        .collect();
    Some(StoreRoute {
        rows,
        end_life,
        menus,
    })
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
    let root = std::env::var_os("CLAW_REFERENCE").map(PathBuf::from);
    if root.is_none() {
        eprintln!("skipped: CLAW_REFERENCE is not set");
    }
    root
}

/// The original's game time at the routes' time zero at the 18 ms step,
/// ms, and the random state then.
const T0_MS: u64 = 828;
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
    sim.place_between_ticks(start.0, start.1);
    let mut still = 0;
    let mut last = None;
    for _ in 0..40 {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
    assert!(still >= 10, "Claw settles at {start:?}");
    assert!(sim.tick() * TICK <= t0_ms, "settled before time zero");
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
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

/// Plays one row of route `r` (ending a life first when the route says
/// so).
fn step_row(sim: &mut Sim, r: &Route, ticks: &[TickInput], k: usize) {
    if r.end_life == Some(k) {
        sim.pass_ingame_menu(END_LIFE_MENU_MS);
        sim.end_life();
    }
    sim.step(ticks.get(k).copied().unwrap_or_default());
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs; returns its rows and that trace, or
/// `None` when the store lacks the trace.
fn play(
    level: &SimLevel,
    root: &Path,
    r: &Route,
    clock: &str,
) -> Option<(Vec<TraceRow>, Vec<TraceRow>)> {
    let dir = root.join("level5").join(r.store);
    let text = std::fs::read_to_string(dir.join(format!("trace-{clock}-r1.csv"))).ok()?;
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) = time_zero(&dir, clock)?;
    let mut sim = at_time_zero(level, clock, r.start, t0, seed);
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        step_row(&mut sim, r, &ticks, k);
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        k += 1;
    }
    Some((rows, reference))
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
        let Some(StoreRoute {
            rows,
            end_life,
            menus,
        }) = store_route(&root.join("level5").join(r.store))
        else {
            println!("{}: no route in the store", r.store);
            continue;
        };
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        assert_eq!(end_life, r.end_life.into_iter().collect::<Vec<_>>());
        let our_menus: Vec<(usize, u32)> = r
            .end_life
            .map(|k| (k, END_LIFE_MENU_MS))
            .into_iter()
            .collect();
        assert_eq!(menus, our_menus, "{}: in-game menus", r.store);
    }
}

#[test]
fn install_level5_plank_and_statue_routes_follow_the_reference_traces() {
    let Some(level) = level5() else { return };
    let Some(root) = reference_root() else { return };
    let mut failures = Vec::new();
    for r in &ROUTES {
        for &clock in r.clocks {
            println!("== level5/{} {clock}", r.store);
            let Some((rows, reference)) = play(&level, &root, r, clock) else {
                println!("skipped: no reference trace");
                continue;
            };
            failures.extend(new_divergences(r.store, clock, &rows, &reference));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ---------------------------------------------------------------- planks

/// The pieces of the row whose spawner stands at `at`, left to right.
fn row(sim: &Sim, at: (i32, i32)) -> Vec<ObjectId> {
    let spawner = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == break_plank::SPAWNER && (o.x, o.y) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("the row at {at:?}"));
    let o = sim.objects().get(spawner).unwrap();
    break_plank::data_of(o)
        .map(|d| d.pieces.clone())
        .unwrap_or_default()
}

/// What a piece shows after a row: state, frame, hidden.
fn piece(sim: &Sim, id: ObjectId) -> (i32, i32, bool) {
    let o = sim.objects().get(id).expect("the piece");
    (o.state, o.frame, o.draw_flags & draw_flags::HIDDEN != 0)
}

/// The first row from `from` on which `f` holds over the recorded rows.
fn first_row<T>(rows: &[T], from: usize, f: impl Fn(&T) -> bool) -> Option<usize> {
    (from..rows.len()).find(|&k| f(&rows[k]))
}

/// What a plank route showed, row by row.
struct PlankRun {
    /// For each row: each watched plank row's pieces, (state, frame,
    /// hidden).
    pieces: Vec<Vec<Vec<(i32, i32, bool)>>>,
    /// For each row: Claw's (x, y, physics type).
    claw: Vec<(i32, i32, i32)>,
    /// The rows on which the crumble sound played.
    sounds: Vec<usize>,
}

/// Plays `r` at the 18 ms step and records, after each row, every piece
/// of the rows at `rows`, Claw's (x, y, physics type) and whether the
/// crumble sound played.
fn plank_run(level: &SimLevel, r: &Route, rows: &[(i32, i32)], n: usize) -> PlankRun {
    let mut sim = at_time_zero(level, "c18", r.start, T0_MS, SEED);
    let ticks = route_ticks(r);
    let mut pieces = Vec::new();
    let mut claw = Vec::new();
    let mut sounds = Vec::new();
    for k in 0..n {
        step_row(&mut sim, r, &ticks, k);
        pieces.push(
            rows.iter()
                .map(|&at| row(&sim, at).iter().map(|&p| piece(&sim, p)).collect())
                .collect(),
        );
        let c = sim.player().expect("Claw");
        claw.push((c.x, c.y, c.physics_type));
        if sim
            .last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == "LEVEL_CRUMBLEBRIDGE2"))
        {
            sounds.push(k);
        }
    }
    PlankRun {
        pieces,
        claw,
        sounds,
    }
}

const ROW_500: (i32, i32) = (18727, 4633);
const ROW_500_MID: (i32, i32) = (18727, 4825);
const ROW_500_LOW: (i32, i32) = (18728, 5015);
const ROW_750: (i32, i32) = (26215, 6104);

#[test]
fn install_level5_every_plank_row_is_built_with_its_floor_cells() {
    let Some(level) = level5() else { return };
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level.clone(),
        SimConfig {
            activation: claw_sim::ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 2);
    let spawners: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == break_plank::SPAWNER)
        .map(|(_, o)| (o.x, o.y, o.state, o.draw_flags))
        .collect();
    assert_eq!(spawners.len(), 21);
    assert!(
        spawners
            .iter()
            .all(|s| s.2 == plank_state::IDLE && s.3 & draw_flags::HIDDEN != 0)
    );
    let pieces: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == break_plank::PIECE)
        .map(|(_, o)| o)
        .collect();
    assert_eq!(pieces.len(), 111);
    let mut empty = Vec::new();
    for o in &pieces {
        assert_eq!(o.move_rect, break_plank::FALLBACK_MOVE_RECT);
        assert_eq!(o.state, plank_state::IDLE);
        assert_eq!(o.frame, 1);
        assert_eq!(o.z, 1011);
        let d = break_plank::data_of(o).expect("piece data");
        if d.floor_tile < 0 {
            empty.push((o.x, o.y));
        } else {
            assert_eq!(d.floor_tile, 509, "{}, {}", o.x, o.y);
        }
    }
    // The first piece of the middle row stands over an empty cell.
    assert_eq!(empty, [ROW_500_MID]);
    // The rows' delays.
    let mut delays = std::collections::BTreeMap::new();
    for o in &pieces {
        *delays
            .entry(break_plank::data_of(o).unwrap().delay_ms)
            .or_insert(0) += 1;
    }
    println!("{delays:?}");
    assert_eq!(delays.keys().copied().collect::<Vec<_>>(), [250, 500, 750]);
}

#[test]
fn install_level5_a_walk_into_a_window_crumbles_the_rows_on_the_originals_ticks() {
    let Some(level) = level5() else { return };
    let PlankRun {
        pieces,
        claw,
        sounds,
    } = plank_run(
        &level,
        &ROUTES[5],
        &[ROW_500, ROW_500_MID, ROW_500_LOW],
        260,
    );
    let state = |k: usize, r: usize, i: usize| pieces[k][r][i].0;
    let wait = |r: usize, i: usize| {
        first_row(&pieces, 0, |p| p[r][i].0 == plank_state::WAIT).expect("it waits")
    };
    let crumble = |r: usize, i: usize| {
        first_row(&pieces, 0, |p| p[r][i].0 == plank_state::CRUMBLE).expect("it crumbles")
    };
    // Claw reaches x 18832 on row 17; piece 18855 sees him on row 18.
    assert_eq!(claw[17].0, 18832);
    assert_eq!(state(17, 0, 2), plank_state::IDLE);
    let e = wait(0, 2);
    assert_eq!(e, 18);
    // The 500 ms wait is 28 ticks; the neighbours wait from then.
    let c = crumble(0, 2);
    assert_eq!(c, e + 28);
    assert_eq!((wait(0, 1), wait(0, 3)), (c, c));
    // Frame 2 and its sound on C + 6, the floor gone and Claw falling on
    // C + 26, hidden on C + 61.
    assert_eq!(pieces[c + 5][0][2].1, 1);
    assert_eq!(pieces[c + 6][0][2].1, 2);
    assert_eq!(sounds.first(), Some(&(c + 6)));
    assert_eq!(pieces[c + 26][0][2].1, 6);
    assert_eq!(claw[c + 25].2, ptype::STAND);
    assert_eq!(claw[c + 26].2, ptype::FALL);
    assert!(!pieces[c + 60][0][2].2);
    assert!(pieces[c + 61][0][2].2);
    // Landing on the rows below triggers them as walking does.
    assert_eq!(
        first_row(&pieces, 0, |p| p[1]
            .iter()
            .any(|q| q.0 != plank_state::IDLE)),
        Some(96)
    );
    assert_eq!(
        first_row(&pieces, 0, |p| p[2]
            .iter()
            .any(|q| q.0 != plank_state::IDLE)),
        Some(174)
    );
}

#[test]
fn install_level5_the_750_ms_row_waits_42_ticks() {
    let Some(level) = level5() else { return };
    let PlankRun { pieces, .. } = plank_run(&level, &ROUTES[6], &[ROW_750], 160);
    let wait = first_row(&pieces, 0, |p| p[0][1].0 == plank_state::WAIT).expect("it waits");
    let crumble =
        first_row(&pieces, 0, |p| p[0][1].0 == plank_state::CRUMBLE).expect("it crumbles");
    assert_eq!(crumble - wait, 42);
    assert_eq!(pieces[crumble + 26][0][1].1, 6);
    assert!(pieces[crumble + 61][0][1].2);
}

#[test]
fn install_level5_the_trigger_window_runs_from_x_minus_23_to_x_plus_18() {
    let Some(level) = level5() else { return };
    // Standing at x 18809 (piece 18791's x + 18) or 18832 (piece 18855's
    // x - 23), the piece is crumbling at time zero; at 18810, 18820 and
    // 18831 nothing starts until a seagull's beak knocks Claw into a
    // window (row 100 and later).
    for (r, piece, started) in [
        (&ROUTES[1], 1, true),
        (&ROUTES[2], 1, false),
        (&ROUTES[0], 1, false),
        (&ROUTES[3], 2, false),
        (&ROUTES[4], 2, true),
    ] {
        let PlankRun { pieces, .. } = plank_run(&level, r, &[ROW_500], r.rows);
        let first = first_row(&pieces, 0, |p| {
            p[0].iter().any(|q| q.0 != plank_state::IDLE)
        });
        println!("{}: first change {first:?}", r.store);
        if started {
            assert_eq!(pieces[0][0][piece].0, plank_state::CRUMBLE, "{}", r.store);
        } else {
            assert!(first.is_none_or(|k| k >= 100), "{}: {first:?}", r.store);
        }
    }
}

#[test]
fn install_level5_a_death_restores_the_row_on_the_tick_after_the_lives_step() {
    let Some(level) = level5() else { return };
    let r = &ROUTES[7];
    let PlankRun { pieces, claw, .. } = plank_run(&level, r, &[ROW_500, ROW_500_MID], 600);
    // Both rows broke under Claw before he ended his life.
    assert!(pieces[399][0].iter().all(|p| p.2), "the top row is gone");
    assert!(pieces[399][1].iter().any(|p| p.2));
    let restored = first_row(&pieces, 400, |p| {
        p.iter()
            .flatten()
            .all(|q| q.0 == plank_state::IDLE && q.1 == 1 && !q.2)
    })
    .expect("restored");
    println!("restored on row {restored}");
    assert_eq!(restored, 494);
    // The floor is back: walking right again Claw stands on the row and
    // starts its second piece on row 591.
    assert_eq!(
        first_row(&pieces, 495, |p| p[0][1].0 == plank_state::WAIT),
        Some(591)
    );
    assert!(claw[595..].iter().any(|c| c.1 == 4511));
}

// ---------------------------------------------------------------- statues

fn statue_at(sim: &Sim, name: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == name && (o.x, o.y) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{name} at {at:?}"))
}

const FRONT: (i32, i32) = (2191, 2842);
const BEHIND: (i32, i32) = (2106, 2806);

/// A goodie as it was on the row it appeared.
#[derive(Debug, Clone, Copy)]
struct Goodie {
    row: usize,
    at: (i32, i32, i32),
    vx: f64,
    vy: f64,
}

/// Plays a statue route at the 18 ms step and records the statue's
/// (state, frame) after each row, and the goodies made on each row.
fn statue_run(
    level: &SimLevel,
    r: &Route,
    name: &str,
    at: (i32, i32),
    n: usize,
) -> (Vec<(i32, i32)>, Vec<Goodie>, Sim) {
    let mut sim = at_time_zero(level, "c18", r.start, T0_MS, SEED);
    let id = statue_at(&sim, name, at);
    let ticks = route_ticks(r);
    let mut seen: std::collections::BTreeSet<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == pickup::BOUNCING_GOODIE)
        .map(|(g, _)| g)
        .collect();
    let mut rows = Vec::new();
    let mut goodies = Vec::new();
    for k in 0..n {
        step_row(&mut sim, r, &ticks, k);
        let o = sim.objects().get(id).expect("the statue stays");
        rows.push((o.state, o.frame));
        for (g, o) in sim.objects().all() {
            if &*o.logic == pickup::BOUNCING_GOODIE && seen.insert(g) {
                let (vx, vy) = goodie_speed(&sim, g);
                goodies.push(Goodie {
                    row: k,
                    at: (o.x, o.y, o.z),
                    vx,
                    vy,
                });
            }
        }
    }
    (rows, goodies, sim)
}

fn goodie_speed(sim: &Sim, g: ObjectId) -> (f64, f64) {
    match &sim.objects().get(g).expect("the goodie").data {
        ObjectData::Pickup(d) => d.goodie.map_or((0.0, 0.0), |m| (m.vx, m.vy)),
        _ => (0.0, 0.0),
    }
}

#[test]
fn install_level5_a_statue_breaks_on_the_originals_ticks_and_stays() {
    let Some(level) = level5() else { return };
    for (r, name, at, hit, drop) in [
        (&ROUTES[8], statue::FRONT, FRONT, 28, (2191, 2818, 4999)),
        (&ROUTES[9], statue::BEHIND, BEHIND, 22, (2106, 2782, 1011)),
    ] {
        let (rows, goodies, sim) = statue_run(&level, r, name, at, 120);
        let broke = first_row(&rows, 0, |s| s.0 == statue_state::BREAKING).expect("it breaks");
        assert_eq!(broke, hit, "{name}");
        assert_eq!(rows[hit + 1].1, 2);
        // Images 3 to 10 every 7 ticks from hit + 8.
        for (i, image) in (3..=10).enumerate() {
            let k = hit + 8 + 7 * i;
            assert_eq!(
                (rows[k - 1].1, rows[k].1),
                (image - 1, image),
                "{name} image {image}"
            );
        }
        assert_eq!(rows[hit + 62].0, statue_state::BREAKING);
        assert_eq!(rows[hit + 63].0, statue_state::BROKEN);
        assert_eq!(rows[hit + 64], (statue_state::IDLE, 10));
        assert_eq!(rows.last(), Some(&(statue_state::IDLE, 10)), "rubble kept");
        // Eight goodies on the hit row, 24 px above the statue at its
        // depth minus or plus one, fanned from -100 to 75 px/s and thrown
        // up at 200 to 500 px/s.
        println!("{name}: {goodies:?}");
        assert_eq!(goodies.len(), 8, "{name}");
        assert!(goodies.iter().all(|g| g.row == hit && g.at == drop));
        let vx: Vec<f64> = goodies.iter().map(|g| g.vx).collect();
        assert_eq!(vx, [-100.0, -75.0, -50.0, -25.0, 0.0, 25.0, 50.0, 75.0]);
        assert!(goodies.iter().all(|g| (-500.0..=-200.0).contains(&g.vy)));
        let o = sim.objects().get(statue_at(&sim, name, at)).unwrap();
        assert!(
            o.flags & claw_sim::object::flags::SAFE != 0,
            "safe for good"
        );
        assert_eq!(o.draw_flags & draw_flags::HIDDEN, 0, "the rubble is drawn");
    }
}

#[test]
fn install_level5_one_swing_from_x_2257_breaks_both_statues_drawn_in_depth_order() {
    let Some(level) = level5() else { return };
    // The front statue's route, from 48 px further left: Claw's sword
    // reaches 112 px.
    let r = Route {
        start: (2257, 2800),
        ..route("statue-front", (2305, 2800), BOTH, 300, ROUTES[8].spans)
    };
    let mut sim = at_time_zero(&level, "c18", r.start, T0_MS, SEED);
    let front = statue_at(&sim, statue::FRONT, FRONT);
    let behind = statue_at(&sim, statue::BEHIND, BEHIND);
    let ticks = route_ticks(&r);
    let mut broke = (None, None);
    for k in 0..40 {
        step_row(&mut sim, &r, &ticks, k);
        let st = |id| sim.objects().get(id).unwrap().state;
        if broke.0.is_none() && st(front) == statue_state::BREAKING {
            broke.0 = Some(k);
        }
        if broke.1.is_none() && st(behind) == statue_state::BREAKING {
            broke.1 = Some(k);
        }
    }
    assert!(broke.0.is_some() && broke.0 == broke.1, "{broke:?}");
    // The list is the draw order: the behind statue and its treasure, then
    // Claw, then the front statue's treasure and the front statue.
    let order: Vec<(String, i32)> = sim
        .objects()
        .listed()
        .into_iter()
        .filter_map(|id| sim.objects().get(id))
        .filter(|o| {
            matches!(
                &*o.logic,
                statue::FRONT | statue::BEHIND | pickup::BOUNCING_GOODIE
            ) || sim.player().is_some_and(|c| c.serial == o.serial)
        })
        .map(|o| (o.logic.to_string(), o.z))
        .collect();
    println!("{order:?}");
    let depths: Vec<i32> = order.iter().map(|(_, z)| *z).collect();
    assert!(depths.is_sorted());
    let first = |name: &str| order.iter().position(|(n, _)| n == name).unwrap();
    let claw = order.iter().position(|(_, z)| *z == 4000).expect("Claw");
    assert_eq!(order[first(statue::BEHIND)].1, 1010);
    assert_eq!(order[first(statue::FRONT)].1, 5000);
    assert!(first(statue::BEHIND) < claw && claw < first(statue::FRONT));
    for (i, (n, z)) in order.iter().enumerate() {
        if n == pickup::BOUNCING_GOODIE {
            assert!(
                (*z == 1011 && i < claw) || (*z == 4999 && i > claw),
                "{order:?}"
            );
        }
    }
}
