//! The third level's thieves on synthetic levels: the cutthroat (knife
//! throws, the sword slash up close, the throw that replaces a slash, the
//! Home and idle flip with Claw hidden, the once-only alert, pacing) and
//! the robber thief (arrows on sight at any distance, no blow up close,
//! the Home and idle flip, a hit cancelling a shot, the stationary
//! record), their building, being lifted and thrown, death and loot, and
//! the order of their random draws.

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use claw_frame::{AudioCue, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, cutthroat, lift, robberthief};
use claw_sim::logic::projectile::{self, ProjectileData, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};
use claw_sim::rng::Rng;
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

/// Animations shaped like the thieves': a slash and throws with one event
/// frame, flinches of about a third of a second, one-frame poses.
fn thief_anims() -> Vec<Arc<Animation>> {
    let strike = [(200, 100, 0), (201, 80, 1), (202, 200, 0), (200, 0, 0)];
    let throw = [(300, 300, 0), (301, 100, 1), (302, 150, 0), (300, 0, 0)];
    let flinch = [(600, 50, 0), (601, 200, 0), (600, 50, 0), (201, 0, 0)];
    let mut v = Vec::new();
    for c in ["CUTTHROAT", "ROBBERTHIEF"] {
        let k = |s: &str| format!("LEVEL_{c}_{s}");
        v.push(anim(&k("HITLOW"), &flinch, true));
        v.push(anim(&k("HITHIGH"), &flinch, true));
        v.push(anim(&k("FALL"), &[(950, 0, 0)], true));
        v.push(anim(&k("HOME"), &[(201, 0, 0)], true));
        v.push(anim(&k("JUMP"), &[(5, 100, 0), (6, 0, 0)], true));
        v.push(anim(&k("LIFTED"), &[(6, 50, 0), (6, 0, 0)], true));
        v.push(anim(&k("LIFTED2"), &[(7, 100, 0), (8, 100, 0)], false));
        v.push(anim(&k("THROWN"), &[(8, 0, 0)], true));
    }
    v.push(anim(cutthroat::STRIKE_ANIM, &strike, true));
    v.push(anim(cutthroat::THROW_ANIM, &throw, true));
    v.push(anim(cutthroat::THROW_LOW_ANIM, &throw, true));
    for (i, key) in cutthroat::STAND.iter().enumerate() {
        v.push(anim(key, &[(100 + i as i32, 0, 0)], true));
    }
    v.push(anim(
        "LEVEL_CUTTHROAT_WALK",
        &[(1, 150, 0), (2, 150, 0)],
        false,
    ));
    v.push(anim(robberthief::SHOOT_ANIM, &throw, true));
    v.push(anim(robberthief::SHOOT_LOW_ANIM, &throw, true));
    v.push(anim(
        robberthief::IDLE_ANIM,
        &[(50, 400, 0), (51, 400, 0), (52, 0, 0)],
        true,
    ));
    v.push(anim(
        "LEVEL_ROBBERTHIEF_FASTADVANCE",
        &[(3, 150, 0), (4, 150, 0)],
        false,
    ));
    v
}

/// The lines the thieves say, with their lengths.
fn lines() -> Vec<(&'static str, u32)> {
    let mut v = vec![
        (cutthroat::ALERT, 800),
        (cutthroat::LIFTED_LINE, 700),
        (cutthroat::DEATH_SOUND, 600),
        (robberthief::LIFTED_LINE, 700),
        ("CLAW_1001", 900),
    ];
    v.extend(cutthroat::IDLE_LINES.iter().map(|&k| (k, 600)));
    v.extend(robberthief::IDLE_LINES.iter().map(|&k| (k, 600)));
    v
}

/// The flat floor of the common helpers (feet rest on 959) on level
/// `number`, with the thieves' animations and lines and the given
/// records.
fn arena(number: u32, objects: Vec<ObjectSpec>) -> SimLevel {
    dress(flat_level(), number, objects)
}

fn dress(mut l: SimLevel, number: u32, objects: Vec<ObjectSpec>) -> SimLevel {
    for a in thief_anims() {
        l.animations.insert(a.key.clone(), a);
    }
    for (k, ms) in lines() {
        l.sounds.insert(k.to_string(), Some(ms));
    }
    l.number = number;
    l.objects = objects;
    l
}

/// The flat floor with a wall one tile wide in column 17 (x 1088 to
/// 1151) standing on it, five tiles high: Claw at x 1000 is hidden from
/// a thief to the right of it.
fn walled(number: u32, objects: Vec<ObjectSpec>) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    for r in rows.iter_mut().take(15).skip(10) {
        r.replace_range(17..18, "#");
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let l = SimLevel::synthetic(claw_sim::tiles::TileMap::from_ascii(64, &rows), (1000, 600));
    dress(l, number, objects)
}

/// A record of `logic` at (x, y) with its image set.
fn record(logic: &str, x: i32, y: i32) -> ObjectSpec {
    let mut o = ObjectSpec::at(logic, x, y, 4000);
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

/// A simulation of `level` in which every object is awake from the start
/// and Claw can take a long fight.
fn sim_with(level: SimLevel, seed: u32) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.with_world(|w| w.claw.stats.health = 10_000);
    sim
}

fn first(sim: &Sim, g: GameLogic) -> ObjectId {
    sim.objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(g))
        .map(|(id, _)| id)
        .min()
        .expect("the enemy exists")
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .expect("Claw")
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

fn count(cues: &[String], key: &str) -> usize {
    cues.iter().filter(|k| *k == key).count()
}

/// The projectiles that exist, by object.
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

/// The projectiles that appeared since `seen` was last updated.
fn new_shots(sim: &Sim, seen: &mut BTreeSet<ObjectId>) -> Vec<(ObjectId, ProjectileSpec)> {
    shots(sim)
        .into_iter()
        .filter(|(id, _)| seen.insert(*id))
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

/// Width of a box, px.
fn span(r: Rect) -> i32 {
    r.right - r.left
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

/// The draws `f` makes on the world's stream.
fn draws_of(sim: &mut Sim, f: impl FnOnce(&mut claw_sim::logic::World)) -> u32 {
    let before = sim.rng().state();
    sim.with_world(f);
    draws_between(before, sim.rng().state())
}

/// An attacker of `type_flags` dealing `damage`, outside the live list.
fn attacker(sim: &mut Sim, type_flags: u32, damage: i32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = damage;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits `id` as a new attack of Claw.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId) {
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.uppercut = false;
        let a = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = a.0;
            o.y = a.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        claw_sim::logic::on_hit(w, id, attacker);
    });
}

/// One thief of `logic` at x on level `number`, Claw at x 1000, built.
fn lone(logic: &str, number: u32, x: i32, seed: u32) -> (Sim, ObjectId) {
    let mut sim = sim_with(arena(number, vec![record(logic, x, 900)]), seed);
    sim.step(TickInput::default());
    let g = if logic == "CutThroat" {
        GameLogic::CutThroat
    } else {
        GameLogic::RobberThief
    };
    let id = first(&sim, g);
    (sim, id)
}

/// Runs `n` ticks with `input`, removing the shots after noting them: the
/// ticks (from 0) on which a shot appeared, with its request, and the
/// sounds of those ticks.
fn shots_over(
    sim: &mut Sim,
    n: usize,
    input: TickInput,
) -> Vec<(usize, ProjectileSpec, Vec<String>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for t in 0..n {
        sim.step(input);
        for (_, spec) in new_shots(sim, &mut seen) {
            out.push((t, spec, cues(sim)));
        }
        clear_shots(sim);
    }
    out
}

// ----------------------------------------------------------------------
// Building

#[test]
fn the_thieves_are_built_as_human_enemies_of_their_level() {
    for (number, cut_health) in [(3, 7), (4, 9)] {
        let mut sim = sim_with(
            arena(
                number,
                vec![
                    record("CutThroat", 3000, 900),
                    record("RobberThief", 3600, 900),
                ],
            ),
            4,
        );
        sim.step(TickInput::default());
        let c = first(&sim, GameLogic::CutThroat);
        let r = first(&sim, GameLogic::RobberThief);
        for (id, health, smarts) in [(c, cut_health, 99), (r, 7, 99)] {
            let o = obj(&sim, id);
            assert_eq!((o.health, o.damage), (health, 10), "level {number}");
            assert_eq!(o.type_flags, types::ENEMY);
            assert_eq!(o.hit_type_flags, enemy::HUMAN_HIT_BY);
            assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
            let d = data(&sim, id);
            assert_eq!(d.smarts, smarts);
            assert!(d.sight && d.may_lift && d.ledge_guard);
            // Dropped onto the floor: the movement box's foot on 959.
            assert_eq!(o.y + o.move_rect.bottom, 959);
        }
        let o = obj(&sim, c);
        assert!(matches!(o.data, ObjectData::CutThroat(_)));
        assert_eq!(&*o.image_set, "LEVEL_CUTTHROAT");
        assert_eq!(
            (o.move_rect, o.hit_rect),
            (cutthroat::MOVE_RECT, cutthroat::HIT_RECT)
        );
        let d = data(&sim, c);
        assert_eq!(d.ranges, cutthroat::RANGES);
        assert_eq!(d.strike_rect, cutthroat::STRIKE);
        assert_eq!(d.death_sound, cutthroat::DEATH_SOUND);
        let o = obj(&sim, r);
        assert!(matches!(o.data, ObjectData::RobberThief(_)));
        assert_eq!(&*o.image_set, "LEVEL_ROBBERTHIEF");
        let d = data(&sim, r);
        assert_eq!(d.ranges, robberthief::RANGES);
        assert_eq!(d.death_sound, "", "no death sound of his own");
        assert_eq!(d.human.anims.idle, robberthief::IDLE_ANIM);
        assert!(robberthief::IDLE_LINES.contains(&d.human.sounds.idle_voice));
        assert_eq!(o.anim.key(), Some(robberthief::IDLE_ANIM));
    }
}

#[test]
fn a_robber_thief_record_with_a_first_user_value_never_walks() {
    let mut rec = bounded("RobberThief", 3000, 900, 2900, 3100);
    rec.record[enemy::field::USER_0] = 1;
    let mut sim = sim_with(arena(3, vec![rec]), 9);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::RobberThief);
    assert!(data(&sim, id).stationary);
    let x = obj(&sim, id).x;
    for _ in 0..1500 {
        sim.step(TickInput::default());
        assert_ne!(state_of(&sim, id), Some(EnemyState::Pace));
        assert_eq!(obj(&sim, id).x, x);
    }
}

// ----------------------------------------------------------------------
// The cutthroat

#[test]
fn the_cutthroat_throws_a_knife_every_two_seconds_on_the_third_level() {
    for (number, gap) in [(3, 112), (4, 84)] {
        // Claw 250 px to his left: in sight, in his outer boxes.
        let (mut sim, id) = lone("CutThroat", number, 1250, 4);
        let thrown = shots_over(&mut sim, 500, TickInput::default());
        assert!(thrown.len() >= 3, "{}", thrown.len());
        let o = obj(&sim, id);
        for (_, spec, cues) in &thrown {
            assert_eq!(spec.logic, cutthroat::KNIFE_LOGIC);
            assert_eq!(spec.image_set, cutthroat::KNIFE_IMAGE);
            assert_eq!(spec.damage, 10);
            assert_eq!(spec.type_flags, types::ENEMY_SHOT);
            assert_eq!((spec.vx, spec.vy), (-400, 0), "toward Claw, flat");
            assert_eq!((spec.x, spec.y), (o.x - 65, o.y - 15), "chest high");
            assert_eq!(spec.z, o.z - 1);
            assert_eq!(spec.owner, Some(id));
            assert_eq!(count(cues, cutthroat::THROW_SOUND), 1);
        }
        for p in thrown.windows(2) {
            assert_eq!(p[1].0 - p[0].0, gap, "level {number}");
        }
        assert_eq!(obj(&sim, id).x, o.x, "he never walks up to Claw");
    }
}

#[test]
fn the_cutthroat_throws_low_at_a_crouching_claw() {
    let (mut sim, id) = lone("CutThroat", 3, 1250, 4);
    let mut low_anim = false;
    let mut seen = BTreeSet::new();
    let mut thrown = Vec::new();
    for _ in 0..300 {
        sim.step(hold(DOWN));
        low_anim |= obj(&sim, id).anim.key() == Some(cutthroat::THROW_LOW_ANIM);
        for (_, spec) in new_shots(&sim, &mut seen) {
            thrown.push((spec, cues(&sim)));
        }
        clear_shots(&mut sim);
    }
    assert!(low_anim);
    assert!(!thrown.is_empty());
    let o = obj(&sim, id);
    for (spec, cues) in thrown {
        assert_eq!((spec.x, spec.y), (o.x - 65, o.y + 20), "low");
        assert_eq!(count(&cues, cutthroat::THROW_LOW_SOUND), 1);
    }
}

#[test]
fn the_cutthroat_slashes_up_close_and_a_ready_throw_replaces_the_slash() {
    // Claw 40 px to his left: his innermost box.
    let (mut sim, id) = lone("CutThroat", 3, 1040, 7);
    let claw = claw_id(&sim);
    let mut slashed = false;
    let mut replaced = false;
    let mut knives = 0;
    let mut seen = BTreeSet::new();
    for _ in 0..600 {
        sim.step(TickInput::default());
        // Keep Claw where he is, whatever the blows do to him.
        sim.with_world(|w| {
            let c = w.objects.get_mut(claw).unwrap();
            c.x = 1000;
        });
        slashed |= obj(&sim, id).attack_rect == cutthroat::STRIKE;
        let d = data(&sim, id);
        if state_of(&sim, id) == Some(EnemyState::Fire) && d.prev == EnemyState::Strike {
            replaced = true;
            assert!(d.busy, "the throw leaves him busy");
        }
        knives += new_shots(&sim, &mut seen).len();
        clear_shots(&mut sim);
        assert_ne!(state_of(&sim, id), Some(EnemyState::Advance));
        assert_ne!(state_of(&sim, id), Some(EnemyState::FastAdvance));
    }
    assert!(slashed, "the sword reaches 75 px in front of him");
    assert!(replaced, "a ready throw replaces a slash just started");
    assert!(knives > 0);
}

#[test]
fn a_hidden_claw_in_his_outer_boxes_flips_him_between_home_and_idle() {
    let mut sim = sim_with(walled(3, vec![record("CutThroat", 1250, 900)]), 5);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::CutThroat);
    idle(&mut sim, 5);
    let x = obj(&sim, id).x;
    let mut alerts = 0;
    let mut last = None;
    for _ in 0..300 {
        sim.step(TickInput::default());
        alerts += count(&cues(&sim), cutthroat::ALERT);
        let s = state_of(&sim, id);
        assert!(
            matches!(s, Some(EnemyState::Idle | EnemyState::Home)),
            "{s:?}"
        );
        assert_ne!(s, last, "a new state every tick");
        last = s;
        assert_eq!(obj(&sim, id).x, x, "he never walks up to a hidden Claw");
    }
    assert!(shots(&sim).is_empty());
    assert!(alerts <= 1);
}

#[test]
fn the_cutthroat_shouts_his_alert_once_seen_or_not() {
    for level in [
        arena(3, vec![record("CutThroat", 1250, 900)]),
        walled(3, vec![record("CutThroat", 1250, 900)]),
    ] {
        let mut sim = sim_with(level, 4);
        let mut alerts = 0;
        for _ in 0..400 {
            sim.step(TickInput::default());
            alerts += count(&cues(&sim), cutthroat::ALERT);
            clear_shots(&mut sim);
        }
        assert_eq!(alerts, 1);
        let id = first(&sim, GameLogic::CutThroat);
        assert!(data(&sim, id).alerted);
    }
}

#[test]
fn far_from_claw_the_cutthroat_paces_after_four_seconds_idle() {
    let mut sim = sim_with(
        arena(3, vec![bounded("CutThroat", 6000, 900, 5900, 6100)]),
        11,
    );
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::CutThroat);
    let wait = ticks(cutthroat::PACE_WAIT_MS);
    let mut paced_at = None;
    let mut poses = BTreeSet::new();
    for t in 1..=600 {
        sim.step(TickInput::default());
        if let Some(k) = obj(&sim, id).anim.key() {
            poses.insert(k.to_string());
        }
        if state_of(&sim, id) == Some(EnemyState::Pace) {
            paced_at = Some(t);
            break;
        }
    }
    let t = paced_at.expect("he starts pacing");
    assert!(t >= wait - 1, "not before four seconds: {t}");
    assert!(poses.len() > 1, "his stand poses change: {poses:?}");
}

#[test]
fn the_cutthroat_draws_in_the_games_order() {
    let (mut sim, id) = lone("CutThroat", 3, 6000, 3);
    // Going idle: one of 0..100, then one of 0..2 below 33.
    for _ in 0..20 {
        let r = Rng::new(sim.rng().state()).below(100);
        let n = draws_of(&mut sim, |w| cutthroat::go_idle(w, id));
        assert_eq!(n, if r < 33 { 2 } else { 1 });
        sim.step(TickInput::default());
    }
    // Idle, a pose change (the shared timer has run out) in mode 6 with
    // the idle wait not up: one draw.
    sim.with_world(|w| {
        let now = w.now_ms;
        let d = enemy::data_mut(w, id).unwrap();
        d.pose = enemy::Timer::new(0, 0);
        d.mode = 6;
        d.pace_wait = enemy::Timer::new(now, 4000);
    });
    assert_eq!(draws_of(&mut sim, |w| cutthroat::idle_tick(w, id)), 1);
    // No pose change, mode 6, the wait up: the pacing draw only.
    sim.with_world(|w| {
        let d = enemy::data_mut(w, id).unwrap();
        d.pace_wait = enemy::Timer::new(0, 0);
    });
    let r = Rng::new(sim.rng().state()).below(100);
    assert_eq!(draws_of(&mut sim, |w| cutthroat::idle_tick(w, id)), 1);
    assert_eq!(
        state_of(&sim, id) == Some(EnemyState::Pace),
        r < cutthroat::PACE_CHANCE as u32
    );
    // Home in an outer box with Claw out of sight: one of 0..121, then
    // going idle's.
    sim.with_world(|w| {
        enemy::go_home(w, id);
        enemy::data_mut(w, id).unwrap().mode = 1;
        enemy::data_mut(w, id).unwrap().alerted = true;
    });
    let mut r = Rng::new(sim.rng().state());
    r.below(121);
    let idle_draw = r.below(100);
    let n = draws_of(&mut sim, |w| cutthroat::home_tick(w, id));
    assert_eq!(n, if idle_draw < 33 { 3 } else { 2 });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn a_cutthroat_is_lifted_overhead_and_recovers_from_his_first_throw() {
    let (mut sim, id) = lone("CutThroat", 3, 3000, 12);
    let claw = claw_id(&sim);
    sim.teleport_player(3060, 899);
    idle(&mut sim, 2);
    assert!(obj(&sim, id).special_flags & claw_sim::object::special::LIFTABLE != 0);
    sim.with_world(|w| lift::lifted(w, id, claw));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Lifted));
    assert_eq!(
        span(obj(&sim, id).move_rect),
        span(cutthroat::CARRIED_MOVE_RECT)
    );
    sim.with_world(|w| {
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_OVERHEAD_FRAME;
        claw_sim::logic::tick(w, id);
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::Flailing));
    sim.step(TickInput::default());
    let (c, o) = (sim.player().unwrap(), obj(&sim, id));
    assert_eq!((o.x, o.y), (c.x, c.y - 30), "30 px over his head");
    // Thrown: two bounces, a rest, and the fall costs him 4 of his 7.
    sim.with_world(|w| lift::thrown(w, id, 300, -650));
    let mut seen = Vec::new();
    for _ in 0..200 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id).unwrap();
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        if s == EnemyState::Home || s == EnemyState::Idle {
            break;
        }
    }
    assert_eq!(
        &seen[..4],
        &[
            EnemyState::Thrown,
            EnemyState::Bounce,
            EnemyState::Pause,
            EnemyState::Bounce
        ],
        "{seen:?}"
    );
    assert_eq!(obj(&sim, id).health, 3);
    assert_eq!(obj(&sim, id).type_flags, types::ENEMY, "an enemy again");
}

#[test]
fn a_robber_thief_is_carried_fifty_px_over_claws_head() {
    let (mut sim, id) = lone("RobberThief", 3, 3000, 12);
    let claw = claw_id(&sim);
    sim.teleport_player(3060, 899);
    idle(&mut sim, 2);
    sim.with_world(|w| {
        lift::lifted(w, id, claw);
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_OVERHEAD_FRAME;
        claw_sim::logic::tick(w, id);
    });
    sim.step(TickInput::default());
    let (c, o) = (sim.player().unwrap(), obj(&sim, id));
    assert_eq!((o.x, o.y), (c.x, c.y - 50));
    assert_eq!(span(o.move_rect), span(robberthief::CARRIED_MOVE_RECT));
}

#[test]
fn a_thrown_cutthroat_hurts_the_thieves_he_meets() {
    let mut sim = sim_with(
        arena(
            3,
            vec![
                record("CutThroat", 3000, 900),
                record("RobberThief", 3200, 900),
            ],
        ),
        12,
    );
    sim.teleport_player(3060, 899);
    idle(&mut sim, 3);
    let c = first(&sim, GameLogic::CutThroat);
    let r = first(&sim, GameLogic::RobberThief);
    let claw = claw_id(&sim);
    sim.with_world(|w| {
        lift::lifted(w, c, claw);
        w.objects.get_mut(claw).unwrap().frame = lift::CLAW_OVERHEAD_FRAME;
        claw_sim::logic::tick(w, c);
    });
    sim.step(TickInput::default());
    sim.with_world(|w| lift::thrown(w, c, 500, -300));
    for _ in 0..60 {
        sim.step(TickInput::default());
        if state_of(&sim, r) == Some(EnemyState::Dead) {
            break;
        }
    }
    assert_eq!(
        state_of(&sim, r),
        Some(EnemyState::Dead),
        "10 damage kills a 7-health thief"
    );
}

// ----------------------------------------------------------------------
// The robber thief

#[test]
fn the_robber_thief_shoots_on_sight_beyond_his_range_boxes() {
    // His limits cut his boxes to 1145..1355: Claw at 1000 is outside
    // every one, on screen and in sight.
    let mut sim = sim_with(
        arena(3, vec![bounded("RobberThief", 1250, 900, 1245, 1255)]),
        4,
    );
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::RobberThief);
    let shot = shots_over(&mut sim, 500, TickInput::default());
    assert!(shot.len() >= 4, "{}", shot.len());
    let o = obj(&sim, id);
    for (_, spec, cues) in &shot {
        assert_eq!(spec.logic, robberthief::ARROW_LOGIC);
        assert_eq!(spec.image_set, robberthief::ARROW_IMAGE);
        assert_eq!(spec.damage, 10);
        assert_eq!(spec.type_flags, types::ENEMY_SHOT);
        assert_eq!((spec.vx, spec.vy), (-400, 0));
        assert_eq!((spec.x, spec.y), (o.x - 65, o.y - 15));
        assert_eq!(spec.owner, Some(id));
        assert_eq!(count(cues, robberthief::SHOT_SOUND), 1, "named low");
    }
    // 1500 ms apart, one tick more for each failed roll of 0..100.
    for p in shot.windows(2) {
        let gap = p[1].0 - p[0].0;
        assert!((84..=87).contains(&gap), "{gap}");
    }
    assert_eq!(obj(&sim, id).x, o.x, "he never walks up to Claw");
}

#[test]
fn the_robber_thief_shoots_low_at_a_crouching_claw() {
    let (mut sim, id) = lone("RobberThief", 3, 1250, 4);
    let shot = shots_over(&mut sim, 300, hold(DOWN));
    assert!(!shot.is_empty());
    let o = obj(&sim, id);
    for (_, spec, cues) in shot {
        assert_eq!((spec.x, spec.y), (o.x - 65, o.y + 20));
        assert_eq!(count(&cues, robberthief::SHOT_LOW_SOUND), 1, "named high");
    }
}

#[test]
fn the_robber_thief_has_no_blow_up_close() {
    let (mut sim, id) = lone("RobberThief", 3, 1030, 6);
    let mut fired = false;
    for _ in 0..400 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id);
        assert!(
            !matches!(
                s,
                Some(EnemyState::Strike | EnemyState::Advance | EnemyState::FastAdvance)
            ),
            "{s:?}"
        );
        assert_ne!(obj(&sim, id).attack_rect, robberthief::STRIKE);
        fired |= s == Some(EnemyState::Fire);
        clear_shots(&mut sim);
    }
    assert!(fired, "he shoots instead");
}

#[test]
fn a_hidden_claw_in_range_flips_the_robber_thief_and_he_speaks() {
    let mut sim = sim_with(walled(3, vec![record("RobberThief", 1250, 900)]), 5);
    sim.step(TickInput::default());
    let id = first(&sim, GameLogic::RobberThief);
    idle(&mut sim, 5);
    let x = obj(&sim, id).x;
    let mut last = None;
    let mut spoke = 0;
    for _ in 0..400 {
        sim.step(TickInput::default());
        let s = state_of(&sim, id);
        assert!(
            matches!(s, Some(EnemyState::Idle | EnemyState::Home)),
            "{s:?}"
        );
        assert_ne!(s, last, "a new state every tick");
        last = s;
        assert_eq!(obj(&sim, id).x, x);
        let c = cues(&sim);
        spoke += robberthief::IDLE_LINES
            .iter()
            .map(|k| count(&c, k))
            .sum::<usize>();
    }
    assert!(shots(&sim).is_empty());
    assert!(spoke > 0, "he says his lines");
}

#[test]
fn a_flinch_before_the_shot_frame_cancels_the_arrow() {
    let (mut sim, id) = lone("RobberThief", 3, 1250, 4);
    for _ in 0..50 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Fire) {
            break;
        }
    }
    assert_eq!(state_of(&sim, id), Some(EnemyState::Fire));
    assert!(shots(&sim).is_empty(), "the arrow has not left yet");
    let blow = attacker(&mut sim, types::PLAYER, 1);
    hit(&mut sim, id, blow);
    assert_eq!(state_of(&sim, id), Some(EnemyState::HitLow));
    for _ in 0..30 {
        sim.step(TickInput::default());
        if state_of(&sim, id) == Some(EnemyState::Fire) {
            break;
        }
        assert!(shots(&sim).is_empty(), "no arrow from the cancelled shot");
    }
}

#[test]
fn the_robber_thief_draws_in_the_games_order() {
    let (mut sim, id) = lone("RobberThief", 3, 6000, 3);
    // Going idle: one of 0..4 (his line), then one of 0..100.
    for _ in 0..10 {
        let pick = Rng::new(sim.rng().state()).below(4) as usize;
        assert_eq!(draws_of(&mut sim, |w| robberthief::go_idle(w, id)), 2);
        assert_eq!(
            data(&sim, id).human.sounds.idle_voice,
            robberthief::IDLE_LINES[pick]
        );
    }
    // Home with Claw out of sight: going idle's two.
    sim.with_world(|w| enemy::go_home(w, id));
    assert_eq!(draws_of(&mut sim, |w| robberthief::home_tick(w, id)), 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // Home with Claw in sight: the shot roll only.
    let (mut sim, id) = lone("RobberThief", 3, 1250, 3);
    sim.with_world(|w| enemy::go_home(w, id));
    assert_eq!(draws_of(&mut sim, |w| robberthief::home_tick(w, id)), 1);
}

// ----------------------------------------------------------------------
// Knives and arrows in flight

#[test]
fn knives_and_arrows_fly_flat_and_turn_their_boxes_when_flying_left() {
    let mut sim = sim_with(arena(3, vec![]), 1);
    for (image, right_box, left_box) in [
        (
            cutthroat::KNIFE_IMAGE,
            Rect::new(-13, -5, 14, 4),
            Rect::new(-14, -5, 13, 4),
        ),
        (
            robberthief::ARROW_IMAGE,
            Rect::new(-19, -3, 16, 2),
            Rect::new(-16, -3, 19, 2),
        ),
    ] {
        for (vx, want) in [(400, right_box), (-400, left_box)] {
            let id = sim
                .with_world(|w| {
                    projectile::spawn(
                        w,
                        ProjectileSpec {
                            logic: "Bullet",
                            x: 3000,
                            y: 900,
                            z: 3999,
                            vx,
                            damage: 10,
                            type_flags: types::ENEMY_SHOT,
                            image_set: image,
                            ..ProjectileSpec::default()
                        },
                    )
                })
                .unwrap();
            let o = obj(&sim, id);
            assert_eq!((o.move_rect, o.attack_rect), (want, want), "{image} {vx}");
            assert_eq!(o.hit_rect, Rect::UNSET, "nothing can hit it");
            assert_eq!(o.draw_flags & draw_flags::MIRROR != 0, vx < 0);
            assert_eq!(&*o.image_set, image);
            sim.step(TickInput::default());
            let x0 = obj(&sim, id).x;
            idle(&mut sim, 10);
            let moved = obj(&sim, id).x - x0;
            // 7.2 px a tick, flat.
            assert!((moved.abs() - 72).abs() <= 1, "{moved}");
            assert_eq!(obj(&sim, id).y, 900);
            sim.with_world(|w| {
                w.objects.remove(id);
            });
        }
    }
}

#[test]
fn a_knife_that_reaches_claw_costs_him_ten_and_is_gone() {
    let (mut sim, id) = lone("CutThroat", 3, 1250, 4);
    let mut seen = BTreeSet::new();
    let mut knife = None;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if let Some((k, _)) = new_shots(&sim, &mut seen).first() {
            knife = Some(*k);
            break;
        }
    }
    let knife = knife.expect("a knife");
    let before = sim.claw().stats.health;
    for _ in 0..60 {
        sim.step(TickInput::default());
        if sim.objects().get(knife).is_none() {
            break;
        }
    }
    assert!(sim.objects().get(knife).is_none(), "the knife is gone");
    assert_eq!(sim.claw().stats.health, before - 10);
    let _ = id;
}

// ----------------------------------------------------------------------
// Death and loot

#[test]
fn a_dead_thief_drops_his_loot_and_only_the_cutthroat_cries_out() {
    let mut cut_cried = false;
    for seed in 1..30 {
        for logic in ["CutThroat", "RobberThief"] {
            let mut rec = record(logic, 3000, 900);
            rec.record[35] = 33;
            rec.record[15] = 24;
            let mut sim = sim_with(arena(3, vec![rec]), seed);
            sim.teleport_player(3250, 899);
            idle(&mut sim, 2);
            let g = if logic == "CutThroat" {
                GameLogic::CutThroat
            } else {
                GameLogic::RobberThief
            };
            let id = first(&sim, g);
            let shot = attacker(&mut sim, types::LEVEL_SHOT, 8);
            hit(&mut sim, id, shot);
            assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
            let d = data(&sim, id);
            assert_eq!(
                d.death_anim,
                if logic == "CutThroat" {
                    cutthroat::DEATH_ANIM
                } else {
                    robberthief::DEATH_ANIM
                }
            );
            let drops: Vec<i32> = sim
                .objects()
                .all()
                .filter_map(|(_, o)| match &o.data {
                    ObjectData::Pickup(p) => p.drop_code,
                    _ => None,
                })
                .collect();
            assert_eq!(drops, vec![33, 24], "{logic}");
            // The hit's sounds come out with the next tick's.
            sim.step(TickInput::default());
            let c = cues(&sim);
            assert_eq!(count(&c, enemy::DROP_SOUND), 1);
            if logic == "CutThroat" {
                cut_cried |= count(&c, cutthroat::DEATH_SOUND) == 1;
            } else {
                assert!(
                    !c.iter().any(|k| k.starts_with("LEVEL_ROBBERTHIEF_")),
                    "{c:?}"
                );
            }
        }
    }
    assert!(cut_cried);
}
