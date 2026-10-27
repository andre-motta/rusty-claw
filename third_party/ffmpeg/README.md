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
| `rusty_ffmpeg/` | FFmpeg's interface in Rust (rusty_ffmpeg 0.17.0, MIT) with a build script that links the build |

## Linux

You need what the game itself needs to build (a C compiler, `make`, `curl`)
plus `tar` with xz support and `sha256sum`, which every desktop distribution
has. No root access and no FFmpeg package are needed.

```sh
make ffmpeg            # once per machine: about a minute on a desktop
make run-game ARGS='--install /path/to/your/install'
```

`make ffmpeg` runs `build.sh`, which:

1. downloads the FFmpeg source release and nasm into
   `~/.cache/rusty-claw/ffmpeg/downloads` and checks their SHA-256;
2. builds nasm into `~/.cache/rusty-claw/ffmpeg/nasm-<version>` (x86 only);
3. configures FFmpeg with the line in `build.sh` (LGPL only, static, the
   listed decoders, demuxers and parsers and nothing else), builds it with up
   to 8 jobs and installs it into the prefix,
   `~/.cache/rusty-claw/ffmpeg/<version>-<hash>`, where the hash covers the
   pins and the configure line;
4. records the build in `<prefix>/share/rusty-claw/ffmpeg-build.txt` and
   removes the build tree.

Running it again with the same pins does nothing. Changing a pin or the
configure line builds into a new prefix; old prefixes can be deleted.
`$XDG_CACHE_HOME` replaces `~/.cache` when set.

Settings, all optional:

- `FFMPEG_PREFIX=/some/dir` builds into, and links from, that directory
  instead (for `make` and for plain `cargo` alike);
- `FFMPEG_JOBS=N` sets the parallel compile jobs.

## How the game links it

Movies are behind two cargo features, both off by default: `ffmpeg` on the
`claw-movie` crate and `movies` on the game (`claw-game`), which turns on the
first. A build without them has no FFmpeg in it and plays no movies.

The Makefile's `run-game`, `check`, `check-movie`, `check-game` and
`test-install` turn the features on when the prefix holds a finished build
and print which way they built (`movies: on` or `movies: off`). With cargo
directly:

```sh
cargo build --release -p claw-game --features movies
```

The `rusty_ffmpeg` build script finds the prefix (`FFMPEG_PREFIX`, else the
one `build.sh --prefix` prints) and links FFmpeg's static libraries from it.
The Rust declarations are committed (`rusty_ffmpeg/src/binding.rs`), so the
build needs neither libclang nor FFmpeg's headers. Without a build it stops
with one line:

```
error: rusty_ffmpeg@0.17.0+ffmpeg.8.1: no FFmpeg build in ...; run `make ffmpeg` to build it (or set FFMPEG_PREFIX to a finished build)
```

The game binary then needs no FFmpeg at run time: `ldd target/release/rusty-claw`
lists no `libav*` or `libsw*` library.

To check the build is the LGPL one:

```sh
grep configure "$(sh third_party/ffmpeg/build.sh --prefix)/share/rusty-claw/ffmpeg-build.txt"
cargo test -p claw-movie --features ffmpeg --test smoke
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

## Windows and Steam Deck

Not covered yet: these notes are for Linux builds.
