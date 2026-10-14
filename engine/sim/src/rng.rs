//! Seeded deterministic random numbers.
//!
//! Game randomness (the camera shake, idle voice chances and later enemy
//! decisions) comes from one shared stream, drawn in a fixed order inside the
//! tick, so two runs with the same seed and the same input produce the same
//! game.

/// A linear congruential generator returning 15-bit values, like the
/// classic C library `rand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: u32,
}

/// Largest value [`Rng::next_u15`] returns.
pub const RAND_MAX: u32 = 0x7FFF;

impl Rng {
    /// A generator starting from `seed`.
    pub const fn new(seed: u32) -> Self {
        Rng { state: seed }
    }

    /// The next value in `0..=RAND_MAX`.
    pub fn next_u15(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.state >> 16) & RAND_MAX
    }

    /// `next_u15() % n`, the form the game logic uses; `n` must not be 0.
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u15() % n
    }

    /// The internal state, for snapshots and tests.
    pub const fn state(&self) -> u32 {
        self.state
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng::new(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_is_fixed() {
        let mut rng = Rng::new(1);
        let first: Vec<u32> = (0..5).map(|_| rng.next_u15()).collect();
        assert_eq!(first, vec![41, 18467, 6334, 26500, 19169]);
    }

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(1234);
        let mut b = Rng::new(1234);
        for _ in 0..1000 {
            assert_eq!(a.next_u15(), b.next_u15());
        }
        assert!(a.below(36) < 36);
    }
}
