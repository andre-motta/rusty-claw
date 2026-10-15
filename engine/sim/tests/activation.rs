//! Activation windows: the original sizes, the widened mode with its
//! breakable exceptions near both plane edges, the leave pass, and level
//! objects in the simulation.

mod common;

use std::sync::Arc;

use claw_sim::activation::{Activation, Centres};
use claw_sim::claw_frame::AspectPolicy;
use claw_sim::container::Container;
use claw_sim::level::ObjectSpec;
use claw_sim::object::{LogicKind, Object, ObjectId, flags};
use claw_sim::view::PlaneGeometry;
use claw_sim::{
    ActivationConfig, ActivationMode, Sim, SimConfig, SimLevel, TickInput, TileMap, ViewConfig,
};

const PLANE: PlaneGeometry = PlaneGeometry {
    width: 6400,
    height: 3200,
    wrap_x: false,
    wrap_y: false,
};

#[test]
fn original_half_extents() {
    let cfg = ActivationConfig::original();
    assert_eq!(cfg.half_extents(0, cfg.view), (800, 600));
    assert_eq!(cfg.half_extents(1, cfg.view), (1280, 960));
    assert_eq!(cfg.half_extents(2, cfg.view), (384, 288));
}

#[test]
fn widened_half_extents_keep_the_off_screen_margins() {
    for (w, expect) in [
        (854, [(907, 600), (1387, 960), (491, 288)]),
        (1120, [(1040, 600), (1520, 960), (624, 288)]),
        (1706, [(1333, 600), (1813, 960), (917, 288)]),
    ] {
        let cfg = ActivationConfig::widened((w, 480));
        for (class, e) in expect.iter().enumerate() {
            assert_eq!(cfg.half_extents(class, cfg.view), *e, "{w} class {class}");
        }
    }
    assert_eq!(ActivationConfig::widened((853, 480)).view, (854, 480));
}

fn object(logic: &str, class: usize, x: i32, y: i32) -> Object {
    let mut o = Object::new(0, x, y, 0, Arc::from(logic), LogicKind::Inert);
    o.flags = flags::WORLD
        | match class {
            1 => flags::CLASS_1,
            2 => flags::CLASS_2,
            _ => 0,
        };
    o
}

/// Files the objects dormant, runs one enter pass for camera `point`, and
/// returns which are live.
fn live_after_enter(cfg: ActivationConfig, objs: &[Object], point: (i32, i32)) -> Vec<bool> {
    let mut c = Container::new();
    let mut a = Activation::new(cfg.clone(), PLANE);
    let ids: Vec<ObjectId> = objs
        .iter()
        .map(|o| {
            let id = c.add(o.clone());
            a.file(c.get(id).unwrap(), id);
            id
        })
        .collect();
    a.enter(&mut c, Centres::new(&cfg, PLANE, point));
    ids.iter()
        .map(|&id| c.get(id).unwrap().is_listed())
        .collect()
}

#[test]
fn original_windows_near_both_plane_edges() {
    // Near the left edge the centre stays at 320; near the right at 6080;
    // near the top at 240 and near the bottom at 2960.
    let cfg = ActivationConfig::original();
    for (point, cx, cy) in [((100, 100), 320, 240), ((6390, 3190), 6080, 2960)] {
        let objs = [
            object("TreasurePowerup", 2, cx + 384, cy),
            object("TreasurePowerup", 2, cx + 385, cy),
            object("TreasurePowerup", 2, cx - 384, cy - 288),
            object("TreasurePowerup", 2, cx, cy + 289),
            object("Officer", 0, cx + 800, cy + 600),
            object("Officer", 0, cx - 801, cy),
            object("Elevator", 1, cx + 1280, cy - 960),
            object("Elevator", 1, cx, cy + 961),
        ];
        let live = live_after_enter(cfg.clone(), &objs, point);
        assert_eq!(
            live,
            vec![true, false, true, false, true, false, true, false],
            "camera {point:?}"
        );
    }
}

#[test]
fn widened_windows_with_breakable_exceptions_near_both_edges() {
    // 32:9 at 480 px: the wide view centre stays 853 px from the plane
    // edges, the original-view centre 320 px.
    let cfg = ActivationConfig::widened((1706, 480));
    assert_eq!(cfg.mode, ActivationMode::Widened);
    for (point, wide, orig) in [((100, 1600), 853, 320), ((6390, 1600), 6400 - 853, 6080)] {
        let dir = if point.0 < 3200 { 1 } else { -1 };
        let objs = [
            // A pickup: widened window around the wide centre.
            object("TreasurePowerup", 2, wide + dir * 917, 1600),
            object("TreasurePowerup", 2, wide + dir * 918, 1600),
            // A crate: the original window around the original centre.
            object("Crate", 2, orig + dir * 384, 1600),
            object("Crate", 2, orig + dir * 385, 1600),
            object("PowderKeg", 2, orig - dir * 384, 1600),
            // An enemy: widened.
            object("Officer", 0, wide + dir * 1333, 1600),
            object("Officer", 0, wide + dir * 1334, 1600),
        ];
        let live = live_after_enter(cfg.clone(), &objs, point);
        assert_eq!(
            live,
            vec![true, false, true, false, true, true, false],
            "camera {point:?}"
        );
    }
    // Away from the edges both centres are the camera point; a crate keeps
    // the 384 px window while a pickup gets 917 px.
    let objs = [
        object("BehindCrate", 2, 3000 + 385, 1600),
        object("TreasurePowerup", 2, 3000 + 917, 1600),
    ];
    assert_eq!(
        live_after_enter(cfg, &objs, (3000, 1600)),
        vec![false, true]
    );
}

#[test]
fn frozen_edge_simulates_like_the_original() {
    let objs: Vec<Object> = (0..40)
        .map(|i| object("TreasurePowerup", 2, 100 + i * 40, 1600))
        .collect();
    for point in [(100, 1600), (2000, 1600), (6390, 1600)] {
        assert_eq!(
            live_after_enter(ActivationConfig::original(), &objs, point),
            live_after_enter(ActivationConfig::frozen_edge(), &objs, point)
        );
    }
}

#[test]
fn leaving_puts_objects_to_sleep_where_they_left() {
    let cfg = ActivationConfig::original();
    let mut c = Container::new();
    let mut a = Activation::new(cfg.clone(), PLANE);
    let id = c.add(object("Rat", 0, 3000, 1600));
    let gone = c.add({
        let mut o = object("Bullet", 0, 3000, 1600);
        o.flags |= flags::DESTROY_OUTSIDE;
        o
    });
    for i in [id, gone] {
        a.file(c.get(i).unwrap(), i);
    }
    let here = Centres::new(&cfg, PLANE, (3000, 1600));
    a.enter(&mut c, here);
    assert!(c.get(id).unwrap().is_listed());
    // Move both out of their window.
    for i in [id, gone] {
        c.get_mut(i).unwrap().x = 3000 + 801;
    }
    let report = a.leave(&mut c, here, &mut |_, _, _| {});
    assert_eq!(report.slept, vec![id]);
    assert_eq!(report.destroyed, vec![gone]);
    assert!(c.get(gone).is_none());
    assert_eq!(c.get(id).unwrap().grid_point, (3801, 1600));
    // The camera moving toward its new point wakes it again.
    a.enter(&mut c, Centres::new(&cfg, PLANE, (3010, 1600)));
    assert!(c.get(id).unwrap().is_listed());
}

fn level_with_objects() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(100); 15];
    rows.push("#".repeat(100));
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
    let mut near = ObjectSpec::at("TreasurePowerup", 1200, 900, 0);
    near.image_set = "GAME_TREASURE".into();
    let far = ObjectSpec::at("TreasurePowerup", 5000, 900, 0);
    let ambient = ObjectSpec::at("GlobalAmbientSound", 6000, 900, 0);
    let mut always = ObjectSpec::at("AniCycle", 6000, 100, 0);
    always.record[10] = 2;
    let unknown = ObjectSpec::at("NotALogic", 1100, 900, 0);
    let empty = ObjectSpec::at("", 1100, 900, 0);
    let outside = ObjectSpec::at("Rat", 99_999, 900, 0);
    level.objects = vec![near, far, ambient, always, unknown, empty, outside];
    level
}

#[test]
fn level_objects_load_dormant_and_wake_near_the_camera() {
    let mut sim = Sim::new(level_with_objects(), SimConfig::default());
    // At load the windows are around the start point (y = 600), so the
    // pickup at y = 900 is still dormant; the camera then moves to Claw
    // and the next enter pass wakes it.
    assert_eq!(sim.activation().dormant_count(), 2);
    sim.step(TickInput::default());
    sim.step(TickInput::default());
    let d = sim.diagnostics();
    assert_eq!(d.dropped_records, 2);
    assert_eq!(d.unknown_logics.get("NotALogic"), Some(&1));
    let live: Vec<String> = sim
        .objects()
        .listed()
        .iter()
        .map(|&id| sim.objects().get(id).unwrap().logic.to_string())
        .collect();
    // The global ambient sound names no registered sound, so it removed
    // itself on its first call; the always-active AniCycle is live.
    assert!(!live.contains(&"GlobalAmbientSound".to_string()));
    assert!(live.contains(&"AniCycle".to_string()));
    assert!(live.contains(&"NotALogic".to_string()));
    assert_eq!(live.iter().filter(|l| *l == "TreasurePowerup").count(), 1);
    assert_eq!(sim.activation().dormant_count(), 1);
}

#[test]
fn frozen_edge_snapshots_draw_dormant_objects_in_the_wide_view() {
    // The 640x480 view around Claw spans x 680..=1319. One pickup sits
    // 100 px past its right edge, one 264 px past it; both are dormant.
    let mut level = level_with_objects();
    for x in [1320 + 100, 1320 + 264] {
        let mut side = ObjectSpec::at("TreasurePowerup", x, 900, 0);
        side.image_set = "GAME_TREASURE".into();
        level.objects.push(side);
    }
    let treasures = |snap: &claw_sim::SimSnapshot| {
        snap.renderables
            .iter()
            .filter(|r| r.image_set_key == "GAME_TREASURE")
            .count()
    };
    let wide = ViewConfig {
        render_width: 3440,
        render_height: 1440,
        aspect_policy: AspectPolicy::FrozenEdgeClamped,
        ..ViewConfig::default()
    };
    let mut sim = Sim::new(level.clone(), SimConfig::for_view(wide));
    let snap = sim.step(TickInput::default());
    assert_eq!(
        treasures(&snap),
        3,
        "the dormant ones in and near the side band are drawn frozen"
    );

    // A 4:3 presentation has no extra width: frozen-edge draws exactly
    // what the original draws, and nothing dormant beyond the view.
    let frozen_cfg = SimConfig::default();
    assert_eq!(frozen_cfg.activation.mode, ActivationMode::FrozenEdge);
    let mut frozen = Sim::new(level.clone(), frozen_cfg);
    let original_cfg = SimConfig {
        activation: ActivationConfig::original(),
        ..SimConfig::default()
    };
    let mut original = Sim::new(level, original_cfg);
    for _ in 0..2 {
        let frozen_snap = frozen.step(TickInput::default());
        let original_snap = original.step(TickInput::default());
        assert_eq!(frozen_snap.renderables, original_snap.renderables);
    }
    // By the second tick the pickup near Claw is awake and drawn; the two
    // beyond the view stay dormant and undrawn.
    assert_eq!(treasures(&original.snapshot()), 1);
}

#[test]
fn widened_view_follows_the_aspect_policy() {
    let view = ViewConfig {
        render_width: 5120,
        render_height: 1440,
        aspect_policy: AspectPolicy::WidenedActivation,
        ..ViewConfig::default()
    };
    let cfg = SimConfig::for_view(view);
    assert_eq!(cfg.activation.mode, ActivationMode::Widened);
    assert_eq!(cfg.activation.view, (1708, 480));
    let pillar = SimConfig::for_view(ViewConfig {
        aspect_policy: AspectPolicy::Pillarbox,
        ..view
    });
    assert_eq!(pillar.activation, ActivationConfig::original());
}

#[test]
fn level_lists_every_image_set_its_objects_can_show() {
    let mut level = level_with_objects();
    let anim = claw_sim::anim::Animation {
        key: "CLAW_STAND".into(),
        image_set: "CLAW_EXTRA".into(),
        frames: Vec::new(),
        frame_sounds: Vec::new(),
    };
    level.animations.insert(anim.key.clone(), Arc::new(anim));
    let mut want: Vec<String> = ["CLAW", "CLAW_EXTRA", "GAME_TREASURE"]
        .into_iter()
        .chain(claw_sim::logic::runtime_image_sets())
        .map(str::to_string)
        .collect();
    want.sort();
    want.dedup();
    assert_eq!(level.image_sets(), want);
    // Every set a logic switches to at run time is listed, even for a
    // level with no objects or animations.
    let empty = claw_sim::SimLevel::synthetic(claw_sim::TileMap::new(1, 1, 64, 64), (0, 0));
    let runtime = claw_sim::logic::runtime_image_sets();
    for set in claw_sim::level::RUNTIME_IMAGE_SETS
        .into_iter()
        .chain(runtime)
    {
        assert!(empty.image_sets().iter().any(|s| s == set), "{set}");
        assert_eq!(claw_sim::level::runtime_image_set(set), set);
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "not listed as a run-time image set")]
fn switching_to_an_unlisted_image_set_is_caught_in_debug_builds() {
    let _ = claw_sim::level::runtime_image_set("SOME_OTHER_SET");
}
