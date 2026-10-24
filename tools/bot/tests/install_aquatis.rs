//! The search in the twelfth level's boss room on your own install
//! (skipped without `CLAW_INSTALL`): Aquatis is read as the boss whose
//! gem ends the level, and a tentacle's squeeze beside him, which holds
//! Claw as a bear's hug does, offers the fight's mash. Nothing from an
//! install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::actions::{self, Action, Arena};
use claw_bot::route::{self, TuningName};
use claw_bot::search::{BossStage, boss_end, boss_stage};
use claw_sim::{Buttons, Sim, SimLevel, TickInput};

/// In the hidden passage under the water, left of the boss room, where
/// the reference runs of the tentacles start: the teleport point, which
/// settles onto the passage's floor.
const PASSAGE: (i32, i32) = (36000, 3779);

/// The random state those runs set when Claw has settled there.
const SEGMENT_SEED: u32 = 111_567;

fn level12() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(12)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// The game settled in the passage.
fn in_the_passage(level: SimLevel) -> Sim {
    let mut sim = Sim::new(level, route::sim_config(TuningName::Original, SEGMENT_SEED));
    route::settle_at(&mut sim, PASSAGE.0, PASSAGE.1, SEGMENT_SEED);
    sim
}

/// Plays `mask` for a tick, with the presses it makes.
fn step(sim: &mut Sim, prev: &mut u32, mask: u32) {
    sim.step(TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask & !*prev),
    });
    *prev = mask;
}

/// Ticks Claw stands still in the passage before he walks, as in the
/// reference runs of the tentacles (the random stream then makes the
/// first tentacle that reaches him grab him rather than strike).
const IDLE: usize = 15;

/// Walks right along the passage until a tentacle holds Claw.
fn walk_into_a_hold(sim: &mut Sim, prev: &mut u32) {
    for i in 0..1000 {
        if actions::held(sim) {
            return;
        }
        let mask = if i < IDLE { 0 } else { Buttons::RIGHT.0 };
        step(sim, prev, mask);
    }
    panic!("no tentacle holds Claw");
}

#[test]
fn install_level12_aquatis_is_the_boss_whose_gem_ends_the_level() {
    let Some(level) = level12() else { return };
    let start = in_the_passage(level);
    let end = boss_end(&start).expect("the twelfth level ends with a boss");
    assert_eq!(end.door, (36407, 2049));
    assert_eq!(end.room, (37148, 3607));
    assert_eq!(end.boss, (37912, 3600));
    assert_eq!(end.gem, (37635, 3455));
    // No throw drowns him: he hangs over the water already.
    assert_eq!(end.water, None);
}

#[test]
fn install_level12_a_tentacle_hold_in_the_fight_offers_the_mash() {
    let Some(level) = level12() else { return };
    let mut sim = in_the_passage(level);
    let mut prev = 0;
    walk_into_a_hold(&mut sim, &mut prev);
    assert_eq!(boss_stage(&sim), BossStage::Fight, "Aquatis is in play");
    assert_eq!(actions::phase(&sim), actions::Phase::Busy);
    assert!(actions::choices_in(&sim, &Arena::default()).contains(&Action::Mash));
}
