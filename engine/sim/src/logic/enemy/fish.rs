//! The Fish of the eleventh and twelfth levels: pond fish that patrol the
//! wading pools and leap at Claw.
//!
//! A fish swims up and down its pool along its record's height (its swim
//! line), between the ends of its record's x range, at [`SWIM_SPEED`]
//! px/s ([`EnemyState::Swim`]). Swimming towards a Claw close by, it
//! leaps out of the water at him when he stands above the line
//! ([`EnemyState::JumpStrike`], a ballistic arc [`LEAP`] px across and up
//! under [`LEAP_GRAVITY`], [`LEAP_TICKS`] ticks back into the water), and
//! otherwise dashes at where he stands ([`EnemyState::Snip`],
//! [`DASH_SPEED`] px/s). A leap that comes down outside the pool leaves it
//! flopping on dry ground until it slides back in
//! ([`EnemyState::OutOfWater`]). Its whole picture hurts Claw on contact
//! ([`DAMAGE`]), and any blow or shot that reaches it kills it at once.
//! It is never lifted and drops nothing.
//!
//! A flopping fish faces by one toggle the whole game shares
//! ([`crate::logic::World::fish_flop`]), flipped at every flop.
//!
//! # The rules
//!
//! - Built: the construction step every enemy shares (a fish placed
//!   beside its range starts on the nearer limit); its box from its first
//!   picture for moving, being hit and hurting alike ([`RECT`]); hit by
//!   Claw, his pistol shots, his magic and the level's shots and
//!   explosions; the range boxes [`RANGES`] (the second empty); damage
//!   [`DAMAGE`] unless its record sets one; drawn in front of Claw or
//!   behind him by who stands higher. Its swim line is its height then,
//!   its home its range then; it is not dropped to the floor. Then it
//!   swims.
//! - Swimming (going idle: moved without tile tests, its range back to
//!   its home, no gravity, on its swim line; on at 90 px/s to the right
//!   when it was moving right and is not exactly on its right limit,
//!   otherwise to the left, facing the way it swims): exactly on its left
//!   limit it turns right, otherwise exactly on its right limit it turns
//!   left. With Claw within 100 px across and 300 px up or down but
//!   outside the 50 px box around it (range mode 3) and the fish swimming
//!   towards him: with Claw above its line, 80 draws of 101 make it leap;
//!   with Claw level or below and across inside its home, 90 of 101 make
//!   it dash. The draw is made on every tick the test holds.
//! - The leap: flying under tile collision with [`LEAP_GRAVITY`], no
//!   range, the slowest launch that would carry it 150 px towards Claw
//!   and 150 px above its line; the leaving splash. On the picture 202 of
//!   its strike, once, the bite; once below its line within its home, the
//!   splash back in and it swims again; landed on ground first, it flops.
//!   A wall it meets turns its speed across around at half, so a fish
//!   that leaps at a pit's wall falls back into its pool.
//! - The dash: 180 px/s towards Claw, on its line, to where Claw stood
//!   when it started; the bite on picture 202; then it swims on.
//! - The flop: flying under the leap's gravity, sliding at 60 px/s
//!   towards its range (to the right unless it lies at or past its right
//!   limit), a flop every 250 ms that faces it by the shared toggle and
//!   flips it; once below its line within its home, the splash back in and
//!   it swims again.
//! - Hits: any hit in any state but dead kills it (a pistol shot is used
//!   up, magic flies on), without the hit gate the other enemies have.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): its dash is the crabs'
//! `Snip` (2195), which the game registers under the same name for both,
//! run with the Fish's own handler; its swim, leap and flop are new
//! names, 2270 to 2272 in the eleventh level's block
//! ([`super::mercat::STATE_BLOCK`]), in the order the game registers them
//! ([`STATES`]); it dies in the shared `Dead`.
//!
//! # Random draws, in order
//!
//! - building and going idle: none;
//! - the pre-tick: the shared roll (frame pulse ticks only);
//! - swimming: one of 0..101 on every tick the leap's or the dash's test
//!   holds;
//! - the leap, the dash and the flop: none;
//! - hit: the shared death's (one of 0..100, then one of 0..10 for one of
//!   Claw's lines after his blow or shot).
//!
//! # Defaults taken
//!
//! - The second range box, which the game never sets, is empty (0, 0):
//!   it only decides a mode the Fish never acts on (measured).
//! - The flop toggle starts at 1 when the game is created and is kept
//!   for the level's whole simulation; a new level's simulation starts it
//!   at 1 again. The original keeps it across level loads, which differs
//!   only for a fish flopping on the twelfth level after one flopped on
//!   the eleventh in the same run; no flop was seen in any measured leap.
//! - The flop itself was never seen in a run of the original: it follows
//!   the rules above.
//!
//! Its data ([`FishData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::{
    Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, Handler, Timer,
    UNBOUNDED, data, data_mut, die, exact_x, facing_rect, human, nothing, obj, obj_mut, play, rand,
    record_bounds, record_bounds_y, set_anim, set_facing, set_state,
};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, flags, special, types};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Fish"];

/// Its picture.
pub const IMAGE_SET: &str = "LEVEL_FISH";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_FISH_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Its three new states, in the order the game registers them.
pub const STATES: [EnemyState; 3] = [
    EnemyState::Swim,
    EnemyState::JumpStrike,
    EnemyState::OutOfWater,
];

/// Its dash: the crabs' state of the same name.
pub const DASH: EnemyState = EnemyState::Snip;

/// Contact damage when its record sets none (no record does); not scaled
/// by the difficulty.
pub const DAMAGE: i32 = 10;
/// Its box, from its first picture: movement, hit, contact and strike
/// boxes alike.
pub const RECT: Rect = Rect::new(-19, -30, 24, 13);
/// The five range boxes (half width, half height), outermost first; the
/// second is never set.
pub const RANGES: [(i32, i32); 5] = [(400, 50), (0, 0), (300, 50), (100, 300), (50, 50)];
/// Kinds of object that can hit it: Claw, his pistol shots, the level's
/// shots and explosions, and his magic.
pub const HIT_BY: u32 = 0x300_0022;

/// Swimming speed, px/s.
pub const SWIM_SPEED: i32 = 90;
/// Dashing speed, px/s.
pub const DASH_SPEED: i32 = 180;
/// Sliding speed of a flopping fish, px/s.
pub const FLOP_SPEED: i32 = 60;
/// How far across towards Claw and how far up a leap aims, px.
pub const LEAP: (i32, i32) = (150, 150);
/// Gravity of a leap and a flop, px/s^2.
pub const LEAP_GRAVITY: f64 = 2000.0;
/// The chance in 101 that a swimming fish leaps at a Claw above its
/// line.
pub const LEAP_CHANCE: i32 = 80;
/// The chance in 101 that a swimming fish dashes at a Claw within its
/// range.
pub const DASH_CHANCE: i32 = 90;
/// Ticks of a leap at the 18 ms step, back into the water on the last
/// (measured).
pub const LEAP_TICKS: u32 = 44;
/// Ticks of a dash at the 18 ms step (measured).
pub const DASH_TICKS: u32 = 31;
/// Time between two flops, ms.
pub const FLOP_MS: u32 = 250;
/// The picture of its strike on which it bites.
pub const CHOMP_FRAME: i32 = 202;

/// Swimming.
pub const SWIM_ANIM: &str = "LEVEL_FISH_SWIM";
/// The leap and the dash.
pub const STRIKE_ANIM: &str = "LEVEL_FISH_STRIKE1";
/// Flopping.
pub const FLOP_ANIM: &str = "LEVEL_FISH_FLOUNDER";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_FISH_KILLFALL";
/// The bite.
pub const CHOMP_SOUND: &str = "LEVEL_FISH_CHOMP1";
/// Leaving the water.
pub const OUT_SOUND: &str = "LEVEL_FISH_WATEROUT";
/// Back into the water.
pub const IN_SOUND: &str = "LEVEL_FISH_WATERIN";
/// A flop.
pub const FLOP_SOUND: &str = "LEVEL_FISH_FISHFLOP";
/// Its death.
pub const DEATH_SOUND: &str = "LEVEL_FISH_FISHDEATH";

/// The Fish class: a plain enemy, never lifted.
pub static CLASS: Class = Class {
    name: "Fish",
    human: false,
    construct,
    pre_tick: nothing,
    handler,
    go_home: nothing,
    go_idle,
    carry: None,
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims {
        fall: DEATH_ANIM,
        ..ClassAnims::NONE
    },
    sounds: ClassSounds::NONE,
};

/// A fish's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FishData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Its swim line: its height when built.
    pub line: i32,
    /// Its home: its range across when built (min, max).
    pub home: (i32, i32),
    /// Where a dash stops: Claw's x when it started.
    pub target: i32,
    /// The bite of the current leap or dash has sounded.
    pub chomped: bool,
    /// The time between two flops.
    pub flop: Timer,
}

impl World<'_> {
    /// The flop toggle the whole game shares, 1 or 0: a flopping fish
    /// faces left on 1 and right on 0, then flips it (see the module
    /// documentation). It starts at 1.
    pub fn fish_flop(&self) -> i32 {
        if self.out.fish_flop_zero { 0 } else { 1 }
    }

    /// Sets the flop toggle to `value` (0 for 0, anything else for 1).
    pub fn set_fish_flop(&mut self, value: i32) {
        self.out.fish_flop_zero = value == 0;
    }
}

/// The fish data of `id`.
pub fn fish_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a FishData> {
    match &w.objects.get(id)?.data {
        ObjectData::Fish(d) => Some(d),
        _ => None,
    }
}

fn fish_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut FishData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Fish(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Swim => Some(swim_tick),
        EnemyState::JumpStrike => Some(leap_tick),
        EnemyState::Snip => Some(dash_tick),
        EnemyState::OutOfWater => Some(flop_tick),
        _ => None,
    }
}

/// One logic call: the shared enemy logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id, &CLASS);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds a fish (see the module documentation), then it swims. Draws
/// nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    super::clamp_on_record_range(w, id);
    // The record's damage, which the object starts with, or the default.
    let damage = match obj(w, id).damage {
        0 => DAMAGE,
        d => d,
    };
    let fields = EnemyFields {
        mode: 6,
        bounds: record_bounds(w, id),
        bounds_y: record_bounds_y(w, id),
        ranges: RANGES,
        z_sort: true,
        z_base: 0,
        ay: 0.0,
        contact_rect: RECT,
        strike_rect: RECT,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        ..EnemyFields::default()
    };
    let frames = w.image_frames(IMAGE_SET);
    let rect = frames
        .and_then(|(first, _)| w.frame_box(IMAGE_SET, first))
        .unwrap_or(RECT);
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    if let Some((first, _)) = frames {
        o.frame = first;
    }
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_BY;
    o.special_flags &= !special::LIFTABLE;
    o.flags |= flags::DESTROY_FAR;
    o.hit_handler = true;
    o.move_rect = rect;
    o.hit_rect = rect;
    o.attack_rect = rect;
    if super::faces_right(o) {
        o.mirror_rects();
    }
    o.damage = damage;
    let (line, x_min, x_max) = (o.y, fields.bounds.0, fields.bounds.1);
    o.data = ObjectData::Fish(FishData {
        enemy: EnemyFields {
            contact_rect: rect,
            strike_rect: rect,
            ..fields
        }
        .into(),
        line,
        home: (x_min, x_max),
        ..FishData::default()
    });
    go_idle(w, id);
}

// ----------------------------------------------------------------------
// Swimming

fn home(w: &World, id: ObjectId) -> (i32, i32) {
    fish_data(w, id).map_or(UNBOUNDED, |f| f.home)
}

fn line(w: &World, id: ObjectId) -> i32 {
    fish_data(w, id).map_or_else(|| obj(w, id).y, |f| f.line)
}

/// The fish's exact height: its y plus the fraction its last movement
/// step carried, unless something else has moved it since.
pub fn exact_y(w: &World, id: ObjectId) -> f64 {
    let o = obj(w, id);
    let carry = data(w, id)
        .filter(|d| d.moved_to == Some((o.x, o.y)))
        .map_or(0.0, |d| d.carry.1);
    f64::from(o.y) + carry
}

/// Its box in use as its attack box, facing the way it faces.
fn arm(w: &mut World, id: ObjectId) {
    let rect = data(w, id).map_or(RECT, |d| d.contact_rect);
    let o = obj_mut(w, id);
    o.attack_rect = facing_rect(o, rect);
}

/// Goes idle: it swims again (see the module documentation). Its exact
/// position across is kept. Draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    let (min, max) = home(w, id);
    let line = line(w, id);
    let ex = exact_x(w, id);
    let (x, y) = {
        let o = obj_mut(w, id);
        o.physics_type = ptype::DIRECT;
        let at = (o.x, o.y);
        o.y = line;
        at
    };
    let old_vx = data(w, id).map_or(0.0, |d| d.vx);
    if let Some(d) = data_mut(w, id) {
        d.bounds = (min, max);
        d.ay = 0.0;
        d.vx = 0.0;
        d.vy = 0.0;
        // Set on its line: its fraction across stays, the one up and down
        // goes.
        if d.moved_to == Some((x, y)) {
            d.moved_to = Some((x, line));
        }
        d.carry.1 = 0.0;
    }
    let right = old_vx.trunc() > 0.0 && ex != f64::from(max);
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(if right { SWIM_SPEED } else { -SWIM_SPEED });
    }
    set_facing(w, id, right);
    set_state(w, id, EnemyState::Swim);
    set_anim(w, id, SWIM_ANIM);
}

/// Swim (see the module documentation). Draws: one of 0..101 when the
/// leap's or the dash's test holds.
pub fn swim_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let (min, max) = data(w, id).map_or(UNBOUNDED, |d| d.bounds);
    let ex = exact_x(w, id);
    if ex == f64::from(min) {
        set_swim(w, id, true);
    } else if ex == f64::from(max) {
        set_swim(w, id, false);
    }
    let Some((mode, claw_right, vx)) = data(w, id).map(|d| (d.mode, d.claw_right, d.vx)) else {
        return;
    };
    let towards = (vx < 0.0 && !claw_right) || (vx > 0.0 && claw_right);
    if mode != 3 || !towards {
        return;
    }
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return;
    };
    if f64::from(cy) < exact_y(w, id) {
        if rand(w, 101) < LEAP_CHANCE {
            enter_leap(w, id);
        }
        return;
    }
    let (home_min, home_max) = home(w, id);
    if (home_min..=home_max).contains(&cx) && rand(w, 101) < DASH_CHANCE {
        enter_dash(w, id, cx);
    }
}

/// Swims on to the right (`right`) or the left, facing that way.
fn set_swim(w: &mut World, id: ObjectId, right: bool) {
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(if right { SWIM_SPEED } else { -SWIM_SPEED });
    }
    set_facing(w, id, right);
}

// ----------------------------------------------------------------------
// The leap

/// The leap at Claw (see the module documentation): its strike box armed,
/// flying, the strike, no range, gravity, the launch towards 150 px across
/// towards Claw and 150 px above its line, and the leaving splash.
pub fn enter_leap(w: &mut World, id: ObjectId) {
    arm(w, id);
    obj_mut(w, id).physics_type = ptype::FLY;
    set_state(w, id, EnemyState::JumpStrike);
    set_anim(w, id, STRIKE_ANIM);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    let dx = f64::from(if right { LEAP.0 } else { -LEAP.0 });
    let target_y = f64::from(line(w, id) - RANGES[3].1 / 2);
    let dy = target_y - exact_y(w, id);
    if let Some(d) = data_mut(w, id) {
        d.bounds = UNBOUNDED;
        d.bounds_y = UNBOUNDED;
        d.ay = LEAP_GRAVITY;
        let (vx, vy) = human::launch(dx, dy, d.ay);
        d.vx = vx;
        d.vy = vy;
    }
    play(w, OUT_SOUND);
    if let Some(f) = fish_data_mut(w, id) {
        f.chomped = false;
    }
}

/// The bite on picture 202 of its strike, once per leap or dash.
fn chomp(w: &mut World, id: ObjectId) {
    if obj(w, id).frame == CHOMP_FRAME && fish_data(w, id).is_some_and(|f| !f.chomped) {
        if let Some(f) = fish_data_mut(w, id) {
            f.chomped = true;
        }
        play(w, CHOMP_SOUND);
    }
}

/// True once the fish is below its line within its home.
fn back_in(w: &World, id: ObjectId) -> bool {
    let (min, max) = home(w, id);
    let ex = exact_x(w, id);
    exact_y(w, id) > f64::from(line(w, id)) && ex >= f64::from(min) && ex <= f64::from(max)
}

/// JumpStrike (see the module documentation).
pub fn leap_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        arm(w, id);
    }
    if obj(w, id).physics_type == ptype::STAND {
        arm(w, id);
        enter_flop(w, id);
    }
    chomp(w, id);
    if back_in(w, id) {
        play(w, IN_SOUND);
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// The dash

/// The dash at `claw_x` (see the module documentation): its strike box
/// armed, the strike, 180 px/s towards Claw.
pub fn enter_dash(w: &mut World, id: ObjectId, claw_x: i32) {
    arm(w, id);
    set_state(w, id, DASH);
    set_anim(w, id, STRIKE_ANIM);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(if right { DASH_SPEED } else { -DASH_SPEED });
    }
    if let Some(f) = fish_data_mut(w, id) {
        f.target = claw_x;
        f.chomped = false;
    }
}

/// The dash: the bite on picture 202; at or past where Claw stood, its
/// contact box and it swims on.
pub fn dash_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    chomp(w, id);
    let target = f64::from(fish_data(w, id).map_or(0, |f| f.target));
    let vx = data(w, id).map_or(0.0, |d| d.vx);
    let ex = exact_x(w, id);
    if (vx > 0.0 && ex >= target) || (vx < 0.0 && ex <= target) {
        arm(w, id);
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// The flop

/// The flop on dry ground (see the module documentation).
pub fn enter_flop(w: &mut World, id: ObjectId) {
    obj_mut(w, id).physics_type = ptype::FLY;
    set_state(w, id, EnemyState::OutOfWater);
    set_anim(w, id, FLOP_ANIM);
    let past = obj(w, id).x >= home(w, id).1;
    if let Some(d) = data_mut(w, id) {
        d.vx = f64::from(if past { -FLOP_SPEED } else { FLOP_SPEED });
    }
    let now = w.now_ms;
    if let Some(f) = fish_data_mut(w, id) {
        f.flop = Timer::new(now, u64::from(FLOP_MS));
    }
    play(w, FLOP_SOUND);
}

/// OutOfWater: every 250 ms a flop that faces it by the shared toggle and
/// flips it; once below its line within its home, the splash back in and
/// it swims again.
pub fn flop_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    if fish_data(w, id).is_some_and(|f| f.flop.done(now)) {
        let toggle = w.fish_flop();
        set_facing(w, id, toggle == 0);
        w.set_fish_flop(1 - toggle);
        if let Some(f) = fish_data_mut(w, id) {
            f.flop = Timer::new(now, u64::from(FLOP_MS));
        }
        play(w, FLOP_SOUND);
    }
    if back_in(w, id) {
        play(w, IN_SOUND);
        go_idle(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`: unless it is dead, a pistol shot is used up and
/// the fish dies at once. Draws: the shared death's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if super::state(w, id) == Some(EnemyState::Dead) {
        return;
    }
    if w.objects.get(attacker).map(|a| a.type_flags) == Some(types::PISTOL_SHOT) {
        w.destroy(attacker);
    }
    die(w, id, DeathKind::Hit, Some(attacker));
}

#[cfg(test)]
mod tests {
    use super::super::mercat;
    use super::*;

    #[test]
    fn its_states_open_the_upper_half_of_the_eleventh_levels_block() {
        for (i, s) in STATES.into_iter().enumerate() {
            assert_eq!(s as i32, 2270 + i as i32, "{}", s.name());
            assert!(mercat::STATE_BLOCK.contains(&(s as i32)));
            assert!(handler(s).is_some(), "{}", s.name());
        }
        // The dash keeps the crabs' number.
        assert_eq!(DASH as i32, 2195);
        assert!(handler(DASH).is_some(), "the dash");
        assert!(!CLASS.human, "a plain enemy");
        assert!(CLASS.carry.is_none(), "never lifted");
    }

    #[test]
    fn its_moves_at_the_eighteen_ms_step() {
        // A dash covers about 98 px: 31 ticks at 3.24 px a tick.
        assert_eq!(DASH_SPEED * 18 * DASH_TICKS as i32 / 1000, 100);
        // The second range box is empty.
        assert_eq!(RANGES[1], (0, 0));
        // A flop every 250 ms: every 14th tick.
        assert_eq!(FLOP_MS.div_ceil(18), 14);
        // The leap aims half the third box's height above the line.
        assert_eq!(RANGES[3].1 / 2, LEAP.1);
    }

    #[test]
    fn its_leap_peaks_155_px_up_and_lands_256_px_on() {
        let (vx, vy) = human::launch(150.0, -150.0, LEAP_GRAVITY);
        assert!((vx - 325.7).abs() < 0.1, "{vx}");
        assert!((vy + 786.3).abs() < 0.1, "{vy}");
        let apex = vy * vy / (2.0 * LEAP_GRAVITY);
        assert!((apex - 154.5).abs() < 0.1, "{apex}");
        let t = -2.0 * vy / LEAP_GRAVITY;
        assert!((vx * t - 256.0).abs() < 1.0, "{}", vx * t);
    }

    #[test]
    fn its_art_is_its_own() {
        for key in [
            SWIM_ANIM,
            STRIKE_ANIM,
            FLOP_ANIM,
            DEATH_ANIM,
            CHOMP_SOUND,
            OUT_SOUND,
            IN_SOUND,
            FLOP_SOUND,
            DEATH_SOUND,
        ] {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
