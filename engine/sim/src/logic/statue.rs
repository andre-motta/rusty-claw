//! Statues: breakable stone figures in the scenery, the statue branch of
//! the crates ([`super::crates`]).
//!
//! A statue breaks like a single crate (one hit at health 1, a damaged
//! picture and a 500 ms pause at health 2, never at 999 or more), with
//! three differences: its treasure comes out as a fan of bouncing goodies,
//! one per non-empty loot field in record order, from `-12.5 n` px/s
//! sideways in steps of 25 and each with its own random upward speed; that
//! fan is released at the statue's depth minus one (`FrontStatue`) or
//! plus one (`BehindStatue`, [`drop_z_offset`]), 24 px above it; and a
//! broken statue is never removed but stays as rubble, the last picture of
//! the break, for the rest of the attempt.
//!
//! The two names differ only in depth: the level places a front statue in
//! front of Claw and the enemies and a behind statue behind them. Neither
//! blocks or carries anything.
//!
//! Not built yet ([`BUILT`] is false): statues stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # Defaults taken
//!
//! - The hit box is the first picture's box, unmirrored, while the picture
//!   is drawn mirrored as the record asks; on the fifth level's statues
//!   that leaves the hittable box 6 px to the right of the art, as in the
//!   original game.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[FRONT, BEHIND];

/// Image sets this family may switch an object to: none (a statue keeps
/// its record's set).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[BREAK_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The statue drawn in front of Claw.
pub const FRONT: &str = "FrontStatue";
/// The statue drawn behind Claw.
pub const BEHIND: &str = "BehindStatue";
/// Animation of a breaking statue, the crates' break.
pub const BREAK_ANIM: &str = super::crates::BREAK_ANIM;
/// Depth given to a statue placed at depth 0 (either name).
pub const DEFAULT_Z: i32 = 1100;
/// Vertical offset of the point the treasure is released from, px: 24 px
/// above the statue's position.
pub const DROP_Y: i32 = -24;
/// Sideways speed step between two goodies of the fan, px/s.
pub const FAN_STEP: i32 = 25;

/// The depth offset of the treasure of the statue running `name`.
pub fn drop_z_offset(name: &str) -> Option<i32> {
    match name {
        FRONT => Some(-1),
        BEHIND => Some(1),
        _ => None,
    }
}

/// The sideways speed of the first goodie of a fan of `n`, px/s.
pub fn fan_start(n: i32) -> i32 {
    -FAN_STEP * n / 2
}

/// A statue's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatueData {}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_treasure_flies_just_in_front_and_behind_treasure_just_behind() {
        assert_eq!(drop_z_offset(FRONT), Some(-1));
        assert_eq!(drop_z_offset(BEHIND), Some(1));
        assert_eq!(drop_z_offset("FrontCrate"), None);
    }

    #[test]
    fn the_fan_is_centred_on_the_statue() {
        // Eight treasures: -100, -75, ..., 75.
        assert_eq!(fan_start(8), -100);
        assert_eq!(fan_start(8) + 7 * FAN_STEP, 75);
        // Odd counts round towards zero.
        assert_eq!(fan_start(3), -37);
        assert_eq!(fan_start(1), -12);
        assert_eq!(fan_start(0), 0);
    }
}
