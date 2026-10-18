//! Saw blades: round saws set into slots in the ninth level's decks that
//! rise, spin and sink again forever, on a rhythm fixed by the level
//! clock.
//!
//! A saw is a floor spike ([`super::floor_spike`]) with other art and a
//! few differences. The names differ only in their phase on the clock:
//! `SawBlade` starts its cycle at clock 0 (or at the record's speed field,
//! in ms), `SawBlade2` 750 ms later, `SawBlade3` 1500 ms later and
//! `SawBlade4` (which no level places) 2250 ms later ([`phase_of`]); every
//! saw of the ninth level is up for 1500 ms and down for 1500 ms.
//!
//! - Its strike box is the box of its retracted picture narrowed by 20 px
//!   on each side and raised by 20 px ([`strike_box`]): (-33, -10, 34, 17)
//!   on the ninth level, fixed when it is set up.
//! - It turns dangerous when its rise ([`UP_ANIM`], 31 ticks) ends, 32
//!   ticks after the clock enters its up half, and harmless on the tick
//!   after the clock leaves it; while up it keeps spinning ([`SPIN_ANIM`]),
//!   and its sink ([`DOWN_ANIM`]) takes 28 ticks. A saw woken inside its
//!   up half rises at once, so it may be dangerous for only two ticks.
//! - Its hit costs Claw [`DAMAGE`] as a hazard's shot (so at most twice per
//!   up half, his protection lasting 850 ms) and reaches enemies, kegs and
//!   crates the same way.
//! - Its rise and sink sounds ([`RISE_SOUND`], [`SINK_SOUND`]) are not on
//!   the ninth level: nothing plays and nothing is reported (the hum comes
//!   from sound objects beside the saws).
//!
//! It draws no random number.
//!
//! The cycle ([`state`], the floor spikes' numbers):
//!
//! | State | Per call |
//! | --- | --- |
//! | Down | harmless, the picture held; when the clock is in the up half, go to RiseStart |
//! | RiseStart | play the rise sound when the level has it and the saw was on screen, start the rise, go to Rising |
//! | Rising | advance the rise; when it ends, turn dangerous, start the spin and go to Up |
//! | Up | dangerous; advance the spin; when the clock leaves the up half, go to SinkStart |
//! | SinkStart | turn harmless, start the sink, play the sink sound as the rise's, go to Sinking |
//! | Sinking | advance the sink; when it ends, go to Down |
//!
//! Each clock edge takes effect one call late, as for the spikes: the
//! call that sees it only picks the next state. So with E the first Down
//! call that sees the up half and F the first Up call that sees the down
//! half, a saw shows the rise's pictures from E + 2, is dangerous from
//! E + 32 to F, harmless from F + 1 and back Down on F + 29.
//!
//! # State numbers
//!
//! A saw is no enemy: it runs the floor spikes' states ([`state`]) with
//! their numbers.
//!
//! # Defaults taken
//!
//! - A saw plays its rise and sink sounds only when the level has them, so
//!   on the ninth level nothing is played and nothing is reported.
//! - The hook-handed pirates and the PegLeg whose patrols end beside a saw
//!   get no special case: the saw's hit reaches them whenever its strike
//!   box overlaps their hit box.
//! - Asleep in the side areas of a wide display, a saw never woken is
//!   drawn sunk, as its set-up leaves it ([`first_look`]); woken inside its
//!   up half it then rises at once.
//!
//! Its data ([`SawData`]) is its own.

pub use super::floor_spike::state;

use claw_frame::SoundChannel;

use super::platform::{level_clock, toggle_on};
use super::{ObjectData, World, sound};
use crate::level::SimLevel;
use crate::logic::Look;
use crate::object::{Object, ObjectId, Rect, engine_state, flags, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SawBlade", "SawBlade2", "SawBlade3", "SawBlade4"];

/// The saws' picture on the ninth level (the records name it).
pub const IMAGE_SET: &str = "LEVEL_FLOORSAW";

/// Image sets this family may switch an object to: none (the saws keep
/// the set their records name).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SAWBLADE_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The rise.
pub const UP_ANIM: &str = "LEVEL_SAWBLADE_UP";
/// The spin while up.
pub const SPIN_ANIM: &str = "LEVEL_SAWBLADE_SPIN";
/// The sink.
pub const DOWN_ANIM: &str = "LEVEL_SAWBLADE_DOWN";
/// The rise's sound (no level has it).
pub const RISE_SOUND: &str = "LEVEL_SAWBLADEUP";
/// The sink's sound (no level has it).
pub const SINK_SOUND: &str = "LEVEL_SAWBLADEDOWN";

/// What a saw is to whatever it touches: a hazard's shot of the level.
pub const TYPE: u32 = types::LEVEL_SHOT;
/// Damage of a saw placed without any.
pub const DAMAGE: i32 = 10;
/// Depth of a saw placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Length of each half of the cycle when the record gives none, ms.
pub const DEFAULT_HALF_MS: i32 = 1500;
/// How much the strike box is narrowed on each side and raised, px.
pub const SHRINK: i32 = 20;
/// Ticks from the first tick the clock is in the up half to the first
/// dangerous tick (measured).
pub const DANGER_DELAY_TICKS: u32 = 32;
/// Ticks of the rise at the 18 ms step.
pub const RISE_TICKS: u32 = 31;
/// Ticks of the sink at the 18 ms step.
pub const SINK_TICKS: u32 = 28;

/// The cycle offset of the saw running `name`, ms: `SawBlade` takes
/// `record_speed`, the others a quarter, half and three quarters of the
/// default cycle; `None` for a name no saw runs.
pub fn phase_of(name: &str, record_speed: i32) -> Option<i32> {
    match name {
        "SawBlade" => Some(record_speed),
        "SawBlade2" => Some(750),
        "SawBlade3" => Some(1500),
        "SawBlade4" => Some(2250),
        _ => None,
    }
}

/// The strike box of a saw whose retracted picture covers `picture`: 20 px
/// narrower on each side and 20 px taller at the top.
pub fn strike_box(picture: Rect) -> Rect {
    Rect::new(
        picture.left + SHRINK,
        picture.top - SHRINK,
        picture.right - SHRINK,
        picture.bottom,
    )
}

/// A saw's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SawData {
    /// Clock offset of the cycle, ms.
    pub phase: i32,
    /// Length of the up half, ms.
    pub on_ms: i32,
    /// Length of the whole cycle, ms.
    pub period: i32,
}

/// Record field holding `SawBlade`'s phase, ms.
pub const SPEED_FIELD: usize = 60;

/// True while a saw with this cycle is in its up half at level clock
/// `ms` (see [`super::platform::toggle_on`]).
pub fn up_at(ms: u32, d: &SawData) -> bool {
    toggle_on(ms, d.phase, d.on_ms, d.period)
}

/// True while the saw `o` hurts what it touches.
pub fn dangerous(o: &Object) -> bool {
    o.flags & flags::HARMLESS == 0
}

/// The set-up of saw `o` on its first call (see the module
/// documentation): its strike box from the picture it shows, harmless, a
/// hazard's shot, its depth and damage, its cycle; it waits Down.
fn set_up(level: &SimLevel, o: &mut Object, d: &mut SawData) {
    if let Some(b) = level.frame_box(&o.image_set, o.frame) {
        o.attack_rect = strike_box(b);
    }
    o.flags |= flags::HARMLESS;
    o.type_flags = TYPE;
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    if o.damage == 0 {
        o.damage = DAMAGE;
    }
    let speed = o.record.as_ref().map_or(0, |r| r[SPEED_FIELD]);
    d.phase = phase_of(&o.logic, speed).unwrap_or(speed);
    let or_default = |v: i32| if v == 0 { DEFAULT_HALF_MS } else { v };
    d.on_ms = or_default(o.speed_x);
    d.period = d.on_ms.wrapping_add(or_default(o.speed_y));
    o.state = state::DOWN;
}

/// How a saw whose first call has not run yet looks once it has (see
/// [`super::first_look`]): sunk, showing the picture it shows now (its
/// set-up changes nothing drawn). `None` once it has run.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    if o.state != engine_state::INIT {
        return None;
    }
    let mut s = o.clone();
    set_up(level, &mut s, &mut SawData::default());
    Some(vec![Look::of(&s)])
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::SawBlade(d) => d,
        _ => SawData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::SawBlade(d);
    }
}

/// Plays `key` from the saw when the level has that sound and the saw was
/// on screen in the previous tick; nothing at all otherwise.
fn cue(w: &mut World, id: ObjectId, key: &str) {
    if !w.level.sounds.contains_key(key) {
        return;
    }
    if w.objects.get(id).is_some_and(|o| w.on_screen(o)) {
        sound::play_from(w, id, key, SoundChannel::Effect);
    }
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

/// Starts animation `key` without advancing it (none when the level does
/// not have it).
fn set_anim(w: &mut World, id: ObjectId, key: &str) {
    super::enemy::set_anim(w, id, key);
}

/// Advances the animation one call; true when it has ended.
fn advance(w: &mut World, id: ObjectId) -> bool {
    w.advance_anim(id);
    w.objects.get(id).is_none_or(|o| o.anim.is_done())
}

fn step(w: &mut World, id: ObjectId, d: &mut SawData) {
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
        state::DOWN if up_at(ms, d) => set_state(w, id, state::RISE_START),
        state::RISE_START => {
            cue(w, id, RISE_SOUND);
            set_anim(w, id, UP_ANIM);
            set_state(w, id, state::RISING);
        }
        // The rise and the sink advance on every call; the guard is the
        // call's work.
        state::RISING if advance(w, id) => {
            set_harmless(w, id, false);
            set_anim(w, id, SPIN_ANIM);
            set_state(w, id, state::UP);
        }
        state::UP => {
            w.advance_anim(id);
            if !up_at(ms, d) {
                set_state(w, id, state::DROP_START);
            }
        }
        state::DROP_START => {
            set_harmless(w, id, true);
            set_anim(w, id, DOWN_ANIM);
            cue(w, id, SINK_SOUND);
            set_state(w, id, state::DROPPING);
        }
        state::DROPPING if advance(w, id) => set_state(w, id, state::DOWN),
        _ => {}
    }
}

/// `attacker` hit `id`. Nothing hits a saw.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A saw hurts through its type alone.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strike_box_of_the_ninth_levels_saws() {
        // The retracted picture: 108 x 9 px at (1, 14).
        let picture = Rect::new(-53, 10, 54, 17);
        assert_eq!(strike_box(picture), Rect::new(-33, -10, 34, 17));
    }

    #[test]
    fn the_names_differ_only_in_their_phase() {
        let phases: Vec<_> = NAMES.iter().map(|n| phase_of(n, 0)).collect();
        assert_eq!(phases, [Some(0), Some(750), Some(1500), Some(2250)]);
        assert_eq!(phase_of("SawBlade", 300), Some(300));
        assert_eq!(phase_of("FloorSpike", 0), None);
    }

    #[test]
    fn its_timings_at_eighteen_milliseconds() {
        assert_eq!(DANGER_DELAY_TICKS, RISE_TICKS + 1);
        // The rise and the sink fit each half of the cycle.
        let half = (DEFAULT_HALF_MS as u32).div_ceil(18);
        assert!(RISE_TICKS.max(SINK_TICKS) < half);
        // The floor spikes' states.
        assert_eq!(state::DOWN, 1007);
        assert_eq!(state::RISE_START, 1002);
    }
}
