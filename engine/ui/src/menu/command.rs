//! What each menu entry does: the command catalogue.
//!
//! A menu file gives every entry a command number. The catalogue turns
//! those numbers into a [`MenuCommand`] named by what it does, plus the
//! slider an entry also works as and its hotkey letter. Entries for
//! features this game does not have (multiplayer, movies, help, credits,
//! community custom level picking, score upload, player profiles) map to
//! [`MenuCommand::Unavailable`]: they stay in the menu, greyed out, and
//! play the disabled sound. The game enables two of them where it has the
//! screen: the in-game menu's help and the main menu's credits.

/// Which menu a tree belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuKind {
    /// The main menu.
    Main,
    /// The menu over a game in progress.
    InGame,
    /// The menu after the last life is lost.
    GameOver,
}

/// A submenu, by what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// The top of the menu.
    Root,
    /// Single player: new game, load game, saving, back.
    SinglePlayer,
    /// The levels a new game can start at.
    NewGameLevels,
    /// The levels a game can be loaded from.
    LoadGameLevels,
    /// The start and the two save points of one level.
    LoadEntries {
        /// The level, 1 to 14.
        level: u32,
    },
    /// How saving works.
    SaveInfo,
    /// Options.
    Options,
    /// Sound, speech, ambient sound and music.
    Audio,
    /// Detail, front layer, the status bar area and movie settings.
    Display,
    /// Window and picture settings of this game.
    Video,
    /// Gameplay settings of this game.
    Gameplay,
    /// Key and gamepad bindings.
    Controls,
    /// Are you sure you want to quit?
    QuitConfirm,
    /// Are you sure you want to lose a life?
    EndLifeConfirm,
    /// Are you sure you want to end the game?
    EndGameConfirm,
    /// Multiplayer (not available).
    Multiplayer,
    /// Movies (not available).
    Movies,
    /// Any other submenu.
    Other,
}

/// A setting that an entry switches on and off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toggle {
    /// Sound as a whole.
    Sound,
    /// Speech.
    Voice,
    /// Ambient sounds.
    Ambient,
    /// Music.
    Music,
    /// High detail.
    HighDetail,
    /// The front layer of the level.
    FrontLayer,
    /// Interlaced movies (kept only).
    MovieInterlace,
}

/// A setting that an entry changes with left and right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slider {
    /// Sound volume, 0 to 80 in steps of 8.
    SoundVolume,
    /// Music volume, 0 to 100 in steps of 10.
    MusicVolume,
    /// Size of the play area, 1 to 10.
    PlayArea,
}

impl Slider {
    /// The change one step makes.
    pub fn step(self) -> i32 {
        match self {
            Slider::SoundVolume => 8,
            Slider::MusicVolume => 10,
            Slider::PlayArea => 1,
        }
    }

    /// The lowest and highest values the slider sets.
    pub fn range(self) -> (i32, i32) {
        match self {
            Slider::SoundVolume => (0, 80),
            Slider::MusicVolume => (0, 100),
            Slider::PlayArea => (1, 10),
        }
    }

    /// `value` moved by `delta`, kept in range, never moving the other way
    /// than `delta` (a value above the range stays when raised). The music
    /// volume is first rounded to the nearest step.
    pub fn adjust(self, value: i32, delta: i32) -> i32 {
        let (lo, hi) = self.range();
        let v = if self == Slider::MusicVolume {
            (value + 5).div_euclid(10) * 10
        } else {
            value
        };
        let next = (v + delta).clamp(lo, hi);
        match delta.signum() {
            1 => next.max(value),
            -1 => next.min(value),
            _ => value,
        }
    }
}

/// Settings of this game's own Video page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoSetting {
    /// Window size.
    Resolution,
    /// Fill, centred or 4:3.
    Aspect,
    /// Fit or whole-number scaling.
    Scale,
    /// Fullscreen.
    Fullscreen,
    /// Waiting for the display refresh.
    Vsync,
    /// Frame-rate cap.
    FrameCap,
    /// Smooth motion between game steps.
    Interpolation,
    /// Rope swings drawn smoothly between the rope's frames.
    SmoothRope,
    /// Status bar at the picture's edges or in the centred 4:3 area.
    StatusBar,
}

/// Settings of this game's own Gameplay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplaySetting {
    /// The mid-air hit jump fix: no second jump after a hit that ends in
    /// mid-air.
    MidAirHitFix,
    /// How keys held or pressed across a pause or a screen count when play
    /// goes on: as in the original, or every key released.
    PauseInput,
    /// Whether the time the help screen is up reaches a lit dynamite fuse:
    /// as in the original, or frozen like every other screen.
    FuseClock,
    /// Play-test aid: Claw takes no damage (death tiles still end his
    /// life). Off by default.
    Invincible,
    /// Play-test aid: losing a life never lowers the lives count. Off by
    /// default.
    InfiniteLives,
}

/// A feature this game does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unavailable {
    /// Multiplayer, racing and the lobby.
    Multiplayer,
    /// The movies.
    Movies,
    /// The help screen.
    Help,
    /// The credits.
    Credits,
    /// Picking a community custom level from the menu.
    CustomLevels,
    /// Uploading scores.
    UploadScores,
    /// Player profiles.
    Players,
}

/// What choosing an entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuCommand {
    /// Shows the entry's submenu.
    Open(Page),
    /// Goes back to the submenu above.
    Back,
    /// Starts a new game at a level.
    NewGame {
        /// The level, 1 to 14.
        level: u32,
    },
    /// Loads a level from its start (entry 0) or a save point (1, 2).
    Load {
        /// The level, 1 to 14.
        level: u32,
        /// 0 the level start, 1 and 2 the save points.
        entry: u8,
    },
    /// Quits the game.
    Quit,
    /// Closes the in-game menu.
    Resume,
    /// Loses a life.
    EndLife,
    /// Ends the game and goes to the main menu.
    EndGame,
    /// Switches a setting.
    Toggle(Toggle),
    /// A slider's own entry: choosing it only plays the select sound.
    Adjust(Slider),
    /// Changes a Video setting.
    Video(VideoSetting),
    /// Changes a Gameplay setting.
    Gameplay(GameplaySetting),
    /// Shows the key and gamepad bindings.
    Controls,
    /// Rebinds the action in this position of the controls list.
    Bind(u8),
    /// Puts every binding back to the defaults.
    DefaultBindings,
    /// The gamepad stick's dead zone: choosing it raises the dead zone one
    /// step (from the highest back to the lowest); Left and Right lower
    /// and raise it.
    PadDeadZone,
    /// A feature this game does not have.
    Unavailable(Unavailable),
    /// Nothing.
    Nothing,
}

/// One entry's meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueEntry {
    /// What choosing it does.
    pub command: MenuCommand,
    /// The slider left and right change, if any.
    pub slider: Option<Slider>,
    /// The letter key that chooses it, if any.
    pub hotkey: Option<char>,
}

impl CatalogueEntry {
    fn new(command: MenuCommand, hotkey: Option<char>) -> CatalogueEntry {
        CatalogueEntry {
            command,
            slider: None,
            hotkey,
        }
    }
}

/// The command catalogue of one menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandCatalogue {
    kind: MenuKind,
}

/// Hotkeys of the level entries, levels 1 to 14.
const LEVEL_KEYS: &[u8; 14] = b"RAFWOLDSPCVUIT";
/// Hotkeys of the movie entries in order.
const MOVIE_KEYS: &[u8; 10] = b"UIEOTHRGWC";

fn key(b: &[u8], i: usize) -> Option<char> {
    b.get(i).map(|&c| char::from(c))
}

impl CommandCatalogue {
    /// The catalogue of `kind`'s menu.
    pub fn new(kind: MenuKind) -> CommandCatalogue {
        CommandCatalogue { kind }
    }

    /// The menu this catalogue belongs to.
    pub fn kind(&self) -> MenuKind {
        self.kind
    }

    /// The meaning of command number `id`.
    pub fn entry(&self, id: u32) -> CatalogueEntry {
        match self.kind {
            MenuKind::Main => main_entry(id),
            MenuKind::InGame => in_game_entry(id),
            MenuKind::GameOver => CatalogueEntry::new(MenuCommand::Nothing, None),
        }
    }
}

fn main_entry(id: u32) -> CatalogueEntry {
    use MenuCommand as C;
    use Unavailable as U;
    let e = CatalogueEntry::new;
    let c = |ch: char| Some(ch);
    match id {
        10001 => e(C::Open(Page::SinglePlayer), c('S')),
        10002 => e(C::Unavailable(U::Multiplayer), c('M')),
        10003 => e(C::Unavailable(U::Movies), c('R')),
        10004 => e(C::Open(Page::Options), c('O')),
        10005 => e(C::Unavailable(U::Help), c('H')),
        10006 => e(C::Open(Page::QuitConfirm), c('Q')),
        10007 => e(C::Unavailable(U::Credits), c('C')),
        11001 => e(C::Open(Page::NewGameLevels), c('N')),
        11002 => e(C::Open(Page::LoadGameLevels), c('L')),
        11003 => e(C::Unavailable(U::CustomLevels), c('C')),
        11004 => e(C::Open(Page::SaveInfo), c('S')),
        11005 => e(C::Unavailable(U::UploadScores), c('U')),
        11006 | 11115 | 11215 | 11401 | 12004 | 13006 | 13405 | 13506 | 15011 => e(C::Back, c('B')),
        14002 => e(C::Back, c('N')),
        11101..=11114 => e(
            C::NewGame { level: id - 11100 },
            key(LEVEL_KEYS, (id - 11101) as usize),
        ),
        11201..=11214 => e(
            C::Open(Page::LoadEntries { level: id - 11200 }),
            key(LEVEL_KEYS, (id - 11201) as usize),
        ),
        112_011..=112_144 => {
            let level = (id - 112_000) / 10;
            let index = id % 10;
            if !(1..=14).contains(&level) {
                return e(C::Nothing, None);
            }
            match index {
                1 => e(
                    C::Load { level, entry: 0 },
                    if level >= 4 { c('S') } else { None },
                ),
                2 => e(C::Load { level, entry: 1 }, c('1')),
                3 => e(C::Load { level, entry: 2 }, c('2')),
                4 => e(C::Back, c('B')),
                _ => e(C::Nothing, None),
            }
        }
        12001 => e(C::Unavailable(U::Multiplayer), c('R')),
        12002 => e(C::Unavailable(U::Multiplayer), None),
        12003 => e(C::Unavailable(U::Multiplayer), c('E')),
        13001 => e(C::Unavailable(U::Players), c('E')),
        13003 => e(C::Controls, c('C')),
        13004 => e(C::Open(Page::Audio), c('A')),
        13005 => e(C::Open(Page::Display), c('I')),
        13401 => CatalogueEntry {
            slider: Some(Slider::SoundVolume),
            ..e(C::Toggle(Toggle::Sound), c('S'))
        },
        13402 => e(C::Toggle(Toggle::Voice), c('V')),
        13403 => e(C::Toggle(Toggle::Ambient), c('A')),
        13404 => CatalogueEntry {
            slider: Some(Slider::MusicVolume),
            ..e(C::Toggle(Toggle::Music), c('M'))
        },
        13502 => e(C::Toggle(Toggle::HighDetail), c('D')),
        13503 => e(C::Toggle(Toggle::FrontLayer), c('F')),
        13504 => CatalogueEntry {
            slider: Some(Slider::PlayArea),
            ..e(C::Adjust(Slider::PlayArea), c('G'))
        },
        13505 => e(C::Toggle(Toggle::MovieInterlace), c('M')),
        14001 => e(C::Quit, c('Y')),
        15001..=15010 => e(
            C::Unavailable(U::Movies),
            key(MOVIE_KEYS, (id - 15001) as usize),
        ),
        _ => e(C::Nothing, None),
    }
}

fn in_game_entry(id: u32) -> CatalogueEntry {
    use MenuCommand as C;
    let e = CatalogueEntry::new;
    let c = |ch: char| Some(ch);
    match id {
        20001 => e(C::Resume, c('R')),
        20002 => e(C::Open(Page::EndLifeConfirm), c('L')),
        20003 => e(C::Open(Page::Options), c('O')),
        20004 => e(C::Unavailable(Unavailable::Help), c('H')),
        20005 => e(C::Open(Page::EndGameConfirm), c('E')),
        22001 => e(C::EndLife, c('Y')),
        24001 => e(C::EndGame, c('Y')),
        22002 | 24002 => e(C::Back, c('N')),
        23001 => e(C::Controls, c('C')),
        23002 => e(C::Open(Page::Display), c('I')),
        23003 => e(C::Open(Page::Audio), c('A')),
        23004 | 23205 | 23304 => e(C::Back, c('B')),
        23201 => CatalogueEntry {
            slider: Some(Slider::SoundVolume),
            ..e(C::Toggle(Toggle::Sound), c('S'))
        },
        23202 => e(C::Toggle(Toggle::Voice), c('V')),
        23203 => e(C::Toggle(Toggle::Ambient), c('A')),
        23204 => CatalogueEntry {
            slider: Some(Slider::MusicVolume),
            ..e(C::Toggle(Toggle::Music), c('M'))
        },
        23301 => e(C::Toggle(Toggle::FrontLayer), c('F')),
        23302 => CatalogueEntry {
            slider: Some(Slider::PlayArea),
            ..e(C::Adjust(Slider::PlayArea), c('G'))
        },
        23303 => e(C::Toggle(Toggle::MovieInterlace), c('M')),
        _ => e(C::Nothing, None),
    }
}
