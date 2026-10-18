//! How the game's state grows along a level, on your own install (skipped
//! without `CLAW_INSTALL`): the numbers that show the object container
//! stays small by reusing the slots of removed objects, and that decide
//! whether the activation grids keep a cell for every place of the level.
//!
//! Every committed original-speeds route (`routes/level<N>-original.csv`)
//! is played. Until the third level has one, the search bot plays it from
//! the start toward the farthest pickup its coarse graph reaches (its
//! ropes and path elevators do not run yet, so it gets only part of the
//! way), and the level is also left idle as long as the first level's
//! route. For each run the test prints:
//!
//! - the object slots at the end against the level's records (an object
//!   made while the level runs takes the slot of one that went before,
//!   when there is one, so the slots follow the most objects at once
//!   rather than every object made: one catnip alone sheds a sparkle a
//!   tick for as long as it lasts);
//! - the bytes one copy of the whole game allocates (the copy the search
//!   bot makes of every game it keeps) and the time it takes, at the
//!   start and at the end;
//! - the 99th percentile of the time of one tick along the run;
//! - the search bot's expanded nodes per second from the level's start
//!   with the same node budget on every level.
//!
//! It holds the rules those decisions were taken on: slots at the end at
//! most twice the records, a copy at the end at most twice the bytes of
//! one at the start. The third level's node rate against the first level's (the grids stay
//! dense while it is at least half) is printed, not asserted: the rates are
//! wall-clock numbers that move with the build profile and the machine's
//! load, so read them from a release build run alone.
//!
//! Nothing from an install is stored in the repository.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::field::{Grid, UNREACHABLE};
use claw_bot::route::{self, TuningName};
use claw_bot::{Goal, SearchConfig, search_report};
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// Counts the bytes the current thread allocates.
struct Counting;

thread_local! {
    static ALLOCATED: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: every call goes straight to the system allocator; the counter
// is a const-initialised thread-local that never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATED.try_with(|c| c.set(c.get() + layout.size() as u64));
        // SAFETY: the caller's layout is passed on unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let grown = new_size.saturating_sub(layout.size()) as u64;
        let _ = ALLOCATED.try_with(|c| c.set(c.get() + grown));
        // SAFETY: as for `alloc` and `dealloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Copies timed per measurement.
const CLONES: u32 = 20;

/// Node budget of the node-rate measurement, the same on every level.
const RATE_NODES: u64 = 1500;

/// Node budget of the third level's bot run.
const LEVEL3_NODES: u64 = 20_000;

/// Ticks the third level is left idle when the first level has no route.
const IDLE_TICKS: usize = 6000;

/// Distance from a pickup that counts as reaching it, px.
const REACH_RADIUS: i32 = 64;

/// The state of the shared random stream at the third level's time zero
/// when no reference run names one: the default seed.
const LEVEL3_SEED: u32 = 1;

/// One copy of the game: the bytes it allocated and the mean time over
/// [`CLONES`] copies.
fn clone_cost(sim: &Sim) -> (u64, Duration) {
    let before = ALLOCATED.with(Cell::get);
    let copy = sim.clone();
    let bytes = ALLOCATED.with(Cell::get) - before;
    drop(copy);
    let t = Instant::now();
    for _ in 0..CLONES {
        std::hint::black_box(sim.clone());
    }
    (bytes, t.elapsed() / CLONES)
}

/// What one run measured.
#[derive(Debug)]
struct Run {
    name: String,
    records: usize,
    ticks: usize,
    slots_start: usize,
    slots_end: usize,
    live_end: usize,
    clone_start: (u64, Duration),
    clone_end: (u64, Duration),
    tick_p99: Duration,
}

fn level(store: &claw_assets::AssetStore, n: u32) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// Plays `script` from `start`, measuring along the way.
fn measure(name: &str, start: &Sim, script: &InputScript) -> Run {
    let mut sim = start.clone();
    let clone_start = clone_cost(&sim);
    let slots_start = sim.objects().slots();
    let mut times = Vec::with_capacity(script.ticks.len());
    for &t in &script.ticks {
        let at = Instant::now();
        sim.step(t);
        times.push(at.elapsed());
    }
    times.sort_unstable();
    let tick_p99 = times
        .get((times.len() * 99 / 100).min(times.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    Run {
        name: name.to_string(),
        records: sim.level().objects.len(),
        ticks: script.ticks.len(),
        slots_start,
        slots_end: sim.objects().slots(),
        live_end: sim.objects().len(),
        clone_start,
        clone_end: clone_cost(&sim),
        tick_p99,
    }
}

/// A committed route's script and the game it starts from.
fn route_start(store: &claw_assets::AssetStore, file: &Path) -> (Sim, InputScript) {
    let text = std::fs::read_to_string(file).unwrap();
    let key = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("# {k} ")))
            .map(str::trim)
            .unwrap_or_else(|| panic!("{} names its {k}", file.display()))
            .to_string()
    };
    let n: u32 = key("level").parse().unwrap();
    let tuning = TuningName::parse(&key("tuning")).unwrap();
    let seed: u32 = key("seed").parse().unwrap();
    let script = InputScript::parse(&text).unwrap();
    let sim = Sim::new(level(store, n), route::sim_config(tuning, seed));
    (sim, script)
}

/// Expanded nodes per second of a search from `start` with
/// [`RATE_NODES`] nodes.
fn node_rate(start: &Sim) -> (u64, f64) {
    let cfg = SearchConfig {
        nodes: RATE_NODES,
        threads: 1,
        ..SearchConfig::default()
    };
    let t = Instant::now();
    let expanded = match search_report(start, Goal::LevelComplete, &cfg) {
        Ok(f) => f.stats.expanded,
        Err(e) => e.stats.expanded,
    };
    (expanded, expanded as f64 / t.elapsed().as_secs_f64())
}

fn kb(b: u64) -> f64 {
    b as f64 / 1024.0
}

fn us(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

/// The third level without a route: the bot's way from the start toward
/// the farthest pickup (in a straight line) that the coarse graph says it
/// can reach, and, when that search runs out, its way to the farthest
/// point that search got to.
fn level3_bot_run(start: &Sim) -> Option<Run> {
    let cfg = SearchConfig {
        nodes: LEVEL3_NODES,
        threads: 4,
        ..SearchConfig::default()
    };
    let claw = start.player().expect("Claw");
    let (sx, sy, feet) = (claw.x, claw.y, claw.move_rect.bottom);
    let grid = Grid::build(start.level(), cfg.reach);
    let mut pickups: Vec<(i32, i32)> = start
        .level()
        .objects
        .iter()
        .filter(|o| o.logic.ends_with("Powerup"))
        .map(|o| (o.record[5], o.record[6]))
        .collect();
    pickups.sort_by_key(|&(x, y)| std::cmp::Reverse((x - sx).abs() + (y - sy).abs()));
    let mut target = pickups
        .into_iter()
        .find(|&(x, y)| grid.field_to(x, y + feet, REACH_RADIUS).at(sx, sy) != UNREACHABLE)
        .expect("a pickup of the third level is reachable");
    for _ in 0..2 {
        let goal = Goal::Reach {
            x: target.0,
            y: target.1,
            radius: REACH_RADIUS,
        };
        match search_report(start, goal, &cfg) {
            Ok(found) => {
                println!(
                    "LEVEL3 bot run to {target:?}: {} ticks, {} nodes",
                    found.script.ticks.len(),
                    found.stats.expanded
                );
                return Some(measure("LEVEL3 bot run", start, &found.script));
            }
            Err(e) => {
                println!("LEVEL3 bot run to {target:?}: none ({e})");
                target = e.stats.best_at;
            }
        }
    }
    None
}

#[test]
fn install_scale_slots_copies_ticks_and_node_rates() {
    let Some(root) = install() else { return };
    let store = Install::open(&root).unwrap().store().unwrap();
    let routes = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("routes");
    let mut runs = Vec::new();
    for n in 1..=14 {
        let file = routes.join(format!("level{n}-original.csv"));
        if file.exists() {
            let (sim, script) = route_start(&store, &file);
            runs.push(measure(&format!("LEVEL{n} route"), &sim, &script));
        }
    }

    let start3 = Sim::new(
        level(&store, 3),
        route::sim_config(TuningName::Original, LEVEL3_SEED),
    );
    if !runs.iter().any(|r| r.name == "LEVEL3 route")
        && let Some(run) = level3_bot_run(&start3)
    {
        runs.push(run);
    }
    // The third level idle for as long as the first level's route: its
    // spawns without Claw's.
    let idle = InputScript {
        ticks: vec![TickInput::default(); runs.first().map_or(IDLE_TICKS, |r| r.ticks)],
        ..InputScript::default()
    };
    runs.push(measure("LEVEL3 idle", &start3, &idle));

    println!(
        "{:<16} {:>7} {:>6} {:>11} {:>9} {:>6} {:>22} {:>22} {:>9}",
        "run",
        "records",
        "ticks",
        "slots",
        "slots/rec",
        "live",
        "copy start KiB / us",
        "copy end KiB / us",
        "tick p99"
    );
    for r in &runs {
        println!(
            "{:<16} {:>7} {:>6} {:>5}->{:<5} {:>9.2} {:>6} {:>12.0} / {:>7.0} {:>12.0} / {:>7.0} {:>6.0} us",
            r.name,
            r.records,
            r.ticks,
            r.slots_start,
            r.slots_end,
            r.slots_end as f64 / r.records as f64,
            r.live_end,
            kb(r.clone_start.0),
            us(r.clone_start.1),
            kb(r.clone_end.0),
            us(r.clone_end.1),
            us(r.tick_p99),
        );
    }

    let mut rates = Vec::new();
    for n in [1u32, 2, 3] {
        let sim = Sim::new(
            level(&store, n),
            route::sim_config(TuningName::Original, LEVEL3_SEED),
        );
        let (nodes, rate) = node_rate(&sim);
        println!("LEVEL{n} search: {nodes} nodes, {rate:.0} nodes/s");
        rates.push(rate);
    }

    for r in &runs {
        assert!(
            r.slots_end <= 2 * r.records,
            "{}: {} slots at the end for {} records",
            r.name,
            r.slots_end,
            r.records
        );
        assert!(
            r.clone_end.0 <= 2 * r.clone_start.0,
            "{}: a copy at the end allocates {} bytes, at the start {}",
            r.name,
            r.clone_end.0,
            r.clone_start.0
        );
    }
    println!(
        "LEVEL3 node rate is {:.2} of LEVEL1's (dense grids while at least 0.5, in a release build run alone)",
        rates[2] / rates[0]
    );
}
