//! Katherine, the boss of the fourth level: a whip-carrying fighter met in
//! a small walled clearing.
//!
//! She is built like Raux: a human enemy with the boss layer on top, so
//! she waits in the bosses' `Startup` until the boss stage's introduction
//! ends ([`crate::logic::boss_stage`]), goes back to where she was placed
//! when Claw respawns, and releases the gem that ends the level when she
//! dies. Once the fight is on she walks at Claw and, within 200 px,
//! cracks her whip, somersaults over him or waits; up close she stabs or
//! steps back. Unlike Raux she can be lifted and thrown.
//!
//! # The fight
//!
//! - Her five range boxes are 40 px tall each way: 500, 200, 200, 100 and
//!   50 px wide each way (so mode 1 never comes; the second box is the
//!   third's twin).
//! - Home decides: Claw farther than 200 px (mode 0): she runs at him at
//!   200 px/s, walks at 200 px/s within 200 px and comes Home within
//!   100 px; within 200 px (modes 2 and 3) on a frame pulse tick she
//!   whips (roll below 400, with a line half the time each way), leaps
//!   over him (roll 400 to 599) or waits; within 50 px (mode 4) she backs
//!   off or stabs, even odds.
//! - The whip reaches 175 px in front of her ([`WHIP`]), which meets
//!   Claw's hit box up to 190 px between their centres; the stab reaches
//!   48 px ([`STAB`]), up to 63 px between the centres.
//! - The leap ([`EnemyState::Flip`]) aims 200 to 400 px past Claw on his
//!   far side, on her own height; a wall on the way or no floor there
//!   refuses it. In the air and until her landing ends she cannot be hit.
//!   She leaves from her exact position, which standing sits a changing
//!   part of a pixel below her whole pixel once gravity pulls on her (from
//!   her first step back, flight or reset on; built on her floor she
//!   stands exactly on her pixel until then), so her flights match the
//!   original's pixel for pixel.
//! - When Claw, facing her within 100 px, swings at her, she leaps away on
//!   one tick in five while she is free (the pulse tick's roll below 200),
//!   in place of what she chose that tick.
//! - She blocks 60% of sword blows and every pistol shot, stick of
//!   dynamite, magic shot and thrown object, which are destroyed; after a
//!   flinch she only goes Home.
//! - Claw can lift her when she stands more than 20 px in front of him
//!   (her hit box's near edge must be in front of him); thrown, she loses
//!   4 health when she comes to rest. Out of time in the air she would be
//!   gone without the gem; her clearing's walls keep a throw from getting
//!   there.
//!
//! She has 100 health and deals 5 damage (both times the difficulty
//! factors) with her whip, her stab and her touch.
//!
//! # Boss stage
//!
//! Her level's door and stager ([`STAGE`]) are the shared stage pieces:
//! the door leads into the stager's box and the stager plays the
//! introduction. She makes herself the current boss when she is set up,
//! and the stager's health bar reads her health against [`METER_MAX`].
//! When the fight flag reaches 2 she goes idle (her long line, every
//! time), and Home on the next tick: no wait. Her defeat goes through
//! [`crate::logic::World::boss_defeated`], which launches the gem to the
//! point her record's speed fields name.
//!
//! # When Claw respawns
//!
//! Claw's respawn resets her on her first call after it, in the first of
//! the respawn's update passes ([`reset`]): back at her place on her
//! floor with full health, in the state she was in. The respawn's passes
//! take no game time for her ([`between_ticks`]): she makes her draws and
//! her decisions on where Claw now is (a pacing or running Katherine goes
//! Home, Home may whip, leap or pace), but what takes time waits for the
//! tick's own call: a strike under way ends there, not in a pass, and she
//! does not move in the passes. So wherever Claw comes back, she ends the
//! respawn's tick at her place, and Home when she was striking.
//!
//! # After a skipped introduction
//!
//! As for Raux, nothing in her depends on the introduction. With it
//! skipped, a jump keeps Claw above all her boxes (more than 40 px over
//! her) for at least 500 ms at either speed, so she wakes through the
//! out-of-range timer and fights; Claw then comes down on her and takes
//! her touch's damage. The stager stays armed: Claw standing, walking or
//! getting up in its box later runs the whole introduction in the middle
//! of the fight, and her first blow then ends his freeze.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]), so she waits for the fight
//! in the bosses' `Startup` and uses the shared numbers for the states she
//! shares with other enemies (her own idle and Home included). Her
//! somersault is two states of her own, the first of the fourth level's
//! block [`STATE_BLOCK`]: [`EnemyState::Flip`], the flight over Claw, and
//! [`EnemyState::FlipLand`], the landing. The shared `Jump` keeps its own
//! handler for the other classes.
//!
//! In a reference trace of the original game on this level her states
//! carry other numbers; [`REFERENCE_STATE_IDS`] maps hers to them, so a
//! comparison can line her flip and landing up.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   fourth level owns 2120 to 2139, with `Flip` 2120 and `FlipLand` 2121.
//! - Her "on screen" tests (her go-idle line, her whip lines, her landing
//!   line and her wait for Claw outside her range boxes) follow the rule
//!   Raux uses, whatever the view's size.
//! - Thrown and out of time in the air, she is removed without the boss's
//!   end (no gem), as the original does; her clearing's walls keep a throw
//!   from getting there.
//! - A springboard-like hit throws her as it throws any human enemy (her
//!   level has none).
//!
//! # Random draws, in order
//!
//! Each tick (her logic call):
//!
//! 1. the shared pre-tick's roll of 0..=1000 (on frame pulse ticks);
//! 2. one draw whose lowest bit picks the block sound of the tick;
//! 3. the state's draws: Idle one of 0..100 on a frame pulse tick when
//!    Claw is out of range (modes 5, 6); Home in mode 4 one of 0..100, then
//!    moving back's one of 0..51 or one of 0..100 for the stab's sound; in
//!    modes 2 and 3 one of 0..100 for the whip's line (roll below 400) or
//!    the leap's one of 0..201 (roll 400 to 599); in modes 0, 5 and 6 the
//!    shared Home's one of 0..421; FastAdvance one, discarded; Pace one of
//!    0..100 per limit reached; the landing one of 0..100 at its end;
//! 4. a leap scheduled by the dodge: its one of 0..201.
//!
//! When hit by Claw's sword and not blocking: one of 0..4 for a high
//! flinch's sound, then the hit burst's; at her death the death's.
//!
//! Her data ([`KatherineData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]; what she keeps as a boss is in its human data
//! ([`super::human::BossFields`]).

use std::ops::RangeInclusive;

use super::human::{self, BossFields, HumanAnims, HumanData};
use super::lift::{self, Carry};
use super::states::{self, enter_hit, enter_strike, go_home, hit_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit,
    attacker, build, data, data_mut, die, face_claw, hit_burst, human_fields, line_of_sight,
    motion, obj, obj_mut, on_screen, play, rand, say, scaled, set_anim, set_anim_now, set_state,
    take_damage,
};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World, boss_stage};
use crate::object::{ObjectId, Rect, engine_state, flags};
use crate::physics::ptype;
use crate::player::attack::kind as attack_kind;
use crate::player::state as claw_state;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Katherine"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_KATHERINE"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_KATHERINE_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the fourth level's families. Her
/// somersault takes the first two ([`EnemyState::Flip`],
/// [`EnemyState::FlipLand`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2120..=2139;

/// Her states and the numbers a reference trace of the original game on
/// this level gives them.
pub const REFERENCE_STATE_IDS: [(EnemyState, i32); 4] = [
    (EnemyState::Flip, 2005),
    (EnemyState::FlipLand, 2033),
    (EnemyState::Startup, 2028),
    (EnemyState::Home, 2018),
];

/// The pieces of the boss stage on her level: the door into her clearing
/// and the stager next to where it drops Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// Her health before the difficulty factor (the record's health is not
/// used), which is also the health a full health bar stands for.
pub const METER_MAX: i32 = 100;

/// Her image set.
pub const IMAGE_SET: &str = "LEVEL_KATHERINE";

/// Walking, running, backing off and pacing.
pub const WALK_ANIM: &str = "LEVEL_KATHERINE_WALK";
/// The whip.
pub const WHIP_ANIM: &str = "LEVEL_KATHERINE_STRIKE1";
/// The stab.
pub const STAB_ANIM: &str = "LEVEL_KATHERINE_STRIKE2";
/// Falling, and the body flying off after death.
pub const FALL_ANIM: &str = "LEVEL_KATHERINE_FALL";
/// Low hit reaction.
pub const HIT_LOW_ANIM: &str = "LEVEL_KATHERINE_HITLOW";
/// High hit reaction.
pub const HIT_HIGH_ANIM: &str = "LEVEL_KATHERINE_HITHIGH";
/// Standing at Home after a throw.
pub const HOME_ANIM: &str = "LEVEL_KATHERINE_HOME";
/// Idle, and waiting for the fight: plays once and holds its last frame.
pub const IDLE_ANIM: &str = "LEVEL_KATHERINE_IDLE";
/// Being lifted.
pub const LIFTED_ANIM: &str = "LEVEL_KATHERINE_LIFTED1";
/// Held overhead, and flying after a throw.
pub const LIFTED2_ANIM: &str = "LEVEL_KATHERINE_LIFTED2";
/// The somersault in the air.
pub const FLIP_ANIM: &str = "LEVEL_KATHERINE_FLIP2";
/// The somersault's landing.
pub const FLIP_LAND_ANIM: &str = "LEVEL_KATHERINE_FLIP3";
/// Bouncing after a throw.
pub const THROWN_ANIM: &str = "LEVEL_KATHERINE_THROWN";
/// The high block (she never blocks high).
pub const BLOCK_HIGH_ANIM: &str = "LEVEL_KATHERINE_BLOCKHIGH";
/// The low block.
pub const BLOCK_LOW_ANIM: &str = "LEVEL_KATHERINE_BLOCKLOW";

/// The whip's box, facing left: 13 to 175 px in front of her.
pub const WHIP: Rect = Rect::new(-175, -22, -13, 9);
/// The stab's box, facing left: up to 48 px in front of her.
pub const STAB: Rect = Rect::new(-48, -45, 1, -7);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-15, -50, 15, 50);
/// Movement box while Claw carries her.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-15, -40, 15, 40);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (200, 40), (200, 40), (100, 40), (50, 40)];

/// Health before the difficulty factor.
pub const HEALTH: i32 = METER_MAX;
/// Damage of her strikes and touch before the difficulty factor.
pub const DAMAGE: i32 = 5;
/// Smarts base (raised to 99 like every enemy's below 100; nothing she
/// does reads it).
pub const SMARTS: i32 = 50;
/// Her walking and running speed at Claw, px/s.
pub const ADVANCE_SPEED: f64 = 200.0;

/// Her death sound, also at a death by a throw's landing.
pub const DEATH_SOUND: &str = "LEVEL_KATHERINE_00470002";
/// Her line on going idle, every time she is on screen.
pub const IDLE_LINE: &str = "LEVEL_KATHERINE_00470018";
/// Her lines with a whip, one of them half the time each.
pub const WHIP_LINES: [&str; 2] = ["LEVEL_KATHERINE_00470000", "LEVEL_KATHERINE_00470003"];
/// Her line at the end of a somersault, one time in ten.
pub const LANDING_LINE: &str = "LEVEL_KATHERINE_00470015";
/// Said a third of the time when lifted.
pub const LIFTED_VOICE: &str = "LEVEL_KATHERINE_00470012";
/// The whip's sound.
pub const WHIP_SOUND: &str = "LEVEL_KATHERINE_WHIP1";
/// The stab's sounds, one of them half the time each.
pub const STAB_SOUNDS: [&str; 2] = ["LEVEL_STAB2", "LEVEL_STAB3"];
/// The somersault's sound.
pub const FLIP_SOUND: &str = "LEVEL_KATHERINE_FLIP1";
/// The block sounds, picked each tick by a draw's lowest bit.
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_KATHERINE_BLOCK1", "LEVEL_KATHERINE_BLOCK2"];

/// The hit cooldown of a sword blow, ms.
pub const HIT_COOLDOWN_MS: u64 = 500;
/// A sword blow with a roll below this is blocked.
pub const BLOCK_ROLL: i32 = 600;
/// The dodge: a pulse tick's roll below this leaps away.
pub const DODGE_ROLL: i32 = 200;
/// Home in modes 1 to 3: a roll below this whips.
pub const WHIP_ROLL: i32 = 400;
/// Home in modes 1 to 3: a roll below this (and not below
/// [`WHIP_ROLL`]) leaps over Claw.
pub const FLIP_ROLL: i32 = 600;
/// Shortest leap past Claw, px; it is up to 200 px longer.
pub const FLIP_MIN: i32 = 200;
/// Number of leap lengths (0..201 px added).
pub const FLIP_SPREAD: u32 = 201;
/// The step of the leap's ray test, px.
pub const FLIP_RAY_STEP: i32 = 32;
/// Chance in 100 of her landing line.
pub const LANDING_LINE_CHANCE: i32 = 10;
/// Chance in 100, per frame pulse tick out of range, of pacing.
pub const PACE_CHANCE: i32 = 5;

/// The Katherine class.
pub static CLASS: Class = Class {
    name: "Katherine",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle,
    carry: Some(Carry {
        beside: (-20, 20, -15),
        overhead: (0, 0, -60),
        move_rect: CARRIED_MOVE_RECT,
    }),
    pick_lift_voice: super::nothing,
    lift_roll: true,
    anims: ClassAnims {
        advance: WALK_ANIM,
        fast_advance: WALK_ANIM,
        pace: WALK_ANIM,
        walk: "",
        hit_low: HIT_LOW_ANIM,
        hit_high: HIT_HIGH_ANIM,
        home: HOME_ANIM,
        jump: "",
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

/// Katherine's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KatherineData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The dodge scheduled a leap for this tick's post-tick (in place of
    /// any other scheduled call).
    pub flip_due: bool,
    /// How far below her whole-pixel height she really stands, px (see
    /// [`next_stand_fraction`]); 0 while she is not standing.
    pub stand_y: f64,
    /// Whether gravity pulls on her while she stands, so that [`Self::stand_y`]
    /// changes from tick to tick. Built on her floor, she stands exactly on
    /// her pixel (through her strikes, blocks, flinches and pacing) until
    /// her first step back, her first flight or Claw's first respawn; from
    /// then on it stays so.
    pub falls: bool,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        EnemyState::HitLow | EnemyState::HitHigh => Some(hit_tick),
        EnemyState::Flip => Some(flip_tick),
        EnemyState::FlipLand => Some(flip_land_tick),
        _ => None,
    }
}

/// The number a reference trace of the original game gives her state `s`,
/// when it differs from the state's own number.
pub fn reference_state_id(s: EnemyState) -> Option<i32> {
    REFERENCE_STATE_IDS
        .iter()
        .find(|(state, _)| *state == s)
        .map(|&(_, id)| id)
}

/// Katherine's own data.
pub fn data_of_her<'a>(w: &'a World, id: ObjectId) -> Option<&'a KatherineData> {
    match &w.objects.get(id)?.data {
        ObjectData::Katherine(d) => Some(d),
        _ => None,
    }
}

fn own_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut KatherineData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Katherine(d) => Some(d),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the reset when Claw has respawned, the human logic call
/// with her own post-tick (see [`run`]), then her death's report. Being
/// removed, she stops being the current boss.
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

/// The human logic call ([`human::run`]) with her post-tick: a leap the
/// dodge scheduled runs there in place of any other scheduled call. The
/// first call (building), a kill by another logic and the removal go
/// through the shared logic call unchanged.
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
    let no_time = between_ticks(w);
    super::pre_tick(w, id, &CLASS);
    pre_tick(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if (!no_time || decides(s))
        && let Some(h) = handler(s).or_else(|| states::shared_handler(s, true))
    {
        h(w, id);
    }
    post_tick(w, id, no_time);
}

/// True while Claw's respawn runs between two ticks: the respawn's update
/// passes, which run with a frame time of 0 ([`World::dt_ms`]) and take no
/// game time (her call never runs in the warp's set-up, the other call
/// with no frame time). In those calls she makes her pre-tick's draws and
/// her decisions (the states of [`decides`] and a scheduled call), but
/// nothing that takes time happens: a strike, a flinch, a block, a step
/// back or a landing does not go on and she does not move. So a strike
/// that would end in the respawn's tick ends in the tick's own call.
///
/// Inferred: this is what gives the original's random draws, states and
/// positions on the respawn steps of her clearing's runs, with Claw coming
/// back near her (she goes Home from pacing and decides at once) and far
/// from her (her strike ends in that step and she paces from the next).
pub fn between_ticks(w: &World) -> bool {
    w.dt_ms == 0
}

/// The states whose call is a decision on where Claw is, which she makes
/// in the respawn's update passes too (see [`between_ticks`]): waiting
/// for the fight, idle, Home, pacing, walking and running at Claw.
pub fn decides(s: EnemyState) -> bool {
    matches!(
        s,
        EnemyState::Startup
            | EnemyState::Idle
            | EnemyState::Home
            | EnemyState::Pace
            | EnemyState::Advance
            | EnemyState::FastAdvance
    )
}

/// The post-tick: a block's release before a scheduled call (as
/// [`human::run`] does), the scheduled call or the dodge's leap, then the
/// movement step, noting where it started and whether a wall stopped it.
/// With `no_time` (the respawn's update passes, see [`between_ticks`])
/// there is no movement step.
fn post_tick(w: &mut World, id: ObjectId, no_time: bool) {
    let flip = own_mut(w, id).is_some_and(|k| std::mem::take(&mut k.flip_due));
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    if flip || deferred.is_some() {
        let now = super::state(w, id);
        let armed = data_mut(w, id).and_then(|d| d.human.release_in.take());
        if armed.is_some() && armed == now {
            obj_mut(w, id).flags &= !flags::SAFE;
            if let Some(d) = data_mut(w, id) {
                d.busy = false;
            }
        }
    }
    if flip {
        enter_flip(w, id);
    } else {
        match deferred {
            Some(Deferred::GoIdle) => go_idle(w, id),
            Some(Deferred::JumpCheck) => states::jump_check(w, id, &CLASS),
            Some(Deferred::Dropped) => lift::let_go(w, id),
            Some(Deferred::Human(call)) => human::run_deferred(w, id, &CLASS, call),
            None => {}
        }
    }
    if no_time {
        return;
    }
    let (x, stood) = {
        let o = obj(w, id);
        (o.x, o.physics_type == ptype::STAND)
    };
    let vx = data(w, id).map_or(0.0, |d| d.vx);
    if let Some(d) = data_mut(w, id) {
        d.human.pre_move_x = x;
    }
    motion::integrate(w, id);
    let (stands, ay) = (
        obj(w, id).physics_type == ptype::STAND,
        data(w, id).map_or(0.0, |d| d.ay),
    );
    if let Some(d) = data_mut(w, id) {
        // A wall turns the speed across around at half.
        d.human.tile_hit = vx != 0.0 && d.vx == -0.5 * vx;
    }
    if let Some(k) = own_mut(w, id) {
        if stood && stands {
            k.stand_y = if k.falls {
                next_stand_fraction(k.stand_y, ay)
            } else {
                0.0
            };
        } else {
            k.stand_y = 0.0;
            if !stands {
                k.falls = true;
            }
        }
    }
}

/// Her exact height below her whole pixel after one more tick standing,
/// from `f` (px): each tick's gravity moves her exact position down by
/// half a tick's fall from rest (0.324 px at 2000 px/s^2) while the floor
/// holds her pixel, until the exact position reaches the next pixel and
/// the floor puts her back on her own: 0, 0.324, 0.648, 0.972, then 0
/// again.
pub fn next_stand_fraction(f: f64, ay: f64) -> f64 {
    let next = f + 0.5 * ay * super::TICK_S * super::TICK_S;
    if next < 1.0 { next } else { 0.0 }
}

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: a 500 ms cooldown; during a flinch she only goes back
///   Home; with a roll below 600 she blocks low (no damage); otherwise she
///   flinches (high after an uppercut), takes the damage and shows a hit
///   burst;
/// - a pistol shot (or dynamite), magic or a thrown object: it is
///   destroyed and she blocks low, unhurt;
/// - an environment shot: nothing;
/// - a springboard: thrown toward its target, as any human enemy.
///
/// At 0 health or below she dies (and the death is reported).
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
            let now = w.now_ms;
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, HIT_COOLDOWN_MS);
            }
            let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
            if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else if (0..BLOCK_ROLL).contains(&roll) {
                human::enter_block(w, id, false);
            } else {
                let high = w.claw.attack.uppercut;
                enter_hit(w, id, high);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
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
/// with her death sound. Gone out of time after a throw she is not dead
/// in this sense (no death state), and nothing is reported. She never
/// shows the health bar herself: the bar the introduction makes reads her
/// health (see [`crate::logic::boss_stage`]).
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

/// She died: when Claw is alive, her defeat is reported (once).
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
/// has moved on since her last reset and he is no longer dying. The
/// original resets the boss from Claw's respawn itself, so she is reset
/// on her first call after it, whether she runs before or after Claw in
/// the respawn's passes.
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

/// The reset when Claw respawns: back where she was placed with full
/// health; standing, she is put straight down on the floor below that
/// point (her level record places her 8 px above it, and the original
/// shows her on the floor at the end of the respawn's step), exactly on
/// her pixel with gravity pulling on her from then on (so one tick's fall
/// below it at the end of the respawn's tick, see [`next_stand_fraction`]).
/// Her state, speed, timers and busy flag stay as they are.
pub fn reset(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    with_boss(w, id, |b| b.attempt = attempt);
    if let Some(k) = own_mut(w, id) {
        k.stand_y = 0.0;
        k.falls = true;
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

/// Builds Katherine: the current boss, her place kept for the respawn
/// reset, her boxes, 100 health and 5 damage (times the difficulty
/// factors), liftable, always active; then she drops to the floor and
/// waits for the fight in `Startup`. Draws nothing.
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
        strike_rect: WHIP,
        strike_anim: WHIP_ANIM,
        death_anim: FALL_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: LIFTED_VOICE,
        smarts: super::smarts(SMARTS),
        may_lift: true,
        human: HumanData {
            anims: HumanAnims {
                jump_back: "",
                block_high: BLOCK_HIGH_ANIM,
                block_low: BLOCK_LOW_ANIM,
                idle: IDLE_ANIM,
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
        o.data = ObjectData::Katherine(KatherineData {
            enemy,
            flip_due: false,
            stand_y: 0.0,
            falls: false,
        });
    }
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

// ----------------------------------------------------------------------
// Pre-tick

/// Her pre-tick steps after the shared ones: the block sound of the tick,
/// then the dodge. Draws: one, whose lowest bit picks the block sound.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let pick = (w.rng.next_u15() & 1) as usize;
    if let Some(d) = data_mut(w, id) {
        d.human.sounds.block_low = BLOCK_SOUNDS[pick];
    }
    if dodges(w, id) {
        if let Some(d) = data_mut(w, id) {
            d.deferred = None;
        }
        if let Some(k) = own_mut(w, id) {
            k.flip_due = true;
        }
    }
}

/// True when she leaps away from Claw's swing this tick: he attacks
/// facing her from within 100 px (mode 3 or 4), the tick's roll is below
/// 200 and she is free. Draws nothing.
pub fn dodges(w: &World, id: ObjectId) -> bool {
    let attacking = w.claw.attack.kind != attack_kind::NONE;
    let claw_facing_right = w.claw.facing_right;
    data(w, id).is_some_and(|d| {
        let faces_her = claw_facing_right != d.claw_right;
        attacking
            && faces_her
            && (d.mode == 3 || d.mode == 4)
            && (0..DODGE_ROLL).contains(&d.roll)
            && !d.busy
    })
}

// ----------------------------------------------------------------------
// States

/// Startup: waits, idle, until the fight is on; then she goes idle.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        go_idle(w, id);
    }
}

/// Goes idle: stops, says her idle line when on screen and takes her idle
/// animation, advanced once at once. No wait is armed and she does not
/// turn to Claw. Draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    if on_screen(w, id) {
        say(w, id, IDLE_LINE);
    }
    set_anim_now(w, id, IDLE_ANIM);
}

/// Idle: Home when Claw is inside any of her boxes (modes 0 to 4);
/// otherwise a 5% chance of pacing on each frame pulse tick. Draws: one of
/// 0..100 on a frame pulse tick out of range.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let mode = data(w, id).map_or(6, |d| d.mode);
    match mode {
        0..=4 => go_home(w, id),
        _ if w.pulse && rand(w, 100) < PACE_CHANCE => start_pacing(w, id),
        _ => {}
    }
}

/// Home: faces Claw, then by mode:
///
/// - 4 (within 50 px): backs off on a draw below 50; otherwise a draw
///   picks the stab's sound (below 50 the first) and she stabs;
/// - 1 to 3 (within 200 px), by the tick's roll: below 400 she whips (a
///   draw picks her line, said when on screen), 400 to 599 she leaps over
///   Claw, otherwise she waits;
/// - 0, 5 and 6: the shared Home's rule with her speeds: faces Claw, one
///   draw of 0..421, runs at Claw (mode 0) or paces (modes 5 and 6).
///
/// A move that cannot be made leaves her at Home to decide again next
/// tick; her own choice never falls through to the shared rule.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let (mode, roll) = data(w, id).map_or((6, i32::MAX), |d| (d.mode, d.roll));
    match mode {
        4 => {
            if rand(w, 100) < 50 {
                human::enter_move_back(w, id);
                if super::state(w, id) == Some(EnemyState::MoveBack)
                    && let Some(k) = own_mut(w, id)
                {
                    k.falls = true;
                }
            } else {
                let sound = STAB_SOUNDS[usize::from(rand(w, 100) >= 50)];
                start_strike(w, id, STAB, STAB_ANIM, sound);
            }
        }
        1..=3 => {
            if (0..WHIP_ROLL).contains(&roll) {
                let line = WHIP_LINES[usize::from(rand(w, 100) >= 50)];
                if on_screen(w, id) {
                    say(w, id, line);
                }
                start_strike(w, id, WHIP, WHIP_ANIM, WHIP_SOUND);
            } else if (WHIP_ROLL..FLIP_ROLL).contains(&roll) {
                enter_flip(w, id);
            }
        }
        _ => {
            let _ = rand(w, 421);
            match mode {
                0 => enter_fast_advance(w, id),
                5 | 6 => start_pacing(w, id),
                _ => {}
            }
        }
    }
}

/// Starts a strike with `rect` (facing left), `anim` and `sound`.
fn start_strike(w: &mut World, id: ObjectId, rect: Rect, anim: &'static str, sound: &'static str) {
    if let Some(d) = data_mut(w, id) {
        d.strike_rect = rect;
        d.strike_anim = anim;
        d.human.sounds.strike = sound;
    }
    enter_strike(w, id);
}

/// `speed` toward Claw.
fn toward_claw(w: &World, id: ObjectId, speed: f64) -> f64 {
    if data(w, id).is_some_and(|d| d.claw_right) {
        speed
    } else {
        -speed
    }
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

/// Starts running toward Claw at 200 px/s, facing him.
pub fn enter_fast_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::FastAdvance);
    set_anim(w, id, WALK_ANIM);
}

/// FastAdvance: the shared rule at her speed: at the limit on Claw's side
/// she goes idle (and carries on); she faces Claw; then by mode she keeps
/// running at him (0, 1, 5), walks (2), goes Home (3, 4) or paces (6).
/// Draws: one, discarded.
pub fn fast_advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = obj(w, id).x;
    let right = data(w, id).is_some_and(|d| d.claw_right);
    if (x == min && !right) || (x == max && right) {
        go_idle(w, id);
    }
    face_claw(w, id);
    w.advance_anim(id);
    let _ = w.rng.next_u15();
    match mode(w, id) {
        0 | 1 | 5 => {
            let vx = toward_claw(w, id, ADVANCE_SPEED);
            set_vx(w, id, vx);
        }
        2 => enter_advance(w, id),
        3 | 4 => go_home(w, id),
        6 => start_pacing(w, id),
        _ => {}
    }
}

/// Starts walking toward Claw at 200 px/s, facing him.
pub fn enter_advance(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::Advance);
    set_anim(w, id, WALK_ANIM);
}

/// Advance: the shared rule at her speed: at either limit she goes idle
/// (and carries on); she faces Claw and walks at him; then by mode she
/// runs (0), keeps walking (1, 2) or goes Home (3 to 6).
pub fn advance_tick(w: &mut World, id: ObjectId) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = obj(w, id).x;
    if x == min || x == max {
        go_idle(w, id);
    }
    face_claw(w, id);
    let vx = toward_claw(w, id, ADVANCE_SPEED);
    set_vx(w, id, vx);
    w.advance_anim(id);
    match mode(w, id) {
        0 => enter_fast_advance(w, id),
        1 | 2 => {}
        _ => go_home(w, id),
    }
}

// ----------------------------------------------------------------------
// The somersault

/// Where a leap of `d` px past Claw at `claw_x` lands, Claw being on her
/// right or not: on his far side.
pub fn flip_target(claw_x: i32, claw_right: bool, d: i32) -> i32 {
    if claw_right {
        claw_x.saturating_add(d)
    } else {
        claw_x.saturating_sub(d)
    }
}

/// The somersault: 200 to 400 px past Claw on his far side, at her own
/// height, when nothing Solid lies on her row on the way and she could
/// stand there; otherwise nothing. Busy, facing Claw and safe, she flies
/// there on the slowest arc with her somersault animation and sound. She
/// leaves from exactly where she stands: across, with the part of a pixel
/// her last move left; up and down, the part of a pixel she stands below
/// her whole pixel (see [`next_stand_fraction`]); the leap's length counts
/// from there. Draws: one of 0..201, made first.
pub fn enter_flip(w: &mut World, id: ObjectId) {
    let d = rand(w, FLIP_SPREAD) + FLIP_MIN;
    let Some(claw_x) = w.claw_object().map(|c| c.x) else {
        return;
    };
    let right = data(w, id).is_some_and(|d| d.claw_right);
    let tx = flip_target(claw_x, right, d);
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    if !line_of_sight(w, x, y, tx, FLIP_RAY_STEP) || !human::floor_ok(w, id, tx) {
        return;
    }
    let fy = data_of_her(w, id).map_or(0.0, |k| k.stand_y);
    let fx = super::exact_x(w, id) - f64::from(x);
    if let Some(dat) = data_mut(w, id) {
        let (vx, vy) = human::launch(f64::from(tx) - (f64::from(x) + fx), 0.0, dat.ay);
        dat.vx = vx;
        dat.vy = vy;
        dat.busy = true;
        dat.carry = (fx, fy);
        dat.moved_to = Some((x, y));
    }
    face_claw(w, id);
    let o = obj_mut(w, id);
    o.flags |= flags::SAFE;
    o.physics_type = ptype::FLY;
    set_state(w, id, EnemyState::Flip);
    set_anim(w, id, FLIP_ANIM);
    play(w, FLIP_SOUND);
}

/// Flip: in the air until she lands; then she stops and lands
/// ([`EnemyState::FlipLand`]).
pub fn flip_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type == ptype::STAND {
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
            d.vy = 0.0;
        }
        set_state(w, id, EnemyState::FlipLand);
        set_anim(w, id, FLIP_LAND_ANIM);
    }
}

/// FlipLand: when the landing animation ends, hits are allowed again, she
/// may say her landing line (one time in ten, on screen), is free and goes
/// Home. Draws: one of 0..100 at the end.
pub fn flip_land_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    obj_mut(w, id).flags &= !flags::SAFE;
    if rand(w, 100) < LANDING_LINE_CHANCE && on_screen(w, id) {
        say(w, id, LANDING_LINE);
    }
    if let Some(d) = data_mut(w, id) {
        d.busy = false;
    }
    go_home(w, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn her_flip_maps_to_the_reference_numbers() {
        assert_eq!(reference_state_id(EnemyState::Flip), Some(2005));
        assert_eq!(reference_state_id(EnemyState::FlipLand), Some(2033));
        assert_eq!(reference_state_id(EnemyState::Startup), Some(2028));
        assert_eq!(reference_state_id(EnemyState::Home), Some(2018));
        assert_eq!(reference_state_id(EnemyState::Strike), None);
    }

    #[test]
    fn her_states_are_in_her_block() {
        for s in [EnemyState::Flip, EnemyState::FlipLand] {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
    }

    #[test]
    fn her_strike_boxes_mirror_when_she_faces_right() {
        assert_eq!(WHIP.mirrored(), Rect::new(13, -22, 175, 9));
        assert_eq!(STAB.mirrored(), Rect::new(-1, -45, 48, -7));
    }

    #[test]
    fn the_leap_lands_past_claw_on_his_far_side() {
        assert_eq!(flip_target(1000, true, 200), 1200);
        assert_eq!(flip_target(1000, false, 400), 600);
    }

    #[test]
    fn she_has_her_own_handlers_for_her_own_states() {
        for s in [
            EnemyState::Startup,
            EnemyState::Idle,
            EnemyState::Home,
            EnemyState::Advance,
            EnemyState::FastAdvance,
            EnemyState::Flip,
            EnemyState::FlipLand,
        ] {
            assert!(handler(s).is_some(), "{s:?}");
        }
        assert!(handler(EnemyState::Strike).is_none());
    }
}
