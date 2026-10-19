//! Mercat's trident bolt ([`crate::logic::enemy::mercat`]): a bolt of
//! lightning thrown straight ahead at [`SPEED`] px/s without falling. It
//! hurts Claw for [`DAMAGE`] and bursts on him into a short explosion
//! that stays where it burst for [`EXPLOSION_TICKS`] ticks and hurts the
//! human enemies in its box (his own thrower among them, when Claw stands
//! close in front of him); a wall ends it. It sheds sparkles and carries a
//! light as it flies, and goes when Claw respawns.
//!
//! It flies as the projectiles do ([`crate::logic::projectile`]): the
//! projectile family sets it up through [`set_up`] and runs each of its
//! calls through [`step`], and Claw's touch reaches [`impact`]; whatever
//! a bolt keeps beyond the projectiles' data is its [`Fields`].
//!
//! # The rules
//!
//! - Thrown ([`launch`]): an enemy shot dealing [`DAMAGE`] whatever the
//!   difficulty, which nothing can hit, with its movement and attack box
//!   from the first frame of its picture ([`FRAME_BOX`] flying left,
//!   turned to face right with its picture for a bolt flying right), a
//!   light one depth behind it, and [`SPARKLES`] sparkles shed every tick
//!   in flight. It joins the level one depth behind its thrower and moves
//!   from the next tick.
//! - In flight, every tick: gone on the tick after it touched a wall or
//!   when it is outside the level (without bursting); otherwise its
//!   sparkles, each at a random point of its box (a draw across, then one
//!   up and down), its light brought to it, and its move.
//! - On Claw: his shot rule takes its damage and calls [`impact`]
//!   instead of removing it. It bursts where it is: unhurtable, frozen in
//!   place, the level's own kind of shot with the same box and damage,
//!   playing its explosion, which lasts [`EXPLOSION_TICKS`] ticks. Claw,
//!   just hurt, is shielded from it; every human enemy, crate and keg in
//!   its box takes its hit through its own rule (a human enemy's level
//!   shot rule: the damage, a burst, a 2000 ms cooldown and a low flinch),
//!   on every tick it is there and its cooldown lets it.
//! - Claw's respawn takes it away: it makes one more call as he comes
//!   back (the first update pass of the respawn, which takes no game time,
//!   so it sheds its sparkles where it is) and is gone at its next call.
//! - Removed, its light goes with it.
//!
//! In flight it does not hurt enemies (they are not hit by enemy shots),
//! and crates it passes break and let it through.

use super::{Class, Phase, ProjectileData, ProjectileSpec, outside_level, shed_sparkles, store};
use crate::level::runtime_image_set;
use crate::logic::{ObjectData, World, effect};
use crate::object::{Object, ObjectId, Rect, draw_flags, flags, special, types};
use crate::physics::res;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["TridentBullet"];

/// The bolt's picture.
pub const IMAGE_SET: &str = "LEVEL_TRIDENT_TRIDENTPROJECTILE";
/// The explosion's pictures.
pub const EXPLOSION_SET: &str = "LEVEL_TRIDENT_TRIDENTEXPLOSION";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET, EXPLOSION_SET];

/// The explosion.
pub const EXPLOSION_ANIM: &str = "LEVEL_TRIDENTEXPLOSION";

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[EXPLOSION_ANIM];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Logic name of the bolt.
pub const LOGIC: &str = NAMES[0];

/// Damage to Claw and to the human enemies in the explosion, whatever the
/// difficulty.
pub const DAMAGE: i32 = 20;
/// Speed, px/s.
pub const SPEED: i32 = 400;
/// The bolt's box, flying left, when the level does not know its first
/// frame; turned to face right for a bolt flying right.
pub const FRAME_BOX: Rect = Rect::new(-32, -14, 31, 13);
/// Ticks of the explosion at the 18 ms step (measured).
pub const EXPLOSION_TICKS: u32 = 34;
/// Sparkles a bolt sheds per tick.
pub const SPARKLES: u32 = 2;

/// What a bolt keeps beyond the projectiles' data (see
/// [`ProjectileData::trident`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fields {
    /// The object that threw it.
    pub thrower: Option<ObjectId>,
    /// Claw's attempt number when it was thrown: once he has respawned
    /// since, it goes.
    pub attempt: u32,
    /// It has made its one call since Claw respawned.
    pub respawn_call: bool,
}

/// A bolt's data. A bolt keeps the projectiles' data
/// ([`ProjectileData`]); this is not used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TridentData {}

/// Throws a bolt for `thrower` from `at` (x, y, depth) at `vx` px/s (see
/// the module documentation); its light is made with it. Returns the
/// bolt, `None` when the level cannot make one.
pub fn launch(w: &mut World, thrower: ObjectId, at: (i32, i32, i32), vx: i32) -> Option<ObjectId> {
    let (x, y, z) = at;
    let id = super::spawn(
        w,
        ProjectileSpec {
            logic: LOGIC,
            x,
            y,
            z,
            vx,
            vy: 0,
            damage: DAMAGE,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: special::OWN_IMPACT,
            image_set: IMAGE_SET,
            owner: Some(thrower),
            fuse_ms: None,
            power: 0,
        },
    )?;
    if Class::TridentBullet.built() {
        let flare = effect::spawn_flare(w, id, x, y, z - 1);
        if let Some(o) = w.objects.get_mut(id)
            && let ObjectData::Projectile(d) = &mut o.data
        {
            d.flare = Some(flare);
            d.sparkles = SPARKLES;
            d.trident.thrower = Some(thrower);
            d.trident.attempt = w.claw.stats.attempt;
        }
    }
    Some(id)
}

/// Sets a new bolt up from `spec` (the projectiles' spawn calls it after
/// the shared set-up): an enemy shot of [`DAMAGE`] that nothing hits, its
/// own impact, told before it is removed, and turned with its boxes for a
/// bolt flying right.
pub(super) fn set_up(o: &mut Object, d: &mut ProjectileData, spec: &ProjectileSpec) {
    o.type_flags = types::ENEMY_SHOT;
    o.hit_type_flags = 0;
    o.hit_rect = Rect::UNSET;
    o.damage = DAMAGE;
    o.special_flags |= special::OWN_IMPACT;
    o.flags |= flags::NOTIFY_DESTROY;
    if spec.vx > 0 && o.draw_flags & draw_flags::MIRROR == 0 {
        o.draw_flags |= draw_flags::MIRROR;
        o.mirror_rects();
    }
    d.sparkles = 0;
}

/// One call of a bolt, flying or exploding.
pub(super) fn step(w: &mut World, id: ObjectId, mut d: ProjectileData) {
    if respawned_since(w, &d) {
        if d.trident.respawn_call {
            // Claw's respawn has taken it away.
            w.destroy(id);
            store(w, id, d);
            return;
        }
        d.trident.respawn_call = true;
    }
    if d.phase == Phase::Exploding {
        w.advance_anim(id);
        if w.objects.get(id).is_none_or(|o| o.anim.is_done()) {
            w.destroy(id);
        }
        store(w, id, d);
        return;
    }
    let gone = d.last_result & res::HIT != 0 || outside_level(w, id);
    if gone {
        // It spends this tick where the wall stopped it and is removed at
        // the end of the pass, before it can hit anything.
        w.destroy(id);
    }
    shed_sparkles(w, id, &d);
    if let Some(f) = d.flare {
        let (x, y) = (d.motion.x as i32, d.motion.y as i32);
        if let Some(fo) = w.objects.get_mut(f) {
            fo.x = x;
            fo.y = y;
        }
    }
    if !gone {
        super::motion::integrate(w, id, &mut d);
    }
    store(w, id, d);
}

/// True once Claw has respawned since the bolt was thrown: he has lost a
/// life since and is back.
fn respawned_since(w: &World, d: &ProjectileData) -> bool {
    w.claw.stats.attempt != d.trident.attempt && !w.claw.respawn_pending
}

/// The bolt touched Claw and bursts where it is (see the module
/// documentation). A bolt already bursting stays as it is.
pub(super) fn impact(w: &mut World, id: ObjectId, d: &mut ProjectileData) {
    if d.phase == Phase::Exploding {
        return;
    }
    d.phase = Phase::Exploding;
    d.frozen = true;
    let first = w.image_frames(EXPLOSION_SET).map(|(f, _)| f);
    if let Some(o) = w.objects.get_mut(id) {
        o.type_flags = types::LEVEL_SHOT;
        o.flags |= flags::SAFE;
        o.image_set = runtime_image_set(EXPLOSION_SET);
        if let Some(f) = first {
            o.frame = f;
        }
    }
    w.set_anim(id, EXPLOSION_ANIM);
}

/// One logic call: the projectiles' ([`super::tick`]).
pub fn tick(w: &mut World, id: ObjectId) {
    super::tick(w, id);
}

/// `attacker` hit `id`. Nothing hits a bolt.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. The victim's own hit handler decides.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::enemy::mercat;

    #[test]
    fn the_bolt_is_mercats() {
        assert_eq!(SPEED, mercat::BOLT_SPEED);
        assert_eq!(DAMAGE, mercat::BOLT_DAMAGE);
        // 34 explosion ticks outlast no hazard window of Claw's: his 850 ms
        // after the bolt's hit cover them.
        const { assert!(EXPLOSION_TICKS * 18 < 850) };
        assert_eq!(FRAME_BOX.right - FRAME_BOX.left, 63);
        // Flying right its box is (-31, -14, 32, 13).
        assert_eq!(FRAME_BOX.mirrored(), Rect::new(-31, -14, 32, 13));
        assert_eq!(LOGIC, "TridentBullet");
    }
}
