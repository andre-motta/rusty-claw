//! Claw's Bearhug state (5008): held in a bear sailor's hug (see
//! [`crate::logic::enemy::bear_sailor`]).
//!
//! Once filled in, the bear puts Claw in the state when it grabs him:
//! whatever he was doing ends (an attack is cut short, a carried object is
//! dropped, and Left and Right count as pressed again when still held),
//! he stops dead and loses his own movement, picture changes and input
//! handling for as long as he is held; only the shared bookkeeping after
//! every state runs (timers, the weapon toggle). The bear places him
//! [`crate::logic::enemy::bear_sailor::HUG_X`] px in front of itself, at
//! its own height, sets the picture he shows
//! ([`crate::logic::enemy::bear_sailor::held_frame`]), counts his key
//! presses and squeezes his health; when it lets go, Claw stands where he
//! was held and drops the 1 px to the floor. A hit from anything else
//! leaves the state as from any other, which ends the hug.
//!
//! Not filled in yet ([`BUILT`] is false): nothing enters the state, and a
//! Claw found in it returns to standing, exactly as the fallback rows do.

use super::{Ctx, fallback_tick};

/// Whether the hug is filled in yet: while false, the bear never grabs
/// Claw and the state's row falls back to standing.
pub const BUILT: bool = false;

/// Claw's pictures while held, one per squeeze picture of the bear.
pub const HELD_FRAMES: [i32; 3] = [450, 451, 452];

/// One tick of the Bearhug state (5008).
pub fn hug_tick(c: &mut Ctx) {
    fallback_tick(c);
}
