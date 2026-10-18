//! Level logics: what enemies, shots, effects, checkpoints, platforms,
//! decoration, crates, kegs, pickups and sound objects do.
//!
//! Each family lives in its own file and declares:
//!
//! - `NAMES`: the logic names it runs;
//! - `IMAGE_SETS` and `ANIM_PREFIXES`: the image sets it may switch an
//!   object to and the animations it may play, so the level loads them up
//!   front and nothing is read or decoded while drawing;
//! - `tick(w, id)`: one logic call, made once per update pass; the engine
//!   also calls it with the state set to [`engine_state::DESTROYING`]
//!   before destroying an object flagged [`flags::NOTIFY_DESTROY`], and
//!   with [`engine_state::SLEEPING`] when an object flagged
//!   [`flags::NOTIFY_SLEEP`] goes dormant; an object whose health ran out
//!   through automatic hit damage, or that another logic killed with
//!   [`World::kill`], sees [`engine_state::KILLED`] on its next call (with
//!   the kind of kill in [`Object::kill_kind`]);
//! - `on_hit(w, id, attacker)` and `on_attack(w, id, victim)`: called by
//!   the pair-collision pass, in the order the pairs are tested, for
//!   objects with [`Object::hit_handler`] or [`Object::attack_handler`];
//! - its own data type, held in [`Object::data`] through [`ObjectData`].
//!
//! [`kind_for`] maps a logic name to its family. A family whose logics are
//! not written yet declares `BUILT = false` ([`built`]): the registry binds
//! a name only to a built family ([`runs`]), so until then its names stay
//! inert placeholders as unclaimed names do (drawn, no collision, and
//! reported by the level load as not implemented). [`tick`], [`on_hit`],
//! [`on_attack`] and the lift hooks dispatch a call to the object's
//! family.
//!
//! [`World`] is the context of every call. Besides the objects and Claw
//! it gives the level's number, Claw's attempt counter, the frame ranges
//! and frame boxes of the image sets, the screen shake request, the
//! sound-only random stream and the boss fight state ([`world_boss`]). A
//! family adds helpers of its own in an `impl World` block in its own
//! file.
//!
//! Families that need another family's objects go through the spawn
//! functions [`projectile::spawn`], [`effect::spawn`] and
//! [`pickup::spawn_drop`]. Until the owning family fills them in, they
//! spawn an inert placeholder whose data records the request.
//!
//! [`engine_state::DESTROYING`]: crate::object::engine_state::DESTROYING
//! [`engine_state::SLEEPING`]: crate::object::engine_state::SLEEPING
//! [`engine_state::KILLED`]: crate::object::engine_state::KILLED
//! [`flags::NOTIFY_DESTROY`]: crate::object::flags::NOTIFY_DESTROY
//! [`flags::NOTIFY_SLEEP`]: crate::object::flags::NOTIFY_SLEEP

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::object::{LogicKind, Object, ObjectId, flags};
use crate::registry::{REG_ALWAYS, REG_CLASS_1, REG_CLASS_2, Registry};

pub mod boss_stage;
pub mod cannon;
pub mod checkpoint;
pub mod crates;
pub mod decor;
pub mod effect;
pub mod enemy;
pub mod floor_spike;
pub mod keg;
pub mod path_elevator;
pub mod pickup;
pub mod platform;
pub mod projectile;
pub mod rope;
pub mod sound;
pub mod sound_object;
pub mod springboard;
pub mod stepping_stone;
pub mod world;
pub mod world_boss;

pub use world::{Outbox, World};
pub use world_boss::BossState;

/// The level logic families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameLogic {
    /// The officer enemy.
    Officer,
    /// The soldier enemy.
    Soldier,
    /// The rat enemy.
    Rat,
    /// Shots, thrown dynamite and other flying objects.
    Projectile,
    /// Hit bursts, splashes and explosions.
    Effect,
    /// Checkpoint flags and save points.
    Checkpoint,
    /// Elevators and pegs.
    Platform,
    /// Decoration.
    Decor,
    /// Crates.
    Crate,
    /// Powder kegs.
    Keg,
    /// Pickups.
    Pickup,
    /// Sound triggers and ambient sounds.
    SoundObject,
    /// The punk rat, the rat that rides a small cannon.
    PunkRat,
    /// Cannons: the tower cannons and the punk rat's ground cannon.
    Cannon,
    /// Raux, the boss of the second level.
    Raux,
    /// The boss fight's stage: the warp into the boss's room, the
    /// introduction and the boss's health bar.
    BossStage,
    /// The cutthroat, the knife-throwing thief.
    CutThroat,
    /// The robber thief, the archer.
    RobberThief,
    /// Floor spikes that rise and sink on the level clock.
    FloorSpike,
    /// Platforms that follow a closed route on the level clock.
    PathElevator,
    /// Swinging ropes.
    Rope,
    /// Springboards that throw Claw or an enemy into the air.
    SpringBoard,
    /// Stepping stones that sink on the level clock or under Claw.
    SteppingStone,
    /// Katherine, the boss of the fourth level.
    Katherine,
}

/// Every family, in a fixed order.
pub const ALL: [GameLogic; 24] = [
    GameLogic::Officer,
    GameLogic::Soldier,
    GameLogic::Rat,
    GameLogic::Projectile,
    GameLogic::Effect,
    GameLogic::Checkpoint,
    GameLogic::Platform,
    GameLogic::Decor,
    GameLogic::Crate,
    GameLogic::Keg,
    GameLogic::Pickup,
    GameLogic::SoundObject,
    GameLogic::PunkRat,
    GameLogic::Cannon,
    GameLogic::Raux,
    GameLogic::BossStage,
    GameLogic::CutThroat,
    GameLogic::RobberThief,
    GameLogic::FloorSpike,
    GameLogic::PathElevator,
    GameLogic::Rope,
    GameLogic::SpringBoard,
    GameLogic::SteppingStone,
    GameLogic::Katherine,
];

/// A logic's own data, by family.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ObjectData {
    /// No data.
    #[default]
    None,
    /// An enemy.
    Enemy(enemy::EnemyData),
    /// A projectile.
    Projectile(projectile::ProjectileData),
    /// An effect.
    Effect(effect::EffectData),
    /// A checkpoint.
    Checkpoint(checkpoint::CheckpointData),
    /// A platform.
    Platform(platform::PlatformData),
    /// Decoration.
    Decor(decor::DecorData),
    /// A crate.
    Crate(crates::CrateData),
    /// A powder keg.
    Keg(keg::KegData),
    /// A pickup.
    Pickup(pickup::PickupData),
    /// A sound object.
    Sound(sound_object::SoundData),
    /// A punk rat.
    PunkRat(enemy::punkrat::PunkRatData),
    /// A cannon.
    Cannon(cannon::CannonData),
    /// Raux.
    Raux(enemy::raux::RauxData),
    /// A piece of a boss fight's stage.
    BossStage(boss_stage::BossStageData),
    /// A cutthroat.
    CutThroat(enemy::cutthroat::CutThroatData),
    /// A robber thief.
    RobberThief(enemy::robberthief::RobberThiefData),
    /// A floor spike.
    FloorSpike(floor_spike::FloorSpikeData),
    /// A path elevator.
    PathElevator(path_elevator::PathElevatorData),
    /// A rope.
    Rope(rope::RopeData),
    /// A springboard.
    SpringBoard(springboard::SpringBoardData),
    /// A stepping stone.
    SteppingStone(stepping_stone::SteppingStoneData),
    /// Katherine.
    Katherine(enemy::katherine::KatherineData),
}

/// Calls `$f($args)` in the module of family `$g`.
macro_rules! dispatch {
    ($g:expr, $f:ident ( $($arg:expr),* )) => {
        match $g {
            GameLogic::Officer => enemy::officer::$f($($arg),*),
            GameLogic::Soldier => enemy::soldier::$f($($arg),*),
            GameLogic::Rat => enemy::rat::$f($($arg),*),
            GameLogic::Projectile => projectile::$f($($arg),*),
            GameLogic::Effect => effect::$f($($arg),*),
            GameLogic::Checkpoint => checkpoint::$f($($arg),*),
            GameLogic::Platform => platform::$f($($arg),*),
            GameLogic::Decor => decor::$f($($arg),*),
            GameLogic::Crate => crates::$f($($arg),*),
            GameLogic::Keg => keg::$f($($arg),*),
            GameLogic::Pickup => pickup::$f($($arg),*),
            GameLogic::SoundObject => sound_object::$f($($arg),*),
            GameLogic::PunkRat => enemy::punkrat::$f($($arg),*),
            GameLogic::Cannon => cannon::$f($($arg),*),
            GameLogic::Raux => enemy::raux::$f($($arg),*),
            GameLogic::BossStage => boss_stage::$f($($arg),*),
            GameLogic::CutThroat => enemy::cutthroat::$f($($arg),*),
            GameLogic::RobberThief => enemy::robberthief::$f($($arg),*),
            GameLogic::FloorSpike => floor_spike::$f($($arg),*),
            GameLogic::PathElevator => path_elevator::$f($($arg),*),
            GameLogic::Rope => rope::$f($($arg),*),
            GameLogic::SpringBoard => springboard::$f($($arg),*),
            GameLogic::SteppingStone => stepping_stone::$f($($arg),*),
            GameLogic::Katherine => enemy::katherine::$f($($arg),*),
        }
    };
}

/// The constants `$c` of family `$g`.
macro_rules! constant {
    ($g:expr, $c:ident) => {
        match $g {
            GameLogic::Officer => enemy::officer::$c,
            GameLogic::Soldier => enemy::soldier::$c,
            GameLogic::Rat => enemy::rat::$c,
            GameLogic::Projectile => projectile::$c,
            GameLogic::Effect => effect::$c,
            GameLogic::Checkpoint => checkpoint::$c,
            GameLogic::Platform => platform::$c,
            GameLogic::Decor => decor::$c,
            GameLogic::Crate => crates::$c,
            GameLogic::Keg => keg::$c,
            GameLogic::Pickup => pickup::$c,
            GameLogic::SoundObject => sound_object::$c,
            GameLogic::PunkRat => enemy::punkrat::$c,
            GameLogic::Cannon => cannon::$c,
            GameLogic::Raux => enemy::raux::$c,
            GameLogic::BossStage => boss_stage::$c,
            GameLogic::CutThroat => enemy::cutthroat::$c,
            GameLogic::RobberThief => enemy::robberthief::$c,
            GameLogic::FloorSpike => floor_spike::$c,
            GameLogic::PathElevator => path_elevator::$c,
            GameLogic::Rope => rope::$c,
            GameLogic::SpringBoard => springboard::$c,
            GameLogic::SteppingStone => stepping_stone::$c,
            GameLogic::Katherine => enemy::katherine::$c,
        }
    };
}

/// The logic names family `g` runs.
pub fn names(g: GameLogic) -> &'static [&'static str] {
    constant!(g, NAMES)
}

/// The family that claims `name`, or `None` when no family does. Names
/// are case-sensitive.
pub fn kind_for(name: &str) -> Option<LogicKind> {
    ALL.iter()
        .find(|&&g| names(g).contains(&name))
        .map(|&g| LogicKind::Game(g))
}

/// Whether family `g` runs its logics yet. A family whose logics are not
/// written yet says so with `BUILT = false` in its own file; its objects
/// stay inert placeholders until it does.
pub fn built(g: GameLogic) -> bool {
    match g {
        GameLogic::PunkRat => enemy::punkrat::BUILT,
        GameLogic::Cannon => cannon::BUILT,
        GameLogic::Raux => enemy::raux::BUILT,
        GameLogic::BossStage => boss_stage::BUILT,
        GameLogic::CutThroat => enemy::cutthroat::BUILT,
        GameLogic::RobberThief => enemy::robberthief::BUILT,
        GameLogic::FloorSpike => floor_spike::BUILT,
        GameLogic::PathElevator => path_elevator::BUILT,
        GameLogic::Rope => rope::BUILT,
        GameLogic::SpringBoard => springboard::BUILT,
        GameLogic::SteppingStone => stepping_stone::BUILT,
        GameLogic::Katherine => enemy::katherine::BUILT,
        _ => true,
    }
}

/// The family that runs `name` now: the one that claims it
/// ([`kind_for`]), when that family is [`built`]; otherwise `None` (the
/// object stays an inert placeholder). The registry binds names with it.
pub fn runs(name: &str) -> Option<LogicKind> {
    kind_for(name).filter(|k| matches!(k, LogicKind::Game(g) if built(*g)))
}

/// Every image set a family may switch an object to, sorted and without
/// duplicates.
pub fn runtime_image_sets() -> Vec<&'static str> {
    let mut sets: BTreeSet<&'static str> = enemy::IMAGE_SETS.iter().copied().collect();
    sets.extend(enemy::human::IMAGE_SETS.iter().copied());
    for g in ALL {
        sets.extend(constant!(g, IMAGE_SETS).iter().copied());
    }
    sets.into_iter().collect()
}

/// Every animation key prefix a family may play, sorted and without
/// duplicates.
pub fn runtime_anim_prefixes() -> Vec<&'static str> {
    let mut prefixes: BTreeSet<&'static str> = enemy::ANIM_PREFIXES.iter().copied().collect();
    prefixes.extend(enemy::human::ANIM_PREFIXES.iter().copied());
    for g in ALL {
        prefixes.extend(constant!(g, ANIM_PREFIXES).iter().copied());
    }
    prefixes.into_iter().collect()
}

fn family(w: &World, id: ObjectId) -> Option<GameLogic> {
    match w.objects.get(id)?.kind {
        LogicKind::Game(g) => Some(g),
        _ => None,
    }
}

/// One logic call of `id`.
pub fn tick(w: &mut World, id: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.tick)(w, id);
        }
        dispatch!(g, tick(w, id));
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.on_hit)(w, id, attacker);
        }
        dispatch!(g, on_hit(w, id, attacker));
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.on_attack)(w, id, victim);
        }
        dispatch!(g, on_attack(w, id, victim));
    }
}

/// `carrier` lifted `id`. Kegs, crates and the human enemies Claw can
/// carry (officers, soldiers, cutthroats, robber thieves and Katherine,
/// see [`enemy::lift`]) react; other families ignore it.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::lifted(w, id, carrier),
        Some(GameLogic::Crate) => crates::lifted(w, id, carrier),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine,
        ) => enemy::lift::lifted(w, id, carrier),
        _ => {}
    }
}

/// `id` was thrown at (vx, vy) px/s.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::thrown(w, id, vx, vy),
        Some(GameLogic::Crate) => crates::thrown(w, id, vx, vy),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine,
        ) => enemy::lift::thrown(w, id, vx, vy),
        _ => {}
    }
}

/// Claw starts winding up a throw of `id`: a held human enemy shifts a little back and down (see [`enemy::lift::wind_up`]); other
/// families ignore it.
pub fn wind_up(w: &mut World, id: ObjectId) {
    if matches!(
        family(w, id),
        Some(
            GameLogic::Officer
                | GameLogic::Soldier
                | GameLogic::CutThroat
                | GameLogic::RobberThief
                | GameLogic::Katherine
        )
    ) {
        enemy::lift::wind_up(w, id);
    }
}

/// `id` was put down or let go.
pub fn dropped(w: &mut World, id: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::dropped(w, id),
        Some(GameLogic::Crate) => crates::dropped(w, id),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine,
        ) => enemy::lift::dropped(w, id),
        _ => {}
    }
}

/// A new object running `name` at (x, y, z), set up as the level loader
/// sets up a placed object: its family (or an inert placeholder), a level
/// object in its activation class, and always active when its logic is.
pub fn new_object(name: &str, x: i32, y: i32, z: i32) -> Object {
    let entry = Registry::shared().get(name);
    let reg_flags = entry.map_or(0, |e| e.flags);
    let kind = match entry.map(|e| e.kind) {
        Some(k @ LogicKind::Game(_)) => k,
        _ => LogicKind::Inert,
    };
    let logic: Arc<str> = entry.map_or_else(|| Arc::from(name), |e| e.name.clone());
    let mut o = Object::new(0, x, y, z, logic, kind);
    o.reg_flags = reg_flags;
    o.flags = flags::WORLD;
    if reg_flags & REG_CLASS_1 != 0 {
        o.flags |= flags::CLASS_1;
    } else if reg_flags & REG_CLASS_2 != 0 {
        o.flags |= flags::CLASS_2;
    }
    if reg_flags & REG_ALWAYS != 0 {
        o.flags |= flags::ALWAYS_ACTIVE;
    }
    o
}

/// An inert, non-colliding, undrawn object standing in for something a
/// family does not create yet; `data` records what was asked for.
pub(crate) fn placeholder(name: &str, x: i32, y: i32, z: i32, data: ObjectData) -> Object {
    let mut o = new_object(name, x, y, z);
    o.kind = LogicKind::Inert;
    o.flags |= flags::NO_COLLIDE;
    o.data = data;
    o
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit {
    use claw_frame::TickInput;

    use super::*;
    use crate::level::{ObjectSpec, SimLevel};
    use crate::logic::enemy::EnemyState;
    use crate::tiles::TileMap;
    use crate::{Sim, SimConfig};

    #[test]
    fn every_name_belongs_to_one_family() {
        let mut seen = BTreeSet::new();
        for g in ALL {
            for &n in names(g) {
                assert!(seen.insert(n), "{n} is claimed twice");
                assert_eq!(kind_for(n), Some(LogicKind::Game(g)), "{n}");
                assert!(Registry::shared().get(n).is_some(), "{n} is registered");
            }
        }
        assert_eq!(kind_for("CaptainClaw"), None);
        assert_eq!(kind_for("officer"), None, "names are case-sensitive");
    }

    #[test]
    fn runtime_lists_are_sorted_and_unique() {
        for list in [runtime_image_sets(), runtime_anim_prefixes()] {
            let mut sorted = list.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(list, sorted);
            assert!(list.iter().all(|s| !s.is_empty()));
        }
    }

    /// A flat floor with an officer, a soldier, a rat, a cutthroat and a
    /// robber thief beside Claw, all built.
    fn enemies_beside_claw() -> (Sim, ObjectId, [ObjectId; 5]) {
        let mut rows = vec![".".repeat(60); 15];
        rows.push("#".repeat(60));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
        for (name, x) in [
            ("Officer", 900),
            ("Soldier", 1100),
            ("Rat", 1200),
            ("CutThroat", 1300),
            ("RobberThief", 1400),
        ] {
            level.objects.push(ObjectSpec::at(name, x, 900, 4000));
        }
        let mut sim = Sim::new(level, SimConfig::default());
        for _ in 0..3 {
            sim.step(TickInput::default());
        }
        let find = |g: GameLogic| {
            sim.objects()
                .all()
                .find(|(_, o)| o.kind == LogicKind::Game(g))
                .map(|(id, _)| id)
                .expect("the enemy exists")
        };
        let ids = [
            find(GameLogic::Officer),
            find(GameLogic::Soldier),
            find(GameLogic::Rat),
            find(GameLogic::CutThroat),
            find(GameLogic::RobberThief),
        ];
        let claw = sim
            .objects()
            .all()
            .find(|(_, o)| o.kind == LogicKind::Player)
            .map(|(id, _)| id)
            .expect("Claw");
        (sim, claw, ids)
    }

    /// What lifting changes on an object: state, flags, position, type,
    /// physics and data.
    fn look(sim: &Sim, id: ObjectId) -> (i32, u32, i32, i32, u32, i32, ObjectData) {
        let o = sim.objects().get(id).expect("the object exists");
        (
            o.state,
            o.flags,
            o.x,
            o.y,
            o.type_flags,
            o.physics_type,
            o.data.clone(),
        )
    }

    fn enemy_state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
        sim.objects()
            .get(id)
            .and_then(|o| EnemyState::from_id(o.state))
    }

    #[test]
    fn lifting_an_officer_or_a_soldier_reaches_its_family() {
        let (sim, claw, [officer, soldier, rat, cutthroat, robber]) = enemies_beside_claw();
        let rat_before = look(&sim, rat);
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            assert!(enemy_state(&sim, id).is_some(), "built");
            sim.with_world(|w| lifted(w, id, claw));
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Lifted));
            sim.with_world(|w| thrown(w, id, 300, -650));
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Thrown));
        }
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            sim.with_world(|w| {
                lifted(w, id, claw);
                dropped(w, id);
            });
            sim.step(TickInput::default());
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Dropped));
        }
        // The rat is never lifted.
        let mut sim = sim.clone();
        sim.with_world(|w| {
            lifted(w, rat, claw);
            thrown(w, rat, 300, -650);
            dropped(w, rat);
        });
        assert_eq!(look(&sim, rat), rat_before);
    }

    #[test]
    fn winding_up_reaches_a_lifted_officer_or_soldier() {
        let (sim, claw, [officer, soldier, rat, cutthroat, robber]) = enemies_beside_claw();
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            let winding = |sim: &Sim| {
                enemy::data_of(sim.objects().get(id).expect("the enemy exists"))
                    .expect("enemy data")
                    .winding_up
            };
            sim.with_world(|w| lifted(w, id, claw));
            assert!(!winding(&sim));
            sim.with_world(|w| wind_up(w, id));
            assert!(winding(&sim));
            // Lifted again, it starts without the wind-up.
            sim.with_world(|w| lifted(w, id, claw));
            assert!(!winding(&sim));
        }
        // The rat is never carried and ignores it.
        let mut sim = sim.clone();
        let before = look(&sim, rat);
        sim.with_world(|w| wind_up(w, rat));
        assert_eq!(look(&sim, rat), before);
    }

    #[test]
    fn an_enemy_nobody_lifted_ignores_throws_and_drops() {
        let (sim, _, [officer, soldier, _, cutthroat, robber]) = enemies_beside_claw();
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            let before = look(&sim, id);
            sim.with_world(|w| {
                thrown(w, id, 300, -650);
                dropped(w, id);
            });
            assert_eq!(look(&sim, id), before);
        }
    }

    #[test]
    fn new_objects_follow_the_registry() {
        let o = new_object("Elevator", 1, 2, 3);
        assert_eq!(o.kind, LogicKind::Game(GameLogic::Platform));
        assert_eq!(o.activation_class(), 1);
        assert_eq!((o.x, o.y, o.z), (1, 2, 3));
        let o = new_object("GlobalAmbientSound", 0, 0, 0);
        assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
        let o = new_object("NoSuchLogic", 0, 0, 0);
        assert_eq!(o.kind, LogicKind::Inert);
        assert_eq!(&*o.logic, "NoSuchLogic");
        assert_eq!(o.activation_class(), 0);
    }
}
