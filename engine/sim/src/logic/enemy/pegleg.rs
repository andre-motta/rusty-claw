//! PegLeg, the peg-legged gunner of the ninth level (and of the tenth),
//! with the bullet he fires ([`bullet`]).
//!
//! He is a human enemy who never swings at Claw. He fires a bullet from
//! 150 to 200 px away (it leaves on the 19th tick of the shot, the shot
//! ends on the 31st), and when Claw comes closer than 150 px he rolls
//! [`ROLL_DISTANCE`] px away from him ([`EnemyState::RollBackward`]) and
//! fires again. From 200 to 500 px he walks in or rolls towards Claw
//! ([`EnemyState::RollForward`]). Rolls are [`ROLL_SPEED`] px/s, stop on
//! their target, on a patrol bound or after [`ROLL_MS`], and nothing hurts
//! him while he rolls.
//!
//! On the ninth level he aims each shot at Claw's posture (low when Claw
//! ducks) and stops a forward roll [`ROLL_STOP_SHORT`] px short of Claw.
//! Pistol shots never hurt him: they make him roll at Claw. Standing sword
//! stabs, punches and kicks land only half the time, a crouching stab
//! always lands and opens a window of [`CROUCH_WINDOW_MS`] in which the
//! next blow always lands, and a throw never costs him health: after
//! landing from one he pauses, taunts and fires six shots
//! [`FIRE_WINDOW_SHOT_TICKS`] ticks apart, alternating high and low. He has
//! [`HEALTH`] health and drops his record's loot.
//!
//! # The rules
//!
//! - Built: the health and damage above, going by sight, liftable; the
//!   range boxes [`RANGES`]; then dropped to the floor and Home (not
//!   idle). Going idle (the shared human go idle) plays his idle pose
//!   ([`IDLE_ANIM`]), as Home does, so an idle PegLeg is drawn, and seen
//!   on screen, with its pictures.
//! - Going Home: the shared go Home, his idle picture (not advanced), his
//!   hit box back, and a draw of 0..1001 nothing reads.
//! - Home: the picture advances; the alert once, the first time he sees
//!   Claw (half the time one of his two lines); after a throw's landing,
//!   the first time he sees Claw, one of his two taunts, the firing spell
//!   of [`FIRE_WINDOW_MS`] and a shot. Otherwise a draw of 0..101: Claw 200
//!   to 500 px away (modes 0, 1), below 50 he walks at him, otherwise he
//!   rolls at him; 150 to 200 px (mode 2), below 70 he fires, otherwise he
//!   rolls away; within 150 px (modes 3, 4), below 70 he rolls away,
//!   otherwise he stays. Out of range he stays while he sees Claw and
//!   paces otherwise. He never turns in Home.
//! - Firing: he stops; while his gun cools (1000 ms after a bullet) he only
//!   goes Home; otherwise he faces Claw, a quarter of the time says one of
//!   his four firing lines, and picks the shot: on the ninth level outside
//!   the spell low at a ducking Claw and high otherwise, else alternating
//!   high and low. On the shot's event the bullet leaves with the gunshot;
//!   at the shot's end he fires again inside the spell (and on the tenth
//!   level a second shot of a pair), otherwise he goes Home.
//! - Rolling forward: in the window after a crouching stab he only goes
//!   Home; otherwise he faces Claw, aims 150 px towards him (on the ninth
//!   level no further than 35 px short of him, firing at once when he is
//!   already there), plays the roll, is busy and has no hit box. On the
//!   tick he starts he moves right whichever side Claw is on (a patrol
//!   bound still holds him); then towards Claw. He stops and fires on his
//!   target (or 35 px short of Claw on the ninth level), on the patrol
//!   bound ahead, or after [`ROLL_MS`].
//! - Rolling back: in the window he only goes Home; standing on the bound
//!   behind him he fires instead; otherwise he faces Claw and rolls 150 px
//!   away, firing on his target, on either bound or after [`ROLL_MS`].
//! - Hits: none while he rolls (not even counted). Claw's blows, by kind
//!   ([`melee_group`]): the standing stab, punches, kicks and the jumping
//!   stab land on a roll of 0..101 below 50, or always inside the window,
//!   as a high flinch; the crouching stab lands as a low flinch outside the
//!   window and opens it; any other kind follows the shared human rule. A
//!   pistol shot rolls him at Claw (it does no harm and flies on). Magic,
//!   thrown objects and blasts: the shared human rules.
//! - Lifted: one of his four lifted lines, his hit box back.
//! - Thrown: he lands unhurt; the first landing sets his taunt.
//!
//! # Random draws, in order
//!
//! - building and every going Home: one of 0..1001;
//! - Home: the alert's one of 0..101 (then one of 0..2 below 50), once; or
//!   the taunt's one of 0..2 and the shot's; then in modes 0 to 4 one of
//!   0..101, and what it leads to;
//! - firing: while the gun cools, going Home's; otherwise one of 0..101
//!   (then one of 0..4 below 25);
//! - a roll in the window: going Home's;
//! - hit by Claw's blow: the standing kinds one of 0..101, then (landing)
//!   one of 0..4 and the burst's two; the crouching stab (landing) the
//!   burst's two; others the shared rule's;
//! - lifted: one of 0..4, which is the shared lift's own draw (see
//!   [`pick_lift_voice`]).
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he decides in `Home` and
//! shoots in `Fire` (2013) with handlers of his own and inherits every
//! other shared human state with its number. His two new names,
//! [`EnemyState::RollForward`] and [`EnemyState::RollBackward`], take the
//! first two numbers of the ninth level's block
//! ([`super::hook_pirate::STATE_BLOCK`]).
//!
//! # Defaults taken
//!
//! - The ninth level's block of state numbers: rolls at 2220 and 2221 (see
//!   [`super::hook_pirate`]).
//! - The "on screen" test of his sight under a widened view follows the
//!   rule the soldier's sight test uses.
//! - The entry tick of a forward roll moves him right at [`ROLL_SPEED`]
//!   whichever side Claw is on (a patrol bound still holds him), as the
//!   original does; the right sign comes from the next tick.
//! - A backward roll that reaches its target fires and ends that tick's
//!   roll checks.
//! - Carried, he keeps the movement box of his class (the original swaps
//!   in the box saved at his last lift after his first throw).
//!
//! Its data ([`PegLegData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use super::human::{self, HumanAnims, HumanData};
use super::lift::{self, Carry};
use super::states::{self, enter_hit, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, FIRE_BLOW,
    FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit, attacker,
    build, data, data_mut, die, exact_x, face_claw, faces_right, facing_rect, hit_burst,
    human_fields, obj, obj_mut, play, rand, say, sees_claw, set_anim, set_state, take_damage,
};
use crate::logic::projectile::{self, ProjectileSpec};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, types};
use crate::physics::ptype;
use crate::player::attack::kind as attack_kind;
use crate::rng::Rng;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PegLeg"];

/// His picture, set when he is built (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_PEGLEG";

/// Image sets this family may switch an object to: his own and his
/// bullet's.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, bullet::IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_PEGLEG_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// His two new states, in number order.
pub const STATES: [EnemyState; 2] = [EnemyState::RollForward, EnemyState::RollBackward];

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 7;
/// Damage of touching him, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Smarts base (raised to 99; nothing of his reads it).
pub const SMARTS: i32 = 50;
/// Hit box (none while he rolls forward, see [`ROLLING_HIT_RECT`]).
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// His hit box while he rolls forward: no box at all for hits (no hit
/// reaches him while he rolls), but Claw's reach still measures it as it
/// is stored, a box with no size whose right edge is his position, so
/// Claw facing left can lift him from as close as he could lift a body
/// with that edge (a rolling PegLeg is lifted measured to his position,
/// not to the edge of his usual box). Claw facing right never takes him
/// for a target.
pub const ROLLING_HIT_RECT: Rect = Rect::new(Rect::UNSET_LEFT + 1, 0, 0, 0);
/// Movement box: the box of his first picture.
pub const MOVE_RECT: Rect = Rect::new(-12, -43, 39, 50);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-37, 17, 48, 49);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];
/// Speed of a roll, px/s.
pub const ROLL_SPEED: i32 = 250;
/// Length of a roll, px.
pub const ROLL_DISTANCE: i32 = 150;
/// How far short of Claw a forward roll stops on the ninth level, px.
pub const ROLL_STOP_SHORT: i32 = 35;
/// The longest a roll lasts, ms.
pub const ROLL_MS: u32 = 1500;
/// Ticks of a 150 px roll at the 18 ms step (measured).
pub const ROLL_TICKS: u32 = 34;
/// Time after a bullet before he can fire again, ms.
pub const FIRE_COOLDOWN_MS: u32 = 1000;
/// The firing spell after a taunt, ms.
pub const FIRE_WINDOW_MS: u32 = 3000;
/// Ticks between two shots of the firing spell at the 18 ms step
/// (measured).
pub const FIRE_WINDOW_SHOT_TICKS: u32 = 31;
/// Shots of the firing spell (measured).
pub const FIRE_WINDOW_SHOTS: u32 = 6;
/// The window after a crouching stab lands in which every blow lands, ms.
pub const CROUCH_WINDOW_MS: u32 = 2000;
/// The bullet's tick and the shot's last tick, counted from the shot's
/// first tick (measured).
pub const SHOT_TICKS: (u32, u32) = (19, 31);
/// Ticks of the high flinch at the 18 ms step (measured).
pub const HIT_HIGH_TICKS: u32 = 19;
/// Ticks of the low flinch after a crouching stab at the 18 ms step
/// (measured).
pub const HIT_LOW_TICKS: u32 = 17;
/// Ticks of the pause after landing from a throw, at the 18 ms step
/// (measured).
pub const THROW_PAUSE_TICKS: u32 = 28;
/// The cooldown after any blow of Claw's that reaches him, ms.
pub const BLOW_COOLDOWN_MS: u64 = 300;
/// The cooldown after a blast, ms.
pub const BLAST_COOLDOWN_MS: u64 = 2000;
/// Chance in 101 that his first sight of Claw says an alert line.
pub const ALERT_CHANCE: i32 = 50;
/// Chance in 101 that a shot says a firing line.
pub const FIRE_LINE_CHANCE: i32 = 25;
/// Of a draw of 0..101, the values below which a standing blow lands.
pub const BLOW_LANDS_BELOW: i32 = 50;
/// Of a draw of 0..101 in Home, the values below which he walks (modes 0,
/// 1) rather than rolls at Claw.
pub const WALK_BELOW: i32 = 50;
/// Of a draw of 0..101 in Home, the values below which he fires (mode 2)
/// or rolls away (modes 3, 4).
pub const ACT_BELOW: i32 = 70;
/// The draw of going Home: one of 0..1001.
pub const HOME_SPREAD: u32 = 1001;

/// The high shot, at a standing Claw.
pub const HIGH_SHOT_ANIM: &str = "LEVEL_PEGLEG_STRIKE1";
/// The low shot, at a ducking Claw.
pub const LOW_SHOT_ANIM: &str = "LEVEL_PEGLEG_STRIKE2";
/// The forward roll.
pub const FORWARD_ROLL_ANIM: &str = "LEVEL_PEGLEG_FORWARDROLL";
/// The backward roll.
pub const BACKWARD_ROLL_ANIM: &str = "LEVEL_PEGLEG_BACKWARDROLL";
/// The idle pose, shown in `Home` and while idle.
pub const IDLE_ANIM: &str = "LEVEL_PEGLEG_IDLE";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_PEGLEG_KILLFALL";
/// The shot's sound.
pub const GUNSHOT_SOUND: &str = "LEVEL_PEGLEG_SOLGUNFR8-22";
/// A roll's sound.
pub const ROLL_SOUND: &str = "LEVEL_PEGLEG_ROLL1";
/// His death.
pub const DEATH_SOUND: &str = "LEVEL_PEGLEG_00410004";
/// His low flinch's sound.
pub const HIT_LOW_SOUND: &str = "LEVEL_PEGLEG_00410003";
/// His alert lines.
pub const ALERT_LINES: [&str; 2] = ["LEVEL_PEGLEG_00410001", "LEVEL_PEGLEG_00410002"];
/// His taunts after a throw.
pub const TAUNTS: [&str; 2] = ["LEVEL_PEGLEG_00410010", "LEVEL_PEGLEG_00410011"];
/// His firing lines.
pub const FIRE_LINES: [&str; 4] = [
    "LEVEL_PEGLEG_00410005",
    "LEVEL_PEGLEG_00410006",
    "LEVEL_PEGLEG_00410007",
    "LEVEL_PEGLEG_00410008",
];
/// His lifted lines.
pub const LIFTED_LINES: [&str; 4] = [
    "LEVEL_PEGLEG_00410015",
    "LEVEL_PEGLEG_00410016",
    "LEVEL_PEGLEG_00410017",
    "LEVEL_PEGLEG_00410009",
];

/// Whether the PegLeg of level `level` aims at Claw's posture and stops
/// short of him: on the ninth level; on the tenth he fires pairs,
/// alternating, and rolls the full length.
pub fn aims_at_posture(level: u32) -> bool {
    level == 9
}

/// The bullet PegLeg fires: the soldier's bullet with Claw's pistol
/// picture, an enemy's shot that flies straight at [`SPEED`](bullet::SPEED)
/// px/s (9 px a tick) and costs Claw [`DAMAGE`](bullet::DAMAGE). It leaves
/// [`MUZZLE_HIGH`](bullet::MUZZLE_HIGH) or [`MUZZLE_LOW`](bullet::MUZZLE_LOW)
/// from him, mirrored when he faces left. It is not a family of its own:
/// the projectile family runs it.
pub mod bullet {
    /// Logic name of the bullet.
    pub const LOGIC: &str = "Bullet";
    /// Its picture.
    pub const IMAGE_SET: &str = "GAME_BULLETS";
    /// Damage to Claw.
    pub const DAMAGE: i32 = 10;
    /// Speed, px/s.
    pub const SPEED: i32 = 500;
    /// Where the high shot leaves, facing right.
    pub const MUZZLE_HIGH: (i32, i32) = (55, -24);
    /// Where the low shot leaves, facing right.
    pub const MUZZLE_LOW: (i32, i32) = (55, 15);

    /// Where a shot leaves, from PegLeg's position, and its speed: high or
    /// low (`low`), facing right or left (`right`).
    pub fn start(low: bool, right: bool) -> ((i32, i32), i32) {
        let (x, y) = if low { MUZZLE_LOW } else { MUZZLE_HIGH };
        if right {
            ((x, y), SPEED)
        } else {
            ((-x, y), -SPEED)
        }
    }
}

/// The animations of the shared states.
const ANIMS: ClassAnims = ClassAnims {
    advance: "LEVEL_PEGLEG_ADVANCE",
    fast_advance: "LEVEL_PEGLEG_FASTADVANCE",
    pace: "LEVEL_PEGLEG_ADVANCE",
    walk: "",
    hit_low: "LEVEL_PEGLEG_HITLOW",
    hit_high: "LEVEL_PEGLEG_HITHIGH",
    home: "LEVEL_PEGLEG_HOME",
    jump: "LEVEL_PEGLEG_JUMP",
    fall: DEATH_ANIM,
    lifted: "LEVEL_PEGLEG_LIFTED1",
    lifted2: "LEVEL_PEGLEG_LIFTED2",
    bounce: "LEVEL_PEGLEG_THROWN",
};

/// The sounds of the shared states: a low flinch plays his own hit sound
/// in place of the shared first one.
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: [HIT_LOW_SOUND, super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries him: the shared human offsets and his own box.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The PegLeg class.
pub static CLASS: Class = Class {
    name: "PegLeg",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home,
    go_idle: human::go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    lift_roll: true,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A PegLeg's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PegLegData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// He landed after a throw and has not taunted Claw since.
    pub taunt: bool,
    /// The next alternating shot is the low one.
    pub low_next: bool,
    /// Shots of the current pair (tenth level).
    pub pair_shots: u32,
    /// The current shot's bullet has left.
    pub fired: bool,
    /// Where the current shot's bullet leaves, from him.
    pub muzzle: (i32, i32),
    /// The current shot's bullet speed, px/s.
    pub bullet_vx: i32,
    /// His gun cools after a bullet.
    pub fire_cooldown: Timer,
    /// The firing spell after a taunt.
    pub fire_window: Timer,
    /// The window after a crouching stab landed.
    pub crouch_window: Timer,
    /// The longest a roll lasts.
    pub roll_timer: Timer,
    /// Where the current roll stops.
    pub target: i32,
}

/// The PegLeg data of `id`.
pub fn peg_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a PegLegData> {
    match &w.objects.get(id)?.data {
        ObjectData::PegLeg(d) => Some(d),
        _ => None,
    }
}

fn peg_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut PegLegData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::PegLeg(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::Fire => Some(fire_tick),
        EnemyState::RollForward => Some(roll_forward_tick),
        EnemyState::RollBackward => Some(roll_backward_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call. A throw's landing costs
/// him nothing (the shared landing's cost is given back first), and a
/// landing during the call sets his taunt.
pub fn tick(w: &mut World, id: ObjectId) {
    let resting_landing = super::state(w, id) == Some(EnemyState::Bounce)
        && data(w, id).is_some_and(|d| d.prev == EnemyState::Pause && !d.dead);
    if resting_landing {
        obj_mut(w, id).health += lift::LANDING_DAMAGE;
    }
    let before = data(w, id).map(|d| d.bounces);
    human::tick(w, id);
    let after = data(w, id).map(|d| d.bounces);
    if before.is_some()
        && before != after
        && after.is_some_and(|b| b > 0)
        && let Some(p) = peg_mut(w, id)
    {
        p.taunt = true;
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building and going Home

/// Builds a PegLeg (see the module documentation). Draws: going Home's one
/// of 0..1001.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        sight: true,
        contact_rect: CONTACT,
        strike_rect: Rect::UNSET,
        strike_anim: "",
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: "",
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
        image_set: IMAGE_SET,
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let x = obj(w, id).x;
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::PegLeg(PegLegData {
            enemy,
            target: x,
            ..PegLegData::default()
        });
    }
    go_home(w, id);
}

/// Goes Home: the shared go Home, his idle picture (not advanced), his hit
/// box back. Draws: one of 0..1001 (nothing reads it).
pub fn go_home(w: &mut World, id: ObjectId) {
    states::go_home(w, id);
    set_anim(w, id, IDLE_ANIM);
    restore_hit_rect(w, id);
    let _unread = rand(w, HOME_SPREAD);
}

fn restore_hit_rect(w: &mut World, id: ObjectId) {
    let o = obj_mut(w, id);
    o.hit_rect = facing_rect(o, HIT_RECT);
}

// ----------------------------------------------------------------------
// Home

/// Home (see the module documentation). Draws: the alert's (once) or the
/// taunt's and the shot's; then in modes 0 to 4 one of 0..101 and what it
/// leads to.
pub fn home_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let taunt = peg_data(w, id).is_some_and(|p| p.taunt);
    let alerted = data(w, id).is_none_or(|d| d.alerted);
    if !taunt && !alerted && sees_claw(w, id) {
        if rand(w, 101) < ALERT_CHANCE {
            let line = ALERT_LINES[rand(w, 2) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    if taunt && sees_claw(w, id) {
        let line = TAUNTS[rand(w, 2) as usize];
        say(w, id, line);
        let now = w.now_ms;
        if let Some(p) = peg_mut(w, id) {
            p.taunt = false;
            p.fire_window = Timer::new(now, u64::from(FIRE_WINDOW_MS));
        }
        fire(w, id);
        return;
    }
    match data(w, id).map_or(6, |d| d.mode) {
        0 | 1 => {
            if rand(w, 101) < WALK_BELOW {
                states::enter_advance(w, id);
            } else {
                roll_forward(w, id);
            }
        }
        2 => {
            if rand(w, 101) < ACT_BELOW {
                fire(w, id);
            } else {
                roll_backward(w, id);
            }
        }
        3 | 4 => {
            if rand(w, 101) < ACT_BELOW {
                roll_backward(w, id);
            }
        }
        _ => {
            if !sees_claw(w, id) {
                start_pacing(w, id);
            }
        }
    }
}

// ----------------------------------------------------------------------
// Firing

/// Whether his firing spell runs.
fn in_fire_window(w: &World, id: ObjectId) -> bool {
    let now = w.now_ms;
    peg_data(w, id).is_some_and(|p| !p.fire_window.done(now))
}

/// Fires (see the module documentation): while his gun cools he only goes
/// Home. Draws: going Home's, or one of 0..101 (then one of 0..4 below
/// 25).
pub fn fire(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    let now = w.now_ms;
    if peg_data(w, id).is_some_and(|p| !p.fire_cooldown.done(now)) {
        go_home(w, id);
        return;
    }
    face_claw(w, id);
    set_state(w, id, EnemyState::Fire);
    if rand(w, 101) < FIRE_LINE_CHANCE {
        let line = FIRE_LINES[rand(w, 4) as usize];
        say(w, id, line);
    }
    let low = if aims_at_posture(w.level_number()) && !in_fire_window(w, id) {
        w.claw_object()
            .is_some_and(|c| c.physics_type == ptype::DUCK)
    } else {
        let p = peg_mut(w, id);
        p.is_some_and(|p| {
            let low = p.low_next;
            p.low_next = !low;
            low
        })
    };
    set_anim(w, id, if low { LOW_SHOT_ANIM } else { HIGH_SHOT_ANIM });
    let right = faces_right(obj(w, id));
    let (muzzle, vx) = bullet::start(low, right);
    if let Some(p) = peg_mut(w, id) {
        p.muzzle = muzzle;
        p.bullet_vx = vx;
        p.fired = false;
    }
}

/// Fire: on the shot's event the bullet leaves with the gunshot and the
/// gun starts cooling; at the shot's end he fires again inside the spell
/// (and for the second shot of a tenth-level pair), otherwise he goes
/// Home.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    if event == 1 && peg_data(w, id).is_some_and(|p| !p.fired) {
        release_bullet(w, id);
    }
    if !obj(w, id).anim.is_done() {
        return;
    }
    let now = w.now_ms;
    let window = in_fire_window(w, id);
    if !aims_at_posture(w.level_number()) && !window {
        let shots = peg_mut(w, id).map_or(2, |p| {
            p.pair_shots += 1;
            p.pair_shots
        });
        if shots < 2 {
            cool_off(w, id, now);
            fire(w, id);
        } else {
            if let Some(p) = peg_mut(w, id) {
                p.pair_shots = 0;
            }
            go_home(w, id);
        }
    } else if window {
        cool_off(w, id, now);
        fire(w, id);
    } else {
        go_home(w, id);
    }
}

/// The gun is ready at once.
fn cool_off(w: &mut World, id: ObjectId, now: u64) {
    if let Some(p) = peg_mut(w, id) {
        p.fire_cooldown = Timer::new(now, 0);
    }
}

/// The bullet leaves one depth behind him, with the gunshot, and his gun
/// starts cooling.
fn release_bullet(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((muzzle, vx)) = peg_mut(w, id).map(|p| {
        p.fired = true;
        p.fire_cooldown = Timer::new(now, u64::from(FIRE_COOLDOWN_MS));
        (p.muzzle, p.bullet_vx)
    }) else {
        return;
    };
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    projectile::spawn(
        w,
        ProjectileSpec {
            logic: bullet::LOGIC,
            x: x + muzzle.0,
            y: y + muzzle.1,
            z: z - 1,
            vx,
            vy: 0,
            damage: bullet::DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: bullet::IMAGE_SET,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    play(w, GUNSHOT_SOUND);
}

// ----------------------------------------------------------------------
// Rolls

/// Whether the window after a crouching stab runs.
fn in_crouch_window(w: &World, id: ObjectId) -> bool {
    let now = w.now_ms;
    peg_data(w, id).is_some_and(|p| !p.crouch_window.done(now))
}

fn claw_x(w: &World) -> i32 {
    w.claw_object().map_or(0, |c| c.x)
}

fn claw_left(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| !d.claw_right)
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn start_roll_timer(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if let Some(p) = peg_mut(w, id) {
        p.roll_timer = Timer::new(now, u64::from(ROLL_MS));
    }
}

fn roll_over(w: &World, id: ObjectId) -> bool {
    let now = w.now_ms;
    peg_data(w, id).is_some_and(|p| p.roll_timer.done(now))
}

fn target(w: &World, id: ObjectId) -> i32 {
    peg_data(w, id).map_or(0, |p| p.target)
}

/// Where a forward roll from `x` stops, Claw at `cx` on the left
/// (`left`) or not: 150 px towards him; on the ninth level (`short`) no
/// further than 35 px short of him when he is within those 150 px. The
/// second value is true when he is already at or past that spot.
pub fn forward_target(x: i32, cx: i32, left: bool, short: bool) -> (i32, bool) {
    let mut target = if left {
        x - ROLL_DISTANCE
    } else {
        x + ROLL_DISTANCE
    };
    if short && ((left && cx >= target) || (!left && cx <= target)) {
        target = if left {
            cx + ROLL_STOP_SHORT
        } else {
            cx - ROLL_STOP_SHORT
        };
        let there = if left { x <= target } else { x >= target };
        return (target, there);
    }
    (target, false)
}

/// Starts a forward roll at Claw (see the module documentation). Draws:
/// in the window, going Home's; when he is already beside Claw, the
/// shot's.
pub fn roll_forward(w: &mut World, id: ObjectId) {
    if in_crouch_window(w, id) {
        go_home(w, id);
        return;
    }
    face_claw(w, id);
    let left = claw_left(w, id);
    let speed = f64::from(ROLL_SPEED);
    set_vx(w, id, if left { -speed } else { speed });
    let x = obj(w, id).x;
    let short = aims_at_posture(w.level_number());
    let (t, there) = forward_target(x, claw_x(w), left, short);
    if let Some(p) = peg_mut(w, id) {
        p.target = t;
    }
    if there {
        fire(w, id);
        return;
    }
    play(w, ROLL_SOUND);
    // The entry tick moves right whichever side Claw is on.
    set_vx(w, id, speed);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::RollForward);
    set_anim(w, id, FORWARD_ROLL_ANIM);
    obj_mut(w, id).hit_rect = ROLLING_HIT_RECT;
    start_roll_timer(w, id);
}

/// RollForward: towards Claw; stops and fires on its target (or 35 px
/// short of Claw on the ninth level), on the patrol bound ahead or once
/// its time is up, each time with his hit box back.
pub fn roll_forward_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let left = claw_left(w, id);
    let speed = f64::from(ROLL_SPEED);
    set_vx(w, id, if left { -speed } else { speed });
    let x = obj(w, id).x;
    let t = target(w, id);
    let mut reached = if left { x <= t } else { x >= t };
    if aims_at_posture(w.level_number()) {
        let cx = claw_x(w);
        reached |= if left {
            x <= cx + ROLL_STOP_SHORT
        } else {
            x >= cx - ROLL_STOP_SHORT
        };
    }
    if reached {
        restore_hit_rect(w, id);
        fire(w, id);
        return;
    }
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let ex = exact_x(w, id);
    if (left && ex == f64::from(min)) || (!left && ex == f64::from(max)) {
        restore_hit_rect(w, id);
        fire(w, id);
    }
    if roll_over(w, id) {
        restore_hit_rect(w, id);
        fire(w, id);
    }
}

/// Starts a backward roll away from Claw (see the module documentation).
/// Draws: in the window, going Home's; on the bound behind him, the
/// shot's.
pub fn roll_backward(w: &mut World, id: ObjectId) {
    if in_crouch_window(w, id) {
        go_home(w, id);
        return;
    }
    face_claw(w, id);
    let right = !claw_left(w, id);
    let speed = f64::from(ROLL_SPEED);
    set_vx(w, id, if right { -speed } else { speed });
    let x = obj(w, id).x;
    if let Some(p) = peg_mut(w, id) {
        p.target = if right {
            x - ROLL_DISTANCE
        } else {
            x + ROLL_DISTANCE
        };
    }
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let ex = exact_x(w, id);
    if (right && ex == f64::from(min)) || (!right && ex == f64::from(max)) {
        fire(w, id);
        return;
    }
    play(w, ROLL_SOUND);
    set_state(w, id, EnemyState::RollBackward);
    set_anim(w, id, BACKWARD_ROLL_ANIM);
    start_roll_timer(w, id);
}

/// RollBackward: away from Claw; fires on its target, on either patrol
/// bound or once its time is up.
pub fn roll_backward_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let right = !claw_left(w, id);
    let speed = f64::from(ROLL_SPEED);
    set_vx(w, id, if right { -speed } else { speed });
    let x = obj(w, id).x;
    let t = target(w, id);
    if (right && x <= t) || (!right && x >= t) {
        fire(w, id);
        return;
    }
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let ex = exact_x(w, id);
    if ex == f64::from(min) || ex == f64::from(max) {
        fire(w, id);
    }
    if roll_over(w, id) {
        fire(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// How his melee rule groups Claw's blows by their kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeleeGroup {
    /// The standing and jumping stabs, punches, the uppercut and kicks:
    /// half of them land, all of them in the window.
    Standing,
    /// The crouching stab: it lands outside the window and opens it.
    Crouching,
    /// Anything else: the shared human rule.
    Other,
}

/// The group of a blow of Claw's of kind `kind`.
pub fn melee_group(kind: i32) -> MeleeGroup {
    match kind {
        attack_kind::SWIPE
        | 2
        | attack_kind::PUNCH
        | attack_kind::UPPERCUT
        | attack_kind::JUMP_SWIPE
        | attack_kind::KICK => MeleeGroup::Standing,
        attack_kind::DUCK_SWIPE => MeleeGroup::Crouching,
        _ => MeleeGroup::Other,
    }
}

/// `attacker` hit `id` (see the module documentation). Draws: by what hit,
/// as the module documentation lists, then the death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if matches!(
        super::state(w, id),
        Some(EnemyState::RollForward | EnemyState::RollBackward)
    ) {
        return;
    }
    let blow = w.objects.get(attacker).map_or(0, |a| a.special_flags);
    if let Some(d) = data_mut(w, id) {
        if blow & FIRE_BLOW != 0 {
            d.death_anim = FIRE_DEATH_ANIM;
        } else if blow & ICE_BLOW != 0 {
            d.death_anim = ICE_DEATH_ANIM;
        }
    }
    if !accept_hit(w, id) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    let now = w.now_ms;
    match kind {
        attacker::CLAW => blow_rule(w, id, attacker),
        // The shot does no harm and flies on.
        attacker::PISTOL_SHOT => roll_forward(w, id),
        attacker::MAGIC | attacker::THROWN => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            enter_hit(w, id, false);
        }
        attacker::LEVEL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, BLAST_COOLDOWN_MS);
            }
            enter_hit(w, id, false);
        }
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// A blow of Claw's himself: the cooldown first; in a flinch he only goes
/// Home; otherwise by the blow's group ([`melee_group`]).
fn blow_rule(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.cooldown = Timer::new(now, BLOW_COOLDOWN_MS);
    }
    if matches!(
        super::state(w, id),
        Some(EnemyState::HitLow | EnemyState::HitHigh)
    ) {
        go_home(w, id);
        return;
    }
    match melee_group(w.claw.attack.kind) {
        MeleeGroup::Standing => {
            let r = rand(w, 101);
            if r < BLOW_LANDS_BELOW || in_crouch_window(w, id) {
                enter_hit(w, id, true);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
            }
        }
        MeleeGroup::Crouching => {
            if !in_crouch_window(w, id) {
                enter_hit(w, id, false);
                if let Some(p) = peg_mut(w, id) {
                    p.crouch_window = Timer::new(now, u64::from(CROUCH_WINDOW_MS));
                }
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
            }
        }
        MeleeGroup::Other => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            let high = w.claw.attack.uppercut;
            enter_hit(w, id, high);
        }
    }
}

/// Lifted: one of his four lifted lines, always, and his hit box back.
/// The choice reads the draw the shared lift makes right after this step
/// (the original makes that one draw only), so the stream is left where it
/// was.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    let before = w.rng.state();
    let pick = w.rng.next_u15() % 4;
    *w.rng = Rng::new(before);
    say(w, id, LIFTED_LINES[pick as usize]);
    restore_hit_rect(w, id);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_rolls_are_in_the_ninth_levels_block() {
        let block = super::super::hook_pirate::STATE_BLOCK;
        assert_eq!(STATES.map(|s| s as i32), [2220, 2221]);
        for s in STATES {
            assert!(block.contains(&(s as i32)), "{}", s.name());
        }
        assert_eq!(EnemyState::Fire as i32, 2013);
        assert!(CLASS.human, "a human enemy");
        for s in [
            EnemyState::Home,
            EnemyState::Fire,
            EnemyState::RollForward,
            EnemyState::RollBackward,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(handler(EnemyState::Idle).is_none());
    }

    #[test]
    fn his_bullet_leaves_ahead_of_him_high_or_low() {
        assert_eq!(bullet::start(false, true), ((55, -24), 500));
        assert_eq!(bullet::start(true, true), ((55, 15), 500));
        assert_eq!(bullet::start(false, false), ((-55, -24), -500));
        assert_eq!(bullet::start(true, false), ((-55, 15), -500));
        // 9 px a tick, rounded.
        assert_eq!((bullet::SPEED * 18 + 500) / 1000, 9);
    }

    #[test]
    fn his_timings_at_eighteen_milliseconds() {
        // 150 px at 250 px/s: 34 ticks of 4.5 px.
        assert_eq!(
            ((ROLL_DISTANCE * 1000) as u32).div_ceil((ROLL_SPEED * 18) as u32),
            ROLL_TICKS
        );
        assert!(ROLL_TICKS < ROLL_MS.div_ceil(18));
        // The spell fires again whenever a shot ends inside its 3 s: the
        // sixth shot starts inside it and ends after it.
        let window = FIRE_WINDOW_MS.div_ceil(18);
        assert!((FIRE_WINDOW_SHOTS - 1) * FIRE_WINDOW_SHOT_TICKS < window);
        assert!(FIRE_WINDOW_SHOTS * FIRE_WINDOW_SHOT_TICKS >= window);
        assert_eq!(SHOT_TICKS.1, FIRE_WINDOW_SHOT_TICKS);
        assert!(aims_at_posture(9));
        assert!(!aims_at_posture(10));
    }

    #[test]
    fn a_forward_roll_stops_short_of_claw_on_the_ninth_level() {
        // Claw far to the right: the full 150 px.
        assert_eq!(forward_target(1000, 1400, false, true), (1150, false));
        // Claw 100 px to the right: 35 px short of him.
        assert_eq!(forward_target(1000, 1100, false, true), (1065, false));
        // Claw 30 px to the right: already there, fire at once.
        assert_eq!(forward_target(1000, 1030, false, true), (995, true));
        // To the left, the same mirrored.
        assert_eq!(forward_target(1000, 900, true, true), (935, false));
        assert_eq!(forward_target(1000, 980, true, true), (1015, true));
        // The tenth level rolls the full length, past Claw.
        assert_eq!(forward_target(1000, 1030, false, false), (1150, false));
    }

    #[test]
    fn his_melee_groups() {
        for k in [1, 2, 3, 4, 8, 13] {
            assert_eq!(melee_group(k), MeleeGroup::Standing, "{k}");
        }
        assert_eq!(melee_group(7), MeleeGroup::Crouching);
        for k in [0, 5, 6, 9, 10, 11, 12, 14, 20] {
            assert_eq!(melee_group(k), MeleeGroup::Other, "{k}");
        }
    }
}
