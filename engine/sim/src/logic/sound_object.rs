//! Sound triggers, Claw's dialog triggers and ambient sounds (to be
//! filled in; until then they are silent).

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "AmbientSound",
    "BigSoundTrigger",
    "ClawDialogBigSoundTrigger",
    "ClawDialogSmallSoundTrigger",
    "GlobalAmbientSound",
    "SmallSoundTrigger",
    "SoundTrigger",
    "SpotAmbientSound",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// A sound object's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundData {}

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
