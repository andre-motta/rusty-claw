//! Wolvington in a synthetic walled room, with the boss fight flag set
//! directly: how he is built, waiting for the fight and waking (with the
//! fight flag, and with Claw more than 40 px above him for 500 ms), his
//! decisions at Home and while walking, his volley of magic shots and the
//! shots themselves (where they leave, how fast they fly, what they hurt,
//! their light), the reach and timing of his uppercut and his low thrust,
//! the sprint, where each state goes Home to, his hop back from Claw's
//! sword, his blocks and flinches, lifting and throwing him, his death and
//! its report, the reset when Claw respawns, and the part of a pixel he
//! stands below his own.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, wolvington};
use claw_sim::logic::projectile;
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::{PUNCHABLE, kind as attack_kind};
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// Where he is placed: 1 px above the floor, as on his level.
const PLACED: (i32, i32) = (2600, 897);
/// His x and y standing on the room's floor.
const AT: (i32, i32) = (2600, 898);
/// The gem's target in his record.
const GEM_TARGET: (i32, i32) = (2549, 731);
/// Claw's y standing on the floor.
const CLAW_Y: i32 = 899;

/// An animation of (duration, event) elements; `once` makes the last
/// element finish it.
fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames
        .iter()
        .enumerate()
        .map(|(i, &(d, _))| (i as i32 + 1, d))
        .collect();
    let mut a = if once {
        Animation::once(key, &pairs)
    } else {
        Animation::looping(key, &pairs)
    };
    for (f, &(_, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

fn plain(ms: &[i32]) -> Vec<(i32, i32)> {
    ms.iter().map(|&d| (d, 0)).collect()
}

/// His animations with the lengths and events of his level's, and his
/// shot's.
fn his_anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_WOLVINGTON_{s}");
    let cast = [
        (0, 0),
        (70, 0),
        (100, 0),
        (70, 0),
        (200, 1),
        (100, 0),
        (0, 0),
    ];
    vec![
        anim(
            &k("STRIKE1"),
            &[(70, 0), (70, 0), (70, 0), (180, 1), (80, 0), (0, 0)],
            true,
        ),
        anim(
            &k("STRIKE2"),
            &[
                (40, 0),
                (40, 0),
                (40, 0),
                (40, 0),
                (40, 0),
                (200, 1),
                (40, 0),
                (40, 0),
                (40, 0),
            ],
            true,
        ),
        anim(&k("STRIKE3"), &cast, true),
        anim(&k("STRIKE4"), &cast, true),
        anim(&k("IDLE"), &plain(&[290, 250, 300, 325]), false),
        anim(&k("ADVANCE"), &plain(&[170, 170, 180, 170]), false),
        anim(&k("FASTADVANCE"), &plain(&[150; 8]), false),
        anim(&k("HITLOW"), &plain(&[50, 200, 50, 0]), true),
        anim(&k("HITHIGH"), &plain(&[50, 200, 50, 0]), true),
        anim(&k("BLOCK"), &plain(&[50, 200]), true),
        anim(&k("JUMP"), &plain(&[100, 200, 0]), true),
        anim(&k("KILLFALL"), &plain(&[100]), false),
        anim(&k("LIFTED"), &plain(&[100]), false),
        anim(&k("LIFTED2"), &plain(&[162; 4]), false),
        anim(&k("THROWN"), &plain(&[100]), false),
        anim(wolvington::shot::ANIM, &plain(&[50; 4]), false),
        anim(wolvington::light::IMAGE_SET, &plain(&[100]), false),
    ]
}

/// His walled room: a floor whose top is at y 960 (feet rest on 959)
/// between Solid walls at x 1920..1983 and 3200..3263, rising from y 320.
fn room() -> SimLevel {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..15 {
        let mut row = ".".repeat(80);
        if r >= 5 {
            row.replace_range(30..31, "#");
            row.replace_range(50..51, "#");
        }
        rows.push(row);
    }
    rows.push("#".repeat(80));
    rows.push("#".repeat(80));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (2200, 900));
    for a in his_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("Wolvington", PLACED.0, PLACED.1, 0);
    o.image_set = "LEVEL_WOLVINGTON".into();
    // The record's own health does not count.
    o.record[18] = 200;
    o.record[55] = GEM_TARGET.0;
    o.record[56] = GEM_TARGET.1;
    level.objects = vec![o];
    level
}

fn arena(seed: u32) -> Sim {
    let level = room();
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

fn his_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Wolvington))
        .map(|(id, _)| id)
        .expect("Wolvington exists")
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

fn cues(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// The room with Claw `dx` px from him (standing on the floor), the fight
/// flag as given, after `ticks` ticks.
fn with_claw(seed: u32, dx: i32, flag: u8, ticks: usize) -> (Sim, ObjectId) {
    let mut sim = arena(seed);
    let id = his_id(&sim);
    sim.teleport_player(AT.0 + dx, CLAW_Y);
    sim.with_world(|w| w.set_boss_fight(flag));
    idle(&mut sim, ticks);
    (sim, id)
}

/// He is in the fight with Claw `dx` px away (tough), put at Home,
/// standing still and free.
fn in_fight(dx: i32) -> (Sim, ObjectId) {
    let (mut sim, id) = with_claw(3, dx, fight::ON, 3);
    assert_ne!(state_of(&sim, id), Some(EnemyState::Startup));
    home(&mut sim, id);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    (sim, id)
}

/// Puts him at Home, standing still and free, where he stands.
fn home(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        enemy::go_home(w, id);
        let d = enemy::data_mut(w, id).unwrap();
        d.busy = false;
    });
}

/// Runs `f` on him now, with the range mode `mode`, the tick's roll
/// `roll` and the stream at `stream`.
fn now_with(
    sim: &mut Sim,
    id: ObjectId,
    mode: u8,
    roll: i32,
    stream: u32,
    f: fn(&mut logic::World, ObjectId),
) {
    sim.set_rng_state(stream);
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.mode = mode;
        d.roll = roll;
        f(w, id);
    });
}

/// A stream state whose next draw of 0..`n` is in `want`; searched from 0.
fn stream_with(n: u32, want: std::ops::Range<u32>) -> u32 {
    (0..1_000_000u32)
        .find(|&s| want.contains(&Rng::new(s).below(n)))
        .expect("a stream")
}

fn draws_between(a: u32, b: u32) -> u32 {
    let mut r = Rng::new(a);
    for n in 0..=64 {
        if r.state() == b {
            return n;
        }
        r.next_u15();
    }
    panic!("more than 64 draws");
}

/// A synthetic attacker of `type_flags` with `damage`.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits him as a new attack of Claw; returns the random draws
/// the hit made.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId, upper: bool) -> u32 {
    let before = sim.rng().state();
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.uppercut = upper;
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, attacker);
    });
    draws_between(before, sim.rng().state())
}

fn shots(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::WolvingtonShot))
        .map(|(id, _)| id)
        .collect()
}

fn lights(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::WolvingtonLight))
        .map(|(id, _)| id)
        .collect()
}

// ----------------------------------------------------------------------
// Building, waiting for the fight and waking

#[test]
fn he_is_built_as_the_boss_waiting_for_the_fight() {
    let (sim, id) = with_claw(3, -350, fight::NONE, 2);
    let o = obj(&sim, id);
    assert_eq!(o.kind, LogicKind::Game(GameLogic::Wolvington));
    assert!(matches!(o.data, ObjectData::Wolvington(_)), "his own data");
    assert_eq!(sim.boss().current(), Some(id), "the current boss");
    assert_eq!(o.health, 100, "the record's health does not count");
    assert_eq!(o.damage, 5);
    assert_eq!(o.image_set, wolvington::IMAGE_SET);
    assert_eq!(o.move_rect, wolvington::MOVE_RECT);
    assert_eq!(o.hit_rect, wolvington::HIT_RECT);
    assert_eq!(o.attack_rect, wolvington::CONTACT);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    assert!(o.special_flags & special::LIFTABLE != 0, "liftable");
    assert!(o.special_flags & PUNCHABLE != 0);
    assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
    assert_eq!(o.physics_type, ptype::STAND);
    assert_eq!((o.x, o.y), AT, "dropped onto the floor");
    let d = data(&sim, id);
    assert_eq!(d.smarts, 99);
    assert_eq!(d.ranges, wolvington::RANGES);
    assert!(d.may_lift);
    let b = d.human.boss.expect("the boss layer");
    assert_eq!(b.start, PLACED, "placed position kept");
    assert_eq!(b.full_health, 100);
    let carry = enemy::class(GameLogic::Wolvington).unwrap().carry.unwrap();
    assert_eq!(carry.beside, (-20, 20, -15));
    assert_eq!(carry.overhead, (10, -10, -45));
    assert_eq!(carry.move_rect, wolvington::CARRIED_MOVE_RECT);
    // Claw 350 px away: mode 0, and he waits in Startup.
    assert_eq!(d.mode, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert_eq!(obj(&sim, id).anim.key(), Some(wolvington::IDLE_ANIM));
    assert!(logic::built(GameLogic::Wolvington));
    assert!(logic::built(GameLogic::WolvingtonShot));
    assert!(logic::built(GameLogic::WolvingtonLight));
}

#[test]
fn startup_holds_until_the_fight_is_on_then_idle_home_and_a_decision() {
    let (mut sim, id) = with_claw(3, -300, fight::NONE, 2);
    for flag in [fight::NONE, fight::STAGING] {
        sim.with_world(|w| w.set_boss_fight(flag));
        idle(&mut sim, 40);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    }
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    sim.step(TickInput::default());
    // The shared human idle: he says nothing, faces Claw and stops.
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert!(cues(&sim).is_empty(), "{:?}", cues(&sim));
    assert_eq!(data(&sim, id).vx, 0.0);
    assert_eq!(
        data(&sim, id).pace_wait.len,
        enemy::human::IDLE_WAIT_MS,
        "the 2000 ms idle wait"
    );
    // Claw anywhere on the floor is in range: Home on the next tick.
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn his_boxes_reach_40_px_up_and_down_and_cover_the_room() {
    let (sim, id) = with_claw(3, -300, fight::NONE, 2);
    let mut d = data(&sim, id).0.as_ref().clone();
    let m = |d: &mut enemy::EnemyFields, dx: i32, dy: i32| {
        enemy::range_mode(d, AT, (AT.0 + dx, AT.1 + dy), false, 0)
    };
    assert_eq!(m(&mut d, -56, 0), 4);
    assert_eq!(m(&mut d, 55, 0), 4);
    assert_eq!(m(&mut d, 56, 0), 2);
    assert_eq!(m(&mut d, -57, 0), 2);
    assert_eq!(m(&mut d, -200, 0), 2);
    assert_eq!(m(&mut d, -201, 0), 1);
    assert_eq!(m(&mut d, -300, 0), 1);
    assert_eq!(m(&mut d, -301, 0), 0);
    assert_eq!(m(&mut d, -500, 0), 0);
    assert_eq!(m(&mut d, -501, 0), 6);
    assert_eq!(m(&mut d, -30, -40), 4);
    assert_eq!(m(&mut d, -30, -41), 6, "more than 40 px above him");
    assert_eq!(m(&mut d, -30, 39), 4);
    assert_eq!(m(&mut d, -30, 40), 6);
}

/// Claw 60 px above him for `ticks` ticks from his third tick,
/// then back on the floor: whether he left Startup with the fight flag at
/// 0.
fn woke_after_out_of_range(ticks: usize) -> bool {
    let (mut sim, id) = with_claw(3, -169, fight::NONE, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    let at = sim.player().map(|o| (o.x, o.y)).unwrap();
    for _ in 0..ticks {
        sim.with_world(|w| {
            let c = w.claw_id.unwrap();
            let o = w.objects.get_mut(c).unwrap();
            o.x = at.0;
            o.y = AT.1 - 60;
        });
        sim.step(TickInput::default());
    }
    sim.teleport_player(at.0, at.1);
    sim.step(TickInput::default());
    state_of(&sim, id) != Some(EnemyState::Startup)
}

#[test]
fn claw_above_his_band_for_500_ms_wakes_him_without_the_fight() {
    // The skip's usual jump keeps Claw above the band from step 24 to step
    // 50 of the original's run (27 ticks, 468 ms), and back in it on 51:
    // he sleeps on. 28 ticks (486 ms) still leave him asleep; on the 29th
    // tick above (504 ms) he goes idle, as Jump held 13 ticks does.
    assert!(!woke_after_out_of_range(27));
    assert!(!woke_after_out_of_range(28));
    assert!(woke_after_out_of_range(29));
}

#[test]
fn a_dormant_boss_touched_by_claw_stays_asleep() {
    // Claw standing in him while he waits: the touch hurts Claw, he sleeps on.
    let (mut sim, id) = with_claw(3, -5, fight::NONE, 2);
    idle(&mut sim, 30);
    assert!(sim.stats().health < 100, "the touch hurt Claw");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
}

// ----------------------------------------------------------------------
// Home and the walk at Claw

#[test]
fn home_at_range_starts_a_volley_the_sprint_or_the_walk_by_the_roll() {
    for (roll, want) in [
        (0, EnemyState::MagicVolley),
        (99, EnemyState::MagicVolley),
        (100, EnemyState::Sprint),
        (299, EnemyState::Sprint),
        (300, EnemyState::Advance),
        (999, EnemyState::Advance),
        (1000, EnemyState::Home),
        (0x1FFF_FFFF, EnemyState::Home),
    ] {
        for mode in 0..=2 {
            let (mut sim, id) = in_fight(-300);
            now_with(&mut sim, id, mode, roll, 7, wolvington::home_tick);
            assert_eq!(state_of(&sim, id), Some(want), "roll {roll} mode {mode}");
        }
    }
}

#[test]
fn up_close_he_strikes_on_half_the_rolls_and_steps_back_otherwise() {
    // A standing Claw gets the uppercut, with its sound.
    let (mut sim, id) = in_fight(-50);
    now_with(&mut sim, id, 4, 499, 7, wolvington::home_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Uppercut));
    assert_eq!(data(&sim, id).vx, 0.0);
    sim.step(TickInput::default());
    // A crouching one the low thrust.
    let (mut sim, id) = in_fight(-50);
    sim.with_world(|w| {
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().physics_type = ptype::DUCK;
    });
    now_with(&mut sim, id, 3, 0, 7, wolvington::home_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::SwordThrust));
    // Any other roll (every tick between pulses) steps back: one draw.
    let (mut sim, id) = in_fight(-50);
    now_with(&mut sim, id, 4, 0x1FFF_FFFF, 7, wolvington::home_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveBack));
    assert_eq!(draws_between(7, sim.rng().state()), 1);
    // Out of range he paces.
    let (mut sim, id) = in_fight(-300);
    now_with(&mut sim, id, 5, 0, 7, wolvington::home_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Pace));
}

#[test]
fn walking_at_claw_he_runs_from_afar_and_may_volley_or_sprint() {
    let walk = |mode: u8, roll: i32| {
        let (mut sim, id) = in_fight(-250);
        sim.with_world(|w| enemy::states::enter_advance(w, id));
        now_with(&mut sim, id, mode, roll, 7, wolvington::advance_tick);
        (state_of(&sim, id), data(&sim, id).vx)
    };
    assert_eq!(walk(0, 500).0, Some(EnemyState::FastAdvance));
    assert_eq!(walk(1, 9).0, Some(EnemyState::MagicVolley));
    assert_eq!(walk(2, 10).0, Some(EnemyState::Sprint));
    assert_eq!(walk(2, 19).0, Some(EnemyState::Sprint));
    assert_eq!(walk(2, 20), (Some(EnemyState::Advance), -60.0));
    assert_eq!(walk(1, 0x1FFF_FFFF), (Some(EnemyState::Advance), -60.0));
    assert_eq!(walk(3, 0).0, Some(EnemyState::Home));
}

// ----------------------------------------------------------------------
// The volley and the shots

#[test]
fn a_volley_casts_high_on_even_draws_low_on_odd_ones_and_ends_after_a_3() {
    for (draw, high, last) in [
        (0, true, false),
        (1, false, false),
        (2, true, false),
        (3, false, true),
    ] {
        let (mut sim, id) = in_fight(-300);
        let s = stream_with(4, draw..draw + 1);
        sim.set_rng_state(s);
        sim.with_world(|w| wolvington::enter_volley(w, id));
        assert_eq!(state_of(&sim, id), Some(EnemyState::MagicVolley));
        assert_eq!(draws_between(s, sim.rng().state()), 1);
        sim.with_world(|w| wolvington::volley_tick(w, id));
        assert_eq!(
            state_of(&sim, id),
            Some(EnemyState::MagicShot),
            "draw {draw}"
        );
        let want = if high {
            wolvington::HIGH_CAST_ANIM
        } else {
            wolvington::LOW_CAST_ANIM
        };
        assert_eq!(obj(&sim, id).anim.key(), Some(want), "draw {draw}");
        let d = data(&sim, id);
        assert_eq!(d.wolvington.volley_on, !last, "draw {draw}");
        // Facing left (Claw on his left): 65 px ahead, 20 px up or 10 down.
        assert_eq!((d.shot.dx, d.shot.dy), (-65, if high { -20 } else { 10 }));
        assert_eq!(d.shot.vx, -300);
    }
}

#[test]
fn a_cast_releases_its_shot_on_its_16th_tick_and_lasts_34() {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| {
        w.set_boss_fight(fight::ON);
        wolvington::enter_cast(w, id, true);
        let d = enemy::data_mut(w, id).unwrap();
        d.wolvington.volley_on = false;
    });
    assert!(cues(&sim).is_empty());
    let mut released = None;
    let mut home_at = None;
    for t in 1..=40 {
        sim.with_world(|w| wolvington::cast_tick(w, id));
        if released.is_none() && !shots(&sim).is_empty() {
            released = Some(t);
        }
        if state_of(&sim, id) == Some(EnemyState::Home) {
            home_at = Some(t);
            break;
        }
    }
    assert_eq!(released, Some(16));
    assert_eq!(home_at, Some(34));
    assert_eq!(shots(&sim).len(), 1, "one shot per cast");
}

#[test]
fn his_shot_flies_at_300_px_s_hurts_claw_for_5_and_carries_a_light() {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| {
        wolvington::enter_cast(w, id, false);
        let d = enemy::data_mut(w, id).unwrap();
        d.wolvington.volley_on = false;
        for _ in 0..16 {
            wolvington::cast_tick(w, id);
        }
    });
    let shot = shots(&sim)[0];
    let o = obj(&sim, shot).clone();
    assert_eq!(&*o.logic, "WolvingtonBullet");
    assert_eq!((o.x, o.y), (AT.0 - 65, AT.1 + 10), "the low shot");
    assert_eq!(o.z, obj(&sim, id).z - 1);
    assert_eq!(o.damage, 5);
    assert_eq!(o.type_flags, types::ENEMY_SHOT);
    assert_eq!(o.hit_type_flags, 0);
    assert!(o.draw_flags & draw_flags::MIRROR != 0, "flying left");
    assert_eq!(o.move_rect, wolvington::shot::FRAME_BOX.mirrored());
    assert_eq!(o.attack_rect, wolvington::shot::FRAME_BOX.mirrored());
    assert!(o.special_flags & projectile::RESPAWN_PURGE != 0);
    let light = lights(&sim);
    assert_eq!(light.len(), 1, "its light");
    let l = obj(&sim, light[0]);
    assert!(l.draw_flags & draw_flags::HIDDEN != 0);
    assert!(l.flags & flags::NO_COLLIDE != 0);
    assert_eq!(l.z, o.z - 1);
    // It flies 5.4 px a tick (6 and 5), with its light at its position at
    // the start of the tick; two sparkles a tick, two draws each.
    let mut xs = vec![o.x];
    for _ in 0..5 {
        let at = obj(&sim, shot).x;
        let before = sim.rng().state();
        sim.step(TickInput::default());
        xs.push(obj(&sim, shot).x);
        assert!(draws_between(before, sim.rng().state()) >= 4);
        assert_eq!(obj(&sim, lights(&sim)[0]).x, at);
    }
    for w in xs.windows(2) {
        assert!(w[0] - w[1] == 5 || w[0] - w[1] == 6, "{xs:?}");
    }
    assert!((27..=28).contains(&(xs[0] - xs[5])), "{xs:?}");
    // It reaches Claw (300 px away) and is gone with its light: 5 off his
    // health.
    sim.with_world(|w| w.claw.stats.health = 100);
    let mut gone = false;
    for _ in 0..80 {
        sim.step(TickInput::default());
        if sim.objects().get(shot).is_none() {
            gone = true;
            break;
        }
    }
    assert!(gone);
    assert_eq!(sim.stats().health, 95);
    sim.step(TickInput::default());
    assert!(lights(&sim).is_empty(), "the light goes with it");
}

#[test]
fn a_wall_stops_his_shot() {
    // Claw on his right, the shot flies right into the wall 600 px away.
    let (mut sim, id) = in_fight(300);
    sim.with_world(|w| {
        wolvington::enter_cast(w, id, true);
        let d = enemy::data_mut(w, id).unwrap();
        d.wolvington.volley_on = false;
        for _ in 0..16 {
            wolvington::cast_tick(w, id);
        }
    });
    let shot = shots(&sim)[0];
    assert_eq!(obj(&sim, shot).x, AT.0 + 65);
    assert_eq!(obj(&sim, shot).y, AT.1 - 20);
    assert!(obj(&sim, shot).draw_flags & draw_flags::MIRROR == 0);
    // Claw stays clear of it: crouching under the high shot.
    let mut gone = None;
    for t in 0..200 {
        sim.with_world(|w| {
            let c = w.claw_id.unwrap();
            w.objects.get_mut(c).unwrap().y = 2000;
        });
        sim.step(TickInput::default());
        if sim.objects().get(shot).is_none() {
            gone = Some(t);
            break;
        }
    }
    assert!(gone.is_some(), "the wall stopped it");
}

// ----------------------------------------------------------------------
// Strikes and the sprint

/// The ticks of a strike on which his attack box is `rect` (facing left),
/// and the tick he is Home.
fn strike_ticks(start: fn(&mut logic::World, ObjectId), rect: Rect) -> (Vec<u32>, u32) {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| start(w, id));
    let mut live = Vec::new();
    for t in 1..=60 {
        sim.with_world(|w| {
            let o = w.objects.get_mut(id).unwrap();
            o.attack_rect = wolvington::CONTACT;
            let s = enemy::state(w, id).unwrap();
            match s {
                EnemyState::Uppercut => wolvington::uppercut_tick(w, id),
                EnemyState::SwordThrust => wolvington::thrust_tick(w, id),
                _ => {}
            }
        });
        if obj(&sim, id).attack_rect == rect {
            live.push(t);
        }
        if state_of(&sim, id) == Some(EnemyState::Home) {
            return (live, t);
        }
    }
    panic!("the strike ends");
}

#[test]
fn the_uppercut_is_live_on_its_16th_to_27th_ticks_and_home_on_the_37th() {
    let (live, home) = strike_ticks(wolvington::enter_uppercut, wolvington::UPPERCUT);
    assert_eq!(live, (16..=27).collect::<Vec<_>>());
    assert_eq!(home, 37);
}

#[test]
fn the_thrust_is_live_on_its_13th_to_22nd_ticks_and_home_on_the_28th() {
    let (live, home) = strike_ticks(wolvington::enter_thrust, wolvington::THRUST);
    assert_eq!(live, (13..=22).collect::<Vec<_>>());
    assert_eq!(home, 28);
}

#[test]
fn the_sprint_runs_at_480_px_s_and_uppercuts_within_56_px() {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| wolvington::enter_sprint(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Sprint));
    let d = data(&sim, id);
    assert_eq!(d.prev, EnemyState::FastAdvance);
    assert_eq!(d.vx, -120.0, "the run's entry speed");
    assert_eq!(
        obj(&sim, id).anim.key(),
        Some(wolvington::FAST_ADVANCE_ANIM)
    );
    now_with(&mut sim, id, 1, 0, 7, wolvington::sprint_tick);
    assert_eq!(data(&sim, id).vx, -480.0);
    now_with(&mut sim, id, 6, 0, 7, wolvington::sprint_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Sprint), "off screen");
    now_with(&mut sim, id, 3, 0, 7, wolvington::sprint_tick);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Uppercut));
    assert_eq!(data(&sim, id).vx, 0.0);
}

// ----------------------------------------------------------------------
// Going Home, hits, blocks and the dodge

#[test]
fn going_home_counters_after_a_flinch_volleys_after_a_throw() {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::HitLow);
        wolvington::go_home(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Sprint));
    let d = data(&sim, id);
    assert_eq!(d.cooldown.len, wolvington::COUNTER_COOLDOWN_MS);
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Bounce);
        wolvington::go_home(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::MagicVolley));
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Uppercut);
        let d = enemy::data_mut(w, id).unwrap();
        d.vx = 50.0;
        d.vy = 20.0;
        wolvington::go_home(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    let d = data(&sim, id);
    assert_eq!((d.vx, d.vy), (0.0, 20.0), "he stops across only");
}

#[test]
fn claws_blows_cost_him_their_damage_and_end_a_flinch_in_the_sprint() {
    let (mut sim, id) = in_fight(-40);
    let blow = attacker(&mut sim, types::PLAYER, 5);
    // A blow: 5 off, the burst's three draws, a low flinch, 300 ms.
    assert_eq!(hit(&mut sim, id, blow, false), 3);
    assert_eq!(obj(&sim, id).health, 95);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(data(&sim, id).cooldown.len, wolvington::HIT_COOLDOWN_MS);
    // A blow during the flinch, once the cooldown is over: no damage, the
    // sprint.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
    assert_eq!(hit(&mut sim, id, blow, false), 0);
    assert_eq!(obj(&sim, id).health, 95);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Sprint));
    // An uppercut: a high flinch, one more draw for its sound.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
    home(&mut sim, id);
    assert_eq!(hit(&mut sim, id, blow, true), 4);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitHigh));
}

#[test]
fn shots_magic_and_thrown_objects_are_destroyed_and_he_blocks() {
    for kind in [types::PISTOL_SHOT, enemy::attacker::MAGIC, types::THROWN] {
        let (mut sim, id) = in_fight(-200);
        let a = attacker(&mut sim, kind, 10);
        assert_eq!(hit(&mut sim, id, a, false), 0);
        assert_eq!(obj(&sim, id).health, 100, "unhurt");
        assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
        assert!(
            obj(&sim, a).flags & flags::DESTROY != 0,
            "{kind:#x} destroyed"
        );
    }
    // The level's own shots and explosions do nothing.
    let (mut sim, id) = in_fight(-200);
    let a = attacker(&mut sim, types::LEVEL_SHOT, 10);
    hit(&mut sim, id, a, false);
    assert_eq!(obj(&sim, id).health, 100);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

/// Claw attacking with `kind`, facing him from `dx` px, with the range
/// mode `mode`: the draws of his pre-tick step and whether the hop back
/// was scheduled.
fn dodge(kind: i32, dx: i32, mode: u8) -> (u32, bool) {
    let (mut sim, id) = in_fight(dx);
    sim.set_rng_state(stream_with(100, 0..99));
    let before = sim.rng().state();
    let scheduled = sim.with_world(|w| {
        w.claw.attack.kind = kind;
        w.claw.facing_right = dx < 0;
        let d = enemy::data_mut(w, id).unwrap();
        d.mode = mode;
        d.claw_right = dx > 0;
        d.deferred = None;
        wolvington::pre_tick(w, id);
        enemy::data(w, id).unwrap().deferred.is_some()
    });
    (draws_between(before, sim.rng().state()), scheduled)
}

#[test]
fn only_a_sword_swing_from_within_56_px_draws_for_the_hop_back() {
    // The standing, crouching and air swings draw one of 0..100.
    for kind in [
        attack_kind::SWIPE,
        attack_kind::DUCK_SWIPE,
        attack_kind::JUMP_SWIPE,
    ] {
        for mode in [3, 4] {
            assert_eq!(dodge(kind, -40, mode).0, 1, "{kind} {mode}");
        }
    }
    // Punches, kicks, the pistol and swings from farther off draw nothing.
    for (kind, mode) in [
        (attack_kind::PUNCH, 4),
        (attack_kind::KICK, 3),
        (attack_kind::PISTOL, 4),
        (attack_kind::SWIPE, 2),
    ] {
        assert_eq!(dodge(kind, -40, mode), (0, false), "{kind} {mode}");
    }
    // A swing facing away draws nothing.
    let (mut sim, id) = in_fight(-40);
    sim.with_world(|w| {
        w.claw.attack.kind = attack_kind::SWIPE;
        w.claw.facing_right = false;
        let d = enemy::data_mut(w, id).unwrap();
        d.mode = 4;
        d.claw_right = false;
    });
    let before = sim.rng().state();
    sim.with_world(|w| wolvington::pre_tick(w, id));
    assert_eq!(draws_between(before, sim.rng().state()), 0);
}

#[test]
fn the_hop_back_is_scheduled_on_a_draw_below_his_smarts_when_free() {
    let (mut sim, id) = in_fight(-40);
    let below = stream_with(100, 0..99);
    let at = stream_with(100, 99..100);
    for (stream, busy, want) in [
        (below, false, true),
        (at, false, false),
        (below, true, false),
    ] {
        sim.set_rng_state(stream);
        let scheduled = sim.with_world(|w| {
            w.claw.attack.kind = attack_kind::SWIPE;
            w.claw.facing_right = true;
            let d = enemy::data_mut(w, id).unwrap();
            d.mode = 3;
            d.claw_right = false;
            d.busy = busy;
            d.deferred = None;
            wolvington::pre_tick(w, id);
            enemy::data(w, id).unwrap().deferred.is_some()
        });
        assert_eq!(scheduled, want, "busy {busy}");
    }
    // Run in a tick, he hops back 100 px, safe, with his jump sound.
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.busy = false;
    });
    let before_x = obj(&sim, id).x;
    let mut hopped = false;
    for _ in 0..3 {
        sim.set_rng_state(below);
        sim.with_world(|w| {
            w.claw.attack.kind = attack_kind::SWIPE;
            w.claw.facing_right = true;
        });
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::JumpBack) {
            hopped = true;
            break;
        }
    }
    assert!(hopped);
    assert!(obj(&sim, id).flags & flags::SAFE != 0);
    assert!(cues(&sim).contains(&wolvington::JUMP_SOUND.to_string()));
    let mut landed = None;
    for t in 0..40 {
        sim.with_world(|w| w.claw.attack.kind = attack_kind::NONE);
        sim.step(TickInput::default());
        if state_of(&sim, id) != Some(EnemyState::JumpBack) {
            landed = Some(t);
            break;
        }
    }
    assert!(landed.is_some());
    // Aimed 100 px back, he lands a little short of it.
    let moved = obj(&sim, id).x - before_x;
    assert!((95..=100).contains(&moved), "about 100 px back: {moved}");
}

// ----------------------------------------------------------------------
// The standing fraction

#[test]
fn standing_he_sinks_a_third_of_a_pixel_a_tick_once_he_has_flown() {
    let g = enemy::GRAVITY;
    let mut f = 0.0;
    let mut seen = Vec::new();
    for _ in 0..5 {
        f = wolvington::next_stand_fraction(f, g);
        seen.push((f * 1000.0).round() as i32);
    }
    assert_eq!(seen, vec![324, 648, 972, 0, 324]);
    assert_eq!(
        wolvington::next_stand_fraction(wolvington::LANDED_FRACTION, g),
        0.0
    );
    // Built on his floor he stands on his pixel; after a hop back he lands
    // a step short of it.
    let (mut sim, id) = in_fight(-300);
    idle(&mut sim, 5);
    let d = data(&sim, id);
    assert!(!d.wolvington.falls);
    assert_eq!(d.wolvington.stand_y, 0.0);
}

// ----------------------------------------------------------------------
// Lifting, death and the reset

#[test]
fn claw_can_lift_him_from_startup_and_a_throw_he_survives_ends_in_a_volley() {
    let (mut sim, id) = with_claw(3, -40, fight::NONE, 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert!(obj(&sim, id).special_flags & special::LIFTABLE != 0);
    sim.with_world(|w| {
        let claw = w.claw_id.unwrap();
        logic::lifted(w, id, claw);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
    sim.with_world(|w| {
        enemy::set_state(w, id, EnemyState::Flailing);
        logic::thrown(w, id, 200, -300);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Thrown));
    let mut volley = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::MagicVolley)
            || state_of(&sim, id) == Some(EnemyState::MagicShot)
        {
            volley = true;
            break;
        }
    }
    assert!(volley, "the throw ends in a volley");
    assert_eq!(obj(&sim, id).health, 96, "the rest costs him 4");
}

#[test]
fn his_death_by_claw_is_reported_once_and_releases_the_gem() {
    let (mut sim, id) = in_fight(-40);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 5);
    let blow = attacker(&mut sim, types::PLAYER, 5);
    hit(&mut sim, id, blow, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), Some(id));
    assert!(
        sim.objects()
            .all()
            .any(|(_, o)| &*o.logic == pickup::END_GEM),
        "the gem"
    );
    // Gone off screen, he stops being the current boss.
    let mut gone = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if sim.objects().get(id).is_none() {
            gone = true;
            break;
        }
    }
    assert!(gone);
    assert_eq!(sim.boss().current(), None);
}

#[test]
fn the_reset_puts_him_back_on_his_floor_with_full_health_in_his_state() {
    let (mut sim, id) = in_fight(-300);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = 2300;
        o.health = 35;
        enemy::set_state(w, id, EnemyState::Advance);
        wolvington::reset(w, id);
    });
    let o = obj(&sim, id);
    assert_eq!((o.x, o.y, o.health), (AT.0, AT.1, 100));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Advance));
    assert!(data(&sim, id).wolvington.falls);
}
