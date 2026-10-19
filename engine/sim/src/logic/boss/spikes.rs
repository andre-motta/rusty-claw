//! The stakes at the left wall of RedTail's arena: a static hazard that
//! hurts Claw for [`DAMAGE`] on touch. Nothing drives them, they never
//! animate and nothing can hit them; a Claw held against them (by the
//! wind) is hurt again each time his protection after a hit ends.
//!
//! Not built yet ([`BUILT`] is false): the placed record stays an inert
//! placeholder and the calls below do nothing.

use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailSpikes"];

/// Their picture.
pub const IMAGE_SET: &str = "LEVEL_BOSSPIKES";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// What touching them costs Claw, before the difficulty factor.
pub const DAMAGE: i32 = 20;

/// Their attack box: their picture's.
pub const ATTACK_RECT: Rect = Rect::new(-32, -104, 31, 103);

/// The stakes' data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpikesData {}

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
