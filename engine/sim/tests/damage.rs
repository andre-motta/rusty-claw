//! Claw's health and progress on synthetic levels: damage and the window
//! after a hit, every kind of pickup and its cap, powerups, death, lives,
//! checkpoints, save points and what carries between levels.

mod common;

use std::sync::Arc;

use claw_frame::{
    AudioCue, GameEvent, MusicTrack, ScreenTransition, SoundCue, TransitionKind, Weapon,
};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::ObjectData;
use claw_sim::logic::effect::EffectKind;
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, special, types};
use claw_sim::player::death::{CLOSE_MS, HOLD_MS, OPEN_MS, powerup, ticks_of};
use claw_sim::player::hit::PickupFields;
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::{Carry, SavePos, Sim, SimConfig, SimLevel, StartState, TickInput};
use common::*;

// ---------------------------------------------------------------- helpers

/// A flat floor (feet rest on y = 959) with death tiles in columns 25 to
/// 27 (x 1600 to 1791) and `objects` placed on it; Claw starts at
/// x = 1000.
fn level(objects: &[(&str, i32, i32)]) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    let mut floor = "#".repeat(200);
    floor.replace_range(25..28, "xxx");
    rows.push(floor);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    l.sounds.insert("GAME_MAPPIECE".to_string(), Some(1520));
    for line in POWERUP_LINES {
        l.sounds.insert(line.to_string(), Some(LINE_MS));
    }
    for &(name, x, y) in objects {
        l.objects.push(ObjectSpec::at(name, x, y, 5));
    }
    l
}

/// Claw's powerup lines, registered in [`level`] at [`LINE_MS`] each.
const POWERUP_LINES: [&str; 4] = [
    "CLAW_1110064",
    "CLAW_1110001",
    "CLAW_1110022",
    "CLAW_1110012",
];
/// Length of each of Claw's lines in [`level`], ms.
const LINE_MS: u32 = 900;

fn sim_of(objects: &[(&str, i32, i32)]) -> Sim {
    Sim::new(level(objects), SimConfig::default())
}

/// An object of kind `type_flags` with `f`, touching Claw from `dx`
/// pixels to his right.
fn toucher(sim: &mut Sim, type_flags: u32, f: PickupFields, dx: i32) -> ObjectId {
    let (x, y) = {
        let o = sim.player().unwrap();
        (o.x + dx, o.y)
    };
    let mut o = Object::new(0, x, y, 10, Arc::from("Toucher"), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = type_flags;
    o.attack_rect = Rect::new(-20, -20, 20, 20);
    f.store(&mut o);
    sim.spawn(o)
}

fn pickup(kind: i32, points: i32) -> PickupFields {
    PickupFields {
        kind,
        points,
        ..PickupFields::default()
    }
}

fn special_pickup(code: u32, smarts: i32) -> PickupFields {
    PickupFields {
        kind: 53,
        points: code as i32,
        smarts,
        ..PickupFields::default()
    }
}

fn step(sim: &mut Sim) -> (Vec<AudioCue>, Vec<GameEvent>) {
    let snap = sim.step(TickInput::default());
    (snap.audio, snap.events)
}

fn played(audio: &[AudioCue], key: &str) -> bool {
    audio
        .iter()
        .any(|c| matches!(c, AudioCue::Play(SoundCue { key: k, .. }) if k == key))
}

fn music(audio: &[AudioCue]) -> Vec<MusicTrack> {
    audio
        .iter()
        .filter_map(|c| match c {
            AudioCue::Music { track, .. } => Some(*track),
            _ => None,
        })
        .collect()
}

fn removed_next(sim: &mut Sim, id: ObjectId) -> bool {
    let asked = sim
        .objects()
        .get(id)
        .is_some_and(|o| o.flags & flags::DESTROY != 0);
    step(sim);
    asked && sim.objects().get(id).is_none()
}

fn stays(sim: &mut Sim, id: ObjectId) -> bool {
    let o = sim.objects().get(id).unwrap();
    o.flags & (flags::DESTROY | flags::NO_COLLIDE) == 0
}

fn set_health(sim: &mut Sim, h: i32) {
    sim.with_world(|w| w.claw.stats.health = h);
}

/// Steps until `f` holds, at most `n` ticks; returns every event seen.
fn run_until(
    sim: &mut Sim,
    input: TickInput,
    n: usize,
    f: impl Fn(&Sim) -> bool,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    for _ in 0..n {
        let snap = sim.step(input);
        events.extend(snap.events);
        if f(sim) {
            return events;
        }
    }
    panic!("condition not reached in {n} ticks; events {events:?}");
}

// ------------------------------------------------------------ pickups

#[test]
fn health_is_capped_and_refused_at_full_health() {
    let mut sim = sim_of(&[]);
    set_health(&mut sim, 80);
    let p = toucher(&mut sim, types::HEALTH, pickup(26, 10), 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().health, 90);
    assert!(played(&audio, "GAME_MILK"));
    assert!(removed_next(&mut sim, p));
    let p = toucher(&mut sim, types::HEALTH, pickup(25, 25), 0);
    step(&mut sim);
    assert_eq!(sim.stats().health, 100, "capped");
    assert!(removed_next(&mut sim, p));
    // At full health a health pickup stays in the level.
    let p = toucher(&mut sim, types::HEALTH, pickup(24, 5), 0);
    let (audio, _) = step(&mut sim);
    step(&mut sim);
    assert_eq!(sim.stats().health, 100);
    assert!(audio.is_empty());
    assert!(stays(&mut sim, p));
    // The large potion is taken even at full health.
    sim.with_world(|w| w.destroy(p));
    step(&mut sim);
    let p = toucher(&mut sim, types::SUPER_HEALTH, pickup(0, 100), 0);
    step(&mut sim);
    assert_eq!(sim.stats().health, 100);
    assert!(removed_next(&mut sim, p));
}

#[test]
fn catnip_comes_from_large_health_pickups() {
    let mut sim = sim_of(&[]);
    let mut f = pickup(22, 15000);
    f.smarts = 12000;
    toucher(&mut sim, types::HEALTH, f, 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_kind, powerup::CATNIP);
    assert_eq!(sim.stats().powerup_ms, 12000);
    assert_eq!(music(&audio), vec![MusicTrack::Powerup]);
    assert!(played(&audio, "GAME_CATNMAG"));
    // Without a duration the amount is the duration; the same powerup
    // adds up.
    toucher(&mut sim, types::HEALTH, pickup(23, 30000), 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_ms, 12000 - 18 + 30000);
    assert!(music(&audio).is_empty(), "no restart of the tune");
}

#[test]
fn treasure_scores_counts_and_flies_to_the_status_bar() {
    let mut sim = sim_of(&[]);
    let coin = toucher(&mut sim, types::TREASURE, pickup(33, 100), 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().score, 100);
    assert!(played(&audio, "GAME_COIN"));
    let o = sim.objects().get(coin).unwrap();
    assert!(o.special_flags & special::FLYING_TO_HUD != 0);
    assert!(o.flags & flags::NO_COLLIDE != 0);
    assert!(o.flags & flags::DESTROY == 0, "the flight removes it");
    for (kind, points) in [(1, 500), (4, 1500), (41, 2500), (8, 5000), (12, 7500)] {
        toucher(&mut sim, types::TREASURE, pickup(kind, points), 0);
        step(&mut sim);
    }
    for (kind, points) in [(16, 10000), (46, 15000), (50, 25000), (6, 2500)] {
        toucher(&mut sim, types::TREASURE, pickup(kind, points), 0);
        step(&mut sim);
    }
    let s = sim.stats();
    assert_eq!(
        s.score,
        100 + 500 + 1500 + 2500 + 5000 + 7500 + 10000 + 15000 + 25000 + 2500
    );
    assert_eq!(
        s.treasure_level,
        [1, 1, 1, 1, 1, 1, 1, 1, 1],
        "necklaces are not counted"
    );
    assert_eq!(s.treasure_game, s.treasure_level);
}

#[test]
fn ammunition_is_capped_at_99() {
    let mut sim = sim_of(&[]);
    toucher(&mut sim, types::PISTOL_AMMO, pickup(21, 10), 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().pistol, 20);
    assert!(played(&audio, "GAME_AMMUNITION"));
    toucher(&mut sim, types::PISTOL_AMMO, pickup(19, 85), 0);
    step(&mut sim);
    assert_eq!(sim.stats().pistol, 99);
    toucher(&mut sim, types::MAGIC_AMMO, pickup(30, 35), 0);
    step(&mut sim);
    assert_eq!(sim.stats().magic, 40);
    toucher(&mut sim, types::MAGIC_AMMO, pickup(30, 70), 0);
    step(&mut sim);
    assert_eq!(sim.stats().magic, 99);
    // Dynamite is a special pickup: its smarts, or 5.
    let mut f = special_pickup(9904, 0);
    f.kind = 34;
    toucher(&mut sim, types::SPECIAL, f, 0);
    step(&mut sim);
    assert_eq!(sim.stats().dynamite, 8);
    f.smarts = 95;
    toucher(&mut sim, types::SPECIAL, f, 0);
    step(&mut sim);
    assert_eq!(sim.stats().dynamite, 99);
}

#[test]
fn extra_lives_are_capped_at_10_and_fly_to_the_status_bar() {
    let mut sim = sim_of(&[]);
    let mut f = special_pickup(9918, 0);
    f.kind = 55;
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().lives, 8);
    assert!(played(&audio, "GAME_EXTRALIFE"));
    assert!(sim.objects().get(p).unwrap().special_flags & special::FLYING_TO_HUD != 0);
    sim.with_world(|w| w.claw.stats.lives = 10);
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().lives, 10);
    assert!(!played(&audio, "GAME_EXTRALIFE"));
    assert!(sim.objects().get(p).unwrap().special_flags & special::FLYING_TO_HUD != 0);
}

#[test]
fn curses_are_removed_without_effect_or_sound() {
    let mut sim = sim_of(&[]);
    let mut f = special_pickup(9907, 0);
    f.kind = 37;
    let before = sim.stats().clone();
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    let (audio, _) = step(&mut sim);
    assert!(audio.is_empty(), "{audio:?}");
    assert_eq!(sim.stats(), &before);
    assert!(removed_next(&mut sim, p));
}

#[test]
fn timed_powerups_and_swords() {
    let mut sim = sim_of(&[]);
    toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::GHOST, 0),
        0,
    );
    let (audio, _) = step(&mut sim);
    assert_eq!(
        (sim.stats().powerup_kind, sim.stats().powerup_ms),
        (powerup::GHOST, 30000)
    );
    assert!(played(&audio, "CLAW_1110064"));
    assert!(played(&audio, "GAME_PICKUP1"));
    // A sword pickup with damage is a refill that stays. Its line is a
    // Claw line: refused while the ghost line still runs.
    let mut f = special_pickup(powerup::FIRE_SWORD, 20000);
    f.damage = 1;
    let sword = toucher(&mut sim, types::SPECIAL, f, 0);
    let (audio, _) = step(&mut sim);
    assert_eq!(
        (sim.stats().powerup_kind, sim.stats().powerup_ms),
        (powerup::FIRE_SWORD, 20000)
    );
    assert!(!played(&audio, "CLAW_1110001"), "{audio:?}");
    assert_eq!(music(&audio), vec![MusicTrack::Powerup]);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_ms, 20000, "touching it again refills");
    assert!(audio.is_empty(), "{audio:?}");
    assert!(stays(&mut sim, sword));
    sim.with_world(|w| w.destroy(sword));
    // Once the ghost line is over the next line is said.
    for _ in 0..ticks_of(LINE_MS) {
        step(&mut sim);
    }
    // Without damage the sword is used up.
    let ice = toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::ICE_SWORD, 0),
        0,
    );
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_kind, powerup::ICE_SWORD);
    assert!(played(&audio, "CLAW_1110012"));
    assert!(removed_next(&mut sim, ice));
}

#[test]
fn powerup_timer_cap_and_music() {
    let mut sim = sim_of(&[]);
    toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::VADER, 900_000),
        0,
    );
    let (audio, _) = step(&mut sim);
    assert_eq!(music(&audio), vec![MusicTrack::Powerup]);
    assert_eq!(sim.stats().powerup_ms, 900_000);
    step(&mut sim);
    assert_eq!(sim.stats().powerup_ms, 900_000 - 18, "counts down per tick");
    let shots = types::ENEMY | types::ENEMY_SHOT | types::LEVEL_SHOT;
    assert_eq!(
        sim.player().unwrap().hit_type_flags & shots,
        0,
        "Vader shields"
    );
    toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::VADER, 900_000),
        0,
    );
    step(&mut sim);
    assert_eq!(sim.stats().powerup_ms, 999_000, "stacking is capped");
    assert_eq!(sim.snapshot().hud.powerup.unwrap().remaining_ms, 999_000);
    sim.with_world(|w| w.claw.stats.powerup_ms = 30);
    step(&mut sim);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_kind, 0);
    assert_eq!(
        music(&audio),
        vec![MusicTrack::Level],
        "the level tune comes back"
    );
    assert_eq!(sim.player().unwrap().hit_type_flags & shots, shots);
    assert_eq!(sim.snapshot().hud.powerup, None);
    // A different powerup replaces the running one and restarts the tune.
    toucher(&mut sim, types::HEALTH, pickup(22, 15000), 0);
    step(&mut sim);
    toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::GHOST, 500),
        0,
    );
    let (audio, _) = step(&mut sim);
    assert_eq!(
        (sim.stats().powerup_kind, sim.stats().powerup_ms),
        (powerup::GHOST, 500)
    );
    assert_eq!(music(&audio), vec![MusicTrack::Powerup]);
}

#[test]
fn extra_life_every_500000_points() {
    let mut sim = sim_of(&[]);
    toucher(&mut sim, types::TREASURE, pickup(49, 499_999), 0);
    step(&mut sim);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().lives, 7);
    assert!(!played(&audio, "GAME_EXTRALIFE"));
    toucher(&mut sim, types::TREASURE, pickup(33, 100), 0);
    step(&mut sim);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().lives, 8);
    assert!(played(&audio, "GAME_EXTRALIFE"));
    assert_eq!(sim.stats().next_extra_life, 1_000_000);
    // A jump past two steps brings one life per tick; at the cap the
    // threshold still moves on.
    sim.with_world(|w| w.claw.stats.lives = 9);
    toucher(&mut sim, types::TREASURE, pickup(49, 1_000_000), 0);
    step(&mut sim);
    step(&mut sim);
    assert_eq!(sim.stats().lives, 10);
    step(&mut sim);
    assert_eq!(sim.stats().lives, 10);
    assert_eq!(sim.stats().next_extra_life, 2_000_000);
}

#[test]
fn end_of_level_reports_the_map_piece_length() {
    let mut sim = sim_of(&[]);
    let mut f = special_pickup(9901, 0);
    f.kind = 31;
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    let (audio, events) = step(&mut sim);
    assert_eq!(events, vec![GameEvent::LevelComplete { after_ms: 1520 }]);
    assert!(played(&audio, "GAME_MAPPIECE"));
    assert!(removed_next(&mut sim, p));
    // Claw stands still for two ticks while the level ends.
    let x = sim.player().unwrap().x;
    sim.step(hold(RIGHT));
    assert_eq!(sim.player().unwrap().x, x);
    sim.step(hold(RIGHT));
    assert!(sim.player().unwrap().x > x);
}

#[test]
fn warps_move_claw_and_may_stay() {
    let mut sim = sim_of(&[]);
    let mut f = special_pickup(9902, 1);
    f.kind = 32;
    f.target = (3000, 899);
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    step(&mut sim);
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y), (3000, 899));
    assert!(stays(&mut sim, p), "a warp with smarts stays");
    f.smarts = 0;
    let p = toucher(&mut sim, types::SPECIAL, f, 0);
    step(&mut sim);
    assert!(removed_next(&mut sim, p));
}

// ------------------------------------------------------------- damage

fn enemy(sim: &mut Sim, damage: i32) -> ObjectId {
    let id = toucher(sim, types::ENEMY, PickupFields::default(), 0);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().damage = damage);
    id
}

#[test]
fn enemy_contact_hurts_with_a_window_between_hits() {
    let mut sim = sim_of(&[]);
    enemy(&mut sim, 10);
    let mut health = Vec::new();
    let mut first_audio = Vec::new();
    for i in 0..60 {
        let (audio, _) = step(&mut sim);
        if i == 0 {
            first_audio = audio;
        }
        health.push(sim.stats().health);
    }
    assert_eq!(health[0], 90);
    let second = health.iter().position(|&h| h == 80).unwrap();
    assert_eq!(second, 48, "850 ms window: {health:?}");
    let hits = ["GAME_HIT1", "GAME_HIT2", "GAME_HIT3", "GAME_HIT4"];
    let lines = [
        "CLAW_1110048",
        "CLAW_1110049",
        "CLAW_1110050",
        "CLAW_1110051",
    ];
    assert!(hits.iter().any(|k| played(&first_audio, k)));
    assert!(lines.iter().any(|k| played(&first_audio, k)));
    assert!(!played(&first_audio, "CLAW_HIT"));
    // The hit burst is asked for next to Claw.
    let burst = sim
        .objects()
        .all()
        .find(|(_, o)| {
            matches!(&o.data, ObjectData::Effect(e) if e.request == Some(EffectKind::HitBurst))
        })
        .map(|(_, o)| (o.x, o.y))
        .expect("a hit burst");
    let o = sim.player().unwrap();
    assert!(
        (burst.0 - o.x).abs() <= 8 && (burst.1 - o.y).abs() <= 60,
        "{burst:?}"
    );
}

#[test]
fn shots_hurt_and_are_removed_except_the_levels_own() {
    let mut sim = sim_of(&[]);
    let a = toucher(&mut sim, types::ENEMY_SHOT, PickupFields::default(), 0);
    sim.with_world(|w| w.objects.get_mut(a).unwrap().damage = 15);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().health, 85);
    assert!(played(&audio, "CLAW_HIT"));
    let claw = sim.player().unwrap();
    assert_eq!(claw.hit_type_flags & types::CLAW_SHIELDED, 0);
    assert!(
        claw.hit_type_flags & types::ENEMY_SHOT != 0,
        "enemies' shots still hit"
    );
    assert!(removed_next(&mut sim, a));
    // Enemies' shots are not blocked by the window.
    let b = toucher(&mut sim, types::ENEMY_SHOT, PickupFields::default(), 0);
    step(&mut sim);
    assert_eq!(sim.stats().health, 84, "no damage counts as 1");
    assert!(removed_next(&mut sim, b));
    // The level's own shots fly on; they are blocked by the window.
    let c = toucher(&mut sim, types::LEVEL_SHOT, PickupFields::default(), 0);
    step(&mut sim);
    assert_eq!(sim.stats().health, 84);
    assert!(stays(&mut sim, c));
    idle(&mut sim, 50);
    assert_eq!(sim.stats().health, 83, "hits again once the window is over");
    sim.with_world(|w| w.destroy(c));
    idle(&mut sim, 50);
    // A shot that goes off on impact is left to itself.
    let d = toucher(&mut sim, types::ENEMY_SHOT, PickupFields::default(), 0);
    sim.with_world(|w| w.objects.get_mut(d).unwrap().special_flags = special::OWN_IMPACT);
    step(&mut sim);
    assert_eq!(sim.stats().health, 82);
    assert!(stays(&mut sim, d));
}

#[test]
fn harmless_enemies_and_negative_damage_do_nothing() {
    let mut sim = sim_of(&[]);
    let e = enemy(&mut sim, 10);
    sim.with_world(|w| w.objects.get_mut(e).unwrap().special_flags = special::HARMLESS_TOUCH);
    idle(&mut sim, 5);
    assert_eq!(sim.stats().health, 100);
    sim.with_world(|w| {
        let o = w.objects.get_mut(e).unwrap();
        o.special_flags = 0;
        o.damage = -5;
    });
    step(&mut sim);
    assert_eq!(sim.stats().health, 100);
    assert_eq!(sim.stats().invulnerable_ms, 850, "the window still opens");
}

// --------------------------------------------------- death and respawn

fn transition(sim: &Sim) -> Option<ScreenTransition> {
    sim.snapshot().transition
}

#[test]
fn losing_all_health_falls_out_of_view_then_respawns() {
    let mut sim = sim_of(&[]);
    let (x0, y0) = {
        let o = sim.player().unwrap();
        (o.x, o.y)
    };
    sim.step(hold(RIGHT));
    sim.step(hold(RIGHT));
    let e = enemy(&mut sim, 150);
    let (_, events) = step(&mut sim);
    assert!(events.is_empty());
    sim.with_world(|w| w.destroy(e));
    let o = sim.player().unwrap();
    assert_eq!(o.state, state::DEATH_FALL);
    assert_eq!(sim.stats().health, 0);
    assert_eq!(o.z, 8500);
    assert!(o.flags & flags::NO_COLLIDE != 0 && o.flags & flags::SAFE != 0);
    let y_dead = o.y;
    let died_at = {
        let mut ticks = 0;
        loop {
            let (audio, events) = step(&mut sim);
            ticks += 1;
            if !events.is_empty() {
                assert_eq!(events, vec![GameEvent::ClawDied { lives_left: 6 }]);
                assert!(played(&audio, "GAME_CIRCLEFADE"));
                break sim.tick() - 1;
            }
            assert!(ticks < 100);
        }
    };
    let o = sim.player().unwrap();
    assert!(
        o.y > y_dead + 700,
        "fell out of view: {} -> {}",
        y_dead,
        o.y
    );
    let t = transition(&sim).expect("closing circle");
    assert_eq!(
        (t.kind, t.closing, t.start_tick),
        (TransitionKind::Circle, true, died_at)
    );
    assert_eq!(t.duration_ms, CLOSE_MS);
    assert_eq!(
        (t.center.x, t.center.y),
        (f64::from(o.x + 30), f64::from(o.y + 55))
    );
    // The body stays put while the circle closes.
    let y = o.y;
    for _ in 1..ticks_of(CLOSE_MS) {
        step(&mut sim);
        assert_eq!(sim.player().unwrap().y, y);
        assert_eq!(sim.player().unwrap().state, state::DEATH_FALL);
    }
    let (audio, events) = step(&mut sim);
    assert!(events.is_empty());
    assert!(played(&audio, "GAME_FLAGWAVE"), "back at the level start");
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y, o.state), (x0, y0, state::STAND));
    assert_eq!(sim.stats().health, 100);
    assert_eq!(sim.stats().lives, 6);
    let respawned = sim.tick() - 1;
    assert_eq!(respawned, died_at + ticks_of(CLOSE_MS));
    let t = transition(&sim).expect("opening circle");
    assert!(!t.closing);
    assert_eq!(t.start_tick, respawned + ticks_of(HOLD_MS));
    assert_eq!(t.duration_ms, OPEN_MS);
    assert_eq!(
        (t.center.x, t.center.y),
        (f64::from(x0), f64::from(y0 - 32))
    );
    // Frozen and safe until the circle is open.
    let frozen = ticks_of(HOLD_MS) + ticks_of(OPEN_MS);
    for _ in 1..frozen {
        sim.step(hold(RIGHT));
        assert_eq!(sim.player().unwrap().x, x0);
        assert!(sim.player().unwrap().flags & flags::SAFE != 0);
    }
    sim.step(hold(RIGHT));
    assert_eq!(transition(&sim), None);
    assert!(sim.player().unwrap().flags & flags::SAFE == 0);
    assert!(sim.player().unwrap().x > x0, "moves again");
}

#[test]
fn death_on_a_death_tile_respawns_at_the_last_checkpoint() {
    let mut sim = sim_of(&[("Checkpoint", 1210, 899)]);
    let events = run_until(&mut sim, hold(RIGHT), 200, |s| {
        s.claw().checkpoint.is_some()
    });
    assert!(
        events.contains(&GameEvent::Checkpoint { x: 1210, y: 867 }),
        "{events:?}"
    );
    assert_eq!(sim.claw().checkpoint, Some((1210, 867)));
    // The flag rises once and waves.
    let flag = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "Checkpoint")
        .map(|(id, _)| id)
        .unwrap();
    step(&mut sim);
    assert_eq!(sim.objects().get(flag).unwrap().state, 1);
    // Walk on into the death tiles.
    let events = run_until(&mut sim, hold(RIGHT), 300, |s| {
        s.player().unwrap().state == state::DEATH
    });
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, GameEvent::Checkpoint { .. })),
        "fires once"
    );
    assert_eq!(sim.claw().death_tile_hits, 1);
    assert_eq!(sim.stats().attempt, 0, "not counted before the lives step");
    let events = run_until(&mut sim, TickInput::default(), 10, |s| s.stats().lives == 6);
    assert_eq!(events, vec![GameEvent::ClawDied { lives_left: 6 }]);
    assert_eq!(sim.stats().attempt, 1);
    let mut flag_sound = false;
    for _ in 0..ticks_of(CLOSE_MS) {
        let (audio, events) = step(&mut sim);
        assert!(events.is_empty(), "the lives step runs once");
        flag_sound |= played(&audio, "GAME_FLAGWAVE");
    }
    assert!(flag_sound, "the flag sound marks a checkpoint respawn");
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y, o.state), (1210, 899, state::STAND));
    assert_eq!(sim.stats().health, 100);
}

#[test]
fn without_level_logics_a_death_tile_is_only_counted() {
    let cfg = SimConfig {
        level_logics: false,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(level(&[]), cfg);
    run_until(&mut sim, hold(RIGHT), 300, |s| s.claw().death_tile_hits > 0);
    for _ in 0..120 {
        let snap = sim.step(hold(RIGHT));
        assert!(snap.events.is_empty(), "{:?}", snap.events);
        assert_eq!(snap.transition, None);
    }
    let o = sim.player().unwrap();
    assert!(o.state != state::DEATH && o.state != state::DEATH_FALL);
    assert!(o.x >= 1600, "on the death tiles: {}", o.x);
    let s = sim.stats();
    assert_eq!((s.health, s.lives, s.attempt), (100, 7, 0));
}

#[test]
fn the_last_life_ends_the_game() {
    let mut sim = sim_of(&[]);
    sim.with_world(|w| w.claw.stats.lives = 1);
    enemy(&mut sim, 100);
    let events = run_until(&mut sim, TickInput::default(), 100, |s| {
        s.stats().lives == 0
    });
    assert_eq!(events, vec![GameEvent::GameOver]);
    assert_eq!(transition(&sim), None);
    for _ in 0..300 {
        let (_, events) = step(&mut sim);
        assert!(events.is_empty());
    }
    assert_eq!(sim.player().unwrap().state, state::DEATH_FALL);
    assert_eq!(sim.stats().lives, 0);
}

#[test]
fn seven_lives_give_six_respawns() {
    let mut sim = sim_of(&[]);
    let mut died = Vec::new();
    let mut over = 0;
    for n in 0..7 {
        set_health(&mut sim, 1);
        let e = enemy(&mut sim, 5);
        let lives = sim.stats().lives;
        assert_eq!(sim.stats().attempt, n, "one attempt per death so far");
        let events = run_until(&mut sim, TickInput::default(), 200, |s| {
            s.stats().lives < lives
        });
        assert_eq!(
            sim.stats().attempt,
            n + 1,
            "counted at the step that takes the life"
        );
        sim.with_world(|w| w.destroy(e));
        for ev in events {
            match ev {
                GameEvent::ClawDied { lives_left } => died.push(lives_left),
                GameEvent::GameOver => over += 1,
                _ => {}
            }
        }
        if sim.stats().lives == 0 {
            break;
        }
        run_until(&mut sim, TickInput::default(), 300, |s| {
            s.snapshot().transition.is_none()
        });
    }
    assert_eq!(died, vec![6, 5, 4, 3, 2, 1]);
    assert_eq!(over, 1);
}

#[test]
fn death_drops_the_powerup() {
    let mut sim = sim_of(&[]);
    toucher(
        &mut sim,
        types::SPECIAL,
        special_pickup(powerup::GHOST, 0),
        0,
    );
    step(&mut sim);
    assert_eq!(sim.stats().powerup_kind, powerup::GHOST);
    set_health(&mut sim, 1);
    enemy(&mut sim, 5);
    let (audio, _) = step(&mut sim);
    assert_eq!(sim.stats().powerup_kind, 0);
    assert_eq!(music(&audio), vec![MusicTrack::Level]);
}

// ----------------------------------------------------- saves and carry

#[test]
fn save_points_save_unless_cheated() {
    let mut sim = sim_of(&[("FirstSuperCheckpoint", 1210, 899)]);
    let events = run_until(&mut sim, hold(RIGHT), 200, |s| {
        s.claw().checkpoint.is_some()
    });
    assert_eq!(
        events,
        vec![
            GameEvent::Checkpoint { x: 1210, y: 867 },
            GameEvent::SavePoint { slot: 1 },
            GameEvent::Message { id: 1, ms: 5000 },
        ]
    );
    let mut sim = sim_of(&[("SecondSuperCheckpoint", 1210, 899)]);
    sim.with_world(|w| w.claw.stats.cheated = true);
    let events = run_until(&mut sim, hold(RIGHT), 200, |s| {
        s.claw().checkpoint.is_some()
    });
    assert_eq!(
        events,
        vec![
            GameEvent::Checkpoint { x: 1210, y: 867 },
            GameEvent::Message { id: 2, ms: 5000 },
        ]
    );
}

#[test]
fn save_points_reach_higher_than_flags() {
    // Claw standing under the flag's top but within the extra reach.
    let mut sim = sim_of(&[("FirstSuperCheckpoint", 1000, 899 + 160)]);
    let events = run_until(&mut sim, TickInput::default(), 50, |s| {
        s.claw().checkpoint.is_some()
    });
    assert!(events.contains(&GameEvent::SavePoint { slot: 1 }));
    let mut sim = sim_of(&[("Checkpoint", 1000, 899 + 160)]);
    idle(&mut sim, 50);
    assert_eq!(sim.claw().checkpoint, None);
}

#[test]
fn starting_from_a_save_point() {
    let level = level(&[
        ("FirstSuperCheckpoint", 2010, 899),
        ("SecondSuperCheckpoint", 2600, 899),
    ]);
    let carry = Carry {
        score: 12345,
        health: 60,
        lives: 3,
        ..Carry::default()
    };
    let start = StartState {
        carry: Some(carry.clone()),
        save: Some(SavePos {
            slot: 1,
            x: 2010,
            y: 867,
        }),
    };
    let mut sim = Sim::with_start(level, SimConfig::default(), start);
    let (audio, events) = step(&mut sim);
    assert!(events.is_empty());
    assert!(!played(&audio, "GAME_FLAGWAVE"));
    let o = sim.player().unwrap();
    assert_eq!((o.x, o.y), (2010, 899));
    assert_eq!(sim.claw().checkpoint, Some((2010, 867)));
    assert_eq!(sim.carry(), carry);
    // The save point's own flag is gone; the other one is still there.
    let flag = |s: &Sim, name: &str| {
        s.objects()
            .all()
            .find(|(_, o)| &*o.logic == name)
            .map(|(_, o)| o.draw_flags & draw_flags::HIDDEN == 0)
    };
    idle(&mut sim, 20);
    assert_eq!(flag(&sim, "FirstSuperCheckpoint"), None);
    assert_eq!(flag(&sim, "SecondSuperCheckpoint"), Some(true));
}

#[test]
fn carry_round_trip() {
    let carry = Carry {
        score: 777_000,
        health: 42,
        lives: 4,
        pistol: 20,
        magic: 7,
        dynamite: 2,
        weapon: Weapon::Magic,
        treasure_game: [1, 2, 3, 4, 5, 6, 7, 8, 9],
    };
    let start = StartState {
        carry: Some(carry.clone()),
        save: None,
    };
    let mut sim = Sim::with_start(level(&[]), SimConfig::default(), start);
    assert_eq!(sim.carry(), carry);
    let s = sim.stats();
    assert_eq!(s.treasure_level, [0; 9]);
    assert_eq!(s.next_extra_life, 1_000_000);
    assert_eq!((s.powerup_kind, s.powerup_ms), (0, 0));
    idle(&mut sim, 10);
    assert_eq!(sim.carry(), carry);
    assert_eq!(sim.snapshot().hud.lives, 4);
    // A new game.
    let sim = Sim::with_start(level(&[]), SimConfig::default(), StartState::default());
    assert_eq!(sim.carry(), Carry::default());
}
