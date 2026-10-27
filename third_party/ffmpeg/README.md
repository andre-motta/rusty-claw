# FFmpeg for movies

The game decodes its movies with [FFmpeg](https://ffmpeg.org). Rather than use
whatever FFmpeg a system has (many lack the H.264 and HEVC decoders, and
headers are a separate package), rusty-claw builds one pinned FFmpeg release
from source, with only the parts the movies need, and links it statically
into the game. See [NOTICE](NOTICE) for the version, the checksum, the
configure line and the licence terms.

| File | What it is |
|---|---|
| `build.sh` | Downloads, checks and builds FFmpeg (and the nasm assembler it needs) |
| `NOTICE` | FFmpeg's version, source, configure line and LGPL terms |
| `rusty_ffmpeg/` | FFmpeg's interface in Rust (rusty_ffmpeg 0.17.0, MIT) with a build script that finds or builds FFmpeg and links it |
| `windows/` | The same FFmpeg built with Microsoft's compiler for Windows 11, and a check of the finished `.exe` |
| `deck/` | A build of the whole game for the Steam Deck in the Steam Runtime SDK container, and a glibc check |

## Linux

You need what the game itself needs to build (a C compiler, `make`, `curl`
or `wget`) plus `tar` with xz support and `sha256sum`, which every desktop
distribution has. No root access and no FFmpeg package are needed.

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

1. downloads the FFmpeg source release and nasm into
   `~/.cache/rusty-claw/ffmpeg/downloads` and checks their SHA-256;
2. builds nasm into `~/.cache/rusty-claw/ffmpeg/nasm-<version>` (x86 only);
3. configures FFmpeg with the line in `build.sh` (LGPL only, static, the
   listed decoders, demuxers and parsers and nothing else; [NOTICE](NOTICE)
   repeats it), builds it with up to 8 jobs at a lower priority (`nice -n
   10`) and installs it into the prefix,
   `~/.cache/rusty-claw/ffmpeg/<version>-<hash>`, where the hash covers the
   pins and the configure line;
4. records the build in `<prefix>/share/rusty-claw/ffmpeg-build.txt`,
   writes the prefix to `~/.cache/rusty-claw/ffmpeg/last-build.txt` (which
   a cargo build that found no FFmpeg watches, so its next build links
   this one) and removes the build tree.

The demuxers are `avi`, `smacker`, `mov`, `matroska`, `mpegps` and
`mpegvideo`: `mpegps` reads VOB files but leaves the codec of their picture
stream to the raw demuxers' probes, so a VOB opens only with the raw
`mpegvideo` demuxer built in too.

Running it again with the same pins does nothing. Changing a pin or the
configure line builds into a new prefix; old prefixes can be deleted.
`$XDG_CACHE_HOME` replaces `~/.cache` when set.

Settings, all optional:

- `RUSTY_CLAW_FFMPEG` chooses what a build does about FFmpeg: `auto` (the
  default, also when unset or empty) links it, building it first where it
  can, and builds the game without movies where it cannot; `off` builds
  without FFmpeg and never builds it; `require` makes a build without
  FFmpeg an error, with the reason (the Steam Deck build uses it);
- `FFMPEG_PREFIX=/some/dir` builds into, and links from, that directory
  instead (for `make` and for plain `cargo` alike). It must be an absolute
  path; `build.sh` stops on a relative one and the `rusty_ffmpeg` build
  script treats it as no FFmpeg, and an empty `FFMPEG_PREFIX` is the same
  as none. The Makefile passes it on to cargo only when you set it, so
  switching between `make` and plain `cargo` does not rebuild FFmpeg's
  binding;
- `FFMPEG_JOBS=N` sets the parallel compile jobs.

## How the game links it

The `rusty_ffmpeg` build script decides, on every platform:

1. It looks for a finished build in the prefix (`FFMPEG_PREFIX`, else the
   one `build.sh --prefix` prints; on Windows, see below) and links FFmpeg's
   static libraries from it.
2. When there is none and the machine can build one (Linux, the Steam Deck
   and other Unix systems, building for themselves, with `sh`, a C
   compiler, `make`, `curl` or `wget`, `tar` and `xz`), it runs `build.sh`
   into the prefix, prints a cargo warning that it did, and links it.
3. Otherwise it links nothing and prints one warning, for example

   ```
   warning: rusty_ffmpeg@0.17.0+ffmpeg.8.1: this build plays no movies: building FFmpeg needs make, which this system lacks (install what is missing, run `make ffmpeg`, then build again); see third_party/ffmpeg/README.md
   ```

   The game still builds and runs; it plays no movies and says so once at
   start-up. Once a build appears in the prefix (`make ffmpeg`), the next
   `cargo build` notices it and links it.

The script tells the `claw-movie` crate whether it linked FFmpeg, and
`claw-movie` builds either its player or a stand-in that plays nothing, so
the rest of the game has one code path. The Makefile's `run-game`, `check`,
`check-movie`, `check-game` and `test-install` print which way they build
(`movies: on`, or `movies: off` with `RUSTY_CLAW_FFMPEG=off`).

The cargo features `movies` (on `claw-game`) and `ffmpeg` (on `claw-movie`)
that older instructions name are kept so those commands still work, and do
nothing: `cargo build --features movies` is the same build as without it.

The Rust declarations are committed (`rusty_ffmpeg/src/binding.rs`), so the
build needs neither libclang nor FFmpeg's headers. The game binary needs no
FFmpeg at run time: `ldd target/release/rusty-claw` lists no `libav*` or
`libsw*` library.

To check the build is the LGPL one:

```sh
grep configure "$(sh third_party/ffmpeg/build.sh --prefix)/share/rusty-claw/ffmpeg-build.txt"
cargo test -p claw-movie --test smoke
```

The smoke test checks that exactly the listed decoders, demuxers and parsers
are there, that FFmpeg reports the LGPL, and that its configure line has
`--disable-gpl` and `--disable-nonfree`.

## Updating FFmpeg

1. Pick the new release and the rusty_ffmpeg version made for it (its version
   ends in `+ffmpeg.X.Y`).
2. In `build.sh`, change `FFMPEG_VERSION` and `FFMPEG_SHA256` (verify the
   release's signature from ffmpeg.org before trusting the checksum).
3. Replace `rusty_ffmpeg/src` with the new crate's `src`, keep the one-line
   change in `src/lib.rs` (`include!("binding.rs")`), and update the version
   in `rusty_ffmpeg/Cargo.toml` and `Cargo.lock`.
4. Update `NOTICE`, run `make ffmpeg` and `make check`.

## Movies in H.264 and HEVC

Upscaled copies of the movies are expected as H.264 or HEVC (H.265), 8-bit
or 10-bit (HEVC Main10), in MP4, MKV or MOV. The player hands YUV pictures to
the renderer as 8-bit I420 (Y at full size, U and V at half width and half
height) with their colour matrix (BT.601 or BT.709) and range (limited or
full), and the GPU turns them into colour; 10-bit and other chroma layouts
are converted to 8-bit 4:2:0 by swscale first. Smacker pictures stay RGBA so
their palette colours are exact.

`engine/movie/tests/codecs.rs` makes H.264 (MP4 and MKV, with AAC sound) and
HEVC Main and Main10 (MKV, with AAC sound) clips of 2560x1920 at 29.97
pictures a second at test time, with the system's `ffmpeg` tool, plays them
through the player and checks their pictures. With `CLAW_CODECS_BENCH=1` it
also prints how fast they decode with every core and with four, and asks for
at least 1.5 times real time with every core:

```sh
CLAW_CODECS_BENCH=1 cargo test --release -p claw-movie --test codecs -- --nocapture
```

Run it while the machine is otherwise idle, since it measures speed. The
H.264 clips need the libopenh264 encoder and the HEVC clips hevc_nvenc or
hevc_vaapi; a clip whose encoder is missing is skipped with the reason
printed. The HEVC clips encode on the GPU, so they are made only when
`CLAW_GPU_TESTS=1` is set as well; run that while no other GPU work or
benchmark runs, so neither skews the other. Without `CLAW_CODECS_BENCH=1`,
as in `make check`, the test checks the pictures of the clips it can make
without the GPU, and `make test-install MODULE=movie` runs only the movie
module's install tests.

## Windows 11 (MSVC)

The Windows build uses the same pinned FFmpeg, with the same checksums and
the same list of decoders, demuxers and parsers: `windows/build.sh` reads them
from `build.sh`. Only the platform parts of the configure line differ
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
   pacman -S --needed make gcc diffutils tar xz curl
   ```

   (`gcc` builds only the nasm assembler that FFmpeg's fast paths need; FFmpeg
   itself is built with MSVC.)

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

   It downloads FFmpeg and nasm into `%LOCALAPPDATA%\rusty-claw\ffmpeg`,
   checks their SHA-256, builds nasm, then builds FFmpeg into
   `%LOCALAPPDATA%\rusty-claw\ffmpeg\<version>-<hash>-msvc`, and ends by
   printing that folder. A user folder with a space in its
   path cannot hold the build; set `FFMPEG_PREFIX=C:/rusty-claw/ffmpeg` (any
   folder without spaces) before running it. Running it again with the same
   pins does nothing but print the folder.
5. Type `exit` to return to the Developer prompt.

To build the game:

6. The `rusty_ffmpeg` build script finds a build in the default folder
   itself: the newest `%LOCALAPPDATA%\rusty-claw\ffmpeg\<version>-<hash>-msvc`
   whose record names the pinned version, checksum and component list. If
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
  (and any MSVC `-libpath:` there is dropped).
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
`x86_64-pc-windows-msvc`, but the FFmpeg build and the link have not been run
on Windows 11 yet.

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
building Linux games: everything the Deck has, nothing newer. FFmpeg is built
from the pinned source inside it and linked in statically, so the Deck needs
no FFmpeg; the game uses only the Deck's own libraries (glibc, ALSA, udev,
Vulkan and the display libraries it loads at run time).

On any x86-64 Linux PC with `podman` or `docker` (the Deck itself in Desktop
Mode works too, if `podman --version` answers there):

```sh
sh third_party/ffmpeg/deck/build.sh
```

The first run downloads the SDK image, Rust (the version
`rust-toolchain.toml` names), FFmpeg, nasm and the crates into
`~/.cache/rusty-claw/deck`, and takes a while; later runs reuse them. The
script checks that the binary loads no FFmpeg library and that its glibc
floor is at most 2.31 (`DECK_GLIBC_MAX` changes the limit), then copies it to
`target/deck/rusty-claw`.

On the Deck:

1. Copy `target/deck/rusty-claw` to `~/Games/rusty-claw/` on the Deck (any
   way you like, for example `scp`), and your game install next to it (for
   example `~/Games/Claw`).
2. In Desktop Mode, in Konsole, check the glibc and the libraries, then play
   the logo:

   ```sh
   ldd --version | head -n 1                                   # at least the floor
   ldd ~/Games/rusty-claw/rusty-claw | grep -c -E 'libav|libsw' # 0
   ~/Games/rusty-claw/rusty-claw --install ~/Games/Claw --movie -1
   ```

   The View or Menu button (the pad's Back and Start) skips it.
3. For Game Mode, add `rusty-claw` with "Add a Non-Steam Game to My Library"
   and set its launch options to `--install /home/deck/Games/Claw`.

Status: the recipe is reviewed (`bash -n`, shellcheck); the container build
and a run on a Deck are still to be done.
