//! The benchmark along a route file (`--bench-route`): reading the route,
//! the settings it plays with, its report and verdict, and the trace it
//! plays, which is the same on every run and at every display rate.
//! Synthetic levels built in code, plus one case on the first level of the
//! user's own install with the committed route (read from `CLAW_INSTALL`
//! at run time, skipped when it is unset; no GPU).

use std::path::{Path, PathBuf};

use claw_frame::{Buttons, SimulatedRefresh};
use claw_game::bench::{
    Report, RouteFile, RouteOutcome, RouteReport, Verdict, route_session, trace_digest,
};
use claw_game::pause::PauseInput;
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{load_level, new_sim, replay_headless};
use claw_sim::input::ScriptTuning;
use claw_sim::trace::TraceRow;
use claw_sim::tuning::FuseClock;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TileMap};

fn level() -> SimLevel {
    let mut rows = vec![".".repeat(120); 15];
    rows.push("#".repeat(120));
    rows.push("#".repeat(120));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// Walks right, jumps, walks back with a running jump: every held-mask
/// change at least two ticks after the last.
fn walk() -> InputScript {
    let (r, l, j) = (Buttons::RIGHT.0, Buttons::LEFT.0, Buttons::JUMP.0);
    let mut held = Vec::new();
    for (mask, n) in [(0, 10), (r, 60), (r | j, 20), (r, 30), (0, 10), (l, 50)] {
        held.extend(std::iter::repeat_n(mask, n));
    }
    held.extend(std::iter::repeat_n(l | j, 25));
    held.extend(std::iter::repeat_n(0, 30));
    InputScript::from_held(&held)
}

/// The route's trace through the benchmark's session at a display of `hz`.
fn bench_trace(sim: Sim, script: &InputScript, settings: &Settings, hz: f64) -> Vec<TraceRow> {
    let mut session = route_session(sim, script, settings);
    let mut refresh = SimulatedRefresh::new(hz);
    while (session.ticks() as usize) < script.ticks.len() {
        session.frame(refresh.next().unwrap_or_default());
    }
    session.trace()[..script.ticks.len()].to_vec()
}

#[test]
fn the_route_plays_the_same_at_every_rate_and_on_every_run() {
    let script = walk();
    let settings = Settings::new("/nowhere", LevelChoice::Retail(1));
    let sim = Sim::new(level(), SimConfig::default());
    let a = bench_trace(sim.clone(), &script, &settings, 144.0);
    let b = bench_trace(sim.clone(), &script, &settings, 144.0);
    assert_eq!(a.len(), script.ticks.len());
    assert_eq!(a, b, "two runs");
    assert_eq!(trace_digest(&a), trace_digest(&b));
    for hz in [60.0, 240.0] {
        assert_eq!(
            bench_trace(sim.clone(), &script, &settings, hz),
            a,
            "{hz} Hz"
        );
    }
    let mut direct = sim.clone();
    for (k, t) in script.ticks.iter().enumerate() {
        direct.step(*t);
        assert_eq!(
            TraceRow::capture(&direct, k as u64, "script"),
            a[k],
            "tick {k} stepped directly"
        );
    }
    assert!(
        a.iter().any(|r| r.x != a[0].x),
        "Claw moved along the route"
    );
}

#[test]
fn the_digest_follows_the_trace() {
    let script = walk();
    let settings = Settings::new("/nowhere", LevelChoice::Retail(1));
    let rows = bench_trace(
        Sim::new(level(), SimConfig::default()),
        &script,
        &settings,
        60.0,
    );
    let d = trace_digest(&rows);
    assert_eq!(d, trace_digest(&rows.clone()));
    let mut other = rows.clone();
    other[40].x += 1;
    assert_ne!(d, trace_digest(&other));
    assert_ne!(d, trace_digest(&rows[..rows.len() - 1]));
}

#[test]
fn a_route_file_names_how_it_is_played() {
    let text = "# level 2\n# tuning original\n# seed 4105512405\n# mid_air_hit_fix = on\n\
                # pause_input = release_all\n# fuse_clock = frozen\n\
                tick,held_mask,pressed_mask\n0,0x0,0x0\n1,0x2000000,0x2000000\n"
        .to_string();
    let route = RouteFile::parse("r.csv", text.clone()).unwrap();
    assert_eq!(route.script.ticks.len(), 2);
    let mut s = Settings::new("/nowhere", LevelChoice::Retail(1));
    route.apply(&mut s);
    assert_eq!(s.level, LevelChoice::Retail(2));
    assert_eq!(s.tuning, Some(ScriptTuning::Original));
    assert_eq!(s.seed, Some(4_105_512_405));
    assert!(s.mid_air_hit_fix);
    assert_eq!(s.pause_input, PauseInput::ReleaseAll);
    assert_eq!(s.fuse_clock, FuseClock::Frozen);

    // The command line wins over the route.
    let mut s = Settings::new("/nowhere", LevelChoice::Retail(1));
    s.from_command_line
        .extend(["level", "mid_air_hit_fix", "pause_input", "fuse_clock"]);
    route.apply(&mut s);
    assert_eq!(s.level, LevelChoice::Retail(1));
    assert!(!s.mid_air_hit_fix);
    assert_eq!(s.pause_input, PauseInput::Original);
    assert_eq!(s.fuse_clock, FuseClock::Original);

    // A route without those lines leaves the settings alone.
    let plain =
        RouteFile::parse("p.csv", "tick,held_mask,pressed_mask\n0,0x0,0x0\n".into()).unwrap();
    let mut s = Settings::new("/nowhere", LevelChoice::Retail(3));
    plain.apply(&mut s);
    assert_eq!(s.level, LevelChoice::Retail(3));
    assert_eq!(s.tuning, None);
    assert_eq!(s.seed, None);

    assert!(RouteFile::parse("e.csv", "tick,held_mask,pressed_mask\n".into()).is_err());
    assert!(RouteFile::parse("b.csv", "tick,held_mask,pressed_mask\n0,zz,0\n".into()).is_err());
    assert!(RouteFile::read(Path::new("/nonexistent/route.csv")).is_err());
}

fn outcome(loaded: usize, ran: usize) -> RouteOutcome {
    RouteOutcome {
        file: "level1.csv".into(),
        rows: 100,
        ticks_run: ran,
        level_completed: ran == 100,
        sets_loaded_while_drawing: loaded,
        trace_digest: 0xabc,
        trace: Vec::new(),
    }
}

fn report() -> Report {
    let mut r = Report {
        mode: "headless",
        level: "LEVEL1".into(),
        ..Report::default()
    };
    r.samples.frame(1.0, 1);
    r.samples.frame(2.0, 0);
    r
}

#[test]
fn sets_loaded_while_drawing_fail_the_route_benchmark() {
    let ok = RouteReport {
        report: report(),
        route: outcome(0, 100),
    };
    assert!(ok.report.passed());
    assert!(ok.passed());
    let check = ok.checks().pop().unwrap();
    assert_eq!(check.name, "loaded while drawing");
    assert_eq!(check.verdict, Verdict::Pass);
    assert!(check.text.contains("at the route's end"), "{}", check.text);
    let line = ok.summary();
    assert!(line.starts_with("bench PASS"), "{line}");
    assert!(
        line.contains("route level1.csv: 100 of 100 ticks, level completed"),
        "{line}"
    );
    assert!(line.contains("trace digest 0000000000000abc"), "{line}");
    let json = ok.to_json();
    assert!(
        json.contains("\"bench_route\": {\"file\": \"level1.csv\""),
        "{json}"
    );
    assert!(json.contains("\"sets_loaded_while_drawing\": 0"), "{json}");
    assert!(json.contains("\"finished\": true"), "{json}");
    assert!(json.contains("\"pass\": true"), "{json}");
    assert!(json.ends_with("}}"), "{json}");
    assert_eq!(
        json.matches('{').count(),
        json.matches('}').count(),
        "balanced: {json}"
    );

    let bad = RouteReport {
        report: report(),
        route: outcome(2, 40),
    };
    assert!(bad.report.passed(), "the frame budgets alone pass");
    assert!(!bad.passed());
    assert_eq!(bad.checks().pop().unwrap().verdict, Verdict::Fail);
    let line = bad.summary();
    assert!(line.starts_with("bench FAIL"), "{line}");
    assert!(
        line.contains("2 sprite sets after 40 of the route's 100 ticks"),
        "{line}"
    );
    assert!(line.contains("stopped before its end"), "{line}");
    let json = bad.to_json();
    assert!(json.contains("\"pass\": false"), "{json}");
    assert!(
        json.contains("\"loaded_while_drawing_ok\": false"),
        "{json}"
    );
    assert!(json.contains("\"finished\": false"), "{json}");
}

/// The committed route of the first level, played twice through the
/// benchmark's session and once as a headless replay: the same trace each
/// time (no drawing; the drawn run is `make bench ... --bench-route`).
#[test]
fn install_the_first_level_route_plays_the_same_twice() {
    let Some(install) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/acceptance/routes/level1.csv");
    let route = RouteFile::read(&path).unwrap();
    let mut settings = Settings::new(install, LevelChoice::Retail(1));
    settings.mid_air_hit_fix = false;
    route.apply(&mut settings);
    let bundle = load_level(&settings.install, &settings.level).unwrap();
    let start = || new_sim(&bundle, settings.sim_config(settings.res), settings.start);
    let a = bench_trace(start(), &route.script, &settings, 144.0);
    let b = bench_trace(start(), &route.script, &settings, 144.0);
    assert_eq!(a.len(), route.script.ticks.len());
    assert_eq!(trace_digest(&a), trace_digest(&b), "two runs");
    assert!(a == b);
    let replayed = replay_headless(&bundle, &settings, route.script.clone(), 60.0, None);
    assert!(
        replayed == a,
        "a replay at 60 Hz plays the benchmark's trace"
    );
    let completed = a.iter().any(|r| r.events.contains("level_complete"));
    println!(
        "{}: {} ticks, trace digest {:016x}, level completed: {completed}",
        path.display(),
        a.len(),
        trace_digest(&a)
    );
}
