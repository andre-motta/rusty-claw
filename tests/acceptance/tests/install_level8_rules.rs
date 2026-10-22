//! Three rules of the eighth level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of a route where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's.
//!
//! The routes are `routes/earlier/level8-original.csv`, the eighth level's
//! original-speeds route as it was before the first three rules, played
//! from the level's start and as the second level of a game (with the
//! progress `routes/level7-original.level8.csv` starts with),
//! `routes/earlier/level8-original-2.csv`, the same route as it was
//! before the last four, played the same two ways, and
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
//!   game, which the route then follows to the level's end;
//! - walking off a ledge with an enemy in his hands, Claw drops it, as a
//!   hit or the hold limit does, and it keeps his walk's speed across:
//!   Claw walks off Gabriel's deck carrying him in tick 16549 of the
//!   second route; Gabriel falls left at 225 px/s, lands on the dock in
//!   tick 16577 without a bounce or a landing's cost, and gets up a
//!   second after the drop, in tick 16605, where he landed;
//! - a pirate Gabriel calls in leaps from the height it was made at when
//!   its leap comes at its first call: with Gabriel down on the dock, the
//!   pirate he makes in tick 16899 is past the end of its run already, 17
//!   px above the pixel its building set on his deck, and it leaps from
//!   there in tick 16900;
//! - Gabriel's bombs are always active: Claw falls to his death below the
//!   dock in tick 17073 while three of them fly, and they fly their whole
//!   arcs, exploding in ticks 17104, 17110 and 17115;
//! - Claw's respawn resets the current boss before its update passes: in
//!   tick 17242 Gabriel starts his fight over and his pirate, listed
//!   before him, is marked for removal before any object runs, so the
//!   pirate makes its roll in the first pass only. The route then ends
//!   without the level's end, as it did in the original game, and the
//!   stream stays the original's to the original run's last tick, 112
//!   ticks past the route's last row, with nothing held.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::enemy::gabriel::bomb;
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

/// The original game's random state after these ticks of the second
/// earlier route, from the level's start or as the game's second level.
const SECOND_RNG: [(usize, u32); 22] = [
    (16548, 3_431_305_229),
    (16549, 3_305_136_796),
    (16577, 1_400_284_286),
    (16605, 852_194_993),
    (16606, 3_613_277_872),
    (16899, 1_353_536_739),
    (16900, 1_353_536_739),
    (16960, 894_291_960),
    (16961, 2_070_089_435),
    (17070, 1_927_534_570),
    (17073, 3_792_741_106),
    (17103, 3_781_115_327),
    (17104, 2_593_804_849),
    (17110, 255_971_122),
    (17115, 4_239_181_700),
    (17241, 645_825_042),
    (17242, 3_374_936_348),
    (17243, 4_194_941_807),
    (17300, 3_169_484_595),
    (17488, 2_636_022_607),
    (17550, 480_159_316),
    (17600, 3_810_136_343),
];

/// Ticks the original run of the second earlier route lasted.
const SECOND_RUN_TICKS: usize = 17601;

/// Where Gabriel lands on the dock after Claw walks off his deck with
/// him, and where he gets up.
const DROPPED_AT: (i32, i32) = (43032, 5257);
const UP_AT: (i32, i32) = (43032, 5238);

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
    in_game("earlier/level8-original.csv")
}

/// Route `name` as the game's second level: its rows, with the progress
/// the game's eighth level starts with.
fn in_game(name: &str) -> String {
    let rows = routes_file(name);
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

    /// Gabriel's object id.
    fn gabriel_id(&self) -> ObjectId {
        self.sim
            .objects()
            .all()
            .find(|(_, o)| matches!(o.data, ObjectData::Gabriel(_)))
            .map(|(id, _)| id)
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

/// The second earlier route (`text`) to tick 16548: Claw on Gabriel's deck
/// with him in his hands.
fn at_the_deck_edge(level: &SimLevel, text: &str) -> (Run, ObjectId) {
    let mut run = Run::new(level, text);
    run.to(16548);
    let g = run.gabriel_id();
    assert_eq!(run.sim.claw().special.lifted, Some(g), "carrying Gabriel");
    (run, g)
}

fn state_of(run: &Run, id: ObjectId) -> Option<EnemyState> {
    run.sim
        .objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

#[test]
fn install_a_ledge_walk_drops_what_claw_carries() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    for text in [
        routes_file("earlier/level8-original-2.csv"),
        in_game("earlier/level8-original-2.csv"),
    ] {
        let (mut run, g) = at_the_deck_edge(&level, &text);
        // He walks off the deck's edge with Left held: Gabriel is dropped,
        // with the walk's speed across.
        run.to(16549);
        assert_eq!(run.sim.claw().special.lifted, None, "let go");
        assert_eq!(state_of(&run, g), Some(EnemyState::Dropped));
        let (o, _) = run.gabriel();
        assert_eq!(o.physics_type, ptype::FLY);
        let vx = match &o.data {
            ObjectData::Gabriel(d) => d.enemy.vx,
            _ => unreachable!(),
        };
        assert_eq!(vx, -225.0, "the walk's speed across");
        // He lands on the dock without a bounce and gets up a second
        // after the drop where he landed, unhurt.
        run.to(16576);
        assert_eq!(run.gabriel().0.physics_type, ptype::FLY);
        run.to(16577);
        let (o, _) = run.gabriel();
        assert_eq!((o.x, o.y), DROPPED_AT, "on the dock");
        assert_eq!(o.physics_type, ptype::STAND);
        run.to(16604);
        assert_eq!(state_of(&run, g), Some(EnemyState::Dropped), "no bounce");
        assert_eq!(run.gabriel().0.health, 100, "no landing's cost");
        run.to(16606);
        let (o, _) = run.gabriel();
        assert_eq!((o.x, o.y), UP_AT, "up where he landed");
        assert_ne!(state_of(&run, g), Some(EnemyState::Dropped));
        run.check_rng(&SECOND_RNG);
    }
    println!(
        "a ledge walk drops Gabriel: he lands at {DROPPED_AT:?} in tick 16577 and gets up at {UP_AT:?}"
    );
}

#[test]
fn install_a_pirate_leaping_at_its_first_call_leaves_from_where_it_was_made() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    let mut run = Run::new(&level, &routes_file("earlier/level8-original-2.csv"));
    run.to(16898);
    assert!(run.his_pirates().is_empty());
    run.to(16899);
    let pirates = run.his_pirates();
    assert_eq!(pirates.len(), 1, "he calls a pirate in");
    let p = pirates[0];
    let o = run.sim.objects().get(p).unwrap();
    assert_eq!((o.x, o.y), (43182, 5211), "set on his deck");
    let carry = claw_sim::logic::enemy::data_of(o).unwrap().carry;
    assert_eq!(carry, (0.0, 17.0), "made 17 px lower");
    assert_eq!(state_of(&run, p), Some(EnemyState::GabrielAttack));
    // Past the end of its run already: it leaps at its first call, from
    // the height it was made at.
    run.to(16900);
    assert_eq!(state_of(&run, p), Some(EnemyState::Jump));
    let o = run.sim.objects().get(p).unwrap();
    assert_eq!((o.x, o.y), (43182, 5205));
    // It lands on the deck in tick 16960 and makes its draws in 16962.
    run.to(16959);
    assert_eq!(state_of(&run, p), Some(EnemyState::Jump));
    run.to(16962);
    assert_eq!(run.sim.objects().get(p).unwrap().y, 5038);
    run.check_rng(&SECOND_RNG);
    println!("a pirate leaping at its first call leaves from 5228.0: y 5205 after tick 16900");
}

#[test]
fn install_gabriels_bombs_fly_on_while_claw_lies_dead() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    let mut run = Run::new(&level, &routes_file("earlier/level8-original-2.csv"));
    run.to(17070);
    let bombs = run.of_logic("GabrielBomb");
    assert_eq!(bombs.len(), 3, "he lobs three bombs");
    for &b in &bombs {
        let o = run.sim.objects().get(b).unwrap();
        assert!(o.flags & flags::ALWAYS_ACTIVE != 0, "always active");
    }
    run.to(17073);
    assert_eq!(run.claw_at(), (42295, 5713), "Claw dead below the dock");
    // They fly their whole arcs and explode one after the other.
    let exploding = |run: &Run| {
        bombs
            .iter()
            .filter(|&&b| {
                run.sim
                    .objects()
                    .get(b)
                    .is_some_and(|o| o.state == bomb::state::EXPLODING)
            })
            .count()
    };
    run.to(17103);
    assert_eq!(exploding(&run), 0);
    assert!(bombs.iter().all(|&b| run.sim.objects().get(b).is_some()));
    for (tick, n) in [(17104, 1), (17110, 2), (17115, 3)] {
        run.to(tick - 1);
        assert_eq!(exploding(&run), n - 1, "before tick {tick}");
        run.to(tick);
        assert_eq!(exploding(&run), n, "in tick {tick}");
    }
    run.check_rng(&SECOND_RNG);
    println!("Gabriel's bombs fly on and explode in ticks 17104, 17110 and 17115");
}

/// Plays the second earlier route (`text`) to the original run's end:
/// the respawn resets Gabriel before its passes.
fn the_respawn_resets_him_first(level: &SimLevel, text: &str) {
    let mut run = Run::new(level, text);
    run.to(17072);
    let lives = run.sim.stats().lives;
    run.to(17241);
    let pirates = run.his_pirates();
    assert_eq!(pirates.len(), 1, "one pirate of his");
    let p = pirates[0];
    let g = run.gabriel_id();
    let listed = run.sim.objects().listed();
    let at = |id| listed.iter().position(|&x| x == id).unwrap();
    assert!(at(p) < at(g), "the pirate is listed before him");
    run.to(17242);
    assert_eq!(run.claw_at(), ROOM, "back at the room's checkpoint");
    assert!(run.sim.objects().get(p).is_none(), "the pirate is gone");
    assert!(run.his_pirates().is_empty());
    assert_eq!(run.gabriel().0.health, 100);
    run.to(SECOND_RUN_TICKS - 1);
    run.check_rng(&SECOND_RNG);
    assert_eq!(run.claw_at(), (43133, 5212));
    assert_eq!(run.sim.stats().health, 100);
    assert_eq!(run.sim.stats().lives, lives - 1, "one life lost");
    assert!(run.sim.boss().defeated().is_none(), "Gabriel undefeated");
}

#[test]
fn install_the_respawn_resets_the_boss_before_its_passes() {
    let Some(root) = install() else { return };
    let level = level8(&root);
    the_respawn_resets_him_first(&level, &routes_file("earlier/level8-original-2.csv"));
    the_respawn_resets_him_first(&level, &in_game("earlier/level8-original-2.csv"));
    println!(
        "the respawn resets Gabriel first: the stream is the original's to tick {}",
        SECOND_RUN_TICKS - 1
    );
}
