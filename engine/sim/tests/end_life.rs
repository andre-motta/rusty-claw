//! Ending a life from the in-game menu ([`Sim::end_life`]): the death
//! happens at the start of the next tick, from standing, from a ladder and
//! while carrying something; Claw respawns at the last checkpoint with a
//! life less, or the game ends after the last life. Synthetic levels built
//! in code, plus one case on the first level of the user's own install
//! (read from `CLAW_INSTALL` at run time, skipped when it is unset).

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{GameEvent, TickInput};
use claw_sim::anim::Animation;
use claw_sim::logic;
use claw_sim::logic::keg::{self, Phase};
use claw_sim::object::{ObjectId, special as sflags};
use claw_sim::physics::ptype;
use claw_sim::player::{death, special, state};
use claw_sim::{Carry, Sim, SimConfig, SimLevel, StartState};
use common::*;

const LIFT: u32 = 0x20;

/// A game with `lives` lives on `level`, left to settle for 30 ticks.
fn game(level: SimLevel, lives: i32) -> Sim {
    let start = StartState {
        carry: Some(Carry {
            lives,
            ..Carry::default()
        }),
        save: None,
    };
    let mut sim = Sim::with_start(level, SimConfig::default(), start);
    idle(&mut sim, 30);
    sim
}

fn claw(sim: &Sim) -> &claw_sim::object::Object {
    sim.player().expect("Claw")
}

/// Steps idle ticks until `event` is reported, at most `limit`; the tick
/// count it took.
fn until_event(sim: &mut Sim, event: &GameEvent, limit: usize) -> Option<usize> {
    (1..=limit).find(|_| sim.step(TickInput::default()).events.contains(event))
}

/// Ends the life and checks the next tick starts the timer death from where
/// Claw was, before anything else moves him.
fn end_and_check_the_fall(sim: &mut Sim) {
    let before = (claw(sim).x, claw(sim).y);
    sim.end_life();
    assert!(sim.end_life_pending());
    assert_ne!(claw(sim).state, state::DEATH_FALL, "not before the tick");
    sim.step(TickInput::default());
    assert!(!sim.end_life_pending());
    let c = claw(sim);
    assert_eq!(c.state, state::DEATH_FALL);
    assert_eq!(c.physics_type, ptype::FALL);
    assert_eq!(c.z, death::DEATH_FALL_Z);
    assert_eq!(c.x, before.0, "falls straight down from where he was");
    assert!(
        (0..=4).contains(&(c.y - before.1)),
        "the fall starts from rest: {} -> {}",
        before.1,
        c.y
    );
    assert_eq!(sim.stats().health, 0);
}

/// After a death with lives left: Claw comes back standing at `at` with
/// full health.
fn check_the_respawn(sim: &mut Sim, at: i32) {
    idle(sim, 200);
    let c = claw(sim);
    assert_eq!(c.state, state::STAND);
    assert_eq!(c.x, at);
    assert_eq!(sim.stats().health, 100);
    assert!(sim.snapshot().transition.is_none());
}

#[test]
fn ending_a_life_while_standing_respawns_at_the_checkpoint() {
    let mut sim = game(flat_level(), 3);
    let flag = (1400, 900);
    sim.set_checkpoint(Some(flag));
    assert_eq!(claw(&sim).state, state::STAND);
    end_and_check_the_fall(&mut sim);
    let died = until_event(&mut sim, &GameEvent::ClawDied { lives_left: 2 }, 120);
    assert!(died.is_some(), "the lives step ran once he had fallen");
    assert_eq!(sim.stats().lives, 2);
    assert_eq!(sim.stats().attempt, 1);
    check_the_respawn(&mut sim, flag.0);
    assert_eq!(sim.stats().lives, 2);
}

#[test]
fn ending_a_life_twice_or_while_dying_takes_one_life() {
    let mut sim = game(flat_level(), 3);
    sim.end_life();
    sim.end_life();
    sim.step(TickInput::default());
    idle(&mut sim, 5);
    // Already dying: nothing more happens.
    sim.end_life();
    let mut deaths = 0;
    for _ in 0..300 {
        let snap = sim.step(TickInput::default());
        deaths += snap
            .events
            .iter()
            .filter(|e| matches!(e, GameEvent::ClawDied { .. }))
            .count();
    }
    assert_eq!(deaths, 1);
    assert_eq!(sim.stats().lives, 2);
    assert_eq!(claw(&sim).state, state::STAND);
}

#[test]
fn ending_a_life_on_a_ladder_falls_from_the_ladder() {
    let mut sim = game(ladder_level(), 3);
    for &input in &script(&[(UP | JUMP, 108), (UP, 300)]).ticks {
        sim.step(input);
    }
    assert_eq!(claw(&sim).state, state::CLIMB);
    // Still holding up: the ladder does not keep him.
    let before = (claw(&sim).x, claw(&sim).y);
    sim.end_life();
    sim.step(TickInput {
        held: claw_sim::Buttons(UP),
        pressed: claw_sim::Buttons::NONE,
    });
    let c = claw(&sim);
    assert_eq!(c.state, state::DEATH_FALL);
    assert_eq!(c.physics_type, ptype::FALL);
    assert_eq!(c.x, before.0);
    // He hangs there for 750 ms, falls off the screen through the floor,
    // then comes back at the level start (no checkpoint yet).
    let mut lowest = c.y;
    for _ in 0..90 {
        sim.step(TickInput::default());
        lowest = lowest.max(claw(&sim).y);
    }
    assert!(lowest > 960, "fell through the floor: {lowest}");
    let died = until_event(&mut sim, &GameEvent::ClawDied { lives_left: 2 }, 120);
    assert!(died.is_some());
    let start = sim.claw().spawn_point.0;
    check_the_respawn(&mut sim, start);
}

/// Claw's lift and carry animations, so that he can carry a keg.
fn carry_level() -> SimLevel {
    let mut l = flat_level();
    let anim = |key: &str, frames: &[(i32, i32)], once: bool| {
        Arc::new(if once {
            Animation::once(key, frames)
        } else {
            Animation::looping(key, frames)
        })
    };
    for a in [
        anim(special::LIFT_ANIM, &[(40, 100), (42, 100), (43, 100)], true),
        anim(
            special::LIFT_LOW_ANIM,
            &[(41, 100), (42, 100), (43, 100)],
            true,
        ),
        anim(special::CARRY_ANIM, &[(43, 1000)], false),
        anim(special::CARRY_WALK_ANIM, &[(45, 100), (46, 100)], false),
        anim("CLAW_STAND", &[(11, 110), (12, 110)], false),
    ] {
        l.animations.insert(a.key.clone(), a);
    }
    l
}

fn keg_beside(sim: &mut Sim, dx: i32) -> ObjectId {
    let (x, y) = (claw(sim).x, claw(sim).y);
    // Claw's feet are 60 px below his position, a keg's 69 px below its.
    let id = sim.spawn(logic::new_object("PowderKeg", x + dx, y + 60 - 69, 4000));
    sim.step(TickInput::default());
    assert!(sim.objects().get(id).expect("keg").special_flags & sflags::LIFTABLE != 0);
    id
}

#[test]
fn ending_a_life_while_carrying_lets_go_of_the_load() {
    let mut sim = game(carry_level(), 3);
    let id = keg_beside(&mut sim, 40);
    sim.step(press(LIFT));
    idle(&mut sim, 20);
    assert!(sim.claw().special.carrying);
    assert_eq!(sim.claw().special.lifted, Some(id));
    end_and_check_the_fall(&mut sim);
    assert_eq!(sim.claw().special.lifted, None);
    assert!(!sim.claw().special.carrying);
    let phase = keg::data_of(sim.objects().get(id).expect("keg"))
        .expect("keg data")
        .phase;
    assert_ne!(phase, Phase::Lifted, "the keg is let go");
    let died = until_event(&mut sim, &GameEvent::ClawDied { lives_left: 2 }, 120);
    assert!(died.is_some());
    let start = sim.claw().spawn_point.0;
    check_the_respawn(&mut sim, start);
    assert!(!sim.claw().special.carrying);
}

#[test]
fn ending_the_last_life_ends_the_game() {
    let mut sim = game(flat_level(), 1);
    end_and_check_the_fall(&mut sim);
    let mut events = Vec::new();
    for _ in 0..400 {
        events.extend(sim.step(TickInput::default()).events);
    }
    assert_eq!(events, vec![GameEvent::GameOver]);
    assert_eq!(sim.stats().lives, 0);
    assert_eq!(sim.stats().health, 0);
    assert_eq!(claw(&sim).state, state::DEATH_FALL, "he stays dead");
}

#[test]
fn a_game_with_an_end_of_life_waiting_replays_from_a_clone() {
    // The request is part of the game's state: a copy made before the
    // tick dies the same way.
    let mut a = game(flat_level(), 3);
    a.end_life();
    let mut b = a.clone();
    for _ in 0..150 {
        let sa = a.step(TickInput::default());
        let sb = b.step(TickInput::default());
        assert_eq!(sa.events, sb.events);
    }
    assert_eq!(claw(&a).y, claw(&b).y);
    assert_eq!(a.rng_state(), b.rng_state());
}

// ------------------------------------------------------------ install

fn install_level1() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    let store = install.store().expect("mounting the install");
    let data = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

#[test]
fn install_ending_a_life_on_level1_respawns_at_the_last_checkpoint() {
    let Some(l) = install_level1() else { return };
    let (sx, sy) = l.start;
    let flag = l
        .objects
        .iter()
        .filter(|o| o.logic == "Checkpoint")
        .map(|o| (o.record[5], o.record[6]))
        .min_by_key(|&(x, y)| {
            let (dx, dy) = (i64::from(x - sx), i64::from(y - sy));
            dx * dx + dy * dy
        })
        .expect("a checkpoint flag");
    let mut sim = Sim::new(l, SimConfig::default());
    sim.teleport_player(flag.0 - 10, flag.1);
    let mut events = Vec::new();
    for _ in 0..120 {
        events.extend(sim.step(TickInput::default()).events);
    }
    let point = (flag.0, flag.1 - 32);
    assert!(
        events.contains(&GameEvent::Checkpoint {
            x: point.0,
            y: point.1
        }),
        "{events:?}"
    );
    // Walk away from the flag, then give up the life.
    for _ in 0..60 {
        sim.step(hold(RIGHT));
    }
    idle(&mut sim, 20);
    assert_ne!(claw(&sim).x, flag.0);
    end_and_check_the_fall(&mut sim);
    let died = until_event(&mut sim, &GameEvent::ClawDied { lives_left: 6 }, 200);
    assert!(died.is_some(), "the lives step ran");
    assert_eq!(sim.stats().attempt, 1);
    idle(&mut sim, 200);
    let o = claw(&sim);
    assert_eq!(o.state, state::STAND);
    assert_eq!(o.x, flag.0);
    assert!(
        (o.y - point.1).abs() <= 64,
        "on the ground by the flag: {}",
        o.y
    );
    assert_eq!(sim.stats().health, 100);
    assert_eq!(sim.stats().lives, 6);
}
