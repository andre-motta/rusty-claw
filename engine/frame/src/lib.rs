//! Frame driver and the contract types shared by the renderer and the
//! simulation.
//!
//! The simulation advances in fixed 18 ms ticks and publishes a
//! [`SimSnapshot`] after each one; the renderer draws between the previous and
//! the current snapshot at the display rate. Both sides only meet through the
//! types in this crate, so neither depends on the other.
//!
//! This change defines the types only. The tick accumulator and the
//! interpolation helpers land with the renderer.

#![forbid(unsafe_code)]

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
    /// Drawn as a solid flash colour this frame.
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

/// Everything the renderer needs from one simulation tick.
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
    fn view_defaults_to_the_original_view() {
        let view = ViewConfig::default();
        assert_eq!((view.logic_width, view.logic_height), (640, 480));
        assert_eq!(view.aspect_policy, AspectPolicy::FrozenEdgeClamped);
        assert_eq!(view.scale_mode, ScaleMode::Fit);
    }
}
