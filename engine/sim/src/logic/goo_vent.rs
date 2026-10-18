//! Goo vents: vents in the sixth level's sewer floors that keep bubbling
//! over. A small pool of goo swells, sprays sideways into a flat splash
//! up to 150 px wide and sinks back, then starts again at once.
//!
//! Each eruption is one pass of the vent's animation ([`ANIM`]): 17
//! pictures of 200 ms each, 1 up to 9 and back down to 1. At the 18 ms
//! step a picture lasts 12 ticks, so a pass takes 204 ticks plus the
//! finishing call, [`PASS_TICKS`] in all. When a pass ends the vent waits
//! its record's counter in ms (0 on every placed vent) and starts the
//! next one. The cycle starts on the vent's first live call and runs only
//! while it is live, so two vents erupt in step only when they woke in
//! the same tick.
//!
//! The danger box is not the picture's box: every call sets it from the
//! picture shown at the start of the call, before the animation moves
//! ([`attack_rect`]), from 80 px wide on the fourth picture to 150 px on
//! the ninth, always from 12 px above the vent to 7 to 13 px below it.
//! The box therefore lags the picture by one tick: a vent is dangerous
//! for [`DANGER_TICKS`] ticks of each pass, from the tick after the
//! fourth picture appears. Whatever stands in the splash takes it as a
//! hazard's shot of the level ([`TYPE`]): Claw loses 1 health (the vent
//! deals no damage of its own, and his hit rule turns 0 into 1), is
//! pushed 27 px away from the vent in his hit state and cannot be hurt by
//! a vent again for 48 ticks; the guards, the seagulls and the rats
//! flinch without losing health, a powder keg explodes and a crate
//! breaks. Nothing can hit a vent. It plays no sound and draws no random
//! number.
//!
//! # Timing
//!
//! With C the vent's first live call (its set-up), pass n starts on
//! C + 1 + 205n: the first picture is shown on that call, the fourth on
//! its 37th call, and the box is set from its 38th call ((-40, -12, 40, 7))
//! to its 169th; the widest box is up on calls 98 to 109, and the pass
//! ends on its 205th call, which starts the next pass at once. A vent that
//! falls asleep keeps its picture and box and goes on from there when it
//! wakes.
//!
//! # Defaults taken
//!
//! - A vent is drawn at its record's depth, 0 on every placed vent, through
//!   the usual depth order: behind the floor's goo cover-ups (8000) around
//!   it. Its depth matters to play only in that it runs before Claw, so
//!   the box a call sets is the one that call's collision pass tests.

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["GooVent"];

/// The vent's picture, set on its first call (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_GOOVENT";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The eruption.
pub const ANIM: &str = "LEVEL_GOOVENT_GOOVENT";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// What a vent is to whatever it touches: a hazard's shot of the level.
pub const TYPE: u32 = types::LEVEL_SHOT;
/// What may hit a vent: a kind no object carries with a box that could
/// reach it, so nothing does.
pub const HIT_MASK: u32 = 0x1;
/// Record field holding the wait between two passes, ms.
pub const WAIT_FIELD: usize = 59;
/// Ticks of one pass at the 18 ms step, the finishing call included.
pub const PASS_TICKS: u32 = 205;
/// Dangerous ticks of each pass.
pub const DANGER_TICKS: u32 = 132;
/// Ticks of a pass before the danger starts, counted from the pass's
/// first call: the box first appears on the call after the fourth
/// picture does.
pub const HARMLESS_LEAD_TICKS: u32 = 37;
/// Ticks Claw cannot be hurt by a vent again after a hit.
pub const REHIT_TICKS: u32 = 48;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Erupting, pass after pass.
    pub const IDLE: i32 = 1;
}

/// The danger boxes by picture, 4 to 9 (pictures 1 to 3 have none).
pub const ATTACK_RECTS: [Rect; 6] = [
    Rect::new(-40, -12, 40, 7),
    Rect::new(-50, -12, 50, 8),
    Rect::new(-60, -12, 60, 10),
    Rect::new(-65, -12, 65, 11),
    Rect::new(-70, -12, 70, 12),
    Rect::new(-75, -12, 75, 13),
];

/// The danger box a call sets when picture `frame` is shown at its start:
/// `Some(Some(box))` for pictures 4 to 9, `Some(None)` (no box, harmless)
/// for pictures 1 to 3, and `None` for any other picture, which leaves
/// the box as it was.
pub fn attack_rect(frame: i32) -> Option<Option<Rect>> {
    match frame {
        1..=3 => Some(None),
        4..=9 => Some(Some(ATTACK_RECTS[(frame - 4) as usize])),
        _ => None,
    }
}

/// A vent's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GooVentData {
    /// The wait before each pass, ms (the record's counter).
    pub wait_ms: u32,
    /// Game time the current wait started, ms.
    pub start_ms: u64,
    /// First and last frame numbers of the vent's image set.
    pub frames: Option<(i32, i32)>,
    /// The eruption being played.
    pub anim: SpriteAnim,
}

/// The vent data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&GooVentData> {
    match &o.data {
        ObjectData::GooVent(d) => Some(d),
        _ => None,
    }
}

/// True when the vent `o` hurts what its box touches in this tick's
/// collision pass: its box is set.
pub fn dangerous(o: &Object) -> bool {
    o.attack_rect.is_set()
}

/// True while a pass waits to start at game time `now_ms`: the wait
/// counts as unsigned, so a wait of 0 never holds.
pub fn waiting(d: &GooVentData, now_ms: u64) -> bool {
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
        ObjectData::GooVent(d) => d,
        _ => GooVentData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::GooVent(d);
    }
}

/// Starts a pass: the eruption from its first picture (not shown until
/// the next call) and the wait from now.
fn start_pass(w: &World, d: &mut GooVentData) {
    d.anim.set(w.level.animations.get(ANIM).cloned());
    d.start_ms = w.now_ms;
}

fn set_up(w: &mut World, id: ObjectId, d: &mut GooVentData) {
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
    o.state = state::IDLE;
    start_pass(w, d);
}

fn step(w: &mut World, id: ObjectId, d: &mut GooVentData) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => set_up(w, id, d),
        state::IDLE => {
            if waiting(d, w.now_ms) {
                return;
            }
            if let Some(o) = w.objects.get_mut(id)
                && let Some(b) = attack_rect(o.frame)
            {
                o.attack_rect = b.unwrap_or(Rect::UNSET);
            }
            let frames = d.frames.unwrap_or_default();
            advance_sprite(w, id, &mut d.anim, frames);
            if d.anim.is_done() {
                start_pass(w, d);
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a vent.
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
    fn the_box_widens_from_the_fourth_picture_to_the_ninth() {
        for frame in 1..=3 {
            assert_eq!(attack_rect(frame), Some(None), "{frame}");
        }
        assert_eq!(attack_rect(4), Some(Some(Rect::new(-40, -12, 40, 7))));
        assert_eq!(attack_rect(9), Some(Some(Rect::new(-75, -12, 75, 13))));
        for frame in [0, 10, -1] {
            assert_eq!(attack_rect(frame), None, "{frame} keeps the box");
        }
        // Always 12 px above the vent, wider and lower picture by picture.
        for pair in ATTACK_RECTS.windows(2) {
            assert_eq!(pair[0].top, -12);
            assert!(pair[1].right > pair[0].right);
            assert!(pair[1].bottom >= pair[0].bottom);
            assert_eq!(pair[1].left, -pair[1].right);
        }
    }

    #[test]
    fn a_wait_of_zero_never_holds_and_the_wait_counts_unsigned() {
        let d = |wait_ms, start_ms| GooVentData {
            wait_ms,
            start_ms,
            ..GooVentData::default()
        };
        for now in [0, 18, 5000, u64::MAX] {
            assert!(!waiting(&d(0, 1000), now), "{now}");
        }
        assert!(waiting(&d(100, 1000), 1000));
        assert!(waiting(&d(100, 1000), 1099));
        assert!(!waiting(&d(100, 1000), 1100));
        // Before its start the difference wraps to a huge count.
        assert!(!waiting(&d(100, 1000), 999));
    }

    #[test]
    fn a_pass_is_seventeen_pictures_of_twelve_ticks() {
        // 17 pictures of 200 ms, each 12 ticks at 18 ms, and the
        // finishing call.
        assert_eq!(PASS_TICKS, 17 * 200u32.div_ceil(18) + 1);
        // Pictures 4 to 9 and back to 4: 11 pictures.
        assert_eq!(DANGER_TICKS, 11 * 12);
        // Pictures 1 to 3 rising, then the call that first sees picture 4.
        assert_eq!(HARMLESS_LEAD_TICKS, 3 * 12 + 1);
        // The harmless tail: pictures 3, 2 and 1 on the way down, the
        // finishing call included.
        assert_eq!(PASS_TICKS - HARMLESS_LEAD_TICKS - DANGER_TICKS, 3 * 12);
    }
}
