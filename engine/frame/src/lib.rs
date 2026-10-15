//! Frame driver and the contract types shared by the renderer and the
//! simulation.
//!
//! The simulation advances in fixed 18 ms ticks and publishes a
//! [`SimSnapshot`] after each one; the renderer draws between the previous and
//! the current snapshot at the display rate. Both sides only meet through the
//! types in this crate, so neither depends on the other.
//!
//! Besides the types, the crate holds the frame driver:
//!
//! - [`step`]: the 18 ms accumulator with its catch-up limit and the
//!   interpolation factor alpha;
//! - [`input`]: per-tick input built from key events, with presses latched so
//!   that none is lost or doubled at any display rate;
//! - [`interp`]: drawing positions between the previous and the current
//!   snapshot, with the snap rules for jumps;
//! - [`refresh`]: a deterministic simulated display clock for tests and
//!   benchmarks;
//! - [`pace`]: a frame-rate cap for drawing that does not wait for the
//!   display's refresh;
//! - [`driver`]: the three combined into one per-frame call.
//!
//! [`output`] holds what a tick hands over besides sprites: the status bar
//! values, sound and music cues, game events and screen transitions.

#![forbid(unsafe_code)]

pub mod driver;
pub mod input;
pub mod interp;
pub mod output;
pub mod pace;
pub mod refresh;
pub mod step;

pub use driver::{FrameDriver, FrameReport};
pub use input::InputLatch;
pub use interp::{InterpolatedFrame, InterpolatedRenderable, interpolate};
pub use output::{
    AudioCue, GameEvent, HudState, MusicTrack, PowerupHud, ScreenTransition, SoundChannel,
    SoundCue, TransitionKind, Weapon,
};
pub use pace::{DEFAULT_FPS_CAP, FramePacer};
pub use refresh::SimulatedRefresh;
pub use step::{Advance, FixedStep, FrameClock};

/// Length of one simulation tick in milliseconds.
pub const TICK_MS: u32 = 18;

/// Default width of the gameplay (logic) view in pixels.
pub const DEFAULT_LOGIC_WIDTH: u32 = 640;

/// Default height of the gameplay (logic) view in pixels.
pub const DEFAULT_LOGIC_HEIGHT: u32 = 480;

/// A set of game buttons, one bit per button.
///
/// The bit values are fixed so that recorded input scripts stay valid across
/// versions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Buttons(pub u32);

impl Buttons {
    /// No button.
    pub const NONE: Buttons = Buttons(0);
    /// Jump.
    pub const JUMP: Buttons = Buttons(1);
    /// Melee attack.
    pub const ATTACK: Buttons = Buttons(1 << 1);
    /// Fire the selected projectile weapon.
    pub const PROJECTILE: Buttons = Buttons(1 << 2);
    /// Cycle the selected projectile weapon.
    pub const TOGGLE: Buttons = Buttons(1 << 3);
    /// Lift or throw.
    pub const LIFT: Buttons = Buttons(1 << 5);
    /// Select the pistol.
    pub const PISTOL: Buttons = Buttons(1 << 6);
    /// Select magic.
    pub const MAGIC: Buttons = Buttons(1 << 7);
    /// Select dynamite.
    pub const DYNAMITE: Buttons = Buttons(1 << 8);
    /// Move left.
    pub const LEFT: Buttons = Buttons(1 << 24);
    /// Move right.
    pub const RIGHT: Buttons = Buttons(1 << 25);
    /// Look up or climb up.
    pub const UP: Buttons = Buttons(1 << 26);
    /// Crouch or climb down.
    pub const DOWN: Buttons = Buttons(1 << 27);

    /// True when every button of `other` is in `self`.
    pub const fn contains(self, other: Buttons) -> bool {
        self.0 & other.0 == other.0
    }

    /// True when no button is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The union of both sets.
    pub const fn union(self, other: Buttons) -> Buttons {
        Buttons(self.0 | other.0)
    }
}

impl std::ops::BitOr for Buttons {
    type Output = Buttons;

    fn bitor(self, rhs: Buttons) -> Buttons {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Buttons) {
        self.0 |= rhs.0;
    }
}

/// Input for one simulation tick.
///
/// `held` is the state of every button during the tick. `pressed` holds the
/// buttons that went down since the previous tick; a press and release that
/// both happen between two ticks still shows up in `pressed` of the next tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TickInput {
    /// Buttons held during this tick.
    pub held: Buttons,
    /// Buttons pressed since the previous tick.
    pub pressed: Buttons,
}

/// A point in level (main plane) pixel coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    /// Horizontal position in pixels.
    pub x: f64,
    /// Vertical position in pixels, growing downwards.
    pub y: f64,
}

/// How a sprite is combined with what is already drawn, by draw-mode
/// number. Mode 0 draws opaque pixels over the destination; the other modes
/// combine source and destination through colour lookup tables.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DrawMode(pub u32);

impl DrawMode {
    /// Opaque pixels replace the destination.
    pub const NORMAL: DrawMode = DrawMode(0);
    /// Also opaque pixels over the destination; the image sets' own default
    /// mode number.
    pub const NORMAL_ALT: DrawMode = DrawMode(1);
    /// Translucent: each pixel becomes the table entry for the pair
    /// (destination, sprite), by default the level's averaging table.
    pub const GHOST: DrawMode = DrawMode(2);
    /// Shadow: the sprite gives only the shape; each covered pixel becomes the
    /// table entry for (destination, colour).
    pub const SHADOW: DrawMode = DrawMode(3);
    /// Recolour: each sprite pixel becomes the table entry for (sprite,
    /// colour). There is no default table, so without one nothing is drawn.
    pub const REMAP: DrawMode = DrawMode(4);
    /// Solid colour in the sprite's shape.
    pub const COLOR_FILL: DrawMode = DrawMode(5);
    /// Blend between destination and sprite in brightness order, by the
    /// colour value (0 shows the destination, 255 the sprite).
    pub const BLEND: DrawMode = DrawMode(6);

    /// The colour value a sprite uses with the shadow, recolour, fill and
    /// blend modes when nothing else is given.
    pub const DEFAULT_COLOR: u8 = 0x80;
}

/// One sprite to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Renderable {
    /// Stable id of the object that owns the sprite, used to pair the
    /// previous and the current snapshot for interpolation.
    pub id: u32,
    /// Image-set registry key, for example `LEVEL_RAT`.
    pub image_set_key: String,
    /// Frame number inside the image set.
    pub frame: i32,
    /// Position of the object in level pixels.
    pub x: f64,
    /// Position of the object in level pixels.
    pub y: f64,
    /// Draw order; larger values draw later.
    pub z: i32,
    /// Mirrored horizontally.
    pub mirror: bool,
    /// Flipped vertically.
    pub invert: bool,
    /// Blinking: the renderer alternates visible and hidden phases of a
    /// fixed length it chooses itself, timed from the frame's simulation
    /// time, so every blinking object blinks in step. The flag carries no
    /// colour and no period of its own.
    pub flash: bool,
    /// How the sprite combines with the destination.
    pub draw_mode: DrawMode,
    /// The object jumped this tick; draw it at the current position instead
    /// of interpolating from the previous one.
    pub teleported: bool,
}

/// A palette fade in progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteFade {
    /// Brightness at the start of the fade, 0 (black) to 256 (unchanged).
    pub from: u32,
    /// Brightness at the end of the fade.
    pub to: u32,
    /// Tick at which the fade started.
    pub start_tick: u64,
    /// Length of the fade in milliseconds.
    pub duration_ms: u32,
}

/// Everything the presentation needs from one simulation tick.
///
/// `audio` and `events` hold only what happened in the tick that produced
/// the snapshot. A frame can run several ticks and the frame driver keeps
/// only the last two snapshots, so they must be collected from every tick
/// as it runs, not from the snapshot being drawn; interpolation ignores
/// them. Every other field describes the state after the tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimSnapshot {
    /// Number of the tick that produced this snapshot.
    pub tick: u64,
    /// Camera position (top-left of the logic view) in level pixels.
    pub camera: Point,
    /// Sprites to draw.
    pub renderables: Vec<Renderable>,
    /// Palette fade in progress, if any.
    pub palette_fade: Option<PaletteFade>,
    /// The camera jumped this tick (for example a snap to a far target or a
    /// level start); the frame is drawn at the current camera instead of
    /// interpolating from the previous one, whatever the distance.
    pub camera_teleported: bool,
    /// Status bar values after the tick.
    pub hud: HudState,
    /// Sound and music cues of this tick only.
    pub audio: Vec<AudioCue>,
    /// Game events of this tick only.
    pub events: Vec<GameEvent>,
    /// Screen transition in progress, if any.
    pub transition: Option<ScreenTransition>,
}

/// How the presentation fills a display whose shape differs from the logic
/// view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AspectPolicy {
    /// Show only the logic view, with bars at the sides.
    Pillarbox,
    /// Fill the display width; objects outside their activity area are drawn
    /// still, and the render camera stays inside the level.
    #[default]
    FrozenEdgeClamped,
    /// As [`AspectPolicy::FrozenEdgeClamped`], but the render camera stays on
    /// the logic view centre and the area beyond the level is filled.
    FrozenEdgeCentered,
    /// Fill the display width and keep every visible object active. This
    /// changes gameplay and is recorded in replays and traces.
    WidenedActivation,
}

/// How the composed image is scaled to the output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScaleMode {
    /// Largest integer factor with nearest sampling, then the remaining
    /// factor with linear filtering, filling the display height.
    #[default]
    Fit,
    /// Largest integer factor only; the rest is letterboxed.
    Integer,
}

/// The gameplay view and the output it is presented on.
///
/// The simulation reads only `logic_width` and `logic_height` (and the
/// aspect policy when it widens activity); everything else is presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewConfig {
    /// Width of the gameplay view in level pixels.
    pub logic_width: u32,
    /// Height of the gameplay view in level pixels.
    pub logic_height: u32,
    /// Output width in physical pixels.
    pub render_width: u32,
    /// Output height in physical pixels.
    pub render_height: u32,
    /// How a display of another shape is filled.
    pub aspect_policy: AspectPolicy,
    /// How the composed image is scaled.
    pub scale_mode: ScaleMode,
}

impl ViewConfig {
    /// The same view presented on an output of `width` x `height` pixels.
    pub fn with_render_size(mut self, width: u32, height: u32) -> ViewConfig {
        self.render_width = width.max(1);
        self.render_height = height.max(1);
        self
    }

    /// Size in level pixels of the area that is drawn: the logic view for
    /// [`AspectPolicy::Pillarbox`], otherwise the logic height and the width
    /// that fills the output's shape, rounded up to an even number and never
    /// narrower than the logic view.
    pub fn presentation_size(&self) -> (u32, u32) {
        let lw = self.logic_width.max(1);
        let lh = self.logic_height.max(1);
        if self.aspect_policy == AspectPolicy::Pillarbox {
            return (lw, lh);
        }
        let rw = u64::from(self.render_width.max(1));
        let rh = u64::from(self.render_height.max(1));
        let wide = (u64::from(lh) * rw).div_ceil(rh);
        let wide = wide + (wide & 1);
        let wide = u32::try_from(wide).unwrap_or(!1);
        (wide.max(lw), lh)
    }

    /// How far, in level pixels, every activity area reaches beyond its
    /// normal half size on each side (horizontal, vertical).
    ///
    /// Non-zero only for [`AspectPolicy::WidenedActivation`], where it is the
    /// presentation's extra half size over the logic view, so that what is on
    /// screen keeps the same margin to the area edges as in the logic view.
    ///
    /// This is the single source of the widened window size: the widened
    /// activation view is `logic + 2 * margin` on each axis. Take it from
    /// here rather than recomputing it from the render size, because the
    /// presentation width is rounded up to an even number (at 5120x1440 the
    /// margin is 534 px, not 533), and a recomputed value would let
    /// activation and drawing differ by a pixel.
    pub fn activation_margin(&self) -> (u32, u32) {
        if self.aspect_policy != AspectPolicy::WidenedActivation {
            return (0, 0);
        }
        let (pw, ph) = self.presentation_size();
        (
            pw.saturating_sub(self.logic_width) / 2,
            ph.saturating_sub(self.logic_height) / 2,
        )
    }

    /// The centre of the logic view for a snapshot camera, which names the
    /// view's top-left corner.
    pub fn logic_center(&self, camera: Point) -> Point {
        Point {
            x: camera.x + f64::from(self.logic_width / 2),
            y: camera.y + f64::from(self.logic_height / 2),
        }
    }

    /// The snapshot camera (top-left of the logic view) whose view is centred
    /// on `center`.
    pub fn camera_for_center(&self, center: Point) -> Point {
        Point {
            x: center.x - f64::from(self.logic_width / 2),
            y: center.y - f64::from(self.logic_height / 2),
        }
    }
}

impl Default for ViewConfig {
    fn default() -> Self {
        ViewConfig {
            logic_width: DEFAULT_LOGIC_WIDTH,
            logic_height: DEFAULT_LOGIC_HEIGHT,
            render_width: DEFAULT_LOGIC_WIDTH,
            render_height: DEFAULT_LOGIC_HEIGHT,
            aspect_policy: AspectPolicy::default(),
            scale_mode: ScaleMode::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_bits_are_stable() {
        assert_eq!(Buttons::JUMP.0, 0x1);
        assert_eq!(Buttons::ATTACK.0, 0x2);
        assert_eq!(Buttons::PROJECTILE.0, 0x4);
        assert_eq!(Buttons::TOGGLE.0, 0x8);
        assert_eq!(Buttons::LIFT.0, 0x20);
        assert_eq!(Buttons::PISTOL.0, 0x40);
        assert_eq!(Buttons::MAGIC.0, 0x80);
        assert_eq!(Buttons::DYNAMITE.0, 0x100);
        assert_eq!(Buttons::LEFT.0, 1 << 24);
        assert_eq!(Buttons::RIGHT.0, 1 << 25);
        assert_eq!(Buttons::UP.0, 1 << 26);
        assert_eq!(Buttons::DOWN.0, 1 << 27);
    }

    #[test]
    fn buttons_combine() {
        let both = Buttons::LEFT | Buttons::JUMP;
        assert!(both.contains(Buttons::LEFT));
        assert!(both.contains(Buttons::JUMP));
        assert!(!both.contains(Buttons::RIGHT));
        assert!(Buttons::NONE.is_empty());
    }

    #[test]
    fn presentation_follows_the_output_shape() {
        let base = ViewConfig::default();
        let cases = [
            ((640, 480), (640, 480)),
            ((1920, 1200), (768, 480)),
            ((1920, 1080), (854, 480)),
            ((3840, 2160), (854, 480)),
            ((2560, 1080), (1138, 480)),
            ((3440, 1440), (1148, 480)),
            ((5120, 1440), (1708, 480)),
            ((1280, 800), (768, 480)),
            ((480, 640), (640, 480)),
        ];
        for ((rw, rh), want) in cases {
            let view = base.with_render_size(rw, rh);
            assert_eq!(view.presentation_size(), want, "{rw}x{rh}");
            let pillar = ViewConfig {
                aspect_policy: AspectPolicy::Pillarbox,
                ..view
            };
            assert_eq!(pillar.presentation_size(), (640, 480));
        }
    }

    #[test]
    fn activation_margin_only_when_widened() {
        let view = ViewConfig::default().with_render_size(5120, 1440);
        assert_eq!(view.activation_margin(), (0, 0));
        let wide = ViewConfig {
            aspect_policy: AspectPolicy::WidenedActivation,
            ..view
        };
        assert_eq!(wide.activation_margin(), (534, 0));
        let wide_169 = wide.with_render_size(1920, 1080);
        assert_eq!(wide_169.activation_margin(), (107, 0));
    }

    #[test]
    fn camera_and_center_round_trip() {
        let view = ViewConfig::default();
        let center = view.logic_center(Point { x: 10.0, y: 20.0 });
        assert_eq!(center, Point { x: 330.0, y: 260.0 });
        assert_eq!(view.camera_for_center(center), Point { x: 10.0, y: 20.0 });
    }

    #[test]
    fn view_defaults_to_the_original_view() {
        let view = ViewConfig::default();
        assert_eq!((view.logic_width, view.logic_height), (640, 480));
        assert_eq!(view.aspect_policy, AspectPolicy::FrozenEdgeClamped);
        assert_eq!(view.scale_mode, ScaleMode::Fit);
    }
}
