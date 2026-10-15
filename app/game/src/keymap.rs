//! The keyboard map: physical keys to game buttons.
//!
//! The default map is the original game's: the arrow keys move, climb,
//! crouch and look up, Space jumps, Ctrl attacks, Alt fires the selected
//! weapon, Shift switches weapons, Z lifts and throws, and 1, 2 and 3
//! select the pistol, magic and dynamite; the left and right Ctrl, Alt
//! and Shift keys both work. Keys are matched by their physical position,
//! so the map works on any keyboard layout.

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
                (KeyCode::ControlLeft, B(Buttons::ATTACK)),
                (KeyCode::ControlRight, B(Buttons::ATTACK)),
                (KeyCode::AltLeft, B(Buttons::PROJECTILE)),
                (KeyCode::AltRight, B(Buttons::PROJECTILE)),
                (KeyCode::ShiftLeft, B(Buttons::TOGGLE)),
                (KeyCode::ShiftRight, B(Buttons::TOGGLE)),
                (KeyCode::KeyZ, B(Buttons::LIFT)),
                (KeyCode::Digit1, B(Buttons::PISTOL)),
                (KeyCode::Digit2, B(Buttons::MAGIC)),
                (KeyCode::Digit3, B(Buttons::DYNAMITE)),
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
        "arrows move, climb, crouch and look up; Space jumps; Ctrl attacks; \
         Alt fires the weapon; Shift switches weapons; Z lifts and throws; \
         1, 2, 3 select pistol, magic, dynamite; P or Pause pauses; \
         F7 interpolation; F8 fit or integer scaling; F9 aspect (fill, centered, 4:3); \
         F10 window size; F11 fullscreen; Escape quits"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_is_the_original_one() {
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
        for (k, want) in [
            (KeyCode::ControlLeft, Buttons::ATTACK),
            (KeyCode::ControlRight, Buttons::ATTACK),
            (KeyCode::AltLeft, Buttons::PROJECTILE),
            (KeyCode::AltRight, Buttons::PROJECTILE),
            (KeyCode::ShiftLeft, Buttons::TOGGLE),
            (KeyCode::ShiftRight, Buttons::TOGGLE),
            (KeyCode::KeyZ, Buttons::LIFT),
            (KeyCode::Digit1, Buttons::PISTOL),
            (KeyCode::Digit2, Buttons::MAGIC),
            (KeyCode::Digit3, Buttons::DYNAMITE),
        ] {
            assert_eq!(b(k), want, "{k:?}");
        }
        assert_eq!(
            map.buttons(),
            Buttons::LEFT
                | Buttons::RIGHT
                | Buttons::UP
                | Buttons::DOWN
                | Buttons::JUMP
                | Buttons::ATTACK
                | Buttons::PROJECTILE
                | Buttons::TOGGLE
                | Buttons::LIFT
                | Buttons::PISTOL
                | Buttons::MAGIC
                | Buttons::DYNAMITE
        );
        assert!(map.help().contains("Ctrl attacks"));
        assert_eq!(
            map.action(KeyCode::Escape),
            Some(Action::Command(Command::Quit))
        );
        assert_eq!(map.action(KeyCode::KeyQ), None);
    }
}
