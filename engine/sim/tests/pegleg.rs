//! PegLeg on a synthetic floor with the ninth level's animation timings:
//! how he is built (Home, going by sight), his decisions by range, his
//! shot (the bullet on the 19th tick of the shot, 55 px ahead, 24 px up or
//! 15 px down at a ducking Claw, 500 px/s, 10 damage, the shot over on the
//! 31st tick, the gun cooling 1000 ms), his rolls (4.5 px a tick, the
//! forward one 35 px short of Claw on the ninth level and starting with a
//! step to the right, both stopping on a patrol bound), the hits he
//! ignores while rolling, the pistol shot that rolls him at Claw, the
//! blows that land half the time, the crouching stab that always lands
//! and opens the window, the lift in the middle of a roll, the throw that
//! never hurts him and the taunt and firing spell after it, his idle pose,
//! and the tenth level's pairs and full-length forward roll.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::pegleg::{self, PegLegData, bullet};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::kind as attack_kind;
use claw_sim::rng::Rng;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// An animation of (image, duration, event) elements; `once` makes the
/// last element finish it.
fn anim(key: &str, frames: &[(i32, i32, i32)], once: bool) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames.iter().map(|&(i, d, _)| (i, d)).collect();
    let mut a = if once {
        Animation::once(key, &pairs)
    } else {
        Animation::looping(key, &pairs)
    };
    for (f, &(_, _, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

/// A shot: the bullet leaves on its third picture.
const SHOT: &[(i32, i32, i32)] = &[(200, 100, 0), (201, 200, 0), (202, 100, 1), (203, 100, 0)];
/// A flinch.
const HIT: &[(i32, i32, i32)] = &[(400, 50, 0), (401, 200, 0), (400, 50, 0), (400, 0, 0)];
/// A roll, looping.
const ROLL: &[(i32, i32, i32)] = &[(850, 100, 0), (851, 100, 0), (852, 100, 0), (853, 100, 0)];

fn pegleg_anims() -> Vec<Arc<Animation>> {
    let a = pegleg::CLASS.anims;
    vec![
        anim(pegleg::HIGH_SHOT_ANIM, SHOT, true),
        anim(pegleg::LOW_SHOT_ANIM, SHOT, true),
        anim(pegleg::FORWARD_ROLL_ANIM, ROLL, false),
        anim(pegleg::BACKWARD_ROLL_ANIM, ROLL, false),
        anim(pegleg::IDLE_ANIM, &[(100, 400, 0), (101, 400, 0)], false),
        anim(a.hit_low, HIT, true),
        anim(a.hit_high, HIT, true),
        anim(a.home, &[(1, 0, 0)], true),
        anim(a.bounce, &[(1, 0, 0)], true),
        anim(a.lifted, &[(600, 80, 0), (601, 0, 0)], true),
        anim(a.lifted2, &[(602, 100, 0), (603, 100, 0)], false),
        anim(a.advance, &[(10, 100, 0), (11, 100, 0)], false),
        anim(a.fast_advance, &[(12, 100, 0), (13, 100, 0)], false),
        anim(a.fall, &[(900, 100, 0)], false),
    ]
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(anim(key, &[(200, 150, 0), (201, 200, 0)], true));
    }
    v.push(anim(
        claw_sim::player::special::LIFT_ANIM,
        &[(40, 100, 0), (42, 100, 0), (43, 100, 0)],
        true,
    ));
    v.push(anim(
        claw_sim::player::special::CARRY_ANIM,
        &[(43, 1000, 0)],
        false,
    ));
    v.push(anim(
        claw_sim::player::special::THROW_ANIM,
        &[(44, 100, 0), (47, 50, 1), (48, 100, 0)],
        true,
    ));
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250, 0)], false));
    v
}

/// Where the arena's PegLeg stands and his patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor (feet rest on 959) as level `number`, with a PegLeg at
/// `AT` between `LIMITS`; every object awake.
fn arena(number: u32) -> SimLevel {
    let mut level = flat_level();
    level.number = number;
    for a in pegleg_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("PegLeg", AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    // The record's own health does not count.
    o.record[18] = 40;
    level.objects = vec![o];
    level
}

/// The arena on level `number` with Claw put `dx` px from PegLeg on the
/// first tick, tough enough to take any number of shots.
fn with_claw(number: u32, seed: u32, dx: i32) -> (Sim, ObjectId) {
    let level = arena(number);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(AT + dx, 899);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::PegLeg))
        .map(|(id, _)| id)
        .expect("a PegLeg");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn pdata(sim: &Sim, id: ObjectId) -> PegLegData {
    match &obj(sim, id).data {
        ObjectData::PegLeg(d) => d.clone(),
        d => panic!("PegLeg data: {d:?}"),
    }
}

/// A random stream whose next draw of 0..`n` is `want`.
fn stream_drawing(n: u32, want: u32) -> Rng {
    (1..100_000)
        .map(Rng::new)
        .find(|r| r.clone().below(n) == want)
        .expect("a seed")
}

/// A bullet in the level: its id, position and speed across.
type Bullet = (ObjectId, (i32, i32), i32);

/// The bullets in the level.
fn bullets(sim: &Sim) -> Vec<Bullet> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == bullet::LOGIC)
        .map(|(id, o)| (id, (o.x, o.y), o.speed_x))
        .collect()
}

/// The bullets that appear over `seen` (each tick's bullets), with the
/// tick each first appears on.
fn new_bullets(seen: &[Vec<Bullet>]) -> Vec<(usize, Bullet)> {
    let mut out: Vec<(usize, Bullet)> = Vec::new();
    for (k, tick) in seen.iter().enumerate() {
        for b in tick {
            if !out.iter().any(|(_, o)| o.0 == b.0) {
                out.push((k, *b));
            }
        }
    }
    out
}

/// Puts PegLeg Home, alerted, and runs his Home step with the next draw
/// of 0..101 being `r`.
fn home_with(sim: &mut Sim, id: ObjectId, r: u32) {
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(d) = enemy::data_mut(w, id) {
            d.alerted = true;
        }
        *w.rng = stream_drawing(101, r);
        pegleg::home_tick(w, id);
    });
}

/// Puts PegLeg Home, standing, free and with his hit box.
fn at_home(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(d) = enemy::data_mut(w, id) {
            d.vx = 0.0;
            d.busy = false;
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.hit_rect = pegleg::HIT_RECT;
        }
    });
}

/// Hits PegLeg with a new attack of Claw's of `kind` doing `damage`, the
/// next draw of 0..101 being `roll`.
fn claw_blow(sim: &mut Sim, id: ObjectId, kind: i32, damage: i32, roll: u32) {
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        if let Some(c) = w.objects.get_mut(claw) {
            c.damage = damage;
        }
        w.claw.attack.kind = kind;
        w.claw.attack.uppercut = false;
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        if let Some(d) = enemy::data_mut(w, id) {
            d.cooldown = enemy::Timer::default();
        }
        *w.rng = stream_drawing(101, roll);
        pegleg::on_hit(w, id, claw);
    });
}

/// An attacker of type `kind` doing `damage`, at PegLeg.
fn attacker(sim: &mut Sim, kind: u32, damage: i32) -> ObjectId {
    sim.with_world(|w| {
        let mut o = logic::new_object("TestBlow", AT, 900, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = kind;
        o.damage = damage;
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    })
}

#[test]
fn the_synthetic_animations_keep_the_ninth_levels_timings() {
    let level = arena(9);
    let calls = |key: &str| {
        let mut p = claw_sim::anim::AnimPlayer::default();
        p.set(level.animations[key].clone());
        let mut events = Vec::new();
        let mut n = 0;
        while !p.is_done() && n < 1000 {
            n += 1;
            if p.advance_in(18, 0, false, None).event == 1 {
                events.push(n);
            }
        }
        (n, events.first().copied())
    };
    assert_eq!(calls(pegleg::HIGH_SHOT_ANIM), (31, Some(19)));
    assert_eq!(calls(pegleg::CLASS.anims.hit_high), (19, None));
}

#[test]
fn he_is_built_home_going_by_sight() {
    let (sim, id) = with_claw(9, 1, 400);
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (7, 10));
    assert_eq!(&*o.image_set, pegleg::IMAGE_SET);
    assert_eq!(o.hit_rect, pegleg::HIT_RECT);
    let p = pdata(&sim, id);
    assert!(p.enemy.sight, "going by sight");
    assert!(p.enemy.may_lift);
    assert_eq!(p.enemy.ranges, pegleg::RANGES);
    assert_eq!(p.enemy.bounds, LIMITS);
    // Built Home, not idle: his first decision comes on the next tick.
    let (mut sim, id) = with_claw(9, 1, 400);
    sim.with_world(|w| {
        let before = w.rng.clone();
        pegleg::go_home(w, id);
        let mut expect = before;
        expect.below(1001);
        assert_eq!(*w.rng, expect, "going Home draws one of 0..1001");
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).anim.key(), Some(pegleg::IDLE_ANIM));
}

#[test]
fn his_home_decisions_follow_the_range() {
    // 250 px (mode 1): below 50 he walks, otherwise he rolls at Claw.
    let (mut sim, id) = with_claw(9, 2, -250);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 49);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Advance));
    let (mut sim, id) = with_claw(9, 2, -250);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 50);
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollForward));
    // 180 px (mode 2): below 70 he fires, otherwise he rolls away.
    let (mut sim, id) = with_claw(9, 2, -180);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 69);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    let (mut sim, id) = with_claw(9, 2, -180);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 70);
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollBackward));
    // 120 px (mode 3): below 70 he rolls away, otherwise he stays.
    let (mut sim, id) = with_claw(9, 2, -120);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 69);
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollBackward));
    let (mut sim, id) = with_claw(9, 2, -120);
    sim.step(TickInput::default());
    home_with(&mut sim, id, 70);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

/// Starts a shot at once and returns PegLeg's states and the bullets
/// over `n` ticks of `input`.
fn shoot(
    sim: &mut Sim,
    id: ObjectId,
    n: usize,
    input: TickInput,
) -> (Vec<Option<EnemyState>>, Vec<Vec<Bullet>>) {
    sim.with_world(|w| pegleg::fire(w, id));
    (0..n)
        .map(|_| {
            sim.step(input);
            (state_of(sim, id), bullets(sim))
        })
        .unzip()
}

#[test]
fn his_bullet_leaves_on_the_19th_tick_of_the_shot_high_or_low() {
    for (input, dy) in [(TickInput::default(), -24), (hold(DOWN), 15)] {
        let (mut sim, id) = with_claw(9, 3, -180);
        // Claw crouches first when he holds Down.
        for _ in 0..3 {
            sim.step(input);
        }
        let ducking = sim.player().unwrap().physics_type == ptype::DUCK;
        assert_eq!(ducking, dy > 0);
        sim.with_world(|w| {
            if let Some(d) = enemy::data_mut(w, id) {
                d.alerted = true;
            }
        });
        let (x, y) = (obj(&sim, id).x, obj(&sim, id).y);
        let (states, seen) = shoot(&mut sim, id, 40, input);
        let shots = new_bullets(&seen);
        assert_eq!(shots.len(), 1);
        let (first, (b, (bx, by), vx)) = shots[0];
        assert_eq!(first + 1, 19, "the bullet on S + 19");
        let len = states
            .iter()
            .take_while(|s| **s == Some(EnemyState::Fire))
            .count();
        assert_eq!(len + 1, 31, "the shot over on S + 31");
        // Facing Claw on his left, the bullet left 55 px ahead, at 500
        // px/s: 9 px on its next tick.
        assert_eq!(vx, -bullet::SPEED);
        assert_eq!((bx, by), (x - 55, y + dy));
        let next = seen[first + 1].iter().find(|o| o.0 == b).unwrap().1;
        assert_eq!(next.0 - bx, -9);
    }
}

#[test]
fn his_gun_cools_for_1000_ms_after_a_bullet() {
    let (mut sim, id) = with_claw(9, 3, -180);
    let (states, _) = shoot(&mut sim, id, 31, TickInput::default());
    assert_eq!(states.last().copied().flatten(), Some(EnemyState::Home));
    // Fired again at once: only Home.
    sim.with_world(|w| pegleg::fire(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    let cooldown = pdata(&sim, id).fire_cooldown;
    assert_eq!(cooldown.len, 1000);
}

#[test]
fn a_forward_roll_starts_right_and_stops_35_px_short_of_claw() {
    // Claw 100 px to his left: the roll aims 35 px short of him.
    let (mut sim, id) = with_claw(9, 4, -100);
    at_home(&mut sim, id);
    let x0 = sim.with_world(|w| enemy::exact_x(w, id));
    let cx = sim.player().unwrap().x;
    // Started from Home, the movement step of that same tick moves him
    // right, whichever side Claw is on.
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        pegleg::roll_forward(w, id);
        enemy::motion::integrate(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollForward));
    assert_eq!(pdata(&sim, id).target, cx + 35);
    assert!(pdata(&sim, id).enemy.busy);
    // No hit box for hits while he rolls.
    assert_eq!(obj(&sim, id).hit_rect, pegleg::ROLLING_HIT_RECT);
    let x1 = sim.with_world(|w| enemy::exact_x(w, id));
    assert!((x1 - x0 - 4.5).abs() < 1e-9, "{x0} {x1}");
    let mut xs = vec![obj(&sim, id).x];
    while state_of(&sim, id) == Some(EnemyState::RollForward) {
        sim.step(TickInput::default());
        xs.push(obj(&sim, id).x);
    }
    // Then 4.5 px a tick to the left, and he fires 35 px short of Claw
    // with his hit box back.
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    let stop = *xs.last().unwrap();
    assert!(stop <= cx + 35, "{stop}");
    assert!(stop > cx + 35 - 5, "{stop}");
    assert_eq!(obj(&sim, id).hit_rect, pegleg::HIT_RECT);
}

#[test]
fn on_the_tenth_level_a_forward_roll_aims_its_full_length() {
    // Claw 100 px to his left: the roll aims 150 px on, not 35 px short of
    // him, so it carries on past where the ninth level's stops and reaches
    // Claw's place.
    let (mut sim, id) = with_claw(10, 4, -100);
    at_home(&mut sim, id);
    let x0 = obj(&sim, id).x;
    let cx = sim.player().unwrap().x;
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        pegleg::roll_forward(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollForward));
    assert_eq!(pdata(&sim, id).target, x0 - 150);
    let mut xs = Vec::new();
    while state_of(&sim, id) == Some(EnemyState::RollForward) {
        sim.step(TickInput::default());
        xs.push(obj(&sim, id).x);
        assert!(xs.len() < 100, "the roll ends");
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    assert!(xs.iter().any(|&x| x < cx + 35), "{xs:?}");
    assert!(*xs.last().unwrap() <= cx, "{xs:?}");
}

#[test]
fn going_idle_he_shows_his_idle_pose() {
    // Claw far out of his range and sight.
    let (mut sim, id) = with_claw(10, 4, -1500);
    sim.with_world(|w| (pegleg::CLASS.go_idle)(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!(obj(&sim, id).anim.key(), Some(pegleg::IDLE_ANIM));
    // Idle, the pose plays on: its second picture after 400 ms.
    let mut frames = Vec::new();
    for _ in 0..30 {
        sim.step(TickInput::default());
        if state_of(&sim, id) != Some(EnemyState::Idle) {
            break;
        }
        frames.push(obj(&sim, id).frame);
    }
    assert!(frames.contains(&100) && frames.contains(&101), "{frames:?}");
}

#[test]
fn a_backward_roll_covers_150_px_in_34_ticks_or_stops_on_his_bound() {
    let (mut sim, id) = with_claw(9, 4, -120);
    let x0 = obj(&sim, id).x;
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        pegleg::roll_backward(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollBackward));
    assert_eq!(pdata(&sim, id).target, x0 + 150);
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::RollBackward) {
        sim.step(TickInput::default());
        n += 1;
    }
    assert_eq!(
        n,
        pegleg::ROLL_TICKS as usize + 1,
        "34 ticks, firing on the next"
    );
    assert_eq!(obj(&sim, id).x, x0 + 153);
    // Rolling back towards his right bound, 100 px behind him: he stops
    // exactly on it.
    let (mut sim, id) = with_claw(9, 4, -120);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.x = LIMITS.1 - 100;
        }
    });
    sim.teleport_player(LIMITS.1 - 220, 899);
    sim.step(TickInput::default());
    sim.with_world(|w| pegleg::roll_backward(w, id));
    while state_of(&sim, id) == Some(EnemyState::RollBackward) {
        sim.step(TickInput::default());
    }
    assert_eq!(obj(&sim, id).x, LIMITS.1);
    // On that bound a backward roll fires at once.
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(ObjectData::PegLeg(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            p.fire_cooldown = enemy::Timer::default();
        }
        pegleg::roll_backward(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
}

#[test]
fn nothing_hurts_him_while_he_rolls() {
    let (mut sim, id) = with_claw(9, 4, -120);
    sim.with_world(|w| pegleg::roll_backward(w, id));
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, 0);
    assert_eq!(obj(&sim, id).health, 7);
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollBackward));
    // Not even counted: the same attack lands once he stops.
    let serial = sim.with_world(|w| w.claw.attack.serial);
    assert_ne!(pdata(&sim, id).enemy.serial_seen, Some(serial));
}

#[test]
fn a_pistol_shot_rolls_him_at_claw_unhurt_and_flies_on() {
    let (mut sim, id) = with_claw(9, 4, -250);
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        pegleg::on_hit(w, id, shot);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::RollForward));
    assert_eq!(obj(&sim, id).health, 7);
    assert!(obj(&sim, shot).flags & flags::DESTROY == 0);
}

#[test]
fn standing_blows_land_half_the_time_and_always_inside_the_window() {
    // A roll of 50: nothing at all.
    let (mut sim, id) = with_claw(9, 5, 400);
    at_home(&mut sim, id);
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, 50);
    assert_eq!(obj(&sim, id).health, 7);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    // A roll of 49: the high flinch and 5 off.
    let (mut sim, id) = with_claw(9, 5, 400);
    at_home(&mut sim, id);
    claw_blow(&mut sim, id, attack_kind::PUNCH, 5, 49);
    assert_eq!(obj(&sim, id).health, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitHigh));
    // The crouching stab lands (low flinch, 2 off) and opens the window,
    // inside which a standing stab lands whatever its roll and kills him.
    let (mut sim, id) = with_claw(9, 5, 400);
    at_home(&mut sim, id);
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, 99);
    assert_eq!(obj(&sim, id).health, 5);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(pdata(&sim, id).crouch_window.len, 2000);
    // A second crouching stab inside the window does nothing.
    at_home(&mut sim, id);
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, 99);
    assert_eq!(obj(&sim, id).health, 5);
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, 99);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    // In the window he cannot roll: he goes Home instead.
    let (mut sim, id) = with_claw(9, 5, 400);
    at_home(&mut sim, id);
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, 99);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    sim.with_world(|w| pegleg::roll_backward(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn a_blow_meeting_him_in_a_flinch_only_sends_him_home() {
    let (mut sim, id) = with_claw(9, 5, 400);
    sim.with_world(|w| enemy::set_state(w, id, EnemyState::HitLow));
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, 0);
    assert_eq!(obj(&sim, id).health, 7);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn lifted_he_says_one_of_his_four_lines_from_the_lifts_own_draw() {
    let (mut sim, id) = with_claw(9, 2, 400);
    sim.with_world(|w| pegleg::roll_forward(w, id));
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| pegleg::pick_lift_voice(w, id));
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
    // His hit box is back.
    assert_eq!(obj(&sim, id).hit_rect, pegleg::HIT_RECT);
    // The whole lift draws once, even out of a roll.
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        logic::lifted(w, id, claw);
    });
    let mut expect = before;
    expect.below(100);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
}

#[test]
fn thrown_he_lands_unhurt_then_taunts_and_fires_a_spell_of_six() {
    let (mut sim, id) = with_claw(9, 2, -160);
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        logic::lifted(w, id, claw);
        enemy::set_state(w, id, EnemyState::Flailing);
        logic::thrown(w, id, 0, 0);
    });
    let mut seen = Vec::new();
    for _ in 0..400 {
        sim.step(TickInput::default());
        seen.push((state_of(&sim, id), bullets(&sim), pdata(&sim, id).taunt));
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    // Never hurt by the landings.
    assert_eq!(obj(&sim, id).health, 7);
    // The landing set the taunt, which the first Home step in sight
    // spends: a spell of six shots, 31 ticks apart, alternating high and
    // low.
    assert!(seen.iter().any(|s| s.2), "the taunt was set");
    let fire = seen
        .iter()
        .position(|s| s.0 == Some(EnemyState::Fire))
        .expect("he fires");
    let spell = seen[fire..]
        .iter()
        .take_while(|s| s.0 == Some(EnemyState::Fire))
        .count();
    assert_eq!(spell, 6 * 31);
    let ticks: Vec<Vec<Bullet>> = seen[fire..fire + spell]
        .iter()
        .map(|s| s.1.clone())
        .collect();
    let shots: Vec<usize> = new_bullets(&ticks)
        .into_iter()
        .filter(|(_, b)| !seen[fire].1.iter().any(|o| o.0 == b.0))
        .map(|(k, _)| fire + k)
        .collect();
    assert_eq!(shots.len(), 6, "{shots:?}");
    assert!(shots.windows(2).all(|p| p[1] - p[0] == 31), "{shots:?}");
    assert!(!seen[fire].2, "the taunt is spent");
}

#[test]
fn on_the_tenth_level_he_fires_pairs_alternating_high_and_low() {
    let (mut sim, id) = with_claw(10, 3, -180);
    let (states, seen) = shoot(&mut sim, id, 70, TickInput::default());
    let fire = states
        .iter()
        .take_while(|s| **s == Some(EnemyState::Fire))
        .count();
    assert_eq!(fire + 1, 62, "two shots, Home on S + 62");
    let shots = new_bullets(&seen);
    assert_eq!(shots.len(), 2);
    assert_eq!(shots[1].0 - shots[0].0, 31);
    let y = obj(&sim, id).y;
    // One high, one low, in either order (his alternation goes on from
    // his earlier shots).
    let mut heights: Vec<i32> = shots.iter().map(|(_, b)| b.1.1 - y).collect();
    heights.sort();
    assert_eq!(heights, [-24, 15]);
    assert!(!pegleg::aims_at_posture(10));
}
