//! The game's movies: which movie plays at which moment, where its file
//! is, and, with the `ffmpeg` feature, playing it.
//!
//! - [`MovieIndex`] numbers the movies as the game does (-2 to 14) and
//!   names each one's file and archive entry; [`MovieSet`] is a set of
//!   them.
//! - [`locate_all`] lists where a movie can play from ([`Source`]), best
//!   first, under the folders of [`Roots`].
//! - [`Player`] decodes a movie on a thread of its own: pictures as
//!   [`VideoFrame`]s, due by the movie's [`Clock`], and the soundtrack as a
//!   [`MovieAudio`] for the audio engine. [`open_first`] opens the first
//!   source that plays.
//!
//! Decoding goes through FFmpeg, built from source at a pinned version with
//! only the decoders, demuxers and parsers the movies need (see
//! `third_party/ffmpeg`). It is linked only with the `ffmpeg` feature;
//! without it the crate still builds, [`available`] says that no decoder is
//! there and [`Player::open`] returns [`MovieError::NotBuilt`], so callers
//! keep one code path and play no movie.

#![deny(unsafe_code)]

pub mod audio;
pub mod clock;
pub mod fec;
pub mod frame;
pub mod index;
pub mod lookup;
pub mod range;
pub mod source;

#[cfg(feature = "ffmpeg")]
#[allow(unsafe_code)]
mod player;
#[cfg(not(feature = "ffmpeg"))]
#[path = "player_stub.rs"]
mod player;

use std::io::{Read, Seek};

pub use audio::MovieAudio;
pub use clock::Clock;
pub use frame::{ChromaSiting, FrameLayout, Matrix, VideoFrame};
pub use index::{MovieIndex, MovieSet};
pub use lookup::locate_all;
pub use player::Player;
pub use range::PatchedRange;
pub use source::{FileKind, MovieMode, Roots, SkipRule, Source};

/// A reader a movie can be decoded from: a file, an archive entry, or bytes
/// in memory.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

/// Why a movie does not play.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MovieError {
    /// This build has no movie decoder (it was built without the `ffmpeg`
    /// feature).
    #[error("this build plays no movies (built without FFmpeg)")]
    NotBuilt,
    /// No source of the movie was found.
    #[error("no movie file found")]
    NotFound,
    /// The source could not be opened or read as a movie.
    #[error("cannot open the movie: {0}")]
    Open(String),
    /// The movie uses a format or codec this build cannot decode.
    #[error("unsupported movie: {0}")]
    Unsupported(String),
    /// Decoding failed.
    #[error("cannot decode the movie: {0}")]
    Decode(String),
}

/// Whether this build can decode movies (it was built with the `ffmpeg`
/// feature).
pub fn available() -> bool {
    cfg!(feature = "ffmpeg")
}

/// The version of the linked FFmpeg, for example `8.1.3`, or `None` without
/// the `ffmpeg` feature.
pub fn ffmpeg_version() -> Option<String> {
    player::ffmpeg_version()
}

/// Opens the first of `candidates` that plays, with its soundtrack at
/// `audio_rate` Hz, and says which it was. Each candidate that fails logs
/// one line and the next is tried. Fails with [`MovieError::NotFound`]
/// when there is no candidate, and with the last candidate's error when
/// none opens.
pub fn open_first(candidates: &[Source], audio_rate: u32) -> Result<(Player, Source), MovieError> {
    let mut last = MovieError::NotFound;
    for source in candidates {
        match Player::open(source, audio_rate) {
            Ok(player) => return Ok((player, source.clone())),
            Err(MovieError::NotBuilt) => return Err(MovieError::NotBuilt),
            Err(e) => {
                eprintln!("claw-movie: {}: {e}", source.describe());
                last = e;
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_there_exactly_when_the_decoder_is() {
        assert_eq!(ffmpeg_version().is_some(), available());
        if let Some(version) = ffmpeg_version() {
            assert!(!version.is_empty());
        }
    }

    #[test]
    fn nothing_to_open_is_not_found() {
        assert_eq!(open_first(&[], 48000).err(), Some(MovieError::NotFound));
    }

    #[test]
    fn a_missing_file_fails_and_the_next_candidate_is_tried() {
        let missing = Source::File {
            path: "/nonexistent/LOGO.avi".into(),
            kind: FileKind::Original,
        };
        let err = open_first(&[missing.clone(), missing], 48000)
            .err()
            .unwrap();
        if available() {
            assert!(matches!(err, MovieError::Open(_)), "{err:?}");
        } else {
            assert_eq!(err, MovieError::NotBuilt);
        }
    }
}
