//! The wind RedTail raises across his arena from the ledge. It is an
//! invisible object that waits until RedTail raises his wind flag
//! ([`crate::logic::enemy::redtail::RedTailLink::wind`]); then it blows
//! for [`BLOW_MS`], whistling, blowing debris across the arena
//! ([`super::debris`], one piece every [`DEBRIS_MS`]) and pushing Claw
//! toward the stakes ([`push_claw`]) while his position lies in
//! [`BOX`]. The blow ends early when the flag is cleared (Claw's
//! respawn), and its end clears the flag.
//!
//! The push is the conveyor belts' ([`crate::logic::conveyor_belt`]):
//! Claw's existing movement reads it from his next tick on, a standing
//! Claw drifting 5 px a tick, a walking or jumping one moving at his own
//! speed plus [`PUSH`], a falling one not at all. The wind writes it on
//! every tick it blows, after Claw's and RedTail's ticks, whatever Claw
//! is doing.
//!
//! Not built yet ([`BUILT`] is false): the placed record stays an inert
//! placeholder and the calls below do nothing. [`push_claw`] is the seam
//! the built wind writes Claw's push through.

use crate::logic::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailWind"];

/// Image sets this family may switch an object to: none (it is hidden).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = false;

/// How long one blow lasts, ms (278 ticks at the 18 ms step).
pub const BLOW_MS: u32 = 5000;
/// The time between two pieces of debris, ms (every 6 ticks).
pub const DEBRIS_MS: u32 = 100;
/// The push on Claw while he is in [`BOX`], px/s (to the left).
pub const PUSH: f64 = -300.0;
/// Where Claw's position must lie for the push: left and top inside,
/// right and bottom outside (x0, y0, x1, y1).
pub const BOX: (i32, i32, i32, i32) = (37100, 1920, 37900, 2150);
/// The whistle that loops while it blows.
pub const WHISTLE: &str = "LEVEL_REDTAIL_WINDWHISTLING";

/// The wind's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindData {}

/// True when Claw's position (`x`, `y`) lies in the wind's box.
pub fn in_box(x: i32, y: i32) -> bool {
    let (x0, y0, x1, y1) = BOX;
    (x0..x1).contains(&x) && (y0..y1).contains(&y)
}

/// Writes the wind's push on Claw: [`PUSH`] when `inside` (Claw's
/// position in [`BOX`]), else none. Claw's own movement applies it from
/// his next tick.
pub fn push_claw(w: &mut World, inside: bool) {
    w.claw.conveyor_force = if inside { PUSH } else { 0.0 };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_takes_its_left_and_top_edges() {
        assert!(in_box(37100, 1920));
        assert!(in_box(37899, 2149));
        assert!(!in_box(37900, 2000));
        assert!(!in_box(37500, 2150));
        assert!(!in_box(37099, 2000));
        // A blow lasts 278 ticks at the 18 ms step; debris every 6.
        assert_eq!(BLOW_MS.div_ceil(18), 278);
        assert_eq!(DEBRIS_MS.div_ceil(18), 6);
        // A standing Claw drifts 5 px a tick.
        assert_eq!((PUSH * 0.018).trunc(), -5.0);
    }
}
