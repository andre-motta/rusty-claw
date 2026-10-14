//! The per-frame loop: accumulate time, run whole ticks, keep the previous
//! and the current snapshot, and say where between them to draw.
//!
//! ```
//! use claw_frame::{FrameDriver, SimSnapshot, SimulatedRefresh};
//!
//! let mut driver = FrameDriver::new(SimSnapshot::default());
//! let mut tick = 0;
//! for dt in SimulatedRefresh::new(144.0).take(144) {
//!     driver.frame(dt, || {
//!         tick += 1;
//!         SimSnapshot { tick, ..SimSnapshot::default() }
//!     });
//!     let _frame = driver.interpolated();
//! }
//! assert_eq!(driver.current().tick, 55); // 1000 ms / 18 ms
//! ```

use std::time::Duration;

use crate::interp::{InterpolatedFrame, interpolate};
use crate::step::FixedStep;
use crate::{SimSnapshot, TickInput};

/// What one frame did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameReport {
    /// Ticks run.
    pub ticks: u32,
    /// Interpolation factor after the ticks, in [0, 1).
    pub alpha: f64,
}

/// Fixed-step accumulator plus the snapshot pair.
#[derive(Debug, Clone)]
pub struct FrameDriver {
    step: FixedStep,
    prev: SimSnapshot,
    curr: SimSnapshot,
    alpha: f64,
}

impl FrameDriver {
    /// Starts from `first`, used as both previous and current snapshot so
    /// that nothing slides in from a stale position.
    pub fn new(first: SimSnapshot) -> FrameDriver {
        FrameDriver {
            step: FixedStep::new(),
            prev: first.clone(),
            curr: first,
            alpha: 0.0,
        }
    }

    /// Restarts from `snapshot`, as on a level start or reload.
    pub fn restart(&mut self, snapshot: SimSnapshot) {
        self.prev = snapshot.clone();
        self.curr = snapshot;
        self.step.reset();
        self.alpha = 0.0;
    }

    /// Adds one frame of `dt` and runs `tick` once per whole tick, keeping
    /// the last two snapshots.
    pub fn frame(&mut self, dt: Duration, mut tick: impl FnMut() -> SimSnapshot) -> FrameReport {
        let adv = self.step.advance(dt);
        for _ in 0..adv.ticks {
            let next = tick();
            self.prev = std::mem::replace(&mut self.curr, next);
        }
        self.alpha = adv.alpha;
        FrameReport {
            ticks: adv.ticks,
            alpha: adv.alpha,
        }
    }

    /// As [`FrameDriver::frame`], for a simulation that takes per-tick input:
    /// `input` is called once per tick, before `tick`.
    pub fn frame_with_input(
        &mut self,
        dt: Duration,
        mut input: impl FnMut() -> TickInput,
        mut tick: impl FnMut(TickInput) -> SimSnapshot,
    ) -> FrameReport {
        self.frame(dt, || tick(input()))
    }

    /// The frame to draw now.
    pub fn interpolated(&self) -> InterpolatedFrame<'_> {
        interpolate(&self.prev, &self.curr, self.alpha)
    }

    /// The frame to draw now, without interpolation (the current snapshot).
    pub fn latest(&self) -> InterpolatedFrame<'_> {
        interpolate(&self.curr, &self.curr, 0.0)
    }

    /// The previous snapshot.
    pub fn previous(&self) -> &SimSnapshot {
        &self.prev
    }

    /// The current snapshot.
    pub fn current(&self) -> &SimSnapshot {
        &self.curr
    }

    /// The interpolation factor of the last frame.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// The accumulator, for pause and resume.
    pub fn step_mut(&mut self) -> &mut FixedStep {
        &mut self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refresh::SimulatedRefresh;
    use crate::step::{MAX_TICKS_PER_FRAME, TICK_US};
    use crate::{Buttons, InputLatch, Point};

    /// Per-tick record (tick, camera x) and per-frame (ticks, alpha) pairs.
    type Run = (Vec<(u64, i64)>, Vec<(u32, f64)>);

    /// Runs `seconds` of a simulated display.
    fn run(hz: f64, jitter: Option<u64>, seconds: f64) -> Run {
        let mut refresh = SimulatedRefresh::new(hz);
        if let Some(seed) = jitter {
            refresh = refresh.with_jitter(0.2, seed);
        }
        let mut driver = FrameDriver::new(SimSnapshot::default());
        let mut trace = Vec::new();
        let mut frames = Vec::new();
        let mut tick = 0u64;
        let mut x = 0i64;
        let mut elapsed = 0u64;
        let limit = (seconds * 1e6) as u64;
        while elapsed < limit {
            let dt = refresh.next().unwrap();
            elapsed += dt.as_micros() as u64;
            let r = driver.frame(dt, || {
                tick += 1;
                x += 9;
                trace.push((tick, x));
                SimSnapshot {
                    tick,
                    camera: Point {
                        x: x as f64,
                        y: 0.0,
                    },
                    ..SimSnapshot::default()
                }
            });
            frames.push((r.ticks, r.alpha));
        }
        (trace, frames)
    }

    #[test]
    fn ticks_do_not_depend_on_the_display_rate() {
        let (base, _) = run(60.0, None, 3.0);
        for (hz, jitter) in [
            (144.0, None),
            (240.0, None),
            (60.0, Some(1)),
            (144.0, Some(2)),
        ] {
            let (trace, frames) = run(hz, jitter, 3.0);
            let n = base.len().min(trace.len());
            assert!(n >= 160, "{hz} Hz ran {n} ticks");
            assert_eq!(base[..n], trace[..n], "{hz} Hz");
            for (ticks, alpha) in frames {
                assert!(ticks <= MAX_TICKS_PER_FRAME);
                assert!((0.0..1.0).contains(&alpha));
            }
        }
    }

    #[test]
    fn tick_count_follows_time() {
        for hz in [60.0, 144.0, 240.0] {
            let (trace, _) = run(hz, None, 1.8);
            // 1.8 s holds exactly 100 ticks; the last frame may overshoot by
            // less than one frame.
            let expected = 1_800_000 / TICK_US;
            let n = trace.len() as u64;
            assert!(n == expected || n == expected + 1, "{hz} Hz: {n}");
        }
    }

    #[test]
    fn each_press_reaches_exactly_one_tick() {
        for hz in [60.0, 144.0, 240.0] {
            let mut refresh = SimulatedRefresh::new(hz).with_jitter(0.2, hz as u64);
            let mut driver = FrameDriver::new(SimSnapshot::default());
            let mut latch = InputLatch::new();
            let mut presses = 0u32;
            let mut seen = 0u32;
            let mut ticks = 0u64;
            for frame in 0..2000u32 {
                // Tap jump on every seventh frame, between ticks.
                if frame % 7 == 0 {
                    latch.press(Buttons::JUMP);
                    latch.release(Buttons::JUMP);
                    presses += 1;
                }
                let dt = refresh.next().unwrap();
                driver.frame_with_input(
                    dt,
                    || latch.next_tick(),
                    |input| {
                        ticks += 1;
                        if input.pressed.contains(Buttons::JUMP) {
                            seen += 1;
                            assert!(input.held.contains(Buttons::JUMP));
                        }
                        SimSnapshot {
                            tick: ticks,
                            ..SimSnapshot::default()
                        }
                    },
                );
            }
            // Presses still latched at the end have not reached a tick yet.
            let mut pending = 0;
            for _ in 0..4 {
                pending += u32::from(latch.next_tick().pressed.contains(Buttons::JUMP));
            }
            assert_eq!(seen + pending, presses, "{hz} Hz");
        }
    }

    #[test]
    fn restart_uses_one_snapshot_for_both() {
        let mut driver = FrameDriver::new(SimSnapshot::default());
        driver.frame(Duration::from_millis(40), || SimSnapshot {
            tick: 1,
            ..SimSnapshot::default()
        });
        driver.restart(SimSnapshot {
            tick: 50,
            ..SimSnapshot::default()
        });
        assert_eq!(driver.previous().tick, 50);
        assert_eq!(driver.current().tick, 50);
        assert_eq!(driver.alpha(), 0.0);
    }
}
