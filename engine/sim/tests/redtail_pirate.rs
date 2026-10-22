//! The red-tail pirate on a synthetic floor with the seventh level's
//! animation timings: how it is built, its decisions (it engages on its
//! first decision step, never pauses in front of Claw, and rests 2000 ms
//! after a strike from further out), its strikes high and low (live on
//! their 7th tick, Home on the 18th or 14th), the hop back from any attack
//! of Claw's in front of it (on the attack's first tick, a chance of 90,
//! 60, 30, then 0, back to 90 once Claw is out of range), its flinch and
//! death on the second blow, the parry and the shot block it makes only on
//! the eighth and thirteenth levels (on the eighth a roll below 25 of
//! 0..101), the blasts it never parries, its lifted lines and its taunt
//! after a throw.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::redtail_pirate::{
    self, HIGH_STRIKE, HIGH_STRIKE_ANIM, LOW_STRIKE, LOW_STRIKE_ANIM, RedTailPirateData,
};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::kind as attack_kind;
use claw_sim::rng::Rng;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

const ATTACK: u32 = 0x2;
const LIFT: u32 = 0x20;

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

/// The high strike: live on its third and fourth pictures (calls 7 to
/// 12), done on call 18.
const STRIKE1: &[(i32, i32, i32)] = &[
    (200, 50, 0),
    (201, 50, 0),
    (202, 50, 1),
    (203, 80, 1),
    (204, 50, 0),
    (200, 0, 0),
];
/// The low strike: live on its third picture (calls 7 to 10), done on
/// call 14.
const STRIKE2: &[(i32, i32, i32)] = &[
    (210, 50, 0),
    (211, 50, 0),
    (212, 70, 1),
    (213, 50, 0),
    (210, 0, 0),
];
/// A flinch: 19 calls.
const HIT: &[(i32, i32, i32)] = &[(400, 50, 0), (401, 200, 0), (400, 50, 0), (400, 0, 0)];
/// The hop: 10 calls, then its last picture held.
const JUMP: &[(i32, i32, i32)] = &[(500, 70, 0), (501, 90, 0), (501, 0, 0)];
/// A swing of Claw: wind-up, the active frame (event 1), follow-through.
const SWING: &[(i32, i32, i32)] = &[(24, 50, 0), (25, 50, 0), (27, 100, 1), (24, 50, 0)];

fn pirate_anims() -> Vec<Arc<Animation>> {
    let a = redtail_pirate::CLASS.anims;
    vec![
        anim(HIGH_STRIKE_ANIM, STRIKE1, true),
        anim(LOW_STRIKE_ANIM, STRIKE2, true),
        anim(a.hit_low, HIT, true),
        anim(a.hit_high, HIT, true),
        anim(a.jump, JUMP, true),
        anim(a.home, &[(1, 0, 0)], true),
        anim(a.bounce, &[(1, 0, 0)], true),
        anim(a.lifted, &[(600, 50, 0), (601, 60, 0), (601, 0, 0)], true),
        anim(a.lifted2, &[(602, 100, 0), (603, 100, 0)], false),
        anim(a.advance, &[(10, 100, 0), (11, 100, 0)], false),
        anim(a.fast_advance, &[(12, 90, 0), (13, 90, 0)], false),
        anim(a.fall, &[(900, 100, 0)], false),
        anim(
            redtail_pirate::IDLE_ANIM,
            &[(100, 400, 0), (101, 400, 0)],
            false,
        ),
        anim(
            redtail_pirate::BLOCK_ANIM,
            &[(700, 70, 0), (701, 90, 0)],
            true,
        ),
        anim(
            redtail_pirate::QUICK_BLOCK_ANIM,
            &[(710, 30, 0), (711, 30, 0)],
            true,
        ),
    ]
}

fn claw_anims() -> Vec<Arc<Animation>> {
    let mut v = Vec::new();
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        v.push(anim(key, SWING, true));
    }
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(anim(key, &[(200, 150, 0), (201, 200, 0)], true));
    }
    v.push(anim(
        claw_sim::player::special::LIFT_ANIM,
        &[(40, 100, 0), (42, 100, 0), (43, 100, 0)],
        true,
    ));
    v.push(anim(
        claw_sim::player::special::CARRY_ANIM,
        &[(43, 1000, 0)],
        false,
    ));
    v.push(anim(
        claw_sim::player::special::THROW_ANIM,
        &[(44, 100, 0), (47, 50, 1), (48, 100, 0)],
        true,
    ));
    v.push(anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false));
    v.push(anim("CLAW_DUCK", &[(67, 250, 0)], false));
    v
}

/// Where the arena's pirate stands and its patrol limits.
const AT: i32 = 3000;
const LIMITS: (i32, i32) = (2700, 3300);

/// The flat floor (feet rest on 959) as level `number`, with a pirate at
/// `AT` between `LIMITS`; every object awake.
fn arena(number: u32) -> SimLevel {
    let mut level = flat_level();
    level.number = number;
    for a in pirate_anims().into_iter().chain(claw_anims()) {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("RedTailPirate", AT, 900, 0);
    o.record[field::X_MIN] = LIMITS.0;
    o.record[field::X_MAX] = LIMITS.1;
    // The record's own health and smarts do not count.
    o.record[17] = 40;
    o.record[18] = 20;
    level.objects = vec![o];
    level
}

/// The arena on level `number` with Claw put `dx` px from the pirate on
/// the first tick, tough enough to take any number of strikes.
fn with_claw(number: u32, seed: u32, dx: i32) -> (Sim, ObjectId) {
    let level = arena(number);
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
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::RedTailPirate))
        .map(|(id, _)| id)
        .expect("a pirate");
    (sim, id)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn pdata(sim: &Sim, id: ObjectId) -> RedTailPirateData {
    match &obj(sim, id).data {
        ObjectData::RedTailPirate(d) => d.clone(),
        d => panic!("pirate data: {d:?}"),
    }
}

/// The pirate's states over `n` ticks of `input`.
fn states(sim: &mut Sim, id: ObjectId, n: usize, input: TickInput) -> Vec<Option<EnemyState>> {
    (0..n)
        .map(|_| {
            sim.step(input);
            state_of(sim, id)
        })
        .collect()
}

/// The ticks (indices) on which `s` starts in `seen`.
fn starts(seen: &[Option<EnemyState>], s: EnemyState) -> Vec<usize> {
    (0..seen.len())
        .filter(|&k| seen[k] == Some(s) && (k == 0 || seen[k - 1] != Some(s)))
        .collect()
}

#[test]
fn the_synthetic_animations_keep_the_seventh_levels_timings() {
    let level = arena(7);
    let calls = |key: &str| {
        let mut p = claw_sim::anim::AnimPlayer::default();
        p.set(level.animations[key].clone());
        let mut events = Vec::new();
        let mut n = 0;
        while !p.is_done() {
            n += 1;
            if p.advance_in(18, 0, false, None).event == 1 {
                events.push(n);
            }
        }
        (n, events.first().copied())
    };
    assert_eq!(calls(HIGH_STRIKE_ANIM), (18, Some(7)));
    assert_eq!(calls(LOW_STRIKE_ANIM), (14, Some(7)));
    assert_eq!(calls(redtail_pirate::CLASS.anims.hit_low), (19, None));
    assert_eq!(calls(redtail_pirate::CLASS.anims.jump).0, 10);
}

#[test]
fn a_pirate_is_built_idle_with_its_own_numbers() {
    let (sim, id) = with_claw(7, 1, 400);
    let o = obj(&sim, id);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!((o.health, o.damage), (10, 10));
    assert_eq!(&*o.image_set, redtail_pirate::IMAGE_SET);
    assert_eq!(o.hit_rect, redtail_pirate::HIT_RECT.mirrored());
    let p = pdata(&sim, id);
    assert_eq!(p.jump_back_chance, 90);
    assert!(!p.taunt);
    assert_eq!(p.enemy.smarts, 99);
    assert_eq!(p.enemy.ranges, redtail_pirate::RANGES);
    assert_eq!(p.enemy.bounds, LIMITS);
    assert!(!p.enemy.sight, "not going by sight");
    assert!(p.enemy.may_lift);
    // Standing on the floor, 1 px above a standing Claw's origin.
    assert_eq!(o.y, 959 - 61);
    assert_eq!(o.special_flags & special::LIFTABLE, special::LIFTABLE);
}

#[test]
fn it_engages_at_once_and_never_pauses_in_front_of_claw() {
    for seed in 1..8 {
        // Claw 150 px away: mode 2, it decides on its first step there.
        let (mut sim, id) = with_claw(7, seed, -150);
        // Built idle on the first tick, Home on the next: it decides at
        // once.
        let seen = states(&mut sim, id, 600, TickInput::default());
        assert_ne!(seen[0], Some(EnemyState::Idle), "seed {seed}");
        assert!(
            !seen.contains(&Some(EnemyState::Pause)),
            "seed {seed}: it never pauses"
        );
        assert!(!seen[1..].contains(&Some(EnemyState::Idle)), "seed {seed}");
        // A strike from further out (mode 1 or 2) rests it 2000 ms: the
        // next strike comes at least 112 ticks later.
        let strikes = starts(&seen, EnemyState::Strike);
        assert!(!strikes.is_empty(), "seed {seed}");
    }
}

#[test]
fn a_strike_from_further_out_rests_it_2000_ms() {
    // Claw 150 px away is out of the high strike's 96 px reach; now and
    // then the pirate swings at the air from there, and the next strike
    // comes no sooner than 112 ticks (2016 ms) later.
    let mut checked = 0;
    for seed in 1..30 {
        let (mut sim, id) = with_claw(7, seed, -150);
        let mut strikes: Vec<(usize, i32)> = Vec::new();
        let mut prev = None;
        for k in 0..400 {
            sim.step(TickInput::default());
            let s = state_of(&sim, id);
            if s == Some(EnemyState::Strike) && prev != s {
                let d = (obj(&sim, id).x - sim.player().unwrap().x).abs();
                strikes.push((k, d));
            }
            prev = s;
        }
        for w in strikes.windows(2) {
            let ((a, d), (b, _)) = (w[0], w[1]);
            if d > 100 {
                assert!(b - a >= 112, "seed {seed}: {a} at {d} px, then {b}");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "some seed swings at the air");
}

#[test]
fn the_high_strike_is_live_on_its_7th_tick_and_over_on_its_18th() {
    let (mut sim, id) = with_claw(7, 3, -70);
    let mut seen = Vec::new();
    for _ in 0..300 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        seen.push((state_of(&sim, id), o.attack_rect));
    }
    let strikes: Vec<usize> = (1..seen.len() - 20)
        .filter(|&k| seen[k].0 == Some(EnemyState::Strike) && seen[k - 1].0 != seen[k].0)
        .collect();
    assert!(strikes.len() > 5);
    for &s in &strikes {
        let live: Vec<usize> = (s..seen.len())
            .take_while(|&k| seen[k].0 == Some(EnemyState::Strike))
            .filter(|&k| seen[k].1 == HIGH_STRIKE || seen[k].1 == HIGH_STRIKE.mirrored())
            .map(|k| k - s)
            .collect();
        assert_eq!(live, [7], "strike at {s}");
        let len = (s..seen.len())
            .take_while(|&k| seen[k].0 == Some(EnemyState::Strike))
            .count();
        assert_eq!(len, 18, "strike at {s}");
    }
}

#[test]
fn at_a_crouching_claw_it_strikes_low_and_is_home_on_the_14th_tick() {
    let (mut sim, id) = with_claw(7, 3, -70);
    let mut seen = Vec::new();
    for _ in 0..1500 {
        sim.step(hold(DOWN));
        let o = obj(&sim, id);
        let c = sim.player().unwrap();
        let crouching = c.physics_type == ptype::DUCK;
        let close = (o.x - c.x).abs() < 100;
        seen.push((state_of(&sim, id), o.attack_rect, crouching, close));
    }
    let strikes: Vec<usize> = (20..seen.len() - 20)
        .filter(|&k| seen[k].0 == Some(EnemyState::Strike) && seen[k - 1].0 != seen[k].0)
        .collect();
    assert!(strikes.len() > 5);
    let mut low = 0;
    for &s in &strikes {
        // Every strike is live on its 7th tick, high or low.
        let rect = seen[s + 7].1;
        assert!(
            [LOW_STRIKE, HIGH_STRIKE]
                .iter()
                .any(|r| rect == *r || rect == r.mirrored()),
            "strike at {s}"
        );
        // Close in (the strikes from further out swing high), with Claw
        // crouching throughout (not knocked out of his crouch by a hit):
        // the low box, Home on the 14th tick.
        if seen[s].3 && (s - 1..=s + 7).all(|k| seen[k].2) {
            assert!(
                rect == LOW_STRIKE || rect == LOW_STRIKE.mirrored(),
                "strike at {s}"
            );
            let len = (s..seen.len())
                .take_while(|&k| seen[k].0 == Some(EnemyState::Strike))
                .count();
            assert_eq!(len, 14, "strike at {s}");
            low += 1;
        }
    }
    assert!(low >= 2, "{low}");
}

/// Claw, 70 px left of the pirate and facing it, swings every `gap` ticks
/// for `swings` swings; returns the ticks of each swing, the pirate's
/// states and its hop chance after each tick.
fn swings(seed: u32, gap: usize, swings: usize) -> (Vec<usize>, Vec<Option<EnemyState>>, Vec<i32>) {
    let (mut sim, id) = with_claw(7, seed, -70);
    let mut at = Vec::new();
    let mut seen = Vec::new();
    let mut chance = Vec::new();
    for k in 0..gap * swings {
        let input = if k % gap == 0 {
            at.push(k);
            press(ATTACK)
        } else {
            TickInput::default()
        };
        sim.step(input);
        if sim.objects().get(id).is_none() {
            break;
        }
        seen.push(state_of(&sim, id));
        chance.push(pdata(&sim, id).jump_back_chance);
    }
    (at, seen, chance)
}

#[test]
fn it_hops_back_on_the_tick_claws_attack_starts_with_a_falling_chance() {
    let mut hopped = 0;
    let mut on_first = 0;
    for seed in 1..10 {
        let (at, seen, chance) = swings(seed, 40, 8);
        let hops = starts(&seen, EnemyState::JumpBack);
        // Every hop starts while a swing runs: on its first tick, or a
        // later one when the first roll failed (the roll is made on every
        // tick of the attack until it passes).
        for &h in &hops {
            assert!(
                at.iter().any(|&a| (a..a + 10).contains(&h)),
                "seed {seed}: hop at {h}, swings {at:?}"
            );
        }
        on_first += hops.iter().filter(|h| at.contains(h)).count();
        hopped += hops.len();
        // The chance only falls, 30 at a time, and never below 0: three
        // hops at most.
        let chance: Vec<i32> = std::iter::once(90).chain(chance).collect();
        for w in chance.windows(2) {
            assert!(
                w[1] == w[0] || w[1] == (w[0] - 30).max(0),
                "seed {seed}: {w:?}"
            );
        }
        assert!(hops.len() <= 3, "seed {seed}");
        assert!(chance.iter().all(|&c| c >= 0));
    }
    assert!(hopped > 0);
    // At 90 in 100 the first tick's roll mostly passes.
    assert!(on_first * 2 > hopped, "{on_first} of {hopped}");
}

#[test]
fn its_hop_lands_about_96_px_away_after_18_ticks() {
    for seed in 1..10 {
        let (mut sim, id) = with_claw(7, seed, -70);
        let from = obj(&sim, id).x;
        sim.step(press(ATTACK));
        if state_of(&sim, id) != Some(EnemyState::JumpBack) {
            continue;
        }
        assert_eq!(pdata(&sim, id).jump_back_chance, 60);
        let mut n = 1;
        while state_of(&sim, id) == Some(EnemyState::JumpBack) {
            sim.step(TickInput::default());
            n += 1;
        }
        assert_eq!(n, 19, "18 ticks in the air, Home on the 19th");
        let dx = obj(&sim, id).x - from;
        assert!((96..=97).contains(&dx), "{dx}");
        return;
    }
    panic!("no seed hopped");
}

/// The arena with the pirate's right patrol limit exactly 100 px right
/// of its whole pixel and `fraction` px of its exact position past it.
fn limit_100_px_away(seed: u32, fraction: f64) -> (Sim, ObjectId) {
    let (mut sim, id) = with_claw(7, seed, -70);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        let (x, y) = (o.x, o.y);
        let d = o.data.enemy_mut().unwrap();
        d.bounds.1 = x + 100;
        d.carry.0 = fraction;
        d.moved_to = Some((x, y));
    });
    (sim, id)
}

#[test]
fn a_hop_back_whose_spot_is_a_fraction_past_its_limit_is_refused() {
    for seed in 1..10 {
        let (mut on, id) = limit_100_px_away(seed, 0.0);
        on.step(press(ATTACK));
        if state_of(&on, id) != Some(EnemyState::JumpBack) {
            continue;
        }
        // The spot 100 px from the exact position is on the limit: it
        // hops. A tenth of a pixel further, the spot is past the limit
        // although its whole pixel is not: it stays on the floor.
        let (mut past, id) = limit_100_px_away(seed, 0.1);
        let exact = past.with_world(|w| enemy::exact_x(w, id));
        assert!((exact - f64::from(obj(&past, id).x) - 0.1).abs() < 1e-9);
        past.step(press(ATTACK));
        assert_ne!(state_of(&past, id), Some(EnemyState::JumpBack));
        assert_eq!(obj(&past, id).physics_type, ptype::STAND);
        assert_eq!(obj(&past, id).flags & flags::SAFE, 0);
        return;
    }
    panic!("no seed hopped");
}

#[test]
fn out_of_range_its_hop_chance_goes_back_to_90() {
    for seed in 1..10 {
        let (mut sim, id) = with_claw(7, seed, -70);
        sim.step(press(ATTACK));
        if pdata(&sim, id).jump_back_chance == 90 {
            continue;
        }
        // Claw far away for over 500 ms: at its next Home step the chance
        // is back to 90 and it paces.
        idle(&mut sim, 40);
        sim.teleport_player(AT + 1500, 899);
        idle(&mut sim, 40);
        assert!(pdata(&sim, id).jump_back_chance < 90);
        sim.with_world(|w| {
            if let Some(o) = w.objects.get_mut(id) {
                o.state = EnemyState::Home as i32;
            }
        });
        sim.step(TickInput::default());
        assert_eq!(pdata(&sim, id).jump_back_chance, 90);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Pace));
        return;
    }
    panic!("no seed spent its chance");
}

#[test]
fn on_the_seventh_level_every_blow_lands_and_the_second_kills() {
    for seed in 1..6 {
        // Claw right beside it, facing it: once its hops are spent, blows
        // land; it never parries.
        let (mut sim, id) = with_claw(7, seed, -40);
        let mut seen = Vec::new();
        let mut healths = Vec::new();
        for k in 0..900 {
            sim.step(if k % 25 == 0 {
                press(ATTACK)
            } else {
                TickInput::default()
            });
            if sim.objects().get(id).is_none() {
                break;
            }
            seen.push(state_of(&sim, id));
            healths.push(obj(&sim, id).health);
        }
        assert!(!seen.contains(&Some(EnemyState::Block)), "seed {seed}");
        assert!(!seen.contains(&Some(EnemyState::BlockHigh)), "seed {seed}");
        let dead = starts(&seen, EnemyState::Dead);
        assert_eq!(dead.len(), 1, "seed {seed}: it dies");
        let flinches = starts(&seen, EnemyState::HitLow);
        assert_eq!(flinches.len(), 1, "seed {seed}: one flinch, then death");
        let f = flinches[0];
        assert_eq!(healths[f], 5);
        let len = (f..seen.len())
            .take_while(|&k| seen[k] == Some(EnemyState::HitLow))
            .count();
        assert_eq!(len, 19, "seed {seed}");
    }
}

#[test]
fn on_the_eighth_level_it_parries_some_sword_blows() {
    let mut parried = 0;
    for seed in 1..20 {
        let (mut sim, id) = with_claw(8, seed, -40);
        for k in 0..600 {
            let health = sim.objects().get(id).map(|o| o.health);
            sim.step(if k % 25 == 0 {
                press(ATTACK)
            } else {
                TickInput::default()
            });
            if sim.objects().get(id).is_none() {
                break;
            }
            if state_of(&sim, id) == Some(EnemyState::BlockHigh) {
                // A parry costs it nothing.
                assert_eq!(Some(obj(&sim, id).health), health, "seed {seed}");
                parried += 1;
                break;
            }
        }
    }
    assert!(parried > 0, "a pirate of the eighth level parries");
    assert_eq!(redtail_pirate::block_chances(7), None);
}

/// A random stream whose next draw of 0..`n` is `want`.
fn stream_drawing(n: u32, want: u32) -> Rng {
    (1..100_000)
        .map(Rng::new)
        .find(|r| r.clone().below(n) == want)
        .expect("a seed")
}

/// An attacker of type `kind` doing `damage`, at the pirate.
fn attacker(sim: &mut Sim, kind: u32, damage: i32) -> ObjectId {
    sim.with_world(|w| {
        let mut o = logic::new_object("TestBlow", AT, 900, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = kind;
        o.damage = damage;
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    })
}

/// Hits the pirate with `attacker` as a new attack of Claw's, the parry
/// roll being `roll`; returns the stream as it was before the hit.
fn hit_with_roll(sim: &mut Sim, id: ObjectId, attacker: ObjectId, roll: u32) -> Rng {
    let rng = stream_drawing(101, roll);
    sim.with_world(|w| {
        // A sword blow does 5.
        if let Some(c) = w.claw_id.and_then(|c| w.objects.get_mut(c)) {
            c.damage = 5;
        }
        w.claw.attack.kind = attack_kind::SWIPE;
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        *w.rng = rng.clone();
        redtail_pirate::on_hit(w, id, attacker);
    });
    rng
}

#[test]
fn on_the_eighth_level_a_roll_below_25_parries_a_sword_blow() {
    // A roll of 24: the high guard, its parry sound, no harm, and the
    // attack is not counted as taken. Only the roll and the sound's pick
    // are drawn.
    let (mut sim, id) = with_claw(8, 3, 400);
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    let before = hit_with_roll(&mut sim, id, claw, 24);
    let mut expect = before.clone();
    expect.below(101);
    expect.below(2);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(state_of(&sim, id), Some(EnemyState::BlockHigh));
    assert_eq!(obj(&sim, id).health, 10);
    let serial = sim.with_world(|w| w.claw.attack.serial);
    assert_ne!(pdata(&sim, id).enemy.serial_seen, Some(serial));
    // The guard holds for 10 ticks, then it is Home again.
    let seen = states(&mut sim, id, 12, TickInput::default());
    let guard = seen
        .iter()
        .take_while(|s| **s == Some(EnemyState::BlockHigh))
        .count();
    assert_eq!(guard + 1, 10, "the tick of the parry and 9 more");
    assert_eq!(obj(&sim, id).health, 10);
    // A roll of 25: the blow lands (5 of its 10) and it flinches.
    let (mut sim, id) = with_claw(8, 3, 400);
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    hit_with_roll(&mut sim, id, claw, 25);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 5);
    let serial = sim.with_world(|w| w.claw.attack.serial);
    assert_eq!(pdata(&sim, id).enemy.serial_seen, Some(serial));
}

#[test]
fn on_the_seventh_level_the_same_roll_does_not_parry() {
    let (mut sim, id) = with_claw(7, 3, 400);
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    hit_with_roll(&mut sim, id, claw, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 5);
}

#[test]
fn a_blast_is_never_parried() {
    // Gabriel's bomb blast on a pirate he called in drew a roll of 7 and
    // the pirate flinched: blasts (the level's own shots) go through the
    // hit rule on the eighth level whatever the roll.
    for roll in [0, 7, 24] {
        let (mut sim, id) = with_claw(8, 3, 400);
        let blast = attacker(&mut sim, types::LEVEL_SHOT, 5);
        let before = hit_with_roll(&mut sim, id, blast, roll);
        assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow), "{roll}");
        assert_eq!(obj(&sim, id).health, 5, "{roll}");
        // The roll and the hit line first, as for any hit.
        let mut expect = before.clone();
        expect.below(101);
        expect.below(101);
        assert_ne!(
            sim.with_world(|w| w.rng.clone()),
            expect,
            "the hit rule draws on"
        );
        assert!(
            obj(&sim, blast).flags & flags::DESTROY == 0,
            "the blast stays"
        );
    }
}

#[test]
fn a_blast_reaches_a_pirate_through_the_collision_pass() {
    // A bomb's blast (the level's own shot, 100 px wide) over the pirate
    // hurts it, once; the bomb in flight (an enemy's shot) does not.
    let (mut sim, id) = with_claw(8, 3, 400);
    let y = obj(&sim, id).y;
    let bomb = sim.with_world(|w| {
        let mut o = logic::new_object("TestBomb", AT + 20, y, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = types::ENEMY_SHOT;
        o.attack_rect = Rect::new(-50, -50, 50, 50);
        o.damage = 5;
        w.spawn(o)
    });
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).health, 10, "a bomb in flight passes through");
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(bomb) {
            o.type_flags = types::LEVEL_SHOT;
        }
    });
    let mut seen = Vec::new();
    for _ in 0..16 {
        sim.step(TickInput::default());
        seen.push((state_of(&sim, id), obj(&sim, id).health));
    }
    assert_eq!(seen[0], (Some(EnemyState::HitLow), 5));
    assert!(seen.iter().all(|&(_, h)| h == 5), "one hit: {seen:?}");
}

#[test]
fn on_the_eighth_level_a_low_roll_blocks_a_pistol_shot() {
    // The shot is destroyed and the pirate takes the quick guard, safe
    // while it lasts.
    let (mut sim, id) = with_claw(8, 3, 400);
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    let before = hit_with_roll(&mut sim, id, shot, 24);
    let mut expect = before.clone();
    expect.below(101);
    expect.below(2);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Block));
    assert_eq!(
        obj(&sim, id).anim.key(),
        Some(redtail_pirate::QUICK_BLOCK_ANIM)
    );
    assert!(obj(&sim, id).flags & flags::SAFE != 0);
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
    assert_eq!(obj(&sim, id).health, 10);
    // At 25 it is hit.
    let (mut sim, id) = with_claw(8, 3, 400);
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8);
    hit_with_roll(&mut sim, id, shot, 25);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 2);
}

#[test]
fn every_new_blow_draws_twice_before_the_hit_rule() {
    // Hit by a blow of Claw's it has already taken, nothing is drawn.
    let (mut sim, id) = with_claw(7, 5, 400);
    let attacker = sim.with_world(|w| w.claw_id.expect("Claw"));
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| {
        let serial = w.claw.attack.serial;
        if let Some(d) = enemy::data_mut(w, id) {
            d.serial_seen = Some(serial);
        }
        redtail_pirate::on_hit(w, id, attacker);
    });
    assert_eq!(sim.with_world(|w| w.rng.clone()), before);
    assert_eq!(obj(&sim, id).health, 10);
    // A new one draws the parry roll and the line roll first.
    sim.with_world(|w| {
        w.claw.attack.serial = w.claw.attack.serial.wrapping_add(1);
        if let Some(d) = enemy::data_mut(w, id) {
            d.cooldown = enemy::Timer::default();
        }
    });
    let mut expect = before.clone();
    expect.below(101);
    expect.below(101);
    let (r, cooldown_blocked) = sim.with_world(|w| {
        let now = w.now_ms;
        if let Some(d) = enemy::data_mut(w, id) {
            d.cooldown = enemy::Timer::new(now, 10_000);
        }
        redtail_pirate::on_hit(w, id, attacker);
        (w.rng.clone(), obj_health(w, id))
    });
    // The cooldown turned the hit away after the two draws.
    assert_eq!(r, expect);
    assert_eq!(cooldown_blocked, 10);
}

fn obj_health(w: &logic::World, id: ObjectId) -> i32 {
    w.objects.get(id).map_or(0, |o| o.health)
}

#[test]
fn lifted_it_always_says_one_of_its_five_lines() {
    let (mut sim, id) = with_claw(7, 2, 400);
    let before = sim.with_world(|w| w.rng.clone());
    sim.with_world(|w| redtail_pirate::pick_lift_voice(w, id));
    let mut expect = before.clone();
    expect.below(5);
    assert_eq!(sim.with_world(|w| w.rng.clone()), expect);
    assert_eq!(pdata(&sim, id).enemy.lifted_voice, "");
}

#[test]
fn landing_after_a_throw_sets_its_taunt() {
    let (mut sim, id) = with_claw(7, 2, 400);
    // A landing during its call: the shared throw rule counts it.
    sim.with_world(|w| {
        let claw = w.claw_id.expect("Claw");
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 0, 0);
    });
    let mut landed = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if pdata(&sim, id).taunt {
            landed = true;
            break;
        }
    }
    assert!(landed, "a landing sets the taunt");
    let _ = (LIFT, Rect::UNSET);
}
