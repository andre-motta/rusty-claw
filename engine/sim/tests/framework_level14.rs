//! The framework the fourteenth level's families build on: Omar, his
//! shield pieces and his energy balls are declared, and each stays an
//! inert placeholder until its family is built; the tiger guard and the
//! lava hazards (the fountains, the mouths, the hands and their balls)
//! run; the tiger's and Omar's classes, data and state numbers; the
//! seams the boss fight builds on (his ring on the level clock, a route's
//! set-up of Claw's health); the level's platform, springboard and lava
//! table; and, on the install, the level's inert census, the art the new
//! families need loaded with it, the records the new families take, what
//! the earlier families do with the level (the pegs' and elevators'
//! surfaces, the fire shooters' animated shots, the lava that kills), a
//! mouth's drip and the drop it holds after a lost life, and the level's
//! costs at its start.
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here. The free numbers of the fourteenth level's block
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
use claw_frame::{GameEvent, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::AnimPlayer;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::omar::{self, bullet, shield};
use claw_sim::logic::enemy::{self, EnemyState, attacker, goo, tiger_guard};
use claw_sim::logic::lava::{self, ball, geyser, hand, mouth};
use claw_sim::logic::platform::{self, SurfaceKind};
use claw_sim::logic::{self, GameLogic, ObjectData, dart_trap, springboard};
use claw_sim::object::{LogicKind, Object, Rect, flags, types};
use claw_sim::player::state as claw_state;
use claw_sim::registry::Registry;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The families declared for the fourteenth level, with the names they
/// claim.
const DECLARED: [(GameLogic, &[&str]); 8] = [
    (GameLogic::Omar, &["Omar"]),
    (GameLogic::OmarShield, &["OmarShield"]),
    (GameLogic::OmarBullet, &["OmarBullet"]),
    (GameLogic::TigerGuard, &["TigerGuard"]),
    (GameLogic::LavaGeyser, &["LavaGeyser"]),
    (GameLogic::LavaMouth, &["LavaMouth"]),
    (GameLogic::LavaHand, &["LavaHand"]),
    (GameLogic::LavaHandProjectile, &["LavaHandProjectile"]),
];

/// Logics the fourteenth level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration, platform,
/// pickup, warp, checkpoint, sound object, trap and enemy of the earlier
/// levels the level places already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each name of the new families the fourteenth level
/// places.
const PLACED: [(&str, usize); 5] = [
    ("Omar", 1),
    ("TigerGuard", 26),
    ("LavaGeyser", 23),
    ("LavaMouth", 19),
    ("LavaHand", 6),
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
    assert_eq!(logic::built(GameLogic::Omar), omar::BUILT);
    assert_eq!(logic::built(GameLogic::OmarShield), shield::BUILT);
    assert_eq!(logic::built(GameLogic::OmarBullet), bullet::BUILT);
    assert_eq!(logic::built(GameLogic::TigerGuard), tiger_guard::BUILT);
    assert_eq!(logic::built(GameLogic::LavaGeyser), geyser::BUILT);
    assert_eq!(logic::built(GameLogic::LavaMouth), mouth::BUILT);
    assert_eq!(logic::built(GameLogic::LavaHand), hand::BUILT);
    assert_eq!(logic::built(GameLogic::LavaHandProjectile), ball::BUILT);
    // The tigers and the lava hazards run.
    const { assert!(tiger_guard::BUILT && geyser::BUILT && mouth::BUILT && hand::BUILT && ball::BUILT) };
    for (name, g) in [
        ("TigerGuard", GameLogic::TigerGuard),
        ("LavaGeyser", GameLogic::LavaGeyser),
        ("LavaMouth", GameLogic::LavaMouth),
        ("LavaHand", GameLogic::LavaHand),
        ("LavaHandProjectile", GameLogic::LavaHandProjectile),
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
    let names: Vec<&str> = ["Omar"].into_iter().filter(|n| !built(n)).collect();
    let level = level_with(&names);
    let mut sim = Sim::new(level, SimConfig::default());
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
    // The fountains and the mouths wake with the small window, everything
    // else of the level's new families with the enemies'.
    for (name, class) in [
        ("TigerGuard", 0),
        ("LavaGeyser", 2),
        ("LavaMouth", 2),
        ("LavaHand", 0),
        ("LavaHandProjectile", 0),
        ("Omar", 0),
        ("OmarShield", 0),
        ("OmarBullet", 0),
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
fn the_tiger_and_omar_are_human_enemies() {
    let class = enemy::class(GameLogic::TigerGuard).expect("a class");
    assert_eq!(class.name, "TigerGuard");
    assert!(class.human, "a human enemy");
    assert!(class.carry.is_some(), "it can be lifted");
    let class = enemy::class(GameLogic::Omar).expect("a class");
    assert_eq!(class.name, "Omar");
    assert!(class.human, "a human enemy");
    assert!(class.carry.is_none(), "never lifted");
    for g in [
        GameLogic::OmarShield,
        GameLogic::OmarBullet,
        GameLogic::LavaGeyser,
        GameLogic::LavaMouth,
        GameLogic::LavaHand,
        GameLogic::LavaHandProjectile,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?}");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::TigerGuard(tiger_guard::TigerGuardData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Omar(omar::OmarData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    for d in [
        ObjectData::OmarShield(Default::default()),
        ObjectData::OmarBullet(Default::default()),
        ObjectData::LavaGeyser(Default::default()),
        ObjectData::LavaMouth(Default::default()),
        ObjectData::LavaHand(Default::default()),
        ObjectData::LavaHandProjectile(Default::default()),
    ] {
        assert_eq!(d.enemy(), None);
    }
    // Their numbers.
    assert_eq!((tiger_guard::HEALTH, tiger_guard::DAMAGE), (15, 10));
    assert_eq!(
        (tiger_guard::KNIFE_DAMAGE, tiger_guard::LUNGE_DAMAGE),
        (15, 20)
    );
    assert_eq!(tiger_guard::HIT_RECT, Rect::new(-20, -50, 20, 50));
    assert_eq!((omar::HEALTH, omar::DAMAGE), (100, 30));
    assert_eq!((omar::BLOW_DAMAGE, omar::BALL_DAMAGE), (25, 15));
    assert_eq!(omar::BODY_RECT, Rect::new(-11, -47, 23, 61));
    assert_eq!(shield::RECT, Rect::new(-14, -6, 12, 21));
    assert_eq!(lava::DAMAGE, 10);
}

#[test]
fn the_fourteenth_levels_state_numbers_hold_the_tiger_and_omar() {
    let block = tiger_guard::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2320, 2339));
    let named = [
        (EnemyState::LungeAttack, 2320, "LungeAttack"),
        (EnemyState::FrontFlip, 2321, "FrontFlip"),
        (EnemyState::BackFlip, 2322, "BackFlip"),
        (EnemyState::MagicAttack, 2330, "MagicAttack"),
        (EnemyState::MoveToPosition, 2331, "MoveToPosition"),
        (EnemyState::Wait, 2332, "Wait"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(
        tiger_guard::STATES,
        named[..3].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    assert_eq!(
        omar::STATES,
        named[3..].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    // The free numbers of the block stay free.
    for id in (2323..=2329).chain(2333..=2339) {
        assert_eq!(EnemyState::from_id(id), None, "{id}");
    }
    // The thirteenth level's block ends where the fourteenth's starts.
    assert_eq!(
        *enemy::redtail::STATE_BLOCK.end() + 1,
        *block.start(),
        "blocks of 20"
    );
    // The shared states they reuse keep their numbers: the tiger's knife
    // combo is the state of RedTail's knives' name.
    assert_eq!(EnemyState::KnifeAttack as i32, 2300);
    assert_eq!(EnemyState::Block as i32, 2141);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::FastAdvance as i32, 2010);
    assert_eq!(EnemyState::Startup as i32, 2022);
    assert_eq!(EnemyState::BlockLow as i32, 2034);
    assert_eq!(EnemyState::Dead as i32, 2006);
}

#[test]
fn omars_ring_turns_with_the_level_clock() {
    // Piece 0 on the first arena step of the original's runs (the level
    // clock at 1458 ms) sits at -364.5 degrees; one tick later the ring
    // has turned 4.5 degrees back.
    assert_eq!(omar::ring_angle(0, 1458), -364.5);
    assert_eq!(omar::ring_angle(0, 1476) - omar::ring_angle(0, 1458), -4.5);
    assert_eq!(omar::ring_point(0, 18, omar::HOME), (42329, 2468));
    assert_eq!(omar::RING_PIECES, 9);
}

#[test]
fn a_routes_health_set_up_reaches_claw_until_a_respawn_refills_it() {
    let route = "# level 14\n# health 90\n# checkpoint 1500,880\ntick,a,b\n0,0,0\n";
    assert_eq!(claw_sim::input::script_health(route), Ok(Some(90)));
    assert_eq!(
        claw_sim::input::script_checkpoint(route),
        Ok(Some((1500, 880)))
    );
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 2);
    sim.set_health(90);
    idle(&mut sim, 1);
    assert_eq!(sim.claw().stats.health, 90);
    assert_eq!(sim.hud().health, 90);
}

/// A flat floor with a tiger guard 150 px right of Claw, built.
fn tiger_beside_claw() -> (Sim, claw_sim::object::ObjectId) {
    let mut level = flat_level();
    level
        .objects
        .push(ObjectSpec::at(tiger_guard::NAMES[0], 1150, 900, 0));
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 3);
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == tiger_guard::NAMES[0])
        .map(|(id, _)| id)
        .expect("the tiger");
    (sim, id)
}

/// An attacker of kind `kind` with damage 5 beside the tiger.
fn attacker_of(sim: &mut Sim, kind: u32) -> claw_sim::object::ObjectId {
    let mut o = Object::new(
        0,
        1140,
        880,
        0,
        "Bullet".into(),
        claw_sim::object::LogicKind::Inert,
    );
    o.type_flags = kind;
    o.damage = 5;
    o.attack_rect = Rect::new(-5, -5, 5, 5);
    sim.spawn(o)
}

#[test]
fn a_tiger_blocks_shots_throws_and_blasts_and_takes_blows_and_magic() {
    for (kind, blocks, hurt, gone) in [
        (attacker::PISTOL_SHOT, true, false, true),
        (attacker::THROWN, true, false, false),
        (attacker::LEVEL_SHOT, true, false, false),
        (attacker::MAGIC, false, true, false),
        (attacker::CLAW, false, true, false),
    ] {
        let (mut sim, id) = tiger_beside_claw();
        let a = attacker_of(&mut sim, kind);
        sim.with_world(|w| {
            w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
            tiger_guard::on_hit(w, id, a);
        });
        let o = sim.objects().get(id).unwrap();
        let state = EnemyState::from_id(o.state);
        assert_eq!(state == Some(EnemyState::Block), blocks, "{kind:#x}");
        assert_eq!(
            o.health < tiger_guard::HEALTH,
            hurt,
            "{kind:#x}: {}",
            o.health
        );
        let shot = sim.objects().get(a).unwrap();
        assert_eq!(shot.flags & flags::DESTROY != 0, gone, "{kind:#x}");
    }
    // While it lunges or flips forward, Claw's blows do nothing.
    let (mut sim, id) = tiger_beside_claw();
    let a = attacker_of(&mut sim, attacker::CLAW);
    sim.with_world(|w| {
        tiger_guard::enter_lunge(w, id);
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        tiger_guard::on_hit(w, id, a);
    });
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.health, tiger_guard::HEALTH);
    assert_eq!(EnemyState::from_id(o.state), Some(EnemyState::LungeAttack));
    // The flips take it out of pair collision.
    let (mut sim, id) = tiger_beside_claw();
    sim.with_world(|w| tiger_guard::enter_front_flip(w, id, true));
    assert!(sim.objects().get(id).unwrap().flags & flags::NO_COLLIDE != 0);
    sim.with_world(|w| tiger_guard::go_home(w, id));
    assert!(sim.objects().get(id).unwrap().flags & flags::NO_COLLIDE == 0);
}

#[test]
fn a_tiger_is_built_with_the_shared_human_boxes_and_its_records_colour() {
    let (sim, id) = tiger_beside_claw();
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.health, tiger_guard::HEALTH);
    assert_eq!(o.type_flags, types::ENEMY);
    let d = enemy::data_of(o).unwrap();
    assert_eq!(d.ranges, tiger_guard::RANGES);
    assert!(d.may_lift);
    assert_eq!(o.image_set, tiger_guard::IMAGE_SET);
    // A white one on a record whose smarts is 1.
    let mut level = flat_level();
    let mut spec = ObjectSpec::at(tiger_guard::NAMES[0], 1150, 900, 0);
    spec.record[tiger_guard::SMARTS_FIELD] = 1;
    level.objects.push(spec);
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 3);
    let (_, o) = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == tiger_guard::NAMES[0])
        .unwrap();
    assert_eq!(o.image_set, tiger_guard::WHITE_SET);
    assert!(matches!(&o.data, ObjectData::TigerGuard(t) if t.white));
}

#[test]
fn the_fourteenth_levels_table() {
    // Platform rectangles, each with its 32 px depth.
    assert_eq!(
        platform::surface(14, SurfaceKind::Elevator),
        Rect::new(-34, -6, 23, 72)
    );
    assert_eq!(
        platform::surface(14, SurfaceKind::TogglePeg),
        Rect::new(-34, -6, 24, 72)
    );
    assert_eq!(
        platform::surface(14, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(14, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    // Springboards: the springy ferns (no springboard is placed).
    let s = springboard::level_table(14).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    // Lava death tiles that kill as goo does, the splash 50 px above.
    assert!(goo::is_goo_level(14));
    assert_eq!(goo::splash_y(14), Some(50));
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        tiger_guard::IMAGE_SET,
        tiger_guard::WHITE_SET,
        geyser::IMAGE_SET,
        mouth::IMAGE_SET,
        hand::IMAGE_SET,
        ball::IMAGE_SET,
        ball::BLAST_SET,
        omar::IMAGE_SET,
        shield::FIRE_SET,
        shield::ICE_SET,
        bullet::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        tiger_guard::KNIFE_ANIM,
        tiger_guard::LUNGE_ANIM,
        tiger_guard::FRONT_FLIP_ANIM,
        tiger_guard::BACK_FLIP_ANIM,
        tiger_guard::BLOCK_ANIM,
        tiger_guard::IDLE_ANIM,
        geyser::ANIM,
        mouth::ANIM,
        hand::ANIM,
        ball::FLIGHT_ANIM,
        ball::BLAST_ANIM,
        omar::IDLE_ANIM,
        omar::DEATH_ANIM,
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

/// Calls an animation takes to finish at 18 ms (`None` when it loops).
fn calls(level: &SimLevel, key: &str) -> Option<usize> {
    let mut p = AnimPlayer::default();
    p.set(level.animations[key].clone());
    let mut n = 0;
    while !p.is_done() {
        n += 1;
        if n > 1000 {
            return None;
        }
        p.advance_in(18, 0, false, None);
    }
    Some(n)
}

#[test]
fn install_level14_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    assert_eq!(level.number, 14);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL14 inert census: {:?}", d.inert_logics);
    assert!(d.unknown_logics.is_empty(), "{:?}", d.unknown_logics);
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 3309, "the level's records");
    assert_eq!(level.start, (1730, 3135));

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
    for name in ["OmarShield", "OmarBullet", "LavaHandProjectile"] {
        assert_eq!(placed(name), 0, "{name} is made at run time");
    }
    // The earlier slices' families the level reuses, all running.
    for (name, n) in [
        ("BehindAniCandy", 1703),
        ("GlitterlessPowerup", 354),
        ("GooCoverup", 354),
        ("FrontAniCandy", 112),
        ("CrumblingPeg", 94),
        ("BehindCrate", 92),
        ("TProjectile", 63),
        ("TreasurePowerup", 59),
        ("DoNothingNormal", 47),
        ("BehindCandy", 38),
        ("TogglePeg", 38),
        ("PathElevator", 37),
        ("FloorSpike", 35),
        ("HealthPowerup", 29),
        ("SpotAmbientSound", 18),
        ("Chameleon", 17),
        ("Elevator", 17),
        ("FloorSpike2", 17),
        ("SpecialPowerup", 15),
        ("ClawDialogSoundTrigger", 13),
        ("Checkpoint", 11),
        ("JumpSwitch", 11),
        ("AniRope", 9),
        ("FloorSpike3", 8),
        ("CursePowerup", 7),
        ("TogglePeg3", 7),
        ("GlobalAmbientSound", 6),
        ("DoNothing", 4),
        ("TogglePeg2", 4),
        ("TriggerElevator", 4),
        ("TogglePeg4", 3),
        ("MagicPowerup", 2),
        ("AmmoPowerup", 1),
        ("BossStager", 1),
        ("BossWarp", 1),
        ("FirstSuperCheckpoint", 1),
        ("OneWayTriggerElevator", 1),
        ("SecondSuperCheckpoint", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
}

#[test]
fn install_level14_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    let preloaded = level.image_sets();
    for set in [
        tiger_guard::IMAGE_SET,
        tiger_guard::WHITE_SET,
        geyser::IMAGE_SET,
        mouth::IMAGE_SET,
        hand::IMAGE_SET,
        ball::IMAGE_SET,
        ball::BLAST_SET,
        omar::IMAGE_SET,
        shield::FIRE_SET,
        shield::ICE_SET,
        bullet::IMAGE_SET,
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The tiger's first picture gives its movement box; the lava ball's
    // its box; the mouth's set runs to its 22nd picture, the fountain's
    // to its 19th.
    assert_eq!(
        level.frame_box(tiger_guard::IMAGE_SET, 1),
        Some(tiger_guard::MOVE_RECT)
    );
    assert_eq!(level.frame_box(ball::IMAGE_SET, 1), Some(ball::RECT));
    assert_eq!(level.image_frames(mouth::IMAGE_SET), Some((1, 22)));
    assert_eq!(level.image_frames(geyser::IMAGE_SET), Some((1, 19)));
    // The tiger's slash ends on its 17th call, the lunge on its 48th; the
    // hand's rise on its 68th; the fountain's eruption loops.
    assert_eq!(calls(&level, tiger_guard::KNIFE_ANIM), Some(17));
    assert_eq!(calls(&level, tiger_guard::LUNGE_ANIM), Some(48));
    assert_eq!(
        calls(&level, hand::ANIM),
        Some(hand::RISE_TICKS as usize + 1)
    );
    assert_eq!(calls(&level, geyser::ANIM), None);
    for a in [
        "HOME",
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "IDLE",
        "JUMP",
        "KILLFALL",
        "LIFTED1",
        "LIFTED2",
        "STRIKE1",
        "STRIKE3",
        "THROWN",
        "FRONTFLIP",
        "BACKFLIP",
        "BLOCK",
    ] {
        let key = format!("LEVEL_TIGER_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    for key in [
        ball::FLIGHT_ANIM,
        ball::BLAST_ANIM,
        mouth::ANIM,
        omar::IDLE_ANIM,
    ] {
        assert!(level.animations.contains_key(key), "{key} is loaded");
    }
    for s in tiger_guard::ALERT_LINES
        .iter()
        .chain(&tiger_guard::KNIFE_LINES)
        .chain(&tiger_guard::LIFTED_LINES)
        .chain(&tiger_guard::FLIP_SOUNDS)
        .chain(&[
            tiger_guard::TAUNT,
            tiger_guard::LUNGE_SOUND,
            tiger_guard::BLOCK_SOUND,
            tiger_guard::DEATH_SOUND,
            mouth::DROP_SOUND,
            mouth::SPLASH_SOUND,
            ball::BLAST_SOUND,
        ])
    {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level14_the_new_families_take_their_records() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    // 13 white tigers, 6 unbounded ones (the jump-switch room's).
    let tigers = records(&level, tiger_guard::NAMES[0]);
    let white = tigers
        .iter()
        .filter(|r| tiger_guard::is_white(r.record[tiger_guard::SMARTS_FIELD]))
        .count();
    assert_eq!(white, 13);
    let unbounded = tigers
        .iter()
        .filter(|r| r.record[enemy::field::X_MIN] == 0 && r.record[enemy::field::X_MAX] == 0)
        .count();
    assert_eq!(unbounded, 6);
    // The fountains' and mouths' delays, the hands' bursts and rests.
    let delays = |name: &str| -> BTreeSet<i32> {
        records(&level, name)
            .iter()
            .map(|r| r.record[lava::SPEED_FIELD])
            .collect()
    };
    assert_eq!(
        delays(geyser::NAMES[0]),
        BTreeSet::from([0, 500, 1000, 1500])
    );
    assert_eq!(
        delays(mouth::NAMES[0]),
        BTreeSet::from([0, 500, 1000, 1500, 2000])
    );
    let bursts: BTreeSet<u32> = records(&level, hand::NAMES[0])
        .iter()
        .map(|r| hand::burst_of(r.record[lava::SMARTS_FIELD]))
        .collect();
    assert_eq!(bursts, BTreeSet::from([1, 2, 3]));
    // Awake, each is built: the tigers at Home or deciding, health 15,
    // liftable, white or orange by the record; the hazards with their
    // kinds and boxes; Omar inert while unbuilt.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "TigerGuard" => {
                assert_eq!(o.health, tiger_guard::HEALTH);
                let white = tiger_guard::is_white(o.record.as_ref().unwrap()[17]);
                let set = if white {
                    tiger_guard::WHITE_SET
                } else {
                    tiger_guard::IMAGE_SET
                };
                assert_eq!(o.image_set, set);
                assert!(enemy::data_of(o).is_some_and(|d| d.may_lift));
                *n.entry("tigers").or_default() += 1;
            }
            "LavaGeyser" => {
                assert_eq!(o.type_flags, types::ENEMY);
                assert_eq!(o.damage, lava::DAMAGE);
                *n.entry("geysers").or_default() += 1;
            }
            "LavaMouth" => {
                assert_eq!(o.type_flags, types::ENEMY);
                *n.entry("mouths").or_default() += 1;
            }
            "LavaHand" => {
                assert!(o.flags & flags::SAFE != 0);
                assert_eq!(o.attack_type_flags, types::PLAYER);
                let r = o.record.as_ref().unwrap();
                let b = Rect::new(r[51] - o.x, r[52] - o.y, r[53] - o.x, r[54] - o.y);
                assert_eq!(o.attack_rect, b);
                *n.entry("hands").or_default() += 1;
            }
            "Omar" if !omar::BUILT => {
                assert_eq!(o.kind, LogicKind::Inert);
            }
            _ => {}
        }
    }
    assert_eq!(
        n,
        BTreeMap::from([
            ("geysers", 23),
            ("hands", 6),
            ("mouths", 19),
            ("tigers", 26)
        ])
    );
}

#[test]
fn install_level14_unwoken_tigers_and_lava_look_as_they_will_when_woken() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    // As placed, before their first call (asleep far from Claw), and once
    // built: a tiger's first look is its built colour on its floor; the
    // lava's need no first look of their own.
    let asleep = Sim::new(level.clone(), SimConfig::default());
    // Every object awake: the level's set-up builds them all.
    let built_sim = awake(level.clone());
    let names = [
        tiger_guard::NAMES[0],
        geyser::NAMES[0],
        mouth::NAMES[0],
        hand::NAMES[0],
    ];
    let mut n = 0;
    for (_, o) in asleep.objects().all() {
        if !names.contains(&&*o.logic) || o.state != 0 {
            continue;
        }
        let (_, b) = built_sim
            .objects()
            .all()
            .find(|(_, b)| b.logic == o.logic && b.editor == o.editor)
            .unwrap();
        let want = logic::Look::of(b);
        let looks = logic::first_look(&level, o).unwrap_or_else(|| vec![logic::Look::of(o)]);
        let got = &looks[0];
        assert_eq!(
            (got.image_set.as_str(), got.frame, got.x, got.y),
            (want.image_set.as_str(), want.frame, want.x, want.y),
            "{} at {:?}",
            o.logic,
            o.editor
        );
        n += 1;
    }
    assert!(n >= 60, "{n} unwoken");
}

#[test]
fn install_level14_reused_families_take_the_fourteenth_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    assert!(level.sound_ms("LEVEL_DEATHTILE").is_some());
    // The fire shooters' shots are the animated ones of their user value.
    let shooters = records(&level, "TProjectile");
    assert_eq!(shooters.len(), 63);
    assert!(
        shooters
            .iter()
            .all(|r| r.record[dart_trap::USER0_FIELD] == 1)
    );
    let sets: BTreeSet<i32> = shooters
        .iter()
        .map(|r| r.record[dart_trap::USER1_FIELD])
        .collect();
    assert_eq!(sets, BTreeSet::from([1, 2]));
    for set in dart_trap::arrow::ANIMATED_SETS {
        assert!(level.image_frames(set).is_some(), "{set}");
    }
    // Elevators and pegs take the level's surfaces.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Elevator" | "TriggerElevator" | "OneWayTriggerElevator" => {
                assert_eq!(o.hit_rect, platform::surface(14, SurfaceKind::Elevator));
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(o.hit_rect, platform::surface(14, SurfaceKind::TogglePeg));
            }
            _ => {}
        }
    }
}

#[test]
fn install_level14_the_lava_kills_and_claw_starts_again_at_the_checkpoint() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    // Put down over the lava lake with the fountains, Claw falls into the
    // lava, dies in it, and comes back at the point set as his checkpoint.
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    sim.with_world(|w| w.claw.special.warp_to = Some((23300, 2300)));
    sim.set_checkpoint(Some((22250, 2219)));
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
    assert!(died, "he dies in the lava");
    assert!(sim.claw().death_tile_hits > 0, "on its Death tiles");
    sim.step(TickInput::default());
    let c = sim.player().unwrap();
    assert_ne!(c.state, claw_state::DEATH);
    assert_eq!(c.x, 22250, "back at the checkpoint");
}

#[test]
fn install_level14_a_mouth_drips_and_holds_its_drop_through_its_delay_after_a_lost_life() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 14);
    // The three mouths at (3104, 3311), (3232, 3311) and (3360, 3311),
    // with no delay, 1000 ms and 2000 ms, kept awake; Claw stands at the
    // level's start and ends a life.
    let find = |sim: &Sim, x: i32| {
        sim.objects()
            .all()
            .find(|(_, o)| &*o.logic == mouth::NAMES[0] && o.editor.0 == x)
            .map(|(id, _)| id)
            .unwrap()
    };
    let mut sim = awake(level);
    let (first, delayed) = (find(&sim, 3104), find(&sim, 3232));
    let look = |sim: &Sim, id| {
        let o = sim.objects().get(id).unwrap();
        (o.state, o.frame, o.attack_rect)
    };
    // Dripping: pictures 2 to 22, three ticks each, the 22nd followed by
    // the first.
    let mut frames = Vec::new();
    for _ in 0..200 {
        frames.push(look(&sim, first));
        sim.step(TickInput::default());
    }
    let start = frames
        .iter()
        .position(|f| f.0 == mouth::state::FALLING)
        .unwrap();
    // Built while the level loads, it waits on its first picture with no
    // box, and starts dripping on the first tick, still on that picture.
    assert_eq!(start, 1);
    assert_eq!(frames[0].0, mouth::state::IDLE);
    assert_eq!(frames[start].1, 1);
    assert!(!frames[start].2.is_set(), "no box before the drip");
    let drip: Vec<i32> = frames[start + 1..start + 1 + 66]
        .iter()
        .map(|f| f.1)
        .collect();
    let mut want = Vec::new();
    for f in (2..=22).chain(1..=1) {
        want.extend([f; 3]);
    }
    assert_eq!(drip, want);
    for f in &frames[start + 1..start + 1 + 66] {
        if let Some(b) = mouth::drop_box(f.1) {
            assert_eq!(f.2, b);
        }
    }
    // A lost life while the delayed mouth drips: on the lives step it
    // shows its first picture and waits its 1000 ms again, its last drop's
    // box still live, then drips again.
    sim.end_life();
    let mut held = None;
    let mut waited = 0;
    for _ in 0..400 {
        let lives = sim.claw().stats.lives;
        sim.step(TickInput::default());
        if held.is_none() {
            if sim.claw().stats.lives < lives {
                held = Some(look(&sim, delayed).2);
            }
            continue;
        }
        let (state, frame, rect) = look(&sim, delayed);
        if state != mouth::state::IDLE {
            break;
        }
        assert_eq!(frame, 1);
        assert_eq!(Some(rect), held, "the drop's box is held");
        waited += 1;
    }
    let held = held.expect("a life was lost");
    assert!(held.is_set(), "caught mid-drip");
    // 1000 ms from the lives step: it drips again on the 56th tick after
    // it.
    assert_eq!(waited, 55);
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
fn install_level14_costs_at_its_start() {
    let Some(store) = store() else { return };
    // The level's start with every family it places, against the first
    // and the thirteenth levels' starts: how long a copy of the game and
    // an idle tick take, for the slice's measurements.
    for n in [1, 13, 14] {
        let (objects, clone, tick) = start_costs(&store, n);
        println!("LEVEL{n} start: {objects} objects, one copy {clone:?}, one idle tick {tick:?}");
        if n == 14 {
            assert!(objects >= 3309 - 20);
        }
    }
}
