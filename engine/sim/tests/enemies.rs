//! The officer, the soldier and the rat on synthetic levels: range boxes
//! and Home, patrols turning at their limits, the soldier's shots and the
//! rat's bombs (read from the spawn requests), hit reactions by what hit,
//! death and loot, the strike token, being lifted and thrown, and the
//! order of random draws of each class.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState, lift, motion, officer, rat, soldier};
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

/// The projectile spawn requests that exist, by object.
fn shots(sim: &Sim) -> Vec<(ObjectId, ProjectileSpec)> {
    let mut v: Vec<(ObjectId, ProjectileSpec)> = sim
        .objects()
        .all()
        .filter_map(|(id, o)| match &o.data {
            ObjectData::Projectile(ProjectileData {
                spec: Some(spec), ..
            }) => Some((id, *spec)),
            _ => None,
        })
        .collect();
    v.sort_unstable_by_key(|s| s.0);
    v
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
    let mut seen = 0;
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
        let all = shots(&sim);
        for (_, spec) in &all[seen..] {
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
        seen = all.len();
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
    let mut seen = 0;
    for t in 0..300 {
        sim.step(TickInput::default());
        let all = shots(&sim);
        for (_, spec) in &all[seen..] {
            thrown_at.push(t);
            let o = obj(&sim, id);
            assert_eq!(spec.logic, "RatBomb");
            assert_eq!(spec.image_set, "LEVEL_RATBOMB");
            assert_eq!((spec.vx, spec.vy), (-500, 0));
            assert_eq!((spec.x, spec.y), (o.x - 13, o.y - 22));
            assert_eq!(spec.z, o.z - 1);
            assert_eq!(spec.special_flags, rat::BOMB_ACROSS);
        }
        seen = all.len();
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
    for _ in 0..200 {
        sim.step(TickInput::default());
    }
    for (_, spec) in shots(&sim) {
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
    // Claw's blow: low flinch, a 300 ms cooldown, the burst's two draws.
    let (mut sim, id) = lone("Officer", Some(50));
    let blow = attacker(&mut sim, types::PLAYER, 5, 0);
    assert_eq!(hit(&mut sim, id, blow, true, false), 2);
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
    assert_eq!(hit(&mut sim, id, blow, true, true), 3);
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

/// The officer's go-idle draws: the mutter roll, and which mutter.
fn officer_go_idle(m: &mut Rng) {
    if m.below(100) < 33 {
        m.below(2);
    }
}

#[test]
fn the_officer_draws_in_the_games_order() {
    // Built on the first tick, with Claw 300 px away (out of range).
    let mut sim = sim_with(arena(vec![record("Officer", 1300, 900)]), 77);
    let mut m = Rng::new(sim.rng().state());
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        // Construction: the death sound, then going idle.
        m.below(100);
        officer_go_idle(m);
    });
    let id = first(&sim, GameLogic::Officer);
    // The first tick: no per-tick draw while the level starts, a pose
    // (its timer has not run yet), and Claw out of range sends it idle
    // again in the post-tick.
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        m.below(5);
        officer_go_idle(m);
    });
    while sim.tick() * 18 < 100 {
        step_draws(&mut sim, &mut m, TickInput::default(), |_| {});
    }
    // Idle out of range: the per-tick draw, a pose each second.
    for _ in 0..40 {
        let t = sim.tick() * 18;
        let pose = data(&sim, id).pose.done(t);
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            m.below(1001);
            if pose {
                m.below(5);
            }
        });
    }
    // Claw comes in range: Idle sends it Home (no draw), Home draws one
    // discarded and the taunt roll, and it strikes.
    sim.teleport_player(1230, 899);
    let t = sim.tick() * 18;
    let pose = data(&sim, id).pose.done(t);
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        m.below(1001);
        if pose {
            m.below(5);
        }
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Home));
    step_draws(&mut sim, &mut m, TickInput::default(), |m| {
        m.below(1001);
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
    let mut m = Rng::new(sim.rng().state());
    step_draws(&mut sim, &mut m, TickInput::default(), |_| {});
    let id = first(&sim, GameLogic::Soldier);
    let mut paced = false;
    for _ in 0..400 {
        let t = sim.tick() * 18;
        if state_of(&sim, id) != Some(EnemyState::Idle) {
            paced = true;
            break;
        }
        let d = data(&sim, id);
        let (pose, wait) = (d.pose.done(t), d.pace_wait.done(t));
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if t >= 100 {
                m.below(1001);
            }
            if pose {
                m.below(2);
            }
            if wait && t >= 100 {
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
    let mut homes = 0;
    for _ in 0..100 {
        let at_home = state_of(&sim, id) == Some(EnemyState::Home);
        homes += usize::from(at_home);
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            m.below(1001);
            if at_home {
                m.next_u15();
            }
        });
    }
    assert!(homes > 0);
}

#[test]
fn the_rat_draws_in_the_games_order() {
    // A walking rat out of sight: one draw per tick.
    let mut sim = sim_with(arena(vec![bounded("Rat", 3000, 926, 2900, 3100)]), 13);
    let mut m = Rng::new(sim.rng().state());
    step_draws(&mut sim, &mut m, TickInput::default(), |_| {});
    for _ in 0..200 {
        let t = sim.tick() * 18;
        step_draws(&mut sim, &mut m, TickInput::default(), |m| {
            if t >= 100 {
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
        // The burst's two draws, then the death roll.
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
