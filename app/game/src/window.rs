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
//! ([`StatusLayer`]), and the in-game menu is drawn over the frozen level.
//! The window loads levels when the flow asks, plays the sound cues of
//! every tick it runs and applies the options changed in the menus at once
//! (the output size, the aspect policy, the scaling, fullscreen, vsync,
//! the frame-rate cap, interpolation and the key bindings).
//!
//! Watching a script and the benchmark show one level without menus and
//! are silent. The window title shows Claw's health, score and lives.
//!
//! With vsync on, waiting for the display paces the frames. Otherwise each
//! frame first waits for the frame-rate cap (300 frames per second unless
//! set otherwise), so the game never draws thousands of frames a second.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use claw_assets::{Install, Palette};
use claw_frame::{
    AudioCue, FrameClock, FramePacer, GameEvent, PaletteFade, ScreenTransition, TICK_MS,
};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::palette::{self, FULL_BRIGHTNESS};
use claw_render::{
    Anchor, FrameInput, Gpu, ImageId, LevelGraphics, Offscreen, Renderer, Scene, SceneBuilder,
    ScreenBuilder, ScreenScene, ScreenSprite, wgpu,
};
use claw_sim::{InputScript, Sim};
use claw_ui::hud::{Hud, HudArt};
use claw_ui::menu::command::{MenuKind, Page, VideoSetting};
use claw_ui::menu::decor::{SPARKLE, Sparkles};
use claw_ui::menu::draw::{self, FontArt};
use claw_ui::menu::{CommandCatalogue, MenuTree};
use claw_ui::screens::assets::{
    BOOTY_SCREENS, MENU_PICTURE, MENU_SCREENS, ScreenAssets, TITLE_PICTURE, TITLE_SCREENS,
    picture_frame,
};
use claw_ui::screens::booty::{self, BootyArt, BootyScreen, BootyStats};
use claw_ui::screens::title::{self, TitleScreen};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::{AudioBridge, ViewInfo};
use crate::bench::{PresentDetail, Report, Samples, Summary, WARMUP_S, route};
use crate::flow::{self, Effect, Flow, Screen, Start};
use crate::input::{InputContext, InputRouter, Recorder, Routed};
use crate::keymap::Command;
use crate::load::{LevelBundle, load_graphics, load_level, new_sim, preload, settle_at};
use crate::saves::SaveStore;
use crate::session::Session;
use crate::settings::{Aspect, Scale, Settings, UserDirs, WINDOW_SIZES, view_for};

/// The screen layer of a game in progress: the status bar, the message on
/// screen and the screen transition, for each frame drawn.
///
/// Messages age with game time only: by the ticks that run, and not while a
/// transition runs (or while paused, when no tick runs). They are hidden
/// while a transition runs.
#[derive(Debug, Clone)]
pub struct StatusLayer {
    hud: Hud,
    transition: Option<ScreenTransition>,
    fresh: bool,
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
            fresh: true,
        }
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
            self.transition = sim.snapshot().transition;
        }
        if ticks > 0 && self.transition.is_none() {
            self.hud
                .messages
                .advance(f64::from(ticks) * f64::from(TICK_MS));
        }
    }

    /// The screen transition in progress.
    pub fn transition(&self) -> Option<ScreenTransition> {
        self.transition
    }

    /// Fills the screen layer and the transition of `input`, a frame of
    /// `sim` in a presentation of `presentation` game pixels.
    pub fn apply(&mut self, input: &mut FrameInput<'_>, sim: &Sim, presentation: (u32, u32)) {
        input.screen = self.hud.layout(&sim.hud(), presentation, input.time_ms);
        if self.transition.is_none() {
            input.screen.extend(self.hud.message_layout());
        }
        input.transition = self.transition;
    }
}

/// What the window does.
#[derive(Debug, Clone)]
pub enum WindowMode {
    /// The game with its menus, straight in the level given.
    Play,
    /// Play a script, then hand over to the keyboard.
    Watch(InputScript),
    /// Run the benchmark route for this many seconds after the warm-up.
    Bench(f64),
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
        BootyScreen::new(stats, art)
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

/// A notice in the bitmap font at the top of the canvas.
fn notice_sprites(art: &mut dyn draw::Art, text: &str) -> Vec<ScreenSprite> {
    let (w, _) = draw::text_size(art, text);
    draw::text_sprites(art, text, Anchor::TopLeft, 320 - w / 2, 8)
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

struct App {
    settings: Settings,
    mode: WindowMode,
    level: Option<Level>,
    screens: Option<Screens>,
    flow: Option<Flow>,
    router: InputRouter,
    audio: Option<AudioBridge>,
    recorder: Option<Recorder>,
    /// True once the recorder was attached to a session.
    recorded: bool,
    /// Levels started so far.
    levels_started: u32,
    /// The level loaded before the window opened, for the first start.
    first_level: Option<(LevelBundle, LevelGraphics)>,
    /// The text the title shows.
    shown: Option<String>,
    /// Sound cues of the ticks of the current frame, played after it.
    cues: Vec<AudioCue>,
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
    user_paused: bool,
    menu_paused: bool,
    focused: bool,
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
        if self.user_paused || self.menu_paused {
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

        if self.flow.is_none() {
            // Watching a script or the benchmark: one level, no menus.
            let first_output = self.first_output;
            let Some(level) = self.level.as_mut() else {
                bail!("no level to show");
            };
            let sim = new_sim(
                &level.bundle,
                self.settings.sim_config(first_output),
                self.settings.start,
            );
            announce_widening(&self.settings, first_output, &sim);
            self.preloaded = preload(&mut level.graphics, &level.bundle.level, &sim.snapshot());
            level.status = StatusLayer::new(&mut level.graphics, &level.bundle);
            let mut session = Session::new(sim);
            if self.settings.record.is_some() && !matches!(self.mode, WindowMode::Bench(_)) {
                let recorder = Recorder::new();
                session.set_before_tick(recorder.hook());
                self.recorder = Some(recorder);
                self.recorded = true;
            }
            match &self.mode {
                WindowMode::Play => {}
                WindowMode::Watch(script) => session = session.with_replay(script.clone()),
                WindowMode::Bench(seconds) => {
                    let script = route(seconds + WARMUP_S);
                    let route_ticks = script.ticks.len();
                    session = session.with_looped_replay(script);
                    self.bench = Some(BenchState {
                        seconds: *seconds,
                        route_ticks,
                        ..BenchState::default()
                    });
                }
            }
            level.session = session;
            live.renderer.set_level(&level.graphics);
            live.builder = Some(SceneBuilder::new(&level.graphics, view, max));
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
                    builder: SceneBuilder::new(&level.graphics, view, max),
                    output: Offscreen::new(&live.gpu, want),
                });
            }
        }
        self.load_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "rusty-claw: {} at {}x{} on {}, {:?}, paced by {}, ready in {:.0} ms",
            self.level
                .as_ref()
                .map_or("menus", |l| l.bundle.name.as_str()),
            live.config.width,
            live.config.height,
            live.gpu.describe(),
            live.config.present_mode,
            self.pacing,
            self.load_ms,
        );
        if !matches!(self.mode, WindowMode::Bench(_)) {
            eprintln!("rusty-claw: {}", self.router.help());
        }
        self.live = Some(live);
        self.clock.reset();
        // The flow's first effects (the title, or the level to play).
        self.run_effects(event_loop);
        Ok(())
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
        let mut sim = Sim::with_start(
            bundle.sim_level.clone(),
            self.settings.sim_config(self.first_output),
            start,
        );
        if self.levels_started == 0
            && let Some((x, y)) = self.settings.start
        {
            settle_at(&mut sim, x, y);
        }
        self.levels_started += 1;
        announce_widening(&self.settings, self.first_output, &sim);
        self.preloaded = preload(&mut graphics, &bundle.level, &sim.snapshot());
        let mut status = StatusLayer::new(&mut graphics, &bundle);
        let mut session = Session::new(sim);
        if self.settings.record.is_some() && !self.recorded {
            let recorder = Recorder::new();
            session.set_before_tick(recorder.hook());
            self.recorder = Some(recorder);
            self.recorded = true;
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
        if let Some(flow) = self.flow.as_mut() {
            flow.set_in_game_menu(tree);
            flow.measure(MenuKind::InGame, &mut graphics);
            status.set_centred(flow.status_bar_centred());
        }
        if let Some(audio) = &mut self.audio {
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
        if let Some(live) = &mut self.live {
            live.renderer.set_level(&graphics);
            let view = view_for(
                self.aspect,
                self.scale,
                self.settings.widen_activation,
                (live.config.width, live.config.height),
            );
            live.builder = Some(SceneBuilder::new(&graphics, view, live.gpu.max_target()));
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
        self.menu_paused = false;
        self.user_paused = false;
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
        self.menu_paused = false;
        self.user_paused = false;
    }

    fn save_recording(&mut self) {
        if let (Some(recorder), Some(level), Some(path)) =
            (self.recorder.take(), &self.level, &self.settings.record)
        {
            match recorder.save(&level.session, path) {
                Ok(n) => eprintln!("rusty-claw: recorded {n} ticks to {}", path.display()),
                Err(e) => eprintln!("rusty-claw: {e:#}"),
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
        match e {
            Effect::StartLevel { level, start } => {
                if let Err(err) = self.start_level(&level, start) {
                    eprintln!("rusty-claw: {err:#}");
                    if let Some(f) = self.flow.as_mut() {
                        f.level_failed(&format!("{err:#}"));
                    }
                }
            }
            Effect::EndLevel => self.end_level(),
            Effect::PauseGame => {
                self.menu_paused = true;
                self.update_pause();
            }
            Effect::ResumeGame => {
                self.menu_paused = false;
                self.update_pause();
                if let Some(audio) = &mut self.audio {
                    audio.restart_music();
                }
            }
            Effect::Sound(cue) => {
                if let Some(audio) = &mut self.audio {
                    audio.apply(&[cue], &ViewInfo::default());
                }
            }
            Effect::MenuChord => {
                if let Some(audio) = &mut self.audio {
                    audio.play_menu_music();
                }
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
            Effect::ShowBooty(stats) => {
                if let (Some(f), Some(s)) = (self.flow.as_mut(), self.screens.as_mut()) {
                    f.set_booty(s.booty(stats));
                }
            }
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
        if let Some(level) = &self.level {
            live.builder = Some(SceneBuilder::new(
                &level.graphics,
                view,
                live.gpu.max_target(),
            ));
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

    fn command(&mut self, event_loop: &ActiveEventLoop, cmd: Command) {
        if let Some(f) = self.flow.as_mut()
            && matches!(cmd, Command::Quit | Command::Pause)
        {
            if f.pause_key() {
                self.run_effects(event_loop);
            }
            return;
        }
        match cmd {
            Command::Quit => event_loop.exit(),
            Command::ToggleFullscreen => {
                if let Some(live) = &self.live {
                    let full = live.window.fullscreen().is_some();
                    live.window
                        .set_fullscreen((!full).then_some(Fullscreen::Borderless(None)));
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
                }
            }
            Command::NextAspect => {
                self.aspect = self.aspect.next();
                eprintln!("rusty-claw: aspect {}", self.aspect.name());
                self.rebuild_view();
            }
            Command::ToggleScale => {
                self.scale = self.scale.toggled();
                self.rebuild_view();
            }
            Command::ToggleInterpolation => {
                self.interp = !self.interp;
                eprintln!(
                    "rusty-claw: interpolation {}",
                    if self.interp { "on" } else { "off" }
                );
            }
            Command::Pause => {
                self.user_paused = !self.user_paused;
                self.update_pause();
                self.update_title();
            }
        }
    }

    fn update_pause(&mut self) {
        let paused =
            self.user_paused || self.menu_paused || (!self.focused && self.bench.is_none());
        let Some(level) = &mut self.level else {
            return;
        };
        let session = &mut level.session;
        if paused && !session.is_paused() {
            let at = session.stamp(self.last_frame.elapsed());
            session.release_all(at);
            session.pause();
            if let Some(audio) = &mut self.audio {
                audio.pause();
            }
        } else if !paused && session.is_paused() {
            session.resume();
            if let Some(audio) = &mut self.audio {
                audio.resume();
            }
            self.clock.reset();
            self.pacer.reset();
            if let Some(b) = &mut self.bench {
                b.last_present = None;
            }
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, down: bool) {
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
            Routed::Command(cmd) => self.command(event_loop, cmd),
            Routed::Menu(m) => {
                if let Some(f) = self.flow.as_mut() {
                    f.menu_input(flow::menu_input(m), from_pad);
                    self.run_effects(event_loop);
                }
            }
        }
    }

    /// Polls the input devices other than the keyboard.
    fn poll_inputs(&mut self, event_loop: &ActiveEventLoop) {
        for r in self.router.poll(Instant::now()) {
            self.route(event_loop, r, true);
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
        if self.flow.is_some() {
            self.flow_frame(event_loop);
        } else {
            self.frame_inner(event_loop);
        }
        self.play_outputs();
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
            let (cues, events) = level.session.drain_outputs();
            self.cues.extend(cues);
            level.status.events(&events);
            level.status.update(level.session.sim(), f.ticks);
            if let Some(audio) = &mut self.audio {
                audio.events(&events);
            }
            if let Some(flow) = self.flow.as_mut() {
                flow.play_events(&events, level.session.sim());
            }
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

    /// The standalone screen the flow shows, if any.
    fn screen_scene(&mut self) -> Option<ScreenScene> {
        let flow = self.flow.as_ref()?;
        let screens = self.screens.as_mut()?;
        let fade = steady_fade(flow.brightness());
        let mut scene = match flow.screen() {
            Screen::Title => flow.title()?.scene(),
            Screen::Booty => flow.booty()?.scene(),
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
                    sprites.extend(draw::sprites(
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
            scene
                .sprites
                .extend(notice_sprites(&mut screens.graphics, text));
        }
        scene.fade = fade;
        Some(scene)
    }

    /// The in-game menu's sprites over the level, and the notice.
    fn overlay(flow: &Flow, graphics: &mut LevelGraphics) -> Vec<ScreenSprite> {
        let mut sprites = Vec::new();
        if flow.screen() == Screen::Pause
            && let Some(nav) = flow.in_game_menu()
        {
            if nav.page() == Page::Controls {
                sprites.extend(controls_panel("GAME_CONFIGCONTROLS"));
            }
            sprites.extend(draw::sprites(
                nav,
                graphics,
                &flow.menu_state(),
                flow.elapsed_ms(),
            ));
        }
        if let Some(text) = flow.notice() {
            sprites.extend(notice_sprites(graphics, text));
        }
        draw::centred(sprites)
    }

    fn draw_flow(&mut self) {
        let screen = self.screen_scene();
        let Some(live) = &mut self.live else { return };
        let scene;
        let use_screens;
        if let Some(sc) = screen {
            let Some(s) = self.screens.as_mut() else {
                return;
            };
            scene = live.screen_builder.build(&mut s.graphics, &sc, 0.0);
            use_screens = true;
        } else if let (Some(level), Some(builder)) = (&mut self.level, live.builder.as_mut()) {
            let view = *builder.view();
            let frame = level.session.interpolated(self.interp);
            let mut input = FrameInput::from_frame(&frame, &view);
            let presentation = builder.composition().presentation;
            level
                .status
                .apply(&mut input, level.session.sim(), presentation);
            if let Some(f) = &self.flow {
                input.screen.extend(App::overlay(f, &mut level.graphics));
            }
            let mut built = builder.build(&mut level.graphics, &input);
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
        status.events(&game_events);
        status.update(session.sim(), f.ticks);
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
                route_ticks: b.route_ticks,
                ticks_past_route: session.ticks_past_script(),
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
                for r in self.router.set_focused(focused) {
                    self.route(event_loop, r, false);
                }
                self.update_pause();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.repeat {
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(event_loop, code, event.state == ElementState::Pressed);
                }
            }
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
    let mut first_level = None;
    let mut level_state = None;
    match (&menus, level) {
        (Some(_), l) => first_level = l,
        (None, Some((bundle, mut graphics))) => {
            let status = StatusLayer::new(&mut graphics, &bundle);
            let sim = Sim::new(bundle.sim_level.clone(), settings.sim_config(settings.res));
            level_state = Some(Level {
                bundle,
                graphics,
                session: Session::new(sim),
                status,
            });
        }
        (None, None) => bail!("no level to show"),
    }
    let mut screens = None;
    let mut flow = None;
    let mut audio = None;
    if let Some(start) = menus {
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
        let saves = match SaveStore::open_default(&UserDirs::from_env(), Some(&settings.install)) {
            Ok(store) => Some(store),
            Err(e) => {
                eprintln!("rusty-claw: saved games are off: {e:#}");
                None
            }
        };
        let mut f = Flow::new(settings.clone(), saves, main, font, start);
        f.measure(MenuKind::Main, &mut s.graphics);
        let mut bridge = AudioBridge::new(f.audio_settings());
        bridge.load_level(&s.assets.store, &s.assets.registries);
        f.set_music_available(bridge.has_soundfont());
        audio = Some(bridge);
        screens = Some(s);
        flow = Some(f);
    }
    let mut app = App {
        aspect: settings.aspect,
        scale: settings.scale,
        interp: settings.interpolation,
        router: InputRouter::from_settings(&settings),
        settings,
        mode,
        level: level_state,
        screens,
        flow,
        audio,
        recorder: None,
        recorded: false,
        levels_started: 0,
        first_level,
        shown: None,
        cues: Vec::new(),
        live: None,
        clock: FrameClock::new(),
        pacer: FramePacer::uncapped(),
        pacing: String::new(),
        events: Vec::new(),
        last_frame: Instant::now(),
        user_paused: false,
        menu_paused: false,
        focused: true,
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
