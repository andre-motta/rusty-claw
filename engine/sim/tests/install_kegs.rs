//! Kegs rest on the floor at spawn, on the first two levels of the user's
//! own install. The tests read the folder named by the `CLAW_INSTALL`
//! environment variable at run time and skip themselves when it is unset;
//! nothing from an install is stored in the repository.
//!
//! The settled positions are also compared, keg by keg, with a reference
//! list of the original's settled keg positions, read at run time from
//! `$CLAW_REFERENCE/survey/kegs-level0<N>.csv` (columns index, record_x,
//! record_y, settled_x, settled_y, one row per keg in record order); that
//! part skips itself when `CLAW_REFERENCE` is unset.

use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::logic::{self, keg};
use claw_sim::object::{Object, ObjectId};
use claw_sim::physics::ptype;
use claw_sim::tiles::{CLIMB, GROUND, SOLID};
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level(store: &AssetStore, n: u32) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// A level's kegs in record order, with their record positions, after
/// each has run its first logic call.
fn settled_kegs(level: SimLevel) -> (Sim, Vec<(ObjectId, i32, i32)>) {
    let mut sim = Sim::new(level, SimConfig::default());
    let kegs: Vec<(ObjectId, i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| keg::NAMES.contains(&&*o.logic))
        .map(|(id, o)| (id, o.x, o.y))
        .collect();
    for &(id, _, _) in &kegs {
        assert_eq!(sim.objects().get(id).unwrap().state, keg::state::INIT);
        sim.with_world(|w| logic::tick(w, id));
    }
    (sim, kegs)
}

/// First floor row (Solid, Ground or Climb) at or below `y` in column `x`.
fn floor_below(sim: &Sim, x: i32, y: i32) -> Option<i32> {
    let map = &sim.level().map;
    (y..map.height()).find(|&row| matches!(map.attr(x, row), SOLID | GROUND | CLIMB))
}

/// The bottom of the box a keg stands with.
fn feet(o: &Object) -> i32 {
    o.y + o.move_rect.bottom
}

/// The reference list of settled kegs for level `n`, when
/// `CLAW_REFERENCE` is set: (record_x, record_y, settled_x, settled_y).
fn reference(n: u32) -> Option<Vec<(i32, i32, i32, i32)>> {
    let Some(root) = std::env::var_os("CLAW_REFERENCE") else {
        eprintln!("CLAW_REFERENCE is not set; skipping the comparison");
        return None;
    };
    let path = PathBuf::from(root).join(format!("survey/kegs-level{n:02}.csv"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h.trim() == name)
            .unwrap_or_else(|| panic!("no column {name} in {}", path.display()))
    };
    let cols = [
        col("record_x"),
        col("record_y"),
        col("settled_x"),
        col("settled_y"),
    ];
    let rows = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            let n = |i: usize| -> i32 { v[cols[i]].trim().parse().expect("a number") };
            (n(0), n(1), n(2), n(3))
        })
        .collect();
    Some(rows)
}

/// Every keg of level `n` settled within 1 px of the reference, matched by
/// its record position in record order.
fn matches_reference(n: u32, sim: &Sim, kegs: &[(ObjectId, i32, i32)]) {
    let Some(rows) = reference(n) else { return };
    assert_eq!(rows.len(), kegs.len(), "level {n}: kegs in the reference");
    for (i, (&(id, rx, ry), &(sx, sy, ex, ey))) in kegs.iter().zip(&rows).enumerate() {
        assert_eq!((rx, ry), (sx, sy), "level {n} keg {i}: record order");
        let o = sim.objects().get(id).unwrap();
        assert!(
            (o.x - ex).abs() <= 1 && (o.y - ey).abs() <= 1,
            "level {n} keg {i} at ({rx}, {ry}) settled at ({}, {}), reference ({ex}, {ey})",
            o.x,
            o.y
        );
    }
}

#[test]
fn install_level2_kegs_rest_on_the_first_floor_below() {
    let Some(store) = store() else { return };
    let (sim, kegs) = settled_kegs(level(&store, 2));
    assert_eq!(kegs.len(), 50, "PowderKegs on LEVEL2");
    for (i, &(id, rx, ry)) in kegs.iter().enumerate() {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.x, rx, "keg {i}: it does not move sideways");
        assert_eq!(o.physics_type, ptype::STAND, "keg {i}: standing");
        let floor = floor_below(&sim, rx, ry - 32 + o.move_rect.bottom)
            .unwrap_or_else(|| panic!("keg {i} at ({rx}, {ry}): no floor below"));
        assert!(
            (feet(o) - (floor - 1)).abs() <= 1,
            "keg {i} at ({rx}, {ry}): feet at {}, floor at {floor}",
            feet(o)
        );
        assert!(o.y > ry, "keg {i} at ({rx}, {ry}): it came down to {}", o.y);
    }
    matches_reference(2, &sim, &kegs);
}

#[test]
fn install_level1_kegs_settle_where_the_reference_has_them() {
    let Some(store) = store() else { return };
    let (sim, kegs) = settled_kegs(level(&store, 1));
    assert_eq!(kegs.len(), 58, "PowderKegs on LEVEL1");
    for (i, &(id, rx, ry)) in kegs.iter().enumerate() {
        let o = sim.objects().get(id).unwrap();
        let floor = floor_below(&sim, rx, ry - 32 + o.move_rect.bottom)
            .unwrap_or_else(|| panic!("keg {i} at ({rx}, {ry}): no floor below"));
        assert_eq!(feet(o), floor - 1, "keg {i} at ({rx}, {ry})");
    }
    matches_reference(1, &sim, &kegs);
}

#[test]
fn install_a_keg_settles_on_its_first_logic_call_in_play() {
    let Some(store) = store() else { return };
    let lv = level(&store, 1);
    let (settled, kegs) = settled_kegs(lv.clone());
    let &(id, rx, ry) = kegs.first().expect("a keg");
    let want = settled.objects().get(id).unwrap().y;
    assert!(want > ry, "the keg comes down");

    // Claw walks up to the keg in play: it settles once it runs.
    let mut sim = Sim::new(lv, SimConfig::default());
    sim.teleport_player(rx - 100, ry);
    let mut ran = false;
    for _ in 0..20 {
        sim.step(TickInput::default());
        let o = sim.objects().get(id).unwrap();
        if o.state != keg::state::INIT {
            ran = true;
            assert_eq!((o.x, o.y), (rx, want), "settled in play");
        }
    }
    assert!(ran, "the keg ran its logic near Claw");
}
