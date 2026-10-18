//! Dart traps: hidden blowpipes in the ninth level's walls (and in later
//! levels') that watch a rectangle of the level and shoot an arrow
//! ([`arrow`]) while Claw stands in it.
//!
//! A trap draws nothing while it waits: the hole Claw sees is a separate
//! piece of decoration. When Claw's hit box overlaps its trigger
//! rectangle (the record's bounds), it plays its puff (the record's own
//! animation, pictures 1 to 5, 100 ms each, with [`SOUND`] on the fourth),
//! and on the call that ends the puff it shoots one arrow from its own
//! position in the record's direction. It then waits the record's speed
//! field in ms ([`DEFAULT_WAIT_MS`] when 0) before it can trigger again,
//! so with Claw inside it fires every `31 + ceil(wait / 18)` ticks (the
//! puff aimed up is 6 ticks shorter). It never moves, nothing can hit it
//! and it draws no random number.
//!
//! Not built yet ([`BUILT`] is false): dart traps stay inert placeholders
//! (drawn, no collision, reported as not implemented), and the calls below
//! do nothing.
//!
//! # State numbers
//!
//! A trap is no enemy: its states are its own ([`state`]), held in its
//! object's state as for the other level objects.
//!
//! # Defaults taken
//!
//! - The trap's states are its own, not enemy states, so the ninth level's
//!   enemy block keeps no number for its wait.
//! - The record's first-shot delay (its smarts field, in ms, applied when
//!   Claw enters the rectangle afresh) and the animated shots of the
//!   fourteenth level's launchers (its user value 1 naming the picture set
//!   `LEVEL_PROJECTILES_<n>`) are built with the trap, though no ninth
//!   level record uses them; the levels that do confirm them.
//!
//! Its data ([`DartTrapData`]) is its own.

use super::World;
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId, Rect};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TProjectile"];

/// Image sets this family may switch an object to: the arrows.
pub const IMAGE_SETS: &[&str] = &[arrow::IMAGE_SET];

/// Prefixes of the animations this family may play: the puffs, which the
/// records name too.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SHOOTERS_PUFFDART"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The puff's sound (a frame sound of the puff).
pub const SOUND: &str = "LEVEL_BLOWDART";
/// Depth of a trap placed at depth 0.
pub const DEFAULT_Z: i32 = 4010;
/// The wait after a shot when the record gives none, ms.
pub const DEFAULT_WAIT_MS: u32 = 500;
/// Speed of the arrows on an axis the record leaves at 0, px/s.
pub const DEFAULT_SPEED: i32 = 100;
/// Direction of a trap whose record gives none.
pub const DEFAULT_DIRECTION: i32 = 4;
/// Record field holding the wait, ms.
pub const WAIT_FIELD: usize = 60;
/// Record field holding the direction.
pub const DIRECTION_FIELD: usize = 63;
/// Ticks of the puff aimed left, right or down at the 18 ms step: the
/// arrow leaves on the last (measured).
pub const PUFF_TICKS: u32 = 31;
/// Ticks of the puff aimed up at the 18 ms step.
pub const PUFF_UP_TICKS: u32 = 25;
/// The puff's sound tick, from the trigger (measured).
pub const SOUND_TICK: u32 = 19;

/// The trap's states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting for Claw, drawing nothing.
    pub const IDLE: i32 = 1;
    /// Puffing; the arrow leaves when the puff ends.
    pub const FIRING: i32 = 2;
    /// Waiting before it can trigger again, drawing nothing.
    pub const WAIT: i32 = 3;
}

/// Ticks between two arrows while Claw stays in the rectangle, at the
/// 18 ms step, for a wait of `wait_ms` and a puff aimed up (`up`) or not.
pub fn period_ticks(wait_ms: u32, up: bool) -> u32 {
    let puff = if up { PUFF_UP_TICKS } else { PUFF_TICKS };
    puff + wait_ms.div_ceil(18)
}

/// The arrow a dart trap shoots: the soldier's bullet with an arrow
/// picture, an enemy's shot that flies straight in the trap's direction
/// and costs Claw [`DAMAGE`](arrow::DAMAGE); the record's user value picks
/// the picture. It is not a family of its own: the projectile family runs
/// it.
pub mod arrow {
    use crate::object::Rect;

    /// Logic name of the arrow.
    pub const LOGIC: &str = "Bullet";
    /// Its pictures.
    pub const IMAGE_SET: &str = "LEVEL_PROJECTILES";
    /// Damage to Claw, whatever the record says.
    pub const DAMAGE: i32 = 5;

    /// The velocity of an arrow shot in `direction` (2 down, 4 left, 6
    /// right, 8 up) by a trap with speeds `speed`, px/s; `None` for any
    /// other direction, whose arrow does not move.
    pub fn velocity(direction: i32, (speed_x, speed_y): (i32, i32)) -> Option<(i32, i32)> {
        match direction {
            2 => Some((0, speed_y)),
            4 => Some((-speed_x, 0)),
            6 => Some((speed_x, 0)),
            8 => Some((0, -speed_y)),
            _ => None,
        }
    }

    /// The box of the arrow picture `frame` (1 right, 2 left, 3 down, 4
    /// up), the box it hits with.
    pub fn frame_box(frame: i32) -> Option<Rect> {
        match frame {
            1 => Some(Rect::new(-1, -3, 58, 2)),
            2 => Some(Rect::new(-58, -3, 1, 2)),
            3 => Some(Rect::new(-4, 0, 3, 55)),
            4 => Some(Rect::new(-4, -57, 3, -2)),
            _ => None,
        }
    }
}

/// A dart trap's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DartTrapData {
    /// The trigger rectangle, relative to the trap, once set up.
    pub trigger: Option<Rect>,
}

/// How a dart trap whose first call has not run yet looks once it has
/// (see [`super::first_look`]): `None`, as it looks now.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = (level, o);
    None
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits a trap.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: Claw stepped into the trigger rectangle.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trap_fires_every_puff_and_wait() {
        // Measured: 87 ticks with a 1000 ms wait.
        assert_eq!(period_ticks(1000, false), 87);
        assert_eq!(period_ticks(500, false), 59);
        assert_eq!(period_ticks(500, true), 53);
        assert_eq!(PUFF_TICKS - PUFF_UP_TICKS, 6);
        assert_eq!(period_ticks(DEFAULT_WAIT_MS, false), 31 + 28);
    }

    #[test]
    fn arrows_fly_the_traps_way() {
        let speeds = (500, 1000);
        assert_eq!(arrow::velocity(2, speeds), Some((0, 1000)));
        assert_eq!(arrow::velocity(4, speeds), Some((-500, 0)));
        assert_eq!(arrow::velocity(6, speeds), Some((500, 0)));
        assert_eq!(arrow::velocity(8, speeds), Some((0, -1000)));
        assert_eq!(arrow::velocity(5, speeds), None);
        // Each picture points the way it flies and reaches 55 to 58 px
        // ahead of the trap.
        for (frame, dir) in [(1, 6), (2, 4), (3, 2), (4, 8)] {
            let b = arrow::frame_box(frame).expect("an arrow");
            let (vx, vy) = arrow::velocity(dir, (1, 1)).unwrap();
            let reach = match (vx, vy) {
                (1, 0) => b.right,
                (-1, 0) => -b.left,
                (0, 1) => b.bottom,
                _ => -b.top,
            };
            assert!((55..=58).contains(&reach), "{frame}: {reach}");
        }
        assert_eq!(arrow::frame_box(0), None);
    }

    #[test]
    fn its_states_are_its_own() {
        let states = [state::SET_UP, state::IDLE, state::FIRING, state::WAIT];
        for (i, s) in states.iter().enumerate() {
            assert!(!states[i + 1..].contains(s));
            assert_eq!(crate::logic::enemy::EnemyState::from_id(*s), None);
        }
    }
}
