//! A help screen in a route, on your own install (skipped without
//! `CLAW_INSTALL`): at the ninth level's start Claw jumps and taps the
//! dynamite key near the top of the jump, and the help screen is up for
//! three seconds at the boundary after the stick leaves his hand. Its time
//! burns the fuse down, so the stick goes off at once and its blast
//! reaches him a few ticks later, long before the fuse alone would have
//! burnt; the route played back keeps the screen where it is. Nothing from
//! an install is stored in the repository.

use std::path::PathBuf;

use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::Goal;
use claw_bot::route::{self, TuningName};
use claw_sim::input::MenuScreen;
use claw_sim::{Buttons, Directive, DirectiveKind, InputScript, Sim, SimLevel};

fn level9() -> Option<SimLevel> {
    let Some(root) = std::env::var_os("CLAW_INSTALL").map(PathBuf::from) else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(9)).unwrap();
    Some(SimLevel::from_level(&data, &store).unwrap())
}

/// The first tick in which Claw loses health, playing the route's help
/// screens as the bot plays them.
fn first_hurt(start: &Sim, script: &InputScript) -> Option<usize> {
    let mut s = start.clone();
    let health = s.stats().health;
    (0..script.ticks.len()).find(|&i| {
        route::play_directives(&mut s, script, i);
        s.step(script.ticks[i]);
        s.stats().health < health
    })
}

#[test]
fn install_a_help_screen_in_a_route_sets_off_the_dynamite_at_claws_side() {
    let Some(level) = level9() else { return };
    let start = Sim::new(level, route::sim_config(TuningName::Original, 7));
    let mut held = vec![0; 30];
    held.extend([Buttons::JUMP.0; 14]);
    held.extend([0; 8]);
    held.extend([Buttons::DYNAMITE.0; 2]);
    held.extend([0; 40]);
    let plain = InputScript::from_held(&held);
    let mut s = start.clone();
    let thrown = (0..held.len())
        .find(|&i| {
            s.step(plain.ticks[i]);
            s.objects().all().any(|(_, o)| &*o.logic == "Dynamite")
        })
        .expect("the stick is thrown");
    let mut script = plain.clone();
    script.directives.push(Directive {
        tick: thrown + 1,
        kind: DirectiveKind::Menu {
            screen: MenuScreen::Help,
            wall_ms: 3000,
            keys: Vec::new(),
        },
    });
    let hurt = first_hurt(&start, &script).expect("the blast reaches Claw");
    assert!(
        (thrown + 2..thrown + 14).contains(&hurt),
        "thrown on tick {thrown}, hurt on tick {hurt}"
    );
    assert_eq!(first_hurt(&start, &plain), None);
    let far = Goal::Reach {
        x: 100_000,
        y: 0,
        radius: 1,
    };
    assert!(route::play(&start, &script, &far).health_lost > 0);
    assert_eq!(route::play(&start, &plain, &far).health_lost, 0);
}
