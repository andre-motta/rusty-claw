//! Captain Claw: the player state machine.
//!
//! Claw is one object whose logic runs once per tick. A numeric state id
//! selects a row of the [state table](STATE_TABLE); the row's `tick`
//! handler turns input into a velocity, integrates it (see
//! [`crate::kinematics`]), asks the tile physics to move Claw to the
//! requested whole-pixel position, and switches state from the resulting
//! physics type: 4 means nothing is under the feet, 6 means a ladder was
//! grabbed, 1 means Claw stands. After every handler a shared post-tick
//! step runs (the weapon toggle and its click, timers, the position
//! clamp, death-tile marks).
//!
//! Every state id of the game is declared in the table. Ids without
//! behaviour in the game run only the post-tick step. States whose
//! behaviour is not filled in yet (hits, deaths, lifting, throwing,
//! freezing, swinging, being held) are flagged fallback rows that return
//! Claw to standing.
//!
//! The parts that grow with combat and progress live in submodules, each
//! with its own data in [`Claw`]:
//!
//! - [`stats`]: health, score, lives, ammunition, weapon, powerup and
//!   treasure counts, and what carries between levels;
//! - [`attack`]: choosing and running attacks, called by the host states
//!   in the game's order (Jump, Fall and Crouch before their update, Walk
//!   after it, Stand before its update);
//! - [`special`]: the hit reaction, lifting, throwing and freezing states;
//! - [`hit`]: what happens when something touches Claw;
//! - [`death`]: dying, the death states and the powerup timer;
//! - [`powerup_aid`]: the play-test aid that gives any powerup;
//! - [`progress`]: starting a level from a carried state or a save, and
//!   handing the progress on;
//! - [`swing`]: hanging from a rope;
//! - [`hug`]: held in a bear sailor's hug;
//! - [`bounce`]: the throw a springboard gives him.
//!
//! The rows of the hit (31), lift (5000), throw (5001), death-fall (5004),
//! death (5005), swing (5007), bear hug (5008) and freeze (5009) states
//! call into those submodules, which fall back to standing until they are
//! filled in (or, once filled in, when Claw is found in the state without
//! what it needs: a swing with no rope).

use std::collections::HashMap;
use std::sync::Arc;

use claw_frame::{AudioCue, Buttons, SoundCue, TickInput};

use crate::anim::Animation;
use crate::container::Container;
use crate::kinematics::Mover;
use crate::level::SimLevel;
use crate::logic::world::{Outbox, World};
use crate::object::{Object, ObjectId, Rect, draw_flags, flags, types};
use crate::physics::{self, Collider, Platform, opt, ptype};
use crate::rng::Rng;
use crate::tiles::{CLIMB, DEATH};
use crate::tuning::{PlayAids, Tuning};

pub mod attack;
pub mod bounce;
pub mod death;
pub mod hit;
pub mod hug;
pub mod look;
pub mod powerup_aid;
pub mod progress;
pub mod special;
pub mod stats;
pub mod swing;

pub use bounce::bounce;
pub use stats::{Carry, PlayerStats, SavePos, StartState};

/// Animations by registry key.
pub type AnimSet = HashMap<String, Arc<Animation>>;

/// State ids. The numbers are fixed: recorded traces use them.
pub mod state {
    /// One-shot set-up at level start.
    pub const INIT: i32 = 0;
    /// Rising after a jump.
    pub const JUMP: i32 = 4;
    /// Walking or running right.
    pub const MOVE_RIGHT: i32 = 8;
    /// Walking or running left.
    pub const MOVE_LEFT: i32 = 9;
    /// One-shot placement after a death or restart.
    pub const RESPAWN: i32 = 23;
    /// Standing.
    pub const STAND: i32 = 24;
    /// Falling.
    pub const FALL: i32 = 25;
    /// Crouching.
    pub const CROUCH: i32 = 26;
    /// Climbing a ladder or rope.
    pub const CLIMB: i32 = 27;
    /// Hit reaction.
    pub const HIT: i32 = 31;
    /// Lifting an object.
    pub const LIFT: i32 = 5000;
    /// Throwing a lifted object.
    pub const THROW: i32 = 5001;
    /// Landing after a long fall.
    pub const LAND: i32 = 5002;
    /// Looking up.
    pub const LOOK_UP: i32 = 5003;
    /// Falling to death.
    pub const DEATH_FALL: i32 = 5004;
    /// Death on a hazard.
    pub const DEATH: i32 = 5005;
    /// Teetering on a ledge.
    pub const ON_EDGE: i32 = 5006;
    /// Swinging on a rope.
    pub const SWING: i32 = 5007;
    /// Held by an enemy.
    pub const BEARHUG: i32 = 5008;
    /// Frozen.
    pub const FREEZE: i32 = 5009;
}

/// Image set Claw is drawn from.
pub const IMAGE_SET: &str = "CLAW";

/// Voice line Claw may say when he starts teetering on a ledge.
pub const TEETER_VOICE: &str = "CLAW_1021";
/// Sound of Claw's feet touching the ground after a fall.
pub const LANDING_SOUND: &str = "CLAW_LEFTFOOT1";
/// Sound of the flag when Claw comes back at a checkpoint.
pub const CHECKPOINT_RESPAWN_SOUND: &str = "GAME_FLAGWAVE";

/// Catnip's walking speed against the plain walk: 300 px/s where Claw
/// walks at 225 px/s in the original's own numbers. It scales the tuned
/// walking speed, so [`Tuning::original_speeds`] walks at exactly 300.
pub const CATNIP_WALK_FACTOR: f64 = 300.0 / 225.0;
/// Catnip's climbing speed against the plain climb: 250 px/s where Claw
/// climbs at 200 px/s in the original's own numbers. It scales both tuned
/// climbing speeds.
pub const CATNIP_CLIMB_FACTOR: f64 = 250.0 / 200.0;
/// Animation speed of a plain walk.
pub const WALK_ANIM_SPEED: f32 = 1.0;
/// Animation speed of a run.
pub const RUN_ANIM_SPEED: f32 = 0.9;
/// Animation speed of a walk with catnip.
pub const CATNIP_ANIM_SPEED: f32 = 0.8;
/// Vertical speed Claw records while Jump is held in a plain jump, px/s.
/// The rise itself is aimed at the jump's target height and does not use
/// it.
pub const JUMP_HOLD_SPEED: i32 = -425;
/// Vertical speed recorded while Jump is held in a running jump or with
/// catnip, px/s.
pub const JUMP_HOLD_SPEED_FAST: i32 = -500;
/// Vertical speed recorded while Jump is held with the super jump switch,
/// px/s.
pub const JUMP_HOLD_SPEED_SUPER: i32 = -750;

/// Game-wide switches that change Claw's movement. All are off in normal
/// play; the cheat codes that turn them on are not wired yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MovementOverrides {
    /// Jumps rise as high as with catnip (a full hold reaches 195 px),
    /// catnip or not.
    pub super_jump: bool,
}

/// How far a state table row is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// Implemented in this milestone.
    Implemented,
    /// Belongs to a later milestone: the handler returns Claw to standing
    /// (or, for a hit ending in mid-air with the mid-air hit jump fix on,
    /// landing, see [`crate::tuning::Behaviour::fix_double_jump`]) and the
    /// entry is counted.
    Fallback,
    /// The game has no behaviour for this id: only the post-tick step runs.
    Unused,
}

/// Tick handler of a row.
pub type TickFn = fn(&mut Ctx);
/// Entry handler of a row: called with the previous state id.
pub type EnterFn = fn(&mut Ctx, i32);
/// Exit handler of a row: called with the next state id.
pub type ExitFn = fn(&mut Ctx, i32);

/// One row of the state table.
#[derive(Debug, Clone, Copy)]
pub struct StateRow {
    /// State id.
    pub id: i32,
    /// Name.
    pub name: &'static str,
    /// Implementation status.
    pub kind: RowKind,
    /// Animation keys the state plays.
    pub animations: &'static [&'static str],
    /// Physics type the state sets, when it sets one.
    pub physics_type: Option<i32>,
    /// Entry.
    pub enter: EnterFn,
    /// Per-tick handler.
    pub tick: TickFn,
    /// Exit.
    pub exit: ExitFn,
}

/// Captain Claw's own data.
#[derive(Debug, Clone, PartialEq)]
pub struct Claw {
    /// Facing right.
    pub facing_right: bool,
    /// An attack is running.
    pub attacking: bool,
    /// Health, score, lives, ammunition, weapon, powerup and treasures.
    pub stats: PlayerStats,
    /// The running attack.
    pub attack: attack::AttackData,
    /// Lifting, throwing, freezing and the hit reaction.
    pub special: special::SpecialData,
    /// Pixels fallen since the fall began.
    pub fall_height: i32,
    /// Position after the level-start placement; the respawn fallback.
    pub spawn_point: (i32, i32),
    /// Checkpoint position, when one was reached.
    pub checkpoint: Option<(i32, i32)>,
    /// Pixels risen during the current jump.
    pub jump_height: i32,
    /// Y where the jump started.
    pub jump_start_y: i32,
    /// Y the jump rises to.
    pub jump_peak_y: i32,
    /// Y of the ladder top found while climbing, 0 when unknown.
    pub climb_peak_y: i32,
    /// Climb animation selector: 8 up, 2 down, 5 idle.
    pub climb_dir: i32,
    /// Conveyor drift added to the horizontal speed, px/s.
    pub conveyor_force: f64,
    /// Blocks looking up after a climb ends on the ground, ms.
    pub post_climb_lockout_ms: i32,
    /// Up held time left before looking up, ms.
    pub look_up_time: i32,
    /// Time since the jump started, ms.
    pub jump_time: i32,
    /// Time left before the camera pans for a crouch or look-up, ms.
    pub look_down_time: i32,
    /// Walking time left before running, ms.
    pub running_time: i32,
    /// Hold window left for the variable jump, ms.
    pub jump_press_time: f64,
    /// Idle time left before the idle animation, ms.
    pub afk_time: i32,
    /// Second timer counted down by the post-tick step, ms.
    pub aux_timer_ms: i32,
    /// Position clamp: left, top, right, bottom.
    pub clamp: Rect,
    /// Floating-point motion.
    pub mover: Mover,
    /// Death-tile contacts reported (dying is not filled in yet).
    pub death_tile_hits: u32,
    /// Times a fallback row was entered.
    pub fallback_entries: u32,
    /// Game-wide movement switches.
    pub movement: MovementOverrides,
    /// An attack ended or was cancelled (every change of state cancels
    /// it): Left and Right leave the key latch, so on the next tick a
    /// direction still held counts as pressed again (see
    /// [`crate::Sim::step`]).
    pub repress_directions: bool,
    /// The lives step of a death with lives left has run: Claw respawns
    /// between this tick and the next (see [`death`]).
    pub respawn_pending: bool,
    /// The rope Claw grabbed last (see [`swing`]); kept after he lets go.
    pub rope: Option<ObjectId>,
    /// Time left before Claw can grab the rope he let go of again, ms.
    pub rope_regrab_ms: i32,
    /// How he is drawn while a powerup runs (see [`look`]).
    pub look: look::Look,
    /// The keys he pressed while held in a hug (see [`hug`]).
    pub hug: hug::HugData,
}

impl Default for Claw {
    fn default() -> Self {
        Claw {
            facing_right: true,
            attacking: false,
            stats: PlayerStats::new_game(),
            attack: attack::AttackData::default(),
            special: special::SpecialData::default(),
            fall_height: 0,
            spawn_point: (0, 0),
            checkpoint: None,
            jump_height: 0,
            jump_start_y: 0,
            jump_peak_y: 0,
            climb_peak_y: 0,
            climb_dir: 5,
            conveyor_force: 0.0,
            post_climb_lockout_ms: 0,
            look_up_time: 0,
            jump_time: 0,
            look_down_time: 0,
            running_time: 0,
            jump_press_time: 0.0,
            afk_time: 0,
            aux_timer_ms: 0,
            clamp: Rect::new(i32::MIN, i32::MIN, i32::MAX, i32::MAX),
            mover: Mover::default(),
            death_tile_hits: 0,
            fallback_entries: 0,
            movement: MovementOverrides::default(),
            repress_directions: false,
            respawn_pending: false,
            rope: None,
            rope_regrab_ms: 0,
            look: look::Look::default(),
            hug: hug::HugData::default(),
        }
    }
}

/// Everything a player handler works with during one call.
///
/// Claw's own object is held in `o` for the call; its slot in `objects`
/// holds a stand-in meanwhile, so `objects` gives every other object.
/// [`Ctx::with_world`] puts Claw back for code that works on any object.
pub struct Ctx<'a> {
    /// Claw's object.
    pub o: &'a mut Object,
    /// Claw's data.
    pub p: &'a mut Claw,
    /// Claw's object id.
    pub id: ObjectId,
    /// Every other object (Claw's slot holds a stand-in during the call).
    pub objects: &'a mut Container,
    /// The level.
    pub level: &'a SimLevel,
    /// Platform objects.
    pub platforms: &'a [Platform],
    /// This tick's input.
    pub input: TickInput,
    /// Tuning data.
    pub t: &'a Tuning,
    /// Game randomness.
    pub rng: &'a mut Rng,
    /// The sound-only random stream (see [`World::sound_rng`]).
    pub sound_rng: &'a mut Rng,
    /// Scripted camera point, reset at level start and respawn.
    pub camera_override: &'a mut Option<(i32, i32)>,
    /// Integer centre of the main plane's view.
    pub plane_centre: (i32, i32),
    /// Sounds, events and screen effects of the tick.
    pub out: &'a mut Outbox,
    /// Number of the tick being run.
    pub tick: u64,
    /// Simulation time at the start of the tick, ms.
    pub now_ms: u64,
    /// Game time the call covers, ms: one 18 ms tick, or 0 for a call
    /// that takes no game time (the settle after a warp, see
    /// [`crate::Sim::step`]). Claw's movement, timers and animation advance
    /// by it.
    pub dt_ms: u32,
    /// The tick is a frame pulse tick (see [`World::pulse`]).
    pub pulse: bool,
    /// The logic view in level pixels.
    pub view: Rect,
    /// The level objects run their logics
    /// ([`crate::sim::SimConfig::level_logics`]). Off, Claw keeps the
    /// first playable version's behaviour where a level logic would take
    /// over (a death tile is counted, not deadly).
    pub level_logics: bool,
    /// The play-test aids ([`crate::sim::SimConfig::aids`]).
    pub aids: PlayAids,
}

impl std::fmt::Debug for Ctx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("state", &self.o.state)
            .field("x", &self.o.x)
            .field("y", &self.o.y)
            .finish_non_exhaustive()
    }
}

impl Ctx<'_> {
    /// [`Ctx::dt_ms`] as a signed number of ms, for the timers.
    pub fn dt(&self) -> i32 {
        self.dt_ms as i32
    }

    /// [`Ctx::dt_ms`] in seconds, for the movement.
    pub fn dt_s(&self) -> f64 {
        f64::from(self.dt_ms) * 0.001
    }

    /// Runs `f` with a [`World`] in which Claw's object is back in its
    /// slot, for code that works on any object (spawning, lifting, other
    /// objects' hooks, sounds placed at an object).
    pub fn with_world<R>(&mut self, f: impl FnOnce(&mut World<'_>) -> R) -> R {
        self.objects.swap_payload(self.id, self.o);
        let r = {
            let mut w = World {
                objects: &mut *self.objects,
                claw_id: Some(self.id),
                claw: &mut *self.p,
                level: self.level,
                rng: &mut *self.rng,
                sound_rng: &mut *self.sound_rng,
                tuning: self.t,
                tick: self.tick,
                now_ms: self.now_ms,
                dt_ms: self.dt_ms,
                pulse: self.pulse,
                view: self.view,
                out: &mut *self.out,
                aids: self.aids,
            };
            f(&mut w)
        };
        self.objects.swap_payload(self.id, self.o);
        r
    }

    /// The level's retail number (see [`SimLevel::number`]).
    pub fn level_number(&self) -> u32 {
        self.level.number
    }

    /// Claw's deaths so far in this level (see
    /// [`stats::PlayerStats::attempt`]).
    pub fn attempt(&self) -> u32 {
        self.p.stats.attempt
    }

    /// Asks for Claw to say the line `key`; the simulation passes it to
    /// [`crate::logic::sound::claw_say`] once this handler returns (see
    /// [`Outbox::say`]).
    pub fn say(&mut self, key: &str, force: bool) {
        self.out.say(key, force);
    }

    /// Plays `key` centred on `channel` at the configured volume.
    pub fn play(&mut self, key: &str, channel: claw_frame::SoundChannel) {
        self.out.cue(AudioCue::Play(SoundCue {
            key: key.to_string(),
            channel,
            ..SoundCue::default()
        }));
    }

    /// Catnip is running: Claw walks, climbs and jumps faster and higher.
    pub fn has_catnip(&self) -> bool {
        self.p.stats.powerup_kind == death::powerup::CATNIP && self.p.stats.powerup_ms > 0
    }

    fn held(&self, b: Buttons) -> bool {
        self.input.held.contains(b)
    }

    fn pressed(&self, b: Buttons) -> bool {
        self.input.pressed.contains(b)
    }

    fn set_anim(&mut self, key: &str, advance_now: bool) {
        if let Some(a) = self.level.animations.get(key) {
            self.o.anim.set(a.clone());
            if advance_now {
                self.advance_anim();
            }
        }
    }

    /// Advances Claw's animation one tick (within his image set's frame
    /// range, when the level knows it) and plays the sounds of the element
    /// it applies; returns the event value.
    fn advance_anim(&mut self) -> i32 {
        let mirrored = self.o.draw_flags & draw_flags::MIRROR != 0;
        let frames = self.level.image_frames(&self.o.image_set);
        let e = self
            .o
            .anim
            .advance_in(self.dt_ms, self.o.frame, mirrored, frames);
        if let Some(f) = e.frame {
            self.o.frame = f;
        }
        if let Some((dx, dy)) = e.moved_by {
            self.o.x += dx;
            self.o.y += dy;
        }
        if let Some((x, y)) = e.moved_to {
            self.o.x = x;
            self.o.y = y;
        }
        if let Some(index) = e.applied
            && let Some(anim) = self.o.anim.animation().cloned()
            && !anim.sounds_of(index).is_empty()
        {
            let id = self.id;
            self.with_world(|w| w.frame_sound(id, &anim, index));
        }
        e.event
    }

    fn physics(&mut self, tx: i32, ty: i32, mask: u32) -> u32 {
        let c = Collider {
            map: &self.level.map,
            platforms: self.platforms,
            step: self.t.physics.step,
        };
        physics::physics(&c, self.o, tx, ty, mask)
    }

    /// The ridden platform's move this tick.
    fn elevator_delta(&self) -> (i32, i32) {
        if self.o.flags & flags::ON_ELEVATOR == 0 {
            return (0, 0);
        }
        self.o
            .object_below
            .and_then(|id| self.platforms.iter().find(|p| p.id == id))
            .map_or((0, 0), |p| (p.move_x, p.move_y))
    }

    fn set_hit_rect_for_facing(&mut self) {
        let r = self.t.player.hit_rect;
        self.o.hit_rect = if self.p.facing_right { r } else { r.mirrored() };
    }

    fn set_stand_move_rect(&mut self) {
        self.o.move_rect = self.t.player.move_rect;
    }

    fn set_mirror(&mut self, on: bool) {
        if on {
            self.o.draw_flags |= draw_flags::MIRROR;
        } else {
            self.o.draw_flags &= !draw_flags::MIRROR;
        }
    }

    fn face_right(&mut self) {
        if !self.p.facing_right {
            self.set_mirror(false);
            self.o.mirror_rects();
            self.p.facing_right = true;
            self.p.running_time = self.t.player.run_onset_ms;
        }
    }

    fn face_left(&mut self) {
        if self.p.facing_right {
            self.set_mirror(true);
            self.o.mirror_rects();
            self.p.facing_right = false;
            self.p.running_time = self.t.player.run_onset_ms;
        }
    }

    fn speeds_zero(&mut self) {
        self.o.speed_x = 0;
        self.o.speed_y = 0;
    }

    /// Drops whatever the transition interrupts: the running attack.
    fn reset_actions(&mut self) {
        attack::cancel(self);
    }

    fn up_down_mask(&self, base: u32) -> u32 {
        if self.held(Buttons::UP) {
            base | opt::GRAB_UP
        } else if self.held(Buttons::DOWN) {
            base | opt::GRAB_DOWN
        } else {
            base
        }
    }

    fn ledge_on_left_only(&self) -> bool {
        if self.o.flags & flags::ON_ELEVATOR != 0 {
            return false;
        }
        let m = self.o.move_rect;
        let tp = &self.t.player;
        if !physics::support_at(&self.level.map, self.o, m.left) {
            return false;
        }
        let mut dx = m.left + tp.ledge_probe_inset;
        while dx < m.right {
            if physics::support_at(&self.level.map, self.o, dx) {
                return false;
            }
            dx += tp.ledge_probe_stride.max(1);
        }
        !physics::support_at(&self.level.map, self.o, m.right)
    }

    fn ledge_on_right_only(&self) -> bool {
        if self.o.flags & flags::ON_ELEVATOR != 0 {
            return false;
        }
        let m = self.o.move_rect;
        let tp = &self.t.player;
        if !physics::support_at(&self.level.map, self.o, m.right) {
            return false;
        }
        let mut dx = m.right - tp.ledge_probe_inset;
        while dx > m.left {
            if physics::support_at(&self.level.map, self.o, dx) {
                return false;
            }
            dx -= tp.ledge_probe_stride.max(1);
        }
        !physics::support_at(&self.level.map, self.o, m.left)
    }
}

// ----------------------------------------------------------------------
// Transition helpers

/// Enters Stand: speeds and timers reset, standing boxes.
pub fn enter_stand(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::STAND;
    c.set_anim("CLAW_STAND", true);
    c.o.physics_type = ptype::STAND;
    c.o.flags |= flags::HARMLESS;
    c.speeds_zero();
    c.o.damage = 0;
    c.set_hit_rect_for_facing();
    c.set_stand_move_rect();
    let tp = &c.t.player;
    c.p.look_up_time = tp.look_up_delay_ms;
    c.p.running_time = tp.run_onset_ms;
    c.p.afk_time = tp.idle_delay_ms;
    c.p.jump_press_time = 0.0;
    c.p.mover.zero_carries();
    c.o.anim.set_speed(1.0);
}

fn start_jump_common(c: &mut Ctx) {
    c.o.state = state::JUMP;
    c.set_anim("CLAW_JUMP", false);
    c.o.flags &= !flags::ON_ELEVATOR;
    c.o.physics_type = ptype::JUMP;
    c.speeds_zero();
    c.set_hit_rect_for_facing();
    c.set_stand_move_rect();
    let tp = &c.t.player;
    c.p.jump_press_time = tp.jump_hold_window_ms;
    c.p.jump_height = 0;
    c.p.jump_start_y = c.o.y;
    c.p.jump_time = 0;
    c.p.jump_peak_y = c.o.y - tp.jump_base_height;
    c.p.mover.y.a = tp.gravity;
    c.p.mover.x.p = f64::from(c.o.x);
    c.p.mover.y.p = f64::from(c.o.y);
    let (px, peak) = (c.p.mover.x.p, f64::from(c.p.jump_peak_y));
    c.p.mover.aim_at(px, peak);
}

/// Starts a jump: aims the launch at the base height above the current Y.
pub fn begin_jump(c: &mut Ctx) {
    c.reset_actions();
    start_jump_common(c);
}

/// Starts a fall from rest; `keep_anim` keeps the current animation (the
/// jump's apex).
pub fn begin_fall(c: &mut Ctx, keep_anim: bool) {
    c.reset_actions();
    fall_from_rest(c, keep_anim);
}

/// The fall start without cancelling the running attack.
fn fall_from_rest(c: &mut Ctx, keep_anim: bool) {
    if !keep_anim {
        c.set_anim("CLAW_FALL", false);
    }
    c.o.state = state::FALL;
    c.o.flags &= !flags::ON_ELEVATOR;
    c.o.physics_type = ptype::FALL;
    c.speeds_zero();
    c.set_hit_rect_for_facing();
    c.set_stand_move_rect();
    c.p.fall_height = 0;
    c.p.mover.y.v = 0.0;
    c.p.mover.y.a = c.t.player.gravity;
}

/// Grabs a ladder.
pub fn enter_climb(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::CLIMB;
    c.set_anim("CLAW_CLIMB", false);
    c.o.physics_type = ptype::CLIMB;
    c.o.flags &= !flags::ON_ELEVATOR;
    c.speeds_zero();
    c.p.climb_dir = 0;
    c.p.climb_peak_y = 0;
    c.p.mover.y.a = 0.0;
}

fn teeter_voice(c: &mut Ctx) {
    if c.rng.below(100) < c.t.player.teeter_voice_percent {
        c.play(TEETER_VOICE, claw_frame::SoundChannel::Voice);
    }
}

/// Starts teetering on a ledge.
pub fn enter_on_edge(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::ON_EDGE;
    c.set_anim("CLAW_TEETER", true);
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    c.p.running_time = c.t.player.run_onset_ms;
    teeter_voice(c);
}

/// Starts looking up.
pub fn enter_look_up(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::LOOK_UP;
    c.set_anim("CLAW_LOOKUP", false);
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    c.p.look_down_time = c.t.player.look_up_look_delay_ms;
}

fn begin_walk(c: &mut Ctx, right: bool) {
    c.reset_actions();
    c.o.state = if right {
        state::MOVE_RIGHT
    } else {
        state::MOVE_LEFT
    };
    if right {
        c.face_right();
    } else {
        c.face_left();
    }
    c.set_anim("CLAW_WALK", false);
    c.o.physics_type = ptype::WALK;
    let s = c.t.player.walk_speed as i32;
    c.o.speed_x = if right { s } else { -s };
    c.o.speed_y = 0;
    c.p.post_climb_lockout_ms = 0;
    c.p.mover.zero_carries();
}

/// Starts walking right.
pub fn begin_walk_right(c: &mut Ctx) {
    begin_walk(c, true);
}

/// Starts walking left.
pub fn begin_walk_left(c: &mut Ctx) {
    begin_walk(c, false);
}

/// Crouches.
pub fn enter_crouch(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::CROUCH;
    c.set_anim("CLAW_DUCK", false);
    c.o.physics_type = ptype::DUCK;
    c.speeds_zero();
    c.o.hit_rect = c.t.player.crouch_hit_rect;
    c.o.move_rect = c.t.player.crouch_move_rect;
    if !c.p.facing_right {
        c.o.mirror_rects();
    }
    c.p.look_down_time = c.t.player.crouch_look_delay_ms;
}

/// Lands after a long fall.
pub fn enter_land(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::LAND;
    c.set_anim("CLAW_LAND", true);
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    c.p.running_time = c.t.player.run_onset_ms;
}

// ----------------------------------------------------------------------
// One-shot states

/// Level-start set-up: places Claw at the main plane's view centre, drops
/// him onto the floor below and records that as the spawn point.
pub fn init(c: &mut Ctx) {
    *c.camera_override = None;
    let tp = &c.t.player;
    let w = c.level.map.width();
    let h = c.level.map.height();
    let clamp = Rect::new(
        tp.clamp_inset,
        tp.clamp_inset,
        w - tp.clamp_inset,
        h - tp.clamp_inset,
    );
    *c.p = Claw {
        clamp,
        stats: c.p.stats.clone(),
        death_tile_hits: c.p.death_tile_hits,
        fallback_entries: c.p.fallback_entries,
        ..Claw::default()
    };
    c.p.stats.health = tp.respawn_health;
    c.p.stats.invulnerable_ms = 0;
    c.o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    c.o.x = c.plane_centre.0;
    c.o.y = c.plane_centre.1;
    c.o.z = tp.z;
    c.o.hit_handler = true;
    c.o.flags = (c.o.flags & !flags::ON_ELEVATOR) | flags::HARMLESS;
    c.o.type_flags = types::PLAYER;
    c.o.hit_type_flags = types::CLAW_HIT_BY;
    c.speeds_zero();
    c.o.object_below = None;
    c.o.move_rect = tp.move_rect;
    c.o.hit_rect = tp.hit_rect;
    c.o.attack_rect = tp.attack_rect;
    physics::align_to_ground(&c.level.map, c.o);
    c.p.climb_dir = 5;
    c.p.spawn_point = (c.o.x, c.o.y);
    c.p.facing_right = true;
    c.set_mirror(false);
    enter_stand(c);
    c.p.mover.double_count = tp.double_count_fraction;
    c.p.mover.init(tp.gravity, tp.step_limit, tp.velocity_cap);
    c.o.teleported = true;
}

/// Places Claw at the checkpoint, or at the spawn point without one; the
/// flag sound plays when he comes back somewhere else than the spawn
/// point.
pub fn respawn(c: &mut Ctx) {
    *c.camera_override = None;
    let tp = &c.t.player;
    c.p.stats.health = tp.respawn_health;
    let (x, y) = c.p.checkpoint.unwrap_or(c.p.spawn_point);
    c.o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    c.o.x = x;
    c.o.y = y;
    c.o.z = tp.z;
    c.o.flags &= !(flags::NO_COLLIDE | flags::SAFE | flags::ON_DEATH_TILE);
    c.o.hit_type_flags |= types::CLAW_SHIELDED;
    c.speeds_zero();
    c.o.draw_flags &= !(draw_flags::HIDDEN | draw_flags::FLASH);
    physics::align_to_ground(&c.level.map, c.o);
    enter_stand(c);
    c.p.stats.invulnerable_ms = 0;
    c.p.climb_dir = 5;
    c.p.attacking = false;
    c.p.conveyor_force = 0.0;
    c.p.aux_timer_ms = 0;
    c.p.rope_regrab_ms = 0;
    c.o.teleported = true;
    if c.o.x != c.p.spawn_point.0 {
        c.play(CHECKPOINT_RESPAWN_SOUND, claw_frame::SoundChannel::Effect);
    }
}

// ----------------------------------------------------------------------
// Movement handlers

/// True while an attack runs whose active part is not over: the host
/// states ignore movement input meanwhile.
pub fn attack_holds_input(c: &Ctx) -> bool {
    c.p.attacking && !c.p.attack.interruptible
}

/// Stand: starts jumps and walks, teeters at ledges, crouches, looks up.
/// While an attack runs and cannot be interrupted yet, Jump, Left and
/// Right are ignored and the ledge test comes first.
///
/// A walk starts when Right or Left is pressed, not while it is held: a
/// direction held on comes back as a press on the tick after an attack
/// ends or any change of state (see [`Claw::repress_directions`]), so a
/// direction held through a standing attack walks on the tick after the
/// attack's end.
pub fn stand_tick(c: &mut Ctx) {
    if !attack_holds_input(c) {
        if c.pressed(Buttons::JUMP) {
            stand_jump_start(c);
            return;
        }
        if c.pressed(Buttons::RIGHT) {
            stand_walk_start(c, true);
            return;
        }
        if c.pressed(Buttons::LEFT) {
            stand_walk_start(c, false);
            return;
        }
    }
    if c.o.flags & flags::ON_ELEVATOR == 0 {
        if c.ledge_on_right_only() {
            if c.p.facing_right {
                c.set_mirror(true);
                c.o.mirror_rects();
                c.p.facing_right = false;
                c.p.running_time = c.t.player.run_onset_ms;
            }
            c.o.state = state::ON_EDGE;
            c.set_anim("CLAW_TEETER", true);
            c.o.physics_type = ptype::STAND;
            c.speeds_zero();
            c.p.running_time = c.t.player.run_onset_ms;
            teeter_voice(c);
            on_edge_update(c);
            return;
        }
        if c.ledge_on_left_only() {
            c.face_right();
            enter_on_edge(c);
            on_edge_update(c);
            return;
        }
    }
    attack::select(c);
    stand_update(c);
}

fn stand_jump_start(c: &mut Ctx) {
    start_jump_common(c);
    jump_hold_update(c);
    let air = c.t.player.air_speed;
    let vx = if c.held(Buttons::RIGHT) {
        c.face_right();
        air
    } else if c.held(Buttons::LEFT) {
        c.face_left();
        -air
    } else {
        0.0
    };
    c.o.speed_x = vx as i32;
    jump_integrate(c, vx + c.p.conveyor_force);
}

fn stand_walk_start(c: &mut Ctx, right: bool) {
    c.reset_actions();
    c.o.state = if right {
        state::MOVE_RIGHT
    } else {
        state::MOVE_LEFT
    };
    if right {
        c.face_right();
    } else {
        c.face_left();
    }
    c.set_anim("CLAW_WALK", false);
    c.o.physics_type = ptype::WALK;
    let s = c.t.player.walk_speed as i32;
    c.o.speed_x = if right { s } else { -s };
    c.o.speed_y = 0;
    c.p.post_climb_lockout_ms = 0;
    c.p.mover.zero_carries();
    walk_tick(c, right);
}

/// The standing update: drift, physics in place, crouch, look up, idle.
fn stand_update(c: &mut Ctx) {
    c.speeds_zero();
    let mask = c.up_down_mask(opt::PLATFORMS | opt::CENTRE_ON_LADDER);
    if c.held(Buttons::UP) || c.held(Buttons::DOWN) {
        c.p.afk_time = c.t.player.idle_delay_ms;
    }
    let mut tx = c.o.x;
    if c.p.conveyor_force != 0.0 {
        c.p.mover.zero_carries();
        c.p.mover.x.v = c.p.conveyor_force;
        c.p.mover.step(c.dt_s());
        tx += c.p.mover.take_x();
    }
    let (ex, ey) = c.elevator_delta();
    let y = c.o.y;
    c.physics(tx + ex, y + ey, mask);
    let event = match c.o.physics_type {
        ptype::FALL => {
            begin_fall(c, false);
            return;
        }
        ptype::CLIMB => {
            enter_climb(c);
            return;
        }
        _ => animate(c),
    };
    let free = !c.p.attacking || c.p.attack.interruptible;
    if free && c.held(Buttons::DOWN) && c.o.physics_type == ptype::STAND {
        enter_crouch(c);
        return;
    }
    if !c.p.attacking
        && c.p.post_climb_lockout_ms == 0
        && c.held(Buttons::UP)
        && c.o.physics_type == ptype::STAND
    {
        c.p.look_up_time = (c.p.look_up_time - c.dt()).max(0);
        if c.p.look_up_time == 0 {
            enter_look_up(c);
            return;
        }
    }
    if !c.p.attacking && c.p.afk_time > 0 {
        c.p.afk_time = (c.p.afk_time - c.dt()).max(0);
        if c.p.afk_time == 0 {
            c.set_anim("CLAW_IDLE", true);
        }
    }
    if event == attack::STAND_FRAME_EVENT {
        attack::frame_event(c, event);
    }
    if c.o.anim.is_done() && c.o.anim.animation().is_some() {
        c.set_anim("CLAW_STAND", true);
        c.p.afk_time = c.t.player.idle_delay_ms;
    }
}

/// Walk or run one tick in the given direction.
fn walk_tick(c: &mut Ctx, right: bool) {
    let tp = &c.t.player;
    c.p.running_time = (c.p.running_time - c.dt()).max(0);
    let (speed, anim_speed) = if c.has_catnip() {
        (tp.walk_speed * CATNIP_WALK_FACTOR, CATNIP_ANIM_SPEED)
    } else if c.p.running_time == 0 {
        (tp.run_speed, RUN_ANIM_SPEED)
    } else {
        (tp.walk_speed, WALK_ANIM_SPEED)
    };
    c.o.anim.set_speed(anim_speed);
    let v = if right { speed } else { -speed } + c.p.conveyor_force;
    c.o.speed_x = v as i32;
    c.p.mover.x.v = v;
    c.p.mover.step(c.dt_s());
    let tx = c.o.x + c.p.mover.take_x();
    let (ex, ey) = c.elevator_delta();
    let y = c.o.y;
    c.physics(tx + ex, y + ey, opt::PLATFORMS);
    match c.o.physics_type {
        ptype::STAND => enter_stand(c),
        ptype::FALL => begin_fall(c, false),
        _ => {
            c.advance_anim();
        }
    }
}

/// The animation step of a host state: the attack update while an attack
/// runs, otherwise the plain animation advance. Returns the event value
/// (-1 when the attack update ran).
fn animate(c: &mut Ctx) -> i32 {
    if c.p.attacking {
        attack::update(c);
        -1
    } else {
        c.advance_anim()
    }
}

fn walk_dispatch(c: &mut Ctx, right: bool) {
    let (fwd, back) = if right {
        (Buttons::RIGHT, Buttons::LEFT)
    } else {
        (Buttons::LEFT, Buttons::RIGHT)
    };
    if c.held(Buttons::DOWN) {
        enter_stand(c);
        stand_update(c);
    } else if c.held(fwd) {
        walk_tick(c, right);
    } else if c.held(back) {
        begin_walk(c, !right);
        walk_tick(c, !right);
    } else {
        enter_stand(c);
        stand_update(c);
    }
    if attack::select(c) {
        stand_after_attack_start(c);
    } else if c.pressed(Buttons::JUMP) {
        begin_jump(c);
    }
}

/// Stops walking because an attack started: back to standing (unless
/// lifting), with the standing boxes and timers, and the stand animation
/// as the one to come back to after the attack.
fn stand_after_attack_start(c: &mut Ctx) {
    if c.o.state == state::LIFT {
        return;
    }
    c.o.state = state::STAND;
    c.o.physics_type = ptype::STAND;
    c.speeds_zero();
    c.set_hit_rect_for_facing();
    c.set_stand_move_rect();
    let tp = &c.t.player;
    c.p.running_time = tp.run_onset_ms;
    c.p.look_up_time = tp.look_up_delay_ms;
    c.p.afk_time = tp.idle_delay_ms;
    c.p.attack.restore_anim = Some("CLAW_STAND".to_string());
}

/// MoveRight.
pub fn move_right_tick(c: &mut Ctx) {
    walk_dispatch(c, true);
}

/// MoveLeft.
pub fn move_left_tick(c: &mut Ctx) {
    walk_dispatch(c, false);
}

/// Raises the jump target while Jump is held inside the hold window.
fn jump_hold_update(c: &mut Ctx) {
    if !(c.held(Buttons::JUMP) && c.p.jump_press_time > 0.0) {
        return;
    }
    let tp = &c.t.player;
    c.p.jump_press_time = (c.p.jump_press_time - tp.jump_hold_per_tick_ms).max(0.0);
    let (speed_y, k) = if c.p.movement.super_jump {
        (JUMP_HOLD_SPEED_SUPER, tp.jump_hold_factor_powered)
    } else if c.has_catnip() {
        (JUMP_HOLD_SPEED_FAST, tp.jump_hold_factor_powered)
    } else if c.p.running_time == 0 {
        (JUMP_HOLD_SPEED_FAST, tp.jump_hold_factor_running)
    } else {
        (JUMP_HOLD_SPEED, tp.jump_hold_factor)
    };
    c.o.speed_y = speed_y;
    let target = (f64::from(c.p.jump_start_y) - (tp.jump_hold_window_ms - c.p.jump_press_time) * k)
        .trunc() as i32;
    c.p.jump_peak_y = c.p.jump_peak_y.min(target);
    let (px, peak) = (c.p.mover.x.p, f64::from(c.p.jump_peak_y));
    c.p.mover.aim_at(px, peak);
}

/// Integrates one rising tick and moves Claw; shared by every jump start
/// and the jump state.
fn jump_integrate(c: &mut Ctx, vx: f64) {
    c.p.mover.x.v = vx;
    c.p.mover.step(c.dt_s());
    let tx = c.o.x + c.p.mover.take_x();
    let mut ty = c.o.y + c.p.mover.take_y();
    c.p.jump_time += c.dt();
    if c.p.mover.y.v >= 0.0 {
        c.p.mover.snap_to_y(f64::from(c.p.jump_peak_y));
        ty = c.p.jump_peak_y;
    }
    let mask = if c.held(Buttons::UP) {
        opt::GRAB_UP | opt::CENTRE_ON_LADDER
    } else {
        0
    };
    ty = ty.max(c.p.jump_peak_y);
    let y_before = c.o.y;
    c.physics(tx, ty, mask);
    c.p.jump_height += y_before - c.o.y;
    match c.o.physics_type {
        ptype::FALL => {
            if attack::air_attack_running(c) {
                fall_from_rest(c, false);
            } else {
                begin_fall(c, false);
            }
        }
        ptype::CLIMB => enter_climb(c),
        _ => {
            animate(c);
            if c.o.y <= c.p.jump_peak_y {
                if attack::carry_over_jump_top(c) {
                    fall_from_rest(c, true);
                } else {
                    begin_fall(c, true);
                }
            }
        }
    }
}

/// Jump: attack selection, then the jump update.
pub fn jump_tick(c: &mut Ctx) {
    attack::select(c);
    jump_update(c);
}

/// The jump update: hold update, steering, rise to the target, then fall
/// from the apex.
pub fn jump_update(c: &mut Ctx) {
    jump_hold_update(c);
    if !c.held(Buttons::JUMP) {
        c.p.jump_press_time = 0.0;
    }
    let vx = steer(c);
    jump_integrate(c, vx + c.p.conveyor_force);
}

/// Air steering: faces and moves with Left or Right.
fn steer(c: &mut Ctx) -> f64 {
    let air = c.t.player.air_speed;
    let v = if c.held(Buttons::RIGHT) {
        c.face_right();
        air
    } else if c.held(Buttons::LEFT) {
        c.face_left();
        -air
    } else {
        0.0
    };
    c.o.speed_x = v as i32;
    v
}

/// Fall: attack selection, steering, gravity, landing.
pub fn fall_tick(c: &mut Ctx) {
    attack::select(c);
    let vx = steer(c);
    c.p.mover.x.v = vx;
    c.p.mover.step(c.dt_s());
    let tx = c.o.x + c.p.mover.take_x();
    let ty = c.o.y + c.p.mover.take_y();
    let mask = if c.held(Buttons::UP) {
        opt::PLATFORMS | opt::CENTRE_ON_LADDER | opt::GRAB_UP
    } else {
        opt::PLATFORMS | opt::CENTRE_ON_LADDER
    };
    let y_before = c.o.y;
    c.physics(tx, ty, mask);
    c.p.fall_height += c.o.y - y_before;
    match c.o.physics_type {
        ptype::STAND => {
            c.play(LANDING_SOUND, claw_frame::SoundChannel::Effect);
            if c.p.fall_height >= c.t.player.land_fall_height {
                enter_land(c);
            } else if c.held(Buttons::RIGHT) {
                begin_walk(c, true);
            } else if c.held(Buttons::LEFT) {
                begin_walk(c, false);
            } else {
                enter_stand(c);
            }
        }
        ptype::CLIMB => enter_climb(c),
        _ => {
            if c.p.attacking {
                attack::update(c);
            } else {
                c.advance_anim();
                if c.p.fall_height > c.t.player.long_fall_height {
                    c.o.frame = c.t.player.long_fall_frame;
                }
            }
        }
    }
}

/// Land: waits for the landing animation.
pub fn land_tick(c: &mut Ctx) {
    c.speeds_zero();
    let (ex, ey) = c.elevator_delta();
    let (x, y) = (c.o.x, c.o.y);
    c.physics(x + ex, y + ey, opt::PLATFORMS | opt::CENTRE_ON_LADDER);
    match c.o.physics_type {
        ptype::FALL => {
            begin_fall(c, false);
            return;
        }
        ptype::CLIMB => {
            enter_climb(c);
            return;
        }
        _ => {}
    }
    c.advance_anim();
    if c.o.anim.is_done() {
        if c.held(Buttons::RIGHT) {
            begin_walk(c, true);
        } else if c.held(Buttons::LEFT) {
            begin_walk(c, false);
        } else {
            enter_stand(c);
        }
    }
}

fn in_place(c: &mut Ctx, mask: u32) {
    let mut tx = c.o.x;
    if c.p.conveyor_force != 0.0 {
        c.p.mover.zero_carries();
        c.p.mover.x.v = c.p.conveyor_force;
        c.p.mover.step(c.dt_s());
        tx += c.p.mover.take_x();
    }
    let (ex, ey) = c.elevator_delta();
    let y = c.o.y;
    c.physics(tx + ex, y + ey, mask);
}

/// Crouch: attack selection, then stays down while Down is held and turns
/// in place.
pub fn crouch_tick(c: &mut Ctx) {
    attack::select(c);
    c.p.look_down_time = (c.p.look_down_time - c.dt()).max(0);
    c.speeds_zero();
    in_place(c, opt::PLATFORMS);
    if c.o.physics_type == ptype::FALL {
        begin_fall(c, false);
        return;
    }
    animate(c);
    if attack_holds_input(c) {
        return;
    }
    if !c.held(Buttons::DOWN) && c.o.physics_type == ptype::DUCK {
        enter_stand(c);
    } else if c.held(Buttons::LEFT) && c.p.facing_right {
        c.set_mirror(true);
        c.o.mirror_rects();
        c.p.facing_right = false;
        c.p.running_time = c.t.player.run_onset_ms;
    } else if c.held(Buttons::RIGHT) && !c.p.facing_right {
        c.face_right();
    }
}

/// LookUp: holds while Up is held; Jump starts a jump at once.
pub fn look_up_tick(c: &mut Ctx) {
    if c.pressed(Buttons::JUMP) {
        begin_jump(c);
        jump_update(c);
        return;
    }
    c.p.look_down_time = (c.p.look_down_time - c.dt()).max(0);
    in_place(c, opt::PLATFORMS);
    if c.o.physics_type == ptype::FALL {
        begin_fall(c, false);
        return;
    }
    c.advance_anim();
    if !c.held(Buttons::UP) && c.o.physics_type == ptype::STAND {
        enter_stand(c);
    }
}

/// Climb: up and down a ladder, over its top, or let go with a jump.
pub fn climb_tick(c: &mut Ctx) {
    let tp = &c.t.player;
    if c.pressed(Buttons::JUMP) && !c.held(Buttons::UP) {
        c.reset_actions();
        start_jump_common(c);
        return;
    }
    let m = c.o.move_rect;
    if c.p.climb_peak_y == 0 && c.level.map.attr(c.o.x, c.o.y + m.top) != CLIMB {
        let first = c.o.y + m.top + 1;
        let last = c.o.y + m.bottom;
        c.p.climb_peak_y = (first..=last)
            .find(|&y| c.level.map.attr(c.o.x, y) == CLIMB)
            .unwrap_or(0);
    }
    let climb = if c.has_catnip() {
        CATNIP_CLIMB_FACTOR
    } else {
        1.0
    };
    let vy = if c.held(Buttons::UP) {
        if c.p.climb_dir != 8 {
            c.p.climb_dir = 8;
            c.set_anim("CLAW_CLIMB", false);
        }
        -tp.climb_up_speed * climb
    } else if c.held(Buttons::DOWN) {
        if c.p.climb_dir != 2 {
            c.p.climb_dir = 2;
            c.set_anim("CLAW_CLIMBDOWN", false);
        }
        tp.climb_down_speed * climb
    } else {
        if c.p.climb_dir != 5 {
            c.p.climb_dir = 5;
            c.set_anim("CLAW_CLIMBIDLE", false);
        }
        0.0
    };
    c.o.speed_y = vy as i32;
    c.p.mover.y.v = vy;
    c.p.mover.step(c.dt_s());
    let ty = c.o.y + c.p.mover.take_y();
    let x = c.o.x;
    c.physics(x, ty, 0);
    let mut frame_set = false;
    if c.p.climb_peak_y != 0 {
        let h = c.o.y + c.o.move_rect.bottom - c.p.climb_peak_y;
        if (0..=tp.climb_over_height).contains(&h) {
            let band = (h / tp.climb_over_band.max(1)).min(6);
            c.o.frame = tp.climb_over_first_frame - band;
            frame_set = true;
        }
    }
    match c.o.physics_type {
        ptype::FALL => begin_fall(c, false),
        ptype::STAND => {
            c.p.post_climb_lockout_ms = c.t.player.post_climb_lockout_ms;
            enter_stand(c);
        }
        _ => {
            if !frame_set {
                c.advance_anim();
            }
        }
    }
}

/// OnEdge: teeters until a move, a jump or the ledge test fails.
pub fn on_edge_tick(c: &mut Ctx) {
    if c.pressed(Buttons::JUMP) {
        begin_jump(c);
        jump_update(c);
    } else if c.held(Buttons::RIGHT) {
        begin_walk(c, true);
        walk_tick(c, true);
    } else if c.held(Buttons::LEFT) {
        begin_walk(c, false);
        walk_tick(c, false);
    } else {
        on_edge_update(c);
    }
}

fn on_edge_update(c: &mut Ctx) {
    c.speeds_zero();
    let mask = c.up_down_mask(opt::PLATFORMS | opt::CENTRE_ON_LADDER);
    let (ex, ey) = c.elevator_delta();
    let (x, y) = (c.o.x, c.o.y);
    c.physics(x + ex, y + ey, mask);
    match c.o.physics_type {
        ptype::FALL => {
            begin_fall(c, false);
            return;
        }
        ptype::CLIMB => {
            enter_climb(c);
            return;
        }
        _ => {
            c.advance_anim();
        }
    }
    let still = if c.p.facing_right {
        c.ledge_on_left_only()
    } else {
        c.ledge_on_right_only()
    };
    if !still {
        enter_stand(c);
    }
}

// ----------------------------------------------------------------------
// Fallback and unused rows

/// Fallback for states not filled in yet: back to standing.
///
/// A hit reaction ends with the stand reset. By default Claw is left
/// standing even in mid-air, where a jump press starts a second jump, as
/// in the original. With the mid-air hit jump fix on, a hit that ends in
/// mid-air sets the state to Land instead of Stand, keeping the stand
/// animation; Land accepts no jump press and turns into a fall on its next
/// tick when there is no floor.
pub fn fallback_tick(c: &mut Ctx) {
    c.p.fallback_entries += 1;
    let land = c.o.state == state::HIT && c.t.behaviour.fix_double_jump && hit_ends_airborne(c);
    enter_stand(c);
    if land {
        c.o.state = state::LAND;
    }
}

/// True when Claw has no floor under him: a standing move in place would
/// start a fall. The probe runs on a copy, so Claw is left untouched.
fn hit_ends_airborne(c: &mut Ctx) -> bool {
    let saved = c.o.clone();
    c.o.physics_type = ptype::STAND;
    let (x, y) = (c.o.x, c.o.y);
    c.physics(x, y, opt::PLATFORMS);
    let airborne = c.o.physics_type == ptype::FALL;
    *c.o = saved;
    airborne
}

fn no_tick(_: &mut Ctx) {}

fn enter_plain(_: &mut Ctx, _: i32) {}

fn no_exit(_: &mut Ctx, _: i32) {}

fn enter_init(c: &mut Ctx, _: i32) {
    init(c);
}

fn enter_respawn(c: &mut Ctx, _: i32) {
    respawn(c);
}

fn enter_row_stand(c: &mut Ctx, _: i32) {
    enter_stand(c);
}

fn enter_row_jump(c: &mut Ctx, _: i32) {
    begin_jump(c);
}

fn enter_row_right(c: &mut Ctx, _: i32) {
    begin_walk(c, true);
}

fn enter_row_left(c: &mut Ctx, _: i32) {
    begin_walk(c, false);
}

fn enter_row_fall(c: &mut Ctx, _: i32) {
    begin_fall(c, false);
}

fn enter_row_crouch(c: &mut Ctx, _: i32) {
    enter_crouch(c);
}

fn enter_row_climb(c: &mut Ctx, _: i32) {
    enter_climb(c);
}

fn enter_row_land(c: &mut Ctx, _: i32) {
    enter_land(c);
}

fn enter_row_look_up(c: &mut Ctx, _: i32) {
    enter_look_up(c);
}

fn enter_row_on_edge(c: &mut Ctx, _: i32) {
    enter_on_edge(c);
}

fn init_tick(c: &mut Ctx) {
    init(c);
}

fn respawn_tick(c: &mut Ctx) {
    respawn(c);
}

const fn unused(id: i32) -> StateRow {
    StateRow {
        id,
        name: "Unused",
        kind: RowKind::Unused,
        animations: &[],
        physics_type: None,
        enter: enter_plain,
        tick: no_tick,
        exit: no_exit,
    }
}

/// A fallback row whose handler lives in a submodule; the handler falls
/// back to standing until it is filled in.
const fn fallback_via(id: i32, name: &'static str, tick: TickFn) -> StateRow {
    StateRow {
        id,
        name,
        kind: RowKind::Fallback,
        animations: &[],
        physics_type: None,
        enter: enter_plain,
        tick,
        exit: no_exit,
    }
}

const fn row(
    id: i32,
    name: &'static str,
    animations: &'static [&'static str],
    physics_type: i32,
    enter: EnterFn,
    tick: TickFn,
) -> StateRow {
    StateRow {
        id,
        name,
        kind: RowKind::Implemented,
        animations,
        physics_type: Some(physics_type),
        enter,
        tick,
        exit: no_exit,
    }
}

/// Every state id of the game, in id order.
pub static STATE_TABLE: [StateRow; 43] = [
    row(
        state::INIT,
        "Init",
        &["CLAW_STAND"],
        ptype::STAND,
        enter_init,
        init_tick,
    ),
    unused(1),
    unused(2),
    unused(3),
    row(
        state::JUMP,
        "Jump",
        &["CLAW_JUMP"],
        ptype::JUMP,
        enter_row_jump,
        jump_tick,
    ),
    unused(5),
    unused(6),
    unused(7),
    row(
        state::MOVE_RIGHT,
        "MoveRight",
        &["CLAW_WALK"],
        ptype::WALK,
        enter_row_right,
        move_right_tick,
    ),
    row(
        state::MOVE_LEFT,
        "MoveLeft",
        &["CLAW_WALK"],
        ptype::WALK,
        enter_row_left,
        move_left_tick,
    ),
    unused(10),
    unused(11),
    unused(12),
    unused(13),
    unused(14),
    unused(15),
    unused(16),
    unused(17),
    unused(18),
    unused(19),
    unused(20),
    unused(21),
    unused(22),
    row(
        state::RESPAWN,
        "Respawn",
        &["CLAW_STAND"],
        ptype::STAND,
        enter_respawn,
        respawn_tick,
    ),
    row(
        state::STAND,
        "Stand",
        &["CLAW_STAND", "CLAW_IDLE"],
        ptype::STAND,
        enter_row_stand,
        stand_tick,
    ),
    row(
        state::FALL,
        "Fall",
        &["CLAW_FALL", "CLAW_JUMP"],
        ptype::FALL,
        enter_row_fall,
        fall_tick,
    ),
    row(
        state::CROUCH,
        "Crouch",
        &["CLAW_DUCK"],
        ptype::DUCK,
        enter_row_crouch,
        crouch_tick,
    ),
    row(
        state::CLIMB,
        "Climb",
        &["CLAW_CLIMB", "CLAW_CLIMBDOWN", "CLAW_CLIMBIDLE"],
        ptype::CLIMB,
        enter_row_climb,
        climb_tick,
    ),
    unused(28),
    unused(29),
    unused(30),
    fallback_via(state::HIT, "Hit", special::hit_tick),
    fallback_via(state::LIFT, "Lift", special::lift_tick),
    fallback_via(state::THROW, "Throw", special::throw_tick),
    row(
        state::LAND,
        "Land",
        &["CLAW_LAND"],
        ptype::STAND,
        enter_row_land,
        land_tick,
    ),
    row(
        state::LOOK_UP,
        "LookUp",
        &["CLAW_LOOKUP"],
        ptype::STAND,
        enter_row_look_up,
        look_up_tick,
    ),
    fallback_via(state::DEATH_FALL, "DeathFall", death::death_fall_tick),
    fallback_via(state::DEATH, "Death", death::death_tick),
    row(
        state::ON_EDGE,
        "OnEdge",
        &["CLAW_TEETER"],
        ptype::STAND,
        enter_row_on_edge,
        on_edge_tick,
    ),
    fallback_via(state::SWING, "Swing", swing::swing_tick),
    fallback_via(state::BEARHUG, "Bearhug", hug::hug_tick),
    fallback_via(state::FREEZE, "Freeze", special::freeze_tick),
    unused(5010),
];

/// Row for ids outside the table: only the post-tick step runs.
pub static UNLISTED_ROW: StateRow = unused(-1);

/// The row of a state id.
pub fn row_of(id: i32) -> &'static StateRow {
    match STATE_TABLE.binary_search_by_key(&id, |r| r.id) {
        Ok(i) => &STATE_TABLE[i],
        Err(_) => &UNLISTED_ROW,
    }
}

/// Moves Claw to state `next` through the current row's exit and the next
/// row's entry.
pub fn transition(c: &mut Ctx, next: i32) {
    let prev = c.o.state;
    (row_of(prev).exit)(c, next);
    c.o.state = next;
    (row_of(next).enter)(c, prev);
}

/// Runs one logic call: the state's handler, then the post-tick step.
///
/// The call that settles a move between two ticks (a warp: Claw was put
/// at the target and the call counts no time) first drops a conveyor
/// belt's push, so he does not drift at the target; the belt he left
/// would clear it in the next tick anyway, before his update.
pub fn tick(c: &mut Ctx) {
    if c.o.teleported && c.dt_ms == 0 {
        c.p.conveyor_force = 0.0;
    }
    let row = row_of(c.o.state);
    (row.tick)(c);
    post_tick(c);
}

/// Shared bookkeeping after every handler: the weapon toggle (with its
/// click), timers (the rope regrab timer among them), the position clamp
/// and death-tile marks.
pub fn post_tick(c: &mut Ctx) {
    if c.pressed(Buttons::TOGGLE) && !c.p.attacking {
        c.p.stats.weapon = stats::next_weapon(c.p.stats.weapon);
        c.play(attack::TOGGLE_SOUND, claw_frame::SoundChannel::Effect);
    }
    if c.p.stats.invulnerable_ms > 0 {
        c.p.stats.invulnerable_ms = (c.p.stats.invulnerable_ms - c.dt()).max(0);
        if c.p.stats.invulnerable_ms == 0 {
            c.o.hit_type_flags |= types::CLAW_SHIELDED;
            c.o.draw_flags &= !draw_flags::FLASH;
        }
    }
    death::powerup_tick(c);
    c.p.post_climb_lockout_ms = (c.p.post_climb_lockout_ms - c.dt()).max(0);
    c.p.aux_timer_ms = (c.p.aux_timer_ms - c.dt()).max(0);
    c.p.rope_regrab_ms = (c.p.rope_regrab_ms - c.dt()).max(0);
    if c.o.state != state::DEATH_FALL {
        let r = c.p.clamp;
        c.o.x = c.o.x.clamp(r.left, r.right.max(r.left));
        c.o.y = c.o.y.clamp(r.top, r.bottom.max(r.top));
    }
    let dying = c.o.state == state::DEATH_FALL || c.o.state == state::DEATH;
    if c.o.flags & flags::ON_DEATH_TILE != 0 {
        c.o.flags &= !flags::ON_DEATH_TILE;
        if !dying {
            death::kill(c, death::DeathCause::DeathTile);
        }
    } else if c.o.flags & flags::ON_ELEVATOR != 0
        && !dying
        && physics::tile_under_feet(&c.level.map, c.o, 0) == DEATH
    {
        death::kill(c, death::DeathCause::DeathTile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_complete() {
        let ids: Vec<i32> = STATE_TABLE.iter().map(|r| r.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        for id in 0..=31 {
            assert_eq!(row_of(id).id, id);
        }
        for id in 5000..=5010 {
            assert_eq!(row_of(id).id, id);
        }
        assert_eq!(row_of(40).kind, RowKind::Unused);
        let implemented: Vec<i32> = STATE_TABLE
            .iter()
            .filter(|r| r.kind == RowKind::Implemented)
            .map(|r| r.id)
            .collect();
        assert_eq!(
            implemented,
            vec![0, 4, 8, 9, 23, 24, 25, 26, 27, 5002, 5003, 5006]
        );
        let fallbacks = STATE_TABLE
            .iter()
            .filter(|r| r.kind == RowKind::Fallback)
            .count();
        assert_eq!(fallbacks, 8);
    }
}
