//! The player state table: spawning, crouching, looking up, teetering,
//! fallback rows, the mid-air hit jump fix and respawning.

mod common;

use claw_sim::object::Rect;
use claw_sim::physics::ptype;
use claw_sim::player::{RowKind, row_of, state};
use claw_sim::tiles::TileMap;
use claw_sim::tuning::Behaviour;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, Tuning};
use common::*;

#[test]
fn spawn_uses_the_start_point_and_drops_to_the_floor() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y), (1000, 959 - 60));
    assert_eq!(o.state, state::STAND);
    assert_eq!(sim.claw().spawn_point, (1000, 899));
    let snap = sim.step(TickInput::default());
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y), (1000, 899));
    let claw = snap
        .renderables
        .iter()
        .find(|r| r.image_set_key == "CLAW")
        .unwrap();
    assert!(
        claw.teleported,
        "the spawn is not interpolated from the creation spot"
    );
    assert_eq!(claw.z, 4000);
}

#[test]
fn start_points_outside_the_plane_are_clamped() {
    let map = TileMap::from_ascii(64, &["....", "....", "####"]);
    let sim = pressing_sim(SimLevel::synthetic(map, (5000, -40)), SimConfig::default());
    let o = sim.player().unwrap();
    // The centre clamps to the plane (x = 255); the first post-tick step
    // then keeps Claw 50 px inside it.
    assert_eq!(o.x, 256 - 50);
    assert_eq!(o.y + 60, 127);
}

#[test]
fn claw_stays_fifty_pixels_inside_the_plane() {
    let rows = run_level(flat_level(), &script(&[(LEFT, 6000)]));
    assert_eq!(rows.last().unwrap().x, 50);
}

#[test]
fn crouch_and_stand_up() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    sim.step(press(DOWN));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::CROUCH);
    assert_eq!(o.physics_type, ptype::DUCK);
    assert_eq!(o.move_rect, Rect::new(-16, 10, 16, 60));
    sim.step(hold(DOWN | LEFT));
    assert!(
        sim.player().unwrap().draw_flags & 0x2 != 0,
        "turns in place"
    );
    assert_eq!(sim.player().unwrap().x, 1000);
    sim.step(TickInput::default());
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::STAND);
    assert_eq!(o.move_rect, Rect::new(-16, -52, 16, 60));
}

#[test]
fn look_up_and_jump_from_it() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    for _ in 0..37 {
        sim.step(hold(UP));
    }
    assert_eq!(sim.player().unwrap().state, state::LOOK_UP);
    sim.step(press(UP | JUMP));
    assert_eq!(sim.player().unwrap().state, state::JUMP);
    assert!(sim.player().unwrap().y < 899);
}

/// A ledge: floor under columns 0..9, a drop to the right.
fn ledge_level() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(30); 10];
    rows[9] = format!("{}{}", "#".repeat(10), ".".repeat(20));
    rows.push("#".repeat(30));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (600, 100))
}

#[test]
fn standing_over_a_ledge_teeters() {
    // Box (-16..16): at x = 650 only the left edge (634) is over the floor
    // that ends at x = 639.
    let mut sim = pressing_sim(ledge_level(), SimConfig::default());
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::STAND);
    let y = sim.player().unwrap().y;
    sim.teleport_player(650, y);
    sim.step(TickInput::default());
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::ON_EDGE);
    assert_eq!(o.draw_flags & 0x2, 0, "faces the drop on the right");
    idle(&mut sim, 5);
    assert_eq!(sim.player().unwrap().state, state::ON_EDGE);
    sim.step(press(LEFT));
    assert_eq!(sim.player().unwrap().state, state::MOVE_LEFT);
}

#[test]
fn walking_off_a_ledge_falls_and_lands() {
    let rows = run_level(ledge_level(), &script(&[(RIGHT, 600), (0, 1500)]));
    assert!(rows.iter().any(|r| r.state == state::FALL));
    let last = rows.last().unwrap();
    assert_eq!(last.y + 60, 10 * 64 - 1);
    assert_eq!(last.state, state::STAND);
}

#[test]
fn every_state_id_has_a_row() {
    for id in (0..=31).chain(5000..=5010) {
        assert_eq!(row_of(id).id, id);
    }
    assert_eq!(row_of(state::HIT).kind, RowKind::Fallback);
    assert_eq!(row_of(state::STAND).kind, RowKind::Implemented);
    assert_eq!(row_of(2).kind, RowKind::Unused);
    assert_eq!(
        row_of(state::STAND).animations,
        &["CLAW_STAND", "CLAW_IDLE"]
    );
    assert_eq!(row_of(state::CLIMB).physics_type, Some(ptype::CLIMB));
}

#[test]
fn unused_ids_run_only_the_post_tick_step() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    sim.set_player_state(2);
    sim.step(hold(RIGHT));
    let o = sim.player().unwrap();
    assert_eq!((o.state, o.x), (2, 1000));
}

#[test]
fn later_milestone_states_fall_back_to_stand() {
    for id in [
        state::LIFT,
        state::THROW,
        state::DEATH_FALL,
        state::DEATH,
        state::SWING,
        state::BEARHUG,
        state::FREEZE,
        state::HIT,
    ] {
        let mut sim = pressing_sim(flat_level(), SimConfig::default());
        sim.set_player_state(id);
        sim.step(TickInput::default());
        assert_eq!(sim.player().unwrap().state, state::STAND, "state {id}");
        assert_eq!(sim.claw().fallback_entries, 1);
    }
}

fn fix_config(fix: bool) -> SimConfig {
    SimConfig {
        tuning: Tuning {
            behaviour: Behaviour {
                fix_double_jump: fix,
            },
            ..Tuning::default()
        },
        ..SimConfig::default()
    }
}

fn hit_in_mid_air(cfg: SimConfig) -> Sim {
    let mut sim = pressing_sim(flat_level(), cfg);
    sim.step(press(JUMP));
    for _ in 0..3 {
        sim.step(hold(JUMP));
    }
    assert!(sim.player().unwrap().y < 899 - 20);
    sim.set_player_state(state::HIT);
    sim.step(TickInput::default());
    sim
}

#[test]
fn hit_ending_in_mid_air_lands_with_the_fix() {
    let mut sim = hit_in_mid_air(fix_config(true));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::LAND, "Land, not Stand, in mid-air");
    let y = o.y;
    sim.step(press(JUMP));
    assert_eq!(sim.player().unwrap().state, state::FALL, "no second jump");
    assert!(sim.player().unwrap().y >= y);
}

#[test]
fn hit_ending_on_the_ground_stands_either_way() {
    for cfg in [SimConfig::default(), fix_config(true)] {
        let mut sim = pressing_sim(flat_level(), cfg);
        sim.step(TickInput::default());
        let before = sim.player().unwrap().clone();
        sim.set_player_state(state::HIT);
        sim.step(TickInput::default());
        let o = sim.player().unwrap();
        assert_eq!(o.state, state::STAND);
        assert_eq!((o.x, o.y), (before.x, before.y));
    }
}

#[test]
fn the_original_mid_air_hit_jump_is_the_default() {
    for cfg in [SimConfig::default(), fix_config(false)] {
        let mut sim = hit_in_mid_air(cfg);
        assert_eq!(sim.player().unwrap().state, state::STAND);
        let y = sim.player().unwrap().y;
        sim.step(press(JUMP));
        assert_eq!(sim.player().unwrap().state, state::JUMP);
        assert!(sim.player().unwrap().y < y, "jumps again in mid-air");
    }
}

#[test]
fn respawn_returns_to_the_spawn_point_or_checkpoint() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    for _ in 0..50 {
        sim.step(hold(RIGHT));
    }
    assert!(sim.player().unwrap().x > 1100);
    sim.respawn();
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y, o.state), (1000, 899, state::STAND));
    sim.set_checkpoint(Some((3000, 100)));
    sim.respawn();
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y), (3000, 899));
    let snap = sim.step(TickInput::default());
    assert!(snap.renderables.iter().any(|r| r.teleported));
}

#[test]
fn long_jumps_land_walking_when_a_direction_is_held() {
    let rows = run_level(flat_level(), &script(&[(JUMP | RIGHT, 300), (RIGHT, 1000)]));
    let after = rows
        .iter()
        .skip_while(|r| r.state != state::FALL)
        .find(|r| r.state != state::FALL)
        .unwrap();
    assert_eq!(after.state, state::MOVE_RIGHT);
}
