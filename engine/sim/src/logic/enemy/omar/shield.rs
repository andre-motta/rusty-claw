//! A piece of Omar's ring: one of the nine pieces of fire or ice that
//! circle him in the first phase of his fight (see [`super::ring_angle`]).
//! A piece touching Claw costs him [`DAMAGE`]; it stops Claw's pistol
//! shots and sword shots, which end on it; only Omar's first phase ending
//! removes it.
//!
//! Not built yet ([`BUILT`] is false): nothing makes one, and the calls
//! below do nothing.

use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["OmarShield"];

/// The fire piece's picture.
pub const FIRE_SET: &str = "LEVEL_FIRESHIELD";
/// The ice piece's picture.
pub const ICE_SET: &str = "LEVEL_ICESHIELD";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[FIRE_SET, ICE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[FIRE_SET, ICE_SET];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// What touching a piece costs Claw.
pub const DAMAGE: i32 = 1;
/// A piece's hit and attack box.
pub const RECT: Rect = Rect::new(-14, -6, 12, 21);

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShieldData {
    /// Omar, whose ring it belongs to.
    pub omar: Option<ObjectId>,
    /// Its place in the ring, 0 to 8.
    pub index: usize,
}

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
