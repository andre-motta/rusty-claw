//! Skull cannons: the ninth level's skull-shaped cannons, which fire a
//! cannonball ([`ball`]) every three seconds whether Claw is near or not.
//!
//! A skull cannon is a tower cannon of the second level's kind
//! ([`super::cannon`]) with its own art: built on its first call (balls at
//! [`SPEED`] px/s, depth [`DEFAULT_Z`] when placed at 0), it waits in
//! `Home` and fires on its first waiting call and then every
//! [`PERIOD_TICKS`] ticks ([`PERIOD_MS`] of game time), counted from the
//! tick it woke. Its firing animation ([`FIRING_ANIM`], [`FIRE_TICKS`]
//! ticks) releases the ball on its first call, with the animation's own
//! sounds. Facing right or left (a mirrored record), the ball always
//! leaves 50 px to the right of the cannon and 10 px below it
//! ([`ball::start`]), so a cannon facing left fires its balls from behind
//! itself, through itself. It never tests where Claw is and draws no
//! random number.
//!
//! Not built yet ([`BUILT`] is false): skull cannons stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! # State numbers
//!
//! As every cannon it waits in `Home` and fires in `Fire`
//! ([`crate::logic::enemy::EnemyState`]).
//!
//! # Defaults taken
//!
//! - The muzzle is (50, 10) for both facings, as measured (the mirrored
//!   muzzle is overwritten when the cannon is built).
//!
//! Its data ([`SkullCannonData`]) is its own.

use super::World;
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SkullCannon"];

/// The cannon's picture.
pub const IMAGE_SET: &str = "LEVEL_SKULLCANNON";

/// Image sets this family may switch an object to: its own and its ball's.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, ball::IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SKULLCANNON_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// Firing.
pub const FIRING_ANIM: &str = "LEVEL_SKULLCANNON_FIRING";
/// Waiting.
pub const HOME_ANIM: &str = "LEVEL_SKULLCANNON_HOME";

/// Speed of its balls when the record gives none, px/s.
pub const SPEED: i32 = 500;
/// Depth of a cannon placed at depth 0.
pub const DEFAULT_Z: i32 = 3990;
/// Time between two shots, ms of game time.
pub const PERIOD_MS: u32 = 3000;
/// Ticks between two shots at the 18 ms step (measured).
pub const PERIOD_TICKS: u32 = 167;
/// Ticks of the firing animation at the 18 ms step.
pub const FIRE_TICKS: u32 = 43;

/// The ball a skull cannon fires: the second level's cannonball, an
/// enemy's shot that costs Claw [`DAMAGE`](ball::DAMAGE), also when he
/// ducks, and flies straight at the cannon's speed. The ninth level has
/// no animation for it, so it shows its first picture throughout. It is
/// not a family of its own: the projectile family runs it.
pub mod ball {
    /// Logic name of the ball.
    pub const LOGIC: &str = "CannonBall";
    /// Its picture.
    pub const IMAGE_SET: &str = "LEVEL_CANNONBALL";
    /// The picture it shows throughout on the ninth level.
    pub const FRAME: i32 = 1;
    /// Damage to Claw.
    pub const DAMAGE: i32 = 10;
    /// Where it leaves, from the cannon, whichever way the cannon faces.
    pub const MUZZLE: (i32, i32) = (50, 10);

    /// Where a ball of the cannon at (`x`, `y`) leaves, and its speed for a
    /// cannon facing left (`mirrored`) or right.
    pub fn start((x, y): (i32, i32), mirrored: bool) -> ((i32, i32), i32) {
        let speed = if mirrored {
            -super::SPEED
        } else {
            super::SPEED
        };
        ((x + MUZZLE.0, y + MUZZLE.1), speed)
    }
}

/// A skull cannon's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkullCannonData {
    /// Balls fired so far.
    pub fired: u32,
}

/// How a skull cannon whose first call has not run yet looks once it has
/// (see [`super::first_look`]): `None`, as it looks now.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = (level, o);
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits a cannon.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A cannon attacks nothing; its balls do.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balls_leave_on_the_right_whichever_way_the_cannon_faces() {
        // The crossfire shaft's cannons: 326 fires right, 322 left from
        // behind itself.
        assert_eq!(ball::start((7490, 6693), false), ((7540, 6703), 500));
        assert_eq!(ball::start((8251, 6948), true), ((8301, 6958), -500));
        // 9 px a tick, rounded.
        assert_eq!((SPEED * 18 + 500) / 1000, 9);
    }

    #[test]
    fn it_fires_as_a_tower_cannon() {
        assert_eq!(PERIOD_MS.div_ceil(18), PERIOD_TICKS);
        assert_eq!(u64::from(PERIOD_MS), super::super::cannon::PERIOD_MS);
        assert_eq!(SPEED, super::super::cannon::SPEED);
        assert_eq!(DEFAULT_Z, super::super::cannon::DEFAULT_Z);
        assert_eq!(ball::MUZZLE, super::super::cannon::MUZZLE);
        assert_eq!(ball::LOGIC, super::super::cannon::BALL_LOGIC);
        // Seven pictures of 100 ms, six calls each, and the finishing call.
        assert_eq!(FIRE_TICKS, 7 * 100u32.div_ceil(18) + 1);
    }
}
