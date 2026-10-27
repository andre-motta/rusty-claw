//! Links the pinned static FFmpeg (../build.sh) when it can be had, and
//! otherwise links nothing, so a plain `cargo build` of the game works
//! everywhere and has movies wherever FFmpeg can be built.
//!
//! Whether FFmpeg was linked reaches the crates that use this one as the
//! links metadata `found` (`DEP_FFMPEG_FOUND`, `1` or `0`); claw-movie's
//! build script turns it into the `claw_ffmpeg` cfg. Two more, `av1` (the
//! linked build has dav1d) and `h26x` (it has the H.264 and HEVC decoders),
//! both read from the linked build's record, become the `claw_ffmpeg_av1`
//! and `claw_ffmpeg_h26x` cfgs there.
//!
//! RUSTY_CLAW_FFMPEG chooses what happens:
//! - unset, empty or `auto`: link the finished build in the prefix; when
//!   there is none, build it with ../build.sh where that can work (a Unix
//!   host building for itself, with sh, a C compiler, make, curl or wget,
//!   tar and xz) and link the build it made; when that cannot be done or
//!   fails, link nothing and say why in one cargo warning. A linked build
//!   without AV1 gets a cargo warning with the reason too;
//! - `off`: link nothing;
//! - `require`: as `auto`, but linking nothing is a build error, and so is
//!   linking a build without AV1 unless RUSTY_CLAW_FFMPEG_AV1=off.
//!
//! The prefix is FFMPEG_PREFIX when set (an absolute path; empty counts as
//! unset), else the one `build.sh --prefix` names for the pinned version and
//! configure line, in the user's cache, so a build survives `cargo clean`
//! and is shared by every checkout. build.sh builds without AV1, into the
//! prefix that build has, when AV1 cannot be had; this script then links
//! the prefix `build.sh --prefix` names after the build.
//!
//! RUSTY_CLAW_FFMPEG_H26X=on asks for the build with the H.264 and HEVC
//! decoders (off by default; build.sh reads it too, and its prefix differs),
//! and RUSTY_CLAW_FFMPEG_AV1=off for the one without AV1. A linked build
//! that disagrees with the H.264 switch (through FFMPEG_PREFIX) gets a
//! warning, or an error under `require`.
//!
//! Windows builds never build FFmpeg here (that needs MSYS2 and MSVC
//! together, see ../README.md): without FFMPEG_PREFIX they use the newest
//! finished build of windows/build.sh in %LOCALAPPDATA%\rusty-claw\ffmpeg
//! whose record is exactly the one windows/build.sh writes for the current
//! pins and switches (windows_record.rs); in `auto` mode, when no build has
//! AV1, the one without it, with the same warning as on Linux. When none
//! is there, the warning names the switches that select a build that is.
//! The FFI declarations are committed in src/binding.rs, so nothing is
//! generated here.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The FFmpeg libraries the build has, each before the ones it uses.
const LIBS: [&str; 5] = ["avformat", "avcodec", "swscale", "swresample", "avutil"];

/// Where a build records itself, under its prefix.
const STAMP: &str = "share/rusty-claw/ffmpeg-build.txt";

/// The file build.sh and windows/build.sh write, in their cache folder, each
/// time they finish a build or find one finished.
const SIGNAL: &str = "last-build.txt";

/// The configure flag the H.264 and HEVC decoders come with.
const H26X_FLAG: &str = "--enable-decoder=h264,hevc";

/// What RUSTY_CLAW_FFMPEG asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Off,
    Auto,
    Require,
}

/// What the environment asks for.
#[derive(Clone, Copy)]
struct Wish {
    mode: Mode,
    /// RUSTY_CLAW_FFMPEG_H26X=on.
    h26x: bool,
    /// RUSTY_CLAW_FFMPEG_AV1=off.
    av1_off: bool,
    /// Building for Windows.
    windows: bool,
}

/// Where FFmpeg should be, or why it cannot be.
struct Search {
    /// The prefix that holds, or would hold, the build.
    prefix: Option<PathBuf>,
    /// The prefix is FFMPEG_PREFIX, not the one build.sh names.
    chosen: bool,
    /// Whether ../build.sh may build into `prefix`.
    may_build: bool,
    /// Why there is no prefix to use, when `prefix` is `None` or holds no
    /// build and nothing builds one.
    missing: String,
    /// The SIGNAL file whose change means a build may have appeared.
    watch: Option<PathBuf>,
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let script = manifest.join("../build.sh");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=windows_record.rs");
    println!("cargo::rerun-if-changed={}", script.display());
    for var in [
        "RUSTY_CLAW_FFMPEG",
        "RUSTY_CLAW_FFMPEG_H26X",
        "RUSTY_CLAW_FFMPEG_AV1",
        "FFMPEG_PREFIX",
        "XDG_CACHE_HOME",
        "HOME",
        "LOCALAPPDATA",
    ] {
        println!("cargo::rerun-if-env-changed={var}");
    }

    let mode = match env::var("RUSTY_CLAW_FFMPEG").unwrap_or_default().trim() {
        "" | "auto" => Mode::Auto,
        "off" => Mode::Off,
        "require" => Mode::Require,
        other => {
            println!("cargo::error=RUSTY_CLAW_FFMPEG must be auto, off or require, not '{other}'");
            process::exit(0)
        }
    };
    let h26x = match env::var("RUSTY_CLAW_FFMPEG_H26X")
        .unwrap_or_default()
        .trim()
    {
        "" | "off" => false,
        "on" => true,
        other => {
            println!("cargo::error=RUSTY_CLAW_FFMPEG_H26X must be on or off, not '{other}'");
            process::exit(0)
        }
    };
    let av1_off = match env::var("RUSTY_CLAW_FFMPEG_AV1").unwrap_or_default().trim() {
        "" | "auto" => false,
        "off" => true,
        other => {
            println!("cargo::error=RUSTY_CLAW_FFMPEG_AV1 must be auto or off, not '{other}'");
            process::exit(0)
        }
    };
    let wish = Wish {
        mode,
        h26x,
        av1_off,
        windows: env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows"),
    };
    if mode == Mode::Off {
        println!("cargo::metadata=found=0");
        return;
    }

    let search = search(&script, wish);
    if let Some(prefix) = &search.prefix {
        let stamp = prefix.join(STAMP);
        if stamp.is_file() {
            link(prefix, &script, wish, search.watch.as_deref());
            return;
        }
        if search.may_build {
            // build.sh builds without AV1, into another prefix, when AV1
            // cannot be had; `--prefix` names that one afterwards.
            let built = build(&script).and_then(|()| {
                let built = if search.chosen {
                    prefix.clone()
                } else {
                    default_prefix(&script)?
                };
                if built.join(STAMP).is_file() {
                    return Ok(built);
                }
                // An attempt with AV1 that failed in FFmpeg's make (which
                // may not happen again) is not recorded, so `--prefix` still
                // names the build with AV1, and a later build tries it
                // again; the build without AV1 that build.sh made instead
                // is the one to link now.
                if !search.chosen && !wish.av1_off {
                    let without = default_prefix_without_av1(&script)?;
                    if without.join(STAMP).is_file() {
                        return Ok(without);
                    }
                }
                Err(format!(
                    "building FFmpeg left no build in {}",
                    built.display()
                ))
            });
            match built {
                Ok(built) => {
                    println!(
                        "cargo::warning=built FFmpeg for the movies into {} (once per machine; \
                         RUSTY_CLAW_FFMPEG=off builds the game without it)",
                        built.display()
                    );
                    link(&built, &script, wish, search.watch.as_deref());
                    return;
                }
                Err(why) => {
                    return without(
                        mode,
                        &format!("{why}; `make ffmpeg` tries again and shows why"),
                        search.watch.as_deref(),
                    );
                }
            }
        }
    }
    without(mode, &search.missing, search.watch.as_deref())
}

/// Links the finished build in `prefix`. A build that appears later in
/// another prefix (after a pin change, or once AV1 could be built) rewrites
/// `watch`, which runs this script again. Says when the build lacks AV1
/// (an error under `require` unless RUSTY_CLAW_FFMPEG_AV1=off) or
/// disagrees with RUSTY_CLAW_FFMPEG_H26X.
fn link(prefix: &Path, script: &Path, wish: Wish, watch: Option<&Path>) {
    let stamp = prefix.join(STAMP);
    println!("cargo::rerun-if-changed={}", stamp.display());
    if let Some(file) = watch.filter(|f| f.is_file()) {
        println!("cargo::rerun-if-changed={}", file.display());
    }
    let record = fs::read_to_string(&stamp).unwrap_or_default();
    let av1 = record
        .lines()
        .any(|l| l.starts_with("dav1d ") && l.trim() != "dav1d none");
    let h26x = record
        .lines()
        .filter_map(|l| l.strip_prefix("configure "))
        .any(|l| l.split_whitespace().any(|flag| flag == H26X_FLAG));
    let require = wish.mode == Mode::Require;

    if h26x != wish.h26x {
        let (has, switch) = if h26x {
            ("has", "off")
        } else {
            ("lacks", "on")
        };
        let message = format!(
            "the FFmpeg build in {} {has} the H.264 and HEVC decoders, but \
             RUSTY_CLAW_FFMPEG_H26X is {switch}",
            prefix.display()
        );
        if require {
            println!("cargo::error=RUSTY_CLAW_FFMPEG=require: {message}");
            process::exit(0)
        }
        println!("cargo::warning={message}; the movie player follows the build");
    }
    if !av1 && !wish.av1_off {
        let why = no_av1_reason(prefix, script, wish);
        if require {
            println!(
                "cargo::error=RUSTY_CLAW_FFMPEG=require needs AV1, but the FFmpeg build in {} \
                 has none: {why} (with RUSTY_CLAW_FFMPEG_AV1=off as well, it takes a build \
                 without AV1)",
                prefix.display()
            );
            process::exit(0)
        }
        println!(
            "cargo::warning=this FFmpeg decodes no AV1, so upscaled AV1 movies will not play \
             (the original movies do): {why}; see third_party/ffmpeg/README.md"
        );
    }

    let lib = prefix.join("lib");
    println!("cargo::rustc-link-search=native={}", lib.display());
    for name in LIBS {
        println!("cargo::rustc-link-lib=static={name}");
    }
    // The libraries FFmpeg uses: those in the prefix (dav1d) are static and
    // come first, before the system ones they use in turn.
    let (ours, system): (Vec<String>, Vec<String>) = system_libs(&lib)
        .into_iter()
        .partition(|name| in_prefix(&lib, name));
    for name in ours {
        println!("cargo::rustc-link-lib=static={name}");
    }
    for name in system {
        println!("cargo::rustc-link-lib={name}");
    }
    println!("cargo::metadata=av1={}", u8::from(av1));
    println!("cargo::metadata=h26x={}", u8::from(h26x));
    println!("cargo::metadata=prefix={}", prefix.display());
    println!("cargo::metadata=found=1");
}

/// Why the build in `prefix` has no AV1: build.sh's own reason (`--av1`)
/// on Unix, when it has one.
fn no_av1_reason(prefix: &Path, script: &Path, wish: Wish) -> String {
    let made_without = format!("the build in {} was made without it", prefix.display());
    if wish.windows {
        return format!(
            "{made_without} (third_party/ffmpeg/windows/build.sh makes one with it; `set \
             RUSTY_CLAW_FFMPEG_AV1=off` takes this one without this warning)"
        );
    }
    if !cfg!(unix) {
        return made_without;
    }
    let Ok(output) = Command::new("sh").arg(script).arg("--av1").output() else {
        return made_without;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    match text.trim().strip_prefix("no: ") {
        Some(why) if output.status.success() && !why.is_empty() => why.to_string(),
        // build.sh would try AV1 here, for example after an attempt that
        // failed for a reason it does not keep (FFmpeg's make; see build.sh).
        _ if output.status.success() && text.trim() == "yes" => format!(
            "{made_without}, though AV1 can be tried here: `make ffmpeg` builds it with AV1 \
             (an attempt that fails leaves its log in av1-failed.log in the cache folder)"
        ),
        _ => made_without,
    }
}

/// Whether the static library `name` is in `lib` (`libNAME.a`, or MSVC's
/// `NAME.lib`).
fn in_prefix(lib: &Path, name: &str) -> bool {
    lib.join(format!("lib{name}.a")).is_file() || lib.join(format!("{name}.lib")).is_file()
}

/// Links nothing: one warning in `auto` mode, an error in `require` mode.
fn without(mode: Mode, why: &str, watch: Option<&Path>) {
    // A build that appears later (`make ffmpeg`, windows/build.sh) runs
    // this script again through the signal file they write. Cargo would run
    // the script on every build while a watched path is missing, so the
    // file is made here, empty and dated long ago (a file newer than this
    // run would run it once more), when it is not there yet. The cache
    // folder itself is not watched, as a failed build changes it each time.
    if let Some(file) = watch {
        if !file.exists() {
            if let Some(dir) = file.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let long_ago = UNIX_EPOCH + Duration::from_secs(946_684_800);
            let _ = fs::write(file, "")
                .and_then(|()| fs::File::options().write(true).open(file))
                .and_then(|f| f.set_modified(long_ago));
        }
        if file.is_file() {
            println!("cargo::rerun-if-changed={}", file.display());
        }
    }
    if mode == Mode::Require {
        println!("cargo::error=RUSTY_CLAW_FFMPEG=require, but {why}");
        process::exit(0)
    }
    println!("cargo::warning=this build plays no movies: {why}; see third_party/ffmpeg/README.md");
    println!("cargo::metadata=found=0");
}

/// Where the build is, for the target being built.
fn search(script: &Path, wish: Wish) -> Search {
    let target = env::var("TARGET").unwrap_or_default();
    let host = env::var("HOST").unwrap_or_default();
    let windows = wish.windows;

    // An empty FFMPEG_PREFIX is the same as none, as in build.sh and the
    // Makefile. A relative one would be read from the crate's directory, not
    // from where the build was started, so it is refused.
    if let Some(prefix) = env::var_os("FFMPEG_PREFIX").filter(|p| !p.is_empty()) {
        let prefix = PathBuf::from(prefix);
        if !prefix.has_root() {
            return Search {
                prefix: None,
                chosen: true,
                may_build: false,
                missing: format!(
                    "FFMPEG_PREFIX must be an absolute path, not '{}'",
                    prefix.display()
                ),
                watch: None,
            };
        }
        let watch = if windows {
            windows_cache_root()
        } else {
            unix_cache_root()
        }
        .map(|r| r.join(SIGNAL));
        let mut missing = format!(
            "no finished FFmpeg build in FFMPEG_PREFIX ({})",
            prefix.display()
        );
        let can_build = if windows {
            Err("on Windows, third_party/ffmpeg/windows/build.sh builds it".to_string())
        } else {
            can_build_for(&target, &host)
        };
        if let Err(why) = &can_build {
            missing = format!("{missing}, and {why}");
        }
        return Search {
            missing,
            may_build: can_build.is_ok(),
            prefix: Some(prefix),
            chosen: true,
            watch,
        };
    }

    if windows {
        let root = windows_cache_root();
        let windows_script = script.with_file_name("windows").join("build.sh");
        println!("cargo::rerun-if-changed={}", windows_script.display());
        let picked = root.as_deref().map(|r| {
            windows_pick(
                wish.h26x,
                wish.av1_off,
                wish.mode == Mode::Require,
                |h26x, av1| windows_build(r, script, &windows_script, h26x, av1),
            )
        });
        let (found, other) = match picked {
            Some(Ok(found)) => (Some(found), None),
            Some(Err(other)) => (None, other),
            None => (None, None),
        };
        let other = other
            .map(|settings| {
                format!(
                    "; one made with other switches is there, which `set {settings}` in the \
                     Developer prompt selects"
                )
            })
            .unwrap_or_default();
        let mut switches = Vec::new();
        if wish.h26x {
            switches.push("RUSTY_CLAW_FFMPEG_H26X=on");
        }
        if wish.av1_off {
            switches.push("RUSTY_CLAW_FFMPEG_AV1=off");
        }
        let switch = if switches.is_empty() {
            String::new()
        } else {
            format!(" with {}", switches.join(" and "))
        };
        return Search {
            prefix: found,
            chosen: false,
            may_build: false,
            missing: format!(
                "no FFmpeg build for Windows with the current pins and switches was found (run \
                 third_party/ffmpeg/windows/build.sh{switch} in MSYS2 first, or set \
                 FFMPEG_PREFIX){other}"
            ),
            watch: root.map(|r| r.join(SIGNAL)),
        };
    }

    let watch = unix_cache_root().map(|r| r.join(SIGNAL));
    let prefix = match default_prefix(script) {
        Ok(prefix) => prefix,
        Err(why) => {
            return Search {
                prefix: None,
                chosen: false,
                may_build: false,
                missing: why,
                watch,
            };
        }
    };
    let (may_build, missing) = match can_build_for(&target, &host) {
        Ok(()) => (true, String::new()),
        Err(why) => (false, why),
    };
    Search {
        prefix: Some(prefix),
        chosen: false,
        may_build,
        missing,
        watch,
    }
}

/// build.sh's cache folder: `${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/ffmpeg`.
fn unix_cache_root() -> Option<PathBuf> {
    let base = match env::var_os("XDG_CACHE_HOME").filter(|p| !p.is_empty()) {
        Some(cache) => PathBuf::from(cache),
        None => PathBuf::from(env::var_os("HOME").filter(|p| !p.is_empty())?).join(".cache"),
    };
    base.has_root()
        .then(|| base.join("rusty-claw").join("ffmpeg"))
}

/// windows/build.sh's cache folder: `%LOCALAPPDATA%\rusty-claw\ffmpeg`.
fn windows_cache_root() -> Option<PathBuf> {
    let base = PathBuf::from(env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty())?);
    base.has_root()
        .then(|| base.join("rusty-claw").join("ffmpeg"))
}

/// The prefix `build.sh --prefix` prints.
fn default_prefix(script: &Path) -> Result<PathBuf, String> {
    prefix_from(Command::new("sh").arg(script).arg("--prefix"), script)
}

/// The prefix `build.sh --prefix` prints for the build without AV1.
fn default_prefix_without_av1(script: &Path) -> Result<PathBuf, String> {
    prefix_from(
        Command::new("sh")
            .arg(script)
            .arg("--prefix")
            .env("RUSTY_CLAW_FFMPEG_AV1", "off"),
        script,
    )
}

/// The prefix `command`, a `build.sh --prefix`, prints.
fn prefix_from(command: &mut Command, script: &Path) -> Result<PathBuf, String> {
    let output = command
        .output()
        .map_err(|e| format!("cannot run {} ({e})", script.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let prefix = text.trim();
    if !output.status.success() || prefix.is_empty() {
        return Err(format!("{} --prefix failed", script.display()));
    }
    Ok(PathBuf::from(prefix))
}

/// Whether ../build.sh can build FFmpeg for `target` here: only for the
/// host itself, on a Unix host with the tools it uses.
fn can_build_for(target: &str, host: &str) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("FFmpeg is built automatically only on Linux and other Unix systems".into());
    }
    if target != host {
        return Err(format!(
            "FFmpeg is built automatically only for the machine itself, not for {target} \
             (set FFMPEG_PREFIX to an FFmpeg built for it)"
        ));
    }
    // Each line it prints is a missing tool.
    let check = "for t in make tar xz; do command -v \"$t\" >/dev/null 2>&1 || echo \"$t\"; done; \
                 command -v cc >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1 || \
                 command -v clang >/dev/null 2>&1 || echo 'a C compiler (cc)'; \
                 command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || \
                 echo 'curl or wget'; \
                 command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || \
                 echo sha256sum";
    let output = Command::new("sh")
        .arg("-c")
        .arg(check)
        .output()
        .map_err(|e| format!("cannot run sh to build FFmpeg ({e})"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let missing: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "building FFmpeg needs {}, which this system lacks (install what is missing, run \
             `make ffmpeg`, then build again)",
            missing.join(", ")
        ))
    }
}

/// Builds FFmpeg with ../build.sh, into the prefix `build.sh --prefix`
/// names afterwards (or FFMPEG_PREFIX), in about a minute. It runs its
/// compilers under `nice` and takes a lock, so two builds at once wait for
/// each other and build it once. FFmpeg's make joins cargo's job server.
fn build(script: &Path) -> Result<(), String> {
    let mut command = Command::new("sh");
    command.arg(script);
    // Cargo's and an outer make's settings are not FFmpeg's: configure must
    // not read them. Cargo's job server is handed on as MAKEFLAGS, which
    // build.sh's make joins (and ninja, which cannot, runs fewer jobs).
    for var in [
        "MAKEFLAGS",
        "MFLAGS",
        "MAKELEVEL",
        "CARGO_MAKEFLAGS",
        "TARGET",
        "HOST",
        "DEBUG",
        "OPT_LEVEL",
        "PROFILE",
        "NUM_JOBS",
    ] {
        command.env_remove(var);
    }
    if let Some(flags) = env::var_os("CARGO_MAKEFLAGS") {
        command.env("MAKEFLAGS", flags);
    }
    let output = command
        .output()
        .map_err(|e| format!("cannot run {} ({e})", script.display()))?;
    // The whole log stays in the build script's output, for `cargo build -vv`.
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    let errors = String::from_utf8_lossy(&output.stderr);
    eprint!("{errors}");
    if output.status.success() {
        return Ok(());
    }
    let last = errors
        .lines()
        .rev()
        .find(|l| l.starts_with("ffmpeg: "))
        .or_else(|| errors.lines().rev().find(|l| !l.trim().is_empty()))
        .unwrap_or("no message")
        .trim();
    Err(format!("building FFmpeg failed ({last})"))
}

/// The newest finished build of windows/build.sh under `root` whose record
/// is exactly the one windows/build.sh writes for the pins of `script` (its
/// version, checksums and configure line, with dav1d when `av1` and the
/// H.264 and HEVC decoders when `h26x`) and the Windows flags of
/// `windows_script`.
fn windows_build(
    root: &Path,
    script: &Path,
    windows_script: &Path,
    h26x: bool,
    av1: bool,
) -> Option<PathBuf> {
    let pins = fs::read_to_string(script).ok()?;
    let windows = fs::read_to_string(windows_script).ok()?;
    let expected = windows_record(&pins, &windows, h26x, av1)?;
    let version = shell_value(&pins, "FFMPEG_VERSION")?;
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(&format!("{version}-")) && name.ends_with("-msvc")) {
            continue;
        }
        let stamp = entry.path().join(STAMP);
        let Ok(record) = fs::read_to_string(&stamp) else {
            continue;
        };
        if record.replace("\r\n", "\n").trim_end() != expected {
            continue;
        }
        let when = fs::metadata(&stamp)
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| when > *t) {
            best = Some((when, entry.path()));
        }
    }
    best.map(|(_, path)| path)
}

// windows_record() and shell_value(), shared with a test.
include!("windows_record.rs");

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
