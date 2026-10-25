//! Rules of the thirteenth level's play on your own install (skipped
//! without `CLAW_INSTALL`), pinned to a moment of
//! `routes/level13-introskip.csv`, played from the level's start, where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's:
//!
//! - RedTail's blow count after an intro skip: the route jumps in at tick
//!   13250, skipping his introduction, and its jump slash reaches him in
//!   tick 13322 while he still waits in his start-up pose on his ledge. In
//!   a game played from the level's start his count is already past its
//!   limit there, so that first blow is a third one: no damage (he keeps
//!   his 100 health), the count back to 0, and one draw in the tick, his
//!   own per-pulse roll; Claw standing below on the sand, his leap away
//!   only sends him home, in tick 13323.
//!
//! The route ends the level in tick 14579 with 60 health and 7 lives, no
//! life lost.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::enemy::redtail;
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the route.
const RNG: [(usize, u32); 3] = [
    (13321, 1_891_090_372),
    (13322, 3_158_011_767),
    (13323, 3_158_011_767),
];

/// The tick the jump slash reaches RedTail in.
const BLOW: usize = 13322;

/// The tick the route ends the level in.
const LAST_TICK: usize = 14579;

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

/// RedTail's state, health and blow count, once he is built.
fn redtail_now(sim: &Sim) -> Option<(Option<EnemyState>, i32, u32)> {
    let o = sim.objects().get(sim.redtail_link().redtail?)?;
    match &o.data {
        ObjectData::RedTail(d) => Some((EnemyState::from_id(o.state), o.health, d.hits)),
        _ => None,
    }
}

#[test]
fn install_the_thirteenth_levels_intro_skip_route_follows_the_original_through_its_rules() {
    let Some(root) = install() else {
        return;
    };
    let level = level(&root, 13);
    let script = InputScript::parse(&routes_file("level13-introskip.csv")).unwrap();
    let seed = script.header.seed.expect("the route names its seed");
    let mut sim = Sim::new(level, route::sim_config(TuningName::Original, seed));
    let mut rng = Vec::with_capacity(LAST_TICK + 1);
    let mut redtail = Vec::with_capacity(LAST_TICK + 1);
    let mut ended = None;
    for t in 0..=LAST_TICK {
        sim.play_directives(&script, t);
        let input = script
            .ticks
            .get(t)
            .copied()
            .unwrap_or_else(TickInput::default);
        sim.step(input);
        rng.push(sim.rng_state());
        redtail.push(redtail_now(&sim));
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            ended = Some(t);
            break;
        }
    }
    assert!(!sim.claw_placed(), "played from the level's start");
    for &(t, want) in &RNG {
        assert_eq!(rng[t], want, "random state after tick {t}");
    }
    // Built with his count past its limit; the skip leaves it there.
    assert_eq!(
        redtail[BLOW - 1],
        Some((
            Some(EnemyState::Startup),
            redtail::HEALTH,
            redtail::HIT_COUNTER_LEVEL_START
        ))
    );
    // The blow: unhurt, the count back to 0, still waiting; then home.
    assert_eq!(
        redtail[BLOW],
        Some((Some(EnemyState::Startup), redtail::HEALTH, 0))
    );
    assert_eq!(
        redtail[BLOW + 1],
        Some((Some(EnemyState::Home), redtail::HEALTH, 0))
    );

    assert_eq!(ended, Some(LAST_TICK), "the level ends in tick {LAST_TICK}");
    let stats = sim.stats();
    assert_eq!((stats.health, stats.lives), (60, 7));
}
