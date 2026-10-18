//! The keyboard and gamepad maps: physical keys and pad buttons to game
//! buttons and commands.
//!
//! The default keyboard map is the original game's: the arrow keys move,
//! climb, crouch and look up, Space jumps, Ctrl attacks, Alt fires the
//! selected weapon, Shift switches weapons, Z lifts and throws, and 1, 2
//! and 3 select the pistol, magic and dynamite; the left and right Ctrl,
//! Alt and Shift keys both work. Escape opens the in-game menu, the Pause
//! key pauses play without a menu (each press steps out of it one tick at
//! a time, see [`crate::pause`]) and F1 shows the help screen. With a
//! play-test aid on, F6 gives Claw the next powerup. Keys are
//! matched by their physical position, so the map works on any keyboard
//! layout.
//!
//! The default pad map follows the original joystick buttons 1 to 9 on a
//! standard gamepad (the Steam Deck's included):
//!
//! | Pad button | Original button | Action |
//! | --- | --- | --- |
//! | South (A) | 1 | jump |
//! | East (B) | 2 | attack |
//! | West (X) | 3 | fire the weapon |
//! | North (Y) | 4 | switch weapons |
//! | left shoulder | 5 | lift and throw |
//! | right shoulder | 6 | pistol |
//! | left trigger | 7 | magic |
//! | right trigger | 8 | dynamite |
//! | Start | 9 | the in-game menu |
//! | d-pad and left stick | directions | move, climb, crouch, look up |
//!
//! Every action can be bound to any keys and pad buttons. Each key has a
//! stable name (`ArrowLeft`, `KeyZ`, `Digit1`, `ControlLeft`, ...), the
//! same in the config file and in messages. A key belongs to one action at
//! a time: binding a key that another game action holds swaps the two,
//! and the keys of the commands (quit, pause, the display keys) are
//! reserved, so a binding that would take one is refused.

use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Result, anyhow, bail};
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
    /// Pause or resume without a menu (a gamepad's Start opens the
    /// in-game menu instead, see [`crate::pause::game_key`]).
    Pause,
    /// Show the help screen.
    Help,
    /// Play-test aid: give Claw the next powerup (see
    /// [`crate::session::Session::cycle_powerup`]); only while a play-test
    /// aid is on.
    CyclePowerup,
}

/// A key's meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A game button, held while the key is down.
    Button(Buttons),
    /// A command, run when the key goes down.
    Command(Command),
}

/// Every bindable action with its stable name, in the order the config
/// file lists them.
pub const ACTIONS: [(&str, Action); 21] = [
    ("left", Action::Button(Buttons::LEFT)),
    ("right", Action::Button(Buttons::RIGHT)),
    ("up", Action::Button(Buttons::UP)),
    ("down", Action::Button(Buttons::DOWN)),
    ("jump", Action::Button(Buttons::JUMP)),
    ("attack", Action::Button(Buttons::ATTACK)),
    ("projectile", Action::Button(Buttons::PROJECTILE)),
    ("toggle", Action::Button(Buttons::TOGGLE)),
    ("lift", Action::Button(Buttons::LIFT)),
    ("pistol", Action::Button(Buttons::PISTOL)),
    ("magic", Action::Button(Buttons::MAGIC)),
    ("dynamite", Action::Button(Buttons::DYNAMITE)),
    ("quit", Action::Command(Command::Quit)),
    ("fullscreen", Action::Command(Command::ToggleFullscreen)),
    ("window_size", Action::Command(Command::NextWindowSize)),
    ("aspect", Action::Command(Command::NextAspect)),
    ("scale", Action::Command(Command::ToggleScale)),
    (
        "interpolation",
        Action::Command(Command::ToggleInterpolation),
    ),
    ("pause", Action::Command(Command::Pause)),
    ("help", Action::Command(Command::Help)),
    ("powerup", Action::Command(Command::CyclePowerup)),
];

impl Action {
    /// The action's stable name, as in `key_<name>` and `pad_<name>`.
    pub fn name(self) -> Option<&'static str> {
        ACTIONS.iter().find(|(_, a)| *a == self).map(|(n, _)| *n)
    }

    /// The action named `name`.
    pub fn from_name(name: &str) -> Option<Action> {
        ACTIONS.iter().find(|(n, _)| *n == name).map(|(_, a)| *a)
    }

    /// True for the commands: their keys cannot be taken by another
    /// binding.
    pub fn is_reserved(self) -> bool {
        matches!(self, Action::Command(_))
    }
}

macro_rules! key_names {
    ($($k:ident)*) => {
        /// Every key with its stable name.
        pub const KEY_NAMES: &[(KeyCode, &str)] = &[$((KeyCode::$k, stringify!($k)),)*];
    };
}

key_names! {
    Backquote Backslash BracketLeft BracketRight Comma Digit0 Digit1 Digit2 Digit3 Digit4
    Digit5 Digit6 Digit7 Digit8 Digit9 Equal IntlBackslash IntlRo IntlYen KeyA KeyB KeyC KeyD
    KeyE KeyF KeyG KeyH KeyI KeyJ KeyK KeyL KeyM KeyN KeyO KeyP KeyQ KeyR KeyS KeyT KeyU KeyV
    KeyW KeyX KeyY KeyZ Minus Period Quote Semicolon Slash AltLeft AltRight Backspace CapsLock
    ContextMenu ControlLeft ControlRight Enter SuperLeft SuperRight ShiftLeft ShiftRight Space
    Tab Convert KanaMode Lang1 Lang2 Lang3 Lang4 Lang5 NonConvert Delete End Help Home Insert
    PageDown PageUp ArrowDown ArrowLeft ArrowRight ArrowUp NumLock Numpad0 Numpad1 Numpad2
    Numpad3 Numpad4 Numpad5 Numpad6 Numpad7 Numpad8 Numpad9 NumpadAdd NumpadBackspace
    NumpadClear NumpadClearEntry NumpadComma NumpadDecimal NumpadDivide NumpadEnter
    NumpadEqual NumpadHash NumpadMemoryAdd NumpadMemoryClear NumpadMemoryRecall
    NumpadMemoryStore NumpadMemorySubtract NumpadMultiply NumpadParenLeft NumpadParenRight
    NumpadStar NumpadSubtract Escape Fn FnLock PrintScreen ScrollLock Pause BrowserBack
    BrowserFavorites BrowserForward BrowserHome BrowserRefresh BrowserSearch BrowserStop Eject
    LaunchApp1 LaunchApp2 LaunchMail MediaPlayPause MediaSelect MediaStop MediaTrackNext
    MediaTrackPrevious Power Sleep AudioVolumeDown AudioVolumeMute AudioVolumeUp WakeUp Meta
    Hyper Turbo Abort Resume Suspend Again Copy Cut Find Open Paste Props Select Undo Hiragana
    Katakana F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18 F19 F20 F21 F22
    F23 F24 F25 F26 F27 F28 F29 F30 F31 F32 F33 F34 F35
}

/// The stable name of `key`.
pub fn key_name(key: KeyCode) -> Option<&'static str> {
    KEY_NAMES.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// The key named `name`: a stable name, ignoring case, or one of the short
/// forms `A`..`Z`, `0`..`9`, `Left`, `Right`, `Up`, `Down`, `Ctrl`, `Alt`
/// and `Shift` (the left-hand keys).
pub fn parse_key(name: &str) -> Option<KeyCode> {
    let name = name.trim();
    if let Some(&(k, _)) = KEY_NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)) {
        return Some(k);
    }
    let lower = name.to_ascii_lowercase();
    let alias = match lower.as_str() {
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "ctrl" | "control" => "ControlLeft",
        "alt" => "AltLeft",
        "shift" => "ShiftLeft",
        "esc" => "Escape",
        "return" => "Enter",
        _ => {
            let mut chars = lower.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_lowercase() => {
                    parse_key(&format!("Key{}", c.to_ascii_uppercase()))
                }
                (Some(c), None) if c.is_ascii_digit() => parse_key(&format!("Digit{c}")),
                _ => None,
            };
        }
    };
    parse_key(alias)
}

/// Why a binding was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// The key belongs to a command, which keeps it.
    Reserved {
        /// The command holding the key.
        holder: Action,
    },
    /// The action cannot be bound (a button set with no name).
    Unknown,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Conflict::Reserved { holder } => write!(
                f,
                "that key is reserved for {}",
                holder.name().unwrap_or("a command")
            ),
            Conflict::Unknown => f.write_str("that action cannot be bound"),
        }
    }
}

impl std::error::Error for Conflict {}

/// What a binding changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rebound<K> {
    /// Another action that held the new key, and the key it got instead
    /// (the bound action's old key), if any.
    pub swapped: Option<(Action, Option<K>)>,
}

/// A many-to-one map from inputs (keys or pad buttons) to actions, with
/// the binding rules shared by the keyboard and the pad.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bindings<K> {
    entries: Vec<(K, Action)>,
}

/// The position of `action` in [`ACTIONS`].
fn action_index(action: Action) -> usize {
    ACTIONS
        .iter()
        .position(|(_, a)| *a == action)
        .unwrap_or(ACTIONS.len())
}

impl<K: Copy + PartialEq> Bindings<K> {
    /// Keeps the entries in [`ACTIONS`] order, each action's inputs in the
    /// order they were bound, so equal maps compare equal.
    fn normalize(&mut self) {
        self.entries.sort_by_key(|&(_, a)| action_index(a));
    }

    fn action(&self, key: K) -> Option<Action> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|&(_, a)| a)
    }

    fn keys(&self, action: Action) -> Vec<K> {
        self.entries
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|&(k, _)| k)
            .collect()
    }

    fn buttons(&self) -> Buttons {
        self.entries
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Button(b) => Some(*b),
                Action::Command(_) => None,
            })
            .fold(Buttons::NONE, |acc, b| acc | b)
    }

    fn rebind(&mut self, action: Action, old: Option<K>, new: K) -> Result<Rebound<K>, Conflict> {
        if action.name().is_none() {
            return Err(Conflict::Unknown);
        }
        let old = old.filter(|o| self.action(*o) == Some(action));
        let holder = self.action(new);
        match holder {
            Some(h) if h == action => {
                // Already bound: just drop the replaced key.
                if let Some(o) = old.filter(|o| *o != new) {
                    self.entries.retain(|(k, _)| *k != o);
                }
                return Ok(Rebound { swapped: None });
            }
            Some(h) if h.is_reserved() => return Err(Conflict::Reserved { holder: h }),
            _ => {}
        }
        let mut swapped = None;
        if let Some(h) = holder {
            // Swap: the holder gets the replaced key, or loses the key.
            self.entries.retain(|(k, _)| *k != new);
            if let Some(o) = old {
                self.entries.retain(|(k, _)| *k != o);
                self.entries.push((o, h));
            }
            swapped = Some((h, old));
        } else if let Some(o) = old {
            self.entries.retain(|(k, _)| *k != o);
        }
        self.entries.push((new, action));
        self.normalize();
        Ok(Rebound { swapped })
    }

    fn unbind(&mut self, key: K) -> Option<Action> {
        let a = self.action(key);
        self.entries.retain(|(k, _)| *k != key);
        a
    }

    /// Applies the explicit `(action, keys)` sets over these (default)
    /// bindings: explicit sets win; a default binding whose key an explicit
    /// set takes is dropped, unless it is a reserved one.
    fn apply_sets(&mut self, sets: &[(Action, Vec<K>)], name: impl Fn(K) -> String) -> Result<()> {
        let mut taken: Vec<(K, Action)> = Vec::new();
        for (action, keys) in sets {
            for &k in keys {
                if let Some(&(_, other)) = taken.iter().find(|(t, _)| *t == k) {
                    if other != *action {
                        bail!(
                            "{} is bound to both {} and {}",
                            name(k),
                            other.name().unwrap_or("?"),
                            action.name().unwrap_or("?")
                        );
                    }
                    continue;
                }
                taken.push((k, *action));
            }
        }
        let explicit: Vec<Action> = sets.iter().map(|(a, _)| *a).collect();
        for &(k, a) in &taken {
            if let Some(holder) = self.action(k)
                && holder != a
                && !explicit.contains(&holder)
                && holder.is_reserved()
            {
                bail!(
                    "{} is reserved for {}; bind {} to another key first",
                    name(k),
                    holder.name().unwrap_or("a command"),
                    holder.name().unwrap_or("it")
                );
            }
        }
        self.entries
            .retain(|(k, a)| !explicit.contains(a) && !taken.iter().any(|(t, _)| t == k));
        self.entries.extend(taken);
        self.normalize();
        Ok(())
    }
}

/// The keyboard map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    map: Bindings<KeyCode>,
}

/// The default keyboard bindings.
pub const DEFAULT_KEYS: [(KeyCode, Action); 24] = {
    use Action::{Button as B, Command as C};
    [
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
        (KeyCode::F1, C(Command::Help)),
        (KeyCode::F6, C(Command::CyclePowerup)),
    ]
};

const DEFAULT_HELP: &str = "arrows move, climb, crouch and look up; Space jumps; Ctrl attacks; \
     Alt fires the weapon; Shift switches weapons; Z lifts and throws; \
     1, 2, 3 select pistol, magic, dynamite; Pause pauses (each press steps one tick); \
     F1 help; F6 next powerup (with a play-test aid on); F7 interpolation; \
     F8 fit or integer scaling; F9 aspect (fill, centered, 4:3); F10 window size; F11 fullscreen; Escape opens the menu (quits without menus)";

impl Default for Keymap {
    fn default() -> Self {
        Keymap {
            map: Bindings {
                entries: DEFAULT_KEYS.to_vec(),
            },
        }
    }
}

/// Splits a binding list (`"ArrowLeft, KeyA"`); `none` or nothing is an
/// empty list.
fn split_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("none"))
}

/// The `(action, value)` pairs of config `values` whose keys start with
/// `prefix`.
fn config_sets<'a>(
    values: &'a BTreeMap<String, String>,
    prefix: &str,
) -> Result<Vec<(Action, &'a str, &'a str)>> {
    let mut out = Vec::new();
    for (k, v) in values {
        let Some(name) = k.strip_prefix(prefix) else {
            continue;
        };
        if prefix == "pad_" && name == "dead_zone" {
            continue;
        }
        let action = Action::from_name(name).ok_or_else(|| anyhow!("{k}: unknown action"))?;
        out.push((action, k.as_str(), v.as_str()));
    }
    Ok(out)
}

impl Keymap {
    /// What `key` does, if anything.
    pub fn action(&self, key: KeyCode) -> Option<Action> {
        self.map.action(key)
    }

    /// The keys bound to `action`.
    pub fn keys(&self, action: Action) -> Vec<KeyCode> {
        self.map.keys(action)
    }

    /// Every game button the map can produce.
    pub fn buttons(&self) -> Buttons {
        self.map.buttons()
    }

    /// Binds `new` to `action`, replacing its key `old` (or adding a key
    /// when `old` is `None`). When another game action holds `new`, the two
    /// swap: that action gets `old`, or loses the key when `old` is `None`.
    /// A key a command holds is refused.
    pub fn rebind(
        &mut self,
        action: Action,
        old: Option<KeyCode>,
        new: KeyCode,
    ) -> Result<Rebound<KeyCode>, Conflict> {
        self.map.rebind(action, old, new)
    }

    /// Removes `key` from its action; returns the action it had.
    pub fn unbind(&mut self, key: KeyCode) -> Option<Action> {
        self.map.unbind(key)
    }

    /// Back to the default map.
    pub fn reset(&mut self) {
        *self = Keymap::default();
    }

    /// The config value of `action`'s keys: names joined by `, `, or
    /// `none`.
    pub fn config_value(&self, action: Action) -> String {
        let names: Vec<&str> = self.keys(action).into_iter().filter_map(key_name).collect();
        if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        }
    }

    /// Every `key_<action>` setting of this map.
    pub fn to_config(&self) -> Vec<(String, String)> {
        ACTIONS
            .iter()
            .map(|&(n, a)| (format!("key_{n}"), self.config_value(a)))
            .collect()
    }

    /// The default map with the `key_<action>` settings of `values` over
    /// it. A key named twice, an unknown key name, or a key a command keeps
    /// is an error.
    pub fn from_config(values: &BTreeMap<String, String>) -> Result<Keymap> {
        let mut sets = Vec::new();
        for (action, k, v) in config_sets(values, "key_")? {
            let mut keys = Vec::new();
            for name in split_list(v) {
                keys.push(parse_key(name).ok_or_else(|| anyhow!("{k}: unknown key {name:?}"))?);
            }
            sets.push((action, keys));
        }
        let mut map = Keymap::default();
        map.map
            .apply_sets(&sets, |k| key_name(k).unwrap_or("?").to_string())?;
        Ok(map)
    }

    /// Help text: one line naming every key.
    pub fn help(&self) -> String {
        if *self == Keymap::default() {
            return DEFAULT_HELP.to_string();
        }
        ACTIONS
            .iter()
            .map(|&(n, a)| format!("{n}: {}", self.config_value(a)))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// A button of a standard gamepad (the layout of an Xbox-style pad, which
/// the Steam Deck's controls also present).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PadButton {
    /// Bottom face button (A, Cross).
    South,
    /// Right face button (B, Circle).
    East,
    /// Left face button (X, Square).
    West,
    /// Top face button (Y, Triangle).
    North,
    /// Left shoulder button (LB, L1).
    LeftShoulder,
    /// Right shoulder button (RB, R1).
    RightShoulder,
    /// Left trigger (LT, L2).
    LeftTrigger,
    /// Right trigger (RT, R2).
    RightTrigger,
    /// Select or Back (View).
    Select,
    /// Start (Menu).
    Start,
    /// The logo button.
    Mode,
    /// Left stick pressed.
    LeftStick,
    /// Right stick pressed.
    RightStick,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
}

/// Every pad button with its stable name.
pub const PAD_BUTTON_NAMES: [(PadButton, &str); 17] = [
    (PadButton::South, "South"),
    (PadButton::East, "East"),
    (PadButton::West, "West"),
    (PadButton::North, "North"),
    (PadButton::LeftShoulder, "LeftShoulder"),
    (PadButton::RightShoulder, "RightShoulder"),
    (PadButton::LeftTrigger, "LeftTrigger"),
    (PadButton::RightTrigger, "RightTrigger"),
    (PadButton::Select, "Select"),
    (PadButton::Start, "Start"),
    (PadButton::Mode, "Mode"),
    (PadButton::LeftStick, "LeftStick"),
    (PadButton::RightStick, "RightStick"),
    (PadButton::DPadUp, "DPadUp"),
    (PadButton::DPadDown, "DPadDown"),
    (PadButton::DPadLeft, "DPadLeft"),
    (PadButton::DPadRight, "DPadRight"),
];

impl PadButton {
    /// The button's stable name.
    pub fn name(self) -> &'static str {
        PAD_BUTTON_NAMES
            .iter()
            .find(|(b, _)| *b == self)
            .map_or("?", |(_, n)| n)
    }

    /// The button named `name`, ignoring case.
    pub fn from_name(name: &str) -> Option<PadButton> {
        PAD_BUTTON_NAMES
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name.trim()))
            .map(|(b, _)| *b)
    }

    /// The original joystick button number this button stands for: 1 to 4
    /// the face buttons, 5 and 6 the shoulders, 7 and 8 the triggers and 9
    /// Start.
    pub fn number(self) -> Option<u8> {
        Some(match self {
            PadButton::South => 1,
            PadButton::East => 2,
            PadButton::West => 3,
            PadButton::North => 4,
            PadButton::LeftShoulder => 5,
            PadButton::RightShoulder => 6,
            PadButton::LeftTrigger => 7,
            PadButton::RightTrigger => 8,
            PadButton::Start => 9,
            _ => return None,
        })
    }
}

/// The default pad bindings; the left stick also moves.
pub const DEFAULT_PAD: [(PadButton, Action); 13] = {
    use Action::{Button as B, Command as C};
    [
        (PadButton::DPadLeft, B(Buttons::LEFT)),
        (PadButton::DPadRight, B(Buttons::RIGHT)),
        (PadButton::DPadUp, B(Buttons::UP)),
        (PadButton::DPadDown, B(Buttons::DOWN)),
        (PadButton::South, B(Buttons::JUMP)),
        (PadButton::East, B(Buttons::ATTACK)),
        (PadButton::West, B(Buttons::PROJECTILE)),
        (PadButton::North, B(Buttons::TOGGLE)),
        (PadButton::LeftShoulder, B(Buttons::LIFT)),
        (PadButton::RightShoulder, B(Buttons::PISTOL)),
        (PadButton::LeftTrigger, B(Buttons::MAGIC)),
        (PadButton::RightTrigger, B(Buttons::DYNAMITE)),
        (PadButton::Start, C(Command::Pause)),
    ]
};

/// The gamepad map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadMap {
    map: Bindings<PadButton>,
}

impl Default for PadMap {
    fn default() -> Self {
        PadMap {
            map: Bindings {
                entries: DEFAULT_PAD.to_vec(),
            },
        }
    }
}

impl PadMap {
    /// What `button` does, if anything.
    pub fn action(&self, button: PadButton) -> Option<Action> {
        self.map.action(button)
    }

    /// The pad buttons bound to `action`.
    pub fn buttons_of(&self, action: Action) -> Vec<PadButton> {
        self.map.keys(action)
    }

    /// Every game button the map can produce.
    pub fn buttons(&self) -> Buttons {
        self.map.buttons()
    }

    /// Binds `new` to `action` with the keyboard's rules (see
    /// [`Keymap::rebind`]).
    pub fn rebind(
        &mut self,
        action: Action,
        old: Option<PadButton>,
        new: PadButton,
    ) -> Result<Rebound<PadButton>, Conflict> {
        self.map.rebind(action, old, new)
    }

    /// Removes `button` from its action; returns the action it had.
    pub fn unbind(&mut self, button: PadButton) -> Option<Action> {
        self.map.unbind(button)
    }

    /// Back to the default map.
    pub fn reset(&mut self) {
        *self = PadMap::default();
    }

    /// The config value of `action`'s pad buttons, or `none`.
    pub fn config_value(&self, action: Action) -> String {
        let names: Vec<&str> = self
            .buttons_of(action)
            .into_iter()
            .map(PadButton::name)
            .collect();
        if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        }
    }

    /// Every `pad_<action>` setting of this map.
    pub fn to_config(&self) -> Vec<(String, String)> {
        ACTIONS
            .iter()
            .map(|&(n, a)| (format!("pad_{n}"), self.config_value(a)))
            .collect()
    }

    /// The default map with the `pad_<action>` settings of `values` over
    /// it (`pad_dead_zone` is not a binding and is skipped).
    pub fn from_config(values: &BTreeMap<String, String>) -> Result<PadMap> {
        let mut sets = Vec::new();
        for (action, k, v) in config_sets(values, "pad_")? {
            let mut buttons = Vec::new();
            for name in split_list(v) {
                buttons.push(
                    PadButton::from_name(name)
                        .ok_or_else(|| anyhow!("{k}: unknown pad button {name:?}"))?,
                );
            }
            sets.push((action, buttons));
        }
        let mut map = PadMap::default();
        map.map.apply_sets(&sets, |b| b.name().to_string())?;
        Ok(map)
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

    #[test]
    fn every_key_has_a_unique_name() {
        assert_eq!(KEY_NAMES.len(), 194);
        for (i, &(k, n)) in KEY_NAMES.iter().enumerate() {
            assert_eq!(parse_key(n), Some(k), "{n}");
            assert_eq!(parse_key(&n.to_ascii_lowercase()), Some(k), "{n}");
            assert_eq!(key_name(k), Some(n));
            assert!(KEY_NAMES[..i].iter().all(|&(o, m)| o != k && m != n));
        }
        assert_eq!(parse_key("z"), Some(KeyCode::KeyZ));
        assert_eq!(parse_key("7"), Some(KeyCode::Digit7));
        assert_eq!(parse_key("Left"), Some(KeyCode::ArrowLeft));
        assert_eq!(parse_key("ctrl"), Some(KeyCode::ControlLeft));
        assert_eq!(parse_key("nope"), None);
        assert_eq!(parse_key(""), None);
    }

    #[test]
    fn the_powerup_key_is_f6_and_f9_stays_the_aspect_key() {
        let map = Keymap::default();
        assert_eq!(
            map.action(KeyCode::F6),
            Some(Action::Command(Command::CyclePowerup))
        );
        assert_eq!(
            map.action(KeyCode::F9),
            Some(Action::Command(Command::NextAspect))
        );
        assert_eq!(
            Action::from_name("powerup"),
            Some(Action::Command(Command::CyclePowerup))
        );
        // No other default binding takes F6, and it is reserved.
        let f6 = DEFAULT_KEYS
            .iter()
            .filter(|(k, _)| *k == KeyCode::F6)
            .count();
        assert_eq!(f6, 1);
        assert!(Action::Command(Command::CyclePowerup).is_reserved());
        assert!(map.help().contains("F6 next powerup"));
    }
}
