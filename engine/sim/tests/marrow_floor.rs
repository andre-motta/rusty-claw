//! Marrow's trap floor on a synthetic room at the tenth level's place:
//! the two slabs of the level as placed, polling the Marrow link: their
//! set-up, their opening and closing schedules, the phase health and the
//! floors-open status they write, their reset after a death of Claw's,
//! their stop once Marrow is gone, the rider they carry, and how a slab
//! looks before it first wakes.

mod common;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, marrow};
use claw_sim::logic::marrow_floor::{self, state};
use claw_sim::logic::platform::{self, PLATFORM, SurfaceKind};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, flags};
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The two slabs as the level places them: (x, low end, high end,
/// direction).
const SLABS: [(i32, i32, i32, i32); 2] = [(40714, 40534, 40714, 0), (40911, 40911, 41071, -1)];
/// Their height.
const SLAB_Y: i32 = 7393;
/// Marrow as placed.
const MARROW_AT: (i32, i32) = (41063, 7255);

/// A synthetic room: a flat floor whose top is at y 7488, below the slabs,
/// across the whole width of the tenth level's boss room, the level
/// number 10.
fn room_level() -> SimLevel {
    let mut rows = vec![".".repeat(660); 117];
    rows.push("#".repeat(660));
    rows.push("#".repeat(660));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (40480, 7200));
    level.number = 10;
    level
        .sounds
        .insert(marrow_floor::SLIDE_SOUND.to_string(), Some(679));
    level
}

fn slab_spec(i: usize) -> ObjectSpec {
    let (x, lo, hi, dir) = SLABS[i];
    let mut o = ObjectSpec::at("MarrowFloor", x, SLAB_Y, [2500, 2000][i]);
    o.image_set = marrow_floor::IMAGE_SET.into();
    o.record[marrow_floor::field::X_MIN] = lo;
    o.record[marrow_floor::field::X_MAX] = hi;
    o.record[marrow_floor::field::DIRECTION] = dir;
    o.record[enemy::field::SPEED_X] = 300;
    o
}

/// The room with both slabs, and Marrow when `with_marrow`.
fn room(with_marrow: bool) -> Sim {
    let mut level = room_level();
    level.objects = vec![slab_spec(0), slab_spec(1)];
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
    sim
}

fn slab(sim: &Sim, i: usize) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == "MarrowFloor" && o.editor.0 == SLABS[i].0)
        .map(|(id, _)| id)
        .expect("the slab")
}

fn at(sim: &Sim, id: ObjectId) -> (i32, i32) {
    let o = sim.objects().get(id).expect("the slab");
    (o.x, o.state)
}

fn slides(sim: &Sim) -> usize {
    sim.last_audio()
        .iter()
        .filter(|c| matches!(c, AudioCue::Play(s) if s.key == marrow_floor::SLIDE_SOUND))
        .count()
}

/// Steps until slab `i` is idle again, counting the ticks.
fn ticks_to_idle(sim: &mut Sim, i: usize) -> usize {
    let id = slab(sim, i);
    for k in 1..200 {
        sim.step(TickInput::default());
        if at(sim, id).1 == state::IDLE {
            return k;
        }
    }
    panic!("the slab never stops");
}

#[test]
fn a_slab_is_a_platform_of_the_big_trap_elevators_surface() {
    let sim = room(false);
    for (i, placed) in SLABS.iter().enumerate() {
        let o = sim.objects().get(slab(&sim, i)).expect("slab");
        assert_eq!(o.state, state::IDLE, "set up while the level loads");
        assert_eq!(o.type_flags, PLATFORM);
        assert!(o.flags & flags::NO_COLLIDE != 0, "out of pair collision");
        assert_eq!(o.hit_rect, platform::surface(10, SurfaceKind::BigElevator));
        assert_eq!(o.image_set, marrow_floor::IMAGE_SET);
        let ObjectData::MarrowFloor(d) = &o.data else {
            panic!("its data");
        };
        assert_eq!((d.closed_x, d.open), (placed.0, false));
    }
}

#[test]
fn without_marrow_the_slabs_stay_closed_where_the_level_puts_them() {
    let mut sim = room(false);
    idle(&mut sim, 100);
    assert_eq!(sim.marrow_link(), None);
    for (i, placed) in SLABS.iter().enumerate() {
        assert_eq!(at(&sim, slab(&sim, i)), (placed.0, state::IDLE));
    }
}

#[test]
fn marrow_built_the_slabs_open_in_whole_five_pixel_steps() {
    let mut sim = room(true);
    let link = sim.marrow_link().copied().expect("the link");
    assert!(link.open_request);
    // On the first tick both enter their opening (the slide sound once
    // each), and the first step comes on the next.
    sim.step(TickInput::default());
    assert_eq!(slides(&sim), 2);
    let (l, r) = (slab(&sim, 0), slab(&sim, 1));
    assert_eq!(at(&sim, l), (40714, state::OPENING));
    assert_eq!(at(&sim, r), (40911, state::OPENING));
    for k in 1..=31 {
        sim.step(TickInput::default());
        assert_eq!(at(&sim, r).0, 40911 + 5 * k);
        assert_eq!(at(&sim, l).0, 40714 - 5 * k);
        let moved = sim.objects().get(r).expect("slab").platform_move;
        assert_eq!(moved, (5, 0), "a rider follows");
    }
    // The right one ends on its 32nd step, open; the last step carries
    // no rider.
    sim.step(TickInput::default());
    assert_eq!(at(&sim, r), (41071, state::IDLE));
    assert_eq!(sim.objects().get(r).expect("slab").platform_move, (0, 0));
    assert!(sim.marrow_link().expect("link").floors_open);
    for _ in 0..4 {
        sim.step(TickInput::default());
    }
    assert_eq!(at(&sim, l), (40534, state::IDLE));
    // Open and asked to stay open, they stay.
    idle(&mut sim, 50);
    assert_eq!(
        (at(&sim, l), at(&sim, r)),
        ((40534, state::IDLE), (41071, state::IDLE))
    );
}

#[test]
fn the_open_request_cleared_closes_the_slabs_and_sets_the_phase_health() {
    let mut sim = room(true);
    idle(&mut sim, 40);
    let marrow_id = sim.marrow_link().expect("link").marrow;
    sim.with_world(|w| {
        w.objects.get_mut(marrow_id).expect("Marrow").health = 80;
        w.marrow_link_mut().expect("link").open_request = false;
    });
    // Seen on the next tick (entry, sound), stepping from the one after:
    // 32 steps for the right slab, 36 for the left.
    sim.step(TickInput::default());
    assert_eq!(slides(&sim), 2);
    assert_eq!(at(&sim, slab(&sim, 1)).1, state::CLOSING);
    assert_eq!(ticks_to_idle(&mut sim, 1), 32);
    assert_eq!(at(&sim, slab(&sim, 1)).0, 40911);
    let link = sim.marrow_link().copied().expect("link");
    assert_eq!(link.phase_health, 80);
    assert!(!link.floors_open);
    assert_eq!(ticks_to_idle(&mut sim, 0), 4);
    assert_eq!(at(&sim, slab(&sim, 0)).0, 40714);
}

#[test]
fn a_move_once_started_runs_to_its_end() {
    let mut sim = room(true);
    idle(&mut sim, 10);
    // The request clears while they open: they finish opening, then close.
    sim.with_world(|w| w.marrow_link_mut().expect("link").open_request = false);
    let r = slab(&sim, 1);
    idle(&mut sim, 23);
    assert_eq!(at(&sim, r), (41071, state::IDLE));
    sim.step(TickInput::default());
    assert_eq!(at(&sim, r).1, state::CLOSING);
}

#[test]
fn a_death_of_claws_snaps_the_slabs_shut_and_opens_them_again_in_the_respawn() {
    let mut sim = room(true);
    idle(&mut sim, 50);
    let marrow_id = sim.marrow_link().expect("link").marrow;
    sim.with_world(|w| w.objects.get_mut(marrow_id).expect("Marrow").health = 55);
    sim.end_life();
    let mut respawn = None;
    for k in 0..400 {
        sim.step(TickInput::default());
        if sim.claw().stats.attempt == 1 && sim.claw().stats.health > 0 {
            respawn = Some(k);
            break;
        }
    }
    assert!(respawn.is_some(), "Claw respawns");
    // In the respawn's tick: Marrow's health restored before the reset
    // reads it, the slabs back at their record's x and already a step into
    // their new opening.
    let link = sim.marrow_link().copied().expect("link");
    assert_eq!(link.phase_health, marrow::HEALTH);
    assert!(!link.floors_open);
    assert_eq!(at(&sim, slab(&sim, 1)), (40916, state::OPENING));
    assert_eq!(at(&sim, slab(&sim, 0)), (40709, state::OPENING));
    assert_eq!(ticks_to_idle(&mut sim, 1), 31);
}

#[test]
fn once_marrow_is_gone_the_slabs_stop_where_they_are() {
    let mut sim = room(true);
    idle(&mut sim, 10);
    let marrow_id = sim.marrow_link().expect("link").marrow;
    let r = slab(&sim, 1);
    let x = at(&sim, r).0;
    sim.with_world(|w| w.unlink_marrow(marrow_id));
    idle(&mut sim, 60);
    assert_eq!(at(&sim, r), (x, state::OPENING), "stopped halfway");
}

#[test]
fn a_dormant_slab_looks_as_the_level_places_it() {
    let sim = room(false);
    let level = room_level();
    let o = sim.objects().get(slab(&sim, 1)).expect("slab");
    assert_eq!(logic::first_look(&level, o), None);
    let mut other = o.clone();
    other.image_set = "LEVEL_OTHER".into();
    let look = logic::first_look(&level, &other).expect("its own picture");
    assert_eq!(look[0].image_set, marrow_floor::IMAGE_SET);
    assert_eq!((look[0].x, look[0].y), (40911, SLAB_Y));
}

#[test]
fn claw_standing_on_an_opening_slab_is_carried_with_it() {
    let mut sim = room(true);
    let r = slab(&sim, 1);
    // Close the bridge, then put Claw on the right slab and open it.
    idle(&mut sim, 40);
    sim.with_world(|w| w.marrow_link_mut().expect("link").open_request = false);
    idle(&mut sim, 40);
    assert_eq!(at(&sim, r), (40911, state::IDLE));
    let top = SLAB_Y + platform::surface(10, SurfaceKind::BigElevator).top;
    sim.teleport_player(40930, top - 70);
    idle(&mut sim, 30);
    let before = sim.player().expect("Claw").x;
    sim.with_world(|w| w.marrow_link_mut().expect("link").open_request = true);
    idle(&mut sim, 10);
    let after = sim.player().expect("Claw").x;
    assert_eq!(after - before, 5 * 9, "carried at 5 px a tick");
}
