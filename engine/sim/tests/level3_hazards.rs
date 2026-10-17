//! Floor spikes on synthetic levels: the set-up, the cycle on the level
//! clock tick by tick, the strike box fixed at set-up, what a risen spike
//! does to Claw, to an enemy and to a keg, and the sound rule.
//!
//! The tests named `install_*` read the user's own install from the folder
//! named by `CLAW_INSTALL` at run time and skip themselves when it is
//! unset; nothing from an install is stored in the repository.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::{ImageFrames, ObjectSpec, frame_box};
use claw_sim::logic::floor_spike::{self, FloorSpikeData, state};
use claw_sim::object::{ObjectId, Rect, flags, types};
use claw_sim::{ObjectData, Sim, SimConfig, SimLevel, TickInput};
use common::*;

const SET: &str = "LEVEL_FLOORSPIKES2";

fn element(image_op: i32, next_op: i32, duration: i32) -> AnimFrame {
    AnimFrame {
        image_op,
        next_op,
        duration,
        ..AnimFrame::default()
    }
}

fn add_anim(level: &mut SimLevel, key: &str, frames: Vec<AnimFrame>) {
    level.animations.insert(
        key.to_string(),
        Arc::new(Animation {
            key: key.to_string(),
            frames,
            ..Animation::default()
        }),
    );
}

/// The spikes' six frames: the art grows upwards from a fixed base.
fn spike_frames() -> ImageFrames {
    let mut f = ImageFrames::range(1, 6);
    for (i, (h, oy)) in [(9, 9), (9, 9), (13, 7), (18, 5), (23, 2), (28, 0)]
        .into_iter()
        .enumerate()
    {
        f.boxes.insert(i as i32 + 1, frame_box(16, h, 0, oy));
    }
    f
}

/// The flat level (feet rest on 959) with the spikes' frames and slides,
/// and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.images.insert(SET.into(), spike_frames());
    // Rise: next image every 100 ms until the last, then finish.
    add_anim(
        &mut level,
        floor_spike::RISE_ANIM,
        vec![element(1, 3, 100), element(0, 9, 0)],
    );
    // Drop: jump to the last image, previous every 100 ms until the
    // first, then finish.
    add_anim(
        &mut level,
        floor_spike::DROP_ANIM,
        vec![element(5, 0, 0), element(2, 2, 100), element(0, 9, 0)],
    );
    for key in [floor_spike::RISE_SOUND, floor_spike::DROP_SOUND] {
        level.sounds.insert(key.to_string(), Some(300));
    }
    level.objects = objects;
    level
}

/// A spike record running `logic` at (x, y), placed as the third level
/// places them (frame -1 reads as the set's first, depth and fields 0).
fn spike(logic: &str, x: i32, y: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at(logic, x, y, 0);
    s.image_set = SET.into();
    s.frame = 1;
    s
}

fn find(sim: &Sim, logic: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic}"))
}

fn data(sim: &Sim, id: ObjectId) -> FloorSpikeData {
    match &sim.objects().get(id).unwrap().data {
        ObjectData::FloorSpike(d) => d.clone(),
        other => panic!("not a spike: {other:?}"),
    }
}

fn st(sim: &Sim, id: ObjectId) -> i32 {
    sim.objects().get(id).unwrap().state
}

fn dangerous(sim: &Sim, id: ObjectId) -> bool {
    floor_spike::dangerous(sim.objects().get(id).unwrap())
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio().iter().any(|c| match c {
        AudioCue::Play(s) => s.key == key,
        _ => false,
    })
}

/// A simulation of `level` in which every object is awake from the start
/// (activation windows twice the level's size).
fn awake(level: SimLevel) -> Sim {
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

/// [`awake`], with Claw standing far from the spikes, out of their reach.
fn away(level: SimLevel) -> Sim {
    let mut sim = awake(level);
    sim.teleport_player(400, 899);
    sim
}

#[test]
fn the_set_up_fixes_the_strike_box_and_the_cycle() {
    let mut sim = away(level_with(vec![
        spike("FloorSpike", 1000, 950),
        spike("FloorSpike2", 1020, 950),
        spike("FloorSpike3", 1040, 950),
        spike("FloorSpike4", 1060, 950),
    ]));
    idle(&mut sim, 1);
    for (logic, phase) in [
        ("FloorSpike", 0),
        ("FloorSpike2", 750),
        ("FloorSpike3", 1500),
        ("FloorSpike4", 2250),
    ] {
        let id = find(&sim, logic);
        let o = sim.objects().get(id).unwrap();
        // The retracted frame's box, raised by 10 px.
        assert_eq!(o.attack_rect, Rect::new(-8, -5, 7, 12), "{logic}");
        assert_eq!(o.type_flags, types::LEVEL_SHOT);
        assert!(!o.hit_rect.is_set(), "nothing hits a spike");
        assert_eq!(o.hit_type_flags, 0);
        assert_eq!((o.z, o.damage), (2000, 10));
        let d = data(&sim, id);
        assert_eq!((d.phase, d.on_ms, d.period), (phase, 1500, 3000), "{logic}");
    }
}

#[test]
fn a_spike_rises_on_the_clock_one_call_late_and_hurts_only_when_up() {
    // A second-variant spike live from the start: down until the clock
    // reaches 750 ms (tick 42), the rising sound on tick 43, dangerous
    // from tick 74 (32 calls after the edge).
    let mut sim = awake(level_with(vec![spike("FloorSpike2", 1100, 950)]));
    let id = find(&sim, "FloorSpike2");
    assert_eq!(st(&sim, id), state::DOWN);
    let mut rise_sound = None;
    let mut first_danger = None;
    let mut drop_start = None;
    let mut down_again = None;
    let mut log = Vec::new();
    for t in 1..=240u64 {
        sim.step(TickInput::default());
        assert_eq!(sim.tick(), t);
        if played(&sim, floor_spike::RISE_SOUND) {
            rise_sound.get_or_insert(t);
        }
        let s = st(&sim, id);
        log.push((
            t,
            s,
            dangerous(&sim, id),
            sim.objects().get(id).unwrap().frame,
        ));
        if dangerous(&sim, id) {
            first_danger.get_or_insert(t);
        } else if first_danger.is_some() {
            drop_start.get_or_insert(t);
        }
        if s == state::DOWN && drop_start.is_some() {
            down_again.get_or_insert(t);
        }
    }
    assert_eq!(
        log[40].1,
        state::DOWN,
        "tick 41 is still down: {:?}",
        log[40]
    );
    assert_eq!(log[41].1, state::RISE_START, "tick 42 sees the edge");
    assert_eq!(rise_sound, Some(43));
    assert_eq!(first_danger, Some(74));
    // The frames of the rise: 2 on the first call after the start, then
    // one every 6 calls, the last on the 25th.
    let frame_at = |t: u64| log[t as usize - 1].3;
    assert_eq!(
        [44, 50, 56, 62, 68].map(frame_at),
        [2, 3, 4, 5, 6],
        "{:?}",
        &log[42..75]
    );
    assert_eq!(frame_at(43), 1);
    // The up half ends at 750 + 1500 = 2250 ms: tick 125 (2250) still
    // passes, tick 126 (2268) fails and stores the drop; the spike stays
    // dangerous in that tick and is harmless from tick 127.
    assert!(log[124].2 && log[125].2);
    assert_eq!(log[125].1, state::DROP_START);
    assert_eq!(drop_start, Some(127));
    // Dangerous for 53 ticks of this cycle (74 to 126).
    assert_eq!(log.iter().filter(|r| r.2).count(), 53);
    // The drop: set on tick 127, then frame 6 and 5 on its first two
    // calls, then one every 6 calls; down again on its 32nd call.
    assert_eq!([127, 128, 129].map(frame_at), [6, 6, 5]);
    assert_eq!([135, 141, 147, 153].map(frame_at), [4, 3, 2, 1]);
    assert_eq!(down_again, Some(159));
}

#[test]
fn a_spike_hurts_claw_as_a_hazard_shot_and_survives() {
    let mut sim = away(level_with(vec![spike("FloorSpike", 1100, 950)]));
    let id = find(&sim, "FloorSpike");
    // Wait for the spike to be up, then put Claw on it.
    for _ in 0..200 {
        sim.step(TickInput::default());
        if dangerous(&sim, id) {
            break;
        }
    }
    assert!(dangerous(&sim, id));
    let health = sim.stats().health;
    sim.teleport_player(1100, 899);
    sim.step(TickInput::default());
    assert_eq!(sim.stats().health, health - 10);
    assert!(played(&sim, "CLAW_HIT"));
    let c = sim.player().unwrap();
    assert_eq!(c.hit_type_flags & types::LEVEL_SHOT, 0, "shielded");
    assert!(
        sim.objects()
            .get(id)
            .is_some_and(|o| o.flags & flags::DESTROY == 0)
    );
    // Kept on the spike, the 850 ms window holds the next hit back for 47
    // ticks; the 48th hits again while the spike is still up.
    let mut hits = Vec::new();
    let mut h = sim.stats().health;
    for t in 1..=60 {
        sim.teleport_player(1100, 899);
        sim.step(TickInput::default());
        if sim.stats().health < h {
            hits.push(t);
            h = sim.stats().health;
        }
        if !dangerous(&sim, id) {
            break;
        }
    }
    assert_eq!(hits.first(), Some(&48), "{hits:?}");
}

#[test]
fn a_down_spike_never_hurts() {
    let mut sim = awake(level_with(vec![spike("FloorSpike2", 1000, 950)]));
    let health = sim.stats().health;
    // Claw stands on it from the start: it stays down until tick 42 and
    // rises harmlessly until tick 73.
    for t in 1..=73 {
        sim.teleport_player(1000, 899);
        sim.step(TickInput::default());
        assert_eq!(sim.stats().health, health, "tick {t}");
    }
    sim.teleport_player(1000, 899);
    sim.step(TickInput::default());
    assert_eq!(sim.stats().health, health - 10, "dangerous on tick 74");
}

#[test]
fn the_strike_box_does_not_grow_with_the_art() {
    let mut sim = away(level_with(vec![spike("FloorSpike", 1100, 950)]));
    let id = find(&sim, "FloorSpike");
    for _ in 0..200 {
        sim.step(TickInput::default());
        if dangerous(&sim, id) {
            break;
        }
    }
    let o = sim.objects().get(id).unwrap();
    assert_eq!(o.frame, 6, "fully risen");
    assert_eq!(o.attack_rect, Rect::new(-8, -5, 7, 12));
    // The risen art is 16 px wide and 28 px tall, but only the bottom
    // strip hurts: Claw beside the spike, his body 2 px clear of the
    // strike box, is not hurt.
    let health = sim.stats().health;
    sim.teleport_player(1100 + 31, 899);
    sim.step(TickInput::default());
    assert_eq!(sim.stats().health, health);
}

#[test]
fn a_risen_spike_hurts_an_enemy_walking_over_it() {
    let mut level = level_with(vec![spike("FloorSpike", 3000, 950)]);
    let mut rat = ObjectSpec::at("Rat", 3000, 900, 0);
    rat.image_set = "LEVEL_RAT".into();
    rat.record[51] = 2990;
    rat.record[53] = 3010;
    level.objects.push(rat);
    let mut sim = away(level);
    let rat = find(&sim, "Rat");
    let mut hit = false;
    for _ in 0..200 {
        sim.step(TickInput::default());
        if sim.collision_events().iter().any(|e| {
            matches!(e, claw_sim::container::CollisionEvent::Hit { victim, .. } if *victim == rat)
        }) {
            hit = true;
            break;
        }
    }
    assert!(hit, "the risen spike reached the rat");
}

#[test]
fn the_sounds_play_only_for_a_spike_on_screen() {
    // One spike beside Claw, one far off screen but awake.
    let level = level_with(vec![
        spike("FloorSpike2", 1100, 950),
        spike("FloorSpike2", 9000, 950),
    ]);
    let mut sim = awake(level);
    let mut ups = 0;
    let mut downs = 0;
    for _ in 0..200 {
        sim.step(TickInput::default());
        let count = |key: &str| {
            sim.last_audio()
                .iter()
                .filter(|c| matches!(c, AudioCue::Play(s) if s.key == key))
                .count()
        };
        ups += count(floor_spike::RISE_SOUND);
        downs += count(floor_spike::DROP_SOUND);
    }
    let far = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "FloorSpike2" && o.x == 9000)
        .unwrap()
        .1;
    assert_ne!(far.state, 0, "the far spike runs its cycle too");
    assert_eq!(data(&sim, find(&sim, "FloorSpike2")).period, 3000);
    assert_eq!((ups, downs), (1, 1));
}

/// Adds a `FloorSpike` (phase 0) that takes its first call in the next
/// tick, as a spike does when it wakes; returns it.
fn wake_spike(sim: &mut Sim, x: i32) -> ObjectId {
    let mut o = claw_sim::logic::new_object("FloorSpike", x, 950, 0);
    o.image_set = SET.into();
    o.frame = 1;
    sim.spawn(o)
}

/// The ticks in which the spike `id` is dangerous, from the next tick to
/// tick `until`.
fn danger_ticks(sim: &mut Sim, id: ObjectId, until: u64) -> Vec<u64> {
    let mut v = Vec::new();
    while sim.tick() < until {
        sim.step(TickInput::default());
        if dangerous(sim, id) {
            v.push(sim.tick());
        }
    }
    v
}

#[test]
fn a_spike_woken_inside_its_up_half_rises_at_once() {
    // Set up on tick 41 (738 ms), inside FloorSpike's up half (0 to 1500
    // ms): it sees the half on its next call and rises at once instead of
    // waiting for the next cycle; dangerous 32 calls after that call,
    // until the half ends (tick 84 is the first past 1500 ms).
    let mut sim = away(level_with(Vec::new()));
    idle(&mut sim, 40);
    let id = wake_spike(&mut sim, 2000);
    let danger = danger_ticks(&mut sim, id, 120);
    assert_eq!(danger, (74..=84).collect::<Vec<_>>());
    // Set up on tick 71 (1278 ms): the half ends while it rises, so once
    // up it is dangerous for two ticks only, the finishing call and the
    // next, which stores the drop.
    let mut sim = away(level_with(Vec::new()));
    idle(&mut sim, 70);
    let id = wake_spike(&mut sim, 2000);
    let danger = danger_ticks(&mut sim, id, 140);
    assert_eq!(danger, vec![104, 105]);
}

// ---------------------------------------------------------------- install

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level3(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(3)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

#[test]
fn install_level3_spikes_strike_from_the_retracted_frame() {
    let Some(store) = store() else { return };
    let level = level3(&store);
    let records: Vec<&ObjectSpec> = level
        .objects
        .iter()
        .filter(|o| o.logic.starts_with("FloorSpike"))
        .collect();
    assert_eq!(records.len(), 257);
    for anim in [floor_spike::RISE_ANIM, floor_spike::DROP_ANIM] {
        assert!(level.animations.contains_key(anim), "{anim} is loaded");
    }
    // Every spike everywhere awake at once.
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level.clone(),
        SimConfig {
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.step(TickInput::default());
    let mut n = 0;
    for (_, o) in sim.objects().all() {
        if !o.logic.starts_with("FloorSpike") {
            continue;
        }
        n += 1;
        assert_eq!(
            o.attack_rect,
            Rect::new(-8, -5, 7, 12),
            "{} at {}",
            o.logic,
            o.x
        );
        assert_eq!((o.z, o.damage), (2000, 10));
        assert_ne!(o.state, 0);
    }
    assert_eq!(n, 257);
}

#[test]
fn install_level3_a_second_variant_spike_live_from_the_start_rises_on_time() {
    let Some(store) = store() else { return };
    let level = level3(&store);
    let mut sim = Sim::new(level, SimConfig::default());
    let live: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "FloorSpike2" && o.state != 0)
        .map(|(id, _)| id)
        .collect();
    assert!(
        !live.is_empty(),
        "second-variant spikes are live at the start"
    );
    let mut sound = None;
    let mut danger = None;
    for t in 1..=80u64 {
        sim.step(TickInput::default());
        if played(&sim, floor_spike::RISE_SOUND) {
            sound.get_or_insert(t);
        }
        if live.iter().any(|&id| dangerous(&sim, id)) {
            danger.get_or_insert(t);
        }
    }
    assert_eq!(sound, Some(43));
    assert_eq!(danger, Some(74));
}
