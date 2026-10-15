//! Starting a level from carried progress or a save, and handing the
//! progress on to the next level.
//!
//! For now [`apply_start`] makes a save point Claw's checkpoint and puts
//! him there, then copies the carried values into his stats; the level's
//! own treasure counts start at zero.

use super::stats::{Carry, StartState};
use super::{Claw, Ctx, respawn};

/// Applies `start` to Claw at the start of a level.
pub fn apply_start(c: &mut Ctx, start: &StartState) {
    if let Some(save) = start.save
        && save.x > 0
        && save.y > 0
    {
        c.p.checkpoint = Some((save.x, save.y));
        respawn(c);
    }
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
