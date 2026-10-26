//! Rules of the fourteenth level's play on your own install (skipped
//! without `CLAW_INSTALL`), pinned to a moment of
//! `routes/level14-original.csv`, played from the level's start, where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's:
//!
//! - the in-game menu's frames keep the enemies' rhythm going: Omar makes
//!   his roll of the frame pulse in ticks 16019 and 16022, as Claw reaches
//!   the ladder top above his chamber, where his introduction freezes
//!   Claw. The route ends the life from the in-game menu before tick
//!   16025, and the menu's five frames (it opens, three for the
//!   confirmation page, one for the yes) run the pulse countdown on from
//!   the 15 ms it stood at after tick 16024, so tick 16025 is no pulse
//!   tick and makes no draw, and Omar's next roll comes in tick 16026, a
//!   tick later than without the menu, and so every third tick after it.
//!
//! Claw dies the End Life's death in tick 16025 and is back on the ladder
//! top in tick 16119 with one life fewer, and the route ends the level in
//! tick 23811 with 75 health and 6 lives, no other life lost.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::input::ScriptCommand;
use claw_sim::player::state;
use claw_sim::{DirectiveKind, InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the route.
const RNG: [(usize, u32); 5] = [
    (16022, 3_327_783_446),
    (16024, 3_327_783_446),
    (16025, 3_327_783_446),
    (16026, 1_739_104_385),
    (16028, 1_739_104_385),
];

/// The tick before which the route ends Claw's life from the in-game
/// menu.
const END_LIFE: usize = 16025;

/// The tick Claw is back on the ladder top in.
const BACK: usize = 16119;

/// The tick the route ends the level in.
const LAST_TICK: usize = 23811;

/// Claw's health and lives at the level's end.
const AT_THE_END: (i32, i32) = (75, 6);

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
fn install_the_in_game_menus_frames_keep_the_enemies_rhythm_on_the_fourteenth_level() {
    let Some(root) = install() else {
        return;
    };
    let level = level(&root, 14);
    let script = InputScript::parse(&routes_file("level14-original.csv")).unwrap();
    let seed = script.header.seed.expect("the route names its seed");
    let mut sim = Sim::new(level, route::sim_config(TuningName::Original, seed));
    let mut rng = Vec::with_capacity(LAST_TICK + 1);
    let mut pulse = Vec::with_capacity(LAST_TICK + 1);
    let mut claw = Vec::with_capacity(LAST_TICK + 1);
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
        pulse.push(sim.pulse());
        let c = sim.player().expect("Claw");
        claw.push((c.x, c.y, c.state, sim.stats().lives));
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
    let end_lifes: Vec<usize> = script
        .directives()
        .iter()
        .filter(|d| d.kind == DirectiveKind::Command(ScriptCommand::EndLife))
        .map(|d| d.tick)
        .collect();
    assert_eq!(end_lifes, [END_LIFE], "the route's one End Life");
    for &(t, want) in &RNG {
        assert_eq!(rng[t], want, "random state after tick {t}");
    }
    // Omar's rolls in ticks 16019 and 16022, then, after the menu's five
    // frames, none in 16025 and the next in 16026.
    let pulses: Vec<usize> = (16017..=16032).filter(|&t| pulse[t]).collect();
    assert_eq!(pulses, [16019, 16022, 16026, 16029, 16032]);
    // Frozen on the ladder top, the death from there, and back.
    let lives = claw[END_LIFE - 1].3;
    assert_eq!(claw[END_LIFE - 1].0, 41839);
    assert_eq!(claw[END_LIFE - 1].1, 1959);
    assert_eq!(claw[END_LIFE].2, state::DEATH_FALL);
    assert_eq!(
        claw[BACK],
        (41839, 1959, claw[BACK].2, lives - 1),
        "back on the ladder top with a life fewer"
    );
    assert_ne!(claw[BACK - 1].1, 1959, "still falling a tick before");

    assert_eq!(ended, Some(LAST_TICK), "the level ends in tick {LAST_TICK}");
    let stats = sim.stats();
    assert_eq!((stats.health, stats.lives), AT_THE_END);
    assert_eq!(stats.lives, lives - 1, "no other life lost");
}
