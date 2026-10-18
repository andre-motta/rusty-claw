//! Conveyor belts: strips of moving track laid on the ninth level's
//! floors, each belt a left end, a row of 64 px middle pieces and a right
//! end, every piece its own object.
//!
//! A belt adds no floor and takes part in no collision: the floor is the
//! level's own ground row. It adds a push. While Claw's box overlaps a
//! piece's box, the piece writes its speed (the record's speed field,
//! clamped to [`SPEED_LIMIT`]) into Claw's conveyor force, which his own
//! movement adds to his speed when he stands, ducks, looks up, walks or
//! rises in a jump; the fall ignores it. Nothing else is ever moved.
//!
//! - A piece pushes from the second tick after Claw's update first ends in
//!   its box: the first tick it only notices him.
//! - While Claw rises in a jump the pieces leave the push as it was, so a
//!   jump from a belt keeps the belt's speed until its top.
//! - Each piece writes the force on its own, in update order, and the last
//!   write wins; a piece that loses Claw writes 0, so crossing from one
//!   piece into one that updates earlier leaves him one tick without push.
//! - A warp clears the push.
//!
//! The piece's box is the box of its picture, except that the right end
//! keeps only the left half of its picture ([`RIGHT_END`]); Claw's box for
//! the test runs from his position down to his feet. The pieces cycle
//! their pictures at a pace set by their speed ([`cycle_anim`]). A belt
//! plays no sound (sound objects beside the belts hum) and draws no random
//! number.
//!
//! Not built yet ([`BUILT`] is false): the pieces stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! A piece is no enemy: its states are its own ([`state`]).
//!
//! # Defaults taken
//!
//! - The ninth level's 479 pieces need no special handling for speed: at
//!   most 33 are live at once, each doing one animation step and one box
//!   test per tick. The tick cost and the cost of copying the game along
//!   the level's bot route are measured when the slice closes and
//!   compared with the third and eighth levels'.
//!
//! Its data ([`BeltData`]) is its own.

use super::World;
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["ConveyorBelt"];

/// Image sets this family may switch an object to: none (the pieces keep
/// the sets their records name).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: the picture cycles,
/// forward and backward.
pub const ANIM_PREFIXES: &[&str] = &["GAME_CYCLE", "GAME_REVERSECYCLE"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The right end's picture, whose box keeps only its left half.
pub const RIGHT_END: &str = "LEVEL_CONVEYRIGHT";
/// The pieces' pictures on the ninth level: left end, middle, right end.
pub const PIECES: [&str; 3] = ["LEVEL_CONVEYLEFT", "LEVEL_CONVEYMIDDLE", RIGHT_END];
/// The fastest push, px/s, either way; record speeds beyond it are
/// clamped.
pub const SPEED_LIMIT: i32 = 300;
/// Record field holding the speed, px/s.
pub const SPEED_FIELD: usize = 60;
/// Depth of a piece placed at depth 0.
pub const DEFAULT_Z: i32 = 1;
/// What a piece is, for collisions.
pub const TYPE: u32 = 1;
/// What may hit a piece (nothing reaches it: it has no hit box).
pub const HIT_MASK: u32 = 0x300_0072;
/// Ticks from Claw's update first ending in a piece's box to the first
/// tick the piece pushes him.
pub const PUSH_DELAY_TICKS: u32 = 2;

/// A piece's states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Claw is not on it.
    pub const IDLE: i32 = 1;
    /// Claw is on it: it pushes him.
    pub const MOVE: i32 = 2;
}

/// The record speed `speed` clamped to the belts' limit.
pub fn clamp_speed(speed: i32) -> i32 {
    speed.clamp(-SPEED_LIMIT, SPEED_LIMIT)
}

/// The picture cycle of a piece of clamped speed `speed`: backward cycles
/// for a speed below 0, forward ones from 0 up, faster the faster the
/// belt.
pub fn cycle_anim(speed: i32) -> &'static str {
    let s = speed.unsigned_abs();
    match (speed < 0, s) {
        (true, 300..) => "GAME_CYCLE50",
        (true, 150..) => "GAME_CYCLE100",
        (true, 75..) => "GAME_CYCLE200",
        (true, _) => "GAME_CYCLE500",
        (false, 300..) => "GAME_REVERSECYCLE50",
        (false, 150..) => "GAME_REVERSECYCLE100",
        (false, 75..) => "GAME_REVERSECYCLE200",
        (false, _) => "GAME_REVERSECYCLE500",
    }
}

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BeltData {
    /// The clamped speed, px/s.
    pub speed: i32,
}

/// How a piece whose first call has not run yet looks once it has (see
/// [`super::first_look`]): `None`, as it looks now.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = (level, o);
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits a piece.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A piece attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_are_clamped_both_ways() {
        assert_eq!(clamp_speed(500), 300);
        assert_eq!(clamp_speed(-500), -300);
        assert_eq!(clamp_speed(175), 175);
        assert_eq!(clamp_speed(-150), -150);
    }

    #[test]
    fn the_cycle_follows_the_speed() {
        // The ninth level's speeds, clamped.
        for (speed, anim) in [
            (-300, "GAME_CYCLE50"),
            (-250, "GAME_CYCLE100"),
            (-200, "GAME_CYCLE100"),
            (-150, "GAME_CYCLE100"),
            (-100, "GAME_CYCLE200"),
            (75, "GAME_REVERSECYCLE200"),
            (100, "GAME_REVERSECYCLE200"),
            (150, "GAME_REVERSECYCLE100"),
            (175, "GAME_REVERSECYCLE100"),
            (300, "GAME_REVERSECYCLE50"),
        ] {
            assert_eq!(cycle_anim(speed), anim, "{speed}");
        }
        assert_eq!(cycle_anim(0), "GAME_REVERSECYCLE500");
        assert_eq!(cycle_anim(-74), "GAME_CYCLE500");
        for anim in [cycle_anim(-300), cycle_anim(300)] {
            assert!(ANIM_PREFIXES.iter().any(|p| anim.starts_with(p)));
        }
    }
}
