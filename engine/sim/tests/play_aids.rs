//! The play-test aids on synthetic levels: with invincibility nothing that
//! hurts Claw hurts him (an enemy's touch, a shot, the level's own shots
//! and an explosion), while death tiles still end his
//! life; with infinite lives a death never lowers the lives count. With
//! both off the game plays as without them.

mod common;

use std::sync::Arc;

use claw_frame::{AudioCue, GameEvent, SoundCue};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::effect::{self, EffectKind};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::tuning::PlayAids;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// A flat floor (feet rest on y = 959) with death tiles in columns 25 to
/// 27 (x 1600 to 1791) and a checkpoint at x = 1210; Claw starts at
/// x = 1000.
fn level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(25..28, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    l.objects.push(ObjectSpec::at("Checkpoint", 1210, 899, 5));
    l
}

fn aids(invincible: bool, infinite_lives: bool) -> PlayAids {
    PlayAids {
        invincible,
        infinite_lives,
    }
}

fn sim_with(aids: PlayAids) -> Sim {
    let mut sim = pressing_sim(
        level(),
        SimConfig {
            aids,
            ..SimConfig::default()
        },
    );
    // Let Claw land and stand.
    idle(&mut sim, 40);
    sim
}

/// An object of kind `type_flags` dealing `damage`, touching Claw.
fn toucher(sim: &mut Sim, type_flags: u32, damage: i32) -> ObjectId {
    let (x, y) = {
        let o = sim.player().unwrap();
        (o.x, o.y)
    };
    let mut o = Object::new(0, x, y, 10, Arc::from("Toucher"), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = type_flags;
    o.damage = damage;
    o.attack_rect = Rect::new(-20, -20, 20, 20);
    sim.spawn(o)
}

/// Claw's position, state and health, and the window after a hit.
fn claw_now(sim: &Sim) -> (i32, i32, i32, i32, i32) {
    let o = sim.player().unwrap();
    (
        o.x,
        o.y,
        o.state,
        sim.stats().health,
        sim.stats().invulnerable_ms,
    )
}

fn sounds(sim: &Sim) -> Vec<String> {
    sim.snapshot()
        .audio
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(SoundCue { key, .. }) => Some(key.clone()),
            _ => None,
        })
        .collect()
}

/// Runs `n` idle ticks, checking after each that Claw stands where he
/// stood, unhurt and silent.
fn untouched_for(sim: &mut Sim, n: usize, what: &str) {
    let before = claw_now(sim);
    assert_eq!(before.2, state::STAND, "{what}: Claw stands");
    for i in 0..n {
        sim.step(TickInput::default());
        assert_eq!(claw_now(sim), before, "{what}: tick {i}");
        let s = sounds(sim);
        assert!(
            !s.iter()
                .any(|k| k.starts_with("GAME_HIT") || k.starts_with("CLAW_")),
            "{what}: no hit or pain sound: {s:?}"
        );
    }
}

#[test]
fn both_aids_are_off_by_default() {
    assert_eq!(SimConfig::default().aids, PlayAids::NONE);
    assert_eq!(PlayAids::default(), PlayAids::NONE);
    assert!(!PlayAids::NONE.any());
    assert_eq!(aids(true, true).names(), ["invincible", "infinite_lives"]);
    assert_eq!(aids(false, true).names(), ["infinite_lives"]);
    assert_eq!(aids(true, false).union(aids(false, true)), aids(true, true));
}

#[test]
fn without_the_aid_the_same_touches_hurt() {
    for (kind, damage) in [
        (types::ENEMY, 10),
        (types::ENEMY_SHOT, 15),
        (types::LEVEL_SHOT, 20),
    ] {
        let mut sim = sim_with(PlayAids::NONE);
        toucher(&mut sim, kind, damage);
        sim.step(TickInput::default());
        assert_eq!(sim.stats().health, 100 - damage, "kind {kind:#x}");
        assert_eq!(sim.player().unwrap().state, state::HIT);
    }
}

#[test]
fn an_invincible_claw_takes_nothing_from_an_enemys_touch() {
    let mut sim = sim_with(aids(true, false));
    let e = toucher(&mut sim, types::ENEMY, 10);
    untouched_for(&mut sim, 60, "an enemy's body");
    // Even a touch that would take all his health.
    sim.with_world(|w| w.objects.get_mut(e).unwrap().damage = 500);
    untouched_for(&mut sim, 10, "a deadly touch");
}

#[test]
fn an_invincible_claw_lets_shots_fly_through_him() {
    for (kind, what) in [
        (types::ENEMY_SHOT, "an enemy's shot"),
        (types::LEVEL_SHOT, "the level's own shot (a hazard)"),
    ] {
        let mut sim = sim_with(aids(true, false));
        let shot = toucher(&mut sim, kind, 30);
        untouched_for(&mut sim, 10, what);
        let o = sim.objects().get(shot).expect("the shot is still there");
        assert_eq!(o.flags & flags::DESTROY, 0, "{what}: not used up");
    }
}

#[test]
fn an_invincible_claw_takes_nothing_from_an_explosion() {
    // Without the aid the explosion hurts, as a check of the set-up.
    for invincible in [false, true] {
        let mut sim = sim_with(aids(invincible, false));
        let (x, y) = {
            let o = sim.player().unwrap();
            (o.x, o.y)
        };
        sim.with_world(|w| {
            effect::spawn(
                w,
                EffectKind::Explosion {
                    damage: 40,
                    radius: 80,
                    source: None,
                },
                x,
                y,
            )
        })
        .expect("an explosion");
        if invincible {
            untouched_for(&mut sim, 5, "an explosion");
        } else {
            idle(&mut sim, 5);
            assert_eq!(sim.stats().health, 60, "the explosion hurts");
        }
    }
}

#[test]
fn an_invincible_claw_moves_as_if_nothing_touched_him() {
    // The same walk with an enemy on the way and without one.
    let walk = |aid: PlayAids, enemy: bool| {
        let mut sim = sim_with(aid);
        if enemy {
            let e = toucher(&mut sim, types::ENEMY, 10);
            sim.with_world(|w| {
                w.objects.get_mut(e).unwrap().attack_rect = Rect::new(0, -40, 300, 40);
            });
        }
        sim.step(press(RIGHT));
        let mut path = Vec::new();
        for _ in 0..30 {
            sim.step(hold(RIGHT));
            let o = sim.player().unwrap();
            path.push((o.x, o.y, o.state, sim.stats().health));
        }
        path
    };
    let invincible = aids(true, false);
    assert_eq!(walk(invincible, true), walk(invincible, false));
    assert_ne!(
        walk(PlayAids::NONE, true),
        walk(PlayAids::NONE, false),
        "the enemy is on the way"
    );
}

#[test]
fn death_tiles_still_end_an_invincible_claws_life() {
    let mut sim = sim_with(aids(true, false));
    let mut events = Vec::new();
    sim.step(press(RIGHT));
    for _ in 0..400 {
        let snap = sim.step(hold(RIGHT));
        events.extend(snap.events);
        if sim.stats().lives < 7 {
            break;
        }
    }
    assert_eq!(sim.claw().death_tile_hits, 1);
    assert_eq!(sim.stats().lives, 6, "a life is lost");
    assert!(
        events.contains(&GameEvent::ClawDied { lives_left: 6 }),
        "{events:?}"
    );
}

#[test]
fn with_infinite_lives_a_death_keeps_the_lives_count() {
    let mut sim = sim_with(aids(false, true));
    let mut died = Vec::new();
    // More deaths than a new game has lives, the last ones on one life.
    for n in 0..9 {
        if n == 7 {
            sim.with_world(|w| w.claw.stats.lives = 1);
        }
        let lives = sim.stats().lives;
        let e = toucher(&mut sim, types::ENEMY, 500);
        for _ in 0..300 {
            let snap = sim.step(TickInput::default());
            for ev in snap.events {
                match ev {
                    GameEvent::ClawDied { lives_left } => died.push(lives_left),
                    GameEvent::GameOver => panic!("game over with infinite lives"),
                    _ => {}
                }
            }
            if sim.respawn_pending() {
                break;
            }
        }
        assert!(sim.respawn_pending(), "death {n}: Claw died");
        assert_eq!(sim.stats().lives, lives, "death {n}: no life lost");
        assert_eq!(sim.stats().attempt, n + 1, "death {n}: counted");
        sim.with_world(|w| w.destroy(e));
        sim.step(TickInput::default());
        let o = sim.player().unwrap();
        assert_eq!(o.state, state::STAND, "death {n}: back and standing");
        assert_eq!(sim.stats().health, 100);
    }
    assert_eq!(died, [7, 7, 7, 7, 7, 7, 7, 1, 1]);
}

#[test]
fn with_infinite_lives_a_death_tile_respawns_at_the_checkpoint() {
    let mut sim = sim_with(aids(true, true));
    sim.step(press(RIGHT));
    for _ in 0..400 {
        sim.step(hold(RIGHT));
        if sim.respawn_pending() {
            break;
        }
    }
    assert!(sim.respawn_pending(), "the death tile kills");
    assert_eq!(sim.claw().checkpoint, Some((1210, 867)));
    assert_eq!(sim.stats().lives, 7);
    sim.step(TickInput::default());
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y, o.state), (1210, 899, state::STAND));
}

#[test]
fn where_nothing_hurts_claw_the_aids_change_nothing() {
    // Walking both ways and jumping on clear floor, away from the death
    // tiles: nothing hurts Claw, so the game plays the same with both aids
    // on as with both off, tick for tick.
    let run = |aids: PlayAids| {
        let mut sim = pressing_sim(
            level(),
            SimConfig {
                aids,
                ..SimConfig::default()
            },
        );
        let mut out = Vec::new();
        for k in 0..300u32 {
            let input = match k % 100 {
                0..30 => hold(RIGHT),
                50..80 => hold(LEFT),
                _ => press(JUMP),
            };
            let snap = sim.step(input);
            let o = sim.player().unwrap();
            assert_eq!(sim.stats().health, 100, "nothing hurts Claw: tick {k}");
            assert!(o.x < 1600, "clear of the death tiles: tick {k}");
            out.push((
                o.x,
                o.y,
                o.state,
                sim.stats().lives,
                sim.rng_state(),
                snap.events.len(),
            ));
        }
        out
    };
    let off = run(PlayAids::NONE);
    assert!(
        off.iter().any(|r| r.0 != off[0].0) && off.iter().any(|r| r.1 != off[0].1),
        "Claw walked and jumped"
    );
    assert_eq!(run(aids(true, true)), off);
    assert_eq!(run(aids(true, false)), off);
    assert_eq!(run(aids(false, true)), off);
}

#[test]
fn the_aids_reach_every_logic_call() {
    for a in [
        PlayAids::NONE,
        aids(true, false),
        aids(false, true),
        aids(true, true),
    ] {
        let mut sim = sim_with(a);
        assert_eq!(sim.with_world(|w| w.aids), a);
    }
}
