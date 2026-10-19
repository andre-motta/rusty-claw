//! A warp door on a synthetic floor with a powder keg awake beside it: the
//! warp between two ticks runs every other awake object once more with no
//! game time before Claw's first tick at the target, with the frame pulse
//! of the tick that touched the door, and sends the objects by the door to
//! sleep before that first tick.
//!
//! The keg makes the enemies' per-tick roll on every frame pulse tick, so
//! the draws between two ticks count its calls: a door touched on a pulse
//! tick makes it roll once more in the warp; when the first tick at the
//! target is a pulse tick it is already asleep there and makes none.

mod common;

use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss_stage;
use claw_sim::object::ObjectId;
use claw_sim::rng::Rng;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

/// Where the door leads: far from the start, out of reach of the
/// activation around the door.
const TARGET_X: i32 = 8000;
/// The door, 150 px right of the start.
const DOOR_X: i32 = 1150;
/// The keg, 100 px behind Claw's start.
const KEG_X: i32 = 900;

/// The flat level with a door at [`DOOR_X`] leading to [`TARGET_X`] and a
/// powder keg standing at [`KEG_X`]; Claw starts at x 1000.
fn door_level() -> SimLevel {
    let y = {
        let mut sim = Sim::new(flat_level(), SimConfig::default());
        idle(&mut sim, 40);
        sim.player().unwrap().y
    };
    let mut level = flat_level();
    level.start = (1000, y);
    let mut door = ObjectSpec::at(boss_stage::WARP, DOOR_X, y, 0);
    door.record[55] = TARGET_X;
    door.record[56] = y;
    level.objects.push(door);
    let mut keg = ObjectSpec::at("PowderKeg", KEG_X, 890, 0);
    keg.image_set = "LEVEL_POWDERKEG".to_string();
    keg.frame = 1;
    keg.record[0] = KEG_X;
    level.objects.push(keg);
    level
}

fn keg(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == "PowderKeg")
        .map(|(id, _)| id)
        .expect("the keg")
}

/// The number of draws that take the random stream from `from` to `to`.
fn draws(from: u32, to: u32) -> usize {
    let mut rng = Rng::new(from);
    for n in 0..64 {
        if rng.state() == to {
            return n;
        }
        rng.next_u15();
    }
    panic!("{to} is not within 64 draws of {from}");
}

/// One walk into the door after `wait` idle ticks.
struct Walk {
    /// The random state and the pulse after the tick before the one that
    /// touched the door.
    before_touch: (u32, bool),
    /// The same after the tick that touched the door.
    touch: (u32, bool),
    /// The same after the first tick at the target.
    arrival: (u32, bool),
    /// The keg was listed (awake) after the tick that touched the door,
    /// and after the first tick at the target.
    keg_listed: (bool, bool),
}

fn walk(wait: usize) -> Walk {
    let mut sim = pressing_sim(door_level(), SimConfig::default());
    idle(&mut sim, 10 + wait);
    let k = keg(&sim);
    let listed = |sim: &Sim| sim.objects().listed().contains(&k);
    let mut rows = vec![(sim.rng_state(), sim.pulse(), listed(&sim))];
    for _ in 0..120 {
        sim.step(key_down(&sim, RIGHT));
        rows.push((sim.rng_state(), sim.pulse(), listed(&sim)));
        if sim.player().unwrap().x >= TARGET_X - 100 {
            let n = rows.len();
            let at = |i: usize| (rows[i].0, rows[i].1);
            return Walk {
                before_touch: at(n - 3),
                touch: at(n - 2),
                arrival: at(n - 1),
                keg_listed: (rows[n - 2].2, rows[n - 1].2),
            };
        }
    }
    panic!("the door never moved Claw");
}

#[test]
fn a_warp_runs_the_awake_objects_once_more_with_the_touching_ticks_pulse() {
    let mut touched_on_pulse = false;
    let mut arrived_on_pulse = false;
    for wait in 0..3 {
        let w = walk(wait);
        assert_eq!(
            w.keg_listed,
            (true, false),
            "wait {wait}: awake by the door, asleep at the target"
        );
        // The keg rolls in the tick that touches the door on a pulse.
        assert_eq!(
            draws(w.before_touch.0, w.touch.0),
            usize::from(w.touch.1),
            "wait {wait}: the touching tick"
        );
        // Between that tick and the end of the first tick at the target:
        // the keg's roll in the warp when the touching tick was a pulse
        // tick, and nothing in the first tick at the target, pulse or not,
        // since the keg went to sleep in the warp.
        assert_eq!(
            draws(w.touch.0, w.arrival.0),
            usize::from(w.touch.1),
            "wait {wait}: the warp and the first tick at the target"
        );
        touched_on_pulse |= w.touch.1;
        arrived_on_pulse |= w.arrival.1;
    }
    assert!(touched_on_pulse && arrived_on_pulse, "both cases seen");
}
