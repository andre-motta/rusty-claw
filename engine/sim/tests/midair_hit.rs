//! The mid-air hit jump: a hit that ends while Claw is in the air leaves
//! him standing there by default, as in the original, so a jump press
//! starts a second jump. Any hit works: an enemy's shot or his own
//! dynamite. With the mid-air hit jump fix on he cannot jump again before
//! he lands.

mod common;

use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::object::{ObjectId, types};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::tuning::Behaviour;
use claw_sim::{Sim, SimConfig, TickInput, Tuning};
use common::*;

/// The flat level with Claw settled on the floor, with the fix `fix`, or
/// the default behaviour when `None`.
fn settled(fix: Option<bool>) -> Sim {
    let mut cfg = SimConfig::default();
    if let Some(fix) = fix {
        cfg.tuning = Tuning {
            behaviour: Behaviour {
                fix_double_jump: fix,
            },
            ..Tuning::default()
        };
    }
    let mut sim = Sim::new(flat_level(), cfg);
    idle(&mut sim, 30);
    let o = sim.player().unwrap();
    assert_eq!((o.state, o.y), (state::STAND, 899));
    sim
}

fn launch(sim: &mut Sim, spec: ProjectileSpec) -> ObjectId {
    sim.with_world(|w| projectile::spawn(w, spec))
        .expect("launched")
}

/// An enemy's musket ball where Claw is.
fn musket_ball(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "Bullet",
        x,
        y,
        z: 300,
        vx: -500,
        damage: 10,
        type_flags: types::ENEMY_SHOT,
        image_set: "LEVEL_MUSKETBALL",
        ..ProjectileSpec::default()
    }
}

/// Claw's own stick of dynamite, dropped at (x, y).
fn dynamite(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "Dynamite",
        x,
        y,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        ..ProjectileSpec::default()
    }
}

/// Runs idle ticks until the hit reaction is over; returns Claw's height
/// then. Claw must still be in the air.
fn until_the_hit_ends(sim: &mut Sim) -> i32 {
    for _ in 0..60 {
        if sim.player().unwrap().state != state::HIT {
            break;
        }
        sim.step(TickInput::default());
    }
    let o = sim.player().unwrap();
    assert_ne!(o.state, state::HIT, "the reaction ends");
    assert!(o.y < 899 - 10, "still in the air: y = {}", o.y);
    o.y
}

/// Jumps, and while Claw rises an enemy's shot hits him. Returns the sim
/// once the reaction is over.
fn shot_in_mid_air(fix: Option<bool>) -> Sim {
    let mut sim = settled(fix);
    sim.step(press(JUMP));
    for _ in 0..4 {
        sim.step(hold(JUMP));
    }
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    assert!(y < 899 - 20);
    let health = sim.stats().health;
    launch(&mut sim, musket_ball(x + 5, y));
    for _ in 0..5 {
        sim.step(TickInput::default());
        if sim.stats().health < health {
            break;
        }
    }
    assert_eq!(sim.stats().health, health - 10, "the shot hit");
    until_the_hit_ends(&mut sim);
    sim
}

/// The tick, counted from the drop, at which a stick of dynamite dropped
/// at Claw's feet hurts him while he stands.
fn dynamite_hurts_at() -> usize {
    let mut sim = settled(None);
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let health = sim.stats().health;
    launch(&mut sim, dynamite(x + 20, y + 40));
    for n in 1..200 {
        sim.step(TickInput::default());
        if sim.stats().health < health {
            return n;
        }
    }
    panic!("the dynamite never went off");
}

/// Drops a stick of dynamite at Claw's feet and jumps just before it goes
/// off, so its blast catches him in the air. Returns the sim once the
/// reaction is over.
fn own_dynamite_in_mid_air(fix: Option<bool>) -> Sim {
    let at = dynamite_hurts_at();
    let mut sim = settled(fix);
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let health = sim.stats().health;
    launch(&mut sim, dynamite(x + 20, y + 40));
    let jump_at = at - 4;
    for n in 1..at {
        sim.step(if n == jump_at {
            press(JUMP)
        } else {
            TickInput::default()
        });
    }
    sim.step(TickInput::default());
    assert_eq!(sim.stats().health, health - 15, "the blast hit");
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::HIT);
    assert_eq!(o.physics_type, ptype::FALL, "hit in the air");
    until_the_hit_ends(&mut sim);
    sim
}

/// Presses jump and checks that Claw jumps again from where he hangs.
fn jumps_again(sim: &mut Sim) {
    let o = sim.player().unwrap();
    assert_eq!(
        o.state,
        state::STAND,
        "standing in mid-air, as in the original"
    );
    let y = o.y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::JUMP, "a second jump");
    assert!(o.y < y, "he rises: {} from {y}", o.y);
}

/// Presses jump and checks that Claw only falls.
fn cannot_jump_again(sim: &mut Sim) {
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::LAND, "the fix lands him");
    let y = o.y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::FALL, "no second jump");
    assert!(o.y >= y, "he falls: {} from {y}", o.y);
}

#[test]
fn the_fix_is_off_by_default() {
    assert!(!Behaviour::default().fix_double_jump);
    assert!(!SimConfig::default().tuning.behaviour.fix_double_jump);
}

#[test]
fn a_shot_in_mid_air_lets_claw_jump_again_by_default() {
    jumps_again(&mut shot_in_mid_air(None));
    jumps_again(&mut shot_in_mid_air(Some(false)));
}

#[test]
fn a_shot_in_mid_air_with_the_fix_on_leaves_him_falling() {
    cannot_jump_again(&mut shot_in_mid_air(Some(true)));
}

#[test]
fn his_own_dynamite_in_mid_air_lets_claw_jump_again_by_default() {
    jumps_again(&mut own_dynamite_in_mid_air(None));
}

#[test]
fn his_own_dynamite_in_mid_air_with_the_fix_on_leaves_him_falling() {
    cannot_jump_again(&mut own_dynamite_in_mid_air(Some(true)));
}

#[test]
fn a_hit_on_the_ground_stands_either_way() {
    for fix in [None, Some(true)] {
        let mut sim = settled(fix);
        let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
        launch(&mut sim, musket_ball(x + 5, y));
        for _ in 0..60 {
            sim.step(TickInput::default());
            if sim.player().unwrap().state == state::STAND && sim.stats().health < 100 {
                break;
            }
        }
        let o = sim.player().unwrap();
        assert_eq!((o.state, o.y), (state::STAND, 899), "fix {fix:?}");
    }
}
