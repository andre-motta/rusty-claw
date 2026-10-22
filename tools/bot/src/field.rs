//! The coarse tile graph the search is guided by, and distance fields over
//! it.
//!
//! The level's collision tiles are cut into square cells of [`CELL`]
//! pixels. A cell is open when Claw's feet can be in it (it is not mostly
//! solid, holds no death tile, and the [`HEADROOM`] cells above it are not
//! mostly solid either, so Claw fits), and it gives
//! support when Claw can stand
//! there: a solid or ground top under it, a ladder, the track of one of
//! the level's elevators or pegs, the route of a path elevator, the swing
//! of a rope (Claw hangs from its end and lets go with a jump from
//! anywhere along it), the top of a springboard or the top of a stepping
//! stone (which is there only part of the time, so the search still times
//! the hop). Above every springboard and every ground blower is its throw:
//! the cells up to the throw's height over its top (a blower's floor), and
//! a jump's width to either side, where Claw rises and steers after it
//! throws him; each cell of a throw remembers the springboard or blower
//! that throws him there ([`Grid::throw_base`]). Moves between neighbouring
//! open cells follow what Claw can do, generously:
//!
//! - down: always (falling), except through a one-way floor (a ground
//!   tile, which Claw stands on and cannot drop through) that is not a
//!   breaking plank's (a plank gives way under him a moment after he
//!   steps on it, so he stands on it and then falls through);
//! - up: only within a jump's reach above support in the same column, or
//!   on a ladder, or within a jump's reach and Claw's height below one
//!   (he catches a ladder with his body, so a jump from the floor reaches
//!   a ladder that ends higher than his feet can), or in a springboard's
//!   or a blower's throw, or within Claw's height above a throw and below
//!   a ladder (he catches it at the top of the throw);
//! - sideways: only within a jump's reach above support somewhere within
//!   a jump's width, or in a springboard's throw, so a gap can be crossed
//!   but open sky cannot;
//! - through a warp: from every open cell where Claw's feet are when his
//!   body touches the warp to the cells where they are once it has put
//!   him at its destination, at no cost.
//!
//! A [`DistanceField`] holds the cost of the cheapest path from every cell
//! to a target, in pixels. The graph never claims less than the level
//! allows by tiles, so the distance is an optimistic guide: the search
//! still plays every move in the simulation.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use claw_sim::SimLevel;
use claw_sim::logic::{
    break_plank, ground_blower, path_elevator, platform, rope, springboard, stepping_stone,
};
use claw_sim::player::swing;
use claw_sim::tiles::{CLIMB, DEATH, GROUND, SOLID};

/// Size of a graph cell, px.
pub const CELL: i32 = 16;

/// Cells above an open cell that must not be mostly solid for Claw to
/// stand in it (112 px tall).
pub const HEADROOM: i32 = 6;

/// Cells above a low cell that must not be mostly solid: Claw fits there
/// crouching (50 px tall), as on a platform that carries him under a low
/// rock.
pub const LOW_HEADROOM: i32 = 3;

/// Claw's height, px: a jump catches a ladder whose foot is this much
/// above the top of the jump's reach.
pub const BODY: i32 = 96;

/// How far below a rope's end Claw's feet hang, px: he hangs
/// [`swing::HANG_Y`] below it, and his feet are about 60 px below that.
const ROPE_FEET: i32 = swing::HANG_Y + 60;

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

/// How far either side of a ground blower its nozzle reaches, px: the
/// nozzle box is about the width of the jet's picture and a little more.
pub const BLOWER_HALF_WIDTH: i32 = 48;

/// How far below a ground blower its floor is looked for, px (the floor
/// of the level's blowers is 90 px below them).
const BLOWER_FLOOR_SEARCH: i32 = 200;

/// Image sets of the warps among the pickups.
const WARP_SETS: [&str; 2] = ["GAME_WARP", "GAME_VERTWARP"];

/// Record fields of a warp: where it puts Claw, x and y.
const WARP_TO: [usize; 2] = [55, 56];

/// How far from a warp's position, across and up, Claw's feet may be
/// when his body touches it (his feet up to his height lower), px.
const WARP_TOUCH: (i32, i32) = (40, 24);

/// Where Claw's feet are below a warp's destination, from and to, px: he
/// is put there with his position at it and falls to the floor.
const WARP_FEET: (i32, i32) = (40, 80);

/// Record fields of a path elevator: its built-in route, and the first of
/// the sixteen that hold its legs.
const PATH_BUILT_IN: usize = 15;
const PATH_LEGS: usize = 19;

/// The coarse graph of a level.
#[derive(Debug, Clone)]
pub struct Grid {
    cols: i32,
    rows: i32,
    open: Vec<bool>,
    /// The cell is open only for a crouching Claw (see [`LOW_HEADROOM`]).
    low: Vec<bool>,
    climb: Vec<bool>,
    /// The cell holds a one-way floor (and no ladder): nothing falls out
    /// of it downward.
    floor: Vec<bool>,
    /// Cells down to support (0 on support), saturating; `u16::MAX` with
    /// no support below before a closed cell.
    air: Vec<u16>,
    /// The least `air` within a jump's width to either side.
    env: Vec<u16>,
    /// Cells up to the nearest ladder cell above in the same column,
    /// through open cells (0 on a ladder), saturating; `u16::MAX` with
    /// none before a closed cell.
    to_ladder: Vec<u16>,
    /// The cell is in a springboard's or a ground blower's throw: Claw rises
    /// through it and steers across it.
    thrown: Vec<bool>,
    /// The cell is at most Claw's height above a throw's cell in the same
    /// column, through open cells: at the top of a throw his body reaches
    /// it, so he catches a ladder there.
    lifted: Vec<bool>,
    /// The cells of every throw, each with where Claw's feet are on the
    /// springboard or over the blower that throws him (the first throw
    /// marked, where two overlap).
    throw_bases: HashMap<usize, (i32, i32)>,
    /// Each elevator's and path elevator's way.
    rides: Vec<Ride>,
    /// The warps: for each cell Claw's feet are in at a warp's
    /// destination, the cells they are in when he touches it.
    warps: HashMap<usize, Vec<usize>>,
    reach_cells: u16,
    body_cells: u16,
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
        let mut floor = vec![false; n];
        let mut support = vec![false; n];
        let mut solid_cell = vec![false; n];
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
                solid_cell[i] = solid >= 8;
                open[i] = solid < 8 && !death;
                climb[i] = ladder && open[i];
                floor[i] = ground && !ladder && open[i];
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
        // No room for Claw under a solid cell close above; crouching room
        // under one a little higher.
        let mut low = vec![false; n];
        for r in 0..rows {
            for c in 0..cols {
                let i = (r * cols + c) as usize;
                let solid_above = |k: i32| r >= k && solid_cell[((r - k) * cols + c) as usize];
                if open[i] && (1..=HEADROOM).any(solid_above) {
                    if !(1..=LOW_HEADROOM).any(solid_above) {
                        low[i] = true;
                        continue;
                    }
                    open[i] = false;
                    climb[i] = false;
                    floor[i] = false;
                    support[i] = false;
                }
            }
        }
        let mut grid = Grid {
            cols,
            rows,
            open,
            low,
            climb,
            floor,
            air: Vec::new(),
            env: Vec::new(),
            to_ladder: Vec::new(),
            thrown: vec![false; n],
            lifted: Vec::new(),
            throw_bases: HashMap::new(),
            rides: Vec::new(),
            warps: HashMap::new(),
            reach_cells: (reach.jump_height / CELL).max(1) as u16,
            body_cells: (BODY / CELL) as u16,
        };
        grid.mark_platforms(level, &mut support);
        grid.mark_path_elevators(level, &mut support);
        grid.mark_ropes(level, &mut support);
        grid.mark_stepping_stones(level, &mut support);
        grid.mark_springboards(level, &mut support, reach);
        grid.mark_blowers(level, &mut support, reach);
        grid.mark_planks(level);
        grid.mark_warps(level);
        grid.lifted = vec![false; n];
        for c in 0..cols {
            let mut since = u16::MAX;
            for r in (0..rows).rev() {
                let i = grid.index(c, r);
                since = if !grid.open[i] {
                    u16::MAX
                } else if grid.thrown[i] {
                    0
                } else {
                    since.saturating_add(1)
                };
                grid.lifted[i] = since <= grid.body_cells;
            }
        }
        grid.to_ladder = vec![u16::MAX; n];
        for c in 0..cols {
            let mut above = u16::MAX;
            for r in 0..rows {
                let i = grid.index(c, r);
                let v = if !grid.open[i] {
                    u16::MAX
                } else if grid.climb[i] {
                    0
                } else {
                    above.saturating_add(1)
                };
                grid.to_ladder[i] = v;
                above = v;
            }
        }
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
    fn mark_platforms(&mut self, level: &SimLevel, support: &mut [bool]) {
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
            // The surface is near the top of the platform's picture, or,
            // where the level's riding surface lies below the platform's
            // position (the seventh level's air carts), that far below;
            // take a band of cells around it, the width of a peg either
            // side.
            let peg = spec.logic.contains("Peg");
            let drop = if peg { 0 } else { elevator_drop(level) };
            if !peg {
                let (ty_min, ty_max) = (y_min.min(y_max), y_min.max(y_max));
                let steps = ((x_max - x_min).max(ty_max - ty_min) / CELL).max(1);
                self.rides.push(Ride {
                    points: (0..=steps)
                        .map(|k| {
                            (
                                x_min + (x_max - x_min) * k / steps,
                                ty_min + (ty_max - ty_min) * k / steps,
                            )
                        })
                        .collect(),
                    looped: false,
                });
            }
            let (y_min, y_max) = (y_min.min(y_max) + drop, y_min.max(y_max) + drop);
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

    /// Marks the cells around every path elevator's route as support.
    fn mark_path_elevators(&mut self, level: &SimLevel, support: &mut [bool]) {
        for spec in &level.objects {
            if !path_elevator::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let r = &spec.record;
            let legs = path_elevator::built_in(r[PATH_BUILT_IN])
                .or_else(|| path_elevator::record_legs(&r[PATH_LEGS..PATH_LEGS + 16]))
                .unwrap_or_default();
            let (mut x, mut y) = (r[5], r[6]);
            let mut points = vec![(x, y)];
            for &(dir, d) in &legs {
                let mut done = 0;
                while done < d {
                    let step = (d - done).min(CELL);
                    done += step;
                    let (dx, dy) = path_elevator::along(dir, step);
                    x += dx;
                    y += dy;
                    points.push((x, y));
                }
            }
            let drop = elevator_drop(level);
            for &(px, py) in &points {
                let py = py + drop;
                self.mark_band(px - 2 * CELL, px + 2 * CELL, py - 3 * CELL, py, support);
            }
            self.rides.push(Ride {
                points,
                looped: true,
            });
        }
    }

    /// Marks where Claw's feet are while he hangs from a rope, along its
    /// whole swing, as support.
    fn mark_ropes(&self, level: &SimLevel, support: &mut [bool]) {
        for spec in &level.objects {
            if !rope::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let (x, y) = (spec.record[5], spec.record[6]);
            for &(ex, ey) in &rope::END {
                for ex in [ex, -ex] {
                    let (fx, fy) = (x + ex, y + ey + ROPE_FEET);
                    self.mark_band(
                        fx - swing::HANG_X,
                        fx + swing::HANG_X,
                        fy - CELL,
                        fy,
                        support,
                    );
                }
            }
        }
    }

    /// Marks where Claw stands on every stepping stone as support.
    fn mark_stepping_stones(&self, level: &SimLevel, support: &mut [bool]) {
        let rect = stepping_stone::surface(level.number);
        for spec in &level.objects {
            if !stepping_stone::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let (x, y) = (spec.record[5], spec.record[6]);
            let top = y + rect.top;
            self.mark_band(x + rect.left, x + rect.right, top - CELL, top, support);
        }
    }

    /// Joins the cells where Claw touches each warp to the cells his feet
    /// are in at its destination (see [`Grid::warps`]).
    fn mark_warps(&mut self, level: &SimLevel) {
        for spec in &level.objects {
            if !WARP_SETS.contains(&spec.image_set.as_str()) {
                continue;
            }
            let (x, y) = (spec.record[5], spec.record[6]);
            let (tx, ty) = (spec.record[WARP_TO[0]], spec.record[WARP_TO[1]]);
            if tx == 0 && ty == 0 {
                continue;
            }
            let cells = |left: i32, top: i32, right: i32, bottom: i32| {
                let mut out = Vec::new();
                for r in top.div_euclid(CELL)..=bottom.div_euclid(CELL) {
                    for c in left.div_euclid(CELL)..=right.div_euclid(CELL) {
                        if let Some(i) = self.cell_index(c * CELL, r * CELL)
                            && self.open[i]
                        {
                            out.push(i);
                        }
                    }
                }
                out
            };
            let (dx, dy) = WARP_TOUCH;
            let entries = cells(x - dx, y - dy, x + dx, y + dy + BODY);
            if entries.is_empty() {
                continue;
            }
            let (above, below) = WARP_FEET;
            for d in cells(tx, ty + above, tx, ty + below) {
                self.warps.entry(d).or_default().extend(&entries);
            }
        }
    }

    /// Marks where Claw stands on every springboard as support, and the
    /// cells of its throw (see [`Grid::thrown`]): from its top up to the
    /// throw's height, as wide as its top and a jump's width either side.
    fn mark_springboards(&mut self, level: &SimLevel, support: &mut [bool], reach: Reach) {
        let rect = springboard::surface(level.number);
        for spec in &level.objects {
            if !springboard::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let r = &spec.record;
            let (x, top) = (r[5], r[6] + rect.top);
            let height = springboard::height_of(r[springboard::HEIGHT_FIELD]);
            self.mark_band(x + rect.left, x + rect.right, top - CELL, top, support);
            self.mark_throw(x + rect.left, x + rect.right, top, height, reach);
        }
    }

    /// Marks the floor under every ground blower's nozzle as support, and
    /// the cells of its throw (see [`Grid::thrown`]): from the floor up to
    /// the throw's height, as wide as the nozzle and a jump's width either
    /// side. A blower throws Claw whenever he is over its nozzle as the jet
    /// starts, standing or not, so the search still times the throw.
    fn mark_blowers(&mut self, level: &SimLevel, support: &mut [bool], reach: Reach) {
        for spec in &level.objects {
            if !ground_blower::NAMES.contains(&spec.logic.as_str()) {
                continue;
            }
            let r = &spec.record;
            let (x, y) = (r[5], r[6]);
            let floor = (y..=y + BLOWER_FLOOR_SEARCH)
                .find(|&yy| matches!(level.map.attr(x, yy), SOLID | GROUND))
                .unwrap_or(y + BLOWER_FLOOR_SEARCH);
            let height = ground_blower::height_of(r[ground_blower::HEIGHT_FIELD]);
            let (left, right) = (x - BLOWER_HALF_WIDTH, x + BLOWER_HALF_WIDTH);
            self.mark_band(left, right, floor - CELL, floor, support);
            self.mark_throw(left, right, floor, height, reach);
        }
    }

    /// Marks the open cells of a throw from a top spanning `left` to
    /// `right` up to `height` above it, a jump's width either side, as
    /// [`Grid::thrown`].
    fn mark_throw(&mut self, left: i32, right: i32, top: i32, height: i32, reach: Reach) {
        let base = ((left + right) / 2, top - 2);
        let (left, right) = (left - reach.jump_width, right + reach.jump_width);
        let mut yy = top - height;
        while yy <= top {
            let mut xx = left;
            while xx <= right {
                if let Some(i) = self.cell_index(xx, yy)
                    && self.open[i]
                {
                    self.thrown[i] = true;
                    self.throw_bases.entry(i).or_insert(base);
                }
                xx += CELL;
            }
            yy += CELL;
        }
    }

    /// Lets Claw fall through the floor of every breaking plank: the cells
    /// of each piece's floor tile stay support but are no longer a one-way
    /// floor, since the piece gives way under him.
    fn mark_planks(&mut self, level: &SimLevel) {
        let Some(planks) = break_plank::level_table(level.number) else {
            return;
        };
        let (tw, th) = (level.map.tile_w().max(1), level.map.tile_h().max(1));
        for spec in &level.objects {
            if spec.logic != break_plank::SPAWNER {
                continue;
            }
            let r = &spec.record;
            for i in 0..r[break_plank::WIDTH_FIELD].max(1) {
                let (col, row) =
                    break_plank::floor_cell(planks, r[5] + i * planks.spacing, r[6], th);
                let (left, top) = (col * tw, row * th);
                let mut yy = top;
                while yy < top + th {
                    let mut xx = left;
                    while xx < left + tw {
                        if let Some(c) = self.cell_index(xx, yy) {
                            self.floor[c] = false;
                        }
                        xx += CELL;
                    }
                    yy += CELL;
                }
            }
        }
    }

    /// Marks the open cells of a box as support.
    fn mark_band(&self, left: i32, right: i32, top: i32, bottom: i32, support: &mut [bool]) {
        let mut yy = top;
        while yy <= bottom {
            let mut xx = left;
            while xx <= right {
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

    /// Where Claw's feet are on the springboard or over the blower whose
    /// throw (x, y) is in, when it is in one.
    pub fn throw_base(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        self.cell_index(x, y)
            .and_then(|i| self.throw_bases.get(&i))
            .copied()
    }

    /// The way of the elevator or path elevator whose position is
    /// (`x`, `y`) (the way that passes nearest, within a cell), and the
    /// index of its place nearest there; `None` when none passes there.
    pub fn ride_of(&self, x: i32, y: i32) -> Option<(&Ride, usize)> {
        self.rides
            .iter()
            .filter_map(|r| {
                let (i, d) = r
                    .points
                    .iter()
                    .map(|&(px, py)| (px - x).abs().max((py - y).abs()))
                    .enumerate()
                    .min_by_key(|&(_, d)| d)?;
                (d <= CELL).then_some((d, r, i))
            })
            .min_by_key(|&(d, _, _)| d)
            .map(|(_, r, i)| (r, i))
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
            let sideways = self.env[b] <= self.reach_cells || self.thrown[b];
            let below = (r + 1 < self.rows).then(|| self.index(c, r + 1));
            let rising = !self.low[b]
                && (self.air[b] <= self.reach_cells
                    || self.thrown[b]
                    || self.climb[b]
                    || below.is_some_and(|i| self.climb[i])
                    || (self.to_ladder[b] <= self.body_cells
                        && (self.air[b] <= self.reach_cells + self.body_cells || self.lifted[b])));
            let mut relax = |a: usize, cost: u32, heap: &mut BinaryHeap<Reverse<(u32, usize)>>| {
                if !self.open[a] {
                    return;
                }
                // Crouching is slow going.
                let cost = if self.low[a] || self.low[b] {
                    2 * cost
                } else {
                    cost
                };
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
                let above = self.index(c, r - 1);
                if !self.floor[above] {
                    relax(above, step, &mut heap);
                }
            }
            if let Some(entries) = self.warps.get(&b) {
                for &e in entries {
                    relax(e, 0, &mut heap);
                }
            }
        }
        DistanceField {
            cols: self.cols,
            rows: self.rows,
            dist,
        }
    }
}

/// The way an elevator or a path elevator takes: the places its position
/// passes, about a cell apart, in the order a path elevator passes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ride {
    /// The places, about a cell apart.
    pub points: Vec<(i32, i32)>,
    /// A path elevator's route, which it goes round in this order; else
    /// an elevator's track, which it goes along both ways.
    pub looped: bool,
}

impl Ride {
    /// How far the ride takes Claw, px, from place `from` to place `to`:
    /// on round a route, along it; along a track, either way.
    pub fn travel(&self, from: usize, to: usize) -> u32 {
        let n = self.points.len().max(1);
        let places = if self.looped {
            (to + n - from % n) % n
        } else {
            from.abs_diff(to)
        };
        places as u32 * CELL as u32
    }
}

/// How far below an elevator's position its riding surface lies in
/// `level`, px: the top of the level's elevator rectangle when it is below
/// the position (the seventh level's air carts ride 70 px below it), else
/// 0 (the surface is near the top of the picture).
pub fn elevator_drop(level: &SimLevel) -> i32 {
    platform::surface(level.number, platform::SurfaceKind::Elevator)
        .top
        .max(0)
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
    use claw_sim::level::ObjectSpec;

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
    fn a_gap_too_low_for_claw_is_closed() {
        // 16 px tiles: a ceiling over the right half of the floor, 48 px
        // above it (too low for Claw, even crouching), then 112 px.
        let gap = |n: usize| {
            let mut rows = vec![".".repeat(40); 8];
            rows[7 - n].replace_range(20..40, &"#".repeat(20));
            rows.push("#".repeat(40));
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            let l = SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40));
            Grid::build(&l, Reach::default())
        };
        // Feet on the floor (row 8 is the floor, row 7 the feet cell).
        let feet = 7 * 16 + 8;
        let g = gap(3);
        assert!(g.is_open(100, feet));
        assert!(!g.is_open(500, feet));
        let f = g.field_to(600, feet, 8);
        assert_eq!(f.at(100, feet), UNREACHABLE);
        let g = gap(7);
        assert!(g.is_open(500, feet));
        assert_ne!(g.field_to(600, feet, 8).at(100, feet), UNREACHABLE);
    }

    #[test]
    fn a_one_way_floor_is_not_fallen_through() {
        // A floor of ground tiles across the level, with a ladder through
        // it at the left: the way down from above the floor is the ladder.
        let rows = [
            "..........",
            "H=========",
            "H.........",
            "H.........",
            "##########",
        ];
        let g = Grid::build(&level(&rows), Reach::default());
        let f = g.field_to(600, 240, 20);
        let on_floor = f.at(600, 60);
        let at_ladder = f.at(30, 60);
        assert_ne!(on_floor, UNREACHABLE);
        assert!(
            on_floor > at_ladder,
            "from the floor the way down is back along it to the ladder: {on_floor} {at_ladder}"
        );
        // Without the ladder there is no way down at all.
        let rows = [
            "..........",
            "==========",
            "..........",
            "..........",
            "##########",
        ];
        let g = Grid::build(&level(&rows), Reach::default());
        let f = g.field_to(600, 240, 20);
        assert_eq!(f.at(600, 60), UNREACHABLE);
    }

    #[test]
    fn a_narrow_gap_is_crossed_and_death_tiles_are_closed() {
        let rows = ["..........", "..........", "###xx#####"];
        let g = Grid::build(&level(&rows), Reach::default());
        assert!(!g.is_open(3 * 64 + 10, 2 * 64 + 10));
        let f = g.field_to(600, 120, 20);
        assert_ne!(f.at(60, 120), UNREACHABLE);
    }

    #[test]
    fn claw_falls_through_a_breaking_plank_row_but_not_a_one_way_floor() {
        // 64 px tiles on the fifth level: a floor at the bottom, and a
        // ground row across the whole level in row 4; with no plank row
        // there it is a one-way floor, and with a row of planks along it
        // the way down from above it is straight through.
        let rows = [
            "..........",
            "..........",
            "..........",
            "..........",
            "==========",
            "..........",
            "..........",
            "##########",
        ];
        let field = |objects: Vec<ObjectSpec>| {
            let mut l = level(&rows);
            l.number = 5;
            l.objects = objects;
            Grid::build(&l, Reach::default())
                .field_to(320, 6 * 64 + 32, 20)
                .at(320, 4 * 64 - 8)
        };
        assert_eq!(field(Vec::new()), UNREACHABLE);
        // Ten pieces 64 px apart from x 0, their floor cells in row 4
        // (60 px above the pieces' y).
        let mut row = ObjectSpec::at(break_plank::SPAWNER, 0, 4 * 64 + 60, 0);
        row.record[break_plank::WIDTH_FIELD] = 10;
        let through = field(vec![row.clone()]);
        assert_ne!(through, UNREACHABLE);
        assert!(through <= 4 * 64, "straight down: {through}");
        // A row of three pieces at the left lets him through only there.
        row.record[break_plank::WIDTH_FIELD] = 3;
        let around = field(vec![row]);
        assert!(around > through, "{around} {through}");
        assert_ne!(around, UNREACHABLE);
    }

    /// 16 px tiles: a floor at the bottom (feet in row 28) and a high
    /// floor on the right in row 5, with a ladder up to it whose foot is
    /// `foot` rows above the lower floor's feet row.
    fn ladder_level(foot: usize) -> SimLevel {
        let mut rows = vec![".".repeat(40); 30];
        rows[29] = "#".repeat(40);
        rows[5].replace_range(22..40, &"#".repeat(18));
        for row in rows.iter_mut().take(29 - foot).skip(1) {
            row.replace_range(21..22, "H");
        }
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40))
    }

    #[test]
    fn a_jump_catches_a_ladder_above_its_reach_with_claws_body() {
        let feet = 28 * 16 + 8;
        let reach = Reach::default();
        // The foot of the ladder 192 px up: past the jump's 160 px, but
        // Claw's body is there at the top of the jump.
        let g = Grid::build(&ladder_level(12), reach);
        let f = g.field_to(30 * 16, 4 * 16 + 8, 16);
        assert_ne!(f.at(21 * 16 + 8, feet), UNREACHABLE);
        // 288 px up is out of reach.
        let g = Grid::build(&ladder_level(18), reach);
        let f = g.field_to(30 * 16, 4 * 16 + 8, 16);
        assert_eq!(f.at(21 * 16 + 8, feet), UNREACHABLE);
    }

    /// 16 px tiles: floor on both sides of a death pit 480 px wide (feet
    /// in row 28), with `objects` placed.
    fn pit_level(objects: Vec<ObjectSpec>) -> SimLevel {
        let mut row = "#".repeat(10);
        row.push_str(&"x".repeat(30));
        row.push_str(&"#".repeat(10));
        let mut rows = vec![".".repeat(50); 29];
        rows.push(row);
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut level = SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40));
        level.objects = objects;
        level
    }

    #[test]
    fn a_rope_or_a_path_elevator_crosses_a_pit_too_wide_to_jump() {
        let feet = 28 * 16 + 8;
        let field = |level: &SimLevel| {
            Grid::build(level, Reach::default())
                .field_to(45 * 16, feet, 16)
                .at(5 * 16, feet)
        };
        assert_eq!(field(&pit_level(Vec::new())), UNREACHABLE);
        // A rope over the middle whose end swings 100 px above the floor's
        // feet at the bottom of its swing.
        let rope = ObjectSpec::at("AniRope", 25 * 16, feet - 100 - 87 - ROPE_FEET, 0);
        assert_ne!(field(&pit_level(vec![rope])), UNREACHABLE);
        // A path elevator going across and back at the floor's height.
        let mut lift = ObjectSpec::at("PathElevator", 10 * 16, feet + 8, 0);
        lift.record[PATH_LEGS..PATH_LEGS + 4].copy_from_slice(&[6, 480, 4, 480]);
        assert_ne!(field(&pit_level(vec![lift])), UNREACHABLE);
    }

    #[test]
    fn a_warp_joins_one_side_of_a_pit_to_the_other() {
        let feet = 28 * 16 + 8;
        let field = |level: &SimLevel| {
            Grid::build(level, Reach::default())
                .field_to(45 * 16, feet, 16)
                .at(2 * 16, feet)
        };
        // A warp on the left floor, its destination standing on the right
        // floor: the right floor is as near from where the warp is touched
        // as from where it puts Claw.
        let warp = |set: &str, to: (i32, i32)| {
            let mut w = ObjectSpec::at("SpecialPowerup", 6 * 16, feet - 60, 0);
            w.image_set = set.to_string();
            w.record[WARP_TO[0]] = to.0;
            w.record[WARP_TO[1]] = to.1;
            w
        };
        let near = Grid::build(&pit_level(Vec::new()), Reach::default())
            .field_to(45 * 16, feet, 16)
            .at(43 * 16, feet);
        let through = field(&pit_level(vec![warp("GAME_WARP", (43 * 16, feet - 60))]));
        assert_ne!(through, UNREACHABLE);
        assert!(through <= near + 5 * CELL as u32, "{through} {near}");
        // Without a destination, or as another pickup, it joins nothing.
        assert_eq!(
            field(&pit_level(vec![warp("GAME_WARP", (0, 0))])),
            UNREACHABLE
        );
        assert_eq!(
            field(&pit_level(vec![warp(
                "GAME_POWERUPS_GHOST",
                (43 * 16, feet - 60)
            )])),
            UNREACHABLE
        );
    }

    #[test]
    fn the_seventh_levels_air_carts_carry_claw_70_px_below_their_position() {
        // A path elevator going across and back with its position 70 px
        // above the floor's feet: the seventh level's air carts ride 70 px
        // below their position, so the pit is crossed at the floor's
        // height, with no climb on to the cart and down from it.
        let feet = 28 * 16 + 8;
        let field = |number: u32| {
            let mut lift = ObjectSpec::at("PathElevator", 10 * 16, feet + 8 - 70, 0);
            lift.record[PATH_LEGS..PATH_LEGS + 4].copy_from_slice(&[6, 480, 4, 480]);
            let mut level = pit_level(vec![lift]);
            level.number = number;
            Grid::build(&level, Reach::default())
                .field_to(45 * 16, feet, 16)
                .at(5 * 16, feet)
        };
        assert_eq!(elevator_drop(&pit_level(Vec::new())), 0);
        let mut seventh = pit_level(Vec::new());
        seventh.number = 7;
        assert_eq!(elevator_drop(&seventh), 70);
        let low = field(7);
        assert_ne!(low, UNREACHABLE);
        // Along the cart's route at the floor's height, no climb.
        assert!(low < 45 * 16 - 5 * 16 + 4 * CELL as u32, "{low}");
    }

    /// 16 px tiles on the fourth level (whose springboards are ferns): a
    /// floor (feet in row 28) and, from column 24 on, a cliff whose top is
    /// row 5 (feet in row 4), 384 px above the floor, with `objects`.
    fn cliff_level(objects: Vec<ObjectSpec>) -> SimLevel {
        let mut rows = vec![".".repeat(40); 30];
        rows[29] = "#".repeat(40);
        for row in rows.iter_mut().skip(5) {
            row.replace_range(24..40, &"#".repeat(16));
        }
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut level = SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40));
        level.number = 4;
        level.objects = objects;
        level
    }

    #[test]
    fn a_springboard_throws_claw_up_a_cliff_too_high_to_jump() {
        let (floor, top) = (28 * 16 + 8, 4 * 16 + 8);
        let field = |level: &SimLevel| {
            Grid::build(level, Reach::default())
                .field_to(30 * 16, top, 16)
                .at(5 * 16, floor)
        };
        assert_eq!(field(&cliff_level(Vec::new())), UNREACHABLE);
        // A fern on the floor by the cliff, its top on the floor's top.
        let fern = |height: i32| {
            let rect = springboard::surface(4);
            let mut f = ObjectSpec::at("SpringBoard", 21 * 16, 29 * 16 - rect.top, 0);
            f.record[springboard::HEIGHT_FIELD] = height;
            f
        };
        // The usual throw (450 px) clears the cliff; a throw of 200 px
        // does not.
        assert_ne!(field(&cliff_level(vec![fern(0)])), UNREACHABLE);
        let low = cliff_level(vec![fern(200)]);
        assert_eq!(field(&low), UNREACHABLE);
        // The throw's cells: up to its height over the fern's top, and a
        // jump's width beside it.
        let g = Grid::build(&low, Reach::default());
        let thrown = |x: i32, y: i32| g.thrown[g.cell_index(x, y).unwrap()];
        assert!(thrown(21 * 16, 29 * 16 - 190));
        assert!(!thrown(21 * 16, 29 * 16 - 220));
        assert!(thrown(21 * 16 - 32 - 180, 20 * 16));
        assert!(!thrown(21 * 16 - 32 - 210, 20 * 16));
    }

    #[test]
    fn a_throws_cells_lead_back_to_its_springboard() {
        // The fern by the cliff: its top on the floor's top (y 464), its
        // middle at x 336.
        let rect = springboard::surface(4);
        let fern = ObjectSpec::at("SpringBoard", 21 * 16, 29 * 16 - rect.top, 0);
        let g = Grid::build(&cliff_level(vec![fern]), Reach::default());
        let top = 29 * 16;
        let base = (21 * 16 + (rect.left + rect.right) / 2, top - 2);
        // Over the fern up to the throw's height, and beside it within a
        // jump's width.
        assert_eq!(g.throw_base(21 * 16, top - 100), Some(base));
        assert_eq!(g.throw_base(21 * 16, top - 300), Some(base));
        assert_eq!(g.throw_base(21 * 16 - 150, top - 400), Some(base));
        // Above the throw's height, or beyond its width: no base.
        assert_eq!(g.throw_base(21 * 16, top - 600), None);
        assert_eq!(g.throw_base(21 * 16 - 300, top - 300), None);
        // A level without springboards has no throw.
        let bare = Grid::build(&cliff_level(Vec::new()), Reach::default());
        assert_eq!(bare.throw_base(21 * 16, top - 300), None);
    }

    #[test]
    fn a_ground_blower_throws_claw_up_a_cliff_too_high_to_jump() {
        let (floor, top) = (28 * 16 + 8, 4 * 16 + 8);
        let field = |level: &SimLevel| {
            Grid::build(level, Reach::default())
                .field_to(30 * 16, top, 16)
                .at(5 * 16, floor)
        };
        // A blower set into the floor by the cliff, 90 px above the floor's
        // top as the sixth level's are; its floor is found below it.
        let blower = |height: i32| {
            let mut b = ObjectSpec::at("GroundBlower", 21 * 16, 29 * 16 - 90, 0);
            b.record[ground_blower::HEIGHT_FIELD] = height;
            b
        };
        let with = |b: ObjectSpec| {
            let mut level = cliff_level(vec![b]);
            level.number = 6;
            level
        };
        // The usual throw (450 px) clears the cliff; a throw of 200 px
        // does not.
        assert_ne!(field(&with(blower(0))), UNREACHABLE);
        let low = with(blower(200));
        assert_eq!(field(&low), UNREACHABLE);
        // The throw's cells: up to its height over the floor, and a jump's
        // width beside the nozzle.
        let g = Grid::build(&low, Reach::default());
        let thrown = |x: i32, y: i32| g.thrown[g.cell_index(x, y).unwrap()];
        assert!(thrown(21 * 16, 29 * 16 - 190));
        assert!(!thrown(21 * 16, 29 * 16 - 220));
        assert!(thrown(21 * 16 - BLOWER_HALF_WIDTH - 180, 20 * 16));
        assert!(!thrown(21 * 16 - BLOWER_HALF_WIDTH - 210, 20 * 16));
    }

    #[test]
    fn claw_catches_a_ladder_at_the_top_of_a_throw() {
        // The cliff of 384 px with a ladder on its face (column 23, from
        // row 2 down to row `foot`), and a blower whose 200 px throw alone
        // falls short: his body catches the ladder when its foot is within
        // his height above the throw's top (464 - 200 = 264 px).
        let (floor, top) = (28 * 16 + 8, 4 * 16 + 8);
        let field = |foot: usize| {
            let mut rows = vec![".".repeat(40); 30];
            rows[29] = "#".repeat(40);
            for row in rows.iter_mut().skip(5) {
                row.replace_range(24..40, &"#".repeat(16));
            }
            for row in rows.iter_mut().take(foot + 1).skip(2) {
                row.replace_range(23..24, "H");
            }
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            let mut level = SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40));
            level.number = 6;
            let mut b = ObjectSpec::at("GroundBlower", 18 * 16, 29 * 16 - 90, 0);
            b.record[ground_blower::HEIGHT_FIELD] = 200;
            level.objects = vec![b];
            Grid::build(&level, Reach::default())
                .field_to(30 * 16, top, 16)
                .at(5 * 16, floor)
        };
        // The ladder's foot at 176 px, 88 px above the throw's top.
        assert_ne!(field(10), UNREACHABLE);
        // At 112 px, 152 px above it: out of reach.
        assert_eq!(field(6), UNREACHABLE);
    }

    #[test]
    fn a_stepping_stone_splits_a_pit_too_wide_to_jump() {
        let feet = 28 * 16 + 8;
        let field = |stones: &[i32]| {
            let rect = stepping_stone::surface(4);
            let objects = stones
                .iter()
                .map(|&x| ObjectSpec::at("SteppingStone2", x, 29 * 16 - rect.top, 0))
                .collect();
            let mut level = pit_level(objects);
            level.number = 4;
            Grid::build(&level, Reach::default())
                .field_to(45 * 16, feet, 16)
                .at(5 * 16, feet)
        };
        assert_eq!(field(&[]), UNREACHABLE);
        // A stone in the middle of the pit leaves two gaps a jump crosses.
        assert_ne!(field(&[25 * 16]), UNREACHABLE);
        // A stone near one side leaves the other gap too wide.
        assert_eq!(field(&[12 * 16]), UNREACHABLE);
    }

    #[test]
    fn claw_crouches_under_a_low_rock_but_never_stands_or_jumps_into_it() {
        // 16 px tiles, feet in row 28: a rock over columns 15 to 24 whose
        // underside is `gap` rows above the feet row.
        let field = |gap: usize| {
            let mut rows = vec![".".repeat(40); 30];
            rows[29] = "#".repeat(40);
            for r in rows.iter_mut().take(29 - gap) {
                r.replace_range(15..25, &"#".repeat(10));
            }
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            let g = Grid::build(
                &SimLevel::synthetic(TileMap::from_ascii(16, &rows), (40, 40)),
                Reach::default(),
            );
            let feet = 28 * 16 + 8;
            (g.field_to(35 * 16, feet, 16).at(5 * 16, feet), g)
        };
        // Four free rows (64 px): room to crouch, at twice the cost.
        let (d, g) = field(4);
        assert_ne!(d, UNREACHABLE);
        assert!(d > 30 * 16, "{d}");
        assert!(g.is_open(20 * 16, 28 * 16 + 8));
        // Two free rows: no room at all.
        assert_eq!(field(2).0, UNREACHABLE);
    }
}
