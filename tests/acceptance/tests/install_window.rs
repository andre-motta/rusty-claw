//! A 640x480 game window holds 60 frames per second or more.
//!
//! Opens the game window at 640x480 on level 1 with the benchmark's combat
//! route for a few seconds and checks the presented frame rate and that the
//! route lasted the run. It also prints the
//! frame work and the number of frames that missed the display's refresh.
//! Windows must be opened from the main thread, so this file runs without
//! the test harness. It is skipped without `CLAW_INSTALL` or without a
//! display (neither `WAYLAND_DISPLAY` nor `DISPLAY` set).

use std::process::ExitCode;
use std::time::Instant;

use claw_acceptance::install;
use claw_game::settings::{LevelChoice, Settings};
use claw_game::window::{self, WindowMode};
use claw_game::{load_graphics, load_level};

const SECONDS: f64 = 3.0;

fn main() -> ExitCode {
    // Arguments meant for the test harness (a name filter) are ignored;
    // listing tests lists nothing here.
    if std::env::args().any(|a| a == "--list") {
        return ExitCode::SUCCESS;
    }
    let Some(root) = install() else {
        return ExitCode::SUCCESS;
    };
    if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipped: no display");
        return ExitCode::SUCCESS;
    }
    let started = Instant::now();
    let mut settings = Settings::new(root, LevelChoice::Retail(1));
    settings.res = (640, 480);
    let bundle = load_level(&settings.install, &settings.level).expect("level 1");
    let graphics = load_graphics(&bundle, None).expect("graphics");
    let report = match window::run(
        settings,
        WindowMode::Bench(SECONDS),
        bundle,
        graphics,
        started,
    ) {
        Ok(Some(r)) => r,
        Ok(None) => {
            eprintln!("install_window: no report");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("install_window: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}", report.to_json());
    println!("{}", report.summary());
    let fps = report.fps();
    println!(
        "640x480 window: {fps:.1} frames per second at a {:.1} Hz display ({}), {} missed \
         refreshes, frame work p99 {:.3} ms",
        report.display_hz,
        report.display_hz_source,
        report.missed_vblanks(),
        report.frame_work().p99
    );
    if report.route_kind != "combat" || report.route_ticks == 0 {
        eprintln!(
            "install_window: not the combat route: {}",
            report.route_kind
        );
        return ExitCode::FAILURE;
    }
    if report.ticks_past_route != 0 {
        eprintln!("install_window: the route ran out");
        return ExitCode::FAILURE;
    }
    if report.output != (640, 480) {
        eprintln!("install_window: the window is {:?}", report.output);
        return ExitCode::FAILURE;
    }
    if fps < 60.0 {
        eprintln!("install_window: {fps:.1} frames per second, under 60");
        return ExitCode::FAILURE;
    }
    println!("test install_window ... ok");
    ExitCode::SUCCESS
}
