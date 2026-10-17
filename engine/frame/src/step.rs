//! The fixed-step accumulator.
//!
//! The simulation always advances in ticks of exactly 18 ms. Each rendered
//! frame adds its real duration to an accumulator and runs as many ticks as
//! fit; what is left over, as a fraction of a tick, is `alpha`, the position
//! between the previous and the current snapshot at which the frame is drawn.
//!
//! Two limits keep a slow or stalled machine from spiralling: a frame never
//! adds more than 100 ms, and never runs more than four ticks. When the tick
//! limit is reached the accumulator keeps only its remainder below one tick,
//! so the game slows down instead of trying to catch up forever.
//!
//! Time is kept in whole microseconds so that accumulation is exact and a
//! run depends only on the sequence of frame durations.
//!
//! Stepping out of a pause one tick at a time does not go through the
//! accumulator's time: [`FixedStep::step_once`] makes the next frame run
//! exactly one tick at once, whatever time is left over from before, and
//! counts real time from an empty accumulator after it.

use std::time::{Duration, Instant};

use crate::TICK_MS;

/// Length of one tick in microseconds.
pub const TICK_US: u64 = TICK_MS as u64 * 1000;

/// Most time one frame may add, in microseconds.
pub const MAX_FRAME_US: u64 = 100_000;

/// Most ticks one frame may run.
pub const MAX_TICKS_PER_FRAME: u32 = 4;

/// What one frame of the accumulator did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Advance {
    /// Ticks to run this frame (0 to [`MAX_TICKS_PER_FRAME`]).
    pub ticks: u32,
    /// Position between the previous and the current snapshot, in [0, 1).
    pub alpha: f64,
    /// Time dropped this frame by the frame clamp or the catch-up limit, in
    /// microseconds.
    pub dropped_us: u64,
}

/// The 18 ms accumulator.
#[derive(Debug, Clone, Default)]
pub struct FixedStep {
    accumulator_us: u64,
    paused: bool,
    total_ticks: u64,
}

impl FixedStep {
    /// An empty accumulator.
    pub fn new() -> FixedStep {
        FixedStep::default()
    }

    /// Adds one frame of `frame` duration and returns how many ticks to run.
    pub fn advance(&mut self, frame: Duration) -> Advance {
        let us = u64::try_from(frame.as_micros()).unwrap_or(u64::MAX);
        self.advance_us(us)
    }

    /// As [`FixedStep::advance`], with the frame duration in microseconds.
    pub fn advance_us(&mut self, frame_us: u64) -> Advance {
        if self.paused {
            return Advance {
                ticks: 0,
                alpha: self.alpha(),
                dropped_us: frame_us,
            };
        }
        let added = frame_us.min(MAX_FRAME_US);
        let mut dropped = frame_us - added;
        self.accumulator_us += added;
        let mut ticks = 0;
        while self.accumulator_us >= TICK_US && ticks < MAX_TICKS_PER_FRAME {
            self.accumulator_us -= TICK_US;
            ticks += 1;
        }
        if self.accumulator_us >= TICK_US {
            let kept = self.accumulator_us % TICK_US;
            dropped += self.accumulator_us - kept;
            self.accumulator_us = kept;
        }
        self.total_ticks += u64::from(ticks);
        Advance {
            ticks,
            alpha: self.alpha(),
            dropped_us: dropped,
        }
    }

    /// The current interpolation factor, in [0, 1).
    pub fn alpha(&self) -> f64 {
        self.accumulator_us as f64 / TICK_US as f64
    }

    /// Time in the accumulator, in microseconds.
    pub fn accumulator_us(&self) -> u64 {
        self.accumulator_us
    }

    /// Ticks run since creation.
    pub fn total_ticks(&self) -> u64 {
        self.total_ticks
    }

    /// Stops ticking: frames add no time and alpha stays where it is, so
    /// everything drawn from simulation time (fades, blinking) freezes too.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes ticking. Reset the frame clock as well, so that the first
    /// frame after the pause adds no time.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// True while paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Empties the accumulator, for a level start.
    pub fn reset(&mut self) {
        self.accumulator_us = 0;
    }

    /// Makes the next frame run exactly one tick at once, for stepping out
    /// of a pause: the time left in the accumulator is dropped and one
    /// whole tick takes its place, so the next [`FixedStep::advance`] runs
    /// that tick first and then counts its own duration from an empty
    /// accumulator (a frame of zero time runs the one tick alone). Calling
    /// it again before that frame still leaves exactly one tick.
    pub fn step_once(&mut self) {
        self.accumulator_us = TICK_US;
    }
}

/// Measures the duration of each rendered frame from a monotonic clock.
#[derive(Debug, Clone, Default)]
pub struct FrameClock {
    last: Option<Instant>,
}

impl FrameClock {
    /// A clock whose first frame lasts zero.
    pub fn new() -> FrameClock {
        FrameClock::default()
    }

    /// Time since the previous call; zero on the first call and after
    /// [`FrameClock::reset`].
    pub fn frame(&mut self) -> Duration {
        let now = Instant::now();
        let dt = self.last.map_or(Duration::ZERO, |t| now - t);
        self.last = Some(now);
        dt
    }

    /// Makes the next frame last zero, for example after a pause or when the
    /// window regains focus.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_whole_18_ms_steps() {
        let mut step = FixedStep::new();
        let a = step.advance_us(17_999);
        assert_eq!(a.ticks, 0);
        let a = step.advance_us(1);
        assert_eq!(a.ticks, 1);
        assert_eq!(a.alpha, 0.0);
        let a = step.advance_us(27_000);
        assert_eq!(a.ticks, 1);
        assert!((a.alpha - 0.5).abs() < 1e-12);
        assert_eq!(step.total_ticks(), 2);
    }

    #[test]
    fn long_frames_are_clamped_and_capped() {
        let mut step = FixedStep::new();
        // 100 ms adds 5 ticks' worth, but only 4 run; the rest is dropped
        // down to the remainder below one tick.
        let a = step.advance_us(1_000_000);
        assert_eq!(a.ticks, 4);
        assert_eq!(step.accumulator_us(), 100_000 % TICK_US);
        assert_eq!(a.dropped_us, 900_000 + TICK_US);
        assert!(a.alpha < 1.0);
    }

    #[test]
    fn pause_freezes_alpha() {
        let mut step = FixedStep::new();
        step.advance_us(9_000);
        step.pause();
        let a = step.advance_us(50_000);
        assert_eq!(a.ticks, 0);
        assert!((a.alpha - 0.5).abs() < 1e-12);
        step.resume();
        assert_eq!(step.advance_us(9_000).ticks, 1);
    }

    #[test]
    fn step_once_runs_one_tick_then_real_time() {
        let mut step = FixedStep::new();
        step.advance_us(9_000);
        step.pause();
        step.advance_us(500_000);
        step.resume();
        step.step_once();
        step.step_once();
        assert_eq!(step.accumulator_us(), TICK_US);
        // A frame of no time runs the single tick and leaves nothing over.
        let a = step.advance_us(0);
        assert_eq!(a.ticks, 1);
        assert_eq!(a.alpha, 0.0);
        assert_eq!(step.accumulator_us(), 0);
        // Then real time counts from an empty accumulator.
        assert_eq!(step.advance_us(17_999).ticks, 0);
        assert_eq!(step.advance_us(1).ticks, 1);
        // With time in the same frame, the tick comes first and the time
        // after it counts as usual.
        step.step_once();
        let a = step.advance_us(27_000);
        assert_eq!(a.ticks, 2);
        assert!((a.alpha - 0.5).abs() < 1e-12);
        assert_eq!(step.total_ticks(), 4);
    }

    #[test]
    fn clock_starts_at_zero() {
        let mut clock = FrameClock::new();
        assert_eq!(clock.frame(), Duration::ZERO);
        clock.reset();
        assert_eq!(clock.frame(), Duration::ZERO);
    }
}
