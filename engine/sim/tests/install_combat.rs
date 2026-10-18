//! Claw's arsenal on the first level of your own install (skipped without
//! `CLAW_INSTALL`): every animation his attacks and special states use
//! exists and reports its events, a pistol shot asks for its bullet with
//! the right speed and damage and spends a shot, and a run that uses every
//! weapon traces the same at 60, 144 and 240 Hz.

use std::collections::BTreeSet;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{AudioCue, Buttons, TickInput, Weapon};
use claw_sim::anim::AnimPlayer;
use claw_sim::logic::projectile::{ProjectileData, ProjectileSpec};
use claw_sim::object::types;
use claw_sim::player::attack::{self, MELEE, SHOTS, kind};
use claw_sim::player::{self, special, state};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{InputLatch, InputScript, ObjectData, Sim, SimConfig, SimLevel};

const JUMP: u32 = 0x1;
const ATTACK: u32 = 0x2;
const PROJECTILE: u32 = 0x4;
const TOGGLE: u32 = 0x8;
const MAGIC: u32 = 0x80;
const DYNAMITE: u32 = 0x100;
const DOWN: u32 = 0x800_0000;

fn level1() -> Option<Arc<SimLevel>> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(Arc::new(
        SimLevel::from_level(&level, &store).expect("level 1 prepares"),
    ))
}

/// The events an animation reports while it plays (at most 30 s).
fn events_of(level: &SimLevel, key: &str) -> BTreeSet<i32> {
    let anim = level.animations.get(key).expect("animation").clone();
    let set = claw_sim::level::runtime_image_set(player::IMAGE_SET);
    let frames = level.image_frames(&set);
    let mut p = AnimPlayer::default();
    p.set(anim);
    let mut frame = 0;
    let mut seen = BTreeSet::new();
    for _ in 0..30_000 / 18 {
        let e = p.advance_in(18, frame, false, frames);
        if let Some(f) = e.frame {
            frame = f;
        }
        seen.insert(e.event);
        if p.is_done() {
            break;
        }
    }
    seen
}

#[test]
fn every_attack_animation_exists_and_reports_its_events() {
    let Some(level) = level1() else { return };
    // (animation, events it must report)
    let mut wanted: Vec<(&str, Vec<i32>)> = Vec::new();
    for m in &MELEE {
        wanted.push((m.anim, vec![attack::EVENT_STRIKE]));
        if let Some(s) = m.sword_anim {
            wanted.push((s, vec![attack::EVENT_SWORD_SHOT, attack::EVENT_STRIKE]));
        }
    }
    for s in &SHOTS {
        let main = match s.kind {
            kind::PISTOL | kind::DUCK_PISTOL => vec![attack::EVENT_RELEASE, attack::EVENT_REFIRE],
            kind::DYNAMITE | kind::DUCK_DYNAMITE => vec![attack::EVENT_CHARGE],
            _ => vec![attack::EVENT_RELEASE],
        };
        wanted.push((s.anim, main));
        let empty = if s.kind == kind::PISTOL {
            vec![attack::EVENT_REFIRE]
        } else {
            Vec::new()
        };
        wanted.push((s.empty_anim, empty));
        if let Some(post) = s.post_anim {
            wanted.push((post, vec![attack::EVENT_RELEASE]));
        }
    }
    for key in special::HIT_ANIMS {
        wanted.push((key, vec![special::KNOCKBACK_EVENT]));
    }
    wanted.push((special::CARRY_WALK_ANIM, vec![special::CARRY_STEP_EVENT]));
    wanted.push((special::THROW_ANIM, vec![special::THROW_EVENT]));
    for key in [
        special::LIFT_ANIM,
        special::LIFT_LOW_ANIM,
        special::CARRY_ANIM,
    ] {
        wanted.push((key, Vec::new()));
    }
    let missing: Vec<&str> = wanted
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !level.animations.contains_key(*k))
        .collect();
    assert!(missing.is_empty(), "missing animations: {missing:?}");
    for (key, events) in &wanted {
        let seen = events_of(&level, key);
        for e in events {
            assert!(seen.contains(e), "{key} reports event {e}: {seen:?}");
        }
        let empty = SHOTS.iter().any(|s| s.empty_anim == *key);
        if empty {
            assert!(
                !seen.contains(&attack::EVENT_RELEASE),
                "{key}: an empty weapon launches nothing"
            );
        }
    }
    // Rapid fire rewinds to the element before the shot.
    for s in SHOTS.iter().filter(|s| s.weapon == Weapon::Pistol) {
        let a = &level.animations[s.anim];
        if let Some(next) = a.frames.get(s.rewind_to + 1)
            && a.frames.iter().any(|f| f.event == attack::EVENT_REFIRE)
        {
            assert_eq!(next.event, attack::EVENT_RELEASE, "{}", s.anim);
        }
    }
}

fn shots(sim: &Sim) -> Vec<ProjectileSpec> {
    let mut v: Vec<(u32, ProjectileSpec)> = sim
        .objects()
        .all()
        .filter_map(|(_, o)| match &o.data {
            ObjectData::Projectile(ProjectileData { spec: Some(s), .. }) => Some((o.serial, *s)),
            _ => None,
        })
        .collect();
    // In the order they were made: a new object may take a lower slot,
    // and id, freed by a removed one.
    v.sort_by_key(|(serial, _)| *serial);
    v.into_iter().map(|(_, s)| s).collect()
}

fn new_sim(level: &Arc<SimLevel>) -> Sim {
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    sim
}

fn collect(sim: &Sim, out: &mut BTreeSet<String>) {
    for c in sim.last_audio() {
        if let AudioCue::Play(s) = c {
            out.insert(s.key.clone());
        }
    }
}

#[test]
fn a_pistol_shot_asks_for_its_bullet_and_spends_a_shot() {
    let Some(level) = level1() else { return };
    let mut sim = new_sim(&level);
    assert_eq!(sim.player().unwrap().state, state::STAND);
    assert_eq!(sim.stats().weapon, Weapon::Pistol);
    let ammo = sim.stats().pistol;
    assert!(ammo > 0);
    sim.step(TickInput {
        held: Buttons(PROJECTILE),
        pressed: Buttons(PROJECTILE),
    });
    assert_eq!(sim.claw().attack.kind, kind::PISTOL);
    let mut sounds = BTreeSet::new();
    for _ in 0..60 {
        if !shots(&sim).is_empty() {
            break;
        }
        sim.step(TickInput::default());
        collect(&sim, &mut sounds);
    }
    let o = sim.player().unwrap();
    let all = shots(&sim);
    assert_eq!(all.len(), 1, "one bullet");
    let s = all[0];
    let dir = if sim.claw().facing_right { 1 } else { -1 };
    assert_eq!(s.logic, "PistolBullet");
    assert_eq!((s.vx, s.vy), (attack::SHOT_SPEED * dir, 0));
    assert_eq!(s.damage, attack::PISTOL_DAMAGE);
    assert_eq!(s.damage, 8);
    assert_eq!(s.vx.abs(), 700);
    assert_eq!(
        (s.type_flags, s.hit_type_flags),
        (types::PISTOL_SHOT, types::ENEMY)
    );
    assert_eq!((s.x, s.y, s.z), (o.x + 71 * dir, o.y - 21, o.z - 1));
    assert_eq!(sim.stats().pistol, ammo - 1, "a shot is spent");
    for _ in 0..60 {
        sim.step(TickInput::default());
        collect(&sim, &mut sounds);
    }
    assert!(!sim.claw().attacking);
    let missing: Vec<&String> = sounds
        .iter()
        .filter(|k| !level.sounds.contains_key(*k))
        .collect();
    assert!(missing.is_empty(), "unregistered sounds: {missing:?}");
}

/// A run with every weapon: pistol with rapid fire, toggles, magic, a
/// charged dynamite throw, crouching shots, blows and a shot in the air.
fn arsenal_script() -> InputScript {
    let mut held: Vec<u32> = vec![0; 20];
    let tap = |held: &mut Vec<u32>, b: u32, gap: usize| {
        held.extend([b; 2]);
        held.extend(std::iter::repeat_n(0, gap));
    };
    // Pistol, with two rapid-fire presses inside the shot's animation.
    tap(&mut held, PROJECTILE, 6);
    tap(&mut held, PROJECTILE, 4);
    tap(&mut held, PROJECTILE, 80);
    tap(&mut held, TOGGLE, 5);
    tap(&mut held, PROJECTILE, 80);
    tap(&mut held, MAGIC, 80);
    held.extend([DYNAMITE; 40]);
    held.extend([0; 80]);
    tap(&mut held, ATTACK, 40);
    held.extend([DOWN; 10]);
    held.extend([DOWN | PROJECTILE; 2]);
    held.extend([DOWN; 60]);
    held.extend([0; 10]);
    held.extend([JUMP; 4]);
    held.extend([JUMP | PROJECTILE; 2]);
    held.extend([0; 80]);
    tap(&mut held, TOGGLE, 3);
    tap(&mut held, TOGGLE, 3);
    tap(&mut held, ATTACK, 60);
    InputScript::from_held(&held)
}

fn trace_direct(sim: &mut Sim, script: &InputScript) -> String {
    let rows: Vec<TraceRow> = script
        .ticks
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            sim.step(t);
            TraceRow::capture(sim, i as u64, "script")
        })
        .collect();
    trace::to_csv(&rows)
}

/// The script's button changes seen by a latch at a simulated display rate.
fn trace_at_rate(sim: &mut Sim, script: &InputScript, hz: u64) -> String {
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
            rows.push(TraceRow::capture(sim, tick, "script"));
            tick += 1;
        }
        frame += 1;
    }
    trace::to_csv(&rows)
}

#[test]
fn the_arsenal_traces_the_same_at_60_144_and_240_hz() {
    let Some(level) = level1() else { return };
    let script = arsenal_script();
    let (base, used) = {
        let mut sim = new_sim(&level);
        let start = sim.stats().clone();
        let csv = trace_direct(&mut sim, &script);
        let end = sim.stats().clone();
        let used = (
            start.pistol - end.pistol,
            start.magic - end.magic,
            start.dynamite - end.dynamite,
        );
        (csv, used)
    };
    assert!(used.0 >= 2, "pistol shots spent: {used:?}");
    assert!(used.1 >= 1, "magic spent: {used:?}");
    assert!(used.2 >= 1, "dynamite spent: {used:?}");
    for hz in [60, 144, 240] {
        let mut sim = new_sim(&level);
        assert_eq!(trace_at_rate(&mut sim, &script, hz), base, "{hz} Hz");
    }
}
