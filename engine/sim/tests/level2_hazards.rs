//! The second level's hazards on synthetic levels built in code: the punk
//! rat and its ground cannon, the tower cannons and their cannonballs,
//! enemies dying in goo, the goo cover-ups and the cycling signs.
//!
//! The animations are built in code with the element lengths of the
//! game's own, so the timings below follow from the animation player's
//! rules at the 18 ms step.

mod common;

use std::sync::Arc;

use claw_sim::activation::ActivationConfig;
use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::claw_frame::AudioCue;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::enemy::{self, DeathKind, EnemyState, goo, punkrat};
use claw_sim::logic::{self, GameLogic, ObjectData, cannon, decor};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, draw_flags, flags, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};
use common::*;

// ----------------------------------------------------------------------
// Set-up

/// A (frame, ms) animation, finishing on its last element when `once`,
/// with event values at the given element indices.
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

/// The one-element "next image every 100 ms" loop.
fn cycle100() -> Arc<Animation> {
    Arc::new(Animation {
        key: decor::CYCLE_ANIM.into(),
        frames: vec![AnimFrame {
            image_op: 1,
            next_op: 6,
            duration: 100,
            ..AnimFrame::default()
        }],
        ..Animation::default()
    })
}

/// The hazards' animations, with the element lengths of the game's own.
fn add_animations(l: &mut SimLevel) {
    let hundreds = |n: i32| -> Vec<(i32, i32)> { (1..=n).map(|f| (f, 100)).collect() };
    let mut cannon_firing = hundreds(5);
    cannon_firing.push((1, 0));
    let mut tower_firing = hundreds(5);
    tower_firing.push((1, 25));
    let list = [
        anim(punkrat::STAND_ANIM, &[(1, 1000), (2, 1000)], false, &[]),
        anim(punkrat::WALK_ANIM, &hundreds(4), false, &[]),
        anim(
            punkrat::STRIKE_ANIM,
            &[(1, 1000), (2, 250), (3, 500)],
            true,
            &[],
        ),
        anim(
            punkrat::RECOIL_ANIM,
            &[(1, 250), (2, 250), (3, 250)],
            true,
            &[],
        ),
        anim("LEVEL_CANNON_FIRING", &cannon_firing, true, &[(0, 1)]),
        anim("LEVEL_CANNON_HOME", &[(1, 0)], true, &[]),
        anim(
            "LEVEL_TOWERCANNONLEFT_FIRING",
            &tower_firing,
            true,
            &[(1, 1)],
        ),
        anim("LEVEL_TOWERCANNONLEFT_HOME", &[(1, 0)], true, &[]),
        anim(
            "LEVEL_TOWERCANNONRIGHT_FIRING",
            &tower_firing,
            true,
            &[(1, 1)],
        ),
        anim("LEVEL_TOWERCANNONRIGHT_HOME", &[(1, 0)], true, &[]),
        anim("LEVEL_CANNONBALL_FALL", &hundreds(4), false, &[]),
        cycle100(),
    ];
    for a in list {
        l.animations.insert(a.key.clone(), a);
    }
}

/// The flat level (floor top at y = 960, Claw starting at x = 1000) with
/// the hazards' animations and `objects`.
fn level_with(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut l = flat_level();
    add_animations(&mut l);
    l.objects = objects;
    l
}

/// Height at which a punk rat stands on the flat level's floor.
const RAT_Y: i32 = 919;
/// Height at which Claw stands on the flat level's floor.
const CLAW_Y: i32 = 899;

/// A punk rat record at (x, RAT_Y), mirrored when asked.
fn rat(x: i32, mirrored: bool) -> ObjectSpec {
    let mut r = ObjectSpec::at("PunkRat", x, RAT_Y, 0);
    r.image_set = punkrat::IMAGE_SET.into();
    if mirrored {
        r.record[11] = draw_flags::MIRROR as i32;
    }
    r
}

fn tower(name: &str, x: i32, y: i32) -> ObjectSpec {
    let mut r = ObjectSpec::at(name, x, y, 0);
    r.image_set = format!("LEVEL_{}", name.to_uppercase());
    r
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn all_of(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect();
    v.sort_unstable();
    v
}

fn one(sim: &Sim, logic: &str) -> ObjectId {
    let v = all_of(sim, logic);
    assert_eq!(v.len(), 1, "one {logic}");
    v[0]
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    sim.objects()
        .get(id)
        .and_then(|o| EnemyState::from_id(o.state))
}

fn cannon_data(sim: &Sim, id: ObjectId) -> cannon::CannonData {
    match &obj(sim, id).data {
        ObjectData::Cannon(d) => d.clone(),
        other => panic!("not a cannon: {other:?}"),
    }
}

fn claw_id(sim: &mut Sim) -> ObjectId {
    sim.with_world(|w| w.claw_id.expect("Claw"))
}

fn health(sim: &Sim) -> i32 {
    sim.claw().stats.health
}

fn cues(sim: &Sim) -> Vec<String> {
    sim.last_audio()
        .iter()
        .filter_map(|c| match c {
            AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

fn step(sim: &mut Sim) {
    sim.step(TickInput::default());
}

/// Steps until `f` holds, at most `limit` ticks; the tick it first held
/// on.
fn step_until(sim: &mut Sim, limit: u64, mut f: impl FnMut(&Sim) -> bool) -> Option<u64> {
    for _ in 0..limit {
        step(sim);
        if f(sim) {
            return Some(sim.tick());
        }
    }
    None
}

/// Keeps Claw out of harm's way: nothing hurts him.
fn shield_claw(sim: &mut Sim) {
    let id = claw_id(sim);
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().hit_type_flags = 0;
    });
}

// ----------------------------------------------------------------------
// The punk rat

#[test]
fn a_punk_rat_is_built_with_its_cannon_set_down() {
    let mut sim = Sim::new(level_with(vec![rat(1300, false)]), SimConfig::default());
    // Woken with the level, it was built in the level's set-up (its tick
    // 0), and its cannon with it.
    let id = one(&sim, "PunkRat");
    let o = obj(&sim, id);
    assert_eq!(o.kind, LogicKind::Game(GameLogic::PunkRat));
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveRight));
    assert_eq!(o.z, punkrat::Z);
    assert_eq!(o.physics_type, ptype::WALK);
    assert_eq!(o.hit_rect, punkrat::HIT_RECT);
    assert_eq!(o.move_rect, punkrat::MOVE_RECT);
    assert!(!o.attack_rect.is_set(), "its touch hurts nobody");
    assert_eq!((o.type_flags, o.hit_type_flags), (types::ENEMY, 0x300_0032));
    assert_eq!(o.health, 2);
    assert_eq!(o.speed_x, 500);
    assert_eq!(
        o.anim.key(),
        Some(punkrat::STAND_ANIM),
        "stands on its first walk"
    );
    let d = enemy::data_of(o).unwrap();
    assert_eq!(d.bounds, (1270, 1315));
    assert_eq!((d.vx, d.ay), (60.0, 0.0));
    assert!(!d.ledge_guard && !d.z_sort && !d.may_lift);
    let c = one(&sim, cannon::GROUND_CANNON);
    assert_eq!(punkrat_cannon(&mut sim, id), Some(c));
    let co = obj(&sim, c);
    assert_eq!((co.x, co.y, co.z), (1300, RAT_Y + 15, punkrat::Z - 1));
    assert_eq!(co.state, cannon::CONSTRUCTED, "waits for its rat");
    assert_eq!(co.speed_x, 500);
    assert_eq!(co.draw_flags & draw_flags::MIRROR, 0);
    assert!(co.flags & flags::NO_COLLIDE != 0);
    assert_eq!(cannon_data(&sim, c).muzzle, (50, 10));
    // The cannon runs before its rat in every pass.
    let listed = sim.objects().listed();
    let at = |x| listed.iter().position(|&i| i == x).unwrap();
    assert!(at(c) < at(id));
    // It never fires on its own.
    idle(&mut sim, 50);
    assert_eq!(cannon_data(&sim, c).fired, 0);
}

fn punkrat_cannon(sim: &mut Sim, id: ObjectId) -> Option<ObjectId> {
    sim.with_world(|w| punkrat::cannon_of(w, id))
}

/// AC3: an unmirrored punk rat turns at the right end of its beat on its
/// 15th tick, strikes at the left end on its 57th, and its cannon's first
/// ball leaves on its 157th tick, then one every 226 ticks; a ball that
/// reaches Claw costs him 10 health.
#[test]
fn a_punk_rat_strikes_and_its_ball_hurts_claw() {
    let mut sim = Sim::new(level_with(vec![rat(1300, false)]), SimConfig::default());
    sim.teleport_player(1700, CLAW_Y);
    let id = one(&sim, "PunkRat");
    let c = one(&sim, cannon::GROUND_CANNON);
    let before = health(&sim);
    idle(&mut sim, 13);
    assert_eq!(obj(&sim, id).x, 1314);
    step(&mut sim);
    assert_eq!(obj(&sim, id).x, 1315, "on the right end of its beat");
    assert_eq!(state_of(&sim, id), Some(EnemyState::MoveRight));
    let turned = step_until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::MoveLeft)
    });
    assert_eq!(turned, Some(15));
    assert_eq!(obj(&sim, id).x, 1313, "and walks back in the same tick");
    assert_eq!(obj(&sim, id).anim.key(), Some(punkrat::WALK_ANIM));
    let struck = step_until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::Strike)
    });
    assert_eq!(struck, Some(57));
    assert_eq!(obj(&sim, id).x, 1270, "at the left end of its beat");
    let recoil = step_until(&mut sim, 200, |s| {
        state_of(s, id) == Some(EnemyState::Recoil)
    });
    assert_eq!(recoil, Some(156), "the strike lasts 99 ticks");
    assert_eq!(state_of(&sim, c), Some(EnemyState::Fire), "told to fire");
    assert_eq!(cannon_data(&sim, c).fired, 0);
    step(&mut sim);
    assert_eq!(sim.tick(), 157);
    let cd = cannon_data(&sim, c);
    assert_eq!(cd.fired, 1, "the ball leaves on the cannon's next call");
    let ball = cd.last_ball.unwrap();
    let b = obj(&sim, ball);
    assert_eq!((b.x, b.y, b.z), (1350, RAT_Y + 25, punkrat::Z - 2));
    assert_eq!(b.type_flags, types::ENEMY_SHOT);
    assert_eq!(b.damage, 10);
    assert_eq!(b.draw_flags & draw_flags::MIRROR, 0);
    // The ball flies 9 px a tick and costs Claw 10 health.
    assert_eq!(health(&sim), before);
    let hit = step_until(&mut sim, 100, |s| health(s) != before).expect("the ball hits Claw");
    assert_eq!(health(&sim), before - 10);
    assert!(
        sim.objects()
            .get(ball)
            .is_none_or(|o| o.flags & flags::DESTROY != 0),
        "the ball is gone"
    );
    assert!(hit > 157 + 30, "{hit}");
    let walked = step_until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::MoveRight)
    });
    assert_eq!(walked, Some(199), "the recoil lasts 43 ticks");
    // The next ball, one cycle later.
    let next = step_until(&mut sim, 300, |s| cannon_data(s, c).fired == 2);
    assert_eq!(next, Some(157 + 226));
}

#[test]
fn a_mirrored_punk_rat_strikes_at_its_right_end_and_fires_left() {
    let mut sim = Sim::new(level_with(vec![rat(1700, true)]), SimConfig::default());
    sim.teleport_player(1100, CLAW_Y);
    let id = one(&sim, "PunkRat");
    let c = one(&sim, cannon::GROUND_CANNON);
    let co = obj(&sim, c);
    assert_eq!((co.x, co.y), (1680, RAT_Y + 15));
    assert_ne!(co.draw_flags & draw_flags::MIRROR, 0);
    assert_eq!(co.speed_x, -500);
    assert_eq!(cannon_data(&sim, c).muzzle, (-50, 10));
    let struck = step_until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::Strike)
    });
    assert_eq!(struck, Some(15));
    let fired = step_until(&mut sim, 200, |s| cannon_data(s, c).fired == 1);
    assert_eq!(fired, Some(115));
    let ball = cannon_data(&sim, c).last_ball.unwrap();
    let b = obj(&sim, ball);
    assert_eq!((b.x, b.y), (1630, RAT_Y + 25));
    assert_ne!(b.draw_flags & draw_flags::MIRROR, 0);
    assert_eq!(b.attack_rect, Rect::new(-22, -10, 9, 9));
    step(&mut sim);
    assert_eq!(
        obj(&sim, ball).x,
        1630 - 9,
        "9 px a tick from the tick after"
    );
    // It recoils, then walks left first.
    let walked = step_until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::MoveLeft)
    });
    assert_eq!(walked, Some(15 + 99 + 43));
    let before = health(&sim);
    step_until(&mut sim, 100, |s| health(s) != before).expect("the ball hits Claw");
    assert_eq!(health(&sim), before - 10);
}

#[test]
fn an_invisible_claw_stops_the_punk_rat_striking() {
    let mut sim = Sim::new(level_with(vec![rat(1300, false)]), SimConfig::default());
    sim.with_world(|w| {
        w.claw.stats.powerup_kind = enemy::INVISIBILITY_POWERUP;
        w.claw.stats.powerup_ms = 1_000_000;
    });
    let id = one(&sim, "PunkRat");
    let mut ends = 0;
    let mut last = state_of(&sim, id);
    for _ in 0..400 {
        step(&mut sim);
        let s = state_of(&sim, id);
        assert_ne!(s, Some(EnemyState::Strike));
        if s != last {
            ends += 1;
        }
        last = s;
    }
    assert!(ends >= 8, "it paces its beat ({ends} turns)");
    let x = obj(&sim, id).x;
    assert!((1270..=1315).contains(&x));
}

#[test]
fn any_hit_kills_a_punk_rat_and_its_cannon_stays_silent() {
    let mut sim = Sim::new(level_with(vec![rat(1300, false)]), SimConfig::default());
    sim.teleport_player(1250, CLAW_Y);
    shield_claw(&mut sim);
    idle(&mut sim, 5);
    let id = one(&sim, "PunkRat");
    let c = one(&sim, cannon::GROUND_CANNON);
    // A pistol shot kills it and flies on.
    let shot = sim.spawn(logic::new_object("PistolBullet", 1300, 900, 0));
    sim.with_world(|w| {
        let o = w.objects.get_mut(shot).unwrap();
        o.type_flags = types::PISTOL_SHOT;
        o.damage = 1;
    });
    let claw = claw_id(&mut sim);
    sim.with_world(|w| logic::on_hit(w, id, shot));
    assert_eq!(state_of(&sim, id), Some(EnemyState::Dead));
    assert_eq!(
        obj(&sim, shot).flags & flags::DESTROY,
        0,
        "the shot flies on"
    );
    let d = enemy::data_of(obj(&sim, id)).unwrap().clone();
    assert_eq!(d.death, DeathKind::Hit);
    assert_eq!((d.vx, d.vy), (150.0, -700.0), "thrown away from Claw");
    assert_eq!(obj(&sim, id).z, enemy::DEATH_DEPTH);
    assert!(
        !sim.objects()
            .all()
            .any(|(_, o)| matches!(o.data, ObjectData::Pickup(_))),
        "no loot"
    );
    // A body can be hit again, with no check of Claw's attack number.
    let rng = sim.rng_state();
    sim.with_world(|w| logic::on_hit(w, id, claw));
    assert_ne!(sim.rng_state(), rng, "the death's draws again");
    assert_eq!(enemy::data_of(obj(&sim, id)).unwrap().vy, -700.0);
    // The body falls away and is removed with a splash; the cannon stays,
    // unfired.
    let gone = step_until(&mut sim, 200, |s| s.objects().get(id).is_none());
    assert!(gone.is_some());
    idle(&mut sim, 400);
    let cd = cannon_data(&sim, c);
    assert_eq!(cd.fired, 0);
    assert_eq!(obj(&sim, c).state, cannon::CONSTRUCTED);
}

/// The random draws of a punk rat under a scripted stream: on each frame
/// pulse tick its roll of 0..=1000, then, while walking, one of 0..100;
/// none while striking or recoiling, none by its cannon or its balls; a
/// hit draws the death's.
#[test]
fn a_punk_rats_draws_follow_the_stream() {
    let mut sim = Sim::new(
        level_with(vec![rat(1300, false)]),
        SimConfig {
            seed: 77,
            ..SimConfig::default()
        },
    );
    // Claw stays behind the rat, out of its lane.
    let id = one(&sim, "PunkRat");
    let mut mirror: Rng = sim.rng().clone();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..500 {
        let s = state_of(&sim, id).unwrap();
        seen.insert(s);
        step(&mut sim);
        if sim.pulse() {
            mirror.below(1001);
            if matches!(s, EnemyState::MoveLeft | EnemyState::MoveRight) {
                mirror.below(100);
            }
        }
        assert_eq!(sim.rng().state(), mirror.state(), "tick {}", sim.tick());
    }
    for s in [
        EnemyState::MoveLeft,
        EnemyState::MoveRight,
        EnemyState::Strike,
        EnemyState::Recoil,
    ] {
        assert!(seen.contains(&s), "{s:?}");
    }
    assert!(cannon_data(&sim, one(&sim, cannon::GROUND_CANNON)).fired >= 2);
    // Claw's blow kills it: one of 0..100, then one of 0..10 when he
    // speaks (a quarter of the time).
    let claw = claw_id(&mut sim);
    sim.with_world(|w| logic::on_hit(w, id, claw));
    if mirror.below(100) < 25 {
        mirror.below(10);
    }
    assert_eq!(sim.rng().state(), mirror.state());
    // The body still rolls on frame pulse ticks until it is gone.
    for _ in 0..30 {
        let alive = sim.objects().get(id).is_some();
        step(&mut sim);
        if alive && sim.pulse() {
            mirror.below(1001);
        }
        assert_eq!(sim.rng().state(), mirror.state());
    }
}

// ----------------------------------------------------------------------
// Tower cannons and cannonballs

/// Ball spawns of cannon `c` over `n` ticks: (tick, x, y, vx).
fn shots_of(sim: &mut Sim, cannons: &[ObjectId], n: usize) -> Vec<Vec<(u64, i32, i32, i32)>> {
    let mut out = vec![Vec::new(); cannons.len()];
    let mut fired: Vec<u32> = cannons.iter().map(|&c| cannon_data(sim, c).fired).collect();
    for _ in 0..n {
        step(sim);
        for (i, &c) in cannons.iter().enumerate() {
            let d = cannon_data(sim, c);
            if d.fired != fired[i] {
                fired[i] = d.fired;
                let b = obj(sim, d.last_ball.unwrap());
                out[i].push((sim.tick(), b.x, b.y, b.speed_x));
            }
        }
    }
    out
}

/// AC4: each tower cannon fires from the tick after it is built, then
/// every 167 ticks (3006 ms), its ball leaving seven ticks after the
/// order, from its muzzle (50 px to its side, 10 px down) towards its
/// side at 500 px/s; a ball that reaches Claw costs him 10 health.
#[test]
fn tower_cannons_fire_on_their_period_from_their_muzzles() {
    let mut sim = Sim::new(
        level_with(vec![
            tower("TowerCannonLeft", 1500, 890),
            tower("TowerCannonRight", 500, 890),
        ]),
        SimConfig::default(),
    );
    let left = one(&sim, "TowerCannonLeft");
    let right = one(&sim, "TowerCannonRight");
    for (c, speed, muzzle) in [(left, -500, (-50, 10)), (right, 500, (50, 10))] {
        let o = obj(&sim, c);
        assert_eq!(state_of(&sim, c), Some(EnemyState::Home), "built at set-up");
        assert_eq!((o.speed_x, o.z), (speed, cannon::DEFAULT_Z));
        assert!(o.flags & flags::NO_COLLIDE != 0);
        assert_eq!(cannon_data(&sim, c).muzzle, muzzle);
    }
    let start = health(&sim);
    let shots = shots_of(&mut sim, &[left, right], 450);
    for (i, (x, vx)) in [(1450, -500), (550, 500)].into_iter().enumerate() {
        let ticks: Vec<u64> = shots[i].iter().map(|s| s.0).collect();
        assert_eq!(ticks, vec![8, 8 + 167, 8 + 334], "cannon {i}");
        for s in &shots[i] {
            assert_eq!((s.1, s.2, s.3), (x, 900, vx), "cannon {i}");
        }
    }
    // Six balls reached Claw, 10 health each.
    assert_eq!(health(&sim), start - 60);
}

#[test]
fn a_cannonball_flies_straight_and_goes_at_a_wall() {
    let mut l = level_with(vec![tower("TowerCannonRight", 700, 890)]);
    // A wall at x = 1280 across the ball's height, left of Claw.
    l.map.set_cell(20, 14, 0);
    let mut sim = Sim::new(l, SimConfig::default());
    sim.teleport_player(1400, CLAW_Y);
    let c = one(&sim, "TowerCannonRight");
    let start = health(&sim);
    step_until(&mut sim, 20, |s| cannon_data(s, c).fired == 1).unwrap();
    let ball = cannon_data(&sim, c).last_ball.unwrap();
    let b = obj(&sim, ball);
    assert_eq!(b.kind, LogicKind::Game(GameLogic::Projectile));
    assert_eq!(b.anim.key(), Some("LEVEL_CANNONBALL_FALL"));
    assert_eq!(b.hit_type_flags, 0x2);
    assert!(!b.hit_rect.is_set(), "nothing can hit it");
    assert_eq!(b.attack_rect, Rect::new(-9, -10, 22, 9));
    let mut xs = vec![b.x];
    while sim.objects().get(ball).is_some() {
        step(&mut sim);
        if let Some(o) = sim.objects().get(ball) {
            xs.push(o.x);
            assert_eq!(o.y, 900, "no gravity");
        }
        assert!(xs.len() < 200);
    }
    assert_eq!(xs[0], 750);
    let (flying, last) = xs.split_at(xs.len() - 1);
    assert!(flying.windows(2).all(|p| p[1] - p[0] == 9), "{xs:?}");
    // The wall stops it short; it spends that tick there and is gone.
    assert_eq!(last[0], 1254, "{xs:?}");
    assert!(last[0] + 22 < 1280);
    assert_eq!(health(&sim), start, "Claw behind the wall is safe");
}

#[test]
fn cannonballs_pass_enemies_and_kegs() {
    // A tower cannon fires right along the floor at a punk rat and a keg.
    let mut keg = ObjectSpec::at("PowderKeg", 600, 926, 0);
    keg.image_set = "LEVEL_POWDERKEG".into();
    let mut sim = Sim::new(
        level_with(vec![
            tower("TowerCannonRight", 300, RAT_Y + 15),
            rat(500, false),
            keg,
        ]),
        SimConfig::default(),
    );
    let c = one(&sim, "TowerCannonRight");
    let r = one(&sim, "PunkRat");
    let k = one(&sim, "PowderKeg");
    step_until(&mut sim, 20, |s| cannon_data(s, c).fired == 1).unwrap();
    let ball = cannon_data(&sim, c).last_ball.unwrap();
    let mut passed = false;
    for _ in 0..120 {
        step(&mut sim);
        match sim.objects().get(ball) {
            Some(b) if b.x > 700 => {
                passed = true;
                break;
            }
            Some(_) => {}
            None => break,
        }
    }
    assert!(passed, "the ball flew past the rat and the keg");
    assert_ne!(state_of(&sim, r), Some(EnemyState::Dead));
    assert!(sim.objects().get(k).is_some());
}

/// Timer cannons need no Claw: a tower cannon awake in the widened
/// windows of a wide presentation fires although Claw is far away, and
/// stays dormant (built later) in the original windows.
#[test]
fn widened_activation_wakes_a_distant_tower_cannon() {
    let objects = vec![tower("TowerCannonRight", 2250, 890)];
    let mut widened = Sim::new(
        level_with(objects.clone()),
        SimConfig {
            activation: ActivationConfig::widened((1920, 1080)),
            ..SimConfig::default()
        },
    );
    let c = one(&widened, "TowerCannonRight");
    let fired = step_until(
        &mut widened,
        20,
        |s| matches!(&obj(s, c).data, ObjectData::Cannon(d) if d.fired == 1),
    );
    assert_eq!(fired, Some(8), "fires 1250 px from Claw");
    let mut original = Sim::new(level_with(objects), SimConfig::default());
    let c = one(&original, "TowerCannonRight");
    idle(&mut original, 200);
    assert_eq!(obj(&original, c).state, 0, "dormant, never built");
}

#[test]
fn a_mirrored_tower_record_keeps_the_original_muzzle_quirk() {
    let mut left = tower("TowerCannonLeft", 1500, 890);
    left.record[11] = draw_flags::MIRROR as i32;
    let mut right = tower("TowerCannonRight", 500, 890);
    right.record[11] = draw_flags::MIRROR as i32;
    let sim = Sim::new(level_with(vec![left, right]), SimConfig::default());
    let l = one(&sim, "TowerCannonLeft");
    let r = one(&sim, "TowerCannonRight");
    // The left one fires right from its left side, the right one left
    // from its right side.
    assert_eq!(obj(&sim, l).speed_x, 500);
    assert_eq!(cannon_data(&sim, l).muzzle, (-50, 10));
    assert_eq!(obj(&sim, r).speed_x, -500);
    assert_eq!(cannon_data(&sim, r).muzzle, (50, 10));
}

// ----------------------------------------------------------------------
// Goo

/// A level `number` whose floor row is death tiles under x 2560..3839,
/// with `logic` placed above them and Claw on a ledge out of the way.
fn pool(number: u32, logic: &str) -> (Sim, ObjectId) {
    let mut rows = vec![".".repeat(200); 15];
    let mut death = "#".repeat(200);
    death.replace_range(40..60, &"x".repeat(20));
    rows.push(death);
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    l.number = number;
    l.map.set_cell(15, 12, 0);
    add_animations(&mut l);
    let mut r = ObjectSpec::at(logic, 3000, 600, 0);
    r.record[15] = 33;
    l.objects = vec![r];
    let mut sim = Sim::new(
        l,
        SimConfig {
            seed: 5,
            activation: ActivationConfig::widened((12800, 1088)),
            ..SimConfig::default()
        },
    );
    idle(&mut sim, 30);
    let id = one(&sim, logic);
    (sim, id)
}

/// Drops `id` 100 px so it falls back onto the death tiles; the tick it
/// died in.
fn drop_in(sim: &mut Sim, id: ObjectId) -> (Option<u64>, Vec<String>) {
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.y -= 100;
        o.physics_type = ptype::FALL;
    });
    let mut heard = Vec::new();
    for _ in 0..60 {
        step(sim);
        heard.extend(cues(sim));
        let dead = sim
            .objects()
            .get(id)
            .is_none_or(|o| EnemyState::from_id(o.state) == Some(EnemyState::Dead));
        if dead {
            return (Some(sim.tick()), heard);
        }
    }
    (None, heard)
}

#[test]
fn an_officer_in_goo_drops_its_loot_and_is_gone_in_the_same_step() {
    let (mut sim, id) = pool(2, "Officer");
    let (died, heard) = drop_in(&mut sim, id);
    assert!(died.is_some());
    assert!(
        sim.objects().get(id).is_none(),
        "removed in the step it died"
    );
    assert!(heard.iter().any(|k| k == enemy::DEATH_TILE_SOUND));
    let loot = sim
        .objects()
        .all()
        .filter(|(_, o)| matches!(o.data, ObjectData::Pickup(_)))
        .count();
    assert_eq!(loot, 1, "its loot first");
    assert!(
        !sim.snapshot()
            .renderables
            .iter()
            .any(|r| r.image_set_key == "LEVEL_OFFICER"),
        "never drawn in the goo"
    );
}

#[test]
fn rats_sink_in_goo_without_a_sound() {
    let (mut sim, id) = pool(2, "Rat");
    let (died, heard) = drop_in(&mut sim, id);
    assert!(died.is_some());
    assert!(sim.objects().get(id).is_none());
    assert!(!heard.iter().any(|k| k == enemy::DEATH_TILE_SOUND));
    // A punk rat in goo goes the same way, with no draw.
    let mut l = level_with(vec![rat(1300, false)]);
    l.number = 4;
    let mut sim = Sim::new(l, SimConfig::default());
    let r = one(&sim, "PunkRat");
    let rng = sim.rng_state();
    sim.with_world(|w| {
        assert_eq!(goo::death_tile_kind(w), DeathKind::Goo);
        goo::enter_goo(w, r);
    });
    assert!(obj(&sim, r).flags & flags::DESTROY != 0);
    assert_eq!(sim.rng_state(), rng, "no draw");
    step(&mut sim);
    assert!(sim.objects().get(r).is_none());
    assert!(!cues(&sim).iter().any(|k| k == enemy::DEATH_TILE_SOUND));
}

#[test]
fn on_spike_levels_bodies_stay_a_second_and_rats_die_silently() {
    let (mut sim, id) = pool(1, "Officer");
    let (died, heard) = drop_in(&mut sim, id);
    assert!(died.is_some());
    let o = obj(&sim, id);
    assert_ne!(o.draw_flags & draw_flags::FLASH, 0);
    assert_eq!(enemy::data_of(o).unwrap().death, DeathKind::Spikes);
    assert!(heard.iter().any(|k| k == enemy::DEATH_TILE_SOUND));
    let (mut sim, id) = pool(1, "Rat");
    let (died, heard) = drop_in(&mut sim, id);
    assert!(died.is_some());
    let o = obj(&sim, id);
    assert_ne!(o.draw_flags & draw_flags::FLASH, 0, "stays, flashing");
    assert!(!heard.iter().any(|k| k == enemy::DEATH_TILE_SOUND));
    idle(&mut sim, 60);
    assert!(sim.objects().get(id).is_none(), "gone after a second");
}

// ----------------------------------------------------------------------
// Goo cover-ups and cycling signs

#[test]
fn goo_cover_ups_sit_in_front_and_never_change() {
    let mut deep = ObjectSpec::at("GooCoverup", 1200, 950, 0);
    deep.image_set = "LEVEL_GOOCOVERUP".into();
    deep.frame = 1;
    let mut placed = ObjectSpec::at("GooCoverup", 1264, 950, 5010);
    placed.image_set = "LEVEL_GOOCENTER".into();
    placed.frame = 1;
    let mut sim = Sim::new(level_with(vec![deep, placed]), SimConfig::default());
    let ids = all_of(&sim, "GooCoverup");
    let zs: Vec<i32> = ids.iter().map(|&i| obj(&sim, i).z).collect();
    assert_eq!(zs, vec![decor::GOO_COVERUP_Z, 5010]);
    idle(&mut sim, 50);
    for &i in &ids {
        let o = obj(&sim, i);
        assert_eq!(o.kind, LogicKind::Game(GameLogic::Decor));
        assert!(o.flags & flags::NO_COLLIDE != 0);
        assert_eq!(o.frame, 1);
        assert!(o.anim.animation().is_none());
    }
}

#[test]
fn a_cycling_sign_shows_each_image_for_six_ticks_at_its_own_depth() {
    let mut sign = ObjectSpec::at("AniCycle", 1200, 900, 0);
    sign.image_set = "LEVEL_ARROWSIGN_RIGHT".into();
    sign.frame = 1;
    let mut l = level_with(vec![sign]);
    l.images
        .insert("LEVEL_ARROWSIGN_RIGHT".into(), ImageFrames::range(1, 3));
    let mut sim = Sim::new(l, SimConfig::default());
    let id = one(&sim, "AniCycle");
    // Its first call sets it up without advancing.
    step_until(&mut sim, 10, |s| obj(s, id).state == decor::SET_UP).unwrap();
    assert_eq!(obj(&sim, id).frame, 1);
    assert_eq!(obj(&sim, id).z, 0, "keeps depth 0");
    let mut seen = Vec::new();
    for _ in 0..19 {
        step(&mut sim);
        seen.push(obj(&sim, id).frame);
    }
    let mut want = vec![2; 6];
    want.extend([3; 6]);
    want.extend([1; 6]);
    want.push(2);
    assert_eq!(seen, want);
    assert!(obj(&sim, id).flags & flags::NO_COLLIDE != 0);
}

#[test]
fn the_new_logics_are_no_longer_inert() {
    let mut cover = ObjectSpec::at("GooCoverup", 1200, 950, 5010);
    cover.image_set = "LEVEL_GOOCENTER".into();
    let mut sign = ObjectSpec::at("AniCycle", 1300, 900, 0);
    sign.image_set = "LEVEL_ARROWSIGN_RIGHT".into();
    let objects = vec![
        rat(1300, false),
        tower("TowerCannonLeft", 1500, 890),
        tower("TowerCannonRight", 500, 890),
        cover,
        sign,
        ObjectSpec::at("AniCycleNormal", 1400, 900, 0),
    ];
    let names: Vec<String> = objects.iter().map(|o| o.logic.clone()).collect();
    let mut sim = Sim::new(level_with(objects), SimConfig::default());
    idle(&mut sim, 20);
    let d = sim.diagnostics();
    for n in names
        .iter()
        .map(String::as_str)
        .chain(["GroundCannon", "CannonBall"])
    {
        assert!(!d.inert_logics.contains_key(n), "{n} is inert");
        assert!(logic::runs(n).is_some(), "{n} runs");
    }
    // Every image set and animation they use is loaded with the level.
    let sets = sim.level().image_sets();
    for s in ["LEVEL_PUNKRAT", "LEVEL_CANNON", "LEVEL_CANNONBALL"] {
        assert!(sets.iter().any(|x| x == s), "{s}");
    }
    let prefixes = logic::runtime_anim_prefixes();
    for key in [
        punkrat::WALK_ANIM,
        punkrat::STAND_ANIM,
        punkrat::STRIKE_ANIM,
        punkrat::RECOIL_ANIM,
        "LEVEL_CANNON_FIRING",
        "LEVEL_CANNON_HOME",
        "LEVEL_TOWERCANNONLEFT_FIRING",
        "LEVEL_TOWERCANNONRIGHT_HOME",
        "LEVEL_CANNONBALL_FALL",
    ] {
        assert!(prefixes.iter().any(|p| key.starts_with(p)), "{key}");
    }
}
