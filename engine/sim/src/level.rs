//! What the simulation needs from a level: the main plane's collision
//! tiles, the start point, the object records and Claw's animations.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use claw_assets::wwd::OBJECT_INTS;
use claw_assets::{Ani, AssetStore, LevelData, Result};

use crate::anim::Animation;
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

/// Image sets the simulation's own logics switch objects to at run time.
///
/// A logic that sets an object's `image_set` to anything other than the set
/// the object was placed with must list that set here and assign it with
/// [`runtime_image_set`], so that [`SimLevel::image_sets`] names it and its
/// graphics are loaded with the level instead of while drawing. Debug
/// builds check the assignment against this list.
pub const RUNTIME_IMAGE_SETS: [&str; 1] = [player::IMAGE_SET];

/// `name` as an object's image set, for a logic switching an object's set
/// at run time. The set must be in [`RUNTIME_IMAGE_SETS`]; debug builds
/// check it.
pub fn runtime_image_set(name: &str) -> String {
    debug_assert!(
        RUNTIME_IMAGE_SETS.contains(&name),
        "image set {name:?} is switched to at run time but not listed in RUNTIME_IMAGE_SETS"
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
    /// Animations by registry key.
    pub animations: AnimSet,
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
            flags: 0,
        }
    }

    /// Prepares a loaded level; reads Claw's animations from `store`.
    pub fn from_level(level: &LevelData, store: &AssetStore) -> Result<SimLevel> {
        let (map, tile_report) = match level.main_plane.and_then(|i| level.planes.get(i)) {
            Some(plane) => TileMap::from_plane(plane, &level.tile_attributes),
            None => (TileMap::new(1, 1, 64, 64), TileReport::default()),
        };
        let objects = level
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
        let mut animations = HashMap::new();
        for (key, &file) in &level.registries.anis {
            if !key.starts_with("CLAW_") {
                continue;
            }
            let ani = Ani::parse(&store.read(file)?)?;
            animations.insert(key.clone(), Arc::new(Animation::from_ani(key, &ani)));
        }
        Ok(SimLevel {
            map,
            tile_report,
            start: (level.start_x, level.start_y),
            objects,
            animations,
            flags: level.header.flags,
        })
    }

    /// Every image set the level's objects can show, sorted and without
    /// duplicates: the sets placed objects start with, the sets the
    /// simulation's own logics switch to at run time
    /// ([`RUNTIME_IMAGE_SETS`]) and the sets named in the loaded animations'
    /// headers. Meant for loading graphics at level load, so that drawing
    /// never has to read or decode a set.
    ///
    /// The animation header names are best-effort: no object takes its
    /// image set from an animation, so a header name may not be a set the
    /// renderer draws with. Listing one costs a lookup at load and nothing
    /// else. The sets that matter are the placed ones and
    /// [`RUNTIME_IMAGE_SETS`].
    pub fn image_sets(&self) -> Vec<String> {
        let mut sets: BTreeSet<&str> = BTreeSet::new();
        sets.extend(RUNTIME_IMAGE_SETS);
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
