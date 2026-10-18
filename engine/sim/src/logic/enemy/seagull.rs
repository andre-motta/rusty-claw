//! The seagull: the first enemy that flies.
//!
//! It patrols a fixed height of sky between its two limits at 120 px/s,
//! turning at each limit, with no gravity and no tile collision: it flies
//! through walls and floors. When Claw is ahead of it in its direction of
//! flight, inside its limits, lower than it by more than 100 and less than
//! 240 px and further away along x than down, it screeches and dives at a
//! point 40 px above where Claw stands, at a fixed 600 px/s along the
//! sum of the two distances, without re-aiming. During the dive its beak
//! ([`BEAK`]) hurts Claw on contact; outside a dive it cannot hurt him.
//! The dive ends once the bird is lower than Claw was when it started, or
//! at a limit; it then flies back up to its patrol height
//! ([`EnemyState::Withdraw`]) and patrols again.
//!
//! It has 1 health, so any hit kills it; a hit it survives (only with a
//! stronger difficulty) flinches in [`EnemyState::GenericHit`]. Dead, it
//! drops its record's loot, falls through the level and goes once off
//! screen. It cannot be lifted and never speaks.
//!
//! Not built yet ([`BUILT`] is false): seagulls stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the seagull patrols in
//! `Idle`, dives in `Strike` and dies in `Dead` with handlers of its own
//! under those numbers. Its two new names take the third and fourth
//! numbers of the fifth level's block ([`STATE_BLOCK`], shared with the
//! town guards): [`EnemyState::Withdraw`] and [`EnemyState::GenericHit`].
//!
//! # Defaults taken
//!
//! - No separate flying-enemy layer: this module holds the flight rules
//!   (no gravity, no tile collision, no vertical limits), since the
//!   seagull is the only flyer built on them.
//! - The "off screen" test that removes a dead seagull, under a widened
//!   view, follows the rule the other enemies' falling bodies use.
//! - The seagulls of later levels run this code with their own animations
//!   and records; their timings are checked with those levels.
//!
//! Its data ([`SeagullData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Seagull"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SEAGULL_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers of the fifth level's families (see
/// [`super::townguard::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::townguard::STATE_BLOCK;

/// Image set.
pub const IMAGE_SET: &str = "LEVEL_SEAGULL";
/// Patrol speed, px/s.
pub const PATROL_SPEED: i32 = 120;
/// Speed of the dive and of the climb back, px/s, measured along the sum
/// of the horizontal and vertical distances.
pub const DIVE_SPEED: f64 = 600.0;
/// The dive aims this far above where Claw stands, px.
pub const AIM_ABOVE: i32 = 40;
/// Claw must be more than this far below the bird, and less than the
/// second value, for a dive, px.
pub const DIVE_BAND: (i32, i32) = (100, 240);
/// The beak's box during a dive, facing left.
pub const BEAK: Rect = Rect::new(-26, -5, -5, 22);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-30, -30, 30, 30);
/// Depth given to a seagull placed at depth 0: in front of Claw.
pub const DEFAULT_Z: i32 = 8500;
/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 1;
/// Damage of the beak before the difficulty factor.
pub const DAMAGE: i32 = 10;

/// The velocity that flies from `from` to `to` at `speed`, measured along
/// the sum of the two distances (the dive and the climb back); zero when
/// the two points are the same.
pub fn fixed_speed_velocity(from: (f64, f64), to: (f64, f64), speed: f64) -> (f64, f64) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l1 = dx.abs() + dy.abs();
    if l1 == 0.0 {
        return (0.0, 0.0);
    }
    (speed * dx / l1, speed * dy / l1)
}

/// The seagull class.
pub static CLASS: Class = Class {
    name: "Seagull",
    human: false,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// A seagull's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SeagullData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_new_states_follow_the_guards_in_the_block() {
        assert_eq!(EnemyState::Withdraw as i32, STATE_BLOCK.start() + 2);
        assert_eq!(EnemyState::GenericHit as i32, STATE_BLOCK.start() + 3);
        assert!(!CLASS.human, "the seagull is no human enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
    }

    #[test]
    fn the_dive_flies_at_a_fixed_speed_along_both_distances() {
        // Claw 200 px ahead and 150 px below: the aim is 110 px below.
        let (vx, vy) = fixed_speed_velocity((0.0, 0.0), (200.0, 110.0), DIVE_SPEED);
        assert!(
            (vx - 387.1).abs() < 0.1 && (vy - 212.9).abs() < 0.1,
            "{vx} {vy}"
        );
        assert!((vx.abs() + vy.abs() - DIVE_SPEED).abs() < 1e-9);
        // Diving left and down.
        let (vx, vy) = fixed_speed_velocity((6404.0, 2700.0), (6304.0, 2900.0), DIVE_SPEED);
        assert!((vx - -200.0).abs() < 1e-9 && (vy - 400.0).abs() < 1e-9);
        assert_eq!(
            fixed_speed_velocity((5.0, 5.0), (5.0, 5.0), DIVE_SPEED),
            (0.0, 0.0)
        );
    }

    #[test]
    fn the_beak_reaches_ahead_and_below_a_bird_facing_left() {
        const { assert!(BEAK.right < 0 && BEAK.bottom > 0) };
        const { assert!(HIT_RECT.left < BEAK.left) };
    }
}
