//! Claw's Bearhug state (5008): held in a bear sailor's hug (see
//! [`crate::logic::enemy::bear_sailor`]).
//!
//! The bear drives the whole state; Claw has no rule of his own in it.
//!
//! - The grab ([`enter_bearhug`]): whatever he was doing ends (an attack
//!   is cut short, a carried object is dropped, and Left and Right count
//!   as pressed again on the next tick when still held), he stops dead,
//!   harmless, with his standing boxes for the way he faces and his
//!   standing timers; his picture stays as it was. He is grabbed in
//!   whatever state he is, and grabbed again when already held.
//! - Held: his state's tick moves nothing, plays no animation and reads no
//!   key; it only notes the keys newly pressed on the tick
//!   ([`HugData::pressed`]), which the bear counts towards his escape.
//!   Only the shared bookkeeping after every state runs (timers, the
//!   weapon toggle).
//! - The bear places him [`crate::logic::enemy::bear_sailor::HUG_X`] px
//!   in front of itself at its own height, facing it ([`place`]), shows
//!   one of three pictures of his on him while it squeezes ([`show_frame`])
//!   and takes one health point a squeeze ([`squeeze`]): no flinch, no
//!   sound, no push; the squeeze that empties his health releases him and
//!   kills him.
//! - The release ([`release_bearhug`]): only while he is held: Left and
//!   Right count as pressed again when held, a carried object is dropped,
//!   and he stands where he was held. Held at the bear's height, 1 px above
//!   the floor, he drops that pixel over the next ticks.
//! - Anything else that hits him while held leaves the state as from any
//!   other (the hit reaction, with no push back since he is not standing),
//!   and the bear lets go on its next tick.

use claw_frame::Buttons;

use super::bounce::with_ctx;
use super::death::{self, DeathCause};
use super::{Ctx, enter_stand, state};
use crate::logic::World;
use crate::object::flags;
use crate::physics::ptype;

/// Whether the hug is filled in: the bear grabs Claw and the state's row
/// holds him.
pub const BUILT: bool = true;

/// Claw's pictures while held, one per squeeze picture of the bear.
pub const HELD_FRAMES: [i32; 3] = [450, 451, 452];

/// The keys whose press counts towards an escape: every game key (moving,
/// jumping, attacking, firing, lifting, the weapon keys).
pub const ESCAPE_KEYS: Buttons = Buttons(
    Buttons::JUMP.0
        | Buttons::ATTACK.0
        | Buttons::PROJECTILE.0
        | Buttons::TOGGLE.0
        | Buttons::LIFT.0
        | Buttons::PISTOL.0
        | Buttons::MAGIC.0
        | Buttons::DYNAMITE.0
        | Buttons::LEFT.0
        | Buttons::RIGHT.0
        | Buttons::UP.0
        | Buttons::DOWN.0,
);

/// What the held state keeps: the keys pressed on the last tick he spent
/// held, and that tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HugData {
    /// The keys newly pressed on [`HugData::tick`].
    pub pressed: Buttons,
    /// The tick the keys were noted on; `None` before any.
    pub tick: Option<u64>,
}

/// One tick of the Bearhug state (5008): nothing moves; the keys pressed
/// on the tick are noted for the bear.
pub fn hug_tick(c: &mut Ctx) {
    c.p.hug = HugData {
        pressed: c.input.pressed,
        tick: Some(c.tick),
    };
}

/// True when Claw, held, pressed a key that counts towards an escape on
/// the tick being run (his own call of the tick runs before the bear's).
pub fn escape_pressed(w: &World) -> bool {
    let h = &w.claw.hug;
    h.tick == Some(w.tick) && h.pressed.0 & ESCAPE_KEYS.0 != 0
}

/// True while Claw is held.
pub fn held(w: &World) -> bool {
    w.claw_object().is_some_and(|o| o.state == state::BEARHUG)
}

/// A bear grabs Claw (see the module documentation); false without a
/// Claw.
pub fn enter_bearhug(w: &mut World) -> bool {
    let Some(id) = w.claw_id else {
        return false;
    };
    with_ctx(w, id, enter).is_some()
}

fn enter(c: &mut Ctx) {
    c.reset_actions();
    c.o.state = state::BEARHUG;
    c.o.flags |= flags::HARMLESS;
    c.o.physics_type = ptype::DIRECT;
    c.speeds_zero();
    c.o.damage = 0;
    c.set_hit_rect_for_facing();
    c.set_stand_move_rect();
    let tp = &c.t.player;
    c.p.look_up_time = tp.look_up_delay_ms;
    c.p.running_time = tp.run_onset_ms;
    c.p.jump_press_time = 0.0;
    c.p.mover.zero_carries();
}

/// The bear lets Claw go: when he is still held, he stands where he is
/// (see the module documentation). Nothing happens when he is not held.
pub fn release_bearhug(w: &mut World) {
    let Some(id) = w.claw_id else { return };
    if !held(w) {
        return;
    }
    with_ctx(w, id, enter_stand);
}

/// Puts the held Claw at (x, y), facing right or left; his running time
/// starts again, as with every turn.
pub fn place(w: &mut World, x: i32, y: i32, facing_right: bool) {
    let Some(id) = w.claw_id else { return };
    with_ctx(w, id, |c| {
        c.o.x = x;
        c.o.y = y;
        if facing_right {
            c.face_right();
        } else {
            c.face_left();
        }
        c.p.running_time = c.t.player.run_onset_ms;
        c.p.mover.zero_carries();
    });
}

/// Shows picture `frame` of his own image set on Claw, without touching
/// his animation: it stays until his animation next sets a picture.
pub fn show_frame(w: &mut World, frame: i32) {
    if let Some(o) = w.claw_id.and_then(|id| w.objects.get_mut(id)) {
        o.frame = frame;
    }
}

/// One squeeze of the hug: one health point off Claw, whatever the
/// difficulty, unless he is invulnerable; when that empties his health he
/// is released and dies. True when he died. With the invincibility aid on
/// ([`crate::tuning::PlayAids::invincible`]) a squeeze takes nothing.
pub fn squeeze(w: &mut World) -> bool {
    let Some(id) = w.claw_id else {
        return false;
    };
    let s = &w.claw.stats;
    if (s.powerup_kind == death::powerup::VADER && s.powerup_ms > 0) || w.aids.invincible {
        return false;
    }
    w.claw.stats.health -= 1;
    if w.claw.stats.health > 0 {
        return false;
    }
    release_bearhug(w);
    with_ctx(w, id, |c| death::kill(c, DeathCause::Damage));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_game_key_counts_towards_an_escape() {
        for b in [
            Buttons::LEFT,
            Buttons::RIGHT,
            Buttons::UP,
            Buttons::DOWN,
            Buttons::JUMP,
            Buttons::ATTACK,
            Buttons::PROJECTILE,
            Buttons::LIFT,
        ] {
            assert!(ESCAPE_KEYS.contains(b), "{b:?}");
        }
        assert!(!ESCAPE_KEYS.is_empty());
    }
}
