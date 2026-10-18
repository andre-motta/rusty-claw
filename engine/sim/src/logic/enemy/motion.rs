//! The movement step of enemies, run at the end of every post-tick, and
//! what follows it: death tiles, the ledge guard and the depth against
//! Claw.
//!
//! The step:
//!
//! 1. A frozen enemy (dead in place, or carried by Claw) does not move at
//!    all.
//! 2. One riding a platform moves with it first; one that is not standing
//!    rides nothing.
//! 3. Each axis moves by `(v + a dt / 2) dt`, at most 24 px; the speed
//!    then grows by `a dt` (after a capped step, it is what the energy of
//!    the capped move gives). Across there is no acceleration; up and down
//!    there is gravity, except for a standing enemy, which does not move
//!    up or down and has no vertical speed. `dt` is the 18 ms tick, except
//!    on the first step after a gap (an enemy woken after sleeping outside
//!    its activation window): that step covers the whole time since the
//!    last step, so a woken walker first makes up the way it did not walk
//!    while asleep, up to the 24 px step limit (a rat asleep for 3.7 s
//!    moves 24 px, then stops on its patrol limit). A step with no game
//!    time since the last one, as in a respawn's update passes, covers
//!    none and moves nothing.
//! 4. The new position is clamped to the patrol limits: a walker that
//!    would cross a limit stops exactly on it.
//! 5. The object moves there through the tiles and platforms. When the
//!    tiles stop it short, the carried fraction is dropped, and when they
//!    stop it up or down its vertical speed is the one the move it made
//!    gives: `sqrt(v0^2 + 2 a (y - p0))` with the sign of `v0`, from the
//!    speed `v0` and exact height `p0` it had before the step (a thrown
//!    enemy falling at 804 px/s that lands 9.396 px lower lands at
//!    827.04 px/s, not 840). Unless it moves without tile tests: a ceiling
//!    then turns its vertical speed around, and a wall on either side moves
//!    that patrol limit to where it stood before the step and turns its
//!    speed across around at half.
//! 6. Unless it moves without tile tests: landing on a death tile kills
//!    it (spikes; goo is not told apart yet), and a standing enemy with the
//!    ledge guard whose leading edge has no floor under it goes back to
//!    where it stood before the step and makes that its patrol limit. It
//!    goes back across only when it entered the step standing, so one
//!    riding a platform up or down stays on it; one that entered the step
//!    in the air goes back on both coordinates, its speed up or down again
//!    from the move it made.
//!    The leading edge is the side its exact position moved to in the
//!    step, after the tiles had their say: a thrown enemy whose move into
//!    a step's side is stopped while it lands on the step's corner moved
//!    back by the fraction it carried, so it looks at its back edge, finds
//!    no floor there and lands where it was, in the air.
//! 7. With the depth sort, it is drawn in front of Claw when his feet are
//!    at or above its own, behind him otherwise.

use crate::logic::World;
use crate::object::{ObjectId, flags};
use crate::physics::{self, Collider, opt, ptype, res};
use crate::tiles::{CLEAR, CLIMB, GROUND, SOLID};
use crate::tuning::TICK_MS;

use super::{DeathKind, EnemyData, EnemyState, TICK_S, die, obj, obj_mut, with_data};

/// Largest move per axis in one step, px. It is also all that limits the
/// first step after a gap: that step covers the whole time since the
/// previous one (after 108 ms asleep a pacing officer first moves 13 px,
/// after longer sleeps 24 px; a rat woken after 3.744 s moves 24 px and is
/// held on its patrol limit). The rest of the rule is inferred with it:
/// the catch-up fires after any gap over one 18 ms tick, not only after a
/// sleep, and an enemy that has never moved (no time of a last step yet)
/// covers one 18 ms tick.
pub const STEP_LIMIT: f64 = 24.0;
/// Depth above the base of an enemy drawn in front of Claw.
pub const DEPTH_IN_FRONT: i32 = 4000;
/// Depth above the base of an enemy drawn behind Claw.
pub const DEPTH_BEHIND: i32 = 3899;
/// How far a narrow ledge reaches on either side of an enemy, px.
pub const NARROW_REACH: i32 = 49;

/// Moves one axis at `v` with acceleration `a` for one tick: the move,
/// capped to [`STEP_LIMIT`], and the speed after it.
pub fn axis_step(v: &mut f64, a: f64) -> f64 {
    axis_step_for(v, a, TICK_S)
}

/// [`axis_step`] over `dt` seconds.
pub fn axis_step_for(v: &mut f64, a: f64, dt: f64) -> f64 {
    let d = (*v + 0.5 * a * dt) * dt;
    if d.abs() > STEP_LIMIT {
        let d = STEP_LIMIT.copysign(d);
        if a != 0.0 {
            *v = v.signum() * (*v * *v + 2.0 * a * d).max(0.0).sqrt();
        }
        d
    } else {
        *v += a * dt;
        d
    }
}

/// `p` kept between the limits `min` and `max`.
fn clamp_to(p: f64, (min, max): (i32, i32)) -> f64 {
    p.min(f64::from(max)).max(f64::from(min))
}

fn is_floor(a: u8) -> bool {
    a == SOLID || a == GROUND || a == CLIMB
}

/// The movement step and what follows it (see the module documentation).
pub fn integrate(w: &mut World, id: ObjectId) {
    let death = with_data(w, id, |w, d| step(w, id, d)).flatten();
    if let Some(kind) = death {
        die(w, id, kind, None);
    }
}

/// The time the step at `now` covers, s: one tick, or after a gap (the
/// enemy slept, so no step ran for more than a tick) the whole time since
/// the last step, which only the [`STEP_LIMIT`] of the move bounds. A step
/// with no game time since the last one covers none: a second step in the
/// same moment, as in the update passes of a respawn, where the clock
/// stands still, moves nothing. The first step covers one tick. Records
/// `now` as the time of the last step.
fn step_seconds(d: &mut EnemyData, now: u64) -> f64 {
    let last = d.last_move_ms.replace(now);
    let since = last.map_or(0, |t| now.saturating_sub(t));
    if last.is_some() && since == 0 {
        return 0.0;
    }
    let ms = if since > u64::from(TICK_MS) {
        since
    } else {
        u64::from(TICK_MS)
    };
    ms as f64 * 0.001
}

fn step(w: &mut World, id: ObjectId, d: &mut EnemyData) -> Option<DeathKind> {
    if d.frozen {
        d.last_move_ms = Some(w.now_ms);
        return None;
    }
    let (x0, y0, entry, ride) = {
        let o = obj(w, id);
        let ride = if o.flags & flags::ON_ELEVATOR != 0 {
            o.object_below
                .and_then(|b| w.objects.get(b))
                .map_or((0, 0), |p| p.platform_move)
        } else {
            (0, 0)
        };
        (o.x, o.y, o.physics_type, ride)
    };
    // Something else moved the object since the last step: start from
    // where it is now.
    if d.moved_to != Some((x0, y0)) {
        d.carry = (0.0, 0.0);
    }
    let dt = step_seconds(d, w.now_ms);
    let standing = entry == ptype::STAND;
    // Where the step starts from, exactly, and the speed up or down then.
    let p0 = (f64::from(x0) + d.carry.0, f64::from(y0) + d.carry.1);
    let v0y = d.vy;
    let px = clamp_to(
        f64::from(x0) + d.carry.0 + f64::from(ride.0) + axis_step_for(&mut d.vx, 0.0, dt),
        d.bounds,
    );
    let tx = px.trunc() as i32;
    let (py, ty) = if standing {
        d.vy = 0.0;
        let y = y0 + ride.1;
        (f64::from(y), y)
    } else {
        let ay = d.ay;
        let py = clamp_to(
            f64::from(y0) + d.carry.1 + f64::from(ride.1) + axis_step_for(&mut d.vy, ay, dt),
            d.bounds_y,
        );
        (py, py.trunc() as i32)
    };
    {
        let o = obj_mut(w, id);
        o.flags &= !flags::ON_DEATH_TILE;
        if !standing {
            o.flags &= !flags::ON_ELEVATOR;
        }
    }
    let r = collide(w, id, tx, ty);
    if standing {
        d.vy = 0.0;
    }
    let (x, y, now) = {
        let o = obj(w, id);
        (o.x, o.y, o.physics_type)
    };
    d.carry.0 = if x == tx { px - f64::from(tx) } else { 0.0 };
    d.carry.1 = if !standing && y == ty {
        py - f64::from(ty)
    } else {
        0.0
    };
    if now == ptype::DIRECT {
        d.moved_to = Some((x, y));
        if d.z_sort {
            depth_sort(w, id, d);
        }
        return None;
    }
    if !standing && y != ty {
        d.vy = speed_after(v0y, d.ay, f64::from(y) - p0.1);
    }
    if r & res::CEILING != 0 {
        d.vy = -d.vy;
    } else if r & res::WALL != 0 && r & res::RIGHT != 0 {
        d.bounds.1 = x0;
        d.vx *= -0.5;
    } else if r & res::WALL != 0 && r & res::LEFT != 0 {
        d.bounds.0 = x0;
        d.vx *= -0.5;
    }
    let state = super::EnemyState::from_id(obj(w, id).state);
    let death = (r & res::DEATH_TILE != 0 && state != Some(EnemyState::Dead) && !d.dead)
        .then_some(DeathKind::Spikes);
    if now == ptype::STAND && d.ledge_guard && !d.stationary {
        let moved = f64::from(x) + d.carry.0 - p0.0;
        let before = Before {
            x: x0,
            y: y0,
            p0,
            v0y,
            standing,
        };
        ledge_guard(w, id, d, before, moved);
    }
    let o = obj(w, id);
    d.moved_to = Some((o.x, o.y));
    if d.z_sort {
        depth_sort(w, id, d);
    }
    death
}

/// Moves the enemy toward (tx, ty) through the tiles and platforms.
///
/// Only a falling or flying enemy, or one riding a platform, needs the
/// platforms: one standing on the tiles or moving without tile tests
/// moves exactly as with them, so it skips gathering them.
fn collide(w: &mut World, id: ObjectId, tx: i32, ty: i32) -> u32 {
    let o = obj(w, id);
    let tiles_only =
        o.flags & flags::ON_ELEVATOR == 0 && matches!(o.physics_type, ptype::STAND | ptype::DIRECT);
    if !tiles_only {
        return w.physics(id, tx, ty, opt::PLATFORMS);
    }
    let c = Collider {
        map: &w.level.map,
        platforms: &[],
        step: w.tuning.physics.step,
    };
    match w.objects.get_mut(id) {
        Some(o) => physics::physics(&c, o, tx, ty, opt::PLATFORMS),
        None => 0,
    }
}

/// Where an enemy was before its step: its position, its exact position,
/// its speed up or down, and whether it started the step standing.
#[derive(Debug, Clone, Copy)]
struct Before {
    x: i32,
    y: i32,
    p0: (f64, f64),
    v0y: f64,
    standing: bool,
}

/// The speed after a move of `d` px from speed `v0` under acceleration
/// `a`, with the sign of `v0` (negative when `v0` is 0): the speed the
/// energy of the move gives.
pub fn speed_after(v0: f64, a: f64, d: f64) -> f64 {
    let v = (v0 * v0 + 2.0 * a * d).max(0.0).sqrt();
    if v0 > 0.0 { v } else { -v }
}

/// A standing enemy whose leading edge (the side its exact position
/// `moved` to in the step) has no floor under it (a floor pixel with a
/// clear one above) goes back to where it was before the step and makes
/// that its patrol limit on that side. An enemy that entered the step in
/// the air also goes back up or down, and its vertical speed is corrected
/// for the move it made; one that entered it standing keeps its height,
/// which is where the platform it rides carried it.
fn ledge_guard(w: &mut World, id: ObjectId, d: &mut EnemyData, before: Before, moved: f64) {
    let o = obj(w, id);
    if moved == 0.0 || !o.move_rect.is_set() {
        return;
    }
    let edge = o.x
        + if moved < 0.0 {
            o.move_rect.left
        } else {
            o.move_rect.right
        };
    let feet = o.y + o.move_rect.bottom;
    let map = &w.level.map;
    if is_floor(map.attr(edge, feet + 1)) && map.attr(edge, feet) == CLEAR {
        return;
    }
    if moved > 0.0 {
        d.bounds.1 = before.x;
    } else {
        d.bounds.0 = before.x;
    }
    let o = obj_mut(w, id);
    o.x = before.x;
    d.carry.0 = 0.0;
    if before.standing {
        return;
    }
    let y = o.y;
    o.y = before.y;
    if y != before.y {
        d.carry.1 = 0.0;
        d.vy = speed_after(before.v0y, d.ay, f64::from(before.y) - before.p0.1);
    }
}

/// Drawn in front of Claw when his feet are at or above the enemy's,
/// behind him otherwise.
fn depth_sort(w: &mut World, id: ObjectId, d: &EnemyData) {
    let Some(claw_feet) = w.claw_object().map(|c| c.y + c.move_rect.bottom) else {
        return;
    };
    let o = obj_mut(w, id);
    let feet = o.y + o.move_rect.bottom;
    let z = d.z_base
        + if claw_feet <= feet {
            DEPTH_IN_FRONT
        } else {
            DEPTH_BEHIND
        };
    if o.z != z {
        o.z = z;
        o.flags |= flags::RESORT;
    }
}

/// True when the enemy stands on a narrow ledge: its own column is ledge,
/// and the ledge ends within 49 px on both sides. A column is ledge when
/// the row under the feet is floor (solid, ground or climb) and the pixel
/// one row up is clear, so a wall rising beside the enemy ends its ledge
/// even where the floor under the wall goes on.
pub fn on_narrow_ledge(w: &World, id: ObjectId) -> bool {
    let o = obj(w, id);
    if !o.move_rect.is_set() {
        return false;
    }
    let map = &w.level.map;
    let y = o.y + o.move_rect.bottom + 1;
    let ledge = |x: i32| ledge_column(map.attr(x, y), map.attr(x, y - 1));
    if !ledge(o.x) {
        return false;
    }
    let ends = |dir: i32| (1..=NARROW_REACH).any(|k| !ledge(o.x + dir * k));
    ends(-1) && ends(1)
}

/// A column the narrow-ledge test walks on: floor under the feet
/// (`under`) with clear air one pixel above it (`above`).
fn ledge_column(under: u8, above: u8) -> bool {
    is_floor(under) && above == CLEAR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_moves_at_the_mean_speed_and_caps_at_24_px() {
        let mut v = 120.0;
        let d = axis_step(&mut v, 0.0);
        assert!((d - 2.16).abs() < 1e-12);
        assert_eq!(v, 120.0);
        // Falling: the move uses the mean speed of the tick.
        let mut v = 0.0;
        let d = axis_step(&mut v, 2000.0);
        assert!((d - 0.324).abs() < 1e-12);
        assert!((v - 36.0).abs() < 1e-12);
        // Fast: capped, the speed from the energy of the capped move.
        let mut v = 2000.0;
        let d = axis_step(&mut v, 2000.0);
        assert_eq!(d, 24.0);
        assert!((v - (2000.0f64 * 2000.0 + 2.0 * 2000.0 * 24.0).sqrt()).abs() < 1e-9);
        let mut v = -2000.0;
        assert_eq!(axis_step(&mut v, 0.0), -24.0);
        assert_eq!(v, -2000.0);
    }

    #[test]
    fn a_step_covers_the_game_time_since_the_last_one() {
        let mut d = EnemyData::default();
        let tick = 18.0 * 0.001;
        // The first step covers one tick.
        assert_eq!(step_seconds(&mut d, 1000), tick);
        assert_eq!(step_seconds(&mut d, 1018), tick);
        // A second step in the same moment (a respawn's update passes,
        // with the clock standing still) covers none.
        assert_eq!(step_seconds(&mut d, 1018), 0.0);
        assert_eq!(step_seconds(&mut d, 1018), 0.0);
        // The next tick covers one tick again, and a gap all of it.
        assert_eq!(step_seconds(&mut d, 1036), tick);
        assert_eq!(step_seconds(&mut d, 1136), 100.0 * 0.001);
    }

    #[test]
    fn a_ledge_column_is_floor_with_clear_air_above() {
        for floor in [SOLID, GROUND, CLIMB] {
            assert!(ledge_column(floor, CLEAR));
            // A wall over the floor ends the ledge.
            assert!(!ledge_column(floor, SOLID));
            assert!(!ledge_column(floor, GROUND));
        }
        assert!(!ledge_column(CLEAR, CLEAR));
    }

    #[test]
    fn clamping_keeps_the_limits() {
        assert_eq!(clamp_to(10.5, (0, 10)), 10.0);
        assert_eq!(clamp_to(-3.0, (0, 10)), 0.0);
        assert_eq!(clamp_to(5.25, super::super::UNBOUNDED), 5.25);
    }
}
