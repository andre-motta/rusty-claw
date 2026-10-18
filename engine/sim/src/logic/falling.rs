//! What the presentation needs of a body the game removes as it falls out
//! of the view (see [`World::remove_falling_body`]): its picture, the
//! frames its animation would show next, its fall and the box its pictures
//! cover.
//!
//! Working this out reads the body and the level only: it plays a copy of
//! the body's animation, draws nothing from the random streams and changes
//! nothing in the game.
//!
//! [`World::remove_falling_body`]: super::World::remove_falling_body

use claw_frame::bodies::MAX_MS;
use claw_frame::{DrawMode, RemovedBody};

use super::enemy::motion::STEP_LIMIT;
use crate::level::SimLevel;
use crate::object::{Object, Rect, draw_flags};
use crate::tuning::TICK_MS;

/// The fall of a removed body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fall {
    /// Speed across, px/s.
    pub vx: f64,
    /// Speed up or down, px/s (down is positive).
    pub vy: f64,
    /// Acceleration down, px/s/s.
    pub gravity: f64,
    /// Largest move on each axis in one tick, px; 0 for none.
    pub step_limit: f64,
    /// How far the exact position is past the whole pixels, px.
    pub carry: (f64, f64),
}

/// The fall of `o`, from its family's data: an enemy's speed, gravity and
/// carried fraction, with the enemies' step limit. `None` for an object
/// whose family keeps no fall.
pub fn fall_of(o: &Object) -> Option<Fall> {
    let d = o.data.enemy()?;
    Some(Fall {
        vx: d.vx,
        vy: d.vy,
        gravity: d.ay,
        step_limit: STEP_LIMIT,
        carry: d.carry,
    })
}

/// The body `o`, falling as `fall`, as a presentation draws it on: the
/// frame it shows now, then the frames a copy of its animation shows in
/// the following ticks (up to [`MAX_MS`], a run of the same frame at the
/// end kept once). `None` when it is not drawn.
pub fn removed_body(level: &SimLevel, o: &Object, fall: Fall) -> Option<RemovedBody> {
    if o.image_set.is_empty() || o.draw_flags & draw_flags::HIDDEN != 0 {
        return None;
    }
    let mirror = o.draw_flags & draw_flags::MIRROR != 0;
    let range = level.image_frames(&o.image_set);
    let mut anim = o.anim.clone();
    let mut frame = o.frame;
    let mut frames = vec![frame];
    for _ in 0..MAX_MS / TICK_MS {
        if anim.animation().is_none() || anim.is_done() {
            break;
        }
        if let Some(f) = anim.advance_in(TICK_MS, frame, mirror, range).frame {
            frame = f;
        }
        frames.push(frame);
    }
    while frames.len() > 1 && frames[frames.len() - 1] == frames[frames.len() - 2] {
        frames.pop();
    }
    let mut bounds: Option<Rect> = None;
    let mut probe = o.clone();
    probe.x = 0;
    probe.y = 0;
    for &f in &frames {
        probe.frame = f;
        if let Some(b) = level.draw_box(&probe) {
            bounds = Some(match bounds {
                None => b,
                Some(u) => Rect::new(
                    u.left.min(b.left),
                    u.top.min(b.top),
                    u.right.max(b.right),
                    u.bottom.max(b.bottom),
                ),
            });
        }
    }
    Some(RemovedBody {
        id: o.serial,
        image_set_key: o.image_set.clone(),
        frames,
        x: f64::from(o.x) + fall.carry.0,
        y: f64::from(o.y) + fall.carry.1,
        vx: fall.vx,
        vy: fall.vy,
        gravity: fall.gravity,
        step_limit: fall.step_limit,
        z: o.z,
        mirror,
        invert: o.draw_flags & draw_flags::INVERT != 0,
        draw_mode: DrawMode::NORMAL,
        draw_color: None,
        bounds: bounds.map(|b| (b.left, b.top, b.right, b.bottom)),
    })
}
