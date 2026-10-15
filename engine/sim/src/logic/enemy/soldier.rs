//! The soldier: a musket-carrying guard of the early levels.
//!
//! A soldier goes by sight: it notices Claw only when it can see him (he
//! is less than 50 px above or below it, the soldier is on screen and no
//! wall stands between them on the soldier's row). Until then it stands
//! idle, switching between two poses once a second, and after four
//! seconds may start pacing (one time in twenty, each tick).
//!
//! When it sees Claw it goes Home and decides at once, facing him:
//!
//! - within 80 px across (and 50 px up or down) it clubs him with the
//!   musket butt (the shared strike, 77 px in front of it, 10 damage);
//! - closer than 300 px it shouts its alert the first time (once per
//!   soldier), going idle instead when it lost sight of him;
//! - whenever it sees him and its gun is ready (500 ms after the last
//!   shot), it fires: on the firing animation's shot frame a musket ball
//!   leaves 65 px in front of it and 38 px up, flying at 500 px/s toward
//!   him, once per firing animation;
//! - when it does not see him it falls back on the shared human Home: it
//!   walks or runs toward him, or paces.
//!
//! The rest is the shared human behaviour, as for the officer. It has 3
//! health on the first level (7 on later ones) and 10 damage; its musket
//! ball always deals 10.

use super::lift::Carry;
use super::states::{enter_strike, go_home, human_home_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyState, HUMAN_HIT_BY, Handler, STARTUP_QUIET_MS,
    Shot, Timer, build, data, data_mut, face_claw, faces_right, human_fields, human_health, obj,
    on_screen, rand, say, sees_claw, set_anim, set_anim_now, set_state,
};
use crate::logic::World;
use crate::logic::projectile::{self, ProjectileSpec};
use crate::object::{ObjectId, Rect, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Soldier"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_SOLDIER"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SOLDIER"];

/// The stand poses, one picked at random each second.
pub const STAND: [&str; 2] = ["LEVEL_SOLDIER_STAND", "LEVEL_SOLDIER_STAND1"];
/// The musket-butt strike.
pub const STRIKE_ANIM: &str = "LEVEL_SOLDIER_STRIKE1";
/// Firing.
pub const FIRE_ANIM: &str = "LEVEL_SOLDIER_STRIKE";
/// Firing low, at a crouching Claw (second level only).
pub const FIRE_LOW_ANIM: &str = "LEVEL_SOLDIER_STRIKE2";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_SOLDIER_FALL";

/// The musket-butt strike box, facing left.
pub const STRIKE: Rect = Rect::new(-77, -50, 0, -30);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-11, -60, 11, 50);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-41, -23, 43, 31);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-15, -50, 15, 50);
/// The five range boxes: firing at 400 and 300 px, striking at 80 px,
/// all 50 px up or down.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (300, 50), (300, 50), (80, 50), (80, 50)];

/// Damage of a soldier's strike and touch.
pub const DAMAGE: i32 = 10;
/// Smarts base of a soldier.
pub const SMARTS: i32 = 25;

/// The death sound.
pub const DEATH_SOUND: &str = "LEVEL_SOLDIER_00320006";
/// The line when lifted.
pub const LIFTED_LINE: &str = "LEVEL_SOLDIER_00340016";
/// The cry when hit, low or high.
pub const HIT_CRY: &str = "LEVEL_SOLDIER_00320005";
/// The musket butt landing.
pub const STRIKE_SOUND: &str = "LEVEL_SOLDIER_RFHEADBT8-22";
/// The gunshot.
pub const GUNSHOT: &str = "LEVEL_SOLDIER_SOLGUNFR8-22";
/// The alert, shouted the first time it sees Claw close.
pub const ALERT: &str = "LEVEL_SOLDIER_00320020";

/// Logic of the musket ball.
pub const BALL_LOGIC: &str = "Bullet";
/// Image set of the musket ball.
pub const BALL_IMAGE: &str = "LEVEL_MUSKETBALL";
/// Damage of the musket ball, whatever the difficulty.
pub const BALL_DAMAGE: i32 = 10;
/// Speed of the musket ball, px/s.
pub const BALL_SPEED: i32 = 500;
/// Where the ball leaves, in front of the soldier and up.
pub const MUZZLE: (i32, i32) = (65, -38);
/// Where the low ball leaves (second level, Claw crouching): in front and
/// down.
pub const MUZZLE_LOW: (i32, i32) = (65, 10);
/// Least time between shots, ms.
pub const FIRE_WAIT_MS: u64 = 500;

/// Time between stand poses, ms.
pub const POSE_MS: u64 = 1000;
/// Time idle before pacing may start, ms.
pub const PACE_WAIT_MS: u64 = 4000;
/// Chance per tick of starting to pace, percent.
pub const PACE_CHANCE: i32 = 5;

/// The soldier class.
pub static CLASS: Class = Class {
    name: "Soldier",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (0, 0, -65),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice: super::nothing,
    anims: ClassAnims {
        advance: "LEVEL_SOLDIER_ADVANCE",
        fast_advance: "LEVEL_SOLDIER_FASTADVANCE",
        pace: "LEVEL_SOLDIER_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_SOLDIER_HITLOW",
        hit_high: "LEVEL_SOLDIER_HITHIGH",
        home: "LEVEL_SOLDIER_HOME",
        jump: "LEVEL_SOLDIER_JUMP",
        fall: "LEVEL_SOLDIER_FALL",
        lifted: "LEVEL_SOLDIER_LIFTED",
        lifted2: "LEVEL_SOLDIER_LIFTED2",
        bounce: "LEVEL_SOLDIER_THROWN",
    },
    sounds: ClassSounds {
        strike: STRIKE_SOUND,
        hit_low: [HIT_CRY, super::HIT_LOW_SECOND],
        hit_high: HIT_CRY,
    },
};

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id, &CLASS);
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    super::on_hit(w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Fire => Some(fire_tick),
        _ => None,
    }
}

/// Builds the soldier: its boxes, going by sight, 3 health on the first
/// level (7 on later ones), 10 damage, its loot and patrol limits from the
/// level record; then it drops to the floor and goes idle. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = super::EnemyFields {
        ranges: RANGES,
        sight: true,
        contact_rect: CONTACT,
        strike_rect: STRIKE,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: LIFTED_LINE,
        smarts: super::smarts(SMARTS),
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: "LEVEL_SOLDIER",
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: human_health(w),
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    go_idle(w, id);
}

/// Goes idle: stops, takes the first stand pose and waits four seconds
/// before pacing may start. Draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    set_anim(w, id, STAND[0]);
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, PACE_WAIT_MS);
    }
    w.advance_anim(id);
}

/// Idle: a new stand pose each second; Home when it sees Claw; otherwise,
/// after four seconds (and not while the level starts), one time in twenty
/// it starts pacing. Draws: one of 0..2 for a new pose; one of 0..100
/// once pacing may start.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.pose.done(now)) {
        if let Some(d) = data_mut(w, id) {
            d.pose = Timer::new(now, POSE_MS);
        }
        let pick = rand(w, 2) as usize;
        set_anim_now(w, id, STAND[pick]);
    }
    if sees_claw(w, id) {
        go_home(w, id);
        return;
    }
    let wait = data(w, id).map_or(Timer::default(), |d| d.pace_wait);
    if wait.done(now) && now >= STARTUP_QUIET_MS && rand(w, 100) < PACE_CHANCE {
        start_pacing(w, id);
    }
}

/// Home: faces Claw; within striking range (modes 3, 4) it strikes;
/// within 300 px (modes 0 to 2) it goes idle when it lost sight of Claw
/// and shouts its alert the first time (on screen); then, seeing Claw, it
/// fires when its gun is ready, and otherwise it falls back on the shared
/// human Home (which does nothing once it went idle). Draws: one
/// discarded, then the shared Home's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let _ = w.rng.next_u15();
    let mode = data(w, id).map_or(6, |d| d.mode);
    match mode {
        3 | 4 => {
            enter_strike(w, id);
            return;
        }
        0..=2 => {
            if !sees_claw(w, id) {
                go_idle(w, id);
            }
            if data(w, id).is_some_and(|d| !d.alerted) {
                if on_screen(w, id) {
                    say(w, id, ALERT);
                }
                if let Some(d) = data_mut(w, id) {
                    d.alerted = true;
                }
            }
        }
        _ => {}
    }
    if sees_claw(w, id) {
        fire_check(w, id);
    } else {
        human_home_tick(w, id);
    }
}

/// Fires when the gun is ready (500 ms after the last shot; the first
/// shot at once).
pub fn fire_check(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.fire_wait.done(now)) {
        enter_fire(w, id);
        if let Some(d) = data_mut(w, id) {
            d.fire_wait = Timer::new(now, FIRE_WAIT_MS);
        }
    }
}

/// Starts firing: faces Claw and loads the shot toward him (low on the
/// second level when he crouches).
pub fn enter_fire(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Fire);
    face_claw(w, id);
    let low = w.level_number() == 2
        && w.claw_object()
            .is_some_and(|c| c.physics_type == ptype::DUCK);
    let right = faces_right(obj(w, id));
    let (dx, dy) = if low { MUZZLE_LOW } else { MUZZLE };
    let shot = Shot {
        dx: if right { dx } else { -dx },
        dy,
        vx: if right { BALL_SPEED } else { -BALL_SPEED },
        vy: 0,
    };
    set_anim(w, id, if low { FIRE_LOW_ANIM } else { FIRE_ANIM });
    if let Some(d) = data_mut(w, id) {
        d.shot = shot;
        d.shots = 0;
    }
}

/// Fire: on the animation's shot frame the musket ball leaves (once per
/// firing animation) with the gunshot; Home when the animation ends.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    if event == 1 && data(w, id).is_some_and(|d| d.shots == 0) {
        if let Some(d) = data_mut(w, id) {
            d.shots = 1;
        }
        fire(w, id);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

/// Launches the musket ball and plays the gunshot.
fn fire(w: &mut World, id: ObjectId) {
    let Some(shot) = data(w, id).map(|d| d.shot) else {
        return;
    };
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    projectile::spawn(
        w,
        ProjectileSpec {
            logic: BALL_LOGIC,
            x: x + shot.dx,
            y: y + shot.dy,
            z: z - 1,
            vx: shot.vx,
            vy: shot.vy,
            damage: BALL_DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: BALL_IMAGE,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    super::play(w, GUNSHOT);
}
