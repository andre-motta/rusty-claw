//! The tiger guards of the fourteenth level: tiger-headed temple guards
//! with knives and claws, orange or white.
//!
//! A tiger is a human enemy that duels at close range. It runs at Claw at
//! [`SPEED`] px/s, and from up to 500 px away it may strike: a knife combo
//! of two to four slashes ([`KNIFE_RECT`], [`KNIFE_DAMAGE`] each), a lunge
//! (a half-second crouch, then a dash at [`LUNGE_SPEED`] towards where
//! Claw stood with its claws out, [`LUNGE_RECT`], [`LUNGE_DAMAGE`]), or a
//! front flip that lands [`FLIP_DISTANCE`] px nearer and slashes once.
//! Between strikes it rests, and close in it nearly always flips back once
//! while it rests.
//!
//! What makes it hard is its defence: Claw's pistol shots and his sticks
//! of dynamite, the objects he throws and every blast only make it block
//! for a quarter of a second, unhurt (a shot or a stick that reaches it is
//! gone). Only Claw's blows and his magic hurt it, and his blows only
//! while it is not lunging or flipping. During both flips nothing reaches
//! it at all. While Claw attacks it from in front at close range it also
//! hops back, two times in five per try.
//!
//! It has [`HEALTH`] health (three sword blows), can be lifted and thrown,
//! drops its record's loot, growls once when it first sees Claw, taunts
//! and strikes back after landing from a throw, and speaks on some knife
//! combos and on being lifted. The record's smarts field picks its colour
//! ([`WHITE_SET`] when 1) and nothing else.
//!
//! # The rules
//!
//! - Built: the health and damage above, smarts 99 (unused), liftable,
//!   not going by sight, the shared human range boxes [`RANGES`]; dropped
//!   to the floor; then it goes Home (not idle).
//! - Going Home: its touch costs [`DAMAGE`] again, it is no longer
//!   attacking, stopped, Home, its idle pose (not advanced), and back in
//!   pair collision.
//! - Pre-tick, after the shared human steps: unless it flips, blocks or
//!   lunges, while Claw attacks (any attack) facing it within 150 px
//!   (range modes 3 and 4) with the reaction's 500 ms cooldown over, a
//!   draw of 0..100 below [`JUMP_BACK_CHANCE`] schedules the hop back for
//!   the post-tick when it is free and restarts the cooldown. The chance
//!   never changes.
//! - Home: the pose advances. Once, the first time it sees Claw, half the
//!   time one of its three growls. After a throw's landing, when it sees
//!   Claw: its taunt, then even odds of a lunge or a knife combo of 2 to 4
//!   slashes, and nothing else this tick. Then it faces Claw and, by range
//!   mode: 0 and 1 (Claw 200 to 500 px away): 80 in 101 it runs at him,
//!   otherwise it strikes; 2 (150 to 200 px): 70 in 101 it strikes,
//!   otherwise it runs; 3 and 4 (within 150 px): 10 in 101 it flips back,
//!   otherwise it strikes; 5 and 6: it paces.
//! - Striking: while its rest runs it only goes Home again. Otherwise it
//!   rests 1250 to 2000 ms from now ([`REST_MS`]), stops, faces Claw and
//!   draws: below 40 of 101 the lunge, below 80 a knife combo of 2 to 4
//!   slashes, otherwise the front flip with one slash after it.
//! - The knife combo: outside range mode 4 (Claw beyond 100 px) it runs at
//!   him instead; otherwise 40 times in 101 one of its four knife lines,
//!   then a slash (the slash picture's box live on every tick that shows
//!   it, six ticks), and while slashes remain, the combo again (mode test
//!   included); then Home.
//! - The lunge: attacking (Claw's blows do not hurt it), aimed at Claw's x
//!   now, the lunge growl; still during the crouch, then at the lunge
//!   speed; the claws' box live on the strike picture; Home once its
//!   whole-pixel x has reached the aimed x towards the side Claw is on
//!   now, on the patrol limit on that side, or when the strike ends.
//! - The front flip: attacking, out of pair collision, 250 px/s towards
//!   Claw for [`FLIP_DISTANCE`] px with a flip sound. Every tick it asks
//!   from the side Claw is on now: once its whole-pixel x has reached the
//!   landing point towards that side, the one slash (or Home) at once and
//!   nothing more that tick, so a flip that carries the tiger past Claw
//!   ends on the first tick he is behind it, its slash live in that tick;
//!   on the limit on Claw's side, the same end; after [`FLIP_MS`] the
//!   slash too, and Home in every case. The slash after the flip runs
//!   still attacking.
//! - The back flip: on the limit behind it, the front flip with a slash
//!   instead; otherwise out of pair collision, 250 px/s away from Claw for
//!   [`FLIP_DISTANCE`] px with a flip sound; Home once past that point
//!   away from the side Claw is on now, on either limit or after
//!   [`FLIP_MS`].
//! - The block: it faces Claw, stops and holds its guard for [`BLOCK_MS`];
//!   then Home. A block leaves the attacking flag, the flips' collision
//!   and the strike rest as they were.
//! - Hits: the shared gate first (the attack of Claw's that hit last does
//!   not hit again, nor anything during the hit cooldown); then Claw's
//!   blow: nothing while attacking, otherwise the shared human blow rule
//!   (a flinch); a pistol shot or a stick of dynamite: gone, the block
//!   sound and the block; a thrown object or a blast (a stick's, a lava
//!   ball's): the block; magic: the damage and a low flinch; a jump switch:
//!   the leap to its target. At 0 health or below it dies.
//! - Lifted: one time in ten its rarer lifted line, otherwise the other.
//! - Every landing after a throw sets the taunt.
//!
//! # State numbers
//!
//! It decides in `Home` with a handler of its own and blocks in the town
//! guards' `Block`. The fourteenth level's block [`STATE_BLOCK`] (2320 to
//! 2339) holds its new names from 2320 ([`STATES`]: the lunge and the two
//! flips) and Omar's from 2330 ([`super::omar::STATES`]). Its knife combo
//! is the state of the same name as RedTail's knives, `KnifeAttack`
//! (2300): one number per state name. Every other state is the shared
//! human one with its number.
//!
//! # Random draws, in order
//!
//! - building: none;
//! - the pre-tick: the shared roll (frame pulse ticks only); with the
//!   reaction's conditions, one of 0..100;
//! - Home: the growl's one of 0..101 (then one of 0..3 below 50), or the
//!   taunt's one of 0..101 (then one of 0..3 for a knife combo); then
//!   modes 0 to 4: one of 0..101;
//! - striking with its rest over: one of 0..751, one of 0..101, then one
//!   of 0..3 for a knife combo;
//! - the knife combo in mode 4: one of 0..101, then one of 0..4 below 40;
//! - each flip: one of 0..2;
//! - the shared states, hits, death and loot: theirs; lifted: one of
//!   0..101.
//!
//! # Defaults taken
//!
//! - The range boxes are the shared human ones, box 1 300 by 40 px among
//!   them, as the original's runs show on every tiger.
//! - A blast of a lava ball on a tiger is a blast: it blocks, unhurt.
//! - The "on screen" test of its growl and taunt under a widened view is
//!   the one the other human enemies' sight test uses.
//! - The lunge's and the knife's windows at 60 frames per second follow
//!   the animations' own milliseconds, as everything here does.
//!
//! Its data ([`TigerGuardData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use super::human::{self, HumanAnims, HumanCall, HumanData};
use super::lift::Carry;
use super::redtail_pirate::{self, Speeds};
use super::states::{self, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit,
    attacker, build, data, data_mut, die, exact_x, face_claw, facing_rect, hit_burst, human_fields,
    obj, obj_mut, play, rand, record, say, sees_claw, set_anim, set_anim_now, set_state,
    take_damage,
};
use crate::level::SimLevel;
use crate::logic::{Look, ObjectData, World, jump_switch};
use crate::object::{Object, ObjectId, Rect, engine_state, flags};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TigerGuard"];

/// The orange tiger's picture.
pub const IMAGE_SET: &str = "LEVEL_TIGER";
/// The white tiger's picture (the same animations over its own frames).
pub const WHITE_SET: &str = "LEVEL_TIGERWHITE";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, WHITE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_TIGER_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the fourteenth level's families: the
/// tiger's from 2320 ([`STATES`]) and Omar's from 2330
/// ([`super::omar::STATES`]); the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2320..=2339;

/// Its new state names, in number order from the start of
/// [`STATE_BLOCK`].
pub const STATES: [EnemyState; 3] = [
    EnemyState::LungeAttack,
    EnemyState::FrontFlip,
    EnemyState::BackFlip,
];

/// The record field whose value 1 makes a tiger white.
pub const SMARTS_FIELD: usize = 17;

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 15;
/// What touching it costs Claw, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// What a slash of the knife combo costs Claw.
pub const KNIFE_DAMAGE: i32 = 15;
/// What the lunge's claws cost Claw.
pub const LUNGE_DAMAGE: i32 = 20;
/// Smarts base (raised to 99; nothing of the tiger's reads it).
pub const SMARTS: i32 = 50;
/// Its running speed, and its walking speed, px/s.
pub const SPEED: i32 = 180;
/// The lunge's dash, px/s.
pub const LUNGE_SPEED: f64 = 300.0;
/// Both flips' speed, px/s.
pub const FLIP_SPEED: f64 = 250.0;
/// How far a flip goes, px.
pub const FLIP_DISTANCE: i32 = 100;
/// A flip that has not arrived ends after this, ms.
pub const FLIP_MS: u64 = 1000;
/// How long a block holds, ms.
pub const BLOCK_MS: u64 = 250;
/// The rest after a strike: this plus a draw of 0..751, ms.
pub const REST_MS: u64 = 1250;
/// The spread of the draw added to [`REST_MS`].
pub const REST_SPREAD: u32 = 751;
/// Time after a passed reaction roll before the next one, ms.
pub const REACTION_COOLDOWN_MS: u64 = 500;
/// Chance in 100 of hopping back from Claw's attack.
pub const JUMP_BACK_CHANCE: i32 = 40;
/// Chance in 101 of the growl on first seeing Claw.
pub const ALERT_CHANCE: i32 = 50;
/// Chance in 101 of a lunge (else a knife combo) after the taunt.
pub const TAUNT_LUNGE_CHANCE: i32 = 50;
/// Chance in 101 of a knife line on entering the knife combo.
pub const KNIFE_LINE_CHANCE: i32 = 40;
/// Chance in 101 of the rarer lifted line.
pub const RARE_LIFT_CHANCE: i32 = 10;

/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: its first picture's.
pub const MOVE_RECT: Rect = Rect::new(-23, -53, 48, 48);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-58, -27, 49, 15);
/// The contact box outside its attacks: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// A knife slash's box, facing left.
pub const KNIFE_RECT: Rect = Rect::new(-63, -58, 0, -1);
/// The lunge's claws, facing left.
pub const LUNGE_RECT: Rect = Rect::new(-74, -39, -1, 15);
/// The five range boxes (half width, half height), outermost first: the
/// shared human ones.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];

/// The picture of a slash that cuts.
pub const KNIFE_FRAME: i32 = 202;
/// The lunge's crouch picture: no movement.
pub const CROUCH_FRAME: i32 = 800;
/// The lunge's strike picture: the claws cut.
pub const LUNGE_FRAME: i32 = 402;

/// The knife combo's slash.
pub const KNIFE_ANIM: &str = "LEVEL_TIGER_STRIKE1";
/// The lunge.
pub const LUNGE_ANIM: &str = "LEVEL_TIGER_STRIKE3";
/// The front flip.
pub const FRONT_FLIP_ANIM: &str = "LEVEL_TIGER_FRONTFLIP";
/// The back flip.
pub const BACK_FLIP_ANIM: &str = "LEVEL_TIGER_BACKFLIP";
/// The block.
pub const BLOCK_ANIM: &str = "LEVEL_TIGER_BLOCK";
/// Standing idle, and its pose at Home.
pub const IDLE_ANIM: &str = "LEVEL_TIGER_IDLE";
/// Running, walking and pacing.
pub const RUN_ANIM: &str = "LEVEL_TIGER_FASTADVANCE";
/// Its jump: the hop back and the jump switch's leap.
pub const JUMP_ANIM: &str = "LEVEL_TIGER_JUMP";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_TIGER_KILLFALL";

/// Its death sound.
pub const DEATH_SOUND: &str = "LEVEL_TIGERGUARD_00510002";
/// Its three growls on first seeing Claw.
pub const ALERT_LINES: [&str; 3] = [
    "LEVEL_TIGERGUARD_00510012",
    "LEVEL_TIGERGUARD_00510013",
    "LEVEL_TIGERGUARD_00510025",
];
/// Its taunt after a throw.
pub const TAUNT: &str = "LEVEL_TIGERGUARD_00510028";
/// Its four knife lines.
pub const KNIFE_LINES: [&str; 4] = [
    "LEVEL_TIGERGUARD_00510004",
    "LEVEL_TIGERGUARD_00510006",
    "LEVEL_TIGERGUARD_00510018",
    "LEVEL_TIGERGUARD_00510029",
];
/// The lunge's growl.
pub const LUNGE_SOUND: &str = "LEVEL_TIGERGUARD_00510009";
/// Its lifted lines: the usual one, then the rarer one.
pub const LIFTED_LINES: [&str; 2] = ["LEVEL_TIGERGUARD_00510020", "LEVEL_TIGERGUARD_00510000"];
/// The sound of a shot or a stick blocked.
pub const BLOCK_SOUND: &str = "LEVEL_TIGERGUARD_BLOCK1";
/// The flips' two sounds.
pub const FLIP_SOUNDS: [&str; 2] = ["LEVEL_TIGERGUARD_FLIP", "LEVEL_TIGERGUARD_FLIP2"];

/// Its running and walking speeds.
pub const SPEEDS: Speeds = Speeds {
    advance: SPEED as f64,
    fast: SPEED as f64,
};

/// The animations of the shared states.
pub const ANIMS: ClassAnims = ClassAnims {
    advance: RUN_ANIM,
    fast_advance: RUN_ANIM,
    pace: RUN_ANIM,
    walk: "",
    hit_low: "LEVEL_TIGER_HITLOW",
    hit_high: "LEVEL_TIGER_HITHIGH",
    home: "LEVEL_TIGER_HOME",
    jump: JUMP_ANIM,
    fall: DEATH_ANIM,
    lifted: "LEVEL_TIGER_LIFTED1",
    lifted2: "LEVEL_TIGER_LIFTED2",
    bounce: "LEVEL_TIGER_THROWN",
};

/// The sounds of the shared states: no strike of its own, and its hits
/// play only the shared hit sounds.
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: ["", super::HIT_LOW_SECOND],
    hit_high: "",
};

/// How Claw carries a tiger: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The tiger guard class.
pub static CLASS: Class = Class {
    name: "TigerGuard",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle: human::go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    lift_roll: false,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A tiger guard's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TigerGuardData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// White (the record's smarts field 1) rather than orange.
    pub white: bool,
    /// Cooldown of the reaction to Claw's attacks.
    pub reaction: Timer,
    /// The rest after its last strike.
    pub strike_rest: Timer,
    /// It landed after a throw and has not taunted Claw since.
    pub taunt: bool,
    /// Attacking: lunging, flipping forward or slashing after the flip.
    /// Claw's blows do not hurt it.
    pub attacking: bool,
    /// Slashes left in the knife combo.
    pub slashes: u32,
    /// A slash follows the front flip.
    pub slash_after: bool,
    /// The lunge's speed, px/s (negative to the left).
    pub lunge_speed: f64,
    /// The x the lunge or the flip ends past.
    pub target: i32,
    /// The flip started towards the right (its end tests read the side
    /// Claw is on now).
    pub flip_right: bool,
    /// The flip's time limit.
    pub flip: Timer,
    /// The block's time.
    pub block: Timer,
}

/// The tiger data of `id`.
pub fn tiger_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a TigerGuardData> {
    match &w.objects.get(id)?.data {
        ObjectData::TigerGuard(d) => Some(d),
        _ => None,
    }
}

/// The tiger data of `id`, mutable.
pub fn tiger_data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut TigerGuardData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::TigerGuard(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        EnemyState::KnifeAttack => Some(knife_tick),
        EnemyState::LungeAttack => Some(lunge_tick),
        EnemyState::FrontFlip => Some(front_flip_tick),
        EnemyState::BackFlip => Some(back_flip_tick),
        EnemyState::Block => Some(block_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call. A landing after a throw
/// during the call sets the taunt.
pub fn tick(w: &mut World, id: ObjectId) {
    let before = data(w, id).map(|d| d.bounces);
    human::tick(w, id);
    let after = data(w, id).map(|d| d.bounces);
    if before.is_some()
        && before != after
        && after != Some(0)
        && let Some(t) = tiger_data_mut(w, id)
    {
        t.taunt = true;
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building

/// Whether a record's smarts value makes a white tiger.
pub fn is_white(smarts: i32) -> bool {
    smarts == 1
}

/// Builds a tiger guard (see the module documentation). Draws: none.
pub fn construct(w: &mut World, id: ObjectId) {
    let white = is_white(record(w, id, SMARTS_FIELD));
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: KNIFE_RECT,
        strike_anim: KNIFE_ANIM,
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
        image_set: if white { WHITE_SET } else { IMAGE_SET },
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
        o.data = ObjectData::TigerGuard(TigerGuardData {
            enemy,
            white,
            ..TigerGuardData::default()
        });
    }
    go_home(w, id);
}

/// How a tiger whose first logic call has not run yet looks once it has
/// (see [`crate::logic::first_look`]): white or orange by its record, on
/// its patrol limit when placed beside it, standing on the floor below its
/// place (32 px higher, then down). Worked out on a copy, from the level's
/// tiles only: nothing in the game changes and no random number is drawn.
/// `None` once the tiger is built.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    if o.state != engine_state::INIT {
        return None;
    }
    let field = |i: usize| o.record.as_deref().map_or(0, |r| r[i]);
    let mut k = o.clone();
    let set = if is_white(field(SMARTS_FIELD)) {
        WHITE_SET
    } else {
        IMAGE_SET
    };
    k.image_set = crate::level::runtime_image_set(set);
    k.move_rect = MOVE_RECT;
    let bounds = (
        super::limit(field(super::field::X_MIN), super::UNBOUNDED.0),
        super::limit(field(super::field::X_MAX), super::UNBOUNDED.1),
    );
    let bounds_y = (
        super::limit(field(super::field::Y_MIN), super::UNBOUNDED.0),
        super::limit(field(super::field::Y_MAX), super::UNBOUNDED.1),
    );
    super::clamp_on_construction(&mut k, bounds, bounds_y);
    k.y -= 32;
    crate::physics::align_to_ground(&level.map, &mut k);
    Some(vec![Look::of(&k)])
}

// ----------------------------------------------------------------------
// Pre-tick

/// The tiger's own pre-tick step, after the shared ones: the reaction to
/// Claw's attacks (see the module documentation). Draws: with the
/// reaction's conditions, one of 0..100.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    if matches!(
        super::state(w, id),
        Some(
            EnemyState::BackFlip
                | EnemyState::FrontFlip
                | EnemyState::Block
                | EnemyState::LungeAttack
        )
    ) {
        return;
    }
    let now = w.now_ms;
    let Some((right, mode, busy)) = data(w, id).map(|d| (d.claw_right, d.mode, d.busy)) else {
        return;
    };
    let cooldown = tiger_data(w, id).map_or(Timer::default(), |t| t.reaction);
    let facing = w.claw.attacking && w.claw.facing_right != right;
    if !(facing && matches!(mode, 3 | 4) && cooldown.done(now)) {
        return;
    }
    if rand(w, 100) >= JUMP_BACK_CHANCE {
        return;
    }
    if !busy && let Some(d) = data_mut(w, id) {
        d.deferred = Some(Deferred::Human(HumanCall::JumpBack));
    }
    if let Some(t) = tiger_data_mut(w, id) {
        t.reaction = Timer::new(now, REACTION_COOLDOWN_MS);
    }
}

// ----------------------------------------------------------------------
// Home

/// Goes Home (see the module documentation). It leaves the busy flag as
/// it is.
pub fn go_home(w: &mut World, id: ObjectId) {
    obj_mut(w, id).damage = super::scaled(DAMAGE, super::DAMAGE_FACTOR);
    if let Some(t) = tiger_data_mut(w, id) {
        t.attacking = false;
    }
    states::go_home(w, id);
    set_anim(w, id, IDLE_ANIM);
    obj_mut(w, id).flags &= !flags::NO_COLLIDE;
}

/// Home (see the module documentation). Draws: the growl's or the
/// taunt's, then modes 0 to 4: one of 0..101, then the move's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let taunt = tiger_data(w, id).is_some_and(|t| t.taunt);
    let alerted = data(w, id).is_none_or(|d| d.alerted);
    if !taunt && !alerted && sees_claw(w, id) {
        if rand(w, 101) < ALERT_CHANCE {
            let line = ALERT_LINES[rand(w, 3) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    if taunt && sees_claw(w, id) {
        say(w, id, TAUNT);
        if let Some(t) = tiger_data_mut(w, id) {
            t.taunt = false;
        }
        if rand(w, 101) < TAUNT_LUNGE_CHANCE {
            enter_lunge(w, id);
        } else {
            let n = 2 + rand(w, 3) as u32;
            enter_knife(w, id, n);
        }
        return;
    }
    face_claw(w, id);
    match data(w, id).map_or(6, |d| d.mode) {
        0 | 1 => {
            if rand(w, 101) < 80 {
                enter_fast_advance(w, id);
            } else {
                strike(w, id);
            }
        }
        2 => {
            if rand(w, 101) >= 70 {
                enter_fast_advance(w, id);
            } else {
                strike(w, id);
            }
        }
        3 | 4 => {
            if rand(w, 101) < 10 {
                enter_back_flip(w, id);
            } else {
                strike(w, id);
            }
        }
        _ => start_pacing(w, id),
    }
}

/// Strikes (see the module documentation). Draws: with its rest over,
/// one of 0..751, one of 0..101, then one of 0..3 for a knife combo.
pub fn strike(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if !tiger_data(w, id).is_some_and(|t| t.strike_rest.done(now)) {
        go_home(w, id);
        return;
    }
    let rest = REST_MS + rand(w, REST_SPREAD) as u64;
    if let Some(t) = tiger_data_mut(w, id) {
        t.strike_rest = Timer::new(now, rest);
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
    }
    face_claw(w, id);
    let r = rand(w, 101);
    if r < 40 {
        enter_lunge(w, id);
    } else if r < 80 {
        let n = 2 + rand(w, 3) as u32;
        enter_knife(w, id, n);
    } else {
        enter_front_flip(w, id, true);
    }
}

// ----------------------------------------------------------------------
// Running and walking at Claw

fn enter_fast_advance(w: &mut World, id: ObjectId) {
    redtail_pirate::enter_fast_advance_at(w, id, SPEEDS);
}

fn advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::advance_tick_at(w, id, SPEEDS);
}

fn fast_advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::fast_advance_tick_at(w, id, SPEEDS);
}

// ----------------------------------------------------------------------
// The knife combo

/// Enters the knife combo with `n` slashes (see the module documentation).
/// Draws: in range mode 4, one of 0..101, then one of 0..4 below 40.
pub fn enter_knife(w: &mut World, id: ObjectId, n: u32) {
    if let Some(t) = tiger_data_mut(w, id) {
        t.slashes = n;
    }
    face_claw(w, id);
    obj_mut(w, id).flags &= !flags::NO_COLLIDE;
    if data(w, id).map_or(6, |d| d.mode) != 4 {
        enter_fast_advance(w, id);
        return;
    }
    if rand(w, 101) < KNIFE_LINE_CHANCE {
        let line = KNIFE_LINES[rand(w, 4) as usize];
        say(w, id, line);
    }
    set_state(w, id, EnemyState::KnifeAttack);
    set_anim(w, id, KNIFE_ANIM);
    obj_mut(w, id).damage = super::scaled(KNIFE_DAMAGE, super::DAMAGE_FACTOR);
}

/// KnifeAttack: the slash's box live while the slash picture shows; at
/// the slash's end the next slash, or Home.
pub fn knife_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = obj_mut(w, id);
    if o.frame == KNIFE_FRAME {
        o.attack_rect = facing_rect(o, KNIFE_RECT);
    }
    if !obj(w, id).anim.is_done() {
        return;
    }
    let left = tiger_data_mut(w, id).map_or(0, |t| {
        t.slashes = t.slashes.saturating_sub(1);
        t.slashes
    });
    if left > 0 {
        enter_knife(w, id, left);
    } else {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The lunge

/// Enters the lunge (see the module documentation). Draws: none.
pub fn enter_lunge(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    let target = w.claw_object().map_or(obj(w, id).x, |c| c.x);
    if let Some(t) = tiger_data_mut(w, id) {
        t.attacking = true;
        t.lunge_speed = if right { LUNGE_SPEED } else { -LUNGE_SPEED };
        t.target = target;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
    }
    play(w, LUNGE_SOUND);
    set_state(w, id, EnemyState::LungeAttack);
    set_anim_now(w, id, LUNGE_ANIM);
    obj_mut(w, id).damage = super::scaled(LUNGE_DAMAGE, super::DAMAGE_FACTOR);
}

/// True when the tiger, moving right when `right`, has reached `target`
/// by its exact position.
fn past(w: &World, id: ObjectId, right: bool, target: i32) -> bool {
    let x = exact_x(w, id);
    if right {
        x >= f64::from(target)
    } else {
        x <= f64::from(target)
    }
}

/// True when the tiger, looking right when `right`, has reached `target`
/// by its whole-pixel position (the exact x cut towards zero).
fn past_whole(w: &World, id: ObjectId, right: bool, target: i32) -> bool {
    let x = exact_x(w, id).trunc();
    if right {
        x >= f64::from(target)
    } else {
        x <= f64::from(target)
    }
}

/// True when Claw is on the tiger's right now (the pre-tick's side).
fn claw_right(w: &World, id: ObjectId) -> bool {
    data(w, id).is_some_and(|d| d.claw_right)
}

/// True when the tiger stands exactly on its patrol limit on the right
/// when `right`, else on the left.
fn on_limit(w: &World, id: ObjectId, right: bool) -> bool {
    let (min, max) = data(w, id).map_or(super::UNBOUNDED, |d| d.bounds);
    let x = exact_x(w, id);
    if right {
        x == f64::from(max)
    } else {
        x == f64::from(min)
    }
}

/// LungeAttack: still while crouching, then the dash; the claws' box live
/// on the strike picture; Home past the aimed x or on the limit, both
/// towards the side Claw is on now, or at the strike's end.
pub fn lunge_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some((speed, target)) = tiger_data(w, id).map(|t| (t.lunge_speed, t.target)) else {
        return;
    };
    let frame = obj(w, id).frame;
    if let Some(d) = data_mut(w, id) {
        d.vx = if frame == CROUCH_FRAME { 0.0 } else { speed };
    }
    if frame == LUNGE_FRAME {
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, LUNGE_RECT);
    }
    let right = claw_right(w, id);
    if past_whole(w, id, right, target) || on_limit(w, id, right) || obj(w, id).anim.is_done() {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The flips

fn flip_sound(w: &mut World) {
    let s = FLIP_SOUNDS[rand(w, 2) as usize];
    play(w, s);
}

/// Enters the front flip, with a slash after it when `slash` (see the
/// module documentation). Draws: one of 0..2.
pub fn enter_front_flip(w: &mut World, id: ObjectId, slash: bool) {
    let now = w.now_ms;
    face_claw(w, id);
    let right = data(w, id).is_some_and(|d| d.claw_right);
    let x = obj(w, id).x;
    if let Some(t) = tiger_data_mut(w, id) {
        t.slash_after = slash;
        t.attacking = true;
        t.flip_right = right;
        t.target = if right {
            x.saturating_add(FLIP_DISTANCE)
        } else {
            x.saturating_sub(FLIP_DISTANCE)
        };
        t.flip = Timer::new(now, FLIP_MS);
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = if right { FLIP_SPEED } else { -FLIP_SPEED };
    }
    set_state(w, id, EnemyState::FrontFlip);
    set_anim(w, id, FRONT_FLIP_ANIM);
    flip_sound(w);
    obj_mut(w, id).flags |= flags::NO_COLLIDE;
}

/// The front flip's end: the slash after it when there is one, else Home.
fn end_front_flip(w: &mut World, id: ObjectId) {
    if tiger_data(w, id).is_some_and(|t| t.slash_after) {
        enter_knife(w, id, 1);
    } else {
        go_home(w, id);
    }
}

/// FrontFlip: past its point towards the side Claw is on now, its end and
/// nothing more this tick; on the limit on Claw's side, its end; after its
/// time limit, the slash too, and Home in every case.
pub fn front_flip_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let Some((target, timer, slash)) = tiger_data(w, id).map(|t| (t.target, t.flip, t.slash_after))
    else {
        return;
    };
    let right = claw_right(w, id);
    if past_whole(w, id, right, target) {
        end_front_flip(w, id);
        return;
    }
    if on_limit(w, id, right) {
        end_front_flip(w, id);
    }
    if timer.done(now) {
        if slash {
            enter_knife(w, id, 1);
        }
        go_home(w, id);
    }
}

/// Enters the back flip (see the module documentation): on the limit
/// behind it, the front flip with a slash instead. Draws: one of 0..2.
pub fn enter_back_flip(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    face_claw(w, id);
    let claw_right = data(w, id).is_some_and(|d| d.claw_right);
    let away_right = !claw_right;
    if on_limit(w, id, away_right) {
        enter_front_flip(w, id, true);
        return;
    }
    let x = obj(w, id).x;
    if let Some(t) = tiger_data_mut(w, id) {
        t.target = if away_right {
            x.saturating_add(FLIP_DISTANCE)
        } else {
            x.saturating_sub(FLIP_DISTANCE)
        };
        t.flip = Timer::new(now, FLIP_MS);
        t.flip_right = away_right;
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = if away_right { FLIP_SPEED } else { -FLIP_SPEED };
    }
    set_state(w, id, EnemyState::BackFlip);
    set_anim(w, id, BACK_FLIP_ANIM);
    flip_sound(w);
    obj_mut(w, id).flags |= flags::NO_COLLIDE;
}

/// BackFlip: Home past its point away from the side Claw is on now, on
/// either limit, or after its time limit.
pub fn back_flip_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let Some((target, timer)) = tiger_data(w, id).map(|t| (t.target, t.flip)) else {
        return;
    };
    let away_right = !claw_right(w, id);
    if past(w, id, away_right, target)
        || on_limit(w, id, true)
        || on_limit(w, id, false)
        || timer.done(now)
    {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// The block

/// Blocks (see the module documentation).
pub fn enter_block(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    face_claw(w, id);
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
    }
    if let Some(t) = tiger_data_mut(w, id) {
        t.block = Timer::new(now, BLOCK_MS);
    }
    set_state(w, id, EnemyState::Block);
    set_anim(w, id, BLOCK_ANIM);
}

/// Block: Home once its time is up.
pub fn block_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    if tiger_data(w, id).is_some_and(|t| t.block.done(now)) {
        go_home(w, id);
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id` (see the module documentation). Draws: a blow's or
/// magic's hit burst and flinch, then the death's.
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
            if tiger_data(w, id).is_some_and(|t| t.attacking) {
                return;
            }
            if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else {
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
                if let Some(d) = data_mut(w, id) {
                    d.cooldown = Timer::new(now, 300);
                }
                let high = w.claw.attack.uppercut;
                states::enter_hit(w, id, high);
            }
        }
        attacker::PISTOL_SHOT => {
            w.destroy(attacker);
            play(w, BLOCK_SOUND);
            enter_block(w, id);
        }
        attacker::MAGIC => {
            take_damage(w, id, attacker);
            hit_burst(w, id, attacker);
            states::enter_hit(w, id, false);
        }
        attacker::THROWN | attacker::LEVEL_SHOT => enter_block(w, id),
        attacker::SPRING => jump_switch::launch(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
    }
}

/// Lifted: one time in ten its rarer lifted line, otherwise the usual
/// one. Draws: one of 0..101.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    let line = if rand(w, 101) < RARE_LIFT_CHANCE {
        LIFTED_LINES[1]
    } else {
        LIFTED_LINES[0]
    };
    say(w, id, line);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_states_open_the_fourteenth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2320, 2339));
        assert_eq!(
            *super::super::redtail::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        for (i, s) in STATES.iter().enumerate() {
            assert_eq!(*s as i32, *STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
        // Its knife combo reuses the number of the state of that name.
        assert_eq!(EnemyState::KnifeAttack as i32, 2300);
        assert!(CLASS.human, "a human enemy");
        assert!(CLASS.carry.is_some(), "it can be lifted");
        assert!(!CLASS.lift_roll, "its lifted line is its own draw");
    }

    #[test]
    fn it_handles_its_own_decisions_attacks_and_block() {
        for s in [
            EnemyState::Home,
            EnemyState::Advance,
            EnemyState::FastAdvance,
            EnemyState::KnifeAttack,
            EnemyState::LungeAttack,
            EnemyState::FrontFlip,
            EnemyState::BackFlip,
            EnemyState::Block,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        // The shared hop, flinch, idle, pacing and leap.
        for s in [
            EnemyState::JumpBack,
            EnemyState::HitLow,
            EnemyState::Idle,
            EnemyState::Pace,
            EnemyState::Jump,
            EnemyState::Strike,
        ] {
            assert!(handler(s).is_none(), "{}", s.name());
        }
    }

    #[test]
    fn its_numbers() {
        // Three sword blows kill it.
        assert_eq!(HEALTH / 5, 3);
        // The lunge: 19 moving ticks at 5.4 px a tick, about 103 px.
        assert!((LUNGE_SPEED * 0.018 * 19.0 - 102.6).abs() < 1e-9);
        // A flip: 4.5 px a tick, past 100 px on the 23rd tick.
        assert_eq!(
            (f64::from(FLIP_DISTANCE) / (FLIP_SPEED * 0.018)).ceil(),
            23.0
        );
        // The block: 14 ticks; the rest: 70 to 112 ticks.
        assert_eq!(BLOCK_MS.div_ceil(18), 14);
        assert_eq!(REST_MS.div_ceil(18), 70);
        assert_eq!((REST_MS + u64::from(REST_SPREAD) - 1).div_ceil(18), 112);
        // The shared human boxes, box 1 300 by 40 px.
        assert_eq!(RANGES, super::super::chameleon::RANGES);
        assert_eq!(RANGES[1], (300, 40));
        assert!(is_white(1) && !is_white(0) && !is_white(2));
    }

    #[test]
    fn its_animations_and_sounds_are_its_own() {
        let a = ANIMS;
        for anim in [
            a.advance,
            a.hit_low,
            a.hit_high,
            a.home,
            a.jump,
            a.fall,
            a.lifted,
            a.lifted2,
            a.bounce,
            KNIFE_ANIM,
            LUNGE_ANIM,
            FRONT_FLIP_ANIM,
            BACK_FLIP_ANIM,
            BLOCK_ANIM,
            IDLE_ANIM,
        ] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for s in ALERT_LINES
            .iter()
            .chain(&KNIFE_LINES)
            .chain(&LIFTED_LINES)
            .chain(&FLIP_SOUNDS)
            .chain(&[TAUNT, LUNGE_SOUND, BLOCK_SOUND, DEATH_SOUND])
        {
            assert!(s.starts_with("LEVEL_TIGERGUARD_"), "{s}");
        }
    }
}
