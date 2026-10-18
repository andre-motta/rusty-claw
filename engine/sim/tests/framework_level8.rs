//! The framework the eighth level's families build on: Gabriel with his
//! bombs, the cannon switch, the cannon button and Gabriel's cannon are
//! declared, and each stays an inert placeholder until its family is
//! built; Gabriel's class, data, link and state numbers, and the cannon's
//! turn; the pirates' boss fight hooks; the level's platform, springboard
//! and splash table; and, on the install, the level's inert census, the
//! boss room's records, its boss stage bound to the shared stage pieces,
//! the art its families need loaded with it, and what the earlier families
//! do with the level (the water cover-ups, the toggle pegs, the buoys, the
//! sinking pots, the ropes, the seagulls, the pirates, the bears, the punk
//! rats and the kegs).
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant (and the pirates' boss
//! fight against [`redtail_pirate::LEVEL8_BUILT`]), so building a family
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
use claw_sim::logic::cannon_switch::{self, button, cannon};
use claw_sim::logic::enemy::{self, EnemyState, field, gabriel, goo, redtail_pirate};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{
    self, GameLogic, ObjectData, boss_stage, decor, keg, rope, springboard, stepping_stone,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The record field after the horizontal speed: the vertical speed.
const SPEED_Y: usize = field::SPEED_X + 1;

/// The families declared for the eighth level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 5] = [
    (GameLogic::Gabriel, &["Gabriel"]),
    (GameLogic::GabrielBomb, &["GabrielBomb"]),
    (GameLogic::CannonSwitch, &["CannonSwitch"]),
    (GameLogic::CannonButton, &["CannonButton"]),
    (GameLogic::GabrielCannon, &["GabrielCannon"]),
];

/// Logics the eighth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the water cover-ups, the start sign, the
/// `DoNothing` pictures), platform (the toggle pegs, the sinking pots, the
/// buoys), rope, crate, keg, pickup, checkpoint, sound object and enemy
/// the level places outside the declared families already runs, and so do
/// the door and the stager of its boss stage.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the eighth level places; the
/// bombs are lobbed at run time, never placed.
const PLACED: [(&str, usize); 5] = [
    ("Gabriel", 1),
    ("GabrielBomb", 0),
    ("CannonSwitch", 1),
    ("CannonButton", 1),
    ("GabrielCannon", 1),
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
    assert_eq!(logic::built(GameLogic::Gabriel), gabriel::BUILT);
    assert_eq!(logic::built(GameLogic::GabrielBomb), gabriel::bomb::BUILT);
    assert_eq!(logic::built(GameLogic::CannonSwitch), cannon_switch::BUILT);
    assert_eq!(logic::built(GameLogic::CannonButton), button::BUILT);
    assert_eq!(logic::built(GameLogic::GabrielCannon), cannon::BUILT);
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
    // Gabriel wakes with the large window, his bombs and his props with
    // the enemies'; none keeps the original window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for (name, class) in [
        ("Gabriel", 1),
        ("GabrielBomb", 0),
        ("CannonSwitch", 0),
        ("CannonButton", 0),
        ("GabrielCannon", 0),
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
// The data literal below ends in `..Default::default()` so it keeps
// compiling when Gabriel gains fields of his own.
#[allow(clippy::needless_update)]
fn gabriel_is_a_human_class_whose_data_holds_enemy_data() {
    let class = enemy::class(GameLogic::Gabriel).expect("a class");
    assert_eq!(class.name, "Gabriel");
    assert!(class.human, "Gabriel is a human enemy");
    for g in [
        GameLogic::GabrielBomb,
        GameLogic::CannonSwitch,
        GameLogic::CannonButton,
        GameLogic::GabrielCannon,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    let mut d = ObjectData::Gabriel(gabriel::GabrielData {
        enemy: enemy_data.clone(),
        ..Default::default()
    });
    assert_eq!(d.enemy(), Some(&enemy_data));
    d.enemy_mut().unwrap().roll = 5;
    assert_eq!(d.enemy().unwrap().roll, 5);
    for d in [
        ObjectData::GabrielBomb(Default::default()),
        ObjectData::CannonSwitch(Default::default()),
        ObjectData::CannonButton(Default::default()),
        ObjectData::GabrielCannon(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // His stage is the shared one.
    for name in gabriel::STAGE {
        assert!(boss_stage::NAMES.contains(&name), "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    assert_eq!(gabriel::METER_MAX, 100);
    assert_eq!(gabriel::CANNON_HIT_DAMAGE * gabriel::CANNON_HITS, 100);
    assert_eq!(gabriel::bomb::DAMAGE, 5);
    assert_eq!(gabriel::bomb::BLAST, Rect::new(-50, -50, 50, 50));
}

#[test]
fn the_link_is_there_only_with_gabriel_built() {
    // No Gabriel: no link, and nobody fights.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 2);
    sim.with_world(|w| {
        assert_eq!(w.gabriel_link(), None);
        assert!(w.gabriel_link_mut().is_none());
        assert!(!w.gabriel_fights());
        assert!(!redtail_pirate::in_gabriel_fight(w));
    });
    // Gabriel placed beside Claw: built, he is the current boss and holds
    // the link, the fight not yet won; otherwise still none.
    let mut sim = Sim::new(level_with(&["Gabriel"]), SimConfig::default());
    idle(&mut sim, 3);
    sim.with_world(|w| {
        if gabriel::BUILT {
            assert!(w.current_boss().is_some(), "he is the boss");
            let link = w.gabriel_link().expect("his link");
            assert!(!link.defeated);
            assert!(w.gabriel_fights());
        } else {
            assert_eq!(w.current_boss(), None);
            assert_eq!(w.gabriel_link(), None);
            assert!(!w.gabriel_fights());
        }
    });
}

#[test]
fn the_eighth_levels_state_numbers_hold_gabriels_states() {
    let block = gabriel::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2200, 2219));
    let named = [
        (EnemyState::MultiBomb, 2200, "MultiBomb"),
        (EnemyState::ThugAttack, 2201, "ThugAttack"),
        (EnemyState::CannonAttack, 2202, "CannonAttack"),
        (EnemyState::CannonHit, 2203, "CannonHit"),
        (EnemyState::Blocked, 2204, "Blocked"),
        (EnemyState::DeadAct1, 2205, "DeadAct1"),
        (EnemyState::DeadAct2, 2206, "DeadAct2"),
        (EnemyState::DeadAct3, 2207, "DeadAct3"),
        (EnemyState::DeadAct4, 2208, "DeadAct4"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(gabriel::STATES, named.map(|(s, _, _)| s));
    // The seventh level's block ends where the eighth's starts.
    assert_eq!(*enemy::bear_sailor::STATE_BLOCK.end() + 1, *block.start());
    // The cannon's turn sits in the cannons' block, after their upward
    // fire.
    assert_eq!(
        (EnemyState::Rotate as i32, EnemyState::Rotate.name()),
        (2061, "Rotate")
    );
    assert_eq!(EnemyState::from_id(2061), Some(EnemyState::Rotate));
    assert_eq!(EnemyState::VertFiring as i32, 2060);
    // The shared states he and his cannon reach keep their numbers.
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Fire as i32, 2013);
    assert_eq!(EnemyState::Lifted as i32, 2017);
    assert_eq!(EnemyState::Thrown as i32, 2019);
    assert_eq!(EnemyState::GabrielAttack as i32, 2183);
    // The props that are no enemies keep states of their own.
    assert_ne!(cannon_switch::state::IDLE, cannon_switch::state::SWITCH);
    assert_eq!(EnemyState::from_id(cannon_switch::state::SWITCH), None);
    assert_eq!(EnemyState::from_id(button::state::IDLE), None);
}

#[test]
fn the_eighth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(8, SurfaceKind::Elevator),
        Rect::new(-44, -8, 46, 64)
    );
    assert_eq!(
        platform::surface(8, SurfaceKind::TogglePeg),
        Rect::new(-24, -8, 32, 64)
    );
    assert_eq!(
        platform::surface(8, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    // The buoys.
    assert_eq!(
        platform::surface(8, SurfaceKind::SteppingStone),
        Rect::new(-24, -10, 30, 62)
    );
    assert_eq!(stepping_stone::surface(8), Rect::new(-24, -10, 30, 62));
    // Springboards: the fern, which no record places.
    let s = springboard::level_table(8).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING")
    );
    // Water death tiles, which kill as goo does, with no splash height.
    assert!(goo::is_goo_level(8));
    assert_eq!(goo::splash_y(8), Some(0));
    // The pirates parry a quarter of Claw's sword blows and block a
    // quarter of his shots, once the level's rules are built.
    assert_eq!(redtail_pirate::block_chances(8), Some((25, 25)));
    assert_eq!(
        redtail_pirate::blocks_on(8),
        redtail_pirate::LEVEL8_BUILT.then_some((25, 25))
    );
    assert_eq!(redtail_pirate::blocks_on(7), None);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        gabriel::IMAGE_SET,
        gabriel::bomb::IMAGE_SET,
        cannon_switch::IMAGE_SET,
        button::IMAGE_SET,
        cannon::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        gabriel::IDLE1_ANIM,
        gabriel::IDLE2_ANIM,
        gabriel::THUG_ANIM,
        gabriel::STRIKE_ANIM,
        gabriel::BLOCKED_ANIM,
        gabriel::CANNON_HIT_ANIM,
        gabriel::DEATH_ANIMS[3],
        gabriel::bomb::FALL_ANIM,
        gabriel::bomb::EXPLODE_ANIM,
        cannon_switch::SWITCH_ANIM,
        button::IDLE_ANIM,
        cannon::FIRE_ANIM,
        cannon::VERT_FIRE_ANIM,
        cannon::RISE_ANIM,
        cannon::REST_ANIM,
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

/// The one record of `name`.
fn record<'a>(level: &'a SimLevel, name: &str) -> &'a ObjectSpec {
    let rs = records(level, name);
    assert_eq!(rs.len(), 1, "{name}");
    rs[0]
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

/// The picture each of the first `n` calls of an animation shows at
/// 18 ms.
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

/// Lengths of the runs of equal values in `v`.
fn runs(v: &[i32]) -> Vec<(i32, usize)> {
    let mut out: Vec<(i32, usize)> = Vec::new();
    for &x in v {
        match out.last_mut() {
            Some((y, n)) if *y == x => *n += 1,
            _ => out.push((x, 1)),
        }
    }
    out
}

#[test]
fn install_level8_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 8);
    assert_eq!(level.number, 8);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL8 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 2704, "the level's records");

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
        ("GooCoverup", 704),
        ("Seagull", 73),
        ("RedTailPirate", 41),
        ("BearSailor", 40),
        ("PunkRat", 20),
        ("StartSteppingStone", 37),
        ("AniRope", 29),
        ("TogglePeg", 16),
        ("TogglePeg2", 10),
        ("TogglePeg3", 6),
        ("TogglePeg4", 3),
        ("Elevator", 3),
        ("OneWayTriggerElevator", 1),
        ("AniCycle", 1),
        ("PowderKeg", 105),
        (boss_stage::STAGER, 1),
        (boss_stage::WARP, 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // None of the earlier levels' enemies or hazards the eighth does not
    // use, no tower cannons, and no end-of-level pickup: the boss ends the
    // level.
    for name in [
        "Officer",
        "Soldier",
        "Rat",
        "CutThroat",
        "RobberThief",
        "TownGuard1",
        "TownGuard2",
        "HermitCrab",
        "CrabNest",
        "Raux",
        "Katherine",
        "Wolvington",
        "TowerCannonLeft",
        "TowerCannonRight",
        "FloorSpike",
        "CrumblingPeg",
        "SpringBoard",
        "SteppingStone",
        "PathElevator",
        "BreakPlank",
        "JumpSwitch",
        "FrontStatue",
        "BehindStatue",
        "GooVent",
        "GroundBlower",
        "EndOfLevelPowerup",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level8_boss_room_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 8);
    let at = |r: &ObjectSpec| (r.record[5], r.record[6], r.record[7]);
    // Gabriel on his deck, facing left, the gem's target to his left.
    let g = record(&level, "Gabriel");
    assert_eq!(at(g), (43223, 5068, 2000));
    assert_eq!(g.record[11], 0, "faces left");
    assert_eq!((g.record[field::SPEED_X], g.record[SPEED_Y]), (43036, 5028));
    // His record's health is not his health.
    assert_eq!(g.record[18], 0);
    // The props, all at depth 1000; the cannon and the button mirrored.
    let c = record(&level, "GabrielCannon");
    assert_eq!(at(c), (43203, 5184, 1000));
    assert_eq!(c.record[11], 2, "mirrored");
    let (along, up) = cannon::muzzles(true);
    assert_eq!(
        (c.record[5] + along.0, c.record[6] + along.1),
        (43153, 5238)
    );
    assert_eq!((c.record[5] + up.0, c.record[6] + up.1), (43229, 5174));
    let s = record(&level, "CannonSwitch");
    assert_eq!(at(s), (43277, 5022, 1000));
    assert_eq!(s.record[11], 0);
    let b = record(&level, "CannonButton");
    assert_eq!(at(b), (42961, 4779, 1000));
    assert_eq!(b.record[11], 2, "mirrored");
    // Only the cannon reads its speed fields, and they are empty: its
    // balls fly at 500 px/s.
    assert_eq!((c.record[field::SPEED_X], c.record[SPEED_Y]), (0, 0));
    // The stage: the door far to the west leads into the stager's box.
    let stager = record(&level, boss_stage::STAGER);
    assert_eq!(at(stager), (43054, 5184, 1000));
    let door = record(&level, boss_stage::WARP);
    assert_eq!(at(door), (31631, 5167, 1000));
    assert_eq!(
        (door.record[field::SPEED_X], door.record[SPEED_Y]),
        (43058, 5212)
    );
    // The three toggle pegs that climb towards the button.
    let mut climb: Vec<(i32, i32)> = ["TogglePeg", "TogglePeg2", "TogglePeg3"]
        .iter()
        .flat_map(|n| records(&level, n))
        .map(|r| (r.record[5], r.record[6]))
        .filter(|&(x, y)| (42700..43000).contains(&x) && (4900..5300).contains(&y))
        .collect();
    climb.sort();
    assert_eq!(climb, [(42770, 4980), (42770, 5186), (42821, 5088)]);
}

#[test]
fn install_level8_boss_stage_binds_around_gabriel() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 8);
    let target = {
        let door = record(&level, boss_stage::WARP);
        (door.record[field::SPEED_X], door.record[SPEED_Y])
    };

    // Claw put down where the door drops him starts the introduction.
    // With Gabriel built he is the current boss and his health bar is
    // full, and the fight comes on as in the original game. Without him
    // there is no current boss and no health bar, and his line takes no
    // time: one tick instead of its length and 100 ms.
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in gabriel::STAGE {
        assert_eq!(d.inert_logics.get(name), None, "{name} runs");
    }
    let line_ms = sim
        .with_world(|w| w.sound_ms(boss_stage::ENEMY_LINE))
        .expect("his line is loaded");
    let line = boss_stage::delay_ticks(line_ms + boss_stage::LINE_PAD_MS);
    println!("LEVEL8 boss line: {line_ms} ms, {line} ticks");
    assert_eq!(line, 222);
    sim.teleport_player(target.0, target.1);
    let mut staging = None;
    let mut on = None;
    for t in 1..=1000 {
        sim.step(TickInput::default());
        match sim.boss().fight() {
            fight::STAGING => {
                staging.get_or_insert(t);
                if gabriel::BUILT {
                    assert!(sim.boss().current().is_some(), "he is the boss");
                    let meter = sim.hud().boss_meter.map(|m| (m.value, m.max));
                    assert_eq!(meter, Some((100, gabriel::METER_MAX)));
                } else {
                    assert_eq!(sim.boss().current(), None, "no boss is built");
                    assert_eq!(sim.hud().boss_meter, None);
                }
            }
            fight::ON => {
                on = Some(t);
                break;
            }
            _ => {}
        }
    }
    println!("LEVEL8 introduction: staging from tick {staging:?}, fight on at {on:?}");
    let (staging, on) = (staging.expect("the introduction starts"), on.expect("on"));
    let length = on - staging;
    if gabriel::BUILT {
        assert_eq!(length, gabriel::STAGING_TICKS as usize);
    } else {
        assert_eq!(
            length + line as usize - 1,
            gabriel::STAGING_TICKS as usize,
            "the introduction's length without him"
        );
    }
}

#[test]
fn install_level8_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 8);
    let preloaded = level.image_sets();
    for (set, frames) in [
        (gabriel::IMAGE_SET, None),
        (gabriel::bomb::IMAGE_SET, None),
        (cannon_switch::IMAGE_SET, Some((1, 3))),
        (button::IMAGE_SET, Some((1, 3))),
        (cannon::IMAGE_SET, Some((1, 27))),
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
    // Gabriel's box is his first idle picture's, and the button's its
    // first lamp's.
    assert_eq!(level.frame_box(gabriel::IMAGE_SET, 100), Some(gabriel::BOX));
    assert_eq!(
        level.frame_box(button::IMAGE_SET, 1),
        Some(button::HIT_RECT)
    );

    // Gabriel's animations and their timings at 18 ms.
    for a in [
        "BLOCKED",
        "HITHIGH",
        "HITLOW",
        "IDLE1",
        "IDLE2",
        "KILLFALL1",
        "KILLFALL2",
        "KILLFALL3",
        "KILLFALL4",
        "STRIKE1",
        "STRIKE7",
    ] {
        let key = format!("LEVEL_GABRIEL_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    assert_eq!(
        calls(&level, gabriel::IDLE1_ANIM).0,
        gabriel::IDLE1_TICKS as usize
    );
    assert_eq!(
        calls(&level, gabriel::IDLE2_ANIM).0,
        gabriel::IDLE2_TICKS as usize
    );
    assert_eq!(calls(&level, gabriel::THUG_ANIM).0, 13);
    assert_eq!(calls(&level, gabriel::BLOCKED_ANIM).0, 30);
    assert_eq!(calls(&level, gabriel::CANNON_HIT_ANIM).0, 19);
    // The strike: its bombs and its lever order on the first call showing
    // picture 401, the 9th, done on the 39th.
    assert_eq!(calls(&level, gabriel::STRIKE_ANIM).0, 39);
    let strike = pictures(&level, gabriel::STRIKE_ANIM, 39);
    assert_eq!(
        strike.iter().position(|&f| f == gabriel::STRIKE_FRAME),
        Some(8)
    );
    // The death acts after the first, which loops.
    let first = &level.animations[gabriel::DEATH_ANIMS[0]];
    assert!(first.frames.iter().any(|f| f.next_op == 0), "it loops");
    assert_eq!(calls(&level, gabriel::DEATH_ANIMS[1]).0, 31);
    assert_eq!(
        calls(&level, gabriel::DEATH_ANIMS[2]).0,
        gabriel::DEATH_ACT_TICKS[2] as usize
    );
    assert_eq!(
        calls(&level, gabriel::DEATH_ANIMS[3]).0,
        gabriel::DEATH_ACT_TICKS[3] as usize
    );

    // The bomb's blast.
    assert!(level.animations.contains_key(gabriel::bomb::FALL_ANIM));
    assert_eq!(
        calls(&level, gabriel::bomb::EXPLODE_ANIM).0,
        gabriel::bomb::EXPLODE_TICKS as usize
    );

    // The cannon: each turn 39 calls, each shot 11 with the ball on the
    // first; no waiting animation.
    for anim in [cannon::RISE_ANIM, cannon::REST_ANIM] {
        assert_eq!(
            calls(&level, anim).0,
            cannon::ROTATE_TICKS as usize,
            "{anim}"
        );
    }
    for anim in [cannon::FIRE_ANIM, cannon::VERT_FIRE_ANIM] {
        let (n, events) = calls(&level, anim);
        assert_eq!(n, cannon::FIRE_TICKS as usize, "{anim}");
        assert_eq!(events.first(), Some(&1), "{anim}");
    }
    let rise = pictures(&level, cannon::RISE_ANIM, 39);
    assert_eq!(rise.last(), Some(&cannon::RAISED_FRAME));
    // The level ships a waiting animation for the cannon, which it never
    // plays.
    assert!(level.animations.contains_key("LEVEL_GABRIELCANNON_HOME"));
    assert_eq!(cannon::HOME_ANIM, None);

    // The switch's flip: pictures 1, 2, 3, 2 for 6 calls each, done on the
    // 25th; with the tick that starts it on picture 1, the flip shows its
    // pictures for 7, 6, 6 and 6 ticks.
    assert_eq!(
        calls(&level, cannon_switch::SWITCH_ANIM).0,
        cannon_switch::SWITCH_TICKS as usize
    );
    let flip = pictures(&level, cannon_switch::SWITCH_ANIM, 24);
    assert_eq!(runs(&flip), [(1, 6), (2, 6), (3, 6), (2, 6)]);
    // The button's blink: lamps 1 and 3, 12 calls each.
    let idle = &level.animations[button::IDLE_ANIM];
    assert!(idle.frames.iter().all(|f| f.next_op == 0), "it loops");
    let blink = pictures(&level, button::IDLE_ANIM, 48);
    let lamps = button::BLINK_TICKS as usize;
    assert_eq!(
        runs(&blink),
        [(1, lamps), (3, lamps), (1, lamps), (3, lamps)]
    );

    // The pirates' and bears' animations are the seventh level's.
    let level7 = level_n(&store, 7);
    let mut same = 0;
    for (key, anim) in &level.animations {
        if key.starts_with("LEVEL_REDTAILPIRATE_") || key.starts_with("LEVEL_BEARSAILOR_") {
            assert_eq!(level7.animations.get(key), Some(anim), "{key}");
            same += 1;
        }
    }
    assert!(same >= 30, "{same}");
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level8_reused_families_take_the_eighth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 8);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut covers: BTreeMap<(String, i32), usize> = BTreeMap::new();
    let mut pegs: BTreeMap<String, usize> = BTreeMap::new();
    let mut stones: BTreeMap<String, usize> = BTreeMap::new();
    let mut pots: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut cycles: BTreeMap<String, usize> = BTreeMap::new();
    let mut enemies: BTreeMap<String, usize> = BTreeMap::new();
    let mut ropes = 0;
    for (_, o) in sim.objects().all() {
        let record = o.record.as_ref();
        match &*o.logic {
            "GooCoverup" => {
                assert_eq!(o.state, decor::SET_UP);
                assert_eq!(o.z, record.unwrap()[7], "the record's depth");
                *covers.entry((o.image_set.clone(), o.z)).or_default() += 1;
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(
                    o.hit_rect,
                    platform::surface(8, SurfaceKind::TogglePeg),
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
                *pegs.entry(o.image_set.clone()).or_default() += 1;
            }
            "StartSteppingStone" => {
                assert_eq!(o.hit_rect, stepping_stone::surface(8));
                *stones.entry(o.image_set.clone()).or_default() += 1;
            }
            "Elevator" | "OneWayTriggerElevator" => {
                assert_eq!(
                    o.hit_rect,
                    platform::surface(8, SurfaceKind::Elevator),
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
                *pots
                    .entry((o.logic.to_string(), o.image_set.clone()))
                    .or_default() += 1;
            }
            "AniCycle" => {
                let ObjectData::Decor(d) = &o.data else {
                    panic!("set up: {:?}", o.data)
                };
                assert!(d.anim.animation().is_some(), "{} is animated", o.image_set);
                *cycles.entry(o.image_set.clone()).or_default() += 1;
            }
            name if rope::NAMES.contains(&name) => {
                assert!(matches!(o.data, ObjectData::Rope(_)), "set up");
                ropes += 1;
            }
            "Seagull" | "RedTailPirate" | "BearSailor" | "PunkRat" => {
                assert!(o.data.enemy().is_some(), "{} is built", o.logic);
                *enemies.entry(o.logic.to_string()).or_default() += 1;
            }
            _ => {}
        }
    }
    println!(
        "LEVEL8 cover-ups {covers:?}, pegs {pegs:?}, stones {stones:?}, pots {pots:?}, \
         animated pieces {cycles:?}, ropes {ropes}, enemies {enemies:?}"
    );
    // 704 water cover-ups, all drawn in front of Claw at their records'
    // depth.
    assert_eq!(
        covers,
        BTreeMap::from([(("LEVEL_WATERCOVER1".to_string(), 8000), 704)])
    );
    // 35 toggle pegs: cannons, pegs and buoys.
    assert_eq!(
        pegs,
        BTreeMap::from([
            ("LEVEL_CANNON2".to_string(), 19),
            ("LEVEL_PEG".to_string(), 11),
            ("LEVEL_SINKBOEY".to_string(), 5),
        ])
    );
    // 37 sinking buoys.
    assert_eq!(stones, BTreeMap::from([("LEVEL_SINKBOEY".to_string(), 37)]));
    // Four sinking pots.
    assert_eq!(
        pots,
        BTreeMap::from([
            (("Elevator".to_string(), "LEVEL_SINKPOT".to_string()), 3),
            (
                (
                    "OneWayTriggerElevator".to_string(),
                    "LEVEL_SINKPOT".to_string()
                ),
                1
            ),
        ])
    );
    assert_eq!(cycles, BTreeMap::from([("LEVEL_STARTSIGN".to_string(), 1)]));
    assert_eq!(ropes, 29);
    assert_eq!(
        enemies,
        BTreeMap::from([
            ("BearSailor".to_string(), 40),
            ("PunkRat".to_string(), 20),
            ("RedTailPirate".to_string(), 41),
            ("Seagull".to_string(), 73),
        ])
    );
    for set in covers
        .keys()
        .map(|k| &k.0)
        .chain(pegs.keys())
        .chain(stones.keys())
        .chain(pots.keys().map(|k| &k.1))
        .chain(cycles.keys())
    {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // 105 kegs, all after Claw at depth 5000.
    let kegs = records(&level, keg::NAMES[0]);
    assert_eq!(kegs.len(), 105);
    assert!(kegs.iter().all(|r| r.record[7] == 5000));
    // The pirates and bears: their own pictures, never standing still.
    for (name, set, n) in [
        ("RedTailPirate", redtail_pirate::IMAGE_SET, 41),
        ("BearSailor", enemy::bear_sailor::IMAGE_SET, 40),
    ] {
        let rs = records(&level, name);
        assert_eq!(rs.len(), n);
        for r in rs {
            assert_eq!(r.image_set, set, "{name}");
            assert_eq!(r.record[7], 0, "{name} depth");
            assert_eq!(r.record[field::USER_0], 0, "{name} never stands still");
        }
    }
}
