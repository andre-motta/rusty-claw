//! The search's guide in Omar's fight on the fourteenth level: each step
//! of the fight (fetching the sword whose shot hurts him through his
//! ring, going down to the floor between his ledges, the good shot that
//! moves him to the other ledge and turns his ring, then the blows in the
//! hall) is nearer the gem than the step before, and the whole fight
//! stays nearer than the way to the boss room's door. The sword a shot
//! needs is the one opposite the ring's element.

use claw_bot::search::{
    DOOR_BASE, FIGHT_BASE, OMAR_GUIDE_MAX, OMAR_WAY_CAP, OmarFight, omar_guide, omar_sword,
};
use claw_sim::logic::enemy::omar;
use claw_sim::player::death::powerup;

/// A state of the first phase: Omar's health, whether Claw holds the
/// sword the next shot needs, and how far he is from it and the floor.
fn ring(health: i32, armed: bool, to_sword: u32, to_floor: u32) -> OmarFight {
    OmarFight {
        ring: true,
        health,
        armed,
        to_floor,
        to_sword,
        to_hall: OMAR_WAY_CAP,
    }
}

/// A state of the second phase: his health and the way to him.
fn hall(health: i32, to_hall: u32) -> OmarFight {
    OmarFight {
        ring: false,
        health,
        armed: false,
        to_floor: OMAR_WAY_CAP,
        to_sword: OMAR_WAY_CAP,
        to_hall,
    }
}

/// The fight as the search plays it, step by step, with Claw at his
/// farthest from what he heads for at the start of each way.
fn fight() -> Vec<(String, OmarFight)> {
    let mut steps = Vec::new();
    let mut health = omar::HEALTH;
    while health > 0 {
        for to_sword in [OMAR_WAY_CAP, 1500, 0] {
            steps.push((
                format!("{health}: to the sword {to_sword}"),
                ring(health, false, to_sword, OMAR_WAY_CAP),
            ));
        }
        for to_floor in [OMAR_WAY_CAP, 1500, 0] {
            steps.push((
                format!("{health}: armed, to the floor {to_floor}"),
                ring(health, true, 0, to_floor),
            ));
        }
        health -= omar::ELEMENT_DAMAGE;
    }
    let mut health = omar::HEALTH;
    while health > 0 {
        for to_hall in [OMAR_WAY_CAP, 800, 0] {
            steps.push((
                format!("hall {health}: to him {to_hall}"),
                hall(health, to_hall),
            ));
        }
        health -= omar::BLOW_DAMAGE;
    }
    steps
}

#[test]
fn every_step_of_omars_fight_is_nearer_than_the_one_before() {
    let steps = fight();
    for w in steps.windows(2) {
        let (a, b) = (omar_guide(&w[0].1), omar_guide(&w[1].1));
        assert!(b < a, "{} ({a}) then {} ({b})", w[0].0, w[1].0);
    }
}

#[test]
fn the_whole_fight_is_nearer_than_the_way_to_the_door() {
    for (name, f) in fight() {
        let g = omar_guide(&f);
        assert!(g <= OMAR_GUIDE_MAX, "{name}");
        assert!(FIGHT_BASE + g < DOOR_BASE, "{name}");
    }
    // Far beyond the caps, a state counts no further.
    let far = ring(omar::HEALTH, false, u32::MAX / 4, u32::MAX / 4);
    assert_eq!(omar_guide(&far), OMAR_GUIDE_MAX);
}

#[test]
fn a_wrong_shot_that_heals_him_is_further_away() {
    // Holding the sword, on the floor: a good shot is nearer, a wrong one
    // (which heals him 25) is a whole shot further.
    let now = omar_guide(&ring(50, true, 0, 0));
    let good = omar_guide(&ring(25, false, OMAR_WAY_CAP, OMAR_WAY_CAP));
    let healed = omar_guide(&ring(75, true, 0, 0));
    assert!(good < now);
    assert!(healed > now);
}

#[test]
fn the_shot_through_a_ring_needs_the_opposite_element() {
    assert_eq!(omar_sword(false), powerup::ICE_SWORD);
    assert_eq!(omar_sword(true), powerup::FIRE_SWORD);
}
