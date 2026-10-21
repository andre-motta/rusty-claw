//! Loading a level from the player's install: assets, simulation and
//! graphics.
//!
//! Levels are read in place from the install. A community custom level is
//! the level file in the install's `Custom` folder plus its optional folder
//! of the same name; only its data files are read, and any script or plugin
//! that ships with it is ignored.
//!
//! Besides the level's own images, the graphics hold one image of the
//! game's own, the shade ([`SHADE_SET`]): a square of the palette's darkest
//! colour that, drawn translucent over the picture, gives the dimmed copy
//! of the level that the in-game menu shows behind its entries.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use claw_assets::{AssetStore, Frame, Install, LevelData, LevelSource, Palette, Source};
use claw_frame::SimSnapshot;
use claw_render::effects::LevelTables;
use claw_render::images::{LEVEL_FLAG_INTERLEAVED, SourceFrame};
use claw_render::{HdPack, ImageSource, LevelGraphics, StoreImages};
use claw_sim::{Sim, SimConfig, SimLevel, StartState, TickInput};

use crate::settings::{LevelChoice, Settings};

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

/// The image set of the shade: one opaque square of [`SHADE_SIZE`] game
/// pixels in the palette's darkest colour (see the module documentation).
pub const SHADE_SET: &str = "RUSTYCLAW_SHADE";

/// Side of the shade's square, in game pixels.
pub const SHADE_SIZE: u32 = 128;

/// The darkest colour of `palette`, leaving out index 0, which sprites
/// leave transparent.
pub fn darkest_index(palette: &Palette) -> u8 {
    let light = |c: &[u8; 3]| u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114;
    (1..=255u8)
        .min_by_key(|&i| light(&palette.0[usize::from(i)]))
        .unwrap_or(1)
}

/// The shade's frame for `palette`.
pub fn shade_frame(palette: &Palette) -> Frame {
    Frame {
        width: SHADE_SIZE,
        height: SHADE_SIZE,
        offset_x: 0,
        offset_y: 0,
        flags: 0,
        indices: vec![darkest_index(palette); (SHADE_SIZE * SHADE_SIZE) as usize],
        opaque: None,
    }
}

/// A level's images and the shade.
struct LevelImages {
    store: StoreImages,
    shade: Frame,
}

impl ImageSource for LevelImages {
    fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>> {
        if key == SHADE_SET {
            return Some(vec![SourceFrame {
                number: 1,
                frame: self.shade.clone(),
                path: None,
            }]);
        }
        self.store.image_set(key)
    }
}

/// The level's graphics: planes resolved and tile sets decoded, with an
/// optional image pack, and the shade.
pub fn load_graphics(bundle: &LevelBundle, pack: Option<&Path>) -> Result<LevelGraphics> {
    let pack = match pack {
        Some(dir) => {
            Some(HdPack::load(dir).with_context(|| format!("image pack {}", dir.display()))?)
        }
        None => None,
    };
    let level = &bundle.level;
    let images = LevelImages {
        store: StoreImages::new(bundle.store.clone(), level.registries.clone()),
        shade: shade_frame(&level.palette),
    };
    Ok(LevelGraphics::new(
        level.planes.clone(),
        level.header.flags & LEVEL_FLAG_INTERLEAVED != 0,
        level.palette.clone(),
        LevelTables::for_level(&bundle.store, level.level, &level.palette),
        Box::new(images),
        pack,
    ))
}

/// Decodes every sprite set the level's objects, the simulation's
/// run-time sets and the first snapshot name, so that no file is read or
/// decoded while frames are drawn. Returns the number of sets that loaded.
pub fn preload(graphics: &mut LevelGraphics, level: &LevelData, first: &SimSnapshot) -> usize {
    let runtime = claw_sim::logic::runtime_image_sets();
    preload_keys(
        level.objects.iter().map(|o| o.image_set.as_str()),
        &runtime,
        first,
    )
    .iter()
    .filter(|k| graphics.sprite(k, -1).is_some())
    .count()
}

/// The sprite sets [`preload`] loads, sorted and without duplicates: the
/// sets `placed` objects start with, Claw's own, the sets the simulation's
/// logics switch objects to while the level runs (`runtime`, for example
/// the mark a sound object shows) and the sets `first` names.
pub fn preload_keys<'a>(
    placed: impl IntoIterator<Item = &'a str>,
    runtime: &[&'a str],
    first: &'a SimSnapshot,
) -> Vec<&'a str> {
    let mut keys: Vec<&str> = placed
        .into_iter()
        .chain(claw_sim::level::RUNTIME_IMAGE_SETS)
        .chain(runtime.iter().copied())
        .chain(first.renderables.iter().map(|r| r.image_set_key.as_str()))
        .filter(|k| !k.is_empty())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
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

/// The start point override for `level` about to start from `start`:
/// `settings.start` (the `--start` option) applies only to the first level
/// started (`first`), when it is the level the settings name and it does
/// not start at a save point.
pub fn start_override(
    settings: &Settings,
    first: bool,
    level: &LevelChoice,
    start: &StartState,
) -> Option<(i32, i32)> {
    settings
        .start
        .filter(|_| first && *level == settings.level && start.save.is_none())
}

/// Moves Claw to (x, y) and runs idle ticks until he and the camera have
/// stood still for ten ticks, at most [`SETTLE_LIMIT`].
pub fn settle_at(sim: &mut Sim, x: i32, y: i32) {
    settle_at_with(sim, x, y, |_| {});
}

/// [`settle_at`], calling `after_tick` after each idle tick.
pub fn settle_at_with(sim: &mut Sim, x: i32, y: i32, mut after_tick: impl FnMut(&mut Sim)) {
    sim.teleport_player(x, y);
    let mut still = 0;
    let mut last = None;
    for _ in 0..SETTLE_LIMIT {
        sim.step(TickInput::default());
        after_tick(sim);
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

#[cfg(test)]
mod tests {
    use super::*;

    use claw_frame::{DrawMode, Renderable};
    use claw_sim::SavePos;

    #[test]
    fn the_start_point_applies_to_the_first_level_named_only() {
        let mut s = Settings::new("/i", LevelChoice::Retail(2));
        s.start = Some((100, 200));
        let fresh = StartState::default();
        let at_save = StartState {
            save: Some(SavePos {
                slot: 1,
                x: 5,
                y: 6,
            }),
            ..StartState::default()
        };
        let two = LevelChoice::Retail(2);
        assert_eq!(start_override(&s, true, &two, &fresh), Some((100, 200)));
        assert_eq!(start_override(&s, false, &two, &fresh), None);
        assert_eq!(
            start_override(&s, true, &LevelChoice::Retail(1), &fresh),
            None
        );
        assert_eq!(start_override(&s, true, &two, &at_save), None);
        s.start = None;
        assert_eq!(start_override(&s, true, &two, &fresh), None);
    }

    #[test]
    fn preloading_covers_the_sets_objects_switch_to_while_running() {
        let sprite = |key: &str| Renderable {
            id: 1,
            image_set_key: key.to_string(),
            frame: 1,
            x: 0.0,
            y: 0.0,
            z: 0,
            mirror: false,
            invert: false,
            flash: false,
            draw_mode: DrawMode::default(),
            draw_color: None,
            teleported: false,
        };
        let first = SimSnapshot {
            renderables: vec![sprite("LEVEL_FIRST"), sprite("")],
            ..SimSnapshot::default()
        };
        let runtime = claw_sim::logic::runtime_image_sets();
        let keys = preload_keys(["LEVEL_PLACED", "", "LEVEL_PLACED"], &runtime, &first);
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(keys, sorted, "sorted and without duplicates");
        assert!(!keys.contains(&""));
        for key in ["LEVEL_PLACED", "LEVEL_FIRST"]
            .into_iter()
            .chain(claw_sim::level::RUNTIME_IMAGE_SETS)
            .chain(runtime.iter().copied())
        {
            assert!(keys.contains(&key), "{key} is not preloaded");
        }
        // The mark a sound object shows only appears once the level runs.
        assert!(keys.contains(&claw_sim::logic::sound::EXCLAMATION_IMAGE_SET));
    }

    #[test]
    fn the_shade_is_the_darkest_colour_but_index_0() {
        let mut p = Palette::default();
        for (i, c) in p.0.iter_mut().enumerate() {
            *c = [200, 200, (i as u8).wrapping_add(10)];
        }
        p.0[0] = [0, 0, 0];
        p.0[77] = [3, 2, 1];
        assert_eq!(darkest_index(&p), 77);
        let f = shade_frame(&p);
        assert_eq!((f.width, f.height), (SHADE_SIZE, SHADE_SIZE));
        assert!(f.indices.iter().all(|&i| i == 77));
        assert_eq!(f.indices.len(), (SHADE_SIZE * SHADE_SIZE) as usize);
    }
}
