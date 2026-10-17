//! Pickups: treasure, health, ammunition, powerups, warps, extra lives,
//! the map piece that ends a level, and curses.
//!
//! Everything here is driven by one table, [`POWERUPS`], that maps a
//! powerup code (the numbers crates and enemies carry as loot) to the kind
//! of pickup, its amount, the logic that runs it, its picture and the sound
//! Claw plays when he takes it.
//!
//! A placed pickup, on its first logic call, finds its picture in the table
//! and takes the code, amount and sound from it (the level's own amount is
//! ignored). It sets the box that touches Claw, may start a sparkling
//! glitter next to itself and then waits. Claw's side applies the effect
//! (see [`crate::player::hit`]): it removes health, ammunition and special
//! pickups itself, and hands treasure and extra lives back with
//! [`send_to_hud`], after which they fly for 400 ms toward the score or
//! lives corner of the view and remove themselves. Taken treasure leaves a
//! small score number that rises 11 px and disappears.
//!
//! Items released by crates and enemies are bouncing goodies
//! ([`spawn_drop`], [`drop_loot`]): they hop out, can be taken 125 ms after
//! they appear, bounce up to four times at half speed and then become the
//! ordinary pickup of their code where they are. An item whose picture the
//! level does not have is not dropped.
//!
//! The map piece that ends a level shows `GAME_MAPPIECE` on odd levels
//! other than 13 and `LEVEL_GEM` on the others (see [`map_piece_set`]).
//!
//! A defeated boss releases the level's end as an [`END_GEM`]
//! ([`spawn_end_gem`], through [`World::boss_defeated`]): the map piece's
//! picture rises out of him on the slowest arc that reaches its target
//! under a pull of [`END_GEM_GRAVITY`] px/s/s downwards, with the camera
//! following it. Each tick the gem first points the camera at where it
//! is; once it is moving down and below its target's y it lets Claw go
//! ([`World::release_claw_boss_end`]), gives the camera back and turns,
//! where it is, into the end-of-level pickup that Claw collects to end the
//! level. Otherwise it moves for the tick, with exact positions for constant
//! acceleration, and shows the whole pixels of its position (towards 0).
//!
//! [`World::boss_defeated`]: super::World::boss_defeated
//! [`World::release_claw_boss_end`]: super::World::release_claw_boss_end
//!
//! A pickup touches Claw with the box of its picture's current frame, as
//! the level reports it (padded for treasure, special and curse pickups);
//! when the level does not know the frame it uses a 32 px square instead.
//!
//! [`census`] counts the treasure a level holds, by kind, for the end of
//! level tally.
//!
//! Pickups and glitter show a still image instead of cycling through their
//! set.

use claw_frame::SoundChannel;

use super::{ObjectData, World, new_object, sound};
use crate::level::{SimLevel, runtime_image_set};
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags, special, types};
use crate::physics::{ptype, res};
use crate::player::stats::TREASURE_KINDS;
use crate::registry::Registry;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "AmmoPowerup",
    BOUNCING_GOODIE,
    "CoinPowerup",
    CURSE,
    END_GEM,
    END_OF_LEVEL_LOGIC,
    "GlitterlessPowerup",
    "GoldPowerup",
    "HealthPowerup",
    "MagicPowerup",
    POINTS_ICON,
    GLITTER,
    SPECIAL,
    SUPER,
    TREASURE,
];

/// Logic name of the gem a defeated boss releases (see [`spawn_end_gem`]).
pub const END_GEM: &str = "EndLevelGem";
/// Logic name of the end-of-level pickup.
pub const END_OF_LEVEL_LOGIC: &str = "EndOfLevelPowerup";
/// Logic name of a bouncing goodie.
pub const BOUNCING_GOODIE: &str = "BouncingGoodie";
/// Logic name of the glitter that sparkles next to a pickup.
pub const GLITTER: &str = "PowerupGlitter";
/// Logic name of the score number left by taken treasure.
pub const POINTS_ICON: &str = "PointsIcon";
const TREASURE: &str = "TreasurePowerup";
const SPECIAL: &str = "SpecialPowerup";
const CURSE: &str = "CursePowerup";
const SUPER: &str = "SuperPowerup";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    "GAME_AMMO_DEATHBAG",
    "GAME_AMMO_SHOT",
    "GAME_AMMO_SHOTBAG",
    "GAME_CATNIPS_NIP1",
    "GAME_CATNIPS_NIP2",
    "GAME_CURSES_AMMO",
    "GAME_CURSES_FREEZE",
    "GAME_CURSES_HEALTH",
    "GAME_CURSES_LIFE",
    "GAME_CURSES_MAGIC",
    "GAME_CURSES_TREASURE",
    "GAME_DYNAMITE",
    "GAME_GLITTER",
    "GAME_GLITTERRED",
    "GAME_GREENGLITTER",
    "GAME_HEALTH_POTION1",
    "GAME_HEALTH_POTION2",
    "GAME_HEALTH_POTION3",
    "GAME_MAGICCLAW",
    "GAME_MAGIC_GLOW",
    "GAME_MAGIC_STARGLOW",
    "GAME_MAPPIECE",
    "GAME_POINTS",
    "GAME_POWERUPS_EXTRALIFE",
    "GAME_POWERUPS_FIRESWORD",
    "GAME_POWERUPS_GHOST",
    "GAME_POWERUPS_ICESWORD",
    "GAME_POWERUPS_INVULNERABLE",
    "GAME_POWERUPS_LIGHTNINGSWORD",
    "GAME_POWERUPS_PLASMASWORD",
    "GAME_TREASURE_CHALICES_BLUE",
    "GAME_TREASURE_CHALICES_GREEN",
    "GAME_TREASURE_CHALICES_PURPLE",
    "GAME_TREASURE_CHALICES_RED",
    "GAME_TREASURE_COINS",
    "GAME_TREASURE_CROSSES_BLUE",
    "GAME_TREASURE_CROSSES_GREEN",
    "GAME_TREASURE_CROSSES_PURPLE",
    "GAME_TREASURE_CROSSES_RED",
    "GAME_TREASURE_CROWNS_BLUE",
    "GAME_TREASURE_CROWNS_GREEN",
    "GAME_TREASURE_CROWNS_PURPLE",
    "GAME_TREASURE_CROWNS_RED",
    "GAME_TREASURE_GECKOS_BLUE",
    "GAME_TREASURE_GECKOS_GREEN",
    "GAME_TREASURE_GECKOS_PURPLE",
    "GAME_TREASURE_GECKOS_RED",
    "GAME_TREASURE_GOLDBARS",
    "GAME_TREASURE_JEWELEDSKULL_BLUE",
    "GAME_TREASURE_JEWELEDSKULL_GREEN",
    "GAME_TREASURE_JEWELEDSKULL_PURPLE",
    "GAME_TREASURE_JEWELEDSKULL_RED",
    "GAME_TREASURE_NECKLACE",
    "GAME_TREASURE_RINGS_BLUE",
    "GAME_TREASURE_RINGS_GREEN",
    "GAME_TREASURE_RINGS_PURPLE",
    "GAME_TREASURE_RINGS_RED",
    "GAME_TREASURE_SCEPTERS_BLUE",
    "GAME_TREASURE_SCEPTERS_GREEN",
    "GAME_TREASURE_SCEPTERS_PURPLE",
    "GAME_TREASURE_SCEPTERS_RED",
    "GAME_VERTWARP",
    "GAME_WARP",
    "LEVEL_GEM",
    "LEVEL_HEALTH",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_GLITTER", "GAME_POINTS"];

// ------------------------------------------------------------ the table

/// One row of the powerup table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Powerup {
    /// What kind of object the pickup is (see [`crate::object::types`]).
    pub type_flags: u32,
    /// The amount: points for treasure, health, shots or charges, the
    /// Catnip time in ms, or the special pickup's id.
    pub points: i32,
    /// The logic that runs the pickup.
    pub logic: &'static str,
    /// Its picture.
    pub image_set: &'static str,
    /// The sound Claw plays when he takes it.
    pub sound: &'static str,
}

const fn p(
    type_flags: u32,
    points: i32,
    logic: &'static str,
    image_set: &'static str,
    sound: &'static str,
) -> Powerup {
    Powerup {
        type_flags,
        points,
        logic,
        image_set,
        sound,
    }
}

const T: u32 = types::TREASURE;
const H: u32 = types::HEALTH;
const A: u32 = types::PISTOL_AMMO;
const M: u32 = types::MAGIC_AMMO;
const S: u32 = types::SPECIAL;
const AMMO: &str = "AmmoPowerup";
const HEALTH: &str = "HealthPowerup";
const MAGIC: &str = "MagicPowerup";

/// Number of powerup codes.
pub const POWERUP_CODES: usize = 62;

/// Special pickup id of the map piece that ends the level.
pub const END_OF_LEVEL: i32 = 9901;
/// Special pickup id of a warp.
pub const WARP: i32 = 9902;
/// Special pickup id of an extra life.
pub const EXTRA_LIFE: i32 = 9918;
/// Code of a warp.
pub const WARP_CODE: i32 = 32;
/// Code of the map piece.
pub const MAP_PIECE_CODE: i32 = 31;
/// Code of a coin (code 0 is a coin too, and placed coins report 33).
pub const COIN_CODE: i32 = 33;

/// The powerup table, indexed by code.
pub const POWERUPS: [Powerup; POWERUP_CODES] = [
    p(T, 100, TREASURE, "GAME_TREASURE_COINS", "GAME_COIN"),
    p(T, 500, TREASURE, "GAME_TREASURE_GOLDBARS", "GAME_TREASURE"),
    p(T, 1500, TREASURE, "GAME_TREASURE_RINGS_RED", "GAME_RINGS"),
    p(T, 1500, TREASURE, "GAME_TREASURE_RINGS_GREEN", "GAME_RINGS"),
    p(T, 1500, TREASURE, "GAME_TREASURE_RINGS_BLUE", "GAME_RINGS"),
    p(
        T,
        1500,
        TREASURE,
        "GAME_TREASURE_RINGS_PURPLE",
        "GAME_RINGS",
    ),
    p(T, 2500, TREASURE, "GAME_TREASURE_NECKLACE", "GAME_PICKUP1"),
    p(T, 5000, TREASURE, "GAME_TREASURE_CROSSES_RED", "GAME_CROSS"),
    p(
        T,
        5000,
        TREASURE,
        "GAME_TREASURE_CROSSES_GREEN",
        "GAME_CROSS",
    ),
    p(
        T,
        5000,
        TREASURE,
        "GAME_TREASURE_CROSSES_BLUE",
        "GAME_CROSS",
    ),
    p(
        T,
        5000,
        TREASURE,
        "GAME_TREASURE_CROSSES_PURPLE",
        "GAME_CROSS",
    ),
    p(
        T,
        7500,
        TREASURE,
        "GAME_TREASURE_SCEPTERS_RED",
        "GAME_SCEPTER",
    ),
    p(
        T,
        7500,
        TREASURE,
        "GAME_TREASURE_SCEPTERS_GREEN",
        "GAME_SCEPTER",
    ),
    p(
        T,
        7500,
        TREASURE,
        "GAME_TREASURE_SCEPTERS_BLUE",
        "GAME_SCEPTER",
    ),
    p(
        T,
        7500,
        TREASURE,
        "GAME_TREASURE_SCEPTERS_PURPLE",
        "GAME_SCEPTER",
    ),
    p(
        T,
        10000,
        TREASURE,
        "GAME_TREASURE_GECKOS_RED",
        "GAME_PICKUP2",
    ),
    p(
        T,
        10000,
        TREASURE,
        "GAME_TREASURE_GECKOS_GREEN",
        "GAME_PICKUP2",
    ),
    p(
        T,
        10000,
        TREASURE,
        "GAME_TREASURE_GECKOS_BLUE",
        "GAME_PICKUP2",
    ),
    p(
        T,
        10000,
        TREASURE,
        "GAME_TREASURE_GECKOS_PURPLE",
        "GAME_PICKUP2",
    ),
    p(A, 25, AMMO, "GAME_AMMO_DEATHBAG", "GAME_AMMUNITION"),
    p(A, 5, AMMO, "GAME_AMMO_SHOT", "GAME_AMMUNITION"),
    p(A, 10, AMMO, "GAME_AMMO_SHOTBAG", "GAME_AMMUNITION"),
    p(H, 15000, HEALTH, "GAME_CATNIPS_NIP1", "GAME_CATNMAG"),
    p(H, 30000, HEALTH, "GAME_CATNIPS_NIP2", "GAME_CATNMAG"),
    p(H, 5, HEALTH, "LEVEL_HEALTH", "GAME_FOODITEM"),
    p(H, 25, HEALTH, "GAME_HEALTH_POTION3", "GAME_MILK"),
    p(H, 10, HEALTH, "GAME_HEALTH_POTION1", "GAME_MILK"),
    p(H, 15, HEALTH, "GAME_HEALTH_POTION2", "GAME_MILK"),
    p(M, 5, MAGIC, "GAME_MAGIC_GLOW", "GAME_MAGICPOWERUP"),
    p(M, 10, MAGIC, "GAME_MAGIC_STARGLOW", "GAME_MAGICPOWERUP"),
    p(M, 35, MAGIC, "GAME_MAGICCLAW", "GAME_MAGICPOWERUP"),
    p(
        S,
        END_OF_LEVEL,
        END_OF_LEVEL_LOGIC,
        "GAME_MAPPIECE",
        "GAME_MAGICPOWERUP",
    ),
    p(S, WARP, SPECIAL, "GAME_WARP", "GAME_MAGICPOWERUP"),
    p(T, 100, TREASURE, "GAME_TREASURE_COINS", "GAME_COIN"),
    p(S, 9904, AMMO, "GAME_DYNAMITE", "GAME_AMMUNITION"),
    p(S, 9905, CURSE, "GAME_CURSES_AMMO", "GAME_CURSE3"),
    p(S, 9906, CURSE, "GAME_CURSES_MAGIC", "GAME_CURSE2"),
    p(S, 9907, CURSE, "GAME_CURSES_HEALTH", "GAME_CURSE6"),
    p(S, 9908, CURSE, "GAME_CURSES_LIFE", "GAME_CURSE5"),
    p(S, 9909, CURSE, "GAME_CURSES_TREASURE", "GAME_CURSE4"),
    p(S, 9910, CURSE, "GAME_CURSES_FREEZE", "GAME_CURSE1"),
    p(
        T,
        2500,
        TREASURE,
        "GAME_TREASURE_CHALICES_RED",
        "GAME_PICKUP1",
    ),
    p(
        T,
        2500,
        TREASURE,
        "GAME_TREASURE_CHALICES_GREEN",
        "GAME_PICKUP1",
    ),
    p(
        T,
        2500,
        TREASURE,
        "GAME_TREASURE_CHALICES_BLUE",
        "GAME_PICKUP1",
    ),
    p(
        T,
        2500,
        TREASURE,
        "GAME_TREASURE_CHALICES_PURPLE",
        "GAME_PICKUP1",
    ),
    p(
        T,
        15000,
        TREASURE,
        "GAME_TREASURE_CROWNS_RED",
        "GAME_PICKUP1",
    ),
    p(
        T,
        15000,
        TREASURE,
        "GAME_TREASURE_CROWNS_GREEN",
        "GAME_PICKUP1",
    ),
    p(
        T,
        15000,
        TREASURE,
        "GAME_TREASURE_CROWNS_BLUE",
        "GAME_PICKUP1",
    ),
    p(
        T,
        15000,
        TREASURE,
        "GAME_TREASURE_CROWNS_PURPLE",
        "GAME_PICKUP1",
    ),
    p(
        T,
        25000,
        TREASURE,
        "GAME_TREASURE_JEWELEDSKULL_RED",
        "GAME_PICKUP1",
    ),
    p(
        T,
        25000,
        TREASURE,
        "GAME_TREASURE_JEWELEDSKULL_GREEN",
        "GAME_PICKUP1",
    ),
    p(
        T,
        25000,
        TREASURE,
        "GAME_TREASURE_JEWELEDSKULL_BLUE",
        "GAME_PICKUP1",
    ),
    p(
        T,
        25000,
        TREASURE,
        "GAME_TREASURE_JEWELEDSKULL_PURPLE",
        "GAME_PICKUP1",
    ),
    p(S, 9911, SPECIAL, "GAME_POWERUPS_GHOST", "GAME_PICKUP1"),
    p(
        S,
        9917,
        SPECIAL,
        "GAME_POWERUPS_INVULNERABLE",
        "GAME_PICKUP1",
    ),
    p(
        S,
        EXTRA_LIFE,
        SPECIAL,
        "GAME_POWERUPS_EXTRALIFE",
        "GAME_EXTRALIFE",
    ),
    p(S, 9912, SPECIAL, "GAME_POWERUPS_FIRESWORD", "GAME_PICKUP1"),
    p(
        S,
        9913,
        SPECIAL,
        "GAME_POWERUPS_LIGHTNINGSWORD",
        "GAME_PICKUP1",
    ),
    p(S, 9914, SPECIAL, "GAME_POWERUPS_ICESWORD", "GAME_PICKUP1"),
    p(
        S,
        9915,
        SPECIAL,
        "GAME_POWERUPS_PLASMASWORD",
        "GAME_PICKUP1",
    ),
    p(S, WARP, SPECIAL, "GAME_VERTWARP", "GAME_MAGICPOWERUP"),
    p(H, 20, HEALTH, "LEVEL_HEALTH", "GAME_FOODITEM"),
];

/// The table row of `code`, if it is one.
pub fn powerup(code: i32) -> Option<&'static Powerup> {
    usize::try_from(code).ok().and_then(|c| POWERUPS.get(c))
}

/// The picture of the map piece on level `level`: `GAME_MAPPIECE` on odd
/// levels other than 13, `LEVEL_GEM` on the others.
pub fn map_piece_set(level: u32) -> &'static str {
    if level % 2 == 1 && level != 13 {
        "GAME_MAPPIECE"
    } else {
        "LEVEL_GEM"
    }
}

/// The picture of powerup `code` on level `level` (see [`map_piece_set`]).
pub fn image_set_in(code: i32, level: u32) -> Option<&'static str> {
    if code == MAP_PIECE_CODE {
        return Some(map_piece_set(level));
    }
    powerup(code).map(|e| e.image_set)
}

/// The code a picture belongs to on level `level`: the first row showing
/// it, with the map piece's row showing that level's picture only. The
/// other map piece picture belongs to no row there.
pub fn code_in(image_set: &str, level: u32) -> Option<i32> {
    if image_set.is_empty() {
        return None;
    }
    if image_set == map_piece_set(level) {
        return Some(MAP_PIECE_CODE);
    }
    POWERUPS
        .iter()
        .enumerate()
        .find(|&(c, e)| c as i32 != MAP_PIECE_CODE && e.image_set == image_set)
        .map(|(c, _)| c as i32)
}

/// The code a picture belongs to on any level: the first row showing it,
/// with the map piece found under either of its pictures (see
/// [`code_in`] for one level).
pub fn code_of(image_set: &str) -> Option<i32> {
    if image_set.is_empty() {
        return None;
    }
    if image_set == "LEVEL_GEM" {
        return Some(MAP_PIECE_CODE);
    }
    POWERUPS
        .iter()
        .position(|e| e.image_set == image_set)
        .map(|c| c as i32)
}

/// The treasure kind (index into the nine treasure counters: coin,
/// goldbar, ring, chalice, cross, scepter, gecko, crown, skull) of powerup
/// `code`; code 0 counts as a coin. `None` for codes that are not counted
/// as treasure (the necklace among them).
pub fn treasure_kind(code: i32) -> Option<usize> {
    match code {
        0 | COIN_CODE => Some(0),
        1 => Some(1),
        2..=5 => Some(2),
        41..=44 => Some(3),
        7..=10 => Some(4),
        11..=14 => Some(5),
        15..=18 => Some(6),
        45..=48 => Some(7),
        49..=52 => Some(8),
        _ => None,
    }
}

/// The treasure kind of a picture (see [`treasure_kind`]).
pub fn treasure_kind_of(image_set: &str) -> Option<usize> {
    code_of(image_set).and_then(treasure_kind)
}

// ------------------------------------------------------------ data

/// A pickup's data. Claw's side reads [`PickupData::kind`],
/// [`PickupData::points`], [`PickupData::smarts`] and
/// [`PickupData::sound`] together with the object's type, and the object's
/// `speed_x` and `speed_y` (a warp's destination).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PickupData {
    /// The powerup code a dropped item was released with.
    pub drop_code: Option<i32>,
    /// The code found from the picture (33 for a coin), 0 when the
    /// picture is in no row of the table.
    pub kind: i32,
    /// The amount, or the special pickup's id (see [`Powerup::points`]).
    pub points: i32,
    /// The level's own duration value, used by Catnip and kept by warps.
    pub smarts: i32,
    /// The sound Claw plays when he takes it ("" for none).
    pub sound: &'static str,
    /// The glitter sparkling next to it.
    pub glitter: Option<ObjectId>,
    /// For a glitter: the pickup it belongs to.
    pub owner: Option<ObjectId>,
    /// The flight to the status bar, once taken.
    pub flight: Option<Flight>,
    /// Motion of a bouncing goodie.
    pub goodie: Option<Goodie>,
    /// Flight of the gem a defeated boss releases.
    pub gem: Option<EndGem>,
}

/// The flight of the gem a defeated boss releases (see [`spawn_end_gem`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EndGem {
    /// Exact position, level pixels.
    pub pos: (f64, f64),
    /// Speed, px/s (down is positive).
    pub vel: (f64, f64),
    /// Where it heads for.
    pub target: (i32, i32),
}

/// A taken pickup's flight to the status bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flight {
    /// Where it flies to, in level pixels (fixed when it starts).
    pub target: (i32, i32),
    /// Horizontal speed, px/s.
    pub vx: i32,
    /// Vertical speed, px/s.
    pub vy: i32,
    /// Time left before it is removed, ms.
    pub left_ms: i32,
}

/// Motion of a bouncing goodie.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Goodie {
    /// Horizontal speed, px/s.
    pub vx: f64,
    /// Vertical speed, px/s (down is positive).
    pub vy: f64,
    /// Downward acceleration, px/s/s.
    pub gravity: f64,
    /// Sub-pixel movement carried to the next tick.
    pub rest: (f64, f64),
    /// Time since it appeared, ms.
    pub age_ms: u32,
    /// Bounces so far.
    pub bounces: u32,
    /// It landed on a deadly tile and flashes until it is removed.
    pub dying_ms: Option<i32>,
}

/// Mutable access to the pickup data of `o`, creating it if needed.
fn data_mut(o: &mut Object) -> &mut PickupData {
    if !matches!(o.data, ObjectData::Pickup(_)) {
        o.data = ObjectData::Pickup(PickupData::default());
    }
    match &mut o.data {
        ObjectData::Pickup(d) => d,
        _ => unreachable!("set above"),
    }
}

/// The pickup data of `o`, if it has any.
pub fn data_of(o: &Object) -> Option<&PickupData> {
    match &o.data {
        ObjectData::Pickup(d) => Some(d),
        _ => None,
    }
}

/// Marks the pickup `o` as taken by Claw: a treasure or an extra life
/// stops touching and, on its next calls, flies to the status bar and
/// removes itself. Claw's side calls this after adding the score or the
/// life.
pub fn send_to_hud(o: &mut Object) {
    o.special_flags |= special::FLYING_TO_HUD;
    o.flags |= flags::NO_COLLIDE;
}

// ------------------------------------------------------------ constants

/// Logic states of a pickup (the engine's own states are 28 to 30).
pub mod state {
    /// First call.
    pub const INIT: i32 = 0;
    /// Waiting to be taken.
    pub const IDLE: i32 = 5;
    /// Taken: the flight starts on this call.
    pub const FLIGHT_START: i32 = 9000;
    /// Flying to the status bar.
    pub const FLYING: i32 = 9001;
    /// A goodie on its way out of a crate or an enemy.
    pub const GOODIE_FALLING: i32 = 100;
    /// A goodie bouncing.
    pub const GOODIE_BOUNCING: i32 = 101;
    /// A goodie that landed on a deadly tile.
    pub const GOODIE_DYING: i32 = 102;
}

/// Box of a pickup's picture, relative to its position, when the level does
/// not know its frame: the touch box before padding.
pub const FRAME_BOX: Rect = Rect::new(-16, -16, 15, 15);
/// Touch box of treasure, special and curse pickups: the picture's box
/// grown by 6 px left and right, 10 px up and 2 px down.
pub const PADDED_BOX: Rect = Rect::new(
    FRAME_BOX.left - 6,
    FRAME_BOX.top - 10,
    FRAME_BOX.right + 6,
    FRAME_BOX.bottom + 2,
);
/// Touch box of a bouncing goodie once it can be taken: the picture's box
/// with its top raised 10 px.
pub const GOODIE_BOX: Rect = Rect::new(
    FRAME_BOX.left,
    FRAME_BOX.top - 10,
    FRAME_BOX.right,
    FRAME_BOX.bottom,
);

/// A treasure, special or curse pickup's touch box around the picture box
/// `b`: grown by 6 px left and right, 10 px up and 2 px down.
pub const fn padded(b: Rect) -> Rect {
    Rect::new(b.left - 6, b.top - 10, b.right + 6, b.bottom + 2)
}

/// A collectable goodie's touch box around the picture box `b`: its top
/// raised 10 px.
pub const fn goodie_box(b: Rect) -> Rect {
    Rect::new(b.left, b.top - 10, b.right, b.bottom)
}

/// Box of `o`'s current frame relative to its position, as the level
/// reports it (frame -1 is the set's first frame); [`FRAME_BOX`] when the
/// level does not know the frame.
pub fn picture_box(w: &World, o: &Object) -> Rect {
    let frame = if o.frame < 0 {
        w.image_frames(&o.image_set)
            .map_or(o.frame, |(first, _)| first)
    } else {
        o.frame
    };
    w.frame_box(&o.image_set, frame).unwrap_or(FRAME_BOX)
}

/// Pickups start a glitter next to themselves (the high-detail setting of
/// the original, on by default here).
pub const GLITTER_ON: bool = true;
/// Depth of a pickup placed at depth 0.
pub const PICKUP_Z: i32 = 1000;
/// Depth of a dropped goodie.
pub const GOODIE_Z: i32 = 4001;
/// Depth of the score number.
pub const POINTS_ICON_Z: i32 = 8900;
/// Upward speed of an item dropped by a single crate, px/s.
pub const DROP_SPEED: i32 = 300;
/// Gravity on goodies, px/s/s.
pub const GOODIE_GRAVITY: f64 = 800.0;
/// Time before a goodie can be taken, ms.
pub const GOODIE_COLLECT_MS: u32 = 125;
/// A goodie stops bouncing after this many bounces.
pub const GOODIE_BOUNCES: u32 = 4;
/// Fastest bounce, px/s up.
pub const GOODIE_MAX_BOUNCE: f64 = 500.0;
/// Time a goodie on a deadly tile flashes before it is removed, ms.
pub const GOODIE_DYING_MS: i32 = 1000;
/// Length of the flight to the status bar, ms.
pub const FLIGHT_MS: i32 = 400;
/// Treasure flies to this point: 20 px inside the left edge of the view
/// and 40 px above its top (the score corner of a 640x480 view).
pub const SCORE_CORNER: (i32, i32) = (20, -40);
/// An extra life flies to this point: 20 px inside the right edge of the
/// view and 40 px below its top (the lives corner of a 640x480 view).
pub const LIVES_CORNER: (i32, i32) = (-20, 40);

// ------------------------------------------------------------ dispatch

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let logic = o.logic.clone();
    match &*logic {
        BOUNCING_GOODIE => goodie_tick(w, id),
        END_GEM => gem_tick(w, id),
        GLITTER => glitter_tick(w, id),
        POINTS_ICON => points_icon_tick(w, id),
        _ => pickup_tick(w, id),
    }
}

/// `attacker` hit `id`. Pickups are only ever touched by Claw, whose side
/// handles it.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Nothing to do: Claw's side handles the touch.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

// ------------------------------------------------------------ pickups

fn flies(logic: &str) -> bool {
    matches!(
        logic,
        TREASURE | "GlitterlessPowerup" | "GoldPowerup" | "CoinPowerup" | SPECIAL
    )
}

fn pickup_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let (logic, st) = (o.logic.clone(), o.state);
    match st {
        engine_state::DESTROYING => drop_glitter(w, id),
        state::INIT => init_pickup(w, id),
        state::IDLE => {
            let o = w.objects.get_mut(id).expect("checked");
            if flies(&logic) && o.special_flags & special::FLYING_TO_HUD != 0 {
                o.state = state::FLIGHT_START;
            }
        }
        state::FLIGHT_START => start_flight(w, id),
        state::FLYING => fly(w, id),
        _ => {}
    }
}

/// The first call of a pickup: picture, kind, amount, touch box, glitter.
fn init_pickup(w: &mut World, id: ObjectId) {
    let level = w.level_number();
    let picture = picture_box(w, w.objects.get(id).expect("caller checked"));
    let o = w.objects.get_mut(id).expect("caller checked");
    let logic = o.logic.clone();
    if &*logic == CURSE {
        // Curses only work in multiplayer games: in single player a curse
        // removes itself on its first call, before it is ever drawn.
        o.flags |= flags::DESTROY | flags::NO_COLLIDE;
        return;
    }
    if o.z == 0 {
        o.z = PICKUP_Z;
        o.flags |= flags::RESORT;
    }
    o.flags |= flags::NOTIFY_DESTROY;
    o.type_flags = match &*logic {
        TREASURE | "GlitterlessPowerup" | "GoldPowerup" | "CoinPowerup" => types::TREASURE,
        HEALTH => types::HEALTH,
        MAGIC => types::MAGIC_AMMO,
        AMMO if o.image_set == "GAME_DYNAMITE" => types::SPECIAL,
        AMMO => types::PISTOL_AMMO,
        SUPER => types::SUPER_HEALTH,
        _ => types::SPECIAL,
    };
    let padded = matches!(
        &*logic,
        TREASURE | "GlitterlessPowerup" | "GoldPowerup" | "CoinPowerup" | SPECIAL
    );
    o.attack_rect = if padded {
        self::padded(picture)
    } else {
        picture
    };
    let smarts = o.record.as_ref().map_or(0, |r| r[17]);
    let code = code_in(&o.image_set, level);
    let glitter = GLITTER_ON
        && match &*logic {
            "GlitterlessPowerup" | "GoldPowerup" | "CoinPowerup" => false,
            SPECIAL => o.image_set != "GAME_WARP",
            _ => true,
        };
    let d = data_mut(o);
    d.glitter = None;
    d.flight = None;
    d.goodie = None;
    d.smarts = smarts;
    match code.and_then(|c| powerup(c).map(|e| (c, e))) {
        Some((c, e)) => {
            d.kind = if c == 0 { COIN_CODE } else { c };
            d.points = e.points;
            d.sound = e.sound;
        }
        None => {
            d.kind = 0;
            d.points = 100;
            d.sound = "";
        }
    }
    if &*logic == SUPER {
        d.points = 100;
    }
    o.state = state::IDLE;
    if glitter {
        let g = spawn_glitter(w, id);
        if let Some(o) = w.objects.get_mut(id) {
            data_mut(o).glitter = Some(g);
        }
    }
}

/// Removes the glitter of `id`, if it has one.
fn drop_glitter(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let Some(g) = data_mut(o).glitter.take() else {
        return;
    };
    w.destroy(g);
}

/// State 9000: start flying to the score corner (treasure, which leaves a
/// score number behind) or the lives corner (an extra life).
fn start_flight(w: &mut World, id: ObjectId) {
    drop_glitter(w, id);
    let view = w.view;
    let o = w.objects.get_mut(id).expect("caller checked");
    o.flags |= flags::NO_COLLIDE | flags::DESTROY_OUTSIDE;
    let life = &*o.logic == SPECIAL;
    let target = if life {
        (view.right + 1 + LIVES_CORNER.0, view.top + LIVES_CORNER.1)
    } else {
        (view.left + SCORE_CORNER.0, view.top + SCORE_CORNER.1)
    };
    let flight = Flight {
        target,
        vx: (target.0 - o.x) * 1000 / FLIGHT_MS,
        vy: (target.1 - o.y) * 1000 / FLIGHT_MS,
        left_ms: FLIGHT_MS,
    };
    let (x, y) = (o.x, o.y);
    let d = data_mut(o);
    d.flight = Some(flight);
    let points = d.points;
    o.state = state::FLYING;
    if !life {
        spawn_points_icon(w, x, y, points);
    }
}

/// State 9001: one step of the flight; removed when the 400 ms are over
/// (the step that ends them still moves).
fn fly(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).expect("caller checked");
    let Some(mut f) = data_mut(o).flight else {
        o.flags |= flags::DESTROY;
        return;
    };
    let dt = TICK_MS as i32;
    f.left_ms = (f.left_ms - dt).max(0);
    if f.left_ms == 0 {
        o.flags |= flags::DESTROY;
    }
    o.x += f.vx * dt / 1000;
    o.y += f.vy * dt / 1000;
    data_mut(o).flight = Some(f);
}

// ------------------------------------------------------------ glitter

/// Makes the glitter of pickup `owner`. It is not run when it is made: its
/// first call, with the draw of its start delay, comes when the update pass
/// reaches it, in the same pass when it lands after the object the pass
/// visits next, else in the next tick.
fn spawn_glitter(w: &mut World, owner: ObjectId) -> ObjectId {
    let (x, y, z) = w.objects.get(owner).map_or((0, 0, 0), |o| (o.x, o.y, o.z));
    let mut g = new_object(GLITTER, x, y, z + 1);
    g.flags |= flags::NO_COLLIDE;
    g.data = ObjectData::Pickup(PickupData {
        owner: Some(owner),
        ..PickupData::default()
    });
    w.spawn(g)
}

fn glitter_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    match o.state {
        state::INIT => {
            let delay = w.rng.below(251);
            let o = w.objects.get_mut(id).expect("checked");
            o.image_set = runtime_image_set("GAME_GLITTER");
            o.frame = -1;
            if o.z == 0 {
                o.z = PICKUP_Z + 1;
                o.flags |= flags::RESORT;
            }
            o.flags |= flags::NO_COLLIDE;
            o.time_delay = delay;
            o.state = state::IDLE;
        }
        engine_state::DESTROYING => {}
        _ => {
            // A glitter whose pickup is gone (or no longer owns it) goes
            // too.
            let owner = data_of(o).and_then(|d| d.owner);
            let owned = owner
                .and_then(|p| w.objects.get(p))
                .and_then(data_of)
                .is_some_and(|d| d.glitter == Some(id));
            if !owned {
                w.destroy(id);
            }
        }
    }
}

// ------------------------------------------------------------ score number

/// Frame of the score number showing `points` (0 for amounts it has no
/// picture for).
pub fn points_frame(points: i32) -> i32 {
    match points {
        100 => 1,
        500 => 2,
        1500 => 3,
        2500 => 4,
        5000 => 5,
        7500 => 6,
        10000 => 7,
        15000 => 8,
        25000 => 9,
        _ => 0,
    }
}

fn spawn_points_icon(w: &mut World, x: i32, y: i32, points: i32) -> ObjectId {
    let mut o = new_object(POINTS_ICON, x, y, POINTS_ICON_Z);
    o.image_set = runtime_image_set("GAME_POINTS");
    o.frame = points_frame(points);
    o.flags |= flags::NO_COLLIDE;
    o.data = ObjectData::Pickup(PickupData {
        points,
        ..PickupData::default()
    });
    w.spawn(o)
}

fn points_icon_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    match o.state {
        state::INIT => {
            o.flags |= flags::NO_COLLIDE | flags::DESTROY_OUTSIDE;
            o.state = state::IDLE;
            w.set_anim(id, "GAME_POINTS");
        }
        state::IDLE => {
            w.advance_anim(id);
            let o = w.objects.get_mut(id).expect("checked");
            if o.anim.is_done() {
                o.flags |= flags::DESTROY;
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------ goodies

/// True when the level has the image set `set`. A level that reports no
/// image sets at all (a level made in code) is taken to have every set.
pub fn level_has_set(level: &SimLevel, set: &str) -> bool {
    level.images.is_empty() || level.images.contains_key(set)
}

/// Drops the pickup `powerup_code` at (x, y), as a broken crate does, and
/// returns its id: a bouncing goodie that hops up at 300 px/s. A negative
/// code drops a coin; a code past the table, or one whose picture the
/// level does not have (see [`level_has_set`]), drops nothing. The
/// goodie's depth is [`GOODIE_Z`]; callers may change it before the pass
/// ends.
pub fn spawn_drop(w: &mut World, x: i32, y: i32, powerup_code: i32) -> Option<ObjectId> {
    let code = powerup_code.max(0);
    let set = image_set_in(code, w.level_number())?;
    if !level_has_set(w.level, set) {
        return None;
    }
    let mut o = new_object(BOUNCING_GOODIE, x, y, GOODIE_Z);
    o.image_set = runtime_image_set(set);
    o.frame = -1;
    // A goodie is not a pickup yet: it cannot touch anything until it can
    // be taken, and it stays active wherever it flies.
    o.type_flags = 1;
    o.flags |= flags::NO_COLLIDE | flags::ALWAYS_ACTIVE;
    o.physics_type = ptype::FLY;
    o.move_rect = FRAME_BOX;
    o.state = state::GOODIE_FALLING;
    o.data = ObjectData::Pickup(PickupData {
        drop_code: Some(code),
        goodie: Some(Goodie {
            vx: 0.0,
            vy: -f64::from(DROP_SPEED),
            gravity: GOODIE_GRAVITY,
            ..Goodie::default()
        }),
        ..PickupData::default()
    });
    Some(w.spawn(o))
}

/// Drops several items at (x, y), as a defeated enemy or a crate in a
/// stack does: `slots` are powerup codes in loot order, zero for none.
/// The items fan out horizontally 25 px/s apart, starting at
/// `-25 * n / 2` for `n` items (a single item gets a random speed of
/// -200..=200 px/s instead), and hop up at a random 200..=500 px/s. Returns
/// the goodies in slot order. The caller plays its own drop sound.
///
/// Random draws per item, in order: the horizontal speed (only for a
/// single item), then the vertical speed.
pub fn drop_loot(w: &mut World, x: i32, y: i32, slots: [i32; 9]) -> Vec<ObjectId> {
    let n = slots.iter().filter(|&&c| c != 0).count() as i32;
    let mut vx = (-25 * n) / 2;
    let mut out = Vec::new();
    for c in slots {
        if c != 0 {
            let id = spawn_drop(w, x, y, c);
            let gx = if n == 1 {
                w.rng.below(401) as i32 - 200
            } else {
                vx
            };
            let gy = -(w.rng.below(301) as i32 + 200);
            if let Some(id) = id {
                if let Some(o) = w.objects.get_mut(id)
                    && let Some(g) = data_mut(o).goodie.as_mut()
                {
                    g.vx = f64::from(gx);
                    g.vy = f64::from(gy);
                    g.gravity = GOODIE_GRAVITY;
                }
                out.push(id);
            }
        }
        vx += 25;
    }
    out
}

/// Gives a goodie its pickup values from its picture and lets it touch
/// Claw with `picture`, its picture's box.
fn make_collectable(o: &mut Object, level: u32, picture: Rect) {
    let code = code_in(&o.image_set, level);
    if o.type_flags == 1 {
        o.type_flags = code.and_then(powerup).map_or(1, |e| e.type_flags);
    }
    o.flags &= !flags::NO_COLLIDE;
    o.attack_rect = goodie_box(picture);
    let d = data_mut(o);
    match code.and_then(|c| powerup(c).map(|e| (c, e))) {
        Some((c, e)) => {
            d.kind = if c == 0 { COIN_CODE } else { c };
            d.points = e.points;
            d.sound = e.sound;
        }
        None => {
            d.kind = 0;
            d.points = 100;
            d.sound = "";
        }
    }
}

/// Turns a goodie into the ordinary pickup of its code, where it is.
fn settle(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).expect("caller checked");
    let code = data_mut(o).drop_code.unwrap_or(0);
    let logic = powerup(code).map_or(TREASURE, |e| e.logic);
    o.logic = Registry::shared()
        .get(logic)
        .map_or_else(|| logic.into(), |e| e.name.clone());
    o.flags &= !flags::ALWAYS_ACTIVE;
    o.state = state::INIT;
    data_mut(o).goodie = None;
    pickup_tick(w, id);
}

fn goodie_tick(w: &mut World, id: ObjectId) {
    let level = w.level_number();
    let picture = picture_box(w, w.objects.get(id).expect("caller checked"));
    let o = w.objects.get_mut(id).expect("caller checked");
    let st = o.state;
    if st == engine_state::DESTROYING {
        return;
    }
    let Some(mut g) = data_mut(o).goodie else {
        return;
    };
    if st == state::GOODIE_DYING {
        let left = g.dying_ms.unwrap_or(0) - TICK_MS as i32;
        g.dying_ms = Some(left);
        if left <= 0 {
            o.flags |= flags::DESTROY;
        }
        data_mut(o).goodie = Some(g);
        return;
    }
    // Ballistic motion through the tiles.
    let dt = f64::from(TICK_MS) / 1000.0;
    g.vy += g.gravity * dt;
    let fx = g.vx * dt + g.rest.0;
    let fy = g.vy * dt + g.rest.1;
    let (dx, dy) = (fx.trunc(), fy.trunc());
    g.rest = (fx - dx, fy - dy);
    let (tx, ty) = (o.x + dx as i32, o.y + dy as i32);
    o.physics_type = if o.physics_type == ptype::STAND {
        ptype::FLY
    } else {
        o.physics_type
    };
    let r = w.physics(id, tx, ty, 0);
    let o = w.objects.get_mut(id).expect("checked");
    if r & res::CEILING != 0 && g.vy < 0.0 {
        g.vy = 0.0;
        g.rest.1 = 0.0;
    }
    if r & res::WALL != 0 {
        g.rest.0 = 0.0;
    }
    let landed = o.physics_type == ptype::STAND;
    g.age_ms = g.age_ms.saturating_add(TICK_MS);
    if r & res::DEATH_TILE != 0 {
        o.state = state::GOODIE_DYING;
        o.draw_flags |= draw_flags::FLASH;
        o.flags |= flags::NO_COLLIDE;
        g.dying_ms = Some(GOODIE_DYING_MS);
        data_mut(o).goodie = Some(g);
        return;
    }
    let taken = o.special_flags & special::FLYING_TO_HUD != 0;
    if st == state::GOODIE_FALLING {
        if g.age_ms >= GOODIE_COLLECT_MS && o.type_flags == 1 {
            make_collectable(o, level, picture);
        }
        if taken {
            data_mut(o).goodie = Some(g);
            settle(w, id);
            return;
        }
        if landed {
            make_collectable(o, level, picture);
            bounce(o, &mut g);
            data_mut(o).goodie = Some(g);
            sound::play_from(w, id, "GAME_PUBOUNCE1", SoundChannel::Effect);
            return;
        }
    } else if st == state::GOODIE_BOUNCING {
        if g.bounces >= GOODIE_BOUNCES || taken {
            data_mut(o).goodie = Some(g);
            settle(w, id);
            return;
        }
        if landed {
            bounce(o, &mut g);
            data_mut(o).goodie = Some(g);
            sound::play_from(w, id, "GAME_PUBOUNCE1", SoundChannel::Effect);
            return;
        }
    }
    data_mut(o).goodie = Some(g);
}

/// A landing goodie hops back up at half speed (at most 500 px/s).
fn bounce(o: &mut Object, g: &mut Goodie) {
    g.bounces += 1;
    o.state = state::GOODIE_BOUNCING;
    o.physics_type = ptype::FLY;
    o.y -= 1;
    let v = (g.vy * g.vy - 2.0 * g.gravity).max(0.0).sqrt();
    g.vy = (-0.5 * v).max(-GOODIE_MAX_BOUNCE);
    g.rest.1 = 0.0;
}

// ------------------------------------------------------------ the boss's gem

/// The pull on the gem a defeated boss releases, px/s/s downwards.
pub const END_GEM_GRAVITY: f64 = 100.0;
/// The gem is not launched when the speed of its arc would be below this,
/// px/s.
pub const END_GEM_MIN_SPEED: f64 = 0.1;

/// The launch speed of the slowest arc from `from` to `to` under
/// [`END_GEM_GRAVITY`]: with the offset `d`, a flight time
/// `t = sqrt(2 |d| / g)` and the speed `d / t - (0, g) t / 2`, whose square
/// is `g |d| + g * (rise)`. Zero when that speed is below
/// [`END_GEM_MIN_SPEED`].
pub fn end_gem_launch(from: (f64, f64), to: (i32, i32)) -> (f64, f64) {
    let d = (f64::from(to.0) - from.0, f64::from(to.1) - from.1);
    let len = d.0.hypot(d.1);
    if len == 0.0 {
        return (0.0, 0.0);
    }
    let t = (2.0 * len / END_GEM_GRAVITY).sqrt();
    let v = (d.0 / t, d.1 / t - END_GEM_GRAVITY * t / 2.0);
    if v.0.hypot(v.1) < END_GEM_MIN_SPEED {
        (0.0, 0.0)
    } else {
        v
    }
}

/// Releases a level's end at (x, y, z), as a defeated boss does: the gem
/// shows the map piece's picture of the level, touches nothing and stays
/// awake. With `target` (0, 0) it lets Claw go and becomes the
/// end-of-level pickup at once; otherwise it is launched on the slowest
/// arc that reaches `target` (see [`end_gem_launch`]) and moves from the
/// next tick. Draws nothing.
pub fn spawn_end_gem(w: &mut World, (x, y, z): (i32, i32, i32), target: (i32, i32)) -> ObjectId {
    let set = map_piece_set(w.level_number());
    let mut o = new_object(END_GEM, x, y, z);
    o.image_set = runtime_image_set(set);
    o.frame = -1;
    o.flags |= flags::NO_COLLIDE | flags::ALWAYS_ACTIVE;
    o.physics_type = ptype::FLY;
    o.state = state::IDLE;
    let pos = (f64::from(x), f64::from(y));
    o.data = ObjectData::Pickup(PickupData {
        gem: Some(EndGem {
            pos,
            vel: end_gem_launch(pos, target),
            target,
        }),
        ..PickupData::default()
    });
    let id = w.spawn(o);
    if target == (0, 0) {
        land_gem(w, id);
    }
    id
}

/// One tick of the gem: the camera to where it is; once it moves down
/// below its target's y it lands, otherwise it moves.
fn gem_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != state::IDLE {
        return;
    }
    let Some(mut g) = data_mut(o).gem else {
        return;
    };
    let at = (o.x, o.y);
    w.set_camera_override(Some(at));
    if g.vel.1 > 0.0 && g.pos.1 > f64::from(g.target.1) {
        land_gem(w, id);
        return;
    }
    let dt = f64::from(TICK_MS) / 1000.0;
    g.pos.0 += g.vel.0 * dt;
    g.pos.1 += g.vel.1 * dt + 0.5 * END_GEM_GRAVITY * dt * dt;
    g.vel.1 += END_GEM_GRAVITY * dt;
    let o = w.objects.get_mut(id).expect("checked");
    o.x = g.pos.0.trunc() as i32;
    o.y = g.pos.1.trunc() as i32;
    data_mut(o).gem = Some(g);
}

/// The gem lands: Claw is let go, the camera is given back, and the gem
/// becomes the end-of-level pickup where it is, set up at once.
fn land_gem(w: &mut World, id: ObjectId) {
    w.release_claw_boss_end();
    w.set_camera_override(None);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.logic = Registry::shared()
        .get(END_OF_LEVEL_LOGIC)
        .map_or_else(|| END_OF_LEVEL_LOGIC.into(), |e| e.name.clone());
    o.flags &= !flags::NO_COLLIDE;
    o.physics_type = ptype::STAND;
    o.state = state::INIT;
    o.data = ObjectData::Pickup(PickupData::default());
    pickup_tick(w, id);
}

// ------------------------------------------------------------ curses and census

/// Applies the curse `curse_id` to Claw; true when it took effect. Curses
/// only work in multiplayer games, so in single player none takes effect
/// and nothing changes; Claw's side then removes the curse without its
/// sound.
pub fn curse_effect(w: &mut World, curse_id: i32) -> bool {
    let _ = (w, curse_id);
    false
}

/// Logics whose treasure is their picture.
const PICTURE_LOGICS: &[&str] = &[
    "AmmoPowerup",
    "CoinPowerup",
    "GlitterlessPowerup",
    "GoldPowerup",
    "HealthPowerup",
    "MagicPowerup",
    SPECIAL,
    SUPER,
    TREASURE,
];

/// Logics whose treasure is the code they drop (code 0 is a coin).
const CRATE_LOGICS: &[&str] = &["BehindCrate", "CratePowerup", "FrontCrate"];

/// Treasures the level holds, per kind (see
/// [`crate::player::stats::TREASURE_KINDS`] and [`treasure_kind`]),
/// counted from the level's records before anything runs:
///
/// - single crates count the code they drop (0 is a coin);
/// - pickups count their picture;
/// - every other object (stacked crates, enemies and so on) counts each
///   treasure code among its loot fields (the powerup field and the eight
///   user rectangle values).
///
/// Records the level does not load (no logic, an unknown logic, a
/// position outside the main plane) and path elevators are left out. The
/// count includes what enemies drop, so a perfect tally needs every enemy
/// defeated.
pub fn census(level: &SimLevel) -> [u32; TREASURE_KINDS] {
    let mut out = [0; TREASURE_KINDS];
    let plane = level.plane();
    let registry = Registry::shared();
    for spec in &level.objects {
        let r = &spec.record;
        let (x, y) = (r[5], r[6]);
        if spec.logic.is_empty()
            || x < 0
            || y < 0
            || x >= plane.width
            || y >= plane.height
            || registry.get(&spec.logic).is_none()
            || spec.logic == "PathElevator"
        {
            continue;
        }
        let logic = spec.logic.as_str();
        if CRATE_LOGICS.contains(&logic) {
            let code = if r[15] == 0 { COIN_CODE } else { r[15] };
            if let Some(k) = treasure_kind(code) {
                out[k] += 1;
            }
        } else if PICTURE_LOGICS.contains(&logic) {
            if let Some(k) = treasure_kind_of(&spec.image_set) {
                out[k] += 1;
            }
        } else {
            for &code in std::iter::once(&r[15]).chain(&r[35..43]) {
                if code != 0
                    && let Some(k) = treasure_kind(code)
                {
                    out[k] += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_complete_and_consistent() {
        assert_eq!(POWERUPS.len(), POWERUP_CODES);
        assert_eq!(code_of("GAME_TREASURE_COINS"), Some(0));
        assert_eq!(code_of("LEVEL_HEALTH"), Some(24), "the first row wins");
        assert_eq!(code_of("GAME_MAPPIECE"), Some(31));
        assert_eq!(code_of("LEVEL_GEM"), Some(31));
        assert_eq!(code_of("NOT_A_SET"), None);
        assert_eq!(code_of(""), None);
        for (c, e) in POWERUPS.iter().enumerate() {
            assert!(IMAGE_SETS.contains(&e.image_set), "{c}: {}", e.image_set);
            assert!(
                NAMES.contains(&e.logic) || e.logic == "EndOfLevelPowerup",
                "{c}"
            );
            if e.type_flags == types::TREASURE {
                assert_eq!(e.logic, TREASURE, "{c}");
            }
        }
        assert_eq!(treasure_kind(6), None, "the necklace is not counted");
        assert_eq!(treasure_kind(0), Some(0));
        assert_eq!(treasure_kind(52), Some(8));
        assert_eq!(treasure_kind(53), None);
    }

    #[test]
    fn padded_boxes() {
        assert_eq!(PADDED_BOX, Rect::new(-22, -26, 21, 17));
        assert_eq!(GOODIE_BOX, Rect::new(-16, -26, 15, 15));
        assert_eq!(padded(FRAME_BOX), PADDED_BOX);
        assert_eq!(goodie_box(FRAME_BOX), GOODIE_BOX);
    }

    #[test]
    fn the_map_piece_picture_depends_on_the_level() {
        for (level, set) in [
            (1, "GAME_MAPPIECE"),
            (2, "LEVEL_GEM"),
            (11, "GAME_MAPPIECE"),
            (13, "LEVEL_GEM"),
            (14, "LEVEL_GEM"),
        ] {
            assert_eq!(map_piece_set(level), set, "level {level}");
            assert_eq!(image_set_in(MAP_PIECE_CODE, level), Some(set));
            assert_eq!(code_in(set, level), Some(MAP_PIECE_CODE));
        }
        // The other picture is no map piece there.
        assert_eq!(code_in("LEVEL_GEM", 1), None);
        assert_eq!(code_in("GAME_MAPPIECE", 2), None);
        assert_eq!(code_in("GAME_TREASURE_COINS", 2), Some(0));
        assert_eq!(code_in("LEVEL_HEALTH", 1), Some(24));
        assert_eq!(code_in("", 1), None);
        assert_eq!(image_set_in(0, 2), Some("GAME_TREASURE_COINS"));
        assert_eq!(image_set_in(62, 1), None);
    }

    #[test]
    fn score_number_frames() {
        assert_eq!(points_frame(100), 1);
        assert_eq!(points_frame(25000), 9);
        assert_eq!(points_frame(9901), 0);
    }
}
