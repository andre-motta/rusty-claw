//! The soldier: an enemy with a musket (to be filled in; until then a
//! soldier stands still and cannot be hurt). Its data is the shared [`super::EnemyData`].

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Soldier"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_SOLDIER"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SOLDIER"];

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
