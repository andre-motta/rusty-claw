//! Hit bursts, splashes and explosions (to be filled in; until then
//! [`spawn`] leaves an inert placeholder).

use super::World;
use crate::logic::{ObjectData, placeholder};
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["HitBurst", "Splash"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["GAME_CLAWHIT", "GAME_DYNAMITEEXPLO", "GAME_ENEMYHIT"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[
    "GAME_BIGHITBURST",
    "GAME_DYNAMITEEXPLO",
    "GAME_MEDHITBURST",
    "GAME_SMLHITBURST",
    "GAME_SPLASH",
];

/// An effect's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectData {
    /// What was asked for when it was spawned.
    pub request: Option<EffectKind>,
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

/// What effect to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// The spark of a blow landing.
    HitBurst,
    /// A splash in water or goo.
    Splash,
    /// An explosion, hurting what is within `radius` px.
    Explosion {
        /// Damage dealt.
        damage: i32,
        /// Reach, px.
        radius: i32,
        /// What exploded.
        source: Option<ObjectId>,
    },
}

/// Shows an effect at (x, y) and returns its id. Until effects are filled
/// in, this spawns an inert placeholder there whose data holds the
/// request.
pub fn spawn(w: &mut World, kind: EffectKind, x: i32, y: i32) -> Option<ObjectId> {
    let name = match kind {
        EffectKind::HitBurst => "HitBurst",
        EffectKind::Splash => "Splash",
        EffectKind::Explosion { .. } => "Explosion",
    };
    let data = ObjectData::Effect(EffectData {
        request: Some(kind),
    });
    Some(w.spawn(placeholder(name, x, y, 0, data)))
}

/// Shows a hit burst at (x, y, z). With an `owner` the burst follows it;
/// `claw_image` picks Claw's burst art over an enemy's. Until effects are
/// filled in, this leaves the same placeholder as [`spawn`].
pub fn spawn_hit_burst(
    w: &mut World,
    x: i32,
    y: i32,
    z: i32,
    owner: Option<ObjectId>,
    claw_image: bool,
) -> Option<ObjectId> {
    let _ = (z, owner, claw_image);
    spawn(w, EffectKind::HitBurst, x, y)
}

/// Starts the glitter that trails `target` while a powerup lasts and
/// returns the object that drives it; the caller removes it when the
/// powerup ends. Until effects are filled in, this does nothing.
pub fn spawn_glitter(w: &mut World, target: ObjectId) -> Option<ObjectId> {
    let _ = (w, target);
    None
}
