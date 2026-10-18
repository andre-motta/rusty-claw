//! The fourth level's floor spikes, goo cover-ups and animated goo on
//! synthetic levels: a five-image spike strikes from its retracted image
//! with its own box, rises in 26 calls and keeps the halves and phase its
//! record gives; cover-ups and animated goo keep the depth they were
//! placed with.

mod common;

use std::sync::Arc;

use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::decor::{self, DecorData};
use claw_sim::logic::floor_spike::{self, FloorSpikeData, state};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::{ObjectData, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const SET: &str = "LEVEL_FLOORSPIKES";

/// Record field holding a spike's phase.
const SPEED_FIELD: usize = 60;

fn element(image_op: i32, next_op: i32, duration: i32) -> AnimFrame {
    AnimFrame {
        image_op,
        next_op,
        duration,
        ..AnimFrame::default()
    }
}

fn add_anim(level: &mut SimLevel, key: &str, frames: Vec<AnimFrame>) {
    level.animations.insert(
        key.to_string(),
        Arc::new(Animation {
            key: key.to_string(),
            frames,
            ..Animation::default()
        }),
    );
}

/// The fourth level's spikes: five images, the retracted one 16 x 7 px
/// with its hotspot 1 px right and 10 px down.
fn spike_frames() -> ImageFrames {
    let mut f = ImageFrames::range(1, 5);
    for (i, (h, ox, oy)) in [(7, 1, 10), (11, 1, 8), (17, 1, 5), (23, 1, 2), (28, 1, 0)]
        .into_iter()
        .enumerate()
    {
        f.boxes.insert(i as i32 + 1, frame_box(16, h, ox, oy));
    }
    f
}

/// The flat level with the spikes' images and slides and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 4;
    level.images.insert(SET.into(), spike_frames());
    add_anim(
        &mut level,
        floor_spike::RISE_ANIM,
        vec![element(1, 3, 100), element(0, 9, 0)],
    );
    add_anim(
        &mut level,
        floor_spike::DROP_ANIM,
        vec![element(5, 0, 0), element(2, 2, 100), element(0, 9, 0)],
    );
    add_anim(&mut level, decor::CYCLE_ANIM, vec![element(1, 0, 100)]);
    level.objects = objects;
    level
}

/// A `FloorSpike` record at (x, 950) with the given halves and phase.
fn spike(x: i32, up_ms: i32, down_ms: i32, phase: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("FloorSpike", x, 950, 0);
    s.image_set = SET.into();
    s.frame = 1;
    s.record[55] = up_ms;
    s.record[56] = down_ms;
    s.record[SPEED_FIELD] = phase;
    s
}

/// Every object awake from the start, Claw far from the spikes.
fn awake(level: SimLevel) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(400, 899);
    sim
}

fn spikes(sim: &Sim) -> Vec<ObjectId> {
    let mut v: Vec<(i32, ObjectId)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "FloorSpike")
        .map(|(id, o)| (o.x, id))
        .collect();
    v.sort_unstable();
    v.into_iter().map(|(_, id)| id).collect()
}

fn data(sim: &Sim, id: ObjectId) -> FloorSpikeData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::FloorSpike(d) => d.clone(),
        other => panic!("not a spike: {other:?}"),
    }
}

#[test]
fn the_record_sets_the_halves_and_the_phase() {
    let sim = awake(level_with(vec![
        spike(1000, 0, 0, 0),
        spike(1100, 1000, 1000, 0),
        spike(1200, 6000, 2000, 7500),
        spike(1300, 800, 800, 2400),
    ]));
    let got: Vec<_> = spikes(&sim)
        .into_iter()
        .map(|id| {
            let d = data(&sim, id);
            (d.on_ms, d.period, d.phase, d.frames)
        })
        .collect();
    assert_eq!(
        got,
        [
            (1500, 3000, 0, Some((1, 5))),
            (1000, 2000, 0, Some((1, 5))),
            (6000, 8000, 7500, Some((1, 5))),
            (800, 1600, 2400, Some((1, 5))),
        ]
    );
    for id in spikes(&sim) {
        let o = sim.objects().get(id).unwrap();
        // The retracted image's box, raised by 10 px.
        assert_eq!(o.attack_rect, Rect::new(-7, -3, 8, 12));
        assert_eq!(o.state, state::DOWN);
    }
}

#[test]
fn a_five_image_spike_is_dangerous_from_the_26th_call_after_the_edge() {
    // Up 1000 ms, down 1000 ms, phase 0: the first tick (18 ms) sees the
    // up half, so the edge call E is tick 1 and the spike is dangerous
    // from tick 27 until tick 56, the first past 1000 ms, which stores
    // the drop. The next edge is tick 112 (2016 ms), dangerous from 138.
    let mut sim = awake(level_with(vec![spike(1100, 1000, 1000, 0)]));
    let id = spikes(&sim)[0];
    let mut log = Vec::new();
    for t in 1..=200u64 {
        sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        log.push((t, o.state, floor_spike::dangerous(o), o.frame));
    }
    let at = |t: u64| log[t as usize - 1];
    assert_eq!(at(1).1, state::RISE_START, "tick 1 sees the edge");
    let danger: Vec<u64> = log.iter().filter(|r| r.2).map(|r| r.0).collect();
    let mut want: Vec<u64> = (27..=56).collect();
    want.extend(138..=167);
    assert_eq!(danger, want);
    // The rise: image 2 on the first call after the start, then one
    // every 6 calls, the last (5) on the 19th; finished on the 25th.
    assert_eq!([3, 9, 15, 21].map(|t| at(t).3), [2, 3, 4, 5]);
    assert_eq!(at(2).3, 1);
    assert_eq!(at(26).1, state::RISING);
    assert_eq!(at(27).1, state::UP);
    // The drop: stored on tick 56, set on 57, then 26 calls back to
    // down on the first image.
    assert_eq!(at(56).1, state::DROP_START);
    assert_eq!(at(57).1, state::DROPPING);
    assert_eq!(at(82).1, state::DROPPING);
    assert_eq!((at(83).1, at(83).3), (state::DOWN, 1));
    assert_eq!(at(111).1, state::DOWN);
    assert_eq!(at(112).1, state::RISE_START);
}

#[test]
fn goo_cover_ups_and_animated_goo_keep_their_placed_depth() {
    let mut objects = Vec::new();
    for (logic, set, z) in [
        ("GooCoverup", "LEVEL_TREETRUNKCOVER", 8000),
        ("GooCoverup", "LEVEL_GOOMIDDLE", 0),
        ("AniCycle", "LEVEL_TRIPLEGOO", 5000),
        ("AniCycle", "LEVEL_SINGLEGOO", 5000),
    ] {
        let mut s = ObjectSpec::at(logic, 1000 + 70 * objects.len() as i32, 980, z);
        s.image_set = set.into();
        s.frame = 1;
        objects.push(s);
    }
    let mut level = level_with(objects);
    for set in ["LEVEL_TRIPLEGOO", "LEVEL_SINGLEGOO"] {
        level.images.insert(set.into(), ImageFrames::range(1, 4));
    }
    let mut sim = awake(level);
    idle(&mut sim, 30);
    let mut got: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.record.is_some())
        .map(|(_, o)| (o.x, o.image_set.clone(), o.z, o.frame, o.data.clone()))
        .collect();
    got.sort_by_key(|g| g.0);
    let depths: Vec<_> = got.iter().map(|g| (g.1.as_str(), g.2)).collect();
    assert_eq!(
        depths,
        [
            ("LEVEL_TREETRUNKCOVER", 8000),
            ("LEVEL_GOOMIDDLE", decor::GOO_COVERUP_Z),
            ("LEVEL_TRIPLEGOO", 5000),
            ("LEVEL_SINGLEGOO", 5000),
        ]
    );
    // The cover-ups stay still; the goo loops its images.
    assert_eq!((got[0].3, got[1].3), (1, 1));
    for g in &got[2..] {
        assert_ne!(g.3, 1, "{} has moved on", g.1);
        let ObjectData::Decor(DecorData { frames, .. }) = &g.4 else {
            panic!("{:?}", g.4)
        };
        assert_eq!(*frames, Some((1, 4)));
    }
}
