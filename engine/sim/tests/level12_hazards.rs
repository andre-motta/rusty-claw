//! The twelfth level's stalactites and earthquake triggers on synthetic
//! levels: a stalactite lets go on the tick after Claw's position enters
//! its rectangle (left and top edges in), falls `trunc(0.324 n^2)` px,
//! lands, crumbles for 19 ticks out of every collision and is hidden
//! until Claw loses a life; it hurts Claw hanging and falling, for 5; a
//! pistol shot flies through it. An earthquake trigger fires on Claw's
//! touch with one draw from the random stream and a five-second shake,
//! takes no part in any collision for seven seconds and fires again if
//! Claw is still in its box.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::logic::{shake, stalactite};
use claw_sim::object::{LogicKind, ObjectId, draw_flags, flags, types};
use claw_sim::rng::Rng;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A standing Claw on the flat level's floor (top at 960).
const STAND_Y: i32 = 899;
/// A stalactite whose feet row lands on the floor: 44 px above it.
const LANDED_Y: i32 = 960 - 44;
/// A stalactite 202 px above that, as the first one over the twelfth
/// level's water's edge.
const HANG_Y: i32 = LANDED_Y - 202;

/// An animation of `images`, `ms` each, finishing on the last.
fn once(key: &str, images: &[i32], ms: i32) -> Animation {
    let mut frames: Vec<AnimFrame> = images
        .iter()
        .map(|&i| AnimFrame {
            image_op: 3,
            image_index: i,
            duration: ms,
            flags: 0x2,
            ..AnimFrame::default()
        })
        .collect();
    frames.last_mut().unwrap().next_op = 9;
    Animation {
        key: key.to_string(),
        frames,
        ..Animation::default()
    }
}

/// The flat level as the twelfth level, Claw starting at `start_x`, with
/// `objects`.
fn level_with(start_x: i32, objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 12;
    level.start = (start_x, STAND_Y);
    level.animations.insert(
        stalactite::HOME_ANIM.to_string(),
        Arc::new(once(stalactite::HOME_ANIM, &[1], 0)),
    );
    level.animations.insert(
        stalactite::CRUMBLE_ANIM.to_string(),
        Arc::new(once(stalactite::CRUMBLE_ANIM, &[2, 3, 4, 5, 6, 7], 50)),
    );
    level
        .images
        .insert(stalactite::IMAGE_SET.into(), ImageFrames::range(1, 7));
    level.objects = objects;
    level
}

/// A stalactite at (x, y) watching the rectangle `trigger` (left, top,
/// right, bottom).
fn stalactite_at(x: i32, y: i32, trigger: [i32; 4]) -> ObjectSpec {
    let mut s = ObjectSpec::at(stalactite::NAMES[0], x, y, 1010);
    s.image_set = stalactite::IMAGE_SET.into();
    s.frame = -1;
    for (i, v) in stalactite::TRIGGER_FIELDS.iter().zip(trigger) {
        s.record[*i] = v;
    }
    s
}

/// An earthquake trigger at (x, y) with the box `area` (left, top, right,
/// bottom), as the level files place it (hidden).
fn shake_at(x: i32, y: i32, area: [i32; 4]) -> ObjectSpec {
    let mut s = ObjectSpec::at(shake::NAMES[0], x, y, 0);
    s.image_set = "GAME_SOUNDICON".into();
    s.record[11] = draw_flags::HIDDEN as i32;
    for (i, v) in shake::ATTACK_FIELDS.iter().zip(area) {
        s.record[*i] = v;
    }
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

fn state(sim: &Sim, id: ObjectId) -> i32 {
    sim.objects().get(id).unwrap().state
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|a| matches!(a, AudioCue::Play(s) if s.key == key))
}

/// Puts Claw down at `x` after the level's first tick, and runs the tick
/// after it.
fn put_claw(sim: &mut Sim, x: i32) {
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some((x, STAND_Y)));
    sim.step(TickInput::default());
}

#[test]
fn a_stalactite_lets_go_on_the_tick_after_claw_enters_and_falls_a_third_of_n_squared() {
    let trigger = [1200, 800, 1272, 960];
    let level = level_with(1000, vec![stalactite_at(1300, HANG_Y, trigger)]);
    let mut sim = Sim::new(level, original());
    let id = find(&sim, stalactite::NAMES[0]);
    put_claw(&mut sim, 1199);
    // Left of the rectangle it hangs, its picture and box set up.
    for _ in 0..5 {
        sim.step(TickInput::default());
        assert_eq!(state(&sim, id), stalactite::state::IDLE);
    }
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.hit_rect, stalactite::BOX);
    assert_eq!(o.attack_rect, stalactite::BOX);
    assert_eq!(o.type_flags, stalactite::TYPE);
    assert_eq!(o.damage, stalactite::DAMAGE);
    assert_eq!(o.frame, 1);
    assert_eq!(o.flags & flags::NO_COLLIDE, 0, "a hanging one collides");
    // Claw put on the rectangle's left edge between two ticks: it lets go
    // in the next tick (T), and falls from T + 1 with the snap.
    sim.with_world(|w| w.claw.special.warp_to = Some((1200, STAND_Y)));
    sim.step(TickInput::default());
    assert_eq!(state(&sim, id), stalactite::state::DELAY);
    let mut ys = Vec::new();
    let mut snap = None;
    let mut dead = None;
    for n in 1..=30u32 {
        sim.step(TickInput::default());
        if played(&sim, stalactite::SNAP_SOUND) {
            snap = Some(n);
        }
        if dead.is_none() && state(&sim, id) == stalactite::state::DEAD {
            dead = Some(n);
            assert!(played(&sim, stalactite::HIT_SOUND));
        }
        ys.push(sim.objects().get(id).unwrap().y);
    }
    assert_eq!(snap, Some(1));
    for n in 1..=25u32 {
        assert_eq!(
            ys[n as usize - 1],
            (HANG_Y + stalactite::fall_after(n)).min(LANDED_Y),
            "T + {n}"
        );
    }
    assert_eq!(ys[24], LANDED_Y, "landed on T + 25");
    assert_eq!(dead, Some(26), "crumbles on T + 26");
    assert_ne!(
        sim.objects().get(id).unwrap().flags & flags::NO_COLLIDE,
        0,
        "out of every collision"
    );
    // Claw, 28 px left of its box, is never hurt.
    assert_eq!(sim.claw().stats.health, 100);
}

#[test]
fn it_crumbles_for_19_ticks_and_hangs_again_after_a_lost_life() {
    let level = level_with(
        1000,
        vec![stalactite_at(1300, HANG_Y, [1200, 800, 1272, 960])],
    );
    let mut sim = Sim::new(level, original());
    let id = find(&sim, stalactite::NAMES[0]);
    put_claw(&mut sim, 1230);
    let mut frames = Vec::new();
    let mut hidden = None;
    for k in 0..80 {
        sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        if o.state == stalactite::state::DEAD {
            frames.push(o.frame);
        }
        if hidden.is_none() && o.state == stalactite::state::RESTART {
            hidden = Some(k);
            assert_ne!(o.draw_flags & draw_flags::HIDDEN, 0);
        }
    }
    // Pictures 2 to 7, three ticks each, after its first tick of crumble.
    assert_eq!(frames.len() as u32, stalactite::CRUMBLE_TICKS);
    let mut want = vec![1];
    for f in 2..=7 {
        want.extend([f; 3]);
    }
    assert_eq!(frames, want);
    assert!(hidden.is_some());
    // It stays gone while Claw lives.
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    assert_eq!(state(&sim, id), stalactite::state::RESTART);
    // He loses a life: it hangs again at its home, shown and colliding.
    sim.end_life();
    for _ in 0..400 {
        sim.step(TickInput::default());
        if state(&sim, id) == stalactite::state::IDLE {
            break;
        }
    }
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.state, stalactite::state::IDLE);
    assert_eq!((o.x, o.y), (1300, HANG_Y));
    assert_eq!(o.draw_flags & draw_flags::HIDDEN, 0);
    assert_eq!(o.flags & flags::NO_COLLIDE, 0);
}

#[test]
fn a_stalactite_hurts_claw_falling_on_him_and_hanging() {
    // Falling on a Claw standing under it.
    let level = level_with(
        1000,
        vec![stalactite_at(1300, HANG_Y, [1200, 800, 1320, 960])],
    );
    let mut sim = Sim::new(level, original());
    put_claw(&mut sim, 1290);
    let mut hits = Vec::new();
    for k in 0..60 {
        let before = sim.claw().stats.health;
        sim.step(TickInput::default());
        if sim.claw().stats.health < before {
            hits.push((k, before - sim.claw().stats.health));
        }
    }
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].1, stalactite::DAMAGE);
    // Hanging low enough to touch a Claw walking under it.
    let level = level_with(1000, vec![stalactite_at(1300, 850, [0, 0, 1, 1])]);
    let mut sim = Sim::new(level, original());
    let id = find(&sim, stalactite::NAMES[0]);
    put_claw(&mut sim, 1200);
    let mut hit = false;
    for _ in 0..80 {
        let before = sim.claw().stats.health;
        sim.step(key_down(&sim, RIGHT));
        if sim.claw().stats.health == before - stalactite::DAMAGE {
            hit = true;
            break;
        }
    }
    assert!(hit, "a hanging stalactite hurts");
    assert_eq!(state(&sim, id), stalactite::state::IDLE, "and stays");
}

#[test]
fn a_pistol_shot_flies_through_a_hanging_stalactite() {
    let level = level_with(1000, vec![stalactite_at(1300, 940, [0, 0, 1, 1])]);
    let mut sim = Sim::new(level, original());
    let id = find(&sim, stalactite::NAMES[0]);
    put_claw(&mut sim, 1100);
    // Claw's shot, as his pistol fires it, through the stalactite's box.
    let claw = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id);
    let spec = ProjectileSpec {
        logic: "PistolBullet",
        x: 1171,
        y: STAND_Y - 21,
        z: 3999,
        vx: 700,
        damage: 8,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_BULLETS",
        owner: claw,
        ..ProjectileSpec::default()
    };
    let bullet = sim
        .with_world(|w| projectile::spawn(w, spec))
        .expect("a shot");
    let mut furthest = 0;
    for _ in 0..40 {
        sim.step(TickInput::default());
        if let Some(o) = sim.objects().get(bullet) {
            furthest = furthest.max(o.x);
        }
    }
    assert!(furthest > 1500, "the shot flies on past it: {furthest}");
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.state, o.x, o.y), (stalactite::state::IDLE, 1300, 940));
}

#[test]
fn a_shake_fires_on_claws_touch_with_one_draw_and_rearms_389_ticks_later() {
    let area = [1150, 700, 1400, 960];
    let with = level_with(1000, vec![shake_at(1200, 900, area)]);
    // The same level with the trigger far away, asleep: the draws of
    // everything else.
    let without = level_with(1000, vec![shake_at(9000, 900, area)]);
    let mut sim = Sim::new(with, original());
    let mut twin = Sim::new(without, original());
    let id = find(&sim, shake::NAMES[0]);
    for s in [&mut sim, &mut twin] {
        s.step(TickInput::default());
    }
    assert_eq!(state(&sim, id), shake::state::ARMED);
    let o = sim.objects().get(id).unwrap();
    assert_eq!(
        o.attack_rect,
        claw_sim::object::Rect::new(-50, -200, 200, 60)
    );
    assert_ne!(o.flags & flags::SAFE, 0, "never hit");
    // Claw walks right into its box.
    let mut fired = None;
    for k in 0..200 {
        let input = key_down(&sim, RIGHT);
        sim.step(input);
        twin.step(input);
        if state(&sim, id) == shake::state::WAIT {
            fired = Some(k);
            // Exactly one more draw than the level without it, and the
            // shake starts.
            let mut r = Rng::new(twin.rng_state());
            let draw = r.below(6);
            assert_eq!(sim.rng_state(), r.state());
            let rumble = shake::rumble_for(draw, None);
            for (i, key) in shake::RUMBLES.iter().enumerate() {
                assert_eq!(played(&sim, key), rumble == Some(i), "{key}");
            }
            break;
        }
    }
    fired.expect("it fires");
    assert!(sim.camera().shake_until_ms > 0, "the screen shakes");
    // Claw stays inside: out of every collision for 388 ticks, then back
    // in on the 389th, when it fires again at once.
    let start = |sim: &Sim| {
        shake::data_of(sim.objects().get(id).unwrap())
            .unwrap()
            .start_ms
    };
    let first = start(&sim);
    let mut again = None;
    for k in 1..=400u32 {
        sim.step(TickInput::default());
        if start(&sim) != first {
            again = Some(k);
            break;
        }
        let out = sim.objects().get(id).unwrap().flags & flags::NO_COLLIDE != 0;
        assert!(out, "out of every collision on tick {k}");
    }
    assert_eq!(again, Some(389));
    assert_eq!(state(&sim, id), shake::state::WAIT);
}
