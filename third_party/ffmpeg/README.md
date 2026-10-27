# FFmpeg for movies

The game decodes its movies with [FFmpeg](https://ffmpeg.org). Rather than use
whatever FFmpeg a system has (its decoders differ from system to system, and
headers are a separate package), rusty-claw builds one pinned FFmpeg release
from source, with only the parts the movies need, together with one pinned
release of [dav1d](https://code.videolan.org/videolan/dav1d), the AV1 decoder
FFmpeg uses for upscaled movies, and links both statically into the game.
See [NOTICE](NOTICE) for the versions, the checksums, the configure line and
the licence terms.

| File | What it is |
|---|---|
| `build.sh` | Downloads, checks and builds dav1d and FFmpeg (and the nasm assembler they need, and meson and ninja for dav1d where the system has none) |
| `av1-fallback-drill.sh` | A drill of `build.sh` going on without AV1 when FFmpeg's configure cannot use dav1d (see Updating FFmpeg) |
| `NOTICE` | FFmpeg's and dav1d's versions, sources, configure line and licence terms |
| `rusty_ffmpeg/` | FFmpeg's interface in Rust (rusty_ffmpeg 0.17.0, MIT) with a build script that finds or builds FFmpeg and links it (`windows_record.rs` is the part it shares with a test) |
| `windows/` | The same FFmpeg built with Microsoft's compiler for Windows 11, and a check of the finished `.exe` |
| `deck/` | A build of the whole game for the Steam Deck in the Steam Runtime SDK container, and a glibc check |

## Linux

You need what the game itself needs to build (a C compiler, `make`, `curl`
or `wget`) plus `tar` with xz and gzip support and `sha256sum`, which every
desktop distribution has. No root access and no FFmpeg package are needed.

AV1 (dav1d) is an extra on top of that. It needs `pkg-config`, and dav1d
builds with meson (0.54 or newer) and ninja (1.8.2 or newer, or samu). When
the system has them, `build.sh` uses them; when it lacks either, it
downloads the pinned release of meson (run as `python3 meson.py`, which
needs `python3` 3.7 or newer) or the pinned source of ninja (built with
`python3 configure.py --bootstrap`, which needs `python3` and a C++
compiler, `c++` or `$CXX`, with as many jobs as the build's own ninja
runs) into `~/.cache/rusty-claw/ffmpeg`, checks them against their SHA-256
like every other download and keeps them there for later builds. Nothing
is installed into the system, and nothing comes from PyPI.

FFmpeg is built without AV1, into a prefix of its own, when any of that is
missing, when `RUSTY_CLAW_FFMPEG_AV1=off`, when downloading or building
meson, ninja or dav1d fails, or when FFmpeg itself fails to configure,
build or install with dav1d (it is then built once more without it; when
that fails too, the build stops, since AV1 was not the cause). The
original movies play either way; only upscaled AV1 copies need AV1. The
build says why:
`build.sh` in its output, and cargo in a warning, for example

```
warning: rusty_ffmpeg@0.17.0+ffmpeg.8.1: this FFmpeg decodes no AV1, so upscaled AV1 movies will not play (the original movies do): pkg-config is missing (install it, then run make ffmpeg); see third_party/ffmpeg/README.md
```

A failed attempt is kept in `~/.cache/rusty-claw/ffmpeg/av1-failed.txt`
(with the build log, when there is one, in `av1-failed.log`) together with
what could change how it ends: everything the build with AV1 is made of
(FFmpeg's, nasm's, dav1d's, meson's and ninja's pins, the whole configure
line with AV1 and dav1d's meson options) and the compilers, meson, ninja,
python3 and pkg-config found. Later builds go on without AV1, without
trying again, until one of those changes (a build with AV1 for the same
pins and settings that is already there is still used); `make ffmpeg`
always tries again (`sh third_party/ffmpeg/build.sh --retry-av1`). Only
failures that would happen again are kept: getting or building meson,
ninja or dav1d, and FFmpeg's configure with dav1d. When FFmpeg's `make` or
`make install` fails with dav1d, which can be a failure of the moment (a
compiler stopped when memory ran out), that build goes on without AV1 and
cargo links it, but nothing is kept, and the next build that builds
FFmpeg tries AV1 again (a `make` failure that repeats costs that attempt
on each such build, until `make ffmpeg` gets through or
`RUSTY_CLAW_FFMPEG_AV1=off` is set). After installing what was missing, run `make
ffmpeg`, and the next cargo build links the build with AV1. With
`FFMPEG_PREFIX`, `make ffmpeg` replaces the build there without AV1 by the
one with AV1 once that is built, and leaves it as it is when the attempt
fails again. (Older checkouts installed meson
and ninja into a `tools` folder in `~/.cache/rusty-claw/ffmpeg`; nothing
uses it any more, and it can be deleted.)

Movies come with a plain build:

```sh
cargo build --release -p claw-game     # or: make run-game ARGS='--install /path/to/your/install'
```

The first time, the build finds no FFmpeg in your cache, runs `build.sh`
itself (about a minute on a desktop, once per machine) and says so in a
cargo warning; later builds, in this checkout or any other, link the same
FFmpeg without building it again, and `cargo clean` does not remove it.
`make ffmpeg` runs the same step on its own, with its progress shown, for
example before an offline build.

`build.sh` (run by cargo or by `make ffmpeg`):

1. downloads the FFmpeg and dav1d source releases, nasm and, where needed,
   meson and ninja into `~/.cache/rusty-claw/ffmpeg/downloads` and checks
   their SHA-256;
2. builds nasm into `~/.cache/rusty-claw/ffmpeg/nasm-<version>` (x86 only);
3. unpacks meson into `~/.cache/rusty-claw/ffmpeg/meson-<version>` and
   builds ninja into `~/.cache/rusty-claw/ffmpeg/ninja-<version>` when the
   system has none (above), and builds dav1d as a static library into the
   prefix, `~/.cache/rusty-claw/ffmpeg/<version>-<hash>`, where the hash
   covers the pins, whether dav1d is built and the configure line (when
   any of that fails, it goes on without AV1, in the prefix of that build);
4. configures FFmpeg with the line in `build.sh` (LGPL only, static, the
   listed decoders, demuxers and parsers and nothing else; [NOTICE](NOTICE)
   repeats it), builds it with up to 8 jobs at a lower priority (`nice -n
   10`) and installs it into the same prefix (when that fails with dav1d,
   it goes on without AV1 as in step 3);
5. records the build in `<prefix>/share/rusty-claw/ffmpeg-build.txt`,
   writes the prefix to `~/.cache/rusty-claw/ffmpeg/last-build.txt` when it
   names another one (cargo builds watch that file, so their next build
   links this one) and removes the build tree.

The decoders are those of the game's movies (MPEG-4 Part 2, MPEG-2, MP3,
MP2, AC-3, AAC, PCM, Smacker) plus AV1 (through dav1d) and Opus, the
upscaled movies' format. The demuxers are `avi`, `smacker`, `mov`,
`matroska` (which reads WebM too), `mpegps` and `mpegvideo`: `mpegps` reads
VOB files but leaves the codec of their picture stream to the raw demuxers'
probes, so a VOB opens only with the raw `mpegvideo` demuxer built in too.

Running it again with the same pins does nothing. Changing a pin, the
configure line, `RUSTY_CLAW_FFMPEG_H26X` or `RUSTY_CLAW_FFMPEG_AV1`, or
getting AV1 where it was left out before, builds into a new prefix; old
prefixes can be deleted. `$XDG_CACHE_HOME` replaces `~/.cache` when set.
`build.sh --prefix` prints the prefix and `build.sh --av1` whether that
build has AV1 (`yes`, or `no: ` and the reason); both only look at the
system and the cache, and write nothing.

Settings, all optional:

- `RUSTY_CLAW_FFMPEG` chooses what a build does about FFmpeg: `auto` (the
  default, also when unset or empty) links it, building it first where it
  can, and builds the game without movies where it cannot; `off` builds
  without FFmpeg and never builds it; `require` makes a build without
  FFmpeg an error, with the reason, and a build without AV1 too unless
  `RUSTY_CLAW_FFMPEG_AV1=off` is set as well (the Steam Deck build uses
  it, and with it `cargo test -p claw-movie` fails rather than skips
  without AV1);
- `RUSTY_CLAW_FFMPEG_AV1=off` builds FFmpeg without AV1 every time, in
  the prefix that build has, and never tries meson, ninja or dav1d (`auto`,
  the default, tries them). It is for machines where they cannot work;
  set it for every make and cargo command alike;
- `FFMPEG_PREFIX=/some/dir` builds into, and links from, that directory
  instead (for `make` and for plain `cargo` alike). It must be an absolute
  path; `build.sh` stops on a relative one and the `rusty_ffmpeg` build
  script treats it as no FFmpeg, and an empty `FFMPEG_PREFIX` is the same
  as none. The Makefile passes it on to cargo only when you set it, so
  switching between `make` and plain `cargo` does not rebuild FFmpeg's
  binding;
- `FFMPEG_JOBS=N` sets the parallel compile jobs (by default the CPU
  count, at most 8; when cargo builds FFmpeg, FFmpeg's make takes its jobs
  from cargo's job server instead, and ninja runs half of them, for dav1d
  and when it builds the pinned ninja);
- `RUSTY_CLAW_FFMPEG_H26X=on` adds the H.264 and HEVC decoders (below).

## H.264 and HEVC (off by default)

The build leaves out the H.264 and HEVC (H.265) decoders: they are
patent-pooled formats, so nothing published (a release, a package, a
binary you hand to others) carries their decoders, and the original movies
never need them. People who build the game for themselves can add them
with `RUSTY_CLAW_FFMPEG_H26X=on`, for example to play their own copies in
those formats:

```sh
RUSTY_CLAW_FFMPEG_H26X=on make ffmpeg
RUSTY_CLAW_FFMPEG_H26X=on cargo build --release -p claw-game
```

The switch is read by `build.sh` and by the `rusty_ffmpeg` build script,
and is part of the prefix's hash, so the build with them lives in a prefix
of its own next to the default one, and switching back and forth relinks
without rebuilding FFmpeg. Set it the same way for every cargo command (a
build with the switch and one without each link their own FFmpeg).

## How the game links it

The `rusty_ffmpeg` build script decides, on every platform:

1. It looks for a finished build in the prefix (`FFMPEG_PREFIX`, else the
   one `build.sh --prefix` prints; on Windows, see below) and links FFmpeg's
   static libraries from it.
2. When there is none and the machine can build one (Linux, the Steam Deck
   and other Unix systems, building for themselves, with `sh`, a C
   compiler, `make`, `curl` or `wget`, `tar` and `xz`), it runs `build.sh`,
   prints a cargo warning that it did, and links the build it made: when
   AV1 could not be had, that is the build without AV1, in its own prefix
   (`build.sh --prefix` names it afterwards, or, after a failure of
   FFmpeg's make with dav1d that is not kept, `RUSTY_CLAW_FFMPEG_AV1=off
   build.sh --prefix` does).
   A linked build without AV1 gets one more warning, with the reason, each
   time the build script runs (an error with `RUSTY_CLAW_FFMPEG=require`).
   The H.264 and HEVC decoders, and AV1, are read from the linked build's
   record; a build in `FFMPEG_PREFIX` that disagrees with
   `RUSTY_CLAW_FFMPEG_H26X` gets a warning (an error under `require`), and
   the player follows the build.
3. Otherwise it links nothing and prints one warning, for example

   ```
   warning: rusty_ffmpeg@0.17.0+ffmpeg.8.1: this build plays no movies: building FFmpeg needs make, which this system lacks (install what is missing, run `make ffmpeg`, then build again); see third_party/ffmpeg/README.md
   ```

   The game still builds and runs; it plays no movies and says so once at
   start-up. Once a build appears in the prefix (`make ffmpeg`), the next
   `cargo build` notices it and links it.

Every cargo command that builds the `rusty_ffmpeg` build script does this,
not only `cargo build`: the first `cargo check`, `cargo clippy`, `cargo doc`
or `cargo test`, and the first time an IDE's rust-analyzer checks the
workspace, also build FFmpeg (once per machine) when there is none. To keep
an IDE from doing that, set `RUSTY_CLAW_FFMPEG=off` in its environment (for
rust-analyzer, `"rust-analyzer.cargo.extraEnv": {"RUSTY_CLAW_FFMPEG":
"off"}` in VS Code's settings); it then checks the build without movies.

The script tells the `claw-movie` crate whether it linked FFmpeg, and
`claw-movie` builds either its player or a stand-in that plays nothing, so
the rest of the game has one code path. The Makefile's `run-game`, `check`,
`check-movie`, `check-game` and `test-install` print which way they build
(`movies: on`, or `movies: off` with `RUSTY_CLAW_FFMPEG=off`).

The cargo features `movies` (on `claw-game`) and `ffmpeg` (on `claw-movie`)
that older instructions name are kept so those commands still work, and do
nothing: `cargo build --features movies` is the same build as without it.

The Rust declarations are committed (`rusty_ffmpeg/src/binding.rs`), so the
build needs neither libclang nor FFmpeg's headers. dav1d's static library
is linked from the prefix like FFmpeg's. The game binary needs no FFmpeg
and no dav1d at run time: `ldd target/release/rusty-claw` lists no
`libav*`, `libsw*` or `libdav1d` library.

To check the build is the LGPL one:

```sh
grep configure "$(sh third_party/ffmpeg/build.sh --prefix)/share/rusty-claw/ffmpeg-build.txt"
cargo test -p claw-movie --test smoke
```

The smoke test checks that exactly the listed decoders, demuxers and parsers
are there, that FFmpeg reports the LGPL, and that its configure line has
`--disable-gpl` and `--disable-nonfree`. The lists follow the build: the
AV1 decoder (`libdav1d`) and parser whenever dav1d was built, and the H.264
and HEVC decoders and parsers when, and only when, the test is built with
`RUSTY_CLAW_FFMPEG_H26X=on` (`RUSTY_CLAW_FFMPEG_H26X=on cargo test -p
claw-movie --test smoke` checks that build).

## Updating FFmpeg

1. Pick the new release and the rusty_ffmpeg version made for it (its version
   ends in `+ffmpeg.X.Y`).
2. In `build.sh`, change `FFMPEG_VERSION` and `FFMPEG_SHA256` (verify the
   release's signature from ffmpeg.org before trusting the checksum). dav1d
   is updated the same way, with `DAV1D_VERSION` and `DAV1D_SHA256` (its
   releases are at downloads.videolan.org with a signature and checksums).
3. Replace `rusty_ffmpeg/src` with the new crate's `src`, keep the one-line
   change in `src/lib.rs` (`include!("binding.rs")`), and update the version
   in `rusty_ffmpeg/Cargo.toml` and `Cargo.lock`.
4. Update `NOTICE`, run `make ffmpeg` and `make check`.

ninja publishes no source tarball, so `NINJA_URL` is the archive GitHub
makes of the release tag, and GitHub does not promise those bytes stay the
same. If a build says `ninja-<version>.tar.gz has SHA-256 ..., expected
...`, the pin needs refreshing: check the new archive against the release
tag's source and update `NINJA_SHA256` (or move to a newer ninja). Until
then, machines without a system ninja build FFmpeg without AV1, and the
original movies still play.

`av1-fallback-drill.sh` checks the fallback from FFmpeg's side: in a
scratch cache, it hides dav1d from FFmpeg's configure (a `pkg-config` that
knows no dav1d first on `PATH`), runs `build.sh`, and checks that the
result is FFmpeg without AV1, with the reason recorded, and that a second
run reuses it:

```sh
sh third_party/ffmpeg/av1-fallback-drill.sh ~/.cache/rusty-claw-drill
FFMPEG_PREFIX=<the prefix it prints> cargo test -p claw-movie --test smoke
```

## Upscaled movies: AV1 and Opus

Upscaled copies of the movies are expected as AV1 video, 8-bit or 10-bit,
with Opus sound, in MKV or WebM: royalty-free formats with open decoders,
which every build with AV1 plays (every build that could make dav1d; see
Linux above). (MP4 and MOV hold AV1 too.) The player hands YUV
pictures to the renderer as 8-bit I420 (Y at full size, U and V at half
width and half height) with their colour matrix (BT.601 or BT.709), range
(limited or full) and colour siting (AV1 pictures carry theirs in the
stream; left, as in MPEG video, when the stream leaves it open), and the
GPU turns them into colour; 10-bit pictures and other chroma layouts are
converted to 8-bit 4:2:0 by swscale first. Smacker pictures stay RGBA so
their palette colours are exact.

`engine/movie/tests/codecs.rs` makes AV1 clips, 8-bit in WebM and 10-bit
in MKV, both with Opus sound, of 2560x1920 at 29.97 pictures a second at
test time with the system's `ffmpeg` tool (libsvtav1, else libaom-av1;
libopus, else FFmpeg's own Opus encoder), plays them through the player and
checks their pictures (in a build without AV1 it skips them, or fails with
`RUSTY_CLAW_FFMPEG=require`). In a build with `RUSTY_CLAW_FFMPEG_H26X=on` it also
makes H.264 (MP4 and MKV, with AAC sound, libopenh264) and HEVC Main and
Main10 clips (MKV, with AAC sound, hevc_nvenc or hevc_vaapi). With
`CLAW_CODECS_BENCH=1` it also prints how fast they decode with every core
and with four, and asks for at least 1.5 times real time with every core:

```sh
CLAW_CODECS_BENCH=1 cargo test --release -p claw-movie --test codecs -- --nocapture
```

Run it while the machine is otherwise idle, since it measures speed. A clip
whose encoder is missing is skipped with the reason printed. Clips that
encode on the GPU (av1_nvenc, the HEVC ones) are made only when
`CLAW_GPU_TESTS=1` is set as well; run that while no other GPU work or
benchmark runs, so neither skews the other. Without `CLAW_CODECS_BENCH=1`,
as in `make check`, the test checks the pictures of the clips it can make
without the GPU, and `make test-install MODULE=movie` runs only the movie
module's install tests.

Decoding and converting to I420, 2560x1920 at 29.97 pictures a second, on
a 32-thread desktop x86-64 CPU (2026-10-27, dav1d 1.5.4, FFmpeg 8.1.3):

| Clip | Every core | Cores 0-3 (`taskset -c 0-3`, as on a Steam Deck) |
|---|---|---|
| AV1 8-bit WebM + Opus | 315.5 fps (10.5x real time) | 196.5 fps (6.6x) |
| AV1 10-bit MKV + Opus | 262.8 fps (8.8x) | 145.7 fps (4.9x) |

## Windows 11 (MSVC)

The Windows build uses the same pinned FFmpeg and dav1d, with the same
checksums and the same list of decoders, demuxers and parsers:
`windows/build.sh` reads them from `build.sh`, and builds dav1d with MSVC
through MSYS2's meson and ninja (the pinned meson and ninja of `build.sh`
are for Linux systems without them). A dav1d build that fails stops it,
with the reason; `RUSTY_CLAW_FFMPEG_AV1=off` then builds FFmpeg without
AV1 (and without needing meson, ninja or pkg-config), into a folder of its
own. Cargo takes that build when it finds none with AV1, and warns that it
decodes no AV1; with `RUSTY_CLAW_FFMPEG_AV1=off` set for cargo too, it
takes it without the warning (and under `RUSTY_CLAW_FFMPEG=require`, it
takes it only then). Only the platform parts of the configure line differ
(`sh third_party/ffmpeg/windows/build.sh --configure` prints it):
Microsoft's compiler (`--toolchain=msvc`), Windows threads instead of POSIX
threads, and the DLL C runtime (`-MD`) that Rust uses. FFmpeg's own build
system needs a POSIX shell, which MSYS2 provides; the compiler is MSVC, so
the result links into the Rust build. The game then carries FFmpeg inside
`rusty-claw.exe`, with no FFmpeg DLL.

On Windows the game's build never builds FFmpeg itself: that needs MSYS2
and MSVC together, below. Without it, `cargo build` still builds the game,
without movies, and prints a warning that says so.

Once per machine:

1. Install the [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/downloads/)
   with the "Desktop development with C++" workload (the MSVC compiler and a
   Windows 11 SDK), [Rust](https://rustup.rs) (the default `x86_64-pc-windows-msvc`
   host), [Git](https://git-scm.com) and [MSYS2](https://www.msys2.org)
   (into the default `C:\msys64`).
2. Open the "MSYS2 MSYS" shell from the Start menu once and install the
   tools the FFmpeg build uses:

   ```sh
   pacman -S --needed make gcc diffutils tar xz curl meson ninja pkgconf
   ```

   (`gcc` builds only the nasm assembler that FFmpeg's and dav1d's fast
   paths need; FFmpeg and dav1d themselves are built with MSVC, dav1d with
   meson and ninja.)

To build FFmpeg (once per FFmpeg version; it takes several minutes):

3. Open "x64 Native Tools Command Prompt for VS 2022" from the Start menu
   (the x64 one, not x86) and start MSYS2 inside it, so that MSVC's tools stay
   on the path:

   ```bat
   C:\msys64\msys2_shell.cmd -defterm -here -no-start -use-full-path -msys
   ```

4. In that MSYS2 shell, from the checkout (`C:\src\rusty-claw` is
   `/c/src/rusty-claw` there):

   ```sh
   cd /c/src/rusty-claw
   sh third_party/ffmpeg/windows/build.sh
   ```

   With `RUSTY_CLAW_FFMPEG_H26X=on sh third_party/ffmpeg/windows/build.sh`
   it adds the H.264 and HEVC decoders (see above; then set the same
   variable for cargo in step 7). When the dav1d build fails, it stops and
   says so; `RUSTY_CLAW_FFMPEG_AV1=off sh third_party/ffmpeg/windows/build.sh`
   then builds FFmpeg without AV1 (then set `RUSTY_CLAW_FFMPEG_AV1=off` for
   cargo in step 7 as well). The script ends by printing the `set` lines
   cargo needs for the switches it was run with.
   It downloads FFmpeg, dav1d and nasm into `%LOCALAPPDATA%\rusty-claw\ffmpeg`,
   checks their SHA-256, builds nasm and dav1d, then builds FFmpeg into
   `%LOCALAPPDATA%\rusty-claw\ffmpeg\<version>-<hash>-msvc`, and ends by
   printing that folder. A user folder with a space in its
   path cannot hold the build; set `FFMPEG_PREFIX=C:/rusty-claw/ffmpeg` (any
   folder without spaces) before running it. Running it again with the same
   pins does nothing but print the folder.
5. Type `exit` to return to the Developer prompt.

To build the game:

6. The `rusty_ffmpeg` build script finds a build in the default folder
   itself: the newest `%LOCALAPPDATA%\rusty-claw\ffmpeg\<version>-<hash>-msvc`
   whose record is exactly the one `windows/build.sh` writes for the
   current pins and switches (`sh third_party/ffmpeg/windows/build.sh
   --stamp` prints it), so a build from older pins, or with other
   decoders, is never used, except that a build without AV1 stands in
   when no build has AV1 (not under `RUSTY_CLAW_FFMPEG=require`). The
   build script works that record out itself
   (`rusty_ffmpeg/windows_record.rs`), and
   `engine/movie/tests/windows_record.rs` checks, on Linux too, that it
   matches what `--stamp` prints with every switch (and runs the tests of
   the choice between builds in `windows_record.rs`). If
   you built into another folder (`FFMPEG_PREFIX` in step 4), tell cargo
   too, in the Developer prompt, with the folder the script printed:

   ```bat
   set FFMPEG_PREFIX=C:\rusty-claw\ffmpeg
   ```

   (`setx FFMPEG_PREFIX "..."` keeps it for every new prompt; in PowerShell,
   `$env:FFMPEG_PREFIX = '...'`.)
7. Build, in the Developer prompt, from the checkout:

   ```bat
   cargo build --release -p claw-game
   ```

   If you ran step 4 with switches, set the same ones first (step 4 printed
   them), for example:

   ```bat
   set RUSTY_CLAW_FFMPEG_AV1=off
   set RUSTY_CLAW_FFMPEG_H26X=on
   ```

   Cargo does not find a build made with `RUSTY_CLAW_FFMPEG_H26X=on`
   without it. A build made with `RUSTY_CLAW_FFMPEG_AV1=off` is found
   without the variable when no build has AV1, with a warning that it
   decodes no AV1; the variable takes it without one. When cargo finds no
   build for the current switches but one with others, its warning names
   the `set` line that selects it.
   With `set RUSTY_CLAW_FFMPEG=require` first, a build that finds no FFmpeg
   stops with the reason instead of building without movies.
   The result is `target\release\rusty-claw.exe`, one file.
8. Check it carries FFmpeg and needs no FFmpeg DLL, from the MSYS2 shell of
   step 3:

   ```sh
   sh third_party/ffmpeg/windows/check-exe.sh target/release/rusty-claw.exe
   ```

   (or `dumpbin /dependents target\release\rusty-claw.exe` in the Developer
   prompt: no `avcodec`, `avformat`, `avutil`, `swscale` or `swresample` DLL
   in the list).
9. Play the logo movie from your install, then the game:

   ```bat
   target\release\rusty-claw.exe --install "C:\Games\Claw" --movie -1
   target\release\rusty-claw.exe --install "C:\Games\Claw"
   ```

   Escape, or a pad's Back or Start button, skips a movie.

Notes for whoever maintains this:

- FFmpeg names its MSVC static libraries `avcodec.lib` and so on, which is
  what the `rusty_ffmpeg` build script links; the system libraries FFmpeg
  needs (`bcrypt.lib`, `user32.lib`, ...) are rewritten in the installed
  pkg-config files as `-lbcrypt` and so on, the form the build script reads
  (and any MSVC `-libpath:` there is dropped). Meson names dav1d's static
  library `libdav1d.a`; the script renames it `dav1d.lib`, the name
  FFmpeg's configure and the build script look for, and copies it into the
  prefix, where the build script links it statically like FFmpeg's.
- MSYS2 has a `link` of its own; FFmpeg's build calls MSVC's `link.exe` from
  next to `cl.exe`, so the order on the path does not matter.
- The Rust declarations in `rusty_ffmpeg/src/binding.rs` are shared by Linux
  and Windows. FFmpeg's structures use fixed-size fields, so their layout is
  the same on both; the one `unsigned long` field (`AVIOContext::checksum`,
  4 bytes on Windows) and the `va_list` logging functions are never used by
  the player. `cargo check --target x86_64-pc-windows-msvc -p claw-game`,
  with `FFMPEG_PREFIX` naming any folder that holds a
  `share/rusty-claw/ffmpeg-build.txt`, checks the Windows build compiles
  with the player without a Windows machine; without `FFMPEG_PREFIX` it
  checks the build without movies and prints the warning.

Status: these steps are reviewed and the game compiles for
`x86_64-pc-windows-msvc` (also with a build record from `--stamp`, which
the build script matches exactly), but the dav1d and FFmpeg builds and the
link have not been run on Windows 11 yet.

## Steam Deck

A Linux binary runs only where the system's glibc is at least as new as the
newest glibc symbol version the binary imports (its glibc floor). A build on
a current desktop distribution has a floor too new for older systems:

| Built on | Command | glibc floor |
|---|---|---|
| Fedora 44 (glibc 2.43), 2026-10-27 | `objdump -T target/release/rusty-claw \| grep -o 'GLIBC_[0-9.]*' \| sort -V \| tail -1` | GLIBC_2.43 |

(`sh third_party/ffmpeg/deck/glibc-floor.sh BINARY [MAX]` prints the same
and fails above MAX.) So the Deck build happens in the Steam Runtime 3
"sniper" SDK container (Debian 11, glibc 2.31), which Valve publishes for
building Linux games: everything the Deck has, nothing newer. FFmpeg and dav1d are
built from the pinned sources inside it and linked in statically, so the
Deck needs no FFmpeg; the game uses only the Deck's own libraries (glibc, ALSA, udev,
Vulkan and the display libraries it loads at run time).

On any x86-64 Linux PC with `podman` or `docker` (the Deck itself in Desktop
Mode works too, if `podman --version` answers there):

```sh
sh third_party/ffmpeg/deck/build.sh
```

The first run downloads the SDK image, Rust (the version
`rust-toolchain.toml` names), FFmpeg, dav1d, nasm, the crates and, if the
SDK lacks them, the pinned meson and ninja (checked and built like the
rest, never from pip) into `~/.cache/rusty-claw/deck`, and takes a while;
later runs reuse them. The script always tries AV1 (`--retry-av1`) and
stops when FFmpeg was built without it, checks that the binary loads no
FFmpeg or dav1d library and that its glibc floor is at most 2.31
(`DECK_GLIBC_MAX` changes the limit), then copies it to
`target/deck/rusty-claw`.

On the Deck:

1. Copy `target/deck/rusty-claw` to `~/Games/rusty-claw/` on the Deck (any
   way you like, for example `scp`), and your game install next to it (for
   example `~/Games/Claw`).
2. In Desktop Mode, in Konsole, check the glibc and the libraries, then play
   the logo:

   ```sh
   ldd --version | head -n 1                                   # at least the floor
   ldd ~/Games/rusty-claw/rusty-claw | grep -c -E 'libav|libsw|libdav1d' # 0
   ~/Games/rusty-claw/rusty-claw --install ~/Games/Claw --movie -1
   ```

   The View or Menu button (the pad's Back and Start) skips it.
3. For Game Mode, add `rusty-claw` with "Add a Non-Steam Game to My Library"
   and set its launch options to `--install /home/deck/Games/Claw`.

Status: the recipe is reviewed (`bash -n`, shellcheck); the container build
and a run on a Deck are still to be done.
