//! The mid-air hit jump on the first level of your own install (skipped
//! without `CLAW_INSTALL`), with the level's own animations: a soldier's
//! musket ball or Claw's own dynamite hits him in the air, and once the
//! hit reaction is over a jump press starts a second jump by default, as
//! in the original, and not with the mid-air hit jump fix on.

use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{Buttons, TickInput};
use claw_sim::logic::enemy::soldier;
use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::object::{ObjectId, types};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel};

const JUMP: u32 = 0x1;

fn level1() -> Option<Arc<SimLevel>> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(Arc::new(
        SimLevel::from_level(&level, &store).expect("level 1 prepares"),
    ))
}

fn press(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask),
    }
}

fn hold(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons::NONE,
    }
}

/// Claw settled at the level's start, with the fix on or off.
fn settled(level: &Arc<SimLevel>, fix: bool) -> Sim {
    let mut cfg = SimConfig::default();
    cfg.tuning.behaviour.fix_double_jump = fix;
    let mut sim = Sim::new(level.clone(), cfg);
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.player().unwrap().state, state::STAND);
    sim
}

fn launch(sim: &mut Sim, spec: ProjectileSpec) -> ObjectId {
    sim.with_world(|w| projectile::spawn(w, spec))
        .expect("launched")
}

/// A soldier's musket ball flying at Claw from just in front of him.
fn musket_ball(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: soldier::BALL_LOGIC,
        x: x + 20,
        y,
        z: 300,
        vx: -soldier::BALL_SPEED,
        damage: soldier::BALL_DAMAGE,
        type_flags: types::ENEMY_SHOT,
        image_set: soldier::BALL_IMAGE,
        ..ProjectileSpec::default()
    }
}

/// Claw's own stick of dynamite, dropped beside him.
fn dynamite(x: i32, y: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "Dynamite",
        x: x + 20,
        y: y + 40,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        ..ProjectileSpec::default()
    }
}

/// Runs until the hit reaction that has just started is over; Claw must
/// have stayed in the air.
fn through_the_reaction(sim: &mut Sim, ground: i32) {
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::HIT);
    assert_eq!(o.physics_type, ptype::FALL, "hit in the air");
    for _ in 0..200 {
        if sim.player().unwrap().state != state::HIT {
            break;
        }
        sim.step(TickInput::default());
    }
    let o = sim.player().unwrap();
    assert_ne!(o.state, state::HIT, "the reaction ends");
    assert!(o.y < ground - 10, "still in the air: {} over {ground}", o.y);
}

/// Jumps, and near the top an enemy's shot hits Claw.
fn shot_in_mid_air(level: &Arc<SimLevel>, fix: bool) -> Sim {
    let mut sim = settled(level, fix);
    let ground = sim.player().unwrap().y;
    sim.step(press(JUMP));
    for _ in 0..8 {
        sim.step(hold(JUMP));
    }
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let health = sim.stats().health;
    launch(&mut sim, musket_ball(x, y));
    for _ in 0..10 {
        sim.step(TickInput::default());
        if sim.stats().health < health {
            break;
        }
    }
    assert_eq!(sim.stats().health, health - soldier::BALL_DAMAGE);
    through_the_reaction(&mut sim, ground);
    sim
}

/// Drops a stick of dynamite beside Claw and jumps just before it goes
/// off, so the blast catches him in the air.
fn own_dynamite_in_mid_air(level: &Arc<SimLevel>, fix: bool) -> Sim {
    // When the blast hurts a Claw who stands still.
    let mut dry = settled(level, fix);
    let (x, y) = dry.player().map(|o| (o.x, o.y)).unwrap();
    let health = dry.stats().health;
    launch(&mut dry, dynamite(x, y));
    let at = (1..300)
        .find(|_| {
            dry.step(TickInput::default());
            dry.stats().health < health
        })
        .expect("the dynamite goes off");
    let mut sim = settled(level, fix);
    let ground = sim.player().unwrap().y;
    launch(&mut sim, dynamite(x, y));
    for n in 1..at {
        sim.step(if n == at - 3 {
            press(JUMP)
        } else {
            TickInput::default()
        });
    }
    sim.step(TickInput::default());
    assert!(sim.stats().health < health, "the blast hit");
    through_the_reaction(&mut sim, ground);
    sim
}

fn jumps_again(sim: &mut Sim) {
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND, "standing in mid-air");
    let y = o.y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::JUMP, "a second jump");
    assert!(o.y < y, "he rises: {} from {y}", o.y);
}

fn cannot_jump_again(sim: &mut Sim) {
    let y = sim.player().unwrap().y;
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::FALL, "no second jump");
    assert!(o.y >= y, "he falls: {} from {y}", o.y);
}

#[test]
fn install_a_soldiers_shot_in_mid_air_allows_a_second_jump_unless_fixed() {
    let Some(level) = level1() else { return };
    jumps_again(&mut shot_in_mid_air(&level, false));
    cannot_jump_again(&mut shot_in_mid_air(&level, true));
}

#[test]
fn install_his_own_dynamite_in_mid_air_allows_a_second_jump_unless_fixed() {
    let Some(level) = level1() else { return };
    jumps_again(&mut own_dynamite_in_mid_air(&level, false));
    cannot_jump_again(&mut own_dynamite_in_mid_air(&level, true));
}
