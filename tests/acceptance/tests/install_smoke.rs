//! Every retail level of your own install (skipped without `CLAW_INSTALL`)
//! plays 60 seconds of seeded random input with every level logic on,
//! without a panic. Logics the game does not run yet stay inert
//! placeholders; they are listed for each level.

use claw_acceptance::install;
use claw_acceptance::play::*;
use claw_frame::GameEvent;
use claw_game::load_level;
use claw_game::settings::LevelChoice;
use claw_sim::{Sim, SimConfig};

/// Ticks in 60 seconds.
const TICKS: usize = 60_000 / 18;

/// A small seeded generator (xorshift).
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Random held buttons, each mix kept for a random 1 to 40 ticks: mostly
/// moving and jumping, with attacks, shots, weapon keys and lifting.
fn random_input(seed: u64, ticks: usize) -> Vec<u32> {
    let mut r = Seeded(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let moves = [0, LEFT, RIGHT, UP, DOWN, LEFT | JUMP, RIGHT | JUMP, JUMP];
    let actions = [0, 0, 0, ATTACK, FIRE, LIFT, PISTOL, MAGIC, DYNAMITE, 0x8];
    let mut held = Vec::with_capacity(ticks);
    while held.len() < ticks {
        let mask = moves[r.below(moves.len() as u64) as usize]
            | actions[r.below(actions.len() as u64) as usize];
        let n = 1 + r.below(40) as usize;
        held.extend(std::iter::repeat_n(mask, n));
    }
    held.truncate(ticks);
    held
}

#[test]
fn install_every_retail_level_survives_a_minute_of_random_input() {
    let Some(root) = install() else { return };
    for n in 1..=14u32 {
        let bundle = load_level(&root, &LevelChoice::Retail(n))
            .unwrap_or_else(|e| panic!("level {n} loads: {e:#}"));
        let mut p = Player::new(Sim::new(bundle.sim_level.clone(), SimConfig::default()));
        for held in random_input(u64::from(n), TICKS) {
            p.step(held);
        }
        assert_eq!(p.tick() as usize, TICKS);
        let deaths = p
            .events
            .iter()
            .filter(|(_, e)| matches!(e, GameEvent::ClawDied { .. }))
            .count();
        let d = p.sim.diagnostics();
        let inert: usize = d.inert_logics.values().sum();
        println!(
            "level {n}: {TICKS} ticks, Claw at {:?} with health {}, {deaths} deaths, {} kills, \
             {} objects, {} unknown and {inert} inert records ({:?})",
            p.at(),
            p.health(),
            p.kills.len(),
            p.sim.objects().len(),
            d.unknown_logics.values().sum::<usize>(),
            d.inert_logics.keys().take(6).collect::<Vec<_>>(),
        );
    }
}
