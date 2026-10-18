//! The framework the tenth level's families build on: Marrow, his parrot
//! and the two slabs of his trap floor are declared, and each stays an
//! inert placeholder until its family is built; the two enemies' classes,
//! data and state numbers; the Marrow link the three share; the level's
//! platform, springboard and death tile table and the ninth level's
//! families' tenth-level rules; and, on the install, the level's inert
//! census, the boss room's records, its boss stage bound to the shared
//! stage pieces, the art the new families need loaded with it, and what
//! the earlier families do with the level (the hook-handed pirates,
//! PegLegs and dart traps, the seagulls, the pegs, the big trap elevators,
//! the ropes and the warps).
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here. The free numbers of the tenth level's block of
//! state numbers stay free for its families to take.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{
    self, EnemyState, field, goo, hook_pirate, marrow, parrot, pegleg, seagull,
};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{
    self, GameLogic, ObjectData, boss_stage, dart_trap, marrow_floor, rope, springboard,
    stepping_stone,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the tenth level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 3] = [
    (GameLogic::Marrow, &["Marrow"]),
    (GameLogic::Parrot, &["Parrot"]),
    (GameLogic::MarrowFloor, &["MarrowFloor"]),
];

/// Logics the tenth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the skull candles, the room's two slab covers, the
/// `DoNothing` pictures), platform (the crumbling and toggle pegs, the
/// elevators with the big trap art), rope, crate, pickup, warp,
/// checkpoint, sound object and enemy (the hook-handed pirates, PegLegs,
/// dart traps and seagulls) the level places outside the declared
/// families already runs, and so do the door and the stager of its boss
/// stage.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the tenth level places.
const PLACED: [(&str, usize); 3] = [("Marrow", 1), ("Parrot", 1), ("MarrowFloor", 2)];

/// The record field after the horizontal speed: the vertical speed.
const SPEED_Y: usize = field::SPEED_X + 1;

/// The record field holding a dart trap's arrow picture.
const ARROW_FRAME: usize = field::USER_0;

/// The hook-handed pirate's name.
fn hook() -> &'static str {
    hook_pirate::NAMES[0]
}

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
    assert_eq!(logic::built(GameLogic::Marrow), marrow::BUILT);
    assert_eq!(logic::built(GameLogic::Parrot), parrot::BUILT);
    assert_eq!(logic::built(GameLogic::MarrowFloor), marrow_floor::BUILT);
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
    // never collide, and the level load reports them as not implemented;
    // asleep in the side areas of a wide display they look as they are.
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
        assert_eq!(logic::first_look(&level, o), None, "{}", o.logic);
    }
    assert!(sim.collision_events().is_empty());
}

#[test]
fn the_registry_keeps_each_names_activation_class() {
    // All three wake with the enemies' window; none keeps the original
    // window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for name in ["Marrow", "Parrot", "MarrowFloor"] {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            0,
            "{name}"
        );
        assert!(!cfg.is_exception(name), "{name}");
    }
}

#[test]
// The data literals below end in `..Default::default()` so they keep
// compiling when the families gain fields of their own.
#[allow(clippy::needless_update)]
fn marrow_is_a_human_boss_and_his_parrot_a_plain_enemy() {
    let class = enemy::class(GameLogic::Marrow).expect("a class");
    assert_eq!(class.name, "Marrow");
    assert!(class.human, "Marrow is a human enemy");
    assert!(class.carry.is_none(), "Marrow is never lifted");
    let class = enemy::class(GameLogic::Parrot).expect("a class");
    assert_eq!(class.name, "Parrot");
    assert!(!class.human, "the parrot is no human enemy");
    assert!(class.carry.is_none());
    assert!(enemy::class(GameLogic::MarrowFloor).is_none());
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::Marrow(marrow::MarrowData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Parrot(parrot::ParrotData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    assert_eq!(ObjectData::MarrowFloor(Default::default()).enemy(), None);
    // His stage is the shared one.
    for name in marrow::STAGE {
        assert!(boss_stage::NAMES.contains(&name), "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    assert_eq!(marrow::METER_MAX, 100);
    assert_eq!(marrow::HEALTH / marrow::PHASE_DAMAGE, 4);
    assert_eq!(marrow::BULLET_DAMAGE, 20);
    assert_eq!(marrow::BULLET_SPEED, 600);
    assert_eq!(marrow::MUZZLE, (65, -24));
    assert_eq!((parrot::HEALTH, parrot::DAMAGE), (1, 10));
    // His bullet is the projectile family's.
    assert_eq!(
        logic::kind_for(pegleg::bullet::LOGIC),
        Some(LogicKind::Game(GameLogic::Projectile))
    );
    // The parrot's numbers are the seagull's, its machine its own.
    assert_eq!(parrot::Z, 8500);
}

#[test]
fn the_link_is_there_only_with_marrow_built() {
    // No Marrow: no link.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 2);
    assert_eq!(sim.marrow_link(), None);
    sim.with_world(|w| {
        assert_eq!(w.marrow_link(), None);
        assert!(w.marrow_link_mut().is_none());
    });
    // Marrow placed beside Claw: built, he holds the link from his
    // building, the slabs asked to open; otherwise still none.
    let mut sim = Sim::new(level_with(&["Marrow"]), SimConfig::default());
    idle(&mut sim, 3);
    let marrow_id = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Marrow")
        .map(|(id, _)| id)
        .expect("Marrow is placed");
    if marrow::BUILT {
        let link = sim.marrow_link().copied().expect("his link");
        assert_eq!(link.marrow, marrow_id);
        assert!(link.open_request);
        assert!(sim.boss().current().is_some(), "he is the boss");
    } else {
        assert_eq!(sim.marrow_link(), None);
        assert_eq!(sim.boss().current(), None);
    }
}

#[test]
fn the_link_lives_from_marrows_building_to_his_removal() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 1);
    sim.with_world(|w| {
        w.link_marrow(7);
        let l = *w.marrow_link().expect("linked");
        assert_eq!(l, marrow::MarrowLink::new(7));
        assert!(l.open_request && !l.parrot_call && !l.floors_open);
        assert_eq!(l.phase_health, 0);
        // The parrot and the slabs write into it.
        let l = w.marrow_link_mut().expect("linked");
        l.parrot_call = true;
        l.phase_health = 75;
    });
    // It lasts across ticks.
    idle(&mut sim, 2);
    let l = *sim.marrow_link().expect("still linked");
    assert!(l.parrot_call);
    assert_eq!(l.phase_health, 75);
    // Only his own removal ends it.
    sim.with_world(|w| w.unlink_marrow(8));
    assert!(sim.marrow_link().is_some());
    sim.with_world(|w| w.unlink_marrow(7));
    assert_eq!(sim.marrow_link(), None);
    // A copy of the game copies the link.
    sim.with_world(|w| w.link_marrow(3));
    let copy = sim.clone();
    assert_eq!(copy.marrow_link(), sim.marrow_link());
}

#[test]
fn the_tenth_levels_state_numbers_hold_the_whistle_and_the_flights() {
    let block = marrow::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2240, 2259));
    let named = [
        (EnemyState::WaitForParrot, 2240, "WaitForParrot"),
        (EnemyState::FlyAbout, 2241, "FlyAbout"),
        (EnemyState::StrikeSetup, 2242, "StrikeSetup"),
        (EnemyState::FlyToIdle, 2243, "FlyToIdle"),
        (EnemyState::FlyToMarrow, 2244, "FlyToMarrow"),
        (EnemyState::CarryMarrow, 2245, "CarryMarrow"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(marrow::STATES, [named[0].0]);
    assert_eq!(
        parrot::STATES,
        named[1..].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    // The ninth level's block ends where the tenth's starts.
    assert_eq!(*hook_pirate::STATE_BLOCK.end() + 1, *block.start());
    // The shared states they reach keep their numbers: the bosses' wait,
    // the shot, Raux's flinches and the seagull's.
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::Fire as i32, 2013);
    assert_eq!(EnemyState::HitLow as i32, 2004);
    assert_eq!(EnemyState::HitHigh as i32, 2005);
    assert_eq!(EnemyState::GenericHit as i32, 2143);
    // The slabs keep states of their own.
    for s in [
        marrow_floor::state::IDLE,
        marrow_floor::state::OPENING,
        marrow_floor::state::CLOSING,
    ] {
        assert_eq!(EnemyState::from_id(s), None, "{s}");
    }
}

#[test]
fn the_tenth_levels_table() {
    // Platform rectangles, each with its 32 px depth, and the big trap
    // elevator's, which only this level has.
    assert_eq!(
        platform::surface(10, SurfaceKind::Elevator),
        Rect::new(-44, -8, 46, 64)
    );
    assert_eq!(
        platform::surface(10, SurfaceKind::TogglePeg),
        Rect::new(-24, -8, 24, 64)
    );
    assert_eq!(
        platform::surface(10, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(10, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    assert_eq!(stepping_stone::surface(10), Rect::new(-16, -1, 16, 64));
    assert_eq!(
        platform::surface(10, SurfaceKind::BigElevator),
        Rect::new(-88, -20, 94, 52)
    );
    for level in (1..=14).filter(|&l| l != 10) {
        assert_eq!(
            platform::surface(level, SurfaceKind::BigElevator),
            Rect::new(0, 0, 0, 32),
            "level {level}"
        );
    }
    // Springboards: the fern, which no record places.
    let s = springboard::level_table(10).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING")
    );
    // Spike death tiles, no goo and no splash.
    assert!(!goo::is_goo_level(10));
    assert_eq!(goo::splash_y(10), None);
    // The ninth level's families on the tenth: the hook-handed pirate
    // neither holds his hook out nor parries a standing sword stab, and
    // PegLeg fires pairs and rolls the full length.
    assert!(!hook_pirate::holds_and_parries_swords(10));
    assert!(!pegleg::aims_at_posture(10));
    // The slabs step 5 px a tick.
    assert_eq!(marrow_floor::step(300, 18), 5);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        marrow::IMAGE_SET,
        marrow::BULLET_IMAGE_SET,
        parrot::IMAGE_SET,
        marrow_floor::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        marrow::HOME_ANIM,
        marrow::FIRE_ANIM,
        marrow::STRIKE_ANIM,
        marrow::WHISTLE_ANIM,
        marrow::DEATH_ANIM,
        parrot::IDLE_ANIM,
        parrot::FLY_ANIM,
        parrot::HIT_ANIM,
        parrot::STRIKE_ANIM,
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

/// How many records of each value of `key` the records of `name` hold.
fn tally<K: Ord>(
    level: &SimLevel,
    name: &str,
    key: impl Fn(&ObjectSpec) -> K,
) -> BTreeMap<K, usize> {
    let mut out = BTreeMap::new();
    for r in records(level, name) {
        *out.entry(key(r)).or_default() += 1;
    }
    out
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

/// Whether the animation `key` loops (never finishes).
fn loops(level: &SimLevel, key: &str) -> bool {
    level.animations[key].frames.iter().all(|f| f.next_op == 0)
}

#[test]
fn install_level10_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    assert_eq!(level.number, 10);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL10 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 2536, "the level's records");
    assert_eq!(level.start, (750, 7695));

    let placed = |name: &str| records(&level, name).len();
    for (name, n) in PLACED {
        assert_eq!(placed(name), n, "{name}");
    }
    // Each declared family is on the level, and reported inert until the
    // family is built.
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
        (hook(), 48),
        ("PegLeg", 33),
        ("TProjectile", 70),
        ("Seagull", 9),
        ("CrumblingPeg", 154),
        ("TogglePeg", 16),
        ("TogglePeg2", 9),
        ("TogglePeg3", 8),
        ("TogglePeg4", 6),
        ("Elevator", 10),
        ("OneWayTriggerElevator", 14),
        ("TriggerElevator", 1),
        ("AniRope", 27),
        ("SpecialPowerup", 17),
        ("Checkpoint", 9),
        ("BossStager", 1),
        ("BossWarp", 1),
        ("AmbientSound", 47),
        ("BigSoundTrigger", 33),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // None of the earlier levels' enemies, bosses or hazards the tenth
    // does not use: Marrow's gem ends the level.
    for name in [
        "Officer",
        "Soldier",
        "Rat",
        "CutThroat",
        "RobberThief",
        "TownGuard1",
        "TownGuard2",
        "RedTailPirate",
        "BearSailor",
        "PunkRat",
        "HermitCrab",
        "CrabNest",
        "Raux",
        "Katherine",
        "Wolvington",
        "Gabriel",
        "PowderKeg",
        "TowerCannonLeft",
        "TowerCannonRight",
        "SkullCannon",
        "FloorSpike",
        "SteppingStone",
        "SpringBoard",
        "BreakPlank",
        "JumpSwitch",
        "GooCoverup",
        "GooVent",
        "GroundBlower",
        "ConveyorBelt",
        "SawBlade",
        "PathElevator",
        "AniCycle",
        "EndOfLevelPowerup",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level10_boss_room_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    let at = |r: &ObjectSpec| (r.record[5], r.record[6], r.record[7]);
    // Marrow on the right ledge, facing left, the gem's target over the
    // middle of the pit.
    let m = record(&level, "Marrow");
    assert_eq!(at(m), (41063, 7255, 0));
    assert_eq!(m.record[11], 0, "faces left");
    assert_eq!((m.record[field::SPEED_X], m.record[SPEED_Y]), (40800, 7170));
    assert_eq!(m.record[18], 0, "his record's health is not his health");
    assert_eq!((m.record[field::X_MIN], m.record[field::X_MAX]), (0, 0));
    // His parrot above him, its box the room: the patrol line at its top,
    // the room's centre in the middle of its x range.
    let p = record(&level, "Parrot");
    assert_eq!(at(p), (41049, 7141, 0));
    let bounds = |r: &ObjectSpec| {
        (
            r.record[field::X_MIN],
            r.record[field::Y_MIN],
            r.record[field::X_MAX],
            r.record[field::Y_MAX],
        )
    };
    assert_eq!(bounds(p), (40488, 7029, 41128, 7287));
    assert_eq!((40488 + 41128) / 2, marrow::ROOM_CENTRE_X);
    // The two slabs at the closed bridge, their tracks to the open ends.
    let mut floors: Vec<_> = records(&level, "MarrowFloor")
        .into_iter()
        .map(|r| {
            (
                at(r),
                (r.record[field::X_MIN], r.record[field::X_MAX]),
                r.record[field::SPEED_X],
                r.record[dart_trap::DIRECTION_FIELD],
                r.image_set.clone(),
            )
        })
        .collect();
    floors.sort();
    let trap = marrow_floor::IMAGE_SET.to_string();
    assert_eq!(
        floors,
        [
            ((40714, 7393, 2500), (40534, 40714), 300, 0, trap.clone()),
            ((40911, 7393, 2000), (40911, 41071), 300, -1, trap),
        ]
    );
    // Each travels a whole number of 5 px steps.
    assert_eq!((40714 - 40534) / 5, marrow_floor::OPEN_TICKS.0 as i32);
    assert_eq!((41071 - 40911) / 5, marrow_floor::OPEN_TICKS.1 as i32);
    // The covers that hide the open slabs, in front of them (the first
    // is used elsewhere in the level too).
    for (set, x) in [("LEVEL_COVERIN1", 40528), ("LEVEL_ENDCOVER", 41091)] {
        let covers: Vec<_> = level
            .objects
            .iter()
            .filter(|o| o.image_set == set && o.record[5] > 40000)
            .map(|r| (r.logic.as_str(), at(r)))
            .collect();
        assert_eq!(covers, [("DoNothingNormal", (x, 7393, 3000))], "{set}");
    }
    // The stage: the door in the middle of the level leads into the
    // stager's box on the left ledge.
    let stager = record(&level, boss_stage::STAGER);
    assert_eq!(at(stager), (40497, 7249, 1000));
    let door = record(&level, boss_stage::WARP);
    assert_eq!(at(door), (32432, 7769, 1000));
    assert_eq!(
        (door.record[field::SPEED_X], door.record[SPEED_Y]),
        (40484, 7245)
    );
}

#[test]
fn install_level10_boss_stage_binds_around_marrow() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    let target = {
        let door = record(&level, boss_stage::WARP);
        (door.record[field::SPEED_X], door.record[SPEED_Y])
    };
    // Claw put down where the door drops him starts the introduction.
    // With Marrow built he is the current boss and his health bar is full,
    // and the fight comes on as in the original game. Without him there
    // is no current boss and no health bar, and his line takes no time:
    // one tick instead of its length and 100 ms.
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    for name in marrow::STAGE {
        assert_eq!(d.inert_logics.get(name), None, "{name} runs");
    }
    let line_ms = sim
        .with_world(|w| w.sound_ms(boss_stage::ENEMY_LINE))
        .expect("his line is loaded");
    let line = boss_stage::delay_ticks(line_ms + boss_stage::LINE_PAD_MS);
    println!("LEVEL10 boss line: {line_ms} ms, {line} ticks");
    assert_eq!(line_ms, 3635);
    sim.teleport_player(target.0, target.1);
    let mut staging = None;
    let mut on = None;
    for t in 1..=1000 {
        sim.step(TickInput::default());
        match sim.boss().fight() {
            fight::STAGING => {
                staging.get_or_insert(t);
                if marrow::BUILT {
                    assert!(sim.boss().current().is_some(), "he is the boss");
                    let meter = sim.hud().boss_meter.map(|m| (m.value, m.max));
                    assert_eq!(meter, Some((100, marrow::METER_MAX)));
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
    println!("LEVEL10 introduction: staging from tick {staging:?}, fight on at {on:?}");
    let (staging, on) = (staging.expect("the introduction starts"), on.expect("on"));
    let length = on - staging;
    if marrow::BUILT {
        assert_eq!(length, marrow::STAGING_TICKS as usize);
    } else {
        assert_eq!(
            length + line as usize - 1,
            marrow::STAGING_TICKS as usize,
            "the introduction's length without him"
        );
    }
}

#[test]
fn install_level10_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    let preloaded = level.image_sets();
    for set in [
        marrow::IMAGE_SET,
        marrow::BULLET_IMAGE_SET,
        parrot::IMAGE_SET,
        marrow_floor::IMAGE_SET,
        "LEVEL_COVERIN1",
        "LEVEL_ENDCOVER",
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The slab's one 208 x 64 picture.
    assert_eq!(level.image_frames(marrow_floor::IMAGE_SET), Some((1, 1)));
    assert_eq!(
        level.frame_box(marrow_floor::IMAGE_SET, 1),
        Some(Rect::new(-104, -32, 103, 31))
    );

    // Marrow's animations and their timings at 18 ms: the shot's bullet on
    // the 19th call, done on the 34th; the strike's on the 16th, done on
    // the 26th; the whistle done on the 16th, the flinches on the 19th, the
    // block on the 11th, the death on the 45th; his start-up pose loops.
    for a in [
        "ADVANCE",
        "FASTADVANCE",
        "STRIKE1",
        "STRIKE2",
        "IMPACT",
        "KILLFALL",
        "HITLOW",
        "HITHIGH",
        "JUMP",
        "HOME",
        "IDLE1",
        "LIFTED",
        "BLOCK",
    ] {
        let key = format!("LEVEL_MARROW_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    let (n, events) = calls(&level, marrow::FIRE_ANIM);
    assert_eq!(n as u32, marrow::FIRE_TICKS.1);
    assert_eq!(
        events.first().map(|&e| e as u32),
        Some(marrow::FIRE_TICKS.0)
    );
    let (n, events) = calls(&level, marrow::STRIKE_ANIM);
    assert_eq!((n, events.first().copied()), (26, Some(16)));
    assert_eq!(
        calls(&level, marrow::WHISTLE_ANIM).0 as u32,
        marrow::CALL_TICK
    );
    for a in ["LEVEL_MARROW_HITLOW", "LEVEL_MARROW_HITHIGH"] {
        assert_eq!(calls(&level, a).0, 19, "{a}");
    }
    assert_eq!(calls(&level, "LEVEL_MARROW_BLOCK").0, 11);
    assert_eq!(calls(&level, marrow::DEATH_ANIM).0, 45);
    assert!(loops(&level, marrow::HOME_ANIM));
    for s in [marrow::WHISTLE_SOUND, marrow::PISTOL_SOUND] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }

    // The parrot's: the flinch done on the 19th call, the perch's loop on
    // the 57th, the swoop's one picture; the flight loops.
    assert_eq!(calls(&level, parrot::HIT_ANIM).0 as u32, parrot::HIT_TICKS);
    assert_eq!(calls(&level, parrot::IDLE_ANIM).0, 57);
    assert_eq!(calls(&level, parrot::STRIKE_ANIM).0, 1);
    assert!(loops(&level, parrot::FLY_ANIM));
    for s in [parrot::HIT_SOUND, parrot::SWOOP_SOUND]
        .iter()
        .chain(&parrot::SQUAWKS)
    {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // The slabs' slide, and the staging's two lines.
    assert!(level.sound_ms(marrow_floor::SLIDE_SOUND).is_some());
    assert_eq!(level.sound_ms(boss_stage::ENEMY_LINE), Some(3635));
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level10_reused_families_take_the_tenth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    let at = |r: &ObjectSpec| (r.record[5], r.record[6]);

    // The hook-handed pirates and PegLegs: their own pictures, depth 0,
    // no animation, never standing still; two pirates and one PegLeg
    // patrol between bounds, and one of each carries loot.
    for (name, set, n, bounded) in [
        (hook(), hook_pirate::image_set(), 48, 2),
        ("PegLeg", pegleg::IMAGE_SET, 33, 1),
    ] {
        let rs = records(&level, name);
        assert_eq!(rs.len(), n, "{name}");
        for r in &rs {
            assert_eq!(r.image_set, set, "{name}");
            assert_eq!(r.record[7], 0, "{name} depth");
            assert!(r.anim.is_empty(), "{name} names no animation");
            assert_eq!(r.record[field::USER_0], 0, "{name} never stands still");
        }
        let with_bounds = rs
            .iter()
            .filter(|r| r.record[field::X_MIN] != 0 && r.record[field::X_MAX] != 0)
            .count();
        assert_eq!(with_bounds, bounded, "{name} bounded");
        let loot = rs.iter().filter(|r| r.record[field::POWERUP] != 0).count();
        assert_eq!(loot, 1, "{name} loot");
    }

    // The dart traps: each picture set aims one way and shoots its arrow,
    // at depth 2000, with a trigger rectangle and a wait.
    let darts = tally(&level, "TProjectile", |r| {
        (
            r.image_set.clone(),
            r.record[dart_trap::DIRECTION_FIELD],
            r.record[ARROW_FRAME],
        )
    });
    assert_eq!(
        darts,
        BTreeMap::from([
            (("LEVEL_SHOOTERS_PUFFDARTDOWN".to_string(), 2, 3), 27),
            (("LEVEL_SHOOTERS_PUFFDARTLEFT".to_string(), 4, 2), 35),
            (("LEVEL_SHOOTERS_PUFFDARTRIGHT".to_string(), 6, 1), 8),
        ])
    );
    for r in records(&level, "TProjectile") {
        assert_eq!(r.anim, r.image_set, "{:?}", at(r));
        assert_eq!(r.record[7], 2000, "{:?}", at(r));
        assert!(r.record[field::X_MAX] > r.record[field::X_MIN]);
        assert!(r.record[field::Y_MAX] > r.record[field::Y_MIN]);
        let wait = r.record[dart_trap::WAIT_FIELD];
        assert!((500..=10000).contains(&wait), "{:?}: {wait}", at(r));
        let speed = r.record[field::SPEED_X];
        assert!(
            speed == 0 || (500..=1100).contains(&speed),
            "{:?}: {speed}",
            at(r)
        );
        let (vx, vy) =
            dart_trap::arrow::velocity(r.record[dart_trap::DIRECTION_FIELD], (1, 1)).unwrap();
        let b = dart_trap::arrow::frame_box(r.record[ARROW_FRAME]).unwrap();
        let ahead = match (vx, vy) {
            (1, 0) => b.right,
            (-1, 0) => -b.left,
            (0, 1) => b.bottom,
            _ => -b.top,
        };
        assert!(ahead > 50, "{:?}", at(r));
    }
    // The dart traps' arrows and puffs are loaded with the level.
    let preloaded = level.image_sets();
    for set in [
        dart_trap::arrow::IMAGE_SET,
        "LEVEL_SHOOTERS_PUFFDARTLEFT",
        "LEVEL_SHOOTERS_PUFFDARTDOWN",
        "LEVEL_SHOOTERS_PUFFDARTRIGHT",
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
    }
    for frame in 1..=3 {
        assert_eq!(
            level.frame_box(dart_trap::arrow::IMAGE_SET, frame),
            dart_trap::arrow::frame_box(frame),
            "arrow {frame}"
        );
    }
    for side in ["LEFT", "RIGHT", "DOWN"] {
        let anim = format!("LEVEL_SHOOTERS_PUFFDART{side}");
        assert_eq!(
            calls(&level, &anim),
            (dart_trap::PUFF_TICKS as usize, vec![])
        );
    }
    assert!(level.sound_ms(dart_trap::SOUND).is_some());

    // The awake level: the elevators with the big trap art take the big
    // surface, the others and the pegs the level's own; the seagulls,
    // pirates, PegLegs and dart traps are built; the ropes are set up.
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut platforms: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut pegs: BTreeMap<String, usize> = BTreeMap::new();
    let mut built_enemies: BTreeMap<String, usize> = BTreeMap::new();
    let mut traps = 0;
    let mut ropes = 0;
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Elevator" | "OneWayTriggerElevator" | "TriggerElevator" => {
                let kind = if o.image_set == "LEVEL_TRAPELEVATOR" {
                    SurfaceKind::BigElevator
                } else {
                    SurfaceKind::Elevator
                };
                assert_eq!(
                    o.hit_rect,
                    platform::surface(10, kind),
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
                *platforms
                    .entry((o.logic.to_string(), o.image_set.clone()))
                    .or_default() += 1;
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(o.hit_rect, platform::surface(10, SurfaceKind::TogglePeg));
                *pegs.entry(o.logic.to_string()).or_default() += 1;
            }
            "CrumblingPeg" => {
                assert_eq!(o.hit_rect, platform::surface(10, SurfaceKind::CrumblingPeg));
                *pegs.entry(o.logic.to_string()).or_default() += 1;
            }
            "Seagull" | "PegLeg" => {
                assert!(o.data.enemy().is_some(), "{} is built", o.logic);
                *built_enemies.entry(o.logic.to_string()).or_default() += 1;
            }
            name if name == hook() => {
                assert!(o.data.enemy().is_some(), "the hook-handed pirate is built");
                *built_enemies
                    .entry("hook-handed pirate".into())
                    .or_default() += 1;
            }
            "TProjectile" => {
                assert!(matches!(o.data, ObjectData::DartTrap(_)), "set up");
                traps += 1;
            }
            name if rope::NAMES.contains(&name) => {
                assert!(matches!(o.data, ObjectData::Rope(_)), "set up");
                ropes += 1;
            }
            _ => {}
        }
    }
    println!(
        "LEVEL10 platforms {platforms:?}, pegs {pegs:?}, enemies {built_enemies:?}, \
         traps {traps}, ropes {ropes}"
    );
    let trap_art = |logic: &str| (logic.to_string(), "LEVEL_TRAPELEVATOR".to_string());
    let plain = |logic: &str| (logic.to_string(), "LEVEL_ELEVATOR1".to_string());
    assert_eq!(
        platforms,
        BTreeMap::from([
            (plain("Elevator"), 8),
            (trap_art("Elevator"), 2),
            (plain("OneWayTriggerElevator"), 5),
            (trap_art("OneWayTriggerElevator"), 9),
            (trap_art("TriggerElevator"), 1),
        ])
    );
    assert_eq!(
        pegs,
        BTreeMap::from([
            ("CrumblingPeg".to_string(), 154),
            ("TogglePeg".to_string(), 16),
            ("TogglePeg2".to_string(), 9),
            ("TogglePeg3".to_string(), 8),
            ("TogglePeg4".to_string(), 6),
        ])
    );
    assert_eq!(
        built_enemies,
        BTreeMap::from([
            ("PegLeg".to_string(), 33),
            ("Seagull".to_string(), 9),
            ("hook-handed pirate".to_string(), 48),
        ])
    );
    assert_eq!(traps, 70);
    assert_eq!(ropes, 27);
    // The vertical trigger elevator: 8288 to 8671 at 175 px/s.
    let lift = record(&level, "TriggerElevator");
    assert_eq!(lift.image_set, "LEVEL_TRAPELEVATOR");
    assert_eq!(
        (
            lift.record[field::Y_MIN],
            lift.record[field::Y_MAX],
            lift.record[SPEED_Y]
        ),
        (8288, 8671, 175)
    );
    // The seagulls patrol between bounds, with the ninth level's files.
    let gulls = records(&level, "Seagull");
    assert!(
        gulls
            .iter()
            .all(|r| r.record[field::X_MIN] != 0 && r.record[field::X_MAX] != 0)
    );
    assert!(gulls.iter().all(|r| r.record[field::POWERUP] == 0));
    let level9 = level_n(&store, 9);
    let mut same = 0;
    for (key, anim) in &level.animations {
        if key.starts_with(seagull::ANIM_PREFIXES[0]) {
            assert_eq!(level9.animations.get(key), Some(anim), "{key}");
            same += 1;
        }
    }
    assert!(same > 0);
    // The vertical warp to the middle of the level.
    let vertical: Vec<_> = records(&level, "SpecialPowerup")
        .into_iter()
        .filter(|r| at(r) == (4252, 8348))
        .map(|r| (r.record[field::SPEED_X], r.record[SPEED_Y]))
        .collect();
    assert_eq!(vertical, [(20250, 7097)]);
}

#[test]
fn install_level10_costs_at_its_start() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 10);
    // The level's start with every family it places: how long a copy of
    // the game and an idle tick take, for the slice's measurements along
    // the bot route.
    let mut sim = Sim::new(level, SimConfig::default());
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
    println!(
        "LEVEL10 start: {} objects, one copy {clone:?}, one idle tick {tick:?}",
        sim.objects().all().count()
    );
    // A few records leave or turn into other objects on their first
    // calls (sound objects that wake with Claw, a stack of crates).
    assert!(sim.objects().all().count() >= 2536 - 10);
}
