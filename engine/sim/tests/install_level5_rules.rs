//! The fifth level of your own install, played tick by tick along the
//! routes the original was run with at one 18 ms step per frame and the
//! original speeds (skipped without `CLAW_INSTALL`): at the tick where
//! each of these rules first shows on those routes, the game does what
//! the original did there. The routes are the fifth level's
//! original-speeds route and the fifth level of the game that starts on
//! the fourth, kept here as they were run (both were found before these
//! rules, and neither wins under them).
//!
//! 1. A standing enemy's exact height still moves under gravity each
//!    step; the tiles keep it on its floor pixel, and once the exact
//!    height has left that pixel it goes back onto it with the upward
//!    speed that move back implies. A built enemy's exact height stays at
//!    the point it was placed at while it is dropped onto the floor. The
//!    town guard placed at 21220,5389 is built in tick 7261 of the first
//!    route on the floor at 5416 with its exact height still 5389, sleeps
//!    two ticks, and its first step in tick 7264 puts it back on the floor
//!    at 326.66 px/s up. Standing on, that speed settles towards 36 px/s:
//!    after tick 7518, as it walks off its ledge, it is 36.29 px/s up, so
//!    its first falling step lifts it to 5415.6708.
//! 2. Going Home stops an enemy across only: that guard goes Home in mid
//!    air in tick 7526 at 21243,5431 and keeps falling at 251.71 px/s, so
//!    it lands on the spikes and dies in tick 7537, where the original
//!    draws its loot.
//! 3. Going off screen cancels the 500 ms out-of-range wait: the town
//!    guard placed with its left limit at 24502 comes back on screen in
//!    tick 9608 with Claw out of its range and stays in mode 6, so at its
//!    limit in tick 9610 it goes idle without its idle line roll.
//! 4. An enemy's movement step covers the difference of two game clock
//!    readings in double-precision seconds, the clock one tick ahead of
//!    the tick's start: on the game's fifth level the town guard pacing
//!    right at 120 px/s towards its limit 11415 is at 11414.999999999998
//!    after tick 3594, reaches the limit in tick 3595 and makes its limit
//!    roll in tick 3596.
//! 5. Claw is on an enemy's right when his pixel is at or past the
//!    enemy's exact x: in tick 5312 of the game's fifth level the dead
//!    guard flying at 16222.6 has Claw, at 16222 and swinging his sword,
//!    on its left, so Claw faces it and it makes its sword reaction roll.
//!
//! The random stream equals the original's after every tick checked here.

use claw_assets::{Install, LevelData, LevelSource};
use claw_sim::logic::enemy::{self, EnemyState};
use claw_sim::object::Object;
use claw_sim::player::progress::parse_start_lines;
use claw_sim::{InputScript, Sim, SimConfig, SimLevel, StartState, Tuning};

/// The random state the reference runs of the fifth level start from.
const LEVEL5_RNG_AT_T0: u32 = 4_105_512_405;

/// The fifth level's original-speeds route, its first 9611 ticks: held
/// masks in hex, each with its run of ticks.
const BOT_ROUTE: &str = "
    0:2 2000000:198 2:2 0:12 2000000:6 0:6 2:2 0:6 2000000:36 2000001:9 2000000:32 2000001:14
    2000000:26 1:14 0:2 2000000:24 1:14 0:2 2000000:24 2000001:14 2000000:14 6000000:6 4000000:30
    2000000:6 2000001:9 2000000:8 1000000:12 2000001:9 2000000:32 2000001:5 2000000:2 2:2 0:6
    2000000:6 0:6 2:2 0:6 2000000:30 8000000:6 1000000:6 2000000:6 0:6 8000002:2 8000000:6 1:14 0:2
    2:2 0:6 2000000:48 0:6 8000002:2 8000000:6 2000001:2 2000000:2 0:6 4:2 0:42 4:2 0:36 2000000:18
    2000001:9 2000000:26 2000001:5 2000000:80 2000001:9 2000000:8 2:2 0:12 2000000:30 0:6
    2000000:72 2000001:5 2000000:2 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 0:6 2:2 0:6
    2000000:120 1000000:6 2000000:30 8000000:6 8000002:2 8000000:6 2000000:6 0:6 2:2 0:6 2000000:6
    0:6 2:2 0:6 2000000:30 2000001:14 2000000:44 2000001:14 2000000:26 2000001:14 2000000:14 0:6
    2000000:6 1:9 0:2 2000000:12 0:6 1:14 0:2 2000000:24 2000001:14 2000000:8 6000000:6 2000001:14
    2000000:8 2:2 0:6 2000000:6 2000001:14 2000000:38 1:14 0:2 2000000:24 2000001:14 2000000:26
    2000001:9 2000000:14 0:6 2000000:6 1:14 0:2 1000000:6 2000000:6 0:18 2000000:18 0:6 1000001:2
    1000000:14 2000000:6 1:14 0:2 1000000:12 0:18 2000001:9 2000000:20 1:14 0:2 2000000:6 1000000:6
    0:6 1:14 0:2 1000000:6 2000000:6 1000000:6 1000001:14 1000000:2 2000000:6 1000000:12 2000001:14
    2000000:14 0:6 2000000:12 2000001:9 2000000:38 0:6 8000000:6 1:5 0:2 2000001:9 2000000:38 2:2
    0:12 2000000:12 8000000:6 2000000:6 2000001:2 2000000:8 2:2 0:6 2000000:36 8000000:6 2000001:14
    2000000:104 2000001:2 2000000:2 2:2 0:12 2000000:6 8000000:6 2000000:6 2:2 0:6 2000000:60 2:2
    0:6 2000000:60 0:6 8000002:2 8000000:6 2000001:2 2000000:8 0:6 8000002:2 8000000:6 2000001:14
    2000000:38 0:6 8000002:2 8000000:6 2000001:2 2000000:2 8000000:6 2000000:6 0:6 8000002:2
    8000000:6 2000001:14 2000000:20 2000001:5 2000000:8 2:2 0:12 2000001:5 2000000:20 2000001:14
    2000000:32 2000001:14 2000000:26 2000001:14 2000000:26 2000001:9 2000000:8 0:6 2000000:6 1:14
    0:2 2000000:24 2000001:14 2000000:14 2:2 0:6 2000000:6 1:9 0:2 2:2 0:6 2000000:12 2000001:14
    2000000:26 2000001:14 2000000:26 2000001:14 2000000:26 2000001:14 2000000:26 2000001:14
    2000000:14 0:6 2000000:6 1000001:2 1000000:2 2:2 0:6 2000000:12 1:9 0:2 2:2 0:6 2000000:12
    2000001:14 2000000:26 2000001:14 2000000:2 4000000:6 1000001:14 1000000:2 2000000:6 1000000:6
    6000000:6 1000001:14 1000000:2 0:6 2000000:6 0:6 1:5 0:8 2:2 0:6 1000001:14 1000000:20
    2000000:6 1000000:6 1000001:14 1000000:8 5000000:6 2000001:14 2000000:2 2:2 0:6 1000000:6
    2000001:14 2000000:8 0:6 8000000:6 2000001:2 2000000:2 1:2 0:2 2000000:6 1000000:6 0:6
    2000001:2 2000000:32 8000000:6 2000000:6 2000001:9 2000000:38 2000001:5 2000000:14 0:6
    2000001:5 2000000:134 2000001:5 2000000:2 2:2 0:6 2000000:6 0:6 8000002:2 8000000:6 2000001:14
    2000000:50 2000001:5 2000000:2 2:2 0:6 2000000:6 4:2 0:12 2000000:18 8000000:6 2000001:2
    2000000:2 0:6 8000002:2 8000000:6 2000001:9 2000000:2 8000000:6 2000001:14 2000000:86 2:2 0:6
    2000000:66 2:2 0:6 2000000:12 8000000:6 2000001:14 2000000:20 8000000:6 2000001:14 2000000:32
    2:2 0:6 2000000:6 2000001:5 2000000:20 2000001:5 2000000:14 2000001:5 2000000:2 2:2 0:12
    2000000:12 4:2 0:12 2000000:24 0:6 2:2 0:6 2000000:24 1:2 0:2 2:2 0:6 2000000:6 1000000:6
    8000000:6 2000001:14 2000000:14 1:2 0:2 2:2 0:6 2000000:6 0:6 8000002:2 8000000:6 2000001:14
    2000000:20 2000001:9 2000000:2 2:2 0:12 2000000:6 2000001:9 2000000:2 2:2 0:6 2000000:30 0:6
    8000002:2 8000000:6 2000001:9 2000000:2 4:2 0:36 2000000:6 0:6 2:2 0:6 2000000:6 2000001:2
    2000000:8 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 0:6 2:2 0:6 2000000:18 2000001:2
    2000000:2 2:2 0:6 2000000:6 0:6 8000002:2 8000000:6 2000001:14 2000000:20 2000001:5 2000000:20
    2000001:9 2000000:2 2:2 0:6 2000000:18 0:6 8000002:2 8000000:6 2000001:14 2000000:8 2000001:2
    2000000:2 2:2 0:6 2000000:6 0:6 8000000:6 2000001:2 2000000:2 0:6 2:2 0:6 2000000:12 2000001:2
    2000000:2 2:2 0:6 2000000:12 0:6 2:2 0:6 2000000:84 2:2 0:12 2000001:2 2000000:2 0:6 2:2 0:6
    2000000:18 2000001:5 2000000:2 2:2 0:12 2000000:6 0:6 2000001:2 2000000:2 2:2 0:6 2000000:6 0:6
    2000000:24 2000001:14 2000000:8 2:2 0:12 2000000:6 2000001:9 2000000:20 2000001:14 2000000:38
    2000001:14 2000000:26 2000001:5 2000000:8 0:18 2000001:5 2000000:26 2000001:9 2000000:2 0:24
    2000001:14 2000000:2 5000000:6 2000001:14 2000000:26 1000001:14 1000000:14 5000000:6 2000001:14
    2000000:2 0:6 1000000:6 4000000:6 2000001:14 2000000:2 1000000:6 2:2 0:6 2000001:9 2000000:86
    0:6 2000001:5 2000000:2 1000000:6 2:2 0:6 2000001:14 2000000:38 2000001:5 2000000:8 2:2 0:6
    2000000:120 8000000:6 1:9 0:2 2:2 0:6 2000000:6 1000000:6 8000000:6 1:14 0:8 2000000:12 1:5 0:2
    1000000:12 0:6 8000002:2 8000000:6 2000000:6 8000000:6 4:2 0:12 2000000:6 8000000:12 1000001:14
    1000000:2 2000000:24 1000000:6 8000000:6 1000001:2 1000000:2 0:6 2:2 0:18 2:2 0:6 2000000:84
    2000001:9 2000000:8 2:2 0:6 2000000:6 2000001:9 2000000:20 2000001:2 2000000:2 0:18 1:9 0:2
    2000000:12 6000000:6 1000001:14 1000000:2 2000000:6 0:6 4000000:6 1000001:14 1000000:2 0:6 2:2
    0:6 1:14 0:2 1000000:24 5000001:14 5000000:2 2000001:14 2000000:2 1000000:12 6000000:6
    2000001:14 2000000:14 0:6 2000000:12 2000001:5 2000000:2 2:2 0:6 2000000:6 0:6 8000002:2
    8000000:6 2000001:14 2000000:8 8000000:6 2000001:2 2000000:2 2000001:9 2000000:44 2:2 0:12
    2000000:6 0:6 2:2 0:6 2000000:12 8000002:2 8000000:6 0:18 2000000:6 2000001:2 2000000:8 2:2 0:6
    2000000:24 0:6 4:2 0:24 2000000:6 8000000:12 2000000:6 2:2 0:18 2000000:12 8000000:6 2000001:2
    2000000:2 8000000:6 1:14 0:2 8000002:2 8000000:6 2000001:14 2000000:50 2:2 0:6 2000000:96
    1000000:6 0:6 2000001:5 2000000:20 2000001:9 2000000:20 2000001:9 2000000:20 2000001:9
    2000000:20 2000001:9 2000000:38 2000001:9 2000000:8 2:2 0:6 2000000:6 1:9 0:2 2:2 0:6
    2000000:12 2000001:9 2000000:20 2000001:9 2000000:38 2000001:2 2000000:2 2:2 0:6 2000000:12 2:2
    0:6 2000000:54 2000001:14 2000000:62 2000001:9 2000000:8 0:6 2000000:6 1:14 0:2 2000000:6
    2000001:14 2000000:2 0:6 2:2 0:6 1:14 0:8 1000000:6 2000000:6 1000001:14 1000000:2 2000000:18
    2000001:14 2000000:2 0:6 2:2 0:6 1:5 0:8 2:2 0:6 1000001:14 1000000:2 0:18 2000001:14 2000000:2
    0:18 8000002:2 8000000:6 1000001:2 1000000:2 0:6 2000001:9 2000000:8 2000001:14 2000000:104 2:2
    0:30 2000000:6 2000001:9 2000000:20 2000001:14 2000000:26 2000001:9 2000000:26 0:6 1:14 0:2
    2000000:24 1:14 0:2 2000000:24 2000001:14 2000000:14 1000000:6 0:6 1:14 0:2 2000000:24
    2000001:14 2000000:8 4000000:24 2000001:5 2000000:2 2:2 0:6 2000001:9 2000000:8 2:2 0:6
    1000000:6 2000000:6 2000001:2 2000000:20 2000001:5 2000000:2 2:2 0:6 2000000:12 1:2 0:2 2:2 0:6
    2000000:12 2000001:5 2000000:26 2000001:9 2000000:20 0:6 2000000:6 4:2 0:42 8000002:2 8000000:6
    1:14 0:2 2000001:2 2000000:20 2:2 0:12 2000000:36 2000001:9 2000000:44 2:2 0:12 2000000:6
    2000001:14 2000000:2 2:2 0:6 2000000:6 1:14 0:20 1000001:14 1000000:2 2000000:6 1000000:6
    2000000:6 1000001:14 1000000:20 1000001:14 1000000:20 2000001:5 2000000:2 1000000:6 2:2 0:6
    2000001:14 2000000:14 0:6 1000001:2 1000000:2 2000000:6 2:2 0:6 2000000:6 0:18 2000000:36 2:2
    0:6 2000000:42 8000000:6 2000001:2 2000000:2 2000001:9 2000000:32 2000001:14 2000000:2 0:6 2:2
    0:12 2000000:84 0:6 2:2 0:6 2000000:6 0:6 2:2 0:6 2000000:32
";

/// The fifth level of the game that starts on the fourth, its first 5313
/// ticks.
const PAIR_ROUTE: &str = "
    0:2 2000000:198 2:2 0:12 2000000:6 0:6 8000002:2 8000000:6 2000001:14 2000000:20 0:6 2000001:9
    2000000:38 2000001:14 2000000:14 0:6 2000000:6 1:14 0:2 2000000:24 1:14 0:2 2000000:24
    2000001:14 2000000:20 6000000:6 4000000:6 2000001:14 2000000:2 0:6 1000000:6 2000001:14
    2000000:20 0:6 8000002:2 8000000:6 2000001:2 2000000:2 0:6 8000002:2 8000000:6 2000000:6 1:2
    0:2 2:2 0:6 2000000:30 2:2 0:6 2000000:6 0:6 4:2 0:12 2000000:54 0:6 8000002:2 8000000:6 4:2
    0:18 8000002:2 8000000:6 1000001:14 1000000:2 2:2 0:66 2000000:18 2000001:14 2000000:38
    2000001:5 2000000:80 2000001:9 2000000:8 2:2 0:12 2000000:30 0:6 2000000:84 4:2 0:18 2000000:12
    0:6 8000002:2 8000000:6 2000001:14 2000000:74 8000000:6 2000001:2 2000000:2 8000000:6
    2000001:14 2000000:14 1000000:6 2000000:12 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 2:2
    0:6 2000000:48 8000000:6 2000001:2 2000000:2 2000001:14 2000000:62 0:6 2:2 0:24 2:2 0:6
    2000000:30 0:6 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 0:6 2:2 0:6 2000000:102
    8000000:6 1000001:2 1000000:2 2000000:6 2000001:14 2000000:38 1:14 0:2 2000000:24 2000001:14
    2000000:26 2000001:9 2000000:14 2:2 0:6 1:14 0:2 1000000:12 0:18 2000000:12 1000000:6 0:6 1:14
    0:2 1000000:12 0:18 2000001:14 2000000:14 1:14 0:2 2:2 0:6 2000000:6 8000000:6 1000001:2
    1000000:2 1000001:14 1000000:14 0:6 2000001:14 2000000:14 0:6 2000000:6 2000001:14 2000000:44
    1000000:6 8000000:6 2000001:2 2000000:2 2000001:9 2000000:20 0:6 8000002:2 8000000:6 2000001:5
    2000000:2 0:12 2000000:12 0:18 2000000:24 2:2 0:12 2000000:12 8000000:6 2000001:14 2000000:134
    2000001:2 2000000:2 0:6 2:2 0:6 2000000:6 0:18 2000000:24 2000001:5 2000000:2 2:2 0:12
    2000001:2 2000000:44 2:2 0:6 2000000:54 0:6 1:2 0:2 2:2 0:6 1000000:6 2000000:6 0:6 2:2 0:6
    2000000:66 0:6 8000002:2 8000000:6 2000000:6 0:6 8000002:2 8000000:6 2000001:9 2000000:2
    8000000:6 2000001:14 2000000:26 0:6 2000000:6 2000001:9 2000000:8 0:12 2000001:14 2000000:32
    2000001:14 2000000:26 2000001:14 2000000:26 2000001:9 2000000:20 1000001:2 1000000:2 2:2 0:6
    2000000:6 0:6 1:14 0:2 2000000:24 2000001:14 2000000:14 1000000:6 2000000:6 1:14 0:2 2000000:24
    2000001:14 2000000:20 0:6 1:14 0:2 2000000:24 2000001:14 2000000:26 1:14 0:2 2000000:6 0:6
    2000000:12 2000001:14 2000000:26 2000001:14 2000000:8 2:2 0:6 2000000:12 1:14 0:2 2000000:24
    2000001:14 2000000:8 6000000:6 4000000:6 1000001:14 1000000:2 0:6 2000000:6 6000000:6 4000000:6
    1000001:14 1000000:26 1000001:14 1000000:14 5000000:6 2000001:5 2000000:2 4000000:6 1000001:14
    1000000:2 2000000:6 2:2 0:6 2000001:14 2000000:8 1000000:12 2000000:6 1:14 0:2 2000000:6 2:2
    0:6 1000000:6 0:6 2000001:14 2000000:2 2:2 0:6 2000000:6 2000001:2 2000000:8 1000000:6 0:6
    8000002:2 8000000:6 0:6 2000001:5 2000000:32 2000001:14 2000000:32 0:6 2000001:2 2000000:20
    2000001:5 2000000:134 2:2 0:12 2000000:12 2:2 0:12 2000000:66 2:2 0:12 2000000:12 8000000:6
    8000002:2 8000000:6 2000000:6 0:6 2:2 0:6 2000000:6 4:2 0:12 2000000:12 0:6 2000001:2 2000000:2
    2:2 0:18 8000002:2 8000000:6 2000001:14 2000000:92 2:2 0:6 2000000:102 0:6 2000000:54 2:2 0:6
    2000001:2 2000000:20 2000001:9 2000000:20 2000001:2 2000000:2 2:2 0:6 2000000:12 0:6 2:2 0:18
    2:2 0:6 2000000:42 2000001:5 2000000:2 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 2:2 0:6
    2000000:54 2000001:5 2000000:2 2:2 0:6 2000000:6 2000001:5 2000000:8 0:6 2:2 0:6 2000000:6
    2000001:2 2000000:8 2:2 0:6 2000000:6 8000000:6 2000001:2 2000000:2 2000001:5 2000000:2 0:6 2:2
    0:6 2000000:24 2000001:5 2000000:2 0:6 2:2 0:6 8000002:2 8000000:6 0:6 2:2 0:6 2000000:12 2:2
    0:12 2000000:12 0:6 2:2 0:6 2000000:48 2000001:2 2000000:8 0:6 2000000:6 2000001:14 2000000:14
    2:2 0:12 8000002:2 8000000:6 2000001:2 2000000:2 2:2 0:6 2000000:30 2000001:2 2000000:2 2:2 0:6
    2000000:6
";

/// The progress that game carries into the fifth level.
const PAIR_START: &str = "
    # start_score = 280700
    # start_health = 75
    # start_lives = 7
    # start_pistol = 9
    # start_magic = 5
    # start_dynamite = 3
    # start_weapon = pistol
    # start_treasure_game = 132, 0, 5, 3, 10, 1, 18, 1, 0
";

/// The original's random state after these ticks of the first route.
const BOT_RNG: [(usize, u32); 12] = [
    (5191, 2_665_789_629),
    (5192, 3_185_387_631),
    (5193, 1_293_948_698),
    (7517, 2_889_711_068),
    (7525, 1_507_913_256),
    (7526, 2_804_837_872),
    (7536, 2_815_673_668),
    (7537, 2_213_006_152),
    (9607, 1_156_913_810),
    (9608, 75_725_067),
    (9609, 75_725_067),
    (9610, 432_490_461),
];

/// The original's random state after these ticks of the game's fifth
/// level.
const PAIR_RNG: [(usize, u32); 6] = [
    (3593, 1_210_602_628),
    (3594, 1_210_602_628),
    (3595, 3_685_527_863),
    (3596, 1_590_577_612),
    (5311, 2_180_785_897),
    (5312, 1_373_479_525),
];

/// The town guard that falls on to the spikes: its serial number.
const FALLING_GUARD: u32 = 359;
/// The town guard that comes back on screen: its serial number.
const RETURNING_GUARD: u32 = 222;
/// The town guard pacing to its limit on the game's fifth level.
const PACING_GUARD: u32 = 353;
/// The dead town guard flying past Claw on the game's fifth level.
const FLYING_GUARD: u32 = 161;

fn level5() -> Option<SimLevel> {
    let Some(path) = std::env::var_os("CLAW_INSTALL") else {
        eprintln!("skipped: CLAW_INSTALL is not set");
        return None;
    };
    let install = Install::open(path).expect("install opens");
    let store = install.store().expect("store");
    let level = LevelData::build(&store, LevelSource::Retail(5)).expect("level loads");
    Some(SimLevel::from_level(&level, &store).expect("level prepares"))
}

/// The routes' settings: the original speeds without the mid-air hit fix.
fn config() -> SimConfig {
    let mut cfg = SimConfig {
        tuning: Tuning::original_speeds(),
        seed: LEVEL5_RNG_AT_T0,
        ..SimConfig::default()
    };
    cfg.tuning.behaviour.fix_double_jump = false;
    cfg
}

/// The input script of `runs` (hex held masks with their runs of ticks),
/// `len` ticks long.
fn route(runs: &str, len: usize) -> InputScript {
    let mut held = Vec::new();
    for run in runs.split_whitespace() {
        let (mask, n) = run.split_once(':').expect("mask:ticks");
        let mask = u32::from_str_radix(mask, 16).expect("hex mask");
        let n: usize = n.parse().expect("ticks");
        held.extend(std::iter::repeat_n(mask, n));
    }
    assert_eq!(held.len(), len);
    InputScript::from_held(&held)
}

/// The object with serial number `serial`, if it is in the level.
fn by_serial(sim: &Sim, serial: u32) -> Option<&Object> {
    sim.objects()
        .all()
        .find(|(_, o)| o.serial == serial)
        .map(|(_, o)| o)
}

fn check_rng(sim: &Sim, table: &[(usize, u32)], tick: usize) {
    if let Some(&(_, want)) = table.iter().find(|r| r.0 == tick) {
        assert_eq!(sim.rng_state(), want, "random state after tick {tick}");
    }
}

/// An enemy's exact position: its pixel plus the fractions its last step
/// carried, unless something else has moved it since.
fn exact(o: &Object) -> (f64, f64) {
    let d = enemy::data_of(o).expect("enemy data");
    if d.moved_to == Some((o.x, o.y)) {
        (f64::from(o.x) + d.carry.0, f64::from(o.y) + d.carry.1)
    } else {
        (f64::from(o.x), f64::from(o.y))
    }
}

fn vy(o: &Object) -> f64 {
    enemy::data_of(o).expect("enemy data").vy
}

fn state(o: &Object) -> Option<EnemyState> {
    EnemyState::from_id(o.state)
}

#[test]
fn install_level5_a_standing_enemy_keeps_its_speed_up_and_going_home_keeps_it() {
    let Some(level) = level5() else { return };
    let script = route(BOT_ROUTE, 9611);
    let mut sim = Sim::new(level, config());
    let mut checked = 0;
    for (tick, t) in script.ticks.iter().take(7538).enumerate() {
        sim.step(*t);
        check_rng(&sim, &BOT_RNG, tick);
        let Some(g) = by_serial(&sim, FALLING_GUARD) else {
            continue;
        };
        match tick {
            7261 => {
                assert_eq!((g.x, g.y), (21220, 5416), "built on the floor");
                assert_eq!(exact(g).1, 5389.0, "its exact height where it was placed");
                assert_eq!(vy(g), 0.0);
                assert!(!g.is_listed(), "asleep again");
            }
            7264 => {
                assert!(g.is_listed());
                assert_eq!(exact(g).1, 5416.0, "back on the floor");
                assert!((vy(g) + 326.655_782).abs() < 1e-6, "{}", vy(g));
            }
            7518 => {
                assert_eq!(g.y, 5416);
                assert!((vy(g) + 36.290_185).abs() < 1e-6, "{}", vy(g));
            }
            7519 => {
                assert_eq!(g.y, 5415, "lifted by its speed up");
                assert!((exact(g).1 - 5_415.670_777).abs() < 1e-6, "{}", exact(g).1);
            }
            7526 => {
                assert_eq!((g.x, g.y), (21243, 5431));
                assert_eq!(enemy::data_of(g).unwrap().vx, 0.0, "stopped across");
                assert!((vy(g) - 251.709_815).abs() < 1e-6, "falling on: {}", vy(g));
            }
            7536 => assert_ne!(state(g), Some(EnemyState::Dead)),
            7537 => {
                assert_eq!(g.y, 5520, "on the spikes");
                assert_eq!(state(g), Some(EnemyState::Dead));
            }
            _ => continue,
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn install_level5_an_enemy_back_on_screen_waits_500_ms_before_mode_5() {
    let Some(level) = level5() else { return };
    let script = route(BOT_ROUTE, 9611);
    let mut sim = Sim::new(level, config());
    for (tick, t) in script.ticks.iter().enumerate() {
        sim.step(*t);
        check_rng(&sim, &BOT_RNG, tick);
        if tick < 9607 {
            continue;
        }
        let g = by_serial(&sim, RETURNING_GUARD).expect("the guard");
        let d = enemy::data_of(g).expect("enemy data");
        assert_eq!(d.mode, 6, "mode 6 after tick {tick}");
        match tick {
            9607 => assert_eq!(d.miss_since, None, "no wait while off screen"),
            9608 | 9609 => {
                assert_eq!(d.miss_since, Some(18 * 9608), "the wait starts again");
                assert_eq!(state(g), Some(EnemyState::Pace));
            }
            _ => {
                assert_eq!((g.x, g.y), (24502, 5438), "on its left limit");
                assert_eq!(state(g), Some(EnemyState::Idle));
            }
        }
    }
}

#[test]
fn install_level5_the_movement_step_follows_the_game_clock_and_the_side_the_exact_x() {
    let Some(level) = level5() else { return };
    let script = route(PAIR_ROUTE, 5313);
    let carry = parse_start_lines(PAIR_START).expect("start lines");
    let mut sim = Sim::with_start(level, config(), StartState { carry, save: None });
    let mut checked = 0;
    for (tick, t) in script.ticks.iter().enumerate() {
        sim.step(*t);
        check_rng(&sim, &PAIR_RNG, tick);
        match tick {
            3594..=3596 => {
                let g = by_serial(&sim, PACING_GUARD).expect("the guard");
                let vx = enemy::data_of(g).unwrap().vx;
                let x = exact(g).0;
                match tick {
                    3594 => {
                        assert_eq!(x, 11_414.999_999_999_998, "a hair short");
                        assert_eq!(vx, 120.0);
                    }
                    3595 => {
                        assert_eq!(x, 11_415.0, "on its limit");
                        assert_eq!(vx, 120.0);
                    }
                    _ => assert_eq!(vx, -120.0, "turned back"),
                }
            }
            5311 => {
                let g = by_serial(&sim, FLYING_GUARD).expect("the guard");
                assert_eq!(state(g), Some(EnemyState::Dead));
                assert_eq!(g.x, 16222);
                assert!((exact(g).0 - 16_222.6).abs() < 1e-6, "{}", exact(g).0);
            }
            5312 => {
                let claw = sim.player().expect("Claw");
                assert_eq!(claw.x, 16222);
                let g = by_serial(&sim, FLYING_GUARD).expect("the guard");
                let d = enemy::data_of(g).unwrap();
                assert!(!d.claw_right, "Claw on its left");
            }
            _ => continue,
        }
        checked += 1;
    }
    assert_eq!(checked, 5);
}
