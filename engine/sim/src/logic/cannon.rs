//! Cannons: the tower cannons, which fire a cannonball every three
//! seconds from a fixed muzzle, and the small ground cannon a punk rat
//! sets down in front of itself and fires.
//!
//! A cannon is a fixed gun: it never moves, never collides (nothing can
//! hit or touch it) and never looks for Claw. Its first logic call builds
//! it:
//!
//! - its horizontal speed, the speed of its balls, is 500 px/s when the
//!   record gives none, and its depth 3990 when the record gives 0;
//! - a mirrored record turns its speed around before the muzzle is set
//!   (so a mirrored record still fires from the unmirrored muzzle, as in
//!   the original);
//! - the muzzle is 50 px ahead and 10 px below its position;
//! - a tower cannon on the left side of a tower then turns its speed
//!   around and moves its muzzle to its left: it fires left at 500 px/s
//!   from 50 px left of its position; one on the right fires right from
//!   50 px right of it. Both go Home;
//! - a ground cannon (created only by a punk rat, which builds it at once)
//!   stays as built, showing the first image of its set, until its punk
//!   rat fires it.
//!
//! State numbers ([`EnemyState`]): a cannon waits in `Home`, fires in
//! `Fire` and fires upwards in `VertFiring` (which no cannon of this
//! family uses). A ground cannon waits in [`CONSTRUCTED`] until its first
//! shot.
//!
//! - Home (tower cannons): the Home animation plays; whenever 3000 ms or
//!   more of game time have passed since the last shot was ordered (and
//!   at once on the first call), the cannon fires. The timer counts game
//!   time, so a cannon that slept fires on its first call after waking.
//!   It never tests where Claw is: it fires whether he is near or not.
//! - Home (a ground cannon): nothing.
//! - Fire: the firing animation plays; on the first call whose animation
//!   event is 1 one cannonball leaves the muzzle, at the cannon's depth
//!   less one, with the cannon's speed; when the animation ends the
//!   cannon goes Home.
//!
//! A tower cannon thus fires every 167 ticks of 18 ms (3006 ms), its ball
//! leaving on the seventh call of its firing animation. Its shot sounds
//! are the firing animation's frame sounds. No cannon draws from the
//! random stream.

use super::enemy::{EnemyState, Timer};
use super::projectile::{self, ProjectileSpec};
use super::{ObjectData, World};
use crate::level::runtime_image_set;
use crate::object::{ObjectId, draw_flags, engine_state, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TowerCannonLeft", "TowerCannonRight", "GroundCannon"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[
    GROUND_IMAGE,
    "LEVEL_TOWERCANNONLEFT",
    "LEVEL_TOWERCANNONRIGHT",
];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[
    "LEVEL_CANNON_",
    "LEVEL_TOWERCANNONLEFT_",
    "LEVEL_TOWERCANNONRIGHT_",
];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Logic name of the ground cannon.
pub const GROUND_CANNON: &str = "GroundCannon";
/// Image set of the ground cannon.
pub const GROUND_IMAGE: &str = "LEVEL_CANNON";
/// Logic name of the ball a cannon fires.
pub const BALL_LOGIC: &str = "CannonBall";

/// Speed of a cannon's balls when the record gives none, px/s.
pub const SPEED: i32 = 500;
/// Damage a cannon record is given when it has none. Its balls never use
/// it: every ball deals its own damage.
pub const DAMAGE: i32 = 10;
/// Depth of a cannon placed at depth 0.
pub const DEFAULT_Z: i32 = 3990;
/// Where a ball leaves, relative to the cannon, before any turn.
pub const MUZZLE: (i32, i32) = (50, 10);
/// Where a ball fired upwards leaves, relative to the cannon.
pub const VERTICAL_MUZZLE: (i32, i32) = (-26, -10);
/// Time between two shots of a tower cannon, ms of game time.
pub const PERIOD_MS: u64 = 3000;
/// Kinds of object a ball can be hit by (nothing reaches it: it has no
/// hit box).
pub const BALL_HIT_BY: u32 = 0x2;

/// State of a ground cannon built and waiting for its first shot.
pub const CONSTRUCTED: i32 = 1000;

/// The kinds of cannon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A tower cannon on the left side of a tower, firing left.
    TowerLeft,
    /// A tower cannon on the right side of a tower, firing right.
    TowerRight,
    /// The ground cannon a punk rat fires.
    Ground,
}

impl Kind {
    /// The kind running logic `name`.
    pub fn from_logic(name: &str) -> Option<Kind> {
        Some(match name {
            "TowerCannonLeft" => Kind::TowerLeft,
            "TowerCannonRight" => Kind::TowerRight,
            GROUND_CANNON => Kind::Ground,
            _ => return None,
        })
    }

    /// Its image set.
    pub fn image_set(self) -> &'static str {
        match self {
            Kind::TowerLeft => "LEVEL_TOWERCANNONLEFT",
            Kind::TowerRight => "LEVEL_TOWERCANNONRIGHT",
            Kind::Ground => GROUND_IMAGE,
        }
    }

    /// The animation it plays while waiting.
    pub fn home_anim(self) -> &'static str {
        match self {
            Kind::TowerLeft => "LEVEL_TOWERCANNONLEFT_HOME",
            Kind::TowerRight => "LEVEL_TOWERCANNONRIGHT_HOME",
            Kind::Ground => "LEVEL_CANNON_HOME",
        }
    }

    /// The animation it plays while firing.
    pub fn firing_anim(self) -> &'static str {
        match self {
            Kind::TowerLeft => "LEVEL_TOWERCANNONLEFT_FIRING",
            Kind::TowerRight => "LEVEL_TOWERCANNONRIGHT_FIRING",
            Kind::Ground => "LEVEL_CANNON_FIRING",
        }
    }

    /// Whether it fires on its own timer (the tower cannons).
    pub fn timed(self) -> bool {
        self != Kind::Ground
    }
}

/// A cannon's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CannonData {
    /// Its kind; `None` before its first call.
    pub kind: Option<Kind>,
    /// Where a ball leaves, relative to the cannon.
    pub muzzle: (i32, i32),
    /// Where a ball fired upwards leaves, relative to the cannon.
    pub vertical_muzzle: (i32, i32),
    /// Fires upwards (no cannon of this family does).
    pub vertical: bool,
    /// The shot timer of a tower cannon: the last shot was ordered at
    /// `start`, the next is due `len` ms later. Both start at 0, so the
    /// first call at Home fires.
    pub timer: Timer,
    /// The current firing animation has released its ball.
    pub shot: bool,
    /// Balls fired so far.
    pub fired: u32,
    /// The last ball fired.
    pub last_ball: Option<ObjectId>,
    /// The punk rat that set this ground cannon down and fires it.
    pub owner: Option<ObjectId>,
}

/// The data of cannon `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a CannonData> {
    match &w.objects.get(id)?.data {
        ObjectData::Cannon(d) => Some(d),
        _ => None,
    }
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut CannonData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Cannon(d) => Some(d),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(state) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    if state == engine_state::INIT {
        construct(w, id);
        return;
    }
    match EnemyState::from_id(state) {
        Some(EnemyState::Home) => home_tick(w, id),
        Some(EnemyState::Fire | EnemyState::VertFiring) => firing_tick(w, id),
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing can hit a cannon.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A cannon attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// Builds cannon `id` (see the module documentation). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let Some(kind) = w.objects.get(id).and_then(|o| Kind::from_logic(&o.logic)) else {
        return;
    };
    let first = w.image_frames(kind.image_set()).map(|f| f.0);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.image_set = runtime_image_set(kind.image_set());
    if let Some(f) = first {
        o.frame = f;
    }
    o.flags |= flags::NO_COLLIDE | flags::DESTROY_FAR;
    if o.speed_x == 0 {
        o.speed_x = SPEED;
    }
    if o.damage == 0 {
        o.damage = DAMAGE;
    }
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    o.state = CONSTRUCTED;
    o.data = ObjectData::Cannon(CannonData {
        kind: Some(kind),
        ..CannonData::default()
    });
    if o.draw_flags & draw_flags::MIRROR != 0 {
        // A mirrored record turns before its muzzle is set.
        mirror(w, id);
    }
    if let Some(d) = data_mut(w, id) {
        d.muzzle = MUZZLE;
        d.vertical_muzzle = VERTICAL_MUZZLE;
    }
    match kind {
        Kind::TowerLeft => {
            if let Some(o) = w.objects.get_mut(id) {
                o.speed_x = -o.speed_x;
            }
            if let Some(d) = data_mut(w, id) {
                d.muzzle.0 = -MUZZLE.0;
            }
            go_home(w, id);
        }
        Kind::TowerRight => go_home(w, id),
        Kind::Ground => {}
    }
}

/// Records `owner` as the punk rat that fires cannon `id`.
pub fn set_owner(w: &mut World, id: ObjectId, owner: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.owner = Some(owner);
    }
}

/// Turns cannon `id` around: an unmirrored cannon is mirrored with its
/// boxes; then, when it fires right, it fires left instead, from muzzles
/// moved to its other side.
pub fn mirror(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.draw_flags & draw_flags::MIRROR == 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
    if o.speed_x > 0 {
        o.speed_x = -o.speed_x;
        if let ObjectData::Cannon(d) = &mut o.data {
            d.muzzle.0 = -d.muzzle.0;
            d.vertical_muzzle.0 = -d.vertical_muzzle.0;
        }
    }
}

/// Starts animation `key` on `id`, or none when the level has no such
/// animation.
fn set_anim(w: &mut World, id: ObjectId, key: &str) {
    super::enemy::set_anim(w, id, key);
}

/// Goes Home with the Home animation (not advanced in this call).
pub fn go_home(w: &mut World, id: ObjectId) {
    let Some(kind) = data(w, id).and_then(|d| d.kind) else {
        return;
    };
    if let Some(o) = w.objects.get_mut(id) {
        o.state = EnemyState::Home as i32;
    }
    set_anim(w, id, kind.home_anim());
}

/// Fires: the firing animation starts (not advanced in this call) and the
/// ball is released by a later call. A punk rat fires its ground cannon
/// with this. Nothing happens for an object that is not a built cannon.
pub fn fire(w: &mut World, id: ObjectId) {
    let Some((kind, vertical)) = data(w, id).and_then(|d| Some((d.kind?, d.vertical))) else {
        return;
    };
    if let Some(d) = data_mut(w, id) {
        d.shot = false;
    }
    let state = if vertical {
        EnemyState::VertFiring
    } else {
        EnemyState::Fire
    };
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state as i32;
    }
    set_anim(w, id, kind.firing_anim());
}

/// Home: a tower cannon plays its Home animation and fires when its timer
/// has run out, restarting the timer; a ground cannon does nothing.
fn home_tick(w: &mut World, id: ObjectId) {
    let Some((kind, timer)) = data(w, id).and_then(|d| Some((d.kind?, d.timer))) else {
        return;
    };
    if !kind.timed() {
        return;
    }
    w.advance_anim(id);
    let now = w.now_ms;
    if timer.done(now) {
        if let Some(d) = data_mut(w, id) {
            d.timer = Timer::new(now, PERIOD_MS);
        }
        fire(w, id);
    }
}

/// Fire: the first call whose animation event is 1 releases the ball;
/// when the animation ends the cannon goes Home.
fn firing_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    if event == 1 && data(w, id).is_some_and(|d| !d.shot) {
        if let Some(d) = data_mut(w, id) {
            d.shot = true;
        }
        release(w, id);
    }
    if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
        go_home(w, id);
    }
}

/// Launches a cannonball from the muzzle at the cannon's speed, one depth
/// step behind the cannon.
fn release(w: &mut World, id: ObjectId) {
    let Some(d) = data(w, id) else {
        return;
    };
    let (mx, my) = if d.vertical {
        d.vertical_muzzle
    } else {
        d.muzzle
    };
    let Some((x, y, z, vx, vy)) = w
        .objects
        .get(id)
        .map(|o| (o.x, o.y, o.z, o.speed_x, o.speed_y))
    else {
        return;
    };
    let ball = projectile::spawn(
        w,
        ProjectileSpec {
            logic: BALL_LOGIC,
            x: x + mx,
            y: y + my,
            z: z - 1,
            vx,
            vy,
            damage: 0,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: BALL_HIT_BY,
            special_flags: 0,
            image_set: "",
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    if let Some(d) = data_mut(w, id) {
        d.fired += 1;
        d.last_ball = ball;
    }
}
