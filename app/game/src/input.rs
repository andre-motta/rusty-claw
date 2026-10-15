//! Input routing: every key the window receives, and every input device
//! polled each frame, goes through the [`InputRouter`], which turns it into
//! a game button, a command or a menu input. The window acts on what comes
//! out and knows nothing about keys itself.
//!
//! Keys go through the keyboard map ([`crate::keymap`]). Game controllers
//! are polled in [`InputRouter::poll`], which the window calls once per
//! frame; no controller is read yet, so it returns nothing.
//!
//! [`Recorder`] keeps the input of every game tick a session plays, so a
//! game can be written out as an input script and replayed.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use anyhow::{Context, Result};
use claw_frame::{Buttons, TickInput};
use claw_sim::InputScript;
use winit::keyboard::KeyCode;

use crate::keymap::{Action, Command, Keymap};
use crate::session::{BeforeTick, Session};

/// A menu action from a key or a controller. Menus come later; nothing is
/// routed to them yet.
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

/// Turns keys and controller input into game buttons, commands and menu
/// actions.
#[derive(Debug, Clone, Default)]
pub struct InputRouter {
    keymap: Keymap,
}

impl InputRouter {
    /// A router over `keymap`.
    pub fn new(keymap: Keymap) -> InputRouter {
        InputRouter { keymap }
    }

    /// The keyboard map.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// What `key` going down (`down`) or up means: a button changes either
    /// way; a command runs when its key goes down.
    pub fn key(&mut self, key: KeyCode, down: bool) -> Option<Routed> {
        match self.keymap.action(key)? {
            Action::Button(b) => Some(Routed::Button(b, down)),
            Action::Command(c) if down => Some(Routed::Command(c)),
            Action::Command(_) => None,
        }
    }

    /// Polls the input devices other than the keyboard; called once per
    /// frame. No controller is read yet: always empty.
    pub fn poll(&mut self, now: Instant) -> Vec<Routed> {
        let _ = now;
        Vec::new()
    }

    /// Help text for the controls.
    pub fn help(&self) -> &'static str {
        self.keymap.help()
    }
}

/// Keeps the input of every tick a session plays.
///
/// Attach [`Recorder::hook`] to the session with
/// [`Session::set_before_tick`]: before each tick it reads the input the
/// simulation played on the tick before. [`Recorder::script`] adds the last
/// tick's.
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

    /// The input of every tick `session` has run, as a script.
    pub fn script(&self, session: &Session) -> InputScript {
        let mut ticks = self.ticks.borrow().clone();
        if (ticks.len() as u64) < session.ticks() {
            ticks.push(session.sim().input());
        }
        InputScript { ticks }
    }

    /// Writes the recording to `path` as an input script.
    pub fn save(&self, session: &Session, path: &Path) -> Result<usize> {
        let script = self.script(session);
        std::fs::write(path, script.to_csv())
            .with_context(|| format!("writing the recording {}", path.display()))?;
        Ok(script.ticks.len())
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
        let mut r = InputRouter::new(Keymap::default());
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
        let n = recorder.save(&session, &path).unwrap();
        assert_eq!(n as u64, session.ticks());
        let back = InputScript::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, recorder.script(&session));
    }
}
