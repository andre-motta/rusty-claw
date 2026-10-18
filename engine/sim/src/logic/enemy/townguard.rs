//! The town guards of the fifth and sixth levels: `TownGuard1`, a guard
//! with a sword and a truncheon, and `TownGuard2`, a heavier one. They are
//! one family with two classes ([`guard1`] and [`guard2`]): the same
//! states, pre-tick and hit rules over two image sets and two sets of
//! boxes and voices.
//!
//! Like the officer they walk up to Claw and strike, high at a standing
//! Claw and low at a crouching one, but they add three things:
//!
//! - they read Claw's sword: while Claw swings at a guard that faces him,
//!   the guard may shout and hop back 100 px from where it stands (a
//!   chance that starts at 90 for `TownGuard2`, 0 for `TownGuard1` on the
//!   fifth level, and falls by 30 with every hop);
//! - `TownGuard2` parries one in four of Claw's sword blows on the fifth
//!   level, in a guard pose ([`EnemyState::Block`]) during which nothing
//!   hurts it;
//! - their idle routine never looks at Claw: two idle poses
//!   ([`EnemyState::Idle`] then [`EnemyState::Idle2`]) with a random wait
//!   between them, then pacing; only the sword reaction, a hit, a lift, a
//!   jump switch or Claw leaving range cut it short.
//!
//! Both can be lifted and thrown, drop their record's loot and talk a
//! lot. A jump switch they walk over launches them through the air
//! ([`crate::logic::jump_switch`]).
//!
//! # The rules
//!
//! - Built: 9 health on the fifth level (10 on others), 10 damage, smarts
//!   99, liftable, not going by sight; the range boxes (500, 300, 200,
//!   110 and 100 px across, all 40 px up or down); then dropped to the
//!   floor and idle.
//! - Pre-tick, after the shared human steps, every tick and in every
//!   state: the sword reaction, then the strike choice. The sword
//!   reaction runs while Claw swings his sword (standing, crouching or in
//!   the air) facing the guard, with Claw within 110 px (modes 3 and 4)
//!   and the reaction's 500 ms cooldown over: a draw below the hop chance
//!   makes the guard shout one of its two lines, schedules the hop back
//!   for the post-tick when the guard is free, lowers the chance by 30
//!   (not below 0) and restarts the cooldown. The strike choice: the low
//!   strike while Claw crouches, the high one otherwise; a strike already
//!   running keeps its animation but strikes with the box chosen on its
//!   strike tick.
//! - Going idle: unless the range mode is 6, a fifth of the time the go
//!   idle line; then the shared human go idle with the first idle pose.
//! - Idle: the first pose plays to its end (82 ticks for `TownGuard1`, 85
//!   for `TownGuard2`); then the second pose waits 0 to 2000 ms on its
//!   first picture, plays, and the guard starts pacing.
//! - Home: the alert once, the first time it sees Claw (70% of the time
//!   one of its two lines); it faces Claw and draws `r` of 0..121; within
//!   110 px (mode 3) it strikes when `r` is below its smarts (picking one
//!   of its two strike sounds), else steps in when below smarts + 20;
//!   within 100 px (mode 4) it backs off or strikes, even odds; out of
//!   range (modes 5, 6) its hop chance goes back to 90 and it starts
//!   pacing. Still at Home, the shared human Home decides (running or
//!   walking at Claw further away, a strike or a pause up close).
//! - Strike: the shared strike, its event 20 ticks in (high) or 21 (low),
//!   Home on 36 or 35; the strike sound picked at Home plays on the event
//!   tick.
//! - Block (the parry): 35 ticks of the low strike's animation, unhurt,
//!   keeping the speed it had (a guard parrying out of a walk slides on);
//!   then Home, free.
//! - Hits: Claw's blow, unless parried, makes the guard cry out (one of
//!   two cries) and then flinch as any human enemy (19 ticks, knocked back
//!   at 200 px/s). A pistol shot or a stick of dynamite does the same
//!   (and flies on through it). Magic, thrown objects and the level's
//!   shots hit as they hit any human enemy. A jump switch launches it.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the guards decide in
//! `Home`, idle in `Idle` and strike in `Strike` with handlers of their
//! own under those numbers, and inherit the shared human states with their
//! numbers. Their two new names take the first two numbers of the fifth
//! level's block [`STATE_BLOCK`]: [`EnemyState::Idle2`], the second idle
//! pose, and [`EnemyState::Block`], the parry. The seagull's two new
//! names follow ([`super::seagull`]).
//!
//! # Random draws, in order
//!
//! - building: one of 0..100 (the death sound), then going idle's;
//! - going idle: one of 0..100 for the go idle line unless the mode is 6,
//!   then the shared human go idle's one of 0..100;
//! - the pre-tick: the shared roll (frame pulse ticks only); with the
//!   sword reaction's conditions, one of 0..100, then one of 0..2 when it
//!   is below the hop chance;
//! - Idle: one of 0..2001 on the tick the first pose ends;
//! - Home: one of 0..100 and one of 0..2 for the alert (the second below
//!   70); one of 0..121; in mode 3 one of 0..2 (strike) or moving
//!   ahead's; in mode 4 one of 0..100, then moving back's or one of 0..2
//!   (strike); then the shared Home's while still Home;
//! - hit by Claw's sword: `TownGuard2` on the fifth or sixth level one of
//!   0..100 for the parry (a standing or crouching Claw's sword only),
//!   `TownGuard1` on the sixth level one first; parried, one of 0..2 (the
//!   block sound), one of 0..100, and one of 0..2 below 40 (a line);
//!   otherwise one of 0..2 (the cry), then the shared flinch's.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the fifth
//!   level owns 2140 to 2159, with `Idle2` 2140, `Block` 2141, `Withdraw`
//!   2142 and `GenericHit` 2143; the level's objects (planks, jump
//!   switches, statues) take none. Later families with a parry of their
//!   own reuse `Block` with their own handlers.
//! - The "on screen" test of the alert under a widened view follows the
//!   rule the soldier's sight test uses.
//! - A jump switch launches a guard whenever its box reaches a guard,
//!   free or busy, that passes the shared hit gate (Claw's attack serial,
//!   then the hit cooldown).
//! - The sixth level's differences (the first guard's parry and hop
//!   chances, guards placed to stand still, that level's timings) are
//!   kept where the code branches on the level number, and are confirmed
//!   with that level.
//!
//! Their data ([`TownGuardData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]. [`super::build`] gives a new enemy plain enemy
//! data; a guard then wraps it in its own.

use std::ops::RangeInclusive;

use super::human::{self, HumanAnims, HumanCall, HumanData, HumanSounds};
use super::lift::Carry;
use super::states::{enter_hit, enter_strike, go_home, human_home_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit,
    attacker, build, data, data_mut, die, face_claw, hit_burst, human_fields, nothing, obj,
    obj_mut, play, rand, say, sees_claw, set_anim, set_state, take_damage,
};
use crate::logic::{ObjectData, World, jump_switch};
use crate::object::{ObjectId, Rect, flags};
use crate::physics::ptype;
use crate::player::attack::kind as attack_kind;

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the fifth level's families. The
/// guards take the first two ([`EnemyState::Idle2`],
/// [`EnemyState::Block`]) and the seagull the next two
/// ([`EnemyState::Withdraw`], [`EnemyState::GenericHit`]); the rest are
/// free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2140..=2159;

/// Health on the fifth level, and on any other, before the difficulty
/// factor (the record's health is not used).
pub const HEALTH: (i32, i32) = (9, 10);
/// Damage of a strike or of touching a guard, before the difficulty
/// factor.
pub const DAMAGE: i32 = 10;
/// Smarts base.
pub const SMARTS: i32 = 50;
/// Hit box, both guards.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// The contact box outside strikes: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box while Claw carries a guard.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The five range boxes (half width, half height), outermost first.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (110, 40), (100, 40)];
/// The chance in 100 of hopping back from Claw's sword when a guard is
/// built: the first guard's on the fifth level (90 on the sixth), and the
/// second guard's.
pub const JUMP_BACK_CHANCE: (i32, i32) = (0, 90);
/// The hop chance once a guard has been Home with Claw out of range.
pub const JUMP_BACK_CHANCE_RESET: i32 = 90;
/// What each hop takes off the hop chance.
pub const JUMP_BACK_CHANCE_STEP: i32 = 30;
/// Time after a passed sword reaction before the next roll, ms.
pub const REACTION_COOLDOWN_MS: u64 = 500;
/// The high strike, at a standing Claw: its animation (both guards play
/// the first guard's).
pub const HIGH_STRIKE_ANIM: &str = "LEVEL_TOWNGUARD1_STRIKE2";
/// The low strike, at a crouching Claw, which is also the parry pose.
pub const LOW_STRIKE_ANIM: &str = "LEVEL_TOWNGUARD1_STRIKE1";
/// The parry pose.
pub const BLOCK_ANIM: &str = LOW_STRIKE_ANIM;
/// The strike sounds, one picked at Home before each strike it decides.
pub const STRIKE_SOUNDS: [&str; 2] = ["LEVEL_TOWNGUARD_OFCDKSTB", "LEVEL_TOWNGUARD_00390016"];
/// The parry sounds, one picked at each parry.
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_TOWNGUARD_BLOCK1", "LEVEL_TOWNGUARD_BLOCK2"];
/// The death sounds, one picked when a guard is built.
pub const DEATH_SOUNDS: [&str; 2] = ["LEVEL_TOWNGUARD_00350007", "LEVEL_TOWNGUARD_00350008"];
/// The sound of the hop back.
pub const JUMP_SOUND: &str = "LEVEL_TOWNGUARD_LEJUMP2";
/// Chance in 100 that going idle says the go idle line.
pub const GO_IDLE_LINE_CHANCE: i32 = 20;
/// Chance in 100 that the first sight of Claw says an alert line.
pub const ALERT_CHANCE: i32 = 70;
/// Chance in 100 that a parry says one of the hop lines.
pub const PARRY_LINE_CHANCE: i32 = 40;
/// Home: draws of 0..121; up close, below smarts it strikes, below smarts
/// + 20 it steps in.
pub const HOME_SPREAD: u32 = 121;
/// See [`HOME_SPREAD`].
pub const MOVE_AHEAD_BAND: i32 = 20;
/// The longest wait between the two idle poses, ms (0 to this).
pub const IDLE_WAIT_MAX_MS: u32 = 2000;
/// The parry chance in 100 of the second guard on the fifth level, and on
/// the sixth.
pub const PARRY_CHANCE: (i32, i32) = (25, 50);
/// The parry chance in 100 of the first guard on the sixth level.
pub const GUARD1_PARRY_CHANCE: i32 = 25;

/// What differs between the two guards.
#[derive(Debug, Clone, Copy)]
pub struct Guard {
    /// Logic name.
    pub name: &'static str,
    /// Image set.
    pub image_set: &'static str,
    /// The high strike's box, facing left.
    pub high_strike: Rect,
    /// The low strike's box, facing left.
    pub low_strike: Rect,
    /// Movement box.
    pub move_rect: Rect,
    /// Its own animations (the shared states'), by name.
    pub anims: ClassAnims,
    /// The first idle pose.
    pub idle1: &'static str,
    /// The second idle pose.
    pub idle2: &'static str,
    /// The body flying off after death.
    pub death_anim: &'static str,
    /// Said a third of the time when lifted (on screen).
    pub lifted_line: &'static str,
    /// Said a fifth of the time on going idle.
    pub go_idle_line: &'static str,
    /// The two alert lines.
    pub alert_lines: [&'static str; 2],
    /// The two lines of the hop back (and of some parries).
    pub jump_back_lines: [&'static str; 2],
    /// The two cries when hit.
    pub cries: [&'static str; 2],
}

/// `TownGuard1`.
pub const GUARD1: Guard = Guard {
    name: "TownGuard1",
    image_set: "LEVEL_TOWNGUARD1",
    high_strike: Rect::new(-88, -35, -20, 7),
    low_strike: Rect::new(-90, 4, -22, 35),
    move_rect: Rect::new(-20, -60, 20, 51),
    anims: ClassAnims {
        advance: "LEVEL_TOWNGUARD1_ADVANCE",
        fast_advance: "LEVEL_TOWNGUARD1_FASTADVANCE",
        pace: "LEVEL_TOWNGUARD1_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_TOWNGUARD1_HITLOW",
        hit_high: "LEVEL_TOWNGUARD1_HITHIGH",
        home: "LEVEL_TOWNGUARD1_HOME",
        jump: "LEVEL_TOWNGUARD1_JUMP",
        fall: "LEVEL_TOWNGUARD1_KILLFALL",
        lifted: "LEVEL_TOWNGUARD1_LIFTED1",
        lifted2: "LEVEL_TOWNGUARD1_LIFTED2",
        bounce: "LEVEL_TOWNGUARD1_THROWN",
    },
    idle1: "LEVEL_TOWNGUARD1_IDLE1",
    idle2: "LEVEL_TOWNGUARD1_IDLE2",
    death_anim: "LEVEL_TOWNGUARD1_KILLFALL",
    lifted_line: "LEVEL_TOWNGUARD_00390011",
    go_idle_line: "LEVEL_TOWNGUARD_00390035",
    alert_lines: ["LEVEL_TOWNGUARD_00390002", "LEVEL_TOWNGUARD_00390028"],
    jump_back_lines: ["LEVEL_TOWNGUARD_00390034", "LEVEL_TOWNGUARD_00390015"],
    cries: ["LEVEL_TOWNGUARD_OFCHITHI", "LEVEL_TOWNGUARD_00350009"],
};

/// `TownGuard2`. The fifth level ships no first lift animation for it:
/// lifted, it shows the first picture of its own set until Claw raises it
/// over his head (see [`super::lift`]).
pub const GUARD2: Guard = Guard {
    name: "TownGuard2",
    image_set: "LEVEL_TOWNGUARD2",
    high_strike: Rect::new(-88, -14, -21, 27),
    low_strike: Rect::new(-102, 7, -33, 46),
    move_rect: Rect::new(-20, -40, 20, 71),
    anims: ClassAnims {
        advance: "LEVEL_TOWNGUARD2_ADVANCE",
        fast_advance: "LEVEL_TOWNGUARD2_FASTADVANCE",
        pace: "LEVEL_TOWNGUARD2_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_TOWNGUARD2_HITLOW",
        hit_high: "LEVEL_TOWNGUARD2_HITHIGH",
        home: "LEVEL_TOWNGUARD2_HOME",
        jump: "LEVEL_TOWNGUARD2_JUMP",
        fall: "LEVEL_TOWNGUARD2_KILLFALL",
        lifted: "LEVEL_TOWNGUARD2_LIFTED1",
        lifted2: "LEVEL_TOWNGUARD2_LIFTED2",
        bounce: "LEVEL_TOWNGUARD2_THROWN",
    },
    idle1: "LEVEL_TOWNGUARD2_IDLE1",
    idle2: "LEVEL_TOWNGUARD2_IDLE2",
    death_anim: "LEVEL_TOWNGUARD2_KILLFALL",
    lifted_line: "LEVEL_TOWNGUARD_00370027",
    go_idle_line: "LEVEL_TOWNGUARD_00370009",
    alert_lines: ["LEVEL_TOWNGUARD_00370019", "LEVEL_TOWNGUARD_00370021"],
    jump_back_lines: ["LEVEL_TOWNGUARD_00370013", "LEVEL_TOWNGUARD_00370014"],
    cries: ["LEVEL_TOWNGUARD_OFCHITHI", "LEVEL_TOWNGUARD_00370012"],
};

/// The hit, death, lift and throw sounds of the shared states.
const SOUNDS: ClassSounds = ClassSounds {
    // A strike plays the sound its Home picked ([`HumanSounds::strike`]),
    // none before the first pick.
    strike: "",
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries a guard: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The guard running the logic `name`.
pub fn guard(name: &str) -> Option<&'static Guard> {
    [&GUARD1, &GUARD2].into_iter().find(|g| g.name == name)
}

/// Health of a guard built on level `level`, before the difficulty
/// factor.
pub fn health(level: u32) -> i32 {
    if level == 5 { HEALTH.0 } else { HEALTH.1 }
}

/// The hop chance a guard starts with on level `level`.
pub fn jump_back_chance(g: &Guard, level: u32) -> i32 {
    if g.name == GUARD2.name || level == 6 {
        JUMP_BACK_CHANCE.1
    } else {
        JUMP_BACK_CHANCE.0
    }
}

/// True for the attacks of Claw a guard reacts to and parries: the sword
/// swipes, standing, crouching and in the air (and kind 2, a swipe the
/// game's attack table keeps besides them).
pub fn is_sword_swipe(kind: i32) -> bool {
    matches!(
        kind,
        attack_kind::SWIPE | 2 | attack_kind::DUCK_SWIPE | attack_kind::JUMP_SWIPE
    )
}

/// A town guard's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TownGuardData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The second guard (`TownGuard2`).
    pub second: bool,
    /// Chance in 100 of hopping back from Claw's sword.
    pub jump_back_chance: i32,
    /// Cooldown of the sword reaction.
    pub reaction: Timer,
    /// The wait on the second idle pose's first picture.
    pub idle_wait: Timer,
}

impl TownGuardData {
    /// What differs for this guard's class.
    pub fn guard(&self) -> &'static Guard {
        if self.second { &GUARD2 } else { &GUARD1 }
    }
}

/// The guard data of `id`.
pub fn guard_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a TownGuardData> {
    match &w.objects.get(id)?.data {
        ObjectData::TownGuard(d) => Some(d),
        _ => None,
    }
}

fn guard_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut TownGuardData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::TownGuard(d) => Some(d),
        _ => None,
    }
}

fn which(w: &World, id: ObjectId) -> &'static Guard {
    guard_data(w, id).map_or(&GUARD1, TownGuardData::guard)
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Idle2 => Some(idle2_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Block => Some(block_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call (which the moves back and
/// ahead need), with the class of the guard.
pub fn tick(w: &mut World, id: ObjectId) {
    human::tick(w, id);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

fn construct_guard(w: &mut World, id: ObjectId, g: &'static Guard) {
    let death_sound = DEATH_SOUNDS[usize::from(rand(w, 100) >= 50)];
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: g.high_strike,
        strike_anim: HIGH_STRIKE_ANIM,
        death_anim: g.death_anim,
        death_sound,
        lifted_voice: g.lifted_line,
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                jump_back: g.anims.jump,
                idle: g.idle1,
                ..HumanAnims::default()
            },
            sounds: HumanSounds {
                jump: JUMP_SOUND,
                ..HumanSounds::default()
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: g.image_set,
        move_rect: g.move_rect,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: health(w.level_number()),
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let chance = jump_back_chance(g, w.level_number());
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::TownGuard(TownGuardData {
            enemy,
            second: g.name == GUARD2.name,
            jump_back_chance: chance,
            ..TownGuardData::default()
        });
    }
    go_idle(w, id);
}

/// Builds a `TownGuard1` (see the module documentation). Draws: one of
/// 0..100 (its death sound), then going idle's.
pub fn construct1(w: &mut World, id: ObjectId) {
    construct_guard(w, id, &GUARD1);
}

/// Builds a `TownGuard2` (see the module documentation). Draws: one of
/// 0..100 (its death sound), then going idle's.
pub fn construct2(w: &mut World, id: ObjectId) {
    construct_guard(w, id, &GUARD2);
}

// ----------------------------------------------------------------------
// Pre-tick

/// The guard's own pre-tick steps, after the shared ones: the sword
/// reaction, then the strike choice (see the module documentation).
/// Draws: with the reaction's conditions, one of 0..100, then one of 0..2
/// when it is below the hop chance.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    sword_reaction(w, id);
    let ducking = w
        .claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK);
    let g = which(w, id);
    if let Some(d) = data_mut(w, id) {
        if ducking {
            d.strike_rect = g.low_strike;
            d.strike_anim = LOW_STRIKE_ANIM;
        } else {
            d.strike_rect = g.high_strike;
            d.strike_anim = HIGH_STRIKE_ANIM;
        }
    }
}

/// The sword reaction: Claw swings his sword facing the guard within
/// 110 px with the reaction's cooldown over; a draw below the hop chance
/// shouts a line, schedules the hop back when the guard is free, lowers
/// the chance and restarts the cooldown.
fn sword_reaction(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let swinging = w.claw.attacking && is_sword_swipe(w.claw.attack.kind);
    let claw_facing_right = w.claw.facing_right;
    let Some((right, mode, busy)) = data(w, id).map(|d| (d.claw_right, d.mode, d.busy)) else {
        return;
    };
    let Some((chance, cooldown)) = guard_data(w, id).map(|g| (g.jump_back_chance, g.reaction))
    else {
        return;
    };
    let faces_guard = claw_facing_right != right;
    if !(swinging && faces_guard && matches!(mode, 3 | 4) && cooldown.done(now)) {
        return;
    }
    if rand(w, 100) >= chance {
        return;
    }
    let g = which(w, id);
    let line = g.jump_back_lines[rand(w, 2) as usize];
    say(w, id, line);
    if !busy && let Some(d) = data_mut(w, id) {
        d.deferred = Some(Deferred::Human(HumanCall::JumpBack));
    }
    if let Some(gd) = guard_data_mut(w, id) {
        gd.jump_back_chance = (gd.jump_back_chance - JUMP_BACK_CHANCE_STEP).max(0);
        gd.reaction = Timer::new(now, REACTION_COOLDOWN_MS);
    }
}

// ----------------------------------------------------------------------
// Idle

/// Goes idle: unless the range mode is 6, a fifth of the time it says its
/// go idle line; then the shared human go idle (it faces Claw, stops,
/// takes its first idle pose, not advanced). Draws: one of 0..100 unless
/// the mode is 6, then the human go idle's one of 0..100.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| d.mode != 6) && rand(w, 100) < GO_IDLE_LINE_CHANCE {
        let line = which(w, id).go_idle_line;
        say(w, id, line);
    }
    human::go_idle(w, id);
}

/// Idle: the first pose plays; when it ends, the second pose (not
/// advanced) and a wait of 0 to 2000 ms. It never looks at Claw. Draws:
/// one of 0..2001 on the tick the first pose ends.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let g = which(w, id);
    set_anim(w, id, g.idle2);
    set_state(w, id, EnemyState::Idle2);
    let wait = rand(w, IDLE_WAIT_MAX_MS + 1) as u64;
    let now = w.now_ms;
    if let Some(gd) = guard_data_mut(w, id) {
        gd.idle_wait = Timer::new(now, wait);
    }
}

/// Idle2: on the second pose's first picture until the wait is over; then
/// the pose plays, and when it ends the guard starts pacing (a guard that
/// never walks goes idle again).
pub fn idle2_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !guard_data(w, id).is_some_and(|g| g.idle_wait.done(now)) {
        return;
    }
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        start_pacing(w, id);
    }
}

// ----------------------------------------------------------------------
// Home

/// Home (see the module documentation). Draws: the alert's (once), one of
/// 0..121, the mode's, then the shared Home's while still Home.
pub fn home_tick(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| !d.alerted) && sees_claw(w, id) {
        if rand(w, 100) < ALERT_CHANCE {
            let line = which(w, id).alert_lines[rand(w, 2) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    face_claw(w, id);
    let r = rand(w, HOME_SPREAD);
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    match mode {
        3 if r < smarts => strike(w, id),
        3 if r < smarts + MOVE_AHEAD_BAND => human::enter_move_ahead(w, id),
        4 => {
            if rand(w, 100) < 50 {
                human::enter_move_back(w, id);
            } else {
                strike(w, id);
            }
        }
        5 | 6 => {
            if let Some(gd) = guard_data_mut(w, id) {
                gd.jump_back_chance = JUMP_BACK_CHANCE_RESET;
            }
            start_pacing(w, id);
        }
        _ => {}
    }
    human_home_tick(w, id);
}

/// Picks one of the two strike sounds and strikes. Draws: one of 0..2.
fn strike(w: &mut World, id: ObjectId) {
    let sound = STRIKE_SOUNDS[rand(w, 2) as usize];
    if let Some(d) = data_mut(w, id) {
        d.human.sounds.strike = sound;
    }
    enter_strike(w, id);
}

// ----------------------------------------------------------------------
// The parry

/// Parries: faces Claw, busy and unhurt in the parry pose, keeping its
/// speed; one of the two parry sounds, and two times in five one of its
/// hop lines. Draws: one of 0..2, one of 0..100, then one of 0..2 below
/// 40.
pub fn enter_block(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::Block);
    set_anim(w, id, BLOCK_ANIM);
    obj_mut(w, id).flags |= flags::SAFE;
    let sound = BLOCK_SOUNDS[rand(w, 2) as usize];
    play(w, sound);
    if rand(w, 100) < PARRY_LINE_CHANCE {
        let line = which(w, id).jump_back_lines[rand(w, 2) as usize];
        play(w, line);
    }
}

/// Block: the parry pose plays; when it ends, hits are allowed again and
/// the guard goes Home, free.
pub fn block_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        obj_mut(w, id).flags &= !flags::SAFE;
        go_home(w, id);
        if let Some(d) = data_mut(w, id) {
            d.busy = false;
        }
    }
}

/// Whether Claw's blow that just reached the guard is parried. Draws: one
/// of 0..100 for `TownGuard2` on the fifth or sixth level when Claw swings
/// his sword standing or crouching; for `TownGuard1` on the sixth level,
/// one first.
fn parries(w: &mut World, id: ObjectId) -> bool {
    let level = w.level_number();
    let kind = w.claw.attack.kind;
    let posture = w.claw_object().map_or(0, |c| c.physics_type);
    if which(w, id).name == GUARD2.name {
        if !is_sword_swipe(kind) || !matches!(posture, ptype::STAND | ptype::DUCK) {
            return false;
        }
        match level {
            5 => rand(w, 100) < PARRY_CHANCE.0,
            6 => rand(w, 100) < PARRY_CHANCE.1,
            _ => false,
        }
    } else if level == 6 {
        rand(w, 100) < GUARD1_PARRY_CHANCE && is_sword_swipe(kind) && posture == ptype::STAND
    } else {
        false
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: the parry when it comes; otherwise one of its two
///   cries and the human blow rule (during a flinch it only goes back
///   Home; otherwise the damage, a hit burst, a 300 ms cooldown and the
///   flinch, high after an uppercut);
/// - a pistol shot or a stick of dynamite: the cry and the same rule (a
///   low flinch); the shot flies on;
/// - magic or a thrown object: damage, burst, low flinch;
/// - a shot of the level or an explosion: damage, burst, 2000 ms
///   cooldown, low flinch;
/// - a jump switch: launched towards the switch's target
///   ([`jump_switch::launch`]).
///
/// At 0 health or below it dies.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let blow = w.objects.get(attacker).map_or(0, |a| a.special_flags);
    if let Some(d) = data_mut(w, id) {
        if blow & FIRE_BLOW != 0 {
            d.death_anim = FIRE_DEATH_ANIM;
        } else if blow & ICE_BLOW != 0 {
            d.death_anim = ICE_DEATH_ANIM;
        }
    }
    if !accept_hit(w, id) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    let now = w.now_ms;
    match kind {
        attacker::CLAW => {
            if parries(w, id) {
                enter_block(w, id);
            } else {
                let high = w.claw.attack.uppercut;
                blow_rule(w, id, attacker, high);
            }
        }
        attacker::PISTOL_SHOT => blow_rule(w, id, attacker, false),
        attacker::MAGIC | attacker::THROWN => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            enter_hit(w, id, false);
        }
        attacker::LEVEL_SHOT => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, 2000);
            }
            enter_hit(w, id, false);
        }
        attacker::SPRING => jump_switch::launch(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// A cry, then the human blow rule: during a flinch it only goes back
/// Home; otherwise the damage, a hit burst, a 300 ms cooldown and the
/// flinch. Draws: one of 0..2 (the cry), then the burst's and the
/// flinch's.
fn blow_rule(w: &mut World, id: ObjectId, attacker: ObjectId, high: bool) {
    let cry = which(w, id).cries[rand(w, 2) as usize];
    play(w, cry);
    if matches!(
        super::state(w, id),
        Some(EnemyState::HitLow | EnemyState::HitHigh)
    ) {
        go_home(w, id);
        return;
    }
    let now = w.now_ms;
    take_damage(w, id, attacker);
    hit_burst(w, id, attacker);
    if let Some(d) = data_mut(w, id) {
        d.cooldown = Timer::new(now, 300);
    }
    enter_hit(w, id, high);
}

/// The first town guard's class.
pub mod guard1 {
    use super::{CARRY, Class, GUARD1, SOUNDS, construct1, go_idle, handler, nothing, pre_tick};

    pub use super::{BUILT, on_attack, on_hit, tick};

    /// Logic names this class runs.
    pub const NAMES: &[&str] = &[GUARD1.name];

    /// Image sets this class may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[GUARD1.image_set];

    /// Prefixes of the animations this class may play.
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_TOWNGUARD1_"];

    /// The `TownGuard1` class.
    pub static CLASS: Class = Class {
        name: GUARD1.name,
        human: true,
        construct: construct1,
        pre_tick,
        handler,
        go_home: super::go_home,
        go_idle,
        carry: Some(CARRY),
        pick_lift_voice: nothing,
        anims: GUARD1.anims,
        sounds: SOUNDS,
    };
}

/// The second town guard's class. It plays the first guard's strikes and
/// parry pose with its own image set.
pub mod guard2 {
    use super::{CARRY, Class, GUARD2, SOUNDS, construct2, go_idle, handler, nothing, pre_tick};

    pub use super::{BUILT, on_attack, on_hit, tick};

    /// Logic names this class runs.
    pub const NAMES: &[&str] = &[GUARD2.name];

    /// Image sets this class may switch an object to.
    pub const IMAGE_SETS: &[&str] = &[GUARD2.image_set];

    /// Prefixes of the animations this class may play: its own and the
    /// first guard's (its strikes and parry pose).
    pub const ANIM_PREFIXES: &[&str] = &["LEVEL_TOWNGUARD2_", "LEVEL_TOWNGUARD1_"];

    /// The `TownGuard2` class.
    pub static CLASS: Class = Class {
        name: GUARD2.name,
        human: true,
        construct: construct2,
        pre_tick,
        handler,
        go_home: super::go_home,
        go_idle,
        carry: Some(CARRY),
        pick_lift_voice: nothing,
        anims: GUARD2.anims,
        sounds: SOUNDS,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn their_new_states_open_the_fifth_levels_block() {
        assert_eq!(EnemyState::Idle2 as i32, *STATE_BLOCK.start());
        assert_eq!(EnemyState::Block as i32, STATE_BLOCK.start() + 1);
        for s in [EnemyState::Idle2, EnemyState::Block] {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
        }
        for s in [
            EnemyState::Idle,
            EnemyState::Idle2,
            EnemyState::Home,
            EnemyState::Block,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(handler(EnemyState::Strike).is_none(), "the shared strike");
    }

    #[test]
    fn each_class_names_its_own_guard() {
        assert_eq!(guard("TownGuard1").map(|g| g.name), Some(GUARD1.name));
        assert_eq!(guard("TownGuard2").map(|g| g.name), Some(GUARD2.name));
        assert!(guard("Officer").is_none());
        assert_eq!(
            (guard1::CLASS.name, guard2::CLASS.name),
            (GUARD1.name, GUARD2.name)
        );
        assert!(guard1::CLASS.human && guard2::CLASS.human);
        for (names, sets, prefixes, g) in [
            (
                guard1::NAMES,
                guard1::IMAGE_SETS,
                guard1::ANIM_PREFIXES,
                GUARD1,
            ),
            (
                guard2::NAMES,
                guard2::IMAGE_SETS,
                guard2::ANIM_PREFIXES,
                GUARD2,
            ),
        ] {
            assert_eq!(names, [g.name]);
            assert_eq!(sets, [g.image_set]);
            // Both play the first guard's strikes and parry pose.
            for anim in [HIGH_STRIKE_ANIM, LOW_STRIKE_ANIM, BLOCK_ANIM] {
                assert!(prefixes.iter().any(|p| anim.starts_with(p)), "{anim}");
            }
            let a = g.anims;
            for anim in [
                a.advance,
                a.fast_advance,
                a.pace,
                a.hit_low,
                a.hit_high,
                a.home,
                a.jump,
                a.fall,
                a.lifted,
                a.lifted2,
                a.bounce,
                g.idle1,
                g.idle2,
                g.death_anim,
            ] {
                assert!(anim.starts_with(prefixes[0]), "{}: {anim}", g.name);
            }
        }
    }

    #[test]
    fn strikes_reach_ahead_of_a_guard_facing_left() {
        for g in [GUARD1, GUARD2] {
            for r in [g.high_strike, g.low_strike] {
                assert!(r.right < 0 && r.left < r.right, "{}: {r:?}", g.name);
                assert!(r.top < r.bottom, "{}: {r:?}", g.name);
            }
            // The low strike reaches lower than the high one.
            assert!(g.low_strike.bottom > g.high_strike.bottom, "{}", g.name);
        }
    }

    #[test]
    fn health_is_nine_on_the_fifth_level_only() {
        assert_eq!(health(5), 9);
        assert_eq!(health(6), 10);
        assert_eq!(health(1), 10);
    }

    #[test]
    fn the_hop_chance_starts_at_90_but_for_the_first_guard_on_the_fifth_level() {
        assert_eq!(jump_back_chance(&GUARD1, 5), 0);
        assert_eq!(jump_back_chance(&GUARD1, 6), 90);
        assert_eq!(jump_back_chance(&GUARD2, 5), 90);
        assert_eq!(jump_back_chance(&GUARD2, 6), 90);
        // Three hops from 90 use it up.
        let mut c = 90;
        let mut hops = 0;
        while c > 0 {
            c = (c - JUMP_BACK_CHANCE_STEP).max(0);
            hops += 1;
        }
        assert_eq!(hops, 3);
    }

    #[test]
    fn only_the_sword_swipes_count() {
        for k in [
            attack_kind::SWIPE,
            attack_kind::DUCK_SWIPE,
            attack_kind::JUMP_SWIPE,
        ] {
            assert!(is_sword_swipe(k), "{k}");
        }
        for k in [
            attack_kind::NONE,
            attack_kind::PUNCH,
            attack_kind::UPPERCUT,
            attack_kind::KICK,
            attack_kind::PISTOL,
            attack_kind::MAGIC,
            attack_kind::DYNAMITE,
        ] {
            assert!(!is_sword_swipe(k), "{k}");
        }
    }
}
