//! Three rules of the eighth level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of a route where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's.
//!
//! The routes are `routes/earlier/level8-original.csv`, the eighth level's
//! original-speeds route as it was before these rules, played from the
//! level's start and as the second level of a game (with the progress
//! `routes/level7-original.level8.csv` starts with), and
//! `routes/level8-introskip.csv`, the jump-in route:
//!
//! - an effect leaves its first-call state once its first call has set it
//!   up: Catnip's glitter driver, which Claw carries through the warp
//!   into Gabriel's room, is then not run again by the warp's set-up
//!   pass, and it sheds its two sparkles (four draws) only in the first
//!   tick in the room. Claw touches the door in tick 15571 of the first
//!   route and in tick 15581 of the jump-in route;
//! - a pirate Gabriel called in keeps his fight's mode for good: Claw
//!   dies in tick 17048 of the first route while the pirate Gabriel called
//!   in at tick 17020 leaps from his deck, and the respawn's reset clears
//!   Gabriel's list of pirates; the leaping pirate still takes only its
//!   leap's step in the respawn's passes, with no per-pulse roll. The
//!   route then ends without the level's end, as it did in the original
//!   game, and the stream stays the original's to the original run's last
//!   tick, 112 ticks past the route's last row, with nothing held;
//! - an enemy marked as riding a platform object moves by the platform
//!   only: Claw throws Gabriel onto a peg that reaches past the dock's
//!   edge in the jump-in route. He lands on it in tick 15839 and the ledge
//!   guard, which looks at the tiles only, puts him back 2 px above it,
//!   still marked; his first bounce step (tick 15840) does not move him,
//!   his second bounce (tick 15856) lands him on the peg at once, and the
//!   landing costs him his 4 health in tick 15886, as in the original
//!   game, which the route then follows to the level's end.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::enemy::{EnemyState, redtail_pirate};
use claw_sim::logic::{ObjectData, effect};
use claw_sim::object::{Object, ObjectId, draw_flags, flags};
use claw_sim::physics::ptype;
use claw_sim::{InputScript, Sim, SimLevel, StartState, TickInput};

/// The original game's random state after these ticks of the earlier
/// route, from the level's start or as the game's second level.
const EARLIER_RNG: [(usize, u32); 16] = [
    (15571, 64_002_068),
    (15572, 991_741_522),
    (15573, 289_898_678),
    (16000, 3_606_127_538),
    (16500, 2_764_686_936),
    (16637, 1_395_651_243),
    (17020, 1_817_087_766),
    (17040, 915_087_436),
    (17048, 4_038_326_932),
    (17049, 3_278_242_299),
    (17050, 3_278_242_299),
    (17051, 3_563_865_298),
    (17200, 3_888_861_841),
    (17299, 1_524_377_404),
    (17350, 850_874_024),
    (17411, 3_976_633_367),
];

/// The original game's random state after these ticks of the jump-in
/// route.
const INTRO_SKIP_RNG: [(usize, u32); 3] = [
    (15581, 1_715_156_163),
    (15582, 18_660_521),
    (15583, 803_775_037),
];

/// The original game's random state after these ticks of the jump-in
/// route, around Gabriel's bounces on the peg and to the level's end.
const BOUNCE_RNG: [(usize, u32); 8] = [
    (15839, 270_162_372),
    (15840, 270_162_372),
    (15856, 2_642_138_242),
    (15857, 2_309_728_573),
    (15885, 928_688_468),
    (15887, 2_132_864_889),
    (16000, 695_898_116),
    (16562, 958_507_125),
];

/// Where Gabriel lands on the peg past the dock's edge in the jump-in
/// route, and stays through his first bounce step.
const ON_THE_PEG: (i32, i32) = (42857, 5062);

/// Ticks the original run of the earlier route lasted.
const EARLIER_RUN_TICKS: usize = 17412;

/// Where the boss room's door puts Claw.
const ROOM: (i32, i32) = (43063, 5212);

fn level8(root: &Path) -> SimLevel {
    let store = Install::open(root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(8)).unwrap();
    SimLevel::from_level(&data, &store).unwrap()
}

fn routes_file(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("routes")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The earlier route as the game's second level: its rows, with the
/// progress the game's eighth level starts with.
fn earlier_in_game() -> String {
    let rows = routes_file("earlier/level8-original.csv");
    let game = routes_file("level7-original.level8.csv");
    let starts: String = game
        .lines()
        .filter(|l| l.starts_with("# start_"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(
        !starts.is_empty(),
        "the game's eighth level carries progress"
    );
    format!("{starts}{rows}")
}

/// A route of the eighth level played tick by tick.
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
        let cfg = route::sim_config(TuningName::Original, seed);
        let sim = match route::parse_carry(text).unwrap() {
            Some(carry) => Sim::with_start(
                level.clone(),
                cfg,
                StartState {
                    carry: Some(carry),
                    save: None,
                },
            ),
            None => Sim::new(level.clone(), cfg),
        };
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
            let input = self
                .script
                .ticks
                .get(self.tick)
                .copied()
                .unwrap_or_else(TickInput::default);
            self.sim.step(input);
            self.rng.push(self.sim.rng_state());
            self.tick += 1;
        }
    }

    fn check_rng(&self, table: &[(usize, u32)]) {
        for &(t, want) in table {
            if t < self.rng.len() {
                assert_eq!(self.rng[t], want, "random state after tick {t}");
            }
        }
    }

    fn of_logic(&self, logic: &str) -> Vec<ObjectId> {
        self.sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == logic)
            .map(|(id, _)| id)
            .collect()
    }

    fn claw_at(&self) -> (i32, i32) {
        let c = self.sim.player().expect("Claw");
        (c.x, c.y)
    }

    /// Gabriel: his object and the speed up or down his movement keeps.
    fn gabriel(&self) -> (&Object, f64) {
        self.sim
            .objects()
            .all()
            .find_map(|(_, o)| match &o.data {
                ObjectData::Gabriel(d) => Some((o, d.enemy.vy)),
                _ => None,
            })
            .expect("Gabriel")
    }

    /// Gabriel's list of the pirates he called in.
    fn his_pirates(&self) -> Vec<ObjectId> {
        self.sim
            .objects()
            .all()
            .find_map(|(_, o)| match &o.data {
                ObjectData::Gabriel(d) => Some(d.summoned.clone()),
                _ => None,
            })
            .expect("Gabriel")
    }
}

/// Plays the earlier route (`text`) through the door: the glitter driver
/// Claw carries in is set up and not run again by the warp.
fn through_the_door(level: &SimLevel, text: &str) -> Run {
    let mut run = Run::new(level, text);
    run.to(15570);
    let drivers = run.of_logic("GlitterMother");
    assert_eq!(drivers.len(), 1, "Claw carries Catnip's glitter");
    let o = run.sim.objects().get(drivers[0]).unwrap();
    assert_eq!(o.state, effect::state::SHEDDING);
    assert!(o.draw_flags & draw_flags::HIDDEN != 0);
    run.to(15571);
    assert_ne!(run.claw_at(), ROOM, "at the door");
    run.to(15572);
    assert_eq!(run.claw_at(), ROOM, "in the room");
    assert!(
        run.sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Checkpoint { .. }))
    );
    run.to(15573);
    run.check_rng(&EARLIER_RNG);
    run
}

#[test]
fn install_the_warp_does_not_run_the_glitter_again() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    through_the_door(&level, &routes_file("earlier/level8-original.csv"));
    through_the_door(&level, &earlier_in_game());
    // The jump-in route touches the door in tick 15581 and jumps in the
    // room's first tick.
    let mut run = Run::new(&level, &routes_file("level8-introskip.csv"));
    run.to(15581);
    assert_ne!(run.claw_at(), (43062, 5204));
    run.to(15583);
    run.check_rng(&INTRO_SKIP_RNG);
    let c = run.sim.player().unwrap();
    assert_eq!((c.x, c.y), (43066, 5197));
    println!(
        "the warp leaves the glitter alone: random states {:?} and {:?}",
        &run.rng[15581..=15583],
        INTRO_SKIP_RNG
    );
}

/// Plays the earlier route (`text`) to the original run's end: the
/// leaping pirate keeps its leap across the respawn.
fn across_the_respawn(level: &SimLevel, text: &str) {
    let mut run = Run::new(level, text);
    run.to(17047);
    let lives = run.sim.stats().lives;
    run.check_rng(&EARLIER_RNG);
    run.to(17048);
    assert!(
        run.sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::ClawDied { .. })),
        "Claw dies in tick 17048"
    );
    // The pirate called in at tick 17020 is in its leap, and in his list.
    let leaping: Vec<ObjectId> = run
        .of_logic(redtail_pirate::NAMES[0])
        .into_iter()
        .filter(|&p| {
            let o = run.sim.objects().get(p).unwrap();
            o.state == EnemyState::Jump as i32 && o.x > 43000 && o.y < 5000
        })
        .collect();
    assert_eq!(leaping.len(), 1, "one pirate of his in its leap");
    let p = leaping[0];
    assert!(run.sim.with_world(|w| redtail_pirate::called_in(w, p)));
    assert!(run.his_pirates().contains(&p));
    // The respawn's reset clears his list; the pirate makes no roll.
    run.to(17049);
    assert!(run.his_pirates().is_empty(), "his list is cleared");
    assert_eq!(run.claw_at(), ROOM, "back at the room's checkpoint");
    assert_eq!(run.sim.stats().lives, lives - 1);
    run.to(EARLIER_RUN_TICKS - 1);
    run.check_rng(&EARLIER_RNG);
    assert_eq!(run.claw_at(), (43105, 5212));
    assert_eq!(run.sim.stats().health, 80);
    assert_eq!(run.sim.stats().lives, lives - 1, "one life lost");
    assert!(run.sim.boss().defeated().is_none(), "Gabriel undefeated");
}

#[test]
fn install_a_called_in_pirate_keeps_its_leap_across_the_respawn() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    across_the_respawn(&level, &routes_file("earlier/level8-original.csv"));
    across_the_respawn(&level, &earlier_in_game());
    println!(
        "a called-in pirate keeps its leap across Gabriel's reset: the stream is the original's to tick {}",
        EARLIER_RUN_TICKS - 1
    );
}

#[test]
fn install_a_riding_enemy_moves_by_the_platform_only() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    let mut run = Run::new(&level, &routes_file("level8-introskip.csv"));
    run.to(15839);
    let (g, _) = run.gabriel();
    assert_eq!((g.x, g.y), ON_THE_PEG, "put back above the peg");
    assert!(g.flags & flags::ON_ELEVATOR != 0, "marked as riding");
    let peg = g.object_below.expect("the peg under him");
    assert!(
        run.sim.objects().get(peg).unwrap().logic.contains("Peg"),
        "riding a peg"
    );
    // The first bounce step drops his own move: he stays on the pixel he
    // landed on, his speed takes its gravity step, and the mark ends.
    run.to(15840);
    let (g, vy) = run.gabriel();
    assert_eq!((g.x, g.y), ON_THE_PEG, "the first bounce step");
    assert!((vy + 259.11).abs() < 0.01, "speed up {vy}");
    assert!(g.flags & flags::ON_ELEVATOR == 0, "the mark ends");
    // He lands on the peg again 1 px above its top, and his second
    // bounce, which starts from there, lands him on it at once.
    run.to(15854);
    assert_eq!(run.gabriel().0.physics_type, ptype::FLY);
    run.to(15855);
    let (g, _) = run.gabriel();
    assert_eq!((g.x, g.y, g.object_below), (42857, 5064, Some(peg)));
    run.to(15856);
    let (g, vy) = run.gabriel();
    assert_eq!(g.state, EnemyState::Bounce as i32, "the second bounce");
    assert_eq!((g.x, g.y), (42857, 5064), "the second bounce");
    assert_eq!(g.physics_type, ptype::STAND, "lands at once");
    assert_eq!(g.object_below, Some(peg));
    assert!((vy + 55.22).abs() < 0.01, "speed up {vy}");
    run.to(15885);
    assert_eq!(run.gabriel().0.health, 100);
    run.to(15886);
    assert_eq!(run.gabriel().0.health, 96, "the landing's cost");
    run.to(16562);
    run.check_rng(&BOUNCE_RNG);
    assert!(
        run.sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
        "the level's end"
    );
    println!(
        "a riding enemy moves by the platform only: Gabriel stays at {ON_THE_PEG:?} in tick 15840 and pays the landing in tick 15886"
    );
}
