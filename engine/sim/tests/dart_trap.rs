//! Dart traps on synthetic levels: the set-up (no picture, a safe object
//! watching its rectangle for Claw, its defaults), the trigger, the puff's
//! pictures and blow, the arrow on the call that ends the puff, the wait
//! and the repeat while Claw stays in the rectangle, the arrow's picture,
//! flight and blow, the puff aimed up, the first-shot delay of a fresh
//! trigger, and the look of a trap asleep in the side areas of a wide
//! display.

mod common;

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::input::ScriptTuning;
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::dart_trap::{self, DartTrapData, arrow, state};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{ObjectId, Rect, draw_flags, flags, types};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

const RIGHT_PUFF: &str = "LEVEL_SHOOTERS_PUFFDARTRIGHT";
const LEFT_PUFF: &str = "LEVEL_SHOOTERS_PUFFDARTLEFT";
const UP_PUFF: &str = "LEVEL_SHOOTERS_PUFFDARTUP";
/// The trap of the tests, at Claw's height on the flat level.
const AT: (i32, i32) = (1000, 880);

fn image(i: i32, next_op: i32) -> AnimFrame {
    AnimFrame {
        image_op: 3,
        image_index: i,
        duration: 100,
        next_op,
        ..AnimFrame::default()
    }
}

/// A puff of pictures `images`, 100 ms each, blowing on picture 4.
fn puff(key: &str, images: std::ops::RangeInclusive<i32>) -> Arc<Animation> {
    let last = *images.end();
    let frames: Vec<AnimFrame> = images
        .clone()
        .map(|i| image(i, if i == last { 9 } else { 0 }))
        .collect();
    let frame_sounds = images
        .map(|i| {
            if i == 4 {
                vec![Some(dart_trap::SOUND.to_string())]
            } else {
                Vec::new()
            }
        })
        .collect();
    Arc::new(Animation {
        key: key.to_string(),
        frames,
        frame_sounds,
        ..Animation::default()
    })
}

/// The flat level with the puffs, the arrows' four pictures and the
/// blow's sound, and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    for set in [RIGHT_PUFF, LEFT_PUFF, UP_PUFF] {
        let mut f = ImageFrames::range(1, 5);
        for i in 1..=5 {
            f.boxes.insert(i, frame_box(40, 20, 0, 0));
        }
        level.images.insert(set.into(), f);
    }
    level
        .animations
        .insert(RIGHT_PUFF.into(), puff(RIGHT_PUFF, 1..=5));
    level
        .animations
        .insert(LEFT_PUFF.into(), puff(LEFT_PUFF, 1..=5));
    level
        .animations
        .insert(UP_PUFF.into(), puff(UP_PUFF, 2..=5));
    let mut f = ImageFrames::range(1, 4);
    f.boxes.insert(1, frame_box(60, 6, 29, 0));
    f.boxes.insert(2, frame_box(60, 6, -28, 0));
    f.boxes.insert(3, frame_box(8, 57, 0, 28));
    f.boxes.insert(4, frame_box(8, 57, 0, -29));
    level.images.insert(arrow::IMAGE_SET.into(), f);
    level.sounds.insert(dart_trap::SOUND.into(), Some(300));
    level.objects = objects;
    level
}

/// A trap at [`AT`] puffing `set` (`direction`, arrow picture `user0`),
/// watching (left, top, right, bottom), waiting `wait` ms.
fn trap(set: &str, direction: i32, user0: i32, bounds: [i32; 4], wait: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("TProjectile", AT.0, AT.1, 0);
    s.image_set = set.into();
    s.anim = set.into();
    s.frame = 6;
    for (i, v) in dart_trap::BOUNDS_FIELDS.into_iter().zip(bounds) {
        s.record[i] = v;
    }
    s.record[dart_trap::WAIT_FIELD] = wait;
    s.record[dart_trap::DIRECTION_FIELD] = direction;
    s.record[dart_trap::USER0_FIELD] = user0;
    s.record[55] = 500;
    s.record[56] = 500;
    s.record[16] = 20;
    s
}

/// The trap shooting right at a rectangle 1050..1400 x 800..990.
fn right_trap(wait: i32) -> ObjectSpec {
    trap(RIGHT_PUFF, 6, 1, [1050, 800, 1400, 990], wait)
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

fn data(sim: &Sim, id: ObjectId) -> DartTrapData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::DartTrap(d) => d.clone(),
        d => panic!("{d:?}"),
    }
}

/// One tick of a trap: its state, picture, shut-off flag, arrows shot so
/// far, and whether it blew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Step {
    state: i32,
    frame: i32,
    shut: bool,
    shots: u32,
    blow: bool,
    health: i32,
}

fn record(sim: &mut Sim, id: ObjectId, n: usize) -> Vec<Step> {
    (0..n)
        .map(|_| {
            sim.step(TickInput::default());
            let o = sim.objects().get(id).unwrap();
            Step {
                state: o.state,
                frame: o.frame,
                shut: o.flags & flags::NO_COLLIDE != 0,
                shots: data(sim, id).shots,
                blow: sim
                    .last_audio()
                    .iter()
                    .any(|c| matches!(c, AudioCue::Play(s) if s.key == dart_trap::SOUND)),
                health: sim.claw().stats.health,
            }
        })
        .collect()
}

fn entries(steps: &[Step], s: i32) -> Vec<usize> {
    (0..steps.len())
        .filter(|&k| steps[k].state == s && (k == 0 || steps[k - 1].state != s))
        .collect()
}

/// The rows on which an arrow was shot.
fn shots(steps: &[Step]) -> Vec<usize> {
    (1..steps.len())
        .filter(|&k| steps[k].shots > steps[k - 1].shots)
        .collect()
}

#[test]
fn a_trap_is_set_up_invisible_watching_its_rectangle() {
    let mut s = trap(RIGHT_PUFF, 0, 1, [1050, 800, 1400, 990], 0);
    s.record[55] = 0;
    s.record[56] = 0;
    let mut sim = sim_with(600, vec![s]);
    sim.step(TickInput::default());
    let o = sim.objects().get(find(&sim, "TProjectile")).unwrap();
    assert_eq!((o.state, o.frame), (state::IDLE, 0));
    assert!(o.flags & flags::SAFE != 0);
    assert_eq!((o.type_flags, o.attack_type_flags), (1, types::PLAYER));
    assert_eq!(o.attack_rect, Rect::new(50, -80, 400, 110));
    assert_eq!(o.z, dart_trap::DEFAULT_Z);
    // Its defaults: 100 px/s, 500 ms, left, and 5 damage whatever the
    // record says.
    assert_eq!((o.speed_x, o.speed_y, o.damage), (100, 100, 5));
    assert_eq!(dart_trap::wait_ms(o), dart_trap::DEFAULT_WAIT_MS);
    assert_eq!(dart_trap::direction(o), dart_trap::DEFAULT_DIRECTION);
}

#[test]
fn a_trap_puffs_shoots_on_the_31st_tick_and_repeats_while_claw_stays() {
    // Claw stands in the rectangle from the start.
    let mut sim = sim_with(1300, vec![right_trap(1000)]);
    let id = find(&sim, "TProjectile");
    let steps = record(&mut sim, id, 400);
    let firing = entries(&steps, state::FIRING);
    // 31 + 56 ticks apart with a 1000 ms wait.
    assert!(firing.len() >= 4);
    assert!(firing.windows(2).all(|w| w[1] - w[0] == 87), "{firing:?}");
    let t = firing[1];
    // Shut off from Claw from its trigger until the wait ends, and shut
    // again by the trigger on the tick the wait ends while he stays.
    assert!(steps[firing[0]..].iter().all(|s| s.shut));
    // No picture on T, pictures 1 to 5 from T + 1 for 6 ticks each, none
    // again from T + 31.
    assert_eq!(steps[t].frame, 0);
    let frames: Vec<i32> = (t + 1..t + 31).map(|k| steps[k].frame).collect();
    let want: Vec<i32> = (1..=5).flat_map(|f| [f; 6]).collect();
    assert_eq!(frames, want);
    assert_eq!((steps[t + 31].frame, steps[t + 31].state), (0, state::WAIT));
    assert!((t + 31..t + 87).all(|k| steps[k].frame == 0));
    // The blow on T + 19, the arrow on T + 31.
    let blows: Vec<usize> = (0..steps.len()).filter(|&k| steps[k].blow).collect();
    assert_eq!(blows, firing.iter().map(|f| f + 19).collect::<Vec<_>>());
    let shot = shots(&steps);
    assert_eq!(
        shot,
        firing
            .iter()
            .map(|f| f + 31)
            .filter(|&k| k < 400)
            .collect::<Vec<_>>()
    );
    // The default wait: 31 + 28 ticks.
    let mut sim = sim_with(1300, vec![right_trap(0)]);
    let id = find(&sim, "TProjectile");
    let firing = entries(&record(&mut sim, id, 300), state::FIRING);
    assert!(firing.windows(2).all(|w| w[1] - w[0] == 59), "{firing:?}");
    assert_eq!(dart_trap::period_ticks(500, false), 59);
}

#[test]
fn a_trap_waits_for_claw_in_its_rectangle() {
    // Claw outside the rectangle: nothing.
    let mut sim = sim_with(1450, vec![right_trap(1000)]);
    let id = find(&sim, "TProjectile");
    let steps = record(&mut sim, id, 100);
    assert!(steps.iter().all(|s| s.state == state::IDLE && s.frame == 0));
    // He walks in: the trap triggers on the tick his box first meets it.
    let mut sim = sim_with(1460, vec![right_trap(1000)]);
    let id = find(&sim, "TProjectile");
    sim.step(press(LEFT));
    let mut t = None;
    for k in 1..60 {
        sim.step(hold(LEFT));
        if t.is_none() && sim.objects().get(id).unwrap().state == state::FIRING {
            t = Some(k);
        }
    }
    assert!(t.is_some());
}

#[test]
fn its_arrow_flies_from_the_trap_and_costs_claw_5() {
    let mut sim = sim_with(1300, vec![right_trap(1000)]);
    let id = find(&sim, "TProjectile");
    let mut arrow_id = None;
    let mut health = 100;
    let mut xs = Vec::new();
    let mut hit_at = None;
    for k in 0..80 {
        sim.step(TickInput::default());
        if arrow_id.is_none() {
            arrow_id = data(&sim, id).last_arrow;
            if let Some(a) = arrow_id {
                let o = sim.objects().get(a).unwrap();
                assert_eq!((o.x, o.y, o.z), (AT.0, AT.1, dart_trap::DEFAULT_Z - 1));
                assert_eq!(o.image_set, arrow::IMAGE_SET);
                assert_eq!(o.frame, 1);
                assert_eq!(o.attack_rect, Rect::new(-1, -3, 58, 2));
                assert_eq!((o.type_flags, o.damage), (types::ENEMY_SHOT, 5));
            }
        }
        if let Some(o) = arrow_id.and_then(|a| sim.objects().get(a)) {
            xs.push(o.x);
        }
        let h = sim.claw().stats.health;
        if h < health && hit_at.is_none() {
            hit_at = Some(k);
            assert_eq!(health - h, arrow::DAMAGE);
        }
        health = h;
    }
    assert!(hit_at.is_some());
    // 9 px a tick at 500 px/s.
    assert!(xs.len() > 5);
    assert!(xs.windows(2).all(|w| w[1] - w[0] == 9), "{xs:?}");
}

#[test]
fn an_arrow_shot_left_keeps_its_own_picture_unturned() {
    let mut s = trap(LEFT_PUFF, 4, 2, [600, 800, 950, 990], 1000);
    s.record[dart_trap::BOUNDS_FIELDS[0]] = 600;
    let mut sim = sim_with(700, vec![s]);
    let id = find(&sim, "TProjectile");
    let mut seen = false;
    for _ in 0..40 {
        sim.step(TickInput::default());
        if let Some(a) = data(&sim, id).last_arrow
            && let Some(o) = sim.objects().get(a)
            && !seen
        {
            seen = true;
            assert_eq!(o.frame, 2);
            assert_eq!(o.draw_flags & draw_flags::MIRROR, 0);
            assert_eq!(o.attack_rect, Rect::new(-58, -3, 1, 2));
            assert_eq!(o.speed_x, -500);
        }
    }
    assert!(seen);
}

#[test]
fn a_puff_aimed_up_is_six_ticks_shorter() {
    let mut sim = sim_with(1050, vec![trap(UP_PUFF, 8, 4, [950, 700, 1150, 990], 1000)]);
    let id = find(&sim, "TProjectile");
    let steps = record(&mut sim, id, 300);
    let firing = entries(&steps, state::FIRING);
    assert!(
        firing.windows(2).all(|w| w[1] - w[0] == 25 + 56),
        "{firing:?}"
    );
    let t = firing[1];
    assert_eq!(shots(&steps)[1], t + 25);
    let blows: Vec<usize> = (0..steps.len()).filter(|&k| steps[k].blow).collect();
    assert_eq!(blows[1], t + 13);
    // Shot straight up, it never meets Claw; the trap draws no random
    // number.
    assert!(steps.iter().all(|s| s.health == 100));
}

#[test]
fn a_fresh_trigger_waits_the_first_shot_delay() {
    // A 250 ms delay: the puff starts 13 ticks late after a fresh trigger,
    // not after a repeat.
    let mut s = right_trap(1000);
    s.record[dart_trap::SMARTS_FIELD] = 250;
    let mut sim = sim_with(1300, vec![s]);
    let id = find(&sim, "TProjectile");
    let steps = record(&mut sim, id, 300);
    let firing = entries(&steps, state::FIRING);
    let shot = shots(&steps);
    assert_eq!(shot[0] - firing[0], 31 + 13);
    assert_eq!(shot[1] - firing[1], 31);
    assert_eq!(firing[1] - firing[0], 31 + 13 + 56);
}

#[test]
fn a_trap_draws_no_random_number() {
    let mut sim = sim_with(1050, vec![trap(UP_PUFF, 8, 4, [950, 700, 1150, 990], 500)]);
    let before = sim.with_world(|w| w.rng.clone());
    idle(&mut sim, 300);
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
}

#[test]
fn asleep_a_trap_is_not_drawn() {
    let level = level_with(vec![]);
    let mut o = logic::new_object("TProjectile", 5000, 900, 5000);
    o.image_set = RIGHT_PUFF.into();
    o.frame = 6;
    assert_eq!(logic::first_look(&level, &o), Some(Vec::new()));
    o.state = state::WAIT;
    assert_eq!(logic::first_look(&level, &o), None);
}
