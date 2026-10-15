//! The object record shared by every simulated thing: Claw, the camera,
//! level objects and, later, enemies and projectiles.

use std::sync::Arc;

use crate::anim::AnimPlayer;
use crate::logic::{GameLogic, ObjectData};

/// Index of an object in its container. Stable for the object's lifetime.
pub type ObjectId = u32;

/// An inclusive rectangle relative to an object's position (or absolute,
/// where noted). `left == Rect::UNSET_LEFT` marks an unset rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left edge.
    pub left: i32,
    /// Top edge.
    pub top: i32,
    /// Right edge (inclusive).
    pub right: i32,
    /// Bottom edge (inclusive).
    pub bottom: i32,
}

impl Rect {
    /// The `left` value of an unset rectangle.
    pub const UNSET_LEFT: i32 = i32::MIN;
    /// An unset rectangle.
    pub const UNSET: Rect = Rect {
        left: Rect::UNSET_LEFT,
        top: 0,
        right: 0,
        bottom: 0,
    };

    /// A rectangle from its four edges.
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// True unless the rectangle is unset.
    pub const fn is_set(&self) -> bool {
        self.left != Rect::UNSET_LEFT
    }

    /// The rectangle moved by (x, y).
    pub const fn translate(&self, x: i32, y: i32) -> Rect {
        Rect::new(
            self.left.wrapping_add(x),
            self.top.wrapping_add(y),
            self.right.wrapping_add(x),
            self.bottom.wrapping_add(y),
        )
    }

    /// The rectangle mirrored about x = 0; an unset rectangle stays unset.
    pub const fn mirrored(&self) -> Rect {
        if !self.is_set() {
            return *self;
        }
        Rect::new(
            self.right.wrapping_neg(),
            self.top,
            self.left.wrapping_neg(),
            self.bottom,
        )
    }

    /// Inclusive overlap: touching edges count.
    pub const fn overlaps(&self, other: &Rect) -> bool {
        !(self.left > other.right
            || self.right < other.left
            || self.top > other.bottom
            || self.bottom < other.top)
    }

    /// True when (x, y) lies inside, edges included.
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

/// Bits of [`Object::flags`].
pub mod flags {
    /// Takes no part in pair collision.
    pub const NO_COLLIDE: u32 = 1 << 0;
    /// Never deactivated; made live at level start.
    pub const ALWAYS_ACTIVE: u32 = 1 << 1;
    /// Cannot be hit or attacked in pair collision.
    pub const SAFE: u32 = 1 << 2;
    /// A hit subtracts the attacker's damage from health directly.
    pub const AUTO_HIT_DAMAGE: u32 = 1 << 3;
    /// Riding a platform object.
    pub const ON_ELEVATOR: u32 = 1 << 4;
    /// Destroyed when it leaves its activation window.
    pub const DESTROY_OUTSIDE: u32 = 1 << 5;
    /// Destroyed when the window centre is far outside the plane.
    pub const DESTROY_FAR: u32 = 1 << 6;
    /// Never hits or attacks others in pair collision.
    pub const HARMLESS: u32 = 1 << 7;
    /// Never listed: not updated, drawn or collided.
    pub const UNLISTED: u32 = 1 << 11;
    /// Destroy at the end of this update pass.
    pub const DESTROY: u32 = 1 << 16;
    /// Re-sort by depth at the end of this update pass.
    pub const RESORT: u32 = 1 << 17;
    /// Level object: collides only with level objects and can go dormant.
    pub const WORLD: u32 = 1 << 18;
    /// Gets one last logic call in the notify state before destruction.
    pub const NOTIFY_DESTROY: u32 = 1 << 19;
    /// Gets one logic call in the sleep state when it goes dormant.
    pub const NOTIFY_SLEEP: u32 = 1 << 20;
    /// Run the logic once at creation.
    pub const RUN_AT_CREATION: u32 = 1 << 21;
    /// Standing on a death tile.
    pub const ON_DEATH_TILE: u32 = 1 << 22;
    /// Activation class 1 (large window).
    pub const CLASS_1: u32 = 1 << 23;
    /// Activation class 2 (small window).
    pub const CLASS_2: u32 = 1 << 24;
    /// Animation frame sounds play only while the object is in view.
    pub const SOUND_IN_VIEW: u32 = 1 << 25;
}

/// Bits of [`Object::type_flags`], [`Object::hit_type_flags`] and
/// [`Object::attack_type_flags`]: what kind of object something is, which
/// kinds can hit it and which kinds it attacks. Logics set them at run
/// time; level records leave them 0.
pub mod types {
    /// Captain Claw.
    pub const PLAYER: u32 = 0x2;
    /// An enemy's body.
    pub const ENEMY: u32 = 0x4;
    /// An object Claw threw.
    pub const THROWN: u32 = 0x10;
    /// A shot from Claw's pistol.
    pub const PISTOL_SHOT: u32 = 0x20;
    /// An enemy's shot.
    pub const ENEMY_SHOT: u32 = 0x40;
    /// A special pickup (powerups, extra lives, warps, the end of the
    /// level).
    pub const SPECIAL: u32 = 0x100;
    /// A checkpoint flag.
    pub const CHECKPOINT: u32 = 0x800;
    /// A health pickup.
    pub const HEALTH: u32 = 0x10000;
    /// A treasure.
    pub const TREASURE: u32 = 0x40000;
    /// Magic charges.
    pub const MAGIC_AMMO: u32 = 0x100000;
    /// The large health potion.
    pub const SUPER_HEALTH: u32 = 0x200000;
    /// Pistol shots.
    pub const PISTOL_AMMO: u32 = 0x800000;
    /// A hazard's shot from the level itself.
    pub const LEVEL_SHOT: u32 = 0x1000000;
    /// What Claw is hit by from the start of a level.
    pub const CLAW_HIT_BY: u32 = 0x1B5_0544;
    /// The kinds an invulnerability window turns off and back on.
    pub const CLAW_SHIELDED: u32 = ENEMY | LEVEL_SHOT;
}

/// Bits of [`Object::special_flags`].
pub mod special {
    /// Claw can lift and throw it.
    pub const LIFTABLE: u32 = 0x1;
    /// An enemy whose touch does not hurt.
    pub const HARMLESS_TOUCH: u32 = 0x10;
    /// A shot that handles its own impact on Claw instead of being
    /// destroyed.
    pub const OWN_IMPACT: u32 = 0x20;
    /// A pickup on its way to the status bar; it removes itself.
    pub const FLYING_TO_HUD: u32 = 0x200;
}

/// Bits of [`Object::draw_flags`].
pub mod draw_flags {
    /// Hidden.
    pub const HIDDEN: u32 = 0x1;
    /// Mirrored horizontally (facing left).
    pub const MIRROR: u32 = 0x2;
    /// Flipped vertically.
    pub const INVERT: u32 = 0x4;
    /// Drawn as a flash this frame.
    pub const FLASH: u32 = 0x8;
}

/// Engine-written logic states.
pub mod engine_state {
    /// First logic call.
    pub const INIT: i32 = 0;
    /// Health dropped to zero through automatic hit damage (kill kind 0),
    /// or a logic killed the object with [`crate::logic::World::kill`]
    /// (which records the kind of kill in [`super::Object::kill_kind`]).
    pub const KILLED: i32 = 28;
    /// Last call before destruction.
    pub const DESTROYING: i32 = 29;
    /// Call when the object goes dormant.
    pub const SLEEPING: i32 = 30;
}

/// Which logic runs an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicKind {
    /// Captain Claw.
    Player,
    /// The camera.
    Camera,
    /// A logic this build does not implement yet; the object exists, can be
    /// drawn and activated, and does nothing.
    Inert,
    /// A level logic run by one of the families in [`crate::logic`].
    Game(GameLogic),
}

/// One object.
#[derive(Debug, Clone)]
pub struct Object {
    /// Id given by the creator (level objects: the record's id field).
    pub id: i32,
    /// Unique creation number.
    pub serial: u32,
    /// [`flags`] bits.
    pub flags: u32,
    /// Position in main-plane pixels.
    pub x: i32,
    /// Position in main-plane pixels.
    pub y: i32,
    /// Depth: the list is sorted by it, and it is the draw order.
    pub z: i32,
    /// Logic name.
    pub logic: Arc<str>,
    /// Logic implementation.
    pub kind: LogicKind,
    /// Registration flags of the logic.
    pub reg_flags: u32,
    /// Logic state.
    pub state: i32,
    /// Milliseconds before the logic runs again.
    pub time_delay: u32,
    /// Update passes before the logic runs again.
    pub frame_delay: u32,
    /// Physics type (see [`crate::physics::ptype`]).
    pub physics_type: i32,
    /// Movement box against the tiles.
    pub move_rect: Rect,
    /// Box that others' attacks hit.
    pub hit_rect: Rect,
    /// Box this object attacks with.
    pub attack_rect: Rect,
    /// Probe spacing, px.
    pub move_res_x: i32,
    /// Probe spacing, px.
    pub move_res_y: i32,
    /// What kind of object this is (collision mask).
    pub type_flags: u32,
    /// Kinds that can hit this object.
    pub hit_type_flags: u32,
    /// Kinds this object attacks.
    pub attack_type_flags: u32,
    /// Damage dealt.
    pub damage: i32,
    /// Health.
    pub health: i32,
    /// Published velocity, px/s.
    pub speed_x: i32,
    /// Published velocity, px/s.
    pub speed_y: i32,
    /// [`draw_flags`] bits.
    pub draw_flags: u32,
    /// Image-set key, empty when the object draws nothing.
    pub image_set: String,
    /// Frame number inside the image set.
    pub frame: i32,
    /// Creation position.
    pub editor: (i32, i32, i32),
    /// Activation point: the creation position, or where the object last
    /// went dormant.
    pub grid_point: (i32, i32),
    /// Platform object being ridden.
    pub object_below: Option<ObjectId>,
    /// Object that last hit this one.
    pub hit_ref: Option<ObjectId>,
    /// Object this one last attacked.
    pub att_ref: Option<ObjectId>,
    /// Has a hit handler.
    pub hit_handler: bool,
    /// Has an attack handler.
    pub attack_handler: bool,
    /// Level record values (71 integers), for logics of later milestones.
    pub record: Option<Box<[i32; 71]>>,
    /// Logic-specific flags (see [`special`]).
    pub special_flags: u32,
    /// The logic's own data.
    pub data: ObjectData,
    /// The move a platform made this tick, which riders add to their own.
    /// The update pass clears it at the start of the object's turn, so a
    /// platform logic sets it only in the ticks it moves.
    pub platform_move: (i32, i32),
    /// Kind of the kill the object sees with [`engine_state::KILLED`]: the
    /// kind given to [`crate::logic::World::kill`], or 0 for a kill by
    /// automatic hit damage. The logic reads it when it sees KILLED. An
    /// object that survives a kill keeps the kind until its next kill sets
    /// it again, so a later death by hit damage sees 0, not the old kind.
    pub kill_kind: i32,
    /// Animation player.
    pub anim: AnimPlayer,
    /// The object jumped this tick (drawn without interpolation).
    pub teleported: bool,
    pub(crate) prev: Option<ObjectId>,
    pub(crate) next: Option<ObjectId>,
    pub(crate) listed: bool,
}

impl Object {
    /// A new object at (x, y, z) running `logic`.
    pub fn new(id: i32, x: i32, y: i32, z: i32, logic: Arc<str>, kind: LogicKind) -> Object {
        Object {
            id,
            serial: 0,
            flags: 0,
            x,
            y,
            z,
            logic,
            kind,
            reg_flags: 0,
            state: engine_state::INIT,
            time_delay: 0,
            frame_delay: 0,
            physics_type: 0,
            move_rect: Rect::UNSET,
            hit_rect: Rect::UNSET,
            attack_rect: Rect::UNSET,
            move_res_x: 10,
            move_res_y: 10,
            type_flags: 0,
            hit_type_flags: 0,
            attack_type_flags: 0,
            damage: 0,
            health: 0,
            speed_x: 0,
            speed_y: 0,
            draw_flags: 0,
            image_set: String::new(),
            frame: 0,
            editor: (x, y, z),
            grid_point: (x, y),
            object_below: None,
            hit_ref: None,
            att_ref: None,
            hit_handler: false,
            attack_handler: false,
            record: None,
            special_flags: 0,
            data: ObjectData::None,
            platform_move: (0, 0),
            kill_kind: 0,
            anim: AnimPlayer::default(),
            teleported: false,
            prev: None,
            next: None,
            listed: false,
        }
    }

    /// Activation class: 0 (1600x1200 window), 1 (2560x1920) or 2
    /// (768x576).
    pub fn activation_class(&self) -> usize {
        if self.flags & flags::CLASS_1 != 0 {
            1
        } else if self.flags & flags::CLASS_2 != 0 {
            2
        } else {
            0
        }
    }

    /// True while the object is in the live list.
    pub fn is_listed(&self) -> bool {
        self.listed
    }

    /// Mirrors the movement, hit and attack boxes.
    pub fn mirror_rects(&mut self) {
        self.move_rect = self.move_rect.mirrored();
        self.hit_rect = self.hit_rect.mirrored();
        self.attack_rect = self.attack_rect.mirrored();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_rules() {
        let a = Rect::new(0, 0, 10, 10);
        assert!(a.overlaps(&Rect::new(10, 10, 20, 20)));
        assert!(!a.overlaps(&Rect::new(11, 0, 20, 10)));
        assert_eq!(a.translate(5, -5), Rect::new(5, -5, 15, 5));
        assert_eq!(
            Rect::new(-16, -52, 20, 60).mirrored(),
            Rect::new(-20, -52, 16, 60)
        );
        assert!(!Rect::UNSET.mirrored().is_set());
        assert!(a.contains(10, 0) && !a.contains(11, 0));
    }
}
