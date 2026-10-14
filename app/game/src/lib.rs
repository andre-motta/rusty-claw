//! The game: Captain Claw played from your own install.
//!
//! The binary `rusty-claw` is a thin front end over this library, which the
//! end-to-end tests drive directly:
//!
//! - [`settings`]: command line, config file and the view they describe;
//! - [`load`]: reading a retail or community custom level from the install;
//! - [`session`]: the simulation, the fixed-step frame driver and input;
//! - [`keymap`]: the keyboard map;
//! - [`window`]: the game window;
//! - [`bench`]: benchmarks in a window or offscreen;
//! - [`replay_headless`]: replaying an input script without a window.
//!
//! In this first playable version Claw stands, walks, runs, jumps, falls,
//! lands, crouches, looks up and climbs, with tile collision and a
//! following camera. Enemies, pickups, attacks, sound, menus and the status
//! bar come later.

use claw_frame::SimulatedRefresh;
use claw_sim::InputScript;
use claw_sim::trace::TraceRow;

pub mod bench;
pub mod keymap;
pub mod load;
pub mod session;
pub mod settings;
pub mod window;

pub use load::{LevelBundle, load_graphics, load_level, new_sim, preload};
pub use session::Session;
pub use settings::{Aspect, LevelChoice, Mode, Scale, Settings};

/// Replays `script` on `bundle` without a window, driving the frame loop
/// with a simulated `hz` display clock (with optional jitter of a fifth of
/// a frame from `jitter_seed`), and returns one trace row per script tick.
pub fn replay_headless(
    bundle: &LevelBundle,
    settings: &Settings,
    script: InputScript,
    hz: f64,
    jitter_seed: Option<u64>,
) -> Vec<TraceRow> {
    let sim = new_sim(bundle, settings.sim_config(settings.res), settings.start);
    let len = script.ticks.len();
    let mut session = Session::new(sim).with_replay(script);
    session.record_trace();
    let mut refresh = SimulatedRefresh::new(hz);
    if let Some(seed) = jitter_seed {
        refresh = refresh.with_jitter(0.2, seed);
    }
    while session.replaying() {
        session.frame(refresh.next().unwrap_or_default());
    }
    let mut rows = session.trace().to_vec();
    rows.truncate(len);
    rows
}
