//! Omar's energy ball: cast in the second phase of his fight, it leaves
//! him [`OFFSET`] px out at [`SPEED`] px/s towards the way he faces, flies
//! straight with no gravity, costs Claw [`super::BALL_DAMAGE`] and is gone
//! when it touches Claw or a wall.
//!
//! Not built yet ([`BUILT`] is false): nothing makes one, and the calls
//! below do nothing.

use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["OmarBullet"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_OMARPROJECTILE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none of its own.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Where it leaves Omar, facing left: across and up.
pub const OFFSET: (i32, i32) = (-65, -20);
/// Its speed, px/s.
pub const SPEED: i32 = 300;
/// Its box, flying right.
pub const RECT: Rect = Rect::new(-24, -23, 23, 22);

/// An energy ball's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BulletData {}

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
