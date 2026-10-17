//! Trace parsing and the comparison with a reference trace of the same
//! route, on synthetic traces.

mod common;

use claw_sim::trace::{self, HEADER, Tolerances, TraceRow};
use claw_sim::{Sim, SimConfig};
use common::*;

/// A row at time `t_ms` with step length `dt_ms`; Claw stands at `x`.
fn row(tick: u64, t_ms: u64, dt_ms: u32, x: i32, source: &str) -> TraceRow {
    TraceRow {
        tick,
        t_ms,
        dt_ms,
        x,
        y: 959,
        frac_x: 0.0,
        frac_y: 0.0,
        state_id: 1000,
        physics_type: 1,
        cam_x: x,
        cam_y: 900,
        held: 0,
        pressed: 0,
        source: source.to_string(),
        activation: "original",
        health: 100,
        score: 0,
        lives: 6,
        weapon: "pistol",
        pistol: 10,
        magic: 5,
        dynamite: 3,
        events: String::new(),
        rng: Some(7),
    }
}

/// A rebuild trace of `n` 18 ms ticks; `f` fills in each row from its end
/// time.
fn rebuild(n: u64, f: impl Fn(u64, &mut TraceRow)) -> Vec<TraceRow> {
    (0..n)
        .map(|k| {
            let t = 18 * (k + 1);
            let mut r = row(k, t, 18, 0, "script");
            f(t, &mut r);
            r
        })
        .collect()
}

/// A reference trace with the given step lengths; `f` fills in each row
/// from its end time.
fn reference(steps: &[u32], f: impl Fn(u64, &mut TraceRow)) -> Vec<TraceRow> {
    let mut t = 0;
    steps
        .iter()
        .enumerate()
        .map(|(k, &dt)| {
            t += u64::from(dt);
            let mut r = row(k as u64, t, dt, 0, "reference");
            f(t, &mut r);
            r
        })
        .collect()
}

/// The 60 Hz frame pattern: 17, 17, 16 ms.
fn c60(frames: usize) -> Vec<u32> {
    [17, 17, 16].into_iter().cycle().take(frames).collect()
}

/// Claw walks 1 px per ms: the position a trace shows at the end of a
/// step ending at `t`.
fn walk(t: u64, r: &mut TraceRow) {
    r.x = t as i32;
    r.cam_x = t as i32;
}

/// The end time of the 18 ms tick whose span contains `t`.
fn tick_end(t: u64) -> u64 {
    t.div_ceil(18) * 18
}

#[test]
fn identical_traces_pass() {
    let a = rebuild(200, walk);
    for tol in [Tolerances::exact(), Tolerances::feel()] {
        let c = trace::compare(&a, &a, &tol);
        assert!(c.passed(), "{:?}", c.first_divergence);
        assert_eq!(c.rows_compared, 200);
        assert_eq!(c.compared_ms, 3600);
        assert_eq!(c.rows_unmatched, 0);
        assert!(c.per_column.iter().all(|s| s.diverged == 0));
        assert!(
            c.per_column
                .iter()
                .any(|s| s.column == "x" && s.compared == 200)
        );
    }
    // rng is a compared column only in the exact preset.
    let cols = |t: &Tolerances| -> Vec<String> {
        trace::compare(&a, &a, t)
            .per_column
            .into_iter()
            .map(|s| s.column)
            .collect()
    };
    assert!(cols(&Tolerances::exact()).contains(&"rng".to_string()));
    assert!(!cols(&Tolerances::feel()).contains(&"rng".to_string()));
}

#[test]
fn a_ten_ms_state_lag_passes_within_the_slack_only() {
    // The rebuild changes state with the tick ending at 108 ms (span 90 to
    // 108); the reference, sampled every 10 ms, changes at 110 ms, 10 ms
    // later than the rebuild's change becomes visible at 100 ms.
    let b = rebuild(20, |t, r| r.state_id = if t >= 108 { 1003 } else { 1000 });
    let steps = vec![10; 30];
    let refr = reference(&steps, |t, r| {
        r.state_id = if t >= 110 { 1003 } else { 1000 }
    });
    let with_slack = |ms: u32| {
        let mut tol = Tolerances::exact();
        tol.state_slack_ms = ms;
        tol.rng = false;
        trace::compare(&b, &refr, &tol)
    };
    let c = with_slack(0);
    let d = c.first_divergence.expect("a lag outside the slack");
    assert_eq!((d.t_ms, d.column.as_str()), (100, "state_id"));
    assert_eq!((d.reference.as_str(), d.rebuild.as_str()), ("1000", "1003"));
    assert!(!with_slack(9).passed());
    assert!(with_slack(10).passed());
    assert!(with_slack(36).passed());
    // The rebuild trace ends at 360 ms; the reference runs on to 300 ms.
    assert_eq!(with_slack(10).rows_unmatched, 0);
}

#[test]
fn a_planted_jump_is_reported_at_its_time_and_column() {
    let b = rebuild(100, walk);
    let mut refr = b.clone();
    for r in refr.iter_mut().filter(|r| r.t_ms >= 774) {
        r.x += 3;
    }
    let c = trace::compare(&b, &refr, &Tolerances::exact());
    let d = c.first_divergence.clone().expect("the jump");
    assert_eq!(d.t_ms, 774);
    assert_eq!(d.column, "x");
    assert_eq!((d.reference.as_str(), d.rebuild.as_str()), ("777", "774"));
    assert_eq!((d.reference_row, d.rebuild_row), (42, 42));
    let x = c.per_column.iter().find(|s| s.column == "x").unwrap();
    assert_eq!(x.diverged, 100 - 42);
    assert_eq!(x.max_diff, Some(3.0));
    assert_eq!(x.first_t_ms, Some(774));
    // The camera did not jump.
    let cam = c.per_column.iter().find(|s| s.column == "cam_x").unwrap();
    assert_eq!(cam.diverged, 0);
    // Within a 3 px tolerance it passes.
    let mut tol = Tolerances::exact();
    tol.pos_px = 3;
    assert!(trace::compare(&b, &refr, &tol).passed());
    assert!(trace::compare(&b, &refr, &Tolerances::feel()).passed());

    // The report names it and shows five rows of each trace before it.
    let text = trace::report(&b, &refr, &c, 5);
    assert!(
        text.contains("first divergence at t_ms 774 in column x: reference 777, rebuild 774"),
        "{text}"
    );
    let marked: Vec<&str> = text.lines().filter(|l| l.starts_with("> ")).collect();
    assert_eq!(marked.len(), 2, "{text}");
    assert!(marked[0].starts_with("> 42,774,18,777,"), "{text}");
    assert!(marked[1].starts_with("> 42,774,18,774,"), "{text}");
    assert!(text.contains("  37,684,18,684,"), "{text}");
    assert!(!text.contains("  36,666,"), "{text}");
}

#[test]
fn sixty_hz_reference_rows_line_up_with_18_ms_ticks() {
    let b = rebuild(100, walk);
    // A reference at 17, 17, 16 ms steps that shows, at each of its times,
    // what the rebuild tick containing that time shows.
    let steps = c60(100);
    let refr = reference(&steps, |t, r| {
        let end = tick_end(t) as i32;
        r.x = end;
        r.cam_x = end;
    });
    assert_eq!(
        refr.iter().take(6).map(|r| r.t_ms).collect::<Vec<_>>(),
        [17, 34, 50, 67, 84, 100]
    );
    let c = trace::compare(&b, &refr, &Tolerances::exact());
    assert!(c.passed(), "{:?}", c.first_divergence);
    assert_eq!(c.rows_compared, 100);
    // Reference times 17, 34, 50, 67, 84 and 100 fall in ticks 0 to 5.
    let mut planted = refr.clone();
    planted[2].x += 1;
    let d = trace::compare(&b, &planted, &Tolerances::exact())
        .first_divergence
        .unwrap();
    assert_eq!((d.t_ms, d.reference_row, d.rebuild_row), (50, 2, 2));
    assert_eq!(d.rebuild, "54");
    // A reference row ending exactly on a tick boundary belongs to the tick
    // that ends there.
    let exact_18 = reference(&[18, 18], walk);
    assert!(trace::compare(&b, &exact_18, &Tolerances::exact()).passed());
}

#[test]
fn an_rng_mismatch_is_flagged_only_by_the_exact_preset() {
    let b = rebuild(50, walk);
    let mut refr = b.clone();
    refr[20].rng = Some(8);
    let c = trace::compare(&b, &refr, &Tolerances::exact());
    let d = c.first_divergence.expect("rng compared");
    assert_eq!((d.t_ms, d.column.as_str()), (378, "rng"));
    assert!(trace::compare(&b, &refr, &Tolerances::feel()).passed());
    // A reference without the column is not compared on it.
    let mut no_rng = b.clone();
    no_rng.iter_mut().for_each(|r| r.rng = None);
    assert!(trace::compare(&b, &no_rng, &Tolerances::exact()).passed());
}

#[test]
fn stats_must_match_after_the_slack() {
    let b = rebuild(50, |t, r| r.health = if t >= 360 { 90 } else { 100 });
    let refr = reference(&c60(50), |t, r| r.health = if t > 375 { 90 } else { 100 });
    let mut tol = Tolerances::exact();
    tol.rng = false;
    let d = trace::compare(&b, &refr, &tol).first_divergence.unwrap();
    assert_eq!(d.column, "health");
    tol.stats_slack_ms = 36;
    assert!(trace::compare(&b, &refr, &tol).passed());
    // A value the rebuild never reaches fails at any slack.
    let refr = reference(&c60(50), |t, r| r.health = if t > 375 { 85 } else { 100 });
    tol.stats_slack_ms = 1000;
    let d = trace::compare(&b, &refr, &tol).first_divergence.unwrap();
    assert_eq!((d.column.as_str(), d.reference.as_str()), ("health", "85"));
}

#[test]
fn game_events_match_within_their_slack_in_both_directions() {
    // The rebuild reaches the checkpoint in the tick ending at 216 ms; the
    // reference in the frame ending at 234 ms.
    let b = rebuild(30, |t, r| {
        if t == 216 {
            r.events = "message:3:2000|checkpoint:6200:3100".into();
        }
    });
    let refr = reference(&c60(30), |t, r| {
        if t == 234 {
            r.events = "checkpoint:6200:3100".into();
        }
    });
    let mut tol = Tolerances::feel();
    assert!(trace::compare(&b, &refr, &tol).passed());
    tol.events_slack_ms = 0;
    let d = trace::compare(&b, &refr, &tol).first_divergence.unwrap();
    // The rebuild's event comes first and the reference has none there.
    assert_eq!(
        (
            d.t_ms,
            d.column.as_str(),
            d.reference.as_str(),
            d.rebuild.as_str()
        ),
        (216, "events", "none", "checkpoint:6200:3100")
    );
    // A death only the reference has is reported at its time.
    let refr = reference(&c60(30), |t, r| {
        if t == 234 {
            r.events = "checkpoint:6200:3100".into();
        }
        if t == 400 {
            r.events = "claw_died:5".into();
        }
    });
    let d = trace::compare(&b, &refr, &Tolerances::feel())
        .first_divergence
        .unwrap();
    assert_eq!(
        (d.t_ms, d.column.as_str(), d.reference.as_str()),
        (400, "events", "claw_died:5")
    );
}

#[test]
fn reference_rows_after_the_rebuild_ends_are_counted_not_compared() {
    let b = rebuild(10, walk);
    let refr = rebuild(12, walk);
    let c = trace::compare(&b, &refr, &Tolerances::exact());
    assert!(c.passed());
    assert_eq!(
        (c.rows_compared, c.rows_unmatched, c.compared_ms),
        (10, 2, 180)
    );
    let text = trace::report(&b, &refr, &c, 5);
    assert!(text.contains("2 reference rows run past the end"), "{text}");
    assert!(text.contains("no divergence"), "{text}");
}

#[test]
fn traces_round_trip_through_csv() {
    let rows = rebuild(5, |t, r| {
        walk(t, r);
        r.frac_x = 0.25;
        r.held = 0x200_0000;
        r.events = "checkpoint:1:2".into();
    });
    let text = trace::to_csv(&rows);
    assert!(text.starts_with(HEADER));
    assert!(HEADER.ends_with(",events,rng"));
    assert_eq!(trace::parse(&text).unwrap(), rows);
}

#[test]
fn old_traces_without_the_rng_column_parse() {
    let rows = rebuild(3, walk);
    let old_header = HEADER.strip_suffix(",rng").unwrap();
    let mut text = format!("{old_header}\n");
    for r in &rows {
        let line = r.to_csv();
        text.push_str(line.rsplit_once(',').unwrap().0);
        text.push('\n');
    }
    let parsed = trace::parse(&text).unwrap();
    assert_eq!(parsed.len(), 3);
    assert!(parsed.iter().all(|r| r.rng.is_none()));
    let mut want = rows.clone();
    want.iter_mut().for_each(|r| r.rng = None);
    assert_eq!(parsed, want);
}

#[test]
fn reference_traces_parse_by_column_name() {
    // Columns in another order, an extra column, empty cells where they
    // are allowed, variable steps and missing trailing cells.
    let text = "# reference trace\n\
        t_ms,tick,x,y,state_id,extra,frac_x,activation,rng,source,dt_ms,events\n\
        17,0,100,959,1000,a,,,,reference,17,\n\
        34,1,101,959,1000,b,,,123,reference,17,checkpoint:1:2\n\
        50,2,103,959,1000,c,,,,reference,16\n";
    let rows = trace::parse(text).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows.iter().map(|r| (r.t_ms, r.dt_ms)).collect::<Vec<_>>(),
        [(17, 17), (34, 17), (50, 16)]
    );
    assert!(rows[0].frac_x.is_nan() && rows[0].frac_y.is_nan());
    assert_eq!(rows[0].activation, "");
    assert_eq!(rows[1].rng, Some(123));
    assert_eq!(rows[1].events, "checkpoint:1:2");
    assert_eq!(rows[2].events, "");
    assert_eq!(rows[2].source, "reference");
    // An empty fraction is written back as an empty cell.
    assert!(rows[0].to_csv().starts_with("0,17,17,100,959,,,"));
    // Without dt_ms, the step is the time since the previous row.
    let rows = trace::parse("tick,t_ms\n0,17\n1,34\n2,50\n").unwrap();
    assert_eq!(
        rows.iter().map(|r| r.dt_ms).collect::<Vec<_>>(),
        [17, 17, 16]
    );
    assert!(trace::parse("").unwrap().is_empty());
}

#[test]
fn bad_traces_are_errors() {
    for (text, want) in [
        ("tick,x\n0,1\n", "no t_ms column"),
        ("tick,t_ms,tick\n", "appears twice"),
        ("tick,t_ms,x\n0,18,\n", "bad x"),
        ("tick,t_ms,x\n0,18,1.5\n", "bad x"),
        ("tick,t_ms,x\n0,18,1,2\n", "4 cells"),
        ("tick,t_ms,weapon\n0,18,sword\n", "bad weapon"),
        ("tick,t_ms,activation\n0,18,everything\n", "bad activation"),
        ("tick,t_ms,rng\n0,18,-1\n", "bad rng"),
        ("tick,t_ms\nx,18\n", "bad tick"),
    ] {
        let e = trace::parse(text).expect_err(text);
        assert!(e.contains(want), "{text}: {e}");
    }
}

#[test]
fn tolerance_files_set_presets_groups_and_columns() {
    let tol = Tolerances::from_toml(
        "# feel, but stricter on the camera\n\
         base = \"feel\"\n\
         state_slack_ms = 54   # three ticks\n\
         \n\
         [cam_y]\n\
         max_px = 2\n\
         slack_ms = 0\n\
         \n\
         [score]\n\
         exact = true\n\
         \n\
         [pressed]\n\
         compare = true\n\
         \n\
         [cam_x]\n\
         compare = false\n",
    )
    .unwrap();
    let feel = Tolerances::feel();
    assert_eq!(tol.pos_px, feel.pos_px);
    assert_eq!(tol.state_slack_ms, 54);
    assert!(!tol.rng);
    let cam_y = tol.column("cam_y").unwrap();
    assert_eq!(
        (cam_y.compared, cam_y.max_diff, cam_y.slack_ms),
        (true, 2.0, 0)
    );
    let score = tol.column("score").unwrap();
    assert_eq!((score.max_diff, score.slack_ms), (0.0, 0));
    assert!(tol.column("pressed").unwrap().compared);
    assert!(!tol.column("cam_x").unwrap().compared);
    assert_eq!(tol.column("state_id").unwrap().slack_ms, 54);
    assert!(tol.column("tick").is_none());
    // Without a base, the exact preset.
    assert_eq!(Tolerances::from_toml("").unwrap(), Tolerances::exact());
    let t = Tolerances::from_toml("rng = false\npos_px = 4\n").unwrap();
    assert!(!t.rng && t.pos_px == 4);

    // A column turned on by a file is compared.
    let b = rebuild(10, walk);
    let mut refr = b.clone();
    refr[4].pressed = 1;
    assert!(trace::compare(&b, &refr, &Tolerances::exact()).passed());
    assert_eq!(
        trace::compare(&b, &refr, &tol)
            .first_divergence
            .map(|d| d.column),
        Some("pressed".to_string())
    );
}

#[test]
fn bad_tolerance_files_are_errors() {
    for (text, want) in [
        (
            "pos_px = 1\nbase = \"feel\"\n",
            "base must be the first key",
        ),
        ("base = \"loose\"\n", "unknown base"),
        ("speed = 3\n", "unknown key"),
        ("[tick]\n", "unknown column"),
        ("[x]\nmax_px = 1\n[x]\n", "appears twice"),
        ("[x]\nmax_px = 1\nmax_px = 2\n", "appears twice"),
        ("[x]\nwobble = 1\n", "unknown key"),
        ("[x]\nmax_px = -1\n", "at least 0"),
        ("[x]\nslack_ms = 1.5\n", "whole number"),
        (
            "[x]\nexact = true\nslack_ms = 5\n",
            "exact = true and a tolerance",
        ),
        (
            "[x]\nmax_px = 3\nexact = true\n",
            "exact = true and a tolerance",
        ),
        ("[events]\nmax_px = 3\n", "takes no max_px"),
        ("[x\n", "bad section"),
        ("rng = yes\n", "bad value"),
        ("pos_px\n", "expected key = value"),
    ] {
        let e = Tolerances::from_toml(text).expect_err(text);
        assert!(e.contains(want), "{text:?}: {e}");
    }
    let e = Tolerances::from_toml("\n\n[x]\nmax_px = -1\n").unwrap_err();
    assert!(e.starts_with("line 4:"), "{e}");
}

#[test]
fn traces_record_the_shared_random_stream() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(sim.rng_state(), sim.rng().state());
    let mut rows = Vec::new();
    for (k, input) in script(&[(RIGHT, 500), (0, 300)])
        .ticks
        .into_iter()
        .enumerate()
    {
        sim.step(input);
        rows.push(TraceRow::capture(&sim, k as u64, "script"));
        assert_eq!(rows[k].rng, Some(sim.rng_state()));
    }
    // The column round-trips through the text.
    let parsed = trace::parse(&trace::to_csv(&rows)).unwrap();
    assert_eq!(
        parsed.iter().map(|r| r.rng).collect::<Vec<_>>(),
        rows.iter().map(|r| r.rng).collect::<Vec<_>>()
    );
    // A different seed gives a different state at the start.
    let cfg = SimConfig {
        seed: 99,
        ..SimConfig::default()
    };
    assert_eq!(Sim::new(flat_level(), cfg).rng_state(), 99);
}
