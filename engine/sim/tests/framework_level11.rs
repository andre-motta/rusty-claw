//! The framework the eleventh level's families build on: Mercat, the
//! Siren, the Fish, Mercat's trident bolt and the Siren's scream are
//! declared, and each stays an inert placeholder until its family is
//! built; the Laser runs; the three enemies' classes, data and state
//! numbers; the scream lockout and the fish's flop toggle the families
//! share; the level's platform, springboard, plank and death tile table;
//! and, on the install, the level's inert census, the art the new
//! families need loaded with it, the lasers in their corridors, and what
//! the earlier families do with the level (the break planks' trigger
//! window and timing, the toggle pegs' cycle, the hidden belt under the
//! start room, the water covers drawn in front of Claw, the deep pools
//! that kill and the wading steps' sound).
//!
//! Every expectation that holds only while a family is not built is
//! written against that family's `BUILT` constant, so building a family
//! needs no change here. The free numbers of the eleventh level's block of
//! state numbers stay free for its families to take.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_frame::{GameEvent, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimPlayer, FRAME_SOUND_IN_VIEW, FRAME_SOUND_POSITIONAL};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::decor::SpriteAnim;
use claw_sim::logic::enemy::{self, EnemyState, fish, goo, mercat, siren};
use claw_sim::logic::platform::{self, SurfaceKind, state as peg_state};
use claw_sim::logic::projectile::{self, Class, ProjectileSpec, scream, trident};
use claw_sim::logic::{
    self, GameLogic, ObjectData, break_plank, conveyor_belt, laser, springboard, stepping_stone,
};
use claw_sim::object::{LogicKind, Object, Rect, draw_flags};
use claw_sim::physics::ptype;
use claw_sim::registry::Registry;
use claw_sim::tiles::{DEATH, SOLID};
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// The families declared for the eleventh level, with the names they
/// claim.
const DECLARED: [(GameLogic, &[&str]); 6] = [
    (GameLogic::Mercat, &["Mercat"]),
    (GameLogic::Siren, &["Siren"]),
    (GameLogic::Fish, &["Fish"]),
    (GameLogic::Laser, &["Laser"]),
    (GameLogic::TridentBullet, &["TridentBullet"]),
    (GameLogic::SirenBullet, &["SirenBullet"]),
];

/// Logics the eleventh level leaves inert besides the declared families,
/// each with the reason it may stay so: none. Every decoration (the
/// waterfalls, bubbles, water surfaces and covers, the signs, the pillars),
/// platform (the toggle pegs and elevators), break plank, crate, pickup,
/// warp, checkpoint, sound object and the hidden belt the level places
/// outside the declared families already runs.
const INERT_OTHERS: &[(&str, &str)] = &[];

/// How many records of each declared name the eleventh level places (the
/// two projectiles are thrown at run time only).
const PLACED: [(&str, usize); 4] = [("Mercat", 38), ("Siren", 13), ("Fish", 24), ("Laser", 6)];

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
    assert_eq!(logic::built(GameLogic::Mercat), mercat::BUILT);
    assert_eq!(logic::built(GameLogic::Siren), siren::BUILT);
    assert_eq!(logic::built(GameLogic::Fish), fish::BUILT);
    assert_eq!(logic::built(GameLogic::Laser), laser::BUILT);
    assert_eq!(logic::built(GameLogic::TridentBullet), trident::BUILT);
    assert_eq!(logic::built(GameLogic::SirenBullet), scream::BUILT);
    // The laser runs.
    const { assert!(laser::BUILT) };
    assert_eq!(
        Registry::shared().get("Laser").unwrap().kind,
        LogicKind::Game(GameLogic::Laser)
    );
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
    // A built family's names run.
    for (g, list) in DECLARED.iter().filter(|(g, _)| logic::built(*g)) {
        for &n in *list {
            assert_eq!(logic::runs(n), Some(LogicKind::Game(*g)), "{n}");
        }
    }
    // Placed in a level, the unbuilt enemies are drawn placeholders that
    // never collide, and the level load reports them as not implemented;
    // asleep in the side areas of a wide display they look as they are.
    let names: Vec<&str> = ["Mercat", "Siren", "Fish"]
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
}

#[test]
fn the_registry_keeps_each_names_activation_class() {
    // All six wake with the enemies' window; none keeps the original
    // window when widened.
    let cfg = ActivationConfig::widened((1706, 480));
    for (_, names) in DECLARED {
        for &name in names {
            assert_eq!(
                logic::new_object(name, 0, 0, 0).activation_class(),
                0,
                "{name}"
            );
            assert!(!cfg.is_exception(name), "{name}");
        }
    }
}

#[test]
// The data literals below end in `..Default::default()` so they keep
// compiling when the families gain fields of their own.
#[allow(clippy::needless_update)]
fn mercat_and_the_siren_are_human_enemies_and_the_fish_a_plain_one() {
    for (g, name) in [(GameLogic::Mercat, "Mercat"), (GameLogic::Siren, "Siren")] {
        let class = enemy::class(g).expect("a class");
        assert_eq!(class.name, name);
        assert!(class.human, "{name} is a human enemy");
    }
    let class = enemy::class(GameLogic::Fish).expect("a class");
    assert_eq!(class.name, "Fish");
    assert!(!class.human, "the fish is no human enemy");
    assert!(class.carry.is_none(), "the fish is never lifted");
    for g in [
        GameLogic::Laser,
        GameLogic::TridentBullet,
        GameLogic::SirenBullet,
    ] {
        assert!(enemy::class(g).is_none(), "{g:?}");
    }
    let enemy_data = enemy::EnemyData::default();
    for mut d in [
        ObjectData::Mercat(mercat::MercatData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Siren(siren::SirenData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
        ObjectData::Fish(fish::FishData {
            enemy: enemy_data.clone(),
            ..Default::default()
        }),
    ] {
        assert_eq!(d.enemy(), Some(&enemy_data));
        d.enemy_mut().unwrap().roll = 5;
        assert_eq!(d.enemy().unwrap().roll, 5);
    }
    assert_eq!(ObjectData::Laser(Default::default()).enemy(), None);
    // Their numbers.
    assert_eq!((mercat::health(11), mercat::DAMAGE), (9, 10));
    assert_eq!((siren::HEALTH, siren::DAMAGE), (9, 10));
    assert_eq!(fish::DAMAGE, 10);
    assert_eq!(mercat::BOLT_SPEED, trident::SPEED);
    assert_eq!(mercat::BOLT_DAMAGE, trident::DAMAGE);
    assert_eq!(siren::SCREAM_SPEED, scream::SPEED);
}

#[test]
fn the_bolt_and_the_scream_are_projectile_classes() {
    for (name, class, built) in [
        ("TridentBullet", Class::TridentBullet, trident::BUILT),
        ("SirenBullet", Class::SirenBullet, scream::BUILT),
    ] {
        assert_eq!(Class::from_logic(name), Some(class));
        assert_eq!(class.built(), built, "{name}");
        assert!(class.is_straight(), "{name}");
        // Thrown while its family is unbuilt, it is an inert placeholder
        // holding the request.
        let mut sim = Sim::new(flat_level(), SimConfig::default());
        idle(&mut sim, 1);
        let spec = ProjectileSpec {
            logic: name,
            x: 1200,
            y: 850,
            vx: 300,
            ..ProjectileSpec::default()
        };
        let id = sim
            .with_world(|w| projectile::spawn(w, spec))
            .expect("a registered logic");
        let o = sim.objects().get(id).expect("spawned");
        if built {
            assert_eq!(Some(o.kind), logic::kind_for(name), "{name}");
        } else {
            assert_eq!(o.kind, LogicKind::Inert, "{name}");
            let d = projectile::data(o).expect("the request");
            assert_eq!(d.spec, Some(spec));
            assert_eq!(d.class, None);
        }
    }
    // Their pictures and boxes.
    assert_eq!(Class::TridentBullet.image_set(), trident::IMAGE_SET);
    assert_eq!(Class::SirenBullet.image_set(), scream::IMAGE_SET);
    assert_eq!(
        Class::TridentBullet.frame_box(),
        Rect::new(-32, -14, 31, 13)
    );
    assert_eq!(Class::SirenBullet.frame_box(), Rect::new(-2, -5, 5, 4));
    assert_eq!(Class::SirenBullet.flight_anim(), Some(scream::ANIM));
}

#[test]
fn the_lockout_and_the_flop_toggle_are_shared_and_start_cleared() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    assert_eq!(sim.scream_lock_ms(), 0);
    sim.with_world(|w| {
        assert_eq!(w.scream_lock_ms(), 0);
        assert_eq!(w.fish_flop(), 1, "the toggle starts at 1");
    });
    // Claw walks with Right held: nothing locks his keys while no scream
    // reached him.
    let x0 = sim.player().unwrap().x;
    run(&mut sim, &script(&[(0, 54), (RIGHT, 300)]));
    assert!(sim.player().unwrap().x > x0);
    assert_eq!(sim.scream_lock_ms(), 0);
    // Both are written through the world, last across ticks and are
    // copied with the game.
    sim.with_world(|w| {
        w.set_scream_lock_ms(scream::LOCK_MS);
        w.set_fish_flop(0);
    });
    let copy = sim.clone();
    sim.with_world(|w| assert_eq!(w.fish_flop(), 0));
    assert_eq!(copy.scream_lock_ms(), scream::LOCK_MS);
    if !scream::BUILT {
        // Unbuilt, the lockout neither runs down nor holds the keys.
        let x1 = sim.player().unwrap().x;
        run(&mut sim, &script(&[(0, 54), (RIGHT, 300)]));
        assert_eq!(sim.scream_lock_ms(), scream::LOCK_MS);
        assert!(sim.player().unwrap().x > x1, "his keys are free");
    }
    sim.with_world(|w| w.set_fish_flop(1));
    sim.with_world(|w| assert_eq!(w.fish_flop(), 1));
}

#[test]
fn the_eleventh_levels_state_numbers_hold_the_fish_and_the_song() {
    let block = mercat::STATE_BLOCK;
    assert_eq!((*block.start(), *block.end()), (2260, 2279));
    let named = [
        (EnemyState::Swim, 2270, "Swim"),
        (EnemyState::JumpStrike, 2271, "JumpStrike"),
        (EnemyState::OutOfWater, 2272, "OutOfWater"),
        (EnemyState::Singing, 2273, "Singing"),
    ];
    for (s, id, name) in named {
        assert_eq!(s as i32, id, "{name}");
        assert_eq!(s.name(), name);
        assert_eq!(EnemyState::from_id(id), Some(s));
        assert!(block.contains(&id));
    }
    assert_eq!(
        fish::STATES,
        named[..3].iter().map(|n| n.0).collect::<Vec<_>>()[..]
    );
    assert_eq!(siren::STATES, [EnemyState::Singing]);
    // The tenth level's block ends where the eleventh's starts.
    assert_eq!(
        *enemy::marrow::STATE_BLOCK.end() + 1,
        *block.start(),
        "blocks of 20"
    );
    // The fish's dash keeps the crabs' number; the shared states the
    // three reach keep theirs.
    assert_eq!(fish::DASH, EnemyState::Snip);
    assert_eq!(EnemyState::Snip as i32, 2195);
    assert_eq!(EnemyState::Idle as i32, 2000);
    assert_eq!(EnemyState::Home as i32, 2001);
    assert_eq!(EnemyState::Strike as i32, 2002);
    assert_eq!(EnemyState::Fire as i32, 2013);
    assert_eq!(EnemyState::Dead as i32, 2006);
    assert_eq!(EnemyState::Block as i32, 2141);
    assert_eq!(EnemyState::JumpBack as i32, 2030);
    assert_eq!(EnemyState::HumanIdle as i32, 2035);
    // Mercat needs no new number.
    for s in mercat::STATES {
        assert!(!block.contains(&(s as i32)), "{}", s.name());
    }
    // The laser keeps states of its own.
    for s in [laser::state::SET_UP, laser::state::IDLE] {
        assert_eq!(EnemyState::from_id(s), None, "{s}");
    }
}

#[test]
fn the_eleventh_levels_table() {
    // Platform rectangles, each with its 32 px depth: the elevators and
    // the toggle pegs share one.
    assert_eq!(
        platform::surface(11, SurfaceKind::Elevator),
        Rect::new(-32, -6, 30, 72)
    );
    assert_eq!(
        platform::surface(11, SurfaceKind::TogglePeg),
        Rect::new(-32, -6, 30, 72)
    );
    assert_eq!(
        platform::surface(11, SurfaceKind::CrumblingPeg),
        Rect::new(-24, -46, 24, 64)
    );
    assert_eq!(
        platform::surface(11, SurfaceKind::SteppingStone),
        Rect::new(-16, -1, 16, 64)
    );
    assert_eq!(stepping_stone::surface(11), Rect::new(-16, -1, 16, 64));
    assert_eq!(
        platform::surface(11, SurfaceKind::BigElevator),
        Rect::new(0, 0, 0, 32)
    );
    // Springboards: the fern, which no record places.
    let s = springboard::level_table(11).expect("ferns");
    assert_eq!(s.rect, (-32, 10, 24, 32));
    assert_eq!(
        (s.idle_anim, s.spring_anim),
        ("LEVEL_SPRINGYFERN_IDLE", "LEVEL_SPRINGYFERN_SPRING")
    );
    // Break planks: 64 px apart, the floor cell 66 px above a piece.
    let p = break_plank::level_table(11).expect("planks");
    assert_eq!((p.spacing, p.row_offset), (64, -66));
    // Spike death tiles, no goo and no splash.
    assert!(!goo::is_goo_level(11));
    assert_eq!(goo::splash_y(11), None);
    // Mercat's eleventh-level numbers.
    assert_eq!(mercat::health(11), 9);
    assert_eq!(mercat::bolt_cooldown_ms(11), 1000);
}

#[test]
fn a_piece_triggers_for_claw_within_its_thirty_px() {
    // The eleventh level's piece box and a standing Claw's: his x from
    // X - 16 to X + 13 starts a piece, with his feet inside its band.
    let piece = Rect::new(-33, -87, 30, 86);
    let claw = Rect::new(-16, -52, 16, 60);
    let (x, y) = (5215, 1494);
    let feet_y = y - 73 - claw.bottom;
    let triggers =
        |cx: i32| break_plank::claw_triggers((x, y), piece, (cx, feet_y), claw, ptype::STAND);
    let window: Vec<i32> = (x - 60..x + 60).filter(|&cx| triggers(cx)).collect();
    assert_eq!(
        (window.first(), window.last(), window.len()),
        (Some(&(x - 16)), Some(&(x + 13)), 30)
    );
    // 34 px between two pieces start nothing.
    assert_eq!(64 - 30, 34);
    // Walking counts, falling does not.
    assert!(break_plank::claw_triggers(
        (x, y),
        piece,
        (x, feet_y),
        claw,
        ptype::WALK
    ));
    assert!(!break_plank::claw_triggers(
        (x, y),
        piece,
        (x, feet_y),
        claw,
        ptype::FALL
    ));
    // The fifth level's timing: a 250 ms delay waits 14 ticks.
    assert_eq!(break_plank::wait_ticks(250, 18), 14);
}

#[test]
fn the_families_art_is_listed_for_loading() {
    let sets = logic::runtime_image_sets();
    for set in [
        mercat::IMAGE_SET,
        siren::IMAGE_SET,
        fish::IMAGE_SET,
        laser::IMAGE_SET,
        trident::IMAGE_SET,
        trident::EXPLOSION_SET,
        scream::IMAGE_SET,
    ] {
        assert!(sets.contains(&set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
    let prefixes = logic::runtime_anim_prefixes();
    for anim in [
        mercat::HOME_ANIM,
        mercat::STRIKE_ANIM,
        mercat::BLOCK_ANIM,
        siren::IDLE_ANIM,
        siren::SONG_ANIM,
        fish::SWIM_ANIM,
        fish::STRIKE_ANIM,
        fish::FLOP_ANIM,
        laser::ANIM,
        trident::EXPLOSION_ANIM,
        scream::ANIM,
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
fn install_level11_loads_and_leaves_only_the_declared_families_inert() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    assert_eq!(level.number, 11);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..5 {
        sim.step(TickInput::default());
    }
    let d = sim.diagnostics();
    println!("LEVEL11 inert census: {:?}", d.inert_logics);
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert_eq!(d.dropped_records, 0);
    assert!(sim.player().is_some(), "Claw is placed");
    assert_eq!(level.objects.len(), 3203, "the level's records");
    assert_eq!(level.start, (3393, 1088));

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
        ("BehindAniCandy", 1359),
        ("GlitterlessPowerup", 634),
        ("AniCycle", 594),
        ("TreasurePowerup", 113),
        ("BehindCandy", 84),
        ("SmallSoundTrigger", 76),
        ("TogglePeg", 36),
        ("DoNothingNormal", 26),
        ("HealthPowerup", 24),
        ("SpecialPowerup", 20),
        ("AmbientSound", 19),
        ("TogglePeg3", 19),
        ("TogglePeg2", 17),
        ("TogglePeg4", 15),
        ("Checkpoint", 14),
        ("BehindCrate", 12),
        (break_plank::SPAWNER, 12),
        ("ClawDialogSoundTrigger", 12),
        ("Elevator", 11),
        ("CursePowerup", 6),
        ("FrontCrate", 6),
        ("AmmoPowerup", 2),
        ("MagicPowerup", 2),
        ("OneWayStartElevator", 2),
        ("TriggerElevator", 2),
        ("ConveyorBelt", 1),
        ("EndOfLevelPowerup", 1),
        ("FirstSuperCheckpoint", 1),
        ("SecondSuperCheckpoint", 1),
        ("GlobalAmbientSound", 1),
    ] {
        assert_eq!(placed(name), n, "{name}");
        assert!(logic::runs(name).is_some(), "{name} runs");
    }
    // None of the earlier levels' enemies, bosses or hazards: the map
    // piece ends the level, with no boss.
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
        "BossStager",
        "BossWarp",
        "PowderKeg",
        "TowerCannonLeft",
        "TowerCannonRight",
        "SkullCannon",
        "TProjectile",
        "FloorSpike",
        "SawBlade",
        "SteppingStone",
        "SpringBoard",
        "CrumblingPeg",
        "JumpSwitch",
        "GooCoverup",
        "GooVent",
        "GroundBlower",
        "PathElevator",
        "AniRope",
    ] {
        assert_eq!(placed(name), 0, "{name}");
    }
}

#[test]
fn install_level11_loads_its_families_art_with_the_level() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    let preloaded = level.image_sets();
    for set in [
        mercat::IMAGE_SET,
        siren::IMAGE_SET,
        fish::IMAGE_SET,
        laser::IMAGE_SET,
        trident::IMAGE_SET,
        trident::EXPLOSION_SET,
        scream::IMAGE_SET,
    ] {
        assert!(preloaded.iter().any(|s| s == set), "{set} is preloaded");
        let (first, _) = level.image_frames(set).expect("registered");
        assert!(
            level.frame_box(set, first).is_some(),
            "{set} frame {first} has a box"
        );
    }
    // The first pictures' boxes the families take.
    assert_eq!(
        level.frame_box(mercat::IMAGE_SET, 1),
        Some(mercat::MOVE_RECT)
    );
    assert_eq!(level.frame_box(siren::IMAGE_SET, 1), Some(siren::MOVE_RECT));
    assert_eq!(level.frame_box(fish::IMAGE_SET, 1), Some(fish::RECT));
    assert_eq!(
        level.frame_box(trident::IMAGE_SET, 1),
        Some(trident::FRAME_BOX)
    );
    assert_eq!(level.image_frames(scream::IMAGE_SET), Some((1, 8)));
    assert_eq!(
        level.frame_box(scream::IMAGE_SET, 1),
        Some(scream::FRAME_BOX)
    );
    // The laser's 25 pictures of 116 x 275 px.
    assert_eq!(level.image_frames(laser::IMAGE_SET), Some((1, 25)));
    for frame in [1, 25] {
        assert_eq!(
            level.frame_box(laser::IMAGE_SET, frame),
            Some(Rect::new(-58, -137, 57, 136)),
            "laser {frame}"
        );
    }

    // The laser's pass: 76 calls, the zaps on its pictures 5 to 21.
    assert_eq!(calls(&level, laser::ANIM).0 as u32, laser::PASS_TICKS);
    let pass = &level.animations[laser::ANIM];
    let zaps: Vec<(usize, String)> = (0..pass.frames.len())
        .filter_map(|k| {
            let s = pass.sounds_of(k).first()?.clone()?;
            Some((k, s))
        })
        .collect();
    let want: Vec<(usize, String)> = (4..=20)
        .step_by(2)
        .map(|k| {
            let z = if k % 4 == 0 { laser::ZAP1 } else { laser::ZAP2 };
            (k, z.to_string())
        })
        .collect();
    assert_eq!(zaps, want);
    // Not positional, and played whether the laser is on screen or not.
    for k in 0..pass.frames.len() {
        let f = pass.frames[k].flags;
        assert_eq!(
            f & (FRAME_SOUND_POSITIONAL | FRAME_SOUND_IN_VIEW),
            0,
            "element {k}"
        );
    }
    for s in [laser::ZAP1, laser::ZAP2] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }

    // Mercat's: the throw's event on the 10th call, done on the 19th; the
    // parry done on the 4th, the hop on the 18th, the flinches on the
    // 19th; his walks loop.
    for a in [
        "ADVANCE",
        "FASTADVANCE",
        "HITHIGH",
        "HITLOW",
        "IDLE1",
        "IDLE2",
        "IDLE3",
        "IDLE4",
        "JUMP",
        "KILLFALL",
        "LIFTED1",
        "LIFTED2",
        "STRIKE1",
        "THROWN",
        "HOME",
        "BLOCK",
    ] {
        let key = format!("LEVEL_MERCAT_{a}");
        assert!(level.animations.contains_key(&key), "{key} is loaded");
    }
    let (n, events) = calls(&level, mercat::STRIKE_ANIM);
    assert_eq!(
        (n, events.first().copied()),
        (19, Some(mercat::BOLT_TICK as usize))
    );
    assert_eq!(calls(&level, mercat::BLOCK_ANIM).0, 4);
    assert_eq!(calls(&level, mercat::JUMP_ANIM).0, 18);
    for a in ["LEVEL_MERCAT_HITHIGH", "LEVEL_MERCAT_HITLOW"] {
        assert_eq!(calls(&level, a).0, 19, "{a}");
    }
    assert!(loops(&level, "LEVEL_MERCAT_ADVANCE"));
    for s in [mercat::BOLT_SOUND, mercat::STAB_SOUND]
        .iter()
        .chain(&mercat::BLOCK_SOUNDS)
    {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }

    // The Siren's: the song done on the 73rd call, her idle a loop; her
    // scream's flight done on the 85th.
    assert_eq!(calls(&level, siren::SONG_ANIM).0, 73);
    assert!(loops(&level, siren::IDLE_ANIM));
    // The scream steps through its own eight pictures: the 85th call ends
    // its flight.
    let mut flight = SpriteAnim::default();
    flight.set(level.animations.get(scream::ANIM).cloned());
    let mut o = Object::new(0, 0, 0, 0, Arc::from(scream::NAMES[0]), LogicKind::Inert);
    o.frame = 1;
    let mut n = 0;
    while !flight.is_done() {
        n += 1;
        assert!(n < 1000, "the scream never ends");
        flight.advance(18, &mut o, (1, 8));
    }
    assert_eq!(n, scream::LIFE_TICKS);
    assert_eq!(o.frame, 8);
    assert_eq!(level.sound_ms(siren::CALL_SOUND), Some(siren::SONG_MS));

    // The Fish's: the strike done on the 48th call; the swim and the flop
    // loop.
    assert_eq!(calls(&level, fish::STRIKE_ANIM).0, 48);
    assert!(loops(&level, fish::SWIM_ANIM));
    assert!(loops(&level, fish::FLOP_ANIM));
    for s in [
        fish::CHOMP_SOUND,
        fish::OUT_SOUND,
        fish::IN_SOUND,
        fish::FLOP_SOUND,
        fish::DEATH_SOUND,
    ] {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // The bolt's explosion.
    assert!(level.animations.contains_key(trident::EXPLOSION_ANIM));
    assert!(
        level.skipped_animations.is_empty(),
        "{:?}",
        level.skipped_animations
    );
}

#[test]
fn install_level11_lasers_stand_in_their_corridors() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    let mut lasers: Vec<(i32, i32, i32)> = records(&level, "Laser")
        .into_iter()
        .map(|r| {
            assert_eq!(r.image_set, laser::IMAGE_SET);
            assert!(r.anim.is_empty());
            assert_eq!(r.record[laser::WAIT_FIELD], 1500);
            assert_eq!(r.record[16], 0, "no damage of its own");
            (r.record[5], r.record[6], r.record[7])
        })
        .collect();
    lasers.sort();
    assert_eq!(
        lasers,
        [
            (5795, 1311, 0),
            (8675, 2719, 1000),
            (12516, 3104, 5000),
            (13475, 1119, 0),
            (15459, 4191, 0),
            (16035, 927, 0),
        ]
    );
    // Each spans a corridor 204 px tall, open from 95 or 96 px above it to
    // 107 or 108 px below it at both edges of its box: the box reaches
    // from the ceiling to 8 or 9 px above the floor, so no Claw in the
    // corridor stays out of it.
    let map = &level.map;
    for &(x, y, _) in &lasers {
        for bx in [x + laser::ATTACK_RECT.left, x + laser::ATTACK_RECT.right] {
            let ceiling = (y - 200..y)
                .rev()
                .find(|&cy| map.attr(bx, cy) == SOLID)
                .unwrap();
            let floor = (y..y + 200).find(|&fy| map.attr(bx, fy) == SOLID).unwrap();
            assert_eq!(floor - ceiling - 1, 204, "{x},{y}");
            assert!((96..=97).contains(&(y - ceiling)), "{x},{y}: ceiling");
            assert!(y + laser::ATTACK_RECT.top <= ceiling + 1, "{x},{y}");
            assert!(
                (8..=9).contains(&(floor - (y + laser::ATTACK_RECT.bottom))),
                "{x},{y}"
            );
        }
    }
    // Awake, each is set up with its wait and drawn at its record's depth.
    let mut sim = awake(level);
    idle(&mut sim, 2);
    let mut zs: Vec<i32> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "Laser")
        .map(|(_, o)| {
            let d = laser::data_of(o).expect("set up");
            assert_eq!(d.wait_ms, 1500);
            assert_eq!(o.type_flags, laser::TYPE);
            assert_eq!(o.frame, laser::REST_FRAME);
            o.z
        })
        .collect();
    zs.sort();
    assert_eq!(zs, [0, 0, 0, 0, 1000, 5000]);
}

#[test]
fn install_level11_reused_families_take_the_eleventh_levels_data() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    let at = |r: &ObjectSpec| (r.record[5], r.record[6]);

    // The break planks: 12 rows of 29 pieces at depth 0, ten waiting 500
    // ms, one 250 and one 750; the piece picture 64 x 174 px, the floor
    // cell of each piece Solid from 12 px below its top.
    let rows = records(&level, break_plank::SPAWNER);
    assert_eq!(rows.len(), 12);
    let widths: usize = rows
        .iter()
        .map(|r| r.record[break_plank::WIDTH_FIELD].max(1) as usize)
        .sum();
    assert_eq!(widths, 29);
    assert_eq!(
        tally(&level, break_plank::SPAWNER, |r| r.record
            [break_plank::DELAY_FIELD]),
        BTreeMap::from([(250, 1), (500, 10), (750, 1)])
    );
    assert_eq!(
        level.frame_box(break_plank::IMAGE_SET, 1),
        Some(Rect::new(-33, -87, 30, 86))
    );
    let planks = break_plank::level_table(11).unwrap();
    let tile = level.map.tile_w();
    for r in &rows {
        let (x, y) = at(r);
        let (cx, cy) = break_plank::floor_cell(planks, x, y, tile);
        let top = cy * tile;
        assert_eq!(level.map.attr(cx * tile + 32, top + 12), SOLID, "{x},{y}");
        assert_ne!(level.map.attr(cx * tile + 32, top + 11), SOLID, "{x},{y}");
    }
    assert!(level.sound_ms("LEVEL_CRUMBLEBRIDGE4").is_some());

    // The toggle pegs: the five-picture slider, the level's rectangle.
    for name in ["TogglePeg", "TogglePeg2", "TogglePeg3", "TogglePeg4"] {
        assert!(
            records(&level, name)
                .iter()
                .all(|r| r.image_set == "LEVEL_PEGSLIDER"),
            "{name}"
        );
    }
    assert_eq!(level.image_frames("LEVEL_PEGSLIDER"), Some((1, 5)));

    // The hidden belt under the start room: one piece, never drawn,
    // pushing right at the belts' limit.
    let belt = records(&level, "ConveyorBelt");
    assert_eq!(belt.len(), 1);
    let b = belt[0];
    assert_eq!((at(b), b.record[7]), ((3613, 1486), 1000));
    assert_eq!(b.image_set, "LEVEL_ARROWDOWNRIGHT");
    assert_eq!(b.record[11] as u32 & draw_flags::HIDDEN, draw_flags::HIDDEN);
    assert_eq!(b.record[conveyor_belt::SPEED_FIELD], 1000);
    assert_eq!(conveyor_belt::clamp_speed(1000), conveyor_belt::SPEED_LIMIT);

    // The water: the covers at depth 5000, in front of Claw, the surfaces
    // and signs at 1000, every picture a single one.
    let anicycles = tally(&level, "AniCycle", |r| (r.image_set.clone(), r.record[7]));
    let covers: usize = anicycles
        .iter()
        .filter(|((s, _), _)| s.starts_with("LEVEL_WATERCOVER"))
        .map(|((_, z), n)| {
            assert_eq!(*z, 5000);
            n
        })
        .sum();
    assert_eq!(covers, 295);
    for (set, _) in anicycles.keys() {
        assert_eq!(
            level.image_frames(set).map(|(a, b)| b - a),
            Some(0),
            "{set}"
        );
    }
    // The wading steps' and puddles' sounds.
    let steps = tally(&level, "SmallSoundTrigger", |r| r.anim.clone());
    assert_eq!(
        steps,
        BTreeMap::from([
            ("LEVEL_TRIGGER_PUDDLE1".to_string(), 6),
            ("LEVEL_TRIGGER_PUDDLE2".to_string(), 7),
            ("LEVEL_TRIGGER_WATERSTEP".to_string(), 63),
        ])
    );
    for s in steps.keys() {
        assert!(level.sound_ms(s).is_some(), "{s}");
    }
    // The deep pools: Death 95 px under the surface of the first.
    for x in [6912, 7500, 8959] {
        assert_eq!(level.map.attr(x, 1792), DEATH, "{x}");
        assert_ne!(level.map.attr(x, 1791), DEATH, "{x}");
    }
    assert!(level.sound_ms("LEVEL_DEATHTILE").is_some());

    // The awake level: the pegs and elevators take the level's surface,
    // the belt is set up hidden and the covers keep their depth, drawn
    // after Claw.
    let mut sim = awake(level.clone());
    idle(&mut sim, 2);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (_, o) in sim.objects().all() {
        match &*o.logic {
            "Elevator" | "OneWayStartElevator" | "TriggerElevator" => {
                assert_eq!(o.hit_rect, platform::surface(11, SurfaceKind::Elevator));
                *kinds.entry(o.logic.to_string()).or_default() += 1;
            }
            "TogglePeg" | "TogglePeg2" | "TogglePeg3" | "TogglePeg4" => {
                assert_eq!(o.hit_rect, platform::surface(11, SurfaceKind::TogglePeg));
                *kinds.entry("pegs".into()).or_default() += 1;
            }
            "ConveyorBelt" => {
                assert!(matches!(o.data, ObjectData::ConveyorBelt(_)), "set up");
                assert_eq!(o.draw_flags & draw_flags::HIDDEN, draw_flags::HIDDEN);
                *kinds.entry("belt".into()).or_default() += 1;
            }
            "AniCycle" if o.image_set.starts_with("LEVEL_WATERCOVER") => {
                assert_eq!(o.z, 5000);
                *kinds.entry("covers".into()).or_default() += 1;
            }
            _ => {}
        }
    }
    assert_eq!(
        kinds,
        BTreeMap::from([
            ("Elevator".to_string(), 11),
            ("OneWayStartElevator".to_string(), 2),
            ("TriggerElevator".to_string(), 2),
            ("belt".to_string(), 1),
            ("covers".to_string(), 295),
            ("pegs".to_string(), 87),
        ])
    );
    assert!(sim.player().unwrap().z < 5000, "the covers draw over Claw");
}

#[test]
fn install_level11_toggle_peg_14_cycles_with_the_original() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    let mut sim = awake(level);
    let peg = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "TogglePeg" && (o.x, o.y) == (4192, 1764))
        .map(|(id, _)| id)
        .expect("toggle peg 14");
    let mut rows = Vec::new();
    for _ in 0..500 {
        sim.step(TickInput::default());
        let o = sim.objects().get(peg).unwrap();
        rows.push((o.state, o.frame));
    }
    // The ticks (from the level's start) its slide into the wall starts:
    // where the original's run shows them, on the level clock.
    let starts: Vec<usize> = (1..rows.len())
        .filter(|&k| rows[k].0 == peg_state::START_IN && rows[k - 1].0 != peg_state::START_IN)
        .collect();
    assert_eq!(starts, [83, 250, 416]);
    // One cycle: 1 tick, the slide in over its five pictures (13 ticks,
    // one picture every 3), 69 ticks in the wall, 1 tick, the slide out
    // (14 ticks), 69 ticks out: 167 ticks.
    let c = starts[0];
    let seq = |from: usize, to: usize| -> Vec<(i32, i32)> { rows[from..=to].to_vec() };
    assert_eq!(seq(c, c), [(peg_state::START_IN, 1)]);
    let sliding_in: Vec<i32> = seq(c + 1, c + 13).iter().map(|r| r.1).collect();
    assert_eq!(sliding_in, [1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5]);
    assert!(seq(c + 14, c + 82).iter().all(|&r| r == (peg_state::IN, 5)));
    assert_eq!(seq(c + 83, c + 83), [(peg_state::START, 5)]);
    let sliding_out: Vec<i32> = seq(c + 84, c + 97).iter().map(|r| r.1).collect();
    assert_eq!(sliding_out, [5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 1, 1, 1]);
    assert!(
        seq(c + 98, c + 166)
            .iter()
            .all(|&r| r == (peg_state::OUT, 1))
    );
}

#[test]
fn install_level11_a_deep_pool_kills_as_spikes_do() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    // Put down over the first deep pool, Claw falls through the water
    // onto its Death tiles.
    sim.with_world(|w| w.claw.special.warp_to = Some((7500, 1650)));
    let mut died = None;
    for k in 0..200 {
        sim.step(TickInput::default());
        if sim
            .last_events()
            .iter()
            .any(|e| matches!(e, GameEvent::ClawDied { .. }))
        {
            died = Some(k);
            break;
        }
    }
    assert!(died.is_some(), "he dies in the pool");
    assert!(sim.claw().death_tile_hits > 0, "on its Death tiles");
}

#[test]
fn install_level11_costs_at_its_start() {
    let Some(store) = store() else { return };
    let level = level_n(&store, 11);
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
        "LEVEL11 start: {} objects, one copy {clone:?}, one idle tick {tick:?}",
        sim.objects().all().count()
    );
    // A few records leave or turn into other objects on their first
    // calls (sound objects that wake with Claw, the plank rows).
    assert!(sim.objects().all().count() >= 3203 - 20);
}
