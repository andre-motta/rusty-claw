//! Elevators and pegs: the objects Claw can stand on besides the tiles.
//!
//! Every platform marks itself with the platform type flag
//! ([`PLATFORM`]), keeps out of pair collision and takes its landing
//! surface from the level's platform rectangles ([`surface`]). A platform
//! that moves sets [`Object::platform_move`] to the move it made in the
//! tick; platforms update before Claw (their depth is lower), so Claw adds
//! that move to his own in the same tick and stays on the surface.
//!
//! - `Elevator` runs back and forth along its track as a pure function of
//!   the level clock (see [`clock_position`]): wherever it is woken, it is
//!   where the clock puts it.
//! - `StartElevator`, `TriggerElevator`, `OneWayStartElevator`,
//!   `OneWayTriggerElevator` and `StopElevator` step a few pixels per tick
//!   between the ends of their track depending on whether Claw stands on
//!   them, and go back to where they were placed after Claw loses a life.
//! - `CrumblingPeg` crumbles a moment after Claw lands on it and comes
//!   back after he loses a life.
//! - `TogglePeg` to `TogglePeg4` slide into and out of the wall on a cycle
//!   of the level clock, each variant a quarter period after the previous
//!   one.
//!
//! A platform reads Claw as he was left by the previous tick: whether he
//! stands on it and his attempt number ([`claw_attempt`]).

use claw_frame::SoundChannel;

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::{ObjectData, World, sound};
use crate::level::SimLevel;
use crate::object::{Object, ObjectId, Rect, draw_flags, flags};
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    "Elevator",
    "StartElevator",
    "TriggerElevator",
    "OneWayStartElevator",
    "OneWayTriggerElevator",
    "StopElevator",
    "CrumblingPeg",
    "TogglePeg",
    "TogglePeg2",
    "TogglePeg3",
    "TogglePeg4",
];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[BACKWARD_ANIM, FORWARD_ANIM];

/// Steps forward through the image set, 50 ms per image, and finishes on
/// its last image.
pub const FORWARD_ANIM: &str = "GAME_FORWARD50";
/// Jumps to the image set's last image and steps back, 50 ms per image,
/// finishing on its first image.
pub const BACKWARD_ANIM: &str = "GAME_BACKWARD50";
/// Played when a crumbling peg starts to crumble.
pub const CRUMBLE_SOUND: &str = "LEVEL_PEGCRUMBLE";
/// Played from a toggle peg when it starts to slide in or out.
pub const SLIDE_SOUND: &str = "LEVEL_PEGSLIDE";

/// Type flag of an object others can land on and ride.
pub const PLATFORM: u32 = 0x80;
/// Type flag of a peg that has crumbled or slid into the wall.
pub const NOT_PLATFORM: u32 = 0x1;

/// Depth given to a platform placed at depth 0 (trigger elevators keep
/// theirs).
pub const DEFAULT_Z: i32 = 2000;
/// Speed of an elevator placed without one, px/s.
pub const DEFAULT_SPEED: i32 = 125;
/// Largest step of a trigger elevator per tick, px.
pub const MAX_STEP: i32 = 16;
/// Delay between Claw landing on a crumbling peg and the crumble, when
/// the peg was placed without one, ms.
pub const DEFAULT_CRUMBLE_DELAY_MS: i32 = 25;
/// Length of each half of a toggle peg's cycle when placed without one,
/// ms.
pub const DEFAULT_TOGGLE_MS: i32 = 1500;
/// Rows added below every level platform rectangle.
pub const SURFACE_DEPTH: i32 = 32;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// A clock elevator running.
    pub const RUNNING: i32 = 1;
    /// A trigger elevator stopped for good; a crumbling peg waiting for
    /// Claw; a toggle peg out for good.
    pub const STOPPED: i32 = 5;
    /// A crumbled peg, gone until Claw's next attempt.
    pub const GONE: i32 = 23;
    /// A trigger elevator heading for the low ends of its track.
    pub const TOWARD_MIN: i32 = 1000;
    /// A trigger elevator heading for the high ends of its track.
    pub const TOWARD_MAX: i32 = 1001;
    /// A crumbling peg starting to crumble; a toggle peg starting to slide
    /// out.
    pub const START: i32 = 1002;
    /// A toggle peg sliding into the wall.
    pub const SLIDING_IN: i32 = 1003;
    /// A toggle peg starting to slide into the wall.
    pub const START_IN: i32 = 1004;
    /// A crumbling peg crumbling; a toggle peg sliding out.
    pub const RUNNING_ANIM: i32 = 1005;
    /// A toggle peg out and solid.
    pub const OUT: i32 = 1006;
    /// A toggle peg in the wall.
    pub const IN: i32 = 1007;
    /// A crumbling peg asked to crumble.
    pub const ARMED: i32 = 1111;
}

/// Bits of a trigger elevator's behaviour.
pub mod behaviour {
    /// Stands still while Claw is on it.
    pub const STOP: u32 = 0x1;
    /// Waits for Claw to stand on it.
    pub const WAIT: u32 = 0x2;
    /// Stops for good at the first end it reaches.
    pub const ONE_WAY: u32 = 0x100;
    /// Once started, runs for good.
    pub const TRIGGER: u32 = 0x1000;
}

/// One rectangle of the level platform table, before [`SURFACE_DEPTH`] is
/// added to its bottom.
type R = (i32, i32, i32, i32);

const NONE: R = (0, 0, 0, 0);
const PEG: R = (-24, -46, 24, 32);
const STONE: R = (-16, -1, 16, 32);

/// The platform rectangles of levels 1 to 14: elevator, toggle peg,
/// crumbling peg, stepping stone and big elevator. A rectangle a level
/// does not use is all zeros.
const LEVEL_RECTS: [[R; 5]; 14] = [
    [(-32, -8, 36, 32), (-24, -8, 24, 32), PEG, NONE, NONE],
    [(-44, -8, 46, 32), (-24, -6, 38, 32), PEG, NONE, NONE],
    [(-34, -16, 36, 32), NONE, PEG, NONE, NONE],
    [(-44, -18, 46, 32), NONE, PEG, STONE, NONE],
    [(-27, -5, 27, 35), (-27, -6, 25, 35), PEG, NONE, NONE],
    [(-36, -3, 35, 66), NONE, (-36, -32, 35, 66), STONE, NONE],
    [(-60, 70, 74, 110), NONE, PEG, (-7, -10, 22, 30), NONE],
    [
        (-44, -8, 46, 32),
        (-24, -8, 32, 32),
        PEG,
        (-24, -10, 30, 30),
        NONE,
    ],
    [(-44, -8, 41, 32), NONE, PEG, STONE, NONE],
    [
        (-44, -8, 46, 32),
        (-24, -8, 24, 32),
        PEG,
        STONE,
        (-88, -20, 94, 20),
    ],
    [(-32, -6, 30, 40), (-32, -6, 30, 40), PEG, STONE, NONE],
    [(-22, -18, 18, 40), (-24, -8, 24, 32), PEG, STONE, NONE],
    [(-44, -8, 46, 32), (-24, -8, 24, 32), PEG, STONE, NONE],
    [(-34, -6, 23, 40), (-34, -6, 24, 40), PEG, STONE, NONE],
];

/// Which rectangle of the level table a platform uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    /// Elevators.
    Elevator,
    /// Toggle pegs.
    TogglePeg,
    /// Crumbling pegs.
    CrumblingPeg,
    /// Stepping stones.
    SteppingStone,
    /// The big trap elevator.
    BigElevator,
}

/// The landing surface of a `kind` platform in level `level` (1 to 14),
/// relative to the platform's position: the level's rectangle with
/// [`SURFACE_DEPTH`] added to its bottom. Other level numbers use the
/// first level's rectangles.
pub fn surface(level: u32, kind: SurfaceKind) -> Rect {
    let row = LEVEL_RECTS
        .get(level.wrapping_sub(1) as usize)
        .unwrap_or(&LEVEL_RECTS[0]);
    let (l, t, r, b) = row[kind as usize];
    Rect::new(l, t, r, b + SURFACE_DEPTH)
}

/// The number of the level being played, which picks its platform
/// rectangles. The level does not carry its number yet, so every level
/// uses the first level's rectangles.
pub fn level_number(level: &SimLevel) -> u32 {
    let _ = level;
    1
}

/// Claw's attempt number: it rises by one each time he loses a life.
/// Trigger elevators go back to their place and crumbled pegs come back
/// when it changes. Claw does not count his attempts yet, so it stays 0
/// and nothing resets.
pub fn claw_attempt(w: &World) -> u32 {
    let _ = w;
    0
}

/// The level clock: game milliseconds since the level loaded, counting
/// the tick being run (the first tick sees 18).
pub fn level_clock(w: &World) -> u32 {
    (w.now_ms + u64::from(TICK_MS)) as u32
}

/// What a platform reads of Claw in one call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rider {
    /// Claw stands on this platform.
    pub on: bool,
    /// Claw's attempt number.
    pub attempt: u32,
}

/// One axis of an elevator's track.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Axis {
    /// Low end.
    pub min: i32,
    /// High end.
    pub max: i32,
    /// Length of the track in pixels (0 when it does not move on this
    /// axis).
    pub span: i32,
    /// A whole trip out and back, px.
    pub period: i32,
    /// Clock offset that puts the elevator at its start at time 0, ms
    /// (clock elevators).
    pub phase: i32,
    /// Speed, px/s.
    pub speed: i32,
}

/// The track of a clock elevator on one axis: `min` and `max` are the
/// ends, `pos` the placed position, `speed` px/s (0 for the default) and
/// `flip` starts it heading for the low end.
pub fn clock_axis(min: i32, max: i32, pos: i32, speed: i32, flip: bool) -> Axis {
    let mut span = max.wrapping_sub(min).wrapping_add(1);
    if span < 2 {
        span = 0;
    }
    let speed = if speed == 0 { DEFAULT_SPEED } else { speed };
    let mut phase = pos.wrapping_sub(min).clamp(0, span);
    if flip {
        phase = phase.wrapping_add(span);
    }
    Axis {
        min,
        max,
        span,
        period: span.wrapping_mul(2),
        phase: phase.wrapping_mul(1000).wrapping_div(speed),
        speed,
    }
}

/// Where a clock elevator is on `axis` at level clock `ms`, or `None` when
/// it does not move on that axis.
///
/// The position runs from the low end up to one pixel past the high end
/// and back. The arithmetic is 32-bit and wraps: the product of time and
/// speed wraps after about 4.8 hours of level time at 250 px/s.
pub fn clock_position(axis: &Axis, ms: u32) -> Option<i32> {
    if axis.speed <= 0 || axis.period <= 0 {
        return None;
    }
    let t = (axis.phase as u32)
        .wrapping_add(ms)
        .wrapping_mul(axis.speed as u32)
        / 1000;
    let r = (t as i32) % axis.period;
    Some(if r <= axis.span {
        axis.min.wrapping_add(r)
    } else {
        axis.max.wrapping_add(axis.span).wrapping_sub(r)
    })
}

/// The step of a trigger elevator in one tick at `speed` px/s, px.
pub fn trigger_step(speed: i32) -> i32 {
    let q = (speed as u32).wrapping_mul(TICK_MS) / 1000;
    (q as i32).min(MAX_STEP)
}

/// True while a toggle peg with this cycle is out at level clock `ms`.
/// The test is unsigned: before `phase` it counts from 2^32.
pub fn toggle_on(ms: u32, phase: i32, on_ms: i32, period: i32) -> bool {
    match ms.wrapping_sub(phase as u32).checked_rem(period as u32) {
        Some(r) => r <= on_ms as u32,
        None => true,
    }
}

/// A platform's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlatformData {
    /// Horizontal track (elevators).
    pub x: Axis,
    /// Vertical track (elevators).
    pub y: Axis,
    /// Trigger elevator [`behaviour`] bits.
    pub behaviour: u32,
    /// Claw's attempt number when the platform was set up (trigger
    /// elevators) or finished crumbling (crumbling pegs).
    pub attempt: u32,
    /// Crumble delay, ms.
    pub delay_ms: i32,
    /// Frame at which a peg stops or starts being solid.
    pub switch_frame: i32,
    /// Toggle peg cycle offset, ms.
    pub phase: i32,
    /// Toggle peg time out, ms.
    pub on_ms: i32,
    /// Toggle peg cycle length, ms.
    pub period: i32,
    /// First and last frame numbers of the peg's image set, when known
    /// (see [`super::decor::image_frames`]). A value present before the
    /// first call is kept.
    pub frames: Option<(i32, i32)>,
    /// The peg's animation.
    pub anim: SpriteAnim,
    /// Tick of the elevator's last call.
    pub last_tick: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Clock,
    Trigger(u32),
    Crumbling,
    Toggle(Option<i32>),
}

fn kind_of(name: &str) -> Option<Kind> {
    use behaviour::*;
    Some(match name {
        "Elevator" => Kind::Clock,
        "StartElevator" => Kind::Trigger(WAIT),
        "TriggerElevator" => Kind::Trigger(WAIT | TRIGGER),
        "OneWayStartElevator" => Kind::Trigger(WAIT | ONE_WAY),
        "OneWayTriggerElevator" => Kind::Trigger(WAIT | ONE_WAY | TRIGGER),
        "StopElevator" => Kind::Trigger(STOP),
        "CrumblingPeg" => Kind::Crumbling,
        "TogglePeg" => Kind::Toggle(None),
        "TogglePeg2" => Kind::Toggle(Some(750)),
        "TogglePeg3" => Kind::Toggle(Some(1500)),
        "TogglePeg4" => Kind::Toggle(Some(2250)),
        _ => return None,
    })
}

/// Record fields the platforms read.
mod field {
    pub const SMARTS: usize = 17;
    pub const X_MIN: usize = 51;
    pub const Y_MIN: usize = 52;
    pub const X_MAX: usize = 53;
    pub const Y_MAX: usize = 54;
    pub const SPEED: usize = 60;
    pub const WIDTH: usize = 61;
    pub const HEIGHT: usize = 62;
    pub const DIRECTION: usize = 63;
}

fn rec(o: &Object, i: usize) -> i32 {
    o.record.as_ref().map_or(0, |r| r[i])
}

/// What `id` reads of Claw now.
pub fn rider(w: &World, id: ObjectId) -> Rider {
    let on = w
        .claw_object()
        .is_some_and(|c| c.flags & flags::ON_ELEVATOR != 0 && c.object_below == Some(id));
    Rider {
        on,
        attempt: claw_attempt(w),
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let r = rider(w, id);
    step(w, id, r);
}

/// One logic call of `id` with `rider` as what it reads of Claw.
pub fn step(w: &mut World, id: ObjectId, rider: Rider) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    let Some(kind) = kind_of(&o.logic) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::Platform(d) => d,
        _ => PlatformData::default(),
    };
    match kind {
        Kind::Clock => clock_tick(w, id, &mut d),
        Kind::Trigger(bits) => trigger_tick(w, id, &mut d, bits, rider),
        Kind::Crumbling => crumbling_tick(w, id, &mut d, rider),
        Kind::Toggle(phase) => toggle_tick(w, id, &mut d, phase),
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::Platform(d);
    }
}

/// The set-up every platform shares: out of pair collision, a platform,
/// the level's surface and, for most, a default depth.
fn common_set_up(level: &SimLevel, o: &mut Object, kind: SurfaceKind, default_z: bool) {
    o.flags |= flags::NO_COLLIDE;
    o.type_flags = PLATFORM;
    o.hit_rect = surface(level_number(level), kind);
    if default_z && o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
}

fn elevator_surface(level: &SimLevel, o: &Object, clock: bool) -> SurfaceKind {
    if level_number(level) == 10 && o.image_set == "LEVEL_TRAPELEVATOR" {
        SurfaceKind::BigElevator
    } else if clock && o.image_set == "LEVEL_STEPPINGSTONE" {
        SurfaceKind::SteppingStone
    } else {
        SurfaceKind::Elevator
    }
}

fn clock_tick(w: &mut World, id: ObjectId, d: &mut PlatformData) {
    let level = w.level;
    let ms = level_clock(w);
    let tick = w.tick;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    match o.state {
        state::SET_UP => {
            let surface = elevator_surface(level, o, true);
            common_set_up(level, o, surface, true);
            let (mut x_min, mut x_max) = (rec(o, field::X_MIN), rec(o, field::X_MAX));
            let (mut y_min, mut y_max) = (rec(o, field::Y_MIN), rec(o, field::Y_MAX));
            let (width, height) = (rec(o, field::WIDTH), rec(o, field::HEIGHT));
            if width > 0 {
                x_min = o.x - width / 2;
                x_max = o.x + width / 2;
            }
            if height > 0 {
                y_min = o.y - height / 2;
                y_max = o.y + height / 2;
            }
            let dir = rec(o, field::DIRECTION);
            d.x = clock_axis(x_min, x_max, o.x, o.speed_x, matches!(dir, 1 | 4 | 7));
            d.y = clock_axis(y_min, y_max, o.y, o.speed_y, matches!(dir, 7..=9));
            o.state = state::RUNNING;
        }
        state::RUNNING => {
            let (x0, y0) = (o.x, o.y);
            if let Some(x) = clock_position(&d.x, ms) {
                o.x = x;
            }
            if let Some(y) = clock_position(&d.y, ms) {
                o.y = y;
            }
            o.platform_move = (o.x - x0, o.y - y0);
            // Woken after a sleep, the elevator jumps to where the clock
            // puts it.
            if d.last_tick.is_some_and(|t| t.wrapping_add(1) != tick) {
                o.teleported = true;
            }
        }
        _ => {}
    }
    d.last_tick = Some(tick);
}

fn trigger_tick(w: &mut World, id: ObjectId, d: &mut PlatformData, bits: u32, rider: Rider) {
    let level = w.level;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    if o.state == state::SET_UP {
        let surface = elevator_surface(level, o, false);
        common_set_up(level, o, surface, false);
        d.behaviour |= bits;
        let or_pos = |v: i32, pos: i32| if v == 0 { pos } else { v };
        d.x.min = or_pos(rec(o, field::X_MIN), o.x);
        d.x.max = or_pos(rec(o, field::X_MAX), o.x);
        d.y.min = or_pos(rec(o, field::Y_MIN), o.y);
        d.y.max = or_pos(rec(o, field::Y_MAX), o.y);
        let or_default = |v: i32| if v == 0 { DEFAULT_SPEED } else { v };
        d.x.speed = or_default(o.speed_x);
        d.y.speed = or_default(o.speed_y);
        let dir = rec(o, field::DIRECTION);
        match dir {
            6 | 4 => d.y.speed = 0,
            8 | 2 => d.x.speed = 0,
            _ => {}
        }
        o.state = if matches!(dir, 6 | 2 | 3) {
            state::TOWARD_MAX
        } else {
            state::TOWARD_MIN
        };
        d.attempt = rider.attempt;
        return;
    }
    if d.behaviour & behaviour::WAIT != 0 {
        if !rider.on {
            if d.attempt != rider.attempt {
                reset(o, d, rider);
            }
            return;
        }
        if d.behaviour & behaviour::TRIGGER != 0 {
            d.behaviour &= !behaviour::WAIT;
        }
    } else if d.behaviour & behaviour::STOP != 0 && rider.on {
        return;
    }
    if d.behaviour & behaviour::TRIGGER != 0 && d.attempt != rider.attempt {
        reset(o, d, rider);
        return;
    }
    let (vx, vy) = match o.state {
        state::TOWARD_MIN => (
            if o.x > d.x.min {
                (-trigger_step(d.x.speed)).max(-MAX_STEP)
            } else {
                0
            },
            if o.y > d.y.min {
                (-trigger_step(d.y.speed)).max(-MAX_STEP)
            } else {
                0
            },
        ),
        state::TOWARD_MAX => (
            if o.x < d.x.max {
                trigger_step(d.x.speed)
            } else {
                0
            },
            if o.y < d.y.max {
                trigger_step(d.y.speed)
            } else {
                0
            },
        ),
        _ => return,
    };
    o.x += vx;
    o.y += vy;
    o.platform_move = (vx, vy);
    let arrived = if o.state == state::TOWARD_MIN {
        o.x <= d.x.min && o.y <= d.y.min
    } else {
        o.x >= d.x.max && o.y >= d.y.max
    };
    if arrived {
        o.state = if d.behaviour & behaviour::ONE_WAY != 0 {
            state::STOPPED
        } else if o.state == state::TOWARD_MIN {
            state::TOWARD_MAX
        } else {
            state::TOWARD_MIN
        };
    }
}

/// Puts a trigger elevator back where it was placed; its next call sets it
/// up again.
fn reset(o: &mut Object, d: &mut PlatformData, rider: Rider) {
    o.x = o.editor.0;
    o.y = o.editor.1;
    o.teleported = true;
    o.state = state::SET_UP;
    d.attempt = rider.attempt;
}

/// Shows the first frame of the peg's set and works out its switch frame.
fn peg_set_up(level: &SimLevel, o: &mut Object, d: &mut PlatformData, kind: SurfaceKind) {
    // The placed frame is the set's first unless the level says
    // otherwise; keep the range found on the first set-up.
    let frames = frames_of(level, o, d.frames);
    d.frames = Some(frames);
    o.frame = frames.0;
    common_set_up(level, o, kind, true);
    d.switch_frame = (frames.0 + frames.1) / 2;
}

fn set_anim(w: &World, d: &mut PlatformData, key: &str) {
    d.anim.set(w.level.animations.get(key).cloned());
}

/// Advances the peg's animation by one tick; returns its frame.
fn advance_peg(w: &mut World, id: ObjectId, d: &mut PlatformData) -> i32 {
    let frames = d.frames.unwrap_or_default();
    advance_sprite(w, id, &mut d.anim, frames);
    w.objects.get(id).map_or(0, |o| o.frame)
}

fn set_type(w: &mut World, id: ObjectId, type_flags: u32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.type_flags = type_flags;
    }
}

fn set_state(w: &mut World, id: ObjectId, state: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state;
    }
}

fn crumbling_tick(w: &mut World, id: ObjectId, d: &mut PlatformData, rider: Rider) {
    let level = w.level;
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    match o.state {
        state::SET_UP => {
            peg_set_up(level, o, d, SurfaceKind::CrumblingPeg);
            o.draw_flags &= !draw_flags::HIDDEN;
            d.delay_ms = if o.speed_x == 0 {
                DEFAULT_CRUMBLE_DELAY_MS
            } else {
                o.speed_x
            };
            o.state = state::STOPPED;
            set_anim(w, d, FORWARD_ANIM);
        }
        state::STOPPED if rider.on => {
            o.state = state::START;
            o.time_delay = d.delay_ms as u32;
        }
        state::ARMED => {
            o.state = state::START;
            o.time_delay = d.delay_ms as u32;
        }
        state::START => {
            o.state = state::RUNNING_ANIM;
            sound::play(w, CRUMBLE_SOUND, SoundChannel::Effect);
        }
        state::RUNNING_ANIM => {
            let frame = advance_peg(w, id, d);
            if frame == d.switch_frame {
                set_type(w, id, NOT_PLATFORM);
            }
            if d.anim.is_done() {
                if let Some(o) = w.objects.get_mut(id) {
                    // Without its animation a peg still stops carrying.
                    o.type_flags = NOT_PLATFORM;
                    o.draw_flags |= draw_flags::HIDDEN;
                    o.state = state::GONE;
                }
                d.attempt = rider.attempt;
            }
        }
        state::GONE if d.attempt < rider.attempt => o.state = state::SET_UP,
        _ => {}
    }
}

fn toggle_tick(w: &mut World, id: ObjectId, d: &mut PlatformData, phase: Option<i32>) {
    let level = w.level;
    let ms = level_clock(w);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    match o.state {
        state::SET_UP => {
            peg_set_up(level, o, d, SurfaceKind::TogglePeg);
            d.phase = phase.unwrap_or_else(|| rec(o, field::SPEED));
            let or_default = |v: i32| if v == 0 { DEFAULT_TOGGLE_MS } else { v };
            d.on_ms = or_default(o.speed_x);
            d.period = d.on_ms.wrapping_add(or_default(o.speed_y));
            o.state = if rec(o, field::SMARTS) == 1 {
                state::STOPPED
            } else {
                state::OUT
            };
        }
        state::OUT if !toggle_on(ms, d.phase, d.on_ms, d.period) => o.state = state::START_IN,
        state::START_IN => {
            o.type_flags = NOT_PLATFORM;
            o.state = state::SLIDING_IN;
            set_anim(w, d, FORWARD_ANIM);
            sound::play_from(w, id, SLIDE_SOUND, SoundChannel::Effect);
        }
        state::SLIDING_IN => {
            let frame = advance_peg(w, id, d);
            if frame == d.switch_frame {
                set_type(w, id, NOT_PLATFORM);
            }
            if d.anim.is_done() {
                set_type(w, id, NOT_PLATFORM);
                set_anim(w, d, BACKWARD_ANIM);
                set_state(w, id, state::IN);
            }
        }
        state::IN if toggle_on(ms, d.phase, d.on_ms, d.period) => o.state = state::START,
        state::START => {
            o.state = state::RUNNING_ANIM;
            sound::play_from(w, id, SLIDE_SOUND, SoundChannel::Effect);
        }
        state::RUNNING_ANIM => {
            let frame = advance_peg(w, id, d);
            if frame == d.switch_frame {
                set_type(w, id, PLATFORM);
            }
            if d.anim.is_done() {
                set_type(w, id, PLATFORM);
                set_anim(w, d, FORWARD_ANIM);
                set_state(w, id, state::OUT);
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
    fn clock_track_example() {
        // Ends 1000 and 1160, placed at 1160, 150 px/s.
        let a = clock_axis(1000, 1160, 1160, 150, false);
        assert_eq!((a.span, a.period, a.phase), (161, 322, 1066));
        assert_eq!(clock_position(&a, 36), Some(1156));
        assert_eq!(clock_position(&a, 54), Some(1153));
        // A track with both ends 0 does not move.
        assert_eq!(clock_position(&clock_axis(0, 0, 500, 0, false), 1000), None);
    }

    #[test]
    fn trigger_steps_at_the_tick() {
        assert_eq!(trigger_step(125), 2);
        assert_eq!(trigger_step(150), 2);
        assert_eq!(trigger_step(175), 3);
        assert_eq!(trigger_step(250), 4);
        assert_eq!(trigger_step(5000), MAX_STEP);
    }

    #[test]
    fn toggle_test_is_unsigned() {
        // Default halves: 1500 out, 1500 in.
        assert!(toggle_on(0, 1500, 1500, 3000));
        assert!(toggle_on(704, 1500, 1500, 3000));
        assert!(!toggle_on(705, 1500, 1500, 3000));
        assert!(toggle_on(1454, 2250, 1500, 3000));
        assert!(!toggle_on(1455, 2250, 1500, 3000));
        assert!(!toggle_on(0, 750, 1500, 3000));
        assert!(toggle_on(750, 750, 1500, 3000));
        assert!(toggle_on(1500, 0, 1500, 3000));
        assert!(!toggle_on(1501, 0, 1500, 3000));
        assert!(toggle_on(10, 0, 0, 0), "an empty cycle stays out");
    }

    #[test]
    fn surfaces_come_from_the_level_table() {
        assert_eq!(
            surface(1, SurfaceKind::Elevator),
            Rect::new(-32, -8, 36, 64)
        );
        assert_eq!(
            surface(1, SurfaceKind::TogglePeg),
            Rect::new(-24, -8, 24, 64)
        );
        assert_eq!(
            surface(1, SurfaceKind::CrumblingPeg),
            Rect::new(-24, -46, 24, 64)
        );
        assert_eq!(
            surface(10, SurfaceKind::BigElevator),
            Rect::new(-88, -20, 94, 52)
        );
        assert_eq!(surface(3, SurfaceKind::TogglePeg), Rect::new(0, 0, 0, 32));
        assert_eq!(
            surface(99, SurfaceKind::Elevator),
            surface(1, SurfaceKind::Elevator)
        );
    }
}
