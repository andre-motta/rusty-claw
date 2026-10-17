//! Crates, kegs and pickups on the first level of the user's own install.
//! The tests read the folder named by the `CLAW_INSTALL` environment
//! variable at run time and skip themselves when it is unset; nothing from
//! an install is stored in the repository. Positions and contents come
//! from the level's own records, found by logic name.

mod common;

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::logic::pickup::{self, state as pstate};
use claw_sim::logic::{self, ObjectData, crates, keg};
use claw_sim::object::{LogicKind, ObjectId, Rect, flags};
use claw_sim::trace::{self, TraceRow};
use claw_sim::{GameLogic, InputLatch, InputScript, Sim, SimConfig, SimLevel};
use common::*;

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level1(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(1)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

fn family_of(name: &str) -> Option<GameLogic> {
    [GameLogic::Crate, GameLogic::Keg, GameLogic::Pickup]
        .into_iter()
        .find(|&g| logic::names(g).contains(&name))
}

/// Ids of the objects whose logic is one of `names`.
fn ids_of(sim: &Sim, names: &[&str]) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| names.contains(&&*o.logic))
        .map(|(id, _)| id)
        .collect()
}

/// Runs the first logic call of every object of the three families,
/// wherever it is.
fn init_all(sim: &mut Sim) {
    let ids: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| {
            matches!(
                o.kind,
                LogicKind::Game(GameLogic::Crate | GameLogic::Keg | GameLogic::Pickup)
            ) && o.state == 0
        })
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        sim.with_world(|w| logic::tick(w, id));
    }
}

/// Treasure kind of a picture, written out by name: coin, goldbar, ring,
/// chalice, cross, scepter, gecko, crown, skull.
fn kind_by_name(set: &str) -> Option<usize> {
    let names = [
        "GAME_TREASURE_COINS",
        "GAME_TREASURE_GOLDBARS",
        "GAME_TREASURE_RINGS_",
        "GAME_TREASURE_CHALICES_",
        "GAME_TREASURE_CROSSES_",
        "GAME_TREASURE_SCEPTERS_",
        "GAME_TREASURE_GECKOS_",
        "GAME_TREASURE_CROWNS_",
        "GAME_TREASURE_JEWELEDSKULL_",
    ];
    names.iter().position(|n| set.starts_with(n))
}

/// Treasure kind of a loot code, by the picture it drops.
fn kind_by_code(code: i32) -> Option<usize> {
    pickup::powerup(code).and_then(|e| kind_by_name(e.image_set))
}

#[test]
fn install_level1_census_matches_its_records() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let mut want = [0u32; 9];
    let plane = l.plane();
    for o in &l.objects {
        let r = &o.record;
        if r[5] < 0 || r[6] < 0 || r[5] >= plane.width || r[6] >= plane.height {
            continue;
        }
        let kinds: Vec<usize> = match o.logic.as_str() {
            "FrontCrate" | "BehindCrate" => {
                let code = if r[15] == 0 { 0 } else { r[15] };
                kind_by_code(code).into_iter().collect()
            }
            "TreasurePowerup" | "GlitterlessPowerup" | "HealthPowerup" | "AmmoPowerup"
            | "SpecialPowerup" => kind_by_name(&o.image_set).into_iter().collect(),
            _ => std::iter::once(r[15])
                .chain(r[35..43].iter().copied())
                .filter(|&c| c != 0)
                .filter_map(kind_by_code)
                .collect(),
        };
        for k in kinds {
            want[k] += 1;
        }
    }
    let got = pickup::census(&l);
    println!("LEVEL1 treasure census: {got:?}");
    assert_eq!(got, want);
    assert!(got.iter().all(|&n| n > 0), "every kind is in the level");
    let sim = Sim::new(l, SimConfig::default());
    assert_eq!(sim.treasure_census(), want);
}

#[test]
fn install_level1_every_pickup_logic_binds_to_an_implemented_family() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let records: Vec<(ObjectId, GameLogic)> = sim
        .objects()
        .all()
        .filter_map(|(id, o)| family_of(&o.logic).map(|g| (id, g)))
        .collect();
    assert!(!records.is_empty());
    for &(id, g) in &records {
        let o = sim.objects().get(id).unwrap();
        assert_eq!(o.kind, LogicKind::Game(g), "{}", o.logic);
        *counts.entry(o.logic.to_string()).or_default() += 1;
    }
    println!("LEVEL1 crate, keg and pickup records: {counts:?}");
    init_all(&mut sim);
    for (id, g) in records {
        let Some(o) = sim.objects().get(id) else {
            continue;
        };
        if o.flags & flags::DESTROY != 0 {
            // Curses and stack spawners remove themselves.
            assert!(
                matches!(
                    &*o.logic,
                    "CursePowerup" | "BackStackedCrates" | "FrontStackedCrates"
                ),
                "{}",
                o.logic
            );
            continue;
        }
        match g {
            GameLogic::Pickup => {
                let d = pickup::data_of(o).unwrap_or_else(|| panic!("{}", o.logic));
                assert_eq!(o.state, pstate::IDLE, "{}", o.logic);
                assert_ne!(d.kind, 0, "{} {} is in the table", o.logic, o.image_set);
                assert_ne!(o.type_flags, 0);
            }
            GameLogic::Crate => {
                assert!(matches!(o.data, ObjectData::Crate(_)), "{}", o.logic);
                assert!(o.hit_handler && o.hit_rect.is_set());
            }
            GameLogic::Keg => {
                assert!(keg::data_of(o).is_some());
                assert!(o.hit_handler && o.hit_rect.is_set());
            }
            _ => unreachable!(),
        }
    }
    // The level loaded the animations these logics play.
    for key in [crates::BREAK_ANIM, keg::ANIM_EXPLODE, keg::ANIM_THROWN] {
        assert!(l.animations.contains_key(key), "{key}");
    }
}

#[test]
fn install_level1_breaking_every_crate_releases_its_contents() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let mut want: Vec<i32> = Vec::new();
    let mut stacked = 0;
    for o in &l.objects {
        match o.logic.as_str() {
            "FrontCrate" | "BehindCrate" => {
                let code = o.record[15].max(0);
                if pickup::powerup(code).is_some() {
                    want.push(code);
                }
            }
            "BackStackedCrates" | "FrontStackedCrates" => {
                stacked += crates::stack_contents(&o.record).len();
                want.extend(
                    crates::stack_contents(&o.record)
                        .into_iter()
                        .filter(|&c| c != 0),
                );
            }
            _ => {}
        }
    }
    assert!(stacked > 0);
    let mut sim = Sim::new(l, SimConfig::default());
    init_all(&mut sim);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    let crate_ids = ids_of(&sim, &["FrontCrate", "BehindCrate", crates::STACKED_CRATE]);
    assert_eq!(
        ids_of(&sim, &[crates::STACKED_CRATE]).len(),
        stacked,
        "one crate per item in every stack"
    );
    for id in crate_ids {
        for _ in 0..3 {
            sim.with_world(|w| logic::on_hit(w, id, claw));
            let o = sim.objects().get(id).unwrap();
            if o.flags & flags::SAFE != 0 {
                break;
            }
            // A two-hit crate: wait out its cooldown.
            for _ in 0..30 {
                sim.with_world(|w| logic::tick(w, id));
            }
        }
        assert!(
            sim.objects().get(id).unwrap().flags & flags::SAFE != 0,
            "crate {id} broke"
        );
    }
    let mut got: Vec<i32> = ids_of(&sim, &[pickup::BOUNCING_GOODIE])
        .into_iter()
        .map(|g| {
            pickup::data_of(sim.objects().get(g).unwrap())
                .unwrap()
                .drop_code
                .unwrap()
        })
        .collect();
    got.sort_unstable();
    want.sort_unstable();
    println!("LEVEL1: {} crate items released", got.len());
    assert_eq!(got, want);
}

// ------------------------------------------------------------ determinism

/// A hash of every live crate, keg and pickup and of the random stream.
fn digest(sim: &Sim) -> u64 {
    let mut h = DefaultHasher::new();
    sim.rng().state().hash(&mut h);
    for id in sim.objects().listed() {
        let o = sim.objects().get(id).unwrap();
        if matches!(
            o.kind,
            LogicKind::Game(GameLogic::Crate | GameLogic::Keg | GameLogic::Pickup)
        ) {
            (id, o.x, o.y, o.z, o.state, o.frame, o.flags).hash(&mut h);
            format!("{:?}", o.data).hash(&mut h);
        }
    }
    h.finish()
}

/// Scripted actions between ticks: Claw is moved next to the first crate
/// and the first keg (found by logic name), the crates around him are
/// broken and the keg is lifted and thrown.
fn act(sim: &mut Sim, tick: u64) {
    let first = |sim: &Sim, name: &str| {
        sim.level()
            .objects
            .iter()
            .find(|o| o.logic == name)
            .map(|o| (o.record[5], o.record[6]))
            .unwrap()
    };
    match tick {
        150 => {
            let (x, y) = first(sim, "FrontCrate");
            sim.teleport_player(x - 40, y - 80);
        }
        200 | 260 => {
            let claw = sim.with_world(|w| w.claw_id.unwrap());
            let (cx, cy) = {
                let p = sim.player().unwrap();
                (p.x, p.y)
            };
            let near: Vec<ObjectId> = sim
                .objects()
                .listed()
                .into_iter()
                .filter(|&id| {
                    let o = sim.objects().get(id).unwrap();
                    o.kind == LogicKind::Game(GameLogic::Crate)
                        && (o.x - cx).abs() < 400
                        && (o.y - cy).abs() < 300
                })
                .collect();
            for id in near {
                sim.with_world(|w| logic::on_hit(w, id, claw));
            }
        }
        400 => {
            let (x, y) = first(sim, "PowderKeg");
            sim.teleport_player(x - 60, y - 80);
        }
        430 => {
            let claw = sim.with_world(|w| w.claw_id.unwrap());
            if let Some(&k) = ids_of(sim, &["PowderKeg"])
                .iter()
                .find(|&&k| sim.objects().get(k).unwrap().is_listed())
            {
                // Claw's lifting frames, played here by hand: halfway up,
                // then overhead.
                sim.with_world(|w| logic::lifted(w, k, claw));
                for frame in [keg::LIFT_FRAME, keg::OVERHEAD_FRAME] {
                    sim.with_world(|w| {
                        w.objects.get_mut(claw).unwrap().frame = frame;
                        logic::tick(w, k);
                    });
                }
            }
        }
        460 => {
            if let Some(&k) = ids_of(sim, &["PowderKeg"]).iter().find(|&&k| {
                keg::data_of(sim.objects().get(k).unwrap())
                    .is_some_and(|d| d.phase == keg::Phase::Overhead)
            }) {
                sim.with_world(|w| logic::thrown(w, k, 350, -700));
            }
        }
        _ => {}
    }
}

fn route() -> InputScript {
    script(&[
        (RIGHT, 2000),
        (RIGHT | JUMP, 600),
        (0, 1000),
        (LEFT, 1500),
        (0, 1500),
        (RIGHT, 1200),
        (0, 4000),
    ])
}

fn trace_direct(l: &SimLevel, s: &InputScript) -> String {
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut out = String::new();
    for (t, &input) in s.ticks.iter().enumerate() {
        act(&mut sim, t as u64);
        sim.step(input);
        let row = TraceRow::capture(&sim, t as u64, "script");
        out.push_str(&trace::to_csv(&[row]));
        out.push_str(&format!("{:x}\n", digest(&sim)));
    }
    out
}

fn trace_at_rate(l: &SimLevel, s: &InputScript, hz: u64) -> String {
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut latch = InputLatch::new();
    let events = s.events();
    let mut next_event = 0;
    let mut out = String::new();
    let mut tick = 0u64;
    let n = s.ticks.len() as u64;
    let mut frame = 0u64;
    while tick < n {
        let now_us = frame * 1_000_000 / hz;
        while next_event < events.len() && events[next_event].0 <= now_us {
            let (_, b, down) = events[next_event];
            latch.event(now_us, b, down);
            next_event += 1;
        }
        while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
            act(&mut sim, tick);
            sim.step(latch.take(tick));
            let row = TraceRow::capture(&sim, tick, "script");
            out.push_str(&trace::to_csv(&[row]));
            out.push_str(&format!("{:x}\n", digest(&sim)));
            tick += 1;
        }
        frame += 1;
    }
    out
}

#[test]
fn install_level1_crate_and_keg_traces_are_identical_at_60_144_and_240_hz() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let s = route();
    let direct = trace_direct(&l, &s);
    // The route breaks crates and sets off a keg.
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    let mut broke = 0;
    let mut kegs = 0;
    for (t, &input) in s.ticks.iter().enumerate() {
        act(&mut sim, t as u64);
        sim.step(input);
        broke = broke.max(ids_of(&sim, &[pickup::BOUNCING_GOODIE]).len());
        kegs = kegs.max(
            sim.objects()
                .all()
                .filter(|(_, o)| &*o.logic == "Explosion")
                .count(),
        );
    }
    println!("route: up to {broke} goodies at once, {kegs} explosions");
    assert!(broke > 0, "crates broke");
    assert!(kegs > 0, "the keg went off");
    for hz in [60, 144, 240] {
        assert!(trace_at_rate(&l, &s, hz) == direct, "{hz} Hz");
    }
}

#[test]
fn install_level1_pickups_and_crates_touch_with_their_pictures() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    init_all(&mut sim);
    let (mut pickups, mut crates_seen) = (0, 0);
    for (_, o) in sim.objects().all() {
        if !matches!(
            o.kind,
            LogicKind::Game(GameLogic::Crate | GameLogic::Pickup)
        ) || o.flags & flags::DESTROY != 0
        {
            continue;
        }
        let first = l.image_frames(&o.image_set).map(|(f, _)| f);
        let frame = if o.frame < 0 { first.unwrap() } else { o.frame };
        let picture = l.frame_box(&o.image_set, frame);
        match &*o.logic {
            "FrontCrate" | "BehindCrate" => {
                assert_eq!(Some(o.frame), first, "{}: the set's first image", o.logic);
                assert_eq!(Some(o.hit_rect), picture, "{} {}", o.logic, o.image_set);
                crates_seen += 1;
            }
            "TreasurePowerup" | "GlitterlessPowerup" | "SpecialPowerup" => {
                let b = picture.unwrap_or_else(|| panic!("{} has a box", o.image_set));
                assert_eq!(o.attack_rect, pickup::padded(b), "{}", o.image_set);
                pickups += 1;
            }
            "HealthPowerup" | "AmmoPowerup" | "MagicPowerup" => {
                let b = picture.unwrap_or_else(|| panic!("{} has a box", o.image_set));
                assert_eq!(o.attack_rect, b, "{}", o.image_set);
                pickups += 1;
            }
            _ => {}
        }
    }
    eprintln!("{pickups} pickups and {crates_seen} crates checked");
    assert_eq!(crates_seen, 25 + 9);
    assert!(pickups > 400, "{pickups}");
    // Not every picture is the old 32 px square.
    let sizes: std::collections::BTreeSet<(i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::Pickup))
        // A pickup with no attack box yet holds the unset rectangle.
        .filter(|(_, o)| o.attack_rect.left != Rect::UNSET_LEFT)
        .map(|(_, o)| {
            (
                o.attack_rect.right - o.attack_rect.left,
                o.attack_rect.bottom - o.attack_rect.top,
            )
        })
        .collect();
    assert!(sizes.len() > 3, "{sizes:?}");
    // The first level shows its map piece as GAME_MAPPIECE: the record is
    // the level's end.
    assert_eq!(l.number, 1);
    assert_eq!(pickup::map_piece_set(l.number), "GAME_MAPPIECE");
    let pieces: Vec<(i32, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| o.image_set == "GAME_MAPPIECE")
        .filter_map(|(_, o)| pickup::data_of(o).map(|d| (d.kind, d.points)))
        .collect();
    assert_eq!(pieces, vec![(pickup::MAP_PIECE_CODE, pickup::END_OF_LEVEL)]);
}
