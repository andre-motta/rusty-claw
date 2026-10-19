//! Aquatis's tentacles on a synthetic floor with the twelfth level's
//! animation timings: how a tentacle is built (rooted, its box, no
//! contact box, its reset bit in build order), that it stands still while
//! Aquatis is not current, its attack after its idle time with Claw in
//! reach on its left, the strike (its box on the strike's picture only,
//! idle again on call 20), the grab (Claw held where he stands, placed
//! after 18 calls), the squeeze (a health point every 200 ms from its
//! first call, 14 in 3000 ms, Claw's crushed pictures), the escape on the
//! tenth call with a press, the squeeze that kills, a second tentacle
//! grabbing a held Claw and a third one's strike ending both holds, the
//! thrashing of Aquatis's frenzy (its delay, the swipes facing each way in
//! turn, the box facing right that reaches nobody on its row, nothing
//! hurts it, a reset facing right that leaves it no box), the attack choice that replaces a frenzy started in the same
//! call, death by any hit, the hidden splash, regrowth after 20 s or at
//! once in a frenzy and never after Aquatis's death, the reset Aquatis
//! asks for, and its depth against Claw.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimPlayer, Animation};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::ObjectData;
use claw_sim::logic::enemy::motion::{DEPTH_BEHIND, DEPTH_IN_FRONT};
use claw_sim::logic::enemy::{EnemyState, aquatis, tentacle};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, special, types};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

const ATTACK: u32 = 0x2;

/// The idle sway, looping.
const IDLE: &[(i32, i32)] = &[(150, 150), (151, 150), (152, 150), (153, 150)];
/// The strike: picture 301 on calls 11 to 15, done on call 20.
const STRIKE1: &[(i32, i32)] = &[(300, 100), (400, 70), (301, 90), (401, 60), (150, 0)];
/// The grab: done on call 18.
const STRIKE2: &[(i32, i32)] = &[(300, 100), (400, 80), (301, 100), (301, 0)];
/// The squeeze, looping.
const STRIKE3: &[(i32, i32)] = &[(302, 150), (303, 150), (304, 300)];
/// Sinking into the water.
const KILLFALL: &[(i32, i32)] = &[(500, 200), (501, 200), (950, 200), (954, 200)];
/// Growing back.
const RESPAWN: &[(i32, i32)] = &[(954, 200), (953, 250), (952, 250), (152, 100)];

fn anims() -> Vec<Arc<Animation>> {
    let mut v = vec![
        Animation::looping(tentacle::IDLE_ANIM, IDLE),
        Animation::once(tentacle::STRIKE_ANIM, STRIKE1),
        Animation::once(tentacle::GRAB_ANIM, STRIKE2),
        Animation::looping(tentacle::SQUEEZE_ANIM, STRIKE3),
        Animation::once(tentacle::DEATH_ANIM, KILLFALL),
        Animation::once(tentacle::REGROW_ANIM, RESPAWN),
    ];
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(Animation::once(key, &[(200, 150), (201, 200)]));
    }
    v.push(Animation::looping("CLAW_STAND", &[(11, 110), (12, 110)]));
    v.push(Animation::looping("CLAW_WALK", &[(20, 100), (21, 100)]));
    v.push(Animation::looping("CLAW_FALL", &[(278, 65), (280, 80)]));
    v.push(Animation::once("GAME_SPLASH", &[(1, 500)]));
    v.into_iter().map(Arc::new).collect()
}

/// Where the first tentacle stands; the others stand [`GAP`] px apart to
/// its right.
const AT: (i32, i32) = (3000, 900);
const GAP: i32 = 100;
/// Where Aquatis's object is placed: far enough right that his body
/// never touches Claw where the tests put him.
const AQUATIS: (i32, i32) = (4000, 900);

/// The flat floor (feet rest on 959) as the twelfth level, with `n`
/// tentacles from `AT` and Aquatis's object; every object awake.
fn arena(n: usize) -> SimLevel {
    let mut level = flat_level();
    level.number = 12;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    level.objects = (0..n)
        .map(|i| ObjectSpec::at("Tentacle", AT.0 + GAP * i as i32, AT.1, 0))
        .collect();
    level
        .objects
        .push(ObjectSpec::at("Aquatis", AQUATIS.0, AQUATIS.1, 0));
    level
}

/// The arena with `n` tentacles and Claw put `dx` px from the first
/// tentacle on the first tick; Aquatis current and alive when `current`,
/// otherwise the link names no Aquatis.
fn arena_sim(n: usize, dx: i32, current: bool) -> (Sim, Vec<ObjectId>) {
    let level = arena(n);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(AT.0 + dx, 899);
    sim.step(TickInput::default());
    let mut ids: Vec<(ObjectId, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::Tentacle))
        .map(|(id, o)| (id, o.x))
        .collect();
    ids.sort_by_key(|&(_, x)| x);
    let boss = current.then(|| aquatis_object(&sim));
    sim.with_world(|w| {
        let link = w.aquatis_link_mut();
        link.aquatis = boss;
        link.alive = current;
    });
    (sim, ids.into_iter().map(|(id, _)| id).collect())
}

fn aquatis_object(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == aquatis::NAMES[0])
        .map(|(id, _)| id)
        .expect("Aquatis's object")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn tdata(sim: &Sim, id: ObjectId) -> tentacle::TentacleData {
    match &obj(sim, id).data {
        ObjectData::Tentacle(d) => d.clone(),
        d => panic!("tentacle data: {d:?}"),
    }
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .expect("Claw")
}

fn claw(sim: &Sim) -> &Object {
    sim.player().expect("Claw")
}

fn health(sim: &Sim) -> i32 {
    sim.claw().stats.health
}

/// A seed whose first draw of 0..=100 makes the attack choice a strike
/// (`strike`) or a grab.
fn seed_for(strike: bool) -> u32 {
    (1..)
        .find(|&s| (Rng::new(s).below(101) < 75) == strike)
        .expect("a seed")
}

/// Calls of the animation `key` of the arena until it ends, with the
/// picture after each call.
fn calls(key: &str) -> Vec<i32> {
    let level = arena(1);
    let mut p = AnimPlayer::default();
    p.set(level.animations[key].clone());
    let mut frames = Vec::new();
    let mut frame = 0;
    while !p.is_done() && frames.len() < 200 {
        let e = p.advance_in(18, frame, false, None);
        if let Some(f) = e.frame {
            frame = f;
        }
        frames.push(frame);
    }
    frames
}

#[test]
fn the_synthetic_animations_keep_the_twelfth_levels_timings() {
    let strike = calls(tentacle::STRIKE_ANIM);
    assert_eq!(strike.len(), 20);
    let live: Vec<usize> = (0..strike.len())
        .filter(|&i| strike[i] == tentacle::STRIKE_FRAME)
        .map(|i| i + 1)
        .collect();
    assert_eq!(live, [11, 12, 13, 14, 15]);
    assert_eq!(calls(tentacle::GRAB_ANIM).len(), 18);
}

#[test]
fn a_tentacle_is_built_rooted_with_its_box_and_its_reset_bit() {
    let (sim, ids) = arena_sim(3, -600, false);
    // One bit each, in the order they were built.
    let mut bits: Vec<u32> = ids.iter().map(|&id| tdata(&sim, id).bit).collect();
    bits.sort_unstable();
    assert_eq!(bits, aquatis::TENTACLE_BITS);
    for &id in &ids {
        let o = obj(&sim, id);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
        assert_eq!((o.move_rect, o.hit_rect), (tentacle::BOX, tentacle::BOX));
        assert!(!o.attack_rect.is_set(), "no contact box");
        assert_eq!((o.damage, o.health), (tentacle::DAMAGE, tentacle::HEALTH));
        assert_eq!(o.type_flags, types::ENEMY);
        assert_eq!(o.hit_type_flags, tentacle::HIT_MASK);
        assert_eq!(o.special_flags & special::LIFTABLE, 0);
        assert_eq!(o.y, AT.1, "never dropped to the floor");
        let d = tdata(&sim, id);
        assert_eq!(d.enemy.ranges, tentacle::RANGES);
        // Idle 1000 ms and up to 300 more.
        assert!((1000..=1300).contains(&d.idle.len), "{}", d.idle.len);
    }
}

#[test]
fn without_aquatis_it_stands_still() {
    let (mut sim, ids) = arena_sim(1, -100, false);
    let frame = obj(&sim, ids[0]).frame;
    for _ in 0..200 {
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Idle));
        assert_eq!(obj(&sim, ids[0]).frame, frame, "no sway");
    }
    assert_eq!(health(&sim), 100);
}

#[test]
fn it_attacks_after_its_idle_time_with_claw_in_reach_on_its_left() {
    for dx in [-100, -30] {
        let (mut sim, ids) = arena_sim(1, dx, true);
        let idle = tdata(&sim, ids[0]).idle.len as usize;
        let mut k = 0;
        while state_of(&sim, ids[0]) == Some(EnemyState::Idle) {
            sim.step(TickInput::default());
            k += 1;
            assert!(k < 200, "no attack");
        }
        // The idle time started with the tick that built it.
        assert_eq!(k, idle.div_ceil(18), "dx {dx}");
        assert!(matches!(
            state_of(&sim, ids[0]),
            Some(EnemyState::Strike | EnemyState::StartGrab)
        ));
    }
    // Right of it, or beyond its reach, it never attacks.
    for dx in [30, -tentacle::REACH - 10] {
        let (mut sim, ids) = arena_sim(1, dx, true);
        for _ in 0..300 {
            sim.step(TickInput::default());
            assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Idle), "dx {dx}");
        }
    }
}

#[test]
fn the_attack_choice_strikes_three_times_in_four() {
    for strike in [true, false] {
        let (mut sim, ids) = arena_sim(1, -100, true);
        let seed = seed_for(strike);
        sim.with_world(|w| {
            *w.rng = Rng::new(seed);
            tentacle::attack_choice(w, ids[0]);
        });
        let want = if strike {
            EnemyState::Strike
        } else {
            EnemyState::StartGrab
        };
        assert_eq!(state_of(&sim, ids[0]), Some(want));
    }
}

#[test]
fn the_strike_hurts_on_its_strike_picture_only() {
    let (mut sim, ids) = arena_sim(1, -100, true);
    let id = ids[0];
    sim.with_world(|w| tentacle::enter_strike(w, id));
    let mut live = Vec::new();
    let mut hurt = Vec::new();
    for call in 1..=20 {
        let before = health(&sim);
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        if o.attack_rect.is_set() {
            assert_eq!(o.attack_rect, tentacle::STRIKE_BOX);
            live.push(call);
        }
        if health(&sim) < before {
            hurt.push((call, before - health(&sim)));
        }
        if call < 20 {
            assert_eq!(state_of(&sim, id), Some(EnemyState::Strike), "call {call}");
        }
    }
    assert_eq!(live, [11, 12, 13, 14, 15]);
    assert_eq!(hurt, [(11, tentacle::DAMAGE)]);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

/// Starts the grab of tentacle `id` now.
fn grab(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| tentacle::start_grab(w, id));
}

#[test]
fn the_grab_lifts_claw_and_the_squeeze_takes_a_point_every_200_ms() {
    let (mut sim, ids) = arena_sim(1, -100, true);
    let id = ids[0];
    let at = (claw(&sim).x, claw(&sim).y);
    grab(&mut sim, id);
    assert_eq!(claw(&sim).state, state::BEARHUG, "held at once");
    sim.with_world(|w| assert!(w.tentacle_grab()));
    for call in 1..=17 {
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, id), Some(EnemyState::StartGrab), "{call}");
        assert_eq!((claw(&sim).x, claw(&sim).y), at, "frozen where he stood");
    }
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Squeeze));
    let c = claw(&sim);
    assert_eq!(
        (c.x, c.y),
        (AT.0 + tentacle::HOLD_AT.0, AT.1 + tentacle::HOLD_AT.1)
    );
    assert!(sim.claw().facing_right);
    // The squeeze: a point on its first call and every 12 calls after,
    // with Claw's crushed pictures; let go once 3000 ms have passed.
    let mut points = Vec::new();
    let mut call = 0;
    while state_of(&sim, id) == Some(EnemyState::Squeeze) {
        let before = health(&sim);
        sim.step(TickInput::default());
        call += 1;
        if health(&sim) < before {
            points.push(call);
        }
        if state_of(&sim, id) == Some(EnemyState::Squeeze) {
            assert!((450..=452).contains(&claw(&sim).frame), "crushed");
        }
        assert!(call < 400);
    }
    assert_eq!(points, (0..14).map(|i| 1 + 12 * i).collect::<Vec<_>>());
    assert_eq!(call, 167, "3000 ms at 18 ms");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_ne!(claw(&sim).state, state::BEARHUG);
    assert_eq!(health(&sim), 86);
    sim.with_world(|w| assert!(!w.tentacle_grab()));
}

/// Grabs with the first tentacle and plays the grab to the squeeze's
/// start.
fn squeezing(n: usize) -> (Sim, Vec<ObjectId>) {
    let (mut sim, ids) = arena_sim(n, -100, true);
    grab(&mut sim, ids[0]);
    for _ in 0..18 {
        sim.step(TickInput::default());
    }
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Squeeze));
    (sim, ids)
}

#[test]
fn the_tenth_call_with_a_press_frees_claw() {
    let (mut sim, ids) = squeezing(1);
    let mut presses = 0;
    for k in 1..=40 {
        let input = if k % 3 == 0 {
            presses += 1;
            press(ATTACK)
        } else {
            TickInput::default()
        };
        sim.step(input);
        if presses < 10 {
            assert_eq!(claw(&sim).state, state::BEARHUG, "press {presses}");
        } else {
            assert_ne!(claw(&sim).state, state::BEARHUG);
            assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Idle));
            assert_eq!(k, 30);
            return;
        }
    }
    panic!("never freed");
}

#[test]
fn a_squeeze_that_empties_his_health_kills_him() {
    let (mut sim, ids) = squeezing(1);
    sim.with_world(|w| w.claw.stats.health = 2);
    for _ in 0..13 {
        sim.step(TickInput::default());
    }
    assert_eq!(health(&sim), 0);
    assert_eq!(claw(&sim).state, state::DEATH_FALL);
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Idle));
    sim.with_world(|w| assert!(!w.tentacle_grab()));
}

#[test]
fn a_second_tentacle_grabs_a_held_claw_and_a_third_ones_strike_ends_both_holds() {
    let (mut sim, ids) = squeezing(3);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    // The second tentacle's attack choice while Claw is held: it goes
    // idle first (a draw) and grabs anyway.
    let seed = (1..)
        .find(|&s| {
            let mut r = Rng::new(s);
            r.below(301);
            r.below(101) >= 75
        })
        .unwrap();
    sim.with_world(|w| {
        *w.rng = Rng::new(seed);
        tentacle::attack_choice(w, b);
    });
    assert_eq!(state_of(&sim, b), Some(EnemyState::StartGrab));
    for _ in 0..18 {
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, a), Some(EnemyState::Squeeze));
    }
    // Both hold him, where the second one put him.
    assert_eq!(state_of(&sim, b), Some(EnemyState::Squeeze));
    assert_eq!(
        (claw(&sim).x, claw(&sim).y),
        (AT.0 + GAP + tentacle::HOLD_AT.0, AT.1 + tentacle::HOLD_AT.1)
    );
    // The third one's strike reaches him: both let go on their next call.
    sim.with_world(|w| tentacle::enter_strike(w, c));
    let mut k = 0;
    while claw(&sim).state == state::BEARHUG {
        sim.step(TickInput::default());
        k += 1;
        assert!(k <= 20, "never struck");
    }
    assert_eq!(claw(&sim).state, state::HIT);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, a), Some(EnemyState::Idle));
    assert_eq!(state_of(&sim, b), Some(EnemyState::Idle));
    sim.with_world(|w| assert!(!w.tentacle_grab()));
}

#[test]
fn in_aquatis_frenzy_it_thrashes_both_ways_and_nothing_hurts_it() {
    // Claw far right of it, standing on its row.
    let (mut sim, ids) = arena_sim(1, 600, true);
    let id = ids[0];
    sim.with_world(|w| w.aquatis_link_mut().frenzy = true);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::SuperFrenzy));
    let delay = tdata(&sim, id).frenzy_delay.len;
    assert!(delay <= 200);
    // Nothing hurts it now.
    let claw_id = claw_id(&sim);
    sim.with_world(|w| tentacle::on_hit(w, id, claw_id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::SuperFrenzy));
    let mut facing = Vec::new();
    let mut hurt_at = None;
    for k in 0..120 {
        let before = health(&sim);
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        let right = o.draw_flags & claw_sim::object::draw_flags::MIRROR != 0;
        facing.push(right);
        if right && o.frame != tentacle::STRIKE_FRAME && o.attack_rect.is_set() {
            assert_eq!(o.attack_rect, tentacle::FRENZY_RIGHT_BOX);
        }
        if !right && o.frame != tentacle::STRIKE_FRAME {
            assert!(!o.attack_rect.is_set(), "call {k}");
        }
        if health(&sim) < before && hurt_at.is_none() {
            hurt_at = Some(k);
        }
    }
    // Facing left for the first swipe, then right, then left again: each
    // swipe 20 calls.
    let turns: Vec<usize> = (1..facing.len())
        .filter(|&k| facing[k] != facing[k - 1])
        .collect();
    assert!(turns.len() >= 4, "{turns:?}");
    assert!(turns.windows(2).all(|t| t[1] - t[0] == 20), "{turns:?}");
    // Claw, 600 px right of it on its row, is never hurt: facing right,
    // its box away from the strike's picture reaches nobody.
    assert_eq!(hurt_at, None);
    // The frenzy over, it goes idle with no attack box.
    sim.with_world(|w| w.aquatis_link_mut().frenzy = false);
    let mut k = 0;
    while state_of(&sim, id) == Some(EnemyState::SuperFrenzy) {
        sim.step(TickInput::default());
        k += 1;
        assert!(k < 3);
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert!(!obj(&sim, id).attack_rect.is_set());
    assert_eq!(
        obj(&sim, id).draw_flags & claw_sim::object::draw_flags::MIRROR,
        0
    );
}

#[test]
fn a_reset_while_it_thrashes_facing_right_leaves_it_idle_and_harmless() {
    // Claw 600 px left of it on its row: beyond its strike and its reach.
    let (mut sim, ids) = arena_sim(1, -600, true);
    let id = ids[0];
    sim.with_world(|w| w.aquatis_link_mut().frenzy = true);
    let mirrored = |sim: &Sim| obj(sim, id).draw_flags & claw_sim::object::draw_flags::MIRROR != 0;
    let mut k = 0;
    while !(mirrored(&sim)
        && obj(&sim, id).frame != tentacle::STRIKE_FRAME
        && obj(&sim, id).attack_rect.is_set())
    {
        sim.step(TickInput::default());
        k += 1;
        assert!(k < 200, "never thrashed facing right");
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::SuperFrenzy));
    assert_eq!(obj(&sim, id).attack_rect, tentacle::FRENZY_RIGHT_BOX);
    // Aquatis asks for its reset as his frenzy ends (Claw came back).
    let bit = tdata(&sim, id).bit;
    sim.with_world(|w| {
        let link = w.aquatis_link_mut();
        link.frenzy = false;
        link.reset_bits |= bit;
    });
    let full = health(&sim);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert!(!mirrored(&sim), "facing left");
    assert!(!obj(&sim, id).attack_rect.is_set(), "no box turned left");
    // Idle until it may attack, it never reaches him.
    for _ in 0..50 {
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
        assert!(!obj(&sim, id).attack_rect.is_set());
    }
    assert_eq!(health(&sim), full, "hurt from far left of it");
}

#[test]
fn the_attack_choice_replaces_a_frenzy_started_in_the_same_call() {
    // Claw out of reach until its idle time is over, then in reach as the
    // frenzy starts.
    let (mut sim, ids) = arena_sim(1, -600, true);
    let id = ids[0];
    for _ in 0..80 {
        sim.step(TickInput::default());
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    sim.teleport_player(AT.0 - 100, 899);
    sim.step(TickInput::default());
    sim.with_world(|w| w.aquatis_link_mut().frenzy = true);
    sim.step(TickInput::default());
    assert!(
        matches!(
            state_of(&sim, id),
            Some(EnemyState::Strike | EnemyState::StartGrab)
        ),
        "{:?}",
        state_of(&sim, id)
    );
}

#[test]
fn any_hit_kills_it_and_it_grows_back_after_20_s() {
    let (mut sim, ids) = arena_sim(1, -600, true);
    let id = ids[0];
    let claw_id = claw_id(&sim);
    sim.with_world(|w| tentacle::on_hit(w, id, claw_id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    let o = obj(&sim, id);
    assert_ne!(o.flags & flags::HARMLESS, 0);
    assert!(!o.attack_rect.is_set());
    // One hidden splash, at the height of its box's bottom edge.
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let splashes: Vec<(i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "Splash")
        .map(|(_, o)| (o.x, o.y))
        .collect();
    assert_eq!(splashes, [(AT.0, tentacle::BOX.bottom)]);
    // It grows back once 20000 ms have passed.
    let mut k = 3;
    while state_of(&sim, id) == Some(EnemyState::Dead) {
        sim.step(TickInput::default());
        k += 1;
        assert!(k < 2000);
    }
    // The hit, made between two ticks, is timed from the game time the
    // next tick reads: 1112 ticks of 18 ms after that one.
    assert_eq!(k, 20000usize.div_ceil(18) + 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Respawn));
    assert_eq!(obj(&sim, id).flags & flags::HARMLESS, 0);
    while state_of(&sim, id) == Some(EnemyState::Respawn) {
        sim.step(TickInput::default());
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn it_grows_back_at_once_in_a_frenzy_and_never_after_aquatis_death() {
    let (mut sim, ids) = arena_sim(2, -600, true);
    let claw_id = claw_id(&sim);
    sim.with_world(|w| tentacle::on_hit(w, ids[0], claw_id));
    sim.step(TickInput::default());
    sim.with_world(|w| w.aquatis_link_mut().frenzy = true);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Respawn));
    // Aquatis dead: the idle one dies on its next call, the dead one stays
    // dead.
    let (mut sim, ids) = arena_sim(2, -600, true);
    sim.with_world(|w| tentacle::on_hit(w, ids[0], claw_id));
    sim.with_world(|w| w.aquatis_link_mut().alive = false);
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, ids[1]), Some(EnemyState::Dead));
    for _ in 0..1200 {
        sim.step(TickInput::default());
    }
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Dead));
    assert_eq!(state_of(&sim, ids[1]), Some(EnemyState::Dead));
}

#[test]
fn a_hit_is_ignored_while_it_grabs_or_squeezes_and_once_per_attack() {
    let (mut sim, ids) = arena_sim(2, -100, true);
    let claw_id = claw_id(&sim);
    grab(&mut sim, ids[0]);
    sim.with_world(|w| tentacle::on_hit(w, ids[0], claw_id));
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::StartGrab));
    for _ in 0..18 {
        sim.step(TickInput::default());
    }
    sim.with_world(|w| tentacle::on_hit(w, ids[0], claw_id));
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Squeeze));
    // The attack that hit the second one last does not hit it again.
    sim.with_world(|w| {
        tentacle::on_hit(w, ids[1], claw_id);
        tentacle::regrow(w, ids[1]);
        tentacle::on_hit(w, ids[1], claw_id);
    });
    assert_eq!(state_of(&sim, ids[1]), Some(EnemyState::Respawn));
}

#[test]
fn aquatis_reset_lets_claw_go_and_clears_its_bit() {
    let (mut sim, ids) = squeezing(1);
    sim.with_world(|w| w.reset_aquatis());
    let draws = |sim: &Sim| sim.rng_state();
    let before = draws(&sim);
    sim.step(TickInput::default());
    assert_ne!(claw(&sim).state, state::BEARHUG);
    assert_eq!(state_of(&sim, ids[0]), Some(EnemyState::Idle));
    sim.with_world(|w| {
        assert_eq!(w.aquatis_reset_bits() & aquatis::TENTACLE_BITS[0], 0);
        assert!(!w.tentacle_grab());
    });
    assert_ne!(draws(&sim), before, "going idle draws");
    // Without Aquatis current the bit is left for later.
    let (mut sim, _) = arena_sim(1, -600, false);
    sim.with_world(|w| w.reset_aquatis());
    sim.step(TickInput::default());
    assert_eq!(sim.aquatis_link().reset_bits, aquatis::RESET_BITS);
}

#[test]
fn it_runs_before_claw_while_his_feet_are_below_its_own() {
    // Its feet at 1010, below Claw's (959): in front of him.
    let (mut sim, ids) = arena_sim(1, -600, true);
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, ids[0]).z, DEPTH_IN_FRONT);
    // Raised 100 px, its feet are above his: behind him.
    let id = ids[0];
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.y -= 100;
        }
    });
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).z, DEPTH_BEHIND);
    assert_eq!(obj(&sim, id).move_rect, tentacle::BOX);
}
