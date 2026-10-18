//! The rules that keep the first level in step with the original along a
//! whole bot route, on synthetic levels: a goodie's spin, arming box, move
//! box, wall response, exact motion and the order of its state rule and
//! its move; a save point's effect on the random stream and on the ambient
//! timers; a human enemy built outside its patrol limits; a lying keg's
//! depth; the walk start from standing on the press of a direction; and
//! the respawn between two ticks.
//!
//! The install test `install_lockstep.rs` plays the same rules on the
//! first level, at the ticks where each one first shows on the route the
//! original was run with.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, Buttons, GameEvent, SoundCue, TickInput, TransitionKind};
use claw_sim::anim::Animation;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::checkpoint::{self, SAVE_KEY};
use claw_sim::logic::enemy;
use claw_sim::logic::keg;
use claw_sim::logic::pickup::{self, Goodie, PickupData, state as pstate};
use claw_sim::object::{ObjectId, Rect, flags};
use claw_sim::physics::ptype;
use claw_sim::player::state;
use claw_sim::rng::Rng;
use claw_sim::{Sim, SimConfig, SimLevel, TileMap};
use common::*;

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect()
}

fn obj(sim: &Sim, id: ObjectId) -> &claw_sim::object::Object {
    sim.objects().get(id).expect("object exists")
}

fn goodie(sim: &Sim, id: ObjectId) -> Goodie {
    pickup::data_of(obj(sim, id))
        .and_then(|d: &PickupData| d.goodie)
        .expect("a flying goodie")
}

fn played(audio: &[AudioCue], key: &str) -> bool {
    audio
        .iter()
        .any(|c| matches!(c, AudioCue::Play(SoundCue { key: k, .. }) if k == key))
}

/// Number of draws that take the stream from `from` to `to` (at most
/// 1000).
fn draws_between(from: u32, to: u32) -> usize {
    let mut r = Rng::new(from);
    for n in 0..=1000 {
        if r.state() == to {
            return n;
        }
        r.next_u15();
    }
    panic!("{to} is not within 1000 draws of {from}");
}

// ------------------------------------------------------------ goodies

const COINS: &str = "GAME_TREASURE_COINS";

/// The coin's four pictures, each with its own box: wide, narrower, edge
/// on, narrower again.
const COIN_BOXES: [(i32, Rect); 4] = [
    (1, Rect::new(-14, -13, 13, 14)),
    (2, Rect::new(-10, -13, 9, 14)),
    (3, Rect::new(-4, -14, 3, 13)),
    (4, Rect::new(-10, -13, 9, 14)),
];

/// The flat floor with the coin's pictures and the 100 ms spin, and, when
/// `wall_at` is set, a wall of tiles from that column on.
fn coin_level(wall_at: Option<usize>) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    if let Some(c) = wall_at {
        for r in rows.iter_mut().skip(5) {
            r.replace_range(c..c + 2, "##");
        }
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut frames = ImageFrames::range(1, 4);
    frames.boxes.extend(COIN_BOXES);
    l.images.insert(COINS.to_string(), frames);
    let spin = Animation::looping(
        pickup::GOODIE_ANIM,
        &[(1, 100), (2, 100), (3, 100), (4, 100)],
    );
    l.animations
        .insert(pickup::GOODIE_ANIM.to_string(), Arc::new(spin));
    l
}

/// Drops a coin at (x, y) moving at (vx, vy) px/s, as a defeated enemy's
/// loot does.
fn drop_coin(sim: &mut Sim, (x, y): (i32, i32), (vx, vy): (f64, f64)) -> ObjectId {
    sim.with_world(|w| {
        let id = pickup::spawn_drop(w, x, y, pickup::COIN_CODE).expect("the coin drops");
        let o = w.objects.get_mut(id).unwrap();
        if let claw_sim::ObjectData::Pickup(d) = &mut o.data {
            let g = d.goodie.as_mut().unwrap();
            g.vx = vx;
            g.vy = vy;
        }
        id
    })
}

fn frame_box(frame: i32) -> Rect {
    COIN_BOXES
        .iter()
        .find(|b| b.0 == frame)
        .map(|b| b.1)
        .expect("a coin picture")
}

#[test]
fn a_goodie_spins_and_is_armed_with_the_box_it_shows() {
    let mut sim = Sim::new(coin_level(None), SimConfig::default());
    let id = drop_coin(&mut sim, (1300, 700), (0.0, -400.0));
    // Made showing the set's first picture, moving with that picture's box
    // grown 10 px left, up and right.
    let o = obj(&sim, id);
    assert_eq!(o.frame, 1);
    assert_eq!(o.move_rect, Rect::new(-24, -23, 23, 14));
    assert!(o.flags & flags::NO_COLLIDE != 0);
    let mut frames = Vec::new();
    let mut armed = None;
    for k in 0..12 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        frames.push(o.frame);
        if armed.is_none() && o.flags & flags::NO_COLLIDE == 0 {
            armed = Some((k, o.frame, o.attack_rect));
        }
    }
    // The spin shows the next picture every 100 ms (six calls here).
    assert_eq!(frames, [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2]);
    // It can be taken from its seventh call (125 ms), with the box of the
    // picture it shows then (the top raised 10 px), not the first one's.
    let (k, frame, rect) = armed.expect("armed");
    assert_eq!(k, 6);
    assert_ne!(frame, 1, "{frames:?}");
    assert_eq!(rect, pickup::goodie_box(frame_box(frame)));
    // The box stays once armed, whatever the spin shows.
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).attack_rect, rect);
}

#[test]
fn a_goodie_moves_on_an_exact_position() {
    let mut sim = Sim::new(coin_level(None), SimConfig::default());
    let id = drop_coin(&mut sim, (1300, 700), (-37.0, -403.0));
    sim.step(TickInput::default());
    // One tick of constant acceleration: 0.018 * v + 400 * 0.018^2.
    let g = goodie(&sim, id);
    let dt = 0.018;
    let want = (1300.0 - 37.0 * dt, 700.0 - 403.0 * dt + 400.0 * dt * dt);
    assert!((g.pos.0 - want.0).abs() < 1e-9 && (g.pos.1 - want.1).abs() < 1e-9);
    assert!((g.vy - (-403.0 + 800.0 * dt)).abs() < 1e-9);
    // Shown at the whole pixels of the exact position: 1299.334 and
    // 692.8756 (the speed after the step would give 693).
    let o = obj(&sim, id);
    assert_eq!((o.x, o.y), (1299, 692));
    // Many ticks on, the position is still the exact one.
    for _ in 0..9 {
        sim.step(TickInput::default());
    }
    let g = goodie(&sim, id);
    let t = 10.0 * dt;
    let y = 700.0 - 403.0 * t + 400.0 * t * t;
    assert!((g.pos.1 - y).abs() < 1e-6, "{} {y}", g.pos.1);
    assert_eq!(obj(&sim, id).y, y.trunc() as i32);
}

/// A falling coin at whole pixel `y` whose exact height is `y + frac`,
/// found landed by its next call, falling at 300 px/s; one tick is
/// stepped. Returns its whole y and its exact position after the tick.
fn bounce_from(frac: f64) -> (i32, Goodie) {
    let mut sim = Sim::new(coin_level(None), SimConfig::default());
    let id = drop_coin(&mut sim, (1300, 700), (0.0, 300.0));
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.physics_type = ptype::STAND;
        if let claw_sim::ObjectData::Pickup(d) = &mut o.data {
            d.goodie.as_mut().unwrap().pos.1 = 700.0 + frac;
        }
    });
    sim.step(TickInput::default());
    let o = obj(&sim, id);
    assert_eq!(o.state, pstate::GOODIE_BOUNCING, "it bounced");
    (o.y, goodie(&sim, id))
}

#[test]
fn a_goodie_bounce_lifts_its_exact_height_by_one_and_keeps_the_fraction() {
    // A landing the floor did not stop short keeps the fraction of its
    // last move (1764.7374 was seen on the first level): the bounce lifts
    // the exact height by 1.0 and the hop starts from there, not from the
    // whole pixel above.
    let (y0, whole) = bounce_from(0.0);
    let (y1, part) = bounce_from(0.7374);
    assert_eq!(whole.vy, part.vy, "the same hop");
    assert!(whole.vy < 0.0, "going up");
    let lifted = part.pos.1 - whole.pos.1;
    assert!(
        (lifted - 0.7374).abs() < 1e-9,
        "the fraction is kept: {lifted}"
    );
    // The whole bounce starts from 699.0, 1 px above where it landed, and
    // rises 2.55 px in its first tick: the two are a pixel apart.
    let dt = 0.018;
    let v0 = whole.vy - pickup::GOODIE_GRAVITY * dt;
    let hop = v0 * dt + 0.5 * pickup::GOODIE_GRAVITY * dt * dt;
    assert!(
        (whole.pos.1 - (699.0 + hop)).abs() < 1e-9,
        "{}",
        whole.pos.1
    );
    assert_eq!((y0, y1), (696, 697));
}

#[test]
fn a_wall_turns_a_goodie_around_at_half_speed() {
    // A wall from x 1408 on: the coin's move box reaches 23 px to its
    // right, so the wall stops it with its position at most 1384.
    let mut sim = Sim::new(coin_level(Some(22)), SimConfig::default());
    let id = drop_coin(&mut sim, (1370, 700), (200.0, -100.0));
    let mut turned = None;
    for k in 0..30 {
        let x_before = obj(&sim, id).x;
        sim.step(TickInput::default());
        let g = goodie(&sim, id);
        if g.vx < 0.0 {
            turned = Some((k, x_before, obj(&sim, id).x, g));
            break;
        }
    }
    let (_, before, after, g) = turned.expect("the wall turned it");
    assert_eq!(g.vx, -100.0, "half speed back");
    assert!(
        after <= 1408 - 24,
        "kept off the wall by its move box: {after}"
    );
    assert!(after >= before, "stopped where the wall is");
    assert_eq!(
        g.pos.0,
        f64::from(after),
        "the exact position is where it stopped"
    );
    // It flies back to the left.
    for _ in 0..10 {
        sim.step(TickInput::default());
    }
    assert!(obj(&sim, id).x < after);
}

#[test]
fn a_goodie_acts_on_its_landing_in_its_next_call() {
    let mut sim = Sim::new(coin_level(None), SimConfig::default());
    let id = drop_coin(&mut sim, (1300, 900), (0.0, 0.0));
    let mut landings = Vec::new();
    let mut bounces = Vec::new();
    let mut settled = None;
    for k in 0..400 {
        let snap = sim.step(TickInput::default());
        let o = obj(&sim, id);
        if &*o.logic != pickup::BOUNCING_GOODIE {
            settled = Some(k);
            break;
        }
        if played(&snap.audio, "GAME_PUBOUNCE1") {
            bounces.push(k);
        }
        if o.physics_type == ptype::STAND {
            landings.push(k);
            // Landed by this call's move: the state rule ran before it.
            assert_ne!(o.state, pstate::GOODIE_DYING);
        }
    }
    assert_eq!(landings.len(), 4, "{landings:?}");
    // Each landing is acted on (the bounce and its sound) in the next call.
    let next: Vec<usize> = landings.iter().map(|k| k + 1).collect();
    assert_eq!(bounces, next);
    // After the fourth bounce, the next call that finds it landed again is
    // not needed: the call after the fourth bounce turns it into a pickup.
    assert_eq!(settled, Some(bounces[3] + 1));
}

// --------------------------------------------------------- save points

/// The flat level with a save point Claw reaches while standing at the
/// start.
fn save_level() -> SimLevel {
    let mut l = flat_level();
    l.objects
        .push(ObjectSpec::at("FirstSuperCheckpoint", 1000, 899 + 160, 5));
    l
}

/// Steps with nothing held until the save point saves (or `n` ticks);
/// returns the tick it saved in.
fn until_saved(sim: &mut Sim, n: usize) -> Option<u64> {
    for _ in 0..n {
        let snap = sim.step(TickInput::default());
        if snap.events.contains(&GameEvent::SavePoint { slot: 1 }) {
            return Some(sim.tick() - 1);
        }
    }
    None
}

#[test]
fn a_save_leaves_the_random_stream_in_the_saves_own_state() {
    let mut r = Rng::new(12345);
    checkpoint::after_save(&mut r);
    assert_eq!(r.state(), 1_695_759_300);
    let mut once = Rng::new(SAVE_KEY);
    once.next_u15();
    assert_eq!(once.state(), r.state(), "three times the same draw");
    // Whatever the seed, the stream is the save's after the tick that
    // saved.
    for seed in [1, 4_105_512_405] {
        let cfg = SimConfig {
            seed,
            ..SimConfig::default()
        };
        let mut sim = pressing_sim(save_level(), cfg);
        assert!(until_saved(&mut sim, 50).is_some());
        assert_eq!(sim.rng_state(), 1_695_759_300, "seed {seed}");
    }
    // Not saved after a cheat: the stream goes on.
    let mut sim = pressing_sim(save_level(), SimConfig::default());
    sim.with_world(|w| w.claw.stats.cheated = true);
    assert_eq!(until_saved(&mut sim, 50), None);
    assert!(sim.claw().checkpoint.is_some(), "the flag was touched");
    assert_ne!(sim.rng_state(), 1_695_759_300);
}

const CRIES: &str = "LEVEL_AMBIENT_CRIES";

/// [`save_level`] with one timed global ambient sound off for 540 ms at
/// first, or the plain flat level with that sound.
fn ambient_level(save: bool) -> SimLevel {
    let mut l = if save { save_level() } else { flat_level() };
    l.sounds.insert(CRIES.into(), Some(1000));
    let mut s = ObjectSpec::at("GlobalAmbientSound", 1000, 900, 0);
    s.anim = CRIES.into();
    s.record[16] = 80;
    s.record[19] = 400;
    s.record[20] = 900;
    s.record[21] = 540;
    s.record[22] = 540;
    l.objects.push(s);
    l
}

/// The tick in which the ambient timer first flips (draws), and the tick
/// that saved, if any.
fn first_flip(save: bool) -> (u64, Option<u64>) {
    let mut sim = pressing_sim(ambient_level(save), SimConfig::default());
    let mut saved = None;
    for _ in 0..60 {
        let before = sim.rng_state();
        let snap = sim.step(TickInput::default());
        let k = sim.tick() - 1;
        if snap.events.contains(&GameEvent::SavePoint { slot: 1 }) {
            saved = Some(k);
            continue;
        }
        if draws_between(before, sim.rng_state()) > 0 {
            return (k, saved);
        }
    }
    panic!("no flip");
}

#[test]
fn the_ambient_timers_count_no_time_in_the_tick_of_a_save() {
    // Without a save the 540 ms timer flips in its thirtieth tick; the
    // tick of a save counts nothing, so with one the flip comes a tick
    // later.
    let (plain, none) = first_flip(false);
    assert_eq!((plain, none), (29, None));
    let (later, saved) = first_flip(true);
    assert!(saved.is_some_and(|k| k < plain), "{saved:?}");
    assert_eq!(later, plain + 1);
}

// ------------------------------------------------------------- enemies

#[test]
fn a_human_enemy_placed_beside_its_limits_starts_on_the_nearer_one() {
    // The floor is raised by a tile from x 1216 (column 19) on. A soldier
    // placed over the raised floor 55 px right of its range starts on its
    // limit, on the lower floor below the limit, before it ever moves.
    let rows: Vec<String> = (0..17)
        .map(|r| match r {
            0..=13 => ".".repeat(200),
            14 => format!("{}{}", ".".repeat(19), "#".repeat(181)),
            _ => "#".repeat(200),
        })
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let floor_at = |x: i32| if x < 19 * 64 { 15 * 64 } else { 14 * 64 };
    for (logic, x, (min, max), want) in [
        ("Soldier", 1255, (1194, 1200), 1200),
        ("Officer", 1100, (1150, 1300), 1150),
        ("Officer", 1250, (1150, 1300), 1250),
        ("Soldier", 1255, (0, 0), 1255),
    ] {
        let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 800));
        let mut o = ObjectSpec::at(logic, x, 800, 4000);
        o.image_set = format!("LEVEL_{}", logic.to_uppercase());
        o.record[enemy::field::X_MIN] = min;
        o.record[enemy::field::X_MAX] = max;
        l.objects.push(o);
        let sim = Sim::new(l, SimConfig::default());
        let e = find(&sim, logic)[0];
        let o = obj(&sim, e);
        assert_eq!(o.x, want, "{logic} at {x} within {min}..{max}");
        let feet = o.y + o.move_rect.bottom;
        assert_eq!(
            feet + 1,
            floor_at(want),
            "{logic} stands on the floor at {want}"
        );
    }
}

// ---------------------------------------------------------------- kegs

#[test]
fn a_lying_keg_sorts_its_depth_against_claws_feet() {
    let mut l = flat_level();
    let mut k = ObjectSpec::at("PowderKeg", 1200, 890, 0);
    k.image_set = keg::IMAGE_SET.to_string();
    l.objects.push(k);
    let mut sim = Sim::new(l, SimConfig::default());
    let id = find(&sim, "PowderKeg")[0];
    sim.step(TickInput::default());
    // On the same floor as Claw: in front of him (his feet are not below
    // its own).
    assert_eq!(obj(&sim, id).z, keg::Z_BASE + 4000);
    // Lifted above Claw's feet, it sorts behind him from its next call and
    // runs before him in the update pass.
    sim.with_world(|w| w.objects.get_mut(id).unwrap().y -= 200);
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, id).z, keg::Z_BASE + 3899);
    assert_eq!(obj(&sim, id).z, 3919);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    let listed = sim.objects().listed();
    let at = |x: ObjectId| listed.iter().position(|&i| i == x).unwrap();
    assert!(at(id) < at(claw));
}

// ---------------------------------------------------------------- Claw

#[test]
fn standing_claw_walks_on_the_press_of_a_direction() {
    let mut sim = pressing_sim(flat_level(), SimConfig::default());
    idle(&mut sim, 5);
    let x0 = sim.player().unwrap().x;
    // Held without a press (the press came while he could not walk), he
    // keeps standing.
    for _ in 0..10 {
        sim.step(hold(RIGHT));
    }
    assert_eq!(sim.player().unwrap().x, x0);
    assert_eq!(sim.player().unwrap().state, state::STAND);
    // A press starts the walk at once, and holding keeps it going.
    sim.step(TickInput::default());
    sim.step(press(LEFT));
    assert_eq!(sim.player().unwrap().state, state::MOVE_LEFT);
    for _ in 0..5 {
        sim.step(hold(LEFT));
    }
    assert!(sim.player().unwrap().x < x0);
}

/// The flat level with Claw's blows: a short strike, its live element 125
/// ms.
fn blow_level() -> SimLevel {
    let mut l = flat_level();
    let strike = [(1, 25), (2, 25), (3, 125), (2, 35), (1, 25)];
    for key in [
        "CLAW_SWIPE",
        "CLAW_PUNCH",
        "CLAW_UPPERCUT",
        "CLAW_KICK",
        "CLAW_DUCKSWIPE",
        "CLAW_JUMPSWIPE",
    ] {
        let a = Animation::once(key, &strike);
        l.animations.insert(key.to_string(), Arc::new(a));
    }
    l
}

#[test]
fn a_direction_held_through_an_attack_walks_after_its_end() {
    let mut sim = pressing_sim(blow_level(), SimConfig::default());
    idle(&mut sim, 5);
    let x0 = sim.player().unwrap().x;
    sim.step(press(Buttons::ATTACK.0));
    assert!(sim.claw().attacking, "the blow starts");
    // Right pressed while the blow runs and held on: Claw stands until the
    // blow is over.
    sim.step(press(RIGHT));
    let mut ticks = 0;
    while sim.claw().attacking {
        assert_eq!(sim.player().unwrap().x, x0);
        assert_eq!(sim.player().unwrap().state, state::STAND);
        sim.step(hold(RIGHT));
        ticks += 1;
        assert!(ticks < 100);
    }
    assert!(ticks > 3, "the blow outlasted the press");
    // The tick that ended the blow took Right off the latch: held, it
    // counts as pressed in the next tick, and he walks.
    assert_eq!(sim.player().unwrap().x, x0);
    sim.step(hold(RIGHT));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::MOVE_RIGHT);
    assert!(o.x > x0);
}

// ---------------------------------------------------------------- deaths

/// A flat floor with death tiles in columns 25 to 27 (x 1600 to 1791);
/// Claw starts at x 1000.
fn deadly_level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(25..28, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// Walks Claw right into the death tiles and lets go once he is dying;
/// returns his start point and the snapshot of the tick that took the
/// life, with the respawn waiting.
fn die(sim: &mut Sim) -> ((i32, i32), claw_sim::SimSnapshot) {
    idle(sim, 3);
    let start = (sim.player().unwrap().x, sim.player().unwrap().y);
    sim.step(press(RIGHT));
    for _ in 0..400 {
        let dying = sim.player().unwrap().state == state::DEATH;
        let snap = sim.step(if dying {
            TickInput::default()
        } else {
            hold(RIGHT)
        });
        if snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }) {
            return (start, snap);
        }
    }
    panic!("Claw did not die on the death tiles");
}

#[test]
fn claw_respawns_between_two_ticks() {
    let mut sim = pressing_sim(deadly_level(), SimConfig::default());
    let (start, snap) = die(&mut sim);
    // The tick ends with Claw dead; the closing circle is reported for the
    // presentation only.
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::DEATH);
    assert!(sim.respawn_pending());
    let t = snap.transition.expect("the closing circle");
    assert!(t.closing && t.kind == TransitionKind::Circle);
    // The next tick finds him back at the start, standing at full health,
    // and a direction held then walks at once.
    let tick = sim.tick();
    let snap = sim.step(hold(RIGHT));
    assert_eq!(sim.tick(), tick + 1, "no game time for the circles");
    assert!(!sim.respawn_pending());
    assert_eq!(snap.transition, None);
    let o = sim.player().unwrap();
    assert_eq!(o.y, start.1);
    assert_eq!(o.state, state::MOVE_RIGHT);
    assert!(
        o.x > start.0 && o.x < start.0 + 20,
        "{} from {}",
        o.x,
        start.0
    );
    assert_eq!(sim.stats().health, 100);
    assert!(o.flags & flags::SAFE == 0, "not frozen or safe");
    assert!(
        played(&snap.audio, "GAME_FLAGWAVE"),
        "back at the level start"
    );
}

/// A flying coin at (x, y) that sleeps out of reach like a small object
/// (a goodie is otherwise always awake).
fn sleeping_coin(sim: &mut Sim, x: i32, y: i32) -> ObjectId {
    sim.with_world(|w| {
        let id = pickup::spawn_drop(w, x, y, pickup::COIN_CODE).unwrap();
        w.objects.get_mut(id).unwrap().flags &= !flags::ALWAYS_ACTIVE;
        id
    })
}

#[test]
fn the_respawn_runs_a_pass_at_the_death_then_one_after_the_objects_around_claw_wake() {
    // Death tiles far from the level start, and a rat walking near them.
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(150..153, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut rat = ObjectSpec::at("Rat", 9200, 926, 0);
    rat.image_set = "LEVEL_RAT".to_string();
    level.objects.push(rat);
    let mut sim = pressing_sim(level, SimConfig::default());
    idle(&mut sim, 3);
    let start = (sim.player().unwrap().x, sim.player().unwrap().y);
    sim.teleport_player(9300, 899);
    idle(&mut sim, 3);
    sim.step(press(RIGHT));
    // A coin by the start while Claw dies far from it: it goes to sleep.
    let mut asleep = None;
    let mut died = false;
    for _ in 0..400 {
        let dying = sim.player().unwrap().state == state::DEATH;
        if dying && asleep.is_none() {
            asleep = Some(sleeping_coin(&mut sim, 1100, 300));
        }
        let snap = sim.step(if dying {
            TickInput::default()
        } else {
            hold(RIGHT)
        });
        if snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }) {
            died = true;
            break;
        }
    }
    assert!(died, "Claw died on the death tiles");
    let asleep = asleep.expect("the coin dropped while Claw died");
    assert!(
        !sim.objects().listed().contains(&asleep),
        "the coin by the start sleeps"
    );
    let slept_age = goodie(&sim, asleep).age_ms;
    let rat = find(&sim, "Rat")[0];
    let walking = obj(&sim, rat).clone();
    assert!(
        matches!(
            enemy::EnemyState::from_id(walking.state),
            Some(enemy::EnemyState::MoveLeft | enemy::EnemyState::MoveRight)
        ),
        "the rat is awake and walking when Claw dies"
    );
    // Two coins awake in the air by the start when the life is lost.
    let awake = sim.with_world(|w| pickup::spawn_drop(w, 1100, 300, pickup::COIN_CODE).unwrap());
    let near = sleeping_coin(&mut sim, 1150, 300);
    let age = goodie(&sim, awake).age_ms;
    assert!(sim.respawn_now());
    assert_eq!((sim.player().unwrap().x, sim.player().unwrap().y), start);
    // Two update passes: the coins awake by the start ran twice, whether
    // they are always awake or within the camera's reach.
    assert_eq!(goodie(&sim, awake).age_ms, age + 2 * 18, "two calls");
    assert_eq!(goodie(&sim, near).age_ms, age + 2 * 18, "two calls");
    // The rat by the death tiles was still awake for the first pass, but
    // its move counts the game time since its last one, made in the tick
    // that took the life: it ran without moving, then went to sleep before
    // the second.
    let o = obj(&sim, rat);
    assert_eq!((o.x, o.y, o.state), (walking.x, walking.y, walking.state));
    assert!(!sim.objects().listed().contains(&rat), "asleep after it");
    // The sleeping coin by the start woke between the passes: it ran once.
    assert!(sim.objects().listed().contains(&asleep), "woken");
    assert_eq!(goodie(&sim, asleep).age_ms, slept_age + 18, "one call");
}

#[test]
fn the_respawns_update_passes_count_no_time() {
    // Death tiles 300 px from the start, inside the camera's reach, and a
    // cycling sign by the start.
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(20..23, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut sign = ObjectSpec::at("AniCycle", 1100, 900, 0);
    sign.image_set = "LEVEL_ARROWSIGN_RIGHT".into();
    sign.frame = 1;
    level.objects.push(sign);
    level
        .images
        .insert("LEVEL_ARROWSIGN_RIGHT".into(), ImageFrames::range(1, 3));
    level.images.insert(COINS.into(), ImageFrames::range(1, 4));
    let mut sim = pressing_sim(level, SimConfig::default());
    let (start, _) = die(&mut sim);
    let sign = find(&sim, "AniCycle")[0];
    assert!(sim.objects().listed().contains(&sign), "awake by the death");
    // A coin by the start waiting out a time delay.
    let coin = sim.with_world(|w| {
        let id = pickup::spawn_drop(w, 1050, 300, pickup::COIN_CODE).unwrap();
        w.objects.get_mut(id).unwrap().time_delay = 100;
        id
    });
    let camera = sim.camera().current;
    let sign_before = format!("{:?}", obj(&sim, sign));
    let coin_age = goodie(&sim, coin).age_ms;
    assert!(
        (camera.0 - start.0).abs() < 500,
        "within the camera's reach"
    );
    assert!(sim.respawn_now());
    // The camera has not moved towards the respawned Claw, the sign's
    // animation stands still and the coin's time delay has not run down.
    assert_eq!(sim.camera().current, camera, "no camera step");
    assert_eq!(
        format!("{:?}", obj(&sim, sign)),
        sign_before,
        "nothing moved"
    );
    assert_eq!(obj(&sim, coin).time_delay, 100);
    assert_eq!(goodie(&sim, coin).age_ms, coin_age, "not run");
    // The next tick takes the camera's first 9 px step.
    sim.step(TickInput::default());
    let now = sim.camera().current;
    assert_eq!(now.0, camera.0 - 9, "one step towards {start:?}");
}

/// Walks Claw into the death tiles of [`deadly_level`] and presses `mask`
/// in the tick that takes the life (the dead Claw ignores it); returns his
/// start point, with the respawn waiting.
fn die_pressing(sim: &mut Sim, mask: u32) -> (i32, i32) {
    idle(sim, 3);
    let start = (sim.player().unwrap().x, sim.player().unwrap().y);
    sim.step(press(RIGHT));
    for _ in 0..400 {
        let quiet = if sim.player().unwrap().state == state::DEATH {
            TickInput::default()
        } else {
            hold(RIGHT)
        };
        let mut probe = sim.clone();
        let lost = probe
            .step(quiet)
            .events
            .contains(&GameEvent::ClawDied { lives_left: 6 });
        if lost {
            let snap = sim.step(press(mask));
            assert!(
                snap.events.contains(&GameEvent::ClawDied { lives_left: 6 }),
                "{mask:#x}: the press does not save the life"
            );
            assert!(sim.respawn_pending());
            return start;
        }
        sim.step(quiet);
    }
    panic!("{mask:#x}: Claw did not die on the death tiles");
}

/// Claw's place after `ticks` ticks of walking right from a standing
/// start at the level start of [`deadly_level`].
fn walked_from_start(ticks: usize) -> i32 {
    let mut sim = pressing_sim(deadly_level(), SimConfig::default());
    idle(&mut sim, 3);
    sim.step(press(RIGHT));
    for _ in 1..ticks {
        sim.step(hold(RIGHT));
    }
    sim.player().unwrap().x
}

#[test]
fn a_press_in_the_tick_that_takes_a_life_is_seen_by_the_respawned_claw() {
    // The respawn's two update passes run with the keys of the tick that
    // took the life, pressed ones included, and Claw's movement counts a
    // whole tick in each pass though the frame time is 0.
    let (one, two, three) = (
        walked_from_start(1),
        walked_from_start(2),
        walked_from_start(3),
    );
    for mask in [RIGHT, JUMP | RIGHT] {
        let mut sim = pressing_sim(deadly_level(), SimConfig::default());
        let start = die_pressing(&mut sim, mask);
        assert!(sim.respawn_now());
        assert_eq!(sim.claw_input().pressed, Buttons(mask), "{mask:#x}");
        let o = sim.player().unwrap();
        if mask == RIGHT {
            // A walk started in the first pass: two walking steps.
            assert_eq!((o.x, o.y, o.state), (two, start.1, state::MOVE_RIGHT));
        } else {
            assert_eq!((o.x, o.state), (two, state::JUMP), "{mask:#x}");
            assert!(o.y < start.1, "{mask:#x}: off the floor");
        }
        // The camera, 600 px from the respawned Claw, jumped to him in its
        // own call of the first pass, after his first step, and did not
        // move in the second pass.
        assert_eq!(sim.camera().current.0, one, "{mask:#x}");
        // The next tick is the third walking step.
        sim.step(hold(mask));
        assert_eq!(sim.player().unwrap().x, three, "{mask:#x}");
    }
    // A jump pressed in that tick takes him off the floor in the passes.
    let mut sim = pressing_sim(deadly_level(), SimConfig::default());
    let start = die_pressing(&mut sim, JUMP);
    assert!(sim.respawn_now());
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.state), (start.0, state::JUMP));
    assert!(o.y < start.1, "off the floor");
}

#[test]
fn a_presentation_respawns_claw_before_drawing_the_opening_circle() {
    let mut sim = pressing_sim(deadly_level(), SimConfig::default());
    let (start, _) = die(&mut sim);
    let mut direct = sim.clone();
    let tick = sim.tick();
    assert!(sim.respawn_now());
    assert!(!sim.respawn_now(), "once");
    assert_eq!(sim.tick(), tick);
    assert_eq!((sim.player().unwrap().x, sim.player().unwrap().y), start);
    assert!(played(sim.last_audio(), "GAME_FLAGWAVE"));
    let t = sim.transition().expect("the opening circle");
    assert!(!t.closing);
    assert_eq!(
        (t.center.x, t.center.y),
        (f64::from(start.0), f64::from(start.1 - 32))
    );
    // The next tick is the same game as one that respawned by itself.
    let a = sim.step(hold(RIGHT));
    let b = direct.step(hold(RIGHT));
    assert_eq!(a.transition, None);
    assert_eq!(sim.rng_state(), direct.rng_state());
    assert_eq!(
        (sim.player().unwrap().x, sim.player().unwrap().state),
        (direct.player().unwrap().x, direct.player().unwrap().state)
    );
    assert_eq!(a.renderables, b.renderables);
}
