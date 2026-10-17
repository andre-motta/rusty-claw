//! Claw's Swing state (5007): hanging from a swinging rope.
//!
//! # The grab
//!
//! A rope is a special pickup with the rope code, so Claw's hit handler
//! sees it whenever his body box touches the rope's grab box (see
//! [`crate::logic::rope`]). He grabs it ([`grab`]) unless:
//!
//! - it is the rope he let go of last and his regrab time
//!   ([`Claw::rope_regrab_ms`](super::Claw::rope_regrab_ms), 500 ms after
//!   a release) is still running; any other rope can be grabbed at once;
//! - he is neither jumping nor falling (so never while climbing, standing
//!   or already swinging); a hit reaction that left him falling counts as
//!   falling;
//! - his position is 48 px or more above or below the point 16 px under
//!   the rope's end.
//!
//! Grabbing ends his attack (dropping what he carries, and Left and Right
//! count as pressed again when still held), enters Swing with the swinging
//! animation and the flying physics type, remembers the rope, ends the
//! look-up lockout and places him 8 px to the side he faces of the rope's
//! end and 20 px below it. No sound plays.
//!
//! # Swing
//!
//! Each tick he plays the swinging animation and is placed again 8 px to
//! the side he faces and 20 px below the rope's end, with no tile
//! collision and no attack: the rope runs before him and turns him to
//! face the way its end travels. A jump press lets go: the regrab time
//! starts (500 ms), and an ordinary jump starts from where he hangs
//! ([`super::begin_jump`]), taking no momentum from the rope; the keys held
//! afterwards steer it, and its height is a walking jump's unless his
//! running time ran out before the rope turned him. A hit or a death
//! leaves the state as from any other; a hit leaves him falling, so the
//! rope can catch him again at once.
//!
//! The row stays a flagged fallback row: a Claw put in the state with no
//! rope to hang from returns to standing, as the fallback rows do.

use super::{Ctx, begin_jump, fallback_tick, state};
use crate::logic::{World, rope};
use crate::object::{ObjectId, draw_flags};
use crate::physics::ptype;
use claw_frame::Buttons;

/// Animation Claw swings with.
pub const SWING_ANIM: &str = "CLAW_SWING";
/// How long the rope Claw let go of cannot catch him again, ms.
pub const REGRAB_MS: i32 = 500;
/// How far to the side he faces Claw hangs from the rope's end, px.
pub const HANG_X: i32 = 8;
/// How far below the rope's end Claw hangs, px.
pub const HANG_Y: i32 = 20;
/// Claw's position must be within this many px (exclusive) above or below
/// the point [`GRAB_DROP`] px under the rope's end.
pub const GRAB_REACH: i32 = 48;
/// See [`GRAB_REACH`].
pub const GRAB_DROP: i32 = 16;

/// Where Claw hangs from a rope whose end is at `end`, facing right or
/// not.
pub fn hang_point(end: (i32, i32), facing_right: bool) -> (i32, i32) {
    let dx = if facing_right { HANG_X } else { -HANG_X };
    (end.0 + dx, end.1 + HANG_Y)
}

/// Claw touched the rope `id`: he grabs it when he may (see the module
/// documentation).
pub fn grab(c: &mut Ctx, id: ObjectId) {
    if c.p.rope_regrab_ms != 0 && c.p.rope == Some(id) {
        return;
    }
    if !matches!(c.o.physics_type, ptype::JUMP | ptype::FALL) {
        return;
    }
    let Some(end) = c.objects.get(id).and_then(rope::end_of) else {
        return;
    };
    if (c.o.y - end.1 - GRAB_DROP).abs() >= GRAB_REACH {
        return;
    }
    super::attack::cancel(c);
    c.o.state = state::SWING;
    c.set_anim(SWING_ANIM, false);
    c.o.physics_type = ptype::FLY;
    c.p.rope = Some(id);
    c.p.post_climb_lockout_ms = 0;
    let (x, y) = hang_point(end, c.p.facing_right);
    c.o.x = x;
    c.o.y = y;
    c.p.mover.zero_carries();
}

/// One tick of the Swing state (5007).
pub fn swing_tick(c: &mut Ctx) {
    let Some(end) =
        c.p.rope
            .and_then(|id| c.objects.get(id))
            .and_then(rope::end_of)
    else {
        // No rope to hang from: back to standing, as the fallback rows do.
        fallback_tick(c);
        return;
    };
    c.advance_anim();
    let (x, y) = hang_point(end, c.p.facing_right);
    c.o.x = x;
    c.o.y = y;
    if !c.pressed(Buttons::JUMP) {
        return;
    }
    c.p.rope_regrab_ms = REGRAB_MS;
    begin_jump(c);
}

/// Turns Claw to face right or left, as the rope he hangs from does at
/// each end of its swing: his picture and boxes are mirrored and his
/// running time starts again, as with every turn. Nothing changes when he
/// already faces that way.
pub fn face(w: &mut World, right: bool) {
    if w.claw.facing_right == right {
        return;
    }
    let Some(o) = w.claw_id.and_then(|id| w.objects.get_mut(id)) else {
        return;
    };
    if right {
        o.draw_flags &= !draw_flags::MIRROR;
    } else {
        o.draw_flags |= draw_flags::MIRROR;
    }
    o.mirror_rects();
    w.claw.facing_right = right;
    w.claw.running_time = w.tuning.player.run_onset_ms;
}
