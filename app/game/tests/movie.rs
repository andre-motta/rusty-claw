//! Movies in the game: which runs play them, the logo before the title in
//! the flow (played, skipped or failing to open), the skip inputs, the
//! picture handed to the renderer, and the command line.

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use claw_assets::Palette;
use claw_frame::AudioCue;
use claw_game::flow::{Effect, FADE_MS, Flow, MovieShelf, Screen, Start};
use claw_game::keymap::PadButton;
use claw_game::movie::{RawInput, RunMode, picture, skips};
use claw_game::settings::{Args, LevelChoice, Settings};
use claw_game::{MovieTarget, movies_allowed};
use claw_movie::{FrameLayout, Matrix, MovieIndex, MovieSet, SkipRule, VideoFrame};
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
    };
    let p = picture(&rgba);
    assert_eq!((p.width, p.height), (2, 1));
    assert_eq!(p.layout, PictureLayout::Rgba);
    assert_eq!(p.planes[0], &rgba.planes[0][..]);
    assert_eq!(p.strides, [8, 0, 0]);
    for (matrix, full_range, bt709) in [(Matrix::Bt601, false, false), (Matrix::Bt709, true, true)]
    {
        let yuv = VideoFrame {
            width: 4,
            height: 2,
            pts: Duration::ZERO,
            layout: FrameLayout::I420 { matrix, full_range },
            planes: [vec![16; 8], vec![128; 2], vec![128; 2]],
            strides: [4, 2, 2],
        };
        let p = picture(&yuv);
        assert_eq!(p.layout, PictureLayout::I420 { bt709, full_range });
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
