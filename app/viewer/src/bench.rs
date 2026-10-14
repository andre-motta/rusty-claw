//! Offscreen rendering: the headless benchmark and screenshots.
//!
//! The benchmark drives the real frame loop (fixed 18 ms ticks, a simulated
//! display clock at `--bench-hz`, interpolation) with the camera panning
//! across the level, renders every frame offscreen at `--res`, paced to the
//! simulated display rate (or as fast as the machine allows with
//! `--uncapped`), and reports per-frame times in milliseconds; the pacing
//! wait is never part of them:
//!
//! - `frame_work_ms`: CPU work of a frame (tick, interpolation, scene
//!   building and command submission);
//! - `tick_frame_work_ms`: the same, for frames that ran at least one tick;
//! - `gpu_ms`: time from submission until the GPU finished the frame;
//! - `frame_ms`: both together, the time a frame takes before it can be
//!   presented;
//! - `deck_proxy_ms`: `tick_frame_work_ms.p99` times 3.0, an estimate of the
//!   same work on a handheld-class CPU.
//!
//! Each statistic has `p50`, `p99` and `max`. `sets_loaded_while_drawing`
//! counts sprite sets that were read and decoded during a frame instead of
//! at level load; it stays 0 when the preload is complete.
//!
//! An output size larger than the GPU can create is refused with an error
//! before anything is drawn.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use claw_frame::{FrameDriver, FramePacer, SimulatedRefresh, TickInput};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::{FrameInput, Gpu, Offscreen, Renderer, SceneBuilder};

use crate::stub::Stub;
use crate::{Args, load};

/// Per-frame CPU time on a handheld-class CPU relative to this machine.
pub const DECK_FACTOR: f64 = 3.0;

fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

fn stats(values: &mut [f64]) -> String {
    values.sort_by(f64::total_cmp);
    format!(
        "{{\"p50\": {:.4}, \"p99\": {:.4}, \"max\": {:.4}}}",
        percentile(values, 0.50),
        percentile(values, 0.99),
        values.last().copied().unwrap_or(0.0)
    )
}

fn json_string(s: &str) -> String {
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

/// Runs the headless benchmark for `seconds` and prints JSON.
pub fn run(args: &Args, seconds: f64) -> Result<()> {
    let started = Instant::now();
    let mut loaded = load(args)?;
    let size = args.size();
    let view = args.view(size);
    let gpu = Gpu::headless().context("opening the GPU")?;
    gpu.check_output(size, false)
        .context("choose a smaller --res")?;
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&loaded.graphics);
    let mut builder = SceneBuilder::new(&loaded.graphics, view, gpu.max_target());
    let output = Offscreen::new(&gpu, size);
    let mut stub = Stub::new(&loaded.level, view);
    stub.autopilot = true;
    let mut driver = FrameDriver::new(stub.snapshot());
    {
        // First frame: builds the pipelines (the sprites are preloaded).
        let frame = driver.latest();
        let input = FrameInput::from_frame(&frame, &view);
        let scene = builder.build(&mut loaded.graphics, &input);
        renderer.render(&loaded.graphics, &scene, &output.view, COLOR_FORMAT);
        renderer.wait_idle();
    }
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;

    let mut refresh = SimulatedRefresh::new(args.bench_hz);
    // `--uncapped` (or `--fps-cap 0`) asks for a throughput run.
    let mut pacer = match args.frame_cap() {
        Some(_) => FramePacer::new(args.bench_hz),
        None => FramePacer::uncapped(),
    };
    let (mut work, mut tick_work, mut gpu_ms, mut total) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut ticks = 0u64;
    let run_start = Instant::now();
    while run_start.elapsed().as_secs_f64() < seconds {
        pacer.wait();
        let t0 = Instant::now();
        let dt = refresh.next().unwrap_or_default();
        let report = driver.frame(dt, || stub.step(TickInput::default()));
        ticks += u64::from(report.ticks);
        let frame = if args.no_interp {
            driver.latest()
        } else {
            driver.interpolated()
        };
        let input = FrameInput::from_frame(&frame, &view);
        let scene = builder.build(&mut loaded.graphics, &input);
        renderer.render(&loaded.graphics, &scene, &output.view, COLOR_FORMAT);
        let w = t0.elapsed().as_secs_f64() * 1000.0;
        renderer.wait_idle();
        let all = t0.elapsed().as_secs_f64() * 1000.0;
        work.push(w);
        if report.ticks > 0 {
            tick_work.push(w);
        }
        gpu_ms.push(all - w);
        total.push(all);
    }
    let frames = work.len();
    let comp = builder.composition();
    let mut tick_sorted = tick_work.clone();
    tick_sorted.sort_by(f64::total_cmp);
    let deck = percentile(&tick_sorted, 0.99) * DECK_FACTOR;
    println!(
        "{{\"res\": \"{}x{}\", \"presentation\": \"{}x{}\", \"scale\": {}, \"adapter\": {}, \
         \"seconds\": {seconds}, \"display_hz\": {}, \"paced\": {}, \"interpolation\": {}, \"frames\": {frames}, \
         \"ticks\": {ticks}, \"images\": {}, \"sets_loaded_while_drawing\": {}, \
         \"load_ms\": {load_ms:.1}, \"frame_work_ms\": {}, \
         \"tick_frame_work_ms\": {}, \"gpu_ms\": {}, \"frame_ms\": {}, \"deck_proxy_ms\": {deck:.4}}}",
        size.0,
        size.1,
        comp.presentation.0,
        comp.presentation.1,
        comp.scale,
        json_string(&gpu.describe()),
        args.bench_hz,
        pacer.is_paced(),
        !args.no_interp,
        loaded.graphics.images.records.len(),
        loaded.graphics.loaded_while_drawing(),
        stats(&mut work),
        stats(&mut tick_work),
        stats(&mut gpu_ms),
        stats(&mut total),
    );
    Ok(())
}

/// Renders one frame offscreen at `--res` and saves it as a PNG.
pub fn screenshot(args: &Args, path: &Path) -> Result<()> {
    let mut loaded = load(args)?;
    let size = args.size();
    let view = args.view(size);
    let gpu = Gpu::headless().context("opening the GPU")?;
    gpu.check_output(size, true)
        .context("choose a smaller --res")?;
    let mut renderer = Renderer::new(&gpu);
    renderer.set_level(&loaded.graphics);
    let mut builder = SceneBuilder::new(&loaded.graphics, view, gpu.max_target());
    let mut stub = Stub::new(&loaded.level, view);
    if let Some((x, y)) = args.camera {
        stub.set_center(i64::from(x), i64::from(y));
    }
    let snap = stub.snapshot();
    let frame = claw_frame::interpolate(&snap, &snap, 0.0);
    let input = FrameInput::from_frame(&frame, &view);
    let scene = builder.build(&mut loaded.graphics, &input);
    let rgba = renderer.render_offscreen(&loaded.graphics, &scene);
    claw_render::pack::write_png(path, size.0, size.1, &rgba)?;
    eprintln!(
        "claw-viewer: wrote {} ({}x{}, view {}x{} at scale {})",
        path.display(),
        size.0,
        size.1,
        scene.comp.presentation.0,
        scene.comp.presentation.1,
        scene.comp.scale
    );
    Ok(())
}
