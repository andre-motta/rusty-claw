//! Crates: single crates and stacks of crates that break open and release
//! what they hold.
//!
//! A single crate (`FrontCrate`, `BehindCrate`) shows the first image of
//! its set and is hit within the box of that image. It breaks on one
//! accepted hit, or two when the level gives it more than 1 health (with
//! 500 ms between counted hits, the first showing the next image of the
//! set); with 999 health or more it cannot be broken. Claw's attacks,
//! thrown objects and every kind of shot or blast count. Breaking
//! plays the break animation, releases the crate's powerup code 24 px above
//! its position as a bouncing goodie (a crate holding nothing releases a
//! coin), plays one of two break sounds at random and removes the crate
//! when the animation ends.
//!
//! A stack (`BackStackedCrates`, `FrontStackedCrates`) is a hidden spawner
//! that, when it first runs, builds a column of `Crate` objects 40 px apart,
//! one per non-zero code among its loot fields, and then removes itself.
//! A crate of a stack shares the enemies' base: each of its logic calls
//! starts with their per-tick roll ([`enemy::pulse_roll`]), a draw from
//! the shared random stream on every third tick.
//!
//! Each crate of a stack breaks on any accepted hit and releases its one
//! item with a random hop; the crates above it then fall 40 px under
//! gravity onto the one below, with one small bounce when they land fast.
//!
//! Statues ([`super::statue`]) are single crates with a different drop
//! and no removal; they share the hit rule, the break sound and the break
//! animation from here.

use claw_frame::SoundChannel;

use super::{ObjectData, World, enemy, new_object, pickup, sound};
use crate::level::runtime_image_set;
use crate::object::{Object, ObjectId, Rect, draw_flags, engine_state, flags, types};
use crate::tuning::TICK_MS;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[
    BACK_STACK,
    "BehindCrate",
    STACKED_CRATE,
    FRONT_CRATE,
    FRONT_STACK,
];

const FRONT_CRATE: &str = "FrontCrate";
const FRONT_STACK: &str = "FrontStackedCrates";
const BACK_STACK: &str = "BackStackedCrates";
/// Logic name of one crate of a stack.
pub const STACKED_CRATE: &str = "Crate";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[CRATE_IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[BREAK_ANIM];

/// Picture of a crate in a stack.
pub const CRATE_IMAGE_SET: &str = "LEVEL_CRATES";
/// Animation of a breaking crate.
pub const BREAK_ANIM: &str = "LEVEL_CRATEBREAK";

/// What kind of object a crate is.
pub const CRATE_TYPE: u32 = 0x200;
/// A magic shot.
pub const MAGIC_SHOT: u32 = 0x200_0000;
/// What can hit a crate: Claw, thrown objects, and pistol, magic, enemy
/// and level shots (blasts among them).
pub const CRATE_HIT_BY: u32 = types::PLAYER
    | types::THROWN
    | types::PISTOL_SHOT
    | types::ENEMY_SHOT
    | types::LEVEL_SHOT
    | MAGIC_SHOT;

/// Box of a single crate that attacks hit, relative to its position, when
/// the level does not know its picture's frame.
pub const CRATE_BOX: Rect = Rect::new(-23, -28, 32, 23);
/// Box of a crate in a stack that attacks hit.
pub const STACKED_BOX: Rect = Rect::new(-20, -20, 20, 20);
/// Spacing of the crates in a stack, px.
pub const STACK_STEP: i32 = 40;
/// Time after a counted hit on a two-hit crate in which further hits do
/// nothing, ms.
pub const HIT_COOLDOWN_MS: i32 = 500;
/// A single crate releases its item this far above its position, px.
pub const DROP_RISE: i32 = 24;
/// Health at or above which a crate cannot be broken.
pub const UNBREAKABLE: i32 = 999;
/// Gravity on falling crates of a stack, px/s/s.
pub const STACK_GRAVITY: f64 = 2000.0;
/// A falling crate landing faster than this bounces back at half speed,
/// px/s.
pub const STACK_BOUNCE_SPEED: f64 = 200.0;

/// Logic states of a crate.
pub mod state {
    /// First call.
    pub const INIT: i32 = 0;
    /// Waiting to be broken (a single crate).
    pub const IDLE: i32 = 5;
    /// Breaking (a single crate).
    pub const BREAKING: i32 = 31;
    /// Broken: removed on this call.
    pub const BROKEN: i32 = 10;
    /// Resting on, or falling onto, the crate below (a crate of a stack).
    pub const STACKED: i32 = 40;
    /// Breaking (a crate of a stack).
    pub const STACK_BREAKING: i32 = 41;
}

/// A crate's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrateData {
    /// The powerup code it releases.
    pub content: i32,
    /// Time left in which hits do nothing, ms.
    pub cooldown_ms: i32,
    /// In a stack: the crate below.
    pub below: Option<ObjectId>,
    /// In a stack: the crate above.
    pub above: Option<ObjectId>,
    /// In a stack: position from the bottom when it was built.
    pub index: i32,
    /// In a stack: the height it rests at.
    pub floor: i32,
    /// In a stack: the height of the stack's bottom.
    pub base: i32,
    /// In a stack: falling speed, px/s (down is positive).
    pub vy: f64,
    /// In a stack: sub-pixel fall carried to the next tick.
    pub rest: f64,
}

fn data_mut(o: &mut Object) -> &mut CrateData {
    if !matches!(o.data, ObjectData::Crate(_)) {
        o.data = ObjectData::Crate(CrateData::default());
    }
    match &mut o.data {
        ObjectData::Crate(d) => d,
        _ => unreachable!("set above"),
    }
}

fn data_of(o: &Object) -> Option<&CrateData> {
    match &o.data {
        ObjectData::Crate(d) => Some(d),
        _ => None,
    }
}

/// The codes a stack record holds, bottom crate first: the powerup field
/// (or, when it is 0, the first user value), then each non-zero value of
/// the first user rectangle (left, top, right, bottom) and the second
/// (left, top, right, bottom). The bottom crate is always built, even when
/// it holds nothing.
pub fn stack_contents(record: &[i32]) -> Vec<i32> {
    let field = |i: usize| record.get(i).copied().unwrap_or(0);
    let powerup = field(15);
    let first = field(35);
    let mut out = vec![if powerup != 0 { powerup } else { first }];
    if powerup != 0 && first != 0 {
        out.push(first);
    }
    for i in [36, 37, 38, 39, 40, 41, 42] {
        if field(i) != 0 {
            out.push(field(i));
        }
    }
    out
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let logic = o.logic.clone();
    match &*logic {
        FRONT_STACK | BACK_STACK => build_stack(w, id),
        STACKED_CRATE => stacked_tick(w, id),
        _ => single_tick(w, id),
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    let logic = o.logic.clone();
    match &*logic {
        FRONT_STACK | BACK_STACK => {}
        STACKED_CRATE => break_stacked(w, id, attacker),
        _ => hit_single(w, id, attacker),
    }
}

/// `id` attacked `victim`. Crates attack nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// `carrier` lifted `id`. Crates cannot be lifted.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    let _ = (w, id, carrier);
}

/// `id` was thrown at (vx, vy) px/s. Crates cannot be lifted.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    let _ = (w, id, vx, vy);
}

/// `id` was put down or let go. Crates cannot be lifted.
pub fn dropped(w: &mut World, id: ObjectId) {
    let _ = (w, id);
}

/// Removes a pistol shot that hit a crate (or a statue).
pub(crate) fn absorb_shot(w: &mut World, attacker: ObjectId) {
    if w.objects
        .get(attacker)
        .is_some_and(|a| a.type_flags == types::PISTOL_SHOT)
    {
        w.destroy(attacker);
    }
}

/// Plays one of the two break sounds, chosen at random (a crate's or a
/// statue's). Draws: one of 0..100.
pub(crate) fn break_sound(w: &mut World, id: ObjectId) {
    let key = if w.rng.below(100) < 50 {
        "GAME_CRATEBREAK"
    } else {
        "GAME_CRATEBREAK2"
    };
    sound::play_from(w, id, key, SoundChannel::Effect);
}

// ------------------------------------------------------------ single crates

/// The frame a crate showing `frame` shows after a counted hit: the next
/// image of its set, wrapping to the first after the last (`frames`, the
/// set's first and last frames, when known).
pub fn next_frame(frame: i32, frames: Option<(i32, i32)>) -> i32 {
    match frames {
        Some((first, last)) if frame >= last || frame < first => first,
        _ => frame.saturating_add(1),
    }
}

fn single_tick(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).expect("caller checked");
    let (frames, st) = (w.image_frames(&o.image_set), o.state);
    let first = frames.map_or(o.frame, |(first, _)| first);
    let picture = (st == state::INIT)
        .then(|| w.frame_box(&o.image_set, first))
        .flatten();
    let o = w.objects.get_mut(id).expect("caller checked");
    match o.state {
        state::INIT => {
            o.frame = first;
            o.type_flags = CRATE_TYPE;
            o.hit_type_flags = CRATE_HIT_BY;
            o.hit_handler = true;
            o.hit_rect = picture.unwrap_or(CRATE_BOX);
            if o.z == 0 {
                o.z = if &*o.logic == FRONT_CRATE { 5000 } else { 1100 };
                o.flags |= flags::RESORT;
            }
            if o.health >= UNBREAKABLE {
                o.flags |= flags::NO_COLLIDE | flags::SAFE;
            } else {
                o.health = if o.health > 1 { 2 } else { 1 };
            }
            let content = o.record.as_ref().map_or(0, |r| r[15]);
            let d = data_mut(o);
            d.content = content;
            d.cooldown_ms = 0;
            o.state = state::IDLE;
        }
        state::IDLE => {
            let d = data_mut(o);
            d.cooldown_ms = (d.cooldown_ms - TICK_MS as i32).max(0);
            w.advance_anim(id);
        }
        state::BREAKING => {
            w.advance_anim(id);
            let o = w.objects.get_mut(id).expect("checked");
            if o.anim.is_done() {
                o.state = state::BROKEN;
            }
        }
        state::BROKEN => {
            o.state = state::IDLE;
            o.flags |= flags::DESTROY;
        }
        _ => {}
    }
}

fn hit_single(w: &mut World, id: ObjectId, attacker: ObjectId) {
    absorb_shot(w, attacker);
    let frames = w.objects.get(id).and_then(|o| w.image_frames(&o.image_set));
    let o = w.objects.get_mut(id).expect("caller checked");
    if o.state != state::IDLE || data_mut(o).cooldown_ms > 0 {
        return;
    }
    o.health -= 1;
    if o.health > 0 {
        o.frame = next_frame(o.frame, frames);
        data_mut(o).cooldown_ms = HIT_COOLDOWN_MS;
        sound::play_from(w, id, "GAME_CRATEDAMAGE", SoundChannel::Effect);
        return;
    }
    o.flags |= flags::SAFE;
    o.state = state::BREAKING;
    let (x, y, z, content) = (o.x, o.y, o.z, data_mut(o).content);
    w.set_anim(id, BREAK_ANIM);
    if let Some(item) = pickup::spawn_drop(w, x, y - DROP_RISE, content)
        && let Some(g) = w.objects.get_mut(item)
    {
        g.z = z - 1;
    }
    break_sound(w, id);
}

// ------------------------------------------------------------ stacks

fn build_stack(w: &mut World, id: ObjectId) {
    let o = w.objects.get_mut(id).expect("caller checked");
    if o.state != state::INIT {
        return;
    }
    if o.z == 0 {
        o.z = if &*o.logic == FRONT_STACK { 5000 } else { 1000 };
    }
    o.flags |= flags::NO_COLLIDE | flags::DESTROY;
    o.draw_flags |= draw_flags::HIDDEN;
    o.state = state::IDLE;
    let (x, y, z, frame) = (o.x, o.y, o.z, o.frame);
    let (sx, sy) = (o.speed_x, o.speed_y);
    let contents = stack_contents(o.record.as_deref().map_or(&[][..], |r| &r[..]));
    let mut ids = Vec::with_capacity(contents.len());
    for (i, &content) in contents.iter().enumerate() {
        let i = i as i32;
        let mut c = new_object(STACKED_CRATE, x, y - STACK_STEP * i, z + i + 1);
        c.image_set = runtime_image_set(CRATE_IMAGE_SET);
        c.frame = frame;
        c.type_flags = CRATE_TYPE;
        c.hit_type_flags = CRATE_HIT_BY;
        c.hit_handler = true;
        c.hit_rect = STACKED_BOX;
        c.health = 1;
        c.flags |= flags::NOTIFY_DESTROY;
        if content == pickup::WARP_CODE {
            // A warp keeps the stack's destination.
            c.speed_x = sx;
            c.speed_y = sy;
        }
        c.state = state::STACKED;
        c.data = ObjectData::Crate(CrateData {
            content,
            index: i,
            floor: y - STACK_STEP * i,
            base: y,
            ..CrateData::default()
        });
        ids.push(w.spawn(c));
    }
    for (i, &c) in ids.iter().enumerate() {
        let below = i.checked_sub(1).map(|j| ids[j]);
        let above = ids.get(i + 1).copied();
        if let Some(o) = w.objects.get_mut(c) {
            let d = data_mut(o);
            d.below = below;
            d.above = above;
        }
    }
}

fn stacked_tick(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).expect("caller checked");
    if !matches!(
        o.state,
        state::INIT | engine_state::DESTROYING | engine_state::SLEEPING
    ) {
        // A crate of a stack shares the enemies' base: its calls start
        // with their per-tick roll.
        enemy::pulse_roll(w);
    }
    let o = w.objects.get_mut(id).expect("caller checked");
    match o.state {
        state::INIT => {
            // A crate placed on its own rather than built by a stack.
            o.type_flags = CRATE_TYPE;
            o.hit_type_flags = CRATE_HIT_BY;
            o.hit_handler = true;
            o.hit_rect = STACKED_BOX;
            o.health = 1;
            o.flags |= flags::NOTIFY_DESTROY;
            let content = o.record.as_ref().map_or(0, |r| r[15]);
            let y = o.y;
            let d = data_mut(o);
            d.content = content;
            d.floor = y;
            d.base = y;
            o.state = state::STACKED;
        }
        state::STACKED => fall(w, id),
        state::STACK_BREAKING => {
            w.advance_anim(id);
            let o = w.objects.get_mut(id).expect("checked");
            if o.anim.is_done() {
                o.flags |= flags::DESTROY;
            }
        }
        _ => {}
    }
}

/// A crate of a stack settles on the crate below it (or the stack's
/// bottom), and tells the crate above where to rest.
fn fall(w: &mut World, id: ObjectId) {
    let below_alive = |w: &World, b: Option<ObjectId>| {
        b.and_then(|b| w.objects.get(b)).and_then(data_of).is_some()
    };
    let o = w.objects.get(id).expect("caller checked");
    let d = data_of(o).cloned().unwrap_or_default();
    let has_below = below_alive(w, d.below);
    let o = w.objects.get_mut(id).expect("checked");
    o.flags |= flags::ALWAYS_ACTIVE;
    let mut d = d;
    if !has_below {
        d.below = None;
        d.floor = d.base;
    }
    let dt = f64::from(TICK_MS) / 1000.0;
    if o.y < d.floor || d.vy != 0.0 {
        d.vy += STACK_GRAVITY * dt;
        let fy = d.vy * dt + d.rest;
        let dy = fy.trunc();
        d.rest = fy - dy;
        o.y += dy as i32;
    }
    if o.y >= d.floor {
        o.y = d.floor;
        d.rest = 0.0;
        if d.vy > STACK_BOUNCE_SPEED {
            d.vy = -d.vy / 2.0;
        } else {
            d.vy = 0.0;
            o.flags &= !flags::ALWAYS_ACTIVE;
        }
    }
    let top = o.y - STACK_STEP;
    let above = d.above;
    *data_mut(o) = d;
    if let Some(a) = above.and_then(|a| w.objects.get_mut(a))
        && matches!(a.data, ObjectData::Crate(_))
    {
        data_mut(a).floor = top;
    }
}

fn break_stacked(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let o = w.objects.get_mut(id).expect("caller checked");
    if o.state != state::STACKED {
        return;
    }
    o.flags |= flags::SAFE | flags::ALWAYS_ACTIVE;
    o.state = state::STACK_BREAKING;
    let (x, y, sx, sy) = (o.x, o.y, o.speed_x, o.speed_y);
    let d = data_mut(o);
    let (below, above, content) = (d.below.take(), d.above.take(), d.content);
    // The crates above and below now rest on each other.
    if let Some(a) = above.and_then(|a| w.objects.get_mut(a)) {
        data_mut(a).below = below;
    }
    if let Some(b) = below.and_then(|b| w.objects.get_mut(b)) {
        data_mut(b).above = above;
    }
    w.set_anim(id, BREAK_ANIM);
    absorb_shot(w, attacker);
    if content != 0 {
        sound::play_from(w, id, "GAME_PURELEASE1", SoundChannel::Effect);
    }
    let mut loot = [0; 9];
    loot[8] = content;
    for item in pickup::drop_loot(w, x, y, loot) {
        if content == pickup::WARP_CODE
            && let Some(g) = w.objects.get_mut(item)
        {
            g.speed_x = sx;
            g.speed_y = sy;
        }
    }
    break_sound(w, id);
}
