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
//! # A piece's calls
//!
//! - Idle ([`state::IDLE`]): Claw standing or walking (his physics type,
//!   whatever his state: knocked back on a plank he counts), the bottom of
//!   his movement box strictly between the top and the bottom of the
//!   piece's, and his movement box strictly inside the piece's across: the
//!   piece is triggered. On the fifth level that is Claw's x from the
//!   piece's x - 23 to x + 18; the 22 px between two such windows start
//!   nothing. Pieces are called before Claw, so they see where his last
//!   tick left him.
//! - Triggered (once until the row is restored): it waits
//!   ([`state::WAIT`]) from the current game time.
//! - Wait: once the record's delay has passed (on the first tick at or
//!   past it: 28 ticks for 500 ms, 42 for 750 ms), it triggers its right
//!   neighbour, then its left one, and starts crumbling
//!   ([`state::CRUMBLE`]) on that tick, without advancing the crumble.
//! - Crumble: the crumble advances one tick per call. While it shows the
//!   middle picture of the set (picture 6 of 12, 26 calls in) the floor
//!   cell is cleared, so everything standing there falls in that very
//!   tick; once the crumble has ended (61 calls in) the piece is hidden.
//!   It stays there, hidden, until the row is restored. Its second picture
//!   (6 calls in) plays the crumble sound.
//!
//! A restore writes each piece's floor tile back into its cell and resets
//! the piece: idle, untriggered, visible, showing its first picture.
//!
//! # Defaults taken
//!
//! - The pieces change one cell of the level's main tile plane through the
//!   world ([`World::set_main_cell`]): the next tile query of any mover in
//!   the same tick sees it, and nothing else writes the plane.
//! - A level without a plank table builds its pieces on top of each other
//!   (no spacing) with their floor cells at their own height.

use super::{ObjectData, World, new_object};
use crate::level::runtime_image_set;
use crate::object::{ObjectId, Rect, draw_flags, flags};
use crate::physics::ptype;

/// Logic names this family runs: the spawner placed in the level and the
/// pieces it builds.
pub const NAMES: &[&str] = &[SPAWNER, PIECE];

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_BREAKPLANK_"];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

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

/// Record field of a row's delay, ms.
pub const DELAY_FIELD: usize = 59;
/// Record field of a row's number of pieces.
pub const WIDTH_FIELD: usize = 61;
/// What kind of object a piece is (it collides with nothing, so nothing
/// reads it).
pub const PIECE_TYPE: u32 = 1;
/// What could hit a piece (it collides with nothing, so nothing reads
/// it).
pub const PIECE_HIT_BY: u32 = 0x300_0072;
/// A piece's movement box when the level does not know its first picture:
/// the box of the fifth level's plank.
pub const FALLBACK_MOVE_RECT: Rect = Rect::new(-40, -88, 35, -25);

/// Logic states of the spawner and the pieces.
pub mod state {
    /// First call: the spawner builds its row.
    pub const SET_UP: i32 = 0;
    /// The spawner watching Claw's attempts; a piece waiting for Claw.
    pub const IDLE: i32 = 5;
    /// A triggered piece waiting its delay.
    pub const WAIT: i32 = 6;
    /// A piece crumbling, then hidden.
    pub const CRUMBLE: i32 = 7;
}

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

/// The number of ticks a piece waits for `delay_ms`: the first tick at or
/// past the delay.
pub fn wait_ticks(delay_ms: u32, tick_ms: u32) -> u32 {
    delay_ms.div_ceil(tick_ms.max(1))
}

/// True when Claw, standing at (`cx`, `cy`) with the movement box `cm`
/// and the physics type `physics`, triggers a piece at (`x`, `y`) with the
/// movement box `pm`.
pub fn claw_triggers(
    (x, y): (i32, i32),
    pm: Rect,
    (cx, cy): (i32, i32),
    cm: Rect,
    physics: i32,
) -> bool {
    let feet = cy + cm.bottom;
    matches!(physics, ptype::STAND | ptype::WALK)
        && y + pm.top < feet
        && feet < y + pm.bottom
        && cx + cm.left > x + pm.left
        && cx + cm.right < x + pm.right
}

/// A spawner's or a piece's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BreakPlankData {
    /// The spawner: its pieces, left to right.
    pub pieces: Vec<ObjectId>,
    /// The spawner: Claw's attempt counter when it last looked.
    pub attempt: u32,
    /// A piece: its left neighbour.
    pub left: Option<ObjectId>,
    /// A piece: its right neighbour.
    pub right: Option<ObjectId>,
    /// A piece: how long it waits once triggered, ms.
    pub delay_ms: u32,
    /// A piece: the game time it was triggered at, ms.
    pub wait_start: u64,
    /// A piece: triggered since the row was last restored.
    pub triggered: bool,
    /// A piece: its floor cell (column, row).
    pub cell: (i32, i32),
    /// A piece: the tile its floor cell held when it was built (-1 for
    /// none).
    pub floor_tile: i32,
}

/// The plank data of `o`, if it has any.
pub fn data_of(o: &crate::object::Object) -> Option<&BreakPlankData> {
    match &o.data {
        ObjectData::BreakPlank(d) => Some(d),
        _ => None,
    }
}

fn data<'a>(w: &'a World, id: ObjectId) -> Option<&'a BreakPlankData> {
    w.objects.get(id).and_then(data_of)
}

fn data_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut BreakPlankData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::BreakPlank(d) => Some(d),
        _ => None,
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let (spawner, st) = (&*o.logic == SPAWNER, o.state);
    match (spawner, st) {
        (true, state::SET_UP) => build_row(w, id),
        (true, state::IDLE) => watch_attempts(w, id),
        (false, state::IDLE) => idle_tick(w, id),
        (false, state::WAIT) => wait_tick(w, id),
        (false, state::CRUMBLE) => crumble_tick(w, id),
        _ => {}
    }
}

/// `attacker` hit `id`: nothing hits a plank.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let _ = (w, id, attacker);
}

/// `id` attacked `victim`: a plank attacks nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// The spawner's first call: hidden and out of pair collision, it builds
/// its pieces, links them and remembers Claw's attempt counter.
fn build_row(w: &mut World, id: ObjectId) {
    let planks = level_table(w.level_number()).unwrap_or(LevelPlanks {
        spacing: 0,
        row_offset: 0,
    });
    let o = w.objects.get_mut(id).expect("caller checked");
    o.flags |= flags::NO_COLLIDE;
    o.draw_flags |= draw_flags::HIDDEN;
    o.attack_rect = Rect::UNSET;
    let (x, y, z) = (o.x, o.y, o.z);
    let (delay, width) = o
        .record
        .as_ref()
        .map_or((0, 0), |r| (r[DELAY_FIELD], r[WIDTH_FIELD]));
    let delay_ms = delay.max(0) as u32;
    let width = width.max(1);
    let mut pieces = Vec::with_capacity(width as usize);
    for i in 0..width {
        let piece = build_piece(w, x + i * planks.spacing, y, z + 1, delay_ms, planks);
        pieces.push(piece);
    }
    for (i, &p) in pieces.iter().enumerate() {
        let left = i.checked_sub(1).map(|j| pieces[j]);
        let right = pieces.get(i + 1).copied();
        if let Some(d) = data_mut(w, p) {
            d.left = left;
            d.right = right;
        }
    }
    let attempt = w.attempt();
    let o = w.objects.get_mut(id).expect("caller checked");
    o.data = ObjectData::BreakPlank(BreakPlankData {
        pieces,
        attempt,
        ..BreakPlankData::default()
    });
    o.state = state::IDLE;
}

/// Builds one piece at (x, y, z) and resets it: it saves the tile of its
/// floor cell.
fn build_piece(
    w: &mut World,
    x: i32,
    y: i32,
    z: i32,
    delay_ms: u32,
    planks: LevelPlanks,
) -> ObjectId {
    let mut o = new_object(PIECE, x, y, z);
    o.image_set = runtime_image_set(IMAGE_SET);
    let first = w.image_frames(IMAGE_SET).map(|(first, _)| first);
    if let Some(first) = first {
        o.frame = first;
    }
    o.move_rect = first
        .and_then(|f| w.frame_box(IMAGE_SET, f))
        .unwrap_or(FALLBACK_MOVE_RECT);
    o.flags |= flags::NO_COLLIDE;
    o.type_flags = PIECE_TYPE;
    o.hit_type_flags = PIECE_HIT_BY;
    if o.z == 0 {
        o.z = 1;
        o.flags |= flags::RESORT;
    }
    let cell = floor_cell(planks, x, y, w.level.map.tile_h().max(1));
    let floor_tile = w.main_cell(cell.0, cell.1);
    o.data = ObjectData::BreakPlank(BreakPlankData {
        delay_ms,
        cell,
        floor_tile,
        ..BreakPlankData::default()
    });
    let id = w.spawn(o);
    reset_piece(w, id);
    id
}

/// Restores a piece: idle, untriggered, visible and showing its first
/// picture.
pub fn reset_piece(w: &mut World, id: ObjectId) {
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::IDLE;
    }
    w.set_anim(id, IDLE_ANIM);
    w.advance_anim(id);
    if let Some(d) = data_mut(w, id) {
        d.triggered = false;
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.draw_flags &= !draw_flags::HIDDEN;
    }
    w.advance_anim(id);
}

/// The spawner's later calls: after Claw lost a life, every piece's floor
/// is written back and every piece restored.
fn watch_attempts(w: &mut World, id: ObjectId) {
    let attempt = w.attempt();
    let Some(d) = data(w, id) else { return };
    if d.attempt == attempt {
        return;
    }
    let pieces = d.pieces.clone();
    for p in pieces {
        let Some(floor) = data(w, p).map(|d| (d.cell, d.floor_tile)) else {
            continue;
        };
        let ((col, row), tile) = floor;
        w.set_main_cell(col, row, tile);
        reset_piece(w, p);
    }
    if let Some(d) = data_mut(w, id) {
        d.attempt = attempt;
    }
}

/// Starts a piece's wait, unless it was triggered already.
pub fn trigger(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some(d) = data_mut(w, id) else { return };
    if d.triggered {
        return;
    }
    d.triggered = true;
    d.wait_start = now;
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::WAIT;
    }
}

fn idle_tick(w: &mut World, id: ObjectId) {
    let Some(c) = w
        .claw_object()
        .map(|c| ((c.x, c.y), c.move_rect, c.physics_type))
    else {
        return;
    };
    let o = w.objects.get(id).expect("caller checked");
    if claw_triggers((o.x, o.y), o.move_rect, c.0, c.1, c.2) {
        trigger(w, id);
    }
}

fn wait_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    let Some((start, delay, left, right)) =
        data(w, id).map(|d| (d.wait_start, d.delay_ms, d.left, d.right))
    else {
        return;
    };
    if (now.wrapping_sub(start) as u32) < delay {
        return;
    }
    if let Some(r) = right {
        trigger(w, r);
    }
    if let Some(l) = left {
        trigger(w, l);
    }
    if let Some(o) = w.objects.get_mut(id) {
        o.state = state::CRUMBLE;
    }
    w.set_anim(id, CRUMBLE_ANIM);
}

fn crumble_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let o = w.objects.get(id).expect("caller checked");
    let middle = w
        .image_frames(&o.image_set)
        .map(|(first, last)| (first + last) / 2);
    let (frame, done) = (o.frame, o.anim.is_done());
    if Some(frame) == middle
        && let Some((col, row)) = data(w, id).map(|d| d.cell)
    {
        w.set_main_cell(col, row, -1);
    }
    if done && let Some(o) = w.objects.get_mut(id) {
        o.draw_flags |= draw_flags::HIDDEN;
    }
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

    #[test]
    fn the_waits_end_on_the_first_tick_at_or_past_the_delay() {
        assert_eq!(wait_ticks(250, 18), 14);
        assert_eq!(wait_ticks(500, 18), 28);
        assert_eq!(wait_ticks(750, 18), 42);
        assert_eq!(wait_ticks(540, 18), 30);
        assert_eq!(wait_ticks(0, 18), 0);
    }

    #[test]
    fn claw_triggers_a_piece_from_x_minus_23_to_x_plus_18() {
        let claw = Rect::new(-16, -52, 16, 60);
        // The 500 ms row at (18727, 4633): Claw stands at y 4511.
        let piece = (18791, 4633);
        let on =
            |cx: i32, physics| claw_triggers(piece, FALLBACK_MOVE_RECT, (cx, 4511), claw, physics);
        assert!(on(18791 - 23, ptype::STAND));
        assert!(on(18791 + 18, ptype::STAND));
        assert!(on(18791, ptype::WALK));
        assert!(!on(18791 - 24, ptype::STAND));
        assert!(!on(18791 + 19, ptype::STAND));
        // Jumping, falling, ducking or climbing over it starts nothing.
        for p in [ptype::JUMP, ptype::FALL, ptype::DUCK, ptype::CLIMB] {
            assert!(!on(18791, p), "{p}");
        }
        // The feet must be strictly inside the piece's band.
        let at =
            |cy: i32| claw_triggers(piece, FALLBACK_MOVE_RECT, (18791, cy), claw, ptype::STAND);
        assert!(at(4633 - 88 - 60 + 1));
        assert!(!at(4633 - 88 - 60));
        assert!(at(4633 - 25 - 60 - 1));
        assert!(!at(4633 - 25 - 60));
    }
}
