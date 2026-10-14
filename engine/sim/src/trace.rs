//! Per-tick traces: one CSV row per tick with Claw's position, state and
//! the camera, for comparing runs and checking movement.

use std::fmt::Write as _;

use crate::sim::Sim;
use crate::tuning::TICK_MS;

/// Column names of a trace.
pub const HEADER: &str = "tick,t_ms,dt_ms,x,y,frac_x,frac_y,state_id,physics_type,cam_x,cam_y,held,pressed,source,activation";

/// One trace row.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceRow {
    /// Index of the tick (0 for the first).
    pub tick: u64,
    /// Simulation time at the end of the tick, ms.
    pub t_ms: u64,
    /// Tick length, ms.
    pub dt_ms: u32,
    /// Claw's position.
    pub x: i32,
    /// Claw's position.
    pub y: i32,
    /// Horizontal fraction carried to the next tick.
    pub frac_x: f64,
    /// Vertical fraction carried to the next tick.
    pub frac_y: f64,
    /// Claw's state id.
    pub state_id: i32,
    /// Claw's physics type.
    pub physics_type: i32,
    /// Camera point.
    pub cam_x: i32,
    /// Camera point.
    pub cam_y: i32,
    /// Buttons held this tick.
    pub held: u32,
    /// Buttons pressed this tick.
    pub pressed: u32,
    /// Where the input came from.
    pub source: String,
    /// Activation mode name.
    pub activation: &'static str,
}

impl TraceRow {
    /// The row for the tick `sim` has just run, numbered `tick` (0 for the
    /// first tick of the recording).
    pub fn capture(sim: &Sim, tick: u64, source: &str) -> TraceRow {
        let (x, y, state_id, physics_type) = sim
            .player()
            .map_or((0, 0, -1, 0), |o| (o.x, o.y, o.state, o.physics_type));
        let mover = &sim.claw().mover;
        let (cam_x, cam_y) = sim.camera().current;
        let input = sim.input();
        TraceRow {
            tick,
            t_ms: (tick + 1) * u64::from(TICK_MS),
            dt_ms: TICK_MS,
            x,
            y,
            frac_x: mover.x.carry,
            frac_y: mover.y.carry,
            state_id,
            physics_type,
            cam_x,
            cam_y,
            held: input.held.0,
            pressed: input.pressed.0,
            source: source.to_string(),
            activation: sim.config().activation.mode.name(),
        }
    }

    /// The row as CSV (no line end).
    pub fn to_csv(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{},{},{},{},{},{:.6},{:.6},{},{},{},{},{},{},{},{}",
            self.tick,
            self.t_ms,
            self.dt_ms,
            self.x,
            self.y,
            self.frac_x,
            self.frac_y,
            self.state_id,
            self.physics_type,
            self.cam_x,
            self.cam_y,
            self.held,
            self.pressed,
            self.source,
            self.activation
        );
        s
    }
}

/// A whole trace as CSV text with a header line.
pub fn to_csv(rows: &[TraceRow]) -> String {
    let mut out = String::with_capacity(64 * (rows.len() + 1));
    out.push_str(HEADER);
    out.push('\n');
    for r in rows {
        out.push_str(&r.to_csv());
        out.push('\n');
    }
    out
}
