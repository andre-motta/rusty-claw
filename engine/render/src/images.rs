//! Decoded images in the layout the shaders read, and the graphics of one
//! level: planes, palette, tables, tiles and sprites.
//!
//! Every image (tile or sprite frame) is stored once as palette indices,
//! plus a coverage mask for sprite frames whose transparent pixels come from
//! their run-length encoding. Images flagged with the colour key treat index
//! 0 as transparent. All image bytes live in one byte array that the GPU
//! reads as 32-bit words, and every image has a fixed-size record pointing
//! into it.
//!
//! Plane cells are resolved once at load: each cell becomes the id of its
//! tile image, [`SLOT_EMPTY`] (nothing drawn: an empty cell, a tile that is
//! missing or smaller than the plane's tile size) or [`SLOT_FILL`] (the
//! plane's fill colour).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use claw_assets::pid::{FLAG_COLOR_KEY, FLAG_RLE};
use claw_assets::{AssetStore, Frame, ImageSet, LevelData, Palette, Plane, Registries};

use crate::effects::LevelTables;
use crate::pack::{HdPack, PackError, write_png};
use crate::view::{CELL_EMPTY, CELL_FILL};

/// Image flag: palette index 0 is transparent.
pub const IMAGE_KEYED: u32 = 0x1;
/// Image flag: a coverage mask follows the indices.
pub const IMAGE_MASKED: u32 = 0x2;
/// Image flag: a sprite run-length frame; mirrored copies shift one pixel.
pub const IMAGE_RLE: u32 = 0x4;
/// Image flag: the frame draws translucent unless told otherwise.
pub const IMAGE_GHOST: u32 = 0x8;

/// Image header flag that makes a frame translucent at load.
pub const FRAME_FLAG_GHOST: u32 = 0x40;

/// Resolved cell: nothing is drawn.
pub const SLOT_EMPTY: i32 = -1;
/// Resolved cell: the plane's fill colour.
pub const SLOT_FILL: i32 = -2;

/// Largest width or height of an image kept for drawing.
pub const MAX_IMAGE_DIMENSION: u32 = 1 << 14;

/// One image as the shaders see it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ImageRecord {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `IMAGE_*` flags.
    pub flags: u32,
    /// Byte offset of the indices in [`ImageStore::bytes`].
    pub index_offset: u32,
    /// Byte offset of the coverage mask (1 drawn, 0 transparent).
    pub mask_offset: u32,
    /// Texel offset of the replacement in [`ImageStore::hd`].
    pub hd_offset: u32,
    /// Replacement pixels per image pixel; 0 when there is none.
    pub hd_scale: u32,
    /// Unused.
    pub pad: u32,
}

/// Placement data that only the CPU needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageMeta {
    /// Horizontal hotspot offset.
    pub offset_x: i32,
    /// Vertical hotspot offset.
    pub offset_y: i32,
    /// Logical path of the original image, when it came from the store.
    pub path: Option<String>,
}

/// Every image of a level, packed.
#[derive(Debug, Clone, Default)]
pub struct ImageStore {
    /// Indices and masks, padded to whole 32-bit words.
    pub bytes: Vec<u8>,
    /// Replacement pixels as packed RGBA (red in the low byte).
    pub hd: Vec<u32>,
    /// One record per image id.
    pub records: Vec<ImageRecord>,
    /// One entry per image id.
    pub meta: Vec<ImageMeta>,
}

impl ImageStore {
    fn push_bytes(&mut self, data: &[u8]) -> u32 {
        let at = self.bytes.len();
        self.bytes.extend_from_slice(data);
        let pad = (4 - self.bytes.len() % 4) % 4;
        self.bytes.extend(std::iter::repeat_n(0, pad));
        at as u32
    }

    /// Adds a frame and returns its id. `path` is its logical path, used to
    /// find a replacement in `pack`.
    pub fn add(&mut self, frame: &Frame, path: Option<&str>, pack: Option<&mut HdPack>) -> u32 {
        let (w, h) = (frame.width, frame.height);
        let n = (w as usize) * (h as usize);
        let mut flags = 0;
        if frame.flags & FLAG_COLOR_KEY != 0 {
            flags |= IMAGE_KEYED;
        }
        if frame.flags & FLAG_RLE != 0 {
            flags |= IMAGE_RLE;
        }
        if frame.flags & FRAME_FLAG_GHOST != 0 && frame.flags & FLAG_RLE != 0 {
            flags |= IMAGE_GHOST;
        }
        let ok = frame.indices.len() == n;
        let index_offset = if ok {
            self.push_bytes(&frame.indices)
        } else {
            self.push_bytes(&vec![0; n])
        };
        let mut mask_offset = 0;
        if let Some(mask) = frame.opaque.as_ref().filter(|m| m.len() == n) {
            flags |= IMAGE_MASKED;
            mask_offset = self.push_bytes(mask);
        } else if !ok {
            flags |= IMAGE_MASKED;
            mask_offset = self.push_bytes(&vec![0; n]);
        }
        let (mut hd_offset, mut hd_scale) = (0, 0);
        if let (Some(path), Some(pack)) = (path, pack)
            && let Some(px) = pack.image_for(path, w, h)
        {
            hd_offset = self.hd.len() as u32;
            hd_scale = pack.scale();
            self.hd.extend_from_slice(&px);
        }
        self.records.push(ImageRecord {
            width: w,
            height: h,
            flags,
            index_offset,
            mask_offset,
            hd_offset,
            hd_scale,
            pad: 0,
        });
        self.meta.push(ImageMeta {
            offset_x: frame.offset_x,
            offset_y: frame.offset_y,
            path: path.map(str::to_string),
        });
        (self.records.len() - 1) as u32
    }

    /// The record of image `id`.
    pub fn record(&self, id: u32) -> &ImageRecord {
        &self.records[id as usize]
    }

    /// Palette index at (x, y) of image `id`.
    pub fn index_at(&self, id: u32, x: u32, y: u32) -> u8 {
        let r = self.record(id);
        self.bytes[(r.index_offset + y * r.width + x) as usize]
    }

    /// True when (x, y) of image `id` is drawn.
    pub fn covered(&self, id: u32, x: u32, y: u32) -> bool {
        let r = self.record(id);
        if r.flags & IMAGE_MASKED != 0
            && self.bytes[(r.mask_offset + y * r.width + x) as usize] == 0
        {
            return false;
        }
        !(r.flags & IMAGE_KEYED != 0 && self.index_at(id, x, y) == 0)
    }

    /// Replacement texel (hx, hy) of image `id`.
    pub fn hd_at(&self, id: u32, hx: u32, hy: u32) -> u32 {
        let r = self.record(id);
        self.hd[(r.hd_offset + hy * r.width * r.hd_scale + hx) as usize]
    }
}

/// One frame of an image set, as an [`ImageSource`] returns it.
#[derive(Debug, Clone)]
pub struct SourceFrame {
    /// Frame number in the set.
    pub number: i32,
    /// The decoded frame.
    pub frame: Frame,
    /// Logical path of the frame's file, if known.
    pub path: Option<String>,
}

/// Where a level's images come from.
pub trait ImageSource: Send {
    /// Every decodable frame of the image set `key`, or `None` when there is
    /// no such set.
    fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>>;
}

/// Images from an asset store through a level's registries.
#[derive(Debug, Clone)]
pub struct StoreImages {
    store: AssetStore,
    registries: Registries,
}

impl StoreImages {
    /// Images of `registries` read from `store`.
    pub fn new(store: AssetStore, registries: Registries) -> StoreImages {
        StoreImages { store, registries }
    }
}

impl ImageSource for StoreImages {
    fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>> {
        let set = self.registries.images.get(key)?;
        let mut out = Vec::with_capacity(set.frames.len());
        for (&number, &id) in &set.frames {
            if let Ok(frame) = ImageSet::load_frame(&self.store, set, number) {
                if frame.width > MAX_IMAGE_DIMENSION || frame.height > MAX_IMAGE_DIMENSION {
                    continue;
                }
                out.push(SourceFrame {
                    number,
                    frame,
                    path: Some(self.store.file(id).path().to_string()),
                });
            }
        }
        Some(out)
    }
}

/// Images held in memory, for tests and generated content.
#[derive(Debug, Clone, Default)]
pub struct MemoryImages {
    /// Frames by set key.
    pub sets: HashMap<String, Vec<SourceFrame>>,
}

impl MemoryImages {
    /// Adds a frame to a set; the path defaults to `KEY/number`.
    pub fn insert(&mut self, key: &str, number: i32, frame: Frame) {
        self.sets
            .entry(key.to_string())
            .or_default()
            .push(SourceFrame {
                number,
                frame,
                path: Some(format!("{key}/{number:03}")),
            });
    }
}

impl ImageSource for MemoryImages {
    fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>> {
        self.sets.get(key).cloned()
    }
}

/// Claw's image set.
pub const CLAW_SET: &str = "CLAW";

/// Extra vertical draw offsets of Claw's climb-over frames, 383 to 389.
/// The game adds these when it sets Claw up, so the pose follows his feet
/// while he climbs over a ladder top; the files alone draw it up to 45 px
/// too high.
pub const CLIMB_OVER_FIX: [(i32, i32); 7] = [
    (383, 4),
    (384, 8),
    (385, 12),
    (386, 16),
    (387, 40),
    (388, 54),
    (389, 58),
];

/// The vertical draw offset added to frame `number` of the set `key`.
pub fn draw_offset_fix(key: &str, number: i32) -> i32 {
    if key != CLAW_SET {
        return 0;
    }
    CLIMB_OVER_FIX
        .iter()
        .find(|&&(n, _)| n == number)
        .map_or(0, |&(_, dy)| dy)
}

/// Frames of one loaded set: frame number to image id.
pub type FrameMap = BTreeMap<i32, u32>;

/// Size and hotspot offsets of one loaded frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FrameInfo {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Horizontal hotspot offset.
    pub offset_x: i32,
    /// Vertical hotspot offset.
    pub offset_y: i32,
}

/// Everything the renderer draws for one level.
pub struct LevelGraphics {
    /// Planes in level order.
    pub planes: Vec<Plane>,
    /// True when objects are drawn interleaved with the planes by z (the
    /// level flag every shipped level sets); otherwise every object is drawn
    /// right after the main plane.
    pub interleaved: bool,
    /// The level palette.
    pub palette: Palette,
    /// The table set of the level.
    pub tables: LevelTables,
    /// All images loaded so far.
    pub images: ImageStore,
    /// Per plane: one resolved slot per cell.
    pub plane_slots: Vec<Vec<i32>>,
    sprites: HashMap<String, Option<FrameMap>>,
    loaded_while_drawing: usize,
    source: Box<dyn ImageSource>,
    pack: Option<HdPack>,
}

impl std::fmt::Debug for LevelGraphics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelGraphics")
            .field("planes", &self.planes.len())
            .field("interleaved", &self.interleaved)
            .field("images", &self.images.records.len())
            .field("sprite_sets", &self.sprites.len())
            .finish()
    }
}

/// Header flag of a level that interleaves objects and planes by z.
pub const LEVEL_FLAG_INTERLEAVED: u32 = 0x1;

impl LevelGraphics {
    /// Graphics for `planes`, loading tile sets from `source` and resolving
    /// every cell.
    pub fn new(
        mut planes: Vec<Plane>,
        interleaved: bool,
        palette: Palette,
        tables: LevelTables,
        source: Box<dyn ImageSource>,
        pack: Option<HdPack>,
    ) -> LevelGraphics {
        // A grid whose cell count does not match its size is padded with
        // empty cells, or emptied when the size is absurd.
        for p in &mut planes {
            let cells = usize::try_from(p.w_tiles.max(0))
                .ok()
                .zip(usize::try_from(p.h_tiles.max(0)).ok())
                .and_then(|(w, h)| w.checked_mul(h))
                .filter(|&n| n <= p.tiles.len().max(1 << 24));
            match cells {
                Some(n) => p.tiles.resize(n, CELL_EMPTY),
                None => {
                    p.w_tiles = 0;
                    p.h_tiles = 0;
                    p.tiles.clear();
                }
            }
        }
        let mut g = LevelGraphics {
            planes,
            interleaved,
            palette,
            tables,
            images: ImageStore::default(),
            plane_slots: Vec::new(),
            sprites: HashMap::new(),
            loaded_while_drawing: 0,
            source,
            pack,
        };
        g.resolve_planes();
        g
    }

    /// Graphics of a loaded level, reading images from `store`.
    pub fn from_level(
        level: &LevelData,
        store: &AssetStore,
        pack: Option<HdPack>,
    ) -> LevelGraphics {
        let tables = LevelTables::for_level(store, level.level, &level.palette);
        LevelGraphics::new(
            level.planes.clone(),
            level.header.flags & LEVEL_FLAG_INTERLEAVED != 0,
            level.palette.clone(),
            tables,
            Box::new(StoreImages::new(store.clone(), level.registries.clone())),
            pack,
        )
    }

    /// Graphics with no level, for standalone screens: sprite sets come from
    /// `source`, backgrounds are added with [`LevelGraphics::add_image`], and
    /// the tables are blank (the table draw modes draw index 0).
    pub fn for_screens(palette: Palette, source: Box<dyn ImageSource>) -> LevelGraphics {
        let tables = LevelTables::blank(&palette);
        LevelGraphics::new(Vec::new(), true, palette, tables, source, None)
    }

    fn load_set(&mut self, key: &str) -> Option<FrameMap> {
        let frames = self.source.image_set(key)?;
        let mut map = FrameMap::new();
        for f in frames {
            if map.contains_key(&f.number) {
                continue;
            }
            let id = self
                .images
                .add(&f.frame, f.path.as_deref(), self.pack.as_mut());
            self.images.meta[id as usize].offset_y += draw_offset_fix(key, f.number);
            map.insert(f.number, id);
        }
        Some(map)
    }

    fn resolve_planes(&mut self) {
        let mut sets: HashMap<String, Option<FrameMap>> = HashMap::new();
        let mut all = Vec::with_capacity(self.planes.len());
        for pi in 0..self.planes.len() {
            let names = self.planes[pi].image_sets.clone();
            let (tw, th) = (self.planes[pi].tile_w, self.planes[pi].tile_h);
            let mut maps: Vec<Option<FrameMap>> = Vec::with_capacity(names.len());
            for name in &names {
                if !sets.contains_key(name) {
                    let loaded = self.load_set(name);
                    sets.insert(name.clone(), loaded);
                }
                maps.push(sets.get(name).cloned().flatten());
            }
            let slots: Vec<i32> = self.planes[pi]
                .tiles
                .iter()
                .map(|&cell| {
                    if cell == CELL_EMPTY {
                        return SLOT_EMPTY;
                    }
                    if cell == CELL_FILL {
                        return SLOT_FILL;
                    }
                    let set = (cell as u32 >> 16) as usize;
                    let number = (cell as u32 & 0xFFFF) as i32;
                    let Some(Some(map)) = maps.get(set) else {
                        return SLOT_EMPTY;
                    };
                    match map.get(&number) {
                        Some(&id) => {
                            let r = self.images.record(id);
                            let fits = i64::from(r.width) >= i64::from(tw)
                                && i64::from(r.height) >= i64::from(th);
                            if fits { id as i32 } else { SLOT_EMPTY }
                        }
                        None => SLOT_EMPTY,
                    }
                })
                .collect();
            all.push(slots);
        }
        self.plane_slots = all;
    }

    /// Reads and decodes the sprite sets `keys` now, at level load, so that
    /// drawing them later does no file reading or decoding. Sets already
    /// loaded and keys with no set are remembered and skipped; the result is
    /// the number of keys looked up for the first time.
    ///
    /// Call it with every image set the level's objects can show, including
    /// the sets their logics switch to at run time.
    pub fn preload<I>(&mut self, keys: I) -> usize
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut added = 0;
        for key in keys {
            let key = key.as_ref();
            if key.is_empty() || self.sprites.contains_key(key) {
                continue;
            }
            let loaded = self.load_set(key);
            self.sprites.insert(key.to_string(), loaded);
            added += 1;
        }
        added
    }

    /// True when the sprite set `key` has been looked up (found or not).
    pub fn is_loaded(&self, key: &str) -> bool {
        self.sprites.contains_key(key)
    }

    /// Sprite sets that were first looked up while drawing, because
    /// [`LevelGraphics::preload`] did not name them. Each one read and
    /// decoded files on the frame path; a complete preload keeps this at 0.
    pub fn loaded_while_drawing(&self) -> usize {
        self.loaded_while_drawing
    }

    /// The image id of `frame` of sprite set `key`. A set that was not
    /// preloaded is loaded here on first use (and counted by
    /// [`LevelGraphics::loaded_while_drawing`]). Frame -1 means the set's
    /// first frame.
    pub fn sprite(&mut self, key: &str, frame: i32) -> Option<u32> {
        if !self.sprites.contains_key(key) {
            let loaded = self.load_set(key);
            self.sprites.insert(key.to_string(), loaded);
            self.loaded_while_drawing += 1;
        }
        let map = self.sprites.get(key)?.as_ref()?;
        if frame == -1 {
            return map.values().next().copied();
        }
        map.get(&frame).copied()
    }

    /// Placement data of image `id`.
    pub fn meta(&self, id: u32) -> &ImageMeta {
        &self.images.meta[id as usize]
    }

    /// Size and hotspot offsets of `frame` of sprite set `key` (frame -1 is
    /// the set's first frame), loading the set like [`LevelGraphics::sprite`]
    /// when it was not preloaded.
    pub fn frame_info(&mut self, key: &str, frame: i32) -> Option<FrameInfo> {
        let id = self.sprite(key, frame)?;
        let r = self.images.record(id);
        let m = self.meta(id);
        Some(FrameInfo {
            width: r.width,
            height: r.height,
            offset_x: m.offset_x,
            offset_y: m.offset_y,
        })
    }

    /// The frame numbers of sprite set `key` in ascending order (empty when
    /// there is no such set), loading the set like [`LevelGraphics::sprite`]
    /// when it was not preloaded.
    pub fn frame_numbers(&mut self, key: &str) -> Vec<i32> {
        if self.sprite(key, -1).is_none() {
            return Vec::new();
        }
        self.sprites
            .get(key)
            .and_then(Option::as_ref)
            .map(|m| m.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Adds an image that belongs to no set, such as a screen's background,
    /// and returns its id. It is drawn as is: every pixel opaque, without a
    /// replacement image.
    pub fn add_image(&mut self, frame: &Frame) -> u32 {
        self.images.add(frame, None, None)
    }

    /// Warnings from the image pack, if one is in use.
    pub fn pack_warnings(&self) -> &[String] {
        self.pack.as_ref().map_or(&[], |p| p.warnings.as_slice())
    }

    /// Writes an image pack that repeats every loaded image `scale` times in
    /// each direction with nearest sampling, in the level palette. Drawing
    /// with it must look exactly like drawing without it; it exists to test
    /// the pack path and as a starting point for pack authors.
    pub fn write_nearest_pack(&self, dir: &Path, scale: u32) -> Result<usize, PackError> {
        let io = |source| PackError::Io {
            path: dir.to_path_buf(),
            source,
        };
        std::fs::create_dir_all(dir).map_err(io)?;
        let mut manifest = format!("scale {scale}\n");
        let mut written = 0;
        for (id, meta) in self.images.meta.iter().enumerate() {
            let Some(path) = &meta.path else { continue };
            let id = id as u32;
            let r = *self.images.record(id);
            let (w, h) = (r.width * scale, r.height * scale);
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let (sx, sy) = (x / scale, y / scale);
                    let c = self.palette.color(self.images.index_at(id, sx, sy));
                    let a = if self.images.covered(id, sx, sy) {
                        255
                    } else {
                        0
                    };
                    rgba.extend_from_slice(&[c[0], c[1], c[2], a]);
                }
            }
            let file = format!("{id:06}.png");
            write_png(&dir.join(&file), w, h, &rgba)?;
            manifest.push_str(&format!("{path} = {file}\n"));
            written += 1;
        }
        std::fs::write(dir.join(crate::pack::MANIFEST), manifest).map_err(io)?;
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn frame(w: u32, h: u32, flags: u32, f: impl Fn(u32, u32) -> u8) -> Frame {
        let mut indices = Vec::new();
        for y in 0..h {
            for x in 0..w {
                indices.push(f(x, y));
            }
        }
        Frame {
            width: w,
            height: h,
            offset_x: 0,
            offset_y: 0,
            flags,
            indices,
            opaque: None,
        }
    }

    #[test]
    fn store_packs_words_and_masks() {
        let mut s = ImageStore::default();
        let a = s.add(&frame(3, 1, FLAG_COLOR_KEY, |x, _| x as u8), None, None);
        let mut f = frame(2, 2, FLAG_RLE, |x, y| (10 + x + y) as u8);
        f.opaque = Some(vec![1, 0, 0, 1]);
        let b = s.add(&f, None, None);
        assert_eq!(s.bytes.len() % 4, 0);
        assert!(!s.covered(a, 0, 0));
        assert!(s.covered(a, 1, 0));
        assert_eq!(s.index_at(b, 1, 1), 12);
        assert!(s.covered(b, 0, 0));
        assert!(!s.covered(b, 1, 0));
        assert_eq!(s.record(b).flags, IMAGE_RLE | IMAGE_MASKED);
    }

    #[test]
    fn cells_resolve_to_slots() {
        let mut src = MemoryImages::default();
        src.insert("T", 1, frame(4, 4, 0, |_, _| 7));
        src.insert("T", 2, frame(2, 2, 0, |_, _| 8));
        src.insert("U", 5, frame(4, 4, 0, |_, _| 9));
        let plane = Plane {
            name: "A".into(),
            tile_w: 4,
            tile_h: 4,
            w_tiles: 6,
            h_tiles: 1,
            image_sets: vec!["T".into(), "U".into()],
            tiles: vec![1, 2, 3, CELL_EMPTY, CELL_FILL, (1 << 16) | 5],
            ..Plane::default()
        };
        let pal = Palette::default();
        let tables = LevelTables::blank(&pal);
        let g = LevelGraphics::new(vec![plane], true, pal, tables, Box::new(src), None);
        let s = &g.plane_slots[0];
        assert!(s[0] >= 0);
        assert_eq!(s[1], SLOT_EMPTY, "smaller than the tile");
        assert_eq!(s[2], SLOT_EMPTY, "missing");
        assert_eq!(s[3], SLOT_EMPTY);
        assert_eq!(s[4], SLOT_FILL);
        assert_eq!(g.images.index_at(s[5] as u32, 0, 0), 9);
    }

    /// Counts every set read from the wrapped source.
    #[derive(Debug, Clone)]
    struct Counting {
        inner: MemoryImages,
        reads: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl ImageSource for Counting {
        fn image_set(&self, key: &str) -> Option<Vec<SourceFrame>> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.image_set(key)
        }
    }

    #[test]
    fn preloaded_sets_are_not_read_again() {
        let mut inner = MemoryImages::default();
        inner.insert("A", 1, frame(2, 2, 0, |_, _| 3));
        inner.insert("A", 2, frame(2, 2, 0, |_, _| 4));
        inner.insert("B", 7, frame(1, 1, 0, |_, _| 5));
        let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let src = Counting {
            inner,
            reads: reads.clone(),
        };
        let pal = Palette::default();
        let mut g = LevelGraphics::new(
            Vec::new(),
            true,
            pal.clone(),
            LevelTables::blank(&pal),
            Box::new(src),
            None,
        );
        let count = || reads.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(g.preload(["A", "B", "MISSING", "A", ""]), 3);
        assert_eq!(count(), 3);
        assert!(g.is_loaded("A") && g.is_loaded("MISSING"));
        let images = g.images.records.len();
        assert_eq!(g.preload(["B"]), 0, "already loaded");
        // Drawing reads and decodes nothing more.
        assert!(g.sprite("A", 2).is_some());
        assert!(g.sprite("B", -1).is_some());
        assert!(g.sprite("MISSING", 1).is_none());
        assert_eq!(count(), 3);
        assert_eq!(g.images.records.len(), images);
        assert_eq!(g.loaded_while_drawing(), 0);
        // A set left out of the preload is loaded late, and counted.
        assert!(g.sprite("C", 1).is_none());
        assert_eq!(count(), 4);
        assert_eq!(g.loaded_while_drawing(), 1);
    }

    #[test]
    fn grids_match_their_size() {
        let pal = Palette::default();
        let short = Plane {
            tile_w: 4,
            tile_h: 4,
            w_tiles: 3,
            h_tiles: 2,
            tiles: vec![1, 2],
            ..Plane::default()
        };
        let absurd = Plane {
            tile_w: 4,
            tile_h: 4,
            w_tiles: i32::MAX,
            h_tiles: i32::MAX,
            ..Plane::default()
        };
        let g = LevelGraphics::new(
            vec![short, absurd],
            true,
            pal.clone(),
            LevelTables::blank(&pal),
            Box::new(MemoryImages::default()),
            None,
        );
        assert_eq!(
            g.planes[0].tiles,
            vec![1, 2, CELL_EMPTY, CELL_EMPTY, CELL_EMPTY, CELL_EMPTY]
        );
        assert_eq!(g.plane_slots[0].len(), 6);
        assert_eq!((g.planes[1].w_tiles, g.plane_slots[1].len()), (0, 0));
    }
    #[test]
    fn claw_climb_over_frames_are_drawn_lower() {
        let mut src = MemoryImages::default();
        for n in [1, 383, 386, 389] {
            src.insert(CLAW_SET, n, frame(2, 2, 0, |_, _| 1));
        }
        src.insert("OTHER", 389, frame(2, 2, 0, |_, _| 1));
        let pal = Palette::default();
        let mut g = LevelGraphics::new(
            Vec::new(),
            true,
            pal.clone(),
            LevelTables::blank(&pal),
            Box::new(src),
            None,
        );
        let dy = |g: &mut LevelGraphics, key: &str, n: i32| g.frame_info(key, n).unwrap().offset_y;
        assert_eq!(dy(&mut g, CLAW_SET, 1), 0);
        assert_eq!(dy(&mut g, CLAW_SET, 383), 4);
        assert_eq!(dy(&mut g, CLAW_SET, 386), 16);
        assert_eq!(dy(&mut g, CLAW_SET, 389), 58);
        assert_eq!(dy(&mut g, "OTHER", 389), 0, "other sets keep their offsets");
    }
}
