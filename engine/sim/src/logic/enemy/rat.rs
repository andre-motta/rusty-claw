//! The rat: a small bomb-throwing rodent of the early levels.
//!
//! A rat walks back and forth at 60 px/s between its patrol limits (or a
//! ledge or a wall), without turning its sprite. Whenever it sees Claw
//! (he is less than 50 px above or below it, it is on screen and no wall
//! stands between them on its row) it faces him and, at most once a
//! second, stops to lob a bomb horizontally at him (500 px/s). While that
//! second runs, with Claw just below it (less than 40 px across), it
//! drops a bomb straight down instead, almost every tick it can (its
//! smarts, 99 in 100). After a throw it goes Home, grunts, and walks on:
//! right when it was walking right, left otherwise. A rat placed as
//! stationary never walks; it only throws.
//!
//! Any blow kills it at once (a pistol shot is used up), with no flinch;
//! its body flies off like an officer's. It drops nothing and cannot be
//! lifted. It has 1 health and its touch costs Claw 5; it touches him only
//! at a single point, its position ([`TOUCH_BOX`]).

use super::states::{move_left, move_right, walk_left_tick, walk_right_tick};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, EnemyFields, EnemyState, Handler, Shot, Timer,
    accept_hit, attacker, build, claw_invisible, data, data_mut, die, face_claw, field, obj,
    obj_mut, play, rand, record, record_bounds, record_bounds_y, sees_claw, set_anim, set_state,
};
use crate::logic::World;
use crate::logic::projectile::{self, ProjectileSpec};
use crate::object::{ObjectId, Rect, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Rat"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_RAT"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_RAT"];

/// Walking.
pub const WALK_ANIM: &str = "LEVEL_RAT_WALK";
/// Throwing a bomb down.
pub const THROW_DOWN_ANIM: &str = "LEVEL_RAT_THROWSOUTH";
/// Throwing a bomb across.
pub const THROW_ACROSS_ANIM: &str = "LEVEL_RAT_THROWEASTWEST";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_RAT_DEAD";

/// The grunt when it goes Home.
pub const HOME_SOUND: &str = "GAME_EEHH";
/// The death sound.
pub const DEATH_SOUND: &str = "GAME_MOUSEKILL";

/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-10, 20, 10, 33);
/// Hit box when the level does not know the rat's picture: the box of a
/// 30 x 28 px picture standing on the rat's feet.
pub const FALLBACK_BOX: Rect = Rect::new(-15, 6, 14, 33);
/// Touch box: the single pixel at the rat's position. Claw is hurt by
/// touching a rat only when his own box reaches that point, not when it
/// meets the rat's picture. Provisional: the original's rats keep this box
/// in every state they were seen in, but what gives it to them is not known
/// yet.
pub const TOUCH_BOX: Rect = Rect::new(0, 0, 0, 0);
/// The frame a rat shows when it is placed, when its image set has it.
pub const START_FRAME: i32 = 12;
/// Kinds of object that can hit a rat: Claw, his pistol shots, the
/// level's own shots and explosions, and his magic.
pub const HIT_BY: u32 = 0x300_0022;

/// Health of a rat.
pub const HEALTH: i32 = 1;
/// Damage of a rat's touch.
pub const DAMAGE: i32 = 5;
/// Smarts base of a rat.
pub const SMARTS: i32 = 10;
/// Depth base of the depth against Claw.
pub const Z_BASE: i32 = 5;
/// Walking speed, px/s.
pub const WALK_SPEED: f64 = super::states::WALK_SPEED;

/// Logic of a bomb.
pub const BOMB_LOGIC: &str = "RatBomb";
/// Image set of a bomb.
pub const BOMB_IMAGE: &str = "LEVEL_RATBOMB";
/// Damage of a bomb's explosion.
pub const BOMB_DAMAGE: i32 = 15;
/// Special flag of a bomb thrown across: it goes off on Claw.
pub const BOMB_ACROSS: u32 = 0x20;
/// Least time between throws across, ms.
pub const THROW_WAIT_MS: u64 = 1000;
/// How far across Claw may be for a bomb dropped on him, px (exclusive).
pub const DROP_REACH: i32 = 40;
/// How far up or down Claw may be for a bomb thrown across, px
/// (exclusive).
pub const THROW_HEIGHT: i32 = 50;

/// A bomb dropped straight down.
pub const THROW_DOWN: Shot = Shot {
    dx: 1,
    dy: -12,
    vx: 0,
    vy: 250,
};
/// A bomb thrown to the left.
pub const THROW_LEFT: Shot = Shot {
    dx: -13,
    dy: -22,
    vx: -500,
    vy: 0,
};
/// A bomb thrown to the right.
pub const THROW_RIGHT: Shot = Shot {
    dx: 13,
    dy: -22,
    vx: 500,
    vy: 0,
};

/// The rat class.
pub static CLASS: Class = Class {
    name: "Rat",
    human: false,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle,
    carry: None,
    pick_lift_voice: super::nothing,
    anims: ClassAnims {
        advance: "",
        fast_advance: "",
        pace: "",
        walk: WALK_ANIM,
        hit_low: "",
        hit_high: "",
        home: "",
        jump: "",
        fall: "",
        lifted: "",
        lifted2: "",
        bounce: "",
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["", ""],
        hit_high: "",
    },
};

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id, &CLASS);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::MoveLeft => Some(move_left_tick),
        EnemyState::MoveRight => Some(move_right_tick),
        EnemyState::Throw => Some(throw_tick),
        _ => None,
    }
}

/// Builds the rat: 1 health, 5 damage, its patrol limits (across and up
/// and down) and whether it walks from the level record, its hit box from
/// its first picture and its one-pixel touch box; it goes Home (grunting)
/// and drops to the floor. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        bounds: record_bounds(w, id),
        bounds_y: record_bounds_y(w, id),
        stationary: record(w, id, field::USER_0) > 0,
        ledge_guard: true,
        z_sort: true,
        z_base: Z_BASE,
        may_lift: true,
        smarts: super::smarts(SMARTS),
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        ..EnemyFields::default()
    };
    let frames = w.image_frames("LEVEL_RAT");
    let first = frames.map(|f| f.0);
    let frame_box = first
        .and_then(|f| w.frame_box("LEVEL_RAT", f))
        .unwrap_or(FALLBACK_BOX);
    let o = obj_mut(w, id);
    if o.speed_x == 0 {
        o.speed_x = WALK_SPEED as i32;
    }
    if let Some((lo, hi)) = frames {
        o.frame = if (lo..=hi).contains(&START_FRAME) {
            START_FRAME
        } else {
            lo
        };
    }
    let body = Body {
        image_set: "LEVEL_RAT",
        move_rect: MOVE_RECT,
        hit_rect: frame_box,
        attack_rect: TOUCH_BOX,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HIT_BY,
    };
    build(w, id, body, fields, false);
    go_home(w, id);
}

/// Goes Home: stops and grunts.
pub fn go_home(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Home);
    play(w, HOME_SOUND);
}

/// Goes idle (a rat does nothing there).
pub fn go_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Idle);
}

/// Home: a stationary rat only throws; any other walks on right after
/// walking right, left after anything else.
pub fn home_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some((stationary, prev)) = data(w, id).map(|d| (d.stationary, d.prev)) else {
        return;
    };
    if stationary {
        throw_check(w, id);
        return;
    }
    if prev == EnemyState::MoveRight {
        move_right(w, id, WALK_SPEED);
    } else {
        move_left(w, id, WALK_SPEED);
    }
}

/// MoveLeft: the throw check, then walking.
pub fn move_left_tick(w: &mut World, id: ObjectId) {
    if !throw_check(w, id) {
        walk_left_tick(w, id);
    }
}

/// MoveRight: the throw check, then walking.
pub fn move_right_tick(w: &mut World, id: ObjectId) {
    if !throw_check(w, id) {
        walk_right_tick(w, id);
    }
}

/// When the rat sees Claw it faces him; then it throws a bomb across at
/// him when a second has passed since its last one, or else, with Claw
/// below it less than 40 px across (on a frame pulse tick only), it
/// drops one on him when a draw is below its smarts. True when it started
/// a throw. Draws: one of 0..100 for a drop only.
pub fn throw_check(w: &mut World, id: ObjectId) -> bool {
    if claw_invisible(w) || !sees_claw(w, id) {
        return false;
    }
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return false;
    };
    face_claw(w, id);
    let now = w.now_ms;
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let Some((ready, right, smarts)) =
        data(w, id).map(|d| (d.fire_wait.done(now), d.claw_right, d.smarts))
    else {
        return false;
    };
    if (y - cy).abs() < THROW_HEIGHT && ready {
        if let Some(d) = data_mut(w, id) {
            d.fire_wait = Timer::new(now, THROW_WAIT_MS);
        }
        let shot = if right { THROW_RIGHT } else { THROW_LEFT };
        start_throw(w, id, THROW_ACROSS_ANIM, shot);
        return true;
    }
    if (x - cx).abs() < DROP_REACH && cy > y && w.pulse && rand(w, 100) < smarts {
        start_throw(w, id, THROW_DOWN_ANIM, THROW_DOWN);
        return true;
    }
    false
}

/// Stops and starts a throw with `anim`, loading `shot`.
pub fn start_throw(w: &mut World, id: ObjectId, anim: &str, shot: Shot) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.shots = 0;
        d.shot = shot;
    }
    set_state(w, id, EnemyState::Throw);
    set_anim(w, id, anim);
}

/// Throw: on the animation's throw frame the bomb leaves (once per
/// throwing animation); Home when the animation ends.
pub fn throw_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    if event == 1 {
        let first = data(w, id).is_some_and(|d| d.shots == 0);
        if let Some(d) = data_mut(w, id) {
            d.shots += 1;
        }
        if first {
            release(w, id);
        }
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

/// Launches the loaded bomb.
fn release(w: &mut World, id: ObjectId) {
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
            logic: BOMB_LOGIC,
            x: x + shot.dx,
            y: y + shot.dy,
            z: z - 1,
            vx: shot.vx,
            vy: shot.vy,
            damage: BOMB_DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: if shot.vx != 0 { BOMB_ACROSS } else { 0 },
            image_set: BOMB_IMAGE,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
}

/// `attacker` hit the rat: the same attack of Claw hits only once; any
/// other hit kills it (again, for a body), using up a pistol shot. Draws:
/// the death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if !accept_hit(w, id) {
        return;
    }
    if w.objects
        .get(attacker)
        .is_some_and(|a| a.type_flags == attacker::PISTOL_SHOT)
    {
        w.destroy(attacker);
    }
    die(w, id, DeathKind::Hit, Some(attacker));
}
