//! The shared human-enemy states on synthetic levels: the hop up, the hop
//! back, moving back and moving ahead, the blocks and the release they
//! arm, the human idle state, the springboard launch, and the hit of a
//! springboard-like object; with their gates, speeds, exits and random
//! draws. An officer stands in for any human class; the walking states'
//! wall and standstill exits use Raux, whose logic calls carry what they
//! read.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::human::{self, HumanCall};
use claw_sim::logic::enemy::{self, Deferred, EnemyState, states};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// An animation of (image, duration) elements played once.
fn once(key: &str, frames: &[(i32, i32)]) -> Arc<Animation> {
    Arc::new(Animation::once(key, frames))
}

/// A looping animation.
fn looping(key: &str, frames: &[(i32, i32)]) -> Arc<Animation> {
    Arc::new(Animation::looping(key, frames))
}

/// The officer's animations, plus the human-state ones these tests give
/// it.
fn anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_OFFICER_{s}");
    let mut v = vec![
        once(&k("STRIKE"), &[(200, 400), (201, 0)]),
        once(&k("HITLOW"), &[(600, 300), (201, 0)]),
        once(&k("HITHIGH"), &[(600, 300), (201, 0)]),
        once(&k("HOME"), &[(201, 0)]),
        looping(&k("ADVANCE"), &[(3, 100), (4, 100)]),
        looping(&k("FASTADVANCE"), &[(1, 100), (2, 100)]),
        looping(&k("JUMP"), &[(5, 100)]),
        once(&k("JUMPBACK"), &[(9, 1300), (9, 0)]),
        once(&k("BLOCKLOW"), &[(10, 100), (11, 500), (11, 0)]),
        once(&k("BLOCKHIGH"), &[(12, 100), (13, 500), (13, 0)]),
        looping(&k("IDLE"), &[(14, 3000), (15, 100)]),
    ];
    for key in enemy::officer::STAND {
        v.push(once(key, &[(100, 0)]));
    }
    v
}

/// The human-state animations and sounds the officer gets in these tests.
fn human_parts() -> human::HumanData {
    human::HumanData {
        anims: human::HumanAnims {
            jump_back: "LEVEL_OFFICER_JUMPBACK",
            block_high: "LEVEL_OFFICER_BLOCKHIGH",
            block_low: "LEVEL_OFFICER_BLOCKLOW",
            idle: "LEVEL_OFFICER_IDLE",
        },
        sounds: human::HumanSounds {
            jump: "TEST_JUMP",
            block_high: "TEST_BLOCKHIGH",
            block_low: "TEST_BLOCKLOW",
            idle_voice: "TEST_IDLE_LINE",
            strike: "",
        },
        ..human::HumanData::default()
    }
}

/// `rows` of 64 px tiles with the officer's animations and the given
/// records; every object awake from the start.
fn sim_of(rows: &[String], objects: Vec<ObjectSpec>, seed: u32) -> Sim {
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in [("TEST_IDLE_LINE", 700), ("LEVEL_OFFICER_EHEM1", 600)] {
        level.sounds.insert(k.to_string(), Some(ms));
    }
    level.objects = objects;
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

/// The flat floor of the common helpers: feet rest on 959.
fn flat_rows() -> Vec<String> {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    rows
}

fn officer_at(x: i32, min: i32, max: i32) -> ObjectSpec {
    let mut o = ObjectSpec::at("Officer", x, 900, 4000);
    o.image_set = "LEVEL_OFFICER".into();
    o.record[enemy::field::X_MIN] = min;
    o.record[enemy::field::X_MAX] = max;
    o
}

/// An officer at x = 3000 (patrol limits `min`, `max`) on `rows`, with
/// Claw at `claw_x` and the human-state parts; built, idle, and settled.
fn officer_on(rows: &[String], min: i32, max: i32, claw_x: i32) -> (Sim, ObjectId) {
    let mut sim = sim_of(rows, vec![officer_at(3000, min, max)], 21);
    sim.teleport_player(claw_x, 899);
    idle(&mut sim, 3);
    let id = find(&sim, GameLogic::Officer);
    sim.with_world(|w| {
        w.claw.stats.health = 10_000;
        enemy::data_mut(w, id).unwrap().human = human_parts();
    });
    (sim, id)
}

/// The officer on the flat floor, with Claw 250 px to its right (out of
/// range, on screen).
fn officer() -> (Sim, ObjectId) {
    officer_on(&flat_rows(), 0, 0, 3250)
}

fn find(sim: &Sim, g: GameLogic) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(g))
        .map(|(id, _)| id)
        .expect("the enemy exists")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn data(sim: &Sim, id: ObjectId) -> enemy::EnemyData {
    enemy::data_of(obj(sim, id)).expect("built").clone()
}

fn safe(sim: &Sim, id: ObjectId) -> bool {
    obj(sim, id).flags & flags::SAFE != 0
}

fn cues(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// Steps until `id` stands again (at most `max` ticks), then once more so
/// that its state sees the landing; returns the ticks until it stood and
/// the highest point reached.
fn until_landed(sim: &mut Sim, id: ObjectId, max: usize) -> (usize, i32) {
    let mut top = obj(sim, id).y;
    for n in 1..=max {
        sim.step(TickInput::default());
        top = top.min(obj(sim, id).y);
        if obj(sim, id).physics_type == ptype::STAND {
            sim.step(TickInput::default());
            return (n, top);
        }
    }
    panic!("still in the air after {max} ticks");
}

/// Runs `f` in the world and returns the draws it made.
fn draws_of(sim: &mut Sim, f: impl FnOnce(&mut claw_sim::logic::World)) -> u32 {
    let before = sim.rng().state();
    sim.with_world(f);
    let mut r = Rng::new(before);
    for n in 0..=64 {
        if r.state() == sim.rng().state() {
            return n;
        }
        r.next_u15();
    }
    panic!("more than 64 draws");
}

// ----------------------------------------------------------------------
// The hop up

#[test]
fn the_hop_up_rises_90_px_lands_after_600_ms_and_is_safe_meanwhile() {
    let (mut sim, id) = officer();
    let y0 = obj(&sim, id).y;
    let rng = sim.rng_state();
    sim.with_world(|w| human::enter_jump(w, id));
    assert_eq!(sim.rng_state(), rng, "no draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Jump));
    assert_eq!(obj(&sim, id).physics_type, ptype::FLY);
    assert!(safe(&sim, id));
    let d = data(&sim, id);
    assert!(d.busy);
    assert_eq!((d.vx, d.vy), (0.0, -600.0));
    let (ticks, top) = until_landed(&mut sim, id, 60);
    assert!((89..=90).contains(&(y0 - top)), "rises {}", y0 - top);
    assert!((33..=35).contains(&ticks), "lands after {ticks} ticks");
    assert_eq!(obj(&sim, id).y, y0, "back on the floor");
    assert!(!safe(&sim, id), "hits allowed again");
    assert!(!data(&sim, id).busy);
    // A walking enemy goes Home on landing, which its class then plays.
    assert_ne!(state_of(&sim, id), Some(EnemyState::Jump));
}

#[test]
fn only_a_standing_enemy_hops_up() {
    let (mut sim, id) = officer();
    sim.with_world(|w| w.objects.get_mut(id).unwrap().physics_type = ptype::FLY);
    let before = obj(&sim, id).clone();
    sim.with_world(|w| human::enter_jump(w, id));
    assert_eq!(obj(&sim, id).state, before.state);
    assert_eq!(obj(&sim, id).flags, before.flags);
}

#[test]
fn a_springboard_hit_hops_up_first_and_stays_safe_until_it_lands() {
    let (mut sim, id) = officer();
    let mut spring = Object::new(0, 3300, 900, 0, "TestSpring".into(), LogicKind::Inert);
    spring.type_flags = enemy::attacker::SPRING;
    spring.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let spring = sim.spawn(spring);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        claw_sim::logic::on_hit(w, id, spring);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Jump));
    assert!(safe(&sim, id));
    let d = data(&sim, id);
    assert!(d.busy && d.vx > 0.0, "thrown toward the springboard");
    until_landed(&mut sim, id, 100);
    assert!(!safe(&sim, id));
}

// ----------------------------------------------------------------------
// The hop back

#[test]
fn the_hop_back_lands_100_px_away_from_claw_at_the_same_height() {
    let (mut sim, id) = officer();
    let (x0, y0) = (obj(&sim, id).x, obj(&sim, id).y);
    let rng = sim.rng_state();
    sim.with_world(|w| human::enter_jump_back(w, id));
    assert_eq!(sim.rng_state(), rng, "no draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpBack));
    assert!(safe(&sim, id) && data(&sim, id).busy);
    assert!(enemy::faces_right(obj(&sim, id)), "faces Claw, flying back");
    assert_eq!(obj(&sim, id).anim.key(), Some("LEVEL_OFFICER_JUMPBACK"));
    let d = data(&sim, id);
    assert!((d.vx + 316.227).abs() < 0.01, "{}", d.vx);
    assert!((d.vy + 316.227).abs() < 0.01, "{}", d.vy);
    let (ticks, _) = until_landed(&mut sim, id, 40);
    assert!((17..=19).contains(&ticks), "{ticks} ticks in the air");
    let o = obj(&sim, id);
    assert!((o.x - (x0 - 100)).abs() <= 3, "lands at {}", o.x);
    assert_eq!(o.y, y0);
    assert!(!safe(&sim, id) && !data(&sim, id).busy);
}

#[test]
fn the_hop_back_needs_room_inside_the_limits_and_a_floor() {
    // The spot is past the left patrol limit.
    let (mut sim, id) = officer_on(&flat_rows(), 2950, 3200, 3250);
    let before = (obj(&sim, id).state, obj(&sim, id).flags, data(&sim, id));
    sim.with_world(|w| human::enter_jump_back(w, id));
    assert_eq!(
        (obj(&sim, id).state, obj(&sim, id).flags, data(&sim, id)),
        before
    );
    // A hole under the spot (x = 2900 is in column 45).
    let mut rows = flat_rows();
    rows[15].replace_range(45..46, ".");
    rows[16].replace_range(45..46, ".");
    let (mut sim, id) = officer_on(&rows, 0, 0, 3250);
    let state = obj(&sim, id).state;
    sim.with_world(|w| human::enter_jump_back(w, id));
    assert_eq!(obj(&sim, id).state, state);
    assert!(!safe(&sim, id));
}

#[test]
fn a_scheduled_hop_back_runs_in_the_post_tick() {
    let (mut sim, id) = officer();
    let y0 = obj(&sim, id).y;
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().deferred = Some(Deferred::Human(HumanCall::JumpBack));
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpBack));
    assert!(obj(&sim, id).y < y0, "already moving up");
}

// ----------------------------------------------------------------------
// Moving back and moving ahead

#[test]
fn moving_back_draws_first_and_walks_75_to_125_px_away() {
    let (mut sim, id) = officer();
    let x0 = obj(&sim, id).x;
    let mut m = Rng::new(sim.rng_state());
    let r = m.below(51) as i32;
    sim.with_world(|w| human::enter_move_back(w, id));
    assert_eq!(sim.rng_state(), m.state(), "one draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveBack));
    let d = data(&sim, id);
    assert_eq!(d.human.target_x, x0 - r - 75);
    assert_eq!(d.vx, -150.0);
    assert!(!d.busy, "free as it was");
    assert!(enemy::faces_right(obj(&sim, id)), "facing Claw");
    assert_eq!(obj(&sim, id).anim.key(), Some("LEVEL_OFFICER_ADVANCE"));
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::MoveBack) {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 60);
    }
    let x = obj(&sim, id).x;
    assert!(x <= x0 - r - 75 && x > x0 - r - 75 - 3, "stops at {x}");
}

#[test]
fn moving_back_that_cannot_be_made_still_draws() {
    let (mut sim, id) = officer_on(&flat_rows(), 2950, 3200, 3250);
    let state = obj(&sim, id).state;
    let mut m = Rng::new(sim.rng_state());
    m.below(51);
    sim.with_world(|w| human::enter_move_back(w, id));
    assert_eq!(sim.rng_state(), m.state());
    assert_eq!(obj(&sim, id).state, state);
    assert_eq!(data(&sim, id).vx, 0.0);
}

#[test]
fn moving_ahead_stops_40_to_80_px_short_of_claw() {
    let (mut sim, id) = officer_on(&flat_rows(), 0, 0, 3200);
    let mut m = Rng::new(sim.rng_state());
    let r = m.below(41) as i32;
    sim.with_world(|w| human::enter_move_ahead(w, id));
    assert_eq!(sim.rng_state(), m.state(), "one draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveAhead));
    let target = 3200 - r - 40;
    assert_eq!(data(&sim, id).human.target_x, target);
    assert_eq!(data(&sim, id).vx, 150.0);
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::MoveAhead) {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 60);
    }
    let x = obj(&sim, id).x;
    assert!(x >= target && x < target + 3, "stops at {x}");
    // Short of Claw on its own side; the limit on his side bars it.
    let (mut sim, id) = officer_on(&flat_rows(), 2900, 3100, 3200);
    let state = obj(&sim, id).state;
    sim.with_world(|w| human::enter_move_ahead(w, id));
    assert_eq!(obj(&sim, id).state, state, "the spot is past 3100");
}

/// Raux on `rows` at x = 3000 (limits 2700 to 3300), Claw far away; the
/// fight on, so he idles and decides.
fn raux_on(rows: &[String]) -> (Sim, ObjectId) {
    let mut o = ObjectSpec::at("Raux", 3000, 900, 0);
    o.image_set = "LEVEL_RAUX".into();
    o.record[enemy::field::X_MIN] = 2700;
    o.record[enemy::field::X_MAX] = 3300;
    let mut sim = sim_of(rows, vec![o], 5);
    sim.teleport_player(9000, 899);
    idle(&mut sim, 3);
    let id = find(&sim, GameLogic::Raux);
    (sim, id)
}

#[test]
fn moving_ends_at_a_wall() {
    // A wall one tile high stands on the floor at column 45
    // (x 2880 to 2943); the spot is beyond it.
    let mut rows = flat_rows();
    rows[14].replace_range(45..46, "#");
    let (mut sim, id) = raux_on(&rows);
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.vx = -150.0;
        d.human.target_x = 2860;
        enemy::set_state(w, id, EnemyState::MoveBack);
    });
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::MoveBack) {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 60, "still walking at {}", obj(&sim, id).x);
    }
    assert!(obj(&sim, id).x > 2943, "stopped by the wall");
}

#[test]
fn moving_ends_when_the_enemy_did_not_move() {
    let (mut sim, id) = raux_on(&flat_rows());
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.vx = 0.0;
        d.human.target_x = 2000;
        enemy::set_state(w, id, EnemyState::MoveBack);
    });
    // Standing still at 0 px/s it is not "moving left": the spot is behind
    // it at once.
    sim.step(TickInput::default());
    assert_ne!(state_of(&sim, id), Some(EnemyState::MoveBack));
    // Walking left towards a far spot, held in place by its own limit.
    let (mut sim, id) = raux_on(&flat_rows());
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.vx = -150.0;
        d.bounds.0 = 3000;
        d.human.target_x = 2000;
        // As if it had just moved here.
        d.human.pre_move_x = 3003;
        enemy::set_state(w, id, EnemyState::MoveBack);
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveBack));
    assert_eq!(data(&sim, id).human.pre_move_x, 3000);
    sim.step(TickInput::default());
    assert_ne!(state_of(&sim, id), Some(EnemyState::MoveBack));
}

// ----------------------------------------------------------------------
// Blocks

#[test]
fn a_block_slides_away_slows_to_zero_and_ends_with_its_animation() {
    let (mut sim, id) = officer();
    let rng = sim.rng_state();
    sim.with_world(|w| human::enter_block(w, id, false));
    assert_eq!(sim.rng_state(), rng, "no draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
    assert!(safe(&sim, id) && data(&sim, id).busy);
    assert_eq!(data(&sim, id).vx, -200.0, "away from Claw");
    assert_eq!(obj(&sim, id).anim.key(), Some("LEVEL_OFFICER_BLOCKLOW"));
    sim.step(TickInput::default());
    let mut speeds = vec![data(&sim, id).vx];
    let mut n = 1;
    while state_of(&sim, id) == Some(EnemyState::BlockLow) {
        sim.step(TickInput::default());
        speeds.push(data(&sim, id).vx);
        n += 1;
        assert!(n < 60);
    }
    assert_eq!(
        &speeds[..9],
        &[
            -175.0, -150.0, -125.0, -100.0, -75.0, -50.0, -25.0, 0.0, 0.0
        ]
    );
    // 600 ms of animation.
    assert!((33..=35).contains(&n), "{n} ticks");
    assert!(!safe(&sim, id) && !data(&sim, id).busy);
}

#[test]
fn a_block_plays_its_sound_and_the_high_one_its_own() {
    let (mut sim, id) = officer();
    sim.with_world(|w| human::enter_block(w, id, true));
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockHigh));
    assert_eq!(obj(&sim, id).anim.key(), Some("LEVEL_OFFICER_BLOCKHIGH"));
    sim.step(TickInput::default());
    assert!(cues(&sim).contains(&"TEST_BLOCKHIGH".to_string()));
    let (mut sim, id) = officer();
    sim.with_world(|w| human::enter_block(w, id, false));
    sim.step(TickInput::default());
    assert!(cues(&sim).contains(&"TEST_BLOCKLOW".to_string()));
}

#[test]
fn a_block_slides_faster_when_claw_crosses_over() {
    let (mut sim, id) = officer();
    sim.with_world(|w| human::enter_block(w, id, false));
    idle(&mut sim, 2);
    assert_eq!(data(&sim, id).vx, -150.0);
    // Claw jumps over to its left: the step now points the other way.
    sim.teleport_player(2800, 899);
    let mut speeds = Vec::new();
    for _ in 0..4 {
        sim.step(TickInput::default());
        speeds.push(data(&sim, id).vx);
    }
    assert_eq!(speeds, [-175.0, -200.0, -225.0, -250.0]);
}

#[test]
fn a_block_in_the_air_keeps_falling() {
    let (mut sim, id) = officer();
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.physics_type = ptype::FLY;
        o.y -= 50;
        enemy::data_mut(w, id).unwrap().vy = 120.0;
        human::enter_block(w, id, false);
    });
    assert_eq!(data(&sim, id).vy, 120.0, "the fall is kept");
    sim.step(TickInput::default());
    assert!(data(&sim, id).vy > 120.0);
}

#[test]
fn the_block_release_frees_on_a_later_scheduled_call_in_the_left_state() {
    // Raux runs his logic calls with the release.
    let (sim, id) = raux_on(&flat_rows());
    let arm = |sim: &mut Sim, left: EnemyState, call: Option<Deferred>| {
        sim.with_world(|w| {
            enemy::set_state(w, id, EnemyState::Pause);
            w.objects.get_mut(id).unwrap().flags |= flags::SAFE;
            let now = w.now_ms;
            let d = enemy::data_mut(w, id).unwrap();
            d.pause = enemy::Timer::new(now, 60_000);
            d.busy = true;
            d.human.release_in = Some(left);
            // A hop back that cannot be made: only the release acts.
            d.bounds = (2950, 3050);
            d.deferred = call;
        });
    };
    let hop = Some(Deferred::Human(HumanCall::JumpBack));
    // In the state the block left, with a scheduled call: freed and hits
    // allowed again, before the call; the release is used up.
    let mut s = sim.clone();
    arm(&mut s, EnemyState::Pause, hop);
    s.step(TickInput::default());
    assert_eq!(state_of(&s, id), Some(EnemyState::Pause));
    assert!(!data(&s, id).busy && !safe(&s, id));
    assert_eq!(data(&s, id).human.release_in, None);
    // In another state: nothing, but any scheduled call disarms it.
    let mut s = sim.clone();
    arm(&mut s, EnemyState::Strike, hop);
    s.step(TickInput::default());
    assert!(data(&s, id).busy && safe(&s, id));
    assert_eq!(data(&s, id).human.release_in, None);
    // Without a scheduled call it waits, armed.
    let mut s = sim.clone();
    arm(&mut s, EnemyState::Pause, None);
    s.step(TickInput::default());
    assert!(data(&s, id).busy && safe(&s, id));
    assert_eq!(data(&s, id).human.release_in, Some(EnemyState::Pause));
}

#[test]
fn a_block_arms_the_release_on_the_state_it_left() {
    let (mut sim, id) = officer();
    let left = state_of(&sim, id).unwrap();
    sim.with_world(|w| human::enter_block(w, id, false));
    assert_eq!(data(&sim, id).human.release_in, Some(left));
    assert_eq!(data(&sim, id).prev, left);
}

// ----------------------------------------------------------------------
// Human idle

#[test]
fn going_human_idle_faces_claw_maybe_speaks_and_waits_two_seconds() {
    let (mut sim, id) = officer();
    // Turned away from Claw first.
    sim.with_world(|w| enemy::set_facing(w, id, false));
    let mut m = Rng::new(sim.rng_state());
    let speaks = m.below(100) < 33;
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().vx = 80.0;
        human::go_idle(w, id);
    });
    assert_eq!(sim.rng_state(), m.state(), "one draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert!(enemy::faces_right(obj(&sim, id)));
    let d = data(&sim, id);
    assert_eq!(d.vx, 0.0);
    assert_eq!(d.pace_wait.len, 2000);
    assert_eq!(obj(&sim, id).anim.key(), Some("LEVEL_OFFICER_IDLE"));
    sim.step(TickInput::default());
    assert_eq!(
        cues(&sim).iter().any(|c| c == "TEST_IDLE_LINE"),
        speaks,
        "the line a third of the time"
    );
}

#[test]
fn the_human_idle_state_draws_every_tick_and_goes_home_in_range() {
    // A human class without its own idle handler idles with the shared
    // one, under the state named Idle (and HumanIdle).
    let idle: enemy::Handler = human::idle_tick;
    for s in [EnemyState::Idle, EnemyState::HumanIdle] {
        let h = states::shared_handler(s, true).expect("a human idle handler");
        assert_eq!(h as usize, idle as usize);
    }
    assert!(states::shared_handler(EnemyState::Idle, false).is_none());
    let (mut sim, id) = officer();
    // Out of range: one draw, no change before the wait is over.
    sim.with_world(|w| human::go_idle(w, id));
    let draws = draws_of(&mut sim, |w| human::idle_tick(w, id));
    assert_eq!(draws, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // In range (mode 0 to 4): Home.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().mode = 4);
    let draws = draws_of(&mut sim, |w| human::idle_tick(w, id));
    assert_eq!(draws, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn the_human_idle_state_paces_only_on_a_draw_of_one_after_its_wait() {
    let (mut sim, id) = officer();
    sim.with_world(|w| {
        human::go_idle(w, id);
        enemy::data_mut(w, id).unwrap().mode = 5;
    });
    // A stream whose next draw of 0..201 is 1.
    let one = (0..100_000u32)
        .find(|&s| Rng::new(s).below(201) == 1)
        .unwrap();
    let not_one = (0..100_000u32)
        .find(|&s| Rng::new(s).below(201) != 1)
        .unwrap();
    // Before the wait is over: never.
    let mut s = sim.clone();
    s.set_rng_state(one);
    s.with_world(|w| human::idle_tick(w, id));
    assert_eq!(state_of(&s, id), Some(EnemyState::Idle));
    // After it: on a 1 only.
    let mut s = sim.clone();
    s.with_world(|w| enemy::data_mut(w, id).unwrap().pace_wait = enemy::Timer::default());
    let mut t = s.clone();
    s.set_rng_state(not_one);
    s.with_world(|w| human::idle_tick(w, id));
    assert_eq!(state_of(&s, id), Some(EnemyState::Idle));
    t.set_rng_state(one);
    t.with_world(|w| human::idle_tick(w, id));
    assert_eq!(state_of(&t, id), Some(EnemyState::Pace));
}

// ----------------------------------------------------------------------
// The springboard launch

#[test]
fn the_springboard_launch_draws_a_side_and_lands_stopped() {
    let (mut sim, id) = officer();
    let y0 = obj(&sim, id).y;
    let mut m = Rng::new(sim.rng_state());
    let right = m.below(100) < 50;
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().human.springboard_armed = true;
        human::run_deferred(
            w,
            id,
            enemy::class(GameLogic::Officer).unwrap(),
            HumanCall::JumpOffSpringboard,
        );
    });
    assert_eq!(sim.rng_state(), m.state(), "one draw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::JumpOffSpringboard));
    let d = data(&sim, id);
    assert_eq!(d.vx, if right { 100.0 } else { -100.0 });
    assert!((d.vy + 1498.666).abs() < 0.01, "{}", d.vy);
    assert!(!d.human.springboard_armed);
    assert!(!d.busy, "it stays free");
    let o = obj(&sim, id);
    assert_eq!((o.y, o.physics_type), (y0 - 1, ptype::FLY));
    assert!(!safe(&sim, id));
    until_landed(&mut sim, id, 200);
    assert_eq!(data(&sim, id).vx, 0.0);
    assert_ne!(state_of(&sim, id), Some(EnemyState::JumpOffSpringboard));
}

// ----------------------------------------------------------------------
// Safe enemies are not hit

#[test]
fn claws_blow_does_not_reach_a_safe_enemy() {
    let (mut sim, id) = officer();
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 50);
    // A blow box over the enemy, from an object of Claw's kind, in the
    // collision pass.
    let mut blow = Object::new(0, 3000, 900, 0, "TestBlow".into(), LogicKind::Inert);
    blow.type_flags = types::PLAYER;
    blow.damage = 10;
    blow.flags = flags::WORLD;
    blow.attack_rect = Rect::new(-40, -60, 40, 60);
    sim.spawn(blow);
    let mut open = sim.clone();
    open.step(TickInput::default());
    assert_eq!(obj(&open, id).health, 40, "hit when not safe");
    sim.with_world(|w| human::enter_block(w, id, false));
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).health, 50, "safe while blocking");
}
