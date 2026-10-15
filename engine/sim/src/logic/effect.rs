//! Short-lived combat effects: hit bursts, sparkles, the glitter that
//! trails a powered-up Claw, the light flare that rides on a magic shot,
//! splashes, and stand-alone explosions.
//!
//! Every effect but the explosion is an animation that never collides and
//! removes itself when its animation has finished (the glitter driver and
//! the flare are removed by their owners instead):
//!
//! - a hit burst picks one of three sizes at random, follows the object
//!   that was hit and ends early when that object is gone or off screen;
//! - a sparkle plays its six-frame twinkle once at a random point of the
//!   box of whatever sheds it;
//! - the glitter driver is hidden and sheds two sparkles over its target
//!   every tick until its owner removes it;
//! - the flare is the glow around a magic shot. It is kept hidden for now,
//!   because it needs a light blend the renderer does not offer objects
//!   yet;
//! - a splash plays once where it was asked for.
//!
//! An explosion asked for through [`EffectKind::Explosion`] draws nothing:
//! it hurts everything in its square that the level's own shots can hurt,
//! once, in the first collision pass after its first logic call, never
//! hurts the object it stands for, and then removes itself. Thrown
//! dynamite and the rat's bombs do not use it: they turn into their own
//! explosion.

use claw_frame::SoundChannel;

use super::World;
use crate::level::runtime_image_set;
use crate::logic::{GameLogic, ObjectData, new_object};
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Flare",
    "GlitterBaby",
    "GlitterMother",
    "HitBurst",
    "Splash",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    "GAME_CLAWHIT",
    "GAME_ENEMYHIT",
    "GAME_LIGHTFX_FLARE",
    "GAME_SPARKLE",
    "LEVEL_SPLASH",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[
    "GAME_BIGHITBURST",
    "GAME_GLITTER1",
    "GAME_LIGHTFX_FLARE",
    "GAME_MEDHITBURST",
    "GAME_SMLHITBURST",
    "GAME_SPLASH",
];

/// Logic name of a stand-alone explosion. It is not a level logic: only
/// [`spawn`] creates one.
pub const EXPLOSION: &str = "Explosion";

/// Depth of a hit burst asked for without an owner.
pub const HIT_BURST_Z: i32 = 8501;

/// The three hit burst animations, by size.
pub const HIT_BURST_ANIMS: [&str; 3] = ["GAME_SMLHITBURST", "GAME_MEDHITBURST", "GAME_BIGHITBURST"];

/// Sparkles the glitter driver sheds per tick.
pub const GLITTER_PER_TICK: u32 = 2;

/// An effect's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectData {
    /// What was asked for when it was spawned.
    pub request: Option<EffectKind>,
    /// The object it belongs to: the object a burst follows, the shot a
    /// flare rides on, the object glitter trails, or what an explosion
    /// stands for.
    pub owner: Option<ObjectId>,
    /// Offset from the owner's position that a burst keeps.
    pub offset: (i32, i32),
    /// Set up (image, animation and flags chosen).
    pub started: bool,
    /// An explosion has had its collision pass.
    pub armed: bool,
    /// An explosion made its owner unhittable for its collision pass and
    /// must undo that.
    pub shielded_owner: bool,
}

/// What effect to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// The spark of a blow landing.
    HitBurst,
    /// A splash in water or goo.
    Splash,
    /// An explosion, hurting what is within `radius` px.
    Explosion {
        /// Damage dealt.
        damage: i32,
        /// Reach, px.
        radius: i32,
        /// What exploded.
        source: Option<ObjectId>,
    },
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let name = o.logic.clone();
    match &*name {
        "HitBurst" => hit_burst_tick(w, id),
        "GlitterBaby" | "Splash" => one_shot_tick(w, id),
        "GlitterMother" => glitter_tick(w, id),
        "Flare" => flare_tick(w, id),
        EXPLOSION => explosion_tick(w, id),
        _ => {}
    }
}

/// `attacker` hit `id`. Effects are never hit.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. Effects attack no one; an explosion's hits are
/// handled by its victims.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// Shows an effect at (x, y) and returns its id.
///
/// A hit burst asked for here has no owner: it uses the enemy art, sits at
/// depth [`HIT_BURST_Z`] and stays where it is. A splash sits just in
/// front of Claw. An explosion is described in the module notes.
pub fn spawn(w: &mut World, kind: EffectKind, x: i32, y: i32) -> Option<ObjectId> {
    match kind {
        EffectKind::HitBurst => spawn_hit_burst(w, x, y, HIT_BURST_Z, None, false),
        EffectKind::Splash => {
            let z = w.claw_object().map_or(0, |c| c.z + 2);
            let mut o = new_object("Splash", x, y, z);
            o.data = ObjectData::Effect(EffectData {
                request: Some(kind),
                ..EffectData::default()
            });
            let id = w.spawn(o);
            start_splash(w, id);
            Some(id)
        }
        EffectKind::Explosion {
            damage,
            radius,
            source,
        } => {
            let mut o = new_object(EXPLOSION, x, y, 0);
            o.kind = LogicKind::Game(GameLogic::Effect);
            o.flags |= flags::ALWAYS_ACTIVE | flags::HARMLESS;
            o.type_flags = types::LEVEL_SHOT;
            o.damage = damage;
            o.attack_rect = Rect::new(-radius, -radius, radius, radius);
            o.data = ObjectData::Effect(EffectData {
                request: Some(kind),
                owner: source,
                started: true,
                ..EffectData::default()
            });
            Some(w.spawn(o))
        }
    }
}

/// Shows a hit burst at (x, y, z). With an `owner` the burst follows it,
/// and nothing is shown when the owner is gone, about to be removed or
/// off screen; `claw_image` picks Claw's burst art over an enemy's.
pub fn spawn_hit_burst(
    w: &mut World,
    x: i32,
    y: i32,
    z: i32,
    owner: Option<ObjectId>,
    claw_image: bool,
) -> Option<ObjectId> {
    let offset = match owner {
        Some(owner) => {
            if !shown(w, owner) {
                return None;
            }
            let o = w.objects.get(owner)?;
            (x - o.x, y - o.y)
        }
        None => (0, 0),
    };
    let mut o = new_object("HitBurst", x, y, z);
    o.image_set = runtime_image_set(if claw_image {
        "GAME_CLAWHIT"
    } else {
        "GAME_ENEMYHIT"
    });
    o.data = ObjectData::Effect(EffectData {
        request: Some(EffectKind::HitBurst),
        owner,
        offset,
        ..EffectData::default()
    });
    let id = w.spawn(o);
    start_hit_burst(w, id);
    Some(id)
}

/// Starts the glitter that trails `target` while a powerup lasts and
/// returns the object that drives it; the caller removes it (with
/// [`World::destroy`]) when the powerup ends. `None` when `target` does
/// not exist.
pub fn spawn_glitter(w: &mut World, target: ObjectId) -> Option<ObjectId> {
    w.objects.get(target)?;
    let mut o = new_object("GlitterMother", 100, 100, 0);
    // Not a level object: it never sleeps and is never collided.
    o.flags = flags::NO_COLLIDE;
    o.draw_flags |= draw_flags::HIDDEN;
    o.data = ObjectData::Effect(EffectData {
        owner: Some(target),
        started: true,
        ..EffectData::default()
    });
    Some(w.spawn(o))
}

/// Creates the light flare of a magic shot `owner` at (x, y, z).
pub(crate) fn spawn_flare(w: &mut World, owner: ObjectId, x: i32, y: i32, z: i32) -> ObjectId {
    let mut o = new_object("Flare", x, y, z);
    o.flags |= flags::NO_COLLIDE | flags::ALWAYS_ACTIVE;
    o.type_flags = 1;
    o.data = ObjectData::Effect(EffectData {
        owner: Some(owner),
        ..EffectData::default()
    });
    let id = w.spawn(o);
    start_flare(w, id);
    id
}

/// Sheds one sparkle at a random point of `rect` around (x, y), at depth
/// `z`. Two random draws: x first, then y.
pub(crate) fn spawn_sparkle(w: &mut World, x: f64, y: f64, rect: Rect, z: i32) -> ObjectId {
    let rx = random_in(w, rect.left, rect.right);
    let ry = random_in(w, rect.top, rect.bottom);
    let mut o = new_object(
        "GlitterBaby",
        (x + rx).trunc() as i32,
        (y + ry).trunc() as i32,
        z,
    );
    o.data = ObjectData::Effect(EffectData::default());
    let id = w.spawn(o);
    start_sparkle(w, id);
    id
}

/// A random offset in `lo..=hi`; `lo` or `hi` at random when the range is
/// empty.
fn random_in(w: &mut World, lo: i32, hi: i32) -> f64 {
    let n = i64::from(hi) - i64::from(lo) + 1;
    let r = if n > 0 {
        i64::from(lo) + i64::from(w.rng.below(n.min(i64::from(u32::MAX)) as u32))
    } else if w.rng.next_u15() & 1 == 0 {
        i64::from(lo)
    } else {
        i64::from(hi)
    };
    r as f64
}

/// True when `id` exists, is not about to be removed and was on screen:
/// its box (or its position, without one) overlaps the logic view.
fn shown(w: &World, id: ObjectId) -> bool {
    let Some(o) = w.objects.get(id) else {
        return false;
    };
    if o.flags & flags::DESTROY != 0 || o.draw_flags & draw_flags::HIDDEN != 0 {
        return false;
    }
    let r = if o.hit_rect.is_set() {
        o.hit_rect
    } else if o.move_rect.is_set() {
        o.move_rect
    } else {
        Rect::new(0, 0, 0, 0)
    };
    r.translate(o.x, o.y).overlaps(&w.view)
}

fn data_mut(o: &mut Object) -> &mut EffectData {
    if !matches!(o.data, ObjectData::Effect(_)) {
        o.data = ObjectData::Effect(EffectData::default());
    }
    match &mut o.data {
        ObjectData::Effect(d) => d,
        _ => unreachable!("set just above"),
    }
}

fn effect_data(w: &World, id: ObjectId) -> Option<EffectData> {
    match &w.objects.get(id)?.data {
        ObjectData::Effect(d) => Some(d.clone()),
        _ => None,
    }
}

/// Marks an animated effect as never colliding, removed when it leaves
/// its window, and started.
fn start_common(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NO_COLLIDE | flags::DESTROY_OUTSIDE;
        data_mut(o).started = true;
    }
}

fn start_hit_burst(w: &mut World, id: ObjectId) {
    start_common(w, id);
    if w.objects.get(id).is_some_and(|o| o.image_set.is_empty())
        && let Some(o) = w.objects.get_mut(id)
    {
        o.image_set = runtime_image_set("GAME_ENEMYHIT");
    }
    let size = w.rng.below(3) as usize;
    w.set_anim(id, HIT_BURST_ANIMS[size]);
}

fn start_sparkle(w: &mut World, id: ObjectId) {
    start_common(w, id);
    if let Some(o) = w.objects.get_mut(id) {
        o.image_set = runtime_image_set("GAME_SPARKLE");
    }
    w.set_anim(id, "GAME_GLITTER1");
}

fn start_splash(w: &mut World, id: ObjectId) {
    start_common(w, id);
    if let Some(o) = w.objects.get_mut(id) {
        o.image_set = runtime_image_set("LEVEL_SPLASH");
    }
    w.set_anim(id, "GAME_SPLASH");
}

fn start_flare(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.image_set = runtime_image_set("GAME_LIGHTFX_FLARE");
        // The flare is a light glow blended over the scene; drawn as a
        // plain sprite it would cover the shot, so it stays hidden until
        // objects can ask for that blend.
        o.draw_flags |= draw_flags::HIDDEN;
        data_mut(o).started = true;
    }
    w.set_anim(id, "GAME_LIGHTFX_FLARE");
}

/// Advances the animation and removes the effect once it has finished.
/// True when the effect goes.
fn play_once(w: &mut World, id: ObjectId) -> bool {
    w.advance_anim(id);
    let done = w.objects.get(id).is_none_or(|o| o.anim.is_done());
    if done {
        w.destroy(id);
    }
    done
}

fn hit_burst_tick(w: &mut World, id: ObjectId) {
    let d = effect_data(w, id).unwrap_or_default();
    if !d.started {
        start_hit_burst(w, id);
        return;
    }
    if let Some(owner) = d.owner
        && !shown(w, owner)
    {
        w.destroy(id);
        return;
    }
    if play_once(w, id) {
        return;
    }
    if let Some(owner) = d.owner
        && let Some((ox, oy)) = w.objects.get(owner).map(|o| (o.x, o.y))
        && let Some(o) = w.objects.get_mut(id)
    {
        o.x = ox + d.offset.0;
        o.y = oy + d.offset.1;
    }
}

fn one_shot_tick(w: &mut World, id: ObjectId) {
    let started = effect_data(w, id).is_some_and(|d| d.started);
    if !started {
        let splash = w.objects.get(id).is_some_and(|o| &*o.logic == "Splash");
        if splash {
            start_splash(w, id);
        } else {
            start_sparkle(w, id);
        }
        return;
    }
    play_once(w, id);
}

fn glitter_tick(w: &mut World, id: ObjectId) {
    let Some(target) = effect_data(w, id).and_then(|d| d.owner) else {
        return;
    };
    if let Some(o) = w.objects.get_mut(id) {
        // Sheds every 15 ms, which at the 18 ms tick is every tick.
        o.time_delay = 15;
    }
    let Some((x, y, z, rect)) = w.objects.get(target).map(|t| (t.x, t.y, t.z, t.move_rect)) else {
        return;
    };
    let rect = if rect.is_set() {
        rect
    } else {
        Rect::new(0, 0, 0, 0)
    };
    for _ in 0..GLITTER_PER_TICK {
        spawn_sparkle(w, f64::from(x), f64::from(y), rect, z + 1);
    }
}

fn flare_tick(w: &mut World, id: ObjectId) {
    let d = effect_data(w, id).unwrap_or_default();
    if !d.started {
        start_flare(w, id);
    }
    // A flare placed in a level has no owner and simply stays; one that
    // rides on a shot goes with it.
    if let Some(owner) = d.owner
        && w.objects
            .get(owner)
            .is_none_or(|o| o.flags & flags::DESTROY != 0)
    {
        w.destroy(id);
    }
}

fn explosion_tick(w: &mut World, id: ObjectId) {
    let Some(d) = effect_data(w, id) else { return };
    if !d.armed {
        // Armed for the collision pass that follows this update pass.
        let mut shielded = false;
        if let Some(src) = d.owner
            && let Some(s) = w.objects.get_mut(src)
            && s.flags & flags::SAFE == 0
        {
            s.flags |= flags::SAFE;
            shielded = true;
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.flags &= !flags::HARMLESS;
            let d = data_mut(o);
            d.armed = true;
            d.shielded_owner = shielded;
        }
        return;
    }
    if d.shielded_owner
        && let Some(src) = d.owner
        && let Some(s) = w.objects.get_mut(src)
    {
        s.flags &= !flags::SAFE;
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::HARMLESS;
        data_mut(o).shielded_owner = false;
    }
    w.destroy(id);
}

/// Plays an explosion sound centred on the effects channel.
pub(crate) fn boom(w: &mut World, key: &str) {
    super::sound::play(w, key, SoundChannel::Effect);
}
