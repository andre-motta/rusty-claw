//! Claw's movement in level 1 against the movement targets, and the
//! independence of the simulation from the display rate.
//!
//! Each scenario is a short input script played through the game's frame
//! loop on level 1, at the spot where the scenario makes sense (a long flat
//! floor, the ladder near the start, the step after the start). The
//! measured distances, speeds and times must fall within the movement
//! targets: how far and how fast Claw moves in the original played at 60
//! frames per second, each with the spread its measurement allows. Rows the
//! table marks provisional are reported as such. `CLAW_TOLERANCES` names a
//! TOML file with one table per metric (`value`, `tolerance`, optionally
//! `provisional`, and for speeds marked `pending_decision = "speed policy"`
//! the 60 fps mean `cap60_mean`) to check against instead.
//!
//! Every scenario is also played at simulated 60, 144 and 240 Hz displays
//! (steady and with jitter), both as a replayed script and as key events
//! seen by the game while frames run; all traces must be identical.
//!
//! The scenarios run with the level objects' logics off, so the movement
//! targets measure Claw alone, as they were calibrated.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use claw_acceptance::install;
use claw_frame::SimulatedRefresh;
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{LevelBundle, Session, load_level, new_sim};
use claw_sim::player::state;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{InputScript, Sim};

const TICK: u64 = 18;
const JUMP: u32 = 0x1;
const LEFT: u32 = 0x100_0000;
const RIGHT: u32 = 0x200_0000;
const UP: u32 = 0x400_0000;
const DOWN: u32 = 0x800_0000;

/// A long flat floor with free space above and nothing nearby.
const FLAT: (i32, i32) = (6272, 3150);
/// At the foot of the ladder near the start.
const LADDER: (i32, i32) = (1440, 4760);

/// Ticks needed to hold a key for at least `ms`.
fn ticks(ms: u64) -> usize {
    ms.div_ceil(TICK).max(1) as usize
}

/// A script of (mask, duration in ms) segments, after one tick with every
/// key up (a key already down when play starts counts only once released).
fn script(segments: &[(u32, u64)]) -> InputScript {
    let mut held = vec![0];
    for &(mask, ms) in segments {
        held.extend(std::iter::repeat_n(mask, ticks(ms)));
    }
    InputScript::from_held(&held)
}

/// A scripted move of Claw while the script runs.
#[derive(Debug, Clone, Copy)]
struct Teleport {
    tick: u64,
    dx: i32,
    dy: i32,
}

#[derive(Debug, Clone)]
struct Scenario {
    name: &'static str,
    segments: Vec<(u32, u64)>,
    start: Option<(i32, i32)>,
    teleport: Option<Teleport>,
}

fn scenarios() -> Vec<Scenario> {
    let s = |name, segments: &[(u32, u64)], start, teleport| Scenario {
        name,
        segments: segments.to_vec(),
        start,
        teleport,
    };
    vec![
        s("walk", &[(RIGHT, 1000), (0, 600)], Some(FLAT), None),
        s("run_onset", &[(RIGHT, 3000), (0, 600)], Some(FLAT), None),
        s("jump_tap", &[(JUMP, 18), (0, 1500)], Some(FLAT), None),
        s("jump_hold50", &[(JUMP, 50), (0, 1500)], Some(FLAT), None),
        s("jump_hold100", &[(JUMP, 100), (0, 1500)], Some(FLAT), None),
        s("jump_hold150", &[(JUMP, 150), (0, 1500)], Some(FLAT), None),
        s("jump_hold200", &[(JUMP, 200), (0, 1500)], Some(FLAT), None),
        s("jump_full", &[(JUMP, 800), (0, 1000)], Some(FLAT), None),
        s(
            "fall",
            &[(0, 1500)],
            Some(FLAT),
            Some(Teleport {
                tick: 6,
                dx: 0,
                dy: -448,
            }),
        ),
        s("fall_ledge", &[(RIGHT, 900), (0, 1200)], None, None),
        s(
            "crouch_lookup",
            &[(DOWN, 3500), (0, 2000), (UP, 3500), (0, 2000)],
            Some(FLAT),
            None,
        ),
        s(
            "camera",
            &[(RIGHT, 2000), (0, 1500), (LEFT, 1000), (0, 1500)],
            Some(FLAT),
            None,
        ),
        s(
            "camera_snap",
            &[(0, 2500)],
            Some(FLAT),
            Some(Teleport {
                tick: 26,
                dx: 900,
                dy: 0,
            }),
        ),
        s(
            "climb",
            &[
                (UP | JUMP, 108),
                (UP, 1500),
                (0, 500),
                (DOWN, 1000),
                (0, 500),
            ],
            Some(LADDER),
            None,
        ),
    ]
}

/// One tick of a run, with the state before the first tick at t = 0.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    t: u64,
    x: i32,
    y: i32,
    state: i32,
    cam_x: i32,
    cam_y: i32,
    held: u32,
    running_time: i32,
}

/// How the input reaches the game.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Feed {
    /// The script replayed tick by tick.
    Replay,
    /// Key events at the script's changes, a millisecond into their tick,
    /// handled while the frame containing them is under way.
    Keys,
}

struct Run {
    rows: Vec<Row>,
    csv: String,
}

fn run(bundle: &LevelBundle, sc: &Scenario, hz: f64, jitter: Option<u64>, feed: Feed) -> Run {
    let mut settings = Settings::new(PathBuf::new(), LevelChoice::Retail(1));
    settings.start = sc.start;
    let mut cfg = settings.sim_config(settings.res);
    cfg.level_logics = false;
    let sim = new_sim(bundle, cfg, settings.start);
    let first = row_of(&sim, 0, 0);
    let s = script(&sc.segments);
    let n = s.ticks.len();
    let mut session = Session::new(sim);
    if feed == Feed::Replay {
        session = session.with_replay(s.clone());
    }
    session.record_trace();
    // Before each tick: the scripted move, and the run timer the tick
    // before left (the trace does not carry it).
    let timers = Rc::new(RefCell::new(Vec::new()));
    let seen = timers.clone();
    let teleport = sc.teleport;
    session.set_before_tick(Box::new(move |tick, sim: &mut Sim| {
        if tick > 0 {
            seen.borrow_mut().push(sim.claw().running_time);
        }
        if let Some(t) = teleport
            && t.tick == tick
            && let Some(o) = sim.player()
        {
            let (x, y) = (o.x + t.dx, o.y + t.dy);
            sim.teleport_player(x, y);
        }
    }));
    let mut refresh = SimulatedRefresh::new(hz);
    if let Some(seed) = jitter {
        refresh = refresh.with_jitter(0.25, seed);
    }
    let events = s.events();
    let mut next = 0;
    let mut clock = 0u64;
    while (session.ticks() as usize) < n {
        let dt = refresh.next().unwrap();
        let end = clock + dt.as_micros() as u64;
        if feed == Feed::Keys {
            while let Some(&(at, b, down)) = events.get(next) {
                let happened = at + 1_000;
                if happened > end {
                    break;
                }
                let stamp = session.stamp(Duration::from_micros(happened - clock));
                session.key(stamp, b, down);
                next += 1;
            }
        }
        session.frame(dt);
        clock = end;
    }
    timers.borrow_mut().push(session.sim().claw().running_time);
    let trace: Vec<TraceRow> = session.trace()[..n].to_vec();
    let timers = timers.borrow();
    let mut rows = vec![first];
    for (r, &rt) in trace.iter().zip(timers.iter()) {
        rows.push(Row {
            t: r.t_ms,
            x: r.x,
            y: r.y,
            state: r.state_id,
            cam_x: r.cam_x,
            cam_y: r.cam_y,
            held: r.held,
            running_time: rt,
        });
    }
    // The input source column differs between the feeds by design.
    let csv = trace::to_csv(&trace)
        .replace(",script,", ",input,")
        .replace(",keyboard,", ",input,");
    Run { rows, csv }
}

fn row_of(sim: &Sim, t: u64, held: u32) -> Row {
    let o = sim.player().expect("Claw");
    Row {
        t,
        x: o.x,
        y: o.y,
        state: o.state,
        cam_x: sim.camera().current.0,
        cam_y: sim.camera().current.1,
        held,
        running_time: sim.claw().running_time,
    }
}

// ------------------------------------------------------------------ metrics

/// Least-squares slope of ys over ts, per second.
fn slope(ts: &[f64], ys: &[f64]) -> Option<f64> {
    let n = ts.len() as f64;
    if ts.len() < 2 {
        return None;
    }
    let mt = ts.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = ts.iter().map(|t| (t - mt).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let sxy: f64 = ts.iter().zip(ys).map(|(t, y)| (t - mt) * (y - my)).sum();
    Some(sxy / sxx * 1000.0)
}

fn speed(rows: &[Row], t0: u64, t1: u64, vertical: bool) -> f64 {
    let sel: Vec<&Row> = rows.iter().filter(|r| r.t >= t0 && r.t <= t1).collect();
    let ts: Vec<f64> = sel.iter().map(|r| r.t as f64).collect();
    let ys: Vec<f64> = sel
        .iter()
        .map(|r| f64::from(if vertical { r.y } else { r.x }))
        .collect();
    slope(&ts, &ys).expect("enough rows for a speed")
}

/// First and last time `mask` is held.
fn hold_window(rows: &[Row], mask: u32) -> (u64, u64) {
    let ts: Vec<u64> = rows
        .iter()
        .filter(|r| r.held & mask != 0)
        .map(|r| r.t)
        .collect();
    (*ts.first().expect("held"), *ts.last().expect("held"))
}

/// When the script first holds `mask`: the start of that tick.
fn press_ms(sc: &Scenario, mask: u32) -> u64 {
    let s = script(&sc.segments);
    let k = s
        .ticks
        .iter()
        .position(|t| t.held.0 & mask != 0)
        .expect("pressed");
    k as u64 * TICK
}

/// Camera offset from Claw while `mask` is held: delay from the press to
/// the first offset, the largest offset, the pan speed and the speed back.
fn pan(rows: &[Row], sc: &Scenario, mask: u32, prefix: &str, out: &mut Vec<(String, f64)>) {
    let (a, b) = hold_window(rows, mask);
    let off: Vec<(u64, i32)> = rows.iter().map(|r| (r.t, r.cam_y - r.y)).collect();
    let moving: Vec<(u64, i32)> = off
        .iter()
        .copied()
        .filter(|&(t, o)| a <= t && t <= b + 1 && o != 0)
        .collect();
    let Some(&(first, _)) = moving.first() else {
        return;
    };
    out.push((
        format!("{prefix}_pan_delay_ms"),
        (first - press_ms(sc, mask)) as f64,
    ));
    let peak = *moving
        .iter()
        .max_by_key(|&&(t, o)| (o.abs(), std::cmp::Reverse(t)))
        .unwrap();
    out.push((format!("{prefix}_max_offset_px"), f64::from(peak.1)));
    let grow: Vec<(u64, i32)> = moving
        .iter()
        .copied()
        .filter(|&(t, o)| t <= peak.0 && f64::from(o.abs()) < 0.95 * f64::from(peak.1.abs()))
        .collect();
    if grow.len() >= 3 {
        let ts: Vec<f64> = grow.iter().map(|&(t, _)| t as f64).collect();
        let os: Vec<f64> = grow.iter().map(|&(_, o)| f64::from(o.abs())).collect();
        if let Some(v) = slope(&ts, &os) {
            out.push((format!("{prefix}_pan_speed"), v));
        }
    }
    let end = rows
        .iter()
        .find(|r| r.t > b && r.held & (UP | DOWN) != 0)
        .map_or(b + 3000, |r| r.t);
    let back: Vec<(u64, i32)> = off
        .iter()
        .copied()
        .filter(|&(t, o)| b < t && t < end && o != 0)
        .collect();
    if back.len() >= 3 {
        let ts: Vec<f64> = back.iter().map(|&(t, _)| t as f64).collect();
        let os: Vec<f64> = back.iter().map(|&(_, o)| f64::from(o.abs())).collect();
        if let Some(v) = slope(&ts, &os) {
            out.push((format!("{prefix}_return_speed"), -v));
        }
    }
}

/// Least-squares y = a + b t + c t^2; returns c.
fn quad_c(ts: &[f64], ys: &[f64]) -> Option<f64> {
    let s: Vec<f64> = (0..5).map(|k| ts.iter().map(|t| t.powi(k)).sum()).collect();
    let r: Vec<f64> = (0..3)
        .map(|k| ts.iter().zip(ys).map(|(t, y)| y * t.powi(k)).sum())
        .collect();
    let mut m = [
        [s[0], s[1], s[2], r[0]],
        [s[1], s[2], s[3], r[1]],
        [s[2], s[3], s[4], r[2]],
    ];
    for i in 0..3 {
        let p = (i..3).max_by(|&a, &b| m[a][i].abs().total_cmp(&m[b][i].abs()))?;
        m.swap(i, p);
        if m[i][i].abs() < 1e-12 {
            return None;
        }
        for j in 0..3 {
            if j != i {
                let f = m[j][i] / m[i][i];
                let pivot = m[i];
                for (v, p) in m[j].iter_mut().zip(pivot) {
                    *v -= f * p;
                }
            }
        }
    }
    Some(m[2][3] / m[2][2])
}

fn metrics(sc: &Scenario, rows: &[Row]) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut put = |k: &str, v: f64| out.push((k.to_string(), v));
    match sc.name {
        "walk" => {
            let (a, b) = hold_window(rows, RIGHT);
            put("walk_speed", speed(rows, a + 150, b, false));
            let at_release = rows.iter().rfind(|r| r.t <= b).unwrap().x;
            let furthest = rows.iter().filter(|r| r.t > b).map(|r| r.x).max().unwrap();
            put("stop_slide_px", f64::from(furthest - at_release));
        }
        "run_onset" => {
            let (a, b) = hold_window(rows, RIGHT);
            put("walk_speed", speed(rows, a + 150, b.min(a + 1400), false));
            put("run_speed", speed(rows, a + 1800, b, false));
            let onset = rows
                .iter()
                .find(|r| a <= r.t && r.t <= b && r.running_time <= 0)
                .expect("starts running")
                .t;
            put("run_onset_ms", (onset - press_ms(sc, RIGHT)) as f64);
        }
        name if name.starts_with("jump_") => {
            let key = match name {
                "jump_tap" => "jump_tap",
                "jump_hold50" => "jump_hold_50",
                "jump_hold100" => "jump_hold_100",
                "jump_hold150" => "jump_hold_162",
                "jump_hold200" => "jump_hold_216",
                _ => "jump_full",
            };
            let y0 = rows[0].y;
            let press = press_ms(sc, JUMP);
            let top = rows.iter().min_by_key(|r| (r.y, r.t)).unwrap();
            let land = rows
                .iter()
                .find(|r| r.t > top.t && r.y >= y0)
                .expect("lands");
            put(&format!("{key}_peak"), f64::from(y0 - top.y));
            put(&format!("{key}_time_to_peak_ms"), (top.t - press) as f64);
            put(&format!("{key}_airtime_ms"), (land.t - press) as f64);
        }
        "fall" => {
            let snap = sc.teleport.unwrap().tick * TICK;
            let after: Vec<&Row> = rows.iter().filter(|r| r.t > snap).collect();
            let land_y = after.last().unwrap().y;
            let seg: Vec<&Row> = after.into_iter().filter(|r| r.y < land_y).collect();
            let vel: Vec<f64> = seg
                .windows(2)
                .map(|w| f64::from(w[1].y - w[0].y) * 1000.0 / (w[1].t - w[0].t) as f64)
                .collect();
            let vmax = vel.iter().copied().fold(f64::MIN, f64::max);
            let plateau: Vec<Row> = seg[1..]
                .iter()
                .zip(&vel)
                .filter(|&(_, &v)| v >= 0.93 * vmax)
                .map(|(r, _)| (*r).clone())
                .collect();
            assert!(plateau.len() >= 4, "fall reaches its top speed");
            let (t0, t1) = (plateau[0].t, plateau.last().unwrap().t);
            put("fall_terminal_vy", speed(&plateau, t0, t1, true));
            let i0 = vel.iter().position(|&v| v > 0.0).unwrap_or(0);
            let i1 = vel
                .iter()
                .position(|&v| v >= 0.85 * vmax)
                .unwrap_or(vel.len() - 1);
            let pts = &seg[i0..=i1.min(seg.len() - 1)];
            if pts.len() >= 4 {
                let ts: Vec<f64> = pts.iter().map(|r| r.t as f64 / 1000.0).collect();
                let ys: Vec<f64> = pts.iter().map(|r| f64::from(r.y)).collect();
                if let Some(c) = quad_c(&ts, &ys) {
                    put("fall_gravity", 2.0 * c);
                }
            }
        }
        "fall_ledge" => {
            let y0 = rows[0].y;
            let y1 = rows.last().unwrap().y;
            let first = rows.iter().find(|r| r.y > y0).expect("drops").t;
            let landed = rows.iter().find(|r| r.y == y1 && r.t >= first).unwrap().t;
            put("ledge_drop_px", f64::from(y1 - y0));
            put("ledge_fall_ms", (landed - first) as f64);
        }
        "crouch_lookup" => {
            pan(rows, sc, DOWN, "crouch", &mut out);
            pan(rows, sc, UP, "lookup", &mut out);
            let entry = rows
                .iter()
                .find(|r| r.state == state::LOOK_UP)
                .expect("looks up")
                .t;
            out.push((
                "lookup_state_delay_ms".into(),
                (entry - press_ms(sc, UP)) as f64,
            ));
        }
        "camera" => {
            let (a, b) = hold_window(rows, RIGHT);
            let lag: Vec<f64> = rows
                .iter()
                .filter(|r| a + 1000 <= r.t && r.t <= b)
                .map(|r| f64::from(r.x - r.cam_x))
                .collect();
            put(
                "cam_follow_lag_px",
                lag.iter().sum::<f64>() / lag.len() as f64,
            );
            let settle = rows
                .iter()
                .find(|r| r.t > b && r.held & (LEFT | RIGHT) == 0 && (r.cam_x - r.x).abs() <= 2)
                .expect("camera settles")
                .t;
            put("cam_settle_ms", (settle - b) as f64);
        }
        "camera_snap" => {
            let snap = sc.teleport.unwrap().tick * TICK;
            let x0 = rows[0].x;
            let i = rows
                .iter()
                .position(|r| r.t >= snap && r.x != x0)
                .expect("moved");
            put(
                "cam_snap_first_step_px",
                f64::from(rows[i].cam_x - rows[i - 1].cam_x),
            );
            let settle = rows[i..]
                .iter()
                .find(|r| (r.cam_x - r.x).abs() <= 2)
                .expect("camera settles")
                .t;
            put("cam_snap_settle_ms", (settle - rows[i].t) as f64);
        }
        "climb" => {
            for (key, mask) in [("climb_speed", UP), ("climb_down_speed", DOWN)] {
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
                let (t0, t1) = (moving.first().unwrap().t, moving.last().unwrap().t);
                put(key, speed(&sel, t0, t1, true).abs());
            }
        }
        other => panic!("no metrics for {other}"),
    }
    out
}

// --------------------------------------------------------------- targets

#[derive(Debug, Clone, PartialEq)]
struct Target {
    value: f64,
    tolerance: f64,
    provisional: bool,
}

/// The movement targets: the original at 60 frames per second. Rows marked
/// provisional depend on the frame rate in the original (the speeds, the
/// 216 ms jump and the look-up pan delay); the game follows the 60 fps
/// values for them.
fn builtin() -> BTreeMap<String, Target> {
    let rows: [(&str, f64, f64, bool); 36] = [
        ("walk_speed", 269.22, 3.0, true),
        ("run_speed", 330.0, 3.0, true),
        ("run_onset_ms", 1500.0, 18.0, false),
        ("stop_slide_px", 0.0, 1.0, false),
        ("jump_tap_peak", 45.0, 1.0, false),
        ("jump_tap_time_to_peak_ms", 154.0, 21.0, false),
        ("jump_tap_airtime_ms", 354.0, 21.0, false),
        ("jump_hold_50_peak", 45.0, 1.0, false),
        ("jump_hold_50_time_to_peak_ms", 141.5, 18.0, false),
        ("jump_hold_50_airtime_ms", 341.5, 18.0, false),
        ("jump_hold_100_peak", 58.0, 1.0, false),
        ("jump_hold_100_time_to_peak_ms", 166.0, 18.0, false),
        ("jump_hold_100_airtime_ms", 391.5, 18.0, false),
        ("jump_hold_162_peak", 87.5, 1.0, false),
        ("jump_hold_162_time_to_peak_ms", 258.25, 18.0, false),
        ("jump_hold_162_airtime_ms", 533.0, 18.0, false),
        ("jump_hold_216_peak", 121.0, 5.0, true),
        ("jump_hold_216_time_to_peak_ms", 328.75, 18.0, false),
        ("jump_hold_216_airtime_ms", 645.5, 20.5, false),
        ("jump_full_peak", 145.0, 1.0, false),
        ("jump_full_time_to_peak_ms", 366.5, 18.0, false),
        ("jump_full_airtime_ms", 724.5, 18.0, false),
        ("climb_speed", 220.0, 3.0, true),
        ("climb_down_speed", 220.05, 3.0, true),
        ("fall_terminal_vy", 1042.13, 25.0, false),
        ("ledge_drop_px", 64.0, 1.0, false),
        ("ledge_fall_ms", 200.25, 18.0, false),
        ("cam_follow_lag_px", 0.0, 1.0, false),
        ("cam_settle_ms", 25.0, 18.0, false),
        ("cam_snap_first_step_px", 900.0, 1.0, false),
        ("cam_snap_settle_ms", 0.0, 18.0, false),
        ("crouch_pan_delay_ms", 1024.5, 18.0, false),
        ("crouch_max_offset_px", 200.0, 1.0, false),
        ("lookup_pan_delay_ms", 915.0, 25.0, true),
        ("lookup_max_offset_px", -200.0, 1.0, false),
        ("lookup_state_delay_ms", 657.0, 18.0, false),
    ];
    rows.iter()
        .map(|&(k, value, tolerance, provisional)| {
            (
                k.to_string(),
                Target {
                    value,
                    tolerance,
                    provisional,
                },
            )
        })
        .collect()
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
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.entry(table.clone())
                .or_default()
                .insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
    }
    out
}

/// The target table and where it came from.
fn targets() -> (BTreeMap<String, Target>, String) {
    let Some(path) = std::env::var_os("CLAW_TOLERANCES") else {
        return (builtin(), "built-in targets".into());
    };
    let text = std::fs::read_to_string(&path).expect("CLAW_TOLERANCES is readable");
    let mut out = BTreeMap::new();
    for (name, kv) in parse_toml(&text) {
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
        let provisional = pending || kv.get("provisional").map(String::as_str) == Some("true");
        out.insert(
            name,
            Target {
                value,
                tolerance,
                provisional,
            },
        );
    }
    (out, path.to_string_lossy().into_owned())
}

#[test]
fn install_movement_scenarios_meet_the_targets() {
    let Some(root) = install() else { return };
    let bundle = load_level(&root, &LevelChoice::Retail(1)).expect("level 1");
    let (table, source) = targets();
    println!("movement targets: {source}");
    let mut bad = Vec::new();
    let (mut checked, mut provisional, mut informational) = (0, 0, 0);
    for sc in scenarios() {
        let r = run(&bundle, &sc, 60.0, None, Feed::Replay);
        for (name, got) in metrics(&sc, &r.rows) {
            let Some(t) = table.get(&name) else {
                informational += 1;
                println!("{:14} {name}: {got:.2} (no target; reported only)", sc.name);
                continue;
            };
            checked += 1;
            provisional += usize::from(t.provisional);
            let ok = (got - t.value).abs() <= t.tolerance + 1e-9;
            println!(
                "{:14} {name}: {got:.2} (target {:.2} +- {:.2}{}) {}",
                sc.name,
                t.value,
                t.tolerance,
                if t.provisional { ", provisional" } else { "" },
                if ok { "ok" } else { "OUT" }
            );
            if !ok {
                bad.push(name);
            }
        }
    }
    println!(
        "{checked} metrics checked ({provisional} against provisional targets), \
         {informational} reported only"
    );
    assert!(checked >= 36, "only {checked} metrics checked");
    assert!(bad.is_empty(), "outside the targets: {bad:?}");
}

#[test]
fn install_traces_do_not_depend_on_the_display_rate() {
    let Some(root) = install() else { return };
    let bundle = load_level(&root, &LevelChoice::Retail(1)).expect("level 1");
    for sc in scenarios() {
        let base = run(&bundle, &sc, 60.0, None, Feed::Replay);
        let again = run(&bundle, &sc, 60.0, None, Feed::Replay);
        assert_eq!(base.csv, again.csv, "{}: two runs differ", sc.name);
        for (hz, jitter) in [
            (144.0, None),
            (240.0, None),
            (60.0, Some(1)),
            (144.0, Some(2)),
            (240.0, Some(3)),
        ] {
            for feed in [Feed::Replay, Feed::Keys] {
                let r = run(&bundle, &sc, hz, jitter, feed);
                assert_eq!(
                    r.csv, base.csv,
                    "{}: {hz} Hz {jitter:?} {feed:?} differs from 60 Hz",
                    sc.name
                );
            }
        }
        let k = run(&bundle, &sc, 60.0, None, Feed::Keys);
        assert_eq!(k.csv, base.csv, "{}: key events differ", sc.name);
        println!(
            "{}: {} ticks identical at 60, 144 and 240 Hz",
            sc.name,
            base.rows.len() - 1
        );
    }
}
