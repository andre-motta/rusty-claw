//! Audio settings, channels and play requests.

use std::path::PathBuf;

/// Default per-sound replay gap: 0 ms. The game keeps a replay throttle for
/// every sound but sets its gap to 0, so in practice it never refuses a play;
/// the mixer keeps the throttle with the same default.
pub const DEFAULT_MIN_REPLAY_GAP_MS: u32 = 0;

/// Default number of simultaneous voices (a product policy; see
/// [`AudioSettings::voice_cap`]).
pub const DEFAULT_VOICE_CAP: u16 = 64;

/// Largest accepted voice cap.
pub const MAX_VOICES: u16 = 256;

/// Which configured switch and volume a sound plays under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Channel {
    /// Game sound effects.
    #[default]
    Effect,
    /// Claw's and the characters' speech; muted by the Voice switch.
    Voice,
    /// Ambient loops and background sounds; muted by the Ambient switch.
    Ambient,
    /// Menu and interface sounds. They keep playing while the game is paused.
    Interface,
}

impl Channel {
    /// Whether this channel belongs to the game (paused with it) rather than
    /// the interface.
    pub fn is_game(self) -> bool {
        self != Channel::Interface
    }
}

/// One sound to start.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayRequest {
    /// Sound name in the bank.
    pub key: String,
    /// 0..=100, a percent of the configured sound volume: 100 plays at the
    /// configured volume. Larger values are treated as 100.
    pub volume: u8,
    /// -100 (left only) ..= 100 (right only); 0 is centred.
    pub pan: i8,
    /// Percent change of the sample's own rate; 0 plays it unchanged.
    pub pitch: i16,
    /// Loop until stopped.
    pub looped: bool,
    /// Switch and volume the sound plays under.
    pub channel: Channel,
    /// Caller handle for a later stop.
    pub handle: Option<u32>,
}

impl PlayRequest {
    /// A centred, unpitched, one-shot play at the configured volume.
    pub fn new(key: impl Into<String>, channel: Channel) -> PlayRequest {
        PlayRequest {
            key: key.into(),
            volume: 100,
            pan: 0,
            pitch: 0,
            looped: false,
            channel,
            handle: None,
        }
    }
}

/// Player audio settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSettings {
    /// Audio on or off. Off opens no output device and plays nothing.
    pub enabled: bool,
    /// Sound volume 0..=100 for every sound channel. Default 80.
    pub sound_volume: u8,
    /// Music volume 0..=100. Default 50.
    pub music_volume: u8,
    /// Music switch: music plays only when on.
    pub music: bool,
    /// Voice switch: speech plays only when on.
    pub voice: bool,
    /// Ambient switch: ambient sounds play only when on.
    pub ambient: bool,
    /// Per-sound replay throttle in ms (see
    /// [`DEFAULT_MIN_REPLAY_GAP_MS`]).
    pub min_replay_gap_ms: u32,
    /// Most simultaneous voices, 1..=[`MAX_VOICES`]. Default 64.
    ///
    /// The game itself never limits voices. This mixer does, as a product
    /// policy: when every voice is busy a new sound takes over the oldest
    /// one-shot voice, and looping voices are never taken over.
    pub voice_cap: u16,
    /// SoundFont for music, when one is configured.
    pub soundfont: Option<PathBuf>,
}

impl Default for AudioSettings {
    fn default() -> AudioSettings {
        AudioSettings {
            enabled: true,
            sound_volume: 80,
            music_volume: 50,
            music: true,
            voice: true,
            ambient: true,
            min_replay_gap_ms: DEFAULT_MIN_REPLAY_GAP_MS,
            voice_cap: DEFAULT_VOICE_CAP,
            soundfont: None,
        }
    }
}

/// The part of [`AudioSettings`] the mixer uses, as plain data that can cross
/// to the output thread without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MixSettings {
    pub enabled: bool,
    pub sound_volume: u8,
    pub music_volume: u8,
    pub music: bool,
    pub voice: bool,
    pub ambient: bool,
    pub min_replay_gap_ms: u32,
    pub voice_cap: u16,
}

impl From<&AudioSettings> for MixSettings {
    fn from(s: &AudioSettings) -> MixSettings {
        MixSettings {
            enabled: s.enabled,
            sound_volume: s.sound_volume.min(100),
            music_volume: s.music_volume.min(100),
            music: s.music,
            voice: s.voice,
            ambient: s.ambient,
            min_replay_gap_ms: s.min_replay_gap_ms,
            voice_cap: s.voice_cap.clamp(1, MAX_VOICES),
        }
    }
}

impl MixSettings {
    /// Whether sounds on `channel` may play.
    pub fn channel_on(&self, channel: Channel) -> bool {
        self.enabled
            && match channel {
                Channel::Voice => self.voice,
                Channel::Ambient => self.ambient,
                Channel::Effect | Channel::Interface => true,
            }
    }

    /// The configured volume a channel plays at, from the sound volume `S`:
    /// Effect and Interface `S`; Voice `min(S + 20, 100)` (speech is a little
    /// louder than effects); Ambient `S - 15` (at least 0) when `S > 5`,
    /// otherwise `S` (ambient loops sit below the effects).
    pub fn channel_volume(&self, channel: Channel) -> u8 {
        channel_volume(self.sound_volume, channel)
    }
}

/// The configured volume of `channel` at sound volume `s` (see
/// [`MixSettings::channel_volume`]).
pub fn channel_volume(s: u8, channel: Channel) -> u8 {
    let s = s.min(100);
    match channel {
        Channel::Effect | Channel::Interface => s,
        Channel::Voice => (s + 20).min(100),
        Channel::Ambient if s > 5 => s.saturating_sub(15),
        Channel::Ambient => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let s = AudioSettings::default();
        assert!(s.enabled);
        assert_eq!(s.sound_volume, 80);
        assert_eq!(s.music_volume, 50);
        assert!(s.voice && s.ambient);
        assert_eq!(s.voice_cap, 64);
        assert_eq!(s.min_replay_gap_ms, 0);
        assert!(s.music);
    }

    #[test]
    fn mix_settings_clamp_and_switches() {
        let s = AudioSettings {
            sound_volume: 200,
            voice_cap: 0,
            voice: false,
            ..AudioSettings::default()
        };
        let m = MixSettings::from(&s);
        assert_eq!(m.sound_volume, 100);
        assert_eq!(m.voice_cap, 1);
        assert!(!m.channel_on(Channel::Voice));
        assert!(m.channel_on(Channel::Ambient));
        let off = MixSettings::from(&AudioSettings {
            enabled: false,
            ..AudioSettings::default()
        });
        assert!(!off.channel_on(Channel::Interface));
    }

    #[test]
    fn channel_volumes() {
        for (s, effect, voice, ambient) in [
            (80, 80, 100, 65),
            (50, 50, 70, 35),
            (100, 100, 100, 85),
            (15, 15, 35, 0),
            (10, 10, 30, 0),
            (6, 6, 26, 0),
            (5, 5, 25, 5),
            (0, 0, 20, 0),
            (200, 100, 100, 85),
        ] {
            assert_eq!(channel_volume(s, Channel::Effect), effect, "S = {s}");
            assert_eq!(channel_volume(s, Channel::Interface), effect, "S = {s}");
            assert_eq!(channel_volume(s, Channel::Voice), voice, "S = {s}");
            assert_eq!(channel_volume(s, Channel::Ambient), ambient, "S = {s}");
        }
    }
}
