//! The framework the fourth level's families build on: the springboards,
//! the stepping stones and Katherine are declared, and each stays an inert
//! placeholder until its family is built (the springboards and the
//! stepping stones are); Katherine's class, data and state numbers;
//! the level's platform table, springboard data and goo splash height;
//! and, on the install, the level's inert census, its boss stage bound to
//! the shared stage pieces, and what the earlier families do with the
//! level's floor spikes, goo cover-ups and animated goo.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss_stage::{self, TRIGGER_BOX};
use claw_sim::logic::enemy::{self, EnemyState, goo, katherine};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, GameLogic, ObjectData, decor, floor_spike, springboard};
use claw_sim::logic::{stepping_stone, stepping_stone::SteppingStoneData};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the fourth level, with the names they claim
/// (the springboards' family also runs the thirteenth level's water
/// rocks).
const DECLARED: [(GameLogic, &[&str]); 3] = [
    (GameLogic::SpringBoard, &["SpringBoard", "WaterRock"]),
    (
        GameLogic::SteppingStone,
        &[
            "SteppingStone",
            "SteppingStone2",
            "SteppingStone3",
            "SteppingStone4",
            "StartSteppingStone",
            "OneTimeStartSteppingStone",
        ],
    ),
    (GameLogic::Katherine, &["Katherine"]),
];

/// Logics the fourth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the goo cover-ups, the animated goo), platform,
/// pickup, sound object, hazard and enemy the level places outside the
/// declared families already runs, and so do the door and the stager of
/// its boss stage.
const INERT_OTHERS: &[(&str, &str)] = &[];

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
    // Springboards wake with the elevators' window, stones with the small
    // hazards', Katherine with the enemies'.
    let mut want = vec![("SpringBoard", 1), ("Katherine", 0)];
    want.extend(stepping_stone::NAMES.iter().map(|&n| (n, 2)));
    for (name, class) in want {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
    }
}

#[test]
fn katherine_is_a_human_class_whose_data_holds_enemy_data() {
    let class = enemy::class(GameLogic::Katherine).expect("a class");
    assert_eq!(class.name, "Katherine");
    assert!(class.human, "Katherine is a human enemy");
    for g in [GameLogic::SpringBoard, GameLogic::SteppingStone] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    let mut d = ObjectData::Katherine(katherine::KatherineData {
        enemy: enemy_data.clone(),
        ..Default::default()
    });
    assert_eq!(d.enemy(), Some(&enemy_data));
    d.enemy_mut().unwrap().roll = 5;
    assert_eq!(d.enemy().unwrap().roll, 5);
    for d in [
        ObjectData::SpringBoard(Default::default()),
        ObjectData::SteppingStone(SteppingStoneData::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Her stage is the shared one.
    for name in katherine::STAGE {
        assert!(boss_stage::NAMES.contains(&name), "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    assert_eq!(katherine::METER_MAX, 100);
}

#[test]
fn the_fourth_levels_state_numbers_hold_katherines_flip() {
    let block = katherine::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2120, 2139));
    assert_eq!(EnemyState::Flip as i32, 2120);
    assert_eq!(EnemyState::FlipLand as i32, 2121);
    assert_eq!(EnemyState::from_id(2120), Some(EnemyState::Flip));
    assert_eq!(EnemyState::from_id(2121), Some(EnemyState::FlipLand));
    for id in 2122..=2139 {
        assert_eq!(EnemyState::from_id(id), None, "{id} is free");
    }
    let in_block: Vec<_> = EnemyState::ALL
        .iter()
        .filter(|s| block.contains(&(**s as i32)))
        .collect();
    assert_eq!(in_block, [&EnemyState::Flip, &EnemyState::FlipLand]);
    // The shared jump keeps its own number.
    assert_eq!(EnemyState::Jump as i32, 2012);
    // A reference trace numbers them its own way.
    assert_eq!(
        katherine::REFERENCE_STATE_IDS.map(|(s, id)| (s.name(), id)),
        [
            ("Flip", 2005),
            ("FlipLand", 2033),
            ("Startup", 2028),
            ("Home", 2018)
        ]
    );
}

#[test]
fn the_fourth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(4, SurfaceKind::Elevator),
        Rect::new(-44, -18, 46, 64)
    );
    assert_eq!(
        platform::surface(4, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(4, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    assert_eq!(stepping_stone::surface(4), Rect::new(-16, -1, 16, 64));
    assert_eq!(
        platform::surface(4, SurfaceKind::TogglePeg),
        Rect::new(0, 0, 0, 32),
        "no toggle pegs"
    );
    // Springboards: the fern.
    let s = springboard::level_table(4).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(springboard::surface(4), Rect::new(-32, 10, 24, 64));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING")
    );
    // Goo death tiles, with a 40 px splash.
    assert!(goo::is_goo_level(4));
    assert_eq!(goo::splash_y(4), Some(40));
    assert_eq!(goo::splash_y(2), Some(50));
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

fn level4(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(4)).unwrap();
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

#[test]
fn install_level4_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level4(&store);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL4 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");

    let placed = |name: &str| level.objects.iter().filter(|o| o.logic == name).count();
    // Each declared family is on the level (the one-time contact stone is
    // on no level), and reported inert until the family is built.
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for (g, names) in DECLARED {
        assert!(
            names.iter().any(|n| placed(n) > 0),
            "LEVEL4 places the {g:?} family"
        );
        for &name in names {
            let n = placed(name);
            if n > 0 && logic::runs(name).is_none() {
                want.insert(name.to_string(), n);
            }
        }
    }
    assert_eq!(placed("OneTimeStartSteppingStone"), 0);
    for &(name, _) in INERT_OTHERS {
        want.insert(name.to_string(), placed(name));
    }
    assert_eq!(
        d.inert_logics, want,
        "the logics the level leaves inert are the declared families and the listed ones"
    );
}

#[test]
fn install_level4_boss_stage_binds_around_katherine() {
    let Some(store) = store() else { return };
    let level = level4(&store);
    let records = |name: &str| -> Vec<&ObjectSpec> {
        level.objects.iter().filter(|o| o.logic == name).collect()
    };
    let (doors, stagers, bosses) = (
        records(boss_stage::WARP),
        records(boss_stage::STAGER),
        records("Katherine"),
    );
    assert_eq!((doors.len(), stagers.len(), bosses.len()), (1, 1, 1));
    let target = (doors[0].record[55], doors[0].record[56]);
    let stager = (stagers[0].record[5], stagers[0].record[6]);
    let boss = (bosses[0].record[5], bosses[0].record[6]);
    println!("LEVEL4 stage: door target {target:?}, stager {stager:?}, Katherine {boss:?}");
    // The door drops Claw inside the stager's box, in Katherine's
    // clearing.
    assert!((stager.0 - 31..=stager.0 + 38).contains(&target.0));
    assert!((stager.1 - 68..=stager.1 + 61).contains(&target.1));
    assert!((boss.0 - stager.0).abs() < 400 && (boss.1 - stager.1).abs() < 100);
    assert_eq!(TRIGGER_BOX, Rect::new(-16, -16, 16, 16));

    // Claw put down where the door drops him starts the introduction; with
    // Katherine not built there is no current boss and no health bar, and
    // the fight still comes on at the end of the introduction. Built, she
    // is the current boss and the bar reads her health.
    let built = logic::built(GameLogic::Katherine);
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in katherine::STAGE {
        assert_eq!(d.inert_logics.get(name), None, "{name} runs");
    }
    sim.teleport_player(target.0, target.1);
    let her = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Katherine")
        .map(|(id, _)| id)
        .expect("Katherine is placed");
    let mut staging = None;
    let mut on = None;
    let mut meter = None;
    for t in 1..=900 {
        sim.step(TickInput::default());
        if !built {
            assert_eq!(sim.boss().current(), None, "no boss is built");
            assert_eq!(sim.hud().boss_meter, None);
        } else {
            // Once she is set up (when Claw's arrival wakes her) she is the
            // current boss.
            if matches!(
                sim.objects().get(her).unwrap().data,
                ObjectData::Katherine(_)
            ) {
                assert_eq!(sim.boss().current(), Some(her), "she is the boss");
            }
            if let Some(m) = sim.hud().boss_meter {
                assert_eq!(m.max, katherine::METER_MAX);
                assert_eq!(m.value, sim.objects().get(her).unwrap().health);
                meter.get_or_insert(t);
            }
        }
        match sim.boss().fight() {
            fight::STAGING => {
                staging.get_or_insert(t);
            }
            fight::ON => {
                on = Some(t);
                break;
            }
            _ => {}
        }
    }
    println!(
        "LEVEL4 introduction: staging from tick {staging:?}, health bar from {meter:?}, fight on at {on:?}"
    );
    assert!(staging.is_some(), "the introduction starts");
    assert!(on.is_some(), "the fight comes on");
    if built {
        assert_eq!(sim.boss().current(), Some(her), "she is the boss");
        assert!(meter.is_some(), "the bar reads her health");
    }
}

#[test]
fn install_level4_floor_spikes_strike_from_their_retracted_frame_on_record_timing() {
    let Some(store) = store() else { return };
    let level = level4(&store);
    let set = "LEVEL_FLOORSPIKES";
    assert_eq!(level.image_frames(set), Some((1, 5)), "five images");
    assert!(level.image_sets().iter().any(|s| s == set), "preloaded");
    let mut sim = awake(level);
    sim.step(TickInput::default());
    let mut timings = BTreeSet::new();
    let mut n = 0;
    for (_, o) in sim.objects().all() {
        if !floor_spike::NAMES.contains(&&*o.logic) {
            continue;
        }
        n += 1;
        assert_eq!(&*o.logic, "FloorSpike", "only the first variant");
        assert_eq!(o.image_set, set);
        assert_eq!(o.attack_rect, Rect::new(-7, -3, 8, 12));
        let ObjectData::FloorSpike(d) = &o.data else {
            panic!("set up: {:?}", o.data)
        };
        let record = o.record.as_ref().expect("a placed spike");
        let half = |v: i32| if v == 0 { 1500 } else { v };
        assert_eq!(d.on_ms, half(o.speed_x));
        assert_eq!(d.period, half(o.speed_x) + half(o.speed_y));
        assert_eq!(d.phase, record[60]);
        timings.insert((d.on_ms, d.period - d.on_ms));
    }
    println!("LEVEL4 floor spikes: {n}, up and down halves {timings:?}");
    assert!(n > 0);
    for t in [
        (1500, 1500),
        (1000, 1000),
        (3000, 3000),
        (750, 750),
        (800, 800),
        (6000, 2000),
    ] {
        assert!(timings.contains(&t), "{t:?}");
    }
}

#[test]
fn install_level4_goo_cover_ups_and_animated_goo_draw_at_their_record_depth() {
    let Some(store) = store() else { return };
    let level = level4(&store);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut covers = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    let (mut n_covers, mut n_cycles) = (0, 0);
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "GooCoverup" => {
                n_covers += 1;
                assert_eq!(o.z, 8000, "the record's depth");
                assert_eq!(o.state, decor::SET_UP);
                covers.insert(o.image_set.clone());
            }
            "AniCycle" => {
                n_cycles += 1;
                assert_eq!(o.z, 5000, "the record's depth");
                let ObjectData::Decor(d) = &o.data else {
                    panic!("set up: {:?}", o.data)
                };
                assert!(d.anim.animation().is_some(), "animated");
                cycles.insert(o.image_set.clone());
            }
            _ => {}
        }
    }
    println!("LEVEL4 goo cover-ups {n_covers} {covers:?}, animated goo {n_cycles} {cycles:?}");
    let want_covers: BTreeSet<String> = [
        "LEVEL_GOOMIDDLE",
        "LEVEL_GOOLEFT",
        "LEVEL_GOORIGHT",
        "LEVEL_GOOCOVERUP",
        "LEVEL_TREETRUNKCOVER",
    ]
    .map(String::from)
    .into();
    let want_cycles: BTreeSet<String> = ["LEVEL_TRIPLEGOO", "LEVEL_SINGLEGOO"]
        .map(String::from)
        .into();
    assert_eq!(covers, want_covers);
    assert_eq!(cycles, want_cycles);
    for set in want_covers.iter().chain(&want_cycles) {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, last) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
        assert!(last >= first);
    }
}
