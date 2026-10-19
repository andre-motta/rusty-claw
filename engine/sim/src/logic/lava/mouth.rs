//! The lava mouths: stone mouths in the ceiling that drip lava.
//!
//! A mouth waits its record's start delay (the speed field, ms) from the
//! moment it is built, showing its first picture with no box; then it
//! drips without end: its 22 pictures in turn, three ticks each
//! ([`DRIP_TICKS`] a drip), the 22nd followed by the first. The drop's box
//! is the row of [`DROP`] for the picture shown, set on every tick: a
//! drop swelling under the mouth, then falling to the floor and
//! splashing; the first and the last three pictures have no box, and
//! keep the box they find. Touching the drop costs Claw
//! [`super::DAMAGE`]. Its two sounds play on the drop's first picture and
//! on the splash, at most once in [`SOUND_GAP_MS`].
//!
//! When Claw has lost a life since it last looked, the mouth shows its
//! first picture again and waits its start delay from that moment, but
//! keeps its last drop's box live through the delay (invisible): a mouth
//! caught mid-drip keeps that drop's box until it drips again.
//!
//! It never moves, nothing hits it, and it draws no random number.
//!
//! # Defaults taken
//!
//! - A drop's box held through the delay after a lost life hurts Claw as
//!   a running one does: the collision pass reads only the box.

use claw_frame::SoundChannel;

use super::{DAMAGE, GENERIC, SPEED_FIELD, field, row};
use crate::logic::enemy::Timer;
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, engine_state, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["LavaMouth"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_LAVAMOUTH";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The drip: every picture of the set in turn, 50 ms each.
pub const ANIM: &str = "GAME_CYCLE50";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Ticks of one drip at the 18 ms step.
pub const DRIP_TICKS: u64 = 66;

/// The sound of the drop forming, on its first picture.
pub const DROP_SOUND: &str = "LEVEL_LAVAMOUTH2";
/// The sound of the splash.
pub const SPLASH_SOUND: &str = "LEVEL_LAVAMOUTH1";
/// The picture the drop forms on.
pub const DROP_FRAME: i32 = 2;
/// The picture of the splash.
pub const SPLASH_FRAME: i32 = 19;
/// The shortest time between two of its sounds, ms.
pub const SOUND_GAP_MS: u64 = 250;

/// A mouth's states, in its object's state.
pub mod state {
    /// Waiting its start delay.
    pub const IDLE: i32 = 1;
    /// Dripping.
    pub const FALLING: i32 = 2;
}

const fn r(l: i32, t: i32, rt: i32, b: i32) -> Option<Rect> {
    Some(Rect::new(l, t, rt, b))
}

/// The drop's box by picture, 1 to 22, relative to the mouth's origin
/// (the mouth hangs 133 px above it, the floor is 122 px below it).
pub const DROP: [Option<Rect>; 22] = [
    None,
    r(-14, -136, 13, -128),
    r(-14, -136, 13, -121),
    r(-14, -136, 14, -114),
    r(-14, -136, 13, -106),
    r(-14, -136, 13, -102),
    r(-14, -136, 13, -97),
    r(-14, -136, 13, -91),
    r(-14, -136, 13, -84),
    r(-14, -136, 13, -76),
    r(-14, -136, 13, -70),
    r(-14, -136, 13, -65),
    r(-14, -129, 13, -57),
    r(-14, -123, 14, -54),
    r(-14, -104, 13, -34),
    r(-14, -51, 13, 17),
    r(-14, 4, 13, 70),
    r(-14, 54, 13, 121),
    r(-14, 105, 13, 122),
    None,
    None,
    None,
];

/// A mouth's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MouthData {
    /// The start delay running.
    pub wait: Timer,
    /// Claw's lost lives when it last started waiting.
    pub attempt: u32,
    /// The gap between its sounds.
    pub sound_gap: Timer,
}

/// The drop's box for picture `frame`; none for a picture without one.
pub fn drop_box(frame: i32) -> Option<Rect> {
    row(&DROP, frame)
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut MouthData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::LavaMouth(d) => Some(d),
        _ => None,
    }
}

/// The data of mouth `id`, if it is one.
pub fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a MouthData> {
    match &w.objects.get(id)?.data {
        ObjectData::LavaMouth(d) => Some(d),
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
        state::FALLING => falling_tick(w, id),
        _ => {}
    }
}

/// Builds a mouth: an enemy-kind object that hurts on contact, waiting
/// its start delay.
fn build(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = GENERIC;
    o.damage = DAMAGE;
    o.data = ObjectData::LavaMouth(MouthData::default());
    start_waiting(w, id);
}

/// Shows the first picture and waits the start delay from now,
/// remembering Claw's lost lives; the box stays as it is.
fn start_waiting(w: &mut World, id: ObjectId) {
    let wait = u64::try_from(field(w, id, SPEED_FIELD)).unwrap_or(0);
    let (now, attempt) = (w.now_ms, w.attempt());
    let first = w.image_frames(IMAGE_SET).map_or(1, |(f, _)| f);
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::IDLE;
        o.frame = first;
    }
    if let Some(d) = data_mut(w, id) {
        d.wait = Timer::new(now, wait);
        d.attempt = attempt;
    }
}

/// Idle: once the delay has run out, the drip starts (from the next
/// tick's call on).
fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if data(w, id).is_some_and(|d| d.wait.done(now)) {
        w.advance_anim(id);
        if let Some(o) = w.objects.get_mut(id) {
            o.state = state::FALLING;
        }
        w.set_anim(id, ANIM);
    }
}

/// Dripping: the animation, the drop's box for the picture shown, the
/// sounds, and after a lost life the delay again.
fn falling_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let frame = w.objects.get(id).map_or(0, |o| o.frame);
    if let Some(b) = drop_box(frame)
        && let Some(o) = w.objects.get_mut(id)
    {
        o.attack_rect = b;
    }
    let now = w.now_ms;
    let gap_over = data(w, id).is_some_and(|d| d.sound_gap.done(now));
    let sound = match frame {
        DROP_FRAME => Some(DROP_SOUND),
        SPLASH_FRAME => Some(SPLASH_SOUND),
        _ => None,
    };
    if gap_over && let Some(key) = sound {
        sound::play_from(w, id, key, SoundChannel::Effect);
        if let Some(d) = data_mut(w, id) {
            d.sound_gap = Timer::new(now, SOUND_GAP_MS);
        }
    }
    let attempt = w.attempt();
    if data(w, id).is_some_and(|d| d.attempt != attempt) {
        start_waiting(w, id);
    }
}

/// `attacker` hit `id`: nothing hits a mouth.
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
    fn the_drop_swells_falls_and_splashes() {
        assert_eq!(drop_box(1), None);
        for f in 20..=22 {
            assert_eq!(drop_box(f), None, "{f}");
        }
        assert_eq!(drop_box(23), None);
        // Swelling under the mouth: the bottom drops, the top stays.
        let swell: Vec<i32> = (2..=12).map(|f| drop_box(f).unwrap().bottom).collect();
        assert_eq!(
            swell,
            [-128, -121, -114, -106, -102, -97, -91, -84, -76, -70, -65]
        );
        assert!((2..=12).all(|f| drop_box(f).unwrap().top == -136));
        // The fourth picture's right edge is one px wider.
        assert_eq!(drop_box(4).unwrap().right, 14);
        // The splash reaches the floor 122 px below.
        assert_eq!(drop_box(SPLASH_FRAME), Some(Rect::new(-14, 105, 13, 122)));
        // 22 pictures of 50 ms, three ticks each.
        assert_eq!(50u64.div_ceil(18) * 22, DRIP_TICKS);
        // Harmful on 18 of its 22 pictures.
        assert_eq!((1..=22).filter(|&f| drop_box(f).is_some()).count(), 18);
    }
}
