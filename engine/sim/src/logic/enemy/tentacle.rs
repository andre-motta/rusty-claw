//! The tentacles of the twelfth level: Aquatis's ([`super::aquatis`])
//! three green arms rising out of the water along the ledge where Claw
//! arrives for his fight.
//!
//! A tentacle stays rooted in place. When Claw is up to [`REACH`] px to
//! its left and it has idled a second or so, it either whips sideways at
//! him (three times in four, [`STRIKE_BOX`] for [`DAMAGE`]) or grabs him
//! ([`EnemyState::StartGrab`]): Claw is frozen where he stands, lifted
//! after [`START_GRAB_TICKS`] ticks to [`HOLD_AT`] from the tentacle and
//! squeezed ([`EnemyState::Squeeze`]) for one health every
//! [`SQUEEZE_STEP_MS`] until he mashes his way out ([`MASH_PRESSES`]
//! ticks with a fresh press), [`SQUEEZE_MS`] pass, or he dies. A second
//! tentacle can grab a Claw the first still holds, and any strike on him
//! ends every hold.
//!
//! It has no real health: any single hit puts it down, and it grows back
//! ([`EnemyState::Respawn`]) after [`REGROW_MS`] or at once when Aquatis
//! starts a frenzy. During his frenzy every tentacle thrashes
//! ([`EnemyState::SuperFrenzy`]), swiping left and right, and nothing can
//! hurt it; facing right, its box reaches everything to its right on its
//! own row. When Aquatis dies the tentacles die with him. They talk to him
//! only through the Aquatis link ([`super::aquatis::AquatisLink`]), whose
//! tentacle hold ([`crate::logic::World::tentacle_grab`]) they share.
//!
//! Not built yet ([`BUILT`] is false): the tentacles stay inert
//! placeholders (drawn, no collision, reported as not implemented) and the
//! calls below do nothing.
//!
//! # State numbers
//!
//! It idles in `Idle`, strikes in `Strike` and dies in `Dead`; it thrashes
//! in Aquatis's [`EnemyState::SuperFrenzy`] and its three new names take
//! 2290 to 2292 in the twelfth level's block
//! ([`super::aquatis::STATE_BLOCK`]).
//!
//! # Defaults taken
//!
//! - Its order against Claw in the update pass: the shared enemy depth
//!   sort against Claw's feet ([`super::motion::DEPTH_BEHIND`] while Claw
//!   is below it in the passage under the water, so it updates before
//!   him; [`super::motion::DEPTH_IN_FRONT`] once he is above, so after
//!   him), which is what the original's runs show (3899 and 4000).
//! - Claw's crushed pictures while it squeezes him are his own frames 450
//!   to 452, shown through a world call that sets the frame he shows for
//!   the tick.
//!
//! Its data ([`TentacleData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Tentacle"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_AQUATISTENTACLE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_AQUATISTENTACLE_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Its new states.
pub const STATES: [EnemyState; 3] = [
    EnemyState::StartGrab,
    EnemyState::Squeeze,
    EnemyState::Respawn,
];

/// Damage of its strike.
pub const DAMAGE: i32 = 5;
/// The strike's box facing left, on the strike's picture only.
pub const STRIKE_BOX: Rect = Rect::new(-136, -44, 0, 39);
/// What may hit it: Claw, his pistol shots, the level's blasts and magic.
pub const HIT_MASK: u32 = 0x300_0022;
/// How far left of it Claw must be for an attack, px.
pub const REACH: i32 = 200;
/// Ticks of the grab before Claw is lifted, at the 18 ms step (measured).
pub const START_GRAB_TICKS: u32 = 18;
/// Where the held Claw is put, from the tentacle.
pub const HOLD_AT: (i32, i32) = (50, -70);
/// The longest squeeze, ms.
pub const SQUEEZE_MS: u32 = 3000;
/// Time between two points of damage while it squeezes, ms.
pub const SQUEEZE_STEP_MS: u32 = 200;
/// Ticks with a fresh press that free Claw.
pub const MASH_PRESSES: u32 = 10;
/// Time a cut-down tentacle waits before it grows back, ms.
pub const REGROW_MS: u32 = 20000;

/// Its idle sway.
pub const IDLE_ANIM: &str = "LEVEL_AQUATISTENTACLE_IDLE";
/// The strike, and each swipe of the frenzy.
pub const STRIKE_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE1";
/// The grab.
pub const GRAB_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE2";
/// The squeeze.
pub const SQUEEZE_ANIM: &str = "LEVEL_AQUATISTENTACLE_STRIKE3";

/// The tentacle class.
pub static CLASS: Class = Class {
    name: "Tentacle",
    human: false,
    construct: nothing,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle: nothing,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims::NONE,
    sounds: ClassSounds::NONE,
};

/// A tentacle's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TentacleData {
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
    use super::super::aquatis;
    use super::*;

    #[test]
    fn its_states_follow_aquatis_in_the_twelfth_levels_block() {
        let ids: Vec<i32> = STATES.iter().map(|&s| s as i32).collect();
        assert_eq!(ids, [2290, 2291, 2292]);
        for id in ids {
            assert!(aquatis::STATE_BLOCK.contains(&id));
        }
        assert!(!CLASS.human, "a plain enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        assert!(handler(EnemyState::Idle).is_none(), "not built yet");
    }

    #[test]
    fn a_squeeze_costs_at_most_fourteen_health_at_eighteen_ms() {
        // One point on its first tick, then one every 12 ticks (216 ms),
        // until 3000 ms have passed: 167 ticks.
        let step = SQUEEZE_STEP_MS.div_ceil(18);
        assert_eq!(step, 12);
        let ticks = SQUEEZE_MS.div_ceil(18);
        assert_eq!(ticks, 167);
        assert_eq!((ticks - 1) / step + 1, 14);
        // The strike reaches 136 px to its left.
        assert_eq!(-STRIKE_BOX.left, 136);
    }
}
