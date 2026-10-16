//! Throwing powder kegs at enemies on the first level of your own install
//! (skipped without `CLAW_INSTALL`): Claw lifts a keg placed by the level,
//! carries it toward an officer or a soldier standing on the same floor
//! and throws it; the keg explodes where it strikes the enemy, and the
//! enemy dies of the strike.

use std::sync::Arc;

use claw_assets::{Install, LevelData, LevelSource};
use claw_frame::{Buttons, TickInput};
use claw_sim::container::CollisionEvent;
use claw_sim::logic::enemy::{self, DeathKind, EnemyState};
use claw_sim::logic::keg::{self, Phase};
use claw_sim::object::ObjectId;
use claw_sim::player::state;
use claw_sim::{Sim, SimConfig, SimLevel};

const LIFT: u32 = 0x20;
const LEFT: u32 = 0x100_0000;
const RIGHT: u32 = 0x200_0000;

fn level1() -> Option<Arc<SimLevel>> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(1)).expect("level 1 loads");
    Some(Arc::new(
        SimLevel::from_level(&level, &store).expect("level 1 prepares"),
    ))
}

fn press(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons(mask),
    }
}

fn hold(mask: u32) -> TickInput {
    TickInput {
        held: Buttons(mask),
        pressed: Buttons::NONE,
    }
}

/// The kegs placed by the level with an enemy of logic `foe` on the same
/// floor (placed within 20 px of height) between 150 and 400 px away: the
/// keg's and the enemy's placed positions.
fn keg_and_foe(level: &SimLevel, foe: &str) -> Vec<((i32, i32), (i32, i32))> {
    let at = |name: &str| -> Vec<(i32, i32)> {
        level
            .objects
            .iter()
            .filter(|o| o.logic == name)
            .map(|o| (o.record[5], o.record[6]))
            .collect()
    };
    let foes = at(foe);
    at("PowderKeg")
        .into_iter()
        .flat_map(|k| {
            foes.iter()
                .filter(move |f| {
                    (150..=400).contains(&(f.0 - k.0).abs()) && (f.1 - k.1).abs() <= 20
                })
                .map(move |&f| (k, f))
        })
        .collect()
}

fn placed(sim: &Sim, logic: &str, (x, y): (i32, i32)) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && (o.editor.0, o.editor.1) == (x, y))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("{logic} placed at ({x}, {y})"))
}

fn phase(sim: &Sim, id: ObjectId) -> Option<Phase> {
    sim.objects()
        .get(id)
        .and_then(keg::data_of)
        .map(|d| d.phase)
}

/// Lifts the keg placed at `k`, carries it to 130 px from the `foe`
/// placed at `f`, facing it, and throws it with a tap; checks the strike,
/// the explosion where it struck and the kill. `false` when Claw cannot
/// lift a keg there or finds no floor to stand on beside the enemy.
fn throw_at(level: &Arc<SimLevel>, foe: &str, k: (i32, i32), f: (i32, i32)) -> bool {
    let mut sim = Sim::new(level.clone(), SimConfig::default());
    sim.step(TickInput::default());
    // Nothing hurts Claw here: soldiers shoot from afar, and a hit would
    // make him drop the keg.
    sim.with_world(|w| {
        let id = w.claw_id.expect("Claw");
        w.objects.get_mut(id).expect("Claw").hit_type_flags = 0;
    });
    // The enemy is on the keg's left or right; Claw lifts it from the far
    // side, facing the enemy.
    let side = if f.0 < k.0 { 1 } else { -1 };
    let toward = if side > 0 { LEFT } else { RIGHT };
    sim.teleport_player(k.0 + side * 45, k.1 - 30);
    for _ in 0..30 {
        sim.step(TickInput::default());
    }
    sim.step(press(toward));
    for _ in 0..3 {
        sim.step(TickInput::default());
    }
    let eid = placed(&sim, foe, f);
    // Where Claw stands on the enemy's floor 130 px from it, found by
    // dropping him there in a copy of the run.
    let spot = {
        let mut trial = sim.clone();
        let e = trial.objects().get(eid).expect("the enemy");
        let (ex, feet) = (e.x, e.y + e.move_rect.bottom);
        // Claw's feet are 60 px below his position.
        trial.teleport_player(ex + side * 130, feet - 80);
        for _ in 0..40 {
            trial.step(TickInput::default());
        }
        let c = trial.player().unwrap();
        if c.state != state::STAND {
            return false;
        }
        (c.x, c.y)
    };
    sim.step(press(LIFT));
    // Kegs often stand in groups; any of them will do.
    let Some(kid) = sim.claw().special.lifted else {
        return false;
    };
    assert!(phase(&sim, kid).is_some(), "{foe}: Claw lifted a keg");
    for _ in 0..200 {
        if sim.claw().special.carrying {
            break;
        }
        sim.step(TickInput::default());
    }
    assert!(sim.claw().special.carrying, "{foe}: Claw carries the keg");
    sim.teleport_player(spot.0, spot.1);
    sim.step(hold(toward));
    sim.step(TickInput::default());
    assert_eq!(
        sim.player().unwrap().state,
        state::LIFT,
        "{foe}: still carrying"
    );
    assert_eq!(sim.claw().facing_right, side < 0, "{foe}: facing the enemy");
    sim.step(press(LIFT));
    assert_eq!(sim.player().unwrap().state, state::THROW);

    let mut thrown = false;
    let mut exploded = false;
    let mut strike = None;
    for t in 0..150 {
        sim.step(TickInput::default());
        thrown |= phase(&sim, kid) == Some(Phase::Thrown);
        exploded |= strike.is_some() && phase(&sim, kid) == Some(Phase::Exploding);
        let struck = sim.collision_events().iter().any(|e| {
            matches!(*e, CollisionEvent::Hit { attacker, victim } if attacker == kid && victim == eid)
        });
        if struck && strike.is_none() {
            let (o, e) = (
                sim.objects().get(kid).unwrap(),
                sim.objects().get(eid).unwrap(),
            );
            let (kb, eb) = (
                o.hit_rect.translate(o.x, o.y),
                e.hit_rect.translate(e.x, e.y),
            );
            strike = Some((t, (o.x, o.y), kb, eb));
        }
        if strike.is_some() && sim.objects().get(kid).is_none() {
            break;
        }
        if let Some((_, at, ..)) = strike
            && let Some(o) = sim.objects().get(kid)
        {
            assert_eq!((o.x, o.y), at, "{foe}: the keg stays where it struck");
        }
    }
    assert!(thrown, "{foe}: the keg was thrown");
    let (t, at, kb, eb) = strike.unwrap_or_else(|| panic!("{foe}: the keg strikes the enemy"));
    eprintln!("{foe}: keg struck at {at:?} after {t} ticks; keg box {kb:?}, enemy box {eb:?}");
    assert!(kb.overlaps(&eb), "{foe}: the keg was touching the enemy");
    assert!(exploded, "{foe}: the keg exploded where it struck");
    let o = sim.objects().get(eid).expect("the body is still there");
    let d = enemy::data_of(o).expect("enemy data");
    assert_eq!(
        EnemyState::from_id(o.state),
        Some(EnemyState::Dead),
        "{foe}"
    );
    assert!(
        d.dead && d.death == DeathKind::Hit,
        "{foe}: dead of the strike"
    );
    true
}

/// Throws at the first `foe` of the level where the set-up works.
fn throw_at_first(foe: &str) {
    let Some(level) = level1() else { return };
    let pairs = keg_and_foe(&level, foe);
    assert!(
        pairs.into_iter().any(|(k, f)| throw_at(&level, foe, k, f)),
        "no {foe} near a keg with a floor to throw from"
    );
}

#[test]
fn install_a_thrown_keg_explodes_on_an_officer_and_kills_him() {
    throw_at_first("Officer");
}

#[test]
fn install_a_thrown_keg_explodes_on_a_soldier_and_kills_him() {
    throw_at_first("Soldier");
}
