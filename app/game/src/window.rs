//! The game window: keyboard in, frames and sound out at the display's
//! refresh.
//!
//! The same window runs normal play, watching a replayed script and the
//! window benchmark. Every key goes through the input router, which the
//! window also polls once per frame for other input devices. Normal play
//! opens an audio device (unless sound is off) and plays the sound cues of
//! every tick it runs; watching a script and the benchmark are silent. The
//! window title shows Claw's health, score and lives. The output size, the aspect policy, the scaling and
//! fullscreen change at run time; the 640x480 game view and the simulation
//! stay as they are.
//!
//! With vsync on, waiting for the display paces the frames. Otherwise each
//! frame first waits for the frame-rate cap (300 frames per second unless
//! set otherwise), so the game never draws thousands of frames a second.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use claw_frame::{FrameClock, FramePacer};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::{FrameInput, Gpu, LevelGraphics, Offscreen, Renderer, SceneBuilder, wgpu};
use claw_sim::InputScript;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Fullscreen, Window, WindowId};

use crate::audio::{AudioBridge, ViewInfo};
use crate::bench::{PresentDetail, Report, Samples, Summary, WARMUP_S, route};
use crate::input::{InputRouter, Recorder, Routed};
use crate::keymap::{Command, Keymap};
use crate::load::{LevelBundle, new_sim, preload};
use crate::session::Session;
use crate::settings::{Aspect, Scale, Settings, WINDOW_SIZES, view_for};

/// What the window does.
#[derive(Debug, Clone)]
pub enum WindowMode {
    /// Play with the keyboard.
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
    builder: SceneBuilder,
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

struct App {
    settings: Settings,
    mode: WindowMode,
    bundle: LevelBundle,
    graphics: LevelGraphics,
    router: InputRouter,
    audio: Option<AudioBridge>,
    recorder: Option<Recorder>,
    /// The numbers the title shows: health, score, lives, paused.
    shown: Option<(i32, u32, i32, bool)>,
    session: Option<Session>,
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
    focused: bool,
    size_index: usize,
    started: Instant,
    load_ms: f64,
    preloaded: usize,
    display_hz: Option<f64>,
    bench: Option<BenchState>,
    report: Option<Report>,
    error: Option<anyhow::Error>,
    announced_end: bool,
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
        let mut t = format!("rusty-claw: {}", self.bundle.name);
        if let Some(session) = &self.session {
            let s = session.sim().stats();
            t.push_str(&format!(
                " | health {} | score {} | lives {}",
                s.health, s.score, s.lives
            ));
        }
        if self.user_paused {
            t.push_str(" (paused)");
        }
        t
    }

    /// Sets the window title when the numbers it shows changed.
    fn update_title(&mut self) {
        let Some(session) = &self.session else { return };
        let s = session.sim().stats();
        let now = Some((s.health, s.score, s.lives, self.user_paused));
        if now != self.shown {
            self.shown = now;
            let title = self.title();
            if let Some(live) = &self.live {
                live.window.set_title(&title);
            }
        }
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
        // Waiting for the display paces the frames; any other mode is held
        // to the frame-rate cap.
        let (pacer, pacing) = if config.present_mode == wgpu::PresentMode::Fifo {
            (FramePacer::uncapped(), "display refresh".to_string())
        } else {
            match self.settings.fps_cap {
                Some(n) => (FramePacer::capped(Some(n)), format!("{n} fps cap")),
                None => (FramePacer::uncapped(), "none (throughput run)".to_string()),
            }
        };
        self.pacer = pacer;
        self.pacing = pacing;

        // The simulation starts now that the output size is known: widened
        // activation follows the first output's shape.
        let surface_size = (config.width, config.height);
        // A fullscreen window may not have its final size yet; the monitor
        // tells the shape it will have.
        let first_output = if self.settings.fullscreen {
            window
                .current_monitor()
                .map(|m| m.size())
                .filter(|s| s.width > 0 && s.height > 0)
                .map_or(surface_size, |s| (s.width, s.height))
        } else {
            surface_size
        };
        let sim = new_sim(
            &self.bundle,
            self.settings.sim_config(first_output),
            self.settings.start,
        );
        if self.settings.widens() {
            let (w, h) = sim.config().activation.view;
            eprintln!(
                "rusty-claw: widened activation follows the first output's shape \
                 ({}x{}): {w}x{h} game pixels stay active until the level is loaded again",
                first_output.0, first_output.1
            );
        }
        self.preloaded = preload(&mut self.graphics, &self.bundle.level, &sim.snapshot());
        let mut session = Session::new(sim);
        if matches!(self.mode, WindowMode::Play) && self.settings.audio {
            let mut audio = AudioBridge::new(self.settings.audio_settings());
            let r = audio.load_level(&self.bundle.store, &self.bundle.level.registries);
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
            self.audio = Some(audio);
        }
        if self.settings.record.is_some() && !matches!(self.mode, WindowMode::Bench(_)) {
            let recorder = Recorder::new();
            session.set_before_tick(recorder.hook());
            self.recorder = Some(recorder);
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
        let mut renderer = Renderer::new(&gpu);
        renderer.set_level(&self.graphics);
        let view = view_for(
            self.aspect,
            self.scale,
            self.settings.widen_activation,
            surface_size,
        );
        let builder = SceneBuilder::new(&self.graphics, view, max);
        let mut offscreen = None;
        let want = self.settings.res;
        if self.bench.is_some() && !self.settings.fullscreen && surface_size != want {
            // The desktop cannot hold a window of the requested size (a
            // compositor keeps windows inside the screen). Draw each frame a
            // second time offscreen at that size, so the measured work and
            // the refresh pacing include the full-size frame.
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
            let mut renderer = Renderer::new(&gpu);
            renderer.set_level(&self.graphics);
            let view = view_for(
                self.aspect,
                self.scale,
                self.settings.widen_activation,
                want,
            );
            offscreen = Some(OffscreenCopy {
                renderer,
                builder: SceneBuilder::new(&self.graphics, view, max),
                output: Offscreen::new(&gpu, want),
            });
        }
        self.display_hz = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|&mhz| mhz > 0)
            .map(|mhz| f64::from(mhz) / 1000.0);
        self.load_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "rusty-claw: {} at {}x{} on {}, {:?}, paced by {}, {} activation, ready in {:.0} ms",
            self.bundle.name,
            config.width,
            config.height,
            gpu.describe(),
            config.present_mode,
            self.pacing,
            session.sim().config().activation.mode.name(),
            self.load_ms,
        );
        if matches!(self.mode, WindowMode::Play | WindowMode::Watch(_)) {
            eprintln!("rusty-claw: {}", self.router.help());
        }
        self.session = Some(session);
        self.live = Some(Live {
            window,
            surface,
            config,
            gpu,
            renderer,
            builder,
            offscreen,
        });
        self.clock.reset();
        Ok(())
    }

    fn rebuild_view(&mut self) {
        let Some(live) = &mut self.live else { return };
        let view = view_for(
            self.aspect,
            self.scale,
            self.settings.widen_activation,
            (live.config.width, live.config.height),
        );
        live.builder = SceneBuilder::new(&self.graphics, view, live.gpu.max_target());
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
        let paused = self.user_paused || (!self.focused && self.bench.is_none());
        let Some(session) = &mut self.session else {
            return;
        };
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

    fn key(&mut self, event_loop: &ActiveEventLoop, code: winit::keyboard::KeyCode, down: bool) {
        if let Some(r) = self.router.key(code, down) {
            self.route(event_loop, r);
        }
    }

    /// Acts on one routed input.
    fn route(&mut self, event_loop: &ActiveEventLoop, r: Routed) {
        match r {
            Routed::Button(b, down) => {
                if self.bench.is_some() {
                    return;
                }
                if let Some(session) = &mut self.session {
                    let at = session.stamp(self.last_frame.elapsed());
                    session.key(at, b, down);
                }
            }
            Routed::Command(cmd) => self.command(event_loop, cmd),
            Routed::Menu(_) => {}
        }
    }

    /// Polls the input devices other than the keyboard.
    fn poll_inputs(&mut self, event_loop: &ActiveEventLoop) {
        for r in self.router.poll(Instant::now()) {
            self.route(event_loop, r);
        }
    }

    /// Plays the sound cues of the ticks just run; drops their game events,
    /// which nothing uses yet.
    fn play_outputs(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let (cues, _events) = session.drain_outputs();
        if let Some(audio) = &mut self.audio {
            let r = session.sim().view().rect;
            let view = ViewInfo {
                left: r.left,
                top: r.top,
                width: (r.right - r.left + 1).max(0) as u32,
                height: (r.bottom - r.top + 1).max(0) as u32,
            };
            audio.apply(&cues, &view);
        }
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_none() {
            return;
        }
        self.poll_inputs(event_loop);
        self.frame_inner(event_loop);
        self.play_outputs();
        self.update_title();
    }

    fn frame_inner(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(live), Some(session)) = (&mut self.live, &mut self.session) else {
            return;
        };
        // The cap's wait comes before the frame's clock and work timer.
        self.pacer.wait();
        let dt = self.clock.frame();
        self.last_frame = Instant::now();
        let t0 = self.last_frame;
        let sets_before = self.graphics.loaded_while_drawing();
        let images_before = self.graphics.images.records.len();
        let f = session.frame(dt);
        if !self.announced_end && matches!(self.mode, WindowMode::Watch(_)) && !session.replaying()
        {
            self.announced_end = true;
            eprintln!(
                "rusty-claw: the script ended after {} ticks; the keyboard has control",
                session.ticks()
            );
        }
        let view = *live.builder.view();
        let frame = session.interpolated(self.interp);
        let input = FrameInput::from_frame(&frame, &view);
        let scene = live.builder.build(&mut self.graphics, &input);
        if let Some(copy) = &mut live.offscreen {
            let view = *copy.builder.view();
            let input = FrameInput::from_frame(&frame, &view);
            let scene = copy.builder.build(&mut self.graphics, &input);
            copy.renderer
                .render(&self.graphics, &scene, &copy.output.view, COLOR_FORMAT);
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
            .render(&self.graphics, &scene, &target, live.config.format);
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
            b.images_at_start = self.graphics.images.records.len();
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
                sets_loaded: self.graphics.loaded_while_drawing() - sets_before,
                images_loaded: self.graphics.images.records.len() - images_before,
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
                level: self.bundle.name.clone(),
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
                images_loaded_during_run: self.graphics.images.records.len() - b.images_at_start,
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

/// Opens the window and runs until it is closed. For a benchmark, returns
/// the report.
pub fn run(
    settings: Settings,
    mode: WindowMode,
    bundle: LevelBundle,
    graphics: LevelGraphics,
    started: Instant,
) -> Result<Option<Report>> {
    let event_loop = EventLoop::new().context("opening the display")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let size_index = WINDOW_SIZES
        .iter()
        .position(|&s| s == settings.res)
        .unwrap_or(1);
    let mut app = App {
        aspect: settings.aspect,
        scale: settings.scale,
        interp: settings.interpolation,
        settings,
        mode,
        bundle,
        graphics,
        router: InputRouter::new(Keymap::default()),
        audio: None,
        recorder: None,
        shown: None,
        session: None,
        live: None,
        clock: FrameClock::new(),
        pacer: FramePacer::uncapped(),
        pacing: String::new(),
        events: Vec::new(),
        last_frame: Instant::now(),
        user_paused: false,
        focused: true,
        size_index,
        started,
        load_ms: 0.0,
        preloaded: 0,
        display_hz: None,
        bench: None,
        report: None,
        error: None,
        announced_end: false,
    };
    event_loop.run_app(&mut app).context("running the window")?;
    if let (Some(recorder), Some(session), Some(path)) =
        (&app.recorder, &app.session, &app.settings.record)
    {
        let n = recorder.save(session, path)?;
        eprintln!("rusty-claw: recorded {n} ticks to {}", path.display());
    }
    if let Some(e) = app.error {
        return Err(e);
    }
    if app.bench.is_some() && app.report.is_none() {
        bail!("the window closed before the benchmark finished");
    }
    Ok(app.report)
}
