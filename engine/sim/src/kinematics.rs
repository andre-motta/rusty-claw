//! Floating-point motion with whole-pixel output.
//!
//! Claw's velocity, gravity and sub-pixel position live here as `f64`.
//! Each tick a handler sets the velocity, calls [`Mover::step`] with the
//! tick length, and takes the whole pixels of the step with
//! [`Mover::take_x`] and [`Mover::take_y`]; the fraction is carried to the
//! next tick. The integer position is then moved through the tile physics.
//! Integrating first and colliding second keeps the physics working on the
//! same requested moves whatever the tick length.

/// One axis of motion.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Axis {
    /// Acceleration, px/s^2.
    pub a: f64,
    /// Velocity, px/s.
    pub v: f64,
    /// Position, px.
    pub p: f64,
    /// Displacement of the last step, px.
    pub d: f64,
    /// Fraction carried to the next whole-pixel take.
    pub carry: f64,
    /// Largest displacement of one step, px.
    pub limit: f64,
    /// Upper velocity cap, px/s (negative velocities are not capped).
    pub cap: f64,
}

const BOUND_LO: f64 = -2_147_483_647.0;
const BOUND_HI: f64 = 2_147_483_646.0;

fn sign(v: f64) -> f64 {
    if v > 0.0 { 1.0 } else { -1.0 }
}

impl Axis {
    fn unlimited() -> Axis {
        Axis {
            limit: BOUND_HI,
            cap: BOUND_HI,
            ..Axis::default()
        }
    }

    fn step(&mut self, t: f64) {
        let mut d = (self.v + 0.5 * self.a * t) * t;
        if d > self.limit || d < -self.limit {
            let l = if d > 0.0 { self.limit } else { -self.limit };
            d = l;
            if self.a != 0.0 {
                self.v = sign(self.v) * (self.v * self.v + 2.0 * self.a * l).max(0.0).sqrt();
            }
        }
        let p_old = self.p;
        self.p += d;
        let mut inside = true;
        for bound in [BOUND_HI, BOUND_LO] {
            let out = if bound > 0.0 {
                self.p > bound
            } else {
                self.p < bound
            };
            if out {
                self.v = sign(self.v)
                    * (self.v * self.v + 2.0 * self.a * (bound - self.p))
                        .max(0.0)
                        .sqrt();
                self.p = bound;
                d = bound - p_old;
                inside = false;
            }
        }
        if inside {
            self.v += self.a * t;
        }
        if self.v > self.cap {
            self.v = self.cap;
        }
        self.d = d;
    }

    /// Whole pixels of the last step; the rest is carried.
    fn take(&mut self, double_count: bool) -> i32 {
        let d = self.d;
        if double_count {
            // The fraction of this step enters both the carry and the take.
            let r = (d - d.trunc()) + self.carry;
            let moved = (d + r).trunc();
            self.carry = r - r.trunc();
            moved as i32
        } else {
            let r = d + self.carry;
            let moved = r.trunc();
            self.carry = r - moved;
            moved as i32
        }
    }
}

/// Claw's motion state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mover {
    /// Horizontal axis.
    pub x: Axis,
    /// Vertical axis.
    pub y: Axis,
    /// Count the fractional part twice in the whole-pixel take (see
    /// [`crate::tuning::PlayerTuning::double_count_fraction`]).
    pub double_count: bool,
}

impl Default for Mover {
    fn default() -> Self {
        Mover {
            x: Axis::unlimited(),
            y: Axis::unlimited(),
            double_count: true,
        }
    }
}

impl Mover {
    /// Resets velocity, position and acceleration: velocity zero, gravity
    /// `ay`, the given step limit and velocity cap on both axes.
    pub fn init(&mut self, ay: f64, limit: f64, cap: f64) {
        for (axis, a) in [(&mut self.x, 0.0), (&mut self.y, ay)] {
            axis.a = a;
            axis.v = 0.0;
            axis.p = 0.0;
            axis.d = 0.0;
            axis.carry = 0.0;
            axis.limit = limit;
            axis.cap = cap;
        }
    }

    /// Advances by `t` seconds, x first.
    pub fn step(&mut self, t: f64) {
        self.x.step(t);
        self.y.step(t);
    }

    /// Whole horizontal pixels of the last step.
    pub fn take_x(&mut self) -> i32 {
        self.x.take(self.double_count)
    }

    /// Whole vertical pixels of the last step.
    pub fn take_y(&mut self) -> i32 {
        self.y.take(self.double_count)
    }

    /// Clears both carries.
    pub fn zero_carries(&mut self) {
        self.x.carry = 0.0;
        self.y.carry = 0.0;
    }

    /// Replaces the velocity with the slowest launch from the current
    /// position that reaches (tx, ty) under the current acceleration; with
    /// gravity and a target straight above, the arc's apex is the target.
    pub fn aim_at(&mut self, tx: f64, ty: f64) {
        let (dx, dy) = (tx - self.x.p, ty - self.y.p);
        let (ax, ay) = (self.x.a, self.y.a);
        let b = -(ax * dx + ay * dy);
        let amag = (ax * ax + ay * ay).sqrt();
        let dmag = (dx * dx + dy * dy).sqrt();
        let s = (b + amag * dmag).max(0.0).sqrt();
        if s < 0.1 {
            self.x.v = 0.0;
            self.y.v = 0.0;
            return;
        }
        let a2 = ax * ax + ay * ay;
        if (-0.1..=0.1).contains(&(a2 / 4.0)) {
            let n = dx.abs() + dy.abs();
            self.x.v = dx / n * s;
            self.y.v = dy / n * s;
            return;
        }
        let t2 = (s * s - b) / (a2 / 2.0);
        let t = t2.sqrt();
        self.x.v = (dx - 0.5 * ax * t2) / t;
        self.y.v = (dy - 0.5 * ay * t2) / t;
    }

    /// The vertical velocity the current arc has at height `y`.
    pub fn velocity_at_y(&self, y: f64) -> f64 {
        if self.y.a == 0.0 {
            return self.y.v;
        }
        sign(self.y.v)
            * (self.y.v * self.y.v + 2.0 * self.y.a * (y - self.y.p))
                .max(0.0)
                .sqrt()
    }

    /// Puts the vertical position at `y`, with the arc's velocity there,
    /// shortening the last displacement to match.
    pub fn snap_to_y(&mut self, y: f64) {
        self.y.v = self.velocity_at_y(y);
        self.y.d -= self.y.p - y;
        self.y.p = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_carry_conserves_distance() {
        let mut m = Mover {
            double_count: false,
            ..Mover::default()
        };
        m.init(0.0, 32.0, 1000.0);
        m.x.v = 225.0;
        let mut x = 0;
        for _ in 0..1000 {
            m.step(0.018);
            x += m.take_x();
        }
        assert!((4049..=4050).contains(&x), "{x}");
    }

    #[test]
    fn double_count_runs_ahead() {
        let mut m = Mover::default();
        m.init(0.0, 32.0, 1000.0);
        m.x.v = 225.0;
        let mut x = 0;
        for _ in 0..21 {
            m.step(0.018);
            x += m.take_x();
        }
        // 4.05 px per step taken as about 4.10 on average: nineteen steps
        // of 4, then two of 5.
        assert_eq!(x, 86);
    }

    #[test]
    fn aim_reaches_the_target_height() {
        let mut m = Mover::default();
        m.init(2250.0, 32.0, 1000.0);
        m.y.p = 100.0;
        m.aim_at(0.0, 55.0);
        assert!((m.y.v + 450.0).abs() < 1e-9);
        assert_eq!(m.x.v, 0.0);
        m.aim_at(0.0, 100.0);
        assert_eq!(m.y.v, 0.0);
    }

    #[test]
    fn fall_speed_is_capped() {
        let mut m = Mover::default();
        m.init(2250.0, 32.0, 1000.0);
        for _ in 0..100 {
            m.step(0.018);
        }
        assert_eq!(m.y.v, 1000.0);
        assert!(m.y.d < 32.0);
    }
}
