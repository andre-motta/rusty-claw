//! Aquatis, the boss of the twelfth level: the octopus king who sits in
//! the water of a narrow cave right of the ledge Claw arrives on, with
//! three tentacles ([`super::tentacle`]) rising from the water along that
//! ledge, a row of five stalactites over his head and a glowing crack in
//! the rock behind him.
//!
//! He is a boss-class human enemy who never walks, jumps or blocks. He
//! idles, punches high or low ([`EnemyState::HighPunch`],
//! [`EnemyState::LowPunch`]) when Claw comes close, flinches when hit and
//! cowers after three flinches ([`EnemyState::Scared`]). Claw's sword,
//! pistol, magic and dynamite never take a point of his health: only a
//! falling stalactite does, [`STALACTITE_DAMAGE`] a stalactite, and the
//! fifth kills him.
//!
//! The fight is a loop driven by three helpers, each in its own file:
//!
//! - the crack in the wall ([`crack`]): a blast on it starts a
//!   [`QUAKE_MS`] earthquake that scares him;
//! - the five stalactites over him ([`stalactite`]): when the quake ends
//!   the next one in a fixed order falls on his head; he reels
//!   ([`EnemyState::StalactiteHit`]) and then thrashes in a
//!   [`FRENZY_MS`] frenzy ([`EnemyState::SuperFrenzy`]) with every
//!   tentacle;
//! - the hidden dynamite helper ([`dynamite`]), which puts one stick of
//!   dynamite back on a ledge of the room once Claw has used the last one.
//!
//! He can also be lifted: he never loses the human enemies' liftable
//! flag, so a Claw standing in mid air after Aquatis's own contact hit can
//! lift him on the next tick; Claw, falling, drops him at once, and he
//! falls into the deadly water, where he dies and releases the gem.
//!
//! # The Aquatis link
//!
//! The helpers and the tentacles talk to Aquatis only through one
//! level-wide value kept in the tick's [`Outbox`](crate::logic::Outbox)
//! ([`AquatisLink`], reached through [`World::aquatis_link`] and
//! [`World::aquatis_link_mut`]): his object while he is there, whether he
//! is alive, his frenzy, the earthquake, the armed and the falling
//! stalactite, the outstanding dynamite refill, the stalactites' landing
//! height, the reset bits of his helpers ([`RESET_BITS`]), the frenzy's
//! punch toggle, the order counter of the stalactites and the tentacles'
//! hold on Claw. Every reader treats "no Aquatis" as "do nothing".
//!
//! # The rules
//!
//! - Built: the current boss, his place kept; his box [`BOX`] for moving,
//!   being hit and touching (it never changes with his picture, only with
//!   his facing); hit by Claw, the enemies' bodies, his pistol shots, the
//!   level's shots and explosions and his magic ([`HIT_MASK`]); damage
//!   [`DAMAGE`] and health [`HEALTH`] (times the difficulty factors); the
//!   range boxes [`RANGES`]; never moved by his own movement step (he
//!   hangs where he is placed, over the water); liftable; always active.
//!   The link names him, alive, with the refill outstanding, the landing
//!   height [`LAND_ABOVE`] above him and no reset bit. He waits in
//!   `Startup` with his idle pose.
//! - Each call: Claw's respawn first runs his reset (below); then the
//!   shared human call (the frame pulse's draw, the liftable flag, his
//!   body as his attack box, the range mode, going idle when Claw leaves
//!   every range box, his state, the movement step); a death in the water
//!   is reported; while he does not move by himself he is drawn in front
//!   of Claw when Claw's feet are at or above his own and behind him
//!   otherwise, from his second call on.
//! - Startup: the idle pose plays and holds; when the fight flag reaches
//!   2, the reset. A Claw who skipped the introduction leaves him there,
//!   at full health, never punching, though his body still hurts.
//! - The reset (the start of the fight and every respawn of Claw, only
//!   while he is alive): full health, no stalactite and no flinch
//!   counted, the landing height again, no frenzy, quake, armed or falling
//!   stalactite and no outstanding refill, every reset bit set, and idle.
//!   Neither his place, his facing nor what his death took (below) comes
//!   back.
//! - Going idle: idle; while alive he faces Claw, his idle time of
//!   [`IDLE_MS`] starts, a draw of 0..=100 picks [`IDLE_ANIM`] (below 50)
//!   or [`IDLE2_ANIM`], and a draw of 0..3 picks the line he says.
//! - Idle: nothing at all while he is not alive or a reset bit is still
//!   set (so after the introduction he waits for the crack to wake). The
//!   pose plays; an earthquake makes him cower, and goes on. Then, with
//!   Claw within [`REACH`] px (range modes 2 to 4) and his idle time over,
//!   he attacks; otherwise, when the pose has played out, he goes idle
//!   again. The second pose ends before his idle time does, so only the
//!   first leads to a punch.
//! - The attack: a draw of 0..2; he faces Claw and punches high (0) or low.
//! - A punch: its sound on its first call; on its striking picture
//!   ([`HIGH_PUNCH_FRAME`], [`LOW_PUNCH_FRAME`]) the punch box
//!   ([`HIGH_PUNCH_BOX`], [`LOW_PUNCH_BOX`]) is his attack box; when it
//!   ends, back to his frenzy while it runs, otherwise idle.
//! - Cowering: [`SCARED_ANIM`] for at least [`SCARED_MS`], his line on its
//!   first call; idle once that time is over, the quake has ended and no
//!   stalactite falls.
//! - A hit (see [`on_hit`]): nothing while he is not alive, in his frenzy,
//!   reeling, flinching or dead, or while his hit cooldown runs; otherwise
//!   a body of the enemies' kind (a falling stalactite) is a stalactite's
//!   hit (cooldown [`STALACTITE_COOLDOWN_MS`]), and anything else, unless
//!   he cowers, makes him flinch (cooldown [`HIT_COOLDOWN_MS`], high after
//!   Claw's uppercut, the shared human flinch with no knockback); each
//!   shows a hit burst. A pistol shot that got that far is used up.
//! - The flinch: when it ends the third flinch in a row makes him cower;
//!   otherwise a draw of 0..=100 below 85 attacks, else he goes idle.
//! - A stalactite's hit: he faces Claw, reels ([`IDLE2_ANIM`], two hit
//!   sounds, his line on its first call) and loses [`STALACTITE_DAMAGE`];
//!   the fifth sets his health to 0 and he dies at once. When the reeling
//!   ends his frenzy starts.
//! - The frenzy: the link's frenzy flag, [`FRENZY_MS`], his line on its
//!   first call; each call while it lasts starts the next punch, high when
//!   the link's toggle reads 0 and low otherwise, flipping the toggle;
//!   each punch comes back to it. Once its time is over (at the end of a
//!   punch) the flag is cleared and he goes idle.
//! - Death: [`DEATH_ANIM`], his line on its first call; he hurts nothing,
//!   his body and punch boxes are gone. When it ends, once, with Claw
//!   alive: Claw is held, the gem rises out of him
//!   ([`World::boss_defeated`]), his line ends and he is no longer alive.
//!   He is never removed. With Claw dead at that moment nothing more
//!   happens; Claw's respawn then runs the reset and the fight starts
//!   over, with a body that hurts nothing.
//! - Lifted: the shared human lift ([`super::lift`]), except that he stays
//!   where he hangs until Claw's carry places him ([`lifted`]). Let go, he
//!   leaves from Claw's carry point beside Claw with his own box back and
//!   falls; in the water he dies at once, and with Claw alive the gem
//!   rises from where he sank. Being removed, he is no longer the current
//!   boss nor named in the link.
//! - Claw's respawn: his reset is made by the first of his room's logics
//!   to run after it (see [`reset_on_respawn`]), so every helper resets in
//!   the respawn's own update passes, and his idling waits for the next
//!   tick.
//! - The gem: in the tick it is made it does not move (its flight runs on
//!   game time); see [`World::boss_defeated`] for the rest.
//!
//! # State numbers
//!
//! One number per state name ([`EnemyState`]): he waits in the bosses'
//! `Startup`, idles in `Idle`, flinches in Raux's `HitHigh` and `HitLow`
//! and dies in Gabriel's `DeadAct1`; his five new names open the twelfth
//! level's block [`STATE_BLOCK`] (2280 to 2299) and the tentacles' three
//! follow from 2290 ([`super::tentacle::STATES`]). His helpers keep their
//! own states in their data.
//!
//! # Random draws, in order
//!
//! - each call: the shared pre-tick's (frame pulse ticks only);
//! - going idle while alive: one of 0..=100, then one of 0..3;
//! - the attack: one of 0..2;
//! - the end of a flinch (not the third): one of 0..=100, then the
//!   attack's;
//! - a flinch (in the collision pass): one of 0..4 for a high one, then
//!   the hit burst's two; a stalactite's hit: the hit burst's two.
//!
//! # Defaults taken
//!
//! - The state numbers: blocks of 20 in level order, so the twelfth level
//!   owns 2280 to 2299, Aquatis's names from 2280 and the tentacles' from
//!   2290; the frenzy number is shared by both.
//! - The frenzy's punch toggle and the stalactites' order counter are
//!   kept in the game's state, in the link: 0 and 1 when a level loads,
//!   the counter put back to 1 by every stalactite reset. A reference run
//!   that carries either over from an earlier attempt in the same game
//!   would show a divergence to record.
//! - The run-time depth of Aquatis and the tentacles, which decides their
//!   order against Claw in the update pass (3909 and 3899 while Claw is
//!   below them in the passage under the water, 4010 and 4000 once he is
//!   above): it is the shared enemy depth sort against Claw's feet
//!   ([`super::motion::DEPTH_BEHIND`], [`super::motion::DEPTH_IN_FRONT`]
//!   over a base of 10 for him and 0 for them), which puts an enemy
//!   behind Claw when his feet are below its own.
//! - The fight-pocket store of his room is played with Claw's dynamite at
//!   9 from route time zero: the replay's carried-in start lines do that
//!   from the level's start (all eight lines, see
//!   [`crate::player::progress::start_lines`]):
//!
//!   ```text
//!   # start_score = 0
//!   # start_health = 100
//!   # start_lives = 7
//!   # start_pistol = 10
//!   # start_magic = 5
//!   # start_dynamite = 9
//!   # start_weapon = pistol
//!   # start_treasure_game = 0, 0, 0, 0, 0, 0, 0, 0, 0
//!   ```
//!
//!   Claw's placement and the wait to time zero keep the count. Only the
//!   dynamite helper can tell the two apart (it remembers Claw's count
//!   when it is built, 3 in the original's run and 9 in the replay's), and
//!   that count matters only once the first stick has been taken, which
//!   that route never does.
//! - The gem's flight runs on game time: the update pass of the return
//!   from a help screen (see [`crate::Sim::pass_help_return`]) catches up
//!   the screen's time in one step of the flying gem (each way at most
//!   [`super::motion::STEP_LIMIT`] px, see [`crate::logic::pickup`]),
//!   which, moving down by then, lands where it is on the next tick. This
//!   holds for every boss's gem, as it does in the original.
//!
//! His data ([`AquatisData`]) holds the shared enemy data, which the shared
//! enemy behaviour reaches through [`super::data`] and [`super::with_data`].

use std::ops::RangeInclusive;

use super::lift::Carry;
use super::{
    Class, ClassAnims, ClassSounds, DeathKind, EnemyData, EnemyFields, EnemyState, HIT_LOW_SECOND,
    HIT_SOUNDS, Handler, Timer, UNBOUNDED, data, data_mut, face_claw, facing_rect, hit_burst,
    human, nothing, obj, obj_mut, play, rand, say, scaled, set_anim, set_state,
};
use crate::logic::{GameLogic, ObjectData, World, boss_stage, pickup, sound};
use crate::object::{LogicKind, ObjectId, Rect, engine_state, flags, special, types};
use crate::player::state as claw_state;

pub mod crack;
pub mod dynamite;
pub mod stalactite;

/// Logic names this family runs.
pub const NAMES: &[&str] = &["Aquatis"];

/// His picture.
pub const IMAGE_SET: &str = "LEVEL_KINGAQUATIS";

/// Image sets this family may switch an object to.
pub const IMAGE_SETS: &[&str] = &[IMAGE_SET];

/// Prefixes of the animations this family may play.
pub const ANIM_PREFIXES: &[&str] = &["LEVEL_AQUATIS_"];

/// Whether this family runs its logics yet (see
/// [`crate::logic::built`]).
pub const BUILT: bool = true;

/// Enemy state numbers reserved for the twelfth level's families:
/// Aquatis's five new states ([`STATES`]) from 2280 and the tentacles'
/// three ([`super::tentacle::STATES`]) from 2290; the rest are free.
pub const STATE_BLOCK: RangeInclusive<i32> = 2280..=2299;

/// His new states.
pub const STATES: [EnemyState; 5] = [
    EnemyState::HighPunch,
    EnemyState::LowPunch,
    EnemyState::SuperFrenzy,
    EnemyState::StalactiteHit,
    EnemyState::Scared,
];

/// The pieces of the boss stage on his level: the warp into his room and
/// the stager beside where it puts Claw.
pub const STAGE: [&str; 2] = [boss_stage::WARP, boss_stage::STAGER];

/// His health before the difficulty factor, which is also the health a
/// full health bar stands for.
pub const HEALTH: i32 = 100;
/// Health a falling stalactite takes from him.
pub const STALACTITE_DAMAGE: i32 = 20;
/// Stalactites that kill him.
pub const STALACTITES: i32 = HEALTH / STALACTITE_DAMAGE;
/// Damage of touching him, before the difficulty factor.
pub const DAMAGE: i32 = 10;
/// His box (movement, hit and contact) facing left, the one of his idle
/// picture (measured).
pub const BOX: Rect = Rect::new(-62, -85, 109, 96);
/// How far left of him Claw must be for his punches, px.
pub const REACH: i32 = 200;
/// The five range boxes (half width, half height), outermost first: the
/// first two cover his whole room, the inner three are the human enemies'
/// own; he punches only with Claw in one of the inner three.
pub const RANGES: [(i32, i32); 5] = [(2500, 500), (2500, 500), (REACH, 40), (150, 40), (100, 40)];
/// What may hit him: Claw, the enemies' bodies (a falling stalactite),
/// Claw's pistol shots, the level's shots and explosions, and his magic.
pub const HIT_MASK: u32 = 0x300_0026;
/// How Claw carries him: as the other human enemies, with their carried
/// movement box.
pub const CARRY: Carry = Carry {
    beside: (-20, 20, -15),
    overhead: (10, -10, -45),
    move_rect: CARRIED_MOVE_RECT,
};
/// His movement box while Claw carries him.
pub const CARRIED_MOVE_RECT: Rect = Rect::new(-20, -15, 20, 15);

/// The high punch's box facing left, on its striking picture.
pub const HIGH_PUNCH_BOX: Rect = Rect::new(-118, -76, 0, 10);
/// The low punch's box facing left, on its striking picture.
pub const LOW_PUNCH_BOX: Rect = Rect::new(-130, 10, 0, 60);
/// The high punch's striking picture.
pub const HIGH_PUNCH_FRAME: i32 = 204;
/// The low punch's striking picture.
pub const LOW_PUNCH_FRAME: i32 = 207;

/// His idle time before a punch, ms.
pub const IDLE_MS: u64 = 1000;
/// The shortest time he cowers, ms.
pub const SCARED_MS: u64 = 4000;
/// The hit cooldown after one of Claw's blows or shots, ms.
pub const HIT_COOLDOWN_MS: u64 = 500;
/// The hit cooldown after a stalactite's hit, ms.
pub const STALACTITE_COOLDOWN_MS: u64 = 1000;
/// Chance in 101, at the end of a flinch, that he punches.
pub const FLINCH_ATTACK_CHANCE: i32 = 85;
/// Flinches in a row that make him cower.
pub const FLINCHES: u32 = 3;

/// The earthquake a blast on the crack starts, ms.
pub const QUAKE_MS: u32 = 4000;
/// The crack's wait from one earthquake to the next blast it accepts, ms.
pub const CRACK_COOLDOWN_MS: u32 = 6000;
/// His frenzy after a stalactite's hit, ms.
pub const FRENZY_MS: u32 = 4000;
/// How far above his position the falling stalactites stop, px.
pub const LAND_ABOVE: i32 = 50;

/// Ticks of the introduction at the 18 ms step, from Claw's first tick in
/// the room (measured): the fight comes on one tick later and Claw stands
/// again the tick after.
pub const STAGING_TICKS: u32 = 735;
/// Ticks from his death's last act to the gem's landing at the 18 ms
/// step (measured).
pub const GEM_FLIGHT_TICKS: u32 = 140;
/// Ticks of his death at the 18 ms step (measured).
pub const DEATH_TICKS: u32 = 112;

/// The crack's reset bit.
pub const CRACK_BIT: u32 = 0x2;
/// The tentacles' reset bits, by construction order.
pub const TENTACLE_BITS: [u32; 3] = [0x4, 0x8, 0x10];
/// The stalactites' reset bits, by construction order.
pub const STALACTITE_BITS: [u32; 5] = [0x20, 0x40, 0x80, 0x100, 0x200];
/// Every reset bit his respawn reset sets.
pub const RESET_BITS: u32 = 0x3FE;

/// His idle pose, which also holds while he waits for the fight.
pub const IDLE_ANIM: &str = "LEVEL_AQUATIS_IDLE1";
/// His second idle pose, and his reeling after a stalactite.
pub const IDLE2_ANIM: &str = "LEVEL_AQUATIS_IDLE2";
/// The high punch, which also opens his frenzy.
pub const HIGH_PUNCH_ANIM: &str = "LEVEL_AQUATIS_STRIKE1";
/// The low punch.
pub const LOW_PUNCH_ANIM: &str = "LEVEL_AQUATIS_STRIKE2";
/// Cowering.
pub const SCARED_ANIM: &str = "LEVEL_AQUATIS_IDLE3";
/// His death.
pub const DEATH_ANIM: &str = "LEVEL_AQUATIS_KILLFALL";
/// The high flinch.
pub const HIT_HIGH_ANIM: &str = "LEVEL_AQUATIS_HITHIGH";
/// The low flinch.
pub const HIT_LOW_ANIM: &str = "LEVEL_AQUATIS_HITLOW";

/// The lines he says going idle, one picked each time.
pub const IDLE_LINES: [&str; 3] = [
    "LEVEL_AQUATIS_00480011",
    "LEVEL_AQUATIS_00480012",
    "LEVEL_AQUATIS_00480013",
];
/// The high punch's sound.
pub const HIGH_PUNCH_SOUND: &str = "LEVEL_AQUATIS_00480000";
/// The low punch's sound.
pub const LOW_PUNCH_SOUND: &str = "LEVEL_AQUATIS_00480001";
/// His line as his frenzy starts.
pub const FRENZY_LINE: &str = "LEVEL_AQUATIS_00480002";
/// His line as he cowers.
pub const SCARED_LINE: &str = "LEVEL_AQUATIS_00480005";
/// His line as he dies.
pub const DEATH_LINE: &str = "LEVEL_AQUATIS_00480006";
/// His line as a stalactite hits him.
pub const STALACTITE_LINE: &str = "LEVEL_AQUATIS_00480015";
/// The two sounds of a stalactite's hit.
pub const STALACTITE_SOUNDS: [&str; 2] = ["GAME_SOLHITHI", HIT_SOUNDS[0]];

/// The Aquatis class.
pub static CLASS: Class = Class {
    name: "Aquatis",
    human: true,
    construct,
    pre_tick: nothing,
    handler,
    go_home: go_idle,
    go_idle,
    carry: Some(CARRY),
    pick_lift_voice: nothing,
    lift_roll: true,
    anims: ClassAnims {
        hit_low: HIT_LOW_ANIM,
        hit_high: HIT_HIGH_ANIM,
        ..ClassAnims::NONE
    },
    sounds: ClassSounds {
        strike: "",
        hit_low: ["GAME_SOLHITL2", HIT_LOW_SECOND],
        hit_high: "GAME_SOLHITHI",
    },
};

/// What the helpers and the tentacles share with Aquatis (see the module
/// documentation). It lasts the whole level and starts as
/// [`AquatisLink::default`] leaves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AquatisLink {
    /// Aquatis's object, while he is there.
    pub aquatis: Option<ObjectId>,
    /// He is alive: set when he is built, cleared at the end of his death.
    pub alive: bool,
    /// His frenzy runs; the tentacles thrash while it is set.
    pub frenzy: bool,
    /// An earthquake from the crack runs; he cowers while it is set.
    pub quake: bool,
    /// The next stalactite is armed: set when an earthquake ends, cleared
    /// when it falls.
    pub armed: bool,
    /// A stalactite is falling.
    pub falling: bool,
    /// A stick of dynamite the helper put down waits to be taken.
    pub refill_outstanding: bool,
    /// The height the falling stalactites stop at, px ([`LAND_ABOVE`]
    /// above him).
    pub land_y: i32,
    /// The reset bits still set ([`RESET_BITS`]): his respawn reset sets
    /// them all and each helper clears its own once it has reset.
    pub reset_bits: u32,
    /// The frenzy's punch toggle: the next frenzy punch is high when 0.
    pub frenzy_toggle: u32,
    /// The order of the next stalactite to fall (its record's smarts),
    /// from 1.
    pub stalactite_order: i32,
    /// A tentacle holds Claw.
    pub tentacle_grab: bool,
}

impl Default for AquatisLink {
    /// The link as a level loads: no Aquatis, nothing running, no reset
    /// bit, the toggle at 0 and the order counter at 1.
    fn default() -> AquatisLink {
        AquatisLink {
            aquatis: None,
            alive: false,
            frenzy: false,
            quake: false,
            armed: false,
            falling: false,
            refill_outstanding: false,
            land_y: 0,
            reset_bits: 0,
            frenzy_toggle: 0,
            stalactite_order: 1,
            tentacle_grab: false,
        }
    }
}

/// Aquatis's data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AquatisData {
    /// The data every enemy keeps.
    pub enemy: EnemyData,
    /// The current state's first call has passed (its sound or line
    /// played); a punch sets it, so the frenzy's line plays once.
    pub first_done: bool,
    /// His death's end has been handled.
    pub death_done: bool,
    /// Stalactites that hit him since the last reset.
    pub stalactites: i32,
    /// Flinches in a row.
    pub flinches: u32,
    /// His idle time.
    pub idle: Timer,
    /// His shortest time cowering.
    pub scared: Timer,
    /// His frenzy's time.
    pub frenzy: Timer,
    /// His hit cooldown.
    pub hit_cooldown: Timer,
    /// Claw's attempt number at his last reset (or when he was built).
    pub attempt: u32,
    /// A death in the water has been seen.
    pub death_seen: bool,
    /// The game time up to which his idling waits after the reset Claw's
    /// respawn made: the respawn's tick is over before he idles.
    pub respawn_hold: Option<u64>,
}

/// Aquatis's own data of `id`.
pub fn aquatis_data<'a>(w: &'a World, id: ObjectId) -> Option<&'a AquatisData> {
    match &w.objects.get(id)?.data {
        ObjectData::Aquatis(d) => Some(d),
        _ => None,
    }
}

/// Aquatis's own data of `id`, mutable.
pub fn aquatis_mut<'a>(w: &'a mut World, id: ObjectId) -> Option<&'a mut AquatisData> {
    match &mut w.objects.get_mut(id)?.data {
        ObjectData::Aquatis(d) => Some(d),
        _ => None,
    }
}

impl World<'_> {
    /// The Aquatis link (see [`AquatisLink`]).
    pub fn aquatis_link(&self) -> &AquatisLink {
        &self.out.aquatis
    }

    /// The Aquatis link, mutable (see [`World::aquatis_link`]).
    pub fn aquatis_link_mut(&mut self) -> &mut AquatisLink {
        &mut self.out.aquatis
    }

    /// Aquatis's object, while he is there.
    pub fn aquatis(&self) -> Option<ObjectId> {
        self.out.aquatis.aquatis
    }

    /// The reset bits still set (see [`AquatisLink::reset_bits`]).
    pub fn aquatis_reset_bits(&self) -> u32 {
        self.out.aquatis.reset_bits
    }

    /// Sets every reset bit ([`RESET_BITS`]), as his respawn reset and the
    /// start of a staged fight do: each helper resets itself on its next
    /// call and clears its bit.
    pub fn reset_aquatis(&mut self) {
        self.out.aquatis.reset_bits = RESET_BITS;
    }

    /// Clears the reset bit `bit`; true when it was set.
    pub fn clear_aquatis_reset_bit(&mut self, bit: u32) -> bool {
        let was = self.out.aquatis.reset_bits & bit != 0;
        self.out.aquatis.reset_bits &= !bit;
        was
    }

    /// True while an earthquake from the crack runs.
    pub fn aquatis_quake(&self) -> bool {
        self.out.aquatis.quake
    }

    /// True while a tentacle holds Claw.
    pub fn tentacle_grab(&self) -> bool {
        self.out.aquatis.tentacle_grab
    }

    /// Sets or clears the tentacles' hold on Claw.
    pub fn set_tentacle_grab(&mut self, held: bool) {
        self.out.aquatis.tentacle_grab = held;
    }

    /// True while Aquatis is there and alive.
    pub fn aquatis_alive(&self) -> bool {
        self.out.aquatis.aquatis.is_some() && self.out.aquatis.alive
    }
}

/// True when `id` is a stalactite of either family: one of Aquatis's five
/// or a cave stalactite.
pub fn is_stalactite(w: &World, id: ObjectId) -> bool {
    match w.objects.get(id).map(|o| o.kind) {
        Some(LogicKind::Game(g)) => {
            matches!(g, GameLogic::AquatisStalactite | GameLogic::Stalactite)
        }
        _ => false,
    }
}

fn handler(s: EnemyState) -> Option<Handler> {
    match s {
        EnemyState::Startup => Some(startup_tick),
        EnemyState::Idle => Some(idle_tick),
        EnemyState::HighPunch | EnemyState::LowPunch => Some(punch_tick),
        EnemyState::Scared => Some(scared_tick),
        EnemyState::StalactiteHit => Some(stalactite_hit_tick),
        EnemyState::SuperFrenzy => Some(frenzy_tick),
        EnemyState::HitHigh | EnemyState::HitLow => Some(flinch_tick),
        EnemyState::DeadAct1 => Some(death_tick),
        _ => None,
    }
}

// ----------------------------------------------------------------------
// The logic call

/// One logic call (see the module documentation).
pub fn tick(w: &mut World, id: ObjectId) {
    let raw = obj(w, id).state;
    if raw == engine_state::DESTROYING {
        if w.current_boss() == Some(id) {
            w.set_current_boss(None);
        }
        if w.aquatis() == Some(id) {
            w.aquatis_link_mut().aquatis = None;
        }
    }
    if EnemyState::from_id(raw).is_some() {
        reset_on_respawn(w);
    }
    carried(w, id);
    human::run(w, id, &CLASS);
    if raw == engine_state::INIT {
        return;
    }
    after_call(w, id);
    depth_sort(w, id);
}

/// Let go by Claw, he leaves from Claw's carry point beside Claw with his
/// own box back, so it is with his whole body that he meets the water.
fn carried(w: &mut World, id: ObjectId) {
    let letting_go = data(w, id).is_some_and(|d| d.deferred == Some(super::Deferred::Dropped));
    if !letting_go {
        return;
    }
    let Some((cx, cy)) = w.claw_object().map(|c| (c.x, c.y)) else {
        return;
    };
    let right = w.claw.facing_right;
    let (dx, dy) = (
        if right {
            CARRY.beside.1
        } else {
            CARRY.beside.0
        },
        CARRY.beside.2,
    );
    let saved = data(w, id).map_or(BOX, |d| d.saved_move_rect);
    let o = obj_mut(w, id);
    o.x = cx + dx;
    o.y = cy + dy;
    o.move_rect = facing_rect(o, if saved.is_set() { saved } else { BOX });
}

/// `carrier` (Claw) lifted `id`: the shared lift ([`super::lift::lifted`]),
/// except that he stays where he hangs: the pixel the shared lift raises a
/// lifted enemy by is not his.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    let y = obj(w, id).y;
    super::lift::lifted(w, id, carrier);
    obj_mut(w, id).y = y;
}

/// Claw's respawn runs Aquatis's reset once, before any of his room's
/// logics runs again: the first of Aquatis, his crack, his dynamite helper
/// and his stalactites to run after the respawn makes it (the stalactites
/// never sleep and run before the tentacles), so every helper sees its
/// reset bit in that same update pass, as the original's respawn does.
/// Draws: the reset's.
pub fn reset_on_respawn(w: &mut World) {
    let Some(id) = w.aquatis() else { return };
    if !claw_respawned(w, id) {
        return;
    }
    let attempt = w.attempt();
    let hold = w.now_ms + u64::from(crate::tuning::TICK_MS);
    if let Some(d) = aquatis_mut(w, id) {
        d.attempt = attempt;
        d.respawn_hold = Some(hold);
    }
    reset(w, id);
}

/// True once per death of Claw, from his respawn on: his attempt number
/// has moved on since the last reset and he is no longer dying.
fn claw_respawned(w: &World, id: ObjectId) -> bool {
    let dying = w.claw_object().is_none_or(|c| {
        matches!(
            c.state,
            claw_state::DEATH_FALL | claw_state::DEATH | claw_state::INIT
        )
    });
    let attempt = w.attempt();
    !dying && aquatis_data(w, id).is_some_and(|d| d.attempt != attempt)
}

/// A death seen for the first time (in the water) is reported: when Claw
/// is alive, the gem rises from where he died.
fn after_call(w: &mut World, id: ObjectId) {
    let dead = data(w, id).is_some_and(|d| d.dead && d.death == DeathKind::Goo);
    let seen = aquatis_data(w, id).is_some_and(|d| d.death_seen);
    if !dead || seen {
        return;
    }
    if let Some(d) = aquatis_mut(w, id) {
        d.death_seen = true;
    }
    if w.claw.stats.health > 0 {
        release_gem(w, id);
    }
}

/// His defeat ([`World::boss_defeated`]): Claw held and the gem out of
/// him. The gem's flight runs on game time, so in the tick it is made it
/// does not move: when it would get its first call in this very update
/// pass (it lands after the object the pass goes on with), that call is
/// kept for the next tick and the camera heads for it now, as its first
/// call would have it.
fn release_gem(w: &mut World, id: ObjectId) {
    let kept = w.objects.next_of(id);
    w.boss_defeated(id);
    let gem_z = obj(w, id).z.saturating_add(1);
    let runs_now = kept
        .and_then(|k| w.objects.get(k))
        .is_some_and(|k| k.z <= gem_z || k.flags & flags::RESORT != 0);
    if !runs_now {
        return;
    }
    let gem = w
        .objects
        .all()
        .find(|(_, o)| &*o.logic == pickup::END_GEM && !o.is_listed())
        .map(|(g, o)| (g, (o.x, o.y)));
    if let Some((g, at)) = gem {
        if let Some(o) = w.objects.get_mut(g) {
            o.frame_delay = 1;
        }
        w.set_camera_override(Some(at));
    }
}

/// Drawn in front of Claw when his feet are at or above Aquatis's own,
/// behind him otherwise, while Aquatis does not move by himself (his
/// movement step sorts him otherwise); a change takes effect in the next
/// update pass.
fn depth_sort(w: &mut World, id: ObjectId) {
    let Some(claw_feet) = w.claw_object().map(|c| c.y + c.move_rect.bottom) else {
        return;
    };
    let Some((frozen, sorts, base)) = data(w, id).map(|d| (d.frozen, d.z_sort, d.z_base)) else {
        return;
    };
    if !frozen || !sorts {
        return;
    }
    let o = obj_mut(w, id);
    if o.state == engine_state::DESTROYING {
        return;
    }
    let feet = o.y + o.move_rect.bottom;
    let z = base
        + if claw_feet <= feet {
            super::motion::DEPTH_IN_FRONT
        } else {
            super::motion::DEPTH_BEHIND
        };
    if o.z != z {
        o.z = z;
        o.flags |= flags::RESORT;
    }
}

// ----------------------------------------------------------------------
// Building and the reset

/// Builds Aquatis (see the module documentation). Draws nothing.
pub fn construct(w: &mut World, id: ObjectId) {
    w.set_current_boss(Some(id));
    let y = obj(w, id).y;
    let fields = EnemyFields {
        mode: 6,
        ranges: RANGES,
        bounds: UNBOUNDED,
        bounds_y: UNBOUNDED,
        stationary: true,
        z_sort: true,
        z_base: 10,
        may_lift: true,
        frozen: true,
        contact_rect: BOX,
        strike_rect: Rect::UNSET,
        ..EnemyFields::default()
    };
    let attempt = w.attempt();
    let o = obj_mut(w, id);
    o.image_set = crate::level::runtime_image_set(IMAGE_SET);
    o.type_flags = types::ENEMY;
    o.hit_type_flags = HIT_MASK;
    o.special_flags &= !special::LIFTABLE;
    o.special_flags |= crate::player::attack::PUNCHABLE;
    o.flags |= flags::NOTIFY_DESTROY | flags::ALWAYS_ACTIVE;
    o.hit_handler = true;
    o.move_rect = BOX;
    o.hit_rect = BOX;
    o.attack_rect = BOX;
    if super::faces_right(o) {
        o.mirror_rects();
    }
    o.health = scaled(HEALTH, super::HEALTH_FACTOR);
    o.damage = scaled(DAMAGE, super::DAMAGE_FACTOR);
    o.data = ObjectData::Aquatis(AquatisData {
        enemy: fields.into(),
        attempt,
        ..AquatisData::default()
    });
    *w.aquatis_link_mut() = AquatisLink {
        aquatis: Some(id),
        alive: true,
        frenzy: false,
        quake: false,
        armed: false,
        falling: false,
        refill_outstanding: true,
        land_y: y - LAND_ABOVE,
        reset_bits: 0,
        ..*w.aquatis_link()
    };
    set_state(w, id, EnemyState::Startup);
    set_anim(w, id, IDLE_ANIM);
}

/// The reset (see the module documentation): only while he is alive.
/// Draws: going idle's.
pub fn reset(w: &mut World, id: ObjectId) {
    if w.aquatis() != Some(id) || !w.aquatis_link().alive {
        return;
    }
    let y = obj(w, id).y;
    obj_mut(w, id).health = scaled(HEALTH, super::HEALTH_FACTOR);
    if let Some(d) = aquatis_mut(w, id) {
        d.first_done = false;
        d.death_done = false;
        d.stalactites = 0;
        d.flinches = 0;
    }
    let link = w.aquatis_link_mut();
    link.land_y = y - LAND_ABOVE;
    link.frenzy = false;
    link.armed = false;
    link.quake = false;
    link.falling = false;
    link.refill_outstanding = false;
    link.reset_bits = RESET_BITS;
    go_idle(w, id);
}

// ----------------------------------------------------------------------
// States

/// Startup: the pose plays; when the fight flag reaches 2, the reset.
pub fn startup_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if w.boss_fight() == crate::logic::world_boss::fight::ON {
        reset(w, id);
    }
}

/// Goes idle (see the module documentation). Draws, while alive: one of
/// 0..=100, then one of 0..3.
pub fn go_idle(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Idle);
    if !w.aquatis_link().alive {
        return;
    }
    face_claw(w, id);
    let now = w.now_ms;
    if let Some(d) = aquatis_mut(w, id) {
        d.idle = Timer::new(now, IDLE_MS);
    }
    let anim = if rand(w, 101) < 50 {
        IDLE_ANIM
    } else {
        IDLE2_ANIM
    };
    set_anim(w, id, anim);
    let line = IDLE_LINES[rand(w, 3) as usize];
    say(w, id, line);
}

/// Idle (see the module documentation).
pub fn idle_tick(w: &mut World, id: ObjectId) {
    if !w.aquatis_link().alive || w.aquatis_reset_bits() != 0 {
        return;
    }
    let now = w.now_ms;
    if aquatis_data(w, id).is_some_and(|d| d.respawn_hold.is_some_and(|t| now <= t)) {
        return;
    }
    w.advance_anim(id);
    if w.aquatis_quake() {
        enter_scared(w, id);
    }
    let now = w.now_ms;
    let mode = data(w, id).map_or(6, |d| d.mode);
    let rested = aquatis_data(w, id).is_some_and(|d| d.idle.done(now));
    if matches!(mode, 2..=4) && rested {
        attack(w, id);
        return;
    }
    if obj(w, id).anim.is_done() {
        go_idle(w, id);
    }
}

/// The attack: a draw of 0..2, then he faces Claw and punches high (0) or
/// low.
pub fn attack(w: &mut World, id: ObjectId) {
    let high = rand(w, 2) == 0;
    face_claw(w, id);
    enter_punch(w, id, high);
}

/// A punch, high or low.
pub fn enter_punch(w: &mut World, id: ObjectId, high: bool) {
    let (s, anim) = if high {
        (EnemyState::HighPunch, HIGH_PUNCH_ANIM)
    } else {
        (EnemyState::LowPunch, LOW_PUNCH_ANIM)
    };
    set_state(w, id, s);
    set_anim(w, id, anim);
    if let Some(d) = aquatis_mut(w, id) {
        d.first_done = false;
    }
}

/// HighPunch and LowPunch (see the module documentation).
pub fn punch_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    let high = super::state(w, id) == Some(EnemyState::HighPunch);
    if first_call(w, id) {
        play(
            w,
            if high {
                HIGH_PUNCH_SOUND
            } else {
                LOW_PUNCH_SOUND
            },
        );
    }
    let (frame, punch) = if high {
        (HIGH_PUNCH_FRAME, HIGH_PUNCH_BOX)
    } else {
        (LOW_PUNCH_FRAME, LOW_PUNCH_BOX)
    };
    let o = obj_mut(w, id);
    if o.frame == frame {
        o.attack_rect = facing_rect(o, punch);
    }
    if obj(w, id).anim.is_done() {
        if w.aquatis_link().frenzy {
            set_state(w, id, EnemyState::SuperFrenzy);
        } else {
            go_idle(w, id);
        }
    }
}

/// True on the first call of a state: the flag is then set.
fn first_call(w: &mut World, id: ObjectId) -> bool {
    match aquatis_mut(w, id) {
        Some(d) if !d.first_done => {
            d.first_done = true;
            true
        }
        _ => false,
    }
}

/// Cowering (see the module documentation).
pub fn enter_scared(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::Scared);
    set_anim(w, id, SCARED_ANIM);
    let now = w.now_ms;
    if let Some(d) = aquatis_mut(w, id) {
        d.first_done = false;
        d.scared = Timer::new(now, SCARED_MS);
    }
}

/// Scared (see the module documentation).
pub fn scared_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if first_call(w, id) {
        say(w, id, SCARED_LINE);
    }
    let now = w.now_ms;
    let over = aquatis_data(w, id).is_some_and(|d| d.scared.done(now));
    let link = *w.aquatis_link();
    if over && !link.quake && !link.falling {
        go_idle(w, id);
    }
}

/// A stalactite's hit (see the module documentation).
pub fn enter_stalactite_hit(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::StalactiteHit);
    face_claw(w, id);
    set_anim(w, id, IDLE2_ANIM);
    for s in STALACTITE_SOUNDS {
        play(w, s);
    }
    let count = match aquatis_mut(w, id) {
        Some(d) => {
            d.first_done = false;
            d.stalactites += 1;
            d.stalactites
        }
        None => 0,
    };
    let o = obj_mut(w, id);
    o.health -= STALACTITE_DAMAGE;
    if count >= STALACTITES {
        o.health = 0;
        enter_death(w, id);
    }
}

/// StalactiteHit (see the module documentation).
pub fn stalactite_hit_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if first_call(w, id) {
        say(w, id, STALACTITE_LINE);
    }
    if obj(w, id).anim.is_done() {
        enter_frenzy(w, id);
    }
}

/// His frenzy (see the module documentation).
pub fn enter_frenzy(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::SuperFrenzy);
    set_anim(w, id, HIGH_PUNCH_ANIM);
    w.aquatis_link_mut().frenzy = true;
    let now = w.now_ms;
    if let Some(d) = aquatis_mut(w, id) {
        d.first_done = false;
        d.frenzy = Timer::new(now, u64::from(FRENZY_MS));
    }
}

/// SuperFrenzy (see the module documentation).
pub fn frenzy_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if first_call(w, id) {
        say(w, id, FRENZY_LINE);
    }
    let now = w.now_ms;
    if aquatis_data(w, id).is_some_and(|d| d.frenzy.done(now)) {
        w.aquatis_link_mut().frenzy = false;
        go_idle(w, id);
        return;
    }
    let link = w.aquatis_link_mut();
    let high = link.frenzy_toggle == 0;
    link.frenzy_toggle = u32::from(high);
    enter_punch(w, id, high);
}

/// The flinch's end (see the module documentation). Draws: one of 0..=100
/// unless it is the third, then the attack's.
pub fn flinch_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if !obj(w, id).anim.is_done() {
        return;
    }
    if let Some(d) = data_mut(w, id) {
        d.busy = false;
    }
    let third = match aquatis_mut(w, id) {
        Some(d) => {
            d.flinches += 1;
            if d.flinches >= FLINCHES {
                d.flinches = 0;
                true
            } else {
                false
            }
        }
        None => false,
    };
    if third {
        enter_scared(w, id);
    } else if rand(w, 101) < FLINCH_ATTACK_CHANCE {
        attack(w, id);
    } else {
        go_idle(w, id);
    }
}

/// His death (see the module documentation).
pub fn enter_death(w: &mut World, id: ObjectId) {
    set_state(w, id, EnemyState::DeadAct1);
    set_anim(w, id, DEATH_ANIM);
    if let Some(d) = data_mut(w, id) {
        d.contact_rect = Rect::UNSET;
        d.strike_rect = Rect::UNSET;
    }
    let o = obj_mut(w, id);
    o.flags |= flags::HARMLESS;
    o.attack_rect = Rect::UNSET;
    if let Some(d) = aquatis_mut(w, id) {
        d.first_done = false;
        d.death_done = false;
    }
}

/// DeadAct1 (see the module documentation).
pub fn death_tick(w: &mut World, id: ObjectId) {
    w.advance_anim(id);
    if first_call(w, id) {
        say(w, id, DEATH_LINE);
    }
    let end = obj(w, id).anim.is_done() && aquatis_data(w, id).is_some_and(|d| !d.death_done);
    if !end {
        return;
    }
    if let Some(d) = aquatis_mut(w, id) {
        d.death_done = true;
    }
    if w.claw.stats.health <= 0 {
        return;
    }
    release_gem(w, id);
    sound::enemy_silence(w, id);
    w.aquatis_link_mut().alive = false;
    if let Some(d) = data_mut(w, id) {
        d.dead = true;
    }
}

// ----------------------------------------------------------------------
// Hits

/// `attacker` hit `id` (see the module documentation). Draws: a flinch's
/// (one of 0..4 for a high one), then the hit burst's.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let link = *w.aquatis_link();
    if w.aquatis() != Some(id) || !link.alive || link.frenzy {
        return;
    }
    let state = super::state(w, id);
    if matches!(
        state,
        Some(
            EnemyState::SuperFrenzy
                | EnemyState::StalactiteHit
                | EnemyState::Dead
                | EnemyState::HitHigh
                | EnemyState::HitLow
                | EnemyState::DeadAct1
        )
    ) {
        return;
    }
    let now = w.now_ms;
    if aquatis_data(w, id).is_some_and(|d| !d.hit_cooldown.done(now)) {
        return;
    }
    let Some(kind) = w.objects.get(attacker).map(|a| a.type_flags) else {
        return;
    };
    if kind == types::ENEMY {
        stalactite_hit(w, id, attacker);
    } else if state != Some(EnemyState::Scared) {
        if let Some(d) = aquatis_mut(w, id) {
            d.hit_cooldown = Timer::new(now, HIT_COOLDOWN_MS);
        }
        let high = w.claw.attack.uppercut;
        super::states::enter_hit(w, id, high);
        if let Some(d) = data_mut(w, id) {
            d.vx = 0.0;
            d.frozen = true;
        }
        hit_burst(w, id, attacker);
    }
    if kind == types::PISTOL_SHOT {
        w.destroy(attacker);
    }
}

/// The body of the enemies' kind `attacker` (a falling stalactite of
/// either family) hit `id`, past the gates of [`on_hit`]: the stalactite
/// cooldown, the stalactite's hit and a hit burst.
pub fn stalactite_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    let now = w.now_ms;
    if let Some(d) = aquatis_mut(w, id) {
        d.hit_cooldown = Timer::new(now, STALACTITE_COOLDOWN_MS);
    }
    enter_stalactite_hit(w, id);
    hit_burst(w, id, attacker);
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    super::on_attack(w, id, victim);
}

/// His own sounds and lines, for tests and tools that check them against
/// the level.
pub fn sound_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = IDLE_LINES.to_vec();
    v.extend([
        HIGH_PUNCH_SOUND,
        LOW_PUNCH_SOUND,
        FRENZY_LINE,
        SCARED_LINE,
        DEATH_LINE,
        STALACTITE_LINE,
    ]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn his_states_open_the_twelfth_levels_block() {
        assert_eq!((*STATE_BLOCK.start(), *STATE_BLOCK.end()), (2280, 2299));
        assert_eq!(STATES[0] as i32, *STATE_BLOCK.start());
        for s in STATES {
            assert!(STATE_BLOCK.contains(&(s as i32)), "{}", s.name());
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert_eq!(
            *super::super::mercat::STATE_BLOCK.end() + 1,
            *STATE_BLOCK.start()
        );
        assert!(CLASS.human, "a human enemy");
        assert!(CLASS.carry.is_some(), "liftable");
        for s in [
            EnemyState::Startup,
            EnemyState::Idle,
            EnemyState::HitHigh,
            EnemyState::HitLow,
            EnemyState::DeadAct1,
        ] {
            assert!(handler(s).is_some(), "{}", s.name());
        }
        assert!(handler(EnemyState::Lifted).is_none(), "the shared lift");
    }

    #[test]
    fn five_stalactites_empty_his_health() {
        assert_eq!(STALACTITES, 5);
        assert_eq!(STALACTITE_BITS.len() as i32, STALACTITES);
        // The nine reset bits: the crack, three tentacles, five stalactites.
        let all = STALACTITE_BITS
            .iter()
            .chain(&TENTACLE_BITS)
            .fold(CRACK_BIT, |a, b| a | b);
        assert_eq!(all, RESET_BITS);
        assert_eq!(RESET_BITS.count_ones(), 9);
        // The quake and the frenzy each last 223 ticks at 18 ms.
        assert_eq!(QUAKE_MS.div_ceil(18), 223);
        assert_eq!(FRENZY_MS.div_ceil(18), 223);
    }

    #[test]
    fn the_link_starts_empty_with_the_order_at_one() {
        let l = AquatisLink::default();
        assert_eq!(l.aquatis, None);
        assert!(!l.alive && !l.frenzy && !l.quake && !l.armed && !l.falling);
        assert!(!l.refill_outstanding && !l.tentacle_grab);
        assert_eq!((l.reset_bits, l.frenzy_toggle, l.land_y), (0, 0, 0));
        assert_eq!(l.stalactite_order, 1);
    }

    #[test]
    fn his_art_is_his_own() {
        for anim in [
            IDLE_ANIM,
            IDLE2_ANIM,
            HIGH_PUNCH_ANIM,
            LOW_PUNCH_ANIM,
            SCARED_ANIM,
            DEATH_ANIM,
            HIT_HIGH_ANIM,
            HIT_LOW_ANIM,
        ] {
            assert!(anim.starts_with(ANIM_PREFIXES[0]), "{anim}");
        }
        for s in sound_names() {
            assert!(s.starts_with(ANIM_PREFIXES[0]), "{s}");
        }
    }

    #[test]
    fn his_punches_reach_left_of_him() {
        const { assert!(HIGH_PUNCH_BOX.right <= 0 && LOW_PUNCH_BOX.right <= 0) };
        assert_eq!(RANGES[2].0, REACH);
        const { assert!(RANGES[0].0 > 2000, "his room") };
    }
}
