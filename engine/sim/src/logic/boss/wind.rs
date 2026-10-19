//! The wind RedTail raises across his arena from the ledge. It is an
//! invisible object that waits until RedTail raises his wind flag
//! ([`crate::logic::enemy::redtail::RedTailLink::wind`]); then it blows
//! for [`BLOW_MS`], whistling, blowing debris across the arena
//! ([`super::debris`], one piece every [`DEBRIS_MS`]) and pushing Claw
//! toward the stakes ([`push_claw`]) while his position lies in
//! [`BOX`]. The blow ends early when the flag is cleared (Claw's
//! respawn), and its end clears the flag.
//!
//! The push is the conveyor belts' ([`crate::logic::conveyor_belt`]):
//! Claw's existing movement reads it from his next tick on, a standing
//! Claw drifting 5 px a tick, a walking or jumping one moving at his own
//! speed plus [`PUSH`], a falling one not at all. The wind writes it on
//! every tick it blows, after Claw's and RedTail's ticks, whatever Claw
//! is doing.
//!
//! # The rules
//!
//! - Built on its first call: hidden; it lowers the wind flag when
//!   RedTail is there, and waits.
//! - Waiting: when RedTail is there with his flag up, the blow starts (in
//!   the same tick, without a push yet): [`BLOW_MS`] from now, the
//!   whistle looping.
//! - The whistle plays at [`WHISTLE_VOLUME`] from the blow's start and
//!   stops at its end (the game fades it in over a second and out over
//!   three quarters of one; this build has no fades).
//! - Blowing, every tick: a piece of debris when [`DEBRIS_MS`] have
//!   passed since the last one (the first on the blow's first tick after
//!   its start); the push; then, once the blow's time is over or the flag
//!   is down, no push, the whistle stops and it waits again with the flag
//!   down.

use claw_frame::{AudioCue, SoundCue};

use crate::logic::enemy::Timer;
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, draw_flags, engine_state};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailWind"];

/// Image sets this family may switch an object to: none (it is hidden).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: none.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// How long one blow lasts, ms (278 ticks at the 18 ms step).
pub const BLOW_MS: u32 = 5000;
/// The time between two pieces of debris, ms (every 6 ticks).
pub const DEBRIS_MS: u32 = 100;
/// The push on Claw while he is in [`BOX`], px/s (to the left).
pub const PUSH: f64 = -300.0;
/// Where Claw's position must lie for the push: left and top inside,
/// right and bottom outside (x0, y0, x1, y1).
pub const BOX: (i32, i32, i32, i32) = (37100, 1920, 37900, 2150);
/// The whistle that loops while it blows.
pub const WHISTLE: &str = "LEVEL_REDTAIL_WINDWHISTLING";
/// The handle of the looping whistle.
pub const WHISTLE_VOICE: u32 = 0x0B00_0013;
/// The whistle's volume once it has risen.
pub const WHISTLE_VOLUME: u8 = 80;

/// The wind's states, in its object's state.
pub mod state {
    /// Waiting for RedTail's flag.
    pub const IDLE: i32 = 5;
    /// Blowing.
    pub const BLOW: i32 = 6;
}

/// The wind's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WindData {
    /// The blow's time.
    pub blow: Timer,
    /// The time since the last piece of debris.
    pub debris: Timer,
    /// The whistle is playing.
    pub whistling: bool,
}

/// True when Claw's position (`x`, `y`) lies in the wind's box.
pub fn in_box(x: i32, y: i32) -> bool {
    let (x0, y0, x1, y1) = BOX;
    (x0..x1).contains(&x) && (y0..y1).contains(&y)
}

/// Writes the wind's push on Claw: [`PUSH`] when `inside` (Claw's
/// position in [`BOX`]), else none. Claw's own movement applies it from
/// his next tick.
pub fn push_claw(w: &mut World, inside: bool) {
    w.claw.conveyor_force = if inside { PUSH } else { 0.0 };
}

fn wind_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut WindData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::RedTailWind(d) => Some(d),
        _ => None,
    }
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(st) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match st {
        engine_state::INIT => {
            if let Some(o) = w.objects.get_mut(id) {
                o.draw_flags |= draw_flags::HIDDEN;
                o.data = ObjectData::RedTailWind(WindData::default());
            }
            enter_idle(w, id);
        }
        state::IDLE => {
            let link = *w.redtail_link();
            if link.redtail.is_some() && link.wind {
                enter_blow(w, id);
            }
        }
        state::BLOW => blow(w, id),
        _ => {}
    }
}

/// Waits again: the whistle stops and, when RedTail is there, his flag
/// goes down.
fn enter_idle(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::IDLE;
    }
    if wind_mut(w, id).is_some_and(|d| std::mem::take(&mut d.whistling)) {
        w.cue(AudioCue::Stop {
            voice: WHISTLE_VOICE,
        });
    }
    if w.redtail_link().redtail.is_some() {
        w.redtail_link_mut().wind = false;
    }
}

/// The blow starts: its time and the whistle.
fn enter_blow(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::BLOW;
    }
    if let Some(d) = wind_mut(w, id) {
        d.blow = Timer::new(now, u64::from(BLOW_MS));
        d.whistling = true;
    }
    w.cue(AudioCue::Play(SoundCue {
        key: WHISTLE.to_string(),
        volume: WHISTLE_VOLUME,
        looped: true,
        voice: Some(WHISTLE_VOICE),
        ..SoundCue::default()
    }));
}

/// A tick of the blow: debris, the push, and its end.
fn blow(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let piece = wind_mut(w, id).is_some_and(|d| d.debris.done(now));
    if piece {
        super::debris::launch(w);
        if let Some(d) = wind_mut(w, id) {
            d.debris = Timer::new(now, u64::from(DEBRIS_MS));
        }
    }
    let inside = w.claw_object().is_some_and(|c| in_box(c.x, c.y));
    push_claw(w, inside);
    let over = wind_mut(w, id).is_some_and(|d| d.blow.done(now));
    if over || !w.redtail_link().wind {
        push_claw(w, false);
        enter_idle(w, id);
    }
}

/// `attacker` hit `id`. Nothing hits the wind.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The wind attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_takes_its_left_and_top_edges() {
        assert!(in_box(37100, 1920));
        assert!(in_box(37899, 2149));
        assert!(!in_box(37900, 2000));
        assert!(!in_box(37500, 2150));
        assert!(!in_box(37099, 2000));
        // A blow lasts 278 ticks at the 18 ms step; debris every 6.
        assert_eq!(BLOW_MS.div_ceil(18), 278);
        assert_eq!(DEBRIS_MS.div_ceil(18), 6);
        // A standing Claw drifts 5 px a tick.
        assert_eq!((PUSH * 0.018).trunc(), -5.0);
    }
}
