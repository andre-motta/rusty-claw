//! Loading a level from the player's install: assets, simulation and
//! graphics.
//!
//! Levels are read in place from the install. A community custom level is
//! the level file in the install's `Custom` folder plus its optional folder
//! of the same name; only its data files are read, and any script or plugin
//! that ships with it is ignored.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use claw_assets::{AssetStore, Install, LevelData, LevelSource, Source};
use claw_frame::SimSnapshot;
use claw_render::{HdPack, LevelGraphics};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};

use crate::settings::LevelChoice;

/// A level's data, ready for a simulation.
#[derive(Debug, Clone)]
pub struct LevelBundle {
    /// The asset store with the level's sources mounted.
    pub store: AssetStore,
    /// The level.
    pub level: LevelData,
    /// The level prepared for the simulation.
    pub sim_level: Arc<SimLevel>,
    /// A name for messages and the window title.
    pub name: String,
    /// Time spent reading and preparing the level.
    pub load_time: Duration,
}

/// Opens the install at `install` and loads `choice`.
pub fn load_level(install: &Path, choice: &LevelChoice) -> Result<LevelBundle> {
    let t = Instant::now();
    let inst = Install::open(install)
        .with_context(|| format!("opening the install at {}", install.display()))?;
    let mut store = inst.store().context("reading the install's assets")?;
    let (source, name) = match choice {
        LevelChoice::Retail(n) => (LevelSource::Retail(*n), format!("level {n}")),
        LevelChoice::Custom(file) => {
            let path = inst
                .custom_level_path(file)
                .with_context(|| format!("finding custom level {file:?}"))?;
            store
                .mount(Source::CustomLevel(path))
                .with_context(|| format!("reading custom level {file:?}"))?;
            (LevelSource::Custom, format!("custom level {file}"))
        }
    };
    let level = LevelData::build(&store, source).with_context(|| format!("loading {name}"))?;
    let sim_level =
        SimLevel::from_level(&level, &store).with_context(|| format!("preparing {name}"))?;
    Ok(LevelBundle {
        store,
        level,
        sim_level: Arc::new(sim_level),
        name,
        load_time: t.elapsed(),
    })
}

/// The level's graphics: planes resolved and tile sets decoded, with an
/// optional image pack.
pub fn load_graphics(bundle: &LevelBundle, pack: Option<&Path>) -> Result<LevelGraphics> {
    let pack = match pack {
        Some(dir) => {
            Some(HdPack::load(dir).with_context(|| format!("image pack {}", dir.display()))?)
        }
        None => None,
    };
    Ok(LevelGraphics::from_level(
        &bundle.level,
        &bundle.store,
        pack,
    ))
}

/// Decodes every sprite set the level's objects and the first snapshot
/// name, so that no file is read or decoded while frames are drawn.
/// Returns the number of sets that loaded.
pub fn preload(graphics: &mut LevelGraphics, level: &LevelData, first: &SimSnapshot) -> usize {
    let mut keys: Vec<&str> = level
        .objects
        .iter()
        .map(|o| o.image_set.as_str())
        .chain(first.renderables.iter().map(|r| r.image_set_key.as_str()))
        .filter(|k| !k.is_empty())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.iter()
        .filter(|k| graphics.sprite(k, -1).is_some())
        .count()
}

/// Ticks to wait at most for Claw and the camera to settle.
pub const SETTLE_LIMIT: usize = 222;

/// Starts a simulation of `bundle`. With `start`, Claw is moved there and
/// the simulation runs idle until he and the camera have stood still for
/// ten ticks (at most four seconds).
pub fn new_sim(bundle: &LevelBundle, cfg: SimConfig, start: Option<(i32, i32)>) -> Sim {
    let mut sim = Sim::new(bundle.sim_level.clone(), cfg);
    if let Some((x, y)) = start {
        settle_at(&mut sim, x, y);
    }
    sim
}

/// Moves Claw to (x, y) and runs idle ticks until he and the camera have
/// stood still for ten ticks, at most [`SETTLE_LIMIT`].
pub fn settle_at(sim: &mut Sim, x: i32, y: i32) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..SETTLE_LIMIT {
        sim.step(TickInput::default());
        let now = sim
            .player()
            .map(|o| (o.x, o.y, o.state, sim.camera().current));
        still = if now == last { still + 1 } else { 0 };
        last = now;
        if still >= 10 {
            break;
        }
    }
}
