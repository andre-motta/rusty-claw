//! Placed pickups cycle through their pictures on the first level of the
//! user's own install. The test reads the folder named by the
//! `CLAW_INSTALL` environment variable at run time and skips itself when
//! it is unset; nothing from an install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_sim::logic::{self, pickup};
use claw_sim::object::{ObjectId, Rect};
use claw_sim::tuning::TICK_MS;
use claw_sim::{Sim, SimConfig, SimLevel};

fn store() -> Option<AssetStore> {
    let Some(root) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("CLAW_INSTALL is not set; skipping");
        return None;
    };
    let install =
        Install::open(PathBuf::from(root)).expect("CLAW_INSTALL does not point at an install");
    Some(install.store().expect("mounting the install"))
}

fn level1(store: &AssetStore) -> SimLevel {
    let data = LevelData::build(store, LevelSource::Retail(1)).unwrap();
    SimLevel::from_level(&data, store).unwrap()
}

/// Calls between two images of a 100 ms loop.
const CALLS_PER_IMAGE: usize = 100usize.div_ceil(TICK_MS as usize);

#[test]
fn the_pickup_loops_show_the_next_image_every_100_ms() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    for key in [pickup::COIN_ANIM, pickup::PICKUP_ANIM] {
        let a = l
            .animations
            .get(key)
            .unwrap_or_else(|| panic!("{key} is read with the level"));
        assert_eq!(a.frames.len(), 1, "{key}");
        let e = a.frames[0];
        assert_eq!((e.image_op, e.duration), (1, 100), "{key}");
        assert!(matches!(e.next_op & 0xFFFF, 0 | 8), "{key} loops");
    }
}

/// Runs `calls` logic calls of `id` and returns the frame after each.
fn frames(sim: &mut Sim, id: ObjectId, calls: usize) -> Vec<i32> {
    (0..calls)
        .map(|_| {
            sim.with_world(|w| logic::tick(w, id));
            sim.objects().get(id).expect("still there").frame
        })
        .collect()
}

#[test]
fn a_placed_coin_cycles_through_its_set() {
    let Some(store) = store() else { return };
    let l = level1(&store);
    let (first, last) = l
        .image_frames("GAME_TREASURE_COINS")
        .expect("the level knows the coin pictures");
    assert!(last > first, "the coin has several pictures");
    let mut sim = Sim::new(l, SimConfig::default());
    let coin = sim
        .objects()
        .all()
        .find(|(_, o)| {
            pickup::NAMES.contains(&&*o.logic)
                && o.image_set == "GAME_TREASURE_COINS"
                && o.state == 0
        })
        .map(|(id, _)| id)
        .expect("the first level places coins");
    // Its first call sets it up.
    let start = frames(&mut sim, coin, 1)[0];
    let o = sim.objects().get(coin).unwrap();
    assert_eq!(o.state, pickup::state::IDLE);
    assert_eq!(o.anim.key(), Some(pickup::COIN_ANIM));
    let rect: Rect = o.attack_rect;
    let set = (last - first + 1) as usize;
    let seen = frames(&mut sim, coin, 2 * set * CALLS_PER_IMAGE);
    // The next image every 100 ms, around the whole set and back.
    let mut changes = Vec::new();
    let mut prev = start;
    for (i, &f) in seen.iter().enumerate() {
        if f != prev {
            let want = if prev >= last || prev < first {
                first
            } else {
                prev + 1
            };
            assert_eq!(f, want, "call {i}");
            changes.push(i);
            prev = f;
        }
    }
    assert!(changes.len() >= 2 * set - 1, "{changes:?}");
    for w in changes.windows(2) {
        assert_eq!(w[1] - w[0], CALLS_PER_IMAGE, "{changes:?}");
    }
    for f in first..=last {
        assert!(seen.contains(&f), "frame {f} never shown");
    }
    // The touch box stays the one it was set up with.
    assert_eq!(sim.objects().get(coin).unwrap().attack_rect, rect);
}
