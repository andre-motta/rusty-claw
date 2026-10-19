//! RedTail's flares: fired from his flare gun at a Claw on his own level,
//! a flare flies at [`SPEED`] px/s and falls; it is gone on the tick
//! after it meets a wall or floor, and on Claw it explodes where it is,
//! hurting him for [`DAMAGE`].
//!
//! Not built yet ([`BUILT`] is false): nothing makes one, and the calls
//! below do nothing.

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailBullet"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAILBULLET";
/// The picture of its explosion.
pub const EXPLOSION_SET: &str = "LEVEL_REDTAILBULLET_EXPLOSION";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, EXPLOSION_SET];

/// The explosion.
pub const EXPLOSION_ANIM: &str = "LEVEL_REDTAILBULLETEXPLOSION";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[EXPLOSION_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// What a flare costs Claw.
pub const DAMAGE: i32 = 15;
/// Its speed across, px/s.
pub const SPEED: i32 = 500;

/// A flare's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlareData {}

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
