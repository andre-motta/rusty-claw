//! The boss fight's pieces of the status bar: the health bar and its level
//! band falling with the boss's health, and the name banner, laid out on
//! synthetic art and drawn offscreen by the CPU reference; with
//! `CLAW_INSTALL` set, the
//! second level's art is also drawn on the GPU and compared with the CPU
//! reference (run that one under the shared bench lock and a virtual
//! display).

use std::path::PathBuf;

use claw_assets::{Frame, Install, LevelData, LevelSource, Palette};
use claw_frame::output::BossMeter;
use claw_frame::{HudState, ViewConfig, Weapon};
use claw_render::effects::LevelTables;
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{
    Anchor, FrameInput, Gpu, LevelGraphics, MemoryImages, Renderer, SceneBuilder, ScreenSprite, cpu,
};
use claw_ui::hud::{self, FillRect, Hud, HudArt};

const BAR_INDEX: u8 = 11;
const NAME_INDEX: u8 = 12;

fn solid(w: u32, h: u32, index: u8, offset: (i32, i32)) -> Frame {
    Frame {
        width: w,
        height: h,
        offset_x: offset.0,
        offset_y: offset.1,
        flags: 0,
        indices: vec![index; (w * h) as usize],
        opaque: None,
    }
}

fn graphics(with_boss_art: bool) -> LevelGraphics {
    let mut m = MemoryImages::default();
    if with_boss_art {
        m.insert(hud::BOSS_BAR, 1, solid(260, 28, BAR_INDEX, (0, 0)));
        m.insert(hud::BOSS_NAME, 1, solid(200, 60, NAME_INDEX, (0, 0)));
    }
    let mut pal = Palette::default();
    for (i, c) in pal.0.iter_mut().enumerate() {
        *c = [i as u8, (i * 3 % 256) as u8, 255 - i as u8];
    }
    let tables = LevelTables::blank(&pal);
    LevelGraphics::new(Vec::new(), true, pal, tables, Box::new(m), None)
}

fn art(with_boss_art: bool) -> (LevelGraphics, HudArt) {
    let mut g = graphics(with_boss_art);
    let art = HudArt::load(&mut g, |_| None);
    (g, art)
}

fn fight(meter: Option<BossMeter>, banner: Option<(i32, i32)>) -> HudState {
    HudState {
        health: 100,
        lives: 3,
        weapon: Weapon::Pistol,
        boss_meter: meter,
        boss_banner: banner,
        ..HudState::default()
    }
}

fn meter(value: i32) -> BossMeter {
    BossMeter { value, max: 100 }
}

#[test]
fn the_boss_art_is_preloaded_and_its_absence_is_not_reported() {
    let (mut g, with) = art(true);
    for key in hud::BOSS_IMAGE_SETS {
        assert!(g.is_loaded(key), "{key}");
    }
    let bar = with.boss_bar.expect("the bar's frame");
    assert_eq!((bar.width, bar.height), (260, 28));
    // The band's pieces are made by the status bar itself.
    for key in [hud::BOSS_BAND_FULL, hud::BOSS_BAND_EMPTY] {
        assert_eq!(
            g.frame_numbers(key),
            vec![1, 2, 4, 8, 16, 32, 64, 128],
            "{key}"
        );
    }
    let (g, without) = art(false);
    assert!(g.is_loaded(hud::BOSS_NAME));
    assert!(g.is_loaded(hud::BOSS_BAND_FULL));
    assert_eq!(without.boss_bar, None);
    assert!(
        !without
            .missing
            .iter()
            .any(|m| m == hud::BOSS_NAME || m == hud::BOSS_BAR)
    );
}

#[test]
fn the_percentage_and_the_full_width_follow_the_original() {
    assert_eq!(hud::boss_percent(meter(100)), 100);
    assert_eq!(hud::boss_percent(meter(150)), 100);
    assert_eq!(hud::boss_percent(meter(0)), 0);
    assert_eq!(hud::boss_percent(meter(-5)), 0);
    assert_eq!(hud::boss_percent(BossMeter { value: 2, max: 3 }), 66);
    assert_eq!(hud::boss_percent(BossMeter { value: 5, max: 0 }), 0);
    assert_eq!(hud::boss_full_width(100), 226);
    assert_eq!(hud::boss_full_width(0), 0);
    assert_eq!(hud::boss_full_width(50), 113);
    assert_eq!(hud::boss_full_width(95), 214);
    assert_eq!(hud::boss_full_width(1), 2);
    assert_eq!(hud::boss_full_width(99), 223);
}

#[test]
fn the_bar_sits_at_320_447_with_its_band_inside() {
    let (_, art) = art(true);
    let l = hud::boss_meter_layout(&art, meter(100), (640, 480));
    assert_eq!(
        l.bar,
        ScreenSprite::new(hud::BOSS_BAR, -1, Anchor::TopLeft, 320, 447)
    );
    // The frame covers columns 190..=449 and rows 433..=460; the band
    // starts 18 px right of its left edge and 11 px below its top.
    let full = FillRect {
        left: 208,
        top: 444,
        width: 226,
        height: 7,
        index: hud::BOSS_FULL_INDEX,
    };
    assert_eq!(l.fills, vec![full]);
    let l = hud::boss_meter_layout(&art, meter(0), (640, 480));
    assert_eq!(
        l.fills,
        vec![FillRect {
            index: hud::BOSS_EMPTY_INDEX,
            ..full
        }]
    );
    let l = hud::boss_meter_layout(&art, meter(50), (640, 480));
    assert_eq!(
        l.fills,
        vec![
            FillRect { width: 113, ..full },
            FillRect {
                left: 208 + 113,
                width: 113,
                index: hud::BOSS_EMPTY_INDEX,
                ..full
            }
        ]
    );
    // Centred in a wider picture, at the same height above the bottom.
    let l = hud::boss_meter_layout(&art, meter(100), (854, 480));
    assert_eq!((l.bar.x, l.bar.y), (427, 447));
    assert_eq!(l.fills[0].left, 427 - 130 + 18);
}

#[test]
fn the_bar_falls_with_scripted_health() {
    let (_, art) = art(true);
    let mut hud_ = Hud::new(art);
    let mut last = i32::MAX;
    for health in (0..=100).rev() {
        let h = fight(Some(meter(health)), None);
        let sprites = hud_.layout(&h, (640, 480), 0.0);
        let width = |key: &str| -> i32 {
            sprites
                .iter()
                .filter(|s| s.key == key)
                .map(|s| s.frame)
                .sum()
        };
        let (full, empty) = (width(hud::BOSS_BAND_FULL), width(hud::BOSS_BAND_EMPTY));
        assert_eq!(full + empty, 226, "health {health}");
        assert_eq!(full, hud::boss_full_width(health));
        assert!(full <= last, "never rises as health falls");
        last = full;
        // The pieces sit side by side on the band's row, full part first.
        let band: Vec<&ScreenSprite> = sprites
            .iter()
            .filter(|s| s.key == hud::BOSS_BAND_FULL || s.key == hud::BOSS_BAND_EMPTY)
            .collect();
        let mut x = 208;
        for piece in &band {
            assert_eq!((piece.x, piece.y), (x, 444), "health {health}");
            x += piece.frame;
        }
        assert_eq!(x, 208 + 226);
        assert!(band.len() <= 16);
    }
    let plain = hud_.layout(&fight(None, None), (640, 480), 0.0);
    assert!(plain.iter().all(|s| !s.key.starts_with("STATUSBAR_BOSS")));
}

#[test]
fn the_banner_and_the_bar_come_before_the_status_bar() {
    let (_, art) = art(true);
    let plain = hud::layout(&art, &fight(None, None), (640, 480), 0.0);
    let s = hud::layout(
        &art,
        &fight(Some(meter(40)), Some((320, -220))),
        (640, 480),
        0.0,
    );
    // 40%: 90 px full (64 + 16 + 8 + 2) and 136 px empty (128 + 8).
    let band = 6;
    assert_eq!(s.len(), plain.len() + 2 + band);
    assert_eq!(
        s[0],
        ScreenSprite::new(hud::BOSS_NAME, -1, Anchor::TopLeft, 320, -220)
    );
    assert_eq!(s[1].key, hud::BOSS_BAR);
    let pieces: Vec<(&str, i32)> = s[2..2 + band]
        .iter()
        .map(|p| (p.key.as_str(), p.frame))
        .collect();
    let (f, e) = (hud::BOSS_BAND_FULL, hud::BOSS_BAND_EMPTY);
    assert_eq!(
        pieces,
        vec![(f, 64), (f, 16), (f, 8), (f, 2), (e, 128), (e, 8)]
    );
    assert_eq!(&s[2 + band..], &plain[..]);
    // The banner's 640x480 position is centred in a wider picture.
    let s = hud::layout(&art, &fight(None, Some((320, 160))), (854, 480), 0.0);
    assert_eq!((s[0].x, s[0].y), (427, 160));
}

/// The pieces drawn alone by the CPU reference at `size`.
fn draw(g: &mut LevelGraphics, sprites: Vec<ScreenSprite>, size: (u32, u32)) -> cpu::Composed {
    let view = ViewConfig::default().with_render_size(size.0, size.1);
    let mut b = SceneBuilder::new(g, view, DEFAULT_MAX_TARGET);
    let input = FrameInput {
        center: (320.0, 240.0),
        sprites: Vec::new(),
        time_ms: 0.0,
        fade: None,
        screen: sprites,
        transition: None,
    };
    let scene = b.build(g, &input);
    cpu::compose(g, &scene)
}

#[test]
fn the_bar_and_the_banner_draw_offscreen_where_laid_out() {
    let (mut g, art) = art(true);
    let boss_piece = |s: &ScreenSprite| {
        [
            hud::BOSS_BAR,
            hud::BOSS_NAME,
            hud::BOSS_BAND_FULL,
            hud::BOSS_BAND_EMPTY,
        ]
        .contains(&s.key.as_str())
    };
    let (full, empty) = (hud::BOSS_FULL_INDEX, hud::BOSS_EMPTY_INDEX);
    for (size, k) in [((640, 480), 1usize), ((1280, 960), 2)] {
        let h = fight(Some(meter(60)), Some((320, 160)));
        let sprites: Vec<ScreenSprite> = hud::layout(&art, &h, (640, 480), 0.0)
            .into_iter()
            .filter(boss_piece)
            .collect();
        let c = draw(&mut g, sprites, size);
        let at = |x: usize, y: usize| c.index[(y * k + k / 2) * c.width as usize + x * k + k / 2];
        // The bar: columns 190..=449, rows 433..=460.
        assert_eq!(at(190, 433), BAR_INDEX, "{size:?}");
        assert_eq!(at(449, 460), BAR_INDEX);
        assert_eq!(at(189, 440), 0);
        assert_eq!(at(450, 440), 0);
        assert_eq!(at(300, 461), 0);
        // The band: columns 208..=433 (+18..+243 of the frame), rows
        // 444..=450 (+11..+17); at 60% its left 135 px are full.
        assert_eq!(at(208, 444), full, "{size:?}");
        assert_eq!(at(342, 450), full);
        assert_eq!(at(343, 444), empty);
        assert_eq!(at(433, 450), empty);
        assert_eq!(at(207, 447), BAR_INDEX, "left of the band");
        assert_eq!(at(434, 447), BAR_INDEX, "right of the band");
        assert_eq!(at(300, 443), BAR_INDEX, "above the band");
        assert_eq!(at(300, 451), BAR_INDEX, "below the band");
        // The banner: 200x60 centred at (320, 160).
        assert_eq!(at(220, 130), NAME_INDEX);
        assert_eq!(at(419, 189), NAME_INDEX);
        assert_eq!(at(219, 160), 0);
        assert_eq!(at(320, 190), 0);
    }
    // All full at 100, all empty at 0, every column of the band.
    for (health, want) in [(100, full), (0, empty)] {
        let h = fight(Some(meter(health)), None);
        let sprites: Vec<ScreenSprite> = hud::layout(&art, &h, (640, 480), 0.0)
            .into_iter()
            .filter(boss_piece)
            .collect();
        let c = draw(&mut g, sprites, (640, 480));
        let at = |x: usize, y: usize| c.index[y * c.width as usize + x];
        for x in 208..=433 {
            for y in 444..=450 {
                assert_eq!(at(x, y), want, "health {health} at ({x}, {y})");
            }
        }
    }
}

#[test]
fn install_level2_boss_pieces_draw_on_the_gpu_as_on_the_cpu() {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let level = LevelData::build(&store, LevelSource::Retail(2)).unwrap();
    let mut graphics = LevelGraphics::from_level(&level, &store, None);
    let art = HudArt::from_store(&mut graphics, &store, &level.registries);
    let bar = art.boss_bar.expect("the bar's art");
    assert_eq!((bar.width, bar.height), (260, 28));
    assert!(graphics.frame_info(hud::BOSS_NAME, -1).is_some());
    let gpu = Gpu::headless().expect("a Vulkan GPU");
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&graphics);
    let start = (f64::from(level.start_x), f64::from(level.start_y));
    for (size, health, banner) in [
        ((640, 480), 100, (320, -60)),
        ((1920, 1080), 55, (320, 160)),
        ((3840, 2160), 0, (700, 160)),
    ] {
        let v = ViewConfig::default().with_render_size(size.0, size.1);
        let mut b = SceneBuilder::new(&graphics, v, DEFAULT_MAX_TARGET);
        let presentation = b.composition().presentation;
        let h = fight(Some(meter(health)), Some(banner));
        let screen = hud::layout(&art, &h, presentation, 0.0);
        let input = FrameInput {
            center: start,
            sprites: Vec::new(),
            time_ms: 0.0,
            fade: None,
            screen,
            transition: None,
        };
        let scene = b.build(&mut graphics, &input);
        let want = cpu::compose(&graphics, &scene);
        let got = renderer.read_composed(&graphics, &scene);
        let bad = got
            .index
            .iter()
            .zip(&want.index)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(bad, 0, "{size:?}: {bad} pixels differ");
    }
    assert_eq!(graphics.loaded_while_drawing(), 0);
}
