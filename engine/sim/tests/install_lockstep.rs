//! The first level of your own install, played tick by tick along the bot
//! route the original was run with at one 18 ms step per frame and the
//! original speeds (skipped without `CLAW_INSTALL`): at the tick where
//! each of these rules first shows on that route, the game does what the
//! original did there.
//!
//! 1. A goodie spins through its set while it flies and becomes
//!    collectable with the box of the frame it shows then: the coin an
//!    officer drops in tick 829 (sideways at 163 px/s) is armed in tick
//!    836 with an edge-on frame 8 px wide, and Claw takes it in tick 853
//!    (15372 ms), not 844.
//! 2. A goodie moves on an exact position, one tick of constant
//!    acceleration at a time: in tick 830 the coin dropped at 3403,4355
//!    moving left at 37 px/s is at 3402,4347.
//! 3. A goodie moves with its first picture's box grown 10 px left, up and
//!    right, and a wall on either side turns it around at half speed: the
//!    gecko the officer at 5650,4419 drops stays at x 5650 and flies back
//!    at 87.5 px/s (tick 1629), and Claw takes it in tick 1635 (score
//!    23200).
//! 4. Each call a goodie first acts on what its previous move left: the
//!    health goodie that lands for the fourth time in tick 4770 bounces in
//!    tick 4771 and turns into a pickup in tick 4772, whose glitter draws
//!    its delay in tick 4773.
//! 5. A save point leaves the random stream at the save's own state: after
//!    tick 2024 (36450 ms) it is 1695759300.
//! 6. The ambient sounds' timers count no time in the tick of a save: the
//!    first flip after it comes in tick 2212, not 2211.
//! 7. A human enemy placed outside its patrol limits starts on the nearer
//!    limit, on the floor below it: the soldier placed at 11855,2558 (limits
//!    11794 to 11800) is at 11800,2628 when it wakes in tick 4313, and it
//!    draws nothing in tick 4314.
//! 8. A lying keg sorts its depth against Claw's feet like the enemies:
//!    in tick 4367 the kegs near Claw are at depth 3919 and roll before
//!    him.
//! 9. Standing, Claw starts a walk on the press of Right or Left, and an
//!    attack's end or cancel makes a direction still held count as pressed
//!    on the next tick: Right pressed in tick 4686 during a swipe walks
//!    only in tick 4695, after the swipe's end.
//! 10. A lost life with lives left ends the tick with Claw dead and he
//!     respawns before the next tick: dead on the death tile in tick 5388
//!     (lives 6), at the checkpoint 14223,2298 in tick 5389, walking with
//!     full health.
//!
//! The random stream equals the original's after every tick checked here
//! (the reference trace of the same route holds it for every tick; see
//! `tests/acceptance/tolerances/references.toml`).

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::GameEvent;
use claw_sim::logic::pickup;
use claw_sim::object::ObjectId;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, Tuning};

/// The random state the reference runs of the first level start from.
const RNG_AT_T0: u32 = 4_105_512_405;

/// The route's first 5400 ticks: held masks in hex, each with its run of
/// ticks.
const ROUTE: &str = "
    0:2 2000001:14 2000000:2 0:6 2000000:6 0:6 2000000:6 2000001:14 2000000:32 2000001:14
    2000000:26 2000001:14 2000000:26 2000001:14 2000000:14 4000000:6 2000001:14 2000000:2
    1000000:12 4000000:6 2000001:14 2000000:2 1000000:12 4000000:6 2000001:5 2000000:2
    1000000:6 4000000:6 2000001:14 2000000:14 2000001:14 2000000:26 2000001:14 2000000:26
    2000001:14 2000000:26 2000001:14 2000000:20 0:6 1:14 0:2 2000000:24 2000001:9 2000000:20
    2000001:9 2000000:8 0:6 2000000:6 2000001:9 2000000:56 2000001:9 2000000:38 8000000:6
    2000001:2 2000000:2 2000001:9 2000000:98 8000000:6 2000000:6 8000000:6 2000001:2 2000000:2
    2:2 0:6 2000000:6 2000001:5 2000000:14 0:6 2000001:2 2000000:2 0:18 2000001:9 2000000:26
    1000001:9 1000000:2 2000000:6 0:12 2000001:14 2000000:2 1000000:18 2000000:6 8000000:6
    1:14 0:2 2000001:14 2000000:8 2:2 0:18 2000001:9 2000000:26 2000001:14 2000000:14 0:6
    2000001:2 2000000:14 2:2 0:6 2000000:6 8000000:6 2000001:14 2000000:2 2000001:14
    2000000:20 2000001:2 2000000:20 2000001:14 2000000:26 2000001:14 2000000:8 1000000:6
    2000000:6 2000001:14 2000000:20 8000000:6 2000001:14 2000000:2 2000001:2 2000000:86
    4000000:6 8000000:42 2000001:14 2000000:20 0:6 8000000:6 8000002:2 8000000:6 2000000:48
    2000001:9 2000000:8 0:6 2:2 0:6 2000000:6 2000001:9 2000000:14 2000001:2 2000000:62 0:6
    1000000:6 2000000:6 0:12 8000002:2 8000000:6 1:14 0:2 8000000:6 1:14 0:8 2:2 0:12
    8000000:6 1:14 0:2 2000001:14 2000000:14 4000000:6 2000001:14 2000000:2 0:6 1000000:6
    2000001:2 2000000:68 2000001:2 2000000:44 0:12 2000000:6 2000001:14 2000000:14 2000001:9
    2000000:20 2000001:14 2000000:2 1000000:6 4000000:6 2000001:14 2000000:2 1000000:12
    4000000:6 2000001:14 2000000:2 1000000:12 4000000:6 2000001:5 2000000:2 1000000:6
    4000000:6 2000001:14 2000000:20 2000001:14 2000000:20 0:6 2000001:14 2000000:32 2000001:14
    2000000:26 2000001:14 2000000:32 2000001:14 2000000:14 0:6 2000000:6 1:14 0:2 2000000:24
    2000001:14 2000000:2 0:6 4000000:6 2000001:5 2000000:2 4000000:6 2000001:14 2000000:20
    2000001:14 2000000:20 2:2 0:6 2000001:14 2000000:14 0:6 2000000:6 2000001:14 2000000:32
    2000001:14 2000000:26 2000001:14 2000000:14 0:6 2000000:6 0:18 2000000:12 2000001:14
    2000000:14 0:6 2000000:6 2000001:14 2000000:14 0:12 2000001:14 2000000:14 0:6 1000000:6
    2000001:14 2000000:8 4000000:6 2000001:14 2000000:2 1000000:12 4000000:6 2000001:14
    2000000:2 1000000:6 4000000:6 2000001:5 2000000:2 1000000:6 4000000:6 2000001:14
    2000000:14 1000000:6 0:6 1000001:14 1000000:2 2000000:18 2:2 0:6 1000001:14 1000000:26 1:9
    0:14 2:2 0:6 1:14 0:2 1000000:6 2000000:6 8000000:6 1:2 0:2 1:14 0:2 2:2 0:24 1:14 0:8
    2000000:6 1000000:6 1:14 0:20 1:14 0:14 2:2 0:6 2000001:14 2000000:2 4000000:6 1000001:14
    1000000:2 2000000:6 4000000:6 2000001:14 2000000:2 1000000:6 4000000:6 2000001:14
    2000000:2 1000000:12 4000000:6 2000001:14 2000000:14 2000001:14 2000000:14 0:6 2000000:6
    1:14 0:2 2000000:24 2000001:14 2000000:26 2000001:14 2000000:26 2000001:14 2000000:20
    2000001:14 2000000:2 0:6 4000000:6 2000001:5 2000000:2 4000000:6 2000001:14 2000000:2 0:6
    1000000:12 2000001:14 2000000:20 2000001:14 2000000:26 2000001:14 2000000:8 2:2 0:6
    2000000:6 2000001:14 2000000:26 2000001:14 2000000:32 2000001:14 2000000:20 0:6 2000001:14
    2000000:20 0:6 2000001:14 2000000:14 0:6 2000000:6 2000001:14 2000000:26 2000001:14
    2000000:26 2000001:14 2000000:20 0:6 2000000:6 2000001:14 2000000:14 0:12 2000000:6
    2000001:14 2000000:32 2000001:14 2000000:26 2000001:14 2000000:14 2000001:14 2000000:20
    2000001:14 2000000:20 2000001:14 2000000:26 2:2 0:6 2000000:36 0:6 2000000:6 2:2 0:6
    2000000:120 2:2 0:6 2000000:72 2:2 0:12 1000000:6 2000000:6 2:2 0:6 2000000:30 2000001:9
    2000000:20 2000001:14 2000000:14 2:2 0:6 2000000:18 2:2 0:12 2000000:12 1:14 0:2
    2000000:24 2000001:9 2000000:20 2000001:14 2000000:20 2000001:14 2000000:20 2000001:14
    2000000:20 2000001:14 2000000:20 2000001:14 2000000:14 1000000:6 2000000:6 2000001:14
    2000000:26 2000001:5 2000000:20 2000001:14 2000000:8 4000000:6 1000001:14 1000000:2
    2000000:12 4000000:6 1000001:14 1000000:2 2000000:12 4000000:6 1000001:14 1000000:2 0:6
    2000000:6 0:6 1000001:14 1000000:15
";

/// The original's random state after these ticks.
const RNG: [(usize, u32); 22] = [
    (829, 2_891_961_556),
    (830, 267_573_051),
    (844, 334_421_555),
    (853, 2_784_255_247),
    (1629, 2_123_492_155),
    (1635, 1_214_445_975),
    (2023, 2_346_115_611),
    (2024, 1_695_759_300),
    (2211, 1_524_239_188),
    (2212, 4_237_741_255),
    (4313, 473_672_135),
    (4314, 473_672_135),
    (4366, 3_373_960_567),
    (4367, 1_115_569_017),
    (4593, 451_718_032),
    (4678, 4_130_836_784),
    (4695, 2_724_293_419),
    (4771, 530_431_621),
    (4772, 530_431_621),
    (4773, 3_279_402_804),
    (5388, 476_992_824),
    (5389, 3_185_022_844),
];

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

fn route() -> InputScript {
    let mut held = Vec::new();
    for run in ROUTE.split_whitespace() {
        let (mask, n) = run.split_once(':').expect("mask:ticks");
        let mask = u32::from_str_radix(mask, 16).expect("hex mask");
        let n: usize = n.parse().expect("ticks");
        held.extend(std::iter::repeat_n(mask, n));
    }
    assert_eq!(held.len(), 5400);
    InputScript::from_held(&held)
}

/// The flying goodie moving across at `vx` px/s, if any.
fn goodie(sim: &Sim, vx: f64) -> Option<ObjectId> {
    sim.objects().all().find_map(|(id, o)| {
        let g = pickup::data_of(o)?.goodie?;
        (&*o.logic == pickup::BOUNCING_GOODIE && g.vx == vx).then_some(id)
    })
}

/// The object the level placed at `at` with logic `logic`.
fn placed(sim: &Sim, logic: &str, at: (i32, i32)) -> Option<ObjectId> {
    sim.objects().all().find_map(|(id, o)| {
        let r = o.record.as_ref()?;
        (&*o.logic == logic && (r[5], r[6]) == at).then_some(id)
    })
}

fn count(sim: &Sim, logic: &str) -> usize {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .count()
}

#[test]
fn install_level1_follows_the_original_at_each_rule() {
    let Some(level) = level1() else { return };
    let script = route();
    let cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        seed: RNG_AT_T0,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    let mut coin = None;
    let mut health = None;
    let mut glitters = 0;
    let mut ended = false;
    for (tick, t) in script.ticks.iter().enumerate() {
        let snap = sim.step(*t);
        let claw = sim.player().expect("Claw").clone();
        let score = sim.stats().score;
        if let Some(&(_, want)) = RNG.iter().find(|r| r.0 == tick) {
            assert_eq!(sim.rng_state(), want, "random state after tick {tick}");
        }
        match tick {
            // Rule 2: the coin moving left at 37 px/s, one exact step on.
            830 => {
                let id = goodie(&sim, -37.0).expect("the left coin");
                let o = sim.objects().get(id).unwrap();
                assert_eq!((o.x, o.y), (3402, 4347));
                coin = goodie(&sim, 163.0);
                assert!(coin.is_some(), "the right coin flies");
            }
            // Rule 1: armed with its edge-on frame's box.
            836 => {
                let o = sim.objects().get(coin.unwrap()).unwrap();
                let r = o.attack_rect;
                assert_eq!((r.left, r.top, r.right, r.bottom), (-4, -24, 3, 13));
            }
            844..=852 => assert_eq!(score, 11_000, "tick {tick}: the coin is not taken yet"),
            853 => assert_eq!(score, 11_100, "the coin is taken in tick 853"),
            // Rule 3: the gecko stopped by the wall flies back at half speed.
            1629 => {
                let id = goodie(&sim, -87.5).expect("the gecko turned around");
                assert_eq!(sim.objects().get(id).unwrap().x, 5650);
            }
            1635 => assert_eq!(score, 23_200, "the gecko and a coin are taken"),
            // Rule 7: on its limit, on the floor below it.
            4313 => {
                let id = placed(&sim, "Soldier", (11855, 2558)).expect("the soldier");
                let o = sim.objects().get(id).unwrap();
                assert_eq!((o.x, o.y), (11800, 2628));
            }
            // Rule 8: the kegs near Claw sort behind him.
            4367 => {
                let near: Vec<i32> = sim
                    .objects()
                    .listed()
                    .into_iter()
                    .filter_map(|id| sim.objects().get(id))
                    .filter(|o| &*o.logic == "PowderKeg" && (o.x - claw.x).abs() < 400)
                    .map(|o| o.z)
                    .collect();
                assert!(!near.is_empty(), "kegs near Claw");
                assert!(near.iter().all(|&z| z == 3919), "{near:?}");
            }
            // Rule 9: standing through the swipe, walking after its end.
            4678..=4694 => assert_eq!((claw.x, claw.y), (13714, 1722), "tick {tick}"),
            4695 => assert_eq!(claw.x, 13718, "walks after the swipe's end"),
            // Rule 4: the health goodie turns into a pickup one call after
            // its fourth bounce.
            4771 => {
                health = sim.objects().all().find_map(|(id, o)| {
                    (&*o.logic == pickup::BOUNCING_GOODIE
                        && pickup::data_of(o)?.goodie?.bounces == 4)
                        .then_some(id)
                });
                assert!(health.is_some(), "the goodie bounced four times");
                glitters = count(&sim, pickup::GLITTER);
            }
            4772 => {
                let o = sim.objects().get(health.unwrap()).unwrap();
                assert_eq!(&*o.logic, "HealthPowerup");
                assert_eq!(count(&sim, pickup::GLITTER), glitters + 1);
            }
            // Rule 10: dead at the end of the tick, back before the next.
            5388 => {
                assert_eq!((claw.x, claw.y, claw.state), (15189, 3728, 5005));
                assert_eq!(sim.stats().lives, 6);
                assert!(snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }));
                assert!(sim.respawn_pending());
            }
            5389 => {
                assert_eq!((claw.x, claw.y, claw.state), (14223, 2298, 9));
                assert_eq!(sim.stats().health, 100);
                assert!(sim.transition().is_none(), "the circles take no game time");
                ended = true;
                break;
            }
            _ => {}
        }
    }
    assert!(ended);
}
