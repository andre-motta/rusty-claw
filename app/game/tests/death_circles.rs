//! A death's circles played through a session that shows them, as the
//! window's does:
//! the tick that takes a life ends with Claw dead, the circles run for
//! their frame time with no tick, Claw respawns between the two ticks when
//! the closing circle is over, and play goes on with the next tick. The
//! ticks, and so traces and recordings, are those of the simulation
//! stepped on its own, at any display rate.

use std::time::Duration;

use claw_game::Session;
use claw_game::session::{CIRCLES_MS, Circles};
use claw_sim::player::death::{CLOSE_MS, HOLD_MS, OPEN_MS};
use claw_sim::trace::TraceRow;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TileMap};

const RIGHT: u32 = 0x200_0000;

/// A flat floor with death tiles in columns 25 to 27 (x 1600 to 1791);
/// Claw starts at x 1000.
fn level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(25..28, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// Right held from the third tick for 200 ticks (into the death tiles),
/// then nothing, then Right again: 400 ticks.
fn script() -> InputScript {
    let mut held = vec![0; 2];
    held.extend([RIGHT; 200]);
    held.extend([0; 100]);
    held.extend([RIGHT; 98]);
    InputScript::from_held(&held)
}

/// The trace of the simulation stepped on its own along [`script`].
fn stepped() -> Vec<TraceRow> {
    let mut sim = Sim::new(level(), SimConfig::default());
    let mut rows = Vec::new();
    for (n, t) in script().ticks.iter().enumerate() {
        sim.step(*t);
        rows.push(TraceRow::capture(&sim, n as u64, "script"));
    }
    rows
}

/// What the session showed in each frame: the ticks run so far and the
/// circles, if any.
struct Run {
    trace: Vec<TraceRow>,
    frames: Vec<(u64, Option<Circles>)>,
}

/// Plays [`script`] through a session with frames of `frame_us`.
fn played(frame_us: u64) -> Run {
    let sim = Sim::new(level(), SimConfig::default());
    let mut s = Session::new(sim).with_replay(script()).with_death_circles();
    s.record_trace();
    let mut frames = Vec::new();
    while s.ticks() < 400 {
        s.frame(Duration::from_micros(frame_us));
        frames.push((s.ticks(), s.circles()));
        assert!(frames.len() < 100_000);
    }
    let mut trace = s.trace().to_vec();
    trace.truncate(400);
    Run { trace, frames }
}

#[test]
fn the_circles_take_frame_time_between_two_ticks() {
    let run = played(16_667);
    let deaths: Vec<u64> = run
        .trace
        .iter()
        .filter(|r| r.events.contains("claw_died"))
        .map(|r| r.tick)
        .collect();
    let died = *deaths.first().expect("Claw died");
    // Every frame with circles shows the game after a death's tick, before
    // the next one.
    assert!(
        run.frames
            .iter()
            .filter(|f| f.1.is_some())
            .all(|f| deaths.contains(&(f.0 - 1)))
    );
    let shown: Vec<&(u64, Option<Circles>)> = run
        .frames
        .iter()
        .filter(|f| f.1.is_some() && f.0 == died + 1)
        .collect();
    // They last the three parts' frame time, about 165 frames at 60 Hz.
    let ms = shown.len() as f64 * 16.667;
    let total = f64::from(CIRCLES_MS);
    assert!(
        ms > total - 2.0 * 16.667 && ms < total + 16.667,
        "{ms} ms of circles"
    );
    // The circle closes on the body, the screen is black, and it opens on
    // Claw back at the start.
    let c: Vec<Circles> = shown.iter().map(|f| f.1.unwrap()).collect();
    assert!(c[0].transition.closing && c[0].elapsed_ms < 20.0);
    let closing: Vec<&Circles> = c.iter().filter(|c| c.transition.closing).collect();
    assert!(
        closing
            .windows(2)
            .all(|w| w[0].elapsed_ms < w[1].elapsed_ms)
    );
    assert!(closing.last().unwrap().elapsed_ms < f64::from(CLOSE_MS));
    let opening: Vec<&Circles> = c.iter().filter(|c| !c.transition.closing).collect();
    assert!(opening.first().unwrap().elapsed_ms < 0.0, "black first");
    assert!(opening.first().unwrap().elapsed_ms >= -f64::from(HOLD_MS));
    assert!(opening.last().unwrap().elapsed_ms < f64::from(OPEN_MS));
    assert!(opening.last().unwrap().elapsed_ms > f64::from(OPEN_MS) - 20.0);
    assert_eq!(opening[0].transition.center.x, 1000.0);
    // The ticks are the simulation's own.
    assert_eq!(run.trace, stepped());
}

#[test]
fn the_ticks_do_not_depend_on_the_display_rate() {
    let want = stepped();
    for frame_us in [6_944, 16_667, 33_333, 120_000] {
        let run = played(frame_us);
        assert_eq!(run.trace, want, "frames of {frame_us} us");
        assert!(run.frames.iter().any(|f| f.1.is_some()));
    }
}

#[test]
fn without_the_circles_a_session_respawns_with_the_next_tick() {
    // A headless session (replays, tools) does not hold for the circles:
    // the same ticks, and no frame without one once play runs.
    let sim = Sim::new(level(), SimConfig::default());
    let mut s = Session::new(sim).with_replay(script());
    s.record_trace();
    while s.ticks() < 400 {
        s.frame(Duration::from_micros(18_000));
        assert!(s.circles().is_none());
    }
    assert_eq!(s.trace(), &stepped()[..]);
}
