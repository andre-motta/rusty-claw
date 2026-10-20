//! Stalactites: the falling spikes of the twelfth level's caves.
//!
//! A stalactite hangs from a ceiling and watches an invisible trigger
//! rectangle stored in its record ([`TRIGGER_FIELDS`], in level
//! coordinates). On the first tick it finds Claw's position inside that
//! rectangle (left and top edges in, right and bottom edges out) it lets
//! go:
//!
//! - after the record's delay ([`DELAY_FIELD`], 0 on every placed
//!   stalactite, so on the next tick) it snaps ([`SNAP_SOUND`]) and falls
//!   under [`GRAVITY`], at most [`super::enemy::motion::STEP_LIMIT`] px a
//!   tick: n ticks into the fall it is `trunc(0.324 n^2)` px below its
//!   home at the 18 ms step;
//! - it falls through the water's Death rows and lands on the first Solid
//!   or Ground row under its feet; on the next tick it crumbles
//!   ([`CRUMBLE_ANIM`], [`HIT_SOUND`]), out of every collision, and 19
//!   ticks later it is hidden;
//! - it stays gone until Claw loses a life, then hangs at its home again
//!   on its first call after the death.
//!
//! While it hangs and while it falls it is an enemy's body ([`TYPE`]) with
//! the box of its picture ([`BOX`]): touching it costs Claw [`DAMAGE`] and
//! a flinch through the shared enemy contact. Nothing Claw has can hurt
//! it: what it may be hit by ([`HIT_MASK`]) is a kind no object of the
//! level carries, and a pistol shot, whose own box is unset, flies through
//! it. A stalactite that touches Aquatis reaches his file
//! ([`super::enemy::aquatis::stalactite_hit`]).
//!
//! It draws nothing from the random streams and creates nothing.
//!
//! # Timing
//!
//! With T the tick that finds Claw inside the rectangle and N the ticks
//! its fall takes, it falls from T + 1, lands on T + N, crumbles from
//! T + N + 1 (pictures 2 to 7 on T + N + 2, + 5, ..., + 17) and is hidden
//! on T + N + 20. The first stalactite over the water's edge, 202 px above
//! its floor, lands on T + 25 and is hidden on T + 45 (measured; at 60
//! frames per second it lands on frame T + 27 and is hidden on T + 47).
//!
//! # Defaults taken
//!
//! - Claw's position it tests is the one his last update left: the
//!   stalactites (depth 1010) update before him (4000) in the depth-ordered
//!   update pass, as in the original.

use claw_frame::SoundChannel;

use super::enemy::motion::axis_step_for;
use super::{ObjectData, World, sound};
use crate::object::{Object, ObjectId, Rect, draw_flags, flags, types};
use crate::physics::ptype;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Stalactite"];

/// Its pictures: the hanging spike, then the six of its crumble.
pub const IMAGE_SET: &str = "LEVEL_PROJECTILES_STALACTITE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The hanging picture.
pub const HOME_ANIM: &str = "LEVEL_STALACTITE_HOME";
/// The crumble after its fall.
pub const CRUMBLE_ANIM: &str = "LEVEL_STALACTITE_CRUMBLE";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_STALACTITE_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The snap as it lets go.
pub const SNAP_SOUND: &str = "LEVEL_STALACTITESNAP2";
/// The crash as it lands.
pub const HIT_SOUND: &str = "LEVEL_STALACTITEHIT2";

/// What it is to whatever it touches: an enemy's body.
pub const TYPE: u32 = types::ENEMY;
/// What may hit it: a kind none of the level's objects carries.
pub const HIT_MASK: u32 = 0x1;
/// Damage of touching it, before the difficulty factor.
pub const DAMAGE: i32 = 5;
/// Its box (movement, hit and contact): its hanging picture's, which it
/// keeps through the crumble.
pub const BOX: Rect = Rect::new(-40, -80, 7, 43);
/// Its fall's acceleration, px/s/s downwards.
pub const GRAVITY: f64 = 2000.0;
/// Record fields of the trigger rectangle: left, top, right, bottom, in
/// level coordinates.
pub const TRIGGER_FIELDS: [usize; 4] = [35, 36, 37, 38];
/// Record field of the wait between the trigger and the fall, ms.
pub const DELAY_FIELD: usize = 60;
/// Ticks of the crumble at the 18 ms step: six pictures of 50 ms, the
/// last finishing on the 19th call.
pub const CRUMBLE_TICKS: u32 = 19;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Hanging, watching its rectangle.
    pub const IDLE: i32 = 1;
    /// Let go, waiting the record's delay.
    pub const DELAY: i32 = 2;
    /// Falling.
    pub const FALLING: i32 = 3;
    /// Crumbling where it landed.
    pub const DEAD: i32 = 4;
    /// Gone, until Claw loses a life.
    pub const RESTART: i32 = 5;
}

/// A stalactite's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StalactiteData {
    /// Where it hangs.
    pub home: (i32, i32),
    /// The trigger rectangle, in level coordinates: left, top, right and
    /// bottom (the right and bottom edges out).
    pub trigger: [i32; 4],
    /// The wait between the trigger and the fall, ms.
    pub delay_ms: u32,
    /// Game time it was triggered, ms.
    pub start_ms: u64,
    /// Speed downwards, px/s.
    pub vy: f64,
    /// How far its exact height is past its whole pixel, px.
    pub carry: f64,
    /// Game time of its last call, ms: a fall resumed after sleeping
    /// covers the time since.
    pub last_move_ms: Option<u64>,
    /// Claw's attempt number when it was hidden.
    pub attempt: u32,
}

/// The stalactite data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&StalactiteData> {
    match &o.data {
        ObjectData::Stalactite(d) => Some(d),
        _ => None,
    }
}

/// True when (x, y) lies in the rectangle `[left, top, right, bottom]`
/// with its left and top edges and without its right and bottom ones.
pub fn in_trigger([l, t, r, b]: [i32; 4], (x, y): (i32, i32)) -> bool {
    l <= x && x < r && t <= y && y < b
}

/// How far below its home a stalactite is after `n` ticks of falling at
/// the 18 ms step, px, while the step limit has not been reached (up to
/// 37 ticks, 443 px): `trunc(0.324 n^2)`.
pub fn fall_after(n: u32) -> i32 {
    (0.324 * f64::from(n * n)) as i32
}

/// Ticks a fall of `distance` px takes at the 18 ms step: the first tick
/// whose drop reaches it, at most 24 px a tick past the 37th.
pub fn fall_ticks(distance: i32) -> u32 {
    let mut v = 0.0;
    let mut y = 0.0;
    for n in 1..10_000 {
        y += axis_step_for(&mut v, GRAVITY, f64::from(TICK_MS) * 0.001);
        if y.trunc() as i32 >= distance {
            return n;
        }
    }
    u32::MAX
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::Stalactite(d) => d,
        _ => StalactiteData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Stalactite(d);
    }
}

fn set_state(w: &mut World, id: ObjectId, s: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = s;
    }
}

fn state_of(w: &World, id: ObjectId) -> Option<i32> {
    w.objects.get(id).map(|o| o.state)
}

fn set_up(w: &mut World, id: ObjectId, d: &mut StalactiteData) {
    let level = w.level;
    let first = level.image_frames(IMAGE_SET).map_or(1, |f| f.0);
    let rect = level.frame_box(IMAGE_SET, first).unwrap_or(BOX);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.image_set != IMAGE_SET {
        o.image_set = IMAGE_SET.to_string();
    }
    o.frame = first;
    o.physics_type = ptype::DIRECT;
    o.type_flags = TYPE;
    o.hit_type_flags = HIT_MASK;
    o.attack_type_flags = 0;
    o.move_rect = rect;
    o.hit_rect = rect;
    o.attack_rect = rect;
    o.damage = DAMAGE;
    let field = |i: usize| o.record.as_ref().map_or(0, |r| r[i]);
    d.trigger = TRIGGER_FIELDS.map(field);
    d.delay_ms = field(DELAY_FIELD) as u32;
    d.home = (o.x, o.y);
    enter_idle(w, id);
}

/// Hangs: the hanging picture, shown at once.
fn enter_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, state::IDLE);
    if w.set_anim(id, HOME_ANIM) {
        w.advance_anim(id);
    }
}

/// Lets go: falls from now, in every collision, with the snap.
fn enter_falling(w: &mut World, id: ObjectId, d: &mut StalactiteData) {
    w.advance_anim(id);
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::FALLING;
        o.physics_type = ptype::FLY;
        o.flags &= !flags::NO_COLLIDE;
    }
    d.vy = 0.0;
    sound::play(w, SNAP_SOUND, SoundChannel::Effect);
}

/// Lands: crumbles where it is, out of every collision, with the crash.
fn enter_dead(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::DEAD;
        o.flags |= flags::NO_COLLIDE;
    }
    w.set_anim(id, CRUMBLE_ANIM);
    sound::play(w, HIT_SOUND, SoundChannel::Effect);
}

/// Gone: hidden until Claw loses a life.
fn enter_restart(w: &mut World, id: ObjectId, d: &mut StalactiteData) {
    set_state(w, id, state::RESTART);
    w.set_anim(id, HOME_ANIM);
    if let Some(o) = w.objects.get_mut(id) {
        o.draw_flags |= draw_flags::HIDDEN;
    }
    d.attempt = w.attempt();
}

/// Hangs again at its home, shown and in every collision.
fn restore(w: &mut World, id: ObjectId, d: &mut StalactiteData) {
    if let Some(o) = w.objects.get_mut(id) {
        o.draw_flags &= !draw_flags::HIDDEN;
        o.flags &= !flags::NO_COLLIDE;
        (o.x, o.y) = d.home;
        o.physics_type = ptype::DIRECT;
    }
    d.vy = 0.0;
    d.carry = 0.0;
    enter_idle(w, id);
}

/// The fall's step at the end of a falling call: the move of the time
/// since the last call (one tick, or after a sleep the whole time, which
/// only the step limit bounds), through the tiles without platforms.
fn fall(w: &mut World, id: ObjectId, d: &mut StalactiteData, last: Option<u64>) {
    let since = last.map_or(u64::from(TICK_MS), |t| w.now_ms.saturating_sub(t));
    if since == 0 {
        return;
    }
    let dt = since.max(u64::from(TICK_MS)) as f64 * 0.001;
    let Some((x, y)) = w.objects.get(id).map(|o| (o.x, o.y)) else {
        return;
    };
    let py = f64::from(y) + d.carry + axis_step_for(&mut d.vy, GRAVITY, dt);
    let ty = py.trunc() as i32;
    w.physics(id, x, ty, 0);
    let now_y = w.objects.get(id).map_or(ty, |o| o.y);
    d.carry = if now_y == ty { py - f64::from(ty) } else { 0.0 };
}

fn step(w: &mut World, id: ObjectId, d: &mut StalactiteData) {
    let Some(s) = state_of(w, id) else {
        return;
    };
    let last = d.last_move_ms.replace(w.now_ms);
    match s {
        state::SET_UP => set_up(w, id, d),
        state::IDLE => {
            let inside = w
                .claw_object()
                .is_some_and(|c| in_trigger(d.trigger, (c.x, c.y)));
            if inside {
                set_state(w, id, state::DELAY);
                d.start_ms = w.now_ms;
            }
        }
        state::DELAY if w.now_ms.wrapping_sub(d.start_ms) as u32 >= d.delay_ms => {
            enter_falling(w, id, d);
            fall(w, id, d, last);
        }
        state::FALLING => {
            let landed = w
                .objects
                .get(id)
                .is_some_and(|o| o.physics_type == ptype::STAND);
            if landed {
                enter_dead(w, id);
            } else {
                fall(w, id, d, last);
            }
        }
        state::DEAD => {
            w.advance_anim(id);
            if w.objects.get(id).is_some_and(|o| o.anim.is_done()) {
                enter_restart(w, id, d);
            }
        }
        state::RESTART if w.attempt() != d.attempt => restore(w, id, d),
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hurts a stalactite.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The victim's own hit handler decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fall_is_a_third_of_n_squared_px() {
        let mut v = 0.0;
        let mut y = 0.0;
        for n in 1..=37u32 {
            y += axis_step_for(&mut v, GRAVITY, 0.018);
            assert_eq!(y.trunc() as i32, fall_after(n), "tick {n}");
        }
        // The first five ticks of the stalactite over the water's edge.
        let drops: Vec<i32> = (1..=5).map(fall_after).collect();
        assert_eq!(drops, [0, 1, 2, 5, 8]);
        // Then the step limit: 24 px a tick from the 38th.
        let before = y;
        y += axis_step_for(&mut v, GRAVITY, 0.018);
        assert_eq!(y - before, 24.0);
        assert_eq!(fall_after(37), 443);
    }

    #[test]
    fn a_fall_of_202_px_takes_25_ticks() {
        assert_eq!(fall_ticks(202), 25);
        assert_eq!(fall_ticks(201), 25);
        assert_eq!(fall_ticks(0), 1);
        // Past the step limit, 24 px a tick.
        assert_eq!(fall_ticks(443), 37);
        assert_eq!(fall_ticks(443 + 24), 38);
    }

    #[test]
    fn the_trigger_counts_its_left_and_top_edges_only() {
        let r = [2879, 3582, 3072, 3648];
        assert!(in_trigger(r, (2879, 3582)));
        assert!(in_trigger(r, (3071, 3647)));
        assert!(!in_trigger(r, (3072, 3600)));
        assert!(!in_trigger(r, (3000, 3648)));
        assert!(!in_trigger(r, (2878, 3600)));
    }

    #[test]
    fn its_box_is_its_hanging_pictures() {
        assert_eq!(BOX.bottom, 43, "its feet row is 43 px below it");
        assert_eq!(
            (BOX.right - BOX.left + 1, BOX.bottom - BOX.top + 1),
            (48, 124)
        );
        assert_eq!(CRUMBLE_TICKS, 6 * 50u32.div_ceil(18) + 1);
    }
}
