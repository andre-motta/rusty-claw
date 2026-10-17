//! The start of the first level of your own install against the reference
//! store level1/bot-original at c18 (skipped without `CLAW_INSTALL`): the
//! level's set-up draws, the latch on keys down when play starts, and the
//! random stream through the first 28 ticks of that route.

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::sim::LEVEL_LOAD_SEED;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, Tuning};

const JUMP: u32 = 0x1;
const RIGHT: u32 = 0x200_0000;

/// The random state the reference runs of the first level start from: the
/// level-load state advanced by the six set-up draws.
const RNG_AT_T0: u32 = 4_105_512_405;

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

#[test]
fn install_level1_sets_up_with_six_draws_and_starts_as_the_reference() {
    let Some(level) = level1() else { return };
    let cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        seed: RNG_AT_T0,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    // The objects live at the start draw six times while the level loads,
    // which is the reference's random state at time zero.
    let mut r = Rng::new(LEVEL_LOAD_SEED);
    for _ in 0..6 {
        r.next_u15();
    }
    assert_eq!(r.state(), RNG_AT_T0);
    assert_eq!(sim.rng_after_set_up(), RNG_AT_T0);
    assert_eq!(sim.rng_state(), RNG_AT_T0);
    // The route's first rows: Right and Jump down from the first tick, then
    // let go, then Right pressed again at tick 22 and let go at tick 28.
    let mut held = vec![RIGHT | JUMP; 14];
    held.extend([RIGHT; 2]);
    held.extend([0; 6]);
    held.extend([RIGHT; 6]);
    held.push(0);
    let rows = InputScript::from_held(&held);
    // The reference trace at those ticks: Claw's x, state and the random
    // state after each tick (t_ms 18 to 522).
    let reference: [(i32, i32, u32); 29] = [
        (689, 24, 714_107_639),
        (689, 24, 714_107_639),
        (689, 24, 714_107_639),
        (689, 24, 714_107_639),
        (689, 24, 714_107_639),
        (689, 24, 499_382_750),
        (689, 24, 499_382_750),
        (689, 24, 499_382_750),
        (689, 24, 2_731_780_393),
        (689, 24, 2_731_780_393),
        (689, 24, 2_731_780_393),
        (689, 24, 1_276_479_304),
        (689, 24, 1_276_479_304),
        (689, 24, 1_276_479_304),
        (689, 24, 1_772_955_883),
        (689, 24, 1_772_955_883),
        (689, 24, 1_772_955_883),
        (689, 24, 1_019_121_666),
        (689, 24, 1_019_121_666),
        (689, 24, 1_019_121_666),
        (689, 24, 2_553_378_493),
        (689, 24, 2_553_378_493),
        (693, 8, 2_553_378_493),
        (697, 8, 3_209_916_044),
        (701, 8, 3_209_916_044),
        (705, 8, 3_209_916_044),
        (709, 8, 925_729_567),
        (713, 8, 925_729_567),
        (713, 24, 925_729_567),
    ];
    assert_eq!(state::STAND, 24);
    for (k, (&t, &want)) in rows.ticks.iter().zip(&reference).enumerate() {
        sim.step(t);
        let o = sim.player().expect("Claw");
        assert_eq!(
            (o.x, o.y, o.state, sim.rng_state()),
            (want.0, 4730, want.1, want.2),
            "tick {k} (t_ms {})",
            18 * (k + 1)
        );
    }
}
