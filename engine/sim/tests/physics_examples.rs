//! The four reference cases of the tile physics: a wall, a landing on a
//! one-way floor, a jump up through it, and a climb to a ladder top. Every
//! map is 64 px tiles; "row r" covers y = 64r .. 64r + 63. Claw's standing
//! box is (-16, -52, 16, 60) with 10 px probe spacing.

use claw_sim::object::{LogicKind, Object, Rect};
use claw_sim::physics::{Collider, opt, physics, ptype, res};
use claw_sim::tiles::{CLEAR, Descriptor, GROUND, SOLID, TileMap};

fn claw(x: i32, y: i32, physics_type: i32) -> Object {
    let mut o = Object::new(0, x, y, 0, "CaptainClaw".into(), LogicKind::Inert);
    o.move_rect = Rect::new(-16, -52, 16, 60);
    o.physics_type = physics_type;
    o
}

fn empty() -> TileMap {
    TileMap::new(12, 12, 64, 64)
}

#[test]
fn walking_into_a_wall() {
    let mut map = empty();
    let solid = map.add_descriptor(Descriptor::single(64, SOLID));
    map.fill_cells((0, 11), (7, 7), solid);
    map.fill_cells((5, 5), (5, 6), solid);
    let c = Collider::new(&map);
    let mut o = claw(292, 387, ptype::WALK);
    let mut seen = Vec::new();
    for _ in 0..3 {
        let (tx, ty) = (o.x + 4, o.y);
        let r = physics(&c, &mut o, tx, ty, opt::PLATFORMS);
        seen.push((o.x, o.y, r));
    }
    assert_eq!(
        seen,
        vec![(296, 387, 0), (300, 387, 0), (300, 387, 0x47_0000)]
    );
    assert_eq!(
        seen[2].2,
        res::HIT | res::WALL | res::RIGHT | res::UNCHANGED
    );
    // A long move is stepped 14 px at a time.
    let mut o = claw(250, 387, ptype::WALK);
    let r = physics(&c, &mut o, 290, 387, opt::PLATFORMS);
    assert_eq!((o.x, r), (290, 0));
}

fn ground_band() -> TileMap {
    let mut map = empty();
    let band = map.add_descriptor(Descriptor::Double {
        width: 64,
        height: 64,
        outside: CLEAR,
        inside: GROUND,
        rect: [0, 0, 63, 15],
    });
    map.fill_cells((4, 5), (7, 7), band);
    map
}

#[test]
fn landing_on_ground_from_above() {
    let map = ground_band();
    let c = Collider::new(&map);
    let mask = opt::PLATFORMS | opt::CENTRE_ON_LADDER;
    let mut o = claw(300, 372, ptype::FALL);
    let r = physics(&c, &mut o, 300, 380, mask);
    assert_eq!((o.x, o.y, r), (300, 380, 0));
    let r = physics(&c, &mut o, 300, 392, mask);
    assert_eq!((o.x, o.y, o.physics_type), (300, 387, ptype::STAND));
    assert_eq!(r, 0x101_0000);
    assert_eq!(r, res::HIT | res::FLOOR);
    // Standing on it next tick: supported, nothing moves.
    let r = physics(&c, &mut o, 300, 387, mask);
    assert_eq!(
        (o.y, o.physics_type, r),
        (387, ptype::STAND, res::UNCHANGED)
    );
}

#[test]
fn jumping_up_through_ground() {
    let map = ground_band();
    let c = Collider::new(&map);
    let mut o = claw(300, 520, ptype::JUMP);
    for tick in 1..=10 {
        let (tx, ty) = (o.x, o.y - 14);
        let r = physics(&c, &mut o, tx, ty, 0);
        assert_eq!((o.y, r), (520 - 14 * tick, 0), "tick {tick}");
    }
    assert_eq!(o.y, 380);
    o.physics_type = ptype::FALL;
    let r = physics(&c, &mut o, 300, 384, 0x18);
    assert_eq!((o.y, r), (384, 0));
    let r = physics(&c, &mut o, 300, 392, 0x18);
    assert_eq!((o.y, o.physics_type, r), (387, ptype::STAND, 0x101_0000));
}

#[test]
fn jumping_into_a_solid_ceiling() {
    let mut map = empty();
    let solid = map.add_descriptor(Descriptor::single(64, SOLID));
    map.fill_cells((4, 5), (7, 7), solid);
    let c = Collider::new(&map);
    let mut o = claw(300, 580, ptype::JUMP);
    let r = physics(&c, &mut o, 300, 566, 0);
    assert_eq!((o.y, r), (566, 0));
    let r = physics(&c, &mut o, 300, 552, 0);
    assert_eq!((o.y, o.physics_type, r), (564, ptype::FALL, 0x81_0000));
    assert_eq!(r, res::HIT | res::CEILING);
}

#[test]
fn feet_inside_ground_fall_through() {
    let map = ground_band();
    let c = Collider::new(&map);
    // Apex with the feet inside the band (row 454): the next fall step
    // finds only Ground down to the old feet and falls through.
    let mut o = claw(300, 394, ptype::FALL);
    let r = physics(&c, &mut o, 300, 400, 0x18);
    assert_eq!((o.y, o.physics_type, r), (400, ptype::FALL, 0));
}

#[test]
fn climbing_to_a_ladder_top() {
    let mut map = empty();
    let climb = map.add_descriptor(Descriptor::single(64, claw_sim::tiles::CLIMB));
    let solid = map.add_descriptor(Descriptor::single(64, SOLID));
    map.fill_cells((4, 4), (3, 6), climb);
    map.fill_cells((0, 11), (7, 7), solid);
    let c = Collider::new(&map);
    let mut o = claw(287, 140, ptype::CLIMB);
    let r = physics(&c, &mut o, 287, 136, 0);
    assert_eq!((o.y, o.physics_type, r), (136, ptype::CLIMB, 0));
    let r = physics(&c, &mut o, 287, 132, 0);
    assert_eq!((o.y, o.physics_type, r), (132, ptype::CLIMB, 0));
    // Arrival: one pixel instead of four, Stand, and no result bit.
    let r = physics(&c, &mut o, 287, 128, 0);
    assert_eq!((o.y, o.physics_type, r), (131, ptype::STAND, 0));
    let r = physics(&c, &mut o, 287, 131, 0x18);
    assert_eq!(
        (o.y, o.physics_type, r),
        (131, ptype::STAND, res::UNCHANGED)
    );
    // Down held on the ladder top grabs it again.
    let r = physics(&c, &mut o, 287, 131, 0x1A);
    assert_eq!(
        (o.y, o.physics_type, r),
        (131, ptype::CLIMB, res::UNCHANGED)
    );
}

#[test]
fn death_tiles_mark_the_object() {
    let mut map = empty();
    let death = map.add_descriptor(Descriptor::single(64, claw_sim::tiles::DEATH));
    map.fill_cells((0, 11), (7, 7), death);
    let c = Collider::new(&map);
    let mut o = claw(300, 380, ptype::FALL);
    let r = physics(&c, &mut o, 300, 392, 0x18);
    assert!(r & res::DEATH_TILE != 0);
    assert!(o.flags & claw_sim::object::flags::ON_DEATH_TILE != 0);
    // Death tiles are not floors.
    assert_eq!(o.physics_type, ptype::FALL);
}
