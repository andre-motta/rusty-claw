//! The game flow without a window: synthetic menus and levels, game events
//! as the simulation sends them, saves in a temporary folder and options
//! written to a temporary config file; the pauses and sounds of play
//! around the in-game menu, with a silent sound engine; and a game
//! recorded across two levels and replayed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use claw_assets::Palette;
use claw_assets::ifc::IfcNode;
use claw_audio::music::Track;
use claw_audio::{AudioSettings, AudioStats, Pcm, SoundBank};
use claw_frame::{AudioCue, Buttons, GameEvent, SoundChannel, SoundCue, Weapon};
use claw_game::Session;
use claw_game::audio::{AudioBridge, ViewInfo};
use claw_game::flow::{
    self, Effect, FADE_MS, Flow, IDLE_ROOT_MS, MENU_BED, MENU_BED_VOICE, MovieShelf, Progress,
    START_SLOT, Screen, Start, control_row, replay_movie_index,
};
use claw_game::keymap::{Keymap, PadMap};
use claw_game::load::start_override;
use claw_game::pause::{PauseInput, Pauses};
use claw_game::saves::{SaveRecord, SaveStore, retail_level_name};
use claw_game::session::{GameRecording, LevelRecording, Span, recorded_game_files, start_sim};
use claw_game::settings::{LevelChoice, Settings};
use claw_movie::{MovieIndex, MovieSet};
use claw_sim::level::ObjectSpec;
use claw_sim::logic::checkpoint::{MESSAGE_NOT_SAVED, MESSAGE_SAVED};
use claw_sim::trace::TraceRow;
use claw_sim::tuning::{FuseClock, PlayAids};
use claw_sim::{Carry, SavePos, Sim, SimConfig, SimLevel, StartState, TileMap};
use claw_ui::menu::command::{
    CommandCatalogue, GameplaySetting, MenuCommand, MenuKind, Page, Unavailable,
};
use claw_ui::menu::draw::FontArt;
use claw_ui::menu::nav::PRESS_MS;
use claw_ui::menu::{MenuInput, MenuTree};
use claw_ui::screens::booty::{BootyArt, BootyPhase, BootyScreen};
use claw_ui::screens::credits::{
    self, BLACK_MS, CreditsArt, CreditsEntry, CreditsPhase, CreditsScreen, FontMetrics, UNROLL_MS,
};
use claw_ui::screens::title::TitleScreen;
use tempfile::TempDir;

fn node(command: u32, children: Vec<IfcNode>) -> IfcNode {
    let mut n = IfcNode {
        command,
        label: format!("{command}"),
        children,
        ..IfcNode::default()
    };
    n.slots[0] = "001_main_x".into();
    n.slots[3] = "002_main_x".into();
    n
}

fn leaf(command: u32) -> IfcNode {
    node(command, vec![])
}

/// A main menu with the game's structure: single player (new game, load
/// game, back), options (audio, display, controls, back) and quit.
fn main_menu() -> MenuTree {
    let new_list = (11101..=11114).map(leaf).chain([leaf(11115)]).collect();
    let load_list = (1..=14u32)
        .map(|n| {
            node(
                11200 + n,
                (1..=4).map(|k| leaf(112_000 + 10 * n + k)).collect(),
            )
        })
        .chain([leaf(11215)])
        .collect();
    let root = node(
        10000,
        vec![
            node(
                10001,
                vec![node(11001, new_list), node(11002, load_list), leaf(11006)],
            ),
            node(
                10004,
                vec![
                    node(
                        13004,
                        vec![
                            leaf(13401),
                            leaf(13402),
                            leaf(13403),
                            leaf(13404),
                            leaf(13405),
                        ],
                    ),
                    node(13005, vec![leaf(13503), leaf(13504), leaf(13506)]),
                    leaf(13003),
                    leaf(13006),
                ],
            ),
            node(10006, vec![leaf(14001), leaf(14002)]),
        ],
    );
    MenuTree::from_ifc(root, &CommandCatalogue::new(MenuKind::Main))
}

fn in_game_menu() -> MenuTree {
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

fn level() -> SimLevel {
    let mut rows = vec![".".repeat(60); 15];
    rows.push("#".repeat(60));
    rows.push("#".repeat(60));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    SimLevel::synthetic(TileMap::from_ascii(64, &rows), (500, 600))
}

const CONFIG: &str = "\
# my settings
install = \"/somewhere/else\"   # keep this comment
voice = on
res = \"1280x960\"

# the end
";

/// The flow and a pretend window around it.
struct Rig {
    flow: Flow,
    sim: Option<Sim>,
    started: Vec<(LevelChoice, StartState)>,
    captures: Vec<(claw_game::keymap::Action, bool)>,
    sounds: Vec<String>,
    /// Music started (track, looped), and `None` for a stop.
    music: Vec<Option<(Track, bool)>>,
    ended: usize,
    paused: bool,
    quit: bool,
    /// Movies asked for, in order.
    movies: Vec<MovieIndex>,
    /// How many times the intro was recorded as seen.
    intro_records: usize,
    /// Stops of the menu's loop.
    bed_stops: usize,
    _dir: TempDir,
    saves: PathBuf,
    config: PathBuf,
}

impl Rig {
    fn new(start: Start, prepare: impl FnOnce(&Path)) -> Rig {
        Rig::with_level(start, LevelChoice::Retail(1), prepare)
    }

    /// A rig whose settings name `level` (played at once with
    /// [`Start::Play`]).
    fn with_level(start: Start, level: LevelChoice, prepare: impl FnOnce(&Path)) -> Rig {
        Rig::build(start, level, main_menu(), prepare)
    }

    fn build(start: Start, level: LevelChoice, menu: MenuTree, prepare: impl FnOnce(&Path)) -> Rig {
        Rig::build_with(start, level, menu, None, prepare)
    }

    /// A rig whose window found the movies of `shelf` (handed over before
    /// the first effects, as the window does).
    fn with_movies(start: Start, level: LevelChoice, menu: MenuTree, shelf: MovieShelf) -> Rig {
        Rig::build_with(start, level, menu, Some(shelf), |_| {})
    }

    fn build_with(
        start: Start,
        level: LevelChoice,
        menu: MenuTree,
        shelf: Option<MovieShelf>,
        prepare: impl FnOnce(&Path),
    ) -> Rig {
        let dir = TempDir::new().unwrap();
        let saves = dir.path().join("saves");
        fs::create_dir_all(&saves).unwrap();
        prepare(&saves);
        let config = dir.path().join("config.toml");
        fs::write(&config, CONFIG).unwrap();
        let mut settings = Settings::new(dir.path().join("install"), level);
        settings.config_path = Some(config.clone());
        let store = SaveStore::open(&saves).unwrap();
        let mut flow = Flow::new(
            settings,
            Some(store),
            Some(menu),
            FontArt::uniform(8, 10),
            start,
        );
        if let Some(shelf) = shelf {
            flow.set_movies(shelf);
        }
        let mut rig = Rig {
            flow,
            sim: None,
            started: Vec::new(),
            captures: Vec::new(),
            sounds: Vec::new(),
            music: Vec::new(),
            ended: 0,
            paused: false,
            quit: false,
            movies: Vec::new(),
            intro_records: 0,
            bed_stops: 0,
            _dir: dir,
            saves,
            config,
        };
        rig.effects();
        rig
    }

    fn effects(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                match e {
                    Effect::StartLevel { level: l, start } => {
                        self.sim = Some(Sim::with_start(
                            level(),
                            SimConfig::default(),
                            start.clone(),
                        ));
                        self.started.push((l, start));
                        self.flow.set_in_game_menu(Some(in_game_menu()));
                    }
                    Effect::EndLevel => {
                        self.sim = None;
                        self.ended += 1;
                    }
                    Effect::PauseGame => self.paused = true,
                    Effect::ResumeGame => self.paused = false,
                    Effect::Sound(AudioCue::Play(c)) => self.sounds.push(c.key),
                    Effect::Sound(AudioCue::StopMusic) => self.music.push(None),
                    Effect::Music { track, looped } => self.music.push(Some((track, looped))),
                    Effect::ShowCredits(entry) => {
                        let mut art = CreditsArt::plain(FontMetrics::uniform(8));
                        art.tune_ms = Some(5000.0);
                        self.flow.set_credits(CreditsScreen::new(
                            entry,
                            "THE END\r\n\r\nTHANK YOU FOR PLAYING\r\n",
                            art,
                        ));
                    }
                    Effect::ShowTitle => {
                        self.flow
                            .set_title(TitleScreen::new(None, Palette::default(), Some(1000)))
                    }
                    Effect::ShowBooty(stats) => {
                        let art = BootyArt {
                            map: (None, Palette::default()),
                            tally: (None, Palette::default()),
                            sound_ms: BTreeMap::new(),
                            digit_width: 12,
                            total_offsets: (60, 64),
                        };
                        self.flow.set_booty(BootyScreen::new(stats, art));
                    }
                    Effect::Quit => self.quit = true,
                    Effect::Capture { action, pad } => self.captures.push((action, pad)),
                    Effect::PlayMovie(index) => self.movies.push(index),
                    Effect::RecordIntroSeen => self.intro_records += 1,
                    Effect::Sound(AudioCue::Stop {
                        voice: MENU_BED_VOICE,
                    }) => self.bed_stops += 1,
                    _ => {}
                }
            }
        }
    }

    fn update(&mut self, ms: f64) {
        // In steps of at most 50 ms, as frames would.
        let mut left = ms;
        while left > 0.0 {
            let dt = left.min(50.0);
            left -= dt;
            if let Some(sim) = self.sim.as_mut()
                && !self.paused
            {
                sim.step(Default::default());
            }
            self.flow.update(dt, None, self.sim.as_ref());
            self.effects();
        }
    }

    fn input(&mut self, i: MenuInput) {
        self.flow.menu_input(i, false);
        self.effects();
    }

    /// Chooses the highlighted entry and waits for its press.
    fn choose(&mut self) {
        self.input(MenuInput::Confirm);
        self.update(PRESS_MS + 1.0);
    }

    fn events(&mut self, events: &[GameEvent]) {
        let sim = self.sim.as_ref().expect("a level in play");
        self.flow.play_events(events, sim);
        self.effects();
    }

    fn reach_main_menu(&mut self) {
        let mut guard = 0;
        while self.flow.screen() != Screen::MainMenu {
            self.update(100.0);
            guard += 1;
            assert!(guard < 1000, "stuck at {:?}", self.flow.screen());
        }
        // Let the menu fade in.
        self.update(FADE_MS);
    }

    fn page(&self) -> Page {
        self.flow.main_menu().unwrap().page()
    }
}

#[test]
fn the_title_leads_to_the_main_menu() {
    let mut r = Rig::new(Start::Title, |_| {});
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(r.sounds.contains(&"TITLE".to_string()));
    r.update(999.0);
    assert_eq!(r.flow.screen(), Screen::Title);
    r.update(1.0);
    r.update(FADE_MS);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(r.sounds.contains(&MENU_BED.to_string()));
    assert!(r.sounds.contains(&"INTRO".to_string()));
    // A key skips the title.
    let mut r = Rig::new(Start::Title, |_| {});
    r.update(FADE_MS);
    r.input(MenuInput::Confirm);
    r.update(FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
}

#[test]
fn new_game_level_complete_tally_then_level_2_with_the_carried_progress() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert!(!r.flow.progress().complete(1));
    r.choose(); // single player
    assert_eq!(r.page(), Page::SinglePlayer);
    r.choose(); // new game
    assert_eq!(r.page(), Page::NewGameLevels);
    let level2 = r
        .flow
        .main_menu()
        .unwrap()
        .tree
        .find(MenuCommand::NewGame { level: 2 });
    assert!(!r.flow.main_menu().unwrap().tree.items[level2.unwrap()].enabled);
    r.choose(); // level 1
    r.update(FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::Play);
    assert_eq!(
        r.started,
        vec![(LevelChoice::Retail(1), StartState::default())]
    );
    // Play a little, then the level is won.
    r.update(500.0);
    let carry = r.sim.as_ref().unwrap().carry();
    r.events(&[GameEvent::LevelComplete { after_ms: 1000 }]);
    r.update(900.0);
    assert_eq!(
        r.flow.screen(),
        Screen::Play,
        "the level ends after its delay"
    );
    assert!(
        !r.flow.pause_key() || r.flow.screen() == Screen::Play,
        "no pausing now"
    );
    r.update(150.0);
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert_eq!(r.ended, 1);
    // The next level's start was written with the carried progress.
    let mut store = SaveStore::open(&r.saves).unwrap();
    let rec = store
        .read("LEVEL2", START_SLOT)
        .unwrap()
        .expect("LEVEL2 start");
    assert_eq!(rec.carry, carry, "the carried state is produced and kept");
    assert_eq!(rec.save_pos(), None, "a level start, not a save point");
    drop(store);
    assert!(r.flow.progress().complete(1));
    let (next, start) = r.flow.next_level().expect("the tally leads on");
    assert_eq!((next, start), (2, &rec.start_state()));
    // The tally: skip to the tally, finish it, end the hold.
    let booty = r.flow.booty().expect("tally screens");
    assert_eq!(booty.rows().len(), 9);
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    // The tally fades out into level 2, started from the saved start.
    let mut guard = 0;
    while r.started.len() < 2 {
        r.update(100.0);
        guard += 1;
        assert!(guard < 100, "stuck at {:?}", r.flow.screen());
    }
    assert_eq!(r.flow.screen(), Screen::Play);
    assert_eq!(r.flow.level(), Some(&LevelChoice::Retail(2)));
    // The synthetic level holds no treasure: a perfect tally, one more
    // life.
    let carried = Carry {
        lives: carry.lives + 1,
        ..carry.clone()
    };
    assert_eq!(
        r.started[1],
        (
            LevelChoice::Retail(2),
            StartState {
                carry: Some(carried.clone()),
                save: None,
            }
        )
    );
    assert_eq!(r.sim.as_ref().unwrap().carry(), carried, "Claw carries on");
    assert!(r.flow.booty().is_none() && r.flow.next_level().is_none());
    assert!(r.flow.in_game_menu().is_some(), "the in-game menu is set");
    // Level 2 plays and its in-game menu works.
    r.update(200.0);
    assert!(r.flow.pause_key());
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Pause);
    assert!(r.paused);
    assert!(r.flow.pause_key());
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Play);
    // Level 2 opens in the new game and load lists.
    let nav = r.flow.main_menu().unwrap();
    let new2 = nav.tree.find(MenuCommand::NewGame { level: 2 }).unwrap();
    let load2 = nav
        .tree
        .find(MenuCommand::Load { level: 2, entry: 0 })
        .unwrap();
    let new3 = nav.tree.find(MenuCommand::NewGame { level: 3 }).unwrap();
    assert!(
        nav.tree.items[new2].enabled,
        "level 2 opens in the new game list"
    );
    assert!(
        nav.tree.items[load2].enabled,
        "and its start in the load list"
    );
    assert!(!nav.tree.items[new3].enabled);
}

/// Wins the level in play and skips through its tally until the flow
/// leaves it.
fn win_and_skip_the_tally(r: &mut Rig) {
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    assert_eq!(r.flow.screen(), Screen::Booty);
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    let mut guard = 0;
    while r.flow.screen() == Screen::Booty {
        r.update(100.0);
        guard += 1;
        assert!(guard < 100, "stuck at the tally");
    }
}

/// Wins level 14 and goes through its black hold and its tally to the
/// credits.
fn finish_the_game(r: &mut Rig) {
    r.update(300.0);
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert_eq!(r.flow.booty().unwrap().level(), 14);
    assert!(r.flow.next_level().is_none(), "no level after the last");
    // The black in place of the movie: the map screen waits, its sounds
    // too.
    assert!(r.flow.cut_screen());
    assert_eq!(r.flow.brightness(), 0.0);
    let before = r.sounds.len();
    r.update(flow::CUT_SCREEN_MS - 300.0);
    assert!(r.flow.cut_screen());
    assert_eq!(r.sounds.len(), before, "no map screen sound in the black");
    r.update(300.0);
    assert!(!r.flow.cut_screen());
    assert!(r.sounds.len() > before, "the map screen's sounds");
    r.update(FADE_MS);
    assert_eq!(r.flow.brightness(), 1.0);
    // The tally, skipped.
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    let mut guard = 0;
    while r.flow.screen() == Screen::Booty {
        r.update(50.0);
        guard += 1;
        assert!(guard < 100, "stuck at the tally");
    }
}

#[test]
fn the_end_of_level_14_leads_to_the_credits_then_the_main_menu() {
    let mut r = Rig::with_level(Start::Play, LevelChoice::Retail(14), |_| {});
    assert_eq!(r.flow.screen(), Screen::Play);
    finish_the_game(&mut r);
    let mut store = SaveStore::open(&r.saves).unwrap();
    assert!(store.list().unwrap().is_empty(), "no level 15 to save");
    drop(store);
    assert_eq!(r.flow.screen(), Screen::Credits);
    let c = r.flow.credits().expect("the credits");
    assert_eq!(c.entry(), CreditsEntry::AfterGame);
    assert_eq!(c.phase(), CreditsPhase::Black);
    assert_eq!(r.flow.brightness(), 0.0);
    assert!(
        r.sounds
            .contains(&credits::DELAYED_UNROLL_SOUND.to_string())
    );
    assert!(r.music.is_empty(), "no music before the text");
    // Keys while the parchment opens do nothing.
    r.input(MenuInput::Confirm);
    r.update(BLACK_MS);
    assert_eq!(r.flow.credits().unwrap().phase(), CreditsPhase::Unroll);
    r.update(UNROLL_MS + 20.0);
    assert_eq!(r.flow.screen(), Screen::Credits);
    assert_eq!(r.flow.credits().unwrap().phase(), CreditsPhase::Text);
    assert_eq!(r.music, vec![Some((Track::Credits, true))]);
    // The text rises and loops for as long as nobody presses anything.
    r.update(60_000.0);
    assert_eq!(r.flow.screen(), Screen::Credits);
    assert!(r.flow.credits().unwrap().loops() >= 1);
    // A direction does not leave; Enter does, straight to the menu.
    r.input(MenuInput::Down);
    assert_eq!(r.flow.screen(), Screen::Credits);
    let sounds = r.sounds.len();
    r.input(MenuInput::Confirm);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert_eq!(r.music.last(), Some(&None), "the credits music stops");
    assert_eq!(r.flow.brightness(), 1.0, "no unroll on the menu side");
    assert!(!r.sounds[sounds..].contains(&"UNROLL".to_string()));
    assert!(r.sounds[sounds..].contains(&MENU_BED.to_string()));
    assert!(r.flow.credits().is_none());
    assert_eq!(r.started.len(), 1, "no level after the credits");
    assert_eq!(r.ended, 1);
    assert_eq!(r.page(), Page::Root);
}

#[test]
fn the_credits_leave_on_a_game_key_or_a_click_and_the_corner_plays_the_tune() {
    let mut r = Rig::with_level(Start::Play, LevelChoice::Retail(14), |_| {});
    finish_the_game(&mut r);
    // Presses and clicks while the parchment opens are ignored.
    r.flow.credits_press();
    r.flow.click(300, 300);
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Credits);
    r.update(BLACK_MS + UNROLL_MS + 20.0);
    // The corner toggles the developer's tune.
    r.flow.click(20, 20);
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Credits);
    assert!(r.flow.credits().unwrap().monolith());
    assert_eq!(r.music.last(), Some(&Some((Track::Monolith, false))));
    r.update(5000.0);
    assert!(!r.flow.credits().unwrap().monolith(), "the tune ended");
    assert_eq!(r.music.last(), Some(&Some((Track::Credits, true))));
    // A game key leaves.
    r.flow.credits_press();
    r.effects();
    assert_eq!(r.flow.screen(), Screen::MainMenu);

    let mut r = Rig::with_level(Start::Play, LevelChoice::Retail(14), |_| {});
    finish_the_game(&mut r);
    r.update(BLACK_MS + UNROLL_MS + 20.0);
    // So does a click outside the corner.
    r.flow.click(320, 240);
    r.effects();
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert_eq!(r.music.last(), Some(&None));
}

/// The synthetic main menu with the Credits entry before Quit.
fn main_menu_with_credits() -> MenuTree {
    let mut tree = main_menu();
    let root = 0;
    let quit = *tree.items[root].children.last().unwrap();
    let credits = tree.add_text(
        root,
        "CREDITS",
        MenuCommand::Unavailable(Unavailable::Credits),
    );
    let kids = &mut tree.items[root].children;
    let n = kids.len();
    kids.swap(n - 1, n - 2);
    assert_eq!(tree.items[root].children[n - 1], quit);
    assert_eq!(tree.items[root].children[n - 2], credits);
    tree
}

#[test]
fn the_main_menu_credits_entry_opens_the_credits() {
    let mut r = Rig::build(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu_with_credits(),
        |_| {},
    );
    r.reach_main_menu();
    let entries = main_entries(&r);
    let at = entries
        .iter()
        .position(|&c| c == MenuCommand::Unavailable(Unavailable::Credits))
        .expect("a Credits entry");
    let nav = r.flow.main_menu().unwrap();
    let id = nav.entries()[at];
    assert!(nav.tree.items[id].enabled, "the credits can be opened");
    for _ in 0..at {
        r.input(MenuInput::Down);
    }
    r.update(100.0);
    r.sounds.clear();
    r.choose();
    // The menu rolls up, then the parchment unrolls at once.
    assert!(r.sounds.contains(&"ROLLUP".to_string()));
    r.update(FADE_MS);
    assert_eq!(r.flow.screen(), Screen::Credits);
    let c = r.flow.credits().unwrap();
    assert_eq!(c.entry(), CreditsEntry::FromMenu);
    assert_eq!(c.phase(), CreditsPhase::Unroll);
    assert!(r.sounds.contains(&credits::UNROLL_SOUND.to_string()));
    r.update(UNROLL_MS + 20.0);
    assert_eq!(r.music, vec![Some((Track::Credits, true))]);
    r.input(MenuInput::Back);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(r.started.is_empty());
}

#[test]
fn the_tally_of_level_13_leads_to_level_14() {
    let mut r = Rig::with_level(Start::Play, LevelChoice::Retail(13), |_| {});
    r.update(300.0);
    win_and_skip_the_tally(&mut r);
    assert_eq!(r.flow.level(), Some(&LevelChoice::Retail(14)));
    assert_eq!(r.started.len(), 2);
}

#[test]
fn a_custom_level_tally_leads_to_the_main_menu() {
    let mut r = Rig::with_level(Start::Play, LevelChoice::Custom("X.WWD".into()), |_| {});
    r.update(300.0);
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert!(r.flow.booty().unwrap().phase() == claw_ui::screens::BootyPhase::Counting);
    assert!(r.flow.next_level().is_none());
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    r.reach_main_menu();
    assert_eq!(r.started.len(), 1);
}

#[test]
fn a_perfect_tally_carries_its_extra_life_into_the_next_level_only() {
    let mut r = Rig::new(Start::Play, |_| {});
    r.update(300.0);
    let carry = r.sim.as_ref().unwrap().carry();
    assert!(carry.lives < 10);
    // The synthetic level holds no treasure, so every row is complete.
    win_and_skip_the_tally(&mut r);
    assert_eq!(r.flow.level(), Some(&LevelChoice::Retail(2)));
    let start = r.started[1].1.clone();
    let got = start.carry.expect("progress carried");
    assert_eq!(got.lives, carry.lives + 1, "the perfect tally's life");
    assert_eq!(
        Carry {
            lives: carry.lives,
            ..got
        },
        carry
    );
    let mut store = SaveStore::open(&r.saves).unwrap();
    let rec = store.read("LEVEL2", START_SLOT).unwrap().unwrap();
    assert_eq!(rec.carry, carry, "the saved start has no extra life");
}

#[test]
fn saves_and_settings_are_kept_across_the_level_change() {
    // An earlier game left saves of level 1 and 2 and a level 3 start.
    let old = |level: &str, slot: u8, score: u32| {
        SaveRecord::new(
            level,
            SavePos {
                slot,
                x: if slot == START_SLOT { 0 } else { 100 },
                y: if slot == START_SLOT { 0 } else { 200 },
            },
            Carry {
                score,
                ..Carry::default()
            },
            [0; 9],
        )
    };
    let earlier = [
        old("LEVEL1", 1, 10),
        old("LEVEL1", 2, 20),
        old("LEVEL2", START_SLOT, 30),
        old("LEVEL2", 1, 40),
        old("LEVEL3", START_SLOT, 50),
    ];
    let mut r = Rig::new(Start::Title, |dir| {
        let mut store = SaveStore::open(dir).unwrap();
        for rec in &earlier {
            store.write(rec).unwrap();
        }
    });
    let config_before = fs::read_to_string(&r.config).unwrap();
    r.reach_main_menu();
    r.choose();
    r.choose();
    r.choose(); // a new game of level 1
    r.update(FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::Play);
    r.update(300.0);
    let carry = r.sim.as_ref().unwrap().carry();
    win_and_skip_the_tally(&mut r);
    assert_eq!(r.flow.level(), Some(&LevelChoice::Retail(2)));
    r.update(500.0);
    let mut store = SaveStore::open(&r.saves).unwrap();
    let list = store.list().unwrap();
    // Only the level 2 start changed: it now holds this game's progress.
    for rec in &earlier {
        let now = list
            .iter()
            .find(|x| x.level == rec.level && x.slot == rec.slot)
            .unwrap_or_else(|| panic!("{} {} is kept", rec.level, rec.slot));
        if rec.level == "LEVEL2" && rec.slot == START_SLOT {
            assert_eq!(now.carry, carry);
        } else {
            assert_eq!(now, rec, "{} {} is untouched", rec.level, rec.slot);
        }
    }
    assert_eq!(list.len(), earlier.len());
    // The replaced start is kept as the backup, in the key = value form.
    let backup = fs::read_to_string(r.saves.join("LEVEL2-3.save.bak")).unwrap();
    assert_eq!(SaveRecord::parse(&backup).unwrap(), earlier[2]);
    let text = fs::read_to_string(r.saves.join("LEVEL2-3.save")).unwrap();
    assert!(text.lines().any(|l| l == "level = LEVEL2"), "{text}");
    assert_eq!(fs::read_to_string(&r.config).unwrap(), config_before);
}

#[test]
fn death_game_over_then_the_main_menu() {
    let mut r = Rig::new(Start::Play, |_| {});
    assert_eq!(r.flow.screen(), Screen::Play);
    r.update(200.0);
    r.events(&[GameEvent::ClawDied { lives_left: 2 }]);
    assert_eq!(r.flow.screen(), Screen::Play);
    r.events(&[GameEvent::ClawDied { lives_left: 0 }, GameEvent::GameOver]);
    assert_eq!(r.flow.screen(), Screen::GameOver);
    assert!(r.paused, "the game stops under the fade");
    r.update(1000.0);
    let b = r.flow.game_over().unwrap().brightness();
    assert_eq!(b, 1.0);
    r.update(1100.0);
    assert!(r.flow.game_over().is_some_and(|g| g.brightness() < 1.0));
    r.reach_main_menu();
    assert_eq!(r.ended, 1);
    assert!(r.sim.is_none());
}

#[test]
fn save_points_write_saves_and_loading_one_starts_from_it() {
    let saved = SaveRecord::new(
        retail_level_name(1),
        SavePos {
            slot: 1,
            x: 2345,
            y: 678,
        },
        Carry {
            score: 1234,
            health: 77,
            lives: 4,
            pistol: 9,
            magic: 2,
            dynamite: 1,
            weapon: Weapon::Magic,
            treasure_game: [5, 1, 0, 0, 0, 0, 0, 0, 0],
        },
        [5, 1, 0, 0, 0, 0, 0, 0, 0],
    );
    let rec = saved.clone();
    let mut r = Rig::new(Start::Title, move |dir| {
        SaveStore::open(dir).unwrap().write(&rec).unwrap();
    });
    assert!(r.flow.progress().has(1, 1));
    r.reach_main_menu();
    r.choose(); // single player
    r.input(MenuInput::Down);
    r.choose(); // load game
    assert_eq!(r.page(), Page::LoadGameLevels);
    let nav = r.flow.main_menu().unwrap();
    let sp2 = nav
        .tree
        .find(MenuCommand::Load { level: 1, entry: 2 })
        .unwrap();
    assert!(
        !nav.tree.items[sp2].enabled,
        "save point 2 was never reached"
    );
    r.choose(); // level 1
    assert_eq!(r.page(), Page::LoadEntries { level: 1 });
    r.input(MenuInput::Down);
    r.choose(); // save point 1
    r.update(FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::Play);
    assert_eq!(r.started[0].0, LevelChoice::Retail(1));
    assert_eq!(r.started[0].1, saved.start_state());

    // Touching save point 2 saves at its checkpoint.
    r.update(100.0);
    r.events(&[
        GameEvent::Checkpoint { x: 4000, y: 900 },
        GameEvent::SavePoint { slot: 2 },
    ]);
    let mut store = SaveStore::open(&r.saves).unwrap();
    let rec = store.read("LEVEL1", 2).unwrap().expect("saved");
    assert_eq!((rec.x, rec.y), (4000, 900));
    assert_eq!(rec.carry, r.sim.as_ref().unwrap().carry());
    assert!(r.flow.progress().has(1, 2));
}

#[test]
fn custom_levels_are_never_saved() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    // Not reachable from the menu: start one directly.
    let dir = TempDir::new().unwrap();
    let mut settings = Settings::new(dir.path(), LevelChoice::Custom("X.WWD".into()));
    settings.config_path = None;
    let store = SaveStore::open(dir.path().join("s")).unwrap();
    let mut f = Flow::new(
        settings,
        Some(store),
        None,
        FontArt::uniform(8, 10),
        Start::Play,
    );
    let sim = Sim::new(level(), SimConfig::default());
    f.play_events(&[GameEvent::SavePoint { slot: 1 }], &sim);
    let mut store = SaveStore::open(dir.path().join("s")).unwrap();
    assert!(store.list().unwrap().is_empty());
    let _ = r;
}

#[test]
fn the_in_game_menu_pauses_resumes_and_ends_the_game() {
    let mut r = Rig::new(Start::Play, |_| {});
    r.update(100.0);
    assert!(r.flow.pause_key());
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Pause);
    assert!(r.paused);
    assert!(r.sounds.contains(&"GAME_SELECT".to_string()));
    // Escape resumes from any submenu.
    r.input(MenuInput::Back);
    assert_eq!(r.flow.screen(), Screen::Play);
    assert!(!r.paused);
    // End life asks first; no goes back.
    r.flow.pause_key();
    r.effects();
    r.input(MenuInput::Down);
    r.choose();
    let nav = r.flow.in_game_menu().unwrap();
    assert_eq!(nav.page(), Page::EndLifeConfirm);
    r.input(MenuInput::Down);
    r.choose();
    assert_eq!(r.flow.in_game_menu().unwrap().page(), Page::Root);
    assert_eq!(r.flow.screen(), Screen::Pause);
    assert!(!r.sounds.contains(&"GAME_DISABLE".to_string()));
    // End the game: down to it, confirm.
    for _ in 0..3 {
        r.input(MenuInput::Down);
    }
    r.choose();
    assert_eq!(r.flow.in_game_menu().unwrap().page(), Page::EndGameConfirm);
    r.choose();
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert_eq!(r.ended, 1);
}

/// The config file's lines, minus those of `keys`.
fn other_lines(text: &str, keys: &[&str]) -> Vec<String> {
    text.lines()
        .filter(|l| !keys.iter().any(|k| l.trim_start().starts_with(k)))
        .map(String::from)
        .collect()
}

#[test]
fn options_persist_into_the_config_keeping_other_lines() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.input(MenuInput::Down);
    r.choose(); // options
    assert_eq!(r.page(), Page::Options);
    r.choose(); // audio
    assert_eq!(r.page(), Page::Audio);
    r.input(MenuInput::Down);
    r.choose(); // voice off
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.contains("voice = off"), "{text}");
    assert_eq!(
        other_lines(&text, &["voice"]),
        other_lines(CONFIG, &["voice"]),
        "other lines are kept byte for byte"
    );
    assert!(!r.flow.settings().voice);
    // The sound slider steps by 8 and stops at its ends.
    r.input(MenuInput::Up);
    r.input(MenuInput::Left);
    assert_eq!(r.flow.settings().sound_volume, 72);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "sound_volume = 72"), "{text}");
    for _ in 0..20 {
        r.input(MenuInput::Left);
    }
    assert_eq!(r.flow.settings().sound_volume, 0);
    assert_eq!(
        other_lines(
            &fs::read_to_string(&r.config).unwrap(),
            &["voice", "sound_volume"]
        ),
        other_lines(CONFIG, &["voice"])
    );
    assert!(
        fs::read_to_string(flow_backup(&r.config)).is_ok(),
        "a backup is kept"
    );
}

fn flow_backup(config: &Path) -> PathBuf {
    claw_game::settings::backup_path(config)
}

#[test]
fn the_video_page_cycles_and_persists_settings() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.input(MenuInput::Down);
    r.choose(); // options
    r.input(MenuInput::Down);
    r.choose(); // display
    assert_eq!(r.page(), Page::Display);
    // The Video entry sits before the display page's back entry.
    let nav = r.flow.main_menu().unwrap();
    let entries: Vec<MenuCommand> = nav
        .entries()
        .iter()
        .map(|&k| nav.tree.items[k].command)
        .collect();
    assert_eq!(
        entries[entries.len() - 2..],
        [MenuCommand::Open(Page::Video), MenuCommand::Back]
    );
    for _ in 0..entries.len() - 2 {
        r.input(MenuInput::Down);
    }
    r.choose();
    assert_eq!(r.page(), Page::Video);
    r.input(MenuInput::Down);
    r.choose(); // aspect: fill -> centered
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.contains("aspect = centered"), "{text}");
    let effects_ok = r.flow.settings().aspect.name() == "centered";
    assert!(effects_ok);
    let nav = r.flow.main_menu().unwrap();
    let item = nav.selected().unwrap();
    assert_eq!(
        nav.tree.items[item].text.as_deref(),
        Some("ASPECT CENTERED")
    );
}

#[test]
fn bindings_capture_and_persist() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.input(MenuInput::Down);
    r.choose(); // options
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    r.choose(); // controls
    assert_eq!(r.page(), Page::Controls);
    let nav = r.flow.main_menu().unwrap();
    assert_eq!(nav.entries().len(), flow::CONTROL_ACTIONS.len() + 3);
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    r.input(MenuInput::Down); // jump
    r.choose();
    let (action, pad) = r.captures[0];
    assert!(!pad);
    assert_eq!(action.name(), Some("jump"));
    assert!(r.flow.capturing());
    let mut keymap = Keymap::default();
    let rebound = keymap
        .rebind(action, None, winit::keyboard::KeyCode::KeyJ)
        .unwrap();
    r.flow.captured(
        claw_game::input::CaptureOutcome::Bound {
            action,
            input: claw_game::input::Captured::Key(winit::keyboard::KeyCode::KeyJ),
            swapped: rebound.swapped.map(|(a, _)| (a, None)),
        },
        &keymap,
        &PadMap::default(),
    );
    assert!(!r.flow.capturing());
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(
        text.contains("key_jump = \"Space, KeyJ\"") || text.contains("key_jump = Space, KeyJ"),
        "{text}"
    );
    let row = control_row(&keymap, &PadMap::default(), "jump");
    assert_eq!(row, "JUMP   SPACE  J   SOUTH");
}

#[test]
fn the_pad_dead_zone_steps_applies_and_persists() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.input(MenuInput::Down);
    r.choose(); // options
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    r.choose(); // controls
    assert_eq!(r.page(), Page::Controls);
    // The dead zone row follows the bindings, before DEFAULTS and BACK.
    for _ in 0..flow::CONTROL_ACTIONS.len() {
        r.input(MenuInput::Down);
    }
    let nav = r.flow.main_menu().unwrap();
    let row = nav.selected().unwrap();
    assert_eq!(nav.tree.items[row].command, MenuCommand::PadDeadZone);
    assert_eq!(
        nav.tree.items[row].text.as_deref(),
        Some("STICK DEAD ZONE 34")
    );
    let _ = r.flow.take_effects();

    // Right raises it to the next step and applies it at once.
    r.flow.menu_input(MenuInput::Right, false);
    assert!(
        r.flow.take_effects().contains(&Effect::Bindings),
        "the new dead zone is applied"
    );
    assert_eq!(r.flow.settings().pad.dead_zone, 0.35);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "pad_dead_zone = 0.35"), "{text}");
    assert_eq!(
        other_lines(&text, &["pad_dead_zone"]),
        other_lines(CONFIG, &[]),
        "other lines are kept byte for byte"
    );
    let nav = r.flow.main_menu().unwrap();
    assert_eq!(
        nav.tree.items[row].text.as_deref(),
        Some("STICK DEAD ZONE 35")
    );

    // Left lowers it and stops at the lowest step.
    for _ in 0..30 {
        r.input(MenuInput::Left);
    }
    assert_eq!(r.flow.settings().pad.dead_zone, 0.05);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "pad_dead_zone = 0.05"), "{text}");
    assert_eq!(
        other_lines(&text, &["pad_dead_zone"]),
        other_lines(CONFIG, &[])
    );

    // Choosing it raises it one step.
    r.choose();
    assert_eq!(r.flow.settings().pad.dead_zone, 0.1);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "pad_dead_zone = 0.1"), "{text}");
}

#[test]
fn the_pad_dead_zone_steps_on_its_grid_and_range() {
    assert_eq!(flow::dead_zone_step(66, 1, false), 70);
    assert_eq!(flow::dead_zone_step(66, -1, false), 65);
    assert_eq!(flow::dead_zone_step(65, -1, false), 60);
    assert_eq!(flow::dead_zone_step(5, -1, false), 5);
    assert_eq!(flow::dead_zone_step(95, 1, false), 95);
    assert_eq!(flow::dead_zone_step(95, 1, true), 5);
    assert_eq!(flow::dead_zone_step(93, 1, true), 95);
    assert_eq!(flow::dead_zone_step(40, 0, true), 40);
}

#[test]
fn progress_follows_the_saves() {
    let mut p = Progress::default();
    assert!(p.can_start(1));
    assert!(!p.can_start(2));
    p.insert(2, START_SLOT);
    assert!(p.complete(1) && p.can_start(2) && !p.can_start(3));
    assert!(!p.can_start(15));
    assert_eq!(flow::level_number("LEVEL12"), Some(12));
    assert_eq!(flow::retail_number(&LevelChoice::Custom("A".into())), None);
}

#[test]
fn quitting_from_the_main_menu_fades_then_ends() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.input(MenuInput::Back); // highlights quit and chooses it
    r.update(PRESS_MS + 1.0);
    assert_eq!(r.page(), Page::QuitConfirm);
    r.choose();
    assert!(!r.quit);
    r.update(FADE_MS);
    assert!(r.quit);
    assert_eq!(r.flow.screen(), Screen::Exit);
}

#[test]
fn custom_levels_show_no_saved_message() {
    let saved = GameEvent::Message {
        id: MESSAGE_SAVED,
        ms: 5000,
    };
    let not_saved = GameEvent::Message {
        id: MESSAGE_NOT_SAVED,
        ms: 5000,
    };
    let events = vec![
        GameEvent::Checkpoint { x: 1, y: 2 },
        GameEvent::SavePoint { slot: 1 },
        saved,
        not_saved,
    ];
    for (level, shown) in [
        (LevelChoice::Retail(1), events.clone()),
        (
            LevelChoice::Custom("X.WWD".into()),
            vec![
                GameEvent::Checkpoint { x: 1, y: 2 },
                GameEvent::SavePoint { slot: 1 },
                not_saved,
            ],
        ),
    ] {
        let dir = TempDir::new().unwrap();
        let mut settings = Settings::new(dir.path(), level.clone());
        settings.config_path = None;
        let f = Flow::new(settings, None, None, FontArt::uniform(8, 10), Start::Play);
        let mut got = events.clone();
        f.hide_messages(&mut got);
        assert_eq!(got, shown, "{level:?}");
    }
}

#[test]
fn the_status_bar_placement_persists() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert!(!r.flow.status_bar_centred());
    r.input(MenuInput::Down);
    r.choose(); // options
    r.input(MenuInput::Down);
    r.choose(); // display
    let n = r.flow.main_menu().unwrap().entries().len();
    for _ in 0..n - 2 {
        r.input(MenuInput::Down);
    }
    r.choose(); // video
    assert_eq!(r.page(), Page::Video);
    for _ in 0..flow::VIDEO_SETTINGS.len() - 1 {
        r.input(MenuInput::Down);
    }
    let _ = r.flow.take_effects();
    r.choose(); // status bar: edges -> centred
    assert!(r.flow.status_bar_centred());
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "status_bar = centred"), "{text}");
    let nav = r.flow.main_menu().unwrap();
    let item = nav.selected().unwrap();
    assert_eq!(
        nav.tree.items[item].text.as_deref(),
        Some("STATUS BAR CENTRED")
    );
    // The next run starts with it.
    let reread = Settings::from_args(
        &<claw_game::settings::Args as clap::Parser>::try_parse_from([
            "rusty-claw",
            "--config",
            r.config.to_str().unwrap(),
            "--install",
            "/i",
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(reread.status_bar_centred);
    let f = Flow::new(reread, None, None, FontArt::uniform(8, 10), Start::Title);
    assert!(f.status_bar_centred());
}

#[test]
fn window_keys_show_on_the_video_page_without_saving() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    let before = fs::read_to_string(&r.config).unwrap();
    r.flow.view_changed(|s| s.fullscreen = true);
    assert!(r.flow.settings().fullscreen);
    let nav = r.flow.main_menu().unwrap();
    let full = nav
        .tree
        .find(MenuCommand::Video(
            claw_ui::menu::command::VideoSetting::Fullscreen,
        ))
        .unwrap();
    assert_eq!(nav.tree.items[full].text.as_deref(), Some("FULLSCREEN ON"));
    assert_eq!(fs::read_to_string(&r.config).unwrap(), before);
}

#[test]
fn a_startup_notice_shows() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.flow.notify("saved games are off: no folder");
    assert_eq!(r.flow.notice(), Some("saved games are off: no folder"));
}

/// A sound effect a tenth of a second long.
const HIT: &str = "HIT";
/// An ambient sound four seconds long, played as a loop.
const AMB: &str = "AMB";
/// The handle of the ambient loop.
const AMB_VOICE: u32 = 7;

/// The sounds of the synthetic level and of the menus.
fn sounds() -> SoundBank {
    let mut bank = SoundBank::new();
    bank.insert(HIT, Pcm::mono(22050, vec![500i16; 2205]).into());
    bank.insert(AMB, Pcm::mono(22050, vec![500i16; 22050 * 4]).into());
    bank
}

/// The flow with a level in play and the sound bridge, carrying out the
/// flow's effects as the window does: the level starts or ends, then every
/// effect goes through [`Pauses::follow`]; the screens' sounds replace the
/// level's.
struct Played {
    flow: Flow,
    session: Option<Session>,
    audio: AudioBridge,
    pauses: Pauses,
    /// Levels left, counted at the screens' sounds that follow.
    left: u32,
    _dir: TempDir,
}

impl Played {
    fn new() -> Played {
        let dir = TempDir::new().unwrap();
        let mut settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
        settings.config_path = None;
        let store = SaveStore::open(dir.path().join("saves")).unwrap();
        let flow = Flow::new(
            settings,
            Some(store),
            Some(main_menu()),
            FontArt::uniform(8, 10),
            Start::Play,
        );
        let mut audio = AudioBridge::silent(AudioSettings::default());
        audio.set_sounds(sounds());
        let mut p = Played {
            flow,
            session: None,
            audio,
            pauses: Pauses::default(),
            left: 0,
            _dir: dir,
        };
        p.effects();
        p
    }

    fn effects(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                match &e {
                    Effect::StartLevel { start, .. } => {
                        let sim = Sim::with_start(level(), SimConfig::default(), start.clone());
                        self.session = Some(Session::new(sim));
                        self.flow.set_in_game_menu(Some(in_game_menu()));
                    }
                    Effect::EndLevel => self.session = None,
                    Effect::EndLife => {
                        if let Some(s) = self.session.as_mut() {
                            s.end_life();
                        }
                    }
                    _ => {}
                }
                self.pauses.follow(
                    &e,
                    self.session.as_mut(),
                    Some(&mut self.audio),
                    Duration::ZERO,
                );
                if e == Effect::ScreenSounds {
                    // Between the level left and the screens' sounds, the
                    // level's sounds are over: nothing plays but an
                    // interface sound, and no loop is kept for later.
                    self.left += 1;
                    assert!(!self.audio.is_paused(), "level {} left", self.left);
                    assert_eq!(self.audio.live_loops(), 0, "level {} left", self.left);
                    assert!(
                        self.only_the_interface_plays(),
                        "level {} left: a game sound plays on",
                        self.left
                    );
                    self.audio.set_sounds(sounds());
                }
            }
        }
    }

    fn update(&mut self, ms: f64) {
        let mut left = ms;
        while left > 0.0 {
            let dt = left.min(50.0);
            left -= dt;
            if let Some(s) = self.session.as_mut() {
                s.frame(Duration::from_secs_f64(dt / 1000.0));
                let (cues, events) = s.drain_outputs();
                self.audio.apply(&cues, &ViewInfo::default());
                self.flow.play_events(&events, s.sim());
            }
            let sim = self.session.as_ref().map(Session::sim);
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
        self.update(PRESS_MS + 1.0);
    }

    fn pause_key(&mut self) {
        assert!(self.flow.pause_key());
        self.effects();
        assert_eq!(self.flow.screen(), Screen::Pause);
        assert!(self.audio.is_paused());
    }

    fn end_game(&mut self) {
        self.pause_key();
        for _ in 0..4 {
            self.input(MenuInput::Down);
        }
        self.choose(); // end game
        self.choose(); // yes
        assert_eq!(self.flow.screen(), Screen::MainMenu);
        assert!(self.session.is_none());
    }

    /// From the main menu, a new game at level 1.
    fn new_game(&mut self) {
        self.update(FADE_MS);
        self.choose(); // single player
        self.choose(); // new game
        self.choose(); // level 1
        self.update(FADE_MS + 50.0);
        assert_eq!(self.flow.screen(), Screen::Play);
        assert!(self.session.is_some());
    }

    /// Plays the hit as the simulation would, and waits for it to play to
    /// its end in the mixer.
    fn hit_plays(&mut self) -> bool {
        let before = self.audio.engine().stats().voices_started;
        let cue = AudioCue::Play(SoundCue {
            key: HIT.into(),
            volume: 100,
            channel: SoundChannel::Effect,
            ..SoundCue::default()
        });
        self.audio.apply(&[cue], &ViewInfo::default());
        self.wait(|s| s.voices_started > before && s.active_voices == 0)
    }

    /// Starts the level's ambient loop, as the simulation would, and
    /// waits for it to play.
    fn ambient_loop(&mut self) {
        let before = self.audio.engine().stats().voices_started;
        let cue = AudioCue::Play(SoundCue {
            key: AMB.into(),
            volume: 100,
            looped: true,
            channel: SoundChannel::Ambient,
            voice: Some(AMB_VOICE),
            ..SoundCue::default()
        });
        self.audio.apply(&[cue], &ViewInfo::default());
        assert!(self.wait(|s| s.voices_started > before && s.active_voices >= 1));
        assert_eq!(self.audio.live_loops(), 1);
    }

    /// Plays an interface sound and waits for it to end with nothing else
    /// left playing. The commands sent before it are carried out first, so
    /// a loop started again by them is still playing then.
    fn only_the_interface_plays(&mut self) -> bool {
        let before = self.audio.engine().stats().voices_started;
        self.audio.play_ui(HIT);
        self.wait(|s| s.voices_started > before && s.active_voices == 0)
    }

    /// Polls the sound engine until `done`, for at most 3 s.
    fn wait(&mut self, done: impl Fn(&AudioStats) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            self.audio.apply(&[], &ViewInfo::default());
            if done(&self.audio.engine().stats()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn sounds_run(&mut self) -> bool {
        !self.audio.is_paused() && !self.audio.engine().game_paused() && self.hit_plays()
    }
}

#[test]
fn every_way_out_of_the_in_game_menu_leaves_the_sounds_running() {
    let mut p = Played::new();
    assert_eq!(p.flow.screen(), Screen::Play);
    p.update(100.0);
    assert!(p.sounds_run(), "at the start");

    // Resume with Escape, with the resume entry and with the pause key.
    p.pause_key();
    p.input(MenuInput::Back);
    assert_eq!(p.flow.screen(), Screen::Play);
    assert!(p.sounds_run(), "after Escape");
    p.pause_key();
    p.choose();
    assert_eq!(p.flow.screen(), Screen::Play);
    assert!(p.sounds_run(), "after the resume entry");
    p.pause_key();
    assert!(p.flow.pause_key());
    p.effects();
    assert!(p.sounds_run(), "after the pause key");
    // End life, confirmed: play runs on and Claw dies.
    p.pause_key();
    p.input(MenuInput::Down);
    p.choose(); // end life
    p.choose(); // yes
    assert_eq!(p.flow.screen(), Screen::Play);
    assert!(p.sounds_run(), "after ending a life");
    p.update(200.0);
    assert_eq!(p.session.as_ref().unwrap().sim().stats().health, 0);

    // End the game, then start a new one: the new level's sounds play,
    // and the old level's ambient loop, stopped by the menu, does not come
    // back over the main menu (checked at the screens' sounds).
    p.ambient_loop();
    p.end_game();
    assert_eq!(p.left, 1);
    assert!(!p.audio.is_paused(), "the menus are not paused");
    p.new_game();
    p.update(100.0);
    assert!(p.sounds_run(), "after ending the game and starting again");

    // The same after a game over.
    p.ambient_loop();
    p.flow
        .play_events(&[GameEvent::GameOver], p.session.as_ref().unwrap().sim());
    p.effects();
    assert_eq!(p.flow.screen(), Screen::GameOver);
    assert!(p.audio.is_paused());
    let mut guard = 0;
    while p.flow.screen() != Screen::MainMenu {
        p.update(100.0);
        guard += 1;
        assert!(guard < 200, "stuck at {:?}", p.flow.screen());
    }
    assert_eq!(p.left, 2);
    p.new_game();
    assert!(p.sounds_run(), "after a game over");

    // A level won: its ambient loop, still playing, ends with it.
    p.ambient_loop();
    p.flow.play_events(
        &[GameEvent::LevelComplete { after_ms: 100 }],
        p.session.as_ref().unwrap().sim(),
    );
    let mut guard = 0;
    while p.flow.screen() != Screen::Booty {
        p.update(50.0);
        guard += 1;
        assert!(guard < 200, "stuck at {:?}", p.flow.screen());
    }
    assert_eq!(p.left, 3);
    assert!(p.session.is_none());
}

/// The commands of the entries the main menu shows.
fn main_entries(r: &Rig) -> Vec<MenuCommand> {
    let nav = r.flow.main_menu().unwrap();
    nav.entries()
        .iter()
        .map(|&k| nav.tree.items[k].command)
        .collect()
}

fn selected_text(nav: &claw_ui::menu::Navigator) -> Option<String> {
    nav.tree.items[nav.selected().unwrap()].text.clone()
}

#[test]
fn the_gameplay_page_switches_the_mid_air_hit_fix_and_persists_it() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert!(!r.flow.settings().mid_air_hit_fix, "off by default");
    r.input(MenuInput::Down);
    r.choose(); // options
    assert_eq!(r.page(), Page::Options);
    // The Gameplay entry sits before the options page's back entry.
    let entries = main_entries(&r);
    assert_eq!(
        entries[entries.len() - 2..],
        [MenuCommand::Open(Page::Gameplay), MenuCommand::Back]
    );
    for _ in 0..entries.len() - 2 {
        r.input(MenuInput::Down);
    }
    assert_eq!(
        selected_text(r.flow.main_menu().unwrap()).as_deref(),
        Some("GAMEPLAY")
    );
    r.choose();
    assert_eq!(r.page(), Page::Gameplay);
    assert_eq!(
        main_entries(&r),
        [
            MenuCommand::Gameplay(GameplaySetting::MidAirHitFix),
            MenuCommand::Gameplay(GameplaySetting::PauseInput),
            MenuCommand::Gameplay(GameplaySetting::FuseClock),
            MenuCommand::Gameplay(GameplaySetting::Invincible),
            MenuCommand::Gameplay(GameplaySetting::InfiniteLives),
            MenuCommand::Back
        ]
    );
    assert_eq!(
        selected_text(r.flow.main_menu().unwrap()).as_deref(),
        Some("MID AIR HIT JUMP FIX OFF")
    );
    r.choose(); // off -> on
    assert!(r.flow.settings().mid_air_hit_fix);
    assert_eq!(
        selected_text(r.flow.main_menu().unwrap()).as_deref(),
        Some("MID AIR HIT JUMP FIX ON")
    );
    assert!(
        r.flow
            .settings()
            .sim_config((640, 480))
            .tuning
            .behaviour
            .fix_double_jump,
        "new games play with it"
    );
    assert_eq!(r.flow.notice(), None, "no game in play to tell");
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "mid_air_hit_fix = on"), "{text}");
    // The next run starts with it.
    let args = |extra: &[&str]| {
        let mut v = vec![
            "rusty-claw",
            "--config",
            r.config.to_str().unwrap(),
            "--install",
            "/i",
        ];
        v.extend_from_slice(extra);
        <claw_game::settings::Args as clap::Parser>::try_parse_from(v).unwrap()
    };
    let reread = Settings::from_args(&args(&[])).unwrap();
    assert!(reread.mid_air_hit_fix);
    let f = Flow::new(reread, None, None, FontArt::uniform(8, 10), Start::Title);
    assert!(f.settings().mid_air_hit_fix);
    // The command line wins over the file.
    assert!(
        !Settings::from_args(&args(&["--mid-air-hit-fix", "off"]))
            .unwrap()
            .mid_air_hit_fix
    );
    // Choosing it again turns it off and saves that.
    r.choose();
    assert!(!r.flow.settings().mid_air_hit_fix);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "mid_air_hit_fix = off"), "{text}");
}

#[test]
fn the_gameplay_page_switches_the_pause_and_fuse_behaviour_and_persists_it() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert_eq!(r.flow.settings().pause_input, PauseInput::Original);
    assert_eq!(r.flow.settings().fuse_clock, FuseClock::Original);
    r.input(MenuInput::Down);
    r.choose(); // options
    let entries = main_entries(&r);
    for _ in 0..entries.len() - 2 {
        r.input(MenuInput::Down);
    }
    r.choose(); // gameplay
    assert_eq!(r.page(), Page::Gameplay);
    let text = |r: &Rig| selected_text(r.flow.main_menu().unwrap());
    // The pause input switch.
    r.input(MenuInput::Down);
    assert_eq!(text(&r).as_deref(), Some("PAUSE INPUT ORIGINAL"));
    r.choose();
    assert_eq!(r.flow.settings().pause_input, PauseInput::ReleaseAll);
    assert_eq!(text(&r).as_deref(), Some("PAUSE INPUT RELEASE ALL"));
    // The fuse clock switch.
    r.input(MenuInput::Down);
    assert_eq!(text(&r).as_deref(), Some("FUSE CLOCK ORIGINAL"));
    r.choose();
    assert_eq!(r.flow.settings().fuse_clock, FuseClock::Frozen);
    assert_eq!(text(&r).as_deref(), Some("FUSE CLOCK FROZEN"));
    assert_eq!(
        r.flow.settings().sim_config((640, 480)).fuse_clock,
        FuseClock::Frozen,
        "new games play with it"
    );
    let saved = fs::read_to_string(&r.config).unwrap();
    assert!(
        saved.lines().any(|l| l == "pause_input = release_all"),
        "{saved}"
    );
    assert!(saved.lines().any(|l| l == "fuse_clock = frozen"), "{saved}");
    // The next run starts with both.
    let args = <claw_game::settings::Args as clap::Parser>::try_parse_from([
        "rusty-claw",
        "--config",
        r.config.to_str().unwrap(),
        "--install",
        "/i",
    ])
    .unwrap();
    let reread = Settings::from_args(&args).unwrap();
    assert_eq!(reread.pause_input, PauseInput::ReleaseAll);
    assert_eq!(reread.fuse_clock, FuseClock::Frozen);
    // Choosing them again goes back to the original.
    r.choose();
    assert_eq!(r.flow.settings().fuse_clock, FuseClock::Original);
    r.input(MenuInput::Up);
    r.choose();
    assert_eq!(r.flow.settings().pause_input, PauseInput::Original);
    let saved = fs::read_to_string(&r.config).unwrap();
    assert!(
        saved.lines().any(|l| l == "pause_input = original"),
        "{saved}"
    );
    assert!(
        saved.lines().any(|l| l == "fuse_clock = original"),
        "{saved}"
    );
}

#[test]
fn the_gameplay_page_switches_the_play_test_aids_and_persists_them() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert_eq!(r.flow.settings().aids, PlayAids::NONE, "off by default");
    r.input(MenuInput::Down);
    r.choose(); // options
    let entries = main_entries(&r);
    for _ in 0..entries.len() - 2 {
        r.input(MenuInput::Down);
    }
    r.choose(); // gameplay
    assert_eq!(r.page(), Page::Gameplay);
    let text = |r: &Rig| selected_text(r.flow.main_menu().unwrap());
    for _ in 0..3 {
        r.input(MenuInput::Down);
    }
    assert_eq!(text(&r).as_deref(), Some("INVINCIBLE OFF"));
    r.choose();
    assert!(r.flow.settings().aids.invincible);
    assert_eq!(text(&r).as_deref(), Some("INVINCIBLE ON"));
    r.input(MenuInput::Down);
    assert_eq!(text(&r).as_deref(), Some("INFINITE LIVES OFF"));
    r.choose();
    assert_eq!(text(&r).as_deref(), Some("INFINITE LIVES ON"));
    let both = PlayAids {
        invincible: true,
        infinite_lives: true,
    };
    assert_eq!(r.flow.settings().aids, both);
    assert_eq!(
        r.flow.settings().sim_config((640, 480)).aids,
        both,
        "new games play with them"
    );
    let saved = fs::read_to_string(&r.config).unwrap();
    assert!(saved.lines().any(|l| l == "invincible = on"), "{saved}");
    assert!(saved.lines().any(|l| l == "infinite_lives = on"), "{saved}");
    let args = |config: &Path, extra: &[&str]| {
        let mut v = vec![
            "rusty-claw",
            "--config",
            config.to_str().unwrap(),
            "--install",
            "/i",
        ];
        v.extend_from_slice(extra);
        <claw_game::settings::Args as clap::Parser>::try_parse_from(v).unwrap()
    };
    assert_eq!(
        Settings::from_args(&args(&r.config, &[])).unwrap().aids,
        both
    );
    // Choosing them again turns them off and saves that.
    r.choose();
    r.input(MenuInput::Up);
    r.choose();
    assert_eq!(r.flow.settings().aids, PlayAids::NONE);
    let saved = fs::read_to_string(&r.config).unwrap();
    assert!(saved.lines().any(|l| l == "invincible = off"), "{saved}");
    assert!(
        saved.lines().any(|l| l == "infinite_lives = off"),
        "{saved}"
    );
    // A flag on the command line turns an aid on for that run only: the
    // page shows it, and a change of another option does not save it.
    let run = Settings::from_args(&args(&r.config, &["--invincible"])).unwrap();
    assert!(run.aids.invincible && !run.aids.infinite_lives);
    assert!(
        run.write_back_values(&["invincible"]).is_empty(),
        "not saved"
    );
    assert_eq!(
        claw_game::flow::gameplay_text(GameplaySetting::Invincible, &run),
        "INVINCIBLE ON"
    );
    let mut changed = run.clone();
    changed.fuse_clock = FuseClock::Frozen;
    assert_eq!(
        changed.write_back_values(&["invincible", "fuse_clock"]),
        [("fuse_clock".to_string(), "frozen".to_string())]
    );
    let f = Flow::new(run, None, None, FontArt::uniform(8, 10), Start::Title);
    assert!(f.settings().aids.invincible, "the run plays with it");
}

#[test]
fn the_in_game_options_switch_the_fix_for_the_next_level() {
    let mut r = Rig::new(Start::Play, |_| {});
    r.update(100.0);
    assert!(r.flow.pause_key());
    r.effects();
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    r.choose(); // options
    let nav = r.flow.in_game_menu().unwrap();
    assert_eq!(nav.page(), Page::Options);
    let entries: Vec<MenuCommand> = nav
        .entries()
        .iter()
        .map(|&k| nav.tree.items[k].command)
        .collect();
    assert_eq!(
        entries[entries.len() - 2..],
        [MenuCommand::Open(Page::Gameplay), MenuCommand::Back]
    );
    for _ in 0..entries.len() - 2 {
        r.input(MenuInput::Down);
    }
    r.choose();
    assert_eq!(r.flow.in_game_menu().unwrap().page(), Page::Gameplay);
    r.choose();
    assert!(r.flow.settings().mid_air_hit_fix);
    assert_eq!(
        selected_text(r.flow.in_game_menu().unwrap()).as_deref(),
        Some("MID AIR HIT JUMP FIX ON")
    );
    assert!(
        r.flow.notice().is_some_and(|n| n.contains("next level")),
        "{:?}",
        r.flow.notice()
    );
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "mid_air_hit_fix = on"), "{text}");
}

/// Runs `sim` for `ms` as the window would while play runs: ticks of 18 ms,
/// their events handed to the flow.
fn play_for(r: &mut Rig, ms: f64) {
    let ticks = (ms / 18.0).ceil() as usize;
    for _ in 0..ticks {
        let Some(sim) = r.sim.as_mut() else { return };
        if r.paused {
            r.flow.update(18.0, None, Some(sim));
        } else {
            let events = sim.step(Default::default()).events;
            let sim = r.sim.as_ref().unwrap();
            r.flow.play_events(&events, sim);
            r.flow.update(18.0, None, Some(sim));
        }
        r.effects();
    }
}

/// Opens the in-game menu and chooses end life, then yes; the effects of
/// the yes, in order.
fn end_life_from_the_menu(r: &mut Rig) -> Vec<Effect> {
    assert!(r.flow.pause_key());
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Pause);
    r.input(MenuInput::Down);
    r.choose();
    assert_eq!(r.flow.in_game_menu().unwrap().page(), Page::EndLifeConfirm);
    r.flow.menu_input(MenuInput::Confirm, false);
    let mut effects = r.flow.take_effects();
    for _ in 0..20 {
        r.flow.update(PRESS_MS / 10.0, None, r.sim.as_ref());
        effects.extend(r.flow.take_effects());
    }
    for e in &effects {
        match e {
            Effect::PauseGame => r.paused = true,
            Effect::ResumeGame => r.paused = false,
            Effect::EndLife => r.sim.as_mut().unwrap().end_life(),
            _ => {}
        }
    }
    effects
}

/// A game in play with `lives` lives.
fn rig_with_lives(lives: i32) -> Rig {
    let mut r = Rig::new(Start::Play, |_| {});
    let start = StartState {
        carry: Some(Carry {
            lives,
            ..Carry::default()
        }),
        save: None,
    };
    r.sim = Some(Sim::with_start(level(), SimConfig::default(), start));
    play_for(&mut r, 200.0);
    r
}

#[test]
fn end_life_closes_the_menu_and_claw_dies_at_the_next_tick() {
    let mut r = rig_with_lives(3);
    assert_eq!(r.flow.menu_state().lives, 3);
    let effects = end_life_from_the_menu(&mut r);
    let at = |e: &Effect| effects.iter().position(|x| x == e);
    let resume = at(&Effect::ResumeGame).expect("the menu closes");
    let end = at(&Effect::EndLife).expect("the life ends");
    assert!(resume < end, "back to play first: {effects:?}");
    assert_eq!(r.flow.screen(), Screen::Play);
    assert!(r.sim.as_ref().unwrap().end_life_pending());
    play_for(&mut r, 18.0);
    let sim = r.sim.as_ref().unwrap();
    assert_eq!(sim.stats().health, 0);
    assert_eq!(
        sim.player().unwrap().state,
        claw_sim::player::state::DEATH_FALL
    );
    // He falls, loses a life and comes back; the game goes on.
    play_for(&mut r, 6000.0);
    let sim = r.sim.as_ref().unwrap();
    assert_eq!(sim.stats().lives, 2);
    assert_eq!(sim.stats().health, 100);
    assert_eq!(r.flow.screen(), Screen::Play);
    assert_eq!(r.flow.menu_state().lives, 2);
}

/// The question the in-game menu shows on its page: (set, frame).
fn question(r: &Rig) -> Option<(String, i32)> {
    struct Sized;
    impl claw_ui::menu::draw::Art for Sized {
        fn info(&mut self, _set: &str, _frame: i32) -> Option<claw_render::FrameInfo> {
            Some(claw_render::FrameInfo {
                width: 40,
                height: 20,
                offset_x: 0,
                offset_y: 0,
            })
        }
        fn frames(&mut self, _set: &str) -> Vec<i32> {
            vec![1, 2, 3, 4, 5, 6]
        }
    }
    let nav = r.flow.in_game_menu().unwrap();
    claw_ui::menu::draw::picture(nav, &mut Sized, &r.flow.menu_state(), 0.0)
        .into_iter()
        .map(|m| m.sprite)
        .find(|s| s.key == "GAME_INGAMEMENU_MAIN_CLAWICIDE")
        .map(|s| (s.key.clone(), s.frame))
}

#[test]
fn end_life_on_the_last_life_asks_the_low_lives_question_and_ends_the_game() {
    // Two lives: the usual question.
    let mut r = rig_with_lives(2);
    assert!(r.flow.pause_key());
    r.effects();
    r.input(MenuInput::Down);
    r.choose();
    assert_eq!(question(&r).map(|q| q.1), Some(5));
    // The last life: the low-lives question, and the game ends.
    let mut r = rig_with_lives(1);
    assert!(r.flow.pause_key());
    r.effects();
    r.input(MenuInput::Down);
    r.choose();
    assert_eq!(
        question(&r),
        Some(("GAME_INGAMEMENU_MAIN_CLAWICIDE".to_string(), 6))
    );
    r.input(MenuInput::Back);
    r.input(MenuInput::Back);
    assert_eq!(r.flow.screen(), Screen::Play);
    r.paused = false;
    let effects = end_life_from_the_menu(&mut r);
    assert!(effects.contains(&Effect::EndLife));
    let mut guard = 0;
    while r.flow.screen() == Screen::Play {
        play_for(&mut r, 100.0);
        guard += 1;
        assert!(guard < 100, "no game over");
    }
    assert_eq!(r.flow.screen(), Screen::GameOver);
    assert_eq!(r.sim.as_ref().unwrap().stats().lives, 0);
    r.reach_main_menu();
    assert_eq!(r.ended, 1);
}

#[test]
fn the_front_layer_switch_persists_and_high_detail_stays_on() {
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    assert!(r.flow.settings().front_layer);
    assert!(r.flow.menu_state().switches.front_layer);
    r.input(MenuInput::Down);
    r.choose(); // options
    r.input(MenuInput::Down);
    r.choose(); // display
    assert_eq!(r.page(), Page::Display);
    // The display page's first entry is the front layer.
    r.flow.menu_input(MenuInput::Confirm, false);
    let mut effects = r.flow.take_effects();
    for _ in 0..20 {
        r.flow.update(PRESS_MS / 10.0, None, None);
        effects.extend(r.flow.take_effects());
    }
    assert!(effects.contains(&Effect::FrontLayer(false)), "{effects:?}");
    assert!(!r.flow.settings().front_layer);
    assert!(!r.flow.menu_state().switches.front_layer);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "front_layer = off"), "{text}");
    assert_eq!(
        other_lines(&text, &["front_layer"]),
        other_lines(CONFIG, &[]),
        "other lines are kept byte for byte"
    );
    // The next run starts with it.
    let reread = Settings::from_args(
        &<claw_game::settings::Args as clap::Parser>::try_parse_from([
            "rusty-claw",
            "--config",
            r.config.to_str().unwrap(),
            "--install",
            "/i",
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(!reread.front_layer);
    let f = Flow::new(reread, None, None, FontArt::uniform(8, 10), Start::Title);
    assert!(!f.menu_state().switches.front_layer);
    assert!(f.menu_state().switches.high_detail);
}

#[test]
fn high_detail_stays_on() {
    let tree = {
        let root = node(10000, vec![node(13005, vec![leaf(13502), leaf(13506)])]);
        MenuTree::from_ifc(root, &CommandCatalogue::new(MenuKind::Main))
    };
    let dir = TempDir::new().unwrap();
    let settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
    let mut f = Flow::new(
        settings,
        None,
        Some(tree),
        FontArt::uniform(8, 10),
        Start::Title,
    );
    while f.screen() != Screen::MainMenu {
        f.menu_input(MenuInput::Confirm, false);
        f.update(100.0, None, None);
        let _ = f.take_effects();
    }
    f.update(FADE_MS, None, None);
    f.menu_input(MenuInput::Confirm, false); // display
    f.update(PRESS_MS + 1.0, None, None);
    assert_eq!(f.main_menu().unwrap().page(), Page::Display);
    f.menu_input(MenuInput::Confirm, false); // detail
    f.update(PRESS_MS + 1.0, None, None);
    assert!(f.menu_state().switches.high_detail);
}

/// A synthetic level of the recorded game: a floor, Claw at x = 500, a
/// coin and a gold bar on the way right and, on level 1, the map piece
/// that ends the level further on.
fn game_level(number: u32) -> SimLevel {
    let mut rows = vec![".".repeat(80); 15];
    rows.push("#".repeat(80));
    rows.push("#".repeat(80));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (500, 880));
    let at = |logic: &str, set: &str, x: i32| {
        let mut o = ObjectSpec::at(logic, x, 930, 0);
        o.image_set = set.to_string();
        o.frame = 1;
        o.record[0] = x;
        o
    };
    l.objects = vec![
        at("TreasurePowerup", "GAME_TREASURE_COINS", 700),
        at(
            "TreasurePowerup",
            "GAME_TREASURE_GOLDBARS",
            900 + 100 * number as i32,
        ),
    ];
    if number == 1 {
        l.objects.push(at("SpecialPowerup", "GAME_MAPPIECE", 1300));
    }
    l
}

/// A game played through the flow on synthetic levels, recorded level by
/// level as the window records it.
struct RecordedGame {
    flow: Flow,
    session: Option<Session>,
    recording: GameRecording,
    /// Per level left: its trace, how it started and the progress at its
    /// end.
    levels: Vec<(Vec<TraceRow>, StartState, Carry)>,
    start: Option<StartState>,
    base: PathBuf,
    _dir: TempDir,
}

impl RecordedGame {
    fn new() -> RecordedGame {
        let dir = TempDir::new().unwrap();
        let mut settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
        settings.config_path = None;
        let store = SaveStore::open(dir.path().join("saves")).unwrap();
        let flow = Flow::new(
            settings,
            Some(store),
            None,
            FontArt::uniform(8, 10),
            Start::Play,
        );
        let base = dir.path().join("game.csv");
        let mut g = RecordedGame {
            flow,
            session: None,
            recording: GameRecording::new(&base),
            levels: Vec::new(),
            start: None,
            base,
            _dir: dir,
        };
        g.effects();
        g
    }

    /// Leaves the level in play: its recording is written.
    fn leave(&mut self) {
        if let Some(s) = self.session.take() {
            let (path, ticks) = self.recording.finish(&s).unwrap().unwrap();
            assert_eq!(ticks as u64, s.ticks(), "{}", path.display());
            let start = self.start.take().unwrap();
            self.levels
                .push((s.trace().to_vec(), start, s.sim().carry()));
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
                        let n = flow::retail_number(&level).unwrap();
                        let sim =
                            Sim::with_start(game_level(n), SimConfig::default(), start.clone());
                        let mut session = Session::new(sim);
                        session.record_trace();
                        self.recording.start(&mut session, Some(n), start.clone());
                        self.session = Some(session);
                        self.start = Some(start);
                        self.flow.set_in_game_menu(Some(in_game_menu()));
                    }
                    Effect::EndLevel => self.leave(),
                    Effect::ShowBooty(stats) => {
                        let art = BootyArt {
                            map: (None, Palette::default()),
                            tally: (None, Palette::default()),
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
        let at = s.stamp(Duration::from_millis(3));
        s.key(at, b, down);
    }

    /// The pause key's pause for `ms`, as the window records it.
    fn pause_for(&mut self, ms: f64) {
        let s = self.session.as_mut().expect("a level in play");
        s.open_span(Span::Pause);
        s.pause();
        self.run(ms);
        let s = self.session.as_mut().unwrap();
        s.resume();
        s.close_span();
    }
}

/// `rows` as CSV with the input source left out (live play says keyboard,
/// a replay script).
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
fn a_game_recorded_across_two_levels_replays_the_same_twice() {
    let mut g = RecordedGame::new();
    assert_eq!(g.flow.screen(), Screen::Play);
    // Level 1: walk right, jump, pause a moment, on to the map piece.
    g.run(200.0);
    g.key(Buttons::RIGHT, true);
    g.run(400.0);
    g.key(Buttons::JUMP, true);
    g.run(150.0);
    g.key(Buttons::JUMP, false);
    g.pause_for(300.0);
    let mut guard = 0;
    while g.flow.screen() == Screen::Play && !g.flow.completing() {
        g.run(18.0);
        guard += 1;
        assert!(guard < 1000, "level 1 never ended");
    }
    if g.session.is_some() {
        g.key(Buttons::RIGHT, false);
    }
    while g.flow.screen() != Screen::Booty {
        g.run(100.0);
    }
    for _ in 0..3 {
        g.flow.menu_input(MenuInput::Confirm, false);
        g.effects();
    }
    while g.flow.screen() != Screen::Play {
        g.run(100.0);
    }
    assert_eq!(g.flow.level(), Some(&LevelChoice::Retail(2)));
    // Level 2: walk right a while and jump; the game is left there.
    g.run(250.0);
    g.key(Buttons::RIGHT, true);
    g.run(700.0);
    g.key(Buttons::RIGHT, false);
    g.run(100.0);
    g.key(Buttons::JUMP, true);
    g.run(400.0);
    g.leave();
    assert_eq!(g.levels.len(), 2);

    // One file per level: the one asked for, then <stem>.level2.csv.
    let files = recorded_game_files(&g.base);
    assert_eq!(files, g.recording.written());
    assert_eq!(
        files,
        vec![g.base.clone(), g.base.with_file_name("game.level2.csv")]
    );
    let recs: Vec<LevelRecording> = files
        .iter()
        .map(|p| LevelRecording::read(p).unwrap())
        .collect();
    assert_eq!(recs[0].level(), Some(1));
    assert_eq!(recs[1].level(), Some(2));
    assert_eq!(recs[0].start, StartState::default(), "a new game");
    assert_eq!(recs[1].start, g.levels[1].1, "level 2's carried start");
    assert!(
        recs[0]
            .script
            .directives()
            .iter()
            .any(|d| matches!(d.kind, claw_sim::DirectiveKind::Pause { .. })),
        "the pause is recorded"
    );
    let text = fs::read_to_string(&files[1]).unwrap();
    assert!(text.lines().any(|l| l == "# level 2"), "{text}");
    for (i, f) in files.iter().enumerate() {
        let id = format!("# game_id = {}", g.recording.id());
        let part = format!("# game_part = {}", i + 1);
        let text = fs::read_to_string(f).unwrap();
        assert!(text.lines().any(|l| l == id), "{text}");
        assert!(text.lines().any(|l| l == part), "{text}");
    }
    assert!(text.lines().any(|l| l == "# seed 1"), "{text}");
    assert!(text.lines().any(|l| l == "# tuning default"), "{text}");
    assert!(text.lines().any(|l| l.starts_with("# start_score = ")));

    // Each level replays from its own start, the same twice and the same
    // as played; level 1 ends with the progress level 2 starts with.
    let replay = |rec: &LevelRecording, n: u32| {
        let sim = Sim::with_start(
            game_level(n),
            rec.sim_config(SimConfig::default()),
            rec.start.clone(),
        );
        let s = rec.replay(sim);
        assert_eq!(s.ticks(), rec.script.ticks.len() as u64);
        assert_eq!(s.ticks_past_script(), 0);
        (trace_csv(s.trace()), s.sim().carry())
    };
    for (i, rec) in recs.iter().enumerate() {
        let n = i as u32 + 1;
        let (first, end) = replay(rec, n);
        let (second, again) = replay(rec, n);
        assert_eq!(first, second, "level {n} replays the same twice");
        assert_eq!(end, again);
        assert_eq!(first, trace_csv(&g.levels[i].0), "level {n} as played");
        assert_eq!(end, g.levels[i].2, "level {n} ends as played");
    }
    let (level1, end1) = replay(&recs[0], 1);
    assert!(level1.contains("level_complete"), "level 1 is won");
    let start2 = recs[1].start.carry.clone().expect("progress carried");
    assert_eq!(
        Carry {
            lives: end1.lives,
            ..start2.clone()
        },
        end1,
        "level 2 starts with level 1's progress"
    );
    assert!((0..=1).contains(&(start2.lives - end1.lives)));
}

/// A key event a little after the frame under way began.
fn tap(s: &mut Session, b: Buttons, down: bool) {
    let at = s.stamp(Duration::from_millis(3));
    s.key(at, b, down);
}

/// Plays level `n` from `start`, put at `at` first when given, for a
/// second and a half of walking and jumping, recorded into `rec`; returns
/// the session (its level not yet finished) and its trace.
fn play_recorded(
    rec: &mut GameRecording,
    n: u32,
    start: StartState,
    at: Option<(i32, i32)>,
) -> (Session, String) {
    let sim = start_sim(game_level(n), SimConfig::default(), start.clone(), at);
    let mut s = Session::new(sim);
    s.record_trace();
    rec.start_from(&mut s, Some(n), start, at);
    let dt = Duration::from_secs_f64(1.0 / 60.0);
    for f in 0..90 {
        match f {
            10 => tap(&mut s, Buttons::RIGHT, true),
            40 => tap(&mut s, Buttons::JUMP, true),
            50 => tap(&mut s, Buttons::JUMP, false),
            80 => tap(&mut s, Buttons::RIGHT, false),
            _ => {}
        }
        s.frame(dt);
    }
    let trace = trace_csv(s.trace());
    (s, trace)
}

#[test]
fn a_start_point_is_recorded_and_replayed_from() {
    let dir = TempDir::new().unwrap();
    let base = dir.path().join("game.csv");
    // The window starts the level the command line names at its --start
    // point.
    let mut settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(2));
    settings.start = Some((1200, 900));
    let start = StartState::default();
    let at = start_override(&settings, true, &LevelChoice::Retail(2), &start);
    assert_eq!(at, Some((1200, 900)));
    let mut rec = GameRecording::new(&base);
    let (s, live) = play_recorded(&mut rec, 2, start, at);
    rec.finish(&s).unwrap().unwrap();
    let text = fs::read_to_string(&base).unwrap();
    assert!(
        text.lines().any(|l| l == "# start_at = 1200, 900"),
        "{text}"
    );

    let r = LevelRecording::read(&base).unwrap();
    assert_eq!(r.start_at, Some((1200, 900)));
    assert_eq!(r.level(), Some(2));
    let cfg = r.sim_config(SimConfig::default());
    let replay = || trace_csv(r.replay(r.start_sim(game_level(2), cfg.clone())).trace());
    assert_eq!(replay(), live, "replayed from the start point");
    assert_eq!(replay(), live, "the same twice");
    // From the level's own start it would be another game.
    let spawn = r.replay(Sim::with_start(game_level(2), cfg.clone(), r.start.clone()));
    assert_ne!(trace_csv(spawn.trace()), live);

    // A replay's settings take the level and the start point from it.
    let mut replaying = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
    replaying.follow_recording(&r).unwrap();
    assert_eq!(replaying.level, LevelChoice::Retail(2));
    assert_eq!(replaying.start, Some((1200, 900)));
    // A recording made without one plays from the level's own start.
    let mut plain = GameRecording::new(dir.path().join("plain.csv"));
    let (s, _) = play_recorded(&mut plain, 2, StartState::default(), None);
    let (path, _) = plain.finish(&s).unwrap().unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("start_at"), "{text}");
    assert_eq!(LevelRecording::read(&path).unwrap().start_at, None);
}

#[test]
fn recording_a_game_again_lists_only_its_files_and_removes_nothing() {
    let dir = TempDir::new().unwrap();
    let base = dir.path().join("game.csv");
    // A file of the user's own with a name a later level could take.
    let mine = dir.path().join("game.level4.csv");
    fs::write(&mine, "my own notes\n").unwrap();
    let carried = StartState {
        carry: Some(Carry {
            score: 500,
            lives: 4,
            ..Carry::default()
        }),
        save: None,
    };
    let record = |levels: &[u32]| -> Vec<PathBuf> {
        let mut rec = GameRecording::new(&base);
        for (i, &n) in levels.iter().enumerate() {
            let start = if i == 0 {
                StartState::default()
            } else {
                carried.clone()
            };
            let (s, _) = play_recorded(&mut rec, n, start, None);
            rec.finish(&s).unwrap().unwrap();
        }
        rec.written().to_vec()
    };
    let names = |files: &[PathBuf]| -> Vec<String> {
        files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };

    let first = record(&[1, 2, 3, 4]);
    assert_eq!(
        names(&first),
        [
            "game.csv",
            "game.level2.csv",
            "game.level3.csv",
            "game.level4-2.csv"
        ]
    );
    assert_eq!(recorded_game_files(&base), first);
    assert_eq!(fs::read_to_string(&mine).unwrap(), "my own notes\n");
    let earlier: Vec<String> = first[1..]
        .iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect();

    // A shorter game recorded to the same name: its own files only.
    let second = record(&[1, 2]);
    assert_eq!(names(&second), ["game.csv", "game.level2-2.csv"]);
    assert_eq!(recorded_game_files(&base), second);
    // Nothing written before is removed or changed, but the file asked for.
    for (p, text) in first[1..].iter().zip(&earlier) {
        assert_eq!(&fs::read_to_string(p).unwrap(), text, "{}", p.display());
    }
    assert_eq!(fs::read_to_string(&mine).unwrap(), "my own notes\n");
    let level2 = LevelRecording::read(&second[1]).unwrap();
    assert_eq!(level2.level(), Some(2));
    assert_eq!(level2.start, carried);
}

// Movies.

/// A shelf holding `movies`; `intro_seen` and `skip_logo` as given.
fn shelf_of(movies: impl IntoIterator<Item = i8>, intro_seen: bool, skip_logo: bool) -> MovieShelf {
    MovieShelf {
        available: movies
            .into_iter()
            .map(|i| MovieIndex::new(i).unwrap())
            .collect::<MovieSet>(),
        intro_seen,
        skip_logo,
    }
}

/// Every movie of the game.
fn every_movie() -> Vec<i8> {
    vec![-1, 0, 2, 4, 6, 8, 10, 12, 13, 14]
}

/// The movie playing, failing when none is.
fn playing(r: &Rig) -> MovieIndex {
    match r.flow.screen() {
        Screen::Movie(i) => {
            assert_eq!(r.flow.playing_movie(), Some(i));
            i
        }
        other => panic!("no movie plays: {other:?}"),
    }
}

/// The window says the movie ended (or was skipped).
fn movie_ends(r: &mut Rig) {
    r.flow.movie_finished();
    r.effects();
}

#[test]
fn each_level_with_a_movie_plays_it_before_its_map_screen() {
    for n in 1..=14u32 {
        let mut r = Rig::with_movies(
            Start::Play,
            LevelChoice::Retail(n),
            main_menu(),
            shelf_of(every_movie(), true, false),
        );
        assert!(r.movies.is_empty(), "level {n}: no movie at the start");
        r.update(300.0);
        r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
        let sounds = r.sounds.len();
        r.update(200.0);
        let expected = MovieIndex::after_level(n);
        assert_eq!(
            expected.is_some(),
            [2, 4, 6, 8, 10, 12, 13, 14].contains(&n),
            "level {n}"
        );
        assert_eq!(
            r.movies,
            expected.into_iter().collect::<Vec<_>>(),
            "level {n}"
        );
        assert_eq!(r.ended, 1, "level {n}: the level ended first");
        let Some(index) = expected else {
            assert_eq!(r.flow.screen(), Screen::Booty, "level {n}");
            assert!(!r.flow.cut_screen(), "level {n}");
            continue;
        };
        assert_eq!(playing(&r), index, "level {n}");
        assert!(!r.flow.cut_screen(), "level {n}: the movie, not the black");
        assert!(r.flow.booty().is_some(), "level {n}: the map screen waits");
        assert_eq!(r.music.last(), Some(&None), "level {n}: the music stopped");
        // The map screen's sounds wait for the movie, however long it is,
        // and the keys pressed during it stay the movie's.
        for _ in 0..3 {
            r.input(MenuInput::Confirm);
        }
        r.input(MenuInput::Back);
        r.update(60_000.0);
        assert_eq!(playing(&r), index, "level {n}");
        assert_eq!(
            r.sounds.len(),
            sounds,
            "level {n}: {:?}",
            &r.sounds[sounds..]
        );
        movie_ends(&mut r);
        assert_eq!(r.flow.screen(), Screen::Booty, "level {n}");
        assert!(
            r.flow.brightness() < 0.01,
            "level {n}: the map screen fades in"
        );
        assert!(
            r.sounds.len() > sounds,
            "level {n}: the map screen's sounds"
        );
        assert_eq!(
            r.flow.booty().unwrap().phase(),
            BootyPhase::Map,
            "level {n}: no key of the movie skipped the map screen"
        );
        // Then the tally as always: the next level, or the credits.
        for _ in 0..3 {
            r.input(MenuInput::Confirm);
        }
        let mut guard = 0;
        while r.flow.screen() == Screen::Booty {
            r.update(100.0);
            guard += 1;
            assert!(guard < 200, "level {n}: stuck at the tally");
        }
        if n < 14 {
            assert_eq!(
                r.flow.level(),
                Some(&LevelChoice::Retail(n + 1)),
                "level {n}"
            );
        } else {
            assert_eq!(r.flow.screen(), Screen::Credits);
        }
        assert_eq!(r.movies.len(), 1, "level {n}: one movie only");
    }
}

#[test]
fn the_last_level_plays_its_movie_in_place_of_the_black_hold_or_holds_black_without_it() {
    // Without the movie: the black hold, as without movies.
    let mut r = Rig::with_movies(
        Start::Play,
        LevelChoice::Retail(14),
        main_menu(),
        shelf_of([-1, 0, 2, 13], true, false),
    );
    finish_the_game(&mut r);
    assert!(r.movies.is_empty());
    assert_eq!(r.flow.screen(), Screen::Credits);
    // With it: the movie, then the map screen at once, the tally and the
    // credits.
    let mut r = Rig::with_movies(
        Start::Play,
        LevelChoice::Retail(14),
        main_menu(),
        shelf_of([14], true, false),
    );
    r.update(300.0);
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    assert_eq!(playing(&r), MovieIndex::new(14).unwrap());
    assert!(!r.flow.cut_screen());
    assert!(r.flow.next_level().is_none());
    r.update(flow::CUT_SCREEN_MS * 3.0);
    assert_eq!(playing(&r), MovieIndex::new(14).unwrap());
    let sounds = r.sounds.len();
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert!(!r.flow.cut_screen(), "no black hold after the movie");
    assert!(r.sounds.len() > sounds, "the map screen's sounds");
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    let mut guard = 0;
    while r.flow.screen() == Screen::Booty {
        r.update(50.0);
        guard += 1;
        assert!(guard < 100, "stuck at the tally");
    }
    assert_eq!(r.flow.screen(), Screen::Credits);
    assert_eq!(r.started.len(), 1);
    // A movie that cannot play leaves the map screen to follow at once.
    let mut r = Rig::with_movies(
        Start::Play,
        LevelChoice::Retail(14),
        main_menu(),
        shelf_of([14], true, false),
    );
    r.update(300.0);
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    r.flow.movie_missing();
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert!(!r.flow.cut_screen());
}

#[test]
fn community_custom_levels_and_runs_without_movies_play_none() {
    let mut r = Rig::with_movies(
        Start::Play,
        LevelChoice::Custom("X.WWD".into()),
        main_menu(),
        shelf_of(every_movie(), false, false),
    );
    r.update(300.0);
    r.events(&[GameEvent::LevelComplete { after_ms: 100 }]);
    r.update(200.0);
    assert_eq!(r.flow.screen(), Screen::Booty);
    assert!(r.movies.is_empty(), "no movie, at the start or after it");
    // Without a shelf the window never hands over, and no movie is ever
    // asked for: the start, the end of a level, the idle menu.
    let mut r = Rig::new(Start::Title, |_| {});
    r.reach_main_menu();
    r.update(IDLE_ROOT_MS + FADE_MS + 100.0);
    assert_eq!(r.flow.screen(), Screen::Title);
    let mut r = Rig::with_level(Start::Play, LevelChoice::Retail(2), |_| {});
    r.update(300.0);
    win_and_skip_the_tally(&mut r);
    assert!(r.movies.is_empty());
    assert!(r.flow.movies().is_none());
}

#[test]
fn a_first_run_plays_the_logo_then_the_intro_and_records_the_intro() {
    // skip_logo does not count before the intro was seen.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of(every_movie(), false, true),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
    assert!(r.flow.title().is_none(), "no title yet");
    r.update(10_000.0);
    assert_eq!(playing(&r), MovieIndex::LOGO);
    movie_ends(&mut r);
    assert_eq!(r.movies, [MovieIndex::LOGO, MovieIndex::INTRO]);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    assert!(r.flow.title().is_none(), "no title between the movies");
    assert_eq!(r.intro_records, 0);
    assert!(!r.flow.settings().intro_seen);
    // Skipped (or watched to its end): recorded, and the title follows.
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(r.flow.title().is_some());
    assert!(r.sounds.contains(&"TITLE".to_string()));
    assert_eq!(r.intro_records, 1);
    assert!(r.flow.settings().intro_seen);
    assert!(r.flow.movies().unwrap().intro_seen);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.starts_with(CONFIG), "{text}");
    assert!(text.lines().any(|l| l == "intro_seen = yes"), "{text}");
    // The title plays out as usual.
    r.reach_main_menu();
    assert_eq!(r.movies.len(), 2);
}

#[test]
fn later_runs_play_the_logo_only_unless_it_is_skipped() {
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of(every_movie(), true, false),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::Title);
    assert_eq!(r.movies, [MovieIndex::LOGO], "no intro");
    assert_eq!(r.intro_records, 0);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(!text.contains("intro_seen"), "{text}");
    // skip_logo: straight to the title.
    let r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of(every_movie(), true, true),
    );
    assert!(r.movies.is_empty());
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(r.flow.title().is_some());
    // A run straight in a level plays neither, first run or not.
    for seen in [false, true] {
        let r = Rig::with_movies(
            Start::Play,
            LevelChoice::Retail(3),
            main_menu(),
            shelf_of(every_movie(), seen, false),
        );
        assert!(r.movies.is_empty());
        assert_eq!(r.flow.screen(), Screen::Play);
    }
}

#[test]
fn skip_logo_waits_for_the_intro_only_when_the_intro_plays_on_this_run() {
    // A first run with the intro on the shelf: the logo plays anyway.
    let r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 0], false, true),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
    // A first run without the intro: skip_logo applies, straight to the
    // title, and the intro is not recorded.
    for movies in [vec![-1], vec![-1, 2, 4]] {
        let r = Rig::with_movies(
            Start::Title,
            LevelChoice::Retail(1),
            main_menu(),
            shelf_of(movies.clone(), false, true),
        );
        assert!(r.movies.is_empty(), "{movies:?}");
        assert_eq!(r.flow.screen(), Screen::Title, "{movies:?}");
        assert!(r.flow.title().is_some(), "{movies:?}");
        assert_eq!(r.intro_records, 0);
        assert!(!r.flow.settings().intro_seen);
    }
    // Without skip_logo the logo still plays when the intro is missing.
    let r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1], false, false),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
}

#[test]
fn an_intro_that_failed_is_not_recorded_as_seen() {
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 0], false, false),
    );
    movie_ends(&mut r);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    r.flow.movie_failed();
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(r.flow.title().is_some());
    assert_eq!(r.intro_records, 0);
    assert!(!r.flow.settings().intro_seen);
    assert!(!r.flow.movies().unwrap().intro_seen);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(!text.contains("intro_seen"), "{text}");
    // The next run plays the logo and the intro again.
    let r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 0], r.flow.settings().intro_seen, false),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
}

#[test]
fn a_missing_intro_is_not_recorded_as_seen() {
    let nothing_recorded = |r: &Rig| {
        assert_eq!(r.intro_records, 0);
        assert!(!r.flow.settings().intro_seen);
        assert!(!r.flow.movies().unwrap().intro_seen);
        let text = fs::read_to_string(&r.config).unwrap();
        assert!(!text.contains("intro_seen"), "{text}");
    };
    // Not on the shelf: the logo, then the title.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 2], false, false),
    );
    assert_eq!(r.movies, [MovieIndex::LOGO]);
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::Title);
    assert_eq!(r.movies, [MovieIndex::LOGO]);
    nothing_recorded(&r);
    // On the shelf but it could not play.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 0], false, false),
    );
    movie_ends(&mut r);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    r.flow.movie_missing();
    r.effects();
    assert_eq!(r.flow.screen(), Screen::Title);
    nothing_recorded(&r);
    // Only the intro, a first run: it plays alone.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([0], false, false),
    );
    assert_eq!(r.movies, [MovieIndex::INTRO]);
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::Title);
    assert_eq!(r.intro_records, 1);
}

#[test]
fn the_idle_menu_shows_the_intro_movie_and_the_title_in_turn() {
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of(every_movie(), true, true),
    );
    r.reach_main_menu();
    let stops = r.bed_stops;
    // First the movie, after the menu rolls up; the menu's loop stops.
    r.update(IDLE_ROOT_MS - 100.0);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    r.update(100.0);
    r.update(FADE_MS + 50.0);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    assert_eq!(r.movies, [MovieIndex::INTRO]);
    assert!(r.bed_stops > stops, "the menu's loop stops for the movie");
    assert_eq!(r.music.last(), Some(&None));
    // No idle time runs while it plays.
    r.update(IDLE_ROOT_MS * 2.0);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    assert_eq!(r.intro_records, 0, "seen already");
    // Then the main menu, with its loop again.
    let sounds = r.sounds.len();
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(r.sounds[sounds..].contains(&MENU_BED.to_string()));
    r.update(FADE_MS);
    // Next time the title, then the movie again.
    r.update(IDLE_ROOT_MS + FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::Title);
    assert_eq!(r.movies.len(), 1);
    r.reach_main_menu();
    r.update(IDLE_ROOT_MS + FADE_MS + 50.0);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    assert_eq!(r.movies.len(), 2);
    // Without the intro, the title every time.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([-1, 2], true, true),
    );
    for _ in 0..2 {
        r.reach_main_menu();
        r.update(IDLE_ROOT_MS + FADE_MS + 50.0);
        assert_eq!(r.flow.screen(), Screen::Title);
    }
    assert!(r.movies.is_empty());
}

#[test]
fn the_idle_time_stands_still_while_the_window_is_in_the_background() {
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of(every_movie(), true, true),
    );
    r.reach_main_menu();
    r.update(IDLE_ROOT_MS / 2.0);
    // In the background no idle time counts, however long: neither the
    // intro nor the title starts unseen.
    r.flow.set_background(true);
    r.update(IDLE_ROOT_MS * 5.0);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(r.movies.is_empty());
    // Back in front it counts on from where it was.
    r.flow.set_background(false);
    r.update(IDLE_ROOT_MS / 2.0 - 100.0);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(r.movies.is_empty());
    r.update(100.0);
    r.update(FADE_MS + 50.0);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    // Without movies the title waits for the window too.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu(),
        shelf_of([], true, true),
    );
    r.reach_main_menu();
    r.flow.set_background(true);
    r.update(IDLE_ROOT_MS * 2.0);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    r.flow.set_background(false);
    r.update(IDLE_ROOT_MS + FADE_MS + 50.0);
    assert_eq!(r.flow.screen(), Screen::Title);
}

/// The main menu with its Replay Movies page (the logo, the intro and the
/// eight later movies, then back) beside single player and quit.
fn main_menu_with_movies() -> MenuTree {
    let movies = (15001..=15011).map(leaf).collect();
    let root = node(
        10000,
        vec![
            node(10001, vec![node(11001, vec![leaf(11101)]), leaf(11006)]),
            node(10003, movies),
            node(10006, vec![leaf(14001), leaf(14002)]),
        ],
    );
    MenuTree::from_ifc(root, &CommandCatalogue::new(MenuKind::Main))
}

/// The sounds of `played` other than the menu's loop rising.
fn interface(played: &[String]) -> Vec<&str> {
    played
        .iter()
        .map(String::as_str)
        .filter(|s| *s != MENU_BED)
        .collect()
}

/// Whether the main menu's entry with command number `id` is enabled.
fn enabled(r: &Rig, id: u32) -> bool {
    let tree = &r.flow.main_menu().unwrap().tree;
    tree.items.iter().find(|i| i.id == id).unwrap().enabled
}

#[test]
fn the_replay_movies_entries_follow_the_movies_found() {
    assert_eq!(replay_movie_index(1), Some(MovieIndex::LOGO));
    assert_eq!(replay_movie_index(2), Some(MovieIndex::INTRO));
    let later: Vec<Option<MovieIndex>> = (3..=10).map(replay_movie_index).collect();
    let levels: Vec<Option<MovieIndex>> = [2, 4, 6, 8, 10, 12, 13, 14]
        .into_iter()
        .map(MovieIndex::after_level)
        .collect();
    assert_eq!(later, levels);
    assert_eq!(replay_movie_index(0), None);
    assert_eq!(replay_movie_index(11), None);
    // No shelf, or an empty one: the page is greyed out.
    for shelf in [None, Some(shelf_of([], true, true))] {
        let mut r = Rig::build_with(
            Start::Title,
            LevelChoice::Retail(1),
            main_menu_with_movies(),
            shelf,
            |_| {},
        );
        r.reach_main_menu();
        assert!(!enabled(&r, 10003));
        assert!((15001..=15010).all(|id| !enabled(&r, id)));
        assert!(enabled(&r, 15011), "back");
    }
    // Some movies: the page, and exactly their entries.
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu_with_movies(),
        shelf_of([-1, 4, 13], true, true),
    );
    r.reach_main_menu();
    assert!(enabled(&r, 10003));
    let on: Vec<u32> = (15001..=15010).filter(|&id| enabled(&r, id)).collect();
    assert_eq!(on, [15001, 15004, 15009]);
}

#[test]
fn a_replayed_movie_returns_to_its_page() {
    let mut r = Rig::with_movies(
        Start::Title,
        LevelChoice::Retail(1),
        main_menu_with_movies(),
        shelf_of([-1, 0, 4], true, true),
    );
    r.reach_main_menu();
    // Replay Movies, then the movie after level 4 (the fourth entry).
    r.flow.hotkey('R');
    r.update(PRESS_MS + 1.0);
    assert_eq!(r.page(), Page::Movies);
    for _ in 0..3 {
        r.input(MenuInput::Down);
    }
    let entry = r.flow.main_menu().unwrap().selected().unwrap();
    assert_eq!(r.flow.main_menu().unwrap().tree.items[entry].id, 15004);
    let sounds = r.sounds.len();
    r.choose();
    assert_eq!(interface(&r.sounds[sounds..]), ["SELECT"]);
    assert_eq!(r.movies, [MovieIndex::new(4).unwrap()]);
    assert_eq!(playing(&r), MovieIndex::new(4).unwrap());
    assert!(r.bed_stops > 0, "the menu's loop stops");
    // Keys and time during the movie change nothing after it: no page up,
    // no idle title.
    r.input(MenuInput::Back);
    r.input(MenuInput::Down);
    r.flow.hotkey('B');
    r.update(IDLE_ROOT_MS * 2.0);
    assert_eq!(playing(&r), MovieIndex::new(4).unwrap());
    movie_ends(&mut r);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert_eq!(r.page(), Page::Movies);
    assert_eq!(r.flow.main_menu().unwrap().selected(), Some(entry));
    assert!(r.sounds.contains(&MENU_BED.to_string()));
    r.update(FADE_MS);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    // A greyed-out entry plays the disabled sound and no movie; the logo's
    // entry plays the logo, by its hotkey.
    r.input(MenuInput::Down);
    r.input(MenuInput::Down);
    let grey = r.flow.main_menu().unwrap().selected().unwrap();
    assert_eq!(r.flow.main_menu().unwrap().tree.items[grey].id, 15006);
    let sounds = r.sounds.len();
    r.choose();
    assert_eq!(interface(&r.sounds[sounds..]), ["DISABLE"]);
    assert_eq!(r.movies.len(), 1);
    r.flow.hotkey('U');
    r.update(PRESS_MS + 1.0);
    assert_eq!(playing(&r), MovieIndex::LOGO);
    // The window could not play it: the page again.
    r.flow.movie_missing();
    r.effects();
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert_eq!(r.page(), Page::Movies);
    // The intro replayed records nothing more: it was seen.
    r.update(FADE_MS);
    r.flow.hotkey('I');
    r.update(PRESS_MS + 1.0);
    assert_eq!(playing(&r), MovieIndex::INTRO);
    movie_ends(&mut r);
    assert_eq!(r.intro_records, 0);
    assert_eq!(r.page(), Page::Movies);
}
