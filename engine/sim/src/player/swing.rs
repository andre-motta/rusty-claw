//! Claw's Swing state (5007): hanging from a swinging rope.
//!
//! Once filled in, Claw enters it when he grabs a rope's lower end while
//! jumping or falling (see [`crate::logic::rope`]). Each tick he plays his
//! swinging animation, is placed 8 px to the side he faces and 20 px below
//! the rope's end, with no tile collision, and a jump press lets go,
//! launching an ordinary jump from there. A hit or a death leaves the
//! state as from any other.
//!
//! Not filled in yet: nothing enters the state, and a Claw found in it
//! returns to standing, exactly as the fallback rows do.

use super::{Ctx, fallback_tick};

/// One tick of the Swing state (5007).
pub fn swing_tick(c: &mut Ctx) {
    fallback_tick(c);
}
