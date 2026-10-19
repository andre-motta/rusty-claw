//! The rules that make the twelfth level's water skip, on a synthetic
//! level shaped like the pool's left end: a band of death tiles one tile
//! high whose first column starts at x 2560, a solid floor a tile below it
//! from the same column on, and nothing under the band's left neighbour.
//!
//! Falling straight down past the band's left end, Claw dies only when
//! the single pixel at his centre column meets a death row, and lands
//! when either of his landing samples (X + 4, X + 14) is over the floor:
//! from x 2546 to 2559 he passes the band and lands on the floor beneath
//! it, from x 2560 the band kills him, left of x 2546 he falls past the
//! floor's end. Standing on that floor under the band is safe, and so is
//! walking along it.

mod common;

use claw_sim::input::ScriptTuning;
use claw_sim::player::state;
use claw_sim::tiles::TileMap;
use claw_sim::{Sim, SimConfig, SimLevel, TickInput};
use common::*;

/// The band's first column, px.
const BAND_X: i32 = 2560;
/// The band's top row, px (one tile high).
const BAND_TOP: i32 = 3776;
/// Claw standing on the floor under the band.
const FLOOR_Y: i32 = 3843;
/// Below the floor's row: fallen past it.
const PAST_FLOOR: i32 = 3950;

/// 70 columns and 64 rows of 64 px: the band in row 59 and the floor in
/// row 61 from column 40 on, and a deep floor in row 63 everywhere.
fn pool_end() -> SimLevel {
    let mut rows: Vec<String> = vec![".".repeat(70); 64];
    rows[59].replace_range(40.., &"x".repeat(30));
    rows[61].replace_range(40.., &"#".repeat(30));
    rows[63] = "#".repeat(70);
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (3000, 3600));
    level.start = (3000, 3600);
    level
}

fn sim_at(tuning: ScriptTuning) -> Sim {
    let cfg = SimConfig {
        tuning: tuning.tuning(),
        ..SimConfig::default()
    };
    let mut sim = Sim::new(pool_end(), cfg);
    idle(&mut sim, 30);
    sim
}

/// Claw put down at (x, y) in the air, falling until he dies, lands or
/// passes the floor's row: his last (x, y, state).
fn drop_from(tuning: ScriptTuning, x: i32, y: i32) -> (i32, i32, i32) {
    let mut sim = sim_at(tuning);
    sim.teleport_player(x, y);
    let mut end = (x, y, 0);
    for _ in 0..80 {
        sim.step(TickInput::default());
        let c = sim.player().unwrap();
        end = (c.x, c.y, c.state);
        if c.state == state::DEATH || c.y > PAST_FLOOR {
            break;
        }
        if c.y == FLOOR_Y && matches!(c.state, state::STAND | state::ON_EDGE) {
            break;
        }
    }
    end
}

#[test]
fn the_drop_past_the_bands_left_end_lands_only_from_x_2546_to_2559() {
    for tuning in [ScriptTuning::Original, ScriptTuning::Default] {
        for x in BAND_X - 24..BAND_X + 10 {
            let (ex, ey, st) = drop_from(tuning, x, 3650);
            assert_eq!(ex, x, "{tuning:?} {x}");
            if x < BAND_X - 14 {
                assert!(
                    ey > PAST_FLOOR && st != state::DEATH,
                    "{tuning:?} {x}: {ey} {st}"
                );
            } else if x < BAND_X {
                assert_eq!(ey, FLOOR_Y, "{tuning:?} {x}: {st}");
            } else {
                assert_eq!(st, state::DEATH, "{tuning:?} {x}: {ey}");
                // His feet had reached the band's top row.
                assert!(ey + 61 >= BAND_TOP, "{tuning:?} {x}: {ey}");
            }
        }
    }
}

#[test]
fn standing_and_walking_on_the_floor_under_the_band_is_safe() {
    let mut sim = sim_at(ScriptTuning::Original);
    sim.teleport_player(BAND_X - 2, 3650);
    idle(&mut sim, 40);
    let c = sim.player().unwrap();
    assert_eq!((c.y, c.state), (FLOOR_Y, state::STAND));
    let health = sim.stats().health;
    for k in 0..300 {
        sim.step(if k == 0 { press(RIGHT) } else { hold(RIGHT) });
        let c = sim.player().unwrap();
        assert_ne!(c.state, state::DEATH);
        assert_eq!(c.y, FLOOR_Y);
    }
    let x = sim.player().unwrap().x;
    assert!(x > BAND_X + 1000, "{x}");
    assert_eq!(sim.stats().health, health);
}
