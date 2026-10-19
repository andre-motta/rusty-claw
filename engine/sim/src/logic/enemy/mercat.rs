//! Mercat, the trident-carrying merman of the eleventh and twelfth levels.
//!
//! He is a human enemy who fights mostly from range. Whenever Claw is in
//! his sight and his wait of one to two seconds has run out, he raises
//! his trident and, on the tenth tick of the throw ([`BOLT_TICK`]), throws
//! a bolt of lightning (the trident bolt,
//! [`crate::logic::projectile::trident`]) from [`BOLT_OFFSET`] of his
//! position: it flies straight at [`BOLT_SPEED`] px/s, hurts Claw for
//! [`BOLT_DAMAGE`] and bursts into a short explosion that also hurts the
//! human enemies in it, him included when Claw stands close in front of
//! him. He throws at any distance while Claw is in sight. Only when Claw
//! is within 150 px but out of sight does he stab instead
//! ([`STAB`], [`DAMAGE`]); farther and out of sight he walks in at
//! [`ADVANCE_SPEED`] px/s or runs at [`FAST_ADVANCE_SPEED`] px/s.
//!
//! He parries every crouching sword stab and about a quarter of pistol
//! shots on the eleventh level ([`SHOT_PARRY_CHANCE`]), and while Claw
//! swings at him from within 150 px facing him he hops back 100 px with a
//! chance of 90, 60, 30, then 0 percent ([`HOP_CHANCE`], [`HOP_STEP`]),
//! reset when he loses track of Claw. He has [`HEALTH`] health on the
//! eleventh level ([`health`]), can be lifted and drops his record's loot.
//!
//! Not built yet ([`BUILT`] is false): he stays an inert placeholder
//! (drawn, no collision, reported as not implemented) and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he idles in the shared
//! `Idle`, decides in `Home`, throws in the shared `Fire`, stabs in
//! `Strike`, parries in the town guards' `Block` and hops back in the
//! shared `JumpBack`; he needs no new number. The eleventh level's block
//! [`STATE_BLOCK`] (2260 to 2279) holds the Fish's and the Siren's new
//! names from 2270 ([`super::fish::STATES`], [`super::siren::STATES`]).
//!
//! # Defaults taken
//!
//! - The eleventh level's block of state numbers: blocks of 20 in level
//!   order, so the eleventh level owns 2260 to 2279; the Fish's swim, leap
//!   and flop take 2270 to 2272 and the Siren's song 2273, and the numbers
//!   below 2270 stay free. The Fish's dash is the crabs' `Snip` (2195),
//!   run with the Fish's own handler: one name keeps one number whichever
//!   class runs it.
//! - Whether he is on screen, for his idle and the shared human pre-tick,
//!   under a display wider than the original's: the rule the built
//!   soldier and cutthroat use, the object having been drawn in the last
//!   frame.
//! - A laser's beam reaches him through the shared human hit mask
//!   ([`super::HUMAN_HIT_BY`]) and the shared reaction to the level's own
//!   shots: no health lost (a beam deals 0), a flinch.
//!
//! Its data ([`MercatData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Mercat"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_MERCAT";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_MERCAT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the eleventh level's families: the
/// Fish's ([`super::fish::STATES`]) and the Siren's
/// ([`super::siren::STATES`]) from 2270; the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2260..=2279;

/// The states he runs with his own handler, in the order the game
/// registers them; none is new.
pub const STATES: [EnemyState; 4] = [
    EnemyState::Idle,
    EnemyState::Home,
    EnemyState::Fire,
    EnemyState::Block,
];

/// His health on the eleventh level, before the difficulty factor (the
/// record's is not used).
pub const HEALTH: i32 = 9;
/// His health on every other level, before the difficulty factor.
pub const HEALTH_ELSEWHERE: i32 = 15;
/// Damage of touching him and of his stab, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// His smarts at the default factor.
pub const SMARTS: i32 = 99;

/// His stab's box, facing left.
pub const STAB: Rect = Rect::new(-111, -5, -22, 22);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: the first picture's.
pub const MOVE_RECT: Rect = Rect::new(-51, -69, 40, 50);

/// Walking speed towards Claw, px/s.
pub const ADVANCE_SPEED: i32 = 150;
/// Running speed towards Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 300;

/// Where the bolt leaves, from his position, facing right; mirrored when
/// he faces left.
pub const BOLT_OFFSET: (i32, i32) = (28, 0);
/// The bolt's speed, px/s.
pub const BOLT_SPEED: i32 = 400;
/// The bolt's damage, not scaled by the difficulty.
pub const BOLT_DAMAGE: i32 = 20;
/// The tick of the throw on which the bolt leaves, counted from the
/// throw's first tick at the 18 ms step (measured).
pub const BOLT_TICK: u32 = 10;
/// The wait between bolts on the eleventh level, ms.
pub const BOLT_COOLDOWN_MS: u32 = 1000;
/// The wait between bolts on every other level, ms.
pub const BOLT_COOLDOWN_ELSEWHERE_MS: u32 = 750;
/// His wait after going home, ms: this plus a draw of 0..=1000.
pub const HOME_WAIT_MS: u32 = 1000;
/// Within this distance and out of sight he stabs, px.
pub const STAB_RANGE: i32 = 150;

/// His first chance in 100 to hop back from a swing.
pub const HOP_CHANCE: i32 = 90;
/// What each hop costs the chance.
pub const HOP_STEP: i32 = 30;
/// How far a hop takes him back, px.
pub const HOP_DISTANCE: i32 = 100;
/// The least time between two hop draws, ms.
pub const HOP_WAIT_MS: u32 = 500;
/// The chance in 101 that he parries a pistol shot on the eleventh level
/// (25) and on the twelfth (50).
pub const SHOT_PARRY_CHANCE: (i32, i32) = (25, 50);

/// The decision pose.
pub const HOME_ANIM: &str = "LEVEL_MERCAT_HOME";
/// The first idle pose (three more follow it).
pub const IDLE_ANIM: &str = "LEVEL_MERCAT_IDLE1";
/// The throw, and the stab.
pub const STRIKE_ANIM: &str = "LEVEL_MERCAT_STRIKE1";
/// The parry.
pub const BLOCK_ANIM: &str = "LEVEL_MERCAT_BLOCK";
/// The hop back.
pub const JUMP_ANIM: &str = "LEVEL_MERCAT_JUMP";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_MERCAT_KILLFALL";
/// The bolt's sound.
pub const BOLT_SOUND: &str = "LEVEL_MERCAT_TLIGHTNING";
/// The stab's sound.
pub const STAB_SOUND: &str = "LEVEL_MERCAT_STAB";
/// The parry's sounds, one of two.
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_MERCAT_BLOCK1", "LEVEL_MERCAT_BLOCK2"];

/// His health on level `level`, before the difficulty factor.
pub fn health(level: u32) -> i32 {
    if level == 11 {
        HEALTH
    } else {
        HEALTH_ELSEWHERE
    }
}

/// The wait between his bolts on level `level`, ms.
pub fn bolt_cooldown_ms(level: u32) -> u32 {
    if level == 11 {
        BOLT_COOLDOWN_MS
    } else {
        BOLT_COOLDOWN_ELSEWHERE_MS
    }
}

/// The Mercat class.
pub static CLASS: Class = Class {
    name: "Mercat",
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

/// A Mercat's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MercatData {
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
    fn the_eleventh_levels_block_follows_the_tenths() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2260, 2279));
        assert_eq!(
            *super::super::marrow::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        // He needs no new number.
        for s in STATES {
            assert!(!STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
        assert_eq!(EnemyState::Fire as i32, 2013);
        assert_eq!(EnemyState::Block as i32, 2141);
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Home).is_none(), "not built yet");
    }

    #[test]
    fn his_numbers_follow_the_level() {
        assert_eq!((health(11), health(12)), (9, 15));
        assert_eq!((bolt_cooldown_ms(11), bolt_cooldown_ms(12)), (1000, 750));
        // The bolt flies 7 or 8 px a tick at the 18 ms step.
        assert_eq!(BOLT_SPEED * 18, 7_200);
        // Two hits of 5 kill him on the eleventh level.
        const { assert!(HEALTH <= 2 * 5 && HEALTH > 5) };
        // Four hop draws take the chance to 0.
        assert_eq!(HOP_CHANCE - 3 * HOP_STEP, 0);
        // His stab reaches 111 px ahead of him.
        assert_eq!(-STAB.left, 111);
    }

    #[test]
    fn his_art_is_his_own() {
        for key in [
            HOME_ANIM,
            IDLE_ANIM,
            STRIKE_ANIM,
            BLOCK_ANIM,
            JUMP_ANIM,
            DEATH_ANIM,
            BOLT_SOUND,
            STAB_SOUND,
        ]
        .iter()
        .chain(&BLOCK_SOUNDS)
        {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
