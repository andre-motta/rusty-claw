//! The shared human-enemy states beyond those the first level's officers
//! and soldiers use: backing off from Claw and stepping in towards him, a
//! hop up, a hop back, blocking high and low, the human idle state, and
//! being launched off a springboard. Every later human enemy and boss
//! reuses them.
//!
//! This is not a logic family of its own: it runs no logic name
//! ([`NAMES`] is empty). The human classes reach it through the shared
//! enemy behaviour: every enemy keeps a [`HumanData`] in
//! [`super::EnemyFields::human`], and a call a human state schedules for
//! the post-tick is a [`super::Deferred::Human`] carrying a [`HumanCall`],
//! run by [`run_deferred`].
//!
//! Not built yet: [`HumanCall`] has no calls and the hooks below do
//! nothing.
//!
//! State numbers ([`EnemyState`](super::EnemyState)): one number per state
//! name. The new names take the block 2030-2049 (`JumpBack`, `MoveBack`,
//! `MoveAhead`, `BlockHigh`, `BlockLow`, `HumanIdle`,
//! `JumpOffSpringboard`); names that already have a number keep it
//! (`HitHigh`, `HitLow`, `JumpAtClaw`, `Thrown`, `Jump`).

use super::{Class, EnemyState, Handler};
use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this module runs: none, the human classes use it.
pub const NAMES: &[&str] = &[];

/// Image sets the shared human states may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations the shared human states may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// What the shared human states keep per enemy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HumanData {}

/// A call of the shared human states scheduled for the post-tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanCall {}

/// The shared human handler of state `s`, when there is one.
pub fn handler(s: EnemyState) -> Option<Handler> {
    let _ = s;
    None
}

/// Runs `call`, scheduled for the post-tick of enemy `id` of `class`.
pub fn run_deferred(w: &mut World, id: ObjectId, class: &Class, call: HumanCall) {
    let _ = (w, id, class);
    match call {}
}

/// The shared human step of a logic call of enemy `id`.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// The shared human step when `attacker` hits `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// The shared human step when `id` attacks `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}
