//! Sets the `claw_ffmpeg` cfg when the rusty_ffmpeg build script linked
//! FFmpeg (its links metadata `found`, read here as DEP_FFMPEG_FOUND).
//! Without it the crate builds its player stub, which plays no movie.

use std::env;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(claw_ffmpeg)");
    if env::var("DEP_FFMPEG_FOUND").as_deref() == Ok("1") {
        println!("cargo::rustc-cfg=claw_ffmpeg");
    }
}
