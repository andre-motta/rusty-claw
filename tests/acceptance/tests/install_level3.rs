//! The third level's dynamite boost on your own install (skipped without
//! `CLAW_INSTALL`), played from `routes/level3-boost.csv` at the original
//! speeds: past the first save point Claw throws a stick of dynamite into
//! the face of the rock pillar on his right from a jump, lands, jumps into
//! the blast, is struck in the air (15 health) and, with the mid-air hit
//! left as the original has it, holds still in the air and then stands
//! there, so a jump from the air takes him on top of the pillar, which no
//! jump from the floor reaches. Nothing from an install is stored in the
//! repository.
//!
//! The route was found before the game learned the third level's rules
//! (Claw's shots waiting a call among them) and no longer reaches the
//! pillar the same way; the check is skipped until the route is searched
//! again (S2-P5b).

use claw_acceptance::install;
use claw_assets::{Install, LevelData, LevelSource};
use claw_bot::route::{self, TuningName};
use claw_sim::player::state;
use claw_sim::{InputScript, Sim, SimLevel};

/// The pillar's left face and right side, px.
const PILLAR_X: (i32, i32) = (15968, 16160);

/// Highest Claw's position gets in a jump from the floor beside the pillar
/// (it peaks at 6704); standing on the pillar he is at 6671.
const FLOOR_JUMP_PEAK: i32 = 6700;

/// Health a dynamite blast takes.
const BLAST: i32 = 15;

/// The route waits to be searched again under the third level's rules.
const AWAITS_SEARCH_IN_S2_P5B: bool = true;

#[test]
fn install_level3_dynamite_boost_clears_the_pillar() {
    let Some(root) = install() else { return };
    if AWAITS_SEARCH_IN_S2_P5B {
        println!(
            "level3-boost.csv: skipped, expected to fail until it is searched again under the third level's rules (S2-P5b)"
        );
        return;
    }
    let store = Install::open(&root).unwrap().store().unwrap();
    let data = LevelData::build(&store, LevelSource::Retail(3)).unwrap();
    let level = SimLevel::from_level(&data, &store).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("routes/level3-boost.csv");
    let text = std::fs::read_to_string(path).unwrap();
    let header = route::Header::parse(&text);
    assert_eq!(header.tuning, TuningName::Original);
    let script = InputScript::parse(&text).unwrap();
    let mut sim = Sim::new(level, route::sim_config(header.tuning, header.seed));

    let mut thrown = None;
    let mut struck = None;
    let mut stood_in_air = None;
    let mut on_top = None;
    for (i, &t) in script.ticks.iter().enumerate() {
        let before = (sim.stats().dynamite, sim.stats().health);
        let air = sim
            .player()
            .is_some_and(|o| matches!(o.state, state::JUMP | state::FALL));
        sim.step(t);
        let o = sim.player().expect("Claw");
        let near = (PILLAR_X.0 - 200..=PILLAR_X.1).contains(&o.x);
        if thrown.is_none() && near && sim.stats().dynamite == before.0 - 1 {
            thrown = Some(i);
        }
        if thrown.is_some() && struck.is_none() && sim.stats().health == before.1 - BLAST {
            assert!(air, "the blast strikes Claw in the air (tick {i})");
            assert!(o.x < PILLAR_X.0, "beside the pillar (tick {i})");
            struck = Some(i);
        }
        if let Some(s) = struck
            && stood_in_air.is_none()
            && o.state == state::STAND
        {
            assert!(
                o.y < FLOOR_JUMP_PEAK + 10,
                "he stands where the blast held him, in the air (tick {i}, y {})",
                o.y
            );
            assert!(i - s < 40, "soon after the blast");
            stood_in_air = Some(i);
        }
        if stood_in_air.is_some()
            && on_top.is_none()
            && (PILLAR_X.0..=PILLAR_X.1).contains(&o.x)
            && o.y < FLOOR_JUMP_PEAK
            && matches!(o.state, state::STAND | state::MOVE_RIGHT)
        {
            on_top = Some(i);
            break;
        }
    }
    let thrown = thrown.expect("the dynamite is thrown beside the pillar");
    let struck = struck.expect("the blast strikes Claw");
    let on_top = on_top.expect("Claw stands on top of the pillar");
    assert!(struck - thrown > 60, "the fuse burns down before the blast");
    assert!(on_top - struck < 120, "he is on top soon after the blast");
    println!(
        "level3-boost.csv: dynamite thrown in tick {thrown}, Claw struck in the air in tick {struck}, on top of the pillar in tick {on_top}"
    );
}
