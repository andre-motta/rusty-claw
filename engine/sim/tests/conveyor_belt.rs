//! Conveyor belts on synthetic levels: the push from the second tick after
//! Claw ends on a piece, the standing drift in whole pixels, the walk and
//! the jump's rise that add it, the fall that drops it, the one-tick stall
//! when Claw crosses into a piece that updates earlier, the right end's
//! half box, the teeter at the end of a belt over a drop, a warp that
//! leaves no push, the objects a belt never moves and the look of a piece
//! asleep in the side areas of a wide display.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::conveyor_belt::{self, BeltData, state as belt};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const LEFT_END: &str = conveyor_belt::PIECES[0];
const MIDDLE: &str = conveyor_belt::PIECES[1];
const RIGHT_END: &str = conveyor_belt::RIGHT_END;

/// Where Claw stands on the flat level's floor (top at 960).
const STAND_Y: i32 = 899;
/// A middle piece lies 9 px below the floor it is drawn on.
const PIECE_Y: i32 = 969;

fn cycle(key: &str, image_op: i32, ms: i32) -> (String, Arc<Animation>) {
    let anim = Animation {
        key: key.to_string(),
        frames: vec![AnimFrame {
            image_op,
            duration: ms,
            ..AnimFrame::default()
        }],
        ..Animation::default()
    };
    (key.to_string(), Arc::new(anim))
}

/// `level` with the belts' nine-picture sets and their cycles, and
/// `objects`.
fn with_belts(mut level: SimLevel, objects: Vec<ObjectSpec>) -> SimLevel {
    for (set, b) in [
        (LEFT_END, frame_box(44, 52, 0, -4)),
        (MIDDLE, frame_box(64, 20, 0, -12)),
        (RIGHT_END, frame_box(64, 52, 1, -4)),
    ] {
        let mut f = ImageFrames::range(1, 9);
        for i in 1..=9 {
            f.boxes.insert(i, b);
        }
        level.images.insert(set.into(), f);
    }
    for (prefix, op) in [("GAME_CYCLE", 1), ("GAME_REVERSECYCLE", 2)] {
        for ms in [50, 100, 200, 500] {
            let (k, a) = cycle(&format!("{prefix}{ms}"), op, ms);
            level.animations.insert(k, a);
        }
    }
    level.objects = objects;
    level
}

fn piece(set: &str, x: i32, y: i32, speed: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("ConveyorBelt", x, y, 1000);
    s.image_set = set.into();
    s.frame = 1;
    s.record[conveyor_belt::SPEED_FIELD] = speed;
    s
}

/// A belt of `n` middle pieces from x `from` on the flat level's floor.
fn middles(from: i32, n: i32, speed: i32) -> Vec<ObjectSpec> {
    (0..n)
        .map(|i| piece(MIDDLE, from + 64 * i, PIECE_Y, speed))
        .collect()
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

/// Claw put down standing at x `x` of the flat level with `objects`, and
/// settled: one tick played.
fn standing_at(x: i32, objects: Vec<ObjectSpec>) -> Sim {
    let mut level = flat_level();
    level.start = (x, STAND_Y);
    let mut sim = pressing_sim(with_belts(level, objects), original());
    sim.step(TickInput::default());
    sim
}

fn claw(sim: &Sim) -> (i32, i32, i32) {
    let o = sim.player().expect("Claw");
    (o.x, o.y, o.state)
}

fn force(sim: &Sim) -> i32 {
    sim.claw().conveyor_force as i32
}

fn pieces(sim: &Sim) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == "ConveyorBelt")
        .map(|(id, _)| id)
        .collect()
}

/// Steps once with `input` and returns Claw's move and the push after it.
fn step(sim: &mut Sim, input: TickInput) -> (i32, i32) {
    let x = claw(sim).0;
    sim.step(input);
    (claw(sim).0 - x, force(sim))
}

#[test]
fn a_piece_is_set_up_with_its_clamped_speed_its_box_and_its_cycle() {
    let mut sim = standing_at(600, vec![piece(MIDDLE, 1000, PIECE_Y, 500)]);
    idle(&mut sim, 2);
    let id = pieces(&sim)[0];
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.data, ObjectData::ConveyorBelt(BeltData { speed: 300 }));
    assert_eq!(o.move_rect, Rect::new(-32, -22, 31, -3));
    assert_eq!(o.state, belt::IDLE);
    assert_eq!(o.anim.key(), Some("GAME_REVERSECYCLE50"));
    assert_eq!(
        (o.type_flags, o.hit_type_flags),
        (1, conveyor_belt::HIT_MASK)
    );
    // A piece neither hits nor is hit: no box for either.
    assert!(!o.hit_rect.is_set() && !o.attack_rect.is_set());
    assert!(sim.collision_events().is_empty());
}

#[test]
fn the_push_starts_on_the_second_tick_after_claw_ends_on_a_piece() {
    // Claw walks right from x 1000 onto a 150 belt whose first piece holds
    // him from x 1053.
    let mut sim = standing_at(1000, middles(1100, 6, 150));
    let mut inside = None;
    for k in 0..30 {
        let held = if inside.is_none() { RIGHT } else { 0 };
        let input = if k == 0 { press(RIGHT) } else { hold(held) };
        sim.step(input);
        if inside.is_none() && claw(&sim).0 > 1052 {
            inside = Some(k);
            break;
        }
    }
    assert!(inside.is_some());
    // Stopped there: the piece notices him on the next tick, and pushes on
    // the one after.
    let (_, f1) = step(&mut sim, TickInput::default());
    assert_eq!(f1, 0);
    let (_, f2) = step(&mut sim, TickInput::default());
    assert_eq!(f2, 150);
}

#[test]
fn standing_claw_drifts_by_the_whole_pixels_of_each_tick() {
    for (speed, px) in [
        (150, 3),
        (175, 3),
        (100, 2),
        (300, 5),
        (500, 5),
        (-150, -3),
        (-500, -5),
    ] {
        // One piece at x 1300, Claw on its trailing side.
        let x = if speed > 0 { 1258 } else { 1342 };
        let mut sim = standing_at(x, vec![piece(MIDDLE, 1300, PIECE_Y, speed)]);
        idle(&mut sim, 1);
        for _ in 0..10 {
            let (dx, f) = step(&mut sim, TickInput::default());
            assert_eq!((dx, f), (px, conveyor_belt::clamp_speed(speed)), "{speed}");
            assert_eq!(claw(&sim).2, state::STAND);
        }
    }
}

#[test]
fn walking_adds_the_push_to_the_walk() {
    let walk = |objects: Vec<ObjectSpec>| {
        let mut sim = standing_at(1200, objects);
        idle(&mut sim, 3);
        sim.step(press(RIGHT));
        let x = claw(&sim).0;
        idle_holding(&mut sim, RIGHT, 40);
        claw(&sim).0 - x
    };
    let plain = walk(vec![]);
    let with = walk(middles(1100, 12, 150));
    // 150 px/s more over 40 ticks (108 px), and more with the carries
    // (the walk with a 150 belt averages 416.7 px/s, not 375); a crossing
    // into an earlier listed piece costs the push of a tick.
    assert!((110..=130).contains(&(with - plain)), "{plain} {with}");
}

fn idle_holding(sim: &mut Sim, mask: u32, n: usize) {
    for _ in 0..n {
        sim.step(hold(mask));
    }
}

#[test]
fn a_jump_rises_with_the_push_and_falls_without_it() {
    let mut sim = standing_at(1400, middles(1100, 10, -150));
    idle(&mut sim, 3);
    sim.step(press(JUMP));
    let mut rise = Vec::new();
    let mut fall = Vec::new();
    for _ in 0..60 {
        let x = claw(&sim).0;
        let jumping = sim.player().unwrap().physics_type;
        sim.step(hold(JUMP));
        let o = sim.player().unwrap();
        match (jumping, o.physics_type) {
            (ptype::JUMP, ptype::JUMP) => rise.push((o.x - x, force(&sim))),
            (ptype::FALL, ptype::FALL) => fall.push((o.x - x, force(&sim))),
            _ => {}
        }
        if o.physics_type == ptype::STAND {
            break;
        }
    }
    // The rise keeps the push: -3 or -4 px a tick, the force kept.
    assert!(rise.len() > 10);
    assert!(
        rise.iter()
            .all(|&(dx, f)| (-4..=-3).contains(&dx) && f == -150)
    );
    // The fall drops it: the pieces clear it on the tick after the apex
    // and Claw falls straight down.
    assert!(fall.len() > 5);
    assert!(fall.iter().all(|&(dx, f)| dx == 0 && f == 0), "{fall:?}");
}

/// The pieces of `sim` in list order, by x.
fn list_order(sim: &Sim) -> Vec<i32> {
    sim.objects()
        .listed()
        .into_iter()
        .filter_map(|id| sim.objects().get(id))
        .filter(|o| &*o.logic == "ConveyorBelt")
        .map(|o| o.x)
        .collect()
}

#[test]
fn crossing_into_a_piece_that_updates_earlier_stalls_one_tick() {
    // Two pieces side by side, woken in some order; Claw drifts from one
    // into the other, right on a 150 belt, left on a -150 one.
    let pair = |speed: i32| {
        vec![
            piece(MIDDLE, 1000, PIECE_Y, speed),
            piece(MIDDLE, 1064, PIECE_Y, speed),
        ]
    };
    let run = |speed: i32, x: i32| {
        let mut sim = standing_at(x, pair(speed));
        let order = list_order(&sim);
        let mut stalls = Vec::new();
        for k in 0..30 {
            let (dx, f) = step(&mut sim, TickInput::default());
            if k > 1 && f == 0 {
                stalls.push(dx);
            }
        }
        (order, stalls)
    };
    let (order, right) = run(150, 1000);
    let (_, left) = run(-150, 1064);
    // The tick the piece he leaves sees him gone, it writes 0 after the
    // other wrote the push when it comes later in the list: one tick
    // without drift, once; the other way round there is none.
    if order == [1000, 1064] {
        assert_eq!((right, left), (vec![], vec![0]));
    } else {
        assert_eq!(order, [1064, 1000]);
        assert_eq!((right, left), (vec![0], vec![]));
    }
}

#[test]
fn over_a_seam_the_piece_that_updates_last_sets_the_push() {
    // Claw held by a 150 piece and a 75 piece at once: the push is the
    // speed of the one listed later.
    let mut sim = standing_at(
        1032,
        vec![
            piece(MIDDLE, 1000, PIECE_Y, 150),
            piece(MIDDLE, 1064, PIECE_Y, 75),
        ],
    );
    let order = list_order(&sim);
    sim.step(TickInput::default());
    let last = if order == [1000, 1064] { 75 } else { 150 };
    assert_eq!(force(&sim), last);
}

#[test]
fn the_right_end_pushes_with_its_left_half_only() {
    // A right end at x 1200 lies 9 px lower than the middles, 18 below the
    // floor; its box (-31, -30, 0, 21) holds Claw for 1153 < x < 1216.
    let end_y = 978;
    for (x, held) in [(1153, false), (1154, true), (1215, true), (1216, false)] {
        // The tick after the load the piece has seen him or not.
        let sim = standing_at(x, vec![piece(RIGHT_END, 1200, end_y, 75)]);
        let id = pieces(&sim)[0];
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.move_rect, Rect::new(-31, -30, 0, 21));
        let s = o.state;
        assert_eq!(s == belt::MOVE, held, "x {x}");
    }
}

#[test]
fn a_piece_tests_from_claws_position_down_to_his_feet() {
    let p = {
        let mut o = logic::new_object("ConveyorBelt", 1000, PIECE_Y, 1000);
        o.move_rect = Rect::new(-32, -22, 31, -3);
        o
    };
    let mut c = logic::new_object("CaptainClaw", 1000, STAND_Y, 4000);
    c.move_rect = Rect::new(-16, -52, 16, 60);
    c.physics_type = ptype::STAND;
    let held = |c: &claw_sim::object::Object| conveyor_belt::holds(&p, conveyor_belt::claw_box(c));
    assert!(held(&c));
    // Up to 11 px above standing height he is held, 12 px up he is not.
    c.y = STAND_Y - 11;
    assert!(held(&c));
    c.y = STAND_Y - 12;
    assert!(!held(&c));
    // Edges: right and bottom excluded.
    c.y = STAND_Y;
    c.x = 1000 - 48;
    assert!(!held(&c));
    c.x = 1000 - 47;
    assert!(held(&c));
    c.x = 1000 + 47;
    assert!(!held(&c));
    // Rising in a jump he is never held.
    c.x = 1000;
    c.physics_type = ptype::JUMP;
    assert!(!held(&c));
}

#[test]
fn claw_carried_to_the_end_of_a_belt_over_a_drop_teeters_there() {
    // A floor from column 10 on, a -150 belt from its edge.
    let mut rows = vec![".".repeat(60); 15];
    rows.push(format!("{}{}", ".".repeat(10), "#".repeat(50)));
    rows.push(format!("{}{}", ".".repeat(10), "#".repeat(50)));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (800, STAND_Y));
    level.start = (800, STAND_Y);
    let mut sim = pressing_sim(with_belts(level, middles(672, 4, -150)), original());
    let mut edge = None;
    for k in 0..120 {
        sim.step(TickInput::default());
        if edge.is_none() && claw(&sim).2 == state::ON_EDGE {
            edge = Some((k, claw(&sim).0));
        }
    }
    let (_, x) = edge.expect("he teeters");
    // He stays there, the push kept, without drifting off.
    assert_eq!(claw(&sim), (x, STAND_Y, state::ON_EDGE));
    assert_eq!(force(&sim), -150);
    assert!(x < 660 && x > 600, "{x}");
}

#[test]
fn a_warp_from_a_belt_leaves_no_push() {
    let mut sim = standing_at(1200, middles(1100, 6, -300));
    idle(&mut sim, 3);
    assert_eq!(force(&sim), -300);
    // A warp's move between two ticks: at the target he stands still.
    sim.with_world(|w| w.claw.special.warp_to = Some((3000, STAND_Y)));
    let (_, f) = step(&mut sim, TickInput::default());
    assert_eq!(f, 0);
    assert_eq!(claw(&sim).0, 3000);
    for _ in 0..20 {
        assert_eq!(step(&mut sim, TickInput::default()), (0, 0));
    }
}

#[test]
fn a_belt_moves_nothing_but_claw() {
    // A keg on the belt, Claw elsewhere on it.
    let mut objects = middles(1000, 8, 300);
    objects.push(ObjectSpec::at("PowderKeg", 1300, 930, 5000));
    let mut sim = standing_at(1100, objects);
    idle(&mut sim, 5);
    let keg = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "PowderKeg")
        .map(|(id, o)| (id, o.x))
        .unwrap();
    idle(&mut sim, 30);
    assert_eq!(sim.objects().get(keg.0).unwrap().x, keg.1);
    assert!(claw(&sim).0 > 1200);
}

#[test]
fn a_belt_plays_nothing_and_draws_no_random_number() {
    let mut sim = standing_at(1200, middles(1100, 6, 150));
    let before = sim.with_world(|w| w.rng.clone());
    for _ in 0..50 {
        sim.step(TickInput::default());
        assert!(sim.last_audio().is_empty());
    }
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
}

#[test]
fn asleep_a_piece_is_drawn_with_its_cycle_started() {
    let level = with_belts(flat_level(), vec![]);
    let mk = |set: &str, speed: i32| {
        let mut o = logic::new_object("ConveyorBelt", 5000, PIECE_Y, 1000);
        o.image_set = set.into();
        o.frame = 1;
        let mut r = Box::new([0; 71]);
        r[conveyor_belt::SPEED_FIELD] = speed;
        o.record = Some(r);
        o
    };
    // A reversed cycle steps back from the first picture to the last; a
    // forward one on to the second.
    let look = logic::first_look(&level, &mk(MIDDLE, 150)).expect("a look");
    assert_eq!(look.len(), 1);
    assert_eq!((look[0].frame, look[0].z), (9, 1000));
    let look = logic::first_look(&level, &mk(MIDDLE, -150)).expect("a look");
    assert_eq!(look[0].frame, 2);
    // Once it has run it looks as it is.
    let mut ran = mk(MIDDLE, 150);
    ran.state = belt::IDLE;
    assert_eq!(logic::first_look(&level, &ran), None);
}
