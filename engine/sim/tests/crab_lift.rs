//! Lifting a hermit crab on synthetic levels: the window in which it can
//! be lifted (from going into hiding to its next regroup), Claw's lift,
//! the carried offsets, the throw and what a thrown crab hits, and the
//! drop.

mod common;

use std::sync::Arc;

use claw_sim::anim::Animation;
use claw_sim::container::CollisionEvent;
use claw_sim::level::ObjectSpec;
use claw_sim::logic::enemy::crab::{self, CARRY};
use claw_sim::logic::enemy::{self, EnemyState, field};
use claw_sim::logic::{self, World};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special as sflags, types};
use claw_sim::physics::ptype;
use claw_sim::player::{special, state};
use claw_sim::tiles::TileMap;
use claw_sim::{Buttons, Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The floor's top: a crab stands with its feet on 959, at y 936.
const FLOOR: i32 = 960;
/// A crab's y standing on the floor.
const CRAB_Y: i32 = FLOOR - 1 - crab::MOVE_RECT.bottom;
/// Where Claw starts.
const CLAW_AT: (i32, i32) = (1000, 800);
/// Claw's lift: frame 42 places the crab beside him, frame 43 over his
/// head.
const LIFT_FRAMES: &[(i32, i32)] = &[(40, 100), (42, 100), (43, 100)];

fn anim(key: &str, frames: &[(i32, i32)], once: bool, event: Option<usize>) -> Arc<Animation> {
    let mut a = if once {
        Animation::once(key, frames)
    } else {
        Animation::looping(key, frames)
    };
    if let Some(k) = event {
        a.frames[k].event = 1;
    }
    Arc::new(a)
}

/// The crab's and Claw's animations used here, with the pictures and
/// lengths of the level's and of Claw's.
fn anims() -> Vec<Arc<Animation>> {
    let k = |s: &str| format!("LEVEL_HERMITCRAB_{s}");
    vec![
        anim(&k("CROUCH4"), &[(801, 50), (800, 200)], true, None),
        anim(
            &k("HIDE"),
            &[(802, 750), (801, 500), (802, 200), (801, 750)],
            false,
            None,
        ),
        anim(
            &k("STRIKE1"),
            &[(200, 20), (201, 20), (202, 20), (203, 20), (204, 100)],
            true,
            None,
        ),
        anim(&k("FASTADVANCE"), &[(1, 110), (2, 110)], false, None),
        anim(&k("FALL"), &[(200, 100)], false, None),
        anim(&k("LIFT"), &[(1, 20), (200, 20)], false, None),
        anim(&k("KILLFALL"), &[(950, 0)], true, None),
        anim(special::LIFT_ANIM, LIFT_FRAMES, true, None),
        anim(special::LIFT_LOW_ANIM, LIFT_FRAMES, true, None),
        anim(special::CARRY_ANIM, &[(43, 1000)], false, None),
        anim(
            special::CARRY_WALK_ANIM,
            &[(45, 100), (46, 100)],
            false,
            None,
        ),
        anim(
            special::THROW_ANIM,
            &[(44, 100), (47, 50), (48, 100)],
            true,
            Some(1),
        ),
        anim("CLAW_STAND", &[(11, 110), (12, 110)], false, None),
    ]
}

/// A long flat floor (top at y 960) with Claw starting at [`CLAW_AT`].
fn beach() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(120); 15];
    rows.push("#".repeat(120));
    rows.push("#".repeat(120));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), CLAW_AT);
    for a in anims() {
        level.animations.insert(a.key.clone(), a);
    }
    level
}

/// A crab record at x on the floor, patrolling 300 px either side.
fn hermit(x: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at("HermitCrab", x, 900, 0);
    s.image_set = crab::IMAGE_SET.into();
    s.frame = 1;
    s.record[field::X_MIN] = x - 300;
    s.record[field::X_MAX] = x + 300;
    s
}

/// A game of the beach with crabs at `xs` and Claw landed and still; the
/// built crabs, in record order. The crabs are put into hiding right
/// after they are built, so they do not snip at Claw meanwhile.
fn game(xs: &[i32]) -> (Sim, Vec<ObjectId>) {
    let mut level = beach();
    level.objects.extend(xs.iter().map(|&x| hermit(x)));
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let mut ids: Vec<(i32, ObjectId)> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == "HermitCrab" && o.record.is_some())
        .map(|(id, o)| (o.record.as_ref().unwrap()[5], id))
        .collect();
    ids.sort();
    let ids: Vec<ObjectId> = ids.into_iter().map(|(_, id)| id).collect();
    for &id in &ids {
        assert!(crab::fields_of(obj(&sim, id)).is_some(), "built");
    }
    hide_all(&mut sim, &ids);
    for _ in 0..40 {
        sim.step(TickInput::default());
        hide_all(&mut sim, &ids);
    }
    assert_eq!(claw(&sim).physics_type, ptype::STAND);
    (sim, ids)
}

/// Puts the crabs `ids` that are not hiding into hiding again.
fn hide_all(sim: &mut Sim, ids: &[ObjectId]) {
    for &id in ids {
        if state(sim, id) != Some(EnemyState::Hide) && !busy(sim, id) {
            sim.with_world(|w| crab::enter_hide(w, id));
        }
    }
}

fn busy(sim: &Sim, id: ObjectId) -> bool {
    enemy::data_of(obj(sim, id)).is_some_and(|d| d.busy)
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("the object")
}

fn claw(sim: &Sim) -> &Object {
    sim.player().expect("Claw")
}

fn claw_id(sim: &Sim) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| o.kind == LogicKind::Player)
        .map(|(id, _)| id)
        .expect("Claw")
}

fn state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
    EnemyState::from_id(obj(sim, id).state)
}

fn liftable(sim: &Sim, id: ObjectId) -> bool {
    obj(sim, id).special_flags & sflags::LIFTABLE != 0
}

/// Moves crab `id` to x on the floor, as it stands.
fn put(sim: &mut Sim, id: ObjectId, x: i32) {
    sim.with_world(|w| {
        let o = w.objects.get_mut(id).unwrap();
        o.x = x;
        o.y = CRAB_Y;
    });
}

/// Claw lifts crab `id` standing 40 px in front of him with the lift
/// key; the crab is lifted on that tick.
fn lift_in_front(sim: &mut Sim, id: ObjectId) {
    let x = claw(sim).x + if sim.claw().facing_right { 40 } else { -40 };
    put(sim, id, x);
    assert!(liftable(sim, id));
    sim.step(press(Buttons::LIFT.0));
    assert_eq!(claw(sim).state, state::LIFT);
    assert_eq!(sim.claw().special.lifted, Some(id));
    assert_eq!(state(sim, id), Some(EnemyState::Lifted));
}

#[test]
fn a_crab_is_not_liftable_when_built() {
    let mut level = beach();
    level.objects.push(hermit(1600));
    let mut sim = Sim::new(level, SimConfig::default());
    let mut built = None;
    for _ in 0..5 {
        sim.step(TickInput::default());
        built = sim
            .objects()
            .all()
            .find(|(_, o)| crab::fields_of(o).is_some())
            .map(|(id, _)| id);
        if built.is_some() {
            break;
        }
    }
    let id = built.expect("built");
    assert!(matches!(
        state(&sim, id),
        Some(EnemyState::Regroup | EnemyState::Defense)
    ));
    assert!(!liftable(&sim, id));
}

#[test]
fn a_crab_is_liftable_from_going_into_hiding_until_it_regroups() {
    let mut level = beach();
    level.objects.push(hermit(1600));
    let mut sim = Sim::new(level, SimConfig::default());
    sim.step(TickInput::default());
    let id = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == "HermitCrab")
        .map(|(id, _)| id)
        .expect("the crab");
    // Left to itself for a while: liftable on exactly the ticks it hides,
    // the hiding ticks included, and never otherwise.
    let mut hid = 0;
    for _ in 0..2000 {
        sim.step(TickInput::default());
        let hiding = matches!(state(&sim, id), Some(EnemyState::Hide | EnemyState::Nudge));
        assert_eq!(liftable(&sim, id), hiding, "{:?}", state(&sim, id));
        hid += usize::from(hiding);
    }
    assert!(hid > 0, "it hid");
    // The entries themselves.
    sim.with_world(|w| crab::enter_hide(w, id));
    assert!(liftable(&sim, id));
    sim.with_world(|w| crab::go_home(w, id));
    assert!(!liftable(&sim, id));
    sim.with_world(|w| crab::enter_defense(w, id));
    assert!(!liftable(&sim, id));
}

#[test]
fn a_blow_on_the_crouched_shell_opens_the_window_until_the_knock_regroups() {
    let (mut sim, ids) = game(&[1500]);
    let id = ids[0];
    sim.with_world(|w| {
        crab::go_home(w, id);
        crab::enter_defense(w, id);
        w.objects.get_mut(id).unwrap().frame = crab::CROUCHED_FRAME;
    });
    assert!(!liftable(&sim, id));
    let blow = sim.with_world(|w| {
        let mut o = logic::new_object("TestBlow", 1400, CRAB_Y, 0);
        o.kind = LogicKind::Inert;
        o.type_flags = types::PLAYER;
        o.flags |= flags::NO_COLLIDE;
        w.spawn(o)
    });
    sim.with_world(|w| crab::on_hit(w, id, blow));
    // It hides, then is knocked aside at once: liftable now.
    assert_eq!(state(&sim, id), Some(EnemyState::Nudge));
    assert!(liftable(&sim, id));
    let mut ticks = 0;
    while state(&sim, id) == Some(EnemyState::Nudge) {
        sim.step(TickInput::default());
        ticks += 1;
        assert!(ticks < 10, "the knock ends");
    }
    assert!(ticks <= 2, "{ticks}");
    if state(&sim, id) != Some(EnemyState::Hide) {
        assert!(!liftable(&sim, id));
    }
}

#[test]
fn lifting_a_crab_says_nothing_and_draws_nothing() {
    let (mut sim, ids) = game(&[1600]);
    let id = ids[0];
    let carrier = claw_id(&sim);
    let before = sim.rng().state();
    let cues = sim.with_world(|w| {
        crab::lifted(w, id, carrier);
        w.objects.get(id).map(|o| o.state)
    });
    assert_eq!(sim.rng().state(), before);
    assert_eq!(cues, Some(EnemyState::Lifted as i32));
    let o = obj(&sim, id);
    assert_eq!(o.physics_type, ptype::DIRECT);
    assert_eq!(o.y, CRAB_Y - 1);
    assert_ne!(o.flags & flags::SAFE, 0);
    assert_ne!(o.flags & flags::ALWAYS_ACTIVE, 0);
    assert_eq!(o.move_rect, CARRY.move_rect);
    let d = enemy::data_of(o).unwrap();
    assert!(d.busy && d.frozen);
    assert_eq!(d.bounds, enemy::UNBOUNDED);
    assert_eq!(o.z, claw(&sim).z + 1);
}

#[test]
fn claw_lifts_a_hiding_crab_beside_him_then_over_his_head() {
    let (mut sim, ids) = game(&[1600]);
    let id = ids[0];
    assert!(sim.claw().facing_right);
    lift_in_front(&mut sim, id);
    assert_eq!(claw(&sim).anim.key(), Some(special::LIFT_ANIM));
    let mut beside = false;
    let mut overhead = false;
    for _ in 0..30 {
        sim.step(TickInput::default());
        let (cx, cy, frame) = (claw(&sim).x, claw(&sim).y, claw(&sim).frame);
        let o = obj(&sim, id);
        match frame {
            42 => {
                // Facing right: 10 px behind him, 15 px up, its lift
                // animation, facing away from his front.
                assert_eq!(state(&sim, id), Some(EnemyState::Lifted));
                assert_eq!((o.x - cx, o.y - cy), (CARRY.beside.1, CARRY.beside.2));
                assert_eq!((o.x - cx, o.y - cy), (-10, -15));
                assert_eq!(o.anim.key(), Some(crab::LIFT_ANIM));
                assert!(!enemy::faces_right(o));
                beside = true;
            }
            43 => {
                assert_eq!(state(&sim, id), Some(EnemyState::Flailing));
                assert_eq!((o.x - cx, o.y - cy), (-10, -69));
                assert_eq!(o.anim.key(), Some(crab::HIDE_ANIM));
                overhead = true;
            }
            _ => {}
        }
    }
    assert!(beside && overhead);
    assert!(sim.claw().special.carrying);
    assert_eq!(state(&sim, id), Some(EnemyState::Flailing));
    // It stays over his head as he carries it about, facing left too.
    for _ in 0..10 {
        sim.step(hold(Buttons::LEFT.0));
    }
    assert!(!sim.claw().facing_right);
    sim.step(TickInput::default());
    let (cx, cy) = (claw(&sim).x, claw(&sim).y);
    let o = obj(&sim, id);
    assert_eq!((o.x - cx, o.y - cy), (CARRY.overhead.0, CARRY.overhead.2));
    assert_eq!((o.x - cx, o.y - cy), (10, -69));
    assert!(enemy::faces_right(o));
    // Winding up a throw shifts it 4 px back and 5 px down.
    sim.step(press(Buttons::LIFT.0));
    assert_eq!(claw(&sim).state, state::THROW);
    sim.step(hold(Buttons::LIFT.0));
    let (cx, cy) = (claw(&sim).x, claw(&sim).y);
    let o = obj(&sim, id);
    assert_eq!((o.x - cx, o.y - cy), (10 + 4, -69 + 5));
}

#[test]
fn claw_cannot_lift_a_crab_out_of_its_shell() {
    let (mut sim, ids) = game(&[1600]);
    let id = ids[0];
    sim.with_world(|w| crab::go_home(w, id));
    let x = claw(&sim).x + 40;
    put(&mut sim, id, x);
    sim.step(press(Buttons::LIFT.0));
    assert_ne!(claw(&sim).state, state::LIFT);
    assert_eq!(sim.claw().special.lifted, None);
    assert_ne!(state(&sim, id), Some(EnemyState::Lifted));
}

#[test]
fn a_thrown_crab_falls_lands_and_regroups_unhurt() {
    let (mut sim, ids) = game(&[1600]);
    let id = ids[0];
    let health = obj(&sim, id).health;
    lift_in_front(&mut sim, id);
    for _ in 0..30 {
        sim.step(TickInput::default());
    }
    assert!(sim.claw().special.carrying);
    sim.step(press(Buttons::LIFT.0));
    let mut thrown_at = None;
    for k in 0..30 {
        sim.step(TickInput::default());
        if state(&sim, id) == Some(EnemyState::Thrown) {
            thrown_at = Some(k);
            break;
        }
    }
    assert!(thrown_at.is_some(), "thrown");
    let o = obj(&sim, id);
    assert_eq!(o.type_flags, types::THROWN);
    assert_eq!(o.physics_type, ptype::FLY);
    assert_eq!(o.anim.key(), Some(crab::FALL_ANIM));
    assert_ne!(o.flags & flags::SAFE, 0);
    let (vx, _) = special::throw_velocity(0, true);
    let d = enemy::data_of(o).unwrap();
    assert_eq!(d.vx, f64::from(vx));
    assert!(d.vy < 0.0);
    assert!(!d.frozen);
    // It flies, lands and regroups at once: no bounce, no rest, no damage,
    // feet on the floor, an enemy that can be hit again.
    let mut seen = Vec::new();
    let mut landed = None;
    for k in 0..200 {
        sim.step(TickInput::default());
        let s = state(&sim, id);
        seen.push(s);
        if s != Some(EnemyState::Thrown) {
            landed = Some(k);
            break;
        }
    }
    let landed = landed.expect("it lands");
    assert!(landed > 5, "{landed}");
    assert!(!seen.contains(&Some(EnemyState::Bounce)));
    let o = obj(&sim, id);
    assert!(
        matches!(
            state(&sim, id),
            Some(EnemyState::Regroup | EnemyState::Hide | EnemyState::Defense)
        ),
        "{:?}",
        state(&sim, id)
    );
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.flags & (flags::SAFE | flags::ALWAYS_ACTIVE), 0);
    assert_eq!(o.move_rect, enemy::facing_rect(o, crab::MOVE_RECT));
    assert_eq!(o.y, CRAB_Y);
    assert_eq!(o.health, health);
    let d = enemy::data_of(o).unwrap();
    assert!(!d.busy && !d.dead);
    assert!(o.x > 1100, "thrown forwards: {}", o.x);
}

/// A crab lifted by Claw and thrown at (vx, vy) from (x, y).
fn throw_from(sim: &mut Sim, id: ObjectId, at: (i32, i32), v: (i32, i32)) {
    let carrier = claw_id(sim);
    sim.with_world(|w: &mut World| {
        crab::lifted(w, id, carrier);
        let o = w.objects.get_mut(id).unwrap();
        o.x = at.0;
        o.y = at.1;
        crab::thrown(w, id, v.0, v.1);
    });
}

/// Whether `attacker` hit `victim` on the last tick.
fn hit(sim: &Sim, attacker: ObjectId, victim: ObjectId) -> bool {
    sim.collision_events().iter().any(|e| {
        matches!(*e, CollisionEvent::Hit { attacker: a, victim: v } if a == attacker && v == victim)
    })
}

#[test]
fn a_thrown_crab_hurts_a_human_enemy_and_a_crate_but_passes_another_crab() {
    let (mut sim, ids) = game(&[1600, 1650]);
    let (thrown, other) = (ids[0], ids[1]);
    // Claw out of the way: nothing hurts him.
    let cid = claw_id(&sim);
    sim.with_world(|w| w.objects.get_mut(cid).unwrap().hit_type_flags = 0);
    let officer = sim.spawn(logic::new_object("Officer", 1300, 899, 4000));
    let crate_id = sim.spawn(logic::new_object("FrontCrate", 1200, 930, 4000));
    for _ in 0..3 {
        sim.step(TickInput::default());
        hide_all(&mut sim, &[other]);
    }
    // At the officer's middle.
    let o = obj(&sim, officer);
    assert!(o.hit_rect.is_set() && o.is_listed());
    let (ox, oy, health) = (o.x, o.y, o.health);
    throw_from(&mut sim, thrown, (ox, oy), (0, 0));
    sim.step(TickInput::default());
    assert!(hit(&sim, thrown, officer), "the officer is hit");
    let o = obj(&sim, officer);
    assert!(
        o.health < health || state(&sim, officer) == Some(EnemyState::Dead),
        "{} {:?}",
        o.health,
        state(&sim, officer)
    );
    // At the crate.
    sim.with_world(|w| crab::go_home(w, thrown));
    let c = obj(&sim, crate_id);
    assert!(c.hit_rect.is_set() && c.is_listed());
    let (kx, ky) = (c.x, c.y);
    throw_from(&mut sim, thrown, (kx, ky), (0, 0));
    sim.step(TickInput::default());
    assert!(hit(&sim, thrown, crate_id), "the crate is hit");
    // At the other crab, hiding: it passes through, no knock, no kill.
    sim.with_world(|w| crab::go_home(w, thrown));
    hide_all(&mut sim, &[other]);
    let c = obj(&sim, other);
    let (cx, cy) = (c.x, c.y);
    assert!(c.hit_rect.is_set() && c.is_listed());
    assert_eq!(c.hit_type_flags & types::THROWN, 0);
    throw_from(&mut sim, thrown, (cx, cy), (0, 0));
    for _ in 0..3 {
        sim.step(TickInput::default());
        assert!(!hit(&sim, thrown, other));
        assert!(matches!(
            state(&sim, other),
            Some(EnemyState::Hide | EnemyState::Regroup | EnemyState::Defense)
        ));
    }
}

#[test]
fn a_dropped_crab_gets_up_after_a_second_and_regroups() {
    let (mut sim, ids) = game(&[1600]);
    let id = ids[0];
    lift_in_front(&mut sim, id);
    for _ in 0..30 {
        sim.step(TickInput::default());
    }
    assert_eq!(state(&sim, id), Some(EnemyState::Flailing));
    sim.with_world(|w| special::tell_dropped(w, id));
    sim.step(TickInput::default());
    assert_eq!(state(&sim, id), Some(EnemyState::Dropped));
    let mut up = None;
    for k in 1..=100 {
        sim.step(TickInput::default());
        if state(&sim, id) != Some(EnemyState::Dropped) {
            up = Some(k);
            break;
        }
    }
    let up = up.expect("it gets up");
    assert!((55..=57).contains(&up), "{up}");
    let o = obj(&sim, id);
    assert_eq!(o.type_flags, types::ENEMY);
    assert_eq!(o.flags & (flags::SAFE | flags::NO_COLLIDE), 0);
    assert_eq!(o.move_rect, enemy::facing_rect(o, crab::MOVE_RECT));
    assert_eq!(o.y, CRAB_Y);
    assert!(!liftable(&sim, id) || state(&sim, id) == Some(EnemyState::Hide));
}

#[test]
fn the_carried_box_is_a_point_and_the_offsets_are_the_crabs() {
    assert_eq!(CARRY.beside, (0, -10, -15));
    assert_eq!(CARRY.overhead, (10, -10, -69));
    assert_eq!(CARRY.move_rect, Rect::new(0, 0, 0, 0));
    let carry = crab::CLASS.carry.expect("a crab can be carried");
    assert_eq!(carry.beside, CARRY.beside);
    assert_eq!(crab::CLASS.anims.lifted, crab::LIFT_ANIM);
    assert_eq!(crab::CLASS.anims.lifted2, crab::HIDE_ANIM);
}
