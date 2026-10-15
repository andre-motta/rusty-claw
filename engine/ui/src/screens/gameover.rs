//! The end of a game: after the last life is lost the picture of the game
//! stays frozen and fades to black, then the main menu follows.
//!
//! The game itself goes straight from the last death to the main menu;
//! this screen only holds the frozen picture for [`HOLD_MS`] and fades it
//! out over [`FADE_MS`], so the end is not abrupt. Any choose or back input
//! after [`GUARD_MS`] skips the rest.

use claw_assets::Palette;
use claw_render::ScreenScene;

/// How long the frozen picture holds before fading, ms.
pub const HOLD_MS: f64 = 1500.0;
/// How long the fade to black takes, ms.
pub const FADE_MS: f64 = 1000.0;
/// Inputs are ignored this long after the screen starts, so a key still
/// held from play does not skip it, ms.
pub const GUARD_MS: f64 = 500.0;

/// The game-over hold and fade.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameOverScreen {
    elapsed: f64,
    skipped: bool,
}

impl GameOverScreen {
    /// A new game-over screen.
    pub fn new() -> GameOverScreen {
        GameOverScreen::default()
    }

    /// Lets `dt_ms` pass.
    pub fn update(&mut self, dt_ms: f64) {
        if dt_ms.is_finite() && dt_ms > 0.0 {
            self.elapsed += dt_ms;
        }
    }

    /// A choose or back input.
    pub fn skip(&mut self) {
        if self.elapsed >= GUARD_MS {
            self.skipped = true;
        }
    }

    /// Time since the screen started, ms.
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed
    }

    /// Brightness of the frozen picture, 0 (black) to 1.
    pub fn brightness(&self) -> f64 {
        if self.skipped {
            return 0.0;
        }
        (1.0 - (self.elapsed - HOLD_MS).max(0.0) / FADE_MS).clamp(0.0, 1.0)
    }

    /// True once the picture has faded out (or the screen was skipped).
    pub fn done(&self) -> bool {
        self.skipped || self.elapsed >= HOLD_MS + FADE_MS
    }

    /// What shows once the frozen picture has gone: black.
    pub fn scene(&self) -> ScreenScene {
        ScreenScene {
            background: None,
            palette: Palette::default(),
            sprites: Vec::new(),
            fade: None,
        }
    }
}
