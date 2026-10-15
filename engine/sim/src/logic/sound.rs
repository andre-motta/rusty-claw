//! Playing sounds from logics.
//!
//! Every cue's volume is a percentage of the volume the player set for its
//! channel: 100 plays at the configured volume and never louder. A sound
//! the game plays at a fixed full volume (the bear hug, for example) is
//! therefore a cue at volume 100, which means the configured volume of its
//! channel, never full output. The bridge adds the channel's own offset
//! (voice lines sit above the effects, ambient loops below them).
//!
//! - [`play`] plays a sound centred.
//! - [`play_from`] places a sound at its object: the balance follows the
//!   object's horizontal offset from the centre of the view, linearly from
//!   fully left to fully right over [`PAN_RANGE_PX`] pixels on each side,
//!   and saturates beyond. The volume does not depend on the distance.
//!   Animation frame sounds that belong to their object play this way.
//! - [`claw_say`] and [`enemy_say`] are the voice helpers. Each speaker
//!   kind says one line at a time: a request that is not forced is refused
//!   while a line of the same kind is still running. While a line runs an
//!   exclamation mark is drawn over the speaker. A line never freezes
//!   anything or blocks input. The two helper objects that run the lines
//!   are made on first use and recorded with the ambient manager (see
//!   [`sound_object::find_manager`]), so the queries below do not search
//!   the level's objects.

use std::sync::Arc;

use claw_frame::{AudioCue, SoundChannel, SoundCue};

use super::sound_object::{self, SoundData, SoundPart};
use super::{GameLogic, ObjectData, World};
use crate::level::runtime_image_set;
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, engine_state, flags};
use crate::player::state;
use crate::tuning::TICK_MS;

/// Horizontal distance from the view centre, in pixels, at which a placed
/// sound is fully on one side: a third of a 640 pixel view.
pub const PAN_RANGE_PX: i32 = 213;

/// Volume of Claw's lines on the voice channel.
pub const CLAW_VOICE_VOLUME: u8 = 100;

/// Volume of enemies' lines on the voice channel: they sit 10 points below
/// Claw's lines.
pub const ENEMY_VOICE_VOLUME: u8 = 90;

/// Image set of the exclamation mark drawn over a speaker.
pub const EXCLAMATION_IMAGE_SET: &str = "GAME_EXCLAMATION";

/// Frame of the exclamation mark.
pub const EXCLAMATION_FRAME: i32 = 0;

/// The exclamation mark sits this many pixels above the top of the
/// speaker's movement box.
pub const EXCLAMATION_RISE: i32 = 30;

/// Logic name of the object that runs Claw's lines.
pub const CLAW_DIALOG_LOGIC: &str = "ClawDialogSound";

/// Logic name of the object that runs enemies' lines.
pub const ENEMY_DIALOG_LOGIC: &str = "EnemyDialogSound";

/// The centre of `view` (inclusive edges), rounded to the right and down.
pub const fn view_centre(view: Rect) -> (i32, i32) {
    (
        view.left + (view.right - view.left + 1) / 2,
        view.top + (view.bottom - view.top + 1) / 2,
    )
}

/// Balance of a sound at `x` heard from `centre_x`: from -100 (left) to
/// 100 (right), linear over [`PAN_RANGE_PX`] and saturated beyond.
pub fn positional_pan(x: i32, centre_x: i32) -> i8 {
    let d = x
        .saturating_sub(centre_x)
        .clamp(-PAN_RANGE_PX, PAN_RANGE_PX);
    // |d| <= 213, so the result is within -100..=100.
    (100 * d / PAN_RANGE_PX) as i8
}

/// Plays `key` centred on `channel` at the configured volume.
pub fn play(w: &mut World, key: &str, channel: SoundChannel) {
    w.cue(AudioCue::Play(SoundCue {
        key: key.to_string(),
        channel,
        ..SoundCue::default()
    }));
}

/// Plays `key` for the object `id` on `channel`, at the configured volume
/// and balanced by the object's side of the view (see [`positional_pan`]).
/// Without such an object the sound plays centred.
pub fn play_from(w: &mut World, id: ObjectId, key: &str, channel: SoundChannel) {
    let pan = match w.objects.get(id) {
        Some(o) => positional_pan(o.x, view_centre(w.view).0),
        None => 0,
    };
    w.cue(AudioCue::Play(SoundCue {
        key: key.to_string(),
        pan,
        channel,
        ..SoundCue::default()
    }));
}

/// Who a dialog helper speaks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Speaker {
    /// Captain Claw.
    #[default]
    Claw,
    /// An enemy.
    Enemy,
}

/// The state of a dialog helper: the line being said, if any.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DialogData {
    /// Whose lines this helper runs.
    pub speaker_kind: Speaker,
    /// The object saying the current line (an enemy's lines only).
    pub speaker: Option<ObjectId>,
    /// A line is running.
    pub active: bool,
    /// Time left of the running line, ms.
    pub line_ms: u32,
    /// Tick the line started in; its time starts counting the tick after.
    pub started_tick: u64,
}

/// Says `key` as Claw. Refused (false) when the level has no such sound,
/// or, unless `force` is set, while another of Claw's lines runs or Claw is
/// dying. An accepted line plays on the voice channel and keeps Claw's
/// line running for the sound's length; a forced line does not stop the
/// one already playing, so both are heard.
pub fn claw_say(w: &mut World, key: &str, force: bool) -> bool {
    if !w.level.sounds.contains_key(key) {
        return false;
    }
    let id = dialog_helper(w, Speaker::Claw);
    if !force && (dialog(w, id).is_some_and(|d| d.active) || claw_dying(w, true)) {
        return false;
    }
    start_line(w, id, None, key, CLAW_VOICE_VOLUME);
    true
}

/// Says `key` as the enemy `speaker`. Same rules as [`claw_say`] on the
/// enemies' own line, without the check on Claw: one enemy line runs at a
/// time, and it ends early when its speaker is destroyed or silenced.
pub fn enemy_say(w: &mut World, speaker: ObjectId, key: &str, force: bool) -> bool {
    if !w.level.sounds.contains_key(key) {
        return false;
    }
    let id = dialog_helper(w, Speaker::Enemy);
    if !force && dialog(w, id).is_some_and(|d| d.active) {
        return false;
    }
    start_line(w, id, Some(speaker), key, ENEMY_VOICE_VOLUME);
    true
}

/// Ends the running enemy line when `speaker` is saying it (the sound
/// itself plays on).
pub fn enemy_silence(w: &mut World, speaker: ObjectId) {
    let Some(id) = find_dialog_helper(w, Speaker::Enemy) else {
        return;
    };
    if let Some(d) = dialog_mut(w, id)
        && d.active
        && d.speaker == Some(speaker)
    {
        d.line_ms = 0;
    }
}

/// True while `speaker` is saying an enemy line.
pub fn enemy_is_speaking(w: &World, speaker: ObjectId) -> bool {
    find_dialog_helper(w, Speaker::Enemy)
        .and_then(|id| dialog(w, id))
        .is_some_and(|d| d.active && d.speaker == Some(speaker))
}

/// True while one of Claw's lines runs.
pub fn claw_is_speaking(w: &World) -> bool {
    find_dialog_helper(w, Speaker::Claw)
        .and_then(|id| dialog(w, id))
        .is_some_and(|d| d.active)
}

/// Claw is in a dying state; with `killed`, the killed state counts too.
pub(super) fn claw_dying(w: &World, killed: bool) -> bool {
    w.claw_object().is_some_and(|c| {
        c.state == state::DEATH_FALL
            || c.state == state::DEATH
            || (killed && c.state == engine_state::KILLED)
    })
}

fn start_line(w: &mut World, id: ObjectId, speaker: Option<ObjectId>, key: &str, volume: u8) {
    w.cue(AudioCue::Play(SoundCue {
        key: key.to_string(),
        volume,
        channel: SoundChannel::Voice,
        ..SoundCue::default()
    }));
    let line_ms = w.sound_ms(key).unwrap_or(0);
    let tick = w.tick;
    if let Some(d) = dialog_mut(w, id) {
        d.active = true;
        d.speaker = speaker;
        d.line_ms = line_ms;
        d.started_tick = tick;
    }
}

fn dialog<'a>(w: &'a World, id: ObjectId) -> Option<&'a DialogData> {
    match &w.objects.get(id)?.data {
        ObjectData::Sound(SoundData {
            part: SoundPart::Dialog(d),
        }) => Some(d),
        _ => None,
    }
}

fn dialog_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut DialogData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Sound(SoundData {
            part: SoundPart::Dialog(d),
        }) => Some(d),
        _ => None,
    }
}

/// The dialog helper of `kind`, once made: the one the ambient manager
/// recorded, as every helper is recorded when it is made.
fn find_dialog_helper(w: &World, kind: Speaker) -> Option<ObjectId> {
    let m = sound_object::find_manager(w.objects, w.claw_id)?;
    let id = sound_object::manager_at(w.objects, m)?.dialog_helper(kind)?;
    dialog(w, id)
        .is_some_and(|d| d.speaker_kind == kind)
        .then_some(id)
}

/// The dialog helper of `kind`, created on first use: an always active,
/// non-colliding object that draws the exclamation mark while a line runs.
fn dialog_helper(w: &mut World, kind: Speaker) -> ObjectId {
    if let Some(id) = find_dialog_helper(w, kind) {
        return id;
    }
    let logic = match kind {
        Speaker::Claw => CLAW_DIALOG_LOGIC,
        Speaker::Enemy => ENEMY_DIALOG_LOGIC,
    };
    let (x, y, z) = w.claw_object().map_or((0, 0, 0), |c| (c.x, c.y, c.z));
    let mut o = Object::new(
        0,
        x,
        y,
        z.saturating_add(1),
        Arc::from(logic),
        LogicKind::Game(GameLogic::SoundObject),
    );
    o.flags = flags::WORLD | flags::ALWAYS_ACTIVE | flags::NO_COLLIDE;
    o.draw_flags = draw_flags::HIDDEN;
    o.image_set = runtime_image_set(EXCLAMATION_IMAGE_SET);
    o.frame = EXCLAMATION_FRAME;
    o.data = ObjectData::Sound(SoundData {
        part: SoundPart::Dialog(DialogData {
            speaker_kind: kind,
            ..DialogData::default()
        }),
    });
    let id = w.spawn(o);
    sound_object::register_dialog_helper(w, kind, id);
    id
}

/// One logic call of a dialog helper: while a line runs, follow the
/// speaker with the exclamation mark and count the line down; it ends
/// early when Claw starts dying (Claw's lines) or the speaker is gone
/// (enemies' lines).
pub(super) fn dialog_tick(w: &mut World, id: ObjectId) {
    let Some(d) = dialog(w, id).cloned() else {
        return;
    };
    if !d.active {
        if let Some(o) = w.objects.get_mut(id) {
            o.draw_flags |= draw_flags::HIDDEN;
        }
        return;
    }
    let speaker = match d.speaker_kind {
        Speaker::Claw => w.claw_id,
        Speaker::Enemy => d.speaker,
    };
    let at = speaker.and_then(|s| w.objects.get(s)).and_then(|s| {
        (s.flags & flags::DESTROY == 0).then(|| {
            let top = if s.move_rect.is_set() {
                s.move_rect.top
            } else {
                0
            };
            (s.x, s.y + top - EXCLAMATION_RISE, s.z.saturating_add(1))
        })
    });
    let mut line_ms = d.line_ms;
    let cut = match d.speaker_kind {
        Speaker::Claw => claw_dying(w, false),
        Speaker::Enemy => at.is_none(),
    };
    if cut {
        line_ms = 0;
    }
    if w.tick != d.started_tick {
        line_ms = line_ms.saturating_sub(TICK_MS);
    }
    let ended = line_ms == 0;
    if let Some(o) = w.objects.get_mut(id) {
        if let Some((x, y, z)) = at {
            o.x = x;
            o.y = y;
            if o.z != z {
                o.z = z;
                o.flags |= flags::RESORT;
            }
        }
        if ended {
            o.draw_flags |= draw_flags::HIDDEN;
        } else {
            o.draw_flags &= !draw_flags::HIDDEN;
        }
    }
    if let Some(d) = dialog_mut(w, id) {
        d.line_ms = line_ms;
        if ended {
            d.active = false;
            d.speaker = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_is_linear_and_saturates() {
        let cases = [
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 1),
            (-3, -1),
            (106, 49),
            (107, 50),
            (-107, -50),
            (212, 99),
            (213, 100),
            (-213, -100),
            (5000, 100),
            (-5000, -100),
            (i32::MAX, 100),
        ];
        for (d, pan) in cases {
            assert_eq!(positional_pan(d, 0), pan, "offset {d}");
        }
        assert_eq!(positional_pan(i32::MIN, 1000), -100);
    }

    #[test]
    fn the_view_centre_is_the_middle_of_the_rectangle() {
        assert_eq!(view_centre(Rect::new(680, 760, 1319, 1239)), (1000, 1000));
        assert_eq!(view_centre(Rect::new(0, 0, 639, 479)), (320, 240));
        assert_eq!(view_centre(Rect::new(10, 10, 10, 10)), (10, 10));
    }
}
