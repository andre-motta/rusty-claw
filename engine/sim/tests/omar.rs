//! Omar, his ring and his energy balls on a synthetic arena with the
//! fourteenth level's animation timings: a floor whose top is at y 2560
//! under his chamber's places.
//!
//! How he is built (the current boss, his boxes, his ring of nine pieces
//! of fire, waiting for the fight, idle on his first tick when Claw is out
//! of his reach); his ring on the level clock (the angles, the gap, the
//! pieces' pictures); the first phase (Claw's blows do nothing, a sword
//! shot of the other element hurts him, flips the ring and moves him to
//! the other ledge after 112 ticks, other magic heals him up to 100, the
//! magic cooldown, the pieces that stop shots and hurt Claw by 1, his
//! contact that hurts while nothing can hit him); the end of the first
//! phase (the ring gone, full health, the hall); the second phase (each
//! blow costs 25 and sends him to the hall's other end, magic and pistol
//! shots make him block, his cast and its energy ball); his death (his
//! fall, the gem 176 ticks later, Claw held), his death with Claw dead (no
//! gem, ever) and Claw's respawn that starts the fight over.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::omar::{self, OmarData, bullet, shield};
use claw_sim::logic::enemy::{EnemyState, attacker};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{ObjectData, new_object, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, draw_flags, flags, types};
use claw_sim::player::attack::{FIRE_SHOT_FLAG, ICE_SHOT_FLAG, LIGHTNING_SHOT_FLAG};
use claw_sim::player::special::BOSS_END;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel};

// ----------------------------------------------------------------------
// The arena

/// The width of a tile, px.
const TILE: i32 = 64;
/// The floor's first row: its top is at y 2560.
const FLOOR_ROW: usize = 40;
/// Where Omar is placed, as on the level.
const AT: (i32, i32) = (42241, 2468);
/// Where Claw stands on the floor.
const CLAW_Y: i32 = 2499;
/// Claw within his reach, right of the pillar.
const NEAR: (i32, i32) = (41960, CLAW_Y);
/// Claw out of his reach.
const FAR: (i32, i32) = (40900, CLAW_Y);

fn anims() -> Vec<Arc<Animation>> {
    let mut strike = Animation::once(
        omar::CAST_ANIM,
        &[(211, 100), (212, 150), (213, 200), (214, 70), (215, 500)],
    );
    strike.frames[4].event = 1;
    let mut v = vec![
        Animation::looping(
            omar::IDLE_ANIM,
            &[(200, 300), (100, 150), (101, 850), (102, 100), (200, 600)],
        ),
        strike,
        Animation::once(
            omar::HIT_HIGH_ANIM,
            &[(500, 50), (501, 200), (500, 50), (200, 0)],
        ),
        Animation::once(
            omar::HIT_LOW_ANIM,
            &[(600, 50), (601, 200), (600, 50), (200, 0)],
        ),
        Animation::once(omar::BLOCK_ANIM, &[(700, 70), (701, 200)]),
        Animation::once(
            omar::DEATH_ANIM,
            &[
                (904, 150),
                (905, 120),
                (906, 500),
                (907, 90),
                (908, 250),
                (909, 130),
                (910, 700),
                (904, 100),
                (905, 800),
                (910, 120),
                (911, 120),
                (911, 0),
            ],
        ),
        Animation::looping(
            shield::FIRE_SET,
            &[(1, 80), (2, 80), (3, 80), (4, 80), (5, 80), (6, 80)],
        ),
        Animation::looping(
            shield::ICE_SET,
            &[(1, 65), (2, 65), (3, 65), (4, 65), (5, 65), (6, 65)],
        ),
        Animation::looping(bullet::ANIM, &[(1, 50), (2, 50)]),
    ];
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(Animation::once(key, &[(200, 150), (201, 200)]));
    }
    v.push(Animation::looping("CLAW_STAND", &[(11, 110), (12, 110)]));
    v.into_iter().map(Arc::new).collect()
}

/// The arena: open space over the floor, Omar placed as on the level.
fn arena() -> SimLevel {
    let cols = 690;
    let mut rows: Vec<String> = Vec::new();
    for r in 0..45 {
        let c = if r >= FLOOR_ROW { b'#' } else { b'.' };
        rows.push(String::from_utf8(vec![c; cols]).unwrap());
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(TILE, &rows), NEAR);
    level.number = 14;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for key in omar::sound_names() {
        level.sounds.insert(key.into(), Some(500));
    }
    let mut boss = ObjectSpec::at(omar::NAMES[0], AT.0, AT.1, 0);
    // Stationary, and the gem's target.
    boss.record[43] = 1;
    boss.record[55] = omar::GEM_TARGET.0;
    boss.record[56] = omar::GEM_TARGET.1;
    level.objects = vec![boss];
    level
}

/// The arena awake everywhere, Claw put at `claw` on the first tick.
fn arena_sim(claw: (i32, i32)) -> Sim {
    let level = arena();
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(claw.0, claw.1);
    sim.step(TickInput::default());
    sim
}

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    let mut v: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic && o.flags & flags::DESTROY == 0)
        .map(|(id, _)| id)
        .collect();
    v.sort();
    v
}

fn boss(sim: &Sim) -> ObjectId {
    find(sim, omar::NAMES[0])[0]
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn him(sim: &Sim) -> &Object {
    obj(sim, boss(sim))
}

fn st(sim: &Sim) -> Option<EnemyState> {
    EnemyState::from_id(him(sim).state)
}

fn od(sim: &Sim) -> OmarData {
    match &him(sim).data {
        ObjectData::Omar(d) => d.clone(),
        d => panic!("Omar's data: {d:?}"),
    }
}

fn pos(sim: &Sim) -> (i32, i32) {
    (him(sim).x, him(sim).y)
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .expect("Claw")
}

fn claw_health(sim: &Sim) -> i32 {
    sim.claw().stats.health
}

fn step(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

/// The pieces of his ring, by their place in it.
fn pieces(sim: &Sim) -> Vec<ObjectId> {
    od(sim).pieces
}

/// A blow of Claw's: a new attack of his.
fn blow(sim: &mut Sim, uppercut: bool) {
    let id = boss(sim);
    let claw = claw_id(sim);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        w.claw.attack.uppercut = uppercut;
        omar::on_hit(w, id, claw);
    });
}

/// Something of `kind` with the special flags `special` hits him (or the
/// object `target`): a new object made far away for it.
fn hit_by(sim: &mut Sim, target: ObjectId, kind: u32, special: u32) -> ObjectId {
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        let mut o = new_object("Bullet", 100, 100, 0);
        o.type_flags = kind;
        o.special_flags = special;
        o.damage = 25;
        let by = w.spawn(o);
        claw_sim::logic::on_hit(w, target, by);
        by
    })
}

/// A sword shot of ice, fire or lightning, or the magic claw (0).
fn shot(sim: &mut Sim, element: u32) -> ObjectId {
    let id = boss(sim);
    hit_by(sim, id, attacker::MAGIC, 0x8 | element)
}

fn destroyed(sim: &Sim, id: ObjectId) -> bool {
    sim.objects()
        .get(id)
        .is_none_or(|o| o.flags & flags::DESTROY != 0)
}

/// Starts the fight (the flag at 2) and lets him go idle.
fn start_fight(sim: &mut Sim) {
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    step(sim, 1);
}

/// Waits out a move started by a hit made between two ticks: such a hit
/// counts from the next tick's game time, so the move ends on the 113th
/// tick (a hit in a tick's own collision pass ends it on the 112th).
fn after_move(sim: &mut Sim) {
    step(sim, omar::MOVE_TICKS as usize + 1);
}

/// Sets his health between two ticks.
fn set_health(sim: &mut Sim, h: i32) {
    let id = boss(sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health = h;
        }
    });
}

/// Ends his first phase: an ice shot at 25 health, then his move.
fn to_the_hall(sim: &mut Sim) {
    start_fight(sim);
    set_health(sim, 25);
    shot(sim, ICE_SHOT_FLAG);
    after_move(sim);
    assert_eq!(od(sim).phase, 0);
    assert_eq!(pos(sim), omar::HALL);
}

// ----------------------------------------------------------------------
// Building and waiting

#[test]
fn he_is_built_as_the_current_boss_with_his_ring_of_fire_waiting_for_the_fight() {
    let mut sim = arena_sim(NEAR);
    let id = boss(&sim);
    assert_eq!(sim.boss().current(), Some(id));
    let o = him(&sim);
    assert_eq!((o.health, o.damage, o.z), (100, 30, omar::DEPTH));
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.hit_rect, omar::BODY_RECT);
    assert_eq!(o.attack_rect, omar::CONTACT_RECT);
    assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
    // Claw within his reach: he waits in Startup, his ring around him.
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(pos(&sim), omar::HOME);
    let d = od(&sim);
    assert_eq!((d.phase, d.ice, d.gem_given), (1, false, false));
    assert_eq!(d.pieces.len(), omar::RING_PIECES);
    for &p in &d.pieces {
        let o = obj(&sim, p);
        assert_eq!(o.z, omar::DEPTH + 1);
        assert_eq!(o.type_flags, types::ENEMY);
        assert_eq!((o.hit_rect, o.attack_rect), (shield::RECT, shield::RECT));
        assert_eq!(o.image_set, shield::FIRE_SET);
    }
    // Waiting, nothing moves him; the flag at 2 sends him idle.
    step(&mut sim, 100);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    start_fight(&mut sim);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    assert_eq!(pos(&sim), omar::HOME);
}

#[test]
fn out_of_his_reach_he_goes_idle_on_his_first_tick() {
    let mut sim = arena_sim(FAR);
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    // In the first phase he never leaves Idle by himself.
    step(&mut sim, 600);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    assert_eq!(him(&sim).health, 100);
    assert_eq!(pos(&sim), omar::HOME);
}

#[test]
fn his_ring_turns_on_the_level_clock_with_a_gap() {
    let mut sim = arena_sim(FAR);
    step(&mut sim, 3);
    for _ in 0..200 {
        step(&mut sim, 1);
        // The pieces stand where the previous tick's turn left them: the
        // level clock at that tick's end.
        let ms = (sim.tick() - 1) * 18;
        let at = pos(&sim);
        for (i, &p) in pieces(&sim).iter().enumerate() {
            let o = obj(&sim, p);
            assert_eq!(
                (o.x, o.y),
                omar::ring_point(i, ms, at),
                "piece {i} at {ms} ms"
            );
        }
    }
    // Nine pieces 33 degrees apart: the ring's gap is 96 degrees wide.
    let a: Vec<f64> = (0..omar::RING_PIECES)
        .map(|i| omar::ring_angle(i, 5000))
        .collect();
    assert_eq!(a[1] - a[0], 33.0);
    assert_eq!(a[0] + 360.0 - a[8], 96.0);
}

// ----------------------------------------------------------------------
// The first phase

#[test]
fn in_the_first_phase_claws_blows_do_nothing() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    blow(&mut sim, false);
    blow(&mut sim, true);
    assert_eq!(him(&sim).health, 100);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
}

#[test]
fn an_ice_shot_hurts_the_fire_ring_and_moves_him_to_the_other_ledge() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    let s = shot(&mut sim, ICE_SHOT_FLAG);
    // 25 off, the shot used up, the ring of ice turning the other way, and
    // the move: hit-proof, flashing where he was.
    assert_eq!(him(&sim).health, 75);
    assert!(destroyed(&sim, s));
    assert_eq!(st(&sim), Some(EnemyState::MoveToPosition));
    assert!(him(&sim).flags & flags::SAFE != 0);
    assert!(him(&sim).draw_flags & draw_flags::FLASH != 0);
    let d = od(&sim);
    assert!(d.ice);
    assert!(d.spins.iter().all(|s| s.speed == -omar::RING_SPEED));
    assert_eq!(d.target, omar::LEDGE_LEFT);
    // Nothing reaches him while he moves.
    step(&mut sim, 1);
    assert_eq!(obj(&sim, pieces(&sim)[0]).image_set, shield::ICE_SET);
    step(&mut sim, omar::MOVE_TICKS as usize - 1);
    assert_eq!(pos(&sim), omar::HOME);
    assert_eq!(st(&sim), Some(EnemyState::MoveToPosition));
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(pos(&sim), omar::LEDGE_LEFT);
    assert!(him(&sim).flags & flags::SAFE == 0);
    assert!(him(&sim).draw_flags & draw_flags::FLASH == 0);
    // On the ice ring, a fire shot hurts him and sends him back.
    step(&mut sim, 30);
    shot(&mut sim, FIRE_SHOT_FLAG);
    assert_eq!(him(&sim).health, 50);
    assert!(!od(&sim).ice);
    assert!(od(&sim).spins.iter().all(|s| s.speed == omar::RING_SPEED));
    after_move(&mut sim);
    assert_eq!(pos(&sim), omar::HOME);
}

#[test]
fn other_magic_heals_him_up_to_100_and_the_cooldown_turns_shots_away() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    set_health(&mut sim, 50);
    // A fire shot on the fire ring heals him and is used up; he flinches.
    let s = shot(&mut sim, FIRE_SHOT_FLAG);
    assert_eq!(him(&sim).health, 75);
    assert!(destroyed(&sim, s));
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    // Within 500 ms nothing more reaches him.
    step(&mut sim, 1);
    shot(&mut sim, 0);
    assert_eq!(him(&sim).health, 75);
    // After it, the magic claw and the lightning heal him, to 100 at most.
    step(&mut sim, 30);
    shot(&mut sim, 0);
    assert_eq!(him(&sim).health, 100);
    step(&mut sim, 30);
    shot(&mut sim, LIGHTNING_SHOT_FLAG);
    assert_eq!(him(&sim).health, 100);
    // The flinch ends Home.
    step(&mut sim, 30);
    assert_eq!(st(&sim), Some(EnemyState::Home));
}

#[test]
fn pistol_shots_are_used_up_and_he_blocks() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    let id = boss(&sim);
    let s = hit_by(&mut sim, id, attacker::PISTOL_SHOT, 0);
    assert!(destroyed(&sim, s));
    assert_eq!(st(&sim), Some(EnemyState::BlockLow));
    assert!(him(&sim).flags & flags::SAFE != 0);
    assert_eq!(him(&sim).health, 100);
    step(&mut sim, 20);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert!(him(&sim).flags & flags::SAFE == 0);
    // A thrown object is used up, nothing more.
    let t = hit_by(&mut sim, id, attacker::THROWN, 0);
    assert!(destroyed(&sim, t));
    assert_eq!(st(&sim), Some(EnemyState::Home));
}

#[test]
fn the_pieces_stop_shots_and_hurt_claw_by_one() {
    let mut sim = arena_sim(FAR);
    step(&mut sim, 1);
    let p = pieces(&sim)[2];
    // Pistol shots and magic end on a piece; Claw's blows and the level's
    // shots do nothing to it.
    let a = hit_by(&mut sim, p, attacker::PISTOL_SHOT, 0);
    let b = hit_by(&mut sim, p, attacker::MAGIC, ICE_SHOT_FLAG);
    let c = hit_by(&mut sim, p, types::LEVEL_SHOT, 0);
    assert!(destroyed(&sim, a) && destroyed(&sim, b));
    assert!(!destroyed(&sim, c));
    assert_eq!(him(&sim).health, 100);
    // Claw standing in a piece loses one health.
    let (x, y) = (obj(&sim, p).x, obj(&sim, p).y);
    sim.teleport_player(x, y + 20);
    let before = claw_health(&sim);
    step(&mut sim, 1);
    assert_eq!(claw_health(&sim), before - shield::DAMAGE);
}

#[test]
fn his_touch_hurts_claw_by_30_also_while_he_moves() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    shot(&mut sim, ICE_SHOT_FLAG);
    assert!(him(&sim).flags & flags::SAFE != 0);
    sim.teleport_player(omar::HOME.0, CLAW_Y);
    step(&mut sim, 1);
    assert_eq!(claw_health(&sim), 70);
}

#[test]
fn the_fourth_good_shot_ends_the_ring_and_sends_him_to_the_hall_at_full_health() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    let ring = pieces(&sim);
    set_health(&mut sim, 25);
    shot(&mut sim, ICE_SHOT_FLAG);
    let d = od(&sim);
    assert_eq!(d.phase, 0);
    assert!(d.pieces.is_empty());
    assert!(d.right_end);
    assert_eq!(d.target, omar::HALL);
    assert_eq!(him(&sim).health, 100);
    assert_eq!(st(&sim), Some(EnemyState::MoveToPosition));
    step(&mut sim, 1);
    assert!(ring.iter().all(|&p| sim.objects().get(p).is_none()));
    step(&mut sim, omar::MOVE_TICKS as usize);
    assert_eq!(pos(&sim), omar::HALL);
    assert_eq!(st(&sim), Some(EnemyState::Home));
}

// ----------------------------------------------------------------------
// The second phase

#[test]
fn each_blow_costs_him_25_and_sends_him_to_the_halls_other_end() {
    let mut sim = arena_sim(NEAR);
    to_the_hall(&mut sim);
    blow(&mut sim, false);
    assert_eq!(him(&sim).health, 75);
    assert_eq!(st(&sim), Some(EnemyState::MoveToPosition));
    assert!(!od(&sim).right_end);
    after_move(&mut sim);
    assert_eq!(pos(&sim), omar::HALL_LEFT);
    blow(&mut sim, true);
    assert_eq!(him(&sim).health, 50);
    after_move(&mut sim);
    assert_eq!(pos(&sim), omar::HALL);
    // Magic makes him block; the shot flies on.
    let s = shot(&mut sim, ICE_SHOT_FLAG);
    assert!(!destroyed(&sim, s));
    assert_eq!(st(&sim), Some(EnemyState::BlockLow));
    assert_eq!(him(&sim).health, 50);
}

#[test]
fn he_casts_energy_balls_from_home_in_the_hall() {
    let mut sim = arena_sim(NEAR);
    to_the_hall(&mut sim);
    // Claw left of him: his first cast comes from a pulse roll of 0..=49.
    let mut cast = None;
    for k in 0..3000 {
        step(&mut sim, 1);
        if st(&sim) == Some(EnemyState::MagicAttack) {
            cast = Some(k);
            break;
        }
    }
    assert!(cast.is_some(), "a cast");
    // Facing left, the ball leaves 65 px left of him and 20 up on the
    // cast's 32nd tick, and the cast lasts 60 ticks.
    let mut ball = None;
    for k in 1..=60 {
        step(&mut sim, 1);
        if ball.is_none()
            && let Some(&b) = find(&sim, bullet::NAMES[0]).first()
        {
            ball = Some((k, b));
        }
    }
    let (k, b) = ball.expect("the ball");
    assert_eq!(k, 32, "the ball on the cast's 32nd call");
    assert_eq!(st(&sim), Some(EnemyState::Home));
    let o = obj(&sim, b);
    assert_eq!(o.type_flags, types::ENEMY_SHOT);
    assert_eq!(o.damage, omar::BALL_DAMAGE);
    assert_eq!(o.z, bullet::DEPTH);
    // 5.4 px a tick to the left.
    let x0 = obj(&sim, b).x;
    step(&mut sim, 10);
    assert_eq!(x0 - obj(&sim, b).x, 54);
}

#[test]
fn his_ball_costs_claw_15_and_is_gone() {
    let mut sim = arena_sim(NEAR);
    let b = sim.with_world(|w| bullet::launch(w, (NEAR.0 + 40, CLAW_Y, 5000), -300));
    step(&mut sim, 12);
    assert_eq!(claw_health(&sim), 100 - omar::BALL_DAMAGE);
    assert!(sim.objects().get(b).is_none());
}

#[test]
fn the_fourth_blow_kills_him_and_the_gem_rises_176_ticks_later() {
    let mut sim = arena_sim(NEAR);
    to_the_hall(&mut sim);
    set_health(&mut sim, 25);
    blow(&mut sim, false);
    assert_eq!(st(&sim), Some(EnemyState::Dead));
    assert!(him(&sim).flags & flags::NO_COLLIDE != 0);
    // He dies where the blow found him, at the hall's right end.
    let mut gem = None;
    for k in 1..=200 {
        step(&mut sim, 1);
        if gem.is_none() && !find(&sim, pickup::END_GEM).is_empty() {
            gem = Some(k);
        }
    }
    assert_eq!(gem, Some(omar::GEM_TICKS as usize));
    assert_eq!(st(&sim), Some(EnemyState::Wait));
    assert_eq!(pos(&sim), omar::HALL);
    assert!(od(&sim).gem_given);
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    assert_eq!(sim.boss().defeated(), Some(boss(&sim)));
}

#[test]
fn with_claw_dead_at_his_falls_end_no_gem_comes_ever() {
    let mut sim = arena_sim(NEAR);
    to_the_hall(&mut sim);
    set_health(&mut sim, 25);
    blow(&mut sim, false);
    // The life ends while he falls; his fall ends before Claw is back.
    step(&mut sim, 100);
    sim.end_life();
    step(&mut sim, 80);
    assert_eq!(st(&sim), Some(EnemyState::Dead));
    assert!(find(&sim, pickup::END_GEM).is_empty());
    assert!(od(&sim).gem_given);
    // Claw's respawn starts the fight over: the first phase at his place,
    // a new ring, full health; out of every collision still.
    step(&mut sim, 40);
    assert_eq!(sim.player().unwrap().state, claw_sim::player::state::STAND);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    assert_eq!(pos(&sim), omar::HOME);
    let d = od(&sim);
    assert_eq!((d.phase, d.ice, d.pieces.len()), (1, false, 9));
    assert_eq!(him(&sim).health, 100);
    assert!(him(&sim).flags & flags::NO_COLLIDE != 0);
    assert!(him(&sim).flags & flags::SAFE == 0);
}

#[test]
fn claws_respawn_resets_him_with_his_ring_on_the_level_clock() {
    let mut sim = arena_sim(NEAR);
    start_fight(&mut sim);
    shot(&mut sim, ICE_SHOT_FLAG);
    after_move(&mut sim);
    assert_eq!(pos(&sim), omar::LEDGE_LEFT);
    sim.end_life();
    let mut back = false;
    for _ in 0..300 {
        step(&mut sim, 1);
        if sim.player().unwrap().state == claw_sim::player::state::STAND {
            back = true;
            break;
        }
    }
    assert!(back, "Claw respawns");
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    assert_eq!(pos(&sim), omar::HOME);
    assert_eq!(him(&sim).health, 100);
    let d = od(&sim);
    assert!(!d.ice);
    // The ring turns back from where the clock puts it.
    step(&mut sim, 1);
    let ms = (sim.tick() - 1) * 18;
    for (i, &p) in pieces(&sim).iter().enumerate() {
        let o = obj(&sim, p);
        assert_eq!((o.x, o.y), omar::ring_point(i, ms, omar::HOME), "{i}");
        assert_eq!(o.image_set, shield::FIRE_SET);
    }
}
