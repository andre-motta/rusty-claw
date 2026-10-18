//! Floor spikes: small spike rows set into the ground that slide up, stay
//! up for a while and slide back down, forever, on a rhythm fixed by the
//! level clock.
//!
//! The four names are one logic and differ only in their phase:
//! `FloorSpike` starts its cycle at clock 0 (or at the record's speed
//! field, in ms), `FloorSpike2` 750 ms later, `FloorSpike3` 1500 ms later
//! and `FloorSpike4` 2250 ms later, so a row laid out 1, 2, 3, 4 rises as a
//! travelling wave. While a spike is up it hurts what its strike box
//! touches (10 damage, as a hazard's shot: Claw is knocked back and cannot
//! be hurt by it again for 850 ms; enemies, kegs and crates take it as
//! any hazard's shot); while it is down or moving it is harmless.
//!
//! Set-up, on the first call: the strike box is the box of the frame
//! shown then (the retracted spike) with its top raised by 10 px, and it
//! never grows with the art; the spike is a hazard's shot, starts
//! harmless, has depth 2000 when placed at 0, damage 10 when placed
//! without any, and up and down halves of the record's speeds (1500 ms
//! each when 0). It then waits down.
//!
//! The cycle ([`state`]):
//!
//! | State | Per call |
//! | --- | --- |
//! | Down | harmless; when the clock is in the up half, go to RiseStart |
//! | RiseStart | play the rising sound, start the rise animation, go to Rising |
//! | Rising | advance the rise; when it ends, turn dangerous and go to Up |
//! | Up | dangerous; when the clock leaves the up half, go to DropStart |
//! | DropStart | turn harmless, start the drop animation, play the sinking sound, go to Dropping |
//! | Dropping | advance the drop; when it ends, go to Down |
//!
//! The clock is only read while down or up, so a slide always ends before
//! the next edge is seen, and each edge takes effect one call late: the
//! call that sees it only picks the next state. The test is
//! `(clock - phase) mod period <= up half`, unsigned, so before its phase a
//! spike counts from 2^32 (see [`super::platform::toggle_on`]). The rise
//! steps through the spike's own image set, one image every 100 ms, so its
//! length follows the set: with the third level's six images a spike is
//! dangerous from the 32nd call after the clock enters its up half, with
//! the fourth level's five images from the 26th, and in both until the
//! first call after the clock leaves it (with the default halves and six
//! images, 52 or 53 ticks of every 3 s).
//!
//! Each record may set its own halves (speed x for the up half, speed y
//! for the down half) and, for `FloorSpike`, its phase; the strike box
//! comes from whatever set the record names, so the fourth level's
//! `LEVEL_FLOORSPIKES`, whose retracted image is 16 x 7 px, strikes in
//! (-7, -3, 8, 12) where the third level's `LEVEL_FLOORSPIKES2` strikes in
//! (-8, -5, 7, 12).
//!
//! A sound plays only when the spike was on screen in the previous tick:
//! its current frame's box, placed as it is drawn, overlapping the game
//! view of that tick. The sound is a cue and never changes what the spike
//! does.

use claw_frame::SoundChannel;

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::platform::{level_clock, toggle_on};
use super::{ObjectData, World, sound};
use crate::level::SimLevel;
use crate::object::{Object, ObjectId, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["FloorSpike", "FloorSpike2", "FloorSpike3", "FloorSpike4"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[RISE_ANIM, DROP_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Steps forward through the image set, 100 ms per image, and finishes on
/// its last image.
pub const RISE_ANIM: &str = "GAME_FORWARD100";
/// Jumps to the image set's last image and steps back, 100 ms per image,
/// finishing on its first image.
pub const DROP_ANIM: &str = "GAME_BACKWARD100";
/// Played when a spike starts to rise.
pub const RISE_SOUND: &str = "LEVEL_FLOORSPIKEUP";
/// Played when a spike starts to sink.
pub const DROP_SOUND: &str = "LEVEL_FLOORSPIKEDOWN";

/// Depth given to a spike placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Damage of a spike placed without any.
pub const DEFAULT_DAMAGE: i32 = 10;
/// Length of each half of the cycle when placed without one, ms.
pub const DEFAULT_HALF_MS: i32 = 1500;
/// How far the strike box reaches above the retracted spike's box, px.
pub const STRIKE_RAISE: i32 = 10;

/// Record field holding `FloorSpike`'s phase, ms.
const SPEED_FIELD: usize = 60;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// About to rise: the next call plays the sound and starts the rise.
    pub const RISE_START: i32 = 1002;
    /// Sinking.
    pub const DROPPING: i32 = 1003;
    /// About to sink: the next call turns harmless and starts the drop.
    pub const DROP_START: i32 = 1004;
    /// Rising.
    pub const RISING: i32 = 1005;
    /// Up and dangerous.
    pub const UP: i32 = 1006;
    /// Down and harmless.
    pub const DOWN: i32 = 1007;
}

/// A floor spike's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloorSpikeData {
    /// Clock offset of the cycle, ms.
    pub phase: i32,
    /// Length of the up half, ms.
    pub on_ms: i32,
    /// Length of the whole cycle, ms.
    pub period: i32,
    /// First and last frame numbers of the spike's image set.
    pub frames: Option<(i32, i32)>,
    /// The slide being played.
    pub anim: SpriteAnim,
}

/// The cycle offset of the spike running `name`, ms: `FloorSpike` takes
/// `record_speed`, the others a quarter, half and three quarters of the
/// default cycle.
pub fn phase_of(name: &str, record_speed: i32) -> i32 {
    match name {
        "FloorSpike2" => 750,
        "FloorSpike3" => 1500,
        "FloorSpike4" => 2250,
        _ => record_speed,
    }
}

/// True while a spike with this cycle is in its up half at level clock
/// `ms` (see [`super::platform::toggle_on`]).
pub fn up_at(ms: u32, d: &FloorSpikeData) -> bool {
    toggle_on(ms, d.phase, d.on_ms, d.period)
}

/// True while the spike `o` hurts what it touches.
pub fn dangerous(o: &Object) -> bool {
    o.flags & flags::HARMLESS == 0
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::FloorSpike(d) => d,
        _ => FloorSpikeData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::FloorSpike(d);
    }
}

fn set_up(level: &SimLevel, o: &mut Object, d: &mut FloorSpikeData) {
    if let Some(b) = level.frame_box(&o.image_set, o.frame) {
        o.attack_rect = b;
        o.attack_rect.top -= STRIKE_RAISE;
    }
    o.flags |= flags::HARMLESS;
    o.type_flags = types::LEVEL_SHOT;
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    if o.damage == 0 {
        o.damage = DEFAULT_DAMAGE;
    }
    let speed = o.record.as_ref().map_or(0, |r| r[SPEED_FIELD]);
    d.phase = phase_of(&o.logic, speed);
    let or_default = |v: i32| if v == 0 { DEFAULT_HALF_MS } else { v };
    d.on_ms = or_default(o.speed_x);
    d.period = d.on_ms.wrapping_add(or_default(o.speed_y));
    d.frames = Some(frames_of(level, o, d.frames));
    o.state = state::DOWN;
}

fn set_anim(w: &World, d: &mut FloorSpikeData, key: &str) {
    d.anim.set(w.level.animations.get(key).cloned());
}

/// Plays `key` from the spike when it was on screen in the previous tick.
fn cue(w: &mut World, id: ObjectId, key: &str) {
    if w.objects.get(id).is_some_and(|o| w.on_screen(o)) {
        sound::play_from(w, id, key, SoundChannel::Effect);
    }
}

/// Advances the slide one tick; true when it has ended.
fn slide(w: &mut World, id: ObjectId, d: &mut FloorSpikeData) -> bool {
    let frames = d.frames.unwrap_or_default();
    advance_sprite(w, id, &mut d.anim, frames);
    d.anim.is_done()
}

fn set_harmless(w: &mut World, id: ObjectId, harmless: bool) {
    if let Some(o) = w.objects.get_mut(id) {
        if harmless {
            o.flags |= flags::HARMLESS;
        } else {
            o.flags &= !flags::HARMLESS;
        }
    }
}

fn set_state(w: &mut World, id: ObjectId, s: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = s;
    }
}

fn step(w: &mut World, id: ObjectId, d: &mut FloorSpikeData) {
    let ms = level_clock(w);
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => {
            let level = w.level;
            if let Some(o) = w.objects.get_mut(id) {
                set_up(level, o, d);
            }
        }
        state::DOWN => {
            set_harmless(w, id, true);
            if up_at(ms, d) {
                set_state(w, id, state::RISE_START);
            }
        }
        state::RISE_START => {
            cue(w, id, RISE_SOUND);
            set_anim(w, d, RISE_ANIM);
            set_state(w, id, state::RISING);
        }
        state::RISING => {
            let done = slide(w, id, d);
            if done {
                set_harmless(w, id, false);
                set_anim(w, d, RISE_ANIM);
                set_state(w, id, state::UP);
            }
        }
        state::UP if !up_at(ms, d) => set_state(w, id, state::DROP_START),
        state::DROP_START => {
            set_harmless(w, id, true);
            set_anim(w, d, DROP_ANIM);
            cue(w, id, DROP_SOUND);
            set_state(w, id, state::DROPPING);
        }
        state::DROPPING => {
            let done = slide(w, id, d);
            if done {
                set_anim(w, d, DROP_ANIM);
                set_state(w, id, state::DOWN);
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing can hit a spike.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A spike hurts through its type alone.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_differ_by_a_quarter_cycle() {
        assert_eq!(phase_of("FloorSpike", 60), 60);
        assert_eq!(phase_of("FloorSpike", 0), 0);
        assert_eq!(phase_of("FloorSpike2", 60), 750);
        assert_eq!(phase_of("FloorSpike3", 0), 1500);
        assert_eq!(phase_of("FloorSpike4", 0), 2250);
    }

    #[test]
    fn the_up_test_is_unsigned() {
        let d = |phase| FloorSpikeData {
            phase,
            on_ms: 1500,
            period: 3000,
            ..FloorSpikeData::default()
        };
        // Before its phase a FloorSpike3 counts from 2^32.
        assert!(up_at(704, &d(1500)));
        assert!(!up_at(705, &d(1500)));
        assert!(up_at(1454, &d(2250)));
        assert!(!up_at(1455, &d(2250)));
        assert!(!up_at(749, &d(750)));
        assert!(up_at(756, &d(750)));
        // The up half is 1501 ms, the down half 1499.
        assert!(up_at(1500, &d(0)));
        assert!(!up_at(1501, &d(0)));
        assert!(!up_at(2999, &d(0)));
        assert!(up_at(3000, &d(0)));
    }
}
