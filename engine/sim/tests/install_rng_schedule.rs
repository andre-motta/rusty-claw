//! The random draws of the first level of your own install along the bot
//! route the original was run with at one 18 ms step per frame (skipped
//! without `CLAW_INSTALL`): at every tick where one of the stream's rules
//! shows, the game draws as many numbers as the original, and the state
//! of the stream after the tick is the original's.
//!
//! The rules, each with the tick it first shows at:
//!
//! 1. the small objects' window is 831x623: a crate stack 429 px right of
//!    the start wakes in tick 6 and its two crates roll from tick 8 (three
//!    draws there: the officer's roll, then the crates');
//! 2. an object made in the update pass joins the live list at once and
//!    runs in the same pass when it lands after the object the pass visits
//!    next: the four crates of the back stacks that wake in tick 17 roll in
//!    tick 17 (seven draws);
//! 3. a pickup's glitter is not run when it is made: the treasure woken in
//!    tick 241 makes its glitter, which draws its delay in tick 242;
//! 4. the ambient sounds' timers run once per tick after the rest of it,
//!    from the first tick: no draw in tick 547, the first flip in tick 548
//!    (after the officer's roll);
//! 5. a pacing enemy is at a limit only when its exact position is: no
//!    draw in tick 1029, the limit's draw in tick 1030;
//! 6. a woken enemy first moves for the time it slept, at most 200 ms: the
//!    officer woken in tick 2552 moves from 1924 to 1900 and is outside
//!    its window again, so tick 2555 has three draws, not four;
//! 7. Claw's hit burst draws its y jitter, then its x jitter: the
//!    officer's strike that hurts Claw in tick 476 (8586 ms, health 90).
//!
//! The stream then stays the original's until tick 2860 (51498 ms).

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::rng::Rng;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, Tuning};

/// The random state the reference runs of the first level start from.
const RNG_AT_T0: u32 = 4_105_512_405;

/// The route's first 2861 ticks as (held mask, ticks) runs.
const ROUTE: &[(u32, usize)] = &[
    (0x0, 2),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x0, 6),
    (0x2000000, 6),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 12),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 12),
    (0x4000000, 6),
    (0x2000001, 5),
    (0x2000000, 2),
    (0x1000000, 6),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 9),
    (0x2000000, 56),
    (0x2000001, 9),
    (0x2000000, 38),
    (0x8000000, 6),
    (0x2000001, 2),
    (0x2000000, 2),
    (0x2000001, 9),
    (0x2000000, 26),
    (0x2000001, 5),
    (0x2000000, 20),
    (0x2000001, 5),
    (0x2000000, 14),
    (0x0, 6),
    (0x2000001, 5),
    (0x2000000, 26),
    (0x0, 6),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x0, 6),
    (0x2, 2),
    (0x0, 6),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 2),
    (0x2000000, 2),
    (0x1000000, 6),
    (0x2000000, 12),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x2000001, 9),
    (0x2000000, 8),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x2000001, 9),
    (0x2000000, 56),
    (0x2000001, 2),
    (0x2000000, 56),
    (0x0, 6),
    (0x4000000, 6),
    (0x8000000, 6),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 12),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 18),
    (0x2000001, 5),
    (0x2000000, 26),
    (0x2000001, 9),
    (0x2000000, 50),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 30),
    (0x1000000, 6),
    (0x2, 2),
    (0x0, 12),
    (0x8000000, 6),
    (0x1, 14),
    (0x0, 2),
    (0x8000000, 6),
    (0x1, 14),
    (0x0, 8),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 6),
    (0x0, 6),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 86),
    (0x0, 12),
    (0x2000000, 12),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x1, 14),
    (0x0, 8),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 12),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 9),
    (0x2000000, 32),
    (0x2000001, 9),
    (0x2000000, 26),
    (0x0, 6),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x0, 6),
    (0x1, 14),
    (0x0, 2),
    (0x2000000, 30),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 6),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 26),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 5),
    (0x2000000, 8),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 14),
    (0x2000000, 32),
    (0x2000001, 14),
    (0x2000000, 20),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 9),
    (0x2000000, 14),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x0, 6),
    (0x1000000, 18),
    (0x0, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x2, 2),
    (0x0, 6),
    (0x2000000, 24),
    (0x2000001, 14),
    (0x2000000, 14),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 12),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 2),
    (0x1000000, 12),
    (0x4000000, 6),
    (0x2000001, 14),
    (0x2000000, 8),
    (0x0, 6),
    (0x2, 2),
    (0x0, 6),
    (0x1000001, 2),
    (0x1000000, 2),
    (0x2000000, 6),
    (0x1000000, 6),
    (0x0, 6),
    (0x1000001, 14),
    (0x1000000, 26),
    (0x8000000, 6),
    (0x2000001, 2),
    (0x2000000, 2),
    (0x1, 14),
    (0x0, 2),
    (0x2000000, 6),
    (0x1000000, 6),
    (0x0, 6),
    (0x2, 2),
    (0x0, 4),
];

/// (tick, draws in the tick, the original's random state after it).
const DRAWS: [(usize, usize, u32); 17] = [
    (6, 0, 499_382_750),
    (8, 3, 1_772_955_883),
    (17, 7, 229_811_694),
    (241, 1, 1_318_656_186),
    (242, 7, 1_741_779_371),
    (475, 0, 1_621_690_865),
    (476, 10, 615_850_395),
    (547, 0, 4_261_011_266),
    (548, 2, 3_852_154_828),
    (1028, 3, 3_051_788_951),
    (1029, 0, 3_051_788_951),
    (1030, 1, 4_014_467_838),
    (2552, 4, 746_887_823),
    (2553, 0, 746_887_823),
    (2554, 0, 746_887_823),
    (2555, 3, 910_336_576),
    (2860, 0, 4_028_153_535),
];

fn level1() -> Option<SimLevel> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(SimLevel::from_level(&level, &store).expect("level 1 prepares"))
}

fn draws_between(from: u32, to: u32) -> usize {
    let mut r = Rng::new(from);
    for n in 0..=100 {
        if r.state() == to {
            return n;
        }
        r.next_u15();
    }
    usize::MAX
}

/// The officer placed at (2160, 4284).
fn officer(sim: &Sim) -> Option<(i32, i32, bool)> {
    sim.objects().all().find_map(|(_, o)| {
        let r = o.record.as_ref()?;
        (&*o.logic == "Officer" && r[5] == 2160 && r[6] == 4284).then_some((
            o.x,
            o.y,
            o.is_listed(),
        ))
    })
}

#[test]
fn install_level1_draws_as_the_original_along_the_route() {
    let Some(level) = level1() else { return };
    let mut held = Vec::new();
    for &(mask, n) in ROUTE {
        held.extend(std::iter::repeat_n(mask, n));
    }
    assert_eq!(held.len(), 2861);
    let script = InputScript::from_held(&held);
    let cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        seed: RNG_AT_T0,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    assert_eq!(sim.rng_after_set_up(), RNG_AT_T0);
    let mut stacked = 0;
    for (tick, t) in script.ticks.iter().enumerate() {
        let before = sim.rng_state();
        let woke_from = (tick == 2552).then(|| officer(&sim)).flatten();
        sim.step(*t);
        let after = sim.rng_state();
        if let Some(&(_, n, want)) = DRAWS.iter().find(|d| d.0 == tick) {
            assert_eq!(
                (draws_between(before, after), after),
                (n, want),
                "tick {tick} (t_ms {})",
                18 * (tick + 1)
            );
        }
        match tick {
            // Rule 1: the front stack at 1118,4840 has made its two crates.
            6 => {
                stacked = sim
                    .objects()
                    .listed()
                    .into_iter()
                    .filter(|&id| {
                        let o = sim.objects().get(id).unwrap();
                        &*o.logic == "Crate" && (o.x - 1118).abs() < 64
                    })
                    .count();
            }
            // Rule 7's tick: the officer's strike hurts Claw.
            476 => {
                let claw = sim.player().expect("Claw");
                assert_eq!((claw.state, sim.stats().health), (31, 90));
            }
            // Rule 6: asleep at 1924 before the tick, it wakes, first moves
            // 24 px and is outside its window again by the tick's end.
            2552 => {
                assert_eq!(woke_from.map(|o| (o.0, o.2)), Some((1924, false)));
                assert_eq!(officer(&sim).map(|o| (o.0, o.2)), Some((1900, false)));
            }
            _ => {}
        }
    }
    assert_eq!(stacked, 2, "the front stack's crates are live in tick 6");
}
