//! The game flow without a window: synthetic menus and levels, game events
//! as the simulation sends them, saves in a temporary folder and options
//! written to a temporary config file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use claw_assets::Palette;
use claw_assets::ifc::IfcNode;
use claw_frame::{AudioCue, GameEvent, Weapon};
use claw_game::flow::{
    self, Effect, FADE_MS, Flow, MENU_BED, Progress, START_SLOT, Screen, Start, control_row,
};
use claw_game::keymap::{Keymap, PadMap};
use claw_game::saves::{SaveRecord, SaveStore, retail_level_name};
use claw_game::settings::{LevelChoice, Settings};
use claw_sim::{Carry, SavePos, Sim, SimConfig, SimLevel, StartState, TileMap};
use claw_ui::menu::command::{CommandCatalogue, MenuCommand, MenuKind, Page};
use claw_ui::menu::draw::FontArt;
use claw_ui::menu::nav::PRESS_MS;
use claw_ui::menu::{MenuInput, MenuTree};
use claw_ui::screens::booty::{BootyArt, BootyScreen};
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
    ended: usize,
    paused: bool,
    quit: bool,
    _dir: TempDir,
    saves: PathBuf,
    config: PathBuf,
}

impl Rig {
    fn new(start: Start, prepare: impl FnOnce(&Path)) -> Rig {
        let dir = TempDir::new().unwrap();
        let saves = dir.path().join("saves");
        fs::create_dir_all(&saves).unwrap();
        prepare(&saves);
        let config = dir.path().join("config.toml");
        fs::write(&config, CONFIG).unwrap();
        let mut settings = Settings::new(dir.path().join("install"), LevelChoice::Retail(1));
        settings.config_path = Some(config.clone());
        let store = SaveStore::open(&saves).unwrap();
        let flow = Flow::new(
            settings,
            Some(store),
            Some(main_menu()),
            FontArt::uniform(8, 10),
            start,
        );
        let mut rig = Rig {
            flow,
            sim: None,
            started: Vec::new(),
            captures: Vec::new(),
            sounds: Vec::new(),
            ended: 0,
            paused: false,
            quit: false,
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
fn new_game_level_complete_tally_then_the_main_menu_with_level_1_complete() {
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
    assert!(r.flow.progress().complete(1));
    // The tally: skip to the tally, finish it, end the hold.
    let booty = r.flow.booty().expect("tally screens");
    assert_eq!(booty.rows().len(), 9);
    for _ in 0..3 {
        r.input(MenuInput::Confirm);
    }
    r.reach_main_menu();
    assert_eq!(r.page(), Page::Root);
    assert_eq!(r.started.len(), 1, "no next level: back to the main menu");
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
    // Lose a life is greyed out until the game supports it.
    r.flow.pause_key();
    r.effects();
    r.input(MenuInput::Down);
    r.choose();
    let nav = r.flow.in_game_menu().unwrap();
    assert_eq!(nav.page(), Page::Root, "a greyed-out entry opens nothing");
    assert!(r.sounds.contains(&"GAME_DISABLE".to_string()));
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
        Some("STICK DEAD ZONE 66")
    );
    let _ = r.flow.take_effects();

    // Right raises it to the next step and applies it at once.
    r.flow.menu_input(MenuInput::Right, false);
    assert!(
        r.flow.take_effects().contains(&Effect::Bindings),
        "the new dead zone is applied"
    );
    assert_eq!(r.flow.settings().pad.dead_zone, 0.7);
    let text = fs::read_to_string(&r.config).unwrap();
    assert!(text.lines().any(|l| l == "pad_dead_zone = 0.7"), "{text}");
    assert_eq!(
        other_lines(&text, &["pad_dead_zone"]),
        other_lines(CONFIG, &[]),
        "other lines are kept byte for byte"
    );
    let nav = r.flow.main_menu().unwrap();
    assert_eq!(
        nav.tree.items[row].text.as_deref(),
        Some("STICK DEAD ZONE 70")
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
