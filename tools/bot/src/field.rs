//! The coarse tile graph the search is guided by, and distance fields over
//! it.
//!
//! The level's collision tiles are cut into square cells of [`CELL`]
//! pixels. A cell is open when Claw's feet can be in it (it is not mostly
//! solid and holds no death tile), and it gives support when Claw can stand
//! there: a solid or ground top under it, a ladder, or the track of one of
//! the level's elevators or pegs. Moves between neighbouring open cells
//! follow what Claw can do, generously:
//!
//! - down: always (falling);
//! - up: only within a jump's reach above support in the same column, or
//!   on a ladder;
//! - sideways: only within a jump's reach above support somewhere within
//!   a jump's width, so a gap can be crossed but open sky cannot.
//!
//! A [`DistanceField`] holds the cost of the cheapest path from every cell
//! to a target, in pixels. The graph never claims less than the level
//! allows by tiles, so the distance is an optimistic guide: the search
//! still plays every move in the simulation.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use claw_sim::SimLevel;
use claw_sim::logic::platform;
use claw_sim::tiles::{CLIMB, DEATH, GROUND, SOLID};

/// Size of a graph cell, px.
pub const CELL: i32 = 16;

/// Distance of a cell with no path to the target.
pub const UNREACHABLE: u32 = u32::MAX;

/// How far Claw reaches, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reach {
    /// Height above support Claw can reach with a jump.
    pub jump_height: i32,
    /// Distance to either side of support Claw can cover in the air.
    pub jump_width: i32,
}

impl Default for Reach {
    fn default() -> Reach {
        Reach {
            jump_height: 160,
            jump_width: 192,
        }
    }
}

/// Record fields of a platform's track: x min, y min, x max, y max, then
/// the track width and height (around the placed position) at 61 and 62.
const TRACK: [usize; 4] = [51, 52, 53, 54];
const TRACK_WIDTH: usize = 61;
const TRACK_HEIGHT: usize = 62;

/// The coarse graph of a level.
#[derive(Debug, Clone)]
pub struct Grid {
    cols: i32,
    rows: i32,
    open: Vec<bool>,
    climb: Vec<bool>,
    /// Cells down to support (0 on support), saturating; `u16::MAX` with
    /// no support below before a closed cell.
    air: Vec<u16>,
    /// The least `air` within a jump's width to either side.
    env: Vec<u16>,
    reach_cells: u16,
}

impl Grid {
    /// Builds the graph of `level` with `reach`.
    pub fn build(level: &SimLevel, reach: Reach) -> Grid {
        let map = &level.map;
        let cols = (map.width() + CELL - 1) / CELL;
        let rows = (map.height() + CELL - 1) / CELL;
        let n = (cols * rows) as usize;
        let mut open = vec![false; n];
        let mut climb = vec![false; n];
        let mut support = vec![false; n];
        const SAMPLES: [i32; 4] = [2, 6, 10, 14];
        for r in 0..rows {
            for c in 0..cols {
                let (x0, y0) = (c * CELL, r * CELL);
                let mut solid = 0;
                let mut death = false;
                let mut ladder = false;
                let mut ground = false;
                for dy in SAMPLES {
                    for dx in SAMPLES {
                        match map.attr(x0 + dx, y0 + dy) {
                            SOLID => solid += 1,
                            DEATH => death = true,
                            CLIMB => ladder = true,
                            GROUND => ground = true,
                            _ => {}
                        }
                    }
                }
                let i = (r * cols + c) as usize;
                open[i] = solid < 8 && !death;
                climb[i] = ladder && open[i];
                // Feet in this cell rest on a top at most a cell below its
                // bottom (a floor top may lie inside the next cell).
                let below = SAMPLES.iter().any(|&dx| {
                    [CELL - 2, CELL + 2, CELL + 6, CELL + 10]
                        .iter()
                        .any(|&dy| matches!(map.attr(x0 + dx, y0 + dy), SOLID | GROUND))
                });
                support[i] = open[i] && (ladder || ground || below);
            }
        }
        let mut grid = Grid {
            cols,
            rows,
            open,
            climb,
            air: Vec::new(),
            env: Vec::new(),
            reach_cells: (reach.jump_height / CELL).max(1) as u16,
        };
        grid.mark_platforms(level, &mut support);
        grid.air = vec![u16::MAX; n];
        for c in 0..cols {
            let mut below = u16::MAX;
            for r in (0..rows).rev() {
                let i = grid.index(c, r);
                let a = if !grid.open[i] {
                    u16::MAX
                } else if support[i] {
                    0
                } else {
                    below.saturating_add(1)
                };
                grid.air[i] = a;
                below = a;
            }
        }
        let width = (reach.jump_width / CELL).max(0);
        grid.env = vec![u16::MAX; n];
        for r in 0..rows {
            for c in 0..cols {
                let mut best = u16::MAX;
                for dc in -width..=width {
                    let cc = c + dc;
                    if cc >= 0 && cc < cols {
                        best = best.min(grid.air[grid.index(cc, r)]);
                    }
                }
                let i = grid.index(c, r);
                grid.env[i] = best;
            }
        }
        grid
    }

    /// Marks the surface along every elevator's and peg's track as support.
    fn mark_platforms(&self, level: &SimLevel, support: &mut [bool]) {
        for spec in &level.objects {
            if !platform::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let r = &spec.record;
            let (x, y) = (r[5], r[6]);
            let pick = |v: i32, pos: i32| if v == 0 { pos } else { v };
            let (mut x_min, mut x_max) = (pick(r[TRACK[0]], x), pick(r[TRACK[2]], x));
            let (mut y_min, mut y_max) = (pick(r[TRACK[1]], y), pick(r[TRACK[3]], y));
            if r[TRACK_WIDTH] > 0 {
                x_min = x - r[TRACK_WIDTH] / 2;
                x_max = x + r[TRACK_WIDTH] / 2;
            }
            if r[TRACK_HEIGHT] > 0 {
                y_min = y - r[TRACK_HEIGHT] / 2;
                y_max = y + r[TRACK_HEIGHT] / 2;
            }
            let (x_min, x_max) = (x_min.min(x_max), x_min.max(x_max));
            let (y_min, y_max) = (y_min.min(y_max), y_min.max(y_max));
            // The surface is near the top of the platform's picture; take
            // a band of cells around it, the width of a peg either side.
            let mut yy = y_min - 3 * CELL;
            while yy <= y_max {
                let mut xx = x_min - 2 * CELL;
                while xx <= x_max + 2 * CELL {
                    if let Some(i) = self.cell_index(xx, yy)
                        && self.open[i]
                    {
                        support[i] = true;
                    }
                    xx += CELL;
                }
                yy += CELL;
            }
        }
    }

    fn index(&self, c: i32, r: i32) -> usize {
        (r * self.cols + c) as usize
    }

    /// The index of the cell holding pixel (x, y), if it is in the level.
    pub fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        let (c, r) = (x.div_euclid(CELL), y.div_euclid(CELL));
        (c >= 0 && r >= 0 && c < self.cols && r < self.rows).then(|| self.index(c, r))
    }

    /// True when the cell holding pixel (x, y) is open.
    pub fn is_open(&self, x: i32, y: i32) -> bool {
        self.cell_index(x, y).is_some_and(|i| self.open[i])
    }

    /// The distance field to the open cells within `radius` px of
    /// (`x`, `y`).
    pub fn field_to(&self, x: i32, y: i32, radius: i32) -> DistanceField {
        let r2 = i64::from(radius) * i64::from(radius);
        self.field_where(x - radius, y - radius, x + radius, y + radius, |cx, cy| {
            let (dx, dy) = (i64::from(cx - x), i64::from(cy - y));
            dx * dx + dy * dy <= r2
        })
    }

    /// The distance field to the open cells whose centres lie in the box
    /// from (`left`, `top`) to (`right`, `bottom`).
    pub fn field_to_box(&self, left: i32, top: i32, right: i32, bottom: i32) -> DistanceField {
        self.field_where(left, top, right, bottom, |_, _| true)
    }

    /// The distance field to the open cells around the box whose centres
    /// pass `inside`. With none open there, the nearest open cell below
    /// the box's middle is the target.
    fn field_where(
        &self,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        inside: impl Fn(i32, i32) -> bool,
    ) -> DistanceField {
        let mut targets = Vec::new();
        for r in top.div_euclid(CELL)..=bottom.div_euclid(CELL) {
            for c in left.div_euclid(CELL)..=right.div_euclid(CELL) {
                if c < 0 || r < 0 || c >= self.cols || r >= self.rows {
                    continue;
                }
                let (cx, cy) = (c * CELL + CELL / 2, r * CELL + CELL / 2);
                let i = self.index(c, r);
                if (left..=right).contains(&cx)
                    && (top..=bottom).contains(&cy)
                    && inside(cx, cy)
                    && self.open[i]
                {
                    targets.push(i);
                }
            }
        }
        if targets.is_empty() {
            let x = (left + right) / 2;
            let mut yy = (top + bottom) / 2;
            while let Some(i) = self.cell_index(x, yy) {
                if self.open[i] {
                    targets.push(i);
                    break;
                }
                yy += CELL;
            }
        }
        self.dijkstra(&targets)
    }

    /// Cheapest path cost to `targets` from every cell, walking the moves
    /// backwards from the targets.
    fn dijkstra(&self, targets: &[usize]) -> DistanceField {
        let n = self.open.len();
        let mut dist = vec![UNREACHABLE; n];
        let mut heap = BinaryHeap::new();
        for &t in targets {
            dist[t] = 0;
            heap.push(Reverse((0u32, t)));
        }
        let step = CELL as u32;
        let up = (CELL + CELL / 2) as u32;
        while let Some(Reverse((d, b))) = heap.pop() {
            if d > dist[b] {
                continue;
            }
            let (c, r) = ((b as i32) % self.cols, (b as i32) / self.cols);
            // Moves that end in b: sideways from a neighbour, up from the
            // cell below, down from the cell above.
            let sideways = self.env[b] <= self.reach_cells;
            let below = (r + 1 < self.rows).then(|| self.index(c, r + 1));
            let rising = self.air[b] <= self.reach_cells
                || self.climb[b]
                || below.is_some_and(|i| self.climb[i]);
            let mut relax = |a: usize, cost: u32, heap: &mut BinaryHeap<Reverse<(u32, usize)>>| {
                if !self.open[a] {
                    return;
                }
                let nd = d + cost;
                if nd < dist[a] {
                    dist[a] = nd;
                    heap.push(Reverse((nd, a)));
                }
            };
            if sideways {
                if c > 0 {
                    relax(self.index(c - 1, r), step, &mut heap);
                }
                if c + 1 < self.cols {
                    relax(self.index(c + 1, r), step, &mut heap);
                }
            }
            if rising && let Some(i) = below {
                relax(i, up, &mut heap);
            }
            if r > 0 {
                relax(self.index(c, r - 1), step, &mut heap);
            }
        }
        DistanceField {
            cols: self.cols,
            rows: self.rows,
            dist,
        }
    }
}

/// Path cost from every cell of a [`Grid`] to one target, px.
#[derive(Debug, Clone)]
pub struct DistanceField {
    cols: i32,
    rows: i32,
    dist: Vec<u32>,
}

impl DistanceField {
    /// The distance from pixel (x, y): the least of its cell and the cells
    /// up to three above it (feet sunk into a floor read the cell above),
    /// plus the climb to them. [`UNREACHABLE`] when none has a path.
    pub fn at(&self, x: i32, y: i32) -> u32 {
        let c = x.div_euclid(CELL);
        if c < 0 || c >= self.cols {
            return UNREACHABLE;
        }
        let r0 = y.div_euclid(CELL);
        let mut best = UNREACHABLE;
        for k in 0..4 {
            let r = r0 - k;
            if r < 0 || r >= self.rows {
                continue;
            }
            let d = self.dist[(r * self.cols + c) as usize];
            if d != UNREACHABLE {
                best = best.min(d + (k * CELL) as u32);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_sim::TileMap;

    fn level(rows: &[&str]) -> SimLevel {
        SimLevel::synthetic(TileMap::from_ascii(64, rows), (100, 100))
    }

    #[test]
    fn a_flat_floor_is_walked_and_the_sky_is_not() {
        let l = level(&["..........", "..........", "..........", "##########"]);
        let g = Grid::build(&l, Reach::default());
        let f = g.field_to(600, 180, 20);
        let near = f.at(500, 180);
        let far = f.at(100, 180);
        assert!(near < far, "{near} {far}");
        assert_ne!(far, UNREACHABLE);
        // High above the floor the only way is down, which costs more than
        // the walk along it.
        assert!(f.at(100, 20) > far);
    }

    #[test]
    fn a_wall_too_high_to_jump_cuts_the_level_and_a_ladder_joins_it() {
        let rows = [
            ".....#....",
            ".....#....",
            ".....#....",
            ".....#....",
            "##########",
        ];
        let g = Grid::build(&level(&rows), Reach::default());
        let f = g.field_to(600, 240, 20);
        assert_eq!(f.at(100, 240), UNREACHABLE);
        let rows = [
            "....H.....",
            "....H#....",
            "....H#....",
            "....H#....",
            "##########",
        ];
        let g = Grid::build(&level(&rows), Reach::default());
        let f = g.field_to(600, 240, 20);
        assert_ne!(f.at(100, 240), UNREACHABLE);
    }

    #[test]
    fn a_narrow_gap_is_crossed_and_death_tiles_are_closed() {
        let rows = ["..........", "..........", "###xx#####"];
        let g = Grid::build(&level(&rows), Reach::default());
        assert!(!g.is_open(3 * 64 + 10, 2 * 64 + 10));
        let f = g.field_to(600, 120, 20);
        assert_ne!(f.at(60, 120), UNREACHABLE);
    }
}
