//! Projectiles, explosions and combat effects on synthetic levels: flight
//! speeds, walls, lifetime, the dynamite arc, bounces and fuse, the rat's
//! bomb, explosions hitting each object once, chains, and effects that
//! remove themselves.

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::{AudioCue, SoundChannel, SoundCue};
use claw_sim::container::CollisionEvent;
use claw_sim::level::ImageFrames;
use claw_sim::logic::effect::{self, EffectKind};
use claw_sim::logic::new_object;
use claw_sim::logic::projectile::{self, Phase, ProjectileSpec};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

/// An element of `frames` (frame, ms) with the event value `event`.
fn anim(key: &str, frames: &[(i32, i32)], once: bool, events: &[(usize, i32)]) -> Arc<Animation> {
    let mut a = if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    };
    for &(i, e) in events {
        a.frames[i].event = e;
    }
    Arc::new(a)
}

fn frames(n: i32, ms: i32) -> Vec<(i32, i32)> {
    (1..=n).map(|f| (f, ms)).collect()
}

/// The animations projectiles and effects play, built in code with the
/// frame counts and timings of the game's own.
fn add_animations(l: &mut SimLevel) {
    let list = [
        anim("GAME_CYCLE50", &frames(4, 50), false, &[]),
        anim("GAME_DYNAMITELIT", &frames(8, 50), false, &[]),
        anim("GAME_DYNAMITEHORZ", &frames(7, 50), false, &[]),
        anim("GAME_DYNAMITEEXPLO", &frames(18, 60), true, &[(2, 1)]),
        anim("LEVEL_RATBOMB_FALL", &frames(6, 50), false, &[]),
        anim("LEVEL_RATBOMB_FALLEASTWEST", &frames(5, 50), false, &[]),
        anim("LEVEL_RATBOMB_EXPLODE", &frames(12, 50), true, &[(2, 1)]),
        anim("GAME_SMLHITBURST", &frames(12, 20), true, &[]),
        anim("GAME_MEDHITBURST", &frames(12, 20), true, &[]),
        anim("GAME_BIGHITBURST", &frames(12, 20), true, &[]),
        anim("GAME_GLITTER1", &frames(6, 50), true, &[]),
        anim("GAME_SPLASH", &frames(7, 65), true, &[]),
        anim("GAME_LIGHTFX_FLARE", &[(1, 100)], false, &[]),
    ];
    for a in list {
        l.animations.insert(a.key.clone(), a);
    }
}

/// A 200-tile-wide level: Solid floor whose top is y = 960, a Solid wall
/// whose left face is x = 1600 from y = 640 down, and the animations.
/// Claw starts on the floor at x = 1000 (feet on 959).
fn arena() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    for r in rows.iter_mut().skip(10) {
        r.replace_range(25..26, "#");
    }
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    add_animations(&mut l);
    l
}

/// The arena with Claw and the camera settled.
fn sim() -> Sim {
    let mut sim = Sim::new(arena(), SimConfig::default());
    for _ in 0..60 {
        sim.step(TickInput::default());
    }
    sim
}

fn step(sim: &mut Sim) {
    sim.step(TickInput::default());
}

fn pistol(x: i32, y: i32, vx: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "PistolBullet",
        x,
        y,
        z: 99,
        vx,
        damage: 8,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_BULLETS",
        ..ProjectileSpec::default()
    }
}

fn dynamite(x: i32, y: i32, power: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "Dynamite",
        x,
        y,
        z: 101,
        damage: 15,
        type_flags: types::PISTOL_SHOT,
        hit_type_flags: types::ENEMY,
        image_set: "GAME_DYNAMITELIT",
        power,
        ..ProjectileSpec::default()
    }
}

fn rat_bomb(x: i32, y: i32, vx: i32, vy: i32) -> ProjectileSpec {
    ProjectileSpec {
        logic: "RatBomb",
        x,
        y,
        z: 499,
        vx,
        vy,
        special_flags: if vx != 0 { special::OWN_IMPACT } else { 0 },
        image_set: "LEVEL_RATBOMB",
        ..ProjectileSpec::default()
    }
}

fn launch(sim: &mut Sim, spec: ProjectileSpec) -> ObjectId {
    sim.with_world(|w| projectile::spawn(w, spec))
        .expect("launched")
}

fn obj(sim: &Sim, id: ObjectId) -> Option<&Object> {
    sim.objects().get(id)
}

fn pdata(sim: &Sim, id: ObjectId) -> projectile::ProjectileData {
    projectile::data(obj(sim, id).expect("exists"))
        .expect("projectile data")
        .clone()
}

/// A target that records hits from the level's own shots (explosions) and
/// any other `kinds`, with a 21x21 box.
fn target(sim: &mut Sim, x: i32, y: i32, kinds: u32) -> ObjectId {
    let mut o = Object::new(0, x, y, 300, "Target".into(), LogicKind::Inert);
    o.flags = flags::WORLD | flags::ALWAYS_ACTIVE;
    o.hit_type_flags = kinds;
    o.hit_rect = Rect::new(-10, -10, 10, 10);
    o.hit_handler = true;
    sim.spawn(o)
}

/// Hits `victim` took from `attacker` in the last tick.
fn hits(sim: &Sim, attacker: ObjectId, victim: ObjectId) -> usize {
    sim.collision_events()
        .iter()
        .filter(|e| **e == CollisionEvent::Hit { attacker, victim })
        .count()
}

/// Runs until `id` is gone (at most `limit` ticks); returns the ticks run
/// and the position it was last seen at.
fn run_until_gone(sim: &mut Sim, id: ObjectId, limit: usize) -> (usize, (i32, i32)) {
    let mut last = obj(sim, id).map(|o| (o.x, o.y)).unwrap();
    for n in 1..=limit {
        step(sim);
        match obj(sim, id) {
            Some(o) => last = (o.x, o.y),
            None => return (n, last),
        }
    }
    panic!("still there after {limit} ticks at {last:?}");
}

fn count_logic(sim: &Sim, logic: &str) -> usize {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .count()
}

// ------------------------------------------------------------- straight

#[test]
fn a_pistol_shot_flies_at_700_px_s_and_goes_at_a_wall() {
    let mut sim = sim();
    let b = launch(&mut sim, pistol(1100, 850, 700));
    let o = obj(&sim, b).unwrap();
    assert!(o.is_listed());
    assert_eq!(o.kind, LogicKind::Game(claw_sim::GameLogic::Projectile));
    assert_eq!(
        (&*o.image_set, o.move_rect),
        ("GAME_BULLETS", Rect::new(-2, -2, 1, 1))
    );
    assert_eq!(
        o.special_flags & projectile::RESPAWN_PURGE,
        projectile::RESPAWN_PURGE
    );
    assert!(o.flags & flags::DESTROY_OUTSIDE != 0);
    // 700 px/s is 12.6 px per 18 ms tick, kept as a fraction and drawn at
    // the whole pixel.
    let mut fx = 1100.0;
    for n in 1..=20 {
        step(&mut sim);
        fx += 700.0 * 0.018;
        let x = obj(&sim, b).unwrap().x;
        assert_eq!(x, fx as i32, "tick {n}");
        assert_eq!(obj(&sim, b).unwrap().y, 850);
    }
    // The right edge of its box (x + 1) cannot enter the wall at 1600: it
    // stops short, spends a tick there and is removed.
    let (n, (x, _)) = run_until_gone(&mut sim, b, 60);
    assert!((1586..=1598).contains(&x), "stopped at {x}");
    assert!(n <= 40, "{n}");
}

#[test]
fn a_musket_ball_flies_at_500_px_s_and_mirrors_when_flying_left() {
    let mut sim = sim();
    let spec = ProjectileSpec {
        logic: "Bullet",
        x: 1500,
        y: 700,
        z: 300,
        vx: -500,
        damage: 10,
        type_flags: types::ENEMY_SHOT,
        image_set: "LEVEL_MUSKETBALL",
        ..ProjectileSpec::default()
    };
    let b = launch(&mut sim, spec);
    assert!(obj(&sim, b).unwrap().draw_flags & 0x2 != 0, "mirrored");
    // Its boxes face left too.
    assert_eq!(obj(&sim, b).unwrap().move_rect, Rect::new(-5, -5, 6, 4));
    assert_eq!(obj(&sim, b).unwrap().attack_rect, Rect::new(-5, -5, 6, 4));
    for n in 1..=10 {
        step(&mut sim);
        assert_eq!(obj(&sim, b).unwrap().x, 1500 - 9 * n);
    }
}

#[test]
fn a_projectile_moves_on_its_own_clock_of_doubles() {
    // A musket ball at 500 px/s moves 9 px a tick, but each step covers
    // the game time since its last step as a difference of doubles
    // (seconds at the step's end), so launched before tick 63 its second
    // step is a hair under 0.018 s and truncates to 8 px; the third makes
    // up for it. The cannonball by the second level's ladder lost its
    // pixel this way.
    let mut sim = sim();
    while sim.tick() < 63 {
        step(&mut sim);
    }
    let b = launch(
        &mut sim,
        ProjectileSpec {
            logic: "Bullet",
            x: 1000,
            y: 700,
            z: 300,
            vx: 500,
            damage: 10,
            type_flags: types::ENEMY_SHOT,
            image_set: "LEVEL_MUSKETBALL",
            ..ProjectileSpec::default()
        },
    );
    let mut xs = Vec::new();
    for _ in 0..4 {
        step(&mut sim);
        xs.push(obj(&sim, b).unwrap().x);
    }
    assert_eq!(xs, [1009, 1017, 1027, 1036]);
}

#[test]
fn shots_go_when_they_leave_their_window_or_the_level_but_dynamite_does_not() {
    // No wall to the left of Claw: a shot flying left leaves the 1600 px
    // activation window around the view (800 px either side of its
    // centre) and is removed there.
    let mut sim = sim();
    let centre = sim.view().centre.0;
    let b = launch(&mut sim, pistol(900, 850, -700));
    let (_, (x, _)) = run_until_gone(&mut sim, b, 200);
    assert!(x <= centre - 790 && x >= centre - 820, "removed at {x}");
    // A shot launched outside the level goes at once.
    let mut sim = Sim::new(arena(), SimConfig::default());
    let b = launch(&mut sim, pistol(1000, -40, 700));
    let (n, _) = run_until_gone(&mut sim, b, 5);
    assert_eq!(n, 1);
    // Dynamite stays live wherever it is until it explodes.
    let mut sim = Sim::new(arena(), SimConfig::default());
    let d = launch(&mut sim, dynamite(7000, 300, 0));
    assert!(obj(&sim, d).unwrap().flags & flags::ALWAYS_ACTIVE != 0);
    for _ in 0..60 {
        step(&mut sim);
    }
    assert!(obj(&sim, d).unwrap().is_listed());
}

#[test]
fn a_shot_launched_inside_a_wall_goes_on_its_first_tick() {
    let mut sim = sim();
    let b = launch(&mut sim, pistol(1620, 900, 700));
    let (n, (x, _)) = run_until_gone(&mut sim, b, 5);
    assert_eq!((n, x), (1, 1620), "it never moves");
}

#[test]
fn targets_decide_hits_and_the_magic_claw_pierces() {
    let mut sim = sim();
    let enemy = target(
        &mut sim,
        1300,
        850,
        types::ENEMY | types::PISTOL_SHOT | 0x200_0000,
    );
    let magic = launch(
        &mut sim,
        ProjectileSpec {
            logic: "MagicBullet",
            x: 1100,
            y: 850,
            z: 101,
            vx: 700,
            damage: 25,
            type_flags: 0x200_0000,
            hit_type_flags: types::ENEMY,
            special_flags: 8,
            image_set: "GAME_MAGICCLAW",
            ..ProjectileSpec::default()
        },
    );
    let flare = pdata(&sim, magic).flare.expect("a flare");
    assert_eq!(&*obj(&sim, flare).unwrap().logic, "Flare");
    let mut hit_ticks = Vec::new();
    let mut sparkles = 0;
    for n in 1..=30 {
        let before = count_logic(&sim, "GlitterBaby");
        step(&mut sim);
        if hits(&sim, magic, enemy) > 0 {
            hit_ticks.push(n);
        }
        if n <= 5 {
            sparkles += count_logic(&sim, "GlitterBaby") - before;
        }
        // The flare rides on the shot (where it was before this move).
        let (m, f) = (obj(&sim, magic).unwrap(), obj(&sim, flare).unwrap());
        assert!(
            m.x - f.x > 0 && m.x - f.x <= 13 && f.y == m.y,
            "{} {}",
            m.x,
            f.x
        );
    }
    // The hit is the target's to handle; it does not stop the magic claw,
    // which overlaps the target over several passes.
    assert!(hit_ticks.len() > 1, "{hit_ticks:?}");
    assert!(obj(&sim, magic).unwrap().x > 1300 + 36);
    // Two sparkles a tick, each gone after its 300 ms twinkle.
    assert_eq!(sparkles, 10);
    // At the wall the shot goes, and its flare with it.
    let (_, (x, _)) = run_until_gone(&mut sim, magic, 60);
    assert!((1550..1600).contains(&x), "{x}");
    step(&mut sim);
    assert!(obj(&sim, flare).is_none(), "the flare went with its shot");
    for _ in 0..40 {
        step(&mut sim);
    }
    assert_eq!(count_logic(&sim, "GlitterBaby"), 0);
    // A pistol shot is removed by the target that takes it.
    let shot = launch(&mut sim, pistol(1200, 850, 700));
    let mut taken = false;
    for _ in 0..20 {
        step(&mut sim);
        if hits(&sim, shot, enemy) > 0 {
            sim.with_world(|w| w.destroy(shot));
            taken = true;
            break;
        }
    }
    assert!(taken);
    step(&mut sim);
    assert!(obj(&sim, shot).is_none());
    assert_eq!(hits(&sim, shot, enemy), 0);
}

#[test]
fn an_enemy_shot_hurts_claw() {
    let mut sim = sim();
    let claw = sim.player().map(|o| (o.x, o.y)).unwrap();
    let claw_id = sim.player().map(|o| o.serial).unwrap();
    let health = sim.stats().health;
    let ball = launch(
        &mut sim,
        ProjectileSpec {
            logic: "Bullet",
            x: claw.0 + 120,
            y: claw.1,
            z: 300,
            vx: -500,
            damage: 10,
            type_flags: types::ENEMY_SHOT,
            image_set: "LEVEL_MUSKETBALL",
            ..ProjectileSpec::default()
        },
    );
    // Claw's hit handler takes the damage at the first contact (removing
    // the ball there is his handler's job too).
    let _ = claw_id;
    let mut hit = false;
    for _ in 0..20 {
        step(&mut sim);
        let first = sim
            .collision_events()
            .iter()
            .any(|e| matches!(*e, CollisionEvent::Hit { attacker, .. } if attacker == ball));
        if first {
            hit = true;
            break;
        }
    }
    assert!(hit);
    assert_eq!(sim.stats().health, health - 10);
}

// --------------------------------------------------------------- dynamite

#[test]
fn dynamite_flies_the_throw_arc() {
    for (power, vx, vy) in [
        (0, 0.0, 0.0),
        (46, 214.5, -306.5),
        (100, 316.2, -516.2),
        (-200, -447.2, -847.2),
    ] {
        let (gx, gy) = projectile::throw_velocity(power);
        assert!(
            (gx - vx).abs() < 0.1 && (gy - vy).abs() < 0.1,
            "{power}: {gx} {gy}"
        );
    }
    let mut sim = sim();
    let d = launch(&mut sim, dynamite(1100, 800, 200));
    let o = obj(&sim, d).unwrap();
    assert_eq!(o.type_flags, types::ENEMY, "harmless until it explodes");
    assert!(o.flags & flags::HARMLESS != 0);
    assert_eq!(o.attack_rect, Rect::new(-50, -50, 50, 50));
    assert_eq!(o.anim.key(), Some("GAME_DYNAMITELIT"));
    let mut top = 800;
    for _ in 0..30 {
        step(&mut sim);
        top = top.min(obj(&sim, d).unwrap().y);
    }
    // vy = -847 under 2000 px/s² climbs about 179 px.
    assert!((800 - top - 179).abs() <= 3, "apex {top}");
}

#[test]
fn dynamite_bounces_twice_then_lies_still() {
    let mut sim = sim();
    let mut spec = dynamite(1100, 700, 100);
    spec.fuse_ms = Some(10_000);
    let d = launch(&mut sim, spec);
    let mut landings = Vec::new();
    let mut was_standing = false;
    for n in 1..=200 {
        step(&mut sim);
        let o = obj(&sim, d).unwrap();
        let standing = o.physics_type == ptype::STAND;
        if standing && !was_standing {
            landings.push((n, o.x, o.y));
        }
        was_standing = standing;
    }
    let data = pdata(&sim, d);
    assert_eq!(data.bounces, 2);
    assert!(data.frozen, "lying still");
    assert_eq!(landings.len(), 3, "{landings:?}");
    // Every landing rests the box (bottom 9) on the floor at 960.
    assert!(landings.iter().all(|&(_, _, y)| y == 950), "{landings:?}");
    // Each bounce halves the horizontal speed, so the hops get shorter.
    let hop1 = landings[1].1 - landings[0].1;
    let hop2 = landings[2].1 - landings[1].1;
    assert!(hop1 > 0 && hop2 > 0 && hop2 < hop1, "{landings:?}");
    let o = obj(&sim, d).unwrap();
    assert!(
        o.anim.animation().is_none(),
        "the lying stick stops animating"
    );
    let rest = (o.x, o.y);
    step(&mut sim);
    let o = obj(&sim, d).unwrap();
    assert_eq!((o.x, o.y), rest);
}

#[test]
fn dynamite_goes_off_after_its_fuse_and_hits_once() {
    let mut sim = sim();
    let d = launch(&mut sim, dynamite(1300, 949, 0));
    let near = [
        target(&mut sim, 1300, 900, types::LEVEL_SHOT),
        target(&mut sim, 1250, 950, types::LEVEL_SHOT),
        target(&mut sim, 1355, 950, types::LEVEL_SHOT),
    ];
    let far = target(&mut sim, 1400, 950, types::LEVEL_SHOT);
    let mut sparkles = std::collections::BTreeSet::new();
    let mut exploded = None;
    let mut hit_at = Vec::new();
    let mut counts = [0usize; 4];
    let mut sounds = Vec::new();
    let mut shakes = Vec::new();
    for n in 1..=150 {
        let start_ms = sim.tick() * 18;
        let shake_before = sim.camera().shake_until_ms;
        let snap = sim.step(TickInput::default());
        sounds.extend(snap.audio.iter().cloned().map(|c| (n, c)));
        if sim.camera().shake_until_ms != shake_before {
            shakes.push((n, sim.camera().shake_until_ms - start_ms));
        }
        sparkles.extend(
            sim.objects()
                .all()
                .filter(|(_, o)| &*o.logic == "GlitterBaby")
                .map(|(id, _)| id),
        );
        let Some(o) = obj(&sim, d) else {
            assert!(exploded.is_some());
            break;
        };
        if exploded.is_none() && pdata(&sim, d).phase == Phase::Exploding {
            exploded = Some(n);
            assert_eq!(o.type_flags, types::LEVEL_SHOT);
            assert_eq!(&*o.image_set, "GAME_DYNAMITEEXPLO");
        }
        for (i, &v) in near.iter().chain([far].iter()).enumerate() {
            let h = hits(&sim, d, v);
            counts[i] += h;
            if h > 0 {
                hit_at.push(n);
            }
        }
    }
    // The fuse burns 1250 ms from the throw: it fires in the first tick
    // that starts that long after it was lit (1260 ms, the 71st).
    let exploded = exploded.expect("exploded");
    assert_eq!(exploded, 71);
    // One sparkle a tick while it burns, the tick it goes off included.
    assert_eq!(sparkles.len(), 71);
    // The boom sounds when it goes off, centred.
    assert_eq!(
        sounds,
        vec![(
            exploded,
            AudioCue::Play(SoundCue {
                key: "GAME_KEGBOOM".into(),
                channel: SoundChannel::Effect,
                ..SoundCue::default()
            })
        )]
    );
    // Its box hurts on the animation's hit frame, the 9th tick of the
    // explosion, every target inside it once and nothing outside.
    assert_eq!(counts, [1, 1, 1, 0]);
    hit_at.dedup();
    assert_eq!(hit_at, vec![exploded + 9]);
    // The screen shakes for half a second from the tick it hurts.
    assert_eq!(
        shakes,
        vec![(exploded + 9, u64::from(projectile::DYNAMITE_SHAKE_MS))]
    );
    assert!(obj(&sim, d).is_none(), "removed when its animation ends");
}

#[test]
fn claw_beside_his_own_dynamite_takes_its_damage_once() {
    let mut sim = sim();
    let (x, y) = sim.player().map(|o| (o.x, o.y)).unwrap();
    let health = sim.stats().health;
    let d = launch(&mut sim, dynamite(x + 30, y + 40, 0));
    let (_, _) = run_until_gone(&mut sim, d, 200);
    assert_eq!(sim.stats().health, health - 15);
}

#[test]
fn dynamite_lit_inside_solid_ground_waits_for_its_fuse() {
    let mut sim = sim();
    let d = launch(&mut sim, dynamite(1300, 955, 200));
    let o = obj(&sim, d).unwrap();
    assert_eq!(o.z, projectile::BURIED_Z);
    assert!(pdata(&sim, d).frozen);
    for _ in 0..60 {
        step(&mut sim);
    }
    assert_eq!(obj(&sim, d).map(|o| (o.x, o.y)), Some((1300, 955)));
}

// ---------------------------------------------------------------- rat bomb

#[test]
fn a_dropped_rat_bomb_explodes_where_it_lands() {
    let mut sim = sim();
    // Its box runs 33 to 54 px below its position.
    let b = launch(&mut sim, rat_bomb(1200, 700, 0, 250));
    let o = obj(&sim, b).unwrap();
    assert!(o.flags & flags::HARMLESS != 0 && o.flags & flags::SAFE != 0);
    assert_eq!((o.type_flags, o.damage), (types::ENEMY_SHOT, 15));
    assert_eq!(o.anim.key(), Some("LEVEL_RATBOMB_FALL"));
    let victim = target(&mut sim, 1200, 930, types::LEVEL_SHOT | types::ENEMY_SHOT);
    let mut exploded = None;
    let mut hit_at = Vec::new();
    let mut boom = 0;
    for n in 1..=150 {
        let snap = sim.step(TickInput::default());
        boom += snap
            .audio
            .iter()
            .filter(|c| matches!(c, AudioCue::Play(s) if s.key == "GAME_BOMBEXP"))
            .count();
        let Some(o) = obj(&sim, b) else { break };
        if hits(&sim, b, victim) > 0 {
            hit_at.push(n);
        }
        if exploded.is_none() && pdata(&sim, b).phase == Phase::Exploding {
            exploded = Some((n, o.y));
            assert_eq!(o.type_flags, types::LEVEL_SHOT);
        }
    }
    let (n, y) = exploded.expect("exploded");
    // Landed with the bottom of its box on the floor.
    assert_eq!(y + 54, 959, "at {y}");
    assert!(n > 5, "{n}");
    assert_eq!(boom, 1);
    // Harmless while falling; its grown box hits on the 7th tick of the
    // explosion.
    assert_eq!(hit_at, vec![n + 7]);
    assert!(obj(&sim, b).is_none());
}

#[test]
fn a_thrown_rat_bomb_explodes_at_a_wall_with_a_grown_box() {
    let mut sim = sim();
    let b = launch(&mut sim, rat_bomb(1450, 800, 500, 0));
    let o = obj(&sim, b).unwrap();
    // Thrown right it is drawn mirrored, its boxes flipped.
    assert!(o.draw_flags & 0x2 != 0);
    assert_eq!(o.attack_rect, Rect::new(-6, 33, 17, 54));
    assert!(o.flags & flags::HARMLESS == 0, "it can hit in flight");
    let mut n = 0;
    while pdata(&sim, b).phase != Phase::Exploding {
        step(&mut sim);
        n += 1;
        assert!(n < 40);
        // 500 px/s, straight.
        assert_eq!(obj(&sim, b).unwrap().y, 800);
    }
    assert!(obj(&sim, b).unwrap().x + 17 < 1600);
    for _ in 0..7 {
        step(&mut sim);
    }
    assert_eq!(
        obj(&sim, b).unwrap().attack_rect,
        Rect::new(-46, -7, 57, 94),
        "grown by 40 px on every side when it arms"
    );
}

#[test]
fn a_bomb_that_hits_claw_goes_off_on_him_and_other_shots_do_not() {
    let mut sim = sim();
    let b = launch(&mut sim, rat_bomb(1300, 800, -500, 0));
    let p = launch(&mut sim, pistol(1300, 700, 700));
    step(&mut sim);
    sim.with_world(|w| {
        projectile::impact(w, b);
        projectile::impact(w, p);
    });
    assert_eq!(pdata(&sim, b).phase, Phase::Exploding);
    assert_eq!(obj(&sim, b).unwrap().type_flags, types::LEVEL_SHOT);
    assert_eq!(pdata(&sim, p).phase, Phase::Flying);
    // Going off again restarts the explosion.
    for _ in 0..7 {
        step(&mut sim);
    }
    assert!(pdata(&sim, b).armed);
    sim.with_world(|w| projectile::impact(w, b));
    assert!(!pdata(&sim, b).armed);
    assert_eq!(
        obj(&sim, b).unwrap().anim.key(),
        Some("LEVEL_RATBOMB_EXPLODE")
    );
}

// --------------------------------------------------------------- explosions

#[test]
fn an_explosion_hits_each_object_once_and_chains() {
    let mut sim = sim();
    let d = launch(&mut sim, dynamite(1300, 949, 0));
    // A keg-like object beside the dynamite that explodes when hit, and an
    // object only that second explosion reaches.
    let keg = target(&mut sim, 1340, 940, types::LEVEL_SHOT);
    let next = target(&mut sim, 1420, 940, types::LEVEL_SHOT);
    let mut keg_hits = 0;
    let mut next_hits = 0;
    let mut second = None;
    for _ in 0..200 {
        step(&mut sim);
        for (attacker, victim) in sim.collision_events().iter().filter_map(|e| match *e {
            CollisionEvent::Hit { attacker, victim } => Some((attacker, victim)),
            _ => None,
        }) {
            if victim == keg {
                keg_hits += 1;
                assert_eq!(attacker, d);
            }
            if victim == next {
                next_hits += 1;
                assert_eq!(Some(attacker), second);
            }
        }
        // The keg's own handler would explode it; here the test does.
        if keg_hits == 1 && second.is_none() {
            second = sim.with_world(|w| {
                effect::spawn(
                    w,
                    EffectKind::Explosion {
                        damage: 20,
                        radius: 70,
                        source: Some(keg),
                    },
                    1340,
                    940,
                )
            });
            assert!(second.is_some());
        }
        if obj(&sim, d).is_none() && second.is_some_and(|s| obj(&sim, s).is_none()) {
            break;
        }
    }
    assert_eq!(
        (keg_hits, next_hits),
        (1, 1),
        "each hit once; the keg not by itself"
    );
    let k = obj(&sim, keg).unwrap();
    assert_eq!(k.flags & flags::SAFE, 0, "the source is hittable again");
    assert!(second.is_some_and(|s| obj(&sim, s).is_none()), "removed");
}

#[test]
fn a_stand_alone_explosion_hits_in_its_first_pass_and_goes() {
    let mut sim = sim();
    let a = target(&mut sim, 1300, 900, types::LEVEL_SHOT);
    let b = target(&mut sim, 1390, 900, types::LEVEL_SHOT);
    let e = sim
        .with_world(|w| {
            effect::spawn(
                w,
                EffectKind::Explosion {
                    damage: 10,
                    radius: 60,
                    source: None,
                },
                1300,
                900,
            )
        })
        .unwrap();
    assert!(obj(&sim, e).unwrap().image_set.is_empty(), "draws nothing");
    step(&mut sim);
    assert_eq!((hits(&sim, e, a), hits(&sim, e, b)), (1, 0));
    step(&mut sim);
    assert!(obj(&sim, e).is_none());
    assert_eq!(hits(&sim, e, a), 0);
}

// ----------------------------------------------------------------- effects

#[test]
fn a_hit_burst_follows_its_owner_and_removes_itself() {
    let mut sim = sim();
    let claw = sim.player().map(|o| (o.x, o.y, o.z)).unwrap();
    let burst = sim
        .with_world(|w| {
            let id = w.claw_id.unwrap();
            effect::spawn_hit_burst(w, claw.0 + 5, claw.1 - 3, claw.2 + 1, Some(id), true)
        })
        .expect("Claw is on screen");
    let o = obj(&sim, burst).unwrap();
    assert_eq!(&*o.image_set, "GAME_CLAWHIT");
    assert!(o.flags & flags::NO_COLLIDE != 0);
    assert!(o.anim.key().is_some_and(|k| k.ends_with("HITBURST")));
    // Claw moves; the burst keeps its offset from him.
    sim.teleport_player(claw.0 + 40, claw.1);
    let (n, _) = run_until_gone(&mut sim, burst, 40);
    assert!((20..=26).contains(&n), "{n}");
    // No burst for an owner off screen, or without one: enemy art, fixed.
    let far = target(&mut sim, 9000, 300, 0);
    assert_eq!(
        sim.with_world(|w| effect::spawn_hit_burst(w, 9000, 300, 0, Some(far), false)),
        None
    );
    let lone = sim
        .with_world(|w| effect::spawn(w, EffectKind::HitBurst, 1200, 900))
        .unwrap();
    let o = obj(&sim, lone).unwrap();
    assert_eq!((&*o.image_set, o.z), ("GAME_ENEMYHIT", effect::HIT_BURST_Z));
    run_until_gone(&mut sim, lone, 40);
}

#[test]
fn a_followed_burst_moves_with_its_owner() {
    let mut sim = sim();
    let owner = target(&mut sim, 1200, 900, 0);
    let burst = sim
        .with_world(|w| effect::spawn_hit_burst(w, 1204, 896, 400, Some(owner), false))
        .unwrap();
    step(&mut sim);
    sim.with_world(|w| w.objects.get_mut(owner).unwrap().x = 1250);
    step(&mut sim);
    assert_eq!(obj(&sim, burst).map(|o| (o.x, o.y)), Some((1254, 896)));
    sim.with_world(|w| w.destroy(owner));
    step(&mut sim);
    step(&mut sim);
    assert!(obj(&sim, burst).is_none(), "gone with its owner");
}

#[test]
fn glitter_trails_its_target_until_removed() {
    let mut sim = sim();
    let claw = sim.player().map(|o| (o.x, o.y, o.z, o.move_rect)).unwrap();
    let mother = sim
        .with_world(|w| {
            let id = w.claw_id.unwrap();
            effect::spawn_glitter(w, id)
        })
        .unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..10 {
        let before = count_logic(&sim, "GlitterBaby");
        step(&mut sim);
        assert_eq!(count_logic(&sim, "GlitterBaby") - before, 2, "two a tick");
        for (id, o) in sim.objects().all() {
            if &*o.logic == "GlitterBaby" && seen.insert(id) {
                let r = claw.3.translate(claw.0, claw.1);
                assert!(r.contains(o.x, o.y), "on Claw: {} {}", o.x, o.y);
                assert_eq!(o.z, claw.2 + 1);
            }
        }
    }
    // Each sparkle lasts its six 50 ms frames (19 ticks at this step), so
    // a steady trail holds about 38.
    for _ in 0..30 {
        step(&mut sim);
    }
    let live = count_logic(&sim, "GlitterBaby");
    assert!((34..=40).contains(&live), "{live}");
    sim.with_world(|w| w.destroy(mother));
    for _ in 0..30 {
        step(&mut sim);
    }
    assert_eq!(count_logic(&sim, "GlitterBaby"), 0);
    assert!(obj(&sim, mother).is_none());
}

#[test]
fn effects_remove_themselves_after_their_animation() {
    let mut sim = sim();
    let splash = sim
        .with_world(|w| effect::spawn(w, EffectKind::Splash, 1200, 900))
        .unwrap();
    assert_eq!(&*obj(&sim, splash).unwrap().image_set, "LEVEL_SPLASH");
    // Seven 65 ms frames.
    let (n, _) = run_until_gone(&mut sim, splash, 60);
    assert!((25..=30).contains(&n), "{n}");
    // Without its animation an effect goes at once.
    let mut bare = Sim::new(
        SimLevel::synthetic(TileMap::from_ascii(64, &["...", "###"]), (64, 0)),
        SimConfig::default(),
    );
    let b = bare
        .with_world(|w| effect::spawn(w, EffectKind::HitBurst, 64, 20))
        .unwrap();
    let (n, _) = run_until_gone(&mut bare, b, 3);
    assert_eq!(n, 1);
}

#[test]
fn a_splash_needs_its_art_on_levels_that_list_their_image_sets() {
    let mut l = arena();
    l.images
        .insert("GAME_SPARKLE".into(), ImageFrames::range(1, 6));
    let mut sim = Sim::new(l.clone(), SimConfig::default());
    assert_eq!(
        sim.with_world(|w| effect::spawn(w, EffectKind::Splash, 1200, 900)),
        None
    );
    l.images
        .insert(effect::SPLASH_IMAGE.into(), ImageFrames::range(1, 7));
    let mut sim = Sim::new(l, SimConfig::default());
    assert!(
        sim.with_world(|w| effect::spawn(w, EffectKind::Splash, 1200, 900))
            .is_some()
    );
}

#[test]
fn a_burst_shows_only_while_its_owners_drawn_box_is_on_screen() {
    // The owner's drawn box reaches 300 px to its left, so it shows while
    // its position is just right of the view.
    let mut l = arena();
    let mut frames = ImageFrames::range(1, 1);
    frames.boxes.insert(1, Rect::new(-300, -10, 0, 10));
    l.images.insert("TEST_WIDE".into(), frames);
    let mut sim = Sim::new(l, SimConfig::default());
    for _ in 0..60 {
        step(&mut sim);
    }
    let view = sim.with_world(|w| w.view);
    let owner = target(&mut sim, view.right + 200, 900, 0);
    sim.with_world(|w| {
        let o = w.objects.get_mut(owner).unwrap();
        o.image_set = "TEST_WIDE".into();
        o.frame = 1;
    });
    let shown = sim
        .with_world(|w| effect::spawn_hit_burst(w, view.right + 200, 900, 0, Some(owner), false));
    assert!(shown.is_some());
    sim.with_world(|w| w.objects.get_mut(owner).unwrap().x = view.right + 400);
    let hidden = sim
        .with_world(|w| effect::spawn_hit_burst(w, view.right + 400, 900, 0, Some(owner), false));
    assert_eq!(hidden, None);
    // The burst already shown goes with its owner off screen.
    step(&mut sim);
    step(&mut sim);
    assert!(obj(&sim, shown.unwrap()).is_none());
}

#[test]
fn boxes_come_from_the_first_frame_when_the_level_knows_it() {
    let mut l = arena();
    let mut frames = ImageFrames::range(3, 5);
    frames.boxes.insert(3, Rect::new(-4, -3, 3, 2));
    frames.boxes.insert(4, Rect::new(-40, -30, 30, 20));
    l.images.insert("GAME_BULLETS".into(), frames);
    let mut sim = Sim::new(l, SimConfig::default());
    let p = launch(&mut sim, pistol(1200, 900, 700));
    let o = obj(&sim, p).unwrap();
    assert_eq!(o.frame, 3);
    assert_eq!(
        (o.move_rect, o.attack_rect),
        (Rect::new(-4, -3, 3, 2), Rect::new(-4, -3, 3, 2))
    );
    // Without it, the game's own box stands in.
    let mut sim = Sim::new(arena(), SimConfig::default());
    let p = launch(&mut sim, pistol(1200, 900, 700));
    assert_eq!(obj(&sim, p).unwrap().move_rect, Rect::new(-2, -2, 1, 1));
}

// ------------------------------------------------------------- the family

#[test]
fn unknown_logics_launch_nothing_and_respawn_purges_projectiles() {
    let mut sim = sim();
    let none = sim.with_world(|w| {
        projectile::spawn(
            w,
            ProjectileSpec {
                logic: "NoSuchShot",
                ..ProjectileSpec::default()
            },
        )
    });
    assert_eq!(none, None);
    // A known logic that does not fly yet leaves an inert placeholder.
    let cannon = sim
        .with_world(|w| {
            projectile::spawn(
                w,
                ProjectileSpec {
                    logic: "WolvingtonBullet",
                    x: 1200,
                    y: 800,
                    ..ProjectileSpec::default()
                },
            )
        })
        .unwrap();
    assert_eq!(obj(&sim, cannon).unwrap().kind, LogicKind::Inert);
    let shot = launch(&mut sim, pistol(1100, 850, 700));
    let stick = launch(&mut sim, dynamite(1100, 850, 100));
    sim.with_world(|w| projectile::purge(w.objects));
    step(&mut sim);
    assert!(obj(&sim, shot).is_none() && obj(&sim, stick).is_none());
    // Placed in a level, a projectile record stays put.
    let mut placed = new_object("Bullet", 1200, 800, 0);
    placed.flags |= flags::ALWAYS_ACTIVE;
    let p = sim.spawn(placed);
    step(&mut sim);
    assert_eq!(obj(&sim, p).map(|o| (o.x, o.y)), Some((1200, 800)));
}

#[test]
fn a_cloned_sim_flies_the_same_projectiles() {
    let mut sim = sim();
    launch(&mut sim, dynamite(1100, 800, 150));
    launch(
        &mut sim,
        ProjectileSpec {
            logic: "MagicBullet",
            x: 1100,
            y: 850,
            z: 101,
            vx: 700,
            damage: 25,
            type_flags: 0x200_0000,
            image_set: "GAME_MAGICCLAW",
            ..ProjectileSpec::default()
        },
    );
    for _ in 0..20 {
        step(&mut sim);
    }
    let mut copy = sim.clone();
    for _ in 0..100 {
        let a = sim.step(TickInput::default());
        let b = copy.step(TickInput::default());
        assert_eq!(a, b);
    }
    assert_eq!(sim.rng().state(), copy.rng().state());
}

#[test]
fn every_runtime_set_and_animation_is_listed() {
    let sets = claw_sim::logic::runtime_image_sets();
    for s in projectile::IMAGE_SETS.iter().chain(effect::IMAGE_SETS) {
        assert!(sets.contains(s), "{s}");
    }
    let prefixes = claw_sim::logic::runtime_anim_prefixes();
    for key in [
        "GAME_CYCLE50",
        "GAME_DYNAMITELIT",
        "GAME_DYNAMITEHORZ",
        "GAME_DYNAMITEEXPLO",
        "LEVEL_RATBOMB_FALL",
        "LEVEL_RATBOMB_FALLEASTWEST",
        "LEVEL_RATBOMB_EXPLODE",
        "GAME_SMLHITBURST",
        "GAME_MEDHITBURST",
        "GAME_BIGHITBURST",
        "GAME_GLITTER1",
        "GAME_LIGHTFX_FLARE",
        "GAME_SPLASH",
    ] {
        assert!(prefixes.iter().any(|p| key.starts_with(p)), "{key}");
    }
    let _ = AnimFrame::default();
}
