//! The game clock around screens over play, on synthetic levels: the time
//! a help screen was up reaches the dynamite fuse once, in the update pass
//! of its return at the boundary before the first tick after it, and the
//! screen's frames move the next frame pulse; with the fuse clock frozen
//! neither happens. The fuse is lit in the tick a stick is thrown and goes
//! off 70 ticks later. On a level with Claw alone the pass changes nothing
//! a trace shows.

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::container::CollisionEvent;
use claw_sim::logic::projectile::{self, Phase, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::tuning::FuseClock;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

fn frames(n: i32, ms: i32) -> Vec<(i32, i32)> {
    (1..=n).map(|f| (f, ms)).collect()
}

/// A flat level with the dynamite's animations, built in code with the
/// frame counts and timings of the game's own (the explosion hurts on its
/// third frame).
fn arena() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut explo = Animation::once("GAME_DYNAMITEEXPLO", &frames(18, 60));
    explo.frames[2].event = 1;
    for a in [
        Animation::looping("GAME_DYNAMITELIT", &frames(8, 50)),
        Animation::looping("GAME_DYNAMITEHORZ", &frames(7, 50)),
        explo,
        Animation::once("GAME_GLITTER1", &frames(6, 50)),
    ] {
        l.animations.insert(a.key.clone(), Arc::new(a));
    }
    l
}

fn sim(fuse_clock: FuseClock) -> Sim {
    let cfg = SimConfig {
        fuse_clock,
        ..SimConfig::default()
    };
    let mut sim = Sim::new(arena(), cfg);
    for _ in 0..30 {
        sim.step(TickInput::default());
    }
    sim
}

/// A stick of dynamite lying on the floor far from Claw, lit in the next
/// tick (its fuse starts at that tick's game time).
fn light(sim: &mut Sim) -> ObjectId {
    let spec = ProjectileSpec {
        logic: "Dynamite",
        x: 4000,
        y: 949,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        ..ProjectileSpec::default()
    };
    sim.with_world(|w| projectile::spawn(w, spec))
        .expect("launched")
}

/// A target beside the stick that records the explosion's hits.
fn target(sim: &mut Sim) -> ObjectId {
    let mut o = Object::new(0, 4000, 940, 300, "Target".into(), LogicKind::Inert);
    o.flags = flags::WORLD | flags::ALWAYS_ACTIVE;
    o.hit_type_flags = types::LEVEL_SHOT;
    o.hit_rect = Rect::new(-10, -10, 10, 10);
    o.hit_handler = true;
    sim.spawn(o)
}

fn exploding(sim: &Sim, id: ObjectId) -> bool {
    sim.objects()
        .get(id)
        .and_then(projectile::data)
        .is_some_and(|d| d.phase == Phase::Exploding)
}

/// What happened to a stick lit in tick `lit`: the tick it went off in
/// (`None` when it went off between ticks, before the tick after `pass`)
/// and the tick its blast hit the target. With `pass = Some((k, ms))`,
/// `ms` of help screen time passes at the boundary before tick `k`.
fn run(fuse_clock: FuseClock, pass: Option<(u64, u32)>) -> (u64, Option<u64>, u64) {
    let mut sim = sim(fuse_clock);
    let lit = sim.tick();
    let stick = light(&mut sim);
    let t = target(&mut sim);
    let mut went_off = None;
    let mut hurt = None;
    for _ in 0..200 {
        if let Some((k, ms)) = pass
            && sim.tick() == k
        {
            sim.pass_menu_time(ms);
            assert_eq!(sim.tick(), k, "no tick runs");
        }
        let before = exploding(&sim, stick);
        let n = sim.tick();
        sim.step(TickInput::default());
        if !before && went_off.is_none() && exploding(&sim, stick) {
            went_off = Some(n);
        }
        let hit = sim.collision_events().iter().any(|e| {
            *e == CollisionEvent::Hit {
                attacker: stick,
                victim: t,
            }
        });
        if hit && hurt.is_none() {
            hurt = Some(n);
            break;
        }
    }
    (lit, went_off, hurt.expect("the blast hurt"))
}

#[test]
fn the_fuse_goes_off_70_ticks_after_the_tick_it_is_lit_in() {
    let (lit, went_off, hurt) = run(FuseClock::Original, None);
    assert_eq!(went_off, Some(lit + 70));
    assert_eq!(hurt, lit + 70 + 9);
}

#[test]
fn help_time_reaches_the_fuse_at_the_return_exactly_at_1250_ms() {
    // Help opens after tick lit + 18: 324 ms of fuse have burnt.
    let k = |lit: u64| lit + 19;
    let (lit, ..) = run(FuseClock::Original, None);
    // 324 + 918 = 1242 ms: the stick goes off on its own tick.
    let (_, went_off, hurt) = run(FuseClock::Original, Some((k(lit), 918)));
    assert_eq!(went_off, Some(lit + 70));
    assert_eq!(hurt, lit + 79);
    // 324 + 936 = 1260 ms: it goes off at the return, as if in the tick
    // before help, and its blast hurts in the 8th tick after the return.
    let (_, went_off, hurt) = run(FuseClock::Original, Some((k(lit), 936)));
    assert_eq!(went_off, None, "gone off before the first tick after help");
    assert_eq!(hurt, k(lit) + 8);
    // A long help screen does the same.
    let (_, went_off, hurt) = run(FuseClock::Original, Some((k(lit), 2000)));
    assert_eq!(went_off, None);
    assert_eq!(hurt, k(lit) + 8);
}

#[test]
fn with_the_fuse_clock_frozen_help_time_never_reaches_the_fuse() {
    let (lit, ..) = run(FuseClock::Frozen, None);
    let (_, went_off, hurt) = run(FuseClock::Frozen, Some((lit + 19, 2000)));
    assert_eq!(went_off, Some(lit + 70));
    assert_eq!(hurt, lit + 79);
}

#[test]
fn help_time_changes_nothing_but_the_fuse() {
    // Without a stick, a pass changes nothing at all: the game goes on
    // tick for tick as without the help screen.
    let trace = |pass: bool| {
        let mut sim = sim(FuseClock::Original);
        let mut rows = Vec::new();
        for n in 0..120 {
            if pass && n == 40 {
                sim.pass_menu_time(5000);
            }
            sim.step(TickInput::default());
            rows.push(claw_sim::TraceRow::capture(&sim, n, "script").to_csv());
        }
        rows
    };
    assert_eq!(trace(true), trace(false));
    // A stick still in the air goes off where it is.
    let mut sim = sim(FuseClock::Original);
    let stick = light(&mut sim);
    for _ in 0..3 {
        sim.step(TickInput::default());
    }
    sim.pass_menu_time(1250);
    assert!(exploding(&sim, stick));
    // One that has gone off already is left alone.
    sim.pass_menu_time(1250);
    assert!(exploding(&sim, stick));
}

#[test]
fn the_help_screens_frames_move_the_next_frame_pulse() {
    // The pulse comes every third tick; a help screen of 3000 ms runs 167
    // frames of 18 ms, two more than a whole number of pulse periods, so
    // the next pulse comes two ticks earlier than without it, and one of
    // 3006 ms (167 frames too) the same.
    let pulses = |ms: Option<u32>| {
        let mut sim = sim(FuseClock::Original);
        let mut out = Vec::new();
        for n in 0..12 {
            if n == 4
                && let Some(ms) = ms
            {
                sim.pass_menu_time(ms);
            }
            sim.step(TickInput::default());
            if sim.pulse() {
                out.push(n);
            }
        }
        out
    };
    let plain = pulses(None);
    assert_eq!(plain.windows(2).map(|w| w[1] - w[0]).max(), Some(3));
    let after = |v: Vec<i32>| v.into_iter().find(|&n| n >= 4).unwrap();
    let first = after(plain.clone());
    let moved = |ms| (after(pulses(Some(ms))) - 4).rem_euclid(3);
    assert_eq!(moved(3000), (first - 4 - 2).rem_euclid(3));
    assert_eq!(moved(3006), moved(3000));
    // 54 ms run three frames: a whole period, the pulse as without it.
    assert_eq!(pulses(Some(54)), plain);
    // With the fuse clock frozen the help screen leaves the pulse alone.
    let frozen = {
        let mut sim = sim(FuseClock::Frozen);
        let mut out = Vec::new();
        for n in 0..12 {
            if n == 4 {
                sim.pass_menu_time(3000);
            }
            sim.step(TickInput::default());
            if sim.pulse() {
                out.push(n);
            }
        }
        out
    };
    assert_eq!(frozen, plain);
}
