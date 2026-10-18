//! The hook-handed pirate of the ninth level (and of the tenth): a pirate
//! with a hook for a hand and a dagger.
//!
//! He is a human enemy. He charges at [`FAST_ADVANCE_SPEED`] from 300 px
//! out, walks in at [`ADVANCE_SPEED`] from 200 px and fights in two
//! ranges:
//!
//! - between 100 and 300 px he swings his hook ([`HOOK_SWING`], live on the
//!   7th tick of the swing, over on the 19th), which reaches only 67 px;
//!   on the ninth level he may hold the hook up instead
//!   ([`EnemyState::HoldHook`]) for [`HOLD_TICKS`] ticks, and while he
//!   holds it the box of his whole drawn picture hurts Claw on every tick
//!   that Claw's own protection allows;
//! - within 100 px he stabs with his dagger ([`DAGGER_STAB`], live on the
//!   5th tick, over on the 21st), once every [`STAB_PERIOD_TICKS`] ticks.
//!
//! He parries more than any earlier pirate ([`EnemyState::Block`], the
//! second town guard's parry, [`PARRY_TICKS`] ticks): on the ninth level
//! every standing sword stab, and on both levels every pistol shot while
//! he holds the hook (the shot flies on). Any other pistol shot hits him
//! ([`EnemyState::HitLow`] for [`HIT_LOW_TICKS`] ticks) and two kill him.
//! A crouching or jumping stab lands and he strikes straight back with the
//! hook, with no knockback. He has [`HEALTH`] health, his strikes cost
//! Claw [`DAMAGE`], he can be lifted and thrown except on about one tick
//! in five, and he drops his record's loot.
//!
//! Not built yet ([`BUILT`] is false): the hook-handed pirates stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! # His name
//!
//! The level files name his logic with one word, which this module spells
//! once, in [`NAMES`]. Every other key of his (his picture, animations and
//! sounds) is built from that one name at run time ([`image_set`],
//! [`anim_prefix`], [`key`]); everything else in the game reaches him
//! through [`NAMES`] and calls him the hook-handed pirate.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he decides in `Home` with
//! a handler of his own, parries in the town guards' `Block` (2141) and
//! inherits every other shared human state with its number. His one new
//! name, [`EnemyState::HoldHook`], takes the third number of the ninth
//! level's block ([`STATE_BLOCK`], 2220 to 2239, shared with PegLeg's two
//! rolls at 2220 and 2221).
//!
//! # Defaults taken
//!
//! - The ninth level's block of state numbers: blocks of 20 in level
//!   order, so the ninth level owns 2220 to 2239: PegLeg's rolls 2220 and
//!   2221, the hook hold 2222; the dart traps keep their states in their
//!   own data ([`crate::logic::dart_trap::state`]), so 2223 and the
//!   numbers after it stay free.
//! - The "on screen" test of his alert under a widened view follows the
//!   rule the soldier's sight test uses.
//! - His hold box is the box of the picture shown when the tick starts,
//!   as measured: on the ninth level, facing right, (-49, -46, 34, 59) for
//!   the first hold tick (the picture he showed before), then
//!   [`HOLD_BOXES`] for 4, 6 and the remaining ticks.
//! - Two of his records and a PegLeg end their patrols next to a saw
//!   blade: no special case, the saw's hit reaches them whenever its
//!   strike box overlaps their hit box, as for every enemy.
//!
//! Its data ([`HookPirateData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;
use std::sync::OnceLock;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Logic names this family runs. The one name is the name the level files
/// give this enemy's logic.
pub const NAMES: &[&str] = &["CrazyHook"];

/// Image sets this family may switch an object to: none by a fixed name;
/// his picture's name is built from [`NAMES`] ([`derived_image_sets`]).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: none by a fixed
/// name; his prefix is built from [`NAMES`] ([`derived_anim_prefixes`]).
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers of the ninth level's families: PegLeg's rolls and
/// the hook hold.
pub const STATE_BLOCK: RangeInclusive<i32> = 2220..=2239;

/// His one new state.
pub const STATES: [EnemyState; 1] = [EnemyState::HoldHook];

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 10;
/// Damage of a strike, of the hold box and of touching him, before the
/// difficulty factor.
pub const DAMAGE: i32 = 10;
/// Walking speed towards Claw, px/s.
pub const ADVANCE_SPEED: i32 = 60;
/// Running speed towards Claw, and pacing speed, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 120;
/// The hook swing's box, facing left.
pub const HOOK_SWING: Rect = Rect::new(-67, -46, -18, 26);
/// The dagger stab's box, facing left.
pub const DAGGER_STAB: Rect = Rect::new(-80, 10, -30, 34);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The contact box outside strikes and holds.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first. The
/// last two are equal, so the fourth range never occurs: within 100 px he
/// is always in the innermost one.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (100, 40), (100, 40)];
/// His hold boxes on the ninth level, facing right, by the picture shown:
/// the hold's three pictures, shown for 4 ticks, 6 ticks and the rest of
/// the hold (measured).
pub const HOLD_BOXES: [Rect; 3] = [
    Rect::new(-53, -47, 30, 60),
    Rect::new(-53, -34, 30, 59),
    Rect::new(-60, -32, 43, 59),
];

/// Length of a hook hold, ms; and the rest after a hook swing.
pub const HOLD_MS: u32 = 2000;
/// Ticks of a hook hold at the 18 ms step (measured: 112, ending on the
/// tick the timer runs out).
pub const HOLD_TICKS: u32 = 112;
/// The rest after a dagger stab, ms.
pub const STAB_REST_MS: u32 = 1000;
/// Ticks from one dagger stab to the next at close range, at the 18 ms
/// step (measured).
pub const STAB_PERIOD_TICKS: u32 = 56;
/// The hook swing's live tick and last tick, counted from the deciding
/// tick (measured).
pub const SWING_TICKS: (u32, u32) = (7, 19);
/// The dagger stab's live tick and last tick, counted from the deciding
/// tick (measured).
pub const STAB_TICKS: (u32, u32) = (5, 21);
/// Ticks of a parry at the 18 ms step (measured).
pub const PARRY_TICKS: u32 = 11;
/// Ticks of the low flinch at the 18 ms step (measured).
pub const HIT_LOW_TICKS: u32 = 19;
/// Of a `0..=120` draw in `Home`, the values below which he strikes (or,
/// on the ninth level, holds) in the middle ranges.
pub const STRIKE_DRAW: i32 = 60;
/// Of the same draw, the values below which the ninth level's pirate holds
/// the hook instead of swinging it.
pub const HOLD_DRAW: i32 = 30;
/// Chance in 100, drawn every tick, that he cannot be lifted this tick.
pub const LIFT_VETO: i32 = 20;

/// His animations, by their names after [`anim_prefix`].
pub mod anim {
    /// Walking towards Claw.
    pub const ADVANCE: &str = "ADVANCE";
    /// Running towards Claw, and pacing.
    pub const FAST_ADVANCE: &str = "FASTADVANCE";
    /// The hook swing.
    pub const STRIKE1: &str = "STRIKE1";
    /// The dagger stab.
    pub const STRIKE2: &str = "STRIKE2";
    /// The parry and the hook hold.
    pub const BLOCK: &str = "BLOCK";
    /// The idle pose.
    pub const IDLE1: &str = "IDLE1";
    /// The high flinch.
    pub const HIT_HIGH: &str = "HITHIGH";
    /// The low flinch.
    pub const HIT_LOW: &str = "HITLOW";
}

/// His sounds, by their names after [`anim_prefix`].
pub mod sounds {
    /// The hook swing.
    pub const HOOK_SWING: &str = "HOOKSWING1";
    /// The dagger stab.
    pub const DAGGER_STAB: &str = "DAGGERSTAB1";
    /// The parry, one of two.
    pub const BLOCK: [&str; 2] = ["BLOCK1", "BLOCK2"];
    /// His death.
    pub const DEATH: &str = "00420008";
    /// His alert, one of two.
    pub const ALERT: [&str; 2] = ["00420000", "00420021"];
}

struct Keys {
    image_set: String,
    prefix: String,
}

fn keys() -> &'static Keys {
    static KEYS: OnceLock<Keys> = OnceLock::new();
    KEYS.get_or_init(|| {
        let image_set = format!("LEVEL_{}", NAMES[0].to_ascii_uppercase());
        let prefix = format!("{image_set}_");
        Keys { image_set, prefix }
    })
}

/// His picture, the image set the records name, built from [`NAMES`].
pub fn image_set() -> &'static str {
    &keys().image_set
}

/// The prefix of his animations and sounds, built from [`NAMES`].
pub fn anim_prefix() -> &'static str {
    &keys().prefix
}

/// The full key of his animation or sound `name` (see [`anim`] and
/// [`sounds`]).
pub fn key(name: &str) -> String {
    format!("{}{name}", anim_prefix())
}

/// Image sets this family may switch an object to, built from [`NAMES`]:
/// his picture.
pub fn derived_image_sets() -> [&'static str; 1] {
    [image_set()]
}

/// Prefixes of the animations this family may play, built from
/// [`NAMES`].
pub fn derived_anim_prefixes() -> [&'static str; 1] {
    [anim_prefix()]
}

/// Whether the pirate of level `level` holds the hook and parries standing
/// sword stabs: on the ninth level only.
pub fn holds_and_parries_swords(level: u32) -> bool {
    level == 9
}

/// The hook-handed pirate class.
pub static CLASS: Class = Class {
    name: NAMES[0],
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

/// A hook-handed pirate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HookPirateData {
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
    fn his_keys_are_built_from_his_one_name() {
        assert_eq!(NAMES.len(), 1);
        let upper = NAMES[0].to_ascii_uppercase();
        assert_eq!(image_set(), format!("LEVEL_{upper}"));
        assert_eq!(anim_prefix(), format!("LEVEL_{upper}_"));
        assert_eq!(key(anim::STRIKE1), format!("LEVEL_{upper}_STRIKE1"));
        assert_eq!(derived_image_sets(), [image_set()]);
        assert_eq!(derived_anim_prefixes(), [anim_prefix()]);
        assert!(anim_prefix().starts_with(image_set()));
        assert_eq!(CLASS.name, NAMES[0]);
        assert!(CLASS.human, "a human enemy");
    }

    #[test]
    fn his_one_state_is_in_the_ninth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2220, 2239));
        assert_eq!(EnemyState::HoldHook as i32, STATE_BLOCK.start() + 2);
        assert_eq!(STATES, [EnemyState::HoldHook]);
        // The parry is the second town guard's.
        assert_eq!(EnemyState::Block as i32, 2141);
    }

    #[test]
    fn his_timings_at_eighteen_milliseconds() {
        assert_eq!(HOLD_MS.div_ceil(18), HOLD_TICKS);
        assert_eq!(STAB_REST_MS.div_ceil(18), STAB_PERIOD_TICKS);
        const { assert!(SWING_TICKS.0 < SWING_TICKS.1 && STAB_TICKS.0 < STAB_TICKS.1) };
        const { assert!(HOLD_DRAW < STRIKE_DRAW) };
        // The innermost two ranges are equal.
        assert_eq!(RANGES[3], RANGES[4]);
        // The hook reaches 67 px, the dagger 80 px, both ahead of him.
        assert_eq!(-HOOK_SWING.left, 67);
        assert_eq!(-DAGGER_STAB.left, 80);
        assert!(holds_and_parries_swords(9));
        assert!(!holds_and_parries_swords(10));
    }
}
