//! The search's guide in Marrow's fight on the tenth level: every step of
//! the fight's loop (the parrot's three hits, the bridge, Marrow's five
//! hits, his whistle and the carry) is nearer the gem than the step
//! before, round after round, and the whole fight stays nearer than the
//! way to the boss room's door.

use claw_bot::search::{
    DOOR_BASE, FIGHT_BASE, MARROW_BOX_CLEAR, MARROW_NEAR_CAP, MarrowFight, PARROT_NEAR_CAP,
    marrow_guide, near_box_edge,
};

/// How far a blow on Marrow can leave him from Claw, px: his flinch
/// slides him back.
const PUSHED: u32 = 100;

/// A state of the fight with Claw far from both Marrow and the parrot.
fn far(health: i32, phase_health: i32) -> MarrowFight {
    MarrowFight {
        health,
        phase_health,
        spent: false,
        met: false,
        near_marrow: MARROW_NEAR_CAP,
        parrot_hits: 0,
        near_parrot: PARROT_NEAR_CAP,
    }
}

/// The loop played round after round from the normal entry, each step as
/// the guide sees it, with Claw at his farthest from what he heads for.
fn normal_fight() -> Vec<(String, MarrowFight)> {
    let mut steps = Vec::new();
    let mut phase = 0;
    let mut health = 100;
    while health > 0 {
        // The pit is open: the parrot's hits, Claw coming close to it.
        for hits in 0..=3 {
            for near in [PARROT_NEAR_CAP, 0] {
                let f = MarrowFight {
                    parrot_hits: hits,
                    near_parrot: near,
                    ..far(health, phase)
                };
                steps.push((format!("{health}: parrot hit {hits}, {near} px"), f));
            }
        }
        // The parrot perches and the slabs close: the bridge, and the
        // phase health they write when closed.
        steps.push((
            format!("{health}: bridge closing"),
            MarrowFight {
                met: true,
                ..far(health, phase)
            },
        ));
        phase = health;
        // Claw comes to him and hits him five times by five, a blow
        // pushing him a little way back.
        steps.push((
            format!("{health}: on to Marrow"),
            MarrowFight {
                met: true,
                ..far(health, phase)
            },
        ));
        for _ in 0..5 {
            steps.push((
                format!("{health}: beside Marrow"),
                MarrowFight {
                    met: true,
                    near_marrow: 0,
                    ..far(health, phase)
                },
            ));
            health -= 5;
            if health == 0 {
                // He dies: the gem's stage, past the fight's guide.
                break;
            }
            steps.push((
                format!("{health}: Marrow pushed back"),
                MarrowFight {
                    // The fifth blow spends the round.
                    spent: health <= phase - 25,
                    met: true,
                    near_marrow: PUSHED,
                    ..far(health, phase)
                },
            ));
        }
        if health == 0 {
            break;
        }
        // His whistle is due, he whistles, and the parrot carries him
        // across with the pit opening; Claw is far from him.
        steps.push((
            format!("{health}: whistle due"),
            MarrowFight {
                spent: true,
                met: true,
                near_marrow: 0,
                ..far(health, phase)
            },
        ));
        steps.push((
            format!("{health}: carried"),
            MarrowFight {
                spent: true,
                ..far(health, phase)
            },
        ));
        // The carry's end writes his health as the phase health.
        phase = health;
    }
    steps
}

#[test]
fn every_step_of_the_loop_is_nearer_than_the_one_before() {
    let steps = normal_fight();
    for w in steps.windows(2) {
        let (a, b) = (marrow_guide(&w[0].1), marrow_guide(&w[1].1));
        assert!(
            b <= a,
            "{} ({a}) is nearer than {} ({b}) after it",
            w[0].0,
            w[1].0
        );
    }
}

#[test]
fn each_hit_on_marrow_or_the_parrot_is_progress() {
    for (name, f) in normal_fight() {
        let g = marrow_guide(&f);
        if f.met && !f.spent {
            let hit = MarrowFight {
                health: f.health - 5,
                ..f
            };
            assert!(
                marrow_guide(&hit) < g,
                "a blow on Marrow at {name} is no progress"
            );
        } else if !f.spent && f.parrot_hits < 3 {
            let hit = MarrowFight {
                parrot_hits: f.parrot_hits + 1,
                ..f
            };
            assert!(
                marrow_guide(&hit) < g,
                "a hit on the parrot at {name} is no progress"
            );
        }
    }
}

#[test]
fn the_fight_stays_nearer_than_the_way_to_the_door() {
    for (name, f) in normal_fight() {
        assert!(
            FIGHT_BASE + marrow_guide(&f) < DOOR_BASE,
            "{name} is guided as far as the door"
        );
    }
}

#[test]
fn after_the_intro_skip_every_blow_on_marrow_is_progress() {
    // After the jump-in no phase health is written and the pit stays
    // open: Claw on Marrow's ledge hits him to the end.
    let mut last = None;
    for health in (5..=100).rev().step_by(5) {
        let g = marrow_guide(&MarrowFight {
            met: true,
            near_marrow: 0,
            ..far(health, 0)
        });
        if let Some(l) = last {
            assert!(g < l, "{health} is no nearer than the blow before");
        }
        last = Some(g);
    }
}

/// Claw one pixel inside the right edge of Marrow's outermost box built
/// where he landed, and far below the box built at the height he hung
/// from: on the first box's edge, clear of the second's.
#[test]
fn a_pixel_inside_a_range_box_is_on_its_edge() {
    let landed = (40107, 7233, 41107, 7273);
    let kept = (40107, 7105, 41107, 7145);
    let claw = (41106, 7237);
    assert!(near_box_edge(landed, claw, MARROW_BOX_CLEAR));
    assert!(!near_box_edge(kept, claw, MARROW_BOX_CLEAR));
    // Clear inside and clear outside, across and down.
    assert!(!near_box_edge(landed, (40607, 7253), MARROW_BOX_CLEAR));
    assert!(!near_box_edge(
        landed,
        (41107 + MARROW_BOX_CLEAR, 7253),
        MARROW_BOX_CLEAR
    ));
    assert!(near_box_edge(
        landed,
        (41107 + MARROW_BOX_CLEAR - 1, 7253),
        MARROW_BOX_CLEAR
    ));
    assert!(near_box_edge(landed, (40600, 7273), MARROW_BOX_CLEAR));
    assert!(!near_box_edge(
        landed,
        (40600, 7273 + MARROW_BOX_CLEAR),
        MARROW_BOX_CLEAR
    ));
    // Beside a corner, past the edge's length.
    assert!(!near_box_edge(landed, (41200, 7273), MARROW_BOX_CLEAR));
}
