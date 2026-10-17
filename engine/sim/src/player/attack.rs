//! Claw's attacks: choosing one from the pressed buttons, running it on its
//! animation, launching his shots, and cancelling it.
//!
//! The host states call [`select`] once per tick in the game's order: Jump,
//! Fall and Crouch before their own update, Walk after its movement (an
//! attack started there stops the walk), and Stand before its update. While
//! an attack runs, the hosts call [`update`] in place of their plain
//! animation step.
//!
//! Choosing an attack only picks an attack type, starts its animation and
//! sets the attack box, the damage and where a shot leaves. From then on
//! the animation drives everything: each tick [`update`] advances it, and
//! the event value of the current animation frame says what happens. The
//! animation player reports every frame for at least one tick, so even a
//! 1 ms frame is seen.
//!
//! | Event | What happens |
//! | --- | --- |
//! | 1 | The attack box is live for that tick's collision pass. |
//! | 2 | The selected weapon's shot leaves, once per shot. |
//! | 3 | Pressing fire again rewinds the animation for another shot. |
//! | 4 | Dynamite is charged while the fire or dynamite key is held. |
//! | 5 | A sword powerup sends its shot, once per swing. |
//!
//! On every other tick Claw cannot hit anything.
//!
//! # Blows
//!
//! | Type | Attack | Animation | Attack box (facing right) | Damage |
//! | --- | --- | --- | --- | --- |
//! | 1 | sword swipe | `CLAW_SWIPE` | 30, -2, 112, 19 | 5 |
//! | 3 | punch | `CLAW_PUNCH` | 0, -2, 56, 19 | 5 |
//! | 4 | uppercut | `CLAW_UPPERCUT` | 0, -19, 56, 19 | 5 |
//! | 7 | crouching swipe | `CLAW_DUCKSWIPE` | 30, 19, 105, 37 | 2 |
//! | 8 | jumping swipe | `CLAW_JUMPSWIPE` | 30, -2, 112, 19 | 5 |
//! | 13 | kick | `CLAW_KICK` | 0, -2, 56, 19 | 5 |
//!
//! Standing or walking, the attack button picks a swipe, a punch, an
//! uppercut or a kick (see [`melee_choice`]); with a sword powerup it is
//! always the swipe, in the air the jumping swipe and crouching the
//! crouching swipe. A sword powerup plays the powerup variant of the
//! swipes, whose first frame sends the sword's shot. Catnip doubles the
//! damage of every blow. Facing left, the attack box is mirrored.
//!
//! # Shots
//!
//! The fire button, or a weapon key (which also selects that weapon),
//! starts the selected weapon's attack: standing, crouching or in the air.
//! Without ammunition the empty variant plays and nothing leaves.
//!
//! | Type | Attack | Animation | Shot leaves at (facing right) |
//! | --- | --- | --- | --- |
//! | 5 | pistol | `CLAW_PISTOL` | 71, -21 |
//! | 6 | magic | `CLAW_MAGIC` | 54, -8 |
//! | 9 | pistol in the air | `CLAW_JUMPPISTOL` | 71, -21 |
//! | 10 | magic in the air | `CLAW_JUMPMAGIC` | 75, -8 |
//! | 12 | dynamite | `CLAW_PREDYNAMITE`, then `CLAW_POSTDYNAMITE` | 60, -8 |
//! | 14 | crouching pistol | `CLAW_DUCKPISTOL` | 71, 23 |
//! | 15 | crouching magic | `CLAW_DUCKMAGIC` | 68, 24 |
//! | 16 | crouching dynamite | `CLAW_DUCKPREDYNAMITE`, then `CLAW_DUCKPOSTDYNAMITE` | 60, 12 |
//! | 18 | dynamite in the air | `CLAW_JUMPDYNAMITE` | 60, -16 |
//!
//! Shots are launched through [`logic::projectile::spawn`]:
//!
//! | Shot | Depth | Speed | Damage | Kind | Uses |
//! | --- | --- | --- | --- | --- | --- |
//! | `PistolBullet` | Claw's - 1 | 700 px/s | 8 | pistol shot | one pistol shot, when he has any |
//! | `MagicBullet` | Claw's + 1 | 700 px/s | 25 | magic shot | one magic charge, when he has any |
//! | `Dynamite` | Claw's + 1 | from the throw power | 15 | pistol shot | one stick |
//! | `FireBullet`, `LightningBullet`, `IceBullet` | Claw's + 1 | 700 px/s | 25 | magic shot | nothing |
//!
//! Every shot strikes enemies. Dynamite is thrown with a power from 0 to
//! 200 that grows with the time the key is held, from 50 ms (power 0) to
//! 2000 ms (power 200); thrown in the air it always has the power of a
//! 500 ms hold. Facing left the speeds and the power are negated. The
//! pistol's animation can be rewound for rapid fire while it has shots.
//!
//! Starting a pistol shot, a magic shot or a dynamite throw with
//! ammunition sometimes makes Claw say a line; that roll is one draw of
//! 0..100 from the game's shared random stream, made as the attack starts,
//! as in the original.
//!
//! # Weapons
//!
//! The toggle key cycles pistol, magic and dynamite while no attack runs
//! and plays [`TOGGLE_SOUND`]; both happen in the post-tick step, after
//! the state's handler, so the toggle works the same in every state. The
//! keys 1, 2 and 3 select a weapon and fire it.
//!
//! # Lifting
//!
//! The lift key, or Up with Attack, picks up a liftable object right in
//! front of Claw (see [`can_lift`]); the carrying itself is in
//! [`super::special`].

use claw_frame::{Buttons, SoundChannel, Weapon};

use super::{Ctx, DT, special, state};
use crate::anim::AnimPlayer;
use crate::logic::{self, projectile::ProjectileSpec};
use crate::object::{ObjectId, Rect, draw_flags, flags, special as special_flags, types};
use crate::physics::ptype;

/// Event value of the stand animation that asks for the attack's frame
/// hook.
pub const STAND_FRAME_EVENT: i32 = 99;

/// Bit of an object's special flags: Claw's punches and kicks reach it
/// from closer, and the melee choice picks a close blow against it.
pub const PUNCHABLE: u32 = 0x400;

/// Object kind (type flags) of a curse, which Claw's melee also aims at.
pub const CURSE_TYPE: u32 = 0x200;

/// Animation event: the melee attack box is live.
pub const EVENT_STRIKE: i32 = 1;
/// Animation event: a projectile leaves.
pub const EVENT_RELEASE: i32 = 2;
/// Animation event: the pistol may fire again.
pub const EVENT_REFIRE: i32 = 3;
/// Animation event: dynamite is being charged.
pub const EVENT_CHARGE: i32 = 4;
/// Animation event: a sword powerup shot leaves.
pub const EVENT_SWORD_SHOT: i32 = 5;

/// Powerup codes of the four swords.
pub const SWORD_POWERUPS: std::ops::RangeInclusive<u32> = 0x26B8..=0x26BB;
/// Powerup code of the fire sword.
pub const FIRE_SWORD: u32 = 0x26B8;
/// Powerup code of the lightning sword.
pub const LIGHTNING_SWORD: u32 = 0x26B9;
/// Powerup code of the ice sword.
pub const ICE_SWORD: u32 = 0x26BA;
/// Powerup code of catnip, which doubles melee damage.
pub const CATNIP_POWERUP: u32 = 0x26BC;

/// Attack types.
pub mod kind {
    /// No attack.
    pub const NONE: i32 = 0;
    /// Standing sword swipe.
    pub const SWIPE: i32 = 1;
    /// Punch.
    pub const PUNCH: i32 = 3;
    /// Uppercut.
    pub const UPPERCUT: i32 = 4;
    /// Pistol shot.
    pub const PISTOL: i32 = 5;
    /// Magic.
    pub const MAGIC: i32 = 6;
    /// Crouching swipe.
    pub const DUCK_SWIPE: i32 = 7;
    /// Jumping swipe.
    pub const JUMP_SWIPE: i32 = 8;
    /// Jumping pistol shot.
    pub const JUMP_PISTOL: i32 = 9;
    /// Jumping magic.
    pub const JUMP_MAGIC: i32 = 10;
    /// No animation of its own: a lift attempt.
    pub const LIFT: i32 = 11;
    /// Dynamite throw.
    pub const DYNAMITE: i32 = 12;
    /// Kick.
    pub const KICK: i32 = 13;
    /// Crouching pistol shot.
    pub const DUCK_PISTOL: i32 = 14;
    /// Crouching magic.
    pub const DUCK_MAGIC: i32 = 15;
    /// Crouching dynamite throw.
    pub const DUCK_DYNAMITE: i32 = 16;
    /// Jumping dynamite throw.
    pub const JUMP_DYNAMITE: i32 = 18;
}

/// The melee blows the choice picks from, by index.
pub const MELEE_TABLE: [i32; 4] = [kind::SWIPE, kind::UPPERCUT, kind::PUNCH, kind::KICK];

/// Targets closer than this (px) are aimed at; farther ones are ignored.
pub const TARGET_RANGE: i32 = 150;
/// A punchable target closer than this (px) gets a punch, uppercut or
/// kick.
pub const CLOSE_RANGE: i32 = 32;
/// Objects this far above or below Claw (px) or farther are not targets.
pub const TARGET_HEIGHT: i32 = 100;
/// How much sooner a target that is not punchable counts as in reach, px.
pub const TARGET_MARGIN: i32 = 48;
/// Distance reported when no target is found.
pub const NO_TARGET: i32 = 999_999;

/// A liftable object must be closer than this, px.
pub const LIFT_RANGE: i32 = 64;
/// How far behind Claw's position a liftable object may be, px (none for
/// a punchable one).
pub const LIFT_BEHIND: i32 = 20;
/// Largest height difference between Claw's feet and the object's, px.
pub const LIFT_FEET: i32 = 20;

/// Speed of pistol, magic and sword shots, px/s.
pub const SHOT_SPEED: i32 = 700;
/// Damage of a pistol shot.
pub const PISTOL_DAMAGE: i32 = 8;
/// Damage of a magic shot and of a sword shot.
pub const MAGIC_DAMAGE: i32 = 25;
/// Damage of a stick of dynamite.
pub const DYNAMITE_DAMAGE: i32 = 15;
/// Kind (type flags) of a magic or sword shot.
pub const MAGIC_SHOT_TYPE: u32 = 0x200_0000;
/// Special flag of magic and sword shots.
pub const MAGIC_SHOT_FLAG: u32 = 0x8;
/// Special flag of a fire sword shot.
pub const FIRE_SHOT_FLAG: u32 = 0x4_0000;
/// Special flag of an ice sword shot.
pub const ICE_SHOT_FLAG: u32 = 0x8_0000;
/// Special flag of a lightning sword shot.
pub const LIGHTNING_SHOT_FLAG: u32 = 0x10_0000;
/// Image set of a pistol shot.
pub const PISTOL_SHOT_IMAGE: &str = "GAME_BULLETS";

/// Shortest dynamite charge, ms: a tap throws with power 0.
pub const THROW_MIN_MS: i32 = 50;
/// Longest dynamite charge, ms: power 200.
pub const THROW_MAX_MS: i32 = 2000;
/// Strongest throw power.
pub const THROW_MAX_POWER: i32 = 200;
/// Charge a throw in the air starts with, ms.
pub const AIR_THROW_MS: i32 = 500;

/// Sound of the weapon toggle.
pub const TOGGLE_SOUND: &str = "GAME_CLICK";

/// Damage of every blow with the strong blows override.
pub const STRONG_DAMAGE: i32 = 20;
/// Damage of the crouching swipe with the strong blows override.
pub const STRONG_DUCK_DAMAGE: i32 = 10;
/// Damage of every attack with the one-hit override.
pub const ONE_HIT_DAMAGE: i32 = 100;
/// How much farther the attack box reaches with the long reach override,
/// px.
pub const LONG_REACH: i32 = 320;

/// Game-wide switches that change Claw's attacks. All are off in normal
/// play; the cheat codes that turn them on are not wired yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttackOverrides {
    /// Blows deal [`STRONG_DAMAGE`] ([`STRONG_DUCK_DAMAGE`] crouching),
    /// catnip or not.
    pub strong_blows: bool,
    /// Every attack deals [`ONE_HIT_DAMAGE`].
    pub one_hit: bool,
    /// Every attack deals no damage (unless [`Self::one_hit`] is on).
    pub no_damage: bool,
    /// The attack box reaches [`LONG_REACH`] px farther.
    pub long_reach: bool,
}

/// The running attack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttackData {
    /// Attack type, 0 when none runs.
    pub kind: i32,
    /// The attack's active part is over: the host state may leave it (crouch
    /// to stand, stand to crouch).
    pub interruptible: bool,
    /// Animation to go back to when the attack ends.
    pub restore_anim: Option<String>,
    /// Number of attacks started so far, also counting each rapid-fire
    /// shot. An enemy takes at most one hit per attack: it remembers the
    /// number of the attack that hit it.
    pub serial: u32,
    /// The active frame has been reached (or, for a charged throw, the
    /// attack started).
    pub struck: bool,
    /// The attack is the uppercut: enemies show their high hit reaction.
    pub uppercut: bool,
    /// Where a projectile leaves, relative to Claw facing right.
    pub offset: (i32, i32),
    /// The animation the attack plays; another one playing means the
    /// attack lost its animation.
    pub anim: Option<String>,
    /// The current shot has left (cleared for a rapid-fire shot).
    pub shot_used: bool,
    /// The sword shot of this swing has left.
    pub sword_shot_used: bool,
    /// Animation element a rapid-fire shot rewinds to.
    pub rewind_to: usize,
    /// Dynamite charge, ms.
    pub throw_ms: i32,
    /// Shortest charge, ms.
    pub throw_min_ms: i32,
    /// Longest charge, ms.
    pub throw_max_ms: i32,
    /// Animation that releases the charged throw.
    pub post_anim: Option<String>,
    /// Game-wide attack switches.
    pub overrides: AttackOverrides,
}

/// One melee attack type's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Melee {
    /// Attack type.
    pub kind: i32,
    /// Animation.
    pub anim: &'static str,
    /// Animation with a sword powerup, when it differs.
    pub sword_anim: Option<&'static str>,
    /// Attack box facing right.
    pub rect: Rect,
    /// Damage without powerups.
    pub damage: i32,
    /// Where a shot leaves, facing right, when the type sets it.
    pub offset: Option<(i32, i32)>,
}

/// The melee attack types.
pub const MELEE: [Melee; 6] = [
    Melee {
        kind: kind::SWIPE,
        anim: "CLAW_SWIPE",
        sword_anim: Some("CLAW_POWERUPSWIPE"),
        rect: Rect::new(30, -2, 112, 19),
        damage: 5,
        offset: Some((95, 15)),
    },
    Melee {
        kind: kind::PUNCH,
        anim: "CLAW_PUNCH",
        sword_anim: None,
        rect: Rect::new(0, -2, 56, 19),
        damage: 5,
        offset: None,
    },
    Melee {
        kind: kind::UPPERCUT,
        anim: "CLAW_UPPERCUT",
        sword_anim: None,
        rect: Rect::new(0, -19, 56, 19),
        damage: 5,
        offset: None,
    },
    Melee {
        kind: kind::DUCK_SWIPE,
        anim: "CLAW_DUCKSWIPE",
        sword_anim: Some("CLAW_DUCKPOWERUPSWIPE"),
        rect: Rect::new(30, 19, 105, 37),
        damage: 2,
        offset: Some((95, 25)),
    },
    Melee {
        kind: kind::JUMP_SWIPE,
        anim: "CLAW_JUMPSWIPE",
        sword_anim: Some("CLAW_JUMPPOWERUPSWIPE"),
        rect: Rect::new(30, -2, 112, 19),
        damage: 5,
        offset: Some((75, 10)),
    },
    Melee {
        kind: kind::KICK,
        anim: "CLAW_KICK",
        sword_anim: None,
        rect: Rect::new(0, -2, 56, 19),
        damage: 5,
        offset: None,
    },
];

/// The settings of melee attack type `kind`.
pub fn melee(kind: i32) -> Option<&'static Melee> {
    MELEE.iter().find(|m| m.kind == kind)
}

/// One shooting attack type's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    /// Attack type.
    pub kind: i32,
    /// The weapon whose ammunition decides between the two animations.
    pub weapon: Weapon,
    /// Animation with ammunition.
    pub anim: &'static str,
    /// Animation without ammunition.
    pub empty_anim: &'static str,
    /// Where the shot leaves, facing right.
    pub offset: (i32, i32),
    /// Animation element a rapid-fire shot rewinds to.
    pub rewind_to: usize,
    /// Charge a throw starts with, ms (dynamite only).
    pub throw_ms: Option<i32>,
    /// Animation that releases a charged throw.
    pub post_anim: Option<&'static str>,
    /// A line Claw may say when he starts it with ammunition, and the
    /// chance in percent.
    pub voice: Option<(&'static str, u32)>,
}

const fn shot(
    kind: i32,
    weapon: Weapon,
    anim: &'static str,
    empty_anim: &'static str,
    offset: (i32, i32),
    rewind_to: usize,
) -> Shot {
    Shot {
        kind,
        weapon,
        anim,
        empty_anim,
        offset,
        rewind_to,
        throw_ms: None,
        post_anim: None,
        voice: None,
    }
}

/// The shooting attack types.
pub const SHOTS: [Shot; 9] = [
    Shot {
        voice: Some(("CLAW_1003", 30)),
        ..shot(
            kind::PISTOL,
            Weapon::Pistol,
            "CLAW_PISTOL",
            "CLAW_EMPTYPISTOL",
            (71, -21),
            2,
        )
    },
    shot(
        kind::MAGIC,
        Weapon::Magic,
        "CLAW_MAGIC",
        "CLAW_EMPTYMAGIC",
        (54, -8),
        1,
    ),
    shot(
        kind::JUMP_PISTOL,
        Weapon::Pistol,
        "CLAW_JUMPPISTOL",
        "CLAW_EMPTYJUMPPISTOL",
        (71, -21),
        1,
    ),
    shot(
        kind::JUMP_MAGIC,
        Weapon::Magic,
        "CLAW_JUMPMAGIC",
        "CLAW_EMPTYJUMPMAGIC",
        (75, -8),
        1,
    ),
    Shot {
        throw_ms: Some(0),
        post_anim: Some("CLAW_POSTDYNAMITE"),
        voice: Some(("CLAW_1009", 25)),
        ..shot(
            kind::DYNAMITE,
            Weapon::Dynamite,
            "CLAW_PREDYNAMITE",
            "CLAW_EMPTYDYNAMITE",
            (60, -8),
            1,
        )
    },
    shot(
        kind::DUCK_PISTOL,
        Weapon::Pistol,
        "CLAW_DUCKPISTOL",
        "CLAW_DUCKEMPTYPISTOL",
        (71, 23),
        2,
    ),
    shot(
        kind::DUCK_MAGIC,
        Weapon::Magic,
        "CLAW_DUCKMAGIC",
        "CLAW_DUCKEMPTYMAGIC",
        (68, 24),
        0,
    ),
    Shot {
        throw_ms: Some(0),
        post_anim: Some("CLAW_DUCKPOSTDYNAMITE"),
        voice: Some(("CLAW_1009", 35)),
        ..shot(
            kind::DUCK_DYNAMITE,
            Weapon::Dynamite,
            "CLAW_DUCKPREDYNAMITE",
            "CLAW_DUCKEMPTYDYNAMITE",
            (60, 12),
            1,
        )
    },
    Shot {
        throw_ms: Some(AIR_THROW_MS),
        ..shot(
            kind::JUMP_DYNAMITE,
            Weapon::Dynamite,
            "CLAW_JUMPDYNAMITE",
            "CLAW_EMPTYJUMPDYNAMITE",
            (60, -16),
            1,
        )
    },
];

/// The settings of shooting attack type `kind`.
pub fn shot_of(kind: i32) -> Option<&'static Shot> {
    SHOTS.iter().find(|s| s.kind == kind)
}

/// The sword shots: powerup, logic name, special flag and sound.
pub const SWORD_SHOTS: [(u32, &str, u32, &str); 3] = [
    (FIRE_SWORD, "FireBullet", FIRE_SHOT_FLAG, "CLAW_FIRESWORD"),
    (
        LIGHTNING_SWORD,
        "LightningBullet",
        LIGHTNING_SHOT_FLAG,
        "CLAW_LIGHTSWORD",
    ),
    (ICE_SWORD, "IceBullet", ICE_SHOT_FLAG, "CLAW_ICESWORD"),
];

/// The running powerup, 0 for none.
fn powerup(c: &Ctx) -> u32 {
    if c.p.stats.powerup_ms > 0 {
        c.p.stats.powerup_kind
    } else {
        0
    }
}

fn has_sword(c: &Ctx) -> bool {
    SWORD_POWERUPS.contains(&powerup(c))
}

/// `lo + rand() % (hi - lo + 1)`: one draw from the game's random stream.
fn rand_range(c: &mut Ctx, lo: i32, hi: i32) -> i32 {
    let n = (hi - lo + 1) as u32;
    lo + c.rng.below(n) as i32
}

/// Starts an attack when a button asks for one; true when one started, or
/// when the lift key lifted something.
///
/// The buttons pressed this tick are tested in the game's order, and the
/// first one pressed decides: Attack, Projectile, the Pistol, Magic and
/// Dynamite keys, then Lift. Nothing happens while an attack runs.
pub fn select(c: &mut Ctx) -> bool {
    if c.p.attacking {
        return false;
    }
    let pressed = c.input.pressed;
    let mut lifted = false;
    if pressed.contains(Buttons::ATTACK) {
        match c.o.state {
            state::FALL | state::JUMP => start(c, kind::JUMP_SWIPE),
            state::CROUCH => start(c, kind::DUCK_SWIPE),
            _ if has_sword(c) => start(c, kind::SWIPE),
            _ => melee_choice(c),
        }
    } else if pressed.contains(Buttons::PROJECTILE) {
        start_weapon(c);
    } else if pressed.contains(Buttons::PISTOL) {
        c.p.stats.weapon = Weapon::Pistol;
        start_weapon(c);
    } else if pressed.contains(Buttons::MAGIC) {
        c.p.stats.weapon = Weapon::Magic;
        start_weapon(c);
    } else if pressed.contains(Buttons::DYNAMITE) {
        c.p.stats.weapon = Weapon::Dynamite;
        start_weapon(c);
    } else if pressed.contains(Buttons::LIFT) && c.o.physics_type == ptype::STAND {
        lifted = try_lift(c);
    }
    c.p.attacking || lifted
}

/// Starts the selected weapon's attack for the state Claw is in.
fn start_weapon(c: &mut Ctx) {
    let air = matches!(c.o.state, state::JUMP | state::FALL);
    let crouch = c.o.state == state::CROUCH;
    let kind = match (c.p.stats.weapon, air, crouch) {
        (Weapon::Pistol, true, _) => kind::JUMP_PISTOL,
        (Weapon::Pistol, _, true) => kind::DUCK_PISTOL,
        (Weapon::Pistol, _, _) => kind::PISTOL,
        (Weapon::Magic, true, _) => kind::JUMP_MAGIC,
        (Weapon::Magic, _, true) => kind::DUCK_MAGIC,
        (Weapon::Magic, _, _) => kind::MAGIC,
        (Weapon::Dynamite, true, _) => kind::JUMP_DYNAMITE,
        (Weapon::Dynamite, _, true) => kind::DUCK_DYNAMITE,
        (Weapon::Dynamite, _, _) => kind::DYNAMITE,
    };
    start(c, kind);
}

/// The nearest enemy or curse in front of Claw and the gap to it in px:
/// the distance from Claw's hit box to the near edge of the target's,
/// counted `TARGET_MARGIN` px closer for a target that is not
/// [`PUNCHABLE`]. Objects [`TARGET_HEIGHT`] px or more above or below are
/// skipped, and so are objects already flagged for removal this pass (an
/// enemy that died earlier in the same pass); on equal distances the one
/// earlier in the update order wins. The gap is negative when the target overlaps Claw's front, and
/// [`NO_TARGET`] when there is none.
pub fn find_target(c: &Ctx) -> (Option<ObjectId>, i32) {
    let mut best = NO_TARGET;
    let mut target = None;
    let (cx, cy) = (c.o.x, c.o.y);
    let mut cur = c.objects.head();
    while let Some(id) = cur {
        cur = c.objects.next_of(id);
        if id == c.id {
            continue;
        }
        let Some(o) = c.objects.get(id) else { continue };
        if (o.type_flags != types::ENEMY && o.type_flags != CURSE_TYPE)
            || o.flags & flags::DESTROY != 0
            || !o.hit_rect.is_set()
            || (o.y - cy).abs() >= TARGET_HEIGHT
        {
            continue;
        }
        let margin = if o.special_flags & PUNCHABLE != 0 {
            0
        } else {
            TARGET_MARGIN
        };
        let dist = if c.p.facing_right {
            let edge = o.x + o.hit_rect.left + margin;
            if edge <= cx {
                continue;
            }
            edge - (cx + c.o.hit_rect.right)
        } else {
            let edge = o.x + o.hit_rect.right - margin;
            if cx <= edge {
                continue;
            }
            cx + c.o.hit_rect.left - edge
        };
        if dist < best {
            best = dist;
            target = Some(id);
        }
    }
    (target, best)
}

/// The standing melee choice: a swipe, punch, uppercut or kick.
///
/// The game's random stream is drawn in this order: one draw of 0..=3 on
/// every press, a second one when the first gave 0 or 3, and one of 1..=3
/// for a punchable target closer than [`CLOSE_RANGE`]. Holding Up (and
/// neither Left nor Right) asks for a lift instead and starts no blow.
///
/// - A target farther than [`TARGET_RANGE`] is ignored.
/// - A target that is not punchable gets the swipe.
/// - A punchable target closer than [`CLOSE_RANGE`] gets an uppercut,
///   punch or kick at random; a farther one in range gets the swipe.
/// - Without a target the rolled blow is used: a swipe 1 time in 8, an
///   uppercut 3 in 8, a punch 3 in 8 and a kick 1 in 8.
pub fn melee_choice(c: &mut Ctx) {
    let (target, dist) = find_target(c);
    let mut r = rand_range(c, 0, 3);
    let target = if dist > TARGET_RANGE { None } else { target };
    if r == 0 || r == 3 {
        r = rand_range(c, 0, 3);
    }
    let up_only = c.input.held.contains(Buttons::UP)
        && !c.input.held.contains(Buttons::LEFT)
        && !c.input.held.contains(Buttons::RIGHT);
    if up_only {
        if let Some(t) = target
            && dist < LIFT_RANGE
            && can_lift(c, t)
        {
            special::begin_lift(c, t);
        }
        return;
    }
    let punchable = target
        .and_then(|t| c.objects.get(t))
        .map(|o| o.special_flags & PUNCHABLE != 0);
    let index = match punchable {
        Some(false) => 0,
        Some(true) if dist < CLOSE_RANGE => rand_range(c, 1, 3),
        Some(true) if dist < TARGET_RANGE => 0,
        _ => r,
    };
    start(c, MELEE_TABLE[index as usize]);
}

/// The lift key: lifts the nearest target when it is liftable, closer than
/// [`LIFT_RANGE`] and [`can_lift`] allows it. True when Claw lifted it.
fn try_lift(c: &mut Ctx) -> bool {
    if c.o.physics_type != ptype::STAND {
        return false;
    }
    let (target, dist) = find_target(c);
    match target {
        Some(t) if dist < LIFT_RANGE && can_lift(c, t) => {
            special::begin_lift(c, t);
            true
        }
        _ => false,
    }
}

/// True when Claw can lift `target`: it is liftable and not flagged for
/// removal, no attack runs, Claw stands or walks, the object is not more
/// than [`LIFT_BEHIND`] px behind his position (any distance for a
/// punchable one) and their feet are at most [`LIFT_FEET`] px apart in
/// height.
pub fn can_lift(c: &Ctx, target: ObjectId) -> bool {
    let Some(t) = c.objects.get(target) else {
        return false;
    };
    if t.special_flags & special_flags::LIFTABLE == 0
        || t.flags & flags::DESTROY != 0
        || c.p.attacking
    {
        return false;
    }
    if c.o.physics_type != ptype::STAND && c.o.physics_type != ptype::WALK {
        return false;
    }
    let margin = if t.special_flags & PUNCHABLE != 0 {
        0
    } else {
        LIFT_BEHIND
    };
    let behind = if c.p.facing_right {
        c.o.x - margin > t.x
    } else {
        c.o.x + margin < t.x
    };
    if behind {
        return false;
    }
    let feet = (c.o.y + c.o.move_rect.bottom) - (t.y + t.move_rect.bottom);
    feet.abs() <= LIFT_FEET
}

/// Starts attack type `kind`: counts the attack, saves the animation to
/// come back to and sets the type's animation (not advanced yet), attack
/// box, damage and shot offset. Afterwards the game-wide overrides apply
/// and, facing left, the attack box is mirrored.
///
/// Type 11 lifts what is in reach instead; types without a body end at
/// once, as the cancel helper ends them, and so do attacks whose animation
/// the level lacks.
pub fn start(c: &mut Ctx, kind: i32) {
    let a = &mut c.p.attack;
    a.serial = a.serial.wrapping_add(1);
    a.struck = false;
    a.interruptible = false;
    a.restore_anim = c.o.anim.key().map(str::to_string);
    a.kind = kind;
    a.shot_used = false;
    a.sword_shot_used = false;
    c.p.attacking = true;
    c.p.afk_time = c.t.player.idle_delay_ms;
    if let Some(m) = melee(kind) {
        if !start_melee(c, m) {
            return;
        }
    } else if let Some(s) = shot_of(kind) {
        if !start_shot(c, s) {
            return;
        }
    } else if kind == kind::LIFT {
        c.p.attacking = false;
        c.p.attack.kind = kind::NONE;
        try_lift(c);
    } else {
        cancel(c);
    }
    finish_start(c);
}

/// Sets `name` as the attack's animation; false (and the attack ends)
/// when the level lacks it, since the attack could never end.
fn set_attack_anim(c: &mut Ctx, name: &str) -> bool {
    if !c.level.animations.contains_key(name) {
        cancel(c);
        return false;
    }
    c.set_anim(name, false);
    c.p.attack.anim = Some(name.to_string());
    true
}

fn start_melee(c: &mut Ctx, m: &Melee) -> bool {
    let sword = has_sword(c);
    let anim = match m.sword_anim {
        Some(s) if sword && c.level.animations.contains_key(s) => s,
        _ => m.anim,
    };
    if !set_attack_anim(c, anim) {
        return false;
    }
    c.o.attack_rect = m.rect;
    let catnip = powerup(c) == CATNIP_POWERUP;
    c.o.damage = if catnip { m.damage * 2 } else { m.damage };
    if c.p.attack.overrides.strong_blows {
        c.o.damage = if m.kind == kind::DUCK_SWIPE {
            STRONG_DUCK_DAMAGE
        } else {
            STRONG_DAMAGE
        };
    }
    if let Some(offset) = m.offset {
        c.p.attack.offset = offset;
    }
    c.p.attack.uppercut = m.kind == kind::UPPERCUT;
    true
}

fn start_shot(c: &mut Ctx, s: &Shot) -> bool {
    let ammo = c.p.stats.ammo(s.weapon);
    let anim = if ammo > 0 { s.anim } else { s.empty_anim };
    if ammo > 0
        && let Some((line, percent)) = s.voice
        && c.rng.below(100) < percent
    {
        c.say(line, false);
    }
    if !set_attack_anim(c, anim) {
        return false;
    }
    let a = &mut c.p.attack;
    a.offset = s.offset;
    a.rewind_to = s.rewind_to;
    a.uppercut = false;
    a.post_anim = None;
    if let Some(ms) = s.throw_ms {
        a.throw_ms = ms;
        a.throw_min_ms = THROW_MIN_MS;
        a.throw_max_ms = THROW_MAX_MS;
        if let Some(post) = s.post_anim {
            a.post_anim = Some(post.to_string());
            a.struck = true;
        }
    }
    true
}

/// The part of the start that follows every type: the game-wide damage
/// and reach overrides, then the mirror for facing left.
fn finish_start(c: &mut Ctx) {
    let ov = c.p.attack.overrides;
    if ov.one_hit {
        c.o.damage = ONE_HIT_DAMAGE;
    } else if ov.no_damage {
        c.o.damage = 0;
    }
    if !c.o.attack_rect.is_set() {
        return;
    }
    if ov.long_reach {
        c.o.attack_rect.right += LONG_REACH;
    }
    if !c.p.facing_right {
        c.o.attack_rect = c.o.attack_rect.mirrored();
    }
}

/// One tick of the running attack, in place of the host's animation step.
///
/// The animation advances; when it has finished the attack ends as
/// [`cancel`] ends it. Otherwise Claw cannot hit anything this tick unless
/// the current frame has the strike event, which makes the attack box
/// live; the other events launch shots, rewind for rapid fire and charge
/// dynamite (see the module documentation). From the tick after the
/// first active frame on, the host state may leave the attack.
///
/// When another animation has replaced the attack's (a host state started
/// its own), the attack ends and that animation stays.
pub fn update(c: &mut Ctx) {
    if let Some(own) = &c.p.attack.anim
        && c.o.anim.key() != Some(own.as_str())
    {
        c.p.attack.restore_anim = None;
        cancel(c);
        return;
    }
    let event = c.advance_anim();
    if c.o.anim.is_done() {
        cancel(c);
        return;
    }
    if c.p.attack.struck {
        c.p.attack.interruptible = true;
    }
    c.o.flags |= flags::HARMLESS;
    match event {
        EVENT_STRIKE => {
            c.o.flags &= !flags::HARMLESS;
            c.p.attack.struck = true;
        }
        EVENT_RELEASE => {
            c.p.attack.struck = true;
            if !c.p.attack.shot_used {
                c.p.attack.shot_used = true;
                launch(c);
            }
        }
        EVENT_REFIRE => refire(c),
        EVENT_CHARGE => charge(c),
        EVENT_SWORD_SHOT => {
            c.p.attack.struck = true;
            if !c.p.attack.sword_shot_used {
                c.p.attack.sword_shot_used = true;
                sword_shot(c);
            }
        }
        _ => {}
    }
}

/// Rapid fire: pressing fire again (or the selected weapon's key) while
/// the pistol has shots, or with magic, readies another shot and rewinds
/// the animation to the element before it.
fn refire(c: &mut Ctx) {
    let p = c.input.pressed;
    let weapon = c.p.stats.weapon;
    let asked = p.contains(Buttons::PROJECTILE)
        || (weapon == Weapon::Pistol && p.contains(Buttons::PISTOL))
        || (weapon == Weapon::Magic && p.contains(Buttons::MAGIC));
    if !asked || (weapon == Weapon::Pistol && c.p.stats.pistol <= 0) {
        return;
    }
    c.p.attack.shot_used = false;
    c.p.attack.serial = c.p.attack.serial.wrapping_add(1);
    let to = c.p.attack.rewind_to;
    rewind(c, to);
}

/// Puts the running animation on element `element` as if it had just
/// been reached: its image shows and its full duration is still to wait.
/// No sound plays.
fn rewind(c: &mut Ctx, element: usize) {
    let Some(anim) = c.o.anim.animation().cloned() else {
        return;
    };
    let n = anim.frames.len();
    if element >= n {
        return;
    }
    let frames = c.level.image_frames(&c.o.image_set);
    let mirrored = c.o.draw_flags & draw_flags::MIRROR != 0;
    let mut player = AnimPlayer::default();
    player.set(anim);
    let mut frame = c.o.frame;
    for _ in 0..=4 * n {
        let e = player.advance_in(u32::MAX, frame, mirrored, frames);
        if let Some(f) = e.frame {
            frame = f;
        }
        if e.applied == Some(element) {
            c.o.anim = player;
            c.o.frame = frame;
            return;
        }
    }
}

/// Dynamite charge: while the fire or dynamite key is held the charge
/// grows by the tick; at its longest, or once the key is let go (raised
/// to the shortest charge), the releasing animation starts.
fn charge(c: &mut Ctx) {
    let held = c.input.held;
    let a = &mut c.p.attack;
    if held.contains(Buttons::PROJECTILE) || held.contains(Buttons::DYNAMITE) {
        a.throw_ms += DT;
        if a.throw_ms < a.throw_max_ms {
            return;
        }
        a.throw_ms = a.throw_max_ms;
    } else {
        a.throw_ms = a.throw_ms.max(a.throw_min_ms);
    }
    match a.post_anim.clone() {
        Some(post) => {
            set_attack_anim(c, &post);
        }
        None => cancel(c),
    }
}

/// The power of a dynamite throw from the charge: 0 for the shortest
/// charge up to [`THROW_MAX_POWER`] for the longest, negated facing left.
pub fn throw_power(charge_ms: i32, min_ms: i32, max_ms: i32, facing_right: bool) -> i32 {
    let range = match max_ms.wrapping_sub(min_ms) {
        0 => 1,
        r => r,
    };
    let p = charge_ms
        .wrapping_sub(min_ms)
        .wrapping_mul(THROW_MAX_POWER)
        .wrapping_div(range)
        .clamp(0, THROW_MAX_POWER);
    if facing_right { p } else { -p }
}

/// Where Claw's shots leave: his position plus the shot offset, mirrored
/// when he faces left.
fn shot_origin(c: &Ctx) -> (i32, i32) {
    let (ox, oy) = c.p.attack.offset;
    let x = if c.p.facing_right {
        c.o.x + ox
    } else {
        c.o.x - ox
    };
    (x, c.o.y + oy)
}

/// Launches `spec`; mirrors the shot's picture when Claw faces left and
/// `mirror` asks for it. A shot that is `tagged` (pistol and magic shots,
/// as in the original) carries Claw's attack count. Returns the shot.
fn spawn_shot(c: &mut Ctx, spec: ProjectileSpec, mirror: bool, tagged: bool) -> Option<ObjectId> {
    let id = if tagged {
        let serial = c.p.attack.serial;
        c.with_world(|w| logic::projectile::spawn_tagged(w, spec, serial))?
    } else {
        c.with_world(|w| logic::projectile::spawn(w, spec))?
    };
    if mirror
        && !c.p.facing_right
        && let Some(o) = c.objects.get_mut(id)
    {
        o.draw_flags |= draw_flags::MIRROR;
    }
    Some(id)
}

/// Launches the selected weapon's shot and spends its ammunition (pistol
/// and magic only while there is some; dynamite always).
fn launch(c: &mut Ctx) {
    let (x, y) = shot_origin(c);
    let dir = if c.p.facing_right { 1 } else { -1 };
    let base = ProjectileSpec {
        x,
        y,
        z: c.o.z + 1,
        hit_type_flags: types::ENEMY,
        owner: Some(c.id),
        ..ProjectileSpec::default()
    };
    let weapon = c.p.stats.weapon;
    let spec = match weapon {
        Weapon::Pistol => ProjectileSpec {
            logic: "PistolBullet",
            z: c.o.z - 1,
            vx: SHOT_SPEED * dir,
            damage: PISTOL_DAMAGE,
            type_flags: types::PISTOL_SHOT,
            image_set: PISTOL_SHOT_IMAGE,
            ..base
        },
        Weapon::Magic => ProjectileSpec {
            logic: "MagicBullet",
            vx: SHOT_SPEED * dir,
            damage: MAGIC_DAMAGE,
            type_flags: MAGIC_SHOT_TYPE,
            special_flags: MAGIC_SHOT_FLAG,
            ..base
        },
        Weapon::Dynamite => {
            let a = &c.p.attack;
            ProjectileSpec {
                logic: "Dynamite",
                damage: DYNAMITE_DAMAGE,
                type_flags: types::PISTOL_SHOT,
                power: throw_power(a.throw_ms, a.throw_min_ms, a.throw_max_ms, c.p.facing_right),
                ..base
            }
        }
    };
    let tagged = weapon != Weapon::Dynamite;
    if spawn_shot(c, spec, weapon == Weapon::Magic, tagged).is_none() {
        return;
    }
    let s = &mut c.p.stats;
    match weapon {
        Weapon::Pistol if s.pistol > 0 => s.pistol -= 1,
        Weapon::Magic if s.magic > 0 => s.magic -= 1,
        Weapon::Dynamite => s.dynamite -= 1,
        _ => {}
    }
}

/// The sword powerup's shot: fire, lightning or ice, with its sound. Other
/// powerups send nothing.
fn sword_shot(c: &mut Ctx) {
    let p = powerup(c);
    let Some(&(_, logic, flag, sound)) = SWORD_SHOTS.iter().find(|s| s.0 == p) else {
        return;
    };
    let (x, y) = shot_origin(c);
    let dir = if c.p.facing_right { 1 } else { -1 };
    let spec = ProjectileSpec {
        logic,
        x,
        y,
        z: c.o.z + 1,
        vx: SHOT_SPEED * dir,
        damage: MAGIC_DAMAGE,
        type_flags: MAGIC_SHOT_TYPE,
        hit_type_flags: types::ENEMY,
        special_flags: MAGIC_SHOT_FLAG | flag,
        owner: Some(c.id),
        ..ProjectileSpec::default()
    };
    // Unlike the pistol and magic shots, a sword shot does not carry
    // Claw's attack count.
    if spawn_shot(c, spec, true, false).is_some() {
        c.play(sound, SoundChannel::Effect);
    }
}

/// Ends the running attack, as every state change does: a carried object
/// is let go, the attack type is cleared and the host may act again; when
/// an attack was running, its saved animation comes back (not advanced)
/// and Claw can no longer hit anything. Claw's special states read the
/// end of their own entry from here too (see [`special::SpecialData`]).
///
/// The jump's handover to the fall at its top does not come here while an
/// air attack runs (see [`carry_over_jump_top`]).
pub fn cancel(c: &mut Ctx) {
    if let Some(held) = c.p.special.lifted.take() {
        c.with_world(|w| logic::dropped(w, held));
    }
    special::forget_entries(c);
    c.p.attack.kind = kind::NONE;
    c.p.attack.interruptible = true;
    c.p.attack.anim = None;
    if c.p.attacking {
        c.p.attacking = false;
        if let Some(key) = c.p.attack.restore_anim.take() {
            c.set_anim(&key, false);
        }
        c.o.flags |= flags::HARMLESS;
    }
}

/// Keeps an air attack (the jumping swipe, shot, magic or dynamite throw)
/// going when the jump turns into a fall at its top; true when one does.
/// The fall's own animation is then the one to come back to when the
/// attack ends. Only the jump's handover to the fall asks this; every
/// other change of state ends the attack through [`cancel`].
pub fn carry_over_jump_top(c: &mut Ctx) -> bool {
    let at_top = air_attack_running(c)
        && c.o.state == state::JUMP
        && c.o.physics_type == ptype::JUMP
        && c.o.y <= c.p.jump_peak_y;
    if at_top {
        c.p.attack.restore_anim = Some("CLAW_FALL".to_string());
    }
    at_top
}

/// True while an air attack runs: the jumping swipe, shot, magic or
/// dynamite throw, which carry on when a jump turns into a fall.
pub fn air_attack_running(c: &Ctx) -> bool {
    c.p.attacking
        && matches!(
            c.p.attack.kind,
            kind::JUMP_SWIPE | kind::JUMP_PISTOL | kind::JUMP_MAGIC | kind::JUMP_DYNAMITE
        )
}

/// Called when the stand animation reports [`STAND_FRAME_EVENT`]. No
/// animation of the game uses it yet.
pub fn frame_event(c: &mut Ctx, event: i32) {
    let _ = (c, event);
}
