//! The hidden helper of Aquatis's room ([`super`]) that puts a stick of
//! dynamite back on a ledge of the room.
//!
//! When it is built it remembers how many sticks Claw carries and puts one
//! stick down at its own position (a dynamite pickup, [`STICK_AT`] on the
//! twelfth level). Its box covers the whole room, and a blast anywhere in
//! it, at most once every [`GATE_MS`], puts a new stick down when the last
//! one has been taken (the refill flag of the Aquatis link is clear) and
//! Claw carries no more sticks than at the last refill. Taking a stick
//! clears the flag; Aquatis's respawn reset clears it too.
//!
//! Not built yet ([`BUILT`] is false): it stays an inert placeholder and
//! the calls below do nothing.
//!
//! # Defaults taken
//!
//! - The refill after the first stick has been taken has not been seen on
//!   the original: it follows the rule above (a blast in the room, the
//!   flag clear, Claw holding no more sticks than at the last refill).

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisDynamite"];

/// The stick's picture.
pub const IMAGE_SET: &str = "GAME_DYNAMITE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// The shortest time between two blasts it answers, ms.
pub const GATE_MS: u32 = 750;
/// Where the stick waits on the twelfth level: the helper's position.
pub const STICK_AT: (i32, i32) = (37250, 3490);

/// The helper's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DynamiteData {}

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
