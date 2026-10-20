//! Saw blades on synthetic levels: the set-up (strike box, kind, depth,
//! damage, cycle), the cycle on the level clock tick by tick (dangerous
//! from the 32nd tick after the up half starts until the first tick after
//! it ends, back down 29 ticks later), the variants' phases, a saw woken
//! inside its up half, its blow on Claw (10, at most twice an up half),
//! the sounds only a level that has them plays, and the look of a saw
//! asleep in the side areas of a wide display.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::saw_blade::{self, SawData, state};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, Rect, flags, types};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const SET: &str = saw_blade::IMAGE_SET;
/// A saw rests 20 px above the floor (top at 960).
const SAW_Y: i32 = 940;

fn image(i: i32, ms: i32, next_op: i32) -> AnimFrame {
    AnimFrame {
        image_op: 3,
        image_index: i,
        duration: ms,
        next_op,
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

/// The flat level with the saws' twenty pictures (the sunk one 108 x 9
/// px, 14 px below the saw), their rise (pictures 1 to 10, 50 ms each),
/// spin (10 and 11, 25 ms each, looping) and sink (12 to 20, 50 ms each),
/// and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    let mut f = ImageFrames::range(1, 20);
    for i in 1..=20 {
        let b = if (10..=11).contains(&i) {
            frame_box(108, 37, 1, 0)
        } else {
            frame_box(108, 9, 1, 14)
        };
        f.boxes.insert(i, b);
    }
    level.images.insert(SET.into(), f);
    let rise: Vec<AnimFrame> = (1..=10)
        .map(|i| image(i, 50, if i == 10 { 9 } else { 0 }))
        .collect();
    add_anim(&mut level, saw_blade::UP_ANIM, rise);
    add_anim(
        &mut level,
        saw_blade::SPIN_ANIM,
        vec![image(10, 25, 0), image(11, 25, 0)],
    );
    let sink: Vec<AnimFrame> = (12..=20)
        .map(|i| image(i, 50, if i == 20 { 9 } else { 0 }))
        .collect();
    add_anim(&mut level, saw_blade::DOWN_ANIM, sink);
    level.objects = objects;
    level
}

fn saw(name: &str, x: i32, phase: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at(name, x, SAW_Y, 0);
    s.image_set = SET.into();
    s.frame = 1;
    s.record[saw_blade::SPEED_FIELD] = phase;
    s
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

/// Claw put down at x `x` (standing, feet on 959) with `objects`.
fn sim_with(x: i32, objects: Vec<ObjectSpec>) -> Sim {
    let mut level = level_with(objects);
    level.start = (x, 899);
    pressing_sim(level, original())
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

/// One tick of a saw: its state, whether it hurts, its picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    state: i32,
    dangerous: bool,
    frame: i32,
}

/// Plays `n` ticks and records the saw `id` after each.
fn record(sim: &mut Sim, id: ObjectId, n: usize) -> Vec<Step> {
    (0..n)
        .map(|_| {
            sim.step(TickInput::default());
            let o = sim.objects().get(id).unwrap();
            Step {
                state: o.state,
                dangerous: saw_blade::dangerous(o),
                frame: o.frame,
            }
        })
        .collect()
}

fn entries(steps: &[Step], s: i32) -> Vec<usize> {
    (0..steps.len())
        .filter(|&k| steps[k].state == s && (k == 0 || steps[k - 1].state != s))
        .collect()
}

#[test]
fn a_saw_is_a_hazard_shot_striking_in_its_narrowed_box() {
    let mut sim = sim_with(800, vec![saw("SawBlade", 1000, 0)]);
    let id = find(&sim, "SawBlade");
    sim.step(TickInput::default());
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.attack_rect, Rect::new(-33, -10, 34, 17));
    assert_eq!(o.type_flags, types::LEVEL_SHOT);
    assert_eq!((o.z, o.damage), (saw_blade::DEFAULT_Z, saw_blade::DAMAGE));
    assert!(!o.hit_rect.is_set(), "nothing hits a saw");
    assert_eq!(
        o.data,
        ObjectData::SawBlade(SawData {
            phase: 0,
            on_ms: 1500,
            period: 3000
        })
    );
}

#[test]
fn a_saw_hurts_from_the_end_of_its_rise_to_the_end_of_its_up_half() {
    let mut sim = sim_with(800, vec![saw("SawBlade", 1000, 0)]);
    let id = find(&sim, "SawBlade");
    let steps = record(&mut sim, id, 700);
    let rise = entries(&steps, state::RISE_START);
    let sink = entries(&steps, state::DROP_START);
    assert!(rise.len() >= 3);
    for (&e, &f) in rise.iter().zip(&sink) {
        assert!(f > e);
        // The rise shows pictures 1 to 10 from E + 2, one every 3 ticks.
        assert_eq!(steps[e + 1].state, state::RISING);
        let frames: Vec<i32> = (e + 2..e + 32).step_by(3).map(|k| steps[k].frame).collect();
        assert_eq!(frames, (1..=10).collect::<Vec<_>>());
        // Dangerous from E + 32 to F inclusive, harmless from F + 1.
        let danger: Vec<usize> = (e..f + 5).filter(|&k| steps[k].dangerous).collect();
        assert_eq!(danger, (e + 32..=f).collect::<Vec<_>>());
        assert!((83..=84).contains(&(f - e)), "{e} {f}");
        // It spins while up, two ticks a picture.
        assert!((e + 33..f).all(|k| matches!(steps[k].frame, 10 | 11)));
        assert_eq!(steps[e + 33].frame, steps[e + 34].frame);
        // The sink shows 12 to 20 and ends Down on F + 29.
        assert_eq!(steps[f + 2].frame, 12);
        assert_eq!(steps[f + 28].state, state::DROPPING);
        assert_eq!(
            (steps[f + 29].state, steps[f + 29].frame),
            (state::DOWN, 20)
        );
    }
    // 166 or 167 ticks from rise to rise.
    assert!(
        rise.windows(2)
            .all(|w| (166..=167).contains(&(w[1] - w[0])))
    );
}

#[test]
fn the_variants_rise_a_quarter_and_a_half_cycle_later() {
    let mut sim = sim_with(
        1200,
        vec![
            saw("SawBlade", 1000, 0),
            saw("SawBlade2", 1200, 0),
            saw("SawBlade3", 1400, 0),
        ],
    );
    let ids = ["SawBlade", "SawBlade2", "SawBlade3"].map(|n| find(&sim, n));
    let mut rises = [Vec::new(), Vec::new(), Vec::new()];
    for k in 0..400 {
        sim.step(TickInput::default());
        for (i, &id) in ids.iter().enumerate() {
            if sim.objects().get(id).unwrap().state == state::RISE_START {
                rises[i].push(k);
            }
        }
    }
    let a = rises[0].iter().find(|&&k| k > 100).copied().unwrap();
    let b = rises[1].iter().find(|&&k| k > a).copied().unwrap();
    let c = rises[2].iter().find(|&&k| k > a).copied().unwrap();
    // 750 ms is 41 or 42 ticks, 1500 ms 83 or 84.
    assert!((41..=42).contains(&(b - a)), "{a} {b}");
    assert!((83..=84).contains(&(c - a)), "{a} {c}");
    // The record's speed field moves the first variant's cycle.
    let mut sim = sim_with(800, vec![saw("SawBlade", 1000, 300)]);
    let id = find(&sim, "SawBlade");
    sim.step(TickInput::default());
    match &sim.objects().get(id).unwrap().data {
        ObjectData::SawBlade(d) => assert_eq!(d.phase, 300),
        d => panic!("{d:?}"),
    }
}

#[test]
fn a_saw_woken_late_in_its_up_half_rises_at_once_and_hurts_for_two_ticks() {
    // Its up half runs to clock 600 (phase -900): set up before the first
    // tick, it starts rising on the first, is dangerous on ticks 32 and
    // 33 and starts sinking on 33.
    let mut sim = sim_with(800, vec![saw("SawBlade", 1000, -900)]);
    let id = find(&sim, "SawBlade");
    let steps = record(&mut sim, id, 80);
    assert_eq!(entries(&steps, state::RISE_START)[0], 0);
    let danger: Vec<usize> = (0..80).filter(|&k| steps[k].dangerous).collect();
    assert_eq!(danger, [32, 33]);
    assert_eq!(entries(&steps, state::DROP_START)[0], 33);
}

#[test]
fn a_saw_costs_claw_10_at_most_twice_an_up_half() {
    // Claw stands on the saw's slot.
    let mut sim = sim_with(1000, vec![saw("SawBlade", 1000, 0)]);
    let id = find(&sim, "SawBlade");
    let mut health = Vec::new();
    let mut danger = Vec::new();
    for _ in 0..400 {
        sim.step(TickInput::default());
        health.push(sim.claw().stats.health);
        danger.push(saw_blade::dangerous(sim.objects().get(id).unwrap()));
    }
    let hits: Vec<usize> = (1..health.len())
        .filter(|&k| health[k] < health[k - 1])
        .collect();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|&k| health[k - 1] - health[k] == 10));
    // Every hit lands while the saw is dangerous, the first on the first
    // dangerous tick; his 850 ms protection keeps them 48 ticks apart.
    assert!(hits.iter().all(|&k| danger[k]));
    let first_danger = danger.iter().position(|&d| d).unwrap();
    assert_eq!(hits[0], first_danger);
    assert!(hits.windows(2).all(|w| w[1] - w[0] >= 48));
    // At most two in one up half.
    let mut half = 0;
    let mut n = 0;
    for k in 0..danger.len() {
        if danger[k] && (k == 0 || !danger[k - 1]) {
            half += 1;
            n = 0;
        }
        if hits.contains(&k) {
            n += 1;
            assert!(n <= 2, "up half {half}");
        }
    }
}

#[test]
fn a_saw_plays_its_sounds_only_when_the_level_has_them() {
    let sounds = |with: bool| {
        let mut level = level_with(vec![saw("SawBlade", 900, 0)]);
        level.start = (700, 899);
        if with {
            for key in [saw_blade::RISE_SOUND, saw_blade::SINK_SOUND] {
                level.sounds.insert(key.to_string(), Some(300));
            }
        }
        let mut sim = pressing_sim(level, original());
        let mut heard = Vec::new();
        for _ in 0..200 {
            sim.step(TickInput::default());
            for cue in sim.last_audio() {
                if let claw_sim::claw_frame::AudioCue::Play(s) = cue {
                    heard.push(s.key.clone());
                }
            }
        }
        heard
    };
    assert!(sounds(false).is_empty());
    let heard = sounds(true);
    assert!(heard.iter().any(|k| k == saw_blade::RISE_SOUND));
    assert!(heard.iter().any(|k| k == saw_blade::SINK_SOUND));
}

#[test]
fn a_saw_draws_no_random_number() {
    let mut sim = sim_with(800, vec![saw("SawBlade", 1000, 0)]);
    let before = sim.with_world(|w| w.rng.clone());
    idle(&mut sim, 300);
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
}

#[test]
fn asleep_a_saw_is_drawn_sunk() {
    let level = level_with(vec![]);
    let mut o = logic::new_object("SawBlade2", 5000, SAW_Y, 0);
    o.image_set = SET.into();
    o.frame = 1;
    o.record = Some(Box::new([0; 71]));
    let look = logic::first_look(&level, &o).expect("a look");
    assert_eq!(look.len(), 1);
    assert_eq!((look[0].frame, look[0].x, look[0].y), (1, 5000, SAW_Y));
    assert_eq!(look[0].z, saw_blade::DEFAULT_Z);
    // Once it has run it looks as it is.
    o.state = state::UP;
    o.flags &= !flags::HARMLESS;
    assert_eq!(logic::first_look(&level, &o), None);
}
