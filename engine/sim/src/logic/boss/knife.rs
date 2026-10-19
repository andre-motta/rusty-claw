//! RedTail's knives: thrown three at a time at a Claw on the other level
//! of his arena, they fly straight through everything at the speed they
//! were given and hurt Claw for [`DAMAGE`]; one is gone below the sand
//! ([`GONE_BELOW`]) or after [`LIFE_MS`].
//!
//! Not built yet ([`BUILT`] is false): nothing makes one, and the calls
//! below do nothing.

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailKnife"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAILKNIFE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// What a knife costs Claw.
pub const DAMAGE: i32 = 5;
/// A knife is gone once below this height, px.
pub const GONE_BELOW: i32 = 2150;
/// A knife is gone after this time, ms.
pub const LIFE_MS: u32 = 10_000;

/// A knife's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnifeData {}

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
