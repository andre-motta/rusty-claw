//! Marrow's parrot on a synthetic room at the tenth level's place, with
//! the boss fight flag set directly: its climb and patrol, its dive and
//! swoop that only the fight allows, the hits that never hurt it and send
//! it home after the third, its perch that closes the bridge, the carry of
//! Marrow across the room, and its reset after a death of Claw's.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, marrow, parrot};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The perch, as placed.
const PERCH: (i32, i32) = (41049, 7141);
/// Its box: across and up and down.
const BOX: ((i32, i32), (i32, i32)) = ((40488, 41128), (7029, 7287));
/// Marrow as placed.
const MARROW_AT: (i32, i32) = (41063, 7255);
/// The top of the room's floor.
const FLOOR: i32 = 7296;

fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

/// The room, with Marrow when `with_marrow`, every object awake.
fn room(with_marrow: bool) -> Sim {
    let mut rows = vec![".".repeat(660); 114];
    rows.push("#".repeat(660));
    rows.push("#".repeat(660));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (40480, 7200));
    level.number = 10;
    for a in [
        anim(
            parrot::FLY_ANIM,
            &[(1, 125), (2, 100), (3, 125), (4, 100)],
            false,
        ),
        anim(parrot::HIT_ANIM, &[(10, 50), (11, 200), (12, 50)], true),
        anim(parrot::IDLE_ANIM, &[(20, 480), (21, 482)], true),
        anim(parrot::STRIKE_ANIM, &[(30, 0)], true),
        anim(marrow::WHISTLE_ANIM, &[(40, 150), (41, 100), (42, 0)], true),
    ] {
        level.animations.insert(a.key.clone(), a);
    }
    let mut p = ObjectSpec::at("Parrot", PERCH.0, PERCH.1, 0);
    p.image_set = parrot::IMAGE_SET.into();
    p.record[enemy::field::X_MIN] = BOX.0.0;
    p.record[enemy::field::X_MAX] = BOX.0.1;
    p.record[enemy::field::Y_MIN] = BOX.1.0;
    p.record[enemy::field::Y_MAX] = BOX.1.1;
    level.objects = vec![p];
    if with_marrow {
        let mut m = ObjectSpec::at("Marrow", MARROW_AT.0, MARROW_AT.1, 0);
        m.image_set = marrow::IMAGE_SET.into();
        level.objects.push(m);
    }
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    sim.teleport_player(40600, FLOOR - 61);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .expect("the object")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn at(sim: &Sim, id: ObjectId) -> (i32, i32) {
    let o = obj(sim, id);
    (o.x, o.y)
}

/// A new attack of Claw's reaching the bird.
fn hit(sim: &mut Sim, id: ObjectId) {
    let mut a = Object::new(0, 0, 0, 0, "TestBlade".into(), LogicKind::Inert);
    a.type_flags = types::PLAYER;
    a.damage = 5;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    let blade = sim.spawn(a);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        let p = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(blade) {
            (o.x, o.y) = p;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, blade);
    });
}

fn hits(sim: &Sim, id: ObjectId) -> i32 {
    match &obj(sim, id).data {
        ObjectData::Parrot(p) => p.hits,
        _ => panic!("the parrot's data"),
    }
}

#[test]
fn it_is_built_harmless_and_climbs_to_its_line_then_patrols_it() {
    let mut sim = room(false);
    let id = find(&sim, "Parrot");
    let o = obj(&sim, id);
    assert_eq!(
        (o.health, o.damage, o.z),
        (parrot::HEALTH, parrot::DAMAGE, parrot::Z)
    );
    assert_eq!(o.physics_type, ptype::DIRECT, "no tile collision");
    assert_eq!(o.hit_rect, parrot::HIT_RECT);
    assert!(!o.attack_rect.is_set(), "harmless");
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
    // Straight up at 300 px/s to its line, in 21 ticks.
    let mut climb = 0;
    while at(&sim, id).1 != BOX.1.0 {
        sim.step(TickInput::default());
        climb += 1;
        assert_eq!(at(&sim, id).0, PERCH.0);
        assert!(climb < 40);
    }
    assert_eq!(climb, 21);
    // Then left along the line (it is right of the room's centre), and
    // each leg of 640 px takes 119 ticks.
    let mut ends = Vec::new();
    for k in 0..400 {
        sim.step(TickInput::default());
        let x = at(&sim, id).0;
        if x == BOX.0.0 || x == BOX.0.1 {
            ends.push((k, x));
        }
        assert_eq!(at(&sim, id).1, BOX.1.0, "on its line");
    }
    assert_eq!(ends[0].1, BOX.0.0);
    assert_eq!(ends[1].0 - ends[0].0, parrot::PATROL_TICKS as usize);
    assert_eq!(
        state_of(&sim, id),
        Some(EnemyState::FlyAbout),
        "no fight: no dive"
    );
}

#[test]
fn it_dives_only_once_the_fight_is_on_and_marrow_is_there() {
    for (flag, with_marrow) in [
        (fight::NONE, true),
        (fight::STAGING, true),
        (fight::ON, false),
    ] {
        let mut sim = room(with_marrow);
        let id = find(&sim, "Parrot");
        idle(&mut sim, 2);
        sim.with_world(|w| {
            w.set_boss_fight(flag);
            parrot::enter_strike_setup(w, id);
        });
        assert_eq!(
            state_of(&sim, id),
            Some(EnemyState::FlyAbout),
            "{flag} {with_marrow}"
        );
    }
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    idle(&mut sim, 30);
    sim.with_world(|w| {
        w.set_boss_fight(fight::ON);
        parrot::enter_strike_setup(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::StrikeSetup));
    let ObjectData::Parrot(p) = &obj(&sim, id).data else {
        panic!("its data");
    };
    // Claw left of the centre: the dive point 200 px inside the right end.
    assert_eq!(p.target.0, parrot::DIVE_X.1);
    let claw_y = sim.player().expect("Claw").y;
    assert!((claw_y - 30..=claw_y).contains(&p.target.1));
    // The swoop below the dive point: the beak out, hittable; it flies on
    // to the other end of its line and climbs back, harmless.
    let mut swoop = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        match state_of(&sim, id) {
            Some(EnemyState::Strike) => {
                swoop = true;
                let o = obj(&sim, id);
                assert!(o.attack_rect.is_set());
                assert_eq!(o.flags & flags::NO_COLLIDE, 0);
            }
            Some(EnemyState::FlyAbout) if swoop => break,
            _ => {}
        }
    }
    assert!(swoop);
    assert_eq!(at(&sim, id).0, BOX.0.0, "across to the far end");
    assert!(!obj(&sim, id).attack_rect.is_set());
}

#[test]
fn hits_never_hurt_it_and_the_third_sends_it_to_its_perch() {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    idle(&mut sim, 30);
    hit(&mut sim, id);
    assert_eq!(hits(&sim, id), 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::GenericHit));
    assert!(
        obj(&sim, id).flags & flags::NO_COLLIDE != 0,
        "not hittable again"
    );
    assert_eq!(obj(&sim, id).health, parrot::HEALTH);
    // The same attack again counts nothing.
    sim.with_world(|w| w.claw.attack.serial -= 1);
    hit(&mut sim, id);
    assert_eq!(hits(&sim, id), 1);
    // The flinch ends: it flies on, showing the flinch once more.
    let mut k = 0;
    while state_of(&sim, id) == Some(EnemyState::GenericHit) {
        sim.step(TickInput::default());
        k += 1;
        assert!(k < 40);
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
    assert_eq!(obj(&sim, id).anim.key(), Some(parrot::HIT_ANIM));
    // Two more: home to the perch.
    for _ in 0..2 {
        sim.with_world(|w| w.objects.get_mut(id).unwrap().flags &= !flags::NO_COLLIDE);
        hit(&mut sim, id);
        idle(&mut sim, 25);
    }
    assert_eq!(hits(&sim, id), parrot::HITS_HOME);
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyToIdle));
    for _ in 0..200 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Idle) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!(at(&sim, id), PERCH, "settled exactly on its perch");
    assert_eq!(hits(&sim, id), 0);
    assert!(obj(&sim, id).flags & flags::NO_COLLIDE != 0);
    assert!(
        !sim.marrow_link().expect("link").open_request,
        "the bridge closes"
    );
}

#[test]
fn perched_it_answers_the_whistle_and_carries_marrow_away_from_claw() {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    let m = find(&sim, "Marrow");
    idle(&mut sim, 5);
    sim.with_world(|w| {
        parrot::go_idle(w, id);
        w.objects.get_mut(m).unwrap().health = 75;
        w.marrow_link_mut().unwrap().phase_health = 100;
    });
    // He whistles; the call at the whistle's end; the bird flies to him
    // and lifts him, the slabs asked to open.
    let mut carried_from = None;
    for _ in 0..100 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::CarryMarrow) {
            carried_from = Some(at(&sim, m));
            break;
        }
    }
    assert!(carried_from.is_some(), "the carry starts");
    assert!(sim.marrow_link().expect("link").open_request);
    assert_eq!(obj(&sim, m).physics_type, ptype::DIRECT);
    // Claw left of the centre: to the right ledge (41063), then dropped
    // onto the floor, Home, the phase health his health.
    let mut ticks = 0;
    while state_of(&sim, id) == Some(EnemyState::CarryMarrow) {
        sim.step(TickInput::default());
        ticks += 1;
        assert!(ticks < 300);
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
    let l = *sim.marrow_link().expect("link");
    assert_eq!((l.parrot_call, l.phase_health), (false, 75));
    let mo = obj(&sim, m);
    assert_eq!(mo.physics_type, ptype::STAND);
    assert_eq!(mo.y + mo.move_rect.bottom, FLOOR - 1, "on the floor");
    assert!(mo.x > marrow::ROOM_CENTRE_X);
    assert_eq!(mo.flags & flags::SAFE, 0);
}

#[test]
fn a_death_of_claws_puts_it_back_on_its_perch_flying() {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    idle(&mut sim, 60);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().flags &= !flags::NO_COLLIDE);
    hit(&mut sim, id);
    assert_ne!(at(&sim, id), PERCH);
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.end_life();
    for _ in 0..400 {
        sim.step(TickInput::default());
        if sim.claw().stats.attempt == 1 {
            break;
        }
    }
    sim.step(TickInput::default());
    assert_eq!(hits(&sim, id), 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
    assert_eq!(at(&sim, id).0, PERCH.0, "climbing from its perch");
    assert!(at(&sim, id).1 < PERCH.1 && at(&sim, id).1 > PERCH.1 - 12);
}

/// Steps until the parrot is in `s`, at most `limit` ticks.
fn until(sim: &mut Sim, id: ObjectId, s: EnemyState, limit: usize) {
    for _ in 0..limit {
        if state_of(sim, id) == Some(s) {
            return;
        }
        sim.step(TickInput::default());
    }
    panic!("never {s:?}");
}

/// A whistle answered from the perch with Claw standing at `whistle` while
/// Marrow whistles, at `flight` from the start of the bird's flight to
/// him (so on the carry's first step), and at `after` once the carry is
/// under way: where the carry is aimed across, and where Marrow is dropped.
fn carry(whistle: i32, flight: i32, after: i32) -> (i32, i32) {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    let m = find(&sim, "Marrow");
    sim.teleport_player(whistle, FLOOR - 61);
    idle(&mut sim, 5);
    sim.with_world(|w| {
        parrot::go_idle(w, id);
        w.objects.get_mut(m).unwrap().health = 75;
        w.marrow_link_mut().unwrap().phase_health = 100;
    });
    until(&mut sim, id, EnemyState::FlyToMarrow, 100);
    sim.teleport_player(flight, FLOOR - 61);
    until(&mut sim, id, EnemyState::CarryMarrow, 100);
    assert_eq!(sim.player().expect("Claw").x, flight);
    let ObjectData::Parrot(p) = &obj(&sim, id).data else {
        panic!("its data");
    };
    let aim = p.target.0;
    sim.teleport_player(after, FLOOR - 61);
    for _ in 0..300 {
        sim.step(TickInput::default());
        if state_of(&sim, id) != Some(EnemyState::CarryMarrow) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout), "let go");
    assert_eq!(obj(&sim, m).physics_type, ptype::STAND, "dropped");
    (aim, obj(&sim, m).x)
}

/// The rule the bridge trick rests on: the carry goes to the ledge away
/// from Claw's x on the carry's first step, 40808 (the room's centre) and
/// over counting as Marrow's half, so a Claw at 40808 or right of it then
/// sends Marrow to the left ledge, his own side; where Claw was during the
/// whistle, or goes once the carry is under way, changes nothing.
#[test]
fn the_carry_is_aimed_once_from_claws_x_on_its_first_step() {
    let (left, right) = (parrot::CARRY_X.1, parrot::CARRY_X.0);
    assert_eq!(marrow::ROOM_CENTRE_X, 40808);
    let (aim, x) = carry(40808, 40808, 40808);
    assert_eq!(aim, left, "at the centre: to the left ledge");
    assert!(x < marrow::ROOM_CENTRE_X, "dropped on the left, at {x}");
    let (aim, x) = carry(40807, 40807, 40807);
    assert_eq!(aim, right, "a pixel left of it: to the right ledge");
    assert!(x > marrow::ROOM_CENTRE_X, "dropped on the right, at {x}");
    assert_eq!(carry(40600, 40820, 40820).0, left, "the flight's x counts");
    assert_eq!(carry(40820, 40600, 40600).0, right, "not the whistle's");
    assert_eq!(carry(40600, 40820, 40600).0, left, "decided once");
}

/// Away from its perch it never answers: a whistle while it patrols goes
/// unanswered, and Marrow drops back after his 5000 ms (278 ticks of
/// 18 ms) with the phase health his health, however far below the old
/// one it is.
#[test]
fn a_whistle_while_it_patrols_goes_unanswered_into_the_timeout() {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    let m = find(&sim, "Marrow");
    idle(&mut sim, 30);
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
    sim.with_world(|w| {
        w.objects.get_mut(m).unwrap().health = 49;
        w.marrow_link_mut().unwrap().phase_health = 75;
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, m), Some(EnemyState::WaitForParrot));
    let mut called = false;
    let mut drop = None;
    for k in 1..400 {
        sim.step(TickInput::default());
        called |= sim.marrow_link().expect("link").parrot_call;
        assert!(
            !matches!(
                state_of(&sim, id),
                Some(EnemyState::FlyToMarrow | EnemyState::CarryMarrow)
            ),
            "answered on tick {k}"
        );
        if obj(&sim, m).flags & flags::SAFE == 0 {
            drop = Some(k);
            break;
        }
    }
    assert!(called, "he calls it");
    assert_eq!(drop, Some(278));
    assert_eq!(
        (marrow::WAIT_FOR_PARROT_MS as usize).div_ceil(18),
        278,
        "5000 ms"
    );
    let l = *sim.marrow_link().expect("link");
    assert_eq!((l.phase_health, l.parrot_call), (49, false));
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlyAbout));
}

/// Only the third hit sends it to its perch: with two, the fight on and
/// Claw on the floor, it patrols and swoops for a minute and a half and
/// never perches, so the slabs are never asked to close.
#[test]
fn without_a_third_hit_it_never_perches() {
    let mut sim = room(true);
    let id = find(&sim, "Parrot");
    idle(&mut sim, 30);
    for _ in 0..2 {
        sim.with_world(|w| w.objects.get_mut(id).unwrap().flags &= !flags::NO_COLLIDE);
        hit(&mut sim, id);
        idle(&mut sim, 25);
    }
    assert_eq!(hits(&sim, id), 2);
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    let mut dives = 0;
    for _ in 0..5000 {
        let before = state_of(&sim, id);
        sim.step(TickInput::default());
        let now = state_of(&sim, id);
        if now == Some(EnemyState::StrikeSetup) && before != now {
            dives += 1;
        }
        assert!(
            !matches!(now, Some(EnemyState::Idle | EnemyState::FlyToIdle)),
            "it perches"
        );
        assert!(sim.marrow_link().expect("link").open_request);
    }
    assert!(dives > 0, "it swoops meanwhile");
    assert_eq!(hits(&sim, id), 2);
}
