//! Mercat's trident bolt ([`crate::logic::enemy::mercat`]): a bolt of
//! lightning thrown straight ahead at [`SPEED`] px/s without falling. It
//! hurts Claw for [`DAMAGE`] and bursts on him into a short explosion
//! that stays where it burst for [`EXPLOSION_TICKS`] ticks and hurts the
//! human enemies in its box (his own thrower among them, when Claw stands
//! close in front of him); a wall ends it. It sheds sparkles and carries a
//! light as it flies, and goes when Claw respawns.
//!
//! It flies as the projectiles do ([`crate::logic::projectile`]): the
//! projectile family sets it up through [`set_up`] and runs each of its
//! calls through [`step`], and Claw's touch reaches [`impact`]; whatever
//! a bolt keeps beyond the projectiles' data is its [`Fields`].
//!
//! Not built yet ([`BUILT`] is false): a bolt asked for is an inert
//! placeholder holding the request, and the calls below do nothing.

use super::{ProjectileData, ProjectileSpec, store};
use crate::logic::World;
use crate::object::{Object, ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TridentBullet"];

/// The bolt's picture.
pub const IMAGE_SET: &str = "LEVEL_TRIDENT_TRIDENTPROJECTILE";
/// The explosion's pictures.
pub const EXPLOSION_SET: &str = "LEVEL_TRIDENT_TRIDENTEXPLOSION";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, EXPLOSION_SET];

/// The explosion.
pub const EXPLOSION_ANIM: &str = "LEVEL_TRIDENTEXPLOSION";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[EXPLOSION_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Damage to Claw and to the human enemies in the explosion, whatever the
/// difficulty.
pub const DAMAGE: i32 = 20;
/// Speed, px/s.
pub const SPEED: i32 = 400;
/// The bolt's box, flying left, when the level does not know its first
/// frame; turned to face right for a bolt flying right.
pub const FRAME_BOX: Rect = Rect::new(-32, -14, 31, 13);
/// Ticks of the explosion at the 18 ms step (measured).
pub const EXPLOSION_TICKS: u32 = 34;
/// Sparkles a bolt sheds per tick.
pub const SPARKLES: u32 = 2;

/// What a bolt keeps beyond the projectiles' data (see
/// [`ProjectileData::trident`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {}

/// A bolt's data. A bolt keeps the projectiles' data
/// ([`ProjectileData`]); this is not used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TridentData {}

/// Sets a new bolt up from `spec` (the projectiles' spawn calls it after
/// the shared set-up).
pub(super) fn set_up(o: &mut Object, d: &mut ProjectileData, spec: &ProjectileSpec) {
    let _ = (o, d, spec);
}

/// One call of a bolt, flying or exploding.
pub(super) fn step(w: &mut World, id: ObjectId, d: ProjectileData) {
    store(w, id, d);
}

/// The bolt touched Claw and bursts where it is.
pub(super) fn impact(w: &mut World, id: ObjectId, d: &mut ProjectileData) {
    let _ = (w, id, d);
}

/// One logic call: the projectiles' ([`super::tick`]).
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id);
}

/// `attacker` hit `id`. Nothing hits a bolt.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The victim's own hit handler decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::enemy::mercat;

    #[test]
    fn the_bolt_is_mercats() {
        assert_eq!(SPEED, mercat::BOLT_SPEED);
        assert_eq!(DAMAGE, mercat::BOLT_DAMAGE);
        // 34 explosion ticks outlast no hazard window of Claw's: his 850 ms
        // after the bolt's hit cover them.
        const { assert!(EXPLOSION_TICKS * 18 < 850) };
        assert_eq!(FRAME_BOX.right - FRAME_BOX.left, 63);
    }
}
