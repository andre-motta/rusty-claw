//! Path elevators: moving platforms that follow a closed route of straight
//! legs, each leg one of the eight keypad directions for a given number of
//! pixels, at a fixed speed.
//!
//! Like the plain elevator, a path elevator does not step itself forward:
//! every tick it computes where the level clock puts it on its route and
//! moves there, so it stays in phase however long it slept outside its
//! window, and it tells its riders how far it moved in the same tick
//! ([`Object::platform_move`]). Claw and every enemy standing on it add
//! that move to their own; it runs before them (its depth is lower), so
//! they see this tick's move.
//!
//! The route is written into the level record's four boxes (movement,
//! hit, attack and clip), two legs per box as (direction, distance)
//! pairs: (left, top) then (right, bottom) of each box in that order,
//! stopping at the first pair with a zero direction or distance, at most
//! eight legs. The legs are read from the boxes as the level loader leaves
//! them, after its rule for empty boxes, so a box with zero left and right
//! reads as an unset leg (no move, its time counted); the real platform
//! boxes replace them after. The movement box must hold a first direction,
//! or no route is read. Two built-in routes, chosen by the record's
//! powerup field (1 or 2), take precedence; that field is never loot.
//!
//! Set-up, on the first call: the route; the speed (the record's speed
//! field, 100 px/s when 0); the phase (the record's direction field, in
//! ms); the period, the sum of each leg's `distance * 1000 / speed`
//! (unsigned, truncated per leg); out of pair collision, a platform, the
//! movement box of the frame shown and the level's elevator surface (the
//! big elevator's for the trap elevator of level 10). The depth is kept as
//! placed and the elevator does not move on that call. Without a route it
//! stays still where it was placed.
//!
//! Each later call: `t = (clock + phase) mod period`, unsigned; walk the
//! legs, a whole leg at a time while `t` covers it, then
//! `t * speed / 1000` px (whole pixels, unsigned) along the leg it ends
//! in; the elevator stands at its creation point plus that offset. The
//! move it publishes is zero when either axis moved 100 px or more (the
//! jump after a long sleep, or an open route wrapping back to its start).
//! No sound and no random draw.

use super::platform::{PLATFORM, SurfaceKind, level_clock, surface};
use super::{ObjectData, World};
use crate::object::{Object, ObjectId, Rect, flags};

/// Logic names this family runs.
pub const NAMES: &[&str] = &["PathElevator"];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// Speed of a path elevator placed without one, px/s.
pub const DEFAULT_SPEED: i32 = 100;
/// Most legs a route holds.
pub const MAX_LEGS: usize = 32;
/// Most legs read from a record's boxes.
pub const MAX_RECORD_LEGS: usize = 8;
/// A move this large on either axis, px, is published as no move.
pub const MOVE_GUARD: i32 = 100;

/// Logic states.
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = 0;
    /// Following its route.
    pub const MOVING: i32 = 1;
    /// No route: a still platform.
    pub const STILL: i32 = 5;
}

/// Record fields the path elevator reads.
mod field {
    pub const POWERUP: usize = 15;
    pub const MOVE_RECT: usize = 19;
    pub const SPEED: usize = 60;
    pub const DIRECTION: usize = 63;
}

/// One leg of a route: a keypad direction (1 down-left, 2 down, 3
/// down-right, 4 left, 6 right, 7 up-left, 8 up, 9 up-right; any other
/// value stands still for the leg's time) and a distance in pixels on each
/// axis it moves along.
pub type Leg = (i32, i32);

/// A path elevator's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PathElevatorData {
    /// The route.
    pub legs: Vec<Leg>,
    /// Speed, px/s.
    pub speed: i32,
    /// Clock offset, ms.
    pub phase: i32,
    /// Time of one whole route, ms.
    pub period: u32,
    /// Tick of the last call.
    pub last_tick: Option<u64>,
}

/// The built-in route `n` (1 or 2), if there is one.
pub fn built_in(n: i32) -> Option<Vec<Leg>> {
    match n {
        1 => Some(vec![(6, 100), (7, 100), (6, 100), (1, 100)]),
        2 => Some(vec![(6, 100), (7, 100), (5, 100), (6, 100), (1, 100)]),
        _ => None,
    }
}

/// The legs written in a record's boxes (`ints` from the movement box's
/// left edge on: movement, hit, attack and clip boxes, four integers
/// each), or `None` when the movement box holds no first direction. A box
/// with zero left and right reads with an unset left edge
/// ([`Rect::UNSET_LEFT`]), as the level loader leaves it.
pub fn record_legs(ints: &[i32]) -> Option<Vec<Leg>> {
    let boxes: Vec<[i32; 4]> = ints
        .chunks_exact(4)
        .take(4)
        .map(|b| {
            let mut b = [b[0], b[1], b[2], b[3]];
            if b[0] == 0 && b[2] == 0 {
                b[0] = Rect::UNSET_LEFT;
            }
            b
        })
        .collect();
    let first = boxes.first()?[0];
    if first == 0 || first == Rect::UNSET_LEFT {
        return None;
    }
    let mut legs = Vec::new();
    for b in &boxes {
        for leg in [(b[0], b[1]), (b[2], b[3])] {
            if leg.0 == 0 || leg.1 == 0 || legs.len() == MAX_RECORD_LEGS {
                return Some(legs);
            }
            legs.push(leg);
        }
    }
    Some(legs)
}

/// The time a leg of `distance` px takes at `speed` px/s, ms (unsigned,
/// truncated).
pub fn leg_ms(distance: i32, speed: i32) -> u32 {
    (distance as u32).wrapping_mul(1000) / (speed as u32).max(1)
}

/// The time of the whole route, ms.
pub fn period(legs: &[Leg], speed: i32) -> u32 {
    legs.iter()
        .fold(0u32, |p, &(_, d)| p.wrapping_add(leg_ms(d, speed)))
}

/// The move of `d` px along keypad direction `dir`.
pub fn along(dir: i32, d: i32) -> (i32, i32) {
    match dir {
        1 => (-d, d),
        2 => (0, d),
        3 => (d, d),
        4 => (-d, 0),
        6 => (d, 0),
        7 => (-d, -d),
        8 => (0, -d),
        9 => (d, -d),
        _ => (0, 0),
    }
}

/// Where the route puts the elevator `t` ms into it, relative to its
/// creation point.
pub fn offset(legs: &[Leg], t: u32, speed: i32) -> (i32, i32) {
    let (mut x, mut y) = (0i32, 0i32);
    if speed == 0 {
        return (x, y);
    }
    let mut left = t;
    for &(dir, d) in legs {
        let seg = leg_ms(d, speed);
        if seg > left {
            let part = (left.wrapping_mul(speed as u32) / 1000) as i32;
            let (dx, dy) = along(dir, part);
            return (x.wrapping_add(dx), y.wrapping_add(dy));
        }
        let (dx, dy) = along(dir, d);
        x = x.wrapping_add(dx);
        y = y.wrapping_add(dy);
        left -= seg;
        if left == 0 {
            break;
        }
    }
    (x, y)
}

/// Where the elevator is at level clock `ms`, relative to its creation
/// point; `None` when its period is 0.
pub fn position_at(d: &PathElevatorData, ms: u32) -> Option<(i32, i32)> {
    if d.period == 0 {
        return None;
    }
    let t = ms.wrapping_add(d.phase as u32) % d.period;
    Some(offset(&d.legs, t, d.speed))
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let ms = level_clock(w);
    let (level, tick) = (w.level, w.tick);
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // The data is held outside the object during the call and put back
    // after it.
    let mut d = match std::mem::take(&mut o.data) {
        ObjectData::PathElevator(d) => d,
        _ => PathElevatorData::default(),
    };
    match o.state {
        state::SET_UP => set_up(level, o, &mut d),
        state::MOVING => step(o, &mut d, ms, tick),
        _ => {}
    }
    d.last_tick = Some(tick);
    o.data = ObjectData::PathElevator(d);
}

fn rec(o: &Object, i: usize) -> i32 {
    o.record.as_ref().map_or(0, |r| r[i])
}

fn set_up(level: &crate::level::SimLevel, o: &mut Object, d: &mut PathElevatorData) {
    let legs = built_in(rec(o, field::POWERUP)).or_else(|| {
        o.record
            .as_ref()
            .and_then(|r| record_legs(&r[field::MOVE_RECT..field::MOVE_RECT + 16]))
    });
    let speed = rec(o, field::SPEED);
    d.speed = if speed == 0 { DEFAULT_SPEED } else { speed };
    d.phase = rec(o, field::DIRECTION);
    d.legs = legs.clone().unwrap_or_default();
    d.legs.truncate(MAX_LEGS);
    d.period = period(&d.legs, d.speed);
    o.flags |= flags::NO_COLLIDE;
    o.type_flags = PLATFORM;
    if let Some(b) = level.frame_box(&o.image_set, o.frame) {
        o.move_rect = b;
    }
    let kind = if level.number == 10 && o.image_set == "LEVEL_TRAPELEVATOR" {
        SurfaceKind::BigElevator
    } else {
        SurfaceKind::Elevator
    };
    o.hit_rect = surface(level.number, kind);
    o.platform_move = (0, 0);
    o.state = if legs.is_some() {
        state::MOVING
    } else {
        state::STILL
    };
}

fn step(o: &mut Object, d: &mut PathElevatorData, ms: u32, tick: u64) {
    let Some((dx, dy)) = position_at(d, ms) else {
        return;
    };
    let (x0, y0) = (o.x, o.y);
    o.x = o.editor.0.wrapping_add(dx);
    o.y = o.editor.1.wrapping_add(dy);
    let mv = (o.x.wrapping_sub(x0), o.y.wrapping_sub(y0));
    let guarded =
        mv.0.unsigned_abs() >= MOVE_GUARD as u32 || mv.1.unsigned_abs() >= MOVE_GUARD as u32;
    o.platform_move = if guarded { (0, 0) } else { mv };
    // A jump (after a sleep, or an open route wrapping) is drawn at the
    // new place at once instead of sweeping across.
    let woken = d.last_tick.is_some_and(|t| t.wrapping_add(1) != tick);
    if guarded || woken {
        o.teleported = true;
    }
}

/// `attacker` hit `id`. Nothing hits a path elevator.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`. A path elevator attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The octagon of the first group on the third level: diagonals of 60
    /// and straight legs of 200, at 200 px/s.
    fn octagon() -> Vec<Leg> {
        vec![
            (9, 60),
            (6, 200),
            (3, 60),
            (2, 200),
            (1, 60),
            (4, 200),
            (7, 60),
            (8, 200),
        ]
    }

    #[test]
    fn periods_sum_truncated_legs() {
        assert_eq!(period(&octagon(), 200), 5200);
        let pentagon = [(2, 300), (3, 195), (9, 195), (8, 300), (4, 390)];
        assert_eq!(leg_ms(300, 175), 1714);
        assert_eq!(leg_ms(195, 175), 1114);
        assert_eq!(leg_ms(390, 175), 2228);
        assert_eq!(period(&pentagon, 175), 7884);
    }

    #[test]
    fn a_closed_route_comes_back_to_its_start() {
        let legs = octagon();
        assert_eq!(offset(&legs, 0, 200), (0, 0));
        assert_eq!(offset(&legs, 5200, 200), (0, 0));
        // Partway along the first diagonal: whole pixels on both axes.
        assert_eq!(offset(&legs, 18, 200), (3, -3));
        assert_eq!(offset(&legs, 36, 200), (7, -7));
        // At the end of the first leg, then into the second.
        assert_eq!(offset(&legs, 300, 200), (60, -60));
        assert_eq!(offset(&legs, 318, 200), (63, -60));
        // Half way round.
        assert_eq!(offset(&legs, 2600, 200), (320, 200));
    }

    #[test]
    fn the_clock_and_the_phase_pick_the_place() {
        let d = PathElevatorData {
            legs: octagon(),
            speed: 200,
            phase: 0,
            period: 5200,
            last_tick: None,
        };
        assert_eq!(position_at(&d, 5200 + 18), Some((3, -3)));
        let late = PathElevatorData {
            phase: 300,
            ..d.clone()
        };
        assert_eq!(position_at(&late, 18), Some((63, -60)));
        let empty = PathElevatorData {
            period: 0,
            ..PathElevatorData::default()
        };
        assert_eq!(position_at(&empty, 100), None);
    }

    #[test]
    fn legs_come_from_the_boxes_until_an_empty_pair() {
        let mut ints = [0; 16];
        ints[..6].copy_from_slice(&[9, 60, 6, 200, 3, 60]);
        assert_eq!(record_legs(&ints), Some(vec![(9, 60), (6, 200), (3, 60)]));
        // Every box full: eight legs.
        let full: Vec<i32> = (0..16).map(|i| if i % 2 == 0 { 6 } else { 10 }).collect();
        assert_eq!(record_legs(&full).map(|l| l.len()), Some(8));
        // No first direction: no route.
        assert_eq!(record_legs(&[0; 16]), None);
        // A box with zero left and right but a top reads as an unset leg
        // that stands still for its time.
        let mut odd = [0; 16];
        odd[..4].copy_from_slice(&[6, 100, 2, 50]);
        odd[4..8].copy_from_slice(&[0, 40, 0, 0]);
        let legs = record_legs(&odd).unwrap();
        assert_eq!(legs, vec![(6, 100), (2, 50), (Rect::UNSET_LEFT, 40)]);
        assert_eq!(along(Rect::UNSET_LEFT, 40), (0, 0));
        assert_eq!(period(&legs, 100), 1000 + 500 + 400);
    }

    #[test]
    fn built_in_routes_have_a_pause_leg() {
        assert_eq!(built_in(1).unwrap().len(), 4);
        assert!(built_in(2).unwrap().contains(&(5, 100)));
        assert_eq!(built_in(0), None);
        assert_eq!(along(5, 100), (0, 0));
    }
}
