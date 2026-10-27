//! Movies in the game window.
//!
//! The flow asks for a movie ([`crate::flow::Effect::PlayMovie`]); the
//! window finds its sources ([`claw_movie::locate_all`]), opens the first
//! that plays ([`MoviePlayback::start`]), sends its soundtrack to the audio
//! engine, uploads each picture as it falls due and draws it full screen,
//! at the movie's own shape, centred on black. When the movie ends or is
//! skipped, the window tells the flow so
//! ([`crate::flow::Flow::movie_finished`]); when it cannot play, cannot be
//! shown or stops on a decoding error (one line on the terminal says why),
//! it tells the flow it failed ([`crate::flow::Flow::movie_failed`]), so an
//! intro that never showed is not noted as seen ([`MovieEnd`],
//! [`tell_flow`]). Either way the game goes on.
//!
//! Skipping: a movie file skips on Escape or a pad's Back or Start button;
//! a clip of the movie archive on any key, mouse click or pad button
//! ([`skips`]). Every input while a movie plays is the movie's: none of it
//! reaches the screen that follows.
//!
//! Only the game with its menus plays movies ([`movies_allowed`]): from
//! the title, or straight in a level. Replays, traces, benchmarks and the
//! tools never do, so movies change no recorded or measured run. `--movie
//! INDEX|FILE` plays one movie and ends the program ([`MOVIE_INDICES`] are
//! the indices it takes); `--no-movies` turns them off for the run.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use claw_movie::{
    ChromaSiting, FileKind, FrameLayout, Matrix, MovieError, MovieIndex, MovieSet, Player, Roots,
    SkipRule, Source, VideoFrame, locate_all, open_first,
};
use claw_render::{MoviePicture, PictureLayout};
use winit::keyboard::KeyCode;

use crate::audio::AudioBridge;
use crate::flow::{Flow, MovieShelf};
use crate::input::{GilrsSource, PadEvent, PadEventKind, PadSource};
use crate::keymap::PadButton;
use crate::settings::{Mode, Settings};

/// Rate of a movie's soundtrack when no audio engine plays it.
const SILENT_RATE: u32 = 48000;

/// The movie indices `--movie` takes: the attract clip (-2), the logo
/// (-1), the intro (0), the movies after levels 2, 4, 6, 8, 10, 12 and 13,
/// and the end of the game (14). The odd indices 1 to 11 name no movie.
pub const MOVIE_INDICES: [i8; 11] = [-2, -1, 0, 2, 4, 6, 8, 10, 12, 13, 14];

/// What `--movie` plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovieTarget {
    /// A movie of the install, by its index (-2 to 14).
    Index(MovieIndex),
    /// Any movie file.
    File(PathBuf),
}

impl MovieTarget {
    /// Reads `--movie`'s value: a whole number is an index, one of
    /// [`MOVIE_INDICES`], anything else a file (write `./3` for a file
    /// named `3`).
    pub fn parse(s: &str) -> Result<MovieTarget, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("expected a movie index or a file".into());
        }
        match s.parse::<i32>() {
            Ok(n) => i8::try_from(n)
                .ok()
                .filter(|n| MOVIE_INDICES.contains(n))
                .and_then(MovieIndex::new)
                .map(MovieTarget::Index)
                .ok_or_else(|| {
                    let valid: Vec<String> = MOVIE_INDICES.iter().map(i8::to_string).collect();
                    format!(
                        "movie index {n} names no movie: the movies are {} (or give a movie \
                         file)",
                        valid.join(", ")
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
            chroma: match frame.chroma {
                ChromaSiting::Left => claw_render::ChromaSiting::Left,
                ChromaSiting::Centre => claw_render::ChromaSiting::Centre,
            },
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

/// How a movie in the window ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovieEnd {
    /// It played to the end of its stream.
    Finished,
    /// The player skipped it.
    Skipped,
    /// It could not be shown, stopped on a decoding error, or ended before
    /// any picture showed.
    Failed,
}

/// What happened to a playing movie so far, kept by [`MoviePlayback`]
/// apart from the player so that how a movie ends follows from it alone
/// ([`MovieProgress::end`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MovieProgress {
    /// An input skipped it.
    pub skipped: bool,
    /// A picture could not be shown.
    pub failed: bool,
    /// A picture was handed over to be shown.
    pub shown: bool,
}

impl MovieProgress {
    /// How the movie ended, or `None` while it plays. `stream_over` says
    /// the player has nothing more to show; `decode_error` that decoding
    /// stopped on an error. Pictures that cannot be shown fail it; an
    /// input that skips it skips it; otherwise it finished only when its
    /// stream ended without an error after a picture showed.
    pub fn end(&self, stream_over: bool, decode_error: bool) -> Option<MovieEnd> {
        if self.failed {
            Some(MovieEnd::Failed)
        } else if self.skipped {
            Some(MovieEnd::Skipped)
        } else if !stream_over {
            None
        } else if decode_error || !self.shown {
            Some(MovieEnd::Failed)
        } else {
            Some(MovieEnd::Finished)
        }
    }
}

/// Tells the flow how the movie it asked for ended: over (played to its
/// end or skipped) or failed. Only an intro that is over is noted as seen.
pub fn tell_flow(flow: &mut Flow, end: MovieEnd) {
    match end {
        MovieEnd::Finished | MovieEnd::Skipped => flow.movie_finished(),
        MovieEnd::Failed => flow.movie_failed(),
    }
}

/// A movie playing in the window (see the module documentation).
#[derive(Debug)]
pub struct MoviePlayback {
    player: Player,
    source: Source,
    rule: SkipRule,
    progress: MovieProgress,
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
                    progress: MovieProgress::default(),
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
        let frame = self.player.next_frame()?;
        self.progress.shown = true;
        Some(picture(frame))
    }

    /// An input while the movie plays; true when it skips the movie. The
    /// window keeps every input of a playing movie from the screens,
    /// skipping or not.
    pub fn input(&mut self, event: &RawInput) -> bool {
        let skip = skips(self.rule, event);
        self.progress.skipped |= skip;
        skip
    }

    /// Ends the movie because its pictures cannot be shown: it failed
    /// ([`MovieEnd::Failed`]).
    pub fn fail(&mut self) {
        self.progress.failed = true;
    }

    /// How the movie ended, or `None` while it plays (see
    /// [`MovieProgress::end`]).
    pub fn end(&self) -> Option<MovieEnd> {
        self.progress
            .end(self.player.finished(), self.player.error().is_some())
    }

    /// Whether the movie is over, however it ended.
    pub fn done(&self) -> bool {
        self.end().is_some()
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

/// Opens a source of gamepad events, or says why there is none.
pub type PadOpener = Box<dyn FnMut() -> Result<Box<dyn PadSource>, String>>;

/// The system's gamepads for the input router, noting each button pressed
/// so the window can hand it to a playing movie (the router turns pad
/// buttons into menu inputs, which cannot tell Start from the confirm
/// button). Opened when the router opens it, at its first poll; when
/// gamepads are not available the router says so once, as for its own,
/// and reads no more.
pub struct PadTap {
    opener: PadOpener,
    pads: Option<Box<dyn PadSource>>,
    tried: bool,
    presses: Rc<RefCell<Vec<PadButton>>>,
}

impl std::fmt::Debug for PadTap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PadTap")
            .field("open", &self.pads.is_some())
            .field("tried", &self.tried)
            .field("presses", &self.presses)
            .finish_non_exhaustive()
    }
}

impl PadTap {
    /// A tap on the system's gamepads and the list it notes presses in.
    pub fn new() -> (PadTap, Rc<RefCell<Vec<PadButton>>>) {
        PadTap::with_opener(Box::new(|| {
            GilrsSource::open().map(|pads| Box::new(pads) as Box<dyn PadSource>)
        }))
    }

    /// A tap on the pads `opener` opens (a script in tests) and the list it
    /// notes presses in.
    pub fn with_opener(opener: PadOpener) -> (PadTap, Rc<RefCell<Vec<PadButton>>>) {
        let presses = Rc::new(RefCell::new(Vec::new()));
        let tap = PadTap {
            opener,
            pads: None,
            tried: false,
            presses: Rc::clone(&presses),
        };
        (tap, presses)
    }
}

impl PadSource for PadTap {
    /// Opens the gamepads, once; the router reports a failure, once, as for
    /// its own.
    fn open(&mut self) -> Result<(), String> {
        if self.tried {
            return match self.pads {
                Some(_) => Ok(()),
                None => Err("gamepad support did not start".into()),
            };
        }
        self.tried = true;
        self.pads = Some((self.opener)()?);
        Ok(())
    }

    fn next_event(&mut self) -> Option<PadEvent> {
        if !self.tried {
            let _ = self.open();
        }
        let event = self.pads.as_mut()?.next_event()?;
        if let PadEventKind::Button(button, true) = event.kind {
            self.presses.borrow_mut().push(button);
        }
        Some(event)
    }
}
