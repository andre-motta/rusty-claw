//! A small synthetic level built in code: three planes in the usual
//! arrangement (a wrapping background, the main plane, a front plane that
//! wraps horizontally) and a few sprite sets, with a palette of distinct
//! colours. Nothing here comes from game files.

#![allow(dead_code)]

use claw_assets::pid::{FLAG_COLOR_KEY, FLAG_RLE};
use claw_assets::{Frame, Palette, Plane};
use claw_frame::{AspectPolicy, DrawMode, ScaleMode, ViewConfig};
use claw_render::effects::{LevelTables, Table};
use claw_render::view::{CELL_EMPTY, CELL_FILL, PLANE_MAIN, PLANE_X_WRAP, PLANE_Y_WRAP};
use claw_render::{FrameInput, HdPack, LevelGraphics, MemoryImages, SpriteInput};

pub const TILE: i32 = 16;

pub fn palette() -> Palette {
    let mut p = Palette::default();
    for (i, c) in p.0.iter_mut().enumerate() {
        let i = i as u32;
        *c = [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (255 - i) as u8];
    }
    p
}

pub fn frame(w: u32, h: u32, flags: u32, f: impl Fn(u32, u32) -> u8) -> Frame {
    let mut indices = Vec::new();
    for y in 0..h {
        for x in 0..w {
            indices.push(f(x, y));
        }
    }
    Frame {
        width: w,
        height: h,
        offset_x: 0,
        offset_y: 0,
        flags,
        indices,
        opaque: None,
    }
}

/// A run-length sprite frame with a transparent diagonal notch, so that
/// mirroring and inverting are visible.
pub fn sprite_frame(w: u32, h: u32, base: u8, offset: (i32, i32)) -> Frame {
    let mut f = frame(w, h, FLAG_RLE, |x, y| {
        base.wrapping_add((x * 3 + y * 5) as u8 % 40)
    });
    f.opaque = Some(
        (0..h)
            .flat_map(|y| (0..w).map(move |x| u8::from(!(x + 1 < y && x < 4))))
            .collect(),
    );
    f.offset_x = offset.0;
    f.offset_y = offset.1;
    f
}

fn plane(name: &str, flags: u32, grid: (i32, i32), pct: (i32, i32), z: i32, set: &str) -> Plane {
    let (gw, gh) = grid;
    let mut tiles = Vec::new();
    for r in 0..gh {
        for c in 0..gw {
            let v = (r * 7 + c * 3) % 11;
            tiles.push(match v {
                0 => CELL_EMPTY,
                1 => CELL_FILL,
                9 => 99, // a tile the set does not have
                v => v,
            });
        }
    }
    Plane {
        name: name.into(),
        flags,
        tile_w: TILE,
        tile_h: TILE,
        w_tiles: gw,
        h_tiles: gh,
        move_x_pct: pct.0,
        move_y_pct: pct.1,
        fill_color: 0x1234_5678,
        z,
        image_sets: vec![set.into()],
        tiles,
        ..Plane::default()
    }
}

pub fn images() -> MemoryImages {
    let mut src = MemoryImages::default();
    for t in 2..9 {
        // Background tiles: opaque, unkeyed.
        src.insert(
            "BACK",
            t,
            frame(16, 16, 0, |x, y| (t as u32 * 20 + x + y) as u8),
        );
        // Action tiles: keyed, index 0 shows the plane below.
        src.insert(
            "ACTION",
            t,
            frame(16, 16, FLAG_COLOR_KEY, |x, y| {
                if (x + y + t as u32).is_multiple_of(5) {
                    0
                } else {
                    (100 + t as u32 * 3 + x) as u8
                }
            }),
        );
        // Front tiles: run-length with a mask.
        let mut f = frame(16, 16, FLAG_RLE, |x, y| (200 + x / 2 + y / 4) as u8);
        f.opaque = Some((0..256).map(|i| u8::from(i % 16 < 4)).collect());
        src.insert("FRONT", t, f);
    }
    // A tile larger than the plane's tile size: only its top-left part shows.
    src.insert("ACTION", 10, frame(24, 20, 0, |x, y| (60 + x + y) as u8));
    for n in 1..4 {
        src.insert(
            "SPR",
            n,
            sprite_frame(12, 10, 30 + n as u8 * 40, (n - 2, 1 - n)),
        );
    }
    src.insert("ODD", 1, sprite_frame(7, 5, 150, (0, 0)));
    src.insert(
        "KEYED",
        1,
        frame(6, 6, FLAG_COLOR_KEY, |x, y| if x == y { 0 } else { 77 }),
    );
    let mut ghost = sprite_frame(10, 8, 170, (0, 0));
    ghost.flags |= 0x40;
    src.insert("GHOSTLY", 1, ghost);
    src
}

pub fn planes() -> Vec<Plane> {
    vec![
        plane(
            "Back",
            PLANE_X_WRAP | PLANE_Y_WRAP,
            (8, 6),
            (50, 50),
            -999,
            "BACK",
        ),
        plane("Action", PLANE_MAIN, (90, 50), (100, 100), 0, "ACTION"),
        plane("Front", PLANE_X_WRAP, (10, 70), (150, 125), 9000, "FRONT"),
    ]
}

pub fn graphics(pack: Option<HdPack>) -> LevelGraphics {
    let pal = palette();
    let mut tables = LevelTables::blank(&pal);
    // Distinct, cheap tables: every entry depends on both operands.
    for d in 0..256usize {
        for s in 0..256usize {
            tables.average[d * 256 + s] = ((d + s) / 2) as u8;
            tables.light[d * 256 + s] = (d ^ s) as u8;
        }
    }
    LevelGraphics::new(planes(), true, pal, tables, Box::new(images()), pack)
}

pub fn view(w: u32, h: u32) -> ViewConfig {
    ViewConfig::default().with_render_size(w, h)
}

pub fn view_with(w: u32, h: u32, aspect: AspectPolicy, scale: ScaleMode) -> ViewConfig {
    ViewConfig {
        aspect_policy: aspect,
        scale_mode: scale,
        ..view(w, h)
    }
}

pub fn sprite<'a>(id: u32, key: &'a str, frame: i32, x: f64, y: f64, z: i32) -> SpriteInput<'a> {
    SpriteInput {
        id,
        key,
        frame,
        x,
        y,
        z,
        mirror: false,
        invert: false,
        flash: false,
        mode: DrawMode::NORMAL,
        color: None,
        table: None,
    }
}

/// A busy frame: sprites in front of, between and behind the planes, with
/// every draw mode, mirrored and inverted, overlapping each other.
pub fn busy_frame<'a>(center: (f64, f64)) -> FrameInput<'a> {
    let (cx, cy) = center;
    let mut sprites = Vec::new();
    let mut id = 0;
    let mut add = |s: SpriteInput<'a>| sprites.push(s);
    for (i, mode) in [0u32, 2, 3, 4, 5, 6].into_iter().enumerate() {
        for (j, (mirror, invert)) in [(false, false), (true, false), (false, true), (true, true)]
            .into_iter()
            .enumerate()
        {
            id += 1;
            let mut s = sprite(
                id,
                "SPR",
                (j % 3 + 1) as i32,
                cx - 150.0 + i as f64 * 41.3 + j as f64 * 9.0,
                cy - 100.0 + j as f64 * 23.7 + i as f64 * 4.0,
                [-5, 10, 9500][(i + j) % 3],
            );
            s.mode = DrawMode(mode);
            s.mirror = mirror;
            s.invert = invert;
            s.color = Some((40 * i + 13 * j) as u8);
            if mode == 4 {
                s.table = Some(Table::Light);
            }
            add(s);
        }
    }
    id += 1;
    add(sprite(id, "ODD", 1, cx + 50.0, cy + 30.0, 20));
    id += 1;
    let mut odd = sprite(id, "ODD", 1, cx + 70.0, cy + 30.0, 20);
    odd.mirror = true;
    add(odd);
    id += 1;
    add(sprite(id, "KEYED", 1, cx - 60.0, cy + 60.0, 5));
    id += 1;
    add(sprite(id, "GHOSTLY", 1, cx - 55.0, cy + 58.0, 6));
    id += 1;
    add(sprite(id, "MISSING", 1, cx, cy, 6));
    // Partly off the left edge of the view.
    id += 1;
    add(sprite(id, "SPR", 1, cx - 330.0, cy, 7));
    FrameInput {
        center,
        sprites,
        time_ms: 1000.0,
        fade: None,
    }
}
