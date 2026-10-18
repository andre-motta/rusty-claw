//! The extra width of a wide display on your own install (skipped without
//! `CLAW_INSTALL`). The sides show the level beyond the original view
//! with the objects there asleep and still; what happens there is drawing
//! only and must not look broken:
//!
//! - a body killed by a hit, which the game removes once it leaves the
//!   logic view, goes on falling on screen; the game plays exactly the
//!   same: the third level's original-speeds route, played through the
//!   game's frame loop presented at 3840x2160, at 640x480 and with bars
//!   at the sides, gives the same digest of the whole game after every
//!   tick and the same trace (the activation column apart, which names the
//!   bars' mode), and only the wide presentation may draw falling bodies;
//! - crates, kegs and pickups the game has not woken yet are drawn as they
//!   look once woken: along the first three levels' original-speeds routes
//!   at 3840x2160, none of them appears in the side areas, moves or
//!   changes picture there when the game wakes it, and no curse (which a
//!   single-player game removes as it wakes it) is drawn there.
//!
//! Nothing from an install is stored in the repository.

use std::collections::HashSet;
use std::time::Duration;

use claw_acceptance::install;
use claw_acceptance::play::digest;
use claw_assets::{AssetStore, Install, LevelData, LevelSource};
use claw_bot::route;
use claw_frame::{AspectPolicy, SimSnapshot, TICK_MS, ViewConfig};
use claw_game::Session;
use claw_sim::logic::pickup::PickupData;
use claw_sim::object::flags;
use claw_sim::trace;
use claw_sim::{InputScript, ObjectData, Sim, SimConfig, SimLevel};

/// The level, the route's script and the simulation settings for `view`
/// (the tuning and seed the route's header names).
fn route_start(
    store: &AssetStore,
    n: u32,
    file: &str,
    view: ViewConfig,
) -> (SimLevel, InputScript, SimConfig) {
    let data = LevelData::build(store, LevelSource::Retail(n)).unwrap();
    let level = SimLevel::from_level(&data, store).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("routes")
        .join(file);
    let text = std::fs::read_to_string(path).unwrap();
    let header = route::Header::parse(&text);
    assert!(
        header.start.is_none() && header.carry.is_none(),
        "{file} starts the level"
    );
    let script = InputScript::parse(&text).unwrap();
    let base = route::sim_config(header.tuning, header.seed);
    let cfg = SimConfig {
        tuning: base.tuning,
        seed: base.seed,
        ..SimConfig::for_view(view)
    };
    (level, script, cfg)
}

/// What one presentation of the route gave.
struct Played {
    trace: String,
    digests: Vec<u64>,
    /// Ticks after which falling bodies were drawn.
    ticks_with_bodies: usize,
}

/// Plays `script` from `sim` through the game's frame loop, one tick per
/// frame, presented as `view`.
fn play(sim: Sim, script: &InputScript, view: ViewConfig) -> Played {
    let n = script.ticks.len();
    let mut session = Session::new(sim).with_replay(script.clone());
    session.set_presentation(view);
    session.record_trace();
    let digests = std::rc::Rc::new(std::cell::RefCell::new(Vec::with_capacity(n)));
    let seen = digests.clone();
    session.set_before_tick(Box::new(move |_, sim: &mut Sim| {
        seen.borrow_mut().push(digest(sim));
    }));
    let mut ticks_with_bodies = 0;
    while (session.ticks() as usize) < n {
        session.frame(Duration::from_millis(u64::from(TICK_MS)));
        if session.falling_bodies() > 0 {
            ticks_with_bodies += 1;
        }
    }
    let rows = session.trace()[..n].to_vec();
    let digests = digests.borrow().clone();
    Played {
        trace: trace::to_csv(&rows),
        digests,
        ticks_with_bodies,
    }
}

/// `trace` with every activation cell emptied.
fn without_activation(trace: &str) -> String {
    let mut lines = trace.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = header.iter().position(|c| *c == "activation").unwrap();
    let mut out = header.join(",");
    out.push('\n');
    for line in lines {
        let mut cells: Vec<&str> = line.split(',').collect();
        cells[col] = "";
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

#[test]
fn install_a_wide_presentation_plays_the_same_game() {
    let Some(root) = install() else { return };
    let store = Install::open(&root).unwrap().store().unwrap();
    let wide = ViewConfig::default().with_render_size(3840, 2160);
    let narrow = ViewConfig::default();
    let bars = ViewConfig {
        aspect_policy: AspectPolicy::Pillarbox,
        ..wide
    };
    let file = "level3-original.csv";
    let mut runs = Vec::new();
    for view in [wide, narrow, bars] {
        let (level, script, cfg) = route_start(&store, 3, file, view);
        runs.push(play(Sim::new(level, cfg), &script, view));
    }
    let [wide, narrow, bars] = &runs[..] else {
        unreachable!()
    };
    // The route's kills fall out of the bottom of the view, below the
    // presented area too, so this route may draw none; the simulation's
    // tests kill one at the side.
    assert_eq!(narrow.ticks_with_bodies, 0, "640x480 draws none");
    assert_eq!(bars.ticks_with_bodies, 0, "bars at the sides draw none");
    for (name, other) in [("640x480", narrow), ("bars", bars)] {
        let first = wide
            .digests
            .iter()
            .zip(&other.digests)
            .position(|(a, b)| a != b);
        assert_eq!(
            first, None,
            "{file}: the game differs from {name} before tick {first:?}"
        );
        assert_eq!(wide.digests.len(), other.digests.len());
    }
    assert_eq!(
        wide.trace, narrow.trace,
        "{file}: the same trace at 640x480"
    );
    assert_eq!(
        without_activation(&wide.trace),
        without_activation(&bars.trace),
        "{file}: the same trace with bars"
    );
    println!(
        "{file}: {} ticks, the same game at 3840x2160, 640x480 and with bars; falling bodies drawn after {} ticks",
        wide.digests.len(),
        wide.ticks_with_bodies
    );
}

/// The logics whose sleeping objects are drawn as they look once woken.
fn checked(o: &claw_sim::object::Object) -> bool {
    matches!(
        &*o.logic,
        "PowderKeg"
            | "Crate"
            | "FrontCrate"
            | "BehindCrate"
            | "TreasurePowerup"
            | "AmmoPowerup"
            | "HealthPowerup"
            | "MagicPowerup"
            | "SpecialPowerup"
            | "SuperPowerup"
            | "CursePowerup"
            | "EndOfLevelPowerup"
            | "PowerupGlitter"
    )
}

/// True for an animated pickup, whose picture starts cycling as it wakes.
fn animated(o: &claw_sim::object::Object) -> bool {
    o.logic.ends_with("Powerup") || &*o.logic == "PowerupGlitter"
}

#[test]
fn install_sleeping_crates_kegs_and_pickups_do_not_pop_in() {
    let Some(root) = install() else { return };
    let store = Install::open(&root).unwrap().store().unwrap();
    let view = ViewConfig::default().with_render_size(3840, 2160);
    let (pw, _) = view.presentation_size();
    let side = f64::from((pw - view.logic_width) / 2);
    for (n, file) in [
        (1, "level1-original.csv"),
        (2, "level2-original.csv"),
        (3, "level3-original.csv"),
    ] {
        let (level, script, cfg) = route_start(&store, n, file, view);
        let mut sim = Sim::new(level, cfg);
        // Objects made while the game runs are left out: only what the
        // level places, the crates of its stacks and the glitters of its
        // pickups are checked.
        let placed: HashSet<u32> = sim.objects().all().map(|(_, o)| o.serial).collect();
        let mut awake: HashSet<u32> = sim
            .objects()
            .all()
            .filter(|(_, o)| o.is_listed())
            .map(|(_, o)| o.serial)
            .collect();
        let mut before: Option<SimSnapshot> = None;
        let mut checked_wakes = 0;
        let mut failures = Vec::new();
        for (i, &t) in script.ticks.iter().enumerate() {
            let snap = sim.step(t);
            let cam = snap.camera;
            let in_side = |x: f64, y: f64| {
                let inside_x = (x >= cam.x - side && x < cam.x - 40.0)
                    || (x > cam.x + 680.0 && x <= cam.x + 640.0 + side);
                inside_x && y >= cam.y && y <= cam.y + 480.0
            };
            if let Some(prev) = &before
                && !snap.camera_teleported
            {
                for (_, o) in sim.objects().all() {
                    if !o.is_listed()
                        || o.flags & flags::WORLD == 0
                        || !awake.insert(o.serial)
                        || !checked(o)
                    {
                        continue;
                    }
                    let owner_placed = match &o.data {
                        ObjectData::Pickup(PickupData { owner: Some(p), .. }) => sim
                            .objects()
                            .get(*p)
                            .is_some_and(|p| placed.contains(&p.serial)),
                        // The crates of a stack are made as the stack
                        // wakes.
                        _ => placed.contains(&o.serial) || &*o.logic == "Crate",
                    };
                    let Some(r) = snap.renderables.iter().find(|r| r.id == o.serial) else {
                        continue;
                    };
                    if !owner_placed || !in_side(r.x, r.y) {
                        continue;
                    }
                    checked_wakes += 1;
                    let shown = prev.renderables.iter().any(|p| {
                        p.image_set_key == r.image_set_key
                            && p.x == r.x
                            && p.y == r.y
                            && (p.frame == r.frame || animated(o))
                    });
                    if !shown {
                        failures.push(format!(
                            "tick {i}: {} woke at {},{} ({} frame {}), not drawn so the tick before",
                            o.logic, r.x, r.y, r.image_set_key, r.frame
                        ));
                    }
                }
                for r in &snap.renderables {
                    if r.image_set_key.starts_with("GAME_CURSES") && in_side(r.x, r.y) {
                        failures.push(format!("tick {i}: a curse drawn at {},{}", r.x, r.y));
                    }
                }
            } else {
                awake.extend(
                    sim.objects()
                        .all()
                        .filter(|(_, o)| o.is_listed())
                        .map(|(_, o)| o.serial),
                );
            }
            before = Some(snap);
        }
        assert!(failures.is_empty(), "{file}:\n{}", failures.join("\n"));
        assert!(
            checked_wakes > 10,
            "{file}: {checked_wakes} wakes in the side areas"
        );
        println!(
            "{file}: {checked_wakes} crates, kegs and pickups woke in the side areas, none popped in"
        );
    }
}
