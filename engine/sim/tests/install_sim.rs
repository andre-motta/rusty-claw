//! Tests against the user's own install. They read the folder named by the
//! `CLAW_INSTALL` environment variable at run time and skip themselves when
//! it is unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;

use claw_assets::level::RETAIL_LEVELS;
use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::BREAKABLE_LOGICS;
use claw_sim::player::state;
use claw_sim::tiles::{CLIMB, GROUND, SOLID};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{ActivationConfig, InputScript, Sim, SimConfig, SimLevel, TickInput};
use common::*;

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level(store: &AssetStore, n: u32) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// Where the level start should put Claw: the start point clamped to the
/// plane, dropped onto the first floor below it in its column.
fn expected_spawn(l: &SimLevel) -> (i32, i32) {
    let x = l.start.0.clamp(0, l.map.width() - 1);
    let y = l.start.1.clamp(0, l.map.height() - 1);
    let feet = y + 60;
    let floor = (feet..l.map.height())
        .find(|&yy| matches!(l.map.attr(x, yy), SOLID | GROUND | CLIMB))
        .expect("a floor below the start");
    (x, y + floor - feet - 1)
}

#[test]
fn install_spawn_follows_the_start_point_in_every_level() {
    let Some(store) = store() else { return };
    for n in RETAIL_LEVELS {
        let l = level(&store, n);
        let want = expected_spawn(&l);
        let mut sim = Sim::new(l, SimConfig::default());
        sim.step(TickInput::default());
        let o = sim.player().unwrap();
        assert_eq!((o.x, o.y), want, "LEVEL{n}");
        assert_eq!(o.state, state::STAND, "LEVEL{n}");
        assert_eq!(sim.claw().spawn_point, want, "LEVEL{n}");
        assert_eq!(sim.camera().current, want, "LEVEL{n}");
        idle(&mut sim, 200);
        let o = sim.player().unwrap();
        assert_eq!((o.x, o.y), want, "LEVEL{n} stays put");
    }
}

#[test]
fn install_level1_start() {
    let Some(store) = store() else { return };
    let l = level(&store, 1);
    assert_eq!(l.start, (689, 4723));
    assert!(l.animations.contains_key("CLAW_STAND"));
    let mut sim = Sim::new(l, SimConfig::default());
    let snap = sim.step(TickInput::default());
    let claw = snap
        .renderables
        .iter()
        .find(|r| r.image_set_key == "CLAW")
        .expect("Claw is drawn");
    assert_eq!((claw.x, claw.y), (689.0, 4730.0));
    assert!(
        (11..=18).contains(&claw.frame),
        "standing frame {}",
        claw.frame
    );
}

#[test]
fn install_unregistered_logics_become_placeholders() {
    let Some(store) = store() else { return };
    let mut per_level = Vec::new();
    for n in RETAIL_LEVELS {
        let sim = Sim::new(level(&store, n), SimConfig::default());
        let d = sim.diagnostics();
        assert_eq!(d.dropped_records, 0, "LEVEL{n}");
        let count: usize = d.unknown_logics.values().sum();
        if count > 0 {
            per_level.push((n, d.unknown_logics.len(), count));
        }
        if n == 12 {
            assert_eq!(d.unknown_logics.get("Sign"), Some(&3));
        }
    }
    // One enemy logic of levels 9 and 10 is not in the built-in table yet
    // (it runs as a placeholder in its own activation class, 0), and
    // level 12 names a logic the game never registered.
    assert_eq!(per_level, vec![(9, 1, 45), (10, 1, 48), (12, 1, 3)]);
}

/// A simulation of LEVEL1 with Claw moved to (x, y) and settled.
fn at(store: &AssetStore, x: i32, y: i32) -> Sim {
    let mut sim = Sim::new(level(store, 1), SimConfig::default());
    sim.teleport_player(x, y);
    idle(&mut sim, 60);
    sim
}

/// A long flat stretch of LEVEL1.
const FLAT: (i32, i32) = (6272, 3150);

#[test]
fn install_level1_walk_and_run() {
    let Some(store) = store() else { return };
    let mut sim = at(&store, FLAT.0, FLAT.1);
    let rows = run(&mut sim, &script(&[(RIGHT, 1000), (0, 600)]));
    let (a, b) = hold_window(&rows, RIGHT);
    check(&[("walk_speed", speed(&rows, a + 150, b, false))]);
    let mut sim = at(&store, FLAT.0, FLAT.1);
    let rows = run(&mut sim, &script(&[(RIGHT, 3000), (0, 600)]));
    let (a, b) = hold_window(&rows, RIGHT);
    check(&[
        ("walk_speed", speed(&rows, a + 150, b.min(a + 1400), false)),
        ("run_speed", speed(&rows, a + 1800, b, false)),
    ]);
}

#[test]
fn install_level1_ledge() {
    let Some(store) = store() else { return };
    let rows = run_level(level(&store, 1), &script(&[(RIGHT, 900), (0, 1200)]));
    let y0 = rows[0].y;
    let y1 = rows.last().unwrap().y;
    let first = rows
        .iter()
        .find(|r| r.y > y0)
        .expect("drops off the step")
        .t_ms;
    let landed = rows
        .iter()
        .find(|r| r.y == y1 && r.t_ms >= first)
        .unwrap()
        .t_ms;
    check(&[
        ("ledge_drop_px", f64::from(y1 - y0)),
        ("ledge_fall_ms", (landed - first) as f64),
    ]);
}

#[test]
fn install_level1_jumps() {
    let Some(store) = store() else { return };
    let mut measured = Vec::new();
    for (name, ms) in [
        ("tap", 18),
        ("hold_50", 50),
        ("hold_100", 100),
        ("hold_162", 150),
        ("hold_216", 200),
        ("full", 800),
    ] {
        let mut sim = at(&store, FLAT.0, FLAT.1);
        let rows = run(&mut sim, &script(&[(JUMP, ms), (0, 1500)]));
        let (peak, ttp, air) = jump_metrics(&rows);
        measured.push((format!("jump_{name}_peak"), f64::from(peak)));
        measured.push((format!("jump_{name}_time_to_peak_ms"), ttp as f64));
        measured.push((format!("jump_{name}_airtime_ms"), air as f64));
    }
    let refs: Vec<(&str, f64)> = measured.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    check(&refs);
}

#[test]
fn install_level1_camera_looks() {
    let Some(store) = store() else { return };
    let mut sim = at(&store, FLAT.0, FLAT.1);
    let rows = run(
        &mut sim,
        &script(&[(DOWN, 3500), (0, 2000), (UP, 3500), (0, 2000)]),
    );
    let down_peak = rows.iter().skip(1).map(|r| r.cam_y - r.y).max().unwrap();
    let up_peak = rows.iter().skip(1).map(|r| r.cam_y - r.y).min().unwrap();
    check(&[
        ("crouch_max_offset_px", f64::from(down_peak)),
        ("lookup_max_offset_px", f64::from(up_peak)),
    ]);
}

#[test]
fn install_level1_climb() {
    let Some(store) = store() else { return };
    let mut sim = at(&store, 1440, 4760);
    let s = script(&[
        (UP | JUMP, 108),
        (UP, 1500),
        (0, 500),
        (DOWN, 1000),
        (0, 500),
    ]);
    let rows = run(&mut sim, &s);
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

fn trace_text(store: &AssetStore, n: u32, script: &InputScript) -> String {
    let mut sim = Sim::new(level(store, n), SimConfig::default());
    let rows: Vec<TraceRow> = script
        .ticks
        .iter()
        .map(|&i| {
            sim.step(i);
            TraceRow::capture(&sim, sim.tick() - 1, "script")
        })
        .collect();
    trace::to_csv(&rows)
}

#[test]
fn install_replays_are_identical() {
    let Some(store) = store() else { return };
    let s = script(&[
        (RIGHT, 2000),
        (RIGHT | JUMP, 400),
        (RIGHT, 1500),
        (0, 300),
        (LEFT, 1200),
        (DOWN, 1200),
        (UP | JUMP, 300),
        (0, 1000),
    ]);
    for n in [1, 5, 12] {
        assert_eq!(
            trace_text(&store, n, &s),
            trace_text(&store, n, &s),
            "LEVEL{n}"
        );
    }
}

#[test]
fn install_widened_activation_keeps_breakables_on_the_original_window() {
    let Some(store) = store() else { return };
    for n in [1, 10, 14] {
        let l = level(&store, n);
        let live = |cfg: ActivationConfig| -> (usize, Vec<String>) {
            let sim = Sim::new(
                l.clone(),
                SimConfig {
                    activation: cfg,
                    ..SimConfig::default()
                },
            );
            let ids = sim.objects().listed();
            let breakables: Vec<String> = ids
                .iter()
                .map(|&id| sim.objects().get(id).unwrap())
                .filter(|o| BREAKABLE_LOGICS.contains(&&*o.logic))
                .map(|o| format!("{}@{},{}", o.logic, o.x, o.y))
                .collect();
            (ids.len(), breakables)
        };
        let (n_orig, b_orig) = live(ActivationConfig::original());
        let (n_wide, b_wide) = live(ActivationConfig::widened((1706, 480)));
        assert!(n_wide >= n_orig, "LEVEL{n}: widening wakes more objects");
        assert_eq!(
            b_orig, b_wide,
            "LEVEL{n}: breakables wake as in the original"
        );
    }
}

#[test]
fn install_level1_never_inside_solid_while_exploring() {
    let Some(store) = store() else { return };
    let l = level(&store, 1);
    let map = l.map.clone();
    let mut sim = Sim::new(l, SimConfig::default());
    let s = script(&[
        (RIGHT, 4000),
        (RIGHT | JUMP, 800),
        (RIGHT, 3000),
        (RIGHT | JUMP, 800),
        (RIGHT, 6000),
        (LEFT | JUMP, 800),
        (LEFT, 2000),
        (UP, 1000),
        (DOWN, 1000),
    ]);
    for (i, input) in s.ticks.iter().enumerate() {
        sim.step(*input);
        let o = sim.player().unwrap();
        for y in [o.y + o.move_rect.top, o.y, o.y + o.move_rect.bottom] {
            assert_ne!(map.attr(o.x, y), SOLID, "tick {i} at ({}, {})", o.x, o.y);
        }
    }
}
