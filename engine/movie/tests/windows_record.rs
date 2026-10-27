//! The FFmpeg binding's build script finds a Windows FFmpeg build by the
//! exact record windows/build.sh writes for it, which it works out from the
//! two build scripts itself (third_party/ffmpeg/rusty_ffmpeg/windows_record.rs).
//! These tests run windows/build.sh's `--stamp` (it prints the record
//! without building anything, also on Linux) for every switch and compare.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../../../third_party/ffmpeg/rusty_ffmpeg/windows_record.rs"]
mod windows_record;

use windows_record::{shell_value, windows_record};

fn ffmpeg_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/ffmpeg")
}

/// What `windows/build.sh --stamp` prints with the switches given.
fn stamp(h26x: bool, av1: bool) -> String {
    let output = Command::new("sh")
        .arg(ffmpeg_dir().join("windows/build.sh"))
        .arg("--stamp")
        .env("RUSTY_CLAW_FFMPEG_H26X", if h26x { "on" } else { "off" })
        .env("RUSTY_CLAW_FFMPEG_AV1", if av1 { "auto" } else { "off" })
        .env_remove("FFMPEG_PREFIX")
        .output()
        .expect("sh runs");
    assert!(
        output.status.success(),
        "windows/build.sh --stamp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

#[test]
fn the_build_script_works_out_the_record_windows_build_sh_writes() {
    let pins = fs::read_to_string(ffmpeg_dir().join("build.sh")).expect("build.sh");
    let windows = fs::read_to_string(ffmpeg_dir().join("windows/build.sh")).expect("windows");
    for h26x in [false, true] {
        for av1 in [true, false] {
            let expected = windows_record(&pins, &windows, h26x, av1)
                .unwrap_or_else(|| panic!("no record for h26x {h26x}, av1 {av1}"));
            let printed = stamp(h26x, av1);
            assert_eq!(
                printed.trim_end(),
                expected,
                "h26x {h26x}, av1 {av1}: windows/build.sh --stamp and windows_record differ"
            );
            assert_eq!(printed.contains("--enable-libdav1d"), av1);
            assert_eq!(printed.contains("--enable-decoder=h264,hevc"), h26x);
        }
    }
}

#[test]
fn shell_values_read_like_sh() {
    let text = "A=1.2\nB=\"x-$A \\\ny\"\nC=https://h/$A/c-$A.tar\nD='q'\nE=${A}\n";
    assert_eq!(shell_value(text, "A").as_deref(), Some("1.2"));
    assert_eq!(shell_value(text, "B").as_deref(), Some("x-1.2 y"));
    assert_eq!(
        shell_value(text, "C").as_deref(),
        Some("https://h/1.2/c-1.2.tar")
    );
    // What the pins never use is refused rather than read wrong.
    assert_eq!(shell_value(text, "D"), None);
    assert_eq!(shell_value(text, "E"), None);
    assert_eq!(shell_value(text, "F"), None);
}
