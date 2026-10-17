//! Everything that animates on its own animates on the first two levels of
//! the user's own install: the level runs for a few seconds around its
//! start and around the first object of every logic that loops a picture
//! by itself (animated decoration, the cycling signs, pickups and their
//! glitter, the boss door, the tower cannons, cannon balls), and every
//! such object that stayed live long enough shows more than one image of
//! its set. Still decoration keeps its one image.
//!
//! The test reads the folder named by the `CLAW_INSTALL` environment
//! variable at run time and skips itself when it is unset; nothing from an
//! install is stored in the repository.

use std::collections::BTreeMap;
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::object::ObjectId;
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

/// Logics that loop a picture by themselves while nothing happens to
/// them.
const ANIMATES: &[&str] = &[
    "AniCycle",
    "AniCycleNormal",
    "AmmoPowerup",
    "BehindAniCandy",
    "BossWarp",
    "BouncingGoodie",
    "CannonBall",
    "CoinPowerup",
    "EndOfLevelPowerup",
    "FrontAniCandy",
    "GlitterlessPowerup",
    "GoldPowerup",
    "HealthPowerup",
    "MagicPowerup",
    "PowerupGlitter",
    "SpecialPowerup",
    "SuperPowerup",
    "TowerCannonLeft",
    "TowerCannonRight",
    "TreasurePowerup",
];

/// Logics that keep the one image they were placed with.
const STILL: &[&str] = &[
    "BehindCandy",
    "DoNothing",
    "DoNothingNormal",
    "FrontCandy",
    "GooCoverup",
];

/// Ticks each run lasts (3.6 s).
const RUN_TICKS: u64 = 200;

/// Ticks an object must stay live, running the same logic, to be judged
/// (720 ms): longer than a glitter's longest start delay (250 ms) plus
/// two images of a 100 ms loop. Claw falls past the boss door from any
/// start near it, so it is live for less than a second.
const JUDGED_TICKS: u64 = 40;

/// What one object did while it was watched.
#[derive(Debug, Default)]
struct Watch {
    logic: String,
    image_set: String,
    first_tick: u64,
    last_tick: u64,
    frames: Vec<i32>,
}

impl Watch {
    fn changed(&self) -> bool {
        self.frames.windows(2).any(|w| w[0] != w[1])
    }
}

/// Runs `level` from `start` and watches every live object; an object
/// that leaves the live list or changes logic stops being watched under
/// that entry.
fn watch(level: &SimLevel, start: (i32, i32)) -> Vec<Watch> {
    let mut l = level.clone();
    l.start = start;
    let mut sim = Sim::new(l, SimConfig::default());
    let mut open: BTreeMap<ObjectId, Watch> = BTreeMap::new();
    let mut done = Vec::new();
    for _ in 0..RUN_TICKS {
        sim.step(TickInput::default());
        let tick = sim.tick();
        let live = sim.objects().listed();
        for id in &live {
            let o = sim.objects().get(*id).expect("listed objects exist");
            let fresh = open.get(id).is_none_or(|w| w.logic != *o.logic);
            if fresh && let Some(w) = open.remove(id) {
                done.push(w);
            }
            let w = open.entry(*id).or_insert_with(|| Watch {
                logic: o.logic.to_string(),
                first_tick: tick,
                ..Watch::default()
            });
            w.image_set = o.image_set.to_string();
            w.last_tick = tick;
            w.frames.push(o.frame);
        }
        let gone: Vec<ObjectId> = open
            .keys()
            .copied()
            .filter(|id| !live.contains(id))
            .collect();
        for id in gone {
            done.extend(open.remove(&id));
        }
    }
    done.extend(open.into_values());
    done
}

/// Claw starts this far left of the record a run is for, so he does not
/// take it (or go through it, for a door) on the first tick.
const BESIDE: i32 = 160;

/// Places to run from: the level's start and beside the first record of
/// every logic in `logics` (the boss door among them, which no run from
/// the start reaches).
fn starts(level: &SimLevel, logics: &[&str]) -> Vec<(i32, i32)> {
    let mut out = vec![level.start];
    for name in logics {
        if let Some(s) = level.objects.iter().find(|s| &*s.logic == *name) {
            out.push((s.record[5] - BESIDE, s.record[6]));
        }
    }
    out
}

/// Runs the audit on level `n`; returns how many objects of each logic
/// were judged.
fn audit(store: &AssetStore, n: u32) -> BTreeMap<String, usize> {
    let l = level(store, n);
    let mut judged: BTreeMap<String, usize> = BTreeMap::new();
    let mut still_moving = Vec::new();
    let mut not_moving = Vec::new();
    let logics: Vec<&str> = ANIMATES.iter().chain(STILL).copied().collect();
    for start in starts(&l, &logics) {
        for w in watch(&l, start) {
            if w.last_tick - w.first_tick + 1 < JUDGED_TICKS {
                continue;
            }
            let several = l
                .image_frames(&w.image_set)
                .is_some_and(|(first, last)| last > first);
            if ANIMATES.contains(&w.logic.as_str()) && several {
                *judged.entry(w.logic.clone()).or_default() += 1;
                if !w.changed() {
                    not_moving.push(format!("{} {} at {start:?}", w.logic, w.image_set));
                }
            } else if STILL.contains(&w.logic.as_str()) {
                *judged.entry(w.logic.clone()).or_default() += 1;
                if w.changed() {
                    still_moving.push(format!("{} {} at {start:?}", w.logic, w.image_set));
                }
            }
        }
    }
    assert!(
        not_moving.is_empty(),
        "LEVEL{n}: objects that should animate stayed on one image: {not_moving:#?}"
    );
    assert!(
        still_moving.is_empty(),
        "LEVEL{n}: still decoration changed its image: {still_moving:#?}"
    );
    eprintln!("LEVEL{n} judged: {judged:?}");
    judged
}

#[test]
fn everything_that_animates_on_its_own_animates_on_the_first_level() {
    let Some(store) = store() else { return };
    let judged = audit(&store, 1);
    for logic in [
        "PowerupGlitter",
        "GlitterlessPowerup",
        "SpecialPowerup",
        "BehindAniCandy",
        "BehindCandy",
        "DoNothing",
    ] {
        assert!(
            judged.get(logic).is_some_and(|&c| c > 0),
            "no {logic} judged: {judged:?}"
        );
    }
}

#[test]
fn everything_that_animates_on_its_own_animates_on_the_second_level() {
    let Some(store) = store() else { return };
    let judged = audit(&store, 2);
    for logic in [
        "PowerupGlitter",
        "GlitterlessPowerup",
        "AniCycle",
        "BehindAniCandy",
        "BossWarp",
        "TowerCannonLeft",
        "CannonBall",
        "GooCoverup",
    ] {
        assert!(
            judged.get(logic).is_some_and(|&c| c > 0),
            "no {logic} judged: {judged:?}"
        );
    }
}
