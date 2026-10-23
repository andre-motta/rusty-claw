//! The eleventh level's lasers on synthetic levels: the 159-tick cycle (84
//! ticks waiting, a 76-tick pass dangerous for 75), the fixed box, the
//! picture and box reset at every wait, the zaps carried by the pass, the
//! two hits a pass on a Claw standing in the beam (1 health each, 48
//! ticks apart), a wait of 0, and a laser that sleeps through its wait
//! firing as it wakes.

mod common;

use std::sync::Arc;

use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::laser::{self, ATTACK_RECT, state as laser_state};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A standing Claw on the flat level's floor (top at 960).
const STAND_Y: i32 = 899;
/// A laser 48 px above a standing Claw, as on the eleventh level.
const LASER_Y: i32 = STAND_Y - 48;

/// The pass: pictures 1 to 25, 50 ms each, finishing on the last, the
/// zaps on pictures 5 to 21.
fn pass() -> Animation {
    let mut frames: Vec<AnimFrame> = (1..=25)
        .map(|i| AnimFrame {
            image_op: 3,
            image_index: i,
            duration: 50,
            flags: 0x2,
            ..AnimFrame::default()
        })
        .collect();
    frames.last_mut().unwrap().next_op = 9;
    let frame_sounds = (0..25)
        .map(|k| match k {
            4 | 8 | 12 | 16 | 20 => vec![Some(laser::ZAP1.to_string())],
            6 | 10 | 14 | 18 => vec![Some(laser::ZAP2.to_string())],
            _ => Vec::new(),
        })
        .collect();
    Animation {
        key: laser::ANIM.to_string(),
        frames,
        frame_sounds,
        ..Animation::default()
    }
}

/// The flat level as the eleventh level, Claw starting at `start`, with
/// `objects`.
fn level_with(start: (i32, i32), objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 11;
    level.start = start;
    level
        .animations
        .insert(laser::ANIM.to_string(), Arc::new(pass()));
    level
        .images
        .insert(laser::IMAGE_SET.into(), ImageFrames::range(1, 25));
    level.objects = objects;
    level
}

/// A laser at `x` with a wait of `wait_ms`, as the level files place it
/// (frame -1).
fn laser_at(x: i32, wait_ms: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("Laser", x, LASER_Y, 0);
    s.image_set = laser::IMAGE_SET.into();
    s.frame = -1;
    s.record[laser::WAIT_FIELD] = wait_ms;
    s
}

fn original() -> SimConfig {
    SimConfig {
        tuning: ScriptTuning::Original.tuning(),
        ..SimConfig::default()
    }
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

/// Where the lasers are put: out of Claw's reach at the level's start, so
/// each wakes in a tick, as on the eleventh level.
const FAR: i32 = 5000;

/// Puts Claw down at `x` with a warp and returns the tick count after the
/// tick whose game time the laser was set up at (C): the ticks after it
/// count from 1. The warp's second pass wakes the laser and makes its
/// first call between two ticks, at the game time of the tick that
/// touched the warp, so that tick is the one it waits from, as if it had
/// woken in it.
fn set_up(sim: &mut Sim, id: ObjectId, x: i32) -> u64 {
    sim.step(TickInput::default());
    assert_eq!(sim.objects().get(id).unwrap().state, 0, "asleep");
    sim.with_world(|w| w.claw.special.warp_to = Some((x, STAND_Y)));
    for _ in 0..10 {
        let touched = sim.tick();
        sim.step(TickInput::default());
        if sim.objects().get(id).unwrap().state == laser_state::IDLE {
            return touched;
        }
    }
    panic!("the laser never ran");
}

/// One tick of a laser: its picture after the tick, its box in it and
/// the zap it played (1, 2 or 0).
fn look(sim: &Sim, id: ObjectId) -> (i32, Rect, u8) {
    let o = sim.objects().get(id).unwrap();
    let zap = sim
        .last_audio()
        .iter()
        .find_map(|a| match a {
            AudioCue::Play(s) if s.key == laser::ZAP1 => Some(1),
            AudioCue::Play(s) if s.key == laser::ZAP2 => Some(2),
            _ => None,
        })
        .unwrap_or(0);
    (o.frame, o.attack_rect, zap)
}

#[test]
fn a_cycle_is_159_ticks_dangerous_for_75_from_the_84th() {
    // Claw 500 px away: the laser is live and out of his reach.
    let mut sim = Sim::new(
        level_with((1000, STAND_Y), vec![laser_at(FAR, 1500)]),
        original(),
    );
    let id = find(&sim, "Laser");
    let c = set_up(&mut sim, id, FAR - 500);
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.type_flags, laser::TYPE);
    assert_eq!(o.hit_type_flags, laser::HIT_MASK);
    assert_eq!(o.frame, 1, "its first picture");
    assert!(!o.attack_rect.is_set(), "harmless after its set-up");
    assert_eq!(laser::data_of(o).map(|d| d.wait_ms), Some(1500));
    let rng = sim.rng_state();
    let mut ticks = vec![(sim.tick() - c, look(&sim, id))];
    for _ in 0..600 {
        sim.step(TickInput::default());
        ticks.push((sim.tick() - c, look(&sim, id)));
    }
    assert_eq!(sim.rng_state(), rng, "no random draw");
    let at = |t: u64| ticks.iter().find(|(k, _)| *k == t).unwrap().1;
    for n in 0..3 {
        let p = 159 * n;
        // Waiting through C + 83, the first picture shown.
        for t in p + 1..=p + 83 {
            assert_eq!((at(t).0, at(t).1), (1, Rect::UNSET), "tick {t}");
        }
        // Dangerous on C + 84 to C + 158, the same box throughout, one
        // picture every 3 ticks.
        for t in p + 84..=p + 158 {
            assert_eq!(at(t).1, ATTACK_RECT, "tick {t}");
            assert_eq!(at(t).0, 1 + ((t - p - 84) / 3) as i32, "tick {t}");
        }
        // The finishing call: the first picture and no box.
        assert_eq!((at(p + 159).0, at(p + 159).1), (1, Rect::UNSET));
        // The zaps: the first on the arc's first tick (C + 96), every 6
        // ticks, alternating, nine in all.
        let zaps: Vec<(u64, u8)> = (p + 1..=p + 159)
            .filter(|&t| at(t).2 != 0)
            .map(|t| (t - p, at(t).2))
            .collect();
        let want: Vec<(u64, u8)> = (0..9)
            .map(|i| (96 + 6 * i, if i % 2 == 0 { 1 } else { 2 }))
            .collect();
        assert_eq!(zaps, want, "pass {n}");
    }
}

#[test]
fn a_claw_in_the_beam_loses_1_on_the_first_dangerous_tick_and_48_later() {
    // Claw stands under the laser.
    let mut sim = Sim::new(
        level_with((1000, STAND_Y), vec![laser_at(FAR, 1500)]),
        original(),
    );
    let id = find(&sim, "Laser");
    let c = set_up(&mut sim, id, FAR);
    let mut hits = Vec::new();
    let mut health = sim.claw().stats.health;
    let mut x_at = Vec::new();
    for _ in 0..400 {
        sim.step(TickInput::default());
        let h = sim.claw().stats.health;
        if h < health {
            hits.push((sim.tick() - c, health - h));
        }
        health = h;
        x_at.push(sim.player().unwrap().x);
    }
    // On the first dangerous tick and 48 ticks later, 1 each, and the same
    // in the next pass: without his hit animation (the synthetic level
    // has none) the hit ends at once and does not knock him out of the
    // beam, which on the install it does (see install_level11_objects).
    assert_eq!(hits, [(84, 1), (132, 1), (243, 1), (291, 1)]);
    assert!(x_at.iter().all(|&x| x == FAR));
}

#[test]
fn a_wait_of_zero_fires_pass_after_pass() {
    let mut sim = Sim::new(
        level_with((1000, STAND_Y), vec![laser_at(FAR, 0)]),
        original(),
    );
    let id = find(&sim, "Laser");
    set_up(&mut sim, id, FAR - 500);
    let mut armed = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        armed.push(sim.objects().get(id).unwrap().attack_rect.is_set());
    }
    // 75 dangerous ticks, one harmless (the finishing call), and again.
    let harmless: Vec<usize> = (0..armed.len()).filter(|&k| !armed[k]).collect();
    assert_eq!(harmless.len(), 2, "{harmless:?}");
    assert_eq!(harmless[1] - harmless[0], 76);
    assert!(harmless[0] <= 75);
}

#[test]
fn a_laser_that_slept_through_its_wait_fires_as_it_wakes() {
    // Woken beside Claw, then Claw is put far away and back, the laser
    // asleep for longer than its wait.
    let cfg = SimConfig {
        activation: ActivationConfig::default(),
        ..original()
    };
    let mut sim = Sim::new(level_with((1000, STAND_Y), vec![laser_at(FAR, 1500)]), cfg);
    let id = find(&sim, "Laser");
    set_up(&mut sim, id, FAR - 500);
    // Put Claw far away: the laser sleeps.
    sim.teleport_player(8000, STAND_Y);
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    let o = sim.objects().get(id).unwrap();
    assert!(!o.attack_rect.is_set(), "asleep while waiting");
    assert_eq!(o.frame, 1);
    // Back beside it: it fires on its first call awake.
    sim.teleport_player(FAR - 500, STAND_Y);
    let mut first = None;
    for k in 0..20 {
        sim.step(TickInput::default());
        if sim.objects().get(id).unwrap().attack_rect.is_set() {
            first = Some(k);
            break;
        }
    }
    assert!(first.is_some_and(|k| k <= 2), "{first:?}");
}
