//! The search on small levels built in code: a gap, a ladder and a
//! stand-in hazard, and the same seed giving the same route.

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
