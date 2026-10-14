//! Community custom levels through the game's loader.
//!
//! - At least 20 custom levels from the install's `Custom` folder load and
//!   play: level data, simulation and 200 ticks of the benchmark route, and
//!   for a sample of them the graphics, every sprite set and a built frame.
//!   Many of these levels ship script files; the game reads only data files
//!   and never runs a script or a plugin, so such levels load like any
//!   other. The test counts them.
//! - Level names that would leave the `Custom` folder are refused.

use std::path::Path;
use std::time::{Duration, Instant};

use claw_acceptance::install;
use claw_assets::Install;
use claw_game::bench::route;
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{Session, load_graphics, load_level, new_sim, preload};
use claw_render::{FrameInput, SceneBuilder};

/// Levels that also get their graphics and a frame built: every n-th one.
const GRAPHICS_EVERY: usize = 16;

/// True when the level's folder holds a script file.
fn ships_scripts(wwd: &Path) -> bool {
    let Some(stem) = wwd.file_stem() else {
        return false;
    };
    let Some(dir) = wwd.parent().map(|p| p.join(stem)) else {
        return false;
    };
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("lua"))
            {
                return true;
            }
        }
    }
    false
}

#[test]
fn install_custom_levels_load_and_play() {
    let Some(root) = install() else { return };
    let inst = Install::open(&root).expect("install");
    let paths = inst.custom_levels().expect("custom folder listing");
    let t = Instant::now();
    let (mut ok, mut drawn, mut with_scripts) = (0usize, 0usize, 0usize);
    let mut failed = Vec::new();
    let mut script = route(200.0 * 0.018);
    script.ticks.truncate(200);
    for (i, path) in paths.iter().enumerate() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let settings = Settings::new(&root, LevelChoice::Custom(name.clone()));
        let bundle = match load_level(&settings.install, &settings.level) {
            Ok(b) => b,
            Err(e) => {
                failed.push(format!("{name}: {e:#}"));
                continue;
            }
        };
        let sim = new_sim(&bundle, settings.sim_config(settings.res), None);
        let mut session = Session::new(sim).with_replay(script.clone());
        session.frame(Duration::from_millis(18 * 50));
        while session.replaying() {
            session.frame(Duration::from_millis(72));
        }
        assert!(session.sim().player().is_some(), "{name}: Claw exists");
        ok += 1;
        with_scripts += usize::from(ships_scripts(path));
        if i % GRAPHICS_EVERY == 0 {
            let mut graphics = load_graphics(&bundle, None).expect("graphics");
            preload(&mut graphics, &bundle.level, &session.sim().snapshot());
            let view = settings.view(settings.res);
            let mut builder = SceneBuilder::new(&graphics, view, 1 << 14);
            let frame = session.interpolated(true);
            let scene = builder.build(&mut graphics, &FrameInput::from_frame(&frame, &view));
            assert_eq!(scene.comp.presentation, (640, 480), "{name}");
            drawn += 1;
        }
    }
    eprintln!(
        "custom levels: {ok} of {} loaded and played 200 ticks ({drawn} also drawn), \
         {with_scripts} of them ship script files, which were not run; {:.1} s",
        paths.len(),
        t.elapsed().as_secs_f64()
    );
    for f in failed.iter().take(10) {
        eprintln!("  did not load: {f}");
    }
    assert!(ok >= 20, "only {ok} custom levels loaded");
    assert!(drawn >= 1);
}

#[test]
fn install_custom_level_names_cannot_leave_the_custom_folder() {
    let Some(root) = install() else { return };
    for bad in [
        "../CLAW.REZ",
        "Custom/../CLAW.REZ",
        "../Assets/LEVEL1/WORLDS/WORLD.WWD",
        "/etc/passwd",
        "C:\\CLAW.REZ",
        "sub/dir/level.wwd",
    ] {
        let e = load_level(&root, &LevelChoice::Custom(bad.to_string()));
        assert!(e.is_err(), "{bad:?} was accepted");
    }
}
