//! Claw's progress across levels and saves, and extra lives from score.
//!
//! A level starts from a [`StartState`]:
//!
//! - with nothing, it is a new game: 100 health, no score, 7 lives, 10
//!   pistol shots, 5 magic charges, 3 sticks of dynamite and the pistol;
//! - with carried progress, Claw keeps the score, health, lives,
//!   ammunition, weapon and the game's treasure counts of the previous
//!   level; the level's own treasure counts start at zero and no powerup
//!   carries over;
//! - with a save point, Claw starts where the save was made, that point is
//!   his checkpoint, and the save point's flag is gone from the level.
//!
//! [`carry`] is what a level hands to the next one, and what a save keeps
//! of Claw.
//!
//! Every [`EXTRA_LIFE_STEP`] points of score bring an extra life (one per
//! tick, up to the lives cap), with its sound.

use claw_frame::SoundChannel;

use super::hit::MAX_LIVES;
use super::stats::{Carry, EXTRA_LIFE_STEP, SavePos, StartState};
use super::{Claw, Ctx, respawn};
use crate::object::{ObjectId, draw_flags, flags};

/// Sound of an extra life from score.
pub const EXTRA_LIFE_SOUND: &str = "GAME_EXTRALIFE";

/// Logic names of the save points, by slot (1 and 2).
pub const SAVE_POINT_LOGICS: [&str; 2] = ["FirstSuperCheckpoint", "SecondSuperCheckpoint"];

/// The score at which the extra life after `score` comes.
pub const fn next_extra_life(score: u32) -> u32 {
    (score / EXTRA_LIFE_STEP + 1).saturating_mul(EXTRA_LIFE_STEP)
}

/// Applies `start` to Claw at the start of a level.
pub fn apply_start(c: &mut Ctx, start: &StartState) {
    if let Some(carry) = &start.carry {
        let s = &mut c.p.stats;
        s.score = carry.score;
        s.health = carry.health;
        s.lives = carry.lives;
        s.pistol = carry.pistol;
        s.magic = carry.magic;
        s.dynamite = carry.dynamite;
        s.weapon = carry.weapon;
        s.treasure_game = carry.treasure_game;
    }
    let s = &mut c.p.stats;
    s.treasure_level = Default::default();
    s.powerup_kind = 0;
    s.powerup_ms = 0;
    s.invulnerable_ms = 0;
    s.next_extra_life = next_extra_life(s.score);
    if let Some(save) = start.save
        && save.x > 0
        && save.y > 0
    {
        start_at_save(c, save);
    }
}

/// Puts Claw at the save point, makes it his checkpoint and his fallback
/// respawn point, and removes the save point's flag.
fn start_at_save(c: &mut Ctx, save: SavePos) {
    let health = c.p.stats.health;
    c.p.checkpoint = Some((save.x, save.y));
    c.p.spawn_point = (save.x, save.y);
    respawn(c);
    c.p.spawn_point = (c.o.x, c.o.y);
    c.p.stats.health = health;
    let Some(logic) = (save.slot as usize)
        .checked_sub(1)
        .and_then(|i| SAVE_POINT_LOGICS.get(i))
    else {
        return;
    };
    let flag: Option<ObjectId> = c
        .objects
        .all()
        .filter(|(_, o)| &*o.logic == *logic)
        .min_by_key(|(_, o)| {
            let dx = i64::from(o.x - save.x);
            let dy = i64::from(o.y - save.y);
            dx * dx + dy * dy
        })
        .map(|(id, _)| id);
    if let Some(o) = flag.and_then(|id| c.objects.get_mut(id)) {
        o.flags |= flags::DESTROY | flags::NO_COLLIDE;
        o.draw_flags |= draw_flags::HIDDEN;
    }
}

/// What Claw hands to the next level.
pub fn carry(claw: &Claw) -> Carry {
    let s = &claw.stats;
    Carry {
        score: s.score,
        health: s.health,
        lives: s.lives,
        pistol: s.pistol,
        magic: s.magic,
        dynamite: s.dynamite,
        weapon: s.weapon,
        treasure_game: s.treasure_game,
    }
}

/// Awards the extra life the score has reached, if any: one per tick, up
/// to the lives cap; the next one comes [`EXTRA_LIFE_STEP`] points later
/// either way.
pub fn extra_life_tick(c: &mut Ctx) {
    let s = &mut c.p.stats;
    if s.score < s.next_extra_life {
        return;
    }
    s.next_extra_life = s.next_extra_life.saturating_add(EXTRA_LIFE_STEP);
    if s.lives < MAX_LIVES {
        s.lives += 1;
        c.play(EXTRA_LIFE_SOUND, SoundChannel::Effect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_life_thresholds() {
        assert_eq!(next_extra_life(0), 500_000);
        assert_eq!(next_extra_life(499_999), 500_000);
        assert_eq!(next_extra_life(500_000), 1_000_000);
        assert_eq!(next_extra_life(1_234_567), 1_500_000);
    }
}
