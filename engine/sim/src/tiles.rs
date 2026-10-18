//! Tile collision attributes of the main plane.
//!
//! The main plane is a grid of tiles; each tile number has a descriptor
//! saying what the tile is made of: one attribute for the whole tile, one
//! inside a rectangle and another outside it, or one per pixel. Lookups are
//! per pixel, and a pixel outside the plane takes the attribute of the
//! nearest edge pixel, so the edge tiles extend outward forever.

use claw_assets::wwd::{Plane, TILE_EMPTY, TILE_FILLED, TileDescriptor};

/// Empty space.
pub const CLEAR: u8 = 0;
/// Blocks from every side.
pub const SOLID: u8 = 1;
/// A one-way floor: stops objects coming down onto it only.
pub const GROUND: u8 = 2;
/// A ladder or rope; its top row is a floor.
pub const CLIMB: u8 = 3;
/// Spikes, water or lava: marks the object that lands on it.
pub const DEATH: u8 = 4;

/// Collision attributes of one tile number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descriptor {
    /// One attribute everywhere.
    Single {
        /// Tile width.
        width: i32,
        /// Tile height.
        height: i32,
        /// The attribute.
        attr: u8,
    },
    /// `inside` within the inclusive rectangle, `outside` elsewhere.
    Double {
        /// Tile width.
        width: i32,
        /// Tile height.
        height: i32,
        /// Attribute outside the rectangle.
        outside: u8,
        /// Attribute inside the rectangle.
        inside: u8,
        /// Left, top, right, bottom in tile pixels.
        rect: [i32; 4],
    },
    /// One attribute per pixel, row-major.
    Mask {
        /// Mask width.
        width: i32,
        /// Mask height.
        height: i32,
        /// `width * height` attributes.
        mask: Vec<u8>,
    },
}

/// Attributes outside 0..=4 behave like [`CLEAR`].
fn known(attr: u32) -> u8 {
    if attr <= u32::from(DEATH) {
        attr as u8
    } else {
        CLEAR
    }
}

impl Descriptor {
    /// Converts a level-file descriptor; unknown attribute values become
    /// [`CLEAR`].
    pub fn from_level(desc: &TileDescriptor) -> Descriptor {
        match desc {
            TileDescriptor::Single {
                width,
                height,
                attribute,
            } => Descriptor::Single {
                width: *width,
                height: *height,
                attr: known(*attribute),
            },
            TileDescriptor::Double {
                width,
                height,
                outside,
                inside,
                rect,
            } => Descriptor::Double {
                width: *width,
                height: *height,
                outside: known(*outside),
                inside: known(*inside),
                rect: *rect,
            },
            TileDescriptor::Mask {
                width,
                height,
                mask,
            } => Descriptor::Mask {
                width: *width,
                height: *height,
                mask: mask.iter().map(|&b| known(u32::from(b))).collect(),
            },
        }
    }

    /// A whole tile of one attribute.
    pub const fn single(size: i32, attr: u8) -> Descriptor {
        Descriptor::Single {
            width: size,
            height: size,
            attr,
        }
    }

    /// Attribute at a tile-local pixel.
    pub fn attr_at(&self, lx: i32, ly: i32) -> u8 {
        match self {
            Descriptor::Single { attr, .. } => *attr,
            Descriptor::Double {
                outside,
                inside,
                rect,
                ..
            } => {
                if rect[0] <= lx && lx <= rect[2] && rect[1] <= ly && ly <= rect[3] {
                    *inside
                } else {
                    *outside
                }
            }
            Descriptor::Mask { width, mask, .. } => {
                if lx < 0 || ly < 0 || lx >= *width {
                    return CLEAR;
                }
                let idx = ly as usize * *width as usize + lx as usize;
                mask.get(idx).copied().unwrap_or(CLEAR)
            }
        }
    }

    /// The descriptor's own width (used to centre on ladders).
    pub fn width(&self) -> i32 {
        match self {
            Descriptor::Single { width, .. }
            | Descriptor::Double { width, .. }
            | Descriptor::Mask { width, .. } => *width,
        }
    }
}

/// The main plane's tile grid with its descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileMap {
    w_tiles: i32,
    h_tiles: i32,
    tile_w: i32,
    tile_h: i32,
    cells: Vec<i32>,
    descriptors: Vec<Descriptor>,
    wrap_x: bool,
    wrap_y: bool,
}

/// Plane flag: wraps horizontally.
pub const PLANE_WRAP_X: u32 = 0x4;
/// Plane flag: wraps vertically.
pub const PLANE_WRAP_Y: u32 = 0x8;

/// Problems found while building a [`TileMap`] from level data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TileReport {
    /// Cells that name a descriptor the level does not have (read as clear).
    pub missing_descriptors: usize,
    /// Negative cells other than the empty and filled markers (read as
    /// clear).
    pub bad_cells: usize,
}

impl TileMap {
    /// An empty (all clear) grid.
    pub fn new(w_tiles: i32, h_tiles: i32, tile_w: i32, tile_h: i32) -> TileMap {
        let w_tiles = w_tiles.max(1);
        let h_tiles = h_tiles.max(1);
        TileMap {
            w_tiles,
            h_tiles,
            tile_w: tile_w.max(1),
            tile_h: tile_h.max(1),
            cells: vec![TILE_EMPTY; (w_tiles as usize) * (h_tiles as usize)],
            descriptors: Vec::new(),
            wrap_x: false,
            wrap_y: false,
        }
    }

    /// Builds the grid of a level plane with the level's descriptors.
    pub fn from_plane(plane: &Plane, descriptors: &[TileDescriptor]) -> (TileMap, TileReport) {
        let mut map = TileMap::new(plane.w_tiles, plane.h_tiles, plane.tile_w, plane.tile_h);
        map.wrap_x = plane.flags & PLANE_WRAP_X != 0;
        map.wrap_y = plane.flags & PLANE_WRAP_Y != 0;
        map.descriptors = descriptors.iter().map(Descriptor::from_level).collect();
        let mut report = TileReport::default();
        for (i, cell) in map.cells.iter_mut().enumerate() {
            let v = plane.tiles.get(i).copied().unwrap_or(TILE_EMPTY);
            *cell = if v == TILE_EMPTY || v == TILE_FILLED {
                TILE_EMPTY
            } else if v < 0 {
                report.bad_cells += 1;
                TILE_EMPTY
            } else if (v & 0xFFFF) as usize >= map.descriptors.len() {
                report.missing_descriptors += 1;
                TILE_EMPTY
            } else {
                v & 0xFFFF
            };
        }
        (map, report)
    }

    /// Builds a grid from text rows, one character per tile: `.` clear,
    /// `#` solid, `=` ground, `H` climb, `x` death, `-` a tile whose top
    /// eight rows are ground (a thin one-way platform).
    pub fn from_ascii(tile: i32, rows: &[&str]) -> TileMap {
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.len()).max().unwrap_or(1) as i32;
        let mut map = TileMap::new(w, h, tile, tile);
        let solid = map.add_descriptor(Descriptor::single(tile, SOLID));
        let ground = map.add_descriptor(Descriptor::single(tile, GROUND));
        let climb = map.add_descriptor(Descriptor::single(tile, CLIMB));
        let death = map.add_descriptor(Descriptor::single(tile, DEATH));
        let thin = map.add_descriptor(Descriptor::Double {
            width: tile,
            height: tile,
            outside: CLEAR,
            inside: GROUND,
            rect: [0, 0, tile - 1, 7],
        });
        for (r, row) in rows.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                let d = match ch {
                    '#' => solid,
                    '=' => ground,
                    'H' => climb,
                    'x' => death,
                    '-' => thin,
                    _ => continue,
                };
                map.set_cell(c as i32, r as i32, d);
            }
        }
        map
    }

    /// Adds a descriptor and returns its tile number.
    pub fn add_descriptor(&mut self, desc: Descriptor) -> i32 {
        self.descriptors.push(desc);
        (self.descriptors.len() - 1) as i32
    }

    /// Sets a cell's tile number (-1 for empty).
    pub fn set_cell(&mut self, col: i32, row: i32, tile: i32) {
        if col >= 0 && row >= 0 && col < self.w_tiles && row < self.h_tiles {
            let i = (row * self.w_tiles + col) as usize;
            self.cells[i] = tile;
        }
    }

    /// A cell's tile number: -1 for an empty cell and for a cell outside
    /// the plane.
    pub fn cell(&self, col: i32, row: i32) -> i32 {
        if col >= 0 && row >= 0 && col < self.w_tiles && row < self.h_tiles {
            self.cells[(row * self.w_tiles + col) as usize]
        } else {
            -1
        }
    }

    /// Fills a rectangle of cells (inclusive tile coordinates).
    pub fn fill_cells(&mut self, cols: (i32, i32), rows: (i32, i32), tile: i32) {
        for r in rows.0..=rows.1 {
            for c in cols.0..=cols.1 {
                self.set_cell(c, r, tile);
            }
        }
    }

    /// Plane width in pixels.
    pub fn width(&self) -> i32 {
        self.w_tiles.saturating_mul(self.tile_w)
    }

    /// Plane height in pixels.
    pub fn height(&self) -> i32 {
        self.h_tiles.saturating_mul(self.tile_h)
    }

    /// Tile width in pixels.
    pub fn tile_w(&self) -> i32 {
        self.tile_w
    }

    /// Tile height in pixels.
    pub fn tile_h(&self) -> i32 {
        self.tile_h
    }

    /// True when the plane wraps horizontally.
    pub fn wrap_x(&self) -> bool {
        self.wrap_x
    }

    /// True when the plane wraps vertically.
    pub fn wrap_y(&self) -> bool {
        self.wrap_y
    }

    fn cell_at(&self, px: i32, py: i32) -> (i32, i32, i32, i32) {
        let px = px.clamp(0, self.width() - 1);
        let py = py.clamp(0, self.height() - 1);
        let col = px / self.tile_w;
        let row = py / self.tile_h;
        (col, row, px - col * self.tile_w, py - row * self.tile_h)
    }

    /// Attribute at level pixel (px, py), clamped to the plane.
    pub fn attr(&self, px: i32, py: i32) -> u8 {
        let (col, row, lx, ly) = self.cell_at(px, py);
        let cell = self.cells[(row * self.w_tiles + col) as usize];
        if cell < 0 {
            return CLEAR;
        }
        match self.descriptors.get(cell as usize) {
            Some(d) => d.attr_at(lx, ly),
            None => CLEAR,
        }
    }

    /// The pixel columns of the ladder tile under (px, py): the tile's first
    /// column and that plus the descriptor width minus one.
    pub fn ladder_span(&self, px: i32, py: i32) -> Option<(i32, i32)> {
        let (col, row, _, _) = self.cell_at(px, py);
        let cell = self.cells[(row * self.w_tiles + col) as usize];
        if cell < 0 {
            return None;
        }
        let desc = self.descriptors.get(cell as usize)?;
        let left = col * self.tile_w;
        Some((left, left + desc.width() - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_reads_back_what_was_written() {
        let mut map = TileMap::from_ascii(64, &["..", "#="]);
        let solid = map.cell(0, 1);
        assert!(solid >= 0);
        assert_eq!(map.cell(0, 0), -1, "empty");
        assert_eq!(map.cell(2, 0), -1, "outside");
        assert_eq!(map.cell(-1, 1), -1, "outside");
        map.set_cell(1, 0, solid);
        assert_eq!(map.cell(1, 0), solid);
        assert_eq!(map.attr(64 + 10, 10), SOLID);
        map.set_cell(1, 0, -1);
        assert_eq!(map.cell(1, 0), -1);
        assert_eq!(map.attr(64 + 10, 10), CLEAR);
    }

    #[test]
    fn lookups_clamp_to_the_edges() {
        let map = TileMap::from_ascii(64, &["#..", "...", "..="]);
        assert_eq!(map.attr(0, 0), SOLID);
        assert_eq!(map.attr(-500, -500), SOLID);
        assert_eq!(map.attr(10_000, 10_000), GROUND);
        assert_eq!(map.attr(64, 0), CLEAR);
        assert_eq!(map.attr(191, 191), GROUND);
    }

    #[test]
    fn double_rectangles_are_inclusive() {
        let map = TileMap::from_ascii(64, &["-"]);
        assert_eq!(map.attr(0, 7), GROUND);
        assert_eq!(map.attr(63, 0), GROUND);
        assert_eq!(map.attr(0, 8), CLEAR);
    }

    #[test]
    fn masks_and_unknown_values_read_as_clear_out_of_range() {
        let mut map = TileMap::new(1, 1, 4, 4);
        let mut mask = vec![SOLID; 8];
        mask[1] = 9;
        let d = map.add_descriptor(Descriptor::from_level(&TileDescriptor::Mask {
            width: 4,
            height: 2,
            mask: mask.clone(),
        }));
        map.set_cell(0, 0, d);
        assert_eq!(map.attr(0, 0), SOLID);
        assert_eq!(map.attr(1, 0), CLEAR);
        // Row 3 is outside a two-row mask.
        assert_eq!(map.attr(0, 3), CLEAR);
    }

    #[test]
    fn bad_cells_are_reported_and_clear() {
        let plane = Plane {
            w_tiles: 3,
            h_tiles: 1,
            tile_w: 64,
            tile_h: 64,
            tiles: vec![0, 7, -5],
            ..Plane::default()
        };
        let descs = vec![TileDescriptor::Single {
            width: 64,
            height: 64,
            attribute: 1,
        }];
        let (map, report) = TileMap::from_plane(&plane, &descs);
        assert_eq!(report.missing_descriptors, 1);
        assert_eq!(report.bad_cells, 1);
        assert_eq!(map.attr(10, 10), SOLID);
        assert_eq!(map.attr(70, 10), CLEAR);
        assert_eq!(map.attr(140, 10), CLEAR);
        assert_eq!(map.ladder_span(70, 10), None);
        assert_eq!(map.ladder_span(10, 10), Some((0, 63)));
    }
}
