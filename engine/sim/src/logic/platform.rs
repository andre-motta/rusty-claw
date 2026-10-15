//! Elevators and pegs (to be filled in; until then they stay where they
//! are placed).
//!
//! A platform that moves sets [`crate::object::Object::platform_move`] to
//! the move it made in the tick, which the objects riding it add to
//! their own.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Elevator",
    "StartElevator",
    "TriggerElevator",
    "CrumblingPeg",
    "TogglePeg",
    "TogglePeg2",
    "TogglePeg3",
    "TogglePeg4",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// A platform's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlatformData {}

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
