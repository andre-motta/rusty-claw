//! The lava hands: a hand that rises out of the lava and throws lava
//! balls ([`super::ball`]) at Claw when he enters a box drawn around it.
//!
//! The box is the record's patrol limits, as an attack box that only
//! reaches Claw and never hurts him: it triggers the hand. A trigger takes
//! the hand out of pair collision and starts a burst of the record's
//! smarts value of rises (one when 0 or less), one ball per rise. Each
//! rise (the hand's animation, [`RISE_TICKS`] ticks) faces Claw's side as
//! it starts, and on its first tick showing [`THROW_FRAME`] a ball leaves
//! the hand aimed [`AIM`] px to that side. After the last rise the hand
//! rests the record's speed field (ms) and takes part in pair collision
//! again: Claw in its box then triggers it once the rest is over.
//!
//! With T the tick of the trigger (its pair pass), the first rise shows
//! from T + 1 and its ball leaves on T + 46; each later rise starts 68
//! ticks after the one before; the rest starts on the tick the last rise
//! ends.
//!
//! Nothing hits a hand, it never moves, and it draws no random number.

use super::{BOUNDS_FIELDS, GENERIC, SMARTS_FIELD, SPEED_FIELD, ball, field};
use crate::logic::enemy::Timer;
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, draw_flags, engine_state, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["LavaHand"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_LAVAHAND";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// A rise.
pub const ANIM: &str = "LEVEL_LAVAHAND";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// The picture a ball leaves on.
pub const THROW_FRAME: i32 = 10;
/// How far to Claw's side a ball is aimed, px.
pub const AIM: i32 = 150;
/// Ticks of a rise at the 18 ms step (the next starts on the tick after).
pub const RISE_TICKS: u64 = 67;

/// A hand's states, in its object's state.
pub mod state {
    /// Waiting for Claw, or resting.
    pub const IDLE: i32 = 1;
    /// Rising and throwing.
    pub const THROWING: i32 = 2;
}

/// A hand's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HandData {
    /// The rest after a burst.
    pub rest: Timer,
    /// Balls thrown in this burst.
    pub thrown: u32,
    /// Balls in a burst.
    pub burst: u32,
    /// This rise has thrown its ball.
    pub thrown_this_rise: bool,
    /// This rise throws to the right.
    pub right: bool,
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut HandData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::LavaHand(d) => Some(d),
        _ => None,
    }
}

/// The data of hand `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a HandData> {
    match &w.objects.get(id)?.data {
        ObjectData::LavaHand(d) => Some(d),
        _ => None,
    }
}

/// The balls in a burst for a record's smarts value: at least one.
pub fn burst_of(smarts: i32) -> u32 {
    u32::try_from(smarts).unwrap_or(0).max(1)
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        engine_state::INIT => build(w, id),
        state::THROWING => throwing_tick(w, id),
        _ => {}
    }
}

/// Builds a hand: safe from every hit, its trigger box the record's
/// limits, attacking Claw only to be triggered.
fn build(w: &mut World, id: ObjectId) {
    let [l, t, r, b] = BOUNDS_FIELDS.map(|i| field(w, id, i));
    let burst = burst_of(field(w, id, SMARTS_FIELD));
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.flags |= flags::SAFE;
    o.type_flags = GENERIC;
    o.attack_type_flags = types::PLAYER;
    o.attack_handler = true;
    o.attack_rect = Rect::new(l - o.x, t - o.y, r - o.x, b - o.y);
    // No rest to wait before the first trigger.
    o.data = ObjectData::LavaHand(HandData {
        burst,
        ..HandData::default()
    });
    go_idle(w, id);
}

/// Idle again: back in pair collision, no ball thrown in the burst.
fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.flags &= !flags::NO_COLLIDE;
        o.state = state::IDLE;
    }
    if let Some(d) = data_mut(w, id) {
        d.thrown = 0;
    }
}

/// Starts a rise, facing Claw's side (to the right when he stands at or
/// right of the hand).
fn start_rise(w: &mut World, id: ObjectId) {
    let claw_x = w.claw_object().map(|c| c.x);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let right = claw_x.is_some_and(|cx| o.x <= cx);
    if right {
        o.draw_flags |= draw_flags::MIRROR;
    } else {
        o.draw_flags &= !draw_flags::MIRROR;
    }
    o.state = state::THROWING;
    w.set_anim(id, ANIM);
    if let Some(d) = data_mut(w, id) {
        d.thrown_this_rise = false;
        d.right = right;
    }
}

/// Throwing: a ball on the rise's first throwing picture; at the rise's
/// end the next rise, or the rest after the last.
fn throwing_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some(o) = w.objects.get(id) else {
        return;
    };
    let (x, y, z, frame) = (o.x, o.y, o.z, o.frame);
    let Some(d) = data(w, id).cloned() else {
        return;
    };
    if frame == THROW_FRAME && !d.thrown_this_rise {
        let aim = if d.right { AIM } else { -AIM };
        ball::spawn(w, x, y, z - 1, aim);
        if let Some(d) = data_mut(w, id) {
            d.thrown += 1;
            d.thrown_this_rise = true;
        }
    }
    if !w.objects.get(id).is_some_and(|o| o.anim.is_done()) {
        return;
    }
    if data(w, id).is_some_and(|d| d.thrown >= d.burst) {
        let now = w.now_ms;
        let rest = u64::try_from(field(w, id, SPEED_FIELD)).unwrap_or(0);
        if let Some(d) = data_mut(w, id) {
            d.rest = Timer::new(now, rest);
        }
        go_idle(w, id);
    } else {
        start_rise(w, id);
    }
}

/// `attacker` hit `id`: nothing hits a hand.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` reached `victim` (Claw, the only kind it reaches) with its box:
/// with its rest over, the hand leaves pair collision and starts a burst.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if w.claw_id != Some(victim) {
        return;
    }
    let now = w.now_ms;
    let idle = w.objects.get(id).is_some_and(|o| o.state == state::IDLE);
    if !idle || !data(w, id).is_some_and(|d| d.rest.done(now)) {
        return;
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NO_COLLIDE;
    }
    start_rise(w, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_has_at_least_one_ball() {
        assert_eq!(burst_of(0), 1);
        assert_eq!(burst_of(-3), 1);
        assert_eq!(burst_of(1), 1);
        assert_eq!(burst_of(3), 3);
    }

    #[test]
    fn a_rise_takes_sixty_seven_ticks() {
        // Eleven pictures of 80 ms (five ticks) and three of 60 ms (four).
        assert_eq!(80u64.div_ceil(18) * 11 + 60u64.div_ceil(18) * 3, RISE_TICKS);
        // The ball leaves on the tenth picture's first tick: 46 ticks
        // after the trigger.
        assert_eq!(80u64.div_ceil(18) * 9 + 1, 46);
    }
}
