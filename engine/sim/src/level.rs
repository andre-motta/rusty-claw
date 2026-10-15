//! What the simulation needs from a level: the main plane's collision
//! tiles, the start point, the object records, the animations its logics
//! play (with the sounds of their frames) and the length of every
//! registered sound.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use claw_assets::wwd::OBJECT_INTS;
use claw_assets::{Ani, AssetStore, LevelData, Result, wav};

use crate::anim::Animation;
use crate::logic;
use crate::player::{self, AnimSet};
use crate::tiles::{TileMap, TileReport};
use crate::view::PlaneGeometry;

/// One object placed in the level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSpec {
    /// Logic name.
    pub logic: String,
    /// Image-set key.
    pub image_set: String,
    /// Animation key.
    pub anim: String,
    /// Frame number to show before the logic runs.
    pub frame: i32,
    /// The record's integers.
    pub record: [i32; OBJECT_INTS],
}

impl ObjectSpec {
    /// A record at (x, y, z) running `logic`, with every other field 0.
    pub fn at(logic: &str, x: i32, y: i32, z: i32) -> ObjectSpec {
        let mut record = [0; OBJECT_INTS];
        record[5] = x;
        record[6] = y;
        record[7] = z;
        ObjectSpec {
            logic: logic.to_string(),
            image_set: String::new(),
            anim: String::new(),
            frame: 0,
            record,
        }
    }
}

/// Image sets Claw's own logic switches him to at run time.
///
/// A logic that sets an object's `image_set` to anything other than the set
/// the object was placed with must list that set (here for Claw, in its
/// family's `IMAGE_SETS` for a level logic) and assign it with
/// [`runtime_image_set`], so that [`SimLevel::image_sets`] names it and its
/// graphics are loaded with the level instead of while drawing. Debug
/// builds check the assignment against the lists.
pub const RUNTIME_IMAGE_SETS: [&str; 1] = [player::IMAGE_SET];

/// Animation key prefix of Claw's animations.
pub const PLAYER_ANIM_PREFIX: &str = "CLAW_";

/// `name` as an object's image set, for a logic switching an object's set
/// at run time. The set must be in [`RUNTIME_IMAGE_SETS`] or a family's
/// `IMAGE_SETS`; debug builds check it.
pub fn runtime_image_set(name: &str) -> String {
    debug_assert!(
        RUNTIME_IMAGE_SETS.contains(&name) || logic::runtime_image_sets().contains(&name),
        "image set {name:?} is switched to at run time but not listed as a run-time image set"
    );
    name.to_string()
}

/// A level prepared for the simulation.
#[derive(Debug, Clone)]
pub struct SimLevel {
    /// Main-plane collision tiles.
    pub map: TileMap,
    /// Problems found in the tile data.
    pub tile_report: TileReport,
    /// Player start point.
    pub start: (i32, i32),
    /// Level objects in file order.
    pub objects: Vec<ObjectSpec>,
    /// Animations by registry key: Claw's, those named by the placed
    /// objects and those the families' `ANIM_PREFIXES` cover.
    pub animations: AnimSet,
    /// Every registered sound by key, with its length in ms when its file
    /// could be read.
    pub sounds: BTreeMap<String, Option<u32>>,
    /// Animations other than Claw's whose files could not be read; they
    /// are left out.
    pub skipped_animations: Vec<String>,
    /// Level flags word.
    pub flags: u32,
}

impl SimLevel {
    /// A level made of a tile map and a start point, with no objects or
    /// animations (tests and tools).
    pub fn synthetic(map: TileMap, start: (i32, i32)) -> SimLevel {
        SimLevel {
            map,
            tile_report: TileReport::default(),
            start,
            objects: Vec::new(),
            animations: HashMap::new(),
            sounds: BTreeMap::new(),
            skipped_animations: Vec::new(),
            flags: 0,
        }
    }

    /// Prepares a loaded level: reads the animations its logics play and
    /// the length of every registered sound from `store`. A sound that
    /// cannot be read keeps an unknown length, and an animation other than
    /// Claw's that cannot be read is left out; Claw's animations must load.
    pub fn from_level(level: &LevelData, store: &AssetStore) -> Result<SimLevel> {
        let (map, tile_report) = match level.main_plane.and_then(|i| level.planes.get(i)) {
            Some(plane) => TileMap::from_plane(plane, &level.tile_attributes),
            None => (TileMap::new(1, 1, 64, 64), TileReport::default()),
        };
        let objects: Vec<ObjectSpec> = level
            .objects
            .iter()
            .filter(|o| Some(o.plane) == level.main_plane)
            .map(|o| {
                let mut frame = o.ints[8];
                if frame == -1 {
                    frame = level
                        .registries
                        .images
                        .get(&o.image_set)
                        .and_then(|set| set.frames.keys().next().copied())
                        .unwrap_or(0);
                }
                ObjectSpec {
                    logic: o.logic.clone(),
                    image_set: o.image_set.clone(),
                    anim: o.anim.clone(),
                    frame,
                    record: o.ints,
                }
            })
            .collect();
        let sounds: BTreeMap<String, Option<u32>> = level
            .registries
            .sounds
            .iter()
            .map(|(key, &file)| {
                let ms = store
                    .read(file)
                    .ok()
                    .and_then(|data| wav::duration_ms(&data).ok());
                (key.clone(), ms)
            })
            .collect();
        let placed: BTreeSet<&str> = objects
            .iter()
            .map(|o| o.anim.as_str())
            .filter(|a| !a.is_empty())
            .collect();
        let prefixes = logic::runtime_anim_prefixes();
        let mut animations = HashMap::new();
        let mut skipped_animations = Vec::new();
        for (key, &file) in &level.registries.anis {
            let claw = key.starts_with(PLAYER_ANIM_PREFIX);
            let wanted = claw
                || placed.contains(key.as_str())
                || prefixes.iter().any(|p| key.starts_with(p));
            if !wanted {
                continue;
            }
            let ani = match store.read(file).and_then(|data| Ani::parse(&data)) {
                Ok(ani) => ani,
                Err(e) if claw => return Err(e),
                Err(_) => {
                    skipped_animations.push(key.clone());
                    continue;
                }
            };
            let mut anim = Animation::from_ani(key, &ani);
            for list in &mut anim.frame_sounds {
                for name in list.iter_mut() {
                    if name.as_ref().is_some_and(|n| !sounds.contains_key(n)) {
                        *name = None;
                    }
                }
            }
            animations.insert(key.clone(), Arc::new(anim));
        }
        Ok(SimLevel {
            map,
            tile_report,
            start: (level.start_x, level.start_y),
            objects,
            animations,
            sounds,
            skipped_animations,
            flags: level.header.flags,
        })
    }

    /// Length of the sound registered under `key`, ms; `None` when no
    /// sound is registered under it or its length is unknown.
    pub fn sound_ms(&self, key: &str) -> Option<u32> {
        self.sounds.get(key).copied().flatten()
    }

    /// Every image set the level's objects can show, sorted and without
    /// duplicates: the sets placed objects start with, the sets the
    /// simulation's own logics switch to at run time
    /// ([`RUNTIME_IMAGE_SETS`] and [`logic::runtime_image_sets`]) and the
    /// sets named in the loaded animations' headers. Meant for loading graphics at level load, so that drawing
    /// never has to read or decode a set.
    ///
    /// The animation header names are best-effort: no object takes its
    /// image set from an animation, so a header name may not be a set the
    /// renderer draws with. Listing one costs a lookup at load and nothing
    /// else. The sets that matter are the placed ones and
    /// the run-time lists.
    pub fn image_sets(&self) -> Vec<String> {
        let mut sets: BTreeSet<&str> = BTreeSet::new();
        sets.extend(RUNTIME_IMAGE_SETS);
        sets.extend(logic::runtime_image_sets());
        for o in &self.objects {
            sets.insert(&o.image_set);
        }
        for a in self.animations.values() {
            sets.insert(&a.image_set);
        }
        sets.remove("");
        sets.into_iter().map(str::to_string).collect()
    }

    /// Main-plane size and wrap flags.
    pub fn plane(&self) -> PlaneGeometry {
        PlaneGeometry {
            width: self.map.width(),
            height: self.map.height(),
            wrap_x: self.map.wrap_x(),
            wrap_y: self.map.wrap_y(),
        }
    }
}
