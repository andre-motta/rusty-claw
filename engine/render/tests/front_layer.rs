//! The front layer switch: with it off the level's front plane is left out
//! of the frame and everything else draws as before, objects in front of
//! the plane included; it changes nothing but drawing. Checked on the CPU
//! reference compositor, and on the GPU with an offscreen render when a GPU
//! is available (run it under the shared bench lock and a virtual display;
//! set `CLAW_GPU_TESTS=1` to make a missing GPU a failure).

mod common;

use claw_render::scene::{DEFAULT_MAX_TARGET, Item};
use claw_render::view::{PLANE_MAIN, PLANE_NO_DRAW, front_plane};
use claw_render::{FrameInput, Gpu, LevelGraphics, Renderer, Scene, SceneBuilder, cpu};
use common::*;

fn scene(g: &mut LevelGraphics, front: bool, frame: &FrameInput<'_>) -> Scene {
    let mut b = SceneBuilder::new(g, view(1280, 960), DEFAULT_MAX_TARGET);
    assert!(b.front_layer(), "on by default");
    b.set_front_layer(front);
    assert_eq!(b.front_layer(), front);
    b.build(g, frame)
}

fn planes_of(s: &Scene) -> Vec<u32> {
    s.items
        .iter()
        .filter_map(|i| match i {
            Item::Plane(p) => Some(*p),
            Item::Sprite(_) => None,
        })
        .collect()
}

fn sprites_of(s: &Scene) -> usize {
    s.items
        .iter()
        .filter(|i| matches!(i, Item::Sprite(_)))
        .count()
}

#[test]
fn the_front_plane_is_the_last_plane_unless_it_is_the_main_one() {
    let p = planes();
    assert_eq!(front_plane(&p), Some(2));
    assert_eq!(front_plane(&p[..2]), None, "the main plane is never hidden");
    assert_eq!(front_plane(&p[1..2]), None);
    assert_eq!(front_plane(&[]), None);
    let mut two = vec![p[1].clone(), p[0].clone()];
    assert_eq!(front_plane(&two), Some(1));
    two[1].flags |= PLANE_MAIN;
    assert_eq!(front_plane(&two), None);
}

#[test]
fn off_leaves_out_the_front_plane_only() {
    let frame = busy_frame((1000.0, 400.0));
    for interleaved in [true, false] {
        let mut g = graphics(None);
        g.interleaved = interleaved;
        let on = scene(&mut g, true, &frame);
        let off = scene(&mut g, false, &frame);
        assert_eq!(planes_of(&on), vec![0, 1, 2]);
        assert_eq!(planes_of(&off), vec![0, 1], "interleaved {interleaved}");
        assert_eq!(
            sprites_of(&off),
            sprites_of(&on),
            "every object still draws, those in front of the plane included"
        );
        assert_eq!(off.sprites, on.sprites);
        assert_eq!(off.planes, on.planes);
        // On the CPU reference: the same picture as a level whose front
        // plane is marked not drawn, and not the picture with it.
        let c_on = cpu::compose(&g, &on);
        let c_off = cpu::compose(&g, &off);
        assert_ne!(c_on.index, c_off.index, "the front plane shows");
        let mut marked = graphics(None);
        marked.interleaved = interleaved;
        marked.planes[2].flags |= PLANE_NO_DRAW;
        let marked_scene = scene(&mut marked, true, &frame);
        let c_marked = cpu::compose(&marked, &marked_scene);
        assert_eq!(c_off.index, c_marked.index);
    }
}

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) if std::env::var_os("CLAW_GPU_TESTS").is_none() => {
            eprintln!("skipped the GPU part: {e}");
            None
        }
        Err(e) => panic!("no GPU: {e}"),
    }
}

#[test]
fn an_offscreen_render_shows_the_front_plane_only_with_the_switch_on() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = Renderer::new(&gpu);
    let mut g = graphics(None);
    renderer.set_level(&g);
    let frame = busy_frame((1000.0, 400.0));
    let mut outputs = Vec::new();
    for front in [true, false] {
        let s = scene(&mut g, front, &frame);
        let want = cpu::compose(&g, &s);
        let got = renderer.read_composed(&g, &s);
        assert_eq!(got.index, want.index, "front layer {front}");
        let out = renderer.render_offscreen(&g, &s);
        let reference = cpu::present(&s, &want);
        let worst = out
            .iter()
            .zip(&reference)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(worst <= 1, "front layer {front}: output differs by {worst}");
        outputs.push(out);
    }
    assert_ne!(outputs[0], outputs[1], "the switch changes the picture");
}
