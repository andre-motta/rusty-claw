//! Tuning constants, kept as data so that feel decisions stay reversible.
//!
//! Every number that shapes movement lives here with its default. The
//! defaults follow the original game except where noted:
//!
//! - Horizontal and climbing speeds, the gravity and the jump-hold credit are
//!   tuned so that walking, running, climbing and jumping at the fixed 18 ms
//!   step cover the same distances in the same times as the original played
//!   at 60 frames per second (walking about 270 px/s, a tapped jump 45 px
//!   high, a fully held jump 145 px high). Players who learned the game at
//!   60 fps, speedrunners in particular, rely on that speed.
//! - The motion integrator keeps the original whole-pixel conversion, which
//!   counts the fractional part of each step twice; the nominal speeds below
//!   are chosen for it, so the effective speeds are higher than the nominal
//!   ones (see [`PlayerTuning::double_count_fraction`]).
//! - Behaviour fixes are switches in [`Behaviour`]: the original behaviour
//!   stays available, and is the default where players rely on it.

use crate::object::Rect;

/// Length of one simulation tick in milliseconds.
pub const TICK_MS: u32 = claw_frame::TICK_MS;

/// Movement constants of Captain Claw.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerTuning {
    /// Walking speed in px/s fed to the integrator; Claw covers about
    /// 270 px/s with the whole-pixel conversion.
    pub walk_speed: f64,
    /// Running speed after [`PlayerTuning::run_onset_ms`] of walking; covers
    /// about 330 px/s.
    pub run_speed: f64,
    /// Steering speed while jumping or falling; the same feel as walking.
    pub air_speed: f64,
    /// Climbing speed upwards; covers about 240 px/s.
    pub climb_up_speed: f64,
    /// Climbing speed downwards; covers about 220 px/s.
    pub climb_down_speed: f64,
    /// Gravity while jumping and falling, px/s^2.
    pub gravity: f64,
    /// Upper cap of the velocity, px/s; falling tops out at about
    /// 1040 px/s with the whole-pixel conversion.
    pub velocity_cap: f64,
    /// Largest displacement of one integration step, px.
    pub step_limit: f64,
    /// Height a tapped jump reaches, px.
    pub jump_base_height: i32,
    /// How long holding Jump keeps raising the jump, ms.
    pub jump_hold_window_ms: f64,
    /// Hold time credited for each tick Jump is held, ms. Slightly less
    /// than a tick, so intermediate holds reach the heights they reach at
    /// 60 frames per second; a full hold still reaches the full height.
    pub jump_hold_per_tick_ms: f64,
    /// Height gained per credited hold millisecond: walking or standing.
    pub jump_hold_factor: f64,
    /// Height gained per credited hold millisecond while running.
    pub jump_hold_factor_running: f64,
    /// Height gained per credited hold millisecond with the jump powerup.
    pub jump_hold_factor_powered: f64,
    /// Continuous walking time before running starts, ms.
    pub run_onset_ms: i32,
    /// Up held this long while standing starts looking up, ms.
    pub look_up_delay_ms: i32,
    /// Standing still this long plays the idle animation, ms.
    pub idle_delay_ms: i32,
    /// Crouching this long pans the camera down, ms.
    pub crouch_look_delay_ms: i32,
    /// Looking up this long pans the camera up, ms.
    pub look_up_look_delay_ms: i32,
    /// After a climb ends on the ground, looking up is blocked this long, ms.
    pub post_climb_lockout_ms: i32,
    /// A fall at least this high ends in the landing state, px.
    pub land_fall_height: i32,
    /// A fall higher than this shows the long-fall frame, px.
    pub long_fall_height: i32,
    /// Frame shown during a long fall.
    pub long_fall_frame: i32,
    /// Inset of the first interior ledge probe from the box edge, px.
    pub ledge_probe_inset: i32,
    /// Spacing of the further interior ledge probes, px.
    pub ledge_probe_stride: i32,
    /// Claw is kept this far inside each edge of the main plane, px.
    pub clamp_inset: i32,
    /// Draw depth.
    pub z: i32,
    /// Health after a respawn.
    pub respawn_health: i32,
    /// Chance in percent of the teeter voice line.
    pub teeter_voice_percent: u32,
    /// Height of the climb-over window above a ladder top, px.
    pub climb_over_height: i32,
    /// Height of each climb-over frame band, px.
    pub climb_over_band: i32,
    /// First climb-over frame (shown nearest the top).
    pub climb_over_first_frame: i32,
    /// Movement box while standing, walking, jumping and falling.
    pub move_rect: Rect,
    /// Movement box while crouching.
    pub crouch_move_rect: Rect,
    /// Hurt box facing right (mirrored when facing left).
    pub hit_rect: Rect,
    /// Hurt box while crouching, facing right.
    pub crouch_hit_rect: Rect,
    /// Attack box facing right.
    pub attack_rect: Rect,
    /// The whole-pixel conversion counts the fractional part of every step
    /// twice, as the original does; the jump timing and the speeds above
    /// depend on it. `false` carries the exact fraction instead.
    pub double_count_fraction: bool,
}

impl Default for PlayerTuning {
    fn default() -> Self {
        PlayerTuning {
            walk_speed: 246.0,
            run_speed: 304.0,
            air_speed: 246.0,
            climb_up_speed: 203.0,
            climb_down_speed: 193.5,
            gravity: 2450.0,
            velocity_cap: 1000.0,
            step_limit: 32.0,
            jump_base_height: 45,
            jump_hold_window_ms: 250.0,
            jump_hold_per_tick_ms: 16.7,
            jump_hold_factor: f64::from(0.58_f32),
            jump_hold_factor_running: f64::from(0.68_f32),
            jump_hold_factor_powered: f64::from(0.78_f32),
            run_onset_ms: 1500,
            look_up_delay_ms: 650,
            idle_delay_ms: 15000,
            crouch_look_delay_ms: 1000,
            look_up_look_delay_ms: 250,
            post_climb_lockout_ms: 1000,
            land_fall_height: 1000,
            long_fall_height: 600,
            long_fall_frame: 401,
            ledge_probe_inset: 12,
            ledge_probe_stride: 4,
            clamp_inset: 50,
            z: 4000,
            respawn_health: 100,
            teeter_voice_percent: 45,
            climb_over_height: 120,
            climb_over_band: 17,
            climb_over_first_frame: 389,
            move_rect: Rect::new(-16, -52, 16, 60),
            crouch_move_rect: Rect::new(-16, 10, 16, 60),
            hit_rect: Rect::new(-22, -45, 15, 52),
            crouch_hit_rect: Rect::new(-15, 10, 22, 52),
            attack_rect: Rect::new(-15, -45, 22, 34),
            double_count_fraction: true,
        }
    }
}

/// Camera constants.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraTuning {
    /// A target further than this on either axis is jumped to at once, px.
    pub snap_distance: i32,
    /// Vertical offset while crouching or looking up, px.
    pub look_offset: i32,
    /// Following speed while Claw is on the ground, px/s.
    pub follow_speed: u32,
    /// Following speed while Claw jumps or falls, px/s.
    pub air_speed: u32,
    /// Following speed for objects that fly, px/s (crosses any distance in
    /// one tick).
    pub fly_speed: u32,
    /// Panning speed of the look offsets, px/s.
    pub look_speed: u32,
    /// Speed toward a scripted camera point, px/s.
    pub point_speed: u32,
    /// Range of the shake countdown, ms (`rand % range`).
    pub shake_countdown_range: u32,
    /// Range of one shake jolt per axis, px (`rand % range - range / 2`).
    pub shake_jolt_range: u32,
}

impl Default for CameraTuning {
    fn default() -> Self {
        CameraTuning {
            snap_distance: 500,
            look_offset: 200,
            follow_speed: 500,
            air_speed: 4000,
            fly_speed: 99_999,
            look_speed: 250,
            point_speed: 400,
            shake_countdown_range: 36,
            shake_jolt_range: 13,
        }
    }
}

/// Engine physics constants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicsTuning {
    /// Largest step of the movement loop per axis, px.
    pub step: i32,
    /// Default probe spacing of a new object, px.
    pub move_res: i32,
}

impl Default for PhysicsTuning {
    fn default() -> Self {
        PhysicsTuning {
            step: 14,
            move_res: 10,
        }
    }
}

/// Behaviour switches: fixes of the original's quirks that a player can
/// choose. Each keeps the original behaviour available.
///
/// Every switch is off by default: the game plays as the original.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Behaviour {
    /// The mid-air hit jump fix. Off (the default), a hit reaction that
    /// ends in mid-air leaves Claw standing there, as in the original, so a
    /// jump press starts a second jump: any hit works, an enemy's shot or
    /// his own dynamite, and speedruns rely on it. On, Claw goes to the
    /// Land state instead (which drops into a fall on its next tick), so he
    /// cannot jump again before he lands. On the ground he stands either
    /// way.
    pub fix_double_jump: bool,
}

/// All tuning data of a simulation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tuning {
    /// Captain Claw's movement.
    pub player: PlayerTuning,
    /// The camera.
    pub camera: CameraTuning,
    /// Tile physics.
    pub physics: PhysicsTuning,
    /// Behaviour switches.
    pub behaviour: Behaviour,
}

impl Tuning {
    /// The original game's movement numbers, without the 60 fps tuning: the
    /// game as it plays at a fixed 18 ms step.
    pub fn original_speeds() -> Tuning {
        let mut t = Tuning::default();
        t.player.walk_speed = 225.0;
        t.player.run_speed = 285.0;
        t.player.air_speed = 225.0;
        t.player.climb_up_speed = 200.0;
        t.player.climb_down_speed = 200.0;
        t.player.gravity = 2250.0;
        t.player.jump_hold_per_tick_ms = f64::from(TICK_MS);
        t
    }
}
