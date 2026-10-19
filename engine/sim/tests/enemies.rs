//! The officer, the soldier and the rat on synthetic levels: range boxes
//! and Home, patrols turning at their limits, the soldier's shots and the
//! rat's bombs (read from the spawn requests), hit reactions by what hit,
//! death and loot, the strike token, being lifted and thrown, and the
//! order of random draws of each class.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{
    self, DeathKind, EnemyState, lift, motion, officer, rat, soldier, states,
};
use claw_sim::logic::pickup::PickupData;
use claw_sim::logic::projectile::{ProjectileData, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, ObjectData, Sim, SimConfig, SimLevel};
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

/// The enemies' animations, shaped like the game's: strikes, shots and
/// throws with one event frame, flinches of about a third of a second.
fn enemy_anims() -> Vec<Arc<Animation>> {
    let strike = [
        (200, 400, 0),
        (202, 40, 0),
        (204, 250, 1),
        (202, 40, 0),
        (201, 0, 0),
    ];
    let flinch = [(600, 50, 0), (601, 400, 0), (600, 72, 0), (201, 0, 0)];
    let mut v = Vec::new();
    for c in ["OFFICER", "SOLDIER"] {
        let k = |s: &str| format!("LEVEL_{c}_{s}");
        v.push(anim(&k("STRIKE1"), &strike, true));
        v.push(anim(&k("HITLOW"), &flinch, true));
        v.push(anim(&k("HITHIGH"), &flinch, true));
        v.push(anim(&k("FALL"), &[(950, 100, 0)], false));
        v.push(anim(&k("HOME"), &[(201, 0, 0)], true));
        v.push(anim(&k("FASTADVANCE"), &[(1, 100, 0), (2, 100, 0)], false));
        v.push(anim(&k("ADVANCE"), &[(3, 100, 0), (4, 100, 0)], false));
        v.push(anim(&k("JUMP"), &[(5, 100, 0)], false));
        v.push(anim(&k("LIFTED"), &[(6, 100, 0)], false));
        v.push(anim(&k("LIFTED2"), &[(7, 100, 0)], false));
        v.push(anim(&k("THROWN"), &[(8, 100, 0)], false));
    }
    v.push(anim("LEVEL_OFFICER_STRIKE", &strike, true));
    for key in officer::STAND {
        v.push(anim(key, &[(100, 0, 0)], true));
    }
    // The soldier fires on the event of its STRIKE animation.
    v.push(anim(
        "LEVEL_SOLDIER_STRIKE",
        &[(300, 200, 0), (301, 100, 1), (302, 300, 0), (300, 0, 0)],
        true,
    ));
    for key in soldier::STAND {
        v.push(anim(key, &[(110, 0, 0)], true));
    }
    v.push(anim(rat::WALK_ANIM, &[(1, 100, 0), (2, 100, 0)], false));
    let throw = [(10, 150, 0), (11, 100, 1), (12, 150, 0), (12, 0, 0)];
    v.push(anim(rat::THROW_ACROSS_ANIM, &throw, true));
    v.push(anim(rat::THROW_DOWN_ANIM, &throw, true));
    v.push(anim(rat::DEATH_ANIM, &[(20, 100, 0)], false));
    v
}

/// The sounds the enemies say, with their lengths.
const LINES: [(&str, u32); 8] = [
    (officer::TAUNT, 900),
    (officer::MUTTERS[0], 600),
    (officer::MUTTERS[1], 600),
    (officer::LIFTED_LINES[0], 700),
    (officer::LIFTED_LINES[1], 700),
    (soldier::ALERT, 800),
    (soldier::LIFTED_LINE, 700),
    ("CLAW_1001", 900),
];

/// A level of 64 px tiles from text rows (see [`TileMap::from_ascii`]),
/// with Claw starting at `start`, the enemies' animations and lines, and
/// the given records.
fn level(rows: &[&str], start: (i32, i32), objects: Vec<ObjectSpec>) -> SimLevel {
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, rows), start);
    for a in enemy_anims() {
        l.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in LINES {
        l.sounds.insert(k.to_string(), Some(ms));
    }
    l.objects = objects;
    l
}

/// The flat floor of the common helpers (feet rest on 959) with the
/// enemies' animations and the given records.
fn arena(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut l = flat_level();
    for a in enemy_anims() {
        l.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in LINES {
        l.sounds.insert(k.to_string(), Some(ms));
    }
    l.objects = objects;
    l
}

/// A record of `logic` at (x, y) with its image set.
fn record(logic: &str, x: i32, y: i32) -> ObjectSpec {
    let mut o = ObjectSpec::at(logic, x, y, if logic == "Rat" { 0 } else { 4000 });
    o.image_set = format!("LEVEL_{}", logic.to_uppercase());
    o
}

/// A record with patrol limits.
fn bounded(logic: &str, x: i32, y: i32, min: i32, max: i32) -> ObjectSpec {
    let mut o = record(logic, x, y);
    o.record[enemy::field::X_MIN] = min;
    o.record[enemy::field::X_MAX] = max;
    o
}

/// Gives Claw health enough to stand a long fight.
fn tough_claw(sim: &mut Sim) {
    sim.with_world(|w| w.claw.stats.health = 10_000);
}

/// A simulation of `level` in which every object is awake from the start
/// (activation windows twice the level's size, centred inside it).
fn sim_with(level: SimLevel, seed: u32) -> Sim {
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

fn enemies(sim: &Sim, g: GameLogic) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(g))
        .map(|(id, _)| id)
        .collect();
    v.sort_unstable();
    v
}

fn first(sim: &Sim, g: GameLogic) -> ObjectId {
    enemies(sim, g)[0]
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

fn data(sim: &Sim, id: ObjectId) -> enemy::EnemyData {
    enemy::data_of(obj(sim, id)).expect("built").clone()
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

/// The projectile spawn requests that exist, by object, in the order they were made (by
/// creation number: a new object may take a lower slot, and id, freed by
/// a removed one).
fn shots(sim: &Sim) -> Vec<(ObjectId, ProjectileSpec)> {
    let mut v: Vec<(u32, ObjectId, ProjectileSpec)> = sim
        .objects()
        .all()
        .filter_map(|(id, o)| match &o.data {
            ObjectData::Projectile(ProjectileData {
                spec: Some(spec), ..
            }) => Some((o.serial, id, *spec)),
            _ => None,
        })
        .collect();
    v.sort_unstable_by_key(|s| s.0);
    v.into_iter().map(|(_, id, spec)| (id, spec)).collect()
}

/// The projectile spawn requests that appeared since `seen` was last
/// updated, in spawn order. Shots fly and are removed when they hit or
/// leave, so tests note them by id as they step.
fn new_shots(sim: &Sim, seen: &mut BTreeSet<ObjectId>) -> Vec<ProjectileSpec> {
    shots(sim)
        .into_iter()
        .filter(|(id, _)| seen.insert(*id))
        .map(|(_, spec)| spec)
        .collect()
}

/// Removes every projectile, so that no shot reaches Claw.
fn clear_shots(sim: &mut Sim) {
    let ids: Vec<ObjectId> = shots(sim).into_iter().map(|(id, _)| id).collect();
    sim.with_world(|w| {
        for id in ids {
            w.objects.remove(id);
        }
    });
}

/// Moves Claw far to the right, out of every enemy's sight and range.
fn send_claw_away(sim: &mut Sim) {
    sim.teleport_player(9000, 899);
    idle(sim, 2);
}

// ----------------------------------------------------------------------
// Range boxes, Home and Strike

#[test]
fn claw_in_range_sends_the_officer_home_and_to_strike() {
    let mut sim = sim_with(arena(vec![record("Officer", 1080, 900)]), 3);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Officer);
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (3, 10));
    assert_eq!(feet_of(o), 959, "dropped onto the floor");
    // Claw is 80 px away: in range. The officer decides at Home and
    // strikes; it strikes again and again.
    let mut seen = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id).unwrap();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
    }
    assert!(
        seen.windows(2)
            .any(|p| p == [EnemyState::Home, EnemyState::Strike]),
        "{seen:?}"
    );
    assert!(
        seen.iter().filter(|&&s| s == EnemyState::Strike).count() >= 3,
        "{seen:?}"
    );
}

fn feet_of(o: &Object) -> i32 {
    o.y + o.move_rect.bottom
}

#[test]
fn out_of_range_the_officer_stays_idle_until_claw_comes() {
    // 300 px away: out of the 100 px boxes.
    let mut sim = sim_with(arena(vec![record("Officer", 1300, 900)]), 5);
    idle(&mut sim, 150);
    let id = first(&sim, GameLogic::Officer);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    assert_eq!(data(&sim, id).mode, 5, "out of range, on screen");
    // Claw walks up: in range it goes Home and strikes.
    sim.teleport_player(1220, 899);
    let mut struck = false;
    for _ in 0..60 {
        sim.step(TickInput::default());
        struck |= state_of(&sim, id) == Some(EnemyState::Strike);
    }
    assert!(struck);
    assert_eq!(data(&sim, id).mode, 4);
}

#[test]
fn the_strike_box_is_live_on_one_tick_and_low_when_claw_crouches() {
    let mut sim = sim_with(arena(vec![record("Officer", 1080, 900)]), 3);
    let id = {
        sim.step(TickInput::default());
        first(&sim, GameLogic::Officer)
    };
    let mut live = Vec::new();
    for t in 0..120 {
        sim.step(TickInput::default());
        let o = obj(&sim, id);
        if o.attack_rect != enemy::facing_rect(o, officer::CONTACT) {
            live.push((t, o.attack_rect));
        }
    }
    // The officer faces Claw (left), so its boxes are as given.
    assert!(!live.is_empty());
    for (_, r) in &live {
        assert_eq!(*r, officer::STRIKE_HIGH);
    }
    for p in live.windows(2) {
        assert!(p[1].0 - p[0].0 > 1, "one tick per swing: {live:?}");
    }
    // Claw crouches: the low strike.
    let mut low = false;
    for _ in 0..120 {
        sim.step(hold(DOWN));
        low |= obj(&sim, id).attack_rect == officer::STRIKE_LOW;
    }
    assert!(low);
}

#[test]
fn one_strike_token_serves_the_whole_level() {
    // Two officers on either side of Claw swing at once: only one strike
    // box is live at any tick.
    let mut sim = sim_with(
        arena(vec![
            record("Officer", 1080, 900),
            record("Officer", 920, 900),
        ]),
        11,
    );
    tough_claw(&mut sim);
    let mut both = 0;
    let mut any = 0;
    for _ in 0..400 {
        sim.step(TickInput::default());
        let live = enemies(&sim, GameLogic::Officer)
            .iter()
            .filter(|&&id| {
                let o = obj(&sim, id);
                o.attack_rect.is_set() && o.attack_rect != enemy::facing_rect(o, officer::CONTACT)
            })
            .count();
        any += usize::from(live > 0);
        both += usize::from(live > 1);
        let tokens = sim.with_world(|w| enemy::strike_tokens(w));
        assert!((0..=1).contains(&tokens), "{tokens}");
    }
    assert!(any > 0);
    assert_eq!(both, 0);
}

// ----------------------------------------------------------------------
// Patrols

/// Runs `ticks` idle ticks and returns the range of x the enemy covered
/// and the x where its walking direction turned.
fn patrol(sim: &mut Sim, id: ObjectId, ticks: usize) -> ((i32, i32), Vec<i32>) {
    let mut lo = i32::MAX;
    let mut hi = i32::MIN;
    let mut turns = Vec::new();
    let mut last_dir = 0.0f64;
    for _ in 0..ticks {
        let before = obj(sim, id).x;
        sim.step(TickInput::default());
        let o = obj(sim, id);
        lo = lo.min(o.x);
        hi = hi.max(o.x);
        let vx = data(sim, id).vx;
        if vx != 0.0 {
            if last_dir != 0.0 && vx.signum() != last_dir.signum() {
                turns.push(before);
            }
            last_dir = vx;
        }
    }
    ((lo, hi), turns)
}

#[test]
fn a_pacing_officer_turns_at_its_patrol_limits() {
    let mut sim = sim_with(arena(vec![bounded("Officer", 3000, 900, 2900, 3150)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Officer);
    let ((lo, hi), turns) = patrol(&mut sim, id, 6000);
    assert!(lo >= 2900 && hi <= 3150, "{lo}..{hi}");
    assert_eq!((lo, hi), (2900, 3150), "it walked the whole patrol");
    assert!(turns.contains(&2900) && turns.contains(&3150), "{turns:?}");
    assert!(
        turns.iter().all(|&x| x == 2900 || x == 3150),
        "turns only at the limits: {turns:?}"
    );
}

#[test]
fn a_walking_rat_turns_at_its_limits_without_turning_its_sprite() {
    let mut sim = sim_with(arena(vec![bounded("Rat", 3000, 926, 2950, 3100)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Rat);
    assert_eq!(feet_of(obj(&sim, id)), 959, "on the floor");
    let mirror = obj(&sim, id).draw_flags & draw_flags::MIRROR;
    let ((lo, hi), turns) = patrol(&mut sim, id, 1000);
    assert_eq!((lo, hi), (2950, 3100));
    assert!(turns.contains(&2950) && turns.contains(&3100), "{turns:?}");
    assert_eq!(obj(&sim, id).draw_flags & draw_flags::MIRROR, mirror);
    assert!(matches!(
        state_of(&sim, id),
        Some(EnemyState::MoveLeft | EnemyState::MoveRight)
    ));
}

/// Puts the rat `id` on x `x` with the fraction `carry` carried from its
/// last move, walking at `vx` in `state`.
fn place_walker(sim: &mut Sim, id: ObjectId, x: i32, carry: f64, vx: f64, state: EnemyState) {
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = x;
        o.state = state as i32;
        let y = o.y;
        let d = enemy::data_mut(w, id).unwrap();
        d.carry = (carry, 0.0);
        d.moved_to = Some((x, y));
        d.vx = vx;
    });
}

#[test]
fn a_walker_turns_only_when_exactly_on_its_limit() {
    // On the left limit's pixel with 0.52 px still to go a rat walks on
    // (its next move clamps it onto the limit, and it turns a tick
    // later), and it turns once it stands exactly on the limit. The same
    // on the right.
    let mut sim = sim_with(arena(vec![bounded("Rat", 3000, 926, 2950, 3100)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Rat);
    for (x, vx, walking, turned) in [
        (2950, -60.0, EnemyState::MoveLeft, EnemyState::MoveRight),
        (3100, 60.0, EnemyState::MoveRight, EnemyState::MoveLeft),
    ] {
        let fraction = if vx < 0.0 { 0.52 } else { -0.52 };
        place_walker(&mut sim, id, x, fraction, vx, walking);
        sim.with_world(|w| match walking {
            EnemyState::MoveLeft => states::walk_left_tick(w, id),
            _ => states::walk_right_tick(w, id),
        });
        assert_eq!(state_of(&sim, id), Some(walking), "{x}: a fraction away");
        place_walker(&mut sim, id, x, 0.0, vx, walking);
        sim.with_world(|w| match walking {
            EnemyState::MoveLeft => states::walk_left_tick(w, id),
            _ => states::walk_right_tick(w, id),
        });
        assert_eq!(state_of(&sim, id), Some(turned), "{x}: on the limit");
        assert_eq!(data(&sim, id).vx, -vx, "{x}: turned at its speed");
    }
}

#[test]
fn the_rat_touches_claw_at_a_single_point() {
    let mut sim = sim_with(arena(vec![record("Rat", 1200, 926)]), 7);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Rat);
    let o = obj(&sim, id);
    assert_eq!(o.attack_rect, rat::TOUCH_BOX);
    assert_eq!(rat::TOUCH_BOX, Rect::new(0, 0, 0, 0));
    assert!(
        o.hit_rect.is_set() && o.hit_rect != o.attack_rect,
        "hit box: the picture"
    );
}

#[test]
fn an_enemy_sees_claw_only_once_a_frame_has_shown_it() {
    // An object is made not shown.
    let made = Object::new(0, 0, 0, 0, Arc::from("Rat"), LogicKind::Inert);
    assert!(!made.drawn_last_frame);
    // A stationary rat 200 px from Claw on his floor, nothing between
    // them; Claw lands and the camera comes down to him.
    let mut rec = record("Rat", 1200, 926);
    rec.record[enemy::field::USER_0] = 1;
    let mut sim = sim_with(arena(vec![rec]), 7);
    let id = first(&sim, GameLogic::Rat);
    idle(&mut sim, 60);
    let sees = |sim: &mut Sim| sim.with_world(|w| enemy::sees_claw(w, id));
    let in_view = |sim: &mut Sim| {
        sim.with_world(|w| {
            let o = w.objects.get(id).unwrap().clone();
            w.on_screen(&o)
        })
    };
    // The frame after the last tick showed it: it sees Claw.
    assert!(obj(&sim, id).drawn_last_frame);
    assert!(in_view(&mut sim) && sees(&mut sim));
    // What counts is that frame, not where its picture is now.
    let set_drawn = |sim: &mut Sim, drawn: bool| {
        sim.with_world(|w| w.objects.get_mut(id).unwrap().drawn_last_frame = drawn);
    };
    set_drawn(&mut sim, false);
    assert!(
        in_view(&mut sim) && !sees(&mut sim),
        "in view now, not shown"
    );
    let x = obj(&sim, id).x;
    sim.with_world(|w| w.objects.get_mut(id).unwrap().x = x + 2000);
    set_drawn(&mut sim, true);
    assert!(
        !in_view(&mut sim) && sees(&mut sim),
        "shown, out of view now"
    );
    // The next frame does not show it out there, nor a hidden enemy.
    sim.step(TickInput::default());
    assert!(!obj(&sim, id).drawn_last_frame, "out of view");
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = x;
        o.draw_flags |= draw_flags::HIDDEN;
    });
    sim.step(TickInput::default());
    assert!(!obj(&sim, id).drawn_last_frame, "hidden");
    sim.with_world(|w| w.objects.get_mut(id).unwrap().draw_flags &= !draw_flags::HIDDEN);
    sim.step(TickInput::default());
    assert!(obj(&sim, id).drawn_last_frame, "shown again");
}

/// A corridor six tiles wide between two walls on a flat floor (the left
/// wall's right edge at x 2624, the right wall's left edge at x 3008).
fn corridor(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    for r in rows.iter_mut().skip(10) {
        r.replace_range(40..41, "#");
        r.replace_range(47..48, "#");
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    level(&rows, (1000, 600), objects)
}

/// Steps until the enemy's walking speed turns from `from` (a wall's
/// response); returns the tick, its x before that tick and its limits
/// after it.
fn until_wall(sim: &mut Sim, id: ObjectId, from: f64, ticks: usize) -> (usize, i32, (i32, i32)) {
    for t in 0..ticks {
        let before = obj(sim, id).x;
        sim.step(TickInput::default());
        let d = data(sim, id);
        if d.vx == -0.5 * from {
            return (t, before, d.bounds);
        }
        assert_eq!(d.vx, from, "walking at {from} until the wall");
    }
    panic!("no wall within {ticks} ticks");
}

/// Asserts the walker stays on `x` in `state` with speed `vx` for `ticks`
/// ticks; returns the frames its walk showed.
fn held_at_wall(
    sim: &mut Sim,
    id: ObjectId,
    x: i32,
    state: EnemyState,
    vx: f64,
    ticks: usize,
) -> BTreeSet<i32> {
    let mut frames = BTreeSet::new();
    for t in 0..ticks {
        sim.step(TickInput::default());
        let o = obj(sim, id);
        assert_eq!(o.x, x, "tick {t}: held on the wall's limit");
        assert_eq!(state_of(sim, id), Some(state), "tick {t}");
        assert_eq!(data(sim, id).vx, vx, "tick {t}: pushing into the wall");
        frames.insert(o.frame);
    }
    frames
}

#[test]
fn a_rat_walking_into_a_wall_stays_there_in_its_other_walking_state() {
    // Its left limit 20 px left of where it is placed, none on the right:
    // it walks left to the limit, turns, and walks right into the wall.
    let x0 = 44 * 64;
    let mut sim = sim_with(corridor(vec![bounded("Rat", x0, 926, x0 - 20, 0)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Rat);
    assert_eq!(feet_of(obj(&sim, id)), 959, "on the floor");
    let mut turned = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::MoveRight) {
            turned = true;
            break;
        }
    }
    assert!(turned, "it turned at its left limit");
    assert_eq!(data(&sim, id).vx, states::WALK_SPEED);
    // The wall: its right limit becomes where it stood, its speed is
    // halved and reversed, and it is still walking right.
    let (_, before, bounds) = until_wall(&mut sim, id, states::WALK_SPEED, 400);
    assert!(before > 46 * 64, "at the right wall: {before}");
    assert_eq!(bounds.1, before, "the limit is where it stood");
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveRight));
    // The next tick turns it with that speed, truncated and reversed
    // again: it walks left on paper and pushes right, into the wall, for
    // good.
    let frames = held_at_wall(&mut sim, id, before, EnemyState::MoveLeft, 30.0, 2000);
    assert!(frames.len() > 1, "it walks on the spot: {frames:?}");
}

#[test]
fn a_rat_walking_left_into_a_wall_stays_there_in_its_other_walking_state() {
    // No limits: from Home it walks left, into the left wall.
    let mut sim = sim_with(corridor(vec![record("Rat", 44 * 64, 926)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Rat);
    let (_, before, bounds) = until_wall(&mut sim, id, -states::WALK_SPEED, 400);
    assert!(before < 42 * 64, "at the left wall: {before}");
    assert_eq!(bounds.0, before, "the limit is where it stood");
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveLeft));
    let frames = held_at_wall(&mut sim, id, before, EnemyState::MoveRight, -30.0, 2000);
    assert!(frames.len() > 1, "it walks on the spot: {frames:?}");
}

#[test]
fn any_enemy_in_the_shared_walking_states_is_held_by_a_wall() {
    // The shared walking states are every enemy's: an officer set walking
    // right in the corridor is held by the wall as the rat is.
    let mut sim = sim_with(corridor(vec![record("Officer", 44 * 64, 900)]), 7);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Officer);
    sim.with_world(|w| states::move_right(w, id, 60.0));
    let (_, before, bounds) = until_wall(&mut sim, id, 60.0, 400);
    assert_eq!(bounds.1, before);
    held_at_wall(&mut sim, id, before, EnemyState::MoveLeft, 30.0, 500);
}

#[test]
fn a_walker_turns_with_its_speed_reversed_and_truncated() {
    // At a limit or ledge: the speed it had, reversed and truncated.
    assert_eq!(states::turn_speed(60.0), 60.0);
    assert_eq!(states::turn_speed(45.9), 45.0);
    // After a wall turned it at half speed: as it is, back into the wall.
    assert_eq!(states::turn_speed(-30.0), -30.0);
    assert_eq!(states::turn_speed(-30.7), -30.0);
    assert_eq!(states::turn_speed(0.4), 0.0);
}

#[test]
fn an_unbounded_walker_learns_its_ledges_and_never_falls() {
    // A ledge four tiles wide, high above the floor.
    let mut rows = vec![".".repeat(200); 15];
    rows[8].replace_range(40..44, "####");
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut sim = sim_with(
        level(
            &rows,
            (1000, 600),
            vec![record("Officer", 42 * 64, 8 * 64 - 60)],
        ),
        9,
    );
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Officer);
    let y0 = obj(&sim, id).y;
    let ((lo, hi), _) = patrol(&mut sim, id, 6000);
    assert_eq!(obj(&sim, id).y, y0, "never fell");
    let d = data(&sim, id);
    // The movement box is 20 px either side: the ledge guard sends the
    // walker back to where it stood before its leading edge left the
    // ledge, a step (2 px) at most from the ledge's last pixel.
    assert!((40 * 64 + 20..=40 * 64 + 22).contains(&lo), "{lo} {d:?}");
    assert!((44 * 64 - 23..=44 * 64 - 21).contains(&hi), "{hi} {d:?}");
    assert_eq!(d.bounds, (lo, hi), "the ledges became its limits");
}

#[test]
fn a_walker_starting_outside_its_limits_is_put_on_the_nearest() {
    let mut sim = sim_with(arena(vec![bounded("Soldier", 2500, 900, 2700, 2900)]), 2);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Soldier);
    assert_eq!(obj(&sim, id).x, 2700);
}

#[test]
fn a_rat_placed_beside_its_range_is_built_on_its_limit_standing_on_the_floor() {
    let rat_at = |x: i32| {
        let sim = sim_with(arena(vec![bounded("Rat", x, 900, 2700, 2900)]), 2);
        let o = obj(&sim, first(&sim, GameLogic::Rat));
        (o.x, o.y, o.physics_type)
    };
    let on_limit = rat_at(2700);
    assert_eq!(on_limit.0, 2700);
    // Placed 200 px left of its range, or 300 px right of it: built on the
    // nearer limit, on the floor as if it had been placed there.
    assert_eq!(rat_at(2500), on_limit);
    assert_eq!(rat_at(3200), (2900, on_limit.1, on_limit.2));
}

#[test]
fn the_construction_step_clamps_across_and_up_and_down_when_set() {
    let mut o = Object::new(0, 2500, 900, 0, Arc::from("Rat"), LogicKind::Inert);
    enemy::clamp_on_construction(&mut o, (2700, 2900), (1000, 1100));
    assert_eq!((o.x, o.y), (2700, 1000));
    o.x = 3000;
    o.y = 1200;
    enemy::clamp_on_construction(&mut o, (2700, 2900), (1000, 1100));
    assert_eq!((o.x, o.y), (2900, 1100));
    // Without limits nothing moves.
    o.x = 5;
    o.y = 7;
    enemy::clamp_on_construction(&mut o, enemy::UNBOUNDED, enemy::UNBOUNDED);
    assert_eq!((o.x, o.y), (5, 7));
}

#[test]
fn an_enemy_woken_by_its_placement_and_built_far_from_it_sleeps_again() {
    // Claw starts at x 1000: the rat placed 700 px right of him is inside
    // the enemies' 800 px half width of the activation window and wakes
    // while the level loads; its range starts 1600 px right of him, so it
    // is built outside the window and sleeps at the end of the first tick.
    // A rat placed in its range at the same point stays awake.
    let far = arena(vec![bounded("Rat", 1700, 900, 2600, 2700)]);
    let near = arena(vec![bounded("Rat", 1700, 900, 1600, 1800)]);
    let mut asleep = Vec::new();
    for level in [far, near] {
        let mut sim = Sim::new(level, SimConfig::default());
        let id = first(&sim, GameLogic::Rat);
        assert!(obj(&sim, id).is_listed(), "woken while the level loads");
        sim.step(TickInput::default());
        asleep.push((obj(&sim, id).x, !obj(&sim, id).is_listed()));
    }
    assert_eq!(asleep[0], (2600, true));
    assert!(!asleep[1].1, "awake in its range");
}

// ----------------------------------------------------------------------
// The soldier

#[test]
fn the_soldier_fires_at_most_once_per_firing_window() {
    // Claw at 1000, the soldier 250 px to his right: in sight, out of
    // striking range.
    let mut sim = sim_with(arena(vec![record("Soldier", 1250, 900)]), 4);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Soldier);
    let mut fired_at = Vec::new();
    let mut seen = BTreeSet::new();
    let mut windows = 0;
    let mut in_window = false;
    let mut per_window = 0;
    for t in 0..400 {
        sim.step(TickInput::default());
        let fire = state_of(&sim, id) == Some(EnemyState::Fire);
        if fire && !in_window {
            windows += 1;
            per_window = 0;
        }
        in_window = fire;
        for spec in new_shots(&sim, &mut seen) {
            fired_at.push(t);
            per_window += 1;
            assert!(per_window <= 1, "one shot per firing window");
            let o = obj(&sim, id);
            assert_eq!(spec.logic, "Bullet");
            assert_eq!(spec.image_set, "LEVEL_MUSKETBALL");
            assert_eq!(spec.damage, 10);
            assert_eq!(spec.type_flags, types::ENEMY_SHOT);
            assert_eq!((spec.vx, spec.vy), (-500, 0), "toward Claw");
            assert_eq!((spec.x, spec.y), (o.x - 65, o.y - 38), "from the muzzle");
            assert_eq!(spec.z, o.z - 1);
            assert_eq!(spec.owner, Some(id));
        }
    }
    assert!(windows >= 3, "{windows} firing windows");
    assert!(fired_at.len() >= 3, "{fired_at:?}");
    assert!(fired_at.len() <= windows);
    let gap = 500usize.div_ceil(18);
    for p in fired_at.windows(2) {
        assert!(p[1] - p[0] >= gap, "500 ms apart: {fired_at:?}");
    }
}

#[test]
fn the_soldier_clubs_claw_up_close_and_holds_fire_out_of_sight() {
    // Within 80 px: the musket butt.
    let mut sim = sim_with(arena(vec![record("Soldier", 1060, 900)]), 4);
    let mut struck = false;
    for _ in 0..150 {
        sim.step(TickInput::default());
        let id = first(&sim, GameLogic::Soldier);
        struck |= obj(&sim, id).attack_rect == soldier::STRIKE;
    }
    assert!(struck);
    assert!(shots(&sim).is_empty());
    // A wall between them: no shot, no alert.
    let mut rows = vec![".".repeat(200); 15];
    for r in rows.iter_mut().take(15).skip(10) {
        r.replace_range(17..18, "#");
    }
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut sim = sim_with(
        level(&rows, (1000, 600), vec![record("Soldier", 1250, 900)]),
        4,
    );
    for _ in 0..300 {
        sim.step(TickInput::default());
        assert!(!cues(&sim).iter().any(|k| k == soldier::ALERT));
    }
    assert!(shots(&sim).is_empty(), "{:?}", shots(&sim));
}

#[test]
fn the_soldier_shouts_its_alert_once() {
    let mut sim = sim_with(arena(vec![record("Soldier", 1200, 900)]), 4);
    let mut alerts = 0;
    for _ in 0..400 {
        sim.step(TickInput::default());
        alerts += cues(&sim).iter().filter(|k| *k == soldier::ALERT).count();
    }
    assert_eq!(alerts, 1);
}

// ----------------------------------------------------------------------
// The rat

#[test]
fn the_rat_throws_across_at_most_once_a_second() {
    // A stationary rat 200 px from Claw on the same floor.
    let mut rec = record("Rat", 1200, 926);
    rec.record[enemy::field::USER_0] = 1;
    let mut sim = sim_with(arena(vec![rec]), 6);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Rat);
    let mut thrown_at = Vec::new();
    let mut seen = BTreeSet::new();
    for t in 0..300 {
        sim.step(TickInput::default());
        for spec in new_shots(&sim, &mut seen) {
            thrown_at.push(t);
            let o = obj(&sim, id);
            assert_eq!(spec.logic, "RatBomb");
            assert_eq!(spec.image_set, "LEVEL_RATBOMB");
            assert_eq!((spec.vx, spec.vy), (-500, 0));
            assert_eq!((spec.x, spec.y), (o.x - 13, o.y - 22));
            assert_eq!(spec.z, o.z - 1);
            assert_eq!(spec.special_flags, rat::BOMB_ACROSS);
        }
    }
    assert!(thrown_at.len() >= 4, "{thrown_at:?}");
    let gap = 1000usize.div_ceil(18);
    for p in thrown_at.windows(2) {
        assert!(p[1] - p[0] >= gap, "a second apart: {thrown_at:?}");
    }
    assert_eq!(obj(&sim, id).x, 1200, "a stationary rat never walks");
}

#[test]
fn the_rat_drops_bombs_on_claw_below() {
    // The rat stands on a one-way ledge 40 px above the floor, Claw right
    // under it.
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = level(&rows, (1010, 899), vec![]);
    let ledge = common::thin_ground(&mut l.map, 8);
    l.map.set_cell(15, 14, ledge);
    l.map.set_cell(16, 14, ledge);
    let mut rec = record("Rat", 1000, 14 * 64 - 33 + 32);
    rec.record[enemy::field::USER_0] = 1;
    l.objects = vec![rec];
    let mut sim = sim_with(l, 8);
    idle(&mut sim, 2);
    let id = first(&sim, GameLogic::Rat);
    let (r, c) = (obj(&sim, id), sim.player().unwrap());
    assert_eq!(feet_of(r), 14 * 64 - 1, "the rat stands on the ledge");
    assert!(
        c.y > r.y && c.y - r.y < 50,
        "Claw just below: {} {}",
        c.y,
        r.y
    );
    let mut down = 0;
    let mut across = 0;
    let mut thrown = BTreeMap::new();
    // Each bomb is taken away once noted: its explosion would catch the
    // rat itself, which any blow kills.
    for _ in 0..200 {
        sim.step(TickInput::default());
        thrown.extend(shots(&sim));
        clear_shots(&mut sim);
    }
    for spec in thrown.into_values() {
        if spec.vx == 0 {
            down += 1;
            assert_eq!(spec.vy, 250);
            let o = obj(&sim, id);
            assert_eq!((spec.x, spec.y), (o.x + 1, o.y - 12));
            assert_eq!(spec.special_flags, 0);
        } else {
            across += 1;
        }
    }
    assert!(down > across && across >= 1, "{down} down, {across} across");
}

// ----------------------------------------------------------------------
// Hits and death

/// A synthetic attacker of `type_flags` with `damage`, its attack box over
/// the target.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32, special: u32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.special_flags = special;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits `id` as a new attack of Claw (or the same one when
/// `new_attack` is false); returns the random draws the hit made.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId, new_attack: bool, upper: bool) -> u32 {
    let before = sim.rng().state();
    sim.with_world(|w| {
        if new_attack {
            w.claw.attack.serial += 1;
        }
        w.claw.attack.uppercut = upper;
        let a = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = a.0;
            o.y = a.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        claw_sim::logic::on_hit(w, id, attacker);
    });
    draws_between(before, sim.rng().state())
}

/// Number of draws taking the stream from `a` to `b` (at most 64).
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

/// A sim with one enemy (with `health`), Claw 250 px to its right (on
/// screen, out of range), built and idle.
fn lone(logic: &str, health: Option<i32>) -> (Sim, ObjectId) {
    let y = if logic == "Rat" { 926 } else { 900 };
    let mut rec = record(logic, 3000, y);
    rec.record[15] = 33;
    let mut sim = sim_with(arena(vec![rec]), 12);
    sim.teleport_player(3250, 899);
    idle(&mut sim, 2);
    let g = match logic {
        "Officer" => GameLogic::Officer,
        "Soldier" => GameLogic::Soldier,
        _ => GameLogic::Rat,
    };
    let id = first(&sim, g);
    if let Some(h) = health {
        sim.with_world(|w| w.objects.get_mut(id).unwrap().health = h);
    }
    (sim, id)
}

#[test]
fn hit_reactions_follow_what_hit() {
    let claw_id = |sim: &Sim| {
        sim.objects()
            .all()
            .find(|(_, o)| o.kind == LogicKind::Player)
            .map(|(id, _)| id)
            .unwrap()
    };
    // Claw's blow: low flinch, a 300 ms cooldown, the burst's three draws
    // (its place, then its size).
    let (mut sim, id) = lone("Officer", Some(50));
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    assert_eq!(hit(&mut sim, id, blow, true, false), 3);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 45);
    assert_eq!(
        data(&sim, id).vx,
        -200.0,
        "knocked away from Claw (to the right of it)"
    );
    // The same attack never hits twice; a new one during the cooldown is
    // turned away; during the flinch, a blow only sends it Home.
    assert_eq!(hit(&mut sim, id, blow, false, false), 0);
    assert_eq!(hit(&mut sim, id, blow, true, false), 0);
    assert_eq!(obj(&sim, id).health, 45);
    idle(&mut sim, 17);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(hit(&mut sim, id, blow, true, false), 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, 45);
    let _ = claw_id(&sim);

    // The uppercut: a high flinch, one more draw for its sound.
    let (mut sim, id) = lone("Officer", Some(50));
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    assert_eq!(hit(&mut sim, id, blow, true, true), 4);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitHigh));

    // A pistol shot is used up; no cooldown.
    let (mut sim, id) = lone("Officer", Some(50));
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8, 0);
    hit(&mut sim, id, shot, true, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    assert_eq!(obj(&sim, id).health, 42);
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
    assert!(data(&sim, id).cooldown.done(sim.tick() * 18));

    // Magic and thrown objects: low flinch, the attacker flies on.
    for kind in [0x200_0000, types::THROWN] {
        let (mut sim, id) = lone("Officer", Some(50));
        let a = attacker(&mut sim, kind, 10, 0);
        hit(&mut sim, id, a, true, false);
        assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
        assert_eq!(obj(&sim, id).health, 40);
        assert!(obj(&sim, a).flags & flags::DESTROY == 0);
    }

    // The level's own shots and explosions: a 2000 ms cooldown.
    let (mut sim, id) = lone("Officer", Some(50));
    let boom = attacker(&mut sim, types::LEVEL_SHOT, 15, 0);
    hit(&mut sim, id, boom, true, false);
    assert_eq!(obj(&sim, id).health, 35);
    assert_eq!(data(&sim, id).cooldown.len, 2000);

    // An enemy's shot cannot hurt an enemy.
    let (mut sim, id) = lone("Officer", Some(50));
    let ball = attacker(&mut sim, types::ENEMY_SHOT, 10, 0);
    assert_eq!(hit(&mut sim, id, ball, true, false), 0);
    assert_eq!(obj(&sim, id).health, 50);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn the_knockback_slides_twelve_px_and_stops_at_exactly_zero() {
    let (mut sim, id) = lone("Officer", Some(50));
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    hit(&mut sim, id, blow, true, false);
    let x0 = obj(&sim, id).x;
    let mut speeds = Vec::new();
    for _ in 0..12 {
        sim.step(TickInput::default());
        speeds.push(data(&sim, id).vx);
    }
    assert_eq!(
        &speeds[..9],
        &[
            -175.0, -150.0, -125.0, -100.0, -75.0, -50.0, -25.0, 0.0, 0.0
        ]
    );
    // Claw is to the right: knocked to the left by 12.6 px, which lands
    // 13 px to the left (positions are cut to whole pixels downward).
    assert_eq!(x0 - obj(&sim, id).x, 13);
}

#[test]
fn death_drops_the_loot_once_and_the_body_flies_off() {
    let mut rec = record("Officer", 3000, 900);
    rec.record[35] = 33;
    rec.record[15] = 16;
    let mut sim = sim_with(arena(vec![rec]), 12);
    sim.teleport_player(3250, 899);
    idle(&mut sim, 2);
    let id = first(&sim, GameLogic::Officer);
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    let before = sim.rng().state();
    hit(&mut sim, id, blow, true, false);
    let after = sim.rng().state();
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    let o = obj(&sim, id);
    assert!(o.health <= 0);
    assert!(o.flags & flags::HARMLESS != 0, "a body hurts nobody");
    assert_eq!(o.physics_type, ptype::DIRECT, "no tiles");
    let d = data(&sim, id);
    assert_eq!((d.vx, d.vy), (-150.0, -700.0), "up and away from Claw");
    assert_eq!(d.death, DeathKind::Hit);
    // The loot: one goodie per filled slot, in slot order, fanned out.
    let drops: Vec<(i32, f64)> = sim
        .objects()
        .all()
        .filter_map(|(_, o)| match &o.data {
            ObjectData::Pickup(
                p @ PickupData {
                    drop_code: Some(c), ..
                },
            ) => Some((*c, p.goodie.map_or(f64::NAN, |g| g.vx))),
            _ => None,
        })
        .collect();
    assert_eq!(drops, vec![(33, -25.0), (16, 175.0)]);
    // Draws: the burst's two, a high flinch none, the loot's vertical
    // speeds (two items: no random speed across), then the death roll and,
    // a quarter of the time, Claw's line.
    let n = draws_between(before, after);
    assert!(n == 2 + 2 + 1 || n == 2 + 2 + 2, "{n} draws");
    // Hit again as it flies: no second loot.
    let blow2 = attacker(&mut sim, types::PLAYER, 5, 0);
    idle(&mut sim, 20);
    hit(&mut sim, id, blow2, true, false);
    let count = sim
        .objects()
        .all()
        .filter(|(_, o)| matches!(&o.data, ObjectData::Pickup(p) if p.drop_code.is_some()))
        .count();
    assert_eq!(count, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    // The body leaves the screen and is removed with a splash.
    let mut splash = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        splash |= cues(&sim).iter().any(|k| k == enemy::SPLASH_SOUND);
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    assert!(sim.objects().get(id).is_none(), "removed");
    assert!(splash);
}

#[test]
fn a_blow_of_fire_picks_the_fire_death() {
    let (mut sim, id) = lone("Officer", Some(3));
    let blow = attacker(&mut sim, 0x200_0000, 25, 0x4_0000);
    hit(&mut sim, id, blow, true, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(data(&sim, id).death_anim, enemy::FIRE_DEATH_ANIM);
}

#[test]
fn any_blow_kills_a_rat_and_a_pistol_shot_is_used_up() {
    let (mut sim, id) = lone("Rat", None);
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (1, 5));
    let shot = attacker(&mut sim, types::PISTOL_SHOT, 8, 0);
    let draws = hit(&mut sim, id, shot, true, false);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
    // No burst and no loot: the death roll, and Claw's line a quarter of
    // the time.
    assert!(draws == 1 || draws == 2, "{draws}");
    assert!(
        !sim.objects()
            .all()
            .any(|(_, o)| matches!(&o.data, ObjectData::Pickup(_))),
        "a rat drops nothing"
    );
    // A new blow on the body kills it again (the roll is drawn again).
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    let again = hit(&mut sim, id, blow, true, false);
    assert!(again == 1 || again == 2, "{again}");
}

#[test]
fn a_fall_onto_spikes_kills_in_place() {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("x".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = level(&rows, (1000, 600), vec![record("Officer", 3000, 600)]);
    // Claw stands on a ledge out of the way.
    l.map.set_cell(15, 12, 0);
    let mut sim = sim_with(l, 3);
    idle(&mut sim, 60);
    let id = first(&sim, GameLogic::Officer);
    // It was built standing in the spikes; throw it up so it falls back.
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.y -= 100;
        o.physics_type = ptype::FALL;
    });
    let mut tile = false;
    for _ in 0..60 {
        sim.step(TickInput::default());
        tile |= cues(&sim).iter().any(|k| k == enemy::DEATH_TILE_SOUND);
        if state_of(&sim, id) == Some(EnemyState::Dead) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(data(&sim, id).death, DeathKind::Spikes);
    assert!(tile);
    let at = (obj(&sim, id).x, obj(&sim, id).y);
    idle(&mut sim, 30);
    assert_eq!((obj(&sim, id).x, obj(&sim, id).y), at, "frozen in place");
    idle(&mut sim, 40);
    assert!(sim.objects().get(id).is_none(), "removed after a second");
}

/// A walking rat (it has the ledge guard) standing on an elevator that
/// runs up and down over open air: its leading edge never finds tile floor,
/// so the guard sends it back across on every step that moves it (its
/// patrol limits are opened again each tick to keep the guard firing), but
/// it keeps the height the elevator carried it to and rides the surface
/// the whole way.
#[test]
fn a_guarded_walker_rides_a_platform_up_and_down() {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut lift = ObjectSpec::at("Elevator", 3000, 500, 0);
    // A vertical track from 440 to 560 at 150 px/s.
    lift.record[52] = 440;
    lift.record[54] = 560;
    lift.record[56] = 150;
    let mut sim = sim_with(
        level(
            &rows,
            (1000, 600),
            vec![lift, bounded("Rat", 3000, 900, 2900, 3100)],
        ),
        5,
    );
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Rat);
    let lift = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Elevator")
        .map(|(id, _)| id)
        .expect("the elevator");
    assert!(data(&sim, id).ledge_guard);
    let rides = |sim: &Sim| {
        let o = obj(sim, id);
        let p = obj(sim, lift);
        o.flags & flags::ON_ELEVATOR != 0
            && o.object_below == Some(lift)
            && feet_of(o) == p.y + p.hit_rect.top - 1
    };
    // Built on the floor; stand it on the elevator.
    sim.with_world(|w| {
        let top = w.objects.get(lift).map(|p| p.y + p.hit_rect.top).unwrap();
        let o = w.objects.get_mut(id).unwrap();
        o.x = 3000;
        o.y = top - 1 - o.move_rect.bottom;
        o.physics_type = ptype::STAND;
        o.flags |= flags::ON_ELEVATOR;
        o.object_below = Some(lift);
    });
    let (mut up, mut down, mut guarded) = (0, 0, 0);
    for t in 0..300 {
        sim.with_world(|w| {
            if let Some(ObjectData::Enemy(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
                d.bounds = (2900, 3100);
            }
        });
        let y = obj(&sim, lift).y;
        sim.step(TickInput::default());
        let dy = obj(&sim, lift).y - y;
        let d = data(&sim, id);
        if d.bounds != (2900, 3100) {
            guarded += 1;
            if dy < 0 {
                up += 1;
            } else if dy > 0 {
                down += 1;
            }
        }
        let o = obj(&sim, id);
        assert!(
            rides(&sim),
            "tick {t}: feet {} on an elevator at {} (moved {dy})",
            feet_of(o),
            obj(&sim, lift).y
        );
        assert_eq!(o.physics_type, ptype::STAND, "tick {t}");
    }
    assert!(guarded > 100, "the guard fired on {guarded} ticks");
    assert!(
        up > 0 && down > 0,
        "the guard fired going up ({up}) and down ({down})"
    );
}

// ----------------------------------------------------------------------
// Narrow ledges

#[test]
fn on_a_narrow_ledge_an_enemy_leaps_at_claw_nearby_and_stays_put_otherwise() {
    // A pillar one tile wide, two tiles above the floor.
    let mut rows = vec![".".repeat(200); 15];
    rows[13].replace_range(20..21, "#");
    rows[14].replace_range(20..21, "#");
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let spot = record("Officer", 20 * 64 + 32, 13 * 64 - 52);
    // Claw 150 px away on the floor: it leaps at him.
    let mut sim = sim_with(
        level(&rows, (20 * 64 + 32 - 150, 600), vec![spot.clone()]),
        1,
    );
    let mut leapt = false;
    for _ in 0..120 {
        sim.step(TickInput::default());
        let id = first(&sim, GameLogic::Officer);
        leapt |= state_of(&sim, id) == Some(EnemyState::JumpAtClaw);
    }
    assert!(leapt);
    let id = first(&sim, GameLogic::Officer);
    assert_eq!(feet_of(obj(&sim, id)), 959, "landed on the floor");
    // Claw far away: it stays on its pillar for good.
    let mut sim = sim_with(level(&rows, (3000, 600), vec![spot]), 1);
    idle(&mut sim, 400);
    let id = first(&sim, GameLogic::Officer);
    assert!(data(&sim, id).stationary);
    assert_eq!(obj(&sim, id).x, 20 * 64 + 32);
    let mut ledge = Sim::clone(&sim);
    let on = ledge.with_world(|w| motion::on_narrow_ledge(w, id));
    assert!(on);
}

/// A platform of columns 20 to 30 (its top row the feet rest on is 831),
/// with clear air left of it, and optionally a wall of column 21 rising
/// from it, and an Officer placed at x 1312 with Claw far away.
fn ledge_beside(wall: bool) -> Sim {
    let mut rows = vec![".".repeat(200); 15];
    rows[13].replace_range(20..31, &"#".repeat(11));
    if wall {
        rows[12].replace_range(21..22, "#");
        rows[11].replace_range(21..22, "#");
    }
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let spot = record("Officer", 20 * 64 + 32, 13 * 64 - 52);
    sim_with(level(&rows, (3000, 600), vec![spot]), 1)
}

#[test]
fn a_wall_rising_from_a_wide_floor_ends_a_narrow_ledge() {
    // The floor ends 33 px to the left; on the right a wall rises 32 px
    // away from a floor that goes on for 600 px. The wall ends the ledge,
    // so with Claw far away the Officer stays put.
    let mut sim = ledge_beside(true);
    idle(&mut sim, 400);
    let id = first(&sim, GameLogic::Officer);
    assert_eq!(feet_of(obj(&sim, id)), 831, "on the platform");
    assert!(data(&sim, id).stationary);
    assert_eq!(obj(&sim, id).x, 20 * 64 + 32);
    let mut ledge = Sim::clone(&sim);
    assert!(ledge.with_world(|w| motion::on_narrow_ledge(w, id)));
}

#[test]
fn a_clear_floor_run_beside_an_enemy_is_not_a_narrow_ledge() {
    // The same platform without the wall: the floor goes on to the right,
    // so the ledge is not narrow and the Officer is free to pace.
    let mut sim = ledge_beside(false);
    let id = first(&sim, GameLogic::Officer);
    let mut ledge = Sim::clone(&sim);
    idle(&mut ledge, 1);
    assert_eq!(feet_of(obj(&ledge, id)), 831, "on the platform");
    assert_eq!(obj(&ledge, id).x, 20 * 64 + 32);
    assert!(!ledge.with_world(|w| motion::on_narrow_ledge(w, id)));
    idle(&mut sim, 400);
    assert!(!data(&sim, id).stationary);
}

// ----------------------------------------------------------------------
// Lifting

#[test]
fn a_lifted_officer_is_carried_thrown_and_dies_of_the_third_landing() {
    let (mut sim, id) = lone("Officer", None);
    let claw = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .unwrap();
    sim.teleport_player(3060, 899);
    idle(&mut sim, 2);
    assert!(obj(&sim, id).special_flags & claw_sim::object::special::LIFTABLE != 0);
    sim.with_world(|w| lift::lifted(w, id, claw));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
    assert!(obj(&sim, id).flags & flags::SAFE != 0);
    // Claw's overhead frame raises it over his head.
    sim.with_world(|w| {
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_OVERHEAD_FRAME;
        claw_sim::logic::tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Flailing));
    sim.step(TickInput::default());
    let (c, o) = (sim.player().unwrap(), obj(&sim, id));
    assert_eq!(o.y, c.y - 45);
    // Thrown: it flies, bounces twice, rests, and the fall costs it 4.
    sim.with_world(|w| lift::thrown(w, id, 300, -650));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Thrown));
    assert_eq!(obj(&sim, id).type_flags, types::THROWN);
    let mut seen = Vec::new();
    let mut thud = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        thud |= cues(&sim).iter().any(|k| k == enemy::LAND_SOUND);
        if let Some(s) = state_of(&sim, id)
            && seen.last() != Some(&s)
        {
            seen.push(s);
        }
        if sim.objects().get(id).is_none() {
            break;
        }
    }
    assert!(thud);
    assert_eq!(
        seen,
        vec![
            EnemyState::Thrown,
            EnemyState::Bounce,
            EnemyState::Pause,
            EnemyState::Bounce,
            EnemyState::Dead
        ],
        "{seen:?}"
    );
    assert!(sim.objects().get(id).is_none(), "removed a second later");
}

/// An officer of `level` (its record at x 3000), lifted by Claw, thrown,
/// and then put in flight at the exact position `p` with speeds `v`, its
/// movement box the one it is carried with facing right.
fn officer_in_flight(
    level: SimLevel,
    claw_at: (i32, i32),
    p: (f64, f64),
    v: (f64, f64),
) -> (Sim, ObjectId) {
    let mut sim = sim_with(level, 12);
    sim.teleport_player(claw_at.0, claw_at.1);
    idle(&mut sim, 2);
    let id = first(&sim, GameLogic::Officer);
    let claw = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .unwrap();
    sim.with_world(|w| lift::lifted(w, id, claw));
    sim.with_world(|w| lift::thrown(w, id, 0, 0));
    let at = (p.0.trunc() as i32, p.1.trunc() as i32);
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = at.0;
        o.y = at.1;
        o.move_rect = Rect::new(-48, -44, 42, 19);
        let d = enemy::data_mut(w, id).unwrap();
        d.carry = (p.0 - f64::from(at.0), p.1 - f64::from(at.1));
        d.moved_to = Some(at);
        d.vx = v.0;
        d.vy = v.1;
        d.bounds = enemy::UNBOUNDED;
        d.bounds_y = enemy::UNBOUNDED;
    });
    (sim, id)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn a_thrown_officer_lands_at_the_speed_its_last_move_gives() {
    // Falling at 804 px/s from 9.396 px above where the floor (y 960)
    // stops it: a full tick would move it 14.796 px and end at 840 px/s,
    // but the move it made gives sqrt(804^2 + 2 * 2000 * 9.396), 827.04.
    let (mut sim, id) = officer_in_flight(
        arena(vec![record("Officer", 3000, 900)]),
        (2600, 899),
        (3019.0, 930.604),
        (-100.0, 804.0),
    );
    sim.step(TickInput::default());
    let (o, d) = (obj(&sim, id), data(&sim, id));
    assert_eq!((o.x, o.y, o.physics_type), (3019, 940, ptype::STAND));
    assert!(near(d.vy, 827.042925), "{}", d.vy);
    assert!(near(motion::speed_after(804.0, 2000.0, 9.396), d.vy));
    // The bounce starts from it: lifted 1 px (the speed corrected for
    // that), a third of it upwards, then one tick of gravity.
    sim.step(TickInput::default());
    let (o, d) = (obj(&sim, id), data(&sim, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Bounce));
    assert_eq!((o.x, o.y), (3018, 934));
    assert!(near(d.vy, -238.8737), "{}", d.vy);
}

#[test]
fn a_bouncing_officer_keeps_the_fractions_of_its_exact_position() {
    // Its second landing comes exactly at its target, with 0.832 px across
    // and 0.0752 px down carried; the bounce lifts it 1 px and keeps both,
    // so the next tick's 0.496 px across takes it to the next pixel.
    let (mut sim, id) = officer_in_flight(
        arena(vec![record("Officer", 3000, 900)]),
        (2600, 899),
        (3002.344, 935.2864),
        (248.0 / 3.0, 248.0444),
    );
    sim.with_world(|w| enemy::data_mut(w, id).unwrap().bounces = 1);
    sim.step(TickInput::default());
    let o = obj(&sim, id);
    assert_eq!((o.x, o.y, o.physics_type), (3003, 940, ptype::STAND));
    sim.step(TickInput::default());
    let (o, d) = (obj(&sim, id), data(&sim, id));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Bounce));
    assert_eq!((o.x, o.y), (3004, 937));
    assert!(near(d.vy, -56.3046), "{}", d.vy);
}

/// The arena with a step: the floor (y 960) is raised 64 px (to y 896)
/// from x 3200 on.
fn stepped_arena() -> SimLevel {
    let mut l = arena(vec![record("Officer", 3000, 900)]);
    let mut rows = vec![".".repeat(200); 14];
    rows.push(format!("{}{}", ".".repeat(50), "#".repeat(150)));
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    l.map = TileMap::from_ascii(64, &rows);
    l
}

#[test]
fn a_thrown_officer_caught_by_a_steps_corner_lands_where_it_was() {
    // Falling right into the step's side: the wall stops it across and its
    // right part lands on the step's top, but its exact position moved
    // back by the fraction it carried, so the ledge guard looks at its
    // left edge, finds no floor there and puts it back where it was before
    // the step, standing in the air with its left limit there and its
    // speed down corrected for the move it made.
    let (mut sim, id) = officer_in_flight(
        stepped_arena(),
        (2600, 899),
        (3167.364, 870.368),
        (347.0, 526.0),
    );
    sim.step(TickInput::default());
    let (o, d) = (obj(&sim, id), data(&sim, id));
    assert_eq!((o.x, o.y, o.physics_type), (3167, 870, ptype::STAND));
    assert_eq!(d.bounds.0, 3167);
    assert_eq!(d.vx, 347.0);
    assert!(near(d.vy, 524.5989), "{}", d.vy);
    // It bounces from there, a third as fast.
    sim.step(TickInput::default());
    let (o, d) = (obj(&sim, id), data(&sim, id));
    assert_eq!((o.x, o.y, o.physics_type), (3169, 866, ptype::FLY));
    assert!(near(d.vx, 115.6667) && near(d.vy, -137.5908), "{d:?}");
}

#[test]
fn a_dropped_officer_gets_up_and_goes_home() {
    let (mut sim, id) = lone("Officer", None);
    let claw = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .unwrap();
    sim.with_world(|w| lift::lifted(w, id, claw));
    sim.with_world(|w| lift::dropped(w, id));
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dropped));
    idle(&mut sim, 60);
    assert_ne!(state_of(&sim, id), Some(EnemyState::Dropped));
    let o = obj(&sim, id);
    assert_eq!(o.move_rect, officer::MOVE_RECT, "its own box again");
    assert_eq!(feet_of(o), 959, "on the floor");
    assert!(o.flags & (flags::SAFE | flags::NO_COLLIDE) == 0);
}

#[test]
fn a_rat_cannot_be_lifted() {
    let (mut sim, id) = lone("Rat", None);
    idle(&mut sim, 2);
    assert!(obj(&sim, id).special_flags & claw_sim::object::special::LIFTABLE == 0);
    sim.with_world(|w| lift::lifted(w, id, 0));
    assert_ne!(state_of(&sim, id), Some(EnemyState::Lifted));
}

// ----------------------------------------------------------------------
// Lifting a defeated enemy

/// Claw's id.
fn claw_of(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .unwrap()
}

/// The loot dropped and still lying about.
fn loot_ids(sim: &Sim) -> BTreeSet<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| matches!(&o.data, ObjectData::Pickup(p) if p.drop_code.is_some()))
        .map(|(id, _)| id)
        .collect()
}

/// A lone officer with `health` and one loot slot, killed where it stands
/// as on spikes (its loot dropped once), with Claw beside it.
fn body(health: i32) -> (Sim, ObjectId) {
    let (mut sim, id) = lone("Officer", Some(health));
    sim.teleport_player(3060, 899);
    idle(&mut sim, 2);
    sim.with_world(|w| enemy::die(w, id, DeathKind::Spikes, None));
    assert_eq!(loot_ids(&sim).len(), 1, "the loot of the death");
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    (sim, id)
}

/// Lifts `id` and raises it over Claw's head on his lift frames, checking
/// the lifted animations.
fn lift_overhead(sim: &mut Sim, id: ObjectId) {
    let claw = claw_of(sim);
    sim.with_world(|w| lift::lifted(w, id, claw));
    assert_eq!(state_of(sim, id), Some(EnemyState::Lifted));
    sim.with_world(|w| {
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_LIFT_FRAME;
        claw_sim::logic::tick(w, id);
    });
    assert_eq!(obj(sim, id).anim.key(), Some("LEVEL_OFFICER_LIFTED"));
    sim.with_world(|w| {
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_OVERHEAD_FRAME;
        claw_sim::logic::tick(w, id);
    });
    assert_eq!(state_of(sim, id), Some(EnemyState::Flailing));
    assert_eq!(obj(sim, id).anim.key(), Some("LEVEL_OFFICER_LIFTED2"));
    sim.step(TickInput::default());
}

/// What happened over the next `n` ticks or until `id` is removed.
struct Followed {
    /// Every sound played.
    sounds: Vec<String>,
    /// The states `id` went through.
    seen: Vec<EnemyState>,
    /// Ticks `id` spent dead.
    dead_ticks: usize,
    /// Loot that appeared.
    new_loot: usize,
}

fn follow(sim: &mut Sim, id: ObjectId, n: usize) -> Followed {
    let (mut sounds, mut seen, mut dead_ticks) = (Vec::new(), Vec::new(), 0);
    let mut loot = loot_ids(sim);
    let mut new_loot = 0;
    for _ in 0..n {
        sim.step(TickInput::default());
        sounds.extend(cues(sim));
        for l in loot_ids(sim) {
            new_loot += usize::from(loot.insert(l));
        }
        let Some(s) = state_of(sim, id) else { break };
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        if s == EnemyState::Dead {
            dead_ticks += 1;
        }
    }
    Followed {
        sounds,
        seen,
        dead_ticks,
        new_loot,
    }
}

#[test]
fn a_body_killed_on_spikes_stays_liftable() {
    let (sim, id) = body(3);
    let o = obj(&sim, id);
    assert!(o.special_flags & claw_sim::object::special::LIFTABLE != 0);
    assert_eq!(o.type_flags, types::ENEMY, "still an enemy for Claw");
}

#[test]
fn a_lifted_body_draws_for_its_lifted_line_as_a_live_one_does() {
    let (mut sim, id) = body(3);
    let claw = claw_of(&sim);
    let before = sim.rng().state();
    sim.with_world(|w| lift::lifted(w, id, claw));
    // The officer's line choice, then whether it speaks.
    assert_eq!(draws_between(before, sim.rng().state()), 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
    let o = obj(&sim, id);
    assert!(o.flags & flags::SAFE != 0);
    assert!(data(&sim, id).frozen, "carried, not moving by itself");
}

#[test]
fn a_thrown_body_bounces_and_dies_again_without_loot_or_sound() {
    // Health well above the landing's cost: a body dies again whatever
    // its health.
    let (mut sim, id) = body(50);
    lift_overhead(&mut sim, id);
    sim.with_world(|w| lift::thrown(w, id, 300, -650));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Thrown));
    let o = obj(&sim, id);
    assert_eq!(o.type_flags, types::THROWN);
    assert!(o.flags & flags::HARMLESS != 0, "a body hurts nobody");
    let Followed {
        sounds,
        seen,
        dead_ticks,
        new_loot,
    } = follow(&mut sim, id, 300);
    assert_eq!(
        seen,
        vec![
            EnemyState::Thrown,
            EnemyState::Bounce,
            EnemyState::Pause,
            EnemyState::Bounce,
            EnemyState::Dead
        ],
        "{seen:?}"
    );
    assert!(sim.objects().get(id).is_none(), "removed");
    // Removed 1000 ms after its second death (18 ms ticks).
    assert!((55..=57).contains(&dead_ticks), "{dead_ticks} ticks dead");
    assert_eq!(new_loot, 0, "no second loot");
    let quiet = [
        enemy::DROP_SOUND,
        enemy::DEATH_TILE_SOUND,
        enemy::SPLASH_SOUND,
        officer::DEATH_SOUNDS[0],
        officer::DEATH_SOUNDS[1],
    ];
    assert!(
        !sounds.iter().any(|s| quiet.contains(&s.as_str())),
        "{sounds:?}"
    );
    assert!(sounds.iter().any(|s| s == enemy::LAND_SOUND), "the thud");
}

#[test]
fn a_thrown_body_dies_again_where_it_lies_as_a_thrown_object() {
    let (mut sim, id) = body(3);
    lift_overhead(&mut sim, id);
    sim.with_world(|w| lift::thrown(w, id, 300, -650));
    for _ in 0..300 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Dead) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(data(&sim, id).death, DeathKind::Landing);
    let o = obj(&sim, id);
    assert_eq!(o.type_flags, types::THROWN, "never an enemy again");
    // The enemy side refuses it too.
    let claw = claw_of(&sim);
    sim.with_world(|w| lift::lifted(w, id, claw));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
}

#[test]
fn a_thrown_body_out_of_time_drops_no_second_loot() {
    let (mut sim, id) = body(3);
    lift_overhead(&mut sim, id);
    sim.with_world(|w| lift::thrown(w, id, 300, -650));
    sim.step(TickInput::default());
    // Its time in the air runs out before it lands.
    sim.with_world(|w| {
        let now = w.now_ms;
        enemy::data_mut(w, id).unwrap().thrown_timer = enemy::Timer::new(now, 0);
    });
    let death = data(&sim, id).death_sound;
    let loot = loot_ids(&sim);
    sim.step(TickInput::default());
    let sounds = cues(&sim);
    assert!(sim.objects().get(id).is_none(), "gone");
    assert!(loot_ids(&sim).is_subset(&loot), "no second loot");
    assert!(!sounds.iter().any(|s| s == enemy::DROP_SOUND), "{sounds:?}");
    // Its death sound and a splash, as for a live enemy out of time.
    assert!(
        sounds.iter().any(|s| s == enemy::SPLASH_SOUND),
        "{sounds:?}"
    );
    assert!(sounds.iter().any(|s| s == death), "{sounds:?}");
}

#[test]
fn a_thrown_body_hurts_no_enemy_and_a_thrown_live_one_does() {
    for dead in [true, false] {
        let mut sim = sim_with(
            arena(vec![
                record("Officer", 3000, 900),
                record("Officer", 3400, 900),
            ]),
            12,
        );
        sim.teleport_player(3060, 899);
        idle(&mut sim, 2);
        let ids = enemies(&sim, GameLogic::Officer);
        let (thrown, other) = if obj(&sim, ids[0]).x == 3000 {
            (ids[0], ids[1])
        } else {
            (ids[1], ids[0])
        };
        if dead {
            sim.with_world(|w| enemy::die(w, thrown, DeathKind::Spikes, None));
            sim.step(TickInput::default());
        }
        lift_overhead(&mut sim, thrown);
        sim.with_world(|w| lift::thrown(w, thrown, 300, -650));
        let health = obj(&sim, other).health;
        let mut struck = false;
        for _ in 0..5 {
            // Flying through the other officer.
            let Some((x, y)) = sim.objects().get(other).map(|o| (o.x, o.y)) else {
                break;
            };
            sim.with_world(|w| {
                let o = w.objects.get_mut(thrown).unwrap();
                o.x = x;
                o.y = y;
            });
            sim.step(TickInput::default());
            struck |= sim.collision_events().iter().any(|e| {
                matches!(*e, claw_sim::container::CollisionEvent::Hit { attacker, victim }
                    if attacker == thrown && victim == other)
            });
        }
        if dead {
            assert!(!struck, "the body strikes nobody");
            assert_eq!(obj(&sim, other).health, health);
            assert!(!matches!(
                state_of(&sim, other),
                Some(EnemyState::HitLow | EnemyState::HitHigh | EnemyState::Dead)
            ));
        } else {
            assert!(struck, "a thrown live officer strikes the other");
            assert!(sim.objects().get(other).is_none_or(|o| o.health < health));
        }
    }
}

#[test]
fn a_dropped_body_stays_dead() {
    let (mut sim, id) = body(50);
    lift_overhead(&mut sim, id);
    sim.with_world(|w| lift::dropped(w, id));
    let Followed {
        sounds,
        seen,
        dead_ticks,
        new_loot,
    } = follow(&mut sim, id, 300);
    assert_eq!(
        seen,
        vec![EnemyState::Dropped, EnemyState::Dead],
        "it never gets up: {seen:?}"
    );
    assert!(sim.objects().get(id).is_none(), "removed");
    assert!((55..=57).contains(&dead_ticks), "{dead_ticks} ticks dead");
    assert_eq!(new_loot, 0, "no second loot");
    let quiet = [
        enemy::DROP_SOUND,
        enemy::DEATH_TILE_SOUND,
        officer::DEATH_SOUNDS[0],
        officer::DEATH_SOUNDS[1],
    ];
    assert!(
        !sounds.iter().any(|s| quiet.contains(&s.as_str())),
        "{sounds:?}"
    );
}

#[test]
fn a_dropped_body_lies_on_the_floor_in_its_own_box() {
    let (mut sim, id) = body(50);
    lift_overhead(&mut sim, id);
    sim.with_world(|w| lift::dropped(w, id));
    for _ in 0..200 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Dead) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    let d = data(&sim, id);
    assert!(d.dead && d.busy && d.frozen);
    assert_eq!(d.death, DeathKind::Landing);
    let o = obj(&sim, id);
    assert_eq!(o.move_rect, officer::MOVE_RECT, "its own box again");
    assert_eq!(feet_of(o), 959, "on the floor");
    assert!(o.flags & (flags::SAFE | flags::NO_COLLIDE | flags::ALWAYS_ACTIVE) == 0);
    assert!(o.flags & flags::HARMLESS != 0, "still harmless");
    assert_eq!(o.type_flags, types::ENEMY);
}

#[test]
fn a_goo_death_is_gone_at_once_and_cannot_be_lifted() {
    let (mut sim, id) = lone("Officer", None);
    sim.with_world(|w| enemy::die(w, id, DeathKind::Goo, None));
    assert!(obj(&sim, id).flags & flags::DESTROY != 0);
    let claw = claw_of(&sim);
    sim.with_world(|w| lift::lifted(w, id, claw));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    sim.step(TickInput::default());
    assert!(sim.objects().get(id).is_none(), "removed");
}

// ----------------------------------------------------------------------
// Random draws per class

/// Steps once and checks the stream moved by exactly the draws `expect`
/// makes on `mirror`.
fn step_draws(sim: &mut Sim, mirror: &mut Rng, input: TickInput, expect: impl FnOnce(&mut Rng)) {
    assert_eq!(sim.rng().state(), mirror.state(), "in step before the tick");
    sim.step(input);
    expect(mirror);
    assert_eq!(sim.rng().state(), mirror.state(), "draws of the tick");
}

#[test]
fn nothing_else_draws_while_claw_stands_idle() {
    let mut sim = sim_with(arena(vec![]), 5);
    let s = sim.rng().state();
    idle(&mut sim, 100);
    assert_eq!(sim.rng().state(), s);
}

/// True when tick `k` (counted from 0) is a frame pulse tick, the only
/// ticks on which enemies make their per-tick roll: the pulse countdown
/// starts at 100 ms when play starts and loses 18 ms a tick, so the first
/// pulse is the sixth tick, then every third tick.
fn pulse_tick(k: u64) -> bool {
    k >= 5 && (k - 5).is_multiple_of(3)
}

#[test]
fn the_frame_pulse_comes_every_third_tick_from_the_sixth() {
    let mut sim = sim_with(arena(vec![]), 5);
    let mut pulses = Vec::new();
    for k in 0..30 {
        sim.step(TickInput::default());
        assert_eq!(sim.pulse(), pulse_tick(k), "tick {k}");
        if sim.pulse() {
            pulses.push(k);
        }
    }
    assert_eq!(pulses, [5, 8, 11, 14, 17, 20, 23, 26, 29]);
}

/// The officer's go-idle draws: the mutter roll, and which mutter.
fn officer_go_idle(m: &mut Rng) {
    if m.below(100) < 33 {
        m.below(2);
    }
}

#[test]
fn the_officer_draws_in_the_games_order() {
    // Built while the level loads, with Claw 300 px away (out of range):
    // the death sound, then going idle, drawn from the level-load state;
    // time zero then starts from the seed.
    let mut sim = sim_with(arena(vec![record("Officer", 1300, 900)]), 77);
    assert_eq!(sim.rng().state(), 77);
    let mut m = Rng::new(77);
    let id = first(&sim, GameLogic::Officer);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // The first tick: no per-tick draw (no pulse yet), a pose (its timer
    // has not run yet), and Claw out of range sends it idle again in the
    // post-tick.
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        m.below(5);
        officer_go_idle(m);
    });
    while !pulse_tick(sim.tick()) {
        step_draws(&mut sim, &mut m, TickInput::default(), |_| {});
    }
    // Idle out of range: the per-tick draw on pulse ticks, a pose each
    // second.
    for _ in 0..40 {
        let t = sim.tick() * 18;
        let roll = pulse_tick(sim.tick());
        let pose = data(&sim, id).pose.done(t);
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if roll {
                m.below(1001);
            }
            if pose {
                m.below(5);
            }
        });
    }
    // Claw comes in range: Idle sends it Home (no draw), Home draws one
    // discarded and the taunt roll, and it strikes.
    sim.teleport_player(1230, 899);
    let t = sim.tick() * 18;
    let roll = pulse_tick(sim.tick());
    let pose = data(&sim, id).pose.done(t);
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        if roll {
            m.below(1001);
        }
        if pose {
            m.below(5);
        }
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    let roll = pulse_tick(sim.tick());
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        if roll {
            m.below(1001);
        }
        m.next_u15();
        m.below(100);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
}

#[test]
fn the_soldier_draws_in_the_games_order() {
    // On a ledge high above Claw, out of sight: idle, its two poses, and
    // the pacing roll after four seconds; going idle draws nothing.
    let mut rows = vec![".".repeat(200); 15];
    rows[9].replace_range(18..24, "######");
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut sim = sim_with(
        level(
            &rows,
            (1000, 600),
            vec![record("Soldier", 1300, 9 * 64 - 60)],
        ),
        31,
    );
    // Built while the level loads (no draws); from the first tick on, the
    // per-tick draw and the pacing roll come on pulse ticks only.
    let mut m = Rng::new(sim.rng().state());
    let id = first(&sim, GameLogic::Soldier);
    let mut paced = false;
    for _ in 0..400 {
        let t = sim.tick() * 18;
        let pulse = pulse_tick(sim.tick());
        if state_of(&sim, id) != Some(EnemyState::Idle) {
            paced = true;
            break;
        }
        let d = data(&sim, id);
        let (pose, wait) = (d.pose.done(t), d.pace_wait.done(t));
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if pulse {
                m.below(1001);
            }
            if pose {
                m.below(2);
            }
            if wait && pulse {
                m.below(100);
            }
        });
    }
    assert!(paced, "it started pacing");
    // In sight and in range of its gun: Home draws one discarded, then it
    // fires.
    let mut sim = sim_with(arena(vec![record("Soldier", 1250, 900)]), 31);
    idle(&mut sim, 10);
    let id = first(&sim, GameLogic::Soldier);
    let mut m = Rng::new(sim.rng().state());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    // Its shots are taken away as they appear: the draws Claw makes when
    // one hits him are not the soldier's.
    clear_shots(&mut sim);
    let mut homes = 0;
    for _ in 0..100 {
        let at_home = state_of(&sim, id) == Some(EnemyState::Home);
        homes += usize::from(at_home);
        let pulse = pulse_tick(sim.tick());
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if pulse {
                m.below(1001);
            }
            if at_home {
                m.next_u15();
            }
        });
        clear_shots(&mut sim);
    }
    assert!(homes > 0);
}

#[test]
fn the_rat_draws_in_the_games_order() {
    // A walking rat out of sight: one draw on each pulse tick.
    let mut sim = sim_with(arena(vec![bounded("Rat", 3000, 926, 2900, 3100)]), 13);
    let mut m = Rng::new(sim.rng().state());
    for _ in 0..200 {
        let pulse = pulse_tick(sim.tick());
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if pulse {
                m.below(1001);
            }
        });
    }
}

#[test]
fn a_kill_by_claw_draws_his_line_a_quarter_of_the_time() {
    let mut lines = 0;
    let mut sounds = 0;
    for seed in 1..80 {
        let mut rec = record("Officer", 3000, 900);
        rec.record[15] = 0;
        let mut sim = sim_with(arena(vec![rec]), seed);
        send_claw_away(&mut sim);
        let id = first(&sim, GameLogic::Officer);
        let blow = attacker(&mut sim, types::PLAYER, 5, 0);
        sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 1);
        let before = sim.rng().state();
        hit(&mut sim, id, blow, true, false);
        // The burst's place, then the death roll: with Claw sent away the
        // officer is off screen, so it gets no burst and no size draw.
        let mut m = Rng::new(before);
        m.below(9);
        m.below(9);
        let roll = m.below(100);
        if roll < 25 {
            m.below(10);
            lines += 1;
        } else {
            sounds += 1;
        }
        assert_eq!(sim.rng().state(), m.state(), "seed {seed}");
    }
    assert!(
        lines > 5 && sounds > lines,
        "{lines} lines, {sounds} sounds"
    );
}

/// The hit bursts in the level.
fn bursts(sim: &Sim) -> Vec<&Object> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == "HitBurst")
        .map(|(_, o)| o)
        .collect()
}

#[test]
fn an_enemy_owns_its_hit_burst_and_gets_none_off_screen() {
    // On screen: the burst's place and its size.
    let mut sim = sim_with(arena(vec![record("Officer", 1080, 900)]), 5);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Officer);
    let blow = attacker(&mut sim, types::PLAYER, 1, 0);
    let before = sim.rng().state();
    hit(&mut sim, id, blow, true, false);
    assert_eq!(bursts(&sim).len(), 1, "a burst on screen");
    let drawn_on_screen = draws_between(before, sim.rng().state());
    assert!(drawn_on_screen >= 3, "place and size: {drawn_on_screen}");
    // Off screen: the same hit makes the place's two draws and no burst.
    let mut sim = sim_with(arena(vec![record("Officer", 3000, 900)]), 5);
    send_claw_away(&mut sim);
    let id = first(&sim, GameLogic::Officer);
    let view = sim.view().rect;
    let o = obj(&sim, id);
    assert!(!view.contains(o.x, o.y), "the officer is off screen");
    let blow = attacker(&mut sim, types::PLAYER, 1, 0);
    let before = sim.rng().state();
    hit(&mut sim, id, blow, true, false);
    assert!(bursts(&sim).is_empty(), "no burst off screen");
    let drawn_off_screen = draws_between(before, sim.rng().state());
    assert_eq!(drawn_off_screen + 1, drawn_on_screen, "no size draw");
}

#[test]
fn the_officer_has_seven_health_after_the_first_level() {
    let mut l = arena(vec![
        record("Officer", 1300, 900),
        record("Soldier", 1600, 900),
    ]);
    l.number = 2;
    let mut sim = sim_with(l, 1);
    sim.step(TickInput::default());
    assert_eq!(obj(&sim, first(&sim, GameLogic::Officer)).health, 7);
    assert_eq!(obj(&sim, first(&sim, GameLogic::Soldier)).health, 7);
}

#[test]
fn enemy_lines_share_one_speaking_slot() {
    // Two officers taunting at once: only one line runs at a time.
    let mut sim = sim_with(
        arena(vec![
            record("Officer", 1080, 900),
            record("Officer", 920, 900),
        ]),
        21,
    );
    tough_claw(&mut sim);
    let mut said = Vec::new();
    for t in 0..600 {
        sim.step(TickInput::default());
        for k in cues(&sim) {
            if k == officer::TAUNT || officer::MUTTERS.contains(&k.as_str()) {
                said.push(t);
            }
        }
    }
    assert!(!said.is_empty());
    let gap = 600usize.div_ceil(18);
    for p in said.windows(2) {
        assert!(p[1] - p[0] >= gap, "{said:?}");
    }
}

#[test]
fn going_home_stops_an_enemy_across_and_keeps_its_speed_up_or_down() {
    let mut sim = sim_with(arena(vec![record("Officer", 1300, 900)]), 5);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Officer);
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.vx = 60.0;
        d.vy = 251.71;
        enemy::go_home(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    let d = data(&sim, id);
    assert_eq!((d.vx, d.vy), (0.0, 251.71), "it falls on at its speed");
}

#[test]
fn claw_is_on_an_enemys_right_when_his_pixel_is_at_or_past_its_exact_x() {
    let mut sim = sim_with(arena(vec![record("Officer", 1300, 900)]), 5);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::Officer);
    let claw_x = sim.player().expect("Claw").x;
    let side = |sim: &mut Sim, x: i32, carry: f64| {
        sim.with_world(|w| {
            let o = w.objects.get_mut(id).unwrap();
            o.x = x;
            let y = o.y;
            let d = enemy::data_mut(w, id).unwrap();
            d.carry.0 = carry;
            d.moved_to = Some((x, y));
        });
        sim.step(TickInput::default());
        assert_eq!(sim.player().unwrap().x, claw_x, "Claw stands still");
        data(sim, id).claw_right
    };
    // On the same pixel, Claw is on the enemy's right only when the
    // enemy's exact x is not past his pixel: at 0.6 px past it he is on
    // its left.
    assert!(side(&mut sim, claw_x, 0.0));
    assert!(!side(&mut sim, claw_x, 0.6));
    assert!(side(&mut sim, claw_x - 1, 0.6));
    assert!(!side(&mut sim, claw_x + 1, -0.4));
}
