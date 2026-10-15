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
//! animation ends he stands again. With the double-jump fix (the default,
//! see [`crate::tuning::Behaviour::fix_double_jump`]) a reaction that ends
//! in mid-air hands over to landing instead, which turns into a fall on
//! the next tick, so Claw cannot jump again in mid-air.
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
//! freeze curse or a boss stage) stops Claw: he stands facing right and
//! reads no input until [`World::thaw_claw`] releases him, or until the
//! freeze's time runs out when it has one.
//!
//! A logic may also start the hit reaction with
//! [`World::claw_hit_reaction`]. Requests from logics take effect on
//! Claw's next logic call. Forcing one of these states without its entry
//! (tools and tests) returns Claw to standing, as a state that is not
//! filled in does (see [`super::fallback_tick`]).

use claw_frame::{Buttons, SoundChannel};

use super::{Ctx, DT, DT_S, attack, fallback_tick, state};
use crate::logic::{self, World};
use crate::object::{ObjectId, Rect, flags};
use crate::physics::{opt, ptype};

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
        tx += c.o.speed_x * DT / 1000;
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
        c.p.mover.step(DT_S);
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
    if DT >= c.p.special.lift_ms {
        let_go(c, 0);
        restore_move_rect(c);
        super::enter_stand(c);
    } else {
        c.p.special.lift_ms -= DT;
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
        c.p.special.throw_ms += DT;
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

/// Frozen (state 5009): the animation plays and nothing else happens until
/// the freeze is released or its time runs out; then Claw stands.
pub fn freeze_tick(c: &mut Ctx) {
    if c.p.special.freeze_pending {
        let ms = c.p.special.freeze_ms;
        freeze_now(c, ms);
    } else if !c.p.special.frozen {
        fallback_tick(c);
        return;
    }
    c.advance_anim();
    let s = &mut c.p.special;
    let timed_out = s.freeze_ms > 0 && {
        s.freeze_ms = (s.freeze_ms - DT).max(0);
        s.freeze_ms == 0
    };
    if s.thaw || timed_out {
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
