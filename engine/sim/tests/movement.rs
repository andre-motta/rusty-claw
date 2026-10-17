//! Movement feel on synthetic levels: walking, running, the variable jump,
//! climbing and falling against the tolerance table, and how catnip
//! changes them.

mod common;

use claw_sim::anim::AnimPlayer;
use claw_sim::player::death::powerup;
use claw_sim::player::{self, state};
use claw_sim::{Sim, SimConfig, Tuning};
use common::*;

#[test]
fn walk_and_stop() {
    let rows = run_level(flat_level(), &script(&[(RIGHT, 1000), (0, 600)]));
    let (a, b) = hold_window(&rows, RIGHT);
    let walk = speed(&rows, a + 150, b, false);
    let released = rows.iter().rfind(|r| r.t_ms <= b).unwrap().x;
    let slide = rows
        .iter()
        .filter(|r| r.t_ms > b)
        .map(|r| r.x)
        .max()
        .unwrap()
        - released;
    check(&[("walk_speed", walk), ("stop_slide_px", f64::from(slide))]);
    assert!(
        rows.iter().all(|r| r.y == rows[0].y),
        "walking stays on the floor"
    );
}

#[test]
fn run_onset_and_run_speed() {
    let rows = run_level(flat_level(), &script(&[(RIGHT, 3000), (0, 600)]));
    let (a, b) = hold_window(&rows, RIGHT);
    let walk = speed(&rows, a + 150, b.min(a + 1400), false);
    let run = speed(&rows, a + 1800, b, false);
    let onset = rows
        .iter()
        .find(|r| r.t_ms >= a && r.t_ms <= b && r.running_time <= 0)
        .unwrap()
        .t_ms;
    check(&[
        ("walk_speed", walk),
        ("run_speed", run),
        ("run_onset_ms", onset as f64),
    ]);
}

fn jump(hold_ms: u64) -> (i32, u64, u64) {
    let rows = run_level(flat_level(), &script(&[(JUMP, hold_ms), (0, 1500)]));
    jump_metrics(&rows)
}

#[test]
fn variable_jump() {
    let cases: [(&str, u64); 6] = [
        ("tap", 18),
        ("hold_50", 50),
        ("hold_100", 100),
        ("hold_162", 150),
        ("hold_216", 200),
        ("full", 800),
    ];
    let mut measured: Vec<(String, f64)> = Vec::new();
    for (name, ms) in cases {
        let (peak, ttp, air) = jump(ms);
        measured.push((format!("jump_{name}_peak"), f64::from(peak)));
        measured.push((format!("jump_{name}_time_to_peak_ms"), ttp as f64));
        measured.push((format!("jump_{name}_airtime_ms"), air as f64));
    }
    let refs: Vec<(&str, f64)> = measured.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    check(&refs);
}

#[test]
fn tapped_jump_peaks_at_the_base_height_at_any_tuning() {
    // The jump aims at its target height and stops there, so the tap peak
    // does not depend on gravity or the step.
    for tuning in [Tuning::default(), Tuning::original_speeds()] {
        let cfg = SimConfig {
            tuning,
            ..SimConfig::default()
        };
        let mut sim = pressing_sim(flat_level(), cfg);
        let rows = run(&mut sim, &script(&[(JUMP, 18), (0, 1000)]));
        assert_eq!(jump_metrics(&rows).0, 45);
    }
}

#[test]
fn original_speeds_full_hold_reaches_the_same_height() {
    let cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        ..SimConfig::default()
    };
    let mut sim = pressing_sim(flat_level(), cfg);
    let rows = run(&mut sim, &script(&[(JUMP, 800), (0, 1000)]));
    assert_eq!(jump_metrics(&rows).0, 145);
}

#[test]
fn running_jump_goes_higher() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    let s = script(&[(RIGHT, 1800), (RIGHT | JUMP, 800), (RIGHT, 1000)]);
    let rows = run(&mut sim, &s);
    let start = rows.iter().position(|r| r.state == state::JUMP).unwrap();
    let y0 = rows[start - 1].y;
    let top = rows.iter().map(|r| r.y).min().unwrap();
    assert_eq!(y0 - top, 171);
}

#[test]
fn climb_up_and_down() {
    let s = script(&[
        (UP | JUMP, 108),
        (UP, 1500),
        (0, 500),
        (DOWN, 1000),
        (0, 500),
    ]);
    let rows = run_level(ladder_level(), &s);
    let climbing = |mask: u32| -> f64 {
        let sel: Vec<Row> = rows
            .iter()
            .filter(|r| r.state == state::CLIMB && r.held & mask != 0)
            .skip(2)
            .cloned()
            .collect();
        let moving: Vec<&Row> = sel
            .windows(2)
            .filter(|w| w[1].y != w[0].y)
            .map(|w| &w[1])
            .collect();
        let (t0, t1) = (moving.first().unwrap().t_ms, moving.last().unwrap().t_ms);
        speed(&sel, t0, t1, true).abs()
    };
    check(&[
        ("climb_speed", climbing(UP)),
        ("climb_down_speed", climbing(DOWN)),
    ]);
}

#[test]
fn climbing_over_the_top_stands_on_the_ladder() {
    let s = script(&[(UP | JUMP, 108), (UP, 4000), (0, 300)]);
    let rows = run_level(ladder_level(), &s);
    let last = rows.last().unwrap();
    assert_eq!(last.state, state::STAND);
    // Ladder top is row 5 (y = 320); the feet rest on 319.
    assert_eq!(last.y + 60, 319);
}

/// A ledge at row 5 over a 3500 px drop.
fn tower() -> claw_sim::SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(20); 60];
    rows[5].replace_range(0..4, "####");
    rows.push("#".repeat(20));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    claw_sim::SimLevel::synthetic(claw_sim::TileMap::from_ascii(64, &rows), (100, 200))
}

#[test]
fn long_falls_reach_the_capped_speed_and_land() {
    let rows = run_level(tower(), &script(&[(RIGHT, 800), (0, 4000)]));
    assert_eq!(rows[0].y + 60, 319, "starts on the ledge");
    let fall: Vec<&Row> = rows.iter().filter(|r| r.state == state::FALL).collect();
    let dys: Vec<i32> = fall.windows(2).map(|w| w[1].y - w[0].y).collect();
    let top = *dys.iter().max().unwrap();
    let plateau: Vec<Row> = fall
        .windows(2)
        .filter(|w| f64::from(w[1].y - w[0].y) >= 0.93 * f64::from(top))
        .map(|w| w[1].clone())
        .collect();
    let (t0, t1) = (plateau[0].t_ms, plateau.last().unwrap().t_ms);
    check(&[("fall_terminal_vy", speed(&plateau, t0, t1, true))]);
    let landed = rows
        .iter()
        .position(|r| r.state == state::LAND)
        .expect("long falls land");
    assert_eq!(rows[landed].y + 60, 61 * 64 - 64 - 1);
    assert_eq!(rows.last().unwrap().state, state::STAND);
}

#[test]
fn built_in_table_flags_the_provisional_targets() {
    let table = builtin();
    let provisional: Vec<&str> = table
        .iter()
        .filter(|(_, t)| t.provisional)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        provisional,
        vec![
            "climb_down_speed",
            "climb_speed",
            "jump_hold_216_peak",
            "lookup_pan_delay_ms",
            "run_speed",
            "walk_speed",
        ]
    );
}

// ---------------------------------------------------------------- catnip

/// A simulation on `level` with `tuning` whose Claw has catnip for a
/// minute when `catnip` is set.
fn catnip_sim(level: claw_sim::SimLevel, tuning: Tuning, catnip: bool) -> Sim {
    let cfg = SimConfig {
        tuning,
        ..SimConfig::default()
    };
    let mut sim = pressing_sim(level, cfg);
    sim.step(claw_sim::TickInput::default());
    if catnip {
        sim.with_world(|w| {
            w.claw.stats.powerup_kind = powerup::CATNIP;
            w.claw.stats.powerup_ms = 60_000;
        });
    }
    sim
}

/// Claw's horizontal speed and walk animation speed after walking right
/// for `ms`.
fn walk_after(tuning: Tuning, catnip: bool, ms: u64) -> (i32, AnimPlayer) {
    let mut sim = catnip_sim(flat_level(), tuning, catnip);
    for _ in 0..ticks(ms) {
        sim.step(hold(RIGHT));
    }
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::MOVE_RIGHT);
    (o.speed_x, o.anim.clone())
}

/// The animation `a` with its speed set to `speed`.
fn at_speed(a: &AnimPlayer, speed: f32) -> AnimPlayer {
    let mut a = a.clone();
    a.set_speed(speed);
    a
}

#[test]
fn catnip_walks_at_300_where_claw_walks_at_225_and_runs_at_285() {
    let t = Tuning::original_speeds;
    // Walking.
    assert_eq!(walk_after(t(), false, 500).0, 225);
    assert_eq!(walk_after(t(), true, 500).0, 300);
    // Past the run onset: running without catnip, still 300 with it.
    assert_eq!(walk_after(t(), false, 2000).0, 285);
    assert_eq!(walk_after(t(), true, 2000).0, 300);
    // The tuned walk scales the same way.
    let tuned = Tuning::default().player.walk_speed;
    assert_eq!(walk_after(Tuning::default(), false, 500).0, tuned as i32);
    assert_eq!(
        walk_after(Tuning::default(), true, 500).0,
        (tuned * player::CATNIP_WALK_FACTOR) as i32
    );
}

#[test]
fn catnip_walk_animation_runs_at_0_8() {
    let (_, walk) = walk_after(Tuning::default(), false, 500);
    assert_eq!(walk, at_speed(&walk, player::WALK_ANIM_SPEED));
    let (_, run) = walk_after(Tuning::default(), false, 2000);
    assert_eq!(run, at_speed(&run, player::RUN_ANIM_SPEED));
    for ms in [500, 2000] {
        let (_, catnip) = walk_after(Tuning::default(), true, ms);
        assert_eq!(catnip, at_speed(&catnip, player::CATNIP_ANIM_SPEED));
        assert_ne!(catnip, at_speed(&catnip, player::WALK_ANIM_SPEED));
    }
}

/// Peak height of a standing jump with Jump held for `hold_ms`, and the
/// vertical speed Claw records while Jump is held.
fn catnip_jump(tuning: Tuning, catnip: bool, hold_ms: u64) -> (i32, i32) {
    let mut sim = catnip_sim(flat_level(), tuning, catnip);
    sim.step(press(JUMP));
    let speed_y = sim.player().unwrap().speed_y;
    let rest = run(&mut sim, &script(&[(JUMP, hold_ms), (0, 1500)]));
    let y0 = flat_level_floor_y();
    let top = rest.iter().map(|r| r.y).min().unwrap();
    (y0 - top, speed_y)
}

/// Claw's Y standing on [`flat_level`]'s floor.
fn flat_level_floor_y() -> i32 {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 5);
    sim.player().unwrap().y
}

#[test]
fn catnip_full_hold_jump_peaks_at_195_and_a_tap_at_45() {
    for tuning in [Tuning::default(), Tuning::original_speeds()] {
        assert_eq!(catnip_jump(tuning.clone(), false, 800), (145, -425));
        assert_eq!(catnip_jump(tuning.clone(), true, 800), (195, -500));
        assert_eq!(catnip_jump(tuning.clone(), false, 0).0, 45);
        assert_eq!(catnip_jump(tuning, true, 0).0, 45);
    }
}

#[test]
fn catnip_beats_the_running_jump() {
    let jump = |catnip: bool| {
        let mut sim = catnip_sim(flat_level(), Tuning::default(), catnip);
        let rows = run(
            &mut sim,
            &script(&[(RIGHT, 1800), (RIGHT | JUMP, 800), (RIGHT, 1000)]),
        );
        let start = rows.iter().position(|r| r.state == state::JUMP).unwrap();
        let y0 = rows[start - 1].y;
        y0 - rows.iter().map(|r| r.y).min().unwrap()
    };
    assert_eq!(jump(false), 171);
    assert_eq!(jump(true), 195);
}

#[test]
fn super_jump_switch_peaks_at_195_without_catnip() {
    let mut sim = catnip_sim(flat_level(), Tuning::default(), false);
    sim.with_world(|w| w.claw.movement.super_jump = true);
    let y0 = sim.player().unwrap().y;
    sim.step(press(JUMP));
    assert_eq!(sim.player().unwrap().speed_y, player::JUMP_HOLD_SPEED_SUPER);
    let rows = run(&mut sim, &script(&[(JUMP, 800), (0, 1500)]));
    assert_eq!(y0 - rows.iter().map(|r| r.y).min().unwrap(), 195);
}

/// Claw's vertical speed on the ladder after holding `mask` for a while.
fn climb_speed_y(tuning: Tuning, catnip: bool, mask: u32) -> i32 {
    let mut sim = catnip_sim(ladder_level(), tuning, catnip);
    let mut rows = run(&mut sim, &script(&[(UP | JUMP, 108), (UP, 500)]));
    if mask == DOWN {
        rows = run(&mut sim, &script(&[(DOWN, 200)]));
    }
    assert_eq!(rows.last().unwrap().state, state::CLIMB);
    sim.player().unwrap().speed_y
}

#[test]
fn catnip_climbs_at_250_where_claw_climbs_at_200() {
    let t = Tuning::original_speeds;
    assert_eq!(climb_speed_y(t(), false, UP), -200);
    assert_eq!(climb_speed_y(t(), true, UP), -250);
    assert_eq!(climb_speed_y(t(), false, DOWN), 200);
    assert_eq!(climb_speed_y(t(), true, DOWN), 250);
    let tp = Tuning::default().player;
    assert_eq!(
        climb_speed_y(Tuning::default(), true, UP),
        (-tp.climb_up_speed * player::CATNIP_CLIMB_FACTOR) as i32
    );
    assert_eq!(
        climb_speed_y(Tuning::default(), true, DOWN),
        (tp.climb_down_speed * player::CATNIP_CLIMB_FACTOR) as i32
    );
}

#[test]
fn walking_goes_back_to_225_when_catnip_runs_out() {
    let mut sim = catnip_sim(flat_level(), Tuning::original_speeds(), true);
    sim.with_world(|w| w.claw.stats.powerup_ms = 200);
    for _ in 0..5 {
        sim.step(hold(RIGHT));
    }
    assert_eq!(sim.player().unwrap().speed_x, 300);
    for _ in 0..15 {
        sim.step(hold(RIGHT));
    }
    assert_eq!(sim.stats().powerup_kind, 0);
    assert_eq!(sim.player().unwrap().speed_x, 225);
}
