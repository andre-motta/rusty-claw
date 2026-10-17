//! The screen layer, standalone screens and screen transitions on the CPU
//! reference, at the common output sizes.

mod common;

use claw_assets::{Frame, Palette};
use claw_frame::{
    AspectPolicy, PaletteFade, Point, ScaleMode, ScreenTransition, TICK_MS, TransitionKind,
    ViewConfig,
};
use claw_render::scene::{DEFAULT_MAX_TARGET, Item};
use claw_render::screen::{self, Mask, circle_mask, circle_steps};
use claw_render::{
    Anchor, FrameInput, SceneBuilder, ScreenBuilder, ScreenScene, ScreenSprite, cpu,
};
use common::*;

const SIZES: [(u32, u32); 5] = [
    (640, 480),
    (1920, 1080),
    (3440, 1440),
    (5120, 1440),
    (3840, 2160),
];

const ANCHORS: [Anchor; 6] = [
    Anchor::TopLeft,
    Anchor::TopRight,
    Anchor::BottomLeft,
    Anchor::BottomRight,
    Anchor::TopCenter,
    Anchor::Center,
];

fn views(size: (u32, u32)) -> Vec<ViewConfig> {
    vec![
        view(size.0, size.1),
        view_with(size.0, size.1, AspectPolicy::Pillarbox, ScaleMode::Fit),
        view_with(
            size.0,
            size.1,
            AspectPolicy::FrozenEdgeCentered,
            ScaleMode::Integer,
        ),
    ]
}

fn screen_only<'a>(sprites: Vec<ScreenSprite>) -> FrameInput<'a> {
    FrameInput {
        center: (1000.0, 400.0),
        sprites: Vec::new(),
        time_ms: 0.0,
        fade: None,
        screen: sprites,
        transition: None,
    }
}

#[test]
fn anchors_land_inside_the_presentation_at_the_composition_scale() {
    let mut g = screen_graphics();
    for size in SIZES {
        for v in views(size) {
            let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
            let comp = *b.composition();
            let (pw, ph) = comp.presentation;
            let k = comp.scale;
            let dots: Vec<ScreenSprite> = ANCHORS
                .iter()
                .map(|&a| ScreenSprite::new("DOT", 1, a, 0, 0))
                .collect();
            let scene = b.build(&mut g, &screen_only(dots));
            assert_eq!(scene.sprites.len(), ANCHORS.len(), "{size:?}");
            let c = cpu::compose(&g, &scene);
            for (s, a) in scene.sprites.iter().zip(ANCHORS) {
                let (ox, oy) = a.origin((pw, ph));
                assert!((0..pw as i32).contains(&ox) && (0..ph as i32).contains(&oy));
                // A 1x1 dot's box is its hotspot.
                assert_eq!(s.pos, [ox * k as i32, oy * k as i32], "{size:?} {a:?}");
                assert_eq!(
                    s.scale, k,
                    "{size:?}: the screen layer uses the composition scale"
                );
                let (x0, y0, x1, y1) = s.quad();
                assert!(x0 >= 0 && y0 >= 0, "{size:?} {a:?}");
                assert!(x1 <= i64::from(comp.target.0) && y1 <= i64::from(comp.target.1));
                for y in y0..y1 {
                    for x in x0..x1 {
                        assert_eq!(c.index[(y * i64::from(c.width) + x) as usize], 222);
                    }
                }
            }
        }
    }
}

#[test]
fn screen_sprites_draw_after_the_world_from_their_hotspots() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(1280, 960), DEFAULT_MAX_TARGET);
    let mut f = busy_frame((1000.0, 400.0));
    let world = b.build(&mut g, &f);
    f.screen = screen_sprites();
    let both = b.build(&mut g, &f);
    let n = world.sprites.len();
    assert_eq!(both.items[..world.items.len()], world.items[..]);
    let tail: Vec<Item> = both.items[world.items.len()..].to_vec();
    // MISSING and the sprite fully off the picture are dropped.
    let shown = screen_sprites().len() - 2;
    assert_eq!(
        tail,
        (n..n + shown)
            .map(|i| Item::Sprite(i as u32))
            .collect::<Vec<_>>()
    );
    // PANEL 1 at TopLeft (5, 4): 9x7 with offset (1, -1): box from (5 - 4
    // + 1, 4 - 3 - 1) = (2, 0), at scale 2.
    assert_eq!(both.sprites[n].pos, [4, 0]);
    assert_eq!(both.sprites[n].size, [9, 7]);
    // PANEL 1 at TopRight (-6, 5) in a 640-wide presentation: box from
    // (639 - 6 - 4 + 1, 5 - 3 - 1) = (630, 1), at scale 2.
    assert_eq!(both.sprites[n + 3].pos, [1260, 2]);
    let c = cpu::compose(&g, &both);
    assert_eq!(c.index[0], 222, "the dot at the top-left corner");
    // The panel's first pixel at target (4, 0), and its second column.
    assert_eq!(c.index[4], 230);
    assert_eq!(c.index[6], 231);
    // Normal screen sprites carry no replacement colour without a pack.
    assert!(both.sprites[n..].iter().all(|s| s.flags == 0));
}

fn closing(start_tick: u64, ms: u32, center: (f64, f64)) -> ScreenTransition {
    ScreenTransition {
        kind: TransitionKind::Circle,
        closing: true,
        start_tick,
        duration_ms: ms,
        center: Point {
            x: center.0,
            y: center.1,
        },
    }
}

#[test]
fn circle_masks_at_every_size() {
    let mut g = screen_graphics();
    let start = 100u64;
    let t0 = start as f64 * f64::from(TICK_MS);
    for size in SIZES {
        let v = view(size.0, size.1);
        let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
        let comp = *b.composition();
        let (pw, ph) = comp.presentation;
        let k = comp.scale;
        // With no main plane the presentation is centred on the camera
        // point: level (1000, 400) is presentation (pw / 2, 240).
        let (cx, cy) = ((pw / 2) as i32, 240);
        let mut at = |t: ScreenTransition, ms: f64| {
            let mut f = screen_only(Vec::new());
            f.time_ms = t0 + ms;
            f.transition = Some(t);
            b.build(&mut g, &f).mask
        };
        let close = closing(start, 1250, (1000.0, 400.0));
        let n = circle_steps(cx, cy, (pw, ph));
        assert_eq!(n, ((pw / 2) as f64).hypot(240.0) as i32, "{size:?}");
        assert_eq!(at(close, -18.0), None, "{size:?}: not started");
        assert_eq!(at(close, 0.0), Some(circle_mask(cx, cy, n, k)));
        let half = at(close, 625.0).unwrap();
        assert_eq!(half, circle_mask(cx, cy, n - n / 2, k));
        let r = i64::from(n - n / 2);
        let (kx, tx, ty) = (i64::from(k), i64::from(cx), i64::from(cy));
        // In target pixels: the centre and the pixels just inside show,
        // those just outside do not.
        assert!(half.shows(tx * kx, ty * kx));
        assert!(half.shows((tx + r - 1) * kx, ty * kx));
        assert!(!half.shows((tx + r + 1) * kx, ty * kx));
        assert!(!half.shows(tx * kx, (ty - r - 1) * kx));
        assert_eq!(at(close, 1250.0), Some(Mask::Black));
        assert_eq!(at(close, 5000.0), Some(Mask::Black));
        let open = ScreenTransition {
            closing: false,
            duration_ms: 1000,
            ..close
        };
        assert_eq!(
            at(open, -500.0),
            Some(Mask::Black),
            "{size:?}: the hold is black"
        );
        assert_eq!(at(open, 500.0), Some(circle_mask(cx, cy, n / 2, k)));
        assert_eq!(at(open, 1000.0), None);
        // Off the picture: a closing circle is pulled in, an opening one
        // shows the picture at once.
        let far = closing(start, 1250, (1000.0 + f64::from(pw), 400.0 - 400.0));
        let edge = (pw as i32 - 32, 32);
        assert_eq!(
            at(far, 0.0),
            Some(circle_mask(
                edge.0,
                edge.1,
                circle_steps(edge.0, edge.1, (pw, ph)),
                k
            ))
        );
        let far_open = ScreenTransition {
            closing: false,
            ..far
        };
        assert_eq!(at(far_open, -10.0), None);
        assert_eq!(at(far_open, 10.0), None);
    }
}

#[test]
fn a_transition_drawn_between_ticks_is_timed_by_the_presentation() {
    // A death's circles are drawn while no game time passes: the frame
    // stays at the tick of the death (here tick 5388, at 97002 ms) and the
    // presentation says how far in the circle is.
    let mut g = screen_graphics();
    let v = view(640, 480);
    let mut b = SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET);
    let comp = *b.composition();
    let (pw, ph) = comp.presentation;
    let k = comp.scale;
    let (cx, cy) = ((pw / 2) as i32, 240);
    let n = circle_steps(cx, cy, (pw, ph));
    let frame_ms = 5389.0 * f64::from(TICK_MS);
    // The simulation's transition started at the tick of the death.
    let close = closing(5388, 1250, (1000.0, 400.0));
    let mut at = |t: &ScreenTransition, elapsed: f64| {
        let (t, time) = screen::transition_at(t, frame_ms, elapsed);
        assert_eq!(t.start_tick, 5389);
        assert_eq!(time, frame_ms + elapsed);
        let mut f = screen_only(Vec::new());
        f.time_ms = time;
        f.transition = Some(t);
        b.build(&mut g, &f).mask
    };
    assert_eq!(at(&close, 0.0), Some(circle_mask(cx, cy, n, k)));
    assert_eq!(at(&close, 625.0), Some(circle_mask(cx, cy, n - n / 2, k)));
    assert_eq!(at(&close, 1250.0), Some(Mask::Black));
    let open = ScreenTransition {
        closing: false,
        duration_ms: 1000,
        ..close
    };
    assert_eq!(at(&open, -500.0), Some(Mask::Black), "the black screen");
    assert_eq!(at(&open, 500.0), Some(circle_mask(cx, cy, n / 2, k)));
    assert_eq!(at(&open, 1000.0), None);
    // A frame between ticks starts the transition at the earlier tick.
    let (t, time) = screen::transition_at(&close, frame_ms + 9.0, 100.0);
    assert_eq!((t.start_tick, time), (5389, frame_ms + 100.0));
    let (t, time) = screen::transition_at(&close, 0.0, 10.0);
    assert_eq!((t.start_tick, time), (0, 10.0));
}

#[test]
fn the_mask_covers_the_world_and_the_screen_layer() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut f = busy_frame((1000.0, 400.0));
    f.screen = screen_sprites();
    let plain_scene = b.build(&mut g, &f);
    let plain = cpu::compose(&g, &plain_scene);
    let t0 = 50.0 * f64::from(TICK_MS);
    f.time_ms = t0 + 600.0;
    f.transition = Some(closing(50, 1250, (1000.0, 400.0)));
    let scene = b.build(&mut g, &f);
    let mask = scene.mask.expect("a circle");
    let masked = cpu::compose(&g, &scene);
    let (mut inside, mut outside) = (0, 0);
    for y in 0..480i64 {
        for x in 0..640i64 {
            let o = (y * 640 + x) as usize;
            if mask.shows(x, y) {
                inside += 1;
                assert_eq!(masked.index[o], plain.index[o]);
            } else {
                outside += 1;
                assert_eq!((masked.index[o], masked.hd[o]), (0, 0));
            }
        }
    }
    assert!(inside > 10_000 && outside > 10_000, "{inside} {outside}");
    // The top-left panel lies outside the circle and is hidden too.
    assert_ne!(plain.index[5], 0);
    assert_eq!(masked.index[5], 0);
}

#[test]
fn fades_and_fade_transitions_act_on_the_palette() {
    let mut g = graphics(None);
    let mut b = SceneBuilder::new(&g, view(640, 480), DEFAULT_MAX_TARGET);
    let mut f = busy_frame((1000.0, 400.0));
    f.screen = screen_sprites();
    let plain = b.build(&mut g, &f);
    let fade = ScreenTransition {
        kind: TransitionKind::Fade,
        closing: false,
        start_tick: 10,
        duration_ms: 1500,
        center: Point::default(),
    };
    f.transition = Some(fade);
    f.time_ms = 180.0 - 1.0;
    let held = b.build(&mut g, &f);
    assert_eq!(held.palette, Palette::default(), "black before the reveal");
    assert_eq!(held.brightness, 0.0);
    assert_eq!(held.mask, None);
    f.time_ms = 180.0 + 750.0;
    let mid = b.build(&mut g, &f);
    // From black: 0 + trunc(c * 750 / 1500).
    let want = g.palette.0[200].map(|c| (u32::from(c) * 750 / 1500) as u8);
    assert_eq!(mid.palette.0[200], want);
    assert_eq!(mid.items, plain.items, "indices are untouched");
    f.time_ms = 180.0 + 1500.0;
    assert_eq!(b.build(&mut g, &f).palette, g.palette);
    // A closing fade ends black; it stacks on a fade from the simulation.
    f.transition = Some(ScreenTransition {
        closing: true,
        ..fade
    });
    f.fade = Some(PaletteFade {
        from: 256,
        to: 128,
        start_tick: 0,
        duration_ms: 10,
    });
    f.time_ms = 180.0 + 750.0;
    let both = b.build(&mut g, &f);
    let c = g.palette.0[200][2];
    let halved = u32::from(c) * 128 / 256;
    assert_eq!(
        both.palette.0[200][2],
        (halved as i64 - halved as i64 * 750 / 1500) as u8
    );
    f.time_ms = 180.0 + 1500.0;
    assert_eq!(b.build(&mut g, &f).palette, Palette::default());
}

/// A 640x480 background whose index is (x / 3 + y / 2) % 256.
fn background() -> Frame {
    frame(640, 480, 0, |x, y| ((x / 3 + y / 2) % 256) as u8)
}

fn other_palette() -> Palette {
    let mut p = Palette::default();
    for (i, c) in p.0.iter_mut().enumerate() {
        *c = [(255 - i) as u8, (i * 5 % 256) as u8, (i / 2) as u8];
    }
    p
}

#[test]
fn screens_draw_with_their_own_palette_pillarboxed() {
    let mut g = screen_graphics();
    let bg = g.add_image(&background());
    let screen = ScreenScene {
        background: Some(bg),
        palette: other_palette(),
        sprites: vec![
            ScreenSprite::new("PANEL", 1, Anchor::Center, 0, 0),
            ScreenSprite::new("PANEL", 2, Anchor::BottomRight, -3, -3),
        ],
        fade: None,
    };
    for size in SIZES {
        let v = view(size.0, size.1);
        let b = ScreenBuilder::new(&v, DEFAULT_MAX_TARGET);
        let comp = *b.composition();
        let game = *SceneBuilder::new(&g, v, DEFAULT_MAX_TARGET).composition();
        assert_eq!(comp.presentation, (640, 480));
        assert_eq!(
            comp.scale, game.scale,
            "{size:?}: scaled like the game picture"
        );
        assert_eq!(comp.height, game.height, "{size:?}");
        assert_eq!(comp.width, comp.height * 4 / 3, "{size:?}: never stretched");
        assert_eq!(
            comp.x,
            (size.0 as i32 - comp.width as i32) / 2,
            "{size:?}: centred"
        );
        let scene = b.build(&mut g, &screen, 0.0);
        assert_eq!(
            scene.palette,
            other_palette(),
            "{size:?}: the screen's palette"
        );
        assert!(scene.planes.is_empty());
        assert_eq!(scene.sprites.len(), 3);
        let c = cpu::compose(&g, &scene);
        let k = comp.scale;
        for (x, y) in [(0u32, 0u32), (639, 0), (100, 333), (639, 479)] {
            let o = ((y * k + k / 2) * c.width + x * k + k / 2) as usize;
            let want = if (x, y) == (639, 479) {
                c.index[o]
            } else {
                ((x / 3 + y / 2) % 256) as u8
            };
            assert_eq!(c.index[o], want, "{size:?} ({x}, {y})");
        }
        // The centred panel (hotspot (319, 239), box from (316, 235)).
        let o = ((235 * k) * c.width + 316 * k) as usize;
        assert_eq!(c.index[o], 230);
    }
    // The same indices show the screen's colours, not the level's.
    let v = view(1920, 1080);
    let b = ScreenBuilder::new(&v, DEFAULT_MAX_TARGET);
    let scene = b.build(&mut g, &screen, 0.0);
    let c = cpu::compose(&g, &scene);
    let out = cpu::present(&scene, &c);
    let comp = scene.comp;
    let px = |x: u32, y: u32| {
        let o = ((y * 1920 + x) * 4) as usize;
        [out[o], out[o + 1], out[o + 2]]
    };
    assert_eq!(px(10, 500), [0, 0, 0], "bar");
    let (x, y) = (comp.x as u32 + 1, 1);
    let i = c.index[0];
    assert_eq!(px(x, y), other_palette().0[usize::from(i)]);
    assert_ne!(
        other_palette().0[usize::from(i)],
        g.palette.0[usize::from(i)]
    );
    // A fade on the screen dims its palette.
    let faded = ScreenScene {
        fade: Some(PaletteFade {
            from: 256,
            to: 0,
            start_tick: 0,
            duration_ms: 500,
        }),
        ..screen.clone()
    };
    let scene = b.build(&mut g, &faded, 250.0);
    assert_eq!(scene.palette.0[7], other_palette().0[7].map(|c| c - c / 2));
    let _ = screen::SCREEN_SIZE;
}
