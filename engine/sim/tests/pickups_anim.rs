//! Pickups keep cycling through their pictures on synthetic levels: placed
//! treasure and powerups loop their set from their first call on, and an
//! item a crate drops goes on spinning from the frame it reached once it
//! lands.

use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::pickup::{self, state as pstate};
use claw_sim::logic::{self, crates};
use claw_sim::object::{Object, ObjectId, Rect};
use claw_sim::tuning::TICK_MS;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput, TileMap};

/// Calls between two images of a 100 ms loop: the first call at or past
/// 100 ms after the image was shown.
const CALLS_PER_IMAGE: usize = 100usize.div_ceil(TICK_MS as usize);

/// A long flat floor (its top at y = 960) with `objects` on it; Claw
/// stands at x = 1000. The level knows the two pickup loops, the pictures
/// of the objects below and their frame boxes.
fn level(objects: Vec<ObjectSpec>) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    rows.push("#".repeat(200));
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 880));
    l.objects = objects;
    for key in [pickup::COIN_ANIM, pickup::PICKUP_ANIM] {
        l.animations
            .insert(key.to_string(), Arc::new(next_image(key)));
    }
    register(&mut l, "GAME_TREASURE_COINS", (1, 8));
    register(&mut l, "GAME_TREASURE_RINGS_RED", (1, 5));
    register(&mut l, "LEVEL_HEALTH", (1, 3));
    register(&mut l, "LEVEL_CRATES", (1, 3));
    l
}

/// A loop of one element that shows the next image of the set every
/// 100 ms.
fn next_image(key: &str) -> Animation {
    Animation {
        key: key.to_string(),
        frames: vec![AnimFrame {
            image_op: 1,
            duration: 100,
            ..AnimFrame::default()
        }],
        ..Animation::default()
    }
}

/// The frame box of frame `f`: a different size for every frame.
fn frame_box(f: i32) -> Rect {
    Rect::new(-8 - f, -9 - f, 7 + f, 8 + f)
}

fn register(l: &mut SimLevel, set: &str, (first, last): (i32, i32)) {
    let mut frames = ImageFrames::range(first, last);
    frames
        .boxes
        .extend((first..=last).map(|f| (f, frame_box(f))));
    l.images.insert(set.to_string(), frames);
}

fn spec(logic: &str, image_set: &str, x: i32, y: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at(logic, x, y, 0);
    s.image_set = image_set.to_string();
    s.frame = 1;
    s.record[0] = x;
    s
}

fn find(sim: &Sim, logic: &str, image_set: &str) -> ObjectId {
    sim.objects()
        .all()
        .find(|(_, o)| &*o.logic == logic && o.image_set == image_set)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no {logic} showing {image_set}"))
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("object exists")
}

/// The image after `f` in a set of frames `first..=last`.
fn after(f: i32, (first, last): (i32, i32)) -> i32 {
    if f >= last { first } else { f + 1 }
}

/// Every image change of the objects `ids` over `n` ticks, as (tick,
/// frame) per object.
fn changes(sim: &mut Sim, ids: &[ObjectId], n: usize) -> Vec<Vec<(usize, i32)>> {
    let mut last: Vec<i32> = ids.iter().map(|&id| obj(sim, id).frame).collect();
    let mut out = vec![Vec::new(); ids.len()];
    for t in 1..=n {
        sim.step(TickInput::default());
        for (i, &id) in ids.iter().enumerate() {
            let f = obj(sim, id).frame;
            if f != last[i] {
                out[i].push((t, f));
                last[i] = f;
            }
        }
    }
    out
}

/// Checks that `seen` steps to the next image, wrapping around `range`,
/// every [`CALLS_PER_IMAGE`] ticks, starting after `from`.
fn assert_cycles(what: &str, from: i32, seen: &[(usize, i32)], range: (i32, i32)) {
    let mut f = from;
    for (i, &(t, frame)) in seen.iter().enumerate() {
        assert_eq!(frame, after(f, range), "{what}: change {i} at tick {t}");
        if i > 0 {
            assert_eq!(
                t - seen[i - 1].0,
                CALLS_PER_IMAGE,
                "{what}: change {i} at tick {t}"
            );
        }
        f = frame;
    }
}

#[test]
fn coins_take_the_cycle_and_other_pickups_the_powerup_loop() {
    assert_eq!(
        pickup::idle_anim("TreasurePowerup", "GAME_TREASURE_COINS"),
        "GAME_CYCLE100"
    );
    assert_eq!(
        pickup::idle_anim("GlitterlessPowerup", "GAME_TREASURE_COINS"),
        "GAME_CYCLE100"
    );
    assert_eq!(
        pickup::idle_anim("TreasurePowerup", "GAME_TREASURE_RINGS_RED"),
        "GAME_POWERUP"
    );
    assert_eq!(
        pickup::idle_anim("SpecialPowerup", "GAME_POWERUPS_EXTRALIFE"),
        "GAME_POWERUP"
    );
    for logic in ["HealthPowerup", "MagicPowerup", "AmmoPowerup"] {
        assert_eq!(
            pickup::idle_anim(logic, "GAME_TREASURE_COINS"),
            "GAME_POWERUP",
            "{logic} always takes the powerup loop"
        );
    }
    assert!(
        logic::runtime_anim_prefixes()
            .iter()
            .any(|p| "GAME_POWERUP".starts_with(p)),
        "the powerup loop is read with the level"
    );
}

#[test]
fn placed_pickups_loop_their_pictures_every_100_ms() {
    let cases = [
        (
            "TreasurePowerup",
            "GAME_TREASURE_COINS",
            (1, 8),
            "GAME_CYCLE100",
        ),
        (
            "TreasurePowerup",
            "GAME_TREASURE_RINGS_RED",
            (1, 5),
            "GAME_POWERUP",
        ),
        ("HealthPowerup", "LEVEL_HEALTH", (1, 3), "GAME_POWERUP"),
    ];
    let mut sim = Sim::new(
        level(vec![
            spec(cases[0].0, cases[0].1, 1150, 930),
            spec(cases[1].0, cases[1].1, 1250, 930),
            spec(cases[2].0, cases[2].1, 1350, 930),
        ]),
        SimConfig::default(),
    );
    let ids: Vec<ObjectId> = cases
        .iter()
        .map(|&(logic, set, _, _)| find(&sim, logic, set))
        .collect();
    let n = 20 * CALLS_PER_IMAGE;
    let seen = changes(&mut sim, &ids, n);
    for (i, &(logic, set, range, anim)) in cases.iter().enumerate() {
        let what = format!("{logic} {set}");
        let o = obj(&sim, ids[i]);
        assert_eq!(o.state, pstate::IDLE, "{what}");
        assert_eq!(o.anim.key(), Some(anim), "{what}");
        assert!(seen[i].len() >= 19, "{what}: {:?}", seen[i]);
        assert_cycles(&what, 1, &seen[i], range);
        // It went all the way around its set at least once.
        let shown: Vec<i32> = seen[i].iter().map(|&(_, f)| f).collect();
        for f in range.0..=range.1 {
            assert!(shown.contains(&f), "{what}: frame {f} never shown");
        }
        // The touch box is the one of the frame it showed on its first
        // call, whatever it shows now.
        let want = if logic == "HealthPowerup" {
            frame_box(1)
        } else {
            pickup::padded(frame_box(1))
        };
        assert_eq!(o.attack_rect, want, "{what}");
    }
}

#[test]
fn a_pickup_without_its_loop_in_the_level_keeps_its_image() {
    let mut l = level(vec![spec(
        "TreasurePowerup",
        "GAME_TREASURE_RINGS_RED",
        1250,
        930,
    )]);
    l.animations.clear();
    let mut sim = Sim::new(l, SimConfig::default());
    let id = find(&sim, "TreasurePowerup", "GAME_TREASURE_RINGS_RED");
    let seen = changes(&mut sim, &[id], 10 * CALLS_PER_IMAGE);
    assert!(seen[0].is_empty(), "{:?}", seen[0]);
    assert_eq!(obj(&sim, id).state, pstate::IDLE);
}

/// Breaks a crate holding `code` and follows the item it drops until
/// `follow` ticks past the moment it becomes an ordinary pickup. Returns
/// the item, the tick it settled on, its frame then, and every image
/// change.
fn drop_and_follow(code: i32, follow: usize) -> (Sim, ObjectId, usize, i32, Vec<(usize, i32)>) {
    let mut c = spec("FrontCrate", "LEVEL_CRATES", 1200, 925);
    c.record[15] = code;
    let mut sim = Sim::new(level(vec![c]), SimConfig::default());
    for _ in 0..2 {
        sim.step(TickInput::default());
    }
    let crate_id = find(&sim, "FrontCrate", "LEVEL_CRATES");
    assert_eq!(obj(&sim, crate_id).state, crates::state::IDLE);
    let claw = sim.with_world(|w| w.claw_id.unwrap());
    sim.with_world(|w| logic::on_hit(w, crate_id, claw));
    let g = sim
        .objects()
        .all()
        .find(|(_, o)| &*o.logic == pickup::BOUNCING_GOODIE)
        .map(|(id, _)| id)
        .expect("the crate drops an item");
    let mut frame = obj(&sim, g).frame;
    let mut seen = Vec::new();
    let mut settled = None;
    for t in 1..1000 {
        sim.step(TickInput::default());
        let o = obj(&sim, g);
        if o.frame != frame {
            seen.push((t, o.frame));
            frame = o.frame;
        }
        if settled.is_none() && &*o.logic != pickup::BOUNCING_GOODIE {
            settled = Some((t, o.frame));
        }
        if let Some((s, _)) = settled
            && t >= s + follow
        {
            break;
        }
    }
    let (at, frame) = settled.expect("the item settles");
    (sim, g, at, frame, seen)
}

#[test]
fn a_dropped_coin_goes_on_spinning_once_it_lands() {
    let (sim, g, at, _, seen) = drop_and_follow(0, 30 * CALLS_PER_IMAGE);
    let o = obj(&sim, g);
    assert_eq!(&*o.logic, "TreasurePowerup");
    assert_eq!(o.state, pstate::IDLE);
    assert_eq!(o.anim.key(), Some(pickup::GOODIE_ANIM));
    // One unbroken loop from the first image, through the landing, with
    // the same 100 ms pace on both sides of it.
    assert_cycles("dropped coin", 1, &seen, (1, 8));
    let later = seen.iter().filter(|&&(t, _)| t > at).count();
    assert!(later >= 29, "{later} images after it landed: {seen:?}");
}

#[test]
fn a_dropped_powerup_keeps_the_spin_it_flew_with() {
    // Food settles into a health pickup, which on its own would take the
    // powerup loop; it keeps the goodie's spin and its frame.
    let (sim, g, at, frame, seen) = drop_and_follow(24, 10 * CALLS_PER_IMAGE);
    let o = obj(&sim, g);
    assert_eq!(&*o.logic, "HealthPowerup");
    assert_eq!(o.anim.key(), Some(pickup::GOODIE_ANIM));
    assert_cycles("dropped food", 1, &seen, (1, 3));
    // The first image after it landed follows the one it landed with.
    let next = seen.iter().find(|&&(t, _)| t > at).expect("still animates");
    assert_eq!(next.1, after(frame, (1, 3)));
    // Its touch box is the one of the frame it landed with.
    let landed = seen
        .iter()
        .rev()
        .find(|&&(t, _)| t <= at)
        .map_or(1, |&(_, f)| f);
    assert_eq!(landed, frame);
    assert_eq!(o.attack_rect, frame_box(frame));
}
