//! The robber thief: a human enemy of the third and fourth levels who
//! shoots arrows at Claw whenever he sees him, and has no blow up close.
//! He never walks up to Claw.
//!
//! He goes by sight, as the soldier and the cutthroat do: he sees Claw only
//! when Claw is not invisible, is less than 50 px above or below him, the
//! robber thief is on screen, and no wall stands between them on his row.
//!
//! - Idle, he plays his idle animation. He goes Home on the first tick he
//!   sees Claw, however far away, and whenever Claw is inside one of his
//!   range boxes (modes 0 to 4: 400, 300 and 200 px across for the outer
//!   three, 100 and 50 px for the inner two, 40 or 50 px up or down); with
//!   Claw out of range (modes 5, 6), after two seconds idle, a draw of
//!   0..201 equal to 1 starts him pacing (the shared human idle state).
//! - Home, he faces Claw. When he cannot see Claw he goes idle (so with
//!   Claw in range but hidden he goes Home and idle on alternate ticks).
//!   Otherwise he draws one of 0..100 and, below his smarts (99), shoots
//!   when his bow is ready: 1500 ms after the last arrow, the first at
//!   once.
//! - The arrow leaves on the shooting animation's shot frame, 65 px ahead
//!   of him and 15 px up at a standing Claw, or 20 px down (with the low
//!   shot) at a crouching one: a `Bullet` drawn from [`ARROW_IMAGE`],
//!   400 px/s flat, 10 damage whatever the difficulty. The release sounds
//!   are named the other way round: the high shot plays
//!   [`SHOT_SOUND`] (`ARROWLOW`) and the low one [`SHOT_LOW_SOUND`]
//!   (`ARROWHIGH`).
//! - Going idle he picks one of his four lines and says it a third of the
//!   time (on screen). He shouts no alert.
//!
//! The rest is the shared human behaviour: flinches, death and loot,
//! being lifted and thrown, the leap at Claw from a narrow ledge; a record
//! whose first user value is above 0 never walks. He never blocks or
//! dodges. He has 7 health and 10 damage (his touch), times the
//! difficulty factors. His death plays no sound of his own.
//!
//! State numbers ([`EnemyState`]): one number per state name, whichever
//! class runs it, so the robber thief decides in `Home` and shoots in
//! `Fire`, with his own handlers under those numbers, idles in `Idle`
//! with the shared human idle handler, and inherits every other state
//! with its shared number. He takes no number of his own (see
//! [`super::cutthroat::STATE_BLOCK`]).
//!
//! His data ([`RobberThiefData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; the robber thief then wraps it in his own.
//!
//! # Random draws, in order
//!
//! - building, and every time he goes idle: one of 0..4 (which line), then
//!   one of 0..100 (below 33 he says it, on screen);
//! - the shared pre-tick's roll (frame pulse ticks only);
//! - Idle: one of 0..201, then going idle's when a stationary robber thief
//!   would start pacing;
//! - Home: going idle's when he cannot see Claw, otherwise one of 0..100;
//! - Pace: one of 0..100 per limit reached, then going idle's;
//! - Fire: none.

use super::human::{self, HumanAnims, HumanData, StraightShot};
use super::lift::Carry;
use super::states::go_home;
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HUMAN_HIT_BY,
    Handler, Timer, build, data, data_mut, face_claw, human_fields, obj_mut, rand, sees_claw,
    set_anim, set_state,
};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RobberThief"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_ROBBERTHIEF"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_ROBBERTHIEF_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Logic of the arrow.
pub const ARROW_LOGIC: &str = "Bullet";
/// Image set of the arrow.
pub const ARROW_IMAGE: &str = "LEVEL_ARROW";
/// Damage of the arrow, whatever the difficulty.
pub const ARROW_DAMAGE: i32 = 10;
/// Speed of the arrow, px/s.
pub const ARROW_SPEED: i32 = 400;
/// Where the arrow leaves at a standing Claw: ahead of him and up.
pub const MUZZLE: (i32, i32) = (65, -15);
/// Where the arrow leaves at a crouching Claw: ahead of him and down.
pub const MUZZLE_LOW: (i32, i32) = (65, 20);

/// Idle.
pub const IDLE_ANIM: &str = "LEVEL_ROBBERTHIEF_IDLE";
/// Shooting at a standing Claw.
pub const SHOOT_ANIM: &str = "LEVEL_ROBBERTHIEF_STRIKE1";
/// Shooting low, at a crouching Claw.
pub const SHOOT_LOW_ANIM: &str = "LEVEL_ROBBERTHIEF_STRIKE2";
/// The strike he is given but never makes.
pub const STRIKE_ANIM: &str = "LEVEL_ROBBERTHIEF_STRIKE1";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_ROBBERTHIEF_FALL";

/// The strike box he is given but never uses, facing left.
pub const STRIKE: Rect = Rect::new(-75, 0, 0, 27);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-15, -30, 15, 55);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-30, -50, 31, 8);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The five range boxes (half width, half height), outermost first; the
/// second keeps the human enemies' default.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (300, 40), (200, 50), (100, 50), (50, 50)];

/// Health on every level.
pub const HEALTH: i32 = 7;
/// Damage of his touch.
pub const DAMAGE: i32 = 10;
/// Smarts base.
pub const SMARTS: i32 = 25;

/// The death sound: none (his kills are silent unless Claw speaks).
pub const DEATH_SOUND: &str = "";
/// The line when lifted.
pub const LIFTED_LINE: &str = "LEVEL_ROBBERTHIEF_00340012";
/// His lines on going idle, one picked each time.
pub const IDLE_LINES: [&str; 4] = [
    "LEVEL_ROBBERTHIEF_00340004",
    "LEVEL_ROBBERTHIEF_00360002",
    "LEVEL_ROBBERTHIEF_00360009",
    "LEVEL_ROBBERTHIEF_00360010",
];
/// The arrow leaving at a standing Claw.
pub const SHOT_SOUND: &str = "LEVEL_ROBBERTHIEF_ARROWLOW";
/// The arrow leaving at a crouching Claw.
pub const SHOT_LOW_SOUND: &str = "LEVEL_ROBBERTHIEF_ARROWHIGH";

/// Home: draws of 0..100; below smarts he shoots when ready.
pub const SHOT_SPREAD: u32 = 100;
/// Least time between arrows, ms.
pub const SHOT_WAIT_MS: u64 = 1500;

/// The robber thief class.
pub static CLASS: Class = Class {
    name: "RobberThief",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (0, 0, -50),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice: super::nothing,
    anims: ClassAnims {
        advance: "",
        fast_advance: "LEVEL_ROBBERTHIEF_FASTADVANCE",
        pace: "LEVEL_ROBBERTHIEF_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_ROBBERTHIEF_HITLOW",
        hit_high: "LEVEL_ROBBERTHIEF_HITHIGH",
        home: "LEVEL_ROBBERTHIEF_HOME",
        jump: "LEVEL_ROBBERTHIEF_JUMP",
        fall: "LEVEL_ROBBERTHIEF_FALL",
        lifted: "LEVEL_ROBBERTHIEF_LIFTED",
        lifted2: "LEVEL_ROBBERTHIEF_LIFTED2",
        bounce: "LEVEL_ROBBERTHIEF_THROWN",
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
};

/// A robber thief's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RobberThiefData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
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

/// Builds the robber thief: his boxes, going by sight, liftable, 7
/// health, 10 damage, smarts 99, his bow ready, his loot, patrol limits
/// and stillness from the level record; then he drops to the floor and
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
        human: HumanData {
            anims: HumanAnims {
                idle: IDLE_ANIM,
                ..HumanAnims::default()
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: "LEVEL_ROBBERTHIEF",
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::RobberThief(RobberThiefData { enemy });
    }
    go_idle(w, id);
}

/// Goes idle: picks one of his four lines, then the shared human going
/// idle (faces Claw, stops, says the line a third of the time on screen,
/// his idle animation, two seconds before pacing may start). Draws: one
/// of 0..4, then one of 0..100.
pub fn go_idle(w: &mut World, id: ObjectId) {
    let pick = rand(w, 4) as usize;
    if let Some(d) = data_mut(w, id) {
        d.human.sounds.idle_voice = IDLE_LINES[pick];
    }
    human::go_idle(w, id);
}

/// Home: faces Claw; when he cannot see Claw he goes idle; otherwise,
/// with a draw of 0..100 below his smarts, he shoots when his bow is
/// ready. The animation is not advanced. Draws: going idle's, or one of
/// 0..100.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if !sees_claw(w, id) {
        go_idle(w, id);
        return;
    }
    let r = rand(w, SHOT_SPREAD);
    if data(w, id).is_some_and(|d| r < d.smarts) {
        fire_check(w, id);
    }
}

/// Shoots when the bow is ready (1500 ms after the last arrow; the first
/// at once).
pub fn fire_check(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.fire_wait.done(now)) {
        enter_fire(w, id);
        if let Some(d) = data_mut(w, id) {
            d.fire_wait = Timer::new(now, SHOT_WAIT_MS);
        }
    }
}

/// Starts a shot in the direction he faces (Home faced Claw this tick):
/// high at a standing Claw, low at a crouching one.
pub fn enter_fire(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Fire);
    let low = w
        .claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK);
    human::load_shot(w, id, if low { MUZZLE_LOW } else { MUZZLE }, ARROW_SPEED);
    set_anim(w, id, if low { SHOOT_LOW_ANIM } else { SHOOT_ANIM });
}

/// Fire: on the shot frame the arrow leaves (once per shot) with its
/// sound; Home when the shooting animation ends.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    let low = data(w, id).is_some_and(|d| d.shot.dy == MUZZLE_LOW.1);
    human::fire_tick(
        w,
        id,
        StraightShot {
            image: ARROW_IMAGE,
            damage: ARROW_DAMAGE,
            sound: if low { SHOT_LOW_SOUND } else { SHOT_SOUND },
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrow_flies_as_a_bullet_with_its_own_picture() {
        assert_eq!(ARROW_LOGIC, human::SHOT_LOGIC);
        assert!(crate::logic::projectile::IMAGE_SETS.contains(&ARROW_IMAGE));
        assert_eq!(
            crate::logic::projectile::image_frame_box(ARROW_IMAGE),
            Some(Rect::new(-19, -3, 16, 2))
        );
    }

    #[test]
    fn he_idles_with_the_shared_human_idle_state() {
        assert!(handler(EnemyState::Idle).is_none());
        assert!(super::super::states::shared_handler(EnemyState::Idle, true).is_some());
        assert!(handler(EnemyState::Strike).is_none());
    }

    #[test]
    fn every_animation_he_plays_is_preloaded() {
        let anims = CLASS.anims;
        let keys = [
            IDLE_ANIM,
            SHOOT_ANIM,
            SHOOT_LOW_ANIM,
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
        for k in keys {
            assert!(ANIM_PREFIXES.iter().any(|p| k.starts_with(p)), "{k}");
        }
    }
}
