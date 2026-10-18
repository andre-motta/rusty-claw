//! The framework the sixth level's families build on: the goo vents, the
//! ground blowers and Wolvington with his magic shot and its light are
//! declared, and each stays an inert placeholder until its family is
//! built; Wolvington's class, data and state numbers; the level's
//! platform table, goo splash height and town guard numbers; and, on the
//! install, the level's inert census, its boss stage bound to the shared
//! stage pieces, the art its families need loaded with it, and what the
//! earlier families do with the level (the town guards, the ropes, the
//! goo cover-ups and animated pieces, the contact stones and the kegs).
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss_stage;
use claw_sim::logic::enemy::{self, EnemyState, field, goo, townguard, wolvington};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{
    self, GameLogic, ObjectData, decor, goo_vent, ground_blower, keg, rope, springboard,
    stepping_stone,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the sixth level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 5] = [
    (GameLogic::GooVent, &["GooVent"]),
    (GameLogic::GroundBlower, &["GroundBlower"]),
    (GameLogic::Wolvington, &["Wolvington"]),
    (GameLogic::WolvingtonShot, &["WolvingtonBullet"]),
    (GameLogic::WolvingtonLight, &["WolvingtonLFX"]),
];

/// Logics the sixth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the goo cover-ups, the animated pieces, the
/// `DoNothing` pictures), platform (elevators, trigger elevators,
/// crumbling pegs, the contact stones), rope, crate, keg, pickup,
/// checkpoint, sound object and enemy the level places outside the
/// declared families already runs, and so do the door and the stager of
/// its boss stage.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the sixth level places; the
/// shot and its light are made at run time, never placed.
const PLACED: [(&str, usize); 5] = [
    ("GooVent", 12),
    ("GroundBlower", 14),
    ("Wolvington", 1),
    ("WolvingtonBullet", 0),
    ("WolvingtonLFX", 0),
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
}

#[test]
fn declared_families_stay_inert_placeholders_until_built() {
    let unbuilt: Vec<_> = DECLARED.iter().filter(|(g, _)| !logic::built(*g)).collect();
    let names: Vec<&str> = unbuilt
        .iter()
        .flat_map(|(_, n)| n.iter().copied())
        .collect();
    for &&(_, list) in &unbuilt {
        for &n in list {
            assert_eq!(logic::runs(n), None, "{n}");
            assert_eq!(Registry::shared().get(n).unwrap().kind, LogicKind::Inert);
            assert_eq!(logic::new_object(n, 0, 0, 0).kind, LogicKind::Inert);
        }
    }
    // Placed in a level, they are drawn placeholders that never collide,
    // and the level load reports them as not implemented.
    let mut sim = Sim::new(level_with(&names), SimConfig::default());
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
}

#[test]
fn the_registry_keeps_each_names_activation_class() {
    // Vents, Wolvington, his shot and its light wake with the enemies'
    // window, blowers with the small objects'; none keeps the original
    // window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for (name, class) in [
        ("GooVent", 0),
        ("GroundBlower", 2),
        ("Wolvington", 0),
        ("WolvingtonBullet", 0),
        ("WolvingtonLFX", 0),
    ] {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
        assert!(!cfg.is_exception(name), "{name}");
    }
}

#[test]
fn wolvington_is_a_human_class_whose_data_holds_enemy_data() {
    let class = enemy::class(GameLogic::Wolvington).expect("a class");
    assert_eq!(class.name, "Wolvington");
    assert!(class.human, "Wolvington is a human enemy");
    for g in [
        GameLogic::GooVent,
        GameLogic::GroundBlower,
        GameLogic::WolvingtonShot,
        GameLogic::WolvingtonLight,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    let mut d = ObjectData::Wolvington(wolvington::WolvingtonData {
        enemy: enemy_data.clone(),
    });
    assert_eq!(d.enemy(), Some(&enemy_data));
    d.enemy_mut().unwrap().roll = 5;
    assert_eq!(d.enemy().unwrap().roll, 5);
    for d in [
        ObjectData::GooVent(Default::default()),
        ObjectData::GroundBlower(Default::default()),
        ObjectData::WolvingtonShot(Default::default()),
        ObjectData::WolvingtonLight(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // His stage is the shared one.
    for name in wolvington::STAGE {
        assert!(boss_stage::NAMES.contains(&name), "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    assert_eq!(wolvington::METER_MAX, 100);
    assert_eq!(wolvington::DAMAGE, 5);
    assert_eq!(wolvington::shot::DAMAGE, 5);
}

#[test]
fn the_sixth_levels_state_numbers_hold_wolvingtons_attacks() {
    let block = wolvington::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2160, 2179));
    let named = [
        (EnemyState::MagicVolley, 2160, "MagicVolley"),
        (EnemyState::MagicShot, 2161, "MagicShot"),
        (EnemyState::Uppercut, 2162, "Uppercut"),
        (EnemyState::SwordThrust, 2163, "SwordThrust"),
        (EnemyState::Sprint, 2164, "Sprint"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
    }
    for id in 2165..=2179 {
        assert_eq!(EnemyState::from_id(id), None, "{id} is free");
    }
    let in_block: Vec<_> = EnemyState::ALL
        .iter()
        .filter(|s| block.contains(&(**s as i32)))
        .copied()
        .collect();
    assert_eq!(in_block, named.map(|(s, _, _)| s));
    // The fifth level's block ends where the sixth's starts.
    assert_eq!(*townguard::STATE_BLOCK.end() + 1, *block.start());
    // The shared states he reaches keep their numbers.
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Advance as i32, 2009);
    assert_eq!(EnemyState::FastAdvance as i32, 2010);
    assert_eq!(EnemyState::JumpBack as i32, 2030);
    assert_eq!(EnemyState::BlockLow as i32, 2034);
    // A reference trace numbers them its own way.
    assert_eq!(
        wolvington::REFERENCE_STATE_IDS.map(|(s, id)| (s.name(), id)),
        [
            ("Startup", 2028),
            ("Home", 2018),
            ("Advance", 2021),
            ("MagicVolley", 2047),
            ("MagicShot", 2048),
            ("Uppercut", 2049),
            ("SwordThrust", 2050),
            ("Sprint", 2051),
        ]
    );
}

#[test]
fn the_sixth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(6, SurfaceKind::Elevator),
        Rect::new(-36, -3, 35, 98)
    );
    assert_eq!(
        platform::surface(6, SurfaceKind::CrumblingPeg),
        Rect::new(-36, -32, 35, 98)
    );
    assert_eq!(
        platform::surface(6, SurfaceKind::TogglePeg),
        Rect::new(0, 0, 0, 32),
        "no toggle pegs"
    );
    // The two contact stones.
    assert_eq!(
        platform::surface(6, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    assert_eq!(stepping_stone::surface(6), Rect::new(-16, -1, 16, 64));
    // Springboards: the fern, which no record places.
    let s = springboard::level_table(6).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING")
    );
    // Goo death tiles, with a 10 px splash.
    assert!(goo::is_goo_level(6));
    assert_eq!(goo::splash_y(6), Some(10));
    // The town guards: 10 health, and both start with the 90 hop chance.
    assert_eq!(townguard::health(6), 10);
    assert_eq!(townguard::jump_back_chance(&townguard::GUARD1, 6), 90);
    assert_eq!(townguard::jump_back_chance(&townguard::GUARD2, 6), 90);
    assert_eq!(townguard::GUARD1_PARRY_CHANCE, 25);
    assert_eq!(townguard::PARRY_CHANCE.1, 50);
}

#[test]
fn the_sixth_levels_rope_speeds_set_the_swing() {
    // Half swings of 2000 to 800 ms: 30 to 75 frames per second, a whole
    // swing of 4000 to 1600 ms.
    for (half, q) in [
        (2000, 30),
        (1750, 34),
        (1600, 37),
        (1400, 42),
        (1200, 50),
        (1000, 60),
        (800, 75),
    ] {
        assert_eq!(rope::motion(half, 0), (q, 0.0), "{half}");
    }
    // A phase of 1150 ms starts a 1500 ms rope 46 frames in.
    assert_eq!(rope::motion(1500, 1150), (40, 46.0));
}

#[test]
fn a_keg_runs_its_first_call_at_its_records_depth() {
    // Most of the sixth level's kegs are placed at depth 5000, after
    // Claw: the set-up call, made as the level loads, keeps it; every
    // later call sorts the keg against Claw.
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at(keg::NAMES[0], 1100, 900, 5000));
    let mut sim = Sim::new(level, SimConfig::default());
    let (id, o) = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Keg))
        .expect("the keg");
    assert_eq!(o.state, keg::state::READY, "set up");
    assert_eq!(o.z, 5000, "set up at its record's depth");
    sim.step(TickInput::default());
    let z = sim.objects().get(id).unwrap().z;
    assert!(
        [
            keg::Z_BASE + enemy::motion::DEPTH_IN_FRONT,
            keg::Z_BASE + enemy::motion::DEPTH_BEHIND
        ]
        .contains(&z),
        "sorted against Claw: {z}"
    );
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        goo_vent::IMAGE_SET,
        wolvington::IMAGE_SET,
        wolvington::shot::IMAGE_SET,
        wolvington::light::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        goo_vent::ANIM,
        ground_blower::ANIM,
        "LEVEL_WOLVINGTON_IDLE",
        "LEVEL_WOLVINGTON_STRIKE4",
        wolvington::shot::ANIM,
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

/// Calls an animation takes to finish at 18 ms, and the calls that report
/// event 1.
fn calls(level: &SimLevel, key: &str) -> (usize, Vec<usize>) {
    let mut p = AnimPlayer::default();
    p.set(level.animations[key].clone());
    let mut events = Vec::new();
    let mut n = 0;
    while !p.is_done() {
        n += 1;
        assert!(n < 1000, "{key} never finishes");
        if p.advance_in(18, 0, false, None).event == 1 {
            events.push(n);
        }
    }
    (n, events)
}

#[test]
fn install_level6_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    assert_eq!(level.number, 6);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL6 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");

    let placed = |name: &str| records(&level, name).len();
    for (name, n) in PLACED {
        assert_eq!(placed(name), n, "{name}");
    }
    // Each declared family is on the level or made by one that is, and
    // reported inert until the family is built.
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for (_, names) in DECLARED {
        for &name in names {
            let n = placed(name);
            if n > 0 && logic::runs(name).is_none() {
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
    // The earlier slices' families the level reuses, all running.
    for (name, n) in [
        ("TownGuard1", 56),
        ("TownGuard2", 45),
        ("Seagull", 25),
        ("Rat", 14),
        ("AniRope", 15),
        ("StartSteppingStone", 2),
        ("GooCoverup", 305),
        ("AniCycle", 151),
        ("PowderKeg", 75),
        (boss_stage::STAGER, 1),
        (boss_stage::WARP, 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // Nothing from the earlier levels' families the sixth does not use,
    // and no end-of-level pickup: the boss ends the level.
    for name in [
        "Officer",
        "Soldier",
        "PunkRat",
        "CutThroat",
        "RobberThief",
        "Raux",
        "Katherine",
        "FloorSpike",
        "PathElevator",
        "SpringBoard",
        "SteppingStone",
        "BreakPlank",
        "JumpSwitch",
        "FrontStatue",
        "BehindStatue",
        "TogglePeg",
        "EndOfLevelPowerup",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level6_boss_stage_binds_around_wolvington() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let (doors, stagers, bosses) = (
        records(&level, boss_stage::WARP),
        records(&level, boss_stage::STAGER),
        records(&level, "Wolvington"),
    );
    assert_eq!((doors.len(), stagers.len(), bosses.len()), (1, 1, 1));
    let target = (doors[0].record[55], doors[0].record[56]);
    let stager = (stagers[0].record[5], stagers[0].record[6]);
    let boss = &bosses[0].record;
    println!(
        "LEVEL6 stage: door target {target:?}, stager {stager:?}, Wolvington {:?}",
        (boss[5], boss[6])
    );
    assert_eq!(target, (34645, 4498));
    assert_eq!(stager, (34645, 4476));
    assert_eq!((boss[5], boss[6]), (34960, 4496));
    // The gem's target, 51 px left of him and 166 px above.
    assert_eq!((boss[55], boss[56]), (34909, 4330));
    // His record's health is not his health.
    assert_eq!(boss[18], 200);
    assert_ne!(boss[18], wolvington::HEALTH);

    // Claw put down where the door drops him starts the introduction. While
    // it runs Wolvington is the current boss and his health bar is full; the
    // fight comes on 694 ticks after the introduction's first tick, as in the
    // original game (551 plus his line's 2592 ms, 144 ticks, less the one
    // call the line takes without him).
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in wolvington::STAGE {
        assert_eq!(d.inert_logics.get(name), None, "{name} runs");
    }
    sim.teleport_player(target.0, target.1);
    let mut staging = None;
    let mut on = None;
    for t in 1..=900 {
        sim.step(TickInput::default());
        match sim.boss().fight() {
            fight::STAGING => {
                staging.get_or_insert(t);
                assert!(sim.boss().current().is_some(), "he is the boss");
                let meter = sim.hud().boss_meter.map(|m| (m.value, m.max));
                assert_eq!(meter, Some((100, wolvington::METER_MAX)));
            }
            fight::ON => {
                on = Some(t);
                break;
            }
            _ => {}
        }
    }
    println!("LEVEL6 introduction: staging from tick {staging:?}, fight on at {on:?}");
    let (staging, on) = (staging.expect("the introduction starts"), on.expect("on"));
    assert_eq!(on - staging, 551 + 144 - 1, "the introduction's length");
}

#[test]
fn install_level6_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let preloaded = level.image_sets();
    for (set, frames) in [
        (goo_vent::IMAGE_SET, Some((1, 9))),
        (ground_blower::IMAGE_SET, Some((1, 17))),
        (wolvington::IMAGE_SET, None),
        (wolvington::shot::IMAGE_SET, None),
        (wolvington::light::IMAGE_SET, None),
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, last) = level.image_frames(set).expect("registered");
        if let Some(f) = frames {
            assert_eq!((first, last), f, "{set}");
        }
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The blower's nozzle box comes from its first picture.
    let jet = level.frame_box(ground_blower::IMAGE_SET, 1).expect("a box");
    assert_eq!(jet, Rect::new(-11, 76, -4, 79));
    assert_eq!(ground_blower::nozzle_box(jet), Rect::new(-16, 66, 21, 89));
    println!(
        "LEVEL6 vent box {:?}, shot box {:?}",
        level.frame_box(goo_vent::IMAGE_SET, 1),
        level.frame_box(wolvington::shot::IMAGE_SET, 1)
    );

    // The vent's pass: 17 pictures of 12 ticks, finished on call 205.
    let vent = &level.animations[goo_vent::ANIM];
    assert_eq!(vent.frames.len(), 17);
    let pictures: Vec<i32> = vent.frames.iter().map(|f| f.image_index).collect();
    assert_eq!(
        pictures,
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 7, 6, 5, 4, 3, 2, 1]
    );
    assert_eq!(
        calls(&level, goo_vent::ANIM).0,
        goo_vent::PASS_TICKS as usize
    );
    // The blower's jet loops: 18 pictures, the hiss on the fourth.
    let jet = &level.animations[ground_blower::ANIM];
    assert_eq!(jet.frames.len(), 18);
    assert!(jet.frames.iter().all(|f| f.next_op == 0), "it loops");
    let total: i32 = jet.frames.iter().map(|f| f.duration).sum();
    assert_eq!(total, 825);
    assert_eq!(jet.frames[17].image_index, 18, "a picture the set lacks");
    assert_eq!(
        jet.sounds_of(3),
        [Some(ground_blower::HISS.to_string())],
        "the hiss"
    );
    // The first picture comes back every 57 calls.
    let mut p = AnimPlayer::default();
    p.set(jet.clone());
    let starts: Vec<usize> = (1..=120)
        .filter(|_| p.advance_in(18, 0, false, None).applied == Some(0))
        .collect();
    assert_eq!(starts, [1, 58, 115]);
    assert_eq!(starts[1] - starts[0], ground_blower::LOOP_TICKS as usize);

    // Wolvington's animations and their timings at 18 ms.
    for a in [
        "ADVANCE",
        "BLOCK",
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "IDLE",
        "JUMP",
        "KILLFALL",
        "LIFTED",
        "LIFTED2",
        "STRIKE1",
        "STRIKE2",
        "STRIKE3",
        "STRIKE4",
        "THROWN",
    ] {
        let key = format!("LEVEL_WOLVINGTON_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    let strike = |a: &str| {
        let (n, events) = calls(&level, &format!("LEVEL_WOLVINGTON_{a}"));
        (n, events.first().copied(), events.last().copied())
    };
    // The low thrust: live on calls 13 to 22, done on 28.
    assert_eq!(strike("STRIKE1"), (28, Some(13), Some(22)));
    // The uppercut: live on calls 16 to 27, done on 37.
    assert_eq!(strike("STRIKE2"), (37, Some(16), Some(27)));
    // A cast, high or low: the shot on call 16, done on 34.
    assert_eq!(strike("STRIKE3").0, 34);
    assert_eq!(strike("STRIKE3").1, Some(16));
    assert_eq!(strike("STRIKE4").0, 34);
    assert_eq!(strike("STRIKE4").1, Some(16));
    assert_eq!(strike("HITLOW").0, 19);
    assert_eq!(strike("BLOCK").0, 16);
    assert!(level.animations.contains_key(wolvington::shot::ANIM));
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level6_vents_and_blowers_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let vents = records(&level, "GooVent");
    assert_eq!(vents.len(), 12);
    for r in &vents {
        assert_eq!(r.image_set, goo_vent::IMAGE_SET);
        assert_eq!(r.record[7], 0, "depth 0");
        assert_eq!(r.record[goo_vent::WAIT_FIELD], 0, "no wait");
    }
    let mut blowers: Vec<((i32, i32), i32, i32)> = records(&level, "GroundBlower")
        .iter()
        .map(|r| {
            assert_eq!(r.image_set, ground_blower::IMAGE_SET);
            (
                (r.record[5], r.record[6]),
                r.record[7],
                ground_blower::height_of(r.record[ground_blower::HEIGHT_FIELD]),
            )
        })
        .collect();
    blowers.sort();
    assert_eq!(blowers.len(), 14);
    assert!(blowers.iter().all(|b| b.1 == 5000), "depth 5000");
    let low: Vec<(i32, i32)> = blowers.iter().filter(|b| b.2 != 450).map(|b| b.0).collect();
    assert_eq!(low, [(16628, 4726), (23408, 4790)], "the 400 px blowers");
    assert!(blowers.iter().all(|b| [400, 450].contains(&b.2)));
}

#[test]
fn install_level6_town_guards_follow_the_sixth_levels_rules() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let mut stationary_records = [0, 0];
    let mut wearing_second = Vec::new();
    for (i, name) in ["TownGuard1", "TownGuard2"].into_iter().enumerate() {
        for r in records(&level, name) {
            if r.record[field::USER_0] > 0 {
                stationary_records[i] += 1;
            }
            if i == 0 && r.image_set == "LEVEL_TOWNGUARD2" {
                wearing_second.push((r.record[5], r.record[6]));
            }
        }
    }
    assert_eq!(stationary_records, [7, 5]);
    assert_eq!(wearing_second.len(), 2);
    // The second guard's JUMP is the fifth level's; the first guard's runs
    // four ticks longer on the sixth level.
    let level5 = level_n(&store, 5);
    let jump = |l: &SimLevel| calls(l, townguard::GUARD1.anims.jump).0;
    println!(
        "first guard's jump: {} calls on the fifth level, {} on the sixth",
        jump(&level5),
        jump(&level)
    );
    assert_eq!(jump(&level), jump(&level5) + 4);
    let durations = |l: &SimLevel| -> Vec<i32> {
        l.animations[townguard::GUARD1.anims.jump]
            .frames
            .iter()
            .map(|f| f.duration)
            .collect()
    };
    let (d5, d6) = (durations(&level5), durations(&level));
    assert_eq!((d6[0], d5[0]), (80, 100));
    assert_eq!((d6[2], d5[2]), (90, 0));
    assert_eq!(
        calls(&level, townguard::GUARD2.anims.jump),
        calls(&level5, townguard::GUARD2.anims.jump)
    );

    // Built, every guard has 10 health and the 90 hop chance; the guards
    // whose record sets its first user value stand still, and the first
    // guards wearing the second's picture take their own. One other first
    // guard stands still too: a wall rising beside him ends his narrow
    // ledge, and with Claw far away his leap check puts him for good.
    let mut sim = awake(level);
    idle(&mut sim, 3);
    let mut built = [0, 0];
    let mut stationary = [0, 0];
    let mut put_in_play = Vec::new();
    for (id, o) in sim.objects().all() {
        if !matches!(
            o.kind,
            LogicKind::Game(GameLogic::TownGuard1 | GameLogic::TownGuard2)
        ) {
            continue;
        }
        let ObjectData::TownGuard(d) = &o.data else {
            panic!("{} at {},{}: {:?}", o.logic, o.x, o.y, o.data)
        };
        let i = usize::from(d.second);
        built[i] += 1;
        assert_eq!(o.health, 10);
        assert_eq!(d.jump_back_chance, 90);
        assert_eq!(o.image_set, d.guard().image_set);
        let record = o.record.as_ref().expect("placed");
        if record[field::USER_0] > 0 {
            assert!(d.enemy.stationary);
            stationary[i] += 1;
        } else if d.enemy.stationary {
            put_in_play.push((id, i, (record[5], record[6])));
        }
    }
    assert_eq!(built, [56, 45]);
    assert_eq!(stationary, [7, 5]);
    let [(guard, 0, (1045, 7313))] = put_in_play[..] else {
        panic!("guards put in play: {put_in_play:?}")
    };
    let mut ledge = Sim::clone(&sim);
    assert!(ledge.with_world(|w| enemy::motion::on_narrow_ledge(w, guard)));
}

#[test]
fn install_level6_ropes_swing_at_their_records_speeds() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut ropes = Vec::new();
    for (_, o) in sim.objects().all() {
        if !rope::NAMES.contains(&&*o.logic) {
            continue;
        }
        let ObjectData::Rope(d) = &o.data else {
            panic!("set up: {:?}", o.data)
        };
        let record = o.record.as_ref().expect("a placed rope");
        let (q, _) = rope::motion(o.speed_x, record[60]);
        assert_eq!(d.q, q, "rope at {},{}", o.x, o.y);
        ropes.push((o.x, o.y, o.speed_x, record[60]));
    }
    ropes.sort();
    println!("LEVEL6 ropes: {ropes:?}");
    assert_eq!(ropes.len(), 15);
    let row: Vec<(i32, i32)> = ropes
        .iter()
        .filter(|r| r.1 == 7379)
        .map(|r| (r.0, r.2))
        .collect();
    assert_eq!(row.len(), 7);
    assert_eq!(row[0].0, 7105);
    assert_eq!(row[6].0, 9409);
    assert_eq!(
        row.iter().map(|r| r.1).collect::<Vec<_>>(),
        [2000, 1750, 1600, 1400, 1200, 1000, 800]
    );
    let phased: Vec<i32> = ropes.iter().map(|r| r.3).filter(|&p| p != 0).collect();
    assert_eq!(phased, [1150], "one rope sets a phase");
    assert!(ropes.iter().all(|r| (800..=2000).contains(&r.2)));
}

#[test]
fn install_level6_cover_ups_and_animated_pieces_draw_at_their_record_depth() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut covers: BTreeMap<String, usize> = BTreeMap::new();
    let mut cover_depths: BTreeMap<i32, usize> = BTreeMap::new();
    let mut cycles: BTreeMap<String, usize> = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        let record = o.record.as_ref();
        match &*o.logic {
            "GooCoverup" => {
                assert_eq!(o.z, record.unwrap()[7], "the record's depth");
                assert_eq!(o.state, decor::SET_UP);
                *covers.entry(o.image_set.clone()).or_default() += 1;
                *cover_depths.entry(o.z).or_default() += 1;
            }
            "AniCycle" => {
                assert_eq!(o.z, record.unwrap()[7], "the record's depth");
                let ObjectData::Decor(d) = &o.data else {
                    panic!("set up: {:?}", o.data)
                };
                assert!(d.anim.animation().is_some(), "{} is animated", o.image_set);
                *cycles.entry(o.image_set.clone()).or_default() += 1;
            }
            _ => {}
        }
    }
    println!("LEVEL6 goo cover-ups {covers:?} at {cover_depths:?}, animated pieces {cycles:?}");
    assert_eq!(
        covers,
        BTreeMap::from([
            ("LEVEL_GOOCOVERCENTER".to_string(), 211),
            ("LEVEL_GOOCOVERLEFT".to_string(), 13),
            ("LEVEL_GOOCOVERRIGHT".to_string(), 13),
            ("LEVEL_SEWERCOVERUP1".to_string(), 9),
            ("LEVEL_SEWERCOVERUP2".to_string(), 50),
            ("LEVEL_SEWERCOVERUP3".to_string(), 9),
        ])
    );
    assert_eq!(cover_depths, BTreeMap::from([(8000, 302), (8010, 3)]));
    assert_eq!(
        cycles,
        BTreeMap::from([
            ("LEVEL_PIPETOPLEFT".to_string(), 8),
            ("LEVEL_PIPETOPRIGHT".to_string(), 26),
            ("LEVEL_TRIPLEGOO".to_string(), 86),
            ("LEVEL_WATER1".to_string(), 14),
            ("LEVEL_WATER2".to_string(), 17),
        ])
    );
    for set in covers.keys().chain(cycles.keys()) {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, last) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
        assert!(last >= first);
    }
}

#[test]
fn install_level6_contact_stones_and_kegs() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 6);
    // Two contact stones on the level's stone surface.
    let mut stones: Vec<(i32, i32)> = records(&level, "StartSteppingStone")
        .iter()
        .map(|r| {
            assert_eq!(r.image_set, "LEVEL_SEWAGESTEPSTONE");
            (r.record[5], r.record[6])
        })
        .collect();
    stones.sort();
    assert_eq!(stones, [(21080, 6083), (21529, 5955)]);
    assert!(
        level
            .image_sets()
            .iter()
            .any(|s| s == "LEVEL_SEWAGESTEPSTONE")
    );
    // 75 kegs, 70 of them after Claw at depth 5000.
    let mut depths: BTreeMap<i32, usize> = BTreeMap::new();
    for r in records(&level, keg::NAMES[0]) {
        *depths.entry(r.record[7]).or_default() += 1;
    }
    println!("LEVEL6 keg depths: {depths:?}");
    assert_eq!(depths.values().sum::<usize>(), 75);
    assert_eq!(depths.get(&5000), Some(&70));

    let mut sim = awake(level);
    idle(&mut sim, 2);
    let set_up: Vec<_> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "StartSteppingStone")
        .map(|(_, o)| o.hit_rect)
        .collect();
    assert_eq!(set_up, [stepping_stone::surface(6); 2]);
}
