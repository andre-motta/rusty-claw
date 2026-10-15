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
//!   through automatic hit damage sees [`engine_state::KILLED`] on its next
//!   call;
//! - `on_hit(w, id, attacker)` and `on_attack(w, id, victim)`: called by
//!   the pair-collision pass, in the order the pairs are tested, for
//!   objects with [`Object::hit_handler`] or [`Object::attack_handler`];
//! - its own data type, held in [`Object::data`] through [`ObjectData`].
//!
//! [`kind_for`] maps a logic name to its family; the registry uses it, and
//! names no family claims stay inert placeholders. [`tick`], [`on_hit`],
//! [`on_attack`] and the lift hooks dispatch a call to the object's
//! family.
//!
//! [`World`] is the context of every call. A family adds helpers of its
//! own in an `impl World` block in its own file.
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

pub mod checkpoint;
pub mod crates;
pub mod decor;
pub mod effect;
pub mod enemy;
pub mod keg;
pub mod pickup;
pub mod platform;
pub mod projectile;
pub mod sound;
pub mod sound_object;
pub mod world;

pub use world::{Outbox, World};

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
}

/// Every family, in a fixed order.
pub const ALL: [GameLogic; 12] = [
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
        }
    };
}

/// The logic names family `g` runs.
pub fn names(g: GameLogic) -> &'static [&'static str] {
    constant!(g, NAMES)
}

/// The family that runs `name`, or `None` when no family does (the object
/// stays an inert placeholder). Names are case-sensitive.
pub fn kind_for(name: &str) -> Option<LogicKind> {
    ALL.iter()
        .find(|&&g| names(g).contains(&name))
        .map(|&g| LogicKind::Game(g))
}

/// Every image set a family may switch an object to, sorted and without
/// duplicates.
pub fn runtime_image_sets() -> Vec<&'static str> {
    let mut sets: BTreeSet<&'static str> = enemy::IMAGE_SETS.iter().copied().collect();
    for g in ALL {
        sets.extend(constant!(g, IMAGE_SETS).iter().copied());
    }
    sets.into_iter().collect()
}

/// Every animation key prefix a family may play, sorted and without
/// duplicates.
pub fn runtime_anim_prefixes() -> Vec<&'static str> {
    let mut prefixes: BTreeSet<&'static str> = enemy::ANIM_PREFIXES.iter().copied().collect();
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

/// `carrier` lifted `id`.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::lifted(w, id, carrier),
        Some(GameLogic::Crate) => crates::lifted(w, id, carrier),
        _ => {}
    }
}

/// `id` was thrown at (vx, vy) px/s.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::thrown(w, id, vx, vy),
        Some(GameLogic::Crate) => crates::thrown(w, id, vx, vy),
        _ => {}
    }
}

/// `id` was put down or let go.
pub fn dropped(w: &mut World, id: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::dropped(w, id),
        Some(GameLogic::Crate) => crates::dropped(w, id),
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
    use super::*;

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
