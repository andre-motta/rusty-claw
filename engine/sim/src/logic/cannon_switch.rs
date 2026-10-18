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
//! # The schedule
//!
//! With C the tick Gabriel gives the order and P the tick of Claw's blow
//! on the button, at the 18 ms step: the switch flips and the cannon fires
//! on C + 1, the ball leaves on C + 2, an upward ball reaches Gabriel on
//! C + 10, the cannon is back home on C + 12 and, raised, turns down on
//! C + 13 (home again on C + 52), and the switch waits again on C + 26.
//! The button shows its pressed lamp and the cannon starts turning up on
//! P + 1, and it is up and home on P + 40.
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

use claw_frame::SoundChannel;

use super::{ObjectData, World, sound};
use crate::level::runtime_image_set;
use crate::object::{ObjectId, Rect, engine_state, flags};

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
pub const BUILT: bool = true;

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

/// The first picture of `set` (1 when the level does not know it).
fn first_frame(w: &World, set: &str) -> i32 {
    w.image_frames(set).map_or(1, |(first, _)| first)
}

/// One logic call: on the first, the set-up (its first picture, no
/// collision, waiting); then, waiting, it flips when Gabriel is there, not
/// defeated, and his fire order is set; flipping, it waits again when the
/// flip ends.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(s) = w.objects.get(id).map(|o| o.state) else {
        return;
    };
    match s {
        engine_state::INIT => {
            let frame = first_frame(w, IMAGE_SET);
            let Some(o) = w.objects.get_mut(id) else {
                return;
            };
            o.image_set = runtime_image_set(IMAGE_SET);
            o.frame = frame;
            o.flags |= flags::NO_COLLIDE;
            o.type_flags = 0;
            o.hit_type_flags = 0;
            o.attack_type_flags = 0;
            o.attack_rect = Rect::UNSET;
            o.state = state::IDLE;
            o.data = ObjectData::CannonSwitch(SwitchData {});
        }
        state::IDLE => {
            w.advance_anim(id);
            if w.gabriel_fights() && w.gabriel_link().is_some_and(|l| l.fire_order) {
                if let Some(o) = w.objects.get_mut(id) {
                    o.state = state::SWITCH;
                }
                w.set_anim(id, SWITCH_ANIM);
                sound::play(w, SWITCH_SOUND, SoundChannel::Effect);
            }
        }
        state::SWITCH => {
            w.advance_anim(id);
            if w.objects.get(id).is_none_or(|o| o.anim.is_done())
                && let Some(o) = w.objects.get_mut(id)
            {
                o.state = state::IDLE;
            }
        }
        _ => {}
    }
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
/// turned back down the flag is clear and it blinks again, from where its
/// blink stopped (the pressed lamp stays until the blink's next lamp).
/// With Gabriel defeated it blinks, whatever the flag.
pub mod button {
    use claw_frame::SoundChannel;

    use crate::level::runtime_image_set;
    use crate::logic::{ObjectData, World, sound};
    use crate::object::{ObjectId, Rect, engine_state, types};

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
    pub const BUILT: bool = true;

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

    fn latch_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut bool> {
        match &mut w.objects.get_mut(id)?.data {
            ObjectData::CannonButton(d) => Some(&mut d.latch),
            _ => None,
        }
    }

    /// One logic call: on the first, the set-up (an enemy only Claw hits,
    /// with [`HIT_RECT`] and no attack box, its blink set and waiting);
    /// then, with Gabriel there, not defeated and the button flag set, the
    /// pressed lamp and no blink; otherwise the press sound may play again
    /// and the blink goes on.
    pub fn tick(w: &mut World, id: ObjectId) {
        let Some(s) = w.objects.get(id).map(|o| o.state) else {
            return;
        };
        match s {
            engine_state::INIT => {
                let first = w.image_frames(IMAGE_SET).map_or(1, |(f, _)| f);
                let Some(o) = w.objects.get_mut(id) else {
                    return;
                };
                o.image_set = runtime_image_set(IMAGE_SET);
                o.frame = first;
                o.type_flags = TYPE;
                o.hit_type_flags = HIT_MASK;
                o.attack_type_flags = 0;
                o.attack_rect = Rect::UNSET;
                o.hit_rect = HIT_RECT;
                o.hit_handler = true;
                o.state = state::IDLE;
                o.data = ObjectData::CannonButton(ButtonData::default());
                w.set_anim(id, IDLE_ANIM);
            }
            state::IDLE => {
                let pressed = w.gabriel_fights() && w.gabriel_link().is_some_and(|l| l.button);
                if pressed {
                    let has = w
                        .image_frames(IMAGE_SET)
                        .is_some_and(|(f, l)| (f..=l).contains(&PRESSED_FRAME));
                    if has && let Some(o) = w.objects.get_mut(id) {
                        o.frame = PRESSED_FRAME;
                    }
                } else {
                    if let Some(l) = latch_mut(w, id) {
                        *l = false;
                    }
                    w.advance_anim(id);
                }
            }
            _ => {}
        }
    }

    /// `attacker` hit `id`: a blow of Claw's (see the module
    /// documentation).
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = attacker;
        if w.objects.get(id).is_none_or(|o| o.frame == PRESSED_FRAME) || !w.gabriel_fights() {
            return;
        }
        if let Some(l) = latch_mut(w, id)
            && !*l
        {
            *l = true;
            sound::play(w, SOUND, SoundChannel::Effect);
        }
        if let Some(l) = w.gabriel_link_mut() {
            l.button = true;
        }
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
/// shows its barrel down but keeps firing upwards, and, never raised,
/// never turns down again until the button raises it once more.
///
/// A turn or a shot under way ends even when Gabriel is defeated in the
/// meantime; the cannon then waits on its last picture.
///
/// [`EnemyState::Rotate`]: crate::logic::enemy::EnemyState::Rotate
pub mod cannon {
    use claw_frame::SoundChannel;

    use crate::level::runtime_image_set;
    use crate::logic::enemy::EnemyState;
    use crate::logic::projectile::{self, CANNON_BALL_HIT_BY, ProjectileSpec};
    use crate::logic::{ObjectData, World, sound};
    use crate::object::{ObjectId, draw_flags, engine_state, flags, types};

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
    pub const BUILT: bool = true;

    /// Speed of its balls, px/s.
    pub const SPEED: i32 = 500;
    /// Damage of its balls, and its own when its record gives none.
    pub const DAMAGE: i32 = 10;
    /// Depth of a cannon placed at depth 0.
    pub const DEFAULT_Z: i32 = crate::logic::cannon::DEFAULT_Z;
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
    /// The picture its reset shows: the barrel down.
    pub const REST_FRAME: i32 = 1;

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
        /// The shot under way has released its ball.
        pub shot: bool,
    }

    fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a CannonData> {
        match &w.objects.get(id)?.data {
            ObjectData::GabrielCannon(d) => Some(d),
            _ => None,
        }
    }

    fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut CannonData> {
        match &mut w.objects.get_mut(id)?.data {
            ObjectData::GabrielCannon(d) => Some(d),
            _ => None,
        }
    }

    fn set_state(w: &mut World, id: ObjectId, s: EnemyState) {
        if let Some(o) = w.objects.get_mut(id) {
            o.state = s as i32;
        }
    }

    /// Starts `key` (not advanced), or no animation without one.
    fn set_anim(w: &mut World, id: ObjectId, key: Option<&str>) {
        crate::logic::enemy::set_anim(w, id, key.unwrap_or(""));
    }

    /// One logic call: the set-up on the first, then its state's step.
    pub fn tick(w: &mut World, id: ObjectId) {
        let Some(s) = w.objects.get(id).map(|o| o.state) else {
            return;
        };
        if s == engine_state::INIT {
            construct(w, id);
            return;
        }
        match EnemyState::from_id(s) {
            Some(EnemyState::Home) => home_tick(w, id),
            Some(EnemyState::Rotate) => rotate_tick(w, id),
            Some(EnemyState::Fire | EnemyState::VertFiring) => firing_tick(w, id),
            _ => {}
        }
    }

    /// Builds the cannon: its picture, no collision, removed far outside
    /// the level; its speed 500 px/s and damage 10 when its record gives
    /// none, depth [`DEFAULT_Z`] when it gives 0, the speed turned round
    /// for a mirrored record; then its reset.
    pub fn construct(w: &mut World, id: ObjectId) {
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        o.image_set = runtime_image_set(IMAGE_SET);
        o.flags |= flags::NO_COLLIDE | flags::DESTROY_FAR;
        if o.speed_x == 0 {
            o.speed_x = SPEED;
        }
        if o.damage == 0 {
            o.damage = DAMAGE;
        }
        if o.z == 0 {
            o.z = DEFAULT_Z;
            o.flags |= flags::RESORT;
        }
        if o.draw_flags & draw_flags::MIRROR != 0 {
            o.speed_x = -o.speed_x;
        }
        o.data = ObjectData::GabrielCannon(CannonData::default());
        reset(w, id);
    }

    /// The reset: fired and raised cleared; with Gabriel there his fire
    /// order and button flag cleared and his wait over; the first picture;
    /// home. The direction of fire and the speeds stay.
    pub fn reset(w: &mut World, id: ObjectId) {
        if let Some(d) = data_mut(w, id) {
            d.fired = false;
            d.raised = false;
        }
        if let Some(l) = w.gabriel_link_mut() {
            l.fire_order = false;
            l.button = false;
            l.wait_for_cannon = false;
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.frame = REST_FRAME;
        }
        go_home(w, id);
    }

    fn go_home(w: &mut World, id: ObjectId) {
        set_state(w, id, EnemyState::Home);
        set_anim(w, id, HOME_ANIM);
    }

    /// Home (see the family's documentation).
    pub fn home_tick(w: &mut World, id: ObjectId) {
        let Some(link) = w.gabriel_link().cloned() else {
            return;
        };
        if link.defeated {
            return;
        }
        if link.wait_for_cannon {
            reset(w, id);
            return;
        }
        w.advance_anim(id);
        let Some((fired, raised)) = data(w, id).map(|d| (d.fired, d.raised)) else {
            return;
        };
        if fired && raised {
            if let Some(d) = data_mut(w, id) {
                d.fired = false;
            }
            lower(w, id);
        } else if link.button && !raised {
            if let Some(d) = data_mut(w, id) {
                d.fired = false;
            }
            raise(w, id);
        } else if link.fire_order {
            fire(w, id);
            if let Some(l) = w.gabriel_link_mut() {
                l.fire_order = false;
            }
            if let Some(d) = data_mut(w, id) {
                d.fired = true;
            }
        }
    }

    /// Turns the barrel up: vertical fire from now on.
    fn raise(w: &mut World, id: ObjectId) {
        set_state(w, id, EnemyState::Rotate);
        set_anim(w, id, Some(RISE_ANIM));
        if let Some(o) = w.objects.get_mut(id) {
            (o.speed_x, o.speed_y) = ball_velocity(true);
        }
        if let Some(d) = data_mut(w, id) {
            d.raised = true;
            d.vertical = true;
        }
        sound::play(w, RISE_SOUND, SoundChannel::Effect);
    }

    /// Turns the barrel down: horizontal fire from now on, and the button
    /// pops back.
    fn lower(w: &mut World, id: ObjectId) {
        set_state(w, id, EnemyState::Rotate);
        set_anim(w, id, Some(REST_ANIM));
        if let Some(o) = w.objects.get_mut(id) {
            (o.speed_x, o.speed_y) = ball_velocity(false);
        }
        if let Some(l) = w.gabriel_link_mut() {
            l.button = false;
        }
        if let Some(d) = data_mut(w, id) {
            d.raised = false;
            d.vertical = false;
        }
        sound::play(w, DROP_SOUND, SoundChannel::Effect);
    }

    /// Fires: `VertFiring` with vertical fire, else `Fire`; the firing
    /// animation starts (not advanced in this call).
    fn fire(w: &mut World, id: ObjectId) {
        let vertical = data(w, id).is_some_and(|d| d.vertical);
        if let Some(d) = data_mut(w, id) {
            d.shot = false;
        }
        if vertical {
            set_state(w, id, EnemyState::VertFiring);
            set_anim(w, id, Some(VERT_FIRE_ANIM));
        } else {
            set_state(w, id, EnemyState::Fire);
            set_anim(w, id, Some(FIRE_ANIM));
        }
    }

    /// Rotate: the turn plays; when it ends the cannon is home.
    fn rotate_tick(w: &mut World, id: ObjectId) {
        w.advance_anim(id);
        if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
            go_home(w, id);
        }
    }

    /// Fire and VertFiring: the first call whose animation event is 1
    /// releases the ball; when the animation ends the cannon is home.
    fn firing_tick(w: &mut World, id: ObjectId) {
        let event = w.advance_anim(id);
        if event == 1 && data(w, id).is_some_and(|d| !d.shot) {
            if let Some(d) = data_mut(w, id) {
                d.shot = true;
            }
            release(w, id);
        }
        if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
            go_home(w, id);
        }
    }

    /// One cannonball from the muzzle of the fire's direction, one depth
    /// step behind the cannon, at the cannon's speeds, and the shot's
    /// sound.
    fn release(w: &mut World, id: ObjectId) {
        let vertical = data(w, id).is_some_and(|d| d.vertical);
        let Some((x, y, z, vx, vy, mirrored)) = w.objects.get(id).map(|o| {
            (
                o.x,
                o.y,
                o.z,
                o.speed_x,
                o.speed_y,
                o.draw_flags & draw_flags::MIRROR != 0,
            )
        }) else {
            return;
        };
        let (along, up) = muzzles(mirrored);
        let (mx, my) = if vertical { up } else { along };
        projectile::spawn(
            w,
            ProjectileSpec {
                logic: crate::logic::cannon::BALL_LOGIC,
                x: x + mx,
                y: y + my,
                z: z - 1,
                vx,
                vy,
                damage: 0,
                type_flags: types::ENEMY_SHOT,
                hit_type_flags: CANNON_BALL_HIT_BY,
                special_flags: 0,
                image_set: "",
                owner: Some(id),
                fuse_ms: None,
                power: 0,
            },
        );
        sound::play(w, FIRE_SOUND, SoundChannel::Effect);
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
