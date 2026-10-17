//! The status bar on your own install: every image set and frame the
//! layout names exists, the pieces sit where the original game puts them,
//! and level 1 draws with the bar on the GPU exactly as the CPU reference
//! does. Run with `make test-install MODULE=ui` (needs `CLAW_INSTALL`)
//! under the shared bench lock; skipped otherwise.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{HudState, PowerupHud, ViewConfig, Weapon};
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{FrameInput, Gpu, LevelGraphics, Renderer, SceneBuilder, ScreenSprite, cpu};
use claw_ui::hud::{self, HudArt};

struct Level1 {
    level: LevelData,
    graphics: LevelGraphics,
    art: HudArt,
}

fn level1() -> Option<Level1> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let level = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    let mut graphics = LevelGraphics::from_level(&level, &store, None);
    let art = HudArt::from_store(&mut graphics, &store, &level.registries);
    Some(Level1 {
        level,
        graphics,
        art,
    })
}

fn states() -> Vec<HudState> {
    let mut out = Vec::new();
    for (i, weapon) in [Weapon::Pistol, Weapon::Magic, Weapon::Dynamite]
        .into_iter()
        .enumerate()
    {
        for d in 0..10u32 {
            let n = d as i32;
            out.push(HudState {
                score: d * 11_111_111,
                health: n * 111,
                lives: n + 1,
                weapon,
                pistol_ammo: n * 11,
                magic_ammo: n * 11,
                dynamite_ammo: n * 11,
                powerup: (i == 1).then_some(PowerupHud {
                    kind: 1,
                    remaining_ms: d * 111_000 + 999,
                }),
                timer_ms: (i == 2).then_some(d * 111_000),
                boss_meter: None,
                boss_banner: None,
            });
        }
    }
    out
}

#[test]
fn every_set_and_frame_the_bar_names_exists() {
    let Some(mut l1) = level1() else { return };
    assert!(l1.art.missing.is_empty(), "missing: {:?}", l1.art.missing);
    assert_eq!(l1.art.heart_frames, vec![1, 2, 3]);
    assert_eq!(l1.art.stopwatch.frames.len(), 16);
    for key in hud::IMAGE_SETS {
        assert!(l1.graphics.is_loaded(key), "{key} was not preloaded");
    }
    let mut checked = 0;
    let times: Vec<f64> = (0..400).map(|i| f64::from(i) * 37.0).collect();
    for h in states() {
        for &t in &times {
            for s in hud::layout(&l1.art, &h, (854, 480), t) {
                assert!(
                    l1.graphics.frame_info(&s.key, s.frame).is_some(),
                    "{} frame {} at {t} ms",
                    s.key,
                    s.frame
                );
                checked += 1;
            }
        }
    }
    // Every glyph the messages use.
    for id in [hud::MESSAGE_SAVED, hud::MESSAGE_NOT_SAVED] {
        let text = hud::message_text(id).unwrap();
        let sprites = hud::text_layout(&l1.art, text);
        let shown = text.chars().filter(|&c| hud::glyph(c) != 0).count();
        assert_eq!(sprites.len(), shown, "{text}");
        for s in sprites {
            assert!(l1.graphics.frame_info(&s.key, s.frame).is_some());
        }
    }
    assert_eq!(l1.graphics.loaded_while_drawing(), 0);
    eprintln!("{checked} status bar pieces resolved");
}

/// Left and right columns (inclusive) of a piece's box at 640x480.
fn box_x(g: &mut LevelGraphics, s: &ScreenSprite) -> (i32, i32) {
    let f = g.frame_info(&s.key, s.frame).unwrap();
    let (ox, _) = s.anchor.origin((640, 480));
    let left = ox + s.x - (f.width >> 1) as i32 + f.offset_x;
    (left, left + f.width as i32 - 1)
}

#[test]
fn pieces_sit_where_the_original_puts_them() {
    let Some(mut l1) = level1() else { return };
    let art = &l1.art;
    assert_eq!(
        (
            art.chest_x,
            art.heart_x,
            art.weapon_x,
            art.head_x,
            art.stopwatch_x
        ),
        (14, -30, -25, -17, 9)
    );
    // All digits '0' and a running powerup.
    let h = HudState {
        score: 0,
        health: 0,
        lives: 1,
        weapon: Weapon::Pistol,
        pistol_ammo: 0,
        powerup: Some(PowerupHud {
            kind: 1,
            remaining_ms: 500,
        }),
        ..HudState::default()
    };
    let s = hud::layout(art, &h, (640, 480), 0.0);
    let g = &mut l1.graphics;
    let span = |g: &mut LevelGraphics, key: &str, skip: usize, n: usize| {
        let pieces: Vec<&ScreenSprite> = s
            .iter()
            .filter(|p| p.key == key)
            .skip(skip)
            .take(n)
            .collect();
        assert_eq!(pieces.len(), n, "{key}");
        let first = box_x(g, pieces[0]);
        let last = box_x(g, pieces[n - 1]);
        (first.0, last.1)
    };
    assert_eq!(span(g, hud::CHEST, 0, 1).0, 0);
    assert_eq!(span(g, hud::SCORE_DIGITS, 0, 8), (30, 118));
    assert_eq!(span(g, hud::HEART, 0, 1), (574, 641));
    assert_eq!(span(g, hud::HEALTH_DIGITS, 0, 3), (581, 614));
    assert_eq!(span(g, hud::PISTOL, 0, 1), (588, 639));
    assert_eq!(span(g, hud::SMALL_DIGITS, 0, 2), (592, 607));
    assert_eq!(span(g, hud::HEAD, 0, 1), (598, 641));
    assert_eq!(span(g, hud::SMALL_DIGITS, 2, 1), (602, 609));
    assert_eq!(span(g, hud::STOPWATCH, 0, 1).0, 0);
    assert_eq!(span(g, hud::SCORE_DIGITS, 8, 3), (30, 63));
}

#[test]
fn level1_draws_with_the_bar_on_the_gpu() {
    let Some(mut l1) = level1() else { return };
    let gpu = Gpu::headless().expect("a Vulkan GPU");
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&l1.graphics);
    let start = (f64::from(l1.level.start_x), f64::from(l1.level.start_y));
    let h = HudState {
        score: 4_250,
        health: 73,
        lives: 6,
        weapon: Weapon::Magic,
        pistol_ammo: 10,
        magic_ammo: 5,
        dynamite_ammo: 3,
        powerup: Some(PowerupHud {
            kind: 2,
            remaining_ms: 12_345,
        }),
        timer_ms: None,
        boss_meter: None,
        boss_banner: None,
    };
    for size in [(640, 480), (1920, 1080), (5120, 1440), (3840, 2160)] {
        let v = ViewConfig::default().with_render_size(size.0, size.1);
        let mut b = SceneBuilder::new(&l1.graphics, v, DEFAULT_MAX_TARGET);
        let presentation = b.composition().presentation;
        let mut screen = hud::layout(&l1.art, &h, presentation, 1234.0);
        screen.extend(hud::text_layout(
            &l1.art,
            hud::message_text(hud::MESSAGE_SAVED).unwrap(),
        ));
        let input = FrameInput {
            center: start,
            sprites: Vec::new(),
            time_ms: 1234.0,
            fade: None,
            screen,
            transition: None,
        };
        let scene = b.build(&mut l1.graphics, &input);
        assert!(scene.sprites.len() > 30, "{size:?}");
        let want = cpu::compose(&l1.graphics, &scene);
        let got = renderer.read_composed(&l1.graphics, &scene);
        let bad = got
            .index
            .iter()
            .zip(&want.index)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(bad, 0, "{size:?}: {bad} pixels differ");
    }
}
