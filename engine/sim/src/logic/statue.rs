//! Statues: breakable stone figures in the scenery, the statue branch of
//! the crates ([`super::crates`]).
//!
//! A statue breaks like a single crate (one hit at health 1, a damaged
//! picture and a 500 ms pause at health 2, never at 999 or more), with
//! three differences: its treasure comes out as a fan of bouncing goodies,
//! one per non-empty loot field in record order, from `-12.5 n` px/s
//! sideways in steps of 25 and each with its own random upward speed; that
//! fan is released at the statue's depth minus one (`FrontStatue`) or
//! plus one (`BehindStatue`, [`drop_z_offset`]), 24 px above it; and a
//! broken statue is never removed but stays as rubble, the last picture of
//! the break, for the rest of the attempt.
//!
//! The two names differ only in depth: the level places a front statue in
//! front of Claw and the enemies and a behind statue behind them. Neither
//! blocks or carries anything.
//!
//! # Calls
//!
//! - Set-up ([`state::SET_UP`]): the first picture of its set, hit within
//!   that picture's box by Claw, his thrown objects and every kind of shot
//!   or blast; depth 1100 when placed at 0; health 1, or 2 when the record
//!   gives more than 1, or unbreakable (out of pair collision) at 999 or
//!   more.
//! - Idle ([`state::IDLE`]): the 500 ms pause after a counted hit runs
//!   down.
//! - A hit while idle and not pausing (a pistol shot is used up): one
//!   health less; with health left, the damage sound, the next picture and
//!   the pause; at none, the break animation starts, the statue is safe
//!   from further hits for good, the fan of treasure flies out
//!   ([`drop_fan`]) and one of the two break sounds plays.
//! - Breaking ([`state::BREAKING`]): the break animation advances one tick
//!   per call (63 calls on the fifth level: the second picture on the
//!   first, then the next every 7 calls up to the tenth on call 57), then
//!   the statue is broken ([`state::BROKEN`]) and idle again on the next
//!   call, showing the rubble.
//!
//! Random draws on a break, in order: one of 0..301 per goodie of the fan
//! (its upward speed), then one of 0..100 (the break sound).
//!
//! # Defaults taken
//!
//! - The hit box is the first picture's box, unmirrored, while the picture
//!   is drawn mirrored as the record asks; on the fifth level's statues
//!   that leaves the hittable box 6 px to the right of the art, as in the
//!   original game.

use super::crates::{self, CRATE_HIT_BY, CRATE_TYPE, HIT_COOLDOWN_MS, UNBREAKABLE};
use super::{ObjectData, World, pickup, sound};
use crate::object::{Object, ObjectId, Rect, flags};
use crate::tuning::TICK_MS;
use claw_frame::SoundChannel;

/// Logic names this family runs.
pub const NAMES: &[&str] = &[FRONT, BEHIND];

/// Image sets this family may switch an object to: none (a statue keeps
/// its record's set).
pub const IMAGE_SETS: &[&str] = &[];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &[BREAK_ANIM];

/// Whether this family runs its logics yet (see [`super::built`]).
pub const BUILT: bool = true;

/// The statue drawn in front of Claw.
pub const FRONT: &str = "FrontStatue";
/// The statue drawn behind Claw.
pub const BEHIND: &str = "BehindStatue";
/// Animation of a breaking statue, the crates' break.
pub const BREAK_ANIM: &str = super::crates::BREAK_ANIM;
/// Depth given to a statue placed at depth 0 (either name).
pub const DEFAULT_Z: i32 = 1100;
/// Vertical offset of the point the treasure is released from, px: 24 px
/// above the statue's position.
pub const DROP_Y: i32 = -24;
/// Sideways speed step between two goodies of the fan, px/s.
pub const FAN_STEP: i32 = 25;
/// Slowest upward speed of a goodie of the fan, px/s; the fastest is
/// 300 px/s more.
pub const FAN_RISE: i32 = 200;
/// The damage sound of a statue with health left.
pub const DAMAGE_SOUND: &str = "GAME_CRATEDAMAGE";
/// Hit box when the level does not know the statue's first picture: the
/// box of the fifth level's statue.
pub const FALLBACK_BOX: Rect = Rect::new(-41, -83, 46, 82);

/// Logic states of a statue (the crates' numbers).
pub mod state {
    /// First call: set-up.
    pub const SET_UP: i32 = super::crates::state::INIT;
    /// Standing, whole or broken.
    pub const IDLE: i32 = super::crates::state::IDLE;
    /// Breaking.
    pub const BREAKING: i32 = super::crates::state::BREAKING;
    /// Broken: idle again on this call.
    pub const BROKEN: i32 = super::crates::state::BROKEN;
}

/// The depth offset of the treasure of the statue running `name`.
pub fn drop_z_offset(name: &str) -> Option<i32> {
    match name {
        FRONT => Some(-1),
        BEHIND => Some(1),
        _ => None,
    }
}

/// The sideways speed of the first goodie of a fan of `n`, px/s.
pub fn fan_start(n: i32) -> i32 {
    -FAN_STEP * n / 2
}

/// The loot codes of a statue's record, in the order the fan releases
/// them: the powerup field, then the first user box's left, right, top
/// and bottom, then the second's.
pub fn loot_order(record: &[i32]) -> [i32; 9] {
    let f = |i: usize| record.get(i).copied().unwrap_or(0);
    [
        f(15),
        f(35),
        f(37),
        f(36),
        f(38),
        f(39),
        f(41),
        f(40),
        f(42),
    ]
}

/// True when a fan releases a goodie for `code` on level `level`: a
/// treasure, powerup or item code with a picture.
pub fn releases(code: i32, level: u32) -> bool {
    (1..pickup::POWERUP_CODES as i32).contains(&code)
        && pickup::image_set_in(code, level).is_some_and(|set| !set.is_empty())
}

/// A statue's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatueData {
    /// Time left in which hits do nothing, ms.
    pub cooldown_ms: i32,
    /// Its loot codes, in the order the fan releases them (see
    /// [`loot_order`]).
    pub loot: [i32; 9],
}

/// The statue data of `o`, if it has any.
pub fn data_of(o: &Object) -> Option<&StatueData> {
    match &o.data {
        ObjectData::Statue(d) => Some(d),
        _ => None,
    }
}

fn data_mut(o: &mut Object) -> &mut StatueData {
    if !matches!(o.data, ObjectData::Statue(_)) {
        o.data = ObjectData::Statue(StatueData::default());
    }
    match &mut o.data {
        ObjectData::Statue(d) => d,
        _ => unreachable!("set above"),
    }
}

/// One logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(o) = w.objects.get(id) else { return };
    match o.state {
        state::SET_UP => set_up(w, id),
        state::IDLE => {
            let o = w.objects.get_mut(id).expect("checked");
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
            // Unlike a crate, a broken statue stays.
            w.objects.get_mut(id).expect("checked").state = state::IDLE;
        }
        _ => {}
    }
}

fn set_up(w: &mut World, id: ObjectId) {
    let o = w.objects.get(id).expect("caller checked");
    let first = w.image_frames(&o.image_set).map(|(first, _)| first);
    let picture = first.and_then(|f| w.frame_box(&o.image_set, f));
    let o = w.objects.get_mut(id).expect("caller checked");
    if let Some(first) = first {
        o.frame = first;
    }
    o.type_flags = CRATE_TYPE;
    o.hit_type_flags = CRATE_HIT_BY;
    o.hit_handler = true;
    o.hit_rect = picture.unwrap_or(FALLBACK_BOX);
    if o.z == 0 {
        o.z = DEFAULT_Z;
        o.flags |= flags::RESORT;
    }
    if o.health >= UNBREAKABLE {
        o.flags |= flags::NO_COLLIDE | flags::SAFE;
    } else {
        o.health = if o.health > 1 { 2 } else { 1 };
    }
    let loot = loot_order(o.record.as_deref().map_or(&[][..], |r| &r[..]));
    o.data = ObjectData::Statue(StatueData {
        cooldown_ms: 0,
        loot,
    });
    o.state = state::IDLE;
}

/// `attacker` hit `id` (see the module documentation).
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    crates::absorb_shot(w, attacker);
    let frames = w.objects.get(id).and_then(|o| w.image_frames(&o.image_set));
    let Some(o) = w.objects.get_mut(id) else {
        return;
    };
    // Pair collision never hits a safe object; a broken statue is safe
    // for good.
    if o.flags & flags::SAFE != 0 || o.state != state::IDLE || data_mut(o).cooldown_ms > 0 {
        return;
    }
    o.health -= 1;
    if o.health > 0 {
        o.frame = crates::next_frame(o.frame, frames);
        data_mut(o).cooldown_ms = HIT_COOLDOWN_MS;
        sound::play_from(w, id, DAMAGE_SOUND, SoundChannel::Effect);
        return;
    }
    o.flags |= flags::SAFE;
    o.state = state::BREAKING;
    w.set_anim(id, BREAK_ANIM);
    drop_fan(w, id);
    crates::break_sound(w, id);
}

/// `id` attacked `victim`. Statues attack nothing.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    let _ = (w, id, victim);
}

/// Releases a statue's treasure: for each loot code in order that
/// [`releases`] a goodie, a bouncing goodie 24 px above the statue at its
/// depth minus one (front) or plus one (behind), thrown up at 200 to
/// 500 px/s (one draw of 0..301) and sideways from `-12.5 n` px/s for `n`
/// non-empty codes, 25 px/s more for each goodie released. Returns the
/// goodies.
pub fn drop_fan(w: &mut World, id: ObjectId) -> Vec<ObjectId> {
    let Some(o) = w.objects.get(id) else {
        return Vec::new();
    };
    let (x, y, z) = (o.x, o.y + DROP_Y, o.z);
    let dz = drop_z_offset(&o.logic).unwrap_or(1);
    let loot = data_of(o).map_or([0; 9], |d| d.loot);
    let level = w.level_number();
    let n = loot.iter().filter(|&&c| c != 0).count() as i32;
    let mut vx = fan_start(n);
    let mut out = Vec::new();
    for code in loot {
        if !releases(code, level) {
            continue;
        }
        let vy = -(w.rng.below(301) as i32 + FAN_RISE);
        if let Some(g) = pickup::spawn_drop(w, x, y, code) {
            if let Some(o) = w.objects.get_mut(g) {
                o.z = z + dz;
                if let ObjectData::Pickup(d) = &mut o.data
                    && let Some(m) = d.goodie.as_mut()
                {
                    m.vx = f64::from(vx);
                    m.vy = f64::from(vy);
                }
            }
            out.push(g);
        }
        vx += FAN_STEP;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_treasure_flies_just_in_front_and_behind_treasure_just_behind() {
        assert_eq!(drop_z_offset(FRONT), Some(-1));
        assert_eq!(drop_z_offset(BEHIND), Some(1));
        assert_eq!(drop_z_offset("FrontCrate"), None);
    }

    #[test]
    fn the_fan_is_centred_on_the_statue() {
        // Eight treasures: -100, -75, ..., 75.
        assert_eq!(fan_start(8), -100);
        assert_eq!(fan_start(8) + 7 * FAN_STEP, 75);
        // Odd counts round towards zero.
        assert_eq!(fan_start(3), -37);
        assert_eq!(fan_start(1), -12);
        assert_eq!(fan_start(0), 0);
    }

    #[test]
    fn the_fan_takes_the_powerup_then_the_two_boxes_left_right_top_bottom() {
        let mut r = [0; 71];
        r[15] = 1;
        for (i, f) in (35..=42).enumerate() {
            r[f] = 10 + i as i32;
        }
        assert_eq!(loot_order(&r), [1, 10, 12, 11, 13, 14, 16, 15, 17]);
        assert_eq!(loot_order(&[]), [0; 9]);
    }

    #[test]
    fn only_codes_with_a_picture_are_released() {
        assert!(releases(33, 5), "a coin");
        assert!(releases(1, 5), "a gold bar");
        assert!(!releases(0, 5));
        assert!(!releases(-1, 5));
        assert!(!releases(62, 5));
    }
}
