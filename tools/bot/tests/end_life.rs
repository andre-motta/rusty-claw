//! A route's own end life (the in-game menu's End Life, an `@T command
//! end_life` line) is no death to the bot: the life it gives up, while
//! Claw dies and comes back, is the route's technique. A life lost any
//! other way is a death.

use claw_bot::Goal;
use claw_bot::route::{self, Deaths};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TileMap};

/// An open room 20 tiles wide with a floor, Claw standing on it.
fn start() -> Sim {
    let mut rows = vec![".".repeat(20); 9];
    rows.push("#".repeat(20));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (5 * 64, 8 * 64));
    Sim::new(level, SimConfig::default())
}

/// `ticks` ticks of nothing held, with an end life before tick `at`.
fn script(ticks: usize, at: Option<usize>) -> InputScript {
    let mut text = String::from("tick,held_mask,pressed_mask\n");
    if let Some(t) = at {
        text.push_str(&format!("@{t} command end_life\n"));
    }
    for t in 0..ticks {
        text.push_str(&format!("{t},0x0,0x0\n"));
    }
    InputScript::parse_with_aids(&text).expect("the script parses")
}

/// The tick Claw is back after an end life before tick `at`, and his
/// lives then.
fn back_after(start: &Sim, at: usize) -> (usize, i32) {
    let s = script(at + 1000, Some(at));
    let mut sim = start.clone();
    let lives = sim.stats().lives;
    let mut taken = None;
    for (i, &t) in s.ticks.iter().enumerate() {
        route::play_directives(&mut sim, &s, i);
        sim.step(t);
        if taken.is_none() && sim.stats().lives < lives {
            taken = Some(i);
        }
        if taken.is_some() && !claw_bot::search::dying(&sim) {
            return (i, sim.stats().lives);
        }
    }
    panic!("Claw never comes back");
}

#[test]
fn the_routes_own_end_life_is_no_death() {
    let start = start();
    let lives = start.stats().lives;
    let (back, left) = back_after(&start, 10);
    assert_eq!(left, lives - 1, "one life given up");
    // Played to a few ticks past his return: no death.
    let s = script(back + 5, Some(10));
    let goal = Goal::Reach {
        x: 0,
        y: 0,
        radius: 0,
    };
    let played = route::play(&start, &s, &goal);
    assert!(!played.died, "{played:?}");
}

#[test]
fn a_life_lost_without_the_command_is_a_death() {
    let start = start();
    let (back, _) = back_after(&start, 10);
    let s = script(back + 5, Some(10));
    // Counted as if the route held no end life: the same loss is a death.
    let mut sim = start.clone();
    let mut deaths = Deaths::new(&sim);
    let mut died = false;
    for (i, &t) in s.ticks.iter().enumerate() {
        route::play_directives(&mut sim, &s, i);
        sim.step(t);
        died |= deaths.died(&sim);
    }
    assert!(died);
}

#[test]
fn claw_dying_by_the_end_life_is_no_death_yet() {
    let start = start();
    let s = script(20, Some(10));
    let mut sim = start.clone();
    let mut deaths = Deaths::new(&sim);
    let mut died = false;
    for (i, &t) in s.ticks.iter().enumerate() {
        deaths.before_tick(&s, i);
        route::play_directives(&mut sim, &s, i);
        sim.step(t);
        died |= deaths.died(&sim);
    }
    // Ten ticks into the end life Claw is dying, but the dying is the end
    // life's own: no death yet.
    assert!(claw_bot::search::dying(&sim));
    assert!(!died);
}
