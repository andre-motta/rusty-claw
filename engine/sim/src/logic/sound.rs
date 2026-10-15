//! Playing sounds from logics.
//!
//! [`play`] plays a sound centred at the configured volume; [`play_from`]
//! plays one that belongs to an object. Placing a sound at its object
//! (volume and balance from the distance to the view) is not filled in
//! yet, so [`play_from`] plays it centred too.

use claw_frame::{AudioCue, SoundChannel, SoundCue};

use super::World;
use crate::object::ObjectId;

/// Plays `key` centred on `channel` at the configured volume.
pub fn play(w: &mut World, key: &str, channel: SoundChannel) {
    w.cue(AudioCue::Play(SoundCue {
        key: key.to_string(),
        channel,
        ..SoundCue::default()
    }));
}

/// Plays `key` for the object `id` on `channel`.
pub fn play_from(w: &mut World, id: ObjectId, key: &str, channel: SoundChannel) {
    let _ = id;
    play(w, key, channel);
}
