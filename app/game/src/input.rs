//! Input routing: every key the window receives, and every input device
//! polled each frame, goes through the [`InputRouter`], which turns it into
//! a game button, a command or a menu input. The window acts on what comes
//! out and knows nothing about keys or pads itself.
//!
//! Keys go through the keyboard map ([`crate::keymap`]). Gamepads are read
//! in [`InputRouter::poll`], which the window calls once per frame:
//!
//! - pads can be plugged in and out at any time, and several pads act as
//!   one;
//! - the d-pad and the left stick move; the stick has a radial dead zone
//!   (by default 0.34 of a full deflection, the original joystick's feel)
//!   and points in eight directions;
//! - the buttons follow the pad map (by default the original joystick
//!   layout, see [`crate::keymap`]), and Start pauses;
//! - a pad or key held together with another source of the same button
//!   releases the button only when both are up;
//! - when the window loses focus every pad button is released, and nothing
//!   from a pad counts until the window has focus again;
//! - without gamepad support on the system, or with no pad plugged in, the
//!   game says so once and plays with the keyboard.
//!
//! In menus ([`InputContext::Menu`]) the same inputs become menu inputs:
//! the arrows, the d-pad and the stick move, Enter, Space, the jump keys,
//! the South button and Start choose, and Escape, Backspace and the East
//! button go back. Leaving play releases every game button; when play
//! comes back, the router either counts the keys still down as held again
//! (with [`InputRouter::set_keep_held`], the original rule: they are held,
//! not pressed) or waits for them to be pressed again.
//! [`InputRouter::buttons_down`] says which game buttons the keys and pads
//! hold at any time, menus included. [`InputRouter::capture_next_key`] makes the next key
//! (or pad button) pressed the new binding of an action.
//!
//! [`Recorder`] keeps the input of every game tick a session plays, so a
//! game can be written out as an input script and replayed.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::rc::Rc;
use std::time::Instant;

use claw_frame::{Buttons, TickInput};
use claw_sim::InputScript;
use winit::keyboard::KeyCode;

use crate::keymap::{Action, Command, Conflict, Keymap, PadButton, PadMap};
use crate::session::{BeforeTick, Session};
use crate::settings::{PadSettings, Settings};

/// A menu action from a key or a controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    /// Move up.
    Up,
    /// Move down.
    Down,
    /// Move left or decrease.
    Left,
    /// Move right or increase.
    Right,
    /// Choose.
    Confirm,
    /// Go back.
    Back,
}

/// What an input means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// Game buttons went down (`true`) or up.
    Button(Buttons, bool),
    /// A command, run once.
    Command(Command),
    /// A menu action.
    Menu(MenuInput),
}

/// Where inputs go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputContext {
    /// Game buttons and commands.
    #[default]
    Play,
    /// Menu inputs and commands.
    Menu,
}

/// A stick axis of a gamepad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadAxis {
    /// Left stick, -1 left to 1 right.
    LeftX,
    /// Left stick, -1 down to 1 up.
    LeftY,
}

/// What happened on a gamepad.
#[derive(Debug, Clone, PartialEq)]
pub enum PadEventKind {
    /// The pad was plugged in; its name.
    Connected(String),
    /// The pad was unplugged.
    Disconnected,
    /// A button went down (`true`) or up.
    Button(PadButton, bool),
    /// A stick axis moved to this position.
    Axis(PadAxis, f32),
}

/// An event of one gamepad.
#[derive(Debug, Clone, PartialEq)]
pub struct PadEvent {
    /// Which pad: a number that stays the same while it is plugged in.
    pub pad: usize,
    /// What happened.
    pub kind: PadEventKind,
}

/// A source of gamepad events: the system's gamepads, or a script in
/// tests.
pub trait PadSource {
    /// The next waiting event, or `None` when there is none.
    fn next_event(&mut self) -> Option<PadEvent>;
}

/// The system's gamepads, through gilrs (the standard gamepad layout with
/// the community mapping database, hot-plug included).
pub struct GilrsSource {
    gilrs: gilrs::Gilrs,
    pending: VecDeque<PadEvent>,
}

impl fmt::Debug for GilrsSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GilrsSource")
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl GilrsSource {
    /// Opens the system's gamepads; the pads already plugged in come first
    /// as connections.
    pub fn open() -> Result<GilrsSource, String> {
        let gilrs = gilrs::Gilrs::new().map_err(|e| e.to_string())?;
        let pending = gilrs
            .gamepads()
            .map(|(id, pad)| PadEvent {
                pad: usize::from(id),
                kind: PadEventKind::Connected(pad.name().to_string()),
            })
            .collect();
        Ok(GilrsSource { gilrs, pending })
    }

    fn button(b: gilrs::Button) -> Option<PadButton> {
        use gilrs::Button as G;
        Some(match b {
            G::South => PadButton::South,
            G::East => PadButton::East,
            G::West => PadButton::West,
            G::North => PadButton::North,
            G::LeftTrigger => PadButton::LeftShoulder,
            G::RightTrigger => PadButton::RightShoulder,
            G::LeftTrigger2 => PadButton::LeftTrigger,
            G::RightTrigger2 => PadButton::RightTrigger,
            G::Select => PadButton::Select,
            G::Start => PadButton::Start,
            G::Mode => PadButton::Mode,
            G::LeftThumb => PadButton::LeftStick,
            G::RightThumb => PadButton::RightStick,
            G::DPadUp => PadButton::DPadUp,
            G::DPadDown => PadButton::DPadDown,
            G::DPadLeft => PadButton::DPadLeft,
            G::DPadRight => PadButton::DPadRight,
            _ => return None,
        })
    }
}

impl PadSource for GilrsSource {
    fn next_event(&mut self) -> Option<PadEvent> {
        use gilrs::EventType as E;
        if let Some(e) = self.pending.pop_front() {
            return Some(e);
        }
        while let Some(ev) = self.gilrs.next_event() {
            let pad = usize::from(ev.id);
            let kind = match ev.event {
                E::Connected => {
                    PadEventKind::Connected(self.gilrs.gamepad(ev.id).name().to_string())
                }
                E::Disconnected => PadEventKind::Disconnected,
                E::ButtonPressed(b, _) => match GilrsSource::button(b) {
                    Some(b) => PadEventKind::Button(b, true),
                    None => continue,
                },
                E::ButtonReleased(b, _) => match GilrsSource::button(b) {
                    Some(b) => PadEventKind::Button(b, false),
                    None => continue,
                },
                E::AxisChanged(gilrs::Axis::LeftStickX, v, _) => {
                    PadEventKind::Axis(PadAxis::LeftX, v)
                }
                E::AxisChanged(gilrs::Axis::LeftStickY, v, _) => {
                    PadEventKind::Axis(PadAxis::LeftY, v)
                }
                _ => continue,
            };
            return Some(PadEvent { pad, kind });
        }
        None
    }
}

/// sin(22.5 degrees): a stick points in a direction when it is within
/// 67.5 degrees of it, which splits the circle into eight equal sectors.
const SECTOR: f32 = 0.382_683_43;

/// The directions the stick at `(x, y)` points in (`y` up): none inside
/// the radial `dead_zone`, else one or two of the eight directions.
pub fn stick_buttons(x: f32, y: f32, dead_zone: f32) -> Buttons {
    let clean = |v: f32| {
        if v.is_finite() {
            v.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };
    let (x, y) = (clean(x), clean(y));
    let r = x.hypot(y);
    if r == 0.0 || r < dead_zone {
        return Buttons::NONE;
    }
    let (nx, ny) = (x / r, y / r);
    let mut b = Buttons::NONE;
    if nx > SECTOR {
        b |= Buttons::RIGHT;
    }
    if nx < -SECTOR {
        b |= Buttons::LEFT;
    }
    if ny > SECTOR {
        b |= Buttons::UP;
    }
    if ny < -SECTOR {
        b |= Buttons::DOWN;
    }
    b
}

/// What a capture bound, or tried to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captured {
    /// A key.
    Key(KeyCode),
    /// A pad button.
    Pad(PadButton),
}

/// How a capture ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The input is now bound to the action; `swapped` names another
    /// action that held it and what it got instead.
    Bound {
        /// The action.
        action: Action,
        /// The new input.
        input: Captured,
        /// The action that held the input, and the input it got instead.
        swapped: Option<(Action, Option<Captured>)>,
    },
    /// The input could not be bound.
    Refused {
        /// The action.
        action: Action,
        /// The input pressed.
        input: Captured,
        /// Why.
        conflict: Conflict,
    },
    /// Escape was pressed: nothing changed.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capture {
    Key(Action, Option<KeyCode>),
    Pad(Action, Option<PadButton>),
}

/// One plugged-in pad.
#[derive(Debug, Clone, Default, PartialEq)]
struct PadState {
    name: String,
    held: BTreeSet<PadButton>,
    stick: (f32, f32),
}

enum Backend {
    /// Gamepads off.
    Off,
    /// Opened at the first poll.
    Auto,
    /// Reading.
    Open(Box<dyn PadSource>),
    /// No gamepad support on this system.
    Unavailable,
}

/// Most pad events handled in one poll; the rest wait for the next.
const MAX_EVENTS_PER_POLL: usize = 1024;

const DIRECTIONS: [(Buttons, MenuInput); 4] = [
    (Buttons::UP, MenuInput::Up),
    (Buttons::DOWN, MenuInput::Down),
    (Buttons::LEFT, MenuInput::Left),
    (Buttons::RIGHT, MenuInput::Right),
];

const ALL_DIRECTIONS: Buttons =
    Buttons(Buttons::UP.0 | Buttons::DOWN.0 | Buttons::LEFT.0 | Buttons::RIGHT.0);

/// Turns keys and controller input into game buttons, commands and menu
/// actions.
pub struct InputRouter {
    keymap: Keymap,
    pad: PadSettings,
    backend: Backend,
    context: InputContext,
    focused: bool,
    /// Keys down that hold game buttons.
    keys: Vec<(KeyCode, Buttons)>,
    /// Keys down that are bound to game buttons, whatever the context.
    held_keys: Vec<(KeyCode, Buttons)>,
    /// Keys still down when play comes back count as held.
    keep_held: bool,
    /// Keys down that hold menu inputs.
    menu_keys: Vec<(KeyCode, MenuInput)>,
    pads: BTreeMap<usize, PadState>,
    /// Game buttons the pads hold, as last reported.
    pad_out: Buttons,
    /// Directions the pads hold in a menu, as last reported.
    pad_menu: Buttons,
    capture: Option<Capture>,
    captured: Option<CaptureOutcome>,
    notices: Vec<String>,
    told_no_pad: bool,
}

impl fmt::Debug for InputRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let backend = match self.backend {
            Backend::Off => "off",
            Backend::Auto => "not opened",
            Backend::Open(_) => "open",
            Backend::Unavailable => "unavailable",
        };
        f.debug_struct("InputRouter")
            .field("keymap", &self.keymap)
            .field("pad", &self.pad)
            .field("gamepads", &backend)
            .field("context", &self.context)
            .field("focused", &self.focused)
            .field("pads", &self.pads)
            .finish_non_exhaustive()
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        InputRouter::new(Keymap::default())
    }
}

impl InputRouter {
    fn build(keymap: Keymap, pad: PadSettings, backend: Backend) -> InputRouter {
        InputRouter {
            keymap,
            pad,
            backend,
            context: InputContext::Play,
            focused: true,
            keys: Vec::new(),
            held_keys: Vec::new(),
            keep_held: false,
            menu_keys: Vec::new(),
            pads: BTreeMap::new(),
            pad_out: Buttons::NONE,
            pad_menu: Buttons::NONE,
            capture: None,
            captured: None,
            notices: Vec::new(),
            told_no_pad: false,
        }
    }

    /// A router over `keymap` with the default pad settings; the system's
    /// gamepads are opened at the first poll.
    pub fn new(keymap: Keymap) -> InputRouter {
        InputRouter::build(keymap, PadSettings::default(), Backend::Auto)
    }

    /// A router for the keyboard only.
    pub fn keyboard_only(keymap: Keymap) -> InputRouter {
        InputRouter::build(keymap, PadSettings::default(), Backend::Off)
    }

    /// A router with the key map and gamepad settings of `settings`.
    pub fn from_settings(settings: &Settings) -> InputRouter {
        let backend = if settings.pad.enabled {
            Backend::Auto
        } else {
            Backend::Off
        };
        InputRouter::build(settings.keymap.clone(), settings.pad.clone(), backend)
    }

    /// A router reading gamepads from `source` (a script in tests).
    pub fn with_source(
        keymap: Keymap,
        pad: PadSettings,
        source: Box<dyn PadSource>,
    ) -> InputRouter {
        InputRouter::build(keymap, pad, Backend::Open(source))
    }

    /// The keyboard map.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Replaces the keyboard map.
    pub fn set_keymap(&mut self, keymap: Keymap) {
        self.keymap = keymap;
    }

    /// The gamepad settings.
    pub fn pad_settings(&self) -> &PadSettings {
        &self.pad
    }

    /// Replaces the pad map and the dead zone (turning gamepads on or off
    /// takes effect at the next start).
    pub fn set_pad_map(&mut self, map: PadMap, dead_zone: f32) {
        self.pad.map = map;
        self.pad.dead_zone = dead_zone;
    }

    /// Messages about gamepads: support missing, pads plugged in or out.
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    fn notice(&mut self, message: String) {
        eprintln!("rusty-claw: {message}");
        self.notices.push(message);
    }

    /// Where inputs go now.
    pub fn context(&self) -> InputContext {
        self.context
    }

    /// With `keep` (the original rule, see [`crate::pause::PauseInput`]),
    /// keys bound to game buttons that are still down when play comes back
    /// from a menu are held in play again (the session counts them as held
    /// and not pressed); without it they count only once pressed again.
    pub fn set_keep_held(&mut self, keep: bool) {
        self.keep_held = keep;
    }

    /// The game buttons the keys and the pads hold down now, in play or in
    /// a menu.
    pub fn buttons_down(&self) -> Buttons {
        self.held_keys
            .iter()
            .fold(self.pad_buttons(), |acc, &(_, b)| acc | b)
    }

    /// Sends inputs to `context` from now on. Leaving play releases every
    /// game button held; buttons still held when play comes back count
    /// as held again with [`InputRouter::set_keep_held`], else only once
    /// pressed again.
    pub fn set_context(&mut self, context: InputContext) -> Vec<Routed> {
        let mut out = Vec::new();
        if context == self.context {
            return out;
        }
        match context {
            InputContext::Menu => {
                let held = self.keys_down() | self.pad_out;
                if !held.is_empty() {
                    out.push(Routed::Button(held, false));
                }
                self.keys.clear();
                self.pad_out = Buttons::NONE;
                self.pad_menu = Buttons(self.pad_buttons().0 & ALL_DIRECTIONS.0);
            }
            InputContext::Play => {
                self.menu_keys.clear();
                self.pad_menu = Buttons::NONE;
                self.pad_out = self.pad_buttons();
                if self.keep_held {
                    self.keys = self.held_keys.clone();
                }
            }
        }
        self.context = context;
        out
    }

    /// Tells the router whether the window has focus. Losing it releases
    /// every key and pad button; pads count again only from their next
    /// press once the window has focus.
    pub fn set_focused(&mut self, focused: bool) -> Vec<Routed> {
        let mut out = Vec::new();
        if focused == self.focused {
            return out;
        }
        self.focused = focused;
        if !focused {
            let held = self.keys_down() | self.pad_out;
            if self.context == InputContext::Play && !held.is_empty() {
                out.push(Routed::Button(held, false));
            }
            self.keys.clear();
            self.held_keys.clear();
            self.menu_keys.clear();
            for pad in self.pads.values_mut() {
                pad.held.clear();
                pad.stick = (0.0, 0.0);
            }
            self.pad_out = Buttons::NONE;
            self.pad_menu = Buttons::NONE;
        }
        out
    }

    /// Makes the next key pressed (or, with a pad, the next pad button,
    /// see [`InputRouter::capture_next_pad_button`]) the binding of
    /// `action`, replacing its key `replace` or adding a key. Escape
    /// cancels. The result comes from [`InputRouter::take_capture`].
    pub fn capture_next_key(&mut self, action: Action, replace: Option<KeyCode>) {
        self.capture = Some(Capture::Key(action, replace));
        self.captured = None;
    }

    /// Makes the next pad button pressed the binding of `action`,
    /// replacing `replace` or adding a button. Escape cancels.
    pub fn capture_next_pad_button(&mut self, action: Action, replace: Option<PadButton>) {
        self.capture = Some(Capture::Pad(action, replace));
        self.captured = None;
    }

    /// True while a capture waits for its input.
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// How the last capture ended, once.
    pub fn take_capture(&mut self) -> Option<CaptureOutcome> {
        self.captured.take()
    }

    fn capture_key(&mut self, capture: Capture, key: KeyCode) {
        self.captured = Some(match capture {
            _ if key == KeyCode::Escape => CaptureOutcome::Cancelled,
            Capture::Key(action, replace) => match self.keymap.rebind(action, replace, key) {
                Ok(r) => CaptureOutcome::Bound {
                    action,
                    input: Captured::Key(key),
                    swapped: r.swapped.map(|(a, k)| (a, k.map(Captured::Key))),
                },
                Err(conflict) => CaptureOutcome::Refused {
                    action,
                    input: Captured::Key(key),
                    conflict,
                },
            },
            Capture::Pad(..) => {
                // Waiting for a pad button; other keys do nothing.
                self.capture = Some(capture);
                return;
            }
        });
    }

    fn capture_pad(&mut self, action: Action, replace: Option<PadButton>, button: PadButton) {
        self.captured = Some(match self.pad.map.rebind(action, replace, button) {
            Ok(r) => CaptureOutcome::Bound {
                action,
                input: Captured::Pad(button),
                swapped: r.swapped.map(|(a, b)| (a, b.map(Captured::Pad))),
            },
            Err(conflict) => CaptureOutcome::Refused {
                action,
                input: Captured::Pad(button),
                conflict,
            },
        });
    }

    fn keys_down(&self) -> Buttons {
        self.keys.iter().fold(Buttons::NONE, |acc, &(_, b)| acc | b)
    }

    /// The menu input of `key`, if any.
    fn menu_key(&self, key: KeyCode) -> Option<MenuInput> {
        Some(match key {
            KeyCode::ArrowUp => MenuInput::Up,
            KeyCode::ArrowDown => MenuInput::Down,
            KeyCode::ArrowLeft => MenuInput::Left,
            KeyCode::ArrowRight => MenuInput::Right,
            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => MenuInput::Confirm,
            KeyCode::Escape | KeyCode::Backspace => MenuInput::Back,
            _ => match self.keymap.action(key)? {
                Action::Button(b) if b == Buttons::JUMP => MenuInput::Confirm,
                Action::Button(b) => DIRECTIONS.iter().find(|(d, _)| *d == b)?.1,
                Action::Command(_) => return None,
            },
        })
    }

    /// What `key` going down (`down`) or up means: a button changes either
    /// way; a command runs when its key goes down. A button another key or
    /// a pad still holds is not released.
    pub fn key(&mut self, key: KeyCode, down: bool) -> Option<Routed> {
        if down && let Some(capture) = self.capture.take() {
            self.capture_key(capture, key);
            return None;
        }
        if let Some(Action::Button(b)) = self.keymap.action(key) {
            self.held_keys.retain(|(k, _)| *k != key);
            if down {
                self.held_keys.push((key, b));
            }
        }
        if self.context == InputContext::Menu {
            if let Some(m) = self.menu_key(key) {
                self.menu_keys.retain(|(k, _)| *k != key);
                if down {
                    self.menu_keys.push((key, m));
                    return Some(Routed::Menu(m));
                }
                return None;
            }
            return match self.keymap.action(key)? {
                Action::Command(c) if down => Some(Routed::Command(c)),
                _ => None,
            };
        }
        match self.keymap.action(key)? {
            Action::Button(b) if down => {
                if !self.keys.iter().any(|(k, _)| *k == key) {
                    self.keys.push((key, b));
                }
                Some(Routed::Button(b, true))
            }
            Action::Button(b) => {
                self.keys.retain(|(k, _)| *k != key);
                let still = self.keys_down() | self.pad_out;
                let up = Buttons(b.0 & !still.0);
                (!up.is_empty()).then_some(Routed::Button(up, false))
            }
            Action::Command(c) if down => Some(Routed::Command(c)),
            Action::Command(_) => None,
        }
    }

    /// Directions the menu keys hold now, and the pads; for repeating a
    /// held direction in menus.
    pub fn menu_held(&self) -> Vec<MenuInput> {
        let mut out = Vec::new();
        for &(d, m) in &DIRECTIONS {
            if self.pad_menu.contains(d) || self.menu_keys.iter().any(|(_, k)| *k == m) {
                out.push(m);
            }
        }
        out
    }

    /// Game buttons the pads hold now: their bound buttons and the
    /// stick's directions.
    fn pad_buttons(&self) -> Buttons {
        let mut b = Buttons::NONE;
        for pad in self.pads.values() {
            for &button in &pad.held {
                if let Some(Action::Button(x)) = self.pad.map.action(button) {
                    b |= x;
                }
            }
            b |= stick_buttons(pad.stick.0, pad.stick.1, self.pad.dead_zone);
        }
        b
    }

    /// Reports what changed in the pads' merged state.
    fn sync_pads(&mut self, out: &mut Vec<Routed>) {
        let now = self.pad_buttons();
        match self.context {
            InputContext::Play => {
                let kb = self.keys_down();
                let up = Buttons(self.pad_out.0 & !now.0 & !kb.0);
                let down = Buttons(now.0 & !self.pad_out.0 & !kb.0);
                if !up.is_empty() {
                    out.push(Routed::Button(up, false));
                }
                if !down.is_empty() {
                    out.push(Routed::Button(down, true));
                }
                self.pad_out = now;
            }
            InputContext::Menu => {
                let dirs = Buttons(now.0 & ALL_DIRECTIONS.0);
                for &(d, m) in &DIRECTIONS {
                    if dirs.contains(d) && !self.pad_menu.contains(d) {
                        out.push(Routed::Menu(m));
                    }
                }
                self.pad_menu = dirs;
            }
        }
    }

    fn pad_event(&mut self, e: PadEvent, out: &mut Vec<Routed>) {
        match e.kind {
            PadEventKind::Connected(name) => {
                self.notice(format!("gamepad connected: {name}"));
                self.pads.insert(
                    e.pad,
                    PadState {
                        name,
                        ..PadState::default()
                    },
                );
            }
            PadEventKind::Disconnected => {
                if let Some(p) = self.pads.remove(&e.pad) {
                    self.notice(format!("gamepad disconnected: {}", p.name));
                }
            }
            _ if !self.focused => return,
            PadEventKind::Button(button, down) => {
                let pad = self.pads.entry(e.pad).or_default();
                let fresh = if down {
                    pad.held.insert(button)
                } else {
                    pad.held.remove(&button);
                    false
                };
                if fresh {
                    self.pad_press(button, out);
                }
            }
            PadEventKind::Axis(axis, v) => {
                let pad = self.pads.entry(e.pad).or_default();
                let v = if v.is_finite() {
                    v.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                match axis {
                    PadAxis::LeftX => pad.stick.0 = v,
                    PadAxis::LeftY => pad.stick.1 = v,
                }
            }
        }
        self.sync_pads(out);
    }

    /// The one-shot part of a pad button going down: a capture, a
    /// command, a menu choice.
    fn pad_press(&mut self, button: PadButton, out: &mut Vec<Routed>) {
        if let Some(Capture::Pad(action, replace)) = self.capture {
            self.capture = None;
            self.capture_pad(action, replace, button);
            // The captured press does not act.
            if let Some(Action::Button(b)) = self.pad.map.action(button) {
                self.pad_out |= b;
            }
            return;
        }
        if self.context == InputContext::Menu {
            match button {
                PadButton::South | PadButton::Start => out.push(Routed::Menu(MenuInput::Confirm)),
                PadButton::East => out.push(Routed::Menu(MenuInput::Back)),
                _ => {
                    if let Some(Action::Command(c)) = self.pad.map.action(button) {
                        out.push(Routed::Command(c));
                    }
                }
            }
            return;
        }
        if let Some(Action::Command(c)) = self.pad.map.action(button) {
            out.push(Routed::Command(c));
        }
    }

    fn open_pads(&mut self) {
        if !matches!(self.backend, Backend::Auto) {
            return;
        }
        self.backend = match GilrsSource::open() {
            Ok(source) => Backend::Open(Box::new(source)),
            Err(e) => {
                self.notice(format!(
                    "gamepads are not available ({e}); playing with the keyboard"
                ));
                Backend::Unavailable
            }
        };
    }

    /// Polls the gamepads; called once per frame. The first poll opens the
    /// system's gamepads unless they are off.
    pub fn poll(&mut self, now: Instant) -> Vec<Routed> {
        let _ = now;
        let mut out = Vec::new();
        self.open_pads();
        let Backend::Open(source) = &mut self.backend else {
            return out;
        };
        let mut events = Vec::new();
        while events.len() < MAX_EVENTS_PER_POLL {
            match source.next_event() {
                Some(e) => events.push(e),
                None => break,
            }
        }
        for e in events {
            self.pad_event(e, &mut out);
        }
        if !self.told_no_pad {
            self.told_no_pad = true;
            if self.pads.is_empty() {
                self.notice(
                    "no gamepad connected; playing with the keyboard (a gamepad can be \
                     plugged in at any time)"
                        .into(),
                );
            }
        }
        out
    }

    /// Help text for the controls.
    pub fn help(&self) -> String {
        self.keymap.help()
    }
}

/// Keeps the input of every tick a session plays.
///
/// Attach [`Recorder::hook`] to the session with
/// [`Session::set_before_tick`]: before each tick it reads the input the
/// simulation played on the tick before. [`Recorder::script`] adds the last
/// tick's, and the session's pauses, menus and commands as directives.
/// [`crate::session::save_recording`] writes the script to a file.
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    ticks: Rc<RefCell<Vec<TickInput>>>,
}

impl Recorder {
    /// An empty recording.
    pub fn new() -> Recorder {
        Recorder::default()
    }

    /// The session hook that records.
    pub fn hook(&self) -> BeforeTick {
        let ticks = Rc::clone(&self.ticks);
        Box::new(move |n, sim| {
            if n > 0 {
                ticks.borrow_mut().push(sim.input());
            }
        })
    }

    /// The input of every tick `session` has run, with its pauses, menus
    /// and commands (see [`Session::directives`]) and the header keys of
    /// the script it replayed, if any, as a script.
    pub fn script(&self, session: &Session) -> InputScript {
        let mut ticks = self.ticks.borrow().clone();
        if (ticks.len() as u64) < session.ticks() {
            ticks.push(session.sim().input());
        }
        let n = ticks.len();
        let mut directives = session.directives();
        directives.retain(|d| d.tick <= n);
        // The play-test aids the level was played with, so a replay plays
        // with them.
        let mut header = session.header();
        header.aids = session.sim().config().aids;
        header.powerup = session.sim().config().start_powerup;
        header.powerup_key = session.powerup_key_used();
        InputScript {
            ticks,
            header,
            directives,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use claw_frame::SimulatedRefresh;
    use claw_sim::{Sim, SimConfig, SimLevel, TileMap};

    use super::*;

    #[test]
    fn keys_route_to_buttons_and_commands() {
        let mut r = InputRouter::keyboard_only(Keymap::default());
        assert_eq!(
            r.key(KeyCode::ControlLeft, true),
            Some(Routed::Button(Buttons::ATTACK, true))
        );
        assert_eq!(
            r.key(KeyCode::ControlLeft, false),
            Some(Routed::Button(Buttons::ATTACK, false))
        );
        assert_eq!(
            r.key(KeyCode::Escape, true),
            Some(Routed::Command(Command::Quit))
        );
        assert_eq!(r.key(KeyCode::Escape, false), None);
        assert_eq!(r.key(KeyCode::KeyQ, true), None);
        assert!(r.poll(Instant::now()).is_empty());
        assert_eq!(r.help(), Keymap::default().help());
    }

    fn level() -> SimLevel {
        let mut rows = vec![".".repeat(120); 15];
        rows.push("#".repeat(120));
        rows.push("#".repeat(120));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
    }

    /// Key events: (ms, key, down).
    fn keys() -> Vec<(u64, KeyCode, bool)> {
        use KeyCode as K;
        vec![
            (100, K::ArrowRight, true),
            (900, K::Space, true),
            (1000, K::Space, false),
            (1500, K::ArrowRight, false),
            (1600, K::ControlLeft, true),
            (1610, K::ControlLeft, false),
            (2000, K::ArrowLeft, true),
            (2004, K::ShiftLeft, true),
            (2100, K::ShiftLeft, false),
            (2300, K::ArrowDown, true),
            (2600, K::ArrowLeft, false),
            (2700, K::ArrowDown, false),
            (2800, K::Digit3, true),
            (2810, K::Digit3, false),
        ]
    }

    #[test]
    fn a_recording_replays_into_the_same_trace() {
        let mut router = InputRouter::new(Keymap::default());
        let mut session = Session::new(Sim::new(level(), SimConfig::default()));
        session.record_trace();
        let recorder = Recorder::new();
        session.set_before_tick(recorder.hook());
        let mut refresh = SimulatedRefresh::new(144.0).with_jitter(0.2, 9);
        let mut clock_us = 0u64;
        let events = keys();
        let mut next = 0;
        while clock_us < 3_500_000 {
            let dt = refresh.next().unwrap();
            let end = clock_us + dt.as_micros() as u64;
            while let Some(&(ms, key, down)) = events.get(next) {
                if ms * 1000 > end {
                    break;
                }
                let at = session.stamp(Duration::from_micros(ms * 1000 - clock_us));
                if let Some(Routed::Button(b, d)) = router.key(key, down) {
                    session.key(at, b, d);
                }
                next += 1;
            }
            session.frame(dt);
            clock_us = end;
        }
        let script = recorder.script(&session);
        assert_eq!(script.ticks.len() as u64, session.ticks());
        assert!(
            script
                .ticks
                .iter()
                .any(|t| t.pressed.contains(Buttons::ATTACK))
        );
        let played: Vec<String> = session
            .trace()
            .iter()
            .map(|r| r.to_csv().replace(",keyboard,", ","))
            .collect();
        // Through the file format and back.
        let script = InputScript::parse(&script.to_csv()).unwrap();
        let mut replay = Session::new(Sim::new(level(), SimConfig::default())).with_replay(script);
        replay.record_trace();
        let mut refresh = SimulatedRefresh::new(60.0);
        while replay.replaying() {
            replay.frame(refresh.next().unwrap());
        }
        let replayed: Vec<String> = replay.trace()[..played.len()]
            .iter()
            .map(|r| r.to_csv().replace(",script,", ","))
            .collect();
        assert_eq!(replayed, played);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rec.csv");
        let n = crate::session::save_recording(&recorder, &session, &path).unwrap();
        assert_eq!(n as u64, session.ticks());
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(crate::session::script_behaviour(&text), Some(false));
        let back = InputScript::parse(&text).unwrap();
        assert_eq!(back, recorder.script(&session));
    }
}
