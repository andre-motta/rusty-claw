//! Crates, powder kegs and pickups on synthetic levels: what crates drop
//! and how stacks behave, kegs set off by blasts and carried by Claw,
//! curses, the treasure census, the flight of taken treasure and the
//! values each pickup offers Claw.

use std::collections::BTreeMap;
use std::sync::Arc;

use claw_sim::anim::{AnimFrame, Animation};
use claw_sim::container::CollisionEvent;
use claw_sim::level::{ImageFrames, ObjectSpec};
use claw_sim::logic::effect::{EffectData, EffectKind};
use claw_sim::logic::pickup::{self, PickupData, state as pstate};
use claw_sim::logic::{self, ObjectData, crates, keg};
use claw_sim::object::{LogicKind, Object, ObjectId, Rect, flags, special, types};
use claw_sim::physics::ptype;
use claw_sim::rng::Rng;
use claw_sim::{GameLogic, Sim, SimConfig, SimLevel, TickInput, TileMap};

/// Top of the floor: feet rest on y = 959.
const FLOOR: i32 = 960;

/// A long flat floor with `objects` placed on it; Claw stands at x = 1000.
fn level(objects: Vec<ObjectSpec>) -> SimLevel {
    level_with_rows(objects, &[])
}

/// Like [`level`], with deadly tiles in the floor's top row at the given
/// columns.
fn level_with_rows(objects: Vec<ObjectSpec>, deadly: &[usize]) -> SimLevel {
    let mut rows = vec![".".repeat(200); 15];
    let mut floor: Vec<u8> = "#".repeat(200).into_bytes();
    for &c in deadly {
        floor[c] = b'x';
    }
    rows.push(String::from_utf8(floor).unwrap());
    rows.push("#".repeat(200));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut l = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 880));
    l.objects = objects;
    l
}

fn spec(logic: &str, image_set: &str, x: i32, y: i32) -> ObjectSpec {
    let mut s = ObjectSpec::at(logic, x, y, 0);
    s.image_set = image_set.to_string();
    s.frame = 1;
    s.record[0] = x;
    s
}

fn crate_with(logic: &str, x: i32, powerup: i32) -> ObjectSpec {
    let mut s = spec(logic, "LEVEL_CRATES", x, 925);
    s.record[15] = powerup;
    s
}

fn find(sim: &Sim, logic: &str) -> Vec<ObjectId> {
    sim.objects()
        .all()
        .filter(|(_, o)| &*o.logic == logic)
        .map(|(id, _)| id)
        .collect()
}

fn obj(sim: &Sim, id: ObjectId) -> &Object {
    sim.objects().get(id).expect("object exists")
}

fn step(sim: &mut Sim, n: usize) {
    for _ in 0..n {
        sim.step(TickInput::default());
    }
}

fn claw(sim: &mut Sim) -> ObjectId {
    sim.with_world(|w| w.claw_id.unwrap())
}

fn data(o: &Object) -> &PickupData {
    pickup::data_of(o).expect("pickup data")
}

fn hit(sim: &mut Sim, id: ObjectId, attacker: ObjectId) {
    sim.with_world(|w| logic::on_hit(w, id, attacker));
}

fn sounds(audio: &[claw_sim::claw_frame::AudioCue]) -> Vec<String> {
    audio
        .iter()
        .filter_map(|c| match c {
            claw_sim::claw_frame::AudioCue::Play(s) => Some(s.key.clone()),
            _ => None,
        })
        .collect()
}

/// Explosion requests made by `source`.
fn explosions(sim: &Sim, source: ObjectId) -> Vec<(i32, i32, i32, i32)> {
    sim.objects()
        .all()
        .filter_map(|(_, o)| match o.data {
            ObjectData::Effect(EffectData {
                request:
                    Some(EffectKind::Explosion {
                        damage,
                        radius,
                        source: Some(s),
                    }),
            }) if s == source => Some((o.x, o.y, damage, radius)),
            _ => None,
        })
        .collect()
}

/// A one-shot animation of `n` 60 ms elements whose third reports event 1.
fn blast_anim(key: &str, n: i32) -> Animation {
    let mut frames: Vec<AnimFrame> = (1..=n)
        .map(|i| AnimFrame {
            image_op: 3,
            image_index: i,
            duration: 60,
            event: i32::from(i == 3),
            ..AnimFrame::default()
        })
        .collect();
    frames.last_mut().unwrap().next_op = 9;
    Animation {
        key: key.to_string(),
        frames,
        ..Animation::default()
    }
}

/// The score number's rise: 75 ms elements moving up 2, 2, 2, then 1 five
/// times.
fn points_anim() -> Animation {
    let mut frames: Vec<AnimFrame> = [-2, -2, -2, -1, -1, -1, -1, -1]
        .iter()
        .map(|&dy| AnimFrame {
            move_op: 2,
            operand_y: dy,
            duration: 75,
            ..AnimFrame::default()
        })
        .collect();
    frames.last_mut().unwrap().next_op = 9;
    Animation {
        key: "GAME_POINTS".to_string(),
        frames,
        ..Animation::default()
    }
}

// ------------------------------------------------------------ crates

#[test]
fn a_crate_breaks_and_drops_each_kind_of_content() {
    // (code in the crate, code dropped, type once it can be taken, logic
    // it settles into; None when it removes itself)
    let cases: [(i32, Option<i32>, u32, Option<&str>); 13] = [
        (0, Some(0), types::TREASURE, Some("TreasurePowerup")),
        (-5, Some(0), types::TREASURE, Some("TreasurePowerup")),
        (7, Some(7), types::TREASURE, Some("TreasurePowerup")),
        (24, Some(24), types::HEALTH, Some("HealthPowerup")),
        (22, Some(22), types::HEALTH, Some("HealthPowerup")),
        (21, Some(21), types::PISTOL_AMMO, Some("AmmoPowerup")),
        (34, Some(34), types::SPECIAL, Some("AmmoPowerup")),
        (28, Some(28), types::MAGIC_AMMO, Some("MagicPowerup")),
        (55, Some(55), types::SPECIAL, Some("SpecialPowerup")),
        (32, Some(32), types::SPECIAL, Some("SpecialPowerup")),
        (31, Some(31), types::SPECIAL, Some("EndOfLevelPowerup")),
        (38, Some(38), types::SPECIAL, None),
        (62, None, 0, None),
    ];
    for (content, dropped, kind, settles) in cases {
        let mut sim = Sim::new(
            level(vec![crate_with("FrontCrate", 1200, content)]),
            SimConfig::default(),
        );
        step(&mut sim, 2);
        let c = find(&sim, "FrontCrate")[0];
        let o = obj(&sim, c);
        assert_eq!(o.state, crates::state::IDLE);
        assert_eq!((o.type_flags, o.hit_type_flags), (0x200, 0x300_0072));
        assert_eq!(o.hit_rect, crates::CRATE_BOX);
        assert_eq!(o.z, 5000);
        assert_eq!(o.health, 1);
        let rng_before = sim.rng().clone();
        let claw = claw(&mut sim);
        hit(&mut sim, c, claw);
        let o = obj(&sim, c);
        assert!(o.flags & flags::SAFE != 0, "{content}: no second break");
        assert_eq!(o.state, crates::state::BREAKING);
        // The break sound is the only random draw.
        let mut r = rng_before;
        let pick = r.below(100);
        assert_eq!(sim.rng(), &r, "{content}");
        let goodies = find(&sim, pickup::BOUNCING_GOODIE);
        let Some(code) = dropped else {
            assert!(goodies.is_empty(), "{content}: nothing past the table");
            continue;
        };
        assert_eq!(goodies.len(), 1, "{content}");
        let g = goodies[0];
        let o = obj(&sim, g);
        assert_eq!((o.x, o.y, o.z), (1200, 925 - 24, 4999), "{content}");
        assert_eq!(data(o).drop_code, Some(code));
        assert_eq!(
            o.image_set,
            pickup::powerup(code).unwrap().image_set,
            "{content}"
        );
        assert!(o.flags & flags::NO_COLLIDE != 0, "not yet collectable");
        let snap = sim.step(TickInput::default());
        let want = if pick < 50 {
            "GAME_CRATEBREAK"
        } else {
            "GAME_CRATEBREAK2"
        };
        assert!(sounds(&snap.audio).contains(&want.to_string()), "{content}");
        // Collectable 125 ms after it appeared, with the table's values.
        step(&mut sim, 6);
        let o = obj(&sim, g);
        assert!(o.flags & flags::NO_COLLIDE == 0, "{content}");
        assert_eq!(o.type_flags, kind, "{content}");
        assert_eq!(o.attack_rect, pickup::GOODIE_BOX);
        assert_eq!(data(o).points, pickup::powerup(code).unwrap().points);
        // The crate is gone once its (missing) break animation ends.
        assert!(sim.objects().get(c).is_none(), "{content}");
        // It bounces, then settles into its ordinary pickup.
        step(&mut sim, 300);
        match settles {
            Some(logic) => {
                let o = obj(&sim, g);
                assert_eq!(&*o.logic, logic, "{content}");
                assert_eq!(o.state, pstate::IDLE, "{content}");
                assert_eq!(o.type_flags, kind, "{content}");
                assert!(o.y < FLOOR && o.y > 900, "{content}: rests near the floor");
            }
            None => assert!(sim.objects().get(g).is_none(), "{content}: a curse goes"),
        }
    }
}

#[test]
fn goodies_bounce_four_times_then_settle() {
    let mut sim = Sim::new(
        level(vec![crate_with("BehindCrate", 1200, 1)]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let c = find(&sim, "BehindCrate")[0];
    assert_eq!(obj(&sim, c).z, 1100);
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let g = find(&sim, pickup::BOUNCING_GOODIE)[0];
    let mut bounces = 0;
    let mut ticks = 0;
    while &*obj(&sim, g).logic == pickup::BOUNCING_GOODIE {
        let snap = sim.step(TickInput::default());
        bounces += sounds(&snap.audio)
            .iter()
            .filter(|s| *s == "GAME_PUBOUNCE1")
            .count();
        ticks += 1;
        assert!(ticks < 500);
    }
    assert_eq!(bounces, 4);
    let o = obj(&sim, g);
    assert_eq!(&*o.logic, "TreasurePowerup");
    assert_eq!(data(o).kind, 1);
    assert_eq!(data(o).drop_code, Some(1));
    assert!(o.flags & flags::ALWAYS_ACTIVE == 0);
    assert_eq!(o.attack_rect, pickup::PADDED_BOX);
    // Resting just above the floor, it stays put.
    let at = (o.x, o.y);
    step(&mut sim, 50);
    assert_eq!((obj(&sim, g).x, obj(&sim, g).y), at);
}

#[test]
fn goodies_on_deadly_tiles_flash_and_go() {
    let mut sim = Sim::new(
        level_with_rows(vec![crate_with("FrontCrate", 1200, 0)], &[18, 19]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let c = find(&sim, "FrontCrate")[0];
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let g = find(&sim, pickup::BOUNCING_GOODIE)[0];
    let mut flashed = false;
    for _ in 0..200 {
        step(&mut sim, 1);
        match sim.objects().get(g) {
            Some(o) => flashed |= o.state == pstate::GOODIE_DYING,
            None => break,
        }
    }
    assert!(flashed);
    assert!(sim.objects().get(g).is_none());
}

#[test]
fn two_hit_and_unbreakable_crates() {
    let mut two = crate_with("FrontCrate", 1200, 0);
    two.record[18] = 5;
    let mut never = crate_with("BehindCrate", 1300, 0);
    never.record[18] = 999;
    let mut sim = Sim::new(level(vec![two, never]), SimConfig::default());
    step(&mut sim, 2);
    let (c, n) = (find(&sim, "FrontCrate")[0], find(&sim, "BehindCrate")[0]);
    assert_eq!(obj(&sim, c).health, 2);
    let o = obj(&sim, n);
    assert!(o.flags & flags::SAFE != 0 && o.flags & flags::NO_COLLIDE != 0);
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let o = obj(&sim, c);
    assert_eq!((o.health, o.frame, o.state), (1, 2, crates::state::IDLE));
    let snap = sim.step(TickInput::default());
    assert_eq!(sounds(&snap.audio), vec!["GAME_CRATEDAMAGE"]);
    // Hits within 500 ms do nothing.
    hit(&mut sim, c, claw);
    assert_eq!(obj(&sim, c).health, 1);
    step(&mut sim, 28);
    hit(&mut sim, c, claw);
    assert_eq!(obj(&sim, c).state, crates::state::BREAKING);
    assert_eq!(find(&sim, pickup::BOUNCING_GOODIE).len(), 1);
}

#[test]
fn a_pistol_shot_breaking_a_crate_is_used_up() {
    let mut sim = Sim::new(
        level(vec![crate_with("FrontCrate", 1200, 0)]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let c = find(&sim, "FrontCrate")[0];
    let mut shot = Object::new(0, 1190, 920, 4000, "PistolBullet".into(), LogicKind::Inert);
    shot.flags = flags::WORLD;
    shot.type_flags = types::PISTOL_SHOT;
    shot.attack_rect = Rect::new(-4, -4, 4, 4);
    let s = sim.spawn(shot);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, c).state, crates::state::BREAKING);
    assert!(
        obj(&sim, s).flags & flags::DESTROY != 0,
        "the shot is used up"
    );
    step(&mut sim, 1);
    assert!(sim.objects().get(s).is_none());
}

// ------------------------------------------------------------ stacks

fn stack(logic: &str, x: i32, powerup: i32, first: [i32; 4], second: [i32; 4]) -> ObjectSpec {
    let mut s = spec(logic, "LEVEL_CRATES", x, 925);
    s.record[15] = powerup;
    s.record[35..39].copy_from_slice(&first);
    s.record[39..43].copy_from_slice(&second);
    s
}

fn stack_crates(sim: &Sim) -> Vec<ObjectId> {
    let mut ids = find(sim, crates::STACKED_CRATE);
    ids.sort_by_key(|&id| -obj(sim, id).y);
    ids
}

fn content(o: &Object) -> i32 {
    match &o.data {
        ObjectData::Crate(d) => d.content,
        _ => panic!("not a crate"),
    }
}

#[test]
fn stack_contents_follow_the_record_fields() {
    let r = |p: i32, a: [i32; 4], b: [i32; 4]| {
        let mut r = [0; 71];
        r[15] = p;
        r[35..39].copy_from_slice(&a);
        r[39..43].copy_from_slice(&b);
        crates::stack_contents(&r)
    };
    assert_eq!(r(0, [33, 2, 33, 0], [0; 4]), vec![33, 2, 33]);
    assert_eq!(r(7, [41, 0, 0, 9], [0, 0, 0, 1]), vec![7, 41, 9, 1]);
    assert_eq!(r(7, [0, 3, 0, 0], [0; 4]), vec![7, 3]);
    assert_eq!(
        r(0, [0; 4], [0; 4]),
        vec![0],
        "the bottom crate is always built"
    );
    assert_eq!(r(0, [0, 5, 0, 0], [0; 4]), vec![0, 5]);
}

#[test]
fn a_stack_builds_its_crates_and_goes() {
    let mut sim = Sim::new(
        level(vec![stack(
            "BackStackedCrates",
            1200,
            0,
            [33, 2, 41, 0],
            [0; 4],
        )]),
        SimConfig::default(),
    );
    let spawner = find(&sim, "BackStackedCrates")[0];
    step(&mut sim, 2);
    assert!(
        sim.objects().get(spawner).is_none(),
        "the spawner removes itself"
    );
    let ids = stack_crates(&sim);
    assert_eq!(ids.len(), 3);
    for (i, &id) in ids.iter().enumerate() {
        let o = obj(&sim, id);
        let i = i as i32;
        assert_eq!((o.x, o.y, o.z), (1200, 925 - 40 * i, 1000 + i + 1));
        assert_eq!(o.image_set, "LEVEL_CRATES");
        assert_eq!(o.hit_rect, crates::STACKED_BOX);
        assert_eq!((o.type_flags, o.hit_type_flags), (0x200, 0x300_0072));
        assert!(o.is_listed());
    }
    assert_eq!(
        ids.iter()
            .map(|&id| content(obj(&sim, id)))
            .collect::<Vec<_>>(),
        vec![33, 2, 41]
    );
    // The front stack's default depth.
    let mut sim = Sim::new(
        level(vec![stack("FrontStackedCrates", 1200, 1, [0; 4], [0; 4])]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let ids = stack_crates(&sim);
    assert_eq!(ids.len(), 1);
    assert_eq!(obj(&sim, ids[0]).z, 5001);
}

#[test]
fn breaking_a_stacked_crate_drops_its_item_and_lets_the_ones_above_fall() {
    let mut sim = Sim::new(
        level(vec![stack(
            "BackStackedCrates",
            1200,
            0,
            [33, 2, 41, 0],
            [0; 4],
        )]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let ids = stack_crates(&sim);
    let claw = claw(&mut sim);
    // Claw stays out of the drops, so every item is still there to count.
    sim.with_world(|w| w.objects.get_mut(claw).unwrap().hit_type_flags = 0);
    // Middle crate: one item with a random hop, then the break sound.
    let mut r: Rng = sim.rng().clone();
    let vx = f64::from(r.below(401) as i32 - 200);
    let vy = f64::from(-(r.below(301) as i32 + 200));
    let pick = r.below(100);
    hit(&mut sim, ids[1], claw);
    assert_eq!(sim.rng(), &r);
    let goodies = find(&sim, pickup::BOUNCING_GOODIE);
    assert_eq!(goodies.len(), 1);
    let o = obj(&sim, goodies[0]);
    assert_eq!((o.x, o.y, o.z), (1200, 885, pickup::GOODIE_Z));
    assert_eq!(data(o).drop_code, Some(2));
    let g = data(o).goodie.unwrap();
    assert_eq!((g.vx, g.vy, g.gravity), (vx, vy, 800.0));
    let snap = sim.step(TickInput::default());
    let heard = sounds(&snap.audio);
    assert!(heard.contains(&"GAME_PURELEASE1".to_string()));
    let want = if pick < 50 {
        "GAME_CRATEBREAK"
    } else {
        "GAME_CRATEBREAK2"
    };
    assert!(heard.contains(&want.to_string()));
    // A second hit on the breaking crate does nothing.
    hit(&mut sim, ids[1], claw);
    assert_eq!(find(&sim, pickup::BOUNCING_GOODIE).len(), 1);
    // The top crate falls 40 px onto the bottom one.
    let mut lowest = 0;
    let mut bounced = false;
    for _ in 0..60 {
        step(&mut sim, 1);
        let y = obj(&sim, ids[2]).y;
        if y < lowest {
            bounced = true;
        }
        lowest = lowest.max(y);
    }
    assert!(sim.objects().get(ids[1]).is_none(), "the broken crate goes");
    assert_eq!(obj(&sim, ids[2]).y, 925 - 40);
    assert_eq!(lowest, 925 - 40, "it never sinks into the crate below");
    assert!(bounced, "landing at 400 px/s it bounces once");
    // Now the bottom one: the last crate falls to the bottom.
    hit(&mut sim, ids[0], claw);
    let dropped: Vec<_> = find(&sim, pickup::BOUNCING_GOODIE)
        .into_iter()
        .map(|g| data(obj(&sim, g)).drop_code.unwrap())
        .collect();
    assert_eq!(dropped, vec![2, 33]);
    step(&mut sim, 60);
    assert_eq!(obj(&sim, ids[2]).y, 925);
}

#[test]
fn an_empty_stacked_crate_drops_nothing() {
    let mut sim = Sim::new(
        level(vec![stack(
            "BackStackedCrates",
            1200,
            0,
            [0, 5, 0, 0],
            [0; 4],
        )]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let ids = stack_crates(&sim);
    assert_eq!(content(obj(&sim, ids[0])), 0);
    let claw = claw(&mut sim);
    let mut r = sim.rng().clone();
    r.below(100);
    hit(&mut sim, ids[0], claw);
    assert_eq!(sim.rng(), &r, "only the break sound draws");
    assert!(find(&sim, pickup::BOUNCING_GOODIE).is_empty());
    let snap = sim.step(TickInput::default());
    assert!(!sounds(&snap.audio).contains(&"GAME_PURELEASE1".to_string()));
}

#[test]
fn a_warp_in_a_stack_keeps_its_destination() {
    let mut s = stack("FrontStackedCrates", 1200, 32, [0; 4], [0; 4]);
    s.record[55] = 4000;
    s.record[56] = 300;
    let mut sim = Sim::new(level(vec![s]), SimConfig::default());
    step(&mut sim, 2);
    let c = stack_crates(&sim)[0];
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let g = find(&sim, pickup::BOUNCING_GOODIE)[0];
    assert_eq!((obj(&sim, g).speed_x, obj(&sim, g).speed_y), (4000, 300));
    step(&mut sim, 300);
    let o = obj(&sim, g);
    assert_eq!(&*o.logic, "SpecialPowerup");
    assert_eq!(data(o).points, pickup::WARP);
    assert_eq!((o.speed_x, o.speed_y), (4000, 300));
}

#[test]
fn drop_loot_fans_several_items_out() {
    let mut sim = Sim::new(level(vec![]), SimConfig::default());
    let mut r = sim.rng().clone();
    let slots = [33, 0, 7, 0, 0, 0, 0, 0, 21];
    let ids = sim.with_world(|w| pickup::drop_loot(w, 1300, 900, slots));
    assert_eq!(ids.len(), 3);
    // n = 3: the spread starts at -37 and steps 25 per slot, zero or not.
    let want_vx = [-37.0, 13.0, 163.0];
    for (i, &id) in ids.iter().enumerate() {
        let g = data(obj(&sim, id)).goodie.unwrap();
        assert_eq!(g.vx, want_vx[i]);
        assert_eq!(g.vy, f64::from(-(r.below(301) as i32 + 200)));
    }
    assert_eq!(sim.rng(), &r, "one draw per item when there are several");
    assert_eq!(
        ids.iter()
            .map(|&id| data(obj(&sim, id)).drop_code.unwrap())
            .collect::<Vec<_>>(),
        vec![33, 7, 21]
    );
}

// ------------------------------------------------------------ kegs

fn keg_level(explode_anim: bool) -> SimLevel {
    let mut l = level(vec![spec("PowderKeg", "LEVEL_POWDERKEG", 1200, 890)]);
    if explode_anim {
        l.animations.insert(
            keg::ANIM_EXPLODE.into(),
            Arc::new(blast_anim(keg::ANIM_EXPLODE, 6)),
        );
    }
    l
}

fn blast_object(x: i32, y: i32) -> Object {
    let mut o = Object::new(0, x, y, 5000, "Explosion".into(), LogicKind::Inert);
    o.flags = flags::WORLD;
    o.type_flags = types::LEVEL_SHOT;
    o.damage = 20;
    o.attack_rect = Rect::new(-60, -60, 60, 60);
    o
}

#[test]
fn a_keg_hit_by_a_blast_requests_exactly_one_explosion() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let o = obj(&sim, k);
    assert!(o.special_flags & special::LIFTABLE != 0);
    assert_eq!(o.hit_type_flags, types::PISTOL_SHOT | types::LEVEL_SHOT);
    assert_eq!(o.damage, 20);
    // A blast that stays around for a while hits it only once.
    sim.spawn(blast_object(1200, 930));
    let snap = sim.step(TickInput::default());
    assert!(sounds(&snap.audio).contains(&keg::BLAST_SOUND.to_string()));
    let d = keg::data_of(obj(&sim, k)).unwrap();
    assert_eq!(d.phase, keg::Phase::Exploding);
    assert!(
        explosions(&sim, k).is_empty(),
        "the blast waits for its frame"
    );
    let mut ticks = 1;
    while sim.objects().get(k).is_some() {
        step(&mut sim, 1);
        ticks += 1;
        assert!(ticks < 100);
    }
    assert_eq!(explosions(&sim, k), vec![(1205, 905, 20, 50)]);
}

#[test]
fn the_blast_comes_with_the_explosion_frame() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let claw = claw(&mut sim);
    hit(&mut sim, k, claw);
    // Each 60 ms element lasts four 18 ms calls: the third starts on the
    // ninth call.
    let mut first = None;
    for t in 1..=20 {
        step(&mut sim, 1);
        if first.is_none() && !explosions(&sim, k).is_empty() {
            first = Some(t);
        }
    }
    assert_eq!(first, Some(9));
}

#[test]
fn without_an_explosion_animation_the_keg_blasts_at_once() {
    let mut sim = Sim::new(keg_level(false), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let mut shot = Object::new(0, 1200, 930, 4000, "PistolBullet".into(), LogicKind::Inert);
    shot.flags = flags::WORLD;
    shot.type_flags = types::PISTOL_SHOT;
    shot.attack_rect = Rect::new(-4, -4, 4, 4);
    let s = sim.spawn(shot);
    step(&mut sim, 1);
    assert!(
        obj(&sim, s).flags & flags::DESTROY != 0,
        "the shot is used up"
    );
    assert_eq!(explosions(&sim, k).len(), 1);
    step(&mut sim, 1);
    assert!(sim.objects().get(k).is_none());
    assert!(sim.objects().get(s).is_none());
}

#[test]
fn claws_blows_do_not_touch_a_keg() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let mut blow = Object::new(0, 1200, 930, 4000, "Blow".into(), LogicKind::Inert);
    blow.flags = flags::WORLD;
    blow.type_flags = types::PLAYER;
    blow.attack_rect = Rect::new(-60, -60, 60, 60);
    sim.spawn(blow);
    step(&mut sim, 20);
    assert_eq!(keg::data_of(obj(&sim, k)).unwrap().phase, keg::Phase::Idle);
}

#[test]
fn a_lifted_keg_follows_claw_and_explodes_where_it_lands() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let claw = claw(&mut sim);
    let (cx, cy) = {
        let p = sim.player().unwrap();
        (p.x, p.y)
    };
    sim.with_world(|w| logic::lifted(w, k, claw));
    let o = obj(&sim, k);
    let d = keg::data_of(o).unwrap();
    assert_eq!((d.phase, d.carrier), (keg::Phase::Lifted, Some(claw)));
    assert!(o.flags & flags::SAFE != 0 && o.flags & flags::ALWAYS_ACTIVE != 0);
    let set_frame = |sim: &mut Sim, f: i32| {
        sim.with_world(|w| w.objects.get_mut(claw).unwrap().frame = f);
        sim.with_world(|w| logic::tick(w, k));
    };
    set_frame(&mut sim, 42);
    assert_eq!((obj(&sim, k).x, obj(&sim, k).y), (cx + 30, cy - 70));
    set_frame(&mut sim, 43);
    assert_eq!((obj(&sim, k).x, obj(&sim, k).y), (cx - 10, cy - 105));
    assert_eq!(
        keg::data_of(obj(&sim, k)).unwrap().phase,
        keg::Phase::Overhead
    );
    // Facing left, the keg is on the other side and mirrored.
    sim.with_world(|w| w.claw.facing_right = false);
    sim.with_world(|w| logic::tick(w, k));
    assert_eq!((obj(&sim, k).x, obj(&sim, k).y), (cx + 10, cy - 105));
    assert!(obj(&sim, k).draw_flags & claw_sim::object::draw_flags::MIRROR != 0);
    // Claw starts the throw: the keg leans back and down.
    sim.with_world(|w| w.objects.get_mut(claw).unwrap().state = 5001);
    sim.with_world(|w| logic::tick(w, k));
    assert_eq!((obj(&sim, k).x, obj(&sim, k).y), (cx + 14, cy - 100));
    // Thrown: it flies under gravity, lands and explodes.
    sim.with_world(|w| w.claw.facing_right = true);
    sim.with_world(|w| logic::thrown(w, k, 350, -700));
    let o = obj(&sim, k);
    assert_eq!(o.type_flags, types::THROWN);
    assert_eq!(o.physics_type, ptype::FLY);
    let start_x = o.x;
    let mut ticks = 0;
    while keg::data_of(obj(&sim, k)).unwrap().phase == keg::Phase::Thrown {
        step(&mut sim, 1);
        ticks += 1;
        assert!(ticks < 200);
    }
    let o = obj(&sim, k);
    assert!(
        o.x > start_x + 100,
        "it flew forward: {} -> {}",
        start_x,
        o.x
    );
    assert_eq!(o.y + keg::KEG_BOX.bottom, FLOOR - 1, "landed on the floor");
    step(&mut sim, 40);
    assert_eq!(explosions(&sim, k).len(), 1);
    assert!(sim.objects().get(k).is_none());
}

#[test]
fn a_dropped_keg_falls_and_explodes() {
    let mut sim = Sim::new(keg_level(false), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let claw = claw(&mut sim);
    sim.with_world(|w| logic::lifted(w, k, claw));
    sim.with_world(|w| w.objects.get_mut(claw).unwrap().frame = 43);
    step(&mut sim, 1);
    sim.with_world(|w| logic::dropped(w, k));
    assert_eq!(
        keg::data_of(obj(&sim, k)).unwrap().phase,
        keg::Phase::Dropped
    );
    for _ in 0..60 {
        step(&mut sim, 1);
    }
    assert_eq!(explosions(&sim, k).len(), 1);
    assert!(sim.objects().get(k).is_none());
}

#[test]
fn a_thrown_keg_explodes_on_the_first_enemy_it_strikes() {
    let mut sim = Sim::new(keg_level(false), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let mut enemy = Object::new(0, 1260, 900, 3000, "Enemy".into(), LogicKind::Inert);
    enemy.flags = flags::WORLD;
    enemy.type_flags = types::ENEMY;
    enemy.hit_type_flags = types::THROWN;
    enemy.hit_rect = Rect::new(-20, -60, 20, 60);
    sim.spawn(enemy);
    // Up and forward: it would land some 360 px on.
    sim.with_world(|w| logic::thrown(w, k, 600, -600));
    let mut ticks = 0;
    while explosions(&sim, k).is_empty() {
        step(&mut sim, 1);
        ticks += 1;
        assert!(ticks < 30, "it hits the enemy before landing");
    }
    let (bx, ..) = explosions(&sim, k)[0];
    assert!(bx - keg::BLAST_OFFSET.0 < 1300, "it went off at the enemy");
}

// ------------------------------------------------------------ curses

#[test]
fn every_curse_of_the_first_level_does_nothing_in_single_player() {
    let sets = [
        "GAME_CURSES_LIFE",
        "GAME_CURSES_HEALTH",
        "GAME_CURSES_AMMO",
        "GAME_CURSES_TREASURE",
        "GAME_CURSES_FREEZE",
    ];
    let objects = sets
        .iter()
        .enumerate()
        .map(|(i, s)| spec("CursePowerup", s, 1100 + 40 * i as i32, 930))
        .collect();
    let mut sim = Sim::new(level(objects), SimConfig::default());
    let ids = find(&sim, "CursePowerup");
    assert_eq!(ids.len(), 5);
    let snap = sim.step(TickInput::default());
    let snap2 = sim.step(TickInput::default());
    for id in ids {
        assert!(sim.objects().get(id).is_none(), "removed on its first call");
    }
    for s in [&snap, &snap2] {
        assert!(
            s.renderables
                .iter()
                .all(|r| !r.image_set_key.starts_with("GAME_CURSES")),
            "never drawn"
        );
    }
    // Their effects (ids 9905 to 9910) never take effect.
    let stats = sim.stats().clone();
    let rng = sim.rng().clone();
    let count = sim.objects().len();
    for id in 9905..=9910 {
        assert!(!sim.with_world(|w| pickup::curse_effect(w, id)), "{id}");
    }
    assert_eq!(sim.stats(), &stats);
    assert_eq!(sim.rng(), &rng);
    assert_eq!(sim.objects().len(), count);
}

// ------------------------------------------------------------ census

#[test]
fn the_census_counts_treasure_by_kind() {
    let mut officer = spec("Officer", "LEVEL_OFFICER", 3000, 900);
    officer.record[15] = 45;
    officer.record[35] = 33;
    officer.record[36] = 33;
    officer.record[39] = 49;
    let mut unknown = spec("NotALogic", "GAME_TREASURE_COINS", 3000, 900);
    unknown.record[15] = 1;
    let mut outside = spec("Officer", "LEVEL_OFFICER", 99_999, 900);
    outside.record[15] = 1;
    let mut path = spec("PathElevator", "LEVEL_ELEVATOR", 3000, 900);
    path.record[15] = 1;
    let objects = vec![
        spec("GlitterlessPowerup", "GAME_TREASURE_COINS", 2000, 900),
        spec("GlitterlessPowerup", "GAME_TREASURE_COINS", 2040, 900),
        spec("TreasurePowerup", "GAME_TREASURE_CROSSES_RED", 2080, 900),
        spec("TreasurePowerup", "GAME_TREASURE_NECKLACE", 2120, 900),
        spec("TreasurePowerup", "GAME_TREASURE_GECKOS_BLUE", 2120, 900),
        spec("HealthPowerup", "LEVEL_HEALTH", 2160, 900),
        spec("SpecialPowerup", "GAME_MAPPIECE", 2160, 900),
        spec("CursePowerup", "GAME_CURSES_LIFE", 2160, 900),
        crate_with("FrontCrate", 2200, 0),
        crate_with("BehindCrate", 2240, 7),
        crate_with("FrontCrate", 2280, 6),
        crate_with("FrontCrate", 2280, 24),
        stack("BackStackedCrates", 2320, 0, [33, 2, 41, 0], [0; 4]),
        stack("FrontStackedCrates", 2360, 11, [0, 0, 15, 0], [0, 47, 0, 0]),
        officer,
        unknown,
        outside,
        path,
    ];
    let l = level(objects);
    // coin, goldbar, ring, chalice, cross, scepter, gecko, crown, skull
    let want = [6, 0, 1, 1, 2, 1, 2, 2, 1];
    assert_eq!(pickup::census(&l), want);
    let sim = Sim::new(l, SimConfig::default());
    assert_eq!(sim.treasure_census(), want);
}

// ------------------------------------------------------------ flights

#[test]
fn taken_treasure_flies_to_the_score_corner_and_goes() {
    let mut l = level(vec![spec(
        "TreasurePowerup",
        "GAME_TREASURE_CROSSES_RED",
        1250,
        930,
    )]);
    l.animations
        .insert("GAME_POINTS".into(), Arc::new(points_anim()));
    let mut sim = Sim::new(l, SimConfig::default());
    step(&mut sim, 3);
    let t = find(&sim, "TreasurePowerup")[0];
    let o = obj(&sim, t);
    assert_eq!(o.state, pstate::IDLE);
    assert_eq!(o.z, 1000);
    let glitter = data(o).glitter.expect("treasure glitters");
    let g = obj(&sim, glitter);
    assert_eq!(&*g.logic, pickup::GLITTER);
    assert_eq!((g.x, g.y, g.z), (1250, 930, 1001));
    assert_eq!(g.image_set, "GAME_GLITTER");
    assert!(g.flags & flags::NO_COLLIDE != 0);
    // Claw's side takes it.
    sim.with_world(|w| pickup::send_to_hud(w.objects.get_mut(t).unwrap()));
    assert!(obj(&sim, t).flags & flags::NO_COLLIDE != 0);
    step(&mut sim, 1);
    assert_eq!(obj(&sim, t).state, pstate::FLIGHT_START);
    let view = sim.view().rect;
    step(&mut sim, 1);
    let o = obj(&sim, t);
    assert_eq!(o.state, pstate::FLYING);
    let f = data(o).flight.unwrap();
    let target = (view.left + 20, view.top - 40);
    assert_eq!(f.target, target);
    assert_eq!(
        (f.vx, f.vy),
        (
            (target.0 - 1250) * 1000 / 400,
            (target.1 - 930) * 1000 / 400
        )
    );
    let icons = find(&sim, pickup::POINTS_ICON);
    assert_eq!(icons.len(), 1);
    let icon = obj(&sim, icons[0]);
    assert_eq!((icon.x, icon.y, icon.z, icon.frame), (1250, 930, 8900, 5));
    assert_eq!(icon.image_set, "GAME_POINTS");
    // The flight: 23 moves, the last of which ends the 400 ms.
    let mut moves = 0;
    let mut last = (1250, 930);
    while let Some(o) = sim.objects().get(t) {
        last = (o.x, o.y);
        step(&mut sim, 1);
        moves += 1;
        assert!(moves < 50);
    }
    assert_eq!(moves, 23);
    assert!(sim.objects().get(glitter).is_none(), "the glitter went");
    let near = |a: i32, b: i32| (a - b).abs() <= 40;
    assert!(
        near(last.0, target.0) && near(last.1, target.1),
        "{last:?} vs {target:?}"
    );
    // The score number rises 11 px and goes.
    let icon_id = icons[0];
    let mut top = 930;
    while let Some(o) = sim.objects().get(icon_id) {
        top = o.y;
        step(&mut sim, 1);
    }
    assert_eq!(top, 930 - 11);
}

#[test]
fn an_extra_life_flies_to_the_lives_corner_without_a_score() {
    let mut sim = Sim::new(
        level(vec![spec(
            "SpecialPowerup",
            "GAME_POWERUPS_EXTRALIFE",
            1150,
            930,
        )]),
        SimConfig::default(),
    );
    step(&mut sim, 3);
    let e = find(&sim, "SpecialPowerup")[0];
    assert_eq!(data(obj(&sim, e)).points, pickup::EXTRA_LIFE);
    sim.with_world(|w| pickup::send_to_hud(w.objects.get_mut(e).unwrap()));
    step(&mut sim, 1);
    let view = sim.view().rect;
    step(&mut sim, 1);
    let f = data(obj(&sim, e)).flight.unwrap();
    assert_eq!(f.target, (view.right + 1 - 20, view.top + 40));
    assert!(find(&sim, pickup::POINTS_ICON).is_empty());
    step(&mut sim, 23);
    assert!(sim.objects().get(e).is_none());
}

#[test]
fn a_goodie_taken_in_the_air_flies_from_where_it_was() {
    let mut sim = Sim::new(
        level(vec![crate_with("FrontCrate", 1200, 11)]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let c = find(&sim, "FrontCrate")[0];
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let g = find(&sim, pickup::BOUNCING_GOODIE)[0];
    step(&mut sim, 8);
    sim.with_world(|w| pickup::send_to_hud(w.objects.get_mut(g).unwrap()));
    step(&mut sim, 1);
    assert_eq!(&*obj(&sim, g).logic, "TreasurePowerup");
    step(&mut sim, 2);
    assert_eq!(obj(&sim, g).state, pstate::FLYING);
    step(&mut sim, 23);
    assert!(sim.objects().get(g).is_none());
}

// ------------------------------------------------------------ values

#[test]
fn each_pickup_offers_its_kind_and_amount() {
    // (logic, picture, type, code, amount, sound, padded, glitters)
    #[allow(clippy::type_complexity)]
    let cases: [(&str, &str, u32, i32, i32, &str, bool, bool); 16] = [
        (
            "GlitterlessPowerup",
            "GAME_TREASURE_COINS",
            types::TREASURE,
            33,
            100,
            "GAME_COIN",
            true,
            false,
        ),
        (
            "GlitterlessPowerup",
            "GAME_TREASURE_RINGS_BLUE",
            types::TREASURE,
            4,
            1500,
            "GAME_RINGS",
            true,
            false,
        ),
        (
            "TreasurePowerup",
            "GAME_TREASURE_JEWELEDSKULL_GREEN",
            types::TREASURE,
            50,
            25000,
            "GAME_PICKUP1",
            true,
            true,
        ),
        (
            "TreasurePowerup",
            "GAME_SOMETHING_ELSE",
            types::TREASURE,
            0,
            100,
            "",
            true,
            true,
        ),
        (
            "HealthPowerup",
            "LEVEL_HEALTH",
            types::HEALTH,
            24,
            5,
            "GAME_FOODITEM",
            false,
            true,
        ),
        (
            "HealthPowerup",
            "GAME_HEALTH_POTION1",
            types::HEALTH,
            26,
            10,
            "GAME_MILK",
            false,
            true,
        ),
        (
            "HealthPowerup",
            "GAME_HEALTH_POTION2",
            types::HEALTH,
            27,
            15,
            "GAME_MILK",
            false,
            true,
        ),
        (
            "HealthPowerup",
            "GAME_HEALTH_POTION3",
            types::HEALTH,
            25,
            25,
            "GAME_MILK",
            false,
            true,
        ),
        (
            "HealthPowerup",
            "GAME_CATNIPS_NIP2",
            types::HEALTH,
            23,
            30000,
            "GAME_CATNMAG",
            false,
            true,
        ),
        (
            "AmmoPowerup",
            "GAME_AMMO_SHOTBAG",
            types::PISTOL_AMMO,
            21,
            10,
            "GAME_AMMUNITION",
            false,
            true,
        ),
        (
            "AmmoPowerup",
            "GAME_DYNAMITE",
            types::SPECIAL,
            34,
            9904,
            "GAME_AMMUNITION",
            false,
            true,
        ),
        (
            "MagicPowerup",
            "GAME_MAGICCLAW",
            types::MAGIC_AMMO,
            30,
            35,
            "GAME_MAGICPOWERUP",
            false,
            true,
        ),
        (
            "SpecialPowerup",
            "GAME_WARP",
            types::SPECIAL,
            32,
            pickup::WARP,
            "GAME_MAGICPOWERUP",
            true,
            false,
        ),
        (
            "SpecialPowerup",
            "GAME_POWERUPS_EXTRALIFE",
            types::SPECIAL,
            55,
            pickup::EXTRA_LIFE,
            "GAME_EXTRALIFE",
            true,
            true,
        ),
        (
            "SpecialPowerup",
            "GAME_MAPPIECE",
            types::SPECIAL,
            31,
            pickup::END_OF_LEVEL,
            "GAME_MAGICPOWERUP",
            true,
            true,
        ),
        (
            "SpecialPowerup",
            "GAME_POWERUPS_GHOST",
            types::SPECIAL,
            53,
            9911,
            "GAME_PICKUP1",
            true,
            true,
        ),
    ];
    for (logic, set, kind, code, amount, sound, padded, glitters) in cases {
        let mut s = spec(logic, set, 1200, 930);
        // The level's own amount is ignored; its duration value is kept.
        s.record[14] = 777;
        s.record[17] = 12000;
        s.record[55] = 5000;
        s.record[56] = 600;
        let mut sim = Sim::new(level(vec![s]), SimConfig::default());
        step(&mut sim, 3);
        let id = find(&sim, logic)[0];
        let o = obj(&sim, id);
        let d = data(o);
        assert_eq!(o.state, pstate::IDLE, "{set}");
        assert_eq!(o.type_flags, kind, "{set}");
        assert_eq!((d.kind, d.points, d.sound), (code, amount, sound), "{set}");
        assert_eq!(d.smarts, 12000, "{set}");
        assert_eq!((o.speed_x, o.speed_y), (5000, 600), "{set}");
        let rect = if padded {
            pickup::PADDED_BOX
        } else {
            pickup::FRAME_BOX
        };
        assert_eq!(o.attack_rect, rect, "{set}");
        assert_eq!(d.glitter.is_some(), glitters, "{set}");
        assert!(o.flags & flags::NO_COLLIDE == 0, "{set}");
        assert!(o.flags & flags::NOTIFY_DESTROY != 0, "{set}");
        // Claw is hit by every kind of pickup.
        assert!(kind & types::CLAW_HIT_BY != 0, "{set}");
    }
}

#[test]
fn claw_meets_pickups_through_pair_collision() {
    let mut sim = Sim::new(
        level(vec![spec(
            "TreasurePowerup",
            "GAME_TREASURE_COINS",
            1000,
            930,
        )]),
        SimConfig::default(),
    );
    let mut hits = 0;
    for _ in 0..3 {
        step(&mut sim, 1);
        let t = find(&sim, "TreasurePowerup")[0];
        hits += sim
            .collision_events()
            .iter()
            .filter(|e| {
                matches!(e, claw_sim::container::CollisionEvent::Hit { attacker, .. } if *attacker == t)
            })
            .count();
    }
    // He takes it at the first touch, so it touches him once.
    assert_eq!(hits, 1, "the treasure touches Claw");
}

// ------------------------------------------------------------ determinism

fn digest(sim: &Sim) -> String {
    let mut out = BTreeMap::new();
    for (id, o) in sim.objects().all() {
        if matches!(
            o.kind,
            LogicKind::Game(GameLogic::Pickup | GameLogic::Crate | GameLogic::Keg)
        ) || &*o.logic == "Explosion"
        {
            out.insert(
                id,
                format!(
                    "{}:{}:{}:{}:{}:{}:{:?}",
                    o.logic, o.x, o.y, o.z, o.state, o.frame, o.data
                ),
            );
        }
    }
    format!("{} {:?}", sim.rng().state(), out)
}

fn scenario() -> Vec<String> {
    let mut l = level(vec![
        crate_with("FrontCrate", 1200, 5),
        stack("BackStackedCrates", 1300, 0, [33, 2, 41, 0], [0; 4]),
        spec("PowderKeg", "LEVEL_POWDERKEG", 1100, 890),
        spec("TreasurePowerup", "GAME_TREASURE_COINS", 1350, 930),
    ]);
    l.animations.insert(
        keg::ANIM_EXPLODE.into(),
        Arc::new(blast_anim(keg::ANIM_EXPLODE, 6)),
    );
    let mut sim = Sim::new(l, SimConfig::default());
    let claw = claw(&mut sim);
    let mut out = Vec::new();
    for t in 0..400 {
        match t {
            5 => {
                let c = find(&sim, "FrontCrate")[0];
                hit(&mut sim, c, claw);
            }
            8 => {
                for c in find(&sim, crates::STACKED_CRATE) {
                    hit(&mut sim, c, claw);
                }
            }
            12 => {
                sim.spawn(blast_object(1100, 930));
            }
            _ => {}
        }
        sim.step(TickInput::default());
        out.push(digest(&sim));
    }
    out
}

#[test]
fn crates_kegs_and_goodies_run_the_same_every_time() {
    let a = scenario();
    let b = scenario();
    assert_eq!(a, b);
    // A cloned simulation continues identically.
    let mut sim = Sim::new(
        level(vec![stack(
            "BackStackedCrates",
            1300,
            0,
            [33, 2, 41, 0],
            [0; 4],
        )]),
        SimConfig::default(),
    );
    step(&mut sim, 2);
    let claw = claw(&mut sim);
    let c = find(&sim, crates::STACKED_CRATE)[0];
    hit(&mut sim, c, claw);
    let mut twin = sim.clone();
    for _ in 0..200 {
        sim.step(TickInput::default());
        twin.step(TickInput::default());
        assert_eq!(digest(&sim), digest(&twin));
    }
}

// ------------------------------------------------------------ level details

/// Registers the image set `set` with frames `first..=last` and the given
/// frame boxes in `l`.
fn register(l: &mut SimLevel, set: &str, (first, last): (i32, i32), boxes: &[(i32, Rect)]) {
    let mut frames = ImageFrames::range(first, last);
    frames.boxes.extend(boxes.iter().copied());
    l.images.insert(set.to_string(), frames);
}

#[test]
fn a_thrown_keg_flies_past_an_idle_keg() {
    let mut l = keg_level(false);
    l.objects
        .push(spec("PowderKeg", "LEVEL_POWDERKEG", 1240, 890));
    let mut sim = Sim::new(l, SimConfig::default());
    step(&mut sim, 2);
    let kegs = find(&sim, "PowderKeg");
    let (k, idle) = (kegs[0], kegs[1]);
    // Thrown up and forward, it passes through the lying keg as it rises
    // and lands some 360 px on.
    sim.with_world(|w| logic::thrown(w, k, 600, -600));
    let mut struck = false;
    let mut ticks = 0;
    while explosions(&sim, k).is_empty() {
        step(&mut sim, 1);
        struck |= sim.collision_events().iter().any(|e| {
            matches!(*e, CollisionEvent::Attack { attacker, victim } if attacker == k && victim == idle)
        });
        assert_eq!(
            keg::data_of(obj(&sim, idle)).unwrap().phase,
            keg::Phase::Idle,
            "the lying keg is not set off"
        );
        ticks += 1;
        assert!(ticks < 60, "the thrown keg lands");
    }
    assert!(struck, "the thrown keg touched the lying one");
    let (bx, ..) = explosions(&sim, k)[0];
    assert!(
        bx - keg::BLAST_OFFSET.0 > 1500,
        "it went off where it landed, at {bx}"
    );
}

#[test]
fn a_keg_blast_shakes_the_screen_for_half_a_second() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    let claw = claw(&mut sim);
    hit(&mut sim, k, claw);
    let mut shaken = None;
    for _ in 0..20 {
        let now = 18 * sim.tick();
        step(&mut sim, 1);
        if !explosions(&sim, k).is_empty() {
            shaken = Some(now);
            break;
        }
        assert_eq!(sim.camera().shake_until_ms, 0, "no shake before the blast");
    }
    let now = shaken.expect("the blast came");
    assert_eq!(
        sim.camera().shake_until_ms,
        now + u64::from(keg::BLAST_SHAKE_MS)
    );
}

#[test]
fn a_kill_of_kind_two_sets_a_keg_off_and_other_kills_remove_it() {
    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    assert!(sim.with_world(|w| w.kill(k, keg::KILL_EXPLODE)));
    let snap = sim.step(TickInput::default());
    assert!(sounds(&snap.audio).contains(&keg::BLAST_SOUND.to_string()));
    assert_eq!(
        keg::data_of(obj(&sim, k)).unwrap().phase,
        keg::Phase::Exploding
    );
    step(&mut sim, 60);
    assert_eq!(explosions(&sim, k).len(), 1);
    assert!(sim.objects().get(k).is_none());

    let mut sim = Sim::new(keg_level(true), SimConfig::default());
    step(&mut sim, 2);
    let k = find(&sim, "PowderKeg")[0];
    sim.with_world(|w| w.kill(k, 1));
    let snap = sim.step(TickInput::default());
    assert!(!sounds(&snap.audio).contains(&keg::BLAST_SOUND.to_string()));
    step(&mut sim, 2);
    assert!(sim.objects().get(k).is_none());
    assert!(explosions(&sim, k).is_empty());
}

#[test]
fn pickup_and_crate_boxes_come_from_the_pictures() {
    let coin = Rect::new(-8, -9, 7, 8);
    let food = Rect::new(-12, -10, 11, 9);
    let crate_box = Rect::new(-30, -40, 29, 39);
    let mut l = level(vec![
        spec("TreasurePowerup", "GAME_TREASURE_COINS", 1200, 900),
        spec("HealthPowerup", "LEVEL_HEALTH", 1300, 900),
        crate_with("FrontCrate", 1100, 0),
    ]);
    register(&mut l, "GAME_TREASURE_COINS", (1, 8), &[(1, coin)]);
    register(&mut l, "LEVEL_HEALTH", (1, 1), &[(1, food)]);
    register(&mut l, "LEVEL_CRATES", (1, 3), &[(1, crate_box)]);
    let mut sim = Sim::new(l, SimConfig::default());
    step(&mut sim, 2);
    let t = find(&sim, "TreasurePowerup")[0];
    assert_eq!(obj(&sim, t).attack_rect, pickup::padded(coin));
    assert_eq!(pickup::padded(coin), Rect::new(-14, -19, 13, 10));
    let h = find(&sim, "HealthPowerup")[0];
    assert_eq!(obj(&sim, h).attack_rect, food, "health is not padded");
    let c = find(&sim, "FrontCrate")[0];
    assert_eq!((obj(&sim, c).hit_rect, obj(&sim, c).frame), (crate_box, 1));
    // The crate's coin comes out showing the set's first image, -1, and
    // touches with that image's box once it can be taken.
    let claw = claw(&mut sim);
    hit(&mut sim, c, claw);
    let g = find(&sim, pickup::BOUNCING_GOODIE)[0];
    assert_eq!(obj(&sim, g).frame, -1);
    step(&mut sim, 10);
    assert_eq!(obj(&sim, g).attack_rect, pickup::goodie_box(coin));
}

#[test]
fn a_damaged_crate_shows_the_next_image_of_its_set() {
    assert_eq!(crates::next_frame(3, Some((3, 4))), 4);
    assert_eq!(crates::next_frame(4, Some((3, 4))), 3, "wraps to the first");
    assert_eq!(crates::next_frame(9, Some((3, 4))), 3);
    assert_eq!(crates::next_frame(1, Some((3, 4))), 3);
    assert_eq!(crates::next_frame(7, None), 8);
    // A two-hit crate placed on another image starts on the set's first,
    // and in a set of one image a hit keeps it.
    for ((first, last), placed, damaged) in [((3, 4), 1, 4), ((7, 7), 1, 7)] {
        let mut two = crate_with("FrontCrate", 1200, 0);
        two.record[18] = 5;
        two.frame = placed;
        let mut l = level(vec![two]);
        register(&mut l, "LEVEL_CRATES", (first, last), &[]);
        register(&mut l, "GAME_TREASURE_COINS", (1, 8), &[]);
        let mut sim = Sim::new(l, SimConfig::default());
        step(&mut sim, 2);
        let c = find(&sim, "FrontCrate")[0];
        assert_eq!(obj(&sim, c).frame, first);
        let claw = claw(&mut sim);
        hit(&mut sim, c, claw);
        let o = obj(&sim, c);
        assert_eq!((o.health, o.frame), (1, damaged));
    }
}

#[test]
fn an_item_whose_picture_the_level_lacks_is_not_dropped() {
    let mut l = level(Vec::new());
    register(&mut l, "GAME_TREASURE_COINS", (1, 8), &[]);
    register(&mut l, "LEVEL_GEM", (1, 1), &[]);
    let mut sim = Sim::new(l, SimConfig::default());
    sim.with_world(|w| {
        assert!(pickup::spawn_drop(w, 1100, 800, 0).is_some());
        assert!(pickup::spawn_drop(w, 1100, 800, -5).is_some(), "a coin");
        assert_eq!(pickup::spawn_drop(w, 1100, 800, 1), None, "no gold bars");
        assert_eq!(pickup::spawn_drop(w, 1100, 800, 62), None);
        // Level 1 shows the map piece as GAME_MAPPIECE, which it lacks.
        assert_eq!(
            pickup::spawn_drop(w, 1100, 800, pickup::MAP_PIECE_CODE),
            None
        );
    });
    // A level made in code reports no image sets and drops everything.
    let mut sim = Sim::new(level(Vec::new()), SimConfig::default());
    sim.with_world(|w| assert!(pickup::spawn_drop(w, 1100, 800, 1).is_some()));
}

#[test]
fn the_map_piece_picture_follows_the_level_number() {
    for (number, picture, other) in [
        (1, "GAME_MAPPIECE", "LEVEL_GEM"),
        (2, "LEVEL_GEM", "GAME_MAPPIECE"),
        (13, "LEVEL_GEM", "GAME_MAPPIECE"),
    ] {
        let mut l = level(vec![
            spec("EndOfLevelPowerup", picture, 1200, 900),
            spec("EndOfLevelPowerup", other, 1300, 900),
        ]);
        l.number = number;
        let mut sim = Sim::new(l, SimConfig::default());
        step(&mut sim, 2);
        let ids = find(&sim, "EndOfLevelPowerup");
        let (right, wrong) = (data(obj(&sim, ids[0])), data(obj(&sim, ids[1])));
        assert_eq!(
            (right.kind, right.points),
            (pickup::MAP_PIECE_CODE, pickup::END_OF_LEVEL),
            "level {number}"
        );
        assert_eq!((wrong.kind, wrong.points), (0, 100), "level {number}");
        let g = sim
            .with_world(|w| pickup::spawn_drop(w, 1100, 800, pickup::MAP_PIECE_CODE))
            .unwrap();
        assert_eq!(obj(&sim, g).image_set, picture, "level {number}");
    }
}
