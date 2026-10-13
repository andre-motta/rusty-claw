//! `claw-info`: prints the planes, tile sets, object count and start position
//! of a retail level or a community custom level from your own install.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{ArgGroup, Parser};
use claw_assets::{Install, LevelData, LevelSource, Source};

/// Print a summary of one level.
#[derive(Debug, Parser)]
#[command(name = "claw-info", version)]
#[command(group(ArgGroup::new("which").required(true).args(["level", "custom"])))]
struct Args {
    /// Root folder of your own game install.
    #[arg(long)]
    install: PathBuf,
    /// Retail level number (1 to 14).
    #[arg(long)]
    level: Option<u32>,
    /// Community custom level file in the install's Custom folder, for
    /// example "My Level.WWD".
    #[arg(long)]
    custom: Option<String>,
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("claw-info: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let install = Install::open(&args.install)
        .with_context(|| format!("opening install {}", args.install.display()))?;
    let mut store = install.store().context("mounting assets")?;
    let source = match (args.level, &args.custom) {
        (Some(level), _) => LevelSource::Retail(level),
        (None, Some(name)) => {
            let path = install.custom_level_path(name)?;
            store.mount(Source::CustomLevel(path))?;
            LevelSource::Custom
        }
        (None, None) => anyhow::bail!("pass --level N or --custom FILE"),
    };
    let level = LevelData::build(&store, source).context("loading level")?;
    let elapsed = started.elapsed();
    let mut text = describe(&level)?;
    writeln!(text, "Loaded in {:.0} ms", elapsed.as_secs_f64() * 1000.0)?;
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => Err(e.into()),
        _ => Ok(()),
    }
}

fn describe(level: &LevelData) -> Result<String> {
    let mut out = String::new();
    match &level.custom_name {
        Some(name) => writeln!(out, "Custom level {name:?}, built on level {}", level.level)?,
        None => writeln!(out, "Level {}", level.level)?,
    }
    let h = &level.header;
    writeln!(out, "Name:    {}", h.name)?;
    if !h.author.is_empty() {
        writeln!(out, "Author:  {}", h.author)?;
    }
    writeln!(out, "Start:   {}, {}", level.start_x, level.start_y)?;

    writeln!(out)?;
    writeln!(out, "Planes:  {}", level.planes.len())?;
    writeln!(
        out,
        "  {:<2} {:<16} {:>6} {:>11} {:>9} {:>11} {:>6}  tile set",
        "#", "name", "flags", "tiles", "tile px", "scroll %", "z"
    )?;
    for (i, p) in level.planes.iter().enumerate() {
        let main = if Some(i) == level.main_plane {
            "*"
        } else {
            " "
        };
        writeln!(
            out,
            "{main} {:<2} {:<16} {:>#6x} {:>11} {:>9} {:>11} {:>6}  {}",
            i,
            p.name,
            p.flags,
            format!("{}x{}", p.w_tiles, p.h_tiles),
            format!("{}x{}", p.tile_w, p.tile_h),
            format!("{}/{}", p.move_x_pct, p.move_y_pct),
            p.z,
            p.image_sets.join(", ")
        )?;
    }

    writeln!(out)?;
    let refs = level.check_references();
    let sets: BTreeSet<&String> = level.planes.iter().flat_map(|p| &p.image_sets).collect();
    writeln!(out, "Tile sets: {}", sets.len())?;
    for name in sets {
        match level.registries.images.get(name) {
            Some(set) => {
                let missing = refs
                    .missing_tiles
                    .iter()
                    .filter(|(_, key, _)| key == name)
                    .count();
                writeln!(
                    out,
                    "  {name:<16} {:>5} frames, {missing} used tile numbers without a frame",
                    set.frames.len()
                )?;
            }
            None => writeln!(out, "  {name:<16} not found")?,
        }
    }

    writeln!(out)?;
    let main_name = level
        .main_plane
        .and_then(|i| level.planes.get(i))
        .map_or("none", |p| p.name.as_str());
    writeln!(
        out,
        "Objects: {} (main plane: {main_name})",
        level.objects.len()
    )?;
    writeln!(out, "Tile attributes: {}", level.tile_attributes.len())?;
    let reg = &level.registries;
    writeln!(
        out,
        "Registries: {} image sets, {} sounds, {} animations, {} music tracks",
        reg.images.len(),
        reg.sounds.len(),
        reg.anis.len(),
        reg.music.len()
    )?;
    writeln!(
        out,
        "Object names without a match: {} of {} image sets, {} of {} animations",
        refs.missing_object_image_sets.len(),
        refs.object_image_sets,
        refs.missing_object_anims.len(),
        refs.object_anims
    )?;
    Ok(out)
}
