//! Three rules of the ninth level's play on your own install (skipped
//! without `CLAW_INSTALL`), each pinned to a moment of a route where the
//! original game showed it, by the state of the shared random stream after
//! the ticks around it, which equals the original's.
//!
//! The route is `routes/earlier/level9-original.csv`, the ninth level's
//! original-speeds route as it was before these rules (with three help
//! screens, before ticks 1353, 1402 and 1451), played from the level's
//! start; `routes/level9-original.csv`, found again under the rules,
//! keeps its first 201 ticks:
//!
//! - a powder keg's blast reaches the frame box of the picture the keg
//!   shows as it goes off, mirrored with the keg: the keg by the ladders
//!   near the start goes off in tick 200, its box ends 50 px right of the
//!   keg, and the crate whose box starts 55 px right of it is not hit and
//!   drops no coin;
//! - the return from a help screen runs one update pass over the listed
//!   objects at the game time of the last tick plus the screen's time,
//!   after the screen's frames have run the frame pulse countdown on: the
//!   PegLeg patrolling the deck moves 24 px (the step limit) in that pass,
//!   and Claw, in mid-jump, does not move;
//! - after that pass the game clock goes back to the time of the last
//!   tick: the PegLeg moves the 24 px back in the next tick, and after the
//!   second help screen it stays in the activation window and makes its
//!   pulse roll in tick 1411. The route then ends without the level's end,
//!   as it did in the original game (PegLeg's choices now follow the
//!   original's stream from tick 201, and his shot hurts Claw in tick
//!   305), and the stream stays the original's to the original run's last
//!   tick, 112 ticks past the route's last row, with nothing held.
//!
//! The clock going back reaches the enemies' timers too: one started in
//! the pass counts as done in the next tick. A first-level route with
//! three help screens shows it: a soldier takes a new stand pose in the
//! pass after the third and again in the next tick.
//!
//! Three more, on `routes/earlier/level9-original-2.csv`, the route as it
//! was found under the rules above and before these three (it keeps the
//! first route's first 201 ticks), and on a first-level route:
//!
//! - a powder keg's blast hurts in the collision pass of the tick it goes
//!   off in: the keg on the bridge high in the shaft, shot with the pistol
//!   in tick 1499, goes off in tick 1512 and hurts Claw, falling past it,
//!   by 20 in that tick, at the height he has then;
//! - the second update pass after a warp runs every listed object but
//!   Claw, with no game time: hurt a tick earlier, Claw falls past the
//!   bridge onto a warp below it in tick 1575 and is sent back near the
//!   level's start, where the hook-handed pirate waiting there, set up
//!   long before and asleep since, makes his per-tick draws in that pass.
//!   The route then ends without the level's end, as it did in the
//!   original game, and the stream stays the original's to the original
//!   run's last tick, 112 ticks past the route's last row, with nothing
//!   held. `routes/level9-original.csv`, found again under these rules,
//!   keeps that route's first 1535 ticks and jumps from the stand after
//!   the blast a tick earlier;
//! - Claw's call in the help return's pass sees no key pressed: a weapon
//!   switch pressed in the tick before a help screen switches once, and a
//!   key held down across the screen does not count as pressed after it.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::keg;
use claw_sim::object::Rect;
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the earlier
/// route.
const EARLIER_RNG: [(usize, u32); 20] = [
    (199, 204_343_330),
    (200, 3_115_547_717),
    (201, 4_108_156_404),
    (202, 3_573_693_176),
    (305, 602_289_968),
    (1352, 1_321_746_010),
    (1353, 1_785_564_870),
    (1354, 1_453_479_027),
    (1401, 3_939_021_163),
    (1402, 2_461_506_759),
    (1403, 117_638_744),
    (1411, 1_147_152_567),
    (1450, 39_743_464),
    (1451, 248_805_774),
    (1452, 3_408_787_122),
    (2000, 3_123_492_565),
    (2209, 650_582_488),
    (2404, 462_866_595),
    (2405, 1_982_890_536),
    (2516, 3_798_773_277),
];

/// Ticks the original run of the earlier route lasted.
const EARLIER_RUN_TICKS: usize = 2517;

/// Where the keg by the ladders near the start, the crate beside it and
/// the PegLeg patrolling the deck are placed in the level.
const KEG: (i32, i32) = (1616, 7216);
const CRATE: (i32, i32) = (1700, 7284);
const PEGLEG: (i32, i32) = (1629, 6942);

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

/// A route played tick by tick, its help screens with it.
struct Run {
    sim: Sim,
    script: InputScript,
    /// The next tick to play.
    tick: usize,
    /// The random state after each tick played.
    rng: Vec<u32>,
    /// The tick whose directives were played already.
    played: Option<usize>,
}

impl Run {
    fn new(level: &SimLevel, text: &str, seed: Option<u32>) -> Run {
        let script = InputScript::parse(text).unwrap();
        let seed = seed
            .or(script.header.seed)
            .expect("the route names its seed");
        let sim = Sim::new(level.clone(), route::sim_config(TuningName::Original, seed));
        Run {
            sim,
            script,
            tick: 0,
            rng: Vec::new(),
            played: None,
        }
    }

    /// Plays the directives due before the next tick, once.
    fn directives(&mut self) {
        if self.played != Some(self.tick) {
            self.sim.play_directives(&self.script, self.tick);
            self.played = Some(self.tick);
        }
    }

    /// Plays ticks until `last` has been played, with nothing held past
    /// the route's last row.
    fn to(&mut self, last: usize) {
        while self.tick <= last {
            self.directives();
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

    fn check_rng(&self, table: &[(usize, u32)]) {
        for &(t, want) in table {
            if t < self.rng.len() {
                assert_eq!(self.rng[t], want, "random state after tick {t}");
            }
        }
    }

    /// The object placed at `at` with logic `logic`: (x, y, state).
    fn placed(&self, logic: &str, at: (i32, i32)) -> Option<(i32, i32, i32)> {
        self.sim
            .objects()
            .all()
            .find(|(_, o)| &*o.logic == logic && (o.editor.0, o.editor.1) == at)
            .map(|(_, o)| (o.x, o.y, o.state))
    }

    fn claw_at(&self) -> (i32, i32) {
        let c = self.sim.player().expect("Claw");
        (c.x, c.y)
    }
}

#[test]
fn install_a_kegs_blast_reaches_the_box_of_its_picture() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    // The route found again keeps the earlier route's first 201 ticks.
    let earlier = InputScript::parse(&routes_file("earlier/level9-original.csv")).unwrap();
    let now = InputScript::parse(&routes_file("level9-original.csv")).unwrap();
    assert_eq!(earlier.ticks[..201], now.ticks[..201]);
    let mut run = Run::new(&level, &routes_file("level9-original.csv"), None);
    run.to(199);
    let crate_before = run.placed("BehindCrate", CRATE).expect("the crate");
    assert!(run.placed("PowderKeg", KEG).is_some(), "the keg");
    run.to(200);
    // The keg's blast in tick 200: the box of its explosion picture.
    let (set, frame, mirrored) = run
        .sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "PowderKeg" && (o.editor.0, o.editor.1) == KEG)
        .map(|(_, o)| {
            (
                o.image_set.clone(),
                o.frame,
                o.draw_flags & claw_sim::object::draw_flags::MIRROR != 0,
            )
        })
        .expect("the exploding keg");
    let reach = run
        .sim
        .with_world(|w| keg::blast_box(w, &set, frame, mirrored))
        .expect("the level knows the picture");
    assert_eq!(reach, Rect::new(-41, -39, 50, 68));
    run.to(230);
    // The crate stays as it was and drops nothing.
    assert_eq!(run.placed("BehindCrate", CRATE), Some(crate_before));
    run.check_rng(&EARLIER_RNG[..4]);
}

#[test]
fn install_the_help_screens_return_runs_a_pass_and_the_clock_goes_back() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    let mut run = Run::new(&level, &routes_file("earlier/level9-original.csv"), None);
    run.to(1352);
    run.check_rng(&EARLIER_RNG);
    assert_eq!(run.placed("PegLeg", PEGLEG).map(|p| p.0), Some(1844));
    let claw = run.claw_at();
    assert_eq!(claw, (2476, 7125));
    // The return from the first help screen: the PegLeg covers the step
    // limit, Claw in mid-jump does not move.
    run.directives();
    assert_eq!(run.placed("PegLeg", PEGLEG).map(|p| p.0), Some(1820));
    assert_eq!(run.claw_at(), claw);
    // The clock goes back: the PegLeg moves back as far, losing its step.
    run.to(1353);
    assert_eq!(run.placed("PegLeg", PEGLEG).map(|p| p.0), Some(1844));
    assert_eq!(run.claw_at(), (2476, 7119));
    // Still in the activation window, it makes its pulse roll from x 1723
    // in tick 1411.
    run.to(1410);
    assert_eq!(run.placed("PegLeg", PEGLEG).map(|p| p.0), Some(1723));
    run.to(1411);
    run.check_rng(&EARLIER_RNG);
}

#[test]
fn install_the_earlier_route_follows_the_original_to_its_runs_end() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    let mut run = Run::new(&level, &routes_file("earlier/level9-original.csv"), None);
    run.to(304);
    assert_eq!(run.sim.stats().health, 100);
    run.to(305);
    assert_eq!(run.sim.stats().health, 90, "PegLeg's shot");
    run.to(2208);
    assert_eq!(run.sim.stats().lives, 7);
    run.to(2209);
    assert_eq!(run.sim.stats().lives, 6);
    run.to(2404);
    assert_eq!(run.claw_at(), (1902, 7655));
    assert_eq!(run.sim.stats().score, 12600);
    run.to(EARLIER_RUN_TICKS - 1);
    assert_eq!(run.claw_at(), (1902, 7771));
    run.check_rng(&EARLIER_RNG);
}

/// The original game's random state after these ticks of the first-level
/// route with three help screens.
const HELP_RNG: [(usize, u32); 12] = [
    (59, 3_554_967_446),
    (60, 286_940_986),
    (61, 2_497_464_375),
    (239, 2_018_578_452),
    (240, 80_550_201),
    (241, 80_550_201),
    (419, 3_637_059),
    (420, 227_955_959),
    (421, 3_302_636_510),
    (500, 3_040_703_919),
    (559, 348_926_273),
    (671, 2_319_028_964),
];

/// A first-level route: Claw drops a stick of dynamite at his feet three
/// times (rows 20, 200 and 380, three rows each), and 40 rows after each
/// the help screen is up for 500, 1000 and 2000 ms.
fn three_help_screens() -> String {
    let mut text = String::from("# level 1\ntick,held_mask,pressed_mask\n");
    let help = [(60, 500), (240, 1000), (420, 2000)];
    for t in 0..560 {
        if let Some((_, ms)) = help.iter().find(|(at, _)| *at == t) {
            text += &format!("@{t} menu help {ms}\n");
        }
        let tap = [20, 200, 380].iter().any(|&s| (s..s + 3).contains(&t));
        let first = [20, 200, 380].contains(&t);
        let held = if tap { 0x100 } else { 0 };
        let pressed = if first { 0x100 } else { 0 };
        text += &format!("{t},{held:#x},{pressed:#x}\n");
    }
    text
}

#[test]
fn install_a_timer_started_in_the_help_pass_is_done_at_once() {
    let Some(root) = install() else { return };
    let level = level(&root, 1);
    let mut run = Run::new(&level, &three_help_screens(), Some(4_105_512_405));
    run.to(671);
    run.check_rng(&HELP_RNG);
}

/// The original game's random state after these ticks of the second
/// earlier route.
const SECOND_RNG: [(usize, u32); 14] = [
    (1499, 1_379_831_121),
    (1510, 3_669_044_829),
    (1511, 3_817_579_161),
    (1512, 2_503_100_804),
    (1513, 3_867_919_270),
    (1534, 535_067_815),
    (1535, 2_788_267_004),
    (1575, 2_342_792_979),
    (1576, 3_524_982_215),
    (1577, 2_366_903_483),
    (1600, 1_933_017_861),
    (2000, 3_028_658_582),
    (2424, 1_901_403_666),
    (2536, 3_786_390_703),
];

/// Ticks the original run of the second earlier route lasted.
const SECOND_RUN_TICKS: usize = 2537;

/// Where the powder keg on the bridge high in the shaft stands, and where
/// the hook-handed pirate near the level's start waits.
const BRIDGE_KEG: (i32, i32) = (23001, 5587);
const START_PIRATE: (i32, i32) = (2343, 5996);

#[test]
fn install_a_kegs_blast_hurts_in_the_tick_it_goes_off() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    let mut run = Run::new(&level, &routes_file("earlier/level9-original-2.csv"), None);
    run.to(1498);
    let at_keg = |run: &Run| {
        run.sim
            .objects()
            .all()
            .any(|(_, o)| &*o.logic == "PowderKeg" && (o.x, o.y) == BRIDGE_KEG)
    };
    assert!(at_keg(&run), "the keg on the bridge");
    run.to(1511);
    assert_eq!(run.claw_at(), (23055, 5662));
    assert_eq!(run.sim.stats().health, 55);
    // The blast in tick 1512 hurts Claw in that tick, where he is.
    run.to(1512);
    assert_eq!(run.claw_at(), (23055, 5668));
    assert_eq!(run.sim.stats().health, 35);
    assert_eq!(run.sim.player().unwrap().state, 31, "hurt");
    run.to(1513);
    assert_eq!(run.claw_at(), (23055, 5668));
    assert_eq!(run.sim.stats().health, 35, "hurt once");
    run.check_rng(&SECOND_RNG);
}

#[test]
fn install_a_warps_second_pass_runs_every_object() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    // The route found again keeps the second earlier route's first 1535
    // ticks.
    let earlier = InputScript::parse(&routes_file("earlier/level9-original-2.csv")).unwrap();
    let now = InputScript::parse(&routes_file("level9-original.csv")).unwrap();
    assert_eq!(earlier.ticks[..1535], now.ticks[..1535]);
    assert_ne!(earlier.ticks[1535], now.ticks[1535]);
    let mut run = Run::new(&level, &routes_file("earlier/level9-original-2.csv"), None);
    run.to(1574);
    let pirate = run
        .sim
        .objects()
        .all()
        .find(|(_, o)| (o.x, o.y) == START_PIRATE)
        .map(|(_, o)| o.is_listed());
    assert_eq!(pirate, Some(false), "the pirate waits asleep");
    run.to(1575);
    assert_eq!(run.claw_at(), (23027, 6188), "on the warp below the bridge");
    run.to(1576);
    assert_eq!(run.claw_at(), (2171, 5904), "sent back near the start");
    let pirate = run
        .sim
        .objects()
        .all()
        .find(|(_, o)| (o.x, o.y) == START_PIRATE)
        .map(|(_, o)| o.is_listed());
    assert_eq!(pirate, Some(true), "the warp woke the pirate");
    run.check_rng(&SECOND_RNG);
    run.to(SECOND_RUN_TICKS - 1);
    assert_eq!(run.claw_at(), (3118, 7251));
    let stats = run.sim.stats();
    assert_eq!(
        (stats.health, stats.score, stats.lives),
        (35, 28900, 7),
        "no level end"
    );
    run.check_rng(&SECOND_RNG);
}

/// A first-level route of 300 rows with nothing held but the weapon
/// switch: pressed for one row in row 40, with the help screen up for
/// 500 ms before row 41, and pressed in row 120 and held through row 122,
/// with the help screen up for 500 ms before row 121.
fn switch_before_help() -> String {
    let mut text = String::from("# level 1\ntick,held_mask,pressed_mask\n");
    for t in 0..300 {
        if t == 41 || t == 121 {
            text += &format!("@{t} menu help 500\n");
        }
        let held = if t == 40 || (120..=122).contains(&t) {
            0x8
        } else {
            0
        };
        let pressed = if t == 40 || t == 120 { 0x8 } else { 0 };
        text += &format!("{t},{held:#x},{pressed:#x}\n");
    }
    text
}

/// The original game's random state after these ticks of
/// [`switch_before_help`].
const SWITCH_RNG: [(usize, u32); 8] = [
    (40, 382_094_739),
    (41, 3_141_103_828),
    (60, 4_042_785_042),
    (119, 4_272_996_913),
    (121, 1_044_143_152),
    (123, 1_652_443_699),
    (200, 3_837_821_437),
    (299, 1_539_293_145),
];

#[test]
fn install_a_key_pressed_before_a_help_screen_acts_once() {
    let Some(root) = install() else { return };
    let level = level(&root, 1);
    let mut run = Run::new(&level, &switch_before_help(), Some(4_105_512_405));
    let weapon = |run: &Run| claw_sim::TraceRow::capture(&run.sim, 0, "script").weapon;
    run.to(39);
    assert_eq!(weapon(&run), "pistol");
    run.to(40);
    assert_eq!(weapon(&run), "magic", "the press switches");
    // The help return's pass does not switch again.
    run.directives();
    assert_eq!(weapon(&run), "magic");
    run.to(41);
    assert_eq!(weapon(&run), "magic");
    run.to(119);
    assert_eq!(weapon(&run), "magic");
    run.to(120);
    assert_eq!(weapon(&run), "dynamite");
    // Held across the screen, the key is latched after it.
    for t in 121..=123 {
        run.to(t);
        assert_eq!(weapon(&run), "dynamite", "tick {t}");
    }
    run.to(299);
    assert_eq!(weapon(&run), "dynamite");
    run.check_rng(&SWITCH_RNG);
}

#[test]
fn install_the_route_found_again_ends_the_level_and_play_stays_stopped() {
    let Some(root) = install() else { return };
    let level = level(&root, 9);
    let text = routes_file("level9-original.csv");
    let script = InputScript::parse(&text).unwrap();
    let seed = script.header.seed.expect("the route names its seed");
    let mut sim = Sim::new(level, route::sim_config(TuningName::Original, seed));
    let mut end = None;
    for (t, &input) in script.ticks.iter().enumerate() {
        sim.play_directives(&script, t);
        sim.step(input);
        if sim.level_ended() {
            end = Some(t);
            break;
        }
    }
    assert_eq!(end, Some(2249), "the level's end");
    assert_eq!(sim.stats().health, 35);
    // A help screen after the level's end runs no pass: nothing moves and
    // the stream and the pulse stay as they were.
    let objects = |sim: &Sim| {
        sim.objects()
            .all()
            .map(|(_, o)| (o.serial, o.x, o.y, o.state, o.frame))
            .collect::<Vec<_>>()
    };
    let (rng, pulse, before) = (sim.rng_state(), sim.pulse(), objects(&sim));
    sim.pass_menu_time(3000);
    assert_eq!(sim.rng_state(), rng);
    assert_eq!(objects(&sim), before);
    sim.step(TickInput::default());
    assert_eq!(sim.pulse(), pulse);
    assert_eq!(objects(&sim), before);
}
