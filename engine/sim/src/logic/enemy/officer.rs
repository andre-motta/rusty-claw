//! The officer: a sword-fighting guard of the first levels.
//!
//! An officer stands idle, changing its pose once a second. When Claw
//! comes within 100 px across and 40 px up or down it goes Home, where it
//! decides at once: it turns to face him, sometimes taunts him (a third of
//! the time, when on screen), and strikes. The strike reaches 137 px in
//! front of it, high, or low while Claw crouches, and costs Claw the
//! officer's damage (10). When the strike ends it goes Home again, so with
//! Claw in range it keeps striking. Out of range it goes idle; after four
//! seconds idle it may start pacing (a fifth of the time, each tick), and
//! pacing it turns or stops at its patrol limits and goes Home when Claw
//! comes in range again.
//!
//! The rest is the shared human behaviour: flinching from blows, dying
//! (dropping its loot, its body flying off), being lifted and thrown by
//! Claw, and leaping at Claw from a narrow ledge. It has 3 health on the
//! first level (7 on later ones) and 10 damage.

use super::lift::Carry;
use super::states::{enter_strike, go_home, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyState, HUMAN_HIT_BY, Handler, Timer, build, data,
    data_mut, face_claw, human_fields, human_health, on_screen, rand, say, set_anim_now, set_state,
};
use crate::logic::World;
use crate::object::{ObjectId, Rect};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Officer"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_OFFICER"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_OFFICER"];

/// The stand poses, one picked at random each second.
pub const STAND: [&str; 5] = [
    "LEVEL_OFFICER_STAND1",
    "LEVEL_OFFICER_STAND2",
    "LEVEL_OFFICER_STAND3",
    "LEVEL_OFFICER_STAND4",
    "LEVEL_OFFICER_STAND5",
];
/// The high strike.
pub const STRIKE_HIGH_ANIM: &str = "LEVEL_OFFICER_STRIKE";
/// The low strike, used while Claw crouches.
pub const STRIKE_LOW_ANIM: &str = "LEVEL_OFFICER_STRIKE1";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_OFFICER_FALL";

/// The high strike box, facing left.
pub const STRIKE_HIGH: Rect = Rect::new(-137, -27, 0, -7);
/// The low strike box, facing left.
pub const STRIKE_LOW: Rect = Rect::new(-137, 20, 0, 40);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 51);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-48, -44, 42, 19);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The five range boxes: Claw within 100 px across and 40 px up or down.
pub const RANGES: [(i32, i32); 5] = [(100, 40); 5];

/// Damage of an officer's strike and touch.
pub const DAMAGE: i32 = 10;
/// Smarts base of an officer.
pub const SMARTS: i32 = 50;

/// Death sounds; the officer picks one when it is built.
pub const DEATH_SOUNDS: [&str; 2] = ["LEVEL_OFFICER_00340014", "LEVEL_OFFICER_00340015"];
/// Lines when lifted; the officer picks one each time.
pub const LIFTED_LINES: [&str; 2] = ["LEVEL_OFFICER_00340017", "LEVEL_OFFICER_00340018"];
/// The taunt before a strike.
pub const TAUNT: &str = "LEVEL_OFFICER_00340024";
/// The idle mutters.
pub const MUTTERS: [&str; 2] = ["LEVEL_OFFICER_EHEM1", "LEVEL_OFFICER_EHEM2"];

/// Time between stand poses, ms.
pub const POSE_MS: u64 = 1000;
/// Time idle before pacing may start, ms.
pub const PACE_WAIT_MS: u64 = 4000;

/// The officer class.
pub static CLASS: Class = Class {
    name: "Officer",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (10, -10, -45),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice,
    lift_roll: true,
    anims: ClassAnims {
        advance: "LEVEL_OFFICER_ADVANCE",
        fast_advance: "LEVEL_OFFICER_FASTADVANCE",
        pace: "LEVEL_OFFICER_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_OFFICER_HITLOW",
        hit_high: "LEVEL_OFFICER_HITHIGH",
        home: "LEVEL_OFFICER_HOME",
        jump: "LEVEL_OFFICER_JUMP",
        fall: "LEVEL_OFFICER_FALL",
        lifted: "LEVEL_OFFICER_LIFTED",
        lifted2: "LEVEL_OFFICER_LIFTED2",
        bounce: "LEVEL_OFFICER_THROWN",
    },
    sounds: ClassSounds {
        strike: "LEVEL_OFFICER_OFCSWSTB8-22",
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
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
        _ => None,
    }
}

/// Builds the officer: a death sound picked at random, its boxes, 3 health
/// on the first level (7 on later ones), 10 damage, its loot and patrol
/// limits from the level record; then it drops to the floor and goes
/// idle. Draws: the death sound's, then going idle's.
pub fn construct(w: &mut World, id: ObjectId) {
    let death_sound = DEATH_SOUNDS[usize::from(rand(w, 100) >= 50)];
    let fields = super::EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: STRIKE_HIGH,
        strike_anim: STRIKE_HIGH_ANIM,
        death_anim: DEATH_ANIM,
        death_sound,
        lifted_voice: LIFTED_LINES[0],
        smarts: super::smarts(SMARTS),
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: "LEVEL_OFFICER",
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

/// The officer's own pre-tick step: the strike is low while Claw crouches.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let ducking = w
        .claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK);
    if let Some(d) = data_mut(w, id) {
        if ducking {
            d.strike_rect = STRIKE_LOW;
            d.strike_anim = STRIKE_LOW_ANIM;
        } else {
            d.strike_rect = STRIKE_HIGH;
            d.strike_anim = STRIKE_HIGH_ANIM;
        }
    }
}

/// Picks the line said when lifted. Draws: one of 0..100.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    let line = LIFTED_LINES[usize::from(rand(w, 100) >= 50)];
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = line;
    }
}

/// Goes idle: stops, maybe mutters (a third of the time, when on screen),
/// takes the first stand pose and waits four seconds before pacing may
/// start. Draws: one of 0..100, then one of 0..2 when it mutters.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    if rand(w, 100) < 33 {
        let pick = rand(w, 2) as usize;
        if on_screen(w, id) {
            say(w, id, MUTTERS[pick]);
        }
    }
    set_state(w, id, EnemyState::Idle);
    set_anim_now(w, id, STAND[0]);
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, PACE_WAIT_MS);
    }
}

/// Idle: a new stand pose each second; Home when Claw is in range;
/// otherwise, after four seconds, a fifth of the time it starts pacing.
/// Draws: one of 0..5 for a new pose; one of 0..100 once pacing may start.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.pose.done(now)) {
        if let Some(d) = data_mut(w, id) {
            d.pose = Timer::new(now, POSE_MS);
        }
        let pick = rand(w, 5) as usize;
        set_anim_now(w, id, STAND[pick]);
    }
    let Some((mode, wait)) = data(w, id).map(|d| (d.mode, d.pace_wait)) else {
        return;
    };
    match mode {
        0..=4 => go_home(w, id),
        5 | 6 if wait.done(now) && rand(w, 100) < 20 => start_pacing(w, id),
        _ => {}
    }
}

/// Home: faces Claw; with Claw in range, maybe taunts him (a third of the
/// time, when on screen) and strikes; otherwise starts pacing. Draws: one
/// discarded, then one of 0..100 with Claw in range.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let _ = w.rng.next_u15();
    let mode = data(w, id).map_or(6, |d| d.mode);
    if mode <= 4 {
        if rand(w, 100) < 33 && on_screen(w, id) {
            say(w, id, TAUNT);
        }
        enter_strike(w, id);
    } else {
        start_pacing(w, id);
    }
}
