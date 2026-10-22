//! Level logics: what enemies, shots, effects, checkpoints, platforms,
//! decoration, crates, kegs, pickups and sound objects do.
//!
//! Each family lives in its own file and declares:
//!
//! - `NAMES`: the logic names it runs;
//! - `IMAGE_SETS` and `ANIM_PREFIXES`: the image sets it may switch an
//!   object to and the animations it may play, so the level loads them up
//!   front and nothing is read or decoded while drawing;
//! - `tick(w, id)`: one logic call, made once per update pass; the engine
//!   also calls it with the state set to [`engine_state::DESTROYING`]
//!   before destroying an object flagged [`flags::NOTIFY_DESTROY`], and
//!   with [`engine_state::SLEEPING`] when an object flagged
//!   [`flags::NOTIFY_SLEEP`] goes dormant; an object whose health ran out
//!   through automatic hit damage, or that another logic killed with
//!   [`World::kill`], sees [`engine_state::KILLED`] on its next call (with
//!   the kind of kill in [`Object::kill_kind`]);
//! - `on_hit(w, id, attacker)` and `on_attack(w, id, victim)`: called by
//!   the pair-collision pass, in the order the pairs are tested, for
//!   objects with [`Object::hit_handler`] or [`Object::attack_handler`];
//! - its own data type, held in [`Object::data`] through [`ObjectData`].
//!
//! [`kind_for`] maps a logic name to its family. A family whose logics are
//! not written yet declares `BUILT = false` ([`built`]): the registry binds
//! a name only to a built family ([`runs`]), so until then its names stay
//! inert placeholders as unclaimed names do (drawn, no collision, and
//! reported by the level load as not implemented). [`tick`], [`on_hit`],
//! [`on_attack`] and the lift hooks dispatch a call to the object's
//! family.
//!
//! [`World`] is the context of every call. Besides the objects and Claw
//! it gives the level's number, Claw's attempt counter, the frame ranges
//! and frame boxes of the image sets, the screen shake request, the
//! sound-only random stream and the boss fight state ([`world_boss`]). A
//! family adds helpers of its own in an `impl World` block in its own
//! file.
//!
//! Families that need another family's objects go through the spawn
//! functions [`projectile::spawn`], [`effect::spawn`] and
//! [`pickup::spawn_drop`]. Until the owning family fills them in, they
//! spawn an inert placeholder whose data records the request.
//!
//! [`engine_state::DESTROYING`]: crate::object::engine_state::DESTROYING
//! [`engine_state::SLEEPING`]: crate::object::engine_state::SLEEPING
//! [`engine_state::KILLED`]: crate::object::engine_state::KILLED
//! [`flags::NOTIFY_DESTROY`]: crate::object::flags::NOTIFY_DESTROY
//! [`flags::NOTIFY_SLEEP`]: crate::object::flags::NOTIFY_SLEEP

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::level::SimLevel;
use crate::object::{LogicKind, Object, ObjectId, flags};
use crate::registry::{REG_ALWAYS, REG_CLASS_1, REG_CLASS_2, Registry};

pub mod boss;
pub mod boss_stage;
pub mod break_plank;
pub mod cannon;
pub mod cannon_switch;
pub mod checkpoint;
pub mod conveyor_belt;
pub mod crates;
pub mod dart_trap;
pub mod decor;
pub mod effect;
pub mod enemy;
pub mod falling;
pub mod floor_spike;
pub mod goo_vent;
pub mod ground_blower;
pub mod jump_switch;
pub mod keg;
pub mod laser;
pub mod lava;
pub mod marrow_floor;
pub mod path_elevator;
pub mod pickup;
pub mod platform;
pub mod projectile;
pub mod rope;
pub mod saw_blade;
pub mod shake;
pub mod skull_cannon;
pub mod sound;
pub mod sound_object;
pub mod springboard;
pub mod stalactite;
pub mod statue;
pub mod stepping_stone;
pub mod world;
pub mod world_boss;

pub use world::{Outbox, World};
pub use world_boss::BossState;

/// The level logic families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GameLogic {
    /// The officer enemy.
    Officer,
    /// The soldier enemy.
    Soldier,
    /// The rat enemy.
    Rat,
    /// Shots, thrown dynamite and other flying objects.
    Projectile,
    /// Hit bursts, splashes and explosions.
    Effect,
    /// Checkpoint flags and save points.
    Checkpoint,
    /// Elevators and pegs.
    Platform,
    /// Decoration.
    Decor,
    /// Crates.
    Crate,
    /// Powder kegs.
    Keg,
    /// Pickups.
    Pickup,
    /// Sound triggers and ambient sounds.
    SoundObject,
    /// The punk rat, the rat that rides a small cannon.
    PunkRat,
    /// Cannons: the tower cannons and the punk rat's ground cannon.
    Cannon,
    /// Raux, the boss of the second level.
    Raux,
    /// The boss fight's stage: the warp into the boss's room, the
    /// introduction and the boss's health bar.
    BossStage,
    /// The cutthroat, the knife-throwing thief.
    CutThroat,
    /// The robber thief, the archer.
    RobberThief,
    /// Floor spikes that rise and sink on the level clock.
    FloorSpike,
    /// Platforms that follow a closed route on the level clock.
    PathElevator,
    /// Swinging ropes.
    Rope,
    /// Springboards that throw Claw or an enemy into the air.
    SpringBoard,
    /// Stepping stones that sink on the level clock or under Claw.
    SteppingStone,
    /// Katherine, the boss of the fourth level.
    Katherine,
    /// The first town guard, with a sword and a truncheon.
    TownGuard1,
    /// The second, heavier town guard, who parries.
    TownGuard2,
    /// The seagull, the flying enemy that dives at Claw.
    Seagull,
    /// Rows of planks that give way under Claw.
    BreakPlank,
    /// Invisible markers that launch an enemy to a chosen point.
    JumpSwitch,
    /// Breakable statues that release a fan of treasure.
    Statue,
    /// Goo vents that erupt in a harmful splash.
    GooVent,
    /// Steam vents in the floor that throw Claw straight up.
    GroundBlower,
    /// Wolvington, the boss of the sixth level.
    Wolvington,
    /// Wolvington's magic shot.
    WolvingtonShot,
    /// The light a magic shot of Wolvington's carries.
    WolvingtonLight,
    /// The red-tail pirate, a sword pirate that hops back from Claw's
    /// blows.
    RedTailPirate,
    /// The bear sailor, who grabs Claw in a hug.
    BearSailor,
    /// The hermit crab, which hides in its shell, snips and throws bombs.
    HermitCrab,
    /// Sand nests that hatch hermit crabs when Claw walks onto them.
    CrabNest,
    /// The bomb a bomber crab throws.
    CrabBomb,
    /// Gabriel, the boss of the eighth level.
    Gabriel,
    /// The bombs Gabriel lobs.
    GabrielBomb,
    /// The lever beside Gabriel that flips when he orders his cannon to
    /// fire.
    CannonSwitch,
    /// The button Claw strikes to turn Gabriel's cannon upwards.
    CannonButton,
    /// Gabriel's deck cannon.
    GabrielCannon,
    /// The hook-handed pirate, who holds his hook out and parries.
    HookPirate,
    /// PegLeg, the rolling gunner.
    PegLeg,
    /// Dart traps that shoot arrows while Claw stands in their rectangle.
    DartTrap,
    /// Conveyor belt pieces that push Claw along.
    ConveyorBelt,
    /// Saw blades that rise and sink on the level clock.
    SawBlade,
    /// Skull cannons that fire a cannonball every three seconds.
    SkullCannon,
    /// Marrow, the boss of the tenth level.
    Marrow,
    /// Marrow's parrot, which swoops at Claw and carries Marrow.
    Parrot,
    /// The two slabs that bridge the spike pit of Marrow's room.
    MarrowFloor,
    /// Mercat, the merman who throws trident bolts.
    Mercat,
    /// The Siren, whose scream locks Claw's controls.
    Siren,
    /// The pond fish that leap at Claw from the wading pools.
    Fish,
    /// The electric traps across the eleventh level's corridors.
    Laser,
    /// Mercat's trident bolt.
    TridentBullet,
    /// The Siren's scream.
    SirenBullet,
    /// Aquatis, the boss of the twelfth level.
    Aquatis,
    /// The crack in the wall behind Aquatis that a blast sets off.
    AquatisCrack,
    /// The hidden helper that puts a stick of dynamite back in Aquatis's
    /// room.
    AquatisDynamite,
    /// The five stalactites over Aquatis that fall on him one by one.
    AquatisStalactite,
    /// Aquatis's tentacles, which strike at Claw and grab him.
    Tentacle,
    /// The falling stalactites of the twelfth level's caves.
    Stalactite,
    /// The invisible earthquake triggers of the twelfth level.
    Shake,
    /// The Chameleon, the lizard that fades out and back in elsewhere on
    /// its ledge.
    Chameleon,
    /// RedTail, the boss of the thirteenth level.
    RedTail,
    /// The stakes at the left wall of RedTail's arena.
    RedTailSpikes,
    /// The wind RedTail raises across his arena.
    RedTailWind,
    /// RedTail's knives.
    RedTailKnife,
    /// RedTail's flares.
    RedTailBullet,
    /// The grass, leaves and sand RedTail's wind blows across the arena.
    WindDebris,
    /// The tiger guards of the fourteenth level.
    TigerGuard,
    /// The lava fountains of the fourteenth level.
    LavaGeyser,
    /// The stone mouths that drip lava from the fourteenth level's
    /// ceilings.
    LavaMouth,
    /// The hands that rise out of the fourteenth level's lava and throw
    /// lava balls.
    LavaHand,
    /// A lava hand's lava ball.
    LavaHandProjectile,
    /// Omar, the boss of the fourteenth level.
    Omar,
    /// The pieces of Omar's ring of fire or ice.
    OmarShield,
    /// Omar's energy balls.
    OmarBullet,
}

/// Every family, in a fixed order.
pub const ALL: [GameLogic; 82] = [
    GameLogic::Officer,
    GameLogic::Soldier,
    GameLogic::Rat,
    GameLogic::Projectile,
    GameLogic::Effect,
    GameLogic::Checkpoint,
    GameLogic::Platform,
    GameLogic::Decor,
    GameLogic::Crate,
    GameLogic::Keg,
    GameLogic::Pickup,
    GameLogic::SoundObject,
    GameLogic::PunkRat,
    GameLogic::Cannon,
    GameLogic::Raux,
    GameLogic::BossStage,
    GameLogic::CutThroat,
    GameLogic::RobberThief,
    GameLogic::FloorSpike,
    GameLogic::PathElevator,
    GameLogic::Rope,
    GameLogic::SpringBoard,
    GameLogic::SteppingStone,
    GameLogic::Katherine,
    GameLogic::TownGuard1,
    GameLogic::TownGuard2,
    GameLogic::Seagull,
    GameLogic::BreakPlank,
    GameLogic::JumpSwitch,
    GameLogic::Statue,
    GameLogic::GooVent,
    GameLogic::GroundBlower,
    GameLogic::Wolvington,
    GameLogic::WolvingtonShot,
    GameLogic::WolvingtonLight,
    GameLogic::RedTailPirate,
    GameLogic::BearSailor,
    GameLogic::HermitCrab,
    GameLogic::CrabNest,
    GameLogic::CrabBomb,
    GameLogic::Gabriel,
    GameLogic::GabrielBomb,
    GameLogic::CannonSwitch,
    GameLogic::CannonButton,
    GameLogic::GabrielCannon,
    GameLogic::HookPirate,
    GameLogic::PegLeg,
    GameLogic::DartTrap,
    GameLogic::ConveyorBelt,
    GameLogic::SawBlade,
    GameLogic::SkullCannon,
    GameLogic::Marrow,
    GameLogic::Parrot,
    GameLogic::MarrowFloor,
    GameLogic::Mercat,
    GameLogic::Siren,
    GameLogic::Fish,
    GameLogic::Laser,
    GameLogic::TridentBullet,
    GameLogic::SirenBullet,
    GameLogic::Aquatis,
    GameLogic::AquatisCrack,
    GameLogic::AquatisDynamite,
    GameLogic::AquatisStalactite,
    GameLogic::Tentacle,
    GameLogic::Stalactite,
    GameLogic::Shake,
    GameLogic::Chameleon,
    GameLogic::RedTail,
    GameLogic::RedTailSpikes,
    GameLogic::RedTailWind,
    GameLogic::RedTailKnife,
    GameLogic::RedTailBullet,
    GameLogic::WindDebris,
    GameLogic::TigerGuard,
    GameLogic::LavaGeyser,
    GameLogic::LavaMouth,
    GameLogic::LavaHand,
    GameLogic::LavaHandProjectile,
    GameLogic::Omar,
    GameLogic::OmarShield,
    GameLogic::OmarBullet,
];

/// A logic's own data, by family.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ObjectData {
    /// No data.
    #[default]
    None,
    /// An enemy.
    Enemy(enemy::EnemyData),
    /// A projectile.
    Projectile(projectile::ProjectileData),
    /// An effect.
    Effect(effect::EffectData),
    /// A checkpoint.
    Checkpoint(checkpoint::CheckpointData),
    /// A platform.
    Platform(platform::PlatformData),
    /// Decoration.
    Decor(decor::DecorData),
    /// A crate.
    Crate(crates::CrateData),
    /// A powder keg.
    Keg(keg::KegData),
    /// A pickup.
    Pickup(pickup::PickupData),
    /// A sound object.
    Sound(sound_object::SoundData),
    /// A punk rat.
    PunkRat(enemy::punkrat::PunkRatData),
    /// A cannon.
    Cannon(cannon::CannonData),
    /// Raux.
    Raux(enemy::raux::RauxData),
    /// A piece of a boss fight's stage.
    BossStage(boss_stage::BossStageData),
    /// A cutthroat.
    CutThroat(enemy::cutthroat::CutThroatData),
    /// A robber thief.
    RobberThief(enemy::robberthief::RobberThiefData),
    /// A floor spike.
    FloorSpike(floor_spike::FloorSpikeData),
    /// A path elevator.
    PathElevator(path_elevator::PathElevatorData),
    /// A rope.
    Rope(rope::RopeData),
    /// A springboard.
    SpringBoard(springboard::SpringBoardData),
    /// A stepping stone.
    SteppingStone(stepping_stone::SteppingStoneData),
    /// Katherine.
    Katherine(enemy::katherine::KatherineData),
    /// A town guard, either one.
    TownGuard(enemy::townguard::TownGuardData),
    /// A seagull.
    Seagull(enemy::seagull::SeagullData),
    /// A plank row's spawner or one of its pieces.
    BreakPlank(break_plank::BreakPlankData),
    /// A jump switch.
    JumpSwitch(jump_switch::JumpSwitchData),
    /// A statue.
    Statue(statue::StatueData),
    /// A goo vent.
    GooVent(goo_vent::GooVentData),
    /// A ground blower.
    GroundBlower(ground_blower::GroundBlowerData),
    /// Wolvington.
    Wolvington(enemy::wolvington::WolvingtonData),
    /// A magic shot of Wolvington's.
    WolvingtonShot(enemy::wolvington::shot::ShotData),
    /// The light of a magic shot of Wolvington's.
    WolvingtonLight(enemy::wolvington::light::LightData),
    /// A red-tail pirate.
    RedTailPirate(enemy::redtail_pirate::RedTailPirateData),
    /// A bear sailor.
    BearSailor(enemy::bear_sailor::BearSailorData),
    /// A hermit crab.
    HermitCrab(enemy::crab::HermitCrabData),
    /// A crab nest.
    CrabNest(enemy::crab::nest::NestData),
    /// A crab bomb.
    CrabBomb(enemy::crab::bomb::BombData),
    /// Gabriel.
    Gabriel(enemy::gabriel::GabrielData),
    /// One of Gabriel's bombs.
    GabrielBomb(enemy::gabriel::bomb::BombData),
    /// The cannon switch.
    CannonSwitch(cannon_switch::SwitchData),
    /// The cannon button.
    CannonButton(cannon_switch::button::ButtonData),
    /// Gabriel's cannon.
    GabrielCannon(cannon_switch::cannon::CannonData),
    /// A hook-handed pirate.
    HookPirate(enemy::hook_pirate::HookPirateData),
    /// A PegLeg.
    PegLeg(enemy::pegleg::PegLegData),
    /// A dart trap.
    DartTrap(dart_trap::DartTrapData),
    /// A conveyor belt piece.
    ConveyorBelt(conveyor_belt::BeltData),
    /// A saw blade.
    SawBlade(saw_blade::SawData),
    /// A skull cannon.
    SkullCannon(skull_cannon::SkullCannonData),
    /// Marrow.
    Marrow(enemy::marrow::MarrowData),
    /// Marrow's parrot.
    Parrot(enemy::parrot::ParrotData),
    /// A slab of Marrow's trap floor.
    MarrowFloor(marrow_floor::FloorData),
    /// A Mercat.
    Mercat(enemy::mercat::MercatData),
    /// A Siren.
    Siren(enemy::siren::SirenData),
    /// A fish.
    Fish(enemy::fish::FishData),
    /// A laser.
    Laser(laser::LaserData),
    /// Aquatis.
    Aquatis(enemy::aquatis::AquatisData),
    /// The crack behind Aquatis.
    AquatisCrack(enemy::aquatis::crack::CrackData),
    /// The dynamite helper of Aquatis's room.
    AquatisDynamite(enemy::aquatis::dynamite::DynamiteData),
    /// One of the five stalactites over Aquatis.
    AquatisStalactite(enemy::aquatis::stalactite::StalactiteData),
    /// A tentacle.
    Tentacle(enemy::tentacle::TentacleData),
    /// A falling stalactite of the caves.
    Stalactite(stalactite::StalactiteData),
    /// An earthquake trigger.
    Shake(shake::ShakeData),
    /// A Chameleon.
    Chameleon(enemy::chameleon::ChameleonData),
    /// RedTail.
    RedTail(enemy::redtail::RedTailData),
    /// The stakes of RedTail's arena.
    RedTailSpikes(boss::spikes::SpikesData),
    /// RedTail's wind.
    RedTailWind(boss::wind::WindData),
    /// One of RedTail's knives.
    RedTailKnife(boss::knife::KnifeData),
    /// One of RedTail's flares.
    RedTailBullet(boss::flare::FlareData),
    /// A piece of the wind's debris.
    WindDebris(boss::debris::DebrisData),
    /// A tiger guard.
    TigerGuard(enemy::tiger_guard::TigerGuardData),
    /// A lava fountain.
    LavaGeyser(lava::geyser::GeyserData),
    /// A lava mouth.
    LavaMouth(lava::mouth::MouthData),
    /// A lava hand.
    LavaHand(lava::hand::HandData),
    /// A lava hand's lava ball.
    LavaHandProjectile(lava::ball::BallData),
    /// Omar.
    Omar(enemy::omar::OmarData),
    /// A piece of Omar's ring.
    OmarShield(enemy::omar::shield::ShieldData),
    /// One of Omar's energy balls.
    OmarBullet(enemy::omar::bullet::BulletData),
}

/// Calls `$f($args)` in the module of family `$g`.
macro_rules! dispatch {
    ($g:expr, $f:ident ( $($arg:expr),* )) => {
        match $g {
            GameLogic::Officer => enemy::officer::$f($($arg),*),
            GameLogic::Soldier => enemy::soldier::$f($($arg),*),
            GameLogic::Rat => enemy::rat::$f($($arg),*),
            GameLogic::Projectile => projectile::$f($($arg),*),
            GameLogic::Effect => effect::$f($($arg),*),
            GameLogic::Checkpoint => checkpoint::$f($($arg),*),
            GameLogic::Platform => platform::$f($($arg),*),
            GameLogic::Decor => decor::$f($($arg),*),
            GameLogic::Crate => crates::$f($($arg),*),
            GameLogic::Keg => keg::$f($($arg),*),
            GameLogic::Pickup => pickup::$f($($arg),*),
            GameLogic::SoundObject => sound_object::$f($($arg),*),
            GameLogic::PunkRat => enemy::punkrat::$f($($arg),*),
            GameLogic::Cannon => cannon::$f($($arg),*),
            GameLogic::Raux => enemy::raux::$f($($arg),*),
            GameLogic::BossStage => boss_stage::$f($($arg),*),
            GameLogic::CutThroat => enemy::cutthroat::$f($($arg),*),
            GameLogic::RobberThief => enemy::robberthief::$f($($arg),*),
            GameLogic::FloorSpike => floor_spike::$f($($arg),*),
            GameLogic::PathElevator => path_elevator::$f($($arg),*),
            GameLogic::Rope => rope::$f($($arg),*),
            GameLogic::SpringBoard => springboard::$f($($arg),*),
            GameLogic::SteppingStone => stepping_stone::$f($($arg),*),
            GameLogic::Katherine => enemy::katherine::$f($($arg),*),
            GameLogic::TownGuard1 => enemy::townguard::guard1::$f($($arg),*),
            GameLogic::TownGuard2 => enemy::townguard::guard2::$f($($arg),*),
            GameLogic::Seagull => enemy::seagull::$f($($arg),*),
            GameLogic::BreakPlank => break_plank::$f($($arg),*),
            GameLogic::JumpSwitch => jump_switch::$f($($arg),*),
            GameLogic::Statue => statue::$f($($arg),*),
            GameLogic::GooVent => goo_vent::$f($($arg),*),
            GameLogic::GroundBlower => ground_blower::$f($($arg),*),
            GameLogic::Wolvington => enemy::wolvington::$f($($arg),*),
            GameLogic::WolvingtonShot => enemy::wolvington::shot::$f($($arg),*),
            GameLogic::WolvingtonLight => enemy::wolvington::light::$f($($arg),*),
            GameLogic::RedTailPirate => enemy::redtail_pirate::$f($($arg),*),
            GameLogic::BearSailor => enemy::bear_sailor::$f($($arg),*),
            GameLogic::HermitCrab => enemy::crab::$f($($arg),*),
            GameLogic::CrabNest => enemy::crab::nest::$f($($arg),*),
            GameLogic::CrabBomb => enemy::crab::bomb::$f($($arg),*),
            GameLogic::Gabriel => enemy::gabriel::$f($($arg),*),
            GameLogic::GabrielBomb => enemy::gabriel::bomb::$f($($arg),*),
            GameLogic::CannonSwitch => cannon_switch::$f($($arg),*),
            GameLogic::CannonButton => cannon_switch::button::$f($($arg),*),
            GameLogic::GabrielCannon => cannon_switch::cannon::$f($($arg),*),
            GameLogic::HookPirate => enemy::hook_pirate::$f($($arg),*),
            GameLogic::PegLeg => enemy::pegleg::$f($($arg),*),
            GameLogic::DartTrap => dart_trap::$f($($arg),*),
            GameLogic::ConveyorBelt => conveyor_belt::$f($($arg),*),
            GameLogic::SawBlade => saw_blade::$f($($arg),*),
            GameLogic::SkullCannon => skull_cannon::$f($($arg),*),
            GameLogic::Marrow => enemy::marrow::$f($($arg),*),
            GameLogic::Parrot => enemy::parrot::$f($($arg),*),
            GameLogic::MarrowFloor => marrow_floor::$f($($arg),*),
            GameLogic::Mercat => enemy::mercat::$f($($arg),*),
            GameLogic::Siren => enemy::siren::$f($($arg),*),
            GameLogic::Fish => enemy::fish::$f($($arg),*),
            GameLogic::Laser => laser::$f($($arg),*),
            GameLogic::TridentBullet => projectile::trident::$f($($arg),*),
            GameLogic::SirenBullet => projectile::scream::$f($($arg),*),
            GameLogic::Aquatis => enemy::aquatis::$f($($arg),*),
            GameLogic::AquatisCrack => enemy::aquatis::crack::$f($($arg),*),
            GameLogic::AquatisDynamite => enemy::aquatis::dynamite::$f($($arg),*),
            GameLogic::AquatisStalactite => enemy::aquatis::stalactite::$f($($arg),*),
            GameLogic::Tentacle => enemy::tentacle::$f($($arg),*),
            GameLogic::Stalactite => stalactite::$f($($arg),*),
            GameLogic::Shake => shake::$f($($arg),*),
            GameLogic::Chameleon => enemy::chameleon::$f($($arg),*),
            GameLogic::RedTail => enemy::redtail::$f($($arg),*),
            GameLogic::RedTailSpikes => boss::spikes::$f($($arg),*),
            GameLogic::RedTailWind => boss::wind::$f($($arg),*),
            GameLogic::RedTailKnife => boss::knife::$f($($arg),*),
            GameLogic::RedTailBullet => boss::flare::$f($($arg),*),
            GameLogic::WindDebris => boss::debris::$f($($arg),*),
            GameLogic::TigerGuard => enemy::tiger_guard::$f($($arg),*),
            GameLogic::LavaGeyser => lava::geyser::$f($($arg),*),
            GameLogic::LavaMouth => lava::mouth::$f($($arg),*),
            GameLogic::LavaHand => lava::hand::$f($($arg),*),
            GameLogic::LavaHandProjectile => lava::ball::$f($($arg),*),
            GameLogic::Omar => enemy::omar::$f($($arg),*),
            GameLogic::OmarShield => enemy::omar::shield::$f($($arg),*),
            GameLogic::OmarBullet => enemy::omar::bullet::$f($($arg),*),
        }
    };
}

/// The constants `$c` of family `$g`.
macro_rules! constant {
    ($g:expr, $c:ident) => {
        match $g {
            GameLogic::Officer => enemy::officer::$c,
            GameLogic::Soldier => enemy::soldier::$c,
            GameLogic::Rat => enemy::rat::$c,
            GameLogic::Projectile => projectile::$c,
            GameLogic::Effect => effect::$c,
            GameLogic::Checkpoint => checkpoint::$c,
            GameLogic::Platform => platform::$c,
            GameLogic::Decor => decor::$c,
            GameLogic::Crate => crates::$c,
            GameLogic::Keg => keg::$c,
            GameLogic::Pickup => pickup::$c,
            GameLogic::SoundObject => sound_object::$c,
            GameLogic::PunkRat => enemy::punkrat::$c,
            GameLogic::Cannon => cannon::$c,
            GameLogic::Raux => enemy::raux::$c,
            GameLogic::BossStage => boss_stage::$c,
            GameLogic::CutThroat => enemy::cutthroat::$c,
            GameLogic::RobberThief => enemy::robberthief::$c,
            GameLogic::FloorSpike => floor_spike::$c,
            GameLogic::PathElevator => path_elevator::$c,
            GameLogic::Rope => rope::$c,
            GameLogic::SpringBoard => springboard::$c,
            GameLogic::SteppingStone => stepping_stone::$c,
            GameLogic::Katherine => enemy::katherine::$c,
            GameLogic::TownGuard1 => enemy::townguard::guard1::$c,
            GameLogic::TownGuard2 => enemy::townguard::guard2::$c,
            GameLogic::Seagull => enemy::seagull::$c,
            GameLogic::BreakPlank => break_plank::$c,
            GameLogic::JumpSwitch => jump_switch::$c,
            GameLogic::Statue => statue::$c,
            GameLogic::GooVent => goo_vent::$c,
            GameLogic::GroundBlower => ground_blower::$c,
            GameLogic::Wolvington => enemy::wolvington::$c,
            GameLogic::WolvingtonShot => enemy::wolvington::shot::$c,
            GameLogic::WolvingtonLight => enemy::wolvington::light::$c,
            GameLogic::RedTailPirate => enemy::redtail_pirate::$c,
            GameLogic::BearSailor => enemy::bear_sailor::$c,
            GameLogic::HermitCrab => enemy::crab::$c,
            GameLogic::CrabNest => enemy::crab::nest::$c,
            GameLogic::CrabBomb => enemy::crab::bomb::$c,
            GameLogic::Gabriel => enemy::gabriel::$c,
            GameLogic::GabrielBomb => enemy::gabriel::bomb::$c,
            GameLogic::CannonSwitch => cannon_switch::$c,
            GameLogic::CannonButton => cannon_switch::button::$c,
            GameLogic::GabrielCannon => cannon_switch::cannon::$c,
            GameLogic::HookPirate => enemy::hook_pirate::$c,
            GameLogic::PegLeg => enemy::pegleg::$c,
            GameLogic::DartTrap => dart_trap::$c,
            GameLogic::ConveyorBelt => conveyor_belt::$c,
            GameLogic::SawBlade => saw_blade::$c,
            GameLogic::SkullCannon => skull_cannon::$c,
            GameLogic::Marrow => enemy::marrow::$c,
            GameLogic::Parrot => enemy::parrot::$c,
            GameLogic::MarrowFloor => marrow_floor::$c,
            GameLogic::Mercat => enemy::mercat::$c,
            GameLogic::Siren => enemy::siren::$c,
            GameLogic::Fish => enemy::fish::$c,
            GameLogic::Laser => laser::$c,
            GameLogic::TridentBullet => projectile::trident::$c,
            GameLogic::SirenBullet => projectile::scream::$c,
            GameLogic::Aquatis => enemy::aquatis::$c,
            GameLogic::AquatisCrack => enemy::aquatis::crack::$c,
            GameLogic::AquatisDynamite => enemy::aquatis::dynamite::$c,
            GameLogic::AquatisStalactite => enemy::aquatis::stalactite::$c,
            GameLogic::Tentacle => enemy::tentacle::$c,
            GameLogic::Stalactite => stalactite::$c,
            GameLogic::Shake => shake::$c,
            GameLogic::Chameleon => enemy::chameleon::$c,
            GameLogic::RedTail => enemy::redtail::$c,
            GameLogic::RedTailSpikes => boss::spikes::$c,
            GameLogic::RedTailWind => boss::wind::$c,
            GameLogic::RedTailKnife => boss::knife::$c,
            GameLogic::RedTailBullet => boss::flare::$c,
            GameLogic::WindDebris => boss::debris::$c,
            GameLogic::TigerGuard => enemy::tiger_guard::$c,
            GameLogic::LavaGeyser => lava::geyser::$c,
            GameLogic::LavaMouth => lava::mouth::$c,
            GameLogic::LavaHand => lava::hand::$c,
            GameLogic::LavaHandProjectile => lava::ball::$c,
            GameLogic::Omar => enemy::omar::$c,
            GameLogic::OmarShield => enemy::omar::shield::$c,
            GameLogic::OmarBullet => enemy::omar::bullet::$c,
        }
    };
}

/// The logic names family `g` runs.
pub fn names(g: GameLogic) -> &'static [&'static str] {
    constant!(g, NAMES)
}

/// The family that claims `name`, or `None` when no family does. Names
/// are case-sensitive.
pub fn kind_for(name: &str) -> Option<LogicKind> {
    ALL.iter()
        .find(|&&g| names(g).contains(&name))
        .map(|&g| LogicKind::Game(g))
}

/// Whether family `g` runs its logics yet. A family whose logics are not
/// written yet says so with `BUILT = false` in its own file; its objects
/// stay inert placeholders until it does.
pub fn built(g: GameLogic) -> bool {
    match g {
        GameLogic::PunkRat => enemy::punkrat::BUILT,
        GameLogic::Cannon => cannon::BUILT,
        GameLogic::Raux => enemy::raux::BUILT,
        GameLogic::BossStage => boss_stage::BUILT,
        GameLogic::CutThroat => enemy::cutthroat::BUILT,
        GameLogic::RobberThief => enemy::robberthief::BUILT,
        GameLogic::FloorSpike => floor_spike::BUILT,
        GameLogic::PathElevator => path_elevator::BUILT,
        GameLogic::Rope => rope::BUILT,
        GameLogic::SpringBoard => springboard::BUILT,
        GameLogic::SteppingStone => stepping_stone::BUILT,
        GameLogic::Katherine => enemy::katherine::BUILT,
        GameLogic::TownGuard1 => enemy::townguard::guard1::BUILT,
        GameLogic::TownGuard2 => enemy::townguard::guard2::BUILT,
        GameLogic::Seagull => enemy::seagull::BUILT,
        GameLogic::BreakPlank => break_plank::BUILT,
        GameLogic::JumpSwitch => jump_switch::BUILT,
        GameLogic::Statue => statue::BUILT,
        GameLogic::GooVent => goo_vent::BUILT,
        GameLogic::GroundBlower => ground_blower::BUILT,
        GameLogic::Wolvington => enemy::wolvington::BUILT,
        GameLogic::WolvingtonShot => enemy::wolvington::shot::BUILT,
        GameLogic::WolvingtonLight => enemy::wolvington::light::BUILT,
        GameLogic::RedTailPirate => enemy::redtail_pirate::BUILT,
        GameLogic::BearSailor => enemy::bear_sailor::BUILT,
        GameLogic::HermitCrab => enemy::crab::BUILT,
        GameLogic::CrabNest => enemy::crab::nest::BUILT,
        GameLogic::CrabBomb => enemy::crab::bomb::BUILT,
        GameLogic::Gabriel => enemy::gabriel::BUILT,
        GameLogic::GabrielBomb => enemy::gabriel::bomb::BUILT,
        GameLogic::CannonSwitch => cannon_switch::BUILT,
        GameLogic::CannonButton => cannon_switch::button::BUILT,
        GameLogic::GabrielCannon => cannon_switch::cannon::BUILT,
        GameLogic::HookPirate => enemy::hook_pirate::BUILT,
        GameLogic::PegLeg => enemy::pegleg::BUILT,
        GameLogic::DartTrap => dart_trap::BUILT,
        GameLogic::ConveyorBelt => conveyor_belt::BUILT,
        GameLogic::SawBlade => saw_blade::BUILT,
        GameLogic::SkullCannon => skull_cannon::BUILT,
        GameLogic::Marrow => enemy::marrow::BUILT,
        GameLogic::Parrot => enemy::parrot::BUILT,
        GameLogic::MarrowFloor => marrow_floor::BUILT,
        GameLogic::Mercat => enemy::mercat::BUILT,
        GameLogic::Siren => enemy::siren::BUILT,
        GameLogic::Fish => enemy::fish::BUILT,
        GameLogic::Laser => laser::BUILT,
        GameLogic::TridentBullet => projectile::trident::BUILT,
        GameLogic::SirenBullet => projectile::scream::BUILT,
        GameLogic::Aquatis => enemy::aquatis::BUILT,
        GameLogic::AquatisCrack => enemy::aquatis::crack::BUILT,
        GameLogic::AquatisDynamite => enemy::aquatis::dynamite::BUILT,
        GameLogic::AquatisStalactite => enemy::aquatis::stalactite::BUILT,
        GameLogic::Tentacle => enemy::tentacle::BUILT,
        GameLogic::Stalactite => stalactite::BUILT,
        GameLogic::Shake => shake::BUILT,
        GameLogic::Chameleon => enemy::chameleon::BUILT,
        GameLogic::RedTail => enemy::redtail::BUILT,
        GameLogic::RedTailSpikes => boss::spikes::BUILT,
        GameLogic::RedTailWind => boss::wind::BUILT,
        GameLogic::RedTailKnife => boss::knife::BUILT,
        GameLogic::RedTailBullet => boss::flare::BUILT,
        GameLogic::WindDebris => boss::debris::BUILT,
        GameLogic::TigerGuard => enemy::tiger_guard::BUILT,
        GameLogic::LavaGeyser => lava::geyser::BUILT,
        GameLogic::LavaMouth => lava::mouth::BUILT,
        GameLogic::LavaHand => lava::hand::BUILT,
        GameLogic::LavaHandProjectile => lava::ball::BUILT,
        GameLogic::Omar => enemy::omar::BUILT,
        GameLogic::OmarShield => enemy::omar::shield::BUILT,
        GameLogic::OmarBullet => enemy::omar::bullet::BUILT,
        _ => true,
    }
}

/// The family that runs `name` now: the one that claims it
/// ([`kind_for`]), when that family is [`built`]; otherwise `None` (the
/// object stays an inert placeholder). The registry binds names with it.
pub fn runs(name: &str) -> Option<LogicKind> {
    kind_for(name).filter(|k| matches!(k, LogicKind::Game(g) if built(*g)))
}

/// Every image set a family may switch an object to, sorted and without
/// duplicates.
pub fn runtime_image_sets() -> Vec<&'static str> {
    let mut sets: BTreeSet<&'static str> = enemy::IMAGE_SETS.iter().copied().collect();
    sets.extend(enemy::human::IMAGE_SETS.iter().copied());
    sets.extend(enemy::hook_pirate::derived_image_sets());
    for g in ALL {
        sets.extend(constant!(g, IMAGE_SETS).iter().copied());
    }
    sets.into_iter().collect()
}

/// Every animation key prefix a family may play, sorted and without
/// duplicates.
pub fn runtime_anim_prefixes() -> Vec<&'static str> {
    let mut prefixes: BTreeSet<&'static str> = enemy::ANIM_PREFIXES.iter().copied().collect();
    prefixes.extend(enemy::human::ANIM_PREFIXES.iter().copied());
    prefixes.extend(enemy::hook_pirate::derived_anim_prefixes());
    for g in ALL {
        prefixes.extend(constant!(g, ANIM_PREFIXES).iter().copied());
    }
    prefixes.into_iter().collect()
}

fn family(w: &World, id: ObjectId) -> Option<GameLogic> {
    match w.objects.get(id)?.kind {
        LogicKind::Game(g) => Some(g),
        _ => None,
    }
}

/// One logic call of `id`.
pub fn tick(w: &mut World, id: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.tick)(w, id);
        }
        dispatch!(g, tick(w, id));
    }
}

/// `attacker` hit `id`.
pub fn on_hit(w: &mut World, id: ObjectId, attacker: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.on_hit)(w, id, attacker);
        }
        dispatch!(g, on_hit(w, id, attacker));
    }
}

/// `id` attacked `victim`.
pub fn on_attack(w: &mut World, id: ObjectId, victim: ObjectId) {
    if let Some(g) = family(w, id) {
        #[cfg(test)]
        if let Some(h) = tests::hooks(g) {
            return (h.on_attack)(w, id, victim);
        }
        dispatch!(g, on_attack(w, id, victim));
    }
}

/// `carrier` lifted `id`. Kegs, crates and the human enemies Claw can
/// carry (officers, soldiers, cutthroats, robber thieves, Katherine, the
/// town guards, Wolvington, the red-tail pirates, the bear sailors,
/// Gabriel, the hook-handed pirates, PegLegs, the Mercats, the Sirens,
/// Aquatis and the tiger guards, see [`enemy::lift`]) react; other
/// families ignore it.
pub fn lifted(w: &mut World, id: ObjectId, carrier: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::lifted(w, id, carrier),
        Some(GameLogic::Crate) => crates::lifted(w, id, carrier),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine
            | GameLogic::TownGuard1
            | GameLogic::TownGuard2
            | GameLogic::Wolvington
            | GameLogic::RedTailPirate
            | GameLogic::BearSailor
            | GameLogic::Gabriel
            | GameLogic::HookPirate
            | GameLogic::PegLeg
            | GameLogic::Mercat
            | GameLogic::Siren
            | GameLogic::TigerGuard,
        ) => enemy::lift::lifted(w, id, carrier),
        Some(GameLogic::Aquatis) => enemy::aquatis::lifted(w, id, carrier),
        _ => {}
    }
}

/// `id` was thrown at (vx, vy) px/s.
pub fn thrown(w: &mut World, id: ObjectId, vx: i32, vy: i32) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::thrown(w, id, vx, vy),
        Some(GameLogic::Crate) => crates::thrown(w, id, vx, vy),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine
            | GameLogic::TownGuard1
            | GameLogic::TownGuard2
            | GameLogic::Wolvington
            | GameLogic::RedTailPirate
            | GameLogic::BearSailor
            | GameLogic::Gabriel
            | GameLogic::HookPirate
            | GameLogic::PegLeg
            | GameLogic::Mercat
            | GameLogic::Siren
            | GameLogic::Aquatis
            | GameLogic::TigerGuard,
        ) => enemy::lift::thrown(w, id, vx, vy),
        _ => {}
    }
}

/// Claw starts winding up a throw of `id`: a held human enemy shifts a little back and down (see [`enemy::lift::wind_up`]); other
/// families ignore it.
pub fn wind_up(w: &mut World, id: ObjectId) {
    if matches!(
        family(w, id),
        Some(
            GameLogic::Officer
                | GameLogic::Soldier
                | GameLogic::CutThroat
                | GameLogic::RobberThief
                | GameLogic::Katherine
                | GameLogic::TownGuard1
                | GameLogic::TownGuard2
                | GameLogic::Wolvington
                | GameLogic::RedTailPirate
                | GameLogic::BearSailor
                | GameLogic::Gabriel
                | GameLogic::HookPirate
                | GameLogic::PegLeg
                | GameLogic::Mercat
                | GameLogic::Siren
                | GameLogic::Aquatis
                | GameLogic::TigerGuard
        )
    ) {
        enemy::lift::wind_up(w, id);
    }
}

/// Claw walked off a ledge carrying `id`, his walk moving it `vx` px/s
/// across: a human enemy is let go as when he is hit or holds it too long,
/// keeping that speed across (see [`enemy::lift::walked_off`]), and true
/// is returned. Any other family (a keg, a crate) is left to the caller:
/// false.
pub fn walked_off(w: &mut World, id: ObjectId, vx: i32) -> bool {
    match family(w, id) {
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine
            | GameLogic::TownGuard1
            | GameLogic::TownGuard2
            | GameLogic::Wolvington
            | GameLogic::RedTailPirate
            | GameLogic::BearSailor
            | GameLogic::Gabriel
            | GameLogic::HookPirate
            | GameLogic::PegLeg
            | GameLogic::Mercat
            | GameLogic::Siren
            | GameLogic::Aquatis
            | GameLogic::TigerGuard,
        ) => {
            enemy::lift::walked_off(w, id, vx);
            true
        }
        _ => false,
    }
}

/// `id` was put down or let go.
pub fn dropped(w: &mut World, id: ObjectId) {
    match family(w, id) {
        Some(GameLogic::Keg) => keg::dropped(w, id),
        Some(GameLogic::Crate) => crates::dropped(w, id),
        Some(
            GameLogic::Officer
            | GameLogic::Soldier
            | GameLogic::CutThroat
            | GameLogic::RobberThief
            | GameLogic::Katherine
            | GameLogic::TownGuard1
            | GameLogic::TownGuard2
            | GameLogic::Wolvington
            | GameLogic::RedTailPirate
            | GameLogic::BearSailor
            | GameLogic::Gabriel
            | GameLogic::HookPirate
            | GameLogic::PegLeg
            | GameLogic::Mercat
            | GameLogic::Siren
            | GameLogic::Aquatis
            | GameLogic::TigerGuard,
        ) => enemy::lift::dropped(w, id),
        _ => {}
    }
}

/// One sprite of how a level object looks (see [`first_look`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// Image set.
    pub image_set: String,
    /// Frame number.
    pub frame: i32,
    /// Position.
    pub x: i32,
    /// Position.
    pub y: i32,
    /// Draw order.
    pub z: i32,
    /// Draw flags ([`crate::object::draw_flags`]).
    pub draw_flags: u32,
}

impl Look {
    /// How `o` looks now.
    pub fn of(o: &Object) -> Look {
        Look {
            image_set: o.image_set.clone(),
            frame: o.frame,
            x: o.x,
            y: o.y,
            z: o.z,
            draw_flags: o.draw_flags,
        }
    }
}

/// How the level object `o`, whose first logic call has not run yet,
/// looks once it has: the sprites a presentation draws for it while it
/// sleeps outside the logic view (in the extra width of a wide display),
/// so that it does not pop in, jump or vanish when the game first wakes
/// it. `None` when it looks as it is now; an empty list when it is not
/// drawn at all.
///
/// - A powder keg stands on the floor below it ([`keg::first_look`]).
/// - A stack of crates shows its column of crates, an empty stack nothing;
///   a single crate shows the first image of its set
///   ([`crates::first_look`]).
/// - A pickup shows its glitter next to it; a curse, which goes on its
///   first call in a single-player game, nothing ([`pickup::first_look`]).
/// - Dart traps, conveyor belt pieces, saw blades and skull cannons answer
///   for themselves ([`dart_trap::first_look`],
///   [`conveyor_belt::first_look`], [`saw_blade::first_look`],
///   [`skull_cannon::first_look`]).
/// - The slabs of Marrow's trap floor answer for themselves
///   ([`marrow_floor::first_look`]).
/// - A tiger guard shows its colour (white or orange, by its record) on
///   the floor below its place ([`enemy::tiger_guard::first_look`]).
///
/// It reads the object and the level only, on copies: nothing in the game
/// changes, no logic runs and no random number is drawn.
pub fn first_look(level: &SimLevel, o: &Object) -> Option<Vec<Look>> {
    match o.kind {
        LogicKind::Game(GameLogic::Keg) => keg::first_look(level, o).map(|l| vec![l]),
        LogicKind::Game(GameLogic::Crate) => crates::first_look(level, o),
        LogicKind::Game(GameLogic::Pickup) => pickup::first_look(level, o),
        LogicKind::Game(GameLogic::DartTrap) => dart_trap::first_look(level, o),
        LogicKind::Game(GameLogic::ConveyorBelt) => conveyor_belt::first_look(level, o),
        LogicKind::Game(GameLogic::SawBlade) => saw_blade::first_look(level, o),
        LogicKind::Game(GameLogic::SkullCannon) => skull_cannon::first_look(level, o),
        LogicKind::Game(GameLogic::MarrowFloor) => marrow_floor::first_look(level, o),
        LogicKind::Game(GameLogic::TigerGuard) => enemy::tiger_guard::first_look(level, o),
        _ => None,
    }
}

/// A new object running `name` at (x, y, z), set up as the level loader
/// sets up a placed object: its family (or an inert placeholder), a level
/// object in its activation class, and always active when its logic is.
pub fn new_object(name: &str, x: i32, y: i32, z: i32) -> Object {
    let entry = Registry::shared().get(name);
    let reg_flags = entry.map_or(0, |e| e.flags);
    let kind = match entry.map(|e| e.kind) {
        Some(k @ LogicKind::Game(_)) => k,
        _ => LogicKind::Inert,
    };
    let logic: Arc<str> = entry.map_or_else(|| Arc::from(name), |e| e.name.clone());
    let mut o = Object::new(0, x, y, z, logic, kind);
    o.reg_flags = reg_flags;
    o.flags = flags::WORLD;
    if reg_flags & REG_CLASS_1 != 0 {
        o.flags |= flags::CLASS_1;
    } else if reg_flags & REG_CLASS_2 != 0 {
        o.flags |= flags::CLASS_2;
    }
    if reg_flags & REG_ALWAYS != 0 {
        o.flags |= flags::ALWAYS_ACTIVE;
    }
    o
}

/// An inert, non-colliding, undrawn object standing in for something a
/// family does not create yet; `data` records what was asked for.
pub(crate) fn placeholder(name: &str, x: i32, y: i32, z: i32, data: ObjectData) -> Object {
    let mut o = new_object(name, x, y, z);
    o.kind = LogicKind::Inert;
    o.flags |= flags::NO_COLLIDE;
    o.data = data;
    o
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit {
    use claw_frame::TickInput;

    use super::*;
    use crate::level::{ObjectSpec, SimLevel};
    use crate::logic::enemy::EnemyState;
    use crate::tiles::TileMap;
    use crate::{Sim, SimConfig};

    #[test]
    fn every_name_belongs_to_one_family() {
        let mut seen = BTreeSet::new();
        for g in ALL {
            for &n in names(g) {
                assert!(seen.insert(n), "{n} is claimed twice");
                assert_eq!(kind_for(n), Some(LogicKind::Game(g)), "{n}");
                assert!(Registry::shared().get(n).is_some(), "{n} is registered");
            }
        }
        assert_eq!(kind_for("CaptainClaw"), None);
        assert_eq!(kind_for("officer"), None, "names are case-sensitive");
    }

    #[test]
    fn runtime_lists_are_sorted_and_unique() {
        for list in [runtime_image_sets(), runtime_anim_prefixes()] {
            let mut sorted = list.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(list, sorted);
            assert!(list.iter().all(|s| !s.is_empty()));
        }
    }

    /// A flat floor with an officer, a soldier, a rat, a cutthroat and a
    /// robber thief beside Claw, all built.
    fn enemies_beside_claw() -> (Sim, ObjectId, [ObjectId; 5]) {
        let mut rows = vec![".".repeat(60); 15];
        rows.push("#".repeat(60));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let mut level = SimLevel::synthetic(TileMap::from_ascii(64, &rows), (1000, 600));
        for (name, x) in [
            ("Officer", 900),
            ("Soldier", 1100),
            ("Rat", 1200),
            ("CutThroat", 1300),
            ("RobberThief", 1400),
        ] {
            level.objects.push(ObjectSpec::at(name, x, 900, 4000));
        }
        let mut sim = Sim::new(level, SimConfig::default());
        for _ in 0..3 {
            sim.step(TickInput::default());
        }
        let find = |g: GameLogic| {
            sim.objects()
                .all()
                .find(|(_, o)| o.kind == LogicKind::Game(g))
                .map(|(id, _)| id)
                .expect("the enemy exists")
        };
        let ids = [
            find(GameLogic::Officer),
            find(GameLogic::Soldier),
            find(GameLogic::Rat),
            find(GameLogic::CutThroat),
            find(GameLogic::RobberThief),
        ];
        let claw = sim
            .objects()
            .all()
            .find(|(_, o)| o.kind == LogicKind::Player)
            .map(|(id, _)| id)
            .expect("Claw");
        (sim, claw, ids)
    }

    /// What lifting changes on an object: state, flags, position, type,
    /// physics and data.
    fn look(sim: &Sim, id: ObjectId) -> (i32, u32, i32, i32, u32, i32, ObjectData) {
        let o = sim.objects().get(id).expect("the object exists");
        (
            o.state,
            o.flags,
            o.x,
            o.y,
            o.type_flags,
            o.physics_type,
            o.data.clone(),
        )
    }

    fn enemy_state(sim: &Sim, id: ObjectId) -> Option<EnemyState> {
        sim.objects()
            .get(id)
            .and_then(|o| EnemyState::from_id(o.state))
    }

    #[test]
    fn lifting_an_officer_or_a_soldier_reaches_its_family() {
        let (sim, claw, [officer, soldier, rat, cutthroat, robber]) = enemies_beside_claw();
        let rat_before = look(&sim, rat);
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            assert!(enemy_state(&sim, id).is_some(), "built");
            sim.with_world(|w| lifted(w, id, claw));
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Lifted));
            sim.with_world(|w| thrown(w, id, 300, -650));
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Thrown));
        }
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            sim.with_world(|w| {
                lifted(w, id, claw);
                dropped(w, id);
            });
            sim.step(TickInput::default());
            assert_eq!(enemy_state(&sim, id), Some(EnemyState::Dropped));
        }
        // The rat is never lifted.
        let mut sim = sim.clone();
        sim.with_world(|w| {
            lifted(w, rat, claw);
            thrown(w, rat, 300, -650);
            dropped(w, rat);
        });
        assert_eq!(look(&sim, rat), rat_before);
    }

    #[test]
    fn winding_up_reaches_a_lifted_officer_or_soldier() {
        let (sim, claw, [officer, soldier, rat, cutthroat, robber]) = enemies_beside_claw();
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            let winding = |sim: &Sim| {
                enemy::data_of(sim.objects().get(id).expect("the enemy exists"))
                    .expect("enemy data")
                    .winding_up
            };
            sim.with_world(|w| lifted(w, id, claw));
            assert!(!winding(&sim));
            sim.with_world(|w| wind_up(w, id));
            assert!(winding(&sim));
            // Lifted again, it starts without the wind-up.
            sim.with_world(|w| lifted(w, id, claw));
            assert!(!winding(&sim));
        }
        // The rat is never carried and ignores it.
        let mut sim = sim.clone();
        let before = look(&sim, rat);
        sim.with_world(|w| wind_up(w, rat));
        assert_eq!(look(&sim, rat), before);
    }

    #[test]
    fn an_enemy_nobody_lifted_ignores_throws_and_drops() {
        let (sim, _, [officer, soldier, _, cutthroat, robber]) = enemies_beside_claw();
        for id in [officer, soldier, cutthroat, robber] {
            let mut sim = sim.clone();
            let before = look(&sim, id);
            sim.with_world(|w| {
                thrown(w, id, 300, -650);
                dropped(w, id);
            });
            assert_eq!(look(&sim, id), before);
        }
    }

    #[test]
    fn new_objects_follow_the_registry() {
        let o = new_object("Elevator", 1, 2, 3);
        assert_eq!(o.kind, LogicKind::Game(GameLogic::Platform));
        assert_eq!(o.activation_class(), 1);
        assert_eq!((o.x, o.y, o.z), (1, 2, 3));
        let o = new_object("GlobalAmbientSound", 0, 0, 0);
        assert!(o.flags & flags::ALWAYS_ACTIVE != 0);
        let o = new_object("NoSuchLogic", 0, 0, 0);
        assert_eq!(o.kind, LogicKind::Inert);
        assert_eq!(&*o.logic, "NoSuchLogic");
        assert_eq!(o.activation_class(), 0);
    }
}
