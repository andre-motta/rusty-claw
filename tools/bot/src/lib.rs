//! A deterministic search bot over the headless simulation: it finds
//! input routes that finish a level, for replays, regression tests and
//! comparing the game with a reference trace of the same route.
//!
//! ```no_run
//! use claw_assets::{Install, LevelData, LevelSource};
//! use claw_bot::{Goal, SearchConfig, search};
//! use claw_sim::{Sim, SimConfig, SimLevel};
//!
//! let install = Install::open("/path/to/your/install")?;
//! let store = install.store()?;
//! let level = LevelData::build(&store, LevelSource::Retail(1))?;
//! let sim = Sim::new(SimLevel::from_level(&level, &store)?, SimConfig::default());
//! let route = search(&sim, Goal::LevelComplete, &SearchConfig::default());
//! # Ok::<(), claw_assets::AssetError>(())
//! ```
//!
//! - [`search`]: best-first search over short macro-actions
//!   ([`actions`]), guided by the path distance to the level's end object
//!   on a coarse graph of the level's tiles ([`field`]). The game is only
//!   cloned and stepped: no move of Claw other than by input.
//! - [`route`]: the route file (the input script with `# level`,
//!   `# tuning`, `# seed` and behaviour header lines), the ROUTE.json
//!   report, playing a route back and its timing margin.
//!
//! Every held-mask change in a route is at least
//! [`actions::MIN_SPACING`] ticks after the previous one, so a 60 Hz
//! display never drops a press when the route is fed as key events.

#![forbid(unsafe_code)]

pub mod actions;
pub mod field;
pub mod route;
pub mod search;

pub use search::{Found, Goal, SearchConfig, SearchFailure, SearchStats, search, search_report};
