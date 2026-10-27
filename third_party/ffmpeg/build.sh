#!/bin/sh
# Builds the pinned FFmpeg that the movie player links against, from source,
# as static libraries with only the decoders, demuxers and parsers the movies
# need, under the LGPL (no GPL or non-free parts), together with the pinned
# dav1d (VideoLAN's AV1 decoder, BSD-2-Clause) that FFmpeg decodes AV1 with.
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
# unset). Downloads, the pinned nasm, the meson and ninja it may install and
# the build tree live next to it in the cache; the build tree is removed on
# success.
# FFMPEG_JOBS sets the parallel compile jobs (default: the CPU count, at most 8).
# RUSTY_CLAW_FFMPEG_H26X=on adds the H.264 and HEVC decoders (off by default
# and in anything published; see README.md). The compilers run under
# `nice -n 10` where nice exists.
#
# dav1d is built with meson and ninja. When the system has no meson (0.54
# or newer) or no ninja, they are installed with pip into a virtual
# environment in the cache (never system-wide); when python3 cannot make
# one either, or pkg-config is missing, FFmpeg is built without AV1 and the
# build says so.
set -eu

FFMPEG_VERSION=8.1.3
FFMPEG_URL=https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz
FFMPEG_SHA256=7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3

# FFmpeg's and dav1d's x86 assembly (their fast paths, and the scaler's)
# needs the nasm assembler; a pinned copy is built into the cache.
NASM_VERSION=3.02
NASM_URL=https://www.nasm.us/pub/nasm/releasebuilds/$NASM_VERSION/nasm-$NASM_VERSION.tar.xz
NASM_SHA256=87336eba53b4acfe917424ab5d500d2b0054d9f5148d35c2273ccf2cfb712f0d

# dav1d decodes AV1 video for FFmpeg's libdav1d decoder; it is built as a
# static library into the same prefix.
DAV1D_VERSION=1.5.4
DAV1D_URL=https://downloads.videolan.org/pub/videolan/dav1d/$DAV1D_VERSION/dav1d-$DAV1D_VERSION.tar.xz
DAV1D_SHA256=686616b7c69eb88d44459391ab25cac13b6647a3b288835c5784e71c1514a5c5

# LGPL only. --disable-everything turns off every component (codec, format,
# parser, filter, device, protocol), and the lines after it enable exactly the
# ones the movies use: the game's AVI, VOB and Smacker movies and upscaled
# copies in MKV, WebM, MP4 or MOV with Opus sound (AV1 video comes with
# AV1_FLAGS below). Names are FFmpeg's configure names: `smacker` is the
# Smacker video decoder (registered as smackvid), the mpeg4 decoder brings
# the h263 decoder it is built on, and `matroska` reads WebM too.
# `mpegps` reads VOB files and leaves the codec of their picture stream to
# the probes of the raw demuxers, so a VOB's MPEG video is recognised only
# with the raw `mpegvideo` demuxer built in.
CONFIGURE_FLAGS="--disable-gpl --disable-nonfree --disable-everything --disable-autodetect \
--disable-programs --disable-doc --disable-network --disable-avdevice --disable-avfilter \
--enable-static --disable-shared --enable-pic --enable-pthreads \
--enable-decoder=mpeg4,mpeg2video,mp3,mp3float,mp2,ac3,aac,opus,smacker,smackaud,pcm_s16le,rawvideo \
--enable-demuxer=avi,smacker,mov,matroska,mpegps,mpegvideo \
--enable-parser=mpeg4video,mpegvideo,mpegaudio,ac3,aac,opus \
--enable-protocol=file \
--enable-swscale --enable-swresample"

# Added to the configure line whenever dav1d is built: AV1 video through
# dav1d (FFmpeg's own AV1 decoder only drives hardware decoders), and the
# static link flags of dav1d's pkg-config file.
AV1_FLAGS="--enable-libdav1d --enable-decoder=libdav1d --enable-parser=av1 --pkg-config-flags=--static"

# Added only with RUSTY_CLAW_FFMPEG_H26X=on, for people who build the game
# for themselves: H.264 and HEVC are patent-pooled formats, so a published
# build leaves their decoders out (see README.md).
H26X_FLAGS="--enable-decoder=h264,hevc --enable-parser=h264,hevc"
# End of the pins.

# The meson and ninja that pip installs when the system lacks them.
MESON_PIP=meson==1.10.2
NINJA_PIP=ninja==1.13.0

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
# meson and ninja from pip live here, when the system has none.
tools="$cache_root/tools"

case "${RUSTY_CLAW_FFMPEG_H26X:-off}" in
    on) h26x=yes ;;
    off | "") h26x=no ;;
    *) fail "RUSTY_CLAW_FFMPEG_H26X must be on or off, not '$RUSTY_CLAW_FFMPEG_H26X'" ;;
esac

# Whether a meson of at least 0.54 (dav1d's minimum) runs as "$@".
meson_ok() {
    v=$("$@" --version 2>/dev/null) || return 1
    major=${v%%.*}
    rest=${v#*.}
    minor=${rest%%.*}
    case "$major$minor" in
        *[!0-9]* | "") return 1 ;;
    esac
    [ "$major" -gt 0 ] || [ "$minor" -ge 54 ]
}

# How dav1d gets built, decided before anything is built because it is part
# of the prefix: av1=yes when meson and ninja are there (or pip can install
# them) and pkg-config is there for FFmpeg to find dav1d; else av1=no, with
# the reason in av1_missing.
av1=yes
av1_missing=
meson_cmd=
if command -v meson >/dev/null 2>&1 && meson_ok meson; then
    meson_cmd=meson
elif [ -x "$tools/bin/meson" ] && meson_ok "$tools/bin/meson"; then
    meson_cmd="$tools/bin/meson"
fi
ninja_found=no
for n in ninja ninja-build samu "$tools/bin/ninja"; do
    if command -v "$n" >/dev/null 2>&1; then ninja_found=yes; break; fi
done
need_pip=no
if [ -z "$meson_cmd" ] || [ "$ninja_found" = no ]; then
    need_pip=yes
    if ! python3 -c 'import venv, ensurepip' >/dev/null 2>&1; then
        av1=no
        if [ -z "$meson_cmd" ] && [ "$ninja_found" = no ]; then
            av1_missing="meson and ninja are missing"
        elif [ -z "$meson_cmd" ]; then
            av1_missing="meson (0.54 or newer) is missing"
        else
            av1_missing="ninja is missing"
        fi
        av1_missing="$av1_missing and python3 cannot make a virtual environment with pip to install them"
    fi
fi
if [ "$av1" = yes ] && ! command -v pkg-config >/dev/null 2>&1; then
    av1=no
    av1_missing="pkg-config is missing"
fi

configure_flags() {
    printf '%s' "$CONFIGURE_FLAGS"
    if [ "$av1" = yes ]; then printf ' %s' "$AV1_FLAGS"; fi
    if [ "$h26x" = yes ]; then printf ' %s' "$H26X_FLAGS"; fi
}

stamp_text() {
    printf 'FFmpeg %s\n' "$FFMPEG_VERSION"
    printf 'source %s\n' "$FFMPEG_URL"
    printf 'sha256 %s\n' "$FFMPEG_SHA256"
    if [ "$use_nasm" = yes ]; then
        printf 'nasm %s sha256 %s\n' "$NASM_VERSION" "$NASM_SHA256"
    fi
    if [ "$av1" = yes ]; then
        printf 'dav1d %s sha256 %s\n' "$DAV1D_VERSION" "$DAV1D_SHA256"
    else
        printf 'dav1d none\n'
    fi
    printf 'licence LGPL-2.1-or-later\n'
    printf 'configure %s\n' "$(configure_flags)"
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

# A finished or found build writes its prefix to last-build.txt in the cache
# folder when it names another one: cargo builds watch that file, to link
# the build next time (a rewrite of the same prefix would only rerun them).
signal() {
    if [ "$(cat "$cache_root/last-build.txt" 2>/dev/null || true)" != "$prefix" ]; then
        { mkdir -p "$cache_root" && printf '%s\n' "$prefix" >"$cache_root/last-build.txt"; } 2>/dev/null || true
    fi
}

# What the build decodes besides the original movies.
extras() {
    if [ "$av1" = yes ]; then
        printf 'AV1 (dav1d %s)' "$DAV1D_VERSION"
    else
        printf 'no AV1 (%s; install meson, ninja and pkg-config and run it again for AV1)' "$av1_missing"
    fi
    if [ "$h26x" = yes ]; then printf ', H.264 and HEVC'; fi
}

if up_to_date; then
    signal
    say "FFmpeg $FFMPEG_VERSION is already built in $prefix, with $(extras)"
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
        say "FFmpeg $FFMPEG_VERSION is already built in $prefix, with $(extras)"
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

stage="$work/stage"

# dav1d, built and installed into the staged prefix before FFmpeg, which
# finds it there through its pkg-config file.
if [ "$av1" = yes ]; then
    if [ "$need_pip" = yes ]; then
        wanted=
        [ -n "$meson_cmd" ] || wanted="$MESON_PIP"
        [ "$ninja_found" = yes ] || wanted="$wanted $NINJA_PIP"
        say "installing ${wanted# } with pip into $tools"
        if [ ! -x "$tools/bin/python" ]; then
            python3 -m venv "$tools" >"$work/pip.log" 2>&1 ||
                { tail -n 20 "$work/pip.log" >&2; fail "python3 could not make a virtual environment in $tools"; }
        fi
        # shellcheck disable=SC2086 # the requirements are words without spaces
        "$tools/bin/python" -m pip install --disable-pip-version-check --no-input -q $wanted >>"$work/pip.log" 2>&1 ||
            { tail -n 20 "$work/pip.log" >&2; fail "pip could not install ${wanted# }; install meson and ninja yourself and run it again"; }
        [ -n "$meson_cmd" ] || meson_cmd="$tools/bin/meson"
        PATH="$PATH:$tools/bin"
        export PATH
    fi
    fetch "$DAV1D_URL" "$DAV1D_SHA256"
    say "building dav1d $DAV1D_VERSION"
    tar -xJf "$cache_root/downloads/dav1d-$DAV1D_VERSION.tar.xz" -C "$work"
    dav1d_asm=true
    # dav1d's x86 assembly needs nasm, which is only built for x86 above.
    case "$(uname -m)" in
        x86_64 | amd64 | i?86) [ "$use_nasm" = yes ] || dav1d_asm=false ;;
    esac
    (
        cd "$work/dav1d-$DAV1D_VERSION"
        $nice "$meson_cmd" setup "$work/dav1d-build" . --prefix="$prefix" --libdir=lib \
            --buildtype=release --default-library=static --wrap-mode=nodownload \
            -Denable_tools=false -Denable_tests=false -Denable_examples=false \
            -Denable_docs=false -Denable_asm="$dav1d_asm" >"$work/dav1d-setup.log" 2>&1 ||
            { tail -n 20 "$work/dav1d-setup.log" >&2; exit 1; }
        $nice "$meson_cmd" compile -C "$work/dav1d-build" -j "$jobs" >"$work/dav1d-make.log" 2>&1 ||
            { tail -n 30 "$work/dav1d-make.log" >&2; exit 1; }
        DESTDIR="$stage" "$meson_cmd" install -C "$work/dav1d-build" >"$work/dav1d-install.log" 2>&1 ||
            { tail -n 20 "$work/dav1d-install.log" >&2; exit 1; }
    ) || fail "dav1d build failed (logs in $work)"
    [ -f "$stage$prefix/lib/libdav1d.a" ] || fail "the dav1d build made no libdav1d.a (logs in $work)"
    # FFmpeg looks for dav1d only in the staged prefix, never in the system.
    PKG_CONFIG_LIBDIR="$stage$prefix/lib/pkgconfig"
    PKG_CONFIG_SYSROOT_DIR="$stage"
    export PKG_CONFIG_LIBDIR PKG_CONFIG_SYSROOT_DIR
else
    say "building without AV1: $av1_missing"
fi

flags=$(configure_flags)
fetch "$FFMPEG_URL" "$FFMPEG_SHA256"
say "building FFmpeg $FFMPEG_VERSION with $jobs jobs into $prefix"
tar -xJf "$cache_root/downloads/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$work"
src="$work/ffmpeg-$FFMPEG_VERSION"
(
    cd "$src"
    # shellcheck disable=SC2086 # the flags are words without spaces
    $nice ./configure --prefix="$prefix" $flags "$nasm_flag" >"$work/configure.log" 2>&1 ||
        { tail -n 20 ffbuild/config.log >&2; exit 1; }
    # configure skips a misspelt component quietly; every listed one must be on.
    for flag in $flags; do
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

# FFmpeg's pkg-config files name dav1d's folder as configure saw it, in the
# stage; they move with the prefix.
for pc in "$stage$prefix"/lib/pkgconfig/*.pc; do
    sed "s|$stage$prefix|$prefix|g" "$pc" >"$pc.new" && mv "$pc.new" "$pc"
done

# The licence texts and the build record travel with the libraries.
mkdir -p "$stage$prefix/share/licenses/ffmpeg" "$stage$prefix/share/rusty-claw"
cp "$src/LICENSE.md" "$src/COPYING.LGPLv2.1" "$stage$prefix/share/licenses/ffmpeg/"
if [ "$av1" = yes ]; then
    mkdir -p "$stage$prefix/share/licenses/dav1d"
    cp "$work/dav1d-$DAV1D_VERSION/COPYING" "$stage$prefix/share/licenses/dav1d/"
fi
stamp_text >"$stage$prefix/share/rusty-claw/ffmpeg-build.txt"

mkdir -p "$(dirname "$prefix")"
mv "$stage$prefix" "$prefix"
rm -rf "$work"
signal
say "FFmpeg $FFMPEG_VERSION is built in $prefix, with $(extras)"
