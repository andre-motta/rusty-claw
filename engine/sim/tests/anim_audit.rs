//! Everything that loops a picture by itself does so on a synthetic level,
//! and still decoration keeps its image: animated decoration and the
//! cycling signs, placed treasure and powerups, the glitter next to a
//! pickup, and the boss door all step to the next image of their set on
//! their animation's timing and wrap around it; still candy, the do-nothing
//! decoration and the goo cover-up never change.

use std::collections::BTreeMap;
use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::{boss_stage, decor, pickup};
use claw_sim::object::ObjectId;
use claw_sim::tuning::TICK_MS;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

/// Calls between two images of a loop of `ms` per image.
fn calls_per_image(ms: usize) -> usize {
    ms.div_ceil(TICK_MS as usize)
}

/// A loop of one element that shows the next image of the set every `ms`.
fn next_image(key: &str, ms: i32) -> Animation {
    Animation {
        key: key.to_string(),
        frames: vec![AnimFrame {
            image_op: 1,
            duration: ms,
            ..AnimFrame::default()
        }],
        ..Animation::default()
    }
}

/// One object of the audit: its logic, its picture and that picture's
/// frames, and how many ms each image of its loop lasts (`None` for still
/// pictures).
struct Case {
    logic: &'static str,
    image_set: &'static str,
    frames: (i32, i32),
    x: i32,
    image_ms: Option<usize>,
}

const CASES: &[Case] = &[
    Case {
        logic: "BehindAniCandy",
        image_set: "LEVEL_TORCH",
        frames: (1, 4),
        x: 700,
        image_ms: Some(100),
    },
    Case {
        logic: "FrontAniCandy",
        image_set: "LEVEL_FLAGS",
        frames: (1, 5),
        x: 760,
        image_ms: Some(100),
    },
    Case {
        logic: "AniCycle",
        image_set: "LEVEL_ARROWSIGN_RIGHT",
        frames: (1, 3),
        x: 820,
        image_ms: Some(100),
    },
    Case {
        logic: "AniCycleNormal",
        image_set: "LEVEL_ARROWSIGN_LR",
        frames: (1, 3),
        x: 880,
        image_ms: Some(100),
    },
    Case {
        logic: "GlitterlessPowerup",
        image_set: "GAME_TREASURE_COINS",
        frames: (1, 9),
        x: 1150,
        image_ms: Some(100),
    },
    Case {
        logic: "TreasurePowerup",
        image_set: "GAME_TREASURE_RINGS_RED",
        frames: (1, 5),
        x: 1210,
        image_ms: Some(100),
    },
    Case {
        logic: "HealthPowerup",
        image_set: "LEVEL_HEALTH",
        frames: (1, 3),
        x: 1270,
        image_ms: Some(100),
    },
    Case {
        logic: "SpecialPowerup",
        image_set: "GAME_POWERUPS_EXTRALIFE",
        frames: (1, 7),
        x: 1330,
        image_ms: Some(100),
    },
    Case {
        logic: boss_stage::WARP,
        image_set: boss_stage::WARP_IMAGE,
        frames: (1, 12),
        x: 1390,
        image_ms: Some(50),
    },
    Case {
        logic: "BehindCandy",
        image_set: "LEVEL_ARCHESFRONT",
        frames: (1, 5),
        x: 640,
        image_ms: None,
    },
    Case {
        logic: "FrontCandy",
        image_set: "LEVEL_WEB1",
        frames: (1, 3),
        x: 600,
        image_ms: None,
    },
    Case {
        logic: "DoNothing",
        image_set: "LEVEL_ARCHES",
        frames: (1, 5),
        x: 1450,
        image_ms: None,
    },
    Case {
        logic: "GooCoverup",
        image_set: "LEVEL_GOOCOVERUP",
        frames: (1, 2),
        x: 1500,
        image_ms: None,
    },
];

/// A long flat floor (its top at y = 960) with every case on it; Claw
/// stands at x = 1000, clear of all of them. The level knows the loops the
/// logics start and every picture.
fn level() -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 880));
    for (key, ms) in [
        (decor::CYCLE_ANIM, 100),
        (pickup::COIN_ANIM, 100),
        (pickup::PICKUP_ANIM, 100),
        (pickup::GLITTER_ANIM, 100),
        (boss_stage::WARP_ANIM, 50),
    ] {
        l.animations
            .insert(key.to_string(), Arc::new(next_image(key, ms)));
    }
    for c in CASES {
        l.images.insert(
            c.image_set.to_string(),
            ImageFrames::range(c.frames.0, c.frames.1),
        );
        let mut s = ObjectSpec::at(c.logic, c.x, 930, 0);
        s.image_set = c.image_set.to_string();
        s.frame = c.frames.0;
        s.record[0] = c.x;
        l.objects.push(s);
    }
    l.images
        .insert(pickup::GLITTER_IMAGE.to_string(), ImageFrames::range(1, 16));
    l
}

fn find(sim: &Sim, logic: &str, image_set: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && o.image_set == image_set)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} showing {image_set}"))
}

/// The image after `f` in a set of frames `first..=last`.
fn after(f: i32, (first, last): (i32, i32)) -> i32 {
    if f >= last { first } else { f + 1 }
}

/// Runs `ticks` ticks and returns, per object, the tick and frame of every
/// image change.
fn run(sim: &mut Sim, ids: &[ObjectId], ticks: usize) -> Vec<Vec<(usize, i32)>> {
    let mut last: Vec<i32> = ids
        .iter()
        .map(|&id| sim.objects().get(id).expect("exists").frame)
        .collect();
    let mut out = vec![Vec::new(); ids.len()];
    for t in 1..=ticks {
        sim.step(TickInput::default());
        for (i, &id) in ids.iter().enumerate() {
            let f = sim.objects().get(id).expect("still there").frame;
            if f != last[i] {
                out[i].push((t, f));
                last[i] = f;
            }
        }
    }
    out
}

#[test]
fn looping_pictures_step_through_their_set_and_still_ones_stay() {
    let mut sim = Sim::new(level(), SimConfig::default());
    let ids: Vec<ObjectId> = CASES
        .iter()
        .map(|c| find(&sim, c.logic, c.image_set))
        .collect();
    let starts: Vec<i32> = CASES.iter().map(|c| c.frames.0).collect();
    // 3 s of play.
    let seen = run(&mut sim, &ids, 3000 / TICK_MS as usize);
    for (i, c) in CASES.iter().enumerate() {
        let what = format!("{} {}", c.logic, c.image_set);
        let Some(ms) = c.image_ms else {
            assert!(seen[i].is_empty(), "{what} changed: {:?}", seen[i]);
            continue;
        };
        let calls = calls_per_image(ms);
        let set = (c.frames.1 - c.frames.0 + 1) as usize;
        assert!(
            seen[i].len() >= 2 * set,
            "{what}: twice around its set in 3 s: {:?}",
            seen[i]
        );
        let mut f = starts[i];
        for (k, &(t, frame)) in seen[i].iter().enumerate() {
            assert_eq!(frame, after(f, c.frames), "{what}: change {k} at tick {t}");
            if k > 0 {
                assert_eq!(
                    t - seen[i][k - 1].0,
                    calls,
                    "{what}: change {k} at tick {t}"
                );
            }
            f = frame;
        }
    }
}

#[test]
fn every_pickup_glitter_sparkles() {
    let mut sim = Sim::new(level(), SimConfig::default());
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let glitters: Vec<ObjectId> = sim
        .objects()
        .all()
        .filter(|(_, o)| &*o.logic == pickup::GLITTER)
        .map(|(id, _)| id)
        .collect();
    // The ring, the health and the extra life glitter; the coin does not.
    assert_eq!(glitters.len(), 3);
    let seen = run(&mut sim, &glitters, 3000 / TICK_MS as usize);
    let mut shown: BTreeMap<ObjectId, Vec<i32>> = BTreeMap::new();
    for (i, &g) in glitters.iter().enumerate() {
        let o = sim.objects().get(g).expect("still there");
        assert_eq!(o.image_set, pickup::GLITTER_IMAGE);
        assert_eq!(o.anim.key(), Some(pickup::GLITTER_ANIM));
        shown.insert(g, seen[i].iter().map(|&(_, f)| f).collect());
        assert!(seen[i].len() >= 16, "glitter {g}: {:?}", seen[i]);
        let mut f = 1;
        for (k, &(t, frame)) in seen[i].iter().enumerate() {
            assert_eq!(
                frame,
                after(f, (1, 16)),
                "glitter {g}: change {k} at tick {t}"
            );
            if k > 0 {
                assert_eq!(t - seen[i][k - 1].0, calls_per_image(100));
            }
            f = frame;
        }
    }
    for (g, frames) in shown {
        for f in 1..=16 {
            assert!(frames.contains(&f), "glitter {g}: frame {f} never shown");
        }
    }
}
