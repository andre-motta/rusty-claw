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
//! - `install_lift`: a defeated officer of the first level lifted, carried
//!   and thrown, then removed without a second loot drop;
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
//!   (skipped without a display);
//! - `install_reference`: every route in `tolerances/references.toml`
//!   replayed against its reference traces of the same route, in the same
//!   trace format, read from the folder `CLAW_REFERENCE` names (skipped when
//!   that is unset): every column stays within its tolerance up to the
//!   first divergence the file records for it. `make compare
//!   REF=<level>/<name>` runs one route's comparison and prints it;
//! - `install_differential`: the gate. The same routes played through the
//!   game's own frame loop at 60 Hz and compared at the gate's preset
//!   (exact at one 18 ms step per frame, feel at 60 frames per second),
//!   failing with the first divergence `references.toml` does not list;
//!   the trace equals the one played tick by tick.
//!
//! [`play`] holds what the route tests share: a recorded player and the
//! replay of what it recorded through the game's frame loop. [`reference`]
//! reads reference stores and compares the game with them.
//!
//! GPU tests should run under a shared lock when other GPU work runs on the
//! machine, for example `flock /path/to/lock make test-install ...`.

pub mod play;
pub mod reference;

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
