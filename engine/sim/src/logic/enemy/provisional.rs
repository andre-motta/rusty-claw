//! Shared enemy behaviour that is provisional: striking, pacing, flinching,
//! dying, pausing, dropping loot and moving.
//!
//! These rules follow the shared enemy behaviour as it is understood now,
//! with a few simplifications that the full enemy work replaces:
//!
//! - Movement is this file's own step: horizontal speed through the tile
//!   physics while standing, gravity while falling, the patrol limits
//!   clamping the position (a wall or a ledge moves the limit to where the
//!   enemy stood, so unbounded walkers learn their platform's edges), and
//!   no tiles at all for a body flying off after death. Platforms do not
//!   carry enemies yet: an enemy standing on one stays put and never paces.
//! - Each enemy has its own strike token (one armed strike per swing) and
//!   its own speech slot, where the game shares one of each across the
//!   level: two enemies swinging at the same moment can both hurt Claw, and
//!   two can talk at once.
//! - Jumping at Claw from a narrow ledge, being lifted, carried, thrown and
//!   dropped, fire and ice deaths and the exclamation mark over a speaking
//!   enemy are not filled in.
//!
//! The states:
//!
//! - Strike: the strike animation plays; on its active frame the strike box
//!   becomes the enemy's attack box for that tick and the strike sound
//!   plays; when it ends the enemy goes Home.
//! - Pace: walking at 120 px/s; at a patrol limit it goes idle or turns
//!   (even odds); back Home when Claw comes in range.
//! - Hit (low or high): knocked away from Claw at 200 px/s, slowing by
//!   25 px/s each tick down to a stop; Home when the animation ends.
//! - Dead: a killed body flies up and away from Claw and falls through the
//!   floor; it is removed with a splash once off screen. A death on spikes
//!   or after a throw stays in place and is removed after a second.
//! - Pause: waits, then goes back to the previous state.

use crate::logic::{World, pickup};
use crate::object::{ObjectId, Rect, flags, special};
use crate::physics::{self, opt, ptype, res};

use super::{
    Class, DeathKind, EnemyData, EnemyState, GRAVITY, HIT_SOUNDS, TICK_S, Timer, UNBOUNDED,
    attacker, class_of, data, data_mut, facing_rect, obj, obj_mut, on_screen, play, rand,
    set_facing, set_state, with_data,
};

/// Pacing speed, px/s.
pub const PACE_SPEED: f64 = 120.0;
/// Knockback speed of a hit, px/s.
pub const KNOCKBACK: f64 = 200.0;
/// Knockback slowdown per tick, px/s.
pub const KNOCKBACK_STEP: f64 = 25.0;
/// Speed of a killed body, px/s across and up.
pub const DEATH_LAUNCH: (f64, f64) = (150.0, -700.0);
/// Depth of a killed body: in front of everything.
pub const DEATH_DEPTH: i32 = 8500;
/// Removal delay of a death on spikes or after a throw, ms.
pub const DEATH_DELAY_MS: u64 = 1000;
/// Depth above the base of an enemy drawn in front of Claw.
const DEPTH_IN_FRONT: i32 = 4000;
/// Depth above the base of an enemy drawn behind Claw.
const DEPTH_BEHIND: i32 = 3899;

/// The shared handler of a state.
pub fn handler(s: EnemyState) -> fn(&mut World, ObjectId) {
    match s {
        EnemyState::Strike => strike_tick,
        EnemyState::Pace => pace_tick,
        EnemyState::HitLow | EnemyState::HitHigh => hit_tick,
        EnemyState::Dead => dead_tick,
        EnemyState::Pause => pause_tick,
        EnemyState::Idle | EnemyState::Home => nothing,
    }
}

fn nothing(_: &mut World, _: ObjectId) {}

/// Goes Home: stops and decides again on the next tick.
pub fn go_home(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
        d.vy = 0.0;
    }
    set_state(w, id, EnemyState::Home);
}

/// Starts a strike: busy, the strike animation from the start.
pub fn enter_strike(w: &mut World, id: ObjectId) {
    let anim = data(w, id).map_or("", |d| d.strike_anim);
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    set_state(w, id, EnemyState::Strike);
    w.set_anim(id, anim);
}

/// Strike: on the active frame, with the token, the strike box is live and
/// the strike sound plays; at the end, Home.
pub fn strike_tick(w: &mut World, id: ObjectId) {
    let event = w.advance_anim(id);
    let Some(class) = class_of(w, id) else { return };
    let (token, rect) =
        data(w, id).map_or((false, Rect::UNSET), |d| (d.strike_token, d.strike_rect));
    if event == 1 && token {
        if let Some(d) = data_mut(w, id) {
            d.strike_token = false;
        }
        play(w, class.sounds.strike);
        let o = obj_mut(w, id);
        o.attack_rect = facing_rect(o, rect);
    }
    if obj(w, id).anim.is_done() {
        if let Some(d) = data_mut(w, id) {
            d.strike_token = true;
        }
        go_home(w, id);
        if let Some(d) = data_mut(w, id) {
            d.busy = false;
        }
    }
}

/// Starts pacing: a stationary enemy goes idle instead; otherwise it walks
/// left from its right limit and right from anywhere else.
pub fn start_pacing(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let (stationary, max) = data(w, id).map_or((true, 0), |d| (d.stationary, d.bounds.1));
    if stationary {
        (class.go_idle)(w, id);
        return;
    }
    let right = obj(w, id).x != max;
    if let Some(d) = data_mut(w, id) {
        d.vx = if right { PACE_SPEED } else { -PACE_SPEED };
    }
    set_facing(w, id, right);
    set_state(w, id, EnemyState::Pace);
    w.set_anim(id, class.anims.pace);
}

/// Pace: at the left limit, even odds of going idle or turning right; then
/// the same at the right limit; Home when Claw is in range. Draws: one per
/// limit reached.
pub fn pace_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let Some(class) = class_of(w, id) else { return };
    let (min, max) = data(w, id).map_or(UNBOUNDED, |d| d.bounds);
    if obj(w, id).x == min {
        if rand(w, 100) < 50 {
            (class.go_idle)(w, id);
        } else {
            if let Some(d) = data_mut(w, id) {
                d.vx = PACE_SPEED;
            }
            set_facing(w, id, true);
        }
    }
    if obj(w, id).x == max {
        if rand(w, 100) < 50 {
            (class.go_idle)(w, id);
        } else {
            if let Some(d) = data_mut(w, id) {
                d.vx = -PACE_SPEED;
            }
            set_facing(w, id, false);
        }
    }
    if data(w, id).is_some_and(|d| d.mode <= 4) {
        go_home(w, id);
    }
}

/// Starts a flinch, high or low: busy, its animation and sounds, and a
/// knockback away from Claw. Draws: one for a high flinch's sound.
pub fn enter_hit(w: &mut World, id: ObjectId, high: bool, class: &Class) {
    if let Some(d) = data_mut(w, id) {
        d.busy = true;
    }
    if high {
        w.set_anim(id, class.anims.hit_high);
        play(w, class.sounds.hit_high);
        let pick = rand(w, 4) as usize;
        play(w, HIT_SOUNDS[pick]);
    } else {
        w.set_anim(id, class.anims.hit_low);
        play(w, class.sounds.hit_low[0]);
        play(w, class.sounds.hit_low[1]);
    }
    if let Some(d) = data_mut(w, id) {
        d.vx = if d.claw_right { -KNOCKBACK } else { KNOCKBACK };
    }
    set_state(
        w,
        id,
        if high {
            EnemyState::HitHigh
        } else {
            EnemyState::HitLow
        },
    );
}

/// Hit: the knockback slows by 25 px/s per tick (by which side Claw is on
/// now) until it is exactly 0; Home when the animation ends.
pub fn hit_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if obj(w, id).anim.is_done() {
        go_home(w, id);
        if let Some(d) = data_mut(w, id) {
            d.busy = false;
        }
        return;
    }
    if let Some(d) = data_mut(w, id)
        && d.vx != 0.0
    {
        d.vx += if d.claw_right {
            KNOCKBACK_STEP
        } else {
            -KNOCKBACK_STEP
        };
    }
}

/// Dies: human enemies drop their loot (once), then the enemy is dead,
/// busy and harmless. A kill by a hit throws the body up and away from
/// Claw with no tile collision; then either Claw says a line (a quarter of
/// the time, for kills by his blows or pistol) or the death sound plays.
/// Spikes and goo freeze the body in place.
///
/// Draws: the loot's, then one of 0..100 for a kill by a hit, and one of
/// 0..10 when Claw speaks.
pub fn die(w: &mut World, id: ObjectId, kind: DeathKind, by: Option<ObjectId>) {
    if data(w, id).is_some_and(|d| !d.dead) {
        drop_loot(w, id);
    }
    let by_type = by
        .and_then(|a| w.objects.get(a))
        .map_or(0, |a| a.type_flags);
    if let Some(d) = data_mut(w, id) {
        d.death = kind;
        d.busy = true;
        d.cooldown = Timer::default();
    }
    set_state(w, id, EnemyState::Dead);
    obj_mut(w, id).flags |= flags::HARMLESS;
    let now = w.now_ms;
    match kind {
        DeathKind::Hit => {
            let o = obj_mut(w, id);
            o.z = DEATH_DEPTH;
            o.flags |= flags::RESORT;
            if rand(w, 100) < 25 {
                if by_type == attacker::CLAW || by_type == attacker::PISTOL_SHOT {
                    let line = super::CLAW_KILL_LINES[rand(w, 10) as usize];
                    crate::logic::sound::play(w, line, claw_frame::SoundChannel::Voice);
                }
            } else {
                let sound = data(w, id).map_or("", |d| d.death_sound);
                play(w, sound);
            }
            let anim = data(w, id).map_or("", |d| d.death_anim);
            w.set_anim(id, anim);
            obj_mut(w, id).physics_type = ptype::DIRECT;
            if let Some(d) = data_mut(w, id) {
                d.z_sort = false;
                d.bounds = UNBOUNDED;
                d.ay = GRAVITY;
                d.vx = if d.claw_right {
                    -DEATH_LAUNCH.0
                } else {
                    DEATH_LAUNCH.0
                };
                d.vy = DEATH_LAUNCH.1;
            }
        }
        DeathKind::Landing | DeathKind::Spikes | DeathKind::Goo => {
            if kind != DeathKind::Landing {
                play(w, super::DEATH_TILE_SOUND);
            }
            if let Some(d) = data_mut(w, id) {
                d.frozen = true;
                d.death_timer = Timer::new(now, DEATH_DELAY_MS);
            }
            if kind == DeathKind::Goo {
                w.destroy(id);
            } else {
                obj_mut(w, id).draw_flags |= crate::object::draw_flags::FLASH;
            }
        }
    }
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
    }
}

/// Dead: a killed body is removed with a splash once off screen; a frozen
/// one after its delay.
pub fn dead_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let Some((kind, timer)) = data(w, id).map(|d| (d.death, d.death_timer)) else {
        return;
    };
    match kind {
        DeathKind::Hit => {
            if !on_screen(w, obj(w, id)) {
                play(w, super::SPLASH_SOUND);
                w.destroy(id);
            }
        }
        DeathKind::Landing | DeathKind::Spikes => {
            if timer.done(now) {
                w.destroy(id);
            }
        }
        DeathKind::Goo => {}
    }
}

/// Starts a pause of `ms`.
pub fn enter_pause(w: &mut World, id: ObjectId, ms: u64) {
    let now = w.now_ms;
    if let Some(d) = data_mut(w, id) {
        d.pause = Timer::new(now, ms);
    }
    set_state(w, id, EnemyState::Pause);
}

/// Pause: back to the previous state when the time is up.
pub fn pause_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((pause, prev)) = data(w, id).map(|d| (d.pause, d.prev)) else {
        return;
    };
    if pause.done(now) {
        set_state(w, id, prev);
    }
}

/// Drops the enemy's loot: the drop sound once, then one pickup per filled
/// slot, fanned out 25 px/s apart (a single one gets a random speed across)
/// and thrown up at random. Draws per filled slot: the speed across (single
/// slot only), then the speed up; they are made even when no pickup
/// appears.
pub fn drop_loot(w: &mut World, id: ObjectId) {
    let Some(loot) = data(w, id).map(|d| d.loot) else {
        return;
    };
    let n = loot.iter().filter(|&&c| c != 0).count() as i32;
    if n == 0 {
        return;
    }
    play(w, super::DROP_SOUND);
    let (x, y) = {
        let o = obj(w, id);
        (o.x, o.y)
    };
    for (i, &code) in loot.iter().enumerate() {
        if code == 0 {
            continue;
        }
        let vx = if n == 1 {
            rand(w, 401) - 200
        } else {
            -(25 * n) / 2 + 25 * i as i32
        };
        let vy = -(rand(w, 301) + 200);
        if let Some(p) = pickup::spawn_drop(w, x, y, code)
            && let Some(o) = w.objects.get_mut(p)
        {
            o.speed_x = vx;
            o.speed_y = vy;
        }
    }
}

// ----------------------------------------------------------------------
// Movement

/// Whole pixels to move this tick at `v` px/s, carrying the fraction.
fn step(v: f64, carry: &mut f64) -> i32 {
    let d = v * TICK_S + *carry;
    let whole = d.trunc();
    *carry = d - whole;
    whole as i32
}

/// The movement step after the state handler.
///
/// A frozen enemy does not move. A body flying off after death moves with
/// its speed and gravity through everything. Otherwise a standing enemy
/// moves across through the tile physics (patrol limits clamp it), and one
/// with nothing under its feet falls with gravity until it lands. A wall
/// moves the patrol limit on that side to where the enemy stood; so does a
/// ledge under the front edge of a walker with the ledge guard, which then
/// stays where it was. Spikes or goo under a standing enemy kill it. Last,
/// with the depth sort, the enemy is drawn in front of Claw when his feet
/// are at or above its own, behind him otherwise.
pub fn integrate(w: &mut World, id: ObjectId) {
    let spikes = with_data(w, id, |w, d| {
        let spikes = !d.frozen && move_enemy(w, id, d);
        if d.z_sort {
            depth_sort(w, id, d);
        }
        spikes
    });
    if spikes == Some(true) {
        die(w, id, DeathKind::Spikes, None);
    }
}

/// Moves the enemy one tick; true when it stands on spikes or goo and
/// dies of it.
fn move_enemy(w: &mut World, id: ObjectId, d: &mut EnemyData) -> bool {
    let (x0, y0, pt) = {
        let o = obj(w, id);
        (o.x, o.y, o.physics_type)
    };
    if pt == ptype::DIRECT {
        d.vy += d.ay * TICK_S;
        let dx = step(d.vx, &mut d.carry.0);
        let dy = step(d.vy, &mut d.carry.1);
        let o = obj_mut(w, id);
        o.x += dx;
        o.y += dy;
        return false;
    }
    let falling = pt == ptype::FALL;
    if falling {
        d.vy += d.ay * TICK_S;
    } else {
        d.vy = 0.0;
        d.carry.1 = 0.0;
    }
    let dx = step(d.vx, &mut d.carry.0);
    let dy = if falling {
        step(d.vy, &mut d.carry.1)
    } else {
        0
    };
    let (min, max) = d.bounds;
    let tx = (x0 + dx).clamp(min, max.max(min));
    if tx == x0 && dy == 0 && !falling {
        // Standing still: only check for a floor.
        let r = w.physics(id, x0, y0, opt::PLATFORMS);
        return after_move(w, id, d, r, (x0, y0), 0);
    }
    let r = w.physics(id, tx, y0 + dy, opt::PLATFORMS);
    after_move(w, id, d, r, (x0, y0), tx - x0)
}

fn after_move(
    w: &mut World,
    id: ObjectId,
    d: &mut EnemyData,
    r: u32,
    (x0, y0): (i32, i32),
    dx: i32,
) -> bool {
    if r & res::WALL != 0 && dx != 0 {
        if dx > 0 {
            d.bounds.1 = x0;
        } else {
            d.bounds.0 = x0;
        }
        obj_mut(w, id).x = x0;
        d.carry.0 = 0.0;
    }
    let pt = obj(w, id).physics_type;
    if pt != ptype::FALL {
        d.vy = 0.0;
    }
    if d.ledge_guard && !d.stationary && dx != 0 {
        let o = obj(w, id);
        let edge = if dx < 0 {
            o.move_rect.left
        } else {
            o.move_rect.right
        };
        if o.move_rect.is_set() && !physics::support_at(&w.level.map, o, edge) {
            if dx > 0 {
                d.bounds.1 = x0;
            } else {
                d.bounds.0 = x0;
            }
            let o = obj_mut(w, id);
            o.x = x0;
            o.y = y0;
            o.physics_type = ptype::STAND;
            d.carry.0 = 0.0;
        }
    }
    r & res::DEATH_TILE != 0 && obj(w, id).physics_type == ptype::STAND && !d.dead
}

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

/// Sets the liftable flag from the enemy's data.
pub fn update_liftable(w: &mut World, id: ObjectId) {
    let may = data(w, id).is_some_and(|d| d.may_lift && !d.dead);
    let o = obj_mut(w, id);
    if may {
        o.special_flags |= special::LIFTABLE;
    } else {
        o.special_flags &= !special::LIFTABLE;
    }
}
