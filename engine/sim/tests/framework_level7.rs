//! The framework the seventh level's families build on: the red-tail
//! pirates, the bear sailors, the hermit crabs with their nests and bombs
//! are declared, and each stays an inert placeholder until its family is
//! built; their classes, data and state numbers; Claw's held state for the
//! bear's hug; the level's platform, springboard and splash table; and, on
//! the install, the level's inert census, the art its families need
//! loaded with it, their records, and what the earlier families do with
//! the level (the water cover-ups, the rock springboards, the sinking
//! buoys, the air carts, the seagulls, the pegs and the animated pieces).
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here.
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
use claw_sim::logic::enemy::{self, EnemyState, bear_sailor, crab, field, goo, redtail_pirate};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::{
    self, GameLogic, ObjectData, boss_stage, decor, springboard, stepping_stone,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::player::{self, RowKind, hug, state};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the seventh level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 5] = [
    (GameLogic::RedTailPirate, &["RedTailPirate"]),
    (GameLogic::BearSailor, &["BearSailor"]),
    (GameLogic::HermitCrab, &["HermitCrab"]),
    (GameLogic::CrabNest, &["CrabNest"]),
    (GameLogic::CrabBomb, &["CrabBomb"]),
];

/// Logics the seventh level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the water cover-ups, the animated signs, the
/// `DoNothing` pictures), platform (the air cart elevators, the crumbling
/// pegs, the buoys), springboard, crate, pickup, checkpoint, sound object
/// and enemy the level places outside the declared families already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the seventh level places; the
/// bombs are thrown at run time, never placed.
const PLACED: [(&str, usize); 5] = [
    ("RedTailPirate", 34),
    ("BearSailor", 19),
    ("HermitCrab", 4),
    ("CrabNest", 13),
    ("CrabBomb", 0),
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
    assert_eq!(
        logic::built(GameLogic::RedTailPirate),
        redtail_pirate::BUILT
    );
    assert_eq!(logic::built(GameLogic::BearSailor), bear_sailor::BUILT);
    assert_eq!(logic::built(GameLogic::HermitCrab), crab::BUILT);
    assert_eq!(logic::built(GameLogic::CrabNest), crab::nest::BUILT);
    assert_eq!(logic::built(GameLogic::CrabBomb), crab::bomb::BUILT);
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
    // A built family's names run.
    for (g, list) in DECLARED.iter().filter(|(g, _)| logic::built(*g)) {
        for &n in *list {
            assert_eq!(logic::runs(n), Some(LogicKind::Game(*g)), "{n}");
        }
    }
    // Placed in a level, the unbuilt ones are drawn placeholders that
    // never collide, and the level load reports them as not implemented.
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
    // The pirates, the bears, the crabs and their bombs wake with the
    // enemies' window, the nests with the small objects'; none keeps the
    // original window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for (name, class) in [
        ("RedTailPirate", 0),
        ("BearSailor", 0),
        ("HermitCrab", 0),
        ("CrabNest", 2),
        ("CrabBomb", 0),
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
fn the_pirate_and_the_bear_are_human_classes_and_the_crab_a_plain_one() {
    for (g, name, human) in [
        (GameLogic::RedTailPirate, "RedTailPirate", true),
        (GameLogic::BearSailor, "BearSailor", true),
        (GameLogic::HermitCrab, "HermitCrab", false),
    ] {
        let class = enemy::class(g).expect("a class");
        assert_eq!(class.name, name);
        assert_eq!(class.human, human, "{name}");
    }
    for g in [GameLogic::CrabNest, GameLogic::CrabBomb] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::RedTailPirate(redtail_pirate::RedTailPirateData {
            enemy: enemy_data.clone(),
        }),
        ObjectData::BearSailor(bear_sailor::BearSailorData {
            enemy: enemy_data.clone(),
        }),
        ObjectData::HermitCrab(crab::HermitCrabData {
            enemy: enemy_data.clone(),
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::CrabNest(Default::default()),
        ObjectData::CrabBomb(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Health, damage and the hug's numbers.
    assert_eq!((redtail_pirate::HEALTH, redtail_pirate::DAMAGE), (10, 10));
    assert_eq!((bear_sailor::HEALTH, bear_sailor::DAMAGE), (10, 10));
    assert_eq!((crab::DAMAGE, crab::bomb::DAMAGE), (5, 5));
    assert_eq!(bear_sailor::HUG_X, 30);
    assert_eq!(bear_sailor::ESCAPE_PRESSES, 10);
    assert_eq!(
        bear_sailor::HELD_FRAMES.map(|(_, claw)| claw),
        hug::HELD_FRAMES
    );
}

#[test]
fn the_seventh_levels_state_numbers_hold_the_hug_and_the_crabs() {
    let block = bear_sailor::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2180, 2199));
    assert_eq!(redtail_pirate::STATE_BLOCK, block);
    assert_eq!(crab::STATE_BLOCK, block);
    let named = [
        (EnemyState::StartHug, 2180, "StartHug"),
        (EnemyState::Hug, 2181, "Hug"),
        (EnemyState::Release, 2182, "Release"),
        (EnemyState::GabrielAttack, 2183, "GabrielAttack"),
        (EnemyState::Regroup, 2190, "Regroup"),
        (EnemyState::Hide, 2191, "Hide"),
        (EnemyState::Defense, 2192, "Defense"),
        (EnemyState::Attack, 2193, "Attack"),
        (EnemyState::MovingAttack, 2194, "MovingAttack"),
        (EnemyState::Snip, 2195, "Snip"),
        (EnemyState::Tumble, 2196, "Tumble"),
        (EnemyState::Nudge, 2197, "Nudge"),
        (EnemyState::Launch, 2198, "Launch"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
    }
    for id in (2184..=2189).chain([2199]) {
        assert_eq!(EnemyState::from_id(id), None, "{id} is free");
    }
    let in_block: Vec<_> = EnemyState::ALL
        .iter()
        .filter(|s| block.contains(&(**s as i32)))
        .copied()
        .collect();
    assert_eq!(in_block, named.map(|(s, _, _)| s));
    assert_eq!(&in_block[4..], crab::STATES);
    // The sixth level's block ends where the seventh's starts.
    assert_eq!(*enemy::wolvington::STATE_BLOCK.end() + 1, *block.start());
    // The shared states they reach keep their numbers.
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::HitLow as i32, 2004);
    assert_eq!(EnemyState::Dead as i32, 2006);
    assert_eq!(EnemyState::JumpBack as i32, 2030);
    assert_eq!(EnemyState::MoveBack as i32, 2031);
    assert_eq!(EnemyState::Block as i32, 2141);
}

#[test]
fn the_hug_state_is_declared_and_inert_until_built() {
    let row = player::row_of(state::BEARHUG);
    assert_eq!((row.id, row.name), (5008, "Bearhug"));
    if hug::BUILT {
        return;
    }
    assert_eq!(row.kind, RowKind::Fallback);
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    sim.set_player_state(state::BEARHUG);
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.claw().fallback_entries, 1);
}

#[test]
fn the_seventh_levels_table() {
    // Platform rectangles, each with its 32 px depth: the air carts ride
    // 70 px below their position.
    assert_eq!(
        platform::surface(7, SurfaceKind::Elevator),
        Rect::new(-60, 70, 74, 142)
    );
    assert_eq!(
        platform::surface(7, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(7, SurfaceKind::TogglePeg),
        Rect::new(0, 0, 0, 32),
        "no toggle pegs"
    );
    // The buoys.
    assert_eq!(
        platform::surface(7, SurfaceKind::SteppingStone),
        Rect::new(-7, -10, 22, 62)
    );
    assert_eq!(stepping_stone::surface(7), Rect::new(-7, -10, 22, 62));
    // Springboards: the rocks, 89 px below their position.
    let s = springboard::level_table(7).expect("rocks");
    assert_eq!(s.rect, (-24, 89, 23, 130));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_ROCKSPRING_IDLE", "LEVEL_ROCKSPRING_SPRING")
    );
    assert_eq!(springboard::surface(7), Rect::new(-24, 89, 23, 162));
    // Water death tiles, which kill as goo does, with a 60 px splash.
    assert!(goo::is_goo_level(7));
    assert_eq!(goo::splash_y(7), Some(60));
    // The pirates never parry here.
    assert_eq!(redtail_pirate::block_chances(7), None);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        redtail_pirate::IMAGE_SET,
        bear_sailor::IMAGE_SET,
        crab::IMAGE_SET,
        crab::BOMBER_IMAGE_SET,
        crab::bomb::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        redtail_pirate::HIGH_STRIKE_ANIM,
        redtail_pirate::LOW_STRIKE_ANIM,
        bear_sailor::STRIKE_ANIM,
        bear_sailor::GRAB_ANIM,
        bear_sailor::SQUEEZE_ANIM,
        crab::SNIP_ANIM,
        crab::DEFENSE_ANIM,
        crab::HIDE_ANIM,
        crab::nest::HATCH_ANIM,
        "LEVEL_CRABBOMB_FALL",
        "LEVEL_CRABBOMB_EXPLODE",
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

/// The picture each of the first `n` calls of a looping animation shows
/// at 18 ms.
fn pictures(level: &SimLevel, key: &str, n: usize) -> Vec<i32> {
    let anim = level.animations[key].clone();
    let mut p = AnimPlayer::default();
    p.set(anim.clone());
    let mut shown = Vec::new();
    let mut current = None;
    for _ in 0..n {
        if let Some(i) = p.advance_in(18, 0, false, None).applied {
            current = Some(anim.frames[i].image_index);
        }
        shown.push(current.expect("the first call applies a picture"));
    }
    shown
}

#[test]
fn install_level7_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 7);
    assert_eq!(level.number, 7);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL7 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 3315, "the level's records");

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
    // The earlier slices' families the level reuses, all running.
    for (name, n) in [
        ("GooCoverup", 604),
        ("Seagull", 67),
        ("SpringBoard", 34),
        ("StartSteppingStone", 26),
        ("SteppingStone", 6),
        ("PathElevator", 14),
        ("Elevator", 1),
        ("AniCycle", 39),
        ("CrumblingPeg", 70),
        ("EndOfLevelPowerup", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // No boss, no stage, and none of the earlier levels' enemies or
    // hazards the seventh does not use: the end-of-level pickup ends it.
    for name in [
        boss_stage::STAGER,
        boss_stage::WARP,
        "Officer",
        "Soldier",
        "Rat",
        "PunkRat",
        "CutThroat",
        "RobberThief",
        "TownGuard1",
        "TownGuard2",
        "Raux",
        "Katherine",
        "Wolvington",
        "PowderKeg",
        "FloorSpike",
        "AniRope",
        "BreakPlank",
        "JumpSwitch",
        "TogglePeg",
        "GooVent",
        "GroundBlower",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level7_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 7);
    let preloaded = level.image_sets();
    for set in [
        redtail_pirate::IMAGE_SET,
        bear_sailor::IMAGE_SET,
        crab::IMAGE_SET,
        crab::BOMBER_IMAGE_SET,
        crab::bomb::IMAGE_SET,
        crab::nest::IMAGE_SET,
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, last) = level.image_frames(set).expect("registered");
        assert!(last >= first, "{set}");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The crab's boxes come from its first picture, and the bomber's has
    // the same.
    for set in [crab::IMAGE_SET, crab::BOMBER_IMAGE_SET] {
        assert_eq!(level.frame_box(set, 1), Some(crab::HIT_RECT), "{set}");
    }
    // The crab's shell pictures and its throw and snip pictures.
    for f in crab::SHELL_FRAMES
        .into_iter()
        .chain([crab::THROW_FRAME])
        .chain(crab::SNIP_FRAMES)
    {
        assert!(level.frame_box(crab::IMAGE_SET, f).is_some(), "{f}");
    }
    // Claw's pictures while held.
    for f in hug::HELD_FRAMES {
        assert!(level.frame_box(player::IMAGE_SET, f).is_some(), "{f}");
    }

    // The pirate's animations and their timings at 18 ms.
    for a in [
        "ADVANCE",
        "BLOCK",
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "HOME",
        "IDLE",
        "JUMP",
        "KILLFALL",
        "LIFTED1",
        "LIFTED2",
        "QUICKBLOCK",
        "STRIKE1",
        "STRIKE2",
        "THROWN",
    ] {
        let key = format!("LEVEL_REDTAILPIRATE_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    // Calls to finish, and the first call that reports the blow.
    let strike = |key: &str| {
        let (n, events) = calls(&level, key);
        println!("{key}: done on call {n}, blow on calls {events:?}");
        (n, events.first().copied())
    };
    // The high strike: live from call 7, done on 18; the low one: live
    // from call 7, done on 14.
    assert_eq!(strike(redtail_pirate::HIGH_STRIKE_ANIM), (18, Some(7)));
    assert_eq!(strike(redtail_pirate::LOW_STRIKE_ANIM), (14, Some(7)));
    assert_eq!(
        calls(&level, "LEVEL_REDTAILPIRATE_HITLOW").0,
        redtail_pirate::HIT_LOW_TICKS as usize
    );
    assert_eq!(calls(&level, "LEVEL_REDTAILPIRATE_JUMP").0, 10);

    // The bear's: the blow live from call 16 and done on 35, the grab
    // done on 19, the squeeze a loop of 31 calls.
    for a in [
        "ADVANCE",
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "HOME",
        "IDLE",
        "JUMP",
        "KILLFALL",
        "LIFTED1",
        "LIFTED2",
        "STRIKE1",
        "STRIKE2",
        "STRIKE3",
        "THROWN",
    ] {
        let key = format!("LEVEL_BEARSAILOR_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    assert_eq!(strike(bear_sailor::STRIKE_ANIM), (35, Some(16)));
    assert_eq!(
        calls(&level, bear_sailor::GRAB_ANIM),
        (bear_sailor::GRAB_TICKS as usize, vec![])
    );
    let squeeze = &level.animations[bear_sailor::SQUEEZE_ANIM];
    assert!(squeeze.frames.iter().all(|f| f.next_op == 0), "it loops");
    let shown = pictures(&level, bear_sailor::SQUEEZE_ANIM, 62);
    let mut cycle = vec![304; 8];
    cycle.extend([303; 7]);
    cycle.extend([302; 9]);
    cycle.extend([303; 7]);
    assert_eq!(shown[..31], cycle[..]);
    assert_eq!(shown[31..], cycle[..]);
    assert!(shown.iter().all(|&f| bear_sailor::held_frame(f).is_some()));

    // The crab's: the snip live on calls 7 to 14 and done on 15, the
    // crouch done on 16; the nest's hatch shows its second picture on
    // call 4 and is done on 16; the bomb's blast is done on 16.
    for a in [
        "CROUCH4",
        "FALL",
        "FASTADVANCE",
        "HIDE",
        "HIT",
        "KILLFALL",
        "LIFT",
        "STRIKE1",
    ] {
        let key = format!("LEVEL_HERMITCRAB_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    assert_eq!(calls(&level, crab::SNIP_ANIM).0, 15);
    assert_eq!(calls(&level, crab::DEFENSE_ANIM).0, 16);
    assert_eq!(
        calls(&level, crab::nest::HATCH_ANIM).0,
        crab::nest::DONE_TICK as usize
    );
    let hatch = pictures(&level, crab::nest::HATCH_ANIM, 6);
    assert_eq!(
        hatch.iter().position(|&f| f == 2),
        Some(crab::nest::SPAWN_TICK as usize - 1)
    );
    assert_eq!(
        calls(&level, "LEVEL_CRABBOMB_EXPLODE").0,
        crab::bomb::EXPLODE_TICKS as usize
    );
    assert!(level.animations.contains_key("LEVEL_CRABBOMB_FALL"));
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level7_enemy_and_nest_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 7);
    for (name, set, z) in [
        ("RedTailPirate", redtail_pirate::IMAGE_SET, 30),
        ("BearSailor", bear_sailor::IMAGE_SET, 0),
    ] {
        let rs = records(&level, name);
        for r in &rs {
            assert_eq!(r.image_set, set, "{name}");
            assert_eq!(r.record[7], z, "{name} depth");
            assert_eq!(r.record[field::USER_0], 0, "{name} never stands still");
            let (lo, hi) = (r.record[field::X_MIN], r.record[field::X_MAX]);
            assert!(lo > 0 && hi > lo, "{name} has patrol bounds");
            assert!((lo..=hi).contains(&r.record[5]), "{name} starts inside");
        }
    }
    // The crabs placed by the level: plain crabs, none a bomber.
    for r in records(&level, "HermitCrab") {
        assert_eq!(r.image_set, crab::IMAGE_SET);
        assert_eq!(r.record[field::USER_0], 0, "no bomber");
    }
    // Every nest hatches three crabs; one leaves its box to its picture.
    let nests = records(&level, "CrabNest");
    let mut empty_boxes = 0;
    for r in &nests {
        assert_eq!(r.image_set, crab::nest::IMAGE_SET);
        assert_eq!(crab::nest::crab_count(r.record[field::USER_0]), 3);
        let b = [
            r.record[field::X_MIN],
            r.record[field::Y_MIN],
            r.record[field::X_MAX],
            r.record[field::Y_MAX],
        ];
        if b == [0; 4] {
            empty_boxes += 1;
        }
    }
    assert_eq!(empty_boxes, 1, "one nest uses its picture's box");
}

#[test]
fn install_level7_reused_families_take_the_seventh_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 7);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut covers: BTreeMap<(String, i32), usize> = BTreeMap::new();
    let mut springs: BTreeMap<String, usize> = BTreeMap::new();
    let mut spring_heights: BTreeMap<i32, usize> = BTreeMap::new();
    let mut stones: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut carts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut pegs: BTreeMap<String, usize> = BTreeMap::new();
    let mut cycles: BTreeMap<String, usize> = BTreeMap::new();
    let mut seagulls = 0;
    for (_, o) in sim.objects().all() {
        let record = o.record.as_ref();
        match &*o.logic {
            "GooCoverup" => {
                assert_eq!(o.state, decor::SET_UP);
                *covers.entry((o.image_set.clone(), o.z)).or_default() += 1;
            }
            "SpringBoard" => {
                let ObjectData::SpringBoard(d) = &o.data else {
                    panic!("set up: {:?}", o.data)
                };
                assert_eq!(d.anims, springboard::level_table(7));
                *springs.entry(o.image_set.clone()).or_default() += 1;
                *spring_heights.entry(d.height).or_default() += 1;
            }
            "StartSteppingStone" | "SteppingStone" => {
                assert_eq!(o.hit_rect, stepping_stone::surface(7), "{}", o.logic);
                *stones
                    .entry((o.logic.to_string(), o.image_set.clone()))
                    .or_default() += 1;
            }
            "PathElevator" | "Elevator" => {
                assert_eq!(
                    o.hit_rect,
                    platform::surface(7, SurfaceKind::Elevator),
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
                *carts
                    .entry((o.logic.to_string(), o.image_set.clone()))
                    .or_default() += 1;
            }
            "CrumblingPeg" => {
                *pegs.entry(o.image_set.clone()).or_default() += 1;
            }
            "AniCycle" => {
                assert_eq!(o.z, record.unwrap()[7], "the record's depth");
                let ObjectData::Decor(d) = &o.data else {
                    panic!("set up: {:?}", o.data)
                };
                assert!(d.anim.animation().is_some(), "{} is animated", o.image_set);
                *cycles.entry(o.image_set.clone()).or_default() += 1;
            }
            "Seagull" => {
                assert_eq!(o.kind, LogicKind::Game(GameLogic::Seagull));
                assert!(o.data.enemy().is_some(), "built");
                seagulls += 1;
            }
            _ => {}
        }
    }
    println!(
        "LEVEL7 cover-ups {covers:?}, springboards {springs:?} {spring_heights:?}, \
         stones {stones:?}, carts {carts:?}, pegs {pegs:?}, animated pieces {cycles:?}"
    );
    // 604 water cover-ups placed at depth 0, drawn at the cover-ups'
    // default depth.
    assert_eq!(
        covers,
        BTreeMap::from([(
            ("LEVEL_WATERCOVERUP".to_string(), decor::GOO_COVERUP_Z),
            604
        )])
    );
    // 34 rock springboards, three of them throwing 300 px.
    assert_eq!(
        springs,
        BTreeMap::from([("LEVEL_WATERROCK".to_string(), 34)])
    );
    assert_eq!(spring_heights.get(&300), Some(&3));
    assert_eq!(spring_heights.values().sum::<usize>(), 34);
    // 32 sinking buoys.
    assert_eq!(
        stones,
        BTreeMap::from([
            (
                (
                    "StartSteppingStone".to_string(),
                    "LEVEL_SINKBOEY".to_string()
                ),
                26
            ),
            (
                ("SteppingStone".to_string(), "LEVEL_SINKBOEY".to_string()),
                6
            ),
        ])
    );
    // 15 air carts.
    assert_eq!(
        carts,
        BTreeMap::from([
            (("Elevator".to_string(), "LEVEL_AIRCART".to_string()), 1),
            (
                ("PathElevator".to_string(), "LEVEL_AIRCART".to_string()),
                14
            ),
        ])
    );
    assert_eq!(
        pegs,
        BTreeMap::from([("LEVEL_CRUMBLINPEG".to_string(), 70)])
    );
    assert_eq!(cycles.values().sum::<usize>(), 39);
    assert_eq!(seagulls, 67);
    for set in covers
        .keys()
        .map(|k| &k.0)
        .chain(springs.keys())
        .chain(stones.keys().map(|k| &k.1))
        .chain(carts.keys().map(|k| &k.1))
        .chain(pegs.keys())
        .chain(cycles.keys())
    {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    let rocks = springboard::level_table(7).expect("rocks");
    for anim in [rocks.idle_anim, rocks.spring_anim] {
        assert!(level.animations.contains_key(anim), "{anim} is loaded");
    }
}
