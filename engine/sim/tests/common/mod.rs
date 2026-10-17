//! Shared test helpers: synthetic levels built in code, scripted runs,
//! movement metrics and the feel tolerances.
//!
//! The tolerances below are the movement targets of the project: how far
//! and how fast Claw moves in the original played at 60 frames per second,
//! each with the spread the measurement allows. Set `CLAW_TOLERANCES` to a
//! TOML file with one table per metric (`value`, `tolerance`, optionally
//! `provisional`, `pending_decision` and `cap60_mean`) to check against
//! another table; for speeds marked `pending_decision = "speed policy"`
//! the 60 fps mean is the target.

#![allow(dead_code)]

use std::collections::BTreeMap;

use claw_sim::object::Rect;
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::tiles::{Descriptor, GROUND, TileMap};
use claw_sim::{Buttons, InputScript, Sim, SimConfig, SimLevel, TickInput};

pub const TICK: u64 = 18;
pub const JUMP: u32 = 0x1;
pub const LEFT: u32 = 0x100_0000;
pub const RIGHT: u32 = 0x200_0000;
pub const UP: u32 = 0x400_0000;
pub const DOWN: u32 = 0x800_0000;

/// Ticks needed to hold a key for at least `ms`.
pub fn ticks(ms: u64) -> usize {
    ms.div_ceil(TICK).max(1) as usize
}

/// A script of (mask, duration in ms) segments.
pub fn script(segments: &[(u32, u64)]) -> InputScript {
    let mut held = Vec::new();
    for &(mask, ms) in segments {
        held.extend(std::iter::repeat_n(mask, ticks(ms)));
    }
    InputScript::from_held(&held)
}

/// A long flat level: a Solid floor whose top is at y = 960 (feet rest on
/// 959), start point above it.
pub fn flat_level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// A level with a ladder: Climb tiles in column 20 from row 5 to row 14,
/// Solid floor at row 15. Claw starts at the ladder's foot.
pub fn ladder_level() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(60); 15];
    for r in rows.iter_mut().take(15).skip(5) {
        r.replace_range(20..21, "H");
    }
    rows.push("#".repeat(60));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (20 * 64 + 31, 900))
}

/// A Ground platform tile whose top `rows` pixel rows are Ground.
pub fn thin_ground(map: &mut TileMap, rows: i32) -> i32 {
    map.add_descriptor(Descriptor::Double {
        width: 64,
        height: 64,
        outside: 0,
        inside: GROUND,
        rect: [0, 0, 63, rows - 1],
    })
}

/// One recorded tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub t_ms: u64,
    pub x: i32,
    pub y: i32,
    pub state: i32,
    pub ptype: i32,
    pub cam_x: i32,
    pub cam_y: i32,
    pub held: u32,
    pub running_time: i32,
}

pub fn row_of(sim: &Sim) -> Row {
    let o = sim.player().expect("player");
    Row {
        t_ms: sim.tick() * TICK,
        x: o.x,
        y: o.y,
        state: o.state,
        ptype: o.physics_type,
        cam_x: sim.camera().current.0,
        cam_y: sim.camera().current.1,
        held: sim.input().held.0,
        running_time: sim.claw().running_time,
    }
}

/// Runs `script` and returns the start row (t = 0) and one row per tick.
pub fn run(sim: &mut Sim, script: &InputScript) -> Vec<Row> {
    let start = sim.tick() * TICK;
    let mut rows = vec![row_of(sim)];
    for &input in &script.ticks {
        sim.step(input);
        rows.push(row_of(sim));
    }
    for r in &mut rows {
        r.t_ms -= start;
    }
    rows
}

/// Runs `script` on a fresh simulation of `level`.
pub fn run_level(level: SimLevel, script: &InputScript) -> Vec<Row> {
    let mut sim = pressing_sim(level, SimConfig::default());
    run(&mut sim, script)
}

/// A fresh simulation of `level` in which keys count from the first tick:
/// the latch on keys down when play starts is lifted
/// ([`Sim::unlatch_keys`]), for tests that press from the first tick.
pub fn pressing_sim(level: SimLevel, cfg: SimConfig) -> Sim {
    let mut sim = Sim::new(level, cfg);
    sim.unlatch_keys();
    sim
}

pub fn idle(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

pub fn press(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask),
    }
}

pub fn hold(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons::NONE,
    }
}

// ---------------------------------------------------------------- metrics

/// Least-squares slope of `ys` over `ts`, scaled to per second.
pub fn slope_per_s(ts: &[f64], ys: &[f64]) -> Option<f64> {
    let n = ts.len() as f64;
    if ts.len() < 2 {
        return None;
    }
    let mt = ts.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = ts.iter().map(|t| (t - mt) * (t - mt)).sum();
    if sxx == 0.0 {
        return None;
    }
    let sxy: f64 = ts.iter().zip(ys).map(|(t, y)| (t - mt) * (y - my)).sum();
    Some(sxy / sxx * 1000.0)
}

pub fn speed(rows: &[Row], t0: u64, t1: u64, vertical: bool) -> f64 {
    let sel: Vec<&Row> = rows
        .iter()
        .filter(|r| r.t_ms >= t0 && r.t_ms <= t1)
        .collect();
    let ts: Vec<f64> = sel.iter().map(|r| r.t_ms as f64).collect();
    let ys: Vec<f64> = sel
        .iter()
        .map(|r| f64::from(if vertical { r.y } else { r.x }))
        .collect();
    slope_per_s(&ts, &ys).expect("enough rows")
}

/// First and last time a mask is held.
pub fn hold_window(rows: &[Row], mask: u32) -> (u64, u64) {
    let ts: Vec<u64> = rows
        .iter()
        .filter(|r| r.held & mask != 0)
        .map(|r| r.t_ms)
        .collect();
    (*ts.first().expect("held"), *ts.last().expect("held"))
}

/// Peak height, time to peak and air time of a jump from the first row.
pub fn jump_metrics(rows: &[Row]) -> (i32, u64, u64) {
    let y0 = rows[0].y;
    let top = rows.iter().min_by_key(|r| (r.y, r.t_ms)).expect("rows");
    let land = rows
        .iter()
        .find(|r| r.t_ms > top.t_ms && r.y >= y0)
        .expect("lands");
    (y0 - top.y, top.t_ms, land.t_ms)
}

// ------------------------------------------------------------- tolerances

/// One tolerance: the target and the allowed difference.
#[derive(Debug, Clone, PartialEq)]
pub struct Tolerance {
    pub value: f64,
    pub tolerance: f64,
    pub provisional: bool,
}

/// The built-in movement targets (60 fps feel).
pub fn builtin() -> BTreeMap<String, Tolerance> {
    let t = |value: f64, tolerance: f64| Tolerance {
        value,
        tolerance,
        provisional: false,
    };
    // Targets the measurement still marks provisional, flagged in the output
    // the same way as when the measured table is loaded.
    let p = |value: f64, tolerance: f64| Tolerance {
        value,
        tolerance,
        provisional: true,
    };
    let mut m = BTreeMap::new();
    let rows: [(&str, Tolerance); 31] = [
        ("walk_speed", p(269.22, 3.0)),
        ("run_speed", p(330.0, 3.0)),
        ("run_onset_ms", t(1500.0, 18.0)),
        ("stop_slide_px", t(0.0, 1.0)),
        ("jump_tap_peak", t(45.0, 1.0)),
        ("jump_tap_time_to_peak_ms", t(154.0, 21.0)),
        ("jump_tap_airtime_ms", t(354.0, 21.0)),
        ("jump_hold_50_peak", t(45.0, 1.0)),
        ("jump_hold_50_time_to_peak_ms", t(141.5, 18.0)),
        ("jump_hold_50_airtime_ms", t(341.5, 18.0)),
        ("jump_hold_100_peak", t(58.0, 1.0)),
        ("jump_hold_100_time_to_peak_ms", t(166.0, 18.0)),
        ("jump_hold_100_airtime_ms", t(391.5, 18.0)),
        ("jump_hold_162_peak", t(87.5, 1.0)),
        ("jump_hold_162_time_to_peak_ms", t(258.25, 18.0)),
        ("jump_hold_162_airtime_ms", t(533.0, 18.0)),
        ("jump_hold_216_peak", p(121.0, 5.0)),
        ("jump_hold_216_time_to_peak_ms", t(328.75, 18.0)),
        ("jump_hold_216_airtime_ms", t(645.5, 20.5)),
        ("jump_full_peak", t(145.0, 1.0)),
        ("jump_full_time_to_peak_ms", t(366.5, 18.0)),
        ("jump_full_airtime_ms", t(724.5, 18.0)),
        // The original at 60 frames per second climbs up at 220 px/s, as
        // fast as down.
        ("climb_speed", p(220.05, 3.0)),
        ("climb_down_speed", p(220.05, 3.0)),
        ("fall_terminal_vy", t(1042.13, 25.0)),
        ("ledge_drop_px", t(64.0, 1.0)),
        ("ledge_fall_ms", t(200.25, 18.0)),
        ("cam_follow_lag_px", t(0.0, 1.0)),
        ("cam_settle_ms", t(25.0, 18.0)),
        ("lookup_state_delay_ms", t(657.0, 18.0)),
        ("lookup_pan_delay_ms", p(915.0, 25.0)),
    ];
    for (k, v) in rows {
        m.insert(k.to_string(), v);
    }
    m.insert("crouch_pan_delay_ms".into(), t(1024.5, 18.0));
    m.insert("crouch_max_offset_px".into(), t(200.0, 1.0));
    m.insert("lookup_max_offset_px".into(), t(-200.0, 1.0));
    m.insert("cam_snap_first_step_px".into(), t(900.0, 1.0));
    m.insert("cam_snap_settle_ms".into(), t(0.0, 18.0));
    m
}

/// A minimal reader for tables of `key = number | string | bool` lines.
fn parse_toml(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut table = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            table = name.trim().to_string();
            out.entry(table.clone()).or_default();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').to_string();
            out.entry(table.clone())
                .or_default()
                .insert(k.trim().to_string(), v);
        }
    }
    out
}

/// The tolerance table: `CLAW_TOLERANCES` when set, else the built-in one.
pub fn tolerances() -> (BTreeMap<String, Tolerance>, String) {
    let Some(path) = std::env::var_os("CLAW_TOLERANCES") else {
        return (builtin(), "built-in".to_string());
    };
    let text = std::fs::read_to_string(&path).expect("CLAW_TOLERANCES is readable");
    let mut out = builtin();
    for (name, kv) in parse_toml(&text) {
        if name == "meta" {
            continue;
        }
        let num = |k: &str| kv.get(k).and_then(|v| v.parse::<f64>().ok());
        let pending = kv.get("pending_decision").map(String::as_str) == Some("speed policy");
        let value = if pending {
            num("cap60_mean")
        } else {
            num("value")
        };
        let (Some(value), Some(tolerance)) = (value, num("tolerance")) else {
            continue;
        };
        let provisional = kv.get("provisional").map(String::as_str) == Some("true");
        out.insert(
            name,
            Tolerance {
                value,
                tolerance,
                provisional,
            },
        );
    }
    (out, path.to_string_lossy().into_owned())
}

/// Checks measured metrics against the table and prints a report; fails on
/// any metric outside its tolerance.
pub fn check(measured: &[(&str, f64)]) {
    let (table, source) = tolerances();
    let mut bad = Vec::new();
    for &(name, got) in measured {
        let tol = table
            .get(name)
            .unwrap_or_else(|| panic!("no tolerance for {name}"));
        let ok = (got - tol.value).abs() <= tol.tolerance + 1e-9;
        println!(
            "{name}: {got:.2} (target {:.2} +- {:.2}{}) {} [{source}]",
            tol.value,
            tol.tolerance,
            if tol.provisional { ", provisional" } else { "" },
            if ok { "ok" } else { "OUT" }
        );
        if !ok {
            bad.push(name);
        }
    }
    assert!(bad.is_empty(), "outside tolerance: {bad:?}");
}

/// Movement box rows of Claw's standing box, for invariants.
pub const MOVE_RECT: Rect = Rect::new(-16, -52, 16, 60);

/// True for the air states.
pub fn airborne(s: i32) -> bool {
    s == state::JUMP || s == state::FALL
}

pub fn is_fall(p: i32) -> bool {
    p == ptype::FALL
}
