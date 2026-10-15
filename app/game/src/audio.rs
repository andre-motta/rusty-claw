//! Sound: the bridge between the game's sound cues and the audio engine.
//!
//! When a level loads, [`AudioBridge::load_level`] decodes every sound the
//! level registers (its own and the shared ones) into samples and hands
//! them to the engine as one sound bank, so nothing is read or decoded
//! while the game runs. Each frame the window passes the sound cues of
//! every tick it ran to [`AudioBridge::apply`], which plays them.
//!
//! Game sounds pause with the game (when it is paused or its window loses
//! focus) and resume where they stopped; interface sounds keep playing.
//! Music is not played yet.
//!
//! Only the window's normal play opens an audio device. Replays, benchmarks
//! and headless runs never create a bridge.

use std::time::Instant;

use claw_assets::{AssetStore, Registries, wav};
use claw_audio::{AudioEngine, AudioSettings, Channel, Pcm, PlayRequest, SoundBank};
use claw_frame::{AudioCue, SoundChannel, SoundCue};

/// The part of the level in view, for placing sounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewInfo {
    /// Left edge in level pixels.
    pub left: i32,
    /// Top edge in level pixels.
    pub top: i32,
    /// Width in level pixels.
    pub width: u32,
    /// Height in level pixels.
    pub height: u32,
}

/// What loading a level's sounds did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BankReport {
    /// Sounds the level registers.
    pub registered: usize,
    /// Sounds decoded into the bank.
    pub decoded: usize,
    /// Sounds that could not be read or decoded, with the reason.
    pub failed: Vec<(String, String)>,
    /// Decoded samples, in bytes.
    pub bytes: usize,
    /// Time spent, ms.
    pub ms: f64,
}

/// Decodes a WAV sound file into samples.
pub fn decode_sound(data: &[u8]) -> Result<Pcm, String> {
    let w = wav::parse(data).map_err(|e| e.to_string())?;
    let channels = u8::try_from(w.channels)
        .ok()
        .filter(|c| matches!(c, 1 | 2))
        .ok_or_else(|| format!("{} channels", w.channels))?;
    let pcm = Pcm {
        rate: w.rate,
        channels,
        samples: w.samples.into(),
    };
    if !pcm.is_playable() {
        return Err("no samples".to_string());
    }
    Ok(pcm)
}

/// Decodes every sound in `registries` from `store` into a bank.
pub fn decode_bank(store: &AssetStore, registries: &Registries) -> (SoundBank, BankReport) {
    let t = Instant::now();
    let mut bank = SoundBank::new();
    let mut report = BankReport {
        registered: registries.sounds.len(),
        ..BankReport::default()
    };
    for (key, &file) in &registries.sounds {
        let pcm = store
            .read(file)
            .map_err(|e| e.to_string())
            .and_then(|data| decode_sound(&data));
        match pcm {
            Ok(pcm) => {
                report.bytes += pcm.samples.len() * 2;
                if bank.insert(key.clone(), pcm.into()) {
                    report.decoded += 1;
                }
            }
            Err(e) => report.failed.push((key.clone(), e)),
        }
    }
    report.ms = t.elapsed().as_secs_f64() * 1000.0;
    (bank, report)
}

/// The engine channel of a cue's channel.
pub fn channel(c: SoundChannel) -> Channel {
    match c {
        SoundChannel::Effect => Channel::Effect,
        SoundChannel::Voice => Channel::Voice,
        SoundChannel::Ambient => Channel::Ambient,
        SoundChannel::Interface => Channel::Interface,
    }
}

/// The engine request of a sound cue.
pub fn request(cue: &SoundCue) -> PlayRequest {
    PlayRequest {
        key: cue.key.clone(),
        volume: cue.volume,
        pan: cue.pan,
        pitch: cue.pitch,
        looped: cue.looped,
        channel: channel(cue.channel),
        handle: cue.voice,
    }
}

/// Plays the game's sound cues.
#[derive(Debug)]
pub struct AudioBridge {
    engine: AudioEngine,
    paused: bool,
}

impl AudioBridge {
    /// Starts the audio engine on the default output device (or silently,
    /// with one message, when there is none or sound is off).
    pub fn new(settings: AudioSettings) -> AudioBridge {
        AudioBridge {
            engine: AudioEngine::new(settings),
            paused: false,
        }
    }

    /// A bridge that never opens a device (tests).
    pub fn silent(settings: AudioSettings) -> AudioBridge {
        AudioBridge {
            engine: AudioEngine::null(settings),
            paused: false,
        }
    }

    /// The engine.
    pub fn engine(&self) -> &AudioEngine {
        &self.engine
    }

    /// Stops what plays and loads the sounds of a new level: every sound
    /// `registries` names, decoded now.
    pub fn load_level(&mut self, store: &AssetStore, registries: &Registries) -> BankReport {
        self.engine.stop_all();
        let (bank, report) = decode_bank(store, registries);
        self.engine.set_bank(bank);
        report
    }

    /// Plays the cues of the ticks run since the last frame, in order, and
    /// keeps the output device going. Music cues wait for the music.
    pub fn apply(&mut self, cues: &[AudioCue], view: &ViewInfo) {
        let _ = view;
        for cue in cues {
            match cue {
                AudioCue::Play(s) => self.engine.play(&request(s)),
                AudioCue::Stop { voice } => self.engine.stop(*voice),
                AudioCue::Music { .. } | AudioCue::StopMusic => {}
            }
        }
        self.engine.poll_device();
    }

    /// Pauses the game's sounds.
    pub fn pause(&mut self) {
        if !self.paused {
            self.paused = true;
            self.engine.pause_game();
        }
    }

    /// Resumes the game's sounds where they stopped.
    pub fn resume(&mut self) {
        if self.paused {
            self.paused = false;
            self.engine.resume_game();
        }
    }

    /// True while the game's sounds are paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Plays an interface sound.
    pub fn play_ui(&mut self, key: &str) {
        self.engine.play_ui(key);
    }

    /// Starts the menu music. Music is not played yet.
    pub fn play_menu_music(&mut self) {}

    /// Stops the music. Music is not played yet.
    pub fn stop_music(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_decode_from_wav_files() {
        let mono = wav::build(1, 11025, 16, &[1, 0, 2, 0, 3, 0]);
        let p = decode_sound(&mono).unwrap();
        assert_eq!((p.rate, p.channels), (11025, 1));
        assert_eq!(&p.samples[..], &[1, 2, 3]);
        let eight = wav::build(2, 22050, 8, &[128, 255, 0, 128]);
        let p = decode_sound(&eight).unwrap();
        assert_eq!((p.channels, p.frames()), (2, 2));
        assert!(decode_sound(b"not a sound").is_err());
        assert!(decode_sound(&wav::build(3, 8000, 16, &[0; 12])).is_err());
        assert!(decode_sound(&wav::build(1, 8000, 16, &[])).is_err());
    }

    #[test]
    fn cues_map_to_requests() {
        let cue = SoundCue {
            key: "GAME_HIT1".into(),
            volume: 70,
            pan: -20,
            pitch: 5,
            looped: true,
            channel: SoundChannel::Voice,
            voice: Some(9),
        };
        assert_eq!(
            request(&cue),
            PlayRequest {
                key: "GAME_HIT1".into(),
                volume: 70,
                pan: -20,
                pitch: 5,
                looped: true,
                channel: Channel::Voice,
                handle: Some(9),
            }
        );
        for (c, e) in [
            (SoundChannel::Effect, Channel::Effect),
            (SoundChannel::Voice, Channel::Voice),
            (SoundChannel::Ambient, Channel::Ambient),
            (SoundChannel::Interface, Channel::Interface),
        ] {
            assert_eq!(channel(c), e);
        }
    }

    #[test]
    fn the_bridge_plays_and_pauses_without_a_device() {
        let mut bridge = AudioBridge::silent(AudioSettings::default());
        let mut bank = SoundBank::new();
        bank.insert("A", Pcm::mono(22050, vec![0i16; 100]).into());
        bridge.engine.set_bank(bank);
        let cues = vec![
            AudioCue::Play(SoundCue {
                key: "A".into(),
                ..SoundCue::default()
            }),
            AudioCue::Play(SoundCue {
                key: "MISSING".into(),
                ..SoundCue::default()
            }),
            AudioCue::Stop { voice: 3 },
            AudioCue::StopMusic,
        ];
        bridge.apply(&cues, &ViewInfo::default());
        bridge.pause();
        bridge.pause();
        assert!(bridge.is_paused());
        bridge.resume();
        assert!(!bridge.is_paused());
        bridge.play_ui("A");
        bridge.play_menu_music();
        bridge.stop_music();
        assert_eq!(bridge.engine().output(), claw_audio::Output::Null);
    }
}
