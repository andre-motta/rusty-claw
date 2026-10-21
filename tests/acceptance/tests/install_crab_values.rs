//! The values the seventh level's hermit crabs start with, on your own
//! install (skipped without `CLAW_INSTALL`).
//!
//! A crab's second range box and the side its first regroup faces by are
//! not fixed by the original game: they differ from crab to crab and from
//! run to run, and a box that covers Claw makes the crab decide as if he
//! were far whenever he is outside its nearer boxes. A run's values are
//! given per crab ([`CrabValues`]).
//!
//! The route `routes/earlier/level7-original.csv` is the seventh level's
//! original-speeds route as it was before the crabs' values were given to
//! routes: it wins with every crab at the plain default. Played with the
//! values a run of the original game starts its placed crabs with (record
//! 1647 side 3, box 300 by 40; record 86 side 2, 300 by 40; record 85 an
//! empty box; record 101 a box that covers its whole beach), the crab of
//! record 101 comes home from hiding at tick 3313 and at tick 3314, Claw
//! rising in a jump above it, rolls to hide and hides, drawing its hide
//! time: two draws more than with the default, where it crouches. From
//! there the game goes otherwise, and Claw loses his first life at 79362
//! ms, long before he loses one at the default.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::tuning::TICK_MS;
use claw_sim::{InputScript, Sim, SimLevel};

/// The values of the placed crabs on a direct start of the level.
const DIRECT_START: &str = "\
crab 1647 = 3, 300, 40
crab 86 = 2, 300, 40
crab 85 = 1721092825, 857312352, -117703298
crab 101 = 1294344774, 857312352, 784380376
";

/// The tick at which the crab of record 101 comes home, and the next.
const HOME: usize = 3313;

/// The random state after tick [`HOME`], either way.
const RNG_HOME: u32 = 2_752_003_382;

/// The random state after the next tick with the values, and with every
/// crab at the default.
const RNG_HIDES: u32 = 3_831_550_551;
const RNG_CROUCHES: u32 = 3_552_864_821;

/// What a play of the route showed.
struct Played {
    /// The crab's state after each tick.
    crab: Vec<Option<EnemyState>>,
    /// The random state after each tick.
    rng: Vec<u32>,
    /// The tick of the first life lost.
    first_death: Option<usize>,
    /// The tick of the level's end.
    won: Option<usize>,
}

fn level7(root: &Path) -> SimLevel {
    let store = Install::open(root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(7)).unwrap();
    SimLevel::from_level(&data, &store).unwrap()
}

fn play(level: &SimLevel, text: &str, crabs: &CrabValues) -> Played {
    let script = InputScript::parse(text).unwrap();
    let seed = script.header.seed.expect("the route names its seed");
    let mut sim = Sim::new(level.clone(), route::sim_config(TuningName::Original, seed));
    let crab: ObjectId = sim
        .objects()
        .all()
        .find(|(_, o)| o.serial == 101 && &*o.logic == "HermitCrab")
        .map(|(id, _)| id)
        .expect("the crab of record 101");
    crabs.apply(&mut sim);
    let lives = sim.stats().lives;
    let mut out = Played {
        crab: Vec::new(),
        rng: Vec::new(),
        first_death: None,
        won: None,
    };
    for (k, &t) in script.ticks.iter().enumerate() {
        sim.step(t);
        crabs.apply(&mut sim);
        out.crab.push(
            sim.objects()
                .get(crab)
                .and_then(|o| EnemyState::from_id(o.state)),
        );
        out.rng.push(sim.rng_state());
        if out.first_death.is_none() && sim.stats().lives < lives {
            out.first_death = Some(k);
        }
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. }))
        {
            out.won.get_or_insert(k);
        }
    }
    out
}

/// `seed` after `n` draws.
fn after_draws(seed: u32, n: usize) -> u32 {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        r.next_u15();
    }
    r.state()
}

#[test]
fn install_a_covering_box_makes_a_crab_hide_where_the_default_crouches() {
    let Some(root) = install() else { return };
    let level = level7(&root);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("routes/earlier/level7-original.csv");
    let text = std::fs::read_to_string(&path).expect("the earlier route");
    let values = CrabValues::parse(DIRECT_START).unwrap();
    let given = play(&level, &text, &values);
    let plain = play(&level, &text, &CrabValues::default());

    // Up to the crab's homecoming the two games are the same.
    assert_eq!(given.rng[..=HOME], plain.rng[..=HOME]);
    assert_eq!(given.rng[HOME], RNG_HOME);
    assert_eq!(given.crab[HOME - 1], Some(EnemyState::Hide));
    assert_eq!(given.crab[HOME], Some(EnemyState::Regroup), "it comes home");
    assert_eq!(plain.crab[HOME], Some(EnemyState::Regroup));

    // The next tick it hides with its two draws (its roll to hide and its
    // hide time, besides the five draws of the tick's pulse rolls); with
    // the empty box it crouches without a draw.
    assert_eq!(given.crab[HOME + 1], Some(EnemyState::Hide));
    assert_eq!(plain.crab[HOME + 1], Some(EnemyState::Defense));
    assert_eq!(given.rng[HOME + 1], RNG_HIDES);
    assert_eq!(plain.rng[HOME + 1], RNG_CROUCHES);
    assert_eq!(after_draws(RNG_HOME, 7), RNG_HIDES);
    assert_eq!(after_draws(RNG_HOME, 5), RNG_CROUCHES);

    // From there the games part: with the values Claw loses his first
    // life at 79362 ms; with the default he keeps every life far beyond.
    // (The route was searched before a human enemy went idle at its
    // patrol limit only exactly on it: from tick 6808, where the bear
    // sailor of record 1348 walks a tick more, it no longer wins at the
    // default either, and loses its first life at tick 7519.)
    let at_ms = |k: usize| (k as u64 + 1) * u64::from(TICK_MS);
    let death = given.first_death.expect("a life lost");
    assert_eq!(at_ms(death), 79_362);
    let plain_death = plain.first_death.expect("a life lost at the default");
    assert_eq!(plain_death, 7519);
    assert!(plain.won.is_none());
    println!(
        "earlier level7-original.csv: with the direct start's crab values the first life is lost at tick {death} ({} ms); at the default at tick {plain_death}",
        at_ms(death)
    );
}

#[test]
fn install_the_routes_crab_values_read() {
    let Some(_) = install() else { return };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("routes");
    for name in ["level7-original.csv", "level6-original.level7.csv"] {
        let text = std::fs::read_to_string(dir.join(name)).unwrap();
        let (crabs, variants) = route::crab_values(&text).unwrap();
        assert!(!crabs.is_empty(), "{name}: the run's crab values");
        assert!(!variants.is_empty(), "{name}: further crab values");
    }
}
