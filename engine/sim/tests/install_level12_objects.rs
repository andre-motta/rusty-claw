//! The twelfth level's stalactites and earthquake triggers on your own
//! install, against the original's runs of the same short routes.
//!
//! Each route puts Claw down at a fixed point of the level after its first
//! tick, the way the original's runs put him there (between two ticks, as
//! a warp does), at the height he settles at there, waits until the level
//! clock reaches the time the original's run started at, and starts the
//! shared random stream from the run's state, once. The routes:
//!
//! - on the ledge at the water's edge at 3110, right of the trigger of
//!   the first stalactites: Left on rows 20 to 29 walks Claw into it, and
//!   the three stalactites over the ledge fall, land, crumble and vanish;
//! - at 6696 under the hanging stalactite at 6996: one pistol shot on row
//!   20, which flies through it;
//! - on the lip of the first shaft at 2925: Right on rows 15 to 24 drops
//!   Claw into the shaft (the Mercat over it shoots at him), through the
//!   first earthquake trigger's box (the screen shakes for five seconds,
//!   and the trigger fires no more for seven) and onto the ledge under the
//!   first stalactites, which fall beside him; the same with two pauses
//!   of 2000 ms during the shake, which change nothing;
//! - in the corridor at 25700 right of the tall shaft at 25540: Left on
//!   rows 15 to 199 drops Claw down the shaft, past its hanging
//!   stalactites, until the tall earthquake trigger at its foot comes
//!   within its own window of the view's centre and fires.
//!
//! Each route is played tick by tick and compared with the reference trace
//! of the same route: at the 18 ms step with the original's speeds every
//! column exactly (Claw's position and state, health, score, the camera,
//! the random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the watched stalactites and triggers are compared too
//! at the 18 ms step: their position, their state (lined up by the numbers
//! the run gives them, [`reference_state`]), whether they are out of every
//! collision and whether they are hidden.
//!
//! Divergences that are known, with their causes, are listed in [`KNOWN`];
//! any other fails the test. The routes are written below; each store's
//! own copy of its route must agree with them, its pauses included. Needs
//! `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each test
//! skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but a listed store lacks its route, meta or
//! trace (or, with the reference runs there, a run's `obj-log.csv` or a
//! watched object in it); `CLAW_L12_ONLY` names one store to play alone,
//! and `CLAW_L12_TRACE` a folder to write the replay's traces to. Nothing
//! from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::{self, EnemyState, mercat, siren};
use claw_sim::logic::{self, shake, stalactite};
use claw_sim::object::{ObjectId, draw_flags, engine_state, flags};
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The pistol button.
const PISTOL: u32 = 0x4;

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

/// A watched object: its logic, its serial number in the original's runs
/// and where it is placed.
#[derive(Debug, Clone, Copy)]
struct Watched {
    logic: &'static str,
    serial: &'static str,
    at: (i32, i32),
}

const fn stal(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: stalactite::NAMES[0],
        serial,
        at,
    }
}

const fn quake(serial: &'static str, at: (i32, i32)) -> Watched {
    Watched {
        logic: shake::NAMES[0],
        serial,
        at,
    }
}

/// A Mercat at a route's time zero as the original's run shows him: the
/// one placed at `at`, in `state`, his wait started on level tick
/// `wait.0` for `wait.1` ms (`wait_c60.1` ms at 60 frames per second), and
/// his idle time started on level tick `idle` (level tick 1 is the
/// level's first).
#[derive(Debug, Clone, Copy)]
struct Align {
    at: (i32, i32),
    state: EnemyState,
    wait: (u64, u64),
    wait_c60: (u64, u64),
    idle: u64,
}

const fn idle_since_first_tick(at: (i32, i32)) -> Align {
    Align {
        at,
        state: EnemyState::Idle,
        wait: (2, 1000),
        wait_c60: (2, 1000),
        idle: 1,
    }
}

/// A route of the stores: its name, where Claw is put down (at the height
/// he settles at), how many rows it has, its stretches, its pauses (row
/// and wall time), the clocks it was run at, the Mercats put in step at
/// time zero, the Sirens put in step there (placed at, and the length of
/// the wait at home they started on the level's first tick, ms) and the
/// objects it watches.
struct Route {
    store: &'static str,
    start: (i32, i32),
    rows: usize,
    spans: &'static [Span],
    pauses: &'static [(usize, u32)],
    clocks: &'static [&'static str],
    align: &'static [Align],
    sirens: &'static [((i32, i32), u64)],
    watch: &'static [Watched],
}

/// The original's random state at the routes' time zero.
const SEED: u32 = 111_567;
/// The original's game time at the routes' time zero at the 18 ms step.
const T0_MS: u64 = 828;

/// The three stalactites over the ledge at the water's edge, whose
/// triggers cover its left part.
const LEDGE_STALACTITES: [Watched; 3] = [
    stal("2487", (3135, 3422)),
    stal("2486", (3197, 3423)),
    stal("2485", (3257, 3423)),
];
/// The hanging stalactite the pistol shot flies through.
const PISTOL_STALACTITE: Watched = stal("2472", (6996, 2910));
/// The first earthquake trigger, over the first shaft.
const SHAFT_SHAKE: Watched = quake("353", (2979, 3302));
/// The tall earthquake trigger at the foot of the shaft at 25540.
const TALL_SHAKE: Watched = quake("308", (25540, 3166));

/// The Mercat on the first shaft's far lip, as the original's runs of the
/// shaft routes show him at time zero: at home, his wait of 1026 ms drawn
/// 36 ms into the level (34 ms at 60 frames per second), so he starts his
/// first bolt on row 12 at the 18 ms step and 250 ms after time zero at 60
/// frames per second (the nearest row of the replay, 252 ms, is row 13).
/// The two Mercats above the shaft need nothing.
const SHAFT_MERCATS: [Align; 1] = [Align {
    at: (2829, 2784),
    state: EnemyState::Home,
    wait: (2, 1026),
    wait_c60: (2, 1044),
    idle: 2,
}];

/// The Mercat on the floor left of the pistol spot, idle since the
/// level's first tick in the original's run (the replay wakes him a tick
/// later; he starts to pace on row 67), and the Siren beside the spot,
/// whose wait at home started then and ran out on row 48, where the run
/// shows her song start (her draw for it, before time zero, is not in the
/// run's logs: any wait from 1675 to 1692 ms gives that row). The two
/// Mercats above and right of the spot need nothing.
const PISTOL_MERCATS: [Align; 1] = [idle_since_first_tick((6367, 2916))];

const SHAFT_WATCH: [Watched; 4] = [
    SHAFT_SHAKE,
    LEDGE_STALACTITES[0],
    LEDGE_STALACTITES[1],
    LEDGE_STALACTITES[2],
];

const ROUTES: [Route; 5] = [
    Route {
        store: "stal-ledge",
        start: (3110, 3607),
        rows: 300,
        spans: &[span(20, 29, LEFT)],
        pauses: &[],
        clocks: &["c18", "c60"],
        align: &[],
        sirens: &[],
        watch: &LEDGE_STALACTITES,
    },
    Route {
        store: "pistol-stal-c18",
        start: (6696, 2903),
        rows: 120,
        spans: &[span(20, 21, PISTOL)],
        pauses: &[],
        clocks: &["c18"],
        align: &PISTOL_MERCATS,
        sirens: &[((6541, 2921), 1692)],
        watch: &[PISTOL_STALACTITE],
    },
    Route {
        store: "shake-shaft",
        start: (2925, 2775),
        rows: 500,
        spans: &[span(15, 24, RIGHT)],
        pauses: &[],
        clocks: &["c18", "c60"],
        align: &SHAFT_MERCATS,
        sirens: &[],
        watch: &SHAFT_WATCH,
    },
    Route {
        store: "shake-pause-c18",
        start: (2925, 2775),
        rows: 500,
        spans: &[span(15, 24, RIGHT)],
        pauses: &[(150, 2000), (200, 2000)],
        clocks: &["c18"],
        align: &SHAFT_MERCATS,
        sirens: &[],
        watch: &SHAFT_WATCH,
    },
    Route {
        store: "shake-tall-c18",
        start: (25700, 2199),
        rows: 400,
        spans: &[span(15, 199, LEFT)],
        pauses: &[],
        clocks: &["c18"],
        align: &[],
        sirens: &[],
        watch: &[TALL_SHAKE],
    },
];

/// A store, a clock, and the first divergence of each column known there.
type Known = (&'static str, &'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per store and clock; a column not listed must stay within
/// it, and one listed must not leave it earlier. Every route follows its
/// store exactly at the 18 ms step, its Mercats' bolts, the Siren's
/// scream and the random stream included. At 60 frames per second:
///
/// - shake-shaft: Claw falls through the shaft at about 1000 px/s, and
///   the replay's 18 ms rows meet the run's 16 and 17 ms frames at other
///   moments of the fall, so on two rows his height differs by more than
///   the feel tolerance (up to 9 px); and the camera's jolts during the
///   earthquake are drawn frame by frame from the random stream, which at
///   60 frames per second the replay does not follow, so on five rows the
///   camera's height differs by up to 13 px. Both close again once he
///   lands on the ledge.
const KNOWN: &[Known] = &[("shake-shaft", "c60", &[("y", 1133), ("cam_y", 1067)])];

fn known(store: &str, clock: &str) -> Vec<(&'static str, u64)> {
    KNOWN
        .iter()
        .filter(|(s, c, _)| *s == store && *c == clock)
        .flat_map(|(_, _, k)| k.iter().copied())
        .collect()
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
    for s in r.spans {
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

/// The rows of a store's own copy of a route, (held, pressed) by row, and
/// its pauses, (row, wall ms).
type StoreRoute = (Vec<(u32, u32)>, Vec<(usize, u32)>);

fn store_route(dir: &Path) -> Option<StoreRoute> {
    let text = std::fs::read_to_string(dir.join("route.csv")).ok()?;
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    let mut rows = Vec::new();
    let mut pauses = Vec::new();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix('@') {
            let f: Vec<&str> = rest.split_whitespace().collect();
            assert_eq!(f.get(1), Some(&"pause"), "{l}: only pauses are expected");
            pauses.push((f[0].parse().ok()?, f.get(2)?.parse().ok()?));
        } else if l.starts_with(|c: char| c.is_ascii_digit()) {
            let f: Vec<&str> = l.split(',').collect();
            rows.push((hex(f[1])?, hex(f[2])?));
        }
    }
    Some((rows, pauses))
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
    sim.place_between_ticks(r.start.0, r.start.1);
    while sim.tick() * TICK < t0_ms {
        sim.step(TickInput::default());
    }
    put_in_step(&mut sim, r, clock);
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// The game time the timers read during level tick `tick` (the first is
/// 1): the time at its start.
fn tick_start(tick: u64) -> u64 {
    (tick - 1) * TICK
}

/// Puts the Mercats of `r` in step with the original's run at time zero
/// (see [`Align`]).
fn put_in_step(sim: &mut Sim, r: &Route, clock: &str) {
    for a in r.align {
        let wait = if clock == "c18" { a.wait } else { a.wait_c60 };
        let id = placed(sim, mercat::NAMES[0], a.at);
        sim.with_world(|w| {
            if w.objects
                .get(id)
                .is_some_and(|o| o.state == engine_state::INIT)
            {
                // Built in the original's first ticks and asleep since, his
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
                m.wait = enemy::Timer::new(tick_start(wait.0), wait.1);
                m.idle = enemy::Timer::new(tick_start(a.idle), mercat::IDLE_MS);
            }
        });
    }
    for &(at, len) in r.sirens {
        let id = placed(sim, siren::NAMES[0], at);
        sim.with_world(|w| {
            assert_eq!(enemy::state(w, id), Some(EnemyState::Home));
            if let Some(s) = siren::siren_mut(w, id) {
                s.wait = enemy::Timer::new(0, len);
            }
        });
    }
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

/// The state a run's number for a watched object's state stands for, in
/// this build's numbers: a stalactite hangs (2069), waits its delay
/// (2072), falls (2066), crumbles (2073) or is gone (2074); a trigger
/// waits for Claw (1000) or has fired (2052).
fn reference_state(logic: &str, n: i32) -> Option<i32> {
    if logic == stalactite::NAMES[0] {
        Some(match n {
            2069 => stalactite::state::IDLE,
            2072 => stalactite::state::DELAY,
            2066 => stalactite::state::FALLING,
            2073 => stalactite::state::DEAD,
            2074 => stalactite::state::RESTART,
            _ => return None,
        })
    } else {
        Some(match n {
            1000 => shake::state::ARMED,
            2052 => shake::state::WAIT,
            _ => return None,
        })
    }
}

/// A watched object at one step of a run or of the replay: position,
/// state (the run's own numbers lined up through [`reference_state`]),
/// out of every collision, hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObjAt {
    x: i32,
    y: i32,
    state: Option<i32>,
    out: bool,
    hidden: bool,
}

/// The steps of the watched objects in a run's `obj-log.csv`, by object:
/// (step, the object there).
fn reference_objects(path: &Path, watch: &[Watched]) -> Option<Vec<Vec<(usize, ObjAt)>>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (tick, serial, st, x, y, fl, df) = (
        col("tick")?,
        col("serial")?,
        col("state")?,
        col("x")?,
        col("y")?,
        col("flags")?,
        col("draw_flags")?,
    );
    let mut out = vec![Vec::new(); watch.len()];
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let Some(i) = watch.iter().position(|w| f.get(serial) == Some(&w.serial)) else {
            continue;
        };
        out[i].push((
            f[tick].parse().ok()?,
            ObjAt {
                x: int(f[x])?,
                y: int(f[y])?,
                state: reference_state(watch[i].logic, int(f[st])?),
                out: hex(f[fl])? & flags::NO_COLLIDE != 0,
                hidden: hex(f[df])? & draw_flags::HIDDEN != 0,
            },
        ));
    }
    out.iter().all(|e| !e.is_empty()).then_some(out)
}

/// The first reference time (ms) at which the replay's object leaves the
/// run's, by column (`<serial>_x`, `_y`, `_state`, `_out`, `_hidden`); a
/// step on which the replay has no such object counts for all of them.
fn compare_object(
    serial: &str,
    played: &[Option<ObjAt>],
    reference: &[(usize, ObjAt)],
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
            for name in ["x", "y", "state", "out", "hidden"] {
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
        if got.out != want.out {
            note("out", k);
        }
        if got.hidden != want.hidden {
            note("hidden", k);
        }
    }
    firsts
}

/// The object of logic `logic` placed at `at`.
fn placed(sim: &Sim, logic: &str, at: (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .find(|(_, o)| (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} placed at {at:?}"))
}

fn obj_at(sim: &Sim, id: ObjectId) -> Option<ObjAt> {
    sim.objects().get(id).map(|o| ObjAt {
        x: o.x,
        y: o.y,
        state: Some(o.state),
        out: o.flags & flags::NO_COLLIDE != 0,
        hidden: o.draw_flags & draw_flags::HIDDEN != 0,
    })
}

/// One play of a route: its trace, the reference trace, and the watched
/// objects on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    objects: Vec<Vec<Option<ObjAt>>>,
}

/// Plays route `r` from its store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, r: &Route, clock: &str) -> Played {
    let dir = root.join("level12").join(r.store);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{}: no time zero in its meta", r.store));
    if clock == "c18" {
        assert_eq!(t0, T0_MS, "{}: time zero", r.store);
        assert_eq!(seed, SEED, "{}: random state", r.store);
    }
    let mut sim = at_time_zero(level, clock, r, t0, seed);
    let ids: Vec<ObjectId> = r
        .watch
        .iter()
        .map(|w| placed(&sim, w.logic, w.at))
        .collect();
    let ticks = route_ticks(r);
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut objects = vec![Vec::new(); ids.len()];
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        for (i, &id) in ids.iter().enumerate() {
            objects[i].push(obj_at(&sim, id));
        }
        k += 1;
    }
    Played {
        rows,
        reference,
        objects,
    }
}

/// Compares a play of route `r` at `clock` with the reference trace at
/// the clock's tolerances (exact for c18, feel for c60), and at c18 its
/// watched objects with the run's (when the reference runs are there);
/// returns the divergences not known.
fn new_divergences(root: &Path, r: &Route, clock: &str, p: &Played) -> Vec<String> {
    let store = r.store;
    let tol = if clock == "c18" {
        Tolerances::exact()
    } else {
        Tolerances::feel()
    };
    let cmp = trace::compare(&p.rows, &p.reference, &tol);
    print!("{}", trace::report(&p.rows, &p.reference, &cmp, 3));
    let mut out = Vec::new();
    if cmp.rows_compared != p.reference.len() {
        out.push(format!(
            "{store} {clock}: {} of {} rows compared",
            cmp.rows_compared,
            p.reference.len()
        ));
    }
    let mut firsts: Vec<(String, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| c.first_t_ms.map(|t| (c.column.to_string(), t)))
        .collect();
    if clock == "c18"
        && let Some(runs) = runs_root(root)
    {
        let meta = std::fs::read_to_string(root.join("level12").join(store).join("meta.toml"))
            .expect("meta");
        let id = run_id(&meta, clock).unwrap_or_else(|| panic!("{store}: no run id"));
        let log = runs.join(&id).join("obj-log.csv");
        let reference = reference_objects(&log, r.watch)
            .unwrap_or_else(|| panic!("{store}: {} lacks a watched object", log.display()));
        for (i, w) in r.watch.iter().enumerate() {
            firsts.extend(compare_object(w.serial, &p.objects[i], &reference[i]));
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
        let dir = root.join("level12").join(r.store);
        let (rows, pauses) =
            store_route(&dir).unwrap_or_else(|| panic!("{}: no route in the store", r.store));
        let ours: Vec<(u32, u32)> = route_ticks(r)
            .iter()
            .map(|t| (t.held.0, t.pressed.0))
            .collect();
        assert_eq!(ours.len(), rows.len(), "{}: rows", r.store);
        let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
        assert_eq!(first, None, "{}: first different row", r.store);
        assert_eq!(pauses, r.pauses, "{}: pauses", r.store);
        // Each clock the route lists has its meta, its start and its
        // trace.
        let meta = std::fs::read_to_string(dir.join("meta.toml"))
            .unwrap_or_else(|e| panic!("{}: meta: {e}", r.store));
        assert_eq!(meta_value(&meta, "", "diagnostic"), None, "{}", r.store);
        for clock in r.clocks {
            assert!(
                meta_value(&meta, clock, "t0_ms").is_some(),
                "{} {clock}: no time zero",
                r.store
            );
            let teleport = meta_value(&meta, clock, "teleport")
                .unwrap_or_else(|| panic!("{} {clock}: no start", r.store));
            let x: i32 = teleport.split(',').next().unwrap().parse().unwrap();
            assert_eq!(x, r.start.0, "{} {clock}: start", r.store);
            let trace = dir.join(format!("trace-{clock}-r1.csv"));
            assert!(trace.is_file(), "{}", trace.display());
        }
    }
}

#[test]
fn install_level12_object_routes_follow_the_reference_traces() {
    let Some(level) = level12() else { return };
    let Some(root) = reference_root() else { return };
    let only = std::env::var("CLAW_L12_ONLY").ok();
    let mut failures = Vec::new();
    for r in &ROUTES {
        if only.as_deref().is_some_and(|o| o != r.store) {
            continue;
        }
        for &clock in r.clocks {
            println!("== level12/{} {clock}", r.store);
            let p = play(&level, &root, r, clock);
            if let Some(out) = std::env::var_os("CLAW_L12_TRACE") {
                let out = PathBuf::from(out).join(format!("{}-{clock}.csv", r.store));
                std::fs::write(&out, trace::to_csv(&p.rows)).expect("the trace is written");
            }
            failures.extend(new_divergences(&root, r, clock, &p));
        }
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the routes

/// What one row of a route shows: Claw, the watched object and the sounds
/// of the row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    health: i32,
    obj: Option<ObjAt>,
    frame: i32,
    sounds: Vec<String>,
}

/// Plays `rows` rows of `store`'s route at the 18 ms step from its time
/// zero with the original's random state there, watching the object of
/// logic `logic` placed at `at`.
fn watch(level: &SimLevel, store: &str, rows: usize, logic: &str, at: (i32, i32)) -> Vec<Look> {
    let r = route_named(store);
    let mut sim = at_time_zero(level, "c18", r, T0_MS, SEED);
    let id = placed(&sim, logic, at);
    let ticks = route_ticks(r);
    let mut looks = Vec::new();
    for k in 0..rows {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        let c = sim.player().expect("Claw");
        looks.push(Look {
            x: c.x,
            y: c.y,
            health: sim.claw().stats.health,
            obj: obj_at(&sim, id),
            frame: sim.objects().get(id).map_or(0, |o| o.frame),
            sounds: sim
                .last_audio()
                .iter()
                .filter_map(|a| match a {
                    AudioCue::Play(s) => Some(s.key.clone()),
                    _ => None,
                })
                .collect(),
        });
    }
    looks
}

fn state_of(l: &Look) -> i32 {
    l.obj.and_then(|o| o.state).unwrap_or(-1)
}

/// The first row on which the watched object is in `state`.
fn first_in(rows: &[Look], state: i32) -> Option<usize> {
    rows.iter().position(|l| state_of(l) == state)
}

#[test]
fn install_level12_stalactite_777_falls_25_ticks_and_vanishes_20_later() {
    let Some(level) = level12() else { return };
    let rows = watch(
        &level,
        "stal-ledge",
        120,
        stalactite::NAMES[0],
        LEDGE_STALACTITES[0].at,
    );
    // Claw is at 3070 after row 29, inside its trigger: it lets go on row
    // 30 (T), falls from row 31, lands on row 55 (T + 25) at 3624.
    let t = first_in(&rows, stalactite::state::DELAY).expect("triggered");
    assert_eq!(t, 30);
    assert_eq!(rows[29].x, 3070);
    assert_eq!(first_in(&rows, stalactite::state::FALLING), Some(t + 1));
    let ys: Vec<i32> = rows[t + 1..=t + 5]
        .iter()
        .map(|l| l.obj.unwrap().y)
        .collect();
    assert_eq!(ys, [3422, 3423, 3424, 3427, 3430]);
    for n in 1..=25u32 {
        let y = rows[t + n as usize].obj.unwrap().y;
        assert_eq!(y, (3422 + stalactite::fall_after(n)).min(3624), "T + {n}");
    }
    assert_eq!(rows[t + 25].obj.unwrap().y, 3624);
    // It snaps on T + 1 and crashes on T + 26 as it starts to crumble, out
    // of every collision.
    let snap = |l: &Look| l.sounds.iter().any(|s| s == stalactite::SNAP_SOUND);
    let crash = |l: &Look| l.sounds.iter().any(|s| s == stalactite::HIT_SOUND);
    assert!(snap(&rows[t + 1]) && !snap(&rows[t]));
    assert_eq!(first_in(&rows, stalactite::state::DEAD), Some(t + 26));
    assert!(crash(&rows[t + 26]));
    assert!(rows[t + 26].obj.unwrap().out);
    // Its pictures 2 to 7 every third row, the last on T + 42; hidden from
    // T + 45.
    for (i, f) in (2..=7).enumerate() {
        assert_eq!(rows[t + 27 + 3 * i].frame, f, "picture {f}");
    }
    assert_eq!(first_in(&rows, stalactite::state::RESTART), Some(t + 45));
    assert!(rows[t + 45].obj.unwrap().hidden);
    assert!(!rows[t + 44].obj.unwrap().hidden);
    // Claw, left of its reach, is never hurt.
    assert!(rows.iter().all(|l| l.health == 100));
}

#[test]
fn install_level12_a_pistol_shot_flies_through_a_hanging_stalactite() {
    let Some(level) = level12() else { return };
    let r = route_named("pistol-stal-c18");
    let mut sim = at_time_zero(&level, "c18", r, T0_MS, SEED);
    let stal = placed(&sim, stalactite::NAMES[0], PISTOL_STALACTITE.at);
    let ticks = route_ticks(r);
    let mut furthest = 0;
    for k in 0..110 {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        for (_, o) in sim.objects().all() {
            if &*o.logic == "PistolBullet" {
                furthest = furthest.max(o.x);
            }
        }
        let s = sim.objects().get(stal).unwrap();
        assert_eq!(s.state, stalactite::state::IDLE, "row {k}");
        assert_eq!((s.x, s.y), PISTOL_STALACTITE.at);
    }
    // The shot passes the stalactite's box (x 6956 to 7003) and flies on.
    assert!(furthest > 7400, "{furthest}");
}

#[test]
fn install_level12_the_first_shake_fires_once_and_rearms_389_ticks_later() {
    let Some(level) = level12() else { return };
    let rows = watch(&level, "shake-shaft", 500, shake::NAMES[0], SHAFT_SHAKE.at);
    // It fires on row 83, as Claw falls into its box, with one rumble,
    // and stays out of every collision until row 472.
    let t = first_in(&rows, shake::state::WAIT).expect("fired");
    assert_eq!(t, 83);
    assert!(rows[t].obj.unwrap().out);
    let rumbles: Vec<usize> = (0..rows.len())
        .filter(|&k| {
            rows[k]
                .sounds
                .iter()
                .any(|s| s.starts_with("LEVEL_EARTHQUAKE"))
        })
        .collect();
    assert!(rumbles.len() <= 1, "{rumbles:?}");
    assert!(rumbles.iter().all(|&k| k == t));
    let back = (t..rows.len()).find(|&k| !rows[k].obj.unwrap().out);
    assert_eq!(back, Some(t + 389));
}

#[test]
fn install_level12_pauses_change_nothing_in_the_shake() {
    let Some(level) = level12() else { return };
    // Pauses change nothing in the game: the paused route plays the same
    // rows as the plain one.
    let plain = watch(&level, "shake-shaft", 500, shake::NAMES[0], SHAFT_SHAKE.at);
    let paused = watch(
        &level,
        "shake-pause-c18",
        500,
        shake::NAMES[0],
        SHAFT_SHAKE.at,
    );
    assert_eq!(plain, paused);
}

#[test]
fn install_level12_the_tall_shake_fires_only_near_its_own_position() {
    let Some(level) = level12() else { return };
    let rows = watch(
        &level,
        "shake-tall-c18",
        400,
        shake::NAMES[0],
        TALL_SHAKE.at,
    );
    let t = first_in(&rows, shake::state::WAIT).expect("fired");
    assert_eq!(t, 225);
    // Falling down the shaft, Claw is hit by its hanging stalactites every
    // 48 rows.
    let hits: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .collect();
    assert_eq!(hits, [36, 84, 132, 180, 228, 276]);
    assert!(
        hits.iter()
            .all(|&k| rows[k - 1].health - rows[k].health == 5)
    );
}
