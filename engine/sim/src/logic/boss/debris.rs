//! The grass, leaves and sand RedTail's wind blows across his arena:
//! decoration only, out of every collision, flying left from the arena's
//! right side until it passes [`END_X`] or after [`LIFE_MS`]. Making a
//! piece draws from the shared random stream (its place, depth, look,
//! speed and drift), so the built wind makes every piece to keep the
//! stream in step with the game.
//!
//! Not built yet ([`BUILT`] is false): nothing makes one, and the calls
//! below do nothing.

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["WindDebris"];

/// Its three looks.
pub const LOOKS: [&str; 3] = ["LEVEL_WINDGRASS", "LEVEL_WINDLEAF", "LEVEL_WINDSAND"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &LOOKS;

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_CYCLE50"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// A piece is gone once it reaches this x, px.
pub const END_X: i32 = 36500;
/// A piece is gone after this time, ms.
pub const LIFE_MS: u32 = 10_000;

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DebrisData {}

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
