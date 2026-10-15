//! CPU reference compositor.
//!
//! These functions compute, pixel by pixel, exactly what the shaders
//! compute: the plane tile walk, sprite placement with mirror and invert,
//! every draw mode, image replacements and the final palette and scaling
//! pass. Tests compare the GPU output against them; they are far too slow
//! to draw real frames.

use crate::effects::{Effect, apply};
use crate::images::{LevelGraphics, SLOT_EMPTY, SLOT_FILL};
use crate::scene::{Item, PlaneParams, SPRITE_INVERT, SPRITE_MIRROR, Scene, SpriteParams};
use crate::screen::Mask;
use crate::view::{PLANE_X_WRAP, PLANE_Y_WRAP};

/// A composed frame: palette indices plus the replacement layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composed {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Palette index per pixel, row-major.
    pub index: Vec<u8>,
    /// Replacement colour per pixel as packed RGBA (red in the low byte);
    /// alpha 0 where the palette colour shows.
    pub hd: Vec<u32>,
}

const HD_OPAQUE: u32 = 0xFF00_0000;

fn hd_coord(local: i64, k: i64, len: u32, scale: u32, flip: bool) -> u32 {
    let s = i64::from(scale);
    let v = if flip {
        (2 * k * i64::from(len) - 2 * local - 1) * s / (2 * k)
    } else {
        (2 * local + 1) * s / (2 * k)
    };
    v.clamp(0, i64::from(len) * s - 1) as u32
}

/// The index and replacement colour a plane draws at target pixel (px, py),
/// or `None` where it draws nothing.
pub fn plane_pixel(
    g: &LevelGraphics,
    plane: usize,
    p: &PlaneParams,
    px: i64,
    py: i64,
) -> Option<(u8, u32)> {
    let k = i64::from(p.scale.max(1));
    let qx = px + i64::from(p.origin[0]);
    let qy = py + i64::from(p.origin[1]);
    let mut x = qx.div_euclid(k);
    let mut y = qy.div_euclid(k);
    let (subx, suby) = (qx - x * k, qy - y * k);
    let (w, h) = (i64::from(p.size[0]), i64::from(p.size[1]));
    if p.flags & PLANE_X_WRAP != 0 && w > 0 {
        x = x.rem_euclid(w);
    }
    if p.flags & PLANE_Y_WRAP != 0 && h > 0 {
        y = y.rem_euclid(h);
    }
    if x < 0 || y < 0 || x >= w || y >= h {
        return None;
    }
    let (tw, th) = (i64::from(p.tile[0]), i64::from(p.tile[1]));
    let (col, row) = (x / tw, y / th);
    let (tx, ty) = ((x - col * tw) as u32, (y - row * th) as u32);
    let slot = *g
        .plane_slots
        .get(plane)?
        .get((row * i64::from(p.grid[0]) + col) as usize)?;
    match slot {
        SLOT_EMPTY => None,
        SLOT_FILL => Some((p.fill as u8, 0)),
        id if id >= 0 => {
            let id = id as u32;
            if !g.images.covered(id, tx, ty) {
                return None;
            }
            let index = g.images.index_at(id, tx, ty);
            let r = g.images.record(id);
            let hd = if r.hd_scale > 0 {
                let s = i64::from(r.hd_scale);
                let hx = i64::from(tx) * s + (2 * subx + 1) * s / (2 * k);
                let hy = i64::from(ty) * s + (2 * suby + 1) * s / (2 * k);
                g.images.hd_at(id, hx as u32, hy as u32) | HD_OPAQUE
            } else {
                0
            };
            Some((index, hd))
        }
        _ => None,
    }
}

/// The index and replacement colour a sprite draws at target pixel
/// (px, py) over destination index `dst`, or `None` where it draws nothing.
pub fn sprite_pixel(
    g: &LevelGraphics,
    tables: &[u8],
    s: &SpriteParams,
    px: i64,
    py: i64,
    dst: u8,
) -> Option<(u8, u32)> {
    let (x0, y0, x1, y1) = s.quad();
    if px < x0 || py < y0 || px >= x1 || py >= y1 {
        return None;
    }
    let k = i64::from(s.scale.max(1));
    let (lx, ly) = (px - x0, py - y0);
    let (c, r) = ((lx / k) as u32, (ly / k) as u32);
    let (w, h) = (s.size[0], s.size[1]);
    let mirror = s.flags & SPRITE_MIRROR != 0;
    let invert = s.flags & SPRITE_INVERT != 0;
    let sx = if mirror { w - 1 - c } else { c };
    let sy = if invert { h - 1 - r } else { r };
    if !g.images.covered(s.image, sx, sy) {
        return None;
    }
    let src = g.images.index_at(s.image, sx, sy);
    let effect = s.effect();
    if effect == Effect::Normal {
        let rec = g.images.record(s.image);
        let hd = if rec.hd_scale > 0 {
            let hx = hd_coord(lx, k, w, rec.hd_scale, mirror);
            let hy = hd_coord(ly, k, h, rec.hd_scale, invert);
            g.images.hd_at(s.image, hx, hy) | HD_OPAQUE
        } else {
            0
        };
        return Some((src, hd));
    }
    let table = tables.get(s.table as usize..);
    apply(effect, src, dst, s.color as u8, table).map(|i| (i, 0))
}

/// Composes `scene` into palette indices and the replacement layer.
pub fn compose(g: &LevelGraphics, scene: &Scene) -> Composed {
    let (w, h) = scene.comp.target;
    let mut out = Composed {
        width: w,
        height: h,
        index: vec![0; (w * h) as usize],
        hd: vec![0; (w * h) as usize],
    };
    let tables = g.tables.packed();
    for item in &scene.items {
        match *item {
            Item::Plane(i) => {
                let p = &scene.planes[i as usize];
                for y in 0..h {
                    for x in 0..w {
                        if let Some((idx, hd)) =
                            plane_pixel(g, i as usize, p, i64::from(x), i64::from(y))
                        {
                            let o = (y * w + x) as usize;
                            out.index[o] = idx;
                            out.hd[o] = hd;
                        }
                    }
                }
            }
            Item::Sprite(i) => {
                let s = &scene.sprites[i as usize];
                let (x0, y0, x1, y1) = s.quad();
                for y in y0.max(0)..y1.min(i64::from(h)) {
                    for x in x0.max(0)..x1.min(i64::from(w)) {
                        let o = (y * i64::from(w) + x) as usize;
                        if let Some((idx, hd)) = sprite_pixel(g, &tables, s, x, y, out.index[o]) {
                            out.index[o] = idx;
                            out.hd[o] = hd;
                        }
                    }
                }
            }
        }
    }
    if let Some(mask) = &scene.mask {
        apply_mask(&mut out, mask);
    }
    out
}

/// Turns every pixel the transition mask hides into palette index 0 with no
/// replacement colour.
pub fn apply_mask(c: &mut Composed, mask: &Mask) {
    for y in 0..c.height {
        for x in 0..c.width {
            if !mask.shows(i64::from(x), i64::from(y)) {
                let o = (y * c.width + x) as usize;
                c.index[o] = 0;
                c.hd[o] = 0;
            }
        }
    }
}

fn texel_color(scene: &Scene, c: &Composed, x: u32, y: u32) -> [f32; 3] {
    let o = (y * c.width + x) as usize;
    let hd = c.hd[o];
    if hd >> 24 != 0 {
        let b = hd.to_le_bytes();
        let k = scene.brightness;
        return [
            f32::from(b[0]) / 255.0 * k,
            f32::from(b[1]) / 255.0 * k,
            f32::from(b[2]) / 255.0 * k,
        ];
    }
    let p = scene.palette.color(c.index[o]);
    [
        f32::from(p[0]) / 255.0,
        f32::from(p[1]) / 255.0,
        f32::from(p[2]) / 255.0,
    ]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
    ]
}

/// The final pass: palette lookup (or replacement colour) and scaling to the
/// output, with bars in black. Returns RGBA bytes of the output size.
pub fn present(scene: &Scene, c: &Composed) -> Vec<u8> {
    let comp = &scene.comp;
    let (ow, oh) = comp.output;
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    let (tw, th) = (c.width as i32, c.height as i32);
    for oy in 0..oh {
        for ox in 0..ow {
            let o = ((oy * ow + ox) * 4) as usize;
            out[o + 3] = 255;
            let (rx, ry) = (ox as i64 - i64::from(comp.x), oy as i64 - i64::from(comp.y));
            if rx < 0 || ry < 0 || rx >= i64::from(comp.width) || ry >= i64::from(comp.height) {
                continue;
            }
            let u = (ox as f32 + 0.5 - comp.x as f32) / comp.ratio - 0.5;
            let v = (oy as f32 + 0.5 - comp.y as f32) / comp.ratio - 0.5;
            let (x0, y0) = (u.floor(), v.floor());
            let (fx, fy) = (u - x0, v - y0);
            let (x0, y0) = (x0 as i32, y0 as i32);
            let cx = |x: i32| x.clamp(0, tw - 1) as u32;
            let cy = |y: i32| y.clamp(0, th - 1) as u32;
            let c00 = texel_color(scene, c, cx(x0), cy(y0));
            let c10 = texel_color(scene, c, cx(x0 + 1), cy(y0));
            let c01 = texel_color(scene, c, cx(x0), cy(y0 + 1));
            let c11 = texel_color(scene, c, cx(x0 + 1), cy(y0 + 1));
            let col = mix(mix(c00, c10, fx), mix(c01, c11, fx), fy);
            for i in 0..3 {
                out[o + i] = (col[i].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    out
}
