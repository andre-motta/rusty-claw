//! The framework the ninth level's families build on: the hook-handed
//! pirate, PegLeg, the dart traps, the conveyor belts, the saw blades and
//! the skull cannons are declared, and each stays an inert placeholder
//! until its family is built; the two enemies' classes, data and state
//! numbers; the hook-handed pirate's name, spelled only in his own module
//! and registered from there; the level's platform, springboard and
//! splash table; and, on the install, the level's inert census, its
//! records of the new families, the art they need loaded with it, and
//! what the earlier families do with the level (the springboxes, the
//! seagulls, the elevators, the ropes, the tubes and signs and the kegs).
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
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, field, gabriel, goo, hook_pirate, pegleg};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::{
    self, GameLogic, ObjectData, conveyor_belt, dart_trap, keg, rope, saw_blade, skull_cannon,
    springboard, stepping_stone,
};
use claw_sim::object::{LogicKind, Rect};
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The families declared for the ninth level, with the names they claim.
const DECLARED: [(GameLogic, &[&str]); 6] = [
    (GameLogic::HookPirate, hook_pirate::NAMES),
    (GameLogic::PegLeg, &["PegLeg"]),
    (GameLogic::DartTrap, &["TProjectile"]),
    (GameLogic::ConveyorBelt, &["ConveyorBelt"]),
    (
        GameLogic::SawBlade,
        &["SawBlade", "SawBlade2", "SawBlade3", "SawBlade4"],
    ),
    (GameLogic::SkullCannon, &["SkullCannon"]),
];

/// Logics the ninth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the plain
/// and animated candy, the tubes and signs, the `DoNothing` pictures),
/// platform (the elevators, the path elevators), springboard, rope,
/// crate, keg, pickup, warp, checkpoint, sound object and enemy (the
/// seagulls) the level places outside the declared families already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the ninth level places, with
/// the hook-handed pirate's first; no level places the fourth saw.
const PLACED: [(&str, usize); 8] = [
    ("PegLeg", 23),
    ("TProjectile", 133),
    ("ConveyorBelt", 479),
    ("SawBlade", 20),
    ("SawBlade2", 9),
    ("SawBlade3", 6),
    ("SawBlade4", 0),
    ("SkullCannon", 11),
];

/// Hook-handed pirates on the ninth level.
const HOOK_PIRATES: usize = 45;

/// The record field holding a dart trap's arrow picture.
const ARROW_FRAME: usize = field::USER_0;
/// The record field after the horizontal speed: the vertical speed.
const SPEED_Y: usize = field::SPEED_X + 1;

/// The hook-handed pirate's name.
fn hook() -> &'static str {
    hook_pirate::NAMES[0]
}

/// `r` mirrored left to right.
fn mirrored(r: Rect) -> Rect {
    Rect::new(-r.right, r.top, -r.left, r.bottom)
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
    // The hook-handed pirate has one name, spelled in his own module.
    assert_eq!(hook_pirate::NAMES.len(), 1);
    // The families' own constants agree with the registry.
    assert_eq!(logic::built(GameLogic::HookPirate), hook_pirate::BUILT);
    assert_eq!(logic::built(GameLogic::PegLeg), pegleg::BUILT);
    assert_eq!(logic::built(GameLogic::DartTrap), dart_trap::BUILT);
    assert_eq!(logic::built(GameLogic::ConveyorBelt), conveyor_belt::BUILT);
    assert_eq!(logic::built(GameLogic::SawBlade), saw_blade::BUILT);
    assert_eq!(logic::built(GameLogic::SkullCannon), skull_cannon::BUILT);
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
    // The enemies, the dart traps and the skull cannons wake with the
    // enemies' window, the belts and the saws with the small one; none
    // keeps the original window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for (name, class) in [
        (hook(), 0),
        ("PegLeg", 0),
        ("TProjectile", 0),
        ("SkullCannon", 0),
        ("ConveyorBelt", 2),
        ("SawBlade", 2),
        ("SawBlade2", 2),
        ("SawBlade3", 2),
        ("SawBlade4", 2),
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
// The data literals below end in `..Default::default()` so they keep
// compiling when the two enemies gain fields of their own.
#[allow(clippy::needless_update)]
fn the_two_enemies_are_human_classes_whose_data_holds_enemy_data() {
    for (g, name) in [
        (GameLogic::HookPirate, hook()),
        (GameLogic::PegLeg, "PegLeg"),
    ] {
        let class = enemy::class(g).expect("a class");
        assert_eq!(class.name, name);
        assert!(class.human, "{name} is a human enemy");
    }
    for g in [
        GameLogic::DartTrap,
        GameLogic::ConveyorBelt,
        GameLogic::SawBlade,
        GameLogic::SkullCannon,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?} is no enemy");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::HookPirate(hook_pirate::HookPirateData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::PegLeg(pegleg::PegLegData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::DartTrap(Default::default()),
        ObjectData::ConveyorBelt(Default::default()),
        ObjectData::SawBlade(Default::default()),
        ObjectData::SkullCannon(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Their shots are the projectile family's.
    for name in [
        pegleg::bullet::LOGIC,
        dart_trap::arrow::LOGIC,
        skull_cannon::ball::LOGIC,
    ] {
        assert_eq!(
            logic::kind_for(name),
            Some(LogicKind::Game(GameLogic::Projectile)),
            "{name}"
        );
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    assert_eq!(pegleg::bullet::DAMAGE, 10);
    assert_eq!(dart_trap::arrow::DAMAGE, 5);
    assert_eq!(skull_cannon::ball::DAMAGE, 10);
    assert_eq!(saw_blade::DAMAGE, 10);
}

#[test]
fn the_ninth_levels_state_numbers_hold_the_rolls_and_the_hook_hold() {
    let block = hook_pirate::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2220, 2239));
    let named = [
        (EnemyState::RollForward, 2220, "RollForward"),
        (EnemyState::RollBackward, 2221, "RollBackward"),
        (EnemyState::HoldHook, 2222, "HoldHook"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(pegleg::STATES, [named[0].0, named[1].0]);
    assert_eq!(hook_pirate::STATES, [named[2].0]);
    // The eighth level's block ends where the ninth's starts.
    assert_eq!(*gabriel::STATE_BLOCK.end() + 1, *block.start());
    // The shared states they reach keep their numbers: the parry is the
    // second town guard's, PegLeg's shot the shared one.
    assert_eq!(EnemyState::Block as i32, 2141);
    assert_eq!(EnemyState::Fire as i32, 2013);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::HitLow as i32, 2004);
    assert_eq!(EnemyState::Lifted as i32, 2017);
    assert_eq!(EnemyState::Thrown as i32, 2019);
    // The objects that are no enemies keep states of their own; the saws
    // run the floor spikes'.
    for s in [
        dart_trap::state::FIRING,
        dart_trap::state::WAIT,
        conveyor_belt::state::MOVE,
    ] {
        assert_eq!(EnemyState::from_id(s), None, "{s}");
    }
    assert_eq!(saw_blade::state::UP, logic::floor_spike::state::UP);
    assert_eq!(saw_blade::state::DOWN, 1007);
}

#[test]
fn the_ninth_levels_table() {
    // Platform rectangles, each with its 32 px depth: no toggle peg.
    assert_eq!(
        platform::surface(9, SurfaceKind::Elevator),
        Rect::new(-44, -8, 41, 64)
    );
    assert_eq!(
        platform::surface(9, SurfaceKind::TogglePeg),
        Rect::new(0, 0, 0, 32)
    );
    assert_eq!(
        platform::surface(9, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(9, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    assert_eq!(stepping_stone::surface(9), Rect::new(-16, -1, 16, 64));
    // Springboards: the springbox.
    let s = springboard::level_table(9).expect("springboxes");
    assert_eq!(s.rect, (-24, 0, 22, 40));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGBOX_IDLE", "LEVEL_SPRINGBOX_SPRING")
    );
    assert_eq!(springboard::surface(9), Rect::new(-24, 0, 22, 72));
    // Goo death tiles, with a 50 px splash.
    assert!(goo::is_goo_level(9));
    assert_eq!(goo::splash_y(9), Some(50));
    // The two enemies' ninth level rules.
    assert!(hook_pirate::holds_and_parries_swords(9));
    assert!(pegleg::aims_at_posture(9));
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        hook_pirate::image_set(),
        pegleg::IMAGE_SET,
        pegleg::bullet::IMAGE_SET,
        dart_trap::arrow::IMAGE_SET,
        skull_cannon::IMAGE_SET,
        skull_cannon::ball::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    let hook_anims: Vec<String> = [
        hook_pirate::anim::ADVANCE,
        hook_pirate::anim::FAST_ADVANCE,
        hook_pirate::anim::STRIKE1,
        hook_pirate::anim::STRIKE2,
        hook_pirate::anim::BLOCK,
        hook_pirate::anim::IDLE1,
        hook_pirate::anim::HIT_HIGH,
        hook_pirate::anim::HIT_LOW,
    ]
    .iter()
    .map(|a| hook_pirate::key(a))
    .collect();
    let anims = [
        pegleg::HIGH_SHOT_ANIM,
        pegleg::LOW_SHOT_ANIM,
        pegleg::FORWARD_ROLL_ANIM,
        pegleg::BACKWARD_ROLL_ANIM,
        pegleg::IDLE_ANIM,
        "LEVEL_SHOOTERS_PUFFDARTLEFT",
        "LEVEL_SHOOTERS_PUFFDARTUP",
        conveyor_belt::cycle_anim(-300),
        conveyor_belt::cycle_anim(175),
        saw_blade::UP_ANIM,
        saw_blade::SPIN_ANIM,
        saw_blade::DOWN_ANIM,
        skull_cannon::FIRING_ANIM,
        skull_cannon::HOME_ANIM,
    ];
    for anim in hook_anims.iter().map(String::as_str).chain(anims) {
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

/// Whether the animation `key` loops (never finishes).
fn loops(level: &SimLevel, key: &str) -> bool {
    level.animations[key].frames.iter().all(|f| f.next_op == 0)
}

#[test]
fn install_level9_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 9);
    assert_eq!(level.number, 9);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL9 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 2847, "the level's records");
    assert_eq!(level.start, (1600, 7700));

    let placed = |name: &str| records(&level, name).len();
    assert_eq!(placed(hook()), HOOK_PIRATES);
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
        ("SpringBoard", 54),
        ("Seagull", 49),
        ("PathElevator", 35),
        ("AniRope", 26),
        ("AniCycle", 203),
        ("Elevator", 9),
        ("OneWayTriggerElevator", 3),
        ("OneWayStartElevator", 2),
        ("PowderKeg", 43),
        ("SpecialPowerup", 42),
        ("AmbientSound", 32),
        ("EndOfLevelPowerup", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // None of the earlier levels' enemies, bosses or hazards the ninth
    // does not use: the gem ends the level.
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
        "TowerCannonLeft",
        "TowerCannonRight",
        "FloorSpike",
        "TogglePeg",
        "CrumblingPeg",
        "SteppingStone",
        "BreakPlank",
        "JumpSwitch",
        "GooCoverup",
        "GooVent",
        "GroundBlower",
        "BossStager",
        "BossWarp",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level9_records_of_the_new_families() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 9);
    let at = |r: &ObjectSpec| (r.record[5], r.record[6]);

    // The hook-handed pirates and PegLegs: their own pictures, depth 5000,
    // no animation, never standing still.
    for (name, set, n) in [
        (hook(), hook_pirate::image_set(), HOOK_PIRATES),
        ("PegLeg", pegleg::IMAGE_SET, 23),
    ] {
        let rs = records(&level, name);
        assert_eq!(rs.len(), n, "{name}");
        for r in rs {
            assert_eq!(r.image_set, set, "{name}");
            assert_eq!(r.record[7], 5000, "{name} depth");
            assert!(r.anim.is_empty(), "{name} names no animation");
            assert_eq!(r.record[field::USER_0], 0, "{name} never stands still");
        }
    }
    let bounded = |name: &str| {
        records(&level, name)
            .iter()
            .filter(|r| r.record[field::X_MIN] != 0 && r.record[field::X_MAX] != 0)
            .count()
    };
    assert_eq!(bounded(hook()), 39);
    assert_eq!(bounded("PegLeg"), 17);

    // The dart traps: each picture set aims one way and shoots its arrow.
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
            (("LEVEL_SHOOTERS_PUFFDARTDOWN".to_string(), 2, 3), 43),
            (("LEVEL_SHOOTERS_PUFFDARTLEFT".to_string(), 4, 2), 45),
            (("LEVEL_SHOOTERS_PUFFDARTRIGHT".to_string(), 6, 1), 37),
            (("LEVEL_SHOOTERS_PUFFDARTUP".to_string(), 8, 4), 8),
        ])
    );
    for r in records(&level, "TProjectile") {
        // Each names its own set's puff, and every arrow picture points
        // the way its trap shoots.
        assert_eq!(r.anim, r.image_set, "{:?}", at(r));
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
        // A trigger rectangle and a wait.
        assert!(r.record[field::X_MAX] > r.record[field::X_MIN]);
        assert!(r.record[field::Y_MAX] > r.record[field::Y_MIN]);
        assert!(r.record[dart_trap::WAIT_FIELD] > 0);
    }
    // The trap of the reference run's dart room.
    let room = records(&level, "TProjectile")
        .into_iter()
        .find(|r| at(r) == (1558, 7212))
        .expect("the dart room's trap");
    assert_eq!(room.image_set, "LEVEL_SHOOTERS_PUFFDARTRIGHT");
    assert_eq!(room.record[dart_trap::WAIT_FIELD], 1000);
    assert_eq!(
        (room.record[field::X_MIN], room.record[field::X_MAX]),
        (1625, 1910)
    );

    // The belt pieces: 43 belts of a left end, middles and a right end,
    // all at depth 1000.
    let pieces = tally(&level, "ConveyorBelt", |r| r.image_set.clone());
    assert_eq!(
        pieces,
        BTreeMap::from([
            (conveyor_belt::PIECES[0].to_string(), 43),
            (conveyor_belt::PIECES[1].to_string(), 393),
            (conveyor_belt::PIECES[2].to_string(), 43),
        ])
    );
    let speeds = tally(&level, "ConveyorBelt", |r| {
        r.record[conveyor_belt::SPEED_FIELD]
    });
    assert_eq!(
        speeds,
        BTreeMap::from([
            (-500, 69),
            (-300, 3),
            (-250, 25),
            (-200, 7),
            (-150, 87),
            (-100, 26),
            (75, 1),
            (100, 4),
            (150, 50),
            (175, 164),
            (300, 10),
            (500, 33),
        ])
    );
    for r in records(&level, "ConveyorBelt") {
        assert_eq!(r.record[7], 1000);
        assert!(r.anim.is_empty());
    }
    let clamped = tally(&level, "ConveyorBelt", |r| {
        conveyor_belt::clamp_speed(r.record[conveyor_belt::SPEED_FIELD])
    });
    assert_eq!(clamped[&-300], 72);
    assert_eq!(clamped[&300], 43);

    // The saws: one picture, no animation, the default speeds.
    for name in &saw_blade::NAMES[..3] {
        for r in records(&level, name) {
            assert_eq!(r.image_set, saw_blade::IMAGE_SET, "{name}");
            assert!(r.anim.is_empty(), "{name}");
            assert_eq!(
                (r.record[field::SPEED_X], r.record[SPEED_Y]),
                (0, 0),
                "{name} at {:?}: the default halves",
                at(r)
            );
            assert_eq!(
                saw_blade::phase_of(name, r.record[saw_blade::SPEED_FIELD]),
                saw_blade::phase_of(name, 0),
                "{name}: the default phase"
            );
        }
    }
    let pair: Vec<_> = ["SawBlade", "SawBlade2"]
        .iter()
        .flat_map(|n| records(&level, n))
        .map(&at)
        .filter(|&(x, y)| y == 6716 && (3300..3600).contains(&x))
        .collect();
    assert_eq!(pair, [(3358, 6716), (3555, 6716)], "the saw room's pair");

    // The skull cannons: the crossfire shaft, the column firing left and
    // the one guarding the early path; mirrored ones fire left.
    let mut cannons: Vec<_> = records(&level, "SkullCannon")
        .iter()
        .map(|r| (at(r), r.record[7], r.record[11] == 2))
        .collect();
    cannons.sort();
    assert_eq!(
        cannons,
        [
            ((4668, 7334), 0, false),
            ((7490, 6693), 0, false),
            ((7490, 7078), 0, false),
            ((7490, 7461), 0, false),
            ((8250, 7332), 50, true),
            ((8251, 6948), 50, true),
            ((8252, 6375), 50, true),
            ((12383, 6759), 0, true),
            ((12384, 6567), 0, true),
            ((12386, 6374), 0, true),
            ((12388, 6183), 0, true),
        ]
    );
    for r in records(&level, "SkullCannon") {
        assert_eq!(r.image_set, skull_cannon::IMAGE_SET);
        assert_eq!((r.record[field::SPEED_X], r.record[SPEED_Y]), (0, 0));
    }

    // The end of the level: no boss, the gem.
    let end = records(&level, "EndOfLevelPowerup");
    assert_eq!(end.into_iter().map(at).collect::<Vec<_>>(), [(24224, 4024)]);
}

#[test]
fn install_level9_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 9);
    let preloaded = level.image_sets();
    for (set, frames) in [
        (hook_pirate::image_set(), None),
        (pegleg::IMAGE_SET, None),
        (pegleg::bullet::IMAGE_SET, Some((1, 1))),
        (dart_trap::arrow::IMAGE_SET, Some((1, 4))),
        ("LEVEL_SHOOTERS_PUFFDARTLEFT", Some((1, 5))),
        ("LEVEL_SHOOTERS_PUFFDARTUP", Some((1, 5))),
        (conveyor_belt::PIECES[0], Some((1, 9))),
        (conveyor_belt::PIECES[1], Some((1, 9))),
        (conveyor_belt::PIECES[2], Some((1, 9))),
        (saw_blade::IMAGE_SET, Some((1, 20))),
        (skull_cannon::IMAGE_SET, Some((1, 7))),
        (skull_cannon::ball::IMAGE_SET, Some((1, 5))),
        ("LEVEL_SPRINGBOX1", Some((1, 3))),
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
    // The boxes the families take from their pictures: PegLeg's movement
    // box, the arrows', the belt pieces' push boxes, the saws' strike box,
    // the cannonball's and the hook-handed pirate's hold boxes (facing
    // right, so mirrored).
    assert_eq!(
        level.frame_box(pegleg::IMAGE_SET, 1),
        Some(pegleg::MOVE_RECT)
    );
    for frame in 1..=4 {
        assert_eq!(
            level.frame_box(dart_trap::arrow::IMAGE_SET, frame),
            dart_trap::arrow::frame_box(frame),
            "arrow {frame}"
        );
    }
    assert_eq!(
        level.frame_box(conveyor_belt::PIECES[0], 1),
        Some(Rect::new(-22, -30, 21, 21))
    );
    assert_eq!(
        level.frame_box(conveyor_belt::PIECES[1], 1),
        Some(Rect::new(-32, -22, 31, -3))
    );
    assert_eq!(
        level.frame_box(conveyor_belt::RIGHT_END, 1),
        Some(Rect::new(-31, -30, 32, 21))
    );
    let saw = level.frame_box(saw_blade::IMAGE_SET, 1).expect("a box");
    assert_eq!(saw_blade::strike_box(saw), Rect::new(-33, -10, 34, 17));
    assert_eq!(level.frame_box(saw_blade::IMAGE_SET, 20), Some(saw));
    assert_eq!(
        level.frame_box(skull_cannon::ball::IMAGE_SET, skull_cannon::ball::FRAME),
        Some(Rect::new(-9, -10, 22, 9))
    );
    for (frame, hold) in [200, 850, 851].into_iter().zip(hook_pirate::HOLD_BOXES) {
        let b = level
            .frame_box(hook_pirate::image_set(), frame)
            .expect("a hold picture");
        assert_eq!(mirrored(b), hold, "hold picture {frame}");
    }

    // The hook-handed pirate's animations and their timings at 18 ms: the
    // swing live from the 7th call, the stab from the 5th (to the 16th),
    // the parry and hold pose done on the 11th, showing its three
    // pictures for 4, 6 and the rest of the calls.
    let hook_anim = |a: &str| hook_pirate::key(a);
    for a in [
        hook_pirate::anim::ADVANCE,
        hook_pirate::anim::FAST_ADVANCE,
        hook_pirate::anim::IDLE1,
        hook_pirate::anim::HIT_HIGH,
        hook_pirate::anim::HIT_LOW,
    ] {
        assert!(
            level.animations.contains_key(&hook_anim(a)),
            "{a} is loaded"
        );
    }
    let (n, events) = calls(&level, &hook_anim(hook_pirate::anim::STRIKE1));
    assert_eq!(n as u32, hook_pirate::SWING_TICKS.1);
    assert_eq!(
        events.first().map(|&e| e as u32),
        Some(hook_pirate::SWING_TICKS.0)
    );
    let (n, events) = calls(&level, &hook_anim(hook_pirate::anim::STRIKE2));
    assert_eq!(n as u32, hook_pirate::STAB_TICKS.1);
    assert_eq!(
        events.first().map(|&e| e as u32),
        Some(hook_pirate::STAB_TICKS.0)
    );
    assert_eq!(events.last(), Some(&16));
    let block = hook_anim(hook_pirate::anim::BLOCK);
    assert_eq!(calls(&level, &block).0 as u32, hook_pirate::PARRY_TICKS);
    assert_eq!(
        runs(&pictures(&level, &block, 20)),
        [(200, 4), (850, 6), (851, 10)]
    );
    assert_eq!(
        calls(&level, &hook_anim(hook_pirate::anim::HIT_LOW)).0 as u32,
        hook_pirate::HIT_LOW_TICKS
    );
    for s in [
        hook_pirate::sounds::HOOK_SWING,
        hook_pirate::sounds::DAGGER_STAB,
        hook_pirate::sounds::BLOCK[0],
        hook_pirate::sounds::BLOCK[1],
        hook_pirate::sounds::DEATH,
        hook_pirate::sounds::ALERT[0],
        hook_pirate::sounds::ALERT[1],
    ] {
        assert!(level.sound_ms(&hook_pirate::key(s)).is_some(), "{s}");
    }

    // PegLeg's: each shot's bullet on the 19th call, done on the 31st; the
    // rolls and the idle pose loop.
    for anim in [pegleg::HIGH_SHOT_ANIM, pegleg::LOW_SHOT_ANIM] {
        let (n, events) = calls(&level, anim);
        assert_eq!(n as u32, pegleg::SHOT_TICKS.1, "{anim}");
        assert_eq!(
            events.first().map(|&e| e as u32),
            Some(pegleg::SHOT_TICKS.0),
            "{anim}"
        );
    }
    for anim in [
        pegleg::FORWARD_ROLL_ANIM,
        pegleg::BACKWARD_ROLL_ANIM,
        pegleg::IDLE_ANIM,
    ] {
        assert!(loops(&level, anim), "{anim} loops");
    }
    assert_eq!(
        calls(&level, "LEVEL_PEGLEG_HITHIGH").0 as u32,
        pegleg::HIT_HIGH_TICKS
    );
    for s in [pegleg::GUNSHOT_SOUND, pegleg::ROLL_SOUND] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }

    // The dart traps' puffs: pictures 1 to 5 for 6 calls each, done on
    // the 31st; aimed up, 2 to 5, done on the 25th; no event, so the
    // arrow leaves on the finishing call. The puff's sound is loaded.
    for side in ["LEFT", "RIGHT", "DOWN"] {
        let anim = format!("LEVEL_SHOOTERS_PUFFDART{side}");
        assert_eq!(
            calls(&level, &anim),
            (dart_trap::PUFF_TICKS as usize, vec![])
        );
        assert_eq!(
            runs(&pictures(&level, &anim, 30)),
            [(1, 6), (2, 6), (3, 6), (4, 6), (5, 6)]
        );
    }
    let up = "LEVEL_SHOOTERS_PUFFDARTUP";
    assert_eq!(
        calls(&level, up),
        (dart_trap::PUFF_UP_TICKS as usize, vec![])
    );
    assert_eq!(
        runs(&pictures(&level, up, 24)),
        [(2, 6), (3, 6), (4, 6), (5, 6)]
    );
    assert!(level.sound_ms(dart_trap::SOUND).is_some());

    // The belts' cycles loop.
    for speed in [-300, -150, -100, 0, 100, 175, 300] {
        let anim = conveyor_belt::cycle_anim(speed);
        assert!(loops(&level, anim), "{anim} loops");
    }

    // The saws: the rise 31 calls, pictures 1 to 10 three calls each; the
    // spin two calls a picture, looping; the sink 28 calls. The level has
    // no rise or sink sound.
    assert_eq!(
        calls(&level, saw_blade::UP_ANIM).0 as u32,
        saw_blade::RISE_TICKS
    );
    let rise = pictures(&level, saw_blade::UP_ANIM, 30);
    assert_eq!(runs(&rise), (1..=10).map(|p| (p, 3)).collect::<Vec<_>>());
    assert_eq!(
        calls(&level, saw_blade::DOWN_ANIM).0 as u32,
        saw_blade::SINK_TICKS
    );
    assert!(loops(&level, saw_blade::SPIN_ANIM));
    assert_eq!(
        runs(&pictures(&level, saw_blade::SPIN_ANIM, 8)),
        [(10, 2), (11, 2), (10, 2), (11, 2)]
    );
    for s in [saw_blade::RISE_SOUND, saw_blade::SINK_SOUND] {
        assert_eq!(level.sound_ms(s), None, "{s} is not on the level");
    }

    // The skull cannon: the ball on the first call of its firing, done on
    // the 43rd; waiting is one picture; its sounds are loaded; the level
    // has no cannonball animation.
    let (n, events) = calls(&level, skull_cannon::FIRING_ANIM);
    assert_eq!(n as u32, skull_cannon::FIRE_TICKS);
    assert_eq!(events.first(), Some(&1));
    assert_eq!(calls(&level, skull_cannon::HOME_ANIM).0, 1);
    for s in ["LEVEL_CANONSH1", "LEVEL_FLYUP"] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    assert!(!level.animations.contains_key("LEVEL_CANNONBALL_FALL"));

    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level9_reused_families_take_the_ninth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 9);
    let preloaded = level.image_sets();
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut springs: BTreeMap<String, usize> = BTreeMap::new();
    let mut platforms: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut cycles: BTreeMap<String, usize> = BTreeMap::new();
    let mut enemies: BTreeMap<String, usize> = BTreeMap::new();
    let mut ropes = 0;
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "SpringBoard" => {
                assert_eq!(o.hit_rect, springboard::surface(9));
                *springs.entry(o.image_set.clone()).or_default() += 1;
            }
            "PathElevator" | "Elevator" | "OneWayTriggerElevator" | "OneWayStartElevator" => {
                assert_eq!(
                    o.hit_rect,
                    platform::surface(9, SurfaceKind::Elevator),
                    "{} at {},{}",
                    o.logic,
                    o.x,
                    o.y
                );
                *platforms
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
            "Seagull" => {
                assert!(o.data.enemy().is_some(), "{} is built", o.logic);
                *enemies.entry(o.logic.to_string()).or_default() += 1;
            }
            _ => {}
        }
    }
    println!(
        "LEVEL9 springboards {springs:?}, platforms {platforms:?}, animated pieces {cycles:?}, \
         ropes {ropes}, enemies {enemies:?}"
    );
    assert_eq!(
        springs,
        BTreeMap::from([("LEVEL_SPRINGBOX1".to_string(), 54)])
    );
    let elevator = |logic: &str, n| ((logic.to_string(), "LEVEL_ELEVATOR".to_string()), n);
    assert_eq!(
        platforms,
        BTreeMap::from([
            elevator("Elevator", 9),
            elevator("OneWayStartElevator", 2),
            elevator("OneWayTriggerElevator", 3),
            elevator("PathElevator", 35),
        ])
    );
    assert_eq!(
        cycles,
        BTreeMap::from([
            ("LEVEL_SIGNDOWN".to_string(), 6),
            ("LEVEL_SIGNDOWNRIGHT".to_string(), 8),
            ("LEVEL_SIGNRIGHT".to_string(), 25),
            ("LEVEL_SIGNUP".to_string(), 5),
            ("LEVEL_SIGNUPRIGHT".to_string(), 26),
            ("LEVEL_TUBEDOWN".to_string(), 43),
            ("LEVEL_TUBELEFT".to_string(), 45),
            ("LEVEL_TUBERIGHT".to_string(), 37),
            ("LEVEL_TUBEUP".to_string(), 8),
        ])
    );
    assert_eq!(ropes, 26);
    assert_eq!(enemies, BTreeMap::from([("Seagull".to_string(), 49)]));
    for set in springs
        .keys()
        .chain(platforms.keys().map(|k| &k.1))
        .chain(cycles.keys())
    {
        assert!(preloaded.contains(set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The springbox's animations, and its higher launches: 700 and 600 px
    // on some records.
    let table = springboard::level_table(9).expect("springboxes");
    for anim in [table.idle_anim, table.spring_anim] {
        assert!(level.animations.contains_key(anim), "{anim} is loaded");
    }
    let heights = tally(&level, "SpringBoard", |r| {
        springboard::height_of(r.record[field::Y_MAX])
    });
    assert_eq!(heights.get(&700), Some(&4));
    assert_eq!(heights.get(&600), Some(&3));
    // The seagulls' animations are the eighth level's.
    let level8 = level_n(&store, 8);
    let mut same = 0;
    for (key, anim) in &level.animations {
        if key.starts_with("LEVEL_SEAGULL_") {
            assert_eq!(level8.animations.get(key), Some(anim), "{key}");
            same += 1;
        }
    }
    assert!(same > 0);
    // 43 kegs, all after Claw at depth 5000.
    let kegs = records(&level, keg::NAMES[0]);
    assert_eq!(kegs.len(), 43);
    assert!(kegs.iter().all(|r| r.record[7] == 5000));
}

#[test]
fn install_level9_costs_at_its_start() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 9);
    // The level's start with every family it places, the 479 belt pieces
    // among them: how long a copy of the game and an idle tick take, for
    // the slice's measurements along the bot route.
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
        "LEVEL9 start: {} objects, one copy {clone:?}, one idle tick {tick:?}",
        sim.objects().all().count()
    );
    assert!(sim.objects().all().count() >= 2847);
}
