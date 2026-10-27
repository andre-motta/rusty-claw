//! Movies in the game window.
//!
//! The flow asks for a movie ([`crate::flow::Effect::PlayMovie`]); the
//! window finds its sources ([`claw_movie::locate_all`]), opens the first
//! that plays ([`MoviePlayback::start`]), sends its soundtrack to the audio
//! engine, uploads each picture as it falls due and draws it full screen,
//! at the movie's own shape, centred on black. When the movie ends, is
//! skipped or cannot play (one line on the terminal says why), the window
//! tells the flow ([`crate::flow::Flow::movie_finished`]) and the game goes
//! on.
//!
//! Skipping: a movie file skips on Escape or a pad's Back or Start button;
//! a clip of the movie archive on any key, mouse click or pad button
//! ([`skips`]). Every input while a movie plays is the movie's: none of it
//! reaches the screen that follows.
//!
//! Only the game with its menus plays movies ([`movies_allowed`]): from
//! the title, or straight in a level. Replays, traces, benchmarks and the
//! tools never do, so movies change no recorded or measured run. `--movie
//! INDEX|FILE` plays one movie and ends the program; `--no-movies` turns
//! them off for the run.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use claw_movie::{
    FileKind, FrameLayout, Matrix, MovieError, MovieIndex, MovieMode, MovieSet, Player, Roots,
    SkipRule, Source, VideoFrame, locate_all, open_first,
};
use claw_render::{MoviePicture, PictureLayout};
use winit::keyboard::KeyCode;

use crate::audio::AudioBridge;
use crate::flow::MovieShelf;
use crate::input::{GilrsSource, PadEvent, PadEventKind, PadSource};
use crate::keymap::PadButton;
use crate::settings::{Mode, Settings};

/// Rate of a movie's soundtrack when no audio engine plays it.
const SILENT_RATE: u32 = 48000;

/// What `--movie` plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovieTarget {
    /// A movie of the install, by its index (-2 to 14).
    Index(MovieIndex),
    /// Any movie file.
    File(PathBuf),
}

impl MovieTarget {
    /// Reads `--movie`'s value: a whole number is an index, anything else
    /// a file (write `./3` for a file named `3`).
    pub fn parse(s: &str) -> Result<MovieTarget, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("expected a movie index or a file".into());
        }
        match s.parse::<i32>() {
            Ok(n) => i8::try_from(n)
                .ok()
                .and_then(MovieIndex::new)
                .map(MovieTarget::Index)
                .ok_or_else(|| {
                    format!(
                        "movie index {n} does not exist: the movies are {} to {}",
                        MovieIndex::MIN,
                        MovieIndex::MAX
                    )
                }),
            Err(_) => Ok(MovieTarget::File(PathBuf::from(s))),
        }
    }

    /// Where the movie can play from, best first.
    pub fn candidates(&self, roots: &Roots) -> Vec<Source> {
        match self {
            MovieTarget::Index(i) => locate_all(*i, roots),
            MovieTarget::File(path) => vec![Source::File {
                path: path.clone(),
                kind: FileKind::Original,
            }],
        }
    }
}

/// How a run goes, as far as movies are concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// The game from the title, with its menus.
    Menus,
    /// The game straight in a level (`--level`, `--custom`), with its menus
    /// after it.
    Play,
    /// A recording watched in the window (`--replay` without `--trace`).
    Watch,
    /// The window benchmark (`--bench`).
    WindowBench,
    /// An offscreen benchmark (`--bench --headless`, `--bench-route`).
    HeadlessBench,
    /// A replay written to a trace (`--replay --trace`), with no window.
    Trace,
    /// One movie, then the end of the program (`--movie`).
    Movie,
}

impl RunMode {
    /// The run `settings` describe.
    pub fn of(settings: &Settings) -> RunMode {
        if settings.movie.is_some() {
            return RunMode::Movie;
        }
        match &settings.mode {
            Mode::Play if settings.from_command_line.contains("level") => RunMode::Play,
            Mode::Play => RunMode::Menus,
            Mode::Replay { trace: Some(_), .. } => RunMode::Trace,
            Mode::Replay { trace: None, .. } => RunMode::Watch,
            Mode::Bench { headless: true, .. } => RunMode::HeadlessBench,
            Mode::Bench {
                headless: false, ..
            } => RunMode::WindowBench,
        }
    }
}

/// Whether the game plays its movies in a run of `mode`: only with its
/// menus (from the title, or straight in a level). Replays, traces and
/// benchmarks never do, so their runs stay as they were recorded and
/// measured; a `--movie` run plays the one movie it names instead.
pub fn movies_allowed(mode: &RunMode) -> bool {
    matches!(mode, RunMode::Menus | RunMode::Play)
}

/// Where this run looks for movies: the install's folder.
pub fn roots(settings: &Settings) -> Roots {
    Roots {
        install: settings.install.clone(),
        user_folder: None,
        mode: MovieMode::default(),
    }
}

/// The movies found under `roots`, when this build can play movies (none
/// otherwise).
pub fn shelf(roots: &Roots) -> MovieShelf {
    let available: MovieSet = if claw_movie::available() {
        MovieIndex::all()
            .filter(|&i| !locate_all(i, roots).is_empty())
            .collect()
    } else {
        MovieSet::default()
    };
    MovieShelf {
        available,
        ..MovieShelf::default()
    }
}

/// The one line a build without the movie player prints at start-up.
pub const NO_PLAYER: &str = "this build plays no movies; `make ffmpeg` builds FFmpeg, then \
                             `make run-game` builds the game with them";

/// An input while a movie plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawInput {
    /// A key went down.
    Key(KeyCode),
    /// A mouse button was clicked.
    Click,
    /// A pad button went down.
    Pad(PadButton),
}

/// Whether `event` skips a movie with `rule`: Escape or a pad's Back or
/// Start for a movie file, any key, click or pad button for an archive
/// clip.
pub fn skips(rule: SkipRule, event: &RawInput) -> bool {
    match rule {
        SkipRule::Back => matches!(
            event,
            RawInput::Key(KeyCode::Escape) | RawInput::Pad(PadButton::Select | PadButton::Start)
        ),
        SkipRule::AnyInput => true,
    }
}

/// The renderer's view of a decoded picture, for either layout.
pub fn picture(frame: &VideoFrame) -> MoviePicture<'_> {
    let layout = match frame.layout {
        FrameLayout::Rgba => PictureLayout::Rgba,
        FrameLayout::I420 { matrix, full_range } => PictureLayout::I420 {
            bt709: matrix == Matrix::Bt709,
            full_range,
        },
    };
    MoviePicture {
        width: frame.width,
        height: frame.height,
        layout,
        planes: [&frame.planes[0], &frame.planes[1], &frame.planes[2]],
        strides: frame.strides,
    }
}

/// A movie playing in the window (see the module documentation).
#[derive(Debug)]
pub struct MoviePlayback {
    player: Player,
    source: Source,
    rule: SkipRule,
    skipped: bool,
}

impl MoviePlayback {
    /// Opens the first of `candidates` that plays and sends its soundtrack
    /// to `audio` (without one, the movie runs silent on the wall clock).
    /// `None` when none plays, with one line on the terminal for each
    /// source tried, or one saying there was none.
    pub fn start(candidates: &[Source], audio: Option<&mut AudioBridge>) -> Option<MoviePlayback> {
        let rate = audio.as_ref().map_or(SILENT_RATE, |a| a.out_rate());
        match open_first(candidates, rate) {
            Ok((mut player, source)) => {
                if let Some(a) = audio
                    && let Some(sound) = player.take_audio()
                {
                    a.set_movie(Some(sound));
                }
                Some(MoviePlayback {
                    rule: source.skip_rule(),
                    player,
                    source,
                    skipped: false,
                })
            }
            Err(e) => {
                // Each source that failed to open said why already.
                match e {
                    MovieError::NotBuilt => eprintln!("rusty-claw: {NO_PLAYER}"),
                    MovieError::NotFound => {
                        eprintln!("rusty-claw: the movie does not play: no movie file found")
                    }
                    _ => {}
                }
                None
            }
        }
    }

    /// The picture to upload now: the newest one due, when there is one
    /// not shown yet.
    pub fn frame(&mut self) -> Option<MoviePicture<'_>> {
        self.player.next_frame().map(picture)
    }

    /// An input while the movie plays; true when it skips the movie. The
    /// window keeps every input of a playing movie from the screens,
    /// skipping or not.
    pub fn input(&mut self, event: &RawInput) -> bool {
        let skip = skips(self.rule, event);
        self.skipped |= skip;
        skip
    }

    /// Ends the movie early, as a skip does (for example when its pictures
    /// cannot be shown).
    pub fn end(&mut self) {
        self.skipped = true;
    }

    /// Whether the movie ended or was skipped.
    pub fn done(&self) -> bool {
        self.skipped || self.player.finished()
    }

    /// The movie's shape, width over height.
    pub fn aspect(&self) -> f32 {
        self.player.display_aspect()
    }

    /// Where it plays from.
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Stops the movie and its soundtrack; says why when decoding stopped
    /// early.
    pub fn stop(self, audio: Option<&mut AudioBridge>) {
        if let Some(a) = audio {
            a.set_movie(None);
        }
        if let Some(e) = self.player.error() {
            eprintln!(
                "rusty-claw: the movie {} stopped early: {e}",
                self.source.describe()
            );
        }
    }
}

/// The system's gamepads for the input router, noting each button pressed
/// so the window can hand it to a playing movie (the router turns pad
/// buttons into menu inputs, which cannot tell Start from the confirm
/// button). Opened at the first poll, as the router's own are.
#[derive(Debug, Default)]
pub struct PadTap {
    pads: Option<GilrsSource>,
    tried: bool,
    presses: Rc<RefCell<Vec<PadButton>>>,
}

impl PadTap {
    /// A tap and the list it notes presses in.
    pub fn new() -> (PadTap, Rc<RefCell<Vec<PadButton>>>) {
        let tap = PadTap::default();
        let presses = Rc::clone(&tap.presses);
        (tap, presses)
    }
}

impl PadSource for PadTap {
    fn next_event(&mut self) -> Option<PadEvent> {
        if !self.tried {
            self.tried = true;
            match GilrsSource::open() {
                Ok(pads) => self.pads = Some(pads),
                Err(e) => eprintln!(
                    "rusty-claw: gamepads are not available ({e}); playing with the keyboard"
                ),
            }
        }
        let event = self.pads.as_mut()?.next_event()?;
        if let PadEventKind::Button(button, true) = event.kind {
            self.presses.borrow_mut().push(button);
        }
        Some(event)
    }
}
