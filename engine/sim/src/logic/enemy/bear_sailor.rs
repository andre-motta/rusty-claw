//! The bear sailor of the seventh level (and, unchanged, of the eighth and
//! the thirteenth): a big sailor who walks up to Claw, swings an overhead
//! blow at him and, when Claw comes close enough, grabs him in a hug.
//!
//! It is a human enemy: it idles, paces, advances at 120 px/s (300 px/s
//! when Claw is far), flinches from Claw's blows, can be lifted and
//! thrown, and drops its record's loot. It never blocks and never hops
//! back. It has [`HEALTH`] health, and its blow and its touch cost Claw
//! [`DAMAGE`].
//!
//! Its decision step stops it about 99 px from a standing Claw, where it
//! alternates pauses of 20 to 33 ticks with its overhead blow, live from
//! the blow's 16th tick (the [`STRIKE`] box, which reaches a Claw standing
//! 30 px away) and over on its 35th. When Claw is within its innermost
//! range box ([`RANGES`], 50 px across and 10 px up or down) at a decision
//! step, Claw has health left and the 50 ms since its last hug have run
//! out, it grabs him instead, whatever Claw is doing.
//!
//! # The hug
//!
//! - The grab ([`EnemyState::StartHug`]): Claw is put in his held state
//!   ([`crate::player::hug`]), its grab sound plays and its grab animation
//!   ([`GRAB_ANIM`]) runs for 19 ticks; then Claw is placed [`HUG_X`] px
//!   in front of it at its own height, facing it, and the hug starts.
//! - The hug ([`EnemyState::Hug`]): every [`SQUEEZE_MS`] Claw loses one
//!   health point, whatever the difficulty, starting on its first tick, so
//!   a full hug costs him 28 at the 18 ms step. While held Claw shows one
//!   of three pictures that follow the bear's squeeze ([`held_frame`]). The
//!   hug ends after [`HOLD_MS`], when Claw has pressed keys on
//!   [`ESCAPE_PRESSES`] of its ticks (a tick with several new presses
//!   counts once, and presses during the grab do not count), when Claw
//!   leaves the held state (a hit from something else), or when a squeeze
//!   empties his health, which kills him. Released, Claw stands where he
//!   was held and drops the 1 px to the floor.
//! - After a hug the bear goes back to its decision step and may grab
//!   again once its next pause or blow is over.
//! - The bear dying lets Claw go. A hit that makes it flinch, or Claw's
//!   lift, does not: Claw stays held, with nothing squeezing, until the
//!   bear grabs him again or something else hits him. A springboard
//!   launching the bear in mid-hug lets him go.
//!
//! # The rules
//!
//! - Built: the health and damage above, smarts 100, liftable, not going
//!   by sight; the range boxes [`RANGES`]; then dropped to the floor and
//!   idle.
//! - Going idle: unless the range mode is 6, three times in five one of
//!   its three go idle lines; then the shared human go idle.
//! - Home: the alert once, the first time it sees Claw (seven times in ten
//!   one of its two lines); it faces Claw and draws one number it does not
//!   use; the grab when it applies; then, still Home, the shared human
//!   Home at its own speeds: running at Claw 150 to 500 px away, walking
//!   at him 100 to 150 px away, and closer in its blow (100 times in 421)
//!   or a pause of 300 to 600 ms in its home pose.
//! - Lifted: it stops any line it is saying and says one of its two
//!   lifted lines (the second 35 times in 100).
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): the bear decides in `Home`
//! with a handler of its own and inherits every other shared human state
//! with its number. Its two new names take the first two numbers of the
//! seventh level's block [`STATE_BLOCK`]: [`EnemyState::StartHug`] and
//! [`EnemyState::Hug`]; [`EnemyState::Release`] (third) is reserved for a
//! release state no rule enters, and [`EnemyState::GabrielAttack`]
//! (fourth) is the red-tail pirate's ([`super::redtail_pirate`]). The
//! crabs' states take the block's upper half ([`super::crab`]).
//!
//! # Random draws, in order
//!
//! - building: the shared human go idle's one of 0..100;
//! - going idle: unless the mode is 6, one of 0..100, then one of 0..3
//!   below 60; then the shared human go idle's;
//! - the pre-tick: the shared roll (frame pulse ticks only);
//! - Home: the alert's one of 0..100 (then one of 0..2 below 70); one
//!   draw not used; then, still Home, the shared Home's one of 0..421 (and
//!   one of 0..301 before a pause);
//! - the grab and the hug draw nothing;
//! - lifted: the shared lift's one of 0..100, which also picks its line.
//!
//! # Defaults taken
//!
//! - The block of state numbers: blocks of 20 in level order, so the
//!   seventh level owns 2180 to 2199, the bear's and the pirate's states
//!   from 2180 and the crabs' from 2190.
//! - The "on screen" test of its alert under a widened view follows the
//!   rule the soldier's sight test uses.
//! - The hug's loop sound plays at the configured sound volume and stops
//!   when the hug ends. All bears share one loop: any bear's release stops
//!   it.
//! - Whether its touch can hurt the Claw it grabbed while he is within its
//!   reach is not singled out: the shared collision pass decides.
//! - The eighth and thirteenth levels' bears run this code unchanged;
//!   those levels add only tests and table data.
//!
//! Its data ([`BearSailorData`]) holds the shared enemy data, which the
//! shared enemy behaviour reaches through [`super::data`] and
//! [`super::with_data`].

use std::ops::RangeInclusive;

use claw_frame::{AudioCue, SoundChannel, SoundCue};

use super::human::{self, HumanAnims, HumanData};
use super::lift::Carry;
use super::redtail_pirate::{self, Speeds};
use super::states::{self, enter_pause, enter_strike, start_pacing};
use super::{
    Body, Class, ClassAnims, ClassSounds, EnemyData, EnemyFields, EnemyState, HUMAN_HIT_BY,
    Handler, Timer, build, class_of, data, data_mut, exact_x, face_claw, human_fields, obj,
    obj_mut, play, rand, say, sees_claw, set_anim, set_anim_now, set_state,
};
use crate::logic::{GameLogic, ObjectData, World, sound};
use crate::object::{LogicKind, ObjectId, Rect, engine_state};
use crate::player::hug;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["BearSailor"];

/// Its picture (the records name it; the bear keeps it).
pub const IMAGE_SET: &str = "LEVEL_BEARSAILOR";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_BEARSAILOR_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the seventh level's families: the
/// bear's [`EnemyState::StartHug`] and [`EnemyState::Hug`], the reserved
/// [`EnemyState::Release`], the pirate's [`EnemyState::GabrielAttack`],
/// and from [`CRAB_STATES_START`] the crabs' nine states.
pub const STATE_BLOCK: RangeInclusive<i32> = 2180..=2199;

/// The first number of the crabs' half of [`STATE_BLOCK`].
pub const CRAB_STATES_START: i32 = 2190;

/// Health before the difficulty factor (the record's is not used).
pub const HEALTH: i32 = 10;
/// Damage of its blow and of touching it, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// Smarts base.
pub const SMARTS: i32 = 100;
/// Walking speed towards Claw, and pacing speed, px/s.
pub const ADVANCE_SPEED: i32 = 120;
/// Running speed towards a far Claw, px/s.
pub const FAST_ADVANCE_SPEED: i32 = 300;
/// The overhead blow's box, facing left.
pub const STRIKE: Rect = Rect::new(-101, -60, -41, -21);
/// Hit box.
pub const HIT_RECT: Rect = Rect::new(-20, -50, 20, 50);
/// Movement box.
pub const MOVE_RECT: Rect = Rect::new(-20, -60, 20, 61);
/// Movement box while Claw carries it.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);
/// The contact box outside its blow: touching it hurts Claw.
pub const CONTACT: Rect = Rect::new(-1, -1, 1, 1);
/// The five range boxes (half width, half height), outermost first; the
/// innermost is the hug's.
pub const RANGES: [(i32, i32); 5] = [(500, 40), (300, 40), (150, 40), (100, 40), (50, 10)];

/// The blow.
pub const STRIKE_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE1";
/// The grab, 19 ticks at the 18 ms step.
pub const GRAB_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE2";
/// The squeeze, a loop of 31 ticks at the 18 ms step.
pub const SQUEEZE_ANIM: &str = "LEVEL_BEARSAILOR_STRIKE3";
/// Standing idle.
pub const IDLE_ANIM: &str = "LEVEL_BEARSAILOR_IDLE";
/// The body flying off after death.
pub const DEATH_ANIM: &str = "LEVEL_BEARSAILOR_KILLFALL";
/// The grab sound.
pub const GRAB_SOUND: &str = "LEVEL_BEARSAILOR_00380024";
/// The loop that plays while it squeezes.
pub const HUG_SOUND: &str = "LEVEL_BEARSAILOR_BEARHUG";
/// Handle of the hug loop, shared by every bear.
pub const HUG_VOICE: u32 = 0x0B00_0001;
/// The two alert lines.
pub const ALERT_LINES: [&str; 2] = ["LEVEL_BEARSAILOR_00380000", "LEVEL_BEARSAILOR_00380007"];
/// The three go idle lines.
pub const IDLE_LINES: [&str; 3] = [
    "LEVEL_BEARSAILOR_00380015",
    "LEVEL_BEARSAILOR_00380018",
    "LEVEL_BEARSAILOR_00380019",
];
/// The two lifted lines: the first most of the time, the second
/// [`SECOND_LIFTED_CHANCE`] times in 100.
pub const LIFTED_LINES: [&str; 2] = ["LEVEL_BEARSAILOR_00380020", "LEVEL_BEARSAILOR_00380023"];

/// Chance in 100 that the first sight of Claw says an alert line.
pub const ALERT_CHANCE: i32 = 70;
/// Chance in 100 that going idle says a go idle line.
pub const IDLE_LINE_CHANCE: i32 = 60;
/// Chance in 100 that a lift says the second lifted line.
pub const SECOND_LIFTED_CHANCE: u32 = 35;

/// Ticks of the grab at the 18 ms step: the hug starts on the grab's 19th
/// tick.
pub const GRAB_TICKS: u32 = 19;
/// How far in front of the bear Claw is held, px.
pub const HUG_X: i32 = 30;
/// Time between two squeezes, ms; each costs Claw one health point.
pub const SQUEEZE_MS: u32 = 100;
/// The longest hug, ms.
pub const HOLD_MS: u32 = 3000;
/// Ticks with a key press during the hug that free Claw.
pub const ESCAPE_PRESSES: u32 = 10;
/// Time after a hug before the bear may grab again, ms.
pub const HUG_COOLDOWN_MS: u32 = 50;

/// The bear's squeeze pictures and the picture Claw shows while each is
/// on screen.
pub const HELD_FRAMES: [(i32, i32); 3] = [(302, 450), (303, 451), (304, 452)];

/// The picture Claw shows while the bear shows squeeze picture
/// `bear_frame`; `None` for a picture of no squeeze.
pub fn held_frame(bear_frame: i32) -> Option<i32> {
    HELD_FRAMES
        .iter()
        .find(|(bear, _)| *bear == bear_frame)
        .map(|&(_, claw)| claw)
}

/// Where the bear at `bear_x` holds Claw, and whether Claw then faces
/// right: in front of the bear on the side Claw was on, facing it.
pub fn hold_point(bear_x: i32, claw_left_of_bear: bool) -> (i32, bool) {
    if claw_left_of_bear {
        (bear_x - HUG_X, true)
    } else {
        (bear_x + HUG_X, false)
    }
}

/// The bear's speeds.
pub const SPEEDS: Speeds = Speeds {
    advance: ADVANCE_SPEED as f64,
    fast: FAST_ADVANCE_SPEED as f64,
};

/// The animations of the shared states.
const ANIMS: ClassAnims = ClassAnims {
    advance: "LEVEL_BEARSAILOR_ADVANCE",
    fast_advance: "LEVEL_BEARSAILOR_FASTADVANCE",
    pace: "LEVEL_BEARSAILOR_FASTADVANCE",
    walk: "",
    hit_low: "LEVEL_BEARSAILOR_HITLOW",
    hit_high: "LEVEL_BEARSAILOR_HITHIGH",
    home: "LEVEL_BEARSAILOR_HOME",
    jump: "LEVEL_BEARSAILOR_JUMP",
    fall: DEATH_ANIM,
    lifted: "LEVEL_BEARSAILOR_LIFTED1",
    lifted2: "LEVEL_BEARSAILOR_LIFTED2",
    bounce: "LEVEL_BEARSAILOR_THROWN",
};

/// The sounds of the shared states: the shared human hit sounds; its blow
/// is silent.
const SOUNDS: ClassSounds = ClassSounds {
    strike: "",
    hit_low: ["GAME_SOLHITL2", super::HIT_LOW_SECOND],
    hit_high: "GAME_SOLHITHI",
};

/// How Claw carries a bear: the shared human offsets.
const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};

/// The bear sailor class.
pub static CLASS: Class = Class {
    name: "BearSailor",
    human: true,
    construct,
    pre_tick: super::nothing,
    handler,
    go_home: states::go_home,
    go_idle,
    carry: Some(CARRY),
    pick_lift_voice,
    anims: ANIMS,
    sounds: SOUNDS,
};

/// A bear sailor's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BearSailorData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// Ticks of the running hug with a key press.
    pub escape: u32,
    /// The running hug's length.
    pub hold: Timer,
    /// The wait after a hug before the next grab.
    pub hug_wait: Timer,
    /// The wait between two squeezes.
    pub squeeze: Timer,
    /// This bear started the hug loop that plays.
    pub looping: bool,
}

/// The bear data of `id`.
pub fn bear_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a BearSailorData> {
    match &w.objects.get(id)?.data {
        ObjectData::BearSailor(d) => Some(d),
        _ => None,
    }
}

fn bear_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut BearSailorData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::BearSailor(d) => Some(d),
        _ => None,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Home => Some(home_tick),
        EnemyState::StartHug => Some(grab_tick),
        EnemyState::Hug => Some(hug_tick),
        EnemyState::Advance => Some(advance_tick),
        EnemyState::FastAdvance => Some(fast_advance_tick),
        _ => None,
    }
}

/// One logic call: the shared one, with the hug's safety net before the
/// post-tick's scheduled call (a call scheduled while it hugs, such as a
/// springboard's launch, lets Claw go first). A bear that dies in the
/// call lets a held Claw go.
pub fn tick(w: &mut World, id: ObjectId) {
    let Some(class) = class_of(w, id) else { return };
    let alive = data(w, id).is_some_and(|d| !d.dead);
    let raw = obj(w, id).state;
    if matches!(
        raw,
        engine_state::INIT | engine_state::DESTROYING | engine_state::KILLED
    ) || EnemyState::from_id(raw).is_none()
    {
        super::tick(w, id, class);
    } else {
        super::pre_tick(w, id, class);
        (class.pre_tick)(w, id);
        let s = super::state(w, id).unwrap_or_default();
        if let Some(h) = (class.handler)(s).or_else(|| states::shared_handler(s, class.human)) {
            h(w, id);
        }
        let hugging = super::state(w, id) == Some(EnemyState::Hug);
        if hugging && data(w, id).is_some_and(|d| d.deferred.is_some()) {
            let_go(w, id);
        }
        super::post_tick(w, id, class);
    }
    released_by_death(w, id, alive);
}

/// `attacker` hit `id`: the shared human hit rule; a bear it kills lets a
/// held Claw go.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let alive = data(w, id).is_some_and(|d| !d.dead);
    super::on_hit(w, id, attacker);
    released_by_death(w, id, alive);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

fn released_by_death(w: &mut World, id: ObjectId, was_alive: bool) {
    if was_alive && data(w, id).is_some_and(|d| d.dead) {
        hug::release_bearhug(w);
    }
}

// ----------------------------------------------------------------------
// Building and idling

/// Builds a bear sailor (see the module documentation). Draws: the shared
/// human go idle's one of 0..100.
pub fn construct(w: &mut World, id: ObjectId) {
    let fields = EnemyFields {
        ranges: RANGES,
        contact_rect: CONTACT,
        strike_rect: STRIKE,
        strike_anim: STRIKE_ANIM,
        death_anim: DEATH_ANIM,
        // Its death sound is not shipped: it dies silently.
        death_sound: "",
        lifted_voice: "",
        smarts: super::smarts(SMARTS),
        human: HumanData {
            anims: HumanAnims {
                idle: IDLE_ANIM,
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
        o.data = ObjectData::BearSailor(BearSailorData {
            enemy,
            ..BearSailorData::default()
        });
    }
    go_idle(w, id);
}

/// Goes idle: unless the range mode is 6, three times in five one of its
/// go idle lines; then the shared human go idle. Draws: one of 0..100 and
/// one of 0..3 below 60, unless the mode is 6; then the human go idle's
/// one of 0..100.
pub fn go_idle(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| d.mode != 6) && rand(w, 100) < IDLE_LINE_CHANCE {
        let line = IDLE_LINES[rand(w, 3) as usize];
        say(w, id, line);
    }
    human::go_idle(w, id);
}

/// Lifted: it stops its line and says one of its two lifted lines; the
/// shared lift's draw of 0..100 that follows picks which (below 35 the
/// second).
pub fn pick_lift_voice(w: &mut World, id: ObjectId) {
    sound::enemy_silence(w, id);
    let r = w.rng.clone().below(100);
    let line = LIFTED_LINES[usize::from(r < SECOND_LIFTED_CHANCE)];
    say(w, id, line);
    if let Some(d) = data_mut(w, id) {
        d.lifted_voice = "";
    }
}

// ----------------------------------------------------------------------
// Home

/// Home (see the module documentation). Draws: the alert's (once), one not
/// used, then the shared Home's while still Home.
pub fn home_tick(w: &mut World, id: ObjectId) {
    if data(w, id).is_some_and(|d| !d.alerted) && sees_claw(w, id) {
        if rand(w, 100) < ALERT_CHANCE {
            let line = ALERT_LINES[rand(w, 2) as usize];
            say(w, id, line);
        }
        if let Some(d) = data_mut(w, id) {
            d.alerted = true;
        }
    }
    face_claw(w, id);
    let _ = w.rng.next_u15();
    let now = w.now_ms;
    let close = data(w, id).is_some_and(|d| d.mode == 4);
    let ready = bear_data(w, id).is_some_and(|b| b.hug_wait.done(now));
    if close && ready && w.claw.stats.health > 0 && w.claw_id.is_some() {
        start_hug(w, id);
    }
    human_home(w, id);
}

/// The shared human Home at the bear's speeds: nothing unless still Home;
/// otherwise it faces Claw and, by mode, runs at him (0, 1), walks at him
/// (2), strikes or pauses 300 to 600 ms in its home pose (3, 4: it strikes
/// when the draw is below its smarts) or paces (5, 6). Draws: one of
/// 0..421, then one of 0..301 before a pause.
fn human_home(w: &mut World, id: ObjectId) {
    if super::state(w, id) != Some(EnemyState::Home) {
        return;
    }
    face_claw(w, id);
    let r = rand(w, 421);
    let (mode, smarts) = data(w, id).map_or((6, 0), |d| (d.mode, d.smarts));
    match mode {
        0 | 1 => redtail_pirate::enter_fast_advance_at(w, id, SPEEDS),
        2 => redtail_pirate::enter_advance_at(w, id, SPEEDS),
        3 | 4 => {
            if r < smarts {
                enter_strike(w, id);
            } else {
                set_anim_now(w, id, ANIMS.home);
                let ms = states::PAUSE_MIN + rand(w, states::PAUSE_SPREAD) as u64;
                enter_pause(w, id, ms);
            }
        }
        _ => start_pacing(w, id),
    }
}

fn advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::advance_tick_at(w, id, SPEEDS);
}

fn fast_advance_tick(w: &mut World, id: ObjectId) {
    redtail_pirate::fast_advance_tick_at(w, id, SPEEDS);
}

// ----------------------------------------------------------------------
// The hug

/// The grab: its grab sound, the grab animation (not advanced), and Claw
/// held from this tick.
pub fn start_hug(w: &mut World, id: ObjectId) {
    play(w, GRAB_SOUND);
    set_state(w, id, EnemyState::StartHug);
    set_anim(w, id, GRAB_ANIM);
    hug::enter_bearhug(w);
}

/// StartHug: the grab plays; when it ends Claw is put in front of the
/// bear on his side, facing it, at the bear's height, and the hug starts.
pub fn grab_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    let left = !data(w, id).is_some_and(|d| d.claw_right);
    let exact = exact_x(w, id);
    let x = if left {
        exact - f64::from(HUG_X)
    } else {
        exact + f64::from(HUG_X)
    };
    let y = obj(w, id).y;
    hug::place(w, x.trunc() as i32, y, left);
    enter_hug(w, id);
}

/// Starts the squeeze: the squeeze animation (not advanced), the hold of
/// [`HOLD_MS`], the hug loop unless a bear's loop already plays, and no
/// presses counted yet.
fn enter_hug(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Hug);
    set_anim(w, id, SQUEEZE_ANIM);
    let now = w.now_ms;
    let start_loop = !loop_playing(w);
    if start_loop {
        w.cue(AudioCue::Play(SoundCue {
            key: HUG_SOUND.to_string(),
            looped: true,
            channel: SoundChannel::Effect,
            voice: Some(HUG_VOICE),
            ..SoundCue::default()
        }));
    }
    if let Some(b) = bear_mut(w, id) {
        b.hold = Timer::new(now, u64::from(HOLD_MS));
        b.escape = 0;
        b.looping |= start_loop;
    }
}

/// Hug: a tick with a key press counts; Claw is let go when he is no
/// longer held, the hold is over or he pressed on [`ESCAPE_PRESSES`]
/// ticks (the bear goes Home and waits [`HUG_COOLDOWN_MS`] before the
/// next grab); otherwise, every [`SQUEEZE_MS`], a squeeze (which may kill
/// him, and the bear goes Home); then the squeeze plays on and Claw shows
/// the picture that goes with it.
pub fn hug_tick(w: &mut World, id: ObjectId) {
    let now = w.now_ms;
    if hug::escape_pressed(w)
        && let Some(b) = bear_mut(w, id)
    {
        b.escape += 1;
    }
    let Some((hold, escape, squeeze)) = bear_data(w, id).map(|b| (b.hold, b.escape, b.squeeze))
    else {
        return;
    };
    if !hug::held(w) || hold.done(now) || escape >= ESCAPE_PRESSES {
        let_go(w, id);
        states::go_home(w, id);
        if let Some(b) = bear_mut(w, id) {
            b.hug_wait = Timer::new(now, u64::from(HUG_COOLDOWN_MS));
        }
        return;
    }
    if squeeze.done(now) {
        if hug::squeeze(w) {
            stop_loop(w);
            states::go_home(w, id);
        }
        if let Some(b) = bear_mut(w, id) {
            b.squeeze = Timer::new(now, u64::from(SQUEEZE_MS));
        }
    }
    w.advance_anim(id);
    if let Some(f) = held_frame(obj(w, id).frame) {
        hug::show_frame(w, f);
    }
}

/// Stops the hug loop and lets Claw go.
fn let_go(w: &mut World, id: ObjectId) {
    let _ = id;
    stop_loop(w);
    hug::release_bearhug(w);
}

/// True while a bear's hug loop plays.
fn loop_playing(w: &World) -> bool {
    w.objects.all().any(|(_, o)| {
        o.kind == LogicKind::Game(GameLogic::BearSailor)
            && matches!(&o.data, ObjectData::BearSailor(b) if b.looping)
    })
}

/// Stops the hug loop, whichever bear started it.
fn stop_loop(w: &mut World) {
    if !loop_playing(w) {
        return;
    }
    w.cue(AudioCue::Stop { voice: HUG_VOICE });
    let bears: Vec<ObjectId> = w
        .objects
        .all()
        .filter(|(_, o)| o.kind == LogicKind::Game(GameLogic::BearSailor))
        .map(|(id, _)| id)
        .collect();
    for id in bears {
        if let Some(b) = bear_mut(w, id) {
            b.looping = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_states_open_the_seventh_levels_block() {
        let named = [
            EnemyState::StartHug,
            EnemyState::Hug,
            EnemyState::Release,
            EnemyState::GabrielAttack,
        ];
        for (i, s) in named.into_iter().enumerate() {
            assert_eq!(s as i32, STATE_BLOCK.start() + i as i32, "{}", s.name());
        }
        assert!(STATE_BLOCK.contains(&CRAB_STATES_START));
        assert_eq!(CRAB_STATES_START - STATE_BLOCK.start(), 10);
        assert!(CLASS.human, "a human enemy");
        for s in [EnemyState::Home, EnemyState::StartHug, EnemyState::Hug] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(handler(EnemyState::Release).is_none(), "never entered");
        assert!(handler(EnemyState::Strike).is_none(), "the shared strike");
    }

    #[test]
    fn claw_shows_the_picture_of_the_squeeze() {
        assert_eq!(held_frame(302), Some(450));
        assert_eq!(held_frame(303), Some(451));
        assert_eq!(held_frame(304), Some(452));
        assert_eq!(held_frame(301), None);
        assert_eq!(held_frame(450), None);
        assert_eq!(HELD_FRAMES.map(|(_, c)| c), hug::HELD_FRAMES);
    }

    #[test]
    fn claw_is_held_in_front_of_the_bear_facing_it() {
        // The bear at 3681, Claw to its right: held at 3711, facing left.
        assert_eq!(hold_point(3681, false), (3711, false));
        assert_eq!(hold_point(3681, true), (3651, true));
    }

    #[test]
    fn a_full_hug_squeezes_every_hundred_ms() {
        // 3000 ms of hug, a squeeze every 100 ms from its first tick: at
        // the 18 ms step every sixth tick, the last on tick 163 of 167.
        let ticks = HOLD_MS.div_ceil(18);
        assert_eq!(ticks, 167);
        let per = SQUEEZE_MS.div_ceil(18);
        assert_eq!(per, 6);
        let squeezes = (1..ticks).step_by(per as usize).count();
        assert_eq!(squeezes, 28);
        // The grab after a hug waits 3 ticks.
        assert_eq!(HUG_COOLDOWN_MS.div_ceil(18), 3);
    }

    #[test]
    fn its_animations_and_lines_are_its_own() {
        let a = ANIMS;
        for anim in [
            a.advance,
            a.fast_advance,
            a.pace,
            a.hit_low,
            a.hit_high,
            a.home,
            a.jump,
            a.fall,
            a.lifted,
            a.lifted2,
            a.bounce,
            STRIKE_ANIM,
            GRAB_ANIM,
            SQUEEZE_ANIM,
            IDLE_ANIM,
        ] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for s in ALERT_LINES
            .iter()
            .chain(&IDLE_LINES)
            .chain(&LIFTED_LINES)
            .chain(&[GRAB_SOUND, HUG_SOUND])
        {
            assert!(s.starts_with("LEVEL_BEARSAILOR_"), "{s}");
        }
        // It walks as fast as the pirate and runs faster.
        assert_eq!(SPEEDS.advance, redtail_pirate::SPEEDS.advance);
        const { assert!(FAST_ADVANCE_SPEED > redtail_pirate::FAST_ADVANCE_SPEED) };
    }
}
