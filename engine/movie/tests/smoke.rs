//! The linked FFmpeg is the pinned LGPL build with exactly the components the
//! movies need: every decoder, demuxer and parser on the list is there and
//! nothing else is (no other decoder, no encoder, muxer or network protocol).
//!
//! The lists depend on the build: AV1 through dav1d is in every build that
//! could build dav1d (`claw_ffmpeg_av1`, which the default build on a
//! system with meson or pip has), and the H.264 and HEVC decoders and
//! parsers only in a build made with RUSTY_CLAW_FFMPEG_H26X=on
//! (`claw_ffmpeg_h26x`); without the switch they must be absent.
#![cfg(claw_ffmpeg)]

use std::collections::BTreeSet;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use rusty_ffmpeg::ffi;

/// The decoders third_party/ffmpeg/build.sh always enables, by the names
/// FFmpeg registers them under (its `smacker` is smackvid here), plus h263,
/// which the mpeg4 decoder is built on and always brings.
const DECODERS: [&str; 13] = [
    "aac",
    "ac3",
    "h263",
    "mp2",
    "mp3",
    "mp3float",
    "mpeg2video",
    "mpeg4",
    "opus",
    "pcm_s16le",
    "rawvideo",
    "smackaud",
    "smackvid",
];

/// The AV1 decoder, with dav1d.
const AV1_DECODERS: [&str; 1] = ["libdav1d"];

/// The decoders RUSTY_CLAW_FFMPEG_H26X=on adds.
const H26X_DECODERS: [&str; 2] = ["h264", "hevc"];

/// Whether this build has AV1 and the H.264 and HEVC decoders.
const AV1: bool = cfg!(claw_ffmpeg_av1);
const H26X: bool = cfg!(claw_ffmpeg_h26x);

/// The decoders this build must have, and no others.
fn expected_decoders() -> BTreeSet<String> {
    let mut out = names(&DECODERS);
    if AV1 {
        out.extend(names(&AV1_DECODERS));
    }
    if H26X {
        out.extend(names(&H26X_DECODERS));
    }
    out
}

/// The demuxers it enables, by the names FFmpeg registers them under (avi,
/// smacker, mov, matroska, mpegps and mpegvideo in the configure line).
const DEMUXERS: [&str; 6] = [
    "avi",
    "matroska,webm",
    "mov,mp4,m4a,3gp,3g2,mj2",
    "mpeg",
    "mpegvideo",
    "smk",
];

fn text(p: *const c_char) -> String {
    assert!(!p.is_null());
    // SAFETY: FFmpeg hands out static NUL-terminated strings here.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// Collects what an FFmpeg `*_iterate(&mut opaque)` function returns.
fn iterate<T>(next: unsafe extern "C" fn(*mut *mut c_void) -> *const T) -> Vec<*const T> {
    let mut opaque: *mut c_void = ptr::null_mut();
    let mut out = Vec::new();
    loop {
        // SAFETY: the opaque cursor starts at NULL and is only touched by FFmpeg.
        let item = unsafe { next(&mut opaque) };
        if item.is_null() {
            return out;
        }
        out.push(item);
    }
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn exactly_the_listed_decoders_are_registered_and_no_encoder() {
    let codecs = iterate(ffi::av_codec_iterate);
    let mut decoders = BTreeSet::new();
    let mut encoders = BTreeSet::new();
    for codec in codecs {
        // SAFETY: av_codec_iterate returns valid, static codec descriptions.
        let (name, is_decoder) =
            unsafe { (text((*codec).name), ffi::av_codec_is_decoder(codec) != 0) };
        if is_decoder {
            decoders.insert(name);
        } else {
            encoders.insert(name);
        }
    }
    assert_eq!(decoders, expected_decoders());
    assert!(encoders.is_empty(), "encoders: {encoders:?}");
    if !AV1 {
        println!("this FFmpeg was built without AV1 (no dav1d)");
    }
}

#[test]
fn exactly_the_listed_demuxers_are_registered_and_no_muxer() {
    let demuxers: BTreeSet<String> = iterate(ffi::av_demuxer_iterate)
        .into_iter()
        // SAFETY: av_demuxer_iterate returns valid, static format descriptions.
        .map(|format| text(unsafe { (*format).name }))
        .collect();
    assert_eq!(demuxers, names(&DEMUXERS));
    assert!(iterate(ffi::av_muxer_iterate).is_empty());
}

#[test]
fn exactly_the_listed_parsers_are_registered() {
    // Parsers have no public name; they are known by the codecs they serve.
    let served: BTreeSet<c_int> = iterate(ffi::av_parser_iterate)
        .into_iter()
        // SAFETY: av_parser_iterate returns valid, static parser descriptions.
        .flat_map(|parser| unsafe { (*parser).codec_ids })
        .filter(|&id| id != ffi::AV_CODEC_ID_NONE as c_int)
        .collect();
    let mut expected: Vec<ffi::AVCodecID> = vec![
        ffi::AV_CODEC_ID_MPEG4,      // mpeg4video
        ffi::AV_CODEC_ID_MPEG1VIDEO, // mpegvideo
        ffi::AV_CODEC_ID_MPEG2VIDEO, // mpegvideo
        ffi::AV_CODEC_ID_MP1,        // mpegaudio
        ffi::AV_CODEC_ID_MP2,        // mpegaudio
        ffi::AV_CODEC_ID_MP3,        // mpegaudio
        ffi::AV_CODEC_ID_MP3ADU,     // mpegaudio
        ffi::AV_CODEC_ID_AC3,        // ac3
        ffi::AV_CODEC_ID_EAC3,       // ac3
        ffi::AV_CODEC_ID_AAC,        // aac
        ffi::AV_CODEC_ID_OPUS,       // opus
    ];
    let mut count = 6;
    if AV1 {
        expected.push(ffi::AV_CODEC_ID_AV1); // av1
        count += 1;
    }
    if H26X {
        expected.push(ffi::AV_CODEC_ID_H264); // h264
        expected.push(ffi::AV_CODEC_ID_HEVC); // hevc
        count += 2;
    }
    let expected: BTreeSet<c_int> = expected.into_iter().map(|id| id as c_int).collect();
    assert_eq!(served, expected);
    assert_eq!(iterate(ffi::av_parser_iterate).len(), count);
}

#[test]
fn only_the_file_protocol_reads() {
    let mut opaque: *mut c_void = ptr::null_mut();
    let mut inputs = Vec::new();
    loop {
        // SAFETY: as in `iterate`; 0 asks for input protocols.
        let name = unsafe { ffi::avio_enum_protocols(&mut opaque, 0) };
        if name.is_null() {
            break;
        }
        inputs.push(text(name));
    }
    assert_eq!(inputs, ["file"]);
}

#[test]
fn it_is_the_pinned_lgpl_build() {
    let version = claw_movie::ffmpeg_version().expect("built with FFmpeg");
    assert!(version.starts_with("8.1."), "FFmpeg {version}");
    assert!(claw_movie::available());
    // SAFETY: these return static NUL-terminated strings.
    let (configuration, licences) = unsafe {
        (
            text(ffi::avutil_configuration()),
            [
                text(ffi::avutil_license()),
                text(ffi::avcodec_license()),
                text(ffi::avformat_license()),
                text(ffi::swscale_license()),
                text(ffi::swresample_license()),
            ],
        )
    };
    for flag in ["--disable-gpl", "--disable-nonfree", "--disable-everything"] {
        assert!(
            configuration.contains(flag),
            "{flag} missing from {configuration}"
        );
    }
    assert_eq!(configuration.contains("--enable-libdav1d"), AV1);
    // FFmpeg keeps the lists quoted: --enable-decoder='h264,hevc'.
    assert_eq!(configuration.contains("h264,hevc"), H26X);
    for licence in licences {
        assert_eq!(licence, "LGPL version 2.1 or later");
    }
}
