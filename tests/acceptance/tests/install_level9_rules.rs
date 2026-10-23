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
