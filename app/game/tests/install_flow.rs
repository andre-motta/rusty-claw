//! The game flow on your own install, without a window (skipped without
//! `CLAW_INSTALL`): with temporary data and config folders, a new game
//! starts at level 1 from the main menu, Claw goes to the level's first
//! save point, the game is ended and quit; a second run then loads that
//! save point from the menu and Claw arrives where he was saved, with the
//! same progress.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use claw_assets::Install;
use claw_frame::GameEvent;
use claw_game::flow::{Effect, FADE_MS, Flow, Screen, Start};
use claw_game::load::{LevelBundle, load_level};
use claw_game::saves::SaveStore;
use claw_game::session::Session;
use claw_game::settings::{LevelChoice, Settings, UserDirs};
use claw_render::LevelGraphics;
use claw_sim::{Carry, Sim, StartState};
use claw_ui::menu::command::{CommandCatalogue, MenuCommand, MenuKind, Page};
use claw_ui::menu::draw::FontArt;
use claw_ui::menu::layout::CANVAS;
use claw_ui::menu::nav::PRESS_MS;
use claw_ui::menu::{MenuInput, MenuTree};
use claw_ui::screens::assets::{MENU_PICTURE, MENU_SCREENS, ScreenAssets};
use claw_ui::screens::booty::{BootyArt, BootyScreen};
use claw_ui::screens::title::TitleScreen;
use tempfile::TempDir;

/// The flow with a level in play, driven frame by frame.
struct Game {
    flow: Flow,
    in_game: MenuTree,
    install: PathBuf,
    settings: Settings,
    level: Option<(LevelBundle, Session)>,
    started: Vec<StartState>,
    quit: bool,
    events: Vec<GameEvent>,
}

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
            quit: false,
            events: Vec::new(),
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

    fn carry(&self) -> Carry {
        self.sim().carry()
    }
}

#[test]
fn a_save_point_reached_in_a_new_game_loads_in_the_next_run() {
    let Some(install) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let tmp = TempDir::new().unwrap();

    // First run: title, main menu, single player, new game, level 1.
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
    game.run(500.0);

    // A scripted segment: Claw is put on the first save point and stands.
    let flag = game
        .sim()
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "FirstSuperCheckpoint")
        .map(|(_, o)| (o.x, o.y))
        .expect("level 1 has a first save point");
    if let Some((_, session)) = game.level.as_mut() {
        let mut done = false;
        session.set_before_tick(Box::new(move |_, sim: &mut Sim| {
            if !done {
                done = true;
                sim.teleport_player(flag.0, flag.1);
            }
        }));
    }
    game.events.clear();
    game.run(3000.0);
    // The save point itself saves: touching it sends the save point event.
    assert!(
        game.events
            .iter()
            .any(|e| matches!(e, GameEvent::SavePoint { slot: 1 })),
        "touching the first save point sent no save point event: {:?}",
        game.events
    );
    let carry = game.carry();
    // Where Claw stands on the floor below the save point.
    let stood = game.sim().player().map(|o| (o.x, o.y)).unwrap();
    let mut store = SaveStore::open_default(&dirs(tmp.path()), Some(&install)).unwrap();
    let saved = store
        .read("LEVEL1", 1)
        .unwrap()
        .expect("save point 1 saved");
    assert_eq!(saved.carry, carry);
    let saved_pos = (saved.x, saved.y);
    drop(store);

    // End the game, then quit from the main menu.
    assert!(game.flow.pause_key());
    game.effects();
    game.down(4);
    game.choose();
    assert_eq!(
        game.flow.in_game_menu().unwrap().page(),
        Page::EndGameConfirm
    );
    game.choose();
    assert_eq!(game.flow.screen(), Screen::MainMenu);
    game.run(FADE_MS + 20.0);
    game.input(MenuInput::Back);
    game.run(PRESS_MS + 20.0);
    game.choose();
    game.run(FADE_MS + 50.0);
    assert!(game.quit);
    drop(game);

    // Second run: load game, level 1, save point 1.
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
    game.down(1);
    game.choose();
    game.run(FADE_MS + 50.0);
    assert_eq!(game.flow.screen(), Screen::Play);
    let start = game.started.last().unwrap().clone();
    assert_eq!(start.save.map(|s| (s.x, s.y)), Some(saved_pos));
    assert_eq!(start.carry.as_ref(), Some(&carry));
    // The save point keeps a respawn point above its flag; Claw starts
    // there and drops onto the floor where he stood when it saved.
    assert_eq!(saved_pos.0, flag.0, "saved at the save point");
    assert!(saved_pos.1 <= stood.1, "saved at or above the floor");
    let at = game.sim().player().map(|o| (o.x, o.y)).unwrap();
    assert_eq!(at, stood, "Claw starts at the save point");
    assert_eq!(game.carry(), carry, "with the saved progress");
}

#[test]
fn the_controls_page_fits_the_canvas_with_the_real_font() {
    let Some(install) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let tmp = TempDir::new().unwrap();
    let mut game = Game::new(&install, tmp.path());
    game.reach_main_menu();
    for (command, page) in [
        (MenuCommand::Open(Page::Options), Page::Options),
        (MenuCommand::Controls, Page::Controls),
    ] {
        let nav = game.flow.main_menu().unwrap();
        let at = nav
            .entries()
            .iter()
            .position(|&k| nav.tree.items[k].command == command)
            .unwrap_or_else(|| panic!("{:?} lists {command:?}", nav.page()));
        game.down(at);
        game.choose();
        assert_eq!(game.flow.main_menu().unwrap().page(), page);
    }
    let nav = game.flow.main_menu().unwrap();
    assert_eq!(nav.page(), Page::Controls);
    let cells: Vec<_> = nav
        .entries()
        .iter()
        .map(|&k| (nav.tree.items[k].command, nav.tree.items[k].cell))
        .collect();
    assert!(
        cells.iter().any(|(c, _)| *c == MenuCommand::PadDeadZone),
        "the dead zone row is listed"
    );
    for (c, cell) in &cells {
        assert!(
            cell.x >= 0
                && cell.y >= 0
                && cell.x + cell.w <= CANVAS.0
                && cell.y + cell.h <= CANVAS.1,
            "{c:?} at {cell:?} leaves the canvas"
        );
    }
    for pair in cells.windows(2) {
        let ((a, ca), (b, cb)) = (pair[0], pair[1]);
        assert!(
            ca.y + ca.h <= cb.y,
            "{a:?} at {ca:?} overlaps {b:?} at {cb:?}"
        );
    }
}

/// The entries `nav` shows, with their cells, in order.
fn cells(nav: &claw_ui::menu::Navigator) -> Vec<(MenuCommand, claw_ui::menu::Cell)> {
    nav.entries()
        .iter()
        .map(|&k| (nav.tree.items[k].command, nav.tree.items[k].cell))
        .collect()
}

/// Every entry inside the canvas, each below the one before.
fn assert_fits(cells: &[(MenuCommand, claw_ui::menu::Cell)]) {
    for (c, cell) in cells {
        assert!(
            cell.x >= 0
                && cell.y >= 0
                && cell.x + cell.w <= CANVAS.0
                && cell.y + cell.h <= CANVAS.1,
            "{c:?} at {cell:?} leaves the canvas"
        );
    }
    for pair in cells.windows(2) {
        let ((a, ca), (b, cb)) = (pair[0], pair[1]);
        assert!(
            ca.y + ca.h <= cb.y,
            "{a:?} at {ca:?} overlaps {b:?} at {cb:?}"
        );
    }
}

#[test]
fn the_gameplay_page_fits_both_menus_and_applies_to_the_next_game() {
    let Some(install) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return;
    };
    let tmp = TempDir::new().unwrap();
    let mut game = Game::new(&install, tmp.path());
    game.reach_main_menu();
    // Main menu: options, then gameplay.
    let gameplay = MenuCommand::Open(Page::Gameplay);
    for (command, page) in [
        (MenuCommand::Open(Page::Options), Page::Options),
        (gameplay, Page::Gameplay),
    ] {
        let nav = game.flow.main_menu().unwrap();
        let at = nav
            .entries()
            .iter()
            .position(|&k| nav.tree.items[k].command == command)
            .unwrap_or_else(|| panic!("{:?} lists {command:?}", nav.page()));
        if page == Page::Gameplay {
            assert_fits(&cells(nav));
        }
        game.down(at);
        game.choose();
        assert_eq!(game.flow.main_menu().unwrap().page(), page);
    }
    let nav = game.flow.main_menu().unwrap();
    assert_fits(&cells(nav));
    assert_eq!(
        nav.tree.items[nav.selected().unwrap()].text.as_deref(),
        Some("MID AIR HIT JUMP FIX OFF")
    );
    game.choose();
    assert!(game.flow.settings().mid_air_hit_fix);
    let config = game.flow.settings().config_path.clone().unwrap();
    let text = std::fs::read_to_string(config).unwrap();
    assert!(text.lines().any(|l| l == "mid_air_hit_fix = on"), "{text}");
    // A new game plays with it, as the window starts it.
    game.settings.mid_air_hit_fix = game.flow.settings().mid_air_hit_fix;
    game.input(MenuInput::Back);
    game.input(MenuInput::Back);
    game.run(PRESS_MS + 20.0);
    assert_eq!(game.flow.main_menu().unwrap().page(), Page::Root);
    for command in [
        MenuCommand::Open(Page::SinglePlayer),
        MenuCommand::Open(Page::NewGameLevels),
        MenuCommand::NewGame { level: 1 },
    ] {
        let nav = game.flow.main_menu().unwrap();
        let entries = nav.entries().to_vec();
        let at = entries
            .iter()
            .position(|&k| nav.tree.items[k].command == command)
            .unwrap_or_else(|| panic!("{:?} lists {command:?}", nav.page()));
        let from = entries
            .iter()
            .position(|&k| Some(k) == nav.selected())
            .unwrap_or(0);
        for _ in at..from {
            game.input(MenuInput::Up);
        }
        game.down(at.saturating_sub(from));
        game.choose();
    }
    game.run(FADE_MS + 50.0);
    assert_eq!(game.flow.screen(), Screen::Play);
    assert!(game.sim().config().tuning.behaviour.fix_double_jump);
    // The in-game options list it too, measured with the level's art.
    let (bundle, _) = game.level.as_ref().unwrap();
    let mut graphics = claw_game::load_graphics(bundle, None).unwrap();
    game.flow.measure(MenuKind::InGame, &mut graphics);
    assert!(game.flow.pause_key());
    game.effects();
    let nav = game.flow.in_game_menu().unwrap();
    let at = nav
        .entries()
        .iter()
        .position(|&k| nav.tree.items[k].command == MenuCommand::Open(Page::Options))
        .unwrap();
    game.down(at);
    game.choose();
    let nav = game.flow.in_game_menu().unwrap();
    assert_eq!(nav.page(), Page::Options);
    assert_fits(&cells(nav));
    let at = nav
        .entries()
        .iter()
        .position(|&k| nav.tree.items[k].command == gameplay)
        .expect("the in-game options list the gameplay page");
    game.down(at);
    game.choose();
    let nav = game.flow.in_game_menu().unwrap();
    assert_eq!(nav.page(), Page::Gameplay);
    assert_fits(&cells(nav));
}
