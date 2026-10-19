//! The smooth rope swing on the third level of your own install (skipped
//! without `CLAW_INSTALL`), drawn offscreen on the GPU (run it under the
//! shared bench lock and a virtual display with the other install tests;
//! set `CLAW_GPU_TESTS=1` to make a missing GPU a failure). Claw hangs from
//! the level's first rope through two whole swings at a 60 Hz display:
//! with the setting on, his drawn position, the view and the picture change
//! on every frame of the moving part of a swing; with it off, the view (and
//! with it the level around him) holds on the frames whose ticks keep the
//! rope's frame, as the rope's own steps give: one frame in three for the
//! usual half swing, more for this level's slower ropes. The game's trace
//! is the same either way.

use std::path::PathBuf;

use claw_frame::{Point, SimulatedRefresh};
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{Session, load_graphics, load_level, new_sim};
use claw_render::{FrameInput, Gpu, Renderer, SceneBuilder};
use claw_sim::logic::rope;
use claw_sim::object::ObjectId;
use claw_sim::player::{state, swing};
use claw_sim::trace::TraceRow;
use claw_sim::{Sim, TickInput};

fn install() -> Option<PathBuf> {
    match std::env::var_os("CLAW_INSTALL") {
        Some(p) => Some(PathBuf::from(p)),
        None => {
            eprintln!("CLAW_INSTALL is not set; skipped");
            None
        }
    }
}

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) if std::env::var_os("CLAW_GPU_TESTS").is_none() => {
            eprintln!("skipped: no GPU: {e}");
            None
        }
        Err(e) => panic!("no GPU: {e}"),
    }
}

/// The level's ropes, in placement order.
fn ropes(sim: &Sim) -> Vec<ObjectId> {
    let mut v: Vec<(u32, ObjectId)> = sim
        .objects()
        .all()
        .filter(|(_, o)| rope::NAMES.contains(&&*o.logic))
        .map(|(id, o)| (o.serial, id))
        .collect();
    v.sort_unstable();
    v.into_iter().map(|(_, id)| id).collect()
}

/// Drops Claw onto the rope's end until he hangs from it.
fn hang_on(sim: &mut Sim, id: ObjectId) {
    let at = sim.objects().get(id).map(|o| (o.x, o.y)).unwrap();
    sim.teleport_player(at.0, at.1);
    for _ in 0..300 {
        sim.step(TickInput::default());
        if sim.player().unwrap().state == state::SWING && sim.claw().rope == Some(id) {
            return;
        }
        if let Some(end) = sim.objects().get(id).and_then(rope::end_of) {
            sim.teleport_player(end.0, end.1 + swing::GRAB_DROP);
        }
    }
    panic!("Claw never grabbed the rope");
}

/// One drawn frame.
struct Drawn {
    claw: (f64, f64),
    camera: Point,
    /// In the moving middle of a swing: Claw hangs from the rope within
    /// 100 px of it across.
    middle: bool,
    picture: Vec<u8>,
}

struct Run {
    frames: Vec<Drawn>,
    trace: Vec<TraceRow>,
}

#[test]
fn install_level3_rope_swing_is_smooth_with_the_setting_on() {
    let Some(install) = install() else { return };
    let Some(gpu) = gpu() else { return };
    let choice = LevelChoice::Retail(3);
    let bundle = load_level(&install, &choice).expect("the third level loads");
    let settings = Settings::new(&install, choice);
    let size = (1280, 960);
    let view = settings.view(size);
    let mut sim = new_sim(&bundle, settings.sim_config(size), None);
    let all = ropes(&sim);
    assert!(!all.is_empty(), "the third level has ropes");
    // A rope of the usual half swing (40 frames per second) when there is
    // one.
    let q = |id: ObjectId| {
        let half = sim.objects().get(id).unwrap().speed_x;
        let half = if half == 0 {
            rope::DEFAULT_HALF_SWING_MS
        } else {
            half
        };
        rope::motion(half, 0).0
    };
    let rope_id = all
        .iter()
        .copied()
        .find(|&id| q(id) == 40)
        .unwrap_or(all[0]);
    let rate = q(rope_id);
    eprintln!("rope at {rate} frames per second");
    hang_on(&mut sim, rope_id);
    let rope_x = f64::from(sim.objects().get(rope_id).unwrap().x);
    let claw = sim.player().unwrap().serial;

    let run = |smooth: bool| -> Run {
        let mut graphics = load_graphics(&bundle, None).expect("the graphics load");
        let mut renderer = Renderer::new(&gpu);
        renderer.set_level(&graphics);
        let mut builder = SceneBuilder::new(&graphics, view, gpu.max_target());
        let mut session = Session::new(sim.clone());
        session.set_presentation(view);
        session.set_smooth_rope(smooth);
        session.record_trace();
        let mut refresh = SimulatedRefresh::new(60.0);
        let mut frames = Vec::new();
        // Six seconds: two whole swings of the usual half swing.
        for _ in 0..360 {
            session.frame(refresh.next().unwrap_or_default());
            let swinging = session.sim().player().unwrap().state == state::SWING;
            let frame = session.interpolated(true);
            let c = frame
                .renderables
                .iter()
                .find(|r| r.source.id == claw)
                .expect("Claw is drawn");
            let (x, y) = (c.x, c.y);
            let input = FrameInput::from_frame(&frame, &view);
            let scene = builder.build(&mut graphics, &input);
            let picture = renderer.render_offscreen(&graphics, &scene);
            frames.push(Drawn {
                claw: (x, y),
                camera: frame.camera,
                middle: swinging && (x - rope_x).abs() < 100.0,
                picture,
            });
        }
        Run {
            frames,
            trace: session.trace().to_vec(),
        }
    };
    let on = run(true);
    let off = run(false);
    assert_eq!(on.trace, off.trace, "drawing only: the same game");

    // Frame pairs in the middle of a swing, and how many of them hold
    // Claw's drawn position, the view and the picture.
    let holds = |r: &Run| {
        let (mut pairs, mut claw, mut view, mut picture) = (0usize, 0, 0, 0);
        for w in r.frames.windows(2) {
            if !(w[0].middle && w[1].middle) {
                continue;
            }
            pairs += 1;
            claw += usize::from(w[0].claw == w[1].claw);
            view += usize::from(w[0].camera == w[1].camera);
            picture += usize::from(w[0].picture == w[1].picture);
        }
        (pairs, claw, view, picture)
    };
    let (pairs, claw_holds, view_holds, picture_holds) = holds(&on);
    eprintln!(
        "on: {pairs} frame pairs, Claw holds {claw_holds}, view {view_holds}, picture {picture_holds}"
    );
    assert!(pairs > 100, "{pairs} frame pairs in the middle of a swing");
    assert_eq!(
        (claw_holds, view_holds, picture_holds),
        (0, 0, 0),
        "smooth: Claw, the view and the picture move on every frame"
    );
    let (pairs, claw_holds, view_holds, picture_holds) = holds(&off);
    eprintln!(
        "off: {pairs} frame pairs, Claw holds {claw_holds}, view {view_holds}, picture {picture_holds}"
    );
    // The view moves on the frames whose ticks moved the rope on a frame:
    // `rate` of the display's 60 a second (40, two frames in three, for
    // the usual half swing).
    let share = view_holds as f64 / pairs as f64;
    let expected = 1.0 - f64::from(rate) / 60.0;
    assert!(
        (share - expected).abs() < 0.06,
        "off: the view holds on {view_holds} of {pairs} frames, expected about {expected:.2}"
    );
}
