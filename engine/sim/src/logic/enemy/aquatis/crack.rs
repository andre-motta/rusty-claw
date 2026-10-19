//! The crack in the rock wall behind Aquatis ([`super`]): a blast from
//! one of Claw's sticks of dynamite that reaches its box starts an
//! earthquake.
//!
//! - Built: its picture, the glow ([`GLOW_ANIM`]) playing; an enemy's body
//!   that touches nothing (no attack box); hit only by the level's shots
//!   and explosions, whose touch it feels on its box ([`HIT_RECT`]: its
//!   picture's widened 25 px to the right of its left edge and 100 px past
//!   its right edge). It waits (`Idle`), its reset bit
//!   ([`super::CRACK_BIT`]) cleared.
//! - Waiting: when Aquatis's reset bits hold its bit it waits again
//!   (clearing it); otherwise the glow plays. While it sleeps (Claw far
//!   from it) the bit stays set, so after the staged introduction Aquatis
//!   waits until Claw comes close enough to wake it.
//! - A blast on it, while it waits and [`super::CRACK_COOLDOWN_MS`] have
//!   passed since its last earthquake: with Aquatis alive, not in his
//!   frenzy, and no stalactite armed or falling, the earthquake starts
//!   (`Delay`): one of the rumbles ([`RUMBLES`], a draw of 0..5), a camera
//!   shake of [`super::QUAKE_MS`], the link's quake flag, and its cooldown
//!   from now. Otherwise it waits again, its bit untouched.
//! - The earthquake: when its reset bit is set it waits again (clearing
//!   it); otherwise the glow plays, and once [`super::QUAKE_MS`] have
//!   passed the quake flag is cleared, the next stalactite is armed and it
//!   waits again.
//!
//! The camera shake draws from the shared random stream on every step of
//! its shake.

use claw_frame::SoundChannel;

use super::super::Timer;
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, types};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["AquatisCrack"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_AQUATISWALLCRACK";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The glow.
pub const GLOW_ANIM: &str = "LEVEL_CRACKGLOW";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[GLOW_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its box: its picture's widened 25 px to the right of its left edge and
/// 100 px past its right edge.
pub const HIT_RECT: Rect = Rect::new(-4, -64, 130, 63);
/// How far its box's left edge moves right of its picture's, px.
pub const WIDEN_LEFT: i32 = 25;
/// How far its box's right edge moves right of its picture's, px.
pub const WIDEN_RIGHT: i32 = 100;
/// What may hit it: the level's shots and explosions.
pub const HIT_MASK: u32 = types::LEVEL_SHOT;
/// The earthquake's rumbles, one picked at random.
pub const RUMBLES: [&str; 5] = [
    "LEVEL_EARTHQUAKE1",
    "LEVEL_EARTHQUAKE2",
    "LEVEL_EARTHQUAKE3",
    "LEVEL_EARTHQUAKE4",
    "LEVEL_EARTHQUAKE5",
];

/// Its logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Waiting for a blast.
    pub const IDLE: i32 = 1;
    /// The earthquake runs.
    pub const DELAY: i32 = 2;
}

/// The crack's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrackData {
    /// The earthquake's time.
    pub delay: Timer,
    /// The time from one earthquake to the next blast it accepts.
    pub cooldown: Timer,
}

fn crack_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut CrackData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::AquatisCrack(d) => Some(d),
        _ => None,
    }
}

fn crack_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a CrackData> {
    match &w.objects.get(id)?.data {
        ObjectData::AquatisCrack(d) => Some(d),
        _ => None,
    }
}

fn state_of(w: &World, id: ObjectId) -> Option<i32> {
    w.objects.get(id).map(|o| o.state)
}

fn set_state(w: &mut World, id: ObjectId, s: i32) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = s;
    }
}

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    super::reset_on_respawn(w);
    let bit_set = w.aquatis().is_some() && w.aquatis_reset_bits() & super::CRACK_BIT != 0;
    match state_of(w, id) {
        Some(state::SET_UP) => set_up(w, id),
        Some(state::IDLE) => {
            if bit_set {
                enter_idle(w, id);
            } else {
                w.advance_anim(id);
            }
        }
        Some(state::DELAY) => {
            if bit_set {
                enter_idle(w, id);
                return;
            }
            w.advance_anim(id);
            let now = w.now_ms;
            if crack_data(w, id).is_some_and(|d| d.delay.done(now)) {
                let link = w.aquatis_link_mut();
                link.quake = false;
                link.armed = true;
                enter_idle(w, id);
            }
        }
        _ => {}
    }
}

fn set_up(w: &mut World, id: ObjectId) {
    let rect = w
        .image_frames(IMAGE_SET)
        .and_then(|(first, _)| w.frame_box(IMAGE_SET, first))
        .map_or(HIT_RECT, |b| {
            Rect::new(b.left + WIDEN_LEFT, b.top, b.right + WIDEN_RIGHT, b.bottom)
        });
    let first = w.image_frames(IMAGE_SET).map(|f| f.0);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    if let Some(first) = first {
        o.frame = first;
    }
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_MASK;
    o.attack_type_flags = 0;
    o.attack_rect = Rect::UNSET;
    o.hit_rect = rect;
    o.hit_handler = true;
    o.data = ObjectData::AquatisCrack(CrackData::default());
    enter_idle(w, id);
}

/// Waits: its reset bit cleared, the glow from its start.
fn enter_idle(w: &mut World, id: ObjectId) {
    w.clear_aquatis_reset_bit(super::CRACK_BIT);
    wait(w, id);
}

/// Waits again, the glow from its start, its reset bit untouched.
fn wait(w: &mut World, id: ObjectId) {
    set_state(w, id, state::IDLE);
    w.set_anim(id, GLOW_ANIM);
}

/// `attacker` hit `id`: a blast (see the module documentation). Draws:
/// one of 0..5 when the earthquake starts.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = attacker;
    let now = w.now_ms;
    let ready = crack_data(w, id).is_some_and(|d| d.cooldown.done(now));
    if state_of(w, id) == Some(state::DELAY) || !ready {
        return;
    }
    let link = *w.aquatis_link();
    if !w.aquatis_alive() || link.frenzy || link.falling || link.armed {
        wait(w, id);
        return;
    }
    set_state(w, id, state::DELAY);
    let rumble = RUMBLES[w.rng.below(RUMBLES.len() as u32) as usize];
    sound::play(w, rumble, SoundChannel::Effect);
    if let Some(d) = crack_mut(w, id) {
        d.delay = Timer::new(now, u64::from(super::QUAKE_MS));
        d.cooldown = Timer::new(now, u64::from(super::CRACK_COOLDOWN_MS));
    }
    w.shake(super::QUAKE_MS);
    w.aquatis_link_mut().quake = true;
}

/// `id` attacked `victim`: it never attacks.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_box_is_its_pictures_widened_to_the_right() {
        let picture = Rect::new(-29, -64, 30, 63);
        assert_eq!(
            Rect::new(
                picture.left + WIDEN_LEFT,
                picture.top,
                picture.right + WIDEN_RIGHT,
                picture.bottom
            ),
            HIT_RECT
        );
        assert_eq!(RUMBLES.len(), 5);
    }
}
