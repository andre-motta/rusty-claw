//! The search in the thirteenth level's boss room on your own install
//! (skipped without `CLAW_INSTALL`): RedTail is read as the boss whose
//! gem ends the level, once beaten on his ledge he is on his way to his
//! gem, and struck down in the middle of a leap he fights on beaten.
//! Nothing from an install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_bot::search::{BossStage, boss_beaten, boss_end, boss_stage};
use claw_sim::{Buttons, Sim, SimLevel, TickInput};

/// Left of the boss room's door, where the reference runs of the room
/// start (a walk right takes Claw into the door).
const DOOR_LEFT: (i32, i32) = (32900, 2327);

/// The random state those runs set when Claw has settled there.
const SEGMENT_SEED: u32 = 111_567;

fn level13() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(13)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// The game settled left of the door, RedTail given `boss_health` on
/// Claw's first tick in the room when it is set.
fn beside_the_door_with(level: SimLevel, boss_health: Option<i32>) -> Sim {
    let mut cfg = route::sim_config(TuningName::Original, SEGMENT_SEED);
    cfg.boss_health = boss_health;
    let mut sim = Sim::new(level, cfg);
    route::settle_at(&mut sim, DOOR_LEFT.0, DOOR_LEFT.1, SEGMENT_SEED);
    sim
}

/// The game settled left of the door.
fn beside_the_door(level: SimLevel) -> Sim {
    beside_the_door_with(level, None)
}

/// Plays `mask` for `ticks` ticks, with the presses it makes.
fn hold(sim: &mut Sim, prev: &mut u32, mask: u32, ticks: usize) {
    for _ in 0..ticks {
        sim.step(TickInput {
            held: Buttons(mask),
            pressed: Buttons(mask & !*prev),
        });
        *prev = mask;
    }
}

#[test]
fn install_level13_redtail_is_the_boss_whose_gem_ends_the_level() {
    let Some(level) = level13() else { return };
    let start = beside_the_door(level);
    let end = boss_end(&start).expect("the thirteenth level ends with a boss");
    assert_eq!(end.door, (33002, 2267));
    assert_eq!(end.room, (37176, 2081));
    assert_eq!(end.boss, (37431, 1886));
    assert_eq!(end.gem, (37509, 1750));
    // Blows hurt him: no throw into the water is looked for.
    assert_eq!(end.water, None);
}

#[test]
fn install_level13_the_door_leads_into_redtails_fight() {
    let Some(level) = level13() else { return };
    let mut sim = beside_the_door(level);
    assert_eq!(boss_stage(&sim), BossStage::Door);
    let mut prev = 0;
    for _ in 0..60 {
        hold(&mut sim, &mut prev, Buttons::RIGHT.0, 1);
        if boss_stage(&sim) == BossStage::Fight {
            break;
        }
    }
    assert_eq!(boss_stage(&sim), BossStage::Fight, "RedTail is in play");
    let c = sim.player().expect("Claw");
    assert!(c.x > 37000, "Claw is in the boss room, at x {}", c.x);
}

#[test]
fn install_level13_redtail_beaten_on_his_ledge_is_on_his_way_to_his_gem() {
    let Some(level) = level13() else { return };
    // RedTail starts the fight with 5 health, so one blow beats him.
    let mut sim = beside_the_door_with(level, Some(5));
    let mut prev = 0;
    let (right, jump, attack) = (Buttons::RIGHT.0, Buttons::JUMP.0, Buttons::ATTACK.0);
    // Into the door, the jump-in on the first tick in the room (he stays
    // in his start-up pose), up the left peg onto his ledge and one blow.
    for (mask, ticks) in [
        (0, 15),
        (right, 15),
        (right | jump, 20),
        (jump, 2),
        (0, 98),
        (jump, 14),
        (0, 21),
        (right | jump, 14),
        (right, 11),
        (0, 15),
        (attack, 2),
        (0, 10),
    ] {
        hold(&mut sim, &mut prev, mask, ticks);
    }
    // The blow beats him on his ledge: he jumps down to die on the sand.
    assert_eq!(boss_beaten(&sim), Some(true));
    hold(&mut sim, &mut prev, 0, 50);
    assert_eq!(boss_beaten(&sim), Some(true));
    let boss = sim.boss();
    let id = boss.current().expect("RedTail is in play");
    let redtail = sim.objects().get(id).expect("RedTail");
    assert!(redtail.health <= 0, "beaten, health {}", redtail.health);
    assert_eq!(boss.defeated(), None, "his gem has not risen yet");
    // His gem rises once his fall and his last line are over.
    let mut prev_mask = prev;
    for _ in 0..200 {
        if sim.boss().defeated().is_some() {
            break;
        }
        hold(&mut sim, &mut prev_mask, 0, 1);
    }
    assert!(sim.boss().defeated().is_some(), "his gem rises");
}

#[test]
fn install_level13_redtail_struck_down_in_a_leap_fights_on_beaten() {
    let Some(level) = level13() else { return };
    // RedTail starts the fight with 10 health: two damaging blows beat
    // him. After the jump-in his blow count starts at 1, so the second
    // blow sends him leaping instead of hurting him.
    let mut sim = beside_the_door_with(level, Some(10));
    let mut prev = 0;
    let (right, jump, attack) = (Buttons::RIGHT.0, Buttons::JUMP.0, Buttons::ATTACK.0);
    // The jump-in, up onto his ledge, the first blow (he keeps 5), the
    // second (he leaps away and back), and the third as he leaps back up
    // (blows 55 to 66 ticks after the second land in that leap).
    for (mask, ticks) in [
        (0, 15),
        (right, 15),
        (right | jump, 20),
        (jump, 2),
        (0, 98),
        (jump, 14),
        (0, 21),
        (right | jump, 14),
        (right, 11),
        (0, 15),
        (attack, 2),
        (0, 10),
    ] {
        hold(&mut sim, &mut prev, mask, ticks);
    }
    assert_eq!(boss_beaten(&sim), None, "the first blow leaves him health");
    hold(&mut sim, &mut prev, 0, 21);
    hold(&mut sim, &mut prev, attack, 2);
    hold(&mut sim, &mut prev, 0, 61);
    assert_eq!(boss_beaten(&sim), None, "the second blow sends him leaping");
    hold(&mut sim, &mut prev, attack, 2);
    // Struck down in the air, he lands with no death to come and fights
    // on with no health left.
    let mut beaten_at = None;
    for t in 0..40 {
        if boss_beaten(&sim) == Some(false) {
            beaten_at = Some(t);
            break;
        }
        hold(&mut sim, &mut prev, 0, 1);
    }
    assert!(beaten_at.is_some(), "the third blow strikes him down");
    hold(&mut sim, &mut prev, 0, 150);
    assert_eq!(boss_beaten(&sim), Some(false), "he fights on beaten");
    let boss = sim.boss();
    let id = boss.current().expect("RedTail is in play");
    let redtail = sim.objects().get(id).expect("RedTail");
    assert!(redtail.health <= 0, "beaten, health {}", redtail.health);
    assert_eq!(boss.defeated(), None, "no gem rises");
}
