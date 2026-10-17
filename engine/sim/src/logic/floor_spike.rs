//! Floor spikes: small spike rows set into the ground that slide up, stay
//! up for a while and slide back down, forever, on a rhythm fixed by the
//! level clock.
//!
//! The four names are one logic and differ only in their phase:
//! `FloorSpike` starts its cycle at clock 0, `FloorSpike2` 750 ms later,
//! `FloorSpike3` 1500 ms later and `FloorSpike4` 2250 ms later, so a row
//! laid out 1, 2, 3, 4 rises as a travelling wave. While a spike is up it
//! hurts what stands on it (10 damage, through the same path an
//! explosion takes); while it is down or moving it is harmless. The clock
//! keeps every spike in phase however long it slept outside its window.
//!
//! The spike's rising sound plays only when the spike was on screen in the
//! previous tick: its current frame's box, placed as it is drawn,
//! overlapping the 640x480 game view of that tick. The sound is a cue and
//! never changes what the spike does.
//!
//! Not built yet ([`BUILT`] is false): floor spikes stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["FloorSpike", "FloorSpike2", "FloorSpike3", "FloorSpike4"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// A floor spike's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloorSpikeData {}

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
