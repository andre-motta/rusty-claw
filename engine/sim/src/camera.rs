//! The camera logic.
//!
//! The camera keeps a current point and a target point, both in whole
//! level pixels. The target is Claw's position, shifted 200 px down while
//! he has crouched or 200 px up while he has looked up long enough, or a
//! scripted point. When following Claw, a target more than 500 px away on
//! either axis is jumped to at once; otherwise the current point moves
//! toward the target at a speed that depends on what Claw is doing (500
//! px/s on the ground, 4000 px/s while he jumps or falls), in whole pixels
//! per tick, never overshooting. A shake adds small random jolts to the
//! current point; level logics and Claw's code ask for one through the
//! tick's outbox ([`crate::logic::World::shake`]), and the simulation
//! starts it from the tick of the request. The current point is what the
//! view centres on.
//!
//! A warp's set-up takes no game time ([`Camera::settle`]): the camera
//! only jumps to Claw when he is out of its reach, and moves no further.

use crate::physics::ptype;
use crate::player::state as claw_state;
use crate::rng::Rng;
use crate::tuning::{CameraTuning, TICK_MS};

/// Camera states.
pub mod state {
    /// First call: start at Claw.
    pub const INIT: i32 = 0;
    /// Follow Claw.
    pub const FOLLOW: i32 = 24;
    /// Pan down while Claw crouches.
    pub const LOOK_DOWN: i32 = 26;
    /// Pan up while Claw looks up.
    pub const LOOK_UP: i32 = 5003;
    /// Move to a scripted point.
    pub const POINT: i32 = 8888;
}

/// What the camera reads from Claw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Followed {
    /// Position.
    pub x: i32,
    /// Position.
    pub y: i32,
    /// Claw's state id.
    pub state: i32,
    /// Claw's physics type.
    pub physics_type: i32,
    /// Claw's look delay left, ms; the look offsets wait for 0.
    pub look_down_time: i32,
}

/// The camera's own data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Camera {
    /// Camera state.
    pub state: i32,
    /// Current point: where the view is centred.
    pub current: (i32, i32),
    /// Target point.
    pub target: (i32, i32),
    /// Shake countdown, ms.
    pub shake_countdown: u32,
    /// Shake runs while the simulation clock is below this, ms.
    pub shake_until_ms: u64,
    /// The current point jumped this tick.
    pub teleported: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            state: state::INIT,
            current: (0, 0),
            target: (0, 0),
            shake_countdown: 0,
            shake_until_ms: 0,
            teleported: false,
        }
    }
}

fn approach(cur: i32, target: i32, speed: u32) -> i32 {
    let step = (u64::from(TICK_MS) * u64::from(speed) / 1000).min(i32::MAX as u64) as i32;
    if cur < target {
        cur.saturating_add(step).min(target)
    } else {
        cur.saturating_sub(step).max(target)
    }
}

impl Camera {
    /// Starts a shake lasting `ms` from `now_ms`, unless a longer one runs.
    pub fn start_shake(&mut self, now_ms: u64, ms: u64) {
        self.shake_until_ms = self.shake_until_ms.max(now_ms + ms);
    }

    fn follow(&mut self, claw: &Followed, point: Option<(i32, i32)>, t: &CameraTuning) {
        self.state = state::FOLLOW;
        if claw.state == claw_state::DEATH_FALL {
            return;
        }
        if claw.state == claw_state::CROUCH && claw.look_down_time == 0 {
            self.state = state::LOOK_DOWN;
            self.look(claw, t.look_offset, t);
            return;
        }
        if claw.state == claw_state::LOOK_UP && claw.look_down_time == 0 {
            self.state = state::LOOK_UP;
            self.look(claw, -t.look_offset, t);
            return;
        }
        self.target = point.unwrap_or((claw.x, claw.y));
        let (dx, dy) = (
            self.target.0 - self.current.0,
            self.target.1 - self.current.1,
        );
        if dx.abs() > t.snap_distance || dy.abs() > t.snap_distance {
            self.current = self.target;
            self.teleported = true;
        }
        let speed = match claw.physics_type {
            ptype::JUMP | ptype::FALL => t.air_speed,
            ptype::FLY => {
                self.teleported |= self.current != self.target;
                t.fly_speed
            }
            _ => t.follow_speed,
        };
        self.current = (
            approach(self.current.0, self.target.0, speed),
            approach(self.current.1, self.target.1, speed),
        );
    }

    fn look(&mut self, claw: &Followed, offset: i32, t: &CameraTuning) {
        self.target = (claw.x, claw.y + offset);
        self.current = (
            approach(self.current.0, self.target.0, t.look_speed),
            approach(self.current.1, self.target.1, t.look_speed),
        );
    }

    /// The camera's part of a warp's set-up, which takes no game time: when
    /// it follows Claw (no scripted point) and he is more than the snap
    /// distance away on either axis, the current point jumps to him;
    /// otherwise nothing moves. Nothing is drawn from the random stream.
    pub fn settle(&mut self, claw: Option<&Followed>, point: Option<(i32, i32)>, t: &CameraTuning) {
        let Some(claw) = claw else { return };
        if point.is_some()
            || claw.state == claw_state::DEATH_FALL
            || !matches!(
                self.state,
                state::FOLLOW | state::LOOK_DOWN | state::LOOK_UP
            )
        {
            return;
        }
        let target = (claw.x, claw.y);
        let (dx, dy) = (target.0 - self.current.0, target.1 - self.current.1);
        if dx.abs() > t.snap_distance || dy.abs() > t.snap_distance {
            self.state = state::FOLLOW;
            self.target = target;
            self.current = target;
            self.teleported = true;
        }
    }

    /// One logic call. `claw` is None while there is no player; `point` is
    /// the scripted camera point, if any.
    pub fn tick(
        &mut self,
        claw: Option<&Followed>,
        point: Option<(i32, i32)>,
        t: &CameraTuning,
        rng: &mut Rng,
        now_ms: u64,
    ) {
        self.teleported = false;
        let Some(claw) = claw else {
            return;
        };
        if self.state == state::INIT {
            self.current = (claw.x, claw.y);
            self.target = self.current;
            self.state = state::FOLLOW;
            self.teleported = true;
            return;
        }
        if point.is_some() {
            self.state = state::POINT;
        }
        match self.state {
            state::FOLLOW => self.follow(claw, point, t),
            state::LOOK_DOWN => {
                if claw.state == claw_state::CROUCH && claw.look_down_time == 0 {
                    self.look(claw, t.look_offset, t);
                } else {
                    self.follow(claw, point, t);
                }
            }
            state::LOOK_UP => {
                if claw.state == claw_state::LOOK_UP && claw.look_down_time == 0 {
                    self.look(claw, -t.look_offset, t);
                } else {
                    self.follow(claw, point, t);
                }
            }
            state::POINT => match point {
                Some(p) => {
                    self.target = p;
                    self.current = (
                        approach(self.current.0, p.0, t.point_speed),
                        approach(self.current.1, p.1, t.point_speed),
                    );
                }
                None => self.follow(claw, point, t),
            },
            _ => {}
        }
        if self.shake_until_ms > now_ms {
            if TICK_MS > self.shake_countdown {
                self.shake_countdown = rng.below(t.shake_countdown_range.max(1));
                let half = (t.shake_jolt_range / 2) as i32;
                let jx = rng.below(t.shake_jolt_range.max(1)) as i32 - half;
                let jy = rng.below(t.shake_jolt_range.max(1)) as i32 - half;
                self.current.0 += jx;
                self.current.1 += jy;
            } else {
                self.shake_countdown -= TICK_MS;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claw(x: i32, y: i32) -> Followed {
        Followed {
            x,
            y,
            state: claw_state::STAND,
            physics_type: ptype::STAND,
            look_down_time: 0,
        }
    }

    fn run(cam: &mut Camera, c: &Followed) {
        cam.tick(
            Some(c),
            None,
            &CameraTuning::default(),
            &mut Rng::default(),
            0,
        );
    }

    #[test]
    fn follows_at_nine_pixels_per_tick_on_the_ground() {
        let mut cam = Camera::default();
        run(&mut cam, &claw(1000, 1000));
        assert_eq!(cam.current, (1000, 1000));
        run(&mut cam, &claw(1100, 1000));
        assert_eq!(cam.current, (1009, 1000));
        assert!(!cam.teleported);
    }

    #[test]
    fn far_targets_snap() {
        let mut cam = Camera::default();
        run(&mut cam, &claw(1000, 1000));
        run(&mut cam, &claw(1501, 1000));
        assert_eq!(cam.current, (1501, 1000));
        assert!(cam.teleported);
        run(&mut cam, &claw(2001, 1000));
        assert_eq!(cam.current, (2001 - 500 + 9, 1000));
    }

    #[test]
    fn settling_only_jumps_to_a_claw_out_of_reach() {
        let t = CameraTuning::default();
        let mut cam = Camera::default();
        run(&mut cam, &claw(1000, 1000));
        run(&mut cam, &claw(1000, 1000));
        cam.settle(Some(&claw(1400, 1000)), None, &t);
        assert_eq!(cam.current, (1000, 1000), "within reach: no move");
        assert!(!cam.teleported);
        cam.settle(Some(&claw(3000, 1600)), Some((0, 0)), &t);
        assert_eq!(cam.current, (1000, 1000), "a scripted point holds it");
        cam.settle(Some(&claw(3000, 1600)), None, &t);
        assert_eq!((cam.current, cam.target), ((3000, 1600), (3000, 1600)));
        assert!(cam.teleported);
        let mut fresh = Camera::default();
        fresh.settle(Some(&claw(3000, 1600)), None, &t);
        assert_eq!(fresh.current, (0, 0), "not started yet");
    }

    #[test]
    fn air_speed_and_look_offsets() {
        let mut cam = Camera::default();
        run(&mut cam, &claw(1000, 1000));
        let mut c = claw(1000, 1200);
        c.physics_type = ptype::FALL;
        run(&mut cam, &c);
        assert_eq!(cam.current, (1000, 1072));
        let mut c = claw(1000, 1072);
        c.state = claw_state::CROUCH;
        c.look_down_time = 100;
        run(&mut cam, &c);
        assert_eq!(cam.current, (1000, 1072));
        c.look_down_time = 0;
        run(&mut cam, &c);
        assert_eq!(cam.state, state::LOOK_DOWN);
        assert_eq!(cam.current, (1000, 1076));
        for _ in 0..100 {
            run(&mut cam, &c);
        }
        assert_eq!(cam.current, (1000, 1272));
    }
}
