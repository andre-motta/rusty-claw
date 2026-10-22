//! What the simulation needs from a level: its number, the main plane's
//! collision tiles, the start point, the object records, the animations its
//! logics play (with the sounds of their frames), the frame numbers and
//! frame boxes of its image sets and the length of every registered sound.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use claw_assets::name::TYPE_PID;
use claw_assets::wwd::OBJECT_INTS;
use claw_assets::{Ani, AssetStore, LevelData, Result, pid, wav};

use crate::anim::Animation;
use crate::logic;
use crate::object::{Object, Rect, draw_flags};
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

/// What the simulation knows of one image set: the range of its frame
/// numbers and the box each frame covers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageFrames {
    /// Lowest frame number of the set.
    pub first: i32,
    /// Highest frame number of the set.
    pub last: i32,
    /// Box of each frame whose image could be read, by frame number,
    /// relative to the object's position (see [`frame_box`]).
    pub boxes: BTreeMap<i32, Rect>,
    /// Box each frame's picture is drawn in, by frame number, relative to
    /// the object's position (see [`picture_box`]). A frame missing here
    /// is drawn in its [`ImageFrames::boxes`] box.
    pub picture_boxes: BTreeMap<i32, Rect>,
}

impl ImageFrames {
    /// A set of frames `first..=last` with no boxes known.
    pub fn range(first: i32, last: i32) -> ImageFrames {
        ImageFrames {
            first,
            last,
            boxes: BTreeMap::new(),
            picture_boxes: BTreeMap::new(),
        }
    }
}

/// The box a frame of `width` x `height` pixels with hotspot offset
/// (`offset_x`, `offset_y`) covers around its object's position, edges
/// included: `{ox - w/2, oy - h/2, ox + w/2 - 1, oy + h/2 - 1}`, the halves
/// rounded down, so a frame of odd size loses its last row and column.
/// This is the frame box logics use for pickup, hit and move boxes; where
/// the frame is drawn is its [`picture_box`].
pub fn frame_box(width: i32, height: i32, offset_x: i32, offset_y: i32) -> Rect {
    let (cx, cy) = (width >> 1, height >> 1);
    Rect::new(
        offset_x.saturating_sub(cx),
        offset_y.saturating_sub(cy),
        offset_x.saturating_add(cx).saturating_sub(1),
        offset_y.saturating_add(cy).saturating_sub(1),
    )
}

/// The box a picture of `width` x `height` pixels with hotspot offset
/// (`offset_x`, `offset_y`) is drawn in around its object's position,
/// edges included: every pixel of the picture, from
/// `{ox - w/2, oy - h/2}` (the halves rounded down) to `width` and
/// `height` pixels further, so a picture of odd size keeps its last row
/// and column. This is the box that says whether an object is on screen;
/// the logic boxes stay [`frame_box`].
pub fn picture_box(width: i32, height: i32, offset_x: i32, offset_y: i32) -> Rect {
    let left = offset_x.saturating_sub(width >> 1);
    let top = offset_y.saturating_sub(height >> 1);
    Rect::new(
        left,
        top,
        left.saturating_add(width).saturating_sub(1),
        top.saturating_add(height).saturating_sub(1),
    )
}

/// Size and hotspot offset of a PID image read from its header:
/// `(width, height, offset_x, offset_y)`; `None` when the header is short
/// or the size is not a valid image size.
fn pid_geometry(data: &[u8]) -> Option<(i32, i32, i32, i32)> {
    if data.len() < pid::HEADER_SIZE {
        return None;
    }
    let int = |at: usize| -> Option<i32> {
        let bytes: [u8; 4] = data.get(at..at + 4)?.try_into().ok()?;
        Some(i32::from_le_bytes(bytes))
    };
    let (w, h) = (int(8)?, int(12)?);
    if w <= 0 || h <= 0 || w > pid::MAX_DIMENSION || h > pid::MAX_DIMENSION {
        return None;
    }
    Some((w, h, int(16)?, int(20)?))
}

/// The picture box of a PID image read from its header (see
/// [`picture_box`]); `None` when the header is short or the size is not a
/// valid image size.
pub fn pid_picture_box(data: &[u8]) -> Option<Rect> {
    pid_geometry(data).map(|(w, h, ox, oy)| picture_box(w, h, ox, oy))
}

/// The frame box of a PID image read from its header (size and hotspot
/// offset); `None` when the header is short or the size is not a valid
/// image size.
pub fn pid_frame_box(data: &[u8]) -> Option<Rect> {
    pid_geometry(data).map(|(w, h, ox, oy)| frame_box(w, h, ox, oy))
}

/// Retail level number a level counts as when nothing says otherwise.
pub const DEFAULT_LEVEL_NUMBER: u32 = 1;

/// A level prepared for the simulation.
#[derive(Debug, Clone)]
pub struct SimLevel {
    /// Retail level number (1 to 14); for a community custom level, the
    /// number of the retail level it builds on. Logics whose behaviour
    /// differs between levels read it.
    pub number: u32,
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
    /// Frame range of every registered image set by key, with the frame
    /// boxes of the sets the level's objects can show (those of
    /// [`SimLevel::image_sets`]).
    pub images: BTreeMap<String, ImageFrames>,
}

impl SimLevel {
    /// A level made of a tile map and a start point, with no objects or
    /// animations (tests and tools).
    /// It counts as level [`DEFAULT_LEVEL_NUMBER`] and knows no image
    /// sets.
    pub fn synthetic(map: TileMap, start: (i32, i32)) -> SimLevel {
        SimLevel {
            number: DEFAULT_LEVEL_NUMBER,
            map,
            tile_report: TileReport::default(),
            start,
            objects: Vec::new(),
            animations: HashMap::new(),
            sounds: BTreeMap::new(),
            skipped_animations: Vec::new(),
            flags: 0,
            images: BTreeMap::new(),
        }
    }

    /// Prepares a loaded level: reads the animations its logics play, the
    /// length of every registered sound and the frame boxes of the image
    /// sets its objects can show from `store`. A sound that cannot be read
    /// keeps an unknown length, a frame whose image cannot be read has no
    /// box, and an animation other than Claw's that cannot be read is left
    /// out; Claw's animations must load.
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
        let images = level
            .registries
            .images
            .iter()
            .filter_map(|(key, set)| {
                let first = *set.frames.keys().next()?;
                let last = *set.frames.keys().next_back()?;
                Some((key.clone(), ImageFrames::range(first, last)))
            })
            .collect();
        let mut sim_level = SimLevel {
            number: level.level,
            map,
            tile_report,
            start: (level.start_x, level.start_y),
            objects,
            animations,
            sounds,
            skipped_animations,
            flags: level.header.flags,
            images,
        };
        for key in sim_level.image_sets() {
            let (Some(set), Some(frames)) = (
                level.registries.images.get(&key),
                sim_level.images.get_mut(&key),
            ) else {
                continue;
            };
            for (&index, &file) in &set.frames {
                if store.file(file).type_code() != TYPE_PID {
                    continue;
                }
                let Some((w, h, ox, oy)) = store.read(file).ok().and_then(|d| pid_geometry(&d))
                else {
                    continue;
                };
                frames.boxes.insert(index, frame_box(w, h, ox, oy));
                frames
                    .picture_boxes
                    .insert(index, picture_box(w, h, ox, oy));
            }
        }
        Ok(sim_level)
    }

    /// First and last frame numbers of the image set `set`; `None` when the
    /// level registers no such set.
    pub fn image_frames(&self, set: &str) -> Option<(i32, i32)> {
        self.images.get(set).map(|f| (f.first, f.last))
    }

    /// Box of frame `frame` of the image set `set` relative to the object's
    /// position (see [`frame_box`]); `None` when the set, the frame or its
    /// image is unknown.
    pub fn frame_box(&self, set: &str, frame: i32) -> Option<Rect> {
        self.images.get(set)?.boxes.get(&frame).copied()
    }

    /// Box frame `frame` of the image set `set` is drawn in, relative to
    /// the object's position (see [`picture_box`]): its picture box, or its
    /// frame box when only that is known; `None` when the set, the frame or
    /// its image is unknown.
    pub fn picture_box(&self, set: &str, frame: i32) -> Option<Rect> {
        let f = self.images.get(set)?;
        f.picture_boxes
            .get(&frame)
            .or_else(|| f.boxes.get(&frame))
            .copied()
    }

    /// The box `o` is drawn in, in level pixels: the picture box of its
    /// current frame (every pixel of the picture, so a picture of odd size
    /// keeps its last row and column), mirrored and flipped as it is drawn,
    /// at its position; `None` when the level does not know the frame. An
    /// object is on screen when any pixel of this box is in the view. A
    /// mirrored picture is flipped about its anchor pixel, so its box is
    /// the mirrored box one pixel further left: a soldier facing left whose
    /// box reaches the view's left edge in the plain mirror does not count
    /// as on screen.
    pub fn draw_box(&self, o: &Object) -> Option<Rect> {
        let mut b = self.picture_box(&o.image_set, o.frame)?;
        if o.draw_flags & draw_flags::MIRROR != 0 {
            let m = b.mirrored();
            b = Rect::new(m.left - 1, m.top, m.right - 1, m.bottom);
        }
        if o.draw_flags & draw_flags::INVERT != 0 {
            b = Rect::new(
                b.left,
                b.bottom.wrapping_neg(),
                b.right,
                b.top.wrapping_neg(),
            );
        }
        Some(b.translate(o.x, o.y))
    }

    /// True when `o` is in `view` (level pixels): its draw box overlaps it,
    /// or, when the level does not know its frame, its position is inside.
    pub fn in_view(&self, o: &Object, view: &Rect) -> bool {
        match self.draw_box(o) {
            Some(b) => b.overlaps(view),
            None => view.contains(o.x, o.y),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_boxes_centre_the_image_on_its_hotspot() {
        assert_eq!(frame_box(20, 30, 0, 0), Rect::new(-10, -15, 9, 14));
        assert_eq!(frame_box(20, 30, 4, -6), Rect::new(-6, -21, 13, 8));
        // Odd sizes lose their last row and column, as the halves round
        // down.
        assert_eq!(frame_box(5, 3, 0, 0), Rect::new(-2, -1, 1, 0));
    }

    #[test]
    fn picture_boxes_keep_every_row_and_column_of_the_picture() {
        assert_eq!(picture_box(20, 30, 0, 0), frame_box(20, 30, 0, 0));
        assert_eq!(picture_box(20, 30, 4, -6), frame_box(20, 30, 4, -6));
        // An odd size keeps its last row and column: 5 by 3 pixels.
        assert_eq!(picture_box(5, 3, 0, 0), Rect::new(-2, -1, 2, 1));
        // A picture 84 by 111 with offset (12, 9): 111 rows, from 55 above
        // the anchor's row plus the offset.
        let b = picture_box(84, 111, 12, 9);
        assert_eq!(b, Rect::new(-30, -46, 53, 64));
        assert_eq!(b.bottom - b.top + 1, 111);
        assert_eq!(frame_box(84, 111, 12, 9).bottom, 63);
        let data = pid::build(84, 111, pid::FLAG_RLE, (12, 9), &[]);
        assert_eq!(pid_picture_box(&data), Some(b));
        assert_eq!(pid_picture_box(&data[..pid::HEADER_SIZE - 1]), None);
    }

    #[test]
    fn frame_boxes_come_from_the_image_header() {
        let data = pid::build(20, 30, pid::FLAG_RLE, (4, -6), &[]);
        assert_eq!(pid_frame_box(&data), Some(Rect::new(-6, -21, 13, 8)));
        assert_eq!(pid_frame_box(&data[..pid::HEADER_SIZE - 1]), None);
        let bad = pid::build(0, 30, pid::FLAG_RLE, (0, 0), &[]);
        assert_eq!(pid_frame_box(&bad), None);
        let huge = pid::build(pid::MAX_DIMENSION + 1, 4, 0, (0, 0), &[]);
        assert_eq!(pid_frame_box(&huge), None);
    }

    #[test]
    fn image_sets_report_their_range_and_boxes() {
        let mut l = SimLevel::synthetic(TileMap::new(4, 4, 64, 64), (0, 0));
        assert_eq!(l.number, DEFAULT_LEVEL_NUMBER);
        assert!(l.images.is_empty());
        let mut set = ImageFrames::range(3, 9);
        set.boxes.insert(4, Rect::new(-1, -2, 3, 4));
        l.images.insert("SET".into(), set);
        assert_eq!(l.image_frames("SET"), Some((3, 9)));
        assert_eq!(l.image_frames("OTHER"), None);
        assert_eq!(l.frame_box("SET", 4), Some(Rect::new(-1, -2, 3, 4)));
        assert_eq!(l.frame_box("SET", 5), None);
        assert_eq!(l.frame_box("OTHER", 4), None);
        // Without its picture box a frame is drawn in its frame box.
        assert_eq!(l.picture_box("SET", 4), Some(Rect::new(-1, -2, 3, 4)));
        l.images
            .get_mut("SET")
            .unwrap()
            .picture_boxes
            .insert(4, Rect::new(-1, -2, 3, 5));
        assert_eq!(l.picture_box("SET", 4), Some(Rect::new(-1, -2, 3, 5)));
        assert_eq!(l.frame_box("SET", 4), Some(Rect::new(-1, -2, 3, 4)));
        assert_eq!(l.picture_box("SET", 5), None);
    }
}
