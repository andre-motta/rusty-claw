//! The sixth level's ground blowers on synthetic levels: the jet's loop and
//! its two test ticks, the throw of a Claw over the nozzle (450 px, or the
//! record's height), the second test that throws again a Claw held inside
//! the nozzle box, the carried keg dropped by a throw, the objects a
//! blower leaves alone and the hiss of every live blower.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::container::CollisionEvent;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::ground_blower::{self, HISS, THROW_FRAME, state as blower};
use claw_sim::logic::{self, keg};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const SET: &str = ground_blower::IMAGE_SET;
const LIFT: u32 = 0x20;
/// The blower of the tests: the floor (top at 960) 90 px below it, so a
/// Claw standing on it is at y 899, 29 px below it, as on the sixth level.
const AT: (i32, i32) = (1200, 870);
const STAND_Y: i32 = 899;

fn image(i: i32, ms: i32) -> AnimFrame {
    AnimFrame {
        image_op: 3,
        image_index: i,
        duration: ms,
        ..AnimFrame::default()
    }
}

/// The sixth level's jet: pictures 1 to 17 for 25 ms each, then picture 18
/// (not in the set) for 400 ms, looping; the fourth picture hisses.
fn jet() -> Animation {
    let mut frames: Vec<AnimFrame> = (1..=17).map(|i| image(i, 25)).collect();
    frames.push(image(18, 400));
    frames[3].flags = 0x2;
    let mut frame_sounds = vec![Vec::new(); frames.len()];
    frame_sounds[3] = vec![Some(HISS.to_string())];
    Animation {
        key: ground_blower::ANIM.to_string(),
        frames,
        frame_sounds,
        ..Animation::default()
    }
}

/// `level` as the sixth level with the jet and the blower's image set
/// (its first picture 8 x 5 px, 78 px below the blower's position).
fn as_level6(mut level: SimLevel, objects: Vec<ObjectSpec>) -> SimLevel {
    level.number = 6;
    level
        .animations
        .insert(ground_blower::ANIM.to_string(), Arc::new(jet()));
    let mut frames = ImageFrames::range(1, 17);
    frames.boxes.insert(1, Rect::new(-11, 76, -4, 79));
    level.images.insert(SET.into(), frames);
    level.objects = objects;
    level
}

/// A flat level with Claw starting at `start` and `objects`.
fn level_with(start: (i32, i32), objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.start = start;
    as_level6(level, objects)
}

fn blower_at(x: i32, y: i32, height: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("GroundBlower", x, y, 5000);
    s.image_set = SET.into();
    s.frame = 1;
    s.record[ground_blower::HEIGHT_FIELD] = height;
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

fn frame(sim: &Sim, id: ObjectId) -> i32 {
    sim.objects().get(id).unwrap().frame
}

fn claw(sim: &Sim) -> (i32, i32, i32) {
    let o = sim.player().unwrap();
    (o.x, o.y, o.state)
}

fn hisses(sim: &Sim) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == HISS))
        .count()
}

/// The tick of the blower's set-up (C): a blower live when the level
/// loads is set up by the load, before the first tick (0).
fn set_up(sim: &mut Sim, id: ObjectId) -> u64 {
    for _ in 0..10 {
        if sim.objects().get(id).unwrap().state == blower::IDLE {
            return sim.tick();
        }
        sim.step(TickInput::default());
    }
    panic!("the blower never ran");
}

/// True when the blower threw Claw in the last tick: his jump starts where
/// he is and peaks `height` above him.
fn thrown_now(sim: &Sim, height: i32) -> bool {
    let (_, y, s) = claw(sim);
    let c = sim.claw();
    s == state::JUMP && c.jump_start_y == y && c.jump_peak_y == y - height
}

#[test]
fn the_jet_shows_its_first_picture_on_two_ticks_a_loop_and_hisses_on_the_fourth() {
    // Claw far from the nozzle, the blower live beside him.
    let mut sim = Sim::new(
        level_with((900, 600), vec![blower_at(AT.0, AT.1, 0)]),
        original(),
    );
    let id = find(&sim, "GroundBlower");
    let c = set_up(&mut sim, id);
    assert_eq!(frame(&sim, id), THROW_FRAME, "shown by the set-up");
    let mut ones = vec![0];
    let mut hiss = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        let t = sim.tick() - c;
        if frame(&sim, id) == THROW_FRAME {
            ones.push(t);
        }
        if hisses(&sim) > 0 {
            hiss.push(t);
        }
    }
    assert_eq!(ones, [0, 1, 57, 58, 114, 115, 171, 172]);
    assert_eq!(hiss, [6, 63, 120, 177]);
    // The 18th picture is not in the set: shown for 23 ticks a loop.
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.z, 5000);
    assert!(claw(&sim).2 == state::STAND, "never thrown");
}

#[test]
fn a_claw_standing_on_the_nozzle_is_thrown_450_px_straight_up() {
    let mut sim = Sim::new(
        level_with((AT.0, STAND_Y), vec![blower_at(AT.0, AT.1, 0)]),
        original(),
    );
    let id = find(&sim, "GroundBlower");
    let c = set_up(&mut sim, id);
    let mut throws = Vec::new();
    let mut ys = Vec::new();
    for _ in 0..300 {
        sim.step(TickInput::default());
        if thrown_now(&sim, 450) {
            throws.push(sim.tick() - c);
            assert_eq!(claw(&sim).1, STAND_Y, "from where he stands");
            assert_eq!(frame(&sim, id), THROW_FRAME);
        }
        ys.push((sim.tick() - c, claw(&sim).1));
    }
    // The first test tick throws him; each landing waits for a test tick.
    assert_eq!(throws.first(), Some(&1));
    assert!(throws.len() >= 3, "{throws:?}");
    for &t in &throws {
        // 25 px up on the next tick, 450 px up at the apex, x unchanged.
        let next = ys.iter().find(|(k, _)| *k == t + 1).unwrap().1;
        assert_eq!(next, STAND_Y - 25);
        let apex = ys
            .iter()
            .filter(|(k, _)| *k > t && *k < t + 60)
            .map(|(_, y)| *y)
            .min()
            .unwrap();
        assert_eq!(apex, STAND_Y - 450);
    }
    assert_eq!(claw(&sim).0, AT.0);
}

#[test]
fn the_records_height_sets_the_throw() {
    let mut sim = Sim::new(
        level_with((AT.0, STAND_Y), vec![blower_at(AT.0, AT.1, 400)]),
        original(),
    );
    let id = find(&sim, "GroundBlower");
    let c = set_up(&mut sim, id);
    assert_eq!(
        ground_blower::data_of(sim.objects().get(id).unwrap()).map(|d| d.height),
        Some(400)
    );
    let mut apex = STAND_Y;
    let mut thrown = None;
    for _ in 0..60 {
        sim.step(TickInput::default());
        if thrown.is_none() && thrown_now(&sim, 400) {
            thrown = Some(sim.tick() - c);
        }
        apex = apex.min(claw(&sim).1);
    }
    assert_eq!(thrown, Some(1));
    assert_eq!(apex, STAND_Y - 400);
}

#[test]
fn a_claw_held_in_the_nozzle_box_is_thrown_again_on_the_second_test() {
    // A ceiling 15 px above Claw's head: the first throw lifts him into
    // it and he stays inside the nozzle box (which reaches 22 px above
    // standing), so the loop's second test throws him again from there.
    let mut rows = vec![".".repeat(200); 15];
    rows[12].replace_range(17..20, "###");
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (AT.0, STAND_Y));
    let mut sim = Sim::new(as_level6(level, vec![blower_at(AT.0, AT.1, 0)]), original());
    let id = find(&sim, "GroundBlower");
    let c = set_up(&mut sim, id);
    // Settle on the floor first: the first loop's single test tick
    // throws him from the start.
    let mut starts = Vec::new();
    for _ in 0..120 {
        sim.step(TickInput::default());
        let t = sim.tick() - c;
        let (_, y, s) = claw(&sim);
        if s == state::JUMP && sim.claw().jump_start_y == y && sim.claw().jump_peak_y == y - 450 {
            starts.push((t, y));
        }
    }
    let loop2: Vec<(u64, i32)> = starts
        .iter()
        .copied()
        .filter(|(t, _)| (57..=58).contains(t))
        .collect();
    assert_eq!(loop2.len(), 2, "both test ticks throw: {starts:?}");
    assert_eq!(loop2[0], (57, STAND_Y));
    assert!(
        loop2[1].1 < STAND_Y && loop2[1].1 >= STAND_Y - 22,
        "{loop2:?}"
    );
}

#[test]
fn the_nozzle_box_reaches_68_px_across_and_a_little_above_the_floor() {
    // Claw's movement box (-16, -52, 16, 60) against the nozzle (-16, 66,
    // 21, 89), right and bottom edges excluded.
    let nozzle = Rect::new(-16, 66, 21, 89).translate(AT.0, AT.1);
    let claw_at = |x: i32, y: i32| Rect::new(-16, -52, 16, 60).translate(x, y);
    let meets = |x, y| ground_blower::boxes_meet(claw_at(x, y), nozzle);
    for (x, y, want) in [
        (AT.0 - 31, STAND_Y, true),
        (AT.0 - 32, STAND_Y, false),
        (AT.0 + 36, STAND_Y, true),
        (AT.0 + 37, STAND_Y, false),
        (AT.0, AT.1 + 7, true),
        (AT.0, AT.1 + 6, false),
        (AT.0, AT.1 + 140, true),
        (AT.0, AT.1 + 141, false),
    ] {
        assert_eq!(meets(x, y), want, "({x}, {y})");
    }
    // A Claw whose feet are 22 px above standing is still in; 23 is out.
    assert!(meets(AT.0, STAND_Y - 22));
    assert!(!meets(AT.0, STAND_Y - 23));
}

#[test]
fn a_throw_drops_the_keg_claw_carries() {
    let mut sim = Sim::new(
        level_with((AT.0, STAND_Y), vec![blower_at(AT.0, AT.1, 0)]),
        original(),
    );
    let id = find(&sim, "GroundBlower");
    let c = set_up(&mut sim, id);
    // Thrown at once; wait for him to land on the nozzle again.
    let mut landed = false;
    for _ in 0..100 {
        sim.step(TickInput::default());
        if sim.tick() - c > 2 && claw(&sim).2 == state::STAND {
            landed = true;
            break;
        }
    }
    assert!(landed);
    // A keg beside him: he lifts it before the next test tick.
    let feet = keg::move_box(keg::KEG_BOX).bottom;
    let k = sim.spawn(logic::new_object(
        "PowderKeg",
        AT.0 + 30,
        STAND_Y + 60 - feet,
        4000,
    ));
    sim.step(TickInput::default());
    sim.step(press(LIFT));
    assert_eq!(sim.claw().special.lifted, Some(k), "lifted");
    let phase = |sim: &Sim| keg::data_of(sim.objects().get(k).unwrap()).unwrap().phase;
    let mut thrown = false;
    for _ in 0..80 {
        sim.step(TickInput::default());
        if thrown_now(&sim, 450) {
            thrown = true;
            break;
        }
    }
    assert!(thrown, "thrown while lifting");
    assert_eq!(sim.claw().special.lifted, None, "the keg is let go");
    assert!(
        matches!(phase(&sim), keg::Phase::Dropped | keg::Phase::Idle),
        "{:?}",
        phase(&sim)
    );
}

fn involves(e: &CollisionEvent, id: ObjectId) -> bool {
    match *e {
        CollisionEvent::Attack { attacker, victim }
        | CollisionEvent::Hit { attacker, victim }
        | CollisionEvent::Damage { attacker, victim } => attacker == id || victim == id,
    }
}

#[test]
fn a_blower_throws_nothing_but_claw() {
    // A keg resting on the nozzle, Claw far away.
    let feet = keg::move_box(keg::KEG_BOX).bottom;
    let mut k = ObjectSpec::at("PowderKeg", AT.0, 959 - feet, 0);
    k.image_set = "LEVEL_POWDERKEG".into();
    let mut sim = Sim::new(
        level_with((900, 600), vec![blower_at(AT.0, AT.1, 0), k]),
        original(),
    );
    let id = find(&sim, "GroundBlower");
    set_up(&mut sim, id);
    idle(&mut sim, 10);
    let kid = find(&sim, "PowderKeg");
    let y = sim.objects().get(kid).unwrap().y;
    for _ in 0..130 {
        sim.step(TickInput::default());
        assert_eq!(sim.objects().get(kid).unwrap().y, y, "the keg stays");
        assert!(sim.collision_events().iter().all(|e| !involves(e, id)));
    }
}

#[test]
fn every_live_blower_hisses_on_screen_or_not() {
    // Two blowers set up in the same tick, one on screen and one 60 px
    // past the screen's right edge but inside the small objects' window:
    // both hiss once a loop, in the same tick, without any random draw.
    let mut sim = Sim::new(
        level_with(
            (900, STAND_Y),
            vec![blower_at(1000, AT.1, 0), blower_at(900 + 320 + 60, AT.1, 0)],
        ),
        original(),
    );
    let rng = sim.rng_state();
    let mut heard = Vec::new();
    for _ in 0..130 {
        sim.step(TickInput::default());
        let n = hisses(&sim);
        if n > 0 {
            heard.push((sim.tick(), n));
        }
    }
    assert_eq!(heard.len(), 3, "{heard:?}");
    assert!(heard.iter().all(|&(_, n)| n == 2), "{heard:?}");
    assert_eq!(heard[1].0 - heard[0].0, 57);
    assert_eq!(sim.rng_state(), rng);
}
