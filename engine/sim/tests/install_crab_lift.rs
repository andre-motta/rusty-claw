//! Lifting a hiding hermit crab on the seventh level of your own install,
//! against the original's runs of the same route (the `level7/crab-lift`
//! reference store).
//!
//! The route is the store's own (`route.csv`): Claw is put down at the
//! store's start point (`teleport` in its `meta.toml`) after the level's
//! first tick, between two ticks as a warp does, waits until the level
//! clock reaches the store's time zero, and the shared random stream
//! starts from the store's state there, once. He walks up to a crab,
//! waits for it to hide, lifts it, carries it and throws it. The game is
//! then played tick by tick and compared with every 18 ms trace of the
//! store with the original's speeds: Claw's position, state and health
//! exactly, row for row (the other columns are reported). Both the game
//! and the original must lift (Claw's lift state) and throw (his throw
//! state) on the same rows.
//!
//! Needs `CLAW_INSTALL` and `CLAW_REFERENCE`; the test skips itself when
//! either is unset. With `CLAW_REFERENCE` set, a missing store, route,
//! meta or trace fails with a message naming what is missing. Nothing
//! from an install or a reference store is kept in the repository.

mod common;

use std::path::{Path, PathBuf};

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::input::ScriptTuning;
use claw_sim::logic::enemy::redtail_pirate;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, Tolerances, TraceRow};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The store, under the reference folder.
const STORE: &str = "level7/crab-lift";
/// The clock compared exactly.
const CLOCK: &str = "c18";

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

/// The store's folder; fails when the reference folder lacks it.
fn store_dir(root: &Path) -> PathBuf {
    let dir = root.join(STORE);
    assert!(
        dir.join("meta.toml").is_file() && dir.join("route.csv").is_file(),
        "{STORE}: the reference store is missing under {} (its meta.toml and route.csv \
         are needed); record it, or point CLAW_REFERENCE at a folder that has it",
        root.display()
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
            return Some(v.trim().to_string());
        }
    }
    None
}

/// What the store's meta says about the clock's runs: the start point,
/// the time zero (ms), the random state there, and the traces.
struct Meta {
    start: (i32, i32),
    t0_ms: u64,
    seed: u32,
    traces: Vec<String>,
}

fn meta(dir: &Path) -> Meta {
    let text = std::fs::read_to_string(dir.join("meta.toml"))
        .unwrap_or_else(|e| panic!("{STORE}: no meta.toml: {e}"));
    let get = |section: &str, key: &str| {
        meta_value(&text, section, key).or_else(|| meta_value(&text, "", key))
    };
    let unquote = |v: String| v.trim_matches('"').to_string();
    let teleport = get(CLOCK, "teleport")
        .map(unquote)
        .unwrap_or_else(|| panic!("{STORE}: no teleport in meta.toml"));
    let (x, y) = teleport
        .split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
        .unwrap_or_else(|| panic!("{STORE}: teleport {teleport:?} is not X,Y"));
    let t0_ms = get(CLOCK, "t0_ms")
        .and_then(|v| unquote(v).parse().ok())
        .unwrap_or_else(|| panic!("{STORE}: no t0_ms for {CLOCK}"));
    let seed = get(CLOCK, "rng_at_t0")
        .and_then(|v| unquote(v).parse().ok())
        .unwrap_or_else(|| panic!("{STORE}: no rng_at_t0"));
    let traces: Vec<String> = meta_value(&text, CLOCK, "traces")
        .map(|v| {
            v.split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !traces.is_empty(),
        "{STORE}: no {CLOCK} traces in meta.toml"
    );
    Meta {
        start: (x, y),
        t0_ms,
        seed,
        traces,
    }
}

/// The store's route: held and pressed buttons per row.
fn route(dir: &Path) -> Vec<TickInput> {
    let path = dir.join("route.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{STORE}: {e}"));
    let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).unwrap();
    text.lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            TickInput {
                held: Buttons(hex(f[1])),
                pressed: Buttons(hex(f[2])),
            }
        })
        .collect()
}

/// Draws the original makes at the level's start, before the route's
/// warp, that this game does not make from the same stream: the level's
/// timed sounds take their first timers from it (this game draws them
/// while the level loads), and the red-tail pirate by the level's start
/// goes idle and idles twice (four draws, two of which it makes itself
/// here on the first step when it is built).
fn level_start_draws() -> u32 {
    const TIMED_SOUNDS: u32 = 3;
    const PIRATE_BY_THE_START: u32 = 4;
    const PIRATE_OWN_DRAWS: u32 = 2;
    TIMED_SOUNDS
        + if redtail_pirate::BUILT {
            PIRATE_BY_THE_START - PIRATE_OWN_DRAWS
        } else {
            PIRATE_BY_THE_START
        }
}

/// Where the red-tail pirate by the level's start stands.
const PIRATE_BY_THE_START_X: i32 = 1924;

/// The game at the route's time zero (see the module documentation).
fn at_time_zero(level: &SimLevel, m: &Meta) -> Sim {
    let cfg = SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        seed: m.seed,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level.clone(), cfg);
    let pirate = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "RedTailPirate")
        .min_by_key(|(_, o)| (o.x - PIRATE_BY_THE_START_X).abs())
        .map(|(id, _)| id);
    sim.step(TickInput::default());
    let start = m.start;
    sim.with_world(|w| {
        // The original's runs put Claw down before the leave pass of the
        // step, so the pirate by the level's start never runs after the
        // warp; this game's warp comes after that pass.
        if redtail_pirate::BUILT
            && let Some(id) = pirate
        {
            w.objects.unlink(id);
        }
        w.claw.special.warp_to = Some(start);
        for _ in 0..level_start_draws() {
            w.rng.next_u15();
        }
    });
    while sim.tick() * TICK < m.t0_ms {
        sim.step(TickInput::default());
    }
    let seed = m.seed;
    sim.with_world(|w| *w.rng = Rng::new(seed));
    sim
}

/// The route played for `rows` rows.
fn play(level: &SimLevel, m: &Meta, ticks: &[TickInput], rows: usize) -> Vec<TraceRow> {
    let mut sim = at_time_zero(level, m);
    (0..rows)
        .map(|k| {
            sim.step(ticks.get(k).copied().unwrap_or_default());
            TraceRow::capture(&sim, k as u64, "script")
        })
        .collect()
}

/// The rows on which Claw enters state `s`.
fn entries(rows: &[TraceRow], s: i32) -> Vec<usize> {
    (0..rows.len())
        .filter(|&k| rows[k].state_id == s && (k == 0 || rows[k - 1].state_id != s))
        .collect()
}

/// Columns compared exactly; the others are reported.
fn gated(column: &str) -> bool {
    matches!(column, "x" | "y" | "state_id" | "health")
}

#[test]
#[ignore = "the level7/crab-lift reference runs are not recorded yet"]
fn install_level7_lifting_a_hiding_crab_follows_the_reference_traces() {
    let Some(level) = level7() else { return };
    let Some(root) = reference_root() else { return };
    let dir = store_dir(&root);
    let m = meta(&dir);
    let ticks = route(&dir);
    let mut failures = Vec::new();
    for name in &m.traces {
        println!("== {STORE} {CLOCK} {name}");
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{STORE}: trace {name} is missing: {e}"));
        let reference = trace::parse(&text).unwrap();
        let ours = play(&level, &m, &ticks, reference.len());
        let cmp = trace::compare(&ours, &reference, &Tolerances::exact());
        print!("{}", trace::report(&ours, &reference, &cmp, 3));
        if cmp.rows_compared != reference.len() {
            failures.push(format!(
                "{name}: {} of {} rows compared",
                cmp.rows_compared,
                reference.len()
            ));
        }
        for c in &cmp.per_column {
            if let Some(t) = c.first_t_ms
                && gated(&c.column)
            {
                failures.push(format!(
                    "{name}: {} leaves the original at {t} ms",
                    c.column
                ));
            }
        }
        // The lift and the throw on the original's rows.
        for (what, s) in [("lift", state::LIFT), ("throw", state::THROW)] {
            let theirs = entries(&reference, s);
            assert!(
                !theirs.is_empty(),
                "{name}: the original's run never enters the {what}"
            );
            let mine = entries(&ours, s);
            if mine != theirs {
                failures.push(format!(
                    "{name}: the {what} starts on rows {mine:?}, the original's on {theirs:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{STORE}: lifting a hiding crab leaves the original: {failures:#?}"
    );
}
