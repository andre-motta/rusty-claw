//! The context every level logic runs in: the objects, Claw, the level,
//! the random streams, tuning, the clock, the view and the outbox of the
//! tick.
//!
//! A family adds its own helpers in an `impl World` block in its own file;
//! this file holds what every family shares.

use claw_frame::{AudioCue, GameEvent, PaletteFade, RemovedBody, ScreenTransition, SoundChannel};

use crate::anim::{Animation, FRAME_SOUND_IN_VIEW, FRAME_SOUND_POSITIONAL};
use crate::container::Container;
use crate::level::SimLevel;
use crate::logic::enemy::aquatis::AquatisLink;
use crate::logic::enemy::chameleon::PhaseFrame;
use crate::logic::enemy::marrow::MarrowLink;
use crate::logic::enemy::redtail::RedTailLink;
use crate::logic::sound;
use crate::logic::world_boss::BossState;
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags};
use crate::physics::{self, Collider, Platform};
use crate::player::Claw;
use crate::rng::Rng;
use crate::tuning::{PlayAids, TICK_MS, Tuning};

/// A line Claw's code asks him to say (see [`crate::player::Ctx::say`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceRequest {
    /// Sound key of the line.
    pub key: String,
    /// Say it even while another of Claw's lines runs or he is dying.
    pub force: bool,
}

/// What a tick hands to the presentation, gathered while it runs, and the
/// requests the simulation carries out for the code that made them.
///
/// Sounds and events belong to one tick: the simulation moves them into
/// the tick's snapshot. The palette fade and the screen transition are
/// states: they stay until a logic changes them. A screen shake request is
/// applied to the camera at its next call, or at the end of the tick when
/// the camera has already run; a voice request made by Claw's code is
/// carried out as soon as his handler returns, and one still waiting at
/// the end of the tick is carried out then, so none outlives its tick.
///
/// The boss fight state ([`BossState`], see [`super::world_boss`]) is a
/// state too: it lasts the whole level and starts cleared with it. So are
/// the Marrow link ([`MarrowLink`], see [`World::marrow_link`]), the
/// scream lockout ([`World::scream_lock_ms`]), the fish's flop toggle
/// ([`World::fish_flop`]), the Aquatis link ([`World::aquatis_link`]),
/// the RedTail link ([`World::redtail_link`]) and the Chameleons' shared
/// phase frame ([`World::chameleon_frame`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outbox {
    audio: Vec<AudioCue>,
    events: Vec<GameEvent>,
    fade: Option<PaletteFade>,
    transition: Option<ScreenTransition>,
    spawned: Vec<ObjectId>,
    shake_ms: Option<u32>,
    voice: Vec<VoiceRequest>,
    frame_time_reset: bool,
    tile_edits: Vec<(i32, i32, i32)>,
    pub(crate) boss: BossState,
    pub(crate) marrow: Option<MarrowLink>,
    /// The scream lockout, ms (see
    /// [`crate::logic::projectile::scream`]).
    pub(crate) scream_lock_ms: u32,
    /// The fish's flop toggle reads 0 (see
    /// [`crate::logic::enemy::fish`]); it starts at 1.
    pub(crate) fish_flop_zero: bool,
    /// What Aquatis's helpers and tentacles share with him (see
    /// [`crate::logic::enemy::aquatis`]).
    pub(crate) aquatis: AquatisLink,
    /// What RedTail's helpers share with him (see
    /// [`crate::logic::enemy::redtail`]).
    pub(crate) redtail: RedTailLink,
    /// How the Chameleons' shared phase frame is drawn (see
    /// [`crate::logic::enemy::chameleon`]).
    pub(crate) chameleon: PhaseFrame,
    falling: Vec<ObjectId>,
    removed_bodies: Vec<RemovedBody>,
}

impl Outbox {
    /// An empty outbox.
    pub fn new() -> Outbox {
        Outbox::default()
    }

    /// Adds a sound or music cue.
    pub fn cue(&mut self, cue: AudioCue) {
        self.audio.push(cue);
    }

    /// Adds a game event.
    pub fn event(&mut self, event: GameEvent) {
        self.events.push(event);
    }

    /// Restarts the frame clock, as saving the game does: the rest of the
    /// tick's work after its logic (the ambient sounds' timers) counts no
    /// time.
    pub fn reset_frame_time(&mut self) {
        self.frame_time_reset = true;
    }

    /// Starts or ends a palette fade.
    pub fn set_fade(&mut self, fade: Option<PaletteFade>) {
        self.fade = fade;
    }

    /// Starts or ends a screen transition.
    pub fn set_transition(&mut self, transition: Option<ScreenTransition>) {
        self.transition = transition;
    }

    /// Asks for a screen shake of `ms` milliseconds from the current tick,
    /// as an explosion does (500 ms). Several requests in one tick keep the
    /// longest, and a shake already running longer is kept.
    pub fn shake(&mut self, ms: u32) {
        self.shake_ms = Some(self.shake_ms.map_or(ms, |m| m.max(ms)));
    }

    /// The screen shake asked for and not applied yet, ms.
    pub fn pending_shake(&self) -> Option<u32> {
        self.shake_ms
    }

    /// Asks for Claw to say the line `key` (see
    /// [`crate::logic::sound::claw_say`] for when a line is refused). Only
    /// Claw's own code asks, through [`crate::player::Ctx::say`], since it
    /// runs without a [`World`]; level logics call `claw_say` directly. The
    /// simulation passes the request to `claw_say` as soon as his handler
    /// returns, in request order, and passes any request still waiting at
    /// the end of the tick then.
    pub(crate) fn say(&mut self, key: &str, force: bool) {
        self.voice.push(VoiceRequest {
            key: key.to_string(),
            force,
        });
    }

    /// The voice requests not carried out yet.
    pub fn voice_requests(&self) -> &[VoiceRequest] {
        &self.voice
    }

    /// Cues gathered so far in this tick.
    pub fn audio(&self) -> &[AudioCue] {
        &self.audio
    }

    /// Events gathered so far in this tick.
    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    /// The palette fade in progress.
    pub fn fade(&self) -> Option<PaletteFade> {
        self.fade
    }

    /// The screen transition in progress.
    pub fn transition(&self) -> Option<ScreenTransition> {
        self.transition
    }

    /// The boss fight state.
    pub fn boss(&self) -> &BossState {
        &self.boss
    }

    /// The Marrow link, while Marrow's object exists (see
    /// [`World::marrow_link`]).
    pub fn marrow(&self) -> Option<&MarrowLink> {
        self.marrow.as_ref()
    }

    /// The Aquatis link (see [`World::aquatis_link`]).
    pub fn aquatis(&self) -> &AquatisLink {
        &self.aquatis
    }

    /// The RedTail link (see [`World::redtail_link`]).
    pub fn redtail(&self) -> &RedTailLink {
        &self.redtail
    }

    /// How the Chameleons' shared phase frame is drawn (see
    /// [`World::chameleon_frame`]).
    pub fn chameleon_frame(&self) -> PhaseFrame {
        self.chameleon
    }

    /// The scream lockout, ms: 0 while Claw's controls are free (see
    /// [`World::scream_lock_ms`]).
    pub fn scream_lock_ms(&self) -> u32 {
        self.scream_lock_ms
    }

    /// The frame clock was restarted in this tick (a save, see
    /// [`World::reset_frame_time`]): what runs after the tick's logic counts
    /// no time.
    pub fn frame_time_reset(&self) -> bool {
        self.frame_time_reset
    }

    /// The time the work after the tick's logic counts for this tick, ms:
    /// one tick, or none after [`World::reset_frame_time`].
    pub fn frame_ms(&self) -> u32 {
        if self.frame_time_reset { 0 } else { TICK_MS }
    }

    /// Takes the tick's cues and events, leaving the fade and transition;
    /// the frame clock counts again from the next tick.
    pub(crate) fn take_tick(&mut self) -> (Vec<AudioCue>, Vec<GameEvent>) {
        self.frame_time_reset = false;
        (
            std::mem::take(&mut self.audio),
            std::mem::take(&mut self.events),
        )
    }

    /// Marks `id`, which is being removed, as a falling body a wider
    /// presentation draws on (see [`World::remove_falling_body`]).
    pub(crate) fn falling(&mut self, id: ObjectId) {
        self.falling.push(id);
    }

    /// True when `id` was marked by [`Outbox::falling`]; forgets the mark.
    pub(crate) fn take_falling(&mut self, id: ObjectId) -> bool {
        match self.falling.iter().position(|&f| f == id) {
            Some(i) => {
                self.falling.swap_remove(i);
                true
            }
            None => false,
        }
    }

    /// Forgets every falling mark.
    pub(crate) fn clear_falling(&mut self) {
        self.falling.clear();
    }

    /// Adds a removed body for the presentation.
    pub(crate) fn removed_body(&mut self, body: RemovedBody) {
        self.removed_bodies.push(body);
    }

    /// Takes the removed bodies gathered since the last call.
    pub(crate) fn take_removed_bodies(&mut self) -> Vec<RemovedBody> {
        std::mem::take(&mut self.removed_bodies)
    }

    /// Takes the objects spawned since the last call, in spawn order.
    pub(crate) fn take_spawned(&mut self) -> Vec<ObjectId> {
        std::mem::take(&mut self.spawned)
    }

    /// True when objects wait to be listed.
    pub(crate) fn has_spawned(&self) -> bool {
        !self.spawned.is_empty()
    }

    /// Takes the screen shake asked for.
    pub(crate) fn take_shake(&mut self) -> Option<u32> {
        self.shake_ms.take()
    }

    /// Takes the voice requests, in request order.
    pub(crate) fn take_voice(&mut self) -> Vec<VoiceRequest> {
        std::mem::take(&mut self.voice)
    }

    /// Takes the main-plane cell writes (column, row, tile), in request
    /// order (see [`World::set_main_cell`]).
    pub(crate) fn take_tile_edits(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.tile_edits)
    }
}

/// The context of one logic call.
#[derive(Debug)]
pub struct World<'a> {
    /// Every object of the level. Logics add and remove objects only
    /// through [`World::spawn`] and [`World::destroy`]; calling the
    /// container's own add, remove, insert or unlink from a logic breaks
    /// the update pass's walk over the list.
    pub objects: &'a mut Container,
    /// Claw's object.
    pub claw_id: Option<ObjectId>,
    /// Claw's data.
    pub claw: &'a mut Claw,
    /// The level.
    pub level: &'a SimLevel,
    /// The game's random stream.
    pub rng: &'a mut Rng,
    /// The sound-only random stream, seeded when the level starts: choices
    /// that only change what is heard (which of an animation element's
    /// sounds plays) draw from it, so they never change the game.
    pub sound_rng: &'a mut Rng,
    /// Tuning data.
    pub tuning: &'a Tuning,
    /// Number of the tick being run.
    pub tick: u64,
    /// Simulation time at the start of the tick, ms.
    pub now_ms: u64,
    /// The frame time of the call, ms: one tick ([`TICK_MS`]), or 0 in the
    /// two update passes of a respawn and in Claw's call that settles a
    /// warp, which count no time (animations stand still there).
    pub dt_ms: u32,
    /// The tick is a frame pulse tick (every third tick, see
    /// [`crate::sim::PULSE_MS`]): enemies make their per-tick roll and a
    /// few random choices only then.
    pub pulse: bool,
    /// The logic view in level pixels: what the player sees.
    pub view: Rect,
    /// Sounds, events and screen effects of the tick.
    pub out: &'a mut Outbox,
    /// The play-test aids ([`crate::sim::SimConfig::aids`]).
    pub aids: PlayAids,
    /// The call is an object's first, made in the set-up pass while the
    /// level loads, before time zero.
    pub set_up: bool,
    /// The help screen's time, ms, in the update pass of the return from
    /// it (see [`crate::Sim::pass_help_return`]); 0 in every other call, and
    /// in the calls Claw's own logic makes.
    pub help_ms: u32,
}

impl World<'_> {
    /// Adds `obj` to the level. It exists at once (the id is valid and
    /// [`Container::get`] finds it) and joins the live list at the end of
    /// the current pass, after the objects spawned before it, so it gets
    /// its first logic call in the next update pass (or right away when it
    /// has [`flags::RUN_AT_CREATION`]). An object with [`flags::UNLISTED`]
    /// never joins the list.
    pub fn spawn(&mut self, obj: Object) -> ObjectId {
        let id = self.objects.add(obj);
        self.out.spawned.push(id);
        id
    }

    /// Asks for `id` to be destroyed at the end of the next update pass
    /// that reaches it (or, for an object spawned in this pass, before it
    /// is listed).
    pub fn destroy(&mut self, id: ObjectId) {
        if let Some(o) = self.objects.get_mut(id) {
            o.flags |= flags::DESTROY;
        }
    }

    /// Removes `id` as [`World::destroy`] does, as a body killed by a hit
    /// that fell out of the view. For the game this is exactly a removal;
    /// in addition, the tick's snapshot reports the body as it was at the
    /// end of its last logic call ([`claw_frame::RemovedBody`]), so that a
    /// presentation wider than the logic view can draw its fall on.
    pub fn remove_falling_body(&mut self, id: ObjectId) {
        if self.objects.get(id).is_some() {
            self.destroy(id);
            self.out.falling(id);
        }
    }

    /// Adds a sound or music cue to the tick.
    pub fn cue(&mut self, cue: AudioCue) {
        self.out.cue(cue);
    }

    /// Adds a game event to the tick.
    pub fn event(&mut self, event: GameEvent) {
        self.out.event(event);
    }

    /// Restarts the frame clock (see [`Outbox::reset_frame_time`]).
    pub fn reset_frame_time(&mut self) {
        self.out.reset_frame_time();
    }

    /// Starts the animation registered under `key` on `id`; false when the
    /// level has no such animation or no such object.
    pub fn set_anim(&mut self, id: ObjectId, key: &str) -> bool {
        let Some(anim) = self.level.animations.get(key) else {
            return false;
        };
        let Some(o) = self.objects.get_mut(id) else {
            return false;
        };
        o.anim.set(anim.clone());
        true
    }

    /// Advances the animation of `id` by the call's frame time
    /// ([`World::dt_ms`]): sets its frame (within the frame range of its
    /// image set, when the level knows it), moves it as the element says
    /// and plays the element's sounds. Returns the event value (-1 without
    /// an animation or object).
    pub fn advance_anim(&mut self, id: ObjectId) -> i32 {
        let Some(o) = self.objects.get_mut(id) else {
            return -1;
        };
        let mirrored = o.draw_flags & draw_flags::MIRROR != 0;
        let frames = self.level.image_frames(&o.image_set);
        let e = o.anim.advance_in(self.dt_ms, o.frame, mirrored, frames);
        if let Some(f) = e.frame {
            o.frame = f;
        }
        if let Some((dx, dy)) = e.moved_by {
            o.x += dx;
            o.y += dy;
        }
        if let Some((x, y)) = e.moved_to {
            o.x = x;
            o.y = y;
        }
        if let Some(index) = e.applied
            && let Some(anim) = o.anim.animation().cloned()
        {
            self.frame_sound(id, &anim, index);
        }
        e.event
    }

    /// Plays the sound of element `index` of `anim` for its owner `id`:
    /// one of the element's sounds, chosen with the sound-only random
    /// stream when it has several (a name without a registered sound plays
    /// nothing). It is placed at the owner when the element asks for that,
    /// and skipped while the owner is off screen when the element or the
    /// owner asks for that (see [`World::on_screen`]).
    pub fn frame_sound(&mut self, id: ObjectId, anim: &Animation, index: usize) {
        let sounds = anim.sounds_of(index);
        if sounds.is_empty() {
            return;
        }
        let pick = if sounds.len() > 1 {
            self.sound_rng.below(sounds.len() as u32) as usize
        } else {
            0
        };
        let Some(key) = sounds[pick].as_deref() else {
            return;
        };
        let element = anim.frames.get(index).map_or(0, |f| f.flags);
        let Some(o) = self.objects.get(id) else {
            return;
        };
        let in_view_only =
            o.flags & flags::SOUND_IN_VIEW != 0 || element & FRAME_SOUND_IN_VIEW != 0;
        if in_view_only && !self.on_screen(o) {
            return;
        }
        if element & FRAME_SOUND_POSITIONAL != 0 {
            sound::play_from(self, id, key, SoundChannel::Effect);
        } else {
            sound::play(self, key, SoundChannel::Effect);
        }
    }

    /// Moves `id` toward (tx, ty) through the tile physics and the platform
    /// objects (see [`crate::physics::physics`]); returns the result mask,
    /// 0 when there is no such object.
    pub fn physics(&mut self, id: ObjectId, tx: i32, ty: i32, mask: u32) -> u32 {
        let platforms: Vec<Platform> = crate::sim::platforms(self.objects)
            .into_iter()
            .filter(|p| p.id != id)
            .collect();
        let Some(o) = self.objects.get_mut(id) else {
            return 0;
        };
        let c = Collider {
            map: &self.level.map,
            platforms: &platforms,
            step: self.tuning.physics.step,
        };
        physics::physics(&c, o, tx, ty, mask)
    }

    /// Length of the sound registered under `key`, ms.
    pub fn sound_ms(&self, key: &str) -> Option<u32> {
        self.level.sound_ms(key)
    }

    /// Claw's object.
    pub fn claw_object(&self) -> Option<&Object> {
        self.claw_id.and_then(|id| self.objects.get(id))
    }

    /// Writes `tile` (-1 for none) into the main-plane cell (`col`, `row`),
    /// as the breaking planks do when they give way and when their row is
    /// restored. The write reaches the level as soon as the current logic
    /// call returns, so every later mover of the tick (Claw among them)
    /// meets the changed cell; [`World::main_cell`] reads it back at once.
    /// Cells outside the plane are left alone.
    pub fn set_main_cell(&mut self, col: i32, row: i32, tile: i32) {
        self.out.tile_edits.push((col, row, tile));
    }

    /// The tile number of the main-plane cell (`col`, `row`), with the
    /// writes of the current call (see [`World::set_main_cell`]); -1 for an
    /// empty cell or one outside the plane.
    pub fn main_cell(&self, col: i32, row: i32) -> i32 {
        self.out
            .tile_edits
            .iter()
            .rev()
            .find(|e| (e.0, e.1) == (col, row))
            .map_or_else(|| self.level.map.cell(col, row), |e| e.2)
    }

    /// The level's retail number (see [`SimLevel::number`]).
    pub fn level_number(&self) -> u32 {
        self.level.number
    }

    /// Claw's deaths so far in this level (see
    /// [`crate::player::stats::PlayerStats::attempt`]).
    pub fn attempt(&self) -> u32 {
        self.claw.stats.attempt
    }

    /// First and last frame numbers of the image set `set`, when the level
    /// knows the set.
    pub fn image_frames(&self, set: &str) -> Option<(i32, i32)> {
        self.level.image_frames(set)
    }

    /// Box of frame `frame` of the image set `set` relative to its
    /// object's position (see [`crate::level::frame_box`]), when known.
    pub fn frame_box(&self, set: &str, frame: i32) -> Option<Rect> {
        self.level.frame_box(set, frame)
    }

    /// The box `o` is drawn in, in level pixels (see
    /// [`SimLevel::draw_box`]); `None` when the level does not know the
    /// frame.
    pub fn draw_box(&self, o: &Object) -> Option<Rect> {
        self.level.draw_box(o)
    }

    /// True while `o` is on screen: its draw box overlaps the logic view,
    /// or, when the level does not know its frame, its position is inside
    /// the view.
    pub fn on_screen(&self, o: &Object) -> bool {
        self.level.in_view(o, &self.view)
    }

    /// Asks for a screen shake of `ms` milliseconds (see
    /// [`Outbox::shake`]).
    pub fn shake(&mut self, ms: u32) {
        self.out.shake(ms);
    }

    /// Kills `id` with a kill of kind `kind`: the object gets
    /// [`engine_state::KILLED`] on its next call, with `kind` in
    /// [`Object::kill_kind`], and its logic decides what that kind of kill
    /// does (a powder keg explodes on kind 2). False when there is no such
    /// object.
    ///
    /// The kill takes effect at the object's next logic call, not during
    /// this one: an object that has already run in the current update pass
    /// reacts in the next tick, up to one tick later than an immediate
    /// kill would.
    pub fn kill(&mut self, id: ObjectId, kind: i32) -> bool {
        let Some(o) = self.objects.get_mut(id) else {
            return false;
        };
        o.state = engine_state::KILLED;
        o.kill_kind = kind;
        true
    }
}
