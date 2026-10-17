//! Shots, thrown dynamite and the rat's bombs.
//!
//! A projectile is an ordinary object with an attack box and a kind. It
//! never decides a hit itself: the collision pass finds the overlap and the
//! victim's handler applies the damage and decides whether the projectile
//! goes. Every projectile is marked for removal when Claw respawns (see
//! [`purge`]).
//!
//! Each tick a projectile first applies its state rule (animation, impact
//! and removal checks that use the previous tick's tile result) and then
//! takes one motion step through the tile physics, at most 24 px per axis.
//! It does not move in the tick it is launched.
//!
//! - Straight shots (Claw's pistol shot at 700 px/s, his magic claw and the
//!   sword shots at 700 px/s, the soldier's musket ball at 500 px/s) fly
//!   without gravity and are removed on the tick after they touch a wall,
//!   when they leave their activation window or when they leave the level.
//!   The magic claw and the sword shots pierce: only walls stop them. The
//!   magic claw sheds two sparkles a tick and carries a light flare.
//! - Thrown dynamite flies a ballistic arc under 2000 px/s², chosen so that
//!   it would land the throw power away at the height it was thrown from,
//!   and a little steeper. It bounces twice (halving its horizontal speed,
//!   its vertical speed reversed and halved), lies still from the third
//!   landing, and goes off 1250 ms after it was lit whatever it is doing.
//!   A stick lit inside solid ground waits there for its fuse.
//! - The rat's bomb flies straight when thrown sideways and falls under
//!   1500 px/s² when dropped. It goes off where it lands or touches a
//!   wall. A bomb thrown sideways hurts Claw in flight and goes off on him
//!   ([`impact`]); a dropped one passes through everything until it lands.
//!
//! Exploding, dynamite and bombs become the level's own kind of shot,
//! freeze in place and play their explosion. They hurt everything in
//! their box exactly once, in the collision pass of the tick their
//! animation reaches its hit frame: dynamite a 101x101 px square, a bomb
//! its own box grown by 40 px on every side. Dynamite also shakes the
//! screen for half a second then. They are removed when the explosion
//! animation ends.

mod motion;

pub use motion::Motion;

use super::World;
use crate::anim::AnimPlayer;
use crate::container::Container;
use crate::level::runtime_image_set;
use crate::logic::{ObjectData, effect, new_object, placeholder};
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags, special, types};
use crate::physics::{opt, ptype, res};
use crate::registry::Registry;
use crate::tiles::SOLID;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Bullet",
    "Dynamite",
    "FireBullet",
    "IceBullet",
    "LightningBullet",
    "MagicBullet",
    "PistolBullet",
    "RatBomb",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    "GAME_BULLETS",
    "GAME_DYNAMITEEXPLO",
    "GAME_DYNAMITELIT",
    "GAME_MAGICCLAW",
    "GAME_PROJECTILES_FIRESWORD",
    "GAME_PROJECTILES_ICESWORD",
    "GAME_PROJECTILES_LIGHTNINGSWORD",
    "LEVEL_MUSKETBALL",
    "LEVEL_RATBOMB",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[
    "GAME_CYCLE50",
    "GAME_DYNAMITE",
    "GAME_FIRESWORD",
    "GAME_ICESWORD",
    "GAME_LIGHTNINGSWORD",
    "LEVEL_RATBOMB_",
];

/// Special flag of every projectile: removed when Claw respawns.
pub const RESPAWN_PURGE: u32 = 0x2000;

/// Gravity of thrown dynamite, px/s².
pub const DYNAMITE_GRAVITY: f64 = 2000.0;
/// Gravity of a dropped rat bomb, px/s².
pub const RAT_BOMB_GRAVITY: f64 = 1500.0;
/// Dynamite fuse, ms.
pub const DYNAMITE_FUSE_MS: u32 = 1250;
/// Landings dynamite bounces off before it lies still.
pub const DYNAMITE_BOUNCES: u32 = 2;
/// Damage of a rat bomb.
pub const RAT_BOMB_DAMAGE: i32 = 15;
/// How far an exploding rat bomb's box grows on every side, px.
pub const RAT_BOMB_BLAST_GROWTH: i32 = 40;
/// Box an exploding stick of dynamite hurts with.
pub const DYNAMITE_BLAST: Rect = Rect::new(-50, -50, 50, 50);
/// Screen shake an exploding stick of dynamite starts when it hurts, ms.
pub const DYNAMITE_SHAKE_MS: u32 = 500;
/// Depth given to dynamite and bombs launched at depth 0.
pub const DEFAULT_Z: i32 = 4990;
/// Depth of dynamite waiting in solid ground.
pub const BURIED_Z: i32 = 8500;
/// Sound of exploding dynamite.
pub const DYNAMITE_SOUND: &str = "GAME_KEGBOOM";
/// Sound of an exploding rat bomb.
pub const RAT_BOMB_SOUND: &str = "GAME_BOMBEXP";
/// How far ahead of a sword shot its flare is kept, px.
pub const SWORD_FLARE_LEAD: i32 = 20;

/// The kinds of projectile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Claw's pistol shot.
    PistolBullet,
    /// Claw's magic claw.
    MagicBullet,
    /// The fire sword's shot.
    FireBullet,
    /// The lightning sword's shot.
    LightningBullet,
    /// The ice sword's shot.
    IceBullet,
    /// An enemy's musket ball.
    Bullet,
    /// Thrown dynamite.
    Dynamite,
    /// The rat's bomb.
    RatBomb,
}

impl Class {
    /// The class running logic `name`.
    pub fn from_logic(name: &str) -> Option<Class> {
        Some(match name {
            "PistolBullet" => Class::PistolBullet,
            "MagicBullet" => Class::MagicBullet,
            "FireBullet" => Class::FireBullet,
            "LightningBullet" => Class::LightningBullet,
            "IceBullet" => Class::IceBullet,
            "Bullet" => Class::Bullet,
            "Dynamite" => Class::Dynamite,
            "RatBomb" => Class::RatBomb,
            _ => return None,
        })
    }

    /// The image set it is drawn from.
    pub fn image_set(self) -> &'static str {
        match self {
            Class::PistolBullet => "GAME_BULLETS",
            Class::MagicBullet => "GAME_MAGICCLAW",
            Class::FireBullet => "GAME_PROJECTILES_FIRESWORD",
            Class::LightningBullet => "GAME_PROJECTILES_LIGHTNINGSWORD",
            Class::IceBullet => "GAME_PROJECTILES_ICESWORD",
            Class::Bullet => "LEVEL_MUSKETBALL",
            Class::Dynamite => "GAME_DYNAMITELIT",
            Class::RatBomb => "LEVEL_RATBOMB",
        }
    }

    /// The animation it plays while flying, if any (the rat's bomb picks
    /// one by its throw).
    pub fn flight_anim(self) -> Option<&'static str> {
        match self {
            Class::MagicBullet => Some("GAME_CYCLE50"),
            Class::FireBullet => Some("GAME_FIRESWORD"),
            Class::LightningBullet => Some("GAME_LIGHTNINGSWORD"),
            Class::IceBullet => Some("GAME_ICESWORD"),
            Class::Dynamite => Some("GAME_DYNAMITELIT"),
            _ => None,
        }
    }

    /// Its movement and attack box when the level does not know the first
    /// frame of its image set: that frame's drawn box in the game's own
    /// art, relative to its position. The sword shots use the magic claw's
    /// box.
    pub fn frame_box(self) -> Rect {
        match self {
            Class::PistolBullet => Rect::new(-2, -2, 1, 1),
            Class::MagicBullet | Class::FireBullet | Class::LightningBullet | Class::IceBullet => {
                Rect::new(-36, -18, 31, 13)
            }
            Class::Bullet => Rect::new(-6, -5, 5, 4),
            Class::Dynamite => Rect::new(-13, -14, 14, 9),
            Class::RatBomb => Rect::new(-17, 33, 6, 54),
        }
    }

    /// The magic claw and the sword shots: they pierce and carry a flare.
    pub fn is_magic(self) -> bool {
        matches!(
            self,
            Class::MagicBullet | Class::FireBullet | Class::LightningBullet | Class::IceBullet
        )
    }

    /// Flies straight and is removed at the first wall.
    pub fn is_straight(self) -> bool {
        !matches!(self, Class::Dynamite | Class::RatBomb)
    }
}

/// What a projectile is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    /// Flying, bouncing or lying still.
    #[default]
    Flying,
    /// Playing its explosion.
    Exploding,
}

/// A projectile's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectileData {
    /// What was asked for when it was launched.
    pub spec: Option<ProjectileSpec>,
    /// Its class; `None` for a projectile record placed in a level, which
    /// stays where it is.
    pub class: Option<Class>,
    /// What it is doing.
    pub phase: Phase,
    /// Floating-point position (absolute), velocity and acceleration.
    pub motion: Motion,
    /// Motion stopped: lying still, waiting in the ground or exploding.
    pub frozen: bool,
    /// Tile physics result of the last motion step.
    pub last_result: u32,
    /// Option mask handed to the tile physics.
    pub tile_mask: u32,
    /// Landings bounced off so far.
    pub bounces: u32,
    /// Landings it bounces off before lying still.
    pub bounce_limit: u32,
    /// Simulation time the fuse was lit, ms.
    pub fuse_start_ms: u64,
    /// Fuse length, ms (0: no fuse).
    pub fuse_ms: u32,
    /// The explosion has armed its box.
    pub armed: bool,
    /// The light flare riding on it.
    pub flare: Option<ObjectId>,
    /// Sparkles shed per flying tick.
    pub sparkles: u32,
    /// Claw's attack count when he launched it, for his pistol, magic and
    /// sword shots (0 for any other projectile). Nothing reads it: an
    /// enemy takes one hit per attack of Claw's through its own record of
    /// his attack count, so a shot that passes through several enemies
    /// hits each of them once.
    pub attack_serial: u32,
}

/// What to launch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectileSpec {
    /// Logic name of the projectile.
    pub logic: &'static str,
    /// Start position.
    pub x: i32,
    /// Start position.
    pub y: i32,
    /// Depth.
    pub z: i32,
    /// Horizontal speed, px/s.
    pub vx: i32,
    /// Vertical speed, px/s.
    pub vy: i32,
    /// Damage it deals.
    pub damage: i32,
    /// What kind of object it is.
    pub type_flags: u32,
    /// Kinds that can hit it.
    pub hit_type_flags: u32,
    /// Logic-specific flags.
    pub special_flags: u32,
    /// Image set it is drawn from.
    pub image_set: &'static str,
    /// Object that launched it.
    pub owner: Option<ObjectId>,
    /// Time before it goes off, for dynamite, ms.
    pub fuse_ms: Option<u32>,
    /// Strength of a charged throw.
    pub power: i32,
}

/// Launches `spec` as [`spawn`] does and records Claw's attack count
/// `serial` in the projectile's data (see
/// [`ProjectileData::attack_serial`]).
pub fn spawn_tagged(w: &mut World, spec: ProjectileSpec, serial: u32) -> Option<ObjectId> {
    let id = spawn(w, spec)?;
    if let Some(ObjectData::Projectile(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
        d.attack_serial = serial;
    }
    Some(id)
}

/// The data of projectile `o`, if it is one.
pub fn data(o: &Object) -> Option<&ProjectileData> {
    match &o.data {
        ObjectData::Projectile(d) => Some(d),
        _ => None,
    }
}

fn load(w: &World, id: ObjectId) -> Option<(ProjectileData, Class)> {
    let d = data(w.objects.get(id)?)?;
    let class = d.class?;
    Some((d.clone(), class))
}

fn store(w: &mut World, id: ObjectId, d: ProjectileData) {
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Projectile(d);
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(destroying) = w
        .objects
        .get(id)
        .map(|o| o.state == engine_state::DESTROYING)
    else {
        return;
    };
    let Some((d, class)) = load(w, id) else {
        return;
    };
    if destroying {
        // The shot goes, and its flare with it.
        if let Some(f) = d.flare {
            w.destroy(f);
        }
        return;
    }
    match (d.phase, class) {
        (Phase::Exploding, _) => exploding(w, id, d, class),
        (Phase::Flying, Class::Dynamite) => dynamite_tick(w, id, d),
        (Phase::Flying, Class::RatBomb) => rat_bomb_tick(w, id, d),
        (Phase::Flying, _) => shot_tick(w, id, d, class),
    }
}

/// `attacker` hit `id`. Projectiles cannot be hurt.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Projectiles leave hits to their victims.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// A projectile that hit Claw and goes off instead of disappearing (its
/// special flags ask for this): a rat bomb explodes where it is, even when
/// it is already exploding (its explosion starts again, and its box grows
/// again when it arms). Other projectiles do nothing.
pub fn impact(w: &mut World, id: ObjectId) {
    let Some((mut d, class)) = load(w, id) else {
        return;
    };
    if class == Class::RatBomb {
        explode(w, id, &mut d, class);
        store(w, id, d);
    }
}

/// Marks every projectile for removal, as Claw's respawn does.
pub fn purge(objects: &mut Container) {
    let ids: Vec<ObjectId> = objects
        .all()
        .filter(|(_, o)| o.special_flags & RESPAWN_PURGE != 0)
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if let Some(o) = objects.get_mut(id) {
            o.flags |= flags::DESTROY;
        }
    }
}

/// The launch velocity of dynamite thrown with `power`: the arc that lands
/// `power` px away at the height it was thrown from, made steeper by
/// `2 |power|` px/s upwards. No power drops the stick where it is.
pub fn throw_velocity(power: i32) -> (f64, f64) {
    if power == 0 {
        return (0.0, 0.0);
    }
    let p = f64::from(power);
    let t = (p.abs() / 1000.0).sqrt();
    (p / t, -1000.0 * t - (2.0 * p).abs())
}

/// Launches a projectile and returns its id; `None` when no logic of that
/// name exists. A known logic this build does not fly yet leaves an inert
/// placeholder whose data holds `spec`.
///
/// The projectile is set up at once, joins the live list at the end of the
/// current pass and moves from the next update pass on. Class rules
/// override the request where the game's own do: dynamite is harmless
/// until it explodes and takes its speed from `power`, and a rat bomb
/// always deals [`RAT_BOMB_DAMAGE`] as an enemy shot.
pub fn spawn(w: &mut World, spec: ProjectileSpec) -> Option<ObjectId> {
    let Some(class) = Class::from_logic(spec.logic) else {
        Registry::shared().get(spec.logic)?;
        let data = ObjectData::Projectile(ProjectileData {
            spec: Some(spec),
            ..ProjectileData::default()
        });
        let mut o = placeholder(spec.logic, spec.x, spec.y, spec.z, data);
        o.speed_x = spec.vx;
        o.speed_y = spec.vy;
        return Some(w.spawn(o));
    };
    let mut o = new_object(spec.logic, spec.x, spec.y, spec.z);
    o.flags |= flags::DESTROY_OUTSIDE | flags::DESTROY_FAR;
    o.special_flags = spec.special_flags | RESPAWN_PURGE;
    o.damage = spec.damage;
    o.type_flags = spec.type_flags;
    o.hit_type_flags = spec.hit_type_flags;
    o.physics_type = ptype::FLY;
    let image = match class {
        Class::Bullet if !spec.image_set.is_empty() && IMAGE_SETS.contains(&spec.image_set) => {
            spec.image_set
        }
        _ => class.image_set(),
    };
    o.image_set = runtime_image_set(image);
    // The boxes come from the first frame of the set and keep that size
    // whatever frame the animation shows later.
    o.frame = w.image_frames(image).map_or(1, |(first, _)| first);
    let frame_box = w.frame_box(image, o.frame).unwrap_or(class.frame_box());
    o.move_rect = frame_box;
    o.attack_rect = frame_box;
    let mut d = ProjectileData {
        spec: Some(spec),
        class: Some(class),
        ..ProjectileData::default()
    };
    let (mut vx, mut vy, mut ay) = (f64::from(spec.vx), f64::from(spec.vy), 0.0);
    match class {
        Class::PistolBullet => {}
        Class::MagicBullet
        | Class::FireBullet
        | Class::LightningBullet
        | Class::IceBullet
        | Class::Bullet => {
            if spec.vx < 0 {
                o.draw_flags |= draw_flags::MIRROR;
            }
            if class.is_magic() {
                o.flags |= flags::NOTIFY_DESTROY;
            }
        }
        Class::Dynamite => {
            o.type_flags = types::ENEMY;
            o.flags |= flags::HARMLESS | flags::ALWAYS_ACTIVE;
            o.attack_rect = DYNAMITE_BLAST;
            o.hit_rect = frame_box;
            if o.z == 0 {
                o.z = DEFAULT_Z;
            }
            (vx, vy) = throw_velocity(spec.power);
            ay = DYNAMITE_GRAVITY;
            d.tile_mask = opt::PLATFORMS;
            d.bounce_limit = DYNAMITE_BOUNCES;
            d.fuse_ms = spec.fuse_ms.unwrap_or(DYNAMITE_FUSE_MS);
            d.fuse_start_ms = w.now_ms;
            d.sparkles = 1;
            if w.level.map.attr(spec.x, spec.y + frame_box.bottom) == SOLID {
                d.frozen = true;
                o.z = BURIED_Z;
            }
        }
        Class::RatBomb => {
            o.type_flags = types::ENEMY_SHOT;
            o.damage = RAT_BOMB_DAMAGE;
            o.hit_rect = frame_box;
            o.flags |= flags::SAFE;
            if o.special_flags & special::OWN_IMPACT == 0 {
                o.flags |= flags::HARMLESS;
            }
            if o.z == 0 {
                o.z = DEFAULT_Z;
            }
            if spec.vx == 0 {
                ay = RAT_BOMB_GRAVITY;
            } else if spec.vx > 0 {
                o.draw_flags |= draw_flags::MIRROR;
                o.mirror_rects();
            }
        }
    }
    d.motion = Motion {
        x: f64::from(spec.x),
        y: f64::from(spec.y),
        vx,
        vy,
        ay,
    };
    o.speed_x = vx as i32;
    o.speed_y = vy as i32;
    if class != Class::Dynamite && motion::overlaps_solid(w, &o) {
        // Launched inside a wall: it hits on its first tick.
        d.last_result = res::HIT;
    }
    let (x, y, z) = (o.x, o.y, o.z);
    o.data = ObjectData::Projectile(d);
    let id = w.spawn(o);
    match class {
        Class::RatBomb => {
            let anim = if spec.vx == 0 {
                "LEVEL_RATBOMB_FALL"
            } else {
                "LEVEL_RATBOMB_FALLEASTWEST"
            };
            w.set_anim(id, anim);
            w.advance_anim(id);
        }
        _ => {
            if let Some(anim) = class.flight_anim() {
                w.set_anim(id, anim);
            }
        }
    }
    if class.is_magic() {
        let flare = effect::spawn_flare(w, id, x, y, z - 1);
        if let Some(o) = w.objects.get_mut(id)
            && let ObjectData::Projectile(d) = &mut o.data
        {
            d.flare = Some(flare);
            // The magic claw sheds two sparkles with its flare; the sword
            // shots shed none.
            d.sparkles = if class == Class::MagicBullet { 2 } else { 0 };
        }
    }
    Some(id)
}

/// Advances the animation when there is one.
fn animate(w: &mut World, id: ObjectId) -> i32 {
    let has_anim = w
        .objects
        .get(id)
        .is_some_and(|o| o.anim.animation().is_some());
    if has_anim { w.advance_anim(id) } else { -1 }
}

/// True when `id`'s position is outside the level.
fn outside_level(w: &World, id: ObjectId) -> bool {
    w.objects.get(id).is_some_and(|o| {
        o.x < 0 || o.y < 0 || o.x >= w.level.map.width() || o.y >= w.level.map.height()
    })
}

/// Sheds `d.sparkles` sparkles over the projectile's box.
fn shed_sparkles(w: &mut World, id: ObjectId, d: &ProjectileData) {
    let Some((rect, z)) = w.objects.get(id).map(|o| (o.move_rect, o.z)) else {
        return;
    };
    for _ in 0..d.sparkles {
        effect::spawn_sparkle(w, d.motion.x, d.motion.y, rect, z - 1);
    }
}

fn shot_tick(w: &mut World, id: ObjectId, mut d: ProjectileData, class: Class) {
    animate(w, id);
    let gone = d.last_result & res::HIT != 0 || outside_level(w, id);
    if gone {
        // It spends this tick where the wall stopped it and is removed at
        // the end of the pass, before it can hit anything.
        w.destroy(id);
    }
    if class.is_magic() {
        shed_sparkles(w, id, &d);
        if let Some(f) = d.flare {
            let lead = if class == Class::MagicBullet {
                0
            } else if d.motion.vx < 0.0 {
                -SWORD_FLARE_LEAD
            } else {
                SWORD_FLARE_LEAD
            };
            let (x, y) = (d.motion.x as i32, d.motion.y as i32);
            if let Some(fo) = w.objects.get_mut(f) {
                fo.x = x + lead;
                fo.y = y;
            }
        }
    }
    if !gone {
        motion::integrate(w, id, &mut d);
    }
    store(w, id, d);
}

fn dynamite_tick(w: &mut World, id: ObjectId, mut d: ProjectileData) {
    animate(w, id);
    if d.fuse_ms > 0 && w.now_ms.saturating_sub(d.fuse_start_ms) >= u64::from(d.fuse_ms) {
        explode(w, id, &mut d, Class::Dynamite);
    }
    let landed = w
        .objects
        .get(id)
        .is_some_and(|o| o.physics_type == ptype::STAND);
    if landed {
        let exploding = w
            .objects
            .get(id)
            .is_some_and(|o| o.anim.key() == Some("GAME_DYNAMITEEXPLO"));
        if !exploding {
            w.set_anim(id, "GAME_DYNAMITEHORZ");
        }
        if d.bounces >= d.bounce_limit {
            // Lies still until the fuse fires.
            d.frozen = true;
            if !exploding && let Some(o) = w.objects.get_mut(id) {
                o.anim = AnimPlayer::default();
            }
        } else {
            motion::bounce(w, id, &mut d);
        }
    }
    shed_sparkles(w, id, &d);
    motion::integrate(w, id, &mut d);
    store(w, id, d);
}

fn rat_bomb_tick(w: &mut World, id: ObjectId, mut d: ProjectileData) {
    animate(w, id);
    let landed = w
        .objects
        .get(id)
        .is_some_and(|o| o.physics_type == ptype::STAND);
    if landed || d.last_result & res::HIT != 0 {
        explode(w, id, &mut d, Class::RatBomb);
        store(w, id, d);
        return;
    }
    if outside_level(w, id) {
        w.destroy(id);
        return;
    }
    motion::integrate(w, id, &mut d);
    store(w, id, d);
}

/// Turns dynamite or a rat bomb into its explosion: the level's own kind
/// of shot, frozen, playing its explosion animation and sound. Its box is
/// armed later, on the animation's hit frame.
fn explode(w: &mut World, id: ObjectId, d: &mut ProjectileData, class: Class) {
    let (anim, sound) = match class {
        Class::Dynamite => ("GAME_DYNAMITEEXPLO", DYNAMITE_SOUND),
        Class::RatBomb => ("LEVEL_RATBOMB_EXPLODE", RAT_BOMB_SOUND),
        _ => return,
    };
    d.frozen = true;
    d.phase = Phase::Exploding;
    d.armed = false;
    effect::boom(w, sound);
    let first = w.image_frames("GAME_DYNAMITEEXPLO").map_or(1, |(f, _)| f);
    if let Some(o) = w.objects.get_mut(id) {
        o.type_flags = types::LEVEL_SHOT;
        if class == Class::Dynamite {
            o.image_set = runtime_image_set("GAME_DYNAMITEEXPLO");
            o.frame = first;
        }
    }
    w.set_anim(id, anim);
}

fn exploding(w: &mut World, id: ObjectId, mut d: ProjectileData, class: Class) {
    let Some(has_anim) = w.objects.get_mut(id).map(|o| {
        o.flags |= flags::HARMLESS;
        o.anim.animation().is_some()
    }) else {
        return;
    };
    let event = animate(w, id);
    let (arm, done) = if has_anim {
        let done = w.objects.get(id).is_none_or(|o| o.anim.is_done());
        (!d.armed && event == 1, done)
    } else {
        // Without its animation it hurts in the next collision pass and
        // goes in the tick after.
        (!d.armed, d.armed)
    };
    if arm {
        d.armed = true;
        if let Some(o) = w.objects.get_mut(id) {
            o.flags &= !flags::HARMLESS;
            if class == Class::RatBomb {
                let g = RAT_BOMB_BLAST_GROWTH;
                let r = o.attack_rect;
                o.attack_rect = Rect::new(r.left - g, r.top - g, r.right + g, r.bottom + g);
            }
        }
        if class == Class::Dynamite {
            w.shake(DYNAMITE_SHAKE_MS);
        }
    }
    if done {
        w.destroy(id);
    }
    store(w, id, d);
}
