//! Pickups: treasure, health, ammunition, powerups and curses (to be
//! filled in; until then they cannot be collected).

use super::World;
use crate::level::SimLevel;
use crate::logic::{ObjectData, placeholder};
use crate::object::ObjectId;
use crate::player::stats::TREASURE_KINDS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "AmmoPowerup",
    "CursePowerup",
    "GlitterlessPowerup",
    "HealthPowerup",
    "SpecialPowerup",
    "TreasurePowerup",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    "GAME_GLITTER",
    "GAME_GLITTERRED",
    "GAME_GREENGLITTER",
    "GAME_POINTS",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_GLITTER", "GAME_POINTS"];

/// A pickup's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PickupData {
    /// The powerup code a drop was asked for with.
    pub drop_code: Option<i32>,
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

/// Logic name of a dropped pickup's placeholder.
pub const DROP_PLACEHOLDER: &str = "PowerupDrop";

/// Drops the pickup `powerup_code` at (x, y), as a broken crate or a
/// defeated enemy does, and returns its id. Until pickups are filled in,
/// this spawns an inert placeholder there whose data holds the code.
pub fn spawn_drop(w: &mut World, x: i32, y: i32, powerup_code: i32) -> Option<ObjectId> {
    let data = ObjectData::Pickup(PickupData {
        drop_code: Some(powerup_code),
    });
    Some(w.spawn(placeholder(DROP_PLACEHOLDER, x, y, 0, data)))
}

/// Applies the curse `curse_id` to Claw; true when it took effect. No
/// curse takes effect yet.
pub fn curse_effect(w: &mut World, curse_id: i32) -> bool {
    let _ = (w, curse_id);
    false
}

/// Treasures the level holds, per kind (see
/// [`crate::player::stats::TREASURE_KINDS`]). Not counted yet: all zero.
pub fn census(level: &SimLevel) -> [u32; TREASURE_KINDS] {
    let _ = level;
    [0; TREASURE_KINDS]
}
