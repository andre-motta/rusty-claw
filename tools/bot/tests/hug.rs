//! The search's mash in a bear sailor's hug, on a synthetic floor with a
//! bear: held, Claw is offered the mash, and mashing frees him long
//! before the hug runs out, for a few health points instead of the whole
//! hug's.

use std::sync::Arc;

use claw_bot::actions::{self, Action};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::EnemyState;
use claw_sim::logic::enemy::bear_sailor::{self, GRAB_ANIM, SQUEEZE_ANIM, STRIKE_ANIM};
use claw_sim::logic::enemy::field;
use claw_sim::object::{LogicKind, ObjectId};
use claw_sim::{Buttons, GameLogic, Sim, SimConfig, SimLevel, TickInput, TileMap};

/// An animation of (image, duration) elements; `once` makes the last
/// element finish it. The blow's third picture is live.
fn anim(key: &str, frames: &[(i32, i32)], once: bool, live: Option<usize>) -> Arc<Animation> {
    let mut a = if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    };
    if let Some(i) = live {
        a.frames[i].event = 1;
    }
    Arc::new(a)
}

/// Where the bear stands.
const AT: i32 = 3000;

/// A flat floor (feet on 959) as the seventh level, with a bear at `AT`
/// patrolling 300 px either way, and the bear's and Claw's animations
/// with the seventh level's timings.
fn arena() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    level.number = 7;
    let a = bear_sailor::CLASS.anims;
    let flinch = [(400, 50), (401, 200), (400, 50), (400, 0)];
    let anims = vec![
        anim(
            STRIKE_ANIM,
            &[
                (201, 90),
                (202, 180),
                (203, 100),
                (204, 120),
                (205, 100),
                (205, 0),
            ],
            true,
            Some(2),
        ),
        anim(
            GRAB_ANIM,
            &[(300, 90), (301, 120), (302, 100), (302, 0)],
            true,
            None,
        ),
        anim(
            SQUEEZE_ANIM,
            &[(304, 140), (303, 120), (302, 150), (303, 110)],
            false,
            None,
        ),
        anim(a.hit_low, &flinch, true, None),
        anim(a.hit_high, &flinch, true, None),
        anim(a.jump, &flinch, true, None),
        anim(a.home, &[(1, 0)], true, None),
        anim(a.bounce, &[(1, 0)], true, None),
        anim(a.advance, &[(10, 125), (11, 125)], false, None),
        anim(a.fast_advance, &[(12, 100), (13, 100)], false, None),
        anim(a.fall, &[(900, 100)], false, None),
        anim(
            bear_sailor::IDLE_ANIM,
            &[(100, 500), (101, 500)],
            false,
            None,
        ),
        anim("CLAW_STAND", &[(11, 110), (12, 110)], false, None),
        anim("CLAW_WALK", &[(20, 100), (21, 100)], false, None),
        anim("CLAW_FALL", &[(278, 65), (280, 80)], false, None),
    ];
    for an in anims {
        level.animations.insert(an.key.clone(), an);
    }
    let mut o = ObjectSpec::at("BearSailor", AT, 900, 0);
    o.record[field::X_MIN] = AT - 300;
    o.record[field::X_MAX] = AT + 300;
    level.objects = vec![o];
    level
}

/// The arena with Claw 40 px from the bear, after its first tick.
fn start() -> (Sim, ObjectId) {
    let level = arena();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 1,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(AT + 40, 899);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::BearSailor))
        .map(|(id, _)| id)
        .expect("a bear");
    (sim, id)
}

fn hugging(sim: &Sim, id: ObjectId) -> bool {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
        .is_some_and(|s| matches!(s, EnemyState::StartHug | EnemyState::Hug))
}

/// Plays `mask` for a tick, with the presses it makes.
fn step(sim: &mut Sim, prev: &mut u32, mask: u32) {
    sim.step(TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask & !*prev),
    });
    *prev = mask;
}

/// From the grab on, plays `action` (or nothing, without one) until the
/// bear lets go; returns the ticks held and the health lost.
fn hold(action: Option<Action>) -> (usize, i32) {
    let (mut sim, id) = start();
    let mut prev = 0;
    for _ in 0..200 {
        if actions::held(&sim) {
            break;
        }
        step(&mut sim, &mut prev, 0);
    }
    assert!(actions::held(&sim), "the bear grabs Claw");
    assert!(hugging(&sim, id));
    let health = sim.stats().health;
    let mut ticks = 0;
    'held: while ticks < 400 {
        let masks = action.map_or_else(|| vec![0], |a| a.masks());
        for m in masks {
            if !hugging(&sim, id) {
                break 'held;
            }
            step(&mut sim, &mut prev, m);
            ticks += 1;
        }
    }
    assert!(!actions::held(&sim), "the bear lets go");
    (ticks, health - sim.stats().health)
}

#[test]
fn held_in_a_hug_claw_is_offered_the_mash() {
    let (mut sim, _) = start();
    assert!(!actions::choices(&sim).contains(&Action::Mash));
    for _ in 0..200 {
        if actions::held(&sim) {
            break;
        }
        sim.step(TickInput::default());
    }
    assert!(actions::held(&sim));
    assert_eq!(actions::phase(&sim), actions::Phase::Busy);
    assert!(actions::choices(&sim).contains(&Action::Mash));
}

#[test]
fn mashing_frees_claw_long_before_the_hug_runs_out() {
    let (waited, lost_waiting) = hold(None);
    let (mashed, lost_mashing) = hold(Some(Action::Mash));
    assert!(mashed * 3 < waited, "mashed {mashed}, waited {waited}");
    assert!(
        lost_mashing * 3 < lost_waiting,
        "lost {lost_mashing} mashing, {lost_waiting} waiting"
    );
}

#[test]
fn an_enemy_worn_down_is_another_state() {
    // The same game but for the bear's health: a blow or a shot that hurts
    // it without killing it is progress for the search.
    let (sim, id) = start();
    let mut hurt = sim.clone();
    hurt.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health -= 5;
        }
    });
    assert!(hurt.objects().get(id).is_some_and(|o| o.health > 0));
    assert_ne!(
        claw_bot::search::state_key(&sim),
        claw_bot::search::state_key(&hurt)
    );
}
