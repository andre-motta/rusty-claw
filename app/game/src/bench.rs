//! Benchmarks: Claw on a fixed route through a level while every frame is
//! measured.
//!
//! The route is an input script (run right with jumps, run back, crouch and
//! look up) repeated for the length of the run. Two ways to run it:
//!
//! - in a window ([`crate::window`]): real presentation at the display's
//!   refresh, which also counts frames that missed their refresh;
//! - headless ([`run_headless`]): offscreen at `--res` with a simulated
//!   display clock, rendering as fast as the machine allows and waiting for
//!   the GPU after each frame, which also measures GPU time.
//!
//! The report is one JSON object. Times are milliseconds, each with `p50`,
//! `p99` and `max`:
//!
//! - `frame_work_ms`: CPU work of a frame: input, ticks, interpolation,
//!   scene building and command submission (not the wait for a free
//!   swapchain image);
//! - `tick_frame_work_ms`: the same for frames that ran at least one tick;
//! - `tick_ms`: one simulation tick alone, with `live_objects` the number
//!   of objects in the live list (Claw and the camera included);
//! - `deck_proxy_ms`: `tick_frame_work_ms.p99` times [`DECK_FACTOR`], an
//!   estimate of the same work on a handheld-class CPU;
//! - window runs: `acquire_ms` (waiting for a swapchain image),
//!   `present_interval_ms` and `missed_vblanks` (presents more than 1.5
//!   refresh periods apart) after a one-second warm-up; when the desktop
//!   cannot hold a window of the requested size, every frame is also drawn
//!   offscreen at that size and counted in the frame work
//!   (`offscreen_copy`);
//! - headless runs: `gpu_ms` and `frame_ms` (work plus GPU).
//!
//! `budget` checks the targets: frame work p99 under [`FRAME_BUDGET_MS`]
//! (a 144 Hz refresh), no missed refresh in a window run, and the handheld
//! estimate under [`DECK_BUDGET_MS`] (half a 60 Hz refresh).

use std::fmt::Write as _;
use std::time::Instant;

use anyhow::{Context, Result};
use claw_frame::{Buttons, SimulatedRefresh};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::{FrameInput, Gpu, Offscreen, Renderer, SceneBuilder};
use claw_sim::InputScript;

use crate::load::{load_graphics, load_level, new_sim, preload};
use crate::session::{Session, TickSample};
use crate::settings::Settings;

/// Per-frame CPU time on a handheld-class CPU relative to the machine the
/// benchmark runs on (a planning factor, not a measurement).
pub const DECK_FACTOR: f64 = 3.0;

/// Frame work budget: one refresh at 144 Hz, ms.
pub const FRAME_BUDGET_MS: f64 = 6.9;

/// Budget of the handheld estimate: half a 60 Hz refresh, ms.
pub const DECK_BUDGET_MS: f64 = 8.3;

/// Seconds at the start of a window run left out of the present
/// statistics (window mapping and the first swapchain images).
pub const WARMUP_S: f64 = 1.0;

/// The benchmark route as a script of at least `seconds`.
pub fn route(seconds: f64) -> InputScript {
    let (left, right, up, down, jump) = (
        Buttons::LEFT.0,
        Buttons::RIGHT.0,
        Buttons::UP.0,
        Buttons::DOWN.0,
        Buttons::JUMP.0,
    );
    // One cycle, 18 ms per entry: (mask, ticks).
    let mut cycle: Vec<(u32, usize)> = Vec::new();
    for dir in [right, left] {
        for _ in 0..4 {
            cycle.push((dir, 40));
            cycle.push((dir | jump, 25));
        }
        cycle.push((0, 20));
    }
    cycle.extend([(down, 60), (0, 20), (up, 60), (0, 20), (jump, 1), (0, 40)]);
    let per_cycle: usize = cycle.iter().map(|&(_, n)| n).sum();
    let want = (seconds.max(0.0) * 1000.0 / 18.0).ceil() as usize + 1;
    let mut held = Vec::with_capacity(want + per_cycle);
    while held.len() < want {
        for &(mask, n) in &cycle {
            held.extend(std::iter::repeat_n(mask, n));
        }
    }
    InputScript::from_held(&held)
}

/// p50, p99 and max of a sample.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Summary {
    /// Median.
    pub p50: f64,
    /// 99th percentile.
    pub p99: f64,
    /// Largest value.
    pub max: f64,
    /// Number of values.
    pub n: usize,
}

impl Summary {
    /// Summarizes `values` (nearest-rank percentiles).
    pub fn of(values: &[f64]) -> Summary {
        let mut v = values.to_vec();
        v.sort_by(f64::total_cmp);
        let at = |q: f64| {
            if v.is_empty() {
                return 0.0;
            }
            let rank = (q * v.len() as f64).ceil() as usize;
            v[rank.clamp(1, v.len()) - 1]
        };
        Summary {
            p50: at(0.5),
            p99: at(0.99),
            max: v.last().copied().unwrap_or(0.0),
            n: v.len(),
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4}, \"n\": {}}}",
            self.p50, self.p99, self.max, self.n
        )
    }
}

/// Raw measurements of a run.
#[derive(Debug, Clone, Default)]
pub struct Samples {
    /// CPU work per frame, ms.
    pub work_ms: Vec<f64>,
    /// CPU work of frames that ran a tick, ms.
    pub tick_work_ms: Vec<f64>,
    /// Waiting for a swapchain image, ms (window runs).
    pub acquire_ms: Vec<f64>,
    /// GPU time after submission, ms (headless runs).
    pub gpu_ms: Vec<f64>,
    /// Intervals between presents after the warm-up, ms (window runs).
    pub present_ms: Vec<f64>,
    /// When each interval ended, s after the warm-up (window runs).
    pub present_at_s: Vec<f64>,
    /// Window size changes after the warm-up (window runs).
    pub resizes: usize,
    /// Frames drawn.
    pub frames: usize,
}

impl Samples {
    /// Records one frame's work.
    pub fn frame(&mut self, work_ms: f64, ticks: u32) {
        self.frames += 1;
        self.work_ms.push(work_ms);
        if ticks > 0 {
            self.tick_work_ms.push(work_ms);
        }
    }
}

/// A benchmark result.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// "window" or "headless".
    pub mode: &'static str,
    /// Level name.
    pub level: String,
    /// Requested output size.
    pub res: (u32, u32),
    /// Output size actually drawn.
    pub output: (u32, u32),
    /// Window runs: size of the offscreen copy drawn with every frame when
    /// the window could not get the requested size.
    pub offscreen_copy: Option<(u32, u32)>,
    /// Presentation view in game pixels.
    pub presentation: (u32, u32),
    /// Whole scale of the composition.
    pub scale: u32,
    /// GPU and driver.
    pub adapter: String,
    /// Presentation mode of a window run.
    pub present_mode: String,
    /// Display refresh rate (reported by the monitor, or simulated).
    pub display_hz: f64,
    /// Where `display_hz` came from.
    pub display_hz_source: &'static str,
    /// Length of the measured run, s.
    pub seconds: f64,
    /// Ticks run.
    pub ticks: u64,
    /// Activation mode.
    pub activation: &'static str,
    /// Images loaded when the run started.
    pub images: usize,
    /// Sprite sets loaded before the first frame.
    pub preloaded_sets: usize,
    /// Images loaded during the run (should be 0).
    pub images_loaded_during_run: usize,
    /// Load time up to the first drawn frame, ms.
    pub load_ms: f64,
    /// Frame measurements.
    pub samples: Samples,
    /// Tick measurements.
    pub ticks_measured: Vec<TickSample>,
}

impl Report {
    /// Frame work summary.
    pub fn frame_work(&self) -> Summary {
        Summary::of(&self.samples.work_ms)
    }

    /// Work of frames that ran a tick.
    pub fn tick_frame_work(&self) -> Summary {
        Summary::of(&self.samples.tick_work_ms)
    }

    /// The handheld estimate, ms.
    pub fn deck_proxy_ms(&self) -> f64 {
        self.tick_frame_work().p99 * DECK_FACTOR
    }

    /// Tick cost summary.
    pub fn tick_cost(&self) -> Summary {
        let v: Vec<f64> = self.ticks_measured.iter().map(|t| t.ms).collect();
        Summary::of(&v)
    }

    /// Mean and largest live-object count per tick.
    pub fn live_objects(&self) -> (f64, u32) {
        let n = self.ticks_measured.len().max(1) as f64;
        let sum: u64 = self.ticks_measured.iter().map(|t| u64::from(t.live)).sum();
        let max = self
            .ticks_measured
            .iter()
            .map(|t| t.live)
            .max()
            .unwrap_or(0);
        (sum as f64 / n, max)
    }

    /// Presents more than 1.5 refresh periods apart (window runs).
    pub fn missed_vblanks(&self) -> usize {
        self.missed_vblank_times().len()
    }

    /// When each missed refresh happened, s after the warm-up.
    pub fn missed_vblank_times(&self) -> Vec<f64> {
        if self.display_hz <= 0.0 {
            return Vec::new();
        }
        let period = 1000.0 / self.display_hz;
        self.samples
            .present_ms
            .iter()
            .enumerate()
            .filter(|&(_, &d)| d > 1.5 * period)
            .map(|(i, _)| self.samples.present_at_s.get(i).copied().unwrap_or(-1.0))
            .collect()
    }

    /// Frames per second over the run.
    pub fn fps(&self) -> f64 {
        if self.seconds > 0.0 {
            self.samples.frames as f64 / self.seconds
        } else {
            0.0
        }
    }

    /// Frame work p99 inside the frame budget.
    pub fn work_within_budget(&self) -> bool {
        self.frame_work().p99 < FRAME_BUDGET_MS
    }

    /// The handheld estimate inside its budget.
    pub fn deck_within_budget(&self) -> bool {
        self.deck_proxy_ms() < DECK_BUDGET_MS
    }

    /// The report as one line of JSON.
    pub fn to_json(&self) -> String {
        let (live_mean, live_max) = self.live_objects();
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\"mode\": \"{}\", \"level\": {}, \"res\": \"{}x{}\", \"output\": \"{}x{}\", \
             \"presentation\": \"{}x{}\", \"scale\": {}, \"adapter\": {}, ",
            self.mode,
            json_string(&self.level),
            self.res.0,
            self.res.1,
            self.output.0,
            self.output.1,
            self.presentation.0,
            self.presentation.1,
            self.scale,
            json_string(&self.adapter),
        );
        if let Some((w, h)) = self.offscreen_copy {
            let _ = write!(s, "\"offscreen_copy\": \"{w}x{h}\", ");
        }
        if !self.present_mode.is_empty() {
            let _ = write!(s, "\"present_mode\": \"{}\", ", self.present_mode);
        }
        let _ = write!(
            s,
            "\"display_hz\": {:.2}, \"display_hz_source\": \"{}\", \"activation\": \"{}\", \
             \"seconds\": {:.2}, \"frames\": {}, \"fps\": {:.1}, \"ticks\": {}, \
             \"images\": {}, \"preloaded_sets\": {}, \"images_loaded_during_run\": {}, \
             \"load_ms\": {:.1}, \"frame_work_ms\": {}, \"tick_frame_work_ms\": {}, \
             \"tick_ms\": {}, \"live_objects\": {{\"mean\": {:.1}, \"max\": {}}}, \
             \"deck_proxy_ms\": {:.4}, ",
            self.display_hz,
            self.display_hz_source,
            self.activation,
            self.seconds,
            self.samples.frames,
            self.fps(),
            self.ticks,
            self.images,
            self.preloaded_sets,
            self.images_loaded_during_run,
            self.load_ms,
            self.frame_work().json(),
            self.tick_frame_work().json(),
            self.tick_cost().json(),
            live_mean,
            live_max,
            self.deck_proxy_ms(),
        );
        if self.mode == "window" {
            let _ = write!(
                s,
                "\"acquire_ms\": {}, \"present_interval_ms\": {}, \"missed_vblanks\": {}, \
                 \"missed_vblank_at_s\": [{}], \"resizes_during_run\": {}, ",
                Summary::of(&self.samples.acquire_ms).json(),
                Summary::of(&self.samples.present_ms).json(),
                self.missed_vblanks(),
                self.missed_vblank_times()
                    .iter()
                    .map(|t| format!("{t:.2}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                self.samples.resizes,
            );
        } else {
            let total: Vec<f64> = self
                .samples
                .work_ms
                .iter()
                .zip(&self.samples.gpu_ms)
                .map(|(w, g)| w + g)
                .collect();
            let _ = write!(
                s,
                "\"gpu_ms\": {}, \"frame_ms\": {}, ",
                Summary::of(&self.samples.gpu_ms).json(),
                Summary::of(&total).json(),
            );
        }
        let _ = write!(
            s,
            "\"budget\": {{\"frame_work_p99_under_ms\": {FRAME_BUDGET_MS}, \"frame_work_ok\": {}, \
             \"deck_proxy_under_ms\": {DECK_BUDGET_MS}, \"deck_proxy_ok\": {}",
            self.work_within_budget(),
            self.deck_within_budget(),
        );
        if self.mode == "window" {
            let _ = write!(s, ", \"missed_vblanks_ok\": {}", self.missed_vblanks() == 0);
        }
        s.push_str("}}");
        s
    }
}

/// A JSON string literal.
pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Runs the headless benchmark: loads the level, draws offscreen at
/// `settings.res` for `seconds` of wall time with a simulated `hz` display
/// clock, and returns the report.
pub fn run_headless(settings: &Settings, seconds: f64, hz: f64) -> Result<Report> {
    let started = Instant::now();
    let bundle = load_level(&settings.install, &settings.level)?;
    let mut graphics = load_graphics(&bundle, settings.pack.as_deref())?;
    let size = settings.res;
    let view = settings.view(size);
    let sim = new_sim(&bundle, settings.sim_config(size), settings.start);
    let activation = sim.config().activation.mode.name();
    let preloaded = preload(&mut graphics, &bundle.level, &sim.snapshot());
    let gpu = Gpu::headless().context("opening the GPU")?;
    let max = gpu.max_target();
    if size.0 > max || size.1 > max {
        anyhow::bail!(
            "--res {}x{} is larger than this GPU's largest image ({max} pixels a side)",
            size.0,
            size.1
        );
    }
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&graphics);
    let mut builder = SceneBuilder::new(&graphics, view, max);
    let output = Offscreen::new(&gpu, size);
    // The simulated clock runs faster than the wall clock when frames are
    // quick, so the route covers many times the run.
    let mut session = Session::new(sim).with_replay(route(seconds * 50.0 + 10.0));
    {
        let frame = session.interpolated(false);
        let scene = builder.build(&mut graphics, &FrameInput::from_frame(&frame, &view));
        renderer.render(&graphics, &scene, &output.view, COLOR_FORMAT);
        renderer.wait_idle();
    }
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    let images = graphics.images.records.len();
    session.time_ticks();

    let mut refresh = SimulatedRefresh::new(hz);
    let mut samples = Samples::default();
    let run = Instant::now();
    while run.elapsed().as_secs_f64() < seconds {
        let t0 = Instant::now();
        let dt = refresh.next().unwrap_or_default();
        let f = session.frame(dt);
        let frame = session.interpolated(settings.interpolation);
        let scene = builder.build(&mut graphics, &FrameInput::from_frame(&frame, &view));
        renderer.render(&graphics, &scene, &output.view, COLOR_FORMAT);
        let work = t0.elapsed().as_secs_f64() * 1000.0;
        renderer.wait_idle();
        let all = t0.elapsed().as_secs_f64() * 1000.0;
        samples.frame(work, f.ticks);
        samples.gpu_ms.push(all - work);
    }
    let seconds = run.elapsed().as_secs_f64();
    let comp = *builder.composition();
    Ok(Report {
        mode: "headless",
        level: bundle.name.clone(),
        res: size,
        output: size,
        offscreen_copy: None,
        presentation: comp.presentation,
        scale: comp.scale,
        adapter: gpu.describe(),
        present_mode: String::new(),
        display_hz: hz,
        display_hz_source: "simulated",
        seconds,
        ticks: session.ticks(),
        activation,
        images,
        preloaded_sets: preloaded,
        images_loaded_during_run: graphics.images.records.len() - images,
        load_ms,
        samples,
        ticks_measured: session.tick_samples().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_covers_the_run_and_moves_both_ways() {
        let s = route(10.0);
        assert!(s.ticks.len() as f64 * 18.0 >= 10_000.0);
        let any = |b: Buttons| s.ticks.iter().any(|t| t.held.contains(b));
        assert!(any(Buttons::LEFT) && any(Buttons::RIGHT) && any(Buttons::JUMP));
        assert!(any(Buttons::UP) && any(Buttons::DOWN));
        assert_eq!(route(10.0), s);
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let v: Vec<f64> = (1..=200).map(f64::from).collect();
        let s = Summary::of(&v);
        assert_eq!((s.p50, s.p99, s.max, s.n), (100.0, 198.0, 200.0, 200));
        assert_eq!(Summary::of(&[]).max, 0.0);
    }

    #[test]
    fn report_checks_the_budgets() {
        let mut r = Report {
            mode: "window",
            display_hz: 100.0,
            seconds: 2.0,
            ..Report::default()
        };
        for i in 0..200 {
            r.samples.frame(1.0 + f64::from(i) / 100.0, (i % 2) as u32);
            r.samples.present_ms.push(10.0);
        }
        assert!(r.work_within_budget());
        assert!((r.deck_proxy_ms() - 2.97 * 3.0).abs() < 1e-9);
        assert!(!r.deck_within_budget());
        assert_eq!(r.missed_vblanks(), 0);
        r.samples.present_ms.push(20.0);
        assert_eq!(r.missed_vblanks(), 1);
        assert_eq!(r.fps(), 100.0);
        let json = r.to_json();
        assert!(json.contains("\"missed_vblanks\": 1"), "{json}");
        assert!(json.contains("\"deck_proxy_ok\": false"), "{json}");
        assert!(json.starts_with('{') && json.ends_with('}'));
    }
}
