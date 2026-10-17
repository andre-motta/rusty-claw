//! Recording a game with its pauses, menus and the end of a life, and
//! replaying it: the recording is an input script whose `@` directives hold
//! play for the wall time of each pause and menu and give the commands at
//! their tick boundaries, so the replay runs the same ticks at any display
//! rate. Synthetic levels built in code, plus one case on the first level
//! of the user's own install (read from `CLAW_INSTALL` at run time, skipped
//! when it is unset).

use std::path::PathBuf;
use std::time::Duration;

use claw_frame::{Buttons, SimulatedRefresh};
use claw_game::input::Recorder;
use claw_game::session::{
    Span, behaviour_comment, recording_csv, save_recording, script_behaviour,
};
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{Session, load_level, replay_headless};
use claw_sim::input::{MenuScreen, ScriptCommand};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{
    Carry, Directive, DirectiveKind, InputScript, Sim, SimConfig, SimLevel, StartState, TileMap,
};

const JUMP: Buttons = Buttons::JUMP;
const RIGHT: Buttons = Buttons::RIGHT;
const LEFT: Buttons = Buttons::LEFT;

fn level() -> SimLevel {
    let mut rows = vec![".".repeat(120); 15];
    rows.push("#".repeat(120));
    rows.push("#".repeat(120));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600))
}

/// A game with 3 lives.
fn sim() -> Sim {
    let start = StartState {
        carry: Some(Carry {
            lives: 3,
            ..Carry::default()
        }),
        save: None,
    };
    Sim::with_start(level(), SimConfig::default(), start)
}

/// What the player does, at a wall time in ms.
#[derive(Debug, Clone)]
enum Act {
    Key(Buttons, bool),
    PauseOn,
    PauseOff,
    MenuOn,
    MenuKey(&'static str),
    /// The menu closes; with `true` its end life was confirmed.
    MenuOff(bool),
}

/// Walks, jumps, pauses (pressing jump while paused), walks again, opens
/// the in-game menu and ends the life there, then plays on after the
/// respawn.
fn plan() -> Vec<(u64, Act)> {
    use Act::*;
    vec![
        (300, Key(RIGHT, true)),
        (900, Key(JUMP, true)),
        (1010, Key(JUMP, false)),
        (1500, PauseOn),
        (1700, Key(JUMP, true)),
        (2100, PauseOff),
        (2250, Key(JUMP, false)),
        (2600, Key(RIGHT, false)),
        (2700, Key(LEFT, true)),
        (3300, MenuOn),
        (3500, MenuKey("down")),
        (3700, MenuKey("confirm")),
        (4000, MenuKey("y")),
        (4050, MenuOff(true)),
        (4300, Key(LEFT, false)),
        (8200, Key(RIGHT, true)),
        (9000, Key(RIGHT, false)),
    ]
}

/// Plays `plan` live on a `hz` display with jitter, recording; returns the
/// session, the recorder and the trace as CSV with the source column made
/// the same for every input.
fn play_live(hz: f64, seed: u64, until_ms: u64) -> (Session, Recorder, Vec<String>) {
    let mut session = Session::new(sim());
    session.record_trace();
    let recorder = Recorder::new();
    session.set_before_tick(recorder.hook());
    let mut refresh = SimulatedRefresh::new(hz).with_jitter(0.2, seed);
    let plan = plan();
    let mut next = 0;
    let mut clock_us = 0u64;
    while clock_us < until_ms * 1000 {
        let dt = refresh.next().unwrap();
        let end = clock_us + dt.as_micros() as u64;
        while let Some((ms, act)) = plan.get(next) {
            let at = ms * 1000;
            if at > end {
                break;
            }
            let since = Duration::from_micros(at.saturating_sub(clock_us));
            match act {
                Act::Key(b, down) => {
                    let stamp = session.stamp(since);
                    session.key(stamp, *b, *down);
                }
                Act::PauseOn | Act::MenuOn => {
                    // As the window does: release what is held, stop the
                    // clock, note the stop.
                    let stamp = session.stamp(since);
                    session.release_all(stamp);
                    session.pause();
                    session.open_span(if matches!(act, Act::PauseOn) {
                        Span::Pause
                    } else {
                        Span::Menu(MenuScreen::InGame)
                    });
                }
                Act::MenuKey(name) => session.menu_key(name),
                Act::PauseOff | Act::MenuOff(_) => {
                    session.resume();
                    session.close_span();
                    if matches!(act, Act::MenuOff(true)) {
                        session.end_life();
                    }
                }
            }
            next += 1;
        }
        session.frame(dt);
        clock_us = end;
    }
    let rows = rows_of(session.trace());
    (session, recorder, rows)
}

fn rows_of(trace: &[TraceRow]) -> Vec<String> {
    trace
        .iter()
        .map(|r| {
            r.to_csv()
                .replace(",keyboard,", ",input,")
                .replace(",script,", ",input,")
        })
        .collect()
}

/// Replays `script` on a `hz` display (with jitter from `seed` when given);
/// the trace and the frames spent holding play.
fn replay(script: &InputScript, hz: f64, seed: Option<u64>) -> (Vec<String>, usize) {
    let mut session = Session::new(sim()).with_replay(script.clone());
    session.record_trace();
    let mut refresh = SimulatedRefresh::new(hz);
    if let Some(s) = seed {
        refresh = refresh.with_jitter(0.2, s);
    }
    let mut held_frames = 0;
    while session.replaying() {
        session.frame(refresh.next().unwrap());
        held_frames += usize::from(session.holding());
    }
    let mut rows = rows_of(session.trace());
    rows.truncate(script.ticks.len());
    (rows, held_frames)
}

#[test]
fn a_session_with_a_pause_a_menu_and_end_life_replays_bit_identically() {
    let (session, recorder, live) = play_live(144.0, 7, 10_000);
    let script = recorder.script(&session);
    assert_eq!(script.ticks.len() as u64, session.ticks());
    // The recording: a pause, the menu with its inputs, the end of a life
    // at the menu's boundary, after it.
    let d = script.directives();
    assert_eq!(d.len(), 3, "{d:?}");
    let DirectiveKind::Pause { wall_ms, held } = d[0].kind else {
        panic!("{d:?}")
    };
    assert!((550..=650).contains(&wall_ms), "{wall_ms}");
    assert_eq!(held, None);
    let DirectiveKind::Menu {
        screen,
        wall_ms,
        ref keys,
    } = d[1].kind
    else {
        panic!("{d:?}")
    };
    assert_eq!(screen, MenuScreen::InGame);
    assert!((700..=800).contains(&wall_ms), "{wall_ms}");
    assert_eq!(keys, &["down", "confirm", "y"]);
    assert_eq!(d[2].kind, DirectiveKind::Command(ScriptCommand::EndLife));
    assert_eq!(d[2].tick, d[1].tick);
    assert!(d[0].tick < d[1].tick);
    // The jump pressed while paused lands on the resume tick.
    assert!(script.ticks[d[0].tick].pressed.contains(JUMP));
    // Claw died and came back.
    let deaths = session
        .trace()
        .iter()
        .filter(|r| r.events.contains("claw_died"))
        .count();
    assert_eq!(deaths, 1, "one life ended");
    assert_eq!(session.sim().stats().lives, 2);

    // Through the file format.
    let text = recording_csv(&script, false);
    assert_eq!(script_behaviour(&text), Some(false));
    let back = InputScript::parse(&text).unwrap();
    assert_eq!(back, script);

    // Twice at each display rate: the same ticks as played.
    for hz in [60.0, 144.0, 240.0] {
        for seed in [None, Some(hz as u64)] {
            let (rows, held) = replay(&back, hz, seed);
            assert_eq!(rows, live, "{hz} Hz {seed:?}");
            let (again, _) = replay(&back, hz, seed);
            assert_eq!(again, rows, "{hz} Hz {seed:?} again");
            // Play held for the pause and the menu: about 1.35 s.
            let frames = 1.35 * hz;
            assert!(
                (held as f64) > frames * 0.8 && (held as f64) < frames * 1.25,
                "{hz} Hz: {held} frames held"
            );
        }
    }
}

#[test]
fn a_replay_recorded_again_gives_the_same_script() {
    let (session, recorder, _) = play_live(60.0, 3, 6000);
    let script = recorder.script(&session);
    let mut again = Session::new(sim()).with_replay(script.clone());
    let rec = Recorder::new();
    again.set_before_tick(rec.hook());
    let mut refresh = SimulatedRefresh::new(240.0);
    while again.replaying() {
        again.frame(refresh.next().unwrap());
    }
    assert_eq!(rec.script(&again), script);
    // And saved to a file, as the window does.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rec.csv");
    let n = save_recording(&rec, &again, &path).unwrap();
    assert_eq!(n, script.ticks.len());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with(&behaviour_comment(false)), "{text}");
    assert_eq!(InputScript::parse(&text).unwrap(), script);
}

#[test]
fn a_menu_still_open_when_the_recording_is_saved_is_kept() {
    let mut session = Session::new(sim());
    let recorder = Recorder::new();
    session.set_before_tick(recorder.hook());
    for _ in 0..10 {
        session.frame(Duration::from_millis(18));
    }
    session.pause();
    session.open_span(Span::Menu(MenuScreen::InGame));
    session.menu_key("down");
    session.menu_key("Not A Name");
    for _ in 0..5 {
        session.frame(Duration::from_millis(100));
    }
    let script = recorder.script(&session);
    assert_eq!(
        script.directives(),
        &[Directive {
            tick: 10,
            kind: DirectiveKind::Menu {
                screen: MenuScreen::InGame,
                wall_ms: 500,
                keys: vec!["down".into()],
            },
        }]
    );
    assert_eq!(InputScript::parse(&script.to_csv()).unwrap(), script);
}

#[test]
fn live_input_with_the_directives_plays_the_same_game_as_the_script() {
    let (session, recorder, live) = play_live(144.0, 11, 10_000);
    let script = recorder.script(&session);
    // The script's input as key events a millisecond into their ticks, at
    // wall times moved on by the pauses and menus before them.
    let held_before = |tick: u64| -> u64 {
        script
            .directives()
            .iter()
            .filter(|d| d.tick as u64 <= tick)
            .map(|d| match d.kind {
                DirectiveKind::Pause { wall_ms, .. } | DirectiveKind::Menu { wall_ms, .. } => {
                    u64::from(wall_ms) * 1000
                }
                DirectiveKind::Command(_) => 0,
            })
            .sum()
    };
    for hz in [60.0, 240.0] {
        let mut s = Session::new(sim()).with_directives(script.directives().to_vec());
        s.record_trace();
        let events = script.events();
        let mut next = 0;
        let mut clock = 0u64;
        let mut refresh = SimulatedRefresh::new(hz).with_jitter(0.25, 5);
        while s.ticks() < script.ticks.len() as u64 {
            let dt = refresh.next().unwrap();
            let end = clock + dt.as_micros() as u64;
            while let Some(&(at, b, down)) = events.get(next) {
                let happened = at + 1_000 + held_before(at / claw_sim::input::TICK_US);
                if happened > end {
                    break;
                }
                let stamp = s.stamp(Duration::from_micros(happened - clock));
                assert_eq!(stamp, at + 1_000, "{hz} Hz");
                s.key(stamp, b, down);
                next += 1;
            }
            s.frame(dt);
            clock = end;
        }
        let mut rows = rows_of(s.trace());
        rows.truncate(live.len());
        assert_eq!(rows, live, "{hz} Hz");
    }
}

#[test]
fn old_recordings_parse_and_write_back_unchanged() {
    // A recording as the game wrote it before pauses and menus were kept.
    let old = "# mid_air_hit_fix = off\ntick,held_mask,pressed_mask\n\
               0,0x0,0x0\n1,0x2000000,0x2000000\n2,0x2000001,0x1\n3,0x0,0x0\n";
    let s = InputScript::parse(old).unwrap();
    assert!(s.directives().is_empty());
    assert_eq!(recording_csv(&s, false), old);
    // Directives written by other tools: menu inputs as lowercase key
    // names.
    let routed = "# level 1\n# tuning original\n# seed 7\ntick,held_mask,pressed_mask\n\
                  0,0x0,0x0\n@1 pause 250 held=0x1\n1,0x1,0x1\n\
                  @2 menu help 167 keys=attack,dynamite\n@2 command end_life\n2,0x0,0x0\n";
    let s = InputScript::parse(routed).unwrap();
    assert_eq!(s.directives().len(), 3);
    assert_eq!(s.to_csv(), routed);
    assert_eq!(
        InputScript::parse(&recording_csv(&s, true)).unwrap(),
        s,
        "round trip with the behaviour line"
    );
    // A replay of it holds the pause and the help screen and ends the
    // life before tick 2.
    let mut session = Session::new(sim()).with_replay(s.clone());
    session.record_trace();
    let mut frames = 0;
    while session.replaying() {
        session.frame(Duration::from_millis(10));
        frames += 1;
    }
    assert!(frames >= 41 + 2, "{frames} frames");
    assert_eq!(session.trace()[2].health, 0);
    assert_eq!(session.directives(), s.directives().to_vec());
}

// ------------------------------------------------------------ install

fn install() -> Option<PathBuf> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    Some(PathBuf::from(root))
}

#[test]
fn install_a_route_with_a_menu_and_end_life_replays_the_same_at_every_rate() {
    let Some(root) = install() else { return };
    let settings = Settings::new(root, LevelChoice::Retail(1));
    let bundle = load_level(&settings.install, &settings.level).unwrap();
    // Walk right, open the menu, end the life, then walk again after the
    // respawn.
    let mut held = vec![0u32; 30];
    held.extend([RIGHT.0; 80]);
    held.extend([0; 400]);
    held.extend([RIGHT.0; 60]);
    let mut script = InputScript::from_held(&held);
    script.directives = vec![
        Directive {
            tick: 70,
            kind: DirectiveKind::Pause {
                wall_ms: 300,
                held: None,
            },
        },
        Directive {
            tick: 110,
            kind: DirectiveKind::Menu {
                screen: MenuScreen::InGame,
                wall_ms: 1500,
                keys: vec!["l".into(), "y".into()],
            },
        },
        Directive {
            tick: 110,
            kind: DirectiveKind::Command(ScriptCommand::EndLife),
        },
    ];
    let text = recording_csv(&script, false);
    let script = InputScript::parse(&text).unwrap();
    let base = replay_headless(&bundle, &settings, script.clone(), 60.0, None);
    assert_eq!(base.len(), script.ticks.len());
    assert_eq!(base[110].health, 0, "dies at the menu's tick");
    assert!(
        base.iter().any(|r| r.events.contains("claw_died")),
        "the life ends"
    );
    let first = trace::to_csv(&base);
    for (hz, jitter) in [
        (60.0, None),
        (144.0, Some(3)),
        (240.0, None),
        (240.0, Some(4)),
    ] {
        let rows = replay_headless(&bundle, &settings, script.clone(), hz, jitter);
        assert_eq!(trace::to_csv(&rows), first, "{hz} Hz {jitter:?}");
    }
}
