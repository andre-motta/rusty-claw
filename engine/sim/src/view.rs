//! The main plane's view: where the camera point puts the visible
//! rectangle.
//!
//! The camera hands an integer point to the main plane. On an axis that
//! does not wrap the point is clamped into the plane and truncated to the
//! integer centre; the view rectangle is the centre minus half the view
//! size, pushed back inside the plane at the edges. The rectangle moves at
//! the edges, the centre does not.
//!
//! [`view_origin`] is the same placement for a point between whole pixels:
//! drawing only, for a presentation that moves the view between the
//! positions the game gives it.

use crate::object::Rect;

/// Size and wrap flags of the main plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneGeometry {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// Wraps horizontally.
    pub wrap_x: bool,
    /// Wraps vertically.
    pub wrap_y: bool,
}

/// The view of the main plane for one camera point and view size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaneView {
    /// Integer camera centre after the clamp or wrap.
    pub centre: (i32, i32),
    /// Visible rectangle in plane pixels, inclusive.
    pub rect: Rect,
}

fn axis(pos: i32, size: i32, wrap: bool) -> i32 {
    let size = size.max(1);
    if wrap {
        pos.rem_euclid(size)
    } else if pos < 0 {
        0
    } else if pos >= size {
        size - 1
    } else {
        pos
    }
}

impl PlaneView {
    /// The view for camera point `point` and a view of `view` pixels.
    pub fn new(plane: PlaneGeometry, point: (i32, i32), view: (u32, u32)) -> PlaneView {
        let cx = axis(point.0, plane.width, plane.wrap_x);
        let cy = axis(point.1, plane.height, plane.wrap_y);
        let vw = i32::try_from(view.0).unwrap_or(i32::MAX).max(1);
        let vh = i32::try_from(view.1).unwrap_or(i32::MAX).max(1);
        let edge = |c: i32, half: i32, size: i32, wrap: bool, v: i32| -> (i32, i32) {
            let mut lo = c - half;
            if lo < 0 {
                lo = if wrap { lo + size } else { 0 };
            }
            let mut hi = v + lo - 1;
            if hi >= size && !wrap {
                let d = hi - size + 1;
                hi -= d;
                lo -= d;
            }
            (lo, hi)
        };
        let (left, right) = edge(cx, vw / 2, plane.width, plane.wrap_x, vw);
        let (top, bottom) = edge(cy, vh / 2, plane.height, plane.wrap_y, vh);
        PlaneView {
            centre: (cx, cy),
            rect: Rect::new(left, top, right, bottom),
        }
    }

    /// The centre the activation windows are placed around: the view
    /// rectangle's midpoint plus one, or the centre itself on a wrapping
    /// axis.
    pub fn activation_centre(&self, plane: PlaneGeometry) -> (i32, i32) {
        let r = self.rect;
        let x = if plane.wrap_x {
            self.centre.0
        } else {
            (r.left + r.right) / 2 + 1
        };
        let y = if plane.wrap_y {
            self.centre.1
        } else {
            (r.top + r.bottom) / 2 + 1
        };
        (x, y)
    }
}

/// The top-left corner of the view of `view` pixels for a camera point
/// `point` that may lie between whole pixels: [`PlaneView::new`]'s rule
/// without the truncation, so a whole point gives exactly the corner of
/// its [`PlaneView`] and the corner moves continuously with the point.
/// Drawing only; the game always places its view on whole points.
pub fn view_origin(plane: PlaneGeometry, point: (f64, f64), view: (u32, u32)) -> (f64, f64) {
    let axis = |pos: f64, size: i32, wrap: bool, v: u32| -> f64 {
        let size = f64::from(size.max(1));
        let v = f64::from(i32::try_from(v).unwrap_or(i32::MAX).max(1));
        let half = (v / 2.0).floor();
        if wrap {
            let lo = pos.rem_euclid(size) - half;
            return if lo < 0.0 { lo + size } else { lo };
        }
        let c = pos.clamp(0.0, size - 1.0);
        let lo = (c - half).max(0.0);
        if lo + v > size { size - v } else { lo }
    };
    (
        axis(point.0, plane.width, plane.wrap_x, view.0),
        axis(point.1, plane.height, plane.wrap_y, view.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLANE: PlaneGeometry = PlaneGeometry {
        width: 6400,
        height: 2560,
        wrap_x: false,
        wrap_y: false,
    };

    #[test]
    fn view_rect_is_pushed_inside_at_the_edges() {
        let v = PlaneView::new(PLANE, (1000, 1000), (640, 480));
        assert_eq!(v.rect, Rect::new(680, 760, 1319, 1239));
        let v = PlaneView::new(PLANE, (100, 100), (640, 480));
        assert_eq!(v.centre, (100, 100));
        assert_eq!(v.rect, Rect::new(0, 0, 639, 479));
        let v = PlaneView::new(PLANE, (6399, 2560), (640, 480));
        assert_eq!(v.centre, (6399, 2559));
        assert_eq!(v.rect, Rect::new(5760, 2080, 6399, 2559));
    }

    #[test]
    fn activation_centre_is_clamped_to_half_a_view_from_the_edges() {
        for x in [-50, 0, 200, 319, 320, 321, 3000, 6079, 6080, 6200, 9000] {
            let v = PlaneView::new(PLANE, (x, 1000), (640, 480));
            let (cx, _) = v.activation_centre(PLANE);
            assert_eq!(cx, x.clamp(320, 6400 - 320), "x = {x}");
        }
        for y in [-10, 0, 239, 240, 241, 2319, 2320, 2400, 5000] {
            let v = PlaneView::new(PLANE, (1000, y), (640, 480));
            assert_eq!(
                v.activation_centre(PLANE).1,
                y.clamp(240, 2560 - 240),
                "y = {y}"
            );
        }
    }

    #[test]
    fn wrapping_axes_wrap() {
        let plane = PlaneGeometry {
            wrap_x: true,
            ..PLANE
        };
        let v = PlaneView::new(plane, (6500, 0), (640, 480));
        assert_eq!(v.centre.0, 100);
        assert_eq!(v.rect.left, 6400 - 220);
        assert_eq!(v.activation_centre(plane).0, 100);
    }

    #[test]
    fn the_origin_of_a_whole_point_is_the_corner_of_its_view() {
        let wrapping = PlaneGeometry {
            wrap_x: true,
            wrap_y: true,
            ..PLANE
        };
        let small = PlaneGeometry {
            width: 500,
            height: 300,
            ..PLANE
        };
        for plane in [PLANE, wrapping, small] {
            for x in [
                -50, 0, 100, 319, 320, 321, 3000, 6079, 6080, 6399, 6400, 9000,
            ] {
                for y in [-10, 0, 239, 240, 1000, 2320, 2559, 2560, 5000] {
                    for view in [(640, 480), (641, 481)] {
                        let r = PlaneView::new(plane, (x, y), view).rect;
                        assert_eq!(
                            view_origin(plane, (f64::from(x), f64::from(y)), view),
                            (f64::from(r.left), f64::from(r.top)),
                            "{plane:?} ({x}, {y}) {view:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_origin_moves_continuously_between_whole_points() {
        let o = view_origin(PLANE, (1000.25, 1000.75), (640, 480));
        assert_eq!(o, (680.25, 760.75));
        // Held at the edges as the whole points are.
        assert_eq!(view_origin(PLANE, (100.5, 100.5), (640, 480)), (0.0, 0.0));
        assert_eq!(
            view_origin(PLANE, (6398.5, 2558.5), (640, 480)),
            (5760.0, 2080.0)
        );
    }
}
