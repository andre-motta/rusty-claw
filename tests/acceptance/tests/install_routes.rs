//! The committed bot routes on your own install (skipped without
//! `CLAW_INSTALL`).
//!
//! Every route in `tests/acceptance/routes/` is an input script found by
//! the search bot (`claw-bot`), led by its `# level`, `# tuning`, `# seed`
//! and behaviour header lines. Each one is played in the game built the
//! way the game builds it, with the tuning, seed and behaviour its header
//! names:
//!
//! - stepped tick by tick from the script, then replayed through the
//!   game's frame loop at a simulated 60 Hz display, once from the script
//!   rows and twice as key events: every per-tick digest of the whole game
//!   is identical across the runs, and so is the trace of each replay;
//! - the level is won (the level complete event), Claw never dies, and no
//!   tick moves him further than walking, running, falling or riding can
//!   (nothing moves him but the input);
//! - every change of the held buttons comes at least 2 ticks after the
//!   previous one, so a 60 Hz display never drops a press;
//! - the same route stepped in the bot's own configuration gives the same
//!   digests, so what the bot found is what the game plays.

use std::path::{Path, PathBuf};

use claw_acceptance::install;
use claw_acceptance::play::{Feed, Player, digest, replay};
use claw_frame::GameEvent;
use claw_game::session::script_behaviour;
use claw_game::settings::{LevelChoice, Settings};
use claw_game::{load_level, new_sim};
use claw_sim::{InputScript, Sim, SimConfig, Tuning};

/// Fewest ticks between two changes of the held buttons.
const MIN_SPACING: usize = 2;

/// Largest move of Claw in one tick that input alone can make, px.
const MAX_STEP_PX: i32 = 48;

/// The route header keys.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Header {
    level: u32,
    original: bool,
    seed: u32,
    fix: bool,
}

fn header(text: &str) -> Header {
    let mut h = Header {
        level: 0,
        original: false,
        seed: 1,
        fix: script_behaviour(text).unwrap_or(false),
    };
    let mut tuning = None;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix('#') else {
            continue;
        };
        let mut words = rest.split_whitespace();
        match (words.next(), words.next()) {
            (Some("level"), Some(v)) => h.level = v.parse().expect("a level number"),
            (Some("tuning"), Some(v)) => tuning = Some(v.to_string()),
            (Some("seed"), Some(v)) => h.seed = v.parse().expect("a seed"),
            _ => {}
        }
    }
    assert!(h.level >= 1, "the route names its level");
    match tuning.as_deref() {
        Some("default") => {}
        Some("original") => h.original = true,
        other => panic!("the route names its tuning, found {other:?}"),
    }
    h
}

fn routes() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("routes");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the routes folder")
        .map(|e| e.expect("a folder entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    out.sort();
    out
}

/// Ticks of held-button changes closer than [`MIN_SPACING`] to the
/// previous change.
fn too_close(script: &InputScript) -> Vec<usize> {
    let mut prev = 0;
    let mut last: Option<usize> = None;
    let mut out = Vec::new();
    for (i, t) in script.ticks.iter().enumerate() {
        if t.held.0 != prev {
            if last.is_some_and(|l| i - l < MIN_SPACING) {
                out.push(i);
            }
            last = Some(i);
        }
        prev = t.held.0;
    }
    out
}

/// Steps `script` from `start`, returning the player and the digest
/// before every tick and after the last.
fn step_through(start: &Sim, script: &InputScript) -> (Player, Vec<u64>) {
    let mut p = Player::new(start.clone());
    let mut digests = Vec::with_capacity(script.ticks.len() + 1);
    for t in &script.ticks {
        digests.push(digest(&p.sim));
        p.step(t.held.0);
    }
    digests.push(digest(&p.sim));
    (p, digests)
}

#[test]
fn install_routes_win_the_same_way_every_time() {
    let Some(root) = install() else { return };
    let files = routes();
    assert!(!files.is_empty(), "no committed routes");
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("the route reads");
        let h = header(&text);
        let script = InputScript::parse(&text).expect("the route parses");
        assert!(!script.ticks.is_empty(), "{name}: empty route");
        assert_eq!(
            too_close(&script),
            Vec::<usize>::new(),
            "{name}: held-button changes closer than {MIN_SPACING} ticks"
        );

        // The game as the game builds it, with the route's header.
        let choice = LevelChoice::Retail(h.level);
        let bundle = load_level(&root, &choice).expect("the level loads");
        let mut settings = Settings::new(&root, choice);
        settings.mid_air_hit_fix = h.fix;
        let mut cfg = settings.sim_config(settings.res);
        if h.original {
            cfg.tuning = Tuning::original_speeds();
            cfg.tuning.behaviour.fix_double_jump = h.fix;
        }
        cfg.seed = h.seed;
        let start = new_sim(&bundle, cfg, None);

        let (p, stepped) = step_through(&start, &script);
        assert!(p.teleports.is_empty(), "{name}: the route moved Claw");
        let won = p
            .events
            .iter()
            .position(|(_, e)| matches!(e, GameEvent::LevelComplete { .. }));
        let won_at = won.map(|i| p.events[i].0);
        assert!(won_at.is_some(), "{name}: the level is not won");
        assert!(
            !p.events
                .iter()
                .any(|(_, e)| matches!(e, GameEvent::ClawDied { .. } | GameEvent::GameOver)),
            "{name}: Claw died"
        );
        // No tick moves Claw further than input can.
        let mut q = Player::new(start.clone());
        let mut at = q.at();
        for (i, t) in script.ticks.iter().enumerate() {
            q.step(t.held.0);
            let now = q.at();
            let moved = (now.0 - at.0).abs().max((now.1 - at.1).abs());
            assert!(
                moved <= MAX_STEP_PX,
                "{name}: Claw moved {moved} px in tick {i}, from {at:?} to {now:?}"
            );
            at = now;
        }

        // Through the game's frame loop at 60 Hz: from the script rows,
        // and twice as key events.
        let rows = replay(&start, &p, 60.0, None, Feed::Script);
        let keys = replay(&start, &p, 60.0, None, Feed::Keys);
        let again = replay(&start, &p, 60.0, None, Feed::Keys);
        assert_eq!(rows.digests, stepped, "{name}: script replay differs");
        assert_eq!(keys.digests, stepped, "{name}: key replay differs");
        assert_eq!(again.digests, keys.digests, "{name}: key replays differ");
        assert_eq!(rows.trace, keys.trace, "{name}: traces differ");
        assert_eq!(again.trace, keys.trace, "{name}: traces differ");

        // The bot's own configuration plays the same game.
        let bot_cfg = SimConfig {
            tuning: start.config().tuning.clone(),
            seed: h.seed,
            ..SimConfig::default()
        };
        let bot_start = Sim::new(bundle.sim_level.clone(), bot_cfg);
        let (_, bot) = step_through(&bot_start, &script);
        assert_eq!(bot, stepped, "{name}: the bot's configuration differs");

        println!(
            "{name}: level {} ({} tuning, seed {}) won at tick {} of {}, health {}",
            h.level,
            if h.original { "original" } else { "default" },
            h.seed,
            won_at.unwrap(),
            script.ticks.len(),
            p.health()
        );
    }
}
