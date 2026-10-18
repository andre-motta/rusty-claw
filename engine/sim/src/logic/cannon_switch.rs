//! The props of the eighth level's boss fight: the cannon switch, a lever
//! on Gabriel's deck; the cannon button ([`button`]), a red lamp on a pole
//! at the left of the dock; and Gabriel's cannon ([`cannon`]), in the gap
//! under the bow of his ship.
//!
//! None of them talks to another: all three read and write Gabriel's
//! [`GabrielLink`] ([`World::gabriel_link`]). When Gabriel pulls the
//! lever he sets the fire order; the switch flips while the order is set
//! and the cannon fires and clears it. A strike of Claw's on the button
//! sets the button flag; the cannon turns its barrel up, its next shot
//! flies straight up through the deck into Gabriel, and it turns back
//! down and clears the flag. With no Gabriel, or a defeated one, none of
//! them does anything new. None of them reads the boss fight's flag, so
//! the intro skip changes nothing here, and none draws a random number.
//!
//! # The cannon switch
//!
//! It shows the first picture of its set ([`IMAGE_SET`]) and takes part in
//! no collision. While Gabriel's fire order is set it flips: the
//! [`SWITCH_ANIM`] (pictures 1, 2, 3, 2, back to 1, one every 100 ms) with
//! the [`SWITCH_SOUND`], [`SWITCH_TICKS`] ticks, then it waits again. It
//! updates before the cannon (both are placed at depth 1000; the level's
//! order puts the button first, then the switch, then the cannon), so it
//! sees each order in the tick the cannon fires it.
//!
//! Not built yet ([`BUILT`] is false, and the same for [`button`] and
//! [`cannon`]): the three props stay inert placeholders (drawn, no
//! collision, reported as not implemented), and the calls below do
//! nothing.
//!
//! # State numbers
//!
//! The switch and the button are no enemies: their states are their own
//! ([`state`], [`button::state`]). The cannon runs the shared cannon
//! states (`Home`, `Fire`, `VertFiring`) and its own turn,
//! [`EnemyState::Rotate`], in the cannons' block.
//!
//! # Defaults taken
//!
//! - The button's hit box is the one of its first picture, unmirrored,
//!   fixed when it is set up ([`button::HIT_RECT`]): the record's mirroring
//!   only changes how it is drawn.
//! - The cannon has no waiting animation: going home it keeps the picture
//!   it last showed and its waiting tick moves no animation
//!   ([`cannon::HOME_ANIM`] is `None`).
//! - A horizontal ball reaches Gabriel the way it reaches anything it
//!   touches, should he ever stand in its lane on the dock.
//!
//! [`EnemyState::Rotate`]: super::enemy::EnemyState::Rotate
//! [`GabrielLink`]: super::enemy::gabriel::GabrielLink

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["CannonSwitch"];

/// The switch's picture.
pub const IMAGE_SET: &str = "LEVEL_CANNONSWITCH";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// The flip.
pub const SWITCH_ANIM: &str = "LEVEL_CANNONSWITCH_SWITCH";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[SWITCH_ANIM];

/// The flip's sound.
pub const SWITCH_SOUND: &str = "LEVEL_GABRIELCANNON_SWITCH1";

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// Ticks of a flip at the 18 ms step (measured): pictures of 7, 6, 6 and
/// 6 ticks, then back to waiting.
pub const SWITCH_TICKS: u32 = 25;

/// The switch's states.
pub mod state {
    /// Waiting for an order.
    pub const IDLE: i32 = 1;
    /// Flipping.
    pub const SWITCH: i32 = 2;
}

/// The switch's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SwitchData {}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`. Nothing hits the switch.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The switch attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// The cannon button: a lamp that blinks red and green ([`IDLE_ANIM`](button::IDLE_ANIM),
/// pictures 1 and 3, [`BLINK_TICKS`](button::BLINK_TICKS) ticks each). Only Claw's own blows
/// press it (his sword, punch, uppercut and kick, on their live ticks; his
/// shots, magic, dynamite and thrown objects never do): with Gabriel there
/// and not defeated, a blow while it is not pressed plays [`SOUND`](button::SOUND) once
/// and sets the button flag, and from the next tick it shows the pressed
/// lamp ([`PRESSED_FRAME`](button::PRESSED_FRAME)) for as long as the flag stays set. A pressed
/// button ignores further blows; once the cannon has fired upwards and
/// turned back down the flag is clear and it blinks again. With Gabriel
/// defeated it blinks, whatever the flag.
///
/// Not built yet ([`BUILT`](button::BUILT) is false).
pub mod button {
    use crate::logic::World;
    use crate::object::{ObjectId, Rect, types};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["CannonButton"];

    /// The button's picture.
    pub const IMAGE_SET: &str = "LEVEL_CANNONBUTTON";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// The blink.
    pub const IDLE_ANIM: &str = "LEVEL_CANNONBUTTON_IDLE";

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[IDLE_ANIM];

    /// The sound of a press.
    pub const SOUND: &str = "LEVEL_GABRIELCANNON_BUTTON1";

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// What the button is, for collisions (it attacks nothing).
    pub const TYPE: u32 = types::ENEMY;
    /// What may hit the button: Claw himself.
    pub const HIT_MASK: u32 = types::PLAYER;
    /// Its hit box: its first picture's, unmirrored.
    pub const HIT_RECT: Rect = Rect::new(-12, -16, 11, 15);
    /// The pressed lamp.
    pub const PRESSED_FRAME: i32 = 2;
    /// Ticks each lamp of the blink lasts at the 18 ms step.
    pub const BLINK_TICKS: u32 = 12;

    /// The button's states.
    pub mod state {
        /// Blinking, or held pressed.
        pub const IDLE: i32 = 1;
    }

    /// The button's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct ButtonData {
        /// The press sound played and not yet allowed again.
        pub latch: bool,
    }

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`: a blow of Claw's (see the module
    /// documentation).
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. The button attacks nothing.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

/// Gabriel's cannon, a cannon of the second level's kind ([`super::cannon`])
/// that fires only when Gabriel orders it. It faces left (its record is
/// mirrored) and fires along the dock from [`muzzles`](cannon::muzzles), or, with its
/// barrel up, straight up from the vertical muzzle through the hull and
/// the deck into Gabriel. Each shot is one cannonball ([`SPEED`](cannon::SPEED) px/s,
/// no gravity) and plays [`FIRE_SOUND`](cannon::FIRE_SOUND) with the firing animation's own
/// sound.
///
/// While it waits (`Home`), in this order: with no Gabriel or a defeated
/// one it does nothing at all; when he waits for the cannon it resets
/// (fired and raised cleared, his fire order and button flag cleared, his
/// wait over, the first picture shown); after an upward shot with the
/// barrel up it turns down ([`REST_ANIM`](cannon::REST_ANIM), [`DROP_SOUND`](cannon::DROP_SOUND), horizontal
/// fire, the button flag cleared); with the button flag set and the
/// barrel down it turns up ([`RISE_ANIM`](cannon::RISE_ANIM), [`RISE_SOUND`](cannon::RISE_SOUND), vertical
/// fire); with a fire order it fires (`VertFiring` with the barrel up,
/// else `Fire`) and clears the order. A turn ([`EnemyState::Rotate`])
/// takes [`ROTATE_TICKS`](cannon::ROTATE_TICKS) ticks, a shot [`FIRE_TICKS`](cannon::FIRE_TICKS). The reset keeps
/// the direction of fire, so a cannon reset with its barrel up (when Claw
/// comes back after losing a life, or when a late introduction ends)
/// shows its barrel down but keeps firing upwards.
///
/// Not built yet ([`BUILT`](cannon::BUILT) is false).
///
/// [`EnemyState::Rotate`]: crate::logic::enemy::EnemyState::Rotate
pub mod cannon {
    use crate::logic::World;
    use crate::object::ObjectId;

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["GabrielCannon"];

    /// The cannon's picture.
    pub const IMAGE_SET: &str = "LEVEL_GABRIELCANNON";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_GABRIELCANNON_"];

    /// Firing along the dock.
    pub const FIRE_ANIM: &str = "LEVEL_GABRIELCANNON_HORZFIRE";
    /// Firing upwards.
    pub const VERT_FIRE_ANIM: &str = "LEVEL_GABRIELCANNON_VERTIFIRE";
    /// Turning the barrel up.
    pub const RISE_ANIM: &str = "LEVEL_GABRIELCANNON_RISE";
    /// Turning the barrel back down.
    pub const REST_ANIM: &str = "LEVEL_GABRIELCANNON_REST";
    /// The waiting animation: none (see the family's defaults).
    pub const HOME_ANIM: Option<&str> = None;

    /// The sound of each shot.
    pub const FIRE_SOUND: &str = "LEVEL_GABRIELCANNON_CANNON";
    /// The sound of turning up.
    pub const RISE_SOUND: &str = "LEVEL_GABRIELCANNON_CANNONRISE";
    /// The sound of turning down.
    pub const DROP_SOUND: &str = "LEVEL_GABRIELCANNON_CANNONDROP";

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = false;

    /// Speed of its balls, px/s.
    pub const SPEED: i32 = 500;
    /// The muzzle along the dock, unmirrored, from its position.
    pub const MUZZLE: (i32, i32) = (50, 54);
    /// The upward muzzle, unmirrored, from its position.
    pub const VERTICAL_MUZZLE: (i32, i32) = crate::logic::cannon::VERTICAL_MUZZLE;
    /// Ticks of a turn up or down at the 18 ms step: 19 pictures, two
    /// ticks each, and the finishing call.
    pub const ROTATE_TICKS: u32 = 39;
    /// Ticks of a shot at the 18 ms step; the ball leaves on the first.
    pub const FIRE_TICKS: u32 = 11;
    /// The picture with the barrel up.
    pub const RAISED_FRAME: i32 = 19;

    /// Where its balls leave, from its position, along the dock and
    /// upwards, for a mirrored record (`mirrored`, facing left) or not.
    pub fn muzzles(mirrored: bool) -> ((i32, i32), (i32, i32)) {
        let flip = |(x, y): (i32, i32)| if mirrored { (-x, y) } else { (x, y) };
        (flip(MUZZLE), flip(VERTICAL_MUZZLE))
    }

    /// The velocity of its next ball, px/s, with the barrel up or down.
    pub fn ball_velocity(raised: bool) -> (i32, i32) {
        if raised { (0, -SPEED) } else { (-SPEED, 0) }
    }

    /// The cannon's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct CannonData {
        /// It fired since it last turned.
        pub fired: bool,
        /// Its barrel is up.
        pub raised: bool,
        /// Its next shot goes upwards.
        pub vertical: bool,
    }

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let _ = (w, id);
    }

    /// `attacker` hit `id`. Nothing hits the cannon.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. The cannon attacks nothing; its balls do.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::types;

    #[test]
    fn the_cannon_fires_from_under_the_bow_and_up_through_the_deck() {
        // The level's cannon is mirrored, at (43203, 5184): its balls leave
        // from (43153, 5238) along the dock and (43229, 5174) upwards.
        let (along, up) = cannon::muzzles(true);
        assert_eq!((43203 + along.0, 5184 + along.1), (43153, 5238));
        assert_eq!((43203 + up.0, 5184 + up.1), (43229, 5174));
        assert_eq!(cannon::muzzles(false), ((50, 54), (-26, -10)));
        assert_eq!(cannon::ball_velocity(false), (-500, 0));
        assert_eq!(cannon::ball_velocity(true), (0, -500));
        // 9 px a tick, rounded.
        assert_eq!((cannon::SPEED * 18 + 500) / 1000, 9);
        assert_eq!(cannon::HOME_ANIM, None);
    }

    #[test]
    fn the_props_timings_at_eighteen_milliseconds() {
        // Four pictures of 100 ms: 7, 6, 6 and 6 calls.
        assert_eq!(SWITCH_TICKS, 7 + 6 + 6 + 6);
        // 200 ms lamps.
        assert_eq!(button::BLINK_TICKS, 200u32.div_ceil(18));
        // 19 pictures of 25 ms, two calls each, and the finishing call.
        assert_eq!(cannon::ROTATE_TICKS, 19 * 2 + 1);
        assert_eq!(cannon::RAISED_FRAME, 19);
    }

    #[test]
    fn only_claw_presses_the_button() {
        assert_eq!(button::HIT_MASK, types::PLAYER);
        assert_eq!(button::HIT_RECT.right - button::HIT_RECT.left + 1, 24);
        assert_eq!(button::HIT_RECT.bottom - button::HIT_RECT.top + 1, 32);
        assert_ne!(state::IDLE, state::SWITCH);
    }
}
