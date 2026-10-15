//! Sound objects placed in a level: sound triggers, Claw's dialog triggers
//! and ambient loops. None of them is ever drawn. A record's animation
//! string names its sound; a name the level does not register leaves the
//! object silent.
//!
//! Sound triggers (`SoundTrigger` and its sized variants) play their sound
//! once when Claw's position enters a square around them; nothing else
//! sets them off. The record's smarts field counts the uses: 0 (read as
//! 99999) plays once per life, N plays at most N times, each in a
//! different life, and 1 removes the trigger after its use. A used trigger
//! re-arms once Claw has died and come back (the ambient manager counts
//! those returns as Claw leaves a dying state). Smarts -1 instead re-arms
//! when Claw leaves the square, at the earliest the sound's length plus
//! 100 ms later, so it plays on every new entry (puddle splashes). A
//! trigger only watches Claw while it is awake.
//!
//! Dialog triggers (`ClawDialogSoundTrigger` and its sized variants) ask
//! Claw to say their line on the voice channel (see
//! [`sound::claw_say`]). They count a use even when Claw refuses the line
//! (another line running, or Claw dying): smarts 1 lines are said at most
//! once per level, others at most once per life. They stay awake.
//!
//! Ambient loops belong to one ambient manager object, which updates them
//! once per tick after every other object, listening from the centre of
//! the view:
//!
//! - `GlobalAmbientSound` registers at level load and `AmbientSound` when
//!   it first wakes; both then remove themselves (one without a sound
//!   stays, idle) and their loop lasts to the end of the level. Without an
//!   area the loop plays everywhere at once; with one it fades in over a
//!   second while the view centre is inside it and fades out over a second
//!   once it leaves. A record with a movement box makes the loop come and
//!   go on a random timer instead, drawn from the game's random stream.
//! - `SpotAmbientSound` (and `AmbientPosSound`) holds a loop at its own
//!   position while it is awake: louder as the view centre gets closer,
//!   balanced to its side, silent beyond 640 pixels on either axis, and
//!   stopped when the object goes to sleep.
//!
//! Each sound key has one looping voice, chosen by a handle from
//! [`AMBIENT_VOICE_BASE`] up. When several loops share a key, the voice
//! takes the level and balance of the loudest one that plays. A Play cue
//! for a voice that already plays changes its volume and balance; it does
//! not restart the sound. A loop that stops ends with a Stop cue.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use claw_frame::{AudioCue, SoundChannel, SoundCue};

use super::sound::{self, DialogData};
use super::{GameLogic, ObjectData, World};
use crate::container::Container;
use crate::object::{LogicKind, Object, ObjectId, Rect, draw_flags, engine_state, flags};
use crate::rng::Rng;
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "AmbientPosSound",
    "AmbientSound",
    "BigSoundTrigger",
    "ClawDialogBigSoundTrigger",
    "ClawDialogHugeSoundTrigger",
    "ClawDialogSmallSoundTrigger",
    "ClawDialogSoundTrigger",
    "ClawDialogTallSoundTrigger",
    "ClawDialogTinySoundTrigger",
    "ClawDialogWideSoundTrigger",
    "GlobalAmbientSound",
    "HugeSoundTrigger",
    "SmallSoundTrigger",
    "SoundTrigger",
    "SpotAmbientSound",
    "TallSoundTrigger",
    "TinySoundTrigger",
    "WideSoundTrigger",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[sound::EXCLAMATION_IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Logic name of the object that runs the ambient loops.
pub const AMBIENT_MANAGER_LOGIC: &str = "AmbientSoundManager";

/// First voice handle of the ambient loops; each sound key takes the next
/// one.
pub const AMBIENT_VOICE_BASE: u32 = 0x0A00_0000;

/// Length of an ambient fade in or out, ms.
pub const FADE_MS: u64 = 1000;

/// A spot loop is silent when the view centre is farther than this on
/// either axis, px.
pub const SPOT_RANGE_PX: i32 = 640;

/// A trigger that re-arms on leaving waits this long after its sound
/// before it watches Claw again, ms.
pub const REARM_EXTRA_MS: u32 = 100;

/// The smarts value a record's 0 stands for.
pub const DEFAULT_SMARTS: i32 = 99_999;

/// The kind bit every sound object carries.
pub const SOUND_OBJECT_TYPE: u32 = 0x400_0000;

/// States of the trigger logics.
pub mod trigger_state {
    /// Set-up.
    pub const INIT: i32 = 0;
    /// Watching for Claw.
    pub const ARMED: i32 = 5;
    /// Waiting for Claw to leave (smarts -1).
    pub const WAIT_LEAVE: i32 = 11;
    /// Used; re-arms after Claw's next death.
    pub const SPENT: i32 = 23;
}

/// A sound object's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundData {
    /// What the object is.
    pub part: SoundPart,
}

/// The kinds of sound objects.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SoundPart {
    /// Not set up yet.
    #[default]
    None,
    /// A sound or dialog trigger.
    Trigger(TriggerData),
    /// A spot loop's owner.
    Spot(SpotData),
    /// The helper that runs Claw's or enemies' lines.
    Dialog(DialogData),
    /// The ambient manager.
    Manager(Box<AmbientManager>),
}

/// A sound or dialog trigger.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerData {
    /// Sound key, when the level registers it.
    pub key: Option<String>,
    /// Asks Claw to say the line instead of playing an effect.
    pub dialog: bool,
    /// The square Claw must enter, relative to the trigger.
    pub area: Rect,
    /// Uses left (see the module notes).
    pub smarts: i32,
    /// Claw's death count when the trigger was last used.
    pub attempt: u32,
    /// The ambient manager, which counts Claw's deaths.
    pub manager: Option<ObjectId>,
}

/// The owner of a spot loop.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpotData {
    /// Sound key, when the level registers it.
    pub key: Option<String>,
    /// The ambient manager.
    pub manager: Option<ObjectId>,
}

/// Every ambient loop of the level, and Claw's death count.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AmbientManager {
    /// The loops, in registration order.
    pub entries: Vec<AmbientEntry>,
    /// The voice of each sound key.
    pub voices: BTreeMap<String, KeyVoice>,
    /// Voices handed out so far.
    pub next_voice: u32,
    /// Times Claw has died and come back.
    pub attempt: u32,
    /// Claw was dying at the last update.
    pub claw_dying: bool,
}

/// The looping voice of one sound key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyVoice {
    /// Handle of the voice.
    pub handle: u32,
    /// The voice plays.
    pub playing: bool,
    /// Volume last sent.
    pub volume: u8,
    /// Balance last sent.
    pub pan: i8,
}

/// One ambient loop.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientEntry {
    /// Sound key.
    pub key: String,
    /// The spot object that owns the loop.
    pub owner: Option<ObjectId>,
    /// How the loop decides to play.
    pub kind: AmbientKind,
    /// Percent applied to the loop's level (the record's damage; 0 or less
    /// leaves the level as it is).
    pub percent: i32,
    /// The loop plays.
    pub playing: bool,
    /// Current volume, 0 to 100.
    pub volume: i32,
    /// Current balance, -100 to 100.
    pub pan: i32,
    /// Fade in progress.
    pub fade: Option<Fade>,
}

/// How an ambient loop decides to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientKind {
    /// Plays while the view centre is strictly inside `area` or `extra`,
    /// or everywhere when `area` is unset.
    Area {
        /// The main area, absolute.
        area: Rect,
        /// A second area, absolute (unset when the record has none).
        extra: Rect,
    },
    /// Inside its areas (or everywhere), turns on and off on a random
    /// timer.
    Timed {
        /// The main area, absolute.
        area: Rect,
        /// A second area, absolute.
        extra: Rect,
        /// Bounds of an on period, ms.
        on: (i32, i32),
        /// Bounds of an off period, ms.
        off: (i32, i32),
        /// In an on period.
        on_phase: bool,
        /// Time left in the period, ms.
        timer_ms: i32,
    },
    /// Placed at a point; level and balance follow the distance.
    Spot {
        /// Position.
        x: i32,
        /// Position.
        y: i32,
    },
}

/// A linear volume ramp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fade {
    /// Volume at the start.
    pub from: i32,
    /// Volume at the end.
    pub to: i32,
    /// Simulation time at the start, ms.
    pub start_ms: u64,
    /// Stop the loop at the end.
    pub stop: bool,
}

/// The square a trigger running `logic` watches, relative to the trigger.
pub fn trigger_area(logic: &str) -> Rect {
    let name = logic.strip_prefix("ClawDialog").unwrap_or(logic);
    match name {
        "BigSoundTrigger" => Rect::new(-128, -128, 128, 128),
        "SmallSoundTrigger" => Rect::new(-32, -32, 32, 32),
        "HugeSoundTrigger" => Rect::new(-256, -256, 256, 256),
        "TinySoundTrigger" => Rect::new(-16, -16, 16, 16),
        "TallSoundTrigger" => Rect::new(-32, -100, 32, 100),
        "WideSoundTrigger" => Rect::new(-100, -32, 100, 32),
        _ => Rect::new(-64, -64, 64, 64),
    }
}

/// An ambient loop's volume at `level` (0 to 100) with the record's
/// `percent`.
pub fn ambient_volume(level: i32, percent: i32) -> i32 {
    let v = if percent > 0 {
        i64::from(level) * i64::from(percent) / 100
    } else {
        i64::from(level)
    };
    v.clamp(0, 100) as i32
}

/// Level and balance of a spot loop at (sx, sy) heard from (cx, cy), or
/// `None` beyond [`SPOT_RANGE_PX`] on either axis: the level falls by one
/// point every 6 pixels of distance, and the balance is a quarter of the
/// horizontal distance, negative when the spot is left of the listener.
pub fn spot_level_pan(sx: i32, sy: i32, cx: i32, cy: i32) -> Option<(i32, i32)> {
    let dx = (i64::from(sx) - i64::from(cx)).unsigned_abs();
    let dy = (i64::from(sy) - i64::from(cy)).unsigned_abs();
    let range = SPOT_RANGE_PX as u64;
    if dx > range || dy > range {
        return None;
    }
    let dist = (dx * dx + dy * dy).isqrt() as i32;
    let level = (100 - dist / 6).clamp(0, 100);
    let pan = (dx as i32 / 4).clamp(0, 100);
    Some((level, if sx < cx { -pan } else { pan }))
}

/// Volume of a fade from `from` to `to` after `elapsed_ms` of its
/// [`FADE_MS`].
pub fn fade_volume(from: i32, to: i32, elapsed_ms: u64) -> i32 {
    if elapsed_ms >= FADE_MS {
        return to;
    }
    let step = (i64::from(to) - i64::from(from)) * elapsed_ms as i64 / FADE_MS as i64;
    (i64::from(from) + step) as i32
}

/// A random number from `lo` to `hi` inclusive, drawn as the game draws
/// it: `lo + r % (hi - lo + 1)`, or a coin toss between `lo` and `hi` when
/// that count is 0.
pub fn rand_range(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    let n = i64::from(hi) - i64::from(lo) + 1;
    let r = i64::from(rng.next_u15());
    if n == 0 {
        return if r & 1 != 0 { lo } else { hi };
    }
    (i64::from(lo) + r % n) as i32
}

/// The ambient manager of a level, once a sound object has made one.
pub fn manager_of(objects: &Container) -> Option<&AmbientManager> {
    objects.all().find_map(|(_, o)| match &o.data {
        ObjectData::Sound(SoundData {
            part: SoundPart::Manager(m),
        }) => Some(&**m),
        _ => None,
    })
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else {
        return;
    };
    match &o.data {
        ObjectData::Sound(SoundData {
            part: SoundPart::Manager(_),
        }) => return manager_tick(w, id),
        ObjectData::Sound(SoundData {
            part: SoundPart::Dialog(_),
        }) => return sound::dialog_tick(w, id),
        _ => {}
    }
    let logic = o.logic.clone();
    match &*logic {
        "AmbientSound" => ambient_tick(w, id, false),
        "GlobalAmbientSound" => ambient_tick(w, id, true),
        "SpotAmbientSound" | "AmbientPosSound" => spot_tick(w, id),
        name if name.starts_with("ClawDialog") => trigger_tick(w, id, true),
        _ => trigger_tick(w, id, false),
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

// ------------------------------------------------------------- set-up

/// The record of `id`, or one rebuilt from the object's fields for an
/// object made in code.
fn record_of(o: &Object) -> [i32; 71] {
    if let Some(r) = o.record.as_deref() {
        return *r;
    }
    let mut r = [0; 71];
    let rect = |r: &mut [i32; 71], at: usize, rect: Rect| {
        if rect.is_set() {
            r[at..at + 4].copy_from_slice(&[rect.left, rect.top, rect.right, rect.bottom]);
        }
    };
    r[5] = o.x;
    r[6] = o.y;
    r[7] = o.z;
    r[16] = o.damage;
    rect(&mut r, 19, o.move_rect);
    rect(&mut r, 23, o.hit_rect);
    rect(&mut r, 27, o.attack_rect);
    r
}

/// The sound key named by the record of `id`, when the level registers
/// it.
fn sound_key(w: &World, id: ObjectId) -> Option<String> {
    let o = w.objects.get(id)?;
    let r = o.record.as_deref()?;
    let spec = w
        .level
        .objects
        .iter()
        .find(|s| s.record == *r && *s.logic == *o.logic)?;
    w.level
        .sounds
        .contains_key(&spec.anim)
        .then(|| spec.anim.clone())
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut SoundPart> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Sound(d) => Some(&mut d.part),
        _ => None,
    }
}

fn manager_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut AmbientManager> {
    match data_mut(w, id)? {
        SoundPart::Manager(m) => Some(&mut **m),
        _ => None,
    }
}

/// The ambient manager: `cached` when it still is one, else the level's,
/// made on first use. It runs after every other object, from the tick it
/// is made in.
fn manager(w: &mut World, cached: Option<ObjectId>) -> ObjectId {
    if let Some(id) = cached
        && manager_mut(w, id).is_some()
    {
        return id;
    }
    let found = w.objects.all().find_map(|(id, o)| {
        matches!(
            o.data,
            ObjectData::Sound(SoundData {
                part: SoundPart::Manager(_)
            })
        )
        .then_some(id)
    });
    if let Some(id) = found {
        return id;
    }
    let mut o = Object::new(
        0,
        0,
        0,
        i32::MAX,
        Arc::from(AMBIENT_MANAGER_LOGIC),
        LogicKind::Game(GameLogic::SoundObject),
    );
    o.flags = flags::ALWAYS_ACTIVE | flags::NO_COLLIDE | flags::RUN_AT_CREATION;
    o.draw_flags = draw_flags::HIDDEN;
    o.data = ObjectData::Sound(SoundData {
        part: SoundPart::Manager(Box::default()),
    });
    w.spawn(o)
}

fn attempt(w: &mut World, manager: Option<ObjectId>) -> u32 {
    manager
        .and_then(|id| manager_mut(w, id))
        .map_or(0, |m| m.attempt)
}

/// Marks `id` as an undrawn, non-colliding sound object.
fn quiet(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NO_COLLIDE;
        o.draw_flags |= draw_flags::HIDDEN;
    }
}

// ------------------------------------------------------------ triggers

fn trigger_tick(w: &mut World, id: ObjectId, dialog: bool) {
    let Some(o) = w.objects.get(id) else {
        return;
    };
    let (state, x, y) = (o.state, o.x, o.y);
    if state == trigger_state::INIT {
        return trigger_init(w, id, dialog);
    }
    // The data is moved out for the call and put back at the end.
    let mut t = match data_mut(w, id).map(std::mem::take) {
        Some(SoundPart::Trigger(t)) => t,
        Some(other) => {
            if let Some(p) = data_mut(w, id) {
                *p = other;
            }
            return;
        }
        None => return,
    };
    let claw = w.claw_object().map(|c| (c.x, c.y));
    let inside = claw.is_some_and(|(cx, cy)| t.area.translate(x, y).contains(cx, cy));
    let mut next = state;
    match state {
        trigger_state::ARMED if inside => {
            next = trigger_state::SPENT;
            if dialog {
                if let Some(key) = &t.key {
                    sound::claw_say(w, key, false);
                }
                if t.smarts == 1 {
                    w.destroy(id);
                } else {
                    t.smarts = t.smarts.saturating_sub(1);
                }
            } else {
                if let Some(key) = &t.key {
                    sound::play(w, key, SoundChannel::Effect);
                }
                if t.smarts == 1 {
                    w.destroy(id);
                } else if t.smarts > 1 {
                    t.smarts -= 1;
                }
            }
            t.attempt = attempt(w, t.manager);
            if !dialog
                && t.smarts == -1
                && let Some(key) = &t.key
            {
                next = trigger_state::WAIT_LEAVE;
                let delay = w.sound_ms(key).unwrap_or(0).saturating_add(REARM_EXTRA_MS);
                if let Some(o) = w.objects.get_mut(id) {
                    o.time_delay = delay;
                }
            }
        }
        trigger_state::WAIT_LEAVE if !inside => next = trigger_state::ARMED,
        trigger_state::SPENT if attempt(w, t.manager) != t.attempt => {
            next = trigger_state::ARMED;
        }
        _ => {}
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.state = next;
    }
    if let Some(p) = data_mut(w, id) {
        *p = SoundPart::Trigger(t);
    }
}

fn trigger_init(w: &mut World, id: ObjectId, dialog: bool) {
    let key = sound_key(w, id);
    let cached = match data_mut(w, id) {
        Some(SoundPart::Trigger(t)) => t.manager,
        _ => None,
    };
    let manager = manager(w, cached);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let smarts = match o.record.as_deref().map_or(0, |r| r[17]) {
        0 => DEFAULT_SMARTS,
        s => s,
    };
    let area = trigger_area(&o.logic);
    if !dialog {
        o.flags &= !flags::ALWAYS_ACTIVE;
    }
    o.flags |= flags::NO_COLLIDE;
    o.draw_flags |= draw_flags::HIDDEN;
    o.type_flags = SOUND_OBJECT_TYPE;
    o.hit_rect = area;
    o.state = trigger_state::ARMED;
    o.data = ObjectData::Sound(SoundData {
        part: SoundPart::Trigger(TriggerData {
            key,
            dialog,
            area,
            smarts,
            attempt: 0,
            manager: Some(manager),
        }),
    });
}

// ------------------------------------------------------- ambient loops

/// Converts an absolute record area; all zeros means none.
fn area_of(l: i32, t: i32, r: i32, b: i32) -> Rect {
    if l == 0 && t == 0 && r == 0 && b == 0 {
        Rect::UNSET
    } else {
        Rect::new(l, t, r, b)
    }
}

fn ambient_tick(w: &mut World, id: ObjectId, global: bool) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state != engine_state::INIT {
        return;
    }
    if global {
        o.flags |= flags::ALWAYS_ACTIVE;
    } else {
        o.flags &= !flags::ALWAYS_ACTIVE;
    }
    let r = record_of(o);
    quiet(w, id);
    let Some(key) = sound_key(w, id) else {
        // Without a sound there is nothing to register: the object stays,
        // idle.
        if let Some(o) = w.objects.get_mut(id) {
            o.state = trigger_state::ARMED;
        }
        return;
    };
    let mut area = area_of(r[23], r[24], r[25], r[26]);
    if r[51] > 0 || r[53] > 0 {
        area = area_of(r[51], r[52], r[53], r[54]);
    }
    let extra = if r[28] > 0 {
        Rect::new(r[27], r[28], r[29], r[30])
    } else {
        Rect::UNSET
    };
    let kind = if r[20] > 0 {
        let off = (r[21], r[22]);
        AmbientKind::Timed {
            area,
            extra,
            on: (r[19], r[20]),
            off,
            on_phase: true,
            timer_ms: rand_range(w.rng, off.0, off.1),
        }
    } else {
        AmbientKind::Area { area, extra }
    };
    let m = manager(w, None);
    if let Some(m) = manager_mut(w, m) {
        m.entries.push(AmbientEntry::new(key, None, kind, r[16]));
    }
    w.destroy(id);
}

fn spot_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else {
        return;
    };
    let (state, x, y, percent) = (o.state, o.x, o.y, o.damage);
    match state {
        engine_state::INIT => {}
        engine_state::SLEEPING | engine_state::DESTROYING => {
            let manager = match data_mut(w, id) {
                Some(SoundPart::Spot(s)) => s.manager,
                _ => None,
            };
            if let Some(m) = manager {
                remove_spot(w, m, id);
            }
            if let Some(o) = w.objects.get_mut(id) {
                o.state = engine_state::INIT;
            }
            return;
        }
        _ => return,
    }
    let spot = match data_mut(w, id) {
        Some(SoundPart::Spot(s)) => s.clone(),
        _ => SpotData {
            key: sound_key(w, id),
            manager: None,
        },
    };
    let manager = manager(w, spot.manager);
    if let Some(o) = w.objects.get_mut(id) {
        o.flags |= flags::NOTIFY_SLEEP;
        o.flags &= !flags::ALWAYS_ACTIVE;
        o.state = trigger_state::ARMED;
    }
    quiet(w, id);
    if let Some(key) = spot.key.clone()
        && let Some(m) = manager_mut(w, manager)
    {
        m.entries.retain(|e| e.owner != Some(id));
        m.entries.push(AmbientEntry::new(
            key,
            Some(id),
            AmbientKind::Spot { x, y },
            percent,
        ));
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Sound(SoundData {
            part: SoundPart::Spot(SpotData {
                manager: Some(manager),
                ..spot
            }),
        });
    }
}

/// Removes the spot loop of `owner` and stops its voice when no other loop
/// wants it.
fn remove_spot(w: &mut World, manager: ObjectId, owner: ObjectId) {
    let mut cues = Vec::new();
    if let Some(m) = manager_mut(w, manager) {
        m.entries.retain(|e| e.owner != Some(owner));
        m.sync(&mut cues);
    }
    for c in cues {
        w.cue(c);
    }
}

fn manager_tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let mut data = std::mem::take(&mut o.data);
    if let ObjectData::Sound(SoundData {
        part: SoundPart::Manager(m),
    }) = &mut data
    {
        let dying = sound::claw_dying(w, false);
        if m.claw_dying && !dying {
            m.attempt = m.attempt.wrapping_add(1);
        }
        m.claw_dying = dying;
        let objects = &*w.objects;
        m.entries
            .retain(|e| e.owner.is_none_or(|o| objects.get(o).is_some()));
        let listener = sound::view_centre(w.view);
        let now = w.now_ms;
        for e in &mut m.entries {
            e.update(listener, now, w.rng);
        }
        for e in &mut m.entries {
            e.run_fade(now);
        }
        let mut cues = Vec::new();
        m.sync(&mut cues);
        for c in cues {
            w.cue(c);
        }
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = data;
    }
}

/// Strictly inside: points on the edges are outside.
fn strictly_inside(r: Rect, (x, y): (i32, i32)) -> bool {
    r.is_set() && r.left < x && x < r.right && r.top < y && y < r.bottom
}

impl AmbientEntry {
    /// A stopped loop.
    pub fn new(key: String, owner: Option<ObjectId>, kind: AmbientKind, percent: i32) -> Self {
        AmbientEntry {
            key,
            owner,
            kind,
            percent,
            playing: false,
            volume: 0,
            pan: 0,
            fade: None,
        }
    }

    fn fading_out(&self) -> bool {
        self.fade.is_some_and(|f| f.stop)
    }

    fn start_now(&mut self, volume: i32) {
        self.playing = true;
        self.volume = volume;
        self.fade = None;
    }

    fn fade_in(&mut self, to: i32, now: u64) {
        if !self.playing {
            self.playing = true;
            self.volume = to.min(1);
        }
        self.fade = Some(Fade {
            from: self.volume,
            to,
            start_ms: now,
            stop: false,
        });
    }

    fn fade_out(&mut self, now: u64) {
        if self.playing && !self.fading_out() {
            self.fade = Some(Fade {
                from: self.volume,
                to: 0,
                start_ms: now,
                stop: true,
            });
        }
    }

    /// One update with the listener at `listener`.
    fn update(&mut self, listener: (i32, i32), now: u64, rng: &mut Rng) {
        let full = ambient_volume(100, self.percent);
        match self.kind {
            AmbientKind::Area { area, extra } => {
                if !area.is_set() {
                    if !self.playing {
                        self.start_now(full);
                    }
                    return;
                }
                let inside = strictly_inside(area, listener) || strictly_inside(extra, listener);
                if inside && !self.playing {
                    self.fade_in(full, now);
                } else if !inside {
                    self.fade_out(now);
                }
            }
            AmbientKind::Timed {
                area,
                extra,
                on,
                off,
                mut on_phase,
                mut timer_ms,
            } => {
                let inside = !area.is_set()
                    || strictly_inside(area, listener)
                    || strictly_inside(extra, listener);
                if !inside {
                    on_phase = false;
                    self.fade_out(now);
                } else {
                    timer_ms = timer_ms.saturating_sub(TICK_MS as i32);
                    if timer_ms <= 0 {
                        on_phase = !on_phase;
                        if on_phase {
                            timer_ms = rand_range(rng, on.0, on.1);
                            self.fade_in(full, now);
                        } else {
                            timer_ms = rand_range(rng, off.0, off.1);
                            self.fade_out(now);
                        }
                    }
                }
                self.kind = AmbientKind::Timed {
                    area,
                    extra,
                    on,
                    off,
                    on_phase,
                    timer_ms,
                };
            }
            AmbientKind::Spot { x, y } => match spot_level_pan(x, y, listener.0, listener.1) {
                Some((level, pan)) => {
                    self.start_now(ambient_volume(level, self.percent));
                    self.pan = pan;
                }
                None => {
                    self.playing = false;
                    self.fade = None;
                }
            },
        }
    }

    /// Moves a fade on to `now`; a finished fade-out stops the loop.
    fn run_fade(&mut self, now: u64) {
        let Some(f) = self.fade else {
            return;
        };
        let elapsed = now.saturating_sub(f.start_ms);
        self.volume = fade_volume(f.from, f.to, elapsed);
        if elapsed >= FADE_MS {
            self.fade = None;
            if f.stop {
                self.playing = false;
            }
        }
    }
}

impl AmbientManager {
    /// Brings every key's voice in line with its loops: the loudest
    /// playing loop of a key gives the voice its volume and balance; a key
    /// with no playing loop stops its voice. Changes go to `cues`.
    pub fn sync(&mut self, cues: &mut Vec<AudioCue>) {
        let mut want: BTreeMap<&str, (i32, i32)> = BTreeMap::new();
        for e in self.entries.iter().filter(|e| e.playing) {
            want.entry(&e.key)
                .and_modify(|w| {
                    if e.volume > w.0 {
                        *w = (e.volume, e.pan);
                    }
                })
                .or_insert((e.volume, e.pan));
        }
        let keys: BTreeSet<String> = self
            .voices
            .keys()
            .cloned()
            .chain(want.keys().map(|k| k.to_string()))
            .collect();
        for key in keys {
            match want.get(key.as_str()) {
                Some(&(volume, pan)) => {
                    let volume = volume.clamp(0, 100) as u8;
                    let pan = pan.clamp(-100, 100) as i8;
                    let next = &mut self.next_voice;
                    let v = self.voices.entry(key.clone()).or_insert_with(|| {
                        let handle = AMBIENT_VOICE_BASE.wrapping_add(*next);
                        *next = next.wrapping_add(1);
                        KeyVoice {
                            handle,
                            ..KeyVoice::default()
                        }
                    });
                    if v.playing && v.volume == volume && v.pan == pan {
                        continue;
                    }
                    v.playing = true;
                    v.volume = volume;
                    v.pan = pan;
                    cues.push(AudioCue::Play(SoundCue {
                        key,
                        volume,
                        pan,
                        pitch: 0,
                        looped: true,
                        channel: SoundChannel::Ambient,
                        voice: Some(v.handle),
                    }));
                }
                None => {
                    if let Some(v) = self.voices.get_mut(&key)
                        && v.playing
                    {
                        v.playing = false;
                        cues.push(AudioCue::Stop { voice: v.handle });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rand_range_matches_the_game() {
        let mut a = Rng::new(5);
        let mut b = Rng::new(5);
        for _ in 0..200 {
            let r = rand_range(&mut a, 30_000, 60_000);
            assert_eq!(r, 30_000 + (b.next_u15() as i32) % 30_001);
        }
        let mut a = Rng::new(9);
        let mut b = Rng::new(9);
        let r = rand_range(&mut a, 5, 4);
        assert_eq!(r, if b.next_u15() & 1 != 0 { 5 } else { 4 });
        assert_eq!(rand_range(&mut a, 7, 7), 7);
    }

    #[test]
    fn fades_are_linear_over_a_second() {
        assert_eq!(fade_volume(1, 100, 0), 1);
        assert_eq!(fade_volume(1, 100, 18), 2);
        assert_eq!(fade_volume(1, 100, 500), 50);
        assert_eq!(fade_volume(1, 100, 999), 99);
        assert_eq!(fade_volume(1, 100, 1000), 100);
        assert_eq!(fade_volume(60, 0, 500), 30);
        assert_eq!(fade_volume(60, 0, 5000), 0);
    }
}
