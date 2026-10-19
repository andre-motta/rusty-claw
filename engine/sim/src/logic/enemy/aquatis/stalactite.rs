//! The five stalactites over Aquatis ([`super`]), which fall on him one by
//! one.
//!
//! They hang from the cave's ceiling with the cave stalactites' picture
//! and box ([`crate::logic::stalactite`]) but follow Aquatis, not Claw:
//! when an earthquake from the crack ends, the one whose record's smarts
//! is the next in order (the link's order counter, from 1) falls, under
//! the same gravity, and stops at the link's landing height, where it
//! crumbles and is gone. While it falls it is an enemy's body to whatever
//! it touches, so it hurts Aquatis ([`super::stalactite_hit`]). Each
//! resets itself, back at its home, when its reset bit
//! ([`super::STALACTITE_BITS`]) is set, and puts the order counter back
//! to 1.
//!
//! Not built yet ([`BUILT`] is false): they stay inert placeholders and
//! the calls below do nothing.

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisStalactite"];

/// Image sets this family may switch an object to: the cave stalactites'.
pub const IMAGE_SETS: &[&str] = &[crate::logic::stalactite::IMAGE_SET];

/// Prefixes of the animations this family may play: the cave
/// stalactites'.
pub const ANIM_PREFIXES: &[&str] = crate::logic::stalactite::ANIM_PREFIXES;

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Ticks of a fall from the ceiling over Aquatis to the landing height at
/// the 18 ms step (measured), the hit on him on the 19th.
pub const FALL_TICKS: u32 = 25;

/// A stalactite's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StalactiteData {}

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
