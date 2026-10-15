//! The officer: a sword-fighting enemy (to be filled in; until then an
//! officer stands still and cannot be hurt). Its data is the shared [`super::EnemyData`].

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Officer"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_OFFICER"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_OFFICER"];

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
