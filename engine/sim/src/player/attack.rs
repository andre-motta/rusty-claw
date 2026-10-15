//! Claw's attacks: choosing one from the pressed buttons, running it on its
//! animation, and cancelling it.
//!
//! The host states call [`select`] once per tick in the game's order: Jump,
//! Fall and Crouch before their own update, Walk after its movement (an
//! attack started there stops the walk), and Stand before its update. While
//! an attack runs, the hosts call [`update`] in place of their plain
//! animation step.
//!
//! Choosing an attack only picks an attack type, starts its animation and
//! sets the attack box and the damage. From then on the animation drives
//! everything: each tick [`update`] advances it, and the event value of the
//! current animation frame says what happens. On a frame with event 1 the
//! attack box is live for that tick's collision pass; on every other tick
//! Claw cannot hit anything. The animation player reports every frame for
//! at least one tick, so even a 1 ms frame is seen.
//!
//! Melee attacks are filled in:
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
//! A sword powerup plays the powerup variant of the swipes, and the catnip
//! powerup doubles the damage. The sounds of the swings are those of the
//! animation frames.
//!
//! Standing or walking, the attack button picks a swipe, a punch, an
//! uppercut or a kick (see [`melee_choice`]); in the air it starts the
//! jumping swipe and crouching the crouching swipe. The pistol, magic and
//! dynamite, lifting and the sword powerup shots are not filled in yet: the
//! projectile button does nothing, and the weapon keys only select the
//! weapon.

use claw_frame::{Buttons, Weapon};

use super::{Ctx, state};
use crate::logic;
use crate::object::{ObjectId, Rect, flags, types};

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
    /// Crouching swipe.
    pub const DUCK_SWIPE: i32 = 7;
    /// Jumping swipe.
    pub const JUMP_SWIPE: i32 = 8;
    /// Jumping pistol shot.
    pub const JUMP_PISTOL: i32 = 9;
    /// Jumping magic.
    pub const JUMP_MAGIC: i32 = 10;
    /// Kick.
    pub const KICK: i32 = 13;
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
    /// Number of attacks started so far. An enemy takes at most one hit
    /// per attack: it remembers the number of the attack that hit it.
    pub serial: u32,
    /// The active frame has been reached.
    pub struck: bool,
    /// The attack is the uppercut: enemies show their high hit reaction.
    pub uppercut: bool,
    /// Where a projectile leaves, relative to Claw facing right.
    pub offset: (i32, i32),
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

/// Starts an attack when a button asks for one; true when one started.
///
/// The buttons pressed this tick are tested in the game's order, and the
/// first one pressed decides: Attack, Projectile, the Pistol, Magic and
/// Dynamite keys, then Lift. Nothing happens while an attack runs.
pub fn select(c: &mut Ctx) -> bool {
    if c.p.attacking {
        return false;
    }
    let pressed = c.input.pressed;
    if pressed.contains(Buttons::ATTACK) {
        match c.o.state {
            state::FALL | state::JUMP => start(c, kind::JUMP_SWIPE),
            state::CROUCH => start(c, kind::DUCK_SWIPE),
            _ if has_sword(c) => start(c, kind::SWIPE),
            _ => melee_choice(c),
        }
    } else if pressed.contains(Buttons::PROJECTILE) {
        // Shots are not filled in yet.
    } else if pressed.contains(Buttons::PISTOL) {
        c.p.stats.weapon = Weapon::Pistol;
    } else if pressed.contains(Buttons::MAGIC) {
        c.p.stats.weapon = Weapon::Magic;
    } else if pressed.contains(Buttons::DYNAMITE) {
        c.p.stats.weapon = Weapon::Dynamite;
    }
    c.p.attacking
}

/// The nearest enemy or curse in front of Claw and the gap to it in px:
/// the distance from Claw's hit box to the near edge of the target's,
/// counted `TARGET_MARGIN` px closer for a target that is not
/// [`PUNCHABLE`]. Objects [`TARGET_HEIGHT`] px or more above or below are
/// skipped, and on equal distances the one earlier in the update order
/// wins. The gap is negative when the target overlaps Claw's front, and
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
        // A lift request; lifting is not filled in yet.
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

/// Starts attack type `kind`: counts the attack, saves the animation to
/// come back to and sets the type's animation (not advanced yet), attack
/// box, damage and shot offset. Facing left, the attack box is mirrored.
/// Types without a body here, and attacks whose animation the level lacks,
/// end at once, as the cancel helper ends them.
pub fn start(c: &mut Ctx, kind: i32) {
    let a = &mut c.p.attack;
    a.serial = a.serial.wrapping_add(1);
    a.struck = false;
    a.interruptible = false;
    a.restore_anim = c.o.anim.key().map(str::to_string);
    a.kind = kind;
    c.p.attacking = true;
    c.p.afk_time = c.t.player.idle_delay_ms;
    let Some(m) = melee(kind) else {
        cancel(c);
        return;
    };
    let sword = has_sword(c);
    let anim = match m.sword_anim {
        Some(s) if sword && c.level.animations.contains_key(s) => s,
        _ => m.anim,
    };
    if !c.level.animations.contains_key(anim) {
        // Without its animation the attack could never end.
        cancel(c);
        return;
    }
    c.set_anim(anim, false);
    c.o.attack_rect = m.rect;
    let catnip = powerup(c) == CATNIP_POWERUP;
    c.o.damage = if catnip { m.damage * 2 } else { m.damage };
    if let Some(offset) = m.offset {
        c.p.attack.offset = offset;
    }
    c.p.attack.uppercut = kind == kind::UPPERCUT;
    if !c.p.facing_right {
        c.o.attack_rect = c.o.attack_rect.mirrored();
    }
}

/// One tick of the running attack, in place of the host's animation step.
///
/// The animation advances; when it has finished the attack ends as
/// [`cancel`] ends it. Otherwise Claw cannot hit anything this tick unless
/// the current frame has the strike event, which makes the attack box
/// live. From the tick after the first active frame on, the host state may
/// leave the attack.
pub fn update(c: &mut Ctx) {
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
        // Shots are not filled in yet; their frames still count as the
        // active part.
        EVENT_RELEASE | EVENT_SWORD_SHOT => c.p.attack.struck = true,
        _ => {}
    }
}

/// Ends the running attack, as every state change does: a carried object
/// is let go, the attack type is cleared and the host may act again; when
/// an attack was running, its saved animation comes back (not advanced)
/// and Claw can no longer hit anything.
pub fn cancel(c: &mut Ctx) {
    if let Some(held) = c.p.special.lifted.take() {
        c.with_world(|w| logic::dropped(w, held));
    }
    c.p.attack.kind = kind::NONE;
    c.p.attack.interruptible = true;
    if c.p.attacking {
        c.p.attacking = false;
        if let Some(key) = c.p.attack.restore_anim.take() {
            c.set_anim(&key, false);
        }
        c.o.flags |= flags::HARMLESS;
    }
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
