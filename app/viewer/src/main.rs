//! `claw-viewer`: shows a level from your own install with a free camera,
//! at any resolution and refresh rate.
//!
//! Keys: arrows move the camera (hold Shift to move faster), F fades to
//! black and back, I toggles interpolation, S switches between fit and
//! integer scaling, A cycles the aspect policy, F11 toggles fullscreen and
//! Escape quits.
//!
//! With vsync on (the default) the display's refresh paces the frames; with
//! `--vsync off` drawing is capped at `--fps-cap` frames per second (300 by
//! default). `--fps-cap 0` or `--uncapped` removes the cap, for measuring
//! throughput only.
//!
//! Without a window: `--headless-bench SECONDS` renders offscreen at `--res`
//! and prints frame-time statistics as JSON, `--screenshot FILE.png` saves
//! one frame, and `--dump-visible-tiles` prints the visible tile grid of
//! every plane for `--camera X,Y`. The headless benchmark draws at its
//! simulated display rate (`--bench-hz`) unless `--uncapped` is given.

mod bench;
mod stub;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{ArgAction, ArgGroup, Parser, ValueEnum};
use claw_assets::{Install, LevelData, LevelSource, Source};
use claw_frame::{AspectPolicy, DEFAULT_FPS_CAP, ScaleMode, ViewConfig};
use claw_render::{HdPack, LevelGraphics};

/// How the picture fills an output of another shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Aspect {
    /// Fill the output; the view stays inside the level.
    Native,
    /// Fill the output around the original view, showing beyond the level
    /// near its edges.
    Centered,
    /// The original 4:3 view with bars at the sides.
    #[value(name = "4:3")]
    Classic,
}

/// Scaling of the picture to the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scale {
    /// Fill the output height (whole-pixel steps, then a smooth remainder).
    Fit,
    /// Whole-pixel steps only, with bars.
    Integer,
}

/// Show a level.
#[derive(Debug, Clone, Parser)]
#[command(name = "claw-viewer", version)]
#[command(group(ArgGroup::new("which").required(true).args(["level", "custom"])))]
pub struct Args {
    /// Root folder of your own game install.
    #[arg(long)]
    pub install: PathBuf,
    /// Retail level number (1 to 14).
    #[arg(long)]
    pub level: Option<u32>,
    /// Community custom level file in the install's Custom folder.
    #[arg(long)]
    pub custom: Option<String>,
    /// Output resolution, for example 3840x2160; the window size, or the
    /// offscreen size without a window. Default 1280x720.
    #[arg(long, value_parser = parse_size)]
    pub res: Option<(u32, u32)>,
    /// Borderless fullscreen on the current monitor.
    #[arg(long)]
    pub fullscreen: bool,
    /// Scaling mode.
    #[arg(long, value_enum, default_value = "fit")]
    pub scale: Scale,
    /// Aspect policy.
    #[arg(long, value_enum, default_value = "native")]
    pub aspect: Aspect,
    /// Keep every visible object active (a gameplay change for the game;
    /// the viewer draws the same picture as `--aspect native`). Only with
    /// `--aspect native`.
    #[arg(long)]
    pub widen_activation: bool,
    /// Gameplay view in game pixels (default 640x480).
    #[arg(long, value_parser = parse_size)]
    pub logic: Option<(u32, u32)>,
    /// Wait for the display's refresh: on or off.
    #[arg(
        long,
        value_name = "on|off",
        default_value = "on",
        value_parser = parse_on_off,
        action = ArgAction::Set
    )]
    pub vsync: bool,
    /// With vsync off: the most frames drawn per second; 0 means no cap,
    /// for measuring throughput only.
    #[arg(long, value_name = "N", default_value_t = DEFAULT_FPS_CAP, conflicts_with = "uncapped")]
    pub fps_cap: u32,
    /// No frame-rate cap with vsync off, and no pacing of the headless
    /// benchmark: for measuring throughput only.
    #[arg(long)]
    pub uncapped: bool,
    /// Draw without interpolation (the latest tick only).
    #[arg(long)]
    pub no_interp: bool,
    /// Image pack folder with higher-resolution replacements.
    #[arg(long)]
    pub pack: Option<PathBuf>,
    /// Render offscreen for this many seconds and print statistics.
    #[arg(long, value_name = "SECONDS")]
    pub headless_bench: Option<f64>,
    /// Display rate simulated by the headless benchmark, in Hz.
    #[arg(long, default_value_t = 144.0)]
    pub bench_hz: f64,
    /// Print the visible tile grid of every plane and exit.
    #[arg(long)]
    pub dump_visible_tiles: bool,
    /// Camera point (view centre) in level pixels; default the start point.
    #[arg(long, value_parser = parse_point)]
    pub camera: Option<(i32, i32)>,
    /// Close the window after this many seconds (with the camera panning
    /// across the level), then print frame statistics.
    #[arg(long, value_name = "SECONDS")]
    pub quit_after: Option<f64>,
    /// Render one frame offscreen and save it as a PNG.
    #[arg(long, value_name = "FILE")]
    pub screenshot: Option<PathBuf>,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .to_ascii_lowercase()
        .split_once('x')
        .map(|(a, b)| (a.trim().parse::<u32>(), b.trim().parse::<u32>()))
        .ok_or_else(|| format!("expected WIDTHxHEIGHT, got {s:?}"))?;
    match (w, h) {
        (Ok(w), Ok(h)) if (1..=32768).contains(&w) && (1..=32768).contains(&h) => Ok((w, h)),
        _ => Err(format!("bad size {s:?}")),
    }
}

fn parse_point(s: &str) -> Result<(i32, i32), String> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("expected X,Y, got {s:?}"))?;
    match (x.trim().parse(), y.trim().parse()) {
        (Ok(x), Ok(y)) => Ok((x, y)),
        _ => Err(format!("bad point {s:?}")),
    }
}

fn parse_on_off(s: &str) -> Result<bool, String> {
    match s.trim() {
        "on" | "true" => Ok(true),
        "off" | "false" => Ok(false),
        _ => Err(format!("expected on or off, got {s:?}")),
    }
}

/// Largest frame-rate cap accepted.
pub const MAX_FPS_CAP: u32 = 10_000;

impl Args {
    /// The frame-rate cap, or `None` for no cap.
    pub fn frame_cap(&self) -> Option<u32> {
        (!self.uncapped && self.fps_cap > 0).then_some(self.fps_cap)
    }

    /// Rejects option combinations that cannot be honoured.
    pub fn validate(&self) -> Result<()> {
        if self.fps_cap > MAX_FPS_CAP {
            bail!("--fps-cap takes a frame rate up to {MAX_FPS_CAP}, or 0 for no cap");
        }
        if !(self.bench_hz.is_finite() && (1.0..=10_000.0).contains(&self.bench_hz)) {
            bail!("--bench-hz needs a rate between 1 and 10000 Hz");
        }
        if self.widen_activation && self.aspect != Aspect::Native {
            bail!(
                "--widen-activation needs --aspect native: widened activation fills the \
                 output width, which the {} aspect does not",
                match self.aspect {
                    Aspect::Native => "native",
                    Aspect::Centered => "centered",
                    Aspect::Classic => "4:3",
                }
            );
        }
        Ok(())
    }

    /// The view configuration for an output of `size`.
    pub fn view(&self, size: (u32, u32)) -> ViewConfig {
        let (lw, lh) = self.logic.unwrap_or((640, 480));
        ViewConfig {
            logic_width: lw,
            logic_height: lh,
            render_width: size.0.max(1),
            render_height: size.1.max(1),
            aspect_policy: match self.aspect {
                Aspect::Native if self.widen_activation => AspectPolicy::WidenedActivation,
                Aspect::Native => AspectPolicy::FrozenEdgeClamped,
                Aspect::Centered => AspectPolicy::FrozenEdgeCentered,
                Aspect::Classic => AspectPolicy::Pillarbox,
            },
            scale_mode: match self.scale {
                Scale::Fit => ScaleMode::Fit,
                Scale::Integer => ScaleMode::Integer,
            },
        }
    }

    /// The requested output size.
    pub fn size(&self) -> (u32, u32) {
        self.res.unwrap_or((1280, 720))
    }
}

/// The aspect policy after `current` in the A key cycle. With widened
/// activation requested, the native step is the widened policy, so the
/// cycle comes back to the mode the user asked for.
pub fn next_aspect(current: AspectPolicy, widened: bool) -> AspectPolicy {
    let native = if widened {
        AspectPolicy::WidenedActivation
    } else {
        AspectPolicy::FrozenEdgeClamped
    };
    match current {
        AspectPolicy::FrozenEdgeClamped | AspectPolicy::WidenedActivation => {
            AspectPolicy::FrozenEdgeCentered
        }
        AspectPolicy::FrozenEdgeCentered => AspectPolicy::Pillarbox,
        AspectPolicy::Pillarbox => native,
    }
}

/// A loaded level with its graphics.
#[derive(Debug)]
pub struct Loaded {
    /// The level.
    pub level: LevelData,
    /// Its graphics (tiles and every placed object's sprites decoded).
    pub graphics: LevelGraphics,
    /// Load time.
    pub load: Duration,
}

/// Loads the level the arguments name.
pub fn load(args: &Args) -> Result<Loaded> {
    let t = Instant::now();
    let install = Install::open(&args.install)
        .with_context(|| format!("opening install {}", args.install.display()))?;
    let mut store = install.store().context("mounting assets")?;
    let source = match (args.level, &args.custom) {
        (Some(n), _) => LevelSource::Retail(n),
        (None, Some(name)) => {
            let path = install.custom_level_path(name)?;
            store.mount(Source::CustomLevel(path))?;
            LevelSource::Custom
        }
        (None, None) => bail!("pass --level N or --custom FILE"),
    };
    let level = LevelData::build(&store, source).context("loading level")?;
    let pack = match &args.pack {
        Some(dir) => {
            Some(HdPack::load(dir).with_context(|| format!("image pack {}", dir.display()))?)
        }
        None => None,
    };
    let mut graphics = LevelGraphics::from_level(&level, &store, pack);
    // Decode every sprite set the placed objects use now, so no frame reads
    // or decodes images.
    graphics.preload(level.objects.iter().map(|o| o.image_set.as_str()));
    for w in graphics.pack_warnings() {
        eprintln!("claw-viewer: pack: {w}");
    }
    Ok(Loaded {
        level,
        graphics,
        load: t.elapsed(),
    })
}

fn dump_tiles(args: &Args) -> Result<()> {
    let loaded = load(args)?;
    let view = args.view(args.size());
    let mut builder = claw_render::SceneBuilder::new(&loaded.graphics, view, 1 << 15);
    let (x, y) = args
        .camera
        .unwrap_or((loaded.level.start_x, loaded.level.start_y));
    let (pw, ph) = builder.composition().presentation;
    let cells = builder.visible_cells(&loaded.graphics, x, y);
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"level\": {}, \"view\": {{\"w\": {pw}, \"h\": {ph}}}, \"cameras\": [\n",
        loaded.level.level
    ));
    out.push_str(&format!(" {{\"x\": {x}, \"y\": {y}, \"planes\": [\n"));
    let n = cells.len();
    for (i, ((rect, grid), plane)) in cells.iter().zip(&loaded.graphics.planes).enumerate() {
        let name: String = plane
            .name
            .chars()
            .flat_map(|c| match c {
                '"' => vec!['\\', '"'],
                '\\' => vec!['\\', '\\'],
                c if c.is_control() => vec![' '],
                c => vec![c],
            })
            .collect();
        out.push_str(&format!(
            "  {{\"name\": \"{name}\", \"z\": {}, \"view_left\": {}, \"view_top\": {}, \"cols\": {}, \"rows\": {}, \"grid\": [\n",
            plane.z,
            rect.left,
            rect.top,
            grid.first().map_or(0, Vec::len),
            grid.len()
        ));
        for (r, row) in grid.iter().enumerate() {
            let cells: Vec<String> = row.iter().map(i32::to_string).collect();
            out.push_str(&format!(
                "   [{}]{}\n",
                cells.join(", "),
                if r + 1 < grid.len() { "," } else { "" }
            ));
        }
        out.push_str(&format!("  ]}}{}\n", if i + 1 < n { "," } else { "" }));
    }
    out.push_str(" ]}\n]}\n");
    print!("{out}");
    Ok(())
}

fn run(args: Args) -> Result<()> {
    args.validate()?;
    if args.dump_visible_tiles {
        return dump_tiles(&args);
    }
    if let Some(path) = &args.screenshot {
        return bench::screenshot(&args, path);
    }
    if let Some(seconds) = args.headless_bench {
        if !(seconds.is_finite() && seconds > 0.0) {
            bail!("--headless-bench needs a positive number of seconds");
        }
        return bench::run(&args, seconds);
    }
    window::run(args)
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("claw-viewer: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Args {
        let mut all = vec!["claw-viewer", "--install", "/nowhere", "--level", "1"];
        all.extend_from_slice(extra);
        Args::try_parse_from(all).unwrap()
    }

    #[test]
    fn widened_activation_needs_the_native_aspect() {
        assert!(args(&["--widen-activation"]).validate().is_ok());
        let a = args(&["--widen-activation"]);
        assert_eq!(
            a.view((1920, 1080)).aspect_policy,
            AspectPolicy::WidenedActivation
        );
        for aspect in ["centered", "4:3"] {
            let a = args(&["--widen-activation", "--aspect", aspect]);
            let err = a.validate().unwrap_err().to_string();
            assert!(err.contains("--aspect native"), "{err}");
            assert!(args(&["--aspect", aspect]).validate().is_ok());
        }
    }

    #[test]
    fn vsync_and_the_frame_rate_cap() {
        let a = args(&[]);
        assert!(a.vsync);
        assert_eq!(a.frame_cap(), Some(DEFAULT_FPS_CAP));
        assert!(args(&["--vsync", "off"]).validate().is_ok());
        assert!(!args(&["--vsync", "off"]).vsync);
        assert_eq!(args(&["--fps-cap", "144"]).frame_cap(), Some(144));
        assert_eq!(args(&["--fps-cap", "0"]).frame_cap(), None);
        assert_eq!(args(&["--uncapped"]).frame_cap(), None);
        assert!(args(&["--fps-cap", "20000"]).validate().is_err());
        assert!(args(&["--bench-hz", "0"]).validate().is_err());
        let both = [
            "claw-viewer",
            "--install",
            "/i",
            "--level",
            "1",
            "--fps-cap",
            "60",
        ];
        assert!(Args::try_parse_from(both.iter().chain(&["--uncapped"])).is_err());
    }

    #[test]
    fn aspect_cycle_returns_to_the_requested_mode() {
        for widened in [false, true] {
            let start = args(if widened {
                &["--widen-activation"]
            } else {
                &[]
            })
            .view((1920, 1080))
            .aspect_policy;
            let mut seen = vec![start];
            let mut p = start;
            for _ in 0..3 {
                p = next_aspect(p, widened);
                seen.push(p);
            }
            assert_eq!(p, start, "widened {widened}: {seen:?}");
            assert_eq!(seen[1], AspectPolicy::FrozenEdgeCentered);
            assert_eq!(seen[2], AspectPolicy::Pillarbox);
        }
    }
}
