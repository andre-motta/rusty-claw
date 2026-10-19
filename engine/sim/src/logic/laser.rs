//! Lasers: the eleventh level's electric traps, two crystal clusters in a
//! low corridor, one hanging from its ceiling and one on its floor, that
//! arc lightning between them on a fixed rhythm.
//!
//! A laser waits its record's counter in ms (1500 on every placed laser),
//! then plays one pass of its animation ([`ANIM`]): 25 pictures of 50 ms,
//! each lasting 3 ticks at the 18 ms step, so a pass takes 75 ticks plus
//! the finishing call ([`PASS_TICKS`]). For the whole pass, not only
//! while the arc is drawn, a fixed box 40 px wide and 200 px tall
//! ([`ATTACK_RECT`]) hurts whatever stands in it as a hazard's shot of the
//! level ([`TYPE`]): Claw loses 1 health (the laser deals no damage of its
//! own, and his hit rule turns 0 into 1), is pushed 27 px away from the
//! laser in his hit state and cannot be hurt by a hazard again for 48
//! ticks, so a Claw who stays in the beam is hit on the pass's first tick
//! and 48 ticks later, and that second knock carries him out of it. The
//! guards and other human enemies flinch without losing health and a
//! crate breaks. The pictures of the pass's first 12 and last 9 ticks show
//! only the crystals: the beam hurts there all the same. On the call that
//! finishes the pass the laser goes back to its first picture, drops its
//! box and starts waiting again, so that call is harmless.
//!
//! The corridor is 204 px tall and the box covers it from its ceiling to
//! 8 or 9 px above its floor, so Claw can neither duck nor jump under a
//! firing laser: he crosses while it waits.
//!
//! The pass carries the zaps: `LEVEL_ELECTRICZAP1` on pictures 5, 9, 13,
//! 17 and 21 and `LEVEL_ELECTRICZAP2` on pictures 7, 11, 15 and 19, played
//! at the effects volume wherever the laser is, on screen or not, while it
//! is live. Nothing can hit a laser, and it draws no random number of the
//! game's.
//!
//! # Timing
//!
//! With C the laser's first live call (its set-up) and a wait of 1500 ms,
//! pass n starts on C + 84 + 159n: harmless through C + 83, dangerous on
//! C + 84 to C + 158, harmless again from C + 159 (the finishing call) to
//! C + 242. The wait compares the game clock, which runs while the laser
//! sleeps: a laser that falls asleep during its wait and wakes after it
//! has run out fires on its first call awake; one that falls asleep in a
//! pass goes on with the pass where it stopped. At 60 frames per second
//! the 1500 ms wait is 90 frames and a pass 75, a cycle of 165 frames
//! (measured).
//!
//! # Defaults taken
//!
//! - A laser is drawn at its record's depth (0, 1000 or 5000) through the
//!   usual depth order, so the one at 5000 is drawn over Claw and the
//!   others behind him. Depth changes only the order of the update pass,
//!   and as the box is fixed and the collision pass runs after the whole
//!   update, it changes nothing in play.
//! - The human enemies' reaction to a beam is the shared one to the
//!   level's own shots ([`super::enemy::HUMAN_HIT_BY`]); no placed patrol
//!   reaches a beam.

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Laser"];

/// The laser's pictures, set on its first call (the records name them
/// too).
pub const IMAGE_SET: &str = "LEVEL_LASER";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// One pass of the beam.
pub const ANIM: &str = "LEVEL_LASER_LASER";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// What a laser is to whatever it touches: a hazard's shot of the level.
pub const TYPE: u32 = types::LEVEL_SHOT;
/// What may hit a laser: a kind no object carries with a box that could
/// reach it, so nothing does.
pub const HIT_MASK: u32 = 0x1;
/// Record field holding the wait before each pass, ms.
pub const WAIT_FIELD: usize = 59;
/// The beam's box, the same for every picture of a pass.
pub const ATTACK_RECT: Rect = Rect::new(-20, -100, 20, 100);
/// The picture a waiting laser shows.
pub const REST_FRAME: i32 = 1;
/// Ticks of one pass at the 18 ms step, the finishing call included.
pub const PASS_TICKS: u32 = 76;
/// Dangerous ticks of each pass: every call of the pass but the
/// finishing one.
pub const DANGER_TICKS: u32 = PASS_TICKS - 1;
/// Ticks of the pass, from its first, before the first picture with the
/// arc; the beam hurts on them all the same.
pub const DARK_LEAD_TICKS: u32 = 12;
/// The first zap.
pub const ZAP1: &str = "LEVEL_ELECTRICZAP1";
/// The second zap.
pub const ZAP2: &str = "LEVEL_ELECTRICZAP2";

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting, or firing a pass.
    pub const IDLE: i32 = 1;
}

/// Ticks of the wait before a pass of `wait_ms` at a step of `tick_ms`:
/// the pass starts on the first call whose clock is `wait_ms` or more
/// past the wait's start.
pub fn wait_ticks(wait_ms: u32, tick_ms: u32) -> u32 {
    wait_ms.div_ceil(tick_ms.max(1))
}

/// Ticks of a whole cycle (a wait of `wait_ms`, then a pass) at the 18 ms
/// step: the wait counts from the pass's finishing call.
pub fn period_ticks(wait_ms: u32) -> u32 {
    wait_ticks(wait_ms, crate::tuning::TICK_MS) + PASS_TICKS - 1
}

/// A laser's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LaserData {
    /// The wait before each pass, ms (the record's counter).
    pub wait_ms: u32,
    /// Game time the current wait started, ms.
    pub start_ms: u64,
    /// First and last frame numbers of the laser's image set.
    pub frames: Option<(i32, i32)>,
    /// The pass being played.
    pub anim: SpriteAnim,
}

/// The laser data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&LaserData> {
    match &o.data {
        ObjectData::Laser(d) => Some(d),
        _ => None,
    }
}

/// True when the laser `o` hurts what its box touches in this tick's
/// collision pass: its box is set.
pub fn dangerous(o: &Object) -> bool {
    o.attack_rect.is_set()
}

/// True while a pass waits to start at game time `now_ms`: the wait
/// counts as unsigned, so a wait of 0 never holds.
pub fn waiting(d: &LaserData, now_ms: u64) -> bool {
    (now_ms.wrapping_sub(d.start_ms) as u32) < d.wait_ms
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::Laser(d) => d,
        _ => LaserData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Laser(d);
    }
}

/// Starts waiting: the pass from its first picture (not shown until the
/// pass's first call), the first picture shown, no box, and the wait from
/// now.
fn enter_idle(w: &mut World, id: ObjectId, d: &mut LaserData) {
    d.anim.set(w.level.animations.get(ANIM).cloned());
    d.start_ms = w.now_ms;
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::IDLE;
        o.frame = REST_FRAME;
        o.attack_rect = Rect::UNSET;
    }
}

fn set_up(w: &mut World, id: ObjectId, d: &mut LaserData) {
    let level = w.level;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.image_set != IMAGE_SET {
        o.image_set = IMAGE_SET.to_string();
    }
    if let Some((first, _)) = level.image_frames(IMAGE_SET) {
        o.frame = first;
    }
    d.frames = Some(frames_of(level, o, d.frames));
    o.type_flags = TYPE;
    o.hit_type_flags = HIT_MASK;
    d.wait_ms = o.record.as_ref().map_or(0, |r| r[WAIT_FIELD]) as u32;
    enter_idle(w, id, d);
}

fn step(w: &mut World, id: ObjectId, d: &mut LaserData) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => set_up(w, id, d),
        state::IDLE => {
            if waiting(d, w.now_ms) {
                return;
            }
            if let Some(o) = w.objects.get_mut(id) {
                o.attack_rect = ATTACK_RECT;
            }
            let frames = d.frames.unwrap_or_default();
            advance_sprite(w, id, &mut d.anim, frames);
            if d.anim.is_done() {
                enter_idle(w, id, d);
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a laser.
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
    fn a_cycle_is_eighty_four_waiting_ticks_and_seventy_five_dangerous() {
        // 25 pictures of 50 ms, each 3 ticks at 18 ms, and the finishing
        // call.
        assert_eq!(PASS_TICKS, 25 * 50u32.div_ceil(18) + 1);
        assert_eq!(DANGER_TICKS, 75);
        assert_eq!(wait_ticks(1500, 18), 84);
        assert_eq!(period_ticks(1500), 159);
        // At 60 frames per second (17, 17 and 16 ms) the wait is 90
        // frames: 1500 ms is 30 runs of 50 ms.
        assert_eq!(1500 / 50 * 3, 90);
        // The arc shows from the fifth picture: 4 pictures of 3 ticks.
        assert_eq!(DARK_LEAD_TICKS, 4 * 3);
    }

    #[test]
    fn the_box_spans_the_corridor() {
        assert_eq!(ATTACK_RECT.right - ATTACK_RECT.left, 40);
        assert_eq!(ATTACK_RECT.bottom - ATTACK_RECT.top, 200);
        assert_eq!(ATTACK_RECT.left, -ATTACK_RECT.right);
        assert_eq!(ATTACK_RECT.top, -ATTACK_RECT.bottom);
    }

    #[test]
    fn a_wait_of_zero_never_holds_and_the_wait_counts_unsigned() {
        let d = |wait_ms, start_ms| LaserData {
            wait_ms,
            start_ms,
            ..LaserData::default()
        };
        for now in [0, 18, 5000, u64::MAX] {
            assert!(!waiting(&d(0, 1000), now), "{now}");
        }
        assert!(waiting(&d(1500, 18), 18));
        assert!(waiting(&d(1500, 18), 1517));
        assert!(!waiting(&d(1500, 18), 1518));
        // Before its start the difference wraps to a huge count.
        assert!(!waiting(&d(1500, 1000), 999));
    }
}
