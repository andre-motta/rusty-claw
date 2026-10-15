//! Shots, thrown dynamite and the other flying objects (to be filled in;
//! until then [`spawn`] leaves an inert placeholder).

use super::World;
use crate::logic::{ObjectData, placeholder};
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Bullet",
    "Dynamite",
    "MagicBullet",
    "PistolBullet",
    "RatBomb",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    "GAME_BULLETS",
    "GAME_DYNAMITE",
    "GAME_DYNAMITELIT",
    "GAME_MAGICCLAW",
    "LEVEL_MUSKETBALL",
    "LEVEL_RATBOMB",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["GAME_DYNAMITE"];

/// A projectile's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectileData {
    /// What was asked for when it was launched.
    pub spec: Option<ProjectileSpec>,
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

/// A projectile that hit Claw and goes off instead of disappearing (its
/// special flags ask for this). Until projectiles are filled in, this does
/// nothing.
pub fn impact(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// What to launch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectileSpec {
    /// Logic name of the projectile.
    pub logic: &'static str,
    /// Start position.
    pub x: i32,
    /// Start position.
    pub y: i32,
    /// Depth.
    pub z: i32,
    /// Horizontal speed, px/s.
    pub vx: i32,
    /// Vertical speed, px/s.
    pub vy: i32,
    /// Damage it deals.
    pub damage: i32,
    /// What kind of object it is.
    pub type_flags: u32,
    /// Kinds that can hit it.
    pub hit_type_flags: u32,
    /// Logic-specific flags.
    pub special_flags: u32,
    /// Image set it is drawn from.
    pub image_set: &'static str,
    /// Object that launched it.
    pub owner: Option<ObjectId>,
    /// Time before it goes off, for dynamite, ms.
    pub fuse_ms: Option<u32>,
    /// Strength of a charged throw.
    pub power: i32,
}

/// Launches a projectile and returns its id. Until projectiles are filled
/// in, this spawns an inert placeholder at the start position whose data
/// holds `spec`.
pub fn spawn(w: &mut World, spec: ProjectileSpec) -> Option<ObjectId> {
    let data = ObjectData::Projectile(ProjectileData { spec: Some(spec) });
    let mut o = placeholder(spec.logic, spec.x, spec.y, spec.z, data);
    o.speed_x = spec.vx;
    o.speed_y = spec.vy;
    Some(w.spawn(o))
}
