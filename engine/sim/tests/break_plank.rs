//! Breaking planks on a synthetic level laid out like the fifth level's
//! rows: the trigger window, the wait, the crumble, the floor going, the
//! spread along the row, and the row restored after a death.

mod common;

use claw_sim::anim::Animation;
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::break_plank::{self, FALLBACK_MOVE_RECT, state};
use claw_sim::object::{ObjectId, draw_flags};
use claw_sim::physics::ptype;
use claw_sim::tiles::{CLEAR, Descriptor, GROUND, TileMap};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;
use std::sync::Arc;

/// The row's spawner: five pieces from x 807 at y 793, 500 ms apart.
const ROW: (i32, i32) = (807, 793);
/// Where Claw stands on the row.
const STAND_Y: i32 = 671;
/// The crumble's sound.
const CRUMBLE_SOUND: &str = "LEVEL_CRUMBLEBRIDGE2";

/// A level numbered 5 (so the fifth level's plank table applies) with a
/// floor at y 960 and, in row 11, the planks' floor tiles: one-way floor
/// 28 px below the cell's top, nothing above it. The row's pieces stand at
/// x 807 + 64 i; their cells are columns 12 to 16.
fn plank_level(start: (i32, i32), width: i32, delay: i32) -> (SimLevel, i32) {
    let mut rows = vec![".".repeat(60); 15];
    rows.push("#".repeat(60));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut map = TileMap::from_ascii(64, &rows);
    let plank = map.add_descriptor(Descriptor::Double {
        width: 64,
        height: 64,
        outside: CLEAR,
        inside: GROUND,
        rect: [0, 28, 63, 63],
    });
    for c in 12..12 + width {
        map.set_cell(c, 11, plank);
    }
    let mut level = SimLevel::synthetic(map, start);
    level.number = 5;
    let mut spec = ObjectSpec::at(break_plank::SPAWNER, ROW.0, ROW.1, 1010);
    spec.record[break_plank::DELAY_FIELD] = delay;
    spec.record[break_plank::WIDTH_FIELD] = width;
    level.objects.push(spec);
    let mut frames = ImageFrames::range(1, 12);
    frames.boxes.insert(1, FALLBACK_MOVE_RECT);
    level
        .images
        .insert(break_plank::IMAGE_SET.to_string(), frames);
    let idle = Animation::once(break_plank::IDLE_ANIM, &[(1, 0)]);
    let frames: Vec<(i32, i32)> = (1..=12).map(|i| (i, 75)).collect();
    let mut crumble = Animation::once(break_plank::CRUMBLE_ANIM, &frames);
    crumble.frame_sounds = vec![Vec::new(), vec![Some(CRUMBLE_SOUND.to_string())]];
    for a in [idle, crumble] {
        level.animations.insert(a.key.clone(), Arc::new(a));
    }
    (level, plank)
}

fn spawner(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == break_plank::SPAWNER)
        .map(|(id, _)| id)
        .expect("the spawner")
}

fn pieces(sim: &Sim) -> Vec<ObjectId> {
    let o = sim.objects().get(spawner(sim)).unwrap();
    break_plank::data_of(o).expect("built").pieces.clone()
}

fn piece_state(sim: &Sim, id: ObjectId) -> (i32, i32, bool) {
    let o = sim.objects().get(id).unwrap();
    (o.state, o.frame, o.draw_flags & draw_flags::HIDDEN != 0)
}

/// A game with Claw standing at x `cx` on the row (or above it), after he
/// has landed and stood for a while.
fn standing_at(cx: i32, width: i32, delay: i32) -> (Sim, i32) {
    let (level, plank) = plank_level((cx, STAND_Y - 40), width, delay);
    let mut sim = Sim::new(level, SimConfig::default());
    for _ in 0..30 {
        sim.step(TickInput::default());
        let c = sim.player().unwrap();
        if c.y == STAND_Y && c.physics_type == ptype::STAND {
            break;
        }
    }
    (sim, plank)
}

#[test]
fn a_row_is_built_as_linked_pieces_over_their_floor_cells() {
    let (sim, plank) = standing_at(500, 5, 500);
    let s = sim.objects().get(spawner(&sim)).unwrap();
    assert_eq!(s.state, state::IDLE);
    assert!(s.draw_flags & draw_flags::HIDDEN != 0);
    let ids = pieces(&sim);
    assert_eq!(ids.len(), 5);
    for (i, &id) in ids.iter().enumerate() {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(&*o.logic, break_plank::PIECE);
        assert_eq!((o.x, o.y, o.z), (ROW.0 + 64 * i as i32, ROW.1, 1011));
        assert_eq!(o.move_rect, FALLBACK_MOVE_RECT);
        assert_eq!((o.state, o.frame), (state::IDLE, 1));
        assert!(!o.hit_rect.is_set() && !o.attack_rect.is_set());
        let d = break_plank::data_of(o).unwrap();
        assert_eq!(d.cell, (12 + i as i32, 11));
        assert_eq!(d.floor_tile, plank);
        assert_eq!(d.delay_ms, 500);
        assert_eq!(d.left, i.checked_sub(1).map(|j| ids[j]));
        assert_eq!(d.right, ids.get(i + 1).copied());
    }
    assert!(sim.collision_events().is_empty());
}

#[test]
fn claw_starts_a_piece_only_from_x_minus_23_to_x_plus_18() {
    let x = ROW.0 + 64;
    for (cx, started) in [
        (x - 24, false),
        (x - 23, true),
        (x, true),
        (x + 18, true),
        (x + 19, false),
        (x + 30, false),
        (x + 40, false),
    ] {
        let (mut sim, _) = standing_at(cx, 5, 500);
        idle(&mut sim, 5);
        let p = pieces(&sim);
        let any = p.iter().any(|&id| piece_state(&sim, id).0 != state::IDLE);
        let second = piece_state(&sim, p[1]).0 != state::IDLE;
        if started {
            assert!(second, "Claw at x {cx} starts the second piece");
        } else {
            assert!(!any, "Claw at x {cx} starts nothing");
        }
    }
}

#[test]
fn a_piece_waits_crumbles_and_drops_claw_on_the_measured_ticks() {
    let (level, _) = plank_level((ROW.0 + 64, STAND_Y - 40), 5, 500);
    let mut sim = Sim::new(level, SimConfig::default());
    // Dropped onto the row from 100 px up.
    sim.teleport_player(ROW.0 + 64, STAND_Y - 100);
    let mut rows = Vec::new();
    let mut sounds = Vec::new();
    for k in 0..200 {
        sim.step(TickInput::default());
        let c = sim.player().unwrap();
        rows.push((
            pieces(&sim)
                .iter()
                .map(|&id| piece_state(&sim, id))
                .collect::<Vec<_>>(),
            c.y,
            c.physics_type,
        ));
        if sim
            .last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == CRUMBLE_SOUND))
        {
            sounds.push(k);
        }
    }
    let landed = rows
        .iter()
        .position(|r| r.1 == STAND_Y && r.2 == ptype::STAND)
        .expect("lands on the row");
    let when = |i: usize, s: i32| rows.iter().position(|r| r.0[i].0 == s).unwrap();
    // The piece sees the landing on the next tick (pieces run before
    // Claw).
    let e = when(1, state::WAIT);
    assert_eq!(e, landed + 1);
    let c = when(1, state::CRUMBLE);
    assert_eq!(c - e, 28);
    // Its neighbours start their wait on the same tick; the far pieces
    // one wait later.
    assert_eq!((when(0, state::WAIT), when(2, state::WAIT)), (c, c));
    assert_eq!(when(3, state::WAIT), c + 28);
    assert_eq!(rows[c + 5].0[1].1, 1);
    assert_eq!(rows[c + 6].0[1].1, 2);
    assert_eq!(sounds[0], c + 6);
    assert_eq!(rows[c + 26].0[1].1, 6);
    assert_eq!(rows[c + 25].2, ptype::STAND);
    assert_eq!(
        rows[c + 26].2,
        ptype::FALL,
        "the floor goes under Claw at once"
    );
    assert!(!rows[c + 60].0[1].2);
    assert!(rows[c + 61].0[1].2, "hidden once the crumble ends");
    assert_eq!(rows[c + 61].0[1].0, state::CRUMBLE, "and it stays there");
    // Its cell is empty.
    assert_eq!(sim.with_world(|w| w.main_cell(13, 11)), -1);
}

#[test]
fn a_750_ms_row_waits_42_ticks_and_a_250_ms_row_14() {
    for (delay, k) in [(750, 42), (250, 14)] {
        let (mut sim, _) = standing_at(ROW.0, 3, delay);
        let id = pieces(&sim)[0];
        let mut rows = Vec::new();
        for _ in 0..80 {
            rows.push(piece_state(&sim, id).0);
            sim.step(TickInput::default());
        }
        let e = rows.iter().position(|&s| s == state::WAIT).unwrap();
        let c = rows.iter().position(|&s| s == state::CRUMBLE).unwrap();
        assert_eq!(c - e, k, "{delay} ms");
    }
}

#[test]
fn a_death_restores_the_row_on_the_next_call_of_its_spawner() {
    let (level, plank) = plank_level((ROW.0 + 64, STAND_Y - 40), 5, 250);
    let mut sim = Sim::new(level, SimConfig::default());
    // Let the whole row go: Claw falls to the floor below.
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    let ids = pieces(&sim);
    assert!(ids.iter().all(|&id| piece_state(&sim, id).2), "all gone");
    let gone = sim.with_world(|w| (12..17).map(|c| w.main_cell(c, 11)).collect::<Vec<_>>());
    assert_eq!(gone, [-1; 5]);
    let attempt = sim.stats().attempt;
    sim.end_life();
    let (mut lives_step, mut restored) = (None, None);
    for k in 0..400 {
        sim.step(TickInput::default());
        if lives_step.is_none() && sim.stats().attempt != attempt {
            lives_step = Some(k);
        }
        if ids
            .iter()
            .all(|&id| piece_state(&sim, id) == (state::IDLE, 1, false))
        {
            restored = Some(k);
            break;
        }
    }
    // The spawner runs before Claw: it sees the lost life on the next
    // tick.
    assert!(lives_step.is_some(), "the life is lost");
    assert_eq!(restored, lives_step.map(|k| k + 1));
    assert_eq!(sim.stats().attempt, attempt + 1);
    let cells = sim.with_world(|w| (12..17).map(|c| w.main_cell(c, 11)).collect::<Vec<_>>());
    assert_eq!(cells, [plank; 5]);
    let d = break_plank::data_of(sim.objects().get(ids[1]).unwrap()).unwrap();
    assert!(!d.triggered);
}

#[test]
fn a_copy_of_the_game_keeps_its_own_planks() {
    let (level, plank) = plank_level((ROW.0 + 64, STAND_Y - 40), 5, 500);
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 20);
    let copy = sim.clone();
    for _ in 0..120 {
        sim.step(TickInput::default());
    }
    let cell = |s: &mut Sim| s.with_world(|w| w.main_cell(13, 11));
    assert_eq!(cell(&mut sim), -1);
    let mut copy = copy;
    assert_eq!(cell(&mut copy), plank, "the copy's floor is still there");
    assert_eq!(copy.level().map.cell(13, 11), plank);
    assert_eq!(sim.level().map.cell(13, 11), -1);
}

#[test]
fn a_cell_written_by_a_logic_is_read_back_at_once_and_reaches_the_level() {
    let (level, plank) = plank_level((300, 600), 1, 500);
    let mut sim = Sim::new(level, SimConfig::default());
    let read = sim.with_world(|w| {
        w.set_main_cell(12, 11, -1);
        w.main_cell(12, 11)
    });
    assert_eq!(read, -1);
    assert_eq!(sim.level().map.cell(12, 11), -1);
    sim.with_world(|w| w.set_main_cell(12, 11, plank));
    assert_eq!(sim.level().map.cell(12, 11), plank);
    // Outside the plane nothing is written.
    sim.with_world(|w| w.set_main_cell(-1, 500, plank));
    assert_eq!(sim.level().map.cell(-1, 500), -1);
}
