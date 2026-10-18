//! Breaking planks: short wooden walkways that give way under Claw.
//!
//! A row is placed as one `BreakPlank` record: an invisible spawner that,
//! on its first call, builds one `SinglePlank` piece per plank (the
//! record's width, at least one), [`level_table`]'s spacing apart, links
//! them left and right, and from then on only watches Claw's attempt
//! counter, restoring the whole row on its first call after Claw loses a
//! life.
//!
//! A piece is not a platform: the level's tile plane already holds an
//! invisible one-way floor tile in each piece's cell, and the piece only
//! clears that cell when it breaks and writes it back when the row is
//! restored. So Claw and the enemies stand on planks through ordinary tile
//! collision, and only Claw starts a collapse: when he stands or walks
//! with his feet inside a piece's band and his body inside it
//! horizontally, the piece waits the record's delay, then crumbles and
//! tells its two neighbours to wait and crumble too, so the collapse
//! spreads along the row one piece per delay. Halfway through the crumble
//! the floor goes; at its end the piece hides. Pieces have no hit or
//! attack box and take no part in pair collision.
//!
//! Not built yet ([`BUILT`] is false): spawners and pieces stay inert
//! placeholders (drawn, no collision, reported as not implemented), and
//! the calls below do nothing. While not built, a level's plank rows stay
//! solid (their floor tiles are never cleared).
//!
//! # Defaults taken
//!
//! - The pieces change one cell of the level's main tile plane; the world
//!   is to give this family a single write of one main-plane cell (column,
//!   row, tile), seen by the next tile query of any mover in the same
//!   tick, and nothing else writes the plane.

use super::World;
use crate::object::ObjectId;

/// Logic names this family runs: the spawner placed in the level and the
/// pieces it builds.
pub const NAMES: &[&str] = &[SPAWNER, PIECE];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_BREAKPLANK_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = false;

/// The spawner's logic name.
pub const SPAWNER: &str = "BreakPlank";
/// The pieces' logic name.
pub const PIECE: &str = "SinglePlank";
/// The pieces' image set.
pub const IMAGE_SET: &str = "LEVEL_BREAKPLANK";
/// A piece at rest.
pub const IDLE_ANIM: &str = "LEVEL_BREAKPLANK_IDLE";
/// A piece crumbling; its second element plays the crumble sound.
pub const CRUMBLE_ANIM: &str = "LEVEL_BREAKPLANK_CRUMBLE";

/// A level's plank geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelPlanks {
    /// Distance between two pieces of a row, px.
    pub spacing: i32,
    /// Offset from a piece's y to the row of its floor cell, px.
    pub row_offset: i32,
}

/// The plank geometry of level `level`: the fifth and the eleventh levels
/// have planks; other levels have none.
pub fn level_table(level: u32) -> Option<LevelPlanks> {
    match level {
        5 => Some(LevelPlanks {
            spacing: 64,
            row_offset: -60,
        }),
        11 => Some(LevelPlanks {
            spacing: 64,
            row_offset: -66,
        }),
        _ => None,
    }
}

/// The main-plane cell (column, row) holding the floor of a piece at
/// (`x`, `y`) on a level with `planks`, for tiles of `tile` px.
pub fn floor_cell(planks: LevelPlanks, x: i32, y: i32, tile: i32) -> (i32, i32) {
    let shift = tile.trailing_zeros();
    (x >> shift, (y + planks.row_offset) >> shift)
}

/// A spawner's or a piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BreakPlankData {}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fifth_and_eleventh_levels_have_planks() {
        assert_eq!(
            level_table(5),
            Some(LevelPlanks {
                spacing: 64,
                row_offset: -60
            })
        );
        assert_eq!(level_table(11).map(|p| p.row_offset), Some(-66));
        for n in [0, 1, 2, 3, 4, 6, 10, 12, 14, 15] {
            assert_eq!(level_table(n), None, "level {n}");
        }
    }

    #[test]
    fn a_pieces_floor_cell_is_sixty_px_above_it_on_the_fifth_level() {
        let p = level_table(5).unwrap();
        // The row at (18727, 4825): its first piece's cell is column 292,
        // row 74.
        assert_eq!(floor_cell(p, 18727, 4825, 64), (292, 74));
        // Its second piece, one spacing to the right.
        assert_eq!(floor_cell(p, 18727 + p.spacing, 4825, 64), (293, 74));
        // Arithmetic shifts round towards minus infinity.
        assert_eq!(floor_cell(p, -1, 0, 64), (-1, -1));
    }
}
