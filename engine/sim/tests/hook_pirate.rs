//! The hook-handed pirate on a synthetic floor with the ninth level's
//! animation timings: how he is built, his decisions (the hook hold, the
//! hook swing and the walk in from 100 to 300 px, the dagger stab every
//! 56 ticks within 100 px), the hold whose drawn box hurts Claw on every
//! touch his protection allows, the swing live on its 7th tick and over on
//! its 19th, the stab live on its 5th and over on its 21st, the parry of
//! every standing sword stab on the ninth level (not on the tenth), the
//! crouching stab that lands and is answered at once with the hook, the
//! pistol shot parried only during the hold, the lift veto drawn every
//! tick, and his lifted and go-idle lines.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::enemy::hook_pirate::{self, DAGGER_STAB, HOOK_SWING, HookPirateData, keys};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
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

/// The hook swing: live on its third picture.
const STRIKE1: &[(i32, i32, i32)] = &[
    (300, 30, 0),
    (301, 60, 0),
    (302, 80, 1),
    (303, 60, 0),
    (304, 45, 0),
];
/// The dagger stab: live on its second picture.
const STRIKE2: &[(i32, i32, i32)] = &[(310, 60, 0), (311, 200, 1), (312, 70, 0)];
/// The parry and the hold: pictures 200, 850 and 851, the last held.
const BLOCK: &[(i32, i32, i32)] = &[(200, 70, 0), (850, 100, 0), (851, 0, 0)];
/// A flinch.
const HIT: &[(i32, i32, i32)] = &[(400, 50, 0), (401, 200, 0), (400, 50, 0), (400, 0, 0)];
/// A swing of Claw: wind-up, the active frame (event 1), follow-through.
const SWING: &[(i32, i32, i32)] = &[(24, 50, 0), (25, 50, 0), (27, 100, 1), (24, 50, 0)];

/// The boxes of the hold's pictures, facing left (as drawn unmirrored).
const BOX_200: Rect = Rect::new(-30, -47, 53, 60);
const BOX_850: Rect = Rect::new(-30, -34, 53, 59);
const BOX_851: Rect = Rect::new(-43, -32, 60, 59);

fn pirate_anims() -> Vec<Arc<Animation>> {
    let a = hook_pirate::CLASS.anims;
    vec![
        anim(keys::STRIKE1, STRIKE1, true),
        anim(keys::STRIKE2, STRIKE2, true),
        anim(keys::BLOCK, BLOCK, true),
        anim(a.hit_low, HIT, true),
        anim(a.hit_high, HIT, true),
        anim(a.jump, &[(500, 70, 0), (501, 100, 0), (501, 0, 0)], true),
        anim(a.bounce, &[(1, 0, 0)], true),
        anim(a.lifted, &[(600, 80, 0), (601, 0, 0)], true),
        anim(a.lifted2, &[(602, 100, 0), (603, 100, 0)], false),
        anim(a.advance, &[(10, 150, 0), (11, 150, 0)], false),
        anim(a.fast_advance, &[(12, 100, 0), (13, 100, 0)], false),
        anim(a.fall, &[(900, 100, 0)], false),
        anim(keys::IDLE1, &[(100, 400, 0), (101, 400, 0)], true),
    ]
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        v.push(anim(key, SWING, true));
    }
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

/// Where the arena's pirate stands and his patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor (feet rest on 959) as level `number`, with a pirate at
/// `AT` between `LIMITS`; every object awake. His picture's frames carry
/// the boxes of the hold's pictures.
fn arena(number: u32) -> SimLevel {
    let mut level = flat_level();
    level.number = number;
    for a in pirate_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut frames = ImageFrames::range(1, 999);
    frames.boxes.insert(200, BOX_200);
    frames.boxes.insert(850, BOX_850);
    frames.boxes.insert(851, BOX_851);
    level.images.insert(keys::IMAGE_SET.into(), frames);
    let mut o = ObjectSpec::at(hook_pirate::NAMES[0], AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    // The record's own health does not count.
    o.record[18] = 40;
    level.objects = vec![o];
    level
}

/// The arena on level `number` with Claw put `dx` px from the pirate on
/// the first tick, tough enough to take any number of strikes.
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
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::HookPirate))
        .map(|(id, _)| id)
        .expect("a pirate");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn pdata(sim: &Sim, id: ObjectId) -> HookPirateData {
    match &obj(sim, id).data {
        ObjectData::HookPirate(d) => d.clone(),
        d => panic!("pirate data: {d:?}"),
    }
}

/// The pirate's states over `n` ticks of `input`.
fn states(sim: &mut Sim, id: ObjectId, n: usize, input: TickInput) -> Vec<Option<EnemyState>> {
    (0..n)
        .map(|_| {
            sim.step(input);
            sim.objects()
                .get(id)
                .and_then(|o| EnemyState::from_id(o.state))
        })
        .collect()
}

/// The spans (first, last) of the ticks on which `s` holds in `seen`.
fn spans(seen: &[Option<EnemyState>], s: EnemyState) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < seen.len() {
        if seen[k] == Some(s) {
            let a = k;
            while k + 1 < seen.len() && seen[k + 1] == Some(s) {
                k += 1;
            }
            out.push((a, k));
        }
        k += 1;
    }
    out
}

/// A random stream whose next draw of 0..`n` is `want`.
fn stream_drawing(n: u32, want: u32) -> Rng {
    (1..100_000)
        .map(Rng::new)
        .find(|r| r.clone().below(n) == want)
        .expect("a seed")
}

/// Puts the pirate Home, alerted and rested, the next draw of Home's
/// 0..121 being `r`, and runs one tick.
fn decide_with(sim: &mut Sim, id: ObjectId, r: u32) {
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Home);
        if let Some(d) = enemy::data_mut(w, id) {
            d.alerted = true;
            d.busy = false;
        }
        if let Some(ObjectData::HookPirate(p)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            p.strike_rest = enemy::Timer::default();
        }
        // The pre-tick draws the lift veto first: the stream is set so the
        // draw after it is `r`.
        let target = stream_drawing(121, r);
        *w.rng = rewind_one(target);
    });
    sim.step(TickInput::default());
}

/// The stream one draw before `rng`: the state whose next state is
/// `rng`'s.
fn rewind_one(rng: Rng) -> Rng {
    // The generator is x' = 214013 x + 2531011 (mod 2^32); its inverse
    // multiplier is 3115528533.
    let s = rng.state();
    Rng::new(s.wrapping_sub(2_531_011).wrapping_mul(3_115_528_533))
}

/// An attacker of type `kind` doing `damage`, at the pirate.
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

/// Hits the pirate with a new attack of Claw's of `kind` doing `damage`,
/// Claw in `posture`.
fn claw_blow(sim: &mut Sim, id: ObjectId, kind: i32, damage: i32, posture: i32) {
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        if let Some(c) = w.objects.get_mut(claw) {
            c.damage = damage;
            c.physics_type = posture;
        }
        w.claw.attack.kind = kind;
        w.claw.attack.uppercut = false;
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        hook_pirate::on_hit(w, id, claw);
    });
}

#[test]
fn the_synthetic_animations_keep_the_ninth_levels_timings() {
    let level = arena(9);
    let calls = |key: &str| {
        let mut p = claw_sim::anim::AnimPlayer::default();
        p.set(level.animations[key].clone());
        let mut events = Vec::new();
        let mut n = 0;
        while !p.is_done() {
            n += 1;
            if p.advance_in(18, 0, false, None).event == 1 {
                events.push(n);
            }
        }
        (n, events.first().copied())
    };
    assert_eq!(calls(keys::STRIKE1), (19, Some(7)));
    assert_eq!(calls(keys::STRIKE2), (21, Some(5)));
    assert_eq!(calls(keys::BLOCK), (11, None));
    assert_eq!(calls(hook_pirate::CLASS.anims.hit_low), (19, None));
}

#[test]
fn he_is_built_idle_with_his_own_numbers() {
    let (sim, id) = with_claw(9, 1, 400);
    let o = obj(&sim, id);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!((o.health, o.damage), (10, 10));
    assert_eq!(&*o.image_set, hook_pirate::image_set());
    assert_eq!(o.hit_rect.left, -20);
    assert_eq!(o.attack_rect, hook_pirate::CONTACT);
    let p = pdata(&sim, id);
    assert_eq!(p.enemy.smarts, 99);
    assert_eq!(p.enemy.ranges, hook_pirate::RANGES);
    assert_eq!(p.enemy.bounds, LIMITS);
    assert!(!p.enemy.sight, "not going by sight");
    assert!(p.enemy.may_lift);
    assert!(p.strike_rest.done(0));
    assert_eq!(o.y, 959 - 61);
}

#[test]
fn from_100_to_300_px_he_holds_swings_or_walks_in_by_the_draw() {
    // Claw 250 px away: range mode 1.
    for (r, want) in [
        (0, EnemyState::HoldHook),
        (29, EnemyState::HoldHook),
        (30, EnemyState::Strike),
        (59, EnemyState::Strike),
        (60, EnemyState::FastAdvance),
        (120, EnemyState::FastAdvance),
    ] {
        let (mut sim, id) = with_claw(9, 2, -250);
        decide_with(&mut sim, id, r);
        assert_eq!(state_of(&sim, id), Some(want), "draw {r}");
        if want != EnemyState::FastAdvance {
            // Either way he rests 2000 ms.
            let rest = pdata(&sim, id).strike_rest;
            assert_eq!(rest.len, 2000, "draw {r}");
        }
    }
    // On the tenth level the same draws swing instead of holding.
    let (mut sim, id) = with_claw(10, 2, -250);
    decide_with(&mut sim, id, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    // Claw 150 px away (mode 2): he walks in.
    let (mut sim, id) = with_claw(9, 2, -150);
    decide_with(&mut sim, id, 90);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Advance));
}

#[test]
fn the_hold_lasts_112_ticks_and_its_box_is_his_drawn_picture() {
    let (mut sim, id) = with_claw(9, 2, -250);
    decide_with(&mut sim, id, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HoldHook));
    // Facing Claw on his left: the boxes as drawn, unmirrored.
    assert!(!enemy::faces_right(obj(&sim, id)));
    let mut seen = vec![Some(EnemyState::HoldHook)];
    let mut boxes = vec![obj(&sim, id).attack_rect];
    for _ in 0..120 {
        sim.step(TickInput::default());
        seen.push(state_of(&sim, id));
        boxes.push(obj(&sim, id).attack_rect);
    }
    let hold = spans(&seen, EnemyState::HoldHook)[0];
    assert_eq!(hold, (0, 111), "112 ticks, Home on H + 112");
    assert_eq!(seen[112], Some(EnemyState::Home));
    // The deciding tick keeps the contact point; then the box of the
    // picture shown when each tick starts: picture 200 for 4 ticks, 850
    // for 6, then 851.
    assert_eq!(boxes[0], hook_pirate::CONTACT);
    assert_eq!(&boxes[2..6], &[BOX_200; 4]);
    assert_eq!(&boxes[6..12], &[BOX_850; 6]);
    assert!(boxes[12..=111].iter().all(|b| *b == BOX_851));
    // Back Home, his contact point again.
    assert_eq!(boxes[113], hook_pirate::CONTACT);
}

#[test]
fn the_held_hook_hurts_claw_on_every_touch_his_protection_allows() {
    // Claw put 40 px right of the pirate, inside picture 851's box.
    let (mut sim, id) = with_claw(9, 2, 250);
    sim.with_world(|w| w.claw.stats.health = 100);
    decide_with(&mut sim, id, 10);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HoldHook));
    sim.teleport_player(AT + 40, 899);
    let mut health = Vec::new();
    for _ in 0..110 {
        sim.step(TickInput::default());
        health.push(sim.claw().stats.health);
        // Keep Claw in the box whatever knocks him away.
        let x = sim.player().unwrap().x;
        if x != AT + 40 {
            sim.teleport_player(AT + 40, 899);
        }
    }
    let hits: Vec<usize> = (1..health.len())
        .filter(|&k| health[k] < health[k - 1])
        .collect();
    assert!(hits.len() >= 2, "{hits:?}");
    for w in hits.windows(2) {
        // 850 ms of protection: 48 ticks between two hits.
        assert_eq!(w[1] - w[0], 48, "{hits:?}");
    }
    assert!(hits.iter().all(|&k| health[k - 1] - health[k] == 10));
}

#[test]
fn the_swing_is_live_on_its_7th_tick_and_the_stab_on_its_5th() {
    // The swing at the air from 250 px.
    let (mut sim, id) = with_claw(9, 2, -250);
    decide_with(&mut sim, id, 40);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    let mut seen = vec![(state_of(&sim, id), obj(&sim, id).attack_rect)];
    for _ in 0..25 {
        sim.step(TickInput::default());
        seen.push((state_of(&sim, id), obj(&sim, id).attack_rect));
    }
    let live: Vec<usize> = (0..seen.len())
        .filter(|&k| seen[k].1 == HOOK_SWING)
        .collect();
    assert_eq!(live, [7]);
    let len = seen
        .iter()
        .take_while(|s| s.0 == Some(EnemyState::Strike))
        .count();
    assert_eq!(len, 19, "Home on S + 19");
    // Close in (mode 4) he stabs: live on S + 5, Home on S + 21, the next
    // stab 56 ticks after.
    let (mut sim, id) = with_claw(9, 2, -60);
    let mut seen = Vec::new();
    for _ in 0..300 {
        sim.step(TickInput::default());
        seen.push((state_of(&sim, id), obj(&sim, id).attack_rect));
    }
    let st: Vec<Option<EnemyState>> = seen.iter().map(|s| s.0).collect();
    // The strikes that end before the last tick.
    let strikes: Vec<(usize, usize)> = spans(&st, EnemyState::Strike)
        .into_iter()
        .filter(|s| s.1 + 1 < st.len())
        .collect();
    assert!(strikes.len() >= 4, "{strikes:?}");
    for w in strikes.windows(2) {
        assert_eq!(w[1].0 - w[0].0, 56, "{strikes:?}");
    }
    for &(a, b) in &strikes {
        assert_eq!(b - a + 1, 21, "{strikes:?}");
        let live: Vec<usize> = (a..=b)
            .filter(|&k| seen[k].1 == DAGGER_STAB.mirrored() || seen[k].1 == DAGGER_STAB)
            .map(|k| k - a)
            .collect();
        assert_eq!(live, [5]);
    }
}

#[test]
fn on_the_ninth_level_every_standing_sword_stab_is_parried() {
    let (mut sim, id) = with_claw(9, 3, 400);
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, ptype::STAND);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Block));
    assert_eq!(obj(&sim, id).health, 10);
    assert!(obj(&sim, id).flags & flags::SAFE != 0);
    let seen = states(&mut sim, id, 12, TickInput::default());
    let guard = seen
        .iter()
        .take_while(|s| **s == Some(EnemyState::Block))
        .count();
    assert_eq!(guard + 1, 11, "the tick of the parry and 10 more");
    assert!(obj(&sim, id).flags & flags::SAFE == 0);
    // A jumping stab is not made standing: it lands.
    let (mut sim, id) = with_claw(9, 3, 400);
    claw_blow(&mut sim, id, attack_kind::JUMP_SWIPE, 5, ptype::JUMP);
    assert_eq!(obj(&sim, id).health, 5);
    // A punch lands too.
    let (mut sim, id) = with_claw(9, 3, 400);
    claw_blow(&mut sim, id, attack_kind::PUNCH, 5, ptype::STAND);
    assert_eq!(obj(&sim, id).health, 5);
    // On the tenth level the standing stab lands.
    let (mut sim, id) = with_claw(10, 3, 400);
    claw_blow(&mut sim, id, attack_kind::SWIPE, 5, ptype::STAND);
    assert_eq!(obj(&sim, id).health, 5);
}

#[test]
fn a_blow_that_lands_is_answered_at_once_with_the_hook() {
    let (mut sim, id) = with_claw(9, 3, 400);
    sim.with_world(|w| {
        if let Some(d) = enemy::data_mut(w, id) {
            d.strike_rect = DAGGER_STAB;
        }
    });
    claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, ptype::DUCK);
    // No flinch drawn, no knockback: the hook swing starts in the same
    // pass.
    assert_eq!(obj(&sim, id).health, 8);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    let d = pdata(&sim, id).enemy;
    assert_eq!(d.vx, 0.0);
    assert_eq!(d.strike_rect, HOOK_SWING);
    assert_eq!(d.strike_anim, keys::STRIKE1);
    // A blow meeting him in a flinch still costs him health, then he goes
    // Home.
    let (mut sim, id) = with_claw(9, 3, 400);
    sim.with_world(|w| enemy::set_state(w, id, EnemyState::HitLow));
    claw_blow(&mut sim, id, attack_kind::PUNCH, 5, ptype::STAND);
    assert_eq!(obj(&sim, id).health, 5);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    // Five crouching stabs kill him.
    let (mut sim, id) = with_claw(9, 3, 400);
    for _ in 0..5 {
        sim.with_world(|w| {
            if let Some(d) = enemy::data_mut(w, id) {
                d.cooldown = enemy::Timer::default();
            }
        });
        claw_blow(&mut sim, id, attack_kind::DUCK_SWIPE, 2, ptype::DUCK);
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn a_pistol_shot_is_parried_only_while_he_holds_the_hook() {
    // Out of the hold: one of his hit sounds, then the shared pistol rule.
    let (mut sim, id) = with_claw(9, 3, 400);
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        hook_pirate::on_hit(w, id, shot);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 2);
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
    let seen = states(&mut sim, id, 25, TickInput::default());
    let flinch = seen
        .iter()
        .take_while(|s| **s == Some(EnemyState::HitLow))
        .count();
    assert_eq!(flinch + 1, hook_pirate::HIT_LOW_TICKS as usize);
    // In the hold: the parry, unhurt, the shot flying on.
    let (mut sim, id) = with_claw(9, 3, -250);
    decide_with(&mut sim, id, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HoldHook));
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        hook_pirate::on_hit(w, id, shot);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Block));
    assert_eq!(obj(&sim, id).health, 10);
    assert!(obj(&sim, shot).flags & flags::DESTROY == 0);
}

#[test]
fn about_one_tick_in_five_he_cannot_be_lifted() {
    let (mut sim, id) = with_claw(9, 7, 400);
    let mut vetoed = 0;
    let n = 2000;
    for _ in 0..n {
        sim.step(TickInput::default());
        if obj(&sim, id).special_flags & special::LIFTABLE == 0 {
            vetoed += 1;
        }
    }
    assert!((n / 7..n / 3).contains(&vetoed), "{vetoed} of {n}");
}

#[test]
fn lifted_he_says_one_of_his_two_lines_from_the_lifts_own_draw() {
    // The line reads the draw the shared lift makes next: the stream is
    // left where it was.
    let (mut sim, id) = with_claw(9, 2, 400);
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| hook_pirate::pick_lift_voice(w, id));
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
    assert_eq!(pdata(&sim, id).enemy.lifted_voice, "");
    // The whole lift draws once.
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        logic::lifted(w, id, claw);
    });
    let mut expect = before.clone();
    expect.below(100);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
}

#[test]
fn going_idle_far_off_screen_draws_only_the_shared_draw() {
    let (mut sim, id) = with_claw(9, 2, 400);
    for (mode, draws) in [(6u8, 1), (5, 2)] {
        let before = sim.with_world(|w| {
            if let Some(d) = enemy::data_mut(w, id) {
                d.mode = mode;
            }
            // A draw of 0..100 of 99: no line.
            *w.rng = stream_drawing(100, 99);
            w.rng.clone()
        });
        sim.with_world(|w| hook_pirate::go_idle(w, id));
        let mut expect = before.clone();
        for _ in 0..draws {
            expect.below(100);
        }
        assert_eq!(sim.with_world(|w| w.rng.clone()), expect, "mode {mode}");
        assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    }
}
