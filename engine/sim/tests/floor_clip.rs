//! The embedded fall: a Claw whose movement box ends up inside Solid or
//! one-way Ground tiles falls through every such tile until his box is
//! clear, then lands normally.
//!
//! The way into it in play is a hit taken while crouched: the hit keeps
//! the crouched box (top 10 px below Claw's position instead of 52 px
//! above it), so the push slides him under a ceiling lower than his
//! standing height, onto a floor higher than his feet. When the hit ends
//! he stands up wherever he is, with the standing box; inside the ceiling
//! and the floor, nothing holds him and he falls. A Claw put down with his
//! feet inside a floor falls through it the same way. A Claw whose box is
//! clear is never moved by any of this.
//!
//! Every level here is synthetic: 64 px tiles built in code.

mod common;

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::object::Rect;
use claw_sim::physics::ptype;
use claw_sim::player::{special, state};
use claw_sim::tiles::{CLEAR, Descriptor, GROUND, SOLID, TileMap};
use claw_sim::tuning::Tuning;
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// Tile size.
const T: i32 = 64;
/// Top of the main floor (row 10): Claw standing on it is at y 579.
const FLOOR_TOP: i32 = 10 * T;
/// Claw's y standing on the main floor.
const ON_FLOOR: i32 = FLOOR_TOP - 61;
/// Bottom of the low ceiling over the nook (row 8, columns 4 to 7).
const CEILING_BOTTOM: i32 = 9 * T - 1;
/// Top of the nook's floor: a one-way Ground strip 10 px higher than the
/// main floor, in row 9 of columns 4 to 7.
const NOOK_FLOOR_TOP: i32 = FLOOR_TOP - 10;
/// Right edge of the nook (and of its ceiling).
const NOOK_RIGHT: i32 = 8 * T - 1;
/// Top of the hall's floor (row 16), under the stacked Solid of rows 10
/// to 12 and the clear air of rows 13 to 15.
const HALL_TOP: i32 = 16 * T;
/// Claw's y standing on the hall's floor.
const ON_HALL: i32 = HALL_TOP - 61;
/// Where Claw starts: his standing box's left edge 1 px right of the
/// ceiling over the nook.
const START_X: i32 = NOOK_RIGHT + 1 + 16;

/// The hit's push: 300 ms of the push frame (17 ticks at 3 px), then
/// 200 ms of standing still.
const HIT_FRAMES: &[(i32, i32)] = &[(200, 300), (201, 200)];

fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

/// The hit animations (event 1 on the push frame) and the others the
/// states here play.
fn add_anims(level: &mut SimLevel) {
    let mut v = Vec::new();
    for key in special::HIT_ANIMS {
        let mut a = Animation::once(key, HIT_FRAMES);
        a.frames[0].event = special::KNOCKBACK_EVENT;
        v.push(Arc::new(a));
    }
    v.push(anim("CLAW_STAND", &[(11, 110), (12, 110)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250)], false));
    v.push(anim("CLAW_FALL", &[(278, 65), (280, 80)], false));
    v.push(anim("CLAW_LAND", &[(290, 50)], true));
    for a in v {
        level.animations.insert(a.key.clone(), a);
    }
}

/// The nook level: columns 0 to 3 a Solid wall, a low ceiling over
/// columns 4 to 7 (row 8) with the nook's higher one-way floor under it
/// (row 9), the main floor from column 8 on (row 10), stacked Solid under
/// all of it (rows 10 to 12), clear air (rows 13 to 15) and the hall's
/// floor (row 16).
fn nook_level() -> SimLevel {
    let cols = 30;
    let mut rows: Vec<String> = vec![".".repeat(cols); 17];
    for r in rows.iter_mut().take(17) {
        r.replace_range(0..4, "####");
    }
    rows[8].replace_range(4..8, "####");
    for r in rows.iter_mut().take(13).skip(10) {
        r.replace_range(4..cols, &"#".repeat(cols - 4));
    }
    rows[16] = "#".repeat(cols);
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut map = TileMap::from_ascii(T, &rows);
    let strip = map.add_descriptor(Descriptor::Double {
        width: T,
        height: T,
        outside: CLEAR,
        inside: GROUND,
        rect: [0, NOOK_FLOOR_TOP - 9 * T, T - 1, T - 1],
    });
    for c in 4..8 {
        map.set_cell(c, 9, strip);
    }
    let mut level = SimLevel::synthetic(map, (START_X, 500));
    add_anims(&mut level);
    level
}

/// A flat level with the same animations: the main floor only.
fn flat() -> SimLevel {
    let mut level = flat_level();
    add_anims(&mut level);
    level
}

fn new_sim(level: SimLevel) -> Sim {
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            tuning: Tuning::default(),
            ..SimConfig::default()
        },
    );
    for _ in 0..20 {
        sim.step(TickInput::default());
    }
    sim
}

fn claw(sim: &Sim) -> &claw_sim::object::Object {
    sim.player().expect("Claw")
}

fn hold(mask: u32, first: bool) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons(if first { mask } else { 0 }),
    }
}

/// Crouches Claw where he stands and lets the crouch settle.
fn crouch(sim: &mut Sim) {
    sim.step(hold(DOWN, true));
    for _ in 0..5 {
        sim.step(hold(DOWN, false));
    }
    assert_eq!(claw(sim).state, state::CROUCH);
}

/// What one tick shows of Claw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Look {
    x: i32,
    y: i32,
    state: i32,
    physics_type: i32,
    move_rect: Rect,
}

/// Plays `n` ticks holding `mask` and returns each tick's look.
fn play(sim: &mut Sim, n: usize, mask: u32) -> Vec<Look> {
    (0..n)
        .map(|_| {
            sim.step(hold(mask, false));
            let c = claw(sim);
            Look {
                x: c.x,
                y: c.y,
                state: c.state,
                physics_type: c.physics_type,
                move_rect: c.move_rect,
            }
        })
        .collect()
}

/// Hits Claw from `dx` px to his right (a negative `dx`: his left).
fn hit_from(sim: &mut Sim, dx: i32) {
    let x = claw(sim).x;
    sim.with_world(|w| assert!(w.claw_hit_reaction(x + dx)));
}

#[test]
fn the_nook_is_built_as_described() {
    let level = nook_level();
    let m = &level.map;
    assert_eq!(m.attr(START_X, ON_FLOOR + 60), CLEAR);
    assert_eq!(m.attr(START_X, FLOOR_TOP), SOLID);
    assert_eq!(m.attr(NOOK_RIGHT, CEILING_BOTTOM), SOLID);
    assert_eq!(m.attr(NOOK_RIGHT, CEILING_BOTTOM + 1), CLEAR);
    assert_eq!(m.attr(NOOK_RIGHT, NOOK_FLOOR_TOP - 1), CLEAR);
    assert_eq!(m.attr(NOOK_RIGHT, NOOK_FLOOR_TOP), GROUND);
    assert_eq!(m.attr(NOOK_RIGHT, FLOOR_TOP), SOLID);
    assert_eq!(m.attr(NOOK_RIGHT, 13 * T - 1), SOLID);
    assert_eq!(m.attr(NOOK_RIGHT, 13 * T), CLEAR);
    assert_eq!(m.attr(NOOK_RIGHT, HALL_TOP), SOLID);
    // The standing box reaches into the ceiling, the crouched one does
    // not.
    let t = Tuning::default();
    assert!(ON_FLOOR + t.player.move_rect.top <= CEILING_BOTTOM);
    assert!(ON_FLOOR + t.player.crouch_move_rect.top > CEILING_BOTTOM);
}

#[test]
fn a_standing_claw_stops_at_the_low_ceiling() {
    let mut sim = new_sim(nook_level());
    assert_eq!((claw(&sim).x, claw(&sim).y), (START_X, ON_FLOOR));
    let rows = play(&mut sim, 40, LEFT);
    // His standing box meets the ceiling's edge: he walks no further.
    assert!(rows.iter().all(|l| l.x == START_X && l.y == ON_FLOOR));
}

#[test]
fn a_crouched_hit_slides_claw_under_the_low_ceiling_and_he_falls_through() {
    let t = Tuning::default();
    let mut sim = new_sim(nook_level());
    crouch(&mut sim);
    assert_eq!(claw(&sim).move_rect, t.player.crouch_move_rect);
    hit_from(&mut sim, 50);
    let rows = play(&mut sim, 120, DOWN);
    // The hit keeps the crouched box: 3 px a tick to the left, under the
    // ceiling, for the 17 ticks of the push.
    let push = 17;
    for (k, l) in rows[..push].iter().enumerate() {
        assert_eq!(l.state, state::HIT, "tick {k}");
        assert_eq!(l.move_rect, t.player.crouch_move_rect, "tick {k}");
        assert_eq!(l.x, START_X - 3 * (k as i32 + 1), "tick {k}");
        assert_eq!(l.y, ON_FLOOR, "the hit never moves him up or down");
    }
    let under = START_X - 3 * push as i32;
    assert!(
        under + t.player.move_rect.right < NOOK_RIGHT,
        "all of him in the nook"
    );
    // Over the nook's floor his feet are inside it: nothing holds him, but
    // the hit keeps him where he is.
    let end = rows.iter().position(|l| l.state != state::HIT).unwrap();
    assert!(
        rows[push..end]
            .iter()
            .all(|l| l.x == under && l.y == ON_FLOOR)
    );
    assert_eq!(rows[end - 1].physics_type, ptype::FALL);
    // The hit ends: he stands up there with the standing box, his head in
    // the ceiling and his feet in the floor.
    assert_eq!(rows[end].state, state::STAND);
    assert_eq!((rows[end].x, rows[end].y), (under, ON_FLOOR));
    assert_eq!(rows[end].move_rect, t.player.move_rect);
    // He falls through the one-way floor and the stacked Solid below it,
    // gaining speed, and lands on the hall's floor.
    let landed = rows.iter().position(|l| l.y == ON_HALL).expect("he lands");
    let falling = &rows[end + 1..landed];
    assert!(!falling.is_empty());
    assert!(
        falling
            .iter()
            .all(|l| l.state == state::FALL && l.physics_type == ptype::FALL && l.x == under)
    );
    let steps: Vec<i32> = rows[end..landed]
        .windows(2)
        .map(|p| p[1].y - p[0].y)
        .collect();
    // Gaining speed up to the fall's cap (whole pixels of it: a step may
    // come 1 px short of the one before).
    assert!(steps.windows(2).all(|s| s[1] >= s[0] - 1), "{steps:?}");
    assert!(
        steps.last() > steps.first().map(|s| s + 10).as_ref(),
        "{steps:?}"
    );
    assert!(rows[landed..].iter().all(|l| l.y == ON_HALL));
    assert_ne!(rows[landed].physics_type, ptype::FALL);
}

#[test]
fn a_claw_put_down_with_his_feet_in_stacked_solid_falls_through_it() {
    let mut sim = new_sim(nook_level());
    // Feet 1 px inside the main floor, three tiles of Solid deep.
    let x = 20 * T;
    sim.with_world(|w| w.claw.special.warp_to = Some((x, ON_FLOOR + 1)));
    let rows = play(&mut sim, 80, 0);
    let landed = rows.iter().position(|l| l.y == ON_HALL).expect("he lands");
    assert!(rows[..landed].iter().all(|l| l.physics_type == ptype::FALL));
    assert!(rows[..landed].windows(2).all(|p| p[1].y >= p[0].y));
    assert!(rows[landed..].iter().all(|l| (l.x, l.y) == (x, ON_HALL)));
}

#[test]
fn a_claw_on_top_of_the_floor_is_not_moved() {
    // Put down standing on the floor, or 20 px above it: he stays on its
    // top.
    for dy in [0, -20] {
        let mut sim = new_sim(nook_level());
        let x = 20 * T;
        sim.with_world(|w| w.claw.special.warp_to = Some((x, ON_FLOOR + dy)));
        let rows = play(&mut sim, 60, 0);
        let last = rows.last().unwrap();
        assert_eq!((last.x, last.y), (x, ON_FLOOR), "from {dy}");
        assert!(rows.iter().all(|l| l.y <= ON_FLOOR), "from {dy}");
    }
}

#[test]
fn a_crouched_hit_in_the_open_ends_standing_on_the_same_floor() {
    let t = Tuning::default();
    let mut sim = new_sim(flat());
    let y = claw(&sim).y;
    crouch(&mut sim);
    hit_from(&mut sim, -50);
    let rows = play(&mut sim, 60, 0);
    let end = rows.iter().position(|l| l.state != state::HIT).unwrap();
    assert!(
        rows[..end]
            .iter()
            .all(|l| l.move_rect == t.player.crouch_move_rect)
    );
    assert!(rows.iter().all(|l| l.y == y), "never falls");
    assert_eq!(rows[end].state, state::STAND);
    assert_eq!(rows[end].move_rect, t.player.move_rect);
    assert!(rows[end..].iter().all(|l| l.physics_type != ptype::FALL));
}

#[test]
fn a_crouched_hit_away_from_the_nook_does_not_clip() {
    let mut sim = new_sim(nook_level());
    crouch(&mut sim);
    hit_from(&mut sim, -50);
    let rows = play(&mut sim, 60, DOWN);
    // Pushed right, over the main floor: he stays on it.
    assert!(rows.iter().all(|l| l.y == ON_FLOOR));
    assert!(rows.last().unwrap().x > START_X);
}
