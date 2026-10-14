//! Camera rules inside the simulation: following, the snap over 500 px,
//! the look offsets and their delays.

mod common;

use claw_frame::interpolate;
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, TickInput};
use common::*;

#[test]
fn follows_claw_without_lag_and_settles_on_stop() {
    let rows = run_level(
        flat_level(),
        &script(&[(RIGHT, 2000), (0, 1500), (LEFT, 1000), (0, 1500)]),
    );
    let (a, b) = hold_window(&rows, RIGHT);
    let lag: Vec<f64> = rows
        .iter()
        .filter(|r| r.t_ms >= a + 1000 && r.t_ms <= b)
        .map(|r| f64::from(r.x - r.cam_x))
        .collect();
    let mean_lag = lag.iter().sum::<f64>() / lag.len() as f64;
    let settle = rows
        .iter()
        .filter(|r| r.t_ms > b && r.held & (LEFT | RIGHT) == 0)
        .find(|r| (r.cam_x - r.x).abs() <= 2)
        .unwrap()
        .t_ms
        - b;
    check(&[
        ("cam_follow_lag_px", mean_lag),
        ("cam_settle_ms", settle as f64),
    ]);
    // The camera point is Claw's position while he stands.
    let last = rows.last().unwrap();
    assert_eq!((last.cam_x, last.cam_y), (last.x, last.y));
}

#[test]
fn jumps_over_500_px_snap_in_one_tick() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 30);
    let before = sim.camera().current;
    let (x, y) = {
        let o = sim.player().unwrap();
        (o.x, o.y)
    };
    let prev = sim.snapshot();
    assert!(!prev.camera_teleported);
    sim.teleport_player(x + 900, y);
    let snap = sim.step(TickInput::default());
    let after = sim.camera().current;
    assert!(sim.camera().teleported);
    assert!(snap.camera_teleported, "the snapshot reports the snap");
    assert!(snap.renderables.iter().any(|r| r.teleported));
    // Drawn between the two ticks, the view is already at the new place.
    for alpha in [0.0, 0.3, 0.7] {
        assert_eq!(interpolate(&prev, &snap, alpha).camera, snap.camera);
    }
    check(&[
        ("cam_snap_first_step_px", f64::from(after.0 - before.0)),
        (
            "cam_snap_settle_ms",
            if after.0 == x + 900 { 0.0 } else { 18.0 },
        ),
    ]);
    // Within 500 px the camera approaches at 9 px per tick instead.
    sim.teleport_player(x + 1300, y);
    let snap = sim.step(TickInput::default());
    assert_eq!(sim.camera().current.0, x + 909);
    assert!(!sim.camera().teleported);
    assert!(!snap.camera_teleported);
}

fn pan(mask: u32) -> (Vec<Row>, i32, u64, u64) {
    let rows = run_level(flat_level(), &script(&[(mask, 3500), (0, 2000)]));
    let moving: Vec<&Row> = rows.iter().skip(1).filter(|r| r.cam_y != r.y).collect();
    let peak = moving
        .iter()
        .map(|r| r.cam_y - r.y)
        .max_by_key(|o| o.abs())
        .unwrap();
    let delay = moving[0].t_ms;
    let entered = rows
        .iter()
        .find(|r| r.state == state::LOOK_UP)
        .map_or(0, |r| r.t_ms);
    (rows, peak, delay, entered)
}

#[test]
fn crouching_pans_down_after_a_second() {
    let (rows, peak, delay, _) = pan(DOWN);
    check(&[
        ("crouch_max_offset_px", f64::from(peak)),
        ("crouch_pan_delay_ms", delay as f64),
    ]);
    assert!(rows.iter().any(|r| r.state == state::CROUCH));
    let last = rows.last().unwrap();
    assert_eq!(last.cam_y, last.y, "returns after release");
}

#[test]
fn looking_up_pans_up() {
    let (rows, peak, delay, entered) = pan(UP);
    check(&[
        ("lookup_max_offset_px", f64::from(peak)),
        ("lookup_pan_delay_ms", delay as f64),
        ("lookup_state_delay_ms", entered as f64),
    ]);
    let last = rows.last().unwrap();
    assert_eq!(last.cam_y, last.y);
}

#[test]
fn jumping_moves_the_camera_with_claw() {
    let rows = run_level(flat_level(), &script(&[(JUMP, 800), (0, 1000)]));
    assert!(
        rows.iter().skip(1).all(|r| r.cam_y == r.y),
        "vertical lag stays 0"
    );
}

#[test]
fn scripted_point_and_shake() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    idle(&mut sim, 2);
    let start = sim.camera().current;
    sim.set_camera_point(Some((start.0 + 100, start.1)));
    sim.step(TickInput::default());
    assert_eq!(
        sim.camera().current.0,
        start.0 + 7,
        "400 px/s toward a point"
    );
    sim.set_camera_point(None);
    idle(&mut sim, 20);
    assert_eq!(sim.camera().current, start);
    let rng_before = sim.rng().state();
    sim.start_shake(500);
    sim.step(TickInput::default());
    assert_ne!(
        sim.rng().state(),
        rng_before,
        "the shake draws from the game stream"
    );
    idle(&mut sim, 60);
    assert_eq!(sim.camera().current, start, "the jolts are pulled back");
}

#[test]
fn snapshot_camera_is_the_view_corner() {
    let mut sim = Sim::new(flat_level(), SimConfig::default());
    let snap = sim.step(TickInput::default());
    let (cx, cy) = sim.camera().current;
    let r = sim.view().rect;
    assert_eq!(
        (snap.camera.x, snap.camera.y),
        (f64::from(r.left), f64::from(r.top))
    );
    assert_eq!(r.left, cx - 320);
    // Near the bottom of this short level the view is pushed back inside.
    assert_eq!(r.bottom, sim.level().map.height() - 1);
    assert!(r.top < cy - 240);
}
