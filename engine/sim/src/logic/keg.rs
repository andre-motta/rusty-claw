//! Powder kegs: barrels Claw can lift, carry over his head and throw, and
//! that explode.
//!
//! A keg lying in the level is set off by a pistol shot or by a blast
//! reaching it, so one explosion can set off the kegs around it in a chain.
//! Claw's own blows do not touch it. Claw's side lifts it through
//! [`lifted`]: it follows his lifting frames, then rides over his head on
//! the side he faces. [`thrown`] sends it flying under gravity and
//! [`dropped`] lets it fall; either way it explodes when it lands, and a
//! thrown keg also explodes on the first enemy it strikes that can be hit
//! by thrown objects (it flies past the others, other kegs among them). The
//! strike itself lands as a thrown object's hit, with the keg's damage, and
//! the keg explodes right where it struck at the start of its next logic
//! call. Exploding at once would clear the keg's kind before the enemy's
//! hit handler reads it: the enemy would take the strike as a hit of no
//! known kind, doing no damage, and its rule of one hit per attack of
//! Claw's would then turn the blast away too. Until then the keg is still a
//! thrown object, so every enemy it overlaps in the pass of the strike
//! takes the strike once: as for any thrown object, only each enemy's own
//! record of Claw's attacks limits the hits it takes. A kill of kind
//! [`KILL_EXPLODE`] (see [`World::kill`]) also sets it off; any other kill
//! removes it without a blast.
//!
//! A keg shares the enemies' base: every logic call after its first starts
//! with their per-tick roll ([`enemy::pulse_roll`]), a draw from the shared
//! random stream on every third tick.
//!
//! Kegs rest on the floor at spawn: on its first logic call a keg is put
//! 32 px higher and then down onto the first floor below it, standing, the
//! way enemies are placed.
//!
//! Exploding plays the blast sound and the keg's explosion animation; when
//! the animation reaches its blast frame the keg requests one explosion
//! (damage 20) centred on the blast and a screen shake of
//! [`BLAST_SHAKE_MS`], and it is removed when the animation ends. A keg
//! explodes only once.

use claw_frame::SoundChannel;

use super::effect::{self, EffectKind};
use super::{ObjectData, World, enemy, sound};
use crate::level::runtime_image_set;
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags, special, types};
use crate::physics::ptype;
use crate::player::state as claw_state;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PowderKeg"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_POWDERKEG"];

/// Picture of a keg.
pub const IMAGE_SET: &str = "LEVEL_POWDERKEG";
/// Animation while being lifted.
pub const ANIM_LIFTED: &str = "LEVEL_POWDERKEG_LIFTED";
/// Animation while carried overhead.
pub const ANIM_OVERHEAD: &str = "LEVEL_POWDERKEG_LIFTED2";
/// Animation while flying.
pub const ANIM_THROWN: &str = "LEVEL_POWDERKEG_THROWN";
/// Animation of the explosion.
pub const ANIM_EXPLODE: &str = "LEVEL_POWDERKEG_EXPLODE";
/// Sound of the explosion.
pub const BLAST_SOUND: &str = "LEVEL_KEGBOOM";

/// Special flag: Claw lifts it from a crouch.
pub const LIFT_FROM_CROUCH: u32 = 0x4;
/// Damage of the explosion.
pub const BLAST_DAMAGE: i32 = 20;
/// Reach of the explosion, px.
pub const BLAST_RADIUS: i32 = 50;
/// Centre of the explosion relative to the keg (mirrored when it faces
/// left).
pub const BLAST_OFFSET: (i32, i32) = (5, 15);
/// Screen shake started by the blast, ms.
pub const BLAST_SHAKE_MS: u32 = 500;
/// Kind of kill (see [`World::kill`]) that makes a keg explode.
pub const KILL_EXPLODE: i32 = 2;
/// Event of the explosion animation's blast frame.
pub const BLAST_EVENT: i32 = 1;
/// Gravity on a flying keg, px/s/s.
pub const GRAVITY: f64 = 2000.0;
/// Box of a keg that shots hit, relative to its position (the picture
/// hangs below the position), when the level does not know the keg's
/// frame; otherwise the box of the level's own frame is used.
pub const KEG_BOX: Rect = Rect::new(-20, 22, 19, 69);
/// How far above the bottom of its picture box a keg's feet are, px: a
/// keg stands, and lands, with its feet on the floor.
pub const FEET_ABOVE_BOX: i32 = 9;
/// Where the keg sits while Claw lifts it, relative to him when he faces
/// right (mirrored when he faces left).
pub const LIFT_OFFSET: (i32, i32) = (30, -70);
/// Where the keg sits over Claw's head, relative to him when he faces
/// right (mirrored when he faces left).
pub const OVERHEAD_OFFSET: (i32, i32) = (-10, -105);
/// How far the keg leans back and down once Claw starts his throw.
pub const THROW_LEAN: (i32, i32) = (-4, 5);
/// Claw's frame in which he holds the keg halfway up.
pub const LIFT_FRAME: i32 = 42;
/// Claw's frame in which the keg reaches his head.
pub const OVERHEAD_FRAME: i32 = 43;

/// What a keg is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    /// Lying in the level.
    #[default]
    Idle,
    /// Being lifted.
    Lifted,
    /// Carried over the carrier's head.
    Overhead,
    /// Flying after a throw.
    Thrown,
    /// Falling after being let go.
    Dropped,
    /// Exploding.
    Exploding,
}

/// A keg's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KegData {
    /// What it is doing.
    pub phase: Phase,
    /// Who carries it.
    pub carrier: Option<ObjectId>,
    /// Horizontal speed, px/s.
    pub vx: f64,
    /// Vertical speed, px/s (down is positive).
    pub vy: f64,
    /// Sub-pixel movement carried to the next tick.
    pub rest: (f64, f64),
    /// The explosion it requested, once it has.
    pub blast: Option<ObjectId>,
    /// It has requested its explosion.
    pub blasted: bool,
    /// It struck an enemy in flight and explodes at its next logic call.
    pub struck: bool,
}

fn data_mut(o: &mut Object) -> &mut KegData {
    if !matches!(o.data, ObjectData::Keg(_)) {
        o.data = ObjectData::Keg(KegData::default());
    }
    match &mut o.data {
        ObjectData::Keg(d) => d,
        _ => unreachable!("set above"),
    }
}

/// The keg data of `o`, if it has any.
pub fn data_of(o: &Object) -> Option<&KegData> {
    match &o.data {
        ObjectData::Keg(d) => Some(d),
        _ => None,
    }
}

/// Logic states of a keg (its phase is in [`KegData`]).
pub mod state {
    /// First call.
    pub const INIT: i32 = 0;
    /// Set up.
    pub const READY: i32 = 5;
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state == state::INIT {
        init(w, id);
        return;
    }
    if matches!(o.state, engine_state::DESTROYING | engine_state::SLEEPING) {
        return;
    }
    // Every later call starts with the enemies' per-tick roll.
    enemy::pulse_roll(w);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state == engine_state::KILLED {
        let kind = o.kill_kind;
        o.state = state::READY;
        if data_mut(o).phase == Phase::Exploding {
            return;
        }
        if kind == KILL_EXPLODE {
            explode(w, id);
        } else {
            o.flags |= flags::NO_COLLIDE | flags::SAFE;
            w.destroy(id);
        }
        return;
    }
    if o.state != state::READY {
        return;
    }
    let d = data_mut(o);
    match d.phase {
        Phase::Idle => {}
        Phase::Lifted | Phase::Overhead => carry(w, id),
        Phase::Thrown if d.struck => explode(w, id),
        Phase::Thrown | Phase::Dropped => fly(w, id),
        Phase::Exploding => exploding(w, id),
    }
}

/// The box shots hit on keg `o`: the box of its frame in the level, or
/// [`KEG_BOX`] when the level does not know the frame.
pub fn hit_box(w: &World, o: &Object) -> Rect {
    let frame = if o.frame < 0 {
        w.image_frames(&o.image_set)
            .map_or(o.frame, |(first, _)| first)
    } else {
        o.frame
    };
    w.frame_box(&o.image_set, frame).unwrap_or(KEG_BOX)
}

/// The box a keg with hit box `b` stands and lands with: its bottom is
/// [`FEET_ABOVE_BOX`] px higher.
pub const fn move_box(b: Rect) -> Rect {
    Rect::new(b.left, b.top, b.right, b.bottom - FEET_ABOVE_BOX)
}

/// First logic call: sets the keg up, and kegs rest on the floor at spawn
/// (32 px higher, then down onto the first floor below, standing).
fn init(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.image_set.is_empty() {
        o.image_set = runtime_image_set(IMAGE_SET);
    }
    let o = w.objects.get(id).expect("checked");
    let b = hit_box(w, o);
    let o = w.objects.get_mut(id).expect("checked");
    o.damage = BLAST_DAMAGE;
    o.special_flags |= special::LIFTABLE | LIFT_FROM_CROUCH;
    o.type_flags = types::ENEMY;
    o.hit_type_flags = types::PISTOL_SHOT | types::LEVEL_SHOT;
    o.hit_handler = true;
    o.hit_rect = b;
    o.move_rect = move_box(b);
    *data_mut(o) = KegData::default();
    o.state = state::READY;
    enemy::drop_to_floor(w, id);
}

/// `attacker` hit `id`: a pistol shot (which is used up) or a blast sets
/// off a keg lying in the level.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != state::READY || data_mut(o).phase != Phase::Idle {
        return;
    }
    o.flags |= flags::SAFE;
    if w.objects
        .get(attacker)
        .is_some_and(|a| a.type_flags == types::PISTOL_SHOT)
    {
        w.destroy(attacker);
    }
    explode(w, id);
}

/// `id` attacked `victim`: a thrown keg explodes on the first enemy it
/// strikes that can be hit by thrown objects. The strike's hit on the
/// enemy follows this call and needs the keg as it is (a thrown object
/// with its damage), so the keg only stops attacking here, stops where it
/// is, and explodes at its next logic call. Other enemies it overlaps in
/// the same pass still take its hit as a thrown object.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let accepts = w
        .objects
        .get(victim)
        .is_some_and(|v| v.hit_type_flags & types::THROWN != 0);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if accepts && data_mut(o).phase == Phase::Thrown {
        o.attack_type_flags = 0;
        o.attack_handler = false;
        data_mut(o).struck = true;
    }
}

/// `carrier` lifted `id`: it stops being hittable and follows the
/// carrier's lifting frames.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    if w.objects.get(id).is_some_and(|o| o.state == state::INIT) {
        init(w, id);
    }
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if data_mut(o).phase == Phase::Exploding {
        return;
    }
    o.physics_type = ptype::DIRECT;
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    let d = data_mut(o);
    d.phase = Phase::Lifted;
    d.carrier = Some(carrier);
    w.set_anim(id, ANIM_LIFTED);
    carry(w, id);
}

/// `id` was thrown at (vx, vy) px/s: it flies under gravity, strikes
/// enemies as a thrown object, and explodes on the first one it strikes or
/// when it lands.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    if w.objects.get(id).is_some_and(|o| o.state == state::INIT) {
        init(w, id);
    }
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if data_mut(o).phase == Phase::Exploding {
        return;
    }
    o.physics_type = ptype::FLY;
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    o.type_flags = types::THROWN;
    o.attack_type_flags = types::ENEMY;
    o.attack_handler = true;
    o.attack_rect = o.hit_rect;
    let d = data_mut(o);
    d.phase = Phase::Thrown;
    d.carrier = None;
    d.vx = f64::from(vx);
    d.vy = f64::from(vy);
    d.rest = (0.0, 0.0);
    d.struck = false;
    w.set_anim(id, ANIM_THROWN);
}

/// `id` was put down or let go: it falls and explodes when it lands.
pub fn dropped(w: &mut World, id: ObjectId) {
    if w.objects.get(id).is_some_and(|o| o.state == state::INIT) {
        init(w, id);
    }
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if data_mut(o).phase == Phase::Exploding {
        return;
    }
    o.physics_type = ptype::FLY;
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    let d = data_mut(o);
    d.phase = Phase::Dropped;
    d.carrier = None;
    d.vx = 0.0;
    d.vy = 0.0;
    d.rest = (0.0, 0.0);
}

/// Follows the carrier: halfway up in its lifting frame, then over its
/// head, leaning back once it starts to throw.
fn carry(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).expect("caller checked");
    let Some(carrier) = data_of(o).and_then(|d| d.carrier) else {
        return dropped(w, id);
    };
    let Some(c) = w.objects.get(carrier) else {
        return dropped(w, id);
    };
    let right = if Some(carrier) == w.claw_id {
        w.claw.facing_right
    } else {
        c.draw_flags & draw_flags::MIRROR == 0
    };
    let (cx, cy, frame, throwing) = (c.x, c.y, c.frame, c.state == claw_state::THROW);
    let side = if right { 1 } else { -1 };
    let o = w.objects.get_mut(id).expect("checked");
    let phase = data_mut(o).phase;
    if phase == Phase::Lifted && frame == OVERHEAD_FRAME {
        data_mut(o).phase = Phase::Overhead;
        w.set_anim(id, ANIM_OVERHEAD);
    }
    let place = match phase {
        Phase::Lifted if frame == OVERHEAD_FRAME => Some(OVERHEAD_OFFSET),
        Phase::Lifted if frame == LIFT_FRAME => Some(LIFT_OFFSET),
        Phase::Lifted => None,
        _ if throwing => Some((
            OVERHEAD_OFFSET.0 + THROW_LEAN.0,
            OVERHEAD_OFFSET.1 + THROW_LEAN.1,
        )),
        _ => Some(OVERHEAD_OFFSET),
    };
    let o = w.objects.get_mut(id).expect("checked");
    if right {
        o.draw_flags &= !draw_flags::MIRROR;
    } else {
        o.draw_flags |= draw_flags::MIRROR;
    }
    if let Some((dx, dy)) = place {
        o.x = cx + side * dx;
        o.y = cy + dy;
    }
    w.advance_anim(id);
}

/// Flight after a throw or a drop: explodes on landing.
fn fly(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = w.objects.get_mut(id).expect("caller checked");
    let mut d = data_mut(o).clone();
    let dt = f64::from(TICK_MS) / 1000.0;
    d.vy += GRAVITY * dt;
    let fx = d.vx * dt + d.rest.0;
    let fy = d.vy * dt + d.rest.1;
    let (dx, dy) = (fx.trunc(), fy.trunc());
    d.rest = (fx - dx, fy - dy);
    let (tx, ty) = (o.x + dx as i32, o.y + dy as i32);
    *data_mut(o) = d;
    w.physics(id, tx, ty, 0);
    let o = w.objects.get_mut(id).expect("checked");
    if o.physics_type == ptype::STAND {
        explode(w, id);
    }
}

/// Starts the explosion: blast sound and animation; the blast itself comes
/// with the animation's blast frame (at once when the level has no such
/// animation).
fn explode(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if data_mut(o).phase == Phase::Exploding {
        return;
    }
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    o.type_flags = 0;
    o.attack_type_flags = 0;
    o.attack_handler = false;
    o.physics_type = ptype::DIRECT;
    let d = data_mut(o);
    d.phase = Phase::Exploding;
    d.carrier = None;
    sound::play_from(w, id, BLAST_SOUND, SoundChannel::Effect);
    if !w.set_anim(id, ANIM_EXPLODE) {
        blast(w, id);
        w.destroy(id);
    }
}

fn exploding(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    let o = w.objects.get_mut(id).expect("caller checked");
    let done = o.anim.is_done();
    if event == BLAST_EVENT || done {
        blast(w, id);
    }
    if done {
        w.destroy(id);
    }
}

/// Requests the keg's one explosion.
fn blast(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let mirrored = o.draw_flags & draw_flags::MIRROR != 0;
    let (x, y, damage) = (o.x, o.y, o.damage);
    let d = data_mut(o);
    if d.blasted {
        return;
    }
    d.blasted = true;
    let dx = if mirrored {
        -BLAST_OFFSET.0
    } else {
        BLAST_OFFSET.0
    };
    let kind = EffectKind::Explosion {
        damage,
        radius: BLAST_RADIUS,
        source: Some(id),
    };
    let blast = effect::spawn(w, kind, x + dx, y + BLAST_OFFSET.1);
    w.shake(BLAST_SHAKE_MS);
    if let Some(o) = w.objects.get_mut(id) {
        data_mut(o).blast = blast;
    }
}
