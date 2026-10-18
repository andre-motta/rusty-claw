//! Dart traps: hidden blowpipes in the ninth level's walls (and in later
//! levels') that watch a rectangle of the level and shoot an arrow
//! ([`arrow`]) while Claw stands in it.
//!
//! A trap draws nothing while it waits: the hole Claw sees is a separate
//! piece of decoration. When Claw's hit box overlaps its trigger
//! rectangle (the record's bounds), it plays its puff (the record's own
//! animation, pictures 1 to 5, 100 ms each, with [`SOUND`] on the fourth),
//! and on the call that ends the puff it shoots one arrow from its own
//! position in the record's direction. It then waits the record's speed
//! field in ms ([`DEFAULT_WAIT_MS`] when 0) before it can trigger again,
//! so with Claw inside it fires every `31 + ceil(wait / 18)` ticks (the
//! puff aimed up is 6 ticks shorter). It never moves, nothing can hit it
//! and it draws no random number.
//!
//! | State | Per call |
//! | --- | --- |
//! | Idle | nothing (no picture) |
//! | Firing | entered by the trigger, which shuts the trap off from Claw and starts the puff without advancing it; advance the puff; on its first call with event 1 (no retail puff has one) or on the call that ends it, shoot the arrow once; when it ends, show no picture again and go to Wait with the wait timer started |
//! | Wait | when the trap is open to Claw, mark the next trigger as a fresh one; when the wait has run out, open it to Claw again |
//!
//! With T the tick Claw's box first meets the rectangle, the puff shows
//! its pictures from T + 1, its sound plays on T + 19 and the arrow leaves
//! on T + 31 from the trap; the next trigger can come on T + 31 +
//! ceil(wait / 18), so a trap fires every 87 ticks with a 1000 ms wait.
//!
//! # State numbers
//!
//! A trap is no enemy: its states are its own ([`state`]), held in its
//! object's state as for the other level objects.
//!
//! # Defaults taken
//!
//! - The trap's states are its own, not enemy states, so the ninth level's
//!   enemy block keeps no number for its wait.
//! - The record's first-shot delay (its smarts field, in ms, applied when
//!   Claw enters the rectangle afresh) and the animated shots of the
//!   fourteenth level's launchers (its user value 1 naming the picture set
//!   `LEVEL_PROJECTILES_<n>`) are built with the trap, though no ninth
//!   level record uses them; the levels that do confirm them.
//! - Asleep in the side areas of a wide display, a trap never woken is not
//!   drawn, as it waits ([`first_look`]).
//!
//! Its data ([`DartTrapData`]) is its own.

use super::enemy::{self, Timer};
use super::projectile::{self, ProjectileSpec};
use super::{ObjectData, World};
use crate::level::{SimLevel, runtime_image_set};
use crate::logic::Look;
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags, types};
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TProjectile"];

/// Image sets this family may switch an object to: the arrows.
pub const IMAGE_SETS: &[&str] = &[
    arrow::IMAGE_SET,
    arrow::ANIMATED_SETS[0],
    arrow::ANIMATED_SETS[1],
];

/// Prefixes of the animations this family may play: the puffs, which the
/// records name too.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SHOOTERS_PUFFDART", arrow::ANIMATED_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The puff's sound (a frame sound of the puff).
pub const SOUND: &str = "LEVEL_BLOWDART";
/// Depth of a trap placed at depth 0.
pub const DEFAULT_Z: i32 = 4010;
/// The wait after a shot when the record gives none, ms.
pub const DEFAULT_WAIT_MS: u32 = 500;
/// Speed of the arrows on an axis the record leaves at 0, px/s.
pub const DEFAULT_SPEED: i32 = 100;
/// Direction of a trap whose record gives none.
pub const DEFAULT_DIRECTION: i32 = 4;
/// Record field holding the wait, ms.
pub const WAIT_FIELD: usize = 60;
/// Record field holding the direction.
pub const DIRECTION_FIELD: usize = 63;
/// Record field holding the first-shot delay, ms.
pub const SMARTS_FIELD: usize = 17;
/// Record field holding the arrow's picture.
pub const USER0_FIELD: usize = 43;
/// Record field naming the animated shot's picture set (0 for an arrow).
pub const USER1_FIELD: usize = 44;
/// Record fields holding the trigger rectangle (left, top, right,
/// bottom).
pub const BOUNDS_FIELDS: [usize; 4] = [51, 52, 53, 54];
/// The damage of every trap, whatever its record says.
pub const DAMAGE: i32 = arrow::DAMAGE;
/// Ticks of the puff aimed left, right or down at the 18 ms step: the
/// arrow leaves on the last (measured).
pub const PUFF_TICKS: u32 = 31;
/// Ticks of the puff aimed up at the 18 ms step.
pub const PUFF_UP_TICKS: u32 = 25;
/// The puff's sound tick, from the trigger (measured).
pub const SOUND_TICK: u32 = 19;

/// The trap's states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting for Claw, drawing nothing.
    pub const IDLE: i32 = 1;
    /// Puffing; the arrow leaves when the puff ends.
    pub const FIRING: i32 = 2;
    /// Waiting before it can trigger again, drawing nothing.
    pub const WAIT: i32 = 3;
}

/// Ticks between two arrows while Claw stays in the rectangle, at the
/// 18 ms step, for a wait of `wait_ms` and a puff aimed up (`up`) or not.
pub fn period_ticks(wait_ms: u32, up: bool) -> u32 {
    let puff = if up { PUFF_UP_TICKS } else { PUFF_TICKS };
    puff + wait_ms.div_ceil(18)
}

/// The arrow a dart trap shoots: the soldier's bullet with an arrow
/// picture, an enemy's shot that flies straight in the trap's direction
/// and costs Claw [`DAMAGE`](arrow::DAMAGE); the record's user value picks
/// the picture. It is not a family of its own: the projectile family runs
/// it.
pub mod arrow {
    use crate::object::Rect;

    /// Logic name of the arrow.
    pub const LOGIC: &str = "Bullet";
    /// Its pictures.
    pub const IMAGE_SET: &str = "LEVEL_PROJECTILES";
    /// The picture sets of the animated shots, named by the record's user
    /// value 1 (1 or 2).
    pub const ANIMATED_SETS: [&str; 2] = ["LEVEL_PROJECTILES_1", "LEVEL_PROJECTILES_2"];
    /// The animation of the animated shots.
    pub const ANIMATED_ANIM: &str = "GAME_CYCLE100";
    /// Damage to Claw, whatever the record says.
    pub const DAMAGE: i32 = 5;

    /// The velocity of an arrow shot in `direction` (2 down, 4 left, 6
    /// right, 8 up) by a trap with speeds `speed`, px/s; `None` for any
    /// other direction, whose arrow does not move.
    pub fn velocity(direction: i32, (speed_x, speed_y): (i32, i32)) -> Option<(i32, i32)> {
        match direction {
            2 => Some((0, speed_y)),
            4 => Some((-speed_x, 0)),
            6 => Some((speed_x, 0)),
            8 => Some((0, -speed_y)),
            _ => None,
        }
    }

    /// The box of the arrow picture `frame` (1 right, 2 left, 3 down, 4
    /// up), the box it hits with.
    pub fn frame_box(frame: i32) -> Option<Rect> {
        match frame {
            1 => Some(Rect::new(-1, -3, 58, 2)),
            2 => Some(Rect::new(-58, -3, 1, 2)),
            3 => Some(Rect::new(-4, 0, 3, 55)),
            4 => Some(Rect::new(-4, -57, 3, -2)),
            _ => None,
        }
    }
}

/// A dart trap's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DartTrapData {
    /// The trigger rectangle, relative to the trap, once set up.
    pub trigger: Option<Rect>,
    /// The animation the record names: the puff.
    pub puff: Option<String>,
    /// The next trigger is a fresh one: it waits the record's first-shot
    /// delay.
    pub fresh: bool,
    /// The current puff has shot its arrow.
    pub fired: bool,
    /// The wait after a shot.
    pub wait: Timer,
    /// Arrows shot so far.
    pub shots: u32,
    /// The last arrow shot.
    pub last_arrow: Option<ObjectId>,
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut DartTrapData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::DartTrap(d) => Some(d),
        _ => None,
    }
}

/// The data of dart trap `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a DartTrapData> {
    match &w.objects.get(id)?.data {
        ObjectData::DartTrap(d) => Some(d),
        _ => None,
    }
}

/// A record field of `o`, 0 without a record.
fn field(o: &Object, i: usize) -> i32 {
    o.record.as_deref().map_or(0, |r| r[i])
}

/// The animation trap `o` was placed with (the puff), when the level has
/// it; otherwise the one named as its picture set.
fn placed_puff(level: &SimLevel, o: &Object) -> Option<String> {
    let (x, y, _) = o.editor;
    level
        .objects
        .iter()
        .find(|s| {
            s.record[0] == o.id && s.record[5] == x && s.record[6] == y && *s.logic == *o.logic
        })
        .map(|s| s.anim.clone())
        .filter(|a| level.animations.contains_key(a))
        .or_else(|| {
            level
                .animations
                .contains_key(&o.image_set)
                .then(|| o.image_set.clone())
        })
}

/// The set-up of trap `o` on its first call (see the module
/// documentation): a safe, generic object attacking Claw, with its
/// defaults, its trigger rectangle and no picture; it waits Idle.
fn set_up(level: &SimLevel, o: &mut Object) {
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    o.flags |= flags::SAFE;
    o.type_flags = 1;
    o.attack_type_flags = types::PLAYER;
    o.attack_handler = true;
    if o.speed_x == 0 {
        o.speed_x = DEFAULT_SPEED;
    }
    if o.speed_y == 0 {
        o.speed_y = DEFAULT_SPEED;
    }
    o.damage = DAMAGE;
    let [l, t, r, b] = BOUNDS_FIELDS.map(|i| field(o, i));
    let trigger = Rect::new(l - o.x, t - o.y, r - o.x, b - o.y);
    o.attack_rect = trigger;
    o.frame = 0;
    o.state = state::IDLE;
    o.data = ObjectData::DartTrap(DartTrapData {
        trigger: Some(trigger),
        puff: placed_puff(level, o),
        fresh: true,
        ..DartTrapData::default()
    });
}

/// The wait after a shot of trap `o`, ms.
pub fn wait_ms(o: &Object) -> u32 {
    match field(o, WAIT_FIELD) {
        0 => DEFAULT_WAIT_MS,
        w => w as u32,
    }
}

/// The direction of trap `o`'s arrows.
pub fn direction(o: &Object) -> i32 {
    match field(o, DIRECTION_FIELD) {
        0 => DEFAULT_DIRECTION,
        d => d,
    }
}

/// How a dart trap whose first call has not run yet looks once it has
/// (see [`super::first_look`]): not drawn. `None` once it has run.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    let _ = level;
    (o.state == engine_state::INIT).then(Vec::new)
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => {
            let level = w.level;
            if let Some(o) = w.objects.get_mut(id) {
                set_up(level, o);
            }
        }
        state::FIRING => firing_tick(w, id),
        state::WAIT => wait_tick(w, id),
        _ => {}
    }
}

/// Firing: the puff plays; the arrow leaves on an event call or on the
/// call that ends the puff; then the trap shows nothing and waits.
fn firing_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    let fired = data(w, id).is_some_and(|d| d.fired);
    if event == 1 && !fired {
        shoot(w, id);
    }
    if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
        if data(w, id).is_some_and(|d| !d.fired) {
            shoot(w, id);
        }
        let now = w.now_ms;
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        o.frame = 0;
        o.state = state::WAIT;
        let wait = wait_ms(o);
        if let Some(d) = data_mut(w, id) {
            d.wait = Timer::new(now, u64::from(wait));
        }
    }
}

/// Wait: a trap open to Claw takes its next trigger as a fresh one; the
/// end of the wait opens it to Claw again.
fn wait_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some(open) = w.objects.get(id).map(|o| o.flags & flags::NO_COLLIDE == 0) else {
        return;
    };
    let Some(d) = data_mut(w, id) else {
        return;
    };
    if open {
        d.fresh = true;
    }
    if d.wait.done(now)
        && let Some(o) = w.objects.get_mut(id)
    {
        o.flags &= !flags::NO_COLLIDE;
    }
}

/// Shoots the trap's arrow from its own position (see [`arrow`]).
fn shoot(w: &mut World, id: ObjectId) {
    let Some((x, y, z, speeds, dir, user0, user1)) = w.objects.get(id).map(|o| {
        (
            o.x,
            o.y,
            o.z,
            (o.speed_x, o.speed_y),
            direction(o),
            field(o, USER0_FIELD),
            field(o, USER1_FIELD),
        )
    }) else {
        return;
    };
    if let Some(d) = data_mut(w, id) {
        d.fired = true;
    }
    let (vx, vy) = arrow::velocity(dir, speeds).unwrap_or((0, 0));
    let shot = projectile::spawn(
        w,
        ProjectileSpec {
            logic: arrow::LOGIC,
            x,
            y,
            z: z - 1,
            vx,
            vy,
            damage: DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: "",
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
    if let Some(a) = shot {
        dress_arrow(w, a, user0, user1);
        start_shot_clock(w, a);
    }
    if let Some(d) = data_mut(w, id) {
        d.shots += 1;
        d.last_arrow = shot;
    }
}

/// Starts the flight clock of the shot `id` made in this tick by a trap
/// or a skull cannon, which run the new shot's logic once as they make it:
/// its clock reads the game time at the end of this tick, so its first
/// motion step, in the next tick, covers the difference of two such
/// readings (like every later one) rather than exactly one tick, and now
/// and then moves it a pixel less (see
/// [`crate::logic::projectile::ProjectileData::clock_s`]).
pub fn start_shot_clock(w: &mut World, id: ObjectId) {
    let now = (w.now_ms + u64::from(TICK_MS)) as f64 * 0.001;
    if let Some(ObjectData::Projectile(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
        d.clock_s = Some(now);
    }
}

/// Gives a just shot arrow its own picture: frame `user0` of the arrows,
/// or for `user1` 1 or 2 the animated shot of that set, never mirrored,
/// its boxes those of the picture it shows.
fn dress_arrow(w: &mut World, a: ObjectId, user0: i32, user1: i32) {
    let (set, frame) = match user1 {
        0 => (arrow::IMAGE_SET, user0),
        1 | 2 => {
            let set = arrow::ANIMATED_SETS[(user1 - 1) as usize];
            (set, w.image_frames(set).map_or(1, |f| f.0))
        }
        _ => return,
    };
    let b = w
        .frame_box(set, frame)
        .or_else(|| (user1 == 0).then(|| arrow::frame_box(frame)).flatten());
    let Some(o) = w.objects.get_mut(a) else {
        return;
    };
    o.image_set = runtime_image_set(set);
    o.frame = frame;
    o.draw_flags &= !draw_flags::MIRROR;
    if let Some(b) = b {
        o.move_rect = b;
        o.attack_rect = b;
    }
    if user1 != 0 {
        enemy::set_anim(w, a, arrow::ANIMATED_ANIM);
    } else if let Some(o) = w.objects.get_mut(a) {
        o.anim = Default::default();
    }
}

/// `attacker` hit `id`. Nothing hits a trap.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: Claw stepped into the trigger rectangle. The
/// trap shuts itself off from Claw and starts its puff, not advanced; a
/// fresh trigger first waits the record's first-shot delay.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if w.claw_id != Some(victim) {
        return;
    }
    let Some((puff, fresh)) = data(w, id).map(|d| (d.puff.clone(), d.fresh)) else {
        return;
    };
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.flags |= flags::NO_COLLIDE;
    o.state = state::FIRING;
    if fresh {
        let smarts = field(o, SMARTS_FIELD);
        if smarts > 0 {
            o.time_delay = smarts as u32;
        }
    }
    if let Some(d) = data_mut(w, id) {
        d.fresh = false;
        d.fired = false;
    }
    match puff {
        Some(key) => enemy::set_anim(w, id, &key),
        None => {
            if let Some(o) = w.objects.get_mut(id) {
                o.anim = Default::default();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trap_fires_every_puff_and_wait() {
        // Measured: 87 ticks with a 1000 ms wait.
        assert_eq!(period_ticks(1000, false), 87);
        assert_eq!(period_ticks(500, false), 59);
        assert_eq!(period_ticks(500, true), 53);
        assert_eq!(PUFF_TICKS - PUFF_UP_TICKS, 6);
        assert_eq!(period_ticks(DEFAULT_WAIT_MS, false), 31 + 28);
    }

    #[test]
    fn arrows_fly_the_traps_way() {
        let speeds = (500, 1000);
        assert_eq!(arrow::velocity(2, speeds), Some((0, 1000)));
        assert_eq!(arrow::velocity(4, speeds), Some((-500, 0)));
        assert_eq!(arrow::velocity(6, speeds), Some((500, 0)));
        assert_eq!(arrow::velocity(8, speeds), Some((0, -1000)));
        assert_eq!(arrow::velocity(5, speeds), None);
        // Each picture points the way it flies and reaches 55 to 58 px
        // ahead of the trap.
        for (frame, dir) in [(1, 6), (2, 4), (3, 2), (4, 8)] {
            let b = arrow::frame_box(frame).expect("an arrow");
            let (vx, vy) = arrow::velocity(dir, (1, 1)).unwrap();
            let reach = match (vx, vy) {
                (1, 0) => b.right,
                (-1, 0) => -b.left,
                (0, 1) => b.bottom,
                _ => -b.top,
            };
            assert!((55..=58).contains(&reach), "{frame}: {reach}");
        }
        assert_eq!(arrow::frame_box(0), None);
    }

    #[test]
    fn its_states_are_its_own() {
        let states = [state::SET_UP, state::IDLE, state::FIRING, state::WAIT];
        for (i, s) in states.iter().enumerate() {
            assert!(!states[i + 1..].contains(s));
            assert_eq!(crate::logic::enemy::EnemyState::from_id(*s), None);
        }
    }
}
