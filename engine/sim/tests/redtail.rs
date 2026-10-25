//! RedTail, his stakes, his wind, his knives and his flares on a synthetic
//! arena with the thirteenth level's animation timings: a wall on the
//! left, a floor (the sand) whose top is at y 2112, a ledge whose top is
//! at y 1920 over its middle, the stakes by the wall.
//!
//! How he is built (the current boss on the ledge, his boxes, his count at
//! 1, waiting for the fight), his body that hurts while he waits, the
//! fight flag's reset and his first decision 500 ms later; Claw's blows
//! (each costs him its damage, every third counted blow that finds him
//! free sends him to the other level, one in a flinch only counts, the
//! third in a flinch is lost); the block, the hop back and his own shots
//! passing through him; the gun and its flare, the kick, the slide, the
//! knives' fan, the wind with its push, its debris and its end, the
//! stakes; the jumps between the levels and the landing; the fall guard;
//! his death on the ledge (the jump down, the fall, the line, the gem),
//! his death with Claw dead and the respawn that starts the fight over.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::boss::{debris, flare, knife, spikes, wind};
use claw_sim::logic::enemy::redtail::{self, RedTailData};
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{ObjectData, new_object, pickup};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::player::special::BOSS_END;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel};

// ----------------------------------------------------------------------
// The arena

/// The width of a tile, px.
const TILE: i32 = 64;
/// The columns of the wall on the left (its right edge is at x 37119).
const WALL_COLS: usize = 580;
/// The columns of the ledge.
const LEDGE_COLS: std::ops::Range<usize> = 584..590;
/// Where RedTail is placed: over the ledge.
const AT: (i32, i32) = (37500, 1850);
/// Where he stands on the ledge, and on the sand.
const LEDGE_Y: i32 = 1864;
const SAND_Y: i32 = 2056;
/// Where Claw stands on the sand, and on the ledge.
const CLAW_SAND_Y: i32 = 2051;
const CLAW_LEDGE_Y: i32 = 1859;
/// Where the stakes are placed.
const STAKES: (i32, i32) = (37150, 2017);

fn anims() -> Vec<Arc<Animation>> {
    let mut v = vec![
        Animation::looping(
            redtail::IDLE_ANIM,
            &[
                (100, 200),
                (101, 800),
                (102, 200),
                (100, 200),
                (101, 400),
                (102, 100),
                (100, 300),
                (101, 350),
                (200, 1500),
            ],
        ),
        Animation::looping(
            redtail::RUN_ANIM,
            &[
                (1, 110),
                (2, 110),
                (3, 110),
                (4, 110),
                (5, 110),
                (6, 110),
                (7, 110),
                (8, 110),
            ],
        ),
        Animation::once(redtail::KNIVES_DOWN_ANIM, &[(203, 70), (204, 200)]),
        Animation::once(redtail::KNIVES_UP_ANIM, &[(211, 70), (212, 200)]),
        Animation::once(
            redtail::GUN_ANIM,
            &[
                (206, 70),
                (207, 70),
                (208, 70),
                (209, 200),
                (208, 70),
                (207, 100),
            ],
        ),
        Animation::once(redtail::WIND_ANIM, &[(210, 300)]),
        Animation::once(redtail::SLIDE_ANIM, &[(301, 120), (302, 60_000)]),
        Animation::once(
            redtail::KICK_ANIM,
            &[(400, 50), (402, 200), (401, 50), (200, 0)],
        ),
        Animation::once(redtail::BLOCK_ANIM, &[(204, 0)]),
        Animation::once(
            redtail::HIT_HIGH_ANIM,
            &[(500, 50), (501, 200), (500, 50), (200, 0)],
        ),
        Animation::once(
            redtail::HIT_LOW_ANIM,
            &[(600, 50), (601, 200), (600, 50), (200, 0)],
        ),
        Animation::once(redtail::JUMP_ANIM, &[(800, 50), (801, 200), (802, 60_000)]),
        Animation::once(
            redtail::DEATH_ANIM,
            &[
                (500, 50),
                (501, 250),
                (900, 300),
                (901, 1000),
                (902, 290),
                (903, 230),
                (904, 0),
            ],
        ),
        Animation::looping(knife::SPIN_ANIM, &[(1, 50), (2, 50)]),
        Animation::once(flare::EXPLOSION_ANIM, &[(1, 300), (2, 300)]),
    ];
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(Animation::once(key, &[(200, 150), (201, 200)]));
    }
    v.push(Animation::looping("CLAW_STAND", &[(11, 110), (12, 110)]));
    v.push(Animation::looping("CLAW_WALK", &[(20, 100), (21, 100)]));
    v.push(Animation::looping("CLAW_FALL", &[(278, 65), (280, 80)]));
    v.into_iter().map(Arc::new).collect()
}

/// The arena: the wall, the ledge and the floor, with RedTail and `more`.
fn arena(more: Vec<ObjectSpec>) -> SimLevel {
    let cols = 610;
    let mut rows: Vec<String> = Vec::new();
    for r in 0..36 {
        let mut row: Vec<u8> = vec![b'.'; cols];
        for c in row.iter_mut().take(WALL_COLS) {
            *c = b'#';
        }
        if r == 30 {
            for c in LEDGE_COLS {
                row[c] = b'#';
            }
        }
        if r >= 33 {
            row.fill(b'#');
        }
        rows.push(String::from_utf8(row).unwrap());
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(TILE, &rows), (37300, CLAW_SAND_Y));
    level.number = 13;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    for key in redtail::sound_names() {
        level.sounds.insert(key.into(), Some(500));
    }
    level.sounds.insert(redtail::DEATH_LINE.into(), Some(2360));
    let mut boss = ObjectSpec::at(redtail::NAMES[0], AT.0, AT.1, 9000);
    // The gem's target.
    boss.record[55] = redtail::GEM_TARGET.0;
    boss.record[56] = redtail::GEM_TARGET.1;
    level.objects = vec![boss];
    level.objects.extend(more);
    level
}

fn with_helpers() -> Vec<ObjectSpec> {
    vec![
        ObjectSpec::at(wind::NAMES[0], 37509, 2009, 10_000),
        ObjectSpec::at(spikes::NAMES[0], STAKES.0, STAKES.1, 1000),
    ]
}

/// The arena awake everywhere, Claw put at `claw` on the first tick. The
/// level's set-up builds RedTail before Claw is put there, so his count
/// starts as in a run played from the level's start.
fn arena_sim(more: Vec<ObjectSpec>, claw: (i32, i32), seed: u32) -> Sim {
    let level = arena(more);
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(claw.0, claw.1);
    sim.step(TickInput::default());
    sim
}

/// Claw on the sand left of the ledge, clear of the stakes.
const SAND_FAR: (i32, i32) = (37300, CLAW_SAND_Y);

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    let mut v: Vec<(ObjectId, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, o)| (id, o.x))
        .collect();
    v.sort_by_key(|&(_, x)| x);
    v.into_iter().map(|(id, _)| id).collect()
}

fn boss(sim: &Sim) -> ObjectId {
    find(sim, redtail::NAMES[0])[0]
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn st(sim: &Sim) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, boss(sim)).state)
}

fn rd(sim: &Sim) -> RedTailData {
    match &obj(sim, boss(sim)).data {
        ObjectData::RedTail(d) => d.clone(),
        d => panic!("RedTail's data: {d:?}"),
    }
}

fn pos(sim: &Sim) -> (i32, i32) {
    let o = obj(sim, boss(sim));
    (o.x, o.y)
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

fn claw_xy(sim: &Sim) -> (i32, i32) {
    let o = obj(sim, claw_id(sim));
    (o.x, o.y)
}

fn step(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

/// Steps until RedTail's state is `s`, at most `n` ticks; the ticks taken.
fn step_until(sim: &mut Sim, s: EnemyState, n: usize) -> usize {
    for k in 1..=n {
        step(sim, 1);
        if st(sim) == Some(s) {
            return k;
        }
    }
    panic!("not {s:?} within {n} ticks: {:?}", st(sim));
}

/// Starts the fight (the flag at 2) and lets the reset run.
fn start_fight(sim: &mut Sim) {
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    step(sim, 1);
}

/// Runs `f` on RedTail inside the world.
fn act(sim: &mut Sim, f: fn(&mut claw_sim::logic::World, ObjectId)) {
    let id = boss(sim);
    sim.with_world(|w| f(w, id));
}

/// A blow of Claw's sword dealing `damage`: a new attack of his.
fn blow(sim: &mut Sim, damage: i32) {
    let id = boss(sim);
    let claw = claw_id(sim);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        if let Some(o) = w.objects.get_mut(claw) {
            o.damage = damage;
        }
        redtail::on_hit(w, id, claw);
    });
}

/// Something of `kind` hits him: a new object made far away for it.
fn hit_by(sim: &mut Sim, kind: u32) -> ObjectId {
    let id = boss(sim);
    sim.with_world(|w| {
        w.claw.attack.serial += 1;
        let mut o = new_object("Bullet", 100, 100, 0);
        o.type_flags = kind;
        o.damage = 5;
        let by = w.spawn(o);
        redtail::on_hit(w, id, by);
        by
    })
}

/// Gives him the count a run started by a teleport builds him with, so
/// that the next blow is not a third one.
fn teleported_count(sim: &mut Sim) {
    let id = boss(sim);
    sim.with_world(|w| {
        if let Some(ObjectData::RedTail(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            d.hits = redtail::HIT_COUNTER_START;
        }
    });
}

/// Puts him on the sand at `x`, standing, free and at Home.
fn on_sand(sim: &mut Sim, x: i32) {
    let id = boss(sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.x = x;
            o.y = SAND_Y;
            o.physics_type = ptype::STAND;
        }
    });
    step(sim, 1);
    assert_eq!(pos(sim), (x, SAND_Y));
}

// ----------------------------------------------------------------------
// Building and waiting

#[test]
fn he_is_built_on_the_ledge_as_the_current_boss_waiting_with_his_count_past_its_limit() {
    let mut sim = arena_sim(with_helpers(), SAND_FAR, 7);
    let id = boss(&sim);
    assert_eq!(sim.boss().current(), Some(id));
    assert_eq!(sim.redtail_link().redtail, Some(id));
    assert!(!sim.redtail_link().wind);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(pos(&sim), (AT.0, LEDGE_Y));
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (redtail::HEALTH, redtail::DAMAGE));
    assert_eq!(o.move_rect, redtail::BODY_RECT);
    assert_eq!(o.hit_rect, redtail::HIT_RECT);
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.hit_type_flags, redtail::HIT_MASK);
    assert_ne!(o.flags & flags::ALWAYS_ACTIVE, 0);
    assert_eq!(rd(&sim).hits, redtail::HIT_COUNTER_LEVEL_START);
    step(&mut sim, 400);
    let o = obj(&sim, id);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(pos(&sim), (AT.0, LEDGE_Y));
    assert_eq!(o.special_flags & special::LIFTABLE, 0, "never liftable");
    assert_eq!(o.attack_rect, redtail::BODY_RECT, "his body hurts");
}

#[test]
fn played_from_the_level_start_his_first_free_blow_after_a_skip_sends_him_home_unhurt() {
    // Claw at the level's own start point on the sand, never teleported.
    let level = arena(with_helpers());
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.step(TickInput::default());
    assert!(!sim.claw_placed());
    let id = boss(&sim);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(rd(&sim).hits, redtail::HIT_COUNTER_LEVEL_START);
    step(&mut sim, 30);
    // The first counted blow, his introduction skipped: no damage, the
    // count back to 0; Claw below on the sand, he only goes home.
    let before = sim.rng_state();
    blow(&mut sim, 5);
    assert_eq!((obj(&sim, id).health, rd(&sim).hits), (redtail::HEALTH, 0));
    assert_eq!(sim.rng_state(), before, "the blow draws nothing");
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(pos(&sim), (AT.0, LEDGE_Y));
}

#[test]
fn in_a_run_started_by_a_teleport_his_count_starts_at_one() {
    // Claw starts far off inside the wall, out of reach of RedTail's
    // activation, and is put on the sand before the first tick: RedTail is
    // built once he is near, his count at 1.
    let mut level = arena(with_helpers());
    level.start = (2000, 500);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            ..SimConfig::default()
        },
    );
    assert!(
        find(&sim, redtail::NAMES[0])
            .iter()
            .all(|&id| !matches!(obj(&sim, id).data, ObjectData::RedTail(_))),
        "not built yet"
    );
    sim.teleport_player(SAND_FAR.0, SAND_FAR.1);
    assert!(sim.claw_placed());
    step(&mut sim, 3);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(rd(&sim).hits, redtail::HIT_COUNTER_START);
}

#[test]
fn waiting_he_never_attacks_but_his_body_hurts() {
    // Claw on the ledge, in his body.
    let mut sim = arena_sim(with_helpers(), (AT.0 - 10, CLAW_LEDGE_Y), 7);
    step(&mut sim, 3);
    assert_eq!(claw_health(&sim), 100 - redtail::DAMAGE);
    // Beside him, out of his body, nothing ever happens.
    let mut sim = arena_sim(with_helpers(), (AT.0 - 71, CLAW_LEDGE_Y), 7);
    step(&mut sim, 1500);
    assert_eq!(claw_health(&sim), 100);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(sim.boss().fight(), fight::NONE);
}

#[test]
fn the_fight_flag_resets_him_and_his_first_decision_comes_500_ms_later() {
    let mut sim = arena_sim(with_helpers(), SAND_FAR, 7);
    // Hurt and counted before the fight.
    let id = boss(&sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health = 40;
        }
        w.redtail_link_mut().wind = true;
    });
    start_fight(&mut sim);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, redtail::HEALTH);
    assert_eq!(rd(&sim).hits, 0);
    assert!(!sim.redtail_link().wind);
    // Put back where he was placed, a little above the ledge, falling.
    assert_eq!(obj(&sim, id).physics_type, ptype::FLY);
    assert!(pos(&sim).1 < LEDGE_Y);
    step(&mut sim, 8);
    assert_eq!(pos(&sim), (AT.0, LEDGE_Y));
    // 500 ms of Home (28 ticks), then a decision.
    step(&mut sim, 19);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    step(&mut sim, 1);
    assert_ne!(st(&sim), Some(EnemyState::Home));
}

// ----------------------------------------------------------------------
// Claw's blows

#[test]
fn each_blow_costs_its_damage_and_the_third_counted_blow_sends_him_away() {
    // Claw on the ledge 60 px left of him.
    let mut sim = arena_sim(with_helpers(), (AT.0 - 60, CLAW_LEDGE_Y), 7);
    start_fight(&mut sim);
    step(&mut sim, 8);
    let id = boss(&sim);
    blow(&mut sim, 5);
    assert_eq!(obj(&sim, id).health, 95);
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    assert_eq!(rd(&sim).hits, 1);
    step_until(&mut sim, EnemyState::Home, 30);
    blow(&mut sim, 5);
    assert_eq!(obj(&sim, id).health, 90);
    assert_eq!(rd(&sim).hits, 2);
    step_until(&mut sim, EnemyState::Home, 30);
    // The third: no damage, the count back to 0, and in the post-tick he
    // leaps away from Claw's level (down, Claw being on the ledge).
    blow(&mut sim, 5);
    assert_eq!(obj(&sim, id).health, 90);
    assert_eq!(rd(&sim).hits, 0);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::JumpDown));
    assert_eq!(rd(&sim).pending, 0);
    step_until(&mut sim, EnemyState::Home, 60);
    assert_eq!(pos(&sim).1, SAND_Y);
}

#[test]
fn a_blow_in_a_flinchs_last_ticks_counts_without_damage_and_a_third_there_is_lost() {
    let mut sim = arena_sim(with_helpers(), (AT.0 - 60, CLAW_LEDGE_Y), 7);
    start_fight(&mut sim);
    step(&mut sim, 8);
    let id = boss(&sim);
    blow(&mut sim, 5);
    assert_eq!(rd(&sim).hits, 1);
    // Within the 300 ms cooldown a blow is turned away, not counted.
    step(&mut sim, 5);
    blow(&mut sim, 5);
    assert_eq!((rd(&sim).hits, obj(&sim, id).health), (1, 95));
    // On the flinch's 17th tick the cooldown is over: the blow counts, does
    // no damage and sends him Home.
    step(&mut sim, 12);
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    blow(&mut sim, 5);
    assert_eq!((rd(&sim).hits, obj(&sim, id).health), (2, 95));
    assert_eq!(st(&sim), Some(EnemyState::Home));
    // A third counted blow during a flinch: lost, no escape.
    step(&mut sim, 1);
    sim.with_world(|w| {
        if let Some(ObjectData::RedTail(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            d.hits = 1;
        }
    });
    blow(&mut sim, 5);
    assert_eq!((rd(&sim).hits, obj(&sim, id).health), (2, 90));
    step(&mut sim, 17);
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    blow(&mut sim, 5);
    assert_eq!((rd(&sim).hits, obj(&sim, id).health), (0, 90));
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    step_until(&mut sim, EnemyState::Home, 5);
}

#[test]
fn pistol_shots_and_magic_make_him_block_and_his_own_shots_pass_through() {
    let mut sim = arena_sim(with_helpers(), SAND_FAR, 7);
    let id = boss(&sim);
    let shot = hit_by(&mut sim, types::PISTOL_SHOT);
    assert_eq!(st(&sim), Some(EnemyState::Block));
    assert!(
        sim.objects()
            .get(shot)
            .is_none_or(|o| o.flags & flags::DESTROY != 0),
        "used up"
    );
    assert_eq!(obj(&sim, id).health, redtail::HEALTH);
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    hit_by(&mut sim, 0x200_0000);
    assert_eq!(st(&sim), Some(EnemyState::Block));
    step(&mut sim, 1);
    // His own knives and flares do nothing to him.
    let before = rd(&sim);
    hit_by(&mut sim, types::ENEMY_SHOT);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(rd(&sim), before);
}

#[test]
fn an_explosion_makes_him_hop_back_when_free() {
    let mut sim = arena_sim(with_helpers(), (AT.0 - 150, CLAW_LEDGE_Y), 7);
    start_fight(&mut sim);
    step(&mut sim, 8);
    hit_by(&mut sim, types::LEVEL_SHOT);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::JumpBack));
    step_until(&mut sim, EnemyState::Home, 40);
    let (x, y) = pos(&sim);
    assert_eq!(y, LEDGE_Y);
    assert!(
        (AT.0 + 95..=AT.0 + 100).contains(&x),
        "about 100 px away from Claw: {x}"
    );
}

// ----------------------------------------------------------------------
// His attacks

#[test]
fn the_gun_fires_a_flare_on_its_13th_call_that_bursts_on_claw() {
    let mut sim = arena_sim(with_helpers(), (37650, CLAW_SAND_Y), 7);
    on_sand(&mut sim, 37850);
    act(&mut sim, redtail::gun_entry);
    assert_eq!(st(&sim), Some(EnemyState::GunAttack));
    step(&mut sim, 12);
    assert!(find(&sim, flare::NAMES[0]).is_empty());
    step(&mut sim, 1);
    let f = find(&sim, flare::NAMES[0]);
    assert_eq!(f.len(), 1);
    let fl = obj(&sim, f[0]);
    assert_eq!((fl.x, fl.y), (37850 - 66, SAND_Y - 9));
    assert_eq!(fl.damage, flare::DAMAGE);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, f[0]).x, 37850 - 66 - 9);
    let mut k = 0;
    while claw_health(&sim) == 100 && k < 40 {
        step(&mut sim, 1);
        k += 1;
    }
    assert_eq!(claw_health(&sim), 100 - flare::DAMAGE);
    assert_eq!(obj(&sim, f[0]).state, flare::state::EXPLODING);
    assert_eq!(obj(&sim, f[0]).type_flags, types::LEVEL_SHOT);
    // The gun lasts 35 calls.
    step_until(&mut sim, EnemyState::Home, 35);
}

#[test]
fn the_kick_strikes_with_its_box_while_its_striking_picture_shows() {
    let mut sim = arena_sim(with_helpers(), (37800, CLAW_SAND_Y), 7);
    on_sand(&mut sim, 37850);
    act(&mut sim, redtail::kick_entry);
    assert_eq!(st(&sim), Some(EnemyState::KickAttack));
    let id = boss(&sim);
    for call in 1..=19 {
        step(&mut sim, 1);
        let o = obj(&sim, id);
        if (4..=15).contains(&call) {
            assert_eq!(o.attack_rect, redtail::KICK_RECT, "call {call}");
        } else if call < 4 {
            assert_eq!(o.attack_rect, redtail::BODY_RECT, "call {call}");
        }
    }
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(claw_health(&sim), 100 - redtail::DAMAGE);
}

#[test]
fn the_slide_runs_at_claw_and_ends_once_past_him() {
    let mut sim = arena_sim(with_helpers(), (37760, CLAW_SAND_Y), 7);
    on_sand(&mut sim, 37850);
    act(&mut sim, redtail::slide_entry);
    assert_eq!(st(&sim), Some(EnemyState::SlideAttack));
    assert_eq!(rd(&sim).slide_to, 37760 - redtail::SLIDE_PAST);
    let id = boss(&sim);
    step(&mut sim, 1);
    // 5.04 px a tick.
    assert_eq!(pos(&sim).0, 37850 - 6);
    step(&mut sim, 6);
    assert_eq!(obj(&sim, id).attack_rect, redtail::BODY_RECT);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, id).attack_rect, redtail::SLIDE_RECT);
    step_until(&mut sim, EnemyState::Home, 60);
    assert_eq!(claw_health(&sim), 100 - redtail::DAMAGE);
}

#[test]
fn the_knives_fly_in_a_lopsided_fan_at_the_other_level() {
    let mut sim = arena_sim(with_helpers(), (37200, CLAW_SAND_Y), 7);
    act(&mut sim, redtail::knives_entry);
    assert_eq!(st(&sim), Some(EnemyState::KnifeAttack));
    step(&mut sim, 16);
    assert!(find(&sim, knife::NAMES[0]).is_empty());
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    let knives = find(&sim, knife::NAMES[0]);
    assert_eq!(knives.len(), 3);
    let speeds: Vec<(i32, i32)> = {
        let mut v: Vec<(i32, i32)> = knives
            .iter()
            .map(|&k| (obj(&sim, k).speed_x, obj(&sim, k).speed_y))
            .collect();
        v.sort();
        v
    };
    assert_eq!(speeds, vec![(-400, 150), (-400, 300), (-250, 300)]);
    for &k in &knives {
        let o = obj(&sim, k);
        assert_eq!((o.x, o.y), (AT.0 - 35, LEDGE_Y + 4));
        assert_eq!(o.damage, knife::DAMAGE);
        assert_eq!(o.type_flags, types::ENEMY_SHOT);
    }
    // Through the ledge; gone below the sand.
    step(&mut sim, 1);
    for &k in &knives {
        assert!(obj(&sim, k).y > LEDGE_Y + 4);
    }
    step(&mut sim, 200);
    for &k in &knives {
        assert!(sim.objects().get(k).is_none());
    }
}

#[test]
fn the_wind_blows_278_ticks_pushing_claw_and_blowing_debris_every_6() {
    let mut sim = arena_sim(with_helpers(), (37600, CLAW_SAND_Y), 7);
    act(&mut sim, redtail::wind_entry);
    assert_eq!(st(&sim), Some(EnemyState::WindAttack));
    assert!(sim.redtail_link().wind);
    let w = find(&sim, wind::NAMES[0])[0];
    // The wind starts in the next tick here (it runs after him), pushes
    // from the one after; Claw moves from the one after that.
    step(&mut sim, 1);
    assert_eq!(obj(&sim, w).state, wind::state::BLOW);
    assert_eq!(sim.claw().conveyor_force, 0.0);
    step(&mut sim, 1);
    assert_eq!(sim.claw().conveyor_force, wind::PUSH);
    assert_eq!(find(&sim, debris::NAMES[0]).len(), 1);
    let x = claw_xy(&sim).0;
    step(&mut sim, 1);
    assert_eq!(claw_xy(&sim).0, x - 5);
    for k in 1..=12 {
        step(&mut sim, 1);
        assert_eq!(
            find(&sim, debris::NAMES[0]).len(),
            1 + (k + 1) / 6,
            "tick {k}"
        );
    }
    // 278 ticks of it; its end lowers the flag and lets Claw go.
    step(&mut sim, 278 - 15);
    assert_eq!(obj(&sim, w).state, wind::state::BLOW);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, w).state, wind::state::IDLE);
    assert!(!sim.redtail_link().wind);
    assert_eq!(sim.claw().conveyor_force, 0.0);
}

#[test]
fn the_wind_only_blows_from_the_ledge_at_a_claw_on_the_sand_and_stops_with_the_flag() {
    // Claw on the ledge: the ranged choice instead (no wind).
    let mut sim = arena_sim(with_helpers(), (AT.0 - 150, CLAW_LEDGE_Y), 7);
    act(&mut sim, redtail::wind_entry);
    assert!(!sim.redtail_link().wind);
    assert_ne!(st(&sim), Some(EnemyState::WindAttack));
    // Lowered (as his reset does), the blow ends on the wind's next tick.
    let mut sim = arena_sim(with_helpers(), (37600, CLAW_SAND_Y), 7);
    act(&mut sim, redtail::wind_entry);
    step(&mut sim, 30);
    let w = find(&sim, wind::NAMES[0])[0];
    assert_eq!(obj(&sim, w).state, wind::state::BLOW);
    sim.with_world(|w| w.redtail_link_mut().wind = false);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, w).state, wind::state::IDLE);
    assert_eq!(sim.claw().conveyor_force, 0.0);
}

#[test]
fn the_stakes_hurt_claw_for_20() {
    let mut sim = arena_sim(with_helpers(), (STAKES.0 + 20, CLAW_SAND_Y), 7);
    step(&mut sim, 2);
    assert_eq!(claw_health(&sim), 100 - spikes::DAMAGE);
    let s = find(&sim, spikes::NAMES[0])[0];
    let o = obj(&sim, s);
    assert_eq!(o.attack_rect, spikes::ATTACK_RECT);
    assert_eq!(o.type_flags, types::ENEMY);
    assert!(!o.hit_rect.is_set(), "nothing hits them");
}

// ----------------------------------------------------------------------
// The jumps

#[test]
fn he_jumps_down_to_the_sand_and_up_to_the_ledge_landing_with_limits() {
    let mut sim = arena_sim(with_helpers(), (37200, CLAW_SAND_Y), 7);
    let id = boss(&sim);
    sim.with_world(|w| redtail::change_level(w, id, 0));
    assert_eq!(st(&sim), Some(EnemyState::JumpDown));
    assert_eq!(obj(&sim, id).physics_type, ptype::DIRECT);
    let k = step_until(&mut sim, EnemyState::Home, 60);
    assert!((20..=40).contains(&k), "{k}");
    assert_eq!(pos(&sim).1, SAND_Y);
    // The landing's limits; the attack he jumped for was Home.
    let bounds = enemy::data_of(obj(&sim, id)).map(|d| d.bounds);
    assert_eq!(bounds, Some(redtail::LANDED_BOUNDS));
    assert_eq!(rd(&sim).home.len, redtail::HOME_MS);
    let x = pos(&sim).0;
    assert!((redtail::JUMP_DOWN_X..=redtail::JUMP_DOWN_X + 300).contains(&x));
    // Up again: lands on the ledge.
    sim.with_world(|w| redtail::change_level(w, id, 0));
    assert_eq!(st(&sim), Some(EnemyState::JumpUp));
    step_until(&mut sim, EnemyState::Home, 80);
    let (x, y) = pos(&sim);
    assert_eq!(y, LEDGE_Y, "on the ledge");
    let ledge = (LEDGE_COLS.start as i32 * TILE)..(LEDGE_COLS.end as i32 * TILE);
    assert!(ledge.contains(&x), "{x}");
}

#[test]
fn the_fall_guard_catches_him_below_the_sand() {
    let mut sim = arena_sim(with_helpers(), SAND_FAR, 7);
    let id = boss(&sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.y = redtail::FALL_GUARD_Y + 5;
            o.x = 37850;
        }
    });
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(pos(&sim).1, redtail::FALL_GUARD_TO);
    assert_eq!(obj(&sim, id).physics_type, ptype::FLY);
}

// ----------------------------------------------------------------------
// Death

#[test]
fn dying_on_the_ledge_he_jumps_down_falls_says_his_line_and_the_gem_rises() {
    let mut sim = arena_sim(with_helpers(), (AT.0 - 60, CLAW_LEDGE_Y), 7);
    let id = boss(&sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health = 5;
        }
    });
    teleported_count(&mut sim);
    blow(&mut sim, 5);
    assert_eq!(obj(&sim, id).health, 0);
    assert_eq!(st(&sim), Some(EnemyState::JumpDown));
    assert_eq!(rd(&sim).pending, 6);
    step_until(&mut sim, EnemyState::Dead, 60);
    assert_eq!(pos(&sim).1, SAND_Y);
    let o = obj(&sim, id);
    assert_ne!(o.flags & flags::HARMLESS, 0);
    assert_ne!(o.flags & flags::SAFE, 0);
    // The fall (121 calls) and the line (132 ticks) end before the gem;
    // the gem 136 ticks after the fall began.
    let mut k = 0;
    while obj(&sim, claw_id(&sim)).state != BOSS_END && k < 200 {
        step(&mut sim, 1);
        k += 1;
    }
    assert_eq!(k, 136);
    assert!(rd(&sim).defeated);
    assert!(!find(&sim, pickup::END_GEM).is_empty());
    step(&mut sim, 300);
    assert_eq!(st(&sim), Some(EnemyState::Dead), "never removed");
    assert_eq!(sim.boss().current(), Some(id));
}

#[test]
fn dying_with_claw_dead_he_starts_over_at_claws_respawn() {
    let mut sim = arena_sim(with_helpers(), (37200, CLAW_SAND_Y), 7);
    on_sand(&mut sim, 37850);
    let id = boss(&sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health = 0;
        }
        redtail::die(w, id);
    });
    assert_eq!(st(&sim), Some(EnemyState::Dead));
    sim.with_world(|w| w.claw.stats.health = 0);
    step(&mut sim, 150);
    assert!(!rd(&sim).defeated);
    assert!(find(&sim, pickup::END_GEM).is_empty());
    // Claw's respawn: the reset, full health, Home on the ledge.
    sim.with_world(|w| {
        w.claw.stats.health = 100;
        w.claw.stats.attempt += 1;
    });
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    assert_eq!(obj(&sim, id).health, redtail::HEALTH);
    assert_eq!(pos(&sim).0, AT.0);
    // His fall's flag that he hurts nothing stays.
    assert_ne!(obj(&sim, id).flags & flags::HARMLESS, 0);
}

#[test]
fn a_damaging_blow_in_the_death_jump_leaves_him_fighting_until_the_next_one() {
    let mut sim = arena_sim(with_helpers(), (AT.0 - 60, CLAW_LEDGE_Y), 7);
    let id = boss(&sim);
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.health = 5;
        }
    });
    teleported_count(&mut sim);
    blow(&mut sim, 5);
    assert_eq!(st(&sim), Some(EnemyState::JumpDown));
    // After the blow's cooldown, another (not the third counted) blow in
    // the air: the flinch takes the jump's place and the death waits.
    step(&mut sim, 17);
    sim.with_world(|w| {
        if let Some(ObjectData::RedTail(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            d.hits = 0;
        }
    });
    blow(&mut sim, 5);
    assert_eq!(st(&sim), Some(EnemyState::HitLow));
    assert_eq!(rd(&sim).pending, 6, "kept, never made");
    // Out of the jump he falls through the sand to the fall guard, then
    // goes Home and fights on with no health.
    step_until(&mut sim, EnemyState::Home, 60);
    step(&mut sim, 30);
    assert_ne!(st(&sim), Some(EnemyState::Dead));
    assert!(obj(&sim, id).health <= 0);
    // The next blow that gets through runs his death again.
    sim.with_world(|w| {
        if let Some(o) = w.objects.get_mut(id) {
            o.x = 37850;
            o.y = SAND_Y;
            o.physics_type = ptype::STAND;
        }
        if let Some(ObjectData::RedTail(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            d.hits = 0;
        }
    });
    step(&mut sim, 1);
    blow(&mut sim, 5);
    assert_eq!(st(&sim), Some(EnemyState::Dead));
}

#[test]
fn claws_respawn_takes_his_knives_and_flares_away() {
    let mut sim = arena_sim(with_helpers(), (37200, CLAW_SAND_Y), 7);
    act(&mut sim, redtail::knives_entry);
    step(&mut sim, 18);
    let knives = find(&sim, knife::NAMES[0]);
    assert_eq!(knives.len(), 3);
    sim.with_world(|w| w.claw.stats.attempt += 1);
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Home));
    step(&mut sim, 1);
    for k in knives {
        assert!(sim.objects().get(k).is_none());
    }
}
