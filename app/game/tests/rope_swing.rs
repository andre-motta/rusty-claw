//! The smooth rope swing on a synthetic level: with it on, Claw and the
//! view move on every 60 Hz frame of a swing; with it off the view holds on
//! about one frame in three, as the rope's own steps give; and the game
//! itself (its trace and its state) is the same either way.

use std::sync::Arc;

use claw_frame::{InterpolatedFrame, Point, SimulatedRefresh};
use claw_game::Session;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::rope;
use claw_sim::object::ObjectId;
use claw_sim::player::{state, swing};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

const ROPE_AT: (i32, i32) = (2000, 300);

/// A flat level with one rope at [`ROPE_AT`] (the usual 1.5 s half swing),
/// the rope's frames and Claw's swinging animation, everything awake.
fn rope_level() -> Sim {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    level
        .images
        .insert(rope::IMAGE_SET.into(), ImageFrames::range(1, 120));
    let anim = Animation::looping(
        swing::SWING_ANIM,
        &[(500, 75), (501, 75), (502, 75), (503, 75)],
    );
    level
        .animations
        .insert(swing::SWING_ANIM.into(), Arc::new(anim));
    let mut spec = ObjectSpec::at("AniRope", ROPE_AT.0, ROPE_AT.1, 0);
    spec.image_set = rope::IMAGE_SET.into();
    spec.frame = 1;
    level.objects = vec![spec];
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    sim
}

fn the_rope(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| rope::NAMES.contains(&&*o.logic))
        .map(|(id, _)| id)
        .expect("a rope")
}

/// Drops Claw onto the rope's end until he hangs from it.
fn hang_on(sim: &mut Sim) {
    let id = the_rope(sim);
    for _ in 0..200 {
        sim.step(TickInput::default());
        if sim.player().unwrap().state == state::SWING {
            return;
        }
        let end = rope::end_of(sim.objects().get(id).unwrap()).unwrap();
        sim.teleport_player(end.0, end.1 + swing::GRAB_DROP);
    }
    panic!("Claw never grabbed the rope");
}

/// One drawn frame: Claw's drawing position, the view and whether he hangs
/// from the rope after the frame's ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Drawn {
    claw: (f64, f64),
    camera: Point,
    swinging: bool,
}

fn drawn(frame: &InterpolatedFrame<'_>, claw: u32, swinging: bool) -> Drawn {
    let c = frame
        .renderables
        .iter()
        .find(|r| r.source.id == claw)
        .expect("Claw is drawn");
    Drawn {
        claw: (c.x, c.y),
        camera: frame.camera,
        swinging,
    }
}

/// Plays `sim` at a 60 Hz display for `frames` frames with the smooth rope
/// swing on or off, the game tracing each tick.
fn play(sim: Sim, smooth: bool, frames: usize) -> (Vec<Drawn>, Session) {
    let claw = sim.player().unwrap().serial;
    let mut session = Session::new(sim);
    assert!(session.smooth_rope(), "on by default");
    session.set_smooth_rope(smooth);
    session.record_trace();
    let mut refresh = SimulatedRefresh::new(60.0);
    let mut out = Vec::new();
    for _ in 0..frames {
        session.frame(refresh.next().unwrap_or_default());
        let swinging = session.sim().player().unwrap().state == state::SWING;
        out.push(drawn(&session.interpolated(true), claw, swinging));
    }
    (out, session)
}

/// Of the consecutive frames that both show the moving middle of a swing
/// (Claw within 100 px of the rope across), how many draw Claw and the
/// view where the frame before did.
fn holds(frames: &[Drawn]) -> (usize, usize, usize) {
    let middle = |d: &Drawn| d.swinging && (d.claw.0 - f64::from(ROPE_AT.0)).abs() < 100.0;
    let (mut pairs, mut claw, mut camera) = (0, 0, 0);
    for w in frames.windows(2) {
        if !(middle(&w[0]) && middle(&w[1])) {
            continue;
        }
        pairs += 1;
        claw += usize::from(w[0].claw == w[1].claw);
        camera += usize::from(w[0].camera == w[1].camera);
    }
    (pairs, claw, camera)
}

#[test]
fn smooth_on_moves_every_frame_and_off_keeps_the_rope_steps() {
    let mut sim = rope_level();
    hang_on(&mut sim);
    // Six seconds: two whole swings.
    let frames = 360;
    let (on, on_session) = play(sim.clone(), true, frames);
    let (off, off_session) = play(sim, false, frames);
    assert!(on.iter().all(|d| d.swinging), "Claw swings throughout");

    let (pairs, claw, camera) = holds(&on);
    assert!(pairs > 100, "{pairs} frame pairs in the middle of a swing");
    assert_eq!((claw, camera), (0, 0), "smooth: a move on every frame");

    // Off, the view jumps to Claw on each tick (he flies with the rope),
    // so it stands still on the frames that run no tick or a tick on
    // which the rope keeps its frame: about one frame in three.
    let (pairs, _, camera) = holds(&off);
    let share = camera as f64 / pairs as f64;
    assert!(
        (share - 1.0 / 3.0).abs() < 0.05,
        "off: the view holds on {camera} of {pairs} frames"
    );

    // Smooth or not, Claw is drawn within one frame's step of the end
    // (19 px at most) of where the stepped swing draws him.
    for (a, b) in on.iter().zip(&off) {
        assert!(
            (a.claw.0 - b.claw.0).abs() <= 19.0 && (a.claw.1 - b.claw.1).abs() <= 19.0,
            "{a:?} {b:?}"
        );
    }

    // Drawing only: the same game.
    assert_eq!(on_session.ticks(), off_session.ticks());
    assert_eq!(on_session.trace(), off_session.trace());
    assert_eq!(on_session.sim().snapshot(), off_session.sim().snapshot());
}

#[test]
fn letting_go_and_the_ground_are_drawn_the_same_either_way() {
    let mut sim = rope_level();
    hang_on(&mut sim);
    for _ in 0..40 {
        sim.step(TickInput::default());
    }
    // Let go: Claw jumps off and lands; nothing is smoothed from then on.
    sim.step(TickInput {
        held: claw_frame::Buttons::JUMP,
        pressed: claw_frame::Buttons::JUMP,
    });
    sim.step(TickInput::default());
    assert_ne!(sim.player().unwrap().state, state::SWING);
    let (on, a) = play(sim.clone(), true, 120);
    let (off, b) = play(sim, false, 120);
    assert!(on.iter().all(|d| !d.swinging));
    assert_eq!(on, off);
    assert_eq!(a.trace(), b.trace());
}

#[test]
fn without_interpolation_nothing_is_smoothed() {
    let mut sim = rope_level();
    hang_on(&mut sim);
    let claw = sim.player().unwrap().serial;
    let mut on = Session::new(sim.clone());
    let mut off = Session::new(sim);
    off.set_smooth_rope(false);
    let mut refresh = SimulatedRefresh::new(60.0);
    for _ in 0..120 {
        let dt = refresh.next().unwrap_or_default();
        on.frame(dt);
        off.frame(dt);
        assert_eq!(
            drawn(&on.interpolated(false), claw, true),
            drawn(&off.interpolated(false), claw, true)
        );
    }
}
