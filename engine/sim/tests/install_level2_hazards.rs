//! The second level's hazards on the user's own install: the punk rats
//! and their cannons, the tower cannons, the goo cover-ups and the
//! cycling signs run with the level's own animations and records.
//!
//! The tests read the folder named by the `CLAW_INSTALL` environment
//! variable at run time and skip themselves when it is unset; nothing from
//! an install is stored in the repository.

use std::collections::BTreeSet;
use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::activation::BREAKABLE_LOGICS;
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::{EnemyState, goo, punkrat};
use claw_sim::logic::{ObjectData, cannon, decor};
use claw_sim::object::{ObjectId, draw_flags};
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

fn level2(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(2)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// The second level with Claw starting at (x, y) (dropped to the floor
/// below it), so the objects around that point wake with the level.
fn sim_at(level: &SimLevel, x: i32, y: i32) -> Sim {
    let mut l = level.clone();
    l.start = (x, y);
    Sim::new(l, SimConfig::default())
}

fn records<'a>(level: &'a SimLevel, logic: &str) -> Vec<&'a ObjectSpec> {
    level.objects.iter().filter(|o| o.logic == logic).collect()
}

/// The object made from the record at (x, y) running `logic`.
fn placed(sim: &Sim, logic: &str, x: i32, y: i32) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && o.editor.0 == x && o.editor.1 == y)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} placed at ({x}, {y})"))
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

fn cannon_data(sim: &Sim, id: ObjectId) -> cannon::CannonData {
    match &sim.objects().get(id).expect("the cannon exists").data {
        ObjectData::Cannon(d) => d.clone(),
        other => panic!("not a cannon: {other:?}"),
    }
}

fn step(sim: &mut Sim) {
    sim.step(TickInput::default());
}

/// The tick an object first ran (0 when it was set up with the level).
fn built_at(sim: &mut Sim, id: ObjectId) -> u64 {
    for _ in 0..100 {
        if sim.objects().get(id).is_some_and(|o| o.state != 0) {
            return sim.tick();
        }
        step(sim);
    }
    panic!("never woke");
}

#[test]
fn install_level2_census_and_no_inert_hazards() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    assert_eq!(level.number, 2);
    assert!(goo::is_goo_level(level.number), "goo, not spikes");
    for (name, n) in [
        ("PunkRat", 8),
        ("TowerCannonLeft", 8),
        ("TowerCannonRight", 3),
        ("GooCoverup", 101),
        ("AniCycle", 15),
        ("GroundCannon", 0),
        ("CannonBall", 0),
    ] {
        assert_eq!(records(&level, name).len(), n, "{name}");
    }
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    step(&mut sim);
    let d = sim.diagnostics();
    eprintln!("LEVEL2 inert logics: {:?}", d.inert_logics);
    for name in [
        "PunkRat",
        "TowerCannonLeft",
        "TowerCannonRight",
        "GroundCannon",
        "CannonBall",
        "GooCoverup",
        "AniCycle",
    ] {
        assert!(!d.inert_logics.contains_key(name), "{name} is inert");
    }
    // Every set and animation the hazards use is loaded with the level.
    let sets = level.image_sets();
    for s in [
        punkrat::IMAGE_SET,
        cannon::GROUND_IMAGE,
        "LEVEL_TOWERCANNONLEFT",
        "LEVEL_TOWERCANNONRIGHT",
        "LEVEL_CANNONBALL",
    ] {
        assert!(sets.iter().any(|x| x == s), "{s} is loaded");
    }
    for key in [
        punkrat::WALK_ANIM,
        punkrat::STAND_ANIM,
        punkrat::STRIKE_ANIM,
        punkrat::RECOIL_ANIM,
        "LEVEL_CANNON_FIRING",
        "LEVEL_CANNON_HOME",
        "LEVEL_TOWERCANNONLEFT_FIRING",
        "LEVEL_TOWERCANNONLEFT_HOME",
        "LEVEL_TOWERCANNONRIGHT_FIRING",
        "LEVEL_TOWERCANNONRIGHT_HOME",
        "LEVEL_CANNONBALL_FALL",
        decor::CYCLE_ANIM,
    ] {
        assert!(level.animations.contains_key(key), "{key} is loaded");
    }
}

/// Where Claw stands beside the fort at the first tower, on the ledge of
/// the lower punk rat.
const FIRST_TOWER_START: (i32, i32) = (10950, 3330);

/// AC3 on the level's own animations: the lower punk rat of the first
/// tower, at (10843, 3380), turns on its 15th tick, strikes on its 57th,
/// its cannon's ball leaves on its 157th and it walks again on its 199th,
/// then every 226 ticks.
#[test]
fn install_level2_punk_rat_cycle() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let mut sim = sim_at(&level, FIRST_TOWER_START.0, FIRST_TOWER_START.1);
    let id = placed(&sim, "PunkRat", 10843, 3380);
    let t0 = built_at(&mut sim, id);
    assert_eq!(sim.objects().get(id).unwrap().z, punkrat::Z);
    let c = sim
        .with_world(|w| punkrat::cannon_of(w, id))
        .expect("its cannon");
    let mut events = Vec::new();
    let mut last = state_of(&sim, id);
    let mut fired = 0;
    while sim.tick() < t0 + 500 {
        step(&mut sim);
        let s = state_of(&sim, id);
        if s != last {
            events.push((sim.tick() - t0, format!("{s:?}")));
            last = s;
        }
        let f = cannon_data(&sim, c).fired;
        if f != fired {
            fired = f;
            events.push((sim.tick() - t0, "ball".to_string()));
            let ball = cannon_data(&sim, c).last_ball.unwrap();
            let b = sim.objects().get(ball).unwrap();
            assert_eq!((b.x, b.y, b.speed_x), (10843 + 50, 3380 + 25, 500));
            assert_eq!(b.damage, 10);
        }
    }
    let want: Vec<(u64, String)> = [
        (15, "Some(MoveLeft)"),
        (57, "Some(Strike)"),
        (156, "Some(Recoil)"),
        (157, "ball"),
        (199, "Some(MoveRight)"),
        (241, "Some(MoveLeft)"),
        (283, "Some(Strike)"),
        (382, "Some(Recoil)"),
        (383, "ball"),
        (425, "Some(MoveRight)"),
        (467, "Some(MoveLeft)"),
    ]
    .iter()
    .map(|(t, s)| (*t, s.to_string()))
    .collect();
    assert_eq!(events, want);
}

/// The mirrored punk rat at (4917, 3690) strikes on its 15th tick and
/// fires left from 70 px to its left.
#[test]
fn install_level2_mirrored_punk_rat() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let rec = records(&level, "PunkRat")
        .into_iter()
        .find(|r| r.record[5] == 4917)
        .expect("the mirrored punk rat");
    assert_eq!(
        rec.record[11] as u32 & draw_flags::MIRROR,
        draw_flags::MIRROR
    );
    let mut sim = sim_at(&level, 4917 - 300, 3690 - 100);
    let id = placed(&sim, "PunkRat", 4917, 3690);
    let t0 = built_at(&mut sim, id);
    let c = sim.with_world(|w| punkrat::cannon_of(w, id)).unwrap();
    let mut strike = None;
    while sim.tick() < t0 + 200 && cannon_data(&sim, c).fired == 0 {
        step(&mut sim);
        if strike.is_none() && state_of(&sim, id) == Some(EnemyState::Strike) {
            strike = Some(sim.tick() - t0);
        }
    }
    assert_eq!(strike, Some(15));
    assert_eq!(sim.tick() - t0, 115);
    let b = sim
        .objects()
        .get(cannon_data(&sim, c).last_ball.unwrap())
        .unwrap();
    assert_eq!((b.x, b.y, b.speed_x), (4917 - 70, 3690 + 25, -500));
}

/// AC4 on the level's own animations: the six cannons of the first tower
/// fire on their first call after waking, their balls leave seven ticks
/// later from 50 px to their side, and they fire again every 167 ticks;
/// the shot sound plays with each ball.
#[test]
fn install_level2_first_tower_cannons() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let mut sim = sim_at(&level, FIRST_TOWER_START.0, FIRST_TOWER_START.1);
    let mut cannons = Vec::new();
    for (name, x, dx, vx) in [
        ("TowerCannonLeft", 11100, -50, -500),
        ("TowerCannonRight", 11493, 50, 500),
    ] {
        for r in records(&level, name) {
            if r.record[5] == x {
                let id = placed(&sim, name, x, r.record[6]);
                cannons.push((id, x + dx, r.record[6] + 10, vx));
            }
        }
    }
    assert_eq!(cannons.len(), 6);
    let mut fired: Vec<Vec<u64>> = vec![Vec::new(); cannons.len()];
    for &(id, ..) in &cannons {
        assert_ne!(
            sim.objects().get(id).unwrap().state,
            0,
            "woken with the level"
        );
    }
    let mut shot_sound_ticks = BTreeSet::new();
    let mut counts = vec![0u32; cannons.len()];
    while sim.tick() < 360 {
        step(&mut sim);
        let heard = sim.last_audio().iter().any(|c| match c {
            AudioCue::Play(s) => s.key == "LEVEL_CANNONSH2",
            _ => false,
        });
        if heard {
            shot_sound_ticks.insert(sim.tick());
        }
        for (i, &(id, bx, by, vx)) in cannons.iter().enumerate() {
            let d = cannon_data(&sim, id);
            if d.fired != counts[i] {
                counts[i] = d.fired;
                fired[i].push(sim.tick());
                let b = sim.objects().get(d.last_ball.unwrap()).unwrap();
                assert_eq!((b.x, b.y, b.speed_x), (bx, by, vx), "cannon {i}");
            }
        }
    }
    for f in &fired {
        assert_eq!(f, &vec![8, 175, 342]);
    }
    for t in [8, 175, 342] {
        assert!(shot_sound_ticks.contains(&t), "shot sound at {t}");
    }
}

/// No breakable lies in any cannon lane: within 60 px of the lane's
/// height and 1600 px ahead of the muzzle (walls ignored). Objects of
/// other kinds in a lane are listed.
#[test]
fn install_level2_no_breakable_in_a_cannon_lane() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let mut lanes = Vec::new();
    for r in records(&level, "TowerCannonLeft") {
        lanes.push((r.record[5] - 50, r.record[6] + 10, -1));
    }
    for r in records(&level, "TowerCannonRight") {
        lanes.push((r.record[5] + 50, r.record[6] + 10, 1));
    }
    for r in records(&level, "PunkRat") {
        let mirrored = r.record[11] as u32 & draw_flags::MIRROR != 0;
        if mirrored {
            lanes.push((r.record[5] - 70, r.record[6] + 25, -1));
        } else {
            lanes.push((r.record[5] + 50, r.record[6] + 25, 1));
        }
    }
    let mut breakables = Vec::new();
    let mut others = BTreeSet::new();
    for &(x, y, dir) in &lanes {
        for o in &level.objects {
            let (ox, oy) = (o.record[5], o.record[6]);
            let ahead = (ox - x) * dir;
            if (0..=1600).contains(&ahead) && (oy - y).abs() <= 60 {
                if BREAKABLE_LOGICS.contains(&o.logic.as_str()) && o.logic != "PowderKeg" {
                    breakables.push((o.logic.clone(), ox, oy, x, y));
                }
                if !["PunkRat", "TowerCannonLeft", "TowerCannonRight"].contains(&o.logic.as_str()) {
                    others.insert((o.logic.clone(), ox, oy));
                }
            }
        }
    }
    eprintln!("objects in cannon lanes: {others:?}");
    assert!(breakables.is_empty(), "breakables in lanes: {breakables:?}");
}

/// A scripted stretch past the first tower: every picture drawn comes
/// from a set loaded with the level, and the cover-ups and signs set
/// themselves up.
#[test]
fn install_level2_stretch_past_the_cannons_draws_only_loaded_sets() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let sets: BTreeSet<String> = level.image_sets().into_iter().collect();
    let mut sim = sim_at(&level, FIRST_TOWER_START.0, FIRST_TOWER_START.1);
    let mut seen = BTreeSet::new();
    for _ in 0..800 {
        let snap = sim.step(TickInput::default());
        for r in &snap.renderables {
            if !r.image_set_key.is_empty() {
                seen.insert(r.image_set_key.clone());
            }
        }
    }
    let missing: Vec<&String> = seen.iter().filter(|s| !sets.contains(*s)).collect();
    assert!(missing.is_empty(), "drawn but not loaded: {missing:?}");
    for s in [punkrat::IMAGE_SET, cannon::GROUND_IMAGE, "LEVEL_CANNONBALL"] {
        assert!(seen.contains(s), "{s} was drawn");
    }
    // The goo cover-ups and signs that ran are set up.
    let mut ran = 0;
    for (_, o) in sim.objects().all() {
        if matches!(&*o.logic, "GooCoverup" | "AniCycle") && o.state != 0 {
            assert_eq!(o.state, decor::SET_UP);
            ran += 1;
        }
    }
    eprintln!("goo cover-ups and signs set up: {ran}");
}

/// The cover-up placed at depth 0 draws at 8010, the others keep 5010;
/// a sign cycles its three images, six ticks each.
#[test]
fn install_level2_cover_ups_and_signs() {
    let Some(store) = store() else { return };
    let level = level2(&store);
    let zero: Vec<&ObjectSpec> = records(&level, "GooCoverup")
        .into_iter()
        .filter(|r| r.record[7] == 0)
        .collect();
    assert_eq!(zero.len(), 1);
    let (x, y) = (zero[0].record[5], zero[0].record[6]);
    let mut sim = sim_at(&level, x, y - 200);
    let id = placed(&sim, "GooCoverup", x, y);
    built_at(&mut sim, id);
    assert_eq!(sim.objects().get(id).unwrap().z, decor::GOO_COVERUP_Z);
    let sign = records(&level, "AniCycle")[0];
    let (sx, sy) = (sign.record[5], sign.record[6]);
    let mut sim = sim_at(&level, sx, sy);
    let id = placed(&sim, "AniCycle", sx, sy);
    built_at(&mut sim, id);
    let mut frames = Vec::new();
    for _ in 0..19 {
        step(&mut sim);
        frames.push(sim.objects().get(id).unwrap().frame);
    }
    let changes = frames.windows(2).filter(|p| p[0] != p[1]).count();
    assert_eq!(changes, 3, "{frames:?}");
    assert_eq!(sim.objects().get(id).unwrap().z, sign.record[7]);
}
