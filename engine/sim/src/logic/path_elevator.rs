//! Path elevators: moving platforms that follow a closed route of straight
//! legs, each leg one of the eight keypad directions for a given number of
//! pixels, at a fixed speed.
//!
//! Like the plain elevator, a path elevator does not step itself forward:
//! every tick it computes where the level clock puts it on its route and
//! moves there, so it stays in phase however long it slept outside its
//! window, and it tells its rider how far it moved in the same tick.
//!
//! The route is written into the level record's four boxes (movement,
//! hit, attack and clip), two legs per box as (direction, distance)
//! pairs. The legs are read from the boxes as the level loader leaves
//! them, after its rule for empty boxes, so a box with zero left and right
//! reads as an unset leg; the real platform boxes replace them after.
//! Two built-in routes, chosen by the record's powerup field, also exist;
//! that field is never loot.
//!
//! Not built yet ([`BUILT`] is false): path elevators stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PathElevator"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// A path elevator's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathElevatorData {}

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
