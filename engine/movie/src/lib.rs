//! The game's movies.
//!
//! Decoding goes through FFmpeg, built from source at a pinned version with
//! only the decoders, demuxers and parsers the movies need (see
//! `third_party/ffmpeg`). It is linked only with the `ffmpeg` feature; without
//! it the crate still builds and [`available`] says that no decoder is there,
//! so callers keep one code path and play no movie.

#![deny(unsafe_code)]

/// Whether this build can decode movies (it was built with the `ffmpeg`
/// feature).
pub fn available() -> bool {
    cfg!(feature = "ffmpeg")
}

/// The version of the linked FFmpeg, for example `8.1.3`, or `None` without
/// the `ffmpeg` feature.
pub fn ffmpeg_version() -> Option<String> {
    #[cfg(feature = "ffmpeg")]
    {
        Some(ffi::version())
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        None
    }
}

#[cfg(feature = "ffmpeg")]
#[allow(unsafe_code)]
mod ffi {
    use std::ffi::CStr;

    use rusty_ffmpeg::ffi;

    pub(crate) fn version() -> String {
        // SAFETY: av_version_info returns a pointer to a static,
        // NUL-terminated string that lives as long as the program.
        let info = unsafe { CStr::from_ptr(ffi::av_version_info()) };
        info.to_string_lossy().into_owned()
    }
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
}
