//! Recording a game with its pauses, menus and the end of a life, and
//! replaying it: the recording is an input script whose `@` directives hold
//! play for the wall time of each pause and menu and give the commands at
//! their tick boundaries, so the replay runs the same ticks at any display
//! rate. A level's recording replays from the start it names (the
//! progress carried in and the start point), as `--replay` plays it.
//! Synthetic levels built in code, plus cases on the user's own install
//! (read from `CLAW_INSTALL` at run time, skipped when it is unset).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use claw_frame::{Buttons, SimulatedRefresh};
use claw_game::input::Recorder;
use claw_game::session::{
    GameRecording, LevelRecording, Span, behaviour_comment, recording_csv, replay_trace,
    replay_trace_with_crabs, save_recording, script_behaviour, start_sim, start_sim_with_crabs,
};
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{Session, load_level, replay_headless};
use claw_sim::anim::Animation;
use claw_sim::input::{MenuScreen, ScriptCommand};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::crab::{self, CrabKey, Leftovers, nest};
use claw_sim::logic::enemy::field;
use claw_sim::trace::{self, TraceRow};
use claw_sim::tuning::PlayAids;
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

/// The progress a later level starts with.
fn carried() -> StartState {
    StartState {
        carry: Some(Carry {
            score: 31_500,
            lives: 5,
            health: 70,
            ..Carry::default()
        }),
        save: None,
    }
}

/// Plays `sim` for three seconds of walking and jumping with a pause,
/// recorded into `rec` as retail level `level` started from `start` at
/// `at`, with frames of `hz`; returns the live trace and the file written.
fn record_level(
    rec: &mut GameRecording,
    sim: Sim,
    level: u32,
    start: StartState,
    at: Option<(i32, i32)>,
    hz: f64,
) -> (Vec<String>, PathBuf) {
    let mut session = Session::new(sim);
    session.record_trace();
    rec.start_from(&mut session, Some(level), start, at);
    let dt = Duration::from_secs_f64(1.0 / hz);
    let frames = (3.0 * hz) as u32;
    let key = |s: &mut Session, b: Buttons, down: bool| {
        let at = s.stamp(Duration::from_millis(2));
        s.key(at, b, down);
    };
    for f in 0..frames {
        match f {
            _ if f == (0.2 * hz) as u32 => key(&mut session, RIGHT, true),
            _ if f == (0.9 * hz) as u32 => key(&mut session, JUMP, true),
            _ if f == (1.1 * hz) as u32 => key(&mut session, JUMP, false),
            _ if f == (1.5 * hz) as u32 => {
                session.open_span(Span::Pause);
                session.pause();
            }
            _ if f == (1.8 * hz) as u32 => {
                session.resume();
                session.close_span();
            }
            _ if f == (2.4 * hz) as u32 => key(&mut session, RIGHT, false),
            _ => {}
        }
        session.frame(dt);
    }
    let (path, ticks) = rec.finish(&session).unwrap().unwrap();
    assert_eq!(ticks as u64, session.ticks());
    (rows_of(session.trace()), path)
}

#[test]
fn a_later_levels_recording_replays_from_its_start_at_every_rate() {
    let dir = tempfile::tempdir().unwrap();
    let mut rec = GameRecording::new(dir.path().join("game.csv"));
    let at = Some((1400, 700));
    let sim = start_sim(level(), SimConfig::default(), carried(), at);
    let (live, path) = record_level(&mut rec, sim, 2, carried(), at, 144.0);

    // As `--replay FILE --trace OUT` reads it: the level and the start
    // point from the file, whatever level the settings had.
    let r = LevelRecording::read(&path).unwrap();
    assert_eq!(
        (r.level(), r.start.clone(), r.start_at),
        (Some(2), carried(), at)
    );
    let mut settings = Settings::new("/i", LevelChoice::Retail(1));
    settings.follow_recording(&r).unwrap();
    assert_eq!(settings.level, LevelChoice::Retail(2));
    assert_eq!(settings.start, at);
    let cfg = r.sim_config(SimConfig::default());
    for hz in [60.0, 144.0, 240.0] {
        let sim = start_sim(level(), cfg.clone(), r.start.clone(), settings.start);
        let rows = replay_trace(sim, r.script.clone(), settings.pause_input, hz);
        assert_eq!(rows_of(&rows), live, "{hz} Hz");
        assert_eq!(rows[0].score, 31_500, "the carried score");
    }
    // From a new game's values at the level's own start, as the replay
    // played it before, it is another game.
    let fresh = Sim::new(level(), cfg);
    let rows = replay_trace(fresh, r.script.clone(), settings.pause_input, 60.0);
    assert_ne!(rows_of(&rows), live);
}

#[test]
fn a_replay_refuses_a_start_it_was_not_made_with() {
    let dir = tempfile::tempdir().unwrap();
    let mut rec = GameRecording::new(dir.path().join("game.csv"));
    let sim = start_sim(level(), SimConfig::default(), carried(), Some((1400, 700)));
    let (_, path) = record_level(&mut rec, sim, 2, carried(), Some((1400, 700)), 60.0);
    let r = LevelRecording::read(&path).unwrap();
    let settings = |level: LevelChoice, cli: bool, start: Option<(i32, i32)>| {
        let mut s = Settings::new("/i", level);
        if cli {
            s.from_command_line.extend(["level", "custom"]);
        }
        s.start = start;
        s
    };
    // The level it names, or another named on the command line.
    let mut s = settings(LevelChoice::Retail(2), true, None);
    s.follow_recording(&r).unwrap();
    let err = settings(LevelChoice::Retail(3), true, None)
        .follow_recording(&r)
        .unwrap_err();
    assert!(err.to_string().contains("level 2"), "{err}");
    assert!(
        settings(LevelChoice::Custom("a.wwd".into()), true, None)
            .follow_recording(&r)
            .is_err()
    );
    // Its start point, or another one on the command line.
    let mut s = settings(LevelChoice::Retail(2), false, Some((1400, 700)));
    s.follow_recording(&r).unwrap();
    let err = settings(LevelChoice::Retail(2), false, Some((10, 20)))
        .follow_recording(&r)
        .unwrap_err();
    assert!(err.to_string().contains("1400,700"), "{err}");

    // A save point start cannot be moved by --start.
    let text = "# level 2\n# start_slot = 1\n# start_x = 900\n# start_y = 600\n0,0,0\n";
    let at_save = LevelRecording::parse(text).unwrap();
    assert!(
        settings(LevelChoice::Retail(2), false, Some((10, 20)))
            .follow_recording(&at_save)
            .is_err()
    );
    // An older recording names neither: the settings' level and --start
    // play it, as before.
    let old = LevelRecording::parse("0,0,0\n1,0,0\n").unwrap();
    let mut s = settings(LevelChoice::Retail(5), true, Some((10, 20)));
    s.follow_recording(&old).unwrap();
    assert_eq!((s.level, s.start), (LevelChoice::Retail(5), Some((10, 20))));
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

#[test]
fn install_replay_on_the_command_line_plays_a_later_level_from_its_start() {
    let Some(root) = install() else { return };
    let settings = Settings::new(&root, LevelChoice::Retail(2));
    let bundle = load_level(&settings.install, &settings.level).unwrap();
    let (x, y) = bundle.sim_level.start;
    let at = Some((x + 96, y - 32));
    let sim = start_sim(
        bundle.sim_level.clone(),
        settings.sim_config(settings.res),
        carried(),
        at,
    );
    let dir = tempfile::tempdir().unwrap();
    let mut rec = GameRecording::new(dir.path().join("game.csv"));
    let (live, path) = record_level(&mut rec, sim, 2, carried(), at, 60.0);
    // An empty config file, so the user's own does not change the replay.
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "").unwrap();
    let run = |extra: &[&str]| {
        let out = dir.path().join("trace.csv");
        let _ = std::fs::remove_file(&out);
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_rusty-claw"));
        cmd.arg("--config")
            .arg(&config)
            .arg("--install")
            .arg(&root)
            .arg("--no-audio")
            .arg("--replay")
            .arg(&path)
            .arg("--trace")
            .arg(&out)
            .args(extra);
        let o = cmd.output().unwrap();
        let text = std::fs::read_to_string(&out).ok();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
            text,
        )
    };
    let (ok, err, text) = run(&[]);
    assert!(ok, "{err}");
    let rows: Vec<String> = text
        .unwrap()
        .lines()
        .skip(1)
        .map(|l| l.replace(",script,", ",input,"))
        .collect();
    assert_eq!(rows, live, "replayed as played");
    assert!(err.contains("31500"), "the start is announced: {err}");
    // Another level on the command line is refused.
    let (ok, err, text) = run(&["--level", "1"]);
    assert!(!ok && text.is_none(), "{err}");
    assert!(err.contains("level 2"), "{err}");
}

#[test]
fn a_level_played_with_aids_says_so_and_replays_with_them() {
    let both = PlayAids {
        invincible: true,
        infinite_lives: true,
    };
    let dir = tempfile::tempdir().unwrap();
    let mut rec = GameRecording::new(dir.path().join("game.csv"));
    let cfg = SimConfig {
        aids: both,
        ..SimConfig::default()
    };
    let sim = start_sim(level(), cfg, StartState::default(), None);
    let (live, path) = record_level(&mut rec, sim, 1, StartState::default(), None, 60.0);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.lines().any(|l| l == "# invincible = on"), "{text}");
    assert!(text.lines().any(|l| l == "# infinite_lives = on"), "{text}");
    // Never a route.
    let e = InputScript::parse(&text).unwrap_err();
    assert!(e.message.contains("play-test aid"), "{e}");
    // A replay plays it with them.
    let r = LevelRecording::read(&path).unwrap();
    assert_eq!(r.script.header.aids, both);
    let mut settings = Settings::new("/i", LevelChoice::Retail(1));
    settings.follow_recording(&r).unwrap();
    assert_eq!(settings.aids, both);
    assert_eq!(settings.sim_config((640, 480)).aids, both);
    let cfg = r.sim_config(SimConfig::default());
    assert_eq!(cfg.aids, both);
    let sim = start_sim(level(), cfg, r.start.clone(), None);
    let rows = replay_trace(sim, r.script.clone(), settings.pause_input, 60.0);
    assert_eq!(rows_of(&rows), live);
    // Recorded again, the replay keeps them.
    let again = Recorder::new();
    let sim = start_sim(
        level(),
        r.sim_config(SimConfig::default()),
        r.start.clone(),
        None,
    );
    let mut session = Session::new(sim).with_replay(r.script.clone());
    session.set_before_tick(again.hook());
    while session.replaying() {
        session.frame(Duration::from_millis(18));
    }
    assert_eq!(again.script(&session).header.aids, both);
}

#[test]
fn a_level_played_without_aids_has_no_aid_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut rec = GameRecording::new(dir.path().join("game.csv"));
    let sim = start_sim(level(), SimConfig::default(), StartState::default(), None);
    let (_, path) = record_level(&mut rec, sim, 1, StartState::default(), None, 60.0);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("invincible") && !text.contains("infinite_lives"));
    let r = LevelRecording::read(&path).unwrap();
    assert_eq!(r.script.header.aids, PlayAids::NONE);
    assert_eq!(InputScript::parse(&text).unwrap(), r.script, "a route");
    // One aid alone, through the single-file recording.
    let invincible = PlayAids {
        invincible: true,
        ..PlayAids::NONE
    };
    let cfg = SimConfig {
        aids: invincible,
        ..SimConfig::default()
    };
    let mut session = Session::new(Sim::new(level(), cfg));
    let recorder = Recorder::new();
    session.set_before_tick(recorder.hook());
    for _ in 0..20 {
        session.frame(Duration::from_millis(18));
    }
    let path = dir.path().join("one.csv");
    save_recording(&recorder, &session, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.lines().any(|l| l == "# invincible = on"), "{text}");
    assert!(!text.contains("infinite_lives"), "{text}");
    let r = LevelRecording::parse(&text).unwrap();
    assert_eq!(r.sim_config(SimConfig::default()).aids, invincible);
}

/// A level with a hermit crab placed at x 1300 and a crab nest at x 2000
/// whose box (x 1700 to 1800) hatches its three crabs.
fn crab_level() -> SimLevel {
    let mut level = level();
    let mut c = ObjectSpec::at("HermitCrab", 1300, 900, 0);
    c.image_set = crab::IMAGE_SET.into();
    c.frame = 1;
    c.record[field::X_MIN] = 1200;
    c.record[field::X_MAX] = 1400;
    let mut n = ObjectSpec::at("CrabNest", 2000, 896, 0);
    n.image_set = nest::IMAGE_SET.into();
    n.frame = 1;
    n.record[field::USER_0] = 3;
    n.record[field::X_MIN] = 1700;
    n.record[field::Y_MIN] = 800;
    n.record[field::X_MAX] = 1800;
    n.record[field::Y_MAX] = 1000;
    level.objects.extend([c, n]);
    // The nest's hatch and the crabs' crouch, hide and fall, with the
    // lengths of their level's.
    let k = |s: &str| format!("LEVEL_HERMITCRAB_{s}");
    for a in [
        Animation::once(
            nest::HATCH_ANIM,
            &[(1, 50), (2, 50), (3, 50), (4, 50), (5, 50), (6, 0)],
        ),
        Animation::once(&k("CROUCH4"), &[(801, 50), (800, 200)]),
        Animation::looping(
            &k("HIDE"),
            &[(802, 750), (801, 500), (802, 200), (801, 750)],
        ),
        Animation::looping(&k("FALL"), &[(200, 100)]),
    ] {
        level.animations.insert(a.key.clone(), Arc::new(a));
    }
    level
}

/// The record of the first object of `logic` in `sim`.
fn record_of(sim: &Sim, logic: &str) -> u32 {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && o.record.is_some())
        .map(|(_, o)| o.serial)
        .unwrap()
}

/// What every hermit crab of `sim` started with, by which crab it is.
fn crab_values(sim: &Sim) -> Vec<(CrabKey, Leftovers)> {
    let mut out: Vec<(CrabKey, Leftovers)> = sim
        .objects()
        .all()
        .filter_map(|(id, o)| {
            Some((
                crab::which_crab(sim.objects(), id)?,
                crab::fields_of(o)?.leftovers,
            ))
        })
        .collect();
    out.sort_by_key(|(k, _)| format!("{k}"));
    out
}

#[test]
fn a_replay_gives_the_crabs_the_values_its_header_names() {
    let fresh = Sim::new(crab_level(), SimConfig::default());
    let (placed, nest) = (
        record_of(&fresh, "HermitCrab"),
        record_of(&fresh, "CrabNest"),
    );
    // Claw starts in the nest's box: its crabs hatch while he settles.
    let mut text = format!(
        "# level 2\n# start_at = 1750, 700\n# crab {placed} = 2, 578, 432\n\
         # nest_crab {nest} 2 = 7, 382, 382\n# nest_crab * * = 1, 1000000, 1000000\n\
         tick,held_mask,pressed_mask\n"
    );
    for t in 0..120 {
        text.push_str(&format!("{t},0x0,0x0\n"));
    }
    let r = LevelRecording::parse(&text).unwrap();
    assert_eq!(r.crabs.entries.len(), 3);
    let cfg = r.sim_config(SimConfig::default());
    let session = r.replay(r.start_sim(crab_level(), cfg.clone()));
    let l = |side, w, h| Leftovers {
        side,
        box1: (w, h),
        stationary: false,
    };
    let covering = l(1, 1_000_000, 1_000_000);
    let want = vec![
        (CrabKey::Placed(Some(placed)), l(2, 578, 432)),
        (CrabKey::Hatched(Some(nest), Some(1)), covering),
        (CrabKey::Hatched(Some(nest), Some(2)), l(7, 382, 382)),
        (CrabKey::Hatched(Some(nest), Some(3)), covering),
    ];
    assert_eq!(crab_values(session.sim()), want);
    let live = rows_of(session.trace());
    // `--replay FILE --trace OUT` plays the same game at any display rate.
    for hz in [60.0, 144.0] {
        let sim = start_sim_with_crabs(
            crab_level(),
            cfg.clone(),
            r.start.clone(),
            r.start_at,
            &r.crabs,
        );
        let rows = replay_trace_with_crabs(
            sim,
            r.script.clone(),
            claw_game::pause::PauseInput::Original,
            hz,
            r.crabs.clone(),
        );
        assert_eq!(rows_of(&rows), live, "{hz} Hz");
    }
    // Without the values every crab keeps the plain default.
    let plain = LevelRecording::parse(
        &text
            .lines()
            .filter(|l| !l.contains("crab"))
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert!(plain.crabs.is_empty());
    let session = plain.replay(plain.start_sim(crab_level(), cfg));
    assert!(
        crab_values(session.sim())
            .iter()
            .all(|(_, v)| *v == Leftovers::default())
    );
    assert_eq!(crab_values(session.sim()).len(), 4);
}
