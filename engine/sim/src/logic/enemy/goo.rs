//! Enemies entering a death tile: spikes, or the goo of the levels that
//! have goo pools.
//!
//! The movement step of an enemy reports when it ends on a death tile
//! while alive; [`super::die`] then hands the enemy to [`enter_goo`],
//! which decides how it dies there. For now every death tile kills as
//! spikes do: the body stays where it is, flashing, with the death tile
//! sound, and is removed after a second (see [`super::die_of`]).

use super::DeathKind;
use crate::logic::World;
use crate::object::ObjectId;

/// Enemy `id` entered a death tile.
pub fn enter_goo(w: &mut World, id: ObjectId) {
    super::die_of(w, id, DeathKind::Spikes, None);
}
