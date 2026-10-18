//! The fourth level of your own install, played tick by tick along the
//! routes the original was run with at one 18 ms step per frame and the
//! original speeds (skipped without `CLAW_INSTALL`): at the tick where
//! each of these rules first shows on those routes, the game does what
//! the original did there. The routes are the fourth level's
//! original-speeds route, the fourth level of the game that starts on the
//! third, and Claw standing still at the level's start for 30 s, kept
//! here as they were run.
//!
//! 1. Every enemy is built on its patrol limits: the construction step all
//!    enemies share moves one placed beside its range onto the nearer
//!    limit before it is dropped to the floor. The rat placed at
//!    4746,4416 with the limits 5352 to 5464 across wakes in tick 1300 of
//!    the first route, when its placement point comes into the activation
//!    window, and is built at 5352,4858, 1400 px from the window's centre:
//!    it goes back to sleep at the end of that tick and makes no roll in
//!    tick 1301. The same rat does the same in tick 1231 of the game's
//!    fourth level.
//! 2. Claw's teeter line is one of his lines, said through his dialog
//!    helper: he teeters in tick 627 of the game's fourth level at
//!    2531,3405 and says it, and the helper then follows him at his depth
//!    plus one and stays in the object list after the line. In tick 665
//!    a coin settling beside him makes its glitter one depth further on;
//!    the update walk reaches the glitter in the same tick, after the
//!    helper, and its start delay's draw comes in that tick.
//! 3. Standing still from the level's start, Claw's idle wait is 20 s, not
//!    the 15 s of a stand entered later: his idle animation starts in tick
//!    1111 (at 20016 ms), and when it reaches its line picture in tick
//!    1219 he says one of his twelve idle lines, a draw of `0..12`.
//! 4. An enemy's ledge counts only floor with clear air above it: on the
//!    original-speeds route as it was searched before this rule, the
//!    RobberThief placed at 7080,4376 wakes in tick 3280 with the floor
//!    ending 41 px to his left and a wall rising 24 px to his right over
//!    floor that goes on. The wall ends his ledge, so in tick 3281 his
//!    leap check runs, finds Claw at 6432,3807 too far away, and he stays
//!    put for good. In tick 3627 his idle roll gives him the chance to
//!    pace; standing put, he goes idle again instead, which draws a voice
//!    line and an idle roll.
//!
//! The random stream equals the original's after every tick checked here.

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::claw_frame::AudioCue;
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::{pickup, sound};
use claw_sim::object::Object;
use claw_sim::player::progress::parse_start_lines;
use claw_sim::player::{IDLE_WAIT_AFTER_CLOCK_RESET_MS, TEETER_VOICE, attack, state};
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, StartState, Tuning};

/// The random state the reference runs of the fourth level start from.
const LEVEL4_RNG_AT_T0: u32 = 1_617_999_171;

/// The fourth level's original-speeds route, its first 1302 ticks: held
/// masks in hex, each with its run of ticks.
const BOT_ROUTE: &str = "
    0:2 2000001:9 2000000:26 2000001:14 2000000:2 0:18 2000000:6 1:14 0:2 2000000:24 2000001:9
    2000000:14 0:6 8000002:2 8000000:6 1:14 0:2 1000000:36 2000000:6 8000000:6 1:14 0:2 2:2 0:6
    2000000:24 0:6 2000000:6 1000001:2 1000000:8 2000000:12 2:2 0:6 1000000:18 2:2 0:6
    1000000:12 2000001:2 2000000:2 0:6 1000000:6 2:2 0:6 2:2 0:6 2000000:42 1:2 0:2 2000000:6
    2:2 0:6 2000000:6 2000001:5 2000000:2 2:2 0:6 2000000:6 2000001:5 2000000:20 2000001:5
    2000000:38 2000001:14 2000000:14 2:2 0:6 2000000:18 8000000:6 8000002:2 8000000:6 2000000:18
    2000001:2 2000000:8 1000000:6 0:6 2000001:5 2000000:14 6000000:6 2000001:14 2000000:32 0:24
    2000000:24 2000001:5 2000000:20 2000001:5 2000000:50 2000001:5 2000000:14 0:6 2000000:6
    1000000:6 2:2 0:6 1000000:18 2:2 0:6 2000000:6 1000000:24 2000000:6 1000000:30 2000000:24
    2000001:9 2000000:20 2000001:14 2000000:2 0:12 2000000:6 1:9 0:2 2000000:24 0:6 1:14 0:2
    2000000:12 2:2 0:6 2000001:14 2000000:14 0:6 2000000:6 2000001:14 2000000:20 0:6 2000001:14
    2000000:2 1000000:6 2000000:12 0:6 2000001:9 2000000:14 1000000:6 2000000:6 2000001:6
";

/// The fourth level of the game that starts on the third, its first 1233
/// ticks.
const PAIR_ROUTE: &str = "
    0:2 2000001:9 2000000:26 2000001:14 2000000:8 0:18 2000001:14 2000000:26 1:9 0:2 2000000:6
    2:2 0:6 1000001:2 1000000:8 2000000:12 2:2 0:6 1000000:30 2:2 0:6 1000001:2 1000000:2
    2000000:6 0:6 1000000:6 2:2 0:6 2000000:30 0:6 2000000:6 8000000:6 1:14 0:2 1000000:36
    2000000:6 2000001:2 2000000:2 0:6 1000000:6 2:2 0:6 2:2 0:6 2000000:42 1:2 0:2 2000000:6 2:2
    0:6 2000000:6 2000001:5 2000000:2 2:2 0:6 2000000:6 2000001:5 2000000:20 2000001:5
    2000000:38 2000001:14 2000000:14 2:2 0:6 2000000:18 2:2 0:6 2000000:12 2000001:2 2000000:8
    2:2 0:12 2000001:5 2000000:14 6000000:6 2000001:14 2000000:26 2000001:2 2000000:2 2:2 0:18
    2000000:24 2000001:5 2000000:50 2000001:5 2000000:20 2000001:5 2000000:14 0:6 2000000:6
    1000000:6 2:2 0:6 1000000:18 2:2 0:12 1000000:18 2:2 0:12 2000000:42 1:14 0:2 2000000:24
    8000000:6 1000001:2 1000000:2 2000001:14 2000000:8 0:12 1:9 0:2 2000000:24 2000001:5
    2000000:8 2:2 0:6 2000001:14 2000000:20 0:6 1:14 0:2 2000000:24 1:9 0:2 2000000:24 2000001:3
";

/// The fourth level's original-speeds route as it was searched before the
/// narrow-ledge rule, its first 3628 ticks.
const LEDGE_ROUTE: &str = "
    0:2 2000001:9 2000000:26 2000001:14 2000000:2 0:18 2000000:6 1:14 0:2 2000000:24 2000001:9
    2000000:14 0:6 8000002:2 8000000:6 1:14 0:2 1000000:36 2000000:6 8000000:6 1:14 0:2 2:2 0:6
    2000000:24 0:6 2000000:6 1000001:2 1000000:8 2000000:12 2:2 0:6 1000000:18 2:2 0:6 1000000:12
    2000001:2 2000000:2 0:6 1000000:6 2:2 0:6 2:2 0:6 2000000:42 1:2 0:2 2000000:6 2:2 0:6
    2000000:6 2000001:5 2000000:2 2:2 0:6 2000000:6 2000001:5 2000000:20 2000001:5 2000000:38
    2000001:14 2000000:14 2:2 0:6 2000000:18 8000000:6 8000002:2 8000000:6 2000000:18 2000001:2
    2000000:8 1000000:6 0:6 2000001:5 2000000:14 6000000:6 2000001:14 2000000:32 0:24 2000000:24
    2000001:5 2000000:20 2000001:5 2000000:50 2000001:5 2000000:14 0:6 2000000:6 1000000:6 2:2 0:6
    1000000:18 2:2 0:6 2000000:6 1000000:24 2000000:6 1000000:30 2000000:24 2000001:9 2000000:20
    2000001:14 2000000:2 0:12 2000000:6 1:9 0:2 2000000:24 0:6 1:14 0:2 2000000:12 2:2 0:6
    2000001:14 2000000:14 0:6 2000000:6 2000001:14 2000000:20 0:6 2000001:14 2000000:2 1000000:6
    2000000:12 0:6 2000001:9 2000000:14 1000000:6 2000000:6 2000001:9 2000000:8 0:6 2000000:6
    2000001:9 2000000:14 2000001:2 2000000:8 1000000:6 2:2 0:6 2000001:5 2000000:2 0:6 2:2 0:6
    2000001:2 2000000:8 1000000:6 2:2 0:6 8000002:2 8000000:6 2000001:2 2000000:2 0:6 2000001:5
    2000000:56 2000001:2 2000000:8 0:12 2000001:5 2000000:20 8000000:6 1:14 0:2 2000000:54
    2000001:9 2000000:20 2000001:5 2000000:2 2:2 0:6 1:2 0:14 2:2 0:6 2:2 0:6 1000000:6 0:6
    1000000:6 2000000:12 0:6 1000000:6 1000001:5 1000000:20 1:2 0:2 1000000:18 1000001:9 1000000:20
    2:2 0:6 1000001:2 1000000:14 0:6 1000001:2 1000000:2 0:18 1000001:5 1000000:26 1000001:5
    1000000:8 0:18 1000001:5 1000000:32 1000001:9 1000000:20 0:6 1000000:12 1000001:2 1000000:44
    1:14 0:2 6000000:6 1000001:14 1000000:2 2000000:6 6000000:6 1000001:14 1000000:2 2000000:6
    6000000:6 2000001:14 2000000:8 1000000:6 5000000:6 8000000:6 2000001:14 2000000:26 6000000:6
    2000001:14 2000000:2 1000000:6 5000000:6 8000000:12 1000001:5 1000000:14 2000000:6 6000000:6
    2000001:14 2000000:26 6000000:6 1000001:14 1000000:2 2000000:6 6000000:6 8000000:6 1000001:5
    1000000:2 2000000:12 4000000:6 8000000:18 2000001:14 2000000:26 6000000:6 1000001:14 1000000:2
    2000000:6 6000000:6 8000000:6 0:6 8000000:12 2000001:5 2000000:2 0:18 4000000:6 8000000:6
    2000001:14 2000000:26 6000000:6 2000001:14 2000000:2 1000000:6 5000000:6 8000000:6 2000001:14
    2000000:14 8000000:6 2000001:9 2000000:2 8000000:6 2000001:2 2000000:2 1:14 0:2 2:2 0:6
    2000000:6 1000000:12 2000000:6 1:14 0:2 2000000:6 1000000:6 0:12 1:14 0:20 2:2 0:6 2000001:14
    2000000:38 8000000:12 2000001:5 2000000:2 8000000:6 2000001:2 2000000:2 2:2 0:6 2000000:42 2:2
    0:6 1000000:6 0:6 2000001:2 2000000:26 2000001:14 2000000:8 0:6 2:2 0:6 2000000:6 2000001:14
    2000000:8 1000000:6 2000000:12 2:2 0:6 2000000:6 1:14 0:2 2000000:18 2000001:14 2000000:74 0:6
    1:14 0:2 2000000:42 0:6 2000000:18 2:2 0:6 2000000:72 8000000:6 2000001:14 2000000:38 8000000:6
    1000001:2 1000000:8 2:2 0:6 2000000:12 0:6 2:2 0:12 1000000:12 2000000:12 2000001:14 2000000:14
    0:6 2000001:14 2000000:8 6000000:6 2000001:14 2000000:2 1000000:12 4000000:6 2000001:14
";

/// The progress that game carries into the fourth level.
const PAIR_START: &str = "
    # start_score = 99200
    # start_health = 25
    # start_lives = 7
    # start_pistol = 0
    # start_magic = 5
    # start_dynamite = 3
    # start_weapon = pistol
    # start_treasure_game = 167, 3, 9, 2, 4, 1, 2, 1, 0
";

/// The original's random state after these ticks of the first route.
const BOT_RNG: [(usize, u32); 3] = [
    (1299, 2_846_705_220),
    (1300, 3_700_743_159),
    (1301, 3_256_750_988),
];

/// The original's random state after these ticks of the game's fourth
/// level.
const PAIR_RNG: [(usize, u32); 8] = [
    (626, 812_003_758),
    (627, 4_272_489_979),
    (664, 3_467_331_025),
    (665, 2_167_627_143),
    (666, 3_067_654_201),
    (1231, 2_823_685_858),
    (1232, 3_300_803_059),
    (1233, 21_587_178),
];

/// The original's random state after these ticks of Claw standing still.
const STILL_RNG: [(usize, u32); 3] = [
    (1111, 4_005_776_122),
    (1218, 4_005_776_122),
    (1219, 4_105_512_405),
];

/// The original's random state after these ticks of the route searched
/// before the narrow-ledge rule.
const LEDGE_RNG: [(usize, u32); 5] = [
    (3279, 3_889_483_992),
    (3280, 566_709_366),
    (3281, 4_173_513_388),
    (3626, 1_702_573_379),
    (3627, 1_724_676_574),
];

/// The RobberThief beside the wall: his serial number in the game.
const ROBBER: u32 = 1150;

/// The rat placed beside its range: its serial number in the game.
const RAT: u32 = 1136;

fn level4() -> Option<SimLevel> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(4)).expect("level loads");
    Some(SimLevel::from_level(&level, &store).expect("level prepares"))
}

/// The routes' settings: the original speeds without the mid-air hit fix.
fn config() -> SimConfig {
    let mut cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        seed: LEVEL4_RNG_AT_T0,
        ..SimConfig::default()
    };
    cfg.tuning.behaviour.fix_double_jump = false;
    cfg
}

/// The input script of `runs` (hex held masks with their runs of ticks),
/// `len` ticks long.
fn route(runs: &str, len: usize) -> InputScript {
    let mut held = Vec::new();
    for run in runs.split_whitespace() {
        let (mask, n) = run.split_once(':').expect("mask:ticks");
        let mask = u32::from_str_radix(mask, 16).expect("hex mask");
        let n: usize = n.parse().expect("ticks");
        held.extend(std::iter::repeat_n(mask, n));
    }
    assert_eq!(held.len(), len);
    InputScript::from_held(&held)
}

/// The object with serial number `serial`, if it is in the level.
fn by_serial(sim: &Sim, serial: u32) -> Option<&Object> {
    sim.objects()
        .all()
        .find(|(_, o)| o.serial == serial)
        .map(|(_, o)| o)
}

fn check_rng(sim: &Sim, table: &[(usize, u32)], tick: usize) {
    if let Some(&(_, want)) = table.iter().find(|r| r.0 == tick) {
        assert_eq!(sim.rng_state(), want, "random state after tick {tick}");
    }
}

/// The voice lines started in the last tick.
fn voice(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) if s.channel == claw_sim::claw_frame::SoundChannel::Voice => {
                Some(s.key.clone())
            }
            _ => None,
        })
        .collect()
}

/// Claw's dialog helper, once he has said a line.
fn claw_helper(sim: &Sim) -> Option<&Object> {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == sound::CLAW_DIALOG_LOGIC)
        .map(|(_, o)| o)
}

/// Rule 1 on a route: the rat sleeps until `wake`, is built on its left
/// limit and on the floor below it in that tick, and sleeps again by its
/// end.
fn check_rat(sim: &Sim, tick: usize, wake: usize) {
    let rat = by_serial(sim, RAT).expect("the rat's record");
    if tick + 1 == wake {
        assert_eq!((rat.x, rat.y), (4746, 4416), "asleep where it was placed");
    } else if tick == wake || tick == wake + 1 {
        assert_eq!(
            (rat.x, rat.y),
            (5352, 4858),
            "built on its limit, on the floor"
        );
        assert!(!rat.is_listed(), "asleep again after tick {tick}");
    }
}

#[test]
fn install_level4_an_enemy_built_beside_its_range_starts_on_its_limit() {
    let Some(level) = level4() else { return };
    let script = route(BOT_ROUTE, 1302);
    let mut sim = Sim::new(level, config());
    for (tick, t) in script.ticks.iter().enumerate() {
        sim.step(*t);
        check_rng(&sim, &BOT_RNG, tick);
        if tick >= 1299 {
            check_rat(&sim, tick, 1300);
        }
    }
}

#[test]
fn install_level4_the_teeter_line_goes_through_claws_dialog_helper() {
    let Some(level) = level4() else { return };
    let script = route(PAIR_ROUTE, 1233);
    let carry = parse_start_lines(PAIR_START).expect("start lines");
    let mut sim = Sim::with_start(level, config(), StartState { carry, save: None });
    let mut checked = false;
    for (tick, t) in script.ticks.iter().enumerate() {
        sim.step(*t);
        check_rng(&sim, &PAIR_RNG, tick);
        let claw = sim.player().expect("Claw").clone();
        match tick {
            626 => assert!(claw_helper(&sim).is_none(), "no line said yet"),
            627 => {
                assert_eq!((claw.x, claw.y, claw.state), (2531, 3405, state::ON_EDGE));
                assert_eq!(voice(&sim), [TEETER_VOICE]);
                assert!(sim.with_world(|w| sound::claw_is_speaking(w)), "his line");
            }
            664 => {
                let helper = claw_helper(&sim).expect("the helper stays");
                assert!(helper.is_listed());
                assert_eq!(helper.z, claw.z + 1, "at his depth + 1");
            }
            665 => {
                let glitter = sim
                    .objects()
                    .all()
                    .find(|(_, o)| &*o.logic == pickup::GLITTER && o.x == 2706)
                    .map(|(_, o)| o.state)
                    .expect("the settled coin's glitter");
                assert_ne!(glitter, pickup::state::INIT, "run in the tick it was made");
            }
            1230.. => {
                check_rat(&sim, tick, 1231);
                checked = tick == 1232;
            }
            _ => {}
        }
    }
    assert!(checked);
}

#[test]
fn install_level4_standing_still_claw_idles_after_20_s_and_says_an_idle_line() {
    let Some(level) = level4() else { return };
    let mut sim = Sim::new(level, config());
    assert_eq!(sim.claw().afk_time, IDLE_WAIT_AFTER_CLOCK_RESET_MS);
    let idling = |sim: &Sim| sim.player().expect("Claw").anim.key() == Some("CLAW_IDLE");
    let mut said = None;
    for tick in 0..1700 {
        let before = sim.rng().clone();
        sim.step(claw_sim::TickInput::default());
        check_rng(&sim, &STILL_RNG, tick);
        match tick {
            0 => assert_eq!(sim.claw().afk_time, 19_982),
            1110 => assert!(!idling(&sim), "still waiting at 19998 ms"),
            1111 => assert!(idling(&sim), "the idle animation at 20016 ms"),
            1219 => {
                let mut pick = before;
                let line = attack::IDLE_LINES[pick.below(12) as usize];
                assert_eq!(voice(&sim), [line]);
                said = Some(line);
            }
            _ => {}
        }
        if tick != 1219 {
            assert!(
                voice(&sim)
                    .iter()
                    .all(|k| !attack::IDLE_LINES.contains(&k.as_str())),
                "no idle line in tick {tick}"
            );
        }
    }
    assert!(said.is_some());
}

#[test]
fn install_level4_a_wall_beside_an_enemy_ends_its_narrow_ledge() {
    let Some(level) = level4() else { return };
    let script = route(LEDGE_ROUTE, 3628);
    let mut sim = Sim::new(level, config());
    for (tick, t) in script.ticks.iter().enumerate() {
        sim.step(*t);
        check_rng(&sim, &LEDGE_RNG, tick);
        let robber = by_serial(&sim, ROBBER).expect("the RobberThief's record");
        let stationary = enemy::data_of(robber).map(|d| d.stationary);
        match tick {
            3279 => {
                assert!(!robber.is_listed(), "asleep before tick 3280");
                assert_eq!((robber.x, robber.y), (7080, 4376), "where he was placed");
            }
            3280 => {
                assert!(robber.is_listed(), "awake in tick 3280");
                assert_eq!((robber.x, robber.y), (7080, 4388), "on the floor");
                assert_eq!(stationary, Some(false), "not yet put");
            }
            3281.. => {
                assert_eq!(stationary, Some(true), "put for good after tick {tick}");
                assert_eq!(robber.x, 7080);
                if tick == 3281 || tick == 3627 {
                    assert_eq!(
                        EnemyState::from_id(robber.state),
                        Some(EnemyState::Idle),
                        "idle after tick {tick}"
                    );
                }
                if tick == 3281 {
                    let claw = sim.player().expect("Claw");
                    assert_eq!((claw.x, claw.y), (6432, 3807), "Claw too far to leap at");
                }
            }
            _ => {}
        }
    }
}
