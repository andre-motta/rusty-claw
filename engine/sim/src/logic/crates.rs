//! Crates and stacks of crates (to be filled in; until then they cannot
//! be broken).

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "BackStackedCrates",
    "BehindCrate",
    "FrontCrate",
    "FrontStackedCrates",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_CRATEBREAK"];

/// A crate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrateData {}

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

/// `carrier` lifted `id`.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    let _ = (w, id, carrier);
}

/// `id` was thrown at (vx, vy) px/s.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    let _ = (w, id, vx, vy);
}

/// `id` was put down or let go.
pub fn dropped(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}
