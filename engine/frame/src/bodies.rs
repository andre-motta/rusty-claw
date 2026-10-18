//! Falling bodies on a wide display.
//!
//! A body killed by a hit is thrown up and falls through the floor, and
//! the game removes it as soon as its picture leaves the logic view. On a
//! display wider than the logic view that happens in plain sight, in the
//! extra width at the sides. The simulation reports each such removal in
//! the tick's snapshot ([`SimSnapshot::removed_bodies`]) with what it
//! takes to go on drawing the body, and [`FallingBodies`] keeps a copy of
//! it that goes on falling and animating on screen until its picture
//! leaves the presented area, or for [`MAX_MS`] at most.
//!
//! The copies are drawing only. They are never part of the game: nothing
//! collides with them, they make no sound, draw no random numbers, use no
//! object of the game, and traces and replays never see them. They go on
//! a level start, a respawn, a restart, a camera jump or a death. A
//! presentation no wider than the logic view keeps none, so it shows
//! exactly what the game shows.

use crate::{DrawMode, GameEvent, Renderable, SimSnapshot, TICK_MS, ViewConfig};

/// Longest time a copy is drawn, ms.
pub const MAX_MS: u32 = 3000;

/// Room left around the picture box of a body whose frames the game does
/// not know, px.
pub const UNKNOWN_BOX: (i32, i32, i32, i32) = (-64, -64, 64, 64);

/// A body the game removed in a tick because it fell out of the logic
/// view, as the game had it at the end of that tick's logic (where the
/// last sprite of it would have been drawn): its picture, the frames its
/// animation would have shown in the following ticks, and its fall.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemovedBody {
    /// Id of the sprite it had (pairs the copy with the body's last
    /// sprite, so it is drawn on without a jump).
    pub id: u32,
    /// Image-set registry key.
    pub image_set_key: String,
    /// The frame it showed in the tick it was removed, then the frame of
    /// each following tick (the last one stays).
    pub frames: Vec<i32>,
    /// Exact position in level pixels, where the tick left it.
    pub x: f64,
    /// Exact position in level pixels, where the tick left it.
    pub y: f64,
    /// Speed across, px/s.
    pub vx: f64,
    /// Speed up or down, px/s (down is positive).
    pub vy: f64,
    /// Acceleration down, px/s/s.
    pub gravity: f64,
    /// Largest move on each axis in one tick, px; 0 for none.
    pub step_limit: f64,
    /// Draw order.
    pub z: i32,
    /// Mirrored horizontally.
    pub mirror: bool,
    /// Flipped vertically.
    pub invert: bool,
    /// How the sprite combines with the destination.
    pub draw_mode: DrawMode,
    /// Colour value of the draw mode, as [`Renderable::draw_color`].
    pub draw_color: Option<u8>,
    /// Box covering the picture of every frame in `frames`, relative to the
    /// position and as drawn (mirrored and flipped): left, top, right,
    /// bottom. `None` when the game does not know the frames.
    pub bounds: Option<(i32, i32, i32, i32)>,
}

impl RemovedBody {
    /// The frame shown `ticks` ticks after the removal.
    pub fn frame_at(&self, ticks: usize) -> i32 {
        match self.frames.get(ticks) {
            Some(&f) => f,
            None => self.frames.last().copied().unwrap_or(0),
        }
    }
}

/// One copy being drawn.
#[derive(Debug, Clone, PartialEq)]
struct Copy {
    body: RemovedBody,
    /// Ticks since the removal.
    age: usize,
}

/// Moves one axis at `v` with acceleration `a` for `dt` seconds: the move,
/// capped to `limit` when it is not 0, and the speed after it.
fn axis_step(v: &mut f64, a: f64, dt: f64, limit: f64) -> f64 {
    let d = (*v + 0.5 * a * dt) * dt;
    if limit > 0.0 && d.abs() > limit {
        let d = limit.copysign(d);
        if a != 0.0 {
            *v = v.signum() * (*v * *v + 2.0 * a * d).max(0.0).sqrt();
        }
        d
    } else {
        *v += a * dt;
        d
    }
}

impl Copy {
    /// One tick of the fall.
    fn step(&mut self) {
        let dt = f64::from(TICK_MS) * 0.001;
        let b = &mut self.body;
        b.x += axis_step(&mut b.vx, 0.0, dt, b.step_limit);
        b.y += axis_step(&mut b.vy, b.gravity, dt, b.step_limit);
        self.age += 1;
    }

    /// The whole-pixel position it is drawn at.
    fn at(&self) -> (i32, i32) {
        (self.body.x.trunc() as i32, self.body.y.trunc() as i32)
    }

    fn renderable(&self) -> Renderable {
        let (x, y) = self.at();
        let b = &self.body;
        Renderable {
            id: b.id,
            image_set_key: b.image_set_key.clone(),
            frame: b.frame_at(self.age),
            x: f64::from(x),
            y: f64::from(y),
            z: b.z,
            mirror: b.mirror,
            invert: b.invert,
            flash: false,
            draw_mode: b.draw_mode,
            draw_color: b.draw_color,
            teleported: false,
        }
    }

    /// True while its picture overlaps `area` (left, top, right, bottom).
    fn overlaps(&self, area: (f64, f64, f64, f64)) -> bool {
        let (x, y) = self.at();
        let (l, t, r, b) = self.body.bounds.unwrap_or(UNKNOWN_BOX);
        let (l, t, r, b) = (
            f64::from(x + l),
            f64::from(y + t),
            f64::from(x + r),
            f64::from(y + b),
        );
        r >= area.0 && l <= area.2 && b >= area.1 && t <= area.3
    }
}

/// The copies of removed bodies a presentation draws on (see the [module
/// documentation](self)).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FallingBodies {
    copies: Vec<Copy>,
    last_tick: Option<u64>,
}

impl FallingBodies {
    /// No copies.
    pub fn new() -> FallingBodies {
        FallingBodies::default()
    }

    /// Drops every copy, as a level start, a restart or a respawn does.
    pub fn clear(&mut self) {
        self.copies.clear();
        self.last_tick = None;
    }

    /// Number of copies being drawn.
    pub fn len(&self) -> usize {
        self.copies.len()
    }

    /// True when no copy is drawn.
    pub fn is_empty(&self) -> bool {
        self.copies.is_empty()
    }

    /// Takes the snapshot of the next tick, presented as `view`: moves the
    /// copies one tick on, starts a copy of each body the tick removed,
    /// drops the copies whose picture left the presented area or whose
    /// time is up, and adds the sprites of the others to the snapshot.
    ///
    /// Call it once for every tick, in order. A snapshot that does not
    /// follow the last one (a restart or another level), a camera jump or
    /// the end of the game drops every copy first (a respawn clears them
    /// through [`FallingBodies::clear`]; Claw's death alone does not, so a
    /// body falling while he dies finishes its fall); a presentation no
    /// wider than the logic view keeps none.
    pub fn absorb(&mut self, view: &ViewConfig, snapshot: &mut SimSnapshot) {
        let follows = self
            .last_tick
            .is_none_or(|t| snapshot.tick == t.wrapping_add(1));
        self.last_tick = Some(snapshot.tick);
        let game_over = snapshot
            .events
            .iter()
            .any(|e| matches!(e, GameEvent::GameOver));
        if !follows || snapshot.camera_teleported || game_over {
            self.copies.clear();
        }
        let (pw, ph) = view.presentation_size();
        if pw <= view.logic_width.max(1) {
            self.copies.clear();
            return;
        }
        for c in &mut self.copies {
            c.step();
        }
        self.copies
            .extend(snapshot.removed_bodies.iter().map(|b| Copy {
                body: b.clone(),
                age: 0,
            }));
        // The presented area: the presentation width around the logic view's
        // centre, widened on each side by the extra width, since a
        // presentation held inside the level by its edge is shifted by up to
        // that much.
        let lw = f64::from(view.logic_width);
        let extra = f64::from(pw) - lw;
        let centre = snapshot.camera.x + lw / 2.0;
        let half = f64::from(pw) / 2.0 + extra / 2.0;
        let area = (
            centre - half,
            snapshot.camera.y,
            centre + half,
            snapshot.camera.y + f64::from(ph),
        );
        let max_age = (MAX_MS / TICK_MS) as usize;
        self.copies.retain(|c| c.age <= max_age && c.overlaps(area));
        snapshot
            .renderables
            .extend(self.copies.iter().map(Copy::renderable));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AspectPolicy, Point};

    fn wide() -> ViewConfig {
        ViewConfig::default().with_render_size(1920, 1080)
    }

    fn body(x: f64, y: f64) -> RemovedBody {
        RemovedBody {
            id: 77,
            image_set_key: "LEVEL_RAT".to_string(),
            frames: vec![5, 6, 7],
            x,
            y,
            vx: 150.0,
            vy: -700.0,
            gravity: 2000.0,
            step_limit: 24.0,
            z: 9000,
            bounds: Some((-20, -20, 20, 20)),
            ..RemovedBody::default()
        }
    }

    fn snap(tick: u64, removed: Vec<RemovedBody>) -> SimSnapshot {
        SimSnapshot {
            tick,
            camera: Point { x: 0.0, y: 0.0 },
            removed_bodies: removed,
            ..SimSnapshot::default()
        }
    }

    #[test]
    fn a_removed_body_goes_on_falling_and_animating() {
        let mut bodies = FallingBodies::new();
        let view = wide();
        let mut s = snap(1, vec![body(680.0, 200.0)]);
        bodies.absorb(&view, &mut s);
        assert_eq!(bodies.len(), 1);
        let r = &s.renderables[0];
        assert_eq!((r.id, r.frame, r.z), (77, 5, 9000));
        assert_eq!((r.x, r.y), (680.0, 200.0), "where the game left it");
        let mut ys = vec![r.y];
        for t in 2..30 {
            let mut s = snap(t, Vec::new());
            bodies.absorb(&view, &mut s);
            let Some(r) = s.renderables.first() else {
                break;
            };
            if t == 2 {
                assert_eq!(r.frame, 6);
                // One tick of the fall: 2.7 px across, (-700 + 18) * 0.018
                // up.
                assert_eq!((r.x, r.y), (682.0, 187.0));
            }
            if t >= 3 {
                assert_eq!(r.frame, 7, "the last frame stays");
            }
            ys.push(r.y);
        }
        // Up, then down.
        let top = ys.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(top < 200.0);
        assert!(ys.last().copied().unwrap() > top);
    }

    #[test]
    fn a_copy_goes_once_its_picture_leaves_the_presented_area() {
        let mut bodies = FallingBodies::new();
        let view = wide();
        let mut s = snap(1, vec![body(700.0, 400.0)]);
        bodies.absorb(&view, &mut s);
        let mut gone = None;
        for t in 2..200 {
            let mut s = snap(t, Vec::new());
            bodies.absorb(&view, &mut s);
            if s.renderables.is_empty() {
                gone = Some(t);
                break;
            }
            // Still drawn: its picture reaches into the presented height.
            assert!(s.renderables[0].y - 20.0 <= 480.0);
        }
        let gone = gone.expect("the copy leaves at the bottom");
        assert!(gone < 60, "tick {gone}");
        assert!(bodies.is_empty());
    }

    #[test]
    fn a_copy_lasts_three_seconds_at_most() {
        let mut bodies = FallingBodies::new();
        let view = wide();
        let mut b = body(700.0, 240.0);
        b.vx = 0.0;
        b.vy = 0.0;
        b.gravity = 0.0;
        let mut s = snap(1, vec![b]);
        bodies.absorb(&view, &mut s);
        let mut last = 1;
        for t in 2..400 {
            let mut s = snap(t, Vec::new());
            bodies.absorb(&view, &mut s);
            if s.renderables.is_empty() {
                break;
            }
            last = t;
        }
        assert_eq!(last, 1 + u64::from(MAX_MS / TICK_MS));
    }

    #[test]
    fn a_body_outside_the_presented_width_is_not_drawn() {
        let mut bodies = FallingBodies::new();
        // 1920x1080: 854 px wide, 107 px each side of the logic view; with
        // the shift allowance the area reaches 214 px past each side.
        let mut s = snap(1, vec![body(640.0 + 214.0 + 30.0, 240.0)]);
        bodies.absorb(&wide(), &mut s);
        assert!(s.renderables.is_empty());
        assert!(bodies.is_empty());
    }

    #[test]
    fn a_presentation_as_wide_as_the_logic_view_keeps_none() {
        for view in [
            ViewConfig::default(),
            ViewConfig {
                aspect_policy: AspectPolicy::Pillarbox,
                ..wide()
            },
        ] {
            let mut bodies = FallingBodies::new();
            let mut s = snap(1, vec![body(680.0, 200.0)]);
            bodies.absorb(&view, &mut s);
            assert!(s.renderables.is_empty());
            assert!(bodies.is_empty());
        }
    }

    #[test]
    fn a_jump_the_game_over_or_a_restart_drops_every_copy() {
        let view = wide();
        let start = |bodies: &mut FallingBodies| {
            bodies.clear();
            let mut s = snap(1, vec![body(680.0, 200.0)]);
            bodies.absorb(&view, &mut s);
            assert_eq!(bodies.len(), 1);
        };
        let mut bodies = FallingBodies::new();

        start(&mut bodies);
        let mut s = snap(2, Vec::new());
        s.camera_teleported = true;
        bodies.absorb(&view, &mut s);
        assert!(bodies.is_empty() && s.renderables.is_empty());

        start(&mut bodies);
        let mut s = snap(2, Vec::new());
        s.events.push(GameEvent::ClawDied { lives_left: 2 });
        bodies.absorb(&view, &mut s);
        assert_eq!(bodies.len(), 1, "Claw's death alone keeps the fall");

        start(&mut bodies);
        let mut s = snap(2, Vec::new());
        s.events.push(GameEvent::GameOver);
        bodies.absorb(&view, &mut s);
        assert!(bodies.is_empty());

        start(&mut bodies);
        let mut s = snap(0, Vec::new());
        bodies.absorb(&view, &mut s);
        assert!(bodies.is_empty(), "a snapshot out of order is a restart");

        start(&mut bodies);
        bodies.clear();
        assert!(bodies.is_empty());
    }
}
