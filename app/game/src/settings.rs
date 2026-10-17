//! Settings: the command line, the optional config file and their defaults,
//! and writing changed options back to the config file.
//!
//! Every setting can come from the config file or the command line; the
//! command line wins. The config file is `--config FILE`, or else
//! `$XDG_CONFIG_HOME/rusty-claw/config.toml` (by default
//! `~/.config/rusty-claw/config.toml`) when it exists. It holds one
//! `key = value` line per setting, with `#` comments:
//!
//! ```toml
//! install = "/path/to/your/install"
//! level = 1                 # or: custom = "My Level.WWD"
//! res = "2560x1440"         # window size
//! fullscreen = false
//! aspect = "fill"           # fill, centered or 4:3
//! scale = "fit"             # fit or integer
//! widen_activation = false
//! mid_air_hit_fix = off     # on: no second jump after a hit in mid-air
//! pause_input = original    # release_all: keys released on pause and menus
//! fuse_clock = original     # frozen: help time does not burn the fuse
//! invincible = off          # play-test aid: Claw takes no damage
//! infinite_lives = off      # play-test aid: no life is ever lost
//! vsync = true
//! fps_cap = 300             # with vsync off, 1 to 10000
//! interpolation = true
//! status_bar = "edges"      # edges, or centred in the 4:3 area
//! front_layer = on          # off: the level's front plane is not drawn
//! audio = on                # on or off
//! sound_volume = 80         # 0 to 100
//! music_volume = 50         # 0 to 100
//! voice = on                # Claw's and the enemies' speech
//! ambient = on              # ambient sounds
//! soundfont = "/path/to/a/soundfont.sf2"
//! gamepad = on              # on or off
//! pad_dead_zone = 0.34      # stick dead zone, 0.05 to 0.95
//! key_jump = "Space"        # key_<action>: key names, comma separated
//! pad_jump = "South"        # pad_<action>: pad button names
//! ```
//!
//! The bindable actions are `left`, `right`, `up`, `down`, `jump`,
//! `attack`, `projectile`, `toggle`, `lift`, `pistol`, `magic`,
//! `dynamite`, and the commands `quit`, `fullscreen`, `window_size`,
//! `aspect`, `scale`, `interpolation` and `pause` (see [`crate::keymap`]
//! for the key and pad button names).
//!
//! With vsync on, the display's refresh paces drawing. With vsync off,
//! drawing is capped at `fps_cap` frames per second (300 by default);
//! `--fps-cap 0` or `--uncapped` removes the cap, which is meant only for
//! measuring throughput, so the settings file cannot remove it. The cap also paces headless benchmarks, which
//! draw at their simulated display rate unless `--uncapped` is given.
//!
//! Options changed in the game are written back with
//! [`ConfigFile::update_in_place`]: only the values of the changed keys
//! change, missing keys are appended, and every other line and comment is
//! kept byte for byte. The write is atomic (a temporary file in the same
//! folder, flushed to disk, then renamed over the file) and keeps the
//! previous file as `config.toml.bak`. A file that does not parse is
//! never written, and a change made to the file while the game runs is
//! merged rather than overwritten. Values given on the command line are
//! never written back.
//!
//! The mid-air hit jump fix (`mid_air_hit_fix`, `--mid-air-hit-fix`) is off
//! by default: a hit that ends while Claw is in the air leaves him able to
//! jump again, as in the original. It applies to games started after it
//! changes. Replays and benchmarks do not follow the config file for it:
//! they play with it off unless the command line turns it on, and a replay
//! follows the line its recording carries (see
//! [`crate::session::script_behaviour`]), so a recorded route plays the
//! same on every machine. A replay also plays with the movement tuning and
//! the random seed its script's header names (`# tuning default|original`,
//! `# seed N`; see [`Settings::follow_header`]), on the level it names
//! (`# level N`) and from the start it names: the progress carried in, the
//! save point and the start point it was recorded with (see
//! [`Settings::follow_recording`]).
//!
//! Two more behaviour switches, set in the config file, on the command
//! line and on the Gameplay page, keep the original's pause and screens by
//! default (see [`crate::pause`]):
//! `pause_input` (`--pause-input`) is how keys count across the pause key
//! and the screens over play (`original`, or `release_all` to release
//! every key when play stops), and `fuse_clock` (`--fuse-clock`) whether
//! the time the help screen was up burns the dynamite fuse (`original`, or
//! `frozen`). Like the mid-air hit jump fix they apply to games started
//! after they change, replays and benchmarks play with their defaults
//! unless the command line sets them, and a replay follows the lines its
//! recording carries (see [`crate::session::script_switches`]).
//!
//! Two play-test aids, for trying out the hard parts of a level, are off
//! by default and set on the Gameplay page or in the config file
//! (`invincible`, `infinite_lives`): with invincibility Claw takes no
//! damage (death tiles and pits still end his life), with infinite lives a
//! death never lowers the lives count. `--invincible` and
//! `--infinite-lives` turn one on for that run only; it is not written back
//! unless it is changed on the Gameplay page. Like the behaviour switches
//! they apply to games started after they change, replays and benchmarks
//! play without them unless the command line turns them on, and a replay
//! plays with the aids its recording names (`# invincible = on`,
//! `# infinite_lives = on`, see [`claw_sim::input::ScriptHeader::aids`]).
//! A run with an aid on says nothing about how hard a level is.
//!
//! The front layer switch (`front_layer`, also in the Display menu) only
//! changes what is drawn: off, the level's front plane is left out.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Parser, ValueEnum};
use claw_audio::AudioSettings;
use claw_frame::{
    AspectPolicy, DEFAULT_FPS_CAP, DEFAULT_LOGIC_HEIGHT, DEFAULT_LOGIC_WIDTH, ScaleMode, ViewConfig,
};
use claw_sim::input::ScriptTuning;
use claw_sim::tuning::{FuseClock, PlayAids};
use claw_sim::{ScriptHeader, SimConfig};

use crate::keymap::{Action, Keymap, PadMap};
use crate::pause::PauseInput;

/// Default window size: the original view at twice its size.
pub const DEFAULT_WINDOW: (u32, u32) = (1280, 960);

/// Window sizes cycled at run time.
pub const WINDOW_SIZES: [(u32, u32); 7] = [
    (640, 480),
    (1280, 960),
    (1920, 1080),
    (2560, 1440),
    (3440, 1440),
    (3840, 2160),
    (5120, 1440),
];

/// Widest presentation, in game pixels, for which objects outside the
/// active area are drawn (still) at the sides: a 32:9 display. The game can
/// switch to any shape up to this at run time without restarting.
pub const FROZEN_EDGE_REACH: u32 = 1708;

/// How the picture fills a display of another shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Aspect {
    /// Fill the display; the view stays inside the level. Objects in the
    /// extra width beyond the original view stay still until Claw comes near
    /// enough to wake them, as they would off screen in the original.
    #[default]
    #[value(alias = "native")]
    Fill,
    /// Fill the display around the original view, showing beyond the level
    /// near its edges.
    Centered,
    /// The original 4:3 view with bars at the sides.
    #[value(name = "4:3")]
    Classic,
}

impl Aspect {
    /// The next policy in the run-time cycle.
    pub fn next(self) -> Aspect {
        match self {
            Aspect::Fill => Aspect::Centered,
            Aspect::Centered => Aspect::Classic,
            Aspect::Classic => Aspect::Fill,
        }
    }

    /// Name as written in settings.
    pub fn name(self) -> &'static str {
        match self {
            Aspect::Fill => "fill",
            Aspect::Centered => "centered",
            Aspect::Classic => "4:3",
        }
    }
}

/// Scaling of the picture to the display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Scale {
    /// Fill the display height: whole-pixel steps, then a smooth remainder.
    #[default]
    Fit,
    /// Whole-pixel steps only, with bars.
    Integer,
}

impl Scale {
    /// The other mode.
    pub fn toggled(self) -> Scale {
        match self {
            Scale::Fit => Scale::Integer,
            Scale::Integer => Scale::Fit,
        }
    }
}

/// Which level to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelChoice {
    /// A retail level, 1 to 14.
    Retail(u32),
    /// A community custom level file in the install's `Custom` folder.
    Custom(String),
}

impl Default for LevelChoice {
    fn default() -> Self {
        LevelChoice::Retail(1)
    }
}

/// Play Captain Claw from your own install.
#[derive(Debug, Clone, Default, Parser)]
#[command(name = "rusty-claw", version)]
#[command(group(ArgGroup::new("which").args(["level", "custom"])))]
pub struct Args {
    /// Config file (default: $XDG_CONFIG_HOME/rusty-claw/config.toml when it
    /// exists).
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Root folder of your own game install (the one holding `Assets` or
    /// `CLAW.REZ`).
    #[arg(long, value_name = "PATH")]
    pub install: Option<PathBuf>,
    /// Retail level number, 1 to 14 (default 1).
    #[arg(long, value_name = "N")]
    pub level: Option<u32>,
    /// Community custom level file in the install's `Custom` folder.
    #[arg(long, value_name = "FILE.WWD")]
    pub custom: Option<String>,
    /// Window size, for example 2560x1440 (default 1280x960); with
    /// `--bench --headless`, the offscreen output size.
    #[arg(long, value_name = "WxH", value_parser = parse_size)]
    pub res: Option<(u32, u32)>,
    /// Borderless fullscreen on the current monitor.
    #[arg(long, conflicts_with = "windowed")]
    pub fullscreen: bool,
    /// A window, even when the config file asks for fullscreen.
    #[arg(long)]
    pub windowed: bool,
    /// How the picture fills a display of another shape.
    #[arg(long, value_enum)]
    pub aspect: Option<Aspect>,
    /// Scaling of the picture to the display.
    #[arg(long, value_enum)]
    pub scale: Option<Scale>,
    /// Keep every object on a wide display active (a gameplay change: enemies
    /// and traps at the sides of a wide view move as they come into sight).
    #[arg(long)]
    pub widen_activation: bool,
    /// The mid-air hit jump fix, on or off (default off). On, a hit that
    /// ends while Claw is in the air leaves him falling; off, he can jump
    /// again before landing, as in the original. Replays and benchmarks
    /// use off unless this is given.
    #[arg(long, value_name = "on|off", value_parser = parse_on_off)]
    pub mid_air_hit_fix: Option<bool>,
    /// How keys count across the pause key and the screens over play:
    /// original (default; nothing is released, a key pressed while paused
    /// is pressed on the first tick) or release_all.
    #[arg(long, value_name = "original|release_all", value_parser = parse_pause_input)]
    pub pause_input: Option<PauseInput>,
    /// Whether the time the help screen was up burns the dynamite fuse:
    /// original (default) or frozen.
    #[arg(long, value_name = "original|frozen", value_parser = parse_fuse_clock)]
    pub fuse_clock: Option<FuseClock>,
    /// Play-test aid: Claw takes no damage from enemies, shots, hazards,
    /// explosions or bosses (death tiles and pits still end his life). For
    /// this run only; not saved.
    #[arg(long)]
    pub invincible: bool,
    /// Play-test aid: losing a life never lowers the lives count, so play
    /// goes on from the checkpoint. For this run only; not saved.
    #[arg(long)]
    pub infinite_lives: bool,
    /// Wait for the display's refresh: on or off.
    #[arg(long, value_name = "on|off", value_parser = parse_on_off)]
    pub vsync: Option<bool>,
    /// With vsync off: the most frames drawn per second (default 300); 0
    /// means no cap, for measuring throughput only.
    #[arg(long, value_name = "N", conflicts_with = "uncapped")]
    pub fps_cap: Option<u32>,
    /// No frame-rate cap with vsync off, and no pacing of a headless
    /// benchmark: for measuring throughput only.
    #[arg(long)]
    pub uncapped: bool,
    /// Draw the latest tick only, without interpolating between ticks.
    #[arg(long)]
    pub no_interp: bool,
    /// Image pack folder with higher-resolution replacements.
    #[arg(long, value_name = "DIR")]
    pub pack: Option<PathBuf>,
    /// No sound: open no audio device.
    #[arg(long)]
    pub no_audio: bool,
    /// Write the input of every game tick played in the window to this
    /// file, as an input script `--replay` plays back.
    #[arg(long, value_name = "FILE.csv", conflicts_with_all = ["bench", "trace"])]
    pub record: Option<PathBuf>,
    /// Put Claw at X,Y (level pixels) and let him settle before play or a
    /// replay starts.
    #[arg(long, value_name = "X,Y", value_parser = parse_point)]
    pub start: Option<(i32, i32)>,
    /// Play a recorded input script (columns `tick,held_mask,pressed_mask`,
    /// one row per 18 ms tick) on the level and from the start it names;
    /// in a window unless `--trace` is given.
    #[arg(long, value_name = "FILE.csv")]
    pub replay: Option<PathBuf>,
    /// With `--replay`: run without a window and write one trace row per
    /// script tick.
    #[arg(long, value_name = "OUT.csv", requires = "replay")]
    pub trace: Option<PathBuf>,
    /// Display rate simulated without a window, in Hz (default 60 for
    /// `--trace`, 144 for `--bench --headless`).
    #[arg(long, value_name = "HZ")]
    pub render_hz: Option<f64>,
    /// Run a benchmark for this many seconds with Claw on a fixed route,
    /// print frame statistics as JSON and a PASS or FAIL verdict; exits
    /// with status 3 when a budget is missed.
    #[arg(long, value_name = "SECONDS", conflicts_with = "replay")]
    pub bench: Option<f64>,
    /// With `--bench`: render offscreen at `--res` with a simulated display
    /// clock instead of in a window, paced to that clock unless
    /// `--uncapped` is given.
    #[arg(long, requires = "bench")]
    pub headless: bool,
}

/// Parses `WIDTHxHEIGHT`.
pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .trim()
        .to_ascii_lowercase()
        .split_once('x')
        .map(|(a, b)| (a.trim().parse::<u32>(), b.trim().parse::<u32>()))
        .ok_or_else(|| format!("expected WIDTHxHEIGHT, got {s:?}"))?;
    match (w, h) {
        (Ok(w), Ok(h)) if (1..=32768).contains(&w) && (1..=32768).contains(&h) => Ok((w, h)),
        _ => Err(format!("bad size {s:?}")),
    }
}

/// Parses `X,Y`.
pub fn parse_point(s: &str) -> Result<(i32, i32), String> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("expected X,Y, got {s:?}"))?;
    match (x.trim().parse(), y.trim().parse()) {
        (Ok(x), Ok(y)) => Ok((x, y)),
        _ => Err(format!("bad point {s:?}")),
    }
}

/// Parses `on` or `off` (also `true` and `false`).
pub fn parse_on_off(s: &str) -> Result<bool, String> {
    match s.trim() {
        "on" | "true" => Ok(true),
        "off" | "false" => Ok(false),
        _ => Err(format!("expected on or off, got {s:?}")),
    }
}

/// Parses a pause input switch: `original` or `release_all`.
pub fn parse_pause_input(s: &str) -> Result<PauseInput, String> {
    PauseInput::from_name(s.trim())
        .ok_or_else(|| format!("expected original or release_all, got {s:?}"))
}

/// Parses a fuse clock switch: `original` or `frozen`.
pub fn parse_fuse_clock(s: &str) -> Result<FuseClock, String> {
    FuseClock::from_name(s.trim()).ok_or_else(|| format!("expected original or frozen, got {s:?}"))
}

/// Largest frame-rate cap accepted.
pub const MAX_FPS_CAP: u32 = 10_000;

/// Checks a frame-rate cap: 0 means no cap.
pub fn fps_cap(n: u32) -> Result<Option<u32>, String> {
    match n {
        0 => Ok(None),
        n if n <= MAX_FPS_CAP => Ok(Some(n)),
        _ => Err(format!(
            "expected a frame rate up to {MAX_FPS_CAP}, or 0 for no cap, got {n}"
        )),
    }
}

/// What the run does.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Play in a window.
    Play,
    /// Play a recorded script: in a window, or headless into a trace.
    Replay {
        /// The script.
        script: PathBuf,
        /// Trace output; headless when set.
        trace: Option<PathBuf>,
        /// Simulated display rate for the headless run.
        render_hz: f64,
    },
    /// Benchmark.
    Bench {
        /// Length in seconds.
        seconds: f64,
        /// Offscreen with a simulated display clock.
        headless: bool,
        /// Simulated display rate for the headless run.
        render_hz: f64,
    },
}

/// Every setting, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Install root.
    pub install: PathBuf,
    /// Level to load.
    pub level: LevelChoice,
    /// Window size (or offscreen size for a headless benchmark).
    pub res: (u32, u32),
    /// Borderless fullscreen.
    pub fullscreen: bool,
    /// Aspect policy.
    pub aspect: Aspect,
    /// Scaling.
    pub scale: Scale,
    /// Keep every visible object active on wide displays.
    pub widen_activation: bool,
    /// The mid-air hit jump fix: no second jump after a hit that ends in
    /// mid-air. Off by default, as in the original.
    pub mid_air_hit_fix: bool,
    /// How keys count across the pause key and the screens over play.
    pub pause_input: PauseInput,
    /// Whether the time the help screen was up burns the dynamite fuse.
    pub fuse_clock: FuseClock,
    /// The play-test aids, both off by default.
    pub aids: PlayAids,
    /// Wait for the display refresh.
    pub vsync: bool,
    /// Frame-rate cap when the display refresh does not pace drawing, and
    /// the switch for pacing headless benchmarks; `None` means no cap.
    pub fps_cap: Option<u32>,
    /// Interpolate between ticks.
    pub interpolation: bool,
    /// Keep the status bar inside the centred 4:3 area rather than at the
    /// picture's edges.
    pub status_bar_centred: bool,
    /// Draw the level's front plane.
    pub front_layer: bool,
    /// The movement tuning, when a replayed script names one (otherwise
    /// the default tuning).
    pub tuning: Option<ScriptTuning>,
    /// The seed of the game's random stream, when a replayed script names
    /// one (otherwise the default seed).
    pub seed: Option<u32>,
    /// Image pack folder.
    pub pack: Option<PathBuf>,
    /// Claw's start point override.
    pub start: Option<(i32, i32)>,
    /// Sound on. Off, no audio device is opened.
    pub audio: bool,
    /// Where to write the input played in the window, as an input script.
    pub record: Option<PathBuf>,
    /// What the run does.
    pub mode: Mode,
    /// Sound volume, 0 to 100.
    pub sound_volume: u8,
    /// Music volume, 0 to 100.
    pub music_volume: u8,
    /// Speech on.
    pub voice: bool,
    /// Ambient sounds on.
    pub ambient: bool,
    /// SoundFont for music.
    pub soundfont: Option<PathBuf>,
    /// The keyboard map.
    pub keymap: Keymap,
    /// Gamepad settings.
    pub pad: PadSettings,
    /// The config file this run reads and writes options back to.
    pub config_path: Option<PathBuf>,
    /// Config keys whose value came from the command line; these are
    /// never written back.
    pub from_command_line: BTreeSet<&'static str>,
}

/// Default radial dead zone of the sticks, as a fraction of a full
/// deflection: 0.34, the feel of the original joystick, which reads a
/// direction once the stick leaves the middle 66 % of the way to either
/// end of an axis, that is about 34 % of a full deflection off centre.
pub const DEFAULT_PAD_DEAD_ZONE: f32 = 0.34;

/// Accepted dead zones.
pub const PAD_DEAD_ZONE_RANGE: std::ops::RangeInclusive<f32> = 0.05..=0.95;

/// Gamepad settings.
#[derive(Debug, Clone, PartialEq)]
pub struct PadSettings {
    /// Read gamepads at all.
    pub enabled: bool,
    /// Radial dead zone of the sticks, as a fraction of a full deflection.
    pub dead_zone: f32,
    /// The pad button map.
    pub map: PadMap,
}

impl Default for PadSettings {
    fn default() -> Self {
        PadSettings {
            enabled: true,
            dead_zone: DEFAULT_PAD_DEAD_ZONE,
            map: PadMap::default(),
        }
    }
}

impl Settings {
    /// Settings for `install` and `level` with every other value at its
    /// default.
    pub fn new(install: impl Into<PathBuf>, level: LevelChoice) -> Settings {
        Settings {
            install: install.into(),
            level,
            res: DEFAULT_WINDOW,
            fullscreen: false,
            aspect: Aspect::default(),
            scale: Scale::default(),
            widen_activation: false,
            mid_air_hit_fix: false,
            pause_input: PauseInput::Original,
            fuse_clock: FuseClock::Original,
            aids: PlayAids::NONE,
            vsync: true,
            fps_cap: Some(DEFAULT_FPS_CAP),
            interpolation: true,
            status_bar_centred: false,
            front_layer: true,
            tuning: None,
            seed: None,
            pack: None,
            start: None,
            audio: true,
            record: None,
            mode: Mode::Play,
            sound_volume: 80,
            music_volume: 50,
            voice: true,
            ambient: true,
            soundfont: None,
            keymap: Keymap::default(),
            pad: PadSettings::default(),
            config_path: None,
            from_command_line: BTreeSet::new(),
        }
    }

    /// Resolves the command line: the config file first, then the
    /// arguments on top.
    pub fn from_args(args: &Args) -> Result<Settings> {
        let path = args.config.clone().or_else(default_config_path);
        let file = match &args.config {
            Some(path) => Some(ConfigFile::load(path)?),
            None => match path.as_ref().filter(|p| p.is_file()) {
                Some(path) => Some(ConfigFile::load(path)?),
                None => None,
            },
        };
        let mut s = Settings::resolve(args, file.as_ref())?;
        s.config_path = path;
        Ok(s)
    }

    /// Resolves `args` over an already loaded config file.
    pub fn resolve(args: &Args, file: Option<&ConfigFile>) -> Result<Settings> {
        let mut s = Settings::new(PathBuf::new(), LevelChoice::default());
        let mut install = None;
        if let Some(file) = file {
            install = file.apply(&mut s)?;
        }
        let mut cli = BTreeSet::new();
        if let Some(path) = &args.install {
            install = Some(path.clone());
            cli.insert("install");
        }
        s.install = install.ok_or_else(|| {
            anyhow!("no install given: pass --install PATH or set install in the config file")
        })?;
        match (args.level, &args.custom) {
            (Some(n), _) => s.level = LevelChoice::Retail(n),
            (None, Some(name)) => s.level = LevelChoice::Custom(name.clone()),
            (None, None) => {}
        }
        if args.level.is_some() || args.custom.is_some() {
            cli.extend(["level", "custom"]);
        }
        if let Some(res) = args.res {
            s.res = res;
            cli.insert("res");
        }
        if args.fullscreen {
            s.fullscreen = true;
        }
        if args.windowed {
            s.fullscreen = false;
        }
        if args.fullscreen || args.windowed {
            cli.insert("fullscreen");
        }
        if let Some(a) = args.aspect {
            s.aspect = a;
            cli.insert("aspect");
        }
        if let Some(scale) = args.scale {
            s.scale = scale;
            cli.insert("scale");
        }
        if args.widen_activation {
            s.widen_activation = true;
            cli.insert("widen_activation");
        }
        if let Some(v) = args.vsync {
            s.vsync = v;
            cli.insert("vsync");
        }
        if let Some(n) = args.fps_cap {
            s.fps_cap = fps_cap(n).map_err(|e| anyhow!("--fps-cap: {e}"))?;
            cli.insert("fps_cap");
        }
        if args.uncapped {
            s.fps_cap = None;
            cli.insert("fps_cap");
        }
        if args.no_interp {
            s.interpolation = false;
            cli.insert("interpolation");
        }
        if args.pack.is_some() {
            s.pack.clone_from(&args.pack);
            cli.insert("pack");
        }
        s.start = args.start;
        if args.no_audio {
            s.audio = false;
            cli.insert("audio");
        }
        s.from_command_line = cli;
        s.record.clone_from(&args.record);
        let hz = |default: f64| -> Result<f64> {
            let hz = args.render_hz.unwrap_or(default);
            if !(hz.is_finite() && (1.0..=10_000.0).contains(&hz)) {
                bail!("--render-hz needs a rate between 1 and 10000 Hz");
            }
            Ok(hz)
        };
        s.mode = if let Some(script) = &args.replay {
            Mode::Replay {
                script: script.clone(),
                trace: args.trace.clone(),
                render_hz: hz(60.0)?,
            }
        } else if let Some(seconds) = args.bench {
            if !(seconds.is_finite() && seconds > 0.0) {
                bail!("--bench needs a positive number of seconds");
            }
            Mode::Bench {
                seconds,
                headless: args.headless,
                render_hz: hz(144.0)?,
            }
        } else {
            if args.render_hz.is_some() {
                bail!("--render-hz only applies to --replay with --trace and to --bench");
            }
            Mode::Play
        };
        // Replays and benchmarks play the same whatever the config file
        // says: the fix is off unless the command line turns it on.
        match args.mid_air_hit_fix {
            Some(on) => {
                s.mid_air_hit_fix = on;
                s.from_command_line.insert("mid_air_hit_fix");
            }
            None if s.mode != Mode::Play => s.mid_air_hit_fix = false,
            None => {}
        }
        // The pause and screen switches the same way; a replay follows the
        // lines its recording carries.
        // The aids the same way; a replay plays with those its recording
        // names (see `Settings::follow_header`).
        if s.mode != Mode::Play {
            s.pause_input = PauseInput::Original;
            s.fuse_clock = FuseClock::Original;
            s.aids = PlayAids::NONE;
        }
        if let Mode::Replay { script, .. } = &s.mode
            && let Ok(text) = fs::read_to_string(script)
        {
            let recorded = crate::session::script_switches(&text);
            if let Some(p) = recorded.pause_input {
                s.pause_input = p;
            }
            if let Some(f) = recorded.fuse_clock {
                s.fuse_clock = f;
            }
        }
        // An aid on the command line is on for this run only.
        if args.invincible {
            s.aids.invincible = true;
            s.from_command_line.insert(PlayAids::INVINCIBLE);
        }
        if args.infinite_lives {
            s.aids.infinite_lives = true;
            s.from_command_line.insert(PlayAids::INFINITE_LIVES);
        }
        if let Some(p) = args.pause_input {
            s.pause_input = p;
            s.from_command_line.insert("pause_input");
        }
        if let Some(f) = args.fuse_clock {
            s.fuse_clock = f;
            s.from_command_line.insert("fuse_clock");
        }
        Ok(s)
    }

    /// Takes the mid-air hit jump fix a replayed script was recorded with
    /// (see [`crate::session::script_behaviour`]), unless the command line
    /// set it.
    pub fn follow_script(&mut self, recorded: Option<bool>) {
        if let Some(on) = recorded
            && !self.from_command_line.contains("mid_air_hit_fix")
        {
            self.mid_air_hit_fix = on;
        }
    }

    /// Takes the movement tuning, the seed and the play-test aids a
    /// replayed script's header names; keys the header does not have leave
    /// the settings as they are. An aid the header names is turned on (one
    /// the command line turned on stays on).
    pub fn follow_header(&mut self, header: &ScriptHeader) {
        if header.tuning.is_some() {
            self.tuning = header.tuning;
        }
        if header.seed.is_some() {
            self.seed = header.seed;
        }
        self.aids = self.aids.union(header.aids);
    }

    /// Plays level recording `rec` as it was recorded: its level
    /// (`# level N`) and its start point (`# start_at`), the mid-air hit
    /// jump fix and the pause and screen switches its lines name unless the
    /// command line set them, and the tuning, seed and play-test aids its
    /// header names. Its
    /// carried progress and save point are the replay's to apply (see
    /// [`crate::session::LevelRecording::start_sim`]).
    ///
    /// A recording never plays from a start it was not made with: a level
    /// on the command line other than the one it names, or a `--start`
    /// point other than the one it names or with its save point, is an
    /// error. A recording that names no level plays the settings' level,
    /// and one that names no start point keeps a `--start` point, as
    /// before.
    pub fn follow_recording(&mut self, rec: &crate::session::LevelRecording) -> Result<()> {
        if let Some(n) = rec.level() {
            let named = LevelChoice::Retail(n);
            if self.from_command_line.contains("level") && self.level != named {
                let given = match &self.level {
                    LevelChoice::Retail(m) => format!("--level {m}"),
                    LevelChoice::Custom(name) => format!("--custom {name}"),
                };
                bail!("the recording plays level {n} (its `# level {n}` line), not {given}");
            }
            self.level = named;
        }
        match (rec.start_at, self.start) {
            (Some(at), Some(given)) if at != given => bail!(
                "the recording starts Claw at {},{} (its `# start_at` line), not --start {},{}",
                at.0,
                at.1,
                given.0,
                given.1
            ),
            (Some(at), _) => self.start = Some(at),
            (None, Some(_)) if rec.start.save.is_some() => {
                bail!("the recording starts at a save point; --start cannot move Claw from it")
            }
            (None, _) => {}
        }
        self.follow_script(rec.fix);
        self.follow_header(&rec.script.header);
        if let Some(p) = rec.switches.pause_input
            && !self.from_command_line.contains("pause_input")
        {
            self.pause_input = p;
        }
        if let Some(f) = rec.switches.fuse_clock
            && !self.from_command_line.contains("fuse_clock")
        {
            self.fuse_clock = f;
        }
        Ok(())
    }

    /// The view for an output of `size` pixels: the original 640x480 game
    /// view presented by the aspect and scaling settings.
    pub fn view(&self, size: (u32, u32)) -> ViewConfig {
        view_for(self.aspect, self.scale, self.widen_activation, size)
    }

    /// The audio settings: sound on or off, the volumes, the speech and
    /// ambient switches and the SoundFont.
    pub fn audio_settings(&self) -> AudioSettings {
        AudioSettings {
            enabled: self.audio,
            sound_volume: self.sound_volume,
            music_volume: self.music_volume,
            voice: self.voice,
            ambient: self.ambient,
            soundfont: self.soundfont.clone(),
            ..AudioSettings::default()
        }
    }

    /// The config file value of `key` for the current settings, as
    /// [`ConfigFile::update_in_place`] writes it; `None` for a key that is
    /// not written back (the level choice, a removed frame-rate cap, an
    /// unset path) or unknown.
    pub fn config_value(&self, key: &str) -> Option<String> {
        let on_off = |b: bool| if b { "on" } else { "off" }.to_string();
        let path = |p: &Option<PathBuf>| p.as_ref().and_then(|p| p.to_str()).map(String::from);
        Some(match key {
            "install" => self.install.to_str().filter(|p| !p.is_empty())?.to_string(),
            "res" => format!("{}x{}", self.res.0, self.res.1),
            "fullscreen" => self.fullscreen.to_string(),
            "aspect" => self.aspect.name().to_string(),
            "scale" => match self.scale {
                Scale::Fit => "fit",
                Scale::Integer => "integer",
            }
            .to_string(),
            "widen_activation" => self.widen_activation.to_string(),
            "mid_air_hit_fix" => on_off(self.mid_air_hit_fix),
            "pause_input" => self.pause_input.name().to_string(),
            "fuse_clock" => self.fuse_clock.name().to_string(),
            PlayAids::INVINCIBLE => on_off(self.aids.invincible),
            PlayAids::INFINITE_LIVES => on_off(self.aids.infinite_lives),
            "vsync" => self.vsync.to_string(),
            "fps_cap" => self.fps_cap?.to_string(),
            "interpolation" => self.interpolation.to_string(),
            "status_bar" => if self.status_bar_centred {
                "centred"
            } else {
                "edges"
            }
            .to_string(),
            "front_layer" => on_off(self.front_layer),
            "pack" => path(&self.pack)?,
            "audio" => on_off(self.audio),
            "sound_volume" => self.sound_volume.to_string(),
            "music_volume" => self.music_volume.to_string(),
            "voice" => on_off(self.voice),
            "ambient" => on_off(self.ambient),
            "soundfont" => path(&self.soundfont)?,
            "gamepad" => on_off(self.pad.enabled),
            "pad_dead_zone" => format!("{}", self.pad.dead_zone),
            k => {
                if let Some(a) = k.strip_prefix("key_").and_then(Action::from_name) {
                    self.keymap.config_value(a)
                } else if let Some(a) = k.strip_prefix("pad_").and_then(Action::from_name) {
                    self.pad.map.config_value(a)
                } else {
                    return None;
                }
            }
        })
    }

    /// Marks `key` as changed in the game, so that it is written back even
    /// though the command line set it at start-up.
    pub fn changed_in_game(&mut self, key: &str) {
        self.from_command_line.retain(|k| *k != key);
    }

    /// The `(key, value)` pairs [`Settings::write_back`] writes for `keys`:
    /// every key whose value can be written and did not come from the
    /// command line.
    pub fn write_back_values(&self, keys: &[&str]) -> Vec<(String, String)> {
        keys.iter()
            .filter(|k| !self.from_command_line.contains(**k))
            .filter_map(|k| Some((k.to_string(), self.config_value(k)?)))
            .collect()
    }

    /// Writes the current values of `keys` back to the config file (see
    /// [`Settings::write_back_values`]); returns the keys written.
    pub fn write_back(&self, keys: &[&str]) -> Result<Vec<String>> {
        let path = self
            .config_path
            .as_ref()
            .ok_or_else(|| anyhow!("no config file location: set HOME or pass --config FILE"))?;
        let values = self.write_back_values(keys);
        if values.is_empty() {
            return Ok(Vec::new());
        }
        ConfigFile::update_in_place(path, &values)?;
        Ok(values.into_iter().map(|(k, _)| k).collect())
    }

    /// True when the active area follows the first output's shape.
    pub fn widens(&self) -> bool {
        self.widen_activation && self.aspect == Aspect::Fill
    }

    /// The simulation settings for a first output of `size` pixels.
    ///
    /// The game view is always 640x480. Without widened activation the
    /// simulation is the original one, and objects outside the active area
    /// are drawn still up to the width of a 32:9 display (or the output, if
    /// wider), so the display shape can change at run time. With widened
    /// activation the active area follows the first output's width and stays
    /// so until the level is loaded again; objects outside it are drawn
    /// still across the same reach as without widening, so a wider shape
    /// chosen at run time still shows them.
    ///
    /// The movement tuning and the seed follow [`Settings::tuning`] and
    /// [`Settings::seed`], the mid-air hit jump fix follows
    /// [`Settings::mid_air_hit_fix`], the fuse clock
    /// [`Settings::fuse_clock`] and the play-test aids [`Settings::aids`].
    pub fn sim_config(&self, size: (u32, u32)) -> SimConfig {
        let mut cfg = self.view_sim_config(size);
        if let Some(t) = self.tuning {
            cfg.tuning = t.tuning();
        }
        if let Some(seed) = self.seed {
            cfg.seed = seed;
        }
        cfg.tuning.behaviour.fix_double_jump = self.mid_air_hit_fix;
        cfg.fuse_clock = self.fuse_clock;
        cfg.aids = self.aids;
        cfg
    }

    /// [`Settings::sim_config`] before the behaviour switches.
    fn view_sim_config(&self, size: (u32, u32)) -> SimConfig {
        let frozen = ViewConfig {
            aspect_policy: AspectPolicy::FrozenEdgeClamped,
            ..ViewConfig::default()
        };
        let (pw, _) = frozen.with_render_size(size.0, size.1).presentation_size();
        let reach = pw.max(FROZEN_EDGE_REACH);
        if self.widens() {
            // The activation comes from the first output; the drawing reach
            // is the one the frozen-edge simulation uses.
            let first = self.view(size);
            let mut cfg = SimConfig::for_view(first);
            cfg.view = first.with_render_size(reach, DEFAULT_LOGIC_HEIGHT);
            return cfg;
        }
        SimConfig::for_view(frozen.with_render_size(reach, DEFAULT_LOGIC_HEIGHT))
    }
}

/// The view for an output of `size` with the given presentation settings.
pub fn view_for(aspect: Aspect, scale: Scale, widen: bool, size: (u32, u32)) -> ViewConfig {
    ViewConfig {
        logic_width: DEFAULT_LOGIC_WIDTH,
        logic_height: DEFAULT_LOGIC_HEIGHT,
        render_width: size.0.max(1),
        render_height: size.1.max(1),
        aspect_policy: match aspect {
            Aspect::Fill if widen => AspectPolicy::WidenedActivation,
            Aspect::Fill => AspectPolicy::FrozenEdgeClamped,
            Aspect::Centered => AspectPolicy::FrozenEdgeCentered,
            Aspect::Classic => AspectPolicy::Pillarbox,
        },
        scale_mode: match scale {
            Scale::Fit => ScaleMode::Fit,
            Scale::Integer => ScaleMode::Integer,
        },
    }
}

/// The user's folders for settings and saved games, from `HOME` and the
/// XDG base directory variables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserDirs {
    /// `$XDG_CONFIG_HOME`, or `$HOME/.config`.
    pub config_home: Option<PathBuf>,
    /// `$XDG_DATA_HOME`, or `$HOME/.local/share`.
    pub data_home: Option<PathBuf>,
}

impl UserDirs {
    /// The folders of this process's environment.
    pub fn from_env() -> UserDirs {
        UserDirs::from_lookup(|k| std::env::var_os(k))
    }

    /// The folders named by the variables `get` returns. An XDG variable
    /// that is unset, empty or relative falls back to its place under
    /// `HOME`, as the XDG rules say.
    pub fn from_lookup(get: impl Fn(&str) -> Option<OsString>) -> UserDirs {
        let home = get("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
        let xdg = |var: &str, under_home: &str| {
            get(var)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| home.as_ref().map(|h| h.join(under_home)))
        };
        UserDirs {
            config_home: xdg("XDG_CONFIG_HOME", ".config"),
            data_home: xdg("XDG_DATA_HOME", ".local/share"),
        }
    }

    /// The config file: `<config home>/rusty-claw/config.toml`.
    pub fn config_file(&self) -> Option<PathBuf> {
        Some(
            self.config_home
                .as_ref()?
                .join("rusty-claw")
                .join("config.toml"),
        )
    }

    /// The saved games of the default profile:
    /// `<data home>/rusty-claw/saves/default`.
    pub fn saves_dir(&self) -> Option<PathBuf> {
        Some(
            self.data_home
                .as_ref()?
                .join("rusty-claw")
                .join("saves")
                .join(crate::saves::DEFAULT_PROFILE),
        )
    }
}

/// The default config file location.
pub fn default_config_path() -> Option<PathBuf> {
    UserDirs::from_env().config_file()
}

/// A parsed config file: `key = value` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigFile {
    /// Values by key, unquoted.
    pub values: BTreeMap<String, String>,
    /// Where the file came from, for messages.
    pub origin: String,
}

/// Keys a config file may hold, besides the `key_<action>` and
/// `pad_<action>` bindings.
pub const CONFIG_KEYS: [&str; 27] = [
    "install",
    "level",
    "custom",
    "res",
    "fullscreen",
    "aspect",
    "scale",
    "widen_activation",
    "mid_air_hit_fix",
    "pause_input",
    "fuse_clock",
    PlayAids::INVINCIBLE,
    PlayAids::INFINITE_LIVES,
    "vsync",
    "fps_cap",
    "interpolation",
    "status_bar",
    "front_layer",
    "pack",
    "audio",
    "sound_volume",
    "music_volume",
    "voice",
    "ambient",
    "soundfont",
    "gamepad",
    "pad_dead_zone",
];

/// True when a config file may hold `key`.
pub fn is_config_key(key: &str) -> bool {
    CONFIG_KEYS.contains(&key)
        || key
            .strip_prefix("key_")
            .or_else(|| key.strip_prefix("pad_"))
            .and_then(Action::from_name)
            .is_some()
}

/// Largest config file read, in bytes.
pub const MAX_CONFIG_BYTES: u64 = 1 << 20;

/// How many times a write-back re-reads and merges a file that keeps
/// changing on disk before it gives up.
const MERGE_ATTEMPTS: usize = 8;

impl ConfigFile {
    /// Reads and parses `path`.
    pub fn load(path: &Path) -> Result<ConfigFile> {
        let bytes = read_bounded(path, MAX_CONFIG_BYTES)
            .and_then(|b| b.ok_or_else(|| std::io::ErrorKind::NotFound.into()))
            .with_context(|| format!("reading config file {}", path.display()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| anyhow!("config file {} is not UTF-8 text", path.display()))?;
        ConfigFile::parse(&text, &path.display().to_string())
    }

    /// Parses config text; `origin` names it in messages.
    pub fn parse(text: &str, origin: &str) -> Result<ConfigFile> {
        let mut values = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                bail!("{origin}:{}: expected key = value", i + 1);
            };
            let key = key.trim();
            if !is_config_key(key) {
                bail!(
                    "{origin}:{}: unknown setting {key:?} (known: {}, key_<action>, pad_<action>)",
                    i + 1,
                    CONFIG_KEYS.join(", ")
                );
            }
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            values.insert(key.to_string(), value.to_string());
        }
        Ok(ConfigFile {
            values,
            origin: origin.to_string(),
        })
    }

    /// Applies the file to `s`; returns the install path if it names one.
    pub fn apply(&self, s: &mut Settings) -> Result<Option<PathBuf>> {
        let err = |key: &str, e: String| anyhow!("{}: {key}: {e}", self.origin);
        let flag = |key: &str, v: &str| parse_on_off(v).map_err(|e| err(key, e));
        let volume = |key: &str, v: &str| match v.parse::<u8>() {
            Ok(n) if n <= 100 => Ok(n),
            _ => Err(err(
                key,
                format!("expected a volume from 0 to 100, got {v:?}"),
            )),
        };
        let mut install = None;
        for (key, v) in &self.values {
            match key.as_str() {
                "install" => install = Some(PathBuf::from(v)),
                "level" => {
                    let n = v
                        .parse()
                        .map_err(|_| err(key, format!("bad level {v:?}")))?;
                    s.level = LevelChoice::Retail(n);
                }
                "res" => s.res = parse_size(v).map_err(|e| err(key, e))?,
                "fullscreen" => s.fullscreen = flag(key, v)?,
                "aspect" => s.aspect = Aspect::from_str(v, true).map_err(|e| err(key, e))?,
                "scale" => s.scale = Scale::from_str(v, true).map_err(|e| err(key, e))?,
                "widen_activation" => s.widen_activation = flag(key, v)?,
                "mid_air_hit_fix" => s.mid_air_hit_fix = flag(key, v)?,
                "pause_input" => s.pause_input = parse_pause_input(v).map_err(|e| err(key, e))?,
                "fuse_clock" => s.fuse_clock = parse_fuse_clock(v).map_err(|e| err(key, e))?,
                PlayAids::INVINCIBLE => s.aids.invincible = flag(key, v)?,
                PlayAids::INFINITE_LIVES => s.aids.infinite_lives = flag(key, v)?,
                "vsync" => s.vsync = flag(key, v)?,
                "fps_cap" => {
                    let n = v
                        .parse()
                        .map_err(|_| err(key, format!("bad frame rate {v:?}")))?;
                    if n == 0 {
                        return Err(err(
                            key,
                            "0 (no cap) is only accepted as --fps-cap 0 or --uncapped".into(),
                        ));
                    }
                    s.fps_cap = fps_cap(n).map_err(|e| err(key, e))?;
                }
                "interpolation" => s.interpolation = flag(key, v)?,
                "status_bar" => {
                    s.status_bar_centred = match v.to_ascii_lowercase().as_str() {
                        "edges" => false,
                        "centred" | "centered" => true,
                        _ => {
                            return Err(err(key, format!("expected edges or centred, got {v:?}")));
                        }
                    }
                }
                "front_layer" => s.front_layer = flag(key, v)?,
                "pack" => s.pack = Some(PathBuf::from(v)),
                "audio" => s.audio = flag(key, v)?,
                "sound_volume" => s.sound_volume = volume(key, v)?,
                "music_volume" => s.music_volume = volume(key, v)?,
                "voice" => s.voice = flag(key, v)?,
                "ambient" => s.ambient = flag(key, v)?,
                "soundfont" => s.soundfont = Some(PathBuf::from(v)),
                "gamepad" => s.pad.enabled = flag(key, v)?,
                "pad_dead_zone" => {
                    s.pad.dead_zone = match v.parse::<f32>() {
                        Ok(z) if PAD_DEAD_ZONE_RANGE.contains(&z) => z,
                        _ => {
                            return Err(err(
                                key,
                                format!("expected a dead zone from 0.05 to 0.95, got {v:?}"),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        s.keymap =
            Keymap::from_config(&self.values).map_err(|e| anyhow!("{}: {e}", self.origin))?;
        s.pad.map =
            PadMap::from_config(&self.values).map_err(|e| anyhow!("{}: {e}", self.origin))?;
        // A custom level in the file wins over a level number in the file.
        if let Some(name) = self.values.get("custom") {
            s.level = LevelChoice::Custom(name.clone());
        }
        Ok(install)
    }

    /// Checks that every value of the file is valid.
    pub fn check(&self) -> Result<()> {
        let mut s = Settings::new(PathBuf::new(), LevelChoice::default());
        self.apply(&mut s).map(|_| ())
    }

    /// Writes `updates` (`(key, value)` pairs, values unquoted) into the
    /// config file at `path`, keeping it as it is otherwise:
    ///
    /// - a line of an updated key gets the new value in place, keeping its
    ///   spacing, quotes and comment; keys missing from the file are
    ///   appended at its end; every other line stays byte for byte;
    /// - a file that does not parse, is not UTF-8 or is larger than
    ///   [`MAX_CONFIG_BYTES`] is never written, nor is a value that would
    ///   not load;
    /// - the new file is written to a temporary file in the same folder,
    ///   flushed to disk and renamed over the old one, after the old one
    ///   is kept as `<name>.bak`; the folder is created when missing;
    /// - when the file changes on disk while this runs, the update is made
    ///   again on the new contents, so that change is kept.
    ///
    /// A symbolic link is followed, and the file it points to is updated.
    pub fn update_in_place<K: AsRef<str>, V: AsRef<str>>(
        path: &Path,
        updates: &[(K, V)],
    ) -> Result<()> {
        ConfigFile::update_in_place_with(path, updates, &mut WriteHooks::default())
    }

    /// [`ConfigFile::update_in_place`] with test hooks.
    pub fn update_in_place_with<K: AsRef<str>, V: AsRef<str>>(
        path: &Path,
        updates: &[(K, V)],
        hooks: &mut WriteHooks<'_>,
    ) -> Result<()> {
        // The last value of a key given twice wins.
        let mut pairs: Vec<(String, String)> = Vec::new();
        for (k, v) in updates {
            let (k, v) = (k.as_ref().trim(), v.as_ref());
            if !is_config_key(k) {
                bail!("not writing {}: unknown setting {k:?}", path.display());
            }
            if v.contains(['"', '\n', '\r']) || v.chars().any(char::is_control) {
                bail!(
                    "not writing {}: the value of {k} cannot be written: {v:?}",
                    path.display()
                );
            }
            pairs.retain(|(p, _)| p != k);
            pairs.push((k.to_string(), v.to_string()));
        }
        let target = follow_link(path);
        let origin = target.display().to_string();
        for _ in 0..MERGE_ATTEMPTS {
            let current = read_bounded(&target, MAX_CONFIG_BYTES)
                .with_context(|| format!("reading config file {origin}"))?;
            let text = match &current {
                None => String::new(),
                Some(b) => std::str::from_utf8(b)
                    .map_err(|_| anyhow!("not writing {origin}: it is not UTF-8 text"))?
                    .to_string(),
            };
            let old = ConfigFile::parse(&text, &origin)
                .with_context(|| format!("not writing {origin}: it does not parse"))?;
            let new_text = patch_config(&text, &pairs);
            let new = ConfigFile::parse(&new_text, &origin)
                .with_context(|| format!("not writing {origin}: the update does not parse"))?;
            let mut want = old.values.clone();
            for (k, v) in &pairs {
                want.insert(k.clone(), v.clone());
            }
            if new.values != want {
                bail!("not writing {origin}: the update would change other settings");
            }
            new.check()
                .with_context(|| format!("not writing {origin}: a value would not load"))?;
            match commit_file(&target, new_text.as_bytes(), Some(current), hooks)? {
                Commit::Written => return Ok(()),
                Commit::Changed => continue,
            }
        }
        bail!("not writing {origin}: it kept changing while being written")
    }
}

/// Puts `pairs` into config `text` (see [`ConfigFile::update_in_place`]).
fn patch_config(text: &str, pairs: &[(String, String)]) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut written = vec![false; pairs.len()];
    let mut out = String::with_capacity(text.len() + 64);
    for raw in text.split_inclusive('\n') {
        let content = raw
            .strip_suffix('\n')
            .map(|c| c.strip_suffix('\r').unwrap_or(c))
            .unwrap_or(raw);
        let body = strip_comment(content).trim_end();
        let patched = body.split_once('=').and_then(|(key, after)| {
            let i = pairs.iter().position(|(k, _)| k == key.trim())?;
            written[i] = true;
            let start = key.len() + 1 + (after.len() - after.trim_start().len());
            let old = &body[start..];
            let quoted = old.len() >= 2 && old.starts_with('"') && old.ends_with('"');
            Some(format!(
                "{}{}{}",
                &raw[..start],
                config_value_text(&pairs[i].1, quoted),
                &raw[body.len()..]
            ))
        });
        out.push_str(patched.as_deref().unwrap_or(raw));
    }
    let mut missing = pairs.iter().zip(&written).filter(|(_, w)| !**w).peekable();
    if missing.peek().is_some() && !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    for ((k, v), _) in missing {
        out.push_str(&format!("{k} = {}{eol}", config_value_text(v, false)));
    }
    out
}

/// A value as written in the file: quoted when asked, or when it is
/// empty, has spaces or holds a `#` or `=`.
fn config_value_text(v: &str, quoted: bool) -> String {
    if quoted || v.is_empty() || v.contains(|c: char| c.is_whitespace() || c == '#' || c == '=') {
        format!("\"{v}\"")
    } else {
        v.to_string()
    }
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// A step of an atomic file write, for tests that interrupt one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStep {
    /// Creating the temporary file.
    Temp,
    /// Writing the new contents.
    Data,
    /// Flushing the new contents to disk.
    Flush,
    /// Keeping the old file as the backup.
    Backup,
    /// Renaming the new file over the old one.
    Rename,
    /// Flushing the folder to disk.
    SyncDir,
}

/// Test hooks of an atomic write.
#[derive(Default)]
pub struct WriteHooks<'a> {
    /// Fail with an injected error at this step.
    pub fail_at: Option<WriteStep>,
    /// Called with the file's path once the new contents are on disk in
    /// the temporary file, just before the file is checked for changes and
    /// replaced.
    pub before_commit: Option<&'a mut dyn FnMut(&Path)>,
}

impl fmt::Debug for WriteHooks<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriteHooks")
            .field("fail_at", &self.fail_at)
            .field("before_commit", &self.before_commit.is_some())
            .finish()
    }
}

impl WriteHooks<'_> {
    fn step(&self, step: WriteStep) -> std::io::Result<()> {
        if self.fail_at == Some(step) {
            return Err(std::io::Error::other(format!(
                "injected failure at {step:?}"
            )));
        }
        Ok(())
    }
}

/// The outcome of [`commit_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commit {
    /// The file now holds the new contents.
    Written,
    /// The file changed since it was read; nothing was written.
    Changed,
}

/// Reads at most `max` bytes of `path`: `None` when the file does not
/// exist, an error when it is not a regular file or is larger.
pub fn read_bounded(path: &Path, max: u64) -> std::io::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("larger than {max} bytes"),
        ));
    }
    Ok(Some(bytes))
}

/// The file a symbolic link at `path` points to, or `path`.
fn follow_link(path: &Path) -> PathBuf {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path).unwrap_or_else(|_| {
            fs::read_link(path)
                .map(|t| path.parent().unwrap_or(Path::new(".")).join(t))
                .unwrap_or_else(|_| path.to_path_buf())
        }),
        _ => path.to_path_buf(),
    }
}

/// A temporary file name next to `path`, unique within this process and
/// across processes.
fn temp_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()))
}

/// The folder holding `path`.
fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Removes a temporary file unless it was renamed into place.
struct TempFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Writes `bytes` to a new temporary file next to `path` and flushes it.
/// With `permissions`, the file gets them before any byte is written, so a
/// private file is never readable by others, not even as a copy.
fn write_temp(
    path: &Path,
    bytes: &[u8],
    permissions: Option<&fs::Permissions>,
    hooks: &WriteHooks<'_>,
) -> Result<TempFile> {
    hooks.step(WriteStep::Temp)?;
    let tmp = temp_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .with_context(|| format!("creating {}", tmp.display()))?;
    let guard = TempFile {
        path: tmp,
        armed: true,
    };
    if let Some(p) = permissions {
        file.set_permissions(p.clone())
            .with_context(|| format!("setting the permissions of {}", guard.path.display()))?;
    }
    hooks.step(WriteStep::Data)?;
    file.write_all(bytes)
        .with_context(|| format!("writing {}", guard.path.display()))?;
    hooks.step(WriteStep::Flush)?;
    file.sync_all()
        .with_context(|| format!("flushing {}", guard.path.display()))?;
    Ok(guard)
}

/// Renames the temporary file over `path` and flushes the folder. Once the
/// rename succeeded the new contents are in place, so a failure to flush
/// the folder afterwards is only reported as a warning.
fn rename_into_place(mut tmp: TempFile, path: &Path, hooks: &WriteHooks<'_>) -> Result<()> {
    hooks.step(WriteStep::Rename)?;
    fs::rename(&tmp.path, path)
        .with_context(|| format!("replacing {} with {}", path.display(), tmp.path.display()))?;
    tmp.armed = false;
    let dir = parent_dir(path);
    let synced = hooks
        .step(WriteStep::SyncDir)
        .and_then(|()| File::open(dir))
        .and_then(|d| d.sync_all());
    if let Err(e) = synced {
        eprintln!(
            "rusty-claw: {} was written, but its folder {} could not be flushed to disk: {e}",
            path.display(),
            dir.display()
        );
    }
    Ok(())
}

/// The backup kept of `path`: `<name>.bak` next to it.
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Replaces `path` with `bytes` atomically, keeping the old contents in
/// the backup file. With `expect`, the file must still hold those
/// contents (`None`: must not exist) when it is replaced; otherwise
/// nothing is written and [`Commit::Changed`] is returned.
fn commit_file(
    path: &Path,
    bytes: &[u8],
    expect: Option<Option<Vec<u8>>>,
    hooks: &mut WriteHooks<'_>,
) -> Result<Commit> {
    let dir = parent_dir(path);
    fs::create_dir_all(dir).with_context(|| format!("creating the folder {}", dir.display()))?;
    // The new file and the backup keep the permissions of the file they
    // replace.
    let permissions = fs::metadata(path).ok().map(|m| m.permissions());
    let tmp = write_temp(path, bytes, permissions.as_ref(), hooks)?;
    if let Some(f) = hooks.before_commit.as_mut() {
        f(path);
    }
    let current = read_bounded(path, MAX_CONFIG_BYTES)
        .with_context(|| format!("reading {}", path.display()))?;
    if let Some(expect) = expect
        && current != expect
    {
        return Ok(Commit::Changed);
    }
    hooks.step(WriteStep::Backup)?;
    if let Some(old) = &current {
        let bak = backup_path(path);
        let mut bak_tmp = write_temp(&bak, old, permissions.as_ref(), &WriteHooks::default())?;
        fs::rename(&bak_tmp.path, &bak).with_context(|| format!("writing {}", bak.display()))?;
        bak_tmp.armed = false;
    }
    rename_into_place(tmp, path, hooks)?;
    Ok(Commit::Written)
}

/// Replaces `path` with `bytes` atomically: a temporary file in the same
/// folder, flushed to disk and renamed over `path`, after the old contents
/// are kept as `<name>.bak`. The folder is created when missing.
pub fn write_atomic(path: &Path, bytes: &[u8], hooks: &mut WriteHooks<'_>) -> Result<()> {
    commit_file(path, bytes, None, hooks).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Args {
        Args::try_parse_from(std::iter::once("rusty-claw").chain(s.split_whitespace())).unwrap()
    }

    #[test]
    fn command_line_defaults() {
        let s = Settings::resolve(&args("--install /i"), None).unwrap();
        assert_eq!(s.install, PathBuf::from("/i"));
        assert_eq!(s.level, LevelChoice::Retail(1));
        assert_eq!(s.res, DEFAULT_WINDOW);
        assert_eq!(s.aspect, Aspect::Fill);
        assert_eq!(s.scale, Scale::Fit);
        assert!(s.vsync && s.interpolation && !s.fullscreen && !s.widen_activation);
        assert_eq!(s.mode, Mode::Play);
    }

    #[test]
    fn install_is_required() {
        let e = Settings::resolve(&args("--level 2"), None).unwrap_err();
        assert!(e.to_string().contains("--install"));
    }

    #[test]
    fn command_line_modes() {
        let s = Settings::resolve(
            &args("--install /i --custom a.wwd --replay s.csv --trace t.csv --render-hz 144"),
            None,
        )
        .unwrap();
        assert_eq!(s.level, LevelChoice::Custom("a.wwd".into()));
        assert_eq!(
            s.mode,
            Mode::Replay {
                script: "s.csv".into(),
                trace: Some("t.csv".into()),
                render_hz: 144.0
            }
        );
        let s = Settings::resolve(&args("--install /i --bench 10 --res 3840x2160"), None).unwrap();
        assert_eq!(s.res, (3840, 2160));
        assert_eq!(
            s.mode,
            Mode::Bench {
                seconds: 10.0,
                headless: false,
                render_hz: 144.0
            }
        );
        assert!(Settings::resolve(&args("--install /i --bench 0"), None).is_err());
        assert!(Settings::resolve(&args("--install /i --render-hz 60"), None).is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--level", "1", "--custom", "x"]).is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--trace", "t.csv"]).is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--headless"]).is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--res", "12"]).is_err());
    }

    #[test]
    fn aspect_names() {
        let a = |s: &str| args(&format!("--install /i --aspect {s}")).aspect;
        assert_eq!(a("fill"), Some(Aspect::Fill));
        assert_eq!(a("native"), Some(Aspect::Fill));
        assert_eq!(a("centered"), Some(Aspect::Centered));
        assert_eq!(a("4:3"), Some(Aspect::Classic));
        assert_eq!(Aspect::Classic.next(), Aspect::Fill);
    }

    #[test]
    fn config_file_then_command_line() {
        let file = ConfigFile::parse(
            "# my settings\ninstall = \"/games/claw # 1\"\nlevel = 3\nres = \"2560x1440\"\n\
             fullscreen = true  # big\naspect = \"4:3\"\nscale = integer\nvsync = off\n\
             interpolation = false\n",
            "test",
        )
        .unwrap();
        let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
        assert_eq!(s.install, PathBuf::from("/games/claw # 1"));
        assert_eq!(s.level, LevelChoice::Retail(3));
        assert_eq!(s.res, (2560, 1440));
        assert!(s.fullscreen && !s.vsync && !s.interpolation);
        assert_eq!((s.aspect, s.scale), (Aspect::Classic, Scale::Integer));
        let s = Settings::resolve(
            &args("--install /x --level 5 --windowed --aspect fill --vsync on"),
            Some(&file),
        )
        .unwrap();
        assert_eq!(s.install, PathBuf::from("/x"));
        assert_eq!(s.level, LevelChoice::Retail(5));
        assert!(!s.fullscreen && s.vsync);
        assert_eq!(s.aspect, Aspect::Fill);
    }

    #[test]
    fn config_file_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "install = \"/games/claw\"\nscale = \"integer\"\n").unwrap();
        let a = args(&format!("--config {} --level 4", path.display()));
        let s = Settings::from_args(&a).unwrap();
        assert_eq!(s.install, PathBuf::from("/games/claw"));
        assert_eq!(s.level, LevelChoice::Retail(4));
        assert_eq!(s.scale, Scale::Integer);
        let missing = args(&format!("--config {}", dir.path().join("none").display()));
        assert!(Settings::from_args(&missing).is_err());
    }

    #[test]
    fn config_file_errors_name_the_line() {
        let e = ConfigFile::parse("install = /i\nzoom = 3\n", "cfg").unwrap_err();
        assert!(e.to_string().starts_with("cfg:2: unknown setting"), "{e}");
        assert!(ConfigFile::parse("just words\n", "cfg").is_err());
        let file = ConfigFile::parse("install = /i\nres = big\n", "cfg").unwrap();
        assert!(Settings::resolve(&Args::default(), Some(&file)).is_err());
        let file = ConfigFile::parse("install = /i\ncustom = \"A b.WWD\"\n", "cfg").unwrap();
        let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
        assert_eq!(s.level, LevelChoice::Custom("A b.WWD".into()));
    }

    #[test]
    fn views_keep_the_original_game_view() {
        let mut s = Settings::new("/i", LevelChoice::Retail(1));
        for size in WINDOW_SIZES {
            for aspect in [Aspect::Fill, Aspect::Centered, Aspect::Classic] {
                s.aspect = aspect;
                let v = s.view(size);
                assert_eq!((v.logic_width, v.logic_height), (640, 480));
                assert_eq!((v.render_width, v.render_height), size);
            }
        }
        s.aspect = Aspect::Fill;
        assert_eq!(s.view((3840, 2160)).presentation_size(), (854, 480));
        assert_eq!(s.view((5120, 1440)).presentation_size(), (1708, 480));
        s.aspect = Aspect::Classic;
        assert_eq!(s.view((5120, 1440)).presentation_size(), (640, 480));
    }

    #[test]
    fn simulation_keeps_the_original_activity_unless_widened() {
        use claw_sim::ActivationMode;
        let mut s = Settings::new("/i", LevelChoice::Retail(1));
        for aspect in [Aspect::Fill, Aspect::Centered, Aspect::Classic] {
            s.aspect = aspect;
            let cfg = s.sim_config((1280, 960));
            assert_eq!(cfg.activation.mode, ActivationMode::FrozenEdge);
            assert_eq!(cfg.activation.view, (640, 480));
            assert_eq!(
                claw_sim::sim::presentation_width(&cfg.view),
                FROZEN_EDGE_REACH
            );
        }
        s.aspect = Aspect::Fill;
        let cfg = s.sim_config((7680, 1080));
        assert_eq!(claw_sim::sim::presentation_width(&cfg.view), 3414);
        s.widen_activation = true;
        let cfg = s.sim_config((5120, 1440));
        assert_eq!(cfg.activation.mode, ActivationMode::Widened);
        assert_eq!(cfg.activation.view, (1708, 480));
        assert_eq!(cfg.view.logic_width, 640);
        assert_eq!(
            claw_sim::sim::presentation_width(&cfg.view),
            FROZEN_EDGE_REACH
        );
        // Widening needs the fill policy; the other policies draw only
        // the original view's width of live objects.
        s.aspect = Aspect::Classic;
        assert_eq!(
            s.sim_config((5120, 1440)).activation.mode,
            ActivationMode::FrozenEdge
        );
    }

    #[test]
    fn widened_activation_draws_still_objects_across_any_later_shape() {
        use claw_sim::ActivationMode;
        let mut s = Settings::new("/i", LevelChoice::Retail(1));
        s.widen_activation = true;
        // A 4:3 first window: nothing to widen, but the drawing reach
        // covers a 32:9 shape chosen later, as without widening.
        let cfg = s.sim_config(DEFAULT_WINDOW);
        assert_eq!(cfg.activation.mode, ActivationMode::Widened);
        assert_eq!(cfg.activation.view, (640, 480));
        assert_eq!(
            claw_sim::sim::presentation_width(&cfg.view),
            FROZEN_EDGE_REACH
        );
        s.widen_activation = false;
        assert_eq!(
            s.sim_config(DEFAULT_WINDOW).view.render_width,
            cfg.view.render_width
        );
        // A 16:9 first window widens to its own shape and still draws to
        // the 32:9 reach; an output wider than that sets both.
        s.widen_activation = true;
        let cfg = s.sim_config((3840, 2160));
        assert_eq!(cfg.activation.view, (854, 480));
        assert_eq!(
            claw_sim::sim::presentation_width(&cfg.view),
            FROZEN_EDGE_REACH
        );
        let cfg = s.sim_config((7680, 1080));
        assert_eq!(cfg.activation.view, (3414, 480));
        assert_eq!(claw_sim::sim::presentation_width(&cfg.view), 3414);
    }

    #[test]
    fn audio_and_recording() {
        let s = Settings::resolve(&args("--install /i"), None).unwrap();
        assert!(s.audio && s.audio_settings().enabled);
        assert_eq!(s.record, None);
        let s = Settings::resolve(&args("--install /i --no-audio --record r.csv"), None).unwrap();
        assert!(!s.audio && !s.audio_settings().enabled);
        assert_eq!(s.record, Some(PathBuf::from("r.csv")));
        let file = ConfigFile::parse("install = /i\naudio = off\n", "cfg").unwrap();
        assert!(
            !Settings::resolve(&Args::default(), Some(&file))
                .unwrap()
                .audio
        );
        let file = ConfigFile::parse("install = /i\naudio = on\n", "cfg").unwrap();
        assert!(
            Settings::resolve(&Args::default(), Some(&file))
                .unwrap()
                .audio
        );
        assert!(
            Settings::resolve(&args("--no-audio"), Some(&file))
                .map(|s| !s.audio)
                .unwrap()
        );
        let file = ConfigFile::parse("install = /i\naudio = loud\n", "cfg").unwrap();
        assert!(Settings::resolve(&Args::default(), Some(&file)).is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--record", "r.csv", "--bench", "5"]).is_err());
        assert!(
            Args::try_parse_from([
                "rusty-claw",
                "--record",
                "r",
                "--replay",
                "s",
                "--trace",
                "t"
            ])
            .is_err()
        );
    }

    #[test]
    fn frame_rate_cap() {
        let cap = |a: &str| Settings::resolve(&args(&format!("--install /i {a}")), None);
        assert_eq!(cap("").unwrap().fps_cap, Some(DEFAULT_FPS_CAP));
        assert_eq!(cap("--fps-cap 144").unwrap().fps_cap, Some(144));
        assert_eq!(cap("--fps-cap 0").unwrap().fps_cap, None);
        assert_eq!(cap("--uncapped").unwrap().fps_cap, None);
        assert!(cap("--fps-cap 20000").is_err());
        assert!(Args::try_parse_from(["rusty-claw", "--fps-cap", "60", "--uncapped"]).is_err());
        let file = ConfigFile::parse("install = /i\nvsync = off\nfps_cap = 240\n", "cfg").unwrap();
        let s = Settings::resolve(&Args::default(), Some(&file)).unwrap();
        assert!(!s.vsync);
        assert_eq!(s.fps_cap, Some(240));
        let s = Settings::resolve(&args("--fps-cap 120"), Some(&file)).unwrap();
        assert_eq!(s.fps_cap, Some(120));
        let s = Settings::resolve(&args("--uncapped"), Some(&file)).unwrap();
        assert_eq!(s.fps_cap, None);
        let file = ConfigFile::parse("install = /i\nfps_cap = 0\n", "cfg").unwrap();
        assert!(Settings::resolve(&Args::default(), Some(&file)).is_err());
        let file = ConfigFile::parse("install = /i\nfps_cap = fast\n", "cfg").unwrap();
        assert!(Settings::resolve(&Args::default(), Some(&file)).is_err());
    }
}
