//! Springboards: platforms that throw whoever stands on them straight up.
//! On the fourth level each one is a springy fern; later levels use the
//! same logic with a rock or a box.
//!
//! Claw lands on a springboard like on any platform. In its idle state
//! the springboard loops its idle animation and checks every call whether
//! Claw stands on it; when he does, it starts its spring animation, and
//! on the first call that reaches the animation's launch frame it throws
//! him into a jump whose apex is a fixed height above where he stands
//! ([`DEFAULT_HEIGHT`] unless the record's y_max field gives another),
//! whatever the jump key does. A Claw who stepped off before that call
//! is not thrown. At the end of the spring animation it goes back to
//! idle.
//!
//! Enemies use a second cycle: a human enemy standing on an idle
//! springboard arms it ([`state::ENEMY_ARMED`]); on that cycle's launch
//! frame the springboard raises [`special::LAUNCH`], which the enemy reads
//! in its next call and answers by jumping off. A cycle Claw set off never
//! launches an enemy, and one an enemy set off never launches Claw.
//!
//! The springboard never moves, never takes part in pair collision and is
//! solid in every state. Its landing surface and its two animations come
//! from the level ([`level_table`]).
//!
//! # Timing
//!
//! With L the tick in which Claw lands on a fern (the springboard updates
//! before him, so it sees him on L + 1): the spring animation starts on
//! L + 1 and the logic then waits [`SPRING_DELAY_MS`] (13 ticks); the
//! spring sound plays on L + 18 and the throw comes on L + 21, 378 ms
//! after the landing, inside the springboard's call, so Claw's first
//! rising step is in the same tick. The fern is idle again on L + 49 and
//! sees Claw again from then on. Holding Jump on the fern, or walking on
//! it, changes nothing; walking off before L + 21 cancels the throw.
//!
//! Both sounds come from the animations: the rustle of the idle loop
//! (every 254 ticks) and the spring sound. Neither is limited to the
//! screen, so every live springboard rustles, and ferns in the same phase
//! rustle together.
//!
//! # Defaults taken
//!
//! - The set-up sets the special flag [`special::MARKER`] besides
//!   [`special::ENEMY_ARMABLE`]. Nothing is known to read it, so it is set
//!   and given no behaviour.

use super::decor::{SpriteAnim, advance_sprite, frames_of};
use super::platform::{self, PLATFORM, SURFACE_DEPTH};
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, flags};
use crate::player;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["SpringBoard"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play: the idle and spring
/// animations of every level's table.
pub const ANIM_PREFIXES: &[&str] = &[
    FERN.idle_anim,
    FERN.spring_anim,
    ROCK.idle_anim,
    ROCK.spring_anim,
    BOX.idle_anim,
    BOX.spring_anim,
];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Height of the throw's apex above Claw, px, for a record whose y_max
/// field is zero or negative.
pub const DEFAULT_HEIGHT: i32 = 450;
/// Depth given to a springboard placed at depth 0.
pub const DEFAULT_Z: i32 = 2000;
/// Time the logic waits after starting a spring cycle, ms.
pub const SPRING_DELAY_MS: u32 = 250;
/// Event value of the spring animation's launch frame.
pub const LAUNCH_EVENT: i32 = 1;
/// Record field holding the throw's height (y_max).
pub const HEIGHT_FIELD: usize = 54;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Idle: looping the idle animation, waiting for Claw.
    pub const IDLE: i32 = 5;
    /// The spring cycle Claw started.
    pub const CLAW_SPRING: i32 = 6;
    /// The spring cycle an enemy started.
    pub const ENEMY_SPRING: i32 = 1008;
    /// Armed by an enemy standing on it: the next call starts
    /// [`ENEMY_SPRING`].
    pub const ENEMY_ARMED: i32 = 1111;
}

/// Bits of the springboard's special flags.
pub mod special {
    /// A human enemy standing on it may arm it.
    pub const ENEMY_ARMABLE: u32 = 0x10000;
    /// Set during an enemy cycle from its launch frame to its end: the
    /// enemy on it jumps off.
    pub const LAUNCH: u32 = 0x20000;
    /// Set at set-up with no behaviour (see the module documentation).
    pub const MARKER: u32 = 0x40;
}

/// A level's springboard data: the landing rectangle (before
/// [`SURFACE_DEPTH`] is added to its bottom) and the idle and spring
/// animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelSprings {
    /// Landing rectangle relative to the springboard's position.
    pub rect: (i32, i32, i32, i32),
    /// Animation looped while idle.
    pub idle_anim: &'static str,
    /// Animation of a spring cycle.
    pub spring_anim: &'static str,
}

const FERN: LevelSprings = LevelSprings {
    rect: (-32, 10, 24, 32),
    idle_anim: "LEVEL_SPRINGYFERN_IDLE",
    spring_anim: "LEVEL_SPRINGYFERN_SPRING",
};

const ROCK: LevelSprings = LevelSprings {
    rect: (-24, 89, 23, 130),
    idle_anim: "LEVEL_ROCKSPRING_IDLE",
    spring_anim: "LEVEL_ROCKSPRING_SPRING",
};

const BOX: LevelSprings = LevelSprings {
    rect: (-24, 0, 22, 40),
    idle_anim: "LEVEL_SPRINGBOX_IDLE",
    spring_anim: "LEVEL_SPRINGBOX_SPRING",
};

/// The springboard data of levels 1 to 14; the first three levels have
/// none.
const LEVELS: [Option<LevelSprings>; 14] = [
    None,
    None,
    None,
    Some(FERN),
    Some(FERN),
    Some(FERN),
    Some(ROCK),
    Some(FERN),
    Some(BOX),
    Some(FERN),
    Some(FERN),
    Some(ROCK),
    Some(FERN),
    Some(FERN),
];

/// The springboard data of level `level` (1 to 14); `None` for a level
/// without any, and for other level numbers.
pub fn level_table(level: u32) -> Option<LevelSprings> {
    LEVELS
        .get(level.wrapping_sub(1) as usize)
        .copied()
        .flatten()
}

/// The landing surface of a springboard in level `level`, relative to its
/// position: the level's rectangle with [`SURFACE_DEPTH`] added to its
/// bottom; an empty rectangle for a level without springboards.
pub fn surface(level: u32) -> Rect {
    let (l, t, r, b) = level_table(level).map_or((0, 0, 0, 0), |s| s.rect);
    Rect::new(l, t, r, b + SURFACE_DEPTH)
}

/// The throw's height for a record whose y_max field is `y_max`: the
/// field when above 0, else [`DEFAULT_HEIGHT`].
pub fn height_of(y_max: i32) -> i32 {
    if y_max > 0 { y_max } else { DEFAULT_HEIGHT }
}

/// A springboard's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpringBoardData {
    /// Height of Claw's throw, px.
    pub height: i32,
    /// The running spring cycle has not reached its launch frame yet.
    pub armed: bool,
    /// The level's idle and spring animations.
    pub anims: Option<LevelSprings>,
    /// First and last frame numbers of the springboard's image set, when
    /// known (see [`super::decor::image_frames`]).
    pub frames: Option<(i32, i32)>,
    /// The animation being played.
    pub anim: SpriteAnim,
    /// Event value of the animation element last applied: the element
    /// applied by this call, or the one still being shown.
    pub event: i32,
}

/// The springboard data of `o`, when it is one.
pub fn data_of(o: &Object) -> Option<&SpringBoardData> {
    match &o.data {
        ObjectData::SpringBoard(d) => Some(d),
        _ => None,
    }
}

/// True when Claw stands on `id`, as he was left by his last update.
fn claw_on(w: &World, id: ObjectId) -> bool {
    platform::rider(w, id).on
}

fn set_anim(w: &World, d: &mut SpringBoardData, spring: bool) {
    let key = d
        .anims
        .map(|a| if spring { a.spring_anim } else { a.idle_anim });
    d.anim
        .set(key.and_then(|k| w.level.animations.get(k).cloned()));
    d.event = 0;
}

/// Advances the animation one tick and returns the event value it
/// reports: that of the element applied now, or of the element still
/// shown.
fn advance(w: &mut World, id: ObjectId, d: &mut SpringBoardData) -> i32 {
    let frames = d.frames.unwrap_or_default();
    if let Some(i) = advance_sprite(w, id, &mut d.anim, frames) {
        d.event = d
            .anim
            .animation()
            .and_then(|a| a.frames.get(i))
            .map_or(0, |e| e.event);
    }
    d.event
}

fn set_up(w: &mut World, id: ObjectId, d: &mut SpringBoardData) {
    let level = w.level;
    let number = w.level_number();
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    d.anims = level_table(number);
    d.frames = Some(frames_of(level, o, d.frames));
    o.hit_rect = surface(number);
    o.flags |= flags::NO_COLLIDE;
    o.special_flags |= special::ENEMY_ARMABLE | special::MARKER;
    o.type_flags = PLATFORM;
    o.platform_move = (0, 0);
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    d.height = height_of(o.record.as_ref().map_or(0, |r| r[HEIGHT_FIELD]));
    o.state = state::IDLE;
    set_anim(w, d, false);
}

/// Starts a spring cycle in `next` (the logic waits [`SPRING_DELAY_MS`]
/// first).
fn start_cycle(w: &mut World, id: ObjectId, d: &mut SpringBoardData, next: i32) {
    set_anim(w, d, true);
    d.armed = true;
    if let Some(o) = w.objects.get_mut(id) {
        o.time_delay = SPRING_DELAY_MS;
        o.state = next;
    }
}

/// Back to idle once the spring animation has finished.
fn end_cycle(w: &mut World, id: ObjectId, d: &mut SpringBoardData) {
    set_anim(w, d, false);
    if let Some(o) = w.objects.get_mut(id) {
        o.special_flags &= !special::LAUNCH;
        o.state = state::IDLE;
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::SpringBoard(d) => d,
        _ => SpringBoardData::default(),
    };
    step(w, id, &mut d);
    if let Some(o) = w.objects.get_mut(id) {
        o.data = ObjectData::SpringBoard(d);
    }
}

fn step(w: &mut World, id: ObjectId, d: &mut SpringBoardData) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        state::SET_UP => set_up(w, id, d),
        state::IDLE => {
            advance(w, id, d);
            if claw_on(w, id) {
                start_cycle(w, id, d, state::CLAW_SPRING);
            }
        }
        state::CLAW_SPRING => {
            if advance(w, id, d) == LAUNCH_EVENT && d.armed {
                if claw_on(w, id) {
                    player::bounce(w, d.height);
                }
                d.armed = false;
            }
            if d.anim.is_done() {
                end_cycle(w, id, d);
            }
        }
        state::ENEMY_ARMED => start_cycle(w, id, d, state::ENEMY_SPRING),
        state::ENEMY_SPRING => {
            if advance(w, id, d) == LAUNCH_EVENT && d.armed {
                if let Some(o) = w.objects.get_mut(id) {
                    o.special_flags |= special::LAUNCH;
                }
                d.armed = false;
            }
            if d.anim.is_done() {
                end_cycle(w, id, d);
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
    fn the_fourth_level_lands_claw_on_a_fern() {
        let s = level_table(4).expect("the fourth level has springboards");
        assert_eq!(s, FERN);
        assert_eq!(surface(4), Rect::new(-32, 10, 24, 64));
    }

    #[test]
    fn levels_without_springboards_have_no_data() {
        for n in [0, 1, 2, 3, 15, u32::MAX] {
            assert_eq!(level_table(n), None, "level {n}");
            assert_eq!(surface(n), Rect::new(0, 0, 0, 32), "level {n}");
        }
        assert_eq!(level_table(7), Some(ROCK));
        assert_eq!(level_table(12), Some(ROCK));
        assert_eq!(level_table(9), Some(BOX));
    }
}
