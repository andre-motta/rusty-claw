//! A saved game round trip and the end-of-level tally through the game
//! flow, on your own install (skipped without `CLAW_INSTALL`), with the
//! saved games and the config file in a temporary folder.
//!
//! A new game starts at level 1 from the main menu. Claw takes some
//! treasure and reaches the second save point, which saves the game; then
//! he takes the map piece, and the tally shows what he collected against
//! what the level holds. The game is quit, and a second run loads the save
//! point from the menu: Claw starts there with the progress he had.
//!
//! A recorded game goes from level 1 to level 2: the committed level 1
//! route wins level 1, the tally leads straight into level 2 with the
//! progress carried, and Claw plays there a while. Each level is recorded
//! into its own file, and replaying the files, each from its level's
//! start, plays the same game twice, level 1 ending with the progress
//! level 2 starts with.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use claw_acceptance::install;
use claw_acceptance::play::{floor_under, records};
use claw_assets::Install;
use claw_frame::Buttons;
use claw_frame::GameEvent;
use claw_game::flow::{self, Effect, FADE_MS, Flow, Screen, Start};
use claw_game::load::{LevelBundle, load_level};
use claw_game::saves::SaveStore;
use claw_game::session::{GameRecording, LevelRecording, Session, recorded_game_files};
use claw_game::settings::{LevelChoice, Settings, UserDirs};
use claw_render::LevelGraphics;
use claw_sim::trace::TraceRow;
use claw_sim::{Carry, InputScript, Sim, StartState};
use claw_ui::menu::command::{CommandCatalogue, MenuKind, Page};
use claw_ui::menu::draw::FontArt;
use claw_ui::menu::nav::PRESS_MS;
use claw_ui::menu::{MenuInput, MenuTree};
use claw_ui::screens::BootyStats;
use claw_ui::screens::assets::{MENU_PICTURE, MENU_SCREENS, ScreenAssets};
use claw_ui::screens::booty::{BootyArt, BootyScreen};
use claw_ui::screens::title::TitleScreen;
use tempfile::TempDir;

/// The game flow with a level in play, driven frame by frame as the window
/// drives it.
struct Game {
    flow: Flow,
    in_game: MenuTree,
    install: PathBuf,
    settings: Settings,
    level: Option<(LevelBundle, Session)>,
    started: Vec<StartState>,
    booty: Vec<BootyStats>,
    events: Vec<GameEvent>,
    quit: bool,
}

/// User folders inside `root`.
fn dirs(root: &Path) -> UserDirs {
    let get = |k: &str| -> Option<OsString> {
        match k {
            "HOME" => Some(root.join("home").into()),
            "XDG_DATA_HOME" => Some(root.join("data").into()),
            "XDG_CONFIG_HOME" => Some(root.join("config").into()),
            _ => None,
        }
    };
    UserDirs::from_lookup(get)
}

impl Game {
    fn new(install: &Path, tmp: &Path) -> Game {
        let store = Install::open(install).unwrap().store().unwrap();
        let assets = ScreenAssets::load(store);
        let (_, pal) = assets.picture(MENU_SCREENS, MENU_PICTURE).unwrap();
        let mut g = LevelGraphics::for_screens(pal, Box::new(assets.images()));
        let font = FontArt::from_art(&mut g);
        let main = MenuTree::from_ifc(
            assets.menu(MenuKind::Main).unwrap(),
            &CommandCatalogue::new(MenuKind::Main),
        );
        let d = dirs(tmp);
        let mut settings = Settings::new(install, LevelChoice::Retail(1));
        settings.config_path = d.config_file();
        let saves = SaveStore::open_default(&d, Some(install)).unwrap();
        assert!(
            saves.dir().starts_with(tmp),
            "saves go to the temporary folder"
        );
        let mut flow = Flow::new(
            settings.clone(),
            Some(saves),
            Some(main),
            font,
            Start::Title,
        );
        flow.measure(MenuKind::Main, &mut g);
        let in_game = MenuTree::from_ifc(
            assets.menu(MenuKind::InGame).unwrap(),
            &CommandCatalogue::new(MenuKind::InGame),
        );
        let mut game = Game {
            flow,
            in_game,
            install: install.to_path_buf(),
            settings,
            level: None,
            started: Vec::new(),
            booty: Vec::new(),
            events: Vec::new(),
            quit: false,
        };
        game.effects();
        game
    }

    fn effects(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                match e {
                    Effect::StartLevel { level, start } => {
                        let bundle = load_level(&self.install, &level).expect("level loads");
                        let sim = Sim::with_start(
                            bundle.sim_level.clone(),
                            self.settings.sim_config((640, 480)),
                            start.clone(),
                        );
                        self.level = Some((bundle, Session::new(sim)));
                        self.started.push(start);
                        self.flow.set_in_game_menu(Some(self.in_game.clone()));
                    }
                    Effect::EndLevel => self.level = None,
                    Effect::PauseGame => {
                        if let Some((_, s)) = self.level.as_mut() {
                            s.pause();
                        }
                    }
                    Effect::ResumeGame => {
                        if let Some((_, s)) = self.level.as_mut() {
                            s.resume();
                        }
                    }
                    Effect::ShowTitle => self.flow.set_title(TitleScreen::new(
                        None,
                        claw_assets::Palette::default(),
                        Some(10),
                    )),
                    Effect::ShowBooty(stats) => {
                        self.booty.push(stats.clone());
                        let art = BootyArt {
                            map: (None, claw_assets::Palette::default()),
                            tally: (None, claw_assets::Palette::default()),
                            sound_ms: BTreeMap::new(),
                            digit_width: 12,
                            total_offsets: (60, 64),
                        };
                        self.flow.set_booty(BootyScreen::new(stats, art));
                    }
                    Effect::Quit => self.quit = true,
                    _ => {}
                }
            }
        }
    }

    /// Runs `ms` of frames of about 16.7 ms.
    fn run(&mut self, ms: f64) {
        let mut t = 0.0;
        while t < ms {
            let dt = 16.667;
            t += dt;
            if let Some((_, session)) = self.level.as_mut() {
                session.frame(Duration::from_secs_f64(dt / 1000.0));
                let (_, events) = session.drain_outputs();
                self.events.extend(events.iter().copied());
                self.flow.play_events(&events, session.sim());
            }
            let sim = self.level.as_ref().map(|(_, s)| s.sim());
            self.flow.update(dt, None, sim);
            self.effects();
        }
    }

    fn input(&mut self, i: MenuInput) {
        self.flow.menu_input(i, false);
        self.effects();
    }

    fn choose(&mut self) {
        self.input(MenuInput::Confirm);
        self.run(PRESS_MS + 20.0);
    }

    fn down(&mut self, n: usize) {
        for _ in 0..n {
            self.input(MenuInput::Down);
        }
    }

    fn reach_main_menu(&mut self) {
        for _ in 0..100 {
            if self.flow.screen() == Screen::MainMenu {
                self.run(FADE_MS + 20.0);
                return;
            }
            self.run(100.0);
        }
        panic!("never reached the main menu: {:?}", self.flow.screen());
    }

    fn sim(&self) -> &Sim {
        self.level.as_ref().expect("a level in play").1.sim()
    }

    /// Moves Claw to (x, y) before the next tick: a scripted segment.
    fn teleport(&mut self, x: i32, y: i32) {
        if let Some((_, session)) = self.level.as_mut() {
            let mut done = false;
            session.set_before_tick(Box::new(move |_, sim: &mut Sim| {
                if !done {
                    done = true;
                    sim.teleport_player(x, y);
                }
            }));
        }
    }
}

#[test]
fn install_a_saved_game_round_trip_and_the_tally_through_the_flow() {
    let Some(install) = install() else { return };
    let tmp = TempDir::new().unwrap();

    // A new game of level 1 from the main menu.
    let mut game = Game::new(&install, tmp.path());
    game.reach_main_menu();
    game.choose();
    assert_eq!(game.flow.main_menu().unwrap().page(), Page::SinglePlayer);
    game.choose();
    assert_eq!(game.flow.main_menu().unwrap().page(), Page::NewGameLevels);
    game.choose();
    game.run(FADE_MS + 50.0);
    assert_eq!(game.flow.screen(), Screen::Play);
    assert_eq!(game.started, vec![StartState::default()]);
    game.run(300.0);
    let level = game.sim().level().clone();

    // Treasure: Claw is dropped onto the first treasure that lies over a
    // floor, until he has taken some.
    for (x, y, _) in records(&level, "TreasurePowerup") {
        if game.sim().stats().score >= 1000 {
            break;
        }
        if floor_under(&level, x, y, 60).is_some() {
            game.teleport(x, y - 10);
            game.run(800.0);
        }
    }
    let score = game.sim().stats().score;
    assert!(score > 0, "Claw took treasure");

    // The second save point saves the game.
    let (sx, sy, _) = records(&level, "SecondSuperCheckpoint")[0];
    game.events.clear();
    game.teleport(sx, sy);
    game.run(3000.0);
    assert!(
        game.events.contains(&GameEvent::SavePoint { slot: 2 }),
        "the second save point saved: {:?}",
        game.events
    );
    let carry = game.sim().carry();
    let stood = game.sim().player().map(|o| (o.x, o.y)).unwrap();

    // The map piece ends the level: the tally shows what Claw collected
    // against the level's census.
    let collected = game.sim().stats().treasure_level;
    let census = game.sim().treasure_census();
    let lives = game.sim().stats().lives;
    let piece = level
        .objects
        .iter()
        .find(|o| o.image_set == "GAME_MAPPIECE")
        .map(|o| (o.record[5], o.record[6]))
        .expect("the map piece");
    game.events.clear();
    game.teleport(piece.0, piece.1);
    game.run(500.0);
    assert!(
        game.events
            .iter()
            .any(|e| matches!(e, GameEvent::LevelComplete { .. })),
        "the map piece ends the level"
    );
    game.run(4000.0);
    assert_eq!(game.flow.screen(), Screen::Booty);
    let tally = game.booty.last().expect("the tally is shown").clone();
    assert_eq!(tally.level, 1);
    assert!(!tally.custom);
    assert_eq!(
        tally.collected, collected,
        "the tally counts what Claw took"
    );
    assert_eq!(tally.census, census, "against what the level holds");
    assert_eq!(tally.score, score);
    assert_eq!(tally.lives, lives);
    assert!(
        tally
            .collected
            .iter()
            .zip(&tally.census)
            .all(|(c, n)| c <= n)
    );
    println!(
        "tally: {:?} of {:?}, score {}",
        tally.collected, tally.census, tally.score
    );
    drop(game);

    // A second run loads the save point from the menu.
    let mut game = Game::new(&install, tmp.path());
    game.reach_main_menu();
    game.choose();
    game.down(1);
    game.choose();
    assert_eq!(game.flow.main_menu().unwrap().page(), Page::LoadGameLevels);
    game.choose();
    assert_eq!(
        game.flow.main_menu().unwrap().page(),
        Page::LoadEntries { level: 1 }
    );
    game.down(2);
    game.choose();
    game.run(FADE_MS + 50.0);
    assert_eq!(game.flow.screen(), Screen::Play);
    let start = game.started.last().unwrap().clone();
    assert_eq!(start.save.map(|s| s.slot), Some(2), "save point 2 loaded");
    assert_eq!(start.carry.as_ref(), Some(&carry));
    let at = game.sim().player().map(|o| (o.x, o.y)).unwrap();
    assert_eq!(at, stood, "Claw starts where he was saved");
    assert_eq!(game.sim().carry(), carry, "with the saved progress");
    assert_eq!(game.sim().stats().score, score);
}

/// A game played through the flow from level 1, with the committed level 1
/// route as the input of level 1, recorded level by level as the window
/// records it.
struct RecordedGame {
    flow: Flow,
    install: PathBuf,
    settings: Settings,
    route: InputScript,
    session: Option<Session>,
    recording: GameRecording,
    /// Per level left: its number, trace and the progress at its end.
    levels: Vec<(u32, Vec<TraceRow>, Carry)>,
    started: Vec<(u32, StartState)>,
}

impl RecordedGame {
    fn new(install: &Path, tmp: &Path, route: InputScript) -> RecordedGame {
        let d = dirs(tmp);
        let mut settings = Settings::new(install, LevelChoice::Retail(1));
        settings.config_path = d.config_file();
        settings.follow_header(&route.header);
        let saves = SaveStore::open_default(&d, Some(install)).unwrap();
        let flow = Flow::new(
            settings.clone(),
            Some(saves),
            None,
            FontArt::uniform(8, 10),
            Start::Play,
        );
        let mut g = RecordedGame {
            flow,
            install: install.to_path_buf(),
            settings,
            route,
            session: None,
            recording: GameRecording::new(tmp.join("game.csv")),
            levels: Vec::new(),
            started: Vec::new(),
        };
        g.effects();
        g
    }

    fn leave(&mut self) {
        if let Some(s) = self.session.take() {
            let (_, ticks) = self.recording.finish(&s).unwrap().unwrap();
            assert_eq!(ticks as u64, s.ticks());
            let n = self.started.last().unwrap().0;
            self.levels.push((n, s.trace().to_vec(), s.sim().carry()));
        }
    }

    fn effects(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                match e {
                    Effect::StartLevel { level, start } => {
                        let n = flow::retail_number(&level).expect("a retail level");
                        let bundle = load_level(&self.install, &level).expect("level loads");
                        let sim = Sim::with_start(
                            bundle.sim_level.clone(),
                            self.settings.sim_config(self.settings.res),
                            start.clone(),
                        );
                        let mut session = Session::new(sim);
                        if n == 1 {
                            session = session.with_replay(self.route.clone());
                        }
                        session.record_trace();
                        self.recording.start(&mut session, Some(n), start.clone());
                        self.session = Some(session);
                        self.started.push((n, start));
                    }
                    Effect::EndLevel => self.leave(),
                    Effect::ShowBooty(stats) => {
                        let art = BootyArt {
                            map: (None, claw_assets::Palette::default()),
                            tally: (None, claw_assets::Palette::default()),
                            sound_ms: BTreeMap::new(),
                            digit_width: 12,
                            total_offsets: (60, 64),
                        };
                        self.flow.set_booty(BootyScreen::new(stats, art));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Runs `ms` of frames of about 16.7 ms.
    fn run(&mut self, ms: f64) {
        let mut t = 0.0;
        while t < ms {
            let dt = 16.667;
            t += dt;
            if let Some(session) = self.session.as_mut() {
                session.frame(Duration::from_secs_f64(dt / 1000.0));
                let (_, events) = session.drain_outputs();
                self.flow.play_events(&events, session.sim());
            }
            let sim = self.session.as_ref().map(Session::sim);
            self.flow.update(dt, None, sim);
            self.effects();
        }
    }

    fn key(&mut self, b: Buttons, down: bool) {
        let s = self.session.as_mut().expect("a level in play");
        let at = s.stamp(Duration::from_millis(4));
        s.key(at, b, down);
    }
}

/// `rows` as CSV with the input source left out.
fn trace_csv(rows: &[TraceRow]) -> String {
    let rows: Vec<TraceRow> = rows
        .iter()
        .cloned()
        .map(|mut r| {
            r.source = String::new();
            r
        })
        .collect();
    claw_sim::trace::to_csv(&rows)
}

#[test]
fn install_a_recorded_game_goes_from_level_1_to_level_2_and_replays_the_same() {
    let Some(install) = install() else { return };
    let tmp = TempDir::new().unwrap();
    let route_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("routes/level1.csv");
    let route = InputScript::parse(&std::fs::read_to_string(&route_path).unwrap()).unwrap();
    assert_eq!(route.header.level, Some(1));

    // Level 1 plays its route until the tally.
    let mut g = RecordedGame::new(&install, tmp.path(), route.clone());
    assert_eq!(g.flow.screen(), Screen::Play);
    let mut guard = 0;
    while g.flow.screen() == Screen::Play {
        g.run(1000.0);
        guard += 1;
        assert!(guard < 600, "level 1 is never won");
    }
    assert_eq!(g.flow.screen(), Screen::Booty);
    assert_eq!(g.levels.len(), 1);
    let end1 = g.levels[0].2.clone();
    // Finishing level 1 saved level 2's start.
    let mut store = SaveStore::open_default(&dirs(tmp.path()), Some(&install)).unwrap();
    let saved = store.read("LEVEL2", 3).unwrap().expect("level 2's start");
    assert_eq!(saved.carry, end1);
    drop(store);

    // The tally leads into level 2 with the progress carried.
    for _ in 0..3 {
        g.flow.menu_input(MenuInput::Confirm, false);
        g.effects();
    }
    let mut guard = 0;
    while g.flow.screen() != Screen::Play {
        g.run(100.0);
        guard += 1;
        assert!(guard < 100, "stuck at {:?}", g.flow.screen());
    }
    assert_eq!(g.flow.level(), Some(&LevelChoice::Retail(2)));
    let (n, start2) = g.started[1].clone();
    assert_eq!(n, 2);
    let carry2 = start2.carry.clone().expect("progress carried");
    assert_eq!(
        Carry {
            lives: end1.lives,
            ..carry2.clone()
        },
        end1,
        "level 2 starts with level 1's progress"
    );
    assert!((0..=1).contains(&(carry2.lives - end1.lives)));
    assert_eq!(g.session.as_ref().unwrap().sim().carry(), carry2);

    // Level 2: walk right, jump, stop; then the game is left.
    g.run(400.0);
    g.key(Buttons::RIGHT, true);
    g.run(2000.0);
    g.key(Buttons::JUMP, true);
    g.run(300.0);
    g.key(Buttons::JUMP, false);
    g.run(1000.0);
    g.key(Buttons::RIGHT, false);
    g.run(500.0);
    g.leave();
    assert_eq!(g.levels.len(), 2);

    // One file per level, replayed each from its own start.
    let files = recorded_game_files(&tmp.path().join("game.csv"));
    assert_eq!(files, g.recording.written());
    assert_eq!(files.len(), 2);
    let recs: Vec<LevelRecording> = files
        .iter()
        .map(|p| LevelRecording::read(p).unwrap())
        .collect();
    assert_eq!(recs[0].level(), Some(1));
    assert_eq!(recs[1].level(), Some(2));
    assert_eq!(recs[0].start, StartState::default());
    assert_eq!(recs[1].start, start2);
    assert_eq!(recs[1].script.header.seed, route.header.seed, "the seed");
    let replay = |rec: &LevelRecording| {
        let choice = LevelChoice::Retail(rec.level().unwrap());
        let bundle = load_level(&install, &choice).unwrap();
        // A fresh game's settings: the recording says how it was played.
        let base = Settings::new(&install, choice);
        let sim = Sim::with_start(
            bundle.sim_level.clone(),
            rec.sim_config(base.sim_config(base.res)),
            rec.start.clone(),
        );
        let s = rec.replay(sim);
        assert_eq!(s.ticks(), rec.script.ticks.len() as u64);
        (trace_csv(s.trace()), s.sim().carry())
    };
    for (i, rec) in recs.iter().enumerate() {
        let (first, end) = replay(rec);
        let (second, again) = replay(rec);
        let n = g.levels[i].0;
        assert_eq!(first, second, "level {n} replays the same twice");
        assert_eq!(end, again);
        assert_eq!(first, trace_csv(&g.levels[i].1), "level {n} as played");
        assert_eq!(end, g.levels[i].2, "level {n} ends as played");
        if n == 1 {
            assert!(first.contains("level_complete"), "level 1 is won");
            assert_eq!(end, end1);
        }
        println!(
            "level {n}: {} ticks replayed twice, score {} lives {}",
            rec.script.ticks.len(),
            end.score,
            end.lives
        );
    }
}
