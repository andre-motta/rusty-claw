//! Elevators, pegs and decoration on the first level of the user's own
//! install. The tests read the folder named by `CLAW_INSTALL` at run time
//! and skip themselves when it is unset; objects are found through the
//! level's own records, by logic name. The image-set frame ranges the pegs
//! and decorations play over come from the level itself, as in the game.

mod common;

use std::path::PathBuf;
use std::time::Instant;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::logic::decor;
use claw_sim::logic::platform::{self, PLATFORM, PlatformData, state};
use claw_sim::logic::{self, GameLogic};
use claw_sim::object::{LogicKind, Object, ObjectId, draw_flags, flags};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{
    ActivationConfig, InputLatch, InputScript, ObjectData, Sim, SimConfig, SimLevel, TickInput,
};
use common::*;

/// The roster of this family on the first level.
const ROSTER: [(&str, GameLogic); 13] = [
    ("Elevator", GameLogic::Platform),
    ("StartElevator", GameLogic::Platform),
    ("TriggerElevator", GameLogic::Platform),
    ("CrumblingPeg", GameLogic::Platform),
    ("TogglePeg", GameLogic::Platform),
    ("TogglePeg2", GameLogic::Platform),
    ("TogglePeg3", GameLogic::Platform),
    ("TogglePeg4", GameLogic::Platform),
    ("BehindCandy", GameLogic::Decor),
    ("FrontCandy", GameLogic::Decor),
    ("BehindAniCandy", GameLogic::Decor),
    ("FrontAniCandy", GameLogic::Decor),
    ("DoNothing", GameLogic::Decor),
];

const SMARTS: usize = 17;
const SPEED_X: usize = 55;
const SPEED_Y: usize = 56;
const SPEED: usize = 60;

struct Level1 {
    level: SimLevel,
    data: LevelData,
}

fn level1() -> Option<Level1> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    let store: AssetStore = install.store().expect("mounting the install");
    let data = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    let level = SimLevel::from_level(&data, &store).unwrap();
    Some(Level1 { level, data })
}

/// A simulation of the level as it loads, nothing injected.
fn sim_with(l: &Level1, cfg: SimConfig) -> Sim {
    Sim::new(l.level.clone(), cfg)
}

/// First and last frame numbers of the image set `set` in the level's
/// registry.
fn registry_range(l: &Level1, set: &str) -> Option<(i32, i32)> {
    let set = l.data.registries.images.get(set)?;
    Some((*set.frames.keys().next()?, *set.frames.keys().next_back()?))
}

/// Claw's code counts a death: the attempt counter rises, as the lives
/// step of a death does.
fn count_death(sim: &mut Sim) {
    sim.with_world(|w| w.claw.stats.count_attempt());
}

fn records(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect()
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).unwrap()
}

fn data(sim: &Sim, id: ObjectId) -> PlatformData {
    match &obj(sim, id).data {
        ObjectData::Platform(d) => d.clone(),
        _ => PlatformData::default(),
    }
}

fn riding(sim: &Sim, id: ObjectId) -> bool {
    let c = sim.player().unwrap();
    let p = obj(sim, id);
    c.flags & flags::ON_ELEVATOR != 0
        && c.object_below == Some(id)
        && c.y + c.move_rect.bottom == p.y + p.hit_rect.top - 1
}

fn played(sim: &Sim, key: &str) -> bool {
    sim.last_audio().iter().any(|c| match c {
        claw_sim::claw_frame::AudioCue::Play(s) => s.key == key,
        _ => false,
    })
}

/// Wakes the platform `id` by looking at it, then drops Claw just above
/// its surface and waits until he rides it.
fn land_on(sim: &mut Sim, id: ObjectId) -> bool {
    let (x, y) = (obj(sim, id).x, obj(sim, id).y);
    sim.teleport_player(x, y - 400);
    for _ in 0..3 {
        sim.step(TickInput::default());
    }
    for _ in 0..20 {
        let p = obj(sim, id);
        if p.type_flags == PLATFORM && p.hit_rect.is_set() {
            break;
        }
        sim.step(TickInput::default());
    }
    let p = obj(sim, id);
    let bottom = sim.player().unwrap().move_rect.bottom;
    sim.teleport_player(p.x, p.y + p.hit_rect.top - bottom - 6);
    for _ in 0..60 {
        sim.step(TickInput::default());
        if riding(sim, id) {
            return true;
        }
    }
    false
}

#[test]
fn install_every_roster_logic_binds_to_its_family() {
    let Some(l) = level1() else { return };
    let sim = Sim::new(l.level.clone(), SimConfig::default());
    for (name, family) in ROSTER {
        assert_eq!(
            logic::kind_for(name),
            Some(LogicKind::Game(family)),
            "{name}"
        );
        assert!(
            !sim.diagnostics().inert_logics.contains_key(name),
            "{name} is inert"
        );
        assert!(!records(&sim, name).is_empty(), "{name} is placed");
        for id in records(&sim, name) {
            assert_eq!(obj(&sim, id).kind, LogicKind::Game(family), "{name}");
        }
    }
    for anim in [
        platform::FORWARD_ANIM,
        platform::BACKWARD_ANIM,
        decor::CYCLE_ANIM,
    ] {
        assert!(l.level.animations.contains_key(anim), "{anim} is loaded");
    }
}

/// How a ride on an elevator ended.
#[derive(Debug, PartialEq)]
enum Ride {
    /// Carried to both ends of the track and back to the start.
    BothEnds,
    /// The level stopped Claw while the elevator went on along its track
    /// (a wall or a ledge across it), at this many ticks.
    Blocked(usize),
}

/// Rides the elevator `id` until it has been to both ends of its track
/// and back. Every tick Claw's feet are on its surface and he keeps his
/// place on it, unless the level stops him.
fn ride_to_both_ends(sim: &mut Sim, id: ObjectId, logic: &str) -> Ride {
    assert!(land_on(sim, id), "{logic} {id}: Claw lands on it");
    let d = data(sim, id);
    let clock = logic == "Elevator";
    let moving_x = if clock {
        d.x.span > 0
    } else {
        d.x.speed > 0 && d.x.min != d.x.max
    };
    let axis = if moving_x { d.x } else { d.y };
    let (lo, hi) = if clock {
        (axis.min, axis.max + 1)
    } else {
        (axis.min, axis.max)
    };
    let at = |sim: &Sim| {
        let p = obj(sim, id);
        if moving_x { p.x } else { p.y }
    };
    let offset = sim.player().unwrap().x - obj(sim, id).x;
    let (mut seen_lo, mut seen_hi) = (false, false);
    let start = at(sim);
    for n in 0..6000 {
        let before = sim.player().unwrap().x;
        let track_before = at(sim);
        sim.step(TickInput::default());
        let moved = obj(sim, id).platform_move;
        let carried = before + moved.0;
        if sim.player().unwrap().x != carried {
            // Claw was held back while the elevator made its move along its
            // track and goes on without him.
            assert_ne!(
                moved,
                (0, 0),
                "{logic} {id}: Claw stopped on a still elevator"
            );
            assert_eq!(
                at(sim) - track_before,
                if moving_x { moved.0 } else { moved.1 },
                "{logic} {id}: the elevator kept to its track"
            );
            let c = sim.player().unwrap();
            assert!(
                (c.x - before).abs() < moved.0.abs(),
                "{logic} {id}: Claw was stopped, not thrown"
            );
            let far_before = at(sim);
            idle(sim, 5);
            assert_ne!(at(sim), far_before, "{logic} {id}: it runs on");
            return Ride::Blocked(n);
        }
        assert!(
            riding(sim, id),
            "{logic} {id}: feet on the surface at tick {n}"
        );
        assert_eq!(
            sim.player().unwrap().x - obj(sim, id).x,
            offset,
            "{logic} {id}: no sliding"
        );
        let v = at(sim);
        seen_lo |= v <= lo;
        seen_hi |= v >= hi;
        if seen_lo && seen_hi && (v - start).abs() <= 4 {
            return Ride::BothEnds;
        }
    }
    panic!("{logic} {id}: not carried to both ends ({lo}..{hi}) and back");
}

#[test]
fn install_claw_rides_each_elevator_kind_to_its_ends_and_back() {
    let Some(l) = level1() else { return };
    // Every record reaches both ends but one TriggerElevator, whose track
    // runs into the level's tiles at Claw's height: Claw is stopped while
    // it runs on.
    for (logic, count, blocked) in [
        ("Elevator", 26, 0),
        ("StartElevator", 1, 0),
        ("TriggerElevator", 4, 1),
    ] {
        let ids = records(&Sim::new(l.level.clone(), SimConfig::default()), logic);
        assert_eq!(ids.len(), count, "{logic} records");
        let mut rides = Vec::new();
        for &id in &ids {
            let mut sim = sim_with(&l, SimConfig::default());
            rides.push(ride_to_both_ends(&mut sim, id, logic));
        }
        eprintln!("{logic}: {rides:?}");
        let stopped: Vec<ObjectId> = ids
            .iter()
            .zip(&rides)
            .filter(|(_, r)| matches!(r, Ride::Blocked(_)))
            .map(|(&id, _)| id)
            .collect();
        assert_eq!(stopped.len(), blocked, "{logic}: stopped on {stopped:?}");
        assert_eq!(
            rides.iter().filter(|r| **r == Ride::BothEnds).count(),
            count - blocked,
            "{logic}: {rides:?}"
        );
    }
}

#[test]
fn install_a_death_sends_each_trigger_elevator_back() {
    let Some(l) = level1() else { return };
    for id in records(
        &Sim::new(l.level.clone(), SimConfig::default()),
        "TriggerElevator",
    ) {
        // The whole level stays live, so the elevator runs its next tick
        // wherever Claw comes back.
        let cfg = SimConfig {
            activation: ActivationConfig::widened((100_000, 100_000)),
            ..SimConfig::default()
        };
        let mut sim = sim_with(&l, cfg);
        let placed = (obj(&sim, id).x, obj(&sim, id).y);
        assert!(land_on(&mut sim, id));
        idle(&mut sim, 40);
        assert_ne!((obj(&sim, id).x, obj(&sim, id).y), placed, "{id} moved");
        // Claw dies and comes back at the start; the elevator is back where
        // it was placed, waiting for him.
        count_death(&mut sim);
        sim.respawn();
        idle(&mut sim, 2);
        assert_eq!((obj(&sim, id).x, obj(&sim, id).y), placed, "{id}");
        idle(&mut sim, 40);
        assert_eq!((obj(&sim, id).x, obj(&sim, id).y), placed, "{id} waits");
    }
}

#[test]
fn install_a_crumbling_peg_drops_after_its_delay_and_comes_back() {
    let Some(l) = level1() else { return };
    let mut sim = sim_with(&l, SimConfig::default());
    let id = records(&sim, "CrumblingPeg")[0];
    assert!(land_on(&mut sim, id));
    let p = obj(&sim, id);
    let first = p.frame;
    sim.set_camera_point(Some((p.x, p.y)));
    let (mut t, mut sound, mut fell, mut hidden) = (None, None, None, None);
    let mut carried = Vec::new();
    for k in 0..120u64 {
        sim.step(TickInput::default());
        let p = obj(&sim, id);
        if t.is_none() && p.state == state::START {
            t = Some(k);
        }
        if riding(&sim, id) {
            carried.push(k);
        }
        if sound.is_none() && played(&sim, platform::CRUMBLE_SOUND) {
            sound = Some(k);
        }
        if fell.is_none() && p.type_flags != PLATFORM {
            fell = Some(k);
            let c = sim.player().unwrap();
            assert!(
                c.flags & flags::ON_ELEVATOR == 0,
                "Claw falls in the same tick"
            );
        }
        if hidden.is_none() && p.draw_flags & draw_flags::HIDDEN != 0 {
            hidden = Some(k);
        }
    }
    let t = t.expect("the peg sees Claw");
    assert_eq!(sound, Some(t + 2));
    // The peg carries Claw for 15 ticks (270 ms) after it sees him, then
    // stops being solid; it hides 36 ticks (648 ms) after it sees him.
    assert_eq!(carried, (0..t + 15).collect::<Vec<_>>(), "t = {t}");
    assert_eq!(fell, Some(t + 15));
    assert_eq!(hidden, Some(t + 36));
    assert_eq!(u64::from(claw_sim::claw_frame::TICK_MS) * 15, 270);
    idle(&mut sim, 200);
    assert_eq!(obj(&sim, id).state, state::GONE, "gone until a new attempt");
    count_death(&mut sim);
    idle(&mut sim, 2);
    let p = obj(&sim, id);
    assert_eq!(p.state, state::STOPPED);
    assert_eq!(p.type_flags, PLATFORM);
    assert_eq!(p.frame, first);
    assert_eq!(p.draw_flags & draw_flags::HIDDEN, 0);
}

#[test]
fn install_pegs_and_decorations_play_over_the_levels_frame_ranges() {
    let Some(l) = level1() else { return };
    assert_eq!(registry_range(&l, "LEVEL_CRUMBLINGPEG"), Some((1, 12)));
    for (_, o) in Sim::new(l.level.clone(), SimConfig::default())
        .objects()
        .all()
    {
        if matches!(
            o.kind,
            LogicKind::Game(GameLogic::Platform | GameLogic::Decor)
        ) {
            assert_eq!(
                l.level.image_frames(&o.image_set),
                registry_range(&l, &o.image_set),
                "{} {}",
                o.logic,
                o.image_set
            );
        }
    }
    // After set-up every peg holds its set's range and its switch frame.
    let cfg = SimConfig {
        activation: ActivationConfig::widened((100_000, 100_000)),
        ..SimConfig::default()
    };
    let mut sim = sim_with(&l, cfg);
    idle(&mut sim, 2);
    let mut pegs = 0;
    for (_, o) in sim.objects().all() {
        if o.logic.contains("Peg") {
            let d = match &o.data {
                ObjectData::Platform(d) => d.clone(),
                _ => PlatformData::default(),
            };
            let range = registry_range(&l, &o.image_set).expect("a peg image set");
            assert_eq!(d.frames, Some(range), "{}", o.logic);
            assert_eq!(d.switch_frame, (range.0 + range.1) / 2, "{}", o.logic);
            assert!(range.1 > range.0, "{}: a set of several images", o.logic);
            pegs += 1;
        }
    }
    assert_eq!(pegs, 64 + 23 + 16 + 11 + 6);
}

#[test]
fn install_toggle_pegs_follow_their_phases() {
    let Some(l) = level1() else { return };
    for (logic, variant_phase) in [
        ("TogglePeg", None),
        ("TogglePeg2", Some(750)),
        ("TogglePeg3", Some(1500)),
        ("TogglePeg4", Some(2250)),
    ] {
        let mut sim = sim_with(&l, SimConfig::default());
        let id = records(&sim, logic)
            .into_iter()
            .find(|&id| obj(&sim, id).record.as_ref().unwrap()[SMARTS] != 1)
            .unwrap_or_else(|| panic!("a {logic} that toggles"));
        let r = obj(&sim, id).record.clone().unwrap();
        let phase = variant_phase.unwrap_or(r[SPEED]);
        let half = |v: i32| if v == 0 { 1500 } else { v };
        let (on_ms, period) = (half(r[SPEED_X]), half(r[SPEED_X]) + half(r[SPEED_Y]));
        let on = |ms: u64| platform::toggle_on(ms as u32, phase, on_ms, period);
        let (x, y) = (obj(&sim, id).x, obj(&sim, id).y);
        sim.teleport_player(x, y - 100);
        sim.set_camera_point(Some((x, y)));
        // Wait until it has been set up and has gone round once.
        let mut edges = Vec::new();
        let mut was = None;
        let mut slides = 0;
        for _ in 0..1500 {
            sim.step(TickInput::default());
            let p = obj(&sim, id);
            if p.state == state::SET_UP {
                continue;
            }
            let ms = 18 * sim.tick();
            let now = p.type_flags == PLATFORM;
            if was.is_some_and(|w| w != now) {
                edges.push((ms, now));
            }
            was = Some(now);
            slides += usize::from(played(&sim, platform::SLIDE_SOUND));
        }
        assert!(edges.len() >= 6, "{logic}: {edges:?}");
        for &(ms, solid) in &edges {
            if solid {
                assert!(on(ms - 18 * 18), "{logic}: out 18 calls after the clock");
            } else {
                assert!(!on(ms - 18), "{logic}: in one call after the clock");
            }
        }
        assert!(slides >= edges.len(), "{logic}: a slide sound per move");
    }
}

#[test]
fn install_decorations_animate_and_never_collide() {
    let Some(l) = level1() else { return };
    let cfg = SimConfig {
        activation: ActivationConfig::widened((100_000, 100_000)),
        ..SimConfig::default()
    };
    let mut sim = sim_with(&l, cfg);
    let candies = records(&sim, "BehindAniCandy");
    assert_eq!(candies.len(), 485);
    let before: Vec<i32> = candies.iter().map(|&id| obj(&sim, id).frame).collect();
    // Each candy whose set has several images shows another one within
    // 30 ticks (the default loop changes image every sixth tick).
    let animated: Vec<ObjectId> = candies
        .iter()
        .copied()
        .filter(|&id| {
            l.level
                .image_frames(&obj(&sim, id).image_set)
                .is_some_and(|(first, last)| last > first)
        })
        .collect();
    let mut changed = vec![false; candies.len()];
    for _ in 0..30 {
        sim.step(TickInput::default());
        for (i, (&id, &f)) in candies.iter().zip(&before).enumerate() {
            changed[i] |= obj(&sim, id).frame != f;
        }
    }
    let still: Vec<ObjectId> = candies
        .iter()
        .zip(&changed)
        .filter(|&(id, &c)| !c && animated.contains(id))
        .map(|(&id, _)| id)
        .collect();
    assert!(still.is_empty(), "candies that do not animate: {still:?}");
    // The others show a set of one image, or one the level lacks.
    assert_eq!(animated.len(), 448, "candies with several images");
    for (_, o) in sim.objects().all() {
        if decor::NAMES.contains(&&*o.logic) {
            assert!(o.flags & flags::NO_COLLIDE != 0, "{}", o.logic);
            assert!(o.is_listed(), "{} is live", o.logic);
        }
    }
}

/// Every platform's position, state, solidity and frame.
fn platforms_line(sim: &Sim) -> String {
    let mut s = String::new();
    for (_, o) in sim.objects().all() {
        if matches!(
            o.kind,
            LogicKind::Game(GameLogic::Platform | GameLogic::Decor)
        ) {
            s.push_str(&format!(
                "{},{},{},{},{};",
                o.x, o.y, o.state, o.type_flags, o.frame
            ));
        }
    }
    s
}

fn trace_at_rate(l: &Level1, script: &InputScript, hz: u64) -> String {
    let mut sim = sim_with(l, SimConfig::default());
    let id = records(&sim, "StartElevator")[0];
    assert!(land_on(&mut sim, id));
    let mut latch = InputLatch::new();
    let events = script.events();
    let start = sim.tick();
    let (mut next_event, mut tick, mut frame) = (0, 0u64, 0u64);
    let n = script.ticks.len() as u64;
    let mut out = String::new();
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            sim.step(latch.take(tick));
            let row = TraceRow::capture(&sim, start + tick, "script");
            out.push_str(&trace::to_csv(&[row]));
            out.push_str(&platforms_line(&sim));
            out.push('\n');
            tick += 1;
        }
        frame += 1;
    }
    out
}

#[test]
fn install_platform_traces_are_identical_at_60_144_and_240_hz() {
    let Some(l) = level1() else { return };
    let s = script(&[
        (0, 1500),
        (RIGHT, 400),
        (0, 600),
        (JUMP, 300),
        (0, 1200),
        (LEFT, 500),
        (LEFT | JUMP, 300),
        (0, 2000),
    ]);
    let base = trace_at_rate(&l, &s, 1000);
    for hz in [60, 144, 240] {
        assert_eq!(trace_at_rate(&l, &s, hz), base, "{hz} Hz");
    }
}

#[test]
fn install_tick_cost_with_every_decoration_live() {
    let Some(l) = level1() else { return };
    let mut report = Vec::new();
    for (name, logics, view, ticks) in [
        ("logics off, original window", false, (640, 480), 3000),
        ("logics on, original window", true, (640, 480), 3000),
        (
            "logics off, whole level live",
            false,
            (100_000, 100_000),
            300,
        ),
        ("logics on, whole level live", true, (100_000, 100_000), 300),
    ] {
        let cfg = SimConfig {
            activation: ActivationConfig::widened(view),
            level_logics: logics,
            ..SimConfig::default()
        };
        let mut sim = sim_with(&l, cfg);
        let live = sim.objects().listed().len();
        let t0 = Instant::now();
        idle(&mut sim, ticks);
        let us = t0.elapsed().as_secs_f64() * 1e6 / ticks as f64;
        report.push(format!("{name}: {live} live objects, {us:.1} us per tick"));
    }
    // The family calls alone: every decoration and platform of the level
    // ticked once per round.
    let mut sim = sim_with(&l, SimConfig::default());
    let mut ids: Vec<(ObjectId, GameLogic)> = sim
        .objects()
        .all()
        .filter_map(|(id, o)| match o.kind {
            LogicKind::Game(g @ (GameLogic::Platform | GameLogic::Decor)) => Some((id, g)),
            _ => None,
        })
        .collect();
    ids.sort_unstable();
    let rounds = 1000;
    let t0 = Instant::now();
    sim.with_world(|w| {
        for _ in 0..rounds {
            for &(id, g) in &ids {
                if g == GameLogic::Decor {
                    decor::tick(w, id);
                } else {
                    platform::tick(w, id);
                }
            }
        }
    });
    let us = t0.elapsed().as_secs_f64() * 1e6 / f64::from(rounds);
    report.push(format!(
        "{} platforms and decorations, logic calls only: {us:.1} us per tick",
        ids.len()
    ));
    for line in &report {
        eprintln!("{line}");
    }
}
