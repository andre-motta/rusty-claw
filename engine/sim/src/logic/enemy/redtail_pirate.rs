//! The red-tail pirate of the seventh level (and of the eighth and the
//! thirteenth): a pirate with a sword.
//!
//! It is a human enemy built like the town guards: it idles, paces,
//! walks up to Claw at 120 px/s (240 px/s when he is far), steps in or
//! back at 150 px/s and strikes, high at a standing Claw and low at a
//! crouching one, and it reads Claw's attacks:
//!
//! - it decides on its first decision step with Claw in range, so it
//!   engages at once; the high strike ([`HIGH_STRIKE`]) is live on its 7th
//!   tick and over on its 18th, the low one ([`LOW_STRIKE`]) live on its
//!   7th and over on its 14th. Close in it strikes again every 19 ticks
//!   (high) or 15 (low); a strike from further out keeps it from striking
//!   again for [`REST_MS`];
//! - when Claw starts a sword swing or fires his pistol at it from in
//!   front, it may hop back ([`EnemyState::JumpBack`]) on the tick the
//!   attack starts, before the blow lands: a hop of 18 ticks that lands
//!   about 96 px from where it stood. The chance starts at 90 in 100 and
//!   falls by 30 with every hop ([`jump_back_chance`]), so after three
//!   hops it stands its ground;
//! - on the seventh level it never parries: every hit goes through, and a
//!   low hit makes it flinch for 19 ticks.
//!
//! It has [`HEALTH`] health (two sword blows or two pistol shots), its
//! strikes and its touch cost Claw [`DAMAGE`], it can be lifted and
//! thrown and drops its record's loot.
//!
//! Not built yet ([`BUILT`] is false): red-tail pirates stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the pirate decides in
//! `Home` with a handler of its own, parries in the town guards' `Block`
//! (only on the eighth and thirteenth levels) and inherits every other
//! shared human state with its number. Its one new name,
//! [`EnemyState::GabrielAttack`], the attack of the eighth level's boss
//! fight, takes the fourth number of the seventh level's block
//! ([`STATE_BLOCK`], shared with the bear sailor); the seventh level never
//! enters it.
//!
//! # Defaults taken
//!
//! - The eighth and thirteenth levels' differences (the parry of Claw's
//!   sword and the block of his shots, [`block_chances`], and the boss
//!   fight's mode) are kept where the code branches on the level number,
//!   and are confirmed with those levels; the seventh level runs none of
//!   them.
//! - The "on screen" test of its alert under a widened view follows the
//!   rule the soldier's sight test uses.
//!
//! Its data ([`RedTailPirateData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailPirate"];

/// Its picture, set when it is built (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_REDTAILPIRATE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_REDTAILPIRATE_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers of the seventh level's families (see
/// [`super::bear_sailor::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::bear_sailor::STATE_BLOCK;

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 10;
/// Damage of a strike and of touching it, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Walking speed towards Claw, and pacing speed, px/s.
pub const ADVANCE_SPEED: i32 = 120;
/// Running speed towards a far Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 240;
/// Speed of a step in or back, px/s.
pub const STEP_SPEED: i32 = 150;
/// The high strike's box, at a standing Claw, facing left.
pub const HIGH_STRIKE: Rect = Rect::new(-96, -50, -20, 50);
/// The low strike's box, at a crouching Claw, facing left.
pub const LOW_STRIKE: Rect = Rect::new(-96, 29, 4, 74);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The contact box outside strikes: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first,
/// outside the eighth level's boss fight.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (200, 40), (200, 40), (100, 40), (60, 40)];

/// The high strike.
pub const HIGH_STRIKE_ANIM: &str = "LEVEL_REDTAILPIRATE_STRIKE1";
/// The low strike.
pub const LOW_STRIKE_ANIM: &str = "LEVEL_REDTAILPIRATE_STRIKE2";

/// Time a strike made from further out keeps it from striking again, ms.
pub const REST_MS: u32 = 2000;
/// The hop-back chance in 100 of a new pirate.
pub const FIRST_JUMP_BACK_CHANCE: i32 = 90;
/// How much each hop lowers the hop-back chance.
pub const JUMP_BACK_CHANCE_STEP: i32 = 30;
/// Ticks of a hop back at the 18 ms step.
pub const HOP_TICKS: u32 = 18;
/// Ticks of the low flinch at the 18 ms step.
pub const HIT_LOW_TICKS: u32 = 19;

/// The hop-back chance in 100 after `hops` hops: 90, 60, 30, then 0.
pub fn jump_back_chance(hops: u32) -> i32 {
    let fall = JUMP_BACK_CHANCE_STEP.saturating_mul(hops.min(10) as i32);
    (FIRST_JUMP_BACK_CHANCE - fall).max(0)
}

/// The chances in 100 that a pirate of level `level` parries one of
/// Claw's sword blows and blocks one of his shots; `None` on the levels
/// where it does neither (the seventh among them).
pub fn block_chances(level: u32) -> Option<(i32, i32)> {
    match level {
        8 => Some((25, 25)),
        13 => Some((50, 75)),
        _ => None,
    }
}

/// The strike box for a Claw who crouches (`crouching`) or not, facing
/// left.
pub fn strike_box(crouching: bool) -> Rect {
    if crouching { LOW_STRIKE } else { HIGH_STRIKE }
}

/// The red-tail pirate class.
pub static CLASS: Class = Class {
    name: "RedTailPirate",
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

/// A red-tail pirate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RedTailPirateData {
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
    fn the_hop_back_chance_falls_by_thirty_a_hop() {
        let chances: Vec<i32> = (0..6).map(jump_back_chance).collect();
        assert_eq!(chances, [90, 60, 30, 0, 0, 0]);
        assert_eq!(jump_back_chance(u32::MAX), 0);
    }

    #[test]
    fn it_parries_only_on_the_eighth_and_thirteenth_levels() {
        assert_eq!(block_chances(7), None);
        assert_eq!(block_chances(8), Some((25, 25)));
        assert_eq!(block_chances(13), Some((50, 75)));
        for level in [1, 6, 9, 14] {
            assert_eq!(block_chances(level), None, "{level}");
        }
    }

    #[test]
    fn its_strike_follows_claws_posture() {
        assert_eq!(strike_box(false), HIGH_STRIKE);
        assert_eq!(strike_box(true), LOW_STRIKE);
        // Both reach 96 px ahead; the low one lies below the high one's
        // middle.
        assert_eq!(HIGH_STRIKE.left, LOW_STRIKE.left);
        const { assert!(LOW_STRIKE.top > 0 && LOW_STRIKE.bottom > HIGH_STRIKE.bottom) };
    }

    #[test]
    fn its_one_state_is_in_the_seventh_levels_block() {
        assert!(STATE_BLOCK.contains(&(EnemyState::GabrielAttack as i32)));
        assert_eq!(EnemyState::GabrielAttack as i32, STATE_BLOCK.start() + 3);
        assert!(CLASS.human, "a human enemy");
        // Steps in and back are faster than the walk, slower than the run.
        const { assert!(ADVANCE_SPEED < STEP_SPEED && STEP_SPEED < FAST_ADVANCE_SPEED) };
        // The rest after a strike from further out: 112 ticks.
        assert_eq!(REST_MS.div_ceil(18), 112);
    }
}
