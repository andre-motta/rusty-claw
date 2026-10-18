//! Skull cannons on synthetic levels: the build on the first call, the
//! shot on the first waiting call and then every 167 ticks, the ball 50 px
//! right of the cannon whichever way it faces (a cannon facing left fires
//! through itself), the shot's sounds and its return home, the ball's
//! single picture on a level without its animation, its blow on a ducking
//! Claw, and the look of a cannon asleep in the side areas of a wide
//! display.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::skull_cannon::{self, SkullCannonData, ball};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, draw_flags, flags, types};
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A cannon stands 43 px above the floor (top at 960), as on the ninth
/// level.
const CANNON_Y: i32 = 917;
const SHOT: &str = "LEVEL_CANONSH1";
const FLY: &str = "LEVEL_FLYUP";

fn image(i: i32, ms: i32, next_op: i32) -> AnimFrame {
    AnimFrame {
        image_op: 3,
        image_index: i,
        duration: ms,
        next_op,
        ..AnimFrame::default()
    }
}

/// The flat level with the cannon's seven pictures, its firing (pictures
/// 1 to 7, 100 ms each, the ball on the first, the two sounds on the
/// second and fourth) and waiting animations, the ball's pictures without
/// an animation, and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    let mut f = ImageFrames::range(1, 7);
    for i in 1..=7 {
        f.boxes.insert(i, frame_box(96, 76, 0, 0));
    }
    level.images.insert(skull_cannon::IMAGE_SET.into(), f);
    let mut f = ImageFrames::range(1, 5);
    for i in 1..=5 {
        f.boxes.insert(i, frame_box(32, 20, 7, 0));
    }
    level.images.insert(ball::IMAGE_SET.into(), f);
    let mut frames: Vec<AnimFrame> = (1..=7)
        .map(|i| image(i, 100, if i == 7 { 9 } else { 0 }))
        .collect();
    frames[0].event = 1;
    let mut frame_sounds = vec![Vec::new(); 7];
    frame_sounds[1] = vec![Some(SHOT.to_string())];
    frame_sounds[3] = vec![Some(FLY.to_string())];
    level.animations.insert(
        skull_cannon::FIRING_ANIM.into(),
        Arc::new(Animation {
            key: skull_cannon::FIRING_ANIM.into(),
            frames,
            frame_sounds,
            ..Animation::default()
        }),
    );
    level.animations.insert(
        skull_cannon::HOME_ANIM.into(),
        Arc::new(Animation {
            key: skull_cannon::HOME_ANIM.into(),
            frames: vec![image(1, 0, 9)],
            ..Animation::default()
        }),
    );
    for key in [SHOT, FLY] {
        level.sounds.insert(key.into(), Some(500));
    }
    level.objects = objects;
    level
}

fn cannon(x: i32, mirrored: bool) -> ObjectSpec {
    let mut s = ObjectSpec::at("SkullCannon", x, CANNON_Y, 0);
    s.image_set = skull_cannon::IMAGE_SET.into();
    s.frame = -1;
    if mirrored {
        s.record[11] = draw_flags::MIRROR as i32;
    }
    s
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

fn sim_with(claw_x: i32, objects: Vec<ObjectSpec>) -> Sim {
    let mut level = level_with(objects);
    level.start = (claw_x, 899);
    pressing_sim(level, original())
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

fn data(sim: &Sim, id: ObjectId) -> SkullCannonData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::SkullCannon(d) => d.clone(),
        d => panic!("{d:?}"),
    }
}

fn heard(sim: &Sim, key: &str) -> bool {
    sim.last_audio()
        .iter()
        .any(|c| matches!(c, claw_sim::claw_frame::AudioCue::Play(s) if s.key == key))
}

/// One tick of a cannon: its state, its picture, the balls fired so far,
/// and the shot's sounds heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    state: i32,
    frame: i32,
    fired: u32,
    shot: bool,
    fly: bool,
}

fn record(sim: &mut Sim, id: ObjectId, n: usize) -> Vec<Step> {
    (0..n)
        .map(|_| {
            sim.step(TickInput::default());
            let o = sim.objects().get(id).unwrap();
            Step {
                state: o.state,
                frame: o.frame,
                fired: data(sim, id).fired,
                shot: heard(sim, SHOT),
                fly: heard(sim, FLY),
            }
        })
        .collect()
}

#[test]
fn a_skull_cannon_is_built_on_its_first_call() {
    let mut sim = sim_with(1300, vec![cannon(1000, false), cannon(1600, true)]);
    sim.step(TickInput::default());
    let ids: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "SkullCannon")
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.image_set, skull_cannon::IMAGE_SET);
        assert_eq!(o.z, skull_cannon::DEFAULT_Z);
        assert_eq!(o.damage, 10);
        assert!(o.flags & flags::NO_COLLIDE != 0, "nothing touches it");
        let mirrored = o.draw_flags & draw_flags::MIRROR != 0;
        assert_eq!(o.speed_x, if mirrored { -500 } else { 500 });
    }
}

#[test]
fn it_fires_at_once_and_every_167_ticks() {
    let mut sim = sim_with(1300, vec![cannon(1000, false)]);
    let id = find(&sim, "SkullCannon");
    let steps = record(&mut sim, id, 520);
    let fire = EnemyState::Fire as i32;
    let home = EnemyState::Home as i32;
    let shots: Vec<usize> = (0..steps.len())
        .filter(|&k| steps[k].state == fire && (k == 0 || steps[k - 1].state != fire))
        .collect();
    // Built before the first tick, it fires on its first waiting call.
    assert_eq!(shots, [0, 167, 334, 501]);
    for &c in &shots[..3] {
        // The ball on C + 1, the sounds on C + 7 and C + 19, home on C + 43.
        assert_eq!(steps[c].fired + 1, steps[c + 1].fired);
        assert!(steps[c + 7].shot && steps[c + 19].fly);
        assert_eq!(
            (0..60)
                .filter(|&i| steps[c + i].shot || steps[c + i].fly)
                .count(),
            2
        );
        assert_eq!(steps[c + 42].state, fire);
        assert_eq!(steps[c + 43].state, home);
        // Pictures 1 to 7 while firing, image 1 again at home.
        assert_eq!(steps[c + 1].frame, 1);
        assert_eq!(steps[c + 37].frame, 7);
        assert_eq!(steps[c + 44].frame, 1);
    }
}

/// The ball cannon `id` last fired: its place, speed, picture and mirror.
fn last_ball(sim: &Sim, id: ObjectId) -> ObjectId {
    data(sim, id).last_ball.expect("a ball")
}

#[test]
fn balls_leave_50_px_right_of_the_cannon_whichever_way_it_faces() {
    for (mirrored, vx) in [(false, 500), (true, -500)] {
        let mut sim = sim_with(1300, vec![cannon(1000, mirrored)]);
        let id = find(&sim, "SkullCannon");
        idle(&mut sim, 2);
        let b = last_ball(&sim, id);
        let o = sim.objects().get(b).unwrap();
        assert_eq!((o.x, o.y), (1050, CANNON_Y + 10), "mirrored {mirrored}");
        assert_eq!(o.speed_x, vx);
        assert_eq!((o.type_flags, o.damage), (types::ENEMY_SHOT, ball::DAMAGE));
        assert_eq!(o.z, skull_cannon::DEFAULT_Z - 1);
        assert_eq!(o.draw_flags & draw_flags::MIRROR != 0, mirrored);
        // It flies 8 or 9 px a tick (the facing cannon's ball through the
        // cannon), showing its first picture throughout.
        let mut xs = vec![o.x];
        for _ in 0..20 {
            sim.step(TickInput::default());
            let o = sim.objects().get(b).unwrap();
            assert_eq!(o.frame, 1);
            xs.push(o.x);
        }
        let dir = vx.signum();
        assert!(
            xs.windows(2)
                .all(|w| (8..=9).contains(&((w[1] - w[0]) * dir))),
            "{xs:?}"
        );
        if mirrored {
            assert!(xs[20] < 1000 - 100, "through the cannon: {xs:?}");
        }
    }
}

#[test]
fn a_ball_costs_a_ducking_claw_10() {
    // Claw crouches 200 px right of the cannon, in its ball's lane.
    let mut sim = sim_with(1200, vec![cannon(1000, false)]);
    let mut hit = None;
    let mut before = 0;
    for k in 0..80 {
        sim.step(hold(DOWN));
        let h = sim.claw().stats.health;
        if hit.is_none() && h < 100 {
            hit = Some((k, h, before));
        }
        before = sim.player().unwrap().state;
    }
    let (k, h, crouching) = hit.expect("the ball reaches him");
    assert_eq!(h, 90);
    assert_eq!(crouching, state::CROUCH);
    assert!(k > 10, "{k}");
}

#[test]
fn a_cannon_never_looks_for_claw_and_draws_no_random_number() {
    // Claw behind the cannon, standing or ducking, near or far: the same
    // shots.
    let shots = |x: i32, duck: bool| {
        let mut sim = sim_with(x, vec![cannon(1000, false)]);
        let id = find(&sim, "SkullCannon");
        let before = sim.with_world(|w| w.rng.clone());
        let input = if duck {
            hold(DOWN)
        } else {
            TickInput::default()
        };
        let mut fired = Vec::new();
        for _ in 0..400 {
            sim.step(input);
            fired.push(data(&sim, id).fired);
        }
        assert_eq!(sim.with_world(|w| w.rng.clone()), before);
        fired
    };
    assert_eq!(shots(600, false), shots(900, true));
}

#[test]
fn asleep_a_cannon_is_drawn_waiting_at_home() {
    let level = level_with(vec![]);
    let mut o = logic::new_object("SkullCannon", 5000, CANNON_Y, 50);
    o.image_set = skull_cannon::IMAGE_SET.into();
    o.frame = 1;
    o.draw_flags = draw_flags::MIRROR;
    let look = logic::first_look(&level, &o).expect("a look");
    assert_eq!(look.len(), 1);
    assert_eq!(look[0].image_set, skull_cannon::IMAGE_SET);
    assert_eq!((look[0].frame, look[0].z), (1, 50));
    assert!(look[0].draw_flags & draw_flags::MIRROR != 0);
    // Placed at depth 0 it is drawn at its default depth.
    o.z = 0;
    let look = logic::first_look(&level, &o).expect("a look");
    assert_eq!(look[0].z, skull_cannon::DEFAULT_Z);
    o.state = EnemyState::Home as i32;
    assert_eq!(logic::first_look(&level, &o), None);
}
