//! The sixth level's goo vents on synthetic levels: the 205-tick pass, the
//! danger box one tick behind the picture, the hit on Claw (1 health, the
//! next hit 48 ticks later, at most three times a pass), the keg the splash sets off, and a vent's cycle running
//! only while it is live.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::goo_vent::{self, ATTACK_RECTS, state as vent};
use claw_sim::logic::keg;
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A vent 4 px above the floor (top at 960), as on the sixth level, where
/// a standing Claw is at y 899.
const VENT_Y: i32 = 956;
const STAND_Y: i32 = 899;

fn image(i: i32, ms: i32) -> AnimFrame {
    AnimFrame {
        image_op: 3,
        image_index: i,
        duration: ms,
        ..AnimFrame::default()
    }
}

/// The eruption: pictures 1 to 9 and back to 1, 200 ms each, finishing on
/// the last.
fn eruption() -> Animation {
    let mut frames: Vec<AnimFrame> = (1..=9)
        .chain((1..=8).rev())
        .map(|i| image(i, 200))
        .collect();
    frames.last_mut().unwrap().next_op = 9;
    Animation {
        key: goo_vent::ANIM.to_string(),
        frame_sounds: vec![Vec::new(); frames.len()],
        frames,
        ..Animation::default()
    }
}

/// The flat level as the sixth level, Claw starting at `start`, with
/// `objects`.
fn level_with(start: (i32, i32), objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 6;
    level.start = start;
    level
        .animations
        .insert(goo_vent::ANIM.to_string(), Arc::new(eruption()));
    level
        .images
        .insert(goo_vent::IMAGE_SET.into(), ImageFrames::range(1, 9));
    level.objects = objects;
    level
}

fn vent_at(x: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("GooVent", x, VENT_Y, 0);
    s.image_set = goo_vent::IMAGE_SET.into();
    s.frame = 1;
    s
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

/// One tick of a vent: its picture after the tick and its box in it.
fn look(sim: &Sim, id: ObjectId) -> (i32, Rect) {
    let o = sim.objects().get(id).unwrap();
    (o.frame, o.attack_rect)
}

/// The tick of the vent's set-up (C): a vent live when the level loads is
/// set up by the load, before the first tick (0).
fn set_up(sim: &mut Sim, id: ObjectId) -> u64 {
    for _ in 0..10 {
        if sim.objects().get(id).unwrap().state == vent::IDLE {
            return sim.tick();
        }
        sim.step(TickInput::default());
    }
    panic!("the vent never ran");
}

#[test]
fn a_pass_is_205_ticks_dangerous_for_132_from_the_tick_after_the_fourth_picture() {
    // Claw 500 px away: the vent is live and out of his reach.
    let mut sim = Sim::new(level_with((1000, STAND_Y), vec![vent_at(1500)]), original());
    let id = find(&sim, "GooVent");
    let c = set_up(&mut sim, id);
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.type_flags, goo_vent::TYPE);
    assert_eq!(o.hit_type_flags, goo_vent::HIT_MASK);
    assert!(!o.attack_rect.is_set(), "harmless after its set-up");
    let rng = sim.rng_state();
    let mut ticks = vec![(0, (o.frame, o.attack_rect))];
    for _ in 0..700 {
        sim.step(TickInput::default());
        ticks.push((sim.tick() - c, look(&sim, id)));
        assert!(
            !sim.last_audio()
                .iter()
                .any(|a| matches!(a, AudioCue::Play(_))),
            "silent"
        );
    }
    assert_eq!(sim.rng_state(), rng, "no random draw");
    let at = |t: u64| ticks.iter().find(|(k, _)| *k == t).unwrap().1;
    for n in 0..3 {
        let p = 205 * n;
        // Harmless through C + 37, the fourth picture shown on C + 37.
        for t in p + 1..=p + 37 {
            assert!(!at(t).1.is_set(), "tick {t}");
        }
        assert_eq!(at(p + 1).0, 1);
        assert_eq!(at(p + 37).0, 4);
        // The first box on C + 38, the widest on C + 98 to C + 109.
        assert_eq!(at(p + 38).1, ATTACK_RECTS[0]);
        for t in p + 98..=p + 109 {
            assert_eq!(at(t).1, ATTACK_RECTS[5], "tick {t}");
        }
        // Dangerous through C + 169, harmless again from C + 170.
        let danger = (p + 1..=p + 205).filter(|&t| at(t).1.is_set()).count();
        assert_eq!(danger, 132);
        assert!(at(p + 169).1.is_set());
        assert!(!at(p + 170).1.is_set());
        // The next pass's first picture on C + 206: the last picture of
        // this pass and the first of the next are shown 25 ticks.
        let ones = (p + 170..p + 240).filter(|&t| at(t).0 == 1).count();
        assert_eq!(ones, 25);
    }
    // Every tick's box follows the picture shown at its start.
    for w in ticks.windows(2) {
        let want = goo_vent::attack_rect(w[0].1.0)
            .unwrap()
            .unwrap_or(Rect::UNSET);
        assert_eq!(w[1].1.1, want, "tick {}", w[1].0);
    }
}

#[test]
fn claw_in_the_splash_loses_one_health_at_most_three_times_a_pass() {
    for side in [-1, 1] {
        // Claw standing 16 px from the vent, on either side.
        let x = 1500 - side * 16;
        let mut sim = Sim::new(level_with((x, STAND_Y), vec![vent_at(1500)]), original());
        let id = find(&sim, "GooVent");
        let c = set_up(&mut sim, id);
        let mut hits = Vec::new();
        let mut last = sim.claw().stats.health;
        for _ in 0..205 {
            sim.step(TickInput::default());
            let now = sim.claw().stats.health;
            if now != last {
                assert_eq!(sim.player().unwrap().state, state::HIT);
                hits.push((sim.tick() - c, now));
            }
            last = now;
        }
        // Hit on the first dangerous tick, again 48 ticks later while he is
        // still in the splash, a third time 48 after that; 1 health each.
        // (This level has none of Claw's pictures, so his hit reaction ends
        // at once and he is not pushed out; the push is checked on the
        // install.)
        assert_eq!(hits, [(38, 99), (86, 98), (134, 97)], "side {side}");
    }
}

#[test]
fn the_splash_sets_off_a_keg() {
    let feet = keg::move_box(keg::KEG_BOX).bottom;
    let mut k = ObjectSpec::at("PowderKeg", 1520, 959 - feet, 0);
    k.image_set = "LEVEL_POWDERKEG".into();
    let mut sim = Sim::new(
        level_with((1250, STAND_Y), vec![vent_at(1500), k]),
        original(),
    );
    let id = find(&sim, "GooVent");
    let c = set_up(&mut sim, id);
    let kid = find(&sim, "PowderKeg");
    let phase = |sim: &Sim| {
        sim.objects()
            .get(kid)
            .and_then(keg::data_of)
            .map(|d| d.phase)
    };
    let mut exploded = None;
    for _ in 0..100 {
        sim.step(TickInput::default());
        if exploded.is_none() && phase(&sim) == Some(keg::Phase::Exploding) {
            exploded = Some(sim.tick() - c);
        }
    }
    assert_eq!(exploded, Some(38), "on the first dangerous tick");
}

#[test]
fn a_vent_erupts_only_while_it_is_live() {
    // Far from Claw the vent stays dormant and is never set up; brought
    // near, it is set up by the warp and starts its first pass on the
    // next tick.
    let mut sim = Sim::new(level_with((1000, STAND_Y), vec![vent_at(9000)]), original());
    let id = find(&sim, "GooVent");
    idle(&mut sim, 20);
    assert_eq!(sim.objects().get(id).unwrap().state, 0, "never ran");
    sim.with_world(|w| w.claw.special.warp_to = Some((8500, STAND_Y)));
    sim.step(TickInput::default());
    assert_eq!(sim.objects().get(id).unwrap().state, vent::IDLE);
    assert_eq!(look(&sim, id).0, 1, "the first picture, applied");
    let woke = sim.tick() - 1;
    let mut frames = Vec::new();
    for _ in 0..60 {
        sim.step(TickInput::default());
        frames.push((sim.tick() - woke, look(&sim, id)));
    }
    let first_box = frames.iter().find(|(_, l)| l.1.is_set()).map(|(t, _)| *t);
    assert_eq!(first_box, Some(38));
    // Claw sent away: the vent runs the tick of the warp and falls asleep
    // at its end; for 100 ticks it keeps its picture and box.
    sim.with_world(|w| w.claw.special.warp_to = Some((1000, STAND_Y)));
    sim.step(TickInput::default());
    let before = look(&sim, id);
    idle(&mut sim, 100);
    assert_eq!(look(&sim, id), before, "frozen while dormant");
    assert!(!sim.objects().get(id).unwrap().is_listed());
}
