//! Elevators, pegs and decoration on synthetic levels: tracks against the
//! level clock, riders carried exactly, peg timings and decoration loops.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::container::CollisionEvent;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::decor;
use claw_sim::logic::platform::{self, NOT_PLATFORM, PLATFORM, Rider, SurfaceKind, state};
use claw_sim::object::{ObjectId, draw_flags, flags};
use claw_sim::player::state as claw_state;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{ActivationConfig, InputLatch, InputScript, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const X_MIN: usize = 51;
const Y_MIN: usize = 52;
const X_MAX: usize = 53;
const Y_MAX: usize = 54;
const SPEED_X: usize = 55;
const SPEED_Y: usize = 56;
const SPEED: usize = 60;
const SMARTS: usize = 17;
const DIRECTION: usize = 63;

fn element(image_op: i32, next_op: i32, duration: i32) -> AnimFrame {
    AnimFrame {
        image_op,
        next_op,
        duration,
        ..AnimFrame::default()
    }
}

fn add_anim(level: &mut SimLevel, key: &str, frames: Vec<AnimFrame>) {
    level.animations.insert(
        key.to_string(),
        Arc::new(Animation {
            key: key.to_string(),
            frames,
            ..Animation::default()
        }),
    );
}

/// The flat test level with the three animations the pegs and the
/// decoration loop play.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    // Forward: next image every 50 ms until the last, then finish.
    add_anim(
        &mut level,
        platform::FORWARD_ANIM,
        vec![element(1, 3, 50), element(0, 9, 0)],
    );
    // Backward: jump to the last image, previous every 50 ms until the
    // first, then finish.
    add_anim(
        &mut level,
        platform::BACKWARD_ANIM,
        vec![element(5, 0, 0), element(2, 2, 50), element(0, 9, 0)],
    );
    // Cycle: next image (wrapping) every 100 ms, for good.
    add_anim(&mut level, decor::CYCLE_ANIM, vec![element(1, 0, 100)]);
    level.objects = objects;
    level
}

fn spec(logic: &str, x: i32, y: i32, z: i32, fields: &[(usize, i32)]) -> ObjectSpec {
    let mut s = ObjectSpec::at(logic, x, y, z);
    s.image_set = "TEST_SET".into();
    s.frame = 1;
    for &(i, v) in fields {
        s.record[i] = v;
    }
    s
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

/// Registers the test image set with frames `first..=last` in `level`.
fn with_frames(mut level: SimLevel, (first, last): (i32, i32)) -> SimLevel {
    level
        .images
        .insert("TEST_SET".into(), ImageFrames::range(first, last));
    level
}

fn pos(sim: &Sim, id: ObjectId) -> (i32, i32) {
    let o = sim.objects().get(id).unwrap();
    (o.x, o.y)
}

fn solid(sim: &Sim, id: ObjectId) -> bool {
    sim.objects().get(id).unwrap().type_flags == PLATFORM
}

fn obj_state(sim: &Sim, id: ObjectId) -> i32 {
    sim.objects().get(id).unwrap().state
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio().iter().any(|c| match c {
        claw_sim::claw_frame::AudioCue::Play(s) => s.key == key,
        _ => false,
    })
}

/// True when Claw rides `id` with his feet one pixel above its surface.
fn riding(sim: &Sim, id: ObjectId) -> bool {
    let c = sim.player().unwrap();
    let p = sim.objects().get(id).unwrap();
    c.flags & flags::ON_ELEVATOR != 0
        && c.object_below == Some(id)
        && c.y + c.move_rect.bottom == p.y + p.hit_rect.top - 1
}

/// Drops Claw onto the platform `id` from above and waits until he rides
/// it; returns the ticks it took.
fn land_on(sim: &mut Sim, id: ObjectId) -> usize {
    if !sim.objects().get(id).unwrap().hit_rect.is_set() {
        // Not set up yet.
        sim.step(TickInput::default());
    }
    let (x, y) = pos(sim, id);
    let top = sim.objects().get(id).unwrap().hit_rect.top;
    let bottom = sim.player().unwrap().move_rect.bottom;
    sim.teleport_player(x, y + top - bottom - 10);
    for n in 0..200 {
        sim.step(TickInput::default());
        if riding(sim, id) {
            return n;
        }
    }
    panic!("Claw never landed on the platform");
}

/// The clock elevator's track written out: placed at the high end of
/// 1000..1160 at 150 px/s.
fn expected_x(ms: u64) -> i32 {
    let phase = 160 * 1000 / 150;
    let t = (phase + ms) * 150 / 1000;
    let r = (t % 322) as i32;
    if r <= 161 { 1000 + r } else { 1160 + 161 - r }
}

fn track_elevator(x: i32) -> ObjectSpec {
    spec(
        "Elevator",
        x,
        700,
        0,
        &[(X_MIN, x - 160), (X_MAX, x), (SPEED_X, 150)],
    )
}

#[test]
fn elevator_follows_the_clock_and_turns_at_its_ends() {
    let mut sim = Sim::new(level_with(vec![track_elevator(1160)]), SimConfig::default());
    let id = find(&sim, "Elevator");
    sim.step(TickInput::default());
    // The set-up call does not move it.
    assert_eq!(pos(&sim, id), (1160, 700));
    assert_eq!(obj_state(&sim, id), state::RUNNING);
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.z, platform::DEFAULT_Z);
    assert_eq!(o.type_flags, PLATFORM);
    assert!(o.flags & flags::NO_COLLIDE != 0);
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    let mut turns = 0;
    let mut last_dir = 0;
    for k in 1..800u64 {
        let before = pos(&sim, id).0;
        sim.step(TickInput::default());
        let ms = 18 * (k + 1);
        let (x, y) = pos(&sim, id);
        assert_eq!(x, expected_x(ms), "ms {ms}");
        assert_eq!(y, 700);
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.platform_move, (x - before, 0));
        lo = lo.min(x);
        hi = hi.max(x);
        let dir = (x - before).signum();
        if dir != 0 && dir != last_dir {
            if last_dir != 0 {
                // It turns only at the ends of its track.
                assert!(before <= 1003 || before >= 1158, "turned at {before}");
                turns += 1;
            }
            last_dir = dir;
        }
    }
    assert_eq!(pos(&sim, id).0, expected_x(18 * 800));
    assert_eq!((lo, hi), (1000, 1161));
    assert!(turns >= 6);
    // The worked example: ms 36 and 54.
    let mut sim = Sim::new(level_with(vec![track_elevator(1160)]), SimConfig::default());
    let id = find(&sim, "Elevator");
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    assert_eq!(pos(&sim, id).0, 1156);
    assert_eq!(sim.objects().get(id).unwrap().platform_move, (-4, 0));
    sim.step(TickInput::default());
    assert_eq!(pos(&sim, id).0, 1153);
}

#[test]
fn a_sleeping_elevator_wakes_in_phase_in_every_activation_mode() {
    // Far beyond the activation window around the start.
    let far = 9160;
    let shift = far - 1160;
    for activation in [
        ActivationConfig::original(),
        ActivationConfig::frozen_edge(),
        ActivationConfig::widened((1706, 480)),
    ] {
        let cfg = SimConfig {
            activation,
            ..SimConfig::default()
        };
        let mut sim = Sim::new(level_with(vec![track_elevator(far)]), cfg);
        let id = find(&sim, "Elevator");
        idle(&mut sim, 300);
        assert_eq!(pos(&sim, id), (far, 700), "dormant at its place");
        sim.teleport_player(far, 800);
        let mut live = 0;
        for _ in 0..400 {
            sim.step(TickInput::default());
            if obj_state(&sim, id) == state::RUNNING && sim.objects().get(id).unwrap().is_listed() {
                live += 1;
                if live > 1 {
                    let ms = 18 * sim.tick();
                    assert_eq!(pos(&sim, id).0, expected_x(ms) + shift, "ms {ms}");
                }
            }
        }
        assert!(live > 300);
    }
}

fn rider_level() -> SimLevel {
    // A diagonal track: 1000..1100 across and 600..700 down.
    level_with(vec![spec(
        "Elevator",
        1050,
        650,
        2000,
        &[
            (X_MIN, 1000),
            (X_MAX, 1100),
            (Y_MIN, 600),
            (Y_MAX, 700),
            (SPEED_X, 150),
            (SPEED_Y, 100),
        ],
    )])
}

#[test]
fn a_rider_is_carried_exactly_and_can_jump_off() {
    let mut sim = Sim::new(rider_level(), SimConfig::default());
    let id = find(&sim, "Elevator");
    land_on(&mut sim, id);
    for _ in 0..30 {
        sim.step(TickInput::default());
        assert!(riding(&sim, id));
    }
    let offset = sim.player().unwrap().x - pos(&sim, id).0;
    let mut moved = (0, 0);
    let (mut dx_signs, mut dy_signs) = (Vec::new(), Vec::new());
    for _ in 0..600 {
        sim.step(TickInput::default());
        assert!(
            riding(&sim, id),
            "feet on the surface at tick {}",
            sim.tick()
        );
        let c = sim.player().unwrap();
        assert_eq!(c.x - pos(&sim, id).0, offset, "no sliding");
        assert_eq!(c.state, claw_state::STAND);
        let m = sim.objects().get(id).unwrap().platform_move;
        moved.0 += m.0.abs();
        moved.1 += m.1.abs();
        dx_signs.push(m.0.signum());
        dy_signs.push(m.1.signum());
    }
    assert!(moved.0 > 300 && moved.1 > 300, "both axes moved: {moved:?}");
    assert!(dx_signs.contains(&-1) && dx_signs.contains(&1));
    assert!(dy_signs.contains(&-1) && dy_signs.contains(&1));
    // A jump leaves the platform.
    let surface = sim.player().unwrap().y;
    sim.step(press(JUMP));
    let c = sim.player().unwrap();
    assert_eq!(c.state, claw_state::JUMP);
    assert!(c.flags & flags::ON_ELEVATOR == 0);
    for _ in 0..5 {
        sim.step(hold(JUMP));
    }
    assert!(sim.player().unwrap().y < surface - 20);
}

fn trace_at_rate(level: SimLevel, script: &InputScript, hz: u64) -> String {
    let mut sim = Sim::new(level, SimConfig::default());
    let id = find(&sim, "Elevator");
    land_on(&mut sim, id);
    let mut latch = InputLatch::new();
    let events = script.events();
    let start = sim.tick();
    let (mut next_event, mut tick, mut frame) = (0, 0u64, 0u64);
    let n = script.ticks.len() as u64;
    let mut out = String::new();
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            let row = TraceRow::capture(&sim, start + tick, "script");
            let (x, y) = pos(&sim, id);
            out.push_str(&format!("{},{x},{y}\n", trace::to_csv(&[row])));
            tick += 1;
        }
        frame += 1;
    }
    out
}

#[test]
fn riding_traces_are_identical_at_60_144_and_240_hz() {
    let s = script(&[
        (0, 900),
        (RIGHT, 300),
        (0, 500),
        (JUMP, 400),
        (0, 700),
        (LEFT | JUMP, 300),
        (0, 1500),
    ]);
    let base = trace_at_rate(rider_level(), &s, 1000);
    for hz in [60, 144, 240] {
        assert_eq!(trace_at_rate(rider_level(), &s, hz), base, "{hz} Hz");
    }
}

fn trigger(logic: &str, fields: &[(usize, i32)]) -> (Sim, ObjectId) {
    let sim = Sim::new(
        level_with(vec![spec(logic, 1500, 700, 0, fields)]),
        SimConfig::default(),
    );
    let id = find(&sim, logic);
    (sim, id)
}

#[test]
fn a_start_elevator_moves_only_under_claw() {
    let (mut sim, id) = trigger(
        "StartElevator",
        &[(X_MIN, 1505), (X_MAX, 1800), (SPEED_X, 175)],
    );
    idle(&mut sim, 50);
    assert_eq!(pos(&sim, id), (1500, 700), "waits for Claw");
    assert_eq!(sim.objects().get(id).unwrap().z, 0, "keeps its depth");
    land_on(&mut sim, id);
    // Placed left of its track: the first moving call only turns it.
    let mut xs = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        assert!(riding(&sim, id));
        xs.push(pos(&sim, id).0);
    }
    let steps: Vec<i32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(steps.iter().all(|s| [3, -3, 0].contains(s)), "{steps:?}");
    let top = *xs.iter().max().unwrap();
    assert!(
        (1800..1803).contains(&top),
        "overshoots by less than a step"
    );
    assert!(steps.contains(&-3), "turns back");
    // Off it, it stops (after the call that still sees him on it).
    sim.teleport_player(400, 900);
    idle(&mut sim, 2);
    let x = pos(&sim, id).0;
    idle(&mut sim, 30);
    assert_eq!(pos(&sim, id).0, x);
}

#[test]
fn a_trigger_elevator_starts_for_good_and_resets_on_a_new_attempt() {
    // Vertical: rises from 700 to 500 at the default speed.
    let (mut sim, id) = trigger("TriggerElevator", &[(Y_MIN, 500), (Y_MAX, 700)]);
    idle(&mut sim, 20);
    assert_eq!(pos(&sim, id), (1500, 700));
    land_on(&mut sim, id);
    for _ in 0..20 {
        sim.step(TickInput::default());
        assert!(riding(&sim, id));
        assert_eq!(sim.objects().get(id).unwrap().platform_move, (0, -2));
    }
    // Claw leaves; it carries on to the top and back.
    sim.teleport_player(400, 900);
    let mut ys = Vec::new();
    for _ in 0..300 {
        sim.step(TickInput::default());
        ys.push(pos(&sim, id).1);
    }
    assert_eq!(*ys.iter().min().unwrap(), 500);
    assert_eq!(*ys.iter().max().unwrap(), 700);
    // A new attempt puts it back where it was placed, waiting.
    sim.with_world(|w| {
        platform::step(
            w,
            id,
            Rider {
                on: false,
                attempt: 1,
            },
        )
    });
    assert_eq!(pos(&sim, id), (1500, 700));
    assert_eq!(obj_state(&sim, id), state::SET_UP);
    sim.with_world(|w| {
        platform::step(
            w,
            id,
            Rider {
                on: false,
                attempt: 1,
            },
        )
    });
    for _ in 0..10 {
        sim.with_world(|w| {
            platform::step(
                w,
                id,
                Rider {
                    on: false,
                    attempt: 1,
                },
            )
        });
    }
    assert_eq!(pos(&sim, id), (1500, 700), "waits again");
}

#[test]
fn a_stop_elevator_stands_still_under_claw() {
    // Horizontal (direction 6), heading for the high end.
    let (mut sim, id) = trigger(
        "StopElevator",
        &[(X_MIN, 1400), (X_MAX, 1600), (SPEED_X, 250), (DIRECTION, 6)],
    );
    idle(&mut sim, 30);
    assert_ne!(pos(&sim, id).0, 1500, "runs on its own");
    land_on(&mut sim, id);
    let x = pos(&sim, id).0;
    for _ in 0..30 {
        sim.step(TickInput::default());
        assert!(riding(&sim, id));
        assert_eq!(pos(&sim, id).0, x);
    }
}

#[test]
fn a_one_way_elevator_stops_at_its_end() {
    let (mut sim, id) = trigger(
        "OneWayTriggerElevator",
        &[(X_MIN, 1300), (X_MAX, 1500), (DIRECTION, 4)],
    );
    land_on(&mut sim, id);
    idle(&mut sim, 200);
    assert_eq!(obj_state(&sim, id), state::STOPPED);
    assert_eq!(pos(&sim, id), (1300, 700));
}

/// A peg whose image set the level reports as frames 1 to 12.
fn peg_sim(logic: &str, x: i32, fields: &[(usize, i32)]) -> (Sim, ObjectId) {
    let level = with_frames(level_with(vec![spec(logic, x, 800, 0, fields)]), (1, 12));
    let sim = Sim::new(level, SimConfig::default());
    let id = find(&sim, logic);
    (sim, id)
}

#[test]
fn a_crumbling_peg_drops_claw_and_comes_back_on_a_new_attempt() {
    let (mut sim, id) = peg_sim("CrumblingPeg", 1100, &[]);
    sim.step(TickInput::default());
    let o = sim.objects().get(id).unwrap();
    assert_eq!((o.state, o.z, o.frame), (state::STOPPED, 2000, 1));
    assert_eq!(o.hit_rect.top, -46);
    // Claw lands; the first call that sees him on it is tick T.
    let (x, y) = pos(&sim, id);
    sim.teleport_player(x, y - 200);
    let mut t = None;
    let mut fell = None;
    let mut hidden = None;
    let mut sound = None;
    for k in 0..200u64 {
        sim.step(TickInput::default());
        let st = obj_state(&sim, id);
        if t.is_none() && st == state::START {
            t = Some(k);
        }
        if sound.is_none() && played(&sim, platform::CRUMBLE_SOUND) {
            sound = Some(k);
            assert_eq!(st, state::RUNNING_ANIM);
        }
        if fell.is_none() && t.is_some() && !solid(&sim, id) {
            fell = Some(k);
            assert_eq!(sim.objects().get(id).unwrap().frame, 6);
            let c = sim.player().unwrap();
            assert!(c.flags & flags::ON_ELEVATOR == 0, "Claw falls at once");
        }
        if hidden.is_none() && sim.objects().get(id).unwrap().draw_flags & draw_flags::HIDDEN != 0 {
            hidden = Some(k);
            assert_eq!(st, state::GONE);
        }
        if t.is_none() || fell.is_none() {
            // Riding until it gives way.
            if t.is_some() {
                assert!(riding(&sim, id));
            }
        }
    }
    let t = t.expect("the peg saw Claw");
    assert_eq!(sound, Some(t + 2));
    assert_eq!(fell, Some(t + 15));
    assert_eq!(hidden, Some(t + 36));
    assert_eq!(sim.objects().get(id).unwrap().frame, 12);
    // It stays gone ...
    idle(&mut sim, 100);
    assert_eq!(obj_state(&sim, id), state::GONE);
    // ... until Claw's next attempt.
    sim.with_world(|w| {
        platform::step(
            w,
            id,
            Rider {
                on: false,
                attempt: 1,
            },
        )
    });
    assert_eq!(obj_state(&sim, id), state::SET_UP);
    sim.step(TickInput::default());
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.state, state::STOPPED);
    assert_eq!(o.frame, 1);
    assert_eq!(o.type_flags, PLATFORM);
    assert_eq!(o.draw_flags & draw_flags::HIDDEN, 0);
}

/// Ticks (0-based) at which the peg's solidity changes, with the solidity
/// after the change.
fn toggles(sim: &mut Sim, id: ObjectId, n: u64) -> Vec<(u64, bool)> {
    let mut was = true;
    let mut out = Vec::new();
    for k in 0..n {
        sim.step(TickInput::default());
        let now = solid(sim, id);
        if now != was {
            out.push((k, now));
            was = now;
        }
    }
    out
}

#[test]
fn toggle_pegs_follow_their_phases() {
    // Live from the level start: the clock edge is seen one call late.
    for (logic, first_off_ms) in [
        ("TogglePeg2", 54),
        ("TogglePeg3", 738),
        ("TogglePeg4", 1476),
        ("TogglePeg", 1530),
    ] {
        let (mut sim, id) = peg_sim(logic, 1100, &[]);
        let edges = toggles(&mut sim, id, 400);
        let (k, solid) = edges[0];
        assert!(!solid);
        assert_eq!(18 * (k + 1), first_off_ms, "{logic}");
    }
}

#[test]
fn a_toggle_peg_cycle_takes_its_animation_time() {
    let (mut sim, id) = peg_sim("TogglePeg2", 1100, &[]);
    let mut slides = Vec::new();
    let mut edges = Vec::new();
    let mut was = true;
    let mut states = Vec::new();
    for k in 0..500u64 {
        sim.step(TickInput::default());
        if played(&sim, platform::SLIDE_SOUND) {
            slides.push(k);
        }
        states.push(obj_state(&sim, id));
        let now = solid(&sim, id);
        if now != was {
            edges.push((k, now));
            was = now;
        }
    }
    // Off at ms 54 (tick 2), in the wall 34 ticks later.
    assert_eq!(edges[0], (2, false));
    assert_eq!(states[2 + 34], state::IN);
    assert_eq!(states[2 + 33], state::SLIDING_IN);
    // The clock turns it out again at ms 750: the first call after is
    // tick 41 (ms 756); it is solid again 18 calls later.
    assert_eq!(states[41], state::START);
    assert_eq!(edges[1], (59, true));
    assert_eq!(&slides[..2], &[2, 42]);
    // Off again when the clock passes ms 2250: the call at ms 2268 sees it
    // and the next one starts the slide. Then every 3000 ms.
    assert_eq!(edges[2], (126, false));
    assert!((166..=167).contains(&(edges[4].0 - edges[2].0)));
}

#[test]
fn smart_toggle_pegs_stay_out() {
    let (mut sim, id) = peg_sim("TogglePeg", 1100, &[(SMARTS, 1)]);
    assert!(toggles(&mut sim, id, 600).is_empty());
    assert_eq!(obj_state(&sim, id), state::STOPPED);
}

#[test]
fn toggle_peg_timing_comes_from_the_record() {
    // 2000 ms out, 2000 ms in, offset 500 ms.
    let (mut sim, id) = peg_sim(
        "TogglePeg",
        1100,
        &[(SPEED_X, 2000), (SPEED_Y, 2000), (SPEED, 500)],
    );
    let edges = toggles(&mut sim, id, 600);
    let on = |ms: u64| platform::toggle_on(ms as u32, 500, 2000, 4000);
    for &(k, solid) in &edges {
        let ms = 18 * (k + 1);
        if solid {
            // Out again 18 calls after the call that saw the clock on.
            assert!(on(ms - 18 * 18), "tick {k}");
        } else {
            // In one call after the call that saw it off.
            assert!(!on(ms - 18), "tick {k}");
        }
    }
    assert!(edges.len() >= 4);
}

#[test]
fn decorations_loop_and_never_collide() {
    let mut torch = spec("BehindAniCandy", 1100, 800, 0, &[]);
    torch.frame = 2;
    let mut hand = spec("FrontAniCandy", 1200, 800, 5000, &[]);
    hand.anim = "TEST_WAVE".into();
    let mut level = level_with(vec![
        torch,
        hand,
        spec("FrontCandy", 1300, 800, 0, &[]),
        spec("BehindCandy", 1400, 800, 1000, &[]),
        spec("DoNothing", 1500, 800, 0, &[]),
    ]);
    add_anim(
        &mut level,
        "TEST_WAVE",
        vec![
            AnimFrame {
                image_op: 3,
                image_index: 7,
                duration: 40,
                ..AnimFrame::default()
            },
            AnimFrame {
                image_op: 3,
                image_index: 9,
                duration: 40,
                ..AnimFrame::default()
            },
        ],
    );
    let mut sim = Sim::new(with_frames(level, (1, 4)), SimConfig::default());
    let torch = find(&sim, "BehindAniCandy");
    let hand = find(&sim, "FrontAniCandy");
    sim.step(TickInput::default());
    let o = sim.objects().get(torch).unwrap();
    assert_eq!((o.state, o.z, o.frame), (decor::SET_UP, decor::BEHIND_Z, 2));
    assert_eq!(sim.objects().get(find(&sim, "FrontCandy")).unwrap().z, 5100);
    assert_eq!(
        sim.objects().get(find(&sim, "BehindCandy")).unwrap().z,
        1000
    );
    let mut frames = Vec::new();
    let mut hand_frames = Vec::new();
    for _ in 0..48 {
        // Claw walks through them.
        sim.step(hold(RIGHT));
        frames.push(sim.objects().get(torch).unwrap().frame);
        hand_frames.push(sim.objects().get(hand).unwrap().frame);
        for e in sim.collision_events() {
            let (a, b) = match *e {
                CollisionEvent::Attack { attacker, victim }
                | CollisionEvent::Hit { attacker, victim }
                | CollisionEvent::Damage { attacker, victim } => (attacker, victim),
            };
            for id in [a, b] {
                let logic = &*sim.objects().get(id).unwrap().logic;
                assert!(!decor::NAMES.contains(&logic), "{logic} collided");
            }
        }
    }
    let changes: Vec<i32> = frames.chunks(6).map(|c| c[0]).collect();
    assert_eq!(changes, vec![3, 4, 1, 2, 3, 4, 1, 2]);
    assert_eq!(&hand_frames[..6], &[7, 7, 7, 9, 9, 9]);
    for (_, o) in sim.objects().all() {
        if decor::NAMES.contains(&&*o.logic) {
            assert!(o.flags & flags::NO_COLLIDE != 0, "{}", o.logic);
        }
    }
    let still = find(&sim, "DoNothing");
    assert_eq!(sim.objects().get(still).unwrap().frame, 1);
}

#[test]
fn pegs_without_their_animations_still_work() {
    // A level without the peg animations: the crumble ends at once.
    let mut level = flat_level();
    level.objects = vec![spec("CrumblingPeg", 1100, 800, 0, &[])];
    let mut sim = Sim::new(level, SimConfig::default());
    let id = find(&sim, "CrumblingPeg");
    land_on(&mut sim, id);
    idle(&mut sim, 10);
    assert_eq!(obj_state(&sim, id), state::GONE);
    assert_eq!(
        sim.objects().get(id).unwrap().type_flags,
        NOT_PLATFORM,
        "not a platform once gone"
    );
}

/// Claw's code counts a death: the attempt counter rises, as the lives
/// step of a death does.
fn count_death(sim: &mut Sim) {
    sim.with_world(|w| w.claw.stats.count_attempt());
}

/// Puts Claw on the floor at `x`, off every platform.
fn stand_at(sim: &mut Sim, x: i32) {
    sim.teleport_player(x, 899);
    idle(sim, 3);
}

#[test]
fn a_death_sends_trigger_elevators_back_and_brings_crumbled_pegs_back() {
    let level = with_frames(
        level_with(vec![
            spec(
                "TriggerElevator",
                1500,
                700,
                0,
                &[(Y_MIN, 500), (Y_MAX, 700)],
            ),
            spec("CrumblingPeg", 1100, 800, 0, &[]),
        ]),
        (1, 12),
    );
    let cfg = SimConfig {
        activation: ActivationConfig::widened((100_000, 100_000)),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level, cfg);
    let lift = find(&sim, "TriggerElevator");
    let peg = find(&sim, "CrumblingPeg");
    // Ride the elevator up, then leave it running.
    land_on(&mut sim, lift);
    idle(&mut sim, 30);
    stand_at(&mut sim, 1800);
    idle(&mut sim, 10);
    assert_ne!(pos(&sim, lift), (1500, 700), "it runs for good");
    // Crumble the peg. A death during the crumble does not bring it back:
    // the peg notes the counter when the crumble ends.
    land_on(&mut sim, peg);
    idle(&mut sim, 20);
    assert_eq!(obj_state(&sim, peg), state::RUNNING_ANIM);
    count_death(&mut sim);
    // That death sends the elevator back where it was placed, waiting.
    sim.step(TickInput::default());
    assert_eq!(pos(&sim, lift), (1500, 700));
    idle(&mut sim, 40);
    assert_eq!(pos(&sim, lift), (1500, 700), "waits for Claw again");
    assert_eq!(obj_state(&sim, peg), state::GONE);
    stand_at(&mut sim, 1800);
    idle(&mut sim, 20);
    assert_eq!(obj_state(&sim, peg), state::GONE);
    // The next death brings the peg back, whole, solid and drawn.
    count_death(&mut sim);
    sim.step(TickInput::default());
    assert_eq!(obj_state(&sim, peg), state::SET_UP);
    sim.step(TickInput::default());
    let o = sim.objects().get(peg).unwrap();
    assert_eq!((o.state, o.frame), (state::STOPPED, 1));
    assert_eq!(o.type_flags, PLATFORM);
    assert_eq!(o.draw_flags & draw_flags::HIDDEN, 0);
    assert_eq!(pos(&sim, lift), (1500, 700));
}

#[test]
fn the_level_number_picks_the_platform_rectangles() {
    let mut level = level_with(vec![
        track_elevator(1160),
        spec("TogglePeg", 1250, 800, 0, &[]),
    ]);
    level.number = 7;
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let lift = sim.objects().get(find(&sim, "Elevator")).unwrap();
    assert_eq!(lift.hit_rect, platform::surface(7, SurfaceKind::Elevator));
    assert_eq!(lift.hit_rect.top, 70);
    let peg = sim.objects().get(find(&sim, "TogglePeg")).unwrap();
    assert_eq!(peg.hit_rect, platform::surface(7, SurfaceKind::TogglePeg));
    // The trap elevator of level 10 uses the big rectangle.
    let mut trap = track_elevator(1160);
    trap.image_set = "LEVEL_TRAPELEVATOR".into();
    let mut level = level_with(vec![trap.clone()]);
    level.number = 10;
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let lift = sim.objects().get(find(&sim, "Elevator")).unwrap();
    assert_eq!(
        lift.hit_rect,
        platform::surface(10, SurfaceKind::BigElevator)
    );
    let mut sim = Sim::new(level_with(vec![trap]), SimConfig::default());
    sim.step(TickInput::default());
    let lift = sim.objects().get(find(&sim, "Elevator")).unwrap();
    assert_eq!(lift.hit_rect, platform::surface(1, SurfaceKind::Elevator));
}

/// True when the snapshot draws `id` without interpolation.
fn drawn_at_once(sim: &Sim, snap: &claw_sim::SimSnapshot, id: ObjectId) -> bool {
    let serial = sim.objects().get(id).unwrap().serial;
    snap.renderables
        .iter()
        .find(|r| r.id == serial)
        .is_some_and(|r| r.teleported)
}

#[test]
fn an_elevator_set_up_away_from_its_clock_place_jumps_there_at_once() {
    // Live from the start: its first move is a normal one.
    let mut sim = Sim::new(level_with(vec![track_elevator(1160)]), SimConfig::default());
    let id = find(&sim, "Elevator");
    for _ in 0..40 {
        let snap = sim.step(TickInput::default());
        assert!(!drawn_at_once(&sim, &snap, id), "tick {}", sim.tick());
    }
    // Woken late, it is set up where it was placed and its first move takes
    // it to where the clock puts it, which is drawn as a jump.
    let far = 9160;
    let mut sim = Sim::new(level_with(vec![track_elevator(far)]), SimConfig::default());
    let id = find(&sim, "Elevator");
    idle(&mut sim, 140);
    assert_eq!(pos(&sim, id), (far, 700));
    sim.teleport_player(far, 800);
    let mut first_move = None;
    for _ in 0..40 {
        let snap = sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        if first_move.is_none() && o.state == state::RUNNING && o.platform_move != (0, 0) {
            first_move = Some(o.platform_move);
            assert!(o.platform_move.0.abs() > 6, "{:?}", o.platform_move);
            assert!(drawn_at_once(&sim, &snap, id), "the jump is not swept");
        } else if first_move.is_some() {
            assert!(!drawn_at_once(&sim, &snap, id), "later moves are smooth");
        }
    }
    assert!(first_move.is_some(), "it woke and moved");
}
