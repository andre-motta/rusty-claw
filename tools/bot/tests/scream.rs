//! The search under a Siren's scream, on a synthetic floor: while the
//! scream's lockout has taken Claw's controls only the wait and the walks
//! are offered, a wait through the lockout is progress (the time tells
//! its states apart), and a search that starts locked waits it out and
//! reaches its goal.

use claw_bot::actions::{self, Action};
use claw_bot::search::state_key;
use claw_bot::{Goal, SearchConfig, search_report};
use claw_sim::logic::projectile::scream;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

/// A flat floor 40 tiles wide, Claw standing on it once settled.
fn floor() -> Sim {
    let mut rows = vec![".".repeat(40); 9];
    rows.push("#".repeat(40));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (5 * 64, 7 * 64));
    let mut sim = Sim::new(level, SimConfig::default());
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    assert!(claw_bot::search::on_foot(&sim), "Claw stands on the floor");
    sim
}

/// The game of `sim` with a scream's lockout just set.
fn screamed(sim: &Sim) -> Sim {
    let mut locked = sim.clone();
    locked.with_world(|w| w.set_scream_lock_ms(scream::LOCK_MS));
    locked
}

#[test]
fn a_locked_claw_is_offered_the_wait_and_the_walks_only() {
    let free = floor();
    assert!(!actions::locked(&free));
    let offered = actions::choices(&free);
    assert!(
        offered.len() > 3,
        "on his feet Claw jumps, strikes and more"
    );
    let locked = screamed(&free);
    assert!(actions::locked(&locked));
    assert_eq!(
        actions::choices(&locked),
        vec![Action::Wait, Action::Left, Action::Right]
    );
}

#[test]
fn a_wait_through_the_lockout_is_progress() {
    let mut sim = screamed(&floor());
    let mut keys = vec![state_key(&sim)];
    while actions::locked(&sim) {
        for _ in 0..actions::STEP {
            sim.step(TickInput::default());
        }
        keys.push(state_key(&sim));
    }
    // Claw stands still all along, yet every wait gives another state.
    let mut unique = keys.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "{keys:?}");
    assert!(keys.len() > 10, "the lockout lasts two seconds");
}

#[test]
fn a_search_that_starts_locked_waits_it_out_and_reaches_its_goal() {
    let start = screamed(&floor());
    let goal = Goal::Reach {
        x: 15 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let cfg = SearchConfig {
        nodes: 4_000,
        threads: 2,
        ..SearchConfig::default()
    };
    let found = search_report(&start, goal.clone(), &cfg).expect("a route");
    let played = claw_bot::route::play(&start, &found.script, &goal);
    assert!(played.won(), "{played:?}");
    assert_eq!(found.stats.deaths, 0);
    // No key acts before the lockout has run down.
    let lock_ticks = scream::LOCK_MS.div_ceil(claw_sim::tuning::TICK_MS) as usize;
    assert!(found.script.ticks.len() > lock_ticks);
}
