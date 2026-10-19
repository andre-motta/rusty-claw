//! Shakes: the invisible earthquake triggers of the twelfth level's caves.
//!
//! A Shake is a box ([`ATTACK_FIELDS`], in level coordinates) that waits
//! for Claw. On the first collision pass in which Claw's box touches it,
//! the ground rumbles:
//!
//! - the screen shakes for [`SHAKE_MS`] of game time
//!   ([`World::shake`]), so the camera's jolts draw from the shared random
//!   stream for as long;
//! - it draws one number of 0..6 from the shared random stream, before
//!   the jolts: 1 to 5 pick one of the five rumbles ([`RUMBLES`]) as its
//!   sound, 0 keeps the one it played last (none for the first), and
//!   that sound plays at once;
//! - it takes no part in any collision for [`REARM_MS`] of game time,
//!   then waits for Claw again: a Claw who stays inside gets a five-second
//!   shake every seven seconds.
//!
//! A Shake only acts while it is live, in the small objects' window
//! around the centre of the view (311 px above and below it), so on its
//! tall boxes only the part near its own position can fire: falling down
//! the tall shaft at 25540, Claw sets off the Shake at its foot on the
//! tick that starts with the view's centre 300 px above it, not on the one
//! that starts 316 px above it; and the first Shake, 305 px above a Claw
//! standing on the ledge under its box, still wakes there to re-arm.
//! Nothing hits a Shake; it is never drawn.
//!
//! # Timing
//!
//! With T the tick whose collision pass fires it, the camera jolts on 183
//! of the ticks T + 1 to T + 277 at the 18 ms step (188 of the frames
//! T + 1 to T + 299 at 60 frames per second), and it fires again at the
//! earliest on T + 389 (T + 420 at 60 frames per second), all measured.
//! Pausing changes none of it: the shake and the wait run on game time.

use claw_frame::SoundChannel;

use super::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Shake"];

/// Image sets this family may switch an object to: none, it is never
/// drawn.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// What it is: a kind nothing attacks.
pub const TYPE: u32 = 0x1;
/// What it attacks: Claw.
pub const ATTACK_MASK: u32 = types::PLAYER;
/// Record fields of its box: left, top, right, bottom, in level
/// coordinates.
pub const ATTACK_FIELDS: [usize; 4] = [27, 28, 29, 30];
/// The screen shake it starts, ms of game time.
pub const SHAKE_MS: u32 = 5000;
/// Its wait from one rumble to the next, ms of game time.
pub const REARM_MS: u32 = 7000;
/// The rumbles: a draw of 1 to 5 picks one.
pub const RUMBLES: [&str; 5] = [
    "LEVEL_EARTHQUAKE1",
    "LEVEL_EARTHQUAKE2",
    "LEVEL_EARTHQUAKE3",
    "LEVEL_EARTHQUAKE4",
    "LEVEL_EARTHQUAKE5",
];
/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting for Claw's first touch.
    pub const ARMED: i32 = 1;
    /// Fired: out of every collision until its wait has run out, then
    /// waiting for Claw again.
    pub const WAIT: i32 = 2;
}

/// A Shake's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShakeData {
    /// Game time it last fired, ms.
    pub start_ms: u64,
    /// The rumble it plays (an index into [`RUMBLES`]), once one is drawn.
    pub rumble: Option<usize>,
}

/// The Shake data of `o`, when it is one.
pub fn data_of(o: &crate::object::Object) -> Option<&ShakeData> {
    match &o.data {
        ObjectData::Shake(d) => Some(d),
        _ => None,
    }
}

/// The rumble a draw `r` of 0..6 leaves in the sound slot holding
/// `previous`: 1 to 5 pick one, 0 keeps the previous one.
pub fn rumble_for(r: u32, previous: Option<usize>) -> Option<usize> {
    match r {
        1..=5 => Some(r as usize - 1),
        _ => previous,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    match o.state {
        state::SET_UP => {
            o.flags |= flags::SAFE;
            o.type_flags = TYPE;
            o.attack_type_flags = ATTACK_MASK;
            o.attack_handler = true;
            let field = |i: usize| o.record.as_ref().map_or(0, |r| r[i]);
            let [l, t, r, b] = ATTACK_FIELDS.map(field);
            o.attack_rect = Rect::new(l - o.x, t - o.y, r - o.x, b - o.y);
            o.state = state::ARMED;
            if !matches!(o.data, ObjectData::Shake(_)) {
                o.data = ObjectData::Shake(ShakeData::default());
            }
        }
        state::WAIT => {
            let start = data_of(o).map_or(0, |d| d.start_ms);
            if w.now_ms.wrapping_sub(start) as u32 >= REARM_MS {
                o.flags &= !flags::NO_COLLIDE;
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a Shake.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` touched `victim`: Claw sets it off (see the module documentation).
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if w.claw_id != Some(victim) {
        return;
    }
    let now = w.now_ms;
    let previous = match w.objects.get_mut(id) {
        Some(o) => {
            o.flags |= flags::NO_COLLIDE;
            o.state = state::WAIT;
            data_of(o).and_then(|d| d.rumble)
        }
        None => return,
    };
    w.shake(SHAKE_MS);
    let rumble = rumble_for(w.rng.below(6), previous);
    if let Some(ObjectData::Shake(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
        d.start_ms = now;
        d.rumble = rumble;
    }
    if let Some(i) = rumble {
        sound::play(w, RUMBLES[i], SoundChannel::Effect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draw_of_zero_keeps_the_last_rumble() {
        assert_eq!(rumble_for(0, None), None);
        assert_eq!(rumble_for(0, Some(3)), Some(3));
        for r in 1..=5 {
            assert_eq!(rumble_for(r, Some(0)), Some(r as usize - 1));
        }
    }

    #[test]
    fn its_waits_in_ticks() {
        // Re-armed on the 389th tick after it fired at 18 ms.
        assert_eq!(REARM_MS.div_ceil(18), 389);
        // The shake's 5000 ms end on its 278th tick.
        assert_eq!(SHAKE_MS.div_ceil(18), 278);
    }
}
