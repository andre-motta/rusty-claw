//! Raux, the boss of the second level: a human enemy who waits for the
//! fight to start, then dodges, blocks and strikes back.
//!
//! Not built yet ([`BUILT`] is false): Raux stays an inert placeholder
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! State numbers ([`EnemyState`]): one number per state name, so Raux
//! waits for the fight in the bosses' `Startup` and uses the shared
//! numbers for the states he shares with other enemies; his own leap over
//! Claw is `JumpOver`, the first of his block (2070-2089).
//!
//! His data ([`RauxData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; Raux then wraps it in his own.

use super::{Class, ClassAnims, ClassSounds, EnemyData, EnemyState, Handler, nothing};
use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Raux"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// The Raux class.
pub static CLASS: Class = Class {
    name: "Raux",
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

/// Raux's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RauxData {
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
