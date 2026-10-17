//! What happens when something touches Claw: damage from enemies and
//! shots, and every pickup he collects.
//!
//! The pair-collision pass calls [`claw_hit`] when an object of a kind
//! Claw can be hit by overlaps him. What happens depends on the kind of
//! the object that touched him.
//!
//! # Damage
//!
//! - An enemy's body hurts unless its touch is harmless or Claw carries
//!   it. A random hit sound and a random pain line play, and enemies'
//!   bodies cannot hurt him again for [`INVULNERABLE_MS`].
//! - A shot (an enemy's, the level's own or a thrown object) plays Claw's
//!   hit sound, a random hit sound and a random pain line; enemies' bodies
//!   and the level's shots cannot hurt him again for [`INVULNERABLE_MS`]
//!   (enemies' shots still can). The shot is removed, except the level's
//!   own shots, which fly on, and shots that go off on impact.
//!
//! The damage is the attacker's (1 when it has none). It comes off Claw's
//! health ([`take_damage`]); at 0 or below he dies. Otherwise, unless he
//! is already reeling from a hit, he drops what he carries, stops his
//! attack, starts the hit reaction and a hit burst flashes where he was
//! struck.
//!
//! With the invincibility aid on ([`crate::tuning::PlayAids::invincible`])
//! none of this happens: an enemy's touch, a shot, a hazard or an
//! explosion that reaches Claw is ignored as if he were not there (no
//! sound, no damage, no hit reaction, no window; a shot flies on through
//! him). Pickups work as always, and death tiles still kill him.
//!
//! # Pickups
//!
//! Claw reads a pickup through [`PickupFields`]: for a pickup the pickup
//! family runs, the kind, amount and duration it set up from its picture
//! (see [`PickupFields::of_pickup`]). Collected pickups play
//! their sound (once per tick per sound) and are removed, except where
//! noted:
//!
//! - Health adds its amount, up to [`MAX_HEALTH`]; at full health the
//!   pickup is left in the level. An amount above 100 is catnip instead,
//!   for the pickup's duration (or its amount in ms).
//! - The large health potion adds its amount even at full health.
//! - Treasure adds its points to the score, counts its kind and flies to
//!   the status bar: it is not removed here.
//! - Pistol shots and magic charges add their amount, up to [`MAX_AMMO`].
//! - Special pickups carry a special code: the end of the level, warps,
//!   dynamite, curses, the ghost, Vader and sword powerups, and extra lives
//!   (up to [`MAX_LIVES`], flying to the status bar).

use claw_frame::{AudioCue, GameEvent, SoundChannel};

use super::death::{self, DeathCause, powerup};
use super::{Ctx, special, state};
use crate::logic::{self, effect, pickup, projectile};
use crate::object::{Object, ObjectId, Rect, flags, special as sflags, types};
use crate::physics::ptype;

/// Length of the window after a hit in which enemies cannot hurt Claw, ms.
pub const INVULNERABLE_MS: i32 = 850;
/// Most health Claw can have.
pub const MAX_HEALTH: i32 = 100;
/// Most pistol shots, magic charges or sticks of dynamite Claw can carry.
pub const MAX_AMMO: i32 = 99;
/// Most lives Claw can have.
pub const MAX_LIVES: i32 = 10;
/// Default length of the ghost, Vader and sword powerups, ms.
pub const POWERUP_DEFAULT_MS: u32 = 30_000;
/// Sticks of dynamite a dynamite pickup gives when it names no amount.
pub const DYNAMITE_DEFAULT: i32 = 5;
/// A health pickup with more than this is catnip.
pub const CATNIP_ABOVE: i32 = 100;

/// Claw's sound when a shot hits him.
pub const SHOT_SOUND: &str = "CLAW_HIT";
/// The hit sounds, one chosen at random.
pub const HIT_SOUNDS: [&str; 4] = ["GAME_HIT1", "GAME_HIT2", "GAME_HIT3", "GAME_HIT4"];
/// Claw's pain lines, one chosen at random.
pub const PAIN_LINES: [&str; 4] = [
    "CLAW_1110048",
    "CLAW_1110049",
    "CLAW_1110050",
    "CLAW_1110051",
];
/// The end-of-level sound; the level ends when it has played.
pub const END_OF_LEVEL_SOUND: &str = "GAME_MAPPIECE";
/// Animation Claw falls with after a warp taken in a jump.
pub const FALL_ANIM: &str = "CLAW_FALL";

/// Special codes the special pickups carry.
pub mod special_code {
    /// Ends the level (the map piece).
    pub const END_OF_LEVEL: i32 = 9901;
    /// Moves Claw elsewhere in the level.
    pub const WARP: i32 = 9902;
    /// A swinging rope (no first-level source).
    pub const ROPE: i32 = 9903;
    /// Sticks of dynamite.
    pub const DYNAMITE: i32 = 9904;
    /// First curse code.
    pub const FIRST_CURSE: i32 = 9905;
    /// Last curse code.
    pub const LAST_CURSE: i32 = 9910;
    /// An extra life.
    pub const EXTRA_LIFE: i32 = 9918;
}

/// Attack kinds of the jump attacks, which survive a warp taken in a jump.
const JUMP_ATTACKS: [i32; 4] = [8, 9, 10, 18];

/// Record field holding an object's points (the pickup's amount or special
/// code).
pub const POINTS_FIELD: usize = 14;
/// Record field holding an object's smarts (a pickup's duration, amount
/// override or keep flag).
pub const SMARTS_FIELD: usize = 17;

/// What Claw reads from a pickup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PickupFields {
    /// The pickup's kind: its powerup code (33 for a coin), 0 when unknown.
    pub kind: i32,
    /// Amount (health, ammunition, score) or special code.
    pub points: i32,
    /// Duration or amount override; for a warp, non-zero keeps the warp.
    pub smarts: i32,
    /// For a sword, non-zero makes the pickup a refill that stays.
    pub damage: i32,
    /// For a warp, where it leads.
    pub target: (i32, i32),
}

impl PickupFields {
    /// The fields of pickup `o`: the kind is its id, points and smarts are
    /// its record fields, damage and target its damage and speed.
    pub fn of(o: &Object) -> PickupFields {
        let field = |i: usize| o.record.as_ref().map_or(0, |r| r[i]);
        PickupFields {
            kind: o.id,
            points: field(POINTS_FIELD),
            smarts: field(SMARTS_FIELD),
            damage: o.damage,
            target: (o.speed_x, o.speed_y),
        }
    }

    /// The fields of pickup `o` as Claw reads them: for a pickup the
    /// pickup family set up ([`pickup::data_of`]), its kind, amount and
    /// duration from that set-up (the level's own amount is ignored);
    /// otherwise [`PickupFields::of`]. Damage and the warp target always
    /// come from the object.
    pub fn of_pickup(o: &Object) -> PickupFields {
        let mut f = PickupFields::of(o);
        if let Some(d) = pickup::data_of(o) {
            f.kind = d.kind;
            f.points = d.points;
            f.smarts = d.smarts;
        }
        f
    }

    /// Writes these fields into `o`, where [`PickupFields::of`] reads
    /// them.
    pub fn store(&self, o: &mut Object) {
        o.id = self.kind;
        let r = o
            .record
            .get_or_insert_with(|| Box::new([0; claw_assets::wwd::OBJECT_INTS]));
        r[POINTS_FIELD] = self.points;
        r[SMARTS_FIELD] = self.smarts;
        o.damage = self.damage;
        o.speed_x = self.target.0;
        o.speed_y = self.target.1;
    }
}

/// The sound of a pickup of `kind`, if it has one.
pub fn pickup_sound(kind: i32) -> Option<&'static str> {
    Some(match kind {
        33 => "GAME_COIN",
        1 => "GAME_TREASURE",
        2..=5 => "GAME_RINGS",
        6 | 41..=54 | 56..=59 => "GAME_PICKUP1",
        7..=10 => "GAME_CROSS",
        11..=14 => "GAME_SCEPTER",
        15..=18 => "GAME_PICKUP2",
        19..=21 | 34 => "GAME_AMMUNITION",
        22 | 23 => "GAME_CATNMAG",
        24 | 61 => "GAME_FOODITEM",
        25..=27 => "GAME_MILK",
        28..=32 | 60 => "GAME_MAGICPOWERUP",
        35 => "GAME_CURSE3",
        36 => "GAME_CURSE2",
        37 => "GAME_CURSE6",
        38 => "GAME_CURSE5",
        39 => "GAME_CURSE4",
        40 => "GAME_CURSE1",
        55 => "GAME_EXTRALIFE",
        _ => return None,
    })
}

/// The treasure counter of a pickup of `kind`: coins, gold bars, rings,
/// chalices, crosses, scepters, geckos, crowns and skulls, in that order.
/// Necklaces and other kinds are not counted.
pub fn treasure_category(kind: i32) -> Option<usize> {
    Some(match kind {
        33 => 0,
        1 => 1,
        2..=5 => 2,
        41..=44 => 3,
        7..=10 => 4,
        11..=14 => 5,
        15..=18 => 6,
        45..=48 => 7,
        49..=52 => 8,
        _ => return None,
    })
}

/// `attacker` touched Claw.
pub fn claw_hit(c: &mut Ctx, attacker: ObjectId) {
    let Some(a) = c.objects.get(attacker) else {
        return;
    };
    let hurts = matches!(
        a.type_flags,
        types::ENEMY | types::THROWN | types::ENEMY_SHOT | types::LEVEL_SHOT
    );
    if hurts && c.aids.invincible {
        return;
    }
    match a.type_flags {
        types::ENEMY => enemy_contact(c, attacker),
        types::THROWN | types::ENEMY_SHOT | types::LEVEL_SHOT => shot(c, attacker),
        types::SPECIAL => special_pickup(c, attacker),
        types::HEALTH => health_pickup(c, attacker),
        types::SUPER_HEALTH => {
            let f = fields(c, attacker);
            c.p.stats.health = (c.p.stats.health + f.points).min(MAX_HEALTH);
            consume(c, attacker);
        }
        types::TREASURE => treasure_pickup(c, attacker),
        types::MAGIC_AMMO => {
            let f = fields(c, attacker);
            c.p.stats.magic = (c.p.stats.magic + f.points).min(MAX_AMMO);
            consume(c, attacker);
        }
        types::PISTOL_AMMO => {
            let f = fields(c, attacker);
            c.p.stats.pistol = (c.p.stats.pistol + f.points).min(MAX_AMMO);
            consume(c, attacker);
        }
        _ => {}
    }
}

fn fields(c: &Ctx, id: ObjectId) -> PickupFields {
    c.objects
        .get(id)
        .map(PickupFields::of_pickup)
        .unwrap_or_default()
}

fn damage_of(o: &Object) -> i32 {
    if o.damage == 0 { 1 } else { o.damage }
}

fn pain_sounds(c: &mut Ctx) {
    let hit = HIT_SOUNDS[c.rng.below(4) as usize];
    c.play(hit, SoundChannel::Effect);
    let line = PAIN_LINES[c.rng.below(4) as usize];
    c.play(line, SoundChannel::Voice);
}

fn enemy_contact(c: &mut Ctx, attacker: ObjectId) {
    let Some(a) = c.objects.get(attacker) else {
        return;
    };
    if a.special_flags & sflags::HARMLESS_TOUCH != 0 || c.p.special.lifted == Some(attacker) {
        return;
    }
    let damage = damage_of(a);
    c.p.stats.invulnerable_ms = INVULNERABLE_MS;
    pain_sounds(c);
    c.o.hit_type_flags &= !types::ENEMY;
    take_damage_from(c, damage, Some(attacker));
}

fn shot(c: &mut Ctx, attacker: ObjectId) {
    let Some(a) = c.objects.get(attacker) else {
        return;
    };
    let damage = damage_of(a);
    let own_impact = a.special_flags & sflags::OWN_IMPACT != 0;
    let survives = a.type_flags == types::LEVEL_SHOT;
    c.p.stats.invulnerable_ms = INVULNERABLE_MS;
    c.play(SHOT_SOUND, SoundChannel::Effect);
    pain_sounds(c);
    c.o.hit_type_flags &= !types::CLAW_SHIELDED;
    if own_impact {
        c.with_world(|w| projectile::impact(w, attacker));
    } else if !survives {
        remove(c, attacker);
    }
    take_damage_from(c, damage, Some(attacker));
}

/// The common damage step with no attacker: see [`take_damage_from`].
pub fn take_damage(c: &mut Ctx, damage: i32) {
    take_damage_from(c, damage, None);
}

/// The common damage step: `damage` comes off Claw's health, and health
/// at 0 or below kills him. Otherwise, unless he is already reeling, he
/// drops what he carries, stops his attack, starts the hit reaction away
/// from `attacker` and a hit burst shows where he was struck. Negative
/// damage does nothing, and so does any damage with the invincibility aid
/// on.
pub fn take_damage_from(c: &mut Ctx, damage: i32, attacker: Option<ObjectId>) {
    if damage < 0 || c.aids.invincible {
        return;
    }
    c.p.stats.health -= damage;
    if c.p.stats.health <= 0 {
        death::kill(c, DeathCause::Damage);
        return;
    }
    if c.o.state == state::HIT {
        return;
    }
    if let Some(lifted) = c.p.special.lifted.take() {
        c.with_world(|w| logic::dropped(w, lifted));
    }
    super::attack::cancel(c);
    let a = attacker.and_then(|id| c.objects.get(id));
    let (ax, ay, a_rect) = a.map_or((c.o.x, c.o.y, Rect::UNSET), |a| (a.x, a.y, a.attack_rect));
    let grounded = matches!(c.o.physics_type, ptype::STAND | ptype::WALK | ptype::DUCK);
    let bx = if !grounded {
        c.o.x
    } else if c.o.x > ax {
        c.o.x - 4
    } else {
        c.o.x + 4
    };
    let by = if a_rect.is_set() {
        let y = ay + (a_rect.top + a_rect.bottom) / 2;
        let h = c.o.hit_rect;
        if h.is_set() {
            y.clamp(c.o.y + h.top, (c.o.y + h.bottom).max(c.o.y + h.top))
        } else {
            y
        }
    } else {
        c.o.y
    };
    special::enter_hit(c, ax);
    // The burst's jitter: the y offset is drawn first, then the x.
    let jy = c.rng.below(9) as i32 - 4;
    let jx = c.rng.below(9) as i32 - 4;
    let (id, z) = (c.id, c.o.z + 1);
    c.with_world(|w| effect::spawn_hit_burst(w, bx + jx, by + jy, z, Some(id), true));
}

/// Asks for `id` to be removed at the end of the next update pass.
fn remove(c: &mut Ctx, id: ObjectId) {
    if let Some(o) = c.objects.get_mut(id) {
        o.flags |= flags::DESTROY;
    }
}

/// Plays the pickup's sound, unless the same sound already plays in this
/// tick.
fn pickup_sound_of(c: &mut Ctx, id: ObjectId) {
    let Some(key) = c
        .objects
        .get(id)
        .and_then(|o| pickup_sound(PickupFields::of_pickup(o).kind))
    else {
        return;
    };
    let playing = c
        .out
        .audio()
        .iter()
        .any(|cue| matches!(cue, AudioCue::Play(s) if s.key == key));
    if !playing {
        c.play(key, SoundChannel::Effect);
    }
}

/// A collected pickup: its sound, then it is removed.
fn consume(c: &mut Ctx, id: ObjectId) {
    pickup_sound_of(c, id);
    remove(c, id);
}

/// Hands a pickup to its flight to the status bar; it no longer touches
/// anything and removes itself.
fn fly_to_hud(c: &mut Ctx, id: ObjectId) {
    if let Some(o) = c.objects.get_mut(id) {
        o.special_flags |= sflags::FLYING_TO_HUD;
        o.flags |= flags::NO_COLLIDE;
    }
}

fn health_pickup(c: &mut Ctx, id: ObjectId) {
    let f = fields(c, id);
    if f.points > CATNIP_ABOVE {
        let ms = if f.smarts > 0 { f.smarts } else { f.points };
        death::set_powerup(c, powerup::CATNIP, ms.max(0) as u32);
        consume(c, id);
    } else if c.p.stats.health < MAX_HEALTH {
        c.p.stats.health = (c.p.stats.health + f.points).min(MAX_HEALTH);
        consume(c, id);
    }
}

fn treasure_pickup(c: &mut Ctx, id: ObjectId) {
    let f = fields(c, id);
    let s = &mut c.p.stats;
    s.score = s.score.saturating_add_signed(f.points);
    pickup_sound_of(c, id);
    if let Some(k) = treasure_category(f.kind) {
        let s = &mut c.p.stats;
        s.treasure_level[k] += 1;
        s.treasure_game[k] += 1;
    }
    fly_to_hud(c, id);
}

fn timed_powerup(c: &mut Ctx, id: ObjectId, kind: u32, f: &PickupFields) {
    let ms = if f.smarts > 0 {
        f.smarts as u32
    } else {
        POWERUP_DEFAULT_MS
    };
    death::set_powerup(c, kind, ms);
    consume(c, id);
}

fn sword_pickup(c: &mut Ctx, id: ObjectId, kind: u32, f: &PickupFields) {
    let ms = if f.smarts > 0 {
        f.smarts as u32
    } else {
        POWERUP_DEFAULT_MS
    };
    if f.damage == 0 || c.p.stats.powerup_kind != kind {
        death::set_powerup(c, kind, ms);
        pickup_sound_of(c, id);
    }
    if f.damage > 0 {
        c.p.stats.powerup_ms = ms;
    } else {
        remove(c, id);
    }
}

fn special_pickup(c: &mut Ctx, id: ObjectId) {
    use special_code::*;
    let f = fields(c, id);
    match f.points {
        END_OF_LEVEL => {
            let after_ms = c.level.sound_ms(END_OF_LEVEL_SOUND).unwrap_or(0);
            c.out.event(GameEvent::LevelComplete { after_ms });
            c.o.frame_delay = 2;
            c.play(END_OF_LEVEL_SOUND, SoundChannel::Effect);
            remove(c, id);
        }
        WARP => warp(c, id, &f),
        DYNAMITE => {
            let n = if f.smarts > 0 {
                f.smarts
            } else {
                DYNAMITE_DEFAULT
            };
            c.p.stats.dynamite = (c.p.stats.dynamite + n).min(MAX_AMMO);
            consume(c, id);
        }
        FIRST_CURSE..=LAST_CURSE => {
            let code = f.points;
            if c.with_world(|w| pickup::curse_effect(w, code)) {
                consume(c, id);
            } else {
                remove(c, id);
            }
        }
        p if p == powerup::GHOST as i32 => timed_powerup(c, id, powerup::GHOST, &f),
        p if p == powerup::VADER as i32 => timed_powerup(c, id, powerup::VADER, &f),
        p if p == powerup::FIRE_SWORD as i32
            || p == powerup::LIGHTNING_SWORD as i32
            || p == powerup::ICE_SWORD as i32 =>
        {
            sword_pickup(c, id, p as u32, &f);
        }
        EXTRA_LIFE => {
            if c.p.stats.lives < MAX_LIVES {
                c.p.stats.lives += 1;
                pickup_sound_of(c, id);
            }
            fly_to_hud(c, id);
        }
        // Ropes have no first-level source; the plasma sword and catnip
        // codes do nothing on a special pickup.
        _ => {}
    }
}

/// A warp: a warp taken in a jump turns the jump into a fall; with a
/// target Claw moves there at the end of the tick (see
/// [`super::special::SpecialData::warp_to`]). The warp stays when its smarts
/// are set.
fn warp(c: &mut Ctx, id: ObjectId, f: &PickupFields) {
    if c.o.state == state::JUMP {
        if c.p.attacking && JUMP_ATTACKS.contains(&c.p.attack.kind) {
            c.p.attack.restore_anim = Some(FALL_ANIM.to_string());
        } else {
            super::attack::cancel(c);
            c.set_anim(FALL_ANIM, false);
        }
        c.o.state = state::FALL;
        c.speeds_zero();
        c.o.physics_type = ptype::FALL;
        c.o.flags &= !flags::ON_ELEVATOR;
        c.set_hit_rect_for_facing();
        c.set_stand_move_rect();
        c.p.fall_height = 0;
        c.p.mover.y.v = 0.0;
        c.p.mover.y.a = c.t.player.gravity;
    }
    if f.target.0 > 0 && f.target.1 > 0 {
        // Claw stays here for the rest of the tick; the simulation moves
        // him to the target between this tick and the next.
        c.p.special.warp_to = Some(f.target);
    }
    if f.smarts == 0 {
        remove(c, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treasure_kinds_map_to_their_counters() {
        let want = [
            (33, Some(0)),
            (0, None),
            (1, Some(1)),
            (3, Some(2)),
            (6, None),
            (10, Some(4)),
            (11, Some(5)),
            (18, Some(6)),
            (19, None),
            (41, Some(3)),
            (48, Some(7)),
            (52, Some(8)),
            (53, None),
        ];
        for (kind, k) in want {
            assert_eq!(treasure_category(kind), k, "{kind}");
        }
    }

    #[test]
    fn pickup_fields_round_trip() {
        let f = PickupFields {
            kind: 22,
            points: 15000,
            smarts: 12000,
            damage: 1,
            target: (300, 400),
        };
        let mut o = crate::logic::new_object("HealthPowerup", 0, 0, 0);
        f.store(&mut o);
        assert_eq!(PickupFields::of(&o), f);
        assert_eq!(pickup_sound(22), Some("GAME_CATNMAG"));
        assert_eq!(pickup_sound(0), None);
    }
}
