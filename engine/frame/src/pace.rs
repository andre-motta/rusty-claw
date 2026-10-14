//! Frame pacing: a frame-rate cap for drawing that does not wait for the
//! display's refresh, and the pace of a benchmark's simulated display.
//!
//! Drawing without a limit runs at thousands of frames per second, which
//! makes some GPUs whine audibly and wastes power, so every path that does
//! not wait for the display is paced. [`FramePacer::wait`] is called once at
//! the start of each frame, before the frame's work. It sleeps until the
//! frame is due and spins only for the last fraction of a millisecond, so
//! the rate is accurate without keeping a core busy for the whole interval.
//!
//! Deadlines advance by whole periods: when one frame starts a little late,
//! the next one is due one period after the missed deadline, not one period
//! after the late start, so the average rate holds. A frame that starts more
//! than a whole period late begins a new schedule instead of rushing the
//! following frames to catch up.

use std::time::{Duration, Instant};

/// Frame-rate cap when the display's refresh does not pace drawing.
pub const DEFAULT_FPS_CAP: u32 = 300;

/// The end of each wait that is spun instead of slept: a little more than a
/// sleep usually overruns on a desktop system.
pub const SPIN: Duration = Duration::from_micros(250);

/// Paces frames to a fixed rate, or not at all.
#[derive(Debug, Clone, Default)]
pub struct FramePacer {
    period: Option<Duration>,
    next: Option<Instant>,
}

impl FramePacer {
    /// Frames at most `hz` times per second. A rate that is not a positive
    /// finite number means no limit.
    pub fn new(hz: f64) -> FramePacer {
        let period = (hz.is_finite() && hz > 0.0).then(|| Duration::from_secs_f64(1.0 / hz));
        FramePacer {
            period: period.filter(|p| !p.is_zero()),
            next: None,
        }
    }

    /// A cap of `fps` frames per second; `None` or 0 means no limit.
    pub fn capped(fps: Option<u32>) -> FramePacer {
        match fps {
            Some(n) if n > 0 => FramePacer::new(f64::from(n)),
            _ => FramePacer::uncapped(),
        }
    }

    /// No limit: [`FramePacer::wait`] returns at once.
    pub fn uncapped() -> FramePacer {
        FramePacer::default()
    }

    /// The frame period, or `None` without a limit.
    pub fn period(&self) -> Option<Duration> {
        self.period
    }

    /// The rate in frames per second, or `None` without a limit.
    pub fn hz(&self) -> Option<f64> {
        self.period.map(|p| 1.0 / p.as_secs_f64())
    }

    /// True when frames are paced.
    pub fn is_paced(&self) -> bool {
        self.period.is_some()
    }

    /// Starts a new schedule: the next frame is not held back, for example
    /// after a pause.
    pub fn reset(&mut self) {
        self.next = None;
    }

    /// Schedules a frame starting at `now`: returns the instant to wait
    /// for, or `None` when the frame may start at once, and moves the
    /// schedule on by one period.
    pub fn schedule(&mut self, now: Instant) -> Option<Instant> {
        let period = self.period?;
        let Some(due) = self.next else {
            self.next = Some(now + period);
            return None;
        };
        if now < due {
            self.next = Some(due + period);
            return Some(due);
        }
        // Late: keep the schedule when less than a period behind, so the
        // average rate holds, or start a new one.
        self.next = Some(if now - due > period {
            now + period
        } else {
            due + period
        });
        None
    }

    /// Waits until the next frame is due and returns the time waited.
    pub fn wait(&mut self) -> Duration {
        if self.period.is_none() {
            return Duration::ZERO;
        }
        let start = Instant::now();
        let Some(due) = self.schedule(start) else {
            return Duration::ZERO;
        };
        if let Some(wake) = due.checked_sub(SPIN) {
            let now = Instant::now();
            if wake > now {
                std::thread::sleep(wake - now);
            }
        }
        while Instant::now() < due {
            std::thread::yield_now();
        }
        start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn no_limit_never_waits() {
        for mut p in [
            FramePacer::uncapped(),
            FramePacer::capped(None),
            FramePacer::capped(Some(0)),
            FramePacer::new(0.0),
            FramePacer::new(f64::NAN),
            FramePacer::new(f64::INFINITY),
        ] {
            assert!(!p.is_paced());
            let t = Instant::now();
            assert_eq!(p.schedule(t), None);
            assert_eq!(p.schedule(t), None);
            assert_eq!(p.wait(), Duration::ZERO);
        }
    }

    #[test]
    fn deadlines_advance_by_whole_periods() {
        let mut p = FramePacer::new(250.0);
        assert_eq!(p.period(), Some(4 * MS));
        let t0 = Instant::now();
        // The first frame starts at once.
        assert_eq!(p.schedule(t0), None);
        // Early frames wait for their deadline.
        assert_eq!(p.schedule(t0 + MS), Some(t0 + 4 * MS));
        assert_eq!(p.schedule(t0 + 5 * MS), Some(t0 + 8 * MS));
        // A frame 1 ms late starts at once, and the next one is still due
        // on the original schedule.
        assert_eq!(p.schedule(t0 + 13 * MS), None);
        assert_eq!(p.schedule(t0 + 14 * MS), Some(t0 + 16 * MS));
        // A frame more than a period late starts a new schedule rather than
        // rushing the next frames.
        assert_eq!(p.schedule(t0 + 40 * MS), None);
        assert_eq!(p.schedule(t0 + 41 * MS), Some(t0 + 44 * MS));
        p.reset();
        assert_eq!(p.schedule(t0 + 42 * MS), None);
        assert_eq!(p.schedule(t0 + 43 * MS), Some(t0 + 46 * MS));
    }

    #[test]
    fn a_capped_loop_holds_the_rate() {
        let mut p = FramePacer::capped(Some(DEFAULT_FPS_CAP));
        let frames = 60;
        let start = Instant::now();
        for _ in 0..=frames {
            p.wait();
        }
        let took = start.elapsed().as_secs_f64();
        let want = f64::from(frames) / f64::from(DEFAULT_FPS_CAP);
        // Never faster than the cap; the upper bound leaves room for a busy
        // test machine.
        assert!(took >= want * 0.999, "{took} s for {want} s");
        assert!(took < want * 1.5, "{took} s for {want} s");
    }
}
