//! Claw's deaths: losing all health, touching a death tile, the two death
//! states, and the powerup timer.
//!
//! Until dying is filled in, [`kill`] keeps the earlier behaviour: a death
//! tile is counted in [`super::Claw::death_tile_hits`] and Claw carries on;
//! running out of health changes nothing else. The death states fall back
//! to standing.

use super::{Ctx, fallback_tick};

/// Why Claw dies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeathCause {
    /// Health ran out.
    Damage,
    /// He touched a death tile (spikes, goo, lava).
    DeathTile,
    /// The checkpoint countdown ran out.
    Timer,
}

/// Kills Claw.
pub fn kill(c: &mut Ctx, cause: DeathCause) {
    if cause == DeathCause::DeathTile {
        c.p.death_tile_hits += 1;
    }
}

/// Falling off the screen after losing all health (state 5004).
pub fn death_fall_tick(c: &mut Ctx) {
    fallback_tick(c);
}

/// Dying in place on a death tile (state 5005).
pub fn death_tick(c: &mut Ctx) {
    fallback_tick(c);
}

/// Counts the running powerup down; called after every state handler.
pub fn powerup_tick(c: &mut Ctx) {
    let _ = c;
}
