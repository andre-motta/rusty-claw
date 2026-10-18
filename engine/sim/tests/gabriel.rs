//! Gabriel in a synthetic room: a dock floor with his deck above it at
//! the right, the boss fight flag set directly. How he is built and waits
//! for the fight, his start of fight, the idle and the attack choice, his
//! bombs, the pirate he calls in and its run and leap, the lever order,
//! his block, the cannon hits and his death acts to the gem, the respawn
//! reset, lifting him, and the part of a pixel he stands below his own.

use std::sync::Arc;

use claw_frame::{TICK_MS, TickInput};
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::gabriel::{self, bomb};
use claw_sim::logic::enemy::{self, EnemyState, redtail_pirate, wolvington};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{self, ObjectData, pickup, projectile};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::tiles::TileMap;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel};

/// Where he is placed: 3 px above where he stands on his deck, as on his
/// level.
const PLACED: (i32, i32) = (3223, 5068);
/// The top of his deck (a one-way floor), px: his level's.
const DECK_TOP: i32 = 5100;
/// The dock's floor top, px.
const DOCK_TOP: i32 = 5280;
/// Claw's y standing on the dock.
const CLAW_Y: i32 = DOCK_TOP - 61;
/// The gem's target in his record.
const GEM_TARGET: (i32, i32) = (3036, 5028);

fn anim(key: &str, frames: &[(i32, i32)], once: bool) -> Arc<Animation> {
    Arc::new(if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    })
}

/// His animations, with pictures numbered as his level's and lengths
/// chosen for the tests: his first pose 2000 ms, his second 1500 ms.
fn his_anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_GABRIEL_{s}");
    vec![
        anim(&k("IDLE1"), &[(100, 2000)], true),
        anim(&k("IDLE2"), &[(150, 1500)], true),
        anim(&k("STRIKE1"), &[(1, 100), (2, 100)], true),
        anim(&k("STRIKE7"), &[(400, 100), (401, 100), (402, 100)], true),
        anim(&k("BLOCKED"), &[(500, 300)], true),
        anim(&k("HITHIGH"), &[(600, 300)], true),
        anim(&k("KILLFALL1"), &[(900, 100), (901, 100)], false),
        anim(&k("KILLFALL2"), &[(903, 100)], false),
        anim(&k("KILLFALL3"), &[(907, 500)], true),
        anim(&k("KILLFALL4"), &[(909, 500)], true),
        anim(bomb::FALL_ANIM, &[(1, 100)], false),
        anim(bomb::EXPLODE_ANIM, &[(1, 100), (2, 100)], true),
        anim(redtail_pirate::CLASS.anims.fast_advance, &[(1, 100)], false),
    ]
}

/// The room, in 20 px tiles: the dock floor across at y 5280, his deck a
/// one-way floor at y 5100 from x 3180 eastwards, near his level's.
fn room() -> SimLevel {
    let (cols, rows) = (220, 270);
    let mut lines: Vec<String> = vec![".".repeat(cols); rows];
    lines[(DECK_TOP / 20) as usize].replace_range(159..cols, &"=".repeat(cols - 159));
    for line in lines.iter_mut().skip((DOCK_TOP / 20) as usize) {
        *line = "#".repeat(cols);
    }
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(20, &lines), (2800, CLAW_Y));
    level.number = 8;
    for a in his_anims() {
        level.animations.insert(a.key.clone(), a);
    }
    let mut o = ObjectSpec::at("Gabriel", PLACED.0, PLACED.1, 0);
    o.image_set = gabriel::IMAGE_SET.into();
    o.record[55] = GEM_TARGET.0;
    o.record[56] = GEM_TARGET.1;
    level.objects = vec![o];
    level
}

fn arena(seed: u32) -> Sim {
    let level = room();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    )
}

fn his_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Game(GameLogic::Gabriel))
        .map(|(id, _)| id)
        .expect("Gabriel exists")
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn state_of(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn his(sim: &Sim, id: ObjectId) -> gabriel::GabrielData {
    match &obj(sim, id).data {
        ObjectData::Gabriel(d) => d.clone(),
        _ => panic!("his own data"),
    }
}

fn link(sim: &mut Sim) -> gabriel::GabrielLink {
    sim.with_world(|w| w.gabriel_link().cloned())
        .expect("his link")
}

fn idle(sim: &mut Sim, ticks: usize) {
    for _ in 0..ticks {
        sim.step(TickInput::default());
    }
}

/// The room with Claw on the dock `dx` px from him, the fight flag as
/// given, after `ticks` ticks.
fn with_claw(seed: u32, dx: i32, flag: u8, ticks: usize) -> (Sim, ObjectId) {
    let mut sim = arena(seed);
    let id = his_id(&sim);
    sim.teleport_player(PLACED.0 + dx, CLAW_Y);
    sim.with_world(|w| {
        w.set_boss_fight(flag);
        w.claw.stats.health = 10_000;
    });
    idle(&mut sim, ticks);
    (sim, id)
}

/// He is in the fight, idle, with the cannon's reset acknowledged (no
/// cannon in this room).
fn in_fight() -> (Sim, ObjectId) {
    let (mut sim, id) = with_claw(3, -400, fight::ON, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    sim.with_world(|w| w.gabriel_link_mut().unwrap().wait_for_cannon = false);
    (sim, id)
}

/// A stream state whose next draw of 0..`n` is `want`; searched from 0.
fn stream_with(n: u32, want: u32) -> u32 {
    (0..1_000_000u32)
        .find(|&s| Rng::new(s).below(n) == want)
        .expect("a stream")
}

/// Runs his attack choice now, with the stream set to pick attack `k`.
fn choose(sim: &mut Sim, id: ObjectId, k: u32) {
    sim.set_rng_state(stream_with(3, k));
    sim.with_world(|w| gabriel::choose_attack(w, id));
}

/// A synthetic attacker of `type_flags`.
fn attacker(sim: &mut Sim, type_flags: u32) -> ObjectId {
    let mut a = Object::new(0, 0, 0, 0, "TestShot".into(), LogicKind::Inert);
    a.type_flags = type_flags;
    a.damage = 10;
    a.flags = flags::NO_COLLIDE | flags::UNLISTED;
    sim.spawn(a)
}

/// `attacker` hits him as a new attack of Claw.
fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId) {
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        let at = w.objects.get(id).map(|o| (o.x, o.y)).unwrap();
        if let Some(o) = w.objects.get_mut(attacker) {
            o.x = at.0;
            o.y = at.1;
            o.attack_rect = Rect::new(-5, -5, 5, 5);
        }
        logic::on_hit(w, id, attacker);
    });
}

fn of_logic(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic && o.flags & flags::DESTROY == 0)
        .map(|(id, _)| id)
        .collect()
}

/// Ticks until `f` holds, at most `limit`.
fn until(sim: &mut Sim, limit: usize, mut f: impl FnMut(&Sim) -> bool) -> usize {
    for k in 1..=limit {
        sim.step(TickInput::default());
        if f(sim) {
            return k;
        }
    }
    panic!("not within {limit} ticks");
}

// ----------------------------------------------------------------------
// Building and the start of the fight

#[test]
fn he_is_built_as_the_boss_asleep_on_his_deck() {
    let (sim, id) = with_claw(3, -400, fight::NONE, 2);
    let o = obj(&sim, id);
    assert_eq!(sim.boss().current(), Some(id), "the current boss");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
    assert_eq!(o.health, gabriel::HEALTH);
    assert_eq!(o.damage, gabriel::DAMAGE);
    assert_eq!(o.hit_type_flags, gabriel::HIT_BY);
    assert_eq!(o.move_rect, gabriel::BOX);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    assert!(o.special_flags & special::LIFTABLE != 0, "liftable");
    assert_eq!(o.physics_type, ptype::STAND);
    assert_eq!(o.y + gabriel::BOX.bottom + 1, DECK_TOP, "on his deck");
    let d = his(&sim, id);
    assert_eq!(d.enemy.ranges, [gabriel::RANGE; 5]);
    assert_eq!(d.enemy.smarts, 99);
    assert!(d.link.wait_for_cannon, "his reset asked for");
    assert_eq!(d.link.run_to, PLACED.0 - gabriel::PIRATE_RUN_TO);
    // Asleep for good while the flag stays 0.
    let mut sim = sim;
    idle(&mut sim, 500);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Startup));
}

#[test]
fn his_pose_is_drawn_when_he_is_built() {
    for (want, anim) in [(0, gabriel::IDLE1_ANIM), (75, gabriel::IDLE2_ANIM)] {
        // The level's set-up draws before the stream starts from the seed;
        // what counts is the draw at his building.
        let seed = (0..100_000u32)
            .find(|&s| Rng::new(s).below(100) == want)
            .unwrap();
        let mut level = room();
        level.objects.clear();
        let mut sim = Sim::new(
            level,
            SimConfig {
                seed,
                ..SimConfig::default()
            },
        );
        let id = sim.with_world(|w| {
            let o = logic::new_object("Gabriel", PLACED.0, PLACED.1, 0);
            let id = w.spawn(o);
            *w.rng = Rng::new(seed);
            gabriel::construct(w, id);
            id
        });
        let o = obj(&sim, id);
        assert_eq!(o.anim.key(), Some(anim), "{want}");
    }
}

#[test]
fn the_fight_flag_wakes_him() {
    let (mut sim, id) = with_claw(3, -400, fight::NONE, 2);
    sim.with_world(|w| {
        w.gabriel_link_mut().unwrap().pirates = 2;
        w.gabriel_link_mut().unwrap().button = true;
    });
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    sim.step(TickInput::default());
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    let d = his(&sim, id);
    assert_eq!(d.link.pirates, 0, "no pirates counted");
    assert!(
        !d.link.button && d.link.wait_for_cannon,
        "the cannon's reset asked for"
    );
    assert_eq!(d.cannon_hits, 0);
    assert_eq!(obj(&sim, id).health, gabriel::HEALTH);
}

#[test]
fn he_waits_for_his_pose_and_the_cannon_before_attacking() {
    let (mut sim, id) = with_claw(3, -400, fight::ON, 2);
    // The cannon has not acknowledged his reset: he idles on.
    idle(&mut sim, 200);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    sim.with_world(|w| w.gabriel_link_mut().unwrap().wait_for_cannon = false);
    let n = until(&mut sim, 200, |s| state_of(s, id) != Some(EnemyState::Idle));
    assert!(n <= 2, "his pose had played out: {n}");
}

// ----------------------------------------------------------------------
// The attacks

#[test]
fn the_bombs_three_of_four_lanes_to_his_left() {
    let (mut sim, id) = in_fight();
    choose(&mut sim, id, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::MultiBomb));
    assert!(!enemy::faces_right(obj(&sim, id)), "faces left");
    // The attack's 500 ms wait, then the throw; the bombs leave on its
    // second picture.
    // (the 500 ms wait is 28 ticks, his first picture 100 ms more).
    let n = until(&mut sim, 100, |s| !of_logic(s, bomb::NAMES[0]).is_empty());
    assert_eq!(n, 28 + 7);
    let bombs = of_logic(&sim, bomb::NAMES[0]);
    assert_eq!(bombs.len(), 3);
    let (x, y) = (obj(&sim, id).x, obj(&sim, id).y);
    for &b in &bombs {
        let o = obj(&sim, b);
        assert_eq!(
            (o.x, o.y),
            (x + gabriel::BOMB_SPAWN.0, y + gabriel::BOMB_SPAWN.1)
        );
        assert_eq!(o.type_flags, types::ENEMY_SHOT);
        assert!(o.special_flags & projectile::RESPAWN_PURGE != 0);
        assert!(o.special_flags & special::OWN_IMPACT != 0);
    }
    // They fly left and come down onto the dock, where each explodes.
    let n = until(&mut sim, 200, |s| {
        of_logic(s, bomb::NAMES[0])
            .iter()
            .all(|&b| obj(s, b).state == bomb::state::EXPLODING)
    });
    assert!(n > 10);
    for b in of_logic(&sim, bomb::NAMES[0]) {
        let o = obj(&sim, b);
        assert!(o.x < x, "to his left");
        assert_eq!(o.type_flags, types::LEVEL_SHOT, "the blast");
    }
    // He goes idle when his throw ends.
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
}

#[test]
fn a_bomb_that_strikes_claw_in_flight_explodes_there() {
    let (mut sim, id) = in_fight();
    let b = sim.with_world(|w| bomb::spawn(w, (3000, 600, 0), -100));
    sim.step(TickInput::default());
    let before = sim.rng().state();
    sim.with_world(|w| projectile::impact(w, b));
    let o = obj(&sim, b);
    assert_eq!(o.state, bomb::state::EXPLODING);
    assert_eq!(o.type_flags, types::LEVEL_SHOT);
    assert_ne!(sim.rng().state(), before, "the explosion's sound is drawn");
    // It stays where it went off.
    let at = (o.x, o.y);
    idle(&mut sim, 3);
    let o = obj(&sim, b);
    assert_eq!((o.x, o.y), at);
    let _ = id;
}

#[test]
fn the_pirate_he_calls_in() {
    let (mut sim, id) = in_fight();
    choose(&mut sim, id, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::ThugAttack));
    let n = until(&mut sim, 50, |s| !gabriel_pirates(s).is_empty());
    assert!(n <= 13, "when his call ends: {n}");
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    let p = gabriel_pirates(&sim)[0];
    let o = obj(&sim, p);
    let me = obj(&sim, id);
    assert_eq!(o.x, me.x + gabriel::PIRATE_SPAWN.0, "150 px right of him");
    assert!(o.special_flags & projectile::RESPAWN_PURGE != 0);
    assert_eq!(link(&mut sim).pirates, 1);
}

fn gabriel_pirates(sim: &Sim) -> Vec<ObjectId> {
    match &obj(sim, his_id(sim)).data {
        ObjectData::Gabriel(d) => d.summoned.clone(),
        _ => Vec::new(),
    }
}

#[test]
fn with_two_of_his_pirates_alive_he_only_idles_on() {
    let (mut sim, id) = in_fight();
    sim.with_world(|w| w.gabriel_link_mut().unwrap().pirates = gabriel::MAX_PIRATES);
    choose(&mut sim, id, 1);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    idle(&mut sim, 20);
    assert!(gabriel_pirates(&sim).is_empty());
}

#[test]
fn his_count_of_pirates_falls_when_one_dies() {
    let (mut sim, id) = in_fight();
    choose(&mut sim, id, 1);
    until(&mut sim, 50, |s| !gabriel_pirates(s).is_empty());
    let p = gabriel_pirates(&sim)[0];
    assert_eq!(link(&mut sim).pirates, 1);
    sim.with_world(|w| {
        w.objects.get_mut(p).unwrap().flags |= flags::DESTROY;
    });
    idle(&mut sim, 2);
    assert_eq!(link(&mut sim).pirates, 0);
    assert!(gabriel_pirates(&sim).is_empty());
}

#[test]
fn his_pirate_runs_along_the_deck_and_leaps_from_its_exact_place() {
    let (mut sim, id) = in_fight();
    // A pirate of his on the deck right of him, sent on his attack.
    let p = sim.with_world(|w| {
        let o = logic::new_object(redtail_pirate::NAMES[0], PLACED.0 + 150, PLACED.1, 0);
        let p = w.spawn(o);
        redtail_pirate::construct(w, p);
        gabriel::pirates::take_mode(w, p);
        gabriel::pirates::enter_attack(w, p);
        p
    });
    let o = obj(&sim, p);
    assert_eq!(state_of(&sim, p), Some(EnemyState::GabrielAttack));
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    let d = enemy::data_of(o).unwrap();
    assert_eq!(d.vx, -f64::from(redtail_pirate::GABRIEL_ATTACK_SPEED));
    assert!(d.busy);
    assert_eq!(d.ranges[0], redtail_pirate::GABRIEL_RANGES[0]);
    assert_eq!(d.ranges[1], redtail_pirate::GABRIEL_RANGES[1]);
    // Its run, driven by the boss fight's step: 8 or 9 px a tick. Each
    // call is a tick later than the one before, as in the game: a second
    // step in the same moment covers no time and moves nothing.
    let run_to = link(&mut sim).run_to;
    let mut xs = vec![obj(&sim, p).x];
    let mut ticks = 0u64;
    loop {
        ticks += 1;
        sim.with_world(|w| {
            w.now_ms += ticks * u64::from(TICK_MS);
            assert!(gabriel::pirates::mode_step(w, p));
        });
        xs.push(obj(&sim, p).x);
        if state_of(&sim, p) != Some(EnemyState::GabrielAttack) {
            break;
        }
        assert!(xs.len() < 100);
    }
    let run = &xs[..xs.len() - 1];
    assert!(
        run.windows(2).all(|w| (8..=9).contains(&(w[0] - w[1]))),
        "{xs:?}"
    );
    // At or left of his mark it leaps, up and to the left.
    assert_eq!(state_of(&sim, p), Some(EnemyState::Jump));
    assert!(run[run.len() - 1] <= run_to);
    assert!(run[run.len() - 2] > run_to);
    let d = enemy::data_of(obj(&sim, p)).unwrap();
    assert!(
        d.vx < 0.0 && d.vy < 0.0,
        "up and to the left: {:?}",
        (d.vx, d.vy)
    );
    // From its exact place: the part of a pixel it stands below its own.
    assert!(d.carry.1 > 0.0 && d.carry.1 < 1.0);
    let _ = id;
}

#[test]
fn once_he_is_defeated_his_pirates_die() {
    let (mut sim, id) = in_fight();
    let p = sim.with_world(|w| {
        let o = logic::new_object(redtail_pirate::NAMES[0], 3000, 850, 0);
        let p = w.spawn(o);
        redtail_pirate::construct(w, p);
        p
    });
    sim.with_world(|w| assert!(!gabriel::pirates::mode_step(w, p), "fights on"));
    sim.with_world(|w| w.gabriel_link_mut().unwrap().defeated = true);
    sim.with_world(|w| assert!(gabriel::pirates::mode_step(w, p), "the call ends"));
    assert!(enemy::data_of(obj(&sim, p)).unwrap().dead);
    let _ = id;
}

#[test]
fn a_pirate_running_his_attack_when_he_is_gone_dies() {
    let (mut sim, id) = in_fight();
    let p = sim.with_world(|w| {
        let o = logic::new_object(redtail_pirate::NAMES[0], PLACED.0 + 150, PLACED.1, 0);
        let p = w.spawn(o);
        redtail_pirate::construct(w, p);
        gabriel::pirates::take_mode(w, p);
        gabriel::pirates::enter_attack(w, p);
        p
    });
    sim.with_world(|w| assert!(gabriel::pirates::mode_step(w, p), "runs"));
    assert!(!enemy::data_of(obj(&sim, p)).unwrap().dead);
    // He is gone (drowned): no current boss any more.
    sim.with_world(|w| {
        w.destroy(id);
        w.set_current_boss(None);
    });
    sim.with_world(|w| assert!(gabriel::pirates::mode_step(w, p), "the call ends"));
    assert!(enemy::data_of(obj(&sim, p)).unwrap().dead);
}

#[test]
fn the_lever_order_comes_on_the_throws_second_picture() {
    let (mut sim, id) = in_fight();
    choose(&mut sim, id, 2);
    assert_eq!(state_of(&sim, id), Some(EnemyState::CannonAttack));
    assert!(enemy::faces_right(obj(&sim, id)), "faces right");
    let n = until(
        &mut sim,
        100,
        |s| matches!(&obj(s, his_id(s)).data, ObjectData::Gabriel(d) if d.link.fire_order),
    );
    assert_eq!(n, 28 + 7);
    assert_eq!(obj(&sim, id).frame, gabriel::STRIKE_FRAME);
    until(&mut sim, 100, |s| state_of(s, id) == Some(EnemyState::Idle));
    assert!(!enemy::faces_right(obj(&sim, id)), "faces left");
}

// ----------------------------------------------------------------------
// Hits

#[test]
fn blows_only_make_him_block() {
    let (mut sim, id) = in_fight();
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    hit(&mut sim, id, claw);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Blocked));
    assert_eq!(obj(&sim, id).health, gabriel::HEALTH, "no harm");
    // A pistol shot is destroyed, and he blocks.
    let (mut sim, id) = in_fight();
    let shot = attacker(&mut sim, types::PISTOL_SHOT);
    hit(&mut sim, id, shot);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Blocked));
    assert!(obj(&sim, shot).flags & flags::DESTROY != 0);
    // Blocking, another blow does nothing; he idles when it ends.
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    hit(&mut sim, id, claw);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Blocked));
    until(&mut sim, 100, |s| state_of(s, id) == Some(EnemyState::Idle));
}

#[test]
fn a_blow_wakes_him_from_his_start_up() {
    let (mut sim, id) = with_claw(3, -40, fight::NONE, 2);
    let claw = sim.with_world(|w| w.claw_id.expect("Claw"));
    hit(&mut sim, id, claw);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Blocked));
    until(&mut sim, 100, |s| state_of(s, id) == Some(EnemyState::Idle));
}

#[test]
fn a_cannonball_costs_him_twenty_and_he_answers_with_bombs() {
    let (mut sim, id) = in_fight();
    let ball = attacker(&mut sim, types::ENEMY_SHOT);
    hit(&mut sim, id, ball);
    assert_eq!(state_of(&sim, id), Some(EnemyState::CannonHit));
    assert!(
        obj(&sim, ball).flags & flags::DESTROY != 0,
        "the ball is gone"
    );
    assert_eq!(his(&sim, id).cannon_hits, 1);
    // The 500 ms wait and his flinch; then the cost and the bombs.
    let n = until(&mut sim, 100, |s| {
        state_of(s, id) != Some(EnemyState::CannonHit)
    });
    assert_eq!(state_of(&sim, id), Some(EnemyState::MultiBomb));
    assert_eq!(
        obj(&sim, id).health,
        gabriel::HEALTH - gabriel::CANNON_HIT_DAMAGE
    );
    assert_eq!(n, 28 + 18);
}

#[test]
fn the_fifth_cannonball_starts_his_death_acts_at_once() {
    let (mut sim, id) = in_fight();
    sim.with_world(|w| {
        if let ObjectData::Gabriel(d) = &mut w.objects.get_mut(id).unwrap().data {
            d.cannon_hits = 4;
        }
    });
    let ball = attacker(&mut sim, types::ENEMY_SHOT);
    hit(&mut sim, id, ball);
    let n = until(&mut sim, 100, |s| {
        state_of(s, id) != Some(EnemyState::CannonHit)
    });
    assert_eq!(n, 18, "no wait");
    assert_eq!(state_of(&sim, id), Some(EnemyState::DeadAct1));
    assert_eq!(obj(&sim, id).health, 0);
    let o = obj(&sim, id);
    assert!(o.flags & flags::HARMLESS != 0 && o.flags & flags::SAFE != 0);
    // In his death acts nothing reaches him.
    let ball = attacker(&mut sim, types::ENEMY_SHOT);
    hit(&mut sim, id, ball);
    assert_eq!(state_of(&sim, id), Some(EnemyState::DeadAct1));
}

#[test]
fn his_death_acts_end_in_the_gem() {
    let (mut sim, id) = in_fight();
    sim.with_world(|w| {
        if let ObjectData::Gabriel(d) = &mut w.objects.get_mut(id).unwrap().data {
            d.cannon_hits = 4;
        }
    });
    let ball = attacker(&mut sim, types::ENEMY_SHOT);
    hit(&mut sim, id, ball);
    until(&mut sim, 100, |s| {
        state_of(s, id) == Some(EnemyState::DeadAct1)
    });
    // 4000 ms after his line.
    let n = until(&mut sim, 400, |s| {
        state_of(s, id) == Some(EnemyState::DeadAct2)
    });
    assert!((222..=224).contains(&n), "{n}");
    // He faces right and leaps off his deck onto the dock.
    assert!(enemy::faces_right(obj(&sim, id)));
    until(&mut sim, 200, |s| {
        state_of(s, id) == Some(EnemyState::DeadAct3)
    });
    let o = obj(&sim, id);
    assert!(
        o.x < PLACED.0 && o.y > DECK_TOP,
        "on the dock: {:?}",
        (o.x, o.y)
    );
    until(&mut sim, 200, |s| {
        state_of(s, id) == Some(EnemyState::DeadAct4)
    });
    until(&mut sim, 200, |s| s.boss().defeated().is_some());
    assert_eq!(sim.boss().defeated(), Some(id));
    assert!(link(&mut sim).defeated);
    assert_eq!(of_logic(&sim, pickup::END_GEM).len(), 1, "the gem rises");
    // He stays, defeated.
    idle(&mut sim, 50);
    assert!(sim.objects().get(id).is_some());
}

#[test]
fn claws_respawn_starts_the_fight_over_only_while_the_flag_is_set() {
    // With the flag set: full health, no hits, idle again.
    let (mut sim, id) = in_fight();
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().health = 40;
        if let ObjectData::Gabriel(d) = &mut w.objects.get_mut(id).unwrap().data {
            d.cannon_hits = 3;
        }
    });
    sim.end_life();
    let n = until(&mut sim, 400, |s| {
        obj(s, his_id(s)).health == gabriel::HEALTH
    });
    assert!(n > 1);
    assert_eq!(his(&sim, id).cannon_hits, 0);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Idle));
    // With the flag at 0 (a skipped introduction) nothing of this.
    let (mut sim, id) = with_claw(3, -400, fight::NONE, 2);
    sim.with_world(|w| w.objects.get_mut(id).unwrap().health = 40);
    sim.end_life();
    idle(&mut sim, 200);
    assert_eq!(obj(&sim, id).health, 40);
}

// ----------------------------------------------------------------------
// Lifting him, and his place

#[test]
fn after_a_recovery_his_strike_never_ends() {
    let (mut sim, id) = in_fight();
    sim.with_world(|w| enemy::states::enter_strike(w, id));
    idle(&mut sim, 500);
    assert_eq!(state_of(&sim, id), Some(EnemyState::Strike));
}

#[test]
fn he_keeps_his_picture_while_carried() {
    let (mut sim, id) = in_fight();
    let frame = obj(&sim, id).frame;
    sim.with_world(|w| {
        w.objects.get_mut(id).unwrap().state = EnemyState::Lifted as i32;
        gabriel::lifted_tick(w, id);
    });
    assert_eq!(obj(&sim, id).frame, frame);
}

#[test]
fn standing_he_sinks_below_his_pixel_from_his_start_of_fight() {
    let (mut sim, id) = in_fight();
    // Set down at his start of fight; then a third of a tick's fall each
    // tick until the next pixel, as Wolvington.
    let mut seen = Vec::new();
    for _ in 0..8 {
        seen.push(his(&sim, id).stand_y);
        sim.step(TickInput::default());
    }
    let mut f = his(&sim, id).stand_y;
    for _ in 0..4 {
        let next = wolvington::next_stand_fraction(f, 2000.0);
        sim.step(TickInput::default());
        assert!((his(&sim, id).stand_y - next).abs() < 1e-9);
        f = next;
    }
    assert!(seen.iter().all(|&f| (0.0..1.0).contains(&f)), "{seen:?}");
    assert!(his(&sim, id).falls);
}
