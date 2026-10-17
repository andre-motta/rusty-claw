//! Which part of each plane is visible for a camera position.
//!
//! The camera names a point on the main plane. The main plane centres its
//! view there; every other plane takes the main plane's whole-pixel centre
//! times its own scroll factor (parallax). Each plane then wraps or clamps
//! its position, truncates it to whole pixels and places a view rectangle of
//! the view size around it:
//!
//! - on an axis that wraps, the position is brought into the plane and the
//!   rectangle may run past the plane's far edge (the tile walk wraps);
//! - on an axis that does not wrap, the position is clamped into the plane
//!   and the rectangle is pushed back inside the plane, which keeps the
//!   centre where it was;
//! - a non-wrapping plane smaller than the view ends with a negative left or
//!   top edge, and the cells outside it are empty.
//!
//! Positions are kept as 32-bit floats, and every product of a whole-pixel
//! centre and a scroll factor is rounded to 32 bits, as the game does, so a
//! view matches the game's to the pixel. A scroll factor is the plane's
//! integer percentage times the 32-bit value nearest to 0.01, which for 30 %
//! is one step below the 32-bit value nearest to 0.3.

use claw_assets::Plane;

/// Plane flag: the main (action) plane.
pub const PLANE_MAIN: u32 = 0x1;
/// Plane flag: the plane is not drawn.
pub const PLANE_NO_DRAW: u32 = 0x2;
/// Plane flag: the plane repeats horizontally.
pub const PLANE_X_WRAP: u32 = 0x4;
/// Plane flag: the plane repeats vertically.
pub const PLANE_Y_WRAP: u32 = 0x8;

/// The level's front plane: the plane the front layer switch hides, the
/// last plane of the level unless that is the main plane (a level of three
/// planes, background, action and front, has it at index 2). `None` when
/// the level has no such plane.
pub fn front_plane(planes: &[Plane]) -> Option<usize> {
    let last = planes.len().checked_sub(1)?;
    (last > 0 && planes[last].flags & PLANE_MAIN == 0).then_some(last)
}

/// Cell value of an empty cell.
pub const CELL_EMPTY: i32 = -1;
/// Cell value of a cell filled with the plane's fill colour.
pub const CELL_FILL: i32 = 0xEEEE_EEEE_u32 as i32;

/// The 32-bit float nearest to 0.01.
const PERCENT: f32 = 0.01;

/// A plane's scroll factor from its integer percentage.
pub fn scroll_factor(percent: i32) -> f32 {
    (f64::from(percent) * f64::from(PERCENT)) as f32
}

fn half(n: i32) -> i32 {
    // Rounds toward zero, as a signed halving does.
    n / 2
}

/// A rectangle in plane pixels, inclusive on all sides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    /// Left column.
    pub left: i32,
    /// Top row.
    pub top: i32,
    /// Right column (inclusive).
    pub right: i32,
    /// Bottom row (inclusive).
    pub bottom: i32,
}

/// The view state of one plane.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaneView {
    /// Plane flags.
    pub flags: u32,
    /// Tile width in pixels.
    pub tile_w: i32,
    /// Tile height in pixels.
    pub tile_h: i32,
    /// Width in tiles.
    pub w_tiles: i32,
    /// Height in tiles.
    pub h_tiles: i32,
    /// Horizontal scroll factor.
    pub factor_x: f32,
    /// Vertical scroll factor.
    pub factor_y: f32,
    /// View width.
    pub view_w: i32,
    /// View height.
    pub view_h: i32,
    /// Position (requested view centre) after wrapping or clamping.
    pub pos_x: f32,
    /// Position after wrapping or clamping.
    pub pos_y: f32,
    /// Whole-pixel centre.
    pub center_x: i32,
    /// Whole-pixel centre.
    pub center_y: i32,
    /// The visible rectangle.
    pub rect: Rect,
}

impl PlaneView {
    /// The view of `plane` before any camera is set, with a 640x480 view.
    pub fn new(plane: &Plane) -> PlaneView {
        let mut view = PlaneView {
            flags: plane.flags,
            tile_w: plane.tile_w.max(1),
            tile_h: plane.tile_h.max(1),
            w_tiles: plane.w_tiles.max(0),
            h_tiles: plane.h_tiles.max(0),
            factor_x: scroll_factor(plane.move_x_pct),
            factor_y: scroll_factor(plane.move_y_pct),
            view_w: 640,
            view_h: 480,
            pos_x: 0.0,
            pos_y: 0.0,
            center_x: 0,
            center_y: 0,
            rect: Rect::default(),
        };
        view.set_position(plane.origin_x, plane.origin_y);
        view
    }

    /// True for the main plane.
    pub fn is_main(&self) -> bool {
        self.flags & PLANE_MAIN != 0
    }

    /// True when the plane is drawn.
    pub fn is_drawn(&self) -> bool {
        self.flags & PLANE_NO_DRAW == 0
    }

    /// True when the plane repeats horizontally.
    pub fn wraps_x(&self) -> bool {
        self.flags & PLANE_X_WRAP != 0
    }

    /// True when the plane repeats vertically.
    pub fn wraps_y(&self) -> bool {
        self.flags & PLANE_Y_WRAP != 0
    }

    /// Plane width in pixels.
    pub fn width_px(&self) -> i32 {
        self.tile_w.saturating_mul(self.w_tiles)
    }

    /// Plane height in pixels.
    pub fn height_px(&self) -> i32 {
        self.tile_h.saturating_mul(self.h_tiles)
    }

    /// Sets the view size and recomputes the view.
    pub fn set_view_size(&mut self, width: i32, height: i32) {
        self.view_w = width.max(1);
        self.view_h = height.max(1);
        self.update_view();
    }

    /// Requests a camera position: as is on the main plane, else times the
    /// scroll factors.
    pub fn set_position(&mut self, x: i32, y: i32) {
        if self.is_main() {
            self.pos_x = x as f32;
            self.pos_y = y as f32;
        } else {
            self.pos_x = (f64::from(x) * f64::from(self.factor_x)) as f32;
            self.pos_y = (f64::from(y) * f64::from(self.factor_y)) as f32;
        }
        self.update_view();
    }

    fn axis(pos: f32, size: i32, wrap: bool) -> f32 {
        let size_f = f64::from(size);
        if wrap {
            if size <= 0 {
                return pos;
            }
            let mut p = pos;
            if p < 0.0 {
                p = f64::from(p).rem_euclid(size_f) as f32;
            }
            if f64::from(p) >= size_f {
                p = f64::from(p).rem_euclid(size_f) as f32;
            }
            return p;
        }
        if pos < 0.0 {
            0.0
        } else if size_f <= f64::from(pos) {
            (size - 1).max(0) as f32
        } else {
            pos
        }
    }

    /// Wraps or clamps the position, truncates it and places the rectangle.
    pub fn update_view(&mut self) {
        let (w, h) = (self.width_px(), self.height_px());
        let (xwrap, ywrap) = (self.wraps_x(), self.wraps_y());
        self.pos_x = Self::axis(self.pos_x, w, xwrap);
        self.pos_y = Self::axis(self.pos_y, h, ywrap);
        // Positions are not negative here, so truncation is the floor.
        self.center_x = self.pos_x as i32;
        self.center_y = self.pos_y as i32;
        let mut left = self.center_x - half(self.view_w);
        if left < 0 {
            left = if xwrap { left + w } else { 0 };
        }
        let mut top = self.center_y - half(self.view_h);
        if top < 0 {
            top = if ywrap { top + h } else { 0 };
        }
        let mut right = self.view_w + left - 1;
        let mut bottom = self.view_h + top - 1;
        if right >= w && !xwrap {
            let d = right - w + 1;
            right -= d;
            left -= d;
        }
        if bottom >= h && !ywrap {
            let d = bottom - h + 1;
            bottom -= d;
            top -= d;
        }
        self.rect = Rect {
            left,
            top,
            right,
            bottom,
        };
    }

    /// First tile column and row and how many columns and rows the tile
    /// walk visits for `rect`.
    pub fn cell_range(&self, rect: Rect) -> (i32, i32, i32, i32) {
        let c0 = rect.left.div_euclid(self.tile_w);
        let c1 = rect.right.div_euclid(self.tile_w);
        let r0 = rect.top.div_euclid(self.tile_h);
        let r1 = rect.bottom.div_euclid(self.tile_h);
        (c0, r0, c1 - c0 + 1, r1 - r0 + 1)
    }

    fn indices(first: i32, count: i32, size: i32) -> Vec<i32> {
        let mut out = Vec::with_capacity(count.max(0) as usize);
        let mut i = first;
        for n in 0..count.max(0) {
            if n > 0 {
                i += 1;
                if i >= size {
                    i = 0;
                }
            }
            out.push(i);
        }
        out
    }

    /// The cells the tile walk visits for `rect`, row by row, with
    /// [`CELL_EMPTY`] for any index outside the grid. `tiles` is the plane's
    /// row-major cell grid. A plane that is not drawn has no cells.
    pub fn cells_for(&self, rect: Rect, tiles: &[i32]) -> Vec<Vec<i32>> {
        if !self.is_drawn() {
            return Vec::new();
        }
        let (c0, r0, cols, rows) = self.cell_range(rect);
        let cols = Self::indices(c0, cols, self.w_tiles);
        let rows = Self::indices(r0, rows, self.h_tiles);
        rows.iter()
            .map(|&r| cols.iter().map(|&c| self.cell(tiles, c, r)).collect())
            .collect()
    }

    /// The cells of the current view; see [`PlaneView::cells_for`].
    pub fn visible_cells(&self, tiles: &[i32]) -> Vec<Vec<i32>> {
        self.cells_for(self.rect, tiles)
    }

    /// The cell value at a column and row, or [`CELL_EMPTY`] outside the grid.
    pub fn cell(&self, tiles: &[i32], col: i32, row: i32) -> i32 {
        if col < 0 || row < 0 || col >= self.w_tiles || row >= self.h_tiles {
            return CELL_EMPTY;
        }
        let idx = row as usize * self.w_tiles as usize + col as usize;
        tiles.get(idx).copied().unwrap_or(CELL_EMPTY)
    }
}

/// The views of every plane of a level.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldView {
    /// Plane views in level order.
    pub planes: Vec<PlaneView>,
    /// Index of the main plane: the last plane flagged main.
    pub main: Option<usize>,
}

impl WorldView {
    /// Views for `planes`, with a 640x480 view.
    pub fn new(planes: &[Plane]) -> WorldView {
        let planes: Vec<PlaneView> = planes.iter().map(PlaneView::new).collect();
        let main = planes.iter().rposition(PlaneView::is_main);
        WorldView { planes, main }
    }

    /// Sets the view size of every plane.
    pub fn set_view_size(&mut self, width: i32, height: i32) {
        for p in &mut self.planes {
            p.set_view_size(width, height);
        }
    }

    /// Points the camera at `(x, y)` on the main plane; every other plane
    /// follows the main plane's whole-pixel centre. Without a main plane,
    /// every plane takes the camera point times its factors.
    pub fn set_camera(&mut self, x: i32, y: i32) {
        let (cx, cy) = match self.main {
            Some(m) => {
                let main = &mut self.planes[m];
                main.set_position(x, y);
                (main.center_x, main.center_y)
            }
            None => (x, y),
        };
        for (i, p) in self.planes.iter_mut().enumerate() {
            if Some(i) != self.main {
                p.set_position(cx, cy);
            }
        }
    }

    /// The main plane's view, if there is a main plane.
    pub fn main_view(&self) -> Option<&PlaneView> {
        self.main.map(|m| &self.planes[m])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(flags: u32, tiles: (i32, i32), pct: (i32, i32), z: i32) -> Plane {
        Plane {
            name: "P".into(),
            flags,
            tile_w: 64,
            tile_h: 64,
            w_tiles: tiles.0,
            h_tiles: tiles.1,
            move_x_pct: pct.0,
            move_y_pct: pct.1,
            z,
            ..Plane::default()
        }
    }

    fn worked_world() -> WorldView {
        let mut w = WorldView::new(&[
            plane(PLANE_X_WRAP | PLANE_Y_WRAP, (16, 16), (50, 50), -999),
            plane(PLANE_MAIN, (100, 40), (100, 100), 0),
            plane(PLANE_X_WRAP, (16, 60), (150, 125), 9000),
        ]);
        w.set_view_size(640, 480);
        w
    }

    fn rect(p: &PlaneView) -> (i32, i32, i32, i32) {
        (p.rect.left, p.rect.top, p.rect.right, p.rect.bottom)
    }

    #[test]
    fn worked_example_middle() {
        let mut w = worked_world();
        w.set_camera(1000, 1000);
        assert_eq!(rect(&w.planes[0]), (180, 260, 819, 739));
        assert_eq!(rect(&w.planes[1]), (680, 760, 1319, 1239));
        assert_eq!(rect(&w.planes[2]), (156, 1010, 795, 1489));
        assert_eq!(w.planes[2].pos_x, 476.0);
        let (c0, r0, cols, rows) = w.planes[0].cell_range(w.planes[0].rect);
        assert_eq!((c0, r0, cols, rows), (2, 4, 11, 8));
    }

    #[test]
    fn worked_example_top_left() {
        let mut w = worked_world();
        w.set_camera(100, 100);
        assert_eq!(rect(&w.planes[0]), (754, 834, 1393, 1313));
        assert_eq!(rect(&w.planes[1]), (0, 0, 639, 479));
        assert_eq!(rect(&w.planes[2]), (854, 0, 1493, 479));
        assert_eq!((w.planes[1].center_x, w.planes[1].center_y), (100, 100));
    }

    #[test]
    fn worked_example_bottom_right() {
        let mut w = worked_world();
        w.set_camera(6399, 2560);
        assert_eq!((w.planes[0].pos_x, w.planes[0].pos_y), (127.5, 255.5));
        assert_eq!((w.planes[2].pos_x, w.planes[2].pos_y), (382.5, 3198.75));
        assert_eq!(rect(&w.planes[0]), (831, 15, 1470, 494));
        assert_eq!(rect(&w.planes[1]), (5760, 2080, 6399, 2559));
        assert_eq!(rect(&w.planes[2]), (62, 2958, 701, 3437));
    }

    #[test]
    fn walk_wraps_column_indices_and_empties_outside() {
        let p = {
            let mut p = PlaneView::new(&plane(PLANE_X_WRAP, (4, 2), (100, 100), 0));
            p.set_view_size(320, 128);
            p
        };
        let tiles: Vec<i32> = (0..8).collect();
        let r = Rect {
            left: 200,
            top: -10,
            right: 519,
            bottom: 117,
        };
        let cells = p.cells_for(r, &tiles);
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], vec![CELL_EMPTY; 6]);
        assert_eq!(cells[1], vec![3, 0, 1, 2, 3, 0]);
        assert_eq!(cells[2], vec![7, 4, 5, 6, 7, 4]);
    }

    #[test]
    fn small_non_wrapping_plane_has_negative_left() {
        let mut p = PlaneView::new(&plane(0, (5, 5), (100, 100), 0));
        p.set_view_size(640, 480);
        p.set_position(100, 100);
        assert_eq!(p.rect.left, 320 - 640);
        assert_eq!(p.rect.right, 319);
    }

    #[test]
    fn scroll_factor_is_single_precision() {
        assert_eq!(scroll_factor(100), 1.0);
        assert!(scroll_factor(30) < 0.3_f32);
        assert_eq!(scroll_factor(50), 0.5);
    }
}
