//! RedTail, the pirate captain who ends the thirteenth level on a log
//! stage over the water, with his stakes, his wind, his knives and his
//! flares ([`crate::logic::boss`]).
//!
//! He is a human enemy on the shared boss staging
//! ([`crate::logic::boss_stage`]): the warp takes Claw into his arena, the
//! staged introduction freezes Claw, and the fight starts when the flag
//! reaches 2. His arena has two levels: a ledge on the right, where he
//! starts, and the sand below it. Before every attack he asks where Claw
//! stands: on his own level he fires his flare gun, slides along the sand
//! at Claw or kicks him; on the other level he throws knives, leaps down
//! or up to Claw's level, or (from the ledge, Claw on the sand) raises
//! the wind that drives Claw into the stakes at the left wall.
//!
//! He has [`HEALTH`] health, each damaging blow costing him
//! [`BLOW_DAMAGE`]. Claw's blows are counted: every third counted blow
//! that finds him free makes him leap to the other level instead, unhurt.
//! The count starts at [`HIT_COUNTER_START`] on the first arena step,
//! the value the original game shows there (its set-up never writes the
//! count, which so reads what the memory held); a count of 1 at the start
//! means the second counted blow after an intro skip already sends him
//! away. Matching the original here is a stand-in decision.
//!
//! What his helpers share with him lives in the RedTail link
//! ([`RedTailLink`], reached through [`World::redtail_link`] and
//! [`World::redtail_link_mut`]): his object while he is there and the
//! wind flag he raises and the wind reads.
//!
//! # State numbers
//!
//! The thirteenth level's block [`STATE_BLOCK`] (2300 to 2319) holds his
//! new names from 2300 ([`STATES`]: the knives, the kick, the slide, the
//! gun, the wind and the two jumps) and the Chameleon's from 2310
//! ([`super::chameleon::STATES`]). He reuses the numbers of names that
//! already have one: `Startup`, `Home`, `FastAdvance`, `Block`, `HitHigh`,
//! `HitLow` and `Dead`.
//!
//! Not built yet ([`BUILT`] is false): his record stays an inert
//! placeholder and the calls below do nothing.

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::{World, boss_stage};
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTail"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_REDTAIL";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_REDTAIL_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the thirteenth level's families:
/// RedTail's from 2300 ([`STATES`]) and the Chameleon's from 2310
/// ([`super::chameleon::STATES`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2300..=2319;

/// His new state names, in number order from the start of
/// [`STATE_BLOCK`].
pub const STATES: [EnemyState; 7] = [
    EnemyState::KnifeAttack,
    EnemyState::KickAttack,
    EnemyState::SlideAttack,
    EnemyState::GunAttack,
    EnemyState::WindAttack,
    EnemyState::JumpUp,
    EnemyState::JumpDown,
];

/// The staging objects of his fight.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health, before the difficulty factor.
pub const HEALTH: i32 = 100;
/// What one damaging blow of Claw's costs him (twenty kill him).
pub const BLOW_DAMAGE: i32 = 5;
/// What touching him, his kick and his slide cost Claw, before the
/// difficulty factor.
pub const DAMAGE: i32 = 10;
/// The count of Claw's blows on the first arena step (see the module
/// documentation).
pub const HIT_COUNTER_START: u32 = 1;
/// Every this many counted blows that find him free, he leaps away.
pub const ESCAPE_EVERY: u32 = 3;
/// His speed running at Claw, px/s.
pub const RUN_SPEED: i32 = 250;
/// His hit box.
pub const HIT_RECT: Rect = Rect::new(-31, -51, 38, 62);
/// His movement and contact box: his first picture's.
pub const BODY_RECT: Rect = Rect::new(-37, -36, 54, 55);
/// The slide's box, facing left.
pub const SLIDE_RECT: Rect = Rect::new(-52, -17, 51, 41);
/// The kick's box, facing left.
pub const KICK_RECT: Rect = Rect::new(-56, -28, 0, 22);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [
    (2000, 1000),
    (600, 1000),
    (300, 1000),
    (200, 1000),
    (100, 1000),
];
/// Below this height (greater y) a figure stands on the sand ("low"),
/// px.
pub const LOW_Y: i32 = 1920;
/// The patrol limits his first landing sets (min, max).
pub const LANDED_BOUNDS: (i32, i32) = (37100, 37900);
/// Where the gem he leaves flies to.
pub const GEM_TARGET: (i32, i32) = (37509, 1750);

/// His decision pose.
pub const IDLE_ANIM: &str = "LEVEL_REDTAIL_IDLE";
/// His body falling after death.
pub const DEATH_ANIM: &str = "LEVEL_REDTAIL_KILLFALL";

/// The RedTail class (not built yet: every step does nothing).
pub static CLASS: Class = Class {
    name: "RedTail",
    human: true,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: false,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// What RedTail's helpers share with him (see the module documentation).
/// It lasts the whole level and starts as [`RedTailLink::default`] leaves
/// it: no RedTail, the wind flag down.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RedTailLink {
    /// RedTail's object, while he is there.
    pub redtail: Option<ObjectId>,
    /// The wind flag: raised by his wind attack, read by the wind, which
    /// clears it when its blow ends; his respawn reset clears it too.
    pub wind: bool,
}

/// RedTail's data.
#[derive(Debug, Clone, PartialEq)]
pub struct RedTailData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The count of Claw's blows (see [`HIT_COUNTER_START`]).
    pub hits: u32,
}

impl Default for RedTailData {
    /// Data with the blow count at its start, [`HIT_COUNTER_START`].
    fn default() -> RedTailData {
        RedTailData {
            enemy: EnemyData::default(),
            hits: HIT_COUNTER_START,
        }
    }
}

impl World<'_> {
    /// The RedTail link (see [`RedTailLink`]).
    pub fn redtail_link(&self) -> &RedTailLink {
        &self.out.redtail
    }

    /// The RedTail link, mutable (see [`World::redtail_link`]).
    pub fn redtail_link_mut(&mut self) -> &mut RedTailLink {
        &mut self.out.redtail
    }
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
    fn his_states_open_the_thirteenth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2300, 2319));
        assert_eq!(
            *super::super::aquatis::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        for (i, s) in STATES.iter().enumerate() {
            assert_eq!(*s as i32, *STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Home).is_none(), "not built yet");
    }

    #[test]
    fn the_blow_count_starts_at_one_and_twenty_blows_kill_him() {
        assert_eq!(RedTailData::default().hits, 1);
        assert_eq!(HEALTH / BLOW_DAMAGE, 20);
        let l = RedTailLink::default();
        assert_eq!(l.redtail, None);
        assert!(!l.wind);
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in [IDLE_ANIM, DEATH_ANIM] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        assert!(!"LEVEL_REDTAILPIRATE_HOME".starts_with(ANIM_PREFIXES[0]));
    }
}
