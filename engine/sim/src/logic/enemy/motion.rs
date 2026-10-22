//! The movement step of enemies, run at the end of every post-tick, and
//! what follows it: death tiles, the ledge guard and the depth against
//! Claw.
//!
//! The step:
//!
//! 1. A frozen enemy (dead in place, or carried by Claw) does not move at
//!    all.
//! 2. One marked as riding a platform object (it landed or stands on a
//!    peg or an elevator) moves by the platform only: from its pixel
//!    position by the platform's last move, its exact position put on
//!    that pixel, with no fraction carried. Its own move of the step is
//!    dropped, though its speeds still take their step, and the mark ends
//!    after the step unless it stands. The tiles then have their say as
//!    below; when they move it off that pixel up or down, its vertical
//!    speed is the one that move gives from the speed after the step, and
//!    the ledge guard picks its edge by the dropped move across. The
//!    ledge guard looks at the tiles only and leaves the mark alone, so a
//!    thrown enemy landing on a peg that reaches past a ledge, put back in
//!    the air by the guard, is still marked: its first bounce step does
//!    not move it (it starts from the pixel it landed on, the bounce's
//!    1 px lift being on its exact height only), and a later bounce that
//!    starts with its feet a pixel above the peg lands on it again at once.
//! 3. Each axis moves by `(v + a dt / 2) dt`, at most 24 px; the speed
//!    then grows by `a dt` (after a capped step, it is what the energy of
//!    the capped move gives). Across there is no acceleration; up and down
//!    there is gravity. A standing enemy (not riding a platform) moves its
//!    exact height too, but the tiles keep it on its floor pixel; after
//!    the collision its exact height goes back onto that pixel once it has
//!    left it, with the upward speed that move back implies, so it carries
//!    an upward speed settling towards 36 px/s and starts a fall off a
//!    ledge with it. `dt` is the time on the game clock since the last
//!    step, the difference of two readings in double-precision seconds,
//!    each the clock (one tick ahead of the tick's start) times 0.001: an
//!    18 ms tick is 0.018000000000000682 s at 135.36 s. The clock starts
//!    when the enemy is built ([`super::start_move_clock`]), so its first
//!    step covers the time since then: one tick when it first moves in the
//!    tick after. After a gap (an enemy woken after sleeping outside its
//!    activation window) the step covers the whole time since the last
//!    one, so a woken walker first makes up the way it did not walk while
//!    asleep, up to the 24 px step limit (a rat asleep for 3.7 s moves
//!    24 px, then stops on its patrol limit). The sleep may start at the
//!    end of the tick it was built in: a rat built when its placement
//!    point reaches the edge of its activation window, then dropped to a
//!    floor just outside it, sleeps until the window reaches it again, and
//!    its first step covers all the time since it was built (six ticks,
//!    0.108 s, so 6.48 px at 60 px/s, not 1.08). A step with no game time
//!    since the last one, as in a respawn's update passes, covers none and
//!    moves nothing.
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
//!    speed across around at half. The step keeps what the tiles reported
//!    ([`super::EnemyFields::last_result`]): any stop, a wall, a ceiling or a
//!    landing, sets the stop bit, and a human enemy's move back or move
//!    ahead ends on it.
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
//!    no floor there and lands where it was, in the air. A standing enemy
//!    put back also has its exact height put back on its pixel.
//! 7. With the depth sort, it is drawn in front of Claw when his feet are
//!    at or above its own, behind him otherwise.
//!
//! When the enemy's own logic wrote a new position in the tick
//! ([`super::EnemyFields::teleport_to`], the Chameleon's phase), the step
//! starts from that x instead of where the object stands, with nothing
//! carried: the object moves from where it stands toward it through the
//! tiles, and that x counts as where it stood before the step, so a wall
//! that stops the move, or a ledge that sends it back, makes that x its
//! patrol limit on that side (the ledge also puts it there). Which edge
//! leads is still decided by where the object stood. The move itself
//! stops at the plane's edge: a target outside the plane (the Chameleon's
//! phase sends an unbounded lizard toward x = -2147483647) takes an
//! object on a floor that runs to the edge of the level there and no
//! further, instead of on along the edge's floor to the target.

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

/// The time the step at `now` covers, s: the game clock's reading now
/// less its reading at the last step (see [`clock_seconds`]), one tick or,
/// after a gap (the enemy slept, so no step ran for more than a tick), the
/// whole time since the last step, which only the [`STEP_LIMIT`] of the
/// move bounds. A step with no game time since the last one covers none:
/// a second step in the same moment, as in the update passes of a
/// respawn, where the clock stands still, moves nothing. The clock starts
/// when the enemy is built, so the first step covers the time since then.
/// Records `now` as the time of the last step.
fn step_seconds(d: &mut EnemyData, now: u64) -> f64 {
    step_seconds_since(&mut d.last_move_ms, now)
}

/// The time the movement step at `now` covers, s, for any mover that keeps
/// the time of its last step in `last_ms` (an enemy, or a crate of a stack,
/// which falls by the same clock): the game clock's reading now less its
/// reading then, none in the same moment. Both start the clock when they
/// are built; a mover with no time recorded yet covers one tick. Records
/// `now` in `last_ms`.
pub fn step_seconds_since(last_ms: &mut Option<u64>, now: u64) -> f64 {
    let last = last_ms.replace(now);
    let since = last.map_or(0, |t| now.saturating_sub(t));
    match last {
        Some(_) if since == 0 => 0.0,
        Some(last) => clock_seconds(now) - clock_seconds(last),
        None => f64::from(TICK_MS) * 0.001,
    }
}

/// The game clock at the step that runs at `now` (the tick's start, ms),
/// in seconds: the clock is one tick ahead then, and each reading is
/// rounded to a double on its own, so the time between two steps is the
/// difference of two rounded readings (0.018000000000000682 s at 135.36 s,
/// not 0.018).
fn clock_seconds(now: u64) -> f64 {
    (now + u64::from(TICK_MS)) as f64 * 0.001
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
    // A position its own logic wrote: the step starts from it.
    let jump = d.teleport_to.take();
    if jump.is_some() {
        d.carry.0 = 0.0;
    }
    let start_x = jump.map_or(f64::from(x0) + d.carry.0, f64::from);
    let before_x = jump.unwrap_or(x0);
    let dt = step_seconds(d, w.now_ms);
    let standing = entry == ptype::STAND;
    let lift = std::mem::take(&mut d.bounce_lift);
    if riding_mark(w, id) {
        let from = RideFrom {
            x: x0,
            y: y0 + i32::from(lift && !standing),
            start_x,
            before_x,
            standing,
        };
        return ride_step(w, id, d, from, ride, dt);
    }
    // Where the step starts from, exactly, and the speed up or down then.
    let p0 = (f64::from(x0) + d.carry.0, f64::from(y0) + d.carry.1);
    let v0y = d.vy;
    let px = clamp_to(
        start_x + f64::from(ride.0) + axis_step_for(&mut d.vx, 0.0, dt),
        d.bounds,
    );
    let tx = px.trunc() as i32;
    // A standing enemy's exact height still moves under gravity; the
    // tiles keep it on its floor, and after the collision the exact height
    // goes back there (see `settle_standing`). Not while it rides a
    // platform, which carries it by whole pixels.
    let riding = ride != (0, 0) || obj(w, id).flags & flags::ON_ELEVATOR != 0;
    let stand_y = (standing && !riding).then(|| {
        let ay = d.ay;
        clamp_to(
            f64::from(y0) + d.carry.1 + axis_step_for(&mut d.vy, ay, dt),
            d.bounds_y,
        )
    });
    let tx = if jump.is_some() {
        tx.clamp(0, w.level.map.width().saturating_sub(1))
    } else {
        tx
    };
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
    let r = collide(w, id, tx, ty, d.tiles_only);
    d.last_result = r;
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
    if let Some(moved) = stand_y {
        settle_standing(d, y, moved);
    }
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
    let death = after_tiles(w, id, d, r, before_x);
    if now == ptype::STAND && d.ledge_guard && !d.stationary {
        let moved = f64::from(x) + d.carry.0 - p0.0;
        let before = Before {
            x: before_x,
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

/// What the tiles reported (`r`) does to a move that tests them: a
/// ceiling turns the vertical speed around, a wall on either side makes
/// `before_x` that patrol limit and turns the speed across around at half,
/// and a death tile kills an enemy that is not dead yet.
fn after_tiles(
    w: &World,
    id: ObjectId,
    d: &mut EnemyData,
    r: u32,
    before_x: i32,
) -> Option<DeathKind> {
    if r & res::CEILING != 0 {
        d.vy = -d.vy;
    } else if r & res::WALL != 0 && r & res::RIGHT != 0 {
        d.bounds.1 = before_x;
        d.vx *= -0.5;
    } else if r & res::WALL != 0 && r & res::LEFT != 0 {
        d.bounds.0 = before_x;
        d.vx *= -0.5;
    }
    let state = super::EnemyState::from_id(obj(w, id).state);
    (r & res::DEATH_TILE != 0 && state != Some(EnemyState::Dead) && !d.dead)
        .then_some(DeathKind::Spikes)
}

/// True while the enemy is marked as riding a platform object: the mark
/// set, with an object below it.
fn riding_mark(w: &World, id: ObjectId) -> bool {
    let o = obj(w, id);
    o.flags & flags::ON_ELEVATOR != 0 && o.object_below.is_some()
}

/// Where the step of an enemy marked as riding starts: its pixel position
/// (`y` is the pixel it landed on when the landing lifted it into a
/// bounce in this tick, since the lift raised only its exact height), the
/// x its exact position starts from, where it stood before the step
/// across, and whether it entered the step standing.
#[derive(Debug, Clone, Copy)]
struct RideFrom {
    x: i32,
    y: i32,
    start_x: f64,
    before_x: i32,
    standing: bool,
}

/// The step of an enemy marked as riding a platform object (see the
/// module documentation): its own move of the step is dropped, and it
/// moves from its pixel position by the platform's last move (`ride`)
/// only, its exact position on the pixel it moves to. Its speeds still
/// take their step. The mark ends here unless it stands; the tiles then
/// have their say as in any step, and when they move it off that pixel up
/// or down its vertical speed is the one that move gives from the speed
/// after the step. The ledge guard picks its edge by the dropped move
/// across.
fn ride_step(
    w: &mut World,
    id: ObjectId,
    d: &mut EnemyData,
    from: RideFrom,
    ride: (i32, i32),
    dt: f64,
) -> Option<DeathKind> {
    let standing = from.standing;
    let own_dx =
        clamp_to(from.start_x + axis_step_for(&mut d.vx, 0.0, dt), d.bounds) - from.start_x;
    let tx = from.x + ride.0;
    let ty = from.y + ride.1;
    if standing {
        d.vy = 0.0;
    } else {
        let ay = d.ay;
        axis_step_for(&mut d.vy, ay, dt);
    }
    {
        let o = obj_mut(w, id);
        o.flags &= !flags::ON_DEATH_TILE;
        if !standing {
            o.flags &= !flags::ON_ELEVATOR;
        }
    }
    let r = collide(w, id, tx, ty, d.tiles_only);
    d.last_result = r;
    d.carry = (0.0, 0.0);
    let (x, y, now) = {
        let o = obj(w, id);
        (o.x, o.y, o.physics_type)
    };
    if now == ptype::DIRECT {
        d.moved_to = Some((x, y));
        if d.z_sort {
            depth_sort(w, id, d);
        }
        return None;
    }
    let p = (f64::from(tx), f64::from(ty));
    if !standing && y != ty {
        d.vy = speed_after(d.vy, d.ay, f64::from(y) - p.1);
    }
    let death = after_tiles(w, id, d, r, from.before_x);
    if now == ptype::STAND && d.ledge_guard && !d.stationary {
        let before = Before {
            x: from.before_x,
            y: from.y,
            p0: p,
            v0y: d.vy,
            standing,
        };
        ledge_guard(w, id, d, before, own_dx);
    }
    let o = obj(w, id);
    d.moved_to = Some((o.x, o.y));
    if d.z_sort {
        depth_sort(w, id, d);
    }
    death
}

/// Moves the enemy toward (tx, ty) through the tiles and platforms; an
/// enemy that lands only on the tiles (`tiles_only`, a hermit crab) moves
/// through the tiles alone, falling through every platform object.
///
/// Only a falling or flying enemy, or one riding a platform, needs the
/// platforms: one standing on the tiles or moving without tile tests
/// moves exactly as with them, so it skips gathering them.
fn collide(w: &mut World, id: ObjectId, tx: i32, ty: i32, tiles_only: bool) -> u32 {
    let mask = if tiles_only { 0 } else { opt::PLATFORMS };
    let o = obj(w, id);
    let no_platforms = tiles_only
        || (o.flags & flags::ON_ELEVATOR == 0
            && matches!(o.physics_type, ptype::STAND | ptype::DIRECT));
    if !no_platforms {
        return w.physics(id, tx, ty, mask);
    }
    let c = Collider {
        map: &w.level.map,
        platforms: &[],
        step: w.tuning.physics.step,
    };
    match w.objects.get_mut(id) {
        Some(o) => physics::physics(&c, o, tx, ty, mask),
        None => 0,
    }
}

/// After the step of a standing enemy whose exact height moved to `moved`
/// while the tiles kept it on the floor at `y`: when `moved` no longer
/// truncates to `y`, the exact height goes back onto `y` and the vertical
/// speed is the one that move back implies, `-sqrt(2 a (y - moved))` (0
/// when that is not a move up, and without gravity); otherwise `moved`
/// stays as the carried fraction and the speed stays 0. So a standing
/// enemy under 2000 px/s² carries an upward speed that settles towards
/// 36 px/s and starts a fall off a ledge with it.
fn settle_standing(d: &mut EnemyData, y: i32, moved: f64) {
    if moved.trunc() as i32 == y {
        d.carry.1 = moved - f64::from(y);
        return;
    }
    d.carry.1 = 0.0;
    d.vy = if d.ay == 0.0 {
        0.0
    } else {
        -(2.0 * d.ay * (f64::from(y) - moved)).max(0.0).sqrt()
    };
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
        d.carry.1 = 0.0;
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
    use super::super::EnemyFields;
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
        let reading = |ms: u64| (ms + 18) as f64 * 0.001;
        // With no time recorded (built enemies have the time they were
        // built), the first step covers one tick.
        assert_eq!(step_seconds(&mut d, 1000), 18.0 * 0.001);
        assert_eq!(step_seconds(&mut d, 1018), reading(1018) - reading(1000));
        // A second step in the same moment (a respawn's update passes,
        // with the clock standing still) covers none.
        assert_eq!(step_seconds(&mut d, 1018), 0.0);
        assert_eq!(step_seconds(&mut d, 1018), 0.0);
        // The next tick covers one tick again, and a gap all of it.
        assert_eq!(step_seconds(&mut d, 1036), reading(1036) - reading(1018));
        assert_eq!(step_seconds(&mut d, 1136), reading(1136) - reading(1036));
    }

    #[test]
    fn the_step_time_is_the_difference_of_two_clock_readings() {
        // The tick that starts at 135342 ms runs with the clock at 135.36 s
        // and the last step's reading 135.342 s: the step covers
        // 0.018000000000000682 s, not 18 ms times 0.001.
        let mut d = EnemyData::from(EnemyFields {
            last_move_ms: Some(135_324),
            ..EnemyFields::default()
        });
        assert_eq!(clock_seconds(135_342), 135.36);
        let dt = step_seconds(&mut d, 135_342);
        assert_eq!(dt, 0.018_000_000_000_000_682);
        assert_ne!(dt, 18.0 * 0.001);
        // So a walker covers slightly more or less than 18 ms worth per
        // tick, by the rounding of the two readings.
        let mut d = EnemyData::default();
        step_seconds(&mut d, 18 * 3538);
        let walked: f64 = (3539..=3594u64)
            .map(|k| 120.0 * step_seconds(&mut d, 18 * k))
            .sum();
        assert_ne!(walked, 56.0 * 120.0 * (18.0 * 0.001));
    }

    #[test]
    fn a_standing_enemy_is_put_back_on_its_floor_with_an_upward_speed() {
        let mut d = EnemyData::from(EnemyFields {
            ay: 2000.0,
            ..EnemyFields::default()
        });
        // Built 27 px above its floor at 5416: the first tick moves the
        // exact height 0.324 px down, to 5389.324, and it goes back on the
        // floor at 326.66 px/s up.
        let mut v = 0.0;
        let moved = 5389.0 + axis_step_for(&mut v, 2000.0, 0.018);
        settle_standing(&mut d, 5416, moved);
        assert_eq!(d.carry.1, 0.0);
        assert!((d.vy + (4000.0f64 * 26.676).sqrt()).abs() < 1e-9);
        // Standing on, the speed settles towards 36 px/s up.
        for _ in 0..2000 {
            let mut v = d.vy;
            let moved = 5416.0 + d.carry.1 + axis_step_for(&mut v, 2000.0, 0.018);
            d.vy = 0.0;
            settle_standing(&mut d, 5416, moved);
        }
        assert!(d.vy < -36.0 && d.vy > -36.5, "{}", d.vy);
        // A move down within the pixel stays as the carried fraction, with
        // no speed; one that leaves it downward is put back with none.
        let mut d = EnemyData::from(EnemyFields {
            ay: 2000.0,
            ..EnemyFields::default()
        });
        settle_standing(&mut d, 5416, 5416.324);
        assert!((d.carry.1 - 0.324).abs() < 1e-9);
        assert_eq!(d.vy, 0.0);
        settle_standing(&mut d, 5416, 5417.2);
        assert_eq!((d.carry.1, d.vy), (0.0, 0.0));
        // Without gravity there is no speed to give back.
        let mut d = EnemyData::from(EnemyFields {
            ay: 0.0,
            ..EnemyFields::default()
        });
        settle_standing(&mut d, 100, 99.5);
        assert_eq!((d.carry.1, d.vy), (0.0, 0.0));
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
