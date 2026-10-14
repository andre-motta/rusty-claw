//! The level model: everything a level needs from the asset store.
//!
//! Building a retail level `LEVEL<n>` reads its palette, maps its registries
//! in a fixed order and parses its world file:
//!
//! 1. `LEVEL<n>/TILES` as images with no prefix (so plane tile sets are keys
//!    such as `ACTION`);
//! 2. `SOUNDS` of `LEVEL<n>`, `CLAW` and `GAME` with prefixes `LEVEL`,
//!    `CLAW` and `GAME`;
//! 3. `IMAGES` and `ANIS` of `LEVEL<n>`, `CLAW` and `GAME`, each group with
//!    its prefix;
//! 4. the world `LEVEL<n>/WORLDS/WORLD.WWD`;
//! 5. music from `LEVEL<n>/MUSIC` and `GAME/MUSIC`.
//!
//! A community custom level is a `.WWD` file plus an optional folder of the
//! same name. It builds on the retail level named by the first number in its
//! header name, which must be 1 to 14. After the retail maps, the folder's
//! `TILES` (no prefix), `IMAGES`, `SOUNDS` and `ANIS` (prefix `CUSTOM`) and
//! `LEVEL` (prefix `LEVEL`) folders are mapped; its `PALETTES/MAIN.PAL`
//! replaces the level palette and its `MUSIC/*.XMI` tracks are added. The
//! world is read from the custom file itself. Scripts and plugins that may
//! ship with a custom level are plain data here and are never run.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::ops::RangeInclusive;
use std::path::Path;

use crate::error::{AssetError, Result, io_error};
use crate::name::{TYPE_PAL, TYPE_WWD, TYPE_XMI};
use crate::pal::Palette;
use crate::registry::Registries;
use crate::store::{AssetStore, DirNode, MAX_FILE_SIZE};
use crate::wwd::{ObjectRecord, Plane, TILE_EMPTY, TILE_FILLED, TileDescriptor, Wwd, WwdHeader};

/// Retail level numbers.
pub const RETAIL_LEVELS: RangeInclusive<u32> = 1..=14;

/// Which level to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelSource {
    /// Retail level `n`.
    Retail(u32),
    /// The custom level most recently mounted in the store.
    Custom,
}

/// A loaded level.
#[derive(Debug, Clone)]
pub struct LevelData {
    /// Retail level number (for a custom level, the level it builds on).
    pub level: u32,
    /// Name of the custom level, if this is one.
    pub custom_name: Option<String>,
    /// The world file header.
    pub header: WwdHeader,
    /// The level palette.
    pub palette: Palette,
    /// Planes in file order.
    pub planes: Vec<Plane>,
    /// Index of the main plane.
    pub main_plane: Option<usize>,
    /// Objects placed in the level.
    pub objects: Vec<ObjectRecord>,
    /// Tile collision attributes, indexed by tile number.
    pub tile_attributes: Vec<TileDescriptor>,
    /// Player start x in main-plane pixels.
    pub start_x: i32,
    /// Player start y in main-plane pixels.
    pub start_y: i32,
    /// Image, sound, animation and music registries.
    pub registries: Registries,
}

/// How well a level's names resolve against its registries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    /// Plane tile sets checked.
    pub tilesets: usize,
    /// (plane, tile set) pairs not in the image registry.
    pub missing_tilesets: Vec<(String, String)>,
    /// Distinct tile numbers checked.
    pub tiles: usize,
    /// (plane, tile set, tile number) with no frame.
    pub missing_tiles: Vec<(String, String, i32)>,
    /// Object image-set names checked.
    pub object_image_sets: usize,
    /// Object image-set names not in the image registry.
    pub missing_object_image_sets: Vec<String>,
    /// Object animation names checked.
    pub object_anims: usize,
    /// Object animation names that are neither an animation nor a sound.
    pub missing_object_anims: Vec<String>,
}

impl LevelData {
    /// Builds a level from the store.
    pub fn build(store: &AssetStore, source: LevelSource) -> Result<LevelData> {
        match source {
            LevelSource::Retail(level) => {
                if !RETAIL_LEVELS.contains(&level) {
                    return Err(AssetError::Invalid(format!(
                        "level {level} is not a retail level (1 to 14)"
                    )));
                }
                let mut registries = Registries::new();
                let palette = map_retail(store, level, &mut registries)?;
                let world = folder(store, &format!("LEVEL{level}/WORLDS"))?
                    .file("WORLD", TYPE_WWD)
                    .ok_or_else(|| {
                        AssetError::NotFound(format!("LEVEL{level}/WORLDS/WORLD.WWD"))
                    })?;
                let wwd = Wwd::parse(&store.read(world)?)?;
                map_music(store, level, None, &mut registries)?;
                Ok(Self::assemble(level, None, palette, wwd, registries))
            }
            LevelSource::Custom => {
                let custom = store
                    .custom_level()
                    .ok_or_else(|| AssetError::Invalid("no custom level is mounted".into()))?;
                let data = read_level_file(&custom.wwd_path)?;
                let wwd = Wwd::parse(&data)?;
                let level = wwd
                    .header
                    .base_level()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| RETAIL_LEVELS.contains(n))
                    .ok_or_else(|| {
                        AssetError::Invalid(format!(
                            "custom level {:?} does not name a retail level (1 to 14) in {:?}",
                            custom.name, wwd.header.name
                        ))
                    })?;
                let mut registries = Registries::new();
                let mut palette = map_retail(store, level, &mut registries)?;
                let tree = &custom.tree;
                if let Some(dir) = tree.subdir("TILES") {
                    registries.map_images(store, dir, "");
                }
                if let Some(dir) = tree.subdir("IMAGES") {
                    registries.map_images(store, dir, "CUSTOM");
                }
                if let Some(dir) = tree.subdir("SOUNDS") {
                    registries.map_sounds(store, dir, "CUSTOM");
                }
                if let Some(dir) = tree.subdir("ANIS") {
                    registries.map_anis(store, dir, "CUSTOM");
                }
                if let Some(dir) = tree.subdir("LEVEL") {
                    registries.map_images(store, dir, "LEVEL");
                    registries.map_sounds(store, dir, "LEVEL");
                    registries.map_anis(store, dir, "LEVEL");
                }
                if let Some(id) = tree
                    .subdir("PALETTES")
                    .and_then(|d| d.file("MAIN", TYPE_PAL))
                {
                    palette = Palette::parse(&store.read(id)?)?;
                }
                map_music(store, level, Some(tree), &mut registries)?;
                let name = custom.name.clone();
                Ok(Self::assemble(level, Some(name), palette, wwd, registries))
            }
        }
    }

    fn assemble(
        level: u32,
        custom_name: Option<String>,
        palette: Palette,
        wwd: Wwd,
        registries: Registries,
    ) -> LevelData {
        let main_plane = wwd.main_plane();
        LevelData {
            level,
            custom_name,
            start_x: wwd.header.start_x,
            start_y: wwd.header.start_y,
            header: wwd.header,
            palette,
            planes: wwd.planes,
            main_plane,
            objects: wwd.objects,
            tile_attributes: wwd.tile_descriptors,
            registries,
        }
    }

    /// Checks the level's tile sets, tiles and object names against its
    /// registries.
    pub fn check_references(&self) -> References {
        let reg = &self.registries;
        let mut out = References::default();
        for plane in &self.planes {
            for name in &plane.image_sets {
                out.tilesets += 1;
                if !reg.images.contains_key(name) {
                    out.missing_tilesets
                        .push((plane.name.clone(), name.clone()));
                }
            }
            let Some(set) = plane.image_sets.first().and_then(|n| reg.images.get(n)) else {
                continue;
            };
            let distinct: BTreeSet<i32> = plane
                .tiles
                .iter()
                .copied()
                .filter(|&t| t != TILE_EMPTY && t != TILE_FILLED)
                .collect();
            for tile in distinct {
                out.tiles += 1;
                if !set.frames.contains_key(&tile) {
                    out.missing_tiles
                        .push((plane.name.clone(), set.key.clone(), tile));
                }
            }
        }
        for obj in &self.objects {
            if !obj.image_set.is_empty() {
                out.object_image_sets += 1;
                if !reg.images.contains_key(&obj.image_set) {
                    out.missing_object_image_sets.push(obj.image_set.clone());
                }
            }
            if !obj.anim.is_empty() {
                out.object_anims += 1;
                if !reg.anis.contains_key(&obj.anim) && !reg.sounds.contains_key(&obj.anim) {
                    out.missing_object_anims.push(obj.anim.clone());
                }
            }
        }
        out
    }
}

fn folder<'a>(store: &'a AssetStore, path: &str) -> Result<&'a DirNode> {
    store
        .folder(path)?
        .ok_or_else(|| AssetError::NotFound(format!("folder {path}")))
}

fn sub<'a>(parent: &'a DirNode, name: &str) -> Result<&'a DirNode> {
    parent
        .subdir(name)
        .ok_or_else(|| AssetError::NotFound(format!("folder {}/{name}", parent.path())))
}

/// Reads the level palette and maps the ten retail folders in order.
fn map_retail(store: &AssetStore, level: u32, reg: &mut Registries) -> Result<Palette> {
    let level_dir = folder(store, &format!("LEVEL{level}"))?;
    let claw = folder(store, "CLAW")?;
    let game = folder(store, "GAME")?;
    let palette_id = sub(level_dir, "PALETTES")?
        .file("MAIN", TYPE_PAL)
        .ok_or_else(|| AssetError::NotFound(format!("LEVEL{level}/PALETTES/MAIN.PAL")))?;
    let palette = Palette::parse(&store.read(palette_id)?)?;
    reg.map_images(store, sub(level_dir, "TILES")?, "");
    reg.map_sounds(store, sub(level_dir, "SOUNDS")?, "LEVEL");
    reg.map_sounds(store, sub(claw, "SOUNDS")?, "CLAW");
    reg.map_sounds(store, sub(game, "SOUNDS")?, "GAME");
    for (dir, prefix) in [(level_dir, "LEVEL"), (claw, "CLAW"), (game, "GAME")] {
        reg.map_images(store, sub(dir, "IMAGES")?, prefix);
        reg.map_anis(store, sub(dir, "ANIS")?, prefix);
    }
    Ok(palette)
}

/// Maps the music tracks. The `0` variants (`PLAY0`, `BOSS0`, `POWERUP0`)
/// are preferred, as when no soundfont is loaded.
fn map_music(
    store: &AssetStore,
    level: u32,
    custom: Option<&DirNode>,
    reg: &mut Registries,
) -> Result<()> {
    let pick = |dir: &DirNode, names: &[&str]| names.iter().find_map(|n| dir.file(n, TYPE_XMI));
    if let Some(dir) = folder(store, &format!("LEVEL{level}"))?.subdir("MUSIC") {
        for (key, names) in [("LEVEL", ["PLAY0", "PLAY"]), ("BOSS", ["BOSS0", "BOSS"])] {
            if let Some(id) = pick(dir, &names) {
                reg.music.insert(key.to_string(), id);
            }
        }
    }
    if let Some(dir) = folder(store, "GAME")?.subdir("MUSIC") {
        if let Some(id) = pick(dir, &["POWERUP0", "POWERUP"]) {
            reg.music.insert("POWERUP".to_string(), id);
        }
        if let Some(id) = dir.file("MONOLITH", TYPE_XMI) {
            reg.music.insert("MONOLITH".to_string(), id);
        }
    }
    if let Some(dir) = custom.and_then(|t| t.subdir("MUSIC")) {
        for id in dir.files_of_type(TYPE_XMI) {
            reg.music.insert(store.file(id).name().to_string(), id);
        }
    }
    Ok(())
}

fn read_level_file(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| io_error(path, e))?;
    let mut data = Vec::new();
    file.take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut data)
        .map_err(|e| io_error(path, e))?;
    if data.len() as u64 > MAX_FILE_SIZE {
        return Err(AssetError::Invalid(format!(
            "{} is larger than {MAX_FILE_SIZE} bytes",
            path.display()
        )));
    }
    Ok(data)
}
