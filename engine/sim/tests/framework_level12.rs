//! The framework the twelfth level's families build on: Aquatis, his
//! crack, his dynamite helper, his five stalactites and the tentacles are
//! declared, and each stays an inert placeholder until its family is
//! built; the cave stalactites and the earthquake triggers run; the boss's
//! and the tentacles' classes, data and state numbers; the Aquatis link
//! the helpers share, with its reset bits, and the paths wired into his
//! file (a stalactite's touch, the help screen's return); the level's
//! platform, springboard and water table; and, on the install, the
//! level's inert census, the art the new families need loaded with it,
//! and what the earlier families do with the level (the toggle peg beside
//! the boss that stays out, the elevators' rectangle, the water that kills
//! by its centre pixel, the ledge's edge that refuses dynamite), and the
//! start lines that give Claw nine sticks for the fight-pocket route.
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here. The free numbers of the twelfth level's block of
//! state numbers stay free for its families to take.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; the one that also reads a reference store needs
//! `CLAW_REFERENCE`. Nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::{GameEvent, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::aquatis::{self, AquatisLink, crack, dynamite};
use claw_sim::logic::enemy::{self, EnemyState, goo, tentacle};
use claw_sim::logic::platform::{self, SurfaceKind, state as peg_state};
use claw_sim::logic::{self, GameLogic, ObjectData, pickup, shake, springboard, stalactite};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::player::progress::parse_start_lines;
use claw_sim::player::state as claw_state;
use claw_sim::registry::Registry;
use claw_sim::rng::Rng;
use claw_sim::tiles::{DEATH, SOLID};
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, StartState};
use common::*;

/// The families declared for the twelfth level, with the names they
/// claim.
const DECLARED: [(GameLogic, &[&str]); 7] = [
    (GameLogic::Aquatis, &["Aquatis"]),
    (GameLogic::AquatisCrack, &["AquatisCrack"]),
    (GameLogic::AquatisDynamite, &["AquatisDynamite"]),
    (GameLogic::AquatisStalactite, &["AquatisStalactite"]),
    (GameLogic::Tentacle, &["Tentacle"]),
    (GameLogic::Stalactite, &["Stalactite"]),
    (GameLogic::Shake, &["Shake"]),
];

/// Logics the twelfth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration, platform,
/// pickup, warp, checkpoint, sound object, floor spike, springboard and
/// enemy of the eleventh level the level places already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the twelfth level places.
const PLACED: [(&str, usize); 7] = [
    ("Aquatis", 1),
    ("AquatisCrack", 1),
    ("AquatisDynamite", 1),
    ("AquatisStalactite", 5),
    ("Tentacle", 3),
    ("Stalactite", 170),
    ("Shake", 30),
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
    assert_eq!(logic::built(GameLogic::Aquatis), aquatis::BUILT);
    assert_eq!(logic::built(GameLogic::AquatisCrack), crack::BUILT);
    assert_eq!(logic::built(GameLogic::AquatisDynamite), dynamite::BUILT);
    assert_eq!(
        logic::built(GameLogic::AquatisStalactite),
        aquatis::stalactite::BUILT
    );
    assert_eq!(logic::built(GameLogic::Tentacle), tentacle::BUILT);
    assert_eq!(logic::built(GameLogic::Stalactite), stalactite::BUILT);
    assert_eq!(logic::built(GameLogic::Shake), shake::BUILT);
    // The cave stalactites and the earthquake triggers run.
    const { assert!(stalactite::BUILT && shake::BUILT) };
    for (name, g) in [
        ("Stalactite", GameLogic::Stalactite),
        ("Shake", GameLogic::Shake),
    ] {
        assert_eq!(
            Registry::shared().get(name).unwrap().kind,
            LogicKind::Game(g)
        );
    }
    // The signs are no logic at all.
    assert_eq!(Registry::shared().get("Sign"), None);
    assert_eq!(logic::kind_for("Sign"), None);
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
    let names: Vec<&str> = [
        "Aquatis",
        "AquatisCrack",
        "AquatisDynamite",
        "AquatisStalactite",
        "Tentacle",
    ]
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
        assert_eq!(logic::first_look(&level, o), None, "{}", o.logic);
    }
    assert!(sim.collision_events().is_empty());
    // Unbuilt, nothing writes the link.
    if !aquatis::BUILT {
        assert_eq!(*sim.aquatis_link(), AquatisLink::default());
    }
}

#[test]
fn the_registry_keeps_each_names_activation_class() {
    // Aquatis and his dynamite helper wake with the large window, the
    // earthquake triggers with the small one, the rest with the enemies'.
    for (name, class) in [
        ("Aquatis", 1),
        ("AquatisDynamite", 1),
        ("AquatisCrack", 0),
        ("AquatisStalactite", 0),
        ("Tentacle", 0),
        ("Stalactite", 0),
        ("Shake", 2),
    ] {
        assert_eq!(
            logic::new_object(name, 0, 0, 0).activation_class(),
            class,
            "{name}"
        );
        assert!(
            !ActivationConfig::widened((1706, 480)).is_exception(name),
            "{name}"
        );
    }
}

#[test]
// The data literals below end in `..Default::default()` so they keep
// compiling when the families gain fields of their own.
#[allow(clippy::needless_update)]
fn aquatis_is_a_human_enemy_and_the_tentacle_a_plain_one() {
    let class = enemy::class(GameLogic::Aquatis).expect("a class");
    assert_eq!(class.name, "Aquatis");
    assert!(class.human, "a human enemy");
    let class = enemy::class(GameLogic::Tentacle).expect("a class");
    assert_eq!(class.name, "Tentacle");
    assert!(!class.human, "a plain enemy");
    assert!(class.carry.is_none(), "never lifted");
    for g in [
        GameLogic::AquatisCrack,
        GameLogic::AquatisDynamite,
        GameLogic::AquatisStalactite,
        GameLogic::Stalactite,
        GameLogic::Shake,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?}");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::Aquatis(aquatis::AquatisData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Tentacle(tentacle::TentacleData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::AquatisCrack(Default::default()),
        ObjectData::AquatisDynamite(Default::default()),
        ObjectData::AquatisStalactite(Default::default()),
        ObjectData::Stalactite(Default::default()),
        ObjectData::Shake(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Their numbers.
    assert_eq!((aquatis::HEALTH, aquatis::DAMAGE), (100, 10));
    assert_eq!(aquatis::STALACTITES, 5);
    assert_eq!(aquatis::BOX, Rect::new(-62, -85, 109, 96));
    assert_eq!(tentacle::DAMAGE, 5);
    assert_eq!(tentacle::STRIKE_BOX, Rect::new(-136, -44, 0, 39));
    assert_eq!(crack::HIT_RECT, Rect::new(-4, -64, 130, 63));
    assert_eq!(dynamite::STICK_AT, (37250, 3490));
    // Aquatis's five stalactites wear the cave stalactites' art.
    assert_eq!(aquatis::stalactite::IMAGE_SETS, [stalactite::IMAGE_SET]);
    assert_eq!(
        aquatis::stalactite::ANIM_PREFIXES,
        stalactite::ANIM_PREFIXES
    );
}

#[test]
fn the_twelfth_levels_state_numbers_hold_the_boss_and_the_tentacles() {
    let block = aquatis::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2280, 2299));
    let named = [
        (EnemyState::HighPunch, 2280, "HighPunch"),
        (EnemyState::LowPunch, 2281, "LowPunch"),
        (EnemyState::SuperFrenzy, 2282, "SuperFrenzy"),
        (EnemyState::StalactiteHit, 2283, "StalactiteHit"),
        (EnemyState::Scared, 2284, "Scared"),
        (EnemyState::StartGrab, 2290, "StartGrab"),
        (EnemyState::Squeeze, 2291, "Squeeze"),
        (EnemyState::Respawn, 2292, "Respawn"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(
        aquatis::STATES,
        named[..5].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    assert_eq!(
        tentacle::STATES,
        named[5..].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    // The eleventh level's block ends where the twelfth's starts.
    assert_eq!(
        *enemy::mercat::STATE_BLOCK.end() + 1,
        *block.start(),
        "blocks of 20"
    );
    // The shared states they reach keep their numbers.
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::HitLow as i32, 2004);
    assert_eq!(EnemyState::HitHigh as i32, 2005);
    assert_eq!(EnemyState::Dead as i32, 2006);
    assert_eq!(EnemyState::DeadAct1 as i32, 2205);
    assert_eq!(EnemyState::Lifted as i32, 2017);
    assert_eq!(EnemyState::Dropped as i32, 2021);
    // The stalactites and the triggers keep states of their own.
    for s in [
        stalactite::state::IDLE,
        stalactite::state::DELAY,
        stalactite::state::FALLING,
        stalactite::state::DEAD,
        stalactite::state::RESTART,
        shake::state::ARMED,
        shake::state::WAIT,
    ] {
        assert_eq!(EnemyState::from_id(s), None, "{s}");
    }
}

#[test]
fn the_link_starts_cleared_lasts_and_is_copied_with_the_game() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(*sim.aquatis_link(), AquatisLink::default());
    sim.with_world(|w| {
        assert_eq!(w.aquatis(), None);
        assert_eq!(w.aquatis_reset_bits(), 0);
        assert!(!w.aquatis_quake());
        assert!(!w.tentacle_grab());
        assert_eq!(w.aquatis_link().stalactite_order, 1);
        assert_eq!(w.aquatis_link().frenzy_toggle, 0);
    });
    // Written through the world, it lasts across ticks and is copied with
    // the game.
    sim.with_world(|w| {
        w.reset_aquatis();
        w.set_tentacle_grab(true);
        w.aquatis_link_mut().quake = true;
        w.aquatis_link_mut().land_y = 3550;
    });
    idle(&mut sim, 3);
    let copy = sim.clone();
    assert_eq!(copy.aquatis_link().reset_bits, aquatis::RESET_BITS);
    assert!(copy.aquatis_link().tentacle_grab);
    sim.with_world(|w| {
        assert!(w.aquatis_quake());
        // Each helper clears its own bit once.
        assert!(w.clear_aquatis_reset_bit(aquatis::CRACK_BIT));
        assert!(!w.clear_aquatis_reset_bit(aquatis::CRACK_BIT));
        for bit in aquatis::TENTACLE_BITS
            .iter()
            .chain(&aquatis::STALACTITE_BITS)
        {
            assert!(w.clear_aquatis_reset_bit(*bit));
        }
        assert_eq!(w.aquatis_reset_bits(), 0);
    });
    assert_eq!(copy.aquatis_link().land_y, 3550);
}

#[test]
fn the_help_screens_return_reaches_aquatis_file_without_a_draw() {
    // The return from the help screen passes its time to Aquatis's file:
    // with nothing built it changes nothing and draws nothing.
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 10);
    let rng = sim.rng_state();
    let link = *sim.aquatis_link();
    sim.pass_menu_time(3000);
    assert_eq!(sim.rng_state(), rng);
    assert_eq!(*sim.aquatis_link(), link);
    // A stalactite is told apart from other objects for his file.
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at(stalactite::NAMES[0], 1300, 700, 1010));
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 2);
    let (stal, claw) = {
        let find = |k: LogicKind| {
            sim.objects()
                .all()
                .find(|(_, o)| o.kind == k)
                .map(|(id, _)| id)
                .unwrap()
        };
        (
            find(LogicKind::Game(GameLogic::Stalactite)),
            find(LogicKind::Player),
        )
    };
    sim.with_world(|w| {
        assert!(aquatis::is_stalactite(w, stal));
        assert!(!aquatis::is_stalactite(w, claw));
    });
}

#[test]
fn the_twelfth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(12, SurfaceKind::Elevator),
        Rect::new(-22, -18, 18, 72)
    );
    assert_eq!(
        platform::surface(12, SurfaceKind::TogglePeg),
        Rect::new(-24, -8, 24, 64)
    );
    assert_eq!(
        platform::surface(12, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(12, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    // Springboards: the rocks in the water, with the seventh level's
    // rectangle.
    let s = springboard::level_table(12).expect("rocks");
    assert_eq!(s.rect, (-24, 89, 23, 130));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_ROCKSPRING_IDLE", "LEVEL_ROCKSPRING_SPRING")
    );
    assert_eq!(springboard::level_table(7), Some(s));
    // Water death tiles that kill as goo does, the splash 50 px above.
    assert!(goo::is_goo_level(12));
    assert_eq!(goo::splash_y(12), Some(50));
    // The Mercats' and Sirens' loot holds code 26, which no eleventh-level
    // Mercat carries: the shared table has it.
    assert!(pickup::powerup(26).is_some());
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        aquatis::IMAGE_SET,
        crack::IMAGE_SET,
        dynamite::IMAGE_SET,
        tentacle::IMAGE_SET,
        stalactite::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        aquatis::IDLE_ANIM,
        aquatis::HIGH_PUNCH_ANIM,
        aquatis::LOW_PUNCH_ANIM,
        aquatis::SCARED_ANIM,
        aquatis::DEATH_ANIM,
        crack::GLOW_ANIM,
        tentacle::IDLE_ANIM,
        tentacle::STRIKE_ANIM,
        tentacle::GRAB_ANIM,
        tentacle::SQUEEZE_ANIM,
        stalactite::HOME_ANIM,
        stalactite::CRUMBLE_ANIM,
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
fn install_level12_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    assert_eq!(level.number, 12);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL12 inert census: {:?}", d.inert_logics);
    // The three signs name no logic: they are reported as unknown, and
    // never run or collide.
    assert_eq!(
        d.unknown_logics,
        BTreeMap::from([("Sign".to_string(), 3)]),
        "unknown logics"
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 3193, "the level's records");
    assert_eq!(level.start, (1715, 2200));

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
        ("BehindCandy", 998),
        ("GlitterlessPowerup", 565),
        ("GooCoverup", 560),
        ("BehindAniCandy", 209),
        ("TreasurePowerup", 109),
        ("CrumblingPeg", 83),
        ("Mercat", 52),
        ("TogglePeg", 48),
        ("DoNothing", 38),
        ("HealthPowerup", 38),
        ("Siren", 35),
        ("TogglePeg2", 26),
        ("FloorSpike", 25),
        ("FloorSpike3", 22),
        ("FloorSpike4", 20),
        ("TogglePeg3", 19),
        ("TogglePeg4", 18),
        ("ClawDialogSoundTrigger", 17),
        ("Fish", 15),
        ("Checkpoint", 13),
        ("FloorSpike2", 12),
        ("Elevator", 11),
        ("SpecialPowerup", 11),
        ("SpringBoard", 10),
        ("AmmoPowerup", 7),
        ("CursePowerup", 6),
        ("GlobalAmbientSound", 3),
        ("MagicPowerup", 2),
        ("PathElevator", 2),
        ("BossStager", 1),
        ("BossWarp", 1),
        ("FirstSuperCheckpoint", 1),
        ("SecondSuperCheckpoint", 1),
        ("TriggerElevator", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // None of the earlier levels' other enemies and bosses.
    for name in [
        "Officer",
        "Soldier",
        "Rat",
        "CutThroat",
        "RobberThief",
        "TownGuard1",
        "TownGuard2",
        "Seagull",
        "RedTailPirate",
        "BearSailor",
        "PunkRat",
        "HermitCrab",
        "CrabNest",
        enemy::hook_pirate::NAMES[0],
        "PegLeg",
        "Raux",
        "Katherine",
        "Wolvington",
        "Gabriel",
        "Marrow",
        "Parrot",
        "MarrowFloor",
        "PowderKeg",
        "Laser",
        "ConveyorBelt",
        "AniRope",
        "SteppingStone",
        "GooVent",
        "SawBlade",
        "EndOfLevelPowerup",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level12_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    let preloaded = level.image_sets();
    for set in [
        aquatis::IMAGE_SET,
        crack::IMAGE_SET,
        dynamite::IMAGE_SET,
        tentacle::IMAGE_SET,
        stalactite::IMAGE_SET,
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The stalactite: its hanging picture's box, seven pictures, the
    // crumble done on its 19th call; the snap and the crash.
    assert_eq!(level.image_frames(stalactite::IMAGE_SET), Some((1, 7)));
    assert_eq!(
        level.frame_box(stalactite::IMAGE_SET, 1),
        Some(stalactite::BOX)
    );
    assert_eq!(
        calls(&level, stalactite::CRUMBLE_ANIM) as u32,
        stalactite::CRUMBLE_TICKS
    );
    assert_eq!(calls(&level, stalactite::HOME_ANIM), 1);
    for s in [stalactite::SNAP_SOUND, stalactite::HIT_SOUND] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // The rumbles.
    for s in shake::RUMBLES {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // Aquatis's, the crack's and the tentacles' animations.
    for a in [
        "HITHIGH", "HITLOW", "IDLE1", "IDLE2", "IDLE3", "KILLFALL", "STRIKE1", "STRIKE2",
    ] {
        let key = format!("LEVEL_AQUATIS_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    for a in [
        "IDLE", "STRIKE1", "STRIKE2", "STRIKE3", "KILLFALL", "RESPAWN",
    ] {
        let key = format!("LEVEL_AQUATISTENTACLE_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    assert!(level.animations.contains_key(crack::GLOW_ANIM));
    // The tentacle's strike done on its 20th call, the grab on its 18th.
    assert_eq!(calls(&level, tentacle::STRIKE_ANIM), 20);
    assert_eq!(calls(&level, tentacle::GRAB_ANIM), 18);
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level12_stalactites_and_triggers_take_their_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    // Every stalactite has a trigger rectangle and no delay, at depth 1010.
    for r in records(&level, stalactite::NAMES[0]) {
        let [l, t, rr, b] = stalactite::TRIGGER_FIELDS.map(|i| r.record[i]);
        assert!(l < rr && t < b, "{:?}", (r.record[5], r.record[6]));
        assert_eq!(r.record[stalactite::DELAY_FIELD], 0);
        assert_eq!(r.record[7], 1010);
        assert_eq!(r.image_set, stalactite::IMAGE_SET);
    }
    // The first three, over the water's edge: a shared rectangle within
    // 2 px, 777 202 px over the ledge.
    let first = records(&level, stalactite::NAMES[0])
        .into_iter()
        .find(|r| (r.record[5], r.record[6]) == (3135, 3422))
        .expect("stalactite 777");
    let rect = stalactite::TRIGGER_FIELDS.map(|i| first.record[i]);
    assert!(stalactite::in_trigger(rect, (3070, 3607)));
    assert!(!stalactite::in_trigger(rect, (3110, 3607)));
    let floor = (3422 + 44..3800)
        .find(|&y| level.map.attr(3135, y) == SOLID)
        .unwrap();
    assert_eq!(floor - (3422 + 44), 202);
    assert_eq!(stalactite::fall_ticks(202), 25);
    // Every trigger has a box; the first covers the first shaft.
    let shakes = records(&level, shake::NAMES[0]);
    for r in &shakes {
        let [l, t, rr, b] = shake::ATTACK_FIELDS.map(|i| r.record[i]);
        assert!(l < rr && t < b, "{:?}", (r.record[5], r.record[6]));
    }
    let first = shakes
        .iter()
        .find(|r| (r.record[5], r.record[6]) == (2979, 3302))
        .expect("the first trigger");
    assert_eq!(
        shake::ATTACK_FIELDS.map(|i| first.record[i]),
        [2879, 3199, 3072, 3392]
    );
    // Awake, each is set up: the stalactites hang with their boxes, the
    // triggers wait with theirs.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let mut n = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Stalactite" => {
                assert_eq!(o.state, stalactite::state::IDLE);
                assert_eq!(o.hit_rect, stalactite::BOX);
                *n.entry("stalactites").or_insert(0) += 1;
            }
            "Shake" => {
                assert_eq!(o.state, shake::state::ARMED);
                assert!(o.attack_rect.is_set());
                *n.entry("shakes").or_insert(0) += 1;
            }
            _ => {}
        }
    }
    assert_eq!(n, BTreeMap::from([("shakes", 30), ("stalactites", 170)]));
}

#[test]
fn install_level12_reused_families_take_the_twelfth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    // The water: one Death band from x 2560 to 38399, y 3776 to 3839, a
    // clear gap of 20 px and the Solid floor under it.
    for x in [2560, 20000, 38399] {
        assert_eq!(level.map.attr(x, 3776), DEATH, "{x}");
        assert_eq!(level.map.attr(x, 3839), DEATH, "{x}");
        assert_ne!(level.map.attr(x, 3840), DEATH, "{x}");
        assert_ne!(level.map.attr(x, 3840), SOLID, "{x}");
        assert_eq!(level.map.attr(x, 3860), SOLID, "{x}");
    }
    assert_ne!(level.map.attr(2559, 3800), DEATH);
    assert!(level.sound_ms("LEVEL_DEATHTILE").is_some());
    // The water covers: 560, drawn in front of Claw.
    let covers = records(&level, "GooCoverup");
    assert_eq!(covers.len(), 560);
    // The springboard rocks.
    assert!(
        records(&level, "SpringBoard")
            .iter()
            .all(|r| r.image_set == "LEVEL_WATERROCK")
    );
    // Awake: the pegs and elevators take the level's surface; the toggle
    // peg beside the boss, which stays out for good, stays out.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let boss_peg = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "TogglePeg" && (o.x, o.y) == (37781, 3655))
        .map(|(id, _)| id)
        .expect("the toggle peg beside the boss");
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Elevator" | "TriggerElevator" => {
                assert_eq!(o.hit_rect, platform::surface(12, SurfaceKind::Elevator));
                *kinds.entry(o.logic.to_string()).or_default() += 1;
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(o.hit_rect, platform::surface(12, SurfaceKind::TogglePeg));
                *kinds.entry("pegs".into()).or_default() += 1;
            }
            _ => {}
        }
    }
    assert_eq!(
        kinds,
        BTreeMap::from([
            ("Elevator".to_string(), 11),
            ("TriggerElevator".to_string(), 1),
            ("pegs".to_string(), 111),
        ])
    );
    for _ in 0..600 {
        sim.step(TickInput::default());
        let o = sim.objects().get(boss_peg).unwrap();
        assert_eq!((o.state, o.frame), (peg_state::STOPPED, 1));
    }
}

#[test]
fn install_level12_the_water_kills_by_its_centre_pixel() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    // Put down over open water left of the ledge at the water's edge, Claw
    // falls into the Death band and dies in it.
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some((2700, 3500)));
    let mut died = None;
    for k in 0..200 {
        sim.step(TickInput::default());
        if sim.player().unwrap().state == claw_state::DEATH {
            died = Some((k, sim.player().unwrap().y));
            break;
        }
    }
    let (_, y) = died.expect("he dies in the water");
    assert!((3715..=3728).contains(&y), "{y}");
    assert!(sim.claw().death_tile_hits > 0, "on its Death tiles");
    // Put down with his centre left of the band's first column, he falls
    // past it onto the floor under the water and stands there.
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some((2559, 3600)));
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    let c = sim.player().unwrap();
    assert_eq!((c.x, c.y), (2559, 3799));
    assert_ne!(c.state, claw_state::DEATH);
    assert_eq!(sim.claw().death_tile_hits, 0);
    assert!(
        !sim.last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::ClawDied { .. }))
    );
}

#[test]
fn install_level12_the_ledges_edge_refuses_dynamite() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    // At the end of the ledge at the water's edge Claw teeters.
    sim.with_world(|w| w.claw.special.warp_to = Some((2884, 3567)));
    idle(&mut sim, 60);
    assert_eq!(sim.player().unwrap().state, claw_state::ON_EDGE);
    let sticks = sim.claw().stats.dynamite;
    sim.step(press(Buttons::DYNAMITE.0));
    for _ in 0..80 {
        sim.step(TickInput::default());
    }
    assert_eq!(sim.claw().stats.dynamite, sticks, "no stick leaves");
    assert!(
        !sim.objects().all().any(|(_, o)| &*o.logic == "Dynamite"),
        "no stick in the air"
    );
}

/// The start lines the fight-pocket route's copy carries: Claw's progress
/// as the level starts, with nine sticks of dynamite.
const POCKET_START: &str = "\
# start_score = 0
# start_health = 100
# start_lives = 7
# start_pistol = 10
# start_magic = 5
# start_dynamite = 9
# start_weapon = pistol
# start_treasure_game = 0, 0, 0, 0, 0, 0, 0, 0, 0
";

#[test]
fn install_level12_the_fight_pocket_starts_with_nine_sticks() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    let carry = parse_start_lines(POCKET_START)
        .unwrap()
        .expect("carried in");
    assert_eq!(carry.dynamite, 9);
    // As the replay starts a store's route: Claw put at the start point on
    // the level's first tick and left there until time zero.
    let cfg = SimConfig {
        tuning: claw_sim::input::ScriptTuning::Original.tuning(),
        seed: 111_567,
        ..SimConfig::default()
    };
    let mut sim = Sim::with_start(
        level,
        cfg,
        StartState {
            carry: Some(carry),
            save: None,
        },
    );
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some((38208, 3503)));
    while sim.tick() * TICK < 828 {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| *w.rng = Rng::new(111_567));
    let s = sim.claw().stats.clone();
    assert_eq!(
        (s.dynamite, s.health, s.lives, s.pistol, s.magic, s.score),
        (9, 100, 7, 10, 5, 0)
    );
    let c = sim.player().unwrap();
    assert_eq!((c.x, c.y), (38208, 3523), "settled in the pocket");
    // The store's first row shows the same.
    let Some(root) = std::env::var_os("CLAW_REFERENCE") else {
        return;
    };
    let path = PathBuf::from(root).join("level12/boss-fight-pocket-c18/trace-c18-r1.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows = claw_sim::trace::parse(&text).unwrap();
    let r = &rows[0];
    assert_eq!(
        (r.dynamite, r.health, r.lives, r.pistol, r.magic, r.score),
        (9, 100, 7, 10, 5, 0)
    );
    assert_eq!((r.x, r.y), (38208, 3523));
}

#[test]
fn install_level12_costs_at_its_start() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 12);
    // The level's start with every family it places: how long a copy of
    // the game and an idle tick take, for the slice's measurements.
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
        "LEVEL12 start: {} objects, one copy {clone:?}, one idle tick {tick:?}",
        sim.objects().all().count()
    );
    assert!(sim.objects().all().count() >= 3190 - 20);
}
