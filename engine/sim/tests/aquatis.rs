//! Aquatis and his helpers on a synthetic floor with the twelfth level's
//! animation timings: how he is built (the current boss, hanging where he
//! is placed, his box, liftable, waiting for the fight), his body that
//! hurts while he waits, a tentacle that sees him current and sways, the
//! fight flag's reset and his wait for every helper's reset bit, the idle
//! poses (only the first leads to a punch) and the punch boxes, cowering
//! through a quake, a stalactite's hit,
//! the reel and the frenzy of alternating punches, the fifth stalactite,
//! his death and the gem, Claw's blows that only make him flinch (three
//! in a row and he cowers), the stalactites lost while he flinches,
//! reels or thrashes or within the cooldown of a blow, his death with
//! Claw dead and the respawn that restarts the fight with a harmless
//! body, the lift that leaves him where he hangs; the crack (its blast,
//! its gates, its cooldown, the quake's end arming the next stalactite),
//! the five stalactites (the order of their falls, the landing height,
//! the crumble, the reset), the dynamite helper (the first stick, the
//! refill after it is taken, its gate, the count Claw carries), and the
//! gem's catch-up on the return from a help screen.

mod common;

use std::sync::Arc;

use claw_frame::TickInput;
use claw_sim::activation::ActivationConfig;
use claw_sim::anim::Animation;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::aquatis::{self, crack, dynamite, stalactite as stal};
use claw_sim::logic::enemy::motion::{DEPTH_BEHIND, STEP_LIMIT};
use claw_sim::logic::enemy::{EnemyState, Timer, tentacle};
use claw_sim::logic::world_boss::fight;
use claw_sim::logic::{ObjectData, new_object, pickup, stalactite as cave};
use claw_sim::object::{LogicKind, Object, ObjectId, flags, special, types};
use claw_sim::player::special::BOSS_END;
use claw_sim::{Sim, SimConfig, SimLevel};
use common::*;

// ----------------------------------------------------------------------
// The arena

/// Where Aquatis hangs: his box's bottom just above the floor (959).
const AT: (i32, i32) = (3600, 860);
/// Where the crack is placed, right of him.
const CRACK: (i32, i32) = (3740, 780);
/// Where the dynamite helper is placed, left of him.
const HELPER: (i32, i32) = (2940, 750);
/// The five stalactites over him, (x, order of their fall).
const STALS: [(i32, i32); 5] = [(3546, 4), (3584, 2), (3624, 1), (3660, 3), (3697, 5)];
/// Their height.
const STAL_Y: i32 = 610;

fn anims() -> Vec<Arc<Animation>> {
    let mut v = vec![
        Animation::once(
            aquatis::IDLE_ANIM,
            &[
                (100, 100),
                (101, 100),
                (102, 200),
                (101, 300),
                (102, 150),
                (101, 70),
                (100, 100),
                (200, 0),
            ],
        ),
        Animation::once(
            aquatis::IDLE2_ANIM,
            &[
                (103, 90),
                (104, 90),
                (105, 90),
                (106, 90),
                (105, 90),
                (104, 90),
                (103, 92),
                (104, 92),
                (105, 92),
                (106, 106),
            ],
        ),
        Animation::looping(aquatis::SCARED_ANIM, &[(107, 500), (108, 500), (109, 370)]),
        Animation::once(
            aquatis::HIGH_PUNCH_ANIM,
            &[(201, 90), (202, 50), (203, 50), (204, 200)],
        ),
        Animation::once(
            aquatis::LOW_PUNCH_ANIM,
            &[(205, 100), (206, 50), (207, 200)],
        ),
        Animation::once(
            aquatis::HIT_HIGH_ANIM,
            &[(500, 50), (501, 200), (500, 50), (200, 0)],
        ),
        Animation::once(
            aquatis::HIT_LOW_ANIM,
            &[(600, 50), (601, 200), (600, 50), (200, 0)],
        ),
        Animation::once(
            aquatis::DEATH_ANIM,
            &[
                (500, 200),
                (501, 200),
                (500, 200),
                (950, 200),
                (951, 200),
                (952, 200),
                (953, 200),
                (954, 200),
                (955, 200),
                (956, 152),
            ],
        ),
        Animation::looping(crack::GLOW_ANIM, &[(1, 500), (2, 300), (3, 300), (2, 300)]),
        Animation::looping(cave::HOME_ANIM, &[(1, 1000)]),
        Animation::once(
            cave::CRUMBLE_ANIM,
            &[(2, 50), (3, 50), (4, 50), (5, 50), (6, 50), (7, 50)],
        ),
    ];
    for key in claw_sim::player::special::HIT_ANIMS {
        v.push(Animation::once(key, &[(200, 150), (201, 200)]));
    }
    v.push(Animation::looping("CLAW_STAND", &[(11, 110), (12, 110)]));
    v.push(Animation::looping("CLAW_WALK", &[(20, 100), (21, 100)]));
    v.push(Animation::looping("CLAW_FALL", &[(278, 65), (280, 80)]));
    v.into_iter().map(Arc::new).collect()
}

/// The flat floor as the twelfth level with Aquatis and `more`.
fn arena(more: Vec<ObjectSpec>) -> SimLevel {
    let mut level = flat_level();
    level.number = 12;
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    let mut boss = ObjectSpec::at(aquatis::NAMES[0], AT.0, AT.1, 0);
    // The gem's target: left of him, above the floor.
    boss.record[55] = 3300;
    boss.record[56] = 700;
    level.objects = vec![boss];
    level.objects.extend(more);
    level
}

fn helpers() -> Vec<ObjectSpec> {
    let mut v = vec![ObjectSpec::at(crack::NAMES[0], CRACK.0, CRACK.1, -10)];
    for (x, order) in STALS {
        let mut s = ObjectSpec::at(stal::NAMES[0], x, STAL_Y, 1000);
        s.record[17] = order;
        v.push(s);
    }
    let mut d = ObjectSpec::at(dynamite::NAMES[0], HELPER.0, HELPER.1, 0);
    d.record[51] = -500;
    d.record[52] = -300;
    d.record[53] = 1250;
    d.record[54] = 500;
    v.push(d);
    v
}

/// The arena awake everywhere, Claw put at `claw` on the first tick.
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

/// Claw on the floor far left of Aquatis, out of his punches' reach.
const FAR: (i32, i32) = (2000, 899);
/// Claw on the floor 150 px left of him: within his punches' range boxes,
/// out of his body's and his punches' reach.
const NEAR: (i32, i32) = (AT.0 - 150, 899);

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    let mut v: Vec<(ObjectId, i32)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, o)| (id, o.editor.0))
        .collect();
    v.sort_by_key(|&(_, x)| x);
    v.into_iter().map(|(id, _)| id).collect()
}

fn boss(sim: &Sim) -> ObjectId {
    find(sim, aquatis::NAMES[0])[0]
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object exists")
}

fn st(sim: &Sim) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, boss(sim)).state)
}

fn ad(sim: &Sim) -> aquatis::AquatisData {
    match &obj(sim, boss(sim)).data {
        ObjectData::Aquatis(d) => d.clone(),
        d => panic!("Aquatis's data: {d:?}"),
    }
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

/// Clears every reset bit, as the helpers do once they have reset.
fn clear_bits(sim: &mut Sim) {
    sim.with_world(|w| w.aquatis_link_mut().reset_bits = 0);
}

/// Starts the fight (the flag at 2) and lets the reset run.
fn start_fight(sim: &mut Sim) {
    sim.with_world(|w| w.set_boss_fight(fight::ON));
    step(sim, 1);
}

/// An object of the enemies' kind to stand for a falling stalactite: a
/// cave stalactite placed far away.
fn spike(sim: &mut Sim) -> ObjectId {
    sim.with_world(|w| {
        let mut o = new_object(cave::NAMES[0], 100, 100, 0);
        o.type_flags = types::ENEMY;
        w.spawn(o)
    })
}

/// Lets Aquatis's hit cooldown pass.
fn cool_down(sim: &mut Sim) {
    let id = boss(sim);
    sim.with_world(|w| {
        if let Some(ObjectData::Aquatis(d)) = w.objects.get_mut(id).map(|o| &mut o.data) {
            d.hit_cooldown = Timer::default();
        }
    });
}

fn hit(sim: &mut Sim, by: ObjectId) {
    let id = boss(sim);
    sim.with_world(|w| aquatis::on_hit(w, id, by));
}

// ----------------------------------------------------------------------
// Building and waiting

#[test]
fn he_is_built_hanging_as_the_current_boss_waiting_for_the_fight() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    let id = boss(&sim);
    assert_eq!(sim.boss().current(), Some(id));
    let link = *sim.aquatis_link();
    assert_eq!(link.aquatis, Some(id));
    assert!(link.alive && link.refill_outstanding);
    assert_eq!(link.reset_bits, 0);
    assert_eq!(link.land_y, AT.1 - aquatis::LAND_ABOVE);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    let o = obj(&sim, id);
    assert_eq!((o.health, o.damage), (aquatis::HEALTH, aquatis::DAMAGE));
    assert_eq!(o.move_rect, aquatis::BOX);
    assert_eq!(o.hit_rect, aquatis::BOX);
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.hit_type_flags, aquatis::HIT_MASK);
    assert_ne!(o.flags & flags::ALWAYS_ACTIVE, 0);
    step(&mut sim, 300);
    let o = obj(&sim, id);
    assert_eq!((o.x, o.y), AT, "never moves by himself");
    assert_ne!(o.special_flags & special::LIFTABLE, 0, "liftable");
    assert_eq!(o.attack_rect, aquatis::BOX, "his body hurts");
    // Claw's feet (959) are below his (956): drawn behind Claw.
    assert_eq!(o.z, 10 + DEPTH_BEHIND);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
}

#[test]
fn built_he_is_current_so_a_tentacle_sways_while_he_waits() {
    // A tentacle far left of Claw: he stands right of it, out of its
    // attacks.
    let mut level = arena(vec![ObjectSpec::at(tentacle::NAMES[0], 1000, 900, 0)]);
    let sway = Animation::looping(
        tentacle::IDLE_ANIM,
        &[(150, 150), (151, 150), (152, 150), (153, 150)],
    );
    level.animations.insert(sway.key.clone(), Arc::new(sway));
    let (w, h) = (level.map.width() as u32, level.map.height() as u32);
    let mut sim = Sim::new(
        level,
        SimConfig {
            seed: 7,
            activation: ActivationConfig::widened((2 * w, 2 * h)),
            ..SimConfig::default()
        },
    );
    sim.teleport_player(FAR.0, FAR.1);
    sim.step(TickInput::default());
    let t = find(&sim, tentacle::NAMES[0])[0];
    let mut frames = vec![obj(&sim, t).frame];
    for _ in 0..60 {
        sim.step(TickInput::default());
        frames.push(obj(&sim, t).frame);
    }
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert!(frames.windows(2).any(|f| f[0] != f[1]), "{frames:?}");
}

#[test]
fn waiting_for_the_fight_he_never_punches_but_his_body_hurts() {
    let mut sim = arena_sim(Vec::new(), NEAR, 7);
    step(&mut sim, 400);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
    assert_eq!(claw_health(&sim), 100);
    // Into his body.
    sim.teleport_player(AT.0 - 70, 899);
    step(&mut sim, 2);
    assert_eq!(claw_health(&sim), 100 - aquatis::DAMAGE);
    assert_eq!(st(&sim), Some(EnemyState::Startup));
}

#[test]
fn the_fight_flag_resets_him_and_he_waits_for_every_helper() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    sim.with_world(|w| w.aquatis_link_mut().refill_outstanding = true);
    start_fight(&mut sim);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    let link = *sim.aquatis_link();
    assert_eq!(link.reset_bits, aquatis::RESET_BITS);
    assert!(!link.refill_outstanding);
    // Frozen while a bit is set: his pose does not move.
    let frame = obj(&sim, boss(&sim)).frame;
    step(&mut sim, 200);
    assert_eq!(obj(&sim, boss(&sim)).frame, frame);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    // Every bit cleared: the pose plays and he goes idle again at its end.
    clear_bits(&mut sim);
    let rng = sim.rng_state();
    step(&mut sim, 70);
    assert_ne!(sim.rng_state(), rng, "going idle again draws");
}

// ----------------------------------------------------------------------
// Idle and the punches

#[test]
fn only_his_first_pose_leads_to_a_punch_with_its_box_on_its_frame() {
    for seed in 1..30 {
        let mut sim = arena_sim(Vec::new(), NEAR, seed);
        start_fight(&mut sim);
        clear_bits(&mut sim);
        let pose = |sim: &Sim| {
            obj(sim, boss(sim))
                .anim
                .animation()
                .map(|a| a.key.clone())
                .unwrap_or_default()
        };
        let mut last = pose(&sim);
        let mut calls = 0;
        loop {
            step(&mut sim, 1);
            calls += 1;
            match st(&sim) {
                Some(EnemyState::Idle) if calls < 3000 => {
                    // The second pose ends before his idle time is over:
                    // he goes idle again, drawing his next pose.
                    last = pose(&sim);
                    continue;
                }
                Some(s @ (EnemyState::HighPunch | EnemyState::LowPunch)) => {
                    // From the first pose, after his idle time.
                    assert_eq!(last, aquatis::IDLE_ANIM, "seed {seed}");
                    assert!(calls >= 1000 / 18, "seed {seed}: {calls}");
                    let (frame, punch) = if s == EnemyState::HighPunch {
                        (aquatis::HIGH_PUNCH_FRAME, aquatis::HIGH_PUNCH_BOX)
                    } else {
                        (aquatis::LOW_PUNCH_FRAME, aquatis::LOW_PUNCH_BOX)
                    };
                    let mut armed = false;
                    while st(&sim) == Some(s) {
                        let o = obj(&sim, boss(&sim));
                        if o.frame == frame {
                            assert_eq!(o.attack_rect, punch);
                            armed = true;
                        } else {
                            assert_eq!(o.attack_rect, aquatis::BOX);
                        }
                        step(&mut sim, 1);
                    }
                    assert!(armed, "seed {seed}");
                    assert_eq!(st(&sim), Some(EnemyState::Idle));
                }
                other => panic!("seed {seed}: {other:?}"),
            }
            break;
        }
    }
}

#[test]
fn a_quake_scares_him_until_it_and_the_falling_stalactite_end() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    sim.with_world(|w| w.aquatis_link_mut().quake = true);
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Scared));
    // At least 4000 ms, and while the quake runs.
    step(&mut sim, 300);
    assert_eq!(st(&sim), Some(EnemyState::Scared));
    sim.with_world(|w| {
        let link = w.aquatis_link_mut();
        link.quake = false;
        link.falling = true;
    });
    step(&mut sim, 5);
    assert_eq!(st(&sim), Some(EnemyState::Scared));
    sim.with_world(|w| w.aquatis_link_mut().falling = false);
    step(&mut sim, 1);
    assert_eq!(st(&sim), Some(EnemyState::Idle));
}

// ----------------------------------------------------------------------
// Stalactites, the frenzy, his death and the gem

#[test]
fn a_stalactite_reels_him_and_starts_his_frenzy_of_alternating_punches() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    let spike = spike(&mut sim);
    hit(&mut sim, spike);
    assert_eq!(st(&sim), Some(EnemyState::StalactiteHit));
    assert_eq!(obj(&sim, boss(&sim)).health, 80);
    assert_eq!(ad(&sim).stalactites, 1);
    // A second stalactite while he reels is lost.
    cool_down(&mut sim);
    hit(&mut sim, spike);
    assert_eq!(obj(&sim, boss(&sim)).health, 80);
    // The reel: his second pose, then the frenzy.
    let mut reel = 0;
    while st(&sim) == Some(EnemyState::StalactiteHit) {
        step(&mut sim, 1);
        reel += 1;
        assert!(reel < 80);
    }
    assert!((50..=56).contains(&reel), "{reel}");
    assert_eq!(st(&sim), Some(EnemyState::SuperFrenzy));
    assert!(sim.aquatis_link().frenzy);
    // Nothing reaches him in his frenzy.
    cool_down(&mut sim);
    hit(&mut sim, spike);
    assert_eq!(obj(&sim, boss(&sim)).health, 80);
    // Punches, high first, then alternating, to the first punch end after
    // 4000 ms.
    let mut punches = Vec::new();
    let mut calls = 0;
    while sim.aquatis_link().frenzy {
        step(&mut sim, 1);
        calls += 1;
        let s = st(&sim);
        if matches!(s, Some(EnemyState::HighPunch | EnemyState::LowPunch))
            && punches.last() != s.as_ref()
        {
            punches.push(s.unwrap());
        }
        assert!(calls < 400);
    }
    assert!(calls >= 4000 / 18, "{calls}");
    assert_eq!(punches[0], EnemyState::HighPunch);
    assert!(punches.windows(2).all(|p| p[0] != p[1]), "{punches:?}");
    assert!(punches.len() >= 8, "{punches:?}");
    assert_eq!(st(&sim), Some(EnemyState::Idle));
}

#[test]
fn the_fifth_stalactite_kills_him_and_the_gem_rises_out_of_him() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    let spike = spike(&mut sim);
    for n in 1..=4 {
        hit(&mut sim, spike);
        assert_eq!(obj(&sim, boss(&sim)).health, 100 - 20 * n);
        sim.with_world(|w| w.aquatis_link_mut().frenzy = false);
        let id = boss(&sim);
        sim.with_world(|w| aquatis::go_idle(w, id));
        cool_down(&mut sim);
    }
    hit(&mut sim, spike);
    assert_eq!(st(&sim), Some(EnemyState::DeadAct1));
    let o = obj(&sim, boss(&sim));
    assert_eq!(o.health, 0);
    assert!(!o.attack_rect.is_set(), "he hurts nothing");
    assert_ne!(o.flags & flags::HARMLESS, 0);
    // The death plays out; then Claw is held and the gem rises.
    let mut calls = 0;
    while sim.aquatis_link().alive {
        step(&mut sim, 1);
        calls += 1;
        assert!(calls < 200);
    }
    assert!((100..=120).contains(&calls), "{calls}");
    assert_eq!(sim.player().unwrap().state, BOSS_END);
    let gems = find(&sim, pickup::END_GEM);
    assert_eq!(gems.len(), 1);
    assert_eq!((obj(&sim, gems[0]).x, obj(&sim, gems[0]).y), AT);
    // He stays, the current boss, on his last picture.
    step(&mut sim, 20);
    assert_eq!(st(&sim), Some(EnemyState::DeadAct1));
    assert_eq!(sim.boss().current(), Some(boss(&sim)));
}

#[test]
fn claws_blows_only_make_him_flinch_and_three_make_him_cower() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    let claw = claw_id(&sim);
    for n in 1..=3 {
        hit(&mut sim, claw);
        assert!(
            matches!(st(&sim), Some(EnemyState::HitLow | EnemyState::HitHigh)),
            "blow {n}"
        );
        assert_eq!(obj(&sim, boss(&sim)).health, 100);
        // Flinching, he is not hit again.
        cool_down(&mut sim);
        hit(&mut sim, claw);
        assert_eq!(obj(&sim, boss(&sim)).health, 100);
        while matches!(st(&sim), Some(EnemyState::HitLow | EnemyState::HitHigh)) {
            step(&mut sim, 1);
        }
        if n < 3 {
            // Back to idle (or a punch): wait for idle again.
            let mut k = 0;
            while st(&sim) != Some(EnemyState::Idle) {
                step(&mut sim, 1);
                k += 1;
                assert!(k < 100);
            }
            cool_down(&mut sim);
        }
    }
    assert_eq!(st(&sim), Some(EnemyState::Scared));
    assert_eq!(ad(&sim).flinches, 0);
    // Cowering, Claw's blows do nothing; a stalactite still hits him.
    cool_down(&mut sim);
    hit(&mut sim, claw);
    assert_eq!(st(&sim), Some(EnemyState::Scared));
    let spike = spike(&mut sim);
    hit(&mut sim, spike);
    assert_eq!(st(&sim), Some(EnemyState::StalactiteHit));
}

#[test]
fn a_stalactite_is_lost_within_a_blows_cooldown_and_a_pistol_shot_is_used_up() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    let claw = claw_id(&sim);
    hit(&mut sim, claw);
    // The flinch ends before the 500 ms cooldown does.
    while matches!(st(&sim), Some(EnemyState::HitLow | EnemyState::HitHigh)) {
        step(&mut sim, 1);
    }
    let spike = spike(&mut sim);
    hit(&mut sim, spike);
    assert_eq!(obj(&sim, boss(&sim)).health, 100, "lost in the cooldown");
    step(&mut sim, 30);
    hit(&mut sim, spike);
    assert_eq!(obj(&sim, boss(&sim)).health, 80, "after the cooldown");
    // A pistol shot that gets past the gates is used up.
    let id = boss(&sim);
    sim.with_world(|w| {
        aquatis::go_idle(w, id);
        w.aquatis_link_mut().frenzy = false;
    });
    cool_down(&mut sim);
    let shot = sim.with_world(|w| {
        let mut o = new_object("PistolShotStandIn", 100, 100, 0);
        o.type_flags = types::PISTOL_SHOT;
        w.spawn(o)
    });
    hit(&mut sim, shot);
    assert!(matches!(
        st(&sim),
        Some(EnemyState::HitLow | EnemyState::HitHigh)
    ));
    assert_ne!(obj(&sim, shot).flags & flags::DESTROY, 0);
}

#[test]
fn dying_with_claw_dead_he_restarts_the_fight_at_his_respawn_with_a_harmless_body() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    start_fight(&mut sim);
    clear_bits(&mut sim);
    let spike = spike(&mut sim);
    for _ in 0..4 {
        hit(&mut sim, spike);
        sim.with_world(|w| w.aquatis_link_mut().frenzy = false);
        let id = boss(&sim);
        sim.with_world(|w| aquatis::go_idle(w, id));
        cool_down(&mut sim);
    }
    hit(&mut sim, spike);
    assert_eq!(st(&sim), Some(EnemyState::DeadAct1));
    // Claw has no health left as the death ends.
    step(&mut sim, 90);
    sim.with_world(|w| w.claw.stats.health = 0);
    let mut k = 0;
    while !ad(&sim).death_done {
        step(&mut sim, 1);
        k += 1;
        assert!(k < 100);
    }
    assert!(sim.aquatis_link().alive, "no defeat with Claw dead");
    assert!(find(&sim, pickup::END_GEM).is_empty());
    assert_eq!(st(&sim), Some(EnemyState::DeadAct1));
    // Claw loses the life.
    sim.with_world(|w| w.claw.stats.health = 100);
    sim.end_life();
    // His respawn restarts the fight: full health, idle, every bit set;
    // what his death took stays.
    let mut k = 0;
    while obj(&sim, boss(&sim)).health != 100 {
        step(&mut sim, 1);
        k += 1;
        assert!(k < 600, "never reset");
    }
    assert_eq!(st(&sim), Some(EnemyState::Idle));
    assert_eq!(ad(&sim).stalactites, 0);
    let o = obj(&sim, boss(&sim));
    assert_ne!(o.flags & flags::HARMLESS, 0);
    assert!(!o.attack_rect.is_set());
}

#[test]
fn lifted_he_stays_where_he_hangs_until_claw_lets_go() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    let id = boss(&sim);
    let claw = claw_id(&sim);
    sim.with_world(|w| aquatis::lifted(w, id, claw));
    assert_eq!(st(&sim), Some(EnemyState::Lifted));
    let o = obj(&sim, id);
    assert_eq!((o.x, o.y), AT);
    assert_eq!(o.move_rect, aquatis::CARRIED_MOVE_RECT);
}

// ----------------------------------------------------------------------
// The crack

/// A blast of the level's kind, far away.
fn blast(sim: &mut Sim) -> ObjectId {
    sim.with_world(|w| {
        let mut o = new_object("BlastStandIn", 100, 100, 0);
        o.type_flags = types::LEVEL_SHOT;
        w.spawn(o)
    })
}

fn crack_hit(sim: &mut Sim) {
    let c = find(sim, crack::NAMES[0])[0];
    let b = blast(sim);
    sim.with_world(|w| crack::on_hit(w, c, b));
}

fn crack_state(sim: &Sim) -> i32 {
    obj(sim, find(sim, crack::NAMES[0])[0]).state
}

#[test]
fn a_blast_on_the_crack_starts_a_quake_that_arms_the_next_stalactite() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    step(&mut sim, 2);
    let c = find(&sim, crack::NAMES[0])[0];
    assert_eq!(obj(&sim, c).hit_type_flags, types::LEVEL_SHOT);
    assert_eq!(obj(&sim, c).hit_rect, crack::HIT_RECT);
    assert_eq!(crack_state(&sim), crack::state::IDLE);
    let rng = sim.rng_state();
    crack_hit(&mut sim);
    assert_eq!(crack_state(&sim), crack::state::DELAY);
    assert!(sim.aquatis_link().quake);
    assert_ne!(sim.rng_state(), rng, "the rumble's draw");
    // A second blast during the quake does nothing.
    crack_hit(&mut sim);
    assert_eq!(crack_state(&sim), crack::state::DELAY);
    // 4000 ms later the quake ends and the next stalactite is armed: the
    // first in order falls on that same tick.
    let mut k = 0;
    while sim.aquatis_link().quake {
        step(&mut sim, 1);
        k += 1;
        assert!(k < 300);
    }
    // A blast made between two ticks comes at the next tick's time: the
    // quake ends 223 ticks after that tick.
    assert_eq!(k, 1 + 4000usize.div_ceil(18));
    assert_eq!(crack_state(&sim), crack::state::IDLE);
    let link = *sim.aquatis_link();
    assert!(link.falling && !link.armed);
    assert_eq!(link.stalactite_order, 2);
    // The crack's cooldown: 6000 ms from its last quake.
    crack_hit(&mut sim);
    assert_eq!(crack_state(&sim), crack::state::IDLE);
    assert!(!sim.aquatis_link().quake);
}

#[test]
fn the_crack_waits_while_he_thrashes_or_a_stalactite_is_armed_or_falls() {
    for gate in 0..4 {
        let mut sim = arena_sim(helpers(), FAR, 7);
        step(&mut sim, 2);
        sim.with_world(|w| {
            let link = w.aquatis_link_mut();
            match gate {
                0 => link.frenzy = true,
                1 => link.armed = true,
                2 => link.falling = true,
                _ => link.alive = false,
            }
        });
        let rng = sim.rng_state();
        crack_hit(&mut sim);
        assert_eq!(crack_state(&sim), crack::state::IDLE, "gate {gate}");
        assert!(!sim.aquatis_link().quake);
        assert_eq!(sim.rng_state(), rng);
    }
}

#[test]
fn each_helper_clears_its_own_reset_bit_on_its_next_call() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    step(&mut sim, 2);
    sim.with_world(|w| w.reset_aquatis());
    step(&mut sim, 1);
    // The crack and the five stalactites have reset; no tentacle here.
    let tentacles = aquatis::TENTACLE_BITS.iter().fold(0, |a, b| a | b);
    assert_eq!(sim.aquatis_link().reset_bits, tentacles);
}

// ----------------------------------------------------------------------
// The stalactites

#[test]
fn the_stalactites_fall_one_by_one_in_their_order_and_crumble_above_him() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    step(&mut sim, 2);
    let stals = find(&sim, stal::NAMES[0]);
    assert_eq!(stals.len(), 5);
    // Each its bit, by the order they are built.
    let mut bits: Vec<u32> = stals
        .iter()
        .map(|&s| match &obj(&sim, s).data {
            ObjectData::AquatisStalactite(d) => d.bit,
            _ => 0,
        })
        .collect();
    bits.sort_unstable();
    assert_eq!(bits, aquatis::STALACTITE_BITS);
    let land = sim.aquatis_link().land_y;
    for (round, want) in [1, 2, 3, 4, 5].into_iter().enumerate() {
        let order_of = |sim: &Sim, s: ObjectId| match &obj(sim, s).data {
            ObjectData::AquatisStalactite(d) => d.order,
            _ => 0,
        };
        sim.with_world(|w| w.aquatis_link_mut().armed = true);
        step(&mut sim, 1);
        let falling: Vec<ObjectId> = stals
            .iter()
            .copied()
            .filter(|&s| obj(&sim, s).state == stal::state::FALLING)
            .collect();
        assert_eq!(falling.len(), 1, "round {round}");
        let s = falling[0];
        assert_eq!(order_of(&sim, s), want);
        assert!(sim.aquatis_link().falling && !sim.aquatis_link().armed);
        // It falls through everything to the landing height and crumbles.
        let mut k = 0;
        while obj(&sim, s).state == stal::state::FALLING {
            step(&mut sim, 1);
            k += 1;
            assert!(k < 100);
        }
        assert_eq!(obj(&sim, s).state, stal::state::DEAD);
        assert!(obj(&sim, s).y >= land);
        assert!(!sim.aquatis_link().falling);
        step(&mut sim, 25);
        assert_eq!(obj(&sim, s).state, stal::state::WAITING);
        assert_ne!(obj(&sim, s).draw_flags & 1, 0, "hidden");
    }
    // His reset puts them all back, the order at 1.
    sim.with_world(|w| w.reset_aquatis());
    step(&mut sim, 1);
    for &s in &stals {
        assert_eq!(obj(&sim, s).state, stal::state::IDLE);
        assert_eq!(obj(&sim, s).y, STAL_Y);
    }
    assert_eq!(sim.aquatis_link().stalactite_order, 1);
}

#[test]
fn a_falling_stalactite_hits_him_and_crumbles_anyway() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    start_fight(&mut sim);
    step(&mut sim, 2);
    // Every helper has reset; no tentacle here.
    let tentacles = aquatis::TENTACLE_BITS.iter().fold(0, |a, b| a | b);
    assert_eq!(sim.aquatis_link().reset_bits, tentacles);
    clear_bits(&mut sim);
    sim.with_world(|w| w.aquatis_link_mut().armed = true);
    let mut k = 0;
    while st(&sim) != Some(EnemyState::StalactiteHit) {
        step(&mut sim, 1);
        k += 1;
        assert!(k < 60, "never hit");
    }
    assert_eq!(obj(&sim, boss(&sim)).health, 80);
}

// ----------------------------------------------------------------------
// The dynamite helper

fn sticks(sim: &Sim) -> Vec<ObjectId> {
    find(sim, dynamite::STICK_LOGIC)
        .into_iter()
        .filter(|&s| obj(sim, s).flags & flags::DESTROY == 0)
        .collect()
}

fn helper_hit(sim: &mut Sim) {
    let d = find(sim, dynamite::NAMES[0])[0];
    let b = blast(sim);
    sim.with_world(|w| dynamite::on_hit(w, d, b));
}

#[test]
fn the_dynamite_helper_puts_one_stick_back_after_the_last_is_taken() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    step(&mut sim, 2);
    let d = find(&sim, dynamite::NAMES[0])[0];
    assert_eq!(
        obj(&sim, d).hit_rect,
        claw_sim::object::Rect::new(-500, -300, 1250, 500)
    );
    assert_ne!(obj(&sim, d).draw_flags & 1, 0, "hidden");
    // The first stick, at its own position.
    let first = sticks(&sim);
    assert_eq!(first.len(), 1);
    assert_eq!((obj(&sim, first[0]).x, obj(&sim, first[0]).y), HELPER);
    assert!(sim.aquatis_link().refill_outstanding);
    // A blast while it waits to be taken puts nothing down.
    helper_hit(&mut sim);
    assert_eq!(sticks(&sim).len(), 1);
    // Taken: the flag is cleared.
    let s = first[0];
    sim.with_world(|w| w.destroy(s));
    step(&mut sim, 2);
    assert!(!sim.aquatis_link().refill_outstanding);
    // A blast within the gate's 750 ms of the last does nothing; after
    // it, a new stick.
    helper_hit(&mut sim);
    assert_eq!(sticks(&sim).len(), 0, "within the gate");
    step(&mut sim, 750 / 18 + 1);
    helper_hit(&mut sim);
    step(&mut sim, 1);
    assert_eq!(sticks(&sim).len(), 1);
    assert!(sim.aquatis_link().refill_outstanding);
}

#[test]
fn the_dynamite_helper_refills_only_when_claw_holds_no_more_than_before() {
    let mut sim = arena_sim(helpers(), FAR, 7);
    step(&mut sim, 2);
    let s = sticks(&sim)[0];
    sim.with_world(|w| {
        w.destroy(s);
        w.claw.stats.dynamite += 2;
    });
    step(&mut sim, 2);
    helper_hit(&mut sim);
    step(&mut sim, 1);
    assert!(
        sticks(&sim).is_empty(),
        "Claw holds more sticks than before"
    );
    sim.with_world(|w| w.claw.stats.dynamite -= 2);
    step(&mut sim, 750 / 18 + 1);
    helper_hit(&mut sim);
    step(&mut sim, 1);
    assert_eq!(sticks(&sim).len(), 1);
}

// ----------------------------------------------------------------------
// The help screen's return

#[test]
fn the_flying_gem_catches_up_a_help_screens_time_in_one_capped_step() {
    let mut sim = arena_sim(Vec::new(), FAR, 7);
    let id = boss(&sim);
    sim.with_world(|w| w.boss_defeated(id));
    step(&mut sim, 12);
    let gem = find(&sim, pickup::END_GEM)[0];
    let (x0, y0) = (obj(&sim, gem).x, obj(&sim, gem).y);
    // The return's update pass runs every listed object, which may draw;
    // the gem draws nothing there.
    sim.pass_menu_time(7750);
    let (x1, y1) = (obj(&sim, gem).x, obj(&sim, gem).y);
    let cap = STEP_LIMIT as i32;
    assert!((x1 - x0).abs() <= cap && (x1 - x0).abs() >= cap - 1);
    assert!((y1 - y0) >= cap - 1 && (y1 - y0) <= cap, "moving down");
    // Moving down below its target, it lands where it is on the next tick.
    step(&mut sim, 1);
    let o = obj(&sim, gem);
    assert_ne!(&*o.logic, pickup::END_GEM);
    assert_eq!((o.x, o.y), (x1, y1));
}
