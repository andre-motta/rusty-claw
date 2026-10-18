//! The red-tail pirate of the seventh level (and of the eighth and the
//! thirteenth): a pirate with a sword.
//!
//! It is a human enemy built like the town guards: it idles, paces,
//! walks up to Claw at 120 px/s (240 px/s when he is far), steps in or
//! back at 150 px/s and strikes, high at a standing Claw and low at a
//! crouching one, and it reads Claw's attacks:
//!
//! - it decides on its first decision step with Claw in range, so it
//!   engages at once; the high strike ([`HIGH_STRIKE`]) is live on its 7th
//!   tick and over on its 18th, the low one ([`LOW_STRIKE`]) live on its
//!   7th and over on its 14th. Close in it strikes again every 19 ticks
//!   (high) or 15 (low); a strike from further out keeps it from striking
//!   again for [`REST_MS`];
//! - when Claw starts a sword swing or fires his pistol at it from in
//!   front, it may hop back ([`EnemyState::JumpBack`]) on the tick the
//!   attack starts, before the blow lands: a hop of 18 ticks that lands
//!   about 96 px from where it stood. The chance starts at 90 in 100 and
//!   falls by 30 with every hop ([`jump_back_chance`]), so after three
//!   hops it stands its ground;
//! - on the seventh level it never parries: every hit goes through, and a
//!   low hit makes it flinch for 19 ticks.
//!
//! It has [`HEALTH`] health (two sword blows or two pistol shots), its
//! strikes and its touch cost Claw [`DAMAGE`], it can be lifted and
//! thrown and drops its record's loot.
//!
//! # The rules
//!
//! - Built: the health and damage above, smarts 99, liftable, not going by
//!   sight; the range boxes [`RANGES`] (the second and third are equal, so
//!   Claw 100 to 200 px away is range mode 2); its hop chance at 90; then
//!   dropped to the floor and idle (the shared human idle: it goes Home as
//!   soon as Claw is in range).
//! - Pre-tick, after the shared human steps, every tick and in every
//!   state: the reaction to Claw's attacks, then the strike choice. The
//!   reaction runs while Claw attacks (any attack: a swing, a punch, a
//!   shot, magic, dynamite) facing the pirate, with Claw within 100 px
//!   (range modes 3 and 4) and the reaction's 500 ms cooldown over: a draw
//!   of 0..100 below the hop chance schedules the hop back for the
//!   post-tick when the pirate is free (busy, it does not hop, but the
//!   chance is spent all the same), lowers the chance by 30 (not below 0)
//!   and restarts the cooldown. The strike choice: the low strike while
//!   Claw crouches, the high one otherwise; a strike already running keeps
//!   its animation but strikes with the box chosen on its strike tick.
//! - Home: the alert once, the first time it sees Claw (half the time one
//!   of its two lines), or after a throw's landing its taunt; it faces
//!   Claw and draws `r` of 0..121. Claw 200 to 500 px away (mode 0): it
//!   runs at him. 100 to 200 px (modes 1, 2): when its strike rest is
//!   over and `r` is below 60 it swings high at the air in front of it and
//!   rests [`REST_MS`], otherwise it walks at him. 60 to 100 px (mode 3):
//!   it strikes when `r` is below its smarts and its rest is over (resting
//!   [`CLOSE_REST_MS`]), otherwise it steps in. Within 60 px (mode 4):
//!   even odds of backing off, otherwise it strikes when its rest is over
//!   (resting [`CLOSE_REST_MS`]). Out of range (modes 5, 6): its hop chance
//!   goes back to 90 and it starts pacing. It never pauses in front of
//!   Claw: a step in or back that cannot be made leaves it Home for the
//!   next tick.
//! - Strike: a quarter of the time one of its four strike lines, then the
//!   shared strike.
//! - Hits: every new attack of Claw that reaches it draws twice (the
//!   parry roll of the eighth and thirteenth levels, and a hit line that
//!   no level ships), then the shared human hit rule: a low flinch of 19
//!   ticks with only the first hit sound, a high one after an uppercut.
//! - Lifted: always one of its five lifted lines.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the pirate decides in
//! `Home` with a handler of its own, parries in the town guards' `Block`
//! (only on the eighth and thirteenth levels) and inherits every other
//! shared human state with its number. Its one new name,
//! [`EnemyState::GabrielAttack`], the attack of the eighth level's boss
//! fight, takes the fourth number of the seventh level's block
//! ([`STATE_BLOCK`], shared with the bear sailor); the seventh level never
//! enters it.
//!
//! # Random draws, in order
//!
//! - building: the shared human go idle's one of 0..100;
//! - the pre-tick: the shared roll (frame pulse ticks only); with the
//!   reaction's conditions, one of 0..100;
//! - Home: the alert's one of 0..101 (then one of 0..2 below 50), or the
//!   taunt's one of 0..2; one of 0..121; mode 3: the strike's or stepping
//!   in's; mode 4: one of 0..100, then backing off's or the strike's;
//! - the strike: one of 0..101, then one of 0..4 below 25;
//! - hit by a new attack of Claw: one of 0..101 twice, then the shared
//!   human hit rule's;
//! - lifted: one of 0..5, then the shared lift's one of 0..100.
//!
//! # Defaults taken
//!
//! - The eighth and thirteenth levels' differences (the parry of Claw's
//!   sword and the block of his shots, [`block_chances`], and the boss
//!   fight's mode) are kept where the code branches on the level number,
//!   and are confirmed with those levels; the seventh level runs none of
//!   them, and the boss fight's mode is not built.
//! - The "on screen" test of its alert under a widened view follows the
//!   rule the soldier's sight test uses.
//!
//! Its data ([`RedTailPirateData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, HumanAnims, HumanCall, HumanData};
use super::lift::Carry;
use super::states::{self, enter_strike, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, Deferred, EnemyData, EnemyFields, EnemyState,
    HUMAN_HIT_BY, Handler, Timer, attacker, build, class_of, data, data_mut, face_claw,
    human_fields, obj, obj_mut, play, rand, say, sees_claw, set_anim, set_state,
};
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, flags};
use crate::physics::ptype;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["RedTailPirate"];

/// Its picture, set when it is built (the records name it too).
pub const IMAGE_SET: &str = "LEVEL_REDTAILPIRATE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_REDTAILPIRATE_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers of the seventh level's families (see
/// [`super::bear_sailor::STATE_BLOCK`]).
pub const STATE_BLOCK: RangeInclusive<i32> = super::bear_sailor::STATE_BLOCK;

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 10;
/// Damage of a strike and of touching it, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Smarts base (raised to 99).
pub const SMARTS: i32 = 50;
/// Walking speed towards Claw, and pacing speed, px/s.
pub const ADVANCE_SPEED: i32 = 120;
/// Running speed towards a far Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 240;
/// Speed of a step in or back, px/s.
pub const STEP_SPEED: i32 = 150;
/// The high strike's box, at a standing Claw, facing left.
pub const HIGH_STRIKE: Rect = Rect::new(-96, -50, -20, 50);
/// The low strike's box, at a crouching Claw, facing left.
pub const LOW_STRIKE: Rect = Rect::new(-96, 29, 4, 74);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The contact box outside strikes: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first,
/// outside the eighth level's boss fight.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (200, 40), (200, 40), (100, 40), (60, 40)];

/// The high strike.
pub const HIGH_STRIKE_ANIM: &str = "LEVEL_REDTAILPIRATE_STRIKE1";
/// The low strike.
pub const LOW_STRIKE_ANIM: &str = "LEVEL_REDTAILPIRATE_STRIKE2";
/// The hop back.
pub const JUMP_ANIM: &str = "LEVEL_REDTAILPIRATE_JUMP";
/// Standing idle.
pub const IDLE_ANIM: &str = "LEVEL_REDTAILPIRATE_IDLE";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_REDTAILPIRATE_KILLFALL";
/// The parry pose of the eighth and thirteenth levels.
pub const BLOCK_ANIM: &str = "LEVEL_REDTAILPIRATE_BLOCK";
/// The quick guard against a shot, eighth and thirteenth levels.
pub const QUICK_BLOCK_ANIM: &str = "LEVEL_REDTAILPIRATE_QUICKBLOCK";

/// The death sound.
pub const DEATH_SOUND: &str = "LEVEL_REDTAILPIRATE_00490005";
/// The two alert lines.
pub const ALERT_LINES: [&str; 2] = [
    "LEVEL_REDTAILPIRATE_00490001",
    "LEVEL_REDTAILPIRATE_00490019",
];
/// The two taunts after a throw.
pub const TAUNTS: [&str; 2] = [
    "LEVEL_REDTAILPIRATE_00490014",
    "LEVEL_REDTAILPIRATE_00490028",
];
/// The four strike lines.
pub const STRIKE_LINES: [&str; 4] = [
    "LEVEL_REDTAILPIRATE_00490021",
    "LEVEL_REDTAILPIRATE_00490022",
    "LEVEL_REDTAILPIRATE_00490016",
    "LEVEL_REDTAILPIRATE_00490008",
];
/// The five lifted lines.
pub const LIFTED_LINES: [&str; 5] = [
    "LEVEL_REDTAILPIRATE_00490010",
    "LEVEL_REDTAILPIRATE_00490011",
    "LEVEL_REDTAILPIRATE_00490012",
    "LEVEL_REDTAILPIRATE_00490013",
    "LEVEL_REDTAILPIRATE_00490018",
];
/// The parry sounds (eighth and thirteenth levels).
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_REDTAILPIRATE_BLOCK1", "LEVEL_REDTAILPIRATE_BLOCK2"];

/// Time a strike made from further out keeps it from striking again, ms.
pub const REST_MS: u32 = 2000;
/// Time a strike made close in keeps it from striking again, ms.
pub const CLOSE_REST_MS: u32 = 250;
/// Time after a passed reaction roll before the next one, ms.
pub const REACTION_COOLDOWN_MS: u64 = 500;
/// The hop-back chance in 100 of a new pirate.
pub const FIRST_JUMP_BACK_CHANCE: i32 = 90;
/// How much each hop lowers the hop-back chance.
pub const JUMP_BACK_CHANCE_STEP: i32 = 30;
/// Ticks of a hop back at the 18 ms step.
pub const HOP_TICKS: u32 = 18;
/// Ticks of the low flinch at the 18 ms step.
pub const HIT_LOW_TICKS: u32 = 19;
/// Chance in 100 that the first sight of Claw says an alert line.
pub const ALERT_CHANCE: i32 = 50;
/// Chance in 100 that a strike says a strike line.
pub const STRIKE_LINE_CHANCE: i32 = 25;
/// Home draws one of 0..121.
pub const HOME_SPREAD: u32 = 121;
/// Up to 200 px away it swings at the air when the draw is below this.
pub const FAR_STRIKE_BELOW: i32 = 60;
/// Close in, a draw below its smarts plus this steps in when it does not
/// strike (always, since the draw is at most 120).
pub const MOVE_AHEAD_BAND: i32 = 30;

/// The hop-back chance in 100 after `hops` hops: 90, 60, 30, then 0.
pub fn jump_back_chance(hops: u32) -> i32 {
    let fall = JUMP_BACK_CHANCE_STEP.saturating_mul(hops.min(10) as i32);
    (FIRST_JUMP_BACK_CHANCE - fall).max(0)
}

/// The chances in 100 that a pirate of level `level` parries one of
/// Claw's sword blows and blocks one of his shots; `None` on the levels
/// where it does neither (the seventh among them).
pub fn block_chances(level: u32) -> Option<(i32, i32)> {
    match level {
        8 => Some((25, 25)),
        13 => Some((50, 75)),
        _ => None,
    }
}

/// The strike box for a Claw who crouches (`crouching`) or not, facing
/// left.
pub fn strike_box(crouching: bool) -> Rect {
    if crouching { LOW_STRIKE } else { HIGH_STRIKE }
}

/// The animations of the shared states.
const ANIMS: ClassAnims = ClassAnims {
    advance: "LEVEL_REDTAILPIRATE_ADVANCE",
    fast_advance: "LEVEL_REDTAILPIRATE_FASTADVANCE",
    pace: "LEVEL_REDTAILPIRATE_ADVANCE",
    walk: "",
    hit_low: "LEVEL_REDTAILPIRATE_HITLOW",
    hit_high: "LEVEL_REDTAILPIRATE_HITHIGH",
    home: "LEVEL_REDTAILPIRATE_HOME",
    jump: JUMP_ANIM,
    fall: DEATH_ANIM,
    lifted: "LEVEL_REDTAILPIRATE_LIFTED1",
    lifted2: "LEVEL_REDTAILPIRATE_LIFTED2",
    bounce: "LEVEL_REDTAILPIRATE_THROWN",
};

/// The sounds of the shared states: its strike is silent, and its hits
/// play only the shared hit sounds (its own hit lines are never shipped).
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: ["", super::HIT_LOW_SECOND],
    hit_high: "",
};

/// How Claw carries a pirate: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The red-tail pirate class.
pub static CLASS: Class = Class {
    name: "RedTailPirate",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home: states::go_home,
    go_idle: human::go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A red-tail pirate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RedTailPirateData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Chance in 100 of hopping back from Claw's attack.
    pub jump_back_chance: i32,
    /// Cooldown of the reaction to Claw's attacks.
    pub reaction: Timer,
    /// The rest after its last strike.
    pub strike_rest: Timer,
    /// It landed after a throw and has not taunted Claw since.
    pub taunt: bool,
}

/// The pirate data of `id`.
pub fn pirate_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a RedTailPirateData> {
    match &w.objects.get(id)?.data {
        ObjectData::RedTailPirate(d) => Some(d),
        _ => None,
    }
}

fn pirate_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut RedTailPirateData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::RedTailPirate(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        EnemyState::Block => Some(super::townguard::block_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call (which stepping in and
/// backing off need). A landing after a throw during the call sets the
/// taunt.
pub fn tick(w: &mut World, id: ObjectId) {
    let before = data(w, id).map(|d| d.bounces);
    human::tick(w, id);
    let after = data(w, id).map(|d| d.bounces);
    if before.is_some()
        && before != after
        && let Some(p) = pirate_mut(w, id)
    {
        p.taunt = true;
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds a red-tail pirate (see the module documentation). Draws: the
/// shared human go idle's one of 0..100.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: HIGH_STRIKE,
        strike_anim: HIGH_STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: "",
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_ANIM,
                block_high: BLOCK_ANIM,
                block_low: BLOCK_ANIM,
                idle: IDLE_ANIM,
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: IMAGE_SET,
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::RedTailPirate(RedTailPirateData {
            enemy,
            jump_back_chance: FIRST_JUMP_BACK_CHANCE,
            ..RedTailPirateData::default()
        });
    }
    human::go_idle(w, id);
}

// ----------------------------------------------------------------------
// Pre-tick

/// The pirate's own pre-tick steps, after the shared ones: the reaction to
/// Claw's attacks, then the strike choice (see the module
/// documentation). Draws: with the reaction's conditions, one of 0..100.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    reaction(w, id);
    let crouching = w
        .claw_object()
        .is_some_and(|c| c.physics_type == ptype::DUCK);
    if let Some(d) = data_mut(w, id) {
        d.strike_rect = strike_box(crouching);
        d.strike_anim = if crouching {
            LOW_STRIKE_ANIM
        } else {
            HIGH_STRIKE_ANIM
        };
    }
}

/// True when Claw, attacking, faces the enemy he is on the `claw_right`
/// side of.
fn claw_attacks_facing(w: &World, claw_right: bool) -> bool {
    w.claw.attacking && w.claw.facing_right != claw_right
}

/// The reaction to Claw's attacks: Claw attacks facing the pirate within
/// 100 px with the reaction's cooldown over; a draw below the hop chance
/// schedules the hop back when the pirate is free, lowers the chance and
/// restarts the cooldown.
fn reaction(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((right, mode, busy)) = data(w, id).map(|d| (d.claw_right, d.mode, d.busy)) else {
        return;
    };
    let Some((chance, cooldown)) = pirate_data(w, id).map(|p| (p.jump_back_chance, p.reaction))
    else {
        return;
    };
    if !(claw_attacks_facing(w, right) && matches!(mode, 3 | 4) && cooldown.done(now)) {
        return;
    }
    if rand(w, 100) >= chance {
        return;
    }
    if !busy && let Some(d) = data_mut(w, id) {
        d.deferred = Some(Deferred::Human(HumanCall::JumpBack));
    }
    if let Some(p) = pirate_mut(w, id) {
        p.jump_back_chance = (p.jump_back_chance - JUMP_BACK_CHANCE_STEP).max(0);
        p.reaction = Timer::new(now, REACTION_COOLDOWN_MS);
    }
}

// ----------------------------------------------------------------------
// Home

/// Home (see the module documentation). Draws: the alert's or the taunt's,
/// one of 0..121, then the mode's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    let taunt = pirate_data(w, id).is_some_and(|p| p.taunt);
    let alerted = data(w, id).is_none_or(|d| d.alerted);
    if !taunt && !alerted && sees_claw(w, id) {
        if rand(w, 101) < ALERT_CHANCE {
            let line = ALERT_LINES[rand(w, 2) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    if taunt && sees_claw(w, id) {
        let line = TAUNTS[rand(w, 2) as usize];
        say(w, id, line);
        if let Some(p) = pirate_mut(w, id) {
            p.taunt = false;
        }
    }
    face_claw(w, id);
    let r = rand(w, HOME_SPREAD);
    let now = w.now_ms;
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    let rested = pirate_data(w, id).is_some_and(|p| p.strike_rest.done(now));
    match mode {
        0 => enter_fast_advance(w, id),
        1 | 2 => {
            if rested && r < FAR_STRIKE_BELOW {
                if let Some(d) = data_mut(w, id) {
                    d.strike_rect = HIGH_STRIKE;
                    d.strike_anim = HIGH_STRIKE_ANIM;
                }
                strike(w, id);
                rest(w, id, REST_MS);
            } else {
                enter_advance(w, id);
            }
        }
        3 => {
            if r < smarts && rested {
                strike(w, id);
                rest(w, id, CLOSE_REST_MS);
            } else if r < smarts + MOVE_AHEAD_BAND {
                human::enter_move_ahead(w, id);
            }
        }
        4 => {
            if rand(w, 100) < 50 {
                human::enter_move_back(w, id);
            } else if rested {
                strike(w, id);
                rest(w, id, CLOSE_REST_MS);
            }
        }
        _ => {
            if let Some(p) = pirate_mut(w, id) {
                p.jump_back_chance = FIRST_JUMP_BACK_CHANCE;
            }
            start_pacing(w, id);
        }
    }
}

fn rest(w: &mut World, id: ObjectId, ms: u32) {
    let now = w.now_ms;
    if let Some(p) = pirate_mut(w, id) {
        p.strike_rest = Timer::new(now, u64::from(ms));
    }
}

/// Strikes: a quarter of the time one of its four strike lines, then the
/// shared strike with the animation the pre-tick chose. Draws: one of
/// 0..101, then one of 0..4 below 25.
pub fn strike(w: &mut World, id: ObjectId) {
    if rand(w, 101) < STRIKE_LINE_CHANCE {
        let line = STRIKE_LINES[rand(w, 4) as usize];
        say(w, id, line);
    }
    enter_strike(w, id);
}

// ----------------------------------------------------------------------
// Walking at Claw

/// How fast an enemy walks and runs at Claw, px/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speeds {
    /// Walking.
    pub advance: f64,
    /// Running.
    pub fast: f64,
}

/// The pirate's speeds.
pub const SPEEDS: Speeds = Speeds {
    advance: ADVANCE_SPEED as f64,
    fast: FAST_ADVANCE_SPEED as f64,
};

fn toward_claw(w: &World, id: ObjectId, speed: f64) -> f64 {
    if data(w, id).is_some_and(|d| d.claw_right) {
        speed
    } else {
        -speed
    }
}

fn set_vx(w: &mut World, id: ObjectId, vx: f64) {
    if let Some(d) = data_mut(w, id) {
        d.vx = vx;
    }
}

fn mode(w: &World, id: ObjectId) -> u8 {
    data(w, id).map_or(6, |d| d.mode)
}

fn class_call(w: &mut World, id: ObjectId, f: fn(&Class) -> fn(&mut World, ObjectId)) {
    if let Some(c) = class_of(w, id) {
        f(c)(w, id);
    }
}

/// Starts walking at Claw at `s.advance`, facing him.
pub fn enter_advance_at(w: &mut World, id: ObjectId, s: Speeds) {
    face_claw(w, id);
    let vx = toward_claw(w, id, s.advance);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::Advance);
    let anim = class_of(w, id).map_or("", |c| c.anims.advance);
    set_anim(w, id, anim);
}

/// Advance at `s` (the shared rule at the enemy's own speeds): at either
/// limit it goes idle (and carries on); it faces Claw and walks at him;
/// then by mode it runs (0), keeps walking (1, 2) or goes Home (3 to 6).
pub fn advance_tick_at(w: &mut World, id: ObjectId, s: Speeds) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = obj(w, id).x;
    if x == min || x == max {
        class_call(w, id, |c| c.go_idle);
    }
    face_claw(w, id);
    let vx = toward_claw(w, id, s.advance);
    set_vx(w, id, vx);
    w.advance_anim(id);
    match mode(w, id) {
        0 => enter_fast_advance_at(w, id, s),
        1 | 2 => {}
        _ => class_call(w, id, |c| c.go_home),
    }
}

/// Starts running at Claw at `s.fast`, facing him.
pub fn enter_fast_advance_at(w: &mut World, id: ObjectId, s: Speeds) {
    face_claw(w, id);
    let vx = toward_claw(w, id, s.fast);
    set_vx(w, id, vx);
    set_state(w, id, EnemyState::FastAdvance);
    let anim = class_of(w, id).map_or("", |c| c.anims.fast_advance);
    set_anim(w, id, anim);
}

/// FastAdvance at `s` (the shared rule at the enemy's own speeds): at the
/// limit on Claw's side it goes idle (and carries on); it faces Claw; then
/// by mode it keeps running at him (0, 1, 5), walks (2), goes Home (3, 4)
/// or paces (6). Draws: one, discarded.
pub fn fast_advance_tick_at(w: &mut World, id: ObjectId, s: Speeds) {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = obj(w, id).x;
    let right = data(w, id).is_some_and(|d| d.claw_right);
    if (x == min && !right) || (x == max && right) {
        class_call(w, id, |c| c.go_idle);
    }
    face_claw(w, id);
    w.advance_anim(id);
    let _ = w.rng.next_u15();
    match mode(w, id) {
        0 | 1 | 5 => {
            let vx = toward_claw(w, id, s.fast);
            set_vx(w, id, vx);
        }
        2 => enter_advance_at(w, id, s),
        3 | 4 => class_call(w, id, |c| c.go_home),
        6 => start_pacing(w, id),
        _ => {}
    }
}

fn enter_advance(w: &mut World, id: ObjectId) {
    enter_advance_at(w, id, SPEEDS);
}

fn enter_fast_advance(w: &mut World, id: ObjectId) {
    enter_fast_advance_at(w, id, SPEEDS);
}

fn advance_tick(w: &mut World, id: ObjectId) {
    advance_tick_at(w, id, SPEEDS);
}

fn fast_advance_tick(w: &mut World, id: ObjectId) {
    fast_advance_tick_at(w, id, SPEEDS);
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`: the same attack of Claw that hit last does
/// nothing; any other draws the parry roll and the hit-line roll, may be
/// parried or blocked on the eighth and thirteenth levels, and otherwise
/// goes through the shared human hit rule. Draws: one of 0..101 twice,
/// then the parry's or the hit rule's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let serial = w.claw.attack.serial;
    if data(w, id).is_none_or(|d| d.serial_seen == Some(serial)) {
        return;
    }
    let roll = rand(w, 101);
    let _hit_line = rand(w, 101);
    if let Some((parry, shot_block)) = block_chances(w.level_number())
        && blocked(w, id, attacker, roll, parry, shot_block)
    {
        return;
    }
    super::on_hit(w, id, attacker);
}

/// The parry and the shot block of the eighth and thirteenth levels; true
/// when the hit is stopped.
fn blocked(
    w: &mut World,
    id: ObjectId,
    attacker: ObjectId,
    roll: i32,
    parry: i32,
    shot_block: i32,
) -> bool {
    let kind = w.objects.get(attacker).map_or(0, |a| a.type_flags);
    if kind == attacker::CLAW
        && super::townguard::is_sword_swipe(w.claw.attack.kind)
        && roll < parry
    {
        face_claw(w, id);
        let sound = BLOCK_SOUNDS[rand(w, 2) as usize];
        play(w, sound);
        human::enter_block(w, id, true);
        return true;
    }
    if kind == attacker::PISTOL_SHOT && roll < shot_block {
        w.destroy(attacker);
        face_claw(w, id);
        if let Some(d) = data_mut(w, id) {
            d.busy = true;
        }
        set_state(w, id, EnemyState::Block);
        set_anim(w, id, QUICK_BLOCK_ANIM);
        obj_mut(w, id).flags |= flags::SAFE;
        let sound = BLOCK_SOUNDS[rand(w, 2) as usize];
        play(w, sound);
        return true;
    }
    false
}

/// Lifted: one of its five lifted lines, always. Draws: one of 0..5.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    let line = LIFTED_LINES[rand(w, 5) as usize];
    say(w, id, line);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hop_back_chance_falls_by_thirty_a_hop() {
        let chances: Vec<i32> = (0..6).map(jump_back_chance).collect();
        assert_eq!(chances, [90, 60, 30, 0, 0, 0]);
        assert_eq!(jump_back_chance(u32::MAX), 0);
    }

    #[test]
    fn it_parries_only_on_the_eighth_and_thirteenth_levels() {
        assert_eq!(block_chances(7), None);
        assert_eq!(block_chances(8), Some((25, 25)));
        assert_eq!(block_chances(13), Some((50, 75)));
        for level in [1, 6, 9, 14] {
            assert_eq!(block_chances(level), None, "{level}");
        }
    }

    #[test]
    fn its_strike_follows_claws_posture() {
        assert_eq!(strike_box(false), HIGH_STRIKE);
        assert_eq!(strike_box(true), LOW_STRIKE);
        // Both reach 96 px ahead; the low one lies below the high one's
        // middle.
        assert_eq!(HIGH_STRIKE.left, LOW_STRIKE.left);
        const { assert!(LOW_STRIKE.top > 0 && LOW_STRIKE.bottom > HIGH_STRIKE.bottom) };
    }

    #[test]
    fn its_one_state_is_in_the_seventh_levels_block() {
        assert!(STATE_BLOCK.contains(&(EnemyState::GabrielAttack as i32)));
        assert_eq!(EnemyState::GabrielAttack as i32, STATE_BLOCK.start() + 3);
        assert!(CLASS.human, "a human enemy");
        // Steps in and back are faster than the walk, slower than the run.
        const { assert!(ADVANCE_SPEED < STEP_SPEED && STEP_SPEED < FAST_ADVANCE_SPEED) };
        assert_eq!(f64::from(STEP_SPEED), human::MOVE_SPEED);
        // The rest after a strike from further out: 112 ticks; close in,
        // 14.
        assert_eq!(REST_MS.div_ceil(18), 112);
        assert_eq!(CLOSE_REST_MS.div_ceil(18), 14);
    }

    #[test]
    fn it_handles_its_own_decisions_and_walks() {
        for s in [
            EnemyState::Home,
            EnemyState::Advance,
            EnemyState::FastAdvance,
            EnemyState::Block,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        // The shared strike, hop, flinch and idle.
        for s in [
            EnemyState::Strike,
            EnemyState::JumpBack,
            EnemyState::HitLow,
            EnemyState::Idle,
        ] {
            assert!(handler(s).is_none(), "{}", s.name());
        }
    }

    #[test]
    fn its_animations_and_sounds_are_its_own() {
        let a = ANIMS;
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
            HIGH_STRIKE_ANIM,
            LOW_STRIKE_ANIM,
            IDLE_ANIM,
            BLOCK_ANIM,
            QUICK_BLOCK_ANIM,
        ] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for s in ALERT_LINES
            .iter()
            .chain(&TAUNTS)
            .chain(&STRIKE_LINES)
            .chain(&LIFTED_LINES)
            .chain(&BLOCK_SOUNDS)
            .chain(&[DEATH_SOUND])
        {
            assert!(s.starts_with("LEVEL_REDTAILPIRATE_"), "{s}");
        }
        // A low hit plays the first shared hit sound alone.
        assert_eq!(SOUNDS.hit_low, ["", "GAME_HIT1"]);
        assert_eq!(SOUNDS.hit_high, "");
    }
}
