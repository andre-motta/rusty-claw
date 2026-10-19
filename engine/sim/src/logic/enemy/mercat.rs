//! Mercat, the trident-carrying merman of the eleventh and twelfth levels.
//!
//! He is a human enemy who fights mostly from range. Whenever Claw is in
//! his sight and his wait of one to two seconds has run out, he raises
//! his trident and, on the tenth tick of the throw ([`BOLT_TICK`]), throws
//! a bolt of lightning (the trident bolt,
//! [`crate::logic::projectile::trident`]) from [`BOLT_OFFSET`] of his
//! position: it flies straight at [`BOLT_SPEED`] px/s, hurts Claw for
//! [`BOLT_DAMAGE`] and bursts into a short explosion that also hurts the
//! human enemies in it, him included when Claw stands close in front of
//! him. He throws at any distance while Claw is in sight. Only when Claw
//! is within 150 px but out of sight does he stab instead
//! ([`STAB`], [`DAMAGE`]); farther and out of sight he walks in at
//! [`ADVANCE_SPEED`] px/s or runs at [`FAST_ADVANCE_SPEED`] px/s.
//!
//! He parries every crouching sword stab and about a quarter of pistol
//! shots on the eleventh level ([`SHOT_PARRY_CHANCE`]), and while Claw
//! attacks him from within 150 px facing him he hops back about 96 px
//! with a chance of 90, 60, 30, then 0 percent ([`HOP_CHANCE`],
//! [`HOP_STEP`]), reset when he loses track of Claw. He has [`HEALTH`]
//! health on the eleventh level ([`health`]), can be lifted and drops his
//! record's loot.
//!
//! # The rules
//!
//! - Built: his health and damage, smarts 99, liftable, going by sight,
//!   the shared human range boxes (500, 300, 200, 150 and 100 px across,
//!   40 px up or down), his hop chance at 90; his death sound picked;
//!   dropped to the floor; then idle.
//! - Pre-tick, after the shared human steps, every tick: when Claw attacks
//!   (any attack) facing him within 150 px (range modes 3 and 4) and the
//!   hop wait of [`HOP_WAIT_MS`] is over, a draw of 0..100 below the hop
//!   chance makes him say one of his two hop lines, schedules the hop back
//!   for the post-tick when he is free (busy, he does not hop, but the
//!   chance is spent all the same), lowers the chance by 30 (not below 0)
//!   and restarts the wait. A failed draw changes nothing, so it is drawn
//!   again on the next tick.
//! - Going Home: stopped, his decision pose, and a wait of 1000 ms plus a
//!   draw of 0..=1000.
//! - Home: he faces Claw; nothing more while the wait runs. Then, by
//!   mode: 0 and 1, a fifth of the time one of his three advance lines and
//!   a run at Claw; 2, the same with a walk; 3 and 4, the stab when the
//!   stab's own 100 ms wait is over; 5 and 6, his hop chance back to 90
//!   and pacing. Then, whatever that did: with Claw in sight, the throw
//!   ([`enter_fire`]: stopped; while the bolt's cooldown runs nothing
//!   more, so he stands in the state just chosen; otherwise he faces Claw
//!   and raises the trident), and out of sight the shared human Home at
//!   his own speeds when he is still Home. A stab chosen with Claw in
//!   sight is so replaced by the throw on the same tick, and the busy flag
//!   it set stays set until a hit, a parry or a throw's landing frees him.
//! - Fire: on the tick the throw shows its picture 203 (its tenth), the
//!   bolt's sound and the bolt's cooldown ([`bolt_cooldown_ms`]); the bolt
//!   leaves on the same tick ([`crate::logic::projectile::trident`]) and
//!   he goes Home.
//! - Idle: every 1000 ms (the wait it shares with Home) one of his four
//!   idle poses; with Claw in a range box (modes 0 to 4) Home; out of
//!   range, once his idle time of 2000 ms is over, a fifth of the time he
//!   starts pacing (in mode 5 with a line a quarter or half of the time).
//! - The parry ([`EnemyState::Block`]): 4 ticks unhurt, then Home, free.
//! - Hits: a crouching sword stab is parried (he faces Claw first); any
//!   other blow of Claw's makes him cry out (one of two cries) and then
//!   goes through the shared human blow rule; a pistol shot is parried and
//!   destroyed when a draw of 0..101 is below the level's chance,
//!   otherwise it does the same as a blow (a low flinch, the shot gone);
//!   magic, thrown objects, the level's shots and explosions (his own
//!   bolt's among them) hit as they hit any human enemy.
//! - Lifted: his own running line stops and he says his lifted line,
//!   always.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he idles in the shared
//! `Idle`, decides in `Home`, throws in the shared `Fire`, stabs in
//! `Strike`, parries in the town guards' `Block` and hops back in the
//! shared `JumpBack`; he needs no new number. The eleventh level's block
//! [`STATE_BLOCK`] (2260 to 2279) holds the Fish's and the Siren's new
//! names from 2270 ([`super::fish::STATES`], [`super::siren::STATES`]).
//!
//! # Random draws, in order
//!
//! - building: one of 0..100 (the death sound);
//! - the pre-tick: the shared roll (frame pulse ticks only); with the hop
//!   rule's conditions, one of 0..100, then one of 0..2 when it is below
//!   the hop chance;
//! - going Home: one of 0..1001;
//! - Home, once the wait is over: modes 0 to 2, one of 0..101, then one of
//!   0..3 below 20; then, out of sight and still Home, the shared Home's
//!   one of 0..421 (and one of 0..301 before a pause);
//! - Idle: one of 0..4 when the shared wait is over; in modes 0 to 4 going
//!   Home's; in modes 5 and 6 once the idle time is over one of 0..100,
//!   then below 20 in mode 5 one of 0..101;
//! - the parry: one of 0..2; hit by Claw's blow: one of 0..2 (the cry),
//!   then the shared rule's; hit by a pistol shot: one of 0..101, then
//!   one of 0..2 (the parry's sound, or the cry), then the shared rule's.
//!
//! # Defaults taken
//!
//! - The eleventh level's block of state numbers: blocks of 20 in level
//!   order, so the eleventh level owns 2260 to 2279; the Fish's swim, leap
//!   and flop take 2270 to 2272 and the Siren's song 2273, and the numbers
//!   below 2270 stay free. The Fish's dash is the crabs' `Snip` (2195),
//!   run with the Fish's own handler: one name keeps one number whichever
//!   class runs it.
//! - Whether he is on screen, for his sight and the shared human pre-tick,
//!   under a display wider than the original's: the rule the built
//!   soldier and cutthroat use, the object having been drawn in the last
//!   frame.
//! - A laser's beam reaches him through the shared human hit mask
//!   ([`super::HUMAN_HIT_BY`]) and the shared reaction to the level's own
//!   shots: no health lost (a beam deals 0), a flinch.
//! - The shared lift draws one of 0..100 for a lifted line; he says his
//!   own line instead and makes no such draw, so his lift step takes that
//!   draw back from the stream before it is made ([`pick_lift_voice`]).
//!
//! Its data ([`MercatData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, HumanAnims, HumanData};
use super::lift::Carry;
use super::redtail_pirate::{self, Speeds};
use super::states::{self, enter_hit, enter_pause, enter_strike, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, FIRE_BLOW,
    FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Shot, Timer, accept_hit,
    attacker, build, data, data_mut, die, face_claw, faces_right, hit_burst, human_fields, obj,
    obj_mut, play, rand, say, sees_claw, set_anim, set_anim_now, set_state, take_damage,
};
use crate::logic::projectile::trident;
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, flags};
use crate::physics::ptype;
use crate::rng::Rng;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Mercat"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_MERCAT";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_MERCAT_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the eleventh level's families: the
/// Fish's ([`super::fish::STATES`]) and the Siren's
/// ([`super::siren::STATES`]) from 2270; the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2260..=2279;

/// The states he runs with his own handler, in the order the game
/// registers them; none is new.
pub const STATES: [EnemyState; 4] = [
    EnemyState::Idle,
    EnemyState::Home,
    EnemyState::Fire,
    EnemyState::Block,
];

/// His health on the eleventh level, before the difficulty factor (the
/// record's is not used).
pub const HEALTH: i32 = 9;
/// His health on every other level, before the difficulty factor.
pub const HEALTH_ELSEWHERE: i32 = 15;
/// Damage of touching him and of his stab, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// His smarts at the default factor.
pub const SMARTS: i32 = 99;

/// His stab's box, facing left.
pub const STAB: Rect = Rect::new(-111, -5, -22, 22);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: the first picture's.
pub const MOVE_RECT: Rect = Rect::new(-51, -69, 40, 50);
/// Movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The five range boxes (half width, half height), outermost first: the
/// shared human ones.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];

/// Walking speed towards Claw, px/s.
pub const ADVANCE_SPEED: i32 = 150;
/// Running speed towards Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 300;
/// His speeds, for the shared walking states.
pub const SPEEDS: Speeds = Speeds {
    advance: ADVANCE_SPEED as f64,
    fast: FAST_ADVANCE_SPEED as f64,
};

/// Where the bolt leaves, from his position, facing right; mirrored when
/// he faces left.
pub const BOLT_OFFSET: (i32, i32) = (28, 0);
/// The bolt's speed, px/s.
pub const BOLT_SPEED: i32 = 400;
/// The bolt's damage, not scaled by the difficulty.
pub const BOLT_DAMAGE: i32 = 20;
/// The tick of the throw on which the bolt leaves, counted from the
/// throw's first tick at the 18 ms step (measured).
pub const BOLT_TICK: u32 = 10;
/// The picture of the throw on which the bolt leaves.
pub const BOLT_FRAME: i32 = 203;
/// The wait between bolts on the eleventh level, ms.
pub const BOLT_COOLDOWN_MS: u32 = 1000;
/// The wait between bolts on every other level, ms.
pub const BOLT_COOLDOWN_ELSEWHERE_MS: u32 = 750;
/// The least time between two bolts of one throw, ms (it never blocks a
/// bolt: every bolt is followed by a wait of at least 1000 ms).
pub const BOLT_GAP_MS: u64 = 500;
/// His wait after going home, ms: this plus a draw of 0..=1000.
pub const HOME_WAIT_MS: u32 = 1000;
/// The spread of the draw added to [`HOME_WAIT_MS`].
pub const HOME_WAIT_SPREAD: u32 = 1001;
/// Within this distance and out of sight he stabs, px.
pub const STAB_RANGE: i32 = 150;
/// The wait the stab starts before the next one, ms.
pub const STAB_WAIT_MS: u64 = 100;
/// Chance in 101 of an advance line at a decision in modes 0 to 2.
pub const ADVANCE_LINE_CHANCE: i32 = 20;
/// His idle time before pacing may start, ms.
pub const IDLE_MS: u64 = 2000;
/// The time between two idle poses, ms (the wait he shares with Home).
pub const POSE_MS: u64 = 1000;
/// Chance in 100 that an idle tick out of range starts pacing.
pub const PACE_CHANCE: i32 = 20;

/// His first chance in 100 to hop back from an attack.
pub const HOP_CHANCE: i32 = 90;
/// What each hop costs the chance.
pub const HOP_STEP: i32 = 30;
/// How far a hop aims back, px (it lands about 96 px back).
pub const HOP_DISTANCE: i32 = 100;
/// The least time between two hop draws, ms.
pub const HOP_WAIT_MS: u32 = 500;
/// The chance in 101 that he parries a pistol shot on the eleventh level
/// (25) and on the twelfth (50).
pub const SHOT_PARRY_CHANCE: (i32, i32) = (25, 50);

/// The decision pose.
pub const HOME_ANIM: &str = "LEVEL_MERCAT_HOME";
/// The first idle pose (three more follow it).
pub const IDLE_ANIM: &str = "LEVEL_MERCAT_IDLE1";
/// The four idle poses.
pub const IDLE_ANIMS: [&str; 4] = [
    IDLE_ANIM,
    "LEVEL_MERCAT_IDLE2",
    "LEVEL_MERCAT_IDLE3",
    "LEVEL_MERCAT_IDLE4",
];
/// The throw, and the stab.
pub const STRIKE_ANIM: &str = "LEVEL_MERCAT_STRIKE1";
/// The parry.
pub const BLOCK_ANIM: &str = "LEVEL_MERCAT_BLOCK";
/// The hop back.
pub const JUMP_ANIM: &str = "LEVEL_MERCAT_JUMP";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_MERCAT_KILLFALL";
/// The bolt's sound.
pub const BOLT_SOUND: &str = "LEVEL_MERCAT_TLIGHTNING";
/// The stab's sound.
pub const STAB_SOUND: &str = "LEVEL_MERCAT_STAB";
/// The parry's sounds, one of two.
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_MERCAT_BLOCK1", "LEVEL_MERCAT_BLOCK2"];
/// His two death sounds, one picked when he is built (the first below 50
/// of 100).
pub const DEATH_SOUNDS: [&str; 2] = ["LEVEL_MERCAT_00440014", "LEVEL_MERCAT_00440015"];
/// His three advance lines.
pub const ADVANCE_LINES: [&str; 3] = [
    "LEVEL_MERCAT_00440002",
    "LEVEL_MERCAT_00440003",
    "LEVEL_MERCAT_00440004",
];
/// His two pacing lines (mode 5): the first below 25 of 101, the second
/// above 50.
pub const PACE_LINES: [&str; 2] = ["LEVEL_MERCAT_00440000", "LEVEL_MERCAT_00440001"];
/// His two hop lines.
pub const HOP_LINES: [&str; 2] = ["LEVEL_MERCAT_00440009", "LEVEL_MERCAT_00440007"];
/// His two cries when hit.
pub const CRIES: [&str; 2] = ["LEVEL_MERCAT_00440012", "LEVEL_MERCAT_00440013"];
/// His lifted line.
pub const LIFTED_LINE: &str = "LEVEL_MERCAT_00440006";

/// His animations of the shared states.
pub const ANIMS: ClassAnims = ClassAnims {
    advance: "LEVEL_MERCAT_ADVANCE",
    fast_advance: "LEVEL_MERCAT_FASTADVANCE",
    pace: "LEVEL_MERCAT_FASTADVANCE",
    walk: "",
    hit_low: "LEVEL_MERCAT_HITLOW",
    hit_high: "LEVEL_MERCAT_HITHIGH",
    home: HOME_ANIM,
    jump: JUMP_ANIM,
    fall: DEATH_ANIM,
    lifted: "LEVEL_MERCAT_LIFTED1",
    lifted2: "LEVEL_MERCAT_LIFTED2",
    bounce: "LEVEL_MERCAT_THROWN",
};

/// The hit and strike sounds of the shared states.
const SOUNDS: ClassSounds = ClassSounds {
    strike: STAB_SOUND,
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries him: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// His health on level `level`, before the difficulty factor.
pub fn health(level: u32) -> i32 {
    if level == 11 {
        HEALTH
    } else {
        HEALTH_ELSEWHERE
    }
}

/// The wait between his bolts on level `level`, ms.
pub fn bolt_cooldown_ms(level: u32) -> u32 {
    if level == 11 {
        BOLT_COOLDOWN_MS
    } else {
        BOLT_COOLDOWN_ELSEWHERE_MS
    }
}

/// The chance in 101 that he parries a pistol shot on level `level`; 0
/// (never) off the eleventh and twelfth levels.
pub fn shot_parry_chance(level: u32) -> i32 {
    match level {
        11 => SHOT_PARRY_CHANCE.0,
        12 => SHOT_PARRY_CHANCE.1,
        _ => 0,
    }
}

/// The Mercat class.
pub static CLASS: Class = Class {
    name: "Mercat",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A Mercat's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MercatData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Chance in 100 of hopping back from an attack.
    pub hop_chance: i32,
    /// The wait between two hop draws.
    pub hop_wait: Timer,
    /// The wait after going Home before the next decision, which also
    /// times the idle poses.
    pub wait: Timer,
    /// His idle time before pacing may start.
    pub idle: Timer,
    /// The stab's wait before the next stab.
    pub stab_wait: Timer,
    /// The bolt's cooldown.
    pub cooldown: Timer,
    /// The least time between two bolts of one throw.
    pub bolt_gap: Timer,
    /// The bolt of the current throw has left.
    pub fired: bool,
}

/// The Mercat data of `id`.
pub fn mercat_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a MercatData> {
    match &w.objects.get(id)?.data {
        ObjectData::Mercat(d) => Some(d),
        _ => None,
    }
}

/// The Mercat data of `id`, mutable.
pub fn mercat_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut MercatData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Mercat(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::Fire => Some(fire_tick),
        EnemyState::Block => Some(block_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    human::tick(w, id);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Builds a Mercat (see the module documentation). Draws: one of 0..100
/// (his death sound).
pub fn construct(w: &mut World, id: ObjectId) {
    let death_sound = DEATH_SOUNDS[usize::from(rand(w, 100) >= 50)];
    let fields = EnemyFields {
        ranges: RANGES,
        sight: true,
        contact_rect: CONTACT,
        strike_rect: STAB,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound,
        lifted_voice: "",
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_ANIM,
                idle: IDLE_ANIM,
                ..HumanAnims::default()
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
        health: health(w.level_number()),
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Mercat(MercatData {
            enemy,
            hop_chance: HOP_CHANCE,
            ..MercatData::default()
        });
    }
    go_idle(w, id);
}

// ----------------------------------------------------------------------
// Pre-tick

/// True while Claw attacks (any attack) facing an enemy on his
/// `claw_right` side.
fn claw_attacks_facing(w: &World, claw_right: bool) -> bool {
    w.claw.attacking && w.claw.facing_right != claw_right
}

/// His own pre-tick step, after the shared ones: the hop rule (see the
/// module documentation). Draws: with its conditions, one of 0..100, then
/// one of 0..2 below the hop chance.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((right, mode, busy)) = data(w, id).map(|d| (d.claw_right, d.mode, d.busy)) else {
        return;
    };
    let Some((chance, wait)) = mercat_data(w, id).map(|m| (m.hop_chance, m.hop_wait)) else {
        return;
    };
    if !(claw_attacks_facing(w, right) && matches!(mode, 3 | 4) && wait.done(now)) {
        return;
    }
    if rand(w, 100) >= chance {
        return;
    }
    let line = HOP_LINES[rand(w, 2) as usize];
    say(w, id, line);
    if !busy && let Some(d) = data_mut(w, id) {
        d.deferred = Some(super::Deferred::Human(human::HumanCall::JumpBack));
    }
    if let Some(m) = mercat_data_mut(w, id) {
        m.hop_chance = (m.hop_chance - HOP_STEP).max(0);
        m.hop_wait = Timer::new(now, u64::from(HOP_WAIT_MS));
    }
}

// ----------------------------------------------------------------------
// Home

/// Goes Home: stopped, Home, his decision pose (not advanced), and a wait
/// of 1000 ms plus a draw of 0..=1000. It leaves the busy flag as it is.
/// Draws: one of 0..1001.
pub fn go_home(w: &mut World, id: ObjectId) {
    states::go_home(w, id);
    set_anim(w, id, HOME_ANIM);
    let wait = u64::from(HOME_WAIT_MS) + rand(w, HOME_WAIT_SPREAD) as u64;
    let now = w.now_ms;
    if let Some(m) = mercat_data_mut(w, id) {
        m.wait = Timer::new(now, wait);
    }
}

/// Home (see the module documentation). Draws: once the wait is over, the
/// mode's, then the shared Home's when out of sight and still Home.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    w.advance_anim(id);
    let now = w.now_ms;
    if !mercat_data(w, id).is_some_and(|m| m.wait.done(now)) {
        return;
    }
    match data(w, id).map_or(6, |d| d.mode) {
        0 | 1 => {
            advance_line(w, id);
            redtail_pirate::enter_fast_advance_at(w, id, SPEEDS);
        }
        2 => {
            advance_line(w, id);
            redtail_pirate::enter_advance_at(w, id, SPEEDS);
        }
        3 | 4 => {
            if mercat_data(w, id).is_some_and(|m| m.stab_wait.done(now)) {
                enter_strike(w, id);
                if let Some(m) = mercat_data_mut(w, id) {
                    m.stab_wait = Timer::new(now, STAB_WAIT_MS);
                }
            }
        }
        _ => {
            if let Some(m) = mercat_data_mut(w, id) {
                m.hop_chance = HOP_CHANCE;
            }
            start_pacing(w, id);
        }
    }
    if sees_claw(w, id) {
        enter_fire(w, id);
    } else {
        human_home(w, id);
    }
}

/// A fifth of the time one of his three advance lines. Draws: one of
/// 0..101, then one of 0..3 below 20.
fn advance_line(w: &mut World, id: ObjectId) {
    if rand(w, 101) < ADVANCE_LINE_CHANCE {
        let line = ADVANCE_LINES[rand(w, 3) as usize];
        say(w, id, line);
    }
}

/// The shared human Home at his speeds: nothing unless still Home;
/// otherwise he faces Claw and, by mode, runs at him (0, 1), walks at him
/// (2), stabs or pauses 300 to 600 ms in his decision pose (3, 4: he stabs
/// when the draw is below his smarts) or paces (5, 6). Draws: one of
/// 0..421, then one of 0..301 before a pause.
fn human_home(w: &mut World, id: ObjectId) {
    if super::state(w, id) != Some(EnemyState::Home) {
        return;
    }
    face_claw(w, id);
    let r = rand(w, 421);
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    match mode {
        0 | 1 => redtail_pirate::enter_fast_advance_at(w, id, SPEEDS),
        2 => redtail_pirate::enter_advance_at(w, id, SPEEDS),
        3 | 4 => {
            if r < smarts {
                enter_strike(w, id);
            } else {
                set_anim_now(w, id, HOME_ANIM);
                let ms = states::PAUSE_MIN + rand(w, states::PAUSE_SPREAD) as u64;
                enter_pause(w, id, ms);
            }
        }
        _ => start_pacing(w, id),
    }
}

fn advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::advance_tick_at(w, id, SPEEDS);
}

fn fast_advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::fast_advance_tick_at(w, id, SPEEDS);
}

// ----------------------------------------------------------------------
// The throw

/// The throw's entry: stopped; while the bolt's cooldown runs nothing
/// more (he stays in whatever state he is in); otherwise he faces Claw,
/// raises the trident (the throw, not advanced) and loads the bolt: 28 px
/// in front of him at his height, 400 px/s the way he faces. It leaves
/// the busy flag as it is.
pub fn enter_fire(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    let now = w.now_ms;
    if !mercat_data(w, id).is_some_and(|m| m.cooldown.done(now)) {
        return;
    }
    face_claw(w, id);
    set_state(w, id, EnemyState::Fire);
    set_anim(w, id, STRIKE_ANIM);
    let right = faces_right(obj(w, id));
    let (dx, vx) = if right {
        (BOLT_OFFSET.0, BOLT_SPEED)
    } else {
        (-BOLT_OFFSET.0, -BOLT_SPEED)
    };
    if let Some(d) = data_mut(w, id) {
        d.shot = Shot {
            dx,
            dy: BOLT_OFFSET.1,
            vx,
            vy: 0,
        };
    }
    if let Some(m) = mercat_data_mut(w, id) {
        m.fired = false;
    }
}

/// Fire: on the throw's picture 203 the bolt's sound and its cooldown;
/// then the bolt leaves (when the least time between two has passed) and
/// he goes Home, on the same tick. Draws: going Home's.
pub fn fire_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let level = w.level_number();
    if obj(w, id).frame == BOLT_FRAME && mercat_data(w, id).is_some_and(|m| !m.fired) {
        play(w, BOLT_SOUND);
        if let Some(m) = mercat_data_mut(w, id) {
            m.fired = true;
            m.cooldown = Timer::new(now, u64::from(bolt_cooldown_ms(level)));
        }
    }
    let Some((fired, gap)) = mercat_data(w, id).map(|m| (m.fired, m.bolt_gap)) else {
        return;
    };
    if !fired {
        return;
    }
    if gap.done(now) {
        throw_bolt(w, id);
        if let Some(m) = mercat_data_mut(w, id) {
            m.bolt_gap = Timer::new(now, BOLT_GAP_MS);
        }
    }
    go_home(w, id);
}

/// The bolt leaves as loaded, one depth behind him.
fn throw_bolt(w: &mut World, id: ObjectId) {
    let Some(s) = data(w, id).map(|d| d.shot) else {
        return;
    };
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    trident::launch(w, id, (x + s.dx, y + s.dy, z - 1), s.vx);
}

// ----------------------------------------------------------------------
// Idle

/// Goes idle: stopped across, Idle, his first idle pose advanced once, and
/// his idle time of 2000 ms. He does not face Claw and draws nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    set_anim_now(w, id, IDLE_ANIM);
    let now = w.now_ms;
    if let Some(m) = mercat_data_mut(w, id) {
        m.idle = Timer::new(now, IDLE_MS);
    }
}

/// Idle (see the module documentation). Draws: one of 0..4 when the
/// shared wait is over; going Home's in modes 0 to 4; in modes 5 and 6
/// once the idle time is over one of 0..100, then below 20 in mode 5 one
/// of 0..101.
pub fn idle_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if mercat_data(w, id).is_some_and(|m| m.wait.done(now)) {
        if let Some(m) = mercat_data_mut(w, id) {
            m.wait = Timer::new(now, POSE_MS);
        }
        let pose = IDLE_ANIMS[rand(w, 4) as usize];
        set_anim_now(w, id, pose);
    }
    let mode = data(w, id).map_or(6, |d| d.mode);
    match mode {
        0..=4 => go_home(w, id),
        _ => {
            if !mercat_data(w, id).is_some_and(|m| m.idle.done(now)) {
                return;
            }
            if rand(w, 100) >= PACE_CHANCE {
                return;
            }
            if mode == 5 {
                let r = rand(w, 101);
                if r < 25 {
                    say(w, id, PACE_LINES[0]);
                } else if r > 50 {
                    say(w, id, PACE_LINES[1]);
                }
            }
            start_pacing(w, id);
        }
    }
}

// ----------------------------------------------------------------------
// The parry

/// Parries: busy, the parry pose (not advanced), unhurt, and one of his
/// two parry sounds. Draws: one of 0..2.
pub fn enter_block(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::Block);
    set_anim(w, id, BLOCK_ANIM);
    obj_mut(w, id).flags |= flags::SAFE;
    let sound = BLOCK_SOUNDS[rand(w, 2) as usize];
    play(w, sound);
}

/// Block: the parry pose plays (4 ticks); when it ends, hits are allowed
/// again and he goes Home, free.
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

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: a sword stab while Claw crouches is parried (he faces
///   Claw); any other blow makes him cry out, then the shared blow rule
///   (during a flinch he only goes back Home; otherwise the damage, a hit
///   burst, a 300 ms cooldown and the flinch, high after an uppercut);
/// - a pistol shot: parried and destroyed when a draw of 0..101 is below
///   the level's chance ([`shot_parry_chance`]); otherwise the cry, the
///   damage, a burst, the shot destroyed and a low flinch;
/// - magic or a thrown object: damage, burst, low flinch;
/// - a shot of the level or an explosion: damage, burst, 2000 ms
///   cooldown, low flinch;
/// - a springboard-like object: thrown toward its target.
///
/// At 0 health or below he dies.
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
            let ducking = w
                .claw_object()
                .is_some_and(|c| c.physics_type == ptype::DUCK);
            if super::townguard::is_sword_swipe(w.claw.attack.kind) && ducking {
                face_claw(w, id);
                enter_block(w, id);
            } else {
                cry(w, id);
                let high = w.claw.attack.uppercut;
                blow_rule(w, id, attacker, high);
            }
        }
        attacker::PISTOL_SHOT => {
            if rand(w, 101) < shot_parry_chance(w.level_number()) {
                w.destroy(attacker);
                face_claw(w, id);
                enter_block(w, id);
            } else {
                cry(w, id);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                w.destroy(attacker);
                enter_hit(w, id, false);
            }
        }
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
        attacker::SPRING => states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// One of his two cries. Draws: one of 0..2.
fn cry(w: &mut World, id: ObjectId) {
    let line = CRIES[rand(w, 2) as usize];
    say(w, id, line);
}

/// The shared human blow rule: during a flinch he only goes back Home;
/// otherwise the damage, a hit burst, a 300 ms cooldown and the flinch.
fn blow_rule(w: &mut World, id: ObjectId, attacker: ObjectId, high: bool) {
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

// ----------------------------------------------------------------------
// Lifted

/// The multiplier that undoes one step of the shared random stream.
const RNG_UNSTEP_MUL: u32 = 3_115_528_533;
/// The increment of one step of the shared random stream.
const RNG_STEP_ADD: u32 = 2_531_011;

/// The stream one draw back: the state whose next draw leads to `state`.
pub fn rng_one_back(state: u32) -> u32 {
    state
        .wrapping_sub(RNG_STEP_ADD)
        .wrapping_mul(RNG_UNSTEP_MUL)
}

/// Lifted: his own running line stops and he says his lifted line, always.
/// He makes none of the shared lift's draws: the stream is set one draw
/// back, so the shared lift's draw of 0..100 that follows this step leaves
/// it where it was (see the module documentation).
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    sound::enemy_silence(w, id);
    say(w, id, LIFTED_LINE);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
    let back = rng_one_back(w.rng.state());
    *w.rng = Rng::new(back);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eleventh_levels_block_follows_the_tenths() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2260, 2279));
        assert_eq!(
            *super::super::marrow::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        // He needs no new number.
        for s in STATES {
            assert!(!STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert_eq!(EnemyState::Fire as i32, 2013);
        assert_eq!(EnemyState::Block as i32, 2141);
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Strike).is_none(), "the shared stab");
        assert!(handler(EnemyState::JumpBack).is_none(), "the shared hop");
    }

    #[test]
    fn his_numbers_follow_the_level() {
        assert_eq!((health(11), health(12)), (9, 15));
        assert_eq!((bolt_cooldown_ms(11), bolt_cooldown_ms(12)), (1000, 750));
        assert_eq!(
            (
                shot_parry_chance(11),
                shot_parry_chance(12),
                shot_parry_chance(1)
            ),
            (25, 50, 0)
        );
        // The bolt flies 7 or 8 px a tick at the 18 ms step.
        assert_eq!(BOLT_SPEED * 18, 7_200);
        // Two hits of 5 kill him on the eleventh level.
        const { assert!(HEALTH <= 2 * 5 && HEALTH > 5) };
        // Four hop draws take the chance to 0.
        assert_eq!(HOP_CHANCE - 3 * HOP_STEP, 0);
        // His stab reaches 111 px ahead of him.
        assert_eq!(-STAB.left, 111);
        assert_eq!(RANGES[3].0, STAB_RANGE);
        assert_eq!(HOP_DISTANCE, human::JUMP_BACK_DISTANCE);
    }

    #[test]
    fn one_step_back_undoes_one_draw() {
        for seed in [0u32, 1, 111_567, 1_980_168_385, u32::MAX] {
            let mut r = Rng::new(rng_one_back(seed));
            r.next_u15();
            assert_eq!(r.state(), seed);
        }
    }

    #[test]
    fn his_art_is_his_own() {
        for key in [
            HOME_ANIM,
            STRIKE_ANIM,
            BLOCK_ANIM,
            JUMP_ANIM,
            DEATH_ANIM,
            BOLT_SOUND,
            STAB_SOUND,
            LIFTED_LINE,
            ANIMS.advance,
            ANIMS.fast_advance,
            ANIMS.hit_low,
            ANIMS.hit_high,
            ANIMS.lifted,
            ANIMS.lifted2,
            ANIMS.bounce,
        ]
        .iter()
        .chain(&BLOCK_SOUNDS)
        .chain(&IDLE_ANIMS)
        .chain(&DEATH_SOUNDS)
        .chain(&ADVANCE_LINES)
        .chain(&PACE_LINES)
        .chain(&HOP_LINES)
        .chain(&CRIES)
        {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }
}
