//! Sets the `claw_ffmpeg` cfg when the rusty_ffmpeg build script linked
//! FFmpeg (its links metadata `found`, read here as DEP_FFMPEG_FOUND).
//! Without it the crate builds its player stub, which plays no movie.
//!
//! With FFmpeg, `claw_ffmpeg_av1` says the linked build decodes AV1 (it has
//! dav1d, DEP_FFMPEG_AV1) and `claw_ffmpeg_h26x` that it has the H.264 and
//! HEVC decoders (DEP_FFMPEG_H26X), both as the linked build's record says;
//! the tests check the decoders against them.

use std::env;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(claw_ffmpeg)");
    println!("cargo::rustc-check-cfg=cfg(claw_ffmpeg_av1)");
    println!("cargo::rustc-check-cfg=cfg(claw_ffmpeg_h26x)");
    let set = |name: &str| env::var(name).as_deref() == Ok("1");
    if set("DEP_FFMPEG_FOUND") {
        println!("cargo::rustc-cfg=claw_ffmpeg");
        if set("DEP_FFMPEG_AV1") {
            println!("cargo::rustc-cfg=claw_ffmpeg_av1");
        }
        if set("DEP_FFMPEG_H26X") {
            println!("cargo::rustc-cfg=claw_ffmpeg_h26x");
        }
    }
}
