//! Swinging ropes: ropes that hang from the ceiling and swing back and
//! forth on their own. Claw grabs one by jumping or falling into its lower
//! end, hangs from it in his Swing state
//! ([`crate::player::state::SWING`], see [`crate::player::swing`]) while it
//! swings, and lets go with jump.
//!
//! The swing is an animation, not physics: which of the rope's frames
//! shows is a function of the level clock and the rope's own phase, and
//! each frame has a known position for the lower end, where the small
//! grab box sits. On the first tick after the rope is set up, and on the
//! first tick after each wake, it still shows the frame it had before
//! (one stale tick), and the grab box and the end follow that same frame
//! for the tick.
//!
//! Not built yet ([`BUILT`] is false): ropes stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AniRope"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// A rope's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RopeData {}

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
