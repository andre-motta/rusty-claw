//! Swinging ropes and Claw's Swing state on synthetic levels: the rope's
//! frame on the level clock, the grab and its conditions, hanging from the
//! end as it swings and turning with it, letting go with a jump, the
//! regrab time, and a hit while swinging.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::rope::{self, RopeData};
use claw_sim::object::{Object, ObjectId, Rect, flags, types};
use claw_sim::physics::ptype;
use claw_sim::player::{state, swing};
use claw_sim::{ObjectData, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A half swing so long that the rope runs at no whole frame per second:
/// it stays on the frame its phase gives.
const FROZEN: i32 = 100_000;

/// A rope record at (x, y) with the given half swing (0 for the default)
/// and phase.
fn rope_at(x: i32, y: i32, half_ms: i32, phase_ms: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("AniRope", x, y, 0);
    s.image_set = rope::IMAGE_SET.into();
    s.frame = 1;
    s.record[55] = half_ms;
    s.record[60] = phase_ms;
    s
}

/// The flat level (feet rest on 959) with the rope's frames, Claw's
/// swinging animation and `objects`, every object awake from the start.
fn sim_with(objects: Vec<ObjectSpec>) -> Sim {
    let mut level = flat_level();
    level
        .images
        .insert(rope::IMAGE_SET.into(), ImageFrames::range(1, 120));
    let swing = Animation::looping(
        swing::SWING_ANIM,
        &[(500, 75), (501, 75), (502, 75), (503, 75)],
    );
    level
        .animations
        .insert(swing::SWING_ANIM.into(), Arc::new(swing));
    level.objects = objects;
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    sim
}

fn ropes(sim: &Sim) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "AniRope")
        .map(|(id, _)| id)
        .collect();
    v.sort_unstable();
    v
}

fn data(sim: &Sim, id: ObjectId) -> RopeData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::Rope(d) => d.clone(),
        other => panic!("not a rope: {other:?}"),
    }
}

fn end(sim: &Sim, id: ObjectId) -> (i32, i32) {
    rope::end_of(sim.objects().get(id).unwrap()).unwrap()
}

fn claw(sim: &Sim) -> &Object {
    sim.player().unwrap()
}

fn swinging(sim: &Sim) -> bool {
    claw(sim).state == state::SWING
}

/// Puts Claw in the air at (x, y), falling.
fn drop_claw(sim: &mut Sim, x: i32, y: i32) {
    sim.teleport_player(x, y);
    sim.step(TickInput::default());
}

/// Steps until Claw swings; returns the ticks it took.
fn until_swinging(sim: &mut Sim, max: usize) -> Option<usize> {
    for n in 1..=max {
        sim.step(TickInput::default());
        if swinging(sim) {
            return Some(n);
        }
    }
    None
}

#[test]
fn the_rope_frame_runs_on_the_level_clock_one_tick_late() {
    // The default half swing (1500 ms: 40 frames per second) and a phase
    // of 500 ms (20 frames).
    let mut sim = sim_with(vec![rope_at(2000, 300, 0, 500)]);
    let id = ropes(&sim)[0];
    let d = data(&sim, id);
    assert_eq!((d.q, d.position), (40, 20.0));
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.z, o.speed_x), (rope::DEFAULT_Z, 1500));
    assert_eq!(o.type_flags, types::SPECIAL);
    assert!(
        !o.attack_rect.is_set(),
        "ungrabbable until its first swing call"
    );
    // The frame position as the rope moves it: in seconds of the clock, in
    // this order.
    let (mut time, mut pos) = (0.0f64, 20.0f64);
    for t in 1..=400u32 {
        sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        let frame = rope::frame_at(pos);
        assert_eq!(o.frame, frame, "tick {t}");
        let (e, grab, _) = rope::end_of_frame(frame);
        assert_eq!(data(&sim, id).end, e);
        assert_eq!(o.attack_rect, grab);
        let dt = f64::from(t * 18) * 0.001 - time;
        time += dt;
        pos += 40.0 * dt;
    }
    // A whole swing takes 3 s: frame 21 again 3 s after the start.
    assert_eq!(rope::frame_at(20.0 + 40.0 * 3.0), 21);
}

#[test]
fn a_falling_claw_grabs_the_end_and_hangs_from_it() {
    // A still rope on its first frame: the end 140 px left of the rope and
    // 27 px below it.
    let mut sim = sim_with(vec![rope_at(2000, 500, FROZEN, 0)]);
    let id = ropes(&sim)[0];
    idle(&mut sim, 2);
    assert_eq!(end(&sim, id), (1860, 527));
    drop_claw(&mut sim, 1860, 470);
    assert!(
        until_swinging(&mut sim, 20).is_some(),
        "Claw grabs the rope"
    );
    let c = claw(&sim);
    assert_eq!(c.physics_type, ptype::FLY);
    assert_eq!(sim.claw().rope, Some(id));
    // Facing right: 8 px right of the end, 20 px below it.
    assert!(sim.claw().facing_right);
    assert_eq!((c.x, c.y), (1868, 547));
    // He hangs there, tick after tick, with no tile collision.
    for _ in 0..30 {
        sim.step(TickInput::default());
        assert!(swinging(&sim));
        assert_eq!((claw(&sim).x, claw(&sim).y), (1868, 547));
    }
    assert_eq!(sim.claw().fallback_entries, 0);
}

#[test]
fn claw_grabs_only_jumping_or_falling_and_close_enough_in_height() {
    // The end 16 px above the point Claw's position must be near: a rope
    // whose end sits just above a standing Claw.
    let mut sim = sim_with(vec![rope_at(2000, 856, FROZEN, 0)]);
    let id = ropes(&sim)[0];
    idle(&mut sim, 2);
    assert_eq!(end(&sim, id), (1860, 883));
    sim.teleport_player(1860, 899);
    idle(&mut sim, 20);
    assert_eq!(claw(&sim).state, state::STAND, "standing never grabs");
    // A jump from there grabs it.
    sim.step(press(JUMP));
    assert!(until_swinging(&mut sim, 10).is_some());

    // Touching the grab box but too high: Claw's position 48 px or more
    // above the point 16 px under the end (here 34 px above the end, 50
    // px above that point).
    let mut sim = sim_with(vec![rope_at(2000, 300, FROZEN, 0)]);
    let id = ropes(&sim)[0];
    idle(&mut sim, 2);
    assert_eq!(end(&sim, id), (1860, 327));
    let falling_at = |sim: &mut Sim, y: i32| {
        sim.teleport_player(1860, y);
        sim.with_world(|w| {
            let id = w.claw_id.unwrap();
            w.objects.get_mut(id).unwrap().physics_type = ptype::FALL;
        });
        sim.step(TickInput::default());
        swinging(sim)
    };
    assert!(!falling_at(&mut sim, 327 - 34), "too high");
    assert!(falling_at(&mut sim, 327 - 20), "in reach");
}

#[test]
fn the_rope_turns_claw_the_way_its_end_travels() {
    // A swinging rope (2000 ms half swing) high above the floor.
    let mut sim = sim_with(vec![rope_at(3000, 300, 2000, 0)]);
    let id = ropes(&sim)[0];
    idle(&mut sim, 1);
    // Wait for the end to swing near the bottom, then put Claw there.
    for _ in 0..400 {
        sim.step(TickInput::default());
        if data(&sim, id).frame == 20 {
            break;
        }
    }
    let (ex, ey) = end(&sim, id);
    drop_claw(&mut sim, ex, ey - 20);
    assert!(until_swinging(&mut sim, 10).is_some());
    let mut turns = 0;
    let mut last = sim.claw().facing_right;
    for t in 0..300 {
        sim.step(TickInput::default());
        assert!(swinging(&sim), "tick {t}");
        let d = data(&sim, id);
        let right = (1..=60).contains(&d.frame);
        assert_eq!(sim.claw().facing_right, right, "tick {t}");
        let c = claw(&sim);
        assert_eq!(
            (c.x, c.y),
            swing::hang_point(end(&sim, id), right),
            "tick {t}"
        );
        // Turned by the rope, his picture and boxes are mirrored.
        assert_eq!(
            c.draw_flags & claw_sim::object::draw_flags::MIRROR != 0,
            !right
        );
        if right != last {
            turns += 1;
            assert_eq!(sim.claw().running_time, 1500, "a turn resets the run");
        }
        last = right;
    }
    assert!(turns >= 2, "{turns}");
}

#[test]
fn a_jump_lets_go_and_the_same_rope_waits_500_ms() {
    let mut sim = sim_with(vec![
        rope_at(2000, 500, FROZEN, 0),
        rope_at(2600, 500, FROZEN, 0),
    ]);
    let ids = ropes(&sim);
    idle(&mut sim, 2);
    drop_claw(&mut sim, 1860, 470);
    assert!(until_swinging(&mut sim, 20).is_some());
    let (x, y) = (claw(&sim).x, claw(&sim).y);
    sim.step(press(JUMP));
    let c = claw(&sim);
    assert_eq!(c.state, state::JUMP);
    assert_eq!(c.physics_type, ptype::JUMP);
    assert_eq!((c.x, c.y), (x, y), "the jump starts where he hung");
    assert_eq!((c.speed_x, c.speed_y), (0, 0), "no momentum from the rope");
    assert_eq!(sim.claw().jump_start_y, y);
    assert_eq!(sim.claw().jump_peak_y, y - 45);
    assert_eq!(sim.claw().rope_regrab_ms, 500 - 18);
    // Held in the grab box, he cannot take the same rope again until the
    // regrab time has run out: on the 28th tick counting the release.
    let mut back = None;
    for n in 2..=40 {
        sim.teleport_player(1860, 530);
        sim.step(hold(JUMP));
        if swinging(&sim) {
            back = Some(n);
            break;
        }
        assert!(sim.claw().rope_regrab_ms > 0 || n == 28, "tick {n}");
    }
    assert_eq!(back, Some(28));
    assert_eq!(sim.claw().rope, Some(ids[0]));
    // Another rope can be taken at once.
    sim.step(press(JUMP));
    assert_eq!(claw(&sim).state, state::JUMP);
    let (ex, ey) = end(&sim, ids[1]);
    sim.teleport_player(ex, ey + 3);
    sim.step(hold(JUMP));
    assert!(swinging(&sim));
    assert_eq!(sim.claw().rope, Some(ids[1]));
}

#[test]
fn a_hit_knocks_claw_off_and_the_rope_catches_him_again() {
    let mut sim = sim_with(vec![rope_at(2000, 500, FROZEN, 0)]);
    let id = ropes(&sim)[0];
    idle(&mut sim, 2);
    drop_claw(&mut sim, 1860, 470);
    assert!(until_swinging(&mut sim, 20).is_some());
    let health = sim.stats().health;
    // A hazard's shot over Claw for one tick.
    let mut shot = Object::new(
        0,
        1868,
        547,
        100,
        "TestShot".into(),
        claw_sim::object::LogicKind::Inert,
    );
    shot.flags = flags::WORLD;
    shot.type_flags = types::LEVEL_SHOT;
    shot.attack_rect = Rect::new(-5, -5, 5, 5);
    shot.damage = 10;
    let shot = sim.spawn(shot);
    sim.step(TickInput::default());
    sim.with_world(|w| w.destroy(shot));
    assert_eq!(sim.stats().health, health - 10);
    let c = claw(&sim);
    // The hit leaves him falling: either still reeling, or already back
    // on the rope, which the hit did not keep him from.
    assert!(
        c.state == state::HIT || c.state == state::SWING,
        "{}",
        c.state
    );
    assert_eq!(sim.claw().rope_regrab_ms, 0, "a hit sets no regrab time");
    let back = until_swinging(&mut sim, 5).map_or(swinging(&sim), |_| true);
    assert!(back, "the rope catches him again");
    assert_eq!(sim.claw().rope, Some(id));
}

#[test]
fn claw_put_in_swing_without_a_rope_returns_to_standing() {
    let mut sim = sim_with(Vec::new());
    sim.teleport_player(1000, 500);
    sim.set_player_state(state::SWING);
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::STAND);
    assert_eq!(sim.claw().fallback_entries, 1);
    // In mid-air, he falls from there.
    sim.step(TickInput::default());
    assert_eq!(claw(&sim).state, state::FALL);
}

// ---------------------------------------------------------------- install

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

#[test]
fn install_level3_ropes_swing_on_the_clock_and_hold_claw() {
    let Some(store) = store() else { return };
    let data3 = LevelData::build(&store, LevelSource::Retail(3)).unwrap();
    let level = SimLevel::from_level(&data3, &store).unwrap();
    assert!(level.animations.contains_key(swing::SWING_ANIM));
    assert_eq!(level.image_frames(rope::IMAGE_SET), Some((1, 120)));
    let records: Vec<ObjectSpec> = level
        .objects
        .iter()
        .filter(|o| o.logic == "AniRope")
        .cloned()
        .collect();
    assert_eq!(records.len(), 9);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    let ids = ropes(&sim);
    assert_eq!(ids.len(), 9);
    // Each rope's frame follows its formula.
    let start: Vec<(i32, f64)> = ids
        .iter()
        .map(|&id| {
            let d = data(&sim, id);
            (d.q, d.position)
        })
        .collect();
    for (&id, &(q, p)) in ids.iter().zip(&start) {
        let o = sim.objects().get(id).unwrap();
        assert!(q == 30 || q == 34, "{q}");
        assert_eq!(o.z, rope::DEFAULT_Z);
        assert!(p == 0.0 || p == 29.0 || p == 45.0, "{p}");
    }
    let mut clocks: Vec<(f64, f64)> = start.iter().map(|&(_, p)| (0.0, p)).collect();
    for t in 1..=300u32 {
        sim.step(TickInput::default());
        for (i, &id) in ids.iter().enumerate() {
            let (time, pos) = &mut clocks[i];
            assert_eq!(
                data(&sim, id).frame,
                rope::frame_at(*pos),
                "rope {i} tick {t}"
            );
            let dt = f64::from(t * 18) * 0.001 - *time;
            *time += dt;
            *pos += f64::from(start[i].0) * dt;
        }
    }
    // A jump into the first rope's end grabs it and hangs Claw from it.
    let first = ids[0];
    let (ex, ey) = end(&sim, first);
    sim.teleport_player(ex, ey);
    sim.with_world(|w| {
        let id = w.claw_id.unwrap();
        w.objects.get_mut(id).unwrap().physics_type = ptype::FALL;
    });
    sim.step(TickInput::default());
    let mut grabbed = swinging(&sim);
    for _ in 0..5 {
        if grabbed {
            break;
        }
        let (ex, ey) = end(&sim, first);
        sim.teleport_player(ex, ey);
        sim.step(TickInput::default());
        grabbed = swinging(&sim);
    }
    assert!(grabbed);
    sim.step(TickInput::default());
    let right = sim.claw().facing_right;
    assert_eq!(
        (claw(&sim).x, claw(&sim).y),
        swing::hang_point(end(&sim, first), right)
    );
    // Let go: a walking jump, and the same rope refused for 500 ms.
    sim.step(press(JUMP));
    assert_eq!(claw(&sim).state, state::JUMP);
    assert!(sim.claw().rope_regrab_ms > 0);
    sim.step(hold(JUMP));
    assert_eq!(claw(&sim).speed_y, -425);
}
