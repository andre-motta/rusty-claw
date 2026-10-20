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
//! # The rules
//!
//! - Built: the health and damage above, smarts 99, liftable, not going by
//!   sight; the range boxes [`RANGES`] (the innermost two are equal, so
//!   within 100 px he is always in range mode 4); his strike rest run out;
//!   then dropped to the floor and idle (the shared human idle: he goes
//!   Home as soon as Claw is in range).
//! - Pre-tick, after the shared human steps, every tick and in every
//!   state: the strike choice by range mode (modes 1 and 2 the hook swing,
//!   its animation and sound; modes 3 and 4 the dagger stab; other modes
//!   keep the last choice), so a strike already running keeps its
//!   animation but strikes with the box and sound chosen on its strike
//!   tick; then the lift veto: a draw of 0..100 below [`LIFT_VETO`] makes
//!   him unliftable for that tick.
//! - Home: the alert once, the first time he sees Claw (70 in 100 one of
//!   his two lines); he faces Claw, advances his current picture and draws
//!   `r` of 0..121. Claw 300 to 500 px away (mode 0): he runs at him. 100
//!   to 300 px (modes 1, 2): when his strike rest is over and `r` is below
//!   [`STRIKE_DRAW`], on the ninth level with `r` below [`HOLD_DRAW`] he
//!   holds the hook, otherwise he swings it, and either way rests
//!   [`HOLD_MS`]; otherwise he runs (mode 1) or walks (mode 2) at Claw.
//!   Within 100 px (mode 4): he stabs when his rest is over, resting
//!   [`STAB_REST_MS`], and otherwise stands. Out of range (modes 5, 6) he
//!   paces. He never pauses in front of Claw.
//! - HoldHook: his attack box is the box of the picture shown when the
//!   tick starts (mirrored with him); then the picture advances; Home once
//!   his rest is over. He stays free and unprotected, so he can be hit,
//!   lifted, or sent idle when Claw leaves range.
//! - Hits from Claw himself: on the ninth level a standing sword stab is
//!   parried (he faces Claw, one of his two parry sounds, and two times in
//!   five his parry line). Any other blow costs him its damage with a hit
//!   burst and a 300 ms cooldown; in a flinch he only goes Home, otherwise
//!   one of his two hit sounds, the flinch (high after an uppercut) with no
//!   knockback, and while he lives he swings the hook back at once.
//! - Pistol shots: parried while he holds the hook (the shot flies on);
//!   otherwise one of his hit sounds and the shared pistol rule (damage,
//!   burst, the shot destroyed, the low flinch).
//! - Magic, thrown objects and blasts: the shared human rules.
//! - Lifted: his own line stops, then one of his two lifted lines.
//! - Going idle: unless Claw is far off screen (mode 6), a fifth of the
//!   time one of his two go-idle lines; then the shared human go idle.
//!
//! # Random draws, in order
//!
//! - building: the shared human go idle's one of 0..100;
//! - the pre-tick: the shared roll (frame pulse ticks only); one of 0..100
//!   (the lift veto), every tick;
//! - Home: the alert's one of 0..100 (then one of 0..2 below 70), once;
//!   one of 0..121;
//! - going idle: one of 0..100 unless in mode 6 (then one of 0..2 below
//!   20), then the shared human go idle's one of 0..100;
//! - a parry: one of 0..2, one of 0..100;
//! - hit by Claw's blow: the burst's two, then (not in a flinch) one of
//!   0..2 and the flinch's;
//! - hit by a pistol shot: holding, the parry's; otherwise one of 0..2,
//!   then the burst's two;
//! - lifted: one of 0..100, which is the shared lift's own draw (see
//!   [`pick_lift_voice`]).
//!
//! # His name
//!
//! The level files name his logic with one word, which this module spells
//! once, in [`NAMES`]. Every other key of his (his picture, animations and
//! sounds) is built from that one name when the game is compiled
//! ([`image_set`], [`anim_prefix`], [`keys`], [`key`]); everything else in
//! the game reaches him through [`NAMES`] and calls him the hook-handed
//! pirate.
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
//! - The hold at 60 frames per second lasts its 2000 ms like at the 18 ms
//!   step (no run at 60 frames per second has shown a hold yet).
//! - Two of his records and a PegLeg end their patrols next to a saw
//!   blade: no special case, the saw's hit reaches them whenever its
//!   strike box overlaps their hit box, as for every enemy.
//!
//! Its data ([`HookPirateData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, HumanAnims, HumanData};
use super::lift::Carry;
use super::states::{self, enter_hit, enter_strike, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, FIRE_BLOW,
    FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit, attacker,
    build, data, data_mut, die, face_claw, facing_rect, hit_burst, human_fields, obj, obj_mut,
    play, rand, say, sees_claw, set_anim, set_state, take_damage,
};
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, flags, special};
use crate::physics::ptype;
use crate::rng::Rng;

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
pub const BUILT: bool = true;

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
/// Smarts base (raised to 99).
pub const SMARTS: i32 = 50;
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
/// Home draws one of 0..121.
pub const HOME_SPREAD: u32 = 121;
/// Chance in 100 that his first sight of Claw says an alert line.
pub const ALERT_CHANCE: i32 = 70;
/// Chance in 100 that going idle says a go-idle line.
pub const GO_IDLE_LINE_CHANCE: i32 = 20;
/// Chance in 100 that a parry says his parry line.
pub const PARRY_LINE_CHANCE: i32 = 40;
/// Chance in 100 that being lifted says the first of his lifted lines.
pub const FIRST_LIFTED_LINE_CHANCE: u32 = 35;
/// The cooldown after a blow that lands, ms.
pub const BLOW_COOLDOWN_MS: u64 = 300;
/// The cooldown after a blast, ms.
pub const BLAST_COOLDOWN_MS: u64 = 2000;

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
    /// A jump.
    pub const JUMP: &str = "JUMP";
    /// The body flying off after death.
    pub const KILLFALL: &str = "KILLFALL";
    /// Being lifted.
    pub const LIFTED1: &str = "LIFTED1";
    /// Held overhead.
    pub const LIFTED2: &str = "LIFTED2";
    /// Thrown, and bouncing.
    pub const THROWN: &str = "THROWN";
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
    /// Going idle, one of two.
    pub const GO_IDLE: [&str; 2] = ["00420002", "00420003"];
    /// The parry line.
    pub const PARRY: &str = "00420015";
    /// Lifted: the first of these 35 times in 100, the second otherwise.
    pub const LIFTED: [&str; 2] = ["00420004", "00420016"];
    /// Hit, one of two.
    pub const HIT: [&str; 2] = ["00420009", "00420010"];
}

// ----------------------------------------------------------------------
// His keys, built from his one name when the game is compiled.

/// The start of every key of his.
const KEY_START: &str = "LEVEL_";

/// Length of the key `LEVEL_` + his name in capitals + `sep` + `suffix`.
const fn key_len(sep: &str, suffix: &str) -> usize {
    KEY_START.len() + NAMES[0].len() + sep.len() + suffix.len()
}

/// Copies `part` into `out` from `at`, in capitals when `upper`; returns
/// where the next part starts.
const fn put(out: &mut [u8], at: usize, part: &str, upper: bool) -> usize {
    let b = part.as_bytes();
    let mut i = 0;
    while i < b.len() {
        out[at + i] = if upper {
            b[i].to_ascii_uppercase()
        } else {
            b[i]
        };
        i += 1;
    }
    at + b.len()
}

/// The bytes of the key `LEVEL_` + his name in capitals + `sep` +
/// `suffix`; `N` is its [`key_len`].
const fn key_bytes<const N: usize>(sep: &str, suffix: &str) -> [u8; N] {
    let mut out = [0u8; N];
    let at = put(&mut out, 0, KEY_START, false);
    let at = put(&mut out, at, NAMES[0], true);
    let at = put(&mut out, at, sep, false);
    let end = put(&mut out, at, suffix, false);
    assert!(end == N, "a key's length");
    out
}

/// The key `LEVEL_` + his name in capitals + `sep` + `suffix`, as a
/// constant string.
macro_rules! key_of {
    ($sep:expr, $suffix:expr) => {{
        const N: usize = key_len($sep, $suffix);
        const BYTES: [u8; N] = key_bytes::<N>($sep, $suffix);
        match ::core::str::from_utf8(&BYTES) {
            Ok(s) => s,
            Err(_) => panic!("keys are plain letters and digits"),
        }
    }};
}

/// His full animation and sound keys (see [`anim`] and [`sounds`]), built
/// from [`NAMES`].
pub mod keys {
    use super::{anim, key_bytes, key_len, sounds};

    /// His picture.
    pub const IMAGE_SET: &str = key_of!("", "");
    /// The prefix of his animations and sounds.
    pub const PREFIX: &str = key_of!("_", "");
    /// Walking towards Claw.
    pub const ADVANCE: &str = key_of!("_", anim::ADVANCE);
    /// Running towards Claw, and pacing.
    pub const FAST_ADVANCE: &str = key_of!("_", anim::FAST_ADVANCE);
    /// The hook swing.
    pub const STRIKE1: &str = key_of!("_", anim::STRIKE1);
    /// The dagger stab.
    pub const STRIKE2: &str = key_of!("_", anim::STRIKE2);
    /// The parry and the hook hold.
    pub const BLOCK: &str = key_of!("_", anim::BLOCK);
    /// The idle pose.
    pub const IDLE1: &str = key_of!("_", anim::IDLE1);
    /// The high flinch.
    pub const HIT_HIGH: &str = key_of!("_", anim::HIT_HIGH);
    /// The low flinch.
    pub const HIT_LOW: &str = key_of!("_", anim::HIT_LOW);
    /// A jump.
    pub const JUMP: &str = key_of!("_", anim::JUMP);
    /// The body flying off after death.
    pub const KILLFALL: &str = key_of!("_", anim::KILLFALL);
    /// Being lifted.
    pub const LIFTED1: &str = key_of!("_", anim::LIFTED1);
    /// Held overhead.
    pub const LIFTED2: &str = key_of!("_", anim::LIFTED2);
    /// Thrown, and bouncing.
    pub const THROWN: &str = key_of!("_", anim::THROWN);
    /// The hook swing's sound.
    pub const HOOK_SWING_SOUND: &str = key_of!("_", sounds::HOOK_SWING);
    /// The dagger stab's sound.
    pub const DAGGER_STAB_SOUND: &str = key_of!("_", sounds::DAGGER_STAB);
    /// The parry sounds.
    pub const BLOCK_SOUNDS: [&str; 2] = [
        key_of!("_", sounds::BLOCK[0]),
        key_of!("_", sounds::BLOCK[1]),
    ];
    /// His death.
    pub const DEATH_SOUND: &str = key_of!("_", sounds::DEATH);
    /// His alert lines.
    pub const ALERT_LINES: [&str; 2] = [
        key_of!("_", sounds::ALERT[0]),
        key_of!("_", sounds::ALERT[1]),
    ];
    /// His go-idle lines.
    pub const GO_IDLE_LINES: [&str; 2] = [
        key_of!("_", sounds::GO_IDLE[0]),
        key_of!("_", sounds::GO_IDLE[1]),
    ];
    /// His parry line.
    pub const PARRY_LINE: &str = key_of!("_", sounds::PARRY);
    /// His lifted lines.
    pub const LIFTED_LINES: [&str; 2] = [
        key_of!("_", sounds::LIFTED[0]),
        key_of!("_", sounds::LIFTED[1]),
    ];
    /// His hit sounds.
    pub const HIT_SOUNDS: [&str; 2] = [key_of!("_", sounds::HIT[0]), key_of!("_", sounds::HIT[1])];
}

/// His picture, the image set the records name, built from [`NAMES`].
pub fn image_set() -> &'static str {
    keys::IMAGE_SET
}

/// The prefix of his animations and sounds, built from [`NAMES`].
pub fn anim_prefix() -> &'static str {
    keys::PREFIX
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

/// The animations of the shared states. He has no pose of his own for
/// getting up after a throw: he keeps the picture he landed with.
const ANIMS: ClassAnims = ClassAnims {
    advance: keys::ADVANCE,
    fast_advance: keys::FAST_ADVANCE,
    pace: keys::FAST_ADVANCE,
    walk: "",
    hit_low: keys::HIT_LOW,
    hit_high: keys::HIT_HIGH,
    home: "",
    jump: keys::JUMP,
    fall: keys::KILLFALL,
    lifted: keys::LIFTED1,
    lifted2: keys::LIFTED2,
    bounce: keys::THROWN,
};

/// The sounds of the shared states: his strike plays the sound his
/// pre-tick picked ([`HumanData::sounds`]), none before the first pick.
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries him: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The hook-handed pirate class.
pub static CLASS: Class = Class {
    name: NAMES[0],
    human: true,
    construct,
    pre_tick,
    handler,
    go_home: states::go_home,
    go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    lift_roll: true,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A hook-handed pirate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HookPirateData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The rest after his last strike or hold: no new one starts from
    /// Home until it is over.
    pub strike_rest: Timer,
}

/// The pirate data of `id`.
pub fn pirate_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a HookPirateData> {
    match &w.objects.get(id)?.data {
        ObjectData::HookPirate(d) => Some(d),
        _ => None,
    }
}

fn pirate_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut HookPirateData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::HookPirate(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::HoldHook => Some(hold_tick),
        EnemyState::Block => Some(super::townguard::block_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    human::tick(w, id);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds a hook-handed pirate (see the module documentation). Draws: the
/// shared human go idle's one of 0..100.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: HOOK_SWING,
        strike_anim: keys::STRIKE1,
        death_anim: keys::KILLFALL,
        death_sound: keys::DEATH_SOUND,
        lifted_voice: "",
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                jump_back: "",
                block_high: keys::BLOCK,
                block_low: keys::BLOCK,
                idle: keys::IDLE1,
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: keys::IMAGE_SET,
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::HookPirate(HookPirateData {
            enemy,
            ..HookPirateData::default()
        });
    }
    go_idle(w, id);
}

// ----------------------------------------------------------------------
// Pre-tick and going idle

/// The strike his range mode `mode` chooses: the box (facing left), the
/// animation and the sound; `None` in the modes that keep the last one.
pub fn strike_choice(mode: u8) -> Option<(Rect, &'static str, &'static str)> {
    match mode {
        1 | 2 => Some((HOOK_SWING, keys::STRIKE1, keys::HOOK_SWING_SOUND)),
        3 | 4 => Some((DAGGER_STAB, keys::STRIKE2, keys::DAGGER_STAB_SOUND)),
        _ => None,
    }
}

/// The pirate's own pre-tick steps, after the shared ones: the strike
/// choice, then the lift veto (see the module documentation). Draws: one
/// of 0..100.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id)
        && let Some((rect, anim, sound)) = strike_choice(d.mode)
    {
        d.strike_rect = rect;
        d.strike_anim = anim;
        d.human.sounds.strike = sound;
    }
    if rand(w, 100) < LIFT_VETO {
        obj_mut(w, id).special_flags &= !special::LIFTABLE;
    }
}

/// Goes idle: unless Claw is far off screen (mode 6), a fifth of the time
/// one of his two go-idle lines; then the shared human go idle. Draws: one
/// of 0..100 unless in mode 6 (then one of 0..2 below 20), then the human
/// go idle's one of 0..100.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| d.mode != 6) && rand(w, 100) < GO_IDLE_LINE_CHANCE {
        let line = keys::GO_IDLE_LINES[rand(w, 2) as usize];
        say(w, id, line);
    }
    human::go_idle(w, id);
}

// ----------------------------------------------------------------------
// Home

/// Home (see the module documentation). Draws: the alert's (once), then
/// one of 0..121.
pub fn home_tick(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| !d.alerted) && sees_claw(w, id) {
        if rand(w, 100) < ALERT_CHANCE {
            let line = keys::ALERT_LINES[rand(w, 2) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    face_claw(w, id);
    w.advance_anim(id);
    let r = rand(w, HOME_SPREAD);
    let now = w.now_ms;
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    let rested = pirate_data(w, id).is_some_and(|p| p.strike_rest.done(now));
    match mode {
        0 => states::enter_fast_advance(w, id),
        1 | 2 => {
            if rested && r < STRIKE_DRAW {
                if holds_and_parries_swords(w.level_number()) && r < HOLD_DRAW {
                    enter_hold(w, id);
                } else {
                    enter_strike(w, id);
                }
                rest(w, id, HOLD_MS);
            } else if mode == 1 {
                states::enter_fast_advance(w, id);
            } else {
                states::enter_advance(w, id);
            }
        }
        3 => {
            if r < smarts && rested {
                enter_strike(w, id);
                rest(w, id, STAB_REST_MS);
            }
        }
        4 => {
            if rested {
                enter_strike(w, id);
                rest(w, id, STAB_REST_MS);
            }
        }
        _ => start_pacing(w, id),
    }
}

fn rest(w: &mut World, id: ObjectId, ms: u32) {
    let now = w.now_ms;
    if let Some(p) = pirate_mut(w, id) {
        p.strike_rest = Timer::new(now, u64::from(ms));
    }
}

// ----------------------------------------------------------------------
// The hook hold

/// Starts holding the hook up: his parry picture, not advanced yet; he
/// stays free and unprotected. The rest Home sets right after covers the
/// whole hold.
pub fn enter_hold(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::HoldHook);
    set_anim(w, id, keys::BLOCK);
    rest(w, id, STAB_REST_MS);
}

/// HoldHook: his attack box is the box of the picture shown when the tick
/// starts, mirrored with him; then the picture advances; Home once his
/// rest is over.
pub fn hold_tick(w: &mut World, id: ObjectId) {
    let shown = {
        let o = obj(w, id);
        w.frame_box(&o.image_set, o.frame)
    };
    if let Some(b) = shown
        && let Some(d) = data_mut(w, id)
    {
        d.strike_rect = b;
    }
    let rect = data(w, id).map_or(CONTACT, |d| d.strike_rect);
    let o = obj_mut(w, id);
    o.attack_rect = facing_rect(o, rect);
    w.advance_anim(id);
    let now = w.now_ms;
    if pirate_data(w, id).is_some_and(|p| p.strike_rest.done(now)) {
        states::go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// Parries: faces Claw, busy and unhurt in his parry picture; one of his
/// two parry sounds, and two times in five his parry line. Draws: one of
/// 0..2, one of 0..100.
pub fn enter_parry(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::Block);
    set_anim(w, id, keys::BLOCK);
    obj_mut(w, id).flags |= flags::SAFE;
    let sound = keys::BLOCK_SOUNDS[rand(w, 2) as usize];
    play(w, sound);
    if rand(w, 100) < PARRY_LINE_CHANCE {
        say(w, id, keys::PARRY_LINE);
    }
}

/// Whether Claw's blow that just reached him is parried: on the ninth
/// level a sword stab made standing.
fn parries_blow(w: &World) -> bool {
    holds_and_parries_swords(w.level_number())
        && super::townguard::is_sword_swipe(w.claw.attack.kind)
        && w.claw_object()
            .is_some_and(|c| c.physics_type == ptype::STAND)
}

/// `attacker` hit `id` (see the module documentation). Draws: by what hit,
/// as the module documentation lists, then the death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let blow = w.objects.get(attacker).map_or(0, |a| a.special_flags);
    if let Some(d) = data_mut(w, id) {
        if blow & FIRE_BLOW != 0 {
            d.death_anim = FIRE_DEATH_ANIM;
        } else if blow & ICE_BLOW != 0 {
            d.death_anim = ICE_DEATH_ANIM;
        }
    }
    if !accept_hit(w, id) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    let now = w.now_ms;
    match kind {
        attacker::CLAW => blow_rule(w, id, attacker),
        attacker::PISTOL_SHOT => {
            if super::state(w, id) == Some(EnemyState::HoldHook) {
                // The parry; the shot flies on.
                enter_parry(w, id);
            } else {
                let sound = keys::HIT_SOUNDS[rand(w, 2) as usize];
                play(w, sound);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                w.destroy(attacker);
                enter_hit(w, id, false);
            }
        }
        attacker::MAGIC | attacker::THROWN => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            enter_hit(w, id, false);
        }
        attacker::LEVEL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, BLAST_COOLDOWN_MS);
            }
            enter_hit(w, id, false);
        }
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// A blow of Claw's himself: the parry (ninth level, a standing sword
/// stab), or the damage, a burst and the cooldown, then Home during a
/// flinch, or one of his hit sounds, the flinch with no knockback and,
/// while he lives, the hook swing back at once.
fn blow_rule(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if parries_blow(w) {
        enter_parry(w, id);
        return;
    }
    let now = w.now_ms;
    take_damage(w, id, attacker);
    hit_burst(w, id, attacker);
    if let Some(d) = data_mut(w, id) {
        d.cooldown = Timer::new(now, BLOW_COOLDOWN_MS);
    }
    if matches!(
        super::state(w, id),
        Some(EnemyState::HitLow | EnemyState::HitHigh)
    ) {
        states::go_home(w, id);
        return;
    }
    let sound = keys::HIT_SOUNDS[rand(w, 2) as usize];
    play(w, sound);
    let high = w.claw.attack.uppercut;
    enter_hit(w, id, high);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    if obj(w, id).health > 0 {
        if let Some(d) = data_mut(w, id) {
            d.strike_rect = HOOK_SWING;
            d.strike_anim = keys::STRIKE1;
        }
        enter_strike(w, id);
    }
}

/// Lifted: his own running line stops, then the first of his two lifted
/// lines 35 times in 100, the second otherwise. The choice reads the draw
/// of 0..100 the shared lift makes right after this step (the original
/// makes that one draw only), so the stream is left where it was.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    sound::enemy_silence(w, id);
    let before = w.rng.state();
    let roll = w.rng.below(100);
    *w.rng = Rng::new(before);
    let line = keys::LIFTED_LINES[usize::from(roll >= FIRST_LIFTED_LINE_CHANCE)];
    say(w, id, line);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
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
        assert_eq!(keys::STRIKE1, key(anim::STRIKE1));
        assert_eq!(keys::BLOCK_SOUNDS[1], key(sounds::BLOCK[1]));
        assert_eq!(keys::DEATH_SOUND, key(sounds::DEATH));
        assert_eq!(derived_image_sets(), [image_set()]);
        assert_eq!(derived_anim_prefixes(), [anim_prefix()]);
        assert!(anim_prefix().starts_with(image_set()));
        assert_eq!(CLASS.name, NAMES[0]);
        assert!(CLASS.human, "a human enemy");
        for k in [
            ANIMS.advance,
            ANIMS.fast_advance,
            ANIMS.hit_low,
            ANIMS.hit_high,
            ANIMS.jump,
            ANIMS.fall,
            ANIMS.lifted,
            ANIMS.lifted2,
            ANIMS.bounce,
        ] {
            assert!(k.starts_with(anim_prefix()), "{k}");
        }
    }

    #[test]
    fn his_one_state_is_in_the_ninth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2220, 2239));
        assert_eq!(EnemyState::HoldHook as i32, STATE_BLOCK.start() + 2);
        assert_eq!(STATES, [EnemyState::HoldHook]);
        // The parry is the second town guard's.
        assert_eq!(EnemyState::Block as i32, 2141);
        for s in [EnemyState::Home, EnemyState::HoldHook, EnemyState::Block] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        for s in [
            EnemyState::Idle,
            EnemyState::Strike,
            EnemyState::Advance,
            EnemyState::FastAdvance,
            EnemyState::HitLow,
        ] {
            assert!(handler(s).is_none(), "{}", s.name());
        }
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

    #[test]
    fn his_strike_follows_the_range_mode() {
        assert_eq!(strike_choice(0), None);
        assert_eq!(
            strike_choice(1),
            Some((HOOK_SWING, keys::STRIKE1, keys::HOOK_SWING_SOUND))
        );
        assert_eq!(strike_choice(2), strike_choice(1));
        assert_eq!(
            strike_choice(4),
            Some((DAGGER_STAB, keys::STRIKE2, keys::DAGGER_STAB_SOUND))
        );
        assert_eq!(strike_choice(3), strike_choice(4));
        assert_eq!(strike_choice(5), None);
        assert_eq!(strike_choice(6), None);
    }

    #[test]
    fn his_hold_boxes_are_his_pictures_mirrored() {
        // Facing right, the hold's boxes are the pictures' boxes mirrored.
        assert_eq!(
            HOLD_BOXES[0],
            Rect::new(-30, -47, 53, 60).mirrored(),
            "the first picture"
        );
        assert_eq!(HOLD_BOXES[2], Rect::new(-43, -32, 60, 59).mirrored());
    }
}
