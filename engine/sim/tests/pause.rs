//! The game clock around screens over play, on synthetic levels: the time
//! a help screen was up reaches the dynamite fuse once, in the update pass
//! of its return at the boundary before the first tick after it, and the
//! screen's frames move the next frame pulse; with the fuse clock frozen
//! neither happens. The in-game menu's frames, and those of the End Life
//! it offers, move the next frame pulse too, and nothing else. The fuse is lit in the tick a stick is thrown and goes
//! off 70 ticks later. On a level with Claw alone the pass changes nothing
//! a trace shows, and Claw's call in it sees no key pressed: a weapon
//! switch pressed in the tick before the screen switches once.

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::container::CollisionEvent;
use claw_sim::input::{MenuScreen, ScriptCommand};
use claw_sim::logic::projectile::{self, Phase, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::tuning::FuseClock;
use claw_sim::{Buttons, DirectiveKind, Sim, SimConfig, SimLevel, TickInput, TileMap};

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

/// The ticks among the first 12 that are frame pulse ticks, with
/// `between` played at the boundary before tick 4.
fn pulses_around(fuse_clock: FuseClock, between: impl Fn(&mut Sim)) -> Vec<i32> {
    let mut sim = sim(fuse_clock);
    let mut out = Vec::new();
    for n in 0..12 {
        if n == 4 {
            between(&mut sim);
        }
        sim.step(TickInput::default());
        if sim.pulse() {
            out.push(n);
        }
    }
    out
}

/// The in-game menu up for `ms`.
fn in_game_menu(ms: u32) -> DirectiveKind {
    DirectiveKind::Menu {
        screen: MenuScreen::InGame,
        wall_ms: ms,
        keys: Vec::new(),
    }
}

/// How many ticks later than without `between` the first pulse at or
/// after tick 4 comes, modulo the pulse's three ticks.
fn pulse_shift(fuse_clock: FuseClock, between: impl Fn(&mut Sim)) -> i32 {
    let after = |v: Vec<i32>| v.into_iter().find(|&n| n >= 4).unwrap();
    let plain = after(pulses_around(fuse_clock, |_| {}));
    (after(pulses_around(fuse_clock, between)) - plain).rem_euclid(3)
}

#[test]
fn the_in_game_menus_frames_move_the_next_frame_pulse() {
    // One frame per 18 ms of the menu, rounded up: 300 ms run 17 frames,
    // two short of a whole number of pulse periods (one tick later, as
    // two frames earlier), 54 ms three frames, a whole period, and 18 ms
    // one, two ticks later.
    let menu = |ms| {
        move |sim: &mut Sim| {
            sim.play_directive(&in_game_menu(ms));
        }
    };
    let original = FuseClock::Original;
    assert_eq!(pulse_shift(original, menu(300)), 1);
    assert_eq!(pulse_shift(original, menu(299)), 1);
    assert_eq!(pulse_shift(original, menu(54)), 0);
    assert_eq!(pulse_shift(original, menu(18)), 2);
    // Keys pressed on it add no frame.
    let keyed = |sim: &mut Sim| {
        sim.play_directive(&DirectiveKind::Menu {
            screen: MenuScreen::InGame,
            wall_ms: 300,
            keys: vec!["down".into(), "confirm".into()],
        });
    };
    assert_eq!(pulse_shift(original, keyed), 1);
    // With the fuse clock frozen the menu leaves the pulse alone.
    assert_eq!(pulse_shift(FuseClock::Frozen, menu(300)), 0);
    // The pause screen moves nothing.
    let pause = |sim: &mut Sim| {
        sim.play_directive(&DirectiveKind::Menu {
            screen: MenuScreen::Pause,
            wall_ms: 300,
            keys: Vec::new(),
        });
    };
    assert_eq!(pulse_shift(original, pause), 0);
}

#[test]
fn the_end_lifes_menu_frames_move_the_next_frame_pulse() {
    let end_life = DirectiveKind::Command(ScriptCommand::EndLife);
    // Alone: the menu opens (one frame), the confirmation page (three)
    // and the yes (one), five frames: one tick later.
    let alone = |sim: &mut Sim| {
        assert!(sim.play_directive(&end_life));
    };
    assert_eq!(pulse_shift(FuseClock::Original, alone), 1);
    // Called directly, the same.
    assert_eq!(
        pulse_shift(FuseClock::Original, |sim: &mut Sim| sim.end_life()),
        1
    );
    // A second call before the tick counts nothing more.
    let twice = |sim: &mut Sim| {
        sim.end_life();
        sim.end_life();
    };
    assert_eq!(pulse_shift(FuseClock::Original, twice), 1);
    // After the menu's own time at the same boundary the menu is already
    // open: four more frames. 300 ms (17 frames) and four: 21, a whole
    // number of periods; in either order.
    let after_menu = |sim: &mut Sim| {
        sim.play_directive(&in_game_menu(300));
        sim.play_directive(&end_life);
    };
    assert_eq!(pulse_shift(FuseClock::Original, after_menu), 0);
    let before_menu = |sim: &mut Sim| {
        sim.play_directive(&end_life);
        sim.play_directive(&in_game_menu(300));
    };
    assert_eq!(pulse_shift(FuseClock::Original, before_menu), 0);
    // A menu at an earlier boundary does not open this one.
    let earlier = {
        let mut sim = sim(FuseClock::Original);
        let mut out = Vec::new();
        for n in 0..12 {
            if n == 3 {
                sim.play_directive(&in_game_menu(54));
            }
            if n == 4 {
                sim.play_directive(&end_life);
            }
            sim.step(TickInput::default());
            if sim.pulse() {
                out.push(n);
            }
        }
        out
    };
    let five = pulses_around(FuseClock::Original, alone);
    assert_eq!(
        earlier.iter().filter(|&&n| n >= 4).collect::<Vec<_>>(),
        five.iter().filter(|&&n| n >= 4).collect::<Vec<_>>()
    );
    // With the fuse clock frozen the pulse is left alone.
    assert_eq!(pulse_shift(FuseClock::Frozen, alone), 0);
}

#[test]
fn a_visit_to_the_in_game_menu_counts_at_least_its_opening_frame() {
    let end_life = DirectiveKind::Command(ScriptCommand::EndLife);
    // A menu of no time still shows its opening frame: one frame, as for
    // 18 ms, two ticks later.
    let empty = |sim: &mut Sim| {
        sim.play_directive(&in_game_menu(0));
    };
    assert_eq!(pulse_shift(FuseClock::Original, empty), 2);
    // So a menu of no time and its End Life move the pulse as a lone End
    // Life does (five frames), in either order.
    let alone = |sim: &mut Sim| {
        sim.play_directive(&end_life);
    };
    let menu_then_end = |sim: &mut Sim| {
        sim.play_directive(&in_game_menu(0));
        sim.play_directive(&end_life);
    };
    let end_then_menu = |sim: &mut Sim| {
        sim.play_directive(&end_life);
        sim.play_directive(&in_game_menu(0));
    };
    let five = pulses_around(FuseClock::Original, alone);
    assert_eq!(pulses_around(FuseClock::Original, menu_then_end), five);
    assert_eq!(pulses_around(FuseClock::Original, end_then_menu), five);
    // With the fuse clock frozen nothing moves.
    assert_eq!(pulse_shift(FuseClock::Frozen, empty), 0);
}

#[test]
fn a_key_pressed_before_the_help_screen_acts_once() {
    let weapon = |sim: &Sim| claw_sim::TraceRow::capture(sim, 0, "script").weapon;
    let switch = TickInput {
        held: Buttons::TOGGLE,
        pressed: Buttons::TOGGLE,
    };
    let mut sim = sim(FuseClock::Original);
    let before = weapon(&sim);
    sim.step(switch);
    let switched = weapon(&sim);
    assert_ne!(switched, before, "the press switches the weapon");
    sim.pass_menu_time(500);
    assert_eq!(weapon(&sim), switched, "not again in the return's pass");
    // The last tick's input is left as it was.
    assert_eq!(sim.claw_input(), switch);
    sim.latch_keys();
    sim.step(TickInput {
        held: Buttons::TOGGLE,
        pressed: Buttons::NONE,
    });
    assert_eq!(weapon(&sim), switched, "held across the screen: latched");
}

#[test]
fn the_help_returns_frame_puts_objects_left_outside_the_window_to_sleep() {
    // Claw moved far along just before the screen: the camera's call in
    // the return's pass catches up with him, and the return's frame ends
    // with the leave pass there, so an object by his old place is asleep
    // before the next tick runs, and one by his new place is still live.
    let marker = |x: i32| {
        let mut o = Object::new(0, x, 940, 0, "Marker".into(), LogicKind::Inert);
        o.flags = flags::WORLD;
        o
    };
    let run = |fuse_clock: FuseClock| {
        let mut sim = sim(fuse_clock);
        let behind = sim.spawn(marker(1100));
        sim.step(TickInput::default());
        sim.teleport_player(9000, 900);
        let ahead = sim.spawn(marker(9100));
        sim.pass_menu_time(3000);
        let live = |id: ObjectId| sim.objects().get(id).is_some_and(Object::is_listed);
        (live(behind), live(ahead))
    };
    assert_eq!(run(FuseClock::Original), (false, true));
    // With the fuse clock frozen the return runs no pass at all: the next
    // tick's own leave pass is the first to see the move.
    assert_eq!(run(FuseClock::Frozen), (true, true));
}
