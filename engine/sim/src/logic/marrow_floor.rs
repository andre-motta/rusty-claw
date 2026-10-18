//! Marrow's trap floor on the tenth level: the two sliding slabs that can
//! bridge the spike pit of Marrow's room ([`crate::logic::enemy::marrow`]).
//!
//! Each slab is a riding platform with the level's big trap-elevator
//! surface ([`crate::logic::platform::SurfaceKind::BigElevator`]) that
//! slides along x only, between the two ends of its record's x range.
//! "Open" means the pit is open: the slab has slid under its ledge, out
//! of reach, hidden behind a piece of decoration. Its record's position
//! is the closed end, the bridge. A slab knows nothing of the fight; it
//! only reads the Marrow link ([`World::marrow_link`]):
//!
//! - idle, with no Marrow it does nothing; after a death of Claw's (his
//!   attempt counter changed since it last looked) it snaps back to its
//!   record's x, closed, writes the floors-open status clear and the phase
//!   health from Marrow's health, and goes on in the same tick; then with
//!   the open request set and the slab closed it starts opening, with the
//!   request clear and the slab open it starts closing ([`SLIDE_SOUND`] as
//!   either move starts);
//! - moving, it steps [`step`] px a tick toward its end (toward the low
//!   end when its record's direction is 0 or more and it is opening, the
//!   other way when closing), and a rider moves with it; at the end it is
//!   open (status set) or closed (status clear, phase health from Marrow's
//!   health) and idle again. A move, once started, always runs to its end.
//!
//! Marrow sets the request when he is built and when Claw respawns, so the
//! pit opens with the room and after every death; the parrot clears it
//! when it settles on its perch (the bridge closes) and sets it when it
//! lifts Marrow (the bridge opens under anyone on it). The slabs never
//! read the boss fight flag: they open after an intro skip too.
//!
//! Not built yet ([`BUILT`] is false): the slabs stay inert placeholders
//! (drawn closed where the level puts them, no collision, reported as not
//! implemented), and the calls below do nothing.
//!
//! # Measured on the original, at the 18 ms step
//!
//! - The room is built inside the passes of the door's warp: on Claw's
//!   first tick in the room the slabs are already opening at their record
//!   x and take their first step on the next tick; the right slab is open
//!   [`OPEN_TICKS`]`.1` ticks after that first tick in the room, the left
//!   one [`OPEN_TICKS`]`.0`.
//! - Marrow and the parrot update after the slabs, so a request they
//!   change in tick t starts the slabs moving on t + 1, and a closing ends
//!   on t + 33 (right) and t + 37 (left).
//! - After a respawn the snapped slab already moves in the respawn tick,
//!   so the opening ends on R + 31 and R + 35.
//! - Claw riding the right slab as it opens is stopped by the ledge wall
//!   at x [`RIGHT_RIDER_STOP_X`] and falls on the slab's 31st step.
//!
//! # Defaults taken
//!
//! - Its states are its own numbers ([`state`]), kept in the object's
//!   state as the dart traps keep theirs: the slabs are no enemies.
//! - Two slabs starting a move together each ask for the slide sound in
//!   the same tick through the plain sound path; the mixer's replay gap is
//!   0 by default, so both are heard.
//! - Dormant before the room wakes, a slab looks as the level places it,
//!   closed at its record's x ([`first_look`]).

use super::{Look, World};
use crate::level::SimLevel;
use crate::object::{Object, ObjectId};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["MarrowFloor"];

/// The slab's picture, set when it is built (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_TRAPELEVATOR";

/// Image sets this family may switch an object to: its own.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// Played as a slab starts to open or to close.
pub const SLIDE_SOUND: &str = "LEVEL_PEGSLIDE";

/// The longest step of a slab, px a tick.
pub const MAX_STEP: i32 = 16;

/// Ticks from Claw's first tick in the room to the end of the opening of
/// the left and the right slab, at the 18 ms step (measured).
pub const OPEN_TICKS: (u32, u32) = (36, 32);

/// Where the ledge wall stops Claw riding the right slab as it opens
/// (measured).
pub const RIGHT_RIDER_STOP_X: i32 = 40959;

/// A slab's states, kept in the object's state.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Still, polling the Marrow link.
    pub const IDLE: i32 = 1;
    /// Sliding open (toward its open end).
    pub const OPENING: i32 = 2;
    /// Sliding closed (back toward its record's x).
    pub const CLOSING: i32 = 3;
}

/// A slab's step for a speed of `speed_x` px/s over a call of `ms`
/// milliseconds: the speed times the time, whole pixels, truncated, at
/// most [`MAX_STEP`].
pub fn step(speed_x: i32, ms: u32) -> i32 {
    let d = (i64::from(speed_x.max(0)) * i64::from(ms) / 1000) as i32;
    d.min(MAX_STEP)
}

/// A slab's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloorData {
    /// The closed end: its record's x.
    pub closed_x: i32,
    /// Claw's attempt counter when it last looked.
    pub attempt: u32,
    /// The pit is open on this slab's side.
    pub open: bool,
    /// Its move in the current tick, px across (what a rider follows).
    pub moved: i32,
}

/// How a slab looks before its first call (D44, see
/// [`super::first_look`]): `None`, as the level places it, closed.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = (level, o);
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits a slab.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A slab attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slab_steps_five_px_a_tick_at_the_eighteen_millisecond_step() {
        assert_eq!(step(300, 18), 5);
        // At 60 frames per second: 5, 5 and 4 px.
        assert_eq!([step(300, 17), step(300, 17), step(300, 16)], [5, 5, 4]);
        // Long frames are held to 16 px.
        assert_eq!(step(300, 53), 15);
        assert_eq!(step(300, 60), MAX_STEP);
        assert_eq!(step(0, 18), 0);
    }

    #[test]
    fn the_tenth_levels_slabs_open_in_whole_steps() {
        // The left slab travels 180 px, the right 160: 36 and 32 steps.
        assert_eq!(180 / step(300, 18), OPEN_TICKS.0 as i32);
        assert_eq!(160 / step(300, 18), OPEN_TICKS.1 as i32);
    }

    #[test]
    fn its_states_are_its_own() {
        let all = [state::SET_UP, state::IDLE, state::OPENING, state::CLOSING];
        for s in all {
            assert!(crate::logic::enemy::EnemyState::from_id(s).is_none());
        }
        assert!(SLIDE_SOUND.starts_with("LEVEL_"));
    }
}
