//! The help screen: the controls picture over a frozen game.
//!
//! F1 in play (paused or not) or the in-game menu's Help entry shows it;
//! Escape, Space, Enter or a gamepad's choose or back button closes it, and
//! play goes on. It has no timer and no animation: it shows until closed.
//! What its time does to the game is the session's business (the dynamite
//! fuse burns by the time it was up, see `claw_sim::Sim::pass_menu_time`).

use claw_assets::Palette;
use claw_render::{ImageId, ScreenScene};

/// The help screen.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpScreen {
    background: Option<ImageId>,
    palette: Palette,
}

impl HelpScreen {
    /// The help screen showing `background` (the help picture) in
    /// `palette`; without the picture it is black.
    pub fn new(background: Option<ImageId>, palette: Palette) -> HelpScreen {
        HelpScreen {
            background,
            palette,
        }
    }

    /// The picture.
    pub fn scene(&self) -> ScreenScene {
        ScreenScene {
            background: self.background,
            palette: self.palette.clone(),
            sprites: Vec::new(),
            fade: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_is_the_picture_alone() {
        let s = HelpScreen::new(None, Palette::default()).scene();
        assert!(s.background.is_none() && s.sprites.is_empty() && s.fade.is_none());
    }
}
