//! Drawing between two snapshots.
//!
//! A frame is drawn at `alpha` of the way from the previous snapshot to the
//! current one. What happens to each kind of state:
//!
//! - object and camera positions are interpolated;
//! - a renderable flagged `teleported`, or one that moved more than 500 px on
//!   either axis in one tick, is drawn at its current position, and so is the
//!   camera when it moved more than 500 px;
//! - a renderable only in the current snapshot is drawn there; one only in
//!   the previous snapshot is not drawn;
//! - animation frame, image set, mirror, invert, draw mode and draw order are
//!   taken from the current snapshot, never blended;
//! - effects that run on time (palette fades, blinking) read the frame's
//!   simulation time, `time_ms`.

use std::collections::HashMap;

use crate::{PaletteFade, Point, Renderable, SimSnapshot, TICK_MS};

/// A move longer than this in one tick, on either axis, is drawn as a jump.
pub const SNAP_DISTANCE: f64 = 500.0;

/// One renderable at its drawing position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpolatedRenderable<'a> {
    /// The renderable as in the current snapshot.
    pub source: &'a Renderable,
    /// Drawing position in level pixels.
    pub x: f64,
    /// Drawing position in level pixels.
    pub y: f64,
}

/// Everything needed to draw one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct InterpolatedFrame<'a> {
    /// Tick of the current snapshot.
    pub tick: u64,
    /// Simulation time of the drawn instant in milliseconds.
    pub time_ms: f64,
    /// Camera (top-left of the logic view) at the drawn instant.
    pub camera: Point,
    /// Renderables in drawing order: by z, then by id.
    pub renderables: Vec<InterpolatedRenderable<'a>>,
    /// Palette fade of the current snapshot.
    pub palette_fade: Option<PaletteFade>,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Interpolates a point, or returns `curr` when the move is a jump.
pub fn lerp_point(prev: Point, curr: Point, alpha: f64) -> Point {
    if (curr.x - prev.x).abs() > SNAP_DISTANCE || (curr.y - prev.y).abs() > SNAP_DISTANCE {
        return curr;
    }
    Point {
        x: lerp(prev.x, curr.x, alpha),
        y: lerp(prev.y, curr.y, alpha),
    }
}

/// Simulation time in milliseconds of the instant `alpha` of the way from
/// `prev` to `curr`.
pub fn time_ms(prev: &SimSnapshot, curr: &SimSnapshot, alpha: f64) -> f64 {
    let tick = f64::from(TICK_MS);
    if curr.tick < prev.tick {
        return curr.tick as f64 * tick;
    }
    lerp(prev.tick as f64, curr.tick as f64, alpha) * tick
}

/// The frame drawn `alpha` (in [0, 1]) of the way from `prev` to `curr`.
///
/// For a level start or reload pass the same snapshot twice.
pub fn interpolate<'a>(
    prev: &'a SimSnapshot,
    curr: &'a SimSnapshot,
    alpha: f64,
) -> InterpolatedFrame<'a> {
    let alpha = alpha.clamp(0.0, 1.0);
    let before: HashMap<u32, &Renderable> = prev.renderables.iter().map(|r| (r.id, r)).collect();
    let mut renderables: Vec<InterpolatedRenderable<'a>> = curr
        .renderables
        .iter()
        .map(|r| {
            let at = Point { x: r.x, y: r.y };
            let pos = match before.get(&r.id) {
                Some(p) if !r.teleported => lerp_point(Point { x: p.x, y: p.y }, at, alpha),
                _ => at,
            };
            InterpolatedRenderable {
                source: r,
                x: pos.x,
                y: pos.y,
            }
        })
        .collect();
    renderables.sort_by_key(|r| (r.source.z, r.source.id));
    InterpolatedFrame {
        tick: curr.tick,
        time_ms: time_ms(prev, curr, alpha),
        camera: lerp_point(prev.camera, curr.camera, alpha),
        renderables,
        palette_fade: curr.palette_fade,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DrawMode;

    fn sprite(id: u32, x: f64, y: f64, z: i32) -> Renderable {
        Renderable {
            id,
            image_set_key: "SET".into(),
            frame: 1,
            x,
            y,
            z,
            mirror: false,
            invert: false,
            flash: false,
            draw_mode: DrawMode::NORMAL,
            teleported: false,
        }
    }

    fn snap(tick: u64, cam: (f64, f64), rs: Vec<Renderable>) -> SimSnapshot {
        SimSnapshot {
            tick,
            camera: Point { x: cam.0, y: cam.1 },
            renderables: rs,
            palette_fade: None,
        }
    }

    #[test]
    fn positions_blend_and_jumps_snap() {
        let mut far = sprite(3, 1000.0, 0.0, 0);
        let prev = snap(
            4,
            (0.0, 0.0),
            vec![
                sprite(1, 0.0, 0.0, 0),
                sprite(2, 0.0, 0.0, 0),
                sprite(3, 0.0, 0.0, 0),
            ],
        );
        let mut tele = sprite(2, 10.0, 10.0, 0);
        tele.teleported = true;
        far.z = -1;
        let curr = snap(
            5,
            (100.0, 600.0),
            vec![sprite(1, 10.0, 20.0, 0), tele, far, sprite(4, 7.0, 7.0, 5)],
        );
        let f = interpolate(&prev, &curr, 0.25);
        assert_eq!(f.tick, 5);
        assert!((f.time_ms - 4.25 * 18.0).abs() < 1e-9);
        // Camera moved 600 px on y: snapped.
        assert_eq!(f.camera, Point { x: 100.0, y: 600.0 });
        let ids: Vec<u32> = f.renderables.iter().map(|r| r.source.id).collect();
        assert_eq!(ids, vec![3, 1, 2, 4]);
        let by_id = |id| f.renderables.iter().find(|r| r.source.id == id).unwrap();
        assert_eq!((by_id(1).x, by_id(1).y), (2.5, 5.0));
        assert_eq!((by_id(2).x, by_id(2).y), (10.0, 10.0));
        assert_eq!((by_id(3).x, by_id(3).y), (1000.0, 0.0));
        assert_eq!((by_id(4).x, by_id(4).y), (7.0, 7.0));
    }

    #[test]
    fn same_snapshot_twice_draws_it_as_is() {
        let s = snap(9, (5.0, 6.0), vec![sprite(1, 3.0, 4.0, 0)]);
        let f = interpolate(&s, &s, 0.7);
        assert_eq!(f.camera, Point { x: 5.0, y: 6.0 });
        assert_eq!((f.renderables[0].x, f.renderables[0].y), (3.0, 4.0));
        assert_eq!(f.time_ms, 9.0 * 18.0);
    }
}
