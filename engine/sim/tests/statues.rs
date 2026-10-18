//! Statues on a synthetic level: their set-up, a break and its fan of
//! treasure, the two-hit statue, the unbreakable one, and the rubble that
//! stays.

mod common;

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::statue::{self, state};
use claw_sim::logic::{self, ObjectData, crates, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::rng::Rng;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The statue's picture set, ten pictures like the fifth level's.
const SET: &str = "LEVEL_STATUE";
/// The box of its first picture.
const BOX: Rect = Rect::new(-41, -83, 46, 82);

/// Loot codes of the fifth level's first front statue, in record order
/// (first box: left, top, right, bottom; second box the same): released
/// as 33, 33, 8, 44, 3, 9, 33, 33.
const FRONT_LOOT: [i32; 8] = [33, 8, 33, 44, 3, 33, 9, 33];

/// A flat level with one statue running `name` at (1200, 900) and depth
/// `z`, with `health` and the given loot in its two boxes.
fn statue_level(name: &str, z: i32, health: i32, loot: [i32; 8]) -> SimLevel {
    let mut level = flat_level();
    let mut spec = ObjectSpec::at(name, 1200, 900, z);
    spec.image_set = SET.into();
    spec.record[11] = 2;
    spec.record[18] = health;
    for (i, c) in loot.iter().enumerate() {
        spec.record[35 + i] = *c;
    }
    level.objects.push(spec);
    let mut frames = ImageFrames::range(1, 10);
    frames.boxes.insert(1, BOX);
    level.images.insert(SET.into(), frames);
    for code in [33, 8, 44, 3, 9, 1] {
        let set = pickup::image_set_in(code, 5).unwrap();
        level.images.insert(set.into(), ImageFrames::range(1, 4));
    }
    let frames: Vec<(i32, i32)> = (2..=10).map(|i| (i, 125)).collect();
    let anim = Animation::once(statue::BREAK_ANIM, &frames);
    level.animations.insert(anim.key.clone(), Arc::new(anim));
    level
}

fn game(level: SimLevel) -> (Sim, ObjectId) {
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 2);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| statue::NAMES.contains(&&*o.logic))
        .map(|(id, _)| id)
        .expect("the statue");
    (sim, id)
}

/// A stand-in for Claw's blow: an object of Claw's kind.
fn blow(sim: &mut Sim, kind: u32) -> ObjectId {
    let mut o = Object::new(0, 1200, 900, 0, "Blow".into(), LogicKind::Inert);
    o.type_flags = kind;
    o.damage = 1;
    sim.spawn(o)
}

fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId) {
    sim.with_world(|w| logic::on_hit(w, id, attacker));
}

fn goodies(sim: &Sim) -> Vec<&Object> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == pickup::BOUNCING_GOODIE)
        .map(|(_, o)| o)
        .collect()
}

fn speed(o: &Object) -> (f64, f64) {
    match &o.data {
        ObjectData::Pickup(d) => d.goodie.map_or((0.0, 0.0), |g| (g.vx, g.vy)),
        _ => (0.0, 0.0),
    }
}

#[test]
fn a_statue_is_set_up_as_a_one_hit_crate_with_its_pictures_box() {
    let (sim, id) = game(statue_level(statue::FRONT, 5000, 0, FRONT_LOOT));
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.state, state::IDLE);
    assert_eq!(o.frame, 1);
    assert_eq!(o.hit_rect, BOX, "unmirrored, though drawn mirrored");
    assert_eq!(o.type_flags, crates::CRATE_TYPE);
    assert_eq!(o.hit_type_flags, crates::CRATE_HIT_BY);
    assert_eq!(o.health, 1);
    assert_eq!(o.z, 5000);
    assert!(!o.move_rect.is_set() && !o.attack_rect.is_set());
    let d = statue::data_of(o).unwrap();
    assert_eq!(d.loot, [0, 33, 33, 8, 44, 3, 9, 33, 33]);
    // Placed at depth 0, either kind goes to 1100.
    for name in [statue::FRONT, statue::BEHIND] {
        let (sim, id) = game(statue_level(name, 0, 0, FRONT_LOOT));
        assert_eq!(sim.objects().get(id).unwrap().z, statue::DEFAULT_Z);
    }
}

#[test]
fn a_break_fans_the_treasure_out_and_draws_in_order() {
    for (name, z, dz) in [(statue::FRONT, 5000, -1), (statue::BEHIND, 1010, 1)] {
        let (mut sim, id) = game(statue_level(name, z, 0, FRONT_LOOT));
        let claw = blow(&mut sim, types::PLAYER);
        // The draws: eight upward speeds in drop order, then the break
        // sound's choice.
        let mut rng = Rng::new(sim.rng_state());
        let want: Vec<f64> = (0..8)
            .map(|_| -f64::from(rng.below(301) as i32 + 200))
            .collect();
        let sound = if rng.below(100) < 50 {
            "GAME_CRATEBREAK"
        } else {
            "GAME_CRATEBREAK2"
        };
        hit(&mut sim, id, claw);
        assert_eq!(sim.rng_state(), rng.state(), "nine draws");
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.state, state::BREAKING);
        assert!(o.flags & flags::SAFE != 0);
        let fan = goodies(&sim);
        assert_eq!(fan.len(), 8);
        let vx: Vec<f64> = fan.iter().map(|g| speed(g).0).collect();
        let vy: Vec<f64> = fan.iter().map(|g| speed(g).1).collect();
        assert_eq!(vx, [-100.0, -75.0, -50.0, -25.0, 0.0, 25.0, 50.0, 75.0]);
        assert_eq!(vy, want);
        for g in &fan {
            assert_eq!((g.x, g.y, g.z), (1200, 900 + statue::DROP_Y, z + dz));
        }
        // The codes in record order: the first box left, right, top,
        // bottom, then the second's.
        let codes: Vec<i32> = fan
            .iter()
            .map(|g| match &g.data {
                ObjectData::Pickup(d) => d.drop_code.unwrap(),
                _ => 0,
            })
            .collect();
        assert_eq!(codes, [33, 33, 8, 44, 3, 9, 33, 33]);
        sim.step(Default::default());
        let cues: Vec<&str> = sim
            .last_audio()
            .iter()
            .filter_map(|c| match c {
                AudioCue::Play(s) => Some(s.key.as_str()),
                _ => None,
            })
            .collect();
        assert!(cues.contains(&sound), "{cues:?}");
    }
}

#[test]
fn a_fan_of_odd_size_starts_at_minus_12_5_n_cut_towards_zero() {
    let (mut sim, id) = game(statue_level(
        statue::BEHIND,
        1010,
        0,
        [33, 0, 0, 1, 0, 3, 0, 0],
    ));
    let claw = blow(&mut sim, types::PLAYER);
    hit(&mut sim, id, claw);
    let vx: Vec<f64> = goodies(&sim).iter().map(|g| speed(g).0).collect();
    assert_eq!(vx, [-37.0, -12.0, 13.0]);
}

#[test]
fn a_broken_statue_stays_as_rubble() {
    let (mut sim, id) = game(statue_level(statue::FRONT, 5000, 0, FRONT_LOOT));
    let shot = blow(&mut sim, types::PISTOL_SHOT);
    hit(&mut sim, id, shot);
    assert!(
        sim.objects().get(shot).unwrap().flags & flags::DESTROY != 0,
        "a pistol shot is used up"
    );
    let mut states = Vec::new();
    for _ in 0..120 {
        sim.step(Default::default());
        let o = sim.objects().get(id).expect("never removed");
        states.push((o.state, o.frame));
    }
    let broken = states.iter().position(|s| s.0 == state::BROKEN).unwrap();
    assert_eq!(states[broken + 1].0, state::IDLE);
    assert_eq!(states.last(), Some(&(state::IDLE, 10)));
    // Safe for good: another blow does nothing.
    let claw = blow(&mut sim, types::PLAYER);
    let rng = sim.rng_state();
    let n = goodies(&sim).len();
    hit(&mut sim, id, claw);
    assert_eq!((sim.rng_state(), goodies(&sim).len()), (rng, n));
}

#[test]
fn a_two_hit_statue_shows_damage_and_pauses_500_ms() {
    let (mut sim, id) = game(statue_level(statue::BEHIND, 1010, 5, FRONT_LOOT));
    assert_eq!(sim.objects().get(id).unwrap().health, 2);
    let a = blow(&mut sim, types::PLAYER);
    hit(&mut sim, id, a);
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.health, o.frame, o.state), (1, 2, state::IDLE));
    assert!(goodies(&sim).is_empty());
    // Inside the pause a blow does nothing.
    idle(&mut sim, 10);
    let b = blow(&mut sim, types::PLAYER);
    hit(&mut sim, id, b);
    assert_eq!(sim.objects().get(id).unwrap().health, 1);
    // After it, the next blow breaks it.
    idle(&mut sim, 20);
    let c = blow(&mut sim, types::PLAYER);
    hit(&mut sim, id, c);
    assert_eq!(sim.objects().get(id).unwrap().state, state::BREAKING);
    assert_eq!(goodies(&sim).len(), 8);
}

#[test]
fn a_statue_of_999_health_cannot_be_broken() {
    let (sim, id) = game(statue_level(statue::FRONT, 5000, 999, FRONT_LOOT));
    let o = sim.objects().get(id).unwrap();
    assert!(o.flags & flags::NO_COLLIDE != 0 && o.flags & flags::SAFE != 0);
}
