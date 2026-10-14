//! `claw-viewer`: shows a level from your own install with a free camera,
//! at any resolution and refresh rate.
//!
//! Keys: arrows move the camera (hold Shift to move faster), F fades to
//! black and back, I toggles interpolation, S switches between fit and
//! integer scaling, A cycles the aspect policy, F11 toggles fullscreen and
//! Escape quits.
//!
//! Without a window: `--headless-bench SECONDS` renders offscreen at `--res`
//! and prints frame-time statistics as JSON, `--screenshot FILE.png` saves
//! one frame, and `--dump-visible-tiles` prints the visible tile grid of
//! every plane for `--camera X,Y`.

mod bench;
mod stub;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Parser, ValueEnum};
use claw_assets::{Install, LevelData, LevelSource, Source};
use claw_frame::{AspectPolicy, ScaleMode, ViewConfig};
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
    /// the viewer draws the same picture as `--aspect native`).
    #[arg(long)]
    pub widen_activation: bool,
    /// Gameplay view in game pixels (default 640x480).
    #[arg(long, value_parser = parse_size)]
    pub logic: Option<(u32, u32)>,
    /// Wait for the display's refresh (on or off).
    #[arg(long, default_value = "on", value_parser = parse_on_off)]
    pub vsync: bool,
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
    match s {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(format!("expected on or off, got {s:?}")),
    }
}

impl Args {
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

/// A loaded level with its graphics.
#[derive(Debug)]
pub struct Loaded {
    /// The level.
    pub level: LevelData,
    /// Its graphics (tiles decoded, sprites loaded on demand).
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
    let graphics = LevelGraphics::from_level(&level, &store, pack);
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
