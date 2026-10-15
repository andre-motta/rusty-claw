//! Claw's progress: health, score, lives, ammunition, the selected weapon,
//! the running powerup and the treasure counts, and what of it moves from
//! one level to the next.
//!
//! A new game starts with 100 health, no score, 7 lives, 10 pistol shots,
//! 5 magic charges, 3 sticks of dynamite and the pistol selected; the
//! first extra life from score comes at 500000 points. Health, score,
//! lives, ammunition, the weapon and the game-wide treasure counts carry
//! from a level to the next; the level's treasure counts, the powerup and
//! the invulnerability window do not.

use claw_frame::Weapon;

/// Number of treasure kinds counted: coins, gold bars, rings, chalices,
/// crosses, scepters, geckos, crowns and skulls, in that order.
pub const TREASURE_KINDS: usize = 9;

/// Health at the start of a new game and after a death.
pub const NEW_GAME_HEALTH: i32 = 100;
/// Lives at the start of a new game.
pub const NEW_GAME_LIVES: i32 = 7;
/// Pistol shots at the start of a new game.
pub const NEW_GAME_PISTOL: i32 = 10;
/// Magic charges at the start of a new game.
pub const NEW_GAME_MAGIC: i32 = 5;
/// Sticks of dynamite at the start of a new game.
pub const NEW_GAME_DYNAMITE: i32 = 3;
/// Score step between two extra lives.
pub const EXTRA_LIFE_STEP: u32 = 500_000;

/// Claw's progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStats {
    /// Health; 0 or less is death.
    pub health: i32,
    /// Score.
    pub score: u32,
    /// Lives left.
    pub lives: i32,
    /// Pistol shots.
    pub pistol: i32,
    /// Magic charges.
    pub magic: i32,
    /// Sticks of dynamite.
    pub dynamite: i32,
    /// Selected projectile weapon.
    pub weapon: Weapon,
    /// Running powerup by the game's powerup code, 0 for none.
    pub powerup_kind: u32,
    /// Time left of the running powerup, ms.
    pub powerup_ms: u32,
    /// Time left of the window after a hit in which enemies cannot hurt
    /// Claw, ms.
    pub invulnerable_ms: i32,
    /// Treasures collected in this level, per kind.
    pub treasure_level: [u32; TREASURE_KINDS],
    /// Treasures collected in this game, per kind.
    pub treasure_game: [u32; TREASURE_KINDS],
    /// Score at which the next extra life is awarded.
    pub next_extra_life: u32,
    /// A cheat was used in this game.
    pub cheated: bool,
}

impl PlayerStats {
    /// The values of a new game.
    pub fn new_game() -> PlayerStats {
        PlayerStats {
            health: NEW_GAME_HEALTH,
            score: 0,
            lives: NEW_GAME_LIVES,
            pistol: NEW_GAME_PISTOL,
            magic: NEW_GAME_MAGIC,
            dynamite: NEW_GAME_DYNAMITE,
            weapon: Weapon::Pistol,
            powerup_kind: 0,
            powerup_ms: 0,
            invulnerable_ms: 0,
            treasure_level: [0; TREASURE_KINDS],
            treasure_game: [0; TREASURE_KINDS],
            next_extra_life: EXTRA_LIFE_STEP,
            cheated: false,
        }
    }

    /// Ammunition of `weapon`.
    pub fn ammo(&self, weapon: Weapon) -> i32 {
        match weapon {
            Weapon::Pistol => self.pistol,
            Weapon::Magic => self.magic,
            Weapon::Dynamite => self.dynamite,
        }
    }
}

impl Default for PlayerStats {
    /// The values of a new game.
    fn default() -> Self {
        PlayerStats::new_game()
    }
}

/// The weapon after `w` in the toggle order pistol, magic, dynamite.
pub fn next_weapon(w: Weapon) -> Weapon {
    match w {
        Weapon::Pistol => Weapon::Magic,
        Weapon::Magic => Weapon::Dynamite,
        Weapon::Dynamite => Weapon::Pistol,
    }
}

/// What a level hands to the next one, and what a save holds of Claw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carry {
    /// Score.
    pub score: u32,
    /// Health.
    pub health: i32,
    /// Lives.
    pub lives: i32,
    /// Pistol shots.
    pub pistol: i32,
    /// Magic charges.
    pub magic: i32,
    /// Sticks of dynamite.
    pub dynamite: i32,
    /// Selected weapon.
    pub weapon: Weapon,
    /// Treasures collected in this game, per kind.
    pub treasure_game: [u32; TREASURE_KINDS],
}

impl Default for Carry {
    /// What a new game starts with.
    fn default() -> Self {
        let s = PlayerStats::new_game();
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
}

/// A save point a level is started from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SavePos {
    /// Save slot of the save point (1 or 2).
    pub slot: u8,
    /// Where Claw starts.
    pub x: i32,
    /// Where Claw starts.
    pub y: i32,
}

/// How a level starts: a new game without either part, the progress of
/// the previous level with `carry`, a loaded save with both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartState {
    /// Progress carried in.
    pub carry: Option<Carry>,
    /// Save point to start at.
    pub save: Option<SavePos>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_values() {
        let s = PlayerStats::default();
        assert_eq!(s, PlayerStats::new_game());
        assert_eq!((s.health, s.score, s.lives), (100, 0, 7));
        assert_eq!((s.pistol, s.magic, s.dynamite), (10, 5, 3));
        assert_eq!(s.weapon, Weapon::Pistol);
        assert_eq!(s.next_extra_life, 500_000);
        assert_eq!((s.powerup_kind, s.powerup_ms, s.invulnerable_ms), (0, 0, 0));
        assert_eq!(s.treasure_level, [0; 9]);
        assert_eq!(s.treasure_game, [0; 9]);
        assert!(!s.cheated);
        let c = Carry::default();
        assert_eq!((c.score, c.health, c.lives), (0, 100, 7));
        assert_eq!((c.pistol, c.magic, c.dynamite), (10, 5, 3));
        assert_eq!(StartState::default().carry, None);
        assert_eq!(StartState::default().save, None);
    }

    #[test]
    fn weapons_cycle_in_order() {
        let mut w = Weapon::Pistol;
        let mut seen = Vec::new();
        for _ in 0..4 {
            w = next_weapon(w);
            seen.push(w);
        }
        assert_eq!(
            seen,
            vec![
                Weapon::Magic,
                Weapon::Dynamite,
                Weapon::Pistol,
                Weapon::Magic
            ]
        );
    }
}
