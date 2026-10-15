//! Frame cost on level 1, drawn offscreen with the game's benchmark route
//! at a simulated 144 Hz display: the combat route through the stretch of
//! the level with the most enemies, with the status bar, and the sounds and
//! music played through a silent output.
//!
//! - At 3840x2160 and 5120x1440 the CPU work of a frame (input, ticks,
//!   interpolation, scene building and submission) stays under 6.9 ms at
//!   the 99th percentile, one refresh at 144 Hz, and the work of frames that
//!   run a tick, times 3 for a handheld-class CPU, stays under 8.3 ms.
//! - At 640x480 a whole frame, GPU included, takes under 16.7 ms at the
//!   99th percentile: 60 frames per second or more.
//! - No image is read or decoded while frames are drawn.
//! - Frames are paced to the simulated display: the run draws about 144
//!   frames a second, not as many as the GPU allows, and the route (played
//!   once) lasts the whole run.
//! - The route meets enemies, and the sounds play.
//!
//! The tick cost and the number of live objects are printed with each run,
//! also with widened activation at 5120x1440, which keeps more objects live.
//! Run under a shared lock when other GPU work runs on the machine.

use std::sync::Mutex;

use claw_acceptance::install;
use claw_game::bench::{DECK_BUDGET_MS, FRAME_BUDGET_MS, Report, Summary, run_headless};
use claw_game::settings::{LevelChoice, Settings};

const SECONDS: f64 = 3.0;

/// One benchmark at a time: the tests of this file run in parallel.
static GPU: Mutex<()> = Mutex::new(());

fn bench(res: (u32, u32), widen: bool) -> Option<Report> {
    let root = install()?;
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = Settings::new(root, LevelChoice::Retail(1));
    s.res = res;
    s.widen_activation = widen;
    let r = run_headless(&s, SECONDS, 144.0).expect("benchmark runs");
    let (mean, max) = r.live_objects();
    let tick = r.tick_cost();
    let w = r.frame_work();
    println!(
        "{}x{}{}: {} frames, work p50 {:.3} p99 {:.3} max {:.3} ms, tick frames p99 {:.3} ms \
         (handheld estimate {:.3} ms), tick p50 {:.4} p99 {:.4} max {:.4} ms at {mean:.0} \
         live objects (max {max}), {} activation, {:?}",
        res.0,
        res.1,
        if widen { " widened" } else { "" },
        r.samples.frames,
        w.p50,
        w.p99,
        w.max,
        r.tick_frame_work().p99,
        r.deck_proxy_ms(),
        tick.p50,
        tick.p99,
        tick.max,
        r.activation,
        r.combat,
    );
    println!("{}", r.to_json());
    println!("{}", r.summary());
    assert_eq!(r.images_loaded_during_run, 0, "images decoded mid-run");
    assert!(r.ticks > 100, "the route ran");
    assert_eq!(r.route_kind, "combat");
    assert!(
        r.route_start.is_some(),
        "the route starts at the combat stretch"
    );
    assert!(
        r.route_ticks as u64 >= r.ticks,
        "the route lasts the run: {} ticks for {}",
        r.route_ticks,
        r.ticks
    );
    assert_eq!(r.ticks_past_route, 0, "the route ran out");
    assert!(r.combat.enemies_live_max > 0, "enemies met: {:?}", r.combat);
    let audio = r.audio.clone().expect("the headless run plays sound");
    assert!(audio.sounds > 0 && audio.cues > 0, "{audio:?}");
    assert!(
        r.fps() <= 144.0 * 1.05,
        "{:.1} frames per second at a paced 144 Hz",
        r.fps()
    );
    assert!(
        r.summary()
            .contains("missed vblanks not measurable offscreen")
    );
    Some(r)
}

fn check_budgets(r: &Report) {
    assert!(
        r.frame_work().p99 < FRAME_BUDGET_MS,
        "frame work p99 {:.3} ms",
        r.frame_work().p99
    );
    assert!(
        r.deck_proxy_ms() < DECK_BUDGET_MS,
        "handheld estimate {:.3} ms",
        r.deck_proxy_ms()
    );
}

#[test]
fn install_bench_3840x2160() {
    let Some(r) = bench((3840, 2160), false) else {
        return;
    };
    check_budgets(&r);
}

#[test]
fn install_bench_5120x1440() {
    let Some(r) = bench((5120, 1440), false) else {
        return;
    };
    check_budgets(&r);
    let Some(w) = bench((5120, 1440), true) else {
        return;
    };
    check_budgets(&w);
    assert_eq!(w.activation, "widened");
    assert!(w.live_objects().1 >= r.live_objects().1);
}

#[test]
fn install_bench_640x480_holds_60_fps() {
    let Some(r) = bench((640, 480), false) else {
        return;
    };
    let whole: Vec<f64> = r
        .samples
        .work_ms
        .iter()
        .zip(&r.samples.gpu_ms)
        .map(|(w, g)| w + g)
        .collect();
    let f = Summary::of(&whole);
    assert!(f.p99 < 1000.0 / 60.0, "frame p99 {:.3} ms", f.p99);
    assert!(r.fps() >= 60.0, "{:.1} frames per second", r.fps());
    check_budgets(&r);
}
