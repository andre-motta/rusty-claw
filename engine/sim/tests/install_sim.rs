//! Tests against the user's own install. They read the folder named by the
//! `CLAW_INSTALL` environment variable at run time and skip themselves when
//! it is unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;

use claw_assets::level::RETAIL_LEVELS;
use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::BREAKABLE_LOGICS;
use claw_sim::player::death::powerup;
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
fn install_level1_catnip_full_hold_jump_peaks_at_195() {
    let Some(store) = store() else { return };
    let jump = |catnip: bool, ms: u64| {
        let mut sim = at(&store, FLAT.0, FLAT.1);
        if catnip {
            // As long as the larger catnip pickup lasts.
            sim.with_world(|w| {
                w.claw.stats.powerup_kind = powerup::CATNIP;
                w.claw.stats.powerup_ms = 30_000;
            });
        }
        let rows = run(&mut sim, &script(&[(JUMP, ms), (0, 1500)]));
        assert_eq!(rows.last().unwrap().y, rows[0].y, "lands where it started");
        jump_metrics(&rows).0
    };
    assert_eq!(jump(false, 800), 145);
    assert_eq!(jump(true, 800), 195);
    assert_eq!(jump(true, 18), 45);
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

// ------------------------------------------------- level logics on and off

/// FNV-1a 64 of a text.
fn digest(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The trace columns of the first playable version, one line per row.
fn movement_columns(rows: &[TraceRow]) -> String {
    let mut s = String::new();
    for r in rows {
        let csv = r.to_csv();
        let line: Vec<&str> = csv.splitn(16, ',').take(15).collect();
        s.push_str(&line.join(","));
        s.push('\n');
    }
    s
}

fn movement_digest(mut sim: Sim, start: Option<(i32, i32)>, s: &InputScript) -> u64 {
    if let Some((x, y)) = start {
        sim.teleport_player(x, y);
        idle(&mut sim, 60);
    }
    let rows: Vec<TraceRow> = s
        .ticks
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            sim.step(t);
            TraceRow::capture(&sim, i as u64, "script")
        })
        .collect();
    digest(&movement_columns(&rows))
}

fn explore() -> InputScript {
    script(&[
        (RIGHT, 4000),
        (RIGHT | JUMP, 800),
        (RIGHT, 3000),
        (RIGHT | JUMP, 800),
        (RIGHT, 6000),
        (LEFT | JUMP, 800),
        (LEFT, 2000),
        (UP, 1000),
        (DOWN, 1000),
        (UP | JUMP, 300),
        (0, 1000),
    ])
}

fn without_logics() -> SimConfig {
    SimConfig {
        level_logics: false,
        ..SimConfig::default()
    }
}

/// With level logics off, Claw moves exactly as in the first playable
/// version: the movement columns of these runs hash to the values that
/// version produced.
#[test]
fn install_without_level_logics_movement_is_unchanged() {
    let Some(store) = store() else { return };
    let explore_digests: [u64; 14] = [
        0x1ff6_066e_b8a9_4865,
        0x2fab_7156_cf88_422a,
        0xb574_88ac_96b7_382a,
        0xfd2e_ed1f_48d6_a213,
        0x786b_be78_b388_dcfb,
        0x436d_53e2_5424_1cf5,
        0x135d_f10a_d2a7_55ab,
        0x4a2b_3008_8039_e693,
        0x85a0_7fd1_3fd2_b63d,
        0x9d13_5a37_2262_f64c,
        0xf730_ce72_d752_7402,
        0x90c3_7621_befd_7943,
        0x332e_540b_01a7_c935,
        0xbdbe_4b08_c469_1476,
    ];
    let mut changed = Vec::new();
    for (n, want) in RETAIL_LEVELS.zip(explore_digests) {
        let got = movement_digest(
            Sim::new(level(&store, n), without_logics()),
            None,
            &explore(),
        );
        if got != want {
            changed.push(format!("LEVEL{n} explore"));
        }
    }
    let l1 = level(&store, 1);
    let flat = script(&[
        (RIGHT, 3000),
        (0, 600),
        (JUMP, 800),
        (0, 1000),
        (DOWN, 3500),
        (0, 2000),
        (UP, 3500),
        (0, 2000),
        (LEFT | JUMP, 200),
        (LEFT, 1500),
    ]);
    let climb = script(&[
        (UP | JUMP, 108),
        (UP, 1500),
        (0, 500),
        (DOWN, 1000),
        (0, 500),
    ]);
    let wide = SimConfig {
        activation: ActivationConfig::widened((1706, 480)),
        ..without_logics()
    };
    let runs = [
        (
            "flat",
            movement_digest(Sim::new(l1.clone(), without_logics()), Some(FLAT), &flat),
            0x9193_7967_edf7_d26f,
        ),
        (
            "ladder",
            movement_digest(
                Sim::new(l1.clone(), without_logics()),
                Some((1440, 4760)),
                &climb,
            ),
            0x9bb4_553a_3ab8_16d2,
        ),
        (
            "widened",
            movement_digest(Sim::new(l1, wide), None, &explore()),
            0x5970_dea8_afcb_6061,
        ),
    ];
    for (name, got, want) in runs {
        if got != want {
            changed.push(name.to_string());
        }
    }
    assert!(
        changed.is_empty(),
        "movement traces changed with level logics off: {changed:?}"
    );
}

/// A long, varied input, the same every run.
fn wander(ticks: usize) -> Vec<TickInput> {
    let mut rng = claw_sim::rng::Rng::new(7);
    let moves = [RIGHT, RIGHT | JUMP, LEFT, LEFT | JUMP, UP, DOWN, 0, JUMP];
    let mut out = Vec::with_capacity(ticks);
    let mut held = 0u32;
    for i in 0..ticks {
        let prev = held;
        if i % 40 == 0 {
            held = moves[rng.below(moves.len() as u32) as usize];
        }
        out.push(TickInput {
            held: claw_sim::Buttons(held),
            pressed: claw_sim::Buttons(held & !prev),
        });
    }
    out
}

#[test]
fn install_level1_runs_with_level_logics() {
    let Some(store) = store() else { return };
    let l = level(&store, 1);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let d = sim.diagnostics().clone();
    assert!(d.unknown_logics.is_empty(), "{:?}", d.unknown_logics);
    assert!(
        d.inert_logics.is_empty(),
        "every logic binds: {:?}",
        d.inert_logics
    );
    let bound = sim
        .objects()
        .all()
        .filter(|(_, o)| matches!(o.kind, claw_sim::object::LogicKind::Game(_)))
        .count();
    assert_eq!(bound, l.objects.len(), "every placed object runs a family");
    let inputs = wander(2000);
    let t = std::time::Instant::now();
    let mut ms = Vec::with_capacity(inputs.len());
    let mut live = 0;
    for &input in &inputs {
        let s = std::time::Instant::now();
        sim.step(input);
        ms.push(s.elapsed().as_secs_f64() * 1000.0);
        live = live.max(sim.objects().listed().len());
    }
    let total = t.elapsed();
    ms.sort_by(f64::total_cmp);
    println!(
        "LEVEL1 with level logics: 2000 ticks in {total:?}, tick mean {:.4} ms, p99 {:.4} ms, max {:.4} ms, up to {live} live objects",
        ms.iter().sum::<f64>() / ms.len() as f64,
        ms[ms.len() * 99 / 100],
        ms[ms.len() - 1]
    );
    assert_eq!(sim.tick(), 2000);
    assert_eq!(sim.snapshot().hud.health, sim.stats().health);
    // The level's animations and sounds are loaded for its logics.
    assert!(l.animations.keys().any(|k| k.starts_with("LEVEL_")));
    assert!(l.sounds.values().filter(|ms| ms.is_some()).count() > 100);
    assert!(
        l.skipped_animations.is_empty(),
        "{:?}",
        l.skipped_animations
    );
    // Two runs agree, and the families' stubs leave the movement alone.
    let mut again = Sim::new(l.clone(), SimConfig::default());
    let mut off = Sim::new(l, without_logics());
    for (i, &input) in inputs.iter().enumerate().take(600) {
        again.step(input);
        off.step(input);
        let a = TraceRow::capture(&again, i as u64, "script");
        let b = TraceRow::capture(&off, i as u64, "script");
        assert_eq!(a, b, "tick {i}");
    }
}

#[test]
fn install_every_level_loads_and_runs_with_level_logics() {
    let Some(store) = store() else { return };
    let inputs = wander(300);
    for n in RETAIL_LEVELS {
        let mut sim = Sim::new(level(&store, n), SimConfig::default());
        for &input in &inputs {
            sim.step(input);
        }
        assert_eq!(sim.tick(), 300, "LEVEL{n}");
        let d = sim.diagnostics();
        println!(
            "LEVEL{n}: {} objects, {} with logics no family runs yet ({} names), {} live",
            sim.objects().len(),
            d.inert_logics.values().sum::<usize>(),
            d.inert_logics.len(),
            sim.objects().listed().len()
        );
    }
}
