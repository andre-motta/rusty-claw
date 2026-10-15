//! Decoration: still and animated scenery in front of or behind the
//! action (to be filled in; until then it shows its placed frame).

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "BehindAniCandy",
    "BehindCandy",
    "FrontAniCandy",
    "FrontCandy",
    "DoNothing",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// A decoration's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecorData {}

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
