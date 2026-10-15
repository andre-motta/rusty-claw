//! Checkpoint flags and the two save points of a level (to be filled in;
//! until then touching them does nothing).

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Checkpoint",
    "FirstSuperCheckpoint",
    "SecondSuperCheckpoint",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["GAME_CHECKPOINTFLAG", "GAME_SUPERCHECKPOINT"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_CHECKPOINT"];

/// A checkpoint's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckpointData {}

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
