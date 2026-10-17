//! One frame as an ordered list of draws in composition-target pixels.
//!
//! The frame is composed as palette indices into a target that is the
//! presentation view (see [`ViewConfig::presentation_size`]) times a whole
//! scale factor `k`, then scaled to the output by the final pass. Composing
//! at `k` times the game's resolution lets the camera and objects move by
//! whole output pixels between game pixels, so interpolated motion is smooth
//! at high refresh rates, while every pixel is still an exact palette index
//! for the table effects and fades.
//!
//! Positions become target pixels as `round(position * k) - round(view * k)`
//! for planes and sprites alike, so an object resting on the main plane
//! moves with it exactly.
//!
//! The scene is the single source of truth for the GPU path and for the CPU
//! reference compositor in [`crate::cpu`].
//!
//! The front layer switch ([`SceneBuilder::set_front_layer`]) leaves the
//! level's front plane out of the scene (see [`crate::view::front_plane`]);
//! every object still draws, whatever its depth. It only changes drawing.

use bytemuck::{Pod, Zeroable};
use claw_assets::Palette;
use claw_frame::{
    AspectPolicy, DrawMode, InterpolatedFrame, PaletteFade, ScaleMode, ScreenTransition, ViewConfig,
};

use crate::effects::{Effect, Table};
use crate::images::{IMAGE_GHOST, IMAGE_RLE, LevelGraphics};
use crate::screen::{self, Mask, ScreenSprite};
use crate::view::{PLANE_X_WRAP, PLANE_Y_WRAP, Rect, WorldView, front_plane};

/// Default largest composition target width or height.
pub const DEFAULT_MAX_TARGET: u32 = 8192;

/// How long each visible and hidden phase of a flashing sprite lasts, in
/// milliseconds. Objects set their own period in the game; this is the one
/// a hit uses, and the renderer uses it for every blinking sprite. Phases
/// run from simulation time 0, so all blinking sprites blink in step.
pub const FLASH_PERIOD_MS: f64 = 33.0;

/// Sprite flag: mirrored horizontally.
pub const SPRITE_MIRROR: u32 = 0x1;
/// Sprite flag: flipped vertically.
pub const SPRITE_INVERT: u32 = 0x2;

/// Where the composed image lands on the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Composition {
    /// Presentation view in game pixels.
    pub presentation: (u32, u32),
    /// Whole scale of the composition target.
    pub scale: u32,
    /// Composition target size in pixels.
    pub target: (u32, u32),
    /// Output size in pixels.
    pub output: (u32, u32),
    /// Left edge of the scaled image on the output (may be negative when the
    /// image is cropped by less than one game pixel).
    pub x: i32,
    /// Top edge of the scaled image on the output.
    pub y: i32,
    /// Width of the scaled image on the output.
    pub width: u32,
    /// Height of the scaled image on the output.
    pub height: u32,
    /// Output pixels per target pixel (1 for integer scaling).
    pub ratio: f32,
}

impl Composition {
    /// The composition for `view`, with targets no larger than `max_target`.
    ///
    /// Policies that fill the output's shape scale by the output height
    /// (the presentation width was rounded up to an even number, so the
    /// image may overhang the sides by less than one game pixel); pillarbox
    /// fits the whole logic view. `fit` fills by that factor with a whole
    /// factor `k` in the target and the rest in the final pass; `integer`
    /// uses `k` alone and leaves bars.
    pub fn new(view: &ViewConfig, max_target: u32) -> Composition {
        let (pw, ph) = view.presentation_size();
        let (rw, rh) = (view.render_width.max(1), view.render_height.max(1));
        let s = if view.aspect_policy == AspectPolicy::Pillarbox {
            (f64::from(rw) / f64::from(pw)).min(f64::from(rh) / f64::from(ph))
        } else {
            f64::from(rh) / f64::from(ph)
        };
        let max_k = (max_target / pw.max(ph)).max(1);
        let k = (s.floor() as u32).clamp(1, max_k);
        let target = (pw * k, ph * k);
        let (width, height, ratio) = if view.scale_mode == ScaleMode::Integer && s >= 1.0 {
            (target.0, target.1, 1.0)
        } else {
            let w = (f64::from(pw) * s).round().max(1.0) as u32;
            let h = (f64::from(ph) * s).round().max(1.0) as u32;
            (w, h, (s / f64::from(k)) as f32)
        };
        let x = (i64::from(rw) - i64::from(width)).div_euclid(2) as i32;
        let y = (i64::from(rh) - i64::from(height)).div_euclid(2) as i32;
        Composition {
            presentation: (pw, ph),
            scale: k,
            target,
            output: (rw, rh),
            x,
            y,
            width,
            height,
            ratio,
        }
    }
}

/// One plane draw as the shaders see it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct PlaneParams {
    /// Target pixel of plane pixel (0, 0) is `-origin`: plane pixel of a
    /// target pixel `p` is `floor((p + origin) / scale)`.
    pub origin: [i32; 2],
    /// Plane size in pixels.
    pub size: [i32; 2],
    /// Tile size in pixels.
    pub tile: [i32; 2],
    /// Grid size in tiles.
    pub grid: [i32; 2],
    /// Offset of the plane's first slot in the concatenated slot array.
    pub slots: u32,
    /// Plane flags (wrap bits).
    pub flags: u32,
    /// Fill colour index.
    pub fill: u32,
    /// Composition scale.
    pub scale: u32,
}

/// One sprite draw as the shaders see it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct SpriteParams {
    /// Target pixel of the quad's top-left corner.
    pub pos: [i32; 2],
    /// Image size in game pixels; the quad is `size * scale` target pixels.
    pub size: [u32; 2],
    /// Image id.
    pub image: u32,
    /// `SPRITE_*` flags.
    pub flags: u32,
    /// [`Effect`] number.
    pub effect: u32,
    /// Colour value for the shadow, remap, fill and blend effects.
    pub color: u32,
    /// Byte offset of the table in the packed tables.
    pub table: u32,
    /// Composition scale.
    pub scale: u32,
    /// Unused.
    pub pad: [u32; 2],
}

impl SpriteParams {
    /// The quad in target pixels: left, top, right and bottom (exclusive).
    pub fn quad(&self) -> (i64, i64, i64, i64) {
        let x0 = i64::from(self.pos[0]);
        let y0 = i64::from(self.pos[1]);
        let s = i64::from(self.scale);
        (
            x0,
            y0,
            x0 + i64::from(self.size[0]) * s,
            y0 + i64::from(self.size[1]) * s,
        )
    }

    /// The effect of this sprite.
    pub fn effect(&self) -> Effect {
        Effect::from_mode(self.effect).unwrap_or(Effect::Normal)
    }
}

/// One step of the frame, in drawing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// Draw the plane with this index.
    Plane(u32),
    /// Draw the sprite with this index into [`Scene::sprites`].
    Sprite(u32),
}

/// One sprite to draw, in level pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInput<'a> {
    /// Id of the owning object (ties in z are drawn by id).
    pub id: u32,
    /// Image set key.
    pub key: &'a str,
    /// Frame number; -1 for the set's first frame.
    pub frame: i32,
    /// Object position.
    pub x: f64,
    /// Object position.
    pub y: f64,
    /// Draw order.
    pub z: i32,
    /// Mirrored horizontally.
    pub mirror: bool,
    /// Flipped vertically.
    pub invert: bool,
    /// Blinking.
    pub flash: bool,
    /// Draw mode.
    pub mode: DrawMode,
    /// Colour value; [`DrawMode::DEFAULT_COLOR`] when `None`.
    pub color: Option<u8>,
    /// Table; the mode's default when `None`.
    pub table: Option<Table>,
}

/// Everything a frame needs besides the level.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameInput<'a> {
    /// Camera point: the centre of the logic view in main-plane pixels.
    pub center: (f64, f64),
    /// Sprites in drawing order.
    pub sprites: Vec<SpriteInput<'a>>,
    /// Simulation time of the frame in milliseconds.
    pub time_ms: f64,
    /// Palette fade in progress.
    pub fade: Option<PaletteFade>,
    /// Screen-layer sprites (the status bar and messages), drawn after the
    /// world in this order; see [`crate::screen`].
    pub screen: Vec<ScreenSprite>,
    /// Screen transition in progress; see [`crate::screen`].
    pub transition: Option<ScreenTransition>,
}

impl<'a> FrameInput<'a> {
    /// The input for an interpolated simulation frame.
    pub fn from_frame(frame: &InterpolatedFrame<'a>, view: &ViewConfig) -> FrameInput<'a> {
        let c = view.logic_center(frame.camera);
        FrameInput {
            center: (c.x, c.y),
            sprites: frame
                .renderables
                .iter()
                .map(|r| SpriteInput {
                    id: r.source.id,
                    key: &r.source.image_set_key,
                    frame: r.source.frame,
                    x: r.x,
                    y: r.y,
                    z: r.source.z,
                    mirror: r.source.mirror,
                    invert: r.source.invert,
                    flash: r.source.flash,
                    mode: r.source.draw_mode,
                    color: None,
                    table: None,
                })
                .collect(),
            time_ms: frame.time_ms,
            fade: frame.palette_fade,
            screen: Vec::new(),
            transition: None,
        }
    }
}

/// A frame ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Target and output geometry.
    pub comp: Composition,
    /// One entry per level plane.
    pub planes: Vec<PlaneParams>,
    /// Sprites referenced by [`Item::Sprite`].
    pub sprites: Vec<SpriteParams>,
    /// Drawing order.
    pub items: Vec<Item>,
    /// The palette of this frame (after any fade).
    pub palette: Palette,
    /// Brightness of this frame for replacement images, 0 to 1.
    pub brightness: f32,
    /// Transition mask applied after every item.
    pub mask: Option<Mask>,
}

/// The visible rectangle of every plane for a camera, in fractional plane
/// pixels (left, top).
#[derive(Debug, Clone)]
pub struct ViewSolver {
    world: WorldView,
}

impl ViewSolver {
    /// A solver for `planes` with views of `width` x `height`.
    pub fn new(planes: &[claw_assets::Plane], width: u32, height: u32) -> ViewSolver {
        let mut world = WorldView::new(planes);
        world.set_view_size(width as i32, height as i32);
        ViewSolver { world }
    }

    /// The exact view model, positioned by the last call.
    pub fn world(&self) -> &WorldView {
        &self.world
    }

    fn rects(&mut self, x: i32, y: i32) -> Vec<Rect> {
        self.world.set_camera(x, y);
        self.world.planes.iter().map(|p| p.rect).collect()
    }

    /// The top-left of every plane's view for a camera centre that may lie
    /// between whole pixels: the exact views at the whole pixels on either
    /// side, blended by the fraction (across a wrap seam where needed).
    pub fn origins(&mut self, cx: f64, cy: f64) -> Vec<(f64, f64)> {
        let (fx, fy) = (cx.floor(), cy.floor());
        let (tx, ty) = (cx - fx, cy - fy);
        let clamp = |v: f64| v.clamp(f64::from(i32::MIN / 2), f64::from(i32::MAX / 2)) as i32;
        let (ix, iy) = (clamp(fx), clamp(fy));
        let next = if tx > 0.0 || ty > 0.0 {
            Some(self.rects(ix + 1, iy + 1))
        } else {
            None
        };
        let base = self.rects(ix, iy);
        let mut out = Vec::with_capacity(base.len());
        for (i, r0) in base.iter().enumerate() {
            let p = &self.world.planes[i];
            let (mut l, mut t) = (f64::from(r0.left), f64::from(r0.top));
            if let Some(r1) = next.as_ref().map(|n| n[i]) {
                let unwrap = |d: i32, size: i32, wrap: bool| {
                    let d = f64::from(d);
                    let s = f64::from(size);
                    if wrap && s > 0.0 && d.abs() > s / 2.0 {
                        d - s * d.signum()
                    } else {
                        d
                    }
                };
                l += tx * unwrap(r1.left - r0.left, p.width_px(), p.wraps_x());
                t += ty * unwrap(r1.top - r0.top, p.height_px(), p.wraps_y());
            }
            out.push((l, t));
        }
        out
    }
}

/// Builds scenes for one level and view.
#[derive(Debug, Clone)]
pub struct SceneBuilder {
    view: ViewConfig,
    comp: Composition,
    solver: ViewSolver,
    expand: (i32, i32),
    flash_period_ms: f64,
    front_layer: bool,
}

impl SceneBuilder {
    /// A builder for `graphics` presented as `view`.
    pub fn new(graphics: &LevelGraphics, view: ViewConfig, max_target: u32) -> SceneBuilder {
        let comp = Composition::new(&view, max_target);
        let (pw, ph) = comp.presentation;
        // The centred policy places the logic view and widens it evenly; the
        // others place the whole presentation view against the plane edges.
        let (vw, vh, expand) = if view.aspect_policy == AspectPolicy::FrozenEdgeCentered {
            let lw = view.logic_width.max(1);
            let lh = view.logic_height.max(1);
            (
                lw,
                lh,
                (
                    (pw.saturating_sub(lw) / 2) as i32,
                    (ph.saturating_sub(lh) / 2) as i32,
                ),
            )
        } else {
            (pw, ph, (0, 0))
        };
        SceneBuilder {
            view,
            comp,
            solver: ViewSolver::new(&graphics.planes, vw, vh),
            expand,
            flash_period_ms: FLASH_PERIOD_MS,
            front_layer: true,
        }
    }

    /// Draws the level's front plane (true, the default) or leaves it out.
    pub fn set_front_layer(&mut self, on: bool) {
        self.front_layer = on;
    }

    /// True when the level's front plane is drawn.
    pub fn front_layer(&self) -> bool {
        self.front_layer
    }

    /// The view this builder presents.
    pub fn view(&self) -> &ViewConfig {
        &self.view
    }

    /// The composition geometry.
    pub fn composition(&self) -> &Composition {
        &self.comp
    }

    /// The visible tile grid of every plane for a whole-pixel camera centre,
    /// with each plane's view left and top: what the tile walk visits.
    pub fn visible_cells(
        &mut self,
        graphics: &LevelGraphics,
        cx: i32,
        cy: i32,
    ) -> Vec<(Rect, Vec<Vec<i32>>)> {
        self.solver.world.set_camera(cx, cy);
        let (ex, ey) = self.expand;
        let (pw, ph) = (
            self.comp.presentation.0 as i32,
            self.comp.presentation.1 as i32,
        );
        self.solver
            .world
            .planes
            .iter()
            .zip(&graphics.planes)
            .map(|(p, plane)| {
                let mut r = p.rect;
                r.left -= ex;
                r.top -= ey;
                r.right = r.left + pw - 1;
                r.bottom = r.top + ph - 1;
                (r, p.cells_for(r, &plane.tiles))
            })
            .collect()
    }

    /// Builds the scene of one frame.
    pub fn build(&mut self, graphics: &mut LevelGraphics, input: &FrameInput<'_>) -> Scene {
        let k = self.comp.scale;
        let kf = f64::from(k);
        let origins = self.solver.origins(input.center.0, input.center.1);
        let (ex, ey) = (f64::from(self.expand.0), f64::from(self.expand.1));
        let round = |v: f64| v.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        let mut offset = 0u32;
        let mut planes = Vec::with_capacity(graphics.planes.len());
        for (i, plane) in graphics.planes.iter().enumerate() {
            let (l, t) = origins[i];
            let pv = &self.solver.world.planes[i];
            planes.push(PlaneParams {
                origin: [round((l - ex) * kf), round((t - ey) * kf)],
                size: [pv.width_px(), pv.height_px()],
                tile: [pv.tile_w, pv.tile_h],
                grid: [pv.w_tiles, pv.h_tiles],
                slots: offset,
                flags: plane.flags & (PLANE_X_WRAP | PLANE_Y_WRAP),
                fill: plane.fill_color & 0xFF,
                scale: k,
            });
            offset += graphics.plane_slots[i].len() as u32;
        }

        // Main-plane view, for placing sprites.
        let main = self.solver.world.main;
        let (pw, ph) = self.comp.presentation;
        let (main_left, main_top, main_origin) = match main {
            Some(m) => (origins[m].0 - ex, origins[m].1 - ey, planes[m].origin),
            None => {
                let l = input.center.0 - f64::from(pw / 2);
                let t = input.center.1 - f64::from(ph / 2);
                (l, t, [round(l * kf), round(t * kf)])
            }
        };
        let main_wrap = main.map(|m| {
            let p = &self.solver.world.planes[m];
            (
                p.wraps_x().then_some(f64::from(p.width_px())),
                p.wraps_y().then_some(f64::from(p.height_px())),
            )
        });

        let mut order: Vec<&SpriteInput<'_>> = input.sprites.iter().collect();
        order.sort_by_key(|s| (s.z, s.id));
        let mut sprites = Vec::new();
        let mut sprite_z = Vec::new();
        let (tw, th) = (i64::from(self.comp.target.0), i64::from(self.comp.target.1));
        for s in order {
            if s.flash
                && (input.time_ms / self.flash_period_ms)
                    .floor()
                    .rem_euclid(2.0)
                    != 0.0
            {
                continue;
            }
            let Some(mut effect) = Effect::from_mode(s.mode.0) else {
                continue;
            };
            let Some(id) = graphics.sprite(s.key, s.frame) else {
                continue;
            };
            let rec = *graphics.images.record(id);
            let meta = graphics.meta(id);
            if effect == Effect::Normal && rec.flags & IMAGE_GHOST != 0 && s.table.is_none() {
                effect = Effect::Ghost;
            }
            let table = s.table.unwrap_or(effect.default_table());
            if effect == Effect::Remap && table == Table::None {
                continue;
            }
            let (w, h) = (rec.width, rec.height);
            if w == 0 || h == 0 {
                continue;
            }
            let cx = f64::from(w >> 1);
            let cy = f64::from(h >> 1);
            let ox = f64::from(if s.mirror {
                -meta.offset_x
            } else {
                meta.offset_x
            });
            let oy = f64::from(if s.invert {
                -meta.offset_y
            } else {
                meta.offset_y
            });
            let mut left = s.x - cx + ox;
            let mut top = s.y - cy + oy;
            if s.mirror && rec.flags & IMAGE_RLE != 0 {
                left += 1.0;
            }
            if let Some((wx, wy)) = main_wrap {
                left = wrap_to_view(left, main_left, f64::from(pw), wx);
                top = wrap_to_view(top, main_top, f64::from(ph), wy);
            }
            let pos = [
                round(left * kf) - main_origin[0],
                round(top * kf) - main_origin[1],
            ];
            let params = SpriteParams {
                pos,
                size: [w, h],
                image: id,
                flags: (u32::from(s.mirror) * SPRITE_MIRROR)
                    | (u32::from(s.invert) * SPRITE_INVERT),
                effect: effect as u32,
                color: u32::from(s.color.unwrap_or(DrawMode::DEFAULT_COLOR)),
                table: table.offset(),
                scale: k,
                pad: [0; 2],
            };
            let (x0, y0, x1, y1) = params.quad();
            if x1 <= 0 || y1 <= 0 || x0 >= tw || y0 >= th {
                continue;
            }
            sprites.push(params);
            sprite_z.push(s.z);
        }

        let hidden = if self.front_layer {
            None
        } else {
            front_plane(&graphics.planes)
        };
        let mut items = order_items(graphics, &sprite_z, main, hidden);

        // The screen layer, after the whole world.
        for s in &input.screen {
            if let Some(params) = screen::sprite_params(graphics, s, self.comp.presentation, k) {
                let (x0, y0, x1, y1) = params.quad();
                if x1 <= 0 || y1 <= 0 || x0 >= tw || y0 >= th {
                    continue;
                }
                items.push(Item::Sprite(sprites.len() as u32));
                sprites.push(params);
            }
        }

        let effect = input
            .transition
            .as_ref()
            .map(|t| {
                // The centre in the presentation: level pixels relative to
                // the main plane's view.
                let at = (t.center.x - main_left, t.center.y - main_top);
                screen::transition_effect(t, input.time_ms, at, self.comp.presentation, k)
            })
            .unwrap_or_default();
        let (palette, brightness) = screen::frame_palette(
            &graphics.palette,
            input.fade.as_ref(),
            &effect,
            input.time_ms,
        );
        Scene {
            comp: self.comp,
            planes,
            sprites,
            items,
            palette,
            brightness,
            mask: effect.mask,
        }
    }
}

/// The game's world-to-screen rule on a wrapping axis: bring the position
/// into the plane, then past the far edge when the view runs over the seam
/// and the position lies in the wrapped part.
fn wrap_to_view(pos: f64, view_start: f64, view_len: f64, size: Option<f64>) -> f64 {
    let Some(size) = size.filter(|s| *s > 0.0) else {
        return pos;
    };
    let mut p = pos;
    if p < 0.0 {
        p += size;
    } else if p >= size {
        p -= size;
    }
    let start = view_start.rem_euclid(size);
    let end = start + view_len - 1.0;
    if end >= size && p < start && p <= end - size {
        p += size;
    }
    p + (view_start - start)
}

/// Drawing order: planes in level order, with sprites placed by z between
/// them in interleaved levels, or all after the main plane otherwise. The
/// plane `hidden` (the front plane with the front layer off) is left out.
fn order_items(
    graphics: &LevelGraphics,
    sprite_z: &[i32],
    main: Option<usize>,
    hidden: Option<usize>,
) -> Vec<Item> {
    let drawn =
        |i: usize| graphics.planes[i].flags & crate::view::PLANE_NO_DRAW == 0 && Some(i) != hidden;
    let mut items = Vec::with_capacity(graphics.planes.len() + sprite_z.len());
    let mut next = 0usize;
    let n = graphics.planes.len();
    if n == 0 {
        items.extend((0..sprite_z.len()).map(|i| Item::Sprite(i as u32)));
        return items;
    }
    if graphics.interleaved {
        if drawn(0) {
            items.push(Item::Plane(0));
        }
        for i in 1..n {
            let z = graphics.planes[i].z;
            while next < sprite_z.len() && sprite_z[next] < z {
                items.push(Item::Sprite(next as u32));
                next += 1;
            }
            if drawn(i) {
                items.push(Item::Plane(i as u32));
            }
        }
    } else {
        let split = main.unwrap_or(n - 1);
        for i in 0..=split {
            if drawn(i) {
                items.push(Item::Plane(i as u32));
            }
        }
        items.extend((0..sprite_z.len()).map(|i| Item::Sprite(i as u32)));
        next = sprite_z.len();
        for i in split + 1..n {
            if drawn(i) {
                items.push(Item::Plane(i as u32));
            }
        }
    }
    items.extend((next..sprite_z.len()).map(|i| Item::Sprite(i as u32)));
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_geometry() {
        let v = ViewConfig::default();
        let c = Composition::new(&v.with_render_size(3840, 2160), DEFAULT_MAX_TARGET);
        assert_eq!(c.presentation, (854, 480));
        assert_eq!(c.scale, 4);
        assert_eq!(c.target, (3416, 1920));
        assert_eq!((c.width, c.height), (3843, 2160));
        assert_eq!((c.x, c.y), (-2, 0));
        let c = Composition::new(&v.with_render_size(5120, 1440), DEFAULT_MAX_TARGET);
        assert_eq!((c.presentation, c.scale), ((1708, 480), 3));
        assert_eq!(c.ratio, 1.0);
        let int = ViewConfig {
            scale_mode: ScaleMode::Integer,
            ..v.with_render_size(1920, 1080)
        };
        let c = Composition::new(&int, DEFAULT_MAX_TARGET);
        assert_eq!((c.width, c.height, c.ratio), (1708, 960, 1.0));
        assert_eq!((c.x, c.y), (106, 60));
        let pillar = ViewConfig {
            aspect_policy: AspectPolicy::Pillarbox,
            ..v.with_render_size(1920, 1080)
        };
        let c = Composition::new(&pillar, DEFAULT_MAX_TARGET);
        assert_eq!(
            (c.presentation, c.width, c.height, c.x),
            ((640, 480), 1440, 1080, 240)
        );
        let small = Composition::new(&v.with_render_size(320, 240), DEFAULT_MAX_TARGET);
        assert_eq!((small.scale, small.width, small.height), (1, 320, 240));
        let capped = Composition::new(&v.with_render_size(15360, 8640), 8192);
        assert_eq!(capped.scale, 9);
    }

    #[test]
    fn wrap_rule() {
        // View 100..=739 on a 640 px wrapping plane: 50 lies in the wrapped
        // part and is drawn at 690.
        assert_eq!(wrap_to_view(50.0, 100.0, 640.0, Some(640.0)), 690.0);
        assert_eq!(wrap_to_view(300.0, 100.0, 640.0, Some(640.0)), 300.0);
        assert_eq!(wrap_to_view(-10.0, 0.0, 640.0, Some(1000.0)), 990.0);
        assert_eq!(wrap_to_view(5.0, 0.0, 640.0, None), 5.0);
    }
}
