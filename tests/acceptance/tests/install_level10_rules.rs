//! Two rules of the tenth level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of a route where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's.
//!
//! The route is `routes/earlier/level10-original.csv`, the tenth level's
//! original-speeds route as it was before these rules, played from the
//! level's start; `routes/level10-original.csv`, found again under the
//! rules, keeps its first 14799 ticks:
//!
//! - an enemy's move up or down that would take it past the top or bottom
//!   of its range puts it there, with the speed the energy of the move to
//!   it gives and no gravity added: the parrot carrying Marrow from tick
//!   14672 reaches the top of its range, its flight line at y 7029, and
//!   stays on it from tick 14717 to the carry's end, with Marrow hanging
//!   at y 7124, level with Claw jumping on the far ledge. Dropped at
//!   (40606, 7253) in tick 14798, Marrow goes idle in the next tick
//!   instead of shooting, and goes Home when Claw lands in tick 14817, making Home's
//!   draw in tick 14818;
//! - the respawn shows a frame of its own once its passes are over, which
//!   marks what it draws: Claw, without the bridge the earlier route
//!   counted on, dies on the pit's spikes in tick 15629 and respawns in
//!   tick 15798, where Marrow, put back on his ledge and drawn by that
//!   frame, sees him and goes Home in that tick, making Home's draws in
//!   tick 15799 (with the mark of the last frame before the death he
//!   would see him a tick later). The route then loses two more lives and
//!   ends without the level's end, as it did in the original game, and the
//!   stream stays the original's to the original run's last tick, 112
//!   ticks past the route's last row, with nothing held.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the earlier
/// route.
const EARLIER_RNG: [(usize, u32); 23] = [
    (14671, 3_143_434_138),
    (14672, 3_252_361_956),
    (14717, 957_116_821),
    (14720, 1_499_993_271),
    (14798, 1_243_285_915),
    (14799, 4_024_105_980),
    (14800, 1_333_304_015),
    (14816, 2_017_393_211),
    (14817, 1_083_333_650),
    (14818, 1_550_588_700),
    (15624, 3_781_115_327),
    (15628, 1_507_291_717),
    (15629, 1_070_148_110),
    (15797, 2_678_630_713),
    (15798, 4_189_030_582),
    (15799, 2_233_612_323),
    (15800, 4_099_437_476),
    (17123, 3_921_973_118),
    (17124, 3_292_272_049),
    (17817, 3_402_740_379),
    (17818, 2_326_550_721),
    (18566, 4_148_772_848),
    (18678, 1_560_528_909),
];

/// Ticks the original run of the earlier route lasted.
const EARLIER_RUN_TICKS: usize = 18679;

/// The parrot's flight line: the top of its range, y.
const FLIGHT_LINE: i32 = 7029;

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

/// A route played tick by tick.
struct Run {
    sim: Sim,
    script: InputScript,
    /// The next tick to play.
    tick: usize,
    /// The random state after each tick played.
    rng: Vec<u32>,
}

impl Run {
    fn new(level: &SimLevel, text: &str) -> Run {
        let script = InputScript::parse(text).unwrap();
        let seed = script.header.seed.expect("the route names its seed");
        let sim = Sim::new(level.clone(), route::sim_config(TuningName::Original, seed));
        Run {
            sim,
            script,
            tick: 0,
            rng: Vec::new(),
        }
    }

    /// Plays ticks until `last` has been played, with nothing held past
    /// the route's last row.
    fn to(&mut self, last: usize) {
        while self.tick <= last {
            self.sim.play_directives(&self.script, self.tick);
            let input = self
                .script
                .ticks
                .get(self.tick)
                .copied()
                .unwrap_or_else(TickInput::default);
            self.sim.step(input);
            self.rng.push(self.sim.rng_state());
            self.tick += 1;
            assert!(
                !self
                    .sim
                    .last_events()
                    .iter()
                    .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
                "no level end"
            );
        }
    }

    fn check_rng(&self) {
        for &(t, want) in &EARLIER_RNG {
            if t < self.rng.len() {
                assert_eq!(self.rng[t], want, "random state after tick {t}");
            }
        }
    }

    /// Marrow: (x, y, state).
    fn marrow(&self) -> (i32, i32, i32) {
        let link = self.sim.marrow_link().expect("Marrow's link");
        let o = self.sim.objects().get(link.marrow).expect("Marrow");
        (o.x, o.y, o.state)
    }

    /// The parrot: (x, y).
    fn parrot(&self) -> (i32, i32) {
        let mut parrots = self
            .sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == "Parrot")
            .map(|(_, o)| (o.x, o.y));
        let p = parrots.next().expect("the parrot");
        assert!(parrots.next().is_none(), "one parrot");
        p
    }

    fn claw_at(&self) -> (i32, i32) {
        let c = self.sim.player().expect("Claw");
        (c.x, c.y)
    }
}

#[test]
fn install_the_route_found_again_keeps_the_earlier_routes_start() {
    if install().is_none() {
        return;
    }
    let earlier = InputScript::parse(&routes_file("earlier/level10-original.csv")).unwrap();
    let now = InputScript::parse(&routes_file("level10-original.csv")).unwrap();
    assert_eq!(earlier.ticks[..14799], now.ticks[..14799]);
}

#[test]
fn install_the_parrot_carries_marrow_on_its_flight_line() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(14716);
    assert!(run.parrot().1 > FLIGHT_LINE, "still rising");
    // On the line from tick 14717 to the carry's end, Marrow 95 px under
    // it; with gravity added at the line the bird would come down again
    // some twenty ticks later, and Marrow with it.
    for t in 14717..=14797 {
        run.to(t);
        assert_eq!(run.parrot().1, FLIGHT_LINE, "the parrot after tick {t}");
        assert_eq!(run.marrow().1, 7124, "Marrow after tick {t}");
    }
    assert_eq!(run.parrot(), (40551, FLIGHT_LINE));
    // Dropped, he is out of range of Claw again: he goes idle, not into
    // his shot, and stays idle until Claw lands, when he goes Home.
    run.to(14798);
    let (x, y, _) = run.marrow();
    assert_eq!((x, y), (40606, 7253));
    for t in 14799..=14816 {
        run.to(t);
        assert_eq!(run.marrow().2, EnemyState::Idle as i32, "after tick {t}");
    }
    run.to(14817);
    assert_eq!(run.marrow().2, EnemyState::Home as i32);
    run.to(14818);
    run.check_rng();
}

#[test]
fn install_the_respawn_shows_a_frame_of_its_own() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(15628);
    assert_eq!(run.sim.stats().lives, 7);
    run.to(15629);
    assert_eq!(run.sim.stats().health, 0, "on the pit's spikes");
    run.to(15797);
    assert_eq!(run.sim.stats().lives, 6);
    run.to(15798);
    assert_eq!(run.claw_at(), (40492, 7245));
    // Drawn by the respawn's frame, Marrow sees Claw at once and goes Home.
    let (x, y, st) = run.marrow();
    assert_eq!((x, y), (41063, 7253));
    assert_eq!(st, EnemyState::Home as i32);
    run.to(15800);
    run.check_rng();
}

#[test]
fn install_the_earlier_route_follows_the_original_to_its_runs_end() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(17123);
    assert_eq!(run.sim.stats().lives, 5);
    run.to(17817);
    assert_eq!(run.sim.stats().lives, 4);
    run.to(EARLIER_RUN_TICKS - 1);
    assert_eq!(run.claw_at(), (40681, 7404));
    assert_eq!(run.sim.stats().score, 177_700);
    run.check_rng();
}
