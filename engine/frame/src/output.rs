//! What a tick hands to the presentation besides sprites: the status bar
//! values, sound and music requests, game events and screen transitions.
//!
//! [`HudState`] and [`ScreenTransition`] describe a state and are repeated in
//! every snapshot while they hold. [`AudioCue`]s and [`GameEvent`]s are
//! different: a snapshot carries only those of the one tick that produced
//! it. A frame can run several ticks, and the frame driver keeps only the
//! previous and the current snapshot, so a consumer must collect cues and
//! events from every tick it runs (the game does this as it steps), never
//! from the snapshot it draws. Interpolation ignores them.

use crate::Point;

/// The selected projectile weapon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Weapon {
    /// The pistol.
    #[default]
    Pistol,
    /// Magic.
    Magic,
    /// Dynamite.
    Dynamite,
}

/// A timed powerup shown in the status bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PowerupHud {
    /// Which powerup, by the game's powerup code.
    pub kind: u32,
    /// Time left, ms.
    pub remaining_ms: u32,
}

/// A boss's health bar: the boss's health now and the maximum the bar
/// measures it against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct BossMeter {
    /// The boss's health now.
    pub value: i32,
    /// The health a full bar stands for.
    pub max: i32,
}

/// The values the status bar shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct HudState {
    /// Score.
    pub score: u32,
    /// Health.
    pub health: i32,
    /// Lives left.
    pub lives: i32,
    /// Selected projectile weapon.
    pub weapon: Weapon,
    /// Pistol shots left.
    pub pistol_ammo: i32,
    /// Magic charges left.
    pub magic_ammo: i32,
    /// Sticks of dynamite left.
    pub dynamite_ammo: i32,
    /// Timed powerup running, if any.
    pub powerup: Option<PowerupHud>,
    /// Countdown shown in the status bar, if one runs.
    pub timer_ms: Option<u32>,
    /// The boss's health bar, shown during a boss fight.
    pub boss_meter: Option<BossMeter>,
    /// The boss's name banner during a boss fight's introduction: its
    /// centre on the 640x480 view, scaled like the status bar.
    pub boss_banner: Option<(i32, i32)>,
}

/// Which volume setting and switch a sound follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SoundChannel {
    /// Sound effects.
    #[default]
    Effect,
    /// Voice lines.
    Voice,
    /// Ambient loops.
    Ambient,
    /// Menu and interface sounds.
    Interface,
}

/// A request to play one sound.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoundCue {
    /// Sound registry key, for example `CLAW_LAND`.
    pub key: String,
    /// Loudness from 0 to 100, as a percentage of the volume configured
    /// for the channel: 100 plays at the configured volume.
    pub volume: u8,
    /// Balance from -100 (left only) through 0 (centred) to 100 (right
    /// only).
    pub pan: i8,
    /// Change of the playback rate in percent: 0 plays the sound as
    /// recorded, 10 plays it 10 % faster and higher, -10 slower and lower.
    pub pitch: i16,
    /// Repeat until stopped.
    pub looped: bool,
    /// Volume setting and switch the sound follows.
    pub channel: SoundChannel,
    /// Handle chosen by the simulation for a later [`AudioCue::Stop`].
    pub voice: Option<u32>,
}

impl Default for SoundCue {
    /// No key, at the configured volume, centred, at its own pitch, once,
    /// on the effects channel, without a handle.
    fn default() -> Self {
        SoundCue {
            key: String::new(),
            volume: 100,
            pan: 0,
            pitch: 0,
            looped: false,
            channel: SoundChannel::Effect,
            voice: None,
        }
    }
}

/// A music track the game switches between.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MusicTrack {
    /// The level's own music.
    #[default]
    Level,
    /// The powerup tune.
    Powerup,
    /// The boss fight music.
    Boss,
    /// The short chord played at the end of a level.
    Chord,
}

/// One sound or music request of a tick.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioCue {
    /// Play a sound.
    Play(SoundCue),
    /// Stop the sound started with this handle.
    Stop {
        /// The handle given in [`SoundCue::voice`].
        voice: u32,
    },
    /// Switch the music.
    Music {
        /// Track to play.
        track: MusicTrack,
        /// Repeat it.
        looped: bool,
    },
    /// Stop the music.
    StopMusic,
}

/// Something that happened in a tick that the game outside the simulation
/// reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEvent {
    /// The level is won; the end-of-level screen follows after `after_ms`.
    LevelComplete {
        /// Delay before leaving the level, ms.
        after_ms: u32,
    },
    /// Claw died.
    ClawDied {
        /// Lives left after this death.
        lives_left: i32,
    },
    /// Claw died with no life left.
    GameOver,
    /// Claw reached a checkpoint at (x, y).
    Checkpoint {
        /// Respawn position.
        x: i32,
        /// Respawn position.
        y: i32,
    },
    /// Claw reached a save point.
    SavePoint {
        /// Save slot of the save point.
        slot: u8,
    },
    /// Show an in-game message.
    Message {
        /// Which message.
        id: u32,
        /// How long to show it, ms.
        ms: u32,
    },
}

/// The shape of a screen transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    /// A circle closing on or opening from a point.
    Circle,
    /// A fade to or from black.
    Fade,
}

/// A screen transition in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenTransition {
    /// Shape.
    pub kind: TransitionKind,
    /// Closing (the picture goes away) rather than opening.
    pub closing: bool,
    /// Tick at which the transition started.
    pub start_tick: u64,
    /// Length, ms.
    pub duration_ms: u32,
    /// Centre of a circle transition in level pixels.
    pub center: Point,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimSnapshot;

    #[test]
    fn defaults_are_pinned() {
        assert_eq!(Weapon::default(), Weapon::Pistol);
        assert_eq!(MusicTrack::default(), MusicTrack::Level);
        assert_eq!(SoundChannel::default(), SoundChannel::Effect);
        assert_eq!(
            PowerupHud::default(),
            PowerupHud {
                kind: 0,
                remaining_ms: 0
            }
        );
        assert_eq!(
            HudState::default(),
            HudState {
                score: 0,
                health: 0,
                lives: 0,
                weapon: Weapon::Pistol,
                pistol_ammo: 0,
                magic_ammo: 0,
                dynamite_ammo: 0,
                powerup: None,
                timer_ms: None,
                boss_meter: None,
                boss_banner: None,
            }
        );
        assert_eq!(BossMeter::default(), BossMeter { value: 0, max: 0 });
        assert_eq!(
            SoundCue::default(),
            SoundCue {
                key: String::new(),
                volume: 100,
                pan: 0,
                pitch: 0,
                looped: false,
                channel: SoundChannel::Effect,
                voice: None,
            }
        );
        let s = SimSnapshot::default();
        assert_eq!(s.hud, HudState::default());
        assert!(s.audio.is_empty());
        assert!(s.events.is_empty());
        assert_eq!(s.transition, None);
    }
}
