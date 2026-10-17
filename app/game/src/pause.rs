//! Pausing a game: what asked for the pause and what it holds.
//!
//! Play pauses for the pause key, for a screen over play (the in-game
//! menu, the help screen, the game-over fade) and while the window is in
//! the background. [`Pauses`] keeps those reasons and brings the level in
//! play and the game's sounds in line with them: the session's clock stops
//! and the game's sounds pause (see [`AudioBridge::pause`]).
//!
//! # The pause key
//!
//! The Pause key (Pause/Break, without Shift, Ctrl or Alt) freezes play
//! where it is, with no menu and a small PAUSE sign, and a second press
//! goes on; every key-down toggles, auto-repeat included. It works only
//! while play shows: in a menu or on the help screen it does nothing.
//! While paused, Escape does not open the in-game menu; F1 opens the help
//! screen. When that help screen closes play runs again although the pause
//! is still set: Escape stays blocked (and the sign is gone) until the
//! next press of the Pause key, which ends the pause as any other resume.
//! A gamepad's Start opens the in-game menu, as the original joystick's
//! menu button does (see [`game_key`]).
//!
//! Each resume runs exactly one tick at once, then the game goes on in
//! real time from there (see [`Session::resume`]), so pausing and
//! resuming steps the game one tick at a time. With [`PauseInput`] at its
//! original default, keys are not released when play pauses: on the tick
//! after a pause a key that is down counts as held, and as pressed only if
//! it was not held on the last tick before the pause; after a screen every
//! key down counts as held and none as pressed. With
//! [`PauseInput::ReleaseAll`] every key is released when play stops and a
//! key held on counts only once pressed again.
//!
//! The game's sounds are paused only while a level is in play. Leaving a
//! level (ending the game, loading another level, the end of a game) clears
//! the pauses of the level left, so the next level always starts with its
//! sounds running, whichever way the in-game menu was left, and ends the
//! level's ambient loops, so the resume that follows never plays them over
//! the menus.
//!
//! [`Pauses::follow`] is the pause and sound side of every effect of the
//! game flow, the one place both the window and the flow tests carry it
//! out.

use std::time::Duration;

use crate::audio::{AudioBridge, ViewInfo};
use crate::flow::{Effect, Screen};
use crate::keymap::Command;
use crate::session::Session;

/// How keys held or pressed across a pause or a screen count when play
/// goes on: a behaviour switch, set by `pause_input` in the settings file
/// or `--pause-input` on the command line and kept in recordings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PauseInput {
    /// As in the original: no key is released. After the pause key, a key
    /// down at the resume is held, and pressed when it was not held on the
    /// last tick before the pause (so a jump pressed while paused jumps on
    /// the first tick, and one held through the pause does not jump
    /// again); key presses and releases inside the pause are not seen one
    /// by one. After a screen, every key down is held and none is pressed
    /// until it is released and pressed again.
    #[default]
    Original,
    /// Every key is released when play stops, and a key held on counts
    /// only once pressed again; a key pressed while paused counts as
    /// pressed on the first tick.
    ReleaseAll,
}

impl PauseInput {
    /// Both values, the default first.
    pub const ALL: [PauseInput; 2] = [PauseInput::Original, PauseInput::ReleaseAll];

    /// The name in settings and recordings.
    pub fn name(self) -> &'static str {
        match self {
            PauseInput::Original => "original",
            PauseInput::ReleaseAll => "release_all",
        }
    }

    /// The value named `name`.
    pub fn from_name(name: &str) -> Option<PauseInput> {
        PauseInput::ALL.into_iter().find(|p| p.name() == name)
    }
}

/// Why play is paused.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pauses {
    /// The pause key.
    pub user: bool,
    /// The in-game menu, the help screen or the game-over fade.
    pub menu: bool,
    /// The window is in the background (a benchmark never sets this).
    pub background: bool,
    /// The pause key's flag left set by a help screen opened while paused:
    /// play runs, but Escape stays blocked until the pause key is pressed
    /// again. It pauses nothing.
    pub flag: bool,
}

/// What [`Pauses::apply`] changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Nothing.
    None,
    /// The level in play stopped.
    Paused,
    /// The level in play runs again: the caller restarts its frame clock.
    Resumed,
}

impl Pauses {
    /// True when anything asks for a pause.
    pub fn paused(&self) -> bool {
        self.user || self.menu || self.background
    }

    /// Follows an effect of the game flow: the in-game menu pauses and
    /// resumes play; starting or leaving a level clears the pauses of the
    /// level before it (the background pause stays, it belongs to the
    /// window).
    pub fn effect(&mut self, e: &Effect) {
        match e {
            Effect::PauseGame => self.menu = true,
            Effect::ResumeGame => self.menu = false,
            Effect::StartLevel { .. } | Effect::EndLevel => {
                self.menu = false;
                self.user = false;
                self.flag = false;
            }
            _ => {}
        }
    }

    /// Carries out the pause and sound side of the game flow's effect `e`,
    /// once the level `e` starts or ends has started or ended: `session` is
    /// the level in play afterwards, if any. It follows `e` with the pauses
    /// (see [`Pauses::effect`]), ends the loops of a level left, applies the
    /// pauses when play starts, stops or pauses, restarts the music when
    /// the in-game menu closes, and plays the flow's sounds. The caller sets
    /// the background pause first and restarts its frame clock on
    /// [`Change::Resumed`].
    pub fn follow(
        &mut self,
        e: &Effect,
        session: Option<&mut Session>,
        mut audio: Option<&mut AudioBridge>,
        since_frame: Duration,
    ) -> Change {
        self.effect(e);
        match e {
            Effect::StartLevel { .. } | Effect::PauseGame => {
                self.apply(session, audio, since_frame)
            }
            Effect::EndLevel => {
                if let Some(a) = audio.as_deref_mut() {
                    a.leave_level();
                }
                self.apply(session, audio, since_frame)
            }
            Effect::ResumeGame => {
                let change = self.apply(session, audio.as_deref_mut(), since_frame);
                if let Some(a) = audio {
                    a.restart_music();
                }
                change
            }
            Effect::Sound(cue) => {
                if let Some(a) = audio {
                    a.apply(std::slice::from_ref(cue), &ViewInfo::default());
                }
                Change::None
            }
            Effect::MenuChord => {
                if let Some(a) = audio {
                    a.play_menu_music();
                }
                Change::None
            }
            _ => Change::None,
        }
    }

    /// Brings `session` (the level in play, if any) and `audio` in line
    /// with the pauses. `since_frame` is the time since the last frame, for
    /// releasing held buttons at the right moment when the session's
    /// [`PauseInput`] is [`PauseInput::ReleaseAll`] (with the original
    /// default nothing is released). The game's sounds pause only while a
    /// level is in play.
    pub fn apply(
        &self,
        session: Option<&mut Session>,
        audio: Option<&mut AudioBridge>,
        since_frame: Duration,
    ) -> Change {
        let paused = self.paused();
        let in_play = session.is_some();
        let mut change = Change::None;
        if let Some(s) = session {
            if paused && !s.is_paused() {
                if s.pause_input() == PauseInput::ReleaseAll {
                    let at = s.stamp(since_frame);
                    s.release_all(at);
                }
                s.pause();
                change = Change::Paused;
            } else if !paused && s.is_paused() {
                s.resume();
                change = Change::Resumed;
            }
        }
        if let Some(a) = audio {
            if paused && in_play {
                a.pause();
            } else {
                a.resume();
            }
        }
        change
    }
}

/// What a key of the game's own (Escape, the pause key, F1, a gamepad's
/// Start) does now; see [`game_key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameKey {
    /// Opens the in-game menu, or in it goes back to play
    /// ([`crate::flow::Flow::pause_key`]).
    Menu,
    /// Pauses play without a menu, or goes on.
    Pause,
    /// Goes on from the pause key's flag left by a help screen: play
    /// already runs, this ends the pause as a resume does.
    ClearFlag,
    /// Opens the help screen.
    Help,
    /// Ends the program (without menus).
    Quit,
    /// Nothing.
    Nothing,
}

/// Where a key of the game's own lands (see [`game_key`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyContext {
    /// What the game flow shows; `None` without menus (watching a script,
    /// a benchmark).
    pub screen: Option<Screen>,
    /// The level in play is complete and its tally is due.
    pub completing: bool,
    /// The pauses.
    pub pauses: Pauses,
    /// The key came from a gamepad.
    pub from_pad: bool,
    /// Shift, Ctrl or Alt is held.
    pub modifiers: bool,
}

/// What `cmd` does in `ctx`:
///
/// - Escape (`quit`) opens the in-game menu in play, unless paused or the
///   pause key's flag is set; in the menu it goes back to play. Without
///   menus it ends the program.
/// - The pause key on the keyboard, without Shift, Ctrl or Alt, pauses or
///   goes on while play shows (also without menus); with the flag left by
///   help it clears it. Elsewhere it does nothing.
/// - A gamepad's Start (bound to `pause`) opens the in-game menu as Escape
///   does, as the original joystick's menu button.
/// - F1 (`help`) opens the help screen in play, paused or not, and from
///   the in-game menu.
pub fn game_key(cmd: Command, ctx: KeyContext) -> GameKey {
    let p = ctx.pauses;
    let Some(screen) = ctx.screen else {
        return match cmd {
            Command::Quit => GameKey::Quit,
            Command::Pause if !ctx.from_pad && !ctx.modifiers => GameKey::Pause,
            _ => GameKey::Nothing,
        };
    };
    let menu = |screen: Screen| match screen {
        Screen::Play if p.user || p.flag => GameKey::Nothing,
        Screen::Play | Screen::Pause | Screen::GameOver => GameKey::Menu,
        _ => GameKey::Nothing,
    };
    match cmd {
        Command::Quit => menu(screen),
        Command::Pause if ctx.from_pad => menu(screen),
        Command::Pause => match screen {
            _ if ctx.modifiers => GameKey::Nothing,
            Screen::Play if ctx.completing && !p.user => GameKey::Nothing,
            Screen::Play if p.flag && !p.user => GameKey::ClearFlag,
            Screen::Play => GameKey::Pause,
            _ => GameKey::Nothing,
        },
        Command::Help => match screen {
            Screen::Play if ctx.completing => GameKey::Nothing,
            Screen::Play | Screen::Pause => GameKey::Help,
            _ => GameKey::Nothing,
        },
        _ => GameKey::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use claw_audio::AudioSettings;
    use claw_sim::{Sim, SimConfig, SimLevel, StartState, TileMap};

    use crate::settings::LevelChoice;

    fn session() -> Session {
        let rows = [".".repeat(20), "#".repeat(20)];
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (100, 0));
        Session::new(Sim::new(level, SimConfig::default()))
    }

    #[test]
    fn the_reasons_add_up() {
        let mut p = Pauses::default();
        assert!(!p.paused());
        p.effect(&Effect::PauseGame);
        assert!(p.paused());
        p.background = true;
        p.effect(&Effect::ResumeGame);
        assert!(p.paused(), "still in the background");
        p.background = false;
        assert!(!p.paused());
        p.user = true;
        p.menu = true;
        p.background = true;
        p.effect(&Effect::EndLevel);
        assert_eq!(
            p,
            Pauses {
                background: true,
                ..Pauses::default()
            }
        );
        p.menu = true;
        p.effect(&Effect::StartLevel {
            level: LevelChoice::Retail(1),
            start: StartState::default(),
        });
        assert!(!p.menu);
    }

    #[test]
    fn sounds_pause_only_with_a_level_in_play() {
        let mut audio = AudioBridge::silent(AudioSettings::default());
        let mut s = session();
        let mut p = Pauses::default();
        p.effect(&Effect::PauseGame);
        let since = Duration::ZERO;
        assert_eq!(
            p.apply(Some(&mut s), Some(&mut audio), since),
            Change::Paused
        );
        assert!(s.is_paused() && audio.is_paused());
        assert_eq!(p.apply(Some(&mut s), Some(&mut audio), since), Change::None);
        // The level is left from the menu: the sounds run again.
        p.effect(&Effect::EndLevel);
        assert_eq!(p.apply(None, Some(&mut audio), since), Change::None);
        assert!(!audio.is_paused());
        // In the background without a level: nothing to pause.
        p.background = true;
        p.apply(None, Some(&mut audio), since);
        assert!(!audio.is_paused());
        // A level starts in the background: it pauses until the focus is
        // back.
        let mut s = session();
        assert_eq!(
            p.apply(Some(&mut s), Some(&mut audio), since),
            Change::Paused
        );
        assert!(audio.is_paused());
        p.background = false;
        assert_eq!(
            p.apply(Some(&mut s), Some(&mut audio), since),
            Change::Resumed
        );
        assert!(!s.is_paused() && !audio.is_paused());
    }
}
