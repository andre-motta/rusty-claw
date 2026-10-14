//! A stand-in for the simulation: a free camera and the level's placed
//! objects drawn still at their start positions. It ticks at the same fixed
//! 18 ms step and produces the same snapshots the simulation will, so the
//! viewer exercises the real frame driver, interpolation and renderer.

use claw_assets::LevelData;
use claw_frame::{
    Buttons, DrawMode, PaletteFade, Point, Renderable, SimSnapshot, TICK_MS, TickInput, ViewConfig,
};

/// Camera speed in pixels per second.
pub const CAMERA_SPEED: u32 = 500;
/// Camera speed with the fast modifier.
pub const FAST_SPEED: u32 = 2000;
/// Length of the fade key's fade in milliseconds.
pub const FADE_MS: u32 = 250;

/// The free-camera snapshot source.
#[derive(Debug, Clone)]
pub struct Stub {
    tick: u64,
    center: (i64, i64),
    bounds: (i64, i64),
    view: ViewConfig,
    renderables: Vec<Renderable>,
    fade: Option<PaletteFade>,
    dark: bool,
    /// Move at [`FAST_SPEED`].
    pub fast: bool,
    /// Pan along a fixed path instead of following input.
    pub autopilot: bool,
}

impl Stub {
    /// Starts at the level's start point.
    pub fn new(level: &LevelData, view: ViewConfig) -> Stub {
        let (w, h) = level
            .main_plane
            .and_then(|m| level.planes.get(m))
            .map_or((1, 1), |p| {
                (
                    i64::from(p.tile_w) * i64::from(p.w_tiles),
                    i64::from(p.tile_h) * i64::from(p.h_tiles),
                )
            });
        let renderables = level
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| !o.image_set.is_empty())
            .map(|(i, o)| Renderable {
                id: i as u32,
                image_set_key: o.image_set.clone(),
                frame: o.ints[8],
                x: f64::from(o.ints[5]),
                y: f64::from(o.ints[6]),
                z: o.ints[7],
                mirror: o.ints[11] & 2 != 0,
                invert: o.ints[11] & 4 != 0,
                flash: false,
                draw_mode: DrawMode::NORMAL,
                teleported: false,
            })
            .collect();
        Stub {
            tick: 0,
            center: (i64::from(level.start_x), i64::from(level.start_y)),
            bounds: (w.max(1), h.max(1)),
            view,
            renderables,
            fade: None,
            dark: false,
            fast: false,
            autopilot: false,
        }
    }

    /// Moves the camera centre to (x, y).
    pub fn set_center(&mut self, x: i64, y: i64) {
        self.center = (x, y);
        self.clamp();
    }

    /// Changes the view (only its logic size matters here).
    pub fn set_view(&mut self, view: ViewConfig) {
        self.view = view;
    }

    /// Starts a fade to black, or back from black.
    pub fn toggle_fade(&mut self) {
        let (from, to) = if self.dark { (0, 256) } else { (256, 0) };
        self.dark = !self.dark;
        self.fade = Some(PaletteFade {
            from,
            to,
            start_tick: self.tick,
            duration_ms: FADE_MS,
        });
    }

    fn clamp(&mut self) {
        self.center.0 = self.center.0.clamp(0, self.bounds.0 - 1);
        self.center.1 = self.center.1.clamp(0, self.bounds.1 - 1);
    }

    /// The current snapshot.
    pub fn snapshot(&self) -> SimSnapshot {
        let c = Point {
            x: self.center.0 as f64,
            y: self.center.1 as f64,
        };
        SimSnapshot {
            tick: self.tick,
            camera: self.view.camera_for_center(c),
            renderables: self.renderables.clone(),
            palette_fade: self.fade,
            camera_teleported: false,
        }
    }

    /// Advances one tick.
    pub fn step(&mut self, input: TickInput) -> SimSnapshot {
        self.tick += 1;
        let speed = i64::from(if self.fast { FAST_SPEED } else { CAMERA_SPEED });
        // Whole pixels per tick, as the game's camera moves.
        let step = speed * i64::from(TICK_MS) / 1000;
        if self.autopilot {
            // Sweep right and back across the level, drifting vertically.
            let span = (self.bounds.0 / step).max(1);
            let phase = (self.tick as i64 / span) % 2;
            self.center.0 += if phase == 0 { step } else { -step };
            let t = self.tick as f64 * f64::from(TICK_MS) / 1000.0;
            self.center.1 = (self.bounds.1 as f64 * (0.5 + 0.35 * (t * 0.7).sin())) as i64;
        } else {
            let held = input.held;
            if held.contains(Buttons::LEFT) {
                self.center.0 -= step;
            }
            if held.contains(Buttons::RIGHT) {
                self.center.0 += step;
            }
            if held.contains(Buttons::UP) {
                self.center.1 -= step;
            }
            if held.contains(Buttons::DOWN) {
                self.center.1 += step;
            }
        }
        self.clamp();
        if let Some(f) = self.fade {
            let elapsed = (self.tick - f.start_tick) * u64::from(TICK_MS);
            if elapsed > u64::from(f.duration_ms) && f.to == 256 {
                self.fade = None;
            }
        }
        self.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_assets::Plane;
    use claw_assets::wwd::WwdHeader;

    fn level() -> LevelData {
        LevelData {
            level: 1,
            custom_name: None,
            header: WwdHeader::default(),
            palette: Default::default(),
            planes: vec![Plane {
                flags: 1,
                tile_w: 64,
                tile_h: 64,
                w_tiles: 20,
                h_tiles: 10,
                ..Plane::default()
            }],
            main_plane: Some(0),
            objects: Vec::new(),
            tile_attributes: Vec::new(),
            start_x: 100,
            start_y: 200,
            registries: Default::default(),
        }
    }

    #[test]
    fn camera_moves_whole_pixels_per_tick_and_stays_inside() {
        let mut stub = Stub::new(&level(), ViewConfig::default());
        let s = stub.snapshot();
        assert_eq!((s.camera.x, s.camera.y), (100.0 - 320.0, 200.0 - 240.0));
        let right = TickInput {
            held: Buttons::RIGHT,
            pressed: Buttons::NONE,
        };
        let s = stub.step(right);
        assert_eq!(s.tick, 1);
        assert_eq!(s.camera.x, 100.0 + 9.0 - 320.0);
        stub.fast = true;
        for _ in 0..100 {
            stub.step(right);
        }
        let s = stub.snapshot();
        assert_eq!(s.camera.x, 1279.0 - 320.0);
    }

    #[test]
    fn fade_goes_out_and_back() {
        let mut stub = Stub::new(&level(), ViewConfig::default());
        stub.toggle_fade();
        let f = stub.snapshot().palette_fade.unwrap();
        assert_eq!((f.from, f.to, f.duration_ms), (256, 0, FADE_MS));
        for _ in 0..20 {
            stub.step(TickInput::default());
        }
        assert!(stub.snapshot().palette_fade.is_some());
        stub.toggle_fade();
        assert_eq!(stub.snapshot().palette_fade.unwrap().to, 256);
        for _ in 0..20 {
            stub.step(TickInput::default());
        }
        assert!(stub.snapshot().palette_fade.is_none());
    }
}
