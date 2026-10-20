//! The lava hazards of the fourteenth level: the lava fountains
//! ([`geyser`]), the stone mouths that drip lava from the ceilings
//! ([`mouth`]), and the hands that rise out of the lava ([`hand`]) and
//! throw lava balls ([`ball`]).
//!
//! The fountains and the mouths are clocks, not traps: they ignore Claw,
//! never stop, and start their cycle again after their own delay each
//! time Claw loses a life, so their phase after a respawn is fixed. A
//! hand reacts: Claw entering the box drawn around it makes it throw.
//! Touching a fountain's column, a mouth's drop or a lava ball costs Claw
//! 10 health; a lava ball's blast hurts the enemies too.
//!
//! None of them draws a random number; a lava ball's sparks do (two draws
//! each, two sparks a tick while it flies and rests).

pub mod ball;
pub mod geyser;
pub mod hand;
pub mod mouth;

use crate::logic::World;
use crate::object::{ObjectId, Rect};

/// Record field of a fountain's or a mouth's start delay, and of a hand's
/// rest, ms.
pub const SPEED_FIELD: usize = 60;
/// Record field of a hand's burst (balls per trigger).
pub const SMARTS_FIELD: usize = 17;
/// Record fields of a hand's trigger box: left, top, right, bottom.
pub const BOUNDS_FIELDS: [usize; 4] = [51, 52, 53, 54];

/// What touching a fountain's column, a mouth's drop or a lava ball
/// costs Claw (no difficulty factor).
pub const DAMAGE: i32 = 10;

/// The kind (type flags) of the hazards' hit box: nothing hits them.
pub const GENERIC: u32 = 0x1;

/// A record field of `id`, 0 without a record.
pub(crate) fn field(w: &World, id: ObjectId, i: usize) -> i32 {
    w.objects
        .get(id)
        .and_then(|o| o.record.as_deref())
        .map_or(0, |r| r[i])
}

/// Row `n` (1 based) of a box table, or no box for a number outside it
/// or a row that is all unset.
pub(crate) fn row(table: &[Option<Rect>], n: i32) -> Option<Rect> {
    usize::try_from(n - 1)
        .ok()
        .and_then(|i| table.get(i).copied().flatten())
}
