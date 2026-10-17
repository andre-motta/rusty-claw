//! The keyboard and pad maps, rebinding, and gamepads driven by a scripted
//! event source.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::time::Instant;

use claw_frame::Buttons;
use claw_game::input::{
    CaptureOutcome, Captured, InputContext, InputRouter, MenuInput, PadAxis, PadEvent,
    PadEventKind, PadSource, Routed, stick_buttons,
};
use claw_game::keymap::{
    ACTIONS, Action, Command, Conflict, KEY_NAMES, Keymap, PadButton, PadMap, key_name, parse_key,
};
use claw_game::settings::{PadSettings, Settings};
use winit::keyboard::KeyCode;

const B: fn(Buttons) -> Action = Action::Button;

/// The original's default keys, in its order: left, right, up, down,
/// jump, attack, projectile, toggle, lift, pistol, magic, dynamite.
const SPEC_KEYS: [(Buttons, &[KeyCode]); 12] = [
    (Buttons::LEFT, &[KeyCode::ArrowLeft]),
    (Buttons::RIGHT, &[KeyCode::ArrowRight]),
    (Buttons::UP, &[KeyCode::ArrowUp]),
    (Buttons::DOWN, &[KeyCode::ArrowDown]),
    (Buttons::JUMP, &[KeyCode::Space]),
    (
        Buttons::ATTACK,
        &[KeyCode::ControlLeft, KeyCode::ControlRight],
    ),
    (Buttons::PROJECTILE, &[KeyCode::AltLeft, KeyCode::AltRight]),
    (Buttons::TOGGLE, &[KeyCode::ShiftLeft, KeyCode::ShiftRight]),
    (Buttons::LIFT, &[KeyCode::KeyZ]),
    (Buttons::PISTOL, &[KeyCode::Digit1]),
    (Buttons::MAGIC, &[KeyCode::Digit2]),
    (Buttons::DYNAMITE, &[KeyCode::Digit3]),
];

/// The original joystick buttons 1 to 8.
const SPEC_PAD: [(u8, Buttons); 8] = [
    (1, Buttons::JUMP),
    (2, Buttons::ATTACK),
    (3, Buttons::PROJECTILE),
    (4, Buttons::TOGGLE),
    (5, Buttons::LIFT),
    (6, Buttons::PISTOL),
    (7, Buttons::MAGIC),
    (8, Buttons::DYNAMITE),
];

#[test]
fn default_key_table_is_the_original_one() {
    let map = Keymap::default();
    for (b, keys) in SPEC_KEYS {
        assert_eq!(map.keys(B(b)), keys.to_vec(), "{b:?}");
    }
    let commands = [
        ("quit", vec![KeyCode::Escape]),
        ("fullscreen", vec![KeyCode::F11]),
        ("window_size", vec![KeyCode::F10]),
        ("aspect", vec![KeyCode::F9]),
        ("scale", vec![KeyCode::F8]),
        ("interpolation", vec![KeyCode::F7]),
        // The original's pause key alone, and F1 for help.
        ("pause", vec![KeyCode::Pause]),
        ("help", vec![KeyCode::F1]),
    ];
    for (name, keys) in commands {
        assert_eq!(map.keys(Action::from_name(name).unwrap()), keys, "{name}");
    }
}

#[test]
fn default_pad_table_is_the_original_joystick_layout() {
    let map = PadMap::default();
    let by_number = |n: u8| {
        claw_game::keymap::PAD_BUTTON_NAMES
            .iter()
            .map(|(b, _)| *b)
            .find(|b| b.number() == Some(n))
            .unwrap()
    };
    for (n, b) in SPEC_PAD {
        assert_eq!(map.action(by_number(n)), Some(B(b)), "button {n}");
    }
    // Button 9 opens the menu: Start is bound to the pause command, which
    // opens the in-game menu when it comes from a pad.
    assert_eq!(by_number(9), PadButton::Start);
    assert_eq!(
        map.action(PadButton::Start),
        Some(Action::Command(Command::Pause))
    );
    for (button, dir) in [
        (PadButton::DPadLeft, Buttons::LEFT),
        (PadButton::DPadRight, Buttons::RIGHT),
        (PadButton::DPadUp, Buttons::UP),
        (PadButton::DPadDown, Buttons::DOWN),
    ] {
        assert_eq!(map.action(button), Some(B(dir)));
    }
    assert_eq!(map.action(PadButton::Select), None);
    assert_eq!(PadSettings::default().dead_zone, 0.34);
}

#[test]
fn rebinding_swaps_game_keys_and_refuses_command_keys() {
    let mut map = Keymap::default();
    let jump = B(Buttons::JUMP);
    let lift = B(Buttons::LIFT);
    // Jump takes Z from lift, which gets Space back.
    let r = map
        .rebind(jump, Some(KeyCode::Space), KeyCode::KeyZ)
        .unwrap();
    assert_eq!(r.swapped, Some((lift, Some(KeyCode::Space))));
    assert_eq!(map.keys(jump), vec![KeyCode::KeyZ]);
    assert_eq!(map.keys(lift), vec![KeyCode::Space]);
    // Adding a key another action holds takes it from that action.
    let r = map.rebind(jump, None, KeyCode::Space).unwrap();
    assert_eq!(r.swapped, Some((lift, None)));
    assert!(map.keys(lift).is_empty());
    assert_eq!(map.keys(jump), vec![KeyCode::KeyZ, KeyCode::Space]);
    // Command keys are reserved.
    let before = map.clone();
    assert_eq!(
        map.rebind(jump, None, KeyCode::Escape),
        Err(Conflict::Reserved {
            holder: Action::Command(Command::Quit)
        })
    );
    assert_eq!(
        map.rebind(B(Buttons(1 << 30)), None, KeyCode::KeyQ),
        Err(Conflict::Unknown)
    );
    assert_eq!(map, before);
    // A command can move to a free key.
    map.rebind(
        Action::Command(Command::Quit),
        Some(KeyCode::Escape),
        KeyCode::F12,
    )
    .unwrap();
    assert_eq!(map.action(KeyCode::Escape), None);
    // Every arrow and any other key can be bound.
    map.rebind(B(Buttons::LEFT), Some(KeyCode::ArrowLeft), KeyCode::KeyA)
        .unwrap();
    map.rebind(B(Buttons::RIGHT), None, KeyCode::Semicolon)
        .unwrap();
    assert_eq!(map.action(KeyCode::ArrowLeft), None);
    assert_eq!(map.action(KeyCode::Semicolon), Some(B(Buttons::RIGHT)));
    map.reset();
    assert_eq!(map, Keymap::default());
    assert_eq!(map.unbind(KeyCode::KeyZ), Some(lift));
    assert!(map.keys(lift).is_empty());
}

#[test]
fn bindings_round_trip_through_their_names() {
    let mut map = Keymap::default();
    map.rebind(B(Buttons::JUMP), None, KeyCode::NumpadEnter)
        .unwrap();
    map.rebind(B(Buttons::LIFT), Some(KeyCode::KeyZ), KeyCode::Quote)
        .unwrap();
    map.unbind(KeyCode::Digit3);
    let values: BTreeMap<String, String> = map.to_config().into_iter().collect();
    assert_eq!(values.len(), ACTIONS.len());
    assert_eq!(values["key_jump"], "Space, NumpadEnter");
    assert_eq!(values["key_lift"], "Quote");
    assert_eq!(values["key_dynamite"], "none");
    assert_eq!(values["key_attack"], "ControlLeft, ControlRight");
    assert_eq!(Keymap::from_config(&values).unwrap(), map);
    assert_eq!(
        Keymap::from_config(&BTreeMap::new()).unwrap(),
        Keymap::default()
    );
    // Every key name binds and reads back.
    for &(key, name) in KEY_NAMES {
        if Keymap::default()
            .action(key)
            .is_some_and(|a| a.is_reserved())
        {
            continue;
        }
        let mut values = BTreeMap::new();
        values.insert("key_magic".to_string(), name.to_string());
        let m = Keymap::from_config(&values).unwrap();
        assert_eq!(m.keys(B(Buttons::MAGIC)), vec![key], "{name}");
        assert_eq!(m.config_value(B(Buttons::MAGIC)), name);
        assert_eq!(key_name(key), Some(name));
        assert_eq!(parse_key(name), Some(key));
    }
    let mut pad = PadMap::default();
    pad.rebind(
        B(Buttons::JUMP),
        Some(PadButton::South),
        PadButton::RightTrigger,
    )
    .unwrap();
    let values: BTreeMap<String, String> = pad.to_config().into_iter().collect();
    assert_eq!(values["pad_jump"], "RightTrigger");
    assert_eq!(values["pad_dynamite"], "South");
    assert_eq!(PadMap::from_config(&values).unwrap(), pad);
}

/// Pad events pushed by the test while the router reads them.
#[derive(Clone, Default)]
struct Script(Rc<RefCell<VecDeque<PadEvent>>>);

impl Script {
    fn push(&self, pad: usize, kind: PadEventKind) {
        self.0.borrow_mut().push_back(PadEvent { pad, kind });
    }
    fn button(&self, pad: usize, b: PadButton, down: bool) {
        self.push(pad, PadEventKind::Button(b, down));
    }
    fn stick(&self, pad: usize, x: f32, y: f32) {
        self.push(pad, PadEventKind::Axis(PadAxis::LeftX, x));
        self.push(pad, PadEventKind::Axis(PadAxis::LeftY, y));
    }
}

impl PadSource for Script {
    fn next_event(&mut self) -> Option<PadEvent> {
        self.0.borrow_mut().pop_front()
    }
}

fn router() -> (InputRouter, Script) {
    let script = Script::default();
    let r = InputRouter::with_source(
        Keymap::default(),
        PadSettings::default(),
        Box::new(script.clone()),
    );
    (r, script)
}

fn poll(r: &mut InputRouter) -> Vec<Routed> {
    r.poll(Instant::now())
}

fn down(b: Buttons) -> Routed {
    Routed::Button(b, true)
}

fn up(b: Buttons) -> Routed {
    Routed::Button(b, false)
}

#[test]
fn pad_buttons_map_to_game_buttons_and_commands() {
    let (mut r, pads) = router();
    pads.push(0, PadEventKind::Connected("Steam Deck".into()));
    assert_eq!(poll(&mut r), vec![]);
    assert_eq!(r.notices(), ["gamepad connected: Steam Deck"]);
    pads.button(0, PadButton::South, true);
    pads.button(0, PadButton::East, true);
    assert_eq!(
        poll(&mut r),
        vec![down(Buttons::JUMP), down(Buttons::ATTACK)]
    );
    // A press and release in one frame reach the game as both edges.
    pads.button(0, PadButton::South, false);
    pads.button(0, PadButton::West, true);
    pads.button(0, PadButton::West, false);
    assert_eq!(
        poll(&mut r),
        vec![
            up(Buttons::JUMP),
            down(Buttons::PROJECTILE),
            up(Buttons::PROJECTILE)
        ]
    );
    for (b, want) in [
        (PadButton::North, Buttons::TOGGLE),
        (PadButton::LeftShoulder, Buttons::LIFT),
        (PadButton::RightShoulder, Buttons::PISTOL),
        (PadButton::LeftTrigger, Buttons::MAGIC),
        (PadButton::RightTrigger, Buttons::DYNAMITE),
        (PadButton::DPadLeft, Buttons::LEFT),
        (PadButton::DPadUp, Buttons::UP),
    ] {
        pads.button(0, b, true);
        pads.button(0, b, false);
        assert_eq!(poll(&mut r), vec![down(want), up(want)], "{b:?}");
    }
    pads.button(0, PadButton::Start, true);
    pads.button(0, PadButton::Start, false);
    pads.button(0, PadButton::Select, true);
    assert_eq!(poll(&mut r), vec![Routed::Command(Command::Pause)]);
    // A repeated press of a held button is not a new press.
    pads.button(0, PadButton::East, true);
    assert_eq!(poll(&mut r), vec![]);
}

#[test]
fn the_stick_has_a_radial_dead_zone_and_eight_directions() {
    let dz = 0.66;
    assert_eq!(stick_buttons(0.0, 0.0, dz), Buttons::NONE);
    assert_eq!(stick_buttons(0.659, 0.0, dz), Buttons::NONE);
    assert_eq!(stick_buttons(0.66, 0.0, dz), Buttons::RIGHT);
    assert_eq!(stick_buttons(-1.0, 0.0, dz), Buttons::LEFT);
    assert_eq!(stick_buttons(0.0, 1.0, dz), Buttons::UP);
    assert_eq!(stick_buttons(0.0, -0.7, dz), Buttons::DOWN);
    // Radial: a diagonal that no single axis passes still counts.
    let d = 0.5;
    assert!(d < dz && (d * d + d * d).sqrt() > dz);
    assert_eq!(stick_buttons(d, d, dz), Buttons::RIGHT | Buttons::UP);
    assert_eq!(stick_buttons(-d, -d, dz), Buttons::LEFT | Buttons::DOWN);
    // Sector edges at 22.5 degrees from an axis.
    let a = 21.0_f32.to_radians();
    assert_eq!(stick_buttons(a.cos(), a.sin(), dz), Buttons::RIGHT);
    let a = 24.0_f32.to_radians();
    assert_eq!(
        stick_buttons(a.cos(), a.sin(), dz),
        Buttons::RIGHT | Buttons::UP
    );
    assert_eq!(stick_buttons(f32::NAN, 1.0, dz), Buttons::UP);
    assert_eq!(stick_buttons(5.0, 0.0, dz), Buttons::RIGHT);

    // The default dead zone: about a third of a full deflection.
    let (mut r, pads) = router();
    pads.stick(0, 0.33, 0.0);
    assert_eq!(poll(&mut r), vec![]);
    pads.stick(0, 0.35, 0.0);
    assert_eq!(poll(&mut r), vec![down(Buttons::RIGHT)]);
    pads.stick(0, 0.35, 0.35);
    assert_eq!(poll(&mut r), vec![down(Buttons::UP)]);
    // Each axis event counts: x moves first, then y.
    pads.stick(0, 0.0, 0.3);
    assert_eq!(poll(&mut r), vec![up(Buttons::RIGHT), up(Buttons::UP)]);
    // The dead zone follows the settings.
    let script = Script::default();
    let mut r = InputRouter::with_source(
        Keymap::default(),
        PadSettings {
            dead_zone: 0.25,
            ..PadSettings::default()
        },
        Box::new(script.clone()),
    );
    script.stick(0, 0.0, -0.3);
    assert_eq!(poll(&mut r), vec![down(Buttons::DOWN)]);
}

#[test]
fn pads_plug_in_and_out_and_act_as_one() {
    let (mut r, pads) = router();
    assert_eq!(poll(&mut r), vec![]);
    assert_eq!(r.notices().len(), 1);
    assert!(r.notices()[0].starts_with("no gamepad connected"));
    assert_eq!(poll(&mut r), vec![]);
    assert_eq!(r.notices().len(), 1, "said once");
    pads.push(3, PadEventKind::Connected("Pad A".into()));
    pads.push(7, PadEventKind::Connected("Pad B".into()));
    pads.button(3, PadButton::DPadRight, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::RIGHT)]);
    // The second pad holds right too; releasing the first keeps it held.
    pads.stick(7, 1.0, 0.0);
    pads.button(3, PadButton::DPadRight, false);
    assert_eq!(poll(&mut r), vec![]);
    // Unplugging the pad that holds it releases it.
    pads.push(7, PadEventKind::Disconnected);
    assert_eq!(poll(&mut r), vec![up(Buttons::RIGHT)]);
    assert_eq!(r.notices().last().unwrap(), "gamepad disconnected: Pad B");
    // A pad plugged in again starts with nothing held.
    pads.push(7, PadEventKind::Connected("Pad B".into()));
    pads.button(7, PadButton::South, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::JUMP)]);
}

#[test]
fn keys_and_pads_share_buttons() {
    let (mut r, pads) = router();
    assert_eq!(r.key(KeyCode::ArrowLeft, true), Some(down(Buttons::LEFT)));
    pads.button(0, PadButton::DPadLeft, true);
    assert_eq!(poll(&mut r), vec![]);
    pads.button(0, PadButton::DPadLeft, false);
    assert_eq!(poll(&mut r), vec![]);
    assert_eq!(r.key(KeyCode::ArrowLeft, false), Some(up(Buttons::LEFT)));
    pads.button(0, PadButton::East, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::ATTACK)]);
    assert_eq!(
        r.key(KeyCode::ControlLeft, true),
        Some(down(Buttons::ATTACK))
    );
    assert_eq!(
        r.key(KeyCode::ControlRight, true),
        Some(down(Buttons::ATTACK))
    );
    assert_eq!(r.key(KeyCode::ControlLeft, false), None);
    assert_eq!(r.key(KeyCode::ControlRight, false), None);
    pads.button(0, PadButton::East, false);
    assert_eq!(poll(&mut r), vec![up(Buttons::ATTACK)]);
}

#[test]
fn losing_focus_releases_the_pads() {
    let (mut r, pads) = router();
    pads.button(0, PadButton::South, true);
    pads.stick(0, -1.0, 0.0);
    assert_eq!(poll(&mut r), vec![down(Buttons::JUMP), down(Buttons::LEFT)]);
    assert_eq!(r.key(KeyCode::KeyZ, true), Some(down(Buttons::LIFT)));
    assert_eq!(
        r.set_focused(false),
        vec![up(Buttons::JUMP | Buttons::LEFT | Buttons::LIFT)]
    );
    // Nothing from a pad counts without focus.
    pads.button(0, PadButton::East, true);
    pads.stick(0, 1.0, 0.0);
    assert_eq!(poll(&mut r), vec![]);
    assert_eq!(r.set_focused(true), vec![]);
    pads.button(0, PadButton::South, false);
    assert_eq!(poll(&mut r), vec![]);
    pads.button(0, PadButton::South, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::JUMP)]);
}

#[test]
fn menus_get_menu_inputs() {
    let (mut r, pads) = router();
    pads.button(0, PadButton::East, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::ATTACK)]);
    assert_eq!(r.key(KeyCode::ArrowRight, true), Some(down(Buttons::RIGHT)));
    // Opening a menu releases what the game held.
    assert_eq!(
        r.set_context(InputContext::Menu),
        vec![up(Buttons::RIGHT | Buttons::ATTACK)]
    );
    assert_eq!(r.context(), InputContext::Menu);
    pads.button(0, PadButton::East, false);
    pads.button(0, PadButton::DPadDown, true);
    assert_eq!(poll(&mut r), vec![Routed::Menu(MenuInput::Down)]);
    assert_eq!(r.menu_held(), vec![MenuInput::Down]);
    pads.button(0, PadButton::DPadDown, false);
    pads.stick(0, 0.0, 1.0);
    assert_eq!(poll(&mut r), vec![Routed::Menu(MenuInput::Up)]);
    assert_eq!(poll(&mut r), vec![], "a held direction steps once");
    pads.stick(0, 0.0, 0.0);
    pads.button(0, PadButton::South, true);
    pads.button(0, PadButton::East, true);
    pads.button(0, PadButton::Start, true);
    assert_eq!(
        poll(&mut r),
        vec![
            Routed::Menu(MenuInput::Confirm),
            Routed::Menu(MenuInput::Back),
            Routed::Menu(MenuInput::Confirm)
        ]
    );
    assert_eq!(
        r.key(KeyCode::ArrowUp, true),
        Some(Routed::Menu(MenuInput::Up))
    );
    assert_eq!(r.menu_held(), vec![MenuInput::Up]);
    assert_eq!(r.key(KeyCode::ArrowUp, false), None);
    assert_eq!(
        r.key(KeyCode::Enter, true),
        Some(Routed::Menu(MenuInput::Confirm))
    );
    assert_eq!(
        r.key(KeyCode::Space, true),
        Some(Routed::Menu(MenuInput::Confirm))
    );
    assert_eq!(
        r.key(KeyCode::Escape, true),
        Some(Routed::Menu(MenuInput::Back))
    );
    assert_eq!(
        r.key(KeyCode::F11, true),
        Some(Routed::Command(Command::ToggleFullscreen))
    );
    assert_eq!(r.key(KeyCode::KeyZ, true), None);
    // Back in play, buttons still held from the menu need a new press.
    assert_eq!(r.set_context(InputContext::Play), vec![]);
    pads.button(0, PadButton::South, false);
    assert_eq!(poll(&mut r), vec![up(Buttons::JUMP)]);
    pads.button(0, PadButton::South, true);
    assert_eq!(poll(&mut r), vec![down(Buttons::JUMP)]);
}

#[test]
fn keys_held_through_a_menu_are_held_again_with_the_original_rule() {
    for keep in [true, false] {
        let mut r = InputRouter::keyboard_only(Keymap::default());
        r.set_keep_held(keep);
        assert_eq!(r.key(KeyCode::ArrowRight, true), Some(down(Buttons::RIGHT)));
        assert_eq!(r.buttons_down(), Buttons::RIGHT);
        assert_eq!(
            r.set_context(InputContext::Menu),
            vec![up(Buttons::RIGHT)],
            "a menu takes the keys"
        );
        // Keys go on being followed in the menu: Space is down when the
        // menu closes, Right still is.
        assert_eq!(
            r.key(KeyCode::Space, true),
            Some(Routed::Menu(MenuInput::Confirm))
        );
        assert_eq!(r.buttons_down(), Buttons::RIGHT | Buttons::JUMP);
        assert_eq!(r.set_context(InputContext::Play), vec![]);
        // Released in play: with the original rule the keys were held
        // again, so their release counts; otherwise it is no release.
        let released = r.key(KeyCode::ArrowRight, false);
        if keep {
            assert_eq!(released, Some(up(Buttons::RIGHT)));
        }
        assert_eq!(r.buttons_down(), Buttons::JUMP);
        // Focus loss lets go of everything: no key stays stuck.
        r.set_focused(false);
        assert_eq!(r.buttons_down(), Buttons::NONE);
    }
}

#[test]
fn the_next_key_or_pad_button_can_be_captured() {
    let (mut r, pads) = router();
    let jump = B(Buttons::JUMP);
    r.capture_next_key(jump, Some(KeyCode::Space));
    assert!(r.capturing());
    assert_eq!(
        r.key(KeyCode::KeyW, true),
        None,
        "a captured key does not act"
    );
    assert!(!r.capturing());
    assert_eq!(
        r.take_capture(),
        Some(CaptureOutcome::Bound {
            action: jump,
            input: Captured::Key(KeyCode::KeyW),
            swapped: None
        })
    );
    assert_eq!(r.take_capture(), None);
    assert_eq!(r.key(KeyCode::KeyW, false), Some(up(Buttons::JUMP)));
    assert_eq!(r.key(KeyCode::KeyW, true), Some(down(Buttons::JUMP)));
    assert_eq!(r.key(KeyCode::Space, true), None);
    r.capture_next_key(jump, None);
    r.key(KeyCode::Pause, true);
    assert_eq!(
        r.take_capture(),
        Some(CaptureOutcome::Refused {
            action: jump,
            input: Captured::Key(KeyCode::Pause),
            conflict: Conflict::Reserved {
                holder: Action::Command(Command::Pause)
            }
        })
    );
    r.capture_next_key(jump, None);
    r.key(KeyCode::Escape, true);
    assert_eq!(r.take_capture(), Some(CaptureOutcome::Cancelled));
    assert_eq!(r.keymap().keys(jump), vec![KeyCode::KeyW]);
    assert_eq!(r.key(KeyCode::KeyW, false), Some(up(Buttons::JUMP)));
    // Pads: the next pad button, swapping with its holder.
    r.capture_next_pad_button(jump, Some(PadButton::South));
    assert_eq!(r.key(KeyCode::KeyQ, true), None);
    assert!(r.capturing());
    pads.button(0, PadButton::North, true);
    assert_eq!(poll(&mut r), vec![], "a captured button does not act");
    assert_eq!(
        r.take_capture(),
        Some(CaptureOutcome::Bound {
            action: jump,
            input: Captured::Pad(PadButton::North),
            swapped: Some((B(Buttons::TOGGLE), Some(Captured::Pad(PadButton::South))))
        })
    );
    assert_eq!(
        r.pad_settings().map.action(PadButton::South),
        Some(B(Buttons::TOGGLE))
    );
    pads.button(0, PadButton::North, false);
    pads.button(0, PadButton::North, true);
    assert_eq!(poll(&mut r), vec![up(Buttons::JUMP), down(Buttons::JUMP)]);
}

#[test]
fn gamepads_off_reads_no_pad() {
    let mut s = Settings::new("/i", Default::default());
    s.pad.enabled = false;
    let mut r = InputRouter::from_settings(&s);
    assert_eq!(poll(&mut r), vec![]);
    assert!(r.notices().is_empty());
    let mut r = InputRouter::keyboard_only(Keymap::default());
    assert_eq!(poll(&mut r), vec![]);
    assert!(r.notices().is_empty());
    assert!(r.help().contains("Ctrl attacks"));
}
