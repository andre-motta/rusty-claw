//! Conveyor belts: strips of moving track laid on the ninth level's
//! floors, each belt a left end, a row of 64 px middle pieces and a right
//! end, every piece its own object.
//!
//! A belt adds no floor and takes part in no collision: the floor is the
//! level's own ground row. It adds a push. While Claw's box overlaps a
//! piece's box, the piece writes its speed (the record's speed field,
//! clamped to [`SPEED_LIMIT`]) into Claw's conveyor force, which his own
//! movement adds to his speed when he stands, ducks, looks up, walks or
//! rises in a jump; the fall ignores it. Nothing else is ever moved.
//!
//! - A piece pushes from the second tick after Claw's update first ends in
//!   its box: the first tick it only notices him.
//! - While Claw rises in a jump the pieces leave the push as it was, so a
//!   jump from a belt keeps the belt's speed until its top.
//! - Each piece writes the force on its own, in update order, and the last
//!   write wins; a piece that loses Claw writes 0, so crossing from one
//!   piece into one that updates earlier leaves him one tick without push.
//! - A warp clears the push.
//!
//! The piece's box is the box of its picture, except that the right end
//! keeps only the left half of its picture ([`RIGHT_END`]); Claw's box for
//! the test runs from his position down to his feet. The pieces cycle
//! their pictures at a pace set by their speed ([`cycle_anim`]). A belt
//! plays no sound (sound objects beside the belts hum) and draws no random
//! number.
//!
//! | State | Per call |
//! | --- | --- |
//! | Idle | advance the cycle; when Claw is on the piece, go to Move (no push yet) |
//! | Move | advance the cycle; while Claw rises in a jump, nothing more; when he is on the piece, write its speed into his conveyor force; otherwise write 0 and go back to Idle, restarting the cycle |
//!
//! Claw is on a piece when his box, from his position down to his feet
//! and as wide as his movement box, shares at least one pixel with the
//! piece's box placed at the piece (right and bottom edges excluded), and
//! he is not rising in a jump.
//!
//! # State numbers
//!
//! A piece is no enemy: its states are its own ([`state`]).
//!
//! # Defaults taken
//!
//! - The ninth level's 479 pieces need no special handling for speed: at
//!   most 33 are live at once, each doing one animation step and one box
//!   test per tick. The tick cost and the cost of copying the game along
//!   the level's bot route are measured when the slice closes and
//!   compared with the third and eighth levels'.
//! - Asleep in the side areas of a wide display, a piece never woken is
//!   drawn as its set-up leaves it, its cycle started ([`first_look`]).
//!
//! Its data ([`BeltData`]) is its own.

use super::{ObjectData, World};
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags};
use crate::physics::ptype;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["ConveyorBelt"];

/// Image sets this family may switch an object to: none (the pieces keep
/// the sets their records name).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: the picture cycles,
/// forward and backward.
pub const ANIM_PREFIXES: &[&str] = &["GAME_CYCLE", "GAME_REVERSECYCLE"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The right end's picture, whose box keeps only its left half.
pub const RIGHT_END: &str = "LEVEL_CONVEYRIGHT";
/// The pieces' pictures on the ninth level: left end, middle, right end.
pub const PIECES: [&str; 3] = ["LEVEL_CONVEYLEFT", "LEVEL_CONVEYMIDDLE", RIGHT_END];
/// The fastest push, px/s, either way; record speeds beyond it are
/// clamped.
pub const SPEED_LIMIT: i32 = 300;
/// Record field holding the speed, px/s.
pub const SPEED_FIELD: usize = 60;
/// Depth of a piece placed at depth 0.
pub const DEFAULT_Z: i32 = 1;
/// What a piece is, for collisions.
pub const TYPE: u32 = 1;
/// What may hit a piece (nothing reaches it: it has no hit box).
pub const HIT_MASK: u32 = 0x300_0072;
/// Ticks from Claw's update first ending in a piece's box to the first
/// tick the piece pushes him.
pub const PUSH_DELAY_TICKS: u32 = 2;

/// A piece's states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Claw is not on it.
    pub const IDLE: i32 = 1;
    /// Claw is on it: it pushes him.
    pub const MOVE: i32 = 2;
}

/// The record speed `speed` clamped to the belts' limit.
pub fn clamp_speed(speed: i32) -> i32 {
    speed.clamp(-SPEED_LIMIT, SPEED_LIMIT)
}

/// The picture cycle of a piece of clamped speed `speed`: backward cycles
/// for a speed below 0, forward ones from 0 up, faster the faster the
/// belt.
pub fn cycle_anim(speed: i32) -> &'static str {
    let s = speed.unsigned_abs();
    match (speed < 0, s) {
        (true, 300..) => "GAME_CYCLE50",
        (true, 150..) => "GAME_CYCLE100",
        (true, 75..) => "GAME_CYCLE200",
        (true, _) => "GAME_CYCLE500",
        (false, 300..) => "GAME_REVERSECYCLE50",
        (false, 150..) => "GAME_REVERSECYCLE100",
        (false, 75..) => "GAME_REVERSECYCLE200",
        (false, _) => "GAME_REVERSECYCLE500",
    }
}

/// A piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BeltData {
    /// The clamped speed, px/s.
    pub speed: i32,
}

/// The push box of a piece showing a picture with box `picture` from the
/// set `image_set`: the picture's box, its right half dropped for the
/// right end ([`RIGHT_END`], compared without regard to case).
pub fn push_box(image_set: &str, picture: Rect) -> Rect {
    let mut b = picture;
    if image_set.eq_ignore_ascii_case(RIGHT_END) {
        b.right = b.left + (b.right - b.left) / 2;
    }
    b
}

/// True when the boxes `a` and `b` share at least one pixel, their right
/// and bottom edges excluded.
pub fn intersect(a: Rect, b: Rect) -> bool {
    a.left.max(b.left) < a.right.min(b.right) && a.top.max(b.top) < a.bottom.min(b.bottom)
}

/// The box a piece tests Claw `claw` with: from his position down to his
/// feet, as wide as his movement box; `None` while he rises in a jump,
/// when no piece holds him.
pub fn claw_box(claw: &Object) -> Option<Rect> {
    if claw.physics_type == ptype::JUMP || !claw.move_rect.is_set() {
        return None;
    }
    let m = claw.move_rect;
    Some(Rect::new(
        claw.x + m.left,
        claw.y,
        claw.x + m.right,
        claw.y + m.bottom,
    ))
}

/// True when Claw, testing with `claw` ([`claw_box`]), is on the piece
/// `o`: his box meets the piece's push box placed at the piece.
pub fn holds(o: &Object, claw: Option<Rect>) -> bool {
    match claw {
        Some(c) if o.move_rect.is_set() => intersect(c, o.move_rect.translate(o.x, o.y)),
        _ => false,
    }
}

/// Starts the cycle of a piece of clamped speed `speed` and advances it
/// once by `dt` ms, as each entry into Idle does.
fn start_cycle(level: &SimLevel, o: &mut Object, speed: i32, dt: u32) {
    match level.animations.get(cycle_anim(speed)) {
        Some(a) => o.anim.set(a.clone()),
        None => o.anim = Default::default(),
    }
    advance(level, o, dt);
}

/// Advances the piece's cycle by `dt` ms (the cycles play no sound).
fn advance(level: &SimLevel, o: &mut Object, dt: u32) {
    let mirrored = o.draw_flags & draw_flags::MIRROR != 0;
    let frames = level.image_frames(&o.image_set);
    let e = o.anim.advance_in(dt, o.frame, mirrored, frames);
    if let Some(f) = e.frame {
        o.frame = f;
    }
}

/// The set-up of piece `o` on its first call, the cycle advanced by `dt`
/// ms (see the module documentation).
fn set_up(level: &SimLevel, o: &mut Object, dt: u32) {
    let speed = clamp_speed(o.record.as_ref().map_or(0, |r| r[SPEED_FIELD]));
    if let Some(b) = level.frame_box(&o.image_set, o.frame) {
        o.move_rect = push_box(&o.image_set, b);
    }
    o.type_flags = TYPE;
    o.hit_type_flags = HIT_MASK;
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    o.data = ObjectData::ConveyorBelt(BeltData { speed });
    o.state = state::IDLE;
    start_cycle(level, o, speed, dt);
}

/// How a piece whose first call has not run yet looks once it has (see
/// [`super::first_look`]): its cycle started. `None` once it has run.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    if o.state != engine_state::INIT {
        return None;
    }
    let mut p = o.clone();
    set_up(level, &mut p, TICK_MS);
    Some(vec![Look::of(&p)])
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let level = w.level;
    let dt = w.dt_ms;
    let claw = w
        .claw_object()
        .map(|c| (claw_box(c), c.physics_type == ptype::JUMP));
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state == engine_state::INIT {
        set_up(level, o, dt);
        return;
    }
    let speed = match &o.data {
        ObjectData::ConveyorBelt(d) => d.speed,
        _ => return,
    };
    let state = o.state;
    advance(level, o, dt);
    let Some((claw, jumping)) = claw else {
        return;
    };
    let on = holds(o, claw);
    match state {
        state::IDLE if on => o.state = state::MOVE,
        state::MOVE if !jumping => {
            if on {
                w.claw.conveyor_force = f64::from(speed);
            } else {
                o.state = state::IDLE;
                start_cycle(level, o, speed, dt);
                w.claw.conveyor_force = 0.0;
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a piece.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A piece attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_end_pushes_with_its_left_half() {
        let picture = Rect::new(-31, -30, 32, 21);
        assert_eq!(push_box(RIGHT_END, picture), Rect::new(-31, -30, 0, 21));
        assert_eq!(
            push_box("level_conveyright", picture),
            Rect::new(-31, -30, 0, 21)
        );
        let middle = Rect::new(-32, -22, 31, -3);
        assert_eq!(push_box(PIECES[1], middle), middle);
    }

    #[test]
    fn boxes_meet_without_their_right_and_bottom_edges() {
        let a = Rect::new(0, 0, 10, 10);
        assert!(intersect(a, Rect::new(9, 9, 20, 20)));
        assert!(!intersect(a, Rect::new(10, 0, 20, 10)));
        assert!(!intersect(a, Rect::new(0, 10, 10, 20)));
        assert!(!intersect(a, Rect::new(5, 5, 5, 8)));
    }

    #[test]
    fn speeds_are_clamped_both_ways() {
        assert_eq!(clamp_speed(500), 300);
        assert_eq!(clamp_speed(-500), -300);
        assert_eq!(clamp_speed(175), 175);
        assert_eq!(clamp_speed(-150), -150);
    }

    #[test]
    fn the_cycle_follows_the_speed() {
        // The ninth level's speeds, clamped.
        for (speed, anim) in [
            (-300, "GAME_CYCLE50"),
            (-250, "GAME_CYCLE100"),
            (-200, "GAME_CYCLE100"),
            (-150, "GAME_CYCLE100"),
            (-100, "GAME_CYCLE200"),
            (75, "GAME_REVERSECYCLE200"),
            (100, "GAME_REVERSECYCLE200"),
            (150, "GAME_REVERSECYCLE100"),
            (175, "GAME_REVERSECYCLE100"),
            (300, "GAME_REVERSECYCLE50"),
        ] {
            assert_eq!(cycle_anim(speed), anim, "{speed}");
        }
        assert_eq!(cycle_anim(0), "GAME_REVERSECYCLE500");
        assert_eq!(cycle_anim(-74), "GAME_CYCLE500");
        for anim in [cycle_anim(-300), cycle_anim(300)] {
            assert!(ANIM_PREFIXES.iter().any(|p| anim.starts_with(p)));
        }
    }
}
