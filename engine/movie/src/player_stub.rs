//! The player of a build without the `ffmpeg` feature: it opens nothing.

use std::convert::Infallible;
use std::time::Duration;

use claw_audio::MusicSource;

use crate::{MovieError, ReadSeek, Source, VideoFrame};

pub(crate) fn ffmpeg_version() -> Option<String> {
    None
}

/// A movie player. This build has no decoder, so none can be opened:
/// [`Player::open`] and [`Player::open_reader`] return
/// [`MovieError::NotBuilt`].
#[derive(Debug)]
pub struct Player {
    never: Infallible,
}

impl Player {
    /// Returns [`MovieError::NotBuilt`].
    pub fn open(source: &Source, audio_rate: u32) -> Result<Player, MovieError> {
        let _ = (source, audio_rate);
        Err(MovieError::NotBuilt)
    }

    /// Returns [`MovieError::NotBuilt`].
    pub fn open_reader(
        reader: Box<dyn ReadSeek + Send>,
        name_hint: &str,
        audio_rate: u32,
    ) -> Result<Player, MovieError> {
        let _ = (reader, name_hint, audio_rate);
        Err(MovieError::NotBuilt)
    }

    /// The picture size.
    pub fn size(&self) -> (u32, u32) {
        match self.never {}
    }

    /// The shape the picture shows at.
    pub fn display_aspect(&self) -> f32 {
        match self.never {}
    }

    /// The soundtrack.
    pub fn take_audio(&mut self) -> Option<Box<dyn MusicSource>> {
        match self.never {}
    }

    /// The movie time.
    pub fn clock(&self) -> Duration {
        match self.never {}
    }

    /// The newest picture due.
    pub fn next_frame(&mut self) -> Option<&VideoFrame> {
        match self.never {}
    }

    /// The next decoded picture.
    pub fn decoded_frame(&mut self, wait: Duration) -> Option<VideoFrame> {
        let _ = wait;
        match self.never {}
    }

    /// Where the soundtrack is.
    pub fn audio_position(&self) -> Option<Duration> {
        match self.never {}
    }

    /// Whether the movie is over.
    pub fn finished(&self) -> bool {
        match self.never {}
    }

    /// Why decoding stopped early.
    pub fn error(&self) -> Option<String> {
        match self.never {}
    }
}
