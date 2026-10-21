//! The bear sailor and Claw's held state on a synthetic floor with the
//! seventh level's animation timings: how a bear is built, its approach
//! (running, then walking, stopping just under 100 px from Claw), its blow
//! (live on its 16th tick, Home on its 35th) and its pauses, and the hug:
//! the grab within 50 px (19 ticks, Claw held where he stood), the squeeze
//! (Claw put 30 px in front of the bear at its height, a health point
//! every 100 ms from the first tick, the pictures that follow the bear's),
//! the release after 3000 ms (Claw standing 1 px up, then on the floor),
//! the escape on the tenth tick with a press (presses during the grab do
//! not count), the death on the squeeze that empties his health, Vader's
//! protection, and the ways a hug ends early: a hit from something else,
//! the bear's death, or a call scheduled while it hugs.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::bear_sailor::{
    self, BearSailorData, GRAB_ANIM, SQUEEZE_ANIM, STRIKE, STRIKE_ANIM,
};
use claw_sim::logic::enemy::{Deferred, EnemyState, field};
use claw_sim::logic::{ObjectData, enemy};
use claw_sim::object::{LogicKind, Object, ObjectId, engine_state};
use claw_sim::player::death::powerup;
use claw_sim::player::{hug, state};
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// An animation of (image, duration, event) elements; `once` makes the
/// last element finish it.
fn anim(key: &str, frames: &[(i32, i32, i32)], once: bool) -> Arc<Animation> {
    let pairs: Vec<(i32, i32)> = frames.iter().map(|&(i, d, _)| (i, d)).collect();
    let mut a = if once {
        Animation::once(key, &pairs)
    } else {
        Animation::looping(key, &pairs)
    };
    for (f, &(_, _, event)) in a.frames.iter_mut().zip(frames) {
        f.event = event;
    }
    Arc::new(a)
}

/// The blow: live on its third picture (calls 16 to 21), done on call 35.
const STRIKE1: &[(i32, i32, i32)] = &[
    (201, 90, 0),
    (202, 180, 0),
    (203, 100, 1),
    (204, 120, 0),
    (205, 100, 0),
    (205, 0, 0),
];
/// The grab: done on call 19.
const STRIKE2: &[(i32, i32, i32)] = &[(300, 90, 0), (301, 120, 0), (302, 100, 0), (302, 0, 0)];
/// The squeeze, a loop of 31 calls.
const STRIKE3: &[(i32, i32, i32)] = &[(304, 140, 0), (303, 120, 0), (302, 150, 0), (303, 110, 0)];
/// A flinch: 19 calls.
const HIT: &[(i32, i32, i32)] = &[(400, 50, 0), (401, 200, 0), (400, 50, 0), (400, 0, 0)];

fn bear_anims() -> Vec<Arc<Animation>> {
    let a = bear_sailor::CLASS.anims;
    vec![
        anim(STRIKE_ANIM, STRIKE1, true),
        anim(GRAB_ANIM, STRIKE2, true),
        anim(SQUEEZE_ANIM, STRIKE3, false),
        anim(a.hit_low, HIT, true),
        anim(a.hit_high, HIT, true),
        anim(a.jump, HIT, true),
        anim(a.home, &[(1, 0, 0)], true),
        anim(a.bounce, &[(1, 0, 0)], true),
        anim(a.advance, &[(10, 125, 0), (11, 125, 0)], false),
        anim(a.fast_advance, &[(12, 100, 0), (13, 100, 0)], false),
        anim(a.fall, &[(900, 100, 0)], false),
        anim(
            bear_sailor::IDLE_ANIM,
            &[(100, 500, 0), (101, 500, 0)],
            false,
        ),
    ]
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(anim(key, &[(200, 150, 0), (201, 200, 0)], true));
    }
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_WALK", &[(20, 100, 0), (21, 100, 0)], false));
    v.push(anim("CLAW_FALL", &[(278, 65, 0), (280, 80, 0)], false));
    v
}

/// Where the arena's bear stands and its patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor (feet rest on 959) as the seventh level, with a bear at
/// `AT` between `LIMITS`; every object awake.
fn arena() -> SimLevel {
    let mut level = flat_level();
    level.number = 7;
    for a in bear_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("BearSailor", AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    level.objects = vec![o];
    level
}

/// The arena with Claw put `dx` px from the bear on the first tick, with
/// `health`.
fn with_claw(seed: u32, dx: i32, health: i32) -> (Sim, ObjectId) {
    let level = arena();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(AT + dx, 899);
    sim.with_world(|w| w.claw.stats.health = health);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::BearSailor))
        .map(|(id, _)| id)
        .expect("a bear");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn bdata(sim: &Sim, id: ObjectId) -> BearSailorData {
    match &obj(sim, id).data {
        ObjectData::BearSailor(d) => d.clone(),
        d => panic!("bear data: {d:?}"),
    }
}

/// One row: Claw's state, x, y, health and picture, the bear's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    state: i32,
    x: i32,
    y: i32,
    health: i32,
    frame: i32,
    bear: Option<EnemyState>,
}

fn row(sim: &Sim, id: ObjectId) -> Row {
    let c = sim.player().expect("Claw");
    Row {
        state: c.state,
        x: c.x,
        y: c.y,
        health: sim.claw().stats.health,
        frame: c.frame,
        bear: sim
            .objects()
            .get(id)
            .and_then(|o| EnemyState::from_id(o.state)),
    }
}

/// Steps until the bear grabs Claw (at most `max` ticks), then `n` more
/// rows with `input(j)` (j counting from the grab, 0 on the grab's row);
/// returns the rows from the grab on.
fn hug_rows(
    sim: &mut Sim,
    id: ObjectId,
    max: usize,
    n: usize,
    input: impl Fn(usize) -> TickInput,
) -> Vec<Row> {
    for _ in 0..max {
        sim.step(TickInput::default());
        if state_of(sim, id) == Some(EnemyState::StartHug) {
            let mut rows = vec![row(sim, id)];
            for j in 1..=n {
                sim.step(input(j));
                rows.push(row(sim, id));
            }
            return rows;
        }
    }
    panic!("no grab within {max} ticks");
}

/// The first row from which the bear is no longer hugging.
fn released(rows: &[Row]) -> usize {
    rows.iter()
        .position(|r| !matches!(r.bear, Some(EnemyState::StartHug | EnemyState::Hug)))
        .expect("released")
}

#[test]
fn the_synthetic_animations_keep_the_seventh_levels_timings() {
    let level = arena();
    let calls = |key: &str| {
        let mut p = claw_sim::anim::AnimPlayer::default();
        p.set(level.animations[key].clone());
        let mut events = Vec::new();
        let mut frames = Vec::new();
        let mut n = 0;
        let mut frame = 0;
        while !p.is_done() && n < 40 {
            n += 1;
            let e = p.advance_in(18, frame, false, None);
            if let Some(f) = e.frame {
                frame = f;
            }
            frames.push(frame);
            if e.event == 1 {
                events.push(n);
            }
        }
        (n, events.first().copied(), frames)
    };
    let (n, first, _) = calls(STRIKE_ANIM);
    assert_eq!((n, first), (35, Some(16)));
    assert_eq!(calls(GRAB_ANIM).0, 19);
    assert_eq!(calls(bear_sailor::CLASS.anims.hit_low).0, 19);
    // The squeeze: 304 for 8 calls, 303 for 7, 302 for 9, 303 for 7.
    let (_, _, frames) = calls(SQUEEZE_ANIM);
    let mut want = vec![304; 8];
    want.extend([303; 7]);
    want.extend([302; 9]);
    want.extend([303; 7]);
    want.extend([304; 8]);
    assert_eq!(frames[..39], want[..39]);
}

#[test]
fn a_bear_is_built_idle_with_its_own_numbers() {
    let (sim, id) = with_claw(1, 600, 100);
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (10, 10));
    assert_eq!(&*o.image_set, bear_sailor::IMAGE_SET);
    let b = bdata(&sim, id);
    assert_eq!(b.enemy.smarts, 100);
    assert_eq!(b.enemy.ranges, bear_sailor::RANGES);
    assert_eq!(b.enemy.bounds, LIMITS);
    assert!(!b.enemy.sight);
    assert!(b.enemy.may_lift);
    assert_eq!((b.escape, b.looping), (0, false));
    assert_eq!(o.y, 959 - 61);
}

#[test]
fn it_runs_then_walks_at_claw_and_stops_just_under_100_px_away() {
    let (mut sim, id) = with_claw(4, 290, 10_000);
    let mut xs = vec![obj(&sim, id).x];
    let mut seen = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        xs.push(obj(&sim, id).x);
        seen.push(state_of(&sim, id));
        if seen.last() == Some(&Some(EnemyState::Pause))
            || seen.last() == Some(&Some(EnemyState::Strike))
        {
            break;
        }
    }
    // Running at 300 px/s (5 or 6 px a tick) while Claw is 150 to 500 px
    // away, walking at 120 (2 or 3 a tick) closer in.
    let steps: Vec<i32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(seen.contains(&Some(EnemyState::FastAdvance)));
    assert!(seen.contains(&Some(EnemyState::Advance)));
    for (k, s) in seen.iter().enumerate() {
        match s {
            Some(EnemyState::FastAdvance) => assert!((5..=6).contains(&steps[k]), "{k}"),
            Some(EnemyState::Advance) => assert!((2..=3).contains(&steps[k]), "{k}"),
            _ => {}
        }
    }
    let gap = sim.player().unwrap().x - obj(&sim, id).x;
    assert!((97..=99).contains(&gap), "{gap}");
}

#[test]
fn its_blow_is_live_on_its_16th_tick_and_over_on_its_35th() {
    let (mut sim, id) = with_claw(2, 80, 10_000);
    let mut seen = Vec::new();
    for _ in 0..600 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        seen.push((
            state_of(&sim, id),
            o.attack_rect,
            sim.player().unwrap().state,
        ));
    }
    let strikes: Vec<usize> = (1..seen.len() - 40)
        .filter(|&k| seen[k].0 == Some(EnemyState::Strike) && seen[k - 1].0 != seen[k].0)
        .collect();
    assert!(!strikes.is_empty());
    for &s in &strikes {
        let live: Vec<usize> = (s..s + 40)
            .take_while(|&k| seen[k].0 == Some(EnemyState::Strike))
            .filter(|&k| seen[k].1 == STRIKE || seen[k].1 == STRIKE.mirrored())
            .map(|k| k - s)
            .collect();
        assert_eq!(live, [16], "{s}");
        let len = (s..s + 40)
            .take_while(|&k| seen[k].0 == Some(EnemyState::Strike))
            .count();
        assert_eq!(len, 35, "{s}");
    }
    // Between its blows it pauses 300 to 600 ms in its home pose.
    let pauses: Vec<usize> = (1..seen.len())
        .filter(|&k| seen[k].0 == Some(EnemyState::Pause) && seen[k - 1].0 != seen[k].0)
        .collect();
    for &p in &pauses[..pauses.len() - 1] {
        let len = (p..seen.len())
            .take_while(|&k| seen[k].0 == Some(EnemyState::Pause))
            .count();
        assert!((17..=34).contains(&len), "pause at {p}: {len}");
    }
}

#[test]
fn a_full_hug_squeezes_28_health_and_lets_go_after_3_seconds() {
    let (mut sim, id) = with_claw(3, 40, 100);
    let rows = hug_rows(&mut sim, id, 200, 200, |_| TickInput::default());
    // Grabbed where he stood, held there for the grab's 19 ticks.
    assert_eq!(rows[0].state, state::BEARHUG);
    let at = rows[0].x;
    assert!(
        rows[..19]
            .iter()
            .all(|r| r.x == at && r.state == state::BEARHUG)
    );
    assert!(
        rows[..19]
            .iter()
            .all(|r| r.bear == Some(EnemyState::StartHug))
    );
    // The hug from the 19th tick, Claw 30 px in front of the bear at its
    // height (1 px above his standing height), facing it.
    assert_eq!(rows[19].bear, Some(EnemyState::Hug));
    assert_eq!((rows[19].x, rows[19].y), (AT + 30, 898));
    assert!(!sim.claw().facing_right || at < AT);
    // A squeeze on j = 1, 7, .. 163 (j counted from the hug's start).
    let squeezes: Vec<usize> = (20..rows.len())
        .filter(|&k| rows[k].health < rows[k - 1].health)
        .map(|k| k - 19)
        .collect();
    let want: Vec<usize> = (0..28).map(|n| 1 + 6 * n).collect();
    assert_eq!(squeezes, want);
    // Let go on j = 167: Claw stands where he was held, then drops the
    // pixel to the floor.
    let r = released(&rows);
    assert_eq!(r - 19, 167);
    assert_eq!(rows[r].bear, Some(EnemyState::Home));
    assert_eq!((rows[r].state, rows[r].y), (state::STAND, 898));
    assert_eq!(rows[r + 1].state, state::FALL);
    let back = (r..rows.len())
        .find(|&k| rows[k].y == 899)
        .expect("on the floor");
    assert!(back - r <= 3, "{}", back - r);
    assert_eq!(rows[r].health, 72);
    // Claw's pictures follow the bear's squeeze: 452 x 8, 451 x 7, 450 x 9,
    // 451 x 7 from j = 1.
    let frames: Vec<i32> = rows[20..51].iter().map(|r| r.frame).collect();
    let mut want = vec![452; 8];
    want.extend([451; 7]);
    want.extend([450; 9]);
    want.extend([451; 7]);
    assert_eq!(frames, want);
    // The next grab waits for the bear's next Home step after 50 ms.
    let b = bdata(&sim, id);
    assert_eq!(b.hug_wait.len, 50);
}

#[test]
fn a_press_on_ten_ticks_of_the_hug_frees_claw() {
    // A new press on every tick from the grab on: the grab's presses do
    // not count, the tenth tick of the hug frees him.
    let (mut sim, id) = with_claw(3, 40, 100);
    let rows = hug_rows(&mut sim, id, 200, 60, |j| {
        press(if j % 2 == 0 { UP } else { DOWN })
    });
    assert_eq!(released(&rows) - 19, 10);
    assert_eq!(rows[released(&rows)].health, 98, "two squeezes");
    // One key pressed every other tick: out on the 19th.
    let (mut sim, id) = with_claw(3, 40, 100);
    let rows = hug_rows(&mut sim, id, 200, 60, |j| {
        if j % 2 == 0 {
            press(UP)
        } else {
            TickInput::default()
        }
    });
    assert_eq!(released(&rows) - 19, 19);
    assert_eq!(rows[released(&rows)].health, 97, "three squeezes");
    // Nine presses never free him.
    let (mut sim, id) = with_claw(3, 40, 100);
    let rows = hug_rows(&mut sim, id, 200, 200, |j| {
        if (22..40).contains(&j) && j % 2 == 0 {
            press(UP)
        } else {
            TickInput::default()
        }
    });
    assert_eq!(released(&rows) - 19, 167);
    assert_eq!(bdata(&sim, id).escape, 9);
}

#[test]
fn the_squeeze_that_empties_his_health_kills_him() {
    let (mut sim, id) = with_claw(3, 40, 5);
    let rows = hug_rows(&mut sim, id, 200, 60, |_| TickInput::default());
    let r = released(&rows);
    // The fifth squeeze, j = 25.
    assert_eq!(r - 19, 25);
    assert_eq!((rows[r].health, rows[r].state), (0, state::DEATH_FALL));
    assert_eq!(rows[r].bear, Some(EnemyState::Home));
}

#[test]
fn invulnerable_he_is_held_but_not_squeezed() {
    let (mut sim, id) = with_claw(3, 40, 100);
    sim.with_world(|w| {
        w.claw.stats.powerup_kind = powerup::VADER;
        w.claw.stats.powerup_ms = 60_000;
    });
    let rows = hug_rows(&mut sim, id, 200, 200, |_| TickInput::default());
    assert_eq!(released(&rows) - 19, 167);
    assert!(rows.iter().all(|r| r.health == 100));
}

#[test]
fn a_hit_from_something_else_ends_the_hug() {
    let (mut sim, id) = with_claw(3, 40, 100);
    let _ = hug_rows(&mut sim, id, 200, 30, |_| TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Hug));
    // Claw reels from a hit: the bear lets go on its next tick.
    sim.with_world(|w| {
        let claw = w.claw_id.unwrap();
        if let Some(o) = w.objects.get_mut(claw) {
            o.state = state::HIT;
        }
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_ne!(sim.player().unwrap().state, state::BEARHUG);
}

#[test]
fn the_bears_death_lets_claw_go() {
    let (mut sim, id) = with_claw(3, 40, 100);
    let _ = hug_rows(&mut sim, id, 200, 30, |_| TickInput::default());
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.state = engine_state::KILLED;
        }
    });
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.player().unwrap().state, state::STAND);
}

#[test]
fn a_call_scheduled_while_it_hugs_lets_claw_go_first() {
    let (mut sim, id) = with_claw(3, 40, 100);
    let _ = hug_rows(&mut sim, id, 200, 30, |_| TickInput::default());
    sim.with_world(|w| {
        if let Some(d) = enemy::data_mut(w, id) {
            d.deferred = Some(Deferred::GoIdle);
        }
    });
    sim.step(TickInput::default());
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn a_direction_held_into_the_grab_counts_as_pressed_on_the_next_tick() {
    // Claw walks left into the bear's reach and keeps Left held.
    let (mut sim, id) = with_claw(3, 70, 100);
    let mut grabbed = false;
    for k in 0..200 {
        sim.step(if k == 0 { press(LEFT) } else { hold(LEFT) });
        if state_of(&sim, id) == Some(EnemyState::StartHug) {
            grabbed = true;
            break;
        }
    }
    assert!(grabbed);
    sim.step(hold(LEFT));
    assert!(sim.claw_input().pressed.contains(claw_sim::Buttons::LEFT));
    // It does not count: the grab is not the hug.
    assert_eq!(bdata(&sim, id).escape, 0);
    assert!(hug::ESCAPE_KEYS.contains(claw_sim::Buttons::LEFT));
}

#[test]
fn lifted_its_line_takes_no_draw_of_its_own() {
    let (mut sim, id) = with_claw(3, 600, 100);
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| bear_sailor::pick_lift_voice(w, id));
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
    assert_eq!(bdata(&sim, id).enemy.lifted_voice, "");
}

#[test]
fn going_idle_in_range_may_say_a_line() {
    let (mut sim, id) = with_claw(3, 600, 100);
    // In mode 6 only the shared draw; otherwise one more (and one of 0..3
    // below 60).
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| {
        if let Some(d) = enemy::data_mut(w, id) {
            d.mode = 6;
        }
        bear_sailor::go_idle(w, id);
    });
    let mut expect = before.clone();
    expect.below(100);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    let before = expect;
    sim.with_world(|w| {
        if let Some(d) = enemy::data_mut(w, id) {
            d.mode = 3;
        }
        bear_sailor::go_idle(w, id);
    });
    let mut expect = before.clone();
    if expect.below(100) < 60 {
        expect.below(3);
    }
    expect.below(100);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

/// The bear walking (`fast` false) or running at Claw, 500 px to its left,
/// standing on its left limit's whole pixel with `fraction` of a pixel
/// past it; its state after one tick of the advance rule.
fn advance_at_the_limit(fast: bool, fraction: f64) -> Option<EnemyState> {
    let (mut sim, id) = with_claw(4, -500, 10_000);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = LIMITS.0;
        let (x, y) = (o.x, o.y);
        o.state = if fast {
            EnemyState::FastAdvance
        } else {
            EnemyState::Advance
        } as i32;
        let d = o.data.enemy_mut().unwrap();
        d.carry.0 = fraction;
        d.moved_to = Some((x, y));
        d.claw_right = false;
        d.mode = 1;
        assert_eq!(enemy::exact_x(w, id), f64::from(LIMITS.0) + fraction);
        if fast {
            enemy::redtail_pirate::fast_advance_tick_at(w, id, bear_sailor::SPEEDS);
        } else {
            enemy::redtail_pirate::advance_tick_at(w, id, bear_sailor::SPEEDS);
        }
    });
    state_of(&sim, id)
}

#[test]
fn walking_or_running_at_claw_it_goes_idle_only_exactly_on_its_limit() {
    for fast in [false, true] {
        // Its whole pixel on the limit with 0.76 px left over: it carries
        // on towards Claw.
        let going = advance_at_the_limit(fast, 0.76);
        assert!(
            matches!(going, Some(EnemyState::Advance | EnemyState::FastAdvance)),
            "fast {fast}: {going:?}"
        );
        // Exactly on it: it goes idle.
        let idle = advance_at_the_limit(fast, 0.0);
        assert_eq!(idle, Some(EnemyState::Idle), "fast {fast}");
    }
}
