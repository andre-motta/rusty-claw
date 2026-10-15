//! The main menu's sparkles: fifteen small glints at fixed places that
//! each play their animation once (a frame every 100 ms), then wait a
//! random 1 to 5 seconds before playing it again.

use claw_render::{Anchor, ScreenSprite};

/// Image set of a sparkle.
pub const SPARKLE: &str = "MENU_SPARKLE";
/// Frame time of the sparkle animation, ms.
pub const SPARKLE_FRAME_MS: f64 = 100.0;
/// Shortest and longest wait between two sparkles, ms.
pub const SPARKLE_WAIT_MS: (u32, u32) = (1000, 5000);
/// Where the sparkles are.
pub const SPARKLE_POSITIONS: [(i32, i32); 15] = [
    (286, 49),
    (290, 10),
    (289, 80),
    (316, 23),
    (327, 60),
    (329, 23),
    (375, 80),
    (391, 23),
    (407, 15),
    (415, 12),
    (421, 108),
    (443, 129),
    (347, 81),
    (210, 43),
    (232, 96),
];

#[derive(Debug, Clone, Copy, PartialEq)]
struct Sparkle {
    playing: bool,
    ms: f64,
}

/// The sparkles of the main menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Sparkles {
    frames: Vec<i32>,
    seed: u32,
    sparkles: Vec<Sparkle>,
}

impl Sparkles {
    /// Sparkles cycling through `frames` (the set's frame numbers), with
    /// waits drawn from `seed`.
    pub fn new(frames: Vec<i32>, seed: u32) -> Sparkles {
        let mut s = Sparkles {
            frames,
            seed: seed | 1,
            sparkles: Vec::new(),
        };
        for _ in SPARKLE_POSITIONS {
            let ms = s.wait();
            s.sparkles.push(Sparkle { playing: false, ms });
        }
        s
    }

    fn wait(&mut self) -> f64 {
        // xorshift32
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        let (lo, hi) = SPARKLE_WAIT_MS;
        f64::from(lo + x % (hi - lo + 1))
    }

    /// Lets `dt_ms` pass.
    pub fn update(&mut self, dt_ms: f64) {
        let dt = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        let length = self.frames.len() as f64 * SPARKLE_FRAME_MS;
        for i in 0..self.sparkles.len() {
            let mut s = self.sparkles[i];
            if s.playing {
                s.ms += dt;
                if s.ms >= length {
                    s.playing = false;
                    s.ms = self.wait();
                }
            } else {
                s.ms -= dt;
                if s.ms <= 0.0 {
                    s.playing = !self.frames.is_empty();
                    s.ms = 0.0;
                    if !s.playing {
                        s.ms = self.wait();
                    }
                }
            }
            self.sparkles[i] = s;
        }
    }

    /// The sparkles showing now.
    pub fn sprites(&self) -> Vec<ScreenSprite> {
        let mut out = Vec::new();
        for (s, &(x, y)) in self.sparkles.iter().zip(SPARKLE_POSITIONS.iter()) {
            if !s.playing {
                continue;
            }
            let i = (s.ms / SPARKLE_FRAME_MS).floor() as usize;
            if let Some(&frame) = self.frames.get(i) {
                out.push(ScreenSprite::new(SPARKLE, frame, Anchor::TopLeft, x, y));
            }
        }
        out
    }
}
