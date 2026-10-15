//! Scene building and the CPU reference compositor on a synthetic level.

mod common;

use std::collections::HashSet;

use claw_frame::{
    AspectPolicy, DrawMode, HudState, PaletteFade, Point, Renderable, ScaleMode, SimSnapshot,
    interpolate,
};
use claw_render::effects::{Effect, Table};
use claw_render::scene::{DEFAULT_MAX_TARGET, Item, SPRITE_MIRROR};
use claw_render::{FrameInput, HdPack, SceneBuilder, cpu};
use common::*;

fn single(sprite: claw_render::SpriteInput<'_>, center: (f64, f64)) -> FrameInput<'_> {
    FrameInput {
        center,
        sprites: vec![sprite],
        time_ms: 0.0,
        fade: None,
        screen: Vec::new(),
        transition: None,
    }
}

/// (screen x, screen y, index) of every pixel a lone sprite draws.
fn footprint(mirror: bool, key: &str) -> HashSet<(i64, i64, u8)> {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut s = sprite(1, key, 2, 1000.0, 400.0, 100);
    s.mirror = mirror;
    let scene = b.build(&mut g, &single(s, (1000.0, 400.0)));
    let sp = scene.sprites[0];
    assert_eq!(sp.flags & SPRITE_MIRROR != 0, mirror);
    let tables = g.tables.packed();
    let (x0, y0, x1, y1) = sp.quad();
    let mut out = HashSet::new();
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some((i, _)) = cpu::sprite_pixel(&g, &tables, &sp, x, y, 0) {
                out.insert((x, y, i));
            }
        }
    }
    assert!(!out.is_empty());
    out
}

#[test]
fn mirrored_run_length_sprite_reflects_about_its_column() {
    // View centred on x = 1000 with a 640 px view: level x 1000 is screen
    // column 320.
    let plain = footprint(false, "SPR");
    let mirrored = footprint(true, "SPR");
    let reflected: HashSet<_> = plain.iter().map(|&(x, y, i)| (2 * 320 - x, y, i)).collect();
    assert_eq!(reflected, mirrored);
}

#[test]
fn placement_uses_center_and_offsets() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    // SPR frame 3 is 12x10 with offset (1, -2): box left = x - 6 + 1, top =
    // y - 5 - 2.
    let mut s = sprite(1, "SPR", 3, 1000.0, 400.0, 0);
    let scene = b.build(&mut g, &single(s, (1000.0, 400.0)));
    let main_left = 1000 - 320;
    let main_top = 400 - 240;
    assert_eq!(
        scene.sprites[0].pos,
        [1000 - 6 + 1 - main_left, 400 - 5 - 2 - main_top]
    );
    s.invert = true;
    s.mirror = true;
    let scene = b.build(&mut g, &single(s, (1000.0, 400.0)));
    // Mirrored run-length frames start one pixel further right.
    assert_eq!(
        scene.sprites[0].pos,
        [1000 - 6 - 1 + 1 - main_left, 400 - 5 + 2 - main_top]
    );
}

#[test]
fn interleaved_and_plain_draw_order() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let c = (700.0, 400.0);
    let frame = FrameInput {
        center: c,
        sprites: vec![
            sprite(4, "SPR", 1, 700.0, 400.0, 9000),
            sprite(1, "SPR", 1, 700.0, 400.0, -1000),
            sprite(3, "SPR", 1, 700.0, 400.0, 0),
            sprite(2, "SPR", 1, 700.0, 400.0, -1),
        ],
        time_ms: 0.0,
        fade: None,
        screen: Vec::new(),
        transition: None,
    };
    let scene = b.build(&mut g, &frame);
    use Item::*;
    // Sorted by z: -1000, -1, 0, 9000. Plane 0 always first; z < 0 before
    // the main plane; z == 9000 after the front plane.
    assert_eq!(
        scene.items,
        vec![
            Plane(0),
            Sprite(0),
            Sprite(1),
            Plane(1),
            Sprite(2),
            Plane(2),
            Sprite(3)
        ]
    );
    g.interleaved = false;
    let scene = b.build(&mut g, &frame);
    assert_eq!(
        scene.items,
        vec![
            Plane(0),
            Plane(1),
            Sprite(0),
            Sprite(1),
            Sprite(2),
            Sprite(3),
            Plane(2)
        ]
    );
}

#[test]
fn fractional_camera_moves_by_output_pixels() {
    let mut g = graphics(None);
    // 1280x960: whole scale 2.
    let mut b = SceneBuilder::new(&g, view(1280, 960), DEFAULT_MAX_TARGET);
    assert_eq!(b.composition().scale, 2);
    let at = |b: &mut SceneBuilder, g: &mut claw_render::LevelGraphics, x: f64| {
        b.build(g, &single(sprite(1, "SPR", 1, 900.0, 400.0, 0), (x, 400.0)))
    };
    let s0 = at(&mut b, &mut g, 1000.0);
    let s1 = at(&mut b, &mut g, 1000.5);
    let s2 = at(&mut b, &mut g, 1001.0);
    assert_eq!(s0.planes[1].origin[0], (1000 - 320) * 2);
    assert_eq!(s1.planes[1].origin[0], (1000 - 320) * 2 + 1);
    assert_eq!(s2.planes[1].origin[0], (1001 - 320) * 2);
    // A sprite at rest stays put on the main plane.
    for s in [&s0, &s1, &s2] {
        assert_eq!(
            s.sprites[0].pos[0] + s.planes[1].origin[0],
            (900 - 6 - 1) * 2
        );
    }
    // Near the left edge the main plane is clamped and does not move.
    let e0 = at(&mut b, &mut g, 100.0);
    let e1 = at(&mut b, &mut g, 100.5);
    assert_eq!(e0.planes[1].origin, e1.planes[1].origin);
    assert_eq!(e0.planes[1].origin[0], 0);
}

#[test]
fn centred_policy_widens_around_the_logic_view() {
    let g = graphics(None);
    let wide = view_with(1920, 1080, AspectPolicy::FrozenEdgeCentered, ScaleMode::Fit);
    let mut b = SceneBuilder::new(&g, wide, DEFAULT_MAX_TARGET);
    let cells = b.visible_cells(&g, 0, 400);
    // Logic view clamped to the left edge (left 0), widened by (854 - 640)
    // / 2 = 107 px on each side.
    let (rect, grid) = &cells[1];
    assert_eq!(rect.left, -107);
    assert_eq!(rect.right, -107 + 853);
    assert!(grid[0][..7].iter().all(|&c| c == -1));
    let clamped = view_with(1920, 1080, AspectPolicy::FrozenEdgeClamped, ScaleMode::Fit);
    let mut b = SceneBuilder::new(&g, clamped, DEFAULT_MAX_TARGET);
    let cells = b.visible_cells(&g, 0, 400);
    assert_eq!((cells[1].0.left, cells[1].0.right), (0, 853));
}

#[test]
fn flash_blinks_on_simulation_time() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut s = sprite(1, "SPR", 1, 1000.0, 400.0, 0);
    s.flash = true;
    let mut seen = Vec::new();
    for t in [0.0, 20.0, 40.0, 70.0, 100.0] {
        let mut f = single(s, (1000.0, 400.0));
        f.time_ms = t;
        seen.push(b.build(&mut g, &f).sprites.len());
    }
    assert_eq!(seen, vec![1, 1, 0, 1, 0]);
}

#[test]
fn fades_change_the_palette_only() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut f = busy_frame((1000.0, 400.0));
    let plain = b.build(&mut g, &f);
    f.fade = Some(PaletteFade {
        from: 256,
        to: 0,
        start_tick: 50,
        duration_ms: 250,
    });
    f.time_ms = 900.0 + 125.0;
    let faded = b.build(&mut g, &f);
    assert_eq!(plain.items, faded.items);
    assert_eq!(plain.sprites, faded.sprites);
    let want = g.palette.0[255].map(|c| c - c / 2);
    assert_eq!(faded.palette.0[255], want);
    assert!((faded.brightness - 0.5).abs() < 1e-6);
    let c1 = cpu::compose(&g, &plain);
    let c2 = cpu::compose(&g, &faded);
    assert_eq!(c1, c2, "a fade never changes indices");
}

#[test]
fn modes_and_defaults() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut remap = sprite(1, "SPR", 1, 1000.0, 400.0, 0);
    remap.mode = DrawMode::REMAP;
    let ghostly = sprite(2, "GHOSTLY", 1, 1000.0, 400.0, 0);
    let mut hidden = sprite(3, "SPR", 1, 1000.0, 400.0, 0);
    hidden.mode = DrawMode(9);
    let missing = sprite(4, "NOPE", 1, 1000.0, 400.0, 0);
    let mut shadow = sprite(5, "SPR", 1, 1000.0, 400.0, 0);
    shadow.mode = DrawMode::SHADOW;
    let f = FrameInput {
        center: (1000.0, 400.0),
        sprites: vec![remap, ghostly, hidden, missing, shadow],
        time_ms: 0.0,
        fade: None,
        screen: Vec::new(),
        transition: None,
    };
    let scene = b.build(&mut g, &f);
    // Remap has no default table, mode 9 draws nothing, the missing set is
    // skipped; the flagged frame draws translucent.
    assert_eq!(scene.sprites.len(), 2);
    assert_eq!(scene.sprites[0].effect(), Effect::Ghost);
    assert_eq!(scene.sprites[0].table, Table::Average.offset());
    assert_eq!(scene.sprites[1].effect(), Effect::Shadow);
    assert_eq!(scene.sprites[1].color, 0x80);
}

#[test]
fn ghost_reads_what_is_below() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    // Both above the front plane, so the ghost is the last draw.
    let base = sprite(1, "KEYED", 1, 1000.0, 400.0, 9500);
    let mut ghost = sprite(2, "SPR", 1, 1000.0, 400.0, 9501);
    ghost.mode = DrawMode::GHOST;
    let f = FrameInput {
        center: (1000.0, 400.0),
        sprites: vec![base, ghost],
        time_ms: 0.0,
        fade: None,
        screen: Vec::new(),
        transition: None,
    };
    let scene = b.build(&mut g, &f);
    let below = {
        let mut only = scene.clone();
        assert_eq!(only.items.pop(), Some(Item::Sprite(1)));
        cpu::compose(&g, &only)
    };
    let both = cpu::compose(&g, &scene);
    let sp = scene.sprites[1];
    let tables = g.tables.packed();
    let (x0, y0, x1, y1) = sp.quad();
    let mut checked = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let o = (y * 640 + x) as usize;
            let d = below.index[o];
            let plain = claw_render::scene::SpriteParams { effect: 1, ..sp };
            if let Some((s, _)) = cpu::sprite_pixel(&g, &tables, &plain, x, y, d) {
                assert_eq!(
                    both.index[o],
                    g.tables.average[usize::from(d) * 256 + usize::from(s)]
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 50);
}

#[test]
fn snapshot_camera_is_the_logic_view_corner() {
    let mut g = graphics(None);
    let v = view(640, 480);
    let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
    let r = Renderable {
        id: 7,
        image_set_key: "SPR".into(),
        frame: 1,
        x: 1000.0,
        y: 400.0,
        z: 0,
        mirror: false,
        invert: false,
        flash: false,
        draw_mode: DrawMode::NORMAL,
        teleported: false,
    };
    let snap = SimSnapshot {
        tick: 3,
        camera: Point { x: 680.0, y: 160.0 },
        renderables: vec![r],
        palette_fade: None,
        camera_teleported: false,
        hud: HudState::default(),
        audio: Vec::new(),
        events: Vec::new(),
        transition: None,
    };
    let frame = interpolate(&snap, &snap, 0.0);
    let input = FrameInput::from_frame(&frame, &v);
    assert_eq!(input.center, (1000.0, 400.0));
    let scene = b.build(&mut g, &input);
    assert_eq!(scene.planes[1].origin, [680, 160]);
}

#[test]
fn nearest_pack_looks_like_no_pack() {
    let dir = tempfile::tempdir().unwrap();
    let v = view(1280, 960);
    let center = (1003.0, 377.0);
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
    let plain = b.build(&mut g, &busy_frame(center));
    let plain_out = cpu::present(&plain, &cpu::compose(&g, &plain));
    let written = g.write_nearest_pack(dir.path(), 2).unwrap();
    assert!(written > 20);

    let mut gp = graphics(Some(HdPack::load(dir.path()).unwrap()));
    let mut bp = SceneBuilder::new(&gp, v, DEFAULT_MAX_TARGET);
    let packed = bp.build(&mut gp, &busy_frame(center));
    assert!(gp.pack_warnings().is_empty(), "{:?}", gp.pack_warnings());
    let composed = cpu::compose(&gp, &packed);
    assert!(composed.hd.iter().filter(|&&h| h != 0).count() > 100_000);
    let packed_out = cpu::present(&packed, &composed);
    assert_eq!(plain_out, packed_out);
}

#[test]
fn integer_scaling_is_exact_nearest() {
    let mut g = graphics(None);
    let v = view_with(1500, 1000, AspectPolicy::Pillarbox, ScaleMode::Integer);
    let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
    let scene = b.build(&mut g, &busy_frame((1000.0, 400.0)));
    assert_eq!(scene.comp.scale, 2);
    assert_eq!((scene.comp.x, scene.comp.y), (110, 20));
    let c = cpu::compose(&g, &scene);
    let out = cpu::present(&scene, &c);
    for (ox, oy) in [(0u32, 0u32), (109, 500), (110, 20), (500, 300), (1389, 979)] {
        let o = ((oy * 1500 + ox) * 4) as usize;
        let inside = (110..1390).contains(&ox) && (20..980).contains(&oy);
        let want = if inside {
            let i = c.index[((oy - 20) * 1280 + (ox - 110)) as usize];
            let p = scene.palette.color(i);
            [p[0], p[1], p[2], 255]
        } else {
            [0, 0, 0, 255]
        };
        assert_eq!(out[o..o + 4], want, "({ox}, {oy})");
    }
}

#[test]
fn preloaded_sets_draw_the_same_without_loading_on_the_frame_path() {
    let frame = busy_frame((1000.0, 400.0));
    let mut lazy = graphics(None);
    let mut b = SceneBuilder::new(&lazy, view(640, 480), DEFAULT_MAX_TARGET);
    let lazy_scene = b.build(&mut lazy, &frame);
    assert!(lazy.loaded_while_drawing() > 0);

    let mut eager = graphics(None);
    let keys: Vec<&str> = frame.sprites.iter().map(|s| s.key).collect();
    assert_eq!(eager.preload(&keys), 5, "SPR, ODD, KEYED, GHOSTLY, MISSING");
    let images = eager.images.records.len();
    let mut b = SceneBuilder::new(&eager, view(640, 480), DEFAULT_MAX_TARGET);
    let eager_scene = b.build(&mut eager, &frame);
    assert_eq!(eager.loaded_while_drawing(), 0);
    assert_eq!(eager.images.records.len(), images, "nothing decoded");
    // Image ids follow load order, so compare the composed pictures.
    let a = cpu::compose(&lazy, &lazy_scene);
    let b = cpu::compose(&eager, &eager_scene);
    assert_eq!((a.width, a.height), (b.width, b.height));
    assert!(a.index == b.index, "the same picture");
}
