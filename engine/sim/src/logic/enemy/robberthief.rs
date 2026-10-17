//! The robber thief: a human enemy of the third level who shoots arrows at
//! Claw whenever he sees him, and has no blow up close. He never walks up
//! to Claw.
//!
//! Not built yet ([`BUILT`] is false): robber thieves stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing.
//!
//! State numbers ([`EnemyState`]): one number per state name, whichever
//! class runs it, so the robber thief decides in `Home` and shoots in
//! `Fire`, with his own handlers under those numbers, and inherits every
//! other state with its shared number. He takes no number of his own (see
//! [`super::cutthroat::STATE_BLOCK`]).
//!
//! His arrow is a `Bullet` ([`ARROW_LOGIC`]) drawn from [`ARROW_IMAGE`],
//! which the projectile family lists among its image sets.
//!
//! His data ([`RobberThiefData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; the robber thief then wraps it in his own.

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RobberThief"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Logic of the arrow.
pub const ARROW_LOGIC: &str = "Bullet";
/// Image set of the arrow.
pub const ARROW_IMAGE: &str = "LEVEL_ARROW";

/// The robber thief class.
pub static CLASS: Class = Class {
    name: "RobberThief",
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

/// A robber thief's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RobberThiefData {
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
