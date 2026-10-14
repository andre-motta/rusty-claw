//! A deterministic display clock for tests and benchmarks.
//!
//! [`SimulatedRefresh`] yields frame durations for a display of a given
//! refresh rate, optionally with random jitter from a fixed seed. Durations
//! are whole microseconds, and the rounding error is carried, so the sum of
//! `n` frames stays within one microsecond of `n` periods.

use std::time::Duration;

/// Frame durations of a simulated display.
#[derive(Debug, Clone)]
pub struct SimulatedRefresh {
    period_us: f64,
    jitter: f64,
    state: u64,
    ideal_us: f64,
    emitted_us: u64,
}

impl SimulatedRefresh {
    /// A display refreshing `hz` times per second with no jitter.
    pub fn new(hz: f64) -> SimulatedRefresh {
        let hz = if hz.is_finite() && hz > 0.0 { hz } else { 60.0 };
        SimulatedRefresh {
            period_us: 1_000_000.0 / hz,
            jitter: 0.0,
            state: 0x9E37_79B9_7F4A_7C15,
            ideal_us: 0.0,
            emitted_us: 0,
        }
    }

    /// Adds jitter: every frame lasts the period times a factor drawn
    /// uniformly from [1 - `fraction`, 1 + `fraction`], from `seed`.
    pub fn with_jitter(mut self, fraction: f64, seed: u64) -> SimulatedRefresh {
        self.jitter = fraction.clamp(0.0, 0.95);
        self.state = seed ^ 0x9E37_79B9_7F4A_7C15;
        if self.state == 0 {
            self.state = 1;
        }
        self
    }

    /// The nominal frame period in microseconds.
    pub fn period_us(&self) -> f64 {
        self.period_us
    }

    fn next_unit(&mut self) -> f64 {
        // xorshift64*: small, fast and the same on every platform.
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (r >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The next frame duration in microseconds.
    pub fn next_us(&mut self) -> u64 {
        let factor = if self.jitter > 0.0 {
            1.0 + self.jitter * (2.0 * self.next_unit() - 1.0)
        } else {
            1.0
        };
        self.ideal_us += self.period_us * factor;
        let target = self.ideal_us.round().max(0.0) as u64;
        let dt = target.saturating_sub(self.emitted_us);
        self.emitted_us += dt;
        dt
    }
}

impl Iterator for SimulatedRefresh {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        Some(Duration::from_micros(self.next_us()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_sum_to_whole_periods() {
        let mut r = SimulatedRefresh::new(144.0);
        let total: u64 = (0..144).map(|_| r.next_us()).sum();
        assert_eq!(total, 1_000_000);
    }

    #[test]
    fn jitter_is_bounded_and_repeatable() {
        let a: Vec<u64> = SimulatedRefresh::new(60.0)
            .with_jitter(0.2, 7)
            .take(500)
            .map(|d| d.as_micros() as u64)
            .collect();
        let b: Vec<u64> = SimulatedRefresh::new(60.0)
            .with_jitter(0.2, 7)
            .take(500)
            .map(|d| d.as_micros() as u64)
            .collect();
        assert_eq!(a, b);
        assert!(a.iter().all(|&d| (13_333..=20_001).contains(&d)));
        assert!(a.windows(2).any(|w| w[0] != w[1]));
    }
}
