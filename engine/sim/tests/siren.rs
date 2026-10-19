//! The Siren and the lockout of her scream on a synthetic floor with the
//! eleventh level's animation timings: how she is built (home, going by
//! sight), her song (the call and the first scream on the song's 11th
//! tick, four screams 17 ticks apart, home on the 106th, the cooldown
//! before the next), the scream (3.6 px a tick, gone after 85 ticks, no
//! damage), and the lockout through the input path: set (not added) by
//! each scream, 111 locked ticks after the last, the keys still held
//! counting as pressed on the next, the latch emptied, the dazed picture,
//! frozen while no tick runs (pause, help, menu) and kept through a
//! death and the respawn, and Claw still hurt while locked.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, Timer, field, siren};
use claw_sim::logic::projectile::{self, ProjectileSpec, scream};
use claw_sim::object::{LogicKind, ObjectId, types};
use claw_sim::player::state;
use claw_sim::{Buttons, GameLogic, Sim, SimConfig, SimLevel};
use common::*;

/// An animation of (image, duration) elements; `once` makes the last
/// element finish it.
fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

/// Her animations and her scream's, with the eleventh level's timings.
fn siren_anims() -> Vec<Arc<Animation>> {
    let a = siren::CLASS.anims;
    let flight: Vec<(i32, i32)> = (2..=8).map(|i| (i, 200)).chain([(8, 0)]).collect();
    vec![
        anim(
            siren::SONG_ANIM,
            &[(201, 180), (202, 100), (203, 1000)],
            true,
        ),
        anim(
            siren::IDLE_ANIM,
            &[(103, 80), (101, 100), (102, 250)],
            false,
        ),
        anim(
            a.hit_low,
            &[(600, 50), (601, 200), (600, 50), (200, 0)],
            true,
        ),
        anim(
            a.hit_high,
            &[(500, 50), (501, 200), (500, 50), (200, 0)],
            true,
        ),
        anim(a.advance, &[(1, 170), (2, 170)], false),
        anim(a.fast_advance, &[(1, 100), (2, 100)], false),
        anim(scream::ANIM, &flight, true),
    ]
}

/// Where the arena's Siren stands and her patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2950, 3060);
/// Claw's distance to her left: 139 px (mode 3), as on the eleventh
/// level.
const LEFT: i32 = 139;

/// The flat floor (feet rest on 959) as the eleventh level, with a Siren
/// at `AT` between `LIMITS`.
fn arena() -> SimLevel {
    let mut level = flat_level();
    level.number = 11;
    for a in siren_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at(siren::NAMES[0], AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    level.objects = vec![o];
    level
}

/// The arena with Claw put `LEFT` px left of the Siren on the first tick;
/// every object awake.
fn with_claw() -> (Sim, ObjectId) {
    let level = arena();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.unlatch_keys();
    sim.teleport_player(AT - LEFT, 899);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Siren))
        .map(|(id, _)| id)
        .expect("a Siren");
    (sim, id)
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

/// Her wait at home is over now.
fn wait_over(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        if let Some(s) = siren::siren_mut(w, id) {
            s.wait = Timer::default();
        }
    });
}

fn screams(sim: &Sim) -> Vec<(ObjectId, i32, i32)> {
    sim.objects()
        .all()
        .filter(|(_, o)| scream::is_scream(o))
        .map(|(id, o)| (id, o.x, o.y))
        .collect()
}

/// A flat level with Claw standing on it, keys counting from the first
/// tick, settled.
fn standing() -> Sim {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    sim
}

/// The lockout as a scream on the last tick leaves it.
fn screamed(sim: &mut Sim) {
    sim.with_world(|w| w.set_scream_lock_ms(scream::LOCK_MS));
}

#[test]
fn she_is_built_at_home_going_by_sight() {
    let (sim, id) = with_claw();
    let o = sim.objects().get(id).unwrap();
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(o.health, siren::HEALTH);
    assert_eq!(o.damage, siren::DAMAGE);
    assert_eq!(o.hit_rect, siren::HIT_RECT);
    let d = enemy::data_of(o).unwrap();
    assert!(d.sight && d.may_lift);
    assert_eq!(d.ranges, siren::RANGES);
    // Dropped to the floor: her feet, 44 px below her, on it.
    assert_eq!(o.y + siren::MOVE_RECT.bottom, 959);
    let s = siren::siren_data_of(o).unwrap();
    assert!(s.wait.len >= 1000 && s.wait.len <= 2000, "{:?}", s.wait);
}

#[test]
fn her_song_screams_four_times_and_goes_home() {
    let (mut sim, id) = with_claw();
    wait_over(&mut sim, id);
    let mut states = Vec::new();
    let mut seen: Vec<(usize, ObjectId)> = Vec::new();
    let mut calls = Vec::new();
    for k in 0..110 {
        sim.step(TickInput::default());
        states.push(state_of(&sim, id));
        for (s, _, _) in screams(&sim) {
            if !seen.iter().any(|&(_, o)| o == s) {
                seen.push((k, s));
            }
        }
        if sim
            .last_audio()
            .iter()
            .any(|a| matches!(a, claw_frame::AudioCue::Play(p) if p.key == siren::CALL_SOUND))
        {
            calls.push(k);
        }
    }
    // She decides on the first tick (S) and sings from it.
    let s = 0;
    assert_eq!(states[s], Some(EnemyState::Singing));
    let tick = |n: u32| s + n as usize;
    // Her call and the first scream on the song's 11th tick, three more 17
    // ticks apart.
    assert_eq!(calls, [tick(siren::CALL_TICK)]);
    let made: Vec<usize> = seen.iter().map(|&(k, _)| k).collect();
    assert_eq!(
        made,
        (0..siren::SCREAMS)
            .map(|i| tick(siren::CALL_TICK + i * siren::SCREAM_GAP_TICKS))
            .collect::<Vec<_>>()
    );
    // Home on the song's 106th tick; her next decision while the
    // cooldown runs sends her home again, with no song.
    assert_eq!(
        states[tick(siren::HOME_TICK) - 1],
        Some(EnemyState::Singing)
    );
    assert_eq!(states[tick(siren::HOME_TICK)], Some(EnemyState::Home));
    wait_over(&mut sim, id);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    // The cooldown's 4000 ms from the call are over on the song's 234th
    // tick: a decision then sings again.
    while sim.tick() < 1 + tick(siren::CALL_TICK + 222) as u64 {
        sim.step(TickInput::default());
    }
    wait_over(&mut sim, id);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home), "tick 233");
    wait_over(&mut sim, id);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Singing), "tick 234");
}

#[test]
fn a_scream_drifts_and_fades() {
    let (mut sim, _) = with_claw();
    // Far from Claw, flying left.
    let spec = ProjectileSpec {
        logic: scream::NAMES[0],
        x: 6000,
        y: 900,
        vx: -scream::SPEED,
        type_flags: types::ENEMY_SHOT,
        image_set: scream::IMAGE_SET,
        ..ProjectileSpec::default()
    };
    let id = sim
        .with_world(|w| projectile::spawn(w, spec))
        .expect("a scream");
    {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.damage, 0);
        assert_eq!(o.attack_rect, scream::FRAME_BOX.mirrored());
    }
    let mut last = None;
    for k in 1..=90u32 {
        sim.step(TickInput::default());
        match sim.objects().get(id) {
            Some(o) => last = Some((k, o.x)),
            None => break,
        }
    }
    // Gone on its 85th call, 3.6 px a call, about 300 px from where it
    // left.
    let (k, x) = last.unwrap();
    assert_eq!(k, scream::LIFE_TICKS - 1);
    assert!((6000 - x - 302).abs() <= 2, "{x}");
}

#[test]
fn a_scream_on_claw_locks_him_and_does_no_harm() {
    let (mut sim, id) = with_claw();
    wait_over(&mut sim, id);
    let mut hits = Vec::new();
    for k in 0..240 {
        let before = sim.scream_lock_ms();
        sim.step(TickInput::default());
        if sim.scream_lock_ms() == scream::LOCK_MS && before != scream::LOCK_MS {
            hits.push(k);
        }
        if sim.scream_lock_ms() == scream::LOCK_MS {
            // Set, not added; no damage, no hit reaction, no window.
            assert_eq!(sim.stats().health, 100);
            assert_ne!(sim.player().unwrap().state, state::HIT);
            assert_eq!(sim.claw().stats.invulnerable_ms, 0);
            assert!(
                sim.last_audio().iter().any(|a| matches!(a, claw_frame::AudioCue::Play(p) if p.key == claw_sim::player::hit::SHOT_SOUND))
            );
        }
    }
    // Each of the four screams reaches him, 17 ticks apart, each setting
    // the lockout to 2000 again, and is gone.
    assert_eq!(hits.len(), 4, "{hits:?}");
    assert!(hits.windows(2).all(|p| p[1] - p[0] == 17), "{hits:?}");
    assert!(screams(&sim).is_empty());
}

#[test]
fn the_lockout_takes_the_keys_for_111_ticks() {
    let mut sim = standing();
    screamed(&mut sim);
    let x0 = sim.player().unwrap().x;
    for k in 1..=scream::LOCK_TICKS {
        sim.step(hold(RIGHT));
        assert!(sim.scream_lock_ms() > 0, "tick {k}");
        assert_eq!(sim.claw_input(), TickInput::default(), "tick {k}");
        assert_eq!(sim.player().unwrap().x, x0, "tick {k}");
        assert_eq!(sim.player().unwrap().frame, scream::DAZED_FRAME);
    }
    assert_eq!(sim.scream_lock_ms(), 2);
    // The 112th: polled, the held key pressed, and he walks.
    sim.step(hold(RIGHT));
    assert_eq!(sim.scream_lock_ms(), 0);
    assert_eq!(sim.claw_input().held, Buttons(RIGHT));
    assert_eq!(sim.claw_input().pressed, Buttons(RIGHT));
    assert!(sim.player().unwrap().x > x0);
    // The raw input is left as it was given.
    assert_eq!(sim.input(), hold(RIGHT));
}

#[test]
fn a_jump_held_through_the_lockout_jumps_when_it_ends() {
    let mut sim = standing();
    screamed(&mut sim);
    sim.step(press(JUMP));
    for _ in 2..=scream::LOCK_TICKS {
        sim.step(hold(JUMP));
        assert_eq!(sim.player().unwrap().state, state::STAND);
    }
    sim.step(hold(JUMP));
    assert_eq!(sim.player().unwrap().state, state::JUMP);
}

#[test]
fn a_locked_tick_empties_the_latch() {
    let mut sim = standing();
    screamed(&mut sim);
    for _ in 0..50 {
        sim.step(hold(JUMP));
    }
    // A screen returns: every key down is latched again; the next locked
    // tick empties the latch, so the held jump still fires at the end.
    sim.latch_keys();
    sim.step(hold(JUMP));
    assert_eq!(sim.latched(), Buttons::NONE);
    while sim.scream_lock_ms() > 0 {
        sim.step(hold(JUMP));
    }
    assert_eq!(sim.player().unwrap().state, state::JUMP);
}

#[test]
fn the_lockout_stands_still_while_no_tick_runs() {
    let mut sim = standing();
    screamed(&mut sim);
    idle(&mut sim, 10);
    assert_eq!(sim.scream_lock_ms(), 2000 - 10 * 18);
    // The pause, the help screen and the menu run no tick; the help
    // screen's time reaches the fuses only; play resumes with a whole
    // tick.
    sim.restart_idle_wait();
    sim.pass_menu_time(400);
    sim.latch_keys();
    assert_eq!(sim.scream_lock_ms(), 2000 - 10 * 18);
    idle(&mut sim, 1);
    assert_eq!(sim.scream_lock_ms(), 2000 - 11 * 18);
}

#[test]
fn a_death_keeps_the_lockout_running() {
    let mut sim = standing();
    screamed(&mut sim);
    idle(&mut sim, 20);
    let lives = sim.stats().lives;
    sim.end_life();
    let mut ticks = 20;
    while sim.stats().lives == lives || sim.stats().health == 0 {
        sim.step(TickInput::default());
        ticks += 1;
        assert!(ticks < 400, "no respawn");
    }
    // Back with a life less: the lockout went on 18 ms a tick through the
    // death, the respawn's passes taking nothing.
    let left = 2000u32.saturating_sub(18 * ticks);
    assert_eq!(sim.scream_lock_ms(), left);
    if left > 0 {
        assert_eq!(sim.claw_input(), TickInput::default());
    }
}

#[test]
fn claw_is_hurt_while_locked() {
    let mut sim = standing();
    screamed(&mut sim);
    let (x, y) = {
        let c = sim.player().unwrap();
        (c.x, c.y)
    };
    sim.with_world(|w| {
        projectile::spawn(
            w,
            ProjectileSpec {
                logic: "Bullet",
                x: x + 40,
                y: y - 20,
                vx: -500,
                damage: 10,
                type_flags: types::ENEMY_SHOT,
                ..ProjectileSpec::default()
            },
        )
    });
    idle(&mut sim, 10);
    assert_eq!(sim.stats().health, 90);
    assert!(sim.scream_lock_ms() > 0);
}

#[test]
fn a_crouching_claw_is_screamed_at_too() {
    let (mut sim, id) = with_claw();
    wait_over(&mut sim, id);
    let mut first_hit = None;
    for k in 0..120 {
        sim.step(hold(DOWN));
        if first_hit.is_none() && sim.scream_lock_ms() > 0 {
            first_hit = Some(k);
            assert_eq!(sim.player().unwrap().state, state::CROUCH);
        }
    }
    assert!(first_hit.is_some(), "the scream reaches a crouching Claw");
    // The crouch rises while locked.
    assert_eq!(sim.player().unwrap().state, state::STAND);
}
