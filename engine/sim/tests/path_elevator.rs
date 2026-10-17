//! Path elevators on synthetic levels: the route read from the record's
//! boxes, the place the clock puts the elevator in every tick, Claw and an
//! enemy carried exactly (diagonal legs included), the move guard, a still
//! elevator without a route, and the jump after a sleep.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::path_elevator::{self, PathElevatorData, state};
use claw_sim::logic::platform::{self, PLATFORM, SurfaceKind};
use claw_sim::object::{ObjectId, flags};
use claw_sim::physics::ptype;
use claw_sim::{ObjectData, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The record fields of the four boxes, from the movement box's left.
const BOXES: usize = 19;
const SPEED: usize = 60;
const DIRECTION: usize = 63;
const POWERUP: usize = 15;

/// A path elevator record at (x, y) with `legs` written in its boxes.
fn elevator(x: i32, y: i32, speed: i32, legs: &[(i32, i32)]) -> ObjectSpec {
    let mut s = ObjectSpec::at("PathElevator", x, y, 0);
    for (i, &(dir, d)) in legs.iter().enumerate() {
        s.record[BOXES + 2 * i] = dir;
        s.record[BOXES + 2 * i + 1] = d;
    }
    s.record[SPEED] = speed;
    s
}

/// A square loop of `side` px: up, right, down, left.
fn square(side: i32) -> Vec<(i32, i32)> {
    vec![(8, side), (6, side), (2, side), (4, side)]
}

/// A diamond of diagonal legs of `d` px.
fn diamond(d: i32) -> Vec<(i32, i32)> {
    vec![(9, d), (3, d), (1, d), (7, d)]
}

/// The flat level (feet rest on 959) with `objects`, every object awake
/// from the start.
fn sim_with(objects: Vec<ObjectSpec>) -> Sim {
    let mut level = flat_level();
    level.objects = objects;
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

fn elevators(sim: &Sim) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "PathElevator")
        .map(|(id, _)| id)
        .collect();
    v.sort_unstable();
    v
}

fn data(sim: &Sim, id: ObjectId) -> PathElevatorData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::PathElevator(d) => d.clone(),
        other => panic!("not a path elevator: {other:?}"),
    }
}

fn pos(sim: &Sim, id: ObjectId) -> (i32, i32) {
    let o = sim.objects().get(id).unwrap();
    (o.x, o.y)
}

/// True when Claw rides `id` with his feet one pixel above its surface.
fn riding(sim: &Sim, id: ObjectId) -> bool {
    let c = sim.player().unwrap();
    let p = sim.objects().get(id).unwrap();
    c.flags & flags::ON_ELEVATOR != 0
        && c.object_below == Some(id)
        && c.y + c.move_rect.bottom == p.y + p.hit_rect.top - 1
}

/// Drops Claw onto `id` and waits until he rides it.
fn land_on(sim: &mut Sim, id: ObjectId) {
    let (x, y) = pos(sim, id);
    let top = sim.objects().get(id).unwrap().hit_rect.top;
    let bottom = sim.player().unwrap().move_rect.bottom;
    sim.teleport_player(x, y + top - bottom - 10);
    for _ in 0..100 {
        sim.step(TickInput::default());
        if riding(sim, id) {
            return;
        }
    }
    panic!("Claw never landed on the path elevator");
}

#[test]
fn the_set_up_reads_the_route_and_makes_a_platform() {
    let sim = sim_with(vec![elevator(2000, 600, 200, &diamond(60))]);
    let id = elevators(&sim)[0];
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.state, state::MOVING);
    assert_eq!(o.type_flags, PLATFORM);
    assert!(o.flags & flags::NO_COLLIDE != 0);
    assert_eq!(o.hit_rect, platform::surface(1, SurfaceKind::Elevator));
    assert_eq!(o.z, 0, "the depth is kept as placed");
    assert_eq!(pos(&sim, id), (2000, 600), "the set-up does not move it");
    let d = data(&sim, id);
    assert_eq!(d.legs, diamond(60));
    assert_eq!((d.speed, d.phase, d.period), (200, 0, 1200));
}

#[test]
fn the_elevator_is_where_the_clock_puts_it_and_tells_its_riders() {
    // An octagon: straight legs of 200 px and diagonals of 60.
    let legs = [
        (8, 200),
        (9, 60),
        (6, 200),
        (3, 60),
        (2, 200),
        (1, 60),
        (4, 200),
        (7, 60),
    ];
    let mut spec = elevator(3000, 600, 175, &legs);
    spec.record[DIRECTION] = 1000;
    let mut sim = sim_with(vec![spec]);
    let id = elevators(&sim)[0];
    let d = data(&sim, id);
    assert_eq!(d.phase, 1000);
    let mut last = pos(&sim, id);
    for t in 1..=800u32 {
        sim.step(TickInput::default());
        let ms = t * 18;
        let (dx, dy) = path_elevator::position_at(&d, ms).unwrap();
        let now = pos(&sim, id);
        assert_eq!(now, (3000 + dx, 600 + dy), "tick {t}");
        let o = sim.objects().get(id).unwrap();
        let mv = (now.0 - last.0, now.1 - last.1);
        if t > 1 {
            assert_eq!(o.platform_move, mv, "tick {t}");
            assert!(mv.0.abs() <= 4 && mv.1.abs() <= 4, "tick {t}: {mv:?}");
        }
        last = now;
    }
}

#[test]
fn claw_rides_every_leg_exactly() {
    for legs in [square(150), diamond(80)] {
        let mut sim = sim_with(vec![elevator(3000, 600, 200, &legs)]);
        let id = elevators(&sim)[0];
        land_on(&mut sim, id);
        let offset = sim.player().unwrap().x - pos(&sim, id).0;
        let d = data(&sim, id);
        let ticks = d.period.div_ceil(18) as usize * 2;
        let mut moved = (false, false);
        for t in 0..ticks {
            let before = pos(&sim, id);
            sim.step(TickInput::default());
            let after = pos(&sim, id);
            moved.0 |= after.0 != before.0;
            moved.1 |= after.1 != before.1;
            assert!(riding(&sim, id), "{legs:?} tick {t}");
            // Carried across with no drift: Claw keeps his place on the
            // platform.
            assert_eq!(
                sim.player().unwrap().x - after.0,
                offset,
                "{legs:?} tick {t}"
            );
        }
        assert!(moved.0 && moved.1, "{legs:?}");
    }
}

#[test]
fn an_enemy_rides_a_path_elevator_up_and_down() {
    // A walking rat stood on a route that goes up and down rides it like
    // Claw: it stands on the surface every tick. (Across, a walker's
    // patrol limits and ledge guard hold it where it is, so a route that
    // moves across leaves it behind; no special case is made for that.)
    let mut rat = ObjectSpec::at("Rat", 3000, 900, 0);
    rat.record[enemy::field::X_MIN] = 2990;
    rat.record[enemy::field::X_MAX] = 3010;
    let mut sim = sim_with(vec![elevator(3000, 500, 150, &[(8, 100), (2, 100)]), rat]);
    sim.teleport_player(9000, 899);
    let lift = elevators(&sim)[0];
    let rat = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Rat")
        .map(|(id, _)| id)
        .unwrap();
    idle(&mut sim, 2);
    // Built on the floor; stand it on the elevator.
    sim.with_world(|w| {
        let top = w.objects.get(lift).map(|p| p.y + p.hit_rect.top).unwrap();
        let x = w.objects.get(lift).map(|p| p.x).unwrap();
        let o = w.objects.get_mut(rat).unwrap();
        o.x = x;
        o.y = top - 1 - o.move_rect.bottom;
        o.physics_type = ptype::STAND;
        o.flags |= flags::ON_ELEVATOR;
        o.object_below = Some(lift);
    });
    for t in 0..200 {
        sim.step(TickInput::default());
        let o = sim.objects().get(rat).unwrap();
        let p = sim.objects().get(lift).unwrap();
        assert_eq!(o.object_below, Some(lift), "tick {t}");
        assert_eq!(
            o.y + o.move_rect.bottom,
            p.y + p.hit_rect.top - 1,
            "tick {t}"
        );
        assert!(
            EnemyState::from_id(o.state).is_some(),
            "tick {t}: {}",
            o.state
        );
    }
}

#[test]
fn a_move_of_100_px_or_more_is_published_as_none() {
    // An open route of one leg jumps back to its start every period, drawn
    // there at once.
    let mut spec = elevator(3000, 600, 100, &[(6, 200)]);
    spec.image_set = "TEST_SET".into();
    let mut sim = sim_with(vec![spec]);
    let id = elevators(&sim)[0];
    let serial = sim.objects().get(id).unwrap().serial;
    let mut wrapped = 0;
    for _ in 0..300 {
        let before = pos(&sim, id);
        let snap = sim.step(TickInput::default());
        let after = pos(&sim, id);
        let o = sim.objects().get(id).unwrap();
        if after.0 < before.0 {
            wrapped += 1;
            assert!(before.0 - after.0 >= 100);
            assert_eq!(o.platform_move, (0, 0));
            let r = snap.renderables.iter().find(|r| r.id == serial).unwrap();
            assert!(r.teleported);
        } else {
            assert_eq!(o.platform_move, (after.0 - before.0, 0));
        }
    }
    assert!(wrapped >= 2);
}

#[test]
fn without_a_route_it_stands_still() {
    let mut sim = sim_with(vec![elevator(3000, 600, 200, &[])]);
    let id = elevators(&sim)[0];
    assert_eq!(sim.objects().get(id).unwrap().state, state::STILL);
    idle(&mut sim, 50);
    assert_eq!(pos(&sim, id), (3000, 600));
    assert_eq!(sim.objects().get(id).unwrap().type_flags, PLATFORM);
}

#[test]
fn a_built_in_route_takes_precedence() {
    let mut spec = elevator(3000, 600, 0, &square(40));
    spec.record[POWERUP] = 2;
    let sim = sim_with(vec![spec]);
    let d = data(&sim, elevators(&sim)[0]);
    assert_eq!(d.legs, path_elevator::built_in(2).unwrap());
    assert_eq!(d.speed, path_elevator::DEFAULT_SPEED);
    assert_eq!(d.period, 5000);
}

#[test]
fn a_woken_elevator_is_where_the_clock_puts_it() {
    // Asleep far from Claw with the level's own windows, then woken: on
    // its first call it is at the clock's place, drawn there at once.
    let mut level = flat_level();
    level.objects = vec![elevator(6000, 600, 200, &square(100))];
    let mut sim = Sim::new(level, SimConfig::default());
    let id = elevators(&sim)[0];
    assert_eq!(sim.objects().get(id).unwrap().state, 0, "asleep");
    sim.teleport_player(5800, 899);
    let mut first = None;
    for _ in 0..300 {
        sim.step(TickInput::default());
        if sim.objects().get(id).unwrap().state == state::MOVING && first.is_none() {
            first = Some(sim.tick());
        } else if first.is_some() {
            break;
        }
    }
    let d = data(&sim, id);
    let ms = (sim.tick() * 18) as u32;
    let (dx, dy) = path_elevator::position_at(&d, ms).unwrap();
    assert_eq!(pos(&sim, id), (6000 + dx, 600 + dy));
}

// ---------------------------------------------------------------- install

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

#[test]
fn install_level3_path_elevators_follow_their_routes() {
    let Some(store) = store() else { return };
    let data3 = LevelData::build(&store, LevelSource::Retail(3)).unwrap();
    let level = SimLevel::from_level(&data3, &store).unwrap();
    assert_eq!(
        level
            .objects
            .iter()
            .filter(|o| o.logic == "PathElevator")
            .count(),
        14
    );
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    let ids = elevators(&sim);
    assert_eq!(ids.len(), 14);
    let mut periods: Vec<u32> = ids.iter().map(|&id| data(&sim, id).period).collect();
    periods.sort_unstable();
    periods.dedup();
    assert_eq!(periods, vec![5200, 7884]);
    for &id in &ids {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.state, state::MOVING);
        // The third level's elevator surface, 16 px above the position.
        assert_eq!(o.hit_rect, platform::surface(3, SurfaceKind::Elevator));
        assert_eq!(o.hit_rect.top, -16);
        assert_eq!(o.z, 0);
    }
    let start: Vec<(i32, i32)> = ids.iter().map(|&id| pos(&sim, id)).collect();
    for t in 1..=900u32 {
        sim.step(TickInput::default());
        for &id in &ids {
            let d = data(&sim, id);
            let o = sim.objects().get(id).unwrap();
            let (dx, dy) = path_elevator::position_at(&d, t * 18).unwrap();
            assert_eq!((o.x, o.y), (o.editor.0 + dx, o.editor.1 + dy));
        }
    }
    // Every route is closed: each elevator is back at its creation point
    // whenever the clock is a multiple of its period.
    for (&id, &(x, y)) in ids.iter().zip(&start) {
        let d = data(&sim, id);
        assert_eq!(path_elevator::position_at(&d, d.period * 3), Some((0, 0)));
        assert_eq!(sim.objects().get(id).unwrap().editor.0, x);
        assert_eq!(sim.objects().get(id).unwrap().editor.1, y);
    }
}
