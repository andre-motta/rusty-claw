//! The cutthroat: a human enemy of the third and fourth levels who slashes
//! at Claw up close and throws knives at him from afar. He never walks up
//! to Claw.
//!
//! He goes by sight, as the soldier does: he sees Claw only when Claw is
//! not invisible, is less than 50 px above or below him, the cutthroat is
//! on screen, and no wall stands between them on the cutthroat's row.
//!
//! - Idle, he changes between four stand poses once a second. Whenever
//!   Claw is inside one of his range boxes (modes 0 to 4: 400, 300 and
//!   200 px across for the outer three, 75 and 50 px for the inner two,
//!   all 50 px up or down) he goes Home at once; with Claw out of range
//!   (modes 5, 6), after four seconds idle, he starts pacing one time in
//!   five each tick.
//! - Home, he faces Claw and draws one of 0..121. With Claw within his
//!   inner boxes (modes 3, 4) he slashes with his sword when the draw is
//!   below his smarts (99): the shared strike, 75 px in front of him and
//!   from his origin down 27 px, 10 damage, silent. With Claw in the outer
//!   boxes (modes 0 to 2) he goes idle when he cannot see Claw (so with
//!   Claw hidden there he goes Home and idle on alternate ticks) and
//!   shouts his alert the first time (once per cutthroat, on screen).
//!   Then, seeing Claw, he throws a knife when his throw is ready; not
//!   seeing him and still at Home, he falls back on the shared human Home
//!   (strike or pause up close, pace far away).
//! - The knife leaves on the throw animation's shot frame, 65 px ahead of
//!   him and 15 px up at a standing Claw, or 20 px down (with the low
//!   throw) at a crouching one: a `Bullet` drawn from [`KNIFE_IMAGE`],
//!   400 px/s flat, 10 damage whatever the difficulty.
//! - One timer serves both the stand poses and the throws: a new pose
//!   waits a second, a throw two seconds on the third level (one and a
//!   half on others). A throw ready while he strikes replaces the strike
//!   in the same tick, and he stays busy until a later strike or flinch
//!   ends.
//!
//! The rest is the shared human behaviour: flinches, death and loot,
//! being lifted and thrown, the leap at Claw from a narrow ledge. He never
//! blocks or dodges. He has 7 health on the third level (9 on others) and
//! 10 damage, times the difficulty factors.
//!
//! State numbers ([`EnemyState`]): one number per state name, whichever
//! class runs it, so the cutthroat decides in `Home`, idles in `Idle` and
//! throws in `Fire`, with his own handlers under those numbers, and
//! inherits every other state with its shared number. He and the robber
//! thief take no number of their own: the block [`STATE_BLOCK`] is
//! reserved for the third level and holds no state.
//!
//! His data ([`CutThroatData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; the cutthroat then wraps it in his own.
//!
//! # Random draws, in order
//!
//! - building, and every time he goes idle: one of 0..100, then one of
//!   0..2 when it is below 33 (which line he mutters, on screen only),
//!   made whether or not he is on screen;
//! - the shared pre-tick's roll (frame pulse ticks only);
//! - Idle: one of 0..4 for a new pose; then, in modes 5 and 6 once his
//!   four seconds are up, one of 0..100;
//! - Home: one of 0..121; then going idle's draws when he goes idle; then
//!   the shared Home's when he is still Home and cannot see Claw;
//! - Pace: one of 0..100 per limit reached, then going idle's;
//! - Strike and Fire: none.

use std::ops::RangeInclusive;

use super::human::{self, StraightShot};
use super::lift::Carry;
use super::states::{enter_strike, go_home, human_home_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HUMAN_HIT_BY,
    Handler, Timer, build, data, data_mut, face_claw, human_fields, obj_mut, on_screen, rand, say,
    sees_claw, set_anim, set_anim_now, set_state,
};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["CutThroat"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_CUTTHROAT"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_CUTTHROAT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the third level's families (the
/// cutthroat and the robber thief). No state takes one yet: both classes
/// run their own handlers under the shared numbers.
pub const STATE_BLOCK: RangeInclusive<i32> = 2100..=2119;

/// Logic of the knife.
pub const KNIFE_LOGIC: &str = "Bullet";
/// Image set of the knife.
pub const KNIFE_IMAGE: &str = "LEVEL_KNIFE";
/// Damage of the knife, whatever the difficulty.
pub const KNIFE_DAMAGE: i32 = 10;
/// Speed of the knife, px/s.
pub const KNIFE_SPEED: i32 = 400;
/// Where the knife leaves at a standing Claw: ahead of him and up.
pub const MUZZLE: (i32, i32) = (65, -15);
/// Where the knife leaves at a crouching Claw: ahead of him and down.
pub const MUZZLE_LOW: (i32, i32) = (65, 20);

/// The four stand poses, one picked at random each second.
pub const STAND: [&str; 4] = [
    "LEVEL_CUTTHROAT_STAND1",
    "LEVEL_CUTTHROAT_STAND2",
    "LEVEL_CUTTHROAT_STAND3",
    "LEVEL_CUTTHROAT_STAND4",
];
/// The sword slash.
pub const STRIKE_ANIM: &str = "LEVEL_CUTTHROAT_STRIKE1";
/// The throw at a standing Claw.
pub const THROW_ANIM: &str = "LEVEL_CUTTHROAT_STRIKE2";
/// The low throw, at a crouching Claw.
pub const THROW_LOW_ANIM: &str = "LEVEL_CUTTHROAT_STRIKE3";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_CUTTHROAT_FALL";

/// The sword slash box, facing left.
pub const STRIKE: Rect = Rect::new(-75, 0, 0, 27);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-15, -30, 15, 55);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-46, -20, 46, 14);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (300, 50), (200, 50), (75, 50), (50, 50)];

/// Health on the third level, and on any other.
pub const HEALTH: (i32, i32) = (7, 9);
/// Damage of his slash and touch.
pub const DAMAGE: i32 = 10;
/// Smarts base.
pub const SMARTS: i32 = 50;

/// The death sound.
pub const DEATH_SOUND: &str = "LEVEL_CUTTHROAT_00350010";
/// The line when lifted.
pub const LIFTED_LINE: &str = "LEVEL_CUTTHROAT_00350017";
/// The mutters on going idle, one picked at random.
pub const IDLE_LINES: [&str; 2] = ["LEVEL_CUTTHROAT_00350003", "LEVEL_CUTTHROAT_00350012"];
/// The alert, shouted the first time Claw comes within his outer boxes.
pub const ALERT: &str = "LEVEL_CUTTHROAT_00350013";
/// The knife leaving at a standing Claw.
pub const THROW_SOUND: &str = "LEVEL_CUTTHROAT_THROWHIGH";
/// The knife leaving at a crouching Claw.
pub const THROW_LOW_SOUND: &str = "LEVEL_CUTTHROAT_THROWLOW";

/// Chance in 100 that going idle mutters (on screen).
pub const IDLE_LINE_CHANCE: i32 = 33;
/// Time between stand poses, ms.
pub const POSE_MS: u64 = 1000;
/// Time idle before pacing may start, ms.
pub const PACE_WAIT_MS: u64 = 4000;
/// Chance in 100 per idle tick of starting to pace.
pub const PACE_CHANCE: i32 = 20;
/// Home: draws of 0..121; up close, below smarts he slashes.
pub const HOME_SPREAD: u32 = 121;
/// Least time between throws on the third level, and on any other, ms.
pub const THROW_WAIT_MS: (u64, u64) = (2000, 1500);

/// The cutthroat class.
pub static CLASS: Class = Class {
    name: "CutThroat",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (0, 0, -30),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice: super::nothing,
    anims: ClassAnims {
        advance: "",
        fast_advance: "LEVEL_CUTTHROAT_WALK",
        pace: "LEVEL_CUTTHROAT_WALK",
        walk: "",
        hit_low: "LEVEL_CUTTHROAT_HITLOW",
        hit_high: "LEVEL_CUTTHROAT_HITHIGH",
        home: "LEVEL_CUTTHROAT_HOME",
        jump: "LEVEL_CUTTHROAT_JUMP",
        fall: "LEVEL_CUTTHROAT_FALL",
        lifted: "LEVEL_CUTTHROAT_LIFTED",
        lifted2: "LEVEL_CUTTHROAT_LIFTED2",
        bounce: "LEVEL_CUTTHROAT_THROWN",
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
};

/// A cutthroat's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CutThroatData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Fire => Some(fire_tick),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id, &CLASS);
}

/// `attacker` hit `id`: the human hit rule.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    super::on_hit(w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// Health of a cutthroat on the level being played, before the
/// difficulty factor: 7 on the third level, 9 on the others.
pub fn health(w: &World) -> i32 {
    if w.level_number() == 3 {
        HEALTH.0
    } else {
        HEALTH.1
    }
}

/// Least time between throws on the level being played, ms.
pub fn throw_wait_ms(w: &World) -> u64 {
    if w.level_number() == 3 {
        THROW_WAIT_MS.0
    } else {
        THROW_WAIT_MS.1
    }
}

/// Builds the cutthroat: his boxes, going by sight, liftable, 7 health on
/// the third level (9 on others), 10 damage, smarts 99, his loot and
/// patrol limits from the level record; then he drops to the floor and
/// goes idle. Draws: going idle's.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
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
        image_set: "LEVEL_CUTTHROAT",
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: health(w),
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::CutThroat(CutThroatData { enemy });
    }
    go_idle(w, id);
}

/// Goes idle: a third of the time he mutters one of his two lines (when
/// on screen); he stops, takes his first stand pose (advanced once) and
/// waits four seconds before pacing may start. He does not turn to Claw.
/// Draws: one of 0..100, then one of 0..2 when it is below 33.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if rand(w, 100) < IDLE_LINE_CHANCE {
        let pick = rand(w, 2) as usize;
        if on_screen(w, id) {
            say(w, id, IDLE_LINES[pick]);
        }
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    set_anim_now(w, id, STAND[0]);
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, PACE_WAIT_MS);
    }
}

/// Idle: a new stand pose once the shared pose and throw timer has run
/// out (the timer then runs a second; the pose is advanced once, and not
/// otherwise); then by mode he goes Home (0 to 4), or (5, 6) once his four
/// seconds are up starts pacing one time in five. He does not look for
/// Claw here. Draws: one of 0..4 for a new pose; one of 0..100 in modes
/// 5 and 6 once his four seconds are up.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.pose.done(now)) {
        if let Some(d) = data_mut(w, id) {
            d.pose = Timer::new(now, POSE_MS);
        }
        let pick = rand(w, 4) as usize;
        set_anim_now(w, id, STAND[pick]);
    }
    let Some((mode, wait)) = data(w, id).map(|d| (d.mode, d.pace_wait)) else {
        return;
    };
    match mode {
        0..=4 => go_home(w, id),
        _ => {
            if wait.done(now) && rand(w, 100) < PACE_CHANCE {
                start_pacing(w, id);
            }
        }
    }
}

/// Home: faces Claw and draws one of 0..121. Up close (modes 3, 4) he
/// slashes when the draw is below his smarts; in his outer boxes (modes
/// 0 to 2) he goes idle when he cannot see Claw, and shouts his alert the
/// first time (on screen; the alert counts as shouted even off screen).
/// Then, seeing Claw, he throws when his throw is ready (a throw replaces
/// a slash just started); otherwise the shared human Home runs, which
/// does nothing once he has left Home. Draws: one of 0..121, then going
/// idle's, then the shared Home's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let r = rand(w, HOME_SPREAD);
    let Some((mode, smarts)) = data(w, id).map(|d| (d.mode, d.smarts)) else {
        return;
    };
    match mode {
        3 | 4 if r < smarts => enter_strike(w, id),
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

/// Throws when the shared pose and throw timer has run out; the timer then
/// runs two seconds on the third level (one and a half on others).
pub fn fire_check(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.pose.done(now)) {
        enter_fire(w, id);
        let wait = throw_wait_ms(w);
        if let Some(d) = data_mut(w, id) {
            d.pose = Timer::new(now, wait);
        }
    }
}

/// Starts a throw in the direction he faces (Home faced Claw this tick):
/// high at a standing Claw, low at a crouching one.
pub fn enter_fire(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Fire);
    let low = w
        .claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK);
    human::load_shot(w, id, if low { MUZZLE_LOW } else { MUZZLE }, KNIFE_SPEED);
    set_anim(w, id, if low { THROW_LOW_ANIM } else { THROW_ANIM });
}

/// Fire: on the throw's shot frame the knife leaves (once per throw) with
/// its sound; Home when the throw ends.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    let low = data(w, id).is_some_and(|d| d.shot.dy == MUZZLE_LOW.1);
    human::fire_tick(
        w,
        id,
        StraightShot {
            image: KNIFE_IMAGE,
            damage: KNIFE_DAMAGE,
            sound: if low { THROW_LOW_SOUND } else { THROW_SOUND },
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knife_flies_as_a_bullet_with_its_own_picture() {
        assert_eq!(KNIFE_LOGIC, human::SHOT_LOGIC);
        assert!(crate::logic::projectile::IMAGE_SETS.contains(&KNIFE_IMAGE));
        assert_eq!(
            crate::logic::projectile::image_frame_box(KNIFE_IMAGE),
            Some(Rect::new(-13, -5, 14, 4))
        );
    }

    #[test]
    fn every_animation_he_plays_is_preloaded() {
        let anims = CLASS.anims;
        let keys = [
            STRIKE_ANIM,
            THROW_ANIM,
            THROW_LOW_ANIM,
            DEATH_ANIM,
            anims.fast_advance,
            anims.pace,
            anims.hit_low,
            anims.hit_high,
            anims.home,
            anims.jump,
            anims.fall,
            anims.lifted,
            anims.lifted2,
            anims.bounce,
        ];
        for k in keys.iter().chain(STAND.iter()) {
            assert!(ANIM_PREFIXES.iter().any(|p| k.starts_with(p)), "{k}");
        }
    }
}
