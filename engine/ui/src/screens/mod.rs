//! The title, end-of-level, game-over, help and credits screens.
//!
//! Each screen has `update(dt_ms)` to let time pass and `scene()` for the
//! picture, a 640x480 [`claw_render::ScreenScene`] with the screen's own
//! palette; screens with sounds hand them over with `take_audio()` as
//! interface-channel cues.
//!
//! - [`TitleScreen`]: the title picture while its sound plays;
//! - [`BootyScreen`]: the map screen and the treasure tally;
//! - [`GameOverScreen`]: the frozen game fading out after the last life;
//! - [`HelpScreen`]: the controls picture shown over a frozen game;
//! - [`CreditsScreen`]: the credits rising over the parchment after the
//!   last level;
//! - [`assets`]: the screens' art and sounds from the install.

pub mod assets;
pub mod booty;
pub mod credits;
pub mod gameover;
pub mod help;
pub mod title;

pub use assets::{ScreenAssets, ScreenImages};
pub use booty::{BootyArt, BootyPhase, BootyScreen, BootyStats};
pub use credits::{CreditsArt, CreditsEntry, CreditsPhase, CreditsScreen, CreditsTune};
pub use gameover::GameOverScreen;
pub use help::HelpScreen;
pub use title::TitleScreen;
