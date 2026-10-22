//! Raux, the boss of the second level: a human enemy who waits for the
//! fight to start, then dodges, blocks and strikes back.
//!
//! Raux is a heavier officer. He keeps the human enemy's walking, pacing
//! and striking, and adds three habits:
//!
//! - he dodges when Claw, facing him, attacks from within 150 px (and 20 px
//!   up or down): on a tick with a per-tick roll below 500 he hops back
//!   (roll below 100), hops up (100 to 199) or leaps over Claw to the far
//!   end of his ground (200 to 499), when free;
//! - he blocks one sword blow in ten (roll below 100) and every pistol
//!   shot, stick of dynamite, magic shot and thrown object, which are
//!   destroyed; environment shots do nothing to him;
//! - after every flinch he strikes straight back.
//!
//! He cannot be lifted. He has 100 health and deals 5 damage (both times
//! the difficulty factors), so ten blows of Claw's plain sword end the
//! fight. His strike reaches 30 to 117 px in front of him; his touch, and
//! his body while he leaps over Claw, hurt Claw too.
//!
//! As a boss he is the level's current boss from the moment he is built
//! ([`World::current_boss`]) until his body is removed. He waits in
//! `Startup` until the fight flag says the fight is on
//! ([`World::boss_fight`]), unless Claw is out of his range on his first
//! ticks, hits him or makes him dodge. The boss health bar, when the
//! introduction made one, reads his health (he never sets it himself). When
//! Claw respawns he goes back to where he was placed, with full health. His
//! death, while Claw is alive, is reported once through
//! [`World::boss_defeated`], which holds Claw and releases the gem that
//! ends the level.
//!
//! # After a skipped introduction
//!
//! Nothing in him depends on the introduction: with it skipped (see
//! [`crate::logic::boss_stage`]) the fight flag stays 0, so `Startup` only
//! ends through the ways every enemy state has. While Claw stays within one
//! of his range boxes, does not swing at him from close by and sends
//! nothing at him, he keeps waiting in `Startup`, idle and harmless, for
//! the rest of the level. Claw outside every box for 500 ms with Raux on
//! screen sends him idle and into the fight; so do a sword blow, a dodge
//! and a block.
//!
//! State numbers ([`EnemyState`]): one number per state name, so Raux
//! waits for the fight in the bosses' `Startup` and uses the shared
//! numbers for the states he shares with other enemies (his own idle and
//! Home included); his own leap over Claw is `JumpOver`, the first of his
//! block (2070-2089).
//!
//! His data ([`RauxData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`]; what he keeps as a boss is in its human data
//! ([`super::human::BossFields`]).
//!
//! # Random draws, in order
//!
//! Each tick (his logic call):
//!
//! 1. the shared pre-tick's roll of 0..=1000 (on frame pulse ticks);
//! 2. one of 0..3: the strike and block sounds of the tick;
//! 3. one of 0..100 when Claw attacks facing him from within his two
//!    inner range boxes (it passes below his smarts, always at 100);
//! 4. the state's draws: Idle one of 0..100 on a frame pulse tick once
//!    his wait is over (modes 0, 1, 2, 5, 6); Home one of 0..121, then in mode 4 one of 0..100,
//!    then moving back's or moving ahead's, then the shared Home's when he
//!    is still Home; FastAdvance one, discarded; Pace one of 0..100 per
//!    limit reached.
//!
//! When hit by Claw's sword and not blocking: one of 0..4 for a high
//! flinch's sound, then the hit burst's; at his death the death's.

use super::human::{self, BossFields, HumanAnims, HumanCall, HumanData};
use super::states::{enter_hit, enter_strike, go_home, hit_tick, human_home_tick, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, DeathKind, Deferred, EnemyData, EnemyFields, EnemyState,
    FIRE_BLOW, FIRE_DEATH_ANIM, HUMAN_HIT_BY, Handler, ICE_BLOW, ICE_DEATH_ANIM, Timer, accept_hit,
    attacker, build, data, data_mut, die, face_claw, hit_burst, human_fields, obj, obj_mut, rand,
    say, scaled, set_anim, set_state, take_damage,
};
use crate::logic::world_boss::fight;
use crate::logic::{ObjectData, World};
use crate::object::{ObjectId, Rect, engine_state, flags};
use crate::player::attack::kind as attack_kind;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Raux"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &["LEVEL_RAUX"];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_RAUX_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// His strike.
pub const STRIKE_ANIM: &str = "LEVEL_RAUX_STRIKE";
/// Idle, and waiting for the fight.
pub const IDLE_ANIM: &str = "LEVEL_RAUX_IDLE";
/// The hop back.
pub const JUMP_BACK_ANIM: &str = "LEVEL_RAUX_JUMPBACK";
/// The high block (he never blocks high).
pub const BLOCK_HIGH_ANIM: &str = "LEVEL_RAUX_BLOCKHIGH";
/// The low block.
pub const BLOCK_LOW_ANIM: &str = "LEVEL_RAUX_BLOCKLOW";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_RAUX_DEAD";

/// The strike box, facing left: 30 to 117 px in front of him.
pub const STRIKE: Rect = Rect::new(-117, -4, -30, 21);
/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-15, -60, 15, 52);
/// Hit box: only the 20 px behind his origin.
pub const HIT_RECT: Rect = Rect::new(0, -60, 20, 35);
/// The five range boxes (half width, half height), outermost first: he
/// walks at Claw within 300 px, steps in, strikes or leaps over him
/// within 150 px, and backs off or strikes within 100 px, always within
/// 20 px up or down.
pub const RANGES: [(i32, i32); 5] = [(500, 20), (300, 40), (300, 20), (150, 20), (100, 20)];

/// Health before the difficulty factor.
pub const HEALTH: i32 = 100;
/// Damage of his strike and touch before the difficulty factor.
pub const DAMAGE: i32 = 5;
/// Smarts base.
pub const SMARTS: i32 = 100;

/// His death sound.
pub const DEATH_SOUND: &str = "LEVEL_RAUX_00840005";
/// His line on going idle (with a roll below 500).
pub const IDLE_LINE: &str = "LEVEL_RAUX_00840003";
/// The lifted line he never says (he cannot be lifted).
pub const LIFTED_VOICE: &str = "LEVEL_DOGINAIR";
/// The strike sounds, one picked each tick.
pub const STRIKE_SOUNDS: [&str; 3] = ["LEVEL_STAB", "LEVEL_STAB2", "LEVEL_STAB3"];
/// The block sounds: picked with the first two strike sounds; the third
/// keeps the last one (none until the first pick).
pub const BLOCK_SOUNDS: [&str; 2] = ["LEVEL_RAUX_BLOCK1", "LEVEL_RAUX_BLOCK2"];

/// Time idle before he may start pacing, ms.
pub const IDLE_WAIT_MS: u64 = 4000;
/// His idle line: said when the roll is below this.
pub const IDLE_LINE_ROLL: i32 = 500;
/// Chance in 100 per idle tick, once his wait is over, of pacing.
pub const PACE_CHANCE: i32 = 5;
/// Home: draws of 0..121.
pub const HOME_SPREAD: u32 = 121;
/// Home in mode 2: a draw below this leaps over Claw.
pub const JUMP_OVER_CHANCE: i32 = 20;
/// Home in mode 3: a draw below smarts + this (and not below smarts)
/// steps in.
pub const MOVE_AHEAD_BAND: i32 = 20;
/// The hit cooldown of a sword blow, ms.
pub const HIT_COOLDOWN_MS: u64 = 350;
/// A sword blow with a roll below this is blocked.
pub const BLOCK_ROLL: i32 = 100;
/// Dodge bands of the roll: below the first hop back, below the second hop
/// up, below the third leap over Claw.
pub const DODGE_ROLLS: [i32; 3] = [100, 200, 500];

/// The Raux class.
pub static CLASS: Class = Class {
    name: "Raux",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle,
    carry: None,
    pick_lift_voice: super::nothing,
    lift_roll: true,
    anims: ClassAnims {
        advance: "LEVEL_RAUX_ADVANCE",
        fast_advance: "LEVEL_RAUX_FASTADVANCE",
        pace: "LEVEL_RAUX_FASTADVANCE",
        walk: "",
        hit_low: "LEVEL_RAUX_HITLOW",
        hit_high: "LEVEL_RAUX_HITHIGH",
        home: "LEVEL_RAUX_HOME",
        jump: "LEVEL_RAUX_JUMP",
        fall: "LEVEL_RAUX_FALL",
        lifted: "LEVEL_RAUX_LIFTED",
        lifted2: "LEVEL_RAUX_LIFTED2",
        bounce: "LEVEL_RAUX_THROWN",
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
};

/// Raux's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RauxData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::Home => Some(home_tick),
        EnemyState::HitLow | EnemyState::HitHigh => Some(flinch_tick),
        EnemyState::JumpOver => Some(human::jump_over_tick),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call: the human logic call ([`human::run`]), then his
/// death's report and the health bar. Being removed, he stops being the
/// current boss. Claw's respawn resets him itself ([`reset`]).
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING && w.current_boss() == Some(id) {
        w.set_current_boss(None);
    }
    human::run(w, id, &CLASS);
    after_call(w, id);
}

/// `attacker` hit `id`. A blow carrying fire or ice picks the matching
/// death; the same attack of Claw hits only once and nothing hits during
/// the cooldown. Then, by what hit:
///
/// - Claw himself: a 350 ms cooldown; during a flinch he only goes back
///   Home; with a roll below 100 he blocks low (no damage); otherwise he
///   flinches (high after an uppercut), takes the damage and shows a hit
///   burst;
/// - a pistol shot (or dynamite), magic or a thrown object: it is
///   destroyed and he blocks low, unhurt;
/// - an environment shot: nothing;
/// - a springboard: thrown toward its target, as any human enemy.
///
/// At 0 health or below he dies (and the death is reported).
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
    match kind {
        attacker::CLAW => {
            let now = w.now_ms;
            if let Some(d) = data_mut(w, id) {
                d.cooldown = Timer::new(now, HIT_COOLDOWN_MS);
            }
            let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
            if matches!(
                super::state(w, id),
                Some(EnemyState::HitLow | EnemyState::HitHigh)
            ) {
                go_home(w, id);
            } else if (0..BLOCK_ROLL).contains(&roll) {
                human::enter_block(w, id, false);
            } else {
                let high = w.claw.attack.uppercut;
                enter_hit(w, id, high);
                take_damage(w, id, attacker);
                hit_burst(w, id, attacker);
            }
        }
        attacker::PISTOL_SHOT | attacker::MAGIC | attacker::THROWN => {
            w.destroy(attacker);
            human::enter_block(w, id, false);
        }
        attacker::SPRING => super::states::enter_spring_jump(w, id, attacker),
        _ => {}
    }
    if obj(w, id).health <= 0 {
        die(w, id, DeathKind::Hit, Some(attacker));
        boss_died(w, id);
    }
}

/// `id` attacked `victim`: nothing more than any enemy.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// What follows each logic call: a death seen for the first time (on a
/// death tile, or a kill by another logic) is reported. He never shows the
/// health bar himself: the bar the introduction makes reads his health
/// (see [`crate::logic::boss_stage`]).
fn after_call(w: &mut World, id: ObjectId) {
    let Some((dead, seen)) = data(w, id).map(|d| (d.dead, boss(d).death_seen)) else {
        return;
    };
    if dead && !seen {
        boss_died(w, id);
    }
}

fn boss(d: &EnemyFields) -> BossFields {
    d.human.boss.unwrap_or_default()
}

fn with_boss(w: &mut World, id: ObjectId, f: impl FnOnce(&mut BossFields)) {
    if let Some(b) = data_mut(w, id).and_then(|d| d.human.boss.as_mut()) {
        f(b);
    }
}

/// He died: when Claw is alive, his defeat is reported (once).
fn boss_died(w: &mut World, id: ObjectId) {
    let claw_alive = w.claw.stats.health > 0;
    let mut report = false;
    with_boss(w, id, |b| {
        b.death_seen = true;
        if claw_alive && !b.defeat_reported {
            b.defeat_reported = true;
            report = true;
        }
    });
    if report {
        w.boss_defeated(id);
    }
}

/// The reset when Claw respawns, run by the respawn once Claw is back at
/// his checkpoint and before its update passes
/// ([`World::reset_boss_on_respawn`]): back where he was placed with full
/// health; his state, speed, timers and busy flag stay as they are.
pub fn reset(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some(b) = data(w, id).and_then(|d| d.human.boss) else {
        return;
    };
    with_boss(w, id, |b| b.attempt = attempt);
    let o = obj_mut(w, id);
    o.x = b.start.0;
    o.y = b.start.1;
    o.health = b.full_health;
}

// ----------------------------------------------------------------------
// Building

/// Builds Raux: the current boss, his place kept for the respawn reset,
/// his boxes, 100 health and 5 damage (times the difficulty factors),
/// smarts 100, never liftable, always active; then he drops to the floor
/// and waits for the fight in `Startup`. Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    let start = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    w.set_current_boss(Some(id));
    let full_health = scaled(HEALTH, super::HEALTH_FACTOR);
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: STRIKE,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: LIFTED_VOICE,
        smarts: super::smarts(SMARTS),
        may_lift: false,
        human: HumanData {
            anims: HumanAnims {
                jump_back: JUMP_BACK_ANIM,
                block_high: BLOCK_HIGH_ANIM,
                block_low: BLOCK_LOW_ANIM,
                idle: IDLE_ANIM,
            },
            boss: Some(BossFields {
                start,
                full_health,
                attempt: w.attempt(),
                ..BossFields::default()
            }),
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: "LEVEL_RAUX",
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    o.flags |= flags::ALWAYS_ACTIVE;
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Raux(RauxData { enemy });
    }
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

// ----------------------------------------------------------------------
// Pre-tick

/// His pre-tick steps after the shared ones: the strike and block sounds
/// of the tick, then the dodge. Draws: one of 0..3; then, when Claw
/// attacks facing him from within 150 px (mode 3 or 4), one of 0..100
/// against his smarts.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    let pick = rand(w, 3) as usize;
    if let Some(d) = data_mut(w, id) {
        d.human.sounds.strike = STRIKE_SOUNDS[pick];
        if let Some(&block) = BLOCK_SOUNDS.get(pick) {
            d.human.sounds.block_low = block;
        }
    }
    let attacking = w.claw.attack.kind != attack_kind::NONE;
    let claw_facing_right = w.claw.facing_right;
    let Some((right, mode, busy, roll, smarts)) =
        data(w, id).map(|d| (d.claw_right, d.mode, d.busy, d.roll, d.smarts))
    else {
        return;
    };
    let faces_him = claw_facing_right != right;
    if !(attacking && faces_him && (mode == 3 || mode == 4)) {
        return;
    }
    if rand(w, 100) >= smarts {
        return;
    }
    let call = if roll < 0 {
        None
    } else if roll < DODGE_ROLLS[0] {
        Some(HumanCall::JumpBack)
    } else if roll < DODGE_ROLLS[1] {
        Some(HumanCall::Jump)
    } else if roll < DODGE_ROLLS[2] {
        Some(HumanCall::JumpOver)
    } else {
        None
    };
    if let Some(call) = call
        && !busy
        && let Some(d) = data_mut(w, id)
    {
        d.deferred = Some(Deferred::Human(call));
    }
}

// ----------------------------------------------------------------------
// States

/// Startup: waits, idle, until the fight is on; then he goes idle.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == fight::ON {
        go_idle(w, id);
    }
}

/// Goes idle: stops, says his idle line when the roll is below 500, and
/// waits 4000 ms before pacing may start. He does not turn to Claw. Draws
/// nothing.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    set_state(w, id, EnemyState::Idle);
    set_anim(w, id, IDLE_ANIM);
    let roll = data(w, id).map_or(i32::MAX, |d| d.roll);
    if (0..IDLE_LINE_ROLL).contains(&roll) {
        say(w, id, IDLE_LINE);
    }
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pace_wait = Timer::new(now, IDLE_WAIT_MS);
    }
}

/// Idle: Home when Claw is within 150 px (modes 3, 4); otherwise, once
/// his wait is over, a 5% chance of pacing on each frame pulse tick.
/// Draws: one of 0..100 on a frame pulse tick once the wait is over (not
/// in modes 3, 4).
pub fn idle_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let Some((mode, wait)) = data(w, id).map(|d| (d.mode, d.pace_wait)) else {
        return;
    };
    match mode {
        3 | 4 => go_home(w, id),
        _ if wait.done(now) && w.pulse && rand(w, 100) < PACE_CHANCE => start_pacing(w, id),
        _ => {}
    }
}

/// Home: faces Claw and draws `r` of 0..121; within 300 px (mode 2) he
/// leaps over Claw when `r` is below 20; within 150 px (mode 3) he
/// strikes when `r` is below his smarts, or steps in when below smarts +
/// 20; within 100 px (mode 4) he backs off or strikes, even odds. Still
/// at Home (no choice, or a move that could not be made), the shared Home
/// rule decides. Draws: one of 0..121; in mode 4 one of 0..100; then the
/// move's; then the shared Home's.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    let r = rand(w, HOME_SPREAD);
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    match mode {
        2 if r < JUMP_OVER_CHANCE => human::enter_jump_over(w, id),
        3 if r < smarts => enter_strike(w, id),
        3 if r < smarts + MOVE_AHEAD_BAND => human::enter_move_ahead(w, id),
        4 => {
            if rand(w, 100) < 50 {
                human::enter_move_back(w, id);
            } else {
                enter_strike(w, id);
            }
        }
        _ => {}
    }
    human_home_tick(w, id);
}

/// HitLow and HitHigh: the shared flinch; when it ends he strikes at once.
pub fn flinch_tick(w: &mut World, id: ObjectId) {
    hit_tick(w, id);
    if obj(w, id).anim.is_done() {
        enter_strike(w, id);
    }
}
