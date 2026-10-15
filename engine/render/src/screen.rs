//! The screen layer: sprites pinned to the edges of the picture, standalone
//! screens drawn without a level, and screen transitions.
//!
//! **Screen sprites** ([`ScreenSprite`]) are drawn after the world into the
//! same palette-index picture, so they use the level palette, the draw
//! modes and the tables, and fades and transition masks act on them like on
//! the rest of the picture. Their positions are whole game pixels relative to
//! an [`Anchor`] of the presentation rectangle, the part of the output that
//! shows the game picture (never the bars). Its edges follow the game's
//! inclusive play-area rectangle: left 0, top 0, right `width - 1`, bottom
//! `height - 1`, and its centre is `left + (right - left) / 2` (rounded
//! toward zero) on each axis. A position names the sprite's hotspot: the
//! frame's box starts at `origin + (x, y) - (w >> 1, h >> 1) + (offset_x,
//! offset_y)`, as for world sprites. Screen sprites are drawn at the
//! composition scale and never interpolated, so they stay crisp and still.
//!
//! **Screens** ([`ScreenScene`]) are 640x480 pictures with their own
//! palette: a background image and screen sprites, scaled like the game
//! picture (the same whole factor and final scaling) with bars at the sides,
//! never stretched. Menus and the end-of-level tally use them.
//!
//! **Transitions** ([`ScreenTransition`]) come from the simulation:
//!
//! - a circle closes on a point (everything outside it turns to palette
//!   index 0, black in every level) or opens from it, with the radius
//!   stepping linearly in time from or to the largest distance between the
//!   centre and the picture's corners;
//! - before an opening circle starts and after a closing one ends, the whole
//!   picture is index 0;
//! - a closing circle's centre is pulled inside the picture (a coordinate at
//!   or past the far edge becomes `edge - 32`, one at or before 0 becomes
//!   32); an opening circle centred outside the picture is skipped and the
//!   picture shows at once;
//! - a fade dims the palette to black (closing) or brings it back from
//!   black (opening) with the game's fade rule; before an opening fade starts
//!   the palette is black.
//!
//! The circle test is done per composition-target pixel at the centre of
//! the pixel, which at scale 1 is exactly the game's pixel rule.

use claw_assets::Palette;
use claw_frame::{AspectPolicy, DEFAULT_LOGIC_HEIGHT, DEFAULT_LOGIC_WIDTH};
use claw_frame::{DrawMode, PaletteFade, ScreenTransition, TICK_MS, TransitionKind, ViewConfig};

use crate::effects::{Effect, Table};
use crate::images::{IMAGE_GHOST, LevelGraphics};
use crate::palette::{self, FULL_BRIGHTNESS};
use crate::scene::{Composition, Item, Scene, SpriteParams};

/// Id of an image in [`LevelGraphics::images`], for example a screen's
/// background added with [`LevelGraphics::add_image`].
pub type ImageId = u32;

/// Size of a standalone screen in game pixels.
pub const SCREEN_SIZE: (u32, u32) = (DEFAULT_LOGIC_WIDTH, DEFAULT_LOGIC_HEIGHT);

/// How far inside the picture a closing circle's centre is pulled when it
/// lies on or past an edge, in game pixels.
pub const CIRCLE_EDGE_INSET: i32 = 32;

/// The point of the presentation rectangle a screen sprite is placed from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// Left edge, top edge.
    #[default]
    TopLeft,
    /// Right edge, top edge.
    TopRight,
    /// Left edge, bottom edge.
    BottomLeft,
    /// Right edge, bottom edge.
    BottomRight,
    /// Centre column, top edge.
    TopCenter,
    /// Centre column, centre row.
    Center,
}

impl Anchor {
    /// The anchor point in a presentation of `width` x `height` game
    /// pixels.
    pub fn origin(self, presentation: (u32, u32)) -> (i32, i32) {
        let right = presentation.0.max(1) as i32 - 1;
        let bottom = presentation.1.max(1) as i32 - 1;
        let cx = right / 2;
        let cy = bottom / 2;
        match self {
            Anchor::TopLeft => (0, 0),
            Anchor::TopRight => (right, 0),
            Anchor::BottomLeft => (0, bottom),
            Anchor::BottomRight => (right, bottom),
            Anchor::TopCenter => (cx, 0),
            Anchor::Center => (cx, cy),
        }
    }
}

/// One sprite of the screen layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenSprite {
    /// Image set key.
    pub key: String,
    /// Frame number; -1 for the set's first frame.
    pub frame: i32,
    /// Hotspot position from the anchor, in game pixels.
    pub x: i32,
    /// Hotspot position from the anchor, in game pixels.
    pub y: i32,
    /// The point the position is measured from.
    pub anchor: Anchor,
    /// Draw mode.
    pub mode: DrawMode,
}

impl ScreenSprite {
    /// A sprite drawn normally at (x, y) from `anchor`.
    pub fn new(key: &str, frame: i32, anchor: Anchor, x: i32, y: i32) -> ScreenSprite {
        ScreenSprite {
            key: key.to_string(),
            frame,
            x,
            y,
            anchor,
            mode: DrawMode::NORMAL,
        }
    }
}

/// A standalone 640x480 screen: a background, sprites and a palette of its
/// own.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenScene {
    /// Background image, centred on the screen, drawn first.
    pub background: Option<ImageId>,
    /// The screen's palette.
    pub palette: Palette,
    /// Sprites, anchored to the 640x480 screen, drawn in order.
    pub sprites: Vec<ScreenSprite>,
    /// Palette fade in progress.
    pub fade: Option<PaletteFade>,
}

/// The view a screen is drawn with on the output of `view`: the 640x480
/// screen with bars at the sides, scaled like the game picture.
pub fn screen_view(view: &ViewConfig) -> ViewConfig {
    ViewConfig {
        logic_width: SCREEN_SIZE.0,
        logic_height: SCREEN_SIZE.1,
        aspect_policy: AspectPolicy::Pillarbox,
        ..*view
    }
}

/// The draw parameters of a screen sprite in a composition of whole scale
/// `scale` and presentation `presentation`, or `None` when it draws nothing
/// (unknown set or frame, a draw mode that draws nothing, an empty frame).
pub fn sprite_params(
    graphics: &mut LevelGraphics,
    sprite: &ScreenSprite,
    presentation: (u32, u32),
    scale: u32,
) -> Option<SpriteParams> {
    let mut effect = Effect::from_mode(sprite.mode.0)?;
    let id = graphics.sprite(&sprite.key, sprite.frame)?;
    let rec = *graphics.images.record(id);
    if rec.width == 0 || rec.height == 0 {
        return None;
    }
    let meta = graphics.meta(id);
    if effect == Effect::Normal && rec.flags & IMAGE_GHOST != 0 {
        effect = Effect::Ghost;
    }
    let table = effect.default_table();
    if effect == Effect::Remap && table == Table::None {
        return None;
    }
    let (ox, oy) = sprite.anchor.origin(presentation);
    let left =
        i64::from(ox) + i64::from(sprite.x) - i64::from(rec.width >> 1) + i64::from(meta.offset_x);
    let top =
        i64::from(oy) + i64::from(sprite.y) - i64::from(rec.height >> 1) + i64::from(meta.offset_y);
    let k = i64::from(scale.max(1));
    let clamp = |v: i64| v.clamp(i64::from(i32::MIN / 2), i64::from(i32::MAX / 2)) as i32;
    Some(SpriteParams {
        pos: [clamp(left * k), clamp(top * k)],
        size: [rec.width, rec.height],
        image: id,
        flags: 0,
        effect: effect as u32,
        color: u32::from(DrawMode::DEFAULT_COLOR),
        table: table.offset(),
        scale: scale.max(1),
        pad: [0; 2],
    })
}

/// A transition mask over the composed picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mask {
    /// The whole picture is palette index 0.
    Black,
    /// Only the pixels inside the circle show; the rest is index 0. A
    /// target pixel (x, y) is inside when `(2x + 1 - cx2)^2 + (2y + 1 -
    /// cy2)^2 <= r2`: the centre and radius are in half target pixels and
    /// `r2` is the squared radius.
    Circle {
        /// Centre column, in half target pixels.
        cx2: i32,
        /// Centre row, in half target pixels.
        cy2: i32,
        /// Squared radius, in half target pixels.
        r2: i32,
    },
}

impl Mask {
    /// True when target pixel (x, y) shows the picture.
    pub fn shows(&self, x: i64, y: i64) -> bool {
        match *self {
            Mask::Black => false,
            Mask::Circle { cx2, cy2, r2 } => {
                let dx = 2 * x + 1 - i64::from(cx2);
                let dy = 2 * y + 1 - i64::from(cy2);
                dx * dx + dy * dy <= i64::from(r2)
            }
        }
    }

    /// The mode number and parameters the shaders read: (cx2, cy2, r2,
    /// mode), with mode 0 for no mask, 1 for black and 2 for a circle.
    pub fn uniform(mask: Option<&Mask>) -> [i32; 4] {
        match mask {
            None => [0; 4],
            Some(Mask::Black) => [0, 0, 0, 1],
            Some(Mask::Circle { cx2, cy2, r2 }) => [*cx2, *cy2, *r2, 2],
        }
    }
}

/// Milliseconds since `t` started at simulation time `time_ms` (negative
/// before it starts).
pub fn elapsed_ms(t: &ScreenTransition, time_ms: f64) -> f64 {
    let start = t.start_tick as f64 * f64::from(TICK_MS);
    let e = time_ms - start;
    if e.is_finite() { e } else { 0.0 }
}

/// The step count of a circle centred on art pixel (cx, cy) in a
/// presentation of `width` x `height`: the largest distance from the centre
/// to the four corners, rounded down.
pub fn circle_steps(cx: i32, cy: i32, presentation: (u32, u32)) -> i32 {
    let (w, h) = (i64::from(presentation.0), i64::from(presentation.1));
    let (cx, cy) = (i64::from(cx), i64::from(cy));
    let far = [(0, 0), (w, 0), (0, h), (w, h)]
        .iter()
        .map(|&(x, y)| (x - cx) * (x - cx) + (y - cy) * (y - cy))
        .max()
        .unwrap_or(0);
    isqrt(far) as i32
}

fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut r = (v as f64).sqrt() as i64;
    while r * r > v {
        r -= 1;
    }
    while (r + 1) * (r + 1) <= v {
        r += 1;
    }
    r
}

/// The radius in art pixels of a circle of `steps` steps at `elapsed` ms
/// into a transition of `duration_ms`: growing from 0 when opening,
/// shrinking to 0 when closing.
pub fn circle_radius(steps: i32, elapsed: f64, duration_ms: u32, closing: bool) -> i32 {
    let d = i64::from(duration_ms.max(1));
    let t = (elapsed.max(0.0).floor() as i64).min(d);
    let grown = (i64::from(steps) * t / d) as i32;
    if closing { steps - grown } else { grown }
}

/// The circle centre in whole art pixels for a centre at `p` in the
/// presentation (fractional art pixels), or `None` when an opening circle
/// is skipped because its centre lies outside the picture.
pub fn circle_center(p: (f64, f64), presentation: (u32, u32), closing: bool) -> Option<(i32, i32)> {
    let to_i = |v: f64| {
        if v.is_finite() {
            v.floor()
                .clamp(f64::from(i32::MIN / 2), f64::from(i32::MAX / 2)) as i32
        } else {
            0
        }
    };
    let (mut x, mut y) = (to_i(p.0), to_i(p.1));
    let (w, h) = (presentation.0 as i32, presentation.1 as i32);
    if closing {
        let pull = |v: i32, size: i32| {
            if v >= size {
                size - CIRCLE_EDGE_INSET
            } else if v <= 0 {
                CIRCLE_EDGE_INSET
            } else {
                v
            }
        };
        x = pull(x, w);
        y = pull(y, h);
        Some((x, y))
    } else if x < 0 || y < 0 || x >= w || y >= h {
        None
    } else {
        Some((x, y))
    }
}

/// What a transition does to one frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TransitionEffect {
    /// Mask over the composed picture.
    pub mask: Option<Mask>,
    /// Palette fade from the transition, if it fades.
    pub fade: Option<PaletteFade>,
    /// The palette is black whatever else applies.
    pub black: bool,
}

/// The effect of `t` on a frame at simulation time `time_ms`, with the
/// circle centre `center` given in presentation art pixels, for a
/// composition of `presentation` and whole scale `scale`.
pub fn transition_effect(
    t: &ScreenTransition,
    time_ms: f64,
    center: (f64, f64),
    presentation: (u32, u32),
    scale: u32,
) -> TransitionEffect {
    let e = elapsed_ms(t, time_ms);
    let done = e >= f64::from(t.duration_ms);
    match t.kind {
        TransitionKind::Fade => {
            if t.closing {
                if e < 0.0 {
                    return TransitionEffect::default();
                }
                TransitionEffect {
                    fade: Some(PaletteFade {
                        from: FULL_BRIGHTNESS,
                        to: 0,
                        start_tick: t.start_tick,
                        duration_ms: t.duration_ms,
                    }),
                    black: done,
                    ..TransitionEffect::default()
                }
            } else if e < 0.0 {
                TransitionEffect {
                    black: true,
                    ..TransitionEffect::default()
                }
            } else if done {
                TransitionEffect::default()
            } else {
                TransitionEffect {
                    fade: Some(PaletteFade {
                        from: 0,
                        to: FULL_BRIGHTNESS,
                        start_tick: t.start_tick,
                        duration_ms: t.duration_ms,
                    }),
                    ..TransitionEffect::default()
                }
            }
        }
        TransitionKind::Circle => {
            let Some((cx, cy)) = circle_center(center, presentation, t.closing) else {
                return TransitionEffect::default();
            };
            let mask = if t.closing {
                if e < 0.0 {
                    None
                } else if done {
                    Some(Mask::Black)
                } else {
                    let steps = circle_steps(cx, cy, presentation);
                    Some(circle_mask(
                        cx,
                        cy,
                        circle_radius(steps, e, t.duration_ms, true),
                        scale,
                    ))
                }
            } else if e < 0.0 {
                Some(Mask::Black)
            } else if done {
                None
            } else {
                let steps = circle_steps(cx, cy, presentation);
                Some(circle_mask(
                    cx,
                    cy,
                    circle_radius(steps, e, t.duration_ms, false),
                    scale,
                ))
            };
            TransitionEffect {
                mask,
                ..TransitionEffect::default()
            }
        }
    }
}

/// The mask of a circle of `radius` art pixels centred on art pixel (cx,
/// cy), at whole scale `scale`.
pub fn circle_mask(cx: i32, cy: i32, radius: i32, scale: u32) -> Mask {
    let k = i64::from(scale.max(1));
    let r = 2 * k * i64::from(radius.max(0));
    Mask::Circle {
        cx2: (2 * k * i64::from(cx) + k).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        cy2: (2 * k * i64::from(cy) + k).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        r2: (r * r).min(1 << 30) as i32,
    }
}

/// The palette and brightness of a frame: `base` under `fade`, then under
/// the transition's own fade or black.
pub fn frame_palette(
    base: &Palette,
    fade: Option<&PaletteFade>,
    effect: &TransitionEffect,
    time_ms: f64,
) -> (Palette, f32) {
    if effect.black {
        return (Palette::default(), 0.0);
    }
    let mut pal = palette::faded(base, fade, time_ms);
    let mut bright = palette::brightness(fade, time_ms);
    if let Some(f) = effect.fade.as_ref() {
        pal = palette::faded(&pal, Some(f), time_ms);
        bright *= palette::brightness(Some(f), time_ms);
    }
    (pal, bright)
}

/// Builds the scenes of standalone screens for one output.
#[derive(Debug, Clone)]
pub struct ScreenBuilder {
    comp: Composition,
}

impl ScreenBuilder {
    /// A builder for screens shown on the output of `view` (its render size
    /// and scale mode), with targets no larger than `max_target`.
    pub fn new(view: &ViewConfig, max_target: u32) -> ScreenBuilder {
        ScreenBuilder {
            comp: Composition::new(&screen_view(view), max_target),
        }
    }

    /// The composition geometry.
    pub fn composition(&self) -> &Composition {
        &self.comp
    }

    /// Builds the scene of `screen` at time `time_ms` (for its fade), with
    /// images from `graphics`.
    pub fn build(&self, graphics: &mut LevelGraphics, screen: &ScreenScene, time_ms: f64) -> Scene {
        let k = self.comp.scale;
        let mut sprites = Vec::new();
        if let Some(id) = screen.background
            && (id as usize) < graphics.images.records.len()
        {
            let rec = *graphics.images.record(id);
            if rec.width > 0 && rec.height > 0 {
                let left = (i64::from(SCREEN_SIZE.0) - i64::from(rec.width)) / 2;
                let top = (i64::from(SCREEN_SIZE.1) - i64::from(rec.height)) / 2;
                sprites.push(SpriteParams {
                    pos: [(left * i64::from(k)) as i32, (top * i64::from(k)) as i32],
                    size: [rec.width, rec.height],
                    image: id,
                    flags: 0,
                    effect: Effect::Normal as u32,
                    color: u32::from(DrawMode::DEFAULT_COLOR),
                    table: Table::None.offset(),
                    scale: k,
                    pad: [0; 2],
                });
            }
        }
        for s in &screen.sprites {
            if let Some(p) = sprite_params(graphics, s, self.comp.presentation, k) {
                sprites.push(p);
            }
        }
        let items = (0..sprites.len() as u32).map(Item::Sprite).collect();
        let (palette, brightness) = frame_palette(
            &screen.palette,
            screen.fade.as_ref(),
            &TransitionEffect::default(),
            time_ms,
        );
        Scene {
            comp: self.comp,
            planes: Vec::new(),
            sprites,
            items,
            palette,
            brightness,
            mask: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claw_frame::Point;

    #[test]
    fn anchors_follow_the_inclusive_rectangle() {
        let p = (640, 480);
        assert_eq!(Anchor::TopLeft.origin(p), (0, 0));
        assert_eq!(Anchor::TopRight.origin(p), (639, 0));
        assert_eq!(Anchor::BottomRight.origin(p), (639, 479));
        assert_eq!(Anchor::BottomLeft.origin(p), (0, 479));
        assert_eq!(Anchor::TopCenter.origin(p), (319, 0));
        assert_eq!(Anchor::Center.origin(p), (319, 239));
        assert_eq!(Anchor::TopRight.origin((1708, 480)), (1707, 0));
    }

    #[test]
    fn circle_steps_reach_the_far_corner() {
        assert_eq!(circle_steps(320, 240, (640, 480)), 400);
        assert_eq!(circle_steps(0, 0, (640, 480)), 800);
        assert_eq!(circle_steps(10, 10, (640, 480)), 786);
        assert_eq!(circle_radius(400, 0.0, 1250, true), 400);
        assert_eq!(circle_radius(400, 625.0, 1250, true), 200);
        assert_eq!(circle_radius(400, 1250.0, 1250, true), 0);
        assert_eq!(circle_radius(400, 500.9, 1000, false), 200);
    }

    #[test]
    fn closing_centres_are_pulled_in_and_opening_ones_may_be_skipped() {
        let p = (640, 480);
        assert_eq!(circle_center((700.0, -3.0), p, true), Some((608, 32)));
        assert_eq!(circle_center((0.0, 480.0), p, true), Some((32, 448)));
        assert_eq!(circle_center((5.5, 479.9), p, true), Some((5, 479)));
        assert_eq!(circle_center((700.0, 10.0), p, false), None);
        assert_eq!(circle_center((-0.5, 10.0), p, false), None);
        assert_eq!(circle_center((0.0, 0.0), p, false), Some((0, 0)));
    }

    #[test]
    fn circle_masks_at_scale_one_are_the_pixel_rule() {
        let m = circle_mask(10, 20, 3, 1);
        for y in 10..30 {
            for x in 0..20 {
                let (dx, dy) = (x - 10i64, y - 20i64);
                assert_eq!(m.shows(x, y), dx * dx + dy * dy <= 9, "({x}, {y})");
            }
        }
        // At scale 3 the art pixel (10, 20) is target 30..33 x 60..63.
        let m = circle_mask(10, 20, 0, 3);
        assert!(m.shows(31, 61));
        assert!(!m.shows(30, 60));
        assert!(!Mask::Black.shows(0, 0));
    }

    #[test]
    fn transition_phases() {
        let p = (640, 480);
        let t = ScreenTransition {
            kind: TransitionKind::Circle,
            closing: true,
            start_tick: 100,
            duration_ms: 1250,
            center: Point::default(),
        };
        let at =
            |t: &ScreenTransition, ms: f64| transition_effect(t, 1800.0 + ms, (320.0, 240.0), p, 1);
        assert_eq!(at(&t, -1.0).mask, None);
        assert_eq!(at(&t, 0.0).mask, Some(circle_mask(320, 240, 400, 1)));
        assert_eq!(at(&t, 1250.0).mask, Some(Mask::Black));
        let open = ScreenTransition {
            closing: false,
            duration_ms: 1000,
            ..t
        };
        assert_eq!(at(&open, -100.0).mask, Some(Mask::Black));
        assert_eq!(at(&open, 500.0).mask, Some(circle_mask(320, 240, 200, 1)));
        assert_eq!(at(&open, 1000.0).mask, None);
        let fade = ScreenTransition {
            kind: TransitionKind::Fade,
            closing: false,
            duration_ms: 1500,
            ..t
        };
        assert!(at(&fade, -1.0).black);
        assert!(at(&fade, 10.0).fade.is_some());
        assert_eq!(at(&fade, 1500.0), TransitionEffect::default());
        let out = ScreenTransition {
            closing: true,
            ..fade
        };
        assert_eq!(at(&out, -1.0), TransitionEffect::default());
        assert!(at(&out, 1500.0).black);
    }
}
