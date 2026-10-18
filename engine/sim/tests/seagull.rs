//! The seagull on synthetic levels: its patrol, its dive at Claw, the
//! climb back, its beak, and its death.

mod common;

use claw_sim::claw_frame::AudioCue;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{self, EnemyState, seagull};
use claw_sim::logic::{self, ObjectData};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, types};
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A long flat floor (top at y 960) with a wall column through the sky at
/// x 1664 to 1727, and Claw starting at (1000, 600).
fn sky_level() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(200); 15];
    for r in rows.iter_mut().take(10).skip(5) {
        r.replace_range(26..27, "#");
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// A seagull record at (x, y) patrolling from `x_min` to `x_max`, facing
/// right when `mirrored`.
fn gull(x: i32, y: i32, (x_min, x_max): (i32, i32), mirrored: bool) -> ObjectSpec {
    let mut s = ObjectSpec::at("Seagull", x, y, 0);
    s.record[11] = if mirrored { 2 } else { 0 };
    s.record[enemy::field::X_MIN] = x_min;
    s.record[enemy::field::X_MAX] = x_max;
    s
}

/// A game of `level` with `birds` placed, after Claw has landed.
fn game(mut level: SimLevel, birds: &[ObjectSpec]) -> (Sim, Vec<ObjectId>) {
    level.objects.extend(birds.iter().cloned());
    let mut sim = Sim::new(level, SimConfig::default());
    idle(&mut sim, 1);
    let ids = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "Seagull")
        .map(|(id, _)| id)
        .collect();
    (sim, ids)
}

fn bird(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the seagull")
}

fn state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(bird(sim, id).state)
}

fn velocity(sim: &Sim, id: ObjectId) -> (f64, f64) {
    let d = enemy::data_of(bird(sim, id)).expect("built");
    (d.vx, d.vy)
}

/// Lets Claw land and stand still; returns where he stands.
fn settle(sim: &mut Sim) -> (i32, i32) {
    for _ in 0..80 {
        sim.step(TickInput::default());
    }
    let c = sim.player().expect("Claw");
    (c.x, c.y)
}

#[test]
fn a_seagull_is_built_as_a_flyer_in_front_of_claw() {
    let (sim, ids) = game(sky_level(), &[gull(1600, 500, (1500, 1800), false)]);
    let o = bird(&sim, ids[0]);
    assert_eq!(o.kind, LogicKind::Game(logic::GameLogic::Seagull));
    assert_eq!(state(&sim, ids[0]), Some(EnemyState::Idle));
    assert_eq!((o.health, o.damage), (1, 10));
    assert_eq!(o.z, seagull::DEFAULT_Z);
    assert_eq!(o.hit_rect, seagull::HIT_RECT);
    assert!(!o.attack_rect.is_set(), "harmless while patrolling");
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.hit_type_flags, seagull::HIT_BY);
    assert!(matches!(o.data, ObjectData::Seagull(_)));
    let d = enemy::data_of(o).unwrap();
    assert_eq!(d.ay, 0.0, "no gravity");
    assert_eq!(d.bounds, (1500, 1800));
    assert_eq!(d.bounds_y, enemy::UNBOUNDED);
    // Not on its right limit: it heads right, facing right.
    assert_eq!(d.vx, 120.0);
    assert!(enemy::faces_right(o));
}

#[test]
fn it_patrols_at_120_px_s_through_walls_and_turns_exactly_on_its_limits() {
    // Claw is outside the limits: no dive.
    let (mut sim, ids) = game(sky_level(), &[gull(1600, 500, (1500, 1800), false)]);
    let id = ids[0];
    let mut xs = Vec::new();
    for _ in 0..400 {
        sim.step(TickInput::default());
        let o = bird(&sim, id);
        assert_eq!(o.y, 500, "it never falls");
        assert_eq!(state(&sim, id), Some(EnemyState::Idle));
        xs.push(o.x);
    }
    // 2.16 px a tick: by the 50th row it has flown 51 ticks (one before
    // them), 110.16 px.
    assert_eq!(xs[49] - 1600, 110);
    assert_eq!(xs[50] - xs[49], 2);
    // Through the wall at x 1664..1727 without stopping.
    assert!(xs.contains(&1700));
    // It stands exactly on 1800, turns and heads back, and turns again on
    // 1500.
    let right = xs.iter().position(|&x| x == 1800).expect("reaches 1800");
    assert!(xs[right + 1] < 1800 && xs[right + 2] < xs[right + 1]);
    assert!(!enemy::faces_right(bird(&sim, id)) || xs[399] > xs[398]);
    assert_eq!(*xs.iter().max().unwrap(), 1800);
    assert_eq!(*xs.iter().min().unwrap(), 1500);
}

#[test]
fn it_dives_at_claw_climbs_back_and_patrols_again() {
    let mut level = sky_level();
    level.start = (1300, 800);
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    let (cx, cy) = settle(&mut sim);
    // Its patrol line 143 px above Claw, from 30 px left of him to 250 px
    // right; it starts 10 px short of the right limit, heading right.
    let home = cy - 143;
    level
        .objects
        .push(gull(cx + 240, home, (cx - 30, cx + 250), true));
    let (mut sim, ids) = game(level, &[]);
    let id = ids[0];
    settle_claw_only(&mut sim);
    let health = sim.stats().health;
    let mut rows = Vec::new();
    let mut dives = Vec::new();
    for k in 0..400 {
        sim.step(TickInput::default());
        if sim
            .last_audio()
            .iter()
            .any(|c| matches!(c, AudioCue::Play(s) if s.key == seagull::DIVE_SOUND))
        {
            dives.push(k);
        }
        let o = bird(&sim, id);
        rows.push((
            state(&sim, id).unwrap(),
            o.x,
            o.y,
            velocity(&sim, id),
            o.attack_rect,
            sim.stats().health,
        ));
    }
    let first = |s: EnemyState| rows.iter().position(|r| r.0 == s).unwrap();
    let (strike, withdraw) = (first(EnemyState::Strike), first(EnemyState::Withdraw));
    // It turned on its right limit, then dived within a pulse of three
    // ticks.
    let turned = rows.iter().position(|r| r.1 == cx + 250).unwrap();
    assert!(strike > turned && strike <= turned + 4, "{turned} {strike}");
    // The dive: 600 px/s along the sum of both distances, aimed 40 px
    // above Claw, never re-aimed; the beak is its attack box, facing left.
    let (vx, vy) = rows[strike].3;
    assert!((vx.abs() + vy.abs() - 600.0).abs() < 1e-9);
    assert!(vx < 0.0 && vy > 0.0);
    for r in &rows[strike..withdraw] {
        assert_eq!(r.3, (vx, vy));
        assert_eq!(r.4, seagull::BEAK);
    }
    // The beak cost Claw 10 health once.
    assert_eq!(rows[withdraw].5, health - 10);
    assert_eq!(dives[0], strike, "a screech as it dives");
    // It withdraws on its left limit (it reached it before passing below
    // Claw's height), harmless, straight up at 600 px/s.
    assert_eq!(rows[withdraw].1, cx - 30);
    assert!(!rows[withdraw].4.is_set());
    assert_eq!(rows[withdraw].3, (0.0, -600.0));
    // Back on its patrol line: the last climb overshoots, the next tick
    // puts it back and it patrols right from the left limit.
    let idle = withdraw
        + rows[withdraw..]
            .iter()
            .position(|r| r.0 == EnemyState::Idle)
            .unwrap();
    assert!(rows[idle - 1].2 < home);
    assert_eq!(rows[idle].2, home);
    assert_eq!(rows[idle].3, (120.0, 0.0));
}

/// Steps until Claw stands still.
fn settle_claw_only(sim: &mut Sim) {
    let mut last = None;
    for _ in 0..80 {
        sim.step(TickInput::default());
        let now = sim.player().map(|o| (o.x, o.y, o.state));
        if now == last {
            return;
        }
        last = now;
    }
}

#[test]
fn it_dives_only_inside_the_band_and_towards_claw() {
    for (dy, toward, dives) in [
        (143, true, true),
        (100, true, false),
        (239, true, true),
        (240, true, false),
        (143, false, false),
    ] {
        let mut level = sky_level();
        level.start = (1300, 800);
        let mut sim = Sim::new(level.clone(), SimConfig::default());
        let (cx, cy) = settle(&mut sim);
        // Flying left from its right limit towards Claw, or right, away
        // from him, for 60 ticks before it reaches its right limit.
        let spec = if toward {
            gull(cx + 300, cy - dy, (cx - 300, cx + 300), false)
        } else {
            gull(cx + 160, cy - dy, (cx - 300, cx + 300), true)
        };
        level.objects.push(spec);
        let (mut sim, ids) = game(level, &[]);
        let mut seen = false;
        for _ in 0..60 {
            sim.step(TickInput::default());
            seen |= state(&sim, ids[0]) == Some(EnemyState::Strike);
        }
        assert_eq!(seen, dives, "{dy} px down, towards: {toward}");
    }
}

#[test]
fn one_blow_kills_it_and_its_body_falls_off_the_screen() {
    let mut spec = gull(1300, 700, (1100, 1500), false);
    spec.record[enemy::field::POWERUP] = 33;
    let (mut sim, ids) = game(sky_level(), &[spec]);
    let id = ids[0];
    idle(&mut sim, 3);
    let mut shot = Object::new(0, 1300, 700, 0, "Shot".into(), LogicKind::Inert);
    shot.type_flags = types::PISTOL_SHOT;
    shot.damage = 8;
    let shot = sim.spawn(shot);
    let before = sim.rng_state();
    sim.with_world(|w| logic::on_hit(w, id, shot));
    assert_ne!(sim.rng_state(), before, "the burst and the flinch draw");
    assert!(sim.objects().get(shot).unwrap().flags & flags::DESTROY != 0);
    let o = bird(&sim, id);
    assert_eq!(state(&sim, id), Some(EnemyState::Dead));
    assert!(o.flags & flags::HARMLESS != 0 && o.flags & flags::NO_COLLIDE != 0);
    assert_eq!(o.health, -7);
    let d = enemy::data_of(o).unwrap();
    assert_eq!((d.vy, d.ay), (-100.0, 2000.0));
    // Claw is to the left: it falls away to the right.
    assert_eq!(d.vx, 100.0);
    assert_eq!(d.bounds, enemy::UNBOUNDED);
    // A second blow does nothing.
    let rng = sim.rng_state();
    sim.with_world(|w| logic::on_hit(w, id, shot));
    assert_eq!(sim.rng_state(), rng);
    // It drops through the floor and is gone once off the screen.
    let mut gone = None;
    for k in 0..200 {
        sim.step(TickInput::default());
        if sim.objects().get(id).is_none() {
            gone = Some(k);
            break;
        }
        assert!(bird(&sim, id).y >= 690);
    }
    assert!(gone.is_some(), "removed off screen");
}

#[test]
fn a_seagull_is_never_lifted() {
    let (mut sim, ids) = game(sky_level(), &[gull(1300, 700, (1100, 1500), false)]);
    let id = ids[0];
    let claw = sim
        .objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .unwrap();
    let before = (bird(&sim, id).state, bird(&sim, id).special_flags);
    sim.with_world(|w| {
        logic::lifted(w, id, claw);
        logic::thrown(w, id, 300, -600);
        logic::dropped(w, id);
    });
    assert_eq!((bird(&sim, id).state, bird(&sim, id).special_flags), before);
}
