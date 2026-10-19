//! The framework the thirteenth level's families build on: RedTail, his
//! stakes, his wind, his knives, his flares and the wind's debris are
//! declared, and each stays an inert placeholder until its family is
//! built; the Chameleon and the water rock run; the boss's and the
//! lizard's classes, data and state numbers; the RedTail link with the
//! wind flag, and the seams his fight builds on (the wind's push on Claw,
//! the count of Claw's blows starting at 1, the boss health set-up of a
//! measuring route); the level's platform, springboard and water table;
//! and, on the install, the level's inert census, the art the new
//! families need loaded with it, and what the earlier families do with the
//! level (the arena's toggle pegs, the water that kills, the waterfall
//! pieces with the large window).
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here. The free numbers of the thirteenth level's block
//! of state numbers stay free for its families to take.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset. Nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::{DrawMode, GameEvent, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss::{debris, flare, knife, spikes, wind};
use claw_sim::logic::enemy::chameleon::{self, PhaseFrame};
use claw_sim::logic::enemy::redtail::{self, RedTailLink};
use claw_sim::logic::enemy::{self, EnemyState, goo};
use claw_sim::logic::platform::{self, SurfaceKind, state as peg_state};
use claw_sim::logic::{self, GameLogic, ObjectData, springboard};
use claw_sim::object::{LogicKind, Rect, draw_flags, flags};
use claw_sim::player::state as claw_state;
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The families declared for the thirteenth level, with the names they
/// claim.
const DECLARED: [(GameLogic, &[&str]); 8] = [
    (GameLogic::RedTail, &["RedTail"]),
    (GameLogic::RedTailSpikes, &["RedTailSpikes"]),
    (GameLogic::RedTailWind, &["RedTailWind"]),
    (GameLogic::RedTailKnife, &["RedTailKnife"]),
    (GameLogic::RedTailBullet, &["RedTailBullet"]),
    (GameLogic::WindDebris, &["WindDebris"]),
    (GameLogic::Chameleon, &["Chameleon"]),
    (GameLogic::SpringBoard, &["SpringBoard", "WaterRock"]),
];

/// Logics the thirteenth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration, platform,
/// pickup, warp, checkpoint, sound object, trap and enemy of the earlier
/// levels the level places already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each name of the new families the thirteenth level
/// places.
const PLACED: [(&str, usize); 6] = [
    ("RedTail", 1),
    ("RedTailSpikes", 1),
    ("RedTailWind", 1),
    ("Chameleon", 54),
    ("WaterRock", 11),
    ("AniCycleNormal", 75),
];

/// The flat level with `names` placed beside Claw.
fn level_with(names: &[&str]) -> SimLevel {
    let mut level = flat_level();
    for (i, name) in names.iter().enumerate() {
        level
            .objects
            .push(ObjectSpec::at(name, 1100 + 40 * i as i32, 900, 4000));
    }
    level
}

/// Whether the family claiming `name` is built.
fn built(name: &str) -> bool {
    match logic::kind_for(name) {
        Some(LogicKind::Game(g)) => logic::built(g),
        _ => false,
    }
}

#[test]
fn every_name_belongs_to_one_family() {
    let mut seen = BTreeSet::new();
    for g in logic::ALL {
        for &n in logic::names(g) {
            assert!(seen.insert(n), "{n} is claimed twice");
            assert_eq!(logic::kind_for(n), Some(LogicKind::Game(g)), "{n}");
            assert!(Registry::shared().get(n).is_some(), "{n} is registered");
        }
    }
    for (g, names) in DECLARED {
        assert!(logic::ALL.contains(&g), "{g:?}");
        assert_eq!(logic::names(g), names, "{g:?}");
    }
    // The families' own constants agree with the registry.
    assert_eq!(logic::built(GameLogic::RedTail), redtail::BUILT);
    assert_eq!(logic::built(GameLogic::RedTailSpikes), spikes::BUILT);
    assert_eq!(logic::built(GameLogic::RedTailWind), wind::BUILT);
    assert_eq!(logic::built(GameLogic::RedTailKnife), knife::BUILT);
    assert_eq!(logic::built(GameLogic::RedTailBullet), flare::BUILT);
    assert_eq!(logic::built(GameLogic::WindDebris), debris::BUILT);
    assert_eq!(logic::built(GameLogic::Chameleon), chameleon::BUILT);
    // The Chameleon and the water rock run; the waterfall pieces with the
    // large window are the built decoration.
    const { assert!(chameleon::BUILT && springboard::BUILT) };
    for (name, g) in [
        ("Chameleon", GameLogic::Chameleon),
        ("WaterRock", GameLogic::SpringBoard),
        ("AniCycleNormal", GameLogic::Decor),
    ] {
        assert_eq!(
            Registry::shared().get(name).unwrap().kind,
            LogicKind::Game(g),
            "{name}"
        );
    }
}

#[test]
fn declared_families_stay_inert_placeholders_until_built() {
    let unbuilt: Vec<_> = DECLARED.iter().filter(|(g, _)| !logic::built(*g)).collect();
    for &&(_, list) in &unbuilt {
        for &n in list {
            assert_eq!(logic::runs(n), None, "{n}");
            assert_eq!(Registry::shared().get(n).unwrap().kind, LogicKind::Inert);
            assert_eq!(logic::new_object(n, 0, 0, 0).kind, LogicKind::Inert);
        }
    }
    for (g, list) in DECLARED.iter().filter(|(g, _)| logic::built(*g)) {
        for &n in *list {
            assert_eq!(logic::runs(n), Some(LogicKind::Game(*g)), "{n}");
        }
    }
    // Placed in a level, the unbuilt ones are drawn placeholders that never
    // collide, and the level load reports them as not implemented.
    let names: Vec<&str> = ["RedTail", "RedTailSpikes", "RedTailWind"]
        .into_iter()
        .filter(|n| !built(n))
        .collect();
    let level = level_with(&names);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    idle(&mut sim, 5);
    let d = sim.diagnostics();
    for n in &names {
        assert_eq!(d.inert_logics.get(*n), Some(&1), "{n} is reported");
    }
    let placed: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.record.is_some())
        .collect();
    assert_eq!(placed.len(), names.len());
    for (_, o) in placed {
        assert_eq!(o.kind, LogicKind::Inert, "{}", o.logic);
        assert_eq!(o.state, 0, "{} never runs", o.logic);
        assert_eq!(o.data, ObjectData::None);
    }
    assert!(sim.collision_events().is_empty());
    // Unbuilt, nothing writes the link.
    if !redtail::BUILT {
        assert_eq!(*sim.redtail_link(), RedTailLink::default());
    }
}

#[test]
fn the_registry_keeps_each_names_activation_class() {
    // RedTail, his wind and the water rocks wake with the large window, the
    // stakes with the small one, the lizards, knives, flares, debris and
    // the waterfall sheets with the enemies'.
    for (name, class) in [
        ("RedTail", 1),
        ("RedTailWind", 1),
        ("RedTailSpikes", 2),
        ("RedTailKnife", 0),
        ("RedTailBullet", 0),
        ("WindDebris", 0),
        ("Chameleon", 0),
        ("WaterRock", 1),
        ("AniCycleNormal", 0),
        ("AniCycle", 2),
    ] {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
    }
}

#[test]
// The data literals below end in `..Default::default()` so they keep
// compiling when the families gain fields of their own.
#[allow(clippy::needless_update)]
fn redtail_and_the_chameleon_are_human_enemies() {
    let class = enemy::class(GameLogic::RedTail).expect("a class");
    assert_eq!(class.name, "RedTail");
    assert!(class.human, "a human enemy");
    assert!(class.carry.is_none(), "never lifted");
    let class = enemy::class(GameLogic::Chameleon).expect("a class");
    assert_eq!(class.name, "Chameleon");
    assert!(class.human, "a human enemy");
    assert!(class.carry.is_none(), "never lifted");
    for g in [
        GameLogic::RedTailSpikes,
        GameLogic::RedTailWind,
        GameLogic::RedTailKnife,
        GameLogic::RedTailBullet,
        GameLogic::WindDebris,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?}");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::RedTail(redtail::RedTailData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Chameleon(chameleon::ChameleonData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::RedTailSpikes(Default::default()),
        ObjectData::RedTailWind(Default::default()),
        ObjectData::RedTailKnife(Default::default()),
        ObjectData::RedTailBullet(Default::default()),
        ObjectData::WindDebris(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Their numbers.
    assert_eq!((redtail::HEALTH, redtail::DAMAGE), (100, 10));
    assert_eq!(redtail::BLOW_DAMAGE, 5);
    assert_eq!(redtail::HIT_RECT, Rect::new(-31, -51, 38, 62));
    assert_eq!(spikes::DAMAGE, 20);
    assert_eq!((knife::DAMAGE, flare::DAMAGE), (5, 15));
    assert_eq!((chameleon::HEALTH, chameleon::DAMAGE), (1, 10));
    assert_eq!(chameleon::BODY_RECT, Rect::new(-27, -30, 64, 25));
}

#[test]
fn the_thirteenth_levels_state_numbers_hold_redtail_and_the_chameleon() {
    let block = redtail::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2300, 2319));
    let named = [
        (EnemyState::KnifeAttack, 2300, "KnifeAttack"),
        (EnemyState::KickAttack, 2301, "KickAttack"),
        (EnemyState::SlideAttack, 2302, "SlideAttack"),
        (EnemyState::GunAttack, 2303, "GunAttack"),
        (EnemyState::WindAttack, 2304, "WindAttack"),
        (EnemyState::JumpUp, 2305, "JumpUp"),
        (EnemyState::JumpDown, 2306, "JumpDown"),
        (EnemyState::PhaseOut, 2310, "PhaseOut"),
        (EnemyState::PhaseIn, 2311, "PhaseIn"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(
        redtail::STATES,
        named[..7].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    assert_eq!(
        chameleon::STATES,
        named[7..].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    // The free numbers of the block stay free.
    for id in (2307..=2309).chain(2312..=2319) {
        assert_eq!(EnemyState::from_id(id), None, "{id}");
    }
    // The twelfth level's block ends where the thirteenth's starts.
    assert_eq!(
        *enemy::aquatis::STATE_BLOCK.end() + 1,
        *block.start(),
        "blocks of 20"
    );
    // The shared states they reuse keep their numbers.
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::FastAdvance as i32, 2010);
    assert_eq!(EnemyState::Block as i32, 2141);
    assert_eq!(EnemyState::Dead as i32, 2006);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::Pace as i32, 2003);
}

#[test]
fn the_link_starts_cleared_lasts_and_is_copied_with_the_game() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(*sim.redtail_link(), RedTailLink::default());
    assert_eq!(sim.chameleon_frame(), PhaseFrame::default());
    sim.with_world(|w| {
        assert_eq!(w.redtail_link().redtail, None);
        assert!(!w.redtail_link().wind);
        w.redtail_link_mut().wind = true;
        w.set_chameleon_phase(true, 120);
    });
    idle(&mut sim, 3);
    let copy = sim.clone();
    assert!(copy.redtail_link().wind);
    assert_eq!(
        copy.chameleon_frame(),
        PhaseFrame {
            blend: true,
            amount: 120
        }
    );
}

#[test]
fn the_wind_pushes_claw_as_a_conveyor_belt_does() {
    // The seam the built wind writes through: from the next tick a
    // standing Claw drifts 5 px a tick to the left at the 18 ms step.
    let mut sim = Sim::new(
        flat_level(),
        SimConfig {
            tuning: claw_sim::input::ScriptTuning::Original.tuning(),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 30);
    let x0 = sim.player().unwrap().x;
    let mut xs = Vec::new();
    for _ in 0..4 {
        sim.with_world(|w| wind::push_claw(w, true));
        sim.step(TickInput::default());
        xs.push(sim.player().unwrap().x);
    }
    assert_eq!(xs, [x0 - 5, x0 - 10, x0 - 15, x0 - 20]);
    // Outside the box the push is 0.
    sim.with_world(|w| wind::push_claw(w, false));
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().x, x0 - 20);
}

#[test]
fn a_chameleon_fades_out_and_back_on_a_flat_floor() {
    // A lizard 150 to 200 px from Claw (range mode 2): within a few
    // seconds it tries to phase, fades out in 26 calls, hidden and out of
    // pair collision, its phase picture blended with a falling amount, and
    // comes back drawn and touchable.
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at(chameleon::NAMES[0], 1180, 900, 0));
    for seed in 1..=20 {
        let mut sim = Sim::new(
            level.clone(),
            SimConfig {
                seed,
                ..SimConfig::default()
            },
        );
        let id = |sim: &Sim| {
            sim.objects()
                .all()
                .find(|(_, o)| &*o.logic == chameleon::NAMES[0])
                .map(|(id, _)| id)
                .unwrap()
        };
        let mut out = None;
        let mut amounts = Vec::new();
        for k in 0..2000 {
            sim.step(TickInput::default());
            let o = sim.objects().get(id(&sim)).unwrap();
            if o.state == EnemyState::PhaseOut as i32 {
                out.get_or_insert(k);
                assert!(o.flags & flags::NO_COLLIDE != 0);
                assert_eq!(o.frame, chameleon::PHASE_FRAME);
                assert!(sim.chameleon_frame().blend);
                amounts.push(sim.chameleon_frame().amount);
                let r = sim
                    .snapshot()
                    .renderables
                    .into_iter()
                    .find(|r| r.image_set_key == chameleon::IMAGE_SET)
                    .expect("drawn while fading out");
                assert_eq!(r.draw_mode, DrawMode::BLEND);
            }
            if out.is_some() && o.state == EnemyState::Home as i32 {
                assert!(o.flags & flags::NO_COLLIDE == 0);
                assert!(o.draw_flags & draw_flags::HIDDEN == 0);
                assert!(!sim.chameleon_frame().blend);
                break;
            }
        }
        if out.is_some() {
            // The picture keeps its default amount for the entry tick's
            // drawing; then 245 down to 5, and the jump.
            let mut want = vec![DrawMode::DEFAULT_COLOR];
            want.extend((0..25).map(|i| (245 - 10 * i) as u8));
            assert_eq!(amounts, want, "seed {seed}");
            return;
        }
    }
    panic!("no lizard phased in twenty games");
}

#[test]
fn an_unbounded_chameleon_is_carried_to_the_edge_of_a_floor_that_reaches_it() {
    // A lizard with no patrol limits on a floor that runs to the level's
    // left edge: its phase's far-left target takes it to the edge and no
    // further, at once; one with a wall at its back stays where it is,
    // the target becoming its left limit.
    for wall in [false, true] {
        let mut level = flat_level();
        if wall {
            // A wall whose right side touches the lizard's left edge.
            let mut rows: Vec<String> = vec![".".repeat(200); 15];
            for r in &mut rows[12..15] {
                r.replace_range(17..18, "#");
            }
            rows.push("#".repeat(200));
            rows.push("#".repeat(200));
            let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
            level.map = claw_sim::tiles::TileMap::from_ascii(64, &rows);
        }
        level
            .objects
            .push(ObjectSpec::at(chameleon::NAMES[0], 1180, 900, 0));
        let mut done = false;
        for seed in 1..=40 {
            let mut sim = Sim::new(
                level.clone(),
                SimConfig {
                    seed,
                    ..SimConfig::default()
                },
            );
            let t = Instant::now();
            for _ in 0..1500 {
                sim.step(TickInput::default());
                let (_, o) = sim
                    .objects()
                    .all()
                    .find(|(_, o)| &*o.logic == chameleon::NAMES[0])
                    .unwrap();
                if o.state == EnemyState::PhaseIn as i32 {
                    let d = enemy::data_of(o).unwrap();
                    if wall {
                        assert_eq!(o.x, 1180, "held by the wall");
                        assert!(d.bounds.0 <= -2_147_483_646, "{:?}", d.bounds);
                    } else {
                        assert_eq!(o.x, 0, "at the level's edge");
                    }
                    done = true;
                    break;
                }
            }
            assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
            if done {
                break;
            }
        }
        assert!(done, "no lizard phased (wall {wall})");
    }
}

#[test]
fn the_thirteenth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(13, SurfaceKind::Elevator),
        Rect::new(-44, -8, 46, 64)
    );
    assert_eq!(
        platform::surface(13, SurfaceKind::TogglePeg),
        Rect::new(-24, -8, 24, 64)
    );
    assert_eq!(
        platform::surface(13, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(13, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    // Springboards: the fourth level's springy ferns; the water rocks keep
    // their own surface and art.
    let s = springboard::level_table(13).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(springboard::level_table(4), Some(s));
    assert_eq!(springboard::WATER_ROCK_RECT, Rect::new(-25, 90, 25, 172));
    // Water death tiles that kill as goo does, the splash 50 px above.
    assert!(goo::is_goo_level(13));
    assert_eq!(goo::splash_y(13), Some(50));
    // Toggle pegs with no timing of their own: 1500 ms out, 1500 ms in.
    assert_eq!(platform::DEFAULT_TOGGLE_MS, 1500);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        redtail::IMAGE_SET,
        spikes::IMAGE_SET,
        knife::IMAGE_SET,
        flare::IMAGE_SET,
        flare::EXPLOSION_SET,
        debris::LOOKS[0],
        debris::LOOKS[1],
        debris::LOOKS[2],
        chameleon::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        redtail::IDLE_ANIM,
        redtail::DEATH_ANIM,
        flare::EXPLOSION_ANIM,
        chameleon::IDLE_ANIM,
        chameleon::RUN_ANIM,
        chameleon::STRIKE_ANIM,
        chameleon::DEATH_ANIM,
        springboard::WATER_ROCK.idle_anim,
        springboard::WATER_ROCK.spring_anim,
    ] {
        assert!(prefixes.iter().any(|p| anim.starts_with(p)), "{anim}");
    }
}

// ---------------------------------------------------------------- install

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level_n(store: &AssetStore, n: u32) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// A simulation of `level` in which every object is awake from the start
/// (activation windows twice the level's size).
fn awake(level: SimLevel) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

fn records<'a>(level: &'a SimLevel, name: &str) -> Vec<&'a ObjectSpec> {
    level.objects.iter().filter(|o| o.logic == name).collect()
}

/// Calls an animation takes to finish at 18 ms.
fn calls(level: &SimLevel, key: &str) -> usize {
    let mut p = AnimPlayer::default();
    p.set(level.animations[key].clone());
    let mut n = 0;
    while !p.is_done() {
        n += 1;
        assert!(n < 1000, "{key} never finishes");
        p.advance_in(18, 0, false, None);
    }
    n
}

#[test]
fn install_level13_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 13);
    assert_eq!(level.number, 13);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL13 inert census: {:?}", d.inert_logics);
    assert!(d.unknown_logics.is_empty(), "{:?}", d.unknown_logics);
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 4154, "the level's records");
    assert_eq!(level.start, (710, 2360));

    let placed = |name: &str| records(&level, name).len();
    for (name, n) in PLACED {
        assert_eq!(placed(name), n, "{name}");
    }
    // Each declared family is on the level, and reported inert until the
    // family is built; the run-time ones are not placed.
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for (_, names) in DECLARED {
        for &name in names {
            let n = placed(name);
            if n > 0 && !built(name) {
                want.insert(name.to_string(), n);
            }
        }
    }
    for &(name, _) in INERT_OTHERS {
        want.insert(name.to_string(), placed(name));
    }
    assert_eq!(
        d.inert_logics, want,
        "the logics the level leaves inert are the declared families and the listed ones"
    );
    for name in ["RedTailKnife", "RedTailBullet", "WindDebris"] {
        assert_eq!(placed(name), 0, "{name} is made at run time");
    }
    // The earlier slices' families the level reuses, all running.
    for (name, n) in [
        ("AniCycle", 1001),
        ("GlitterlessPowerup", 755),
        ("GooCoverup", 618),
        ("DoNothingNormal", 300),
        ("CrumblingPeg", 161),
        ("TreasurePowerup", 143),
        ("BehindAniCandy", 103),
        ("BigSoundTrigger", 100),
        ("FrontAniCandy", 99),
        ("FrontCandy", 83),
        ("TogglePeg", 73),
        ("FloorSpike", 61),
        ("DoNothing", 56),
        ("BearSailor", 38),
        ("PowderKeg", 35),
        ("TProjectile", 32),
        ("FrontCrate", 29),
        ("PathElevator", 24),
        ("AniRope", 22),
        ("RedTailPirate", 21),
        ("SpringBoard", 21),
        ("TogglePeg3", 21),
        ("HealthPowerup", 19),
        ("SpecialPowerup", 18),
        ("BehindCrate", 17),
        ("StartSteppingStone", 17),
        ("BehindCandy", 15),
        ("AmbientSound", 14),
        ("ClawDialogSmallSoundTrigger", 14),
        ("CursePowerup", 13),
        ("AmmoPowerup", 11),
        ("Checkpoint", 10),
        ("Elevator", 10),
        ("ClawDialogBigSoundTrigger", 9),
        ("TogglePeg2", 9),
        ("FrontStackedCrates", 8),
        ("GlobalAmbientSound", 6),
        ("OneWayTriggerElevator", 6),
        ("MagicPowerup", 5),
        ("BackStackedCrates", 3),
        ("TriggerElevator", 3),
        ("SpotAmbientSound", 2),
        ("TogglePeg4", 2),
        ("BossStager", 1),
        ("BossWarp", 1),
        ("FirstSuperCheckpoint", 1),
        ("SecondSuperCheckpoint", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
}

#[test]
fn install_level13_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 13);
    let preloaded = level.image_sets();
    for set in [
        redtail::IMAGE_SET,
        spikes::IMAGE_SET,
        knife::IMAGE_SET,
        flare::IMAGE_SET,
        chameleon::IMAGE_SET,
        "LEVEL_WATERROCK",
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The lizard's set reaches its phase picture, of its first picture's
    // size; its body box is its first picture's.
    let (_, last) = level.image_frames(chameleon::IMAGE_SET).unwrap();
    assert_eq!(last, chameleon::PHASE_FRAME);
    assert_eq!(
        level.frame_box(chameleon::IMAGE_SET, 1),
        Some(chameleon::BODY_RECT)
    );
    // Its tongue: the event on the fourth call, done on the sixteenth.
    assert_eq!(calls(&level, chameleon::STRIKE_ANIM), 16);
    for a in ["IDLE", "FASTADVANCE", "HIT", "JUMP", "KILLFALL", "STRIKE1"] {
        let key = format!("LEVEL_CHAMELEON_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    for s in [
        chameleon::TONGUE_SOUND,
        chameleon::DEATH_SOUND,
        chameleon::ROAM_SOUND,
    ] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // The water rock's spring is one call shorter than a fern's.
    assert_eq!(
        calls(&level, springboard::WATER_ROCK.spring_anim) + 1,
        calls(&level, "LEVEL_SPRINGYFERN_SPRING")
    );
    // RedTail's animations.
    for a in [
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "IDLE",
        "JUMP",
        "KILLFALL",
        "STRIKE2",
        "STRIKE3",
        "STRIKE4",
        "STRIKE6",
        "STRIKE7",
        "STRIKE8",
        "BLOCK",
    ] {
        let key = format!("LEVEL_REDTAIL_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level13_the_new_families_take_their_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 13);
    // Eight unbounded lizards; every other within its range of up to
    // 483 px.
    let lizards = records(&level, chameleon::NAMES[0]);
    let unbounded = lizards
        .iter()
        .filter(|r| r.record[enemy::field::X_MIN] == 0 && r.record[enemy::field::X_MAX] == 0)
        .count();
    assert_eq!(unbounded, 8);
    // The water rocks: 250 to 600 px throws.
    let heights: BTreeSet<i32> = records(&level, springboard::WATER_ROCK_NAME)
        .iter()
        .map(|r| springboard::height_of(r.record[springboard::HEIGHT_FIELD]))
        .collect();
    assert_eq!(heights.first(), Some(&250));
    assert_eq!(heights.last(), Some(&600));
    // Awake, each is built: the lizards at Home or deciding, health 1, not
    // liftable; the rocks idle with their own surface; the boss and his
    // props inert while unbuilt.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Chameleon" => {
                assert_eq!(o.health, 1);
                assert_eq!(o.hit_rect.top, chameleon::BODY_RECT.top);
                assert!(enemy::data_of(o).is_some_and(|d| !d.may_lift));
                *n.entry("lizards").or_default() += 1;
            }
            "WaterRock" => {
                assert_eq!(o.state, springboard::state::IDLE);
                assert_eq!(o.hit_rect, springboard::WATER_ROCK_RECT);
                *n.entry("rocks").or_default() += 1;
            }
            "SpringBoard" => {
                assert_eq!(o.hit_rect, springboard::surface(13));
                *n.entry("ferns").or_default() += 1;
            }
            "RedTail" | "RedTailWind" | "RedTailSpikes" if !redtail::BUILT => {
                assert_eq!(o.kind, LogicKind::Inert);
            }
            _ => {}
        }
    }
    assert_eq!(
        n,
        BTreeMap::from([("ferns", 21), ("lizards", 54), ("rocks", 11)])
    );
}

#[test]
fn install_level13_reused_families_take_the_thirteenth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 13);
    assert!(level.sound_ms("LEVEL_DEATHTILE").is_some());
    // The arena's toggle pegs have no timing of their own: 1500 ms out,
    // 1500 ms in, 167 ticks a cycle at the 18 ms step.
    let arena: Vec<_> = records(&level, "TogglePeg")
        .into_iter()
        .filter(|r| (37000..38000).contains(&r.record[5]) && (1900..2200).contains(&r.record[6]))
        .collect();
    assert_eq!(arena.len(), 2);
    for r in &arena {
        assert_eq!((r.record[55], r.record[56]), (0, 0), "no timing");
    }
    let at = (arena[0].record[5], arena[0].record[6]);
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let peg = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "TogglePeg" && (o.editor.0, o.editor.1) == at)
        .map(|(id, _)| id)
        .unwrap();
    let mut outs = Vec::new();
    let mut last = None;
    for k in 0..600 {
        sim.step(TickInput::default());
        let s = sim.objects().get(peg).unwrap().state;
        if s == peg_state::OUT && last != Some(peg_state::OUT) {
            outs.push(k);
        }
        last = Some(s);
    }
    // After its first cycle: 3000 ms, 166 or 167 ticks.
    assert!(outs.len() >= 4, "{outs:?}");
    for w in outs[1..].windows(2) {
        assert!((166..=167).contains(&(w[1] - w[0])), "{outs:?}");
    }
    // Elevators and pegs take the level's surfaces.
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Elevator" | "TriggerElevator" | "OneWayTriggerElevator" => {
                assert_eq!(o.hit_rect, platform::surface(13, SurfaceKind::Elevator));
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(o.hit_rect, platform::surface(13, SurfaceKind::TogglePeg));
            }
            _ => {}
        }
    }
}

#[test]
fn install_level13_the_water_kills_and_claw_starts_again_at_the_level_start() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 13);
    // Put down over the water band left of the first checkpoint, Claw
    // falls into the water and dies in it, and comes back at the level's
    // start.
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    assert_eq!(
        sim.claw().stats.dynamite,
        3,
        "the level starts with three sticks"
    );
    sim.with_world(|w| w.claw.special.warp_to = Some((6200, 2300)));
    let mut died = false;
    for _ in 0..600 {
        sim.step(TickInput::default());
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::ClawDied { .. }))
        {
            died = true;
            break;
        }
    }
    assert!(died, "he dies in the water");
    assert!(sim.claw().death_tile_hits > 0, "on its Death tiles");
    sim.step(TickInput::default());
    let c = sim.player().unwrap();
    assert_ne!(c.state, claw_state::DEATH);
    assert_eq!(c.x, 710, "back at the level's start point");
}

/// One copy of the game and one idle tick at the start of level `n`,
/// with its object count.
fn start_costs(store: &AssetStore, n: u32) -> (usize, std::time::Duration, std::time::Duration) {
    let mut sim = Sim::new(level_n(store, n), SimConfig::default());
    idle(&mut sim, 2);
    let t = Instant::now();
    let copies = 20;
    for _ in 0..copies {
        std::hint::black_box(sim.clone());
    }
    let clone = t.elapsed() / copies;
    let t = Instant::now();
    let steps = 200;
    idle(&mut sim, steps);
    let tick = t.elapsed() / steps as u32;
    (sim.objects().all().count(), clone, tick)
}

#[test]
fn install_level13_costs_at_its_start() {
    let Some(store) = store() else { return };
    // The level's start with every family it places, against the first
    // and the twelfth levels' starts: how long a copy of the game and an
    // idle tick take, for the slice's measurements.
    for n in [1, 12, 13] {
        let (objects, clone, tick) = start_costs(&store, n);
        println!("LEVEL{n} start: {objects} objects, one copy {clone:?}, one idle tick {tick:?}");
        if n == 13 {
            assert!(objects >= 4154 - 20);
        }
    }
}
