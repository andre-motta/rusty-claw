//! The shared human-enemy states beyond those the first level's officers
//! and soldiers use: backing off from Claw and stepping in towards him, a
//! hop up, a hop back, blocking high and low, the human idle state, being
//! launched off a springboard, and the leap over Claw to the far end of
//! the enemy's ground. Every later human enemy and boss reuses them.
//!
//! This is not a logic family of its own: it runs no logic name
//! ([`NAMES`] is empty). The human classes reach it through the shared
//! enemy behaviour: every enemy keeps a [`HumanData`] in
//! [`super::EnemyFields::human`], the shared handlers of these states are
//! found through [`handler`] (see [`super::states::shared_handler`]), and
//! a call a class schedules for the post-tick is a
//! [`super::Deferred::Human`] carrying a [`HumanCall`], run by
//! [`run_deferred`].
//!
//! State numbers ([`EnemyState`](super::EnemyState)): one number per state
//! name, whichever class runs it. The new names take the block 2030-2049
//! (`JumpBack`, `MoveBack`, `MoveAhead`, `BlockHigh`, `BlockLow`,
//! `JumpOffSpringboard`); names that already have a number keep it
//! (`HitHigh`, `HitLow`, `JumpAtClaw`, `Thrown`, `Jump`). The human idle
//! state is the state named `Idle` (2000) of a human class that has no
//! idle handler of its own; `HumanIdle` (2035) runs the same handler. The
//! leap over Claw is `JumpOver` (2070).
//!
//! # The logic call of a class that blocks
//!
//! A block arms a release (free again, hits allowed again) on the state
//! the block left. The release runs only in a later post-tick that also
//! runs a scheduled call, while the enemy is in that state again, and
//! just before the scheduled call; any scheduled call disarms it. A class
//! that blocks runs its logic calls through [`run`], which adds this step
//! and what the walking states need ([`HumanData::pre_move_x`] and
//! [`HumanData::tile_hit`]) to the shared logic call of
//! [`super::tick`].
//!
//! # Hits
//!
//! The jump, the hop back, the blocks and the leap over Claw make the
//! enemy safe ([`flags::SAFE`]): nothing hits it until it lands or the
//! block ends. Its own attack box still hurts Claw.
//!
//! # Random draws
//!
//! - moving back: one of 0..51 at the start, even when the move cannot be
//!   made;
//! - moving ahead: one of 0..41 at the start, even when the move cannot be
//!   made;
//! - going idle (human idle): one of 0..100; each idle tick: one of
//!   0..201;
//! - the springboard launch: one of 0..100;
//! - nothing else here draws.
//!
//! # Straight shots
//!
//! The thieves of the third level fire the same way: the class loads a
//! [`Shot`] and a firing animation, and [`fire_tick`] releases one
//! straight shot (a `Bullet` with the class's picture) on the animation's
//! shot frame, once per firing animation, then goes Home when the
//! animation ends. A flinch or anything else that changes the state
//! before the shot frame cancels the shot.

use crate::logic::projectile::{self, ProjectileSpec};
use crate::logic::{World, springboard};
use crate::object::{ObjectId, Rect, engine_state, flags, types};
use crate::physics::{ptype, res};
use crate::tiles::{CLEAR, CLIMB, GROUND, SOLID};

use super::{
    Class, Deferred, EnemyState, Handler, Shot, Timer, class_of, data, data_mut, face_claw,
    facing_rect, lift, motion, obj, obj_mut, on_screen, play, rand, say, sees_claw, set_anim,
    set_state, states,
};

/// Logic names this module runs: none, the human classes use it.
pub const NAMES: &[&str] = &[];

/// Image sets the shared human states may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations the shared human states may play: none of
/// their own, each class plays its own animations.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Launch speed of the hop up, px/s (upwards).
pub const JUMP_SPEED: f64 = -600.0;
/// Length of the hop back, px.
pub const JUMP_BACK_DISTANCE: i32 = 100;
/// Walking speed of moving back and moving ahead, px/s.
pub const MOVE_SPEED: f64 = 150.0;
/// Shortest walk back, px; the walk is up to 50 px longer.
pub const MOVE_BACK_MIN: i32 = 75;
/// Number of walk-back lengths (0..51 px added).
pub const MOVE_BACK_SPREAD: u32 = 51;
/// Shortest distance from Claw a walk ahead stops at, px; up to 40 px more.
pub const MOVE_AHEAD_MIN: i32 = 40;
/// Number of walk-ahead distances (0..41 px added).
pub const MOVE_AHEAD_SPREAD: u32 = 41;
/// Slide of a block, px/s, away from Claw.
pub const BLOCK_SPEED: f64 = 200.0;
/// Slowdown of a block's slide per tick, px/s.
pub const BLOCK_STEP: f64 = 25.0;
/// Time idle before a human idle enemy may start pacing, ms.
pub const IDLE_WAIT_MS: u64 = 2000;
/// Chance in 100 that going idle says the idle line (when on screen).
pub const IDLE_VOICE_CHANCE: i32 = 33;
/// Each idle tick, a draw of 0..201 equal to 1 starts pacing.
pub const IDLE_PACE_SPREAD: u32 = 201;
/// Launch of the springboard jump: across (left or right), and upwards.
pub const SPRING_SPEED: (f64, f64) = (100.0, 1500.0);
/// The leap over Claw lands this far inside the far patrol limit, px.
pub const JUMP_OVER_INSET: i32 = 20;
/// Attack box of the leap over Claw, facing left: the body hurts Claw
/// while in the air.
pub const JUMP_OVER_RECT: Rect = Rect::new(-14, 1, 9, 39);

/// Animations of a human class used by these states that the class
/// description ([`super::ClassAnims`]) does not hold ("" for none: a state
/// that waits for its animation then ends at once).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HumanAnims {
    /// The hop back.
    pub jump_back: &'static str,
    /// The high block.
    pub block_high: &'static str,
    /// The low block.
    pub block_low: &'static str,
    /// Idle (human idle, and a boss waiting for its fight).
    pub idle: &'static str,
}

/// Sounds of a human enemy used by these states ("" for none: nothing
/// plays). They are kept per enemy, since a class may change them while
/// it plays (Raux picks his strike and block sounds every tick).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HumanSounds {
    /// The hop back.
    pub jump: &'static str,
    /// The high block.
    pub block_high: &'static str,
    /// The low block.
    pub block_low: &'static str,
    /// Said a third of the time on going idle (human idle).
    pub idle_voice: &'static str,
    /// The strike landing, in place of the class's own when set.
    pub strike: &'static str,
}

/// What every boss keeps: the boss layer between a boss's class and the
/// shared human behaviour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BossFields {
    /// Where the boss was placed (before being dropped to the floor): Claw
    /// respawning puts him back there.
    pub start: (i32, i32),
    /// Health when built, and after Claw respawns.
    pub full_health: i32,
    /// The boss's death has been seen.
    pub death_seen: bool,
    /// The boss's defeat has been reported (once per boss).
    pub defeat_reported: bool,
    /// Claw's attempt number at the last reset (or when built).
    pub attempt: u32,
}

/// What the shared human states keep per enemy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HumanData {
    /// The class's animations for these states.
    pub anims: HumanAnims,
    /// The enemy's sounds for these states.
    pub sounds: HumanSounds,
    /// Where moving back or moving ahead stops.
    pub target_x: i32,
    /// The x before the last movement step.
    pub pre_move_x: i32,
    /// The last movement step reported a stop: a wall, a ceiling or a
    /// landing.
    pub tile_hit: bool,
    /// The release a block armed: the state it left (see the module
    /// documentation).
    pub release_in: Option<EnemyState>,
    /// A springboard under the enemy is about to fire.
    pub springboard_armed: bool,
    /// The boss layer, for a boss.
    pub boss: Option<BossFields>,
}

/// A call of the shared human states scheduled for the post-tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanCall {
    /// The hop up ([`enter_jump`]).
    Jump,
    /// The hop back ([`enter_jump_back`]).
    JumpBack,
    /// The leap over Claw ([`enter_jump_over`]).
    JumpOver,
    /// The springboard launch ([`jump_off_springboard`]).
    JumpOffSpringboard,
}

/// The shared human handler of state `s`, when there is one.
pub fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::JumpBack => Some(jump_back_tick),
        EnemyState::MoveBack | EnemyState::MoveAhead => Some(move_tick),
        EnemyState::BlockHigh | EnemyState::BlockLow => Some(block_tick),
        EnemyState::Idle | EnemyState::HumanIdle => Some(idle_tick),
        EnemyState::JumpOffSpringboard => Some(spring_off_tick),
        EnemyState::JumpOver => Some(jump_over_tick),
        _ => None,
    }
}

/// Runs `call`, scheduled for the post-tick of enemy `id` of `class`.
pub fn run_deferred(w: &mut World, id: ObjectId, class: &Class, call: HumanCall) {
    let _ = class;
    match call {
        HumanCall::Jump => enter_jump(w, id),
        HumanCall::JumpBack => enter_jump_back(w, id),
        HumanCall::JumpOver => enter_jump_over(w, id),
        HumanCall::JumpOffSpringboard => jump_off_springboard(w, id),
    }
}

/// One logic call of the human enemy `id`, through [`run`].
pub fn tick(w: &mut World, id: ObjectId) {
    if let Some(class) = class_of(w, id) {
        run(w, id, class);
    }
}

/// The shared human step when `attacker` hits `id`: the human hit rule
/// ([`super::on_hit`]).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    super::on_hit(w, id, attacker);
}

/// The shared human step when `id` attacks `victim`: nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call of an enemy of `class` that may block: the shared
/// logic call ([`super::tick`]) with the block's release before a
/// scheduled call, and the record of the movement step that moving back
/// and moving ahead read. The first call (building), a kill by another
/// logic and the removal go through [`super::tick`] unchanged.
pub fn run(w: &mut World, id: ObjectId, class: &Class) {
    let raw = obj(w, id).state;
    if matches!(
        raw,
        engine_state::INIT | engine_state::DESTROYING | engine_state::KILLED
    ) || EnemyState::from_id(raw).is_none()
    {
        super::tick(w, id, class);
        return;
    }
    super::pre_tick(w, id, class);
    (class.pre_tick)(w, id);
    let s = super::state(w, id).unwrap_or_default();
    if let Some(h) = (class.handler)(s).or_else(|| states::shared_handler(s, class.human)) {
        h(w, id);
    }
    post_tick(w, id, class);
}

/// The post-tick of [`run`]: the block's release and the scheduled call,
/// then the movement step, noting where it started and whether the tiles
/// stopped it.
fn post_tick(w: &mut World, id: ObjectId, class: &Class) {
    let deferred = data_mut(w, id).and_then(|d| d.deferred.take());
    if deferred.is_some() {
        let now = super::state(w, id);
        let armed = data_mut(w, id).and_then(|d| d.human.release_in.take());
        if armed.is_some() && armed == now {
            release(w, id);
        }
    }
    match deferred {
        Some(Deferred::GoIdle) => (class.go_idle)(w, id),
        Some(Deferred::JumpCheck) => states::jump_check(w, id, class),
        Some(Deferred::Dropped) => lift::let_go(w, id),
        Some(Deferred::Human(call)) => run_deferred(w, id, class, call),
        None => {}
    }
    let x = obj(w, id).x;
    if let Some(d) = data_mut(w, id) {
        d.human.pre_move_x = x;
    }
    motion::integrate(w, id);
    note_stop(w, id);
}

/// Notes whether the movement step just made reported a stop
/// ([`HumanData::tile_hit`]): a wall, a ceiling or a landing.
pub fn note_stop(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.human.tile_hit = d.last_result & res::HIT != 0;
    }
}

/// Free again, and hits allowed again.
fn release(w: &mut World, id: ObjectId) {
    obj_mut(w, id).flags &= !flags::SAFE;
    set_busy(w, id, false);
}

// ----------------------------------------------------------------------
// Helpers

fn set_busy(w: &mut World, id: ObjectId, busy: bool) {
    if let Some(d) = data_mut(w, id) {
        d.busy = busy;
    }
}

fn claw_right(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| d.claw_right)
}

fn go_home(w: &mut World, id: ObjectId) {
    if let Some(c) = class_of(w, id) {
        (c.go_home)(w, id);
    }
}

fn human(w: &World, id: ObjectId) -> HumanData {
    data(w, id).map(|d| d.human.clone()).unwrap_or_default()
}

/// True when the enemy could stand at column `x`: the pixel below its feet
/// there is floor (Solid, Ground or a ladder) and the pixel at its feet is
/// clear.
pub fn floor_ok(w: &World, id: ObjectId, x: i32) -> bool {
    let o = obj(w, id);
    if !o.move_rect.is_set() {
        return false;
    }
    let feet = o.y + o.move_rect.bottom;
    let map = &w.level.map;
    matches!(map.attr(x, feet + 1), SOLID | GROUND | CLIMB) && map.attr(x, feet) == CLEAR
}

/// Launch speed (vx, vy) that carries an enemy (dx, dy) px under the
/// downward acceleration `ay` with a flight time of `sqrt(2 |d| / ay)`
/// (see [`states::leap`]); no speed at all when the leap is straight down
/// or has no length.
pub fn launch(dx: f64, dy: f64, ay: f64) -> (f64, f64) {
    let a = ay.abs();
    let dist = dx.hypot(dy);
    if a == 0.0 || (a * dist - ay * dy).max(0.0).sqrt() < 0.1 {
        return (0.0, 0.0);
    }
    states::leap(dx, dy, ay)
}

/// Sets a launch towards (dx, dy) on the enemy.
fn set_launch(w: &mut World, id: ObjectId, dx: i32, dy: i32) {
    if let Some(d) = data_mut(w, id) {
        let (vx, vy) = launch(f64::from(dx), f64::from(dy), d.ay);
        d.vx = vx;
        d.vy = vy;
    }
}

/// True once the enemy is standing again.
fn landed(w: &World, id: ObjectId) -> bool {
    obj(w, id).physics_type == ptype::STAND
}

// ----------------------------------------------------------------------
// Jump

/// The hop up: only while standing. Busy and safe, it flies straight up
/// at 600 px/s with its jump animation (with the enemies' gravity it rises
/// 90 px and lands 600 ms later; see [`states::jump_tick`]).
pub fn enter_jump(w: &mut World, id: ObjectId) {
    if obj(w, id).physics_type != ptype::STAND {
        return;
    }
    let anim = class_of(w, id).map_or("", |c| c.anims.jump);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = JUMP_SPEED;
        d.busy = true;
    }
    let o = obj_mut(w, id);
    o.flags |= flags::SAFE;
    o.physics_type = ptype::FLY;
    set_state(w, id, EnemyState::Jump);
    set_anim(w, id, anim);
}

// ----------------------------------------------------------------------
// Jump back

/// The hop back: 100 px away from Claw at the same height, when that spot
/// is inside its patrol limits and has floor; otherwise nothing. Busy and
/// safe, it flies there facing Claw with its hop-back animation and its
/// jump sound.
pub fn enter_jump_back(w: &mut World, id: ObjectId) {
    let right = claw_right(w, id);
    let x = obj(w, id).x;
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let tx = if right {
        x.saturating_sub(JUMP_BACK_DISTANCE)
    } else {
        x.saturating_add(JUMP_BACK_DISTANCE)
    };
    let inside = if right { min <= tx } else { tx <= max };
    if !inside || !floor_ok(w, id, tx) {
        return;
    }
    set_launch(w, id, tx - x, 0);
    set_busy(w, id, true);
    face_claw(w, id);
    let h = human(w, id);
    play(w, h.sounds.jump);
    obj_mut(w, id).physics_type = ptype::FLY;
    set_state(w, id, EnemyState::JumpBack);
    obj_mut(w, id).flags |= flags::SAFE;
    set_anim(w, id, h.anims.jump_back);
}

/// JumpBack: once it has landed, hits are allowed again, it goes Home and
/// is free.
pub fn jump_back_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if landed(w, id) {
        obj_mut(w, id).flags &= !flags::SAFE;
        go_home(w, id);
        set_busy(w, id, false);
    }
}

// ----------------------------------------------------------------------
// Move back and move ahead

/// Moving back: 75 to 125 px away from Claw, walking at 150 px/s while
/// facing him, when the spot is inside its patrol limits and has floor;
/// otherwise nothing. It stays as free or busy as it was. Draws: one of
/// 0..51, made first.
pub fn enter_move_back(w: &mut World, id: ObjectId) {
    let r = rand(w, MOVE_BACK_SPREAD);
    let right = claw_right(w, id);
    let x = obj(w, id).x;
    let tx = if right {
        x.saturating_sub(r + MOVE_BACK_MIN)
    } else {
        x.saturating_add(r + MOVE_BACK_MIN)
    };
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let inside = if right { min <= tx } else { tx <= max };
    start_move(w, id, tx, inside, !right, EnemyState::MoveBack);
}

/// Moving ahead: up to 40 to 80 px short of Claw on its own side of him,
/// walking at 150 px/s while facing him, when the spot is inside its
/// patrol limits and has floor; otherwise nothing. It stays as free or
/// busy as it was. Draws: one of 0..41, made first.
pub fn enter_move_ahead(w: &mut World, id: ObjectId) {
    let r = rand(w, MOVE_AHEAD_SPREAD);
    let right = claw_right(w, id);
    let Some(cx) = w.claw_object().map(|c| c.x) else {
        return;
    };
    let tx = if right {
        cx.saturating_sub(r + MOVE_AHEAD_MIN)
    } else {
        cx.saturating_add(r + MOVE_AHEAD_MIN)
    };
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let inside = if right { tx <= max } else { min <= tx };
    start_move(w, id, tx, inside, right, EnemyState::MoveAhead);
}

fn start_move(w: &mut World, id: ObjectId, tx: i32, inside: bool, rightward: bool, s: EnemyState) {
    if let Some(d) = data_mut(w, id) {
        d.human.target_x = tx;
    }
    if !inside || !floor_ok(w, id, tx) {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = if rightward { MOVE_SPEED } else { -MOVE_SPEED };
        d.vy = 0.0;
    }
    face_claw(w, id);
    set_state(w, id, s);
    let anim = class_of(w, id).map_or("", |c| c.anims.advance);
    set_anim(w, id, anim);
}

/// MoveBack and MoveAhead: Home once it has reached or passed its spot,
/// when the last movement step reported any stop (a wall, a ceiling or a
/// landing: a guard that walks back over a small gap in its floor and
/// lands goes Home on its next call), or when it did not move (its exact
/// position is still the pixel it started the step from). The spot is
/// reached by the exact position, fraction included: walking left onto
/// 23694, an enemy at 23694.12 walks one tick more.
pub fn move_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let exact = super::exact_x(w, id);
    let Some((vx, h)) = data(w, id).map(|d| (d.vx, d.human.clone())) else {
        return;
    };
    let target = f64::from(h.target_x);
    let arrived = if vx < 0.0 {
        exact <= target
    } else {
        exact >= target
    };
    if arrived || h.tile_hit || exact == f64::from(h.pre_move_x) {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Blocks

/// A block, high or low, from any state (in the air it keeps falling):
/// busy and safe, its block animation and sound, sliding away from Claw
/// at 200 px/s; the release is armed on the state it left (see the module
/// documentation).
pub fn enter_block(w: &mut World, id: ObjectId, high: bool) {
    let left = super::state(w, id).unwrap_or_default();
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.human.release_in = Some(left);
    }
    set_state(
        w,
        id,
        if high {
            EnemyState::BlockHigh
        } else {
            EnemyState::BlockLow
        },
    );
    let h = human(w, id);
    if high {
        set_anim(w, id, h.anims.block_high);
        play(w, h.sounds.block_high);
    } else {
        set_anim(w, id, h.anims.block_low);
        play(w, h.sounds.block_low);
    }
    obj_mut(w, id).flags |= flags::SAFE;
    let right = claw_right(w, id);
    if let Some(d) = data_mut(w, id) {
        d.vx = if right { -BLOCK_SPEED } else { BLOCK_SPEED };
    }
}

/// BlockHigh and BlockLow: when the animation ends, hits are allowed
/// again, it goes Home and is free; until then, while the slide is not
/// exactly 0, it changes by 25 px/s towards Claw's side as it is now (so
/// it stops after 8 ticks while Claw stays on the same side, and speeds
/// up away from him if he crosses over).
pub fn block_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        obj_mut(w, id).flags &= !flags::SAFE;
        go_home(w, id);
        set_busy(w, id, false);
        return;
    }
    if let Some(d) = data_mut(w, id)
        && d.vx != 0.0
    {
        d.vx += if d.claw_right {
            BLOCK_STEP
        } else {
            -BLOCK_STEP
        };
    }
}

// ----------------------------------------------------------------------
// Human idle

/// Goes idle (a human class without its own idle): faces Claw, stops,
/// says its idle line a third of the time when on screen, takes its idle
/// animation and waits 2000 ms before pacing may start. Draws: one of
/// 0..100.
pub fn go_idle(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    if rand(w, 100) < IDLE_VOICE_CHANCE && on_screen(w, id) {
        let line = human(w, id).sounds.idle_voice;
        say(w, id, line);
    }
    let anim = human(w, id).anims.idle;
    set_anim(w, id, anim);
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, IDLE_WAIT_MS);
    }
}

/// Human idle: a sight-driven enemy that sees Claw goes Home (and the
/// tick goes on); then by mode it goes Home (0 to 4), or (5, 6) starts
/// pacing when the draw is 1 and its wait is over. Draws: one of 0..201.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if data(w, id).is_some_and(|d| d.sight) && sees_claw(w, id) {
        go_home(w, id);
    }
    let r = rand(w, IDLE_PACE_SPREAD);
    let now = w.now_ms;
    let Some((mode, wait)) = data(w, id).map(|d| (d.mode, d.pace_wait)) else {
        return;
    };
    match mode {
        0..=4 => go_home(w, id),
        5 | 6 if r == 1 && wait.done(now) => states::start_pacing(w, id),
        _ => {}
    }
}

// ----------------------------------------------------------------------
// Springboard

/// The springboard under the enemy fires: thrown up at about 1500 px/s
/// and 100 px/s left or right at random, with its jump animation. It
/// stays as free as it was and keeps its patrol limits. Draws: one of
/// 0..100 (below 50: right).
///
/// A springboard under the enemy fires it (see [`platform_step`]).
pub fn jump_off_springboard(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::JumpOffSpringboard);
    let right = rand(w, 100) < 50;
    if let Some(d) = data_mut(w, id) {
        d.human.springboard_armed = false;
        d.vx = if right {
            SPRING_SPEED.0
        } else {
            -SPRING_SPEED.0
        };
        // Launched from 1 px up: the speed that reaches the same height.
        d.vy = if d.ay == 0.0 {
            -SPRING_SPEED.1
        } else {
            -(SPRING_SPEED.1 * SPRING_SPEED.1 - 2.0 * d.ay)
                .max(0.0)
                .sqrt()
        };
    }
    let anim = class_of(w, id).map_or("", |c| c.anims.jump);
    let o = obj_mut(w, id);
    o.y -= 1;
    o.flags &= !flags::ON_ELEVATOR;
    o.physics_type = ptype::FLY;
    set_anim(w, id, anim);
}

/// Special flag of a crumbling peg that a human enemy standing on it may
/// set crumbling (no peg sets it yet).
pub const CRUMBLE_ON_ENEMY: u32 = 0x8000;
/// State of an idle peg or springboard, the only one an enemy can poke.
pub const PLATFORM_IDLE: i32 = 5;
/// State an enemy pokes into an idle peg or springboard under it.
pub const PLATFORM_POKED: i32 = 1111;

/// The platform step of the human pre-tick, with the object below the
/// enemy (as the physics last left it), in order:
///
/// 1. standing on an idle crumbling peg that marks itself
///    ([`CRUMBLE_ON_ENEMY`]): the peg starts to crumble;
/// 2. on a springboard raising its launch flag, with the enemy's
///    springboard flag set (standing on it or not): the launch
///    ([`jump_off_springboard`]) is scheduled for the post-tick when the
///    enemy is free;
/// 3. standing on an idle springboard: the enemy arms it
///    ([`springboard::state::ENEMY_ARMED`]) and sets its own springboard
///    flag;
/// 4. standing on any other platform (or a busy springboard): the enemy
///    becomes stationary.
///
/// True when one of them applied: the pre-tick ends there.
pub fn platform_step(w: &mut World, id: ObjectId) -> bool {
    let (standing, below) = {
        let o = obj(w, id);
        (o.flags & flags::ON_ELEVATOR != 0, o.object_below)
    };
    let Some(b) = below else {
        return false;
    };
    let (bflags, bstate) = w
        .objects
        .get(b)
        .map_or((0, -1), |o| (o.special_flags, o.state));
    if standing && bflags & CRUMBLE_ON_ENEMY != 0 && bstate == PLATFORM_IDLE {
        poke(w, b);
        return true;
    }
    if bflags & springboard::special::LAUNCH != 0 && human(w, id).springboard_armed {
        if let Some(d) = data_mut(w, id)
            && !d.busy
        {
            d.deferred = Some(Deferred::Human(HumanCall::JumpOffSpringboard));
        }
        return true;
    }
    if standing && bflags & springboard::special::ENEMY_ARMABLE != 0 && bstate == PLATFORM_IDLE {
        if let Some(d) = data_mut(w, id) {
            d.human.springboard_armed = true;
        }
        poke(w, b);
        return true;
    }
    if standing {
        if let Some(d) = data_mut(w, id) {
            d.stationary = true;
        }
        return true;
    }
    false
}

fn poke(w: &mut World, platform: ObjectId) {
    if let Some(o) = w.objects.get_mut(platform) {
        o.state = PLATFORM_POKED;
    }
}

/// JumpOffSpringboard: once it has landed it stops and goes Home.
pub fn spring_off_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if landed(w, id) {
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
        }
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Jump over

/// The leap over Claw: only while standing, to 20 px inside the far end
/// of its patrol limits past Claw (its left limit when Claw is to its
/// left), when that spot is inside the limits and has floor; otherwise
/// nothing. It aims its own origin at the height of Claw's feet. Busy and
/// safe, it flies there facing Claw with its jump animation.
pub fn enter_jump_over(w: &mut World, id: ObjectId) {
    if obj(w, id).physics_type != ptype::STAND {
        return;
    }
    let Some(claw_feet) = w.claw_object().map(|c| c.y + c.move_rect.bottom) else {
        return;
    };
    let right = claw_right(w, id);
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let tx = if right {
        max.saturating_sub(JUMP_OVER_INSET)
    } else {
        min.saturating_add(JUMP_OVER_INSET)
    };
    if !((min..=max).contains(&tx) && floor_ok(w, id, tx)) {
        return;
    }
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    set_launch(w, id, tx - x, claw_feet - y);
    set_busy(w, id, true);
    obj_mut(w, id).flags |= flags::SAFE;
    face_claw(w, id);
    obj_mut(w, id).physics_type = ptype::FLY;
    set_state(w, id, EnemyState::JumpOver);
    let anim = class_of(w, id).map_or("", |c| c.anims.jump);
    set_anim(w, id, anim);
}

/// JumpOver: in the air its body is its attack box; once it has landed,
/// hits are allowed again, it is free and goes Home.
pub fn jump_over_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    o.attack_rect = facing_rect(o, JUMP_OVER_RECT);
    if landed(w, id) {
        release(w, id);
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Straight shots

/// Logic of a human enemy's straight shot.
pub const SHOT_LOGIC: &str = "Bullet";

/// A straight shot a human class fires: its picture, its damage (whatever
/// the difficulty) and the sound of its release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StraightShot {
    /// Image set of the shot.
    pub image: &'static str,
    /// Damage it deals.
    pub damage: i32,
    /// Sound played as it leaves ("" for none).
    pub sound: &'static str,
}

/// Loads the shot the enemy fires next: it leaves `muzzle` (across in
/// the enemy's facing, and down) from the enemy at `speed` px/s in its
/// facing, flat. The firing animation is the class's to start.
pub fn load_shot(w: &mut World, id: ObjectId, muzzle: (i32, i32), speed: i32) {
    let right = super::faces_right(obj(w, id));
    let shot = Shot {
        dx: if right { muzzle.0 } else { -muzzle.0 },
        dy: muzzle.1,
        vx: if right { speed } else { -speed },
        vy: 0,
    };
    if let Some(d) = data_mut(w, id) {
        d.shot = shot;
        d.shots = 0;
    }
}

/// Firing a straight shot: advances the firing animation; on its first
/// shot frame the loaded shot leaves (once per firing animation) with
/// `shot`'s sound; Home when the animation ends. Draws nothing.
pub fn fire_tick(w: &mut World, id: ObjectId, shot: StraightShot) {
    let event = w.advance_anim(id);
    if event == 1 && data(w, id).is_some_and(|d| d.shots == 0) {
        if let Some(d) = data_mut(w, id) {
            d.shots = 1;
        }
        release_shot(w, id, shot);
    }
    if obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

/// Launches the loaded shot one depth behind the enemy and plays its
/// sound.
fn release_shot(w: &mut World, id: ObjectId, shot: StraightShot) {
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
            logic: SHOT_LOGIC,
            x: x + s.dx,
            y: y + s.dy,
            z: z - 1,
            vx: s.vx,
            vy: s.vy,
            damage: shot.damage,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: shot.image,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    play(w, shot.sound);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hop_back_takes_about_316_ms() {
        let (vx, vy) = launch(100.0, 0.0, 2000.0);
        assert!((vx - 316.227_766).abs() < 1e-3);
        assert!((vy + 316.227_766).abs() < 1e-3);
        assert_eq!(launch(-100.0, 0.0, 2000.0).0, -vx);
    }

    #[test]
    fn a_leap_straight_down_or_of_no_length_has_no_speed() {
        assert_eq!(launch(0.0, 0.0, 2000.0), (0.0, 0.0));
        assert_eq!(launch(0.0, 50.0, 2000.0), (0.0, 0.0));
        assert_eq!(launch(10.0, 0.0, 0.0), (0.0, 0.0));
        assert_ne!(launch(0.0, -50.0, 2000.0), (0.0, 0.0));
    }

    #[test]
    fn the_springboard_launch_is_corrected_for_its_one_pixel_lift() {
        let v = (SPRING_SPEED.1 * SPRING_SPEED.1 - 2.0 * 2000.0).sqrt();
        assert!((v - 1498.666).abs() < 1e-3);
    }

    #[test]
    fn every_new_state_has_a_shared_handler() {
        for s in [
            EnemyState::JumpBack,
            EnemyState::MoveBack,
            EnemyState::MoveAhead,
            EnemyState::BlockHigh,
            EnemyState::BlockLow,
            EnemyState::HumanIdle,
            EnemyState::JumpOffSpringboard,
            EnemyState::JumpOver,
        ] {
            assert!(handler(s).is_some(), "{s:?}");
        }
        assert!(handler(EnemyState::Strike).is_none());
    }
}
