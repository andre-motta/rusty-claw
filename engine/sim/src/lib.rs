//! Headless, deterministic simulation stepped at a fixed 18 ms tick.
//!
//! ```no_run
//! use claw_assets::{Install, LevelData, LevelSource};
//! use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
//!
//! let install = Install::open("/path/to/your/install")?;
//! let store = install.store()?;
//! let level = LevelData::build(&store, LevelSource::Retail(1))?;
//! let mut sim = Sim::new(SimLevel::from_level(&level, &store)?, SimConfig::default());
//! for _ in 0..100 {
//!     let snapshot = sim.step(TickInput::default());
//!     println!("tick {} camera {:?}", snapshot.tick, snapshot.camera);
//! }
//! # Ok::<(), claw_assets::AssetError>(())
//! ```
//!
//! [`Sim::step`] takes one [`TickInput`] and returns a [`SimSnapshot`]; it
//! has no dependency on rendering, windows or time. Two runs with the same
//! level, settings and inputs produce the same snapshots.
//!
//! One tick runs, in order: the activation enter pass, the object update
//! pass (every live object's logic in depth order; Claw before the camera,
//! so the view never lags him by a tick), pair collision, and the
//! activation leave pass.
//!
//! What is here:
//!
//! - [`object`], [`container`], [`registry`] and [`activation`]: the object
//!   model, the depth-sorted live list with its update, destroy and re-sort
//!   passes, pair collision, the name-keyed logic registry and the
//!   activation windows.
//! - [`tiles`] and [`physics`]: per-pixel tile attributes and the movers
//!   that walk objects toward their requested positions.
//! - [`kinematics`], [`player`] and [`camera`]: Claw's floating-point
//!   motion, his state machine on the full state table, and the camera.
//! - [`tuning`]: every movement constant as data. Walking, running,
//!   climbing and jumping are tuned to cover the same distances in the same
//!   times as the original played at 60 frames per second; jump heights are
//!   those of the original (45 px tapped, 145 px held).
//! - [`input`]: the input latch that maps timestamped button events to
//!   ticks, and recorded input scripts; [`trace`]: per-tick traces.
//! - [`rng`]: the seeded random stream.
//!
//! Attacks, damage, pickups and enemies arrive in a later milestone; their
//! states are declared in the state table as fallback rows and their
//! objects run as inert placeholders.

#![forbid(unsafe_code)]

pub mod activation;
pub mod anim;
pub mod camera;
pub mod container;
pub mod input;
pub mod kinematics;
pub mod level;
pub mod object;
pub mod physics;
pub mod player;
pub mod registry;
pub mod rng;
pub mod sim;
pub mod tiles;
pub mod trace;
pub mod tuning;
pub mod view;

pub use claw_frame;
pub use claw_frame::{Buttons, SimSnapshot, TickInput, ViewConfig};

pub use activation::{ActivationConfig, ActivationMode};
pub use input::{InputLatch, InputScript};
pub use level::SimLevel;
pub use sim::{Sim, SimConfig};
pub use tiles::TileMap;
pub use trace::TraceRow;
pub use tuning::Tuning;
