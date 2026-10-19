//! The game: Captain Claw played from your own install.
//!
//! The binary `rusty-claw` is a thin front end over this library, which the
//! end-to-end tests drive directly:
//!
//! - [`settings`]: command line, config file and the view they describe,
//!   and writing changed options back to the config file;
//! - [`load`]: reading a retail or community custom level from the install;
//! - [`session`]: the simulation, the fixed-step frame driver and input;
//! - [`rope_swing`]: drawing Claw's rope swings smoothly between the
//!   rope's frames;
//! - [`keymap`]: the keyboard and gamepad maps, and rebinding;
//! - [`input`]: the input router every key and gamepad goes through, and
//!   recording the input of a game (its pauses, menus and end of a life
//!   included);
//! - [`saves`]: saved games;
//! - [`audio`]: playing the game's sounds;
//! - [`pause`]: what pauses play and what a pause holds;
//! - [`flow`]: the game flow: title, menus, play, the end-of-level tally
//!   and the end of a game;
//! - [`window`]: the game window;
//! - [`bench`]: benchmarks in a window or offscreen;
//! - [`replay_headless`]: replaying an input script without a window.
//!
//! The game starts at the title and its main menu (new game, load game,
//! options, quit) built from the game's own menu files and art; the
//! in-game menu pauses play and can end a life, finishing a level shows the treasure tally,
//! save points save the game, and options are written back to the config
//! file. Claw stands, walks, runs, jumps, climbs and fights, with sound,
//! music, the status bar, gamepads and rebindable keys.

use claw_frame::SimulatedRefresh;
use claw_sim::InputScript;
use claw_sim::trace::TraceRow;

pub mod audio;
pub mod bench;
pub mod flow;
pub mod input;
pub mod keymap;
pub mod load;
pub mod pause;
pub mod rope_swing;
pub mod saves;
pub mod session;
pub mod settings;
pub mod window;

pub use load::{LevelBundle, load_graphics, load_level, new_sim, preload};
pub use session::Session;
pub use settings::{Aspect, LevelChoice, Mode, Scale, Settings};

/// Replays `script` on `bundle` without a window, driving the frame loop
/// with a simulated `hz` display clock (with optional jitter of a fifth of
/// a frame from `jitter_seed`), and returns one trace row per script tick.
/// The script's pauses and menus hold play for their wall time on that
/// clock and its commands take effect before their ticks; the movement
/// tuning and the seed come from `settings` (see
/// [`Settings::follow_header`]).
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
