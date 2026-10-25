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
//!   walks are the approach (once the boss is beaten and on his way to his
//!   gem, only the wait: nothing Claw does hurries his death);
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
//! In Omar's fight Claw goes about the arena (to the swords, down to the
//! ledges, up to the hall), so the fight's macros are every move of the
//! level, with the short wait and the blows turned either way.
//!
//! In Marrow's fight, while the pit is open between Claw and Marrow and
//! his parrot patrols the top of the room or sits on its perch, Claw can
//! only wait for its dive: on his feet the fight's only macro is then
//! the wait ([`parrot_patrols`]), so the search does not try every walk,
//! blow and jump about the ledge at every moment of the patrol (he walks
//! to where he waits while the parrot carries Marrow).
//!
//! Held in a bear's hug, Claw cannot move, but each new press of a key
//! counts towards his escape: the mash ([`Action::Mash`]) presses Up and
//! Down in turn, a new key every [`MIN_SPACING`] ticks. A tentacle's
//! squeeze beside Aquatis holds him the same way, inside the boss fight,
//! so the fight's macros offer the mash too.
//!
//! A boss the search drowns adds the lift to the fight's macros: on his
//! feet near the boss, Claw presses Lift ([`Action::Lift`]); carrying
//! him ([`Phase::Carry`]), he walks either way with him or throws him
//! ([`Action::Throw`]), turned either way, the key held from a tap to the
//! full charge ([`THROW_HOLDS`]).

use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::object::{Object, special};
use claw_sim::player::attack::LIFT_FEET;
use claw_sim::player::special::THROW_CHARGE_MS;
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
/// Lift, and throw what Claw carries.
pub const LIFT: u32 = Buttons::LIFT.0;

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

/// Ticks the key of a throw is held, from a tap to the full charge (3 s
/// at the original speeds, the most a throw charges): one throw each way.
/// See [`throw_holds`] for the game's own tuning.
pub const THROW_HOLDS: [u8; 4] = [2, 56, 112, 170];

/// Ticks past the full charge the longest throw holds its key, so a tick
/// lost never shortens it.
pub const THROW_SPARE: usize = 3;

/// Ticks the key of a throw is held in `sim`: [`THROW_HOLDS`], with the
/// longest held for as many ticks as the tuning takes to charge a throw
/// in full, and [`THROW_SPARE`] more (170 at the original speeds, 183
/// with the default tuning, which counts 16.7 ms a tick).
pub fn throw_holds(sim: &Sim) -> [u8; 4] {
    let per_tick = sim.config().tuning.player.jump_hold_per_tick_ms;
    let mut holds = THROW_HOLDS;
    if per_tick > 0.0 {
        let full = (f64::from(THROW_CHARGE_MS) / per_tick).ceil() as usize + THROW_SPARE;
        holds[3] = full.clamp(usize::from(THROW_HOLDS[2]) + 1, usize::from(u8::MAX)) as u8;
    }
    holds
}

/// Ticks a throw waits after the key is let go: the throw's flight, so
/// that where it ends (a landing, or the water) is the throw's outcome.
pub const THROW_FOLLOW: usize = 48;

/// How near the boss, across, Claw must stand for the lift to be offered,
/// px (the lift itself reaches 64 px).
pub const LIFT_REACH: i32 = 96;

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
    /// Press Lift: pick up what is in reach.
    Lift,
    /// Carrying, turn to `dir` (-1 left, 1 right) and throw, the key held
    /// `hold` ticks to charge the throw, then wait [`THROW_FOLLOW`] ticks
    /// while it flies.
    Throw {
        /// The side to throw to: -1 left, 1 right.
        dir: i8,
        /// Ticks the key is held.
        hold: u8,
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
            Action::Lift => {
                seg(LIFT, MIN_SPACING);
                seg(0, STEP);
            }
            Action::Throw { dir, hold } => {
                seg(dir_mask(dir) | LIFT, usize::from(hold).max(MIN_SPACING));
                seg(0, THROW_FOLLOW);
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
            Action::Throw { dir, hold } => format!("throw{dir:+}x{hold}"),
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
    /// Carrying what he lifted: he walks with it or throws it.
    Carry,
    /// Busy: struck, attacking, lifting or in another state that takes no
    /// steering.
    Busy,
}

/// Claw's phase in `sim`.
pub fn phase(sim: &Sim) -> Phase {
    let Some(o) = sim.player() else {
        return Phase::Busy;
    };
    let claw = sim.claw();
    if claw.attacking {
        return Phase::Busy;
    }
    match o.state {
        state::LIFT if claw.special.carrying && claw.special.lifted.is_some() => Phase::Carry,
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
        // Beaten and on his way to his gem, the boss needs nothing more
        // from Claw, and nothing Claw does hurries his death.
        if crate::search::boss_beaten(sim) == Some(true) {
            return vec![Action::Wait];
        }
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
/// blows, and jumps to leap clear or over. Omar's fight takes Claw about
/// the arena: every move of the level, with the short wait and the blows
/// turned either way on his feet.
fn fight_choices(sim: &Sim) -> Vec<Action> {
    if crate::search::omar_in_play(sim) {
        let mut out = choices(sim);
        if phase(sim) == Phase::Ground && !locked(sim) {
            out.extend([
                Action::Short,
                Action::Strike { dir: -1 },
                Action::Strike { dir: 1 },
            ]);
        }
        return out;
    }
    let holds = jump_holds(sim);
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    match phase(sim) {
        Phase::Ground if parrot_patrols(sim) => out.truncate(1),
        Phase::Ground => {
            out.extend([
                Action::Short,
                Action::Attack,
                Action::Strike { dir: -1 },
                Action::Strike { dir: 1 },
            ]);
            if boss_in_reach(sim) {
                out.push(Action::Lift);
            }
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
        Phase::Carry => {
            for dir in [-1i8, 1] {
                for hold in throw_holds(sim) {
                    out.push(Action::Throw { dir, hold });
                }
            }
        }
        // A tentacle beside Aquatis holds Claw as a bear's hug does.
        Phase::Busy if held(sim) => out.push(Action::Mash),
        Phase::Climb | Phase::Swing | Phase::Busy => {}
    }
    out
}

/// True while Marrow's parrot `o` patrols the top of his room or sits on
/// its perch.
pub fn parrot_waits(o: &Object) -> bool {
    matches!(
        EnemyState::from_id(o.state),
        Some(EnemyState::FlyAbout | EnemyState::Idle)
    )
}

/// True while Marrow's parrot `o` dives at Claw (its set-up and its
/// swoop) or flinches from his hit.
pub fn parrot_dives(o: &Object) -> bool {
    matches!(
        EnemyState::from_id(o.state),
        Some(EnemyState::StrikeSetup | EnemyState::Strike | EnemyState::GenericHit)
    )
}

/// The ends of the spike pit between the ledges of Marrow's room, x.
///
/// These are the retail tenth level's: the bot's wait for the parrot's
/// dives in Marrow's fight ([`parrot_patrols`]) is tuned to that room. On
/// a community custom level whose Marrow room has its pit elsewhere, the
/// wait does not tell the sides apart, so the search there should not
/// count on it: it still plays the fight, but may try every move under
/// the parrot's patrol or wait where a move was needed.
pub const MARROW_PIT: (i32, i32) = (40640, 40991);

/// True in Marrow's fight while the slabs are open or opening, Claw is
/// on a ledge across the pit from Marrow, and the parrot patrols or
/// perches ([`parrot_waits`]): all Claw can do is wait for its dive.
pub fn parrot_patrols(sim: &Sim) -> bool {
    let (Some(link), Some(c)) = (sim.marrow_link(), sim.player()) else {
        return false;
    };
    let Some(m) = sim.objects().get(link.marrow) else {
        return false;
    };
    let side = |x: i32| (x > MARROW_PIT.1) as i32 - (x < MARROW_PIT.0) as i32;
    link.open_request
        && side(c.x) != 0
        && side(c.x) != side(m.x)
        && sim
            .objects()
            .all()
            .any(|(_, o)| matches!(o.data, ObjectData::Parrot(_)) && parrot_waits(o))
}

/// True when the boss in play is one the search drowns
/// ([`crate::search::drowns`]), he can be lifted now and Claw stands within
/// [`LIFT_REACH`] of him across and his lift's reach up or down.
pub fn boss_in_reach(sim: &Sim) -> bool {
    let (Some(c), Some(b)) = (
        sim.player(),
        sim.boss().current().and_then(|id| sim.objects().get(id)),
    ) else {
        return false;
    };
    crate::search::drowns(&b.logic)
        && b.special_flags & special::LIFTABLE != 0
        && (b.x - c.x).abs() <= LIFT_REACH
        && ((b.y + b.move_rect.bottom) - (c.y + c.move_rect.bottom)).abs() <= 2 * LIFT_FEET
}

/// True while a Siren's scream has taken Claw's controls: nothing he
/// presses acts until the lockout has run down, and the keys still held
/// then count as pressed.
pub fn locked(sim: &Sim) -> bool {
    sim.scream_lock_ms() > 0
}

/// The actions worth trying in `sim`, in a fixed order. While a scream
/// has taken Claw's controls ([`locked`]) only the wait and the walks are
/// worth trying: no key acts until the lockout ends, and a walk held
/// through its end walks on at once.
pub fn choices(sim: &Sim) -> Vec<Action> {
    let holds = jump_holds(sim);
    let mut out = vec![Action::Wait, Action::Left, Action::Right];
    if locked(sim) {
        return out;
    }
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
        Phase::Carry => {
            for dir in [-1i8, 1] {
                for hold in throw_holds(sim) {
                    out.push(Action::Throw { dir, hold });
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

/// True while a bear holds Claw in its hug or a tentacle squeezes him.
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
            Action::Lift,
        ];
        for dir in [-1i8, 1] {
            for hold in THROW_HOLDS {
                all.push(Action::Throw { dir, hold });
            }
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
    fn a_lift_presses_the_key_once_and_a_throw_turns_and_charges() {
        let m = Action::Lift.masks();
        assert_eq!(&m[..MIN_SPACING], &[LIFT; MIN_SPACING]);
        assert!(m[MIN_SPACING..].iter().all(|&k| k == 0));
        let m = Action::Throw { dir: -1, hold: 56 }.masks();
        assert_eq!(m.len(), 56 + THROW_FOLLOW);
        assert!(m[..56].iter().all(|&k| k == LEFT | LIFT));
        assert!(m[56..].iter().all(|&k| k == 0));
        assert_eq!(Action::Throw { dir: 1, hold: 170 }.name(), "throw+1x170");
        // The longest hold charges the throw in full at the original speeds.
        let full = THROW_HOLDS[THROW_HOLDS.len() - 1] as u32 * claw_sim::tuning::TICK_MS;
        assert!(full >= THROW_CHARGE_MS as u32);
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
