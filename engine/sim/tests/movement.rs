//! Movement feel on synthetic levels: walking, running, the variable jump,
//! climbing and falling against the tolerance table.

mod common;

use claw_sim::player::state;
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
        let mut sim = Sim::new(flat_level(), cfg);
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
    let mut sim = Sim::new(flat_level(), cfg);
    let rows = run(&mut sim, &script(&[(JUMP, 800), (0, 1000)]));
    assert_eq!(jump_metrics(&rows).0, 145);
}

#[test]
fn running_jump_goes_higher() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
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
