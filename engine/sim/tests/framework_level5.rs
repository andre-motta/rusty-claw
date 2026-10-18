//! The framework the fifth level's families build on: the two town
//! guards, the seagull, the breaking planks, the jump switches and the
//! statues are declared, and each stays an inert placeholder until its
//! family is built; the guards' and the seagull's classes, data and state
//! numbers; the level's platform and plank tables; the jump switches'
//! place among the widened mode's exceptions; and, on the install, the
//! level's inert census, the art its families need loaded with it, its
//! animated decoration, and the records the families will read (the
//! switches' targets, the plank rows and their floor cells, the statues).
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::{ActivationConfig, BREAKABLE_LOGICS, ORIGINAL_WINDOW_LOGICS};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, goo, seagull, townguard};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::{
    self, GameLogic, ObjectData, break_plank, decor, jump_switch, springboard, statue,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::tiles::GROUND;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the fifth level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 6] = [
    (GameLogic::TownGuard1, &["TownGuard1"]),
    (GameLogic::TownGuard2, &["TownGuard2"]),
    (GameLogic::Seagull, &["Seagull"]),
    (GameLogic::BreakPlank, &["BreakPlank", "SinglePlank"]),
    (GameLogic::JumpSwitch, &["JumpSwitch"]),
    (GameLogic::Statue, &["FrontStatue", "BehindStatue"]),
];

/// Logics the fifth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the animated `AniCycle` pieces, the `DoNothing`
/// pictures), platform (elevators, the trigger elevator, the toggle pegs),
/// crate, pickup, checkpoint and sound object the level places outside the
/// declared families already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the fifth level places; the
/// pieces of the plank rows are built at run time, never placed.
const PLACED: [(&str, usize); 8] = [
    ("TownGuard1", 73),
    ("TownGuard2", 30),
    ("Seagull", 29),
    ("BreakPlank", 21),
    ("SinglePlank", 0),
    ("JumpSwitch", 7),
    ("FrontStatue", 2),
    ("BehindStatue", 6),
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
    // Guards, seagulls and planks wake with the enemies' window, switches
    // and statues with the small objects'.
    let want = [
        ("TownGuard1", 0),
        ("TownGuard2", 0),
        ("Seagull", 0),
        ("BreakPlank", 0),
        ("SinglePlank", 0),
        ("JumpSwitch", 2),
        ("FrontStatue", 2),
        ("BehindStatue", 2),
    ];
    for (name, class) in want {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
    }
}

#[test]
fn switches_and_statues_keep_the_original_window_when_widened() {
    let cfg = ActivationConfig::widened((1706, 480));
    for name in ["JumpSwitch", "FrontStatue", "BehindStatue"] {
        assert!(cfg.is_exception(name), "{name}");
        assert!(!ActivationConfig::original().is_exception(name), "{name}");
    }
    assert_eq!(ORIGINAL_WINDOW_LOGICS, ["JumpSwitch"]);
    assert!(!BREAKABLE_LOGICS.contains(&"JumpSwitch"), "not a breakable");
    for name in ["TownGuard1", "TownGuard2", "Seagull", "BreakPlank"] {
        assert!(!cfg.is_exception(name), "{name}");
    }
}

#[test]
fn the_guards_and_the_seagull_are_enemy_classes_whose_data_holds_enemy_data() {
    for (g, name, human) in [
        (GameLogic::TownGuard1, "TownGuard1", true),
        (GameLogic::TownGuard2, "TownGuard2", true),
        (GameLogic::Seagull, "Seagull", false),
    ] {
        let class = enemy::class(g).expect("a class");
        assert_eq!(class.name, name);
        assert_eq!(class.human, human, "{name}");
    }
    for g in [
        GameLogic::BreakPlank,
        GameLogic::JumpSwitch,
        GameLogic::Statue,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::TownGuard(townguard::TownGuardData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Seagull(seagull::SeagullData {
            enemy: enemy_data.clone(),
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::BreakPlank(Default::default()),
        ObjectData::JumpSwitch(Default::default()),
        ObjectData::Statue(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
}

#[test]
fn the_fifth_levels_state_numbers_hold_the_guards_and_the_seagulls_states() {
    let block = townguard::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2140, 2159));
    assert_eq!(seagull::STATE_BLOCK, block);
    let named = [
        (EnemyState::Idle2, 2140, "Idle2"),
        (EnemyState::Block, 2141, "Block"),
        (EnemyState::Withdraw, 2142, "Withdraw"),
        (EnemyState::GenericHit, 2143, "GenericHit"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
    }
    for id in 2144..=2159 {
        assert_eq!(EnemyState::from_id(id), None, "{id} is free");
    }
    let in_block: Vec<_> = EnemyState::ALL
        .iter()
        .filter(|s| block.contains(&(**s as i32)))
        .copied()
        .collect();
    assert_eq!(in_block, named.map(|(s, _, _)| s));
    // The fourth level's block ends where the fifth's starts.
    assert_eq!(*enemy::katherine::STATE_BLOCK.end() + 1, *block.start());
    // The states the guards and the seagull share keep their numbers.
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::Dead as i32, 2006);
    assert_eq!(EnemyState::JumpBack as i32, 2030);
}

#[test]
fn the_fifth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(5, SurfaceKind::Elevator),
        Rect::new(-27, -5, 27, 67)
    );
    assert_eq!(
        platform::surface(5, SurfaceKind::TogglePeg),
        Rect::new(-27, -6, 25, 67)
    );
    assert_eq!(
        platform::surface(5, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(5, SurfaceKind::SteppingStone),
        Rect::new(0, 0, 0, 32),
        "no stepping stones"
    );
    // Spikes, not goo, on its death tiles.
    assert!(!goo::is_goo_level(5));
    assert_eq!(goo::splash_y(5), None);
    // The planks: 64 px apart, their floor 60 px above them.
    let p = break_plank::level_table(5).expect("planks");
    assert_eq!((p.spacing, p.row_offset), (64, -60));
    assert_eq!(break_plank::level_table(4), None);
    // The guards have 9 health on this level.
    assert_eq!(townguard::health(5), 9);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        "LEVEL_TOWNGUARD1",
        "LEVEL_TOWNGUARD2",
        seagull::IMAGE_SET,
        break_plank::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        townguard::HIGH_STRIKE_ANIM,
        townguard::LOW_STRIKE_ANIM,
        "LEVEL_TOWNGUARD1_IDLE1",
        "LEVEL_TOWNGUARD2_IDLE2",
        "LEVEL_SEAGULL_FLYING",
        "LEVEL_SEAGULL_DIVE3",
        break_plank::IDLE_ANIM,
        break_plank::CRUMBLE_ANIM,
        statue::BREAK_ANIM,
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

fn level5(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(5)).unwrap();
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

#[test]
fn install_level5_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level5(&store);
    assert_eq!(level.number, 5);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL5 inert census: {:?}", d.inert_logics);
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
    // Each declared family is on the level and reported inert until the
    // family is built.
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for (g, names) in DECLARED {
        assert!(
            names.iter().any(|n| placed(n) > 0),
            "LEVEL5 places the {g:?} family"
        );
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
    // No boss on this level, and nothing from the earlier levels' new
    // families.
    for name in [
        "BossStager",
        "BossWarp",
        "SpringBoard",
        "SteppingStone",
        "FloorSpike",
        "PathElevator",
        "AniRope",
        "CutThroat",
        "RobberThief",
        "PunkRat",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
    // The toggle pegs return, on this level's rectangle.
    let pegs: usize = ["TogglePeg", "TogglePeg2", "TogglePeg3", "TogglePeg4"]
        .iter()
        .map(|n| placed(n))
        .sum();
    assert_eq!(pegs, 35);
}

#[test]
fn install_level5_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level5(&store);
    let preloaded = level.image_sets();
    // Image sets with their frame ranges and the boxes the families read.
    for (set, frames) in [
        ("LEVEL_TOWNGUARD1", None),
        ("LEVEL_TOWNGUARD2", None),
        (seagull::IMAGE_SET, None),
        (break_plank::IMAGE_SET, Some((1, 12))),
        ("LEVEL_STATUE", Some((1, 10))),
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
    // The seagull's first picture, 64 x 60 px, which gives it its
    // movement box.
    assert_eq!(
        level.frame_box(seagull::IMAGE_SET, 1),
        Some(Rect::new(-33, -47, 30, 12))
    );
    assert_eq!(
        level.frame_box(break_plank::IMAGE_SET, 1),
        Some(Rect::new(-40, -88, 35, -25))
    );
    assert_eq!(
        level.frame_box("LEVEL_STATUE", 1),
        Some(Rect::new(-41, -83, 46, 82))
    );
    // The animations the families play.
    let mut want = vec![
        townguard::HIGH_STRIKE_ANIM.to_string(),
        townguard::LOW_STRIKE_ANIM.to_string(),
    ];
    for guard in ["LEVEL_TOWNGUARD1", "LEVEL_TOWNGUARD2"] {
        for a in [
            "ADVANCE",
            "FASTADVANCE",
            "KILLFALL",
            "IDLE1",
            "IDLE2",
            "HITLOW",
            "HITHIGH",
            "JUMP",
            "HOME",
            "LIFTED2",
            "THROWN",
        ] {
            want.push(format!("{guard}_{a}"));
        }
    }
    want.push("LEVEL_TOWNGUARD1_LIFTED1".to_string());
    for a in ["DIVE3", "FLYING", "HITHIGH", "HITLOW", "KILLFALL"] {
        want.push(format!("LEVEL_SEAGULL_{a}"));
    }
    want.push(break_plank::IDLE_ANIM.to_string());
    want.push(break_plank::CRUMBLE_ANIM.to_string());
    want.push(statue::BREAK_ANIM.to_string());
    for key in &want {
        assert!(level.animations.contains_key(key), "{key} is loaded");
    }
    // The second guard ships no first lift animation.
    assert!(!level.animations.contains_key("LEVEL_TOWNGUARD2_LIFTED1"));
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

/// The level's 38 animated pieces (signs and walkway caps) loop from
/// their record's depth, with their art loaded with the level.
#[test]
fn install_level5_animated_pieces_run_from_their_record() {
    let Some(store) = store() else { return };
    let level = level5(&store);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut cycles = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        if &*o.logic != "AniCycle" {
            continue;
        }
        let ObjectData::Decor(d) = &o.data else {
            panic!("set up: {:?}", o.data)
        };
        assert_eq!(o.state, decor::SET_UP, "{} is set up", o.image_set);
        let record = o.record.as_ref().expect("a placed piece");
        assert_eq!(o.z, record[7], "{} keeps its record's depth", o.image_set);
        assert!(d.anim.animation().is_some(), "{} is animated", o.image_set);
        *cycles.entry(o.image_set.clone()).or_insert(0) += 1;
    }
    println!("LEVEL5 animated pieces: {cycles:?}");
    assert_eq!(
        cycles,
        BTreeMap::from([
            ("LEVEL_SIGN_DOWN".to_string(), 6),
            ("LEVEL_SIGN_RIGHT".to_string(), 9),
            ("LEVEL_SIGN_UP".to_string(), 5),
            ("LEVEL_WALKENDCAP".to_string(), 18),
        ])
    );
    for set in cycles.keys() {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(level.frame_box(set, first).is_some(), "{set} has a box");
    }
}

#[test]
fn install_level5_switches_planks_and_statues_records() {
    let Some(store) = store() else { return };
    let level = level5(&store);

    // Seven switches, each with its target point and inside a first
    // guard's patrol.
    let mut switches: Vec<((i32, i32), (i32, i32))> = records(&level, "JumpSwitch")
        .iter()
        .map(|r| ((r.record[5], r.record[6]), jump_switch::target(&r.record)))
        .collect();
    switches.sort();
    assert_eq!(
        switches,
        [
            ((2402, 2652), (2227, 2646)),
            ((3865, 2862), (3745, 2893)),
            ((5066, 2831), (4944, 2878)),
            ((6478, 3279), (6301, 3007)),
            ((7280, 3313), (7051, 3233)),
            ((13273, 3023), (13432, 3099)),
            ((19054, 4276), (18855, 4417)),
        ]
    );
    let guards = records(&level, "TownGuard1");
    for &((x, y), _) in &switches {
        assert!(
            guards.iter().any(|g| {
                let (lo, hi) = (g.record[51], g.record[53]);
                (lo - 1..=hi + 1).contains(&x) && (31..=42).contains(&(y - g.record[6]))
            }),
            "switch {x},{y} lies in a guard's patrol"
        );
    }

    // Twenty-one plank rows, 111 pieces; each piece's floor is a one-way
    // floor cell but one, which is empty from the start.
    let planks = break_plank::level_table(5).unwrap();
    let rows = records(&level, "BreakPlank");
    let mut delays = BTreeMap::new();
    let mut tiles = BTreeMap::new();
    let mut empty = Vec::new();
    let mut pieces = 0;
    for r in &rows {
        let (x, y) = (r.record[5], r.record[6]);
        let (delay, width) = (r.record[59], r.record[61]);
        *delays.entry(delay).or_insert(0) += 1;
        assert_eq!(r.image_set, break_plank::IMAGE_SET);
        for i in 0..width.max(1) {
            pieces += 1;
            let px = x + i * planks.spacing;
            let (col, row) = break_plank::floor_cell(planks, px, y, level.map.tile_w());
            let tile = level.map.cell(col, row);
            *tiles.entry(tile).or_insert(0) += 1;
            if tile < 0 {
                empty.push((x, y, i, col, row));
            } else {
                // The surface is 28 px below the cell's top.
                let top = row * level.map.tile_h();
                assert_eq!(level.map.attr(px, top + 28), GROUND, "{px},{y}");
                assert_ne!(level.map.attr(px, top + 27), GROUND, "{px},{y}");
            }
        }
    }
    println!("LEVEL5 plank rows: delays {delays:?}, floor tiles {tiles:?}");
    assert_eq!(rows.len(), 21);
    assert_eq!(pieces, 111);
    assert_eq!(delays, BTreeMap::from([(250, 3), (500, 13), (750, 5)]));
    assert_eq!(tiles.len(), 2, "one floor tile, and the empty cell");
    assert_eq!(tiles.get(&-1), Some(&1));
    assert_eq!(empty, [(18727, 4825, 0, 292, 74)]);

    // Eight statues, in front of Claw or behind him by their depth, with
    // four to eight treasures each.
    let mut statues: Vec<(&str, i32, i32, i32, usize)> = ["FrontStatue", "BehindStatue"]
        .iter()
        .flat_map(|&name| records(&level, name).into_iter().map(move |r| (name, r)))
        .map(|(name, r)| {
            let loot = (35..=42).chain([15]).filter(|&i| r.record[i] != 0).count();
            (name, r.record[5], r.record[6], r.record[7], loot)
        })
        .collect();
    statues.sort_by_key(|s| (s.1, s.2));
    assert_eq!(
        statues,
        [
            ("BehindStatue", 2106, 2806, 1010, 8),
            ("FrontStatue", 2191, 2842, 5000, 8),
            ("BehindStatue", 6057, 3196, 1000, 8),
            ("BehindStatue", 11183, 3707, 1010, 4),
            ("BehindStatue", 15234, 4409, 1020, 4),
            ("BehindStatue", 15874, 4409, 1020, 6),
            ("BehindStatue", 27822, 5562, 1010, 8),
            ("FrontStatue", 27901, 5590, 5000, 8),
        ]
    );
    for (name, ..) in &statues {
        assert!(statue::drop_z_offset(name).is_some(), "{name}");
    }
    for name in ["FrontStatue", "BehindStatue"] {
        for r in records(&level, name) {
            assert_eq!(r.image_set, "LEVEL_STATUE");
        }
    }
}

#[test]
fn install_level5_has_no_springboards_and_every_enemy_sets_its_patrol() {
    let Some(store) = store() else { return };
    let level = level5(&store);
    assert!(records(&level, "SpringBoard").is_empty());
    // The table keeps the fern for this level, used by no record.
    assert!(springboard::level_table(5).is_some());
    // Every seagull and guard sets its patrol limits.
    for name in ["Seagull", "TownGuard1", "TownGuard2"] {
        for r in records(&level, name) {
            let (lo, hi) = (r.record[51], r.record[53]);
            assert!(
                lo > 0 && hi > lo,
                "{name} at {},{}",
                r.record[5],
                r.record[6]
            );
        }
    }
}
