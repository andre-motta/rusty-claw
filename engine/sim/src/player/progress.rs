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
//! of Claw. A route or recording of a later level names it in its
//! `# start_...` lines ([`start_lines`], [`parse_start_lines`]).
//!
//! Every [`EXTRA_LIFE_STEP`] points of score bring an extra life (one per
//! tick, up to the lives cap), with its sound.

use claw_frame::SoundChannel;
use claw_frame::Weapon;

use super::hit::MAX_LIVES;
use super::stats::{Carry, EXTRA_LIFE_STEP, SavePos, StartState, TREASURE_KINDS};
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

/// The prefix of the keys of a level's start in a recording's comment
/// lines (`# start_score = 25000`).
pub const START_PREFIX: &str = "start_";

/// The progress keys of a level's start, in the order a recording writes
/// them.
pub const CARRY_KEYS: [&str; 8] = [
    "score",
    "health",
    "lives",
    "pistol",
    "magic",
    "dynamite",
    "weapon",
    "treasure_game",
];

fn weapon_name(w: Weapon) -> &'static str {
    match w {
        Weapon::Pistol => "pistol",
        Weapon::Magic => "magic",
        Weapon::Dynamite => "dynamite",
    }
}

/// The comment lines of the progress a level starts with, as a recording
/// of a game's later level writes them: `# start_score = 25000`, ...,
/// `# start_treasure_game = 10, 2, 0, 0, 0, 0, 0, 0, 0`.
pub fn start_lines(c: &Carry) -> String {
    let counts: Vec<String> = c.treasure_game.iter().map(u32::to_string).collect();
    let values = [
        c.score.to_string(),
        c.health.to_string(),
        c.lives.to_string(),
        c.pistol.to_string(),
        c.magic.to_string(),
        c.dynamite.to_string(),
        weapon_name(c.weapon).to_string(),
        counts.join(", "),
    ];
    CARRY_KEYS
        .iter()
        .zip(values)
        .map(|(k, v)| format!("# {START_PREFIX}{k} = {v}\n"))
        .collect()
}

/// The progress a level starts with, read from the `# start_...` comment
/// lines of `text` (a route or a recording of the level, or the lines
/// alone; the last line of a key counts): `None` with no such line, an
/// error when one of [`CARRY_KEYS`] is missing or unreadable. The bot, the
/// replay tool and the comparison with reference traces start a later
/// level of one game from it, as the game's own flow does.
pub fn parse_start_lines(text: &str) -> Result<Option<Carry>, String> {
    let mut fields: Vec<(&str, &str)> = Vec::new();
    for line in text.lines() {
        let Some((k, v)) = line
            .trim()
            .strip_prefix('#')
            .and_then(|r| r.split_once('='))
        else {
            continue;
        };
        let Some(k) = k.trim().strip_prefix(START_PREFIX) else {
            continue;
        };
        if CARRY_KEYS.contains(&k) {
            fields.retain(|(f, _)| *f != k);
            fields.push((k, v.trim()));
        }
    }
    if fields.is_empty() {
        return Ok(None);
    }
    let get = |k: &str| {
        fields
            .iter()
            .find(|(f, _)| *f == k)
            .map(|(_, v)| *v)
            .ok_or_else(|| format!("{START_PREFIX}{k} is missing"))
    };
    let bad = |k: &str, v: &str| format!("bad {START_PREFIX}{k} {v:?}");
    let int = |k: &str| -> Result<i32, String> {
        let v = get(k)?;
        v.parse().map_err(|_| bad(k, v))
    };
    // In the order of the keys, so the first one missing is named.
    let score = get("score")?;
    let score = score.parse().map_err(|_| bad("score", score))?;
    let health = int("health")?;
    let lives = int("lives")?;
    let pistol = int("pistol")?;
    let magic = int("magic")?;
    let dynamite = int("dynamite")?;
    let weapon = match get("weapon")? {
        "pistol" => Weapon::Pistol,
        "magic" => Weapon::Magic,
        "dynamite" => Weapon::Dynamite,
        v => return Err(bad("weapon", v)),
    };
    let counts = get("treasure_game")?;
    let parsed: Vec<u32> = counts
        .split(',')
        .map(|n| n.trim().parse())
        .collect::<Result<_, _>>()
        .map_err(|_| bad("treasure_game", counts))?;
    let treasure_game: [u32; TREASURE_KINDS] = parsed
        .try_into()
        .map_err(|_| bad("treasure_game", counts))?;
    Ok(Some(Carry {
        score,
        health,
        lives,
        pistol,
        magic,
        dynamite,
        weapon,
        treasure_game,
    }))
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
    fn start_lines_read_back() {
        let c = Carry {
            score: 99_700,
            health: 100,
            lives: 7,
            pistol: 10,
            magic: 5,
            dynamite: 3,
            weapon: Weapon::Pistol,
            treasure_game: [87, 0, 4, 4, 3, 0, 6, 0, 0],
        };
        let text = start_lines(&c);
        assert!(text.starts_with("# start_score = 99700\n# start_health = 100\n"));
        assert_eq!(parse_start_lines(&text), Ok(Some(c)));
        assert_eq!(parse_start_lines("# level 2\n0,0x0,0x0\n"), Ok(None));
        let e = parse_start_lines("# start_score = 1\n").unwrap_err();
        assert!(e.contains("start_health is missing"), "{e}");
    }

    #[test]
    fn extra_life_thresholds() {
        assert_eq!(next_extra_life(0), 500_000);
        assert_eq!(next_extra_life(499_999), 500_000);
        assert_eq!(next_extra_life(500_000), 1_000_000);
        assert_eq!(next_extra_life(1_234_567), 1_500_000);
    }
}
