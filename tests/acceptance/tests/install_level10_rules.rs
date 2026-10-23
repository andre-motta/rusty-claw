//! Rules of the tenth level's play on your own install (skipped without
//! `CLAW_INSTALL`), each pinned to a moment of a route where the original
//! game showed it, by the state of the shared random stream after the
//! ticks around it, which equals the original's.
//!
//! The first two rules are pinned on `routes/earlier/level10-original.csv`,
//! the tenth level's original-speeds route as it was before them, played
//! from the level's start; `routes/earlier/level10-original-2.csv`, found
//! again under them, keeps its first 14799 ticks:
//!
//! - an enemy's move up or down that would take it past the top or bottom
//!   of its range puts it there, with the speed the energy of the move to
//!   it gives and no gravity added: the parrot carrying Marrow from tick
//!   14672 reaches the top of its range, its flight line at y 7029, and
//!   stays on it from tick 14717 to the carry's end, with Marrow hanging
//!   at y 7124, level with Claw jumping on the far ledge. Dropped at
//!   (40606, 7253) in tick 14798, Marrow goes idle in the next tick
//!   instead of shooting, and goes Home when Claw lands in tick 14817, making Home's
//!   draw in tick 14818;
//! - the respawn shows a frame of its own once its passes are over, which
//!   marks what it draws: Claw, without the bridge the earlier route
//!   counted on, dies on the pit's spikes in tick 15629 and respawns in
//!   tick 15798, where Marrow, put back on his ledge and drawn by that
//!   frame, sees him and goes Home in that tick, making Home's draws in
//!   tick 15799 (with the mark of the last frame before the death he
//!   would see him a tick later). The route then loses two more lives and
//!   ends without the level's end, as it did in the original game, and the
//!   stream stays the original's to the original run's last tick, 112
//!   ticks past the route's last row, with nothing held.
//!
//! The next three are pinned on `routes/earlier/level10-original-2.csv`,
//! played from the level's start; `routes/earlier/level10-original-3.csv`,
//! found again under them, keeps its first 17358 ticks:
//!
//! - the parrot keeps the point beside Marrow where it takes hold of him
//!   exact, to the part of a pixel, and takes hold of him once its exact
//!   x has passed that point's: Marrow, whistling from tick 17338, stops
//!   at x 41101.44, so the parrot, coming from the right, aims at x
//!   41043.44 and, at x 41043.249 after tick 17357, starts the carry in
//!   tick 17358 (with the point's whole pixel it would fly a tick longer).
//!   The bird reaches its flight line in tick 17412 and stays on it to the
//!   carry's end, Marrow hanging out of reach above Claw's ledge, and
//!   dropped at (40607, 7253) in tick 17482 he keeps his shot;
//! - standing, Marrow's part of a pixel creeps again from the end of each
//!   whistle, after a drop too: from tick 17354 his exact height is
//!   7253.324, 7253.648, 7253.972, 7253.000, so the carry aims at 7143.324;
//! - Claw's respawn removes every projectile before its passes: Claw dies
//!   in tick 18589, a shot of Marrow's is still flying at (40674, 7229)
//!   after tick 18683, and the respawn between ticks 18683 and 18684 takes
//!   it away, so Claw stands unhurt until Marrow's next shot hits him in
//!   tick 18755. The route ends without the level's end, as it did in the
//!   original game, and the stream stays the original's to the original
//!   run's last tick, 112 ticks past the route's last row, with nothing
//!   held.
//!
//! The sixth is pinned on `routes/earlier/level10-original-3.csv`, played
//! from the level's start; `routes/level10-original.csv`, found again
//! under it, keeps its first 17483 ticks:
//!
//! - an enemy's range boxes are centred on its exact position, which only
//!   its own movement step brings in line with where it is drawn, and the
//!   parrot's drop of Marrow moves only where he is drawn: the third carry
//!   ends in tick 17482 with Marrow dropped at (40607, 7253) and his exact
//!   position still (40607.334, 7125.475), where he hung. In tick 17483
//!   his outermost box is [40107, 41107) across and [7105, 7145) down, so
//!   Claw at (41106, 7237), a pixel inside the box built where Marrow
//!   landed, is in none: Marrow shoots, and his shot hits Claw in tick
//!   17542. Claw dies in tick 18103 and loses a life in tick 18197, and
//!   the route ends without the level's end, as it did in the original
//!   game, and the stream stays the original's to the original run's last
//!   tick, 112 ticks past the route's last row, with nothing held.

use std::path::Path;

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_frame::GameEvent;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::projectile::RESPAWN_PURGE;
use claw_sim::{InputScript, Sim, SimLevel, TickInput};

/// The original game's random state after these ticks of the earlier
/// route.
const EARLIER_RNG: [(usize, u32); 23] = [
    (14671, 3_143_434_138),
    (14672, 3_252_361_956),
    (14717, 957_116_821),
    (14720, 1_499_993_271),
    (14798, 1_243_285_915),
    (14799, 4_024_105_980),
    (14800, 1_333_304_015),
    (14816, 2_017_393_211),
    (14817, 1_083_333_650),
    (14818, 1_550_588_700),
    (15624, 3_781_115_327),
    (15628, 1_507_291_717),
    (15629, 1_070_148_110),
    (15797, 2_678_630_713),
    (15798, 4_189_030_582),
    (15799, 2_233_612_323),
    (15800, 4_099_437_476),
    (17123, 3_921_973_118),
    (17124, 3_292_272_049),
    (17817, 3_402_740_379),
    (17818, 2_326_550_721),
    (18566, 4_148_772_848),
    (18678, 1_560_528_909),
];

/// Ticks the original run of the earlier route lasted.
const EARLIER_RUN_TICKS: usize = 18679;

/// The original game's random state after these ticks of the second
/// earlier route.
const SECOND_RNG: [(usize, u32); 19] = [
    (17353, 270_338_638),
    (17354, 505_715_512),
    (17357, 3_483_353_970),
    (17358, 3_483_353_970),
    (17411, 3_688_787_958),
    (17412, 3_688_787_958),
    (17481, 810_869_068),
    (17482, 810_869_068),
    (17483, 3_560_792_403),
    (17484, 479_048_293),
    (17485, 1_695_505_300),
    (17532, 1_449_261_031),
    (18018, 4_274_414_505),
    (18589, 600_852_122),
    (18684, 1_357_501_034),
    (18699, 2_723_537_189),
    (18754, 1_973_303_692),
    (18755, 4_140_607_726),
    (18762, 707_039_080),
];

/// Ticks the original run of the second earlier route lasted.
const SECOND_RUN_TICKS: usize = 18763;

/// The original game's random state after these ticks of the third
/// earlier route.
const THIRD_RNG: [(usize, u32); 16] = [
    (17482, 3_491_473_489),
    (17483, 1_695_505_300),
    (17484, 3_264_603_822),
    (17485, 1_035_281_081),
    (17541, 529_053_023),
    (17542, 2_946_070_151),
    (18102, 2_634_399_624),
    (18103, 3_030_989_516),
    (18196, 1_926_441_405),
    (18197, 396_584_337),
    (18198, 3_876_216_379),
    (18386, 2_886_365_648),
    (18387, 2_179_900_056),
    (18851, 1_208_009_541),
    (18852, 3_127_800_807),
    (18964, 4_169_230_240),
];

/// Ticks the original run of the third earlier route lasted.
const THIRD_RUN_TICKS: usize = 18965;

/// A range box: left, top, right, bottom.
type RangeBox = (i64, i64, i64, i64);

/// The parrot's flight line: the top of its range, y.
const FLIGHT_LINE: i32 = 7029;

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

/// A route played tick by tick.
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
        let sim = Sim::new(level.clone(), route::sim_config(TuningName::Original, seed));
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
            self.sim.play_directives(&self.script, self.tick);
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

    fn check_rng(&self) {
        self.check(&EARLIER_RNG);
    }

    fn check(&self, table: &[(usize, u32)]) {
        for &(t, want) in table {
            if t < self.rng.len() {
                assert_eq!(self.rng[t], want, "random state after tick {t}");
            }
        }
    }

    /// Marrow's range data: the exact position his boxes are kept at
    /// after a drop, his range mode, and his outermost box from that
    /// position.
    fn marrow_range(&self) -> (Option<(f64, f64)>, u8, Option<RangeBox>) {
        let link = self.sim.marrow_link().expect("Marrow's link");
        let o = self.sim.objects().get(link.marrow).expect("Marrow");
        let d = enemy::data_of(o).expect("Marrow's data");
        let outer = d.range_from.map(|at| enemy::range_box(d, 0, at));
        (d.range_from, d.mode, outer)
    }

    /// Marrow: (x, y, state).
    fn marrow(&self) -> (i32, i32, i32) {
        let link = self.sim.marrow_link().expect("Marrow's link");
        let o = self.sim.objects().get(link.marrow).expect("Marrow");
        (o.x, o.y, o.state)
    }

    /// Marrow's exact height: his y and the part of a pixel his last
    /// movement step carried, when nothing else has moved him since.
    fn marrow_exact_y(&self) -> f64 {
        let link = self.sim.marrow_link().expect("Marrow's link");
        let o = self.sim.objects().get(link.marrow).expect("Marrow");
        let part = enemy::data_of(o)
            .filter(|d| d.moved_to == Some((o.x, o.y)))
            .map_or(0.0, |d| d.carry.1);
        f64::from(o.y) + part
    }

    /// The parrot: (x, y).
    fn parrot(&self) -> (i32, i32) {
        let o = self.parrot_object();
        (o.x, o.y)
    }

    fn parrot_object(&self) -> &claw_sim::object::Object {
        let mut parrots = self
            .sim
            .objects()
            .all()
            .filter(|(_, o)| &*o.logic == "Parrot")
            .map(|(_, o)| o);
        let p = parrots.next().expect("the parrot");
        assert!(parrots.next().is_none(), "one parrot");
        p
    }

    /// The parrot's state, the grab point it keeps exact and the target
    /// it keeps in whole pixels.
    fn parrot_flight(&self) -> (i32, (f64, f64), (i32, i32)) {
        let o = self.parrot_object();
        let ObjectData::Parrot(p) = &o.data else {
            panic!("the parrot's data")
        };
        (o.state, p.grab, p.target)
    }

    /// The projectiles about: (x, y) each.
    fn projectiles(&self) -> Vec<(i32, i32)> {
        self.sim
            .objects()
            .all()
            .filter(|(_, o)| o.special_flags & RESPAWN_PURGE != 0)
            .map(|(_, o)| (o.x, o.y))
            .collect()
    }

    fn claw_at(&self) -> (i32, i32) {
        let c = self.sim.player().expect("Claw");
        (c.x, c.y)
    }
}

#[test]
fn install_the_route_found_again_keeps_the_earlier_routes_start() {
    if install().is_none() {
        return;
    }
    let earlier = InputScript::parse(&routes_file("earlier/level10-original.csv")).unwrap();
    let now = InputScript::parse(&routes_file("earlier/level10-original-2.csv")).unwrap();
    assert_eq!(earlier.ticks[..14799], now.ticks[..14799]);
}

#[test]
fn install_the_route_found_again_keeps_the_second_earlier_routes_start() {
    if install().is_none() {
        return;
    }
    let earlier = InputScript::parse(&routes_file("earlier/level10-original-2.csv")).unwrap();
    let now = InputScript::parse(&routes_file("earlier/level10-original-3.csv")).unwrap();
    assert_eq!(earlier.ticks[..17358], now.ticks[..17358]);
}

#[test]
fn install_the_route_found_again_keeps_the_third_earlier_routes_start() {
    if install().is_none() {
        return;
    }
    let earlier = InputScript::parse(&routes_file("earlier/level10-original-3.csv")).unwrap();
    let now = InputScript::parse(&routes_file("level10-original.csv")).unwrap();
    assert_eq!(earlier.ticks[..17483], now.ticks[..17483]);
}

#[test]
fn install_marrow_dropped_keeps_his_range_boxes_where_he_hung() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original-3.csv"));
    run.to(17481);
    assert_eq!(run.marrow().1, 7125, "hanging under the parrot");
    assert_eq!(run.marrow_range().0, None);
    // The drop moves where he is drawn; his exact position stays where he
    // hung.
    run.to(17482);
    let (x, y, _) = run.marrow();
    assert_eq!((x, y), (40607, 7253));
    let (kept, _, outer) = run.marrow_range();
    let (kx, ky) = kept.expect("kept until his next movement step");
    assert!((kx - 40607.334).abs() < 0.0005, "x {kx}");
    assert!((ky - 7125.475).abs() < 0.0005, "y {ky}");
    assert_eq!(outer, Some((40107, 7105, 41107, 7145)));
    assert_eq!(run.claw_at(), (41106, 7237));
    // Claw is in none of the boxes built from there (a pixel inside the
    // outermost built where Marrow landed): off screen, mode 6, and he
    // shoots; his movement step brings his exact position to the ledge.
    run.to(17483);
    let (kept, mode, _) = run.marrow_range();
    assert_eq!(kept, None);
    assert_eq!(mode, 6);
    assert_eq!(run.marrow().2, EnemyState::Fire as i32);
    run.to(17484);
    assert_eq!(run.marrow().2, EnemyState::Fire as i32);
    run.check(&THIRD_RNG);
    // His shot hits Claw.
    run.to(17541);
    assert_eq!(run.sim.stats().health, 95);
    run.to(17542);
    assert_eq!(run.sim.stats().health, 75);
    run.to(18103);
    assert_eq!(run.sim.stats().health, 0);
    run.to(18196);
    assert_eq!(run.sim.stats().lives, 7);
    run.to(18197);
    assert_eq!(run.sim.stats().lives, 6);
    run.check(&THIRD_RNG);
    run.to(THIRD_RUN_TICKS - 1);
    assert_eq!(run.sim.stats().health, 20);
    assert_eq!(run.sim.stats().lives, 6);
    run.check(&THIRD_RNG);
}

#[test]
fn install_the_parrot_takes_hold_of_marrow_past_its_exact_grab_point() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original-2.csv"));
    run.to(17353);
    // His part of a pixel creeps from the whistle's end, after the drop
    // that ended the last carry too.
    let mut heights = Vec::new();
    for t in 17354..=17357 {
        run.to(t);
        heights.push((run.marrow_exact_y() * 1000.0).round() as i64);
    }
    assert_eq!(heights, [7_253_324, 7_253_648, 7_253_972, 7_253_000]);
    let (state, grab, target) = run.parrot_flight();
    assert_eq!(state, EnemyState::FlyToMarrow as i32);
    assert!((grab.0 - 41043.44).abs() < 0.005, "grab x {}", grab.0);
    assert_eq!(target.0, 41043);
    // The bird's exact x, 41043.249, is past the exact point but not past
    // its whole pixel: the carry starts now, aimed 110 px above Marrow's
    // exact height.
    run.to(17358);
    let (state, _, target) = run.parrot_flight();
    assert_eq!(state, EnemyState::CarryMarrow as i32);
    assert_eq!(target.1, 7143, "aimed at 7143.324");
    assert_eq!(run.parrot(), (41039, 7152));
    let (x, y, _) = run.marrow();
    assert_eq!((x, y), (41097, 7249));
    run.to(17411);
    assert!(run.parrot().1 > FLIGHT_LINE, "still rising");
    // On the line from tick 17412 to the carry's end, Marrow, a tick
    // behind it, 96 px under it.
    for t in 17412..=17481 {
        run.to(t);
        assert_eq!(run.parrot().1, FLIGHT_LINE, "the parrot after tick {t}");
        if t > 17412 {
            assert_eq!(run.marrow().1, 7125, "Marrow after tick {t}");
        }
    }
    // Dropped out of Claw's range, he keeps his shot.
    run.to(17482);
    let (x, y, _) = run.marrow();
    assert_eq!((x, y), (40607, 7253));
    run.to(17485);
    assert_eq!(run.marrow().2, EnemyState::Fire as i32);
    run.check(&SECOND_RNG);
    // The shot hits Claw.
    run.to(17532);
    assert_eq!(run.sim.stats().health, 75);
    run.check(&SECOND_RNG);
}

#[test]
fn install_claws_respawn_removes_the_shot_still_flying() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original-2.csv"));
    run.to(18018);
    assert_eq!(run.sim.stats().lives, 6);
    run.to(18589);
    assert_eq!(run.sim.stats().health, 0);
    run.to(18683);
    assert_eq!(run.sim.stats().lives, 5);
    assert!(
        run.projectiles().contains(&(40674, 7229)),
        "Marrow's shot flying: {:?}",
        run.projectiles()
    );
    run.to(18684);
    assert_eq!(run.claw_at(), (40488, 7245));
    assert!(
        !run.projectiles().iter().any(|&(_, y)| y == 7229),
        "the shot is gone: {:?}",
        run.projectiles()
    );
    for t in 18685..=18754 {
        run.to(t);
        assert_eq!(run.sim.stats().health, 100, "unhurt after tick {t}");
    }
    run.to(18755);
    assert_eq!(run.sim.stats().health, 80);
    assert_eq!(run.claw_at(), (40488, 7245));
    assert_eq!(run.sim.player().expect("Claw").state, 31);
    run.to(SECOND_RUN_TICKS - 1);
    assert_eq!(run.sim.stats().lives, 5);
    run.check(&SECOND_RNG);
}

#[test]
fn install_the_parrot_carries_marrow_on_its_flight_line() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(14716);
    assert!(run.parrot().1 > FLIGHT_LINE, "still rising");
    // On the line from tick 14717 to the carry's end, Marrow 95 px under
    // it; with gravity added at the line the bird would come down again
    // some twenty ticks later, and Marrow with it.
    for t in 14717..=14797 {
        run.to(t);
        assert_eq!(run.parrot().1, FLIGHT_LINE, "the parrot after tick {t}");
        assert_eq!(run.marrow().1, 7124, "Marrow after tick {t}");
    }
    assert_eq!(run.parrot(), (40551, FLIGHT_LINE));
    // Dropped, he is out of range of Claw again: he goes idle, not into
    // his shot, and stays idle until Claw lands, when he goes Home.
    run.to(14798);
    let (x, y, _) = run.marrow();
    assert_eq!((x, y), (40606, 7253));
    for t in 14799..=14816 {
        run.to(t);
        assert_eq!(run.marrow().2, EnemyState::Idle as i32, "after tick {t}");
    }
    run.to(14817);
    assert_eq!(run.marrow().2, EnemyState::Home as i32);
    run.to(14818);
    run.check_rng();
}

#[test]
fn install_the_respawn_shows_a_frame_of_its_own() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(15628);
    assert_eq!(run.sim.stats().lives, 7);
    run.to(15629);
    assert_eq!(run.sim.stats().health, 0, "on the pit's spikes");
    run.to(15797);
    assert_eq!(run.sim.stats().lives, 6);
    run.to(15798);
    assert_eq!(run.claw_at(), (40492, 7245));
    // Drawn by the respawn's frame, Marrow sees Claw at once and goes Home.
    let (x, y, st) = run.marrow();
    assert_eq!((x, y), (41063, 7253));
    assert_eq!(st, EnemyState::Home as i32);
    run.to(15800);
    run.check_rng();
}

#[test]
fn install_the_earlier_route_follows_the_original_to_its_runs_end() {
    let Some(root) = install() else { return };
    let level = level(&root, 10);
    let mut run = Run::new(&level, &routes_file("earlier/level10-original.csv"));
    run.to(17123);
    assert_eq!(run.sim.stats().lives, 5);
    run.to(17817);
    assert_eq!(run.sim.stats().lives, 4);
    run.to(EARLIER_RUN_TICKS - 1);
    assert_eq!(run.claw_at(), (40681, 7404));
    assert_eq!(run.sim.stats().score, 177_700);
    run.check_rng();
}
