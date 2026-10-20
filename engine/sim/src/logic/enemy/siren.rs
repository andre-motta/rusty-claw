//! The Siren of the eleventh and twelfth levels, whose scream locks
//! Claw's controls for two seconds.
//!
//! She is a human enemy who cannot hurt Claw herself except by contact.
//! Whenever Claw is inside her range boxes and her wait has run out she
//! usually sings ([`EnemyState::Singing`]): on the eleventh tick of the
//! song ([`CALL_TICK`], the tick her song's second picture,
//! [`CALL_IMAGE`], shows) her call sounds and her first scream leaves
//! from [`SCREAM_OFFSET`] of her position; three more follow, one every
//! [`SCREAM_GAP_MS`] ([`SCREAM_GAP_TICKS`] ticks), and she goes home when
//! the call's [`SONG_MS`] have run out. A scream
//! ([`crate::logic::projectile::scream`]) flies slowly ([`SCREAM_SPEED`]
//! px/s) and, when it touches Claw, crouching or not, sets the scream
//! lockout ([`crate::logic::World::scream_lock_ms`]) to
//! [`crate::logic::projectile::scream::LOCK_MS`]: his controls are
//! ignored until it has run down, and every key still held then counts
//! as pressed.
//!
//! She has [`HEALTH`] health, can be lifted and drops her record's loot.
//!
//! # The rules
//!
//! - Built: the health, damage and boxes above, going by sight,
//!   liftable, the shared human range boxes ([`RANGES`]); then dropped to
//!   the floor and home.
//! - Going home: the shared go home, her idle pose (not advanced), a wait
//!   of [`HOME_WAIT_MS`] plus a draw of 0..=1000 ms, then a draw of
//!   0..=100: below 25 she says her first home line, above 75 her second.
//! - Home: she faces Claw and her pose advances; nothing more while the
//!   wait runs. Then by mode: Claw 300 to 500 px away (mode 0), a draw of
//!   0..=100 below 50 says her advance line and walks at him, otherwise
//!   she sings; 200 to 300 px (mode 1) the same below 25; closer (modes 2
//!   to 4) she sings; out of range (modes 5, 6) she paces. She never looks
//!   for Claw here: she sings at him through a wall.
//! - Singing: she stops; while the song's cooldown ([`SONG_COOLDOWN_MS`]
//!   from the last call) runs she only goes home; otherwise she faces
//!   Claw and plays her song, its screams leaving on the side she faces.
//!   Each tick the song advances; on the tick its second picture shows
//!   her call sounds and the song's time and cooldown start; from then a
//!   scream leaves whenever the gap since the last one is over, at most
//!   [`SCREAMS`] a song; she goes home once the song's time is over (the
//!   song's last picture holds until then).
//! - Hits, death and lifting: the shared human rules; a hit ends a song
//!   (screams in flight fly on, its cooldown still runs) and every hit
//!   reaction ends with her going home. Lifted, her running line stops
//!   and she always says her lifted line.
//! - On the twelfth level only, a close attack of Claw's facing her may
//!   make her hop back ([`HOP_CHANCE`], lowered by [`HOP_CHANCE_STEP`]
//!   each hop, at most one draw every [`HOP_WAIT_MS`]), without a word.
//!
//! # Random draws, in order
//!
//! - building and every going home: one of 0..=1000, then one of 0..=100;
//! - home, when her wait is over in modes 0 and 1: one of 0..=100; then
//!   what it leads to (singing refused by the cooldown goes home);
//! - singing: none but going home's at its end;
//! - lifted: none (her class has no lift roll,
//!   [`super::Class::lift_roll`], so the shared lift makes no draw for
//!   her);
//! - the twelfth level's hop: one of 0..100 per close attack;
//! - the shared ones: the pulse roll, the human idle and pacing, hits and
//!   death.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): she decides in `Home` and
//! idles in the shared human idle; her one new name,
//! [`EnemyState::Singing`], takes 2273 in the eleventh level's block
//! ([`super::mercat::STATE_BLOCK`]), after the Fish's three.
//!
//! # Defaults taken
//!
//! - Whether she is on screen, for the shared human pre-tick and idle,
//!   under a display wider than the original's: the rule the built
//!   soldier and cutthroat use, the object having been drawn in the last
//!   frame.
//! - A laser's beam reaches her through the shared human hit mask
//!   ([`super::HUMAN_HIT_BY`]) and the shared reaction to the level's own
//!   shots: no health lost, a flinch.
//! - The song lasts as long as her call's sound file, [`SONG_MS`] when the
//!   level does not know its length.
//! - The twelfth level's hop is the shared human hop back; its timing has
//!   not been measured.
//!
//! Its data ([`SirenData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use super::human::{self, HumanAnims, HumanCall, HumanData};
use super::lift::Carry;
use super::states::{self, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, Deferred, EnemyData, EnemyFields, EnemyState,
    HUMAN_HIT_BY, Handler, Timer, build, data, data_mut, face_claw, faces_right, human_fields, obj,
    obj_mut, play, rand, say, set_anim, set_state,
};
use crate::logic::projectile::{self, ProjectileSpec, scream};
use crate::logic::{ObjectData, World, sound};
use crate::object::{ObjectId, Rect, types};
use crate::player::attack::kind as attack_kind;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Siren"];

/// Her picture.
pub const IMAGE_SET: &str = "LEVEL_SIREN";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play: hers, and her
/// scream's (see [`crate::logic::projectile::scream`]).
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_SIREN_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Her one new state.
pub const STATES: [EnemyState; 1] = [EnemyState::Singing];

/// Her health, before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 9;
/// Damage of touching her, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Her smarts at the default factor.
pub const SMARTS: i32 = 99;

/// The contact box: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box: the first picture's.
pub const MOVE_RECT: Rect = Rect::new(-33, -63, 26, 44);
/// Movement box while Claw carries her.
pub const CARRIED_RECT: Rect = Rect::new(-34, -44, 46, 11);
/// The five range boxes (half width, half height), outermost first: the
/// shared human ones.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (200, 40), (150, 40), (100, 40)];

/// Where a scream leaves, from her position, facing right; mirrored when
/// she faces left.
pub const SCREAM_OFFSET: (i32, i32) = (28, -10);
/// A scream's speed, px/s.
pub const SCREAM_SPEED: i32 = 200;
/// The picture of the song on whose tick her call sounds.
pub const CALL_IMAGE: i32 = 202;
/// The tick of the song on which her call sounds and the first scream
/// leaves, counted from the song's first tick at the 18 ms step
/// (measured).
pub const CALL_TICK: u32 = 11;
/// The gap between two screams of a song, ms.
pub const SCREAM_GAP_MS: u32 = 300;
/// Ticks between two screams of a song at the 18 ms step (measured).
pub const SCREAM_GAP_TICKS: u32 = 17;
/// The most screams of one song.
pub const SCREAMS: u32 = 4;
/// How long a song lasts from her call, ms: the length of her call's
/// sound.
pub const SONG_MS: u32 = 1695;
/// Ticks from the song's first tick to her going home at the 18 ms step
/// (measured).
pub const HOME_TICK: u32 = 106;
/// The wait from one song's call to the next song, ms.
pub const SONG_COOLDOWN_MS: u32 = 4000;
/// Her wait after going home, ms: this plus a draw of 0..=1000.
pub const HOME_WAIT_MS: u32 = 1000;
/// The draw of going home's wait: one of 0..1001.
pub const HOME_SPREAD: u32 = 1001;
/// The draws of her home lines and of her advance: one of 0..101.
pub const VOICE_SPREAD: u32 = 101;
/// Of the draw of going home, below this she says her first home line.
pub const FIRST_HOME_LINE_BELOW: i32 = 25;
/// Of the draw of going home, above this she says her second home line.
pub const SECOND_HOME_LINE_ABOVE: i32 = 75;
/// Of the draw in home, the values below which she walks at Claw rather
/// than sings, Claw 300 to 500 px away (mode 0) and 200 to 300 px (mode
/// 1).
pub const ADVANCE_BELOW: [i32; 2] = [50, 25];
/// The chance in 100 of the twelfth level's hop back, when she is built
/// and each time she paces.
pub const HOP_CHANCE: i32 = 90;
/// How much each hop lowers the chance.
pub const HOP_CHANCE_STEP: i32 = 30;
/// The wait between two draws of the hop, ms.
pub const HOP_WAIT_MS: u64 = 500;

/// Her pose at home and idle.
pub const IDLE_ANIM: &str = "LEVEL_SIREN_IDLE";
/// The song.
pub const SONG_ANIM: &str = "LEVEL_SIREN_STRIKE1";
/// Walking at Claw, and pacing.
pub const ADVANCE_ANIM: &str = "LEVEL_SIREN_ADVANCE";
/// Running at Claw.
pub const FAST_ADVANCE_ANIM: &str = "LEVEL_SIREN_FASTADVANCE";
/// Her jumps, and the twelfth level's hop back.
pub const JUMP_ANIM: &str = "LEVEL_SIREN_JUMP";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_SIREN_KILLFALL";
/// Her call.
pub const CALL_SOUND: &str = "LEVEL_SIREN_SIRENCALL";
/// Her two home lines.
pub const HOME_LINES: [&str; 2] = ["LEVEL_SIREN_00460010", "LEVEL_SIREN_00980002"];
/// Her line as she walks at Claw.
pub const ADVANCE_LINE: &str = "LEVEL_SIREN_00460007";
/// Her line when lifted.
pub const LIFTED_LINE: &str = "LEVEL_SIREN_00980003";
/// Her death.
pub const DEATH_SOUND: &str = "LEVEL_SIREN_00980001";

/// The animations of the shared states.
const ANIMS: ClassAnims = ClassAnims {
    advance: ADVANCE_ANIM,
    fast_advance: FAST_ADVANCE_ANIM,
    pace: ADVANCE_ANIM,
    walk: "",
    hit_low: "LEVEL_SIREN_HITLOW",
    hit_high: "LEVEL_SIREN_HITHIGH",
    home: IDLE_ANIM,
    jump: JUMP_ANIM,
    fall: DEATH_ANIM,
    lifted: "LEVEL_SIREN_LIFTED1",
    lifted2: "LEVEL_SIREN_LIFTED2",
    bounce: "LEVEL_SIREN_THROWN",
};

/// The sounds of the shared states: the shared human hit sounds.
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries her: the shared human offsets and her own box.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_RECT,
};

/// The Siren class.
pub static CLASS: Class = Class {
    name: "Siren",
    human: true,
    construct,
    pre_tick,
    handler,
    go_home,
    go_idle: human::go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    lift_roll: false,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A Siren's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SirenData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Her wait after going home.
    pub wait: Timer,
    /// The current song's time, from her call.
    pub song: Timer,
    /// The wait from her last call to the next song.
    pub song_cooldown: Timer,
    /// The gap after the last scream.
    pub scream_gap: Timer,
    /// Screams of the current song.
    pub screams: u32,
    /// Her call of the current song has sounded.
    pub called: bool,
    /// Where the current song's screams leave across, from her.
    pub scream_dx: i32,
    /// The current song's screams' speed, px/s.
    pub scream_vx: i32,
    /// The twelfth level's hop chance in 100.
    pub hop_chance: i32,
    /// The wait between two draws of the twelfth level's hop.
    pub hop_wait: Timer,
}

/// The Siren data of `id`.
pub fn siren_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a SirenData> {
    siren_data_of(w.objects.get(id)?)
}

/// The Siren data of object `o`, if it is a Siren.
pub fn siren_data_of(o: &crate::object::Object) -> Option<&SirenData> {
    match &o.data {
        ObjectData::Siren(d) => Some(d),
        _ => None,
    }
}

/// The Siren data of `id`, mutable.
pub fn siren_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut SirenData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Siren(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::Singing => Some(singing_tick),
        _ => None,
    }
}

/// One logic call: the shared human logic call.
pub fn tick(w: &mut World, id: ObjectId) {
    human::tick(w, id);
}

/// `attacker` hit `id`: the shared human hit rule.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    super::on_hit(w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

// ----------------------------------------------------------------------
// Building and going home

/// Builds a Siren (see the module documentation). Draws: going home's.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        sight: true,
        contact_rect: CONTACT,
        strike_rect: Rect::UNSET,
        strike_anim: "",
        death_anim: DEATH_ANIM,
        death_sound: DEATH_SOUND,
        lifted_voice: "",
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                idle: IDLE_ANIM,
                jump_back: JUMP_ANIM,
                ..HumanAnims::default()
            },
            ..HumanData::default()
        },
        ..human_fields(w, id)
    };
    let body = Body {
        image_set: IMAGE_SET,
        move_rect: MOVE_RECT,
        hit_rect: HIT_RECT,
        attack_rect: CONTACT,
        health: HEALTH,
        damage: DAMAGE,
        hit_by: HUMAN_HIT_BY,
    };
    build(w, id, body, fields, true);
    let o = obj_mut(w, id);
    if let ObjectData::Enemy(enemy) = std::mem::take(&mut o.data) {
        o.data = ObjectData::Siren(SirenData {
            enemy,
            hop_chance: HOP_CHANCE,
            ..SirenData::default()
        });
    }
    go_home(w, id);
}

/// Goes home (see the module documentation). Draws: one of 0..=1000 for
/// the wait, then one of 0..=100 for her line.
pub fn go_home(w: &mut World, id: ObjectId) {
    states::go_home(w, id);
    set_anim(w, id, IDLE_ANIM);
    let spread = rand(w, HOME_SPREAD);
    let now = w.now_ms;
    if let Some(s) = siren_mut(w, id) {
        s.wait = Timer::new(now, u64::from(HOME_WAIT_MS) + spread as u64);
    }
    let r = rand(w, VOICE_SPREAD);
    if r < FIRST_HOME_LINE_BELOW {
        say(w, id, HOME_LINES[0]);
    } else if r > SECOND_HOME_LINE_ABOVE {
        say(w, id, HOME_LINES[1]);
    }
}

// ----------------------------------------------------------------------
// Home and the song

/// Home (see the module documentation). Draws: in modes 0 and 1 once her
/// wait is over, one of 0..=100; then what it leads to.
pub fn home_tick(w: &mut World, id: ObjectId) {
    face_claw(w, id);
    w.advance_anim(id);
    let now = w.now_ms;
    if siren_data(w, id).is_some_and(|s| !s.wait.done(now)) {
        return;
    }
    match data(w, id).map_or(6, |d| d.mode) {
        m @ (0 | 1) => {
            if rand(w, VOICE_SPREAD) < ADVANCE_BELOW[usize::from(m)] {
                say(w, id, ADVANCE_LINE);
                states::enter_advance(w, id);
            } else {
                sing(w, id);
            }
        }
        2..=4 => sing(w, id),
        _ => {
            if let Some(s) = siren_mut(w, id) {
                s.hop_chance = HOP_CHANCE;
            }
            start_pacing(w, id);
        }
    }
}

/// Starts a song (see the module documentation): while the song's
/// cooldown runs she only goes home. Draws: going home's then.
pub fn sing(w: &mut World, id: ObjectId) {
    if let Some(d) = data_mut(w, id) {
        d.vx = 0.0;
    }
    let now = w.now_ms;
    if siren_data(w, id).is_some_and(|s| !s.song_cooldown.done(now)) {
        go_home(w, id);
        return;
    }
    face_claw(w, id);
    set_state(w, id, EnemyState::Singing);
    set_anim(w, id, SONG_ANIM);
    let right = faces_right(obj(w, id));
    if let Some(s) = siren_mut(w, id) {
        (s.scream_dx, s.scream_vx) = if right {
            (SCREAM_OFFSET.0, SCREAM_SPEED)
        } else {
            (-SCREAM_OFFSET.0, -SCREAM_SPEED)
        };
        s.screams = 0;
        s.called = false;
    }
}

/// How long her song lasts from her call, ms: her call's length, or
/// [`SONG_MS`] when the level does not know it.
pub fn song_ms(w: &World) -> u32 {
    w.sound_ms(CALL_SOUND).unwrap_or(SONG_MS)
}

/// Singing (see the module documentation).
pub fn singing_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let now = w.now_ms;
    let shows_call = obj(w, id).frame == CALL_IMAGE;
    let Some(called) = siren_data(w, id).map(|s| s.called) else {
        return;
    };
    if shows_call && !called {
        play(w, CALL_SOUND);
        let len = u64::from(song_ms(w));
        if let Some(s) = siren_mut(w, id) {
            s.called = true;
            s.song = Timer::new(now, len);
            s.song_cooldown = Timer::new(now, u64::from(SONG_COOLDOWN_MS));
        }
    }
    let Some(s) = siren_data(w, id).cloned() else {
        return;
    };
    if s.called && s.scream_gap.done(now) && s.screams < SCREAMS {
        if let Some(s) = siren_mut(w, id) {
            s.screams += 1;
        }
        release_scream(w, id, s.scream_dx, s.scream_vx);
        if let Some(s) = siren_mut(w, id) {
            s.scream_gap = Timer::new(now, u64::from(SCREAM_GAP_MS));
        }
    }
    if s.called && s.song.done(now) {
        go_home(w, id);
    }
}

/// A scream leaves from (`dx`, [`SCREAM_OFFSET`]'s height) of her, one
/// depth behind her, at `vx` px/s.
fn release_scream(w: &mut World, id: ObjectId, dx: i32, vx: i32) {
    let (x, y, z) = {
        let o = obj(w, id);
        (o.x, o.y, o.z)
    };
    projectile::spawn(
        w,
        ProjectileSpec {
            logic: scream::NAMES[0],
            x: x + dx,
            y: y + SCREAM_OFFSET.1,
            z: z - 1,
            vx,
            vy: 0,
            damage: 0,
            type_flags: types::ENEMY_SHOT,
            hit_type_flags: 0,
            special_flags: 0,
            image_set: scream::IMAGE_SET,
            owner: Some(id),
            fuse_ms: None,
            power: 0,
        },
    );
}

// ----------------------------------------------------------------------
// The twelfth level's hop, and lifting

/// Her own pre-tick step: on the twelfth level, the hop back from a close
/// attack of Claw's facing her (see the module documentation). Draws: one
/// of 0..100 when the hop may be tried.
pub fn pre_tick(w: &mut World, id: ObjectId) {
    if w.level_number() != 12 {
        return;
    }
    let attacking = w.claw.attack.kind != attack_kind::NONE;
    let claw_facing_right = w.claw.facing_right;
    let now = w.now_ms;
    let Some((right, mode, busy)) = data(w, id).map(|d| (d.claw_right, d.mode, d.busy)) else {
        return;
    };
    let Some((chance, wait)) = siren_data(w, id).map(|s| (s.hop_chance, s.hop_wait)) else {
        return;
    };
    let faces_her = claw_facing_right != right;
    if !(attacking && faces_her && (mode == 3 || mode == 4) && wait.done(now)) {
        return;
    }
    if rand(w, 100) >= chance {
        return;
    }
    if !busy && let Some(d) = data_mut(w, id) {
        d.deferred = Some(Deferred::Human(HumanCall::JumpBack));
    }
    if let Some(s) = siren_mut(w, id) {
        s.hop_chance = (s.hop_chance - HOP_CHANCE_STEP).max(0);
        s.hop_wait = Timer::new(now, HOP_WAIT_MS);
    }
}

/// Lifted: her running line stops and she always says her lifted line.
/// She makes no draw, and her class has no lift roll
/// ([`super::Class::lift_roll`]), so the shared lift makes none for her
/// either.
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    sound::enemy_silence(w, id);
    say(w, id, LIFTED_LINE);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
}

#[cfg(test)]
mod tests {
    use super::super::mercat;
    use super::*;
    use crate::logic::projectile::scream;

    #[test]
    fn her_song_takes_the_eleventh_levels_2273() {
        assert_eq!(STATES[0] as i32, 2273);
        assert!(mercat::STATE_BLOCK.contains(&(STATES[0] as i32)));
        assert!(CLASS.human, "a human enemy");
        assert!(handler(EnemyState::Home).is_some());
        assert!(handler(EnemyState::Singing).is_some());
        assert!(handler(EnemyState::Idle).is_none(), "the shared human idle");
    }

    #[test]
    fn a_song_screams_four_times_before_it_ends() {
        // The call on tick 11, then a scream every 17 ticks: the fourth on
        // tick 62, inside the song's 1695 ms (95 ticks) from the call.
        let last = CALL_TICK + (SCREAMS - 1) * SCREAM_GAP_TICKS;
        assert_eq!(last, 62);
        assert!((last - CALL_TICK) * 18 < SONG_MS);
        assert_eq!(SONG_MS.div_ceil(18), 95);
        // 300 ms are over on the 17th tick after a scream, 1695 ms on the
        // 95th after the call.
        assert_eq!(SCREAM_GAP_MS.div_ceil(18), SCREAM_GAP_TICKS);
        assert_eq!(CALL_TICK + SONG_MS / 18 + 1, HOME_TICK);
        // A scream flies 3 or 4 px a tick.
        assert_eq!(SCREAM_SPEED * 18, 3_600);
        // Its lockout is two seconds.
        assert_eq!(scream::LOCK_MS, 2000);
    }

    #[test]
    fn her_art_is_her_own() {
        for key in [
            IDLE_ANIM,
            SONG_ANIM,
            ADVANCE_ANIM,
            FAST_ADVANCE_ANIM,
            JUMP_ANIM,
            DEATH_ANIM,
            CALL_SOUND,
            ADVANCE_LINE,
            LIFTED_LINE,
            DEATH_SOUND,
            HOME_LINES[0],
            HOME_LINES[1],
        ] {
            assert!(key.starts_with(ANIM_PREFIXES[0]), "{key}");
        }
    }

    #[test]
    fn her_class_has_no_lift_roll() {
        assert!(!CLASS.lift_roll, "the shared lift draws nothing for her");
    }
}
