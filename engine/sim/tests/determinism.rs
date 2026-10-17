//! Determinism and the collision invariants: identical traces across runs
//! and display rates, Claw never inside Solid, Ground landings only from
//! above.

mod common;

use claw_sim::object::Rect;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::tiles::{SOLID, TileMap};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{
    ActivationConfig, Buttons, InputLatch, InputScript, SimConfig, SimLevel, TickInput,
};
use common::*;

fn trace_of(level: SimLevel, cfg: SimConfig, script: &InputScript) -> String {
    let mut sim = pressing_sim(level, cfg);
    let rows: Vec<TraceRow> = script
        .ticks
        .iter()
        .map(|&input| {
            sim.step(input);
            TraceRow::capture(&sim, sim.tick() - 1, "script")
        })
        .collect();
    trace::to_csv(&rows)
}

/// A course with walls, one-way platforms, a ladder, a pit and a ceiling.
fn course() -> SimLevel {
    let rows = [
        "############################################",
        "#..........................................#",
        "#..........................................#",
        "#.......######.............................#",
        "#.............H............................#",
        "#.............H............................#",
        "#.............H............................#",
        "#.............H...................#........#",
        "#.............H...................#........#",
        "#...=====.....H.......##...-----..#........#",
        "#.............H.......##...................#",
        "##########..##############################x#",
        "############################################",
    ];
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (300, 500))
}

fn random_script(seed: u32, n: usize) -> InputScript {
    let mut rng = Rng::new(seed);
    let keys = [JUMP, LEFT, RIGHT, UP, DOWN];
    let mut held = Vec::with_capacity(n);
    let mut cur = 0u32;
    let mut left = 0;
    for _ in 0..n {
        if left == 0 {
            cur = 0;
            for k in keys {
                if rng.below(100) < 35 {
                    cur |= k;
                }
            }
            left = 1 + rng.below(40);
        }
        left -= 1;
        held.push(cur);
    }
    InputScript::from_held(&held)
}

#[test]
fn two_runs_give_identical_traces() {
    let s = random_script(7, 3000);
    let a = trace_of(course(), SimConfig::default(), &s);
    let b = trace_of(course(), SimConfig::default(), &s);
    assert_eq!(a, b);
    assert!(a.starts_with(trace::HEADER));
    assert_eq!(a.lines().count(), 3001);
}

/// Feeds the script's button changes to a latch at a simulated display
/// rate: each change is seen at the first frame at or after its time and
/// timestamped with that frame; ticks run when their end has passed.
fn trace_at_rate(level: SimLevel, script: &InputScript, hz: u64) -> String {
    let mut sim = pressing_sim(level, SimConfig::default());
    let mut latch = InputLatch::new();
    let events = script.events();
    let mut next_event = 0;
    let mut rows = Vec::new();
    let mut tick = 0u64;
    let n = script.ticks.len() as u64;
    let mut frame = 0u64;
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            rows.push(TraceRow::capture(&sim, sim.tick() - 1, "script"));
            tick += 1;
        }
        frame += 1;
    }
    trace::to_csv(&rows)
}

#[test]
fn traces_are_identical_at_60_144_and_240_hz() {
    let s = random_script(11, 1500);
    let direct = trace_of(course(), SimConfig::default(), &s);
    for hz in [60, 144, 240] {
        assert_eq!(trace_at_rate(course(), &s, hz), direct, "{hz} Hz");
    }
}

#[test]
fn short_taps_between_frames_still_jump() {
    // A press and release inside one tick at 240 Hz reaches that tick.
    let mut latch = InputLatch::new();
    latch.event(4_167, Buttons::JUMP, true);
    latch.event(8_333, Buttons::JUMP, false);
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    sim.step(latch.take(0));
    assert_eq!(sim.player().unwrap().state, state::JUMP);
}

#[test]
fn widened_activation_is_recorded_in_traces() {
    let s = random_script(3, 50);
    let cfg = SimConfig {
        activation: ActivationConfig::widened((1706, 480)),
        ..SimConfig::default()
    };
    let t = trace_of(course(), cfg, &s);
    let row: Vec<&str> = t.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(&row[13..15], ["script", "widened"]);
}

fn solid_at(map: &TileMap, x: i32, y: i32) -> bool {
    map.attr(x, y) == SOLID
}

#[test]
fn claw_never_enters_solid_and_lands_only_from_above() {
    let level = course();
    let map = level.map.clone();
    let rect = Rect::new(-16, -52, 16, 60);
    let mut seen = std::collections::BTreeSet::new();
    let mut ground_landings = 0;
    for seed in 1..=12 {
        let mut sim = pressing_sim(level.clone(), SimConfig::default());
        let mut prev = sim.player().unwrap().clone();
        for (i, input) in random_script(seed, 2500).ticks.into_iter().enumerate() {
            sim.step(input);
            let o = sim.player().unwrap();
            let r = o.move_rect;
            let ctx = format!(
                "seed {seed} tick {i} at ({}, {}) state {}",
                o.x, o.y, o.state
            );
            // The body never overlaps Solid: its centre column and both
            // edges of the movement box, from the top to the feet.
            for x in [o.x + r.left, o.x, o.x + r.right] {
                let mut y = o.y + r.top;
                while y <= o.y + r.bottom {
                    assert!(!solid_at(&map, x, y), "inside Solid at ({x}, {y}): {ctx}");
                    y += o.move_res_y.max(1);
                }
                assert!(
                    !solid_at(&map, x, o.y + r.bottom),
                    "feet in Solid at {x}: {ctx}"
                );
            }
            // Landing moves down onto the surface, never up through it.
            let landed = prev.state == state::FALL && o.state != state::FALL;
            if landed && o.state != state::CLIMB && o.y > 64 {
                assert!(o.y >= prev.y, "landed from below: {ctx}");
                let below = o.y + rect.bottom + 1;
                let feet_free = (o.x + rect.left..=o.x + rect.right).any(|x| {
                    map.attr(x, below) != 0 && map.attr(x, below - 1) != map.attr(x, below)
                });
                assert!(feet_free, "landing has no surface under the feet: {ctx}");
                if map.attr(o.x, below) == claw_sim::tiles::GROUND {
                    ground_landings += 1;
                }
            }
            seen.insert(o.state);
            prev = o.clone();
        }
    }
    for s in [
        state::STAND,
        state::MOVE_RIGHT,
        state::MOVE_LEFT,
        state::JUMP,
        state::FALL,
        state::CROUCH,
        state::CLIMB,
        state::LOOK_UP,
    ] {
        assert!(seen.contains(&s), "the random runs never reached state {s}");
    }
    assert!(ground_landings > 0, "no landing on a one-way platform");
}

#[test]
fn seeds_change_the_random_stream_only() {
    let s = script(&[(RIGHT, 500)]);
    let a = trace_of(flat_level(), SimConfig::default(), &s);
    let b = trace_of(
        flat_level(),
        SimConfig {
            seed: 99,
            ..SimConfig::default()
        },
        &s,
    );
    // The traces differ only in the rng column, which records each run's
    // random stream state.
    let rng_at = trace::HEADER.split(',').position(|c| c == "rng").unwrap();
    let without_rng = |t: &str| -> Vec<String> {
        t.lines()
            .map(|l| {
                let mut cells: Vec<&str> = l.split(',').collect();
                cells.remove(rng_at);
                cells.join(",")
            })
            .collect()
    };
    assert_eq!(
        without_rng(&a),
        without_rng(&b),
        "walking draws no random numbers"
    );
    let first_rng = |t: &str| {
        t.lines()
            .nth(1)
            .unwrap()
            .split(',')
            .nth(rng_at)
            .unwrap()
            .to_string()
    };
    assert_eq!(first_rng(&a), "1", "{a}");
    assert_eq!(first_rng(&b), "99", "{b}");
    let _ = TickInput::default();
}
