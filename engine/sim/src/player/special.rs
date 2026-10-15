//! Claw's special states: the hit reaction, lifting, throwing and being
//! frozen.
//!
//! Their rows in the state table call the tick functions here. Until they
//! are filled in, each falls back to standing (see
//! [`super::fallback_tick`]).

use super::{Ctx, fallback_tick};
use crate::object::ObjectId;

/// Data of the special states.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecialData {
    /// The object Claw carries.
    pub lifted: Option<ObjectId>,
    /// Time left frozen, ms.
    pub freeze_ms: i32,
}

/// Starts the hit reaction, pushed away from an attacker at `attacker_x`.
/// Not filled in yet: Claw keeps doing what he did.
pub fn enter_hit(c: &mut Ctx, attacker_x: i32) {
    let _ = (c, attacker_x);
}

/// The hit reaction (state 31).
pub fn hit_tick(c: &mut Ctx) {
    fallback_tick(c);
}

/// Lifting an object (state 5000).
pub fn lift_tick(c: &mut Ctx) {
    fallback_tick(c);
}

/// Throwing the lifted object (state 5001).
pub fn throw_tick(c: &mut Ctx) {
    fallback_tick(c);
}

/// Frozen (state 5009).
pub fn freeze_tick(c: &mut Ctx) {
    fallback_tick(c);
}
