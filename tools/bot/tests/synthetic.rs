//! The search on small levels built in code: a gap, a ladder and a
//! stand-in hazard, and the same seed giving the same route.

use claw_bot::actions;
use claw_bot::route::{self, spacing_violations};
use claw_bot::{Goal, SearchConfig, search, search_report};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, TileMap};

/// A level from text rows of 64 px tiles (see [`TileMap::from_ascii`]).
fn level(rows: &[String], start: (i32, i32)) -> SimLevel {
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), start)
}

fn sim(level: SimLevel) -> Sim {
    Sim::new(level, SimConfig::default())
}

fn cfg() -> SearchConfig {
    SearchConfig {
        nodes: 4_000,
        threads: 2,
        ..SearchConfig::default()
    }
}

/// Searches, then checks that the route wins when played from the start,
/// with no death and the spacing rule kept.
fn solve(start: &Sim, goal: Goal) -> InputScript {
    let script = search(start, goal.clone(), &cfg()).expect("a route");
    let played = route::play(start, &script, &goal);
    assert!(played.won(), "{played:?}");
    assert!(spacing_violations(&script, 2).is_empty());
    script
}

/// Rows of an open level `w` tiles wide and `h` tall, with a solid floor
/// row at the bottom.
fn room(w: usize, h: usize) -> Vec<String> {
    let mut rows = vec![".".repeat(w); h - 1];
    rows.push("#".repeat(w));
    rows
}

#[test]
fn crosses_a_gap_over_a_pit() {
    // A floor at row 9 with a pit two tiles wide whose bottom is death.
    let mut rows = room(30, 12);
    for r in rows.iter_mut().take(11).skip(9) {
        *r = "#".repeat(30);
    }
    for r in rows.iter_mut().skip(9) {
        r.replace_range(14..16, "..");
    }
    rows[11].replace_range(14..16, "xx");
    let start = sim(level(&rows, (5 * 64, 8 * 64)));
    let script = solve(
        &start,
        Goal::Reach {
            x: 22 * 64,
            y: 9 * 64 - 61,
            radius: 40,
        },
    );
    assert!(script.ticks.len() < 400);
}

#[test]
fn climbs_a_ladder_to_a_ledge() {
    // A ledge at row 4 over the right half, reached only by the ladder in
    // column 12 (far too high to jump).
    let mut rows = room(30, 12);
    rows[4].replace_range(13..30, &"#".repeat(17));
    for r in rows.iter_mut().take(11).skip(3) {
        r.replace_range(12..13, "H");
    }
    let start = sim(level(&rows, (4 * 64, 9 * 64)));
    solve(
        &start,
        Goal::Reach {
            x: 20 * 64,
            y: 4 * 64 - 61,
            radius: 40,
        },
    );
}

#[test]
fn jumps_a_stand_in_hazard() {
    // Death tiles set into the floor between Claw and the goal.
    let mut rows = room(30, 10);
    rows[9].replace_range(13..15, "xx");
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let found = search_report(&start, goal, &cfg()).expect("a route");
    assert_eq!(found.stats.deaths, 0);
    // Walking straight into the hazard is a death the search saw and
    // dropped.
    assert!(found.stats.deaths_pruned > 0);
}

#[test]
fn the_same_seed_gives_the_same_route_on_any_thread_count() {
    let mut rows = room(30, 10);
    rows[9].replace_range(13..15, "xx");
    rows[7].replace_range(18..21, "---");
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 19 * 64,
        y: 7 * 64 - 61,
        radius: 40,
    };
    let run = |seed: u64, threads: usize| {
        let c = SearchConfig {
            seed,
            threads,
            ..cfg()
        };
        search_report(&start, goal.clone(), &c).expect("a route")
    };
    let a = run(7, 1);
    let b = run(7, 4);
    assert_eq!(a.script, b.script);
    assert_eq!(a.stats, b.stats);
    assert_eq!(run(7, 1).script, a.script);
}

#[test]
fn an_unreachable_goal_fails_within_the_budget() {
    // The goal is sealed in rock.
    let mut rows = room(20, 10);
    rows[2] = "#".repeat(20);
    rows[3] = "#".repeat(20);
    let start = sim(level(&rows, (3 * 64, 7 * 64)));
    let c = SearchConfig {
        nodes: 300,
        ..cfg()
    };
    let err = search(
        &start,
        Goal::Reach {
            x: 10 * 64,
            y: 64,
            radius: 20,
        },
        &c,
    )
    .unwrap_err();
    assert!(err.stats.expanded <= 300);
}

#[test]
fn a_search_from_part_way_through_a_route_keeps_the_spacing_across_the_join() {
    let rows = room(30, 10);
    let fresh = sim(level(&rows, (5 * 64, 7 * 64)));
    // A prefix that changes its held mask in its very last tick.
    let right = claw_sim::Buttons::RIGHT.0;
    let mut held = vec![0u32; 20];
    held.push(right);
    let prefix = InputScript::from_held(&held);
    let mut start = fresh.clone();
    for &t in &prefix.ticks {
        start.step(t);
    }
    let goal = Goal::Reach {
        x: 12 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let c = SearchConfig {
        start_held: right,
        start_held_for: 1,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    // The route found first keeps the prefix's mask for the missing tick.
    assert_eq!(found.script.ticks[0].held.0, right);
    assert_eq!(found.script.ticks[0].pressed.0, 0);
    let mut whole = prefix.clone();
    whole.ticks.extend(found.script.ticks.iter().copied());
    assert!(spacing_violations(&whole, 2).is_empty());
    // Pressed buttons are the rising edges across the join, so the route
    // written is the route the search played.
    let masks: Vec<u32> = whole.ticks.iter().map(|t| t.held.0).collect();
    assert_eq!(whole, InputScript::from_held(&masks));
    assert!(route::play(&fresh, &whole, &goal).won());
}

#[test]
fn a_route_from_the_level_start_presses_nothing_in_its_first_ticks() {
    // Keys down when play starts count only once released and pressed
    // again, so the search idles first.
    let rows = room(30, 10);
    let start = sim(level(&rows, (5 * 64, 7 * 64)));
    let goal = Goal::Reach {
        x: 12 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let script = solve(&start, goal);
    assert!(script.ticks[..2].iter().all(|t| t.held.0 == 0));
    assert_ne!(script.ticks[2].held.0, 0, "then it goes");
}

/// The route ticks at which Claw lands and at which jump is pressed, when
/// `script` is played from `start`.
fn landings_and_jumps(start: &Sim, script: &InputScript) -> (Vec<usize>, Vec<usize>) {
    let mut sim = start.clone();
    let (mut lands, mut jumps) = (Vec::new(), Vec::new());
    for (k, &t) in script.ticks.iter().enumerate() {
        let air = actions::phase(&sim) == actions::Phase::Air;
        sim.step(t);
        if air && actions::phase(&sim) != actions::Phase::Air {
            lands.push(k);
        }
        if t.pressed.0 & actions::JUMP != 0 {
            jumps.push(k);
        }
    }
    (lands, jumps)
}

#[test]
fn with_a_landing_gap_no_jump_is_pressed_near_a_landing() {
    // Steps up a staircase of ledges: jump after jump.
    let mut rows = room(40, 12);
    for (i, r) in rows.iter_mut().enumerate().take(11).skip(5) {
        let from = 10 + 4 * (10 - i);
        r.replace_range(from..40, &"#".repeat(40 - from));
    }
    let start = sim(level(&rows, (3 * 64, 9 * 64)));
    let goal = Goal::Reach {
        x: 36 * 64,
        y: 5 * 64 - 61,
        radius: 48,
    };
    let c = SearchConfig {
        land_gap: 2,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    assert!(route::play(&start, &found.script, &goal).won());
    let (lands, jumps) = landings_and_jumps(&start, &found.script);
    assert!(lands.len() >= 2 && jumps.len() >= 2, "{lands:?} {jumps:?}");
    for j in &jumps {
        assert!(
            lands.iter().all(|l| l.abs_diff(*j) > 2),
            "jump at {j}, landings {lands:?}"
        );
    }
}

#[test]
fn a_segment_starts_with_claw_settled_and_the_seed_set() {
    let rows = room(30, 10);
    let mut start = sim(level(&rows, (5 * 64, 7 * 64)));
    route::settle_at(&mut start, 15 * 64, 6 * 64, 111_567);
    let o = start.player().unwrap();
    assert_eq!(o.x, 15 * 64);
    assert_eq!(o.y + o.move_rect.bottom, 9 * 64 - 1, "on the floor");
    assert_eq!(o.state, claw_sim::player::state::STAND);
    assert_eq!(start.rng_state(), 111_567);
    // The search goes on from there.
    let goal = Goal::Reach {
        x: 20 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    solve(&start, goal);
}

#[test]
fn with_the_fragility_check_a_route_still_wins_with_any_change_a_tick_off() {
    // A gap over a pit, crossed with room to spare.
    let mut rows = room(30, 12);
    for r in rows.iter_mut().take(11).skip(9) {
        *r = "#".repeat(30);
    }
    for r in rows.iter_mut().skip(9) {
        r.replace_range(14..16, "..");
    }
    rows[11].replace_range(14..16, "xx");
    let start = sim(level(&rows, (5 * 64, 8 * 64)));
    let goal = Goal::Reach {
        x: 22 * 64,
        y: 9 * 64 - 61,
        radius: 40,
    };
    let c = SearchConfig {
        fragile_cost: 400,
        ..cfg()
    };
    let found = search_report(&start, goal.clone(), &c).expect("a route");
    assert!(route::play(&start, &found.script, &goal).won());
    // Some moves tried fail with their timing shifted; none is on the
    // route found.
    assert!(found.stats.fragile > 0, "{:?}", found.stats);
    assert_eq!(found.stats.fragile_on_route, 0);
    let m = route::margin(&start, &found.script, &goal, 40);
    assert!(m.sampled > 0);
    assert_eq!((m.early_wins, m.late_wins), (m.sampled, m.sampled), "{m:?}");
}
