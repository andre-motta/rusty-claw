//! Katherine in a synthetic walled clearing, with the boss fight flag set
//! directly: how she is built, waiting for the fight and waking, her
//! decisions at Home, the reach of her whip and her stab against Claw's
//! hit box, her somersault over Claw (and the walls and gaps that refuse
//! it), her dodge, blocks and flinches, Claw lifting and throwing her
//! (and the 20 px in front of him where he cannot), her death and its
//! report, the reset when Claw respawns and the respawn's update passes
//! (which take no time for her), when gravity starts to pull on her
//! standing, and the order of her random draws.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, Point, ScreenTransition, TickInput, TransitionKind};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, katherine};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::player::attack::{PUNCHABLE, kind as attack_kind};
use claw_sim::player::state as claw_state;
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};
use common::*;

const LIFT: u32 = 0x20;

/// Where she is placed: 8 px above the floor, as on her level.
const PLACED: (i32, i32) = (2600, 901);
/// Her x and y standing on the clearing's floor.
const AT: (i32, i32) = (2600, 909);
/// The gem's target in her record.
const GEM_TARGET: (i32, i32) = (2565, 735);
/// The clearing's floor between its walls: x from 1984 to 3199.
const FLOOR: (i32, i32) = (1984, 3199);

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

/// Her animations with the lengths of her level's.
fn her_anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_KATHERINE_{s}");
    let e = |ms: &[i32], event: usize| -> Vec<(i32, i32, i32)> {
        ms.iter()
            .enumerate()
            .map(|(i, &d)| (i as i32 + 1, d, i32::from(i + 1 == event)))
            .collect()
    };
    vec![
        anim(
            &k("STRIKE1"),
            &e(&[20, 30, 40, 30, 30, 30, 150, 50, 80, 0], 7),
            true,
        ),
        anim(&k("STRIKE2"), &e(&[50, 200, 90, 200, 80, 100], 3), true),
        anim(&k("FLIP2"), &e(&[100; 6], 0), false),
        anim(&k("FLIP3"), &e(&[150, 120, 100], 0), true),
        anim(&k("HITLOW"), &e(&[50, 200, 50, 0], 0), true),
        anim(&k("HITHIGH"), &e(&[50, 200, 50, 0], 0), true),
        anim(&k("BLOCKLOW"), &e(&[500], 0), true),
        anim(
            &k("IDLE"),
            &e(&[100, 150, 250, 100, 170, 100, 200, 200, 170, 50, 0], 0),
            true,
        ),
        anim(&k("WALK"), &e(&[150; 8], 0), false),
        anim(&k("LIFTED1"), &e(&[100, 100], 0), true),
        anim(&k("LIFTED2"), &e(&[177; 7], 0), false),
        anim(&k("HOME"), &e(&[0], 0), true),
        anim(&k("FALL"), &e(&[100], 0), false),
        anim(&k("THROWN"), &e(&[100], 0), false),
    ]
}

/// Sounds the tests listen for, with their lengths.
const SOUNDS: [(&str, u32); 12] = [
    (katherine::IDLE_LINE, 4745),
    (katherine::DEATH_SOUND, 1505),
    ("LEVEL_KATHERINE_00470000", 985),
    ("LEVEL_KATHERINE_00470003", 819),
    (katherine::LANDING_LINE, 1303),
    (katherine::LIFTED_VOICE, 596),
    (katherine::WHIP_SOUND, 833),
    ("LEVEL_STAB2", 200),
    ("LEVEL_STAB3", 238),
    (katherine::FLIP_SOUND, 775),
    ("LEVEL_KATHERINE_BLOCK1", 398),
    ("LEVEL_KATHERINE_BLOCK2", 501),
];

/// Her walled clearing: a floor whose top is at y 960 (feet rest on 959)
/// between Solid walls at x 1920..1983 and 3200..3263, rising from y 320.
fn clearing() -> SimLevel {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..15 {
        let mut row = ".".repeat(80);
        if r >= 5 {
            row.replace_range(30..31, "#");
            row.replace_range(50..51, "#");
        }
        rows.push(row);
    }
    rows.push("#".repeat(80));
    rows.push("#".repeat(80));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (2200, 900));
    for a in her_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in SOUNDS {
        level.sounds.insert(k.to_string(), Some(ms));
    }
    let mut o = ObjectSpec::at("Katherine", PLACED.0, PLACED.1, 0);
    o.image_set = "LEVEL_KATHERINE".into();
    // The record's own health does not count.
    o.record[18] = 200;
    o.record[55] = GEM_TARGET.0;
    o.record[56] = GEM_TARGET.1;
    level.objects = vec![o];
    level
}

fn arena(seed: u32) -> Sim {
    let level = clearing();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

fn her_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Katherine))
        .map(|(id, _)| id)
        .expect("Katherine exists")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn data(sim: &Sim, id: ObjectId) -> enemy::EnemyData {
    enemy::data_of(obj(sim, id)).expect("built").clone()
}

fn safe(sim: &Sim, id: ObjectId) -> bool {
    obj(sim, id).flags & flags::SAFE != 0
}

fn cues(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// The clearing with Claw `dx` px from her (standing on the floor), the
/// fight flag as given, after `ticks` ticks.
fn with_claw(seed: u32, dx: i32, flag: u8, ticks: usize) -> (Sim, ObjectId) {
    let mut sim = arena(seed);
    let id = her_id(&sim);
    sim.teleport_player(AT.0 + dx, 899);
    sim.with_world(|w| w.set_boss_fight(flag));
    idle(&mut sim, ticks);
    (sim, id)
}

/// She is in the fight with Claw `dx` px away (tough), put at Home,
/// standing still and free.
fn in_fight(dx: i32) -> (Sim, ObjectId) {
    let (mut sim, id) = with_claw(3, dx, fight::ON, 3);
    assert_ne!(state_of(&sim, id), Some(EnemyState::Startup));
    home(&mut sim, id);
    sim.with_world(|w| w.claw.stats.health = 10_000);
    (sim, id)
}

/// Puts her at Home, standing still and free, where she stands.
fn home(sim: &mut Sim, id: ObjectId) {
    sim.with_world(|w| {
        enemy::go_home(w, id);
        let d = enemy::data_mut(w, id).unwrap();
        d.busy = false;
    });
}

/// Runs her Home rule once, now, with the range mode `mode`, the tick's
/// roll `roll` and the stream at `stream`.
fn home_with(sim: &mut Sim, id: ObjectId, mode: u8, roll: i32, stream: u32) {
    sim.set_rng_state(stream);
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.mode = mode;
        d.roll = roll;
        katherine::home_tick(w, id);
    });
}

/// A stream state whose next draw of 0..`n` is in `want`; searched from 0.
fn stream_with(n: u32, want: std::ops::Range<u32>) -> u32 {
    (0..1_000_000u32)
        .find(|&s| want.contains(&Rng::new(s).below(n)))
        .expect("a stream")
}

/// A synthetic attacker of `type_flags` with `damage`.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits her as a new attack of Claw, her roll set to `roll`;
/// returns the random draws the hit made.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId, roll: i32, upper: bool) -> u32 {
    let before = sim.rng().state();
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.uppercut = upper;
        enemy::data_mut(w, id).unwrap().roll = roll;
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, attacker);
    });
    draws_between(before, sim.rng().state())
}

fn draws_between(a: u32, b: u32) -> u32 {
    let mut r = Rng::new(a);
    for n in 0..=64 {
        if r.state() == b {
            return n;
        }
        r.next_u15();
    }
    panic!("more than 64 draws");
}

// ----------------------------------------------------------------------
// Building, waiting for the fight and waking

#[test]
fn she_is_built_as_the_boss_waiting_for_the_fight() {
    let (sim, id) = with_claw(3, -157, fight::NONE, 2);
    let o = obj(&sim, id);
    assert_eq!(o.kind, LogicKind::Game(GameLogic::Katherine));
    assert!(matches!(o.data, ObjectData::Katherine(_)), "her own data");
    assert_eq!(sim.boss().current(), Some(id), "the current boss");
    assert_eq!(o.health, 100, "the record's health does not count");
    assert_eq!(o.damage, 5);
    assert_eq!(o.image_set, "LEVEL_KATHERINE");
    assert_eq!(o.move_rect, katherine::MOVE_RECT);
    assert_eq!(o.hit_rect, katherine::HIT_RECT);
    assert_eq!(o.attack_rect, katherine::CONTACT);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    assert!(o.special_flags & special::LIFTABLE != 0, "liftable");
    assert!(o.special_flags & PUNCHABLE != 0);
    assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
    assert_eq!(o.physics_type, ptype::STAND);
    assert_eq!((o.x, o.y), AT, "dropped onto the floor");
    let d = data(&sim, id);
    assert_eq!(d.smarts, 99);
    assert_eq!(d.ranges, katherine::RANGES);
    assert!(d.may_lift);
    let b = d.human.boss.expect("the boss layer");
    assert_eq!(b.start, PLACED, "placed position kept");
    assert_eq!(b.full_health, 100);
    let carry = enemy::class(GameLogic::Katherine).unwrap().carry.unwrap();
    assert_eq!(carry.beside, (-20, 20, -15));
    assert_eq!(carry.overhead, (0, 0, -60));
    assert_eq!(carry.move_rect, katherine::CARRIED_MOVE_RECT);
    // Claw 157 px away: mode 2, and she waits in Startup.
    assert_eq!(d.mode, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert_eq!(obj(&sim, id).anim.key(), Some(katherine::IDLE_ANIM));
}

#[test]
fn startup_holds_until_the_fight_is_on_then_she_attacks_at_once() {
    let (mut sim, id) = with_claw(3, -157, fight::NONE, 2);
    for flag in [fight::NONE, fight::STAGING] {
        sim.with_world(|w| w.set_boss_fight(flag));
        idle(&mut sim, 40);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    }
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert!(cues(&sim).contains(&katherine::IDLE_LINE.to_string()));
    // No wait: Home on the next tick, Claw being within her boxes.
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
}

#[test]
fn her_idle_line_comes_every_time_she_goes_idle_on_screen() {
    for roll in [0, 500, 999, 0x1FFF_FFFF] {
        let (mut sim, id) = with_claw(3, -157, fight::NONE, 2);
        sim.with_world(|w| {
            enemy::data_mut(w, id).unwrap().roll = roll;
            katherine::go_idle(w, id);
        });
        assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
        let d = data(&sim, id);
        assert_eq!(d.vx, 0.0);
        assert_eq!(d.pace_wait, enemy::Timer::default(), "no wait armed");
        sim.step(TickInput::default());
        assert!(
            cues(&sim).contains(&katherine::IDLE_LINE.to_string()),
            "roll {roll}"
        );
    }
}

/// Claw out of all her boxes (60 px above her) for `ticks` ticks from her
/// third tick, then back: whether she left Startup with the fight flag at
/// 0.
fn woke_after_out_of_range(ticks: usize) -> bool {
    let (mut sim, id) = with_claw(3, -100, fight::NONE, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    let at = sim.player().map(|o| (o.x, o.y)).unwrap();
    for _ in 0..ticks {
        sim.with_world(|w| {
            let c = w.claw_id.unwrap();
            let o = w.objects.get_mut(c).unwrap();
            o.x = at.0;
            o.y = AT.1 - 60;
        });
        sim.step(TickInput::default());
    }
    sim.teleport_player(at.0, at.1);
    sim.step(TickInput::default());
    state_of(&sim, id) != Some(EnemyState::Startup)
}

#[test]
fn claw_out_of_her_boxes_for_500_ms_wakes_her_without_the_fight() {
    // Out of range from a first tick on: 486 ms later (28 ticks out) she
    // still waits; 504 ms later (29 ticks out) she goes idle and fights,
    // as after the intro skip on her level (steps 27 to 55).
    assert!(!woke_after_out_of_range(28));
    assert!(woke_after_out_of_range(29));
}

#[test]
fn her_boxes_reach_40_px_up_and_down() {
    let (sim, id) = with_claw(3, -150, fight::NONE, 2);
    assert_eq!(data(&sim, id).mode, 2);
    let mut d = data(&sim, id).0.as_ref().clone();
    let m = |d: &mut enemy::EnemyFields, dx: i32, dy: i32| {
        enemy::range_mode(d, AT, (AT.0 + dx, AT.1 + dy), false, 0)
    };
    // Each box takes its left edge and leaves out its right one.
    assert_eq!(m(&mut d, -50, 0), 4);
    assert_eq!(m(&mut d, 49, 0), 4);
    assert_eq!(m(&mut d, 50, 0), 3);
    assert_eq!(m(&mut d, -51, 0), 3);
    assert_eq!(m(&mut d, -100, 0), 3);
    assert_eq!(m(&mut d, -101, 0), 2);
    assert_eq!(m(&mut d, -200, 0), 2);
    assert_eq!(m(&mut d, -201, 0), 0, "mode 1 never comes");
    assert_eq!(m(&mut d, -500, 0), 0);
    assert_eq!(m(&mut d, -501, 0), 6);
    assert_eq!(m(&mut d, -30, -40), 4);
    assert_eq!(m(&mut d, -30, -41), 6);
    assert_eq!(m(&mut d, -30, 39), 4);
    assert_eq!(m(&mut d, -30, 40), 6);
}

// ----------------------------------------------------------------------
// Home

#[test]
fn up_close_she_backs_off_or_stabs_even_odds() {
    let (mut sim, id) = in_fight(-40);
    let back = stream_with(100, 0..50);
    home_with(&mut sim, id, 4, 0x1FFF_FFFF, back);
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveBack));
    // Backing off draws its length too: two draws.
    let mut r = Rng::new(back);
    r.next_u15();
    r.next_u15();
    assert_eq!(sim.rng().state(), r.state());

    for (second, sound) in [(0..50, "LEVEL_STAB2"), (50..100, "LEVEL_STAB3")] {
        let (mut sim, id) = in_fight(-40);
        let stream = (0..1_000_000u32)
            .find(|&s| {
                let mut r = Rng::new(s);
                r.below(100) >= 50 && second.contains(&r.below(100))
            })
            .unwrap();
        home_with(&mut sim, id, 4, 0x1FFF_FFFF, stream);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
        let d = data(&sim, id);
        assert_eq!(d.strike_rect, katherine::STAB);
        assert_eq!(d.strike_anim, katherine::STAB_ANIM);
        assert_eq!(d.human.sounds.strike, sound);
        assert!(d.busy);
    }
}

#[test]
fn within_200_px_the_roll_picks_the_whip_the_leap_or_a_wait() {
    for mode in [2, 3] {
        // Below 400: the whip, with a line picked by a draw.
        let (mut sim, id) = in_fight(-157);
        home_with(&mut sim, id, mode, 399, stream_with(100, 0..50));
        assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
        let d = data(&sim, id);
        assert_eq!(d.strike_rect, katherine::WHIP);
        assert_eq!(d.strike_anim, katherine::WHIP_ANIM);
        assert_eq!(d.human.sounds.strike, katherine::WHIP_SOUND);
        sim.step(TickInput::default());
        // 400 to 599: the leap.
        let (mut sim, id) = in_fight(-157);
        home_with(&mut sim, id, mode, 400, 7);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Flip), "mode {mode}");
        let (mut sim, id) = in_fight(-157);
        home_with(&mut sim, id, mode, 599, 7);
        assert_eq!(state_of(&sim, id), Some(EnemyState::Flip));
        // 600 and above, and between pulses: she waits, drawing nothing.
        for roll in [600, 1000, 0x1FFF_FFFF] {
            let (mut sim, id) = in_fight(-157);
            home_with(&mut sim, id, mode, roll, 7);
            assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
            assert_eq!(sim.rng().state(), 7, "no draw");
        }
    }
}

#[test]
fn far_off_she_runs_at_claw_at_200_px_per_s_and_walks_in_closer() {
    let (mut sim, id) = in_fight(-400);
    home_with(&mut sim, id, 0, 0x1FFF_FFFF, 7);
    assert_eq!(state_of(&sim, id), Some(EnemyState::FastAdvance));
    assert_eq!(data(&sim, id).vx, -katherine::ADVANCE_SPEED);
    assert!(!enemy::faces_right(obj(&sim, id)), "facing Claw");
    // She runs at him until within 200 px, walks on at the same speed and
    // comes Home within 100 px.
    let mut seen = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id).unwrap();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        if matches!(s, EnemyState::Advance | EnemyState::FastAdvance) {
            assert_eq!(data(&sim, id).vx.abs(), katherine::ADVANCE_SPEED);
        }
        if s != EnemyState::FastAdvance && s != EnemyState::Advance {
            break;
        }
    }
    assert_eq!(
        &seen[..2],
        &[EnemyState::FastAdvance, EnemyState::Advance],
        "{seen:?}"
    );
    let claw_x = sim.player().unwrap().x;
    assert!((obj(&sim, id).x - claw_x).abs() < 110);
}

// ----------------------------------------------------------------------
// Strikes and their reach

/// Starts `rect`'s strike with Claw `dist` px to her left (standing and
/// facing her), and plays it out: Claw's health lost and the tick the
/// strike box came on.
fn strike_at(dist: i32, whip: bool) -> (i32, Option<usize>) {
    let (mut sim, id) = in_fight(-dist);
    sim.with_world(|w| w.claw.stats.health = 100);
    assert!(sim.claw().facing_right, "Claw faces her");
    let (mode, roll, stream) = if whip {
        (2, 0, stream_with(100, 0..50))
    } else {
        (
            4,
            0x1FFF_FFFF,
            (0..1_000_000u32)
                .find(|&s| Rng::new(s).below(100) >= 50)
                .unwrap(),
        )
    };
    home_with(&mut sim, id, mode, roll, stream);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    let rect = if whip {
        katherine::WHIP
    } else {
        katherine::STAB
    };
    let mut live = None;
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::Strike) {
        // Claw stays where he is.
        sim.with_world(|w| {
            let c = w.claw_id.unwrap();
            let o = w.objects.get_mut(c).unwrap();
            o.x = AT.0 - dist;
        });
        sim.step(TickInput::default());
        n += 1;
        if obj(&sim, id).attack_rect == rect {
            live.get_or_insert(n);
        }
        assert!(n < 60);
    }
    (100 - sim.stats().health, live)
}

#[test]
fn the_whip_reaches_claw_up_to_190_px_and_the_stab_up_to_63() {
    for (dist, lost) in [(30, 5), (100, 5), (170, 5), (190, 5), (191, 0), (195, 0)] {
        let (got, live) = strike_at(dist, true);
        assert_eq!(got, lost, "whip at {dist} px");
        assert_eq!(live, Some(14), "the whip's box on its 14th tick");
    }
    for (dist, lost) in [(30, 5), (60, 5), (63, 5), (64, 0), (70, 0)] {
        let (got, live) = strike_at(dist, false);
        assert_eq!(got, lost, "stab at {dist} px");
        assert_eq!(live, Some(16), "the stab's box on its 16th tick");
    }
}

#[test]
fn the_strike_boxes_mirror_when_she_faces_right() {
    let (mut sim, id) = in_fight(150);
    home_with(&mut sim, id, 2, 0, stream_with(100, 0..50));
    let mut seen = false;
    while state_of(&sim, id) == Some(EnemyState::Strike) {
        sim.step(TickInput::default());
        seen |= obj(&sim, id).attack_rect == Rect::new(13, -22, 175, 9);
    }
    assert!(seen);
}

// ----------------------------------------------------------------------
// The somersault

#[test]
fn the_somersault_flies_200_to_400_px_past_claw_and_lands() {
    let (mut sim, id) = in_fight(-120);
    let claw_x = sim.player().unwrap().x;
    // A draw of 0 makes the shortest leap: 200 px past Claw.
    let stream = stream_with(201, 0..1);
    home_with(&mut sim, id, 2, 450, stream);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Flip));
    assert!(safe(&sim, id) && data(&sim, id).busy);
    assert!(!enemy::faces_right(obj(&sim, id)), "facing Claw");
    let mut flight = 0;
    while state_of(&sim, id) == Some(EnemyState::Flip) {
        sim.step(TickInput::default());
        flight += 1;
        assert!(safe(&sim, id), "safe in the air");
        assert!(flight < 80);
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::FlipLand));
    // She lands as her feet meet the floor, a few px short of the point
    // 200 px past Claw.
    let x = obj(&sim, id).x;
    assert!((x - (claw_x - 200)).abs() <= 6, "200 px past Claw: {x}");
    assert_eq!(obj(&sim, id).y, AT.1);
    // About sqrt(320 / 1000) s in the air.
    assert!((30..=34).contains(&flight), "{flight} ticks");
    let mut landing = 0;
    while state_of(&sim, id) == Some(EnemyState::FlipLand) {
        assert!(safe(&sim, id) && data(&sim, id).busy);
        sim.step(TickInput::default());
        landing += 1;
        assert!(landing < 40);
    }
    assert_eq!(landing, 23, "the landing animation's 23 ticks");
    assert!(!safe(&sim, id) && !data(&sim, id).busy);
    assert_ne!(state_of(&sim, id), Some(EnemyState::FlipLand));
}

#[test]
fn a_wall_or_a_gap_on_the_way_refuses_the_somersault_after_its_draw() {
    // Claw 150 px from the left wall: 200 px past him is inside it.
    let (mut sim, id) = in_fight(-(AT.0 - FLOOR.0 - 150));
    home_with(&mut sim, id, 2, 450, 7);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home), "refused");
    let mut r = Rng::new(7);
    r.next_u15();
    assert_eq!(sim.rng().state(), r.state(), "the draw is made anyway");
    assert!(!data(&sim, id).busy && !safe(&sim, id));
    // The launch speed of a leap of 300 px: 548 ms of flight.
    let (vx, vy) = enemy::human::launch(-300.0, 0.0, 2000.0);
    assert!((vx + 547.72).abs() < 0.01 && (vy + 547.72).abs() < 0.01);
}

#[test]
fn standing_her_exact_height_cycles_under_her_pixel() {
    let g = |f| katherine::next_stand_fraction(f, 2000.0);
    let mut f = 0.0;
    let mut seen = Vec::new();
    for _ in 0..8 {
        f = g(f);
        seen.push((f * 1000.0).round() as i32);
    }
    assert_eq!(seen, [324, 648, 972, 0, 324, 648, 972, 0]);
}

/// Her own data.
fn her(sim: &Sim, id: ObjectId) -> katherine::KatherineData {
    match &obj(sim, id).data {
        ObjectData::Katherine(k) => k.clone(),
        d => panic!("not her data: {d:?}"),
    }
}

#[test]
fn built_on_her_floor_she_stands_exactly_on_her_pixel_until_she_steps_back() {
    // Waiting for the fight, striking or waiting at Home, gravity does not
    // pull on her: her exact height is her pixel's.
    let (mut sim, id) = with_claw(3, -40, fight::NONE, 12);
    let k = her(&sim, id);
    assert!(!k.falls);
    assert_eq!(k.stand_y, 0.0);
    // Her first step back starts it, and it keeps going from then on.
    home_with(&mut sim, id, 4, 0x1FFF_FFFF, stream_with(100, 0..50));
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveBack));
    assert!(her(&sim, id).falls);
    let mut seen = Vec::new();
    for _ in 0..4 {
        sim.step(TickInput::default());
        assert_eq!(obj(&sim, id).physics_type, ptype::STAND);
        seen.push((her(&sim, id).stand_y * 1000.0).round() as i32);
    }
    assert_eq!(seen, [324, 648, 972, 0]);
}

/// The opening circle of Claw's respawn, up while the respawn runs
/// between two ticks.
fn opening_circle() -> ScreenTransition {
    ScreenTransition {
        kind: TransitionKind::Circle,
        closing: false,
        start_tick: 0,
        duration_ms: 500,
        center: Point { x: 0.0, y: 0.0 },
    }
}

#[test]
fn in_the_respawns_passes_her_strike_and_her_movement_wait_for_the_tick() {
    let (mut sim, id) = in_fight(-157);
    home_with(&mut sim, id, 2, 399, stream_with(100, 0..50));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
    // Up to the call that ends the whip.
    for n in 0.. {
        assert!(n < 100, "the whip ends");
        let mut probe = sim.clone();
        probe.with_world(|w| katherine::tick(w, id));
        if state_of(&probe, id) != Some(EnemyState::Strike) {
            break;
        }
        sim.with_world(|w| katherine::tick(w, id));
    }
    let x = obj(&sim, id).x;
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().vx = 100.0);
    // In the respawn's two passes the whip goes on and she does not move;
    // her draws are made.
    for _ in 0..2 {
        let before = sim.rng().state();
        sim.with_world(|w| {
            w.out.set_transition(Some(opening_circle()));
            assert!(katherine::between_ticks(w));
            katherine::tick(w, id);
        });
        assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
        assert_eq!(obj(&sim, id).x, x, "no movement");
        assert!(draws_between(before, sim.rng().state()) >= 1);
    }
    // The tick's own call ends it.
    sim.with_world(|w| {
        enemy::data_mut(w, id).unwrap().vx = 0.0;
        w.out.set_transition(None);
        assert!(!katherine::between_ticks(w));
        katherine::tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    // The death's closing circle is no respawn pass.
    sim.with_world(|w| {
        w.out.set_transition(Some(ScreenTransition {
            closing: true,
            ..opening_circle()
        }));
        assert!(!katherine::between_ticks(w));
    });
}

#[test]
fn her_decisions_are_made_in_the_respawns_passes() {
    for s in [
        EnemyState::Startup,
        EnemyState::Idle,
        EnemyState::Home,
        EnemyState::Pace,
        EnemyState::Advance,
        EnemyState::FastAdvance,
    ] {
        assert!(katherine::decides(s), "{s:?}");
    }
    for s in [
        EnemyState::Strike,
        EnemyState::HitLow,
        EnemyState::HitHigh,
        EnemyState::BlockLow,
        EnemyState::MoveBack,
        EnemyState::Flip,
        EnemyState::FlipLand,
        EnemyState::Lifted,
        EnemyState::Thrown,
        EnemyState::Dead,
    ] {
        assert!(!katherine::decides(s), "{s:?}");
    }
    // Pacing with Claw back in range, she goes Home in a pass, and stays
    // where she is.
    let (mut sim, id) = in_fight(-157);
    sim.with_world(|w| enemy::states::start_pacing(w, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Pace));
    let x = obj(&sim, id).x;
    sim.with_world(|w| {
        w.out.set_transition(Some(opening_circle()));
        katherine::tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).x, x);
}

// ----------------------------------------------------------------------
// The dodge

/// Her in Home with Claw `dx` px away, attacking and facing her or not,
/// her pulse roll `roll`, busy or not: one logic call, and her state.
fn dodge_call(dx: i32, facing: bool, roll_in: std::ops::Range<u32>, busy: bool) -> EnemyState {
    let (mut sim, id) = in_fight(dx);
    sim.with_world(|w| {
        katherine::go_idle(w, id);
        enemy::data_mut(w, id).unwrap().busy = busy;
    });
    let stream = stream_with(1001, roll_in);
    sim.set_rng_state(stream);
    sim.with_world(|w| {
        w.pulse = true;
        w.claw.attack.kind = attack_kind::SWIPE;
        w.claw.facing_right = facing == (dx < 0);
        logic::tick(w, id);
    });
    state_of(&sim, id).unwrap()
}

#[test]
fn she_leaps_away_from_a_facing_swing_on_a_low_roll_when_free() {
    assert_eq!(dodge_call(-60, true, 0..200, false), EnemyState::Flip);
    assert_eq!(dodge_call(-90, true, 199..200, false), EnemyState::Flip);
    assert_ne!(dodge_call(-60, true, 200..1001, false), EnemyState::Flip);
    assert_ne!(dodge_call(-60, false, 0..200, false), EnemyState::Flip);
    assert_ne!(dodge_call(-60, true, 0..200, true), EnemyState::Flip);
    assert_ne!(dodge_call(-150, true, 0..200, false), EnemyState::Flip);
}

// ----------------------------------------------------------------------
// Hits

#[test]
fn she_blocks_sword_blows_with_a_roll_below_600_unhurt() {
    for roll in [0, 499, 599] {
        let (mut sim, id) = in_fight(-60);
        let blow = attacker(&mut sim, types::PLAYER, 10);
        assert_eq!(hit(&mut sim, id, blow, roll, false), 0);
        assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
        assert_eq!(obj(&sim, id).health, 100);
        assert!(safe(&sim, id));
        assert_eq!(data(&sim, id).cooldown.len, katherine::HIT_COOLDOWN_MS);
    }
}

#[test]
fn other_sword_blows_hurt_her_and_she_only_goes_home_after() {
    let (mut sim, id) = in_fight(-60);
    let blow = attacker(&mut sim, types::PLAYER, 10);
    // A low flinch: the hit burst's three draws.
    assert_eq!(hit(&mut sim, id, blow, 600, false), 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 90);
    // During the cooldown nothing lands.
    hit(&mut sim, id, blow, 900, false);
    assert_eq!(obj(&sim, id).health, 90);
    // A blow during the flinch only sends her Home.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
    hit(&mut sim, id, blow, 900, true);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, 90);
    // An uppercut: the high flinch's sound draw, then the burst's.
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
    assert_eq!(hit(&mut sim, id, blow, 900, true), 4);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitHigh));
    // The flinch ends at Home: no strike back.
    let mut n = 0;
    while state_of(&sim, id) == Some(EnemyState::HitHigh) {
        sim.step(TickInput::default());
        n += 1;
        assert!(n < 40);
    }
    assert_ne!(state_of(&sim, id), Some(EnemyState::Strike));
}

#[test]
fn shots_magic_and_thrown_objects_are_destroyed_and_blocked() {
    for kind in [types::PISTOL_SHOT, 0x200_0000, types::THROWN] {
        let (mut sim, id) = in_fight(-120);
        let shot = attacker(&mut sim, kind, 8);
        hit(&mut sim, id, shot, 900, false);
        assert_eq!(state_of(&sim, id), Some(EnemyState::BlockLow));
        assert_eq!(obj(&sim, id).health, 100);
        assert!(obj(&sim, id).flags & flags::SAFE != 0);
        let gone = sim
            .objects()
            .get(shot)
            .is_none_or(|o| o.flags & flags::DESTROY != 0);
        assert!(gone, "{kind:#x} destroyed");
    }
}

// ----------------------------------------------------------------------
// Lifting

/// Claw `dist` px to her left, facing her, presses Lift: whether he lifted
/// her.
fn lifted_from(dist: i32) -> bool {
    let (mut sim, id) = in_fight(-dist);
    sim.step(TickInput::default());
    home(&mut sim, id);
    sim.with_world(|w| {
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().x = AT.0 - dist;
        w.objects.get_mut(id).unwrap().x = AT.0;
    });
    assert!(sim.claw().facing_right);
    sim.step(press(LIFT));
    sim.player().unwrap().state == claw_state::LIFT
        && matches!(
            state_of(&sim, id),
            Some(EnemyState::Lifted | EnemyState::Flailing)
        )
}

#[test]
fn claw_lifts_her_only_more_than_20_px_in_front_of_him() {
    assert!(lifted_from(86), "from 86 px");
    assert!(lifted_from(21));
    assert!(!lifted_from(20), "her hit box's near edge is not in front");
    assert!(!lifted_from(15), "from 15 px nothing happens");
}

#[test]
fn thrown_she_loses_4_health_and_gets_up_at_home() {
    let (mut sim, id) = in_fight(-60);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    sim.with_world(|w| {
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 300, -650);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Thrown));
    let mut seen = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id).unwrap();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        if s == EnemyState::Home || s == EnemyState::Strike {
            break;
        }
    }
    assert!(seen.contains(&EnemyState::Bounce) && seen.contains(&EnemyState::Pause));
    assert_eq!(obj(&sim, id).health, 96);
    assert_eq!(sim.boss().defeated(), None);
}

#[test]
fn a_throw_that_ends_her_releases_the_gem_with_her_death_sound() {
    let (mut sim, id) = in_fight(-60);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 4);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    sim.with_world(|w| {
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 300, -650);
    });
    let mut sound = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        sound |= cues(&sim).contains(&katherine::DEATH_SOUND.to_string());
        if sim.boss().defeated().is_some() {
            break;
        }
    }
    assert_eq!(sim.boss().defeated(), Some(id));
    assert!(sound, "her death sound");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn out_of_time_in_the_air_she_is_gone_without_the_gem() {
    let (mut sim, id) = in_fight(-60);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    sim.with_world(|w| {
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 0, -650);
        // Kept in the air past the thrown time.
        enemy::data_mut(w, id).unwrap().thrown_timer = enemy::Timer::new(0, 1);
    });
    sim.step(TickInput::default());
    idle(&mut sim, 3);
    assert_eq!(sim.boss().defeated(), None, "no gem");
    assert!(
        sim.objects()
            .all()
            .all(|(_, o)| &*o.logic != pickup::END_GEM)
    );
}

// ----------------------------------------------------------------------
// Death and the reset

#[test]
fn ten_unblocked_blows_kill_her_and_her_defeat_releases_the_gem() {
    let (mut sim, id) = in_fight(-60);
    let blow = attacker(&mut sim, types::PLAYER, 10);
    for k in 0..10 {
        sim.with_world(|w| enemy::data_mut(w, id).unwrap().cooldown = enemy::Timer::default());
        if matches!(
            state_of(&sim, id),
            Some(EnemyState::HitLow | EnemyState::HitHigh)
        ) {
            sim.with_world(|w| enemy::go_home(w, id));
        }
        hit(&mut sim, id, blow, 900, false);
        if k < 9 {
            assert_eq!(sim.boss().defeated(), None);
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(sim.boss().defeated(), Some(id));
    let gem = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_GEM)
        .map(|(_, o)| (o.x, o.y));
    assert_eq!(gem, Some(AT), "the gem rises out of her");
    let d = data(&sim, id);
    assert_eq!(d.death_anim, katherine::FALL_ANIM);
    assert_eq!((d.vx, d.vy), (150.0, -700.0), "flies off away from Claw");
}

#[test]
fn claws_respawn_puts_her_back_on_her_floor_with_full_health_in_her_state() {
    let (mut sim, id) = in_fight(-80);
    let blow = attacker(&mut sim, types::PLAYER, 10);
    hit(&mut sim, id, blow, 900, false);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().x = 2800);
    let state = state_of(&sim, id);
    // Claw loses a life: nothing while he is dying.
    sim.with_world(|w| {
        w.claw.stats.count_attempt();
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().state = claw_state::DEATH_FALL;
        katherine::tick(w, id);
    });
    assert_eq!(obj(&sim, id).health, 90);
    // He respawns: her next call puts her back, on her floor.
    sim.with_world(|w| {
        let c = w.claw_id.unwrap();
        w.objects.get_mut(c).unwrap().state = claw_state::STAND;
        katherine::tick(w, id);
    });
    let o = obj(&sim, id);
    assert_eq!(o.y, AT.1, "on the floor the same tick");
    assert!((o.x - AT.0).abs() <= 8, "near her place: {}", o.x);
    assert_eq!(o.health, 100);
    assert_eq!(state_of(&sim, id), state, "her state is kept");
    // Put exactly on her pixel, with gravity pulling on her from then on:
    // one tick's fall below it after the call.
    let k = her(&sim, id);
    assert!(k.falls);
    assert_eq!((k.stand_y * 1000.0).round(), 324.0);
    // Once per respawn.
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 40;
        katherine::tick(w, id);
    });
    assert_eq!(obj(&sim, id).health, 40);
}

// ----------------------------------------------------------------------
// Random draws

#[test]
fn every_tick_she_draws_the_block_sound_and_on_pulses_the_roll_first() {
    let (mut sim, id) = with_claw(3, -157, fight::NONE, 4);
    for _ in 0..12 {
        let before = sim.rng().state();
        let pulse = {
            let mut probe = sim.clone();
            probe.step(TickInput::default());
            probe.pulse()
        };
        sim.step(TickInput::default());
        assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
        let n = draws_between(before, sim.rng().state());
        assert_eq!(n, if pulse { 2 } else { 1 });
        // The low bit of the last draw picks the block sound.
        let mut r = Rng::new(before);
        if pulse {
            r.next_u15();
        }
        let bit = (r.next_u15() & 1) as usize;
        assert_eq!(
            data(&sim, id).human.sounds.block_low,
            katherine::BLOCK_SOUNDS[bit]
        );
    }
}
