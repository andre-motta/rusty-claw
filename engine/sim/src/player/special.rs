//! Claw's special states: the hit reaction, lifting, throwing and being
//! frozen.
//!
//! # Hit reaction (31)
//!
//! [`enter_hit`] starts it when something hurts Claw: one of the two
//! damage animations (an even draw from the game's random stream), and,
//! when he was on his feet, a push of [`KNOCKBACK_SPEED`] px/s away from
//! the attacker. In the air he stays falling. Each tick the animation
//! advances; while its frame has event 1 the push moves him, after that he
//! stays put (the reaction itself never moves him up or down). When the
//! animation ends he stands again, even in mid-air: a jump press there
//! starts a second jump, as in the original (the default). With the
//! mid-air hit jump fix on (see
//! [`crate::tuning::Behaviour::fix_double_jump`]) a reaction that ends in
//! mid-air hands over to landing instead, which turns into a fall on the
//! next tick, so Claw cannot jump again before he lands.
//!
//! # Lifting (5000) and throwing (5001)
//!
//! [`begin_lift`] picks up a liftable object (the lift key or Up with
//! Attack, see [`super::attack`]): Claw plays his lifting animation (the
//! low one for objects lifted from a crouch), the object's family is told
//! through [`logic::lifted`] and his movement box grows by
//! [`LIFT_RAISE`] px upwards. Once lifted he carries it: Left and Right
//! turn him and walk at [`CARRY_SPEED`] px/s (no step on the walk's event
//! frames). Walking off a ledge lets go of the object with a sideways
//! speed of [`LEDGE_RELEASE_SPEED`] px/s in the walking direction (or
//! drops it when he stood still) and he falls. After
//! [`LIFT_HOLD_MS`] of carrying the object is put down and he stands.
//!
//! Attack or Lift starts the throw, and the object's family is told that
//! Claw winds up ([`logic::wind_up`]). Holding the key charges it, up to
//! [`THROW_CHARGE_MS`]; the animation waits meanwhile. On the throw
//! animation's event 1 the object leaves with a power `p` from 0 to 100
//! that grows with the charge, at `200 + 3p` px/s forwards and
//! `600 + 2p` px/s upwards ([`logic::thrown`]), and Claw grunts. When the
//! animation ends he stands (an object still in his hands then, because
//! the animation has no release frame, is dropped). Falling off a ledge
//! while throwing drops the object too.
//!
//! Every interruption (a hit, a freeze, any state change that cancels the
//! attack) lets go of the carried object through [`logic::dropped`].
//!
//! # Frozen (5009)
//!
//! [`enter_freeze`] (or [`World::freeze_claw`] from a logic, such as a
//! freeze curse) stops Claw: he stands facing right and reads no input
//! until [`World::thaw_claw`] releases him, or until the freeze's time runs
//! out when it has one.
//!
//! A boss fight's introduction freezes him with
//! [`World::freeze_claw_now`] instead: at once, during the calling logic,
//! and until the boss fight flag reaches 2 (see
//! [`crate::logic::world_boss`]); his own next logic call then sees the
//! flag and he stands again. A freeze with a time left becomes this
//! freeze, without the time.
//!
//! # Held at the boss's defeat (5010)
//!
//! When a boss is defeated, [`World::freeze_claw_boss_end`] puts Claw in
//! [`BOSS_END`] at once: a state with no handler of its own, so he stays
//! where and as he is (only the bookkeeping after every handler runs).
//! [`World::release_claw_boss_end`] lets him go when the gem lands: his
//! attack ends and he stands. The original also clears Left and Right from
//! its pressed-key latch there, so that a direction held all along counts
//! as pressed again; Claw's movement here reads held keys only, so a held
//! direction moves him at once either way.
//!
//! These calls work on Claw's object directly. Called while Claw's own
//! logic call holds his object (from code that runs inside it without
//! [`super::Ctx::with_world`]), they do nothing and return false.
//!
//! A logic may also start the hit reaction with
//! [`World::claw_hit_reaction`]. Other requests from logics take effect on
//! Claw's next logic call. Forcing one of these states without its entry
//! (tools and tests) returns Claw to standing, as a state that is not
//! filled in does (see [`super::fallback_tick`]).

use std::sync::Arc;

use claw_frame::{Buttons, SoundChannel, TickInput};

use super::{Ctx, attack, fallback_tick, state};
use crate::logic::world_boss::fight;
use crate::logic::{self, World};
use crate::object::{LogicKind, Object, ObjectId, Rect, flags};
use crate::physics::{opt, ptype};
use crate::tuning::{PlayAids, TICK_MS};

/// Speed of the push away from an attacker, px/s.
pub const KNOCKBACK_SPEED: i32 = 200;
/// Event of the damage animation's frames on which the push moves Claw.
pub const KNOCKBACK_EVENT: i32 = 1;
/// The two damage animations.
pub const HIT_ANIMS: [&str; 2] = ["CLAW_DAMAGE1", "CLAW_DAMAGE2"];
/// Chance in percent of the first damage animation.
pub const HIT_ANIM_PERCENT: u32 = 50;

/// Special flag of an object Claw lifts from a crouch.
pub const LIFT_LOW: u32 = 0x4;
/// Lifting animation.
pub const LIFT_ANIM: &str = "CLAW_LIFT";
/// Lifting animation for an object lifted from a crouch.
pub const LIFT_LOW_ANIM: &str = "CLAW_DUCKLIFT";
/// Standing with the object overhead.
pub const CARRY_ANIM: &str = "CLAW_LIFT2";
/// Walking with the object overhead.
pub const CARRY_WALK_ANIM: &str = "CLAW_LIFTWALK";
/// Throwing.
pub const THROW_ANIM: &str = "CLAW_THROW";
/// Claw's grunt when the object leaves his hands.
pub const THROW_SOUND: &str = "CLAW_GRUNTTHROW";
/// Event of the carrying walk's frames on which Claw does not move.
pub const CARRY_STEP_EVENT: i32 = 1;
/// Event of the throw animation's frame on which the object leaves.
pub const THROW_EVENT: i32 = 1;
/// Walking speed while carrying, px/s (provisional: the game's own
/// carrying walk may be faster).
pub const CARRY_SPEED: f64 = 30.0;
/// How much higher the movement box reaches while carrying, px.
pub const LIFT_RAISE: i32 = 25;
/// Longest time Claw carries an object before putting it down, ms.
pub const LIFT_HOLD_MS: i32 = 100_000;
/// Sideways speed of an object let go when Claw walks off a ledge, px/s.
pub const LEDGE_RELEASE_SPEED: i32 = 225;
/// Longest charge of a throw, ms.
pub const THROW_CHARGE_MS: i32 = 3000;
/// Strongest throw power.
pub const THROW_MAX_POWER: i32 = 100;
/// Forward speed of a throw at power 0, px/s.
pub const THROW_SPEED_X: i32 = 200;
/// Forward speed added per point of power, px/s.
pub const THROW_SPEED_X_PER_POWER: i32 = 3;
/// Upward speed of a throw at power 0, px/s.
pub const THROW_SPEED_Y: i32 = 600;
/// Upward speed added per point of power, px/s.
pub const THROW_SPEED_Y_PER_POWER: i32 = 2;
/// Claw's image shown as the throw starts, when his image set has it.
pub const THROW_FIRST_IMAGE: i32 = 44;

/// Data of the special states.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecialData {
    /// The object Claw carries.
    pub lifted: Option<ObjectId>,
    /// Time left frozen, ms; 0 freezes until released.
    pub freeze_ms: i32,
    /// The hit reaction was started by [`enter_hit`].
    pub in_hit: bool,
    /// A logic asked for the hit reaction, away from an attacker at this x.
    pub hit_from: Option<i32>,
    /// The lifting animation is over: Claw carries the object.
    pub carrying: bool,
    /// Carrying and standing still.
    pub carry_still: bool,
    /// Carrying time left before the object is put down, ms.
    pub lift_ms: i32,
    /// Movement box before lifting.
    pub lift_rect: Option<Rect>,
    /// The throw was started by its entry.
    pub throwing: bool,
    /// Throw charge, ms.
    pub throw_ms: i32,
    /// The freeze was started by its entry.
    pub frozen: bool,
    /// A logic asked for a freeze; it starts on Claw's next logic call.
    pub freeze_pending: bool,
    /// A logic asked for the freeze to end.
    pub thaw: bool,
    /// The freeze is a boss fight's: it ends when the fight starts.
    pub boss_freeze: bool,
    /// Where a warp touched in this tick takes Claw. He stays at the warp
    /// for the rest of the tick; the simulation moves him there between
    /// this tick and the next, wakes and sets up the objects around his new
    /// place and runs his logic once with no game time (see
    /// [`crate::Sim::step`]).
    pub warp_to: Option<(i32, i32)>,
    /// Claw was let go after a boss's defeat: the next tick takes Left and
    /// Right out of the latched keys, so a direction still held counts at
    /// once (see [`crate::Sim::step`]).
    pub unlatch_directions: bool,
}

/// Forgets which special state was entered; every cancel of the attack
/// (and so every state change) calls it.
pub(super) fn forget_entries(c: &mut Ctx) {
    let s = &mut c.p.special;
    s.in_hit = false;
    s.carrying = false;
    s.carry_still = false;
    s.throwing = false;
    s.frozen = false;
    s.boss_freeze = false;
}

// ---------------------------------------------------------------- hit

/// Starts the hit reaction, pushed away from an attacker at `attacker_x`.
/// Nothing happens while the reaction already runs.
///
/// What Claw carries is let go and the attack ends. The damage animation
/// is drawn from the game's random stream. Standing, walking or crouching,
/// Claw is pushed at [`KNOCKBACK_SPEED`] px/s away from the attacker;
/// rising or falling he falls (he stands when he rides a platform).
pub fn enter_hit(c: &mut Ctx, attacker_x: i32) {
    if c.o.state == state::HIT && c.p.special.in_hit {
        return;
    }
    begin_hit(c, attacker_x);
}

fn begin_hit(c: &mut Ctx, attacker_x: i32) {
    attack::cancel(c);
    c.o.state = state::HIT;
    let anim = if c.rng.below(100) < HIT_ANIM_PERCENT {
        HIT_ANIMS[0]
    } else {
        HIT_ANIMS[1]
    };
    c.set_anim(anim, false);
    c.speeds_zero();
    if matches!(c.o.physics_type, ptype::STAND | ptype::WALK | ptype::DUCK) {
        c.o.speed_x = if c.o.x > attacker_x {
            KNOCKBACK_SPEED
        } else {
            -KNOCKBACK_SPEED
        };
    }
    c.o.physics_type = match c.o.physics_type {
        ptype::JUMP | ptype::FALL | ptype::FLY if c.o.flags & flags::ON_ELEVATOR == 0 => {
            ptype::FALL
        }
        _ => ptype::STAND,
    };
    let s = &mut c.p.special;
    s.in_hit = true;
    s.hit_from = None;
}

/// The hit reaction (state 31).
pub fn hit_tick(c: &mut Ctx) {
    if let Some(x) = c.p.special.hit_from.take() {
        begin_hit(c, x);
    } else if !c.p.special.in_hit {
        fallback_tick(c);
        return;
    }
    let (ex, ey) = c.elevator_delta();
    let mut tx = c.o.x + ex;
    let ty = c.o.y + ey;
    if c.advance_anim() == KNOCKBACK_EVENT {
        tx += c.o.speed_x * c.dt() / 1000;
    } else {
        c.o.speed_x = 0;
    }
    c.physics(tx, ty, opt::PLATFORMS);
    if c.o.physics_type == ptype::FALL {
        c.o.flags &= !flags::ON_ELEVATOR;
    }
    if c.o.anim.is_done() {
        let land = c.t.behaviour.fix_double_jump && super::hit_ends_airborne(c);
        super::enter_stand(c);
        if land {
            c.o.state = state::LAND;
        }
    }
}

// ---------------------------------------------------------------- lift

/// Lifts `target` (the caller checked that Claw can): the lifting
/// animation starts, the object's family is told, it is drawn just in
/// front of Claw and his movement box grows upwards.
pub fn begin_lift(c: &mut Ctx, target: ObjectId) {
    let Some(t) = c.objects.get_mut(target) else {
        return;
    };
    let low = t.special_flags & LIFT_LOW != 0;
    let z = c.o.z + 1;
    if t.z != z {
        t.z = z;
        t.flags |= flags::RESORT;
    }
    c.o.state = state::LIFT;
    c.set_anim(if low { LIFT_LOW_ANIM } else { LIFT_ANIM }, false);
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    let tp = &c.t.player;
    c.p.afk_time = tp.idle_delay_ms;
    c.p.look_up_time = tp.look_up_delay_ms;
    let s = &mut c.p.special;
    s.carrying = false;
    s.carry_still = false;
    s.lifted = Some(target);
    s.lift_ms = LIFT_HOLD_MS;
    s.throw_ms = 0;
    s.lift_rect = Some(c.o.move_rect);
    c.o.move_rect.top -= LIFT_RAISE;
    c.p.mover.zero_carries();
    let id = c.id;
    c.with_world(|w| logic::lifted(w, target, id));
}

/// Puts the movement box back as it was before lifting.
fn restore_move_rect(c: &mut Ctx) {
    if let Some(r) = c.p.special.lift_rect.take() {
        c.o.move_rect = r;
    }
}

/// Lets go of the carried object: thrown sideways at `vx` px/s, or just
/// dropped when `vx` is 0.
fn let_go(c: &mut Ctx, vx: i32) {
    if let Some(id) = c.p.special.lifted.take() {
        c.with_world(|w| {
            if vx == 0 {
                logic::dropped(w, id);
            } else {
                logic::thrown(w, id, vx, 0);
            }
        });
    }
}

/// Lifting and carrying an object (state 5000).
pub fn lift_tick(c: &mut Ctx) {
    if c.p.special.lifted.is_none() {
        fallback_tick(c);
        return;
    }
    let carrying = c.p.special.carrying;
    let v = c.p.mover.x.v;
    if carrying && c.held(Buttons::LEFT) {
        c.face_left();
        if c.p.special.carry_still || v == CARRY_SPEED {
            c.set_anim(CARRY_WALK_ANIM, false);
            c.p.mover.x.v = -CARRY_SPEED;
            c.p.special.carry_still = false;
        }
    } else if carrying && c.held(Buttons::RIGHT) {
        c.face_right();
        if c.p.special.carry_still || v == -CARRY_SPEED {
            c.set_anim(CARRY_WALK_ANIM, false);
            c.p.mover.x.v = CARRY_SPEED;
            c.p.special.carry_still = false;
        }
    } else {
        c.p.mover.x.v = 0.0;
    }
    let mut tx = c.o.x;
    if c.advance_anim() != CARRY_STEP_EVENT {
        c.p.mover.step(c.dt_s());
        tx += c.p.mover.take_x();
    }
    let (ex, ey) = c.elevator_delta();
    let y = c.o.y;
    c.physics(tx + ex, y + ey, opt::PLATFORMS | opt::CENTRE_ON_LADDER);
    if c.o.physics_type == ptype::FALL {
        let v = c.p.mover.x.v;
        let vx = if v > 0.0 {
            LEDGE_RELEASE_SPEED
        } else if v < 0.0 {
            -LEDGE_RELEASE_SPEED
        } else {
            0
        };
        let_go(c, vx);
        restore_move_rect(c);
        super::begin_fall(c, false);
        return;
    }
    let v = c.p.mover.x.v;
    let s = &c.p.special;
    if (c.o.anim.is_done() && !s.carrying) || (s.carrying && v == 0.0) {
        if c.o.anim.key() != Some(CARRY_ANIM) {
            c.set_anim(CARRY_ANIM, false);
        }
        c.p.special.carrying = true;
        c.p.special.carry_still = true;
    }
    if c.p.special.carrying && (c.pressed(Buttons::ATTACK) || c.pressed(Buttons::LIFT)) {
        if let Some(id) = c.p.special.lifted {
            c.with_world(|w| logic::wind_up(w, id));
        }
        restore_move_rect(c);
        begin_throw(c);
        return;
    }
    if c.dt() >= c.p.special.lift_ms {
        let_go(c, 0);
        restore_move_rect(c);
        super::enter_stand(c);
    } else {
        c.p.special.lift_ms -= c.dt();
    }
}

// ---------------------------------------------------------------- throw

/// Starts the throw of the carried object.
fn begin_throw(c: &mut Ctx) {
    c.o.state = state::THROW;
    c.set_anim(THROW_ANIM, false);
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    if let Some((first, last)) = c.level.image_frames(&c.o.image_set)
        && (first..=last).contains(&THROW_FIRST_IMAGE)
    {
        c.o.frame = THROW_FIRST_IMAGE;
    }
    c.p.special.throwing = true;
}

/// The speed of an object thrown after a charge of `charge_ms`: its power
/// is `charge_ms * 100 / 3000` (whole points, at most 100), the speed
/// `200 + 3p` px/s forwards (negated facing left) and `600 + 2p` px/s up.
pub fn throw_velocity(charge_ms: i32, facing_right: bool) -> (i32, i32) {
    let p = (charge_ms.max(0) * THROW_MAX_POWER / THROW_CHARGE_MS).min(THROW_MAX_POWER);
    let vx = THROW_SPEED_X + THROW_SPEED_X_PER_POWER * p;
    let vy = -(THROW_SPEED_Y + THROW_SPEED_Y_PER_POWER * p);
    (if facing_right { vx } else { -vx }, vy)
}

/// Throwing the lifted object (state 5001).
pub fn throw_tick(c: &mut Ctx) {
    if !c.p.special.throwing {
        fallback_tick(c);
        return;
    }
    c.speeds_zero();
    let (ex, ey) = c.elevator_delta();
    if c.held(Buttons::LEFT) && c.p.facing_right {
        c.face_left();
    } else if c.held(Buttons::RIGHT) && !c.p.facing_right {
        c.face_right();
    }
    let (x, y) = (c.o.x, c.o.y);
    c.physics(x + ex, y + ey, opt::PLATFORMS | opt::CENTRE_ON_LADDER);
    if c.o.physics_type == ptype::FALL {
        let_go(c, 0);
        super::begin_fall(c, false);
        return;
    }
    let charging = c.held(Buttons::ATTACK) || c.held(Buttons::LIFT);
    if charging && c.p.special.throw_ms < THROW_CHARGE_MS {
        c.p.special.throw_ms += c.dt();
        return;
    }
    if c.advance_anim() == THROW_EVENT
        && let Some(id) = c.p.special.lifted.take()
    {
        let (vx, vy) = throw_velocity(c.p.special.throw_ms, c.p.facing_right);
        c.with_world(|w| logic::thrown(w, id, vx, vy));
        c.play(THROW_SOUND, SoundChannel::Effect);
    }
    if c.o.anim.is_done() {
        let_go(c, 0);
        super::enter_stand(c);
    }
}

// ---------------------------------------------------------------- freeze

/// Freezes Claw: the attack ends, he stands facing right and stays so
/// until released, or for `ms` milliseconds when `ms` is above 0.
/// Nothing happens while he is frozen already.
pub fn enter_freeze(c: &mut Ctx, ms: u32) {
    if c.o.state == state::FREEZE && c.p.special.frozen {
        return;
    }
    freeze_now(c, ms_of(ms));
}

fn ms_of(ms: u32) -> i32 {
    i32::try_from(ms).unwrap_or(i32::MAX)
}

fn freeze_now(c: &mut Ctx, ms: i32) {
    super::enter_stand(c);
    c.face_right();
    c.o.state = state::FREEZE;
    let s = &mut c.p.special;
    s.frozen = true;
    s.freeze_pending = false;
    s.thaw = false;
    s.freeze_ms = ms;
}

/// Claw's state while he is held at a boss's defeat (see
/// [`World::freeze_claw_boss_end`]).
pub const BOSS_END: i32 = 5010;

/// Freezes Claw for a boss fight's introduction: unless he is frozen
/// already, the attack ends, he stands and faces right; then the freeze
/// lasts until the boss fight flag reaches 2, also when it was a freeze
/// with a time left. True when he was frozen now.
pub fn enter_boss_freeze(c: &mut Ctx) -> bool {
    let already = c.o.state == state::FREEZE && c.p.special.frozen;
    if !already {
        freeze_now(c, 0);
    }
    let s = &mut c.p.special;
    s.boss_freeze = true;
    s.freeze_ms = 0;
    !already
}

/// Lets Claw go after a boss's defeat (see
/// [`World::release_claw_boss_end`]): only when he is held in
/// [`BOSS_END`], Left and Right stop counting as latched from the next
/// tick, his attack ends and he stands. True when he was let go.
pub fn release_boss_end(c: &mut Ctx) -> bool {
    if c.o.state != BOSS_END {
        return false;
    }
    c.p.special.unlatch_directions = true;
    attack::cancel(c);
    super::enter_stand(c);
    true
}

/// Frozen (state 5009): the animation plays and nothing else happens until
/// the freeze is released or its time runs out (or, for a boss fight's
/// freeze, until the fight starts); then Claw stands.
pub fn freeze_tick(c: &mut Ctx) {
    if c.p.special.freeze_pending {
        let ms = c.p.special.freeze_ms;
        freeze_now(c, ms);
    } else if !c.p.special.frozen {
        fallback_tick(c);
        return;
    }
    c.advance_anim();
    let dt = c.dt();
    let s = &mut c.p.special;
    let timed_out = s.freeze_ms > 0 && {
        s.freeze_ms = (s.freeze_ms - dt).max(0);
        s.freeze_ms == 0
    };
    let fight_on = s.boss_freeze && c.out.boss().fight() == fight::ON;
    if s.thaw || timed_out || fight_on {
        s.thaw = false;
        super::enter_stand(c);
    }
}

// ---------------------------------------------------------------- requests from logics

/// True when Claw can be put in a special state from outside: he is not
/// being placed, dying or dead.
fn claw_can_change(state: i32) -> bool {
    !matches!(
        state,
        state::INIT | state::RESPAWN | state::DEATH_FALL | state::DEATH
    )
}

impl World<'_> {
    /// Starts Claw's hit reaction away from an attacker at `attacker_x`, as
    /// [`enter_hit`] does, on his next logic call. False when nothing
    /// changes: there is no Claw, he is being placed or is dying, or the
    /// reaction runs already.
    pub fn claw_hit_reaction(&mut self, attacker_x: i32) -> bool {
        let Some(o) = self.claw_id.and_then(|id| self.objects.get_mut(id)) else {
            return false;
        };
        if o.state == state::HIT || !claw_can_change(o.state) {
            return false;
        }
        o.state = state::HIT;
        self.claw.special.hit_from = Some(attacker_x);
        true
    }

    /// Freezes Claw as [`enter_freeze`] does, from his next logic call:
    /// until [`World::thaw_claw`], or for `ms` milliseconds when `ms` is
    /// above 0. False when nothing changes: there is no Claw, he is being
    /// placed, is dying or is frozen already.
    pub fn freeze_claw(&mut self, ms: u32) -> bool {
        let Some(o) = self.claw_id.and_then(|id| self.objects.get_mut(id)) else {
            return false;
        };
        if o.state == state::FREEZE || !claw_can_change(o.state) {
            return false;
        }
        o.state = state::FREEZE;
        let s = &mut self.claw.special;
        s.freeze_pending = true;
        s.freeze_ms = ms_of(ms);
        s.thaw = false;
        true
    }

    /// Freezes Claw at once, during this call, for a boss fight's
    /// introduction (see [`enter_boss_freeze`]): unless he is frozen
    /// already, his attack ends, he stands facing right in the frozen
    /// state, and he stays so until his own logic call sees the boss fight
    /// flag at 2. A request of [`World::freeze_claw`] still waiting is
    /// taken over, and a freeze with a time left loses it. True when he
    /// was frozen now; false when there is no Claw, he was frozen already,
    /// or his own logic call holds his object (see the module
    /// documentation).
    pub fn freeze_claw_now(&mut self) -> bool {
        self.with_claw_ctx(enter_boss_freeze).unwrap_or(false)
    }

    /// Holds Claw at a boss's defeat: his state becomes [`BOSS_END`] at
    /// once, with no exit from the state he was in, so he stays where and
    /// as he is (falling or not) until [`World::release_claw_boss_end`].
    /// The caller decides when (the defeat holds him only while his health
    /// is above 0). False when there is no Claw or his own logic call holds
    /// his object.
    pub fn freeze_claw_boss_end(&mut self) -> bool {
        let Some(o) = self.claw_object_mut() else {
            return false;
        };
        o.state = BOSS_END;
        true
    }

    /// Lets Claw go after a boss's defeat (see [`release_boss_end`]): only
    /// when he is held in [`BOSS_END`], his attack ends and he stands
    /// (standing animation, standing physics, speeds 0). True when he was
    /// let go.
    pub fn release_claw_boss_end(&mut self) -> bool {
        self.with_claw_ctx(release_boss_end).unwrap_or(false)
    }

    /// Claw's object, unless there is none or his own logic call holds it
    /// (his slot then holds a stand-in that is not the player).
    fn claw_object_mut(&mut self) -> Option<&mut Object> {
        let id = self.claw_id?;
        self.objects
            .get_mut(id)
            .filter(|o| o.kind == LogicKind::Player)
    }

    /// Runs `f` on Claw's handler context, built around his object taken
    /// out of the container for the call. None when there is no Claw or
    /// his own logic call holds his object.
    fn with_claw_ctx<R>(&mut self, f: impl FnOnce(&mut Ctx) -> R) -> Option<R> {
        self.claw_object_mut()?;
        let id = self.claw_id?;
        let platforms = crate::sim::platforms(self.objects);
        let mut held = Object::new(0, 0, 0, 0, Arc::from(""), LogicKind::Inert);
        if !self.objects.swap_payload(id, &mut held) {
            return None;
        }
        // These calls read no input and never move the camera.
        let mut camera_override = None;
        let view = self.view;
        let r = {
            let mut c = Ctx {
                o: &mut held,
                p: &mut *self.claw,
                id,
                objects: &mut *self.objects,
                level: self.level,
                platforms: &platforms,
                input: TickInput::default(),
                t: self.tuning,
                rng: &mut *self.rng,
                sound_rng: &mut *self.sound_rng,
                camera_override: &mut camera_override,
                plane_centre: ((view.left + view.right) / 2, (view.top + view.bottom) / 2),
                out: &mut *self.out,
                tick: self.tick,
                now_ms: self.now_ms,
                dt_ms: TICK_MS,
                pulse: self.pulse,
                view,
                level_logics: true,
                // Holding and letting go of Claw for a boss never hurts
                // him or takes a life, so the aids do not matter here.
                aids: PlayAids::NONE,
            };
            f(&mut c)
        };
        self.objects.swap_payload(id, &mut held);
        Some(r)
    }

    /// Releases a frozen Claw on his next logic call: he stands again.
    pub fn thaw_claw(&mut self) {
        let frozen = self
            .claw_id
            .and_then(|id| self.objects.get(id))
            .is_some_and(|o| o.state == state::FREEZE);
        if frozen {
            self.claw.special.thaw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::world_boss::fight;
    use crate::tiles::TileMap;
    use crate::{Sim, SimConfig, SimLevel};

    fn flat_sim() -> Sim {
        let mut rows = vec![".".repeat(60); 15];
        rows.push("#".repeat(60));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
        let mut sim = Sim::new(level, SimConfig::default());
        for _ in 0..30 {
            sim.step(TickInput::default());
        }
        sim
    }

    /// Runs `f` on a world whose Claw slot holds the stand-in his own
    /// logic call leaves there while it runs.
    fn inside_claws_call<R>(sim: &mut Sim, f: impl FnOnce(&mut World) -> R) -> R {
        sim.with_world(|w| {
            let id = w.claw_id.unwrap();
            let mut held = Object::new(0, 0, 0, 0, Arc::from(""), LogicKind::Inert);
            assert!(w.objects.swap_payload(id, &mut held));
            let r = f(w);
            w.objects.swap_payload(id, &mut held);
            r
        })
    }

    #[test]
    fn the_boss_calls_do_nothing_while_claws_own_call_holds_him() {
        let mut sim = flat_sim();
        let before = sim.player().unwrap().clone();
        let special = sim.claw().special.clone();
        assert!(!inside_claws_call(&mut sim, |w| w.freeze_claw_now()));
        assert!(!inside_claws_call(&mut sim, |w| w.freeze_claw_boss_end()));
        assert!(!inside_claws_call(&mut sim, |w| w.release_claw_boss_end()));
        let o = sim.player().unwrap();
        assert_eq!(
            (o.state, o.x, o.y, o.kind),
            (before.state, before.x, before.y, LogicKind::Player)
        );
        assert_eq!(sim.claw().special, special);
        // Outside it they work.
        assert!(sim.with_world(|w| w.freeze_claw_now()));
        assert_eq!(sim.player().unwrap().state, state::FREEZE);
        sim.with_world(|w| w.set_boss_fight(fight::ON));
        sim.step(TickInput::default());
        assert!(sim.with_world(|w| w.freeze_claw_boss_end()));
        assert_eq!(sim.player().unwrap().state, BOSS_END);
        assert!(sim.with_world(|w| w.release_claw_boss_end()));
        assert_eq!(sim.player().unwrap().state, state::STAND);
    }

    #[test]
    fn a_timed_freeze_becomes_the_boss_freeze_without_its_time() {
        let mut sim = flat_sim();
        assert!(sim.with_world(|w| w.freeze_claw(500)));
        sim.step(TickInput::default());
        assert!(sim.claw().special.freeze_ms > 0);
        assert!(!sim.with_world(|w| w.freeze_claw_now()), "frozen already");
        let s = &sim.claw().special;
        assert!(s.boss_freeze);
        assert_eq!(s.freeze_ms, 0);
        for _ in 0..60 {
            sim.step(TickInput::default());
        }
        assert_eq!(sim.player().unwrap().state, state::FREEZE);
    }
}
