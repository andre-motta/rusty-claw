//! How Claw is drawn while a powerup runs.
//!
//! - While invisibility runs Claw is drawn see-through, in
//!   [`DrawMode::GHOST`]: each of his pixels becomes the level's averaging
//!   table entry for the pixel below and his own.
//! - While invulnerability runs he is drawn as a dark silhouette, in
//!   [`DrawMode::SHADOW`]: his shape only, each covered pixel becoming the
//!   averaging table entry for the pixel below and a colour that steps
//!   through [`SILHOUETTE_COLORS`] every [`SILHOUETTE_STEP_MS`].
//! - When either ends, is replaced by another powerup, or he dies, he is
//!   drawn normally again; a new level starts him normal.
//!
//! The powerup step after every state handler sets the look
//! ([`super::death::powerup_tick`]), so the tick that starts a powerup still
//! shows him as before and the look follows from the next tick.
//!
//! The silhouette colour runs on its own timer: each step counts it down
//! by the tick length, and once the tick length reaches what is left the
//! colour steps and the timer reloads to [`SILHOUETTE_STEP_MS`], the
//! overshoot dropped. With 18 ms ticks the colour therefore steps every
//! sixth tick (108 ms). The timer and the colour start at 0, so the first
//! invulnerable tick of a level steps at once, to `0x65`; they are kept
//! when the powerup ends, so a later invulnerability carries on from where
//! the last one stopped. The play-test invincibility aid is not this
//! powerup and leaves Claw's look alone.

use claw_frame::DrawMode;

/// Time between two silhouette colour steps, ms.
pub const SILHOUETTE_STEP_MS: u32 = 100;

/// The silhouette colours in the order they step through; after the last
/// comes the first again.
pub const SILHOUETTE_COLORS: [u8; 5] = [0x34, 0x35, 0x36, 0x37, 0x65];

/// Claw's look and the silhouette colour timer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Look {
    /// Draw mode of Claw's sprite.
    pub mode: DrawMode,
    /// Colour value of the mode; `None` for the default.
    pub color: Option<u8>,
    /// Time left before the next silhouette colour step, ms.
    pub step_ms: u32,
    /// Current silhouette colour; 0 before the first step.
    pub step_color: u8,
}

/// The silhouette colour after `c`. Any value outside the cycle, the
/// starting 0 among them, steps to `0x65`.
pub fn next_silhouette_color(c: u8) -> u8 {
    match c {
        0x34 => 0x35,
        0x35 => 0x36,
        0x36 => 0x37,
        0x65 => 0x34,
        _ => 0x65,
    }
}

impl Look {
    /// Drawn normally.
    pub fn set_normal(&mut self) {
        self.mode = DrawMode::NORMAL;
        self.color = None;
    }

    /// Drawn see-through (invisibility).
    pub fn set_ghost(&mut self) {
        self.mode = DrawMode::GHOST;
        self.color = None;
    }

    /// One tick of `dt` ms of the colour-cycling silhouette
    /// (invulnerability).
    pub fn silhouette_tick(&mut self, dt: u32) {
        if dt >= self.step_ms {
            self.step_ms = SILHOUETTE_STEP_MS;
            self.step_color = next_silhouette_color(self.step_color);
        } else {
            self.step_ms -= dt;
        }
        self.mode = DrawMode::SHADOW;
        self.color = Some(self.step_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_cycle_and_anything_else_starts_at_the_last() {
        let mut c = 0;
        let mut seen = Vec::new();
        for _ in 0..7 {
            c = next_silhouette_color(c);
            seen.push(c);
        }
        assert_eq!(seen, [0x65, 0x34, 0x35, 0x36, 0x37, 0x65, 0x34]);
        assert_eq!(next_silhouette_color(0x40), 0x65);
        assert!(SILHOUETTE_COLORS.iter().all(|&c| {
            let next = next_silhouette_color(c);
            SILHOUETTE_COLORS.contains(&next) && next != c
        }));
    }

    #[test]
    fn steps_every_sixth_tick_at_18_ms() {
        let mut l = Look::default();
        let colors: Vec<u8> = (0..14)
            .map(|_| {
                l.silhouette_tick(18);
                assert_eq!(l.mode, DrawMode::SHADOW);
                l.color.unwrap()
            })
            .collect();
        let mut want = vec![0x65; 6];
        want.extend([0x34; 6]);
        want.extend([0x35; 2]);
        assert_eq!(colors, want);
        l.set_normal();
        assert_eq!((l.mode, l.color), (DrawMode::NORMAL, None));
        // The timer and the colour are kept for the next silhouette.
        assert_eq!((l.step_color, l.step_ms), (0x35, 82));
    }
}
