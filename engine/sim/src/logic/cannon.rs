//! Cannons: the tower cannons, which fire a cannonball every few seconds
//! from a fixed muzzle, and the small ground cannon a punk rat fires.
//!
//! Not built yet ([`BUILT`] is false): these objects stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! State numbers ([`EnemyState`](super::enemy::EnemyState)): a cannon
//! waits in `Home`, fires in `Fire` and fires upwards in `VertFiring`.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TowerCannonLeft", "TowerCannonRight", "GroundCannon"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// A cannon's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CannonData {}

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
