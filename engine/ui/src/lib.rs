//! Presentation models: what the status bar, the menus and the standalone
//! screens show, as screen sprites and screens for the renderer. Nothing
//! here opens a window or uses the GPU.
//!
//! - [`hud`]: the status bar (score, health, lives, weapon and ammunition,
//!   the powerup stopwatch) and the in-game messages;
//! - [`menu`]: menu trees and navigation;
//! - [`screens`]: the title, end-of-level and game-over screens.

#![forbid(unsafe_code)]

pub mod hud;
pub mod menu;
pub mod screens;
