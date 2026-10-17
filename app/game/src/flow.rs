//! The game flow: title, main menu, options, loading, play, the in-game
//! menu, the end-of-level tally and the end of a game.
//!
//! [`Flow`] is a state machine with no window, GPU or audio device: the
//! window (or a test) feeds it menu inputs, the time that passed and the
//! game events of play, and carries out the [`Effect`]s it asks for
//! (loading a level, pausing, playing sounds, applying options).
//!
//! | From | Event | To |
//! | --- | --- | --- |
//! | start (plain `--install`) | | title |
//! | start (`--level`, `--custom`) | | play |
//! | title | its sound ends, or a key | main menu |
//! | main menu | idle 120 s (180 s in a submenu) | title |
//! | main menu | new game at level n | play, new-game values |
//! | main menu | load (level n, start or save point) | play from the save |
//! | main menu | quit, confirmed | the program ends |
//! | play | Escape, the pause key or pad Start | in-game menu |
//! | in-game menu | resume | play |
//! | in-game menu | end life, confirmed | play: Claw dies at the next tick |
//! | in-game menu | end the game, confirmed | main menu |
//! | play | level complete, after its delay | tally |
//! | tally | done | main menu, the level marked complete |
//! | play | game over | the frozen game fades, then the main menu |
//!
//! Saved games: touching a save point writes save point 1 or 2 of the
//! level; finishing a level writes the start of the next level (score,
//! health, lives, ammunition and the game's treasure carried over), which
//! is what marks the level complete and opens the next level in the new
//! game and load lists. Community custom levels are never saved, and their
//! save points show no game-saved message ([`Flow::hide_messages`]).
//!
//! End life asks for a confirmation; its question is the low-lives one
//! when Claw has fewer than 2 lives left, since that death ends the game.
//! Confirmed, the menu closes and Claw dies at the start of the next tick
//! ([`Effect::EndLife`]), losing a life as for any other death.
//!
//! Options changed in the menus apply at once and are written back to the
//! config file, keeping its other lines as they are. Settings that the
//! config file has no key for (the music switch, the play area size and
//! interlaced movies) last until the game is closed. The front layer
//! switch hides or shows the level's front plane (its tiles only; objects
//! still draw) and is saved as `front_layer`. High detail stays on: the
//! game always draws in high detail. The window's own keys for the view (fullscreen, window size,
//! aspect, scaling and smooth motion) change it for this run: the Video
//! page shows the change, and choosing an entry there saves it.
//!
//! The Gameplay page under Options holds the mid-air hit jump fix (off by
//! default, as in the original). A change there is saved at once and
//! applies to the next game or level started; the level in play keeps the
//! behaviour it started with.
//!
//! Screens fade out over [`FADE_MS`] before the next one, which fades in
//! over the same time (with the menu's roll-up and unroll sounds).

use std::collections::{BTreeSet, VecDeque};

use claw_frame::{AudioCue, GameEvent, SoundChannel, SoundCue};
use claw_sim::logic::checkpoint::MESSAGE_SAVED;
use claw_sim::{Sim, StartState};
use claw_ui::menu::command::{
    GameplaySetting, MenuCommand, MenuKind, Page, Slider, Toggle, VideoSetting,
};
use claw_ui::menu::draw::{self, FontArt, MenuState};
use claw_ui::menu::{MenuAction, MenuInput, MenuTree, Navigator};
use claw_ui::screens::{BootyScreen, BootyStats, GameOverScreen, TitleScreen};
use winit::keyboard::KeyCode;

use crate::input::{CaptureOutcome, Captured};
use crate::keymap::{Action, Keymap, PadMap, key_name};
use crate::saves::{SaveRecord, SaveStore, retail_level_name};
use crate::settings::{LevelChoice, PAD_DEAD_ZONE_RANGE, Scale, Settings, WINDOW_SIZES};

/// Length of a screen's fade out or in, ms.
pub const FADE_MS: f64 = 700.0;
/// Idle time at the top of the main menu before the title shows, ms.
pub const IDLE_ROOT_MS: f64 = 120_000.0;
/// Idle time in a submenu before the title shows, ms.
pub const IDLE_SUBMENU_MS: f64 = 180_000.0;
/// How long a notice shows, ms.
pub const NOTICE_MS: f64 = 6000.0;
/// The menu's background loop.
pub const MENU_BED: &str = "MENUBED";
/// Handle of the menu's background loop.
pub const MENU_BED_VOICE: u32 = 0x7F00_0020;
/// Volume the menu loop rises to.
pub const MENU_BED_VOLUME: f64 = 40.0;
/// Time the menu loop takes to rise, ms.
pub const MENU_BED_RISE_MS: f64 = 2000.0;
/// Time the menu loop takes to fall when play starts, ms.
pub const MENU_BED_FALL_MS: f64 = 1000.0;
/// Levels of the game.
pub const LEVELS: u32 = 14;
/// Save slot holding a level's start.
pub const START_SLOT: u8 = 3;
/// The frame-rate caps the Video page offers.
pub const FRAME_CAPS: [u32; 5] = [60, 120, 144, 240, 300];
/// The actions listed on the controls page, by their names.
pub const CONTROL_ACTIONS: [&str; 12] = [
    "left",
    "right",
    "up",
    "down",
    "jump",
    "attack",
    "projectile",
    "toggle",
    "lift",
    "pistol",
    "magic",
    "dynamite",
];
/// One step of the pad dead zone on the controls page, in hundredths.
pub const DEAD_ZONE_STEP: i32 = 5;
/// The Video page's settings in order.
pub const VIDEO_SETTINGS: [VideoSetting; 8] = [
    VideoSetting::Resolution,
    VideoSetting::Aspect,
    VideoSetting::Scale,
    VideoSetting::Fullscreen,
    VideoSetting::Vsync,
    VideoSetting::FrameCap,
    VideoSetting::Interpolation,
    VideoSetting::StatusBar,
];

/// The Gameplay page's settings in order.
pub const GAMEPLAY_SETTINGS: [GameplaySetting; 1] = [GameplaySetting::MidAirHitFix];

/// What shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The title.
    Title,
    /// The main menu.
    MainMenu,
    /// A level in play.
    Play,
    /// The in-game menu over the frozen level.
    Pause,
    /// The end-of-level screens.
    Booty,
    /// The frozen level fading out after the last life.
    GameOver,
    /// The program ends.
    Exit,
}

/// What the flow asks of the window.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Load `level` and play it from `start`.
    StartLevel {
        /// The level.
        level: LevelChoice,
        /// Carried progress and save point.
        start: StartState,
    },
    /// The level in play is left.
    EndLevel,
    /// Stop the game clock and the game's sounds.
    PauseGame,
    /// Start them again (and the level music from its start).
    ResumeGame,
    /// End Claw's life at the start of the next tick (the in-game menu's
    /// end life, confirmed); comes right after [`Effect::ResumeGame`].
    EndLife,
    /// Show (true) or hide the level's front plane.
    FrontLayer(bool),
    /// Play or change a sound (interface channel).
    Sound(AudioCue),
    /// Play the menu chord once.
    MenuChord,
    /// Switch to the sounds of the standalone screens.
    ScreenSounds,
    /// The audio settings changed: apply [`Flow::audio_settings`].
    AudioSettings,
    /// A Video setting changed: apply it from [`Flow::settings`].
    Video(VideoSetting),
    /// The key or pad bindings changed: apply them from
    /// [`Flow::settings`].
    Bindings,
    /// Bind the next key (or pad button) pressed to `action`.
    Capture {
        /// The action.
        action: Action,
        /// A pad button rather than a key.
        pad: bool,
    },
    /// Show the title: hand a [`TitleScreen`] to [`Flow::set_title`].
    ShowTitle,
    /// Show the end-of-level screens: hand a [`BootyScreen`] for these
    /// stats to [`Flow::set_booty`].
    ShowBooty(BootyStats),
    /// End the program.
    Quit,
}

/// What a fade-out leads to.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Screen(Screen),
    Start(LevelChoice, StartState),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fade {
    out: bool,
    elapsed: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct Play {
    level: LevelChoice,
    checkpoint: Option<(i32, i32)>,
    complete_in: Option<f64>,
}

/// The retail level number of `level`.
pub fn retail_number(level: &LevelChoice) -> Option<u32> {
    match level {
        LevelChoice::Retail(n) if (1..=LEVELS).contains(n) => Some(*n),
        _ => None,
    }
}

/// The level number of a retail level name (`LEVEL3` is 3).
pub fn level_number(name: &str) -> Option<u32> {
    name.strip_prefix("LEVEL")?.parse().ok()
}

/// Which saves exist: (level, slot) pairs of the retail levels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    saves: BTreeSet<(u32, u8)>,
}

impl Progress {
    /// The saves of `store`.
    pub fn from_store(store: &mut SaveStore) -> Progress {
        let mut p = Progress::default();
        if let Ok(list) = store.list() {
            for r in list {
                if let Some(n) = level_number(&r.level) {
                    p.saves.insert((n, r.slot));
                }
            }
        }
        p
    }

    /// Notes a save of `level`'s `slot`.
    pub fn insert(&mut self, level: u32, slot: u8) {
        self.saves.insert((level, slot));
    }

    /// True when `level`'s `slot` is saved.
    pub fn has(&self, level: u32, slot: u8) -> bool {
        self.saves.contains(&(level, slot))
    }

    /// True when `level` was completed: the next level's start is saved.
    pub fn complete(&self, level: u32) -> bool {
        self.has(level + 1, START_SLOT)
    }

    /// True when a game can start at `level`'s start.
    pub fn can_start(&self, level: u32) -> bool {
        level == 1 || ((2..=LEVELS).contains(&level) && self.complete(level - 1))
    }
}

/// A short, font-friendly name for a key.
pub fn key_label(key: KeyCode) -> String {
    let name = key_name(key).unwrap_or("?");
    let short = name
        .strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .or_else(|| name.strip_prefix("Arrow"))
        .unwrap_or(name);
    let short = match short {
        "ControlLeft" => "LEFT CTRL",
        "ControlRight" => "RIGHT CTRL",
        "ShiftLeft" => "LEFT SHIFT",
        "ShiftRight" => "RIGHT SHIFT",
        "AltLeft" => "LEFT ALT",
        "AltRight" => "RIGHT ALT",
        s => s,
    };
    short.to_ascii_uppercase()
}

/// The controls page's row for `action`: its name, keys and pad buttons.
pub fn control_row(keymap: &Keymap, pad: &PadMap, name: &str) -> String {
    let Some(action) = Action::from_name(name) else {
        return name.to_ascii_uppercase();
    };
    let keys: Vec<String> = keymap
        .keys(action)
        .into_iter()
        .take(2)
        .map(key_label)
        .collect();
    let pads: Vec<String> = pad
        .buttons_of(action)
        .into_iter()
        .take(1)
        .map(|b| b.name().to_ascii_uppercase())
        .collect();
    let or_none = |v: Vec<String>| {
        if v.is_empty() {
            "NONE".to_string()
        } else {
            v.join("  ")
        }
    };
    format!(
        "{}   {}   {}",
        name.to_ascii_uppercase(),
        or_none(keys),
        or_none(pads)
    )
}

/// The controls page's row for the stick dead zone.
pub fn dead_zone_text(settings: &Settings) -> String {
    format!(
        "STICK DEAD ZONE {}",
        (settings.pad.dead_zone * 100.0).round() as i32
    )
}

/// The dead zone (in hundredths) one step from `value` in the direction of
/// `delta`, on the grid of [`DEAD_ZONE_STEP`] and inside the allowed
/// range; with `wrap`, raising the highest gives the lowest.
pub fn dead_zone_step(value: i32, delta: i32, wrap: bool) -> i32 {
    let lo = (PAD_DEAD_ZONE_RANGE.start() * 100.0).round() as i32;
    let hi = (PAD_DEAD_ZONE_RANGE.end() * 100.0).round() as i32;
    let s = DEAD_ZONE_STEP;
    let next = match delta.signum() {
        1 => (value.div_euclid(s) + 1) * s,
        -1 => ((value + s - 1).div_euclid(s) - 1) * s,
        _ => return value,
    };
    if wrap && next > hi {
        lo
    } else {
        next.clamp(lo, hi)
    }
}

/// The Video page's text of `v` for `settings`.
pub fn video_text(v: VideoSetting, settings: &Settings) -> String {
    let on = |b: bool| if b { "ON" } else { "OFF" };
    match v {
        VideoSetting::Resolution => format!("WINDOW {}X{}", settings.res.0, settings.res.1),
        VideoSetting::Aspect => format!("ASPECT {}", settings.aspect.name().to_ascii_uppercase()),
        VideoSetting::Scale => format!(
            "SCALE {}",
            match settings.scale {
                Scale::Fit => "FIT",
                Scale::Integer => "WHOLE",
            }
        ),
        VideoSetting::Fullscreen => format!("FULLSCREEN {}", on(settings.fullscreen)),
        VideoSetting::Vsync => format!("VSYNC {}", on(settings.vsync)),
        VideoSetting::FrameCap => match settings.fps_cap {
            Some(n) => format!("FRAME CAP {n}"),
            None => "FRAME CAP NONE".to_string(),
        },
        VideoSetting::Interpolation => format!("SMOOTH MOTION {}", on(settings.interpolation)),
        VideoSetting::StatusBar => {
            let place = if settings.status_bar_centred {
                "CENTRED"
            } else {
                "EDGES"
            };
            format!("STATUS BAR {place}")
        }
    }
}

/// The Gameplay page's text of `g` for `settings`. The menu font has no
/// hyphen, so "mid-air" is written as two words.
pub fn gameplay_text(g: GameplaySetting, settings: &Settings) -> String {
    let on = |b: bool| if b { "ON" } else { "OFF" };
    match g {
        GameplaySetting::MidAirHitFix => {
            format!("MID AIR HIT JUMP FIX {}", on(settings.mid_air_hit_fix))
        }
    }
}

/// The game flow.
pub struct Flow {
    settings: Settings,
    saves: Option<SaveStore>,
    progress: Progress,
    screen: Screen,
    main: Option<Navigator>,
    in_game: Option<Navigator>,
    font: FontArt,
    title: Option<TitleScreen>,
    booty: Option<BootyScreen>,
    game_over: Option<GameOverScreen>,
    state: MenuState,
    music_available: bool,
    play: Option<Play>,
    fade: Option<Fade>,
    pending: Option<Pending>,
    idle_ms: f64,
    intro_played: bool,
    bed: Option<(f64, f64, f64, f64)>,
    bed_volume: u8,
    notices: VecDeque<(String, f64)>,
    capturing: Option<usize>,
    elapsed: f64,
    told_unsaved: bool,
    effects: Vec<Effect>,
}

impl std::fmt::Debug for Flow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flow")
            .field("screen", &self.screen)
            .field("play", &self.play)
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}

/// How a run starts.
#[derive(Debug, Clone, PartialEq)]
pub enum Start {
    /// At the title.
    Title,
    /// Straight into `settings.level`.
    Play,
}

impl Flow {
    /// A flow over `settings` with the saved games of `saves`, the main
    /// menu `main` (from the game's menu file) and the bitmap font's sizes.
    pub fn new(
        settings: Settings,
        saves: Option<SaveStore>,
        main: Option<MenuTree>,
        font: FontArt,
        start: Start,
    ) -> Flow {
        let mut saves = saves;
        let progress = saves.as_mut().map(Progress::from_store).unwrap_or_default();
        let mut state = MenuState::default();
        state.switches.sound = settings.audio;
        state.switches.voice = settings.voice;
        state.switches.ambient = settings.ambient;
        state.switches.front_layer = settings.front_layer;
        state.sound_volume = i32::from(settings.sound_volume);
        state.music_volume = i32::from(settings.music_volume);
        let mut flow = Flow {
            settings,
            saves,
            progress,
            screen: Screen::Title,
            main: None,
            in_game: None,
            font,
            title: None,
            booty: None,
            game_over: None,
            state,
            music_available: true,
            play: None,
            fade: None,
            pending: None,
            idle_ms: 0.0,
            intro_played: false,
            bed: None,
            bed_volume: 0,
            notices: VecDeque::new(),
            capturing: None,
            elapsed: 0.0,
            told_unsaved: false,
            effects: Vec::new(),
        };
        if let Some(tree) = main {
            flow.main = Some(flow.prepare(tree));
        }
        let reports: Vec<String> = flow
            .saves
            .as_mut()
            .map(SaveStore::take_reports)
            .unwrap_or_default();
        for r in reports {
            flow.push_notice(r);
        }
        match start {
            Start::Title => {
                flow.screen = Screen::Title;
                flow.effects.push(Effect::ShowTitle);
                flow.fade = Some(Fade {
                    out: false,
                    elapsed: 0.0,
                });
            }
            Start::Play => {
                let level = flow.settings.level.clone();
                flow.begin_play(level, StartState::default());
            }
        }
        flow
    }

    /// Adds this game's own pages to a menu: the controls list, the Video
    /// page under Display and the Gameplay page under Options. The added
    /// entries are text; the layout gives each a row of its own beside the
    /// art entries, in large letters (see `claw_ui::menu::layout`).
    fn prepare(&mut self, mut tree: MenuTree) -> Navigator {
        // Puts the last entry of `page` before the page's own back entry.
        let before_back = |tree: &mut MenuTree, page: usize| {
            let kids = tree.items[page].children.clone();
            let n = kids.len();
            if n >= 2 && tree.items[kids[n - 2]].command == MenuCommand::Back {
                tree.items[page].children.swap(n - 1, n - 2);
            }
        };
        if let Some(display) = tree.find_page(Page::Display) {
            let video = tree.add_text(display, "VIDEO", MenuCommand::Open(Page::Video));
            for v in VIDEO_SETTINGS {
                tree.add_text(video, "", MenuCommand::Video(v));
            }
            tree.add_text(video, "BACK", MenuCommand::Back);
            before_back(&mut tree, display);
        }
        if let Some(options) = tree.find_page(Page::Options) {
            let gameplay = tree.add_text(options, "GAMEPLAY", MenuCommand::Open(Page::Gameplay));
            for g in GAMEPLAY_SETTINGS {
                tree.add_text(gameplay, "", MenuCommand::Gameplay(g));
            }
            tree.add_text(gameplay, "BACK", MenuCommand::Back);
            before_back(&mut tree, options);
        }
        if let Some(ctl) = tree.find(MenuCommand::Controls) {
            for i in 0..CONTROL_ACTIONS.len() {
                tree.add_text(ctl, "", MenuCommand::Bind(i as u8));
            }
            tree.add_text(ctl, "", MenuCommand::PadDeadZone);
            tree.add_text(ctl, "DEFAULTS", MenuCommand::DefaultBindings);
            tree.add_text(ctl, "BACK", MenuCommand::Back);
        }
        let mut nav = Navigator::new(tree);
        self.refresh_texts_of(&mut nav);
        nav
    }

    fn refresh_texts_of(&mut self, nav: &mut Navigator) {
        for i in 0..nav.tree.items.len() {
            let text = match nav.tree.items[i].command {
                MenuCommand::Video(v) => Some(video_text(v, &self.settings)),
                MenuCommand::Gameplay(g) => Some(gameplay_text(g, &self.settings)),
                MenuCommand::PadDeadZone => Some(dead_zone_text(&self.settings)),
                MenuCommand::Bind(n) => {
                    let n = usize::from(n);
                    Some(if self.capturing == Some(n) {
                        "PRESS A KEY OR BUTTON".to_string()
                    } else {
                        control_row(
                            &self.settings.keymap,
                            &self.settings.pad.map,
                            CONTROL_ACTIONS.get(n).copied().unwrap_or("?"),
                        )
                    })
                }
                _ => None,
            };
            if let Some(t) = text {
                nav.tree.items[i].text = Some(t);
            }
        }
        draw::remeasure_text(nav, &mut self.font);
    }

    fn refresh_texts(&mut self) {
        if let Some(mut n) = self.main.take() {
            self.refresh_texts_of(&mut n);
            self.main = Some(n);
        }
        if let Some(mut n) = self.in_game.take() {
            self.refresh_texts_of(&mut n);
            self.in_game = Some(n);
        }
    }

    /// Sets the in-game menu (built from the level's art when a level
    /// loads; sizes measured by the caller).
    pub fn set_in_game_menu(&mut self, tree: Option<MenuTree>) {
        self.in_game = tree.map(|t| self.prepare(t));
    }

    /// Re-measures the menus' images and text with `art` (call after the
    /// menus are set and their art is loaded).
    pub fn measure(&mut self, kind: MenuKind, art: &mut dyn draw::Art) {
        let nav = match kind {
            MenuKind::Main => self.main.as_mut(),
            _ => self.in_game.as_mut(),
        };
        if let Some(n) = nav {
            draw::measure(n, art);
        }
    }

    /// Tells the flow whether music can play (a SoundFont was found).
    pub fn set_music_available(&mut self, on: bool) {
        self.music_available = on;
        self.refresh_enabled();
    }

    /// The settings, as changed in the menus.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The audio settings in force (the music switch included).
    pub fn audio_settings(&self) -> claw_audio::AudioSettings {
        let mut a = self.settings.audio_settings();
        a.music = self.state.switches.music;
        a
    }

    /// What shows.
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// Which saves exist.
    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    /// The main menu.
    pub fn main_menu(&self) -> Option<&Navigator> {
        self.main.as_ref()
    }

    /// The in-game menu.
    pub fn in_game_menu(&self) -> Option<&Navigator> {
        self.in_game.as_ref()
    }

    /// The title.
    pub fn title(&self) -> Option<&TitleScreen> {
        self.title.as_ref()
    }

    /// The end-of-level screens.
    pub fn booty(&self) -> Option<&BootyScreen> {
        self.booty.as_ref()
    }

    /// The game-over fade.
    pub fn game_over(&self) -> Option<&GameOverScreen> {
        self.game_over.as_ref()
    }

    /// What the menus show besides their entries.
    pub fn menu_state(&self) -> MenuState {
        self.state
    }

    /// True when the status bar keeps to the centred 4:3 area.
    pub fn status_bar_centred(&self) -> bool {
        self.settings.status_bar_centred
    }

    /// Time since the flow started, ms (for menu animations).
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed
    }

    /// The notice on screen, if any.
    pub fn notice(&self) -> Option<&str> {
        self.notices.front().map(|(s, _)| s.as_str())
    }

    /// Brightness of the screen that shows, 0 to 1, for its fade.
    pub fn brightness(&self) -> f64 {
        match self.fade {
            Some(f) => {
                let t = (f.elapsed / FADE_MS).clamp(0.0, 1.0);
                if f.out { 1.0 - t } else { t }
            }
            None => 1.0,
        }
    }

    /// The level in play, if any.
    pub fn level(&self) -> Option<&LevelChoice> {
        self.play.as_ref().map(|p| &p.level)
    }

    /// True while a key or pad button is awaited for a binding.
    pub fn capturing(&self) -> bool {
        self.capturing.is_some()
    }

    /// Takes the effects asked for so far.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// Hands over the title asked for by [`Effect::ShowTitle`].
    pub fn set_title(&mut self, mut title: TitleScreen) {
        for a in title.take_audio() {
            self.effects.push(Effect::Sound(a));
        }
        self.title = Some(title);
    }

    /// Hands over the screens asked for by [`Effect::ShowBooty`].
    pub fn set_booty(&mut self, booty: BootyScreen) {
        self.booty = Some(booty);
        self.drain_booty_audio();
    }

    /// Shows `text` as a notice, for example a problem found at start-up.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.push_notice(text.into());
    }

    /// The window changed a view setting with one of its own keys: shows
    /// the change on the Video page without saving it (choosing the entry
    /// there saves it).
    pub fn view_changed(&mut self, update: impl FnOnce(&mut Settings)) {
        update(&mut self.settings);
        self.refresh_texts();
    }

    /// Removes from `events` the in-game messages that do not apply: the
    /// game-saved message of a save point on a community custom level,
    /// which is never saved.
    pub fn hide_messages(&self, events: &mut Vec<GameEvent>) {
        let custom = self
            .play
            .as_ref()
            .is_some_and(|p| retail_number(&p.level).is_none());
        if custom {
            events.retain(|e| !matches!(e, GameEvent::Message { id, .. } if *id == MESSAGE_SAVED));
        }
    }

    fn push_notice(&mut self, text: String) {
        eprintln!("rusty-claw: {text}");
        if self.notices.len() < 8 {
            self.notices.push_back((text, NOTICE_MS));
        }
    }

    fn sound(&mut self, key: &str) {
        self.effects.push(Effect::Sound(AudioCue::Play(SoundCue {
            key: key.to_string(),
            channel: SoundChannel::Interface,
            ..SoundCue::default()
        })));
    }

    fn menu_sound(&mut self, kind: MenuKind, what: &str) {
        let key = match kind {
            MenuKind::Main => what.to_string(),
            _ => format!("GAME_{what}"),
        };
        self.sound(&key);
    }

    fn fade_to(&mut self, pending: Pending) {
        if self.pending.is_some() {
            return;
        }
        if matches!(pending, Pending::Start(..)) && self.bed_volume > 0 {
            self.bed_to(0.0, MENU_BED_FALL_MS);
        }
        self.pending = Some(pending);
        self.fade = Some(Fade {
            out: true,
            elapsed: 0.0,
        });
        if matches!(self.screen, Screen::MainMenu | Screen::Title) {
            self.sound("ROLLUP");
        }
    }

    fn fade_in(&mut self) {
        self.fade = Some(Fade {
            out: false,
            elapsed: 0.0,
        });
    }

    fn bed_to(&mut self, to: f64, ms: f64) {
        self.bed = Some((f64::from(self.bed_volume), to, 0.0, ms));
    }

    fn update_bed(&mut self, dt: f64) {
        let Some((from, to, t, len)) = self.bed else {
            return;
        };
        let t = t + dt;
        let v = if t >= len {
            self.bed = None;
            to
        } else {
            self.bed = Some((from, to, t, len));
            from + (to - from) * t / len
        };
        self.set_bed(v);
    }

    fn set_bed(&mut self, v: f64) {
        let v = v.round().clamp(0.0, 100.0) as u8;
        if v == self.bed_volume {
            return;
        }
        self.bed_volume = v;
        let cue = if v == 0 {
            AudioCue::Stop {
                voice: MENU_BED_VOICE,
            }
        } else {
            AudioCue::Play(SoundCue {
                key: MENU_BED.to_string(),
                volume: v,
                looped: true,
                channel: SoundChannel::Interface,
                voice: Some(MENU_BED_VOICE),
                ..SoundCue::default()
            })
        };
        self.effects.push(Effect::Sound(cue));
    }

    fn enter(&mut self, screen: Screen) {
        let from = self.screen;
        self.screen = screen;
        self.idle_ms = 0.0;
        match screen {
            Screen::MainMenu => {
                if matches!(from, Screen::Play | Screen::Pause | Screen::GameOver) {
                    self.end_play();
                }
                if from == Screen::Title
                    && let Some(mut t) = self.title.take()
                {
                    let stop = t.leave();
                    self.effects.push(Effect::Sound(stop));
                }
                self.booty = None;
                self.game_over = None;
                if let Some(n) = self.main.as_mut() {
                    n.reset();
                }
                self.refresh_enabled();
                self.sound("UNROLL");
                if !self.intro_played {
                    self.intro_played = true;
                    self.sound("INTRO");
                }
                self.bed_volume = 0;
                self.set_bed(1.0);
                self.bed_to(MENU_BED_VOLUME, MENU_BED_RISE_MS);
                self.fade_in();
            }
            Screen::Title => {
                self.effects.push(Effect::ShowTitle);
                self.fade_in();
            }
            _ => {}
        }
    }

    fn end_play(&mut self) {
        if self.play.take().is_some() {
            self.effects.push(Effect::EndLevel);
            self.effects.push(Effect::ScreenSounds);
        }
        self.in_game = None;
    }

    fn begin_play(&mut self, level: LevelChoice, start: StartState) {
        self.screen = Screen::Play;
        self.fade = None;
        self.title = None;
        self.play = Some(Play {
            level: level.clone(),
            checkpoint: None,
            complete_in: None,
        });
        if self.bed_volume > 0 {
            self.effects.push(Effect::Sound(AudioCue::Stop {
                voice: MENU_BED_VOICE,
            }));
            self.bed_volume = 0;
            self.bed = None;
        }
        self.effects.push(Effect::StartLevel { level, start });
    }

    /// A level failed to load: back to the main menu with a notice.
    pub fn level_failed(&mut self, why: &str) {
        self.push_notice(format!("the level could not be loaded: {why}"));
        self.play = None;
        self.enter(Screen::MainMenu);
    }

    fn refresh_enabled(&mut self) {
        let progress = self.progress.clone();
        let sound = self.state.switches.sound;
        let music = self.music_available;
        let rule = |item: &claw_ui::menu::MenuItem| match item.command {
            MenuCommand::NewGame { level } => progress.can_start(level),
            MenuCommand::Open(Page::LoadEntries { level }) => progress.can_start(level),
            MenuCommand::Load { level, entry: 0 } => progress.can_start(level),
            MenuCommand::Load { level, entry } => progress.has(level, entry),
            MenuCommand::Unavailable(_) => false,
            MenuCommand::Toggle(Toggle::Voice | Toggle::Ambient) => sound,
            MenuCommand::Toggle(Toggle::Music) => music,
            _ => true,
        };
        if let Some(n) = self.main.as_mut() {
            n.tree.set_enabled_by(rule);
        }
        if let Some(n) = self.in_game.as_mut() {
            n.tree.set_enabled_by(rule);
        }
    }

    /// A new press of a menu input; `from_pad` when it came from a gamepad
    /// (a binding then waits for a pad button).
    pub fn menu_input(&mut self, input: MenuInput, from_pad: bool) {
        self.idle_ms = 0.0;
        if self.pending.is_some() || self.capturing.is_some() {
            return;
        }
        if self.fade.is_some_and(|f| f.out) {
            return;
        }
        match self.screen {
            Screen::Title => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back)
                    && let Some(t) = self.title.as_mut()
                {
                    t.skip();
                }
            }
            Screen::MainMenu => {
                let Some(n) = self.main.as_mut() else { return };
                let out = n.input(input);
                self.menu_actions(MenuKind::Main, out, from_pad);
            }
            Screen::Pause => {
                let Some(n) = self.in_game.as_mut() else {
                    if input == MenuInput::Back {
                        self.resume();
                    }
                    return;
                };
                let out = n.input(input);
                self.menu_actions(MenuKind::InGame, out, from_pad);
            }
            Screen::Booty => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back)
                    && let Some(b) = self.booty.as_mut()
                {
                    b.skip();
                    self.drain_booty_audio();
                }
            }
            Screen::GameOver => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back)
                    && let Some(g) = self.game_over.as_mut()
                {
                    g.skip();
                }
            }
            Screen::Play | Screen::Exit => {}
        }
    }

    /// A letter or digit key in a menu.
    pub fn hotkey(&mut self, letter: char) {
        self.idle_ms = 0.0;
        if self.pending.is_some() || self.capturing.is_some() {
            return;
        }
        let (kind, nav) = match self.screen {
            Screen::MainMenu => (MenuKind::Main, self.main.as_mut()),
            Screen::Pause => (MenuKind::InGame, self.in_game.as_mut()),
            _ => return,
        };
        let Some(n) = nav else { return };
        let out = n.hotkey(letter);
        self.menu_actions(kind, out, false);
    }

    /// Escape or the pause key during a game: in play it opens the in-game
    /// menu, in the in-game menu it resumes. Returns true when the flow
    /// took the key.
    pub fn pause_key(&mut self) -> bool {
        match self.screen {
            Screen::Play => {
                if self.play.as_ref().is_some_and(|p| p.complete_in.is_some()) {
                    return true;
                }
                self.screen = Screen::Pause;
                self.effects.push(Effect::PauseGame);
                if let Some(n) = self.in_game.as_mut() {
                    n.reset();
                }
                self.refresh_enabled();
                self.sound("GAME_SELECT");
                true
            }
            Screen::Pause => {
                self.sound("GAME_SELECT");
                self.resume();
                true
            }
            Screen::GameOver => true,
            _ => false,
        }
    }

    fn resume(&mut self) {
        if self.screen == Screen::Pause {
            self.screen = Screen::Play;
            self.effects.push(Effect::ResumeGame);
        }
    }

    fn menu_actions(&mut self, kind: MenuKind, actions: Vec<MenuAction>, from_pad: bool) {
        for a in actions {
            self.menu_action(kind, a, from_pad);
        }
    }

    fn menu_action(&mut self, kind: MenuKind, action: MenuAction, from_pad: bool) {
        match action {
            MenuAction::Moved => self.menu_sound(kind, "CLICK"),
            MenuAction::Back => self.menu_sound(kind, "SELECT"),
            MenuAction::Disabled(_) => self.menu_sound(kind, "DISABLE"),
            MenuAction::Slider { slider, delta } => self.slide(kind, slider, delta),
            MenuAction::Step {
                command: MenuCommand::PadDeadZone,
                delta,
            } => {
                if self.dead_zone(delta, false) {
                    self.menu_sound(kind, "CLICK");
                }
            }
            MenuAction::Step { .. } => {}
            MenuAction::Command(c) => {
                if c != MenuCommand::Resume {
                    self.menu_sound(kind, "SELECT");
                }
                self.command(c, from_pad);
            }
        }
    }

    fn slide(&mut self, kind: MenuKind, slider: Slider, delta: i32) {
        let before = self.state.value(slider);
        let after = slider.adjust(before, delta);
        if after == before {
            return;
        }
        self.state.set_value(slider, after);
        self.menu_sound(kind, "CLICK");
        match slider {
            Slider::SoundVolume => {
                self.settings.sound_volume = after.clamp(0, 100) as u8;
                self.effects.push(Effect::AudioSettings);
                self.persist(&["sound_volume"]);
            }
            Slider::MusicVolume => {
                self.settings.music_volume = after.clamp(0, 100) as u8;
                self.effects.push(Effect::AudioSettings);
                self.persist(&["music_volume"]);
                if kind == MenuKind::Main {
                    self.effects.push(Effect::MenuChord);
                }
            }
            Slider::PlayArea => {}
        }
    }

    fn persist<K: AsRef<str>>(&mut self, keys: &[K]) {
        let keys: Vec<&str> = keys.iter().map(AsRef::as_ref).collect();
        for k in &keys {
            self.settings.changed_in_game(k);
        }
        if self.settings.config_path.is_none() {
            if !self.told_unsaved {
                self.told_unsaved = true;
                self.push_notice(
                    "options are not saved: no config file location (set HOME)".into(),
                );
            }
            return;
        }
        if let Err(e) = self.settings.write_back(&keys) {
            self.push_notice(format!("options not saved: {e:#}"));
        }
    }

    fn command(&mut self, c: MenuCommand, from_pad: bool) {
        match c {
            MenuCommand::NewGame { level } => {
                self.fade_to(Pending::Start(
                    LevelChoice::Retail(level),
                    StartState::default(),
                ));
            }
            MenuCommand::Load { level, entry } => self.load(level, entry),
            MenuCommand::Quit => self.fade_to(Pending::Quit),
            MenuCommand::Resume => {
                self.menu_sound(MenuKind::InGame, "SELECT");
                self.resume();
            }
            MenuCommand::EndLife => {
                if self.screen == Screen::Pause {
                    self.resume();
                    self.effects.push(Effect::EndLife);
                }
            }
            MenuCommand::EndGame => {
                self.enter(Screen::MainMenu);
            }
            MenuCommand::Toggle(t) => self.toggle(t),
            MenuCommand::Video(v) => self.video(v),
            MenuCommand::Gameplay(g) => self.gameplay(g),
            MenuCommand::Bind(i) => {
                let i = usize::from(i);
                if let Some(action) = CONTROL_ACTIONS.get(i).and_then(|n| Action::from_name(n)) {
                    self.capturing = Some(i);
                    self.effects.push(Effect::Capture {
                        action,
                        pad: from_pad,
                    });
                    self.refresh_texts();
                }
            }
            MenuCommand::DefaultBindings => {
                self.settings.keymap = Keymap::default();
                self.settings.pad.map = PadMap::default();
                self.effects.push(Effect::Bindings);
                let keys = binding_keys(&CONTROL_ACTIONS);
                self.persist(&keys);
                self.refresh_texts();
            }
            MenuCommand::PadDeadZone => {
                self.dead_zone(1, true);
            }
            MenuCommand::Open(_)
            | MenuCommand::Back
            | MenuCommand::Controls
            | MenuCommand::Adjust(_)
            | MenuCommand::Unavailable(_)
            | MenuCommand::Nothing => {}
        }
    }

    /// Steps the pad dead zone (see [`dead_zone_step`]), applies and
    /// saves it; false when it did not change.
    fn dead_zone(&mut self, delta: i32, wrap: bool) -> bool {
        let before = (self.settings.pad.dead_zone * 100.0).round() as i32;
        let after = dead_zone_step(before, delta, wrap);
        if after == before {
            return false;
        }
        self.settings.pad.dead_zone = after as f32 / 100.0;
        self.effects.push(Effect::Bindings);
        self.persist(&["pad_dead_zone"]);
        self.refresh_texts();
        true
    }

    fn load(&mut self, level: u32, entry: u8) {
        let slot = match entry {
            0 => START_SLOT,
            n => n,
        };
        let record = match self.saves.as_mut() {
            Some(store) => {
                let r = store.read(&retail_level_name(level), slot);
                let reports = store.take_reports();
                for m in reports {
                    self.push_notice(m);
                }
                match r {
                    Ok(r) => r,
                    Err(e) => {
                        self.push_notice(format!("cannot read the saved game: {e:#}"));
                        None
                    }
                }
            }
            None => None,
        };
        let start = match record {
            Some(r) => r.start_state(),
            None if entry == 0 && level == 1 => StartState::default(),
            None => {
                self.push_notice("that saved game is gone".into());
                self.progress = self
                    .saves
                    .as_mut()
                    .map(Progress::from_store)
                    .unwrap_or_default();
                self.refresh_enabled();
                return;
            }
        };
        self.fade_to(Pending::Start(LevelChoice::Retail(level), start));
    }

    fn toggle(&mut self, t: Toggle) {
        if t == Toggle::HighDetail {
            // The game always draws in high detail.
            return;
        }
        let on = !self.state.switches.get(t);
        self.state.switches.set(t, on);
        match t {
            Toggle::Sound => {
                self.settings.audio = on;
                self.effects.push(Effect::AudioSettings);
                self.persist(&["audio"]);
                self.refresh_enabled();
            }
            Toggle::Voice => {
                self.settings.voice = on;
                self.effects.push(Effect::AudioSettings);
                self.persist(&["voice"]);
                if on {
                    self.sound(if self.screen == Screen::Pause {
                        "GAME_1055"
                    } else {
                        "1055"
                    });
                }
            }
            Toggle::Ambient => {
                self.settings.ambient = on;
                self.effects.push(Effect::AudioSettings);
                self.persist(&["ambient"]);
            }
            Toggle::Music => self.effects.push(Effect::AudioSettings),
            Toggle::FrontLayer => {
                self.settings.front_layer = on;
                self.effects.push(Effect::FrontLayer(on));
                self.persist(&["front_layer"]);
            }
            Toggle::HighDetail | Toggle::MovieInterlace => {}
        }
    }

    fn video(&mut self, v: VideoSetting) {
        let s = &mut self.settings;
        let key: Option<&'static str> = match v {
            VideoSetting::Resolution => {
                let i = WINDOW_SIZES.iter().position(|&w| w == s.res);
                s.res = WINDOW_SIZES[i.map_or(0, |i| (i + 1) % WINDOW_SIZES.len())];
                Some("res")
            }
            VideoSetting::Aspect => {
                s.aspect = s.aspect.next();
                Some("aspect")
            }
            VideoSetting::Scale => {
                s.scale = s.scale.toggled();
                Some("scale")
            }
            VideoSetting::Fullscreen => {
                s.fullscreen = !s.fullscreen;
                Some("fullscreen")
            }
            VideoSetting::Vsync => {
                s.vsync = !s.vsync;
                Some("vsync")
            }
            VideoSetting::FrameCap => {
                let i = s
                    .fps_cap
                    .and_then(|c| FRAME_CAPS.iter().position(|&f| f == c));
                s.fps_cap = Some(FRAME_CAPS[i.map_or(0, |i| (i + 1) % FRAME_CAPS.len())]);
                Some("fps_cap")
            }
            VideoSetting::Interpolation => {
                s.interpolation = !s.interpolation;
                Some("interpolation")
            }
            VideoSetting::StatusBar => {
                s.status_bar_centred = !s.status_bar_centred;
                Some("status_bar")
            }
        };
        self.effects.push(Effect::Video(v));
        if let Some(k) = key {
            self.persist(&[k]);
        }
        self.refresh_texts();
    }

    fn gameplay(&mut self, g: GameplaySetting) {
        let key = match g {
            GameplaySetting::MidAirHitFix => {
                self.settings.mid_air_hit_fix = !self.settings.mid_air_hit_fix;
                "mid_air_hit_fix"
            }
        };
        self.persist(&[key]);
        if self.play.is_some() {
            self.push_notice("gameplay options apply from the next level started".into());
        }
        self.refresh_texts();
    }

    /// How a binding started by [`Effect::Capture`] ended; `keymap` and
    /// `pad` are the router's maps after it.
    pub fn captured(&mut self, outcome: CaptureOutcome, keymap: &Keymap, pad: &PadMap) {
        self.capturing = None;
        match outcome {
            CaptureOutcome::Bound {
                action, swapped, ..
            } => {
                self.settings.keymap = keymap.clone();
                self.settings.pad.map = pad.clone();
                let mut names = vec![action.name().unwrap_or("")];
                if let Some((other, _)) = swapped {
                    names.push(other.name().unwrap_or(""));
                }
                names.retain(|n| !n.is_empty());
                let keys = binding_keys(&names);
                self.persist(&keys);
                self.effects.push(Effect::Bindings);
            }
            CaptureOutcome::Refused {
                conflict, input, ..
            } => {
                let what = match input {
                    Captured::Key(k) => key_label(k),
                    Captured::Pad(b) => b.name().to_ascii_uppercase(),
                };
                self.push_notice(format!("{what} cannot be bound: {conflict}"));
            }
            CaptureOutcome::Cancelled => {}
        }
        self.refresh_texts();
    }

    /// Game events of the ticks just run, with the simulation after them.
    pub fn play_events(&mut self, events: &[GameEvent], sim: &Sim) {
        for e in events {
            match *e {
                GameEvent::Checkpoint { x, y } => {
                    if let Some(p) = self.play.as_mut() {
                        p.checkpoint = Some((x, y));
                    }
                }
                GameEvent::SavePoint { slot } => self.save_point(slot, sim),
                GameEvent::LevelComplete { after_ms } => {
                    if let Some(p) = self.play.as_mut()
                        && p.complete_in.is_none()
                    {
                        p.complete_in = Some(f64::from(after_ms));
                    }
                }
                GameEvent::GameOver => {
                    if matches!(self.screen, Screen::Play | Screen::Pause) {
                        self.screen = Screen::GameOver;
                        self.game_over = Some(GameOverScreen::new());
                        self.effects.push(Effect::PauseGame);
                    }
                }
                GameEvent::ClawDied { .. } | GameEvent::Message { .. } => {}
            }
        }
    }

    fn save_point(&mut self, slot: u8, sim: &Sim) {
        let Some(play) = self.play.as_ref() else {
            return;
        };
        let Some(n) = retail_number(&play.level) else {
            return;
        };
        // The save point's own checkpoint comes in the same tick, just
        // before it.
        let pos = play
            .checkpoint
            .or(sim.claw().checkpoint)
            .or_else(|| sim.player().map(|o| (o.x, o.y)))
            .unwrap_or((0, 0));
        let record = SaveRecord::new(
            retail_level_name(n),
            claw_sim::SavePos {
                slot,
                x: pos.0,
                y: pos.1,
            },
            sim.carry(),
            sim.stats().treasure_level,
        );
        self.write(&record, n);
    }

    fn write(&mut self, record: &SaveRecord, level: u32) {
        let Some(store) = self.saves.as_mut() else {
            self.push_notice("the game was not saved: there is no folder for saved games".into());
            return;
        };
        let r = store.write(record);
        let reports = store.take_reports();
        for m in reports {
            self.push_notice(m);
        }
        match r {
            Ok(()) => self.progress.insert(level, record.slot),
            Err(e) => self.push_notice(format!("the game was not saved: {e:#}")),
        }
    }

    fn complete_level(&mut self, sim: Option<&Sim>) {
        let Some(play) = self.play.clone() else {
            return;
        };
        let number = retail_number(&play.level);
        let custom = matches!(play.level, LevelChoice::Custom(_));
        let stats = match sim {
            Some(sim) => {
                let s = sim.stats();
                if let Some(n) = number.filter(|&n| n < LEVELS) {
                    let record = SaveRecord::new(
                        retail_level_name(n + 1),
                        claw_sim::SavePos {
                            slot: START_SLOT,
                            x: 0,
                            y: 0,
                        },
                        sim.carry(),
                        [0; claw_sim::player::stats::TREASURE_KINDS],
                    );
                    self.write(&record, n + 1);
                }
                BootyStats {
                    level: number.unwrap_or(1),
                    custom,
                    collected: s.treasure_level,
                    census: sim.treasure_census(),
                    score: s.score,
                    lives: s.lives,
                }
            }
            None => BootyStats {
                level: number.unwrap_or(1),
                custom,
                collected: [0; 9],
                census: [0; 9],
                score: 0,
                lives: 0,
            },
        };
        self.play = None;
        self.in_game = None;
        self.effects.push(Effect::EndLevel);
        self.effects.push(Effect::ScreenSounds);
        self.screen = Screen::Booty;
        self.booty = None;
        self.effects.push(Effect::ShowBooty(stats));
        self.fade_in();
    }

    fn drain_booty_audio(&mut self) {
        if let Some(b) = self.booty.as_mut() {
            for a in b.take_audio() {
                self.effects.push(Effect::Sound(a));
            }
        }
    }

    /// Lets `dt_ms` pass, with the direction `held` held in a menu and the
    /// simulation in play (`None` without a level).
    pub fn update(&mut self, dt_ms: f64, held: Option<MenuInput>, sim: Option<&Sim>) {
        let dt = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        self.elapsed += dt;
        if let Some(sim) = sim {
            // The end life question depends on the lives left.
            self.state.lives = sim.stats().lives;
        }
        if let Some((_, left)) = self.notices.front_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.notices.pop_front();
            }
        }
        self.update_bed(dt);
        if let Some(f) = self.fade.as_mut() {
            f.elapsed += dt;
            if f.elapsed >= FADE_MS {
                let out = f.out;
                self.fade = None;
                if out {
                    self.run_pending();
                    return;
                }
            }
        }
        match self.screen {
            Screen::Title => {
                let done = match self.title.as_mut() {
                    Some(t) => {
                        t.update(dt);
                        t.done()
                    }
                    None => true,
                };
                if done {
                    self.fade_to(Pending::Screen(Screen::MainMenu));
                }
            }
            Screen::MainMenu => {
                self.idle_ms += dt;
                let limit = match self.main.as_ref().map(Navigator::page) {
                    Some(Page::Root) | None => IDLE_ROOT_MS,
                    _ => IDLE_SUBMENU_MS,
                };
                if self.idle_ms >= limit && self.pending.is_none() {
                    self.fade_to(Pending::Screen(Screen::Title));
                }
                if self.fade.is_some_and(|f| f.out) {
                    return;
                }
                if let Some(n) = self.main.as_mut() {
                    let out = n.update(dt, held);
                    self.menu_actions(MenuKind::Main, out, false);
                }
            }
            Screen::Pause => {
                if let Some(n) = self.in_game.as_mut() {
                    let out = n.update(dt, held);
                    self.menu_actions(MenuKind::InGame, out, false);
                }
            }
            Screen::Play => {
                let due = match self.play.as_mut().and_then(|p| p.complete_in.as_mut()) {
                    Some(left) => {
                        *left -= dt;
                        *left <= 0.0
                    }
                    None => false,
                };
                if due {
                    self.complete_level(sim);
                }
            }
            Screen::Booty => {
                let done = match self.booty.as_mut() {
                    Some(b) => {
                        b.update(dt);
                        b.done()
                    }
                    None => false,
                };
                self.drain_booty_audio();
                if done {
                    self.fade_to(Pending::Screen(Screen::MainMenu));
                }
            }
            Screen::GameOver => {
                let done = match self.game_over.as_mut() {
                    Some(g) => {
                        g.update(dt);
                        g.done()
                    }
                    None => true,
                };
                if done {
                    self.enter(Screen::MainMenu);
                }
            }
            Screen::Exit => {}
        }
    }

    fn run_pending(&mut self) {
        let Some(p) = self.pending.take() else { return };
        match p {
            Pending::Screen(s) => self.enter(s),
            Pending::Start(level, start) => self.begin_play(level, start),
            Pending::Quit => {
                self.screen = Screen::Exit;
                self.effects.push(Effect::Quit);
            }
        }
    }
}

/// The `key_<action>` and `pad_<action>` config keys of `names`.
fn binding_keys(names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|n| Action::from_name(n).is_some())
        .flat_map(|n| [format!("key_{n}"), format!("pad_{n}")])
        .collect()
}

/// The flow's menu input for the router's.
pub fn menu_input(m: crate::input::MenuInput) -> MenuInput {
    use crate::input::MenuInput as R;
    match m {
        R::Up => MenuInput::Up,
        R::Down => MenuInput::Down,
        R::Left => MenuInput::Left,
        R::Right => MenuInput::Right,
        R::Confirm => MenuInput::Confirm,
        R::Back => MenuInput::Back,
    }
}

/// True for the screens drawn as standalone 640x480 screens.
pub fn is_standalone(screen: Screen) -> bool {
    matches!(screen, Screen::Title | Screen::MainMenu | Screen::Booty)
}
