//! Activation: which level objects are simulated.
//!
//! Level objects start dormant in a spatial index. Each tick the objects
//! whose stored point lies inside a window around the view centre are put
//! into the live list (enter pass), and live level objects that left their
//! window go back to sleep at their clamped position or are destroyed
//! (leave pass). A dormant object is frozen: no logic, timers, collision or
//! movement until it is woken again.
//!
//! The window depends on the object's activation class: 1600x1200 for
//! enemies, bosses and projectiles, 2560x1920 for elevators and ambient
//! sounds, 831x623 for pickups, decoration and small hazards. These sizes
//! are gameplay rules, not screen sizes, so [`ActivationConfig`] keeps them
//! fixed for the original 640x480 view and offers three modes:
//!
//! - [`ActivationMode::Original`]: the original windows (the default).
//! - [`ActivationMode::FrozenEdge`]: the original simulation; a wider
//!   presentation draws dormant objects in the extra width frozen.
//! - [`ActivationMode::Widened`]: every window grows by the extra half view
//!   width, so everything on a wide screen is live, except breakable
//!   objects and jump switches, which keep the original window around the
//!   centre a 640x480 view would have; that keeps every crate and keg
//!   waking exactly where it would in the original, and every guard
//!   launched by a switch only where the original launches it. This changes gameplay and is recorded in
//!   traces.
//!
//! The small objects' window (class 2) is a measured value: the original
//! game runs it at 831x623 (half extents 415 x 311), and crates, kegs,
//! pickups and their glitters wake and sleep by that window. The original
//! and frozen-edge modes use the measured window, and so do the breakable
//! exceptions of the widened mode; the widened mode's other small objects
//! keep its parameters as they were (64 x 48 px beyond the half view).

use crate::container::Container;
use crate::object::{Object, ObjectId, flags};
use crate::registry::REG_ALWAYS;
use crate::view::{PlaneGeometry, PlaneView};

/// How the activation windows relate to the presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActivationMode {
    /// The original windows around the 640x480 view.
    #[default]
    Original,
    /// The original simulation; dormant objects in a wider presentation are
    /// drawn frozen.
    FrozenEdge,
    /// Windows widened by the extra half view, breakables excepted.
    Widened,
}

impl ActivationMode {
    /// Name used in traces.
    pub fn name(self) -> &'static str {
        match self {
            ActivationMode::Original => "original",
            ActivationMode::FrozenEdge => "frozen-edge",
            ActivationMode::Widened => "widened",
        }
    }
}

/// Logics that keep the original window in widened mode: breakable crates,
/// kegs and statues, which shots from timed traps could otherwise break
/// before Claw arrives.
pub const BREAKABLE_LOGICS: [&str; 9] = [
    "PowderKeg",
    "Crate",
    "FrontCrate",
    "BehindCrate",
    "FrontStackedCrates",
    "BackStackedCrates",
    "CratePowerup",
    "FrontStatue",
    "BehindStatue",
];

/// Other logics that keep the original window in widened mode: the jump
/// switches, which a pacing guard sets off with no help from Claw, so
/// waking one earlier would launch guards before Claw arrives.
pub const ORIGINAL_WINDOW_LOGICS: [&str; 1] = ["JumpSwitch"];

/// Off-screen margin of the small objects' window (class 2) in the
/// original and frozen-edge modes, px: half extents 415 x 311 around the
/// 640x480 view, a window of 831x623. A measured value: the window the
/// original game runs while a level plays (768x576, the earlier value, is
/// [`WIDENED_SMALL_MARGIN`]). With it a crate stack 429 px right of the
/// first level's start wakes in tick 6 and its two crates roll from tick 8
/// (the third pulse), as in the original.
pub const SMALL_MARGIN: (i32, i32) = (95, 71);

/// Off-screen margin of the small objects' window in the widened mode,
/// px: 768x576 around the 640x480 view. The widened mode keeps it as it
/// was for every small object but the breakable exceptions.
pub const WIDENED_SMALL_MARGIN: (i32, i32) = (64, 48);

/// Off-screen margins of the original windows per class, px. The widened
/// mode's breakable exceptions use them around the original view, so they
/// wake where the original wakes them.
pub const ORIGINAL_MARGINS: [(i32, i32); 3] = [(480, 360), (960, 720), SMALL_MARGIN];

/// Activation parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationConfig {
    /// The mode.
    pub mode: ActivationMode,
    /// Activation view in logic pixels: 640x480 in the original and
    /// frozen-edge modes, the presentation width in widened mode. Should be
    /// even.
    pub view: (u32, u32),
    /// The original view, around which excepted objects are activated.
    pub original_view: (u32, u32),
    /// Off-screen margin of each class beyond half the view, px.
    pub margins: [(i32, i32); 3],
    /// Objects flagged to be destroyed far outside the plane go when the
    /// centre is this far outside it, px.
    pub far_margin: (i32, i32),
    /// Index cell size per class (affects only the order of activation
    /// within one pass).
    pub cells: [(i32, i32); 3],
    /// Logic names that keep the original window in widened mode.
    pub exceptions: Vec<String>,
}

impl Default for ActivationConfig {
    fn default() -> Self {
        ActivationConfig::original()
    }
}

impl ActivationConfig {
    /// The original windows: 1600x1200, 2560x1920 and 831x623 around the
    /// 640x480 view.
    pub fn original() -> ActivationConfig {
        ActivationConfig {
            mode: ActivationMode::Original,
            view: (640, 480),
            original_view: (640, 480),
            margins: ORIGINAL_MARGINS,
            far_margin: (320, 220),
            cells: [(500, 250), (1000, 1000), (250, 125)],
            exceptions: BREAKABLE_LOGICS
                .iter()
                .chain(&ORIGINAL_WINDOW_LOGICS)
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// The original simulation for a wider presentation drawn with frozen
    /// edges.
    pub fn frozen_edge() -> ActivationConfig {
        ActivationConfig {
            mode: ActivationMode::FrozenEdge,
            ..ActivationConfig::original()
        }
    }

    /// Windows widened for a presentation `view` pixels wide and tall
    /// (logic pixels; the width is rounded up to an even number).
    pub fn widened(view: (u32, u32)) -> ActivationConfig {
        let w = view.0.max(640).div_ceil(2) * 2;
        let h = view.1.max(480).div_ceil(2) * 2;
        ActivationConfig {
            mode: ActivationMode::Widened,
            view: (w, h),
            margins: [(480, 360), (960, 720), WIDENED_SMALL_MARGIN],
            ..ActivationConfig::original()
        }
    }

    /// Half extents of a class's window around a view of `view` pixels.
    pub fn half_extents(&self, class: usize, view: (u32, u32)) -> (i32, i32) {
        let m = self.margins[class.min(2)];
        ((view.0 / 2) as i32 + m.0, (view.1 / 2) as i32 + m.1)
    }

    /// Half extents of a class's window for the widened mode's breakable
    /// exceptions: the original margins around the original view.
    pub fn exception_half_extents(&self, class: usize) -> (i32, i32) {
        let m = ORIGINAL_MARGINS[class.min(2)];
        (
            (self.original_view.0 / 2) as i32 + m.0,
            (self.original_view.1 / 2) as i32 + m.1,
        )
    }

    /// True when `logic` keeps the original window.
    pub fn is_exception(&self, logic: &str) -> bool {
        self.mode == ActivationMode::Widened && self.exceptions.iter().any(|e| e == logic)
    }
}

/// Window centres for one tick: the activation view's and the original
/// view's (equal outside widened mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Centres {
    /// Centre of the activation view.
    pub main: (i32, i32),
    /// Centre of the original 640x480 view.
    pub original: (i32, i32),
}

impl Centres {
    /// Centres for the camera point `point`.
    pub fn new(cfg: &ActivationConfig, plane: PlaneGeometry, point: (i32, i32)) -> Centres {
        let main = PlaneView::new(plane, point, cfg.view).activation_centre(plane);
        let original = if cfg.mode == ActivationMode::Widened {
            PlaneView::new(plane, point, cfg.original_view).activation_centre(plane)
        } else {
            main
        };
        Centres { main, original }
    }
}

#[derive(Debug, Clone)]
struct Grid {
    shift: (u32, u32),
    cols: i32,
    rows: i32,
    cells: Vec<Vec<ObjectId>>,
}

fn floor_log2(v: i32) -> u32 {
    31 - (v.max(1) as u32).leading_zeros()
}

impl Grid {
    fn new(cell: (i32, i32), plane: PlaneGeometry) -> Grid {
        let shift = (floor_log2(cell.0), floor_log2(cell.1));
        let cols = (plane.width.max(1) >> shift.0) + 1;
        let rows = (plane.height.max(1) >> shift.1) + 1;
        Grid {
            shift,
            cols,
            rows,
            cells: vec![Vec::new(); (cols as usize) * (rows as usize)],
        }
    }

    fn cell_of(&self, x: i32, y: i32) -> usize {
        let c = (x >> self.shift.0).clamp(0, self.cols - 1);
        let r = (y >> self.shift.1).clamp(0, self.rows - 1);
        (r * self.cols + c) as usize
    }

    fn file(&mut self, id: ObjectId, point: (i32, i32)) {
        let i = self.cell_of(point.0, point.1);
        self.cells[i].push(id);
    }

    /// Removes and returns the objects selected by `pick` among the cells
    /// covering `area` (left, top, right, bottom): rows top to bottom,
    /// columns left to right, most recently filed first within a cell.
    fn take(
        &mut self,
        area: (i32, i32, i32, i32),
        mut pick: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        let c0 = (area.0 >> self.shift.0).clamp(0, self.cols - 1);
        let c1 = (area.2 >> self.shift.0).clamp(0, self.cols - 1);
        let r0 = (area.1 >> self.shift.1).clamp(0, self.rows - 1);
        let r1 = (area.3 >> self.shift.1).clamp(0, self.rows - 1);
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                let cell = &mut self.cells[(r * self.cols + c) as usize];
                let mut i = cell.len();
                while i > 0 {
                    i -= 1;
                    if pick(cell[i]) {
                        out.push(cell.remove(i));
                    }
                }
            }
        }
        out
    }

    fn drain_all(&mut self, mut pick: impl FnMut(ObjectId) -> bool) -> Vec<ObjectId> {
        let mut out = Vec::new();
        for cell in &mut self.cells {
            let mut i = cell.len();
            while i > 0 {
                i -= 1;
                if pick(cell[i]) {
                    out.push(cell.remove(i));
                }
            }
        }
        out
    }

    fn count(&self) -> usize {
        self.cells.iter().map(Vec::len).sum()
    }
}

fn window(cfg: &ActivationConfig, obj: &Object, c: &Centres) -> (i32, i32, i32, i32) {
    let class = obj.activation_class();
    let (centre, (hx, hy)) = if cfg.is_exception(&obj.logic) {
        (c.original, cfg.exception_half_extents(class))
    } else {
        (c.main, cfg.half_extents(class, cfg.view))
    };
    (centre.0 - hx, centre.1 - hy, centre.0 + hx, centre.1 + hy)
}

fn inside(cfg: &ActivationConfig, obj: &Object, c: &Centres, p: (i32, i32)) -> bool {
    let w = window(cfg, obj, c);
    p.0 >= w.0 && p.0 <= w.2 && p.1 >= w.1 && p.1 <= w.3
}

/// The dormant objects of the main plane and the two passes.
#[derive(Debug, Clone)]
pub struct Activation {
    cfg: ActivationConfig,
    plane: PlaneGeometry,
    grids: [Grid; 3],
    last: Option<Centres>,
}

/// What a leave pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaveReport {
    /// Objects put to sleep.
    pub slept: Vec<ObjectId>,
    /// Objects destroyed.
    pub destroyed: Vec<ObjectId>,
}

impl Activation {
    /// An empty index over `plane`.
    pub fn new(cfg: ActivationConfig, plane: PlaneGeometry) -> Activation {
        let grids = [
            Grid::new(cfg.cells[0], plane),
            Grid::new(cfg.cells[1], plane),
            Grid::new(cfg.cells[2], plane),
        ];
        Activation {
            cfg,
            plane,
            grids,
            last: None,
        }
    }

    /// The parameters.
    pub fn config(&self) -> &ActivationConfig {
        &self.cfg
    }

    /// Number of dormant objects.
    pub fn dormant_count(&self) -> usize {
        self.grids.iter().map(Grid::count).sum()
    }

    /// Files a dormant object at its stored point.
    pub fn file(&mut self, obj: &Object, id: ObjectId) {
        self.grids[obj.activation_class()].file(id, obj.grid_point);
    }

    /// Makes every dormant always-active object live (level start).
    pub fn wake_always_active(&mut self, objects: &mut Container) -> usize {
        let mut woken = Vec::new();
        for g in &mut self.grids {
            woken.extend(g.drain_all(|id| {
                objects.get(id).is_some_and(|o| {
                    o.flags & flags::ALWAYS_ACTIVE != 0 || o.reg_flags & REG_ALWAYS != 0
                })
            }));
        }
        for &id in &woken {
            objects.insert(id);
        }
        woken.len()
    }

    /// The enter pass: wakes the dormant objects whose stored point is in
    /// their window. Does nothing when the centres have not moved since the
    /// last enter pass. Returns the woken objects in list-insertion order.
    pub fn enter(&mut self, objects: &mut Container, c: Centres) -> Vec<ObjectId> {
        if self.last == Some(c) {
            return Vec::new();
        }
        self.last = Some(c);
        let mut woken = Vec::new();
        for class in 0..3 {
            let mut area = Self::area(self.cfg.half_extents(class, self.cfg.view), c.main);
            if self.cfg.mode == ActivationMode::Widened {
                let o = Self::area(self.cfg.exception_half_extents(class), c.original);
                area = (
                    area.0.min(o.0),
                    area.1.min(o.1),
                    area.2.max(o.2),
                    area.3.max(o.3),
                );
            }
            let Activation { cfg, grids, .. } = self;
            let cfg = &*cfg;
            woken.extend(grids[class].take(area, |id| {
                objects
                    .get(id)
                    .is_some_and(|o| inside(cfg, o, &c, o.grid_point))
            }));
        }
        for &id in &woken {
            objects.insert(id);
        }
        woken
    }

    fn area((hx, hy): (i32, i32), centre: (i32, i32)) -> (i32, i32, i32, i32) {
        (centre.0 - hx, centre.1 - hy, centre.0 + hx, centre.1 + hy)
    }

    /// The leave pass over the live list. `notify(objects, id, state)` is
    /// called for objects that asked for a last call when they are
    /// destroyed or go to sleep.
    pub fn leave(
        &mut self,
        objects: &mut Container,
        c: Centres,
        notify: &mut dyn FnMut(&mut Container, ObjectId, i32),
    ) -> LeaveReport {
        let mut report = LeaveReport::default();
        let plane = self.plane;
        let mut cur = objects.head();
        while let Some(id) = cur {
            cur = objects.next_of(id);
            let Some(o) = objects.get(id) else { continue };
            if o.flags & flags::DESTROY_FAR != 0 {
                let far = self.cfg.far_margin;
                let (cx, cy) = c.main;
                if cx < -far.0
                    || cx > plane.width - 1 + far.0
                    || cy < -far.1
                    || cy > plane.height - 1 + far.1
                {
                    self.destroy(objects, id, notify);
                    report.destroyed.push(id);
                    continue;
                }
            }
            if o.flags & flags::ALWAYS_ACTIVE != 0 || o.flags & flags::WORLD == 0 {
                continue;
            }
            let point = (
                o.x.clamp(0, plane.width.max(1) - 1),
                o.y.clamp(0, plane.height.max(1) - 1),
            );
            let is_inside = inside(&self.cfg, o, &c, point);
            if let Some(o) = objects.get_mut(id) {
                o.grid_point = point;
            }
            if is_inside {
                continue;
            }
            let destroy_outside = objects
                .get(id)
                .is_some_and(|o| o.flags & flags::DESTROY_OUTSIDE != 0);
            if destroy_outside {
                self.destroy(objects, id, notify);
                report.destroyed.push(id);
                continue;
            }
            let wants_notify = objects
                .get(id)
                .is_some_and(|o| o.flags & flags::NOTIFY_SLEEP != 0);
            if wants_notify {
                let prev = objects.get(id).map_or(0, |o| o.state);
                if let Some(o) = objects.get_mut(id) {
                    o.state = crate::object::engine_state::SLEEPING;
                }
                notify(objects, id, crate::object::engine_state::SLEEPING);
                if let Some(o) = objects.get_mut(id)
                    && o.state == crate::object::engine_state::SLEEPING
                {
                    o.state = prev;
                }
            }
            objects.unlink(id);
            if let Some(o) = objects.get(id) {
                let class = o.activation_class();
                self.grids[class].file(id, point);
            }
            report.slept.push(id);
        }
        report
    }

    fn destroy(
        &mut self,
        objects: &mut Container,
        id: ObjectId,
        notify: &mut dyn FnMut(&mut Container, ObjectId, i32),
    ) {
        let wants = objects
            .get(id)
            .is_some_and(|o| o.flags & flags::NOTIFY_DESTROY != 0);
        if wants {
            if let Some(o) = objects.get_mut(id) {
                o.state = crate::object::engine_state::DESTROYING;
            }
            notify(objects, id, crate::object::engine_state::DESTROYING);
        }
        objects.remove(id);
    }
}
