//! Each of Claw's weapons against each enemy of the first level, on your
//! own install (skipped without `CLAW_INSTALL`): the sword (and fists and
//! feet), the pistol, magic and dynamite each kill an officer, a soldier
//! and a rat, placed from the level's own records, with the kill dealt by
//! that weapon and its ammunition spent.

use claw_acceptance::install;
use claw_acceptance::play::*;
use claw_game::load_level;
use claw_game::settings::LevelChoice;
use claw_sim::logic::enemy::DeathKind;
use claw_sim::{Sim, SimConfig, SimLevel};

const NEAR: [i32; 8] = [-70, 70, -50, 50, -100, 100, -30, 30];
const FAR: [i32; 10] = [150, -150, 120, -120, 180, -180, 200, -200, 250, -250];

/// The ammunition `arm` uses.
fn ammo(p: &Player, arm: Arm) -> i32 {
    let s = p.sim.stats();
    match arm {
        Arm::Sword => 0,
        Arm::Pistol => s.pistol,
        Arm::Magic => s.magic,
        Arm::Dynamite => s.dynamite,
    }
}

/// Throws dynamite at the enemy `id`: held for `charge` ticks to throw it
/// farther, then let go; waits for the blast.
fn throw_dynamite(p: &mut Player, id: u32, placed: (i32, i32, i32), charge: usize) -> bool {
    let _ = id;
    let low = 0;
    p.hold(low | DYNAMITE, charge.max(1));
    p.until(150, |p| {
        (!p.kills.iter().any(|k| k.placed == placed)).then_some(low)
    });
    p.kills.iter().any(|k| k.placed == placed)
}

/// Kills the first enemy running `logic` that Claw can be put beside with
/// `arm`; returns what killed it and the ammunition spent.
fn kill_with(
    base: &Player,
    level: &SimLevel,
    logic: &str,
    arm: Arm,
) -> (String, i32, (i32, i32, i32)) {
    for placed in records(level, logic) {
        let offsets: &[i32] = if arm == Arm::Sword { &NEAR } else { &FAR };
        let charges: &[usize] = if arm == Arm::Dynamite {
            &[1, 3, 6, 9, 12, 16, 20, 25, 30, 38, 45, 55]
        } else {
            &[0]
        };
        for &charge in charges {
            let mut p = base.clone();
            let Some(id) = place_beside(&mut p, logic, placed, offsets, arm != Arm::Sword) else {
                break;
            };
            let before = ammo(&p, arm);
            let face = p.obj(id).unwrap().x > p.at().0;
            if p.sim.claw().facing_right != face {
                p.step(if face { RIGHT } else { LEFT });
                p.idle(2);
            }
            let won = match arm {
                Arm::Dynamite => throw_dynamite(&mut p, id, placed, charge),
                _ => fight(&mut p, id, arm, 1200).is_some(),
            };
            let kill = p.kills.iter().find(|k| k.placed == placed);
            if won && p.health() > 0 && kill.is_some_and(|k| k.death == Some(DeathKind::Hit)) {
                let k = kill.unwrap();
                return (k.by.clone(), before - ammo(&p, arm), placed);
            }
        }
    }
    panic!("no {logic} of the level could be killed with {arm:?}");
}

#[test]
fn install_each_weapon_kills_each_enemy() {
    let Some(root) = install() else { return };
    let bundle = load_level(&root, &LevelChoice::Retail(1)).expect("level 1");
    let level = bundle.sim_level.clone();
    let base = Player::new(Sim::new(level.clone(), SimConfig::default()));
    for logic in ["Officer", "Soldier", "Rat"] {
        for arm in [Arm::Sword, Arm::Pistol, Arm::Magic, Arm::Dynamite] {
            let (by, spent, placed) = kill_with(&base, &level, logic, arm);
            println!("{arm:?} against the {logic} at {placed:?}: killed by {by}, {spent} spent");
            match arm {
                Arm::Sword => assert_eq!(by, "CaptainClaw", "{logic}"),
                Arm::Pistol | Arm::Magic => assert_eq!(by, arm.shot(), "{logic}"),
                Arm::Dynamite => assert_eq!(by, "Dynamite", "{logic}"),
            }
            if arm != Arm::Sword {
                assert!(spent >= 1, "{arm:?} against the {logic}: ammunition spent");
            }
        }
    }
}
