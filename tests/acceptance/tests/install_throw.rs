//! Lifting and throwing on the first level against the original's runs of
//! the same inputs, read from the reference store (skipped without
//! `CLAW_INSTALL` or `CLAW_REFERENCE`).
//!
//! The store holds ten throw routes (`level1/throw-*`): Claw lifts a powder
//! keg on the long upper floor or an officer on the lower floor, presses the
//! lift key again and holds it for one tick (a tap), 0.5 s, 1.5 s or 3 s
//! (and 3.6 s for the keg, past the limit); one more route holds the lift
//! key from the lift on without a second press. Each route is played tick by
//! tick from where the original's Claw stood at the route's time zero, with
//! the original speeds and its random state, and checked against the
//! reference trace and the original's numbers for the same route. The keys
//! are the ones the reference trace records as held on each row, a key
//! counting as pressed on the first row it is held: for three of the
//! officer routes the store keeps the route file of an earlier version
//! (the lift on rows 15 and 16, with no walk first) beside the traces of
//! the version that was played (a walk right on rows 1 to 6, the lift on
//! row 9).
//!
//! - the two-stage throw: the wind-up picture (Claw's image 44) on the
//!   press tick and every tick the key stays down, the release and the
//!   throw picture (image 45) on the first tick without the key, and Stand
//!   21 ticks later; a hold of 167 ticks throws by itself at full power
//!   with the key still down; a key held since the lift throws nothing;
//! - Claw's state on every row equals the reference trace's;
//! - the flight: where the object is released, how high it goes and on
//!   which row, and where and on which row it first lands. A thrown keg
//!   lands with its carrying box, 4 px higher than a lying keg stands (Y
//!   2102 on that floor), and explodes there. A thrown officer lands at the
//!   speed the move it made gives (827.04 px/s, not 840, on the tap), and,
//!   thrown into the step at x 2697 on its way down, lands where it was
//!   when the step's corner caught it.

use std::path::{Path, PathBuf};

use claw_acceptance::install;
use claw_acceptance::reference::{
    Clock, Store, Value, load, parse_tables, read_route, reference_root, replay_config, start_game,
};
use claw_frame::{Buttons, TickInput};
use claw_sim::logic::{enemy, keg};
use claw_sim::object::ObjectId;
use claw_sim::physics::ptype;
use claw_sim::player::progress::parse_start_lines;
use claw_sim::player::{special, state};
use claw_sim::rng::Rng;
use claw_sim::trace;
use claw_sim::tuning::TICK_MS;

const TICK_MS_U64: u64 = TICK_MS as u64;

/// One tick of a throw route: Claw's state and image, and the thrown
/// object's position, physics type and speeds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Row {
    claw_state: i32,
    claw_frame: i32,
    obj: Option<Obj>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Obj {
    x: i32,
    y: i32,
    pt: i32,
    vx: f64,
    vy: f64,
    /// The keg exploding, or the enemy's state id.
    state: i32,
}

/// A route of the store played in the game, with its reference trace's
/// Claw states.
struct Run {
    rows: Vec<Row>,
    reference_states: Vec<i32>,
}

fn setup() -> Option<(PathBuf, PathBuf)> {
    let install = install()?;
    let root = reference_root()?;
    Some((install, root))
}

/// The game time at the route's time zero in the original, ms (the
/// store's `t0_ms`).
fn t0_ms(root: &Path, store: &str) -> u64 {
    let text = std::fs::read_to_string(root.join(store).join("meta.toml")).expect("meta.toml");
    parse_tables(&text)
        .expect("meta.toml reads")
        .iter()
        .find(|t| t.name == "c18")
        .and_then(|t| t.get("t0_ms"))
        .and_then(Value::int)
        .expect("t0_ms") as u64
}

/// Plays `level1/throw-<name>` tick by tick from where the original's
/// Claw stood at time zero, following the first `logic` object Claw lifts.
/// With `on_clock` the route starts at the original's game time too: Claw
/// waits there until the game time is the store's `t0_ms`, and the random
/// stream is set to its state at time zero again. A flying keg's steps
/// read the game clock, so the keg routes start on it; an officer beside
/// Claw would strike him while he waits, so the officer routes start once
/// he has settled.
fn play(install: &Path, root: &Path, name: &str, logic: &str, on_clock: bool) -> Run {
    let store_name = format!("level1/throw-{name}");
    let store = Store::open(root, &store_name).unwrap_or_else(|e| panic!("{e}"));
    let run = store.run(Clock::C18).expect("a c18 run");
    let (route_text, _) = read_route(&store.route).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(&run.traces[0]).expect("the reference trace");
    let reference = trace::parse(&text).expect("a trace");
    let start = (reference[0].x, reference[0].y);
    let cfg = replay_config(run, &route_text);
    let bundle = load(install, store.level).unwrap_or_else(|e| panic!("{e}"));
    let carry = parse_start_lines(&route_text).expect("start lines");
    let seed = cfg.seed;
    let mut sim = start_game(&bundle, cfg, Some(start), carry).unwrap_or_else(|e| panic!("{e}"));
    if on_clock {
        let t0 = t0_ms(root, &store_name);
        assert!(
            sim.tick() * TICK_MS_U64 <= t0,
            "{name}: settled after time zero"
        );
        while sim.tick() * TICK_MS_U64 < t0 {
            sim.step(TickInput::default());
        }
        sim.with_world(|w| *w.rng = Rng::new(seed));
    }
    let mut carried: Option<ObjectId> = None;
    let mut rows = Vec::new();
    let mut down = 0;
    for r in &reference {
        sim.step(TickInput {
            held: Buttons(r.held),
            pressed: Buttons(r.held & !down),
        });
        down = r.held;
        if carried.is_none() {
            carried = sim
                .objects()
                .all()
                .find(|(_, o)| &*o.logic == logic && o.physics_type == ptype::DIRECT)
                .map(|(id, _)| id);
        }
        let c = sim.player().expect("Claw");
        let obj = carried.and_then(|id| sim.objects().get(id)).map(|o| {
            let (vx, vy, st) = if let Some(d) = keg::data_of(o) {
                let exploding = i32::from(d.phase == keg::Phase::Exploding);
                (d.vx, d.vy, exploding)
            } else {
                let d = enemy::data_of(o).expect("an enemy");
                (d.vx, d.vy, o.state)
            };
            Obj {
                x: o.x,
                y: o.y,
                pt: o.physics_type,
                vx,
                vy,
                state: st,
            }
        });
        rows.push(Row {
            claw_state: c.state,
            claw_frame: c.frame,
            obj,
        });
    }
    Run {
        rows,
        reference_states: reference.iter().map(|r| r.state_id).collect(),
    }
}

/// The rows (first and last) on which Claw shows `frame`, checking they
/// are one span.
fn span(run: &Run, frame: i32) -> (usize, usize) {
    let rows: Vec<usize> = (0..run.rows.len())
        .filter(|&k| run.rows[k].claw_frame == frame && run.rows[k].claw_state != state::STAND)
        .collect();
    let (first, last) = (rows[0], *rows.last().unwrap());
    assert_eq!(rows.len(), last - first + 1, "image {frame} in one span");
    (first, last)
}

/// Where the thrown object first lands after `release`: its row and
/// position.
fn landing(run: &Run, release: usize) -> (usize, i32, i32) {
    (release..run.rows.len())
        .find_map(|k| {
            let o = run.rows[k].obj?;
            (o.pt == ptype::STAND).then_some((k, o.x, o.y))
        })
        .expect("it lands")
}

/// The highest point of the flight from `release` until `land`: the first
/// row at the least Y, and that Y.
fn peak(run: &Run, release: usize, land: usize) -> (usize, i32) {
    let mut best = (release, i32::MAX);
    for k in release..land {
        let y = run.rows[k].obj.expect("flying").y;
        if y < best.1 {
            best = (k, y);
        }
    }
    best
}

fn states_match(name: &str, run: &Run) {
    for (k, (row, &want)) in run.rows.iter().zip(&run.reference_states).enumerate() {
        assert_eq!(row.claw_state, want, "{name}: Claw's state on row {k}");
    }
}

/// The original's numbers for one throw: the lift row, the release row,
/// the object's position on the release row, its peak row and Y, and its
/// first landing row and position.
struct Throw {
    name: &'static str,
    lift: usize,
    press: usize,
    power: i32,
    release: usize,
    release_at: (i32, i32),
    peak: (usize, i32),
    land: (usize, i32, i32),
}

const KEG_THROWS: [Throw; 5] = [
    Throw {
        name: "keg_tap",
        lift: 30,
        press: 80,
        power: 0,
        release: 81,
        release_at: (9872, 1995),
        peak: (95, 1916),
        land: (121, 9731, 2102),
    },
    Throw {
        name: "keg_h500",
        lift: 30,
        press: 80,
        power: 16,
        release: 108,
        release_at: (9871, 1994),
        peak: (123, 1906),
        land: (150, 9688, 2102),
    },
    Throw {
        name: "keg_h1500",
        lift: 30,
        press: 80,
        power: 49,
        release: 164,
        release_at: (9869, 1993),
        peak: (181, 1884),
        land: (209, 9594, 2102),
    },
    Throw {
        name: "keg_h3000",
        lift: 30,
        press: 80,
        power: 99,
        release: 247,
        release_at: (9867, 1991),
        peak: (268, 1846),
        land: (297, 9428, 2102),
    },
    Throw {
        name: "keg_h3600",
        lift: 30,
        press: 80,
        power: 100,
        release: 248,
        release_at: (9867, 1991),
        peak: (268, 1846),
        land: (298, 9426, 2102),
    },
];

const OFFICER_THROWS: [Throw; 4] = [
    Throw {
        name: "officer_tap",
        lift: 9,
        press: 70,
        power: 0,
        release: 71,
        release_at: (2486, 4807),
        peak: (85, 4728),
        land: (110, 2619, 4899),
    },
    Throw {
        name: "officer_h500",
        lift: 9,
        press: 70,
        power: 16,
        release: 98,
        release_at: (2487, 4806),
        peak: (113, 4718),
        land: (134, 2643, 4828),
    },
    Throw {
        name: "officer_h1500",
        lift: 9,
        press: 70,
        power: 49,
        release: 154,
        release_at: (2489, 4805),
        peak: (171, 4696),
        land: (188, 2695, 4765),
    },
    Throw {
        name: "officer_h3000",
        lift: 9,
        press: 70,
        power: 99,
        release: 237,
        release_at: (2491, 4803),
        peak: (258, 4658),
        land: (277, 2697, 4761),
    },
];

/// The two stages, the speeds at release and the flight of one throw.
fn check_throw(t: &Throw, run: &Run, facing_right: bool) {
    let name = t.name;
    states_match(name, run);
    // The lift: image 42 for 14 ticks, then 43 while carried.
    assert_eq!(span(run, 42), (t.lift, t.lift + 13), "{name}: lifting");
    assert_eq!(
        span(run, 43),
        (t.lift + 14, t.press - 1),
        "{name}: carrying"
    );
    // The wind-up from the press to the last tick the key is down, the
    // throw picture from the release for 21 ticks, then Stand.
    assert_eq!(
        span(run, 44),
        (t.press, t.release - 1),
        "{name}: winding up"
    );
    assert_eq!(
        span(run, 45),
        (t.release, t.release + 20),
        "{name}: throwing"
    );
    assert_eq!(run.rows[t.release + 21].claw_state, state::STAND, "{name}");
    // Nothing leaves before the release; it leaves at the speeds of its
    // power, its vertical speed already one tick of gravity on.
    let before = run.rows[t.release - 1].obj.expect("carried");
    assert_eq!(
        before.pt,
        ptype::DIRECT,
        "{name}: carried until the release"
    );
    let o = run.rows[t.release].obj.expect("thrown");
    let (vx, vy) = special::throw_velocity(t.power * 30, facing_right);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    assert!(
        near(o.vx, f64::from(vx)) && near(o.vy, f64::from(vy) + 36.0),
        "{name}: speeds ({}, {}) for power {}",
        o.vx,
        o.vy,
        t.power
    );
    assert_eq!((o.x, o.y), t.release_at, "{name}: released at");
    let land = landing(run, t.release);
    assert_eq!(land, t.land, "{name}: first landing");
    assert_eq!(peak(run, t.release, land.0), t.peak, "{name}: peak");
}

#[test]
fn a_keg_throw_winds_up_while_the_key_is_down_and_flies_as_in_the_original() {
    let Some((install, root)) = setup() else {
        return;
    };
    for t in &KEG_THROWS {
        let run = play(&install, &root, t.name, "PowderKeg", true);
        check_throw(t, &run, false);
        // It lands with its carrying box, 4 px above a lying keg, and
        // explodes there on the next tick.
        let (land, x, y) = t.land;
        let o = run.rows[land].obj.expect("landed");
        assert_eq!(o.state, 0, "{}: not yet exploding on landing", t.name);
        let o = run.rows[land + 1].obj.expect("exploding");
        assert_eq!(o.state, 1, "{}: exploding the tick after", t.name);
        assert_eq!((o.x, o.y), (x, y));
        assert_eq!(
            y + keg::CARRY_BOX.bottom,
            2106 + keg::move_box(keg::KEG_BOX).bottom
        );
    }
}

#[test]
fn a_full_hold_throws_by_itself_at_full_power() {
    let Some((install, root)) = setup() else {
        return;
    };
    // 3.6 s of the key down from the press: the charge stops after 167
    // held ticks and the keg leaves on the next tick at power 100, the key
    // still down until row 279.
    let run = play(&install, &root, "keg_h3600", "PowderKeg", true);
    let t = &KEG_THROWS[4];
    assert_eq!(t.release, t.press + 168);
    let o = run.rows[t.release].obj.expect("thrown");
    assert!((o.vx + 500.0).abs() < 1e-6 && (o.vy + 764.0).abs() < 1e-6);
    assert!(
        run.rows[t.release + 21..280]
            .iter()
            .all(|r| r.claw_state == state::STAND)
    );
}

#[test]
fn a_key_held_since_the_lift_throws_nothing() {
    let Some((install, root)) = setup() else {
        return;
    };
    let run = play(&install, &root, "keg_held_through", "PowderKeg", true);
    states_match("keg_held_through", &run);
    // Lifted on row 30 and carried to the end, never thrown.
    assert_eq!(span(&run, 42), (30, 43));
    let (first, last) = span(&run, 43);
    assert_eq!((first, last), (44, run.rows.len() - 1));
    assert!(run.rows[30..].iter().all(|r| r.claw_state == state::LIFT));
    assert!(
        run.rows[30..]
            .iter()
            .all(|r| r.obj.expect("carried").pt == ptype::DIRECT)
    );
}

#[test]
fn an_officer_throw_flies_lands_and_bounces_as_in_the_original() {
    let Some((install, root)) = setup() else {
        return;
    };
    for t in &OFFICER_THROWS {
        let run = play(&install, &root, t.name, "Officer", false);
        check_throw(t, &run, true);
    }
    // The tap: the first landing at the speed the last move gives, and the
    // first bounce from it.
    let run = play(&install, &root, "officer_tap", "Officer", false);
    let land = run.rows[110].obj.unwrap();
    assert!((land.vy - 827.042925).abs() < 1e-3, "{}", land.vy);
    let bounce = run.rows[111].obj.unwrap();
    assert_eq!((bounce.x, bounce.y), (2618, 4893));
    assert!((bounce.vy + 238.874).abs() < 1e-3, "{}", bounce.vy);
    // Into the step at x 2697: the corner catches it where it was, it
    // bounces on, and comes to rest on the step.
    let run = play(&install, &root, "officer_h1500", "Officer", false);
    let at = |k: usize| run.rows[k].obj.map(|o| (o.x, o.y, o.pt)).unwrap();
    assert_eq!(at(189), (2697, 4761, ptype::FLY));
    assert_eq!(at(200), (2717, 4768, ptype::STAND));
    assert_eq!(at(206), (2721, 4771, ptype::STAND));
    assert_eq!(at(215), (2721, 4771, ptype::STAND));
    let run = play(&install, &root, "officer_h3000", "Officer", false);
    let at = |k: usize| run.rows[k].obj.map(|o| (o.x, o.y, o.pt)).unwrap();
    assert_eq!(at(278), (2697, 4756, ptype::FLY));
    assert_eq!(at(292), (2697, 4771, ptype::STAND));
    assert_eq!(at(293), (2697, 4768, ptype::FLY));
    assert_eq!(at(298), (2697, 4771, ptype::STAND));
}
