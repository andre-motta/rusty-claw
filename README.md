# rusty-claw

An open-source engine for playing Captain Claw (Monolith, 1997) natively on
modern Linux, with high-resolution and high-refresh output.

## You need your own copy of the game

This repository holds only source code. It contains no game files: no images,
sounds, levels, music or movies, and nothing converted from them. To play,
you need your own legally owned copy of Captain Claw, and the engine reads its
files from that install at run time, in place, without changing them.

Point the game at the folder of your install, the one that holds the
`Assets` folder (an extracted asset tree), `CLAW.REZ` (an asset archive), or
both. Community custom levels are read from the install's `Custom` folder: a
level file `Custom/<name>.WWD` plus an optional folder `Custom/<name>/` with
the level's own tiles, images, sounds, animations, palette and music. Only
those data files are read; scripts or plugins that ship with a custom level
are never run.

## Status

The first level, La Roca, plays from start to finish:

- Claw walks, runs, jumps, climbs, crouches and looks up, tuned to cover the
  same distances in the same times as the original at 60 frames per second.
- He fights with his sword, fists and feet (standing, crouching and in the
  air), the pistol, magic and dynamite, and lifts and throws powder kegs.
- Officers, soldiers and rats guard, pace, strike, shoot, throw bombs,
  flinch and die, dropping their loot.
- Health, lives, damage from enemies, shots and spikes, death and the
  respawn at the last checkpoint, save points, crates, powder kegs,
  treasure, food, ammunition, power-ups, the map piece and the end-of-level
  tally.
- Elevators, crumbling and toggling pegs and the animated scenery; pegs and
  elevators go back in place after a death.
- Sound effects with positional panning, ambient sounds, Claw's and the
  enemies' speech, and the music (with a SoundFont, see below).
- The status bar, screen transitions, the original-style title, main menu,
  options, load game, in-game menu, tally and game over, saved games,
  settings written back to the config file, gamepads and rebindable keys.

The second level, the Battlements, plays from start to finish too: the
punk rats and their little cannons, the tower cannons, enemies falling
into goo, and Raux, the level's boss, with his introduction (or the
jump-in that skips it), the boss health bar, his defeat and the gem that
ends the level. After each level's end-of-level screen the next level
starts with the progress carried, so the first two levels play as one
game. Its original-speed routes, the run from the first level into the
second included, win in the original too and follow it step for step at
one 18 ms step per frame (see [Routes and comparing with the original](#routes-and-comparing-with-the-original)).

Every retail level loads and runs. In levels 3 to 14, the objects the
first two levels do not have (other enemies, bosses, traps and the like)
are drawn where the level places them but stand still; see
[Known gaps](#known-gaps).

## Building on Fedora

You need a C linker, `make`, Vulkan drivers (Mesa's for AMD and Intel
GPUs; the NVIDIA driver brings its own), the ALSA development package (for
sound) and the udev development package (for gamepads), plus the Rust
toolchain from [rustup](https://rustup.rs).

```sh
sudo dnf install gcc make git curl vulkan-loader mesa-vulkan-drivers alsa-lib-devel systemd-devel \
    gcc-c++ pkgconf-pkg-config meson ninja-build
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # then open a new shell
git clone https://github.com/andre-motta/rusty-claw.git
cd rusty-claw
make soundfont                                  # optional: the music's instruments
make run-game ARGS='--install /path/to/your/install'
```

The packages on the second line are what the movie build needs to decode
AV1: `pkgconf-pkg-config` (pkg-config), `meson` and `ninja-build` (ninja),
and `gcc-c++`, a C++ compiler, with which the build makes ninja itself
when the system has none; see below.

On Debian and Ubuntu the packages are `build-essential git curl
libvulkan1 mesa-vulkan-drivers libasound2-dev libudev-dev pkg-config meson
ninja-build` (`pkg-config`, `meson` and `ninja-build` let the movie build
decode AV1, and `build-essential` brings the C++ compiler; see below).

Movies come with a plain `cargo build` (and with `make run-game`) on Linux
and the Steam Deck: the first build downloads the sources of FFmpeg and of
dav1d (its AV1 decoder) and builds them once per machine into your cache
(about a minute, no root needed; `make ffmpeg` does that step on its own).
AV1, the upscaled movies' format, is an extra: dav1d also needs
`pkg-config`, and meson and ninja, which the build downloads (pinned and
checked) and builds into your cache when the system has none, with
`python3` (3.7 or newer) and a C++ compiler. When AV1 cannot be had, the
build goes on without it, cargo says why in a warning, and the original
movies still play. Where FFmpeg cannot be built at all, cargo prints one
warning and the game builds without movies.
`RUSTY_CLAW_FFMPEG=off` builds without FFmpeg on purpose,
`RUSTY_CLAW_FFMPEG_AV1=off` without AV1, and `RUSTY_CLAW_FFMPEG=require`
makes a build without FFmpeg, or without AV1, fail. On Windows, run
FFmpeg's Windows build script once first, or the game builds without
movies; see [third_party/ffmpeg/README.md](third_party/ffmpeg/README.md).

The first build downloads the pinned Rust version from `rust-toolchain.toml`
and the dependencies, and takes a few minutes. The game binary is
`target/release/rusty-claw`; `make run-game ARGS=...` builds and runs it.

The game needs a desktop session (Wayland or X11) and a GPU with Vulkan
support, which in practice means GPUs from about 2016 on.

## Playing

```sh
rusty-claw --install /path/to/your/install                 # the title and main menu
rusty-claw --install /path/to/your/install --level 4       # straight into level 4
rusty-claw --install /path/to/your/install --custom "My Level.WWD"
```

With only `--install` the game starts at the title, then the main menu:
new game (pick a level you have opened), load game (a level's start or one
of its two save points), options (sound, video, controls) and quit.
`--level` and `--custom` start straight into that level. In play, Escape, P
or the gamepad's Start button open the in-game menu: resume, options and
end the game. Finishing a level shows the treasure tally, then the main
menu; the next level is then open in the new game and load lists.
Finishing the fourteenth level shows its map and tally, then the credits
over the parchment (or over a looping movie, see [Movies](#movies));
Escape, Enter, a game key or a click returns to the main menu, whose
Credits entry shows them again. When the game is built
with FFmpeg it also plays the game's movies (see [Movies](#movies)).

The game keeps running when its window loses focus (Claw takes no input
there); `pause_in_background = on` in the config file pauses it instead,
as the original does.

### Keyboard

| Key | Action |
| --- | --- |
| Left, Right | walk; keep walking to run |
| Up | climb up; look up when standing |
| Down | climb down; crouch when standing |
| Space | jump; hold for a higher jump |
| Ctrl | attack: sword, punch, uppercut or kick; crouching and in the air, a sword swipe |
| Alt | fire the selected weapon |
| Shift | switch weapons: pistol, magic, dynamite |
| 1, 2, 3 | select and fire the pistol, magic or dynamite |
| Z | lift an object in front of Claw; again to throw it (hold to throw farther) |
| Escape, P or Pause | the in-game menu |
| F6 | play-test aid: the next powerup, only while an aid is on (see [Play-test aids](#play-test-aids)) |
| F7 | smooth motion between game ticks on or off |
| F8 | fit or integer scaling |
| F9 | aspect: fill, centered, 4:3 |
| F10 | next window size |
| F11 | fullscreen |

Keys are matched by their position, so the map is the same on any keyboard
layout. Hold the fire key longer to throw dynamite farther.

### Gamepad

Any standard gamepad works, the Steam Deck's controls included:

| Pad | Action |
| --- | --- |
| d-pad or left stick | move, climb, crouch, look up |
| South (A) | jump |
| East (B) | attack |
| West (X) | fire the weapon |
| North (Y) | switch weapons |
| left shoulder | lift and throw |
| right shoulder, left trigger, right trigger | pistol, magic, dynamite |
| Start | the in-game menu |

The menus also follow the d-pad, South to choose and East to go back.

### Rebinding

The controls page of the options lists every action with its keys and pad
buttons. Choose an action with the keyboard and press a key, or choose it
with the gamepad and press a pad button: that becomes its binding. A key
belongs to one action at a time, so taking a key from another action swaps
the two; the quit, pause and display keys cannot be taken. The same page
sets the stick dead zone and restores the default bindings. Bindings are
saved in the config file as `key_<action>` and `pad_<action>` lines (see
below), which can also be edited by hand.

## Sound and music

The game plays the sounds of your install: every sound of a level is read
and decoded when the level loads. It uses the default output device of
your desktop (PipeWire, PulseAudio or ALSA); without one it runs silently
and says so once. Sounds pause with the game; the music holds and carries
on.

Music is the game's own score, played through a General MIDI SoundFont (an
`.sf2` instrument bank). None comes with the source. The default bank is
GeneralUser GS by S. Christian Collins, under its own permissive licence:

```sh
make soundfont
```

fetches a pinned copy into `~/.local/share/rusty-claw/soundfonts/` (or under
`$XDG_DATA_HOME`) and checks its SHA-256. On Windows that user folder is
`%LOCALAPPDATA%\rusty-claw\soundfonts\` unless `XDG_DATA_HOME` is set. The game takes the first usable
bank from, in order: the `soundfont` setting, the `CLAW_SOUNDFONT`
environment variable, a packaged game's `share/rusty-claw/soundfonts/`,
that user folder, and the common system paths where distributions install
FluidR3_GM. Without a bank, music is off, one message says how to get one,
and the sound effects are unaffected.

| Option | What it does |
| --- | --- |
| `--no-audio` | no sound; no audio device is opened |

In the config file, `audio = off` does the same. The sound options set the
sound and music volumes and turn music, speech and ambient sounds on or
off.

A replay watched in the window (`--replay FILE.csv`) plays the sound and
music as the game does, from the level's start through the handover to
the keyboard, with your sound settings; its sounds pause where the
recording paused or opened a menu. `--no-audio` or `audio = off` keeps it
silent. Replays that write a trace (`--trace`), benchmarks and headless
runs are always silent.

## Movies

A game built with FFmpeg plays the movies of your install where the
original does, full screen at the movie's own shape on black, with its
sound at the sound volume:

- at start-up, before the title: the company logo, and on the first run
  the intro after it. Once the intro has played to its end or was
  skipped, the game notes it in the config file (`intro_seen = yes`; an
  intro that could not play or be shown is not noted) and later runs
  start with the logo only, or with nothing at all with `skip_logo = on`.
  `skip_logo` waits for the intro only when the intro plays on that run:
  without an intro to play it skips the logo on the first run too;
- after levels 2, 4, 6, 8, 10, 12 and 13, before the map screen, and the
  ending after level 14, before its map screen and the credits;
- in the main menu when nobody touches it for two minutes (three in a
  submenu): the intro and the title in turn, the intro first;
- from the main menu's Replay Movies page, one entry per movie; an entry is
  greyed out when its movie is not found, and the page when none is;
- behind the credits, looping and without its sound (the credits music
  plays as before), in place of the parchment: your own copy named
  `CREDITS` (see below), else the long clip of the install's movie
  archive. The same clip plays after level 14 and from the main menu's
  Credits; the archive's other clip for the end of the game holds only the
  same short placeholder as its first ten clips, so it is not used. The
  credits text, its pace, the corner extra and the keys stay as they are.
  Without a credits movie, with `credits_movie = off` or with movies off,
  or when the movie cannot play or stops on an error, the credits show
  over the parchment.

A run straight in a level (`--level`, `--custom`) skips the start-up
movies, community custom levels have none, and replays, traces,
benchmarks and the tools never play any. `--no-movies` turns them off for
one run, and `--movie INDEX|FILE` plays one movie and exits. INDEX is one
of `-2` (the attract clip), `-1` (the logo), `0` (the intro), `2`, `4`,
`6`, `8`, `10`, `12`, `13` (after those levels) and `14` (the ending);
the odd numbers 1 to 11 name no movie and are refused. FILE is any movie
file. `--movie` exits with status 0 when the movie played to its end or
was skipped, and with status 1 and a one-line message when it was not
found, could not be opened or shown, or stopped before its first picture,
so a script can check your own copies with it.

Escape, or the gamepad's Back or Start button, skips a movie file; any
key, click or gamepad button skips a clip of the movie archive. The
window's own keys (F11 fullscreen, F10 window size, F9 aspect and F8
scaling, or the keys you bound to them) work during a movie and never
skip it. Keys pressed during a movie do nothing on the screen after it.
Pictures show ahead of the sound by the sound output's latency.

Movies, like play, keep going while the game's window is in the
background. With `pause_in_background = on`, a movie holds there instead,
its picture and its sound, and goes on from where it was when the window
is back in front, as play pauses. A movie without sound, or one already
past the end of its sound, has no soundtrack to hold it: it shows no new
picture in the background and goes on at its own time. The main menu's
idle time does not count in the background either, so the intro never
starts while nobody sees it.

Where movies come from, best first:

1. Your own copies, for example upscaled ones, in
   `~/.local/share/rusty-claw/movies/` (or under `$XDG_DATA_HOME`; on
   Windows `%LOCALAPPDATA%\rusty-claw\movies\` unless `XDG_DATA_HOME` is
   set), or the folder `movie_folder` names. Name them after the movie they
   replace, `LOGO`, `CLAW0` to `CLAW8` and `CREDITS`, with the extension `.mkv`,
   `.webm`, `.mp4`, `.mov` or `.avi` (in any case); AV1 video (8-bit or
   10-bit) with Opus sound in `.mkv` or `.webm` is the best choice, and
   plays in every build with AV1, which is every build that could make
   dav1d (cargo warns when it could not; 2560x1920 AV1 decodes at about
   5 times real time on four cores of a desktop CPU; see
   [third_party/ffmpeg/README.md](third_party/ffmpeg/README.md)).
2. The install's own movie files, `LOGO` and `CLAW0` to `CLAW8` as `.avi`
   or `.vob`, in the install folder or its `Movies` folder.
3. The install's movie archive, `CLAW.FEC` (for the credits, only this
   and your own copy count).

No movie file comes with the source, and the game never writes one.

| Setting | What it does |
| --- | --- |
| `movies = upscaled` | your copies first, then the install's movies (the default) |
| `movies = original` | only the install's movie files, then its movie archive |
| `movies = archive` | only the movie archive, as the original does with DVD movies off |
| `movies = off` | no movies |
| `skip_logo = on` | no logo at start-up, once the intro has played or when there is no intro to play |
| `movie_folder = "/path"` | where your copies are |
| `intro_seen = yes` | written by the game once the intro played; `no` shows it again |
| `credits_movie = off` | the credits show over the parchment even when a credits movie is found (`on`, the default, shows the movie behind them) |

The Display page's movie interlacing switch is kept for the run only and
changes nothing: movies always show whole.

Movies need FFmpeg, which the game's build links in;
[third_party/ffmpeg/README.md](third_party/ffmpeg/README.md) has the
details for every platform. On Linux and the Steam Deck the first `cargo
build` (or `make run-game`) builds FFmpeg once per machine and the game
with movies; on Windows, FFmpeg's Windows build script runs first. A game
built without FFmpeg (it could not be built, or `RUSTY_CLAW_FFMPEG=off`)
plays no movies, says so once at start-up (pointing at that README), and
goes on as if the install had none.

The build has no H.264 or HEVC (H.265) decoder, and nothing published
carries one, because they are patent-pooled formats; the install's movies
never need them. If you build the game for yourself, you can add them with
`RUSTY_CLAW_FFMPEG_H26X=on` set for `make ffmpeg` and for cargo alike (see
that README).

## Settings and saved games

Settings live in a config file, `~/.config/rusty-claw/config.toml` (or under
`$XDG_CONFIG_HOME`; on Windows `%APPDATA%\rusty-claw\config.toml` unless
`XDG_CONFIG_HOME` is set), or any file passed with `--config FILE`; command line
options win over it. Options changed in the menus are written back to that
file: only the changed lines change, every other line and comment stays as
it is, the previous file is kept as `config.toml.bak`, and values given on
the command line are not written back.

```toml
install = "/path/to/your/install"
level = 1                  # or: custom = "My Level.WWD"
res = "2560x1440"
fullscreen = false
aspect = "fill"            # fill, centered or 4:3
scale = "fit"              # fit or integer
widen_activation = false
mid_air_hit_fix = off      # on: no second jump after a hit in mid-air
invincible = off           # play-test aid: Claw takes no damage
infinite_lives = off       # play-test aid: a death never costs a life
vsync = true
fps_cap = 300              # with vsync off; 0 for no cap
interpolation = true
smooth_rope = on           # off: rope swings move in the rope's own steps
status_bar = "edges"       # edges, or centred in the 4:3 area
audio = on                 # on or off
sound_volume = 80          # 0 to 100
music_volume = 50          # 0 to 100
voice = on                 # Claw's and the enemies' speech
ambient = on               # ambient sounds
soundfont = "/path/to/a/soundfont.sf2"
gamepad = on               # on or off
pause_in_background = off  # on: play and movies hold while the window is in the background
pad_dead_zone = 0.34       # stick dead zone, 0.05 to 0.95
movies = upscaled          # original, archive or off (see Movies)
skip_logo = off            # on: no logo movie at start-up unless the intro plays
movie_folder = "/path/to/your/movies"
intro_seen = no            # the game sets it once the intro movie played
credits_movie = on         # off: the credits show over the parchment
key_jump = "Space"         # key_<action>: key names, comma separated
pad_jump = "South"         # pad_<action>: pad button names
```

The bindable actions are `left`, `right`, `up`, `down`, `jump`, `attack`,
`projectile`, `toggle`, `lift`, `pistol`, `magic`, `dynamite`, `quit`,
`fullscreen`, `window_size`, `aspect`, `scale`, `interpolation`, `pause`,
`help` and `powerup`.

By default a hit that ends while Claw is in the air leaves him able to jump
again before he lands, as in the original; speedruns rely on it, and any
hit works, an enemy's shot or your own dynamite. The mid-air hit jump fix
turns that off: Claw falls after such a hit. Turn it on with
`mid_air_hit_fix = on`, `--mid-air-hit-fix on` or the gameplay options
(Options, then Gameplay); it applies to the games started after the
change.

### Play-test aids

Three aids help with trying out the hard parts of a level and every
powerup. All are off by default:

- `--invincible`: Claw takes no damage from enemies, shots, hazards,
  explosions or bosses. He never goes into the hurt reaction and is never
  knocked back, so he moves as he does in normal play. Death tiles and pits
  still end his life.
- `--infinite-lives`: losing a life never lowers the lives count, so play
  goes on from the last checkpoint.
- `--powerup NAME[:SECONDS]`: Claw has that powerup at the start of every
  level and again after every respawn, given as a pickup gives it (its
  tune, voice line, look and status bar timer). NAME is `invisible`,
  `invulnerable`, `catnip`, `fire`, `ice` or `lightning` (the three
  swords; the plasma sword's pickup does nothing in the game, so it is
  not offered); SECONDS defaults to 30, the length of a pickup that names
  none. For example `--powerup fire` or `--powerup catnip:90`.

While any of these aids is on, F6 gives Claw the next powerup in that order
for 30 seconds, then none (ending the running one), then the first again,
and names it in the middle of the screen for two seconds: press it in turn
to see each powerup's look and effect. With every aid off F6 does nothing.
F6 is the `powerup` action (`key_powerup` in the config file); F9 stays the
aspect key. A config that bound F6 to another action before must move
`key_powerup` to a free key first. F6 does nothing during a replay or a
bench run.

The first two switches are also on the Gameplay page (Options, then
Gameplay) and in the config file (`invincible = on`, `infinite_lives = on`).
Like the other gameplay switches they apply from the next level started.
The powerup aid is command line only. A flag on the command line turns its
aid on for that run only and is not saved. A small marker in the
bottom-left corner of the screen names each aid that is on (`POWERUP FIRE
SWORD` for `--powerup fire`).

A run with an aid on says nothing about how hard a level is. A recording
made with one says so in its header (`# invincible = on`,
`# infinite_lives = on`, `# powerup = fire:30`, and `# powerup_key = used`
when F6 gave a powerup, each one noted where it was given), and `--replay`
plays it with that aid and those powerups. Routes, the
search bot and the comparisons with the original (`make compare`,
`claw-replay --compare`) refuse such a recording, with a message that says
why.

Saved games are kept in `~/.local/share/rusty-claw/saves/default/` (or
under `$XDG_DATA_HOME`; on Windows `%LOCALAPPDATA%\rusty-claw\saves\default\`
unless `XDG_DATA_HOME` is set). Touching one of a level's two save points saves
the game there, and finishing a level saves the start of the next one with
the score, health, lives, ammunition and treasure carried over. Every write
is atomic and keeps the previous file; a damaged save is moved aside, never
deleted. Community custom levels are not saved.

## Resolution, aspect and refresh

The game always plays in the original 640x480 game view and runs its logic
in fixed 18 ms steps; drawing happens at your display's refresh rate, with
positions interpolated between steps, at any resolution.

| Option | What it does |
| --- | --- |
| `--res WxH` | window size, for example `2560x1440` (default `1280x960`) |
| `--fullscreen` | borderless fullscreen at the monitor's resolution |
| `--aspect fill` | fill a wide display (default); objects in the extra width stay still until Claw comes near, as they would off screen |
| `--aspect centered` | fill a wide display around the game view, showing past the level's edges |
| `--aspect 4:3` | the original 4:3 view with bars at the sides |
| `--scale fit` | fill the display height: whole-pixel steps, then a smooth remainder (default) |
| `--scale integer` | whole-pixel steps only, with bars |
| `--widen-activation` | with `fill`, keep everything visible on a wide display active; this changes gameplay |
| `--vsync on\|off` | wait for the display's refresh (default on) |
| `--fps-cap N` | with vsync off, draw at most N frames per second (default 300) |
| `--uncapped` | no frame-rate cap with vsync off (same as `--fps-cap 0`); for measuring throughput only |
| `--no-interp` | draw the latest game tick only |
| `--smooth-rope on\|off` | draw rope swings smoothly between the rope's frames (default on); off shows the rope's own steps |
| `--pack DIR` | an image pack with higher-resolution replacement images |

A swinging rope steps through its frames a little slower than the game's
18 ms steps, so Claw, the rope's end and the view following him stand still
on some steps; at 60 Hz they would move on about two frames in three, a
stepped look the original has too. The smooth rope swing (on by default,
`smooth_rope` in the config file) draws them on a smooth path between the
rope's frames instead. It changes only what is drawn: the game, its
recordings and its replays are the same either way. It needs the
interpolation between steps (`--no-interp` turns both off).

The status bar sits at the edges of the picture; on a wide display
`status_bar = "centred"` (or the video options) keeps it in the 4:3
area. Window size, aspect, scaling and fullscreen also change while playing
(F8 to F11, or the video options); the game view and the game itself stay
as they are.

The game never draws as fast as the GPU allows unless asked to: with vsync
on, the display's refresh paces drawing, and with vsync off, the frame-rate
cap does. Drawing thousands of frames a second gains nothing, wastes power
and makes some graphics cards whine audibly.

## Replays and benchmarks

An input script is a CSV file with the columns `tick,held_mask,pressed_mask`,
one row per 18 ms game tick (masks: Jump 0x1, Attack 0x2, Fire 0x4, Switch
weapon 0x8, Lift 0x20, Pistol 0x40, Magic 0x80, Dynamite 0x100, Left
0x1000000, Right 0x2000000, Up 0x4000000, Down 0x8000000).

`--record FILE.csv` writes the input of every tick you play in the window
to such a script when the game ends; replaying it with the same level and
start gives the same game. The recording's first line, a `#` comment, notes
whether the mid-air hit jump fix was on, and the replay follows it. Replays
and benchmarks never take that option from the config file: without such a
line, or for a benchmark, it is off unless `--mid-air-hit-fix on` is given.
Recordings made before this option existed were played with the fix on and
have no such line, so replay them with `--mid-air-hit-fix on`. A recording
made with a play-test aid on names it in its header, and the replay plays
with it (see [Play-test aids](#play-test-aids)); `claw-replay` accepts the
same `--invincible`, `--infinite-lives` and `--powerup` flags.

A replay in the window plays the game's sound and music (see
[Sound and music](#sound-and-music)); add `--no-audio` to watch it silently.

```sh
# play and record, then watch (and hear) the recording
rusty-claw --install PATH --level 1 --record game.csv
rusty-claw --install PATH --level 1 --replay game.csv
# no window: write one trace row per tick, with a simulated 144 Hz display
rusty-claw --install PATH --replay walk.csv --trace out.csv --render-hz 144
# benchmark: the combat route for 10 s, statistics as JSON
make bench ARGS='--install PATH --level 1 --bench 10 --res 3840x2160'
# the same, drawn offscreen at the full size with a simulated display
make bench ARGS='--install PATH --level 1 --bench 10 --res 5120x1440 --headless'
```

Traces are identical at every display rate.

The benchmark plays a combat route: Claw starts before the stretch of the
level where the most enemies stand together and takes them on one after
the other, with his sword and the pistol, so enemies fight back and shots,
bursts and pickups fly while frames are measured. The route is planned on a
copy of the game before the run and played once; it lasts a run paced at
the display rate, and the verdict says when an unpaced run outran it. The
offscreen benchmark also plays the sounds and music through a silent
output on the audio thread, so their CPU load is present during the run,
and the handling of the sound and music cues is part of each frame's
measured work.

The report is one line of JSON on standard output: the CPU work per frame,
the cost of each game tick and the number of live objects, frames that
missed the display's refresh (in a window) or GPU time (offscreen), an
estimate for a handheld-class CPU, the route and what it met (enemies
killed, the most enemies and shots live at once). A second line on standard
error gives the verdict, PASS or FAIL, with each budget: frame work under
6.9 ms at the 99th percentile, the handheld estimate under 8.3 ms, and no
missed refresh. Missed refreshes can be counted only in a window with vsync
on; offscreen or with vsync off the line says they are not measurable. The
benchmark exits with status 3 when a measured budget is missed. For each
missed refresh, the JSON describes that frame (its work, the waits around
it, sprite loads and window changes) and names the likely cause, to tell a
slow frame of the game from a stall in the desktop's compositor.

The offscreen benchmark draws at its simulated display rate (`--render-hz`,
144 by default), as a real display would; per-frame times never include
that wait. Add `--uncapped` to draw as fast as the machine allows, for
measuring throughput. When the desktop is smaller than `--res`, a window
benchmark gets what fits and every frame is also drawn offscreen at the
requested size.

## Routes and comparing with the original

A route is an input script that plays a level from its start. Besides the
rows, it can name how it is played in `#` header lines (`# level N`,
`# tuning default|original`, `# seed N`, and the behaviour switches it was
made with), and it can hold `@` lines for what happens between two ticks:
a pause or a menu screen with its wall time, or the in-game menu's end of a
life. Replays, `claw-replay` and the benchmark all follow them the same
way. The game holds while the in-game menu is up, but the menu's frames
keep the enemies' rhythm going: enemies make some of their random choices
only on every third frame, a beat the game counts on every frame it
shows, menus included. A visit to the in-game menu (a `menu ingame`
line, or the menu opened in live play) moves that beat on by one frame
per 18 ms of the menu (rounded up, and at least the frame it opens
with), and an End Life (`command end_life`) by the five frames of its
visit to the menu (it opens, its confirmation page takes three, the yes
one), or four after a `menu ingame` line at the same tick, which already
opened it. Live play, `--replay`, `--trace`, the benchmark,
`claw-replay` and the bot's runs all count them alike, so a recording
plays the same game everywhere.

The routes in `tests/acceptance/routes/` finish all fourteen
levels: `level1.csv` to `level8.csv`, `level10.csv`, `level12.csv` and
`level13.csv` with the default tuning (the ninth, eleventh and fourteenth
levels have no default-tuning route yet), `level1-original.csv` to
`level14-original.csv` with the original game's speeds (the ninth,
eleventh, twelfth, thirteenth and fourteenth levels' ways are described
below),
`level2-introskip.csv`,
`level4-introskip.csv`, `level6-introskip.csv`, `level8-introskip.csv`,
`level10-introskip.csv`, `level12-introskip.csv` and
`level13-introskip.csv` with the jump-in that skips Raux's, Katherine's,
Wolvington's, Gabriel's, Marrow's, Aquatis's and RedTail's introductions,
`level3-boost.csv` with
the dynamite boost on to the rock pillar past the third level's first save
point (`level5-original.csv` and `level4-original.level5.csv` use one too,
on to the block past the fifth level's last stack of pegs), and
`level1-original.level2.csv`, `level2-original.level3.csv`,
`level3-original.level4.csv`, `level4-original.level5.csv`,
`level5-original.level6.csv`, `level6-original.level7.csv`,
`level7-original.level8.csv`, `level9-original.level10.csv`,
`level10-original.level11.csv`, `level11-original.level12.csv`,
`level12-original.level13.csv` and `level13-original.level14.csv`, the next
level of the game the route before starts, which begin with the progress
that game carries in (their
`# start_score = ...` lines, which replays, `claw-replay` and the
comparison apply).

The folder `tests/acceptance/routes/game/` holds one game played from
the first level on through every booty screen, named as a recorded game
names its files: `level1-original.csv` (the same route as
`routes/level1-original.csv`), then `level1-original.level2.csv` and on,
one file per level, up to `level1-original.level14.csv`, the end of
the game. Each later level starts
with the progress the game's level before it ends with (more score and
other health, ammunition and weapon than a two-level game brings in). It
plays its two-level game's rows where they still win from there (the
second, third, fourth, sixth, eleventh, twelfth, thirteenth and
fourteenth levels, the last with its one End Life), and rows the bot
searched again
from that start where they do not: from the third checkpoint on in the
fifth level, from the tenth stretch on in the seventh (which first
selects the pistol, as the game it comes from carries in the dynamite)
and from the start's dock on in the eighth (the game carries in no pistol
shots, so Claw clears the docks with his sword) and from the eighth
stretch on in the tenth (which first changes weapon to the pistol). The
ninth level, which has no two-level game, takes another way than
`level9-original.csv`: that route needs more health than the game brings
in and a pistol shot at a powder keg, so Claw first breaks the stacked
crates by the start for their bottle of health and picks up more on the
way up, and climbs the shaft on four sticks of dynamite with the help
screen, the fourth in place of the keg's shot
(`tools/bot/hints/level9-original.txt` gives its parts). Where a stretch needs
the level's clock where the earlier route had it (the toggling pegs and
the dynamite boost of the fifth level, the air carts of the seventh, the
swinging ropes and the Catnip of the eighth, the ropes of the ninth),
the route first waits
until then and plays the earlier route's rows of that stretch. No level
of it loses a life but the fourteenth level's End Life. `claw-replay
--next` plays such a chain:

```sh
claw-replay --install PATH --script game/level1-original.csv \
    --next game/level1-original.level2.csv ... --next game/level1-original.level14.csv \
    --trace out/game.csv --expect-complete
```

Each level starts with the progress the level before hands on (with
the perfect tally's extra life, as the booty screen gives it); a route
whose `# start_...` lines name other progress is reported, with the
lines the game carries in, and the run writes one trace per level
(`out/game.csv`, `out/game.level2.csv`, ...), so each level's part can
be compared on its own. `--carry-out FILE` writes the progress the last
level hands on as start lines, for `claw-bot --carry`. The install test
`install_routes_play_the_whole_game_through_every_booty_screen` plays
that game through the game's flow, level after level, and on through
the last booty screen to the credits (a few seconds in a release build):

```sh
CLAW_INSTALL=PATH cargo test --release --locked -p claw-acceptance --test install_routes -- \
    install_routes_play_the_whole_game --nocapture
```

`claw-bot --seed level` uses the
random state the level's set-up leaves, as the routes do, and
`--hints-on-foot` counts a hint point as reached only with Claw on his
feet, on a ladder or on a rope (a point written `x,y foot` in the hints
file counts that way on its own). Held in a bear's hug, the bot presses Up
and Down in turn to break free; thrown by a springboard or riding an
elevator, it counts Claw as near as the throw or the ride takes him; a
warp counts as the way to where it puts him. Against Gabriel, whom no
blow hurts, the bot lifts him and throws him, from a tap to the full
charge, and is guided by how far he is from the water beside his dock.
A throw may land him anywhere: an enemy that lands on a platform object
(a peg or an elevator) rides it, and while it rides, its movement moves it
by the platform only, so a thrown Gabriel that lands on a peg reaching
past the dock's edge bounces from where he landed and comes to rest on
the peg at his second bounce. Walking off a ledge with Gabriel (or any
enemy) in his hands, Claw drops him rather than throwing him: he keeps
Claw's walking speed across while he falls, but neither bounces nor pays
for the landing, and gets up a second later; the eighth level's routes
that sit through the introduction walk off Gabriel's deck with him this
way before they throw him into the water.
On the eighth level no jump of Claw's own reaches Gabriel's deck:
`level8-introskip.csv` takes the Catnip near the last checkpoint, and
while it lasts its jump-in, a short hop, lands Claw on the dock and a
jump from there lands him on the deck; the routes that sit through
the introduction get up with a dynamite boost (written into them by hand,
as the third level's is; `tools/bot/hints/level8-original.txt` lists the
held buttons).
`level9-original.csv` takes the warp high above the ninth level's start,
which puts Claw at the foot of the shaft below the level's end, and climbs
the shaft in mid-air: three times it throws a stick of dynamite near the
top of a jump with the help screen up for three seconds after the throw
(`@N menu help 3000` lines), so the stick goes off at once beside him, the
blast stops him in mid-air and a jump pressed on the tick he stands there
is a fresh jump; the fourth blast is a powder keg's, set off with a pistol
shot from below the bridge it stands on. The dynamite jumps reach their
height only with the Catnip PegLeg drops on the deck by the first saw.
When play comes back from a help screen, the game runs every object once
more at the game time plus the screen's time (a stick whose fuse has burnt
by then goes off there; Claw, in mid-jump, does not move) and the frame
pulse moves on by the screen's frames; that redraw ends as any frame does,
so an object the pass carried out of the area around the camera (an enemy
walking away from Claw at its edge) is asleep in the next tick, and no
object wakes there; then the clock goes back, so an
enemy that pass moved goes back as far in the next tick; Claw's own call
in that pass sees no key pressed, so a key pressed just before the screen
acts once. A powder keg's
blast reaches the box of the picture it shows as it goes off, which on
this level spares a crate beside the keg by the ladders near the start,
and it hurts in the tick the keg goes off, so Claw falling past the keg
on the bridge is hurt there at once and jumps on to the bridge from the
stand that follows (a tick later he would drop past it on to a warp that
sends him back near the start). After a warp moves Claw, the game runs
every other object once more with no game time, so an enemy the warp
wakes where it puts him makes the calls of a tick there.
The bot and `claw-replay` play a route's help screens and pauses as the
game does, through one shared function, and the bot keeps them when it
carries on from a route (refusing one recorded with another pause or
screen switch than the original's). `claw-replay` follows a recording's
`# fuse_clock` line (with `frozen` a help screen's return runs no pass)
and refuses one recorded with another `pause_input` than the original's.
`tools/bot/hints/level9-original.txt`
lists the held buttons of the stretches written by hand (the fight with
PegLeg, the walk back to the springboard, the jump into the warp and the
climb).
The tenth level's routes take the long way, checkpoint to checkpoint:
past the spike pit no jump crosses before the fifth checkpoint they climb
the ladder to the room above it, and before the sixth they climb a
column of toggling pegs. In Marrow's room the routes that sit through
the introduction play his whole fight: in each round, with the pit open
between Claw and Marrow, Claw waits for the parrot's dives out of
Marrow's sight and hits it three times, so it perches and the slabs
close into a bridge; he crosses and hits Marrow until Marrow whistles
and the parrot carries him across while the pit opens again. The bot is
guided round by round (the parrot's hits, then Marrow's health, each
round counting for more than any state within it), waits through the
parrot's patrol without trying every move under it, and is searched
with `--tick-cost 0`, so the waits for the dives cost nothing. Which
ledge Claw waits on is told by the ends of the pit of the retail level's
room (`MARROW_PIT` in `tools/bot/src/actions.rs`), so that wait is tuned
to this room and does not carry over to a community custom level's.
An enemy's move up or down that would take it past the top or bottom
of its range puts it there and keeps the speed the move's energy gives,
with no gravity added, so the parrot carrying Marrow high across the
room stays on its flight line to the carry's end, with Marrow hanging
under it, level with Claw's ledge. A move that stays within the range
but is held to the 24 px a step allows still gains gravity's speed for
the step on top of the speed the held move's energy gives, so a hermit
crab hatched in the seventh level and woken after a long sleep falls as
fast as in the original and makes its random roll on the same tick.
After a death the game shows a frame
of its own at the checkpoint once the respawn's passes are over (a warp
does too), so an enemy that must be on screen to see Claw, as Marrow
put back on his ledge must, sees him in the first tick after the
respawn. With both, `level10-original.csv` was searched again from the
end of Marrow's first carry (tick 14799) and `level10.csv` from tick
13940, where their games first change.
Holding Jump raises a jump only through the first quarter
second of game time after its press, and the respawn's passes take no
game time: a jump pressed in the last tick of a death starts in those
passes and keeps the whole quarter second for the ticks after them.
The parrot keeps the point beside Marrow where it takes hold of him to
the part of a pixel, and takes hold of him once it has flown past that
exact point, so on some carries it grabs him a tick earlier or a tick
later than a whole pixel would let it (earlier when it comes from the
right, later when it comes from the left). Standing, Marrow's height creeps a part of a pixel
down a tick and back; a drop onto the floor stops it, and the end of his
next whistle starts it again, so the parrot's flight and the carry aim
at where he really stands. When Claw comes back after a death, every
shot still flying is removed before anything runs, so a shot fired at
the dead Claw never hits him after the respawn. With these,
`level10-original.csv` was searched again from Marrow's third carry
(tick 17358); the other tenth-level routes play as before.
An enemy tests how far away Claw is with range boxes centred on its
exact position, which only its own movement step brings in line with
where it is drawn. When the parrot drops Marrow onto a ledge (and when
his wait for the parrot runs out), the drop moves only where he is
drawn, so in the step after a carry his boxes are still at the height
he hung from: Claw standing inside them measured from the ledge can be
out of them, and Marrow shoots instead of running at him. The search
keeps Claw clear of the edges of the boxes built from either height as
Marrow lands (`MARROW_BOX_CLEAR` in `tools/bot/src/search.rs`), since a
pixel there decides what Marrow does. With this, `level10-original.csv`
was searched again from the step after Marrow's third landing (tick
17483) and `level10.csv` from tick 14860, before his second landing;
`level10-introskip.csv` and `level9-original.level10.csv` play as
before.
`level10-introskip.csv` jumps in, then crosses the open pit as the
original's players do: a stick of dynamite thrown in the air, a jump off
the ledge's edge into its blast, and a fresh jump pressed on the one
tick Claw stands in mid-air after the blast's hit, which lands him on
Marrow's ledge; Marrow, who never fights after the jump-in, falls to
Claw's sword there. `tools/bot/hints/level10-original.txt` lists that
crossing's held buttons.
The eleventh level's map piece lies in a pocket just below the start
room, sealed off from it, so its routes go round the whole level: right
along the upper caves to the first super checkpoint, down the shaft on
the level's right edge, back left through the lower caves past the
second super checkpoint, and up the ladder at x 3100 to the ledge under
the pocket, one jump from the map piece. The bot is led along most of
the way with waypoints, since its coarse graph counts some gaps as
crossable that no jump crosses. Three crossings are written into
`level11-original.csv` by hand: the gap of 192 px between two pillars
in the lower caves (Mercat stands on the far one) and the climb from the
top of a ladder to the ladder at x 3100, which ends higher than a jump
reaches, are each made with a stick of dynamite thrown near the top of a
jump with the help screen up for three seconds after the throw, so the
blast stops Claw in mid-air and a jump pressed on the tick he stands
there is a fresh jump; and the row of toggle pegs left of the checkpoint
at x 8497, which come out one after another in a wave, is crossed after
a wait on the checkpoint's ledge, so Claw is on the last peg early
enough in its time out to jump up to the pillar beyond.
`level10-original.level11.csv` plays the same rows from the progress
`level10-original.csv` ends with. `tools/bot/hints/level11-original.txt`
lists the waypoints and the hand-written stretches' held buttons.
A Siren's scream takes Claw's controls for two seconds: while it does,
the bot tries only the wait and the walks, and counts the time as part of
the state, so a wait through the lockout is progress. Near a toggle peg
it counts where the peg is in its cycle, to a tenth of a second, and
beside a laser whether the beam hurts and the time, so a wait for a peg
to come out or a beam to stop is progress too; and it counts each
enemy's health, so shots that wear down an enemy that parries some of
them are progress (outside a boss room, where the boss's own health is
the progress and the pirates a boss calls in would split the states).
The twelfth level's routes take the long way through the undersea
caves, checkpoint to checkpoint, to the boss room's door high above the
water. The bot is led with waypoints up the climbs its coarse graph
counts as easy: the springboard rocks, the columns of toggle and
crumbling pegs and the elevators. Two climbs are written into the routes
by hand, each a fresh jump pressed on the one tick Claw stands in mid-air
after a falling stalactite hits him: onto the block beyond the shaft
past the first checkpoint, where a stalactite hangs over the block's
edge in the way of any jump, and from a crumbling peg that gives way
under him in the shaft of crumbling pegs before the checkpoint at
x 26237. No blow of Claw's
hurts Aquatis, only the stalactites over his head do, but Aquatis can be
lifted: the routes walk off the toggle peg beside him, drop past him and
steer into his body, whose touch stops Claw in mid-air at the height of
Aquatis's feet; Lift pressed on the next tick picks Aquatis up, and he
drops at once into the water and dies. A help screen of 7.75 seconds
twelve ticks later (an `@N menu help 7750` line) makes the gem rising
from him the pickup on Claw, which ends the level.
`level12-introskip.csv` jumps in instead of sitting through the
introduction, so Aquatis stays in his start-up pose and never punches;
`level11-original.level12.csv` plays `level12-original.csv`'s rows from
the progress `level11-original.csv` ends with, pressing the weapon key
with its first step so that Claw carries the pistol the route fires,
not the dynamite that game ends with. `tools/bot/hints/level12-original.txt`
lists the waypoints and the hand-written stretches' held buttons.
The bot counts where each stalactite near Claw is (hanging, falling and
how far down, or gone), so a wait for one he set off to fall out of the
way is progress, and it counts the time through the landing after a
long fall, which holds Claw on the spot for a while; held by a tentacle
beside Aquatis, it mashes its way out as in a bear's hug.
The thirteenth level's routes take the long way over the island,
checkpoint to checkpoint, passing by both warps down to the caves under
it, up the shaft of toggle pegs below the boss room's door. The bot is
led with waypoints over the springboards, the water rocks and the columns of
toggle pegs its coarse graph counts as easy. The crossings that need a
timing it does not see are written into the routes by hand, each found by
playing the game with every wait in a range and keeping one in the middle
of those that work: the ropes over the water, met and left at the moments
that reach the next rope, a ledge or an elevator sliding beyond; the row
of toggle pegs over the water whose pegs come out one at a time, not in
order across; the two columns of pegs that are out only part of their
cycle, climbed within one of their out phases; and the water rock at the
foot of the shaft below the door, whose throw lands Claw on a peg of one
of the shaft's three ways up as that way comes out (the pegs keep the
level's clock, so `level13-original.csv` waits a few seconds on a quiet
ledge on its way there to meet the way its climb fits). RedTail takes 5 of
his 100 health from each blow, and blows that find him free send him
leaping to the other level of his arena instead (or, when he already
stands on the other level from Claw, only home): every third once his
introduction has played; when the jump-in skipped it, in a game played
from the level's start the first and every third after it, and in a run
started by a teleport the second and every third after it;
`level13-original.csv` and `level13.csv` sit through his introduction
and beat him on the sand, `level13-introskip.csv` jumps in with a slash that
reaches him on his ledge and only sends him home, and beats him on the
sand once he has leapt down to it, and each
takes the gem that rises from him once he has said his last line.
`level12-original.level13.csv` plays `level13-original.csv`'s rows from
the progress `level12-original.csv` ends with.
`tools/bot/hints/level13-original.txt` and `tools/bot/hints/level13.txt`
list the waypoints and the hand-written stretches' held buttons.
The fourteenth level's route takes the long way through the temple,
checkpoint to checkpoint, to the warp down to Omar's chamber. Near the
start the tiger guard by the first lava pit front-flips at Claw; a flip
that carries a tiger past Claw ends with its slash as soon as he is behind
it, so the route jumps back on to the ledge above and lets the tiger
land below him. An enemy that is hit shows a hit burst only when the
last frame drew it, so a lizard hidden in that frame and showing itself
in the tick a slash kills it gets none (nor the roll for the burst's
size), and a burst already running ends once its enemy was not drawn.
With this, `level14-original.csv` and `level13-original.level14.csv` were
searched again from tick 7116, before such a lizard by the path elevator,
and `level13.csv` from tick 6847, before another one on the island.
The bot is
led with waypoints up the climbs its coarse graph counts as easy. The
crossings that need a timing it does not see are written into the route
by hand, each found by playing the game with every wait in a range and
keeping one in the middle of those that work (or the only ones that do):
the elevators met and left
at the right moment, the rope beside them, a long wait on safe ground
before the elevator over the lava past the first save point, so that it
comes by as Claw drops on to it, jumps in place on a toggle
peg that wait for the pegs' cycle (five of them on the lowest peg of the
climb, which bring the path elevator above it round in time), the climb
of toggle pegs and that
elevator, a wait of ten seconds partway up the column past the second
save point, until the elevators beside it come round to where its next
jumps meet them, and three dynamite boosts, each a stick set off at
Claw's side in mid-air by the time a help screen of three seconds is up
(the `@N menu help 3000` lines), on to ledges no jump reaches. The warp
drops Claw, falling, on the ladder top above the chamber, so his
introduction cannot be skipped; `level14-original.csv` ends a life there
instead, with the in-game menu's End Life (its `@N command end_life`
line) while the introduction freezes Claw: he comes back on the ladder
top at full health with one life fewer, free long before the introduction
would have let him go, and Omar is reset with him. The menu's frames
keep the enemies' rhythm going, so Omar's random choices after the End
Life come on the original's beat; with this, both routes kept their rows
through the respawn and Omar's fight after it was searched again. That
is the route's one death; every other route still wins with none. In the first phase of
the fight Omar stands on one of two ledges inside a turning ring of fire
or ice that only a sword shot of the other element hurts, so the route
fetches the ice sword to the right of the chamber and the fire sword to
its left in turn (they can be taken again) and shoots him through the
ring's gap from the floor between his ledges, four times; then it beats
him in the hall above with four blows, each of which sends him to the
hall's other end, and takes the gem that rises from him. The bot counts
the shots and blows left, then the way to the sword the next shot needs,
to the floor between the ledges once Claw holds it, or to Omar's end of
the hall; with that sword in hand near the floor, where the ring's gap is
in its turn is part of its coarse state, and once Omar is struck down for
good, the time. `level13-original.level14.csv` plays
`level14-original.csv`'s rows from the progress `level13-original.csv`
ends with, through the warp and the End Life, which that game reaches
the same way; from there its own fight was searched the same way.
`tools/bot/hints/level14-original.txt` lists the
waypoints and the hand-written stretches' held buttons.
Once a boss is beaten and on his way to his gem, the bot only waits, and
how far Claw is from him counts for nothing; RedTail struck down in the
middle of a leap lands with no death to come and fights on with no health
left until another blow brings it, so the bot counts him then as a blow
away from his death.
The routes
are written by the search bot, `claw-bot`,
which tries short sequences of moves on copies of the headless game and
keeps the ones that bring Claw closer to the level's end. Its budget counts
the positions it tries, not time, so the same arguments always give the
same route. A long level can be searched a stretch at a time: `--goal X,Y`
ends a route where Claw reaches that point (with `--goal-on-foot`, only once
he stands there), and `--from-route FILE --from-tick T` carries on from it.

```sh
# watch the bot's route
make run-game ARGS='--install PATH --level 1 --replay tests/acceptance/routes/level1.csv'
# search a route (the report goes next to it as route.json)
cargo run --release -p claw-bot -- --install PATH --level 1 --out route.csv
# benchmark along a route from the level's start to its end, offscreen
make bench ARGS='--install PATH --level 1 --headless --res 3840x2160 --bench-route tests/acceptance/routes/level1.csv'
```

The route benchmark measures every frame as the combat benchmark does,
fails when a sprite set had to be loaded while drawing, and prints a digest
of the route's trace: the same on every run, at any display rate.

A reference trace is a run of the original game playing the same route,
written in the same trace format, one row per game step. Reference traces
are not part of this repository; point `CLAW_REFERENCE` at the folder that
holds them, one folder per route (`level<N>/<name>/` with the route as
`route.csv`, the traces as `trace-<clock>-r<k>.csv` and `meta.toml` saying
how they were played). A route can be played on two clocks:

- `c18`, one 18 ms game step per frame, compared tick for tick with the
  original speeds and the same random state: every column must be equal;
- `c60`, 60 frames per second, compared with the default tuning within the
  small differences in position and timing that `feel.toml` allows.

A trace's `held` column is the keys held as the game's controls saw them,
as the original's traces record them: while a Siren's scream has taken
Claw's controls it is 0, though the route still holds its keys (the route
and the replay keep them, so a key held to the lockout's end acts on the
first free tick).

```sh
# replay the route with the store's tuning and random state, compare with
# every reference trace, print the first divergence and a per-column table
CLAW_INSTALL=PATH CLAW_REFERENCE=DIR make compare REF=level1/opening
make compare REF=level1/bot CLOCK=c60      # one clock
make compare REF=level1/bot STRICT=1       # fail on any divergence
# one trace with the replay tool
make replay ARGS='--install PATH --script route.csv --tuning original --seed N --compare trace.csv --tolerance exact'
```

`make compare` fails on a divergence that
`tests/acceptance/tolerances/references.toml` does not list for the route,
or that comes earlier than listed; that file records, with a reason, where
each compared route is known to leave its tolerance. A route that starts
somewhere in the middle of a level names its start point in `meta.toml`
(`teleport = "X,Y"`): Claw is placed there and left to settle before the
route starts, and once he stands the comparison also sets the random
stream to the store's `rng_at_t0`, as `claw-replay --start X,Y --seed N`
does (the game's own `--start` with `--replay` places Claw but does not
reset the random stream). `make test-install MODULE=acceptance`
with `CLAW_REFERENCE` set runs every listed route twice: tick by tick
(`install_reference`), and through the game's own frame loop at 60 Hz at
the gate's preset, exact at `c18` and feel at `c60` (`install_differential`).
`tests/acceptance/tolerances/README.md` explains the clocks and the
tolerance values.

The seventh level's hermit crabs each start with a side their first
regroup faces by and a second range box that the original game does not
fix: they differ from crab to crab and from run to run (a launch after an
earlier level can give other values than the launch before it), and a box
that covers Claw makes a crab decide as if he were far whenever he is
outside its nearer boxes. A crab a nest hatched can also start
stationary, which the original game does not fix either: it then has no
ledge guard (it may stand with its leading edge over a drop where another
crab steps back) and strikes with the bomb's throw sound. A route gives a
run's values in header lines,
`# crab RECORD = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]` for a crab
placed in the level (its record: its place in the level's list of
objects, from 0) and
`# nest_crab NEST_RECORD HATCH = SIDE, HALF_WIDTH, HALF_HEIGHT[, STATIONARY]`
for the crab a nest hatched in that place (from 1), where STATIONARY is 1
for a stationary crab and 0, the default when it is left out, for one
that is not (`# nest_crab 1306 3 = 0, 0, 0, 1`); a `*` names every crab
of that kind, and the entry that names a crab most exactly counts; a
placed crab's entry may say STATIONARY too. `claw-replay`
and `make compare` give each crab its values as it is built, before its
first tick; `claw-replay --crabs FILE` takes them from a file of the same
lines instead, and `--crab-variant N` takes the route's further set N.
Without values every crab starts with an empty box, facing left, and not
stationary. A reference store may hold each repeat's crab values in a
file of such lines beside its trace, `crabs-<clock>-r<k>.replay.txt` or
`crabs-<clock>-r<k>.txt` for the `k`-th trace its `meta.toml` lists:
`make compare` and the reference tests then compare that trace with the
route replayed under those values (the replay form first), so repeats
whose crabs started otherwise can all be exact. A later level of such a
game starts each repeat with the progress that repeat carried in, which
can differ from launch to launch: the store then holds, beside the
repeat's trace, `route-<clock>-r<k>.csv`, a copy of the route with that
repeat's own `# start_...` lines, and `make compare` and the reference
tests replay that copy for that trace (`route.csv` for a repeat without
one).
`rusty-claw --replay` gives the crabs the values the route's own header
lines name, as `claw-replay` does, both when the route is watched in the
window and with `--trace`. `claw-bot --crabs FILE` searches under such
values, and `--crab-variants FILE,FILE` makes the route win under further
sets too, which its header names in `# crab_variant N: ...` lines and the
route tests play it under. `level7-original.csv` and
`level6-original.level7.csv` carry the values of the original's runs and
win under the further sets a launch can give.

```sh
# replay a route with its crabs' values from a file
make replay ARGS='--install PATH --script route.csv --crabs crabs.txt'
```

## Development

```sh
make check                        # format check, lints and unit tests
make check-game                   # unit tests of one module
CLAW_INSTALL=/path/to/your/install make test-install MODULE=acceptance
```

Unit tests build their test data in code and never need an install. The
install tests read your install at run time and skip themselves without it:

| Variable | Used by |
| --- | --- |
| `CLAW_INSTALL` | every install test: `make test-install MODULE=<module>` refuses to run without it |
| `CLAW_GOLDEN_DIR` | optional: a folder of reference visible-tile grids (`level<N>_<W>x<H>.json`) the render and acceptance tests compare against |
| `CLAW_TOLERANCES` | optional: a TOML table of movement targets to check instead of the built-in one |
| `CLAW_REFERENCE` | optional: the folder of reference traces that `make compare` and the reference tests read (those tests skip without it) |
| `CLAW_GPU_TESTS=1` | optional: run the renderer's GPU tests on synthetic data without an install |
| `CLAW_AUDIO_SMOKE=1` | optional: open the real audio device in the audio tests |

The acceptance suite (`tests/acceptance`) plays the first level from start
to finish on a route built from the level's own records (every enemy
killed, every checkpoint, a death and the respawn, crates, a powder keg,
treasure and the tally, the elevator and the toggling pegs, the map piece)
and checks that the whole game is identical at 60, 144 and 240 Hz with
jitter. It also covers each weapon against each enemy, a saved game round
trip and the tally through the menus, a minute of random input on every
retail level, every level loading in under 3 s, the visible tile grids,
Claw's movement against the movement targets, frame cost at 3840x2160,
5120x1440 and 640x480, a 640x480 window at 60 frames per second or more,
and community custom levels. Some of these tests use the GPU or open a
window for a few seconds; on a locked or headless desktop run them under a
virtual display (`xvfb-run`). `tests/acceptance/PLAYTEST.md` is the list of
things to try by hand before a release.

Other tools:

```sh
make run-viewer ARGS='--install PATH --level 1'          # free-camera level viewer
make replay ARGS='--install PATH --level 1 --script s.csv --trace t.csv'
cargo run --release -p claw-info -- --install PATH --level 1
```

The viewer follows the same rules as the game: vsync on by default
(`--vsync off` to turn it off), a cap of `--fps-cap N` frames per second
(300 by default) with vsync off, and `--uncapped` only for throughput runs.
Its offscreen benchmark (`--headless-bench SECONDS`) draws at `--bench-hz`
unless `--uncapped` is given.

## Known gaps

- Levels 3 to 14: their own enemies, bosses, traps, ropes, conveyor belts
  and other objects do not move or act yet.
- Movies need a build with FFmpeg (see [Movies](#movies)); without one,
  or without the ending movie, a short black hold stands in for it after
  the last level, and the credits show over the parchment. The movie
  interlacing switch does nothing.
- No multiplayer (curses only act in multiplayer games, so they vanish in
  single player as in the original).
- The credits text uses the game's bitmap font, which has capitals, digits
  and a few signs only: letters show as capitals, and other signs
  (hyphens, apostrophes, slashes, quotes, colons) show as spaces.
- A few details still follow a reasonable guess rather than a measurement
  of the original: carrying an object walks at 30 px/s, rats that reach a
  wall turn back at walking speed, enemies treat goo like spikes, and a
  magic hit on an enemy shows no lingering flame or frost. The play-test list
  (`tests/acceptance/PLAYTEST.md`) asks about each one a player can see on
  the first two levels and names the rest as untested there.
- The detail setting and the play area size are shown but have no effect
  yet; the game always draws in high detail.
- One profile, without profile screens.

## Layout

| Directory | Package | What it is |
| --- | --- | --- |
| `engine/assets` | `claw-assets` | asset store, format readers, level model |
| `engine/frame` | `claw-frame` | fixed-step frame driver and the types shared by renderer and simulation |
| `engine/render` | `claw-render` | palette renderer, screen sprites and transitions |
| `engine/sim` | `claw-sim` | headless simulation and the level logics |
| `engine/audio` | `claw-audio` | mixer, output device and music synthesis |
| `engine/ui` | `claw-ui` | status bar, menus and the title, tally, game-over and credits screens |
| `app/info` | `claw-info` | level summary tool |
| `app/viewer` | `claw-viewer` | level viewer |
| `app/game` | `claw-game` | the game (`rusty-claw` binary) |
| `tools/replay` | `claw-replay` | input replay, traces and their comparison |
| `tools/bot` | `claw-bot` | search bot that writes winning routes |
| `tests/acceptance` | `claw-acceptance` | end-to-end tests |

## Licence

GPL-3.0-or-later; see [LICENSE](LICENSE). The licence covers this source code
only. Captain Claw and its assets belong to their respective owners and are
not part of this project. The SoundFont that `make soundfont` fetches is
covered by its own licence.
