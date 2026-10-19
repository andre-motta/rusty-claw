//! The lava fountains: a column of lava that rises from a lava pool,
//! holds and falls back, again and again.
//!
//! A fountain waits its record's start delay (the speed field, ms, 0 on
//! most) from the moment it is built, then erupts and never stops: its
//! animation loops through its 22 pictures every [`CYCLE_TICKS`] ticks
//! (four ticks a picture), and the column's box is the row of [`COLUMN`]
//! for the picture shown, set on every tick of the eruption. Touching it
//! costs Claw [`super::DAMAGE`]. Before its first eruption it has no box.
//!
//! When Claw has lost a life since it last looked, the fountain waits its
//! start delay again from that moment, still showing its last picture
//! with that picture's box live, then erupts from its first picture. So
//! every fountain's phase after a respawn is fixed by its delay.
//!
//! With C the tick it is built on (showing its first picture, no box): a
//! fountain with no delay erupts on C + 1, its column is live from C + 2
//! and its second picture shows from C + 6. A delay of d ms ends on the
//! first tick whose game time is d ms or more past the start.
//!
//! It never moves, nothing hits it, and it draws no random number. Its
//! sound plays from its animation, once a cycle.
//!
//! # Defaults taken
//!
//! - A column held through the delay after a lost life hurts Claw as a
//!   running one does: the collision pass reads only the box.
//! - Asleep outside its window it keeps its picture and its timer; a
//!   delay that ran out while it slept ends on its first call awake.

use super::{DAMAGE, GENERIC, SPEED_FIELD, field, row};
use crate::logic::enemy::Timer;
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, engine_state, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["LavaGeyser"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_LAVAGEYSER";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The eruption, looping.
pub const ANIM: &str = "LEVEL_LAVAGEYSER";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Ticks of one eruption at the 18 ms step.
pub const CYCLE_TICKS: u64 = 88;

/// A fountain's states, in its object's state.
pub mod state {
    /// Waiting its start delay.
    pub const IDLE: i32 = 1;
    /// Erupting.
    pub const ERUPTING: i32 = 2;
}

const fn r(l: i32, t: i32, rt: i32, b: i32) -> Option<Rect> {
    Some(Rect::new(l, t, rt, b))
}

/// The column's box by picture, 1 to 19, relative to the fountain: it
/// rises from the lava 90 px below the fountain to 81 px above it.
pub const COLUMN: [Option<Rect>; 19] = [
    r(-11, 90, 6, 97),
    r(-12, 65, 13, 96),
    r(-14, 45, 11, 97),
    r(-18, 17, 12, 98),
    r(-15, -10, 12, 98),
    r(-15, -33, 12, 98),
    r(-18, -59, 12, 98),
    r(-17, -76, 12, 98),
    r(-17, -81, 12, 98),
    r(-17, -81, 12, 98),
    r(-17, -81, 12, 98),
    r(-17, -81, 12, 98),
    r(-16, -53, 12, 98),
    r(-17, -31, 12, 98),
    r(-18, -9, 12, 98),
    r(-16, 18, 12, 98),
    r(-14, 41, 12, 98),
    r(-15, 71, 12, 98),
    r(-13, 90, 12, 98),
];

/// A fountain's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GeyserData {
    /// The start delay running.
    pub wait: Timer,
    /// Claw's lost lives when it last started waiting.
    pub attempt: u32,
}

/// The column's box for picture `frame`; none outside the table.
pub fn column(frame: i32) -> Option<Rect> {
    row(&COLUMN, frame)
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut GeyserData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::LavaGeyser(d) => Some(d),
        _ => None,
    }
}

/// The data of fountain `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a GeyserData> {
    match &w.objects.get(id)?.data {
        ObjectData::LavaGeyser(d) => Some(d),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        engine_state::INIT => build(w, id),
        state::IDLE => idle_tick(w, id),
        state::ERUPTING => erupting_tick(w, id),
        _ => {}
    }
}

/// Builds a fountain: an enemy-kind object that hurts on contact, on its
/// first picture, waiting its start delay.
fn build(w: &mut World, id: ObjectId) {
    let first = w.image_frames(IMAGE_SET).map_or(1, |(f, _)| f);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.frame = first;
    o.type_flags = types::ENEMY;
    o.hit_type_flags = GENERIC;
    o.damage = DAMAGE;
    o.data = ObjectData::LavaGeyser(GeyserData::default());
    start_waiting(w, id);
}

/// Waits the start delay from now, remembering Claw's lost lives; the
/// picture and the box stay as they are.
fn start_waiting(w: &mut World, id: ObjectId) {
    let wait = u64::try_from(field(w, id, SPEED_FIELD)).unwrap_or(0);
    let (now, attempt) = (w.now_ms, w.attempt());
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::IDLE;
    }
    if let Some(d) = data_mut(w, id) {
        d.wait = Timer::new(now, wait);
        d.attempt = attempt;
    }
}

/// Idle: once the delay has run out, the eruption starts (from the next
/// tick's call on).
fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.wait.done(now)) {
        w.advance_anim(id);
        if let Some(o) = w.objects.get_mut(id) {
            o.state = state::ERUPTING;
        }
        w.set_anim(id, ANIM);
    }
}

/// Erupting: the animation, the column's box for the picture shown, and
/// after a lost life the delay again.
fn erupting_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if let Some(o) = w.objects.get_mut(id)
        && let Some(b) = column(o.frame)
    {
        o.attack_rect = b;
    }
    if w.objects.get(id).is_some_and(|o| o.anim.is_done()) {
        w.set_anim(id, ANIM);
    }
    let attempt = w.attempt();
    if data(w, id).is_some_and(|d| d.attempt != attempt) {
        start_waiting(w, id);
    }
}

/// `attacker` hit `id`: nothing hits a fountain.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: Claw's own hit rule takes the damage.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_rises_holds_and_falls() {
        assert_eq!(column(0), None);
        assert_eq!(column(20), None);
        assert_eq!(column(1), Some(Rect::new(-11, 90, 6, 97)));
        assert_eq!(column(3), Some(Rect::new(-14, 45, 11, 97)));
        // Full height on pictures 9 to 12.
        for f in 9..=12 {
            assert_eq!(column(f), Some(Rect::new(-17, -81, 12, 98)));
        }
        let tops: Vec<i32> = (1..=19).map(|f| column(f).unwrap().top).collect();
        let peak = tops.iter().copied().min().unwrap();
        assert_eq!(peak, -81);
        assert!(tops[..9].windows(2).all(|w| w[1] <= w[0]), "rising");
        assert!(tops[11..].windows(2).all(|w| w[1] >= w[0]), "falling");
        // 22 pictures of 70 ms, four ticks each.
        assert_eq!(70u64.div_ceil(18) * 22, CYCLE_TICKS);
    }
}
