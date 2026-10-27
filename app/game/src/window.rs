//! The game window: keyboard and gamepads in, frames and sound out at the
//! display's refresh.
//!
//! The same window runs the game with its menus (from the title with
//! [`run_menus`], or straight in a level with [`WindowMode::Play`]),
//! watching a replayed script and the window benchmark. Every key goes
//! through the input router, which the window also polls once per frame
//! for other input devices.
//!
//! With menus, the [`Flow`] decides what shows: the title, the main menu
//! and the end-of-level screens are drawn as standalone 640x480 screens
//! with their own palettes; a level in play is drawn with its status bar,
//! the in-game message and the screen transition in progress
//! ([`StatusLayer`]), and the in-game menu is drawn over a dimmed copy of
//! the frozen level. The pause key freezes the level with a small PAUSE
//! sign and no menu, and the help screen (F1) shows the controls picture
//! (see [`crate::pause`]).
//! The window loads levels when the flow asks, plays the sound cues of
//! every tick it runs and applies the options changed in the menus at once
//! (the output size, the aspect policy, the scaling, fullscreen, vsync,
//! the frame-rate cap, interpolation and the key bindings).
//!
//! Watching a script and the benchmark show one level without menus; the
//! benchmark reads the keyboard only. A watched script plays the game's
//! sound and music as play does, from the level's start through the
//! handover to the keyboard, and its sounds pause where the recording
//! paused (see [`crate::pause::ReplayedHolds`]); with sound off
//! (`--no-audio`, `audio = off`) it opens no audio device. The benchmark
//! is silent. The window title shows Claw's health, score and lives.
//!
//! With `--record`, every level played is recorded into its own file, one
//! after the other as the game goes from level to level (see
//! [`crate::session::GameRecording`]); each file is written when its level
//! is left or the window closes, and says how its level started (the
//! progress carried in, the save point, or the `--start` point). Watching
//! a recording plays its level from that start.
//!
//! The end-of-level screens of a gem level animate the level's gem in its
//! socket on the amulet, from the level's gem animation.
//!
//! With vsync on, waiting for the display paces the frames. Otherwise each
//! frame first waits for the frame-rate cap (300 frames per second unless
//! set otherwise), so the game never draws thousands of frames a second.
//!
//! Movies (see [`crate::movie`]): with its menus the window plays the
//! movies the flow asks for, full screen with their sound, from the
//! folders and sources the movie settings name
//! ([`Settings::movie_roots`]), and [`run_movie`] opens it for one movie
//! only. It hands the flow the movies it found, with whether the intro was
//! seen and the logo is to be skipped ([`crate::flow::MovieShelf`]), unless
//! movies are off (`--no-movies`, `movies = off`). A playing movie takes
//! every input but the keys of the window's own commands (fullscreen,
//! window size, aspect, scaling), which run them; the screen after it sees
//! none of them. In the background (once the window has had the focus) a
//! playing movie holds and the main menu's idle time stands still ([`MoviePlayback::hold`],
//! [`Flow::set_background`]). A `--movie` run whose movie did not play or
//! could not be shown ends with an error.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use claw_assets::{Frame, Install, Palette};
use claw_audio::AudioSettings;
use claw_audio::music::Track;
use claw_frame::{
    AudioCue, DrawMode, FrameClock, FramePacer, GameEvent, PaletteFade, ScreenTransition, TICK_MS,
};
use claw_movie::{MovieIndex, Roots, locate_all};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::palette::{self, FULL_BRIGHTNESS};
use claw_render::screen::transition_at;
use claw_render::{
    Anchor, FrameInput, Gpu, ImageId, ImageSource, LevelGraphics, MoviePresenter, Offscreen,
    Renderer, Scene, SceneBuilder, ScreenBuilder, ScreenScene, ScreenSprite, wgpu,
};
use claw_sim::input::MenuScreen;
use claw_sim::logic::enemy::crab::CrabValues;
use claw_sim::player::powerup_aid::AidPowerup;
use claw_sim::{Sim, SimConfig};
use claw_ui::hud::{FONT, Hud, HudArt, marker_layout, marker_lines};
use claw_ui::menu::command::{MenuKind, Page, VideoSetting};
use claw_ui::menu::decor::{SPARKLE, Sparkles};
use claw_ui::menu::draw::{self, FontArt, MenuSprite};
use claw_ui::menu::{CommandCatalogue, MenuTree};
use claw_ui::screens::HelpScreen;
use claw_ui::screens::assets::{
    BOOTY_SCREENS, HELP_PICTURE, HELP_SCREENS, MENU_PICTURE, MENU_SCREENS, ScreenAssets,
    TITLE_PICTURE, TITLE_SCREENS, picture_frame,
};
use claw_ui::screens::booty::{self, BootyArt, BootyScreen, BootyStats};
use claw_ui::screens::credits::{
    CREDITS_PICTURE, CREDITS_SCREENS, CreditsArt, CreditsEntry, CreditsScreen, FontMetrics,
    MESSAGE_FONT, STRIPS, TEXT_FONT, prepare_parchment, recolour,
};
use claw_ui::screens::title::{self, TitleScreen};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::{AudioBridge, ViewInfo};
use crate::bench::{Combat, PresentDetail, Report, Samples, Summary, WARMUP_S, combat_route};
use crate::flow::{self, Effect, Flow, MovieShelf, Screen, Start};
use crate::input::{InputContext, InputRouter, Routed};
use crate::keymap::{Action, Command, PadButton};
use crate::load::{
    LevelBundle, SHADE_SET, SHADE_SIZE, load_graphics, load_level, preload, start_override,
};
use crate::movie::{
    self, MovieKey, MoviePlayback, MovieTarget, PadTap, RawInput, RunMode, movies_allowed,
};
use crate::pause::{Change, GameKey, KeyContext, PauseInput, Pauses, ReplayedHolds, game_key};
use crate::saves::SaveStore;
use crate::session::{
    Circles, GameRecording, HoldChange, LevelRecording, Session, Span, start_sim,
    start_sim_with_crabs,
};
use crate::settings::{Aspect, Mode, Scale, Settings, UserDirs, WINDOW_SIZES, view_for};

/// The image set of the PAUSE sign (its frame [`PAUSE_FRAME`]), preloaded
/// with every level.
pub const PAUSE_SET: &str = "GAME_MESSAGES";
/// The PAUSE sign's frame.
pub const PAUSE_FRAME: i32 = 2;

/// The PAUSE sign shown while the pause key holds play, in the middle of
/// the picture.
pub fn pause_sign() -> ScreenSprite {
    ScreenSprite::new(PAUSE_SET, PAUSE_FRAME, Anchor::Center, 0, 0)
}

/// The screen layer of a game in progress: the status bar (with a small
/// marker naming the play-test aids that are on, if any), the message on
/// screen and the screen transition, for each frame drawn.
///
/// Messages age with game time only: by the ticks that run, and not while a
/// transition runs (or while paused, when no tick runs). They are hidden
/// while a transition runs (see [`transition_running`]) and while a death's
/// circles are drawn between two ticks ([`StatusLayer::circles`]), which
/// are timed by the frames instead of the ticks.
#[derive(Debug, Clone)]
pub struct StatusLayer {
    hud: Hud,
    transition: Option<ScreenTransition>,
    running: bool,
    fresh: bool,
    circles: Option<Circles>,
}

/// True while `t` runs at tick `tick`: a closing transition until the next
/// one replaces it, an opening one until its length has passed.
pub fn transition_running(t: &ScreenTransition, tick: u64) -> bool {
    if t.closing {
        return true;
    }
    let elapsed = tick.saturating_sub(t.start_tick) as f64 * f64::from(TICK_MS);
    elapsed < f64::from(t.duration_ms)
}

impl StatusLayer {
    /// Measures the status bar art of `bundle`'s level, preloading its image
    /// sets into `graphics`.
    pub fn new(graphics: &mut LevelGraphics, bundle: &LevelBundle) -> StatusLayer {
        let art = HudArt::from_store(graphics, &bundle.store, &bundle.level.registries);
        if !art.missing.is_empty() {
            eprintln!(
                "rusty-claw: status bar art not found: {}",
                art.missing.join(", ")
            );
        }
        StatusLayer {
            hud: Hud::new(art),
            transition: None,
            running: false,
            fresh: true,
            circles: None,
        }
    }

    /// Draws a death's circles as `circles` says from now on (see
    /// [`Session::circles`]), or the simulation's transition with `None`.
    pub fn circles(&mut self, circles: Option<Circles>) {
        self.circles = circles;
    }

    /// Keeps the status bar inside the centred 4:3 area (true) or at the
    /// picture's edges.
    pub fn set_centred(&mut self, centred: bool) {
        self.hud.placement.safe_area = centred;
    }

    /// Takes the game events of the ticks just run (the messages among
    /// them).
    pub fn events(&mut self, events: &[GameEvent]) {
        for e in events {
            self.hud.messages.event(e);
        }
    }

    /// Follows the simulation after a frame that ran `ticks` ticks.
    pub fn update(&mut self, sim: &Sim, ticks: u32) {
        if ticks > 0 || self.fresh {
            self.fresh = false;
            self.transition = sim.transition();
            self.running = self
                .transition
                .is_some_and(|t| transition_running(&t, sim.tick()));
        }
        if ticks > 0 && !self.running {
            self.hud
                .messages
                .advance(f64::from(ticks) * f64::from(TICK_MS));
        }
    }

    /// The screen transition in progress.
    pub fn transition(&self) -> Option<ScreenTransition> {
        self.transition
    }

    /// Names powerup `p`, just given with the play-test key, on screen for
    /// [`POWERUP_MESSAGE_MS`] of game time, as a message.
    pub fn show_powerup(&mut self, p: AidPowerup) {
        self.hud.messages.show(p.label(), POWERUP_MESSAGE_MS);
    }

    /// The message on screen, if any.
    pub fn message(&self) -> Option<&str> {
        self.hud.messages.text()
    }

    /// Fills the screen layer and the transition of `input`, a frame of
    /// `sim` in a presentation of `presentation` game pixels.
    pub fn apply(&mut self, input: &mut FrameInput<'_>, sim: &Sim, presentation: (u32, u32)) {
        input.screen = self.hud.layout(&sim.hud(), presentation, input.time_ms);
        // A small marker while a play-test aid is on.
        let lines = aid_marker_lines(sim.config());
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        input.screen.extend(marker_layout(
            self.hud.art(),
            &lines,
            presentation,
            self.hud.placement,
        ));
        if let Some(c) = self.circles {
            // No game time passes: the frame's time moves on with the
            // circles instead.
            let (t, time) = transition_at(&c.transition, input.time_ms, c.elapsed_ms);
            input.transition = Some(t);
            input.time_ms = time;
            return;
        }
        if !self.running {
            input.screen.extend(self.hud.message_layout());
        }
        input.transition = self.transition;
    }
}

/// How long the name of a powerup given with the play-test key stays on
/// screen, ms of game time.
pub const POWERUP_MESSAGE_MS: u32 = 2000;

/// The lines of the play-test aid marker for a level played with `cfg`:
/// `INVINCIBLE`, `INFINITE LIVES` and, with the powerup aid, `POWERUP`
/// and the powerup's name (`POWERUP FIRE SWORD`), each when on; none with
/// every aid off.
pub fn aid_marker_lines(cfg: &SimConfig) -> Vec<String> {
    let mut lines: Vec<String> = marker_lines(cfg.aids.invincible, cfg.aids.infinite_lives)
        .into_iter()
        .map(str::to_string)
        .collect();
    if let Some(p) = cfg.start_powerup {
        lines.push(format!("POWERUP {}", p.label()));
    }
    lines
}

/// What the window does.
#[derive(Debug, Clone)]
pub enum WindowMode {
    /// The game with its menus, straight in the level given.
    Play,
    /// Play a level's recording from the start it names, then hand over to
    /// the keyboard. The settings should already follow it
    /// ([`Settings::follow_recording`]): its level, start point and
    /// switches.
    Watch(Box<LevelRecording>),
    /// Run the benchmark route for this many seconds after the warm-up.
    Bench(f64),
}

/// The sound of a window without menus: a watched script plays the game's
/// sound and music as play does, unless sound is off (`--no-audio`,
/// `audio = off`), when no audio device opens; the window benchmark is
/// silent. A replay that writes a trace runs without a window and plays
/// nothing. (With menus the window always has sound, which the menus can
/// turn on and off.)
pub fn level_audio(mode: &WindowMode, settings: &Settings) -> Option<AudioSettings> {
    match (mode, &settings.mode) {
        (_, Mode::Replay { trace: Some(_), .. }) => None,
        (WindowMode::Watch(_), _) if settings.audio => Some(settings.audio_settings()),
        _ => None,
    }
}

/// Loads the sounds and music of `bundle` into `audio`, when there is
/// sound, and says how it went.
fn load_sounds(audio: Option<&mut AudioBridge>, bundle: &LevelBundle) {
    let Some(audio) = audio else { return };
    let r = audio.load_level(&bundle.store, &bundle.level.registries);
    eprintln!(
        "rusty-claw: {} of {} sounds ready in {:.0} ms ({:.1} MB)",
        r.decoded,
        r.registered,
        r.ms,
        r.bytes as f64 / 1_048_576.0
    );
    for (key, why) in r.failed.iter().take(5) {
        eprintln!("rusty-claw: sound {key} left out: {why}");
    }
}

struct Live {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    renderer: Renderer,
    builder: Option<SceneBuilder>,
    screen_renderer: Renderer,
    screen_builder: ScreenBuilder,
    /// Benchmark only: a second copy of every frame drawn offscreen at the
    /// requested size, when the window could not get that size.
    offscreen: Option<OffscreenCopy>,
    /// Draws movie pictures, made for the first movie.
    movie: Option<MoviePresenter>,
}

/// Every frame drawn again, offscreen, at the benchmark's requested size.
struct OffscreenCopy {
    renderer: Renderer,
    builder: SceneBuilder,
    output: Offscreen,
}

#[derive(Debug, Default)]
struct BenchState {
    seconds: f64,
    measuring_since: Option<Instant>,
    images_at_start: usize,
    samples: Samples,
    last_present: Option<Instant>,
    route_ticks: usize,
    route_kind: &'static str,
    route_start: Option<(i32, i32)>,
    combat: Combat,
}

/// A level being played.
struct Level {
    bundle: LevelBundle,
    graphics: LevelGraphics,
    session: Session,
    status: StatusLayer,
}

/// The art of the standalone screens.
struct Screens {
    assets: ScreenAssets,
    graphics: LevelGraphics,
    menu: Option<(ImageId, Palette)>,
    title: Option<(ImageId, Palette)>,
    pictures: BTreeMap<String, (ImageId, Palette)>,
    sparkles: Sparkles,
    /// The credits' art, prepared the first time they show.
    credits: Option<CreditsArt>,
}

impl Screens {
    fn load(install: &std::path::Path) -> Result<Screens> {
        let store = Install::open(install)
            .with_context(|| format!("opening the install at {}", install.display()))?
            .store()
            .context("reading the install's assets")?;
        let assets = ScreenAssets::load(store);
        for m in &assets.missing {
            eprintln!("rusty-claw: menu art not found: {m}");
        }
        let menu_pic = assets.picture(MENU_SCREENS, MENU_PICTURE);
        let palette = menu_pic
            .as_ref()
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        let mut graphics = LevelGraphics::for_screens(palette, Box::new(assets.images()));
        let menu = menu_pic.map(|(img, pal)| (graphics.add_image(&picture_frame(&img)), pal));
        let title = assets
            .picture(TITLE_SCREENS, TITLE_PICTURE)
            .map(|(img, pal)| (graphics.add_image(&picture_frame(&img)), pal));
        graphics.preload(draw::extra_sets(MenuKind::Main));
        graphics.preload([SPARKLE, "DIALOGS_CONFIGCONTROLS"]);
        let sparkles = Sparkles::new(graphics.frame_numbers(SPARKLE), 0x5EED);
        Ok(Screens {
            assets,
            graphics,
            menu,
            title,
            pictures: BTreeMap::new(),
            sparkles,
            credits: None,
        })
    }

    fn picture(&mut self, folder: &str, name: &str) -> (Option<ImageId>, Palette) {
        let key = format!("{folder}/{name}");
        if let Some((id, pal)) = self.pictures.get(&key) {
            return (Some(*id), pal.clone());
        }
        match self.assets.picture(folder, name) {
            Some((img, pal)) => {
                let id = self.graphics.add_image(&picture_frame(&img));
                self.pictures.insert(key, (id, pal.clone()));
                (Some(id), pal)
            }
            None => {
                eprintln!("rusty-claw: picture {folder}/{name} not found");
                (None, self.graphics.palette.clone())
            }
        }
    }

    fn help(&mut self) -> HelpScreen {
        let (bg, pal) = self.picture(HELP_SCREENS, HELP_PICTURE);
        HelpScreen::new(bg, pal)
    }

    fn title(&self) -> TitleScreen {
        let (bg, pal) = match &self.title {
            Some((id, pal)) => (Some(*id), pal.clone()),
            None => (None, Palette::default()),
        };
        TitleScreen::new(bg, pal, self.assets.sound_ms(title::TITLE_SOUND))
    }

    fn booty(&mut self, stats: BootyStats) -> BootyScreen {
        self.graphics.preload(BootyScreen::image_sets(stats.level));
        let map = self.picture(BOOTY_SCREENS, &booty::map_picture(stats.level));
        let tally = self.picture(
            BOOTY_SCREENS,
            &booty::tally_picture(stats.level, stats.custom),
        );
        let half = |g: &mut LevelGraphics, f: i32, d: i32| {
            g.frame_info(booty::SYMBOLS, f)
                .map_or(d, |i| i.width as i32 / 2 + 14)
        };
        let art = BootyArt {
            map,
            tally,
            sound_ms: self.assets.sound_ms.clone(),
            digit_width: self
                .graphics
                .frame_info(booty::DIGITS, 0)
                .map_or(12, |f| f.width as i32),
            total_offsets: (
                half(&mut self.graphics, 4, 60),
                half(&mut self.graphics, 5, 64),
            ),
        };
        let gem = booty::gem_frames_from(&self.assets, stats.level);
        if !stats.custom && booty::gem_animation(stats.level).is_some() && gem.is_empty() {
            eprintln!(
                "rusty-claw: the gem animation of level {} was not found",
                stats.level
            );
        }
        BootyScreen::new(stats, art).with_gem_frames(gem)
    }

    /// The credits opened from `entry`; `tune_ms` is the length of the
    /// developer's tune when it can play.
    fn credits(&mut self, entry: CreditsEntry, tune_ms: Option<f64>) -> CreditsScreen {
        let mut art = match self.credits.clone() {
            Some(art) => art,
            None => {
                let art = self.credits_art();
                self.credits = Some(art.clone());
                art
            }
        };
        art.tune_ms = tune_ms;
        let text = self.assets.credits_text().unwrap_or_else(|| {
            eprintln!("rusty-claw: the credits text was not found");
            String::new()
        });
        CreditsScreen::new(entry, &text, art)
    }

    /// The parchment, its strips and the text's fonts, added to the
    /// screens' graphics.
    fn credits_art(&mut self) -> CreditsArt {
        let metrics = FontMetrics::from_art(&mut self.graphics);
        let art = match self.assets.picture(CREDITS_SCREENS, CREDITS_PICTURE) {
            Some((img, pal)) => {
                let p = prepare_parchment(&img, &pal);
                let id = self.graphics.add_image(&p.picture);
                self.graphics.insert_set(STRIPS, &p.strips);
                CreditsArt {
                    picture: Some(id),
                    strips: p.strips.len() as i32,
                    palette: p.palette,
                    text_index: p.text_index,
                    message_index: p.message_index,
                    metrics,
                    tune_ms: None,
                }
            }
            None => {
                eprintln!("rusty-claw: picture {CREDITS_SCREENS}/{CREDITS_PICTURE} not found");
                CreditsArt::plain(metrics)
            }
        };
        let font = self.assets.images().image_set(FONT).unwrap_or_default();
        for (key, index) in [
            (TEXT_FONT, art.text_index),
            (MESSAGE_FONT, art.message_index),
        ] {
            let frames: Vec<(i32, Frame)> = font
                .iter()
                .map(|f| (f.number, recolour(&f.frame, index)))
                .collect();
            self.graphics.insert_set(key, &frames);
        }
        art
    }
}

/// The tiles behind the controls list: the dialog stone, 4 by 3 tiles of
/// 128 pixels centred on the canvas.
fn controls_panel(set: &str) -> Vec<ScreenSprite> {
    let mut out = Vec::new();
    for row in 0..3 {
        for col in 0..4 {
            out.push(ScreenSprite::new(
                set,
                1,
                Anchor::TopLeft,
                64 + 64 + 128 * col,
                48 + 64 + 128 * row,
            ));
        }
    }
    out
}

/// The shade over a whole presentation of `presentation` game pixels,
/// drawn translucent: it dims the level (and its status bar) under the
/// in-game menu, whose entries are drawn after it at full brightness.
pub fn shade_sprites(presentation: (u32, u32)) -> Vec<ScreenSprite> {
    let step = SHADE_SIZE as i32;
    let (w, h) = (presentation.0 as i32, presentation.1 as i32);
    let mut out = Vec::new();
    // A square more on every side: the picture may overhang the output.
    let mut y = -step;
    while y < h + step {
        let mut x = -step;
        while x < w + step {
            out.push(ScreenSprite {
                mode: DrawMode::GHOST,
                ..ScreenSprite::new(SHADE_SET, -1, Anchor::TopLeft, x + step / 2, y + step / 2)
            });
            x += step;
        }
        y += step;
    }
    out
}

/// A notice in the bitmap font at the top of the canvas.
fn notice_sprites(art: &mut dyn draw::Art, text: &str) -> Vec<ScreenSprite> {
    let (w, _) = draw::text_size(art, text);
    draw::text_sprites(art, text, Anchor::TopLeft, 320 - w / 2, 8)
}

/// The point of the 640x480 screen under the window pixel `at`, for the
/// screens' composition `c` (outside the picture it lies off the screen).
pub fn canvas_point(c: &claw_render::Composition, at: (f64, f64)) -> (i32, i32) {
    let w = f64::from(c.width.max(1));
    let h = f64::from(c.height.max(1));
    let x = (at.0 - f64::from(c.x)) * f64::from(c.presentation.0) / w;
    let y = (at.1 - f64::from(c.y)) * f64::from(c.presentation.1) / h;
    (x.floor() as i32, y.floor() as i32)
}

/// A palette fade holding `brightness` (0 to 1).
fn steady_fade(brightness: f64) -> Option<PaletteFade> {
    if brightness >= 1.0 {
        return None;
    }
    let level = (brightness.clamp(0.0, 1.0) * f64::from(FULL_BRIGHTNESS)).round() as u32;
    Some(PaletteFade {
        from: level,
        to: level,
        start_tick: 0,
        duration_ms: 1,
    })
}

/// Dims a built scene to `brightness`.
fn dim(scene: &mut Scene, brightness: f64) {
    if let Some(f) = steady_fade(brightness) {
        scene.palette = palette::faded(&scene.palette, Some(&f), 0.0);
        scene.brightness *= brightness as f32;
    }
}

/// The letter or digit of a key, for menu hotkeys.
fn key_char(code: KeyCode) -> Option<char> {
    use KeyCode as K;
    let letters = [
        K::KeyA,
        K::KeyB,
        K::KeyC,
        K::KeyD,
        K::KeyE,
        K::KeyF,
        K::KeyG,
        K::KeyH,
        K::KeyI,
        K::KeyJ,
        K::KeyK,
        K::KeyL,
        K::KeyM,
        K::KeyN,
        K::KeyO,
        K::KeyP,
        K::KeyQ,
        K::KeyR,
        K::KeyS,
        K::KeyT,
        K::KeyU,
        K::KeyV,
        K::KeyW,
        K::KeyX,
        K::KeyY,
        K::KeyZ,
    ];
    let digits = [
        K::Digit0,
        K::Digit1,
        K::Digit2,
        K::Digit3,
        K::Digit4,
        K::Digit5,
        K::Digit6,
        K::Digit7,
        K::Digit8,
        K::Digit9,
    ];
    if let Some(i) = letters.iter().position(|&k| k == code) {
        return Some(char::from(b'A' + i as u8));
    }
    digits
        .iter()
        .position(|&k| k == code)
        .map(|i| char::from(b'0' + i as u8))
}

/// A scene builder for `graphics` presented as `view`, drawing the front
/// plane or not.
fn scene_builder(
    graphics: &LevelGraphics,
    view: claw_frame::ViewConfig,
    max: u32,
    front_layer: bool,
) -> SceneBuilder {
    let mut b = SceneBuilder::new(graphics, view, max);
    b.set_front_layer(front_layer);
    b
}

/// The name a menu input is recorded under.
fn menu_input_name(m: crate::input::MenuInput) -> &'static str {
    use crate::input::MenuInput as M;
    match m {
        M::Up => "up",
        M::Down => "down",
        M::Left => "left",
        M::Right => "right",
        M::Confirm => "confirm",
        M::Back => "back",
    }
}

struct App {
    settings: Settings,
    mode: WindowMode,
    level: Option<Level>,
    screens: Option<Screens>,
    flow: Option<Flow>,
    router: InputRouter,
    audio: Option<AudioBridge>,
    /// With `--record`: the recording of every level played.
    recording: Option<GameRecording>,
    /// Levels started so far.
    levels_started: u32,
    /// The level loaded before the window opened, for the first start.
    first_level: Option<(LevelBundle, LevelGraphics)>,
    /// The text the title shows.
    shown: Option<String>,
    /// Sound cues of the ticks of the current frame, played after it.
    cues: Vec<AudioCue>,
    /// The replayed pauses and menus that started or ended in the current
    /// frame, followed before its cues play.
    holds: Vec<HoldChange>,
    /// What the replayed pauses and menus did to the sounds so far.
    replayed: ReplayedHolds,
    live: Option<Live>,
    clock: FrameClock,
    pacer: FramePacer,
    pacing: String,
    /// Window events since the last present that reallocate drawing
    /// resources, for the benchmark's missed-refresh detail.
    events: Vec<&'static str>,
    last_frame: Instant,
    aspect: Aspect,
    scale: Scale,
    interp: bool,
    pauses: Pauses,
    focused: bool,
    /// The window has had the focus at least once. A window that never
    /// had it (a display without a window manager) is not in the
    /// background for movies and the idle time.
    focused_once: bool,
    /// Where the mouse pointer is in the window, in pixels.
    cursor: Option<(f64, f64)>,
    /// Shift, Ctrl and Alt as last reported.
    modifiers: ModifiersState,
    size_index: usize,
    started: Instant,
    load_ms: f64,
    preloaded: usize,
    display_hz: Option<f64>,
    first_output: (u32, u32),
    bench: Option<BenchState>,
    report: Option<Report>,
    error: Option<anyhow::Error>,
    announced_end: bool,
    /// Where movies are looked for.
    movie_roots: Roots,
    /// The movie playing.
    movie: Option<MoviePlayback>,
    /// The one movie of a `--movie` run.
    movie_only: Option<MovieTarget>,
    /// The pad buttons pressed since the last frame, for a playing movie
    /// (see [`PadTap`]).
    pad_presses: Option<Rc<RefCell<Vec<PadButton>>>>,
}

/// Says when widened activation follows the first output's shape.
fn announce_widening(settings: &Settings, first_output: (u32, u32), sim: &Sim) {
    if settings.widens() {
        let (w, h) = sim.config().activation.view;
        eprintln!(
            "rusty-claw: widened activation follows the first output's shape \
             ({}x{}): {w}x{h} game pixels stay active until the level is loaded again",
            first_output.0, first_output.1
        );
    }
}

/// Records a window event once until the next present.
fn note(events: &mut Vec<&'static str>, event: &'static str) {
    if !events.contains(&event) {
        events.push(event);
    }
}

fn present_mode(modes: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    use wgpu::PresentMode as P;
    let prefs: &[P] = if vsync {
        &[P::Fifo, P::Mailbox]
    } else {
        &[P::Immediate, P::Mailbox, P::Fifo]
    };
    prefs
        .iter()
        .copied()
        .find(|m| modes.contains(m))
        .unwrap_or(P::Fifo)
}

impl App {
    fn title(&self) -> String {
        let Some(level) = &self.level else {
            return "rusty-claw".to_string();
        };
        let s = level.session.sim().stats();
        let mut t = format!(
            "rusty-claw: {} | health {} | score {} | lives {}",
            level.bundle.name, s.health, s.score, s.lives
        );
        if self.pauses.user || self.pauses.menu {
            t.push_str(" (paused)");
        }
        t
    }

    /// Sets the window title when the text changed.
    fn update_title(&mut self) {
        let title = self.title();
        if self.shown.as_ref() != Some(&title) {
            if let Some(live) = &self.live {
                live.window.set_title(&title);
            }
            self.shown = Some(title);
        }
    }

    /// The frame pacing for a surface presenting with `mode`.
    fn pacing_for(&mut self, mode: wgpu::PresentMode) {
        // Waiting for the display paces the frames; any other mode is held
        // to the frame-rate cap.
        let (pacer, pacing) = if mode == wgpu::PresentMode::Fifo {
            (FramePacer::uncapped(), "display refresh".to_string())
        } else {
            match self.settings.fps_cap {
                Some(n) => (FramePacer::capped(Some(n)), format!("{n} fps cap")),
                None => (FramePacer::uncapped(), "none (throughput run)".to_string()),
            }
        };
        self.pacer = pacer;
        self.pacing = pacing;
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let (w, h) = self.settings.res;
        let mut attrs = Window::default_attributes()
            .with_title(self.title())
            .with_inner_size(PhysicalSize::new(w, h));
        if self.settings.fullscreen {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .context("creating the window")?,
        );
        let instance = Gpu::instance();
        let surface = instance
            .create_surface(window.clone())
            .context("creating the window surface")?;
        let gpu = Gpu::new(instance, Some(&surface)).context("opening the GPU")?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .context("the window surface offers no format")?;
        let size = window.inner_size();
        let max = gpu.max_target();
        if size.width > max || size.height > max {
            bail!(
                "the window is {}x{}, larger than this GPU's largest image ({max} pixels a side)",
                size.width,
                size.height
            );
        }
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("the window surface is not supported by the GPU")?;
        config.format = format;
        config.present_mode = present_mode(&caps.present_modes, self.settings.vsync);
        surface.configure(&gpu.device, &config);
        self.pacing_for(config.present_mode);

        // The simulation starts now that the output size is known: widened
        // activation follows the first output's shape.
        let surface_size = (config.width, config.height);
        // A fullscreen window may not have its final size yet; the monitor
        // tells the shape it will have.
        self.first_output = if self.settings.fullscreen {
            window
                .current_monitor()
                .map(|m| m.size())
                .filter(|s| s.width > 0 && s.height > 0)
                .map_or(surface_size, |s| (s.width, s.height))
        } else {
            surface_size
        };
        let view = view_for(
            self.aspect,
            self.scale,
            self.settings.widen_activation,
            surface_size,
        );
        let screen_builder = ScreenBuilder::new(&view, max);
        let mut live = Live {
            window,
            surface,
            config,
            renderer: Renderer::new(&gpu),
            builder: None,
            screen_renderer: Renderer::new(&gpu),
            screen_builder,
            gpu,
            offscreen: None,
            movie: None,
        };
        if let Some(s) = &self.screens {
            live.screen_renderer.set_level(&s.graphics);
        }
        self.display_hz = live
            .window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|&mhz| mhz > 0)
            .map(|mhz| f64::from(mhz) / 1000.0);

        if self.flow.is_none() && self.movie_only.is_none() {
            // Watching a script or the benchmark: one level, no menus.
            let first_output = self.first_output;
            let Some((bundle, mut graphics)) = self.first_level.take() else {
                bail!("no level to show");
            };
            // A watched recording starts as it was recorded: with the
            // progress carried in, at its save point or its start point.
            // Its hermit crabs start with the values its header names.
            let (start, crabs) = match &self.mode {
                WindowMode::Watch(rec) => (rec.start.clone(), rec.crabs.clone()),
                _ => (claw_sim::StartState::default(), CrabValues::default()),
            };
            let sim = start_sim_with_crabs(
                bundle.sim_level.clone(),
                self.settings.sim_config(first_output),
                start.clone(),
                self.settings.start,
                &crabs,
            );
            // The benchmark plays the level's combat route, planned on a
            // copy of the game (from the given start point when there is
            // one).
            let bench_route = match &self.mode {
                WindowMode::Bench(seconds) => {
                    let seconds = (seconds + WARMUP_S) * 1.25 + 2.0;
                    Some(combat_route(&sim, seconds, self.settings.start.is_some()))
                }
                _ => None,
            };
            announce_widening(&self.settings, first_output, &sim);
            self.preloaded = preload(&mut graphics, &bundle.level, &sim.snapshot());
            let mut status = StatusLayer::new(&mut graphics, &bundle);
            status.set_centred(self.settings.status_bar_centred);
            graphics.preload([PAUSE_SET]);
            let mut session = Session::new(sim)
                .with_pause_input(self.settings.pause_input)
                .with_death_circles();
            session.set_smooth_rope(self.settings.smooth_rope);
            self.router
                .set_keep_held(self.settings.pause_input == PauseInput::Original);
            if let Some(path) = &self.settings.record
                && !matches!(self.mode, WindowMode::Bench(_))
            {
                let mut recording = GameRecording::new(path);
                recording.start_from(
                    &mut session,
                    flow::retail_number(&self.settings.level),
                    start,
                    self.settings.start,
                );
                self.recording = Some(recording);
            }
            match &self.mode {
                WindowMode::Play => {}
                WindowMode::Watch(rec) => {
                    session = session
                        .with_replay(rec.script.clone())
                        .with_crab_values(crabs);
                }
                WindowMode::Bench(seconds) => {
                    let r = bench_route.expect("made for the benchmark");
                    session = session.with_replay(r.script.clone());
                    session.set_before_tick(r.mover());
                    self.bench = Some(BenchState {
                        seconds: *seconds,
                        route_ticks: r.script.ticks.len(),
                        route_kind: r.kind,
                        route_start: self.settings.start.or(r.start()),
                        ..BenchState::default()
                    });
                }
            }
            // A watched script's level music starts as the level shows.
            load_sounds(self.audio.as_mut(), &bundle);
            let level = self.level.insert(Level {
                bundle,
                graphics,
                session,
                status,
            });
            live.renderer.set_level(&level.graphics);
            live.builder = Some(scene_builder(
                &level.graphics,
                view,
                max,
                self.settings.front_layer,
            ));
            level.session.set_presentation(view);
            let want = self.settings.res;
            if self.bench.is_some() && !self.settings.fullscreen && surface_size != want {
                // The desktop cannot hold a window of the requested size (a
                // compositor keeps windows inside the screen). Draw each frame
                // a second time offscreen at that size, so the measured work
                // and the refresh pacing include the full-size frame.
                if want.0 > max || want.1 > max {
                    bail!(
                        "--res {}x{} is larger than this GPU's largest image ({max} pixels a side)",
                        want.0,
                        want.1
                    );
                }
                eprintln!(
                    "rusty-claw: the window got {}x{} instead of {}x{}; every frame is also drawn \
                     offscreen at {}x{} and counted in the frame work",
                    surface_size.0, surface_size.1, want.0, want.1, want.0, want.1
                );
                let mut renderer = Renderer::new(&live.gpu);
                renderer.set_level(&level.graphics);
                let view = view_for(
                    self.aspect,
                    self.scale,
                    self.settings.widen_activation,
                    want,
                );
                live.offscreen = Some(OffscreenCopy {
                    renderer,
                    builder: scene_builder(&level.graphics, view, max, self.settings.front_layer),
                    output: Offscreen::new(&live.gpu, want),
                });
            }
        }
        self.load_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "rusty-claw: {} at {}x{} on {}, {:?}, paced by {}, ready in {:.0} ms",
            match (&self.level, &self.movie_only) {
                (Some(l), _) => l.bundle.name.as_str(),
                (None, Some(_)) => "a movie",
                (None, None) => "menus",
            },
            live.config.width,
            live.config.height,
            live.gpu.describe(),
            live.config.present_mode,
            self.pacing,
            self.load_ms,
        );
        if !matches!(self.mode, WindowMode::Bench(_)) && self.movie_only.is_none() {
            eprintln!("rusty-claw: {}", self.router.help());
        }
        self.live = Some(live);
        self.clock.reset();
        if let Some(target) = self.movie_only.clone() {
            let candidates = target.candidates(&self.movie_roots);
            self.movie = MoviePlayback::start(&candidates, self.audio.as_mut());
            match &self.movie {
                Some(m) => eprintln!("rusty-claw: playing {}", m.source().describe()),
                None => bail!("{} did not play", target.name()),
            }
            self.follow_background();
        }
        // The flow's first effects (the title, or the level to play).
        self.run_effects(event_loop);
        Ok(())
    }

    /// Plays movie `index` for the flow; when it cannot play, the flow goes
    /// on at once.
    fn start_movie(&mut self, index: MovieIndex) {
        let candidates = locate_all(index, &self.movie_roots);
        self.movie = MoviePlayback::start(&candidates, self.audio.as_mut());
        match &self.movie {
            Some(m) => eprintln!("rusty-claw: playing {}", m.source().describe()),
            None => {
                if let Some(f) = self.flow.as_mut() {
                    f.movie_missing();
                }
            }
        }
        self.follow_background();
    }

    /// Holds a playing movie while the window is in the background and
    /// lets it go on when the window is back, and tells the flow, whose
    /// idle time stands still meanwhile.
    fn follow_background(&mut self) {
        let background = self.focused_once && self.in_background();
        if let Some(m) = self.movie.as_mut() {
            m.hold(background);
        }
        if let Some(f) = self.flow.as_mut() {
            f.set_background(background);
        }
    }

    /// Ends the playing movie when it is over: its sound stops and the
    /// flow learns how it ended ([`movie::tell_flow`]) and goes on (a
    /// `--movie` run ends, with an error when the movie failed).
    fn finish_movie(&mut self, event_loop: &ActiveEventLoop) {
        let Some(end) = self.movie.as_ref().and_then(MoviePlayback::end) else {
            return;
        };
        if let Some(m) = self.movie.take() {
            m.stop(self.audio.as_mut());
        }
        if let Some(target) = &self.movie_only {
            if let Err(e) = target.result(end) {
                self.error = Some(anyhow!(e));
            }
            event_loop.exit();
        } else if let Some(f) = self.flow.as_mut() {
            movie::tell_flow(f, end);
            self.run_effects(event_loop);
        }
    }

    /// Draws the playing movie's newest picture, centred at its shape on
    /// black.
    fn draw_movie(&mut self) {
        let (Some(live), Some(movie)) = (&mut self.live, &mut self.movie) else {
            return;
        };
        let Live {
            gpu,
            config,
            movie: presenter,
            ..
        } = live;
        let presenter = presenter.get_or_insert_with(|| MoviePresenter::new(gpu, config.format));
        if let Some(picture) = movie.frame()
            && let Err(e) = presenter.upload(gpu, &picture)
        {
            eprintln!(
                "rusty-claw: the movie {} cannot be shown: {e}",
                movie.source().describe()
            );
            movie.fail();
            return;
        }
        let aspect = movie.aspect();
        let texture = match live.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                live.surface.configure(&live.gpu.device, &live.config);
                return;
            }
            _ => return,
        };
        let size = (texture.texture.width(), texture.texture.height());
        let target = texture.texture.create_view(&Default::default());
        if let Some(p) = &live.movie {
            p.render(&live.gpu, &target, size, aspect);
        }
        live.window.pre_present_notify();
        live.gpu.queue.present(texture);
    }

    /// A frame of a `--movie` run.
    fn movie_frame(&mut self, event_loop: &ActiveEventLoop) {
        self.pacer.wait();
        let _ = self.clock.frame();
        self.last_frame = Instant::now();
        if self.movie.is_none() {
            event_loop.exit();
            return;
        }
        self.finish_movie(event_loop);
        self.draw_movie();
    }

    /// Loads `choice` and starts playing it from `start`.
    fn start_level(
        &mut self,
        choice: &crate::settings::LevelChoice,
        start: claw_sim::StartState,
    ) -> Result<()> {
        let t = Instant::now();
        let (bundle, mut graphics) = match self.first_level.take() {
            Some(l) if *choice == self.settings.level => l,
            _ => {
                let bundle = load_level(&self.settings.install, choice)?;
                let graphics = load_graphics(&bundle, self.settings.pack.as_deref())?;
                for w in graphics.pack_warnings() {
                    eprintln!("rusty-claw: image pack: {w}");
                }
                (bundle, graphics)
            }
        };
        let at = start_override(&self.settings, self.levels_started == 0, choice, &start);
        // Gameplay options changed in the menus apply from this level on.
        if let Some(f) = &self.flow {
            let s = f.settings();
            self.settings.mid_air_hit_fix = s.mid_air_hit_fix;
            self.settings.pause_input = s.pause_input;
            self.settings.fuse_clock = s.fuse_clock;
            self.settings.aids = s.aids;
        }
        let sim = start_sim(
            bundle.sim_level.clone(),
            self.settings.sim_config(self.first_output),
            start.clone(),
            at,
        );
        self.levels_started += 1;
        announce_widening(&self.settings, self.first_output, &sim);
        self.preloaded = preload(&mut graphics, &bundle.level, &sim.snapshot());
        let mut status = StatusLayer::new(&mut graphics, &bundle);
        graphics.preload([PAUSE_SET]);
        let mut session = Session::new(sim)
            .with_pause_input(self.settings.pause_input)
            .with_death_circles();
        session.set_smooth_rope(self.settings.smooth_rope);
        self.router
            .set_keep_held(self.settings.pause_input == PauseInput::Original);
        // Every level is recorded, each into its own file.
        if let Some(path) = &self.settings.record {
            self.recording
                .get_or_insert_with(|| GameRecording::new(path))
                .start_from(&mut session, flow::retail_number(choice), start, at);
        }
        // The in-game menu, drawn from the level's art.
        let tree = self
            .screens
            .as_ref()
            .map(|s| s.assets.menu(MenuKind::InGame))
            .transpose()
            .unwrap_or_else(|e| {
                eprintln!("rusty-claw: the in-game menu cannot be read: {e}");
                None
            })
            .map(|ifc| MenuTree::from_ifc(ifc, &CommandCatalogue::new(MenuKind::InGame)));
        if let Some(t) = &tree {
            graphics.preload(t.image_sets());
            graphics.preload(draw::extra_sets(MenuKind::InGame));
            graphics.preload(["GAME_CONFIGCONTROLS"]);
        }
        graphics.preload([SHADE_SET]);
        if let Some(flow) = self.flow.as_mut() {
            flow.set_in_game_menu(tree);
            flow.measure(MenuKind::InGame, &mut graphics);
            status.set_centred(flow.status_bar_centred());
        }
        load_sounds(self.audio.as_mut(), &bundle);
        if let Some(live) = &mut self.live {
            live.renderer.set_level(&graphics);
            let view = view_for(
                self.aspect,
                self.scale,
                self.settings.widen_activation,
                (live.config.width, live.config.height),
            );
            live.builder = Some(scene_builder(
                &graphics,
                view,
                live.gpu.max_target(),
                self.settings.front_layer,
            ));
            session.set_presentation(view);
        }
        eprintln!(
            "rusty-claw: {} ready in {:.0} ms",
            bundle.name,
            t.elapsed().as_secs_f64() * 1000.0
        );
        self.level = Some(Level {
            bundle,
            graphics,
            session,
            status,
        });
        self.clock.reset();
        self.pacer.reset();
        Ok(())
    }

    /// Leaves the level in play.
    fn end_level(&mut self) {
        self.save_recording();
        self.level = None;
        if let Some(live) = &mut self.live {
            live.builder = None;
        }
    }

    /// Writes the recording of the level in play, if it is recorded.
    fn save_recording(&mut self) {
        if let (Some(recording), Some(level)) = (self.recording.as_mut(), &self.level) {
            match recording.finish(&level.session) {
                Some(Ok((path, n))) => {
                    eprintln!("rusty-claw: recorded {n} ticks to {}", path.display())
                }
                Some(Err(e)) => eprintln!("rusty-claw: {e:#}"),
                None => {}
            }
        }
    }

    /// Carries out what the flow asked for.
    fn run_effects(&mut self, event_loop: &ActiveEventLoop) {
        for _ in 0..8 {
            let Some(flow) = self.flow.as_mut() else {
                return;
            };
            let effects = flow.take_effects();
            if effects.is_empty() {
                return;
            }
            for e in effects {
                self.effect(event_loop, e);
            }
        }
    }

    fn effect(&mut self, event_loop: &ActiveEventLoop, e: Effect) {
        // The level starts or ends first; then every effect goes through
        // the pauses and the sounds (see [`Pauses::follow`]), so a level
        // left from the menu never leaves the sounds paused for the next
        // one, nor its loops playing over the menus.
        match &e {
            Effect::StartLevel { level, start } => {
                if let Err(err) = self.start_level(level, start.clone()) {
                    eprintln!("rusty-claw: {err:#}");
                    if let Some(f) = self.flow.as_mut() {
                        f.level_failed(&format!("{err:#}"));
                    }
                }
            }
            Effect::EndLevel => self.end_level(),
            Effect::ResumeGame => {
                // The keys down as play goes on, menus included.
                if let Some(level) = &mut self.level {
                    level.session.sync_keys(self.router.buttons_down());
                }
            }
            _ => {}
        }
        self.pauses.background = self.in_background();
        let session = self.level.as_mut().map(|l| &mut l.session);
        let change =
            self.pauses
                .follow(&e, session, self.audio.as_mut(), self.last_frame.elapsed());
        self.pause_changed(change);
        let in_menu = self
            .flow
            .as_ref()
            .is_some_and(|f| f.screen() == Screen::Pause);
        match e {
            Effect::PauseGame => {
                // The in-game menu is recorded as a menu span (the game-over
                // fade is not: play ends there).
                if in_menu && let Some(level) = &mut self.level {
                    level.session.open_span(Span::Menu(MenuScreen::InGame));
                }
            }
            Effect::ResumeGame => {
                if let Some(level) = &mut self.level {
                    level.session.close_span();
                }
            }
            Effect::EndLife => {
                if let Some(level) = &mut self.level {
                    level.session.end_life();
                }
            }
            Effect::FrontLayer(on) => {
                self.settings.front_layer = on;
                if let Some(live) = &mut self.live {
                    if let Some(b) = live.builder.as_mut() {
                        b.set_front_layer(on);
                    }
                    if let Some(o) = live.offscreen.as_mut() {
                        o.builder.set_front_layer(on);
                    }
                }
            }
            Effect::StartLevel { .. } | Effect::EndLevel | Effect::Sound(_) | Effect::MenuChord => {
            }
            Effect::ScreenSounds => {
                if let (Some(audio), Some(s)) = (&mut self.audio, &self.screens) {
                    audio.load_level(&s.assets.store, &s.assets.registries);
                }
            }
            Effect::AudioSettings => {
                if let (Some(audio), Some(f)) = (&mut self.audio, &self.flow) {
                    audio.set_settings(f.audio_settings());
                }
            }
            Effect::Video(v) => self.video(v),
            Effect::Bindings => {
                if let Some(f) = &self.flow {
                    let s = f.settings();
                    self.router.set_keymap(s.keymap.clone());
                    self.router.set_pad_map(s.pad.map.clone(), s.pad.dead_zone);
                }
            }
            Effect::Capture { action, pad } => {
                if pad {
                    let first = self
                        .router
                        .pad_settings()
                        .map
                        .buttons_of(action)
                        .first()
                        .copied();
                    self.router.capture_next_pad_button(action, first);
                } else {
                    let first = self.router.keymap().keys(action).first().copied();
                    self.router.capture_next_key(action, first);
                }
            }
            Effect::ShowTitle => {
                if let (Some(f), Some(s)) = (self.flow.as_mut(), &self.screens) {
                    f.set_title(s.title());
                }
            }
            Effect::ShowHelp => {
                if let (Some(f), Some(s)) = (self.flow.as_mut(), self.screens.as_mut()) {
                    f.set_help(s.help());
                }
                // Help over a pause leaves the pause key's flag set: play
                // runs when help closes, until the pause key is pressed.
                if self.pauses.user {
                    self.pauses.user = false;
                    self.pauses.flag = true;
                }
                if let Some(level) = &mut self.level {
                    level.session.open_span(Span::Menu(MenuScreen::Help));
                }
                self.update_title();
            }
            Effect::ShowBooty(stats) => {
                if let (Some(f), Some(s)) = (self.flow.as_mut(), self.screens.as_mut()) {
                    f.set_booty(s.booty(stats));
                }
            }
            Effect::ShowCredits(entry) => {
                let tune_ms = self
                    .audio
                    .as_ref()
                    .and_then(|a| a.track_ms(Track::Monolith));
                if let (Some(f), Some(s)) = (self.flow.as_mut(), self.screens.as_mut()) {
                    f.set_credits(s.credits(entry, tune_ms));
                }
            }
            Effect::Music { track, looped } => {
                if let Some(a) = self.audio.as_mut() {
                    a.play_track(track, looped);
                }
            }
            Effect::PlayMovie(index) => self.start_movie(index),
            // The flow wrote it to the config file already.
            Effect::RecordIntroSeen => {}
            Effect::Quit => event_loop.exit(),
        }
    }

    fn video(&mut self, v: VideoSetting) {
        let Some(f) = &self.flow else { return };
        let s = f.settings().clone();
        let centred = f.status_bar_centred();
        match v {
            VideoSetting::Resolution => {
                if let Some(live) = &self.live
                    && live.window.fullscreen().is_none()
                {
                    let _ = live
                        .window
                        .request_inner_size(PhysicalSize::new(s.res.0, s.res.1));
                }
                self.size_index = WINDOW_SIZES
                    .iter()
                    .position(|&w| w == s.res)
                    .unwrap_or(self.size_index);
            }
            VideoSetting::Aspect | VideoSetting::Scale => {
                self.aspect = s.aspect;
                self.scale = s.scale;
                self.rebuild_view();
            }
            VideoSetting::Fullscreen => {
                if let Some(live) = &self.live {
                    live.window
                        .set_fullscreen(s.fullscreen.then_some(Fullscreen::Borderless(None)));
                }
            }
            VideoSetting::Vsync | VideoSetting::FrameCap => {
                self.settings.vsync = s.vsync;
                self.settings.fps_cap = s.fps_cap;
                let mut mode = None;
                if let Some(live) = &mut self.live {
                    let caps = live.surface.get_capabilities(&live.gpu.adapter);
                    live.config.present_mode = present_mode(&caps.present_modes, s.vsync);
                    live.surface.configure(&live.gpu.device, &live.config);
                    mode = Some(live.config.present_mode);
                }
                if let Some(m) = mode {
                    self.pacing_for(m);
                }
            }
            VideoSetting::Interpolation => self.interp = s.interpolation,
            VideoSetting::SmoothRope => {
                self.settings.smooth_rope = s.smooth_rope;
                if let Some(l) = &mut self.level {
                    l.session.set_smooth_rope(s.smooth_rope);
                }
            }
            VideoSetting::StatusBar => {
                if let Some(l) = &mut self.level {
                    l.status.set_centred(centred);
                }
            }
        }
    }

    fn rebuild_view(&mut self) {
        let Some(live) = &mut self.live else { return };
        let view = view_for(
            self.aspect,
            self.scale,
            self.settings.widen_activation,
            (live.config.width, live.config.height),
        );
        if let Some(level) = &mut self.level {
            live.builder = Some(scene_builder(
                &level.graphics,
                view,
                live.gpu.max_target(),
                self.settings.front_layer,
            ));
            level.session.set_presentation(view);
        }
        live.screen_builder = ScreenBuilder::new(&view, live.gpu.max_target());
        note(&mut self.events, "view rebuilt");
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        let Some(live) = &mut self.live else { return };
        if size.width == 0 || size.height == 0 {
            return;
        }
        let max = live.gpu.max_target();
        live.config.width = size.width.min(max);
        live.config.height = size.height.min(max);
        live.surface.configure(&live.gpu.device, &live.config);
        note(&mut self.events, "resize");
        if let Some(b) = &mut self.bench
            && b.measuring_since
                .is_some_and(|t| t.elapsed().as_secs_f64() >= WARMUP_S)
        {
            b.samples.resizes += 1;
        }
        self.rebuild_view();
    }

    fn command(&mut self, event_loop: &ActiveEventLoop, cmd: Command, from_pad: bool) {
        match cmd {
            Command::Quit | Command::Pause | Command::Help => {
                self.game_key(event_loop, cmd, from_pad);
            }
            Command::CyclePowerup => self.cycle_powerup(),
            Command::ToggleFullscreen => {
                if let Some(live) = &self.live {
                    let full = live.window.fullscreen().is_none();
                    live.window
                        .set_fullscreen(full.then_some(Fullscreen::Borderless(None)));
                    if let Some(f) = self.flow.as_mut() {
                        f.view_changed(|s| s.fullscreen = full);
                    }
                }
            }
            Command::NextWindowSize => {
                if let Some(live) = &self.live {
                    if live.window.fullscreen().is_some() {
                        eprintln!("rusty-claw: leave fullscreen (F11) to change the window size");
                        return;
                    }
                    self.size_index = (self.size_index + 1) % WINDOW_SIZES.len();
                    let (w, h) = WINDOW_SIZES[self.size_index];
                    eprintln!("rusty-claw: window {w}x{h}");
                    let _ = live.window.request_inner_size(PhysicalSize::new(w, h));
                    if let Some(f) = self.flow.as_mut() {
                        f.view_changed(|s| s.res = (w, h));
                    }
                }
            }
            Command::NextAspect => {
                self.aspect = self.aspect.next();
                eprintln!("rusty-claw: aspect {}", self.aspect.name());
                self.rebuild_view();
                let aspect = self.aspect;
                if let Some(f) = self.flow.as_mut() {
                    f.view_changed(|s| s.aspect = aspect);
                }
            }
            Command::ToggleScale => {
                self.scale = self.scale.toggled();
                self.rebuild_view();
                let scale = self.scale;
                if let Some(f) = self.flow.as_mut() {
                    f.view_changed(|s| s.scale = scale);
                }
            }
            Command::ToggleInterpolation => {
                self.interp = !self.interp;
                eprintln!(
                    "rusty-claw: interpolation {}",
                    if self.interp { "on" } else { "off" }
                );
                let interp = self.interp;
                if let Some(f) = self.flow.as_mut() {
                    f.view_changed(|s| s.interpolation = interp);
                }
            }
        }
    }

    /// The play-test key that gives powerups (see
    /// [`Session::cycle_powerup`]): only while play shows (not under a
    /// screen or the menu, nor once the level is won), and only with a
    /// play-test aid on in the level. The powerup's name shows on screen.
    fn cycle_powerup(&mut self) {
        let in_play = self
            .flow
            .as_ref()
            .is_none_or(|f| f.screen() == Screen::Play && !f.completing());
        // A replay or a bench run plays its own input: a powerup given there
        // would leave the recording.
        let bench = matches!(self.mode, WindowMode::Bench(_));
        let Some(level) = &mut self.level else { return };
        if !in_play || bench || level.session.replaying() || level.session.sim().level_ended() {
            return;
        }
        if let Some(p) = level.session.cycle_powerup() {
            level.status.show_powerup(p);
            eprintln!("rusty-claw: play-test powerup {p}");
        }
    }

    /// Escape, the pause key, F1 or a gamepad's Start (see [`game_key`]).
    fn game_key(&mut self, event_loop: &ActiveEventLoop, cmd: Command, from_pad: bool) {
        let ctx = KeyContext {
            screen: self.flow.as_ref().map(Flow::screen),
            completing: self.flow.as_ref().is_some_and(Flow::completing),
            pauses: self.pauses,
            from_pad,
            modifiers: self.modifiers.shift_key()
                || self.modifiers.control_key()
                || self.modifiers.alt_key(),
        };
        match game_key(cmd, ctx) {
            GameKey::Menu => {
                let Some(f) = self.flow.as_mut() else { return };
                if f.screen() == Screen::Pause
                    && let Some(level) = &mut self.level
                {
                    level.session.menu_key("back");
                }
                if f.pause_key() {
                    self.run_effects(event_loop);
                }
            }
            GameKey::Help => {
                if let Some(f) = self.flow.as_mut()
                    && f.help_key()
                {
                    self.run_effects(event_loop);
                }
            }
            GameKey::Pause => {
                let on = !self.pauses.user;
                self.user_pause(on);
            }
            GameKey::ClearFlag => {
                // The pause key's flag left by help: a pause of no time,
                // which goes on as any resume does. The game's sounds
                // already play, so that pause leaves them alone (it would
                // cut the effects playing); the music carries on where the
                // flag held it.
                self.pauses.flag = false;
                let audio = self.audio.take();
                self.user_pause(true);
                self.user_pause(false);
                self.audio = audio;
                self.update_pause();
            }
            GameKey::Quit => event_loop.exit(),
            GameKey::Nothing => {}
        }
    }

    /// Starts (`on`) or ends the pause key's pause, recorded as a pause
    /// span; ending it, the keys down now count for the tick that follows.
    fn user_pause(&mut self, on: bool) {
        self.pauses.user = on;
        if !on && let Some(level) = &mut self.level {
            level.session.sync_keys(self.router.buttons_down());
        }
        self.update_pause();
        self.update_title();
        if let Some(level) = &mut self.level {
            if on {
                level.session.open_span(Span::Pause);
            } else {
                level.session.close_span();
            }
        }
    }

    /// Brings the level in play and the sounds in line with the pauses
    /// (see [`Pauses`]).
    fn update_pause(&mut self) {
        self.pauses.background = self.in_background();
        let session = self.level.as_mut().map(|l| &mut l.session);
        let change = self
            .pauses
            .apply(session, self.audio.as_mut(), self.last_frame.elapsed());
        self.pause_changed(change);
    }

    /// True when the window is in the background (a benchmark never is).
    fn in_background(&self) -> bool {
        !self.focused && self.bench.is_none()
    }

    /// Restarts the frame clock when the level in play runs again.
    fn pause_changed(&mut self, change: Change) {
        if change == Change::Resumed {
            self.clock.reset();
            self.pacer.reset();
            if let Some(b) = &mut self.bench {
                b.last_present = None;
            }
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, down: bool) {
        if self.movie.is_some() {
            // The window's own keys run their command; every other key is
            // the movie's.
            match movie::movie_key(&mut self.router, code, down) {
                MovieKey::Window(cmd) => self.command(event_loop, cmd, false),
                MovieKey::Movie(input) => {
                    if let Some(m) = self.movie.as_mut() {
                        m.input(&input);
                    }
                }
                MovieKey::Released => {}
            }
            return;
        }
        let routed = self.router.key(code, down);
        let menu = self.flow.as_ref().map(Flow::screen);
        if down
            && self.router.context() == InputContext::Menu
            && !self.router.capturing()
            && matches!(menu, Some(Screen::MainMenu | Screen::Pause))
            && !matches!(routed, Some(Routed::Menu(_)))
            && !(menu == Some(Screen::Pause) && routed == Some(Routed::Command(Command::Pause)))
            && let Some(c) = key_char(code)
            && let Some(f) = self.flow.as_mut()
        {
            if menu == Some(Screen::Pause)
                && let Some(level) = &mut self.level
            {
                level.session.menu_key(&c.to_ascii_lowercase().to_string());
            }
            f.hotkey(c);
            self.run_effects(event_loop);
            return;
        }
        if let Some(r) = routed {
            self.route(event_loop, r, false);
        }
    }

    /// Acts on one routed input.
    fn route(&mut self, event_loop: &ActiveEventLoop, r: Routed, from_pad: bool) {
        match r {
            Routed::Button(b, down) => {
                if self.bench.is_some() {
                    return;
                }
                if let Some(level) = &mut self.level {
                    let at = level.session.stamp(self.last_frame.elapsed());
                    level.session.key(at, b, down);
                }
            }
            Routed::Command(cmd) => self.command(event_loop, cmd, from_pad),
            Routed::Menu(m) => {
                if let Some(f) = self.flow.as_mut() {
                    if f.screen() == Screen::Pause
                        && let Some(level) = &mut self.level
                    {
                        level.session.menu_key(menu_input_name(m));
                    }
                    f.menu_input(flow::menu_input(m), from_pad);
                    self.run_effects(event_loop);
                }
            }
        }
    }

    /// A left click: handed to the flow at its place on the 640x480
    /// screen, when a standalone screen shows.
    fn click(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(m) = self.movie.as_mut() {
            m.input(&RawInput::Click);
            return;
        }
        let (Some((px, py)), Some(live)) = (self.cursor, &self.live) else {
            return;
        };
        let Some(f) = self.flow.as_mut() else { return };
        if !flow::is_standalone(f.screen()) {
            return;
        }
        let c = live.screen_builder.composition();
        let (x, y) = canvas_point(c, (px, py));
        f.click(x, y);
        self.run_effects(event_loop);
    }

    /// Polls the input devices other than the keyboard.
    fn poll_inputs(&mut self, event_loop: &ActiveEventLoop) {
        let routed = self.router.poll(Instant::now());
        let presses = self
            .pad_presses
            .as_ref()
            .map(|p| std::mem::take(&mut *p.borrow_mut()))
            .unwrap_or_default();
        if let Some(m) = self.movie.as_mut() {
            // The pads are the movie's while it plays.
            for b in presses {
                m.input(&RawInput::Pad(b));
            }
            let _ = self.router.take_press();
            return;
        }
        for r in routed {
            self.route(event_loop, r, true);
        }
        // A newly pressed game input other than a direction (the credits
        // leave on it).
        if self.router.take_press()
            && let Some(f) = self.flow.as_mut()
        {
            f.credits_press();
            self.run_effects(event_loop);
        }
        if let Some(outcome) = self.router.take_capture()
            && let Some(f) = self.flow.as_mut()
        {
            let pad = self.router.pad_settings().map.clone();
            f.captured(outcome, self.router.keymap(), &pad);
            self.run_effects(event_loop);
        }
    }

    /// Sends inputs to the menus or to play, as the flow's screen asks.
    fn sync_context(&mut self, event_loop: &ActiveEventLoop) {
        let Some(f) = &self.flow else { return };
        let want = if f.screen() == Screen::Play {
            InputContext::Play
        } else {
            InputContext::Menu
        };
        for r in self.router.set_context(want) {
            self.route(event_loop, r, false);
        }
    }

    /// Brings the sounds in line with the pauses and menus a watched script
    /// replayed in this frame (see [`ReplayedHolds`]).
    fn follow_holds(&mut self) {
        let holds = std::mem::take(&mut self.holds);
        if holds.is_empty() {
            return;
        }
        let restart = self.replayed.follow(&holds, &mut self.pauses);
        self.update_pause();
        if restart && let Some(audio) = &mut self.audio {
            audio.restart_music();
        }
    }

    /// Plays the sound cues of the ticks just run.
    fn play_outputs(&mut self) {
        let cues = std::mem::take(&mut self.cues);
        let Some(audio) = &mut self.audio else { return };
        let view = match &self.level {
            Some(level) => {
                let r = level.session.sim().view().rect;
                ViewInfo {
                    left: r.left,
                    top: r.top,
                    width: (r.right - r.left + 1).max(0) as u32,
                    height: (r.bottom - r.top + 1).max(0) as u32,
                }
            }
            None => ViewInfo::default(),
        };
        audio.apply(&cues, &view);
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_none() {
            return;
        }
        self.poll_inputs(event_loop);
        self.follow_background();
        if self.movie_only.is_some() {
            self.movie_frame(event_loop);
        } else if self.flow.is_some() {
            self.flow_frame(event_loop);
        } else {
            self.frame_inner(event_loop);
        }
        // The cues first: those of ticks before a recorded pause start
        // before the pause cuts them; the mixer holds any started while
        // paused until the resume.
        self.play_outputs();
        self.follow_holds();
        self.update_title();
    }

    /// A frame of the game with menus.
    fn flow_frame(&mut self, event_loop: &ActiveEventLoop) {
        self.pacer.wait();
        let dt = self.clock.frame();
        self.last_frame = Instant::now();
        let dt_ms = dt.as_secs_f64() * 1000.0;
        // Play: run the ticks of this frame and hand over their outputs.
        if let Some(level) = &mut self.level {
            let f = level.session.frame(dt);
            let (cues, mut events) = level.session.drain_outputs();
            self.cues.extend(cues);
            if let Some(audio) = &mut self.audio {
                // The level-complete music stop.
                audio.events(&events);
            }
            if let Some(flow) = self.flow.as_mut() {
                flow.play_events(&events, level.session.sim());
                flow.hide_messages(&mut events);
            }
            level.status.events(&events);
            level.status.update(level.session.sim(), f.ticks);
            level.status.circles(level.session.circles());
        }
        let held = self
            .router
            .menu_held()
            .first()
            .copied()
            .map(flow::menu_input);
        if let Some(flow) = self.flow.as_mut() {
            let sim = self.level.as_ref().map(|l| l.session.sim());
            flow.update(dt_ms, held, sim);
        }
        self.run_effects(event_loop);
        self.finish_movie(event_loop);
        self.sync_context(event_loop);
        if let Some(s) = self.screens.as_mut()
            && self
                .flow
                .as_ref()
                .is_some_and(|f| f.screen() == Screen::MainMenu)
        {
            s.sparkles.update(dt_ms);
        }
        self.draw_flow();
    }

    /// The standalone screen the flow shows, if any, and what is drawn over
    /// it after it is built (the menu and the notice).
    fn screen_scene(&mut self) -> Option<(ScreenScene, Vec<MenuSprite>)> {
        let flow = self.flow.as_ref()?;
        let screens = self.screens.as_mut()?;
        let fade = steady_fade(flow.brightness());
        let mut over = Vec::new();
        let mut scene = match flow.screen() {
            Screen::Title => flow.title()?.scene(),
            Screen::Booty => flow.booty()?.scene(),
            Screen::Help => flow.help()?.scene(),
            Screen::Credits => flow.credits()?.scene(),
            Screen::MainMenu => {
                let (bg, pal) = match &screens.menu {
                    Some((id, pal)) => (Some(*id), pal.clone()),
                    None => (None, Palette::default()),
                };
                let mut sprites = screens.sparkles.sprites();
                if let Some(nav) = flow.main_menu() {
                    if nav.page() == Page::Controls {
                        sprites.extend(controls_panel("DIALOGS_CONFIGCONTROLS"));
                    }
                    over.extend(draw::picture(
                        nav,
                        &mut screens.graphics,
                        &flow.menu_state(),
                        flow.elapsed_ms(),
                    ));
                }
                ScreenScene {
                    background: bg,
                    palette: pal,
                    sprites,
                    fade: None,
                }
            }
            _ => return None,
        };
        if let Some(text) = flow.notice() {
            over.extend(
                notice_sprites(&mut screens.graphics, text)
                    .into_iter()
                    .map(MenuSprite::plain),
            );
        }
        scene.fade = fade;
        Some((scene, over))
    }

    /// The in-game menu's sprites over the level, and the notice.
    fn overlay(flow: &Flow, graphics: &mut LevelGraphics) -> Vec<MenuSprite> {
        let mut sprites = Vec::new();
        if flow.screen() == Screen::Pause
            && let Some(nav) = flow.in_game_menu()
        {
            if nav.page() == Page::Controls {
                sprites.extend(
                    controls_panel("GAME_CONFIGCONTROLS")
                        .into_iter()
                        .map(MenuSprite::plain),
                );
            }
            sprites.extend(draw::picture(
                nav,
                graphics,
                &flow.menu_state(),
                flow.elapsed_ms(),
            ));
        }
        if let Some(text) = flow.notice() {
            sprites.extend(
                notice_sprites(graphics, text)
                    .into_iter()
                    .map(MenuSprite::plain),
            );
        }
        draw::centred_picture(sprites)
    }

    fn draw_flow(&mut self) {
        if self.movie.is_some() {
            self.draw_movie();
            return;
        }
        let screen = self.screen_scene();
        let Some(live) = &mut self.live else { return };
        let scene;
        let use_screens;
        if let Some((sc, over)) = screen {
            let Some(s) = self.screens.as_mut() else {
                return;
            };
            let mut built = live.screen_builder.build(&mut s.graphics, &sc, 0.0);
            draw::add_to_scene(&mut built, &mut s.graphics, &over);
            scene = built;
            use_screens = true;
        } else if let (Some(level), Some(builder)) = (&mut self.level, live.builder.as_mut()) {
            let view = *builder.view();
            let frame = level.session.interpolated(self.interp);
            let mut input = FrameInput::from_frame(&frame, &view);
            let presentation = builder.composition().presentation;
            level
                .status
                .apply(&mut input, level.session.sim(), presentation);
            if self.pauses.user {
                input.screen.push(pause_sign());
            }
            let mut over = Vec::new();
            if let Some(f) = &self.flow {
                if f.screen() == Screen::Pause {
                    input.screen.extend(shade_sprites(presentation));
                }
                over = App::overlay(f, &mut level.graphics);
            }
            let mut built = builder.build(&mut level.graphics, &input);
            draw::add_to_scene(&mut built, &mut level.graphics, &over);
            if let Some(g) = self.flow.as_ref().and_then(Flow::game_over) {
                dim(&mut built, g.brightness());
            }
            scene = built;
            use_screens = false;
        } else {
            // Nothing to show yet (a level loading failed, or no art).
            let Some(s) = self.screens.as_mut() else {
                return;
            };
            let blank = ScreenScene {
                background: None,
                palette: Palette::default(),
                sprites: Vec::new(),
                fade: None,
            };
            scene = live.screen_builder.build(&mut s.graphics, &blank, 0.0);
            use_screens = true;
        }
        let texture = match live.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                live.surface.configure(&live.gpu.device, &live.config);
                return;
            }
            _ => return,
        };
        if (texture.texture.width(), texture.texture.height()) != scene.comp.output {
            return;
        }
        let target = texture.texture.create_view(&Default::default());
        if use_screens {
            if let Some(s) = &self.screens {
                live.screen_renderer
                    .render(&s.graphics, &scene, &target, live.config.format);
            }
        } else if let Some(level) = &self.level {
            live.renderer
                .render(&level.graphics, &scene, &target, live.config.format);
        }
        live.window.pre_present_notify();
        live.gpu.queue.present(texture);
    }

    /// A frame of a watched script or the benchmark.
    fn frame_inner(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(live), Some(level)) = (&mut self.live, &mut self.level) else {
            return;
        };
        let Some(builder) = live.builder.as_mut() else {
            return;
        };
        let session = &mut level.session;
        let status = &mut level.status;
        // The cap's wait comes before the frame's clock and work timer.
        self.pacer.wait();
        let dt = self.clock.frame();
        self.last_frame = Instant::now();
        let t0 = self.last_frame;
        let sets_before = level.graphics.loaded_while_drawing();
        let images_before = level.graphics.images.records.len();
        let f = session.frame(dt);
        // Sound cues are played after the frame; messages show in it.
        let (cues, game_events) = session.drain_outputs();
        self.cues.extend(cues);
        self.holds.extend(session.take_holds());
        if let Some(audio) = &mut self.audio {
            // The level-complete music stop.
            audio.events(&game_events);
        }
        status.events(&game_events);
        status.update(session.sim(), f.ticks);
        status.circles(session.circles());
        if !self.announced_end && matches!(self.mode, WindowMode::Watch(_)) && !session.replaying()
        {
            self.announced_end = true;
            eprintln!(
                "rusty-claw: the script ended after {} ticks; the keyboard has control",
                session.ticks()
            );
        }
        let view = *builder.view();
        let frame = session.interpolated(self.interp);
        let mut input = FrameInput::from_frame(&frame, &view);
        let presentation = builder.composition().presentation;
        status.apply(&mut input, session.sim(), presentation);
        if self.pauses.user {
            input.screen.push(pause_sign());
        }
        let scene = builder.build(&mut level.graphics, &input);
        if let Some(copy) = &mut live.offscreen {
            let view = *copy.builder.view();
            let mut input = FrameInput::from_frame(&frame, &view);
            let presentation = copy.builder.composition().presentation;
            status.apply(&mut input, session.sim(), presentation);
            let scene = copy.builder.build(&mut level.graphics, &input);
            copy.renderer
                .render(&level.graphics, &scene, &copy.output.view, COLOR_FORMAT);
        }
        let cpu = t0.elapsed();
        let ta = Instant::now();
        let texture = match live.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                live.surface.configure(&live.gpu.device, &live.config);
                note(&mut self.events, "surface reconfigured");
                note(&mut self.events, "frame skipped");
                return;
            }
            _ => {
                note(&mut self.events, "frame skipped");
                return;
            }
        };
        let acquire = ta.elapsed();
        if (texture.texture.width(), texture.texture.height()) != scene.comp.output {
            // The window changed size since the last configure.
            note(&mut self.events, "frame skipped");
            return;
        }
        let t1 = Instant::now();
        let target = texture.texture.create_view(&Default::default());
        live.renderer
            .render(&level.graphics, &scene, &target, live.config.format);
        let work = (cpu + t1.elapsed()).as_secs_f64() * 1000.0;
        live.window.pre_present_notify();
        let tp = Instant::now();
        live.gpu.queue.present(texture);
        let now = Instant::now();
        let events = std::mem::take(&mut self.events);

        let Some(b) = &mut self.bench else { return };
        let since = *b.measuring_since.get_or_insert(now);
        let measured = now.duration_since(since).as_secs_f64();
        if measured < WARMUP_S {
            b.last_present = Some(now);
            b.images_at_start = level.graphics.images.records.len();
            session.time_ticks();
            return;
        }
        if let Some(prev) = b.last_present.replace(now) {
            let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
            b.samples.present_ms.push(ms(now - prev));
            b.samples.present_at_s.push(measured - WARMUP_S);
            b.samples.present_detail.push(PresentDetail {
                gap_ms: ms(t0.saturating_duration_since(prev)),
                work_ms: work,
                acquire_ms: ms(acquire),
                present_call_ms: ms(now - tp),
                ticks: f.ticks,
                sets_loaded: level.graphics.loaded_while_drawing() - sets_before,
                images_loaded: level.graphics.images.records.len() - images_before,
                events,
            });
        }
        b.samples.frame(work, f.ticks);
        b.samples.acquire_ms.push(acquire.as_secs_f64() * 1000.0);
        if f.ticks > 0 {
            b.combat.look(session.sim());
        }
        if measured >= WARMUP_S + b.seconds {
            let comp = scene.comp;
            let sim = session.sim();
            // Some compositors report the monitor only once the window is
            // shown, or never; then the median present interval stands in.
            let monitor = self.display_hz.or_else(|| {
                live.window
                    .current_monitor()
                    .and_then(|m| m.refresh_rate_millihertz())
                    .filter(|&mhz| mhz > 0)
                    .map(|mhz| f64::from(mhz) / 1000.0)
            });
            let (display_hz, display_hz_source) = match monitor {
                Some(hz) => (hz, "monitor"),
                None => {
                    let median = Summary::of(&b.samples.present_ms).p50;
                    if median > 0.0 {
                        (1000.0 / median, "estimated from presents")
                    } else {
                        (0.0, "unknown")
                    }
                }
            };
            self.report = Some(Report {
                mode: "window",
                level: level.bundle.name.clone(),
                res: self.settings.res,
                output: (live.config.width, live.config.height),
                offscreen_copy: live.offscreen.as_ref().map(|c| c.output.size),
                presentation: comp.presentation,
                scale: comp.scale,
                adapter: live.gpu.describe(),
                present_mode: format!("{:?}", live.config.present_mode),
                pacing: self.pacing.clone(),
                display_hz,
                display_hz_source,
                seconds: measured - WARMUP_S,
                ticks: session.ticks(),
                activation: sim.config().activation.mode.name(),
                images: b.images_at_start,
                preloaded_sets: self.preloaded,
                images_loaded_during_run: level.graphics.images.records.len() - b.images_at_start,
                load_ms: self.load_ms,
                route_kind: b.route_kind,
                route_start: b.route_start,
                route_ticks: b.route_ticks,
                ticks_past_route: session.ticks_past_script(),
                combat: b.combat,
                audio: None,
                samples: std::mem::take(&mut b.samples),
                ticks_measured: session.tick_samples().to_vec(),
            });
            event_loop.exit();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_some() {
            return;
        }
        if let Err(e) = self.open(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size),
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.focused_once |= focused;
                for r in self.router.set_focused(focused) {
                    self.route(event_loop, r, false);
                }
                self.update_pause();
                self.follow_background();
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                // The pause key toggles on every key-down, auto-repeat
                // included, as the original's; other keys ignore repeats.
                let pause_key =
                    self.router.keymap().action(code) == Some(Action::Command(Command::Pause));
                if event.repeat && !pause_key {
                    return;
                }
                self.key(event_loop, code, event.state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y));
            }
            WindowEvent::CursorLeft { .. } => self.cursor = None,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(event_loop),
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(live) = &self.live {
            live.window.request_redraw();
        }
    }
}

/// Opens the window and runs until it is closed: `bundle` with `graphics`
/// is the level to watch or benchmark, or the first level played with
/// [`WindowMode::Play`]. For a benchmark, returns the report.
pub fn run(
    settings: Settings,
    mode: WindowMode,
    bundle: LevelBundle,
    graphics: LevelGraphics,
    started: Instant,
) -> Result<Option<Report>> {
    match mode {
        WindowMode::Play => run_app(
            settings,
            mode,
            Some(Start::Play),
            Some((bundle, graphics)),
            started,
        ),
        _ => run_app(settings, mode, None, Some((bundle, graphics)), started),
    }
}

/// Opens the window at the title and runs the game with its menus until
/// it is closed.
pub fn run_menus(settings: Settings, started: Instant) -> Result<()> {
    run_app(
        settings,
        WindowMode::Play,
        Some(Start::Title),
        None,
        started,
    )
    .map(|_| ())
}

/// Opens the window for one movie, `target`, and closes it when the movie
/// ends or is skipped. Fails when the movie cannot play.
pub fn run_movie(settings: Settings, target: MovieTarget, started: Instant) -> Result<()> {
    if !claw_movie::available() {
        bail!("{}", movie::NO_PLAYER);
    }
    run_app(
        settings,
        WindowMode::Play,
        Some(Start::Movie(target)),
        None,
        started,
    )
    .map(|_| ())
}

fn run_app(
    settings: Settings,
    mode: WindowMode,
    menus: Option<Start>,
    level: Option<(LevelBundle, LevelGraphics)>,
    started: Instant,
) -> Result<Option<Report>> {
    let event_loop = EventLoop::new().context("opening the display")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let size_index = WINDOW_SIZES
        .iter()
        .position(|&s| s == settings.res)
        .unwrap_or(1);
    // The level is built in `open`, once the output size is known.
    if menus.is_none() && level.is_none() {
        bail!("no level to show");
    }
    let first_level = level;
    let mut screens = None;
    let mut flow = None;
    let mut audio = None;
    let run_mode = RunMode::of(&settings);
    let movie_roots = settings.movie_roots(&UserDirs::from_env());
    let movie_only = match &menus {
        Some(Start::Movie(target)) => Some(target.clone()),
        _ => None,
    };
    let movies = movies_allowed(&run_mode) && settings.movies_on();
    if movies && !claw_movie::available() {
        eprintln!("rusty-claw: {}", movie::NO_PLAYER);
    }
    if movie_only.is_some() {
        // With sound off too (silent): the movie runs on its soundtrack's
        // clock, which holds it in the background.
        audio = Some(AudioBridge::without_music(settings.audio_settings()));
    } else if let Some(start) = menus {
        let s = Screens::load(&settings.install)?;
        let main = match s.assets.menu(MenuKind::Main) {
            Ok(ifc) => Some(MenuTree::from_ifc(
                ifc,
                &CommandCatalogue::new(MenuKind::Main),
            )),
            Err(e) => {
                eprintln!("rusty-claw: the main menu cannot be read: {e}");
                None
            }
        };
        let mut s = s;
        let font = FontArt::from_art(&mut s.graphics);
        let (saves, saves_off) =
            match SaveStore::open_default(&UserDirs::from_env(), Some(&settings.install)) {
                Ok(store) => (Some(store), None),
                Err(e) => (None, Some(format!("saved games are off: {e:#}"))),
            };
        let mut f = Flow::new(settings.clone(), saves, main, font, start);
        if movies && claw_movie::available() {
            // Before the first effects, so the start-up movies come before
            // the title.
            let shelf = MovieShelf {
                intro_seen: settings.intro_seen,
                skip_logo: settings.skip_logo,
                ..movie::shelf(&movie_roots)
            };
            f.set_movies(shelf);
        }
        if let Some(why) = saves_off {
            // Shown on screen at start-up (and written to the terminal).
            f.notify(why);
        }
        f.measure(MenuKind::Main, &mut s.graphics);
        let mut bridge = AudioBridge::new(f.audio_settings());
        bridge.load_level(&s.assets.store, &s.assets.registries);
        f.set_music_available(bridge.has_soundfont());
        audio = Some(bridge);
        screens = Some(s);
        flow = Some(f);
    } else if let Some(a) = level_audio(&mode, &settings) {
        audio = Some(AudioBridge::new(a));
    }
    let (pad_tap, pad_presses) =
        if settings.pad.enabled && (movie_only.is_some() || (movies && claw_movie::available())) {
            let (tap, presses) = PadTap::new();
            (Some(tap), Some(presses))
        } else {
            (None, None)
        };
    let mut app = App {
        aspect: settings.aspect,
        scale: settings.scale,
        interp: settings.interpolation,
        // A benchmark reads the keyboard only, so no gamepad is opened. A
        // movie needs the pad buttons themselves (see [`PadTap`]).
        router: if matches!(mode, WindowMode::Bench(_)) {
            InputRouter::keyboard_only(settings.keymap.clone())
        } else if let Some(tap) = pad_tap {
            InputRouter::with_source(settings.keymap.clone(), settings.pad.clone(), Box::new(tap))
        } else {
            InputRouter::from_settings(&settings)
        },
        settings,
        mode,
        level: None,
        screens,
        flow,
        audio,
        recording: None,
        levels_started: 0,
        first_level,
        shown: None,
        cues: Vec::new(),
        holds: Vec::new(),
        replayed: ReplayedHolds::default(),
        live: None,
        clock: FrameClock::new(),
        pacer: FramePacer::uncapped(),
        pacing: String::new(),
        events: Vec::new(),
        last_frame: Instant::now(),
        pauses: Pauses::default(),
        focused: true,
        focused_once: false,
        cursor: None,
        modifiers: ModifiersState::empty(),
        size_index,
        started,
        load_ms: 0.0,
        preloaded: 0,
        display_hz: None,
        first_output: (0, 0),
        bench: None,
        report: None,
        error: None,
        announced_end: false,
        movie_roots,
        movie: None,
        movie_only,
        pad_presses,
    };
    event_loop.run_app(&mut app).context("running the window")?;
    app.save_recording();
    if let Some(e) = app.error {
        return Err(e);
    }
    if app.bench.is_some() && app.report.is_none() {
        bail!("the window closed before the benchmark finished");
    }
    Ok(app.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    use clap::Parser;
    use claw_frame::{Point, TransitionKind};

    use crate::settings::Args;

    fn settings(line: &str) -> Settings {
        let args =
            Args::try_parse_from(std::iter::once("rusty-claw").chain(line.split_whitespace()))
                .unwrap();
        Settings::resolve(&args, None).unwrap()
    }

    fn watch() -> WindowMode {
        let rec = LevelRecording::parse("0,0,0\n1,0,0\n").unwrap();
        WindowMode::Watch(Box::new(rec))
    }

    #[test]
    fn a_watched_replay_plays_sound_and_the_benchmark_and_traces_do_not() {
        // Watched in the window: the bridge plays with the settings' sound.
        let s = settings("--install /i --replay r.csv --res 640x480");
        let a = level_audio(&watch(), &s).expect("a watched replay has sound");
        assert_eq!(a, s.audio_settings());
        assert!(a.enabled && a.music);
        // The volumes and switches of the settings carry over.
        let mut quiet = s.clone();
        quiet.sound_volume = 20;
        quiet.voice = false;
        let a = level_audio(&watch(), &quiet).unwrap();
        assert_eq!((a.sound_volume, a.voice), (20, false));
        // Sound off wins: no bridge, no device.
        let off = settings("--install /i --replay r.csv --no-audio");
        assert!(level_audio(&watch(), &off).is_none());
        let mut file_off = s.clone();
        file_off.audio = false;
        assert!(level_audio(&watch(), &file_off).is_none());
        // The window benchmark is silent.
        let bench = settings("--install /i --bench 10");
        assert!(level_audio(&WindowMode::Bench(10.0), &bench).is_none());
        // A replay that writes a trace is silent too, however it is asked.
        let trace = settings("--install /i --replay r.csv --trace out.csv");
        assert!(matches!(trace.mode, Mode::Replay { trace: Some(_), .. }));
        assert!(level_audio(&watch(), &trace).is_none());
    }

    fn transition(closing: bool) -> ScreenTransition {
        ScreenTransition {
            kind: TransitionKind::Circle,
            closing,
            start_tick: 10,
            duration_ms: 100,
            center: Point::default(),
        }
    }

    #[test]
    fn clicks_land_on_the_screen_under_the_pointer() {
        // A 640x480 screen scaled to 1280x960 inside a 1600x960 window.
        let c = claw_render::Composition {
            presentation: (640, 480),
            scale: 2,
            target: (1280, 960),
            output: (1600, 960),
            x: 160,
            y: 0,
            width: 1280,
            height: 960,
            ratio: 1.0,
        };
        assert_eq!(canvas_point(&c, (160.0, 0.0)), (0, 0));
        assert_eq!(canvas_point(&c, (359.0, 199.0)), (99, 99));
        assert_eq!(canvas_point(&c, (361.0, 100.0)), (100, 50));
        assert_eq!(canvas_point(&c, (1439.9, 959.9)), (639, 479));
        // On the bar at the left, off the screen.
        assert!(canvas_point(&c, (10.0, 10.0)).0 < 0);
    }

    #[test]
    fn an_opening_transition_stops_running_at_its_end() {
        // The first tick at or after `ms` from the start.
        let ticks = |ms: u32| 10 + u64::from(ms.div_ceil(TICK_MS));
        let open = transition(false);
        assert!(transition_running(&open, 10));
        assert!(transition_running(&open, ticks(100) - 1));
        assert!(!transition_running(&open, ticks(100)));
        assert!(!transition_running(&open, ticks(5000)));
        // A closing one runs until another one replaces it.
        assert!(transition_running(&transition(true), ticks(5000)));
    }

    #[test]
    fn the_shade_halves_the_brightness_under_it() {
        use claw_assets::Frame;
        use claw_render::MemoryImages;
        use claw_render::cpu;
        use claw_render::effects::LevelTables;

        // A grey ramp: index i is (i, i, i).
        let mut palette = Palette::default();
        for (i, c) in palette.0.iter_mut().enumerate() {
            *c = [i as u8; 3];
        }
        let mut images = MemoryImages::default();
        images.insert(SHADE_SET, 1, crate::load::shade_frame(&palette));
        let mut graphics = LevelGraphics::new(
            Vec::new(),
            true,
            palette.clone(),
            LevelTables::generate(&palette),
            Box::new(images),
            None,
        );
        let picture = graphics.add_image(&Frame {
            width: 640,
            height: 480,
            offset_x: 0,
            offset_y: 0,
            flags: 0,
            indices: vec![200; 640 * 480],
            opaque: None,
        });
        let builder = ScreenBuilder::new(&claw_frame::ViewConfig::default(), 4096);
        let presentation = builder.composition().presentation;
        let mut screen = ScreenScene {
            background: Some(picture),
            palette,
            sprites: Vec::new(),
            fade: None,
        };
        let middle = |graphics: &mut LevelGraphics, screen: &ScreenScene| {
            let scene = builder.build(graphics, screen, 0.0);
            let c = cpu::compose(graphics, &scene);
            c.index[(c.height / 2 * c.width + c.width / 2) as usize]
        };
        assert_eq!(middle(&mut graphics, &screen), 200);
        screen.sprites = shade_sprites(presentation);
        let shaded = middle(&mut graphics, &screen);
        assert!((99..=102).contains(&shaded), "{shaded}");
    }

    #[test]
    fn the_shade_covers_the_whole_picture_once() {
        for presentation in [(640, 480), (1708, 480), (853, 480), (1, 1)] {
            let sprites = shade_sprites(presentation);
            let step = SHADE_SIZE as i32;
            let half = step / 2;
            assert!(sprites.iter().all(|s| s.key == SHADE_SET
                && s.mode == DrawMode::GHOST
                && s.anchor == Anchor::TopLeft));
            // Every pixel of the picture (and a margin) is under exactly
            // one square.
            let (w, h) = (presentation.0 as i32, presentation.1 as i32);
            for (x, y) in [(-1, -1), (0, 0), (w - 1, h - 1), (w, h), (w / 2, h / 2)] {
                let n = sprites
                    .iter()
                    .filter(|s| {
                        (s.x - half..s.x + half).contains(&x)
                            && (s.y - half..s.y + half).contains(&y)
                    })
                    .count();
                assert_eq!(n, 1, "{presentation:?} at {x},{y}");
            }
        }
    }

    #[test]
    fn the_aid_marker_names_the_powerup_aid() {
        use claw_sim::tuning::PlayAids;
        let mut cfg = SimConfig::default();
        assert!(aid_marker_lines(&cfg).is_empty());
        cfg.start_powerup = Some(AidPowerup::parse("lightning").unwrap());
        assert_eq!(aid_marker_lines(&cfg), ["POWERUP LIGHTNING SWORD"]);
        cfg.aids = PlayAids {
            invincible: true,
            infinite_lives: true,
        };
        assert_eq!(
            aid_marker_lines(&cfg),
            ["INVINCIBLE", "INFINITE LIVES", "POWERUP LIGHTNING SWORD"]
        );
    }
}
