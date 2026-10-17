//! Enemies entering a death tile: spikes, or the goo of the levels that
//! have goo pools.
//!
//! What a death tile is depends on the level, not on the tile: the levels
//! with goo pools (2, 4, 6, 7, 8, 9, 12, 13 and 14, see [`GOO_LEVELS`])
//! have goo, the others spikes. The movement step of an enemy reports when
//! it ends on a death tile while alive and not flying off as a body;
//! [`super::die`] then hands the enemy to [`enter_goo`], which decides how
//! it dies there:
//!
//! - in goo, a human enemy (an officer, a soldier) first drops its loot;
//!   then the enemy is dead, busy and harmless, frozen, and removed in the
//!   same step, before it is ever drawn in the goo. Human enemies play the
//!   death tile sound; the rats sink without a sound. No random draw
//!   besides the loot's;
//! - on spikes, a human enemy dies as before: the body stays where it is,
//!   flashing, with the death tile sound, and is removed after a second
//!   (see [`super::die_of`]). The rats die there the same way, but without
//!   a sound, as they have none for it.

use super::{
    DEATH_DELAY_MS, DeathKind, EnemyState, Timer, class_of, data, data_mut, die_of, obj_mut,
    set_state,
};
use crate::logic::World;
use crate::object::{ObjectId, draw_flags, flags};

/// The levels whose death tiles are goo; every other level's are spikes.
pub const GOO_LEVELS: [u32; 9] = [2, 4, 6, 7, 8, 9, 12, 13, 14];

/// True when the death tiles of level `number` are goo.
pub fn is_goo_level(number: u32) -> bool {
    GOO_LEVELS.contains(&number)
}

/// How an enemy dies on a death tile of the level being played: in goo on
/// the goo levels, on spikes on the others.
pub fn death_tile_kind(w: &World) -> DeathKind {
    if is_goo_level(w.level_number()) {
        DeathKind::Goo
    } else {
        DeathKind::Spikes
    }
}

/// Enemy `id` entered a death tile (see the module documentation).
pub fn enter_goo(w: &mut World, id: ObjectId) {
    let kind = death_tile_kind(w);
    let human = class_of(w, id).is_some_and(|c| c.human);
    if human {
        die_of(w, id, kind, None);
    } else {
        die_silently(w, id, kind);
    }
}

/// The death on a death tile of an enemy with no sound for it (the rats):
/// as [`die_of`] for that kind, without the sound and without loot.
fn die_silently(w: &mut World, id: ObjectId, kind: DeathKind) {
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.death = kind;
        d.busy = true;
        d.cooldown = Timer::default();
    }
    set_state(w, id, EnemyState::Dead);
    obj_mut(w, id).flags |= flags::HARMLESS;
    if let Some(d) = data_mut(w, id) {
        d.frozen = true;
        d.death_timer = Timer::new(now, DEATH_DELAY_MS);
    }
    if kind == DeathKind::Goo {
        w.destroy(id);
    } else {
        obj_mut(w, id).draw_flags |= draw_flags::FLASH;
    }
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
    }
}

/// True when enemy `id` died in goo.
pub fn died_in_goo(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| d.dead && d.death == DeathKind::Goo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goo_levels() {
        for n in 1..=14 {
            assert_eq!(
                is_goo_level(n),
                ![1, 3, 5, 10, 11].contains(&n),
                "level {n}"
            );
        }
        assert!(!is_goo_level(0));
        assert!(!is_goo_level(15));
    }
}
