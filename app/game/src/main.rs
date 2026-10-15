//! `rusty-claw`: play Captain Claw from your own install.
//!
//! ```text
//! rusty-claw --install PATH [--level N | --custom FILE.WWD] [--res WxH]
//!            [--fullscreen] [--aspect fill|centered|4:3] [--scale fit|integer]
//!            [--widen-activation] [--vsync on|off] [--fps-cap N | --uncapped]
//!            [--no-interp] [--no-audio] [--record FILE.csv]
//!            [--replay FILE.csv [--trace OUT.csv] [--render-hz HZ]]
//!            [--bench SECONDS [--headless] [--render-hz HZ]]
//! ```
//!
//! Run `rusty-claw --help` for every option; settings can also come from a
//! config file (see the `settings` module).
//!
//! A benchmark prints its report as one line of JSON on standard output and
//! a one-line PASS or FAIL verdict naming each budget on standard error. It
//! exits with status [`BUDGET_FAILED`] when a measured budget is missed.

use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use claw_game::bench::{Report, run_headless};
use claw_game::settings::{Args, Aspect, Mode, Settings};
use claw_game::window::{self, WindowMode};
use claw_game::{load_graphics, load_level, replay_headless};
use claw_sim::InputScript;

/// Exit status of a benchmark that missed a measured budget.
pub const BUDGET_FAILED: u8 = 3;

/// What a run ended with.
enum Outcome {
    /// Done.
    Done,
    /// A benchmark finished; the report decides the exit status.
    Bench(Box<Report>),
}

fn bench_result(report: Report) -> Outcome {
    println!("{}", report.to_json());
    eprintln!("rusty-claw: {}", report.summary());
    Outcome::Bench(Box::new(report))
}

fn read_script(path: &std::path::Path) -> Result<InputScript> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading input script {}", path.display()))?;
    InputScript::parse(&text).with_context(|| format!("input script {}", path.display()))
}

fn run(args: Args) -> Result<Outcome> {
    let settings = Settings::from_args(&args)?;
    if settings.widen_activation && settings.aspect != Aspect::Fill {
        eprintln!(
            "rusty-claw: --widen-activation applies to the fill aspect only; \
             objects stay active as in the original view"
        );
    }
    let started = Instant::now();
    match settings.mode.clone() {
        Mode::Replay {
            script,
            trace: Some(out),
            render_hz,
        } => {
            let script = read_script(&script)?;
            let bundle = load_level(&settings.install, &settings.level)?;
            let rows = replay_headless(&bundle, &settings, script, render_hz, None);
            std::fs::write(&out, claw_sim::trace::to_csv(&rows))
                .with_context(|| format!("writing {}", out.display()))?;
            eprintln!(
                "rusty-claw: replayed {} ticks of {} at a simulated {render_hz} Hz display, \
                 {} activation, trace {}",
                rows.len(),
                bundle.name,
                rows.first().map_or("original", |r| r.activation),
                out.display()
            );
            Ok(Outcome::Done)
        }
        Mode::Bench {
            seconds,
            headless: true,
            render_hz,
        } => {
            let report = run_headless(&settings, seconds, render_hz)?;
            Ok(bench_result(report))
        }
        mode => {
            let window_mode = match mode {
                Mode::Replay { script, .. } => WindowMode::Watch(read_script(&script)?),
                Mode::Bench { seconds, .. } => WindowMode::Bench(seconds),
                Mode::Play => WindowMode::Play,
            };
            let bundle = load_level(&settings.install, &settings.level)?;
            let graphics = load_graphics(&bundle, settings.pack.as_deref())?;
            for w in graphics.pack_warnings() {
                eprintln!("rusty-claw: image pack: {w}");
            }
            match window::run(settings, window_mode, bundle, graphics, started)? {
                Some(report) => Ok(bench_result(report)),
                None => Ok(Outcome::Done),
            }
        }
    }
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(Outcome::Done) => ExitCode::SUCCESS,
        Ok(Outcome::Bench(report)) if report.passed() => ExitCode::SUCCESS,
        Ok(Outcome::Bench(_)) => ExitCode::from(BUDGET_FAILED),
        Err(e) => {
            eprintln!("rusty-claw: {e:#}");
            ExitCode::FAILURE
        }
    }
}
