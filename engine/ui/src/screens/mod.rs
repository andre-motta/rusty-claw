//! The title, end-of-level and game-over screens.
//!
//! Each screen has `update(dt_ms)` to let time pass and `scene()` for the
//! picture, a 640x480 [`claw_render::ScreenScene`] with the screen's own
//! palette; screens with sounds hand them over with `take_audio()` as
//! interface-channel cues.
//!
//! - [`TitleScreen`]: the title picture while its sound plays;
//! - [`BootyScreen`]: the map screen and the treasure tally;
//! - [`GameOverScreen`]: the frozen game fading out after the last life;
//! - [`assets`]: the screens' art and sounds from the install.

pub mod assets;
pub mod booty;
pub mod gameover;
pub mod title;

pub use assets::{ScreenAssets, ScreenImages};
pub use booty::{BootyArt, BootyPhase, BootyScreen, BootyStats};
pub use gameover::GameOverScreen;
pub use title::TitleScreen;
