//! Projectiles in LEVEL1's real geometry, against the user's own install.
//! The test reads the folder named by the `CLAW_INSTALL` environment
//! variable at run time and skips itself when it is unset; positions come
//! from the level's own object records, found by logic name.

use std::path::PathBuf;
use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::container::CollisionEvent;
use claw_sim::logic::effect::{self, EffectKind};
use claw_sim::logic::projectile::{self, Phase, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, types};
use claw_sim::tiles::{GROUND, SOLID};
use claw_sim::{InputLatch, Sim, SimConfig, SimLevel, TickInput};

fn level1() -> Option<Arc<SimLevel>> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    let store = install.store().expect("mounting the install");
    let data = LevelData::build(&store, LevelSource::Retail(1)).unwrap();
    Some(Arc::new(SimLevel::from_level(&data, &store).unwrap()))
}

fn idle(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

/// A simulation of LEVEL1 with Claw moved to (x, y) and left to settle.
fn settled(level: &Arc<SimLevel>, at: Option<(i32, i32)>) -> Sim {
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    idle(&mut sim, 10);
    if let Some((x, y)) = at {
        sim.teleport_player(x, y);
    }
    idle(&mut sim, 120);
    sim
}

fn claw(sim: &Sim) -> (i32, i32, i32, bool) {
    let o = sim.player().unwrap();
    (o.x, o.y, o.z, sim.claw().facing_right)
}

/// The nearest Solid pixel left or right of (x, y) within `reach` px, as
/// (direction, distance).
fn wall_near(level: &SimLevel, x: i32, y: i32, reach: i32) -> Option<(i32, i32)> {
    let mut best: Option<(i32, i32)> = None;
    for dir in [1, -1] {
        if let Some(d) = (1..reach).find(|&d| level.map.attr(x + dir * d, y) == SOLID)
            && best.is_none_or(|(_, b)| d < b)
        {
            best = Some((dir, d));
        }
    }
    best
}

/// The pistol shot Claw fires standing, facing `dir`.
fn pistol_shot(c: (i32, i32, i32, bool), dir: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "PistolBullet",
        x: c.0 + 71 * dir,
        y: c.1 - 21,
        z: c.2 - 1,
        vx: 700 * dir,
        damage: 8,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_BULLETS",
        ..ProjectileSpec::default()
    }
}

/// The bomb a rat at (x, y) drops when Claw is right below it.
fn dropped_bomb(x: i32, y: i32, z: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "RatBomb",
        x: x + 1,
        y: y - 12,
        z: z - 1,
        vy: 250,
        image_set: "LEVEL_RATBOMB",
        ..ProjectileSpec::default()
    }
}

fn record_positions(level: &SimLevel, logic: &str) -> Vec<(i32, i32, i32)> {
    level
        .objects
        .iter()
        .filter(|o| o.logic == logic)
        .map(|o| (o.record[5], o.record[6], o.record[7]))
        .collect()
}

/// A hand-height spot with a wall within reach: Claw's start, or else where
/// a Soldier stands.
fn shooting_spot(level: &Arc<SimLevel>) -> (Sim, i32, i32) {
    let starts = std::iter::once(None).chain(
        record_positions(level, "Soldier")
            .into_iter()
            .map(|(x, y, _)| Some((x, y))),
    );
    for at in starts {
        let sim = settled(level, at);
        let c = claw(&sim);
        if let Some((dir, d)) = wall_near(level, c.0, c.1 - 21, 600)
            && d > 80
        {
            return (sim, dir, d);
        }
    }
    panic!("no wall at hand height near any spot");
}

#[test]
fn install_a_pistol_shot_hits_a_wall_and_is_removed() {
    let Some(level) = level1() else { return };
    let (mut sim, dir, d) = shooting_spot(&level);
    let c = claw(&sim);
    let shot = sim
        .with_world(|w| projectile::spawn(w, pistol_shot(c, dir)))
        .unwrap();
    let wall_x = c.0 + dir * d;
    let mut last = None;
    let limit = (d / 12) as usize + 5;
    for _ in 0..limit {
        sim.step(TickInput::default());
        match sim.objects().get(shot) {
            Some(o) => last = Some(o.x),
            None => break,
        }
    }
    assert!(sim.objects().get(shot).is_none(), "removed at the wall");
    let x = last.expect("it flew");
    // Its box edge stops short of the wall, within one step.
    let edge = if dir > 0 { x + 1 } else { x - 2 };
    assert!(
        (wall_x - edge).abs() <= 13,
        "stopped at {x}, wall at {wall_x}"
    );
}

/// Runs a dropped rat bomb from the first rat in LEVEL1 and returns its
/// per-tick trace: (tick, x, y, phase, armed, attack box, alive).
fn bomb_run(level: &Arc<SimLevel>, hz: Option<u64>) -> Vec<String> {
    let (rx, ry, rz) = record_positions(level, "Rat")[0];
    let mut sim = settled(level, Some((rx - 120, ry - 40)));
    let bomb = sim
        .with_world(|w| projectile::spawn(w, dropped_bomb(rx, ry, rz)))
        .unwrap();
    let mut rows = Vec::new();
    let n = 120u64;
    let mut record = |sim: &Sim| {
        let row = match sim.objects().get(bomb) {
            Some(o) => {
                let d = projectile::data(o).unwrap();
                format!(
                    "{},{},{},{:?},{},{:?},{:?}",
                    sim.tick(),
                    o.x,
                    o.y,
                    d.phase,
                    d.armed,
                    o.attack_rect,
                    o.flags & flags::HARMLESS != 0
                )
            }
            None => format!("{},gone", sim.tick()),
        };
        rows.push(row);
    };
    match hz {
        None => {
            for _ in 0..n {
                sim.step(TickInput::default());
                record(&sim);
            }
        }
        Some(hz) => {
            // Ticks run as a display at `hz` reaches their end time.
            let mut latch = InputLatch::new();
            let start = sim.tick();
            let mut frame = 0u64;
            let mut tick = 0u64;
            while tick < n {
                let now_us = frame * 1_000_000 / hz;
                while tick < n && (tick + 1) * claw_sim::input::TICK_US <= now_us {
                    sim.step(latch.take(start + tick));
                    record(&sim);
                    tick += 1;
                }
                frame += 1;
            }
        }
    }
    rows
}

#[test]
fn install_a_dropped_rat_bomb_explodes_on_the_floor() {
    let Some(level) = level1() else { return };
    let (rx, ry, rz) = record_positions(&level, "Rat")[0];
    let mut sim = settled(&level, Some((rx - 120, ry - 40)));
    let spec = dropped_bomb(rx, ry, rz);
    let bomb = sim.with_world(|w| projectile::spawn(w, spec)).unwrap();
    let mut exploded = None;
    let mut armed_at = None;
    for n in 1..=200 {
        sim.step(TickInput::default());
        let Some(o) = sim.objects().get(bomb) else {
            break;
        };
        let d = projectile::data(o).unwrap();
        if exploded.is_none() && d.phase == Phase::Exploding {
            exploded = Some((n, o.x, o.y));
        }
        if armed_at.is_none() && d.armed {
            armed_at = Some(n);
            assert_eq!(
                o.attack_rect,
                Rect::new(-57, -7, 46, 94),
                "the box grows by 40 px when it arms"
            );
            assert_eq!(o.flags & flags::HARMLESS, 0);
        }
    }
    let (n, x, y) = exploded.expect("it exploded");
    // Its box touches the floor: Solid or Ground at or just below the
    // bottom of its box.
    let bottom = y + 54;
    let floor = (bottom - 30..=bottom + 1).any(|yy| {
        let a = level.map.attr(x, yy);
        a == SOLID || a == GROUND
    });
    assert!(floor, "exploded at ({x}, {y}) on tick {n}, not on a floor");
    assert_eq!(armed_at, Some(n + 7));
    assert!(
        sim.objects().get(bomb).is_none(),
        "removed after its animation"
    );

    // Dropped from higher up, it falls and goes off where its box lands.
    let mut high = spec;
    high.y -= 150;
    let bomb = sim.with_world(|w| projectile::spawn(w, high)).unwrap();
    let mut landed = None;
    for n in 1..=200 {
        sim.step(TickInput::default());
        let Some(o) = sim.objects().get(bomb) else {
            break;
        };
        if projectile::data(o).unwrap().phase == Phase::Exploding {
            landed = Some((n, o.x, o.y));
            break;
        }
    }
    let (n, x, y) = landed.expect("it landed and exploded");
    assert!(n > 10, "it fell for a while: {n}");
    let below = level.map.attr(x, y + 55);
    assert!(below == SOLID || below == GROUND, "floor under ({x}, {y})");
    assert_ne!(level.map.attr(x, y + 54), SOLID);
}

#[test]
fn install_explosion_hits_a_target_in_real_geometry_once() {
    let Some(level) = level1() else { return };
    let (rx, ry, rz) = record_positions(&level, "Rat")[0];
    let mut sim = settled(&level, Some((rx - 120, ry - 40)));
    let spec = dropped_bomb(rx, ry, rz);
    let bomb = sim.with_world(|w| projectile::spawn(w, spec)).unwrap();
    let mut t = Object::new(0, rx + 1, ry + 30, rz, "Target".into(), LogicKind::Inert);
    t.flags = flags::WORLD | flags::ALWAYS_ACTIVE;
    t.hit_type_flags = types::LEVEL_SHOT;
    t.hit_rect = Rect::new(-10, -10, 10, 10);
    t.hit_handler = true;
    let target: ObjectId = sim.spawn(t);
    let mut hits = 0;
    for _ in 0..200 {
        sim.step(TickInput::default());
        hits += sim
            .collision_events()
            .iter()
            .filter(|e| {
                **e == CollisionEvent::Hit {
                    attacker: bomb,
                    victim: target,
                }
            })
            .count();
        if sim.objects().get(bomb).is_none() {
            break;
        }
    }
    assert_eq!(hits, 1);
}

#[test]
fn install_projectile_traces_are_identical_at_60_144_and_240_hz() {
    let Some(level) = level1() else { return };
    let direct = bomb_run(&level, None);
    assert!(direct.iter().any(|r| r.contains("Exploding")));
    assert!(direct.last().unwrap().ends_with("gone"));
    for hz in [60, 144, 240] {
        assert_eq!(bomb_run(&level, Some(hz)), direct, "{hz} Hz");
    }
}

#[test]
fn install_projectile_boxes_come_from_the_first_frames() {
    let Some(level) = level1() else { return };
    let cases = [
        ("PistolBullet", "GAME_BULLETS", Rect::new(-2, -2, 1, 1)),
        ("MagicBullet", "GAME_MAGICCLAW", Rect::new(-36, -18, 31, 13)),
        ("Bullet", "LEVEL_MUSKETBALL", Rect::new(-6, -5, 5, 4)),
        ("Dynamite", "GAME_DYNAMITELIT", Rect::new(-13, -14, 14, 9)),
        ("RatBomb", "LEVEL_RATBOMB", Rect::new(-17, 33, 6, 54)),
    ];
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    for (logic, set, want) in cases {
        let first = level.image_frames(set).map(|(f, _)| f);
        let known = first.and_then(|f| level.frame_box(set, f));
        println!("{logic}: first frame {first:?}, box {known:?}");
        assert_eq!(known, Some(want), "{logic}");
        let id = sim
            .with_world(|w| {
                projectile::spawn(
                    w,
                    ProjectileSpec {
                        logic,
                        x: 5000,
                        y: 300,
                        image_set: set,
                        ..ProjectileSpec::default()
                    },
                )
            })
            .unwrap();
        assert_eq!(sim.objects().get(id).unwrap().move_rect, want, "{logic}");
    }
}

#[test]
fn install_level1_shows_no_splash() {
    let Some(level) = level1() else { return };
    let mut sim = Sim::new(level, SimConfig::default());
    let (x, y, _, _) = claw(&sim);
    assert_eq!(
        sim.with_world(|w| effect::spawn(w, EffectKind::Splash, x, y)),
        None,
        "LEVEL1 has no splash art"
    );
}
