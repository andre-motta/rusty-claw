//! Pausing a game: what asked for the pause and what it holds.
//!
//! Play pauses for the pause key, for the in-game menu (and the game-over
//! fade) and while the window is in the background. [`Pauses`] keeps those
//! reasons and brings the level in play and the game's sounds in line with
//! them: the session's clock stops and its held buttons are released, and
//! the game's sounds pause (see [`AudioBridge::pause`]).
//!
//! The game's sounds are paused only while a level is in play. Leaving a
//! level (ending the game, loading another level, the end of a game) clears
//! the pauses of the level left, so the next level always starts with its
//! sounds running, whichever way the in-game menu was left.

use std::time::Duration;

use crate::audio::AudioBridge;
use crate::flow::Effect;
use crate::session::Session;

/// Why play is paused.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pauses {
    /// The pause key, when the game runs without menus.
    pub user: bool,
    /// The in-game menu or the game-over fade.
    pub menu: bool,
    /// The window is in the background (a benchmark never sets this).
    pub background: bool,
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
            }
            _ => {}
        }
    }

    /// Brings `session` (the level in play, if any) and `audio` in line
    /// with the pauses. `since_frame` is the time since the last frame, for
    /// releasing held buttons at the right moment. The game's sounds pause
    /// only while a level is in play.
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
                let at = s.stamp(since_frame);
                s.release_all(at);
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
