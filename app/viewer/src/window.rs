//! The windowed viewer.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use claw_frame::{Buttons, FrameClock, FrameDriver, InputLatch, ScaleMode, ViewConfig};
use claw_render::{FrameInput, Gpu, Renderer, SceneBuilder, wgpu};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::stub::Stub;
use crate::{Args, Loaded, load, next_aspect};

/// Frame statistics printed on exit.
#[derive(Debug, Default)]
struct Stats {
    work_ms: Vec<f64>,
    intervals_ms: Vec<f64>,
    last_present: Option<Instant>,
    refresh_ms: Option<f64>,
}

impl Stats {
    fn report(&mut self) -> String {
        let n = self.work_ms.len();
        if n == 0 {
            return "no frames drawn".into();
        }
        self.work_ms.sort_by(f64::total_cmp);
        let at = |q: f64| self.work_ms[((q * n as f64).ceil() as usize).clamp(1, n) - 1];
        // Without a reported refresh rate, take the median present interval.
        let (period, source) = match self.refresh_ms {
            Some(p) => (Some(p), "reported"),
            None if !self.intervals_ms.is_empty() => {
                let mut v = self.intervals_ms.clone();
                v.sort_by(f64::total_cmp);
                (Some(v[v.len() / 2]), "estimated")
            }
            None => (None, "unknown"),
        };
        let late = period.map_or(0, |p| {
            self.intervals_ms.iter().filter(|&&dt| dt > p * 1.5).count()
        });
        format!(
            "{n} frames, frame work p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms; \
             {late} late presents at {} Hz ({source})",
            at(0.5),
            at(0.99),
            self.work_ms[n - 1],
            period.map_or("?".into(), |p| format!("{:.2}", 1000.0 / p)),
        )
    }
}

struct Live {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    renderer: Renderer,
    builder: SceneBuilder,
    view: ViewConfig,
}

struct App {
    args: Args,
    loaded: Loaded,
    stub: Stub,
    driver: FrameDriver,
    latch: InputLatch,
    clock: FrameClock,
    interp: bool,
    live: Option<Live>,
    stats: Stats,
    error: Option<anyhow::Error>,
    started: Option<Instant>,
}

fn present_mode(modes: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    use wgpu::PresentMode as P;
    let prefs: &[P] = if vsync {
        &[P::Mailbox, P::Fifo]
    } else {
        &[P::Immediate, P::Mailbox, P::Fifo]
    };
    prefs
        .iter()
        .copied()
        .find(|m| modes.contains(m))
        .unwrap_or(P::Fifo)
}

fn button(key: KeyCode) -> Option<Buttons> {
    match key {
        KeyCode::ArrowLeft => Some(Buttons::LEFT),
        KeyCode::ArrowRight => Some(Buttons::RIGHT),
        KeyCode::ArrowUp => Some(Buttons::UP),
        KeyCode::ArrowDown => Some(Buttons::DOWN),
        _ => None,
    }
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let (w, h) = self.args.size();
        let mut attrs = Window::default_attributes()
            .with_title(format!(
                "rusty-claw viewer: level {}",
                self.loaded.level.level
            ))
            .with_inner_size(PhysicalSize::new(w, h));
        if self.args.fullscreen {
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
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .context("the window surface is not supported by the GPU")?;
        config.format = format;
        config.present_mode = present_mode(&caps.present_modes, self.args.vsync);
        surface.configure(&gpu.device, &config);
        let mut renderer = Renderer::new(&gpu);
        renderer.set_level(&self.loaded.graphics);
        let view = self.args.view((config.width, config.height));
        let builder = SceneBuilder::new(&self.loaded.graphics, view, gpu.max_target());
        self.stats.refresh_ms = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .filter(|&mhz| mhz > 0)
            .map(|mhz| 1_000_000.0 / f64::from(mhz));
        eprintln!(
            "claw-viewer: {} on {}, {:?}, level loaded in {:.0} ms",
            format_args!("{}x{}", config.width, config.height),
            gpu.describe(),
            config.present_mode,
            self.loaded.load.as_secs_f64() * 1000.0
        );
        self.stub.set_view(view);
        self.live = Some(Live {
            window,
            surface,
            config,
            gpu,
            renderer,
            builder,
            view,
        });
        Ok(())
    }

    fn rebuild_view(&mut self) {
        let Some(live) = &mut self.live else { return };
        let view = ViewConfig {
            aspect_policy: live.view.aspect_policy,
            scale_mode: live.view.scale_mode,
            ..self.args.view((live.config.width, live.config.height))
        };
        live.view = view;
        live.builder = SceneBuilder::new(&self.loaded.graphics, view, live.gpu.max_target());
        self.stub.set_view(view);
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        let Some(live) = &mut self.live else { return };
        if size.width == 0 || size.height == 0 {
            return;
        }
        live.config.width = size.width;
        live.config.height = size.height;
        live.surface.configure(&live.gpu.device, &live.config);
        self.rebuild_view();
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, down: bool, repeat: bool) {
        if let Some(b) = button(code) {
            if down {
                self.latch.press(b);
            } else {
                self.latch.release(b);
            }
            return;
        }
        if matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
            self.stub.fast = down;
            return;
        }
        if !down || repeat {
            return;
        }
        match code {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::KeyF => self.stub.toggle_fade(),
            KeyCode::KeyI => self.interp = !self.interp,
            KeyCode::KeyS => {
                if let Some(live) = &mut self.live {
                    live.view.scale_mode = match live.view.scale_mode {
                        ScaleMode::Fit => ScaleMode::Integer,
                        ScaleMode::Integer => ScaleMode::Fit,
                    };
                    let view = live.view;
                    live.builder =
                        SceneBuilder::new(&self.loaded.graphics, view, live.gpu.max_target());
                }
            }
            KeyCode::KeyA => {
                if let Some(live) = &mut self.live {
                    live.view.aspect_policy =
                        next_aspect(live.view.aspect_policy, self.args.widen_activation);
                    let view = live.view;
                    live.builder =
                        SceneBuilder::new(&self.loaded.graphics, view, live.gpu.max_target());
                    self.stub.set_view(view);
                }
            }
            KeyCode::F11 => {
                if let Some(live) = &self.live {
                    let full = live.window.fullscreen().is_some();
                    live.window
                        .set_fullscreen((!full).then_some(Fullscreen::Borderless(None)));
                }
            }
            _ => {}
        }
    }

    fn frame(&mut self) {
        let dt = self.clock.frame();
        let t0 = Instant::now();
        let (stub, latch) = (&mut self.stub, &mut self.latch);
        self.driver
            .frame_with_input(dt, || latch.next_tick(), |input| stub.step(input));
        let Some(live) = &mut self.live else { return };
        let frame = if self.interp {
            self.driver.interpolated()
        } else {
            self.driver.latest()
        };
        let input = FrameInput::from_frame(&frame, &live.view);
        let scene = live.builder.build(&mut self.loaded.graphics, &input);
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
            // The window changed size since the last configure.
            return;
        }
        let target = texture.texture.create_view(&Default::default());
        live.renderer
            .render(&self.loaded.graphics, &scene, &target, live.config.format);
        self.stats.work_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        live.window.pre_present_notify();
        live.gpu.queue.present(texture);
        let now = Instant::now();
        if let Some(prev) = self.stats.last_present.replace(now) {
            self.stats
                .intervals_ms
                .push((now - prev).as_secs_f64() * 1000.0);
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
            WindowEvent::Focused(false) => {
                self.latch.release_all();
                self.stub.fast = false;
                self.driver.step_mut().pause();
            }
            WindowEvent::Focused(true) => {
                self.driver.step_mut().resume();
                self.clock.reset();
                self.stats.last_present = None;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(
                        event_loop,
                        code,
                        event.state == ElementState::Pressed,
                        event.repeat,
                    );
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let started = *self.started.get_or_insert_with(Instant::now);
        if let Some(limit) = self.args.quit_after
            && started.elapsed().as_secs_f64() >= limit
        {
            event_loop.exit();
            return;
        }
        if let Some(live) = &self.live {
            live.window.request_redraw();
        }
    }
}

/// Opens the window and runs until it is closed.
pub fn run(args: Args) -> Result<()> {
    let loaded = load(&args)?;
    let view = args.view(args.size());
    let stub = Stub::new(&loaded.level, view);
    let driver = FrameDriver::new(stub.snapshot());
    let event_loop = EventLoop::new().context("opening the display")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        interp: !args.no_interp,
        args,
        loaded,
        stub,
        driver,
        latch: InputLatch::new(),
        clock: FrameClock::new(),
        live: None,
        stats: Stats::default(),
        error: None,
        started: None,
    };
    app.stub.autopilot = app.args.quit_after.is_some();
    event_loop.run_app(&mut app).context("running the window")?;
    if let Some(e) = app.error {
        return Err(e);
    }
    eprintln!("claw-viewer: {}", app.stats.report());
    Ok(())
}
