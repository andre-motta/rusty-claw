//! The bear sailor of the seventh level (and, unchanged, of the eighth and
//! the thirteenth): a big sailor who walks up to Claw, swings an overhead
//! blow at him and, when Claw comes close enough, grabs him in a hug.
//!
//! It is a human enemy: it idles, paces, advances at 120 px/s (300 px/s
//! when Claw is far), flinches from Claw's blows, can be lifted and
//! thrown, and drops its record's loot. It never blocks and never hops
//! back. It has [`HEALTH`] health, and its blow and its touch cost Claw
//! [`DAMAGE`].
//!
//! Its decision step stops it about 99 px from a standing Claw, where it
//! alternates pauses of 20 to 33 ticks with its overhead blow, live from
//! the blow's 16th tick (the [`STRIKE`] box, which reaches a Claw standing
//! 30 px away) and over on its 35th. When Claw is within its innermost
//! range box ([`RANGES`], 50 px across and 10 px up or down) at a decision
//! step, Claw has health left and the 50 ms since its last hug have run
//! out, it grabs him instead, whatever Claw is doing.
//!
//! # The hug
//!
//! - The grab ([`EnemyState::StartHug`]): Claw is put in his held state
//!   ([`crate::player::hug`]), its grab sound plays and its grab animation
//!   ([`GRAB_ANIM`]) runs for 19 ticks; then Claw is placed [`HUG_X`] px
//!   in front of it at its own height, facing it, and the hug starts.
//! - The hug ([`EnemyState::Hug`]): every [`SQUEEZE_MS`] Claw loses one
//!   health point, whatever the difficulty, starting on its first tick, so
//!   a full hug costs him 28 at the 18 ms step. While held Claw shows one
//!   of three pictures that follow the bear's squeeze ([`held_frame`]). The
//!   hug ends after [`HOLD_MS`], when Claw has pressed [`ESCAPE_PRESSES`]
//!   keys during it (presses during the grab do not count), when Claw
//!   leaves the held state (a hit from something else), or when a squeeze
//!   empties his health, which kills him. Released, Claw stands where he
//!   was held and drops the 1 px to the floor.
//! - After a hug the bear goes back to its decision step and may grab
//!   again once its next pause is over.
//!
//! Not built yet ([`BUILT`] is false): bear sailors stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the bear decides in `Home`
//! with a handler of its own and inherits every other shared human state
//! with its number. Its two new names take the first two numbers of the
//! seventh level's block [`STATE_BLOCK`]: [`EnemyState::StartHug`] and
//! [`EnemyState::Hug`]; [`EnemyState::Release`] (third) is reserved for a
//! release state no rule enters, and [`EnemyState::GabrielAttack`]
//! (fourth) is the red-tail pirate's ([`super::redtail_pirate`]). The
//! crabs' states take the block's upper half ([`super::crab`]).
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   seventh level owns 2180 to 2199, the bear's and the pirate's states
//!   from 2180 and the crabs' from 2190.
//! - The "on screen" test of its alert under a widened view follows the
//!   rule the soldier's sight test uses.
//! - The hug's loop sound plays at the configured sound volume and stops
//!   when the hug ends.
//! - The eighth and thirteenth levels' bears run this code unchanged;
//!   those levels add only tests and table data.
//!
//! Its data ([`BearSailorData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["BearSailor"];

/// Its picture (the records name it; the bear keeps it).
pub const IMAGE_SET: &str = "LEVEL_BEARSAILOR";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_BEARSAILOR_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the seventh level's families: the
/// bear's [`EnemyState::StartHug`] and [`EnemyState::Hug`], the reserved
/// [`EnemyState::Release`], the pirate's [`EnemyState::GabrielAttack`],
/// and from [`CRAB_STATES_START`] the crabs' nine states.
pub const STATE_BLOCK: RangeInclusive<i32> = 2180..=2199;

/// The first number of the crabs' half of [`STATE_BLOCK`].
pub const CRAB_STATES_START: i32 = 2190;

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 10;
/// Damage of its blow and of touching it, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Walking speed towards Claw, and pacing speed, px/s.
pub const ADVANCE_SPEED: i32 = 120;
/// Running speed towards a far Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 300;
/// The overhead blow's box, facing left.
pub const STRIKE: Rect = Rect::new(-101, -60, -41, -21);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The contact box outside its blow: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first; the
/// innermost is the hug's.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (150, 40), (100, 40), (50, 10)];

/// The blow.
pub const STRIKE_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE1";
/// The grab, 19 ticks at the 18 ms step.
pub const GRAB_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE2";
/// The squeeze, a loop of 31 ticks at the 18 ms step.
pub const SQUEEZE_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE3";
/// The grab sound.
pub const GRAB_SOUND: &str = "LEVEL_BEARSAILOR_00380024";
/// The loop that plays while it squeezes.
pub const HUG_SOUND: &str = "LEVEL_BEARSAILOR_BEARHUG";

/// Ticks of the grab at the 18 ms step: the hug starts on the grab's 19th
/// tick.
pub const GRAB_TICKS: u32 = 19;
/// How far in front of the bear Claw is held, px.
pub const HUG_X: i32 = 30;
/// Time between two squeezes, ms; each costs Claw one health point.
pub const SQUEEZE_MS: u32 = 100;
/// The longest hug, ms.
pub const HOLD_MS: u32 = 3000;
/// Key presses during the hug that free Claw.
pub const ESCAPE_PRESSES: u32 = 10;
/// Time after a hug before the bear may grab again, ms.
pub const HUG_COOLDOWN_MS: u32 = 50;

/// The bear's squeeze pictures and the picture Claw shows while each is
/// on screen.
pub const HELD_FRAMES: [(i32, i32); 3] = [(302, 450), (303, 451), (304, 452)];

/// The picture Claw shows while the bear shows squeeze picture
/// `bear_frame`; `None` for a picture of no squeeze.
pub fn held_frame(bear_frame: i32) -> Option<i32> {
    HELD_FRAMES
        .iter()
        .find(|(bear, _)| *bear == bear_frame)
        .map(|&(_, claw)| claw)
}

/// Where the bear at `bear_x` holds Claw, and whether Claw then faces
/// right: in front of the bear on the side Claw was on, facing it.
pub fn hold_point(bear_x: i32, claw_left_of_bear: bool) -> (i32, bool) {
    if claw_left_of_bear {
        (bear_x - HUG_X, true)
    } else {
        (bear_x + HUG_X, false)
    }
}

/// The bear sailor class.
pub static CLASS: Class = Class {
    name: "BearSailor",
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

/// A bear sailor's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BearSailorData {
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
    fn its_states_open_the_seventh_levels_block() {
        let named = [
            EnemyState::StartHug,
            EnemyState::Hug,
            EnemyState::Release,
            EnemyState::GabrielAttack,
        ];
        for (i, s) in named.into_iter().enumerate() {
            assert_eq!(s as i32, STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
        assert!(STATE_BLOCK.contains(&CRAB_STATES_START));
        assert_eq!(CRAB_STATES_START - STATE_BLOCK.start(), 10);
        assert!(CLASS.human, "a human enemy");
    }

    #[test]
    fn claw_shows_the_picture_of_the_squeeze() {
        assert_eq!(held_frame(302), Some(450));
        assert_eq!(held_frame(303), Some(451));
        assert_eq!(held_frame(304), Some(452));
        assert_eq!(held_frame(301), None);
        assert_eq!(held_frame(450), None);
    }

    #[test]
    fn claw_is_held_in_front_of_the_bear_facing_it() {
        // The bear at 3681, Claw to its right: held at 3711, facing left.
        assert_eq!(hold_point(3681, false), (3711, false));
        assert_eq!(hold_point(3681, true), (3651, true));
    }

    #[test]
    fn a_full_hug_squeezes_every_hundred_ms() {
        // 3000 ms of hug, a squeeze every 100 ms from its first tick: at
        // the 18 ms step every sixth tick, the last on tick 163 of 167.
        let ticks = HOLD_MS.div_ceil(18);
        assert_eq!(ticks, 167);
        let per = SQUEEZE_MS.div_ceil(18);
        assert_eq!(per, 6);
        let squeezes = (1..ticks).step_by(per as usize).count();
        assert_eq!(squeezes, 28);
    }
}
