//! The title screen: the title picture while its sound plays.
//!
//! The title plays its sound once at full volume and shows for the
//! sound's length (5 seconds when the sound is missing). Space, Enter,
//! Escape, a click or a gamepad button end it at once. Other keys are
//! ignored.

use claw_assets::Palette;
use claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_render::{ImageId, ScreenScene};

/// The title's sound.
pub const TITLE_SOUND: &str = "TITLE";
/// How long the title shows without its sound, ms.
pub const DEFAULT_TITLE_MS: u32 = 5000;
/// Handle of the title sound, for stopping it.
pub const TITLE_VOICE: u32 = 0x7F00_0001;

/// The title screen.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleScreen {
    background: Option<ImageId>,
    palette: Palette,
    left_ms: f64,
    done: bool,
    audio: Vec<AudioCue>,
}

impl TitleScreen {
    /// The title with `background` in `palette`, shown for
    /// `sound_ms` (the title sound's length) or [`DEFAULT_TITLE_MS`].
    pub fn new(
        background: Option<ImageId>,
        palette: Palette,
        sound_ms: Option<u32>,
    ) -> TitleScreen {
        let mut audio = Vec::new();
        if sound_ms.is_some() {
            audio.push(AudioCue::Play(SoundCue {
                key: TITLE_SOUND.to_string(),
                channel: SoundChannel::Interface,
                voice: Some(TITLE_VOICE),
                ..SoundCue::default()
            }));
        }
        TitleScreen {
            background,
            palette,
            left_ms: f64::from(sound_ms.unwrap_or(DEFAULT_TITLE_MS)),
            done: false,
            audio,
        }
    }

    /// Lets `dt_ms` pass.
    pub fn update(&mut self, dt_ms: f64) {
        if dt_ms.is_finite() && dt_ms > 0.0 {
            self.left_ms -= dt_ms;
        }
        if self.left_ms <= 0.0 {
            self.done = true;
        }
    }

    /// A key that ends the title (Space, Enter, Escape), a click or a pad
    /// button.
    pub fn skip(&mut self) {
        self.done = true;
    }

    /// True once the title has ended.
    pub fn done(&self) -> bool {
        self.done
    }

    /// Time the title still shows, ms.
    pub fn left_ms(&self) -> f64 {
        self.left_ms.max(0.0)
    }

    /// Sounds to play.
    pub fn take_audio(&mut self) -> Vec<AudioCue> {
        std::mem::take(&mut self.audio)
    }

    /// Stops the title sound, when leaving.
    pub fn leave(&mut self) -> AudioCue {
        AudioCue::Stop { voice: TITLE_VOICE }
    }

    /// The picture.
    pub fn scene(&self) -> ScreenScene {
        ScreenScene {
            background: self.background,
            palette: self.palette.clone(),
            sprites: Vec::new(),
            fade: None,
        }
    }
}
