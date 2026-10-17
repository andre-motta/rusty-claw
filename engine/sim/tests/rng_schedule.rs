//! The schedule of the shared random stream on synthetic levels: which
//! objects are live (the small objects' window), when an object made in
//! the update pass first runs, when a pickup's glitter draws its delay,
//! when the ambient sounds' timers run, when a pacing enemy is at a limit,
//! how far a woken enemy first moves, and the order of the two jitter
//! draws of Claw's hit burst.
//!
//! The install test `install_rng_schedule.rs` plays the same rules on the
//! first level, at the ticks where each one shows.

mod common;

use std::sync::Arc;

use claw_sim::activation::{SMALL_MARGIN, WIDENED_SMALL_MARGIN};
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::effect::EffectData;
use claw_sim::logic::enemy::{self, EnemyState, motion, officer, states};
use claw_sim::logic::pickup::{self, PickupData};
use claw_sim::logic::sound_object;
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::hit::PickupFields;
use claw_sim::rng::Rng;
use claw_sim::{
    ActivationConfig, ActivationMode, Buttons, GameLogic, ObjectData, Sim, SimConfig, SimLevel,
    TickInput,
};
use common::*;

/// Number of draws that take the stream from `from` to `to` (at most
/// 1000).
fn draws_between(from: u32, to: u32) -> usize {
    let mut r = Rng::new(from);
    for n in 0..=1000 {
        if r.state() == to {
            return n;
        }
        r.next_u15();
    }
    panic!("{to} is not within 1000 draws of {from}");
}

/// The values of the next `n` draws from `state`.
fn values(state: u32, n: usize) -> Vec<u32> {
    let mut r = Rng::new(state);
    (0..n).map(|_| r.next_u15()).collect()
}

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect()
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

// ------------------------------------------------------------ rule 1

#[test]
fn the_small_objects_window_is_831_by_623_in_the_original_modes() {
    // Measured on the original game while it plays; in the first level a
    // crate stack 429 px right of the start wakes in tick 6 and its two
    // crates roll from tick 8 (see install_rng_schedule.rs).
    assert_eq!(SMALL_MARGIN, (95, 71));
    for cfg in [
        ActivationConfig::original(),
        ActivationConfig::frozen_edge(),
    ] {
        assert_eq!(cfg.half_extents(2, cfg.view), (415, 311), "{:?}", cfg.mode);
        assert_eq!(cfg.half_extents(0, cfg.view), (800, 600));
        assert_eq!(cfg.half_extents(1, cfg.view), (1280, 960));
    }
    // The widened mode keeps its parameters for its small objects, but its
    // breakable exceptions wake by the measured window.
    let wide = ActivationConfig::widened((640, 480));
    assert_eq!(wide.mode, ActivationMode::Widened);
    assert_eq!(wide.margins[2], WIDENED_SMALL_MARGIN);
    assert_eq!(wide.half_extents(2, wide.original_view), (384, 288));
    assert_eq!(wide.exception_half_extents(2), (415, 311));
}

#[test]
fn a_pickup_wakes_inside_the_measured_window() {
    // Claw starts at x = 1000 on a flat floor; the window's centre is the
    // view's. A treasure 400 px to the right is live from the start (the
    // 768x576 window would leave it asleep), one 420 px away is not.
    let mut l = flat_level();
    let mut near = ObjectSpec::at("TreasurePowerup", 0, 0, 0);
    near.image_set = "GAME_TREASURE_COINS".into();
    let mut far = near.clone();
    let sim = Sim::new(l.clone(), SimConfig::default());
    let (cx, cy) = sim.view().centre;
    near.record[5] = cx + 400;
    near.record[6] = cy;
    far.record[5] = cx + 420;
    far.record[6] = cy;
    l.objects = vec![near, far];
    for cfg in [
        SimConfig::default(),
        SimConfig {
            activation: ActivationConfig::original(),
            ..SimConfig::default()
        },
    ] {
        let sim = Sim::new(l.clone(), cfg);
        let listed: Vec<i32> = find(&sim, "TreasurePowerup")
            .into_iter()
            .filter(|&id| obj(&sim, id).is_listed())
            .map(|id| obj(&sim, id).x)
            .collect();
        assert_eq!(listed, vec![cx + 400]);
    }
}

// ------------------------------------------------------- rules 2 and 3

/// A flat level with a treasure at depth 1000 (its glitter goes at 1001)
/// 600 px right of the start, beyond the small objects' window, and, with
/// `neighbour`, an always-live object at depth 1001 near the start.
fn treasure_level(neighbour: bool) -> SimLevel {
    let mut l = flat_level();
    let mut t = ObjectSpec::at("TreasurePowerup", 1600, 930, 1000);
    t.image_set = "GAME_TREASURE_COINS".into();
    t.record[0] = 1600;
    l.objects.push(t);
    if neighbour {
        let mut n = ObjectSpec::at("NotALogic", 1000, 930, 1001);
        n.record[10] = flags::ALWAYS_ACTIVE as i32;
        l.objects.push(n);
    }
    l
}

/// Walks Claw right until the treasure wakes; returns the sim at the end
/// of that tick, the treasure and the random state before the tick.
fn walk_until_the_treasure_wakes(l: SimLevel) -> (Sim, ObjectId, u32) {
    let mut sim = pressing_sim(l, SimConfig::default());
    let t = find(&sim, "TreasurePowerup")[0];
    assert!(!obj(&sim, t).is_listed(), "asleep at the start");
    for _ in 0..400 {
        let before = sim.rng_state();
        sim.step(TickInput {
            held: Buttons(RIGHT),
            pressed: Buttons(0),
        });
        if obj(&sim, t).is_listed() {
            return (sim, t, before);
        }
    }
    panic!("the treasure never woke");
}

fn glitter_of(sim: &Sim, t: ObjectId) -> ObjectId {
    match &obj(sim, t).data {
        ObjectData::Pickup(PickupData { glitter, .. }) => glitter.expect("treasure glitters"),
        other => panic!("not a pickup: {other:?}"),
    }
}

#[test]
fn a_glitter_landing_right_after_its_pickup_first_runs_in_the_next_tick() {
    // The pickup makes its glitter in its first call. The glitter joins
    // the live list at once, right after the pickup (nothing else is at
    // its depth), so before the object the pass visits next: it first
    // runs, and draws its start delay, in the next tick. It is not run
    // when it is made.
    let (mut sim, t, before) = walk_until_the_treasure_wakes(treasure_level(false));
    let g = glitter_of(&sim, t);
    assert!(obj(&sim, g).is_listed());
    assert_eq!(obj(&sim, g).state, 0, "not run yet");
    assert_eq!(obj(&sim, g).image_set, "", "not run yet");
    assert_eq!(&*obj(&sim, g).logic, pickup::GLITTER);
    // The pickup's first call draws nothing.
    assert_eq!(draws_between(before, sim.rng_state()), 0);
    let before = sim.rng_state();
    sim.step(TickInput::default());
    assert_ne!(obj(&sim, g).state, 0, "its first call");
    assert_eq!(obj(&sim, g).image_set, "GAME_GLITTER");
    // Its delay is the tick's draw (nothing else draws here).
    assert_eq!(draws_between(before, sim.rng_state()), 1);
    let delay = values(before, 1)[0] % 251;
    // The delay counts down from the next pass on.
    assert_eq!(obj(&sim, g).time_delay, delay);
}

#[test]
fn a_glitter_landing_after_the_next_object_runs_in_the_same_pass() {
    // With an object at the glitter's depth after the pickup, the glitter
    // lands after it, so after the object the pass visits next: it runs
    // in the same pass that made it and draws its delay in that tick.
    let (sim, t, before) = walk_until_the_treasure_wakes(treasure_level(true));
    let g = glitter_of(&sim, t);
    assert_eq!(obj(&sim, g).image_set, "GAME_GLITTER", "ran in this tick");
    assert_eq!(draws_between(before, sim.rng_state()), 1);
    assert_eq!(obj(&sim, g).time_delay, values(before, 1)[0] % 251);
}

// ------------------------------------------------------------ rule 4

const CRIES: &str = "LEVEL_AMBIENT_CRIES";

/// A level with one timed global ambient sound: on for 400 to 900 ms, off
/// for `off` ms.
fn ambient_level(off: i32) -> SimLevel {
    let mut l = flat_level();
    l.sounds.insert(CRIES.into(), Some(1000));
    let mut s = ObjectSpec::at("GlobalAmbientSound", 1000, 900, 0);
    s.anim = CRIES.into();
    s.record[16] = 80;
    s.record[19] = 400;
    s.record[20] = 900;
    s.record[21] = off;
    s.record[22] = off;
    l.objects.push(s);
    l
}

#[test]
fn the_ambient_timers_first_count_down_in_the_first_tick_after_the_rest_of_it() {
    // A timed sound registered while the level loads draws its first timer
    // then (here exactly 54 ms) and flips after three ticks of 18 ms: the
    // manager's update runs once per tick, after the update and collision
    // passes, from the first tick on, and not while the level loads.
    let mut sim = Sim::new(ambient_level(54), SimConfig::default());
    let m = sim
        .objects()
        .all()
        .map(|(id, _)| id)
        .find(|&id| sound_object::manager_at(sim.objects(), id).is_some())
        .expect("the manager");
    assert!(!obj(&sim, m).is_listed(), "never in the live list");
    let mut flips = Vec::new();
    for k in 0..5 {
        let before = sim.rng_state();
        sim.step(TickInput::default());
        flips.push((k, draws_between(before, sim.rng_state())));
    }
    // The flip turns the sound on and draws its on time, in tick 2 (the
    // third tick); with a decrement while the level loads it would come in
    // tick 1.
    assert_eq!(flips[..5], [(0, 0), (1, 0), (2, 1), (3, 0), (4, 0)]);
}

#[test]
fn an_ambient_manager_made_after_the_first_tick_is_updated() {
    // An ambient loop 1700 px right of the start is asleep at first, so the
    // first ticks find no manager; it registers when Claw's walk wakes it,
    // and the manager made then is updated from that tick on.
    let mut l = flat_level();
    l.sounds.insert(CRIES.into(), Some(1000));
    let mut s = ObjectSpec::at("AmbientSound", 2700, 900, 0);
    s.anim = CRIES.into();
    l.objects.push(s);
    let mut sim = pressing_sim(l, SimConfig::default());
    idle(&mut sim, 3);
    assert!(sound_object::manager_of(sim.objects()).is_none());
    let walk = TickInput {
        held: Buttons(RIGHT),
        pressed: Buttons(0),
    };
    for _ in 0..400 {
        sim.step(walk);
        if sound_object::manager_of(sim.objects()).is_some() {
            break;
        }
    }
    let m = sound_object::manager_of(sim.objects()).expect("the loop woke and made a manager");
    assert_eq!(m.entries.len(), 1);
    sim.step(walk);
    let m = sound_object::manager_of(sim.objects()).expect("the manager");
    assert!(m.entries[0].playing, "the manager runs once it is made");
}

// ------------------------------------------------------- rules 5 and 6

/// An officer on a flat floor 600 px right of Claw, pacing between 1500
/// and 1700 (built and standing after a few ticks).
fn officer_sim() -> (Sim, ObjectId) {
    let mut l = flat_level();
    let pose = |key: &str| Arc::new(Animation::once(key, &[(100, 0)]));
    for key in officer::STAND.iter().copied().chain([
        "LEVEL_OFFICER_HOME",
        "LEVEL_OFFICER_FASTADVANCE",
        "LEVEL_OFFICER_ADVANCE",
        "LEVEL_OFFICER_STRIKE",
    ]) {
        l.animations.insert(key.to_string(), pose(key));
    }
    let mut o = ObjectSpec::at("Officer", 1600, 900, 4000);
    o.image_set = "LEVEL_OFFICER".into();
    o.record[enemy::field::X_MIN] = 1500;
    o.record[enemy::field::X_MAX] = 1700;
    l.objects.push(o);
    let mut sim = Sim::new(l, SimConfig::default());
    idle(&mut sim, 10);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| matches!(o.kind, LogicKind::Game(GameLogic::Officer)))
        .map(|(id, _)| id)
        .expect("an officer");
    assert_eq!(obj(&sim, id).physics_type, ptype::STAND);
    (sim, id)
}

/// Puts the officer at `x` with `fraction` carried by its last step and
/// pacing, then runs one pacing call; returns the draws it made.
fn pace_at(sim: &mut Sim, id: ObjectId, x: i32, fraction: f64) -> usize {
    let before = sim.rng_state();
    sim.with_world(|w| {
        let y = w.objects.get(id).unwrap().y;
        w.objects.get_mut(id).unwrap().x = x;
        let d = enemy::data_mut(w, id).unwrap();
        d.carry.0 = fraction;
        d.moved_to = Some((x, y));
        d.mode = 6;
        enemy::set_state(w, id, EnemyState::Pace);
        states::pace_tick(w, id);
    });
    draws_between(before, sim.rng_state())
}

#[test]
fn a_pacing_enemy_is_at_its_limit_only_when_it_stands_exactly_on_it() {
    let (mut sim, id) = officer_sim();
    // On the limit's pixel with a fraction carried: not at the limit, no
    // draw; the exact position equals it: a draw of 0..100 (and those of
    // going idle when it is below 50).
    assert_eq!(pace_at(&mut sim, id, 1700, 0.16), 0);
    assert!(pace_at(&mut sim, id, 1700, 0.0) >= 1);
    assert_eq!(pace_at(&mut sim, id, 1500, 0.4), 0);
    assert!(pace_at(&mut sim, id, 1500, 0.0) >= 1);
    // A fraction left over from a move made by something else is not
    // carried: the position is exact.
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.carry.0 = 0.5;
        d.moved_to = Some((0, 0));
        states::pace_tick(w, id);
    });
    sim.with_world(|w| {
        assert_eq!(enemy::exact_x(w, id), 1500.0);
    });
}

/// Runs one movement step of the officer at game time `now`, walking at
/// `vx` from 1600 with its last step at `last`; returns its new x.
fn step_from(sim: &mut Sim, id: ObjectId, vx: f64, last: Option<u64>) -> i32 {
    sim.with_world(|w| {
        let now = w.now_ms;
        let y = w.objects.get(id).unwrap().y;
        w.objects.get_mut(id).unwrap().x = 1600;
        let d = enemy::data_mut(w, id).unwrap();
        d.carry = (0.0, 0.0);
        d.moved_to = Some((1600, y));
        d.vx = vx;
        d.last_move_ms = last.map(|ms| now - ms);
        motion::integrate(w, id);
        w.objects.get(id).unwrap().x
    })
}

#[test]
fn a_woken_enemy_first_moves_for_the_time_it_slept_up_to_200_ms() {
    let (mut sim, id) = officer_sim();
    idle(&mut sim, 100);
    // One tick: 60 px/s for 18 ms.
    assert_eq!(step_from(&mut sim, id, 60.0, Some(18)), 1601);
    // The first step: one tick.
    assert_eq!(step_from(&mut sim, id, 60.0, None), 1601);
    // After 108 ms asleep: 120 px/s for 108 ms is 12.96 px.
    assert_eq!(step_from(&mut sim, id, 120.0, Some(108)), 1612);
    // After a second asleep: 200 ms at most, 12 px at 60 px/s.
    assert_eq!(step_from(&mut sim, id, 60.0, Some(1000)), 1612);
    // Every step records its time.
    sim.step(TickInput::default());
    let d = enemy::data_of(obj(&sim, id)).unwrap().clone();
    assert_eq!(d.last_move_ms, Some((sim.tick() - 1) * 18));
}

// ------------------------------------------------------------ rule 7

#[test]
fn claws_hit_burst_draws_its_y_jitter_before_its_x_jitter() {
    // An enemy touching Claw from his right hurts him in the collision
    // pass; the burst shows 4 px toward the enemy from his x, at his y,
    // each jittered by a draw of 0..9 less 4: the y offset is drawn first.
    for seed in [1, 7, 31, 1234] {
        let mut sim = pressing_sim(
            flat_level(),
            SimConfig {
                seed,
                ..SimConfig::default()
            },
        );
        idle(&mut sim, 5);
        let (x, y) = {
            let o = sim.player().unwrap();
            (o.x, o.y)
        };
        let mut e = Object::new(0, x, y, 10, Arc::from("Toucher"), LogicKind::Inert);
        e.flags = flags::WORLD;
        e.type_flags = types::ENEMY;
        e.attack_rect = Rect::new(-20, -20, 20, 20);
        PickupFields::default().store(&mut e);
        e.damage = 10;
        sim.spawn(e);
        let before = sim.rng_state();
        sim.step(TickInput::default());
        assert_eq!(sim.stats().health, 90);
        let n = draws_between(before, sim.rng_state());
        let d = values(before, n);
        let burst = find(&sim, "HitBurst")
            .into_iter()
            .find_map(|id| match &obj(&sim, id).data {
                ObjectData::Effect(EffectData {
                    owner: Some(_),
                    offset,
                    ..
                }) => Some(*offset),
                _ => None,
            })
            .expect("Claw's hit burst");
        // The damage step's draws end with the two jitter draws and the
        // burst's size.
        let (first, second) = (d[n - 3] % 9, d[n - 2] % 9);
        assert_eq!(
            burst,
            (4 + second as i32 - 4, first as i32 - 4),
            "seed {seed}"
        );
    }
}
