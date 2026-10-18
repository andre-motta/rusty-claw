//! PegLeg, the peg-legged gunner of the ninth level (and of the tenth),
//! with the bullet he fires ([`bullet`]).
//!
//! He is a human enemy who never swings at Claw. He fires a bullet from
//! 150 to 200 px away (it leaves on the 19th tick of the shot, the shot
//! ends on the 31st), and when Claw comes closer than 150 px he rolls
//! [`ROLL_DISTANCE`] px away from him ([`EnemyState::RollBackward`]) and
//! fires again. From 200 to 500 px he walks in or rolls towards Claw
//! ([`EnemyState::RollForward`]). Rolls are [`ROLL_SPEED`] px/s, stop on
//! their target, on a patrol bound or after [`ROLL_MS`], and nothing hurts
//! him while he rolls.
//!
//! On the ninth level he aims each shot at Claw's posture (low when Claw
//! ducks) and stops a forward roll [`ROLL_STOP_SHORT`] px short of Claw.
//! Pistol shots never hurt him: they make him roll at Claw. Standing sword
//! stabs, punches and kicks land only half the time, a crouching stab
//! always lands and opens a window of [`CROUCH_WINDOW_MS`] in which the
//! next blow always lands, and a throw never costs him health: after
//! landing from one he pauses, taunts and fires six shots
//! [`FIRE_WINDOW_SHOT_TICKS`] ticks apart, alternating high and low. He has
//! [`HEALTH`] health and drops his record's loot.
//!
//! Not built yet ([`BUILT`] is false): PegLegs stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he decides in `Home` and
//! shoots in `Fire` (2013) with handlers of his own and inherits every
//! other shared human state with its number. His two new names,
//! [`EnemyState::RollForward`] and [`EnemyState::RollBackward`], take the
//! first two numbers of the ninth level's block
//! ([`super::hook_pirate::STATE_BLOCK`]).
//!
//! # Defaults taken
//!
//! - The ninth level's block of state numbers: rolls at 2220 and 2221 (see
//!   [`super::hook_pirate`]).
//! - The "on screen" test of his sight under a widened view follows the
//!   rule the soldier's sight test uses.
//! - The entry tick of a forward roll moves him right at [`ROLL_SPEED`]
//!   whichever side Claw is on (a patrol bound still holds him), as the
//!   original does; the right sign comes from the next tick.
//!
//! Its data ([`PegLegData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PegLeg"];

/// His picture, set when he is built (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_PEGLEG";

/// Image sets this family may switch an object to: his own and his
/// bullet's.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, bullet::IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_PEGLEG_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// His two new states, in number order.
pub const STATES: [EnemyState; 2] = [EnemyState::RollForward, EnemyState::RollBackward];

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 7;
/// Damage of touching him, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Hit box (none while he rolls forward).
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: the box of his first picture.
pub const MOVE_RECT: Rect = Rect::new(-12, -43, 39, 50);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-37, 17, 48, 49);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];
/// Speed of a roll, px/s.
pub const ROLL_SPEED: i32 = 250;
/// Length of a roll, px.
pub const ROLL_DISTANCE: i32 = 150;
/// How far short of Claw a forward roll stops on the ninth level, px.
pub const ROLL_STOP_SHORT: i32 = 35;
/// The longest a roll lasts, ms.
pub const ROLL_MS: u32 = 1500;
/// Ticks of a 150 px roll at the 18 ms step (measured).
pub const ROLL_TICKS: u32 = 34;
/// Time after a bullet before he can fire again, ms.
pub const FIRE_COOLDOWN_MS: u32 = 1000;
/// The firing spell after a taunt, ms.
pub const FIRE_WINDOW_MS: u32 = 3000;
/// Ticks between two shots of the firing spell at the 18 ms step
/// (measured).
pub const FIRE_WINDOW_SHOT_TICKS: u32 = 31;
/// Shots of the firing spell (measured).
pub const FIRE_WINDOW_SHOTS: u32 = 6;
/// The window after a crouching stab lands in which every blow lands, ms.
pub const CROUCH_WINDOW_MS: u32 = 2000;
/// The bullet's tick and the shot's last tick, counted from the shot's
/// first tick (measured).
pub const SHOT_TICKS: (u32, u32) = (19, 31);
/// Ticks of the high flinch at the 18 ms step (measured).
pub const HIT_HIGH_TICKS: u32 = 19;
/// Ticks of the low flinch after a crouching stab at the 18 ms step
/// (measured).
pub const HIT_LOW_TICKS: u32 = 17;
/// Ticks of the pause after landing from a throw, at the 18 ms step
/// (measured).
pub const THROW_PAUSE_TICKS: u32 = 28;

/// The high shot, at a standing Claw.
pub const HIGH_SHOT_ANIM: &str = "LEVEL_PEGLEG_STRIKE1";
/// The low shot, at a ducking Claw.
pub const LOW_SHOT_ANIM: &str = "LEVEL_PEGLEG_STRIKE2";
/// The forward roll.
pub const FORWARD_ROLL_ANIM: &str = "LEVEL_PEGLEG_FORWARDROLL";
/// The backward roll.
pub const BACKWARD_ROLL_ANIM: &str = "LEVEL_PEGLEG_BACKWARDROLL";
/// The idle pose, shown in `Home`.
pub const IDLE_ANIM: &str = "LEVEL_PEGLEG_IDLE";
/// The shot's sound.
pub const GUNSHOT_SOUND: &str = "LEVEL_PEGLEG_SOLGUNFR8-22";
/// A roll's sound.
pub const ROLL_SOUND: &str = "LEVEL_PEGLEG_ROLL1";

/// Whether the PegLeg of level `level` aims at Claw's posture and stops
/// short of him: on the ninth level; on the tenth he fires pairs,
/// alternating, and rolls the full length.
pub fn aims_at_posture(level: u32) -> bool {
    level == 9
}

/// The bullet PegLeg fires: the soldier's bullet with Claw's pistol
/// picture, an enemy's shot that flies straight at [`SPEED`](bullet::SPEED)
/// px/s (9 px a tick) and costs Claw [`DAMAGE`](bullet::DAMAGE). It leaves
/// [`MUZZLE_HIGH`](bullet::MUZZLE_HIGH) or [`MUZZLE_LOW`](bullet::MUZZLE_LOW)
/// from him, mirrored when he faces left. It is not a family of its own:
/// the projectile family runs it.
pub mod bullet {
    /// Logic name of the bullet.
    pub const LOGIC: &str = "Bullet";
    /// Its picture.
    pub const IMAGE_SET: &str = "GAME_BULLETS";
    /// Damage to Claw.
    pub const DAMAGE: i32 = 10;
    /// Speed, px/s.
    pub const SPEED: i32 = 500;
    /// Where the high shot leaves, facing right.
    pub const MUZZLE_HIGH: (i32, i32) = (55, -24);
    /// Where the low shot leaves, facing right.
    pub const MUZZLE_LOW: (i32, i32) = (55, 15);

    /// Where a shot leaves, from PegLeg's position, and its speed: high or
    /// low (`low`), facing right or left (`right`).
    pub fn start(low: bool, right: bool) -> ((i32, i32), i32) {
        let (x, y) = if low { MUZZLE_LOW } else { MUZZLE_HIGH };
        if right {
            ((x, y), SPEED)
        } else {
            ((-x, y), -SPEED)
        }
    }
}

/// The PegLeg class.
pub static CLASS: Class = Class {
    name: "PegLeg",
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

/// A PegLeg's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PegLegData {
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
    fn his_rolls_are_in_the_ninth_levels_block() {
        let block = super::super::hook_pirate::STATE_BLOCK;
        assert_eq!(STATES.map(|s| s as i32), [2220, 2221]);
        for s in STATES {
            assert!(block.contains(&(s as i32)), "{}", s.name());
        }
        assert_eq!(EnemyState::Fire as i32, 2013);
        assert!(CLASS.human, "a human enemy");
    }

    #[test]
    fn his_bullet_leaves_ahead_of_him_high_or_low() {
        assert_eq!(bullet::start(false, true), ((55, -24), 500));
        assert_eq!(bullet::start(true, true), ((55, 15), 500));
        assert_eq!(bullet::start(false, false), ((-55, -24), -500));
        assert_eq!(bullet::start(true, false), ((-55, 15), -500));
        // 9 px a tick, rounded.
        assert_eq!((bullet::SPEED * 18 + 500) / 1000, 9);
    }

    #[test]
    fn his_timings_at_eighteen_milliseconds() {
        // 150 px at 250 px/s: 34 ticks of 4.5 px.
        assert_eq!(
            ((ROLL_DISTANCE * 1000) as u32).div_ceil((ROLL_SPEED * 18) as u32),
            ROLL_TICKS
        );
        assert!(ROLL_TICKS < ROLL_MS.div_ceil(18));
        // The spell fires again whenever a shot ends inside its 3 s: the
        // sixth shot starts inside it and ends after it.
        let window = FIRE_WINDOW_MS.div_ceil(18);
        assert!((FIRE_WINDOW_SHOTS - 1) * FIRE_WINDOW_SHOT_TICKS < window);
        assert!(FIRE_WINDOW_SHOTS * FIRE_WINDOW_SHOT_TICKS >= window);
        assert_eq!(SHOT_TICKS.1, FIRE_WINDOW_SHOT_TICKS);
        assert!(aims_at_posture(9));
        assert!(!aims_at_posture(10));
    }
}
