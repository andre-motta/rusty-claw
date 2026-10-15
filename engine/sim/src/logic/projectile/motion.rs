//! The motion step shared by every projectile: floating-point position
//! and velocity integrated over the 18 ms tick, then moved through the tile
//! physics, with the rebound rules of thrown objects.

use super::ProjectileData;
use crate::logic::World;
use crate::object::{Object, ObjectId};
use crate::physics::{ptype, res};
use crate::tiles::SOLID;
use crate::tuning::TICK_MS;

/// Largest move per axis in one step, px.
pub const STEP_LIMIT: f64 = 24.0;

/// Length of one step, s.
const STEP_S: f64 = TICK_MS as f64 / 1000.0;

/// Floating-point position (absolute level pixels), velocity (px/s) and
/// vertical acceleration (px/s²).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Motion {
    /// Position.
    pub x: f64,
    /// Position.
    pub y: f64,
    /// Velocity.
    pub vx: f64,
    /// Velocity.
    pub vy: f64,
    /// Vertical acceleration.
    pub ay: f64,
}

fn sign(v: f64) -> f64 {
    if v > 0.0 { 1.0 } else { -1.0 }
}

/// One axis over `t` seconds: the displacement `(v + a t / 2) t`, at most
/// [`STEP_LIMIT`] either way; a clamped step under acceleration takes the
/// speed the arc has at the clamped distance. The speed then gains `a t`.
fn step_axis(p: &mut f64, v: &mut f64, a: f64, t: f64) {
    let mut d = (*v + 0.5 * a * t) * t;
    if !(-STEP_LIMIT..=STEP_LIMIT).contains(&d) {
        d = STEP_LIMIT * sign(d);
        if a != 0.0 {
            *v = sign(*v) * (*v * *v + 2.0 * a * d).max(0.0).sqrt();
        }
    }
    *p += d;
    *v += a * t;
}

impl Motion {
    /// Advances by `t` seconds, x first.
    pub fn step(&mut self, t: f64) {
        step_axis(&mut self.x, &mut self.vx, 0.0, t);
        step_axis(&mut self.y, &mut self.vy, self.ay, t);
    }

    /// Puts the vertical position at `y`, with the speed the arc has
    /// there.
    pub fn snap_y(&mut self, y: f64) {
        if self.ay != 0.0 {
            self.vy = sign(self.vy)
                * (self.vy * self.vy + 2.0 * self.ay * (y - self.y))
                    .max(0.0)
                    .sqrt();
        }
        self.y = y;
    }
}

/// One motion step of `id`, unless it is frozen: integrate, move toward the
/// whole-pixel position through the tile physics, take back what the
/// tiles corrected (vertical speed follows the arc to the corrected
/// height), reverse vertical speed on a ceiling, rebound off a wall at half
/// speed, and keep the result for the next tick's state rule.
pub(super) fn integrate(w: &mut World, id: ObjectId, d: &mut ProjectileData) {
    if d.frozen {
        return;
    }
    d.motion.step(STEP_S);
    let (tx, ty) = (d.motion.x as i32, d.motion.y as i32);
    let r = w.physics(id, tx, ty, d.tile_mask);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.x != tx {
        d.motion.x = f64::from(o.x);
    }
    if o.y != ty {
        d.motion.snap_y(f64::from(o.y));
    }
    if r & res::CEILING != 0 {
        d.motion.vy = -d.motion.vy;
    } else if r & (res::RIGHT | res::LEFT) != 0 {
        d.motion.vx *= -0.5;
    }
    d.last_result = r;
    o.speed_x = d.motion.vx as i32;
    o.speed_y = d.motion.vy as i32;
}

/// Bounces a landed projectile: it rises 1 px and flies again with half
/// its horizontal speed and its landing speed, corrected for that pixel,
/// reversed and halved.
pub(super) fn bounce(w: &mut World, id: ObjectId, d: &mut ProjectileData) {
    d.bounces += 1;
    if let Some(o) = w.objects.get_mut(id) {
        o.physics_type = ptype::FLY;
    }
    let m = &mut d.motion;
    let mut vy = m.vy;
    if m.ay != 0.0 {
        vy = sign(vy) * (vy * vy - 2.0 * m.ay).max(0.0).sqrt();
    }
    m.y -= 1.0;
    m.vx *= 0.5;
    m.vy = vy * -0.5;
}

/// True when the movement box of `o` touches solid tiles at its position,
/// probed at the corners and every 10 px between them.
pub(super) fn overlaps_solid(w: &World, o: &Object) -> bool {
    let r = o.move_rect;
    if !r.is_set() || r.left > r.right || r.top > r.bottom {
        return w.level.map.attr(o.x, o.y) == SOLID;
    }
    let steps = |lo: i32, hi: i32| {
        let mut v: Vec<i32> = (lo..hi).step_by(10).collect();
        v.push(hi);
        v
    };
    let xs = steps(o.x + r.left, o.x + r.right);
    let ys = steps(o.y + r.top, o.y + r.bottom);
    xs.iter()
        .any(|&x| ys.iter().any(|&y| w.level.map.attr(x, y) == SOLID))
}
