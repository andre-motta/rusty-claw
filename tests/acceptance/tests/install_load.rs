//! Level loading and the visible tile grids, through the game's own loader
//! and views.
//!
//! - Every retail level is ready to draw in under 3 s: assets, simulation,
//!   graphics, every sprite set its objects use, the GPU upload and the
//!   first frame. Level 1 is measured cold, GPU start-up included.
//! - With `CLAW_GOLDEN_DIR`, the tile grid every plane shows at each
//!   reference camera matches the reference grids, with zero mismatches, at
//!   the original 640x480 view and the wide views of 480 lines that the
//!   game shows on 16:9, 21:9 and 32:9 displays.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use claw_acceptance::install;
use claw_assets::level::RETAIL_LEVELS;
use claw_game::settings::{Aspect, LevelChoice, Settings};
use claw_game::{Session, load_graphics, load_level, new_sim, preload};
use claw_render::gpu::COLOR_FORMAT;
use claw_render::{FrameInput, Gpu, Offscreen, Renderer, SceneBuilder};

const LIMIT: Duration = Duration::from_secs(3);

/// Loads level `n` up to its first drawn frame; returns the time taken.
fn load_to_first_frame(root: &PathBuf, n: u32, gpu: Option<&Gpu>) -> (Duration, usize) {
    let t = Instant::now();
    let settings = Settings::new(root, LevelChoice::Retail(n));
    let bundle = load_level(&settings.install, &settings.level).expect("level loads");
    let mut graphics = load_graphics(&bundle, None).expect("graphics");
    let sim = new_sim(&bundle, settings.sim_config(settings.res), None);
    let sets = preload(&mut graphics, &bundle.level, &sim.snapshot());
    let session = Session::new(sim);
    let owned;
    let gpu = match gpu {
        Some(g) => g,
        None => {
            owned = Gpu::headless().expect("a Vulkan GPU");
            &owned
        }
    };
    let mut renderer = Renderer::new(gpu);
    renderer.set_level(&graphics);
    let view = settings.view(settings.res);
    let mut builder = SceneBuilder::new(&graphics, view, gpu.max_target());
    let out = Offscreen::new(gpu, settings.res);
    let frame = session.interpolated(true);
    let scene = builder.build(&mut graphics, &FrameInput::from_frame(&frame, &view));
    assert!(!scene.sprites.is_empty(), "level {n}: Claw is drawn");
    renderer.render(&graphics, &scene, &out.view, COLOR_FORMAT);
    renderer.wait_idle();
    (t.elapsed(), sets)
}

#[test]
fn install_every_level_loads_in_under_three_seconds() {
    let Some(root) = install() else { return };
    let (cold, sets) = load_to_first_frame(&root, 1, None);
    eprintln!(
        "level 1: {:.0} ms to the first frame, GPU start-up included ({sets} sprite sets)",
        cold.as_secs_f64() * 1000.0
    );
    assert!(cold < LIMIT, "level 1 took {cold:?}");
    let gpu = Gpu::headless().expect("a Vulkan GPU");
    let mut slowest = (0, Duration::ZERO);
    for n in RETAIL_LEVELS {
        let (t, sets) = load_to_first_frame(&root, n, Some(&gpu));
        eprintln!(
            "level {n}: {:.0} ms ({sets} sprite sets)",
            t.as_secs_f64() * 1000.0
        );
        assert!(t < LIMIT, "level {n} took {t:?}");
        if t > slowest.1 {
            slowest = (n, t);
        }
    }
    eprintln!(
        "slowest: level {} at {:.0} ms",
        slowest.0,
        slowest.1.as_secs_f64() * 1000.0
    );
}

/// The output size whose presentation in the game's fill policy is `w`
/// game pixels wide at 480 lines.
fn output_for(w: u32) -> (u32, u32) {
    (w, 480)
}

#[test]
fn install_visible_tile_grids_match_the_reference() {
    let Some(root) = install() else { return };
    let Some(dir) = std::env::var_os("CLAW_GOLDEN_DIR").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_GOLDEN_DIR is not set");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("CLAW_GOLDEN_DIR is a folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("level") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    let mut by_level: BTreeMap<u32, Vec<serde_json::Value>> = BTreeMap::new();
    let mut other_views = Vec::new();
    for file in &files {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        if doc["view"]["h"].as_u64() == Some(480) {
            let n = doc["level"].as_u64().unwrap() as u32;
            by_level.entry(n).or_default().push(doc);
        } else {
            other_views.push(file.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    let widths: std::collections::BTreeSet<u64> = by_level
        .values()
        .flatten()
        .map(|d| d["view"]["w"].as_u64().unwrap())
        .collect();
    assert!(widths.contains(&640), "no 640x480 reference grids");
    assert!(
        widths.iter().any(|&w| w > 640),
        "no wide reference grids at 480 lines"
    );
    let (mut grids, mut cells, mut mismatches) = (0usize, 0usize, 0usize);
    for (n, docs) in &by_level {
        let mut settings = Settings::new(&root, LevelChoice::Retail(*n));
        settings.aspect = Aspect::Fill;
        let bundle = load_level(&settings.install, &settings.level).expect("level loads");
        let graphics = load_graphics(&bundle, None).expect("graphics");
        for doc in docs {
            let w = doc["view"]["w"].as_u64().unwrap() as u32;
            let view = settings.view(output_for(w));
            let mut b = SceneBuilder::new(&graphics, view, 1 << 14);
            assert_eq!(b.composition().presentation, (w, 480));
            for cam in doc["cameras"].as_array().unwrap() {
                let (x, y) = (
                    cam["x"].as_i64().unwrap() as i32,
                    cam["y"].as_i64().unwrap() as i32,
                );
                let ours = b.visible_cells(&graphics, x, y);
                let theirs = cam["planes"].as_array().unwrap();
                assert_eq!(ours.len(), theirs.len(), "level {n} {w}x480: planes");
                for ((rect, grid), want) in ours.iter().zip(theirs) {
                    grids += 1;
                    let label = format!(
                        "level {n} {w}x480 camera ({x}, {y}) plane {}",
                        want["name"].as_str().unwrap_or("?")
                    );
                    assert_eq!(
                        i64::from(rect.left),
                        want["view_left"].as_i64().unwrap(),
                        "{label}"
                    );
                    assert_eq!(
                        i64::from(rect.top),
                        want["view_top"].as_i64().unwrap(),
                        "{label}"
                    );
                    let rows = want["grid"].as_array().unwrap();
                    assert_eq!(grid.len(), rows.len(), "{label}: rows");
                    for (r, row) in rows.iter().enumerate() {
                        let row = row.as_array().unwrap();
                        assert_eq!(grid[r].len(), row.len(), "{label}: columns");
                        for (c, v) in row.iter().enumerate() {
                            cells += 1;
                            if i64::from(grid[r][c]) != v.as_i64().unwrap() {
                                mismatches += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "reference grids: {} levels, views {:?} at 480 lines, {grids} plane grids, {cells} cells, \
         {mismatches} mismatches",
        by_level.len(),
        widths
    );
    if !other_views.is_empty() {
        eprintln!(
            "not checked here (the game always shows 480 game lines): {}",
            other_views.join(", ")
        );
    }
    assert_eq!(mismatches, 0);
}
