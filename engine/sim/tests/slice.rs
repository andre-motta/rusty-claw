//! The combat slice on synthetic data: Claw's melee attacks (timing, attack
//! box and damage per type, the random choice of blow), and the officer
//! (strike, hit reaction, death), with Claw's health loss and his window
//! of invulnerability.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, Buttons, TickInput};
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::pickup::PickupData;
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags};
use claw_sim::player::attack::{self, PUNCHABLE, kind};
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::trace::{self, TraceRow};
use claw_sim::{InputLatch, InputScript, ObjectData, Sim, SimConfig, SimLevel};
use common::*;

const ATTACK: u32 = 0x2;
const TICK_MS: i32 = 18;

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

/// The attack animations, shaped like the game's: a wind-up, one active
/// frame with the strike event, a follow-through.
fn attack_anims() -> Vec<Arc<Animation>> {
    vec![
        anim(
            "CLAW_SWIPE",
            &[
                (24, 25, 0),
                (25, 25, 0),
                (26, 25, 0),
                (27, 125, 1),
                (26, 35, 0),
                (25, 25, 0),
                (24, 25, 0),
            ],
            true,
        ),
        anim("CLAW_PUNCH", &[(35, 35, 0), (36, 50, 0), (36, 50, 1)], true),
        anim(
            "CLAW_UPPERCUT",
            &[(37, 35, 0), (38, 25, 0), (38, 150, 1), (37, 65, 0)],
            true,
        ),
        anim(
            "CLAW_KICK",
            &[
                (57, 25, 0),
                (58, 25, 0),
                (59, 25, 0),
                (59, 50, 1),
                (59, 50, 0),
                (60, 50, 0),
            ],
            true,
        ),
        anim(
            "CLAW_DUCKSWIPE",
            &[
                (74, 50, 0),
                (75, 50, 0),
                (76, 200, 1),
                (75, 25, 0),
                (74, 40, 0),
            ],
            true,
        ),
        anim(
            "CLAW_JUMPSWIPE",
            &[(284, 50, 0), (283, 125, 1), (284, 75, 0), (285, 75, 0)],
            true,
        ),
        anim("CLAW_STAND", &[(11, 110, 0), (12, 110, 0)], false),
        anim("CLAW_DUCK", &[(67, 250, 0)], false),
        anim("CLAW_JUMP", &[(271, 55, 0), (272, 55, 0)], false),
        anim("CLAW_FALL", &[(278, 65, 0), (280, 80, 0)], false),
    ]
}

/// The officer's animations, shaped like the game's.
fn officer_anims() -> Vec<Arc<Animation>> {
    let mut v = vec![
        anim(
            "LEVEL_OFFICER_STRIKE",
            &[
                (200, 400, 0),
                (202, 40, 0),
                (203, 40, 0),
                (204, 250, 1),
                (202, 40, 0),
                (201, 0, 0),
            ],
            true,
        ),
        anim(
            "LEVEL_OFFICER_STRIKE1",
            &[
                (300, 150, 0),
                (301, 40, 0),
                (302, 300, 1),
                (301, 40, 0),
                (201, 0, 0),
            ],
            true,
        ),
        anim(
            "LEVEL_OFFICER_HITLOW",
            &[(600, 50, 0), (601, 200, 0), (600, 50, 0), (201, 0, 0)],
            true,
        ),
        anim(
            "LEVEL_OFFICER_HITHIGH",
            &[(500, 50, 0), (501, 200, 0), (500, 50, 0), (201, 0, 0)],
            true,
        ),
        anim("LEVEL_OFFICER_FALL", &[(950, 0, 0)], true),
        anim("LEVEL_OFFICER_HOME", &[(201, 0, 0)], true),
        anim(
            "LEVEL_OFFICER_FASTADVANCE",
            &[(1, 100, 0), (2, 100, 0), (3, 100, 0)],
            false,
        ),
    ];
    for (i, key) in enemy::officer::STAND.iter().enumerate() {
        v.push(anim(key, &[(100 + i as i32, 0, 0)], true));
    }
    v
}

/// The flat floor of the common helpers with every animation, and the
/// given level records.
fn arena(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut l = flat_level();
    for a in attack_anims().into_iter().chain(officer_anims()) {
        l.animations.insert(a.key.clone(), a);
    }
    l.objects = objects;
    l
}

/// An officer record at (x, y) carrying `loot` in its powerup field.
fn officer_at(x: i32, y: i32, loot: i32) -> ObjectSpec {
    let mut o = ObjectSpec::at("Officer", x, y, 4000);
    o.image_set = "LEVEL_OFFICER".to_string();
    o.record[15] = loot;
    o
}

fn sim_with(level: SimLevel, seed: u32) -> Sim {
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 30);
    sim
}

fn claw(sim: &Sim) -> &Object {
    sim.player().expect("Claw")
}

fn first_officer(sim: &Sim) -> Option<ObjectId> {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(claw_sim::GameLogic::Officer))
        .map(|(id, _)| id)
}

fn officer(sim: &Sim) -> &Object {
    let id = first_officer(sim).expect("an officer");
    sim.objects().get(id).expect("officer")
}

fn officer_state(sim: &Sim) -> Option<EnemyState> {
    first_officer(sim)
        .and_then(|id| sim.objects().get(id))
        .and_then(|o| EnemyState::from_id(o.state))
}

/// Ticks each element of `frames` is shown: its duration in whole ticks,
/// at least one.
fn shown(duration: i32) -> i32 {
    ((duration + TICK_MS - 1) / TICK_MS).max(1)
}

/// The ticks, counted from the tick the attack starts, on which the
/// strike event is current, and the tick on which the attack ends.
fn expected_timing(key: &str) -> (Vec<i32>, i32) {
    let a = attack_anims()
        .into_iter()
        .find(|a| a.key == key)
        .expect("animation");
    let mut live = Vec::new();
    let mut t = 0;
    for f in &a.frames {
        let n = shown(f.duration);
        if f.event == 1 {
            live.extend(t..t + n);
        }
        t += n;
    }
    (live, t)
}

/// Per tick of an attack: (attacking, attack box live, attack box,
/// damage, attack type).
#[derive(Debug, Clone, PartialEq)]
struct AttackTick {
    attacking: bool,
    live: bool,
    rect: Rect,
    damage: i32,
    kind: i32,
}

fn attack_tick(sim: &Sim) -> AttackTick {
    let o = claw(sim);
    AttackTick {
        attacking: sim.claw().attacking,
        live: o.flags & flags::HARMLESS == 0,
        rect: o.attack_rect,
        damage: o.damage,
        kind: sim.claw().attack.kind,
    }
}

/// Presses Attack on top of `held` and records the attack until it ends.
fn run_attack(sim: &mut Sim, held: u32) -> Vec<AttackTick> {
    sim.step(TickInput {
        held: Buttons(held | ATTACK),
        pressed: Buttons(ATTACK),
    });
    let mut out = vec![attack_tick(sim)];
    for _ in 0..60 {
        if !sim.claw().attacking {
            break;
        }
        sim.step(hold(held));
        out.push(attack_tick(sim));
    }
    out
}

/// The first standing blow chosen with `seed` when nothing else draws
/// from the stream: (blow, number of draws).
fn rolled_blow(rng: &mut Rng) -> i32 {
    let mut r = rng.below(4);
    if r == 0 || r == 3 {
        r = rng.below(4);
    }
    attack::MELEE_TABLE[r as usize]
}

fn seed_for(blow: i32) -> u32 {
    (1..1000)
        .find(|&s| rolled_blow(&mut Rng::new(s)) == blow)
        .expect("a seed gives the blow")
}

fn check_timing(ticks: &[AttackTick], anim: &str, kind: i32, rect: Rect, damage: i32) {
    let (live, end) = expected_timing(anim);
    let got_live: Vec<i32> = ticks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.attacking && t.live)
        .map(|(i, _)| i as i32)
        .collect();
    assert_eq!(got_live, live, "{anim}: live ticks");
    assert!(ticks[0].attacking, "{anim}: started");
    assert_eq!(ticks[0].kind, kind, "{anim}");
    let end_tick = ticks.iter().position(|t| !t.attacking).expect("ends") as i32;
    assert_eq!(end_tick, end, "{anim}: end tick");
    for t in ticks.iter().filter(|t| t.attacking) {
        assert_eq!(t.rect, rect, "{anim}: attack box");
        assert_eq!(t.damage, damage, "{anim}: damage");
    }
    assert!(
        !ticks.last().unwrap().live,
        "{anim}: harmless after the end"
    );
}

#[test]
fn standing_blows_have_their_timing_box_and_damage() {
    let cases = [
        (kind::SWIPE, "CLAW_SWIPE", Rect::new(30, -2, 112, 19)),
        (kind::PUNCH, "CLAW_PUNCH", Rect::new(0, -2, 56, 19)),
        (kind::UPPERCUT, "CLAW_UPPERCUT", Rect::new(0, -19, 56, 19)),
        (kind::KICK, "CLAW_KICK", Rect::new(0, -2, 56, 19)),
    ];
    for (blow, key, rect) in cases {
        let mut sim = sim_with(arena(Vec::new()), seed_for(blow));
        assert_eq!(claw(&sim).state, state::STAND);
        let ticks = run_attack(&mut sim, 0);
        check_timing(&ticks, key, blow, rect, 5);
        assert_eq!(claw(&sim).state, state::STAND);
        assert_eq!(sim.claw().attack.uppercut, blow == kind::UPPERCUT);
    }
}

#[test]
fn crouching_and_jumping_swipes() {
    // Crouched: the crouching swipe, low box, damage 2.
    let mut sim = sim_with(arena(Vec::new()), 1);
    for _ in 0..5 {
        sim.step(hold(DOWN));
    }
    assert_eq!(claw(&sim).state, state::CROUCH);
    let ticks = run_attack(&mut sim, DOWN);
    check_timing(
        &ticks,
        "CLAW_DUCKSWIPE",
        kind::DUCK_SWIPE,
        Rect::new(30, 19, 105, 37),
        2,
    );
    assert_eq!(claw(&sim).state, state::CROUCH);
    // In the air: the jumping swipe.
    let mut sim = sim_with(arena(Vec::new()), 1);
    sim.step(press(JUMP));
    sim.step(hold(JUMP));
    assert_eq!(claw(&sim).state, state::JUMP);
    let ticks = run_attack(&mut sim, JUMP);
    assert_eq!(ticks[0].kind, kind::JUMP_SWIPE);
    assert_eq!(ticks[0].rect, Rect::new(30, -2, 112, 19));
    assert_eq!(ticks[0].damage, 5);
    assert!(ticks.iter().any(|t| t.attacking && t.live));
}

#[test]
fn facing_left_mirrors_the_attack_box() {
    let mut sim = sim_with(arena(Vec::new()), seed_for(kind::SWIPE));
    for _ in 0..3 {
        sim.step(hold(LEFT));
    }
    sim.step(TickInput::default());
    assert!(!sim.claw().facing_right);
    // Nothing else drew from the stream: the same blow comes up.
    let ticks = run_attack(&mut sim, 0);
    check_timing(
        &ticks,
        "CLAW_SWIPE",
        kind::SWIPE,
        Rect::new(-112, -2, -30, 19),
        5,
    );
}

#[test]
fn walking_into_an_attack_stops_the_walk() {
    let mut sim = sim_with(arena(Vec::new()), seed_for(kind::SWIPE));
    for _ in 0..5 {
        sim.step(hold(RIGHT));
    }
    assert_eq!(claw(&sim).state, state::MOVE_RIGHT);
    let x = claw(&sim).x;
    sim.step(TickInput {
        held: Buttons(RIGHT | ATTACK),
        pressed: Buttons(ATTACK),
    });
    assert!(sim.claw().attacking);
    assert_eq!(sim.claw().attack.kind, kind::SWIPE);
    assert_eq!(claw(&sim).state, state::STAND, "the walk ended");
    assert_eq!(
        sim.claw().attack.restore_anim.as_deref(),
        Some("CLAW_STAND"),
        "back to standing after the blow"
    );
    let ticks = run_attack(&mut sim, 0);
    assert!(ticks.iter().any(|t| t.live));
    assert!(claw(&sim).x - x < 10, "no sliding during the blow");
}

/// A stand-in target object: an enemy body with a hit box, at x.
fn target(x: i32, y: i32, punchable: bool) -> Object {
    let mut o = Object::new(0, x, y, 4000, Arc::from("Target"), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = claw_sim::object::types::ENEMY;
    o.hit_rect = Rect::new(-20, -50, 20, 50);
    if punchable {
        o.special_flags |= PUNCHABLE;
    }
    o
}

/// A target: (position or distance, punchable).
type Spot = (i32, bool);

/// Mirrors the melee choice for a target `dist` px away (or none).
fn mirror_choice(rng: &mut Rng, target: Option<Spot>) -> i32 {
    let mut r = rng.below(4) as i32;
    let target = target.filter(|&(d, _)| d <= attack::TARGET_RANGE);
    if r == 0 || r == 3 {
        r = rng.below(4) as i32;
    }
    let index = match target {
        Some((_, false)) => 0,
        Some((d, true)) if d < attack::CLOSE_RANGE => 1 + rng.below(3) as i32,
        Some((d, true)) if d < attack::TARGET_RANGE => 0,
        _ => r,
    };
    attack::MELEE_TABLE[index as usize]
}

#[test]
fn melee_choice_follows_the_random_stream() {
    // Claw stands at x = 1000 facing right; his hit box reaches x + 15.
    // Targets: none, a punchable one 15 px away, one 45 px away, one that
    // is not punchable 15 px away (counted 48 px closer), and one out of
    // range.
    let setups: [(Option<Spot>, Option<Spot>); 5] = [
        (None, None),
        (Some((1050, true)), Some((15, true))),
        (Some((1080, true)), Some((45, true))),
        (Some((1050, false)), Some((-33, false))),
        (Some((1300, true)), Some((265, true))),
    ];
    for (place, seen) in setups {
        let seed = 7;
        let mut sim = sim_with(arena(Vec::new()), seed);
        if let Some((x, punchable)) = place {
            sim.spawn(target(x, claw(&sim).y, punchable));
        }
        let mut mirror = Rng::new(seed);
        for _ in 0..12 {
            let expected = mirror_choice(&mut mirror, seen);
            let ticks = run_attack(&mut sim, 0);
            assert_eq!(ticks[0].kind, expected, "target {place:?}");
            idle(&mut sim, 2);
            // One to three draws per press and none elsewhere.
            assert_eq!(sim.rng().state(), mirror.state(), "target {place:?}");
        }
    }
}

#[test]
fn up_with_attack_draws_but_starts_no_blow() {
    let mut sim = sim_with(arena(Vec::new()), 3);
    let before = sim.rng().clone();
    sim.step(TickInput {
        held: Buttons(UP | ATTACK),
        pressed: Buttons(ATTACK),
    });
    assert!(!sim.claw().attacking);
    let mut mirror = before;
    let r = mirror.below(4);
    if r == 0 || r == 3 {
        mirror.below(4);
    }
    assert_eq!(sim.rng().state(), mirror.state());
}

#[test]
fn officer_strikes_and_claw_loses_health_then_is_shielded() {
    // Claw at x = 1000, the officer 80 px to his right: in range.
    let mut sim = sim_with(arena(vec![officer_at(1080, 900, 0)]), 5);
    let o = officer(&sim);
    assert_eq!(o.health, 3);
    assert_eq!(o.damage, 10);
    assert_eq!(o.type_flags, claw_sim::object::types::ENEMY);
    assert_eq!(o.y + o.move_rect.bottom, 959, "dropped onto the floor");
    let mut hits = Vec::new();
    let mut last = sim.stats().health;
    let mut struck_box = false;
    for i in 0..400 {
        sim.step(TickInput::default());
        let o = officer(&sim);
        if o.attack_rect == enemy::officer::STRIKE_HIGH {
            struck_box = true;
            assert_eq!(officer_state(&sim), Some(EnemyState::Strike));
        }
        let h = sim.stats().health;
        if h != last {
            assert_eq!(last - h, 10, "the officer's damage");
            assert_eq!(sim.stats().invulnerable_ms, 850);
            hits.push(i);
            last = h;
        }
    }
    assert!(struck_box, "the strike box was live");
    assert!(hits.len() >= 3, "the officer keeps striking: {hits:?}");
    let shield_ticks = (850 + TICK_MS - 1) / TICK_MS;
    for pair in hits.windows(2) {
        assert!(pair[1] - pair[0] >= shield_ticks, "shielded: {hits:?}");
    }
    let cues = sim.last_audio().to_vec();
    let _ = cues;
}

#[test]
fn touching_an_officer_hurts_once_per_window() {
    // The officer stands on Claw: its touch hurts, the window shields.
    let mut sim = sim_with(arena(vec![officer_at(1000, 900, 0)]), 9);
    let mut drops = Vec::new();
    let mut last = sim.stats().health;
    for i in 0..300 {
        sim.step(TickInput::default());
        let h = sim.stats().health;
        if h < last {
            drops.push(i);
        }
        last = h;
    }
    assert!(drops.len() >= 3, "{drops:?}");
    for pair in drops.windows(2) {
        assert!(pair[1] - pair[0] >= 48, "{drops:?}");
    }
}

/// Runs until the officer's state is `s`, at most `n` ticks.
fn until_state(sim: &mut Sim, s: EnemyState, n: usize) -> bool {
    for _ in 0..n {
        if officer_state(sim) == Some(s) {
            return true;
        }
        sim.step(TickInput::default());
    }
    officer_state(sim) == Some(s)
}

#[test]
fn a_blow_makes_the_officer_flinch_and_slide_away() {
    let mut sim = sim_with(arena(vec![officer_at(1080, 900, 0)]), 2);
    let id = first_officer(&sim).unwrap();
    // Tougher than usual, to watch the flinch without the death.
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 20);
    // The officer is 45 px from Claw: the choice is the swipe.
    let ticks = run_attack(&mut sim, 0);
    assert_eq!(ticks[0].kind, kind::SWIPE);
    assert_eq!(officer(&sim).health, 15, "one swipe of 5");
    assert!(
        until_state(&mut sim, EnemyState::Home, 30),
        "flinched and came back"
    );
    // Replay the blow tick by tick to watch the slide.
    let mut sim = sim_with(arena(vec![officer_at(1080, 900, 0)]), 2);
    let id = first_officer(&sim).unwrap();
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 20);
    sim.step(TickInput {
        held: Buttons(ATTACK),
        pressed: Buttons(ATTACK),
    });
    let mut xs = Vec::new();
    let mut flinch_cues = Vec::new();
    for _ in 0..40 {
        if officer_state(&sim) == Some(EnemyState::HitLow) {
            xs.push(officer(&sim).x);
        }
        sim.step(TickInput::default());
        flinch_cues.extend(sim.last_audio().iter().filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        }));
    }
    assert!(xs.len() >= 10, "{xs:?}");
    // Knocked away from Claw (to the right) by 12 px over 8 ticks, then
    // still for the rest of the flinch.
    assert_eq!(xs.last().unwrap() - xs[0], 12, "{xs:?}");
    assert!(xs.windows(2).all(|p| p[1] >= p[0]), "{xs:?}");
    assert_eq!(xs[xs.len() - 1], xs[xs.len() - 2], "stopped: {xs:?}");
    for key in ["GAME_SOLHITL2", "GAME_HIT1"] {
        assert!(
            flinch_cues.iter().any(|k| k == key),
            "{key}: {flinch_cues:?}"
        );
    }
}

#[test]
fn an_uppercut_makes_a_high_flinch() {
    let mut highs = 0;
    let mut lows = 0;
    for seed in 1..40 {
        // 15 px away: the choice is a punch, an uppercut or a kick.
        let mut sim = sim_with(arena(vec![officer_at(1050, 900, 0)]), seed);
        let id = first_officer(&sim).unwrap();
        sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 20);
        sim.step(TickInput {
            held: Buttons(ATTACK),
            pressed: Buttons(ATTACK),
        });
        let blow = sim.claw().attack.kind;
        assert!(
            [kind::PUNCH, kind::UPPERCUT, kind::KICK].contains(&blow),
            "{blow}"
        );
        let mut flinch = None;
        for _ in 0..20 {
            sim.step(TickInput::default());
            if let Some(s @ (EnemyState::HitLow | EnemyState::HitHigh)) = officer_state(&sim) {
                flinch = Some(s);
                break;
            }
        }
        let flinch = flinch.expect("the blow landed");
        if blow == kind::UPPERCUT {
            assert_eq!(flinch, EnemyState::HitHigh);
            highs += 1;
        } else {
            assert_eq!(flinch, EnemyState::HitLow);
            lows += 1;
        }
    }
    assert!(highs > 0 && lows > 0, "{highs} {lows}");
}

#[test]
fn the_officer_dies_drops_its_loot_and_flies_off() {
    let mut sim = sim_with(arena(vec![officer_at(1080, 900, 33)]), 4);
    let id = first_officer(&sim).unwrap();
    sim.step(TickInput {
        held: Buttons(ATTACK),
        pressed: Buttons(ATTACK),
    });
    assert!(until_state(&mut sim, EnemyState::Dead, 20), "killed");
    let o = officer(&sim);
    assert!(o.health <= 0);
    assert!(o.flags & flags::HARMLESS != 0, "a body hurts nobody");
    let drops: Vec<i32> = sim
        .objects()
        .all()
        .filter_map(|(_, o)| match &o.data {
            ObjectData::Pickup(PickupData { drop_code }) => *drop_code,
            _ => None,
        })
        .collect();
    assert_eq!(drops, vec![33], "the loot");
    let y0 = officer(&sim).y;
    let mut cues = Vec::new();
    let mut gone = false;
    let mut rose = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        cues.extend(sim.last_audio().iter().filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        }));
        match sim.objects().get(id) {
            Some(o) => rose |= o.y < y0,
            None => {
                gone = true;
                break;
            }
        }
    }
    assert!(rose, "the body flies up first");
    assert!(gone, "removed once off screen");
    assert!(cues.iter().any(|k| k == "GAME_SPLASH"), "{cues:?}");
    // The kill drew the death roll: either Claw's line or the death sound
    // played at the kill (checked in the install test with real keys).
}

/// Feeds a script to a latch at a simulated display rate and returns the
/// trace.
fn trace_at_rate(level: SimLevel, script: &InputScript, hz: u64) -> String {
    let mut sim = Sim::new(level, SimConfig::default());
    let mut latch = InputLatch::new();
    let events = script.events();
    let mut next_event = 0;
    let mut rows = Vec::new();
    let mut tick = 0u64;
    let n = script.ticks.len() as u64;
    let mut frame = 0u64;
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            rows.push(TraceRow::capture(&sim, sim.tick() - 1, "script"));
            tick += 1;
        }
        frame += 1;
    }
    trace::to_csv(&rows)
}

#[test]
fn a_fight_traces_the_same_at_60_144_and_240_hz() {
    let level = || arena(vec![officer_at(1080, 900, 33), officer_at(1450, 900, 0)]);
    let s = script(&[
        (0, 1200),
        (ATTACK, 40),
        (0, 400),
        (ATTACK, 40),
        (0, 600),
        (RIGHT, 700),
        (0, 300),
        (DOWN, 300),
        (DOWN | ATTACK, 40),
        (DOWN, 500),
        (0, 1000),
        (ATTACK, 40),
        (0, 1500),
    ]);
    let mut direct = Sim::new(level(), SimConfig::default());
    let rows: Vec<TraceRow> = s
        .ticks
        .iter()
        .map(|&t| {
            direct.step(t);
            TraceRow::capture(&direct, direct.tick() - 1, "script")
        })
        .collect();
    let base = trace::to_csv(&rows);
    assert!(rows.iter().any(|r| r.health < 100), "Claw was hit");
    for hz in [60, 144, 240] {
        assert_eq!(trace_at_rate(level(), &s, hz), base, "{hz} Hz");
    }
}
