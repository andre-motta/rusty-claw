//! The town guards of the fifth and sixth levels: `TownGuard1`, a guard
//! with a sword and a truncheon, and `TownGuard2`, a heavier one. They are
//! one family with two classes ([`guard1`] and [`guard2`]): the same
//! states, pre-tick and hit rules over two image sets and two sets of
//! boxes and voices.
//!
//! Like the officer they walk up to Claw and strike, high at a standing
//! Claw and low at a crouching one, but they add three things:
//!
//! - they read Claw's sword: while Claw swings at a guard that faces him,
//!   the guard may shout and hop back 100 px from where it stands (a
//!   chance that starts at 90 for `TownGuard2`, 0 for `TownGuard1` on the
//!   fifth level, and falls by 30 with every hop);
//! - `TownGuard2` parries one in four of Claw's sword blows on the fifth
//!   level, in a guard pose ([`EnemyState::Block`]) during which nothing
//!   hurts it;
//! - their idle routine never looks at Claw: two idle poses
//!   ([`EnemyState::Idle`] then [`EnemyState::Idle2`]) with a random wait
//!   between them, then pacing; only the sword reaction, a hit, a lift, a
//!   jump switch or Claw leaving range cut it short.
//!
//! Both can be lifted and thrown, drop their record's loot and talk a
//! lot. A jump switch they walk over launches them through the air
//! ([`crate::logic::jump_switch`]).
//!
//! Not built yet ([`BUILT`] is false): the guards stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the guards decide in
//! `Home`, idle in `Idle` and strike in `Strike` with handlers of their
//! own under those numbers, and inherit the shared human states with their
//! numbers. Their two new names take the first two numbers of the fifth
//! level's block [`STATE_BLOCK`]: [`EnemyState::Idle2`], the second idle
//! pose, and [`EnemyState::Block`], the parry. The seagull's two new
//! names follow ([`super::seagull`]).
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the fifth
//!   level owns 2140 to 2159, with `Idle2` 2140, `Block` 2141, `Withdraw`
//!   2142 and `GenericHit` 2143; the level's objects (planks, jump
//!   switches, statues) take none. Later families with a parry of their
//!   own reuse `Block` with their own handlers.
//! - The "on screen" test of the alert under a widened view follows the
//!   rule the soldier's sight test uses.
//! - A jump switch launches a guard whenever its box reaches a guard,
//!   free or busy, that passes the shared hit gate (Claw's attack serial,
//!   then the hit cooldown); the launch is the shared human jump towards
//!   the switch's target point.
//! - The sixth level's differences (the first guard's parry and hop
//!   chances, guards placed to stand still, that level's timings) are
//!   kept where the code branches on the level number, and are confirmed
//!   with that level.
//!
//! Their data ([`TownGuardData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; a guard then wraps it in its own.

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the fifth level's families. The
/// guards take the first two ([`EnemyState::Idle2`],
/// [`EnemyState::Block`]) and the seagull the next two
/// ([`EnemyState::Withdraw`], [`EnemyState::GenericHit`]); the rest are
/// free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2140..=2159;

/// Health on the fifth level, and on any other, before the difficulty
/// factor (the record's health is not used).
pub const HEALTH: (i32, i32) = (9, 10);
/// Damage of a strike or of touching a guard, before the difficulty
/// factor.
pub const DAMAGE: i32 = 10;
/// Hit box, both guards.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The contact box outside strikes: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box while Claw carries a guard.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (110, 40), (100, 40)];
/// The chance in 100 of hopping back from Claw's sword when a guard is
/// built: the first guard's on the fifth level (90 on the sixth), and the
/// second guard's.
pub const JUMP_BACK_CHANCE: (i32, i32) = (0, 90);
/// The high strike, at a standing Claw: its animation (both guards play
/// the first guard's).
pub const HIGH_STRIKE_ANIM: &str = "LEVEL_TOWNGUARD1_STRIKE2";
/// The low strike, at a crouching Claw, which is also the parry pose.
pub const LOW_STRIKE_ANIM: &str = "LEVEL_TOWNGUARD1_STRIKE1";

/// What differs between the two guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guard {
    /// Logic name.
    pub name: &'static str,
    /// Image set.
    pub image_set: &'static str,
    /// The high strike's box, facing left.
    pub high_strike: Rect,
    /// The low strike's box, facing left.
    pub low_strike: Rect,
    /// Movement box.
    pub move_rect: Rect,
}

/// `TownGuard1`.
pub const GUARD1: Guard = Guard {
    name: "TownGuard1",
    image_set: "LEVEL_TOWNGUARD1",
    high_strike: Rect::new(-88, -35, -20, 7),
    low_strike: Rect::new(-90, 4, -22, 35),
    move_rect: Rect::new(-20, -60, 20, 51),
};

/// `TownGuard2`.
pub const GUARD2: Guard = Guard {
    name: "TownGuard2",
    image_set: "LEVEL_TOWNGUARD2",
    high_strike: Rect::new(-88, -14, -21, 27),
    low_strike: Rect::new(-102, 7, -33, 46),
    move_rect: Rect::new(-20, -40, 20, 71),
};

/// The guard running the logic `name`.
pub fn guard(name: &str) -> Option<&'static Guard> {
    [&GUARD1, &GUARD2].into_iter().find(|g| g.name == name)
}

/// Health of a guard built on level `level`, before the difficulty
/// factor.
pub fn health(level: u32) -> i32 {
    if level == 5 { HEALTH.0 } else { HEALTH.1 }
}

/// A town guard's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TownGuardData {
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

/// The first town guard's class.
pub mod guard1 {
    use super::{Class, ClassAnims, ClassSounds, GUARD1, handler, nothing};

    pub use super::{BUILT, on_attack, on_hit, tick};

    /// Logic names this class runs.
    pub const NAMES: &[&str] = &[GUARD1.name];

    /// Image sets this class may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[GUARD1.image_set];

    /// Prefixes of the animations this class may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_TOWNGUARD1_"];

    /// The `TownGuard1` class.
    pub static CLASS: Class = Class {
        name: GUARD1.name,
        human: true,
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
}

/// The second town guard's class. It plays the first guard's strikes and
/// parry pose with its own image set.
pub mod guard2 {
    use super::{Class, ClassAnims, ClassSounds, GUARD2, handler, nothing};

    pub use super::{BUILT, on_attack, on_hit, tick};

    /// Logic names this class runs.
    pub const NAMES: &[&str] = &[GUARD2.name];

    /// Image sets this class may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[GUARD2.image_set];

    /// Prefixes of the animations this class may play: its own and the
    /// first guard's (its strikes and parry pose).
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_TOWNGUARD2_", "LEVEL_TOWNGUARD1_"];

    /// The `TownGuard2` class.
    pub static CLASS: Class = Class {
        name: GUARD2.name,
        human: true,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn their_new_states_open_the_fifth_levels_block() {
        assert_eq!(EnemyState::Idle2 as i32, *STATE_BLOCK.start());
        assert_eq!(EnemyState::Block as i32, STATE_BLOCK.start() + 1);
        for s in [EnemyState::Idle2, EnemyState::Block] {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
    }

    #[test]
    fn each_class_names_its_own_guard() {
        assert_eq!(guard("TownGuard1"), Some(&GUARD1));
        assert_eq!(guard("TownGuard2"), Some(&GUARD2));
        assert_eq!(guard("Officer"), None);
        assert_eq!(
            (guard1::CLASS.name, guard2::CLASS.name),
            (GUARD1.name, GUARD2.name)
        );
        assert!(guard1::CLASS.human && guard2::CLASS.human);
        for (names, sets, prefixes, g) in [
            (
                guard1::NAMES,
                guard1::IMAGE_SETS,
                guard1::ANIM_PREFIXES,
                GUARD1,
            ),
            (
                guard2::NAMES,
                guard2::IMAGE_SETS,
                guard2::ANIM_PREFIXES,
                GUARD2,
            ),
        ] {
            assert_eq!(names, [g.name]);
            assert_eq!(sets, [g.image_set]);
            // Both play the first guard's strikes and parry pose.
            for anim in [HIGH_STRIKE_ANIM, LOW_STRIKE_ANIM] {
                assert!(prefixes.iter().any(|p| anim.starts_with(p)), "{anim}");
            }
        }
    }

    #[test]
    fn strikes_reach_ahead_of_a_guard_facing_left() {
        for g in [GUARD1, GUARD2] {
            for r in [g.high_strike, g.low_strike] {
                assert!(r.right < 0 && r.left < r.right, "{}: {r:?}", g.name);
                assert!(r.top < r.bottom, "{}: {r:?}", g.name);
            }
            // The low strike reaches lower than the high one.
            assert!(g.low_strike.bottom > g.high_strike.bottom, "{}", g.name);
        }
    }

    #[test]
    fn health_is_nine_on_the_fifth_level_only() {
        assert_eq!(health(5), 9);
        assert_eq!(health(6), 10);
        assert_eq!(health(1), 10);
    }
}
