//! The logic framework seen from outside: every logic of the first level
//! binds to a family, the families' run-time lists reach the level, and
//! the player's state table still declares every row.

use claw_sim::logic::{self, GameLogic};
use claw_sim::object::LogicKind;
use claw_sim::player::{RowKind, STATE_TABLE, row_of, state};
use claw_sim::registry::Registry;

/// Every logic placed in the first level, with the family that runs it.
const LEVEL1: [(&str, GameLogic); 38] = [
    ("BehindAniCandy", GameLogic::Decor),
    ("GlitterlessPowerup", GameLogic::Pickup),
    ("TreasurePowerup", GameLogic::Pickup),
    ("CrumblingPeg", GameLogic::Platform),
    ("DoNothing", GameLogic::Decor),
    ("PowderKeg", GameLogic::Keg),
    ("FrontCandy", GameLogic::Decor),
    ("Elevator", GameLogic::Platform),
    ("Officer", GameLogic::Officer),
    ("FrontCrate", GameLogic::Crate),
    ("SoundTrigger", GameLogic::SoundObject),
    ("TogglePeg", GameLogic::Platform),
    ("BigSoundTrigger", GameLogic::SoundObject),
    ("HealthPowerup", GameLogic::Pickup),
    ("BackStackedCrates", GameLogic::Crate),
    ("TogglePeg2", GameLogic::Platform),
    ("Soldier", GameLogic::Soldier),
    ("BehindCandy", GameLogic::Decor),
    ("Rat", GameLogic::Rat),
    ("ClawDialogSmallSoundTrigger", GameLogic::SoundObject),
    ("TogglePeg3", GameLogic::Platform),
    ("SpecialPowerup", GameLogic::Pickup),
    ("BehindCrate", GameLogic::Crate),
    ("SpotAmbientSound", GameLogic::SoundObject),
    ("CursePowerup", GameLogic::Pickup),
    ("Checkpoint", GameLogic::Checkpoint),
    ("SmallSoundTrigger", GameLogic::SoundObject),
    ("TogglePeg4", GameLogic::Platform),
    ("TriggerElevator", GameLogic::Platform),
    ("FrontStackedCrates", GameLogic::Crate),
    ("GlobalAmbientSound", GameLogic::SoundObject),
    ("AmbientSound", GameLogic::SoundObject),
    ("StartElevator", GameLogic::Platform),
    ("SecondSuperCheckpoint", GameLogic::Checkpoint),
    ("FirstSuperCheckpoint", GameLogic::Checkpoint),
    ("FrontAniCandy", GameLogic::Decor),
    ("AmmoPowerup", GameLogic::Pickup),
    ("ClawDialogBigSoundTrigger", GameLogic::SoundObject),
];

#[test]
fn every_first_level_logic_binds_to_its_family() {
    let registry = Registry::standard();
    for (name, family) in LEVEL1 {
        assert_eq!(
            logic::kind_for(name),
            Some(LogicKind::Game(family)),
            "{name}"
        );
        assert_eq!(
            registry.get(name).map(|e| e.kind),
            Some(LogicKind::Game(family)),
            "{name}"
        );
    }
}

#[test]
fn the_state_table_still_declares_every_row() {
    for id in (0..=31).chain(5000..=5010) {
        assert_eq!(row_of(id).id, id);
    }
    for id in [
        state::HIT,
        state::LIFT,
        state::THROW,
        state::DEATH_FALL,
        state::DEATH,
        state::SWING,
        state::BEARHUG,
        state::FREEZE,
    ] {
        assert_eq!(row_of(id).kind, RowKind::Fallback, "{id}");
    }
    assert_eq!(STATE_TABLE.len(), 43);
}
