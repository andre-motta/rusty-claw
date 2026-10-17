//! Swinging ropes: ropes that hang from the ceiling and swing back and
//! forth on their own. Claw grabs one by jumping or falling into its lower
//! end, hangs from it in his Swing state
//! ([`crate::player::state::SWING`], see [`crate::player::swing`]) while it
//! swings, and lets go with jump.
//!
//! The swing is an animation, not physics: the rope shows one of 120
//! frames (60 from left to right, 60 back), and each of the 60 positions
//! has a known place for the lower end ([`END`]), where a small grab box
//! sits. Which frame shows is a function of the level clock and the
//! rope's own phase: the rope keeps a frame position that runs at a
//! constant number of frames per second.
//!
//! Set-up, on the first call (and nothing else that call): the rope image
//! set; depth 3990 when placed at 0; the record's horizontal speed is the
//! time of one half swing in ms (1500 when 0), so the rope runs at
//! `q = 60000 / half swing` frames per second (whole frames); the record's
//! speed field is a phase in ms, which starts the frame position at
//! `speed * q / 1000` frames (whole frames). It is a special pickup with
//! the rope code ([`crate::player::hit::special_code::ROPE`]): Claw's hit
//! handler sees it whenever his body touches its grab box.
//!
//! Each later call, in this order:
//!
//! 1. the frame is `(whole frame position mod 120) + 1`;
//! 2. the grab box and the end come from the table entry of that frame
//!    (mirrored on the way back, frames 61 to 120): the grab box reaches
//!    13 px either side of the end, 8 px above it and 25 px below it;
//! 3. when Claw hangs from this rope in his Swing state, he is turned to
//!    face the way the end travels: right for frames 1 to 60, left for 61
//!    to 120 (a turn resets his running time, as every turn does);
//! 4. the rope shows the frame;
//! 5. the frame position moves on to this tick's clock:
//!    `dt = clock / 1000 - time; time += dt; position += q * dt`, in
//!    double precision, in that order.
//!
//! So the frame, the box and the end of a tick come from the position
//! reached in the previous live tick: on the first tick after the rope is
//! set up, and on the first tick after each wake, it shows the frame it
//! had before (one stale tick), and the grab box and the end follow that
//! same frame for the tick. No sound and no random draw.

use super::platform::level_clock;
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, flags, types};
use crate::player::{self, hit};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AniRope"];

/// Image set every rope shows.
pub const IMAGE_SET: &str = "LEVEL_ROPE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Depth given to a rope placed at depth 0: just behind Claw, so the rope
/// runs before him in every tick.
pub const DEFAULT_Z: i32 = 3990;
/// Length of a half swing when the rope is placed without one, ms.
pub const DEFAULT_HALF_SWING_MS: i32 = 1500;
/// Frames in a whole swing, there and back.
pub const FRAMES: i32 = 120;
/// Frames in a half swing.
pub const HALF: i32 = 60;

/// Record field holding the rope's phase, ms.
const SPEED_FIELD: usize = 60;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Swinging.
    pub const IDLE: i32 = 1;
}

/// Where the lower end of the rope is for each of the 60 positions of the
/// left-to-right half swing, relative to the rope's position: from the
/// left extreme, through the bottom of the swing, to the right extreme.
/// The way back mirrors it.
#[rustfmt::skip]
pub const END: [(i32, i32); 60] = [
    (-140, 27), (-140, 28), (-139, 29), (-139, 31), (-138, 34), (-137, 36),
    (-135, 38), (-134, 41), (-132, 44), (-130, 47), (-127, 50), (-123, 54),
    (-117, 59), (-110, 62), (-104, 66), (-96, 69), (-89, 73), (-80, 75),
    (-61, 78), (-61, 80), (-52, 82), (-43, 84), (-32, 85), (-23, 86),
    (-12, 87), (-2, 87), (8, 87), (18, 87), (28, 87), (38, 85),
    (49, 84), (57, 83), (66, 81), (75, 79), (85, 76), (95, 73),
    (102, 69), (110, 65), (117, 61), (125, 56), (131, 50), (134, 46),
    (136, 43), (140, 40), (142, 34), (144, 25), (144, 25), (144, 26),
    (144, 26), (143, 26), (143, 26), (143, 26), (142, 26), (142, 26),
    (142, 26), (141, 26), (141, 26), (140, 26), (140, 26), (139, 26),
];

/// The grab box around the end: left, top, right and bottom from it.
pub const GRAB_BOX: Rect = Rect::new(-13, -8, 13, 25);

/// A rope's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RopeData {
    /// Frames per second.
    pub q: i32,
    /// Frame position.
    pub position: f64,
    /// Clock time the position was last moved to, s.
    pub time: f64,
    /// The frame shown, 1 to 120.
    pub frame: i32,
    /// The lower end, relative to the rope's position.
    pub end: (i32, i32),
}

/// The frame shown at frame position `position`: 1 to 120.
pub fn frame_at(position: f64) -> i32 {
    ((position.trunc() as i64) % i64::from(FRAMES)) as i32 + 1
}

/// The end and the grab box of `frame`, relative to the rope's position,
/// and whether the end travels right (frames 1 to 60).
pub fn end_of_frame(frame: i32) -> ((i32, i32), Rect, bool) {
    let k = (frame - 1).rem_euclid(HALF) as usize;
    let (ex, ey) = END[k];
    let right = (1..=HALF).contains(&frame);
    let ex = if right { ex } else { -ex };
    ((ex, ey), GRAB_BOX.translate(ex, ey), right)
}

/// The frames per second of a rope whose half swing lasts `half_ms`, and
/// its starting frame position for the phase `phase_ms`.
pub fn motion(half_ms: i32, phase_ms: i32) -> (i32, f64) {
    let q = 60_000i32.checked_div(half_ms).unwrap_or(0);
    let start = phase_ms.wrapping_mul(q) / 1000;
    (q, f64::from(start))
}

/// The lower end of the rope `o` in level pixels, when `o` is a rope.
pub fn end_of(o: &Object) -> Option<(i32, i32)> {
    match &o.data {
        ObjectData::Rope(d) => Some((o.x + d.end.0, o.y + d.end.1)),
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
        ObjectData::Rope(d) => d,
        _ => RopeData::default(),
    };
    match o.state {
        state::SET_UP => set_up(w.level, o, &mut d),
        state::IDLE => idle(w, id, &mut d),
        _ => {}
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Rope(d);
    }
}

fn set_up(level: &crate::level::SimLevel, o: &mut Object, d: &mut RopeData) {
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    if let Some((first, _)) = level.image_frames(IMAGE_SET) {
        o.frame = first;
    }
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    if o.speed_x == 0 {
        o.speed_x = DEFAULT_HALF_SWING_MS;
    }
    o.type_flags = types::SPECIAL;
    let r = o
        .record
        .get_or_insert_with(|| Box::new([0; claw_assets::wwd::OBJECT_INTS]));
    r[hit::POINTS_FIELD] = hit::special_code::ROPE;
    let phase = r[SPEED_FIELD];
    let (q, start) = motion(o.speed_x, phase);
    d.q = q;
    d.position = start;
    d.time = 0.0;
    o.state = state::IDLE;
}

fn idle(w: &mut World, id: ObjectId, d: &mut RopeData) {
    let frame = frame_at(d.position);
    let (end, grab, right) = end_of_frame(frame);
    d.frame = frame;
    d.end = end;
    if let Some(o) = w.objects.get_mut(id) {
        o.attack_rect = grab;
    }
    let swinging_here = w.claw.rope == Some(id)
        && w.claw_object()
            .is_some_and(|c| c.state == player::state::SWING);
    if swinging_here {
        player::swing::face(w, right);
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.frame = frame;
    }
    let now = f64::from(level_clock(w)) * 0.001;
    let dt = now - d.time;
    d.time += dt;
    d.position += f64::from(d.q) * dt;
}

/// `attacker` hit `id`. Nothing hits a rope.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A rope reaches Claw as a pickup, through his
/// own hit handler.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_half_swing_sets_the_speed_and_the_phase_the_start() {
        assert_eq!(motion(1500, 0), (40, 0.0));
        assert_eq!(motion(2000, 0), (30, 0.0));
        // 60000 / 1750 truncates to 34 frames per second.
        assert_eq!(motion(1750, 875), (34, 29.0));
        assert_eq!(motion(2000, 1500), (30, 45.0));
        assert_eq!(motion(0, 100), (0, 0.0));
    }

    #[test]
    fn frames_run_from_1_to_120() {
        assert_eq!(frame_at(0.0), 1);
        assert_eq!(frame_at(59.999), 60);
        assert_eq!(frame_at(60.0), 61);
        assert_eq!(frame_at(119.5), 120);
        assert_eq!(frame_at(120.0), 1);
        assert_eq!(frame_at(1234.9), 35);
    }

    #[test]
    fn the_way_back_mirrors_the_end() {
        let ((x, y), grab, right) = end_of_frame(1);
        assert_eq!(((x, y), right), ((-140, 27), true));
        assert_eq!(grab, Rect::new(-153, 19, -127, 52));
        let ((x, y), grab, right) = end_of_frame(61);
        assert_eq!(((x, y), right), ((140, 27), false));
        assert_eq!(grab, Rect::new(127, 19, 153, 52));
        // The bottom of the swing, 87 px below the rope.
        assert_eq!(end_of_frame(27).0, (8, 87));
        assert_eq!(end_of_frame(87).0, (-8, 87));
        // The right extreme and the dwell there.
        assert_eq!(end_of_frame(46).0, (144, 25));
        assert_eq!(end_of_frame(60).0, (139, 26));
        assert_eq!(end_of_frame(120).0, (-139, 26));
    }

    #[test]
    fn the_end_moves_at_most_19_px_per_frame() {
        let steps = END.windows(2).map(|p| (p[1].0 - p[0].0).abs());
        assert_eq!(steps.max(), Some(19));
        // One position repeats across: the end stands still for a frame.
        assert_eq!(END[18].0, END[19].0);
    }
}
