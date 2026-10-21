//! Four rules of the seventh level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of an earlier route
//! where the original game showed it.
//!
//! The routes are the seventh level's original-speeds routes as they were
//! before these rules: `routes/earlier/level7-original-2.csv` (the level
//! from its start) and `routes/earlier/level6-original-2.level7.csv` (the
//! level as the second level of a game), and `routes/earlier/level7-original.csv`,
//! the first route of the level. Each is played with the hermit crabs'
//! values that a run of the original game gave them (see
//! [`CrabValues`]):
//!
//! - a hermit crab never lands on a platform object: the third crab
//!   thrown out of the nest of record 284 falls past the crumbling peg of
//!   record 263 at tick 6290 and lands on the sand below at tick 6316;
//!   landing on the peg, as other enemies would, it regroups at once and
//!   draws two numbers the original game does not draw there;
//! - on a level whose death tiles are a liquid, Claw dying on one is not
//!   drawn and a splash shows 60 px above him, just in front of him: one
//!   object more in the update pass, so a glitter made later in that pass
//!   just behind it runs in the same pass and draws its start delay;
//! - a bouncing goodie whose step meets a ceiling turns only its speed up
//!   and down, even when the step also meets a wall: the health potion a
//!   pirate drops keeps flying left and settles 65 px further left than it
//!   would after turning at the wall;
//! - a human enemy walking at Claw goes idle at its patrol limit only when
//!   its exact position is on the limit: the bear sailor of record 1348
//!   reaches the whole pixel of its limit with 0.76 px left over and walks
//!   one tick more.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::crab::{self, CrabKey, CrabValues};
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::pickup;
use claw_sim::object::{ObjectId, draw_flags, flags};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::{InputScript, Sim, SimLevel, StartState};

/// The crabs' values of a run of `earlier/level7-original-2.csv` in the
/// original game in which the crab of record 101 has a box that covers
/// its beach.
const DIRECT_COVERING: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 2, 300, 40
crab 101 = 1294344774, 857312352, 784380376
crab 1647 = 2, 300, 40
nest_crab 185 1 = 3, 200, 40
nest_crab 184 * = 3, 0, 0
nest_crab 185 2 = 3, 0, 0
nest_crab 185 3 = 3, 0, 0
nest_crab 286 * = 3, 0, 0
nest_crab 1306 * = 3, 0, 0
";

/// The crabs' values of a run of the level's part of
/// `earlier/level6-original-2.level7.csv` in the original game.
const PAIR: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 3, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 270720, 52951952, 52951984
nest_crab 165 1 = 3, 200, 40
nest_crab 165 2 = 3, 0, 0
nest_crab 165 3 = 3, 0, 0
nest_crab 185 1 = 2, 200, 40
nest_crab 185 2 = 3, 0, 0
nest_crab 185 3 = 3, 0, 0
nest_crab 284 1 = 3, 0, 0
nest_crab 285 1 = 3, 0, 0
nest_crab 286 1 = 3, 0, 0
nest_crab 286 2 = 3, 0, 0
nest_crab 286 3 = 3, 200, 40
nest_crab 1304 1 = 302023104, 21955672, 0
nest_crab 1305 1 = 3, 21955672, 0
nest_crab 1305 2 = 2, 1380454151, -184661083
nest_crab 1306 1 = 3, 21955672, 0
";

/// The crabs' values of a run of `earlier/level7-original.csv` in the
/// original game.
const DIRECT_EMPTY: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 2, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 2, 300, 40
nest_crab 185 1 = 2, 0, 0
nest_crab 185 2 = 3, 0, 0
nest_crab 286 1 = 3, 0, 0
nest_crab 286 2 = 3, 0, 0
nest_crab 1304 1 = 302023104, 21955672, 0
nest_crab 1305 1 = 2, -2006994985, 1514611473
nest_crab 1305 2 = 3, 0, 0
nest_crab 1305 3 = 3, 0, 0
nest_crab 1351 1 = 3, 0, 0
";

fn level7(root: &Path) -> SimLevel {
    let store = Install::open(root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(7)).unwrap();
    SimLevel::from_level(&data, &store).unwrap()
}

fn earlier(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("routes/earlier")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A route of the seventh level played tick by tick with the crabs
/// starting with `crabs`, from the progress the route starts with.
struct Run {
    sim: Sim,
    script: InputScript,
    crabs: CrabValues,
    /// The next tick to play.
    tick: usize,
    /// The random state after each tick played.
    rng: Vec<u32>,
}

impl Run {
    fn new(level: &SimLevel, text: &str, crabs: &str) -> Run {
        let script = InputScript::parse(text).unwrap();
        let seed = script.header.seed.expect("the route names its seed");
        let cfg = route::sim_config(TuningName::Original, seed);
        let mut sim = match route::parse_carry(text).unwrap() {
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
        let crabs = CrabValues::parse(crabs).unwrap();
        crabs.apply(&mut sim);
        Run {
            sim,
            script,
            crabs,
            tick: 0,
            rng: Vec::new(),
        }
    }

    /// Plays ticks until `last` has been played.
    fn to(&mut self, last: usize) {
        while self.tick <= last {
            self.sim.step(self.script.ticks[self.tick]);
            self.crabs.apply(&mut self.sim);
            self.rng.push(self.sim.rng_state());
            self.tick += 1;
        }
    }

    /// The object of `logic` with creation number `serial`.
    fn find(&self, logic: &str, serial: u32) -> Option<ObjectId> {
        self.sim
            .objects()
            .all()
            .find(|(_, o)| &*o.logic == logic && o.serial == serial)
            .map(|(id, _)| id)
    }

    /// The crab of `key`.
    fn crab(&self, key: CrabKey) -> Option<ObjectId> {
        self.sim
            .objects()
            .all()
            .map(|(id, _)| id)
            .find(|&id| crab::which_crab(self.sim.objects(), id) == Some(key))
    }

    fn at(&self, id: ObjectId) -> (i32, i32) {
        let o = self.sim.objects().get(id).expect("the object");
        (o.x, o.y)
    }
}

/// Every crab of the game lands on platform objects too, as other enemies
/// do.
fn crabs_land_on_platforms(sim: &mut Sim) {
    sim.with_world(|w| {
        let ids: Vec<ObjectId> = w
            .objects
            .all()
            .filter(|(_, o)| &*o.logic == "HermitCrab")
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if let Some(d) = w.objects.get_mut(id).and_then(|o| o.data.enemy_mut()) {
                d.tiles_only = false;
            }
        }
    });
}

/// The third crab of the nest of record 284.
const NEST_284_THIRD: CrabKey = CrabKey::Hatched(Some(284), Some(3));

#[test]
fn install_a_crab_falls_past_a_crumbling_peg() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level7-original-2.csv");
    let mut run = Run::new(&level, &text, DIRECT_COVERING);
    run.to(6257);
    let mut plat = Run::new(&level, &text, DIRECT_COVERING);
    plat.to(6257);
    assert_eq!(run.rng, plat.rng);
    let peg = run
        .find("CrumblingPeg", 263)
        .expect("the peg of record 263");
    // Thrown out of the nest at tick 6258, it falls by the peg.
    let mut path = Vec::new();
    while run.tick <= 6320 {
        run.to(run.tick);
        crabs_land_on_platforms(&mut plat.sim);
        plat.to(plat.tick);
        let t = run.tick - 1;
        let Some(c) = run.crab(NEST_284_THIRD) else {
            continue;
        };
        let o = run.sim.objects().get(c).unwrap();
        path.push((
            t,
            o.x,
            o.y,
            o.physics_type,
            o.flags & flags::ON_ELEVATOR != 0,
        ));
    }
    let first = path.first().expect("the crab is hatched").0;
    assert_eq!(first, 6258, "hatched at tick 6258");
    assert!(path.iter().all(|p| !p.4), "it never rides a platform");
    // Still falling at tick 6292, it lands on the sand at tick 6316 and
    // goes home on the next.
    let landed = path.iter().find(|p| p.3 == ptype::STAND).expect("it lands");
    assert_eq!(landed.0, 6316, "{path:?}");
    assert_eq!((landed.1, landed.2), (20134, 6784));
    // Two draws fewer at tick 6292 than a landing on the peg gives.
    assert_eq!(run.rng[6292], 2_100_543_699);
    assert_eq!(plat.rng[6291], run.rng[6291]);
    assert_eq!(plat.rng[6292], 2_785_218_021);
    let on_peg = plat.crab(NEST_284_THIRD).unwrap();
    let (px, py) = plat.at(on_peg);
    let p = run.sim.objects().get(peg).unwrap();
    println!(
        "the third crab of nest 284 falls past the peg at ({}, {}) and lands at ({}, {}) at tick {}; landing on platforms it stands at ({px}, {py})",
        p.x, p.y, landed.1, landed.2, landed.0
    );
}

#[test]
fn install_claw_disappears_into_the_goo_with_a_splash() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level7-original-2.csv");
    let mut run = Run::new(&level, &text, DIRECT_COVERING);
    run.to(6928);
    assert_ne!(run.sim.player().unwrap().state, state::DEATH);
    run.to(6929);
    let claw = run.sim.player().unwrap().clone();
    assert_eq!(claw.state, state::DEATH, "dead in the goo at tick 6929");
    assert_eq!((claw.x, claw.y), (20646, 7698));
    assert!(claw.draw_flags & draw_flags::HIDDEN != 0, "not drawn");
    let splashes: Vec<(i32, i32, i32)> = run
        .sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "Splash")
        .map(|(_, o)| (o.x, o.y, o.z))
        .collect();
    assert_eq!(splashes, vec![(20646, 7638, claw.z + 2)]);
    // Without the splash, the glitter of the goodie that settles at tick
    // 6934 runs a pass later and draws its start delay later.
    let mut without = Run {
        sim: run.sim.clone(),
        script: run.script.clone(),
        crabs: run.crabs.clone(),
        tick: run.tick,
        rng: run.rng.clone(),
    };
    without.sim.with_world(|w| {
        let ids: Vec<ObjectId> = w
            .objects
            .all()
            .filter(|(_, o)| &*o.logic == "Splash")
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            w.objects.remove(id);
        }
    });
    run.to(6934);
    without.to(6934);
    assert_eq!(run.rng[6933], without.rng[6933]);
    assert_eq!(run.rng[6934], 1_658_831_255);
    assert_eq!(without.rng[6934], 3_311_346_788);
    println!(
        "Claw dies in the goo at (20646, 7698) at tick 6929, hidden, with a splash at (20646, 7638)"
    );
}

#[test]
fn install_a_potion_that_meets_a_ceiling_keeps_flying_left() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level6-original-2.level7.csv");
    let mut run = Run::new(&level, &text, PAIR);
    run.to(4822);
    let before: Vec<ObjectId> = run
        .sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == pickup::BOUNCING_GOODIE)
        .map(|(id, _)| id)
        .collect();
    run.to(4823);
    let goodie = |sim: &Sim, id: ObjectId| match sim.objects().get(id).map(|o| &o.data) {
        Some(ObjectData::Pickup(d)) => d.goodie,
        _ => None,
    };
    // The pirate's loot leaves at tick 4823; the potion flies left at 25
    // px/s and up at 493.
    let potion = run
        .sim
        .objects()
        .all()
        .filter(|(id, o)| &*o.logic == pickup::BOUNCING_GOODIE && !before.contains(id))
        .map(|(id, _)| id)
        .find(|&id| goodie(&run.sim, id).is_some_and(|g| g.vx == -25.0))
        .expect("the potion");
    let g = goodie(&run.sim, potion).unwrap();
    assert_eq!(run.at(potion), (15705, 7386), "it leaves from the pirate");
    assert!((g.vy + 493.0).abs() < 1.0, "{}", g.vy);
    run.to(4842);
    assert!(goodie(&run.sim, potion).unwrap().vy < 0.0, "rising");
    run.to(4843);
    assert_eq!(run.at(potion), (15696, 7262), "at the ceiling at tick 4843");
    let g = goodie(&run.sim, potion).unwrap();
    assert!(g.vy > 0.0, "turned down: {}", g.vy);
    assert_eq!(g.vx, -25.0, "it keeps flying left");
    // It settles 65 px left of where turning at the wall would put it.
    let mut settled = None;
    while run.tick <= 4977 {
        run.to(run.tick);
        let o = run.sim.objects().get(potion);
        if settled.is_none()
            && let Some(o) = o
            && &*o.logic != pickup::BOUNCING_GOODIE
        {
            settled = Some((run.tick - 1, o.x, o.y, o.logic.to_string()));
        }
    }
    let (t, x, y, logic) = settled.expect("the potion settles");
    assert_eq!((x, y), (15652, 7429), "settled at tick {t} as {logic}");
    assert_eq!(logic, "HealthPowerup");
    assert_eq!(run.rng[4977], 672_245_287);
    println!(
        "the potion reaches the ceiling at (15696, 7262) at tick 4843, keeps 25 px/s left and settles at ({x}, {y}) at tick {t}"
    );
}

#[test]
fn install_a_bear_walks_on_a_fraction_short_of_its_limit() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level7-original.csv");
    let mut run = Run::new(&level, &text, DIRECT_EMPTY);
    run.to(6807);
    let bear = run
        .find("BearSailor", 1348)
        .expect("the bear of record 1348");
    let state = |run: &Run| {
        let o = run.sim.objects().get(bear).unwrap();
        EnemyState::from_id(o.state)
    };
    let (x, _) = run.at(bear);
    let limit = enemy::data_of(run.sim.objects().get(bear).unwrap())
        .unwrap()
        .bounds
        .0;
    assert_eq!((x, limit), (21719, 21719), "on its limit's whole pixel");
    let exact = run.sim.with_world(|w| enemy::exact_x(w, bear));
    assert!((exact - 21719.76).abs() < 0.005, "{exact}");
    assert_eq!(state(&run), Some(EnemyState::Advance));
    run.to(6808);
    assert_eq!(state(&run), Some(EnemyState::Advance), "one tick more");
    assert_eq!(run.rng[6808], 95_899_545);
    run.to(6809);
    assert_eq!(state(&run), Some(EnemyState::Idle), "idle on the limit");
    println!(
        "the bear of record 1348 reaches x {x} with {:.2} px left at tick 6807 and goes idle at tick 6809",
        exact - f64::from(x)
    );
}
