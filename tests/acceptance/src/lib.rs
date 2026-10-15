//! End-to-end acceptance tests of the game, in `tests/`.
//!
//! Every test reads your own install at run time through the `CLAW_INSTALL`
//! environment variable and skips itself when it is unset; nothing from an
//! install is stored in the repository. Run them with
//!
//! ```text
//! CLAW_INSTALL=/path/to/your/install make test-install MODULE=acceptance
//! ```
//!
//! - `install_level1`: the first level from start to finish on a chained
//!   route built from the level's records (every enemy killed, every flag
//!   touched, a death and the respawn, crates, kegs, treasure and the
//!   tally, the elevator and the toggling pegs, the map piece), replayed
//!   identically at 60, 144 and 240 Hz with jitter; every logic of the
//!   level implemented, the preload complete, and a stretch with widened
//!   activation;
//! - `install_combat`: each weapon against each enemy of the first level;
//! - `install_flow`: a saved game round trip and the end-of-level tally
//!   through the game flow, in a temporary data folder;
//! - `install_smoke`: every retail level survives a minute of random input;
//! - `install_load`: every retail level loads in under 3 s, and the visible
//!   tile grids at the original and the wide views match the reference grids
//!   in `CLAW_GOLDEN_DIR` (skipped when that is unset);
//! - `install_replay`: the movement scenarios (walk, run, jumps, falls,
//!   crouch and look-up, camera, climb) land within the movement tolerances
//!   (`CLAW_TOLERANCES` names another table), and their traces are identical
//!   at 60, 144 and 240 Hz;
//! - `install_bench`: frame work and its handheld estimate at 3840x2160,
//!   5120x1440 and 640x480 on the combat route, drawn offscreen;
//! - `install_custom`: community custom levels load and play, their scripts
//!   untouched, and names that leave the custom level folder are refused;
//! - `install_window`: a 640x480 window holds 60 frames per second or more
//!   (skipped without a display).
//!
//! [`play`] holds what the route tests share: a recorded player and the
//! replay of what it recorded through the game's frame loop.
//!
//! GPU tests should run under a shared lock when other GPU work runs on the
//! machine, for example `flock /path/to/lock make test-install ...`.

pub mod play;

/// Reads `CLAW_INSTALL`, or prints a skip note and returns `None`.
pub fn install() -> Option<std::path::PathBuf> {
    match std::env::var_os("CLAW_INSTALL") {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => {
            eprintln!("skipped: CLAW_INSTALL is not set");
            None
        }
    }
}
