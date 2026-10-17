//! The cutthroat: a human enemy of the third level who slashes at Claw up
//! close and throws knives at him from afar. He never walks up to Claw.
//!
//! Not built yet ([`BUILT`] is false): cutthroats stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! State numbers ([`EnemyState`]): one number per state name, whichever
//! class runs it, so the cutthroat decides in `Home`, idles in `Idle`,
//! slashes in `Strike` and throws in `Fire`, with his own handlers under
//! those numbers, and inherits every other state with its shared number.
//! He and the robber thief take no number of their own: the block
//! [`STATE_BLOCK`] is reserved for the third level and holds no state.
//!
//! His knife is a `Bullet` ([`KNIFE_LOGIC`]) drawn from [`KNIFE_IMAGE`],
//! which the projectile family lists among its image sets.
//!
//! His data ([`CutThroatData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; the cutthroat then wraps it in his own.

use std::ops::RangeInclusive;

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["CutThroat"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// Enemy state numbers reserved for the third level's families (the
/// cutthroat and the robber thief). No state takes one yet: both classes
/// run their own handlers under the shared numbers.
pub const STATE_BLOCK: RangeInclusive<i32> = 2100..=2119;

/// Logic of the knife.
pub const KNIFE_LOGIC: &str = "Bullet";
/// Image set of the knife.
pub const KNIFE_IMAGE: &str = "LEVEL_KNIFE";

/// The cutthroat class.
pub static CLASS: Class = Class {
    name: "CutThroat",
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

/// A cutthroat's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CutThroatData {
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
