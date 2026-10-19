//! The crabs of the seventh level: the hermit crab, the sand nests that
//! hatch them ([`nest`]) and the bomb a bomber crab throws ([`bomb`]).
//!
//! The hermit crab is a plain enemy (not a human one) that guards a short
//! stretch of beach. Far from Claw it mostly hides in its shell
//! ([`EnemyState::Hide`]) or crouches in it ([`EnemyState::Defense`]);
//! closer in it charges at him ([`EnemyState::MovingAttack`], 300 to 500
//! px/s), snips at him when it is within 50 px ([`EnemyState::Snip`],
//! live on its 7th to 14th ticks, [`DAMAGE`] each), or backs off. A
//! bomber crab (the [`BOMBER_IMAGE_SET`] picture) throws a bomb instead
//! ([`EnemyState::Launch`]) at most every [`LAUNCH_EVERY_MS`].
//!
//! Its shell protects it: a hit while it shows a shell picture
//! ([`SHELL_FRAMES`]) does no harm but knocks it aside
//! ([`EnemyState::Nudge`], 5.4 px a tick for as long as the blow keeps
//! touching it, about 37 px under a standing sword swing), with a boink
//! at most every 200 ms. A hit on any other picture kills it at once,
//! whatever its health, and it drops the one item its record lists. The
//! magic claw gets through the withdrawn shell, not the crouched one.
//!
//! # A crab's tick
//!
//! The shared enemy tick (see [`super`]): the per-tick roll, the side
//! Claw is on and the range mode, then the state's rule, then the
//! movement step (gravity 2000 px/s², the ledge guard, drawn in front of
//! or behind Claw with depth base 15). Hits come later in the tick from
//! the collision pass ([`on_hit`]).
//!
//! The range boxes (half width, half height) are [`RANGES`]: 400, 300,
//! 100 and 50 px across, 50 px up and down, so Claw standing on the
//! crab's floor (his origin 37 px above the crab's) is in them and a jump
//! of more than 13 px takes him out. The second box is not set by the
//! game: each crab keeps whatever it starts with ([`Leftovers`]); a large
//! one keeps the crab in range mode 1 out to its size.
//!
//! The states, with their ticks at the 18 ms step:
//!
//! - [`EnemyState::Regroup`]: the crab faces Claw on entering it (and can
//!   no longer be lifted, see [Lifting](#lifting)), and on its one tick
//!   decides: far from Claw (modes 0, 1, 5 and 6) it hides when a draw of
//!   0..100 is below 40, else it crouches; close (modes 2 to 4) it
//!   crouches.
//! - [`EnemyState::Hide`]: withdrawn ([`HIDE_ANIM`]) for 1000 to 3000 ms
//!   (drawn when the last hide time has run out, else the old one goes
//!   on), then each tick a draw of 0..100 below 90 sends it back to
//!   regroup. Going into hiding makes it liftable.
//! - [`EnemyState::Defense`]: crouched ([`DEFENSE_ANIM`]); on its 16th
//!   tick, by the mode, it hides, backs off 75 to 125 px at 150 px/s
//!   ([`EnemyState::MoveBack`], with its scuttle sound), or attacks: a
//!   hermit chooses again from [`EnemyState::Attack`], a bomber throws
//!   when its last throw is [`LAUNCH_EVERY_MS`] old and regroups
//!   otherwise. Out of range (modes 5 and 6) it regroups. The chances
//!   are [`DEFENSE_CHANCES`].
//! - [`EnemyState::Attack`]: out of range it regroups; within 300 px it
//!   charges ([`EnemyState::MovingAttack`]) at 300 to 500 px/s to a spot
//!   40 to 80 px short of Claw on its own side, scuttling every 200 ms,
//!   and chooses again when it gets there, hits a wall or stops; within
//!   50 px it snips.
//! - [`EnemyState::Snip`]: [`SNIP_ANIM`]; its snip box ([`SNIP_BOX`]) is
//!   live on the snip pictures (ticks 7 to 14), with the snip sound once;
//!   on tick 15 it regroups.
//! - [`EnemyState::Launch`]: [`SNIP_ANIM`]; on its third tick (the throw
//!   picture) the bomb leaves, aimed [`THROW_DISTANCE`] px towards Claw;
//!   on tick 15 it regroups.
//! - [`EnemyState::Tumble`]: thrown out of a nest under 800 px/s² until
//!   it lands; then it stands and regroups.
//! - [`EnemyState::Nudge`]: knocked aside in its shell at 300 px/s
//!   towards a spot 20 to 40 px from Claw on its side; it regroups when
//!   it gets there or stops. Claw on the side of the blow (his own sword)
//!   puts that spot behind it, so each blow moves it one 5.4 px step.
//!
//! The crab's snip, scuttle and shell sounds share one 200 ms timer.
//!
//! # Lifting
//!
//! A crab can be lifted only from the moment it goes into hiding
//! ([`enter_hide`]) until its next regroup ([`go_home`]); it is not
//! liftable when built. A blow on the crouched shell makes it hide and
//! knocks it aside at once, and the knock's regroup ends the window a
//! tick or two later. Claw lifts it as he lifts a guard (his own lift,
//! [`super::lift`]), with these differences:
//!
//! - Lifted ([`lifted`]): it says nothing and draws nothing; its movement
//!   box while carried is a single point ([`CARRY`]). On Claw's lift frame
//!   it shows [`LIFT_ANIM`] 0 px (facing left) or -10 px (facing right)
//!   across and 15 px up from him; then, flailing, the withdrawn shell
//!   ([`HIDE_ANIM`]) over his head, 10 or -10 px across and 69 px up.
//! - Thrown ([`thrown`]): it falls ([`FALL_ANIM`]) as a thrown object,
//!   hurting what a thrown object hurts (the human enemies and the crates
//!   it flies into; other crabs let it pass), with no time limit. On
//!   landing ([`thrown_tick`]) it stops, takes its own movement box back
//!   with its feet where they are, is an enemy again and regroups: no
//!   bounce, no rest and no damage. It is free at once (it crouches on
//!   its next tick) and may leave the active area again, as in the
//!   original.
//! - Dropped: as a guard is, then it regroups.
//!
//! A dead crab is never liftable: a crab killed in its shell (by the
//! magic claw, or on a death tile while knocked aside) loses the flag at
//! once, and [`lifted`] refuses a dead crab, so a body can never be
//! thrown and get up again.
//!
//! # State numbers
//!
//! The crab's nine states are new names and take the upper half of the
//! seventh level's block ([`STATE_BLOCK`], from
//! [`super::bear_sailor::CRAB_STATES_START`]), in the order the game
//! registers them: [`EnemyState::Regroup`] (facing Claw and choosing
//! between hiding and crouching), [`EnemyState::Hide`],
//! [`EnemyState::Defense`], [`EnemyState::Attack`] (choosing between
//! charging, snipping and going back), [`EnemyState::MovingAttack`],
//! [`EnemyState::Snip`], [`EnemyState::Tumble`] (falling out of a nest),
//! [`EnemyState::Nudge`] and [`EnemyState::Launch`]. Its own states that
//! share a name with the human enemies' (deciding, striking, falling) are
//! distinct states of the original game, so they take new numbers too;
//! later families that run them reuse these numbers. It also runs the
//! shared `MoveBack` and `Dead` under their numbers. The nest and the
//! bomb are no enemies: their states live in their own data.
//!
//! # Defaults taken
//!
//! - The block of state numbers: the crabs take 2190 to 2198 of the
//!   seventh level's 2180 to 2199, in registration order.
//! - A crab's second range box and the side its first regroup reads are
//!   left over from memory in the original game; each crab keeps the
//!   value it starts with ([`Leftovers`], set with [`set_leftovers`] as
//!   the reference runs show it), with an empty box and an unmirrored
//!   start as the plain default.
//! - A nest hands its record's loot to its crabs in the original order,
//!   with its skipping of empty entries kept (no retail nest carries
//!   loot).
//! - Under a widened view a dead crab leaves the screen, and a nest wakes
//!   with its window, by the rules the built enemies and the triggers that
//!   wake around Claw already use.
//! - A crab's movement box while carried is a single point at its
//!   position (the game sets none for the crab), so a thrown crab lands
//!   when its position reaches the floor and its feet are put back there.
//! - A nest crab falls onto platforms as the other enemies do.
//! - A dead crab cannot be lifted (see [Lifting](#lifting)).
//!
//! Its data ([`HermitCrabData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`], and the crab's own ([`Fields`]).

use std::ops::RangeInclusive;

use super::lift::{self, Carry};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, Handler,
    Timer, UNBOUNDED, build, data, data_mut, die, face_claw, facing_rect, field, human, limit,
    nothing, obj, obj_mut, play, rand, record, record_bounds, set_anim, set_anim_now, set_facing,
    set_state,
};
use crate::logic::{ObjectData, World, pickup};
use crate::object::{Object, ObjectId, Rect, flags, special, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["HermitCrab"];

/// The plain crab's picture.
pub const IMAGE_SET: &str = "LEVEL_HERMITCRAB";
/// The bomber crab's picture (same frame numbers as the plain one's).
pub const BOMBER_IMAGE_SET: &str = "LEVEL_BOMBERCRAB";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, BOMBER_IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_HERMITCRAB_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers of the seventh level's families (see
/// [`super::bear_sailor::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::bear_sailor::STATE_BLOCK;

/// The crab's nine states, in the order they take their numbers.
pub const STATES: [EnemyState; 9] = [
    EnemyState::Regroup,
    EnemyState::Hide,
    EnemyState::Defense,
    EnemyState::Attack,
    EnemyState::MovingAttack,
    EnemyState::Snip,
    EnemyState::Tumble,
    EnemyState::Nudge,
    EnemyState::Launch,
];

/// Damage of a snip and of touching a crab, when its record sets none (no
/// difficulty factor).
pub const DAMAGE: i32 = 5;
/// The pictures of the shell: crouched, then withdrawn.
pub const SHELL_FRAMES: [i32; 3] = [800, 801, 802];
/// The crouched shell picture: even the magic claw only knocks it aside.
pub const CROUCHED_FRAME: i32 = 800;
/// The picture of the throw.
pub const THROW_FRAME: i32 = 201;
/// The pictures on which a snip is live.
pub const SNIP_FRAMES: [i32; 2] = [203, 204];
/// Hit box: the box of the crab's first picture.
pub const HIT_RECT: Rect = Rect::new(-25, -19, 30, 18);
/// Movement box: the hit box lowered 10 px at the top and 5 at the bottom.
pub const MOVE_RECT: Rect = Rect::new(-25, -9, 30, 23);
/// The snip's box: the hit box widened by 15 px and heightened by 10 px
/// on each side.
pub const SNIP_BOX: Rect = Rect::new(-40, -29, 45, 28);
/// The touch box when it is not snipping.
pub const CONTACT_RECT: Rect = Rect::new(-1, -1, 1, 1);
/// The range boxes (half width, half height), outermost first; the
/// second is left as each crab starts with it (empty by default, see
/// [`Leftovers`]).
pub const RANGES: [(i32, i32); 5] = [(400, 50), (0, 0), (300, 50), (100, 50), (50, 50)];
/// Kinds of object that can hit a crab: Claw, his pistol shots and
/// dynamite, the level's shots and explosions, and his magic.
pub const HIT_BY: u32 = 0x300_0022;
/// Depth base of the depth against Claw.
pub const Z_BASE: i32 = 15;
/// Time between two throws of a bomber crab, ms.
pub const LAUNCH_EVERY_MS: u32 = 4000;
/// The distance a bomb is aimed at, px, towards Claw.
pub const THROW_DISTANCE: i32 = 150;
/// Speed of a knock aside in the shell, px/s.
pub const NUDGE_SPEED: i32 = 300;
/// Speed of a back-off, px/s.
pub const MOVE_BACK_SPEED: i32 = 150;
/// Slowest charge, px/s; a charge from the middle range adds a draw of
/// 0 to [`CHARGE_SPREAD`] - 1.
pub const CHARGE_SPEED: i32 = 300;
/// See [`CHARGE_SPEED`].
pub const CHARGE_SPREAD: u32 = 201;
/// A charge stops 40 px short of Claw plus a draw of 0 to
/// [`CHARGE_STOP_SPREAD`] - 1.
pub const CHARGE_STOP: i32 = 40;
/// See [`CHARGE_STOP`].
pub const CHARGE_STOP_SPREAD: u32 = 41;
/// A knock aside aims 20 px from Claw plus a draw of 0 to
/// [`NUDGE_STOP_SPREAD`] - 1.
pub const NUDGE_STOP: i32 = 20;
/// See [`NUDGE_STOP`].
pub const NUDGE_STOP_SPREAD: u32 = 21;
/// Shortest hide, ms; the length is drawn up to [`HIDE_MAX_MS`].
pub const HIDE_MIN_MS: u64 = 1000;
/// Longest hide, ms.
pub const HIDE_MAX_MS: u64 = 3000;
/// Chance (of 100) of hiding when it regroups far from Claw.
pub const REGROUP_HIDE_CHANCE: i32 = 40;
/// Chance (of 100) per tick of coming out once the hide time is over.
pub const UNHIDE_CHANCE: i32 = 90;
/// The shared sound timer, ms.
pub const SOUND_GAP_MS: u64 = 200;
/// Gravity of a crab on its feet, px/s².
pub const GRAVITY: f64 = 2000.0;
/// Gravity of a crab thrown out of a nest, px/s².
pub const NEST_GRAVITY: f64 = 800.0;

/// The chances (of 100) of hiding and then of backing off at the end of
/// a crouch, by range mode 0 to 4: a hermit's, then a bomber's.
pub const DEFENSE_CHANCES: [[(i32, i32); 5]; 2] = [
    [(50, 20), (50, 20), (30, 40), (20, 30), (10, 20)],
    [(10, 0), (10, 0), (10, 10), (20, 20), (40, 40)],
];

/// The snip.
pub const SNIP_ANIM: &str = "LEVEL_HERMITCRAB_STRIKE1";
/// The crouch in the shell.
pub const DEFENSE_ANIM: &str = "LEVEL_HERMITCRAB_CROUCH4";
/// Withdrawn in the shell.
pub const HIDE_ANIM: &str = "LEVEL_HERMITCRAB_HIDE";
/// Charging and backing off.
pub const RUN_ANIM: &str = "LEVEL_HERMITCRAB_FASTADVANCE";
/// Falling out of a nest, and flying when thrown by Claw.
pub const FALL_ANIM: &str = "LEVEL_HERMITCRAB_FALL";
/// Being lifted by Claw.
pub const LIFT_ANIM: &str = "LEVEL_HERMITCRAB_LIFT";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_HERMITCRAB_KILLFALL";
/// The snip's sound.
pub const SNIP_SOUND: &str = "LEVEL_HERMITCRAB_CRABSNIP";
/// The sound of a throw by a crab built as a bomber (no seventh-level
/// record is); also the snip of a nest's plain crabs.
pub const THROW_SOUND: &str = "GAME_THRWBOMB";
/// The sound of a blow on the shell.
pub const SHELL_SOUND: &str = "GAME_BOINKJR";
/// The scuttle of a charge and of a back-off.
pub const SCUTTLE_SOUND: &str = "LEVEL_HERMITCRAB_CRABLEFT";
/// The death sound.
pub const DEATH_SOUND: &str = "LEVEL_HERMITCRAB_CRABDEATH";

/// The side value with which a regroup faces right.
pub const SIDE_RIGHT: i32 = 2;

/// How Claw carries a crab: beside him on his lift frame, then over his
/// head, and a single point as its movement box meanwhile.
pub const CARRY: Carry = Carry {
    beside: (0, -10, -15),
    overhead: (10, -10, -69),
    move_rect: Rect::new(0, 0, 0, 0),
};

/// Whether picture `frame` is a shell picture, on which a hit only knocks
/// the crab aside.
pub fn is_shell_frame(frame: i32) -> bool {
    SHELL_FRAMES.contains(&frame)
}

/// The hermit crab class.
pub static CLASS: Class = Class {
    name: "HermitCrab",
    human: false,
    construct,
    pre_tick: nothing,
    handler,
    go_home,
    go_idle: nothing,
    carry: Some(CARRY),
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims {
        advance: RUN_ANIM,
        fast_advance: RUN_ANIM,
        pace: "",
        walk: "",
        hit_low: "",
        hit_high: "",
        home: "",
        jump: "",
        fall: FALL_ANIM,
        lifted: LIFT_ANIM,
        lifted2: HIDE_ANIM,
        bounce: "",
    },
    sounds: ClassSounds::NONE,
};

/// What a crab starts with that the game leaves over from memory: the
/// side its first regroup faces by ([`SIDE_RIGHT`] faces right, anything
/// else left) and its second range box (half width, half height). The
/// values differ from crab to crab and from run to run of the original;
/// the plain default is an unmirrored start and an empty box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Leftovers {
    /// The side read by the first regroup.
    pub side: i32,
    /// The second range box.
    pub box1: (i32, i32),
}

/// What a crab keeps beyond the shared enemy data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    /// A bomber: it throws bombs and never snips.
    pub bomber: bool,
    /// A throw is armed for the current launch.
    pub armed: bool,
    /// The hide time.
    pub hide: Timer,
    /// The shared 200 ms sound timer.
    pub sound: Timer,
    /// Time since the last throw.
    pub launch: Timer,
    /// The last object that hit it.
    pub last_attacker: Option<ObjectId>,
    /// The sound of its snip (and of its throw).
    pub strike_sound: &'static str,
    /// What it started with.
    pub leftovers: Leftovers,
}

/// A hermit crab's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HermitCrabData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The crab's own.
    pub crab: Fields,
}

/// The crab's own data of `o`, if it is a built crab.
pub fn fields_of(o: &Object) -> Option<&Fields> {
    match &o.data {
        ObjectData::HermitCrab(d) => Some(&d.crab),
        _ => None,
    }
}

fn fields<'a>(w: &'a World, id: ObjectId) -> Option<&'a Fields> {
    fields_of(w.objects.get(id)?)
}

fn fields_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut Fields> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::HermitCrab(d) => Some(&mut d.crab),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Regroup => Some(regroup_tick),
        EnemyState::Hide => Some(hide_tick),
        EnemyState::Defense => Some(defense_tick),
        EnemyState::Attack => Some(attack_tick),
        EnemyState::MovingAttack => Some(moving_attack_tick),
        EnemyState::Snip => Some(snip_tick),
        EnemyState::Tumble => Some(tumble_tick),
        EnemyState::Nudge => Some(nudge_tick),
        EnemyState::Launch => Some(launch_tick),
        EnemyState::MoveBack => Some(human::move_tick),
        EnemyState::Thrown => Some(thrown_tick),
        _ => None,
    }
}

/// One logic call: the shared enemy tick, keeping where the movement step
/// started (the charges, the knocks and the back-off stop when it did not
/// move them).
pub fn tick(w: &mut World, id: ObjectId) {
    human::run(w, id, &CLASS);
    // Dead by its own step (a death tile) while it could be lifted: no
    // longer.
    if data(w, id).is_some_and(|d| d.dead) {
        obj_mut(w, id).special_flags &= !special::LIFTABLE;
    }
}

/// `id` attacked `victim`: a crab leaves its touches to Claw's own rule.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds the crab: its patrol limits from its record (a crab placed
/// outside them starts on the nearer one), dropped onto the floor below;
/// a bomber when its record's first user value is above 0 (and then it
/// never walks off its spot); its boxes, 5 damage unless its record says
/// otherwise, its health as its record sets it; then it regroups by its
/// leftover side ([`Leftovers`]). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let bounds = record_bounds(w, id);
    let user_0 = record(w, id, field::USER_0);
    let enemy = EnemyFields {
        mode: 6,
        ranges: RANGES,
        bounds,
        bounds_y: (
            limit(record(w, id, field::Y_MIN), UNBOUNDED.0),
            limit(record(w, id, field::Y_MAX), UNBOUNDED.1),
        ),
        stationary: user_0 > 0,
        ledge_guard: true,
        z_sort: true,
        z_base: Z_BASE,
        smarts: record(w, id, 17),
        contact_rect: CONTACT_RECT,
        strike_rect: SNIP_BOX,
        strike_anim: SNIP_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        ..EnemyFields::default()
    };
    let bomber = user_0 != 0;
    let image = if bomber { BOMBER_IMAGE_SET } else { IMAGE_SET };
    let frames = w.image_frames(image);
    let o = obj_mut(w, id);
    if let Some((lo, hi)) = frames
        && !(lo..=hi).contains(&o.frame)
    {
        o.frame = lo;
    }
    let body = Body {
        image_set: image,
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT_RECT,
        health: o.health,
        damage: if o.damage == 0 { DAMAGE } else { o.damage },
        hit_by: HIT_BY,
    };
    build(w, id, body, enemy, false);
    let o = obj_mut(w, id);
    let enemy = match std::mem::take(&mut o.data) {
        ObjectData::Enemy(d) => d,
        _ => EnemyData::default(),
    };
    o.data = ObjectData::HermitCrab(HermitCrabData {
        enemy,
        crab: Fields {
            bomber,
            strike_sound: if bomber { THROW_SOUND } else { SNIP_SOUND },
            ..Fields::default()
        },
    });
    regroup_by_side(w, id);
}

/// Sets what crab `id` starts with (see [`Leftovers`]): its second range
/// box, and its facing as its first regroup leaves it. Meant for the
/// moment right after the crab is built, before its first tick.
pub fn set_leftovers(w: &mut World, id: ObjectId, l: Leftovers) {
    let Some(f) = fields_mut(w, id) else { return };
    f.leftovers = l;
    if let Some(d) = data_mut(w, id) {
        d.ranges[1] = l.box1;
    }
    set_facing(w, id, l.side == SIDE_RIGHT);
}

/// Makes the crab a bomber or a plain crab, as a nest does with the
/// crabs it hatches: its picture follows, its throw is disarmed, and its
/// snip sounds as a nest crab's does (the bomb's throw for a plain crab,
/// the snip for a bomber's throw).
pub fn set_bomber(w: &mut World, id: ObjectId, bomber: bool) {
    if let Some(f) = fields_mut(w, id) {
        f.bomber = bomber;
        f.armed = false;
        f.strike_sound = if bomber { SNIP_SOUND } else { THROW_SOUND };
    }
    let image = if bomber { BOMBER_IMAGE_SET } else { IMAGE_SET };
    obj_mut(w, id).image_set = crate::level::runtime_image_set(image);
}

/// True when crab `id` is a bomber.
pub fn is_bomber(w: &World, id: ObjectId) -> bool {
    fields(w, id).is_some_and(|f| f.bomber)
}

// ----------------------------------------------------------------------
// Regroup, hide and defense

/// Regroups: faces Claw and can no longer be lifted; the velocity is
/// left as it is (a crab knocked aside or backing off moves one more
/// tick).
pub fn go_home(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    enter_regroup(w, id);
}

/// The first regroup, which faces by the leftover side.
fn regroup_by_side(w: &mut World, id: ObjectId) {
    let side = fields(w, id).map_or(0, |f| f.leftovers.side);
    set_facing(w, id, side == SIDE_RIGHT);
    enter_regroup(w, id);
}

fn enter_regroup(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Regroup);
    obj_mut(w, id).special_flags &= !special::LIFTABLE;
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

/// Regroup: far from Claw it hides 40% of the time, else it crouches;
/// close it crouches. Draws: one of 0..100 when far, then the hide's.
pub fn regroup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    match mode(w, id) {
        2..=4 => enter_defense(w, id),
        _ => {
            if rand(w, 100) < REGROUP_HIDE_CHANCE {
                enter_hide(w, id);
            } else {
                enter_defense(w, id);
            }
        }
    }
}

/// Hides: a new hide time of 1000 to 3000 ms when the last one has run
/// out (else the old one goes on), stopped, withdrawn, and liftable until
/// its next regroup. Draws: one of 0..2001 for a new hide time.
pub fn enter_hide(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if fields(w, id).is_some_and(|f| f.hide.done(now)) {
        let spread = (HIDE_MAX_MS - HIDE_MIN_MS + 1) as u32;
        let len = HIDE_MIN_MS + rand(w, spread) as u64;
        if let Some(f) = fields_mut(w, id) {
            f.hide = Timer::new(now, len);
        }
    }
    set_vx(w, id, 0.0);
    set_state(w, id, EnemyState::Hide);
    set_anim(w, id, HIDE_ANIM);
    obj_mut(w, id).special_flags |= special::LIFTABLE;
}

/// Hide: once the hide time is over, a draw of 0..100 below 90 each tick
/// sends it to regroup.
pub fn hide_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    if fields(w, id).is_some_and(|f| f.hide.done(now)) && rand(w, 100) < UNHIDE_CHANCE {
        go_home(w, id);
    }
}

/// Crouches: stopped, crouched.
pub fn enter_defense(w: &mut World, id: ObjectId) {
    set_vx(w, id, 0.0);
    set_state(w, id, EnemyState::Defense);
    set_anim(w, id, DEFENSE_ANIM);
}

/// Defense: once the crouch is over, out of range it regroups; else, with
/// the chances of its mode ([`DEFENSE_CHANCES`]), it hides, backs off
/// (with its scuttle) or attacks. A back-off that cannot start leaves it
/// crouched, to decide again on the next tick. Draws: one of 0..100, then
/// the hide's or one of 0..100, then the back-off's.
pub fn defense_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let m = mode(w, id);
    if m >= 5 {
        go_home(w, id);
        return;
    }
    let bomber = is_bomber(w, id);
    let (hide, back) = DEFENSE_CHANCES[usize::from(bomber)][usize::from(m)];
    if rand(w, 100) < hide {
        enter_hide(w, id);
    } else if rand(w, 100) < back {
        play(w, SCUTTLE_SOUND);
        human::enter_move_back(w, id);
    } else {
        enter_attack(w, id);
    }
}

// ----------------------------------------------------------------------
// Attacks

/// Attacks: stopped; a hermit chooses from [`EnemyState::Attack`]; a
/// bomber starts a throw when its last one is [`LAUNCH_EVERY_MS`] old,
/// else it regroups.
pub fn enter_attack(w: &mut World, id: ObjectId) {
    set_vx(w, id, 0.0);
    if !is_bomber(w, id) {
        set_state(w, id, EnemyState::Attack);
        return;
    }
    let now = w.now_ms;
    if fields(w, id).is_some_and(|f| f.launch.done(now)) {
        if let Some(f) = fields_mut(w, id) {
            f.launch = Timer::new(now, u64::from(LAUNCH_EVERY_MS));
            f.armed = true;
        }
        set_state(w, id, EnemyState::Launch);
        set_anim(w, id, SNIP_ANIM);
    } else {
        go_home(w, id);
    }
}

/// Attack: out of range it regroups; within 300 px it charges; within
/// 50 px it snips. A charge that cannot start leaves it here to try again
/// on the next tick. Draws: one of 0..201 for a charge, then the
/// charge's.
pub fn attack_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    match mode(w, id) {
        2 | 3 => {
            let speed = CHARGE_SPEED + rand(w, CHARGE_SPREAD);
            enter_moving_attack(w, id, speed);
        }
        4 => enter_snip(w, id),
        _ => go_home(w, id),
    }
}

/// A charge at `speed` px/s to a spot 40 to 80 px short of Claw on the
/// crab's side, when it is inside the crab's limit on that side and has
/// floor; otherwise nothing changes. Draws: one of 0..41.
pub fn enter_moving_attack(w: &mut World, id: ObjectId, speed: i32) {
    let r = rand(w, CHARGE_STOP_SPREAD);
    let Some(cx) = w.claw_object().map(|c| c.x) else {
        return;
    };
    let Some((right, (min, max))) = data(w, id).map(|d| (d.claw_right, d.bounds)) else {
        return;
    };
    let (tx, inside) = if right {
        let tx = cx.saturating_sub(CHARGE_STOP + r);
        (tx, tx <= max)
    } else {
        let tx = cx.saturating_add(CHARGE_STOP + r);
        (tx, min <= tx)
    };
    if let Some(d) = data_mut(w, id) {
        d.human.target_x = tx;
    }
    if !inside || !human::floor_ok(w, id, tx) {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = if right {
            f64::from(speed)
        } else {
            -f64::from(speed)
        };
        d.vy = 0.0;
    }
    face_claw(w, id);
    set_state(w, id, EnemyState::MovingAttack);
    set_anim(w, id, RUN_ANIM);
}

/// Plays `key` unless the shared sound timer runs, and starts it again.
fn timed_sound(w: &mut World, id: ObjectId, key: &str) {
    let now = w.now_ms;
    let Some(f) = fields_mut(w, id) else { return };
    if f.sound.done(now) {
        f.sound = Timer::new(now, SOUND_GAP_MS);
        play(w, key);
    }
}

/// True once a mover is at or past its spot, ran into a wall, or did not
/// move in its last step.
fn stopped(w: &World, id: ObjectId) -> bool {
    let exact = super::exact_x(w, id);
    let Some((vx, h)) = data(w, id).map(|d| (d.vx, &d.human)) else {
        return false;
    };
    let target = f64::from(h.target_x);
    let arrived = if vx < 0.0 {
        exact <= target
    } else {
        exact >= target
    };
    arrived || h.tile_hit || exact == f64::from(h.pre_move_x)
}

/// MovingAttack: the scuttle every 200 ms; when it is at its spot, hits
/// a wall or stops, it attacks again.
pub fn moving_attack_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    timed_sound(w, id, SCUTTLE_SOUND);
    if stopped(w, id) {
        enter_attack(w, id);
    }
}

/// Snips.
pub fn enter_snip(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Snip);
    set_anim(w, id, SNIP_ANIM);
}

/// Snip: on the snip pictures the snip box is live (with the snip sound
/// when the shared timer allows); when the snip ends the touch box is
/// back and it regroups.
pub fn snip_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let done = obj(w, id).anim.is_done();
    let frame = obj(w, id).frame;
    if SNIP_FRAMES.contains(&frame) && !done {
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, SNIP_BOX);
        let key = fields(w, id).map_or(SNIP_SOUND, |f| f.strike_sound);
        timed_sound(w, id, key);
    } else if done {
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, CONTACT_RECT);
        go_home(w, id);
    }
}

/// Launch: on the throw picture an armed throw plays its sound and the
/// bomb leaves, aimed [`THROW_DISTANCE`] px towards Claw; when the throw
/// ends it regroups.
pub fn launch_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let done = obj(w, id).anim.is_done();
    let armed = fields(w, id).is_some_and(|f| f.armed);
    if obj(w, id).frame == THROW_FRAME && !done && armed {
        let key = fields(w, id).map_or(SNIP_SOUND, |f| f.strike_sound);
        play(w, key);
        let (x, y, z) = {
            let o = obj(w, id);
            (o.x, o.y, o.z)
        };
        let right = data(w, id).is_some_and(|d| d.claw_right);
        let p = if right {
            THROW_DISTANCE
        } else {
            -THROW_DISTANCE
        };
        bomb::spawn(w, x, y, z - 1, p);
        if let Some(f) = fields_mut(w, id) {
            f.armed = false;
        }
    }
    if done {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Falling out of a nest, and knocks

/// Starts falling out of a nest: flying until it lands.
pub fn enter_tumble(w: &mut World, id: ObjectId) {
    set_anim(w, id, FALL_ANIM);
    set_state(w, id, EnemyState::Tumble);
    obj_mut(w, id).physics_type = ptype::FLY;
}

/// Tumble: once landed it stops, takes its gravity on its feet, stands
/// on the floor below it and regroups.
pub fn tumble_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type != ptype::STAND {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
        d.ay = GRAVITY;
    }
    super::drop_to_floor(w, id);
    go_home(w, id);
}

/// Knocked aside: withdrawn, towards a spot 20 to 40 px from Claw on the
/// side away from the blow (`away_left`: the blow came from the right),
/// at 300 px/s when the spot is inside its limit on that side; otherwise
/// it keeps its speed. Draws: one of 0..21.
pub fn enter_nudge(w: &mut World, id: ObjectId, away_left: bool) {
    set_state(w, id, EnemyState::Nudge);
    set_anim(w, id, HIDE_ANIM);
    let r = rand(w, NUDGE_STOP_SPREAD);
    let cx = w.claw_object().map_or(obj(w, id).x, |c| c.x);
    let Some(d) = data_mut(w, id) else { return };
    let (min, max) = d.bounds;
    let tx = if away_left {
        let tx = cx.saturating_add(NUDGE_STOP + r);
        if min <= tx {
            d.vx = -f64::from(NUDGE_SPEED);
            d.vy = 0.0;
        }
        tx
    } else {
        let tx = cx.saturating_sub(NUDGE_STOP + r);
        if tx <= max {
            d.vx = f64::from(NUDGE_SPEED);
            d.vy = 0.0;
        }
        tx
    };
    d.human.target_x = tx;
}

/// Nudge: it regroups when it is at or past its spot or did not move.
pub fn nudge_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let exact = super::exact_x(w, id);
    let Some((vx, tx, pre)) = data(w, id).map(|d| (d.vx, d.human.target_x, d.human.pre_move_x))
    else {
        return;
    };
    let tx = f64::from(tx);
    let arrived = if vx < 0.0 { exact <= tx } else { exact >= tx };
    if arrived || exact == f64::from(pre) {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Lifted, thrown and dropped by Claw

/// True when `id` is a built hermit crab.
pub fn is_crab(w: &World, id: ObjectId) -> bool {
    fields(w, id).is_some()
}

/// `carrier` (Claw) picked up crab `id`: as a guard is lifted (see
/// [`lift::lifted`]), but with no line and no draw. It is busy, cannot be
/// hit, stays active everywhere, loses its patrol limits, takes the
/// carried movement box of [`CARRY`] and no longer moves by itself. A
/// crab being removed is not taken, nor a dead one (a dead crab is never
/// liftable, see [`on_hit`]), so a body cannot be thrown and get up
/// again. Draws nothing.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    let Some(dead) = data(w, id).map(|d| d.dead) else {
        return;
    };
    if dead || obj(w, id).flags & flags::DESTROY != 0 {
        return;
    }
    let z = w.objects.get(carrier).map(|c| c.z.saturating_add(1));
    if let Some(d) = data_mut(w, id) {
        d.z_sort = false;
    }
    if let Some(z) = z {
        let o = obj_mut(w, id);
        o.z = z;
        o.flags |= flags::RESORT;
    }
    set_state(w, id, EnemyState::Lifted);
    let o = obj_mut(w, id);
    o.physics_type = ptype::DIRECT;
    o.y -= 1;
    o.flags |= flags::ALWAYS_ACTIVE | flags::SAFE;
    let saved = facing_rect(o, o.move_rect);
    o.move_rect = facing_rect(o, CARRY.move_rect);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
        d.frozen = true;
        d.winding_up = false;
        d.saved_move_rect = saved;
        d.vx = 0.0;
        d.vy = 0.0;
    }
}

/// Claw starts winding up a throw of crab `id`: held over his head, it
/// shifts a little back and down (see [`lift::wind_up`]).
pub fn wind_up(w: &mut World, id: ObjectId) {
    lift::wind_up(w, id);
}

/// Claw threw crab `id` at (vx, vy) px/s: as a guard is thrown (see
/// [`lift::thrown`]), a thrown object that hurts what it flies into, but
/// falling ([`FALL_ANIM`]) and with no time limit in the air. Nothing
/// happens unless it is carried.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    if !matches!(
        super::state(w, id),
        Some(EnemyState::Lifted | EnemyState::Flailing)
    ) {
        return;
    }
    lift::thrown(w, id, vx, vy);
    set_anim(w, id, FALL_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.thrown_timer = Timer::default();
    }
}

/// Claw let go of crab `id`: it is dropped as a guard is (see
/// [`lift::dropped`]), and regroups when it gets up.
pub fn dropped(w: &mut World, id: ObjectId) {
    lift::dropped(w, id);
}

/// Thrown: once it has landed it is free, may leave the active area
/// again, starts its fall afresh and stops; it takes its own movement box
/// back with its feet where they are, is an enemy that can be hit again,
/// and regroups. No bounce, no rest and no damage.
pub fn thrown_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).physics_type != ptype::STAND {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.busy = false;
    }
    obj_mut(w, id).flags &= !flags::ALWAYS_ACTIVE;
    set_anim_now(w, id, FALL_ANIM);
    set_vx(w, id, 0.0);
    lift::restore_move_rect(w, id);
    let o = obj_mut(w, id);
    o.type_flags = types::ENEMY;
    o.flags &= !flags::SAFE;
    go_home(w, id);
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit the crab. A dead crab and a second hit by the same
/// explosion are ignored. On the crouched picture it hides, then is
/// knocked aside; on the withdrawn pictures it is knocked aside, unless
/// the magic claw hit it; on any other picture it dies and drops its
/// item. A knock plays the shell sound when the shared timer allows. A
/// pistol shot or a stick of dynamite is used up either way. Draws: the
/// hide's and the knock's, or the death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if super::state(w, id) == Some(EnemyState::Dead) {
        return;
    }
    let Some((kind, ax)) = w.objects.get(attacker).map(|a| (a.type_flags, a.x)) else {
        return;
    };
    let Some(f) = fields_mut(w, id) else { return };
    if f.last_attacker == Some(attacker) && kind == types::LEVEL_SHOT {
        return;
    }
    f.last_attacker = Some(attacker);
    let (frame, x) = {
        let o = obj(w, id);
        (o.frame, o.x)
    };
    let shell =
        frame == CROUCHED_FRAME || (is_shell_frame(frame) && kind != super::attacker::MAGIC);
    if shell {
        if frame == CROUCHED_FRAME {
            enter_hide(w, id);
        }
        timed_sound(w, id, SHELL_SOUND);
        enter_nudge(w, id, ax >= x);
    } else {
        die(w, id, DeathKind::Hit, Some(attacker));
        // A crab killed in its shell (by the magic claw) is liftable no
        // longer.
        obj_mut(w, id).special_flags &= !special::LIFTABLE;
        let item = record(w, id, crate::logic::enemy::field::POWERUP);
        if item > 0 {
            let (x, y) = {
                let o = obj(w, id);
                (o.x, o.y)
            };
            pickup::spawn_drop(w, x, y, item);
        }
    }
    if kind == types::PISTOL_SHOT {
        w.destroy(attacker);
    }
}

// ----------------------------------------------------------------------
// The nest

/// The sand nests: a patch of sand that hatches hermit crabs the first
/// time Claw stands or walks onto it.
///
/// The trigger is the record's box, or the nest picture's box when the
/// record leaves all four values at 0; Claw's movement box must share a
/// pixel with it (its right and bottom edges are outside it) while he
/// stands or walks (jumping, falling or climbing never sets it off). The
/// nest then plays its hatch ([`HATCH_ANIM`]): on the hatch's fourth
/// tick, as its second picture appears, it boinks and throws out its
/// crabs ([`crab_count`]) in a fan from left to right, each a bomber at
/// even odds; the hatch ends on its 16th tick and the nest never hatches
/// again, not even after Claw dies. Nothing can hit a nest.
///
/// Each crab leaves at (x, -(200 to 400)) px/s under 800 px/s²: the fan
/// gives the first [`fan_start`] px/s and each next one [`FAN_STEP`]
/// more, the last crab a drawn -200 to 200 px/s. With three crabs that
/// is -75, -25 and the drawn one. A crab is built at once where the nest
/// is (on the floor below it) and starts falling; its first tick comes
/// in the next update pass. Its flight starts from [`DROP_START`] px
/// above the nest, where its drop to the floor began, so on its first
/// tick it shows that much higher, and it lands later than a fall from
/// the floor would.
///
/// A record whose second user value is set throws its loot instead of
/// crabs; loot (the record's powerup and its two user boxes) goes to the
/// crabs in order, a crab taking the next entry while filled ones are
/// left, empty entries included (no retail nest carries loot).
///
/// Draws, all in the spawn tick, per crab: one of 0..201 (its upward
/// speed), one of 0..100 (a bomber below 50), and for the last crab one
/// of 0..401 (its speed across).
pub mod nest {
    use super::super::field;
    use crate::logic::enemy::{obj, obj_mut, play, rand, record};
    use crate::logic::{ObjectData, World, new_object, pickup};
    use crate::object::{ObjectId, Rect, engine_state, flags};

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["CrabNest"];

    /// The nest's picture (the records name it; the nest keeps it).
    pub const IMAGE_SET: &str = "LEVEL_SANDHOLE";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[];

    /// The hatch.
    pub const HATCH_ANIM: &str = "LEVEL_HATCHNEST";

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &[HATCH_ANIM];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = true;

    /// The sound of the hatch.
    pub const HATCH_SOUND: &str = "GAME_BOINK";
    /// The hatch tick on which the crabs appear.
    pub const SPAWN_TICK: u32 = 4;
    /// The hatch tick on which the nest goes quiet for good.
    pub const DONE_TICK: u32 = 16;
    /// The picture of the hatch on which the crabs appear.
    pub const SPAWN_FRAME: i32 = 2;
    /// Crabs of a nest whose record asks for none.
    pub const DEFAULT_CRABS: i32 = 3;
    /// The most crabs a nest hatches.
    pub const MAX_CRABS: i32 = 9;
    /// Horizontal spacing of the fan of crabs, px/s.
    pub const FAN_STEP: i32 = 50;
    /// The nest picture's box, relative to the nest, when the level does
    /// not know it.
    pub const PICTURE_BOX: Rect = Rect::new(-35, 44, 20, 63);
    /// Kind of object a nest is.
    pub const TYPE: u32 = 1;
    /// Kinds of object that could hit a nest; it has no hit box, so none
    /// does.
    pub const HIT_BY: u32 = 0x300_0072;
    /// Slowest upward speed of a hatched crab, px/s; a draw of 0 to
    /// [`UP_SPREAD`] - 1 adds to it.
    pub const UP_SPEED: i32 = 200;
    /// See [`UP_SPEED`].
    pub const UP_SPREAD: u32 = 201;
    /// The last crab's speed across is a draw of 0 to
    /// [`LAST_SPREAD`] - 1, less 200 px/s.
    pub const LAST_SPREAD: u32 = 401;
    /// Chance (of 100) that a hatched crab is a bomber.
    pub const BOMBER_CHANCE: i32 = 50;
    /// Record field of the second user value: set, the nest throws loot
    /// instead of crabs.
    pub const USER_1: usize = 44;
    /// How far above the nest a hatched crab's flight starts, px: its
    /// building drops it from there onto the floor, but it flies on from
    /// where the drop began.
    pub const DROP_START: i32 = 32;

    /// The nest's states, in its object's state.
    pub mod state {
        /// Waiting for Claw (or done).
        pub const IDLE: i32 = 1;
        /// Hatching.
        pub const HATCH: i32 = 2;
    }

    /// The crabs a nest hatches when its record asks for `user_0`: 3 for
    /// 0, at most 9.
    pub fn crab_count(user_0: i32) -> i32 {
        match user_0 {
            0 => DEFAULT_CRABS,
            n if n > MAX_CRABS => MAX_CRABS,
            n => n,
        }
    }

    /// The horizontal speed of the fan's first crab when `total` crabs and
    /// loot items leave the nest, px/s; each next one leaves
    /// [`FAN_STEP`] faster, except the last crab, whose speed is drawn.
    pub fn fan_start(total: i32) -> i32 {
        -(FAN_STEP * total) / 2
    }

    /// True when `a` and `b` share a pixel, their right and bottom edges
    /// being outside them.
    pub fn meets(a: Rect, b: Rect) -> bool {
        a.left.max(b.left) < a.right.min(b.right) && a.top.max(b.top) < a.bottom.min(b.bottom)
    }

    /// A nest's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct NestData {
        /// The trigger, in level pixels.
        pub trigger: Option<Rect>,
        /// The crabs it hatches.
        pub crabs: i32,
        /// It has hatched (for good).
        pub hatched: bool,
        /// Its crabs are out.
        pub spawned: bool,
        /// The crabs it hatched, in order.
        pub hatchlings: Vec<ObjectId>,
    }

    /// The nest data of `o`, if it is a nest.
    pub fn data_of(o: &crate::object::Object) -> Option<&NestData> {
        match &o.data {
            ObjectData::CrabNest(d) => Some(d),
            _ => None,
        }
    }

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        let mut d = match std::mem::take(&mut o.data) {
            ObjectData::CrabNest(d) => d,
            _ => NestData::default(),
        };
        match o.state {
            engine_state::INIT => set_up(w, id, &mut d),
            state::IDLE => idle(w, id, &mut d),
            state::HATCH => hatch(w, id, &mut d),
            _ => {}
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.data = ObjectData::CrabNest(d);
        }
    }

    fn set_up(w: &mut World, id: ObjectId, d: &mut NestData) {
        let (x, y) = {
            let o = obj(w, id);
            (o.x, o.y)
        };
        let r = |f| record(w, id, f);
        let (l, t, rr, b) = (
            r(field::X_MIN),
            r(field::Y_MIN),
            r(field::X_MAX),
            r(field::Y_MAX),
        );
        d.trigger = Some(if (l, t, rr, b) == (0, 0, 0, 0) {
            let o = obj(w, id);
            w.frame_box(&o.image_set, o.frame)
                .unwrap_or(PICTURE_BOX)
                .translate(x, y)
        } else {
            Rect::new(l, t, rr, b)
        });
        d.crabs = crab_count(r(field::USER_0));
        let o = obj_mut(w, id);
        o.type_flags = TYPE;
        o.hit_type_flags = HIT_BY;
        if o.z == 0 {
            o.z = 1;
            o.flags |= flags::RESORT;
        }
        o.state = state::IDLE;
    }

    fn idle(w: &mut World, id: ObjectId, d: &mut NestData) {
        if d.hatched {
            return;
        }
        let Some(trigger) = d.trigger else { return };
        let Some(c) = w.claw_object() else { return };
        let on_foot = matches!(
            c.physics_type,
            crate::physics::ptype::STAND | crate::physics::ptype::WALK
        );
        if !on_foot || !c.move_rect.is_set() || !meets(c.move_rect.translate(c.x, c.y), trigger) {
            return;
        }
        d.hatched = true;
        obj_mut(w, id).state = state::HATCH;
        w.set_anim(id, HATCH_ANIM);
    }

    fn hatch(w: &mut World, id: ObjectId, d: &mut NestData) {
        w.advance_anim(id);
        if obj(w, id).frame == SPAWN_FRAME && !d.spawned {
            play(w, HATCH_SOUND);
            spawn(w, id, d);
            d.spawned = true;
        }
        if obj(w, id).anim.is_done() {
            obj_mut(w, id).state = state::IDLE;
        }
    }

    /// The loot list of a nest: its powerup, then its first user box's
    /// left, right, top and bottom, then its second's.
    fn loot(w: &World, id: ObjectId) -> [i32; 9] {
        let r = |f| record(w, id, f);
        let [l1, t1, r1, b1] = field::USER_RECT_1;
        let [l2, t2, r2, b2] = field::USER_RECT_2;
        [
            r(field::POWERUP),
            r(l1),
            r(r1),
            r(t1),
            r(b1),
            r(l2),
            r(r2),
            r(t2),
            r(b2),
        ]
    }

    fn spawn(w: &mut World, id: ObjectId, d: &mut NestData) {
        let list = loot(w, id);
        let mut n = list.iter().filter(|&&v| v != 0).count() as i32;
        let total = n.max(d.crabs);
        let mut f = fan_start(total);
        let (x, y, z) = {
            let o = obj(w, id);
            (o.x, o.y, o.z)
        };
        let crabs_only = record(w, id, USER_1) == 0;
        let mut i = 0usize;
        let mut remaining = d.crabs;
        for _ in 0..total {
            let vy = -(UP_SPEED + rand(w, UP_SPREAD));
            if remaining > 0 && crabs_only {
                let crab = w.spawn(new_object("HermitCrab", x, y, z - 1));
                super::construct(w, crab);
                let bomber = rand(w, 100) < BOMBER_CHANCE;
                let vx = if remaining == 1 {
                    rand(w, LAST_SPREAD) - 200
                } else {
                    f
                };
                let at = w.objects.get(crab).map(|o| (o.x, o.y));
                if let Some(e) = crate::logic::enemy::data_mut(w, crab) {
                    e.vx = f64::from(vx);
                    e.vy = f64::from(vy);
                    e.ay = super::NEST_GRAVITY;
                    // Its building puts it on the floor, but its flight
                    // starts from where that drop began, 32 px above the
                    // nest.
                    if let Some(at) = at {
                        e.carry = (0.0, f64::from(y - DROP_START - at.1));
                        e.moved_to = Some(at);
                    }
                }
                super::enter_tumble(w, crab);
                super::set_bomber(w, crab, bomber);
                if n > 0 && i < 9 {
                    if let Some(o) = w.objects.get_mut(crab)
                        && let Some(r) = o.record.as_deref_mut()
                    {
                        r[field::POWERUP] = list[i];
                    } else if let Some(o) = w.objects.get_mut(crab) {
                        let mut r = Box::new([0; claw_assets::wwd::OBJECT_INTS]);
                        r[field::POWERUP] = list[i];
                        o.record = Some(r);
                    }
                    i += 1;
                    n -= 1;
                }
                d.hatchlings.push(crab);
                remaining -= 1;
            } else if n > 0 && i < 9 {
                if let Some(g) = pickup::spawn_drop(w, x, y, list[i])
                    && let Some(ObjectData::Pickup(p)) = w.objects.get_mut(g).map(|o| &mut o.data)
                    && let Some(goodie) = p.goodie.as_mut()
                {
                    goodie.vx = f64::from(f);
                    goodie.vy = f64::from(vy);
                }
                i += 1;
                n -= 1;
            }
            f += FAN_STEP;
        }
    }

    /// `attacker` hit `id`. Nothing hits a nest.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` attacked `victim`. A nest attacks nothing.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        let _ = (w, id, victim);
    }
}

/// The bomb a bomber crab throws: a small round bomb lobbed in an arc
/// towards Claw ([`throw_velocity`]) that bounces twice and then rests,
/// and explodes [`FUSE_MS`] after the throw, or at once when it touches
/// Claw (costing him [`DAMAGE`]). The blast ([`BLAST`]) hurts Claw,
/// kills any crab caught outside its shell, the thrower included, hurts
/// the other enemies, sets off kegs and breaks crates, for the whole of
/// its 16 ticks. Nothing can hit a bomb, and it never sleeps.
///
/// In flight the bomb is an enemy shot whose box is the blast's: it hurts
/// Claw and breaks crates, and touches no enemy. A bomb that reaches Claw
/// when he can be hurt explodes where it is (in the collision pass of the
/// throw tick when it leaves inside his reach), and so does an exploding
/// one, which starts its explosion again; his window after a hit keeps a
/// blast from doing that twice. Its motion is thrown dynamite's, with a
/// 1000 ms fuse, falling through platforms; it does not move in the tick
/// it is thrown, and one thrown inside solid ground is gone at once. It
/// is removed when Claw respawns.
pub mod bomb {
    use crate::logic::projectile::{Motion, RESPAWN_PURGE};
    use crate::logic::{ObjectData, World, new_object};
    use crate::object::{ObjectId, Rect, flags, special, types};
    use crate::physics::{ptype, res};
    use crate::tiles::SOLID;
    use crate::tuning::TICK_MS;

    /// Logic names this family runs.
    pub const NAMES: &[&str] = &["CrabBomb"];

    /// The bomb's picture.
    pub const IMAGE_SET: &str = "LEVEL_CRABBOMB";

    /// Image sets this family may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

    /// Prefixes of the animations this family may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_CRABBOMB_"];

    /// Whether this family runs its logics yet (see
    /// [`crate::logic::built`]).
    pub const BUILT: bool = true;

    /// The flight.
    pub const FALL_ANIM: &str = "LEVEL_CRABBOMB_FALL";
    /// The explosion (its first picture plays the explosion's sound).
    pub const EXPLODE_ANIM: &str = "LEVEL_CRABBOMB_EXPLODE";

    /// Damage of the bomb and of its blast (no difficulty factor).
    pub const DAMAGE: i32 = 5;
    /// The bomb's box in flight and the blast's.
    pub const BLAST: Rect = Rect::new(-50, -50, 50, 50);
    /// The bomb's movement box when the level does not know its picture.
    pub const MOVE_RECT: Rect = Rect::new(-11, -7, 12, 14);
    /// Time from the throw to the explosion, ms.
    pub const FUSE_MS: u32 = 1000;
    /// Gravity on a bomb, px/s^2.
    pub const GRAVITY: f64 = 2000.0;
    /// Landings that bounce; the next one rests.
    pub const BOUNCES: u32 = 2;
    /// Ticks of the explosion at the 18 ms step.
    pub const EXPLODE_TICKS: u32 = 16;
    /// The bomb's states, in its object's state.
    pub mod state {
        /// Flying, bouncing or resting.
        pub const FALLING: i32 = 1;
        /// Exploding.
        pub const EXPLODING: i32 = 2;
    }

    /// The throw's velocity for a bomb aimed `distance` px ahead (negative
    /// to the left), px/s: a time of flight `t = sqrt(|distance| / 1000)`
    /// s gives `(distance / t, -1000 t - 2 |distance|)`.
    pub fn throw_velocity(distance: i32) -> (f64, f64) {
        let p = f64::from(distance);
        let t = (p.abs() / 1000.0).sqrt();
        if t == 0.0 {
            return (0.0, 0.0);
        }
        (p / t, -t * 1000.0 - (2.0 * p).abs())
    }

    /// A bomb's data.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct BombData {
        /// Exact position, velocity and gravity.
        pub motion: Motion,
        /// Its own clock, s (as a projectile's); `None` before its first
        /// step.
        pub clock_s: Option<f64>,
        /// When the fuse was lit, ms.
        pub fuse_start_ms: u64,
        /// Landings bounced off.
        pub bounces: u32,
        /// No motion: resting or exploding.
        pub frozen: bool,
        /// The explosion has reached its blast picture once.
        pub latched: bool,
        /// The crab that threw it.
        pub thrower: Option<ObjectId>,
    }

    /// The bomb data of `o`, if it is a bomb.
    pub fn data_of(o: &crate::object::Object) -> Option<&BombData> {
        match &o.data {
            ObjectData::CrabBomb(d) => Some(d),
            _ => None,
        }
    }

    /// Throws a bomb from (x, y) at depth z, aimed `distance` px ahead,
    /// and returns it. Its fuse is lit now.
    pub fn spawn(w: &mut World, x: i32, y: i32, z: i32, distance: i32) -> ObjectId {
        let mut o = new_object(NAMES[0], x, y, z);
        o.image_set = crate::level::runtime_image_set(IMAGE_SET);
        o.frame = w.image_frames(IMAGE_SET).map_or(1, |(first, _)| first);
        o.move_rect = w.frame_box(IMAGE_SET, o.frame).unwrap_or(MOVE_RECT);
        o.attack_rect = BLAST;
        o.type_flags = types::ENEMY_SHOT;
        o.attack_type_flags = types::PLAYER;
        o.attack_handler = true;
        o.hit_type_flags = 0;
        o.damage = DAMAGE;
        o.special_flags |= special::OWN_IMPACT | RESPAWN_PURGE;
        o.flags |= flags::ALWAYS_ACTIVE;
        o.physics_type = ptype::FLY;
        o.state = state::FALLING;
        let (vx, vy) = throw_velocity(distance);
        o.speed_x = vx as i32;
        o.speed_y = vy as i32;
        let buried = w.level.map.attr(x, y + o.move_rect.bottom) == SOLID;
        o.data = ObjectData::CrabBomb(BombData {
            motion: Motion {
                x: f64::from(x),
                y: f64::from(y),
                vx,
                vy,
                ay: GRAVITY,
            },
            fuse_start_ms: w.now_ms,
            ..BombData::default()
        });
        let id = w.spawn(o);
        w.set_anim(id, FALL_ANIM);
        if buried {
            w.destroy(id);
        }
        id
    }

    /// One logic call.
    pub fn tick(w: &mut World, id: ObjectId) {
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        let state = o.state;
        let mut d = match std::mem::take(&mut o.data) {
            ObjectData::CrabBomb(d) => d,
            _ => BombData::default(),
        };
        match state {
            state::FALLING => falling(w, id, &mut d),
            state::EXPLODING => exploding(w, id, &mut d),
            _ => {}
        }
        if let Some(o) = w.objects.get_mut(id) {
            o.data = ObjectData::CrabBomb(d);
        }
    }

    fn falling(w: &mut World, id: ObjectId, d: &mut BombData) {
        w.advance_anim(id);
        if w.now_ms.saturating_sub(d.fuse_start_ms) >= u64::from(FUSE_MS) {
            d.frozen = true;
            explode(w, id);
        }
        let landed = w
            .objects
            .get(id)
            .is_some_and(|o| o.physics_type == ptype::STAND);
        if landed {
            if d.bounces >= BOUNCES {
                d.frozen = true;
            } else {
                bounce(w, id, d);
            }
        }
        integrate(w, id, d);
    }

    fn exploding(w: &mut World, id: ObjectId, d: &mut BombData) {
        if w.advance_anim(id) == 1 {
            d.latched = true;
        }
        if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
            w.destroy(id);
        }
    }

    /// Starts (or starts again) the explosion: no longer hittable, the
    /// level's own kind of shot, its explosion from its first picture.
    fn explode(w: &mut World, id: ObjectId) {
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        o.state = state::EXPLODING;
        o.flags |= flags::SAFE;
        o.type_flags = types::LEVEL_SHOT;
        w.set_anim(id, EXPLODE_ANIM);
    }

    /// `attacker` hit `id`. Nothing hits a bomb.
    pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
        let _ = (w, id, attacker);
    }

    /// `id` reached `victim`: when that is Claw and he takes the hit, the
    /// bomb explodes where it is (again, when it is already exploding).
    /// His own hit rule applies the damage.
    pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
        if w.claw_id != Some(victim) {
            return;
        }
        let (Some(b), Some(c)) = (w.objects.get(id), w.objects.get(victim)) else {
            return;
        };
        if b.type_flags & c.hit_type_flags == 0 || w.aids.invincible {
            return;
        }
        let mut d = data_of(b).cloned().unwrap_or_default();
        d.frozen = true;
        explode(w, id);
        if let Some(o) = w.objects.get_mut(id) {
            o.data = ObjectData::CrabBomb(d);
        }
    }

    fn sign(v: f64) -> f64 {
        if v > 0.0 { 1.0 } else { -1.0 }
    }

    /// The time the motion step of the tick covers, s: one tick at first,
    /// then the game time since the bomb's own clock (see the projectiles'
    /// motion step).
    fn step_seconds(d: &mut BombData, now_ms: u64) -> f64 {
        let now = (now_ms + u64::from(TICK_MS)) as f64 * 0.001;
        match d.clock_s {
            Some(prev) => {
                let dt = now - prev;
                d.clock_s = Some(prev + dt);
                dt
            }
            None => {
                d.clock_s = Some(now);
                f64::from(TICK_MS) / 1000.0
            }
        }
    }

    /// One motion step through the tiles (not the platforms), unless it
    /// is frozen, as thrown dynamite moves.
    fn integrate(w: &mut World, id: ObjectId, d: &mut BombData) {
        if d.frozen {
            return;
        }
        let t = step_seconds(d, w.now_ms);
        d.motion.step(t);
        let (tx, ty) = (d.motion.x as i32, d.motion.y as i32);
        let r = w.physics(id, tx, ty, 0);
        let Some(o) = w.objects.get_mut(id) else {
            return;
        };
        if o.x != tx {
            d.motion.x = f64::from(o.x);
        }
        if o.y != ty {
            d.motion.snap_y(f64::from(o.y));
        }
        if r & res::CEILING != 0 {
            d.motion.vy = -d.motion.vy;
        } else if r & (res::RIGHT | res::LEFT) != 0 {
            d.motion.vx *= -0.5;
        }
        o.speed_x = d.motion.vx as i32;
        o.speed_y = d.motion.vy as i32;
    }

    /// Bounces a landed bomb: it rises 1 px and flies on with half its
    /// speed across and its landing speed, corrected for that pixel,
    /// reversed and halved.
    fn bounce(w: &mut World, id: ObjectId, d: &mut BombData) {
        d.bounces += 1;
        if let Some(o) = w.objects.get_mut(id) {
            o.physics_type = ptype::FLY;
        }
        let m = &mut d.motion;
        let vy = sign(m.vy) * (m.vy * m.vy - 2.0 * m.ay).max(0.0).sqrt();
        m.y -= 1.0;
        m.vx *= 0.5;
        m.vy = vy * -0.5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_states_fill_the_upper_half_of_the_block_in_order() {
        let start = super::super::bear_sailor::CRAB_STATES_START;
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, start + i as i32, "{}", s.name());
            assert!(STATE_BLOCK.contains(&(s as i32)));
        }
        assert_eq!(*STATE_BLOCK.end(), start + 9, "one number left free");
        assert!(!CLASS.human, "the crab is no human enemy");
    }

    #[test]
    fn only_the_shell_pictures_protect_it() {
        for f in SHELL_FRAMES {
            assert!(is_shell_frame(f), "{f}");
        }
        for f in [1, 2, 200, THROW_FRAME, 203, 204, 799, 803, 950] {
            assert!(!is_shell_frame(f), "{f}");
        }
        assert!(SNIP_FRAMES.iter().all(|f| !is_shell_frame(*f)));
    }

    #[test]
    fn its_boxes_come_from_its_first_picture() {
        let h = HIT_RECT;
        assert_eq!(
            MOVE_RECT,
            Rect::new(h.left, h.top + 10, h.right, h.bottom + 5)
        );
        assert_eq!(
            SNIP_BOX,
            Rect::new(h.left - 15, h.top - 10, h.right + 15, h.bottom + 10)
        );
    }

    #[test]
    fn a_nest_hatches_three_crabs_unless_its_record_asks_otherwise() {
        assert_eq!(nest::crab_count(0), 3);
        assert_eq!(nest::crab_count(1), 1);
        assert_eq!(nest::crab_count(9), 9);
        assert_eq!(nest::crab_count(10), 9);
        assert_eq!(nest::crab_count(100), 9);
        // Three crabs leave at -75 and -25 px/s, the last at a drawn speed.
        assert_eq!(nest::fan_start(3), -75);
        assert_eq!(nest::fan_start(3) + nest::FAN_STEP, -25);
        // The halving truncates towards zero.
        assert_eq!(nest::fan_start(1), -25);
        const { assert!(nest::SPAWN_TICK < nest::DONE_TICK) };
    }

    #[test]
    fn a_nest_trigger_needs_a_shared_pixel() {
        let t = Rect::new(0, 0, 10, 10);
        assert!(nest::meets(Rect::new(9, 9, 20, 20), t));
        assert!(!nest::meets(Rect::new(10, 0, 20, 10), t), "edge to edge");
        assert!(!nest::meets(Rect::new(-10, 10, 5, 20), t));
        assert!(nest::meets(Rect::new(-10, -10, 1, 1), t));
    }

    #[test]
    fn a_bomb_is_lobbed_one_hundred_and_fifty_px() {
        let (vx, vy) = bomb::throw_velocity(THROW_DISTANCE);
        assert!((vx - 387.298).abs() < 1e-3, "{vx}");
        assert!((vy - -687.298).abs() < 1e-3, "{vy}");
        let (vx, vy2) = bomb::throw_velocity(-THROW_DISTANCE);
        assert!((vx - -387.298).abs() < 1e-3, "{vx}");
        assert_eq!(vy, vy2, "the same height either way");
        assert_eq!(bomb::throw_velocity(0), (0.0, 0.0));
        // A 1000 ms fuse: the 56th tick after the throw.
        assert_eq!(bomb::FUSE_MS.div_ceil(18), 56);
    }

    #[test]
    fn a_bomber_is_more_cautious_far_and_bolder_close() {
        let [hermit, bomber] = DEFENSE_CHANCES;
        assert_eq!(hermit[2], (30, 40));
        assert_eq!(bomber[4], (40, 40));
        // Mode 1 decides as mode 0 does.
        assert_eq!(hermit[0], hermit[1]);
        assert_eq!(bomber[0], bomber[1]);
    }
}
