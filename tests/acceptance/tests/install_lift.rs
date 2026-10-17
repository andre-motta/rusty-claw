//! Lifting a defeated officer on the first level of your own install
//! (skipped without `CLAW_INSTALL`): an officer placed from the level's own
//! records, carrying loot, is killed where it stands as on spikes; Claw
//! lifts the body with the lift key, carries it and throws it. The body
//! hurts nobody, dies again as a thrown object that Claw can no longer
//! lift, and is removed without dropping its loot a second time.
//!
//! That the body hurts nobody is checked with a live officer kept in its
//! path for a few ticks of its flight, in a copy of the game: it takes no
//! hit from the body. A control copy, in which the body is made able to
//! hurt, shows the same officer is struck, so the check is not passing
//! for want of anyone in reach.

use std::collections::BTreeSet;

use claw_acceptance::install;
use claw_acceptance::play::*;
use claw_game::load_level;
use claw_game::settings::LevelChoice;
use claw_sim::container::CollisionEvent;
use claw_sim::logic;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::object::{ObjectId, flags, types};
use claw_sim::player::state;
use claw_sim::{ObjectData, Sim, SimConfig};

/// Where Claw stands from the officer's record, nearest first.
const NEAR: [i32; 8] = [-50, 50, -60, 60, -40, 40, -70, 70];

/// The loot lying about.
fn loot(p: &Player) -> BTreeSet<ObjectId> {
    p.sim
        .objects()
        .all()
        .filter(|(_, o)| matches!(&o.data, ObjectData::Pickup(d) if d.drop_code.is_some()))
        .map(|(id, _)| id)
        .collect()
}

fn enemy_state(p: &Player, id: ObjectId) -> Option<EnemyState> {
    p.obj(id).and_then(|o| EnemyState::from_id(o.state))
}

/// Ticks the witness is kept in the flying body's path.
const WITNESS_TICKS: usize = 6;

/// In a copy of `base`, puts a live officer where the flying `body` is and
/// keeps it there for [`WITNESS_TICKS`] ticks. With `harmless` false the
/// body is first made able to hurt (the control). Returns the ticks in
/// which the officer could be hit by a thrown object and its box met the
/// body's, and the hits the body made on it.
fn witness(base: &Player, body: ObjectId, harmless: bool) -> (usize, usize) {
    let mut p = base.clone();
    let (x, y) = p.obj(body).map(|o| (o.x, o.y)).expect("the body");
    let w = p.sim.spawn(logic::new_object("Officer", x, y, 4000));
    if !harmless {
        p.sim.with_world(|wd| {
            wd.objects.get_mut(body).expect("the body").flags &= !flags::HARMLESS;
        });
    }
    let (mut in_reach, mut hits) = (0, 0);
    for _ in 0..WITNESS_TICKS {
        let Some((x, y)) = p.obj(body).map(|o| (o.x, o.y)) else {
            break;
        };
        p.sim.with_world(|wd| {
            if let Some(o) = wd.objects.get_mut(w) {
                (o.x, o.y) = (x, y);
            }
        });
        p.step(0);
        hits += p
            .sim
            .collision_events()
            .iter()
            .filter(|e| {
                matches!(**e, CollisionEvent::Hit { attacker, victim }
                    if attacker == body && victim == w)
            })
            .count();
        let (Some(b), Some(v)) = (p.obj(body), p.obj(w)) else {
            break;
        };
        let meet = b.attack_rect.is_set()
            && v.hit_rect.is_set()
            && b.attack_rect
                .translate(b.x, b.y)
                .overlaps(&v.hit_rect.translate(v.x, v.y));
        if meet && v.hit_type_flags & types::THROWN != 0 {
            in_reach += 1;
        }
    }
    (in_reach, hits)
}

/// Kills the officer placed at `placed` beside Claw, lifts, carries and
/// throws its body, and checks what follows. `false` when Claw cannot be
/// put beside it, it carries no loot, or Claw cannot lift it from there.
fn lift_and_throw(base: &Player, placed: (i32, i32, i32)) -> bool {
    let mut p = base.clone();
    let Some(id) = place_beside(&mut p, "Officer", placed, &NEAR, false) else {
        return false;
    };
    let carries_loot = p
        .obj(id)
        .and_then(enemy::data_of)
        .is_some_and(|d| d.loot.iter().any(|&c| c != 0));
    if !carries_loot {
        return false;
    }

    // Killed where it stands, as on spikes: its loot drops once.
    let before = loot(&p);
    p.sim
        .with_world(|w| enemy::die(w, id, DeathKind::Spikes, None));
    let mut known = loot(&p);
    assert!(
        known.len() > before.len(),
        "{placed:?}: the loot of the death"
    );
    let face = p.obj(id).expect("the body").x > p.at().0;
    if p.sim.claw().facing_right != face {
        p.step(if face { RIGHT } else { LEFT });
    }
    p.idle(2);
    assert_eq!(enemy_state(&p, id), Some(EnemyState::Dead), "{placed:?}");

    p.step(LIFT);
    if p.sim.claw().special.lifted != Some(id) {
        return false;
    }
    assert_eq!(enemy_state(&p, id), Some(EnemyState::Lifted), "{placed:?}");
    let carrying = p.until(60, |p| (!p.sim.claw().special.carrying).then_some(0));
    assert!(carrying, "{placed:?}: Claw carries the body");
    assert_eq!(
        enemy_state(&p, id),
        Some(EnemyState::Flailing),
        "{placed:?}"
    );
    p.step(0);
    p.step(LIFT);
    assert_eq!(p.state(), state::THROW, "{placed:?}: Claw throws");

    let mut seen = Vec::new();
    let mut new_loot = 0;
    let mut struck = Vec::new();
    let mut thrown_type = false;
    let mut witnessed = false;
    for _ in 0..400 {
        p.step(0);
        for l in loot(&p) {
            new_loot += usize::from(known.insert(l));
        }
        struck.extend(p.sim.collision_events().iter().filter_map(|e| match *e {
            CollisionEvent::Hit { attacker, victim } if attacker == id => Some(victim),
            _ => None,
        }));
        let Some(s) = enemy_state(&p, id) else { break };
        if seen.last() != Some(&s) {
            seen.push(s);
        }
        thrown_type |= p.obj(id).is_some_and(|o| o.type_flags == types::THROWN);
        if s == EnemyState::Thrown && !witnessed {
            witnessed = true;
            let (in_reach, hits) = witness(&p, id, true);
            assert!(in_reach > 0, "{placed:?}: the witness was in reach");
            assert_eq!(hits, 0, "{placed:?}: the body struck the witness");
            let (in_reach, hits) = witness(&p, id, false);
            assert!(in_reach > 0, "{placed:?}: control: in reach");
            assert!(hits > 0, "{placed:?}: control: a body able to hurt strikes");
        }
        if s == EnemyState::Dead {
            let o = p.obj(id).unwrap();
            assert_eq!(o.type_flags, types::THROWN, "{placed:?}: not a target");
            assert_eq!(
                enemy::data_of(o).map(|d| d.death),
                Some(DeathKind::Landing),
                "{placed:?}"
            );
        }
    }
    println!("officer body at {placed:?}: {seen:?}");
    assert!(seen.contains(&EnemyState::Thrown), "{placed:?}: {seen:?}");
    assert!(witnessed, "{placed:?}: the flight was checked");
    assert!(thrown_type, "{placed:?}: flew as a thrown object");
    assert!(p.obj(id).is_none(), "{placed:?}: the body is removed");
    assert!(struck.is_empty(), "{placed:?}: the body struck {struck:?}");
    assert_eq!(new_loot, 0, "{placed:?}: no second loot");
    true
}

#[test]
fn install_a_defeated_officer_is_lifted_thrown_and_removed_without_loot() {
    let Some(root) = install() else { return };
    let bundle = load_level(&root, &LevelChoice::Retail(1)).expect("level 1");
    let level = bundle.sim_level.clone();
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    // Nothing hurts Claw here: a hit would make him drop the body.
    sim.with_world(|w| {
        let id = w.claw_id.expect("Claw");
        w.objects.get_mut(id).expect("Claw").hit_type_flags = 0;
    });
    let base = Player::new(sim);
    assert!(
        records(&level, "Officer")
            .into_iter()
            .any(|placed| lift_and_throw(&base, placed)),
        "no officer of the level could be lifted after its death"
    );
}
