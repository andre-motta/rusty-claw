//! The framework the third level's families build on: the families that
//! are declared but not built yet stay inert placeholders, the thieves'
//! classes and data, the images of their knife and arrow, the enemy state
//! numbers reserved for the level, and Claw's Swing state, which returns
//! him to standing when he has no rope to hang from.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, cutthroat, robberthief};
use claw_sim::logic::{self, GameLogic, ObjectData, projectile};
use claw_sim::object::LogicKind;
use claw_sim::player::{self, RowKind, state};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the third level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 5] = [
    (GameLogic::CutThroat, &["CutThroat"]),
    (GameLogic::RobberThief, &["RobberThief"]),
    (
        GameLogic::FloorSpike,
        &["FloorSpike", "FloorSpike2", "FloorSpike3", "FloorSpike4"],
    ),
    (GameLogic::PathElevator, &["PathElevator"]),
    (GameLogic::Rope, &["AniRope"]),
];

/// Logics the third level leaves inert besides the declared families, each
/// with the reason it may stay so: none. Every decoration, platform,
/// pickup, sound object and enemy the level places outside the declared
/// families already runs.
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

fn declared_names() -> Vec<&'static str> {
    DECLARED
        .iter()
        .flat_map(|(_, n)| n.iter().copied())
        .collect()
}

#[test]
fn every_name_belongs_to_one_family() {
    let mut seen = std::collections::BTreeSet::new();
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
    // The families not built yet (each one leaves this list as it is built).
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
    // Thieves and ropes wake with the enemies' window, path elevators with
    // the elevators', floor spikes with the small hazards'.
    for (name, class) in [
        ("CutThroat", 0),
        ("RobberThief", 0),
        ("AniRope", 0),
        ("PathElevator", 1),
        ("FloorSpike", 2),
        ("FloorSpike2", 2),
        ("FloorSpike3", 2),
        ("FloorSpike4", 2),
    ] {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
    }
}

#[test]
fn the_thieves_are_human_classes_whose_data_holds_enemy_data() {
    for (g, name) in [
        (GameLogic::CutThroat, "CutThroat"),
        (GameLogic::RobberThief, "RobberThief"),
    ] {
        let class = enemy::class(g).expect("a class");
        assert_eq!(class.name, name);
        assert!(class.human, "{name} is a human enemy");
    }
    for g in [
        GameLogic::FloorSpike,
        GameLogic::PathElevator,
        GameLogic::Rope,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    let wrapped = [
        ObjectData::CutThroat(cutthroat::CutThroatData {
            enemy: enemy_data.clone(),
        }),
        ObjectData::RobberThief(robberthief::RobberThiefData {
            enemy: enemy_data.clone(),
        }),
    ];
    for mut d in wrapped {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::FloorSpike(Default::default()),
        ObjectData::PathElevator(Default::default()),
        ObjectData::Rope(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
}

#[test]
fn the_knife_and_the_arrow_are_shots_with_their_own_images() {
    assert_eq!(cutthroat::KNIFE_LOGIC, "Bullet");
    assert_eq!(robberthief::ARROW_LOGIC, "Bullet");
    let sets = logic::runtime_image_sets();
    for image in [cutthroat::KNIFE_IMAGE, robberthief::ARROW_IMAGE] {
        assert!(projectile::IMAGE_SETS.contains(&image), "{image}");
        assert!(sets.contains(&image), "{image}");
        assert_eq!(claw_sim::level::runtime_image_set(image), image);
    }
    assert!(projectile::NAMES.contains(&"Bullet"));
}

#[test]
fn the_third_levels_state_numbers_are_reserved_and_empty() {
    let block = cutthroat::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2100, 2119));
    for s in EnemyState::ALL {
        assert!(!block.contains(&(s as i32)), "{} is in the block", s.name());
    }
    for id in block {
        assert_eq!(EnemyState::from_id(id), None, "{id} is free");
    }
    // The thieves run their own handlers under the shared numbers.
    for s in [
        EnemyState::Idle,
        EnemyState::Home,
        EnemyState::Strike,
        EnemyState::Fire,
    ] {
        assert!(EnemyState::from_id(s as i32).is_some());
    }
}

#[test]
fn swing_is_declared_and_without_a_rope_returns_claw_to_standing() {
    let row = player::row_of(state::SWING);
    assert_eq!((row.id, row.name), (5007, "Swing"));
    assert_eq!(row.kind, RowKind::Fallback);
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    sim.set_player_state(state::SWING);
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.claw().fallback_entries, 1);
}

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

#[test]
fn install_level3_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let data = LevelData::build(&store, LevelSource::Retail(3)).unwrap();
    let level = SimLevel::from_level(&data, &store).unwrap();
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL3 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");

    let placed = |name: &str| level.objects.iter().filter(|o| o.logic == name).count();
    // Each declared family's names are on the level, and reported inert
    // until the family is built.
    let mut want: BTreeMap<String, usize> = BTreeMap::new();
    for name in declared_names() {
        let n = placed(name);
        assert!(n > 0, "LEVEL3 places {name}");
        if logic::runs(name).is_none() {
            want.insert(name.to_string(), n);
        }
    }
    for &(name, _) in INERT_OTHERS {
        want.insert(name.to_string(), placed(name));
    }
    assert_eq!(
        d.inert_logics, want,
        "the logics the level leaves inert are the declared families and the listed ones"
    );
}
