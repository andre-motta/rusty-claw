//! The macro-actions the search plays: short runs of held buttons, each a
//! few ticks long.
//!
//! Every action is a list of segments, each one mask held for at least
//! [`MIN_SPACING`] ticks, so any chain of actions changes the held mask at
//! most every [`MIN_SPACING`] ticks: a display at 60 Hz or slower never
//! drops a press.
//!
//! A boss level adds two kinds of macro ([`choices_in`]):
//!
//! - the fight's, offered while a boss is in play: a short wait to time a
//!   blow against the boss's own attacks, a blow struck after turning to
//!   either side, and jumps to leap clear (the retreat) or over him; the
//!   walks are the approach;
//! - the door's, offered on the ground near the boss room's door: walk
//!   toward the door for a few ticks, then jump on through it toward the
//!   boss's side of the room, so that one of them jumps on Claw's first
//!   tick in the room (the jump-in entry, which skips the boss's
//!   introduction, toward the boss as the original's players make it). A
//!   door overhead is jumped up
//!   into instead: jump held for a few ticks, let go for a few, then a
//!   fresh jump toward the boss, so that one of them presses jump on the
//!   first tick in the room.
//!
//! Held in a bear's hug, Claw cannot move, but each new press of a key
//! counts towards his escape: the mash ([`Action::Mash`]) presses Up and
//! Down in turn, a new key every [`MIN_SPACING`] ticks.

use claw_sim::player::state;
use claw_sim::{Buttons, Sim};

/// Fewest ticks between two changes of the held mask in a route.
pub const MIN_SPACING: usize = 2;

/// Length of the basic actions (walk, wait, climb), ticks.
pub const STEP: usize = 6;

/// Jump.
pub const JUMP: u32 = Buttons::JUMP.0;
/// Attack.
pub const ATTACK: u32 = Buttons::ATTACK.0;
/// Fire the selected weapon.
pub const FIRE: u32 = Buttons::PROJECTILE.0;
/// Left.
pub const LEFT: u32 = Buttons::LEFT.0;
/// Right.
pub const RIGHT: u32 = Buttons::RIGHT.0;
/// Up.
pub const UP: u32 = Buttons::UP.0;
/// Down.
pub const DOWN: u32 = Buttons::DOWN.0;

/// Ticks the jump button is held for, from a tap to a full jump, at the
/// original speeds; see [`jump_holds`] for the game's own tuning.
pub const JUMP_HOLDS: [usize; 4] = [2, 5, 9, 14];

/// Ticks the jump button is held for in `sim`, from a tap to a full jump:
/// [`JUMP_HOLDS`], with the full jump held for as many ticks as the
/// tuning's hold window takes to credit in full (14 at the original
/// speeds, 15 with the default tuning, which credits 16.7 ms a tick).
pub fn jump_holds(sim: &Sim) -> [usize; 4] {
    let p = &sim.config().tuning.player;
    let full = if p.jump_hold_per_tick_ms > 0.0 {
        (p.jump_hold_window_ms / p.jump_hold_per_tick_ms).ceil() as usize
    } else {
        JUMP_HOLDS[3]
    };
    let mut holds = JUMP_HOLDS;
    holds[3] = full.clamp(JUMP_HOLDS[2] + 1, usize::from(u8::MAX));
    holds
}

/// Presses of one mash: Up and Down in turn, each held [`MIN_SPACING`]
/// ticks.
pub const MASH_PRESSES: usize = 6;

/// Length of the fight's short wait, ticks.
pub const SHORT: usize = 3;

/// Ticks the jump of a door entry holds jump (with the direction).
pub const ENTRY_JUMP: usize = 12;

/// Ticks a door entry walks before it jumps: one entry each.
pub const ENTRY_WALKS: std::ops::RangeInclusive<u8> = 2..=8;

/// How near the door, across, Claw must stand for the door entries, px.
pub const DOOR_REACH: i32 = 240;

/// Ticks a jump up into a door overhead is held: one entry each.
pub const RISE_HOLDS: std::ops::RangeInclusive<u8> = 4..=8;

/// Ticks the jump is let go before the fresh press of an entry through a
/// door overhead: one entry each.
pub const RISE_WAITS: std::ops::RangeInclusive<u8> = 2..=3;

/// How near a door overhead Claw must stand across for its entries, px.
pub const RISE_REACH: i32 = 48;

/// How far above Claw's position a door counts as overhead, px: higher
/// than he walks into, within a jump.
pub const RISE_HEIGHT: std::ops::RangeInclusive<i32> = 60..=240;

/// One macro-action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    /// Hold nothing.
    Wait,
    /// Walk (and, kept up, run) left.
    Left,
    /// Walk (and, kept up, run) right.
    Right,
    /// Jump, holding the button for `hold` ticks, steering `dir` (-1, 0
    /// or 1) all along, and with `up` holding Up to catch a ladder.
    Jump {
        /// Steering: -1 left, 0 none, 1 right.
        dir: i8,
        /// Ticks the jump button is held.
        hold: u8,
        /// Up is held too.
        up: bool,
    },
    /// Strike once with the sword.
    Attack,
    /// Crouch and strike low.
    LowAttack,
    /// Fire the selected weapon once.
    Fire,
    /// Crouch (and step down onto a ladder below).
    Crouch,
    /// Climb up (and step onto a ladder above).
    Up,
    /// Climb down.
    Down,
    /// Hold Up while steering `dir` (-1 left, 1 right): in the air, keep
    /// going that way and catch the ladder ahead.
    Catch {
        /// Steering: -1 left, 1 right.
        dir: i8,
    },
    /// Hold nothing for a moment ([`SHORT`] ticks), to time a blow.
    Short,
    /// Held in a bear's hug: press Up and Down in turn,
    /// [`MASH_PRESSES`] presses each held [`MIN_SPACING`] ticks, so that
    /// every change of the held mask is a new press towards the escape.
    Mash,
    /// Turn to `dir` (-1 left, 1 right) and strike once with the sword.
    Strike {
        /// The side to strike: -1 left, 1 right.
        dir: i8,
    },
    /// Walk `dir` for `walk` ticks, then jump toward `jump` (jump held
    /// [`ENTRY_JUMP`] ticks): through a door, a jump on the first tick
    /// beyond it.
    Entry {
        /// Direction of the walk: -1 left, 1 right.
        dir: i8,
        /// Ticks walked before the jump.
        walk: u8,
        /// Direction of the jump: -1 left, 1 right.
        jump: i8,
    },
    /// Jump up holding the button for `hold` ticks, let go for `wait`,
    /// then jump toward `dir` (held [`ENTRY_JUMP`] ticks): into a door
    /// overhead, a fresh jump on the first tick beyond it.
    RiseEntry {
        /// Direction of the second jump: -1 left, 1 right.
        dir: i8,
        /// Ticks the first jump is held.
        hold: u8,
        /// Ticks with jump let go before the second.
        wait: u8,
    },
}

impl Action {
    /// The held masks of the action, one per tick.
    pub fn masks(self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut seg = |mask: u32, n: usize| out.extend(std::iter::repeat_n(mask, n));
        match self {
            Action::Wait => seg(0, STEP),
            Action::Left => seg(LEFT, STEP),
            Action::Right => seg(RIGHT, STEP),
            Action::Jump { dir, hold, up } => {
                let d = dir_mask(dir) | if up { UP } else { 0 };
                seg(JUMP | d, usize::from(hold));
                seg(d, MIN_SPACING);
            }
            Action::Attack => {
                seg(ATTACK, MIN_SPACING);
                seg(0, STEP);
            }
            Action::LowAttack => {
                seg(DOWN | ATTACK, MIN_SPACING);
                seg(DOWN, STEP);
            }
            Action::Fire => {
                seg(FIRE, MIN_SPACING);
                seg(0, STEP);
            }
            Action::Crouch => seg(DOWN, STEP),
            Action::Up => seg(UP, STEP),
            Action::Down => seg(DOWN, STEP),
            Action::Catch { dir } => seg(UP | dir_mask(dir), STEP),
            Action::Short => seg(0, SHORT),
            Action::Mash => {
                for i in 0..MASH_PRESSES {
                    seg(if i % 2 == 0 { UP } else { DOWN }, MIN_SPACING);
                }
            }
            Action::Strike { dir } => {
                seg(dir_mask(dir), MIN_SPACING);
                seg(ATTACK, MIN_SPACING);
                seg(0, STEP - MIN_SPACING);
            }
            Action::Entry { dir, walk, jump } => {
                seg(dir_mask(dir), usize::from(walk).max(MIN_SPACING));
                let j = dir_mask(jump);
                seg(j | JUMP, ENTRY_JUMP);
                seg(j, MIN_SPACING);
            }
            Action::RiseEntry { dir, hold, wait } => {
                let d = dir_mask(dir);
                seg(JUMP, usize::from(hold).max(MIN_SPACING));
                seg(0, usize::from(wait).max(MIN_SPACING));
                seg(d | JUMP, ENTRY_JUMP);
                seg(d, MIN_SPACING);
            }
        }
        out
    }

    /// A short name for logs.
    pub fn name(self) -> String {
        match self {
            Action::Jump { dir, hold, up } => {
                format!("jump{dir:+}x{hold}{}", if up { "up" } else { "" })
            }
            Action::Strike { dir } => format!("strike{dir:+}"),
            Action::Catch { dir } => format!("catch{dir:+}"),
            Action::Entry { dir, walk, jump } => format!("entry{dir:+}x{walk}j{jump:+}"),
            Action::RiseEntry { dir, hold, wait } => format!("rise{dir:+}x{hold}w{wait}"),
            other => format!("{other:?}").to_lowercase(),
        }
    }
}

/// The steering mask of a direction.
pub fn dir_mask(dir: i8) -> u32 {
    match dir.signum() {
        -1 => LEFT,
        1 => RIGHT,
        _ => 0,
    }
}

/// Where Claw is, for choosing actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// On his feet.
    Ground,
    /// Jumping or falling.
    Air,
    /// On a ladder.
    Climb,
    /// Hanging from a swinging rope.
    Swing,
    /// Busy: struck, attacking, lifting or in another state that takes no
    /// steering.
    Busy,
}

/// Claw's phase in `sim`.
pub fn phase(sim: &Sim) -> Phase {
    let Some(o) = sim.player() else {
        return Phase::Busy;
    };
    if sim.claw().attacking {
        return Phase::Busy;
    }
    match o.state {
        state::JUMP | state::FALL => Phase::Air,
        state::CLIMB => Phase::Climb,
        state::SWING => Phase::Swing,
        state::STAND
        | state::MOVE_LEFT
        | state::MOVE_RIGHT
        | state::LAND
        | state::CROUCH
        | state::LOOK_UP
        | state::ON_EDGE => Phase::Ground,
        _ => Phase::Busy,
    }
}

/// What the actions of a boss level depend on besides the game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Arena {
    /// The boss room's door, when the level has one.
    pub door: Option<(i32, i32)>,
    /// The side of the boss room's entry the boss is on: -1 left, 1 right
    /// (0 when unknown, taken as right).
    pub boss_side: i8,
}

/// The actions worth trying in `sim`, in a fixed order: [`choices`], with
/// the fight's macros instead while a boss is in play and the door
/// entries near the door (see the module documentation).
pub fn choices_in(sim: &Sim, arena: &Arena) -> Vec<Action> {
    let boss = sim.boss();
    if boss.current().is_some() && boss.defeated().is_none() {
        return fight_choices(sim);
    }
    let mut out = choices(sim);
    if let (Some((dx, dy)), Some(o), Phase::Ground) = (arena.door, sim.player(), phase(sim)) {
        let across = dx - o.x;
        if across.abs() <= DOOR_REACH && (dy - o.y).abs() <= DOOR_REACH {
            let dir = if across >= 0 { 1 } else { -1 };
            let jump = if arena.boss_side == 0 {
                dir
            } else {
                arena.boss_side
            };
            out.extend(ENTRY_WALKS.map(|walk| Action::Entry { dir, walk, jump }));
        }
        if across.abs() <= RISE_REACH && RISE_HEIGHT.contains(&(o.y - dy)) {
            let dir = if arena.boss_side < 0 { -1 } else { 1 };
            for hold in RISE_HOLDS {
                for wait in RISE_WAITS {
                    out.push(Action::RiseEntry { dir, hold, wait });
                }
            }
        }
    }
    out
}

/// The actions of a boss fight: the walks (the approach), the waits, the
/// blows, and jumps to leap clear or over.
fn fight_choices(sim: &Sim) -> Vec<Action> {
    let holds = jump_holds(sim);
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    match phase(sim) {
        Phase::Ground => {
            out.extend([
                Action::Short,
                Action::Attack,
                Action::Strike { dir: -1 },
                Action::Strike { dir: 1 },
            ]);
            for dir in [1i8, -1] {
                for hold in [holds[1], holds[3]] {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
            out.push(Action::Jump {
                dir: 0,
                hold: holds[3] as u8,
                up: false,
            });
        }
        Phase::Air => out.push(Action::Attack),
        Phase::Climb | Phase::Swing | Phase::Busy => {}
    }
    out
}

/// The actions worth trying in `sim`, in a fixed order.
pub fn choices(sim: &Sim) -> Vec<Action> {
    let holds = jump_holds(sim);
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    match phase(sim) {
        Phase::Ground => {
            for dir in [1i8, -1, 0] {
                for hold in holds {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
            // Jumps that catch a ladder overhead.
            for dir in [0i8, 1, -1] {
                out.push(Action::Jump {
                    dir,
                    hold: holds[3] as u8,
                    up: true,
                });
            }
            out.extend([
                Action::Attack,
                Action::LowAttack,
                Action::Crouch,
                Action::Up,
            ]);
            let stats = sim.stats();
            if stats.pistol > 0 {
                out.push(Action::Fire);
            }
        }
        Phase::Air => out.extend([
            Action::Up,
            Action::Catch { dir: 1 },
            Action::Catch { dir: -1 },
            Action::Attack,
        ]),
        Phase::Climb => {
            out.extend([Action::Up, Action::Down]);
            for dir in [1i8, -1] {
                for hold in [holds[1], holds[3]] {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
        }
        Phase::Swing => {
            // Let go at the right moment: a short wait times the release,
            // and the jump that lets go steers either way or neither.
            out.push(Action::Short);
            for dir in [1i8, -1, 0] {
                for hold in [holds[1], holds[3]] {
                    out.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up: false,
                    });
                }
            }
        }
        Phase::Busy => {
            if held(sim) {
                out.push(Action::Mash);
            }
        }
    }
    out
}

/// True while a bear holds Claw in its hug.
pub fn held(sim: &Sim) -> bool {
    sim.player().is_some_and(|o| o.state == state::BEARHUG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_keeps_each_mask_for_at_least_the_spacing() {
        let mut all = vec![
            Action::Wait,
            Action::Left,
            Action::Right,
            Action::Attack,
            Action::LowAttack,
            Action::Fire,
            Action::Crouch,
            Action::Up,
            Action::Down,
            Action::Short,
            Action::Mash,
            Action::Strike { dir: -1 },
            Action::Strike { dir: 1 },
            Action::Catch { dir: -1 },
            Action::Catch { dir: 1 },
        ];
        for dir in [-1i8, 1] {
            for walk in ENTRY_WALKS {
                for jump in [-1i8, 1] {
                    all.push(Action::Entry { dir, walk, jump });
                }
            }
            for hold in RISE_HOLDS {
                for wait in RISE_WAITS {
                    all.push(Action::RiseEntry { dir, hold, wait });
                }
            }
        }
        for dir in [-1, 0, 1] {
            for hold in JUMP_HOLDS {
                for up in [false, true] {
                    all.push(Action::Jump {
                        dir,
                        hold: hold as u8,
                        up,
                    });
                }
            }
        }
        for a in all {
            let m = a.masks();
            let mut run = 1;
            for w in m.windows(2) {
                if w[0] == w[1] {
                    run += 1;
                } else {
                    assert!(run >= MIN_SPACING, "{a:?}");
                    run = 1;
                }
            }
            assert!(run >= MIN_SPACING, "{a:?}");
            // The jump button is never held at the end of an action, so
            // the next jump is a fresh press.
            assert_eq!(m.last().unwrap() & JUMP, 0, "{a:?}");
        }
    }

    #[test]
    fn a_mash_presses_up_and_down_in_turn_every_spacing() {
        let m = Action::Mash.masks();
        assert_eq!(m.len(), MASH_PRESSES * MIN_SPACING);
        assert_eq!(&m[..2 * MIN_SPACING], &[UP, UP, DOWN, DOWN]);
        // Every change of the held mask is a new press of a key.
        let presses = m
            .windows(2)
            .filter(|w| w[0] != w[1] && w[1] & !w[0] != 0)
            .count();
        assert_eq!(presses, MASH_PRESSES - 1);
        assert_eq!(Action::Mash.name(), "mash");
    }

    #[test]
    fn a_door_entry_walks_toward_the_door_and_jumps_toward_the_boss() {
        let m = Action::Entry {
            dir: -1,
            walk: 3,
            jump: 1,
        }
        .masks();
        let mut want = vec![LEFT; 3];
        want.extend([RIGHT | JUMP; ENTRY_JUMP]);
        want.extend([RIGHT; MIN_SPACING]);
        assert_eq!(m, want);
    }
}
