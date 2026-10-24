//! Mercat on a synthetic floor with the eleventh level's animation
//! timings: how he is built (idle, going by sight, the eleventh level's
//! health), going Home and its wait, his decisions (his own charge in
//! modes 0 to 2 only on a roll below 20, the throw at a Claw in
//! sight at any distance, the stab chosen within 150 px and replaced by the
//! throw on the same tick with the busy flag left set, the stab out of
//! sight, pacing out of range with the hop chance back at 90), the throw
//! (the bolt on its tenth tick from 28 px in front of him at 400 px/s, 20
//! damage, his cooldown of 1000 ms on the eleventh level and 750 on the
//! twelfth), the bolt (7 or 8 px a tick, two sparkles a tick, the burst on
//! Claw into a 34-tick explosion that hurts the human enemies in it, him
//! among them, the end at a wall, gone after Claw's respawn), the hop back
//! from an attack facing him (the chance 90, 60, 30, 0, the 500 ms wait,
//! no hop while busy), the parries (every crouching sword stab, a quarter
//! of pistol shots on the eleventh level, half on the twelfth), the cries
//! of the hits that land, his idle poses and the start of pacing, and the
//! lift that makes none of the shared lift's draws.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::mercat::{self, MercatData};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::projectile::{self, trident};
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

/// The throw and the stab: picture 203 (the stab's strike) on the tenth
/// call, done on the 19th.
const STRIKE: &[(i32, i32, i32)] = &[(201, 80, 0), (202, 70, 0), (203, 100, 1), (204, 50, 0)];
/// A flinch: done on the 19th call.
const HIT: &[(i32, i32, i32)] = &[(500, 50, 0), (501, 200, 0), (500, 50, 0), (200, 0, 0)];

fn mercat_anims() -> Vec<Arc<Animation>> {
    let a = mercat::ANIMS;
    let mut v = vec![
        anim(mercat::STRIKE_ANIM, STRIKE, true),
        anim(mercat::BLOCK_ANIM, &[(800, 50, 0)], true),
        anim(
            mercat::JUMP_ANIM,
            &[(800, 80, 0), (801, 200, 0), (802, 0, 0)],
            true,
        ),
        anim(mercat::HOME_ANIM, &[(200, 0, 0)], true),
        anim(a.hit_low, HIT, true),
        anim(a.hit_high, HIT, true),
        anim(a.advance, &[(1, 100, 0), (2, 100, 0)], false),
        anim(a.fast_advance, &[(1, 70, 0), (2, 70, 0)], false),
        anim(a.fall, &[(950, 0, 0)], true),
        anim(a.lifted, &[(601, 50, 0), (900, 0, 0)], true),
        anim(a.lifted2, &[(900, 100, 0), (901, 100, 0)], false),
        anim(a.bounce, &[(905, 0, 0)], true),
        // Eleven pictures of 50 ms: done on the 34th call.
        anim(
            trident::EXPLOSION_ANIM,
            &(1..=11).map(|i| (i, 50, 0)).collect::<Vec<_>>(),
            true,
        ),
    ];
    for (i, key) in mercat::IDLE_ANIMS.iter().enumerate() {
        v.push(anim(key, &[(100 + i as i32, 0, 0)], true));
    }
    v
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
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250, 0)], false));
    v
}

/// Where the arena's Mercat is placed and his patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);
/// His height on the floor: his feet (50 px below him) on 959.
const FLOOR_Y: i32 = 909;

/// The flat floor (feet rest on 959) as level `number`, with a Mercat at
/// `AT` between `LIMITS`; every object awake.
fn arena(number: u32) -> SimLevel {
    let mut level = flat_level();
    level.number = number;
    for a in mercat_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("Mercat", AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    // The record's own health does not count.
    o.record[18] = 40;
    level.objects = vec![o];
    level
}

/// The arena on level `number` with Claw put `dx` px from Mercat on the
/// first tick, tough enough to take any number of bolts.
fn with_claw(number: u32, seed: u32, dx: i32) -> (Sim, ObjectId) {
    with_claw_in(arena(number), seed, dx)
}

/// [`with_claw`] in `level`.
fn with_claw_in(level: SimLevel, seed: u32, dx: i32) -> (Sim, ObjectId) {
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
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Mercat))
        .map(|(id, _)| id)
        .expect("a Mercat");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn mdata(sim: &Sim, id: ObjectId) -> MercatData {
    match &obj(sim, id).data {
        ObjectData::Mercat(d) => d.clone(),
        d => panic!("Mercat data: {d:?}"),
    }
}

/// A random stream whose next draw of 0..`n` is `want`.
fn stream_drawing(n: u32, want: u32) -> Rng {
    (1..100_000)
        .map(Rng::new)
        .find(|r| r.clone().below(n) == want)
        .expect("a seed")
}

/// The bolts in the level: id, position, speed across, bursting.
fn bolts(sim: &Sim) -> Vec<(ObjectId, (i32, i32), i32, bool)> {
    sim.objects()
        .all()
        .filter(|(_, o)| o.is_listed() && &*o.logic == trident::LOGIC)
        .map(|(id, o)| (id, (o.x, o.y), o.speed_x, o.type_flags == types::LEVEL_SHOT))
        .collect()
}

/// True when `key` played on the last tick.
fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|a| matches!(a, AudioCue::Play(s) if s.key == key))
}

/// Puts Mercat Home with his wait over, free and stopped.
fn ready(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.wait = enemy::Timer::default();
            m.enemy.busy = false;
            m.enemy.vx = 0.0;
        }
    });
}

/// Hits Mercat with a new attack of Claw's of `kind` doing `damage`, the
/// next draw of 0..`n` being `roll`.
fn claw_blow(sim: &mut Sim, id: ObjectId, kind: i32, damage: i32, n: u32, roll: u32) {
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
        *w.rng = stream_drawing(n, roll);
        mercat::on_hit(w, id, claw);
    });
}

/// An attacker of type `kind` doing `damage`, at Mercat.
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
fn the_synthetic_animations_keep_the_eleventh_levels_timings() {
    let level = arena(11);
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
    assert_eq!(
        calls(mercat::STRIKE_ANIM),
        (19, Some(mercat::BOLT_TICK as usize))
    );
    assert_eq!(calls(mercat::BLOCK_ANIM).0, 4);
    assert_eq!(calls(mercat::JUMP_ANIM).0, 18);
    assert_eq!(calls(mercat::ANIMS.hit_low).0, 19);
    assert_eq!(
        calls(trident::EXPLOSION_ANIM).0,
        trident::EXPLOSION_TICKS as usize
    );
}

#[test]
fn he_is_built_idle_going_by_sight() {
    let (sim, id) = with_claw(11, 1, 400);
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (9, 10));
    assert_eq!(&*o.image_set, mercat::IMAGE_SET);
    assert_eq!(
        (o.hit_rect, o.move_rect),
        (mercat::HIT_RECT, mercat::MOVE_RECT)
    );
    assert_eq!(o.y, FLOOR_Y, "dropped to the floor");
    assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
    let m = mdata(&sim, id);
    assert!(m.enemy.sight, "going by sight");
    assert!(m.enemy.may_lift);
    assert_eq!(m.enemy.ranges, mercat::RANGES);
    assert_eq!(m.enemy.bounds, LIMITS);
    assert_eq!(m.enemy.smarts, mercat::SMARTS);
    assert_eq!(m.hop_chance, mercat::HOP_CHANCE);
    assert!(mercat::DEATH_SOUNDS.contains(&m.enemy.death_sound));
    // Elsewhere he has 15.
    let (sim, id) = with_claw(12, 1, 400);
    assert_eq!(obj(&sim, id).health, 15);
}

#[test]
fn going_home_he_waits_one_to_two_seconds() {
    let (mut sim, id) = with_claw(11, 2, 400);
    for r in [0, 1000] {
        sim.with_world(|w| {
            *w.rng = stream_drawing(1001, r);
            mercat::go_home(w, id);
        });
        assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
        assert_eq!(obj(&sim, id).anim.key(), Some(mercat::HOME_ANIM));
        assert_eq!(mdata(&sim, id).wait.len, 1000 + u64::from(r));
    }
    // Home with the wait running decides nothing.
    let before = sim.rng_state();
    sim.with_world(|w| mercat::home_tick(w, id));
    assert_eq!(sim.rng_state(), before);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn he_throws_at_a_claw_in_sight_at_any_distance() {
    for (dx, mode) in [(-233, 1), (-160, 2), (305, 0)] {
        let (mut sim, id) = with_claw(11, 3, dx);
        sim.step(TickInput::default());
        assert_eq!(mdata(&sim, id).enemy.mode, mode, "{dx}");
        ready(&mut sim, id);
        sim.with_world(|w| mercat::home_tick(w, id));
        assert_eq!(state_of(&sim, id), Some(EnemyState::Fire), "{dx}");
        let m = mdata(&sim, id);
        assert_eq!(m.enemy.vx, 0.0);
        let right = dx > 0;
        assert_eq!(enemy::faces_right(obj(&sim, id)), right);
        let side = if right { 1 } else { -1 };
        assert_eq!((m.enemy.shot.dx, m.enemy.shot.vx), (28 * side, 400 * side));
    }
    // Off screen he cannot see Claw: the shared Home runs at him.
    let (mut sim, id) = with_claw(11, 3, -450);
    sim.step(TickInput::default());
    ready(&mut sim, id);
    sim.with_world(|w| {
        *w.rng = stream_drawing(101, 50);
        mercat::home_tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::FastAdvance));
    assert_eq!(
        mdata(&sim, id).enemy.vx,
        -f64::from(mercat::FAST_ADVANCE_SPEED)
    );
}

#[test]
fn near_he_charges_by_himself_only_on_a_low_roll() {
    // Off screen he cannot see Claw. Below 20 he says an advance line and
    // charges by mode; on a higher roll he stays Home, says nothing, and
    // the shared Home draws again and sends him in by the same mode. Two
    // draws either way.
    let mut level = arena(11);
    for line in mercat::ADVANCE_LINES {
        level.sounds.insert(line.to_string(), Some(1000));
    }
    for (mode, charge, speed) in [
        (0, EnemyState::FastAdvance, mercat::FAST_ADVANCE_SPEED),
        (1, EnemyState::FastAdvance, mercat::FAST_ADVANCE_SPEED),
        (2, EnemyState::Advance, mercat::ADVANCE_SPEED),
    ] {
        for roll in [0, 19, 20, 36, 100] {
            let (mut sim, id) = with_claw_in(level.clone(), 3, -450);
            sim.step(TickInput::default());
            ready(&mut sim, id);
            let start = stream_drawing(101, roll);
            let mut want = start.clone();
            want.below(101);
            want.below(if roll < 20 { 3 } else { 421 });
            let (spoke, mid) = sim.with_world(|w| {
                if let Some(m) = mercat::mercat_data_mut(w, id) {
                    m.enemy.mode = mode;
                }
                *w.rng = start.clone();
                mercat::home_tick(w, id);
                (logic::sound::enemy_is_speaking(w, id), w.rng.state())
            });
            let at = format!("mode {mode} roll {roll}");
            assert_eq!(mid, want.state(), "{at}");
            assert_eq!(spoke, roll < 20, "{at}");
            assert_eq!(state_of(&sim, id), Some(charge), "{at}");
            assert_eq!(mdata(&sim, id).enemy.vx, -f64::from(speed), "{at}");
        }
    }
}

#[test]
fn in_sight_with_the_bolt_cooling_a_high_roll_keeps_him_home() {
    let (mut sim, id) = with_claw(11, 3, -160);
    sim.step(TickInput::default());
    assert_eq!(mdata(&sim, id).enemy.mode, 2);
    for (roll, after) in [(36, EnemyState::Home), (5, EnemyState::Advance)] {
        ready(&mut sim, id);
        sim.with_world(|w| {
            let now = w.now_ms;
            if let Some(m) = mercat::mercat_data_mut(w, id) {
                m.cooldown = enemy::Timer::new(now, 1000);
            }
            *w.rng = stream_drawing(101, roll);
            mercat::home_tick(w, id);
        });
        // The throw's entry stops him and, the bolt cooling, leaves him in
        // the state the roll chose.
        assert_eq!(state_of(&sim, id), Some(after), "roll {roll}");
        assert_eq!(mdata(&sim, id).enemy.vx, 0.0);
    }
}

#[test]
fn close_in_sight_the_stab_is_chosen_and_replaced_by_the_throw() {
    let (mut sim, id) = with_claw(11, 4, 60);
    sim.step(TickInput::default());
    assert_eq!(mdata(&sim, id).enemy.mode, 4);
    ready(&mut sim, id);
    sim.with_world(|w| mercat::home_tick(w, id));
    let m = mdata(&sim, id);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    assert!(m.enemy.busy, "the stab's busy flag stays set");
    assert_eq!(m.stab_wait.len, mercat::STAB_WAIT_MS);
    // The throw, its bolt and its go Home leave him busy.
    for _ in 0..12 {
        sim.step(TickInput::default());
    }
    assert!(mdata(&sim, id).enemy.busy);
}

#[test]
fn close_and_out_of_sight_he_stabs() {
    let (mut sim, id) = with_claw(11, 5, 100);
    sim.step(TickInput::default());
    // Claw turns invisible: no sight, the range mode stays as it was.
    sim.with_world(|w| {
        w.claw.stats.powerup_kind = enemy::INVISIBILITY_POWERUP;
        w.claw.stats.powerup_ms = 10_000;
    });
    ready(&mut sim, id);
    sim.with_world(|w| mercat::home_tick(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    assert!(mdata(&sim, id).enemy.busy);
    // The stab's own wait keeps the next one back for 100 ms: the shared
    // Home decides then, here (a draw of 99 or more) a pause.
    ready(&mut sim, id);
    sim.with_world(|w| {
        *w.rng = stream_drawing(421, 200);
        mercat::home_tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Pause));
}

#[test]
fn out_of_range_his_hop_chance_is_back_at_90_and_he_paces() {
    let (mut sim, id) = with_claw(11, 6, 2000);
    for _ in 0..3 {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| {
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.hop_chance = 0;
            m.enemy.mode = 6;
        }
    });
    ready(&mut sim, id);
    sim.with_world(|w| mercat::home_tick(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Pace));
    assert_eq!(mdata(&sim, id).hop_chance, 90);
}

#[test]
fn the_bolt_leaves_on_the_tenth_tick_of_the_throw() {
    for (level, cooldown) in [(11, 1000), (12, 750)] {
        let (mut sim, id) = with_claw(level, 7, 300);
        sim.step(TickInput::default());
        ready(&mut sim, id);
        sim.with_world(|w| mercat::home_tick(w, id));
        let mut first = None;
        for k in 1..=12 {
            sim.step(TickInput::default());
            if first.is_none() && !bolts(&sim).is_empty() {
                first = Some(k);
                assert!(played(&sim, mercat::BOLT_SOUND));
                assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
            }
        }
        assert_eq!(first, Some(mercat::BOLT_TICK as usize), "level {level}");
        assert_eq!(mdata(&sim, id).cooldown.len, cooldown);
        let b = bolts(&sim);
        assert_eq!(b.len(), 1, "one bolt per throw");
        // Thrown right from 28 px in front of him at his height.
        let o = sim.objects().get(b[0].0).unwrap();
        assert_eq!((o.damage, o.type_flags), (20, types::ENEMY_SHOT));
        assert_eq!(o.hit_type_flags, 0);
        assert_eq!(b[0].2, 400);
    }
}

#[test]
fn the_bolt_flies_7_or_8_px_a_tick_and_sheds_two_sparkles() {
    let (mut sim, id) = with_claw(11, 8, 300);
    sim.with_world(|w| {
        trident::launch(w, id, (AT + 28, FLOOR_Y, 4000), 400);
    });
    let b = bolts(&sim)[0].0;
    let x0 = obj(&sim, b).x;
    assert_eq!(x0, AT + 28);
    // Flying right its box turns to face right.
    assert_eq!(obj(&sim, b).attack_rect, trident::FRAME_BOX.mirrored());
    let mut xs = vec![x0];
    for _ in 0..10 {
        let before = sim.rng_state();
        sim.step(TickInput::default());
        xs.push(obj(&sim, b).x);
        // Two sparkles a tick: four draws (and the pulse's of Mercat on
        // every third tick, and his own).
        let mut r = Rng::new(before);
        let mut n = 0;
        while r.state() != sim.rng_state() && n < 50 {
            r.next_u15();
            n += 1;
        }
        assert!(n >= 4, "{n}");
    }
    let steps: Vec<i32> = xs.windows(2).map(|p| p[1] - p[0]).collect();
    assert!(steps.iter().all(|&s| s == 7 || s == 8), "{steps:?}");
    // A light flies with it, gone with the bolt.
    let d = projectile::data(obj(&sim, b)).cloned().unwrap();
    assert_eq!(d.sparkles, trident::SPARKLES);
    let flare = d.flare.expect("a light");
    assert!(sim.objects().get(flare).is_some());
}

#[test]
fn the_bolt_bursts_on_claw_and_its_explosion_hurts_mercat() {
    // Claw 60 px in front of him: the bolt meets him at once; its
    // explosion's box covers Mercat, who takes its 20.
    let (mut sim, id) = with_claw(11, 9, 60);
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.step(TickInput::default());
    ready(&mut sim, id);
    sim.with_world(|w| mercat::home_tick(w, id));
    let mut burst = None;
    for k in 1..=12 {
        sim.step(TickInput::default());
        if bolts(&sim).iter().any(|b| b.3) {
            burst = Some(k);
            break;
        }
    }
    assert_eq!(burst, Some(mercat::BOLT_TICK as usize));
    assert_eq!(sim.claw().stats.health, 80);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(obj(&sim, id).health, 9 - 20);
    // The explosion stays where it burst for 34 rows (the row it burst on
    // and its calls but the last), unhurtable; Claw, just hurt, takes
    // nothing more from it.
    let b = bolts(&sim)[0];
    assert!(obj(&sim, b.0).flags & flags::SAFE != 0);
    let mut rows = 0;
    while sim.objects().get(b.0).is_some_and(|o| o.is_listed()) && rows < 100 {
        let o = obj(&sim, b.0);
        assert_eq!((o.x, o.y), b.1);
        rows += 1;
        sim.step(TickInput::default());
    }
    assert_eq!(rows, trident::EXPLOSION_TICKS as usize);
    assert_eq!(sim.claw().stats.health, 80);
}

#[test]
fn a_wall_ends_the_bolt_without_a_burst() {
    let (mut sim, id) = with_claw(11, 10, 300);
    // Flying down into the floor's top row: it meets the floor.
    sim.with_world(|w| {
        trident::launch(w, id, (AT + 28, 950, 4000), -400);
    });
    let b = bolts(&sim)[0].0;
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(b)
            && let ObjectData::Projectile(d) = &mut o.data
        {
            d.motion.vy = 400.0;
        }
    });
    for _ in 0..10 {
        sim.step(TickInput::default());
        assert!(!bolts(&sim).iter().any(|b| b.3), "no burst");
    }
    assert!(bolts(&sim).is_empty(), "gone at the wall");
}

#[test]
fn claws_respawn_takes_the_bolt_away() {
    let (mut sim, id) = with_claw(11, 11, 300);
    sim.with_world(|w| {
        trident::launch(w, id, (AT - 28, FLOOR_Y, 4000), -400);
    });
    sim.step(TickInput::default());
    let b = bolts(&sim)[0].0;
    let removed = |sim: &Sim| obj(sim, b).flags & flags::DESTROY != 0;
    sim.with_world(|w| {
        // Claw has lost a life and respawns after this tick: the bolt
        // flies on.
        w.claw.stats.count_attempt();
        w.claw.respawn_pending = true;
        logic::tick(w, b);
        // He is back: one more call, then gone at the next.
        w.claw.respawn_pending = false;
        logic::tick(w, b);
    });
    assert!(!removed(&sim));
    sim.with_world(|w| logic::tick(w, b));
    assert!(removed(&sim));
}

#[test]
fn an_attack_facing_him_close_makes_him_hop_back_while_free() {
    let (mut sim, id) = with_claw(11, 12, -120);
    sim.step(TickInput::default());
    assert_eq!(mdata(&sim, id).enemy.mode, 3);
    let swing = |sim: &mut Sim, roll: u32| {
        sim.with_world(|w| {
            w.claw.attacking = true;
            w.claw.facing_right = true;
            *w.rng = stream_drawing(100, roll);
            mercat::pre_tick(w, id);
        })
    };
    // A draw at or above the chance changes nothing: drawn again next tick.
    swing(&mut sim, 90);
    let m = mdata(&sim, id);
    assert_eq!((m.hop_chance, m.enemy.deferred), (90, None));
    // Below it: the hop is scheduled, the chance goes 90 to 60 and the
    // wait starts.
    swing(&mut sim, 89);
    let m = mdata(&sim, id);
    assert_eq!(m.hop_chance, 60);
    assert_eq!(m.hop_wait.len, u64::from(mercat::HOP_WAIT_MS));
    assert!(m.enemy.deferred.is_some());
    // Within the wait no draw at all.
    let before = sim.rng_state();
    sim.with_world(|w| mercat::pre_tick(w, id));
    assert_eq!(sim.rng_state(), before);
    // The hop runs in the tick's post-tick: 18 ticks in the air, about 96
    // px back, free again.
    sim.with_world(|w| w.claw.attacking = false);
    let x0 = obj(&sim, id).x;
    let mut n = 0;
    loop {
        sim.step(TickInput::default());
        if state_of(&sim, id) != Some(EnemyState::JumpBack) || n > 60 {
            break;
        }
        n += 1;
    }
    assert_eq!(n, 18);
    assert!(
        (obj(&sim, id).x - x0 - 96).abs() <= 2,
        "{}",
        obj(&sim, id).x
    );
    assert!(!mdata(&sim, id).enemy.busy);
}

#[test]
fn busy_he_spends_his_hop_chance_without_hopping() {
    let (mut sim, id) = with_claw(11, 13, -120);
    sim.step(TickInput::default());
    sim.with_world(|w| {
        if let Some(d) = enemy::data_mut(w, id) {
            d.busy = true;
        }
        w.claw.attacking = true;
        w.claw.facing_right = true;
        let mut chances = Vec::new();
        for _ in 0..4 {
            if let Some(m) = mercat::mercat_data_mut(w, id) {
                m.hop_wait = enemy::Timer::default();
            }
            *w.rng = stream_drawing(100, 0);
            mercat::pre_tick(w, id);
            chances.push(mercat::mercat_data(w, id).unwrap().hop_chance);
        }
        assert_eq!(chances, [60, 30, 0, 0]);
        assert_eq!(enemy::data(w, id).unwrap().deferred, None);
    });
    // Claw facing away: no draw.
    sim.with_world(|w| {
        w.claw.facing_right = false;
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.hop_wait = enemy::Timer::default();
        }
        let before = w.rng.state();
        mercat::pre_tick(w, id);
        assert_eq!(w.rng.state(), before);
    });
}

#[test]
fn he_parries_every_crouching_sword_stab() {
    let (mut sim, id) = with_claw(11, 14, -80);
    sim.step(TickInput::default());
    let claw = sim.player().map(|c| c.x).unwrap();
    assert!(claw < AT);
    sim.with_world(|w| {
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().physics_type = ptype::DUCK;
    });
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 5, 2, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Block));
    let o = obj(&sim, id);
    assert_eq!(o.health, 9, "no damage");
    assert!(o.flags & flags::SAFE != 0);
    assert!(!enemy::faces_right(o), "facing Claw on his left");
    assert!(mdata(&sim, id).enemy.busy);
    // Four ticks of parry, then Home and free.
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::Block) && n < 10 {
        sim.step(TickInput::default());
        n += 1;
    }
    assert_eq!(n, 4);
    assert!(!mdata(&sim, id).enemy.busy);
    assert!(obj(&sim, id).flags & flags::SAFE == 0);
}

#[test]
fn a_standing_stab_lands_with_a_cry() {
    let (mut sim, id) = with_claw(11, 15, -80);
    sim.step(TickInput::default());
    // The cry's draw comes first: the stream moves one draw of 0..2 and
    // then the hit's own (the burst's two).
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, 2, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 4);
    assert_eq!(mdata(&sim, id).enemy.cooldown.len, 300);
    // A blow while he flinches only sends him Home.
    claw_blow(&mut sim, id, attack_kind::PUNCH, 5, 2, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, 4);
    // The next lands and kills him: two blows of 5.
    claw_blow(&mut sim, id, attack_kind::KICK, 5, 2, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn he_parries_some_pistol_shots_on_the_eleventh_and_twelfth_levels() {
    for (level, chance) in [(11, 25), (12, 50)] {
        for (roll, parried) in [(chance - 1, true), (chance, false)] {
            let (mut sim, id) = with_claw(level, 16, -200);
            sim.step(TickInput::default());
            let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
            sim.with_world(|w| {
                w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
                *w.rng = stream_drawing(101, roll as u32);
                mercat::on_hit(w, id, shot);
            });
            let gone = obj(&sim, shot).flags & flags::DESTROY != 0;
            assert!(gone, "the shot is used up either way");
            if parried {
                assert_eq!(state_of(&sim, id), Some(EnemyState::Block), "{level}");
                assert_eq!(obj(&sim, id).health, mercat::health(level));
            } else {
                assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow), "{level}");
                assert_eq!(obj(&sim, id).health, mercat::health(level) - 8);
            }
        }
    }
    // Elsewhere never.
    let (mut sim, id) = with_claw(10, 16, -200);
    sim.step(TickInput::default());
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        *w.rng = stream_drawing(101, 0);
        mercat::on_hit(w, id, shot);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
}

#[test]
fn an_explosion_hurts_him_with_a_long_cooldown() {
    let (mut sim, id) = with_claw(11, 17, -300);
    sim.step(TickInput::default());
    let blast = attacker(&mut sim, types::LEVEL_SHOT, 4);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        mercat::on_hit(w, id, blast);
    });
    assert_eq!(obj(&sim, id).health, 5);
    assert_eq!(mdata(&sim, id).enemy.cooldown.len, 2000);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
}

#[test]
fn idle_he_changes_his_pose_every_second_and_paces_a_fifth_of_the_time() {
    let (mut sim, id) = with_claw(11, 18, 3000);
    sim.with_world(|w| {
        mercat::go_idle(w, id);
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.enemy.mode = 6;
            m.wait = enemy::Timer::default();
        }
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!(mdata(&sim, id).idle.len, mercat::IDLE_MS);
    // The wait over: one of the four poses, and a wait of 1000 ms.
    sim.with_world(|w| {
        *w.rng = stream_drawing(4, 2);
        mercat::idle_tick(w, id);
    });
    assert_eq!(obj(&sim, id).anim.key(), Some(mercat::IDLE_ANIMS[2]));
    assert_eq!(mdata(&sim, id).wait.len, mercat::POSE_MS);
    // His idle time over, a draw below 20 starts pacing; 20 does not.
    for (roll, pace) in [(20, false), (19, true)] {
        sim.with_world(|w| {
            enemy::set_state(w, id, EnemyState::Idle);
            let now = w.now_ms;
            if let Some(m) = mercat::mercat_data_mut(w, id) {
                m.idle = enemy::Timer::default();
                m.wait = enemy::Timer::new(now, 1000);
                m.enemy.mode = 6;
            }
            *w.rng = stream_drawing(100, roll);
            mercat::idle_tick(w, id);
        });
        let st = state_of(&sim, id);
        assert_eq!(st == Some(EnemyState::Pace), pace, "{roll}");
    }
    // Claw in a box: Home.
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Idle);
        if let Some(m) = mercat::mercat_data_mut(w, id) {
            m.enemy.mode = 2;
        }
        mercat::idle_tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn lifted_he_says_his_line_and_makes_no_draw_of_the_shared_lift() {
    let (mut sim, id) = with_claw(11, 19, -40);
    sim.step(TickInput::default());
    sim.with_world(|w| {
        let claw = w.claw_id.unwrap();
        let before = w.rng.state();
        logic::lifted(w, id, claw);
        assert_eq!(w.rng.state(), before, "no draw");
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
}

#[test]
fn he_carries_his_records_loot_and_dies_at_0() {
    let mut level = arena(11);
    level.objects[0].record[field::POWERUP] = 33;
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 20,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(AT - 300, 899);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Mercat))
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(mdata(&sim, id).enemy.loot[8], 33);
    let blast = attacker(&mut sim, types::LEVEL_SHOT, 20);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        mercat::on_hit(w, id, blast);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert!(mdata(&sim, id).enemy.dead);
}
