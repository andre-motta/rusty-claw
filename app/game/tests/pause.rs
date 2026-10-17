//! The pause key and the screens over play, played through a session as
//! the window plays them: which key does what, one tick per resume, how
//! held and pressed keys count after a pause and after a screen, the
//! dynamite fuse across each screen, the stand window taken with a pause,
//! and the behaviour switches in settings and recordings. Synthetic levels
//! built in code, plus the help picture of the user's own install (read
//! from `CLAW_INSTALL` at run time, skipped when it is unset).

use std::sync::Arc;
use std::time::Duration;

use claw_assets::ifc::IfcNode;
use claw_audio::AudioSettings;
use claw_frame::{Buttons, SimulatedRefresh};
use claw_game::Session;
use claw_game::audio::AudioBridge;
use claw_game::flow::{Effect, Flow, Screen, Start};
use claw_game::input::Recorder;
use claw_game::keymap::Command;
use claw_game::pause::{Change, GameKey, KeyContext, PauseInput, Pauses, game_key};
use claw_game::session::{
    FUSE_CLOCK_KEY, PAUSE_INPUT_KEY, ScriptSwitches, Span, recording_text, save_recording,
    script_switches, switch_comments,
};
use claw_game::settings::{Args, LevelChoice, Settings};
use claw_sim::anim::Animation;
use claw_sim::input::MenuScreen;
use claw_sim::logic::projectile::{self, ProjectileSpec};
use claw_sim::object::types;
use claw_sim::player::state;
use claw_sim::trace::TraceRow;
use claw_sim::tuning::FuseClock;
use claw_sim::{Directive, DirectiveKind, InputScript, Sim, SimConfig, SimLevel, TileMap};
use claw_ui::menu::command::{CommandCatalogue, MenuKind};
use claw_ui::menu::draw::FontArt;
use claw_ui::menu::nav::PRESS_MS;
use claw_ui::menu::{MenuInput, MenuTree};

const JUMP: Buttons = Buttons::JUMP;
const RIGHT: Buttons = Buttons::RIGHT;

fn frames(n: i32, ms: i32) -> Vec<(i32, i32)> {
    (1..=n).map(|f| (f, ms)).collect()
}

/// A flat level (feet on y = 959, Claw at x = 1000) with Claw's hurt
/// animations and the dynamite's, at the game's own timings.
fn level() -> SimLevel {
    let mut rows = vec![".".repeat(120); 15];
    rows.push("#".repeat(120));
    rows.push("#".repeat(120));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut explo = Animation::once("GAME_DYNAMITEEXPLO", &frames(18, 60));
    explo.frames[2].event = 1;
    for a in [
        Animation::once("CLAW_DAMAGE1", &[(1, 150), (2, 200)]),
        Animation::once("CLAW_DAMAGE2", &[(1, 150), (2, 200)]),
        Animation::looping("GAME_DYNAMITELIT", &frames(8, 50)),
        Animation::looping("GAME_DYNAMITEHORZ", &frames(7, 50)),
        explo,
    ] {
        l.animations.insert(a.key.clone(), Arc::new(a));
    }
    l
}

fn sim_with(fuse_clock: FuseClock) -> Sim {
    let cfg = SimConfig {
        fuse_clock,
        ..SimConfig::default()
    };
    Sim::new(level(), cfg)
}

/// A game played as the window plays it: key events stamped as they come,
/// one 18 ms frame at a time, the pauses through [`Pauses`].
struct Game {
    s: Session,
    pauses: Pauses,
    rec: Recorder,
}

impl Game {
    fn new(sim: Sim, rule: PauseInput) -> Game {
        let mut s = Session::new(sim).with_pause_input(rule);
        s.record_trace();
        let rec = Recorder::new();
        s.set_before_tick(rec.hook());
        Game {
            s,
            pauses: Pauses::default(),
            rec,
        }
    }

    fn ticks(&mut self, n: u64) {
        for _ in 0..n {
            assert_eq!(self.s.frame(Duration::from_millis(18)).ticks, 1);
        }
    }

    fn until(&mut self, tick: u64) {
        while self.s.ticks() < tick {
            self.ticks(1);
        }
    }

    fn key(&mut self, b: Buttons, down: bool) {
        let at = self.s.stamp(Duration::ZERO);
        self.s.key(at, b, down);
    }

    /// Play stops for `span` and `wall_ms` of frames pass.
    fn stop(&mut self, span: Span) {
        match span {
            Span::Pause => self.pauses.user = true,
            Span::Menu(_) => self.pauses.menu = true,
        }
        self.pauses.apply(Some(&mut self.s), None, Duration::ZERO);
        self.s.open_span(span);
    }

    fn wait(&mut self, ms: u64) {
        assert_eq!(self.s.frame(Duration::from_millis(ms)).ticks, 0);
    }

    /// Play goes on with `down` held (as the input devices report), and
    /// the frame after it runs its one tick.
    fn go_on(&mut self, down: Buttons) {
        self.pauses.user = false;
        self.pauses.menu = false;
        self.s.sync_keys(down);
        self.pauses.apply(Some(&mut self.s), None, Duration::ZERO);
        self.s.close_span();
        assert_eq!(self.s.frame(Duration::ZERO).ticks, 1, "the resume tick");
    }

    fn row(&self, tick: u64) -> &TraceRow {
        &self.s.trace()[tick as usize]
    }

    fn script(&self) -> InputScript {
        self.rec.script(&self.s)
    }
}

/// Replays `script` at `hz` with jitter; the trace without the source
/// column.
fn replay(sim: Sim, script: &InputScript, hz: f64) -> Vec<String> {
    let mut s = Session::new(sim).with_replay(script.clone());
    s.record_trace();
    let mut refresh = SimulatedRefresh::new(hz).with_jitter(0.2, hz as u64);
    while s.replaying() {
        s.frame(refresh.next().unwrap());
    }
    rows(&s.trace()[..script.ticks.len()])
}

fn rows(trace: &[TraceRow]) -> Vec<String> {
    trace
        .iter()
        .map(|r| {
            r.to_csv()
                .replace(",keyboard,", ",input,")
                .replace(",script,", ",input,")
        })
        .collect()
}

// --------------------------------------------------------------- the keys

fn ctx(screen: Option<Screen>, pauses: Pauses) -> KeyContext {
    KeyContext {
        screen,
        completing: false,
        pauses,
        from_pad: false,
        modifiers: false,
    }
}

#[test]
fn the_pause_key_pauses_without_a_menu_and_escape_opens_the_menu() {
    let play = Some(Screen::Play);
    let free = Pauses::default();
    let paused = Pauses {
        user: true,
        ..Pauses::default()
    };
    let flag = Pauses {
        flag: true,
        ..Pauses::default()
    };
    // In play: Escape opens the menu, the pause key pauses, F1 is help.
    assert_eq!(game_key(Command::Quit, ctx(play, free)), GameKey::Menu);
    assert_eq!(game_key(Command::Pause, ctx(play, free)), GameKey::Pause);
    assert_eq!(game_key(Command::Help, ctx(play, free)), GameKey::Help);
    // Paused: Escape does nothing, the pause key goes on, F1 is help.
    assert_eq!(game_key(Command::Quit, ctx(play, paused)), GameKey::Nothing);
    assert_eq!(game_key(Command::Pause, ctx(play, paused)), GameKey::Pause);
    assert_eq!(game_key(Command::Help, ctx(play, paused)), GameKey::Help);
    // After help over a pause: play runs with the flag set; Escape stays
    // blocked until the pause key clears it.
    assert_eq!(game_key(Command::Quit, ctx(play, flag)), GameKey::Nothing);
    assert_eq!(
        game_key(Command::Pause, ctx(play, flag)),
        GameKey::ClearFlag
    );
    // With Shift, Ctrl or Alt held the pause key does nothing.
    let held = KeyContext {
        modifiers: true,
        ..ctx(play, free)
    };
    assert_eq!(game_key(Command::Pause, held), GameKey::Nothing);
    // In the in-game menu the pause key does nothing; Escape goes back,
    // F1 shows help.
    let menu = Some(Screen::Pause);
    let in_menu = Pauses {
        menu: true,
        ..Pauses::default()
    };
    assert_eq!(
        game_key(Command::Pause, ctx(menu, in_menu)),
        GameKey::Nothing
    );
    assert_eq!(game_key(Command::Quit, ctx(menu, in_menu)), GameKey::Menu);
    assert_eq!(game_key(Command::Help, ctx(menu, in_menu)), GameKey::Help);
    // On the help screen neither key does anything (it closes on its own
    // keys).
    let help = Some(Screen::Help);
    for c in [Command::Pause, Command::Help] {
        assert_eq!(game_key(c, ctx(help, in_menu)), GameKey::Nothing, "{c:?}");
    }
    // A gamepad's Start opens the menu, as the original joystick's menu
    // button.
    let pad = KeyContext {
        from_pad: true,
        ..ctx(play, free)
    };
    assert_eq!(game_key(Command::Pause, pad), GameKey::Menu);
    // A complete level waits for its tally: no pause, no help.
    let done = KeyContext {
        completing: true,
        ..ctx(play, free)
    };
    assert_eq!(game_key(Command::Pause, done), GameKey::Nothing);
    assert_eq!(game_key(Command::Help, done), GameKey::Nothing);
    // Without menus (watching a script): Escape quits, the pause key
    // pauses.
    assert_eq!(game_key(Command::Quit, ctx(None, free)), GameKey::Quit);
    assert_eq!(game_key(Command::Pause, ctx(None, free)), GameKey::Pause);
    assert_eq!(game_key(Command::Help, ctx(None, free)), GameKey::Nothing);
    // There is no menu for a gamepad's Start to open: it pauses and goes
    // on as the pause key does.
    let pad = |pauses| KeyContext {
        from_pad: true,
        ..ctx(None, pauses)
    };
    assert_eq!(game_key(Command::Pause, pad(free)), GameKey::Pause);
    assert_eq!(game_key(Command::Pause, pad(paused)), GameKey::Pause);
}

fn in_game_menu() -> MenuTree {
    let node = |command: u32, children: Vec<IfcNode>| {
        let mut n = IfcNode {
            command,
            label: format!("{command}"),
            children,
            ..IfcNode::default()
        };
        n.slots[0] = "001_main_x".into();
        n.slots[3] = "002_main_x".into();
        n
    };
    let leaf = |c| node(c, vec![]);
    let root = node(
        20000,
        vec![
            leaf(20001),
            node(20002, vec![leaf(22001), leaf(22002)]),
            node(20003, vec![leaf(23001), leaf(23004)]),
            leaf(20004),
            node(20005, vec![leaf(24001), leaf(24002)]),
        ],
    );
    MenuTree::from_ifc(root, &CommandCatalogue::new(MenuKind::InGame))
}

#[test]
fn f1_and_the_menus_help_entry_show_the_help_screen_until_it_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
    settings.config_path = None;
    let mut flow = Flow::new(settings, None, None, FontArt::uniform(8, 10), Start::Play);
    flow.set_in_game_menu(Some(in_game_menu()));
    flow.take_effects();
    // F1 in play.
    assert!(flow.help_key());
    assert_eq!(flow.screen(), Screen::Help);
    assert_eq!(flow.take_effects(), [Effect::PauseGame, Effect::ShowHelp]);
    assert!(!flow.help_key(), "F1 on the help screen does nothing");
    for close in [MenuInput::Back, MenuInput::Confirm] {
        flow.menu_input(close, false);
        assert_eq!(flow.screen(), Screen::Play);
        assert_eq!(flow.take_effects(), [Effect::ResumeGame]);
        assert!(flow.help_key());
        flow.take_effects();
    }
    flow.menu_input(MenuInput::Back, false);
    flow.take_effects();
    // The in-game menu's Help entry (and F1 there) takes its place.
    assert!(flow.pause_key());
    flow.take_effects();
    flow.hotkey('H');
    flow.update(PRESS_MS + 1.0, None, None);
    assert_eq!(flow.screen(), Screen::Help);
    let effects = flow.take_effects();
    assert!(effects.contains(&Effect::ShowHelp), "{effects:?}");
    assert!(!effects.contains(&Effect::ResumeGame));
    flow.menu_input(MenuInput::Back, false);
    assert_eq!(flow.screen(), Screen::Play);
    assert!(flow.take_effects().contains(&Effect::ResumeGame));
}

// ------------------------------------------------------------- stepping

#[test]
fn each_resume_runs_exactly_one_tick_whatever_the_pause_lasted() {
    for rule in PauseInput::ALL {
        let mut g = Game::new(sim_with(FuseClock::Original), rule);
        g.ticks(10);
        // Half a tick waits in the accumulator when the pause comes: it is
        // dropped, and paused time is never caught up.
        g.s.frame(Duration::from_millis(9));
        let mut t = g.s.ticks();
        for wall in [5, 500, 2000, 18, 1] {
            g.stop(Span::Pause);
            g.wait(wall);
            g.go_on(Buttons::NONE);
            t += 1;
            assert_eq!(g.s.ticks(), t, "{rule:?} after {wall} ms");
        }
        // Then real time counts from an empty accumulator.
        assert_eq!(g.s.frame(Duration::from_micros(17_999)).ticks, 0);
        assert_eq!(g.s.frame(Duration::from_micros(1)).ticks, 1);
        // A pause right after a resume, before the next frame, still runs
        // the resume's tick.
        let t = g.s.ticks();
        g.stop(Span::Pause);
        g.pauses.user = false;
        g.pauses.apply(Some(&mut g.s), None, Duration::ZERO);
        g.s.close_span();
        g.stop(Span::Pause);
        assert_eq!(g.s.ticks(), t + 1);
        g.go_on(Buttons::NONE);
        assert_eq!(g.s.ticks(), t + 2);
    }
}

#[test]
fn stepping_through_pauses_plays_the_same_ticks_as_without_them() {
    // Walking right with pauses between ticks gives the same game, tick
    // for tick, as walking right without them.
    let run = |pauses: bool| {
        let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
        g.ticks(5);
        g.key(RIGHT, true);
        for _ in 0..10 {
            if pauses {
                g.stop(Span::Pause);
                g.wait(300);
                g.go_on(RIGHT);
            } else {
                g.ticks(1);
            }
        }
        g.ticks(5);
        rows(g.s.trace())
    };
    assert_eq!(run(true), run(false));
}

// ------------------------------------------------- keys across a pause

#[test]
fn a_jump_held_through_a_pause_is_held_not_pressed_and_replays_bit_identically() {
    let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
    g.ticks(30);
    g.key(RIGHT, true);
    g.ticks(10);
    g.key(JUMP, true);
    g.ticks(4);
    assert_eq!(g.s.sim().player().unwrap().state, state::JUMP);
    let at = g.s.ticks();
    g.stop(Span::Pause);
    // Nothing is released at the pause.
    assert_eq!(g.s.keys_down(), RIGHT | JUMP);
    g.wait(700);
    g.go_on(RIGHT | JUMP);
    let r = g.row(at);
    assert_eq!(r.held, (RIGHT | JUMP).0, "held on the resume tick");
    assert_eq!(r.pressed, 0, "not a new press");
    assert_eq!(r.state_id, state::JUMP, "the jump goes on");
    g.ticks(20);
    g.key(JUMP, false);
    g.key(RIGHT, false);
    g.ticks(40);
    let script = g.script();
    assert_eq!(
        script.directives(),
        &[Directive {
            tick: at as usize,
            kind: DirectiveKind::Pause {
                wall_ms: 700,
                held: None
            },
        }]
    );
    // Through the file format, twice at each display rate.
    let text = recording_text(&script, false, PauseInput::Original, FuseClock::Original);
    let back = InputScript::parse(&text).unwrap();
    assert_eq!(back, script);
    let live = rows(&g.s.trace()[..script.ticks.len()]);
    for hz in [60.0, 144.0, 240.0] {
        let first = replay(sim_with(FuseClock::Original), &back, hz);
        assert_eq!(first, live, "{hz} Hz");
        assert_eq!(replay(sim_with(FuseClock::Original), &back, hz), first);
    }
}

/// Stands still, pauses before tick 40, does `during` while paused and
/// goes on with `down` held; the input of the resume tick.
fn resume_input(rule: PauseInput, during: &[(Buttons, bool)], down: Buttons) -> (u32, u32, i32) {
    let mut g = Game::new(sim_with(FuseClock::Original), rule);
    g.ticks(40);
    g.stop(Span::Pause);
    for &(b, d) in during {
        g.key(b, d);
        g.wait(50);
    }
    g.go_on(down);
    let r = g.row(40);
    (r.held, r.pressed, r.state_id)
}

#[test]
fn a_key_pressed_during_a_pause_is_pressed_on_the_resume_tick() {
    // Up before the pause and down at the resume: a press, and the jump
    // starts on the resume tick itself.
    let (held, pressed, s) = resume_input(PauseInput::Original, &[(JUMP, true)], JUMP);
    assert_eq!((held, pressed, s), (JUMP.0, JUMP.0, state::JUMP));
    // A press and release inside the pause is lost.
    let (held, pressed, _) = resume_input(
        PauseInput::Original,
        &[(JUMP, true), (JUMP, false)],
        Buttons::NONE,
    );
    assert_eq!((held, pressed), (0, 0));
}

#[test]
fn a_release_and_press_again_inside_a_pause_is_no_press() {
    let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
    g.ticks(30);
    g.key(JUMP, true);
    g.ticks(5);
    while g.s.sim().player().unwrap().state != state::STAND {
        g.ticks(1);
    }
    // Landed with jump held; it is released and pressed again in a pause.
    g.ticks(2);
    let at = g.s.ticks();
    g.stop(Span::Pause);
    g.key(JUMP, false);
    g.wait(100);
    g.key(JUMP, true);
    g.go_on(JUMP);
    let r = g.row(at);
    assert_eq!((r.held, r.pressed), (JUMP.0, 0));
    assert_eq!(r.state_id, state::STAND, "no jump");
}

#[test]
fn with_release_all_keys_are_released_at_the_pause_and_presses_in_it_count() {
    // A jump held into the pause is released and is not held after it.
    let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::ReleaseAll);
    g.ticks(30);
    g.key(RIGHT, true);
    g.ticks(5);
    g.stop(Span::Pause);
    assert_eq!(g.s.keys_down(), Buttons::NONE);
    g.go_on(RIGHT);
    assert_eq!(g.row(35).held, 0, "released until pressed again");
    // A press and release inside the pause still counts as a press.
    let (held, pressed, _) = resume_input(
        PauseInput::ReleaseAll,
        &[(JUMP, true), (JUMP, false)],
        Buttons::NONE,
    );
    assert_eq!((held, pressed), (JUMP.0, JUMP.0));
}

#[test]
fn no_key_stays_stuck_after_switching_away_during_a_pause() {
    let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
    g.ticks(30);
    g.key(RIGHT, true);
    g.ticks(5);
    g.stop(Span::Pause);
    // The window loses focus: the router lets go of every key.
    g.key(RIGHT, false);
    g.pauses.background = true;
    g.pauses.apply(Some(&mut g.s), None, Duration::ZERO);
    g.wait(1000);
    g.pauses.background = false;
    g.go_on(Buttons::NONE);
    assert_eq!(g.row(35).held, 0);
    g.ticks(10);
    assert!(g.s.trace()[35..].iter().all(|r| r.held == 0));
}

// ------------------------------------------------ keys across a screen

#[test]
fn after_a_screen_keys_down_are_held_and_none_is_pressed() {
    for screen in [MenuScreen::InGame, MenuScreen::Help] {
        let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
        g.ticks(30);
        g.key(RIGHT, true);
        g.ticks(5);
        g.stop(Span::Menu(screen));
        // In the menu, jump goes down (the screen takes the key; the
        // window reports it at the return).
        g.wait(400);
        g.go_on(RIGHT | JUMP);
        let r = g.row(35);
        assert_eq!((r.held, r.pressed), ((RIGHT | JUMP).0, 0), "{screen:?}");
        assert_ne!(r.state_id, state::JUMP, "{screen:?}: no jump");
        // Held on: still no jump; released and pressed again: a jump.
        g.ticks(3);
        g.key(JUMP, false);
        g.ticks(2);
        g.key(JUMP, true);
        g.ticks(1);
        assert_eq!(g.s.sim().player().unwrap().state, state::JUMP, "{screen:?}");
        assert!(g.s.trace()[35..40].iter().all(|r| r.held & RIGHT.0 != 0));
    }
}

// ----------------------------------------------------------- the fuse

/// A game whose stick of dynamite, lit before the first tick beside Claw,
/// sees `span` of `wall_ms` open at the boundary before tick 19 (after 324
/// ms of fuse). Returns the tick its blast hurt Claw and the game.
fn fuse_game(clock: FuseClock, span: Option<(Span, u64)>) -> (usize, Game) {
    let mut g = Game::new(stick_sim(clock), PauseInput::Original);
    g.ticks(19);
    if let Some((span, wall)) = span {
        g.stop(span);
        g.wait(wall);
        g.go_on(Buttons::NONE);
    }
    g.until(120);
    let hurt =
        g.s.trace()
            .iter()
            .position(|r| r.health < 100)
            .expect("the blast hurt Claw");
    (hurt, g)
}

/// The level with a lit stick of dynamite at Claw's feet, lit at time 0.
fn stick_sim(clock: FuseClock) -> Sim {
    let mut sim = sim_with(clock);
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let spec = ProjectileSpec {
        logic: "Dynamite",
        x: x + 20,
        y: y - 10,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        ..ProjectileSpec::default()
    };
    sim.with_world(|w| projectile::spawn(w, spec)).expect("lit");
    sim
}

#[test]
fn the_fuse_burns_through_help_only_and_replays_the_same() {
    let (base, _) = fuse_game(FuseClock::Original, None);
    assert_eq!(base, 79, "lit in tick 0: off at 70, hurts at 79");
    // The pause and the in-game menu freeze the fuse.
    for span in [Span::Pause, Span::Menu(MenuScreen::InGame)] {
        for wall in [500, 1000, 2000] {
            let (hurt, _) = fuse_game(FuseClock::Original, Some((span, wall)));
            assert_eq!(hurt, base, "{span:?} {wall} ms");
        }
    }
    // Help: 324 ms of fuse plus 918 ms is 1242 ms, the stick goes off on
    // its own tick; 936 ms make 1260 and it goes off at the return, its
    // blast hurting on the 8th tick after it.
    let (hurt, _) = fuse_game(
        FuseClock::Original,
        Some((Span::Menu(MenuScreen::Help), 918)),
    );
    assert_eq!(hurt, base);
    for wall in [936, 2000] {
        let (hurt, g) = fuse_game(
            FuseClock::Original,
            Some((Span::Menu(MenuScreen::Help), wall)),
        );
        assert_eq!(hurt, 19 + 8, "help of {wall} ms");
        // The recording replays it: the help directive passes its time.
        let script = g.script();
        assert!(matches!(
            script.directives()[0].kind,
            DirectiveKind::Menu {
                screen: MenuScreen::Help,
                ..
            }
        ));
        let live = rows(&g.s.trace()[..script.ticks.len()]);
        for hz in [60.0, 144.0] {
            assert_eq!(replay(stick_sim(FuseClock::Original), &script, hz), live);
        }
    }
    // With the fuse clock frozen, help freezes it too.
    let (hurt, _) = fuse_game(
        FuseClock::Frozen,
        Some((Span::Menu(MenuScreen::Help), 2000)),
    );
    assert_eq!(hurt, base);
}

#[test]
fn a_replay_watched_with_a_pause_at_a_help_boundary_plays_as_headless() {
    // The help screen recorded at tick 19 burns the fuse (see above). A
    // viewer pauses and resumes right at that boundary and pauses again
    // before the next frame: the resume's tick waits for the recorded
    // help, so the stick goes off on the same tick as in the headless
    // replay.
    let (_, g) = fuse_game(
        FuseClock::Original,
        Some((Span::Menu(MenuScreen::Help), 2000)),
    );
    let script = g.script();
    let headless = replay(stick_sim(FuseClock::Original), &script, 60.0);
    let mut s = Session::new(stick_sim(FuseClock::Original)).with_replay(script.clone());
    s.record_trace();
    while s.ticks() < 19 {
        s.frame(Duration::from_millis(18));
    }
    let mut p = Pauses::default();
    let toggle = |p: &mut Pauses, s: &mut Session, on: bool| {
        p.user = on;
        p.apply(Some(s), None, Duration::ZERO);
        if on {
            s.open_span(Span::Pause);
        } else {
            s.close_span();
        }
    };
    toggle(&mut p, &mut s, true);
    toggle(&mut p, &mut s, false);
    toggle(&mut p, &mut s, true);
    assert_eq!(s.ticks(), 19, "the resume's tick waits for the help");
    assert!(s.holding());
    s.frame(Duration::from_millis(500));
    toggle(&mut p, &mut s, false);
    while s.replaying() {
        s.frame(Duration::from_millis(18));
    }
    assert_eq!(rows(&s.trace()[..script.ticks.len()]), headless);
    let hurt = s.trace().iter().position(|r| r.health < 100);
    assert_eq!(hurt, Some(19 + 8));
}

#[test]
fn the_music_stays_paused_after_help_over_a_pause_until_the_pause_key() {
    let mut audio = AudioBridge::silent(AudioSettings::default());
    let mut s = Session::new(sim_with(FuseClock::Original));
    let mut p = Pauses::default();
    let since = Duration::ZERO;
    s.frame(Duration::from_millis(18));
    // The pause key.
    p.user = true;
    assert_eq!(
        p.apply(Some(&mut s), Some(&mut audio), since),
        Change::Paused
    );
    assert!(audio.is_paused());
    // F1 over the pause: help shows and the pause key's flag stays set
    // (the window turns the pause into the flag).
    p.follow(&Effect::PauseGame, Some(&mut s), Some(&mut audio), since);
    p.user = false;
    p.flag = true;
    // Help closes: play runs, the music stays paused.
    assert_eq!(
        p.follow(&Effect::ResumeGame, Some(&mut s), Some(&mut audio), since),
        Change::Resumed
    );
    assert!(!s.is_paused());
    assert!(audio.is_paused(), "the music waits for the pause key");
    assert_eq!(s.frame(Duration::from_millis(18)).ticks, 1, "play runs");
    // Another pass of the sounds changes nothing while the flag is set.
    p.apply(Some(&mut s), Some(&mut audio), since);
    assert!(audio.is_paused());
    // The pause key clears the flag: a pause of no time, and the music
    // carries on.
    p.flag = false;
    p.user = true;
    p.apply(Some(&mut s), Some(&mut audio), since);
    p.user = false;
    assert_eq!(
        p.apply(Some(&mut s), Some(&mut audio), since),
        Change::Resumed
    );
    assert!(!audio.is_paused());
    // Without a level in play the flag pauses nothing.
    p.flag = true;
    p.apply(None, Some(&mut audio), since);
    assert!(!audio.is_paused());
}

// ------------------------------------------------------ the stand window

#[test]
fn a_jump_pressed_during_a_pause_lands_in_the_stand_window() {
    // A shot hits Claw rising; 22 ticks of Hit, Stand, and the one tick
    // that takes a jump. Paused at the boundary before it, a jump pressed
    // during the pause launches on the resume tick.
    for paused in [true, false] {
        let mut g = Game::new(sim_with(FuseClock::Original), PauseInput::Original);
        g.s.set_before_tick(Box::new(|n, sim: &mut Sim| {
            if n == 36 {
                let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
                let ball = ProjectileSpec {
                    logic: "Bullet",
                    x: x + 5,
                    y,
                    z: 300,
                    vx: -500,
                    damage: 10,
                    type_flags: types::ENEMY_SHOT,
                    image_set: "LEVEL_MUSKETBALL",
                    ..ProjectileSpec::default()
                };
                sim.with_world(|w| projectile::spawn(w, ball));
            }
        }));
        g.ticks(30);
        g.key(JUMP, true);
        g.ticks(1);
        g.key(JUMP, false);
        while g.s.sim().stats().health == 100 {
            g.ticks(1);
        }
        let h = g.s.ticks() - 1;
        g.until(h + 23);
        assert_eq!(g.s.sim().player().unwrap().state, state::STAND);
        if paused {
            g.stop(Span::Pause);
            g.key(JUMP, true);
            g.wait(400);
            g.go_on(JUMP);
        } else {
            // Too late without the pause: pressed a tick after the window.
            g.ticks(1);
            g.key(JUMP, true);
            g.ticks(1);
        }
        g.ticks(3);
        let launched = g.s.trace()[h as usize + 23..]
            .iter()
            .any(|r| r.state_id == state::JUMP);
        assert_eq!(launched, paused);
    }
}

// -------------------------------------------------------------- switches

#[test]
fn the_switches_default_to_the_original_and_are_kept_in_recordings() {
    assert_eq!(PauseInput::default(), PauseInput::Original);
    assert_eq!(FuseClock::default(), FuseClock::Original);
    assert_eq!(
        Session::new(sim_with(FuseClock::Original)).pause_input(),
        PauseInput::Original
    );
    assert_eq!(
        switch_comments(PauseInput::Original, FuseClock::Original),
        "# pause_input = original\n# fuse_clock = original"
    );
    assert_eq!(
        script_switches("# pause_input = release_all\n# fuse_clock = frozen\n"),
        ScriptSwitches {
            pause_input: Some(PauseInput::ReleaseAll),
            fuse_clock: Some(FuseClock::Frozen),
        }
    );
    assert_eq!(
        script_switches("# pause_input = sometimes\n"),
        ScriptSwitches::default()
    );
    assert_eq!(
        (PAUSE_INPUT_KEY, FUSE_CLOCK_KEY),
        ("pause_input", "fuse_clock")
    );
    // A saved recording carries every switch the session played with.
    let mut g = Game::new(sim_with(FuseClock::Frozen), PauseInput::ReleaseAll);
    g.ticks(5);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rec.csv");
    save_recording(&g.rec, &g.s, &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.starts_with(
            "# mid_air_hit_fix = off\n# pause_input = release_all\n# fuse_clock = frozen\n"
        ),
        "{text}"
    );
    assert_eq!(InputScript::parse(&text).unwrap(), g.script());
    // A replay follows them, unless the command line says otherwise.
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "install = /i\npause_input = original\n").unwrap();
    let args = |extra: &[&str]| {
        let mut v = vec![
            "rusty-claw",
            "--config",
            config.to_str().unwrap(),
            "--replay",
            path.to_str().unwrap(),
        ];
        v.extend_from_slice(extra);
        <Args as clap::Parser>::try_parse_from(v).unwrap()
    };
    let s = Settings::from_args(&args(&[])).unwrap();
    assert_eq!(
        (s.pause_input, s.fuse_clock),
        (PauseInput::ReleaseAll, FuseClock::Frozen)
    );
    assert_eq!(s.sim_config((640, 480)).fuse_clock, FuseClock::Frozen);
    let s = Settings::from_args(&args(&["--fuse-clock", "original"])).unwrap();
    assert_eq!(s.fuse_clock, FuseClock::Original);
}

#[test]
fn the_switches_are_settable_in_the_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(
        &path,
        "install = /i\npause_input = release_all\nfuse_clock = frozen\n",
    )
    .unwrap();
    let args = |extra: &[&str]| {
        let mut v = vec![
            "rusty-claw",
            "--config",
            path.to_str().unwrap(),
            "--level",
            "1",
        ];
        v.extend_from_slice(extra);
        <Args as clap::Parser>::try_parse_from(v).unwrap()
    };
    let s = Settings::from_args(&args(&[])).unwrap();
    assert_eq!(
        (s.pause_input, s.fuse_clock),
        (PauseInput::ReleaseAll, FuseClock::Frozen)
    );
    assert_eq!(
        s.config_value("pause_input").as_deref(),
        Some("release_all")
    );
    assert_eq!(s.config_value("fuse_clock").as_deref(), Some("frozen"));
    let s = Settings::from_args(&args(&["--pause-input", "original"])).unwrap();
    assert_eq!(s.pause_input, PauseInput::Original);
    std::fs::write(&path, "install = /i\npause_input = maybe\n").unwrap();
    assert!(Settings::from_args(&args(&[])).is_err());
    // Without the lines: the original.
    std::fs::write(&path, "install = /i\n").unwrap();
    let s = Settings::from_args(&args(&[])).unwrap();
    assert_eq!(
        (s.pause_input, s.fuse_clock),
        (PauseInput::Original, FuseClock::Original)
    );
}

// --------------------------------------------------------------- install

#[test]
fn install_the_help_picture_is_found() {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return;
    };
    let store = claw_assets::Install::open(root).unwrap().store().unwrap();
    let assets = claw_ui::screens::ScreenAssets::load(store);
    let (img, _) = assets
        .picture(
            claw_ui::screens::assets::HELP_SCREENS,
            claw_ui::screens::assets::HELP_PICTURE,
        )
        .expect("the help picture");
    assert_eq!((img.width, img.height), (640, 480));
}
