//! Settings: the command line, the optional config file and their defaults.
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
//! vsync = true
//! fps_cap = 300             # with vsync off, 1 to 10000
//! interpolation = true
//! ```
//!
//! With vsync on, the display's refresh paces drawing. With vsync off,
//! drawing is capped at `fps_cap` frames per second (300 by default);
//! `--fps-cap 0` or `--uncapped` removes the cap, which is meant only for
//! measuring throughput, so the settings file cannot remove it. The cap also paces headless benchmarks, which
//! draw at their simulated display rate unless `--uncapped` is given.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Parser, ValueEnum};
use claw_frame::{
    AspectPolicy, DEFAULT_FPS_CAP, DEFAULT_LOGIC_HEIGHT, DEFAULT_LOGIC_WIDTH, ScaleMode, ViewConfig,
};
use claw_sim::SimConfig;

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
    /// Put Claw at X,Y (level pixels) and let him settle before play or a
    /// replay starts.
    #[arg(long, value_name = "X,Y", value_parser = parse_point)]
    pub start: Option<(i32, i32)>,
    /// Play a recorded input script (columns `tick,held_mask,pressed_mask`,
    /// one row per 18 ms tick); in a window unless `--trace` is given.
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
    /// Wait for the display refresh.
    pub vsync: bool,
    /// Frame-rate cap when the display refresh does not pace drawing, and
    /// the switch for pacing headless benchmarks; `None` means no cap.
    pub fps_cap: Option<u32>,
    /// Interpolate between ticks.
    pub interpolation: bool,
    /// Image pack folder.
    pub pack: Option<PathBuf>,
    /// Claw's start point override.
    pub start: Option<(i32, i32)>,
    /// What the run does.
    pub mode: Mode,
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
            vsync: true,
            fps_cap: Some(DEFAULT_FPS_CAP),
            interpolation: true,
            pack: None,
            start: None,
            mode: Mode::Play,
        }
    }

    /// Resolves the command line: the config file first, then the
    /// arguments on top.
    pub fn from_args(args: &Args) -> Result<Settings> {
        let file = match &args.config {
            Some(path) => Some(ConfigFile::load(path)?),
            None => match default_config_path().filter(|p| p.is_file()) {
                Some(path) => Some(ConfigFile::load(&path)?),
                None => None,
            },
        };
        Settings::resolve(args, file.as_ref())
    }

    /// Resolves `args` over an already loaded config file.
    pub fn resolve(args: &Args, file: Option<&ConfigFile>) -> Result<Settings> {
        let mut s = Settings::new(PathBuf::new(), LevelChoice::default());
        let mut install = None;
        if let Some(file) = file {
            install = file.apply(&mut s)?;
        }
        if let Some(path) = &args.install {
            install = Some(path.clone());
        }
        s.install = install.ok_or_else(|| {
            anyhow!("no install given: pass --install PATH or set install in the config file")
        })?;
        match (args.level, &args.custom) {
            (Some(n), _) => s.level = LevelChoice::Retail(n),
            (None, Some(name)) => s.level = LevelChoice::Custom(name.clone()),
            (None, None) => {}
        }
        if let Some(res) = args.res {
            s.res = res;
        }
        if args.fullscreen {
            s.fullscreen = true;
        }
        if args.windowed {
            s.fullscreen = false;
        }
        if let Some(a) = args.aspect {
            s.aspect = a;
        }
        if let Some(scale) = args.scale {
            s.scale = scale;
        }
        if args.widen_activation {
            s.widen_activation = true;
        }
        if let Some(v) = args.vsync {
            s.vsync = v;
        }
        if let Some(n) = args.fps_cap {
            s.fps_cap = fps_cap(n).map_err(|e| anyhow!("--fps-cap: {e}"))?;
        }
        if args.uncapped {
            s.fps_cap = None;
        }
        if args.no_interp {
            s.interpolation = false;
        }
        if args.pack.is_some() {
            s.pack.clone_from(&args.pack);
        }
        s.start = args.start;
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
        Ok(s)
    }

    /// The view for an output of `size` pixels: the original 640x480 game
    /// view presented by the aspect and scaling settings.
    pub fn view(&self, size: (u32, u32)) -> ViewConfig {
        view_for(self.aspect, self.scale, self.widen_activation, size)
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
    pub fn sim_config(&self, size: (u32, u32)) -> SimConfig {
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

/// The default config file location.
pub fn default_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("rusty-claw").join("config.toml"))
}

/// A parsed config file: `key = value` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigFile {
    /// Values by key, unquoted.
    pub values: BTreeMap<String, String>,
    /// Where the file came from, for messages.
    pub origin: String,
}

/// Keys a config file may hold.
pub const CONFIG_KEYS: [&str; 12] = [
    "install",
    "level",
    "custom",
    "res",
    "fullscreen",
    "aspect",
    "scale",
    "widen_activation",
    "vsync",
    "fps_cap",
    "interpolation",
    "pack",
];

impl ConfigFile {
    /// Reads and parses `path`.
    pub fn load(path: &Path) -> Result<ConfigFile> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
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
            if !CONFIG_KEYS.contains(&key) {
                bail!(
                    "{origin}:{}: unknown setting {key:?} (known: {})",
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
                "pack" => s.pack = Some(PathBuf::from(v)),
                _ => {}
            }
        }
        // A custom level in the file wins over a level number in the file.
        if let Some(name) = self.values.get("custom") {
            s.level = LevelChoice::Custom(name.clone());
        }
        Ok(install)
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
