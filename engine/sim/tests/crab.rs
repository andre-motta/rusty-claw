//! The hermit crabs, the crab nests and the crab bombs on synthetic
//! levels: how a crab is built, its crouch, hide, snip and throw, its
//! shell, its death and its loot; the nest's trigger and hatch; the
//! bomb's arc, bounces, fuse and blast.

mod common;

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::crab::{self, Leftovers, bomb, nest};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, GameLogic};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The floor's top: a crab stands with its feet on 959, at y 936.
const FLOOR: i32 = 960;
/// A crab's y standing on the floor.
const CRAB_Y: i32 = FLOOR - 1 - crab::MOVE_RECT.bottom;

fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

/// The crabs', the nest's and the bomb's animations, with the pictures
/// and lengths of their level's.
fn crab_anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_HERMITCRAB_{s}");
    let mut explode = Animation::once(
        bomb::EXPLODE_ANIM,
        &[(7, 50), (8, 50), (9, 50), (10, 50), (11, 50)],
    );
    explode.frames[1].event = 1;
    vec![
        anim(&k("CROUCH4"), &[(801, 50), (800, 200)], true),
        anim(
            &k("HIDE"),
            &[(802, 750), (801, 500), (802, 200), (801, 750)],
            false,
        ),
        anim(
            &k("STRIKE1"),
            &[(200, 20), (201, 20), (202, 20), (203, 20), (204, 100)],
            true,
        ),
        anim(&k("FASTADVANCE"), &[(1, 110), (2, 110)], false),
        anim(&k("FALL"), &[(200, 100)], false),
        anim(&k("KILLFALL"), &[(950, 0)], true),
        anim(
            nest::HATCH_ANIM,
            &[(1, 50), (2, 50), (3, 50), (4, 50), (5, 50), (6, 0)],
            true,
        ),
        anim(bomb::FALL_ANIM, &[(1, 100)], false),
        Arc::new(explode),
    ]
}

/// A long flat floor (top at y 960) with Claw starting at `claw`.
fn beach(claw: (i32, i32)) -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(120); 15];
    rows.push("#".repeat(120));
    rows.push("#".repeat(120));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), claw);
    for a in crab_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    level
}

/// A crab record at (x, y) on the patrol `bounds` (none for (0, 0)).
fn hermit(x: i32, y: i32, bounds: (i32, i32)) -> ObjectSpec {
    let mut s = ObjectSpec::at("HermitCrab", x, y, 0);
    s.image_set = crab::IMAGE_SET.into();
    s.frame = 1;
    s.record[field::X_MIN] = bounds.0;
    s.record[field::X_MAX] = bounds.1;
    s
}

/// A game of `level` with `records` placed, Claw landed; the built crabs
/// and nests, in record order.
fn game(mut level: SimLevel, records: &[ObjectSpec]) -> (Sim, Vec<ObjectId>) {
    level.objects.extend(records.iter().cloned());
    let mut sim = Sim::new(level, SimConfig::default());
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    let mut ids: Vec<(i32, ObjectId)> = sim
        .objects()
        .all()
        .filter(|(_, o)| matches!(&*o.logic, "HermitCrab" | "CrabNest"))
        .filter(|(_, o)| o.record.is_some())
        .map(|(id, o)| (o.record.as_ref().unwrap()[5], id))
        .collect();
    ids.sort();
    (sim, ids.into_iter().map(|(_, id)| id).collect())
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object")
}

fn state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn cued(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, AudioCue::Play(s) if s.key == key))
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .expect("Claw")
}

/// An object of kind `kind` at (x, y) to hit with.
fn attacker(sim: &mut Sim, kind: u32, at: (i32, i32)) -> ObjectId {
    sim.with_world(|w| {
        let mut o = logic::new_object("TestBlow", at.0, at.1, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = kind;
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    })
}

/// Steps until `f` holds, at most `n` ticks; the ticks it took.
fn until(sim: &mut Sim, n: usize, f: impl Fn(&Sim) -> bool) -> Option<usize> {
    for k in 1..=n {
        sim.step(TickInput::default());
        if f(sim) {
            return Some(k);
        }
    }
    None
}

#[test]
fn the_crab_families_run() {
    for g in [
        GameLogic::HermitCrab,
        GameLogic::CrabNest,
        GameLogic::CrabBomb,
    ] {
        assert!(logic::built(g), "{g:?}");
    }
    for name in ["HermitCrab", "CrabNest", "CrabBomb"] {
        assert!(logic::runs(name).is_some(), "{name}");
    }
}

#[test]
fn a_crab_is_built_on_its_floor_with_its_boxes() {
    let (sim, ids) = game(beach((1500, 800)), &[hermit(1000, 900, (900, 1200))]);
    let o = obj(&sim, ids[0]);
    assert_eq!((o.x, o.y), (1000, CRAB_Y));
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.hit_type_flags, crab::HIT_BY);
    assert_eq!((o.damage, o.health), (crab::DAMAGE, 0));
    // Its boxes as it faces (it has regrouped facing Claw since).
    assert!(enemy::faces_right(o));
    assert_eq!(o.move_rect, enemy::facing_rect(o, crab::MOVE_RECT));
    assert_eq!(o.hit_rect, enemy::facing_rect(o, crab::HIT_RECT));
    assert_eq!(o.attack_rect, enemy::facing_rect(o, crab::CONTACT_RECT));
    let d = enemy::data_of(o).expect("enemy data");
    assert_eq!(d.ranges, crab::RANGES);
    assert_eq!(d.bounds, (900, 1200));
    assert_eq!(d.z_base, crab::Z_BASE);
    assert!(d.ledge_guard && d.z_sort && !d.stationary);
    let f = crab::fields_of(o).expect("crab data");
    assert!(!f.bomber);
    assert_eq!(f.strike_sound, crab::SNIP_SOUND);
    assert_eq!(f.leftovers, Leftovers::default());
    assert_eq!(o.special_flags & claw_sim::object::special::LIFTABLE, 0);
}

#[test]
fn a_crab_placed_beside_its_patrol_starts_on_its_limit() {
    let (sim, ids) = game(
        beach((1100, 800)),
        &[hermit(300, 900, (400, 700)), hermit(900, 900, (400, 700))],
    );
    assert_eq!((obj(&sim, ids[0]).x, obj(&sim, ids[0]).y), (400, CRAB_Y));
    assert_eq!((obj(&sim, ids[1]).x, obj(&sim, ids[1]).y), (700, CRAB_Y));
}

#[test]
fn a_record_bomber_never_walks_and_throws_with_the_bomb_sound() {
    let mut s = hermit(1000, 900, (0, 0));
    s.record[field::USER_0] = 1;
    let (sim, ids) = game(beach((1500, 800)), &[s]);
    let o = obj(&sim, ids[0]);
    assert!(o.image_set.ends_with(crab::BOMBER_IMAGE_SET));
    let f = crab::fields_of(o).unwrap();
    assert!(f.bomber);
    assert_eq!(f.strike_sound, crab::THROW_SOUND);
    assert!(enemy::data_of(o).unwrap().stationary);
}

#[test]
fn leftovers_give_the_second_box_and_the_first_facing() {
    let (mut sim, ids) = game(beach((1500, 800)), &[hermit(1000, 900, (0, 0))]);
    let id = ids[0];
    sim.with_world(|w| {
        crab::set_leftovers(
            w,
            id,
            Leftovers {
                side: crab::SIDE_RIGHT,
                box1: (578, 432),
            },
        )
    });
    let o = obj(&sim, id);
    assert!(enemy::faces_right(o));
    assert_eq!(o.hit_rect, crab::HIT_RECT.mirrored());
    assert_eq!(enemy::data_of(o).unwrap().ranges[1], (578, 432));
    // A large second box keeps Claw in range mode 1 far beyond the
    // others: 500 px away and 400 px up.
    let mut d = enemy::data_of(o).unwrap().0.as_ref().clone();
    let m = enemy::range_mode(&mut d, (1000, 936), (1500, 536), true, 0);
    assert_eq!(m, 1);
    d.ranges[1] = (0, 0);
    assert_ne!(
        enemy::range_mode(&mut d, (1000, 936), (1500, 536), true, 0),
        1
    );
}

/// A crab on the floor with Claw standing `dx` px from it.
fn crab_and_claw(dx: i32) -> (Sim, ObjectId) {
    let claw = (2000 + dx, 900);
    let (sim, ids) = game(beach(claw), &[hermit(2000, 900, (1700, 2300))]);
    (sim, ids[0])
}

#[test]
fn close_to_claw_it_crouches_for_16_ticks_and_attacks_by_its_chances() {
    let (mut sim, id) = crab_and_claw(150);
    let mut defenses = 0;
    for _ in 0..3000 {
        let before = state(&sim, id);
        sim.step(TickInput::default());
        let now = state(&sim, id);
        if now == Some(EnemyState::Defense) && before != now {
            defenses += 1;
            assert_eq!(obj(&sim, id).anim.key(), Some(crab::DEFENSE_ANIM));
            // The crouch decides on its 16th tick.
            let n = until(&mut sim, 40, |s| state(s, id) != Some(EnemyState::Defense));
            assert!(matches!(n, Some(16..)), "{n:?}");
        }
        // Close by it never hides from a regroup, and it never throws.
        assert_ne!(now, Some(EnemyState::Launch));
    }
    assert!(defenses > 5);
}

#[test]
fn far_from_claw_it_only_hides_and_crouches() {
    let (mut sim, id) = crab_and_claw(600);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..2000 {
        sim.step(TickInput::default());
        if let Some(s) = state(&sim, id) {
            seen.insert(s.name());
        }
    }
    assert!(
        seen.contains("Hide") && seen.contains("Defense"),
        "{seen:?}"
    );
    for s in ["Attack", "MovingAttack", "Snip", "MoveBack", "Launch"] {
        assert!(!seen.contains(s), "{s}");
    }
    assert_eq!(obj(&sim, id).x, 2000, "it never moves");
}

#[test]
fn a_snip_is_live_on_its_7th_to_14th_ticks() {
    let (mut sim, id) = crab_and_claw(40);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.attack_rect = enemy::facing_rect(o, crab::CONTACT_RECT);
        crab::enter_snip(w, id);
    });
    let mut live = Vec::new();
    let mut snips = 0;
    for k in 1..=15 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        if o.attack_rect == enemy::facing_rect(o, crab::SNIP_BOX) {
            live.push(k);
        }
        if cued(&sim, crab::SNIP_SOUND) {
            snips += 1;
        }
        if k == 15 {
            assert_eq!(state(&sim, id), Some(EnemyState::Regroup));
            assert_eq!(o.attack_rect, enemy::facing_rect(o, crab::CONTACT_RECT));
        }
    }
    assert_eq!(live, (7..=14).collect::<Vec<_>>());
    assert_eq!(snips, 1);
}

#[test]
fn a_snip_costs_claw_5() {
    let (mut sim, id) = crab_and_claw(30);
    let health = sim.stats().health;
    sim.with_world(|w| crab::enter_snip(w, id));
    for _ in 0..15 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.stats().health, health - crab::DAMAGE);
}

/// A bomber at x 2000 with Claw `dx` px from it.
fn bomber_and_claw(dx: i32) -> (Sim, ObjectId) {
    let mut s = hermit(2000, 900, (0, 0));
    s.record[field::USER_0] = 1;
    let (sim, ids) = game(beach((2000 + dx, 900)), &[s]);
    (sim, ids[0])
}

fn bombs(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == "CrabBomb")
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn a_bomber_throws_on_its_third_launch_tick_and_waits_4000_ms() {
    let (mut sim, id) = bomber_and_claw(500);
    sim.with_world(|w| crab::enter_attack(w, id));
    assert_eq!(state(&sim, id), Some(EnemyState::Launch));
    let n = until(&mut sim, 20, |s| !bombs(s).is_empty());
    assert_eq!(n, Some(3));
    assert!(cued(&sim, crab::THROW_SOUND));
    let b = bombs(&sim)[0];
    let d = bomb::data_of(obj(&sim, b)).unwrap();
    assert!((d.motion.vx - 387.298).abs() < 1e-3, "towards Claw");
    assert!((d.motion.vy + 687.298).abs() < 1e-3);
    assert_eq!((obj(&sim, b).x, obj(&sim, b).y), (2000, CRAB_Y));
    let n = until(&mut sim, 20, |s| state(s, id) != Some(EnemyState::Launch));
    assert_eq!(n, Some(12), "it regroups on the 15th launch tick");
    // Within 4000 ms of the throw an attack regroups instead.
    sim.with_world(|w| crab::enter_attack(w, id));
    assert_eq!(state(&sim, id), Some(EnemyState::Regroup));
    for _ in 0..(4000 / 18) {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| crab::enter_attack(w, id));
    assert_eq!(state(&sim, id), Some(EnemyState::Launch));
}

#[test]
fn a_bomb_lands_267_px_out_and_goes_off_on_its_fuse() {
    let (mut sim, id) = bomber_and_claw(500);
    sim.with_world(|w| crab::enter_attack(w, id));
    until(&mut sim, 20, |s| !bombs(s).is_empty()).expect("thrown");
    let b = bombs(&sim)[0];
    let mut rows = Vec::new();
    for _ in 0..80 {
        sim.step(TickInput::default());
        rows.push(sim.objects().get(b).map(|o| {
            (
                o.x - 2000,
                o.y - CRAB_Y,
                o.physics_type,
                o.state == bomb::state::EXPLODING,
            )
        }));
    }
    let at = |k: usize| rows[k - 1].expect("the bomb");
    // It lands 39 ticks after the throw, 264 px out with its box on the
    // floor, and is on its way up again 267 px out on the next.
    let landed = (1..=80).find(|&k| at(k).2 == ptype::STAND);
    assert_eq!(landed, Some(39));
    assert_eq!((at(39).0, at(39).1), (264, 9));
    assert_eq!((at(40).0, at(40).1), (267, 1));
    // The fuse fires on the 56th tick, 319 px out and 12 px above the
    // throw, before it moves; the blast lasts 16 ticks.
    let boom = (1..=80).find(|&k| at(k).3);
    assert_eq!(boom, Some(56));
    assert_eq!((at(55).0, at(55).1), (319, -12));
    assert_eq!((at(56).0, at(56).1), (319, -12));
    let gone = rows.iter().position(Option::is_none).map(|k| k + 1);
    assert_eq!(gone, Some(56 + 16));
}

#[test]
fn a_bomb_that_meets_claw_goes_off_on_him_and_costs_him_5() {
    let (mut sim, _) = game(beach((1100, 800)), &[]);
    let c = sim.player().map(|c| (c.x, c.y)).unwrap();
    let health = sim.stats().health;
    let b = sim.with_world(|w| bomb::spawn(w, c.0 - 60, c.1, 100, crab::THROW_DISTANCE));
    // Created inside his reach: it goes off in the collision pass of the
    // throw (here, the next tick's).
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, b).state, bomb::state::EXPLODING);
    assert_eq!(sim.stats().health, health - bomb::DAMAGE);
    // His window after the hit keeps the blast from hurting him again.
    for _ in 0..16 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.stats().health, health - bomb::DAMAGE);
    assert!(sim.objects().get(b).is_none());
}

#[test]
fn a_bomb_thrown_inside_solid_ground_is_gone_at_once() {
    let (mut sim, _) = game(beach((5000, 800)), &[]);
    let b = sim.with_world(|w| bomb::spawn(w, 1000, FLOOR + 10, 100, crab::THROW_DISTANCE));
    sim.step(TickInput::default());
    assert!(sim.objects().get(b).is_none());
}

#[test]
fn a_hit_on_the_shell_knocks_it_aside_and_a_hit_outside_kills() {
    let (mut sim, id) = crab_and_claw(-600);
    let blow = attacker(&mut sim, types::PLAYER, (1950, 900));
    // Withdrawn: knocked away from the blow, at 300 px/s.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 802;
        crab::on_hit(w, id, blow);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge));
    assert_eq!(enemy::data_of(obj(&sim, id)).unwrap().vx, 300.0);
    assert!(cued_next(&mut sim, crab::SHELL_SOUND));
    // Crouched: it hides first, then is knocked.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 800;
        crab::on_hit(w, id, blow);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge));
    let f = crab::fields_of(obj(&sim, id)).unwrap();
    assert!(!f.hide.done(sim.tick() * TICK), "a hide time runs");
    // Out of its shell: dead at once, whatever its health.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 1;
        crab::on_hit(w, id, blow);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Dead));
    let d = enemy::data_of(obj(&sim, id)).unwrap();
    assert_eq!((d.vx.abs(), d.vy), (150.0, -700.0));
    assert_eq!(obj(&sim, id).physics_type, ptype::DIRECT);
}

/// True when `key` was cued by the calls made since the last step.
fn cued_next(sim: &mut Sim, key: &str) -> bool {
    sim.step(TickInput::default());
    cued(sim, key)
}

#[test]
fn the_magic_claw_gets_through_the_withdrawn_shell_only() {
    let (mut sim, id) = crab_and_claw(600);
    let magic = attacker(&mut sim, enemy::attacker::MAGIC, (1950, 900));
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 800;
        crab::on_hit(w, id, magic);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge));
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 801;
        crab::on_hit(w, id, magic);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn an_explosion_hits_once_and_a_pistol_shot_is_used_up() {
    let (mut sim, id) = crab_and_claw(600);
    let blast = attacker(&mut sim, types::LEVEL_SHOT, (2050, 900));
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 802;
        crab::on_hit(w, id, blast);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge));
    assert_eq!(enemy::data_of(obj(&sim, id)).unwrap().vx, -300.0, "away");
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 1;
        crab::on_hit(w, id, blast);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge), "the same blast");
    let shot = attacker(&mut sim, types::PISTOL_SHOT, (1950, 900));
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 802;
        crab::on_hit(w, id, shot);
    });
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
}

#[test]
fn a_killed_crab_drops_its_item() {
    let mut s = hermit(2000, 900, (1700, 2300));
    s.record[field::POWERUP] = 33;
    let (mut sim, ids) = game(beach((2500, 800)), &[s]);
    let id = ids[0];
    let blow = attacker(&mut sim, types::PLAYER, (1950, 900));
    let before = sim.objects().all().count();
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().frame = 1;
        crab::on_hit(w, id, blow);
    });
    assert_eq!(state(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.objects().all().count(), before + 1, "a coin");
}

/// A nest at (x, 896) with the trigger box `trigger` (all zeros for the
/// picture's).
fn nest_at(x: i32, trigger: Rect) -> ObjectSpec {
    let mut s = ObjectSpec::at("CrabNest", x, 896, 0);
    s.image_set = nest::IMAGE_SET.into();
    s.frame = 1;
    s.record[field::USER_0] = 3;
    if trigger.is_set() {
        s.record[field::X_MIN] = trigger.left;
        s.record[field::Y_MIN] = trigger.top;
        s.record[field::X_MAX] = trigger.right;
        s.record[field::Y_MAX] = trigger.bottom;
    }
    s
}

#[test]
fn a_nest_hatches_three_crabs_when_claw_walks_into_its_box() {
    let trigger = Rect::new(1700, 800, 1800, 1000);
    let (mut sim, ids) = game(beach((1600, 900)), &[nest_at(2000, trigger)]);
    let n = ids[0];
    assert_eq!(obj(&sim, n).state, nest::state::IDLE);
    assert_eq!(nest::data_of(obj(&sim, n)).unwrap().trigger, Some(trigger));
    assert_eq!(obj(&sim, n).z, 1);
    // Standing outside the box: nothing.
    for _ in 0..30 {
        sim.step(TickInput::default());
    }
    assert_eq!(obj(&sim, n).state, nest::state::IDLE);
    // Walking in: the hatch, the crabs on its fourth tick.
    let walk = until(&mut sim, 200, |s| obj(s, n).state == nest::state::HATCH);
    assert!(walk.is_none(), "Claw still stands");
    let mut sim2 = sim.clone();
    sim2.teleport_player(1750, obj(&sim, claw_id(&sim)).y);
    let k = until(&mut sim2, 5, |s| obj(s, n).state == nest::state::HATCH);
    assert_eq!(k, Some(1));
    let k = until(&mut sim2, 10, |s| {
        !nest::data_of(obj(s, n)).unwrap().hatchlings.is_empty()
    });
    assert_eq!(k, Some(4));
    assert!(cued(&sim2, nest::HATCH_SOUND));
    let hatched = nest::data_of(obj(&sim2, n)).unwrap().hatchlings.clone();
    assert_eq!(hatched.len(), 3);
    let speeds: Vec<(f64, f64)> = hatched
        .iter()
        .map(|&c| {
            let d = enemy::data_of(obj(&sim2, c)).unwrap();
            (d.vx, d.vy)
        })
        .collect();
    assert_eq!((speeds[0].0, speeds[1].0), (-75.0, -25.0));
    for (i, (vx, vy)) in speeds.iter().enumerate() {
        assert!((-400.0..=-200.0).contains(vy), "{i}: {vy}");
        assert!((-200.0..=200.0).contains(vx));
        let o = obj(&sim2, hatched[i]);
        assert_eq!(state(&sim2, hatched[i]), Some(EnemyState::Tumble));
        assert_eq!((o.x, o.z), (2000, 0));
        assert_eq!(o.physics_type, ptype::FLY);
        let f = crab::fields_of(o).unwrap();
        // A nest's plain crab snips with the bomb's sound, its bomber
        // throws with the snip's.
        let want = if f.bomber {
            crab::SNIP_SOUND
        } else {
            crab::THROW_SOUND
        };
        assert_eq!(f.strike_sound, want);
    }
    // Quiet again on the 16th hatch tick, and for good.
    let k = until(&mut sim2, 20, |s| obj(s, n).state == nest::state::IDLE);
    assert_eq!(k, Some(12));
    for _ in 0..300 {
        sim2.step(TickInput::default());
        assert_eq!(obj(&sim2, n).state, nest::state::IDLE);
    }
    // The crabs land on the nest's floor and regroup.
    for &c in &hatched {
        if let Some(o) = sim2.objects().get(c)
            && state(&sim2, c) != Some(EnemyState::Dead)
        {
            assert_eq!(o.y, CRAB_Y);
        }
    }
}

#[test]
fn a_nest_without_a_box_uses_its_picture_and_ignores_a_jump() {
    let (mut sim, ids) = game(beach((1700, 900)), &[nest_at(2000, Rect::UNSET)]);
    let n = ids[0];
    let t = nest::data_of(obj(&sim, n)).unwrap().trigger;
    assert_eq!(t, Some(nest::PICTURE_BOX.translate(2000, 896)));
    // Claw passing over it in the air does not set it off.
    let y = obj(&sim, claw_id(&sim)).y;
    sim.teleport_player(1990, y - 5);
    sim.with_world(|w| {
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().physics_type = ptype::JUMP;
    });
    sim.with_world(|w| logic::tick(w, n));
    assert_eq!(obj(&sim, n).state, nest::state::IDLE);
}

#[test]
fn a_nest_counts_its_crabs() {
    for (user_0, want) in [(0, 3), (1, 1), (5, 5), (12, 9)] {
        let mut s = nest_at(2000, Rect::new(1700, 800, 1800, 1000));
        s.record[field::USER_0] = user_0;
        let (sim, ids) = game(beach((1600, 900)), &[s]);
        assert_eq!(nest::data_of(obj(&sim, ids[0])).unwrap().crabs, want);
    }
}
