//! Wolvington, the boss of the sixth level: a wolf magistrate with a sword
//! and a cape who waits at the end of the port town, with his magic shot
//! ([`shot`]) and the light it carries ([`light`]).
//!
//! He is built like Raux and Katherine: a human enemy with the boss layer
//! on top, so he waits in the bosses' `Startup` until the boss stage's
//! introduction ends ([`crate::logic::boss_stage`]), goes back to where he
//! was placed with his health restored when Claw respawns, and releases
//! the gem that ends the level when he dies. Once the fight is on, at
//! range he walks in, sprints in for a running uppercut or stands and
//! throws a volley of magic shots straight across the room; up close he
//! answers a standing Claw with an uppercut and a crouching one with a
//! low sword thrust, or steps back. He hops back from most sword swings
//! aimed at him from close by, blocks shots, magic and thrown objects,
//! flinches from Claw's blows and then counters with a running uppercut.
//! Claw can lift and throw him, and every throw he survives ends in a
//! volley. He has [`HEALTH`] health; each of Claw's blows costs him 5 and
//! each of his hits costs Claw [`DAMAGE`].
//!
//! # The fight
//!
//! - His five range boxes are 40 px tall each way and 500, 300, 200, 56
//!   and 56 px wide each way ([`RANGES`]), so anywhere on his room's floor
//!   Claw is in range; Claw is out of range only more than 40 px above
//!   him.
//! - Home decides on the tick's roll (made on frame pulse ticks only):
//!   at range (modes 0 to 2) a roll below 100 starts a volley, below 300
//!   the sprint, below 1000 the walk at Claw, and anything else (every
//!   tick between pulses) waits; up close (modes 3 and 4) a roll below 500
//!   strikes (the thrust when Claw crouches, the uppercut otherwise) and
//!   anything else steps back; out of range he paces.
//! - Walking at Claw (60 px/s) he runs (120 px/s) when Claw is farther than
//!   300 px, and on a roll below 10 starts a volley, below 20 the sprint;
//!   within 56 px he comes Home.
//! - The sprint runs at 480 px/s on Claw's side until Claw is within
//!   56 px, then he uppercuts. Its animation plays four times as fast.
//! - A volley is one or more casts: each draw of 0..4 picks the next cast,
//!   high on an even draw and low on an odd one, and a 3 makes that cast
//!   the last. A cast's shot leaves on the cast's 16th tick, 65 px ahead
//!   of him and 20 px above his position (high) or 10 px below it (low),
//!   and flies straight at 300 px/s the way he faced when the cast
//!   started; the cast ends 34 ticks after it started, so a volley's shots
//!   are 35 ticks apart.
//! - The uppercut's box ([`UPPERCUT`]) is live on its 16th to 27th ticks
//!   and he is Home on the 37th; the thrust's ([`THRUST`]) on the 13th to
//!   22nd, Home on the 28th. Neither takes the level's strike token.
//! - When Claw, facing him within 56 px, swings his sword (standing,
//!   crouching or in the air), a draw of 0..100 below his smarts (99)
//!   makes him hop back in the tick's post-tick when he is free.
//! - Claw's blows make him flinch (a 300 ms cooldown); the flinch ends in
//!   the sprint with a 500 ms cooldown, and a blow during a flinch only
//!   ends it the same way. Pistol shots, sticks of dynamite, magic and
//!   thrown objects are destroyed and he blocks; explosions and the
//!   level's own shots do nothing to him.
//! - Lifted and thrown he loses 4 health when he comes to rest, and every
//!   throw he survives ends in a volley.
//! - Standing, he sinks a third of a pixel below his own pixel each tick
//!   once he has been in the air (see [`next_stand_fraction`]), and his
//!   hop back leaves the floor from there, so its arc matches the
//!   original's pixel for pixel.
//!
//! # Boss stage
//!
//! His level's door and stager ([`STAGE`]) are the shared stage pieces:
//! the door leads into the stager's box and the stager plays the
//! introduction. He makes himself the current boss when he is set up, and
//! the stager's health bar reads his health against [`METER_MAX`] (his
//! record's health is not used). When the fight flag reaches 2 he goes
//! idle (the shared human idle: no line, a draw of 0..100), Home on the
//! next tick and decides on the one after (on the first frame pulse tick).
//! His defeat goes through [`crate::logic::World::boss_defeated`], which
//! launches the gem to the point his record's speed fields name.
//!
//! # After a skipped introduction
//!
//! A Claw who jumps into the room on its first tick skips the
//! introduction. Nothing in Wolvington depends on it: with the fight flag
//! at 0 he stays in `Startup` until Claw acts on him (a blow, a swing that
//! makes him hop back, a shot he blocks, a lift), or Claw stays more than
//! 40 px above him for 500 ms with him on screen (he then goes idle and
//! fights), or Claw walks back into the stager's box, which plays the
//! whole introduction and wakes him at its end. With the usual jump of the
//! skip (Jump held 12 ticks) Claw comes back within 40 px of his height
//! two ticks before the half second; one tick more of Jump, or the same
//! jump with Catnip, wakes him in the air. Touching him while he sleeps
//! hurts Claw and leaves him asleep.
//!
//! # When Claw respawns
//!
//! Claw's respawn puts him back on his first call after it: at his place
//! on his floor with full health, in the state he was in (the boss reset
//! keeps state, timers and speed).
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits for the fight in
//! the bosses' `Startup`, idles in the shared human `Idle`, decides in
//! `Home` and walks in `Advance` with handlers of his own under those
//! numbers, and inherits the other shared human states with their
//! numbers. His five attacks are new names, the first five numbers of the
//! sixth level's block [`STATE_BLOCK`]: [`EnemyState::MagicVolley`] (the
//! choice of the next shot), [`EnemyState::MagicShot`] (one cast),
//! [`EnemyState::Uppercut`], [`EnemyState::SwordThrust`] and
//! [`EnemyState::Sprint`] (the run into the uppercut).
//!
//! In a reference trace of the original game on this level his states
//! carry other numbers; [`REFERENCE_STATE_IDS`] maps his to them, so a
//! comparison can line them up.
//!
//! # Random draws, in order
//!
//! Each tick (his logic call):
//!
//! 1. the shared pre-tick's roll of 0..=1000 (on frame pulse ticks);
//! 2. one of 0..100 when Claw swings his sword facing him from within
//!    56 px (modes 3 and 4), free or not;
//! 3. the state's draws: going idle one of 0..100; Idle one of 0..201;
//!    Home the entries' (a volley's one of 0..4, stepping back's one of
//!    0..51); Advance a volley's one of 0..4; the end of a cast with more
//!    to come one of 0..4; FastAdvance one, discarded; Pace one of 0..100
//!    per limit reached.
//!
//! Each of his shots draws two per sparkle (two sparkles) in its own
//! call. When hit by Claw's sword and not flinching: the hit burst's
//! three, then one of 0..4 for a high flinch's sound; at his death the
//! death's.
//! A lift draws one of 0..100 for his line.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   sixth level owns 2160 to 2179, his five attacks 2160 to 2164 in the
//!   order the game registers them; the goo vents and the ground blowers
//!   take none.
//! - The shot's light is drawn as the magic claw's light flare is: kept
//!   hidden until objects can ask for a light blend, following its shot.
//! - The sprint's animation speed and the animation he is left without
//!   after a throw only change what is drawn.
//!
//! His data ([`WolvingtonData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; Wolvington then wraps it in his own.

use std::ops::RangeInclusive;

use super::human::{self, BossFields, HumanAnims, HumanCall, HumanData, HumanSounds};
use super::lift::Carry;
use super::states::{self, enter_hit, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Shot, Timer,
    accept_hit, attacker, build, data, data_mut, die, face_claw, faces_right, facing_rect,
    hit_burst, human_fields, obj, obj_mut, play, rand, scaled, set_anim, set_state, take_damage,
};
use crate::logic::projectile::{self, ProjectileSpec};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage};
use crate::object::{ObjectId, Rect, engine_state, flags, types};
use crate::physics::ptype;
use crate::player::attack::kind as attack_kind;
use crate::player::state as claw_state;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Wolvington"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_WOLVINGTON";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_WOLVINGTON_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the sixth level's families. His five
/// attacks take the first five ([`EnemyState::MagicVolley`] to
/// [`EnemyState::Sprint`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2160..=2179;

/// His states and the numbers a reference trace of the original game on
/// this level gives them.
pub const REFERENCE_STATE_IDS: [(EnemyState, i32); 8] = [
    (EnemyState::Startup, 2028),
    (EnemyState::Home, 2018),
    (EnemyState::Advance, 2021),
    (EnemyState::MagicVolley, 2047),
    (EnemyState::MagicShot, 2048),
    (EnemyState::Uppercut, 2049),
    (EnemyState::SwordThrust, 2050),
    (EnemyState::Sprint, 2051),
];

/// The pieces of the boss stage on his level: the door into his room and
/// the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor (the record's health is not
/// used), which is also the health a full health bar stands for.
pub const HEALTH: i32 = 100;

/// See [`HEALTH`].
pub const METER_MAX: i32 = HEALTH;

/// Damage of his strikes and of touching him, before the difficulty
/// factor; his shot deals the same, unscaled.
pub const DAMAGE: i32 = 5;

/// Smarts base (raised to 99 like every enemy's below 100): the dodge
/// rolls against it.
pub const SMARTS: i32 = 50;

/// Walking at Claw.
pub const ADVANCE_ANIM: &str = "LEVEL_WOLVINGTON_ADVANCE";
/// Running at Claw, pacing, and the sprint.
pub const FAST_ADVANCE_ANIM: &str = "LEVEL_WOLVINGTON_FASTADVANCE";
/// Low hit reaction.
pub const HIT_LOW_ANIM: &str = "LEVEL_WOLVINGTON_HITLOW";
/// High hit reaction.
pub const HIT_HIGH_ANIM: &str = "LEVEL_WOLVINGTON_HITHIGH";
/// Idle, and waiting for the fight.
pub const IDLE_ANIM: &str = "LEVEL_WOLVINGTON_IDLE";
/// The hop back.
pub const JUMP_ANIM: &str = "LEVEL_WOLVINGTON_JUMP";
/// Falling, and the body flying off after death.
pub const FALL_ANIM: &str = "LEVEL_WOLVINGTON_KILLFALL";
/// Being lifted.
pub const LIFTED_ANIM: &str = "LEVEL_WOLVINGTON_LIFTED";
/// Held overhead, and flying after a throw.
pub const LIFTED2_ANIM: &str = "LEVEL_WOLVINGTON_LIFTED2";
/// Bouncing after a throw.
pub const THROWN_ANIM: &str = "LEVEL_WOLVINGTON_THROWN";
/// Both blocks.
pub const BLOCK_ANIM: &str = "LEVEL_WOLVINGTON_BLOCK";
/// The low sword thrust.
pub const THRUST_ANIM: &str = "LEVEL_WOLVINGTON_STRIKE1";
/// The uppercut.
pub const UPPERCUT_ANIM: &str = "LEVEL_WOLVINGTON_STRIKE2";
/// The high cast.
pub const HIGH_CAST_ANIM: &str = "LEVEL_WOLVINGTON_STRIKE3";
/// The low cast.
pub const LOW_CAST_ANIM: &str = "LEVEL_WOLVINGTON_STRIKE4";

/// His death sound, also at a death by a throw's landing.
pub const DEATH_SOUND: &str = "LEVEL_WOLVINGTON_00880003";
/// Said a third of the time when lifted, on screen.
pub const LIFTED_VOICE: &str = "LEVEL_WOLVINGTON_00880007";
/// The hop back's sound.
pub const JUMP_SOUND: &str = "LEVEL_WOLVINGTON_00880005";
/// Both blocks' sound.
pub const BLOCK_SOUND: &str = "LEVEL_BLOCK2";
/// The uppercut's sound, at its start.
pub const UPPERCUT_SOUND: &str = "LEVEL_WOLVINGTON_UPPERCUT";
/// The high cast's sound, at its start.
pub const HIGH_CAST_SOUND: &str = "LEVEL_WOLVINGTON_WMAGIC";
/// The low cast's sound, at its start.
pub const LOW_CAST_SOUND: &str = "LEVEL_WOLVINGTON_WMAGICLOW";

/// The uppercut's box, facing left: 36 to 59 px in front of him, from 63
/// to 8 px above his position.
pub const UPPERCUT: Rect = Rect::new(-59, -63, -36, -8);
/// The low thrust's box, facing left: 24 to 95 px in front of him, from
/// 1 to 42 px below his position.
pub const THRUST: Rect = Rect::new(-95, 1, -24, 42);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (56, 40), (56, 40)];

/// Home at range: a roll below this starts a volley.
pub const VOLLEY_ROLL: i32 = 100;
/// Home at range: a roll below this (and not below [`VOLLEY_ROLL`])
/// starts the sprint.
pub const SPRINT_ROLL: i32 = 300;
/// Home at range: a roll below this (and not below [`SPRINT_ROLL`])
/// walks at Claw.
pub const ADVANCE_ROLL: i32 = 1000;
/// Home up close: a roll below this strikes; any other steps back.
pub const STRIKE_ROLL: i32 = 500;
/// Walking at Claw: a roll below this starts a volley.
pub const ADVANCE_VOLLEY_ROLL: i32 = 10;
/// Walking at Claw: a roll below this (and not below
/// [`ADVANCE_VOLLEY_ROLL`]) starts the sprint.
pub const ADVANCE_SPRINT_ROLL: i32 = 20;
/// Number of volley draws: 0..4, an odd one casts low, 3 casts the last.
pub const VOLLEY_SPREAD: u32 = 4;
/// The volley draw that makes its cast the last.
pub const LAST_CAST: i32 = 3;
/// The sprint's speed, px/s.
pub const SPRINT_SPEED: f64 = 480.0;
/// The sprint's animation speed factor (four times as fast).
pub const SPRINT_ANIM_SPEED: f32 = 0.25;
/// The hit cooldown after a flinch, before the sprint, ms.
pub const COUNTER_COOLDOWN_MS: u64 = 500;
/// The hit cooldown of a sword blow, ms.
pub const HIT_COOLDOWN_MS: u64 = 300;

/// The Wolvington class.
pub static CLASS: Class = Class {
    name: "Wolvington",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle: human::go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (10, -10, -45),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice: super::nothing,
    lift_roll: true,
    anims: ClassAnims {
        advance: ADVANCE_ANIM,
        fast_advance: FAST_ADVANCE_ANIM,
        pace: FAST_ADVANCE_ANIM,
        walk: "",
        hit_low: HIT_LOW_ANIM,
        hit_high: HIT_HIGH_ANIM,
        // He has no pose of his own for standing after a throw: the
        // animation is left empty until the volley's cast sets one.
        home: "",
        jump: JUMP_ANIM,
        fall: FALL_ANIM,
        lifted: LIFTED_ANIM,
        lifted2: LIFTED2_ANIM,
        bounce: THROWN_ANIM,
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
};

/// Wolvington's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WolvingtonData {
    /// The data every enemy keeps. What he keeps of his own lives there
    /// too: his [`Fields`] in [`EnemyFields::wolvington`], the cast's shot
    /// in [`EnemyFields::shot`] and whether it has left in
    /// [`EnemyFields::shots`].
    pub enemy: EnemyData,
}

/// What Wolvington keeps beyond the shared enemy data.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fields {
    /// The volley goes on after the cast under way.
    pub volley_on: bool,
    /// The last volley draw (0..4), counted up as it is used.
    pub volley_draw: i32,
    /// How far below his whole-pixel height he really stands, px (see
    /// [`next_stand_fraction`]); 0 while he is not standing.
    pub stand_y: f64,
    /// Whether gravity pulls on him while he stands, so that
    /// [`Self::stand_y`] changes from tick to tick: from his first time in
    /// the air or Claw's first respawn on.
    pub falls: bool,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::MagicVolley => Some(volley_tick),
        EnemyState::MagicShot => Some(cast_tick),
        EnemyState::Uppercut => Some(uppercut_tick),
        EnemyState::SwordThrust => Some(thrust_tick),
        EnemyState::Sprint => Some(sprint_tick),
        _ => None,
    }
}

/// The number a reference trace of the original game gives his state
/// `s`, when it differs from the state's own number.
pub fn reference_state_id(s: EnemyState) -> Option<i32> {
    REFERENCE_STATE_IDS
        .iter()
        .find(|(state, _)| *state == s)
        .map(|&(_, id)| id)
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the reset when Claw has respawned, the human logic call
/// ([`human::run`]), then his death's report. Being removed, he stops
/// being the current boss.
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING && w.current_boss() == Some(id) {
        w.set_current_boss(None);
    }
    if EnemyState::from_id(raw).is_some() && claw_respawned(w, id) {
        reset(w, id);
    }
    run(w, id);
    after_call(w, id);
}

/// The human logic call ([`human::run`]) with his post-tick, which keeps
/// the part of a pixel he stands below his own (see
/// [`next_stand_fraction`]) and leaves the floor from there when he hops
/// back. The first call (building), a kill by another logic and the
/// removal go through the shared logic call unchanged.
fn run(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if matches!(
        raw,
        engine_state::INIT | engine_state::DESTROYING | engine_state::KILLED
    ) || EnemyState::from_id(raw).is_none()
    {
        super::tick(w, id, &CLASS);
        return;
    }
    super::pre_tick(w, id, &CLASS);
    pre_tick(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if let Some(h) = handler(s).or_else(|| states::shared_handler(s, true)) {
        h(w, id);
    }
    post_tick(w, id);
}

/// The post-tick: a block's release before a scheduled call (as
/// [`human::run`] does), the scheduled call, then the movement step,
/// noting where it started and whether the tiles stopped it, and his
/// standing fraction. A hop back leaves from his exact position.
fn post_tick(w: &mut World, id: ObjectId) {
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    if deferred.is_some() {
        let now = super::state(w, id);
        let armed = data_mut(w, id).and_then(|d| d.human.release_in.take());
        if armed.is_some() && armed == now {
            obj_mut(w, id).flags &= !flags::SAFE;
            if let Some(d) = data_mut(w, id) {
                d.busy = false;
            }
        }
    }
    let before = super::state(w, id);
    match deferred {
        Some(Deferred::GoIdle) => human::go_idle(w, id),
        Some(Deferred::JumpCheck) => states::jump_check(w, id, &CLASS),
        Some(Deferred::Dropped) => super::lift::let_go(w, id),
        Some(Deferred::Human(call)) => human::run_deferred(w, id, &CLASS, call),
        None => {}
    }
    if before != Some(EnemyState::JumpBack) && super::state(w, id) == Some(EnemyState::JumpBack) {
        leave_floor(w, id);
    }
    let (x, stood) = {
        let o = obj(w, id);
        (o.x, o.physics_type == ptype::STAND)
    };
    if let Some(d) = data_mut(w, id) {
        d.human.pre_move_x = x;
    }
    super::motion::integrate(w, id);
    human::note_stop(w, id);
    let stands = obj(w, id).physics_type == ptype::STAND;
    if let Some(d) = data_mut(w, id) {
        let ay = d.ay;
        let f = &mut d.wolvington;
        if stood && stands {
            f.stand_y = if f.falls {
                next_stand_fraction(f.stand_y, ay)
            } else {
                0.0
            };
        } else if stands {
            f.stand_y = LANDED_FRACTION;
        } else {
            f.stand_y = 0.0;
            f.falls = true;
        }
    }
}

/// He leaves the floor from exactly where he stands: across, with the
/// part of a pixel his last move left; up and down, the part of a pixel he
/// stands below his whole pixel.
fn leave_floor(w: &mut World, id: ObjectId) {
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    let fx = super::exact_x(w, id) - f64::from(x);
    if let Some(d) = data_mut(w, id) {
        let fy = d.wolvington.stand_y;
        d.carry = (fx, fy);
        d.moved_to = Some((x, y));
        d.wolvington.falls = true;
    }
}

/// How far below his whole pixel he stands on the tick he lands, px: the
/// last step of the cycle of [`next_stand_fraction`], so he is back on
/// his own pixel the tick after.
///
/// Inferred: with it every hop back of the reference runs leaves from the
/// part of a pixel the original's arc shows, and before his first time in
/// the air he stands exactly on his pixel.
pub const LANDED_FRACTION: f64 = 0.972;

/// His exact height below his whole pixel after one more tick standing,
/// from `f` (px): each tick's gravity moves his exact position down by
/// half a tick's fall from rest (0.324 px at 2000 px/s^2) while the floor
/// holds his pixel, until the exact position reaches the next pixel and
/// the floor puts him back on his own: 0, 0.324, 0.648, 0.972, then 0
/// again.
pub fn next_stand_fraction(f: f64, ay: f64) -> f64 {
    let next = f + 0.5 * ay * super::TICK_S * super::TICK_S;
    if next < 1.0 { next } else { 0.0 }
}

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: during a flinch he only goes Home (which is the sprint,
///   with a 500 ms cooldown); otherwise he takes the damage, shows a hit
///   burst, starts a 300 ms cooldown and flinches (high after an
///   uppercut);
/// - a pistol shot (or dynamite), magic or a thrown object: it is
///   destroyed and he blocks low, unhurt;
/// - an explosion or another shot of the level: nothing;
/// - a springboard: thrown toward its target, as any human enemy.
///
/// At 0 health or below he dies (and the death is reported).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
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
    match kind {
        attacker::CLAW => {
            if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else {
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                let now = w.now_ms;
                if let Some(d) = data_mut(w, id) {
                    d.cooldown = Timer::new(now, HIT_COOLDOWN_MS);
                }
                let high = w.claw.attack.uppercut;
                enter_hit(w, id, high);
            }
        }
        attacker::PISTOL_SHOT | attacker::MAGIC | attacker::THROWN => {
            w.destroy(attacker);
            human::enter_block(w, id, false);
        }
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
        boss_died(w, id);
    }
}

/// `id` attacked `victim`: nothing more than any enemy.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// What follows each logic call: a death seen for the first time (a
/// throw's landing, or a kill by another logic) is reported, a landing's
/// with his death sound. He never shows the health bar himself: the bar
/// the introduction makes reads his health (see
/// [`crate::logic::boss_stage`]).
fn after_call(w: &mut World, id: ObjectId) {
    let Some((dead, seen, kind)) = data(w, id).map(|d| (d.dead, boss(d).death_seen, d.death))
    else {
        return;
    };
    if !dead || seen || super::state(w, id) != Some(EnemyState::Dead) {
        return;
    }
    if kind == DeathKind::Landing {
        play(w, DEATH_SOUND);
    }
    boss_died(w, id);
}

fn boss(d: &EnemyFields) -> BossFields {
    d.human.boss.unwrap_or_default()
}

fn with_boss(w: &mut World, id: ObjectId, f: impl FnOnce(&mut BossFields)) {
    if let Some(b) = data_mut(w, id).and_then(|d| d.human.boss.as_mut()) {
        f(b);
    }
}

/// He died: when Claw is alive, his defeat is reported (once).
fn boss_died(w: &mut World, id: ObjectId) {
    let claw_alive = w.claw.stats.health > 0;
    let mut report = false;
    with_boss(w, id, |b| {
        b.death_seen = true;
        if claw_alive && !b.defeat_reported {
            b.defeat_reported = true;
            report = true;
        }
    });
    if report {
        w.boss_defeated(id);
    }
}

/// True once per death of Claw, from his respawn on: his attempt number
/// has moved on since the last reset and he is no longer dying.
fn claw_respawned(w: &World, id: ObjectId) -> bool {
    let dying = w.claw_object().is_none_or(|c| {
        matches!(
            c.state,
            claw_state::DEATH_FALL | claw_state::DEATH | claw_state::INIT
        )
    });
    let attempt = w.attempt();
    !dying && data(w, id).is_some_and(|d| d.human.boss.is_some_and(|b| b.attempt != attempt))
}

/// The reset when Claw respawns: back where he was placed with full
/// health, standing on the floor below that point when he stands (his
/// record places him 1 px above it); his state, speed, timers and busy
/// flag stay as they are.
pub fn reset(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    with_boss(w, id, |b| b.attempt = attempt);
    if let Some(d) = data_mut(w, id) {
        d.wolvington.stand_y = 0.0;
        d.wolvington.falls = true;
    }
    let map = &w.level.map;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.x = b.start.0;
    o.y = b.start.1;
    o.health = b.full_health;
    if o.physics_type == ptype::STAND {
        crate::physics::align_to_ground(map, o);
    }
}

// ----------------------------------------------------------------------
// Building

/// Builds Wolvington: the current boss, his place kept for the respawn
/// reset, his boxes, 100 health and 5 damage (times the difficulty
/// factors), smarts 99, liftable, always active; then he drops to the
/// floor and waits for the fight in `Startup`. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let start = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    w.set_current_boss(Some(id));
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: UPPERCUT,
        strike_anim: UPPERCUT_ANIM,
        death_anim: FALL_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: LIFTED_VOICE,
        smarts: super::smarts(SMARTS),
        may_lift: true,
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_ANIM,
                block_high: BLOCK_ANIM,
                block_low: BLOCK_ANIM,
                idle: IDLE_ANIM,
            },
            sounds: HumanSounds {
                jump: JUMP_SOUND,
                block_high: BLOCK_SOUND,
                block_low: BLOCK_SOUND,
                idle_voice: "",
                strike: "",
            },
            boss: Some(BossFields {
                start,
                full_health,
                attempt: w.attempt(),
                ..BossFields::default()
            }),
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
    let o = obj_mut(w, id);
    o.flags |= flags::ALWAYS_ACTIVE;
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Wolvington(WolvingtonData { enemy });
    }
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

// ----------------------------------------------------------------------
// Pre-tick

/// The kinds of Claw's attack that make him hop back: the sword swings,
/// standing, crouching and in the air.
pub fn draws_dodge(kind: i32) -> bool {
    matches!(
        kind,
        attack_kind::SWIPE | 2 | attack_kind::DUCK_SWIPE | attack_kind::JUMP_SWIPE
    )
}

/// His pre-tick step after the shared ones, the dodge: when Claw swings
/// his sword facing him from within 56 px (mode 3 or 4), a draw of 0..100
/// below his smarts schedules the hop back for the post-tick when he is
/// free. Draws: that one of 0..100, made free or not.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let kind = w.claw.attack.kind;
    let claw_facing_right = w.claw.facing_right;
    let Some((right, mode, busy, smarts)) =
        data(w, id).map(|d| (d.claw_right, d.mode, d.busy, d.smarts))
    else {
        return;
    };
    let faces_him = claw_facing_right != right;
    if kind == attack_kind::NONE || !faces_him || !draws_dodge(kind) || !(mode == 3 || mode == 4) {
        return;
    }
    if rand(w, 100) < smarts
        && !busy
        && let Some(d) = data_mut(w, id)
    {
        d.deferred = Some(Deferred::Human(HumanCall::JumpBack));
    }
}

// ----------------------------------------------------------------------
// Going Home

/// Goes Home, by the state he leaves: from a flinch, a 500 ms hit cooldown
/// and the sprint (his counter); from a throw's last bounce, a volley;
/// from anything else he stops across and decides again at Home.
pub fn go_home(w: &mut World, id: ObjectId) {
    match super::state(w, id) {
        Some(EnemyState::HitLow | EnemyState::HitHigh) => {
            let now = w.now_ms;
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, COUNTER_COOLDOWN_MS);
            }
            enter_sprint(w, id);
        }
        Some(EnemyState::Bounce) => enter_volley(w, id),
        _ => {
            if let Some(d) = data_mut(w, id) {
                d.vx = 0.0;
            }
            set_state(w, id, EnemyState::Home);
        }
    }
}

// ----------------------------------------------------------------------
// States

/// Startup: waits, idle, until the fight is on; then he goes idle (the
/// shared human idle).
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        human::go_idle(w, id);
    }
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

fn roll(w: &World, id: ObjectId) -> i32 {
    data(w, id).map_or(i32::MAX, |d| d.roll)
}

fn claw_ducks(w: &World) -> bool {
    w.claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK)
}

/// Home: faces Claw, then by mode on the tick's roll (no new draw): at
/// range (0 to 2) a volley, the sprint or the walk at Claw (below 100,
/// 300, 1000), otherwise nothing; up close (3, 4) a strike below 500 (the
/// thrust when Claw crouches, else the uppercut), otherwise a step back;
/// out of range (5, 6) pacing. A step back that cannot be made leaves him
/// Home to decide again next tick.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let r = roll(w, id);
    match mode(w, id) {
        0..=2 => {
            if (0..VOLLEY_ROLL).contains(&r) {
                enter_volley(w, id);
            } else if (VOLLEY_ROLL..SPRINT_ROLL).contains(&r) {
                enter_sprint(w, id);
            } else if (SPRINT_ROLL..ADVANCE_ROLL).contains(&r) {
                states::enter_advance(w, id);
            }
        }
        3 | 4 => {
            if (0..STRIKE_ROLL).contains(&r) {
                if claw_ducks(w) {
                    enter_thrust(w, id);
                } else {
                    enter_uppercut(w, id);
                }
            } else {
                human::enter_move_back(w, id);
            }
        }
        _ => start_pacing(w, id),
    }
}

/// Advance: at a patrol limit he goes idle (and carries on); he faces
/// Claw and walks at him at 60 px/s; then by mode he runs (0), on a roll
/// below 10 starts a volley or below 20 the sprint (1, 2), or goes Home
/// (3 to 6).
pub fn advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = obj(w, id).x;
    if x == min || x == max {
        human::go_idle(w, id);
    }
    face_claw(w, id);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    if let Some(d) = data_mut(w, id) {
        d.vx = if right {
            states::ADVANCE_SPEED
        } else {
            -states::ADVANCE_SPEED
        };
    }
    w.advance_anim(id);
    let r = roll(w, id);
    match mode(w, id) {
        0 => states::enter_fast_advance(w, id),
        1 | 2 => {
            if (0..ADVANCE_VOLLEY_ROLL).contains(&r) {
                enter_volley(w, id);
            } else if (ADVANCE_VOLLEY_ROLL..ADVANCE_SPRINT_ROLL).contains(&r) {
                enter_sprint(w, id);
            }
        }
        _ => go_home(w, id),
    }
}

// ----------------------------------------------------------------------
// The magic volley

/// Starts (or goes on with) a volley: he stops and draws the next cast.
/// Draws: one of 0..4.
pub fn enter_volley(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::MagicVolley);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    let k = rand(w, VOLLEY_SPREAD);
    if let Some(d) = data_mut(w, id) {
        d.wolvington.volley_on = true;
        d.wolvington.volley_draw = k;
    }
}

/// The cast the volley draw `k` asks for: high on an even draw, low on an
/// odd one.
pub fn cast_is_high(k: i32) -> bool {
    k % 2 == 0
}

/// MagicVolley, one tick: a draw of 3 sends him Home and ends the volley
/// after this cast; then he casts high or low by the draw. The animation
/// does not advance.
pub fn volley_tick(w: &mut World, id: ObjectId) {
    let k = data(w, id).map_or(LAST_CAST, |d| d.wolvington.volley_draw);
    if k == LAST_CAST {
        go_home(w, id);
        if let Some(d) = data_mut(w, id) {
            d.wolvington.volley_on = false;
        }
    }
    if let Some(d) = data_mut(w, id) {
        d.wolvington.volley_draw += 1;
    }
    enter_cast(w, id, cast_is_high(k));
}

/// Starts a cast, high or low: its animation and sound, and the shot it
/// will release, which leaves 65 px ahead of him the way he faces now, 20
/// px above his position (high) or 10 px below it (low), at 300 px/s.
pub fn enter_cast(w: &mut World, id: ObjectId, high: bool) {
    set_state(w, id, EnemyState::MagicShot);
    let right = faces_right(obj(w, id));
    let (anim, sound) = if high {
        (HIGH_CAST_ANIM, HIGH_CAST_SOUND)
    } else {
        (LOW_CAST_ANIM, LOW_CAST_SOUND)
    };
    set_anim(w, id, anim);
    play(w, sound);
    let (dx, dy) = shot::spawn_offset(right, high);
    if let Some(d) = data_mut(w, id) {
        d.shot = Shot {
            dx,
            dy,
            vx: shot::speed(right),
            vy: 0,
        };
        d.shots = 0;
    }
}

/// MagicShot: faces Claw; advances the cast; on its first shot frame the
/// shot leaves (once per cast); when the cast ends, the volley goes on or
/// he goes Home. Draws: the volley's one of 0..4 when it goes on.
pub fn cast_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let event = w.advance_anim(id);
    if event == 1 && data(w, id).is_some_and(|d| d.shots == 0) {
        if let Some(d) = data_mut(w, id) {
            d.shots = 1;
        }
        fire(w, id);
    }
    if obj(w, id).anim.is_done() {
        if data(w, id).is_some_and(|d| d.wolvington.volley_on) {
            enter_volley(w, id);
        } else {
            go_home(w, id);
        }
    }
}

/// Releases the cast's shot one depth behind him.
fn fire(w: &mut World, id: ObjectId) {
    let Some(s) = data(w, id).map(|d| d.shot) else {
        return;
    };
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    projectile::spawn(
        w,
        ProjectileSpec {
            logic: shot::NAMES[0],
            x: x + s.dx,
            y: y + s.dy,
            z: z - 1,
            vx: s.vx,
            vy: s.vy,
            damage: shot::DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: shot::IMAGE_SET,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
}

// ----------------------------------------------------------------------
// Strikes

/// Starts the uppercut: its sound, and he stops.
pub fn enter_uppercut(w: &mut World, id: ObjectId) {
    play(w, UPPERCUT_SOUND);
    set_state(w, id, EnemyState::Uppercut);
    set_anim(w, id, UPPERCUT_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
}

/// Starts the low thrust.
pub fn enter_thrust(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::SwordThrust);
    set_anim(w, id, THRUST_ANIM);
}

/// A strike's tick: advances its animation; on every strike frame `rect`
/// is his attack box for that tick; when the animation ends he goes Home.
fn strike(w: &mut World, id: ObjectId, rect: Rect) {
    let event = w.advance_anim(id);
    if event == 1 {
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, rect);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

/// Uppercut: [`UPPERCUT`] on its strike frames, then Home.
pub fn uppercut_tick(w: &mut World, id: ObjectId) {
    strike(w, id, UPPERCUT);
}

/// SwordThrust: [`THRUST`] on its strike frames, then Home.
pub fn thrust_tick(w: &mut World, id: ObjectId) {
    strike(w, id, THRUST);
}

// ----------------------------------------------------------------------
// The sprint

/// Starts the sprint: he runs at Claw, facing him, with his running
/// animation played four times as fast.
pub fn enter_sprint(w: &mut World, id: ObjectId) {
    states::enter_fast_advance(w, id);
    set_state(w, id, EnemyState::Sprint);
    obj_mut(w, id).anim.set_speed(SPRINT_ANIM_SPEED);
}

/// Sprint: advances the animation; at range or out of range on screen
/// (modes 0, 1, 2, 5) he runs at 480 px/s on Claw's side, keeping the way
/// he faces; within 56 px (3, 4) he uppercuts; off screen (6) nothing
/// changes.
pub fn sprint_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    match mode(w, id) {
        0 | 1 | 2 | 5 => {
            if let Some(d) = data_mut(w, id) {
                d.vx = if d.claw_right {
                    SPRINT_SPEED
                } else {
                    -SPRINT_SPEED
                };
            }
        }
        3 | 4 => enter_uppercut(w, id),
        _ => {}
    }
}

/// His magic shot: a ball of light cast from his hand that flies straight
/// across the room without falling, hurts Claw (5, whatever the
/// difficulty) and is gone, breaks the crates it passes and flies on, and
/// is ignored by enemies, kegs and Wolvington himself. A wall stops it on
/// the tick after it touches one. It sheds two sparkles a tick and carries
/// a light ([`super::light`]).
///
/// It flies as the projectiles do ([`crate::logic::projectile`]): the
/// shot is set up there and each call runs there.
pub mod shot {
    use crate::logic::{World, projectile};
    use crate::object::{ObjectId, Rect};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["WolvingtonBullet"];

    /// The shot's picture.
    pub const IMAGE_SET: &str = "LEVEL_WOLVINGTONMAGIC";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// The shot's animation.
    pub const ANIM: &str = "GAME_CYCLE50";

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[ANIM];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = true;

    /// Damage to Claw.
    pub const DAMAGE: i32 = 5;
    /// How far ahead of Wolvington a shot leaves, px.
    pub const AHEAD: i32 = 65;
    /// Height of a high shot's start from his position, px.
    pub const HIGH_Y: i32 = -20;
    /// Height of a low shot's start from his position, px.
    pub const LOW_Y: i32 = 10;
    /// Speed of a shot, px/s.
    pub const SPEED: i32 = 300;
    /// Sparkles a shot sheds per tick.
    pub const SPARKLES: u32 = 2;
    /// The shot's box, flying right (its first frame's), when the level
    /// does not know that frame; turned to face left for a shot flying
    /// left.
    pub const FRAME_BOX: Rect = Rect::new(-29, -18, 38, 15);

    /// Where a shot leaves, from Wolvington's position, when he faces
    /// right (`facing_right`) or left, high or low.
    pub fn spawn_offset(facing_right: bool, high: bool) -> (i32, i32) {
        let x = if facing_right { AHEAD } else { -AHEAD };
        (x, if high { HIGH_Y } else { LOW_Y })
    }

    /// The shot's horizontal speed, px/s, when he faces right or left.
    pub fn speed(facing_right: bool) -> i32 {
        if facing_right { SPEED } else { -SPEED }
    }

    /// A shot's data. A shot keeps the projectiles' data
    /// ([`projectile::ProjectileData`]); this is not used.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct ShotData {}

    /// One logic call: the projectiles' ([`projectile::tick`]).
    pub fn tick(w: &mut World, id: ObjectId) {
        projectile::tick(w, id);
    }

    /// `attacker` hit `id`. Nothing hits a shot.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. The victim's own hit handler decides.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

/// The light a magic shot carries: one picture drawn see-through just
/// behind the shot, following it and removed with it. It never collides.
/// It is kept hidden, as the magic claw's light flare is, until objects
/// can ask for a light blend.
pub mod light {
    use crate::level::runtime_image_set;
    use crate::logic::{ObjectData, World, new_object};
    use crate::object::{ObjectId, draw_flags, flags};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["WolvingtonLFX"];

    /// The light's picture.
    pub const IMAGE_SET: &str = "GAME_LIGHTFX_WOLVINGTONLFX";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[IMAGE_SET];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = true;

    /// A light's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct LightData {
        /// The shot it rides on.
        pub owner: Option<ObjectId>,
    }

    /// Makes the light of shot `owner` at (x, y, z), facing left when
    /// `left`.
    pub fn spawn(
        w: &mut World,
        owner: ObjectId,
        (x, y, z): (i32, i32, i32),
        left: bool,
    ) -> ObjectId {
        let mut o = new_object(NAMES[0], x, y, z);
        o.flags |= flags::NO_COLLIDE | flags::ALWAYS_ACTIVE;
        o.image_set = runtime_image_set(IMAGE_SET);
        o.draw_flags |= draw_flags::HIDDEN;
        if left {
            o.draw_flags |= draw_flags::MIRROR;
        }
        o.data = ObjectData::WolvingtonLight(LightData { owner: Some(owner) });
        let id = w.spawn(o);
        w.set_anim(id, IMAGE_SET);
        id
    }

    /// One logic call: gone once its shot is gone or going.
    pub fn tick(w: &mut World, id: ObjectId) {
        let owner = match w.objects.get(id).map(|o| &o.data) {
            Some(ObjectData::WolvingtonLight(d)) => d.owner,
            _ => None,
        };
        if let Some(owner) = owner
            && w.objects
                .get(owner)
                .is_none_or(|o| o.flags & flags::DESTROY != 0)
        {
            w.destroy(id);
        }
    }

    /// `attacker` hit `id`. Nothing hits a light.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. A light attacks nothing.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_map_to_the_reference_numbers() {
        assert_eq!(reference_state_id(EnemyState::Startup), Some(2028));
        assert_eq!(reference_state_id(EnemyState::Home), Some(2018));
        assert_eq!(reference_state_id(EnemyState::MagicVolley), Some(2047));
        assert_eq!(reference_state_id(EnemyState::Sprint), Some(2051));
        assert_eq!(reference_state_id(EnemyState::Idle), None);
    }

    #[test]
    fn his_attacks_open_the_sixth_levels_block_in_order() {
        let attacks = [
            EnemyState::MagicVolley,
            EnemyState::MagicShot,
            EnemyState::Uppercut,
            EnemyState::SwordThrust,
            EnemyState::Sprint,
        ];
        for (i, s) in attacks.into_iter().enumerate() {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
            assert_eq!(s as i32, STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
    }

    #[test]
    fn a_shot_leaves_ahead_of_him_high_or_low() {
        assert_eq!(shot::spawn_offset(false, true), (-65, -20));
        assert_eq!(shot::spawn_offset(false, false), (-65, 10));
        assert_eq!(shot::spawn_offset(true, true), (65, -20));
        assert_eq!(shot::spawn_offset(true, false), (65, 10));
        assert_eq!(shot::speed(true), 300);
        assert_eq!(shot::speed(false), -300);
        // 5.4 px per 18 ms tick.
        assert_eq!(shot::SPEED * 18, 5400);
    }

    #[test]
    fn even_draws_cast_high_odd_ones_low_and_three_is_the_last() {
        assert!(cast_is_high(0));
        assert!(!cast_is_high(1));
        assert!(cast_is_high(2));
        assert!(!cast_is_high(LAST_CAST), "the last cast is always low");
    }

    #[test]
    fn only_sword_swings_draw_his_hop_back() {
        for k in [
            attack_kind::SWIPE,
            attack_kind::DUCK_SWIPE,
            attack_kind::JUMP_SWIPE,
        ] {
            assert!(draws_dodge(k), "{k}");
        }
        for k in [
            attack_kind::PUNCH,
            attack_kind::UPPERCUT,
            attack_kind::KICK,
            attack_kind::PISTOL,
            attack_kind::MAGIC,
            attack_kind::DYNAMITE,
            attack_kind::NONE,
        ] {
            assert!(!draws_dodge(k), "{k}");
        }
    }

    #[test]
    fn his_strike_boxes_mirror_when_he_faces_right() {
        assert_eq!(UPPERCUT.mirrored(), Rect::new(36, -63, 59, -8));
        assert_eq!(THRUST.mirrored(), Rect::new(24, 1, 95, 42));
    }

    #[test]
    fn he_has_his_own_handlers_for_his_own_states() {
        for s in [
            EnemyState::Startup,
            EnemyState::Home,
            EnemyState::Advance,
            EnemyState::MagicVolley,
            EnemyState::MagicShot,
            EnemyState::Uppercut,
            EnemyState::SwordThrust,
            EnemyState::Sprint,
        ] {
            assert!(handler(s).is_some(), "{s:?}");
        }
        // The shared human idle, flinch and walks.
        for s in [
            EnemyState::Idle,
            EnemyState::HitLow,
            EnemyState::FastAdvance,
            EnemyState::Strike,
        ] {
            assert!(handler(s).is_none(), "{s:?}");
        }
    }
}
