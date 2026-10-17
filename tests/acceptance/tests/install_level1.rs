//! The first level from start to finish on your own install (skipped
//! without `CLAW_INSTALL`).
//!
//! A chained route through the level, built from the level's own records
//! at run time: each stop is a scripted segment that starts where the
//! previous one left Claw, or with a move of Claw to the stop (the only
//! moves the route makes). Along the way:
//!
//! - every officer, soldier and rat is killed: with the sword in some
//!   segments, the pistol in others, and a powder keg Claw lifts and throws
//!   into one;
//! - every checkpoint flag and both save points are touched, and one death
//!   (a crumbling peg over spikes) brings Claw back at the last of them,
//!   with the peg and the elevator he rode back in place;
//! - a crate is broken and its coin and the level's treasure are
//!   collected, and the tally (collected against the level's census) adds
//!   up;
//! - the elevator over the first spike pit and the toggling pegs before the
//!   end are crossed;
//! - a soldier's bullet and a rat's bomb hurt Claw;
//! - the map piece ends the level.
//!
//! The recorded game is then replayed through the game's frame loop at
//! simulated 60, 144 and 240 Hz displays, steady and with jitter, as a
//! script and as key events: every trace and every per-tick digest of the
//! whole game is identical.
//!
//! Also here:
//!
//! - every logic of the level runs in an implemented family, with no inert
//!   placeholder;
//! - the game's preload covers every sprite set the simulation names, so
//!   no set is read while drawing;
//! - the first stretch of the route (to the first save point) also
//!   completes with widened activation (the option for wide displays),
//!   and how it differs is printed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use claw_acceptance::install;
use claw_acceptance::play::*;
use claw_frame::GameEvent;
use claw_game::settings::LevelChoice;
use claw_game::{LevelBundle, load_level};
use claw_sim::logic::enemy::DeathKind;
use claw_sim::logic::pickup;
use claw_sim::logic::platform::{self, PLATFORM};
use claw_sim::object::{LogicKind, ObjectId, flags, special as sflags};
use claw_sim::player::{death, state};
use claw_sim::tiles::DEATH;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};

/// Offsets from an enemy's record Claw is put at for a sword fight.
const NEAR: [i32; 8] = [-70, 70, -50, 50, -100, 100, -30, 30];
/// Offsets for a pistol fight.
const FAR: [i32; 10] = [90, -90, 120, -120, 150, -150, 200, -200, 250, -250];
/// Heights above an enemy Claw strikes from in the air.
const RISES: [i32; 8] = [10, 20, 0, 30, -10, 40, 5, 15];

fn level1() -> Option<LevelBundle> {
    let root = install()?;
    Some(load_level(&root, &LevelChoice::Retail(1)).expect("level 1 loads"))
}

// ------------------------------------------------------------------ stops

/// One stop of the route.
#[derive(Debug, Clone, PartialEq)]
enum Stop {
    /// Lift a powder keg and throw it into an enemy.
    Keg {
        keg: (i32, i32, i32),
        logic: String,
        enemy: (i32, i32, i32),
    },
    /// Break a crate and take what it drops.
    Crate((i32, i32, i32)),
    /// Fight an enemy.
    Enemy(String, (i32, i32, i32)),
    /// Touch a checkpoint flag or a save point.
    Flag(String, (i32, i32, i32)),
    /// Take a treasure.
    Treasure(String, (i32, i32, i32)),
    /// Ride the elevator over the spike pit.
    Elevator((i32, i32, i32)),
    /// Step on a crumbling peg over spikes: the death of the route.
    Death((i32, i32, i32)),
    /// Cross the toggling pegs.
    Pegs(i32),
    /// Take the map piece.
    MapPiece((i32, i32, i32)),
}

impl Stop {
    fn x(&self) -> i32 {
        match self {
            Stop::Keg { keg, .. } => keg.0,
            Stop::Crate(p)
            | Stop::Enemy(_, p)
            | Stop::Flag(_, p)
            | Stop::Treasure(_, p)
            | Stop::Elevator(p)
            | Stop::Death(p)
            | Stop::MapPiece(p) => p.0,
            Stop::Pegs(x) => *x,
        }
    }
}

/// The map piece's record.
fn map_piece(level: &SimLevel) -> (i32, i32, i32) {
    level
        .objects
        .iter()
        .find(|o| o.image_set == "GAME_MAPPIECE")
        .map(|o| (o.record[5], o.record[6], o.record[7]))
        .expect("the level has a map piece")
}

/// The toggling pegs before the end, in the order Claw crosses them (they
/// form a V: down from the left ledge, then up to the right one).
fn peg_section(level: &SimLevel) -> Vec<(String, (i32, i32, i32))> {
    let piece = map_piece(level);
    let mut pegs: Vec<(String, (i32, i32, i32))> = Vec::new();
    for logic in ["TogglePeg", "TogglePeg2", "TogglePeg3", "TogglePeg4"] {
        for r in records(level, logic) {
            if r.0 < piece.0 && piece.0 - r.0 < 1500 && (r.1 - piece.1).abs() < 800 {
                pegs.push((logic.to_string(), r));
            }
        }
    }
    pegs.sort_by_key(|(_, r)| r.0);
    pegs
}

/// Every stop: the keg throw and the coin crate nearest the start first,
/// then the level's stops in the order of the level (by x), and the map
/// piece last.
fn stops(level: &SimLevel) -> Vec<Stop> {
    let mut out = Vec::new();
    for logic in ["Officer", "Soldier", "Rat"] {
        for r in records(level, logic) {
            out.push(Stop::Enemy(logic.to_string(), r));
        }
    }
    for logic in [
        "Checkpoint",
        "FirstSuperCheckpoint",
        "SecondSuperCheckpoint",
    ] {
        for r in records(level, logic) {
            out.push(Stop::Flag(logic.to_string(), r));
        }
    }
    for logic in ["TreasurePowerup", "GlitterlessPowerup"] {
        for r in records(level, logic) {
            out.push(Stop::Treasure(logic.to_string(), r));
        }
    }
    out.push(Stop::Elevator(records(level, "StartElevator")[0]));
    let save = records(level, "FirstSuperCheckpoint")[0];
    let peg = records(level, "CrumblingPeg")
        .into_iter()
        .filter(|r| r.0 > save.0)
        .min_by_key(|r| r.0)
        .expect("a crumbling peg after the first save point");
    out.push(Stop::Death(peg));
    out.push(Stop::Pegs(peg_section(level)[0].1.0));
    out.sort_by_key(Stop::x);

    let start = level.start;
    let near = |r: &(i32, i32, i32)| (r.0 - start.0).abs() + (r.1 - start.1).abs();
    let mut first = Vec::new();
    let coin_crate = level
        .objects
        .iter()
        .filter(|o| o.logic == "FrontCrate" && o.record[15] == pickup::COIN_CODE)
        .map(|o| (o.record[5], o.record[6], o.record[7]))
        .min_by_key(near);
    first.extend(coin_crate.map(Stop::Crate));
    let kegs = records(level, "PowderKeg");
    let mut pairs = Vec::new();
    for logic in ["Officer", "Soldier"] {
        for e in records(level, logic) {
            for k in &kegs {
                let dx = (e.0 - k.0).abs();
                if (120..=300).contains(&dx) && (e.1 - k.1).abs() <= 20 {
                    pairs.push((near(k), *k, logic.to_string(), e));
                }
            }
        }
    }
    pairs.sort();
    let (_, keg, logic, enemy) = pairs.into_iter().next().expect("a keg near an enemy");
    first.push(Stop::Keg { keg, logic, enemy });
    first.extend(out);
    first.push(Stop::MapPiece(map_piece(level)));
    first
}

// --------------------------------------------------------------- the route

/// What the route did, for the checks at the end.
#[derive(Debug, Default)]
struct Notes {
    log: Vec<String>,
    keg_kill: Option<(i32, i32, i32)>,
    crate_broken: bool,
    elevator_crossed: bool,
    pegs_crossed: usize,
    death_checked: bool,
    bullet: bool,
    bomb: bool,
    /// Treasure pickups Claw took, by kind, and their points.
    taken_kinds: [u32; 9],
    taken_points: u64,
    flags_touched: BTreeSet<(i32, i32)>,
    decorations_animated: BTreeSet<ObjectId>,
}

/// True when Claw lies dead.
fn dead(p: &Player) -> bool {
    p.health() <= 0 || matches!(p.state(), state::DEATH | state::DEATH_FALL)
}

fn deaths_of(p: &Player) -> usize {
    p.events
        .iter()
        .filter(|(_, e)| matches!(e, GameEvent::ClawDied { .. } | GameEvent::GameOver))
        .count()
}

/// Plays `f` on a copy of `p` and keeps it when `f` returns true and Claw
/// did not die on the way.
fn attempt(p: &mut Player, f: impl FnOnce(&mut Player) -> bool) -> bool {
    let mut q = p.clone();
    let deaths = deaths_of(&q);
    if f(&mut q) && !dead(&q) && deaths_of(&q) == deaths {
        *p = q;
        true
    } else {
        false
    }
}

fn killed(p: &Player, placed: (i32, i32, i32)) -> bool {
    p.kills.iter().any(|k| k.placed == placed)
}

/// Turns Claw toward x.
fn face(p: &mut Player, x: i32) {
    let right = x > p.at().0;
    if p.sim.claw().facing_right != right {
        p.step(if right { RIGHT } else { LEFT });
        p.idle(2);
    }
}

/// Lifts the keg and throws it into the enemy: Claw stands between them
/// facing the keg, lifts it, turns and throws with a short charge.
fn keg_stop(
    p: &mut Player,
    keg: (i32, i32, i32),
    logic: &str,
    enemy: (i32, i32, i32),
    n: &mut Notes,
) -> String {
    let toward = (enemy.0 - keg.0).signum();
    for side in [40, 30, 50] {
        for charge in [0, 5, 10, 15, 20] {
            let ok = attempt(p, |q| {
                if !stand_at(q, keg.0 + toward * side, keg.1) {
                    return false;
                }
                face(q, keg.0);
                q.step(LIFT);
                let lifted = q.until(60, |q| {
                    (!(q.state() == state::LIFT && q.sim.claw().special.carrying)).then_some(0)
                });
                if !lifted {
                    return false;
                }
                q.step(if toward > 0 { RIGHT } else { LEFT });
                q.idle(2);
                q.hold(ATTACK, 1 + charge);
                q.until(150, |q| (!killed(q, enemy)).then_some(0));
                q.settle(60);
                // Killed by the keg striking him or by its blast.
                q.kills
                    .iter()
                    .any(|k| k.placed == enemy && matches!(&*k.by, "PowderKeg" | "Explosion"))
            });
            if ok {
                n.keg_kill = Some(enemy);
                return format!("thrown into the {logic} (charge {charge})");
            }
        }
    }
    panic!("no throw of the keg at {keg:?} reached the {logic} at {enemy:?}");
}

/// The pickups within `reach` of Claw that are not in `before`.
fn new_pickups(p: &Player, before: &BTreeSet<ObjectId>, reach: i32) -> Vec<ObjectId> {
    let (x, y) = p.at();
    p.sim
        .objects()
        .all()
        .filter(|(id, o)| {
            o.kind == LogicKind::Game(GameLogic::Pickup)
                && !before.contains(id)
                && (o.x - x).abs() < reach
                && (o.y - y).abs() < 200
                && o.flags & flags::NO_COLLIDE == 0
                && &*o.logic != pickup::GLITTER
                && &*o.logic != pickup::POINTS_ICON
        })
        .map(|(id, _)| id)
        .collect()
}

/// Walks Claw toward the pickup `id` until it is taken (or `limit`
/// ticks).
fn walk_to_pickup(p: &mut Player, id: ObjectId, limit: usize) -> bool {
    p.until(limit, |p| {
        let o = p.obj(id)?;
        if o.flags & flags::NO_COLLIDE != 0 {
            return None;
        }
        let dx = o.x - p.at().0;
        Some(if dx > 4 {
            RIGHT
        } else if dx < -4 {
            LEFT
        } else {
            0
        })
    })
}

/// Breaks the crate placed at `placed` with the sword and takes what it
/// drops.
fn crate_stop(p: &mut Player, placed: (i32, i32, i32), n: &mut Notes) -> String {
    for dx in [-60, 60, -45, 45, -75, 75] {
        let mut took = 0;
        let ok = attempt(p, |q| {
            if !stand_at(q, placed.0 + dx, placed.1) {
                return false;
            }
            let Some(id) = placed_object(&q.sim, "FrontCrate", placed) else {
                return false;
            };
            let before: BTreeSet<ObjectId> = q.sim.objects().all().map(|(id, _)| id).collect();
            face(q, placed.0);
            let mut since = 10;
            let broke = q.until(120, |q| {
                if q.obj(id)
                    .is_none_or(|o| o.flags & (flags::DESTROY | flags::NO_COLLIDE) != 0)
                {
                    return None;
                }
                Some(if !q.sim.claw().attacking && since > 2 {
                    since = 0;
                    ATTACK
                } else {
                    since += 1;
                    0
                })
            });
            if !broke {
                return false;
            }
            q.idle(10);
            let drops = new_pickups(q, &before, 300);
            for d in &drops {
                if walk_to_pickup(q, *d, 150) {
                    took += 1;
                }
            }
            q.settle(30);
            !drops.is_empty() && took == drops.len()
        });
        if ok {
            n.crate_broken = true;
            return format!("broken, {took} item(s) taken");
        }
    }
    panic!("the crate at {placed:?} could not be broken");
}

/// Waits facing the enemy `id` until `by` hurts Claw (at most `limit`
/// ticks); true when it did.
fn wait_for_wound(p: &mut Player, id: ObjectId, by: &str, limit: usize) -> bool {
    let k = p.wounds.len();
    if let Some(x) = p.obj(id).map(|o| o.x) {
        face(p, x);
    }
    p.until(limit, |p| {
        (!p.wounds[k..].iter().any(|w| w.by == by) && p.health() > 0).then_some(0)
    });
    p.wounds[k..].iter().any(|w| w.by == by)
}

fn enemy_stop(p: &mut Player, logic: &str, placed: (i32, i32, i32), n: &mut Notes) -> String {
    if killed(p, placed) {
        return "already dead".into();
    }
    // A soldier's bullet and a rat's bomb, each once, at the first such
    // enemy that lets Claw stand at range.
    let wait = match logic {
        "Soldier" if !n.bullet => Some("Bullet"),
        "Rat" if !n.bomb => Some("RatBomb"),
        _ => None,
    };
    if let Some(by) = wait {
        let ok = attempt(p, |q| {
            let Some(id) = place_beside(q, logic, placed, &FAR, true) else {
                return false;
            };
            if !wait_for_wound(q, id, by, 300) || dead(q) {
                return false;
            }
            let won = fight(q, id, Arm::Pistol, 1500).is_some()
                || fight(q, id, Arm::Sword, 1500).is_some();
            q.settle(80);
            won
        });
        if ok {
            match by {
                "Bullet" => n.bullet = true,
                _ => n.bomb = true,
            }
            return format!("waited for a {by}, then killed it");
        }
    }
    // Soldiers with the pistol while it has shots, the others with the
    // sword; then the other way round; then from the air.
    let tries: [(Arm, &[i32]); 2] = if logic == "Soldier" && p.sim.stats().pistol > 2 {
        [(Arm::Pistol, &FAR), (Arm::Sword, &NEAR)]
    } else {
        [(Arm::Sword, &NEAR), (Arm::Pistol, &FAR)]
    };
    for (arm, offsets) in tries {
        if arm == Arm::Pistol && p.sim.stats().pistol == 0 {
            continue;
        }
        let ok = attempt(p, |q| {
            let Some(id) = place_beside(q, logic, placed, offsets, arm == Arm::Pistol) else {
                return false;
            };
            let won = fight(q, id, arm, 1500).is_some();
            q.settle(80);
            won
        });
        if ok {
            return format!("{arm:?}");
        }
    }
    let ok = attempt(p, |q| {
        q.teleport(placed.0 - 55, placed.1 - 10);
        q.idle(5);
        let Some(id) = placed_object(&q.sim, logic, placed) else {
            return false;
        };
        // The segment ends with the kill: Claw may still be falling past
        // the enemy's ledge, and the next segment starts with a move.
        strike_from_the_air(q, id, Arm::Sword, &RISES)
            || (q.sim.stats().pistol > 0 && strike_from_the_air(q, id, Arm::Pistol, &RISES))
    });
    assert!(ok, "the {logic} at {placed:?} could not be killed");
    "from the air".into()
}

fn flag_stop(p: &mut Player, logic: &str, placed: (i32, i32, i32), n: &mut Notes) -> String {
    let (x, y, _) = placed;
    let point = GameEvent::Checkpoint { x, y: y - 32 };
    if p.events.iter().any(|(_, e)| *e == point) {
        n.flags_touched.insert((x, y));
        return "already touched".into();
    }
    let ok = attempt(p, |q| {
        q.teleport(x - 10, y);
        let k = q.events.len();
        q.until(150, |q| {
            (!q.events[k..].iter().any(|(_, e)| *e == point)).then_some(0)
        });
        q.settle(40);
        q.events[k..].iter().any(|(_, e)| *e == point)
    });
    assert!(ok, "the {logic} at {placed:?} was not touched");
    n.flags_touched.insert((x, y));
    "touched".into()
}

/// Walks into the treasure placed at `placed` from beside it, when it lies
/// over a floor with no death tile near.
fn treasure_stop(
    p: &mut Player,
    logic: &str,
    placed: (i32, i32, i32),
    level: &SimLevel,
    n: &mut Notes,
) -> String {
    let (x, y, _) = placed;
    let gone = placed_object(&p.sim, logic, placed)
        .and_then(|id| p.obj(id))
        .is_none_or(|o| o.special_flags & sflags::FLYING_TO_HUD != 0);
    if gone {
        return "already taken".into();
    }
    let Some(floor) = floor_under(level, x, y, 200) else {
        return "skipped: no floor under it".into();
    };
    let map = &level.map;
    if (x - 120..=x + 120)
        .step_by(4)
        .any(|xx| (floor - 64..floor + 64).any(|yy| map.attr(xx, yy) == DEATH))
    {
        return "skipped: death tiles near".into();
    }
    for dx in [-90, 90, -60, 60] {
        let mut taken = None;
        let ok = attempt(p, |q| {
            let Some(id) = placed_object(&q.sim, logic, placed) else {
                return false;
            };
            if q.obj(id)
                .is_none_or(|o| o.special_flags & sflags::FLYING_TO_HUD != 0)
            {
                return false;
            }
            if !stand_at(q, x + dx, floor) {
                return false;
            }
            // Walk into it, jumping when it hangs above Claw's reach.
            let dir = if dx < 0 { RIGHT } else { LEFT };
            let mut t = 0;
            let got = q.until(120, |q| {
                let o = q.obj(id)?;
                if o.special_flags & sflags::FLYING_TO_HUD != 0 {
                    return None;
                }
                t += 1;
                let (cx, cy) = q.at();
                let high = o.y < cy - 30 && (o.x - cx).abs() < 70;
                let held = if (o.x - cx).abs() > 4 { dir } else { 0 };
                Some(held | if high && t % 50 < 40 { JUMP } else { 0 })
            });
            if got {
                taken = q
                    .obj(id)
                    .and_then(pickup::data_of)
                    .map(|d| (d.kind, d.points));
            }
            q.settle(30);
            got && taken.is_some()
        });
        if ok {
            let (kind, points) = taken.expect("checked");
            n.taken_points += points.max(0) as u64;
            if let Some(k) = pickup::treasure_kind(kind) {
                n.taken_kinds[k] += 1;
            }
            return "taken".into();
        }
    }
    "skipped: not reached".into()
}

/// Rides the elevator over the spike pit to the far end of its track,
/// climbs the ladder there and walks out onto the floor beyond.
fn elevator_stop(p: &mut Player, placed: (i32, i32, i32), n: &mut Notes) -> String {
    let mut rode = 0;
    let ok = attempt(p, |q| {
        q.teleport(placed.0, placed.1 - 120);
        let Some(id) = placed_object(&q.sim, "StartElevator", placed) else {
            return false;
        };
        let rides = |q: &Player| {
            let c = q.claw();
            c.flags & flags::ON_ELEVATOR != 0 && c.object_below == Some(id)
        };
        if !q.until(80, |q| (!rides(q)).then_some(0)) {
            return false;
        }
        let x0 = q.at().0;
        let far = q
            .obj(id)
            .and_then(|o| o.record.as_ref().map(|r| r[53]))
            .unwrap_or(placed.0);
        let reached = q.until(300, |q| {
            (rides(q) && q.obj(id).is_some_and(|o| o.x < far - 8)).then_some(0)
        });
        if !reached {
            return false;
        }
        rode = q.at().0 - x0;
        q.until(40, |q| (q.state() != state::CLIMB).then_some(UP));
        q.until(150, |q| (q.state() == state::CLIMB).then_some(UP));
        let mut t = 0;
        q.until(200, |q| {
            t += 1;
            let out = q.state() == state::STAND && q.at().0 > far + 100;
            (!out).then_some(RIGHT | if t % 40 < 30 { JUMP } else { 0 })
        });
        q.settle(30);
        rode > 300 && q.at().0 > far + 100 && q.state() == state::STAND
    });
    assert!(ok, "the elevator at {placed:?} was not crossed");
    n.elevator_crossed = true;
    format!("ridden {rode} px over the spikes, out on the far side")
}

/// Steps on the crumbling peg: it crumbles, Claw falls to the spikes,
/// dies and comes back at the last checkpoint (the first save point); the
/// peg and the elevator he rode are back in place.
fn death_stop(p: &mut Player, peg: (i32, i32, i32), level: &SimLevel, n: &mut Notes) -> String {
    let save = records(level, "FirstSuperCheckpoint")[0];
    let elevator = records(level, "StartElevator")[0];
    let lives = p.sim.stats().lives;
    let k = p.events.len();
    let tile_sounds = |p: &Player| p.sounds.get(death::DEATH_TILE_SOUND).copied().unwrap_or(0);
    let tile_sounds_before = tile_sounds(p);
    p.teleport(peg.0, peg.1 - 120);
    let id = placed_object(&p.sim, "CrumblingPeg", peg).expect("the peg");
    let died = |p: &Player| {
        p.events[k..]
            .iter()
            .any(|(_, e)| matches!(e, GameEvent::ClawDied { .. }))
    };
    p.until(400, |p| (!died(p)).then_some(0));
    assert_eq!(
        p.obj(id).map(|o| o.state),
        Some(platform::state::GONE),
        "the peg crumbled under Claw"
    );
    let deaths: Vec<&GameEvent> = p.events[k..]
        .iter()
        .map(|(_, e)| e)
        .filter(|e| matches!(e, GameEvent::ClawDied { .. }))
        .collect();
    assert_eq!(
        deaths,
        vec![&GameEvent::ClawDied {
            lives_left: lives - 1
        }]
    );
    assert!(tile_sounds(p) > tile_sounds_before, "the spikes killed him");
    // He comes back at the last checkpoint once the circle has opened.
    p.until(300, |p| {
        (p.state() != state::STAND || p.sim.transition().is_some()).then_some(0)
    });
    assert_eq!(p.at().0, save.0, "Claw comes back at the last checkpoint");
    assert_eq!(p.health(), 100);
    assert_eq!(p.sim.stats().attempt, 1);
    // The peg and the elevator are back where they were placed.
    p.teleport(peg.0 - 80, peg.1 - 200);
    p.idle(3);
    let o = p.obj(id).expect("the peg is still there");
    assert_eq!(o.state, platform::state::STOPPED, "the peg is back");
    assert_eq!(o.type_flags, PLATFORM, "the peg is solid again");
    let e = placed_object(&p.sim, "StartElevator", elevator).expect("the elevator");
    p.teleport(elevator.0 - 60, elevator.1 - 150);
    p.idle(3);
    let o = p.obj(e).expect("the elevator");
    assert_eq!(
        (o.x, o.y),
        (elevator.0, elevator.1),
        "the elevator is back in place"
    );
    n.death_checked = true;
    assert!(stand_at(p, save.0, save.1));
    "died on the spikes, back at the save point; the peg and the elevator reset".into()
}

/// Where a hop lands.
enum Landing {
    /// On this platform.
    On(ObjectId),
    /// On the ground.
    Ground,
}

/// Waits `delay` ticks, then jumps from where Claw stands toward
/// `target_x`, `rise` px higher, when the target platform is out; true
/// when he lands on it and stands.
fn hop(p: &mut Player, to: &Landing, target_x: i32, rise: i32, delay: usize) -> bool {
    p.idle(delay);
    if let Landing::On(id) = to
        && p.obj(*id).is_none_or(|o| o.type_flags != PLATFORM)
    {
        return false;
    }
    let dir = if target_x > p.at().0 { RIGHT } else { LEFT };
    // A full jump up to a higher peg, a long one across, a hop down.
    let jump_ticks = match rise {
        r if r > 40 => 40,
        r if r > -40 => 20,
        _ => 6,
    };
    let mut t = 0;
    let landed = p.until(120, |p| {
        t += 1;
        if t > 3 && !matches!(p.state(), state::JUMP | state::FALL) {
            return None;
        }
        let steer = if (target_x - p.at().0).abs() > 6 {
            dir
        } else {
            0
        };
        Some(steer | if t <= jump_ticks { JUMP } else { 0 })
    });
    if !landed || dead(p) {
        return false;
    }
    p.idle(3);
    let c = p.claw();
    let on_it = match to {
        Landing::On(id) => c.flags & flags::ON_ELEVATOR != 0 && c.object_below == Some(*id),
        Landing::Ground => c.flags & flags::ON_ELEVATOR == 0,
    };
    on_it && p.state() == state::STAND && (c.x - target_x).abs() < 40
}

fn pegs_stop(p: &mut Player, level: &SimLevel, n: &mut Notes) -> String {
    let pegs = peg_section(level);
    let first = pegs[0].1;
    let last = pegs.last().expect("pegs").1;
    let floor = floor_under(level, first.0 - 150, first.1 - 200, 400).expect("the left ledge");
    assert!(stand_at(p, first.0 - 150, floor), "on the left ledge");
    let mut hops: Vec<(Landing, i32, i32)> = Vec::new();
    let mut prev_y = floor;
    for (logic, r) in &pegs {
        let id = placed_object(&p.sim, logic, *r).expect("the peg");
        hops.push((Landing::On(id), r.0, prev_y - r.1));
        prev_y = r.1;
    }
    let right = floor_under(level, last.0 + 150, last.1 - 200, 400).expect("the right ledge");
    hops.push((Landing::Ground, last.0 + 150, prev_y - right));
    // A small beam search over when to jump: a few landings that worked
    // are kept at each hop, so a landing too late in a peg's cycle for the
    // next hop is left for another. The landings kept are at least
    // `SPREAD` ticks of waiting apart, so they meet the next peg at
    // different points of its cycle (since kegs rest on the floor at
    // spawn, the route reaches the pegs at a point where the first three
    // landings in a row on one peg all meet the next one retracted).
    const SPREAD: usize = 12;
    let mut beam = vec![p.clone()];
    for (i, (to, x, rise)) in hops.iter().enumerate() {
        let mut next = Vec::new();
        for s in &beam {
            let mut found = 0;
            let mut from = 0;
            for delay in (0..170).step_by(2) {
                if delay < from {
                    continue;
                }
                let mut q = s.clone();
                if attempt(&mut q, |q| hop(q, to, *x, *rise, delay)) {
                    next.push(q);
                    found += 1;
                    from = delay + SPREAD;
                    if found == 3 {
                        break;
                    }
                }
            }
        }
        next.truncate(6);
        assert!(
            !next.is_empty(),
            "hop {i} to x {x} of the peg section failed"
        );
        beam = next;
        n.pegs_crossed += 1;
    }
    *p = beam.swap_remove(0);
    format!("{} hops over the spikes", hops.len())
}

fn map_piece_stop(p: &mut Player, (x, y, _): (i32, i32, i32)) -> String {
    assert!(stand_at(p, x - 120, y + 100), "beside the map piece");
    p.until(200, |p| match x - p.at().0 {
        d if d > 6 => Some(RIGHT),
        d if d < -6 => Some(LEFT),
        _ => None,
    });
    p.settle(30);
    let k = p.events.len();
    let done = |p: &Player| {
        p.events[k..]
            .iter()
            .any(|(_, e)| matches!(e, GameEvent::LevelComplete { .. }))
    };
    for _ in 0..3 {
        if done(p) {
            break;
        }
        p.hold(JUMP, 50);
        p.idle(10);
    }
    p.idle(20);
    "taken".into()
}

/// Notes the decorations whose frame changed since `frames`.
fn watch_decorations(p: &Player, frames: &mut BTreeMap<ObjectId, i32>, n: &mut Notes) {
    for id in p.sim.objects().listed() {
        let Some(o) = p.obj(id) else { continue };
        if o.kind != LogicKind::Game(GameLogic::Decor) {
            continue;
        }
        if frames.insert(id, o.frame).is_some_and(|f| f != o.frame) {
            n.decorations_animated.insert(id);
        }
    }
}

/// Plays the whole route from the level start.
fn play_route(level: &Arc<SimLevel>, cfg: SimConfig) -> (Player, Notes) {
    play_stops(level, cfg, &stops(level))
}

/// Plays `stops` from the level start.
fn play_stops(level: &Arc<SimLevel>, cfg: SimConfig, stops: &[Stop]) -> (Player, Notes) {
    let mut p = Player::new(Sim::new(level.clone(), cfg));
    let mut n = Notes::default();
    let mut frames = BTreeMap::new();
    for stop in stops {
        let (t0, h0) = (p.tick(), p.health());
        let what = match &stop {
            Stop::Keg { keg, logic, enemy } => keg_stop(&mut p, *keg, logic, *enemy, &mut n),
            Stop::Crate(c) => crate_stop(&mut p, *c, &mut n),
            Stop::Enemy(logic, r) => enemy_stop(&mut p, logic, *r, &mut n),
            Stop::Flag(logic, r) => flag_stop(&mut p, logic, *r, &mut n),
            Stop::Treasure(logic, r) => treasure_stop(&mut p, logic, *r, level, &mut n),
            Stop::Elevator(r) => elevator_stop(&mut p, *r, &mut n),
            Stop::Death(r) => death_stop(&mut p, *r, level, &mut n),
            Stop::Pegs(_) => pegs_stop(&mut p, level, &mut n),
            Stop::MapPiece(r) => map_piece_stop(&mut p, *r),
        };
        watch_decorations(&p, &mut frames, &mut n);
        assert!(!dead(&p), "Claw lies dead after {stop:?}");
        n.log.push(format!(
            "{stop:?}: {what} ({} ticks, health {h0} -> {})",
            p.tick() - t0,
            p.health()
        ));
    }
    (p, n)
}

// ------------------------------------------------------------------ tests

#[test]
fn install_level1_from_start_to_finish() {
    let Some(bundle) = level1() else { return };
    let level = bundle.sim_level.clone();
    let start = Sim::new(level.clone(), SimConfig::default());
    let (p, n) = play_route(&level, SimConfig::default());
    for line in &n.log {
        println!("{line}");
    }
    let stats = p.sim.stats().clone();

    // Every enemy is dead of a hit: by the sword, the pistol and the keg.
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    for k in &p.kills {
        *by.entry(k.by.as_str()).or_default() += 1;
    }
    println!("kills by: {by:?}");
    for logic in ["Officer", "Soldier", "Rat"] {
        for r in records(&level, logic) {
            let k = p.kills.iter().find(|k| k.placed == r);
            assert!(
                k.is_some_and(|k| k.death == Some(DeathKind::Hit)),
                "the {logic} at {r:?} was not killed by a hit: {k:?}"
            );
        }
    }
    assert_eq!(p.kills.len(), 54);
    assert!(
        by.get("CaptainClaw").copied().unwrap_or(0) >= 10,
        "sword kills: {by:?}"
    );
    assert!(
        by.get("PistolBullet").copied().unwrap_or(0) >= 3,
        "pistol kills: {by:?}"
    );
    assert!(n.keg_kill.is_some(), "a keg killed an enemy");

    // Every flag and both save points; one death, back at the last flag.
    for logic in [
        "Checkpoint",
        "FirstSuperCheckpoint",
        "SecondSuperCheckpoint",
    ] {
        for (x, y, _) in records(&level, logic) {
            assert!(
                p.events
                    .iter()
                    .any(|(_, e)| *e == GameEvent::Checkpoint { x, y: y - 32 }),
                "the {logic} at ({x}, {y}) was not touched"
            );
        }
    }
    for slot in [1, 2] {
        assert!(
            p.events
                .iter()
                .any(|(_, e)| *e == GameEvent::SavePoint { slot }),
            "save point {slot}"
        );
    }
    assert_eq!(deaths_of(&p), 1, "one death");
    assert!(n.death_checked);
    assert!(
        p.events
            .iter()
            .any(|(_, e)| *e == GameEvent::ClawDied { lives_left: 6 }),
        "the death took one of the seven lives"
    );
    let extra = p.sounds.get("GAME_EXTRALIFE").copied().unwrap_or(0) as i32;
    assert_eq!(stats.lives, 6 + extra, "{extra} extra lives taken");

    // Crates, treasure and the tally: what Claw counted is what he took,
    // never more than the level holds, and the game total follows.
    assert!(n.crate_broken);
    let census = p.sim.treasure_census();
    let collected: u32 = stats.treasure_level.iter().sum();
    println!(
        "treasure collected {:?} of the census {:?} ({collected} items), score {}",
        stats.treasure_level, census, stats.score
    );
    assert!(collected >= 20, "treasure collected: {collected}");
    let mut kinds = [0u32; 9];
    let mut points = 0i64;
    for &(kind, value) in p.treasure.values() {
        if let Some(k) = pickup::treasure_kind(kind) {
            kinds[k] += 1;
        }
        points += i64::from(value);
    }
    assert_eq!(
        stats.treasure_level, kinds,
        "the tally counts every treasure taken"
    );
    for (k, (&got, &held)) in stats.treasure_level.iter().zip(&census).enumerate() {
        assert!(got <= held, "kind {k}: more than the level holds");
        assert!(got >= n.taken_kinds[k], "kind {k}");
    }
    assert_eq!(stats.treasure_game, stats.treasure_level);
    assert_eq!(
        i64::from(stats.score),
        points,
        "the score is the treasure's worth"
    );
    assert!(points as u64 >= n.taken_points);

    // The elevator, the pegs, the bullet and the bomb.
    assert!(n.elevator_crossed);
    assert!(n.pegs_crossed >= 5);
    assert!(
        p.wounds.iter().any(|w| w.by == "Bullet"),
        "a soldier's bullet hurt Claw"
    );
    assert!(
        p.wounds.iter().any(|w| w.by == "RatBomb"),
        "a rat's bomb hurt Claw"
    );
    println!(
        "{} decorations seen animating",
        n.decorations_animated.len()
    );
    assert!(n.decorations_animated.len() >= 20);

    // The map piece ends the level.
    assert!(
        p.events
            .iter()
            .any(|(_, e)| matches!(e, GameEvent::LevelComplete { .. })),
        "the level was completed"
    );
    println!(
        "route: {} ticks ({:.0} s of play), {} moves between segments, {} object slots at the end",
        p.tick(),
        p.tick() as f64 * 0.018,
        p.teleports.len(),
        p.sim.objects().len()
    );

    // The same game at every display rate.
    let base = replay(&start, &p, 60.0, None, Feed::Script);
    assert_eq!(
        base.digests[p.held.len()],
        digest(&p.sim),
        "the replay reproduces the route"
    );
    for (hz, jitter, feed) in [
        (60.0, Some(1), Feed::Keys),
        (144.0, None, Feed::Keys),
        (144.0, Some(2), Feed::Keys),
        (240.0, None, Feed::Script),
        (240.0, Some(3), Feed::Keys),
    ] {
        let r = replay(&start, &p, hz, jitter, feed);
        assert_eq!(r.trace, base.trace, "{hz} Hz {jitter:?} {feed:?}: trace");
        assert_eq!(
            r.digests, base.digests,
            "{hz} Hz {jitter:?} {feed:?}: digests"
        );
    }
}

#[test]
fn install_level1_every_logic_runs_in_an_implemented_family() {
    let Some(bundle) = level1() else { return };
    let level = bundle.sim_level.clone();
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    sim.step(claw_frame::TickInput::default());
    let d = sim.diagnostics();
    assert!(
        d.unknown_logics.is_empty(),
        "unknown logics: {:?}",
        d.unknown_logics
    );
    assert!(
        d.inert_logics.is_empty(),
        "inert placeholders: {:?}",
        d.inert_logics
    );
    assert_eq!(d.dropped_records, 0);
    let mut families: BTreeMap<String, usize> = BTreeMap::new();
    for o in &level.objects {
        let kind = claw_sim::logic::kind_for(&o.logic);
        assert!(
            matches!(kind, Some(LogicKind::Game(_))),
            "{} runs in no family",
            o.logic
        );
        *families.entry(format!("{kind:?}")).or_default() += 1;
    }
    // Every level object became an object of its family.
    let placed = sim
        .objects()
        .all()
        .filter(|(_, o)| o.record.is_some())
        .filter(|(_, o)| !matches!(o.kind, LogicKind::Game(_)))
        .map(|(_, o)| o.logic.to_string())
        .collect::<Vec<_>>();
    assert!(
        placed.is_empty(),
        "placed objects with no family: {placed:?}"
    );
    println!(
        "{} records in {} families: {families:?}",
        level.objects.len(),
        families.len()
    );
}

#[test]
fn install_level1_preload_covers_every_sprite_set_the_simulation_names() {
    let Some(bundle) = level1() else { return };
    let sim = Sim::new(bundle.sim_level.clone(), SimConfig::default());
    let first = sim.snapshot();
    let runtime = claw_sim::logic::runtime_image_sets();
    let keys: BTreeSet<&str> = claw_game::load::preload_keys(
        bundle.level.objects.iter().map(|o| o.image_set.as_str()),
        &runtime,
        &first,
    )
    .into_iter()
    .collect();
    let mut graphics = claw_game::load_graphics(&bundle, None).expect("graphics");
    let mut headers = Vec::new();
    for set in bundle.sim_level.image_sets() {
        if keys.contains(set.as_str()) {
            continue;
        }
        // Only the animations' header names may be missing, and only when
        // they are not sets the renderer draws.
        assert!(
            graphics.sprite(&set, -1).is_none(),
            "the set {set} is drawn but not preloaded"
        );
        headers.push(set);
    }
    println!(
        "{} sets preloaded; {} animation header names that are no sprite sets: {headers:?}",
        keys.len(),
        headers.len()
    );
}

#[test]
fn install_level1_first_stretch_with_widened_activation() {
    let Some(bundle) = level1() else { return };
    let level = bundle.sim_level.clone();
    let save = records(&level, "FirstSuperCheckpoint")[0];
    let stretch: Vec<Stop> = stops(&level)
        .into_iter()
        .take_while(|s| !matches!(s, Stop::Flag(l, _) if l == "FirstSuperCheckpoint"))
        .chain([Stop::Flag("FirstSuperCheckpoint".into(), save)])
        .collect();
    let mut settings = claw_game::Settings::new(install().unwrap(), LevelChoice::Retail(1));
    settings.res = (5120, 1440);
    let original = settings.sim_config(settings.res);
    settings.widen_activation = true;
    let widened = settings.sim_config(settings.res);
    assert_ne!(original.activation, widened.activation);
    let (a, na) = play_stops(&level, original, &stretch);
    let (b, nb) = play_stops(&level, widened, &stretch);
    let summary = |p: &Player| {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for w in &p.wounds {
            *by.entry(w.by.clone()).or_default() += w.lost as usize;
        }
        (p.tick(), p.health(), p.kills.len(), by, p.sim.stats().score)
    };
    println!("frozen-edge activation: {:?}", summary(&a));
    println!("widened activation:     {:?}", summary(&b));
    // The stops whose outcome or length differ (health aside).
    let mut differ = 0;
    for (x, y) in na.log.iter().zip(&nb.log) {
        let (x, y) = (x.split(", health").next(), y.split(", health").next());
        if x != y {
            differ += 1;
            println!(
                "  frozen-edge: {}\n  widened:     {}",
                x.unwrap_or(""),
                y.unwrap_or("")
            );
        }
    }
    println!("{differ} of {} stops differ", na.log.len());
    assert_eq!(na.log.len(), nb.log.len());
    assert!(
        b.events
            .iter()
            .any(|(_, e)| *e == GameEvent::SavePoint { slot: 1 })
    );
}
