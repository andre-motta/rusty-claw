//! Rules of the eleventh level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of
//! `routes/level11-original.csv`, played from the level's start, where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's, and by the trace the
//! game writes:
//!
//! - the trace records the keys held as the game's controls saw them: a
//!   Siren's scream touches Claw in ticks 1017 and 6726 and takes his
//!   controls for the next 111 ticks; the route holds Right from tick 1126
//!   and Left from tick 6835, so the trace's held column is 0 through
//!   ticks 1128 and 6837, though the tick's input holds the key, and Claw
//!   walks from ticks 1129 and 6838 with the key held then;
//! - Mercat charges by himself only on a low roll: his wait runs out in
//!   tick 7777 in his walking range, his roll of 0..101 is 36, so he says
//!   no line and stays Home, and the shared human Home draws its 0..421
//!   and sends him walking in. Two draws in that tick, as in the original.
//!
//! The route ends the level in tick 11973 with 83 health, a score of
//! 431600 and 7 lives.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::trace::TraceRow;
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the route.
const RNG: [(usize, u32); 12] = [
    (1016, 1_104_240_624),
    (1017, 2_178_841_861),
    (1128, 536_860_357),
    (1129, 2_756_846_439),
    (6726, 3_238_399_686),
    (6837, 3_184_629_829),
    (6838, 3_184_629_829),
    (7776, 4_003_863_765),
    (7777, 4_234_222_071),
    (7778, 4_234_222_071),
    (7779, 600_698_078),
    (11973, 3_691_619_702),
];

/// The ticks a scream touched Claw in.
const SCREAMS: [usize; 2] = [1017, 6726];

/// Ticks of the scream lockout after the tick of the scream.
const LOCKED_TICKS: usize = 111;

/// The keys the route holds at the lockouts' ends: Right, then Left.
const HELD_AT_END: [(usize, u32); 2] = [(1126, 0x200_0000), (6835, 0x100_0000)];

/// The tick the route ends the level in.
const LAST_TICK: usize = 11973;

fn level(root: &Path, n: u32) -> SimLevel {
    let store = Install::open(root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, &store).unwrap()
}

fn routes_file(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("routes")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn install_the_eleventh_levels_route_follows_the_original_through_its_rules() {
    let Some(root) = install() else {
        return;
    };
    let level = level(&root, 11);
    let script = InputScript::parse(&routes_file("level11-original.csv")).unwrap();
    let seed = script.header.seed.expect("the route names its seed");
    let mut sim = Sim::new(level, route::sim_config(TuningName::Original, seed));
    let mut rows = Vec::with_capacity(LAST_TICK + 1);
    let mut rng = Vec::with_capacity(LAST_TICK + 1);
    let mut locked = Vec::with_capacity(LAST_TICK + 1);
    let mut ended = None;
    for t in 0..=LAST_TICK {
        sim.play_directives(&script, t);
        let input = script
            .ticks
            .get(t)
            .copied()
            .unwrap_or_else(TickInput::default);
        sim.step(input);
        rows.push(TraceRow::capture(&sim, t as u64, "script"));
        rng.push(sim.rng_state());
        locked.push(sim.input_locked());
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            ended = Some(t);
            break;
        }
    }
    for &(t, want) in &RNG {
        assert_eq!(rng[t], want, "random state after tick {t}");
    }

    // The lockouts: the tick of the scream and the tick after the last
    // locked one keep their keys; the ticks between are locked.
    for &hit in &SCREAMS {
        assert!(!locked[hit], "tick {hit}");
        for (t, &l) in locked.iter().enumerate().skip(hit + 1).take(LOCKED_TICKS) {
            assert!(l, "tick {t} is locked");
        }
        assert!(
            !locked[hit + LOCKED_TICKS + 1],
            "tick {}",
            hit + LOCKED_TICKS + 1
        );
    }
    assert_eq!(
        locked.iter().filter(|&&l| l).count(),
        SCREAMS.len() * LOCKED_TICKS,
        "no other tick is locked"
    );
    for &(from, key) in &HELD_AT_END {
        for (t, (input, row)) in script.ticks[from..from + 3]
            .iter()
            .zip(&rows[from..from + 3])
            .enumerate()
        {
            let t = from + t;
            assert_eq!(input.held.0, key, "the route holds the key in tick {t}");
            assert_eq!(row.held, 0, "the trace holds nothing in tick {t}");
        }
        let free = from + 3;
        assert_eq!(rows[free].held, key, "tick {free}");
        assert_ne!(rows[free].x, rows[free - 1].x, "Claw walks in tick {free}");
        assert_eq!(rows[free - 1].x, rows[free - 2].x);
    }
    // Every other row holds the route's keys.
    for (t, row) in rows.iter().enumerate() {
        if !locked[t] {
            let held = script.ticks.get(t).map_or(0, |i| i.held.0);
            assert_eq!(row.held, held, "tick {t}");
        }
    }

    assert_eq!(ended, Some(LAST_TICK), "the level's end");
    let end = &rows[LAST_TICK];
    assert_eq!((end.health, end.score, end.lives), (83, 431_600, 7));
}
