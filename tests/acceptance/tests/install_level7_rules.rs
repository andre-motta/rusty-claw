//! Eight rules of the seventh level's play and one value of its crabs on
//! your own install (skipped without `CLAW_INSTALL`), each pinned to a
//! moment of an earlier route where the original game showed it.
//!
//! The routes are the seventh level's original-speeds routes as they were
//! before these rules: `routes/earlier/level7-original-2.csv` (the level
//! from its start) and `routes/earlier/level6-original-2.level7.csv` (the
//! level as the second level of a game), `routes/earlier/level7-original.csv`,
//! the first route of the level, and `routes/earlier/level7-original-3.csv`
//! and `routes/earlier/level6-original-3.level7.csv`, the two routes after
//! the first four rules, and `routes/earlier/level7-original-4.csv` and
//! `routes/earlier/level6-original-4.level7.csv`, the two routes after the
//! first seven. Each is played with the hermit crabs' values that
//! a run of the original game gave them (see [`CrabValues`]):
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
//!   one tick more;
//! - a crab a nest hatched can start stationary, with no ledge guard: the
//!   third crab of the nest of record 1306 lands at tick 3661 with its
//!   left edge over a drop and stays there, 2 px left of where the guard
//!   puts a crab that is not stationary, so it leaves the active area a
//!   tick earlier and draws nothing at tick 3959;
//! - a body hit again keeps its place in the list: at tick 4833 the blow
//!   that kills the pirate of record 2399 also hits the body of the pirate
//!   of record 2400, which stays ahead of it, so the two pirates take
//!   their turns and their draws in that order;
//! - a new object takes the place among the level's objects that an
//!   object left last, and a crab knows its last attacker only by that
//!   place: the bomb the crab hatched second by the nest of record 185
//!   throws at tick 6047 takes the place of the bomb whose blast hit the
//!   crab at tick 6026, so its blast, which goes off on Claw at once, does
//!   not kill the crab;
//! - an object is on screen while any pixel of its picture is in the
//!   view, the picture's last row and column included: the pirate of
//!   record 2423, running at Claw on a picture 111 pixels high, has only
//!   its last row in view at tick 9538 and runs on one tick more before it
//!   goes idle (also from a direct start, at tick 9470);
//! - a human enemy's hop back aims 100 px from its exact position and is
//!   refused when that spot is past its patrol limit by any fraction: the
//!   pirate of record 2399, walking back at x 16002.1 with its limit at
//!   16102, stays on the floor when Claw swings at tick 4830, and the
//!   swing's blow hits it at tick 4838.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
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

/// The crabs' values of a run of `earlier/level7-original-3.csv` in the
/// original game in which the crab of record 101 has an empty box.
const DIRECT_EMPTY_3: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 2, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 2, 300, 40
nest_crab 164 * = 0, 0, 0
nest_crab 165 1 = 3, 200, 40
nest_crab 165 2 = 0, 0, 0
nest_crab 165 3 = 0, 0, 0
nest_crab 175 1 = 3, 0, 0
nest_crab 175 2 = 0, 0, 0
nest_crab 175 3 = 0, 0, 0
nest_crab 1304 * = 0, 0, 0
nest_crab 1305 1 = 3, 0, 0
nest_crab 1305 2 = 3, 0, 0
nest_crab 1305 3 = 0, 0, 0
nest_crab 1306 1 = 0, 0, 0
nest_crab 1306 2 = 0, 0, 0
";

/// The crabs' values of a run of the level's part of
/// `earlier/level6-original-3.level7.csv` in the original game.
const PAIR_3: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 3, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 2, 300, 40
nest_crab 164 * = 0, 0, 0
nest_crab 165 * = 3, 0, 0
nest_crab 165 1 = 3, 200, 40
nest_crab 175 * = 3, 0, 0
nest_crab 175 1 = 3, 200, 40
nest_crab 184 1 = 3, 0, 0
nest_crab 184 2 = 2, 200, 40
nest_crab 184 3 = 0, 0, 0
nest_crab 185 1 = 2, 200, 40
nest_crab 185 2 = 3, 0, 0
nest_crab 185 3 = 3, 0, 0
nest_crab 1304 1 = 302023104, 21955672, 0
nest_crab 1304 2 = 0, 0, 0
nest_crab 1304 3 = 0, 0, 0
nest_crab 1305 1 = 3, 0, 0
nest_crab 1305 2 = 0, 0, 0
nest_crab 1305 3 = 0, 0, 0
nest_crab 1306 * = 0, 0, 0
";

/// The third crab of the nest of record 1306.
const NEST_1306_THIRD: CrabKey = CrabKey::Hatched(Some(1306), Some(3));

#[test]
fn install_a_stationary_nest_crab_lands_over_a_drop_and_stays() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level7-original-3.csv");
    let still = format!("{DIRECT_EMPTY_3}nest_crab 1306 3 = 0, 0, 0, 1\n");
    let plain = format!("{DIRECT_EMPTY_3}nest_crab 1306 3 = 0, 0, 0\n");
    let mut run = Run::new(&level, &text, &still);
    let mut other = Run::new(&level, &text, &plain);
    run.to(3660);
    other.to(3660);
    assert_eq!(run.rng, other.rng);
    let c = run.crab(NEST_1306_THIRD).expect("hatched by tick 3660");
    let o = run.sim.objects().get(c).unwrap();
    let d = enemy::data_of(o).unwrap();
    assert!(d.stationary);
    assert_eq!(crab::fields_of(o).unwrap().strike_sound, crab::THROW_SOUND);
    assert_eq!(run.at(c), (13151, 7038), "falling");
    let n = other.crab(NEST_1306_THIRD).unwrap();
    assert!(
        !enemy::data_of(other.sim.objects().get(n).unwrap())
            .unwrap()
            .stationary
    );
    run.to(3661);
    other.to(3661);
    // It lands with its left edge over the drop and stays; the ledge
    // guard puts the other back 2 px.
    assert_eq!(run.at(c), (13149, 7040));
    assert_eq!(other.at(n), (13151, 7038));
    assert_eq!(run.rng[3661], other.rng[3661]);
    run.to(3959);
    other.to(3959);
    assert_eq!(run.rng[3958], other.rng[3958]);
    assert_eq!(run.rng[3959], 1_632_497_895);
    assert_eq!(other.rng[3959], 1_659_840_526);
    println!(
        "the stationary third crab of nest 1306 lands at (13149, 7040) at tick 3661 and stays; random state after tick 3959 {}",
        run.rng[3959]
    );
}

#[test]
fn install_a_body_hit_again_keeps_its_place() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level6-original-3.level7.csv");
    let mut run = Run::new(&level, &text, PAIR_3);
    run.to(4832);
    let a = run.find("RedTailPirate", 2399).expect("the pirate of 2399");
    let b = run.find("RedTailPirate", 2400).expect("the pirate of 2400");
    let dead = |run: &Run, id: ObjectId| {
        enemy::data_of(run.sim.objects().get(id).unwrap()).is_some_and(|d| d.dead)
    };
    let z = |run: &Run, id: ObjectId| run.sim.objects().get(id).unwrap().z;
    assert!(!dead(&run, a));
    assert!(dead(&run, b), "a body since earlier");
    assert_eq!(z(&run, b), enemy::DEATH_DEPTH);
    run.to(4833);
    assert!(dead(&run, a), "killed at tick 4833");
    assert_eq!(z(&run, a), enemy::DEATH_DEPTH);
    run.to(4834);
    let listed = run.sim.objects().listed();
    let place = |id| listed.iter().position(|&l| l == id).unwrap();
    assert_eq!(place(b) + 1, place(a), "the body hit again stays ahead");
    run.to(4839);
    assert_eq!(run.rng[4839], 1_263_798_278);
    println!(
        "the body of the pirate of record 2400 stays ahead of the pirate of record 2399; random state after tick 4839 {}",
        run.rng[4839]
    );
}

#[test]
fn install_a_bomb_in_the_place_of_the_last_attacker_spares_its_crab() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level6-original-3.level7.csv");
    let mut run = Run::new(&level, &text, PAIR_3);
    let key = CrabKey::Hatched(Some(185), Some(2));
    let bombs = |run: &Run| -> Vec<ObjectId> {
        run.sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == "CrabBomb")
            .map(|(id, _)| id)
            .collect()
    };
    let last_attacker = |run: &Run| {
        let c = run.crab(key).expect("the crab");
        crab::fields_of(run.sim.objects().get(c).unwrap())
            .unwrap()
            .last_attacker
    };
    run.to(6046);
    let hit_by = last_attacker(&run).expect("hit by a blast at tick 6026");
    assert!(run.sim.objects().get(hit_by).is_none(), "that bomb is gone");
    run.to(6047);
    let thrown: Vec<ObjectId> = bombs(&run)
        .into_iter()
        .filter(|&b| claw_sim::container::same_slot(b, hit_by))
        .collect();
    assert_eq!(thrown.len(), 1, "the new bomb takes the old bomb's place");
    assert_ne!(thrown[0], hit_by);
    let c = run.crab(key).unwrap();
    assert!(
        !enemy::data_of(run.sim.objects().get(c).unwrap())
            .unwrap()
            .dead,
        "the crab lives on"
    );
    assert_eq!(run.rng[6047], 2_927_231_927);
    // On through the original's run, to its game over at tick 7751.
    for (t, rng) in [(6500, 3_387_032_359), (7200, 2_290_093_832)] {
        run.to(t);
        assert_eq!(run.rng[t], rng, "tick {t}");
    }
    let mut over = None;
    while over.is_none() && run.tick < run.script.ticks.len() {
        run.to(run.tick);
        if run
            .sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::GameOver))
        {
            over = Some(run.tick - 1);
        }
    }
    assert_eq!(over, Some(7751), "the game over");
    assert_eq!(run.rng[7751], 1_775_566_211);
    println!(
        "the bomb thrown at tick 6047 takes the place of the crab's last attacker and spares it; random state after tick 6047 {}",
        run.rng[6047]
    );
}
/// The crabs' values of a run of the level's part of
/// `earlier/level6-original-4.level7.csv` in the original game in which
/// the crab of record 1647 has a box of 300 by 40 and the third crab of
/// the nest of record 175 starts stationary.
const PAIR_4_SMALL: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 3, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 2, 300, 40
nest_crab 164 1 = 0, 0, 0
nest_crab 164 2 = 0, 0, 0
nest_crab 164 3 = 0, 0, 0
nest_crab 165 1 = 3, 200, 40
nest_crab 165 2 = 0, 0, 0
nest_crab 165 3 = 0, 0, 0
nest_crab 175 1 = 3, 200, 40
nest_crab 175 2 = 3, 0, 0
nest_crab 175 3 = 0, 0, 0, 1
nest_crab 184 1 = 0, 0, 0
nest_crab 184 2 = 0, 0, 0
nest_crab 184 3 = 0, 0, 0
nest_crab 185 1 = 3, 200, 40
nest_crab 185 2 = 3, 200, 40
nest_crab 185 3 = 3, 0, 0
nest_crab 284 1 = 0, 0, 0
nest_crab 284 2 = 0, 0, 0
nest_crab 284 3 = 0, 0, 0
nest_crab 285 1 = 0, 0, 0
nest_crab 285 2 = 0, 0, 0
nest_crab 285 3 = 0, 0, 0
nest_crab 286 1 = 3, 300, 40
nest_crab 286 2 = 3, 0, 0
nest_crab 286 3 = 3, 0, 0
nest_crab 1304 1 = 302023104, 21955672, 0
nest_crab 1304 2 = 0, 0, 0
nest_crab 1304 3 = 0, 0, 0
nest_crab 1305 1 = 3, 21955672, 0
nest_crab 1305 2 = 3, 0, 0
nest_crab 1305 3 = 0, 0, 0
nest_crab 1306 1 = 3, 21955672, 0
nest_crab 1306 2 = 0, 0, 0
nest_crab 1306 3 = 0, 0, 0
nest_crab 1351 1 = 0, 0, 0
nest_crab 1351 2 = 0, 0, 0
nest_crab 1351 3 = 0, 0, 0
nest_crab 1352 1 = 3, 0, 0
nest_crab 1352 2 = 3, 0, 0
nest_crab 1352 3 = 0, 0, 0
";

/// The crabs' values of a run of `earlier/level7-original-4.csv` in the
/// original game in which the crab of record 101 has an empty box.
const DIRECT_4_EMPTY: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 2, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = 2, 300, 40
nest_crab 164 1 = 0, 0, 0
nest_crab 164 2 = 0, 0, 0
nest_crab 164 3 = 0, 0, 0
nest_crab 165 1 = 3, 200, 40, 1
nest_crab 165 2 = 0, 0, 0
nest_crab 165 3 = 0, 0, 0
nest_crab 184 1 = 3, 0, 0
nest_crab 184 2 = 0, 0, 0
nest_crab 184 3 = 0, 0, 0
nest_crab 185 1 = 3, 200, 40
nest_crab 185 2 = 3, 0, 0
nest_crab 185 3 = 3, 0, 0
nest_crab 284 1 = 3, 0, 0
nest_crab 284 2 = 0, 0, 0
nest_crab 284 3 = 0, 0, 0
nest_crab 285 1 = 3, 0, 0
nest_crab 285 2 = 0, 0, 0
nest_crab 285 3 = 0, 0, 0
nest_crab 286 1 = 3, 0, 0, 1
nest_crab 286 2 = 3, 0, 0
nest_crab 286 3 = 3, 200, 40
nest_crab 1304 1 = 0, 0, 0
nest_crab 1304 2 = 0, 0, 0
nest_crab 1304 3 = 0, 0, 0
nest_crab 1305 1 = 3, 0, 0
nest_crab 1305 2 = 3, 0, 0
nest_crab 1305 3 = 0, 0, 0
nest_crab 1306 1 = 0, 0, 0
nest_crab 1306 2 = 0, 0, 0
nest_crab 1306 3 = 0, 0, 0, 1
nest_crab 1351 1 = 3, 0, 0
nest_crab 1351 2 = 0, 0, 0
nest_crab 1351 3 = 0, 0, 0
nest_crab 1352 1 = 3, 0, 0
nest_crab 1352 2 = 3, 0, 0
nest_crab 1352 3 = 3, 0, 0
";

/// The crabs' values of a run of the level's part of
/// `earlier/level6-original-4.level7.csv` in the original game in which
/// the crab of record 1647 has a box that covers Claw; no crab starts
/// stationary.
const PAIR_4_COVERING: &str = "\
crab 85 = 1721092825, 857312352, -117703298
crab 86 = 3, 300, 40
crab 101 = 2058048938, -2006994985, 1514611473
crab 1647 = -1051409578, 14418135, 1556241857
nest_crab 164 1 = 0, 0, 0
nest_crab 164 2 = 0, 0, 0
nest_crab 164 3 = 0, 0, 0
nest_crab 165 1 = 3, 200, 40
nest_crab 165 2 = 0, 0, 0
nest_crab 165 3 = 0, 0, 0
nest_crab 175 1 = 3, 200, 40
nest_crab 175 2 = 0, 0, 0
nest_crab 175 3 = 0, 0, 0
nest_crab 184 1 = 0, 0, 0
nest_crab 184 2 = 0, 0, 0
nest_crab 184 3 = 0, 0, 0
nest_crab 185 1 = 3, 200, 40
nest_crab 185 2 = 3, 200, 40
nest_crab 185 3 = 3, 0, 0
nest_crab 284 1 = 0, 0, 0
nest_crab 284 2 = 0, 0, 0
nest_crab 284 3 = 0, 0, 0
nest_crab 285 1 = 0, 0, 0
nest_crab 285 2 = 0, 0, 0
nest_crab 285 3 = 0, 0, 0
nest_crab 286 1 = 3, 200, 40
nest_crab 286 2 = 1105199103, -1, 1
nest_crab 286 3 = 3, 0, 0
nest_crab 1304 1 = 302023104, 21955672, 0
nest_crab 1304 2 = 0, 0, 0
nest_crab 1304 3 = 0, 0, 0
nest_crab 1305 1 = 3, 21955672, 0
nest_crab 1305 2 = 3, 0, 0
nest_crab 1305 3 = 0, 0, 0
nest_crab 1306 1 = 3, 21955672, 0
nest_crab 1306 2 = 0, 0, 0
nest_crab 1306 3 = 0, 0, 0
nest_crab 1351 1 = 0, 0, 0
nest_crab 1351 2 = 0, 0, 0
nest_crab 1351 3 = 0, 0, 0
nest_crab 1352 1 = 0, 0, 0
nest_crab 1352 2 = 0, 0, 0
nest_crab 1352 3 = 0, 0, 0
";

/// The enemy's state.
fn state_of(run: &Run, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(run.sim.objects().get(id).unwrap().state)
}

/// The seventh level with every frame drawn in its frame box, as before
/// a picture's last row and column counted for being on screen.
fn without_picture_boxes(mut level: SimLevel) -> SimLevel {
    for f in level.images.values_mut() {
        f.picture_boxes.clear();
    }
    level
}

#[test]
fn install_a_pirate_with_one_row_in_view_is_on_screen() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level6-original-4.level7.csv");
    let mut run = Run::new(&level, &text, PAIR_4_SMALL);
    let mut before = Run::new(&without_picture_boxes(level.clone()), &text, PAIR_4_SMALL);
    run.to(9537);
    before.to(9537);
    assert_eq!(run.rng, before.rng);
    let pirate = run
        .find("RedTailPirate", 2423)
        .expect("the pirate of record 2423");
    let o = run.sim.objects().get(pirate).unwrap().clone();
    assert_eq!(state_of(&run, pirate), Some(EnemyState::FastAdvance));
    assert_eq!(run.at(pirate), (25726, 6234));
    // Its picture is 84 by 111 pixels: 111 rows, one more than its frame
    // box has.
    let picture = level.picture_box(&o.image_set, o.frame).unwrap();
    let frame = level.frame_box(&o.image_set, o.frame).unwrap();
    assert_eq!(picture.bottom - picture.top + 1, 111);
    assert_eq!(picture.bottom, frame.bottom + 1);
    // It covers rows 6188 to 6298, and the view the next tick tests it
    // against starts at row 6298: one row of it is in view.
    let drawn = run.sim.with_world(|w| w.draw_box(&o)).unwrap();
    let view = run.sim.with_world(|w| w.view);
    assert_eq!((drawn.top, drawn.bottom), (6188, 6298));
    assert_eq!(view.top, 6298);
    // So it is on screen and runs on one tick more before it goes idle.
    run.to(9538);
    before.to(9538);
    assert_eq!(state_of(&run, pirate), Some(EnemyState::FastAdvance));
    assert_eq!(run.at(pirate), (25731, 6234));
    assert_eq!(run.rng[9538], 1_866_851_303);
    // Counted off screen a tick early, it goes idle at once.
    let other = before.find("RedTailPirate", 2423).unwrap();
    assert_eq!(state_of(&before, other), Some(EnemyState::Idle));
    assert_eq!(before.at(other), (25726, 6234));
    assert_eq!(before.rng[9538], 4_002_631_438);
    run.to(9539);
    assert_eq!(state_of(&run, pirate), Some(EnemyState::Idle));
    assert_eq!(run.at(pirate), (25731, 6234));
    assert_eq!(run.rng[9539], 3_435_369_622);
    println!(
        "the pirate of record 2423 has its last row in view at tick 9538 and runs on to (25731, 6234); idle at tick 9539; random state after tick 9538 {}",
        run.rng[9538]
    );
}

#[test]
fn install_a_pirate_with_one_row_in_view_runs_on_from_a_direct_start() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level7-original-4.csv");
    let mut run = Run::new(&level, &text, DIRECT_4_EMPTY);
    let mut before = Run::new(&without_picture_boxes(level), &text, DIRECT_4_EMPTY);
    run.to(9469);
    before.to(9469);
    assert_eq!(run.rng, before.rng);
    let pirate = run
        .find("RedTailPirate", 2423)
        .expect("the pirate of record 2423");
    assert_eq!(state_of(&run, pirate), Some(EnemyState::FastAdvance));
    assert_eq!(run.at(pirate), (25761, 6234));
    // With one row in view at tick 9470 it runs on one tick more.
    run.to(9470);
    before.to(9470);
    assert_eq!(state_of(&run, pirate), Some(EnemyState::FastAdvance));
    assert_eq!(run.at(pirate), (25765, 6234));
    // Counted off screen a tick early, it goes idle at once.
    let other = before.find("RedTailPirate", 2423).unwrap();
    assert_eq!(state_of(&before, other), Some(EnemyState::Idle));
    assert_eq!(before.at(other), (25761, 6234));
    // Idle on the next tick, where it stopped.
    run.to(9471);
    assert_eq!(state_of(&run, pirate), Some(EnemyState::Idle));
    assert_eq!(run.at(pirate), (25765, 6234));
    assert_eq!(run.rng[9470], 2_133_050_505);
    assert_eq!(before.rng[9470], 1_351_267_624);
    assert_eq!(run.rng[9471], 2_058_887_197);
    println!(
        "from a direct start the pirate of record 2423 runs on to (25765, 6234) at tick 9470; idle at tick 9471; random state after tick 9470 {}",
        run.rng[9470]
    );
}

#[test]
fn install_a_hop_back_a_fraction_past_its_limit_is_refused() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let text = earlier("level6-original-4.level7.csv");
    let mut run = Run::new(&level, &text, PAIR_4_COVERING);
    run.to(2000);
    assert_eq!(run.rng[2000], 958_775_274);
    run.to(4829);
    let pirate = run
        .find("RedTailPirate", 2399)
        .expect("the pirate of record 2399");
    // Walking back, a tenth of a pixel past 16002, with its right limit
    // 100 px from the whole pixel.
    assert_eq!(state_of(&run, pirate), Some(EnemyState::MoveBack));
    assert_eq!(run.at(pirate), (16002, 7386));
    let exact = run.sim.with_world(|w| enemy::exact_x(w, pirate));
    assert!((exact - 16002.1).abs() < 0.005, "{exact}");
    let bounds = enemy::data_of(run.sim.objects().get(pirate).unwrap())
        .unwrap()
        .bounds;
    assert_eq!(bounds, (15844, 16102));
    // Claw's swing at tick 4830 makes it try a hop back to 16102.1, past
    // the limit: it walks on, on the floor, and the swing's blow hits it
    // at tick 4838.
    for t in 4830..=4837 {
        run.to(t);
        let o = run.sim.objects().get(pirate).unwrap();
        assert_eq!(state_of(&run, pirate), Some(EnemyState::MoveBack), "{t}");
        assert_eq!(o.physics_type, ptype::STAND, "{t}");
        assert_eq!(o.flags & flags::SAFE, 0, "{t}");
    }
    assert_eq!(run.at(pirate), (16023, 7386));
    run.to(4838);
    assert_eq!(state_of(&run, pirate), Some(EnemyState::HitLow));
    assert_eq!(run.rng[4838], 1_148_499_532);
    // On through the rest of the original's run.
    for (t, rng) in [
        (6000, 1_599_123_078),
        (9000, 1_927_483_480),
        (12000, 4_126_983_214),
        (12748, 2_940_342_670),
    ] {
        run.to(t);
        assert_eq!(run.rng[t], rng, "tick {t}");
    }
    println!(
        "the pirate of record 2399 at x {exact:.2} does not hop back past its limit 16102 and is hit at tick 4838; random state after it {}",
        run.rng[4838]
    );
}
