//! The boss fight's stage: everything around a boss that is not the boss
//! himself.
//!
//! - `BossWarp`: the door that takes Claw into the boss's room;
//! - `BossStager`: the trigger next to where Claw arrives, which plays the
//!   introduction (Claw frozen, the camera on the boss, the two lines, the
//!   name banner) and then starts the fight;
//! - `BossStagerAttack`: the stager's touch, which accepts Claw only while
//!   he stands or walks;
//! - `BossHealthMeter`: the boss's health bar for the whole fight.
//!
//! They work through the boss fight state of [`super::world_boss`].
//!
//! Not built yet ([`BUILT`] is false): these objects stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "BossWarp",
    "BossStager",
    "BossStagerAttack",
    "BossHealthMeter",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The data of a piece of the stage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BossStageData {}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}
