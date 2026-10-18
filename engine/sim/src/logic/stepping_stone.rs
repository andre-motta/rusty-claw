//! Stepping stones: small stones floating on a goo pool that Claw hops
//! across. Two kinds share their art, landing surface and animations:
//!
//! - clock stones (`SteppingStone` to `SteppingStone4`) sink and rise on a
//!   rhythm of the level clock, like the toggle pegs: by default up for
//!   1.5 s and under for 1.5 s, each variant a quarter period after the
//!   previous one ([`phase_of`]), so a row 1, 2, 3, 4 sinks as a wave. A
//!   stone stops carrying Claw the moment it starts to sink and carries
//!   him again from the middle frame of its rise;
//! - contact stones (`StartSteppingStone`) wait, solid, until Claw stands
//!   on one; a short delay later it sinks (Claw loses it 12 ticks after
//!   landing), stays under for a second and comes back up, ready again.
//!   `OneTimeStartSteppingStone` stays under until Claw loses a life; no
//!   level places one.
//!
//! Both are platforms that never move, with the level's stepping-stone
//! surface ([`surface`]). Their two sound keys, [`SINK_SOUND`] and
//! [`RISE_SOUND`], name no sound any level ships, so the stones are
//! silent: a key the level does not register plays nothing, and no other
//! key stands in for it. The sink plays [`SINK_SOUND`] and the rise
//! [`RISE_SOUND`].
//!
//! # Timing
//!
//! A clock stone reads the clock only while up or under, and each clock
//! edge takes effect one call late. With E the call (up) whose clock test
//! fails and R the call (under) whose test passes: the stone stops being
//! solid on E + 1, its sink takes 17 calls (under on E + 17); it starts
//! to rise on R + 1, is solid again on R + 9 (the middle frame of the
//! rise) and up on R + 18. With the default halves it is solid for 75 or
//! 76 ticks of every 3 s.
//!
//! A contact stone that sees Claw on call T (T = L + 1 for a landing in
//! tick L when the stone is drawn behind Claw, L when in front) sinks from
//! T + 7, is not solid from T + 11, waits under from T + 23, rises from
//! T + 80, is solid again on T + 88 and waits for Claw again from T + 98.

use claw_frame::SoundChannel;

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::platform::{
    self, BACKWARD_ANIM, FORWARD_ANIM, NOT_PLATFORM, PLATFORM, SurfaceKind, level_clock, toggle_on,
};
use super::{ObjectData, World, sound};
use crate::object::{Object, ObjectId, Rect, flags};

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "SteppingStone",
    "SteppingStone2",
    "SteppingStone3",
    "SteppingStone4",
    "StartSteppingStone",
    "OneTimeStartSteppingStone",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[BACKWARD_ANIM, FORWARD_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Depth given to a stone placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Length of each half of a clock stone's cycle when placed without one,
/// ms.
pub const DEFAULT_HALF_MS: i32 = 1500;
/// Delay between a contact stone seeing Claw and its sink, ms.
pub const CONTACT_DELAY_MS: u32 = 125;
/// Time a contact stone stays under before rising, ms.
pub const UNDER_MS: u32 = 1000;
/// Played, when the stone was drawn in the previous tick, as a clock
/// stone starts to sink. No level ships it.
pub const SINK_SOUND: &str = "LEVEL_STONERISE";
/// Played, when the stone was drawn in the previous tick, as a clock
/// stone starts to rise. No level ships it.
pub const RISE_SOUND: &str = "LEVEL_STONESINK";

/// Record field holding `SteppingStone`'s own cycle offset.
pub const PHASE_FIELD: usize = 60;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;

    /// States of the clock stones.
    pub mod clock {
        /// Up and solid, waiting for the clock to say sink.
        pub const UP: i32 = 1006;
        /// Not solid from this call: the sink starts.
        pub const SINK_START: i32 = 1004;
        /// Sinking.
        pub const SINKING: i32 = 1003;
        /// Under, waiting for the clock to say rise.
        pub const UNDER: i32 = 1007;
        /// The rise starts.
        pub const RISE_START: i32 = 1002;
        /// Rising; solid from the middle frame.
        pub const RISING: i32 = 1005;
    }

    /// States of the contact stones.
    pub mod contact {
        /// Up and solid, waiting for Claw.
        pub const WAITING: i32 = 1007;
        /// Claw was seen: the sink starts after a short delay.
        pub const DELAYED: i32 = 1002;
        /// Sinking; not solid from the middle frame.
        pub const SINKING: i32 = 1005;
        /// Sunk: waits under for a second.
        pub const SUNK: i32 = 1006;
        /// The rise starts.
        pub const RISE_START: i32 = 1004;
        /// Rising; solid from the middle frame.
        pub const RISING: i32 = 1003;
    }
}

/// The cycle offset of the clock stone running `name`, ms:
/// `SteppingStone` takes `record_speed`, the others a quarter, half and
/// three quarters of the default cycle.
pub fn phase_of(name: &str, record_speed: i32) -> i32 {
    match name {
        "SteppingStone2" => 750,
        "SteppingStone3" => 1500,
        "SteppingStone4" => 2250,
        _ => record_speed,
    }
}

/// True for the contact stones' names.
pub fn is_contact(name: &str) -> bool {
    matches!(name, "StartSteppingStone" | "OneTimeStartSteppingStone")
}

/// The landing surface of a stone in level `level`, relative to its
/// position: the level's stepping-stone rectangle, as the elevators that
/// wear the stone's art use it.
pub fn surface(level: u32) -> Rect {
    platform::surface(level, SurfaceKind::SteppingStone)
}

/// A stepping stone's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SteppingStoneData {
    /// Cycle offset, ms (clock stones).
    pub phase: i32,
    /// Time up in each cycle, ms (clock stones).
    pub on_ms: i32,
    /// Cycle length, ms (clock stones).
    pub period: i32,
    /// Frame at which the stone stops or starts being solid: the middle
    /// of its image set.
    pub switch_frame: i32,
    /// First and last frame numbers of the stone's image set, when known
    /// (see [`super::decor::image_frames`]).
    pub frames: Option<(i32, i32)>,
    /// The sink or rise animation.
    pub anim: SpriteAnim,
    /// Claw's attempt number when the stone was set up (contact stones).
    pub attempt: u32,
}

/// The stepping stone data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&SteppingStoneData> {
    match &o.data {
        ObjectData::SteppingStone(d) => Some(d),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let contact = is_contact(&o.logic);
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::SteppingStone(d) => d,
        _ => SteppingStoneData::default(),
    };
    if contact {
        contact_step(w, id, &mut d);
    } else {
        clock_step(w, id, &mut d);
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::SteppingStone(d);
    }
}

/// The set-up both kinds share: the set's first image, a platform out of
/// pair collision with the level's stone surface, a default depth and the
/// switch frame.
fn common_set_up(w: &mut World, id: ObjectId, d: &mut SteppingStoneData) {
    let level = w.level;
    let number = w.level_number();
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.platform_move = (0, 0);
    let frames = frames_of(level, o, d.frames);
    d.frames = Some(frames);
    if !o.image_set.is_empty() {
        o.frame = frames.0;
    }
    o.type_flags = PLATFORM;
    o.flags |= flags::NO_COLLIDE;
    o.hit_rect = surface(number);
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    d.switch_frame = (frames.0 + frames.1) / 2;
}

fn set_anim(w: &World, d: &mut SteppingStoneData, key: &str) {
    d.anim.set(w.level.animations.get(key).cloned());
}

/// Advances the animation one tick; returns the frame shown.
fn advance(w: &mut World, id: ObjectId, d: &mut SteppingStoneData) -> i32 {
    let frames = d.frames.unwrap_or_default();
    advance_sprite(w, id, &mut d.anim, frames);
    w.objects.get(id).map_or(0, |o| o.frame)
}

fn set_type(w: &mut World, id: ObjectId, type_flags: u32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.type_flags = type_flags;
    }
}

fn set_state(w: &mut World, id: ObjectId, s: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = s;
    }
}

/// Plays `key` from the stone when it was on screen in the previous tick
/// and the level has such a sound; a key no sound is registered under
/// plays nothing.
fn cue(w: &mut World, id: ObjectId, key: &str) {
    if w.level.sounds.contains_key(key) && w.objects.get(id).is_some_and(|o| w.on_screen(o)) {
        sound::play_from(w, id, key, SoundChannel::Effect);
    }
}

fn clock_step(w: &mut World, id: ObjectId, d: &mut SteppingStoneData) {
    use state::clock::*;
    let ms = level_clock(w);
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => {
            common_set_up(w, id, d);
            let Some(o) = w.objects.get_mut(id) else {
                return;
            };
            let field = o.record.as_ref().map_or(0, |r| r[PHASE_FIELD]);
            d.phase = phase_of(&o.logic, field);
            let or_default = |v: i32| if v == 0 { DEFAULT_HALF_MS } else { v };
            d.on_ms = or_default(o.speed_x);
            d.period = d.on_ms.wrapping_add(or_default(o.speed_y));
            o.state = UP;
        }
        UP if !toggle_on(ms, d.phase, d.on_ms, d.period) => set_state(w, id, SINK_START),
        SINK_START => {
            set_type(w, id, NOT_PLATFORM);
            set_anim(w, d, FORWARD_ANIM);
            cue(w, id, SINK_SOUND);
            set_state(w, id, SINKING);
        }
        SINKING => {
            if advance(w, id, d) == d.switch_frame {
                set_type(w, id, NOT_PLATFORM);
            }
            if d.anim.is_done() {
                set_type(w, id, NOT_PLATFORM);
                set_anim(w, d, BACKWARD_ANIM);
                set_state(w, id, UNDER);
            }
        }
        UNDER if toggle_on(ms, d.phase, d.on_ms, d.period) => set_state(w, id, RISE_START),
        RISE_START => {
            cue(w, id, RISE_SOUND);
            set_state(w, id, RISING);
        }
        RISING => {
            if advance(w, id, d) == d.switch_frame {
                set_type(w, id, PLATFORM);
            }
            if d.anim.is_done() {
                set_type(w, id, PLATFORM);
                set_anim(w, d, FORWARD_ANIM);
                set_state(w, id, UP);
            }
        }
        _ => {}
    }
}

fn contact_step(w: &mut World, id: ObjectId, d: &mut SteppingStoneData) {
    use state::contact::*;
    let rider = platform::rider(w, id);
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => {
            common_set_up(w, id, d);
            d.attempt = rider.attempt;
            set_state(w, id, WAITING);
        }
        WAITING if rider.on => {
            if let Some(o) = w.objects.get_mut(id) {
                o.time_delay = CONTACT_DELAY_MS;
                o.state = DELAYED;
            }
            set_anim(w, d, FORWARD_ANIM);
        }
        DELAYED => set_state(w, id, SINKING),
        SINKING => {
            if advance(w, id, d) == d.switch_frame {
                set_type(w, id, NOT_PLATFORM);
            }
            if d.anim.is_done() {
                let one_time = w
                    .objects
                    .get(id)
                    .is_some_and(|o| &*o.logic == "OneTimeStartSteppingStone");
                if !one_time {
                    set_state(w, id, SUNK);
                } else if d.attempt != rider.attempt {
                    // Claw lost a life since: built again where placed.
                    if let Some(o) = w.objects.get_mut(id) {
                        o.x = o.editor.0;
                        o.y = o.editor.1;
                        o.state = state::SET_UP;
                    }
                    d.attempt = rider.attempt;
                }
            }
        }
        SUNK => {
            if let Some(o) = w.objects.get_mut(id) {
                o.time_delay = UNDER_MS;
                o.state = RISE_START;
            }
        }
        RISE_START => {
            set_type(w, id, NOT_PLATFORM);
            set_anim(w, d, BACKWARD_ANIM);
            set_state(w, id, RISING);
        }
        RISING => {
            if advance(w, id, d) == d.switch_frame {
                set_type(w, id, PLATFORM);
            }
            if d.anim.is_done() {
                set_anim(w, d, BACKWARD_ANIM);
                set_state(w, id, WAITING);
            }
        }
        _ => {}
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_differ_by_a_quarter_cycle() {
        assert_eq!(phase_of("SteppingStone", 2000), 2000);
        assert_eq!(phase_of("SteppingStone2", 0), 750);
        assert_eq!(phase_of("SteppingStone3", 0), 1500);
        assert_eq!(phase_of("SteppingStone4", 0), 2250);
        assert!(is_contact("StartSteppingStone"));
        assert!(is_contact("OneTimeStartSteppingStone"));
        assert!(!is_contact("SteppingStone"));
    }

    #[test]
    fn the_fourth_level_stone_surface_is_32_px_wide() {
        assert_eq!(surface(4), Rect::new(-16, -1, 16, 64));
    }
}
