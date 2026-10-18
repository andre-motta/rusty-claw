//! Ground blowers: steam vents set into the sixth level's street and
//! rooftop floors. About once a second a blower fires a jet of steam
//! upward, and if Claw is over its nozzle at that moment the jet throws
//! him straight up, 450 px unless the record's y_max field gives another
//! height ([`height_of`]; two blowers use 400).
//!
//! The throw is the springboard's ([`crate::player::bounce`]): Claw is put
//! into his ordinary jump with a target height instead of a speed, so
//! holding or tapping Jump changes nothing, and the fall after the apex is
//! an ordinary fall. Unlike a springboard a blower does not care whether
//! Claw stands on it: the only test is whether Claw's movement box meets
//! the nozzle box ([`nozzle_box`], right and bottom edges excluded) on a
//! call whose animation step leaves the jet on its first picture
//! ([`THROW_FRAME`]). Walking, crouching, jumping or falling onto the
//! nozzle all throw him alike, as long as his feet are within about 20 px
//! of the floor. A throw drops what he carries, as a springboard's does.
//!
//! The jet ([`ANIM`]) loops freely: pictures 1 to 17 for 25 ms each
//! (2 ticks), then picture 18 for 400 ms (23 ticks), 57 ticks a loop at
//! the 18 ms step, so only two ticks of a loop can throw. The loop runs
//! only while the blower is live: a blower wakes with the small objects'
//! window and sleeps while the camera follows a thrown Claw up, so a
//! Claw standing still on a nozzle is thrown every 85 ticks (78 for a
//! 400 px blower). The fourth picture plays [`HISS`] from every live
//! blower, on screen or not.
//!
//! It touches nothing else: enemies, kegs and crates rest on or walk over
//! a blower unaffected, and nothing can hit it. It draws no random number.
//!
//! # Timing
//!
//! With C the blower's first live call (its set-up, which already shows
//! the first picture), loop n shows the first picture on C + 57n and
//! C + 57n + 1, and those calls test for Claw (all but C itself); the
//! hiss plays on C + 57n + 6. The blower runs after Claw in each tick, so
//! the test sees him where his own update left him, a jump he started in
//! the same tick is replaced by the throw, and his first rising step is
//! in the next tick. A thrown Claw is usually out of the nozzle box by the
//! second test call; a Claw whose first rising step is short enough to
//! leave him inside it is thrown again from there.
//!
//! # Defaults taken
//!
//! - Picture 18 is not in the blower's image set: while the jet shows it
//!   the blower draws nothing (23 ticks of every loop). That has no effect
//!   on play.
//! - The hiss goes through the frame sound path every animated object
//!   uses, with its replay limits, as the springboard's rustle does.
//! - A throw while Claw carries a keg drops the keg exactly as a
//!   springboard's throw does (the throw ends Claw's attack and lift
//!   first).

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, flags};
use crate::player;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["GroundBlower"];

/// The picture every placed blower names; the code keeps the record's.
pub const IMAGE_SET: &str = "LEVEL_BLOWING1";

/// Image sets this family may switch an object to: none (a blower keeps
/// its record's set).
pub const IMAGE_SETS: &[&str] = &[];

/// The jet's loop.
pub const ANIM: &str = "LEVEL_GROUNDBLOWER";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The jet's picture on which a blower throws Claw.
pub const THROW_FRAME: i32 = 1;
/// The hiss, the frame sound of the jet's fourth picture.
pub const HISS: &str = "LEVEL_STEAM3";
/// Height of the throw's apex above Claw, px, for a record whose y_max
/// field is zero or negative: the springboard's.
pub const DEFAULT_HEIGHT: i32 = super::springboard::DEFAULT_HEIGHT;
/// Record field holding the throw's height (y_max).
pub const HEIGHT_FIELD: usize = super::springboard::HEIGHT_FIELD;
/// Depth given to a blower placed at depth 0.
pub const DEFAULT_Z: i32 = 1;
/// What a blower is to other objects: a kind nothing reacts to.
pub const TYPE: u32 = 0x1;
/// What may hit a blower; it has no hit box, so nothing does.
pub const HIT_MASK: u32 = 0x300_0072;
/// Ticks of one loop of the jet at the 18 ms step.
pub const LOOP_TICKS: u32 = 57;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Blowing, loop after loop.
    pub const IDLE: i32 = 1;
}

/// The throw's height for a record whose y_max field is `y_max`: the
/// field when above 0, else [`DEFAULT_HEIGHT`].
pub fn height_of(y_max: i32) -> i32 {
    super::springboard::height_of(y_max)
}

/// The nozzle box of a blower whose record sets no movement box, from the
/// box of the picture it shows when placed: 5 px wider on the left, 25 px
/// on the right and 10 px taller at the top and at the bottom.
pub fn nozzle_box(image_box: Rect) -> Rect {
    Rect::new(
        image_box.left - 5,
        image_box.top - 10,
        image_box.right + 25,
        image_box.bottom + 10,
    )
}

/// True when two boxes, both with their right and bottom edges excluded,
/// share at least one pixel.
pub fn boxes_meet(a: Rect, b: Rect) -> bool {
    a.is_set()
        && b.is_set()
        && a.left.max(b.left) < a.right.min(b.right)
        && a.top.max(b.top) < a.bottom.min(b.bottom)
}

/// True when the box `r` has no area (or is unset).
fn empty(r: Rect) -> bool {
    !r.is_set() || r.right <= r.left || r.bottom <= r.top
}

/// A blower's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroundBlowerData {
    /// Height of Claw's throw, px.
    pub height: i32,
    /// The nozzle box, relative to the blower, once set up.
    pub nozzle: Option<Rect>,
    /// First and last frame numbers of the blower's image set.
    pub frames: Option<(i32, i32)>,
    /// The jet being played.
    pub anim: SpriteAnim,
}

/// The blower data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&GroundBlowerData> {
    match &o.data {
        ObjectData::GroundBlower(d) => Some(d),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::GroundBlower(d) => d,
        _ => GroundBlowerData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::GroundBlower(d);
    }
}

fn set_up(w: &mut World, id: ObjectId, d: &mut GroundBlowerData) {
    let level = w.level;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    d.frames = Some(frames_of(level, o, d.frames));
    if empty(o.move_rect)
        && let Some(b) = level.frame_box(&o.image_set, o.frame)
    {
        o.move_rect = nozzle_box(b);
    }
    d.nozzle = (!empty(o.move_rect)).then_some(o.move_rect);
    o.type_flags = TYPE;
    o.hit_type_flags = HIT_MASK;
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    d.height = height_of(o.record.as_ref().map_or(0, |r| r[HEIGHT_FIELD]));
    o.state = state::IDLE;
    d.anim.set(level.animations.get(ANIM).cloned());
    advance_sprite(w, id, &mut d.anim, d.frames.unwrap_or_default());
}

/// True when Claw's movement box, where his last update left him, meets
/// the nozzle of blower `id`.
fn claw_over(w: &World, id: ObjectId, d: &GroundBlowerData) -> bool {
    let (Some(o), Some(claw), Some(nozzle)) = (w.objects.get(id), w.claw_object(), d.nozzle) else {
        return false;
    };
    boxes_meet(
        claw.move_rect.translate(claw.x, claw.y),
        nozzle.translate(o.x, o.y),
    )
}

fn step(w: &mut World, id: ObjectId, d: &mut GroundBlowerData) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => set_up(w, id, d),
        state::IDLE => {
            advance_sprite(w, id, &mut d.anim, d.frames.unwrap_or_default());
            let on_nozzle = w.objects.get(id).is_some_and(|o| o.frame == THROW_FRAME);
            if on_nozzle && claw_over(w, id, d) {
                player::bounce(w, d.height);
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a blower.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A blower attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nozzle_box_grows_from_the_first_picture() {
        // The first picture of the sixth level's jet, 8 x 5 px, 78 px
        // below the blower's position.
        assert_eq!(
            nozzle_box(Rect::new(-11, 76, -4, 79)),
            Rect::new(-16, 66, 21, 89)
        );
    }

    #[test]
    fn boxes_meet_without_their_right_and_bottom_edges() {
        let a = Rect::new(0, 0, 10, 10);
        assert!(boxes_meet(a, Rect::new(9, 9, 20, 20)));
        assert!(!boxes_meet(a, Rect::new(10, 0, 20, 10)), "right edge");
        assert!(!boxes_meet(a, Rect::new(0, 10, 10, 20)), "bottom edge");
        assert!(!boxes_meet(a, Rect::new(-10, 0, 0, 10)), "left neighbour");
        assert!(!boxes_meet(a, Rect::UNSET));
        assert!(!boxes_meet(Rect::UNSET, a));
        assert!(!boxes_meet(a, Rect::new(5, 5, 5, 8)), "no area");
    }

    #[test]
    fn the_throw_is_450_px_unless_the_record_says_otherwise() {
        assert_eq!(height_of(0), 450);
        assert_eq!(height_of(-1), 450);
        assert_eq!(height_of(400), 400);
        assert_eq!(HEIGHT_FIELD, 54);
    }

    #[test]
    fn a_loop_is_seventeen_short_pictures_and_a_long_one() {
        // 17 pictures of 25 ms (2 ticks each) and one of 400 ms.
        assert_eq!(LOOP_TICKS, 17 * 25u32.div_ceil(18) + 400u32.div_ceil(18));
    }
}
