//! The game flow: title, main menu, options, loading, play, the in-game
//! menu, the end-of-level tally, the end of a game and the credits.
//!
//! [`Flow`] is a state machine with no window, GPU or audio device: the
//! window (or a test) feeds it menu inputs, the time that passed and the
//! game events of play, and carries out the [`Effect`]s it asks for
//! (loading a level, pausing, playing sounds, applying options).
//!
//! | From | Event | To |
//! | --- | --- | --- |
//! | start (plain `--install`) | | title, after the start-up movies when movies play |
//! | start (`--level`, `--custom`) | | play |
//! | logo movie | it ends, is skipped or cannot play | the intro movie on a first run, else the title |
//! | intro movie at start-up | it ends, is skipped or cannot play | title |
//! | title | its sound ends, or a key | main menu |
//! | main menu | idle 120 s (180 s in a submenu) | title, or every other time the intro movie |
//! | intro movie after the idle time | it ends, is skipped or cannot play | main menu |
//! | Replay Movies page | a movie chosen | that movie, then the same page |
//! | main menu | new game at level n | play, new-game values |
//! | main menu | load (level n, start or save point) | play from the save |
//! | main menu | quit, confirmed | the program ends |
//! | main menu | credits | credits (the menu rolls up, the parchment unrolls) |
//! | play | Escape or pad Start (not while the pause key holds play) | in-game menu |
//! | play | F1 (also while the pause key holds play) | help |
//! | in-game menu | help, or F1 | help |
//! | help | Escape, Space, Enter, pad choose or back | play |
//! | in-game menu | resume | play |
//! | in-game menu | end life, confirmed | play: Claw dies at the next tick |
//! | in-game menu | end the game, confirmed | main menu |
//! | play | level complete, after its delay | tally |
//! | play of level 2, 4, 6, 8, 10, 12 or 13 | level complete, after its delay | the level's movie when there is one, then the tally |
//! | play of level 14 | level complete, after its delay | the end-of-game movie (a short black hold without it), then the tally |
//! | tally of level n below 14 | done | play: level n + 1, progress carried |
//! | tally of level 14 | done | the tally fades, then the credits |
//! | credits | Escape, Space, Enter, a non-direction input, a click outside the top-left corner | main menu |
//! | tally of a community custom level | done | main menu |
//! | play | game over | the frozen game fades, then the main menu |
//!
//! Saved games: touching a save point writes save point 1 or 2 of the
//! level; finishing a level writes the start of the next level (score,
//! health, lives, ammunition and the game's treasure carried over), which
//! is what marks the level complete and opens the next level in the new
//! game and load lists. Community custom levels are never saved, and their
//! save points show no game-saved message ([`Flow::hide_messages`]).
//!
//! After the tally the game goes straight on to the next level, as the
//! original does: the next level starts from the start that finishing the
//! level just saved (whether or not it could be written), with one more
//! life when the tally was perfect and gave one; that life is not in the
//! saved start, as in the original. Levels whose objects are not all built
//! yet load and play with those objects drawn but still. A community
//! custom level ends at the main menu.
//!
//! The end of the game: when level 14 is won, its movie plays first, as
//! after every level that has one; without it the screen holds black for
//! [`CUT_SCREEN_MS`] in its place (Escape, Space or Enter end it at once).
//! Then the map screen and the tally follow as after every level.
//! When the tally is done it fades out over
//! [`claw_ui::screens::credits::FADE_OUT_MS`] and the credits follow (see
//! [`claw_ui::screens::credits`]); they lead back to the main menu, which
//! then shows at once, without its unroll. Finishing level 14 saves nothing
//! more: there is no level after it. The main menu's Credits entry opens
//! the same credits.
//!
//! The credits' background: when the credits movie is on the shelf
//! ([`MovieIndex::CREDITS`], see below) the credits show it, looping and
//! muted, behind their rising text in place of the parchment
//! ([`Flow::credits_movie`]); the window plays it while the credits show
//! and draws the text over it. The parchment's unroll and its sound are
//! left out then, and the credits keep their timing, text, music and
//! inputs. When the movie cannot play or stops on an error the window says
//! so ([`Flow::credits_movie_failed`]) and the parchment shows at once,
//! for the rest of that visit.
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
//! The pause key (see [`crate::pause`]) pauses play without a screen: the
//! flow stays in play, and the window holds the level.
//!
//! The Gameplay page under Options holds the behaviour switches: the
//! mid-air hit jump fix (off by default, as in the original), how keys
//! count across a pause or a screen (`pause_input`) and whether help
//! screen time reaches a lit dynamite fuse (`fuse_clock`), both at the
//! original's behaviour by default (see [`crate::settings`]), and the two
//! play-test aids, invincibility and infinite lives, both off by default.
//! Choosing an entry moves it to its next value. A change there is saved at
//! once and applies to the next game or level started; the level in play
//! keeps the behaviour it started with.
//!
//! Screens fade out over [`FADE_MS`] before the next one, which fades in
//! over the same time (with the menu's roll-up and unroll sounds).
//!
//! Movies: the flow plays none until the window hands it the movies it can
//! play ([`Flow::set_movies`]), which it does only for the game with its
//! menus (see [`crate::movie::movies_allowed`]). A movie is a screen of its
//! own ([`Screen::Movie`]): the music and the menu's loop stop before it,
//! the window plays it ([`Effect::PlayMovie`]) and says when it is over
//! ([`Flow::movie_finished`] when it ended or was skipped,
//! [`Flow::movie_missing`] when it could not play, [`Flow::movie_failed`]
//! when it opened but could not be shown or stopped early), and the next
//! screen starts with its own sounds. Menu inputs and the idle timers wait
//! while a movie plays, and no input given to a movie reaches the screen
//! after it.
//! The movies play at these moments, each only when it is on the shelf:
//!
//! - At start-up, for a run that starts at the title: on a first run (the
//!   intro movie was never seen) the logo movie, then the intro movie,
//!   whatever `skip_logo` says; once the intro has played to its end or
//!   was skipped, the flow records it ([`Effect::RecordIntroSeen`], written
//!   back as `intro_seen = yes`), and not when it failed. On later runs
//!   the logo movie only, and not with `skip_logo`; a first run without
//!   the intro on the shelf counts as a later one. A run straight in a
//!   level plays neither.
//! - After levels 2, 4, 6, 8, 10, 12, 13 and 14 (see
//!   [`MovieIndex::after_level`]): the level's movie comes first in its
//!   end-of-level screens, before the map screen, whose sounds wait for it.
//!   Community custom levels have none.
//! - In the main menu, when it is left alone: the idle time leads in turn
//!   to the intro movie (first) and to the title; after the movie the main
//!   menu shows again. Time with the window in the background does not
//!   count ([`Flow::set_background`]), so the intro never starts unseen.
//! - From the Replay Movies page: each of its entries replays one movie
//!   ([`replay_movie_index`]); the page and each entry are greyed out
//!   without one. After the movie the same page shows, the entry still
//!   highlighted.
//! - Behind the credits, looping: the credits movie, on the shelf only
//!   when the window found one and `credits_movie` is on. It is not a
//!   screen of its own and the Replay Movies page does not offer it.

use std::collections::{BTreeSet, VecDeque};

use claw_frame::{AudioCue, GameEvent, SoundChannel, SoundCue};
use claw_movie::{MovieIndex, MovieSet};
use claw_sim::logic::checkpoint::MESSAGE_SAVED;
use claw_sim::tuning::{FuseClock, PlayAids};
use claw_sim::{Sim, StartState};
use claw_ui::menu::command::{
    GameplaySetting, MenuCommand, MenuKind, Page, Slider, Toggle, Unavailable, VideoSetting,
};
use claw_ui::menu::draw::{self, FontArt, MenuState};
use claw_ui::menu::{MenuAction, MenuInput, MenuTree, Navigator};
use claw_ui::screens::credits::{self, Click, DELAYED_UNROLL_SOUND, UNROLL_SOUND};
use claw_ui::screens::{
    BootyScreen, BootyStats, CreditsEntry, CreditsScreen, CreditsTune, GameOverScreen, HelpScreen,
    TitleScreen,
};
use winit::keyboard::KeyCode;

use crate::input::{CaptureOutcome, Captured};
use crate::keymap::{Action, Keymap, PadMap, key_name};
use crate::movie::MovieTarget;
use crate::pause::PauseInput;
use crate::saves::{SaveRecord, SaveStore, retail_level_name};
use crate::settings::{LevelChoice, PAD_DEAD_ZONE_RANGE, Scale, Settings, WINDOW_SIZES};

/// Length of a screen's fade out or in, ms.
pub const FADE_MS: f64 = 700.0;
/// The black hold in place of the end-of-game movie, between the end of
/// level 14 and its map screen, ms.
pub const CUT_SCREEN_MS: f64 = 1000.0;
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
pub const VIDEO_SETTINGS: [VideoSetting; 9] = [
    VideoSetting::Resolution,
    VideoSetting::Aspect,
    VideoSetting::Scale,
    VideoSetting::Fullscreen,
    VideoSetting::Vsync,
    VideoSetting::FrameCap,
    VideoSetting::Interpolation,
    VideoSetting::SmoothRope,
    VideoSetting::StatusBar,
];

/// The Gameplay page's settings in order.
pub const GAMEPLAY_SETTINGS: [GameplaySetting; 5] = [
    GameplaySetting::MidAirHitFix,
    GameplaySetting::PauseInput,
    GameplaySetting::FuseClock,
    GameplaySetting::Invincible,
    GameplaySetting::InfiniteLives,
];

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
    /// The help screen over the frozen level.
    Help,
    /// The end-of-level screens.
    Booty,
    /// The frozen level fading out after the last life.
    GameOver,
    /// The credits.
    Credits,
    /// A movie, which the window plays (see [`Effect::PlayMovie`]).
    Movie(MovieIndex),
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
    /// Show the help screen over the level in play (already paused): hand
    /// a [`HelpScreen`] to [`Flow::set_help`]. [`Effect::ResumeGame`]
    /// follows when it closes.
    ShowHelp,
    /// Show the end-of-level screens: hand a [`BootyScreen`] for these
    /// stats to [`Flow::set_booty`].
    ShowBooty(BootyStats),
    /// Show the credits: hand a [`CreditsScreen`] opened from this entry
    /// to [`Flow::set_credits`].
    ShowCredits(CreditsEntry),
    /// Play a movie full screen, then call [`Flow::movie_finished`] when it
    /// ends or is skipped, or [`Flow::movie_missing`] when it cannot play.
    PlayMovie(MovieIndex),
    /// The intro movie has played for the first time: the flow wrote
    /// `intro_seen = yes` back to the config file (see [`Flow::settings`]),
    /// so later runs start with the logo movie only. Nothing more is asked
    /// of the window.
    RecordIntroSeen,
    /// Start a music track of the screens (looping or once), stopping the
    /// one playing.
    Music {
        /// The track.
        track: claw_audio::music::Track,
        /// Loop it.
        looped: bool,
    },
    /// End the program.
    Quit,
}

/// What a fade-out (or a movie) leads to.
#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Screen(Screen),
    Start(LevelChoice, StartState),
    Credits(CreditsEntry),
    Quit,
    /// A movie, then what follows it.
    Movie(MovieIndex, Box<Pending>),
    /// The end-of-level screens, waiting behind a movie.
    Booty,
    /// The main menu at this page, with this entry highlighted.
    MenuPage(usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fade {
    out: bool,
    elapsed: f64,
    len: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct Play {
    level: LevelChoice,
    checkpoint: Option<(i32, i32)>,
    complete_in: Option<f64>,
}

/// Whether `cue` starts the sound of the credits' parchment unrolling.
fn is_unroll(cue: &AudioCue) -> bool {
    matches!(cue, AudioCue::Play(c) if c.key == UNROLL_SOUND || c.key == DELAYED_UNROLL_SOUND)
}

/// The movie entry `entry` of the Replay Movies page replays (see
/// [`MenuCommand::ReplayMovie`]): 1 the logo, 2 the intro, 3 to 10 the
/// movies after levels 2, 4, 6, 8, 10, 12, 13 and 14.
pub fn replay_movie_index(entry: u8) -> Option<MovieIndex> {
    const AFTER: [u32; 8] = [2, 4, 6, 8, 10, 12, 13, 14];
    match entry {
        1 => Some(MovieIndex::LOGO),
        2 => Some(MovieIndex::INTRO),
        3..=10 => MovieIndex::after_level(AFTER[usize::from(entry - 3)]),
        _ => None,
    }
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
        VideoSetting::SmoothRope => format!("SMOOTH ROPE {}", on(settings.smooth_rope)),
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
/// hyphen or underscore, so "mid-air" is written as two words and the
/// settings file's names with spaces (`release_all` is RELEASE ALL).
pub fn gameplay_text(g: GameplaySetting, settings: &Settings) -> String {
    let on = |b: bool| if b { "ON" } else { "OFF" };
    let name = |n: &str| n.replace('_', " ").to_ascii_uppercase();
    match g {
        GameplaySetting::MidAirHitFix => {
            format!("MID AIR HIT JUMP FIX {}", on(settings.mid_air_hit_fix))
        }
        GameplaySetting::PauseInput => {
            format!("PAUSE INPUT {}", name(settings.pause_input.name()))
        }
        GameplaySetting::FuseClock => {
            format!("FUSE CLOCK {}", name(settings.fuse_clock.name()))
        }
        GameplaySetting::Invincible => format!("INVINCIBLE {}", on(settings.aids.invincible)),
        GameplaySetting::InfiniteLives => {
            format!("INFINITE LIVES {}", on(settings.aids.infinite_lives))
        }
    }
}

/// The value after `v` in `all`, back to the first after the last.
fn next_of<T: Copy + PartialEq>(all: &[T], v: T) -> T {
    let i = all.iter().position(|&x| x == v).map_or(0, |i| i + 1);
    all[i % all.len()]
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
    help: Option<HelpScreen>,
    booty: Option<BootyScreen>,
    game_over: Option<GameOverScreen>,
    credits: Option<CreditsScreen>,
    /// The tally on screen is the last level's: the credits follow it.
    ending: bool,
    /// Time left of the black hold before the last level's map screen, ms.
    cut_left: Option<f64>,
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
    /// The level the tally leads to and how it starts.
    next: Option<(u32, StartState)>,
    /// The movies this run can play, once the window said so.
    movies: Option<MovieShelf>,
    /// What follows the movie on screen.
    after_movie: Option<Pending>,
    /// The main menu's next idle time leads to the intro movie (rather
    /// than the title), when there is one.
    idle_movie: bool,
    /// The window is in the background: the idle time stands still.
    background: bool,
    /// The credits on screen show the credits movie behind their text.
    credits_movie: bool,
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
    /// One movie, then the end of the program. The window plays it without
    /// menus (see [`crate::window::run_movie`]); a flow started this way
    /// ends at once.
    Movie(MovieTarget),
}

/// The movies a run can play and the choices about them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovieShelf {
    /// The movies that have a source to play from.
    pub available: MovieSet,
    /// The intro movie has played on an earlier run.
    pub intro_seen: bool,
    /// The logo movie is not wanted at start-up.
    pub skip_logo: bool,
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
            help: None,
            booty: None,
            game_over: None,
            credits: None,
            ending: false,
            cut_left: None,
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
            next: None,
            movies: None,
            after_movie: None,
            idle_movie: true,
            background: false,
            credits_movie: false,
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
                    len: FADE_MS,
                });
            }
            Start::Play => {
                let level = flow.settings.level.clone();
                flow.begin_play(level, StartState::default());
            }
            Start::Movie(_) => {
                flow.screen = Screen::Exit;
                flow.effects.push(Effect::Quit);
            }
        }
        flow
    }

    /// Hands over the movies this run can play. Without this call the flow
    /// asks for no movie. Called before the first [`Flow::take_effects`]
    /// (or at least before any time passed), a run that starts at the title
    /// plays its start-up movies first (see the module documentation): the
    /// logo then the intro when the intro was never seen and is on the
    /// shelf, else the logo unless `skip_logo` is set.
    pub fn set_movies(&mut self, shelf: MovieShelf) {
        let at_start = self.screen == Screen::Title
            && self.elapsed == 0.0
            && self.pending.is_none()
            && self.play.is_none();
        let has = |i: MovieIndex| shelf.available.contains(i);
        let intro = !shelf.intro_seen && has(MovieIndex::INTRO);
        // skip_logo waits for the intro only when the intro plays now.
        let logo = has(MovieIndex::LOGO) && (intro || !shelf.skip_logo);
        self.movies = Some(shelf);
        self.refresh_enabled();
        if !at_start || !(logo || intro) {
            return;
        }
        self.effects.retain(|e| *e != Effect::ShowTitle);
        if let Some(mut t) = self.title.take() {
            let stop = t.leave();
            self.effects.push(Effect::Sound(stop));
        }
        let title = Pending::Screen(Screen::Title);
        match (logo, intro) {
            (true, true) => self.play_movie(
                MovieIndex::LOGO,
                Pending::Movie(MovieIndex::INTRO, Box::new(title)),
            ),
            (true, false) => self.play_movie(MovieIndex::LOGO, title),
            _ => self.play_movie(MovieIndex::INTRO, title),
        }
    }

    /// True when movie `index` is on the shelf.
    pub fn movie_available(&self, index: MovieIndex) -> bool {
        self.movies
            .as_ref()
            .is_some_and(|m| m.available.contains(index))
    }

    /// The movies this run can play, once [`Flow::set_movies`] was called.
    pub fn movies(&self) -> Option<&MovieShelf> {
        self.movies.as_ref()
    }

    /// True while the credits show and want the credits movie behind their
    /// text ([`MovieIndex::CREDITS`], when it is on the shelf): the window
    /// plays it, looping and muted, and draws the text over it, until this
    /// turns false (the credits closed, or the movie failed).
    pub fn credits_movie(&self) -> bool {
        self.credits_movie && self.screen == Screen::Credits
    }

    /// The credits movie could not play, could not be shown or stopped on
    /// an error: the credits go on over the parchment, at once and for the
    /// rest of this visit.
    pub fn credits_movie_failed(&mut self) {
        self.credits_movie = false;
    }

    /// The movie showing, if any.
    pub fn playing_movie(&self) -> Option<MovieIndex> {
        match self.screen {
            Screen::Movie(index) => Some(index),
            _ => None,
        }
    }

    /// The movie asked for by [`Effect::PlayMovie`] is over: it ended or
    /// was skipped. The screen after it follows. The first time the intro
    /// movie ends this way, the flow records that it was seen
    /// ([`Effect::RecordIntroSeen`]).
    pub fn movie_finished(&mut self) {
        let Screen::Movie(index) = self.screen else {
            return;
        };
        if index == MovieIndex::INTRO {
            self.record_intro_seen();
        }
        self.after_movie_ends();
    }

    /// The movie asked for by [`Effect::PlayMovie`] could not play (no
    /// source of it opened). The screen after it follows; an intro that
    /// could not play is not recorded as seen.
    pub fn movie_missing(&mut self) {
        if matches!(self.screen, Screen::Movie(_)) {
            self.after_movie_ends();
        }
    }

    /// The movie asked for by [`Effect::PlayMovie`] opened but failed: its
    /// pictures could not be shown, or decoding stopped on an error or
    /// before any picture. The screen after it follows, as when it could
    /// not play; an intro that failed is not recorded as seen.
    pub fn movie_failed(&mut self) {
        self.movie_missing();
    }

    fn after_movie_ends(&mut self) {
        let next = self
            .after_movie
            .take()
            .unwrap_or(Pending::Screen(Screen::MainMenu));
        self.pending = Some(next);
        self.run_pending();
    }

    /// Notes that the intro movie has played, once: in the shelf, in the
    /// settings and in the config file.
    fn record_intro_seen(&mut self) {
        let Some(shelf) = self.movies.as_mut() else {
            return;
        };
        if shelf.intro_seen {
            return;
        }
        shelf.intro_seen = true;
        self.settings.intro_seen = true;
        // Without a config file there is nothing to write it to; the run
        // says nothing about it.
        if self.settings.config_path.is_some() {
            self.persist(&["intro_seen"]);
        }
        self.effects.push(Effect::RecordIntroSeen);
    }

    /// Shows movie `index`, then `then`. The music and the menu's loop stop
    /// first.
    fn play_movie(&mut self, index: MovieIndex, then: Pending) {
        self.effects.push(Effect::Sound(AudioCue::StopMusic));
        if self.bed_volume > 0 || self.bed.is_some() {
            self.effects.push(Effect::Sound(AudioCue::Stop {
                voice: MENU_BED_VOICE,
            }));
            self.bed_volume = 0;
            self.bed = None;
        }
        self.screen = Screen::Movie(index);
        self.idle_ms = 0.0;
        self.fade = None;
        self.pending = None;
        self.after_movie = Some(then);
        self.effects.push(Effect::PlayMovie(index));
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

    /// The help screen.
    pub fn help(&self) -> Option<&HelpScreen> {
        self.help.as_ref()
    }

    /// True while the level in play is complete and waits for its tally.
    pub fn completing(&self) -> bool {
        self.play.as_ref().is_some_and(|p| p.complete_in.is_some())
    }

    /// The end-of-level screens.
    pub fn booty(&self) -> Option<&BootyScreen> {
        self.booty.as_ref()
    }

    /// The level the tally on screen leads to and how it starts (`None`
    /// when it leads to the main menu); a perfect tally's extra life is
    /// added when the tally ends.
    pub fn next_level(&self) -> Option<(u32, &StartState)> {
        self.next.as_ref().map(|(n, s)| (*n, s))
    }

    /// The game-over fade.
    pub fn game_over(&self) -> Option<&GameOverScreen> {
        self.game_over.as_ref()
    }

    /// The credits.
    pub fn credits(&self) -> Option<&CreditsScreen> {
        self.credits.as_ref()
    }

    /// True during the black hold before the last level's map screen.
    pub fn cut_screen(&self) -> bool {
        self.cut_left.is_some()
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
        if self.cut_left.is_some() {
            return 0.0;
        }
        match self.fade {
            Some(f) => {
                let t = (f.elapsed / f.len).clamp(0.0, 1.0);
                if f.out { 1.0 - t } else { t }
            }
            None if self.screen == Screen::Credits => {
                self.credits.as_ref().map_or(0.0, CreditsScreen::brightness)
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

    /// Hands over the help screen asked for by [`Effect::ShowHelp`].
    pub fn set_help(&mut self, help: HelpScreen) {
        self.help = Some(help);
    }

    /// Hands over the screens asked for by [`Effect::ShowBooty`].
    pub fn set_booty(&mut self, booty: BootyScreen) {
        self.booty = Some(booty);
        self.drain_booty_audio();
    }

    /// Hands over the credits asked for by [`Effect::ShowCredits`].
    pub fn set_credits(&mut self, credits: CreditsScreen) {
        self.credits = Some(credits);
        self.drain_credits();
    }

    /// A newly pressed input other than the four directions and the menu
    /// keys (a game key binding or a pad button): on the credits it goes
    /// back to the main menu.
    pub fn credits_press(&mut self) {
        if self.screen == Screen::Credits
            && self.pending.is_none()
            && let Some(c) = self.credits.as_mut()
            && c.press()
        {
            self.leave_credits();
        }
    }

    /// A click at (x, y) of the 640x480 screen: on the credits, the
    /// top-left corner toggles the developer's tune and anywhere else goes
    /// back to the main menu.
    pub fn click(&mut self, x: i32, y: i32) {
        if self.screen != Screen::Credits || self.pending.is_some() {
            return;
        }
        let Some(c) = self.credits.as_mut() else {
            return;
        };
        match c.click(x, y) {
            Click::Leave => self.leave_credits(),
            Click::Corner => self.drain_credits(),
            Click::Ignored => {}
        }
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
        self.fade_to_over(pending, FADE_MS);
    }

    fn fade_to_over(&mut self, pending: Pending, len: f64) {
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
            len,
        });
        if matches!(self.screen, Screen::MainMenu | Screen::Title) {
            self.sound("ROLLUP");
        }
    }

    fn fade_in(&mut self) {
        self.fade = Some(Fade {
            out: false,
            elapsed: 0.0,
            len: FADE_MS,
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
                self.next = None;
                self.game_over = None;
                self.credits = None;
                self.credits_movie = false;
                self.ending = false;
                self.cut_left = None;
                if let Some(n) = self.main.as_mut() {
                    n.reset();
                }
                self.refresh_enabled();
                // After the credits the menu shows at once, without its
                // unroll.
                let unroll = from != Screen::Credits;
                if unroll {
                    self.sound("UNROLL");
                }
                if !self.intro_played {
                    self.intro_played = true;
                    self.sound("INTRO");
                }
                self.bed_volume = 0;
                self.set_bed(1.0);
                self.bed_to(MENU_BED_VOLUME, MENU_BED_RISE_MS);
                if unroll {
                    self.fade_in();
                } else {
                    self.fade = None;
                }
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
        self.booty = None;
        self.next = None;
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
        let shelf = self.movies.as_ref().map(|m| m.available.clone());
        // The credits movie plays behind the credits only, never from the
        // Replay Movies page.
        let any_movie = shelf
            .as_ref()
            .is_some_and(|s| s.iter().any(|i| !i.is_credits()));
        let rule = |item: &claw_ui::menu::MenuItem| match item.command {
            MenuCommand::Open(Page::Movies) => any_movie,
            MenuCommand::ReplayMovie(entry) => replay_movie_index(entry)
                .is_some_and(|i| shelf.as_ref().is_some_and(|s| s.contains(i))),
            MenuCommand::NewGame { level } => progress.can_start(level),
            MenuCommand::Open(Page::LoadEntries { level }) => progress.can_start(level),
            MenuCommand::Load { level, entry: 0 } => progress.can_start(level),
            MenuCommand::Load { level, entry } => progress.has(level, entry),
            MenuCommand::Unavailable(Unavailable::Credits) => true,
            MenuCommand::Unavailable(_) => false,
            MenuCommand::Toggle(Toggle::Voice | Toggle::Ambient) => sound,
            MenuCommand::Toggle(Toggle::Music) => music,
            _ => true,
        };
        if let Some(n) = self.main.as_mut() {
            n.tree.set_enabled_by(rule);
        }
        if let Some(n) = self.in_game.as_mut() {
            // The in-game menu shows the help screen.
            n.tree.set_enabled_by(|item| {
                item.command == MenuCommand::Unavailable(Unavailable::Help) || rule(item)
            });
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
            Screen::Help => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back) {
                    self.close_help();
                }
            }
            Screen::Booty => {
                if !matches!(input, MenuInput::Confirm | MenuInput::Back) {
                    return;
                }
                if self.cut_left.is_some() {
                    self.end_cut_screen();
                } else if let Some(b) = self.booty.as_mut() {
                    b.skip();
                    self.drain_booty_audio();
                }
            }
            Screen::Credits => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back)
                    && let Some(c) = self.credits.as_mut()
                    && c.press()
                {
                    self.leave_credits();
                }
            }
            Screen::GameOver => {
                if matches!(input, MenuInput::Confirm | MenuInput::Back)
                    && let Some(g) = self.game_over.as_mut()
                {
                    g.skip();
                }
            }
            Screen::Play | Screen::Movie(_) | Screen::Exit => {}
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

    /// The menu key during a game (Escape, or a gamepad's Start): in play
    /// it opens the in-game menu, in the in-game menu it resumes. Returns
    /// true when the flow took the key. The window does not call it while
    /// the pause key holds play (see [`crate::pause::game_key`]).
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

    /// F1 during a game: the help screen over play (paused or not) or in
    /// place of the in-game menu. Returns true when it opened.
    pub fn help_key(&mut self) -> bool {
        match self.screen {
            Screen::Play if !self.completing() => {
                self.screen = Screen::Help;
                self.effects.push(Effect::PauseGame);
                self.effects.push(Effect::ShowHelp);
                true
            }
            Screen::Pause => {
                self.screen = Screen::Help;
                self.effects.push(Effect::ShowHelp);
                true
            }
            _ => false,
        }
    }

    fn close_help(&mut self) {
        if self.screen == Screen::Help {
            self.screen = Screen::Play;
            self.help = None;
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
                self.push_notice(format!(
                    "options are not saved: no config file location (set {})",
                    crate::settings::CONFIG_HOME_VARS
                ));
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
            MenuCommand::Unavailable(Unavailable::Help) => {
                self.help_key();
            }
            MenuCommand::Unavailable(Unavailable::Credits) => {
                if self.screen == Screen::MainMenu {
                    self.fade_to(Pending::Credits(CreditsEntry::FromMenu));
                }
            }
            MenuCommand::ReplayMovie(entry) => self.replay_movie(entry),
            MenuCommand::Open(_)
            | MenuCommand::Back
            | MenuCommand::Controls
            | MenuCommand::Adjust(_)
            | MenuCommand::Unavailable(_)
            | MenuCommand::Nothing => {}
        }
    }

    /// Replays movie `entry` of the Replay Movies page, then shows the page
    /// again with the entry highlighted.
    fn replay_movie(&mut self, entry: u8) {
        if self.screen != Screen::MainMenu || self.pending.is_some() {
            return;
        }
        let Some(index) = replay_movie_index(entry).filter(|&i| self.movie_available(i)) else {
            return;
        };
        let back = self
            .main
            .as_ref()
            .map_or(Pending::Screen(Screen::MainMenu), |n| {
                let page = n.page_item();
                let at = n
                    .selected()
                    .and_then(|k| n.entries().iter().position(|&e| e == k))
                    .unwrap_or(0);
                Pending::MenuPage(page, at)
            });
        self.play_movie(index, back);
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
            VideoSetting::SmoothRope => {
                s.smooth_rope = !s.smooth_rope;
                Some("smooth_rope")
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
            GameplaySetting::PauseInput => {
                self.settings.pause_input = next_of(&PauseInput::ALL, self.settings.pause_input);
                "pause_input"
            }
            GameplaySetting::FuseClock => {
                self.settings.fuse_clock = next_of(&FuseClock::ALL, self.settings.fuse_clock);
                "fuse_clock"
            }
            GameplaySetting::Invincible => {
                let aids = &mut self.settings.aids;
                aids.invincible = !aids.invincible;
                PlayAids::INVINCIBLE
            }
            GameplaySetting::InfiniteLives => {
                let aids = &mut self.settings.aids;
                aids.infinite_lives = !aids.infinite_lives;
                PlayAids::INFINITE_LIVES
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
        self.next = None;
        self.ending = number == Some(LEVELS);
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
                    // The tally leads to that start, saved or not.
                    self.next = Some((n + 1, record.start_state()));
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
        let movie = number
            .and_then(MovieIndex::after_level)
            .filter(|&i| self.movie_available(i));
        self.play = None;
        self.in_game = None;
        self.effects.push(Effect::EndLevel);
        self.effects.push(Effect::ScreenSounds);
        self.screen = Screen::Booty;
        self.booty = None;
        self.effects.push(Effect::ShowBooty(stats));
        if let Some(index) = movie {
            // The level's movie first; the map screen fades in after it.
            self.fade = None;
            self.play_movie(index, Pending::Booty);
        } else if self.ending {
            // The black in place of the end-of-game movie; the map screen
            // fades in after it.
            self.fade = None;
            self.cut_left = Some(CUT_SCREEN_MS);
        } else {
            self.fade_in();
        }
    }

    /// Ends the black hold before the last level's map screen.
    fn end_cut_screen(&mut self) {
        if self.cut_left.take().is_some() {
            self.fade_in();
            self.drain_booty_audio();
        }
    }

    fn drain_booty_audio(&mut self) {
        if self.cut_left.is_some() || matches!(self.screen, Screen::Movie(_)) {
            // The map screen's sounds wait for the end of the black or of
            // the movie.
            return;
        }
        if let Some(b) = self.booty.as_mut() {
            for a in b.take_audio() {
                self.effects.push(Effect::Sound(a));
            }
        }
    }

    /// Tells the flow whether the window is in the background. The main
    /// menu's idle time stands still there, so the idle intro (or title)
    /// never starts while nobody can see it; it counts on from where it was
    /// once the window is back in front.
    pub fn set_background(&mut self, on: bool) {
        self.background = on;
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
            if f.elapsed >= f.len {
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
                if !self.background {
                    self.idle_ms += dt;
                }
                let limit = match self.main.as_ref().map(Navigator::page) {
                    Some(Page::Root) | None => IDLE_ROOT_MS,
                    _ => IDLE_SUBMENU_MS,
                };
                if self.idle_ms >= limit && self.pending.is_none() {
                    // In turn the intro movie, when there is one, and the
                    // title.
                    let movie = self.idle_movie && self.movie_available(MovieIndex::INTRO);
                    self.idle_movie = !movie;
                    self.fade_to(if movie {
                        Pending::Movie(
                            MovieIndex::INTRO,
                            Box::new(Pending::Screen(Screen::MainMenu)),
                        )
                    } else {
                        Pending::Screen(Screen::Title)
                    });
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
                if let Some(left) = self.cut_left.as_mut() {
                    *left -= dt;
                    if *left <= 0.0 {
                        self.end_cut_screen();
                    }
                    return;
                }
                let done = match self.booty.as_mut() {
                    Some(b) => {
                        b.update(dt);
                        b.done()
                    }
                    None => false,
                };
                self.drain_booty_audio();
                if done && self.pending.is_none() {
                    self.after_booty();
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
            Screen::Credits => {
                match self.credits.as_mut() {
                    Some(c) => c.update(dt),
                    // Without the screen there is nothing to show.
                    None => {
                        self.enter(Screen::MainMenu);
                        return;
                    }
                }
                self.drain_credits();
            }
            Screen::Help | Screen::Movie(_) | Screen::Exit => {}
        }
    }

    /// Opens the credits from `entry`.
    fn enter_credits(&mut self, entry: CreditsEntry) {
        self.screen = Screen::Credits;
        self.idle_ms = 0.0;
        self.fade = None;
        self.booty = None;
        self.next = None;
        self.ending = false;
        self.cut_left = None;
        self.credits = None;
        self.credits_movie = self.movie_available(MovieIndex::CREDITS);
        if self.bed_volume > 0 || self.bed.is_some() {
            self.effects.push(Effect::Sound(AudioCue::Stop {
                voice: MENU_BED_VOICE,
            }));
            self.bed_volume = 0;
            self.bed = None;
        }
        self.effects.push(Effect::ShowCredits(entry));
    }

    /// The credits' sounds and music.
    fn drain_credits(&mut self) {
        let Some(c) = self.credits.as_mut() else {
            return;
        };
        let audio = c.take_audio();
        let music = c.take_music();
        // Over the movie the parchment does not unroll, so neither does its
        // sound play.
        let movie = self.credits_movie();
        for a in audio {
            if movie && is_unroll(&a) {
                continue;
            }
            self.effects.push(Effect::Sound(a));
        }
        for tune in music {
            let (track, looped) = match tune {
                CreditsTune::Credits => (claw_audio::music::Track::Credits, true),
                CreditsTune::Monolith => (claw_audio::music::Track::Monolith, false),
            };
            self.effects.push(Effect::Music { track, looped });
        }
    }

    /// Leaves the credits for the main menu: their music stops (and the
    /// window stops their movie, see [`Flow::credits_movie`]).
    fn leave_credits(&mut self) {
        self.drain_credits();
        self.effects.push(Effect::Sound(AudioCue::StopMusic));
        self.enter(Screen::MainMenu);
    }

    /// The tally ended: the next level, the credits after the last level,
    /// or the main menu after a community custom level.
    fn after_booty(&mut self) {
        if self.ending && self.next.is_none() {
            self.fade_to_over(
                Pending::Credits(CreditsEntry::AfterGame),
                credits::FADE_OUT_MS,
            );
            return;
        }
        match self.next.take() {
            Some((level, mut start)) => {
                // A perfect tally's extra life goes with Claw.
                if self.booty.as_ref().is_some_and(BootyScreen::extra_life)
                    && let Some(c) = start.carry.as_mut()
                    && c.lives < claw_ui::screens::booty::MAX_LIVES
                {
                    c.lives += 1;
                }
                self.fade_to(Pending::Start(LevelChoice::Retail(level), start));
            }
            None => self.fade_to(Pending::Screen(Screen::MainMenu)),
        }
    }

    fn run_pending(&mut self) {
        let Some(p) = self.pending.take() else { return };
        match p {
            Pending::Screen(s) => self.enter(s),
            Pending::Start(level, start) => self.begin_play(level, start),
            Pending::Credits(entry) => self.enter_credits(entry),
            Pending::Quit => {
                self.screen = Screen::Exit;
                self.effects.push(Effect::Quit);
            }
            Pending::Movie(index, then) => self.play_movie(index, *then),
            Pending::Booty => {
                self.screen = Screen::Booty;
                self.idle_ms = 0.0;
                self.fade_in();
                self.drain_booty_audio();
            }
            Pending::MenuPage(page, at) => {
                self.enter(Screen::MainMenu);
                if let Some(n) = self.main.as_mut() {
                    n.show(page, at);
                }
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
    matches!(
        screen,
        Screen::Title | Screen::MainMenu | Screen::Booty | Screen::Help | Screen::Credits
    )
}
