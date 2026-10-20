//! Marrow on a synthetic room at the tenth level's place, with the boss
//! fight flag set directly: how he is built and leaves his start-up, his
//! Home that only the fight opens, his shot, his blocks and flinches, the
//! health rule and his whistle for the parrot with its timeout, his
//! respawn reset, his death and its report, the part of a pixel he stands
//! below his height, and his fall reset.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, marrow};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// Marrow as placed.
const AT: (i32, i32) = (41063, 7255);
/// The top of the room's floor: as on the tenth level, he stands above
/// the height he was placed at.
const FLOOR: i32 = 7296;

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

/// His animations with the lengths of the game's.
fn anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_MARROW_{s}");
    vec![
        anim(&k("HOME"), &[(200, 0, 0)], false),
        anim(
            &k("STRIKE1"),
            &[
                (1, 0, 0),
                (2, 50, 0),
                (3, 80, 0),
                (4, 150, 1),
                (5, 100, 0),
                (6, 100, 0),
                (7, 50, 0),
                (8, 0, 0),
            ],
            true,
        ),
        anim(
            &k("STRIKE2"),
            &[
                (10, 100, 0),
                (11, 50, 0),
                (12, 100, 1),
                (13, 100, 0),
                (14, 60, 0),
            ],
            true,
        ),
        anim(&k("IDLE1"), &[(20, 150, 0), (21, 100, 0), (22, 0, 0)], true),
        anim(
            &k("HITLOW"),
            &[(30, 50, 0), (31, 200, 0), (32, 50, 0), (33, 0, 0)],
            true,
        ),
        anim(
            &k("HITHIGH"),
            &[(34, 50, 0), (35, 200, 0), (36, 50, 0), (37, 0, 0)],
            true,
        ),
        anim(&k("BLOCK"), &[(40, 80, 0), (41, 80, 0)], true),
        anim(&k("FASTADVANCE"), &[(50, 100, 0), (51, 100, 0)], false),
        anim(&k("KILLFALL"), &[(60, 100, 0)], false),
    ]
}

/// A synthetic room: a flat floor whose top is at [`FLOOR`] across the
/// whole width of the tenth level's boss room, the level number 10, with
/// Marrow placed and every object awake.
fn room() -> Sim {
    let mut rows = vec![".".repeat(660); 114];
    rows.push("#".repeat(660));
    rows.push("#".repeat(660));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (40480, 7300));
    level.number = 10;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for k in [
        marrow::WHISTLE_SOUND,
        marrow::PISTOL_SOUND,
        marrow::BLOCK_LINE,
        marrow::BLOCK_SOUNDS[0],
        marrow::BLOCK_SOUNDS[1],
    ] {
        level.sounds.insert(k.to_string(), Some(300));
    }
    let mut m = ObjectSpec::at("Marrow", AT.0, AT.1, 0);
    m.image_set = marrow::IMAGE_SET.into();
    // The record's own health does not count; its speed fields name the
    // point the gem rises to.
    m.record[17] = 7;
    m.record[55] = 40800;
    m.record[56] = 7170;
    level.objects = vec![m];
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    sim
}

fn marrow_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == "Marrow")
        .map(|(id, _)| id)
        .expect("Marrow")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
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

/// The room with Claw `dx` px from Marrow on the floor, tough, the fight
/// flag at `flag`, after `ticks` ticks.
fn with_claw(dx: i32, flag: u8, ticks: usize) -> (Sim, ObjectId) {
    let mut sim = room();
    let id = marrow_id(&sim);
    sim.teleport_player(AT.0 + dx, FLOOR - 61);
    sim.with_world(|w| {
        w.claw.stats.health = 10_000;
        w.set_boss_fight(flag);
    });
    idle(&mut sim, ticks);
    (sim, id)
}

/// A synthetic attacker of `type_flags` with `damage`.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits Marrow as a new attack of Claw, his roll set to `roll`.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId, roll: i32) {
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        enemy::data_mut(w, id).unwrap().roll = roll;
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, attacker);
    });
}

fn link(sim: &Sim) -> marrow::MarrowLink {
    *sim.marrow_link().expect("the Marrow link")
}

#[test]
fn he_is_built_as_the_boss_with_the_slabs_asked_open() {
    let (sim, id) = with_claw(-120, fight::NONE, 0);
    let o = obj(&sim, id);
    assert!(matches!(o.data, ObjectData::Marrow(_)), "his own data");
    assert_eq!(sim.boss().current(), Some(id));
    assert_eq!((o.health, o.damage), (marrow::HEALTH, marrow::DAMAGE));
    assert_eq!(o.move_rect, marrow::MOVE_RECT);
    assert_eq!(o.hit_rect, marrow::HIT_RECT);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    assert_eq!(o.special_flags & special::LIFTABLE, 0, "never liftable");
    assert_eq!(o.y + o.move_rect.bottom, FLOOR - 1, "on the floor");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert_eq!(o.anim.key(), Some(marrow::HOME_ANIM));
    let d = enemy::data_of(o).expect("built");
    assert!(d.sight && !d.may_lift);
    assert_eq!(d.smarts, marrow::SMARTS);
    assert_eq!(d.bounds, enemy::UNBOUNDED);
    assert_eq!(d.ranges, marrow::RANGES);
    let l = link(&sim);
    assert_eq!(l, marrow::MarrowLink::new(id));
}

#[test]
fn out_of_range_on_his_first_tick_he_idles_frozen_whatever_the_flag() {
    let (sim, id) = with_claw(-2000, fight::NONE, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!(
        obj(&sim, id).anim.key(),
        None,
        "no animation: his pose holds"
    );
    assert_eq!(obj(&sim, id).z, marrow::SORTED_Z, "sorted after Claw");
    let d = enemy::data_of(obj(&sim, id)).expect("built");
    assert_eq!(d.pace_wait.len, marrow::IDLE_WAIT_MS);
}

#[test]
fn home_is_reached_only_once_the_fight_is_on() {
    for flag in [fight::NONE, fight::STAGING] {
        let (mut sim, id) = with_claw(-200, flag, 1);
        sim.with_world(|w| marrow::go_home(w, id));
        assert_ne!(state_of(&sim, id), Some(EnemyState::Home));
        // In sight, idle, he stays idle and never shoots.
        sim.with_world(|w| marrow::go_idle(w, id));
        idle(&mut sim, 300);
        assert!(matches!(
            state_of(&sim, id),
            Some(EnemyState::Idle | EnemyState::Pace)
        ));
        assert!(sim.objects().all().all(|(_, o)| &*o.logic != "Bullet"));
    }
    let (mut sim, id) = with_claw(-200, fight::ON, 1);
    sim.with_world(|w| marrow::go_home(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn in_the_fight_he_shoots_claw_in_sight_once_his_wait_is_over() {
    // Waiting in his start-up until the flag reaches 2, then Home: in
    // sight, he fires at once.
    let (mut sim, id) = with_claw(-200, fight::NONE, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    let mut fired = None;
    for k in 0..10 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Fire) {
            fired = Some(k);
            break;
        }
    }
    assert!(fired.is_some(), "he fires");
    let d = enemy::data_of(obj(&sim, id)).expect("built");
    assert!([1000, 2000, 3000, 4000].contains(&d.fire_wait.len));
    let (mx, my) = (obj(&sim, id).x, obj(&sim, id).y);
    let mut shots = Vec::new();
    for _ in 0..60 {
        sim.step(TickInput::default());
        let now: Vec<_> = sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == "Bullet")
            .map(|(sid, o)| (sid, o.x, o.y, o.damage, o.type_flags))
            .collect();
        if shots.is_empty() && !now.is_empty() {
            shots = now;
            assert!(cues(&sim).iter().any(|c| c == marrow::PISTOL_SOUND));
        }
        if state_of(&sim, id) != Some(EnemyState::Fire) {
            break;
        }
    }
    assert_eq!(shots.len(), 1, "one bullet");
    let (_, x, y, damage, kind) = shots[0];
    assert_eq!((x, y), (mx - marrow::MUZZLE.0, my + marrow::MUZZLE.1));
    assert_eq!((damage, kind), (marrow::BULLET_DAMAGE, types::ENEMY_SHOT));
    assert_ne!(state_of(&sim, id), Some(EnemyState::Fire), "the shot ends");
}

#[test]
fn sword_blows_hurt_him_unless_the_roll_or_the_health_rule_blocks_them() {
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    let sword = attacker(&mut sim, types::PLAYER, 5);
    hit(&mut sim, id, sword, 500);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 95);
    // A roll below 400 blocks (and on 300 to 399 he says his line).
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    let sword = attacker(&mut sim, types::PLAYER, 5);
    hit(&mut sim, id, sword, 350);
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
    assert_eq!(obj(&sim, id).health, 100);
    assert!(obj(&sim, id).flags & flags::SAFE != 0);
    // Once his health is 25 below the phase health every blow is blocked.
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 75;
        w.marrow_link_mut().unwrap().phase_health = 100;
        enemy::data_mut(w, id).unwrap().busy = true;
    });
    let sword = attacker(&mut sim, types::PLAYER, 5);
    hit(&mut sim, id, sword, 999);
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
    assert_eq!(obj(&sim, id).health, 75);
}

#[test]
fn shots_and_thrown_things_are_destroyed_and_blocked_and_the_levels_shots_do_nothing() {
    for kind in [types::PISTOL_SHOT, types::THROWN, enemy::attacker::MAGIC] {
        let (mut sim, id) = with_claw(-120, fight::NONE, 2);
        let shot = attacker(&mut sim, kind, 10);
        hit(&mut sim, id, shot, 999);
        assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow), "{kind:#x}");
        assert_eq!(obj(&sim, id).health, 100);
        assert!(obj(&sim, shot).flags & flags::DESTROY != 0, "used up");
    }
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    let before = state_of(&sim, id);
    let blast = attacker(&mut sim, types::LEVEL_SHOT, 10);
    hit(&mut sim, id, blast, 999);
    assert_eq!(state_of(&sim, id), before);
    assert_eq!(obj(&sim, id).health, 100);
}

#[test]
fn twenty_five_below_the_phase_health_he_whistles_and_drops_back_after_five_seconds() {
    let (mut sim, id) = with_claw(-300, fight::NONE, 2);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 75;
        w.marrow_link_mut().unwrap().phase_health = 100;
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
    let o = obj(&sim, id);
    assert!(o.flags & flags::SAFE != 0);
    assert!(
        !enemy::faces_right(o),
        "right of the room's centre: he faces left"
    );
    let grab = sim.with_world(|w| marrow::grab_offset(w, id));
    assert_eq!(grab, -marrow::GRAB_OFFSET);
    assert!(cues(&sim).iter().any(|c| c == marrow::WHISTLE_SOUND));
    // The call when the whistle ends; the drop once 5000 ms are over.
    let mut call = None;
    let mut drop = None;
    for k in 1..400 {
        sim.step(TickInput::default());
        if call.is_none() && link(&sim).parrot_call {
            call = Some(k);
        }
        if obj(&sim, id).flags & flags::SAFE == 0 {
            drop = Some(k);
            break;
        }
    }
    assert!(call.is_some(), "the parrot is called");
    assert_eq!(
        drop,
        Some((marrow::WAIT_FOR_PARROT_MS as usize).div_ceil(18))
    );
    let l = link(&sim);
    assert_eq!((l.phase_health, l.parrot_call), (75, false));
    assert!(!enemy::data_of(obj(&sim, id)).expect("built").busy);
    // The flag at 0: his Home does nothing, so he waits on.
    assert_eq!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
}

/// The phase health is not over before it is first written: no whistle
/// at full health.
#[test]
fn with_the_phase_health_at_0_he_never_whistles() {
    let (mut sim, id) = with_claw(-300, fight::NONE, 2);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 5);
    idle(&mut sim, 50);
    assert_ne!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
}

#[test]
fn claws_respawn_puts_him_back_with_full_health_and_the_slabs_asked_open() {
    let (mut sim, id) = with_claw(-300, fight::NONE, 2);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.health = 60;
        o.x -= 200;
        let l = w.marrow_link_mut().unwrap();
        l.open_request = false;
        l.parrot_call = true;
        l.phase_health = 85;
    });
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.end_life();
    for _ in 0..400 {
        sim.step(TickInput::default());
        if sim.claw().stats.attempt == 1 && sim.claw().stats.health > 0 {
            break;
        }
    }
    let o = obj(&sim, id);
    assert_eq!((o.x, o.health), (AT.0, marrow::HEALTH));
    let l = link(&sim);
    assert!(l.open_request && !l.parrot_call);
    assert_eq!(l.phase_health, 85, "kept");
}

#[test]
fn his_death_ends_the_level_through_the_gem() {
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 5);
    let sword = attacker(&mut sim, types::PLAYER, 5);
    hit(&mut sim, id, sword, 500);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), Some(id));
    sim.step(TickInput::default());
    assert!(
        sim.objects()
            .all()
            .any(|(_, o)| &*o.logic == pickup::END_GEM)
    );
    // His body leaves the screen; its removal ends him as the boss and
    // the Marrow link.
    for _ in 0..200 {
        sim.step(TickInput::default());
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    assert!(sim.objects().get(id).is_none());
    assert_eq!(sim.boss().current(), None);
    assert_eq!(sim.marrow_link(), None);
}

#[test]
fn standing_he_creeps_a_part_of_a_pixel_down_and_back() {
    let (mut sim, id) = with_claw(-2000, fight::NONE, 3);
    let mut parts = Vec::new();
    for _ in 0..8 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        let d = enemy::data_of(o).expect("built");
        assert_eq!(o.physics_type, ptype::STAND);
        parts.push((d.carry.1 * 1000.0).round() as i32);
    }
    let start = parts.iter().position(|&p| p == 0).expect("a whole pixel");
    let cycle: Vec<i32> = parts[start..start + 4].to_vec();
    assert_eq!(cycle, [0, 324, 648, 972]);
}

#[test]
fn below_his_start_height_he_is_put_back_above_it() {
    let (mut sim, id) = with_claw(-2000, fight::NONE, 3);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.y = AT.1 + 40;
        o.physics_type = ptype::FLY;
    });
    sim.step(TickInput::default());
    let o = obj(&sim, id);
    assert!(o.y <= AT.1, "back above the line: {}", o.y);
}

#[test]
fn claw_attacking_him_from_close_by_makes_him_dodge_by_the_roll() {
    use claw_sim::logic::enemy::Deferred;
    use claw_sim::logic::enemy::human::HumanCall;
    let cases = [
        (150, Some(Deferred::Human(HumanCall::JumpBack))),
        (450, Some(Deferred::Human(HumanCall::Jump))),
        (700, None),
    ];
    for (roll, want) in cases {
        let (mut sim, id) = with_claw(-120, fight::NONE, 2);
        let got = sim.with_world(|w| {
            w.claw.attack.kind = claw_sim::player::attack::kind::SWIPE;
            w.claw.facing_right = true;
            let d = enemy::data_mut(w, id).unwrap();
            d.roll = roll;
            d.mode = 3;
            d.busy = false;
            d.deferred = None;
            marrow::pre_tick(w, id);
            enemy::data_mut(w, id).unwrap().deferred
        });
        assert_eq!(got, want, "roll {roll}");
    }
    // Claw facing away, or no attack: no dodge.
    let (mut sim, id) = with_claw(-120, fight::NONE, 2);
    let got = sim.with_world(|w| {
        w.claw.attack.kind = claw_sim::player::attack::kind::SWIPE;
        w.claw.facing_right = false;
        let d = enemy::data_mut(w, id).unwrap();
        d.roll = 150;
        d.mode = 3;
        d.deferred = None;
        marrow::pre_tick(w, id);
        enemy::data_mut(w, id).unwrap().deferred
    });
    assert_eq!(got, None);
}

/// Part of the bridge trick: Marrow running at Claw when the whistle comes
/// (Claw left his range before his last decision) keeps running through
/// the whistle at 120 px/s, about 2 px a tick, which moves the point the
/// parrot takes hold of him; he stands still only from the call on.
#[test]
fn whistling_he_keeps_running_until_the_call() {
    let (mut sim, id) = with_claw(-600, fight::NONE, 2);
    // Running at Claw, on his left, when the whistle comes.
    sim.with_world(|w| {
        enemy::states::enter_fast_advance(w, id);
        w.objects.get_mut(id).unwrap().health = 75;
        w.marrow_link_mut().unwrap().phase_health = 100;
        marrow::enter_wait(w, id);
    });
    let speed = enemy::data_of(obj(&sim, id)).expect("built").vx;
    assert_eq!(speed, -enemy::states::FAST_ADVANCE_SPEED);
    let x0 = obj(&sim, id).x;
    let mut ticks = 0;
    while !link(&sim).parrot_call {
        sim.step(TickInput::default());
        ticks += 1;
        assert!(ticks < 100, "no call");
        assert_eq!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
    }
    let ran = x0 - obj(&sim, id).x;
    let want = enemy::states::FAST_ADVANCE_SPEED * 0.018 * f64::from(ticks - 1);
    assert!(
        (f64::from(ran) - want).abs() <= 1.0,
        "ran {ran} px in {ticks} ticks"
    );
    assert!(ran >= 30, "{ran}");
    let x = obj(&sim, id).x;
    idle(&mut sim, 20);
    assert_eq!(obj(&sim, id).x, x, "still from the call on");
    assert_eq!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
}

/// The timeout of an unanswered whistle sets the phase health to his
/// health whatever it is, not to the old phase health less 25, so the next
/// whistle comes 25 below where this one ended.
#[test]
fn an_unanswered_whistle_ends_with_the_phase_health_his_health() {
    let (mut sim, id) = with_claw(-300, fight::NONE, 2);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 49;
        w.marrow_link_mut().unwrap().phase_health = 75;
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::WaitForParrot));
    let mut k = 0;
    while obj(&sim, id).flags & flags::SAFE != 0 {
        sim.step(TickInput::default());
        k += 1;
        assert!(k < 400);
    }
    assert_eq!(k, 278, "5000 ms at the 18 ms step");
    assert_eq!(link(&sim).phase_health, 49);
}
