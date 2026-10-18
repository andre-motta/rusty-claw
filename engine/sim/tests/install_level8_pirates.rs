//! The eighth level's red-tail pirates on your own install, against the
//! original's run of the same short route.
//!
//! The route puts Claw down 103 px right of the red-tail pirate standing
//! at x 30914 on the floor at 5531, far from the boss room, after the
//! level's first tick, the way the original's runs put him there (between
//! two ticks, as a warp does: the pirate wakes then, strikes him and
//! knocks him back before the route starts), waits until the level clock
//! reaches the time the original's run started at, and starts the shared
//! random stream from the run's state, once. Claw then taps Left to face
//! the pirate and swings his sword every 12 ticks: the pirate flinches
//! from the first blow that reaches it (a roll of 25), parries the second
//! (24) and dies from the third (77).
//!
//! The route is played tick by tick and compared with the reference
//! trace: at the 18 ms step with the original's speeds every column
//! exactly (Claw's position and state, health, score, the camera, the
//! random stream and the events), at 60 frames per second with the
//! default tuning within the feel tolerances. When the reference run also
//! lists the objects near Claw per step (`obj-log.csv` beside the run, in
//! the folder named by `CLAW_REFERENCE_RUNS`, by default `rt` next to the
//! reference store), the pirate's position and state are compared too,
//! exactly at the 18 ms step, its states lined up by the numbers the run
//! gives them ([`REFERENCE_STATES`]). Divergences that are known, with
//! their causes, are listed in [`KNOWN`]; any other fails the test. The
//! route is written below; the store's own copy of it must agree. Needs
//! `CLAW_INSTALL`, and `CLAW_REFERENCE` for the comparisons: each test
//! skips itself when what it needs is unset, and fails when
//! `CLAW_REFERENCE` is set but the store lacks its route, meta or trace.
//! Nothing from an install or a reference store is kept in the
//! repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::{self, EnemyState, redtail_pirate};
use claw_sim::object::{ObjectId, types};
use claw_sim::player::hit::INVULNERABLE_MS;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const ATTACK: u32 = 0x2;

/// The store of the route.
const STORE: &str = "redtail-sword";
/// Where the route puts Claw down.
const START: (i32, i32) = (30990, 5500);
/// The original's game time at the route's time zero at the 18 ms step,
/// ms: when Claw had settled at his place.
const T0_MS: u64 = 828;
/// The route's rows.
const ROWS: usize = 900;
/// The clocks the route was run at.
const CLOCKS: [&str; 2] = ["c18", "c60"];
/// The pirate of the route.
const PIRATE_AT: i32 = 30914;
/// The original's random state at the route's time zero.
const SEED: u32 = 111_567;

/// The route's keys by row: (held, pressed). A Left tap on rows 15 and
/// 16, then a sword swing of two rows every 12 rows from row 20.
fn route_ticks() -> Vec<TickInput> {
    let mut held = vec![0u32; ROWS];
    let mut pressed = vec![0u32; ROWS];
    held[15] = LEFT;
    held[16] = LEFT;
    pressed[15] = LEFT;
    for from in (20..=884).step_by(12) {
        held[from] |= ATTACK;
        held[from + 1] |= ATTACK;
        pressed[from] |= ATTACK;
    }
    held.iter()
        .zip(&pressed)
        .map(|(&h, &p)| TickInput {
            held: Buttons(h),
            pressed: Buttons(p),
        })
        .collect()
}

/// A clock and the first divergence of each column known there.
type Known = (&'static str, &'static [(&'static str, u64)]);

/// The first reference time (ms) at which each column already leaves its
/// tolerance, per clock; a column not listed must stay within it, and one
/// listed must not leave it earlier. What they come from:
///
/// - c18, 2016 ms (pirate_y): the killed pirate's body flies off on the
///   same arc, but on 6 of its rows the original's comes out 1 px lower
///   (its first row of flight rises 12 px, the replay's 13, and the two
///   are level again on the next row): the shared flight of a killed
///   enemy starts from the pirate's whole-pixel height, the original's
///   from a part of a pixel below it. The body touches nothing on its
///   way.
/// - c60 (the default tuning played tick by tick): the original at 60
///   frames per second advances the pirate's strikes once a frame, so the
///   second strike after time zero reaches Claw on its frame 17 (300 ms)
///   and knocks him back, which the replay's strike, live on another
///   tick, does not; Claw's position and health follow from there.
const KNOWN: &[Known] = &[
    ("c18", &[("pirate_y", 2016)]),
    (
        "c60",
        &[
            ("state_id", 300),
            ("health", 300),
            ("x", 350),
            ("cam_x", 350),
        ],
    ),
];

/// The numbers the reference runs give the pirate's states, where they
/// differ from the states' own numbers.
const REFERENCE_STATES: [(i32, EnemyState); 6] = [
    (2003, EnemyState::Strike),
    (2004, EnemyState::Dead),
    (2015, EnemyState::BlockHigh),
    (2018, EnemyState::Home),
    (2021, EnemyState::Advance),
    (2024, EnemyState::HitLow),
];

/// The state a reference run's number stands for.
fn reference_state(id: i32) -> Option<EnemyState> {
    REFERENCE_STATES
        .iter()
        .find(|(n, _)| *n == id)
        .map(|&(_, s)| s)
}

fn known(clock: &str) -> &'static [(&'static str, u64)] {
    KNOWN
        .iter()
        .find(|(c, _)| *c == clock)
        .map_or(&[], |(_, k)| k)
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

/// The pirate at one step of a run or of the replay: position and state
/// number (the run's own numbers lined up through [`REFERENCE_STATES`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PirateAt {
    x: i32,
    y: i32,
    state: Option<EnemyState>,
}

/// The pirate's steps in a run's `obj-log.csv`: the red-tail pirate
/// nearest x [`PIRATE_AT`] on the run's first step, by step.
fn reference_pirate(path: &Path) -> Option<Vec<(usize, PirateAt)>> {
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
        .filter(|f| f.get(logic) == Some(&"RedTailPirate"))
        .collect();
    let int = |s: &str| s.trim().parse::<i32>().ok();
    let ours = rows
        .iter()
        .filter(|f| f[tick] == "0")
        .min_by_key(|f| (int(f[x]).unwrap_or(0) - PIRATE_AT).abs())?[serial];
    rows.iter()
        .filter(|f| f[serial] == ours)
        .map(|f| {
            Some((
                f[tick].parse().ok()?,
                PirateAt {
                    x: int(f[x])?,
                    y: int(f[y])?,
                    state: reference_state(int(f[st])?),
                },
            ))
        })
        .collect()
}

/// The first reference time (ms) at which the replay's pirate leaves the
/// run's, by column (pirate_x, pirate_y, pirate_state); a step on which
/// one of them has no pirate counts for all three.
fn compare_pirate(
    played: &[Option<PirateAt>],
    reference: &[(usize, PirateAt)],
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
            for name in ["pirate_x", "pirate_y", "pirate_state"] {
                note(name, k);
            }
            continue;
        };
        if got.x != want.x {
            note("pirate_x", k);
        }
        if got.y != want.y {
            note("pirate_y", k);
        }
        if want.state.is_none() || got.state != want.state {
            note("pirate_state", k);
        }
    }
    firsts
}

/// What the original's runs show at the route's time zero, from what
/// happened before it: the pirate struck Claw once (10 health off) and
/// strikes again at time zero.
#[derive(Debug, Clone, Copy)]
struct AtTimeZero {
    /// Where Claw stands: put down at x 30990, at the 18 ms step the blow
    /// knocked him 27 px right; at 60 frames per second it reached him in
    /// the air and left him where he was put down.
    claw: (i32, i32),
    /// The time before time zero the blow landed, ms: what is left of the
    /// time after it in which enemies cannot hurt Claw counts from there.
    hit_ms_before: i32,
    /// Ticks the pirate walked at Claw before time zero (at 120 px/s from
    /// its place): two at the 18 ms step, which leave it 4.32 px in.
    walk_ticks: u32,
    /// Ticks before time zero the strike running then started on.
    strike_ticks_before: u64,
}

fn at_t0(clock: &str) -> AtTimeZero {
    if clock == "c18" {
        AtTimeZero {
            claw: (31017, 5500),
            hit_ms_before: 648,
            walk_ticks: 2,
            strike_ticks_before: 2,
        }
    } else {
        AtTimeZero {
            claw: START,
            hit_ms_before: 650,
            walk_ticks: 0,
            strike_ticks_before: 7,
        }
    }
}

/// Claw's health at time zero.
const HEALTH_AT_T0: i32 = 90;

/// The game at the route's time zero (see the module documentation).
///
/// Before time zero the original's pirate struck Claw and stepped in,
/// from draws of the original's random stream, which the replay does not
/// have; the replay puts Claw where that blow left him ([`at_t0`]), keeps
/// the pirate at its place until the strike the original's pirate is in at
/// time zero starts, and starts that strike then with Claw as the blow
/// left him ([`align`]). The stream is the original's from time zero on.
fn at_time_zero(level: &SimLevel, clock: &str, t0_ms: u64, seed: u32) -> Sim {
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
    let at = at_t0(clock);
    sim.with_world(|w| w.claw.special.warp_to = Some(at.claw));
    let strike_at = t0_ms.saturating_sub(at.strike_ticks_before * TICK);
    let mut id = None;
    while sim.tick() * TICK < t0_ms {
        if sim.tick() * TICK >= strike_at {
            if let Some(id) = id.take() {
                align(&mut sim, id, &at);
            }
        } else if let Some(id) = id {
            park(&mut sim, id);
        }
        sim.step(TickInput::default());
        if id.is_none() && sim.tick() * TICK < strike_at {
            id = Some(find(&sim, PIRATE_AT));
        }
    }
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// Keeps the pirate idle at its place before time zero.
fn park(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.x = PIRATE_AT;
        }
        enemy::set_state(w, id, EnemyState::Idle);
        if let Some(d) = enemy::data_mut(w, id) {
            d.busy = false;
            d.vx = 0.0;
            d.deferred = None;
        }
    });
}

/// Starts the strike the original's pirate is in at time zero, at its
/// place then, alerted, its hop chance whole; Claw has the health the
/// blow before left him and what is left of his time safe from enemies
/// after it.
fn align(sim: &mut Sim, id: ObjectId, at: &AtTimeZero) {
    let since_hit = at.hit_ms_before - (at.strike_ticks_before * TICK) as i32;
    sim.with_world(|w| {
        w.claw.stats.health = HEALTH_AT_T0;
        w.claw.stats.invulnerable_ms = (INVULNERABLE_MS - since_hit).max(0);
        if let Some(c) = w.claw_id.and_then(|id| w.objects.get_mut(id)) {
            c.hit_type_flags &= !types::ENEMY;
        }
        let walked = f64::from(at.walk_ticks)
            * f64::from(redtail_pirate::ADVANCE_SPEED)
            * (TICK as f64 / 1000.0);
        let (x, y) = {
            let o = w.objects.get_mut(id).expect("the pirate");
            o.x = PIRATE_AT + walked.trunc() as i32;
            (o.x, o.y)
        };
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(d) = enemy::data_mut(w, id) {
            d.carry = (walked.fract(), 0.0);
            d.moved_to = Some((x, y));
            d.busy = false;
            d.vx = 0.0;
            d.deferred = None;
            d.alerted = true;
        }
        enemy::face_claw(w, id);
        redtail_pirate::strike(w, id);
        let now = w.now_ms;
        if let Some(ObjectData::RedTailPirate(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            p.jump_back_chance = redtail_pirate::FIRST_JUMP_BACK_CHANCE;
            p.strike_rest = enemy::Timer::new(now, u64::from(redtail_pirate::CLOSE_REST_MS));
        }
    });
}

/// The store's time zero and random state for `clock`.
fn time_zero(dir: &Path, clock: &str) -> Option<(u64, u32)> {
    let meta = std::fs::read_to_string(dir.join("meta.toml")).ok()?;
    let t0 = meta_value(&meta, clock, "t0_ms")?.parse().ok()?;
    let seed = meta_value(&meta, clock, "rng_at_t0")
        .or_else(|| meta_value(&meta, "", "rng_at_t0"))?
        .parse()
        .ok()?;
    Some((t0, seed))
}

/// One play of the route: its trace, the reference trace, and the
/// pirate's place and state on each row.
struct Played {
    rows: Vec<TraceRow>,
    reference: Vec<TraceRow>,
    pirate: Vec<Option<PirateAt>>,
}

/// Plays the route from the store's time zero at `clock` for as long as
/// the store's first trace runs. A store without the trace or its meta
/// fails the test.
fn play(level: &SimLevel, root: &Path, clock: &str) -> Played {
    let dir = root.join("level8").join(STORE);
    let path = dir.join(format!("trace-{clock}-r1.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let reference = trace::parse(&text).unwrap();
    let (t0, seed) =
        time_zero(&dir, clock).unwrap_or_else(|| panic!("{STORE}: no time zero in its meta"));
    if clock == "c18" {
        assert_eq!(t0, T0_MS, "{STORE}: time zero");
    }
    let mut sim = at_time_zero(level, clock, t0, seed);
    let id = find(&sim, PIRATE_AT);
    let ticks = route_ticks();
    let end_ms = reference.last().map_or(0, |row| row.t_ms);
    let mut rows = Vec::new();
    let mut pirate = Vec::new();
    let mut k = 0usize;
    while (k as u64) * TICK < end_ms {
        sim.step(ticks.get(k).copied().unwrap_or_default());
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        pirate.push(sim.objects().get(id).map(|o| PirateAt {
            x: o.x,
            y: o.y,
            state: EnemyState::from_id(o.state),
        }));
        k += 1;
    }
    Played {
        rows,
        reference,
        pirate,
    }
}

/// Compares a play at `clock` with the reference trace at the clock's
/// tolerances (exact for c18, feel for c60), and at c18 its pirate with
/// the run's (`pirate`, when the run lists it); returns the divergences
/// not in [`KNOWN`].
fn new_divergences(
    clock: &str,
    played: &Played,
    pirate: Option<&[(usize, PirateAt)]>,
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
            "{STORE} {clock}: {} of {} rows compared",
            cmp.rows_compared,
            reference.len()
        ));
    }
    let mut firsts: Vec<(&str, u64)> = cmp
        .per_column
        .iter()
        .filter_map(|c| Some((c.column.as_str(), c.first_t_ms?)))
        .collect();
    if let Some(p) = pirate {
        firsts.extend(compare_pirate(&played.pirate, p));
    }
    let list = known(clock);
    for (column, first) in firsts {
        println!("first {STORE} {clock} {column} {first}");
        match list.iter().find(|(name, _)| *name == column) {
            Some(&(_, at)) if first >= at => {}
            _ => out.push(format!("{STORE} {clock}: {column} at {first} ms")),
        }
    }
    out
}

#[test]
fn the_route_matches_the_stores_copy() {
    let Some(root) = reference_root() else { return };
    let dir = root.join("level8").join(STORE);
    let rows = store_route(&dir).unwrap_or_else(|| panic!("{STORE}: no route in the store"));
    let ours: Vec<(u32, u32)> = route_ticks()
        .iter()
        .map(|t| (t.held.0, t.pressed.0))
        .collect();
    assert_eq!(ours.len(), rows.len(), "{STORE}: rows");
    let first = ours.iter().zip(&rows).position(|(a, b)| a != b);
    assert_eq!(first, None, "{STORE}: first different row");
}

#[test]
fn install_level8_the_pirate_route_follows_the_reference_traces() {
    let Some(level) = level8() else { return };
    let Some(root) = reference_root() else { return };
    let dir = root.join("level8").join(STORE);
    let meta = std::fs::read_to_string(dir.join("meta.toml"))
        .unwrap_or_else(|e| panic!("{STORE}: meta: {e}"));
    let runs = runs_root(&root);
    let mut failures = Vec::new();
    for clock in CLOCKS {
        println!("== level8/{STORE} {clock}");
        let played = play(&level, &root, clock);
        let pirate = runs
            .as_ref()
            .filter(|_| clock == "c18")
            .and_then(|r| Some(r.join(run_id(&meta, clock)?).join("obj-log.csv")))
            .and_then(|p| reference_pirate(&p));
        if clock == "c18" && pirate.is_none() {
            println!("the run's object log is not there: the pirate is not compared");
        }
        failures.extend(new_divergences(clock, &played, pirate.as_deref()));
    }
    assert!(failures.is_empty(), "new divergences: {failures:#?}");
}

// ------------------------------------------------------------ the route

/// What one row of the route shows: Claw and the pirate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    /// Claw's position, state and health.
    x: i32,
    state: i32,
    health: i32,
    /// The pirate's position, state and health.
    ex: i32,
    ey: i32,
    es: Option<EnemyState>,
    ehealth: i32,
}

/// Plays `rows` rows of the route at the 18 ms step from its time zero,
/// watching the pirate.
fn watch(level: &SimLevel, rows: usize) -> Vec<Look> {
    let mut sim = at_time_zero(level, "c18", T0_MS, SEED);
    let id = find(&sim, PIRATE_AT);
    let ticks = route_ticks();
    (0..rows)
        .map(|k| {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            let c = sim.player().expect("Claw");
            let e = sim.objects().get(id);
            Look {
                x: c.x,
                state: c.state,
                health: sim.claw().stats.health,
                ex: e.map_or(0, |o| o.x),
                ey: e.map_or(0, |o| o.y),
                es: e.and_then(|o| EnemyState::from_id(o.state)),
                ehealth: e.map_or(0, |o| o.health),
            }
        })
        .collect()
}

fn find(sim: &Sim, near: i32) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == "RedTailPirate")
        .min_by_key(|(_, o)| (o.x - near).abs())
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no pirate near {near}"))
}

/// The rows on which `f` holds and did not on the row before.
fn starts(rows: &[Look], f: impl Fn(&Look) -> bool) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| f(&rows[k]) && (k == 0 || !f(&rows[k - 1])))
        .collect()
}

/// The spans (first, last) of the rows on which the pirate is in `s`.
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

#[test]
fn install_level8_the_pirate_parries_a_blow_and_dies_from_the_next() {
    let Some(level) = level8() else { return };
    let rows = watch(&level, 200);
    // The strike running at time zero misses (Claw is still safe after
    // the blow before it); the next one reaches him after his Left tap.
    assert_eq!(rows[0].es, Some(EnemyState::Strike));
    assert_eq!(starts(&rows, |l| l.state == state::HIT), [23]);
    assert_eq!((rows[22].health, rows[23].health), (90, 80));
    // The first swing that reaches it lands (the roll 25): 5 of its 10,
    // a flinch of 19 rows.
    assert_eq!(spans(&rows, EnemyState::HitLow), [(62, 80)]);
    assert_eq!(rows[62].ehealth, 5);
    // The next (the roll 24) is parried: the high guard for 10 rows,
    // nothing off.
    assert_eq!(spans(&rows, EnemyState::BlockHigh), [(86, 95)]);
    assert!(rows[86..=96].iter().all(|l| l.ehealth == 5));
    assert_eq!(rows[96].es, Some(EnemyState::Home));
    // The third (the roll 77) kills it.
    let dead = starts(&rows, |l| l.es == Some(EnemyState::Dead));
    assert_eq!(dead, [110]);
    assert!(rows[110].ehealth <= 0);
    // Claw takes only the one strike.
    assert!(rows[23..].iter().all(|l| l.health == 80));
    assert!(rows[..62].iter().all(|l| l.ehealth == 10));
}
