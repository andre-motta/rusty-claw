//! Links the static FFmpeg that `make ffmpeg` (../build.sh) builds.
//!
//! The prefix is FFMPEG_PREFIX when set (an absolute path; empty counts as
//! unset), else the one `build.sh --prefix` names for the pinned version and
//! configure line. The Makefile sets FFMPEG_PREFIX for cargo only when its
//! user did, so `make` and plain `cargo` see the same environment and do not
//! rebuild this crate in turn. The FFI declarations are
//! committed in src/binding.rs, so nothing is generated here.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

/// The FFmpeg libraries the build has, each before the ones it uses.
const LIBS: [&str; 5] = ["avformat", "avcodec", "swscale", "swresample", "avutil"];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let script = manifest.join("../build.sh");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={}", script.display());
    for var in ["FFMPEG_PREFIX", "XDG_CACHE_HOME", "HOME"] {
        println!("cargo::rerun-if-env-changed={var}");
    }

    let prefix = prefix(&script).unwrap_or_else(|why| fail(&why));
    let stamp = prefix.join("share/rusty-claw/ffmpeg-build.txt");
    println!("cargo::rerun-if-changed={}", stamp.display());
    if !stamp.is_file() {
        fail(&format!(
            "no FFmpeg build in {}; run `make ffmpeg` to build it (or set FFMPEG_PREFIX to a finished build)",
            prefix.display()
        ));
    }

    let lib = prefix.join("lib");
    println!("cargo::rustc-link-search=native={}", lib.display());
    for name in LIBS {
        println!("cargo::rustc-link-lib=static={name}");
    }
    for name in system_libs(&lib) {
        println!("cargo::rustc-link-lib={name}");
    }
    println!("cargo::metadata=prefix={}", prefix.display());
}

/// Stops the build with one error line.
fn fail(message: &str) -> ! {
    println!("cargo::error={message}");
    process::exit(0)
}

fn prefix(script: &Path) -> Result<PathBuf, String> {
    // An empty FFMPEG_PREFIX is the same as none, as in build.sh and the
    // Makefile. A relative one would be read from the crate's directory, not
    // from where the build was started, so it is refused.
    if let Some(prefix) = env::var_os("FFMPEG_PREFIX").filter(|p| !p.is_empty()) {
        let prefix = PathBuf::from(prefix);
        if !prefix.has_root() {
            return Err(format!(
                "FFMPEG_PREFIX must be an absolute path, not '{}'",
                prefix.display()
            ));
        }
        return Ok(prefix);
    }
    let output = Command::new("sh")
        .arg(script)
        .arg("--prefix")
        .output()
        .map_err(|e| {
            format!(
                "cannot run {} ({e}); set FFMPEG_PREFIX to an FFmpeg build",
                script.display()
            )
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let prefix = text.trim();
    if !output.status.success() || prefix.is_empty() {
        return Err(format!(
            "{} --prefix failed; set FFMPEG_PREFIX to an FFmpeg build",
            script.display()
        ));
    }
    Ok(PathBuf::from(prefix))
}

/// The system libraries FFmpeg's pkg-config files ask for (for example `m`
/// on Linux, `bcrypt` on Windows), which a static link must name itself.
fn system_libs(lib: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for name in LIBS {
        let Ok(pc) = fs::read_to_string(lib.join(format!("pkgconfig/lib{name}.pc"))) else {
            continue;
        };
        for line in pc.lines() {
            let Some(flags) = line
                .strip_prefix("Libs:")
                .or_else(|| line.strip_prefix("Libs.private:"))
            else {
                continue;
            };
            for flag in flags.split_whitespace() {
                if let Some(lib) = flag.strip_prefix("-l") {
                    if !LIBS.contains(&lib) {
                        out.insert(lib.to_string());
                    }
                }
            }
        }
    }
    out
}
