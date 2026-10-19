//! The Siren of the eleventh and twelfth levels, whose scream locks
//! Claw's controls for two seconds.
//!
//! She is a human enemy who cannot hurt Claw herself except by contact.
//! Whenever Claw is inside her range boxes and her four-second wait has
//! run out she sings ([`EnemyState::Singing`]): on the eleventh tick of
//! the song ([`CALL_TICK`]) her call sounds and her first scream leaves
//! from [`SCREAM_OFFSET`] of her position; three more follow, one every
//! [`SCREAM_GAP_TICKS`] ticks, until the song's [`SONG_MS`] run out. A
//! scream ([`crate::logic::projectile::scream`]) flies slowly
//! ([`SCREAM_SPEED`] px/s) and, when it touches Claw, crouching or not,
//! sets the scream lockout ([`crate::logic::World::scream_lock_ms`]) to
//! [`crate::logic::projectile::scream::LOCK_MS`]: his controls are
//! ignored until it has run down, and every key still held then counts
//! as pressed.
//!
//! She has [`HEALTH`] health, can be lifted and drops her record's loot.
//!
//! Not built yet ([`BUILT`] is false): she stays an inert placeholder
//! (drawn, no collision, reported as not implemented) and the calls
//! below do nothing.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): she decides in `Home` and
//! idles in the shared human idle; her one new name,
//! [`EnemyState::Singing`], takes 2273 in the eleventh level's block
//! ([`super::mercat::STATE_BLOCK`]), after the Fish's three.
//!
//! # Defaults taken
//!
//! - Whether she is on screen, for the shared human pre-tick and idle,
//!   under a display wider than the original's: the rule the built
//!   soldier and cutthroat use, the object having been drawn in the last
//!   frame.
//! - A laser's beam reaches her through the shared human hit mask
//!   ([`super::HUMAN_HIT_BY`]) and the shared reaction to the level's own
//!   shots: no health lost, a flinch.
//!
//! Its data ([`SirenData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Siren"];

/// Her picture.
pub const IMAGE_SET: &str = "LEVEL_SIREN";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: hers, and her
/// scream's (see [`crate::logic::projectile::scream`]).
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SIREN_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Her one new state.
pub const STATES: [EnemyState; 1] = [EnemyState::Singing];

/// Her health, before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 9;
/// Damage of touching her, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Her smarts at the default factor.
pub const SMARTS: i32 = 99;

/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: the first picture's.
pub const MOVE_RECT: Rect = Rect::new(-33, -63, 26, 44);
/// Movement box while Claw carries her.
pub const CARRIED_RECT: Rect = Rect::new(-34, -44, 46, 11);

/// Where a scream leaves, from her position, facing right; mirrored when
/// she faces left.
pub const SCREAM_OFFSET: (i32, i32) = (28, -10);
/// A scream's speed, px/s.
pub const SCREAM_SPEED: i32 = 200;
/// The tick of the song on which her call sounds and the first scream
/// leaves, counted from the song's first tick at the 18 ms step
/// (measured).
pub const CALL_TICK: u32 = 11;
/// Ticks between two screams of a song at the 18 ms step (measured).
pub const SCREAM_GAP_TICKS: u32 = 17;
/// The most screams of one song.
pub const SCREAMS: u32 = 4;
/// How long a song lasts from her call, ms.
pub const SONG_MS: u32 = 1695;
/// The wait from one song to the next, ms.
pub const SONG_COOLDOWN_MS: u32 = 4000;
/// Her wait after going home, ms: this plus a draw of 0..=1000.
pub const HOME_WAIT_MS: u32 = 1000;

/// Her pose at home and idle.
pub const IDLE_ANIM: &str = "LEVEL_SIREN_IDLE";
/// The song.
pub const SONG_ANIM: &str = "LEVEL_SIREN_STRIKE1";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_SIREN_KILLFALL";
/// Her call.
pub const CALL_SOUND: &str = "LEVEL_SIREN_SIRENCALL";

/// The Siren class.
pub static CLASS: Class = Class {
    name: "Siren",
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

/// A Siren's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SirenData {
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
    use super::super::mercat;
    use super::*;
    use crate::logic::projectile::scream;

    #[test]
    fn her_song_takes_the_eleventh_levels_2273() {
        assert_eq!(STATES[0] as i32, 2273);
        assert!(mercat::STATE_BLOCK.contains(&(STATES[0] as i32)));
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Home).is_none(), "not built yet");
    }

    #[test]
    fn a_song_screams_four_times_before_it_ends() {
        // The call on tick 11, then a scream every 17 ticks: the fourth on
        // tick 62, inside the song's 1695 ms (95 ticks) from the call.
        let last = CALL_TICK + (SCREAMS - 1) * SCREAM_GAP_TICKS;
        assert_eq!(last, 62);
        assert!((last - CALL_TICK) * 18 < SONG_MS);
        assert_eq!(SONG_MS.div_ceil(18), 95);
        // A scream flies 3 or 4 px a tick.
        assert_eq!(SCREAM_SPEED * 18, 3_600);
        // Its lockout is two seconds.
        assert_eq!(scream::LOCK_MS, 2000);
    }

    #[test]
    fn her_art_is_her_own() {
        for key in [IDLE_ANIM, SONG_ANIM, DEATH_ANIM, CALL_SOUND] {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
