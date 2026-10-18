//! Stepping stones: small stones floating on a goo pool that Claw hops
//! across. Two kinds share their art, landing surface and animations:
//!
//! - clock stones (`SteppingStone` to `SteppingStone4`) sink and rise on a
//!   rhythm of the level clock, like the toggle pegs: by default up for
//!   1.5 s and under for 1.5 s, each variant a quarter period after the
//!   previous one ([`phase_of`]), so a row 1, 2, 3, 4 sinks as a wave. A
//!   stone stops carrying Claw the moment it starts to sink and carries
//!   him again from the middle frame of its rise;
//! - contact stones (`StartSteppingStone`) wait, solid, until Claw stands
//!   on one; a short delay later it sinks (Claw loses it 12 ticks after
//!   landing), stays under for a second and comes back up, ready again.
//!   `OneTimeStartSteppingStone` stays under until Claw loses a life; no
//!   level places one.
//!
//! Both are platforms that never move, with the level's stepping-stone
//! surface ([`surface`]). Their two sound keys, [`SINK_SOUND`] and
//! [`RISE_SOUND`], name no sound any level ships, so the stones are
//! silent; the calls are made anyway and never fall back to another key.
//! The sink plays [`SINK_SOUND`] and the rise [`RISE_SOUND`].
//!
//! Not built yet ([`BUILT`] is false): stones stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.

use super::World;
use super::platform::{self, SurfaceKind};
use crate::object::{ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "SteppingStone",
    "SteppingStone2",
    "SteppingStone3",
    "SteppingStone4",
    "StartSteppingStone",
    "OneTimeStartSteppingStone",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// Depth given to a stone placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Length of each half of a clock stone's cycle when placed without one,
/// ms.
pub const DEFAULT_HALF_MS: i32 = 1500;
/// Delay between a contact stone seeing Claw and its sink, ms.
pub const CONTACT_DELAY_MS: u32 = 125;
/// Time a contact stone stays under before rising, ms.
pub const UNDER_MS: u32 = 1000;
/// Played, when the stone was drawn in the previous tick, as a clock
/// stone starts to sink. No level ships it.
pub const SINK_SOUND: &str = "LEVEL_STONERISE";
/// Played, when the stone was drawn in the previous tick, as a clock
/// stone starts to rise. No level ships it.
pub const RISE_SOUND: &str = "LEVEL_STONESINK";

/// The cycle offset of the clock stone running `name`, ms:
/// `SteppingStone` takes `record_speed`, the others a quarter, half and
/// three quarters of the default cycle.
pub fn phase_of(name: &str, record_speed: i32) -> i32 {
    match name {
        "SteppingStone2" => 750,
        "SteppingStone3" => 1500,
        "SteppingStone4" => 2250,
        _ => record_speed,
    }
}

/// True for the contact stones' names.
pub fn is_contact(name: &str) -> bool {
    matches!(name, "StartSteppingStone" | "OneTimeStartSteppingStone")
}

/// The landing surface of a stone in level `level`, relative to its
/// position: the level's stepping-stone rectangle, as the elevators that
/// wear the stone's art use it.
pub fn surface(level: u32) -> Rect {
    platform::surface(level, SurfaceKind::SteppingStone)
}

/// A stepping stone's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SteppingStoneData {}

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
    fn variants_differ_by_a_quarter_cycle() {
        assert_eq!(phase_of("SteppingStone", 2000), 2000);
        assert_eq!(phase_of("SteppingStone2", 0), 750);
        assert_eq!(phase_of("SteppingStone3", 0), 1500);
        assert_eq!(phase_of("SteppingStone4", 0), 2250);
        assert!(is_contact("StartSteppingStone"));
        assert!(is_contact("OneTimeStartSteppingStone"));
        assert!(!is_contact("SteppingStone"));
    }

    #[test]
    fn the_fourth_level_stone_surface_is_32_px_wide() {
        assert_eq!(surface(4), Rect::new(-16, -1, 16, 64));
    }
}
