//! The Fish of the eleventh and twelfth levels: pond fish that patrol the
//! wading pools and leap at Claw.
//!
//! A fish swims up and down its pool along its record's height (its swim
//! line), between the ends of its record's x range, at [`SWIM_SPEED`]
//! px/s ([`EnemyState::Swim`]). Swimming towards a Claw close by, it
//! leaps out of the water at him when he stands above the line
//! ([`EnemyState::JumpStrike`], a ballistic arc [`LEAP`] px across and up
//! under [`LEAP_GRAVITY`], [`LEAP_TICKS`] ticks back into the water), and
//! otherwise dashes at where he stands ([`EnemyState::Snip`],
//! [`DASH_SPEED`] px/s). A leap that comes down outside the pool leaves it
//! flopping on dry ground until it slides back in
//! ([`EnemyState::OutOfWater`]). Its whole picture hurts Claw on contact
//! ([`DAMAGE`]), and any blow or shot that reaches it kills it at once.
//! It is never lifted and drops nothing.
//!
//! A flopping fish faces by one toggle the whole game shares
//! ([`crate::logic::World::fish_flop`]), flipped at every flop.
//!
//! Not built yet ([`BUILT`] is false): it stays an inert placeholder
//! (drawn, no collision, reported as not implemented) and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): its dash is the crabs'
//! `Snip` (2195), which the game registers under the same name for both,
//! run with the Fish's own handler; its swim, leap and flop are new
//! names, 2270 to 2272 in the eleventh level's block
//! ([`super::mercat::STATE_BLOCK`]), in the order the game registers them
//! ([`STATES`]); it dies in the shared `Dead`.
//!
//! # Defaults taken
//!
//! - The second range box, which the game never sets, is empty (0, 0):
//!   it only decides a mode the Fish never acts on (measured).
//! - The flop toggle starts at 1 when the game is created and is kept
//!   for the level's whole simulation; a new level's simulation starts it
//!   at 1 again. The original keeps it across level loads, which differs
//!   only for a fish flopping on the twelfth level after one flopped on
//!   the eleventh in the same run; no flop was seen in any measured leap.
//!
//! Its data ([`FishData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Fish"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_FISH";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_FISH_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Its three new states, in the order the game registers them.
pub const STATES: [EnemyState; 3] = [
    EnemyState::Swim,
    EnemyState::JumpStrike,
    EnemyState::OutOfWater,
];

/// Its dash: the crabs' state of the same name.
pub const DASH: EnemyState = EnemyState::Snip;

/// Contact damage when its record sets none (no record does); not scaled
/// by the difficulty.
pub const DAMAGE: i32 = 10;
/// Its box, from its first picture: movement, hit, contact and strike
/// boxes alike.
pub const RECT: Rect = Rect::new(-19, -30, 24, 13);
/// The five range boxes (half width, half height), outermost first; the
/// second is never set.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (0, 0), (300, 50), (100, 300), (50, 50)];

/// Swimming speed, px/s.
pub const SWIM_SPEED: i32 = 90;
/// Dashing speed, px/s.
pub const DASH_SPEED: i32 = 180;
/// Sliding speed of a flopping fish, px/s.
pub const FLOP_SPEED: i32 = 60;
/// How far across towards Claw and how far up a leap aims, px.
pub const LEAP: (i32, i32) = (150, 150);
/// Gravity of a leap and a flop, px/s^2.
pub const LEAP_GRAVITY: f64 = 2000.0;
/// The chance in 101 that a swimming fish leaps at a Claw above its
/// line.
pub const LEAP_CHANCE: i32 = 80;
/// The chance in 101 that a swimming fish dashes at a Claw within its
/// range.
pub const DASH_CHANCE: i32 = 90;
/// Ticks of a leap at the 18 ms step, back into the water on the last
/// (measured).
pub const LEAP_TICKS: u32 = 44;
/// Ticks of a dash at the 18 ms step (measured).
pub const DASH_TICKS: u32 = 31;
/// Time between two flops, ms.
pub const FLOP_MS: u32 = 250;

/// Swimming.
pub const SWIM_ANIM: &str = "LEVEL_FISH_SWIM";
/// The leap and the dash.
pub const STRIKE_ANIM: &str = "LEVEL_FISH_STRIKE1";
/// Flopping.
pub const FLOP_ANIM: &str = "LEVEL_FISH_FLOUNDER";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_FISH_KILLFALL";
/// The bite.
pub const CHOMP_SOUND: &str = "LEVEL_FISH_CHOMP1";
/// Leaving the water.
pub const OUT_SOUND: &str = "LEVEL_FISH_WATEROUT";
/// Back into the water.
pub const IN_SOUND: &str = "LEVEL_FISH_WATERIN";
/// A flop.
pub const FLOP_SOUND: &str = "LEVEL_FISH_FISHFLOP";
/// Its death.
pub const DEATH_SOUND: &str = "LEVEL_FISH_FISHDEATH";

/// The Fish class: a plain enemy, never lifted.
pub static CLASS: Class = Class {
    name: "Fish",
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

/// A fish's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FishData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

impl World<'_> {
    /// The flop toggle the whole game shares, 1 or 0: a flopping fish
    /// faces left on 1 and right on 0, then flips it (see the module
    /// documentation). It starts at 1.
    pub fn fish_flop(&self) -> i32 {
        if self.out.fish_flop_zero { 0 } else { 1 }
    }

    /// Sets the flop toggle to `value` (0 for 0, anything else for 1).
    pub fn set_fish_flop(&mut self, value: i32) {
        self.out.fish_flop_zero = value == 0;
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
    use super::super::mercat;
    use super::*;

    #[test]
    fn its_states_open_the_upper_half_of_the_eleventh_levels_block() {
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, 2270 + i as i32, "{}", s.name());
            assert!(mercat::STATE_BLOCK.contains(&(s as i32)));
        }
        // The dash keeps the crabs' number.
        assert_eq!(DASH as i32, 2195);
        assert!(!CLASS.human, "a plain enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
        assert!(handler(EnemyState::Swim).is_none(), "not built yet");
    }

    #[test]
    fn its_moves_at_the_eighteen_ms_step() {
        // A dash covers about 98 px: 31 ticks at 3.24 px a tick.
        assert_eq!(DASH_SPEED * 18 * DASH_TICKS as i32 / 1000, 100);
        // The second range box is empty.
        assert_eq!(RANGES[1], (0, 0));
        // A flop every 250 ms: every 14th tick.
        assert_eq!(FLOP_MS.div_ceil(18), 14);
    }

    #[test]
    fn its_art_is_its_own() {
        for key in [
            SWIM_ANIM,
            STRIKE_ANIM,
            FLOP_ANIM,
            DEATH_ANIM,
            CHOMP_SOUND,
            OUT_SOUND,
            IN_SOUND,
            FLOP_SOUND,
            DEATH_SOUND,
        ] {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
