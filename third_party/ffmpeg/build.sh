#!/bin/sh
# Builds the pinned FFmpeg that the movie player links against, from source,
# as static libraries with only the decoders, demuxers and parsers the movies
# need, under the LGPL (no GPL or non-free parts).
#
#   sh third_party/ffmpeg/build.sh            build, or reuse a finished build
#   sh third_party/ffmpeg/build.sh --prefix   print the install prefix only
#
# `make ffmpeg` runs it, and so does the first `cargo build` of the game when
# the prefix holds no build yet (the rusty_ffmpeg build script).
# The prefix is ${XDG_CACHE_HOME:-~/.cache}/rusty-claw/ffmpeg/<version>-<hash>,
# where the hash covers the pins and the configure line, so a change to either
# builds into a new prefix and an unchanged pin reuses the old one in seconds.
# FFMPEG_PREFIX, an absolute path, overrides the prefix (empty is the same as
# unset). Downloads, the pinned nasm and the build tree live next to it in
# the cache; the build tree is removed on success.
# FFMPEG_JOBS sets the parallel compile jobs (default: the CPU count, at most 8).
# The compilers run under `nice -n 10` where nice exists.
set -eu

FFMPEG_VERSION=8.1.3
FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz
FFMPEG_SHA256=7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3

# FFmpeg's x86 assembly (the fast paths of the H.264 and HEVC decoders and of
# the scaler) needs the nasm assembler; a pinned copy is built into the cache.
NASM_VERSION=3.02
NASM_URL=https://www.nasm.us/pub/nasm/releasebuilds/$NASM_VERSION/nasm-$NASM_VERSION.tar.xz
NASM_SHA256=87336eba53b4acfe917424ab5d500d2b0054d9f5148d35c2273ccf2cfb712f0d

# LGPL only. --disable-everything turns off every component (codec, format,
# parser, filter, device, protocol), and the lines after it enable exactly the
# ones the movies use: the game's AVI, VOB and Smacker movies and upscaled
# copies in MP4, MOV or MKV with H.264 or HEVC video. Names are FFmpeg's
# configure names: `smacker` is the Smacker video decoder (registered as
# smackvid), and the mpeg4 decoder brings the h263 decoder it is built on.
# `mpegps` reads VOB files and leaves the codec of their picture stream to
# the probes of the raw demuxers, so a VOB's MPEG video is recognised only
# with the raw `mpegvideo` demuxer built in.
CONFIGURE_FLAGS="--disable-gpl --disable-nonfree --disable-everything --disable-autodetect \
--disable-programs --disable-doc --disable-network --disable-avdevice --disable-avfilter \
--enable-static --disable-shared --enable-pic --enable-pthreads \
--enable-decoder=mpeg4,h264,hevc,mpeg2video,mp3,mp3float,mp2,ac3,aac,smacker,smackaud,pcm_s16le,rawvideo \
--enable-demuxer=avi,smacker,mov,matroska,mpegps,mpegvideo \
--enable-parser=mpeg4video,h264,hevc,mpegvideo,mpegaudio,ac3,aac \
--enable-protocol=file \
--enable-swscale --enable-swresample"

say() { printf 'ffmpeg: %s\n' "$*"; }
fail() { printf 'ffmpeg: %s\n' "$*" >&2; exit 1; }

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

case "$(uname -m)" in
    x86_64 | amd64 | i?86) use_nasm=yes ;;
    *) use_nasm=no ;;
esac

cache_root="${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/ffmpeg"
stamp_text() {
    printf 'FFmpeg %s\n' "$FFMPEG_VERSION"
    printf 'source %s\n' "$FFMPEG_URL"
    printf 'sha256 %s\n' "$FFMPEG_SHA256"
    if [ "$use_nasm" = yes ]; then
        printf 'nasm %s sha256 %s\n' "$NASM_VERSION" "$NASM_SHA256"
    fi
    printf 'licence LGPL-2.1-or-later\n'
    printf 'configure %s\n' "$CONFIGURE_FLAGS"
}
config_hash=$(stamp_text | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12)
case "${FFMPEG_PREFIX:-/}" in
    /*) ;;
    *) fail "FFMPEG_PREFIX must be an absolute path, not '$FFMPEG_PREFIX'" ;;
esac
prefix="${FFMPEG_PREFIX:-$cache_root/$FFMPEG_VERSION-$config_hash}"
stamp="$prefix/share/rusty-claw/ffmpeg-build.txt"

case "${1:-}" in
    --prefix) printf '%s\n' "$prefix"; exit 0 ;;
    "") ;;
    *) fail "unknown option '$1' (expected --prefix or nothing)" ;;
esac

up_to_date() { [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$(stamp_text)" ]; }

# Each finished or found build rewrites last-build.txt in the cache folder:
# a cargo build that found no FFmpeg watches it, to link the build next time.
signal() {
    { mkdir -p "$cache_root" && printf '%s\n' "$prefix" >"$cache_root/last-build.txt"; } 2>/dev/null || true
}

if up_to_date; then
    signal
    say "FFmpeg $FFMPEG_VERSION is already built in $prefix"
    exit 0
fi
if [ -e "$prefix" ]; then
    fail "$prefix exists but holds another build; remove it or set FFMPEG_PREFIX elsewhere"
fi

mkdir -p "$cache_root/downloads" "$cache_root/build"
# One build at a time per machine: a second caller waits, then finds it done.
if command -v flock >/dev/null 2>&1; then
    exec 9>"$cache_root/.lock"
    flock 9
    if up_to_date; then
        signal
        say "FFmpeg $FFMPEG_VERSION is already built in $prefix"
        exit 0
    fi
fi

jobs="${FFMPEG_JOBS:-$(nproc 2>/dev/null || echo 4)}"
if [ -z "${FFMPEG_JOBS:-}" ] && [ "$jobs" -gt 8 ]; then jobs=8; fi

# The builds run at a lower priority, so the desktop stays responsive.
if command -v nice >/dev/null 2>&1; then
    nice="nice -n 10"
else
    nice=
fi

# fetch URL SHA256: downloads into the cache once and checks the checksum.
fetch() {
    dest="$cache_root/downloads/${1##*/}"
    if [ -f "$dest" ] && [ "$(sha256_of "$dest")" = "$2" ]; then
        return 0
    fi
    say "fetching $1"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 2 -o "$dest.part" "$1" || { rm -f "$dest.part"; fail "download failed: $1"; }
    else
        wget -q --tries=3 -O "$dest.part" "$1" || { rm -f "$dest.part"; fail "download failed: $1"; }
    fi
    got=$(sha256_of "$dest.part")
    if [ "$got" != "$2" ]; then
        rm -f "$dest.part"
        fail "${1##*/} has SHA-256 $got, expected $2; nothing was built"
    fi
    mv "$dest.part" "$dest"
}

work="$cache_root/build/$FFMPEG_VERSION-$config_hash"
rm -rf "$work"
mkdir -p "$work"

nasm_flag=--disable-x86asm
if [ "$use_nasm" = yes ]; then
    nasm_flag=--enable-x86asm
    nasm_bin="$cache_root/nasm-$NASM_VERSION/bin/nasm"
    if [ ! -x "$nasm_bin" ]; then
        fetch "$NASM_URL" "$NASM_SHA256"
        say "building nasm $NASM_VERSION"
        tar -xJf "$cache_root/downloads/nasm-$NASM_VERSION.tar.xz" -C "$work"
        (
            cd "$work/nasm-$NASM_VERSION"
            $nice ./configure --prefix="$cache_root/nasm-$NASM_VERSION" >"$work/nasm-configure.log" 2>&1 ||
                { tail -n 20 "$work/nasm-configure.log" >&2; exit 1; }
            $nice make -j"$jobs" nasm >"$work/nasm-make.log" 2>&1 ||
                { tail -n 20 "$work/nasm-make.log" >&2; exit 1; }
            mkdir -p "$cache_root/nasm-$NASM_VERSION/bin"
            cp nasm "$nasm_bin.part" && mv "$nasm_bin.part" "$nasm_bin"
        ) || fail "nasm build failed (logs in $work)"
    fi
    # On PATH rather than --x86asmexe, so the configure line FFmpeg keeps in
    # its libraries names no local path but the prefix.
    PATH="$cache_root/nasm-$NASM_VERSION/bin:$PATH"
    export PATH
fi

fetch "$FFMPEG_URL" "$FFMPEG_SHA256"
say "building FFmpeg $FFMPEG_VERSION with $jobs jobs into $prefix"
tar -xJf "$cache_root/downloads/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$work"
src="$work/ffmpeg-$FFMPEG_VERSION"
stage="$work/stage"
(
    cd "$src"
    # shellcheck disable=SC2086 # the flags are words without spaces
    $nice ./configure --prefix="$prefix" $CONFIGURE_FLAGS "$nasm_flag" >"$work/configure.log" 2>&1 ||
        { tail -n 20 ffbuild/config.log >&2; exit 1; }
    # configure skips a misspelt component quietly; every listed one must be on.
    for flag in $CONFIGURE_FLAGS; do
        case "$flag" in
            --enable-decoder=* | --enable-demuxer=* | --enable-parser=* | --enable-protocol=*)
                kind=${flag#--enable-}
                kind=${kind%%=*}
                for name in $(printf '%s' "${flag#*=}" | tr ',' ' '); do
                    macro=$(printf 'CONFIG_%s_%s 1' "$name" "$kind" | tr '[:lower:]' '[:upper:]')
                    grep -q "^#define $macro\$" config_components.h ||
                        { echo "the $kind '$name' is not enabled" >&2; exit 1; }
                done
                ;;
        esac
    done
    $nice make -j"$jobs" >"$work/make.log" 2>&1 || { tail -n 30 "$work/make.log" >&2; exit 1; }
    make install DESTDIR="$stage" >"$work/install.log" 2>&1 || { tail -n 20 "$work/install.log" >&2; exit 1; }
) || fail "FFmpeg build failed (logs in $work)"

# The licence texts and the build record travel with the libraries.
mkdir -p "$stage$prefix/share/licenses/ffmpeg" "$stage$prefix/share/rusty-claw"
cp "$src/LICENSE.md" "$src/COPYING.LGPLv2.1" "$stage$prefix/share/licenses/ffmpeg/"
stamp_text >"$stage$prefix/share/rusty-claw/ffmpeg-build.txt"

mkdir -p "$(dirname "$prefix")"
mv "$stage$prefix" "$prefix"
rm -rf "$work"
signal
say "FFmpeg $FFMPEG_VERSION is built in $prefix"
