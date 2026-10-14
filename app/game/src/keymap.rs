//! The keyboard map: physical keys to game buttons.
//!
//! The default map uses the arrow keys to move, climb, crouch and look up,
//! and Space to jump. Keys are matched by their physical position, so the
//! map works on any keyboard layout.

use claw_frame::Buttons;
use winit::keyboard::KeyCode;

/// Keys that control the game itself rather than Claw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Quit.
    Quit,
    /// Toggle borderless fullscreen.
    ToggleFullscreen,
    /// Next window size.
    NextWindowSize,
    /// Next aspect policy.
    NextAspect,
    /// Toggle fit and integer scaling.
    ToggleScale,
    /// Toggle interpolation between ticks.
    ToggleInterpolation,
    /// Pause or resume.
    Pause,
}

/// A key's meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A game button, held while the key is down.
    Button(Buttons),
    /// A command, run when the key goes down.
    Command(Command),
}

/// The keyboard map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    entries: Vec<(KeyCode, Action)>,
}

impl Default for Keymap {
    fn default() -> Self {
        use Action::{Button as B, Command as C};
        Keymap {
            entries: vec![
                (KeyCode::ArrowLeft, B(Buttons::LEFT)),
                (KeyCode::ArrowRight, B(Buttons::RIGHT)),
                (KeyCode::ArrowUp, B(Buttons::UP)),
                (KeyCode::ArrowDown, B(Buttons::DOWN)),
                (KeyCode::Space, B(Buttons::JUMP)),
                (KeyCode::Escape, C(Command::Quit)),
                (KeyCode::F11, C(Command::ToggleFullscreen)),
                (KeyCode::F10, C(Command::NextWindowSize)),
                (KeyCode::F9, C(Command::NextAspect)),
                (KeyCode::F8, C(Command::ToggleScale)),
                (KeyCode::F7, C(Command::ToggleInterpolation)),
                (KeyCode::Pause, C(Command::Pause)),
                (KeyCode::KeyP, C(Command::Pause)),
            ],
        }
    }
}

impl Keymap {
    /// What `key` does, if anything.
    pub fn action(&self, key: KeyCode) -> Option<Action> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|&(_, a)| a)
    }

    /// Every game button the map can produce.
    pub fn buttons(&self) -> Buttons {
        self.entries
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Button(b) => Some(*b),
                Action::Command(_) => None,
            })
            .fold(Buttons::NONE, |acc, b| acc | b)
    }

    /// Help text: one line per key.
    pub fn help(&self) -> &'static str {
        "arrows move, climb, crouch and look up; Space jumps; P or Pause pauses; \
         F7 interpolation; F8 fit or integer scaling; F9 aspect (fill, centered, 4:3); \
         F10 window size; F11 fullscreen; Escape quits"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_moves_with_arrows_and_jumps_with_space() {
        let map = Keymap::default();
        let b = |k| match map.action(k) {
            Some(Action::Button(b)) => b,
            other => panic!("{k:?}: {other:?}"),
        };
        assert_eq!(b(KeyCode::ArrowLeft), Buttons::LEFT);
        assert_eq!(b(KeyCode::ArrowRight), Buttons::RIGHT);
        assert_eq!(b(KeyCode::ArrowUp), Buttons::UP);
        assert_eq!(b(KeyCode::ArrowDown), Buttons::DOWN);
        assert_eq!(b(KeyCode::Space), Buttons::JUMP);
        assert_eq!(
            map.buttons(),
            Buttons::LEFT | Buttons::RIGHT | Buttons::UP | Buttons::DOWN | Buttons::JUMP
        );
        assert_eq!(
            map.action(KeyCode::Escape),
            Some(Action::Command(Command::Quit))
        );
        assert_eq!(map.action(KeyCode::KeyQ), None);
    }
}
