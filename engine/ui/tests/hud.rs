//! The status bar and messages on synthetic art built in code: positions,
//! digits, the heart timing, the stopwatch, messages, and the bar drawn by
//! the CPU reference at the common output sizes.

use claw_assets::pid::FLAG_COLOR_KEY;
use claw_assets::{Ani, AniFrame, Frame, Palette};
use claw_frame::{GameEvent, HudState, PowerupHud, ViewConfig, Weapon};
use claw_render::effects::LevelTables;
use claw_render::scene::DEFAULT_MAX_TARGET;
use claw_render::{
    Anchor, FrameInput, LevelGraphics, MemoryImages, SceneBuilder, ScreenSprite, cpu,
};
use claw_ui::hud::{self, Hud, HudArt, Messages, Placement};

/// An opaque frame of one index.
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

/// Index of score digit d.
fn score_index(d: i32) -> u8 {
    10 + d as u8
}
/// Index of health digit d.
fn health_index(d: i32) -> u8 {
    30 + d as u8
}
/// Index of small digit d.
fn small_index(d: i32) -> u8 {
    50 + d as u8
}
/// Index of heart frame n.
fn heart_index(n: i32) -> u8 {
    70 + n as u8
}

fn images() -> MemoryImages {
    let mut m = MemoryImages::default();
    for n in 1..=12 {
        m.insert(hud::CHEST, n, solid(28, 22, 100 + n as u8, (0, 0)));
    }
    for d in 0..10 {
        m.insert(hud::SCORE_DIGITS, d, solid(12, 14, score_index(d), (0, 0)));
        m.insert(
            hud::HEALTH_DIGITS,
            d,
            solid(12, 14, health_index(d), (0, 0)),
        );
    }
    for d in 0..11 {
        m.insert(hud::SMALL_DIGITS, d, solid(8, 10, small_index(d), (-1, 0)));
    }
    for n in 1..=3 {
        m.insert(hud::HEART, n, solid(68, 24, heart_index(n), (-1, 0)));
    }
    for (k, key) in [hud::PISTOL, hud::MAGIC, hud::DYNAMITE].iter().enumerate() {
        for n in 1..=6 {
            m.insert(
                key,
                n,
                solid(52, 20, 120 + (k * 10) as u8 + n as u8, (0, 0)),
            );
        }
    }
    for n in 1..=8 {
        m.insert(hud::HEAD, n, solid(44, 26, 160 + n as u8, (-2, 0)));
    }
    for n in 1..=16 {
        m.insert(hud::STOPWATCH, n, solid(20, 27, 180 + n as u8, (1, 0)));
    }
    m.insert(hud::TIMER, 1, solid(22, 22, 199, (0, 0)));
    for n in 0..=hud::FONT_GLYPHS {
        // Keyed glyphs: index 0 shows what is below.
        let mut f = solid(4 + (n % 5) as u32, 10, 200 + (n % 50) as u8, (0, 0));
        f.flags = FLAG_COLOR_KEY;
        m.insert(hud::FONT, n, f);
    }
    m
}

fn animation(key: &str) -> Option<Ani> {
    let frames: Vec<(i16, i16)> = match key {
        // A pause (no image) first: the first frame shows.
        hud::CHEST_ANIMATION => vec![(0, 500), (1, 500), (2, 100), (99, 50), (3, 50)],
        hud::PISTOL_ANIMATION => vec![(1, 1500), (2, 150), (3, 150)],
        hud::MAGIC_ANIMATION => vec![(1, 1500), (4, 125)],
        hud::DYNAMITE_ANIMATION => vec![(2, 75), (3, 75)],
        hud::HEAD_ANIMATION => vec![(1, 2500), (2, 700)],
        hud::STOPWATCH_ANIMATION => vec![(0, 100)],
        _ => return None,
    };
    Some(Ani {
        frames: frames
            .into_iter()
            .map(|(image_index, duration)| AniFrame {
                image_index,
                duration,
                ..AniFrame::default()
            })
            .collect(),
        ..Ani::default()
    })
}

fn graphics() -> LevelGraphics {
    let mut pal = Palette::default();
    for (i, c) in pal.0.iter_mut().enumerate() {
        *c = [i as u8, (i * 3 % 256) as u8, 255 - i as u8];
    }
    let tables = LevelTables::blank(&pal);
    LevelGraphics::new(Vec::new(), true, pal, tables, Box::new(images()), None)
}

fn art() -> (LevelGraphics, HudArt) {
    let mut g = graphics();
    let art = HudArt::load(&mut g, animation);
    (g, art)
}

fn state() -> HudState {
    HudState {
        score: 12_345_678,
        health: 87,
        lives: 4,
        weapon: Weapon::Pistol,
        pistol_ammo: 9,
        magic_ammo: 25,
        dynamite_ammo: 3,
        powerup: None,
        timer_ms: None,
        boss_meter: None,
        boss_banner: None,
    }
}

type Piece = (String, i32, Anchor, i32, i32);

fn pieces(s: &[ScreenSprite]) -> Vec<Piece> {
    s.iter()
        .map(|p| (p.key.clone(), p.frame, p.anchor, p.x, p.y))
        .collect()
}

fn piece(key: &str, frame: i32, anchor: Anchor, x: i32, y: i32) -> Piece {
    (key.to_string(), frame, anchor, x, y)
}

#[test]
fn offsets_come_from_the_first_frames() {
    let (_, art) = art();
    assert!(art.missing.is_empty(), "{:?}", art.missing);
    assert_eq!(art.chest_x, 14);
    assert_eq!(art.heart_x, -30);
    assert_eq!(art.weapon_x, -25);
    assert_eq!(art.head_x, -17);
    assert_eq!(art.stopwatch_x, 9);
    assert_eq!(art.timer_x, 11);
    assert_eq!(art.heart_frames, vec![1, 2, 3]);
    assert_eq!(art.stopwatch.frames.len(), 16);
    assert_eq!(art.stopwatch.frames[0], (1, 100));
}

#[test]
fn the_default_layout_at_640x480() {
    let (_, art) = art();
    let got = pieces(&hud::layout(&art, &state(), (640, 480), 0.0));
    let (tl, tr) = (Anchor::TopLeft, Anchor::TopRight);
    let mut want = vec![piece(hud::CHEST, 1, tl, 14, 15)];
    for (i, d) in [1, 2, 3, 4, 5, 6, 7, 8].into_iter().enumerate() {
        want.push(piece(hud::SCORE_DIGITS, d, tl, 36 + 11 * i as i32, 15));
    }
    want.push(piece(hud::HEART, 1, tr, -30, 15));
    for (i, d) in [0, 8, 7].into_iter().enumerate() {
        want.push(piece(hud::HEALTH_DIGITS, d, tr, -52 + 11 * i as i32, 14));
    }
    want.push(piece(hud::PISTOL, 1, tr, -25, 46));
    want.push(piece(hud::SMALL_DIGITS, 0, tr, -42, 46));
    want.push(piece(hud::SMALL_DIGITS, 9, tr, -34, 46));
    want.push(piece(hud::HEAD, 1, tr, -17, 72));
    want.push(piece(hud::SMALL_DIGITS, 3, tr, -32, 73));
    assert_eq!(got, want);
}

fn frames_of(s: &[ScreenSprite], key: &str) -> Vec<i32> {
    s.iter().filter(|p| p.key == key).map(|p| p.frame).collect()
}

#[test]
fn numbers_are_zero_padded_low_order_digits() {
    let (_, art) = art();
    let at = |h: HudState| hud::layout(&art, &h, (640, 480), 0.0);
    let s = at(HudState {
        score: 123,
        ..state()
    });
    assert_eq!(
        frames_of(&s, hud::SCORE_DIGITS),
        vec![0, 0, 0, 0, 0, 1, 2, 3]
    );
    let s = at(HudState {
        score: 1_234_567_890,
        ..state()
    });
    assert_eq!(
        frames_of(&s, hud::SCORE_DIGITS),
        vec![3, 4, 5, 6, 7, 8, 9, 0]
    );
    let s = at(HudState {
        health: -5,
        ..state()
    });
    assert_eq!(frames_of(&s, hud::HEALTH_DIGITS), vec![0, 0, 0]);
    let s = at(HudState {
        health: 1234,
        ..state()
    });
    assert_eq!(frames_of(&s, hud::HEALTH_DIGITS), vec![2, 3, 4]);
    // Ammunition of the selected weapon, then the spare lives.
    for (weapon, key, want) in [
        (Weapon::Pistol, hud::PISTOL, vec![0, 9, 3]),
        (Weapon::Magic, hud::MAGIC, vec![2, 5, 3]),
        (Weapon::Dynamite, hud::DYNAMITE, vec![0, 3, 3]),
    ] {
        let s = at(HudState { weapon, ..state() });
        assert_eq!(frames_of(&s, hud::SMALL_DIGITS), want, "{weapon:?}");
        assert_eq!(frames_of(&s, key).len(), 1);
    }
    let s = at(HudState {
        pistol_ammo: 150,
        lives: 0,
        ..state()
    });
    assert_eq!(frames_of(&s, hud::SMALL_DIGITS), vec![5, 0, 0]);
    let s = at(HudState {
        pistol_ammo: -3,
        lives: 10,
        ..state()
    });
    assert_eq!(frames_of(&s, hud::SMALL_DIGITS), vec![0, 0, 9]);
    let s = at(HudState {
        lives: 12,
        ..state()
    });
    assert_eq!(frames_of(&s, hud::SMALL_DIGITS)[2], 1);
    assert_eq!(hud::digits(7, 3), vec![0, 0, 7]);
    assert_eq!(hud::digits(98_765, 2), vec![6, 5]);
}

#[test]
fn heart_steps_by_health() {
    assert_eq!(hud::heart_step_ms(0), 300);
    assert_eq!(hud::heart_step_ms(50), 200);
    assert_eq!(hud::heart_step_ms(100), 100);
    assert_eq!(hud::heart_step_ms(150), 100);
    assert_eq!(hud::heart_step_ms(-20), 300);
    let f = [1, 2, 3];
    let at = |health, ms| hud::heart_frame(&f, health, ms);
    assert_eq!(
        [0.0, 299.9, 300.0, 600.0, 899.0, 900.0].map(|t| at(0, t)),
        [1, 1, 2, 3, 3, 1]
    );
    assert_eq!(
        [0.0, 99.0, 100.0, 250.0, 300.0].map(|t| at(100, t)),
        [1, 1, 2, 3, 1]
    );
    assert_eq!(at(50, 450.0), 3);
    assert_eq!(hud::heart_frame(&[], 50, 450.0), -1);
    let (_, art) = art();
    let heart = |health, t| {
        let h = HudState { health, ..state() };
        frames_of(&hud::layout(&art, &h, (640, 480), t), hud::HEART)
    };
    assert_eq!(heart(87, 0.0), vec![1]);
    // 300 - 174 = 126 ms per step.
    assert_eq!(heart(87, 126.0), vec![2]);
    assert_eq!(heart(87, 3.0 * 126.0), vec![1]);
}

#[test]
fn animations_run_on_time_and_the_weapon_restarts_on_a_change() {
    let (_, art) = art();
    let chest = |t| frames_of(&hud::layout(&art, &state(), (640, 480), t), hud::CHEST);
    assert_eq!(chest(999.0), vec![1]);
    assert_eq!(chest(1000.0), vec![2]);
    assert_eq!(chest(1120.0), vec![2], "a pause keeps the frame before it");
    assert_eq!(chest(1150.0), vec![3]);
    assert_eq!(
        chest(0.0),
        vec![1],
        "a pause at the start shows the first frame"
    );
    assert_eq!(chest(1200.0), vec![1]);
    let mut h = Hud::new(art.clone());
    let icon = |h: &mut Hud, w, t| {
        let s = HudState {
            weapon: w,
            ..state()
        };
        let key = [hud::PISTOL, hud::MAGIC, hud::DYNAMITE][w as usize];
        frames_of(&h.layout(&s, (640, 480), t), key)
    };
    assert_eq!(icon(&mut h, Weapon::Pistol, 5000.0), vec![1]);
    assert_eq!(icon(&mut h, Weapon::Pistol, 6550.0), vec![2]);
    // Switching starts the new weapon's animation from its first frame.
    assert_eq!(icon(&mut h, Weapon::Magic, 7000.0), vec![1]);
    assert_eq!(icon(&mut h, Weapon::Magic, 8500.0), vec![4]);
    assert_eq!(icon(&mut h, Weapon::Pistol, 9000.0), vec![1]);
}

#[test]
fn stopwatch_shows_only_while_a_powerup_runs() {
    let (_, art) = art();
    let at = |powerup, t| {
        let h = HudState { powerup, ..state() };
        hud::layout(&art, &h, (640, 480), t)
    };
    assert!(frames_of(&at(None, 0.0), hud::STOPWATCH).is_empty());
    let none_left = Some(PowerupHud {
        kind: 3,
        remaining_ms: 0,
    });
    assert!(frames_of(&at(none_left, 0.0), hud::STOPWATCH).is_empty());
    let running = Some(PowerupHud {
        kind: 3,
        remaining_ms: 29_999,
    });
    let s = at(running, 250.0);
    let tail = pieces(&s[s.len() - 4..]);
    let tl = Anchor::TopLeft;
    assert_eq!(
        tail,
        vec![
            piece(hud::STOPWATCH, 3, tl, 9, 48),
            piece(hud::SCORE_DIGITS, 0, tl, 36, 49),
            piece(hud::SCORE_DIGITS, 2, tl, 47, 49),
            piece(hud::SCORE_DIGITS, 9, tl, 58, 49),
        ]
    );
    assert_eq!(frames_of(&at(running, 1600.0), hud::STOPWATCH), vec![1]);
    // The countdown clock shows in the same place when no powerup runs.
    let h = HudState {
        timer_ms: Some(65_000),
        ..state()
    };
    let s = hud::layout(&art, &h, (640, 480), 0.0);
    let tail = pieces(&s[s.len() - 4..]);
    assert_eq!(tail[0], piece(hud::TIMER, 1, tl, 11, 48));
    assert_eq!(
        frames_of(&s[s.len() - 3..], hud::SCORE_DIGITS),
        vec![0, 6, 5]
    );
}

#[test]
fn the_safe_area_keeps_the_bar_in_the_middle() {
    let (_, art) = art();
    let s = hud::layout_with(
        &art,
        &state(),
        (854, 480),
        0.0,
        0.0,
        Placement { safe_area: true },
    );
    // 214 extra columns: 107 on each side.
    assert_eq!((s[0].anchor, s[0].x), (Anchor::TopLeft, 14 + 107));
    let heart = s.iter().find(|p| p.key == hud::HEART).unwrap();
    assert_eq!((heart.anchor, heart.x), (Anchor::TopRight, -30 - 107));
    let plain = hud::layout(&art, &state(), (854, 480), 0.0);
    assert_eq!(plain[0].x, 14);
}

#[test]
fn messages_replace_each_other_and_age() {
    let mut m = Messages::default();
    assert!(!m.event(&GameEvent::Message { id: 77, ms: 5000 }));
    assert!(!m.event(&GameEvent::GameOver));
    assert_eq!(m.text(), None);
    assert!(m.event(&GameEvent::Message {
        id: hud::MESSAGE_SAVED,
        ms: 5000
    }));
    assert_eq!(m.text(), Some("Your game has been saved"));
    m.advance(4000.0);
    assert!(m.event(&GameEvent::Message {
        id: hud::MESSAGE_NOT_SAVED,
        ms: 5000
    }));
    // The new message gets its whole time.
    m.advance(4999.0);
    assert_eq!(
        m.text(),
        Some("Your game was not saved because you cheated!")
    );
    m.advance(1.0);
    assert_eq!(m.text(), None);
    m.show("", 1000);
    assert_eq!(m.text(), None);
}

#[test]
fn glyphs_follow_the_font_map() {
    assert_eq!(hud::glyph(' '), 0);
    assert_eq!(hud::glyph('A'), 1);
    assert_eq!(hud::glyph('z'), 26);
    assert_eq!(hud::glyph('0'), 27);
    assert_eq!(hud::glyph('9'), 36);
    assert_eq!(hud::glyph(','), 37);
    assert_eq!(hud::glyph('!'), 39);
    assert_eq!(hud::glyph('*'), 54);
    assert_eq!(hud::glyph('~'), 0);
    assert_eq!(hud::glyph('é'), 0);
}

#[test]
fn text_is_centred_with_its_top_at_110() {
    let (_, art) = art();
    // Glyph widths are 4 + n % 5: 'A' (1) 5, 'B' (2) 6, space (0) 4.
    let s = hud::text_layout(&art, "AB A");
    let w = 5 + 6 + 4 + 5;
    let left = -(w / 2);
    assert_eq!(
        pieces(&s),
        vec![
            piece(hud::FONT, 1, Anchor::TopCenter, left + 2, 115),
            piece(hud::FONT, 2, Anchor::TopCenter, left + 5 + 3, 115),
            piece(hud::FONT, 1, Anchor::TopCenter, left + 15 + 2, 115),
        ]
    );
    assert!(hud::text_layout(&art, "").is_empty());
    let mut h = Hud::new(art);
    assert!(h.message_layout().is_empty());
    h.messages.show("HI", 100);
    assert_eq!(h.message_layout().len(), 2);
}

#[test]
fn the_aid_marker_names_the_aids_in_the_bottom_left_corner() {
    let (_, art) = art();
    assert!(hud::marker_lines(false, false).is_empty());
    assert_eq!(hud::marker_lines(true, false), ["INVINCIBLE"]);
    assert_eq!(hud::marker_lines(false, true), ["INFINITE LIVES"]);
    assert_eq!(
        hud::marker_lines(true, true),
        ["INVINCIBLE", "INFINITE LIVES"]
    );
    let mut h = Hud::new(art.clone());
    assert!(
        h.aid_marker_layout(false, false, (640, 480)).is_empty(),
        "nothing with both aids off"
    );
    // Glyphs 10 px high with their hotspot in the middle: a glyph whose box
    // ends on the row 6 px above the bottom edge has its hotspot at -11.
    // 'I' (9) is 8 px wide, 'N' (14) 8, 'V' (22) 6.
    let one = h.aid_marker_layout(true, false, (640, 480));
    assert_eq!(one.len(), "INVINCIBLE".len());
    assert_eq!(
        pieces(&one[..3]),
        vec![
            piece(hud::FONT, 9, Anchor::BottomLeft, 6 + 4, -11),
            piece(hud::FONT, 14, Anchor::BottomLeft, 6 + 8 + 4, -11),
            piece(hud::FONT, 22, Anchor::BottomLeft, 6 + 16 + 3, -11),
        ]
    );
    // Two lines: the first one line (10 px and a 2 px gap) higher; the
    // space between the words takes its width but draws nothing.
    let two = h.aid_marker_layout(true, true, (640, 480));
    assert_eq!(two.len(), "INVINCIBLE".len() + "INFINITELIVES".len());
    assert!(two[..10].iter().all(|p| p.y == -23), "{two:?}");
    assert!(two[10..].iter().all(|p| p.y == -11), "{two:?}");
    assert_eq!(two[10].x, 6 + 4, "each line starts at the left margin");
    // Kept in the centred 4:3 area with the status bar.
    h.placement = Placement { safe_area: true };
    let kept = h.aid_marker_layout(true, false, (854, 480));
    assert_eq!(kept[0].x, 6 + 4 + 107);
    assert_eq!(
        hud::marker_layout(&art, &["I"], (854, 480), Placement::default())[0].x,
        6 + 4
    );
}

const SIZES: [(u32, u32); 5] = [
    (640, 480),
    (1920, 1080),
    (3440, 1440),
    (5120, 1440),
    (3840, 2160),
];

/// The status bar drawn alone by the CPU reference at `size`.
fn draw(
    g: &mut LevelGraphics,
    sprites: Vec<ScreenSprite>,
    size: (u32, u32),
) -> (claw_render::Scene, cpu::Composed) {
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
    let c = cpu::compose(g, &scene);
    (scene, c)
}

#[test]
fn the_bar_draws_crisp_at_every_size() {
    let (mut g, art) = art();
    let h = HudState {
        powerup: Some(PowerupHud {
            kind: 1,
            remaining_ms: 41_000,
        }),
        ..state()
    };
    for size in SIZES {
        let (pw, ph) = ViewConfig::default()
            .with_render_size(size.0, size.1)
            .presentation_size();
        let sprites = hud::layout(&art, &h, (pw, ph), 0.0);
        // Every hotspot lies inside the presentation.
        for s in &sprites {
            let (ox, oy) = s.anchor.origin((pw, ph));
            let (x, y) = (ox + s.x, oy + s.y);
            assert!(
                (0..pw as i32).contains(&x) && (0..ph as i32).contains(&y),
                "{size:?}: {s:?} at ({x}, {y})"
            );
        }
        let (scene, c) = draw(&mut g, sprites.clone(), size);
        let k = scene.comp.scale;
        assert_eq!(scene.comp.presentation, (pw, ph));
        assert_eq!(scene.sprites.len(), sprites.len(), "{size:?}");
        assert!(scene.sprites.iter().all(|s| s.scale == k), "{size:?}");
        let r = pw as i64 - 1;
        let at = |x: i64, y: i64| {
            let k = i64::from(k);
            let (tx, ty) = (x * k + k / 2, y * k + k / 2);
            c.index[(ty * i64::from(c.width) + tx) as usize]
        };
        // The digits show at their hotspots.
        for (i, d) in [1, 2, 3, 4, 5, 6, 7, 8].into_iter().enumerate() {
            assert_eq!(
                at(36 + 11 * i as i64, 15),
                score_index(d),
                "{size:?} score {i}"
            );
        }
        for (i, d) in [0, 8, 7].into_iter().enumerate() {
            assert_eq!(at(r - 52 + 11 * i as i64, 14), health_index(d), "{size:?}");
        }
        assert_eq!(at(r - 42, 46), small_index(0));
        assert_eq!(at(r - 34, 46), small_index(9));
        assert_eq!(at(r - 32, 73), small_index(3));
        for (i, d) in [0, 4, 1].into_iter().enumerate() {
            assert_eq!(at(36 + 11 * i as i64, 49), score_index(d), "{size:?}");
        }
        // The chest starts at the left edge; the heart covers the right
        // edge, cut there.
        assert_eq!(at(0, 5), 101);
        assert_eq!(at(r - 62, 5), heart_index(1));
        assert_eq!(at(r, 5), heart_index(1));
        assert_eq!(at(r - 66, 5), 0, "left of the heart");
        // The weapon icon ends exactly at the right edge.
        assert_eq!(at(r, 40), 121);
        assert_eq!(at(r - 51, 40), 121);
    }
}

#[test]
fn the_heart_animates_in_the_picture() {
    let (mut g, art) = art();
    for (health, t, frame) in [
        (0, 650.0, 3),
        (100, 650.0, 1),
        (50, 650.0, 1),
        (50, 450.0, 3),
    ] {
        let h = HudState { health, ..state() };
        let (_, c) = draw(&mut g, hud::layout(&art, &h, (854, 480), t), (1920, 1080));
        // Scale 2; heart pixel (853 - 62, 5).
        let (x, y) = ((853 - 62) * 2, 5 * 2);
        assert_eq!(
            c.index[y * c.width as usize + x],
            heart_index(frame),
            "health {health} at {t}"
        );
    }
}
