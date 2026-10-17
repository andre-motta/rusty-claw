//! The logic registry: logic names, their registration flags and the
//! implementation that runs them.
//!
//! Level files and code refer to logics by name. The registration flag
//! decides an object's activation window: 0 for enemies, bosses and
//! projectiles (1600x1200), 1 for elevators and ambient sounds
//! (2560x1920), 2 for pickups, decoration and small hazards (768x576), and
//! 4 for objects that are always active. Names are case-sensitive.
//!
//! Names are bound to their implementation through
//! [`crate::logic::runs`]. Registered names no family runs yet (claimed by
//! no family, or by a family that is not built yet) stay inert
//! placeholders: they exist, keep their activation class and can be drawn,
//! and do nothing. Names that are not registered at all also become inert
//! placeholders, in activation class 0, and are reported by the
//! simulation.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::logic;
use crate::object::LogicKind;

/// Registration flag: activation class 1.
pub const REG_CLASS_1: u32 = 1;
/// Registration flag: activation class 2.
pub const REG_CLASS_2: u32 = 2;
/// Registration flag: always active.
pub const REG_ALWAYS: u32 = 4;

/// Logic name of Captain Claw.
pub const PLAYER_LOGIC: &str = "CaptainClaw";
/// Logic name of the camera.
pub const CAMERA_LOGIC: &str = "CaptainClawScreenPosition";

/// One registered logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicEntry {
    /// The name.
    pub name: Arc<str>,
    /// Registration flags.
    pub flags: u32,
    /// What runs it.
    pub kind: LogicKind,
}

/// Name-keyed logic registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    entries: HashMap<Arc<str>, LogicEntry>,
}

impl Registry {
    /// An empty registry.
    pub fn empty() -> Registry {
        Registry {
            entries: HashMap::new(),
        }
    }

    /// The game's logics.
    pub fn standard() -> Registry {
        let mut r = Registry::empty();
        for &(name, flags) in BUILTIN {
            let kind = match name {
                PLAYER_LOGIC => LogicKind::Player,
                CAMERA_LOGIC => LogicKind::Camera,
                _ => logic::runs(name).unwrap_or(LogicKind::Inert),
            };
            r.register(name, flags, kind);
        }
        r
    }

    /// The game's logics, built once and shared.
    pub fn shared() -> &'static Registry {
        static SHARED: OnceLock<Registry> = OnceLock::new();
        SHARED.get_or_init(Registry::standard)
    }

    /// Registers `name`; a later registration of the same name wins.
    pub fn register(&mut self, name: &str, flags: u32, kind: LogicKind) {
        let name: Arc<str> = Arc::from(name);
        self.entries
            .insert(name.clone(), LogicEntry { name, flags, kind });
    }

    /// Looks a name up.
    pub fn get(&self, name: &str) -> Option<&LogicEntry> {
        self.entries.get(name)
    }

    /// Number of registered names.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Registry::standard()
    }
}

/// Logic names and registration flags.
const BUILTIN: &[(&str, u32)] = &[
    ("CaptainClaw", 0),
    ("CaptainClawHit", 0),
    ("CaptainClawRemoteRacer", 0),
    ("CaptainClawScreenPosition", 0),
    ("ClawMultiRacer", 0),
    ("Flare", 0),
    ("WolvingtonLFX", 0),
    ("Bullet", 0),
    ("MagicBullet", 0),
    ("FireBullet", 0),
    ("LightningBullet", 0),
    ("IceBullet", 0),
    ("PistolBullet", 0),
    ("CannonBall", 0),
    ("WolvingtonBullet", 0),
    ("OmarBullet", 0),
    ("SirenBullet", 0),
    ("TridentBullet", 0),
    ("RedTailKnife", 0),
    ("RedTailBullet", 0),
    ("LogicHit", 2),
    ("LogicAttack", 2),
    ("Officer", 0),
    ("Soldier", 0),
    ("Raux", 0),
    ("Rat", 0),
    ("PunkRat", 0),
    ("RatBomb", 0),
    ("GroundCannon", 0),
    ("SkullCannon", 0),
    ("TowerCannonLeft", 0),
    ("TowerCannonRight", 0),
    ("PowderKeg", 2),
    ("BouncingGoodie", 0),
    ("CutThroat", 0),
    ("RobberThief", 0),
    ("Katherine", 0),
    ("TownGuard1", 0),
    ("TownGuard2", 0),
    ("BearSailor", 0),
    ("RedTailPirate", 0),
    ("RedTail", 1),
    ("Wolvington", 0),
    ("Omar", 0),
    ("OmarShield", 0),
    ("GooVent", 0),
    ("Laser", 0),
    ("Crate", 2),
    ("FrontStackedCrates", 2),
    ("BackStackedCrates", 2),
    ("AniRope", 0),
    ("Dynamite", 0),
    ("BreakPlank", 0),
    ("SinglePlank", 0),
    ("Seagull", 0),
    ("Shake", 2),
    ("JumpSwitch", 2),
    ("HermitCrab", 0),
    ("TProjectile", 0),
    ("GroundBlower", 2),
    ("CrabNest", 2),
    ("ConveyorBelt", 2),
    ("BackgroundLogic", 0),
    ("CrabBomb", 0),
    ("Stalactite", 0),
    ("PegLeg", 0),
    ("Fish", 0),
    ("CannonSwitch", 0),
    ("GabrielCannon", 0),
    ("Gabriel", 1),
    ("GabrielBomb", 0),
    ("CannonButton", 0),
    ("Aquatis", 1),
    ("AquatisCrack", 0),
    ("Tentacle", 0),
    ("AquatisDynamite", 1),
    ("EndLevelGem", 0),
    ("AquatisStalactite", 0),
    ("Siren", 0),
    ("FallingDebris", 0),
    ("LavaGeyser", 2),
    ("LavaMouth", 2),
    ("LavaHand", 0),
    ("LavaHandProjectile", 0),
    ("Chameleon", 0),
    ("TigerGuard", 0),
    ("Mercat", 0),
    ("WindDebris", 0),
    ("RedTailWind", 1),
    ("RedTailSpikes", 2),
    ("Parrot", 0),
    ("Marrow", 0),
    ("MarrowFloor", 0),
    ("ScoreFrame", 0),
    ("WeaponFrame", 0),
    ("HealthFrame", 0),
    ("LivesFrame", 0),
    ("TimerFrame", 0),
    ("StatusNumberDigit", 0),
    ("ScoreRibbon", 0),
    ("MultiStats", 0),
    ("GoldPowerup", 2),
    ("HealthPowerup", 2),
    ("PowerupGlitter", 2),
    ("EndOfLevelPowerup", 2),
    ("SpecialPowerup", 2),
    ("CratePowerup", 2),
    ("FrontCrate", 2),
    ("BehindCrate", 2),
    ("FrontStatue", 2),
    ("BehindStatue", 2),
    ("CrateHit", 2),
    ("CoinPowerup", 2),
    ("TreasurePowerup", 2),
    ("MagicPowerup", 2),
    ("SuperPowerup", 2),
    ("AmmoPowerup", 2),
    ("GlitterlessPowerup", 2),
    ("PointsIcon", 2),
    ("CursePowerup", 2),
    ("BossWarp", 0),
    ("Checkpoint", 2),
    ("CheckpointAttack", 2),
    ("CheckpointTimer", 2),
    ("FirstSuperCheckpoint", 2),
    ("SecondSuperCheckpoint", 2),
    ("SuperCheckpointAttack", 2),
    ("StandardElevator", 1),
    ("SlidingElevator", 1),
    ("StartElevator", 1),
    ("StopElevator", 1),
    ("SpringBoard", 1),
    ("WaterRock", 1),
    ("Elevator", 1),
    ("OneWayStartElevator", 1),
    ("OneWayTriggerElevator", 1),
    ("TriggerElevator", 1),
    ("PathElevator", 1),
    ("SteppingStone", 2),
    ("SteppingStone2", 2),
    ("SteppingStone3", 2),
    ("SteppingStone4", 2),
    ("StartSteppingStone", 2),
    ("OneTimeStartSteppingStone", 2),
    ("CrumblingPeg", 2),
    ("CrumblingPegNoRespawn", 2),
    ("TogglePeg", 2),
    ("TogglePeg2", 2),
    ("TogglePeg3", 2),
    ("TogglePeg4", 2),
    ("AniCycle", 2),
    ("AniCycleNormal", 0),
    ("ChildChar", 0),
    ("ChildScoreNum", 0),
    ("ChildRibbon", 0),
    ("Message", 0),
    ("SingleFrameMessage", 0),
    ("DoNothingNormal", 0),
    ("DoNothing", 2),
    ("BehindCandy", 2),
    ("FrontCandy", 2),
    ("BehindAniCandy", 2),
    ("FrontAniCandy", 2),
    ("GlitterMother", 2),
    ("GlitterBaby", 2),
    ("GooCoverup", 0),
    ("GooBubble", 2),
    ("HitBurst", 2),
    ("Splash", 2),
    ("BlinkingEyes", 2),
    ("FloorSpike", 2),
    ("FloorSpike2", 2),
    ("FloorSpike3", 2),
    ("FloorSpike4", 2),
    ("SawBlade", 2),
    ("SawBlade2", 2),
    ("SawBlade3", 2),
    ("SawBlade4", 2),
    ("TreasureCounter", 0),
    ("SimpleAnimation", 0),
    ("TreasureLogic", 0),
    ("MapPieceLogic", 0),
    ("AmuletGemLogic", 0),
    ("MapProgressLogic", 0),
    ("StationaryLight", 2),
    ("ChildLight", 0),
    ("MenuSparkle", 0),
    ("MenuClaw", 0),
    ("SoundTrigger", 2),
    ("BigSoundTrigger", 2),
    ("SmallSoundTrigger", 2),
    ("TinySoundTrigger", 2),
    ("HugeSoundTrigger", 2),
    ("TallSoundTrigger", 2),
    ("WideSoundTrigger", 2),
    ("ClawDialogSound", 0),
    ("EnemyDialogSound", 0),
    ("ClawDialogSoundTrigger", 2),
    ("ClawDialogBigSoundTrigger", 2),
    ("ClawDialogSmallSoundTrigger", 2),
    ("ClawDialogTinySoundTrigger", 2),
    ("ClawDialogHugeSoundTrigger", 2),
    ("ClawDialogTallSoundTrigger", 2),
    ("ClawDialogWideSoundTrigger", 2),
    ("GlobalAmbientSound", 4),
    ("AmbientSound", 1),
    ("AmbientPosSound", 0),
    ("SpotAmbientSound", 0),
    ("BossStager", 0),
    ("BossStagerAttack", 0),
    ("BossHealthMeter", 0),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry() {
        let r = Registry::standard();
        assert_eq!(r.len(), BUILTIN.len());
        assert_eq!(r.get(PLAYER_LOGIC).unwrap().kind, LogicKind::Player);
        assert_eq!(r.get(CAMERA_LOGIC).unwrap().kind, LogicKind::Camera);
        assert_eq!(r.get("Elevator").unwrap().flags, REG_CLASS_1);
        assert_eq!(r.get("TreasurePowerup").unwrap().flags, REG_CLASS_2);
        assert_eq!(r.get("GlobalAmbientSound").unwrap().flags, REG_ALWAYS);
        assert!(r.get("officer").is_none(), "names are case-sensitive");
        assert_eq!(
            r.get("Officer").unwrap().kind,
            LogicKind::Game(logic::GameLogic::Officer)
        );
        assert_eq!(r.get("Sign"), None);
        assert_eq!(r.get("Laser").unwrap().kind, LogicKind::Inert);
        // Claimed by a family that is not built yet: still a placeholder.
        for name in ["PunkRat", "TowerCannonLeft", "Raux", "BossStager"] {
            let built = logic::runs(name).is_some();
            assert_eq!(r.get(name).unwrap().kind == LogicKind::Inert, !built);
        }
        assert_eq!(Registry::shared(), &r);
    }
}
