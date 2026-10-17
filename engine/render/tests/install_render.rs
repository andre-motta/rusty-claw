//! Tests that use the GPU or your own install. They run with
//! `make test-install MODULE=render` (which needs `CLAW_INSTALL`) and skip
//! themselves otherwise; set `CLAW_GPU_TESTS=1` to run only the GPU tests on
//! synthetic data without an install. Run them under the shared bench lock
//! so that parallel GPU work does not disturb each other.
//!
//! - GPU against the CPU reference on synthetic frames (every draw mode,
//!   mirrored and inverted sprites, fades, every scaling policy), with the
//!   screen layer and transitions, and on standalone screens.
//! - An image pack generated at test time against no pack.
//! - With `CLAW_INSTALL`: level 1 loads in under 3 s, and an offscreen frame
//!   of level 1 with the status bar matches the CPU reference.
//! - With `CLAW_INSTALL` and `CLAW_GOLDEN_DIR`: every visible tile grid in
//!   the golden folder matches, with zero mismatches.

mod common;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{
    AspectPolicy, DrawMode, PaletteFade, Point, ScaleMode, ScreenTransition, TICK_MS,
    TransitionKind, ViewConfig,
};
use claw_render::effects::LevelTables;
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{
    Anchor, FrameInput, Gpu, HdPack, LevelGraphics, MemoryImages, Renderer, Scene, SceneBuilder,
    ScreenBuilder, ScreenScene, ScreenSprite, SpriteInput, cpu,
};
use common::*;

fn install() -> Option<PathBuf> {
    std::env::var_os("CLAW_INSTALL").map(PathBuf::from)
}

fn gpu() -> Option<Gpu> {
    if install().is_none() && std::env::var_os("CLAW_GPU_TESTS").is_none() {
        eprintln!("skipped: set CLAW_INSTALL (make test-install) or CLAW_GPU_TESTS=1");
        return None;
    }
    Some(Gpu::headless().expect("a Vulkan GPU"))
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

fn check_frame(
    renderer: &mut Renderer,
    g: &mut LevelGraphics,
    view: ViewConfig,
    input: &FrameInput<'_>,
    label: &str,
) {
    let mut b = SceneBuilder::new(g, view, DEFAULT_MAX_TARGET);
    let scene = b.build(g, input);
    check_scene(renderer, g, &scene, label);
}

/// The GPU draws `scene` exactly as the CPU reference does.
fn check_scene(renderer: &mut Renderer, g: &LevelGraphics, scene: &Scene, label: &str) {
    let want = cpu::compose(g, scene);
    let got = renderer.read_composed(g, scene);
    assert_eq!(got.width, want.width, "{label}");
    let bad = got
        .index
        .iter()
        .zip(&want.index)
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(bad, 0, "{label}: {bad} index pixels differ");
    assert!(got.hd == want.hd, "{label}: replacement layer differs");
    let out = renderer.render_offscreen(g, scene);
    let reference = cpu::present(scene, &want);
    let d = max_diff(&out, &reference);
    let limit = if scene.comp.ratio == 1.0 { 0 } else { 1 };
    assert!(d <= limit, "{label}: output differs by {d}");
}

const SIZES: [(u32, u32); 5] = [
    (640, 480),
    (1920, 1080),
    (3440, 1440),
    (5120, 1440),
    (3840, 2160),
];

#[test]
fn gpu_matches_cpu_with_the_screen_layer_and_transitions() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu);
    let mut g = graphics(None);
    renderer.set_level(&g);
    let center = (1003.25, 377.75);
    let t0 = 50.0 * f64::from(TICK_MS);
    let circle = |closing: bool, ms: u32| ScreenTransition {
        kind: TransitionKind::Circle,
        closing,
        start_tick: 50,
        duration_ms: ms,
        center: Point {
            x: center.0 + 40.0,
            y: center.1 - 25.0,
        },
    };
    let cases: Vec<(&str, Option<ScreenTransition>, f64)> = vec![
        ("no transition", None, t0),
        ("closing circle", Some(circle(true, 1250)), t0 + 400.0),
        ("closed", Some(circle(true, 1250)), t0 + 1300.0),
        ("opening circle", Some(circle(false, 1000)), t0 + 700.0),
        (
            "fade in",
            Some(ScreenTransition {
                kind: TransitionKind::Fade,
                ..circle(false, 1500)
            }),
            t0 + 300.0,
        ),
    ];
    for size in SIZES {
        for v in [
            view(size.0, size.1),
            view_with(size.0, size.1, AspectPolicy::Pillarbox, ScaleMode::Integer),
        ] {
            for (name, transition, time) in &cases {
                let mut f = busy_frame(center);
                f.screen = screen_sprites();
                f.transition = *transition;
                f.time_ms = *time;
                let label = format!("{size:?} {:?} {name}", v.aspect_policy);
                check_frame(&mut renderer, &mut g, v, &f, &label);
            }
        }
    }
}

#[test]
fn gpu_matches_cpu_on_screens() {
    let Some(gpu) = gpu() else { return };
    let mut g = screen_graphics();
    let bg = g.add_image(&frame(640, 480, 0, |x, y| ((x / 5) ^ (y / 3)) as u8));
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&g);
    let mut pal = palette();
    pal.0.reverse();
    let screen = ScreenScene {
        background: Some(bg),
        palette: pal,
        sprites: screen_sprites(),
        fade: Some(PaletteFade {
            from: 0,
            to: 256,
            start_tick: 0,
            duration_ms: 500,
        }),
    };
    for size in SIZES {
        for scale in [ScaleMode::Fit, ScaleMode::Integer] {
            let v = view_with(size.0, size.1, AspectPolicy::FrozenEdgeClamped, scale);
            let b = ScreenBuilder::new(&v, DEFAULT_MAX_TARGET);
            for t in [100.0, 1000.0] {
                let scene = b.build(&mut g, &screen, t);
                check_scene(
                    &mut renderer,
                    &g,
                    &scene,
                    &format!("{size:?} {scale:?} at {t}"),
                );
            }
        }
    }
}

#[test]
fn gpu_matches_cpu_on_synthetic_frames() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu);
    let mut g = graphics(None);
    renderer.set_level(&g);
    let views = [
        ("640x480", view(640, 480)),
        ("1280x960", view(1280, 960)),
        ("1920x1080 fit", view(1920, 1080)),
        (
            "1500x1000 integer pillarbox",
            view_with(1500, 1000, AspectPolicy::Pillarbox, ScaleMode::Integer),
        ),
        (
            "2560x1080 centred",
            view_with(2560, 1080, AspectPolicy::FrozenEdgeCentered, ScaleMode::Fit),
        ),
        (
            "1720x720 widened",
            view_with(1720, 720, AspectPolicy::WidenedActivation, ScaleMode::Fit),
        ),
    ];
    let centers = [
        (1000.0, 400.0),
        (1003.25, 377.75),
        (5.0, 3.0),
        (1439.0, 799.0),
        (731.6, 250.1),
    ];
    for (label, v) in views {
        for c in centers {
            let mut f = busy_frame(c);
            check_frame(&mut renderer, &mut g, v, &f, &format!("{label} at {c:?}"));
            f.fade = Some(PaletteFade {
                from: 256,
                to: 0,
                start_tick: 50,
                duration_ms: 250,
            });
            f.time_ms = 1000.0;
            check_frame(
                &mut renderer,
                &mut g,
                v,
                &f,
                &format!("{label} at {c:?} faded"),
            );
        }
    }
}

#[test]
fn gpu_pack_looks_like_no_pack() {
    let Some(gpu) = gpu() else { return };
    let dir = tempfile::tempdir().unwrap();
    let v = view(1920, 1080);
    let c = (1001.5, 399.25);
    let mut g = graphics(None);
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&g);
    let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
    let scene = b.build(&mut g, &busy_frame(c));
    let plain = renderer.render_offscreen(&g, &scene);
    g.write_nearest_pack(dir.path(), 2).unwrap();

    let mut gp = graphics(Some(HdPack::load(dir.path()).unwrap()));
    let mut rp = Renderer::new(&gpu);
    rp.set_level(&gp);
    let mut bp = SceneBuilder::new(&gp, v, DEFAULT_MAX_TARGET);
    let scene = bp.build(&mut gp, &busy_frame(c));
    let composed = rp.read_composed(&gp, &scene);
    assert!(composed.hd.iter().any(|&h| h != 0));
    check_frame(&mut rp, &mut gp, v, &busy_frame(c), "pack");
    let packed = rp.render_offscreen(&gp, &scene);
    assert_eq!(max_diff(&plain, &packed), 0);
}

struct Level1 {
    level: LevelData,
    graphics: LevelGraphics,
    load: Duration,
}

fn load_level(root: &PathBuf, n: u32) -> (LevelData, LevelGraphics, Duration) {
    let t = Instant::now();
    let inst = Install::open(root).expect("install");
    let store = inst.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(n)).expect("level");
    let graphics = LevelGraphics::from_level(&level, &store, None);
    (level, graphics, t.elapsed())
}

fn level1() -> Option<Level1> {
    let root = install()?;
    let (level, graphics, load) = load_level(&root, 1);
    Some(Level1 {
        level,
        graphics,
        load,
    })
}

/// The level's placed objects as sprites, at their start positions.
fn object_sprites(level: &LevelData) -> Vec<SpriteInput<'_>> {
    level
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.image_set.is_empty())
        .map(|(i, o)| SpriteInput {
            id: i as u32,
            key: &o.image_set,
            frame: o.ints[8],
            x: f64::from(o.ints[5]),
            y: f64::from(o.ints[6]),
            z: o.ints[7],
            mirror: o.ints[11] & 2 != 0,
            invert: o.ints[11] & 4 != 0,
            flash: false,
            mode: DrawMode::NORMAL,
            color: None,
            table: None,
        })
        .collect()
}

#[test]
fn level1_loads_in_under_three_seconds() {
    let Some(mut l1) = level1() else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let t = Instant::now();
    let gpu = Gpu::headless().expect("a Vulkan GPU");
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&l1.graphics);
    // Load the sprite sets of the placed objects too.
    let mut b = SceneBuilder::new(&l1.graphics, ViewConfig::default(), DEFAULT_MAX_TARGET);
    let start = (f64::from(l1.level.start_x), f64::from(l1.level.start_y));
    let input = FrameInput {
        center: start,
        sprites: object_sprites(&l1.level),
        time_ms: 0.0,
        fade: None,
        screen: Vec::new(),
        transition: None,
    };
    let scene = b.build(&mut l1.graphics, &input);
    renderer.sync_images(&l1.graphics);
    renderer.wait_idle();
    let total = l1.load + t.elapsed();
    eprintln!(
        "level 1: assets and tiles {:.0} ms, with GPU set-up and object sprites {:.0} ms, {} images",
        l1.load.as_secs_f64() * 1000.0,
        total.as_secs_f64() * 1000.0,
        l1.graphics.images.records.len()
    );
    assert!(!scene.sprites.is_empty());
    assert!(total < Duration::from_secs(3), "{total:?}");
}

#[test]
fn level1_offscreen_frame_matches_reference() {
    let Some(mut l1) = level1() else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let gpu = Gpu::headless().expect("a Vulkan GPU");
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&l1.graphics);
    let start = (f64::from(l1.level.start_x), f64::from(l1.level.start_y));
    let level = l1.level.clone();
    for (label, v, c) in [
        ("640x480 at start", ViewConfig::default(), start),
        (
            "1280x960 between pixels",
            view(1280, 960),
            (start.0 + 300.5, start.1 - 40.25),
        ),
        (
            "1708x480 widened",
            view(3416, 960),
            (start.0 + 900.0, start.1),
        ),
    ] {
        let input = FrameInput {
            center: c,
            sprites: object_sprites(&level),
            time_ms: 0.0,
            fade: None,
            screen: status_bar(),
            transition: None,
        };
        check_frame(&mut renderer, &mut l1.graphics, v, &input, label);
    }
    // Smoke: a full 1080p frame has the picture's variety, and the status
    // bar is drawn.
    let mut b = SceneBuilder::new(&l1.graphics, view(1920, 1080), DEFAULT_MAX_TARGET);
    let input = FrameInput {
        center: start,
        sprites: object_sprites(&level),
        time_ms: 0.0,
        fade: None,
        screen: status_bar(),
        transition: None,
    };
    let scene = b.build(&mut l1.graphics, &input);
    assert_eq!(
        scene
            .sprites
            .iter()
            .filter(|s| s.scale == scene.comp.scale)
            .count(),
        scene.sprites.len()
    );
    let out = renderer.render_offscreen(&l1.graphics, &scene);
    // The builder leaves out objects off the view, so count the status bar
    // on a frame that has nothing else.
    let bar = SceneBuilder::new(&l1.graphics, view(1920, 1080), DEFAULT_MAX_TARGET)
        .build(
            &mut l1.graphics,
            &FrameInput {
                center: start,
                sprites: Vec::new(),
                time_ms: 0.0,
                fade: None,
                screen: status_bar(),
                transition: None,
            },
        )
        .sprites
        .len();
    assert!(
        bar >= status_bar().len(),
        "status bar pieces missing: {bar}"
    );
    let mut colors: HashMap<[u8; 3], usize> = HashMap::new();
    for px in out.chunks_exact(4) {
        *colors.entry([px[0], px[1], px[2]]).or_default() += 1;
    }
    assert!(colors.len() > 64, "only {} colours", colors.len());
}

/// The status bar's pieces at their default places, showing score
/// 12345678, health 100, 99 pistol shots and 3 spare lives.
fn status_bar() -> Vec<ScreenSprite> {
    let (tl, tr) = (Anchor::TopLeft, Anchor::TopRight);
    let mut out = vec![ScreenSprite::new(
        "GAME_INTERFACE_TREASURECHEST",
        1,
        tl,
        14,
        15,
    )];
    for i in 0..8 {
        out.push(ScreenSprite::new(
            "GAME_INTERFACE_SCORENUMBERS",
            i + 1,
            tl,
            36 + 11 * i,
            15,
        ));
    }
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_HEALTHHEART",
        1,
        tr,
        -30,
        15,
    ));
    for (i, d) in [1, 0, 0].into_iter().enumerate() {
        let x = -52 + 11 * i as i32;
        out.push(ScreenSprite::new(
            "GAME_INTERFACE_HEALTHNUMBERS",
            d,
            tr,
            x,
            14,
        ));
    }
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_WEAPONS_PISTOL",
        1,
        tr,
        -25,
        46,
    ));
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_SMALLNUMBERS",
        9,
        tr,
        -42,
        46,
    ));
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_SMALLNUMBERS",
        9,
        tr,
        -34,
        46,
    ));
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_LIVESHEAD",
        1,
        tr,
        -17,
        72,
    ));
    out.push(ScreenSprite::new(
        "GAME_INTERFACE_SMALLNUMBERS",
        3,
        tr,
        -32,
        73,
    ));
    out
}

/// A level's planes with no images decoded: enough for the tile grids.
fn planes_only(root: &PathBuf, n: u32) -> LevelGraphics {
    let inst = Install::open(root).expect("install");
    let store = inst.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(n)).expect("level");
    let tables = LevelTables::blank(&level.palette);
    LevelGraphics::new(
        level.planes,
        true,
        level.palette,
        tables,
        Box::new(MemoryImages::default()),
        None,
    )
}

/// The view whose presentation is exactly `w` x `h` for the golden grids.
fn golden_view(w: u32, h: u32) -> ViewConfig {
    ViewConfig {
        logic_width: w.min(640),
        logic_height: h,
        render_width: w,
        render_height: h,
        aspect_policy: AspectPolicy::FrozenEdgeClamped,
        scale_mode: ScaleMode::Fit,
    }
}

#[test]
fn visible_tile_grids_match_golden() {
    let Some(root) = install() else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let Some(dir) = std::env::var_os("CLAW_GOLDEN_DIR").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_GOLDEN_DIR is not set");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("golden folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("level") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    assert!(
        files.iter().any(|p| p.ends_with("level1_640x480.json")),
        "no level1_640x480.json in {}",
        dir.display()
    );
    let mut levels: HashMap<u32, LevelGraphics> = HashMap::new();
    let (mut grids, mut cells, mut mismatches) = (0usize, 0usize, 0usize);
    let mut views = Vec::new();
    for file in &files {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        let n = doc["level"].as_u64().unwrap() as u32;
        let w = doc["view"]["w"].as_u64().unwrap() as u32;
        let h = doc["view"]["h"].as_u64().unwrap() as u32;
        views.push(format!("level{n} {w}x{h}"));
        let g = levels.entry(n).or_insert_with(|| planes_only(&root, n));
        let v = golden_view(w, h);
        let mut b = SceneBuilder::new(g, v, DEFAULT_MAX_TARGET);
        assert_eq!(b.composition().presentation, (w, h));
        for cam in doc["cameras"].as_array().unwrap() {
            let (x, y) = (
                cam["x"].as_i64().unwrap() as i32,
                cam["y"].as_i64().unwrap() as i32,
            );
            let ours = b.visible_cells(g, x, y);
            let theirs = cam["planes"].as_array().unwrap();
            assert_eq!(ours.len(), theirs.len(), "{}: plane count", file.display());
            for (pi, ((rect, grid), want)) in ours.iter().zip(theirs).enumerate() {
                grids += 1;
                let name = want["name"].as_str().unwrap();
                assert_eq!(g.planes[pi].name, name);
                assert_eq!(i64::from(g.planes[pi].z), want["z"].as_i64().unwrap());
                let label = format!("{} camera ({x}, {y}) plane {name}", file.display());
                let rows = want["rows"].as_u64().unwrap() as usize;
                let cols = want["cols"].as_u64().unwrap() as usize;
                assert_eq!(
                    i64::from(rect.left),
                    want["view_left"].as_i64().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    i64::from(rect.top),
                    want["view_top"].as_i64().unwrap(),
                    "{label}"
                );
                assert_eq!(grid.len(), rows, "{label}: rows");
                for (r, row) in want["grid"].as_array().unwrap().iter().enumerate() {
                    let row = row.as_array().unwrap();
                    assert_eq!(grid[r].len(), cols, "{label}: cols");
                    for (c, v) in row.iter().enumerate() {
                        cells += 1;
                        if i64::from(grid[r][c]) != v.as_i64().unwrap() {
                            mismatches += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "golden: {} files ({}), {grids} plane grids, {cells} cells, {mismatches} mismatches",
        files.len(),
        views.join(", ")
    );
    assert_eq!(mismatches, 0);
}
