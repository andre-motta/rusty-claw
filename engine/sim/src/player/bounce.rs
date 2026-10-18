//! The springboard throw: a jump of a given height that another object
//! starts for Claw.
//!
//! A springboard does not give Claw a speed, it gives him a height: the
//! throw is an ordinary jump (state 4) whose apex is set `height` px above
//! where he stands instead of the usual base height, and the launch speed
//! is the one that reaches it under gravity (about 1423 px/s for 450 px).
//! Its hold window is empty, so holding or tapping Jump cannot raise it,
//! and the jump ends exactly at the apex in an ordinary fall. Air control,
//! ceilings and ladder grabs are the jump's own.
//!
//! The throw cancels what Claw was doing (an attack, a carried object is
//! let go) from any state but the deaths and the unused state 28; the
//! fractions of a pixel his motion carries are kept.

use std::sync::Arc;

use claw_frame::TickInput;

use super::{Ctx, start_jump_common, state};
use crate::logic::World;
use crate::object::{LogicKind, Object, ObjectId};
use crate::tuning::TICK_MS;

/// A state with no behaviour that a throw leaves alone, as it does the
/// deaths.
pub const UNUSED_STATE: i32 = 28;

/// True when a throw may start from Claw's state `s`.
pub fn allowed(s: i32) -> bool {
    !matches!(s, UNUSED_STATE | state::DEATH | state::DEATH_FALL)
}

/// Throws Claw into a jump whose apex is `height` px above him; false
/// when there is no Claw or his state refuses it (see [`allowed`]).
///
/// It runs inside the calling object's logic call: Claw's own update later
/// in the same tick is his first rising step.
pub fn bounce(w: &mut World, height: i32) -> bool {
    let Some(id) = w.claw_id else {
        return false;
    };
    if !w.objects.get(id).is_some_and(|o| allowed(o.state)) {
        return false;
    }
    with_ctx(w, id, |c| throw(c, height)).is_some()
}

/// The throw on Claw's own context.
pub fn throw(c: &mut Ctx, height: i32) {
    c.reset_actions();
    start_jump_common(c);
    c.p.jump_press_time = 0.0;
    c.p.jump_peak_y = c.o.y - height;
    let (px, peak) = (c.p.mover.x.p, f64::from(c.p.jump_peak_y));
    c.p.mover.aim_at(px, peak);
}

/// Runs `f` on Claw's context, built from a level logic's world: Claw's
/// object is held out of its slot for the call, as during his own update.
/// The input is empty and the camera request is dropped: a throw reads
/// neither, and neither does a bear's hug.
pub(super) fn with_ctx<R>(w: &mut World, id: ObjectId, f: impl FnOnce(&mut Ctx) -> R) -> Option<R> {
    let platforms = crate::sim::platforms(w.objects);
    let mut held = Object::new(0, 0, 0, 0, Arc::from(""), LogicKind::Inert);
    if !w.objects.swap_payload(id, &mut held) {
        return None;
    }
    let v = w.view;
    let mut camera = None;
    let r = {
        let mut c = Ctx {
            o: &mut held,
            p: &mut *w.claw,
            id,
            objects: &mut *w.objects,
            level: w.level,
            platforms: &platforms,
            input: TickInput::default(),
            t: w.tuning,
            rng: &mut *w.rng,
            sound_rng: &mut *w.sound_rng,
            camera_override: &mut camera,
            plane_centre: ((v.left + v.right) / 2, (v.top + v.bottom) / 2),
            out: &mut *w.out,
            tick: w.tick,
            now_ms: w.now_ms,
            dt_ms: TICK_MS,
            pulse: w.pulse,
            view: v,
            level_logics: true,
            aids: w.aids,
        };
        f(&mut c)
    };
    w.objects.swap_payload(id, &mut held);
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_deaths_and_the_unused_state_refuse_a_throw() {
        for s in [UNUSED_STATE, state::DEATH, state::DEATH_FALL] {
            assert!(!allowed(s), "{s}");
        }
        for s in [
            state::STAND,
            state::MOVE_RIGHT,
            state::MOVE_LEFT,
            state::CROUCH,
            state::LOOK_UP,
            state::LAND,
            state::LIFT,
            state::HIT,
            state::FALL,
        ] {
            assert!(allowed(s), "{s}");
        }
    }
}
