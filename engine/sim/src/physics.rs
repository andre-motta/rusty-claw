//! Tile physics: moving an object toward a requested position.
//!
//! An object's logic decides where it wants to be this tick, in whole
//! pixels, and calls [`physics`]. The move is cut into steps of at most
//! 14 px per axis; each step is handed to a mover chosen by the object's
//! physics type, which tests the object's movement box against the tile
//! attributes, stops it at walls, lands it on floors, bumps its head on
//! ceilings, grabs ladders and reports what happened as a bit mask. The
//! logic then reads the new physics type and the mask.
//!
//! Solid blocks from every side. Ground is a one-way floor that only stops
//! an object whose feet were above it before the step. A ladder's top row is
//! a floor except while climbing. Death tiles mark the object.
//!
//! An object whose movement box is inside a floor finds nothing to stand
//! on: the ground movers count a floor pixel as support only with a free
//! pixel above it, and the fall mover lands only on a floor with a free
//! pixel between the old and the new feet. So an object standing up or
//! put down with its feet inside Solid or Ground turns to Fall and falls
//! through every such tile until its feet come out into clear air, then
//! lands on the next floor as usual. Walls still stop it sideways.
//!
//! Probes are not swept: each tests only the requested position, plus the
//! pixels between the old and the new edge in one column for floors and
//! ceilings. The 14 px step keeps every step shorter than a tile.

use crate::object::{Object, ObjectId, Rect, flags};
use crate::tiles::{CLIMB, DEATH, GROUND, SOLID, TileMap};

/// Physics types.
pub mod ptype {
    /// Standing (ground mover).
    pub const STAND: i32 = 1;
    /// Walking (ground mover).
    pub const WALK: i32 = 2;
    /// Rising (rise mover).
    pub const JUMP: i32 = 3;
    /// Falling (fall mover).
    pub const FALL: i32 = 4;
    /// Crouching (ground mover).
    pub const DUCK: i32 = 5;
    /// Climbing (climb mover).
    pub const CLIMB: i32 = 6;
    /// Moved straight to the target, no tiles tested.
    pub const DIRECT: i32 = 7;
    /// Flying: falls when the target is not above, else rises.
    pub const FLY: i32 = 8;
}

/// Bits of the option mask passed to [`physics`].
pub mod opt {
    /// Grab a ladder at the pixel just above the movement box.
    pub const GRAB_UP: u32 = 0x1;
    /// Grab a ladder two pixels below the box (ground mover, without
    /// [`GRAB_UP`]).
    pub const GRAB_DOWN: u32 = 0x2;
    /// Land on and keep riding platform objects.
    pub const PLATFORMS: u32 = 0x8;
    /// Centre on the ladder tile when grabbing.
    pub const CENTRE_ON_LADDER: u32 = 0x10;
}

/// Bits of the result mask.
pub mod res {
    /// Something stopped the object: set with [`WALL`], [`CEILING`] or
    /// [`FLOOR`].
    pub const HIT: u32 = 1 << 16;
    /// A wall blocked the step.
    pub const WALL: u32 = 1 << 17;
    /// The wall was on the right.
    pub const RIGHT: u32 = 1 << 18;
    /// The wall was on the left.
    pub const LEFT: u32 = 1 << 19;
    /// The object is marked as standing on a death tile.
    pub const DEATH_TILE: u32 = 1 << 20;
    /// The object rides a platform.
    pub const ELEVATOR: u32 = 1 << 21;
    /// Neither coordinate changed in the last step.
    pub const UNCHANGED: u32 = 1 << 22;
    /// A rising object hit a ceiling and now falls.
    pub const CEILING: u32 = 1 << 23;
    /// A falling object landed.
    pub const FLOOR: u32 = 1 << 24;
}

/// A platform object others can stand on: its hit box is the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The platform object.
    pub id: ObjectId,
    /// Position.
    pub x: i32,
    /// Position.
    pub y: i32,
    /// Surface box relative to the position.
    pub hit_rect: Rect,
    /// The platform's horizontal move this tick (riders add it to their
    /// target).
    pub move_x: i32,
    /// The platform's vertical move this tick (riders add it to their
    /// target); only its upward part widens the landing test.
    pub move_y: i32,
}

/// What the movers test against.
#[derive(Debug, Clone, Copy)]
pub struct Collider<'a> {
    /// Tile attributes of the main plane.
    pub map: &'a TileMap,
    /// Platform objects in list order.
    pub platforms: &'a [Platform],
    /// Largest step per axis.
    pub step: i32,
}

impl<'a> Collider<'a> {
    /// A collider over `map` with no platforms and the default step.
    pub fn new(map: &'a TileMap) -> Self {
        Collider {
            map,
            platforms: &[],
            step: 14,
        }
    }

    fn attr(&self, x: i32, y: i32) -> u8 {
        self.map.attr(x, y)
    }
}

fn solid_or_ground(a: u8) -> bool {
    a == SOLID || a == GROUND
}

fn row_samples(o: &Object, tx: i32) -> impl Iterator<Item = i32> + use<> {
    let r = o.move_rect;
    let step = o.move_res_x.max(1);
    let (start, end) = if r.left > r.right {
        (1, 0)
    } else {
        (tx + r.left, tx + r.right)
    };
    (0..)
        .map(move |i: i32| start.saturating_add(i.saturating_mul(step)))
        .take_while(move |&x| x <= end && start <= end)
}

/// Moves `o` toward (tx, ty) and returns the result mask of the last step.
pub fn physics(c: &Collider, o: &mut Object, tx: i32, ty: i32, mask: u32) -> u32 {
    let step = c.step.max(1);
    if (o.x - tx).abs() <= step && (o.y - ty).abs() <= step {
        return dispatch(c, o, tx, ty, mask);
    }
    if o.flags & flags::ON_ELEVATOR != 0 || o.physics_type == ptype::DIRECT {
        return dispatch(c, o, tx, ty, mask);
    }
    let entry = o.physics_type;
    let sx = if o.x <= tx { step } else { -step };
    let sy = if o.y <= ty { step } else { -step };
    // Every step moves toward the target or ends the call; the cap only
    // guards against malformed data.
    let guard = ((o.x - tx).unsigned_abs() + (o.y - ty).unsigned_abs()) / step as u32 + 16;
    let mut result = 0;
    for _ in 0..guard {
        let nx = if sx > 0 {
            (o.x + sx).min(tx)
        } else {
            (o.x + sx).max(tx)
        };
        let ny = if sy > 0 {
            (o.y + sy).min(ty)
        } else {
            (o.y + sy).max(ty)
        };
        result = dispatch(c, o, nx, ny, mask);
        if o.physics_type != entry
            || result & res::HIT != 0
            || (o.x == tx && o.y == ty)
            || result & res::UNCHANGED != 0
        {
            break;
        }
    }
    result
}

fn dispatch(c: &Collider, o: &mut Object, tx: i32, ty: i32, mask: u32) -> u32 {
    let (x0, y0) = (o.x, o.y);
    let mut r = 0;
    match o.physics_type {
        ptype::STAND | ptype::WALK | ptype::DUCK => r = ground(c, o, tx, ty, mask),
        ptype::JUMP => {
            r = rise(c, o, tx, ty, mask);
            if o.physics_type == ptype::FALL {
                r |= res::CEILING;
            }
        }
        ptype::FALL => {
            r = fall(c, o, tx, ty, mask);
            if o.physics_type == ptype::STAND {
                r |= res::FLOOR;
            }
        }
        ptype::CLIMB => r = climb(c, o, tx, ty, mask),
        ptype::DIRECT => {
            o.x = tx;
            o.y = ty;
        }
        ptype::FLY => {
            if ty >= y0 {
                r = fall(c, o, tx, ty, mask);
                if o.physics_type == ptype::STAND {
                    r |= res::FLOOR;
                }
            } else {
                r = rise(c, o, tx, ty, mask);
                if o.physics_type == ptype::FALL {
                    r |= res::CEILING;
                    o.physics_type = ptype::FLY;
                }
            }
        }
        _ => {}
    }
    if r & (res::WALL | res::CEILING | res::FLOOR) != 0 {
        r |= res::HIT;
    }
    if o.flags & flags::ON_DEATH_TILE != 0 {
        r |= res::DEATH_TILE;
    }
    if o.flags & flags::ON_ELEVATOR != 0 {
        r |= res::ELEVATOR;
    }
    if o.x == x0 && o.y == y0 {
        r |= res::UNCHANGED;
    }
    r
}

/// Tests the column at the leading edge; returns the allowed x and the wall
/// bits.
fn wall(c: &Collider, o: &Object, tx: i32, ty: i32) -> (i32, u32) {
    if tx == o.x {
        return (tx, 0);
    }
    let r = o.move_rect;
    if r.top > r.bottom || r.left > r.right {
        return (tx, 0);
    }
    let (x, bits) = if tx > o.x {
        (tx + r.right, res::WALL | res::RIGHT)
    } else {
        (tx + r.left, res::WALL | res::LEFT)
    };
    let last = ty + r.bottom;
    let step = o.move_res_y.max(1);
    let mut y = ty + r.top;
    loop {
        if c.attr(x, y) == SOLID {
            return (o.x, bits);
        }
        if y >= last {
            return (tx, 0);
        }
        y = (y + step).min(last);
    }
}

fn grab(
    c: &Collider,
    o: &mut Object,
    tx: i32,
    y: i32,
    mask: u32,
    probe: (i32, i32),
    r: u32,
) -> u32 {
    let mut tx = tx;
    if mask & opt::CENTRE_ON_LADDER != 0
        && let Some((left, right)) = c.map.ladder_span(probe.0, probe.1)
    {
        tx = (left + right) / 2;
    }
    o.physics_type = ptype::CLIMB;
    o.x = tx;
    o.y = y;
    r
}

fn ground(c: &Collider, o: &mut Object, tx: i32, ty: i32, mask: u32) -> u32 {
    let (tx, r) = wall(c, o, tx, ty);
    let y = if ty < o.y { up(c, o, tx, ty) } else { ty };
    let m = o.move_rect;
    if mask & opt::GRAB_UP != 0 {
        let probe = (tx, y + m.top - 1);
        if c.attr(probe.0, probe.1) == CLIMB {
            return grab(c, o, tx, y, mask, probe, r);
        }
    } else if mask & opt::GRAB_DOWN != 0 {
        let probe = (tx, y + m.bottom + 2);
        if c.attr(probe.0, probe.1) == CLIMB {
            return grab(c, o, tx, y, mask, probe, r);
        }
    }
    if o.flags & flags::ON_ELEVATOR != 0 {
        if !ride_check(c, o, tx, y, mask) {
            o.physics_type = ptype::FALL;
        }
    } else {
        ground_follow(c, o, tx, y);
    }
    o.x = tx;
    o.y = y;
    r
}

/// Decides whether the object stands on something at (tx, ty); sets Fall
/// when nothing supports it. Never changes Y.
fn ground_follow(c: &Collider, o: &mut Object, tx: i32, ty: i32) {
    let m = o.move_rect;
    if m.top > m.bottom || m.left > m.right {
        return;
    }
    let yb = ty + m.bottom + 1;
    for x in row_samples(o, tx) {
        let a = c.attr(x, yb);
        if solid_or_ground(a) {
            if !solid_or_ground(c.attr(x, yb - 1)) {
                return;
            }
        } else if a == CLIMB && o.physics_type != ptype::CLIMB && c.attr(x, yb - 1) != CLIMB {
            return;
        }
    }
    o.physics_type = ptype::FALL;
}

/// New Y for a move down to `ty`; lands the object (Stand) on the first
/// floor its feet cross.
fn down(c: &Collider, o: &mut Object, tx: i32, ty: i32) -> i32 {
    let m = o.move_rect;
    if m.top > m.bottom || m.left > m.right {
        return ty;
    }
    let yb = ty + m.bottom + 1;
    let old_feet = o.y + m.bottom;
    if c.attr(tx, yb) == DEATH {
        o.flags |= flags::ON_DEATH_TILE;
    }
    for x in row_samples(o, tx) {
        let a = c.attr(x, yb);
        if solid_or_ground(a) && yb >= old_feet {
            let mut y = yb;
            while y >= old_feet {
                if !solid_or_ground(c.attr(x, y)) {
                    o.physics_type = ptype::STAND;
                    return y - m.bottom;
                }
                y -= 1;
            }
        } else if a == CLIMB && o.physics_type != ptype::CLIMB && ty > o.y {
            let stop = o.y + m.bottom + 1;
            let mut y = yb - 1;
            while y >= stop {
                if c.attr(x, y) != CLIMB {
                    o.physics_type = ptype::STAND;
                    return y - m.bottom;
                }
                y -= 1;
            }
        }
    }
    ty
}

/// New Y for a move up to `ty`; a Solid ceiling stops the head and turns
/// the object to Fall.
fn up(c: &Collider, o: &mut Object, tx: i32, ty: i32) -> i32 {
    let m = o.move_rect;
    if m.top > m.bottom || m.left > m.right {
        return ty;
    }
    let yt = ty + m.top - 1;
    let old_top = o.y + m.top - 1;
    for x in row_samples(o, tx) {
        if c.attr(x, yt) == SOLID && yt <= old_top {
            // The scan always runs in the left-edge column.
            let column = tx + m.left;
            let mut y = yt;
            while y <= old_top {
                if c.attr(column, y) != SOLID {
                    o.physics_type = ptype::FALL;
                    return y - m.top;
                }
                y += 1;
            }
        }
    }
    ty
}

fn fall(c: &Collider, o: &mut Object, tx: i32, ty: i32, mask: u32) -> u32 {
    let (tx, mut r) = wall(c, o, tx, ty);
    let mut y = ty;
    let mut landed_on_platform = false;
    if mask & opt::PLATFORMS != 0
        && let Some(py) = platform_landing(c, o, tx, ty)
    {
        y = py;
        landed_on_platform = true;
    }
    if !landed_on_platform && o.physics_type != ptype::STAND {
        y = down(c, o, tx, ty);
    }
    let mut tx = tx;
    let m = o.move_rect;
    if mask & opt::GRAB_UP != 0 {
        let probe = (tx, y + m.top - 1);
        if c.attr(probe.0, probe.1) == CLIMB {
            if mask & opt::CENTRE_ON_LADDER != 0
                && let Some((left, right)) = c.map.ladder_span(probe.0, probe.1)
            {
                tx = (left + right) / 2;
            }
            o.physics_type = ptype::CLIMB;
        }
    }
    if o.physics_type == ptype::STAND && r & res::WALL != 0 {
        // The blocked step keeps the old X, so the repeated wall test has
        // nothing to test: the landing step drops the wall bits.
        r &= !(res::WALL | res::RIGHT | res::LEFT);
        let (nx, bits) = wall(c, o, tx, y);
        tx = nx;
        r |= bits;
    }
    o.x = tx;
    o.y = y;
    r
}

fn rise(c: &Collider, o: &mut Object, tx: i32, ty: i32, mask: u32) -> u32 {
    let (mut tx, r) = wall(c, o, tx, ty);
    let y = up(c, o, tx, ty);
    let m = o.move_rect;
    if mask & opt::GRAB_UP != 0 {
        let probe = (tx, y + m.top - 1);
        if c.attr(probe.0, probe.1) == CLIMB {
            if mask & opt::CENTRE_ON_LADDER != 0
                && let Some((left, right)) = c.map.ladder_span(probe.0, probe.1)
            {
                tx = (left + right) / 2;
            }
            o.physics_type = ptype::CLIMB;
        }
    }
    o.x = tx;
    o.y = y;
    r
}

fn climb(c: &Collider, o: &mut Object, tx: i32, ty: i32, _mask: u32) -> u32 {
    let m = o.move_rect;
    let ladder_at =
        |y: i32| c.attr(tx, y + m.top - 1) == CLIMB || c.attr(tx, y + m.bottom + 1) == CLIMB;
    let mut y;
    if o.y < ty {
        y = down(c, o, tx, ty);
        if o.physics_type != ptype::STAND && !ladder_at(y) {
            o.physics_type = ptype::FALL;
        }
    } else {
        y = up(c, o, tx, ty);
        if !ladder_at(y)
            && let Some(top) = ladder_top(c, tx, o.y + m.bottom + 1, y + m.bottom + 1)
            && top > y
        {
            o.physics_type = ptype::STAND;
            y = top - m.bottom - 1;
        }
    }
    let (tx, r) = wall(c, o, tx, y);
    o.x = tx;
    o.y = y;
    r
}

/// Scans up from `y_from - 1` to `y_to` for the first non-ladder pixel and
/// returns the row below it (the ladder's top row).
fn ladder_top(c: &Collider, x: i32, y_from: i32, y_to: i32) -> Option<i32> {
    if y_from <= y_to {
        return None;
    }
    let mut y = y_from - 1;
    while y >= y_to {
        if c.attr(x, y) != CLIMB {
            return Some(y + 1);
        }
        y -= 1;
    }
    None
}

fn platform_hit(p: &Platform, o: &Object, tx: i32, ty: i32) -> Option<i32> {
    if !p.hit_rect.is_set() || !o.move_rect.is_set() || o.y > ty {
        return None;
    }
    let m = o.move_rect;
    let top = p.y + p.hit_rect.top;
    let left = p.x + p.hit_rect.left;
    let right = p.x + p.hit_rect.right;
    let d = p.move_y.min(0);
    let old_feet = o.y + m.bottom;
    let new_feet = ty + m.bottom;
    let overlap = tx + m.right >= left && tx + m.left <= right;
    let reach = if ty == o.y {
        new_feet == top - 1
    } else {
        new_feet >= top - 1
    };
    (old_feet <= top - d && overlap && reach).then_some(top - m.bottom - 1)
}

fn platform_landing(c: &Collider, o: &mut Object, tx: i32, ty: i32) -> Option<i32> {
    for p in c.platforms {
        if let Some(y) = platform_hit(p, o, tx, ty) {
            o.object_below = Some(p.id);
            o.physics_type = ptype::STAND;
            o.flags |= flags::ON_ELEVATOR;
            return Some(y);
        }
    }
    None
}

fn ride_check(c: &Collider, o: &Object, tx: i32, ty: i32, mask: u32) -> bool {
    if mask & opt::PLATFORMS == 0 || !o.move_rect.is_set() {
        return false;
    }
    let Some(below) = o.object_below else {
        return false;
    };
    let Some(p) = c.platforms.iter().find(|p| p.id == below) else {
        return false;
    };
    if !p.hit_rect.is_set() {
        return false;
    }
    let m = o.move_rect;
    let left = p.x + p.hit_rect.left;
    let right = p.x + p.hit_rect.right;
    tx + m.right >= left && tx + m.left <= right && ty + m.bottom == p.y + p.hit_rect.top - 1
}

/// Drops the object onto the first floor at or below its feet, searching
/// only the column at its X. Returns false (and does not move it) when
/// there is no floor below or its movement box is unset.
pub fn align_to_ground(map: &TileMap, o: &mut Object) -> bool {
    if !o.move_rect.is_set() {
        return false;
    }
    let y0 = o.y + o.move_rect.bottom;
    let mut y = y0;
    loop {
        let a = map.attr(o.x, y);
        if a == SOLID || a == GROUND || a == CLIMB {
            o.y += y - y0 - 1;
            return true;
        }
        y += 1;
        if y >= map.height() {
            return false;
        }
    }
}

/// True when the pixel just below the feet at `x + dx` is a floor of any
/// kind (Solid, Ground or Climb), even when the feet are inside it.
pub fn support_at(map: &TileMap, o: &Object, dx: i32) -> bool {
    let a = map.attr(o.x + dx, o.y + o.move_rect.bottom + 1);
    a == SOLID || a == GROUND || a == CLIMB
}

/// Attribute of the feet row itself at `x + dx`.
pub fn tile_under_feet(map: &TileMap, o: &Object, dx: i32) -> u8 {
    map.attr(o.x + dx, o.y + o.move_rect.bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::LogicKind;

    fn body(x: i32, y: i32, ptype: i32) -> Object {
        let mut o = Object::new(0, x, y, 0, "Test".into(), LogicKind::Inert);
        o.move_rect = Rect::new(-16, -52, 16, 60);
        o.physics_type = ptype;
        o
    }

    #[test]
    fn row_samples_skip_the_right_edge() {
        let o = body(100, 0, ptype::STAND);
        let s: Vec<i32> = row_samples(&o, 100).collect();
        assert_eq!(s, vec![84, 94, 104, 114]);
    }

    #[test]
    fn direct_moves_without_tests() {
        let map = TileMap::from_ascii(64, &["###"]);
        let mut o = body(10, 10, ptype::DIRECT);
        let r = physics(&Collider::new(&map), &mut o, 150, 20, 0);
        assert_eq!((o.x, o.y, r), (150, 20, 0));
    }

    #[test]
    fn long_falls_are_stepped() {
        let map = TileMap::from_ascii(64, &["...", "...", "...", "###"]);
        let mut o = body(96, 0, ptype::FALL);
        let r = physics(&Collider::new(&map), &mut o, 96, 400, 0);
        // Feet rest on row 191, one pixel above the floor at 192.
        assert_eq!((o.y, o.physics_type), (131, ptype::STAND));
        assert_eq!(r & (res::HIT | res::FLOOR), res::HIT | res::FLOOR);
    }

    #[test]
    fn an_object_inside_a_floor_falls_through_until_clear() {
        // Three rows of Solid, two of air, a floor at 384.
        let map = TileMap::from_ascii(64, &["...", "###", "###", "###", "...", "...", "###"]);
        let c = Collider::new(&map);
        // Feet 1 px inside the top row: nothing holds him.
        let mut o = body(96, 4, ptype::STAND);
        physics(&c, &mut o, 96, 4, 0);
        assert_eq!((o.y, o.physics_type), (4, ptype::FALL));
        // The fall passes the Solid and lands on the floor below.
        let mut ys = Vec::new();
        for _ in 0..40 {
            if o.physics_type != ptype::FALL {
                break;
            }
            let y = o.y + 12;
            physics(&c, &mut o, 96, y, 0);
            ys.push(o.y);
        }
        assert_eq!((o.y, o.physics_type), (384 - 61, ptype::STAND));
        assert!(ys.windows(2).all(|p| p[1] > p[0]), "{ys:?}");
        // Feet on the top row's top: he stands there and is not moved.
        let mut o = body(96, 3, ptype::STAND);
        physics(&c, &mut o, 96, 3, 0);
        assert_eq!((o.y, o.physics_type), (3, ptype::STAND));
        let mut o = body(96, -40, ptype::FALL);
        physics(&c, &mut o, 96, 30, 0);
        assert_eq!((o.y, o.physics_type), (3, ptype::STAND));
    }

    #[test]
    fn align_drops_to_the_first_floor() {
        let map = TileMap::from_ascii(64, &["...", "...", "###"]);
        let mut o = body(96, 0, ptype::STAND);
        assert!(align_to_ground(&map, &mut o));
        assert_eq!(o.y + 60, 127);
        let mut inside = body(96, 100, ptype::STAND);
        assert!(align_to_ground(&map, &mut inside));
        assert_eq!(inside.y, 99);
        let empty = TileMap::from_ascii(64, &["..."]);
        let mut o = body(96, 0, ptype::STAND);
        assert!(!align_to_ground(&empty, &mut o));
        assert_eq!(o.y, 0);
    }

    #[test]
    fn platforms_carry_riders() {
        let map = TileMap::from_ascii(64, &["....", "....", "....", "...."]);
        let plat = Platform {
            id: 7,
            x: 100,
            y: 200,
            hit_rect: Rect::new(-40, 0, 40, 10),
            move_x: 0,
            move_y: 0,
        };
        let platforms = [plat];
        let c = Collider {
            map: &map,
            platforms: &platforms,
            step: 14,
        };
        let mut o = body(100, 130, ptype::FALL);
        let r = physics(&c, &mut o, 100, 142, opt::PLATFORMS);
        assert_eq!(o.y, 200 - 60 - 1);
        assert_eq!(o.physics_type, ptype::STAND);
        assert_eq!(o.object_below, Some(7));
        assert!(r & res::ELEVATOR != 0 && r & res::FLOOR != 0);
        o.physics_type = ptype::STAND;
        let y = o.y;
        physics(&c, &mut o, 102, y, opt::PLATFORMS);
        assert_eq!(o.physics_type, ptype::STAND);
        physics(&c, &mut o, 102, y, 0);
        assert_eq!(o.physics_type, ptype::FALL);
    }
}
