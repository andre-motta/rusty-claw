//! Movies in the game: which runs play them, the logo before the title in
//! the flow (played, skipped or failing to open), how a movie's end reaches
//! the flow, the skip inputs, the picture handed to the renderer, the
//! gamepads while movies are on, and the command line.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use clap::Parser;
use claw_assets::Palette;
use claw_frame::AudioCue;
use claw_game::flow::{Effect, FADE_MS, Flow, MovieShelf, Screen, Start};
use claw_game::input::{InputRouter, PadEvent, PadEventKind, PadSource};
use claw_game::keymap::{Keymap, PadButton};
use claw_game::movie::{
    MOVIE_INDICES, MovieEnd, MovieProgress, PadTap, RawInput, RunMode, picture, skips, tell_flow,
};
use claw_game::settings::{Args, LevelChoice, PadSettings, Settings};
use claw_game::{MovieTarget, movies_allowed};
use claw_movie::{ChromaSiting, FrameLayout, Matrix, MovieIndex, MovieSet, SkipRule, VideoFrame};
use claw_render::PictureLayout;
use claw_ui::menu::MenuInput;
use claw_ui::menu::draw::FontArt;
use claw_ui::screens::title::TitleScreen;
use winit::keyboard::KeyCode;

fn settings(line: &str) -> Settings {
    let args =
        Args::try_parse_from(std::iter::once("rusty-claw").chain(line.split_whitespace())).unwrap();
    Settings::resolve(&args, None).unwrap()
}

#[test]
fn only_the_game_with_its_menus_plays_movies() {
    for (line, mode, allowed) in [
        ("--install /i", RunMode::Menus, true),
        ("--install /i --level 3", RunMode::Play, true),
        ("--install /i --custom Mine.WWD", RunMode::Play, true),
        ("--install /i --replay r.csv", RunMode::Watch, false),
        (
            "--install /i --replay r.csv --trace t.csv",
            RunMode::Trace,
            false,
        ),
        ("--install /i --bench 5", RunMode::WindowBench, false),
        (
            "--install /i --bench 5 --headless",
            RunMode::HeadlessBench,
            false,
        ),
        ("--install /i --movie -1", RunMode::Movie, false),
        ("--install /i --movie clip.mkv", RunMode::Movie, false),
    ] {
        let s = settings(line);
        assert_eq!(RunMode::of(&s), mode, "{line}");
        assert_eq!(movies_allowed(&mode), allowed, "{line}");
    }
}

#[test]
fn the_command_line_names_one_movie_or_none() {
    let s = settings("--install /i --movie -1");
    assert_eq!(s.movie, Some(MovieTarget::Index(MovieIndex::LOGO)));
    assert!(s.movies);
    let s = settings("--install /i --movie 14");
    assert_eq!(
        s.movie,
        Some(MovieTarget::Index(MovieIndex::new(14).unwrap()))
    );
    let s = settings("--install /i --movie /tmp/up/LOGO.mkv");
    assert_eq!(
        s.movie,
        Some(MovieTarget::File(PathBuf::from("/tmp/up/LOGO.mkv")))
    );
    let s = settings("--install /i --no-movies");
    assert!(!s.movies && s.movie.is_none());
    assert!(settings("--install /i").movies);
    for bad in [
        "--install /i --movie 15",
        "--install /i --movie -3",
        "--install /i --movie 1",
        "--install /i --movie 11",
        "--install /i --movie -1 --level 2",
        "--install /i --movie 0 --no-movies",
        "--install /i --movie 0 --replay r.csv",
    ] {
        let parsed =
            Args::try_parse_from(std::iter::once("rusty-claw").chain(bad.split_whitespace()));
        assert!(parsed.is_err(), "{bad}");
    }
}

#[test]
fn file_movies_skip_on_escape_back_or_start_and_clips_on_anything() {
    let back = [
        RawInput::Key(KeyCode::Escape),
        RawInput::Pad(PadButton::Select),
        RawInput::Pad(PadButton::Start),
    ];
    let other = [
        RawInput::Key(KeyCode::Enter),
        RawInput::Key(KeyCode::Space),
        RawInput::Click,
        RawInput::Pad(PadButton::South),
        RawInput::Pad(PadButton::East),
    ];
    for e in &back {
        assert!(skips(SkipRule::Back, e), "{e:?}");
        assert!(skips(SkipRule::AnyInput, e), "{e:?}");
    }
    for e in &other {
        assert!(!skips(SkipRule::Back, e), "{e:?}");
        assert!(skips(SkipRule::AnyInput, e), "{e:?}");
    }
}

#[test]
fn decoded_pictures_map_to_the_renderer_in_both_layouts() {
    let rgba = VideoFrame {
        width: 2,
        height: 1,
        pts: Duration::ZERO,
        layout: FrameLayout::Rgba,
        planes: [vec![1, 2, 3, 255, 4, 5, 6, 255], Vec::new(), Vec::new()],
        strides: [8, 0, 0],
        chroma: ChromaSiting::Centre,
    };
    let p = picture(&rgba);
    assert_eq!((p.width, p.height), (2, 1));
    assert_eq!(p.layout, PictureLayout::Rgba);
    assert_eq!(p.planes[0], &rgba.planes[0][..]);
    assert_eq!(p.strides, [8, 0, 0]);
    for (matrix, full_range, bt709, siting, chroma) in [
        (
            Matrix::Bt601,
            false,
            false,
            ChromaSiting::Left,
            claw_render::ChromaSiting::Left,
        ),
        (
            Matrix::Bt709,
            true,
            true,
            ChromaSiting::Centre,
            claw_render::ChromaSiting::Centre,
        ),
    ] {
        let yuv = VideoFrame {
            width: 4,
            height: 2,
            pts: Duration::ZERO,
            layout: FrameLayout::I420 { matrix, full_range },
            planes: [vec![16; 8], vec![128; 2], vec![128; 2]],
            strides: [4, 2, 2],
            chroma: siting,
        };
        let p = picture(&yuv);
        assert_eq!(
            p.layout,
            PictureLayout::I420 {
                bt709,
                full_range,
                chroma
            }
        );
        assert_eq!(p.planes[1], &[128, 128]);
        assert_eq!(p.strides, [4, 2, 2]);
    }
}

/// A flow at the title (no menus needed), with the movies of `shelf`
/// handed over before its first effects, as the window does.
struct Rig {
    flow: Flow,
    effects: Vec<Effect>,
}

impl Rig {
    fn new(shelf: Option<MovieShelf>) -> Rig {
        let settings = Settings::new("/nowhere", LevelChoice::Retail(1));
        let mut flow = Flow::new(settings, None, None, FontArt::uniform(8, 10), Start::Title);
        if let Some(shelf) = shelf {
            flow.set_movies(shelf);
        }
        let mut rig = Rig {
            flow,
            effects: Vec::new(),
        };
        rig.run();
        rig
    }

    fn run(&mut self) {
        for _ in 0..8 {
            let effects = self.flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                if e == Effect::ShowTitle {
                    self.flow
                        .set_title(TitleScreen::new(None, Palette::default(), Some(1000)));
                }
                self.effects.push(e);
            }
        }
    }

    fn update(&mut self, ms: f64) {
        self.flow.update(ms, None, None);
        self.run();
    }

    fn took(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }
}

/// The movies of a later run: the intro was seen on an earlier one, so
/// only the logo plays at start-up.
fn shelf(movies: &[MovieIndex]) -> MovieShelf {
    MovieShelf {
        available: movies.iter().copied().collect::<MovieSet>(),
        intro_seen: true,
        ..MovieShelf::default()
    }
}

#[test]
fn the_logo_plays_before_the_title_and_skipping_it_shows_the_title() {
    let mut r = Rig::new(Some(shelf(&[MovieIndex::LOGO, MovieIndex::INTRO])));
    assert_eq!(r.flow.screen(), Screen::Movie(MovieIndex::LOGO));
    assert_eq!(r.flow.playing_movie(), Some(MovieIndex::LOGO));
    let e = r.took();
    assert_eq!(
        e,
        vec![
            Effect::Sound(AudioCue::StopMusic),
            Effect::PlayMovie(MovieIndex::LOGO)
        ],
        "the logo, and no title yet"
    );
    assert!(r.flow.title().is_none());
    // Time and menu inputs do nothing while it plays, however long.
    r.flow.menu_input(MenuInput::Confirm, false);
    r.flow.menu_input(MenuInput::Back, true);
    for _ in 0..100 {
        r.update(5000.0);
    }
    assert_eq!(r.flow.screen(), Screen::Movie(MovieIndex::LOGO));
    assert!(r.took().is_empty());
    // Skipped (or over): the title, with its sound, fading in.
    r.flow.movie_finished();
    r.run();
    assert_eq!(r.flow.screen(), Screen::Title);
    assert_eq!(r.flow.playing_movie(), None);
    let e = r.took();
    assert_eq!(e.first(), Some(&Effect::ShowTitle));
    assert!(
        e.iter()
            .any(|e| matches!(e, Effect::Sound(AudioCue::Play(c)) if c.key == "TITLE")),
        "{e:?}"
    );
    assert!(r.flow.title().is_some());
    assert!(r.flow.brightness() < 0.01, "the title fades in");
    // The skip did not reach the title, and it plays out as usual: only the
    // logo plays at start-up in this run.
    r.update(FADE_MS);
    assert_eq!(r.flow.screen(), Screen::Title);
    r.update(1000.0);
    r.update(FADE_MS);
    assert_eq!(r.flow.screen(), Screen::MainMenu);
    assert!(!r.took().contains(&Effect::PlayMovie(MovieIndex::INTRO)));
    // A late call changes nothing.
    r.flow.movie_finished();
    assert_eq!(r.flow.screen(), Screen::MainMenu);
}

#[test]
fn a_logo_that_cannot_open_goes_straight_to_the_title() {
    let mut r = Rig::new(Some(shelf(&[MovieIndex::LOGO])));
    assert_eq!(r.took().last(), Some(&Effect::PlayMovie(MovieIndex::LOGO)));
    // The window could not open it and says so at once.
    r.flow.movie_finished();
    r.run();
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(r.took().contains(&Effect::ShowTitle));
}

#[test]
fn without_a_logo_or_without_movies_the_title_comes_first() {
    for shelf in [None, Some(shelf(&[])), Some(shelf(&[MovieIndex::INTRO]))] {
        let had_shelf = shelf.is_some();
        let mut r = Rig::new(shelf);
        assert_eq!(r.flow.screen(), Screen::Title);
        let e = r.took();
        assert_eq!(e.first(), Some(&Effect::ShowTitle), "{e:?}");
        assert!(!e.iter().any(|e| matches!(e, Effect::PlayMovie(_))));
        assert_eq!(r.flow.movies().is_some(), had_shelf);
    }
}

#[test]
fn movies_handed_over_after_the_title_showed_leave_it_alone() {
    let settings = Settings::new("/nowhere", LevelChoice::Retail(1));
    let mut flow = Flow::new(settings, None, None, FontArt::uniform(8, 10), Start::Title);
    let _ = flow.take_effects();
    flow.update(100.0, None, None);
    flow.set_movies(shelf(&[MovieIndex::LOGO]));
    assert_eq!(flow.screen(), Screen::Title);
    assert_eq!(flow.playing_movie(), None);
    assert!(
        !flow
            .take_effects()
            .iter()
            .any(|e| matches!(e, Effect::PlayMovie(_)))
    );
}

#[test]
fn a_level_started_from_the_command_line_plays_no_start_up_movie() {
    let settings = Settings::new("/nowhere", LevelChoice::Retail(2));
    let mut flow = Flow::new(settings, None, None, FontArt::uniform(8, 10), Start::Play);
    flow.set_movies(shelf(&[MovieIndex::LOGO, MovieIndex::INTRO]));
    assert_eq!(flow.screen(), Screen::Play);
    assert!(
        !flow
            .take_effects()
            .iter()
            .any(|e| matches!(e, Effect::PlayMovie(_)))
    );
}

#[test]
fn a_movie_only_flow_ends_at_once() {
    let settings = Settings::new("/nowhere", LevelChoice::Retail(1));
    let target = MovieTarget::Index(MovieIndex::LOGO);
    let mut flow = Flow::new(
        settings,
        None,
        None,
        FontArt::uniform(8, 10),
        Start::Movie(target),
    );
    assert_eq!(flow.screen(), Screen::Exit);
    assert_eq!(flow.take_effects(), vec![Effect::Quit]);
}

#[test]
fn the_command_line_takes_the_indices_of_the_movie_map_only() {
    for n in MovieIndex::MIN..=MovieIndex::MAX {
        let index = MovieIndex::new(n).unwrap();
        let in_map = index.fec_entry().is_some();
        assert_eq!(MOVIE_INDICES.contains(&n), in_map, "{n}");
        let parsed = MovieTarget::parse(&n.to_string());
        if in_map {
            assert_eq!(parsed, Ok(MovieTarget::Index(index)));
        } else {
            let e = parsed.unwrap_err();
            assert!(
                e.contains(&format!("movie index {n} names no movie")),
                "{e}"
            );
            assert!(e.contains("-2, -1, 0, 2, 4, 6, 8, 10, 12, 13, 14"), "{e}");
        }
    }
    assert!(MovieTarget::parse("15").is_err());
    assert_eq!(
        MovieTarget::parse("./3"),
        Ok(MovieTarget::File(PathBuf::from("./3")))
    );
    // The help lists the indices and FILE.
    let help = <Args as clap::CommandFactory>::command()
        .render_long_help()
        .to_string();
    assert!(
        help.contains("-2, -1, 0, 2, 4, 6, 8, 10, 12, 13, 14"),
        "{help}"
    );
    assert!(help.contains("FILE, any movie file"), "{help}");
}

#[test]
fn a_movie_ends_by_its_stream_by_a_skip_or_by_failing() {
    let playing = MovieProgress {
        shown: true,
        ..MovieProgress::default()
    };
    // Still playing.
    assert_eq!(playing.end(false, false), None);
    assert_eq!(MovieProgress::default().end(false, false), None);
    // Played to the end of its stream.
    assert_eq!(playing.end(true, false), Some(MovieEnd::Finished));
    // Skipped, whether a picture showed or not.
    for shown in [false, true] {
        let skipped = MovieProgress {
            skipped: true,
            shown,
            ..MovieProgress::default()
        };
        assert_eq!(skipped.end(false, false), Some(MovieEnd::Skipped));
    }
    // A picture that cannot be shown fails it, skipped or not.
    for skipped in [false, true] {
        let failed = MovieProgress {
            failed: true,
            shown: true,
            skipped,
        };
        assert_eq!(failed.end(false, false), Some(MovieEnd::Failed));
    }
    // Decoding stopped at once, with an error or with no picture at all,
    // or on an error later: failed.
    assert_eq!(
        MovieProgress::default().end(true, true),
        Some(MovieEnd::Failed)
    );
    assert_eq!(
        MovieProgress::default().end(true, false),
        Some(MovieEnd::Failed)
    );
    assert_eq!(playing.end(true, true), Some(MovieEnd::Failed));
}

/// A first run at the title with the logo and the intro, the intro now
/// playing.
fn intro_playing() -> Rig {
    let shelf = MovieShelf {
        available: [MovieIndex::LOGO, MovieIndex::INTRO]
            .into_iter()
            .collect::<MovieSet>(),
        intro_seen: false,
        skip_logo: false,
    };
    let mut r = Rig::new(Some(shelf));
    tell_flow(&mut r.flow, MovieEnd::Finished);
    r.run();
    assert_eq!(r.flow.playing_movie(), Some(MovieIndex::INTRO));
    r.took();
    r
}

#[test]
fn the_intro_is_recorded_only_when_it_played_to_its_end_or_was_skipped() {
    for end in [MovieEnd::Finished, MovieEnd::Skipped] {
        let mut r = intro_playing();
        tell_flow(&mut r.flow, end);
        r.run();
        assert_eq!(r.flow.screen(), Screen::Title, "{end:?}");
        assert!(r.took().contains(&Effect::RecordIntroSeen), "{end:?}");
        assert!(r.flow.settings().intro_seen, "{end:?}");
    }
}

#[test]
fn an_intro_that_cannot_be_shown_is_not_recorded() {
    // The pictures could not be uploaded: the window fails the movie.
    let mut r = intro_playing();
    let progress = MovieProgress {
        shown: true,
        failed: true,
        ..MovieProgress::default()
    };
    tell_flow(&mut r.flow, progress.end(false, false).unwrap());
    r.run();
    assert_eq!(r.flow.screen(), Screen::Title);
    assert!(!r.took().contains(&Effect::RecordIntroSeen));
    assert!(!r.flow.settings().intro_seen);
    assert!(!r.flow.movies().unwrap().intro_seen);
}

#[test]
fn an_intro_whose_decoding_stops_at_once_is_not_recorded() {
    for decode_error in [true, false] {
        let mut r = intro_playing();
        // The player ended before any picture showed.
        let end = MovieProgress::default().end(true, decode_error).unwrap();
        tell_flow(&mut r.flow, end);
        r.run();
        assert_eq!(r.flow.screen(), Screen::Title, "{decode_error}");
        assert!(!r.took().contains(&Effect::RecordIntroSeen));
        assert!(!r.flow.settings().intro_seen);
        assert!(!r.flow.movies().unwrap().intro_seen);
    }
}

#[test]
fn a_failed_intro_shows_the_title_as_a_missing_one_does() {
    let mut a = intro_playing();
    a.flow.movie_failed();
    a.run();
    let mut b = intro_playing();
    b.flow.movie_missing();
    b.run();
    assert_eq!(a.flow.screen(), b.flow.screen());
    assert_eq!(a.took(), b.took());
}

/// Gamepads that never open: the opener fails and counts its calls.
fn failing_tap(calls: Rc<Cell<u32>>) -> PadTap {
    let (tap, _) = PadTap::with_opener(Box::new(move || {
        calls.set(calls.get() + 1);
        Err("no gamepad support here".into())
    }));
    tap
}

#[test]
fn gamepads_that_cannot_open_under_a_movie_tap_give_the_usual_single_notice() {
    let calls = Rc::new(Cell::new(0));
    let mut router = InputRouter::with_source(
        Keymap::default(),
        PadSettings::default(),
        Box::new(failing_tap(Rc::clone(&calls))),
    );
    assert!(router.poll(Instant::now()).is_empty());
    assert_eq!(
        router.notices(),
        ["gamepads are not available (no gamepad support here); playing with the keyboard"]
    );
    // No more polling and no second notice.
    for _ in 0..5 {
        assert!(router.poll(Instant::now()).is_empty());
    }
    assert_eq!(router.notices().len(), 1);
    assert_eq!(calls.get(), 1);
}

/// Pad events the test hands to a tap.
#[derive(Clone, Default)]
struct Script(Rc<RefCell<VecDeque<PadEvent>>>);

impl PadSource for Script {
    fn next_event(&mut self) -> Option<PadEvent> {
        self.0.borrow_mut().pop_front()
    }
}

#[test]
fn a_movie_tap_on_open_gamepads_notes_presses_and_says_when_none_is_connected() {
    let script = Script::default();
    let source = script.clone();
    let mut source = Some(source);
    let (tap, presses) = PadTap::with_opener(Box::new(move || {
        source
            .take()
            .map(|s| Box::new(s) as Box<dyn PadSource>)
            .ok_or_else(|| "opened twice".to_string())
    }));
    let mut router =
        InputRouter::with_source(Keymap::default(), PadSettings::default(), Box::new(tap));
    router.poll(Instant::now());
    assert_eq!(router.notices().len(), 1);
    assert!(router.notices()[0].starts_with("no gamepad connected"));
    script.0.borrow_mut().extend([
        PadEvent {
            pad: 0,
            kind: PadEventKind::Connected("Pad".into()),
        },
        PadEvent {
            pad: 0,
            kind: PadEventKind::Button(PadButton::Start, true),
        },
    ]);
    router.poll(Instant::now());
    assert_eq!(*presses.borrow(), [PadButton::Start]);
    assert_eq!(router.notices().last().unwrap(), "gamepad connected: Pad");
}
