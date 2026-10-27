#!/bin/sh
# Builds the pinned FFmpeg that the movie player links against, from source,
# as static libraries with only the decoders, demuxers and parsers the movies
# need, under the LGPL (no GPL or non-free parts), together with the pinned
# dav1d (VideoLAN's AV1 decoder, BSD-2-Clause) that FFmpeg decodes AV1 with.
#
#   sh third_party/ffmpeg/build.sh              build, or reuse a finished build
#   sh third_party/ffmpeg/build.sh --retry-av1  the same, trying AV1 again after
#                                               an earlier attempt failed
#   sh third_party/ffmpeg/build.sh --prefix     print the install prefix only
#   sh third_party/ffmpeg/build.sh --av1        print whether that build has AV1:
#                                               `yes`, or `no: ` and the reason
#
# `make ffmpeg` runs it with --retry-av1, and the first `cargo build` of the
# game runs it without when the prefix holds no build yet (the rusty_ffmpeg
# build script). --prefix and --av1 only look around (which tools there
# are, and python3's version); they write nothing.
# The prefix is ${XDG_CACHE_HOME:-~/.cache}/rusty-claw/ffmpeg/<version>-<hash>,
# where the hash covers the pins and the configure line, so a change to either
# builds into a new prefix and an unchanged pin reuses the old one in seconds.
# FFMPEG_PREFIX, an absolute path, overrides the prefix (empty is the same as
# unset); a build there without AV1 gives way to the same build with AV1
# once one succeeds (make ffmpeg after installing what AV1 needed).
# Downloads, the pinned nasm, meson and ninja it may need and the build tree
# live next to it in the cache; the build tree is removed on success.
# FFMPEG_JOBS sets the parallel compile jobs (default: the CPU count, at most
# 8; inside a make or cargo job server, FFmpeg's make takes its jobs from it
# and ninja runs half as many). RUSTY_CLAW_FFMPEG_H26X=on adds the H.264 and
# HEVC decoders (off by default and in anything published; see README.md).
# The compilers run under `nice -n 10` where nice exists.
#
# AV1 is an extra. dav1d is built with meson and ninja: the system's when it
# has them (meson 0.54 or newer; ninja 1.8.2 or newer, or samu), else the
# pinned releases below, downloaded into the cache and checked like FFmpeg:
# meson runs with python3 (3.7 or newer), and ninja is built with python3
# and a C++ compiler. FFmpeg is built without AV1, into a prefix of its own,
# when RUSTY_CLAW_FFMPEG_AV1=off, when pkg-config or those tools are
# missing, or when getting the tools, building dav1d or building FFmpeg with
# it fails; the build says why, and the original movies play either way. A
# failed attempt is kept in av1-failed.txt in the cache folder together with
# what could change its outcome (the pins, and the compilers, meson, ninja,
# python3 and pkg-config found), and later builds go on without AV1 until
# one of those changes or --retry-av1 tries again; a build with AV1 that is
# already there is still used.
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

# The meson and ninja that build dav1d where the system has none: meson's
# release (pure Python, run as `python3 meson.py`) and ninja's source
# (built with `python3 configure.py --bootstrap`), both kept in the cache.
# They only build dav1d; nothing of them goes into the game.
MESON_VERSION=1.10.2
MESON_URL=https://github.com/mesonbuild/meson/releases/download/$MESON_VERSION/meson-$MESON_VERSION.tar.gz
MESON_SHA256=7890287d911dd4ee1ebd0efb61ed0321bfcd87c725df923a837cf90c6508f96b
# ninja publishes no source tarball of its own, so its source comes as the
# archive GitHub makes of the release tag. Those bytes are not guaranteed to
# stay the same: a SHA-256 mismatch on it means the pin needs refreshing
# (see README.md), and until then builds without a system ninja go on
# without AV1.
NINJA_VERSION=1.13.2
NINJA_URL=https://github.com/ninja-build/ninja/archive/v$NINJA_VERSION/ninja-$NINJA_VERSION.tar.gz
NINJA_SHA256=974d6b2f4eeefa25625d34da3cb36bdcebe7fbce40f4c16ac0835fd1c0cbae17

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

say() { printf 'ffmpeg: %s\n' "$*"; }
fail() { printf 'ffmpeg: %s\n' "$*" >&2; exit 1; }

action=build
retry_av1=no
case "${1:-}" in
    --prefix) action=prefix ;;
    --av1) action=av1 ;;
    --retry-av1) retry_av1=yes ;;
    "") ;;
    *) fail "unknown option '$1' (expected --prefix, --av1, --retry-av1 or nothing)" ;;
esac

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}
sha256_text() { { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-12; }

case "$(uname -m)" in
    x86_64 | amd64 | i?86) use_nasm=yes ;;
    *) use_nasm=no ;;
esac

cache_root="${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/ffmpeg"
# A failed AV1 attempt, and its log.
av1_record="$cache_root/av1-failed.txt"
av1_log="$cache_root/av1-failed.log"

case "${RUSTY_CLAW_FFMPEG_H26X:-off}" in
    on) h26x=yes ;;
    off | "") h26x=no ;;
    *) fail "RUSTY_CLAW_FFMPEG_H26X must be on or off, not '$RUSTY_CLAW_FFMPEG_H26X'" ;;
esac
case "${RUSTY_CLAW_FFMPEG_AV1:-auto}" in
    auto | "") av1_off=no ;;
    off) av1_off=yes ;;
    *) fail "RUSTY_CLAW_FFMPEG_AV1 must be auto or off, not '$RUSTY_CLAW_FFMPEG_AV1'" ;;
esac

# Whether the version $1 (as `--version` prints it) is at least $2.$3.$4.
version_ge() {
    rest=$1.0.0
    va=${rest%%.*}
    rest=${rest#*.}
    vb=${rest%%.*}
    rest=${rest#*.}
    vc=${rest%%.*}
    vc=${vc%%[!0-9]*}
    case "$va" in *[!0-9]* | "") return 1 ;; esac
    case "$vb" in *[!0-9]* | "") return 1 ;; esac
    [ -n "$vc" ] || vc=0
    if [ "$va" -ne "$2" ]; then [ "$va" -gt "$2" ]; return; fi
    if [ "$vb" -ne "$3" ]; then [ "$vb" -gt "$3" ]; return; fi
    [ "$vc" -ge "$4" ]
}

# Whether the tool $1 reports a version of at least $2.$3.$4; the version
# it reports is left in tool_version.
tool_ok() {
    tool_version=$("$1" --version 2>/dev/null) || return 1
    version_ge "$tool_version" "$2" "$3" "$4"
}

# How dav1d gets built, decided before anything is built because it is part
# of the prefix. Only cheap looks: which tools are on PATH, their versions
# and python3's. av1=yes when RUSTY_CLAW_FFMPEG_AV1 is not off, pkg-config
# is there for FFmpeg to find dav1d, meson and ninja are there or can be
# had from their pinned releases, and no earlier attempt with the same
# outcome-deciding things failed (see av1_key); else av1=no, with the reason
# in av1_missing. meson_cmd and ninja_cmd are the system's tools; an empty
# one is the pinned release, run or built from the cache.
av1=yes
av1_missing=
meson_cmd=
meson_version=
ninja_cmd=
ninja_version=
python3_cmd=
python3_version=
cxx_cmd=
pkg_config_cmd=$(command -v pkg-config 2>/dev/null || true)
if [ "$av1_off" = yes ]; then
    av1=no
    av1_missing="RUSTY_CLAW_FFMPEG_AV1=off leaves it out"
elif [ -z "$pkg_config_cmd" ]; then
    av1=no
    av1_missing="pkg-config is missing (install it, then run make ffmpeg)"
else
    if command -v meson >/dev/null 2>&1 && tool_ok meson 0 54 0; then
        meson_cmd=$(command -v meson)
        meson_version=$tool_version
    fi
    for n in ninja ninja-build samu; do
        if command -v "$n" >/dev/null 2>&1 && tool_ok "$n" 1 8 2; then
            ninja_cmd=$(command -v "$n")
            ninja_version=$tool_version
            break
        fi
    done
    if [ -z "$meson_cmd" ] || [ -z "$ninja_cmd" ]; then
        python3_cmd=$(command -v python3 2>/dev/null || true)
        if [ -n "$python3_cmd" ]; then
            python3_version=$("$python3_cmd" -c 'import sys; print("%d.%d.%d" % sys.version_info[:3])' 2>/dev/null || true)
            [ -n "$python3_version" ] || python3_cmd=
        fi
    fi
    if [ -z "$meson_cmd" ] && ! { [ -n "$python3_cmd" ] && version_ge "$python3_version" 3 7 0; }; then
        av1=no
        if [ -n "$python3_cmd" ]; then
            av1_missing="meson 0.54 or newer is missing, and python3 $python3_version is too old to run the pinned meson (3.7 or newer); install either, then run make ffmpeg"
        else
            av1_missing="meson 0.54 or newer is missing, and so is python3 (3.7 or newer) to run the pinned meson; install either, then run make ffmpeg"
        fi
    fi
    if [ "$av1" = yes ] && [ -z "$ninja_cmd" ]; then
        if [ -n "${CXX:-}" ]; then
            # Its first word, as in CXX="ccache g++".
            set -f
            # shellcheck disable=SC2086 # split into words on purpose
            set -- $CXX
            set +f
            cxx_cmd=$(command -v "${1:-}" 2>/dev/null || true)
        else
            for c in c++ g++ clang++; do
                if command -v "$c" >/dev/null 2>&1; then
                    cxx_cmd=$(command -v "$c")
                    break
                fi
            done
        fi
        if [ -z "$python3_cmd" ] || [ -z "$cxx_cmd" ]; then
            av1=no
            av1_missing="ninja 1.8.2 or newer is missing, and building the pinned ninja needs python3 and a C++ compiler (c++); install either, then run make ffmpeg"
        fi
    fi
fi

# What could change how an AV1 attempt ends: the pins it uses and the tools
# it would use.
av1_key() {
    {
        printf 'dav1d %s %s\n' "$DAV1D_VERSION" "$DAV1D_SHA256"
        printf 'cc %s %s\n' "${CC:-}" "$(command -v cc 2>/dev/null || true)"
        printf 'nasm %s %s\n' "$use_nasm" "$NASM_SHA256"
        printf 'pkg-config %s\n' "$pkg_config_cmd"
        if [ -n "$meson_cmd" ]; then
            printf 'meson %s %s\n' "$meson_cmd" "$meson_version"
        else
            printf 'meson %s %s python3 %s %s\n' "$MESON_VERSION" "$MESON_SHA256" "$python3_cmd" "$python3_version"
        fi
        if [ -n "$ninja_cmd" ]; then
            printf 'ninja %s %s\n' "$ninja_cmd" "$ninja_version"
        else
            printf 'ninja %s %s python3 %s %s c++ %s %s\n' "$NINJA_VERSION" "$NINJA_SHA256" \
                "$python3_cmd" "$python3_version" "${CXX:-}" "$cxx_cmd"
        fi
        uname -m
    } | sha256_text
}

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
        printf 'meson %s sha256 %s (where the system has none)\n' "$MESON_VERSION" "$MESON_SHA256"
        printf 'ninja %s sha256 %s (where the system has none)\n' "$NINJA_VERSION" "$NINJA_SHA256"
    else
        printf 'dav1d none\n'
    fi
    printf 'licence LGPL-2.1-or-later\n'
    printf 'configure %s\n' "$(configure_flags)"
}

case "${FFMPEG_PREFIX:-/}" in
    /*) ;;
    *) fail "FFMPEG_PREFIX must be an absolute path, not '$FFMPEG_PREFIX'" ;;
esac
# Sets config_hash, prefix and stamp for the current choices.
choose_prefix() {
    config_hash=$(stamp_text | sha256_text)
    prefix="${FFMPEG_PREFIX:-$cache_root/$FFMPEG_VERSION-$config_hash}"
    stamp="$prefix/share/rusty-claw/ffmpeg-build.txt"
}
up_to_date() { [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$(stamp_text)" ]; }
choose_prefix

# An AV1 attempt that failed before with the same key is not made again
# unless --retry-av1 asks, but a build with AV1 for these choices that is
# already there (made before the failure, or in FFMPEG_PREFIX) is still the
# one to use. Only the record and the stamp are read.
if [ "$av1" = yes ] && [ "$retry_av1" = no ] && [ -f "$av1_record" ] &&
    [ "$(sed -n 1p "$av1_record")" = "key $(av1_key)" ] && ! up_to_date; then
    av1=no
    av1_missing="$(sed -n 2p "$av1_record") (an earlier build found this; make ffmpeg tries again)"
    choose_prefix
fi

case "$action" in
    prefix) printf '%s\n' "$prefix"; exit 0 ;;
    av1)
        if [ "$av1" = yes ]; then echo yes; else printf 'no: %s\n' "$av1_missing"; fi
        exit 0
        ;;
esac

# A found build writes its prefix to last-build.txt in the cache folder when
# it names another one, and a finished build always rewrites it: cargo
# builds watch that file, to link the build next time (a rewrite of the same
# prefix for a found build would only rerun them, but a rebuilt prefix that
# was deleted must rerun a build that fell back to no movies).
signal() {
    if [ "${1:-}" = rebuilt ] ||
        [ "$(cat "$cache_root/last-build.txt" 2>/dev/null || true)" != "$prefix" ]; then
        { mkdir -p "$cache_root" && printf '%s\n' "$prefix" >"$cache_root/last-build.txt"; } 2>/dev/null || true
    fi
}

# What the build decodes besides the original movies.
extras() {
    if [ "$av1" = yes ]; then
        printf 'AV1 (dav1d %s)' "$DAV1D_VERSION"
    else
        printf 'no AV1 (%s)' "$av1_missing"
    fi
    if [ "$h26x" = yes ]; then printf ', H.264 and HEVC'; fi
}

if up_to_date; then
    signal
    say "FFmpeg $FFMPEG_VERSION is already built in $prefix, with $(extras)"
    exit 0
fi
# In FFMPEG_PREFIX, the same build without AV1 is replaced once one with
# AV1 is ready (a failed attempt leaves it as it is); any other build there
# is left alone.
replace=no
if [ -e "$prefix" ]; then
    if [ -n "${FFMPEG_PREFIX:-}" ] && [ "$av1" = yes ] && [ -f "$stamp" ] &&
        [ "$(cat "$stamp")" = "$(av1=no; stamp_text)" ]; then
        replace=yes
    else
        fail "$prefix exists but holds another build; remove it or set FFMPEG_PREFIX elsewhere"
    fi
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
# make joins the job server of a make or cargo that runs this (cargo hands
# its own to the rusty_ffmpeg build script, which passes it on as
# MAKEFLAGS), unless FFMPEG_JOBS says otherwise; ninja cannot, so it runs
# half the jobs then.
make_jobs=-j$jobs
ninja_jobs=$jobs
case "${MAKEFLAGS:-}" in
    *jobserver-auth=* | *jobserver-fds=*)
        if [ -z "${FFMPEG_JOBS:-}" ]; then
            make_jobs=
            ninja_jobs=$(((jobs + 1) / 2))
        else
            unset MAKEFLAGS
        fi
        ;;
esac

# The builds run at a lower priority, so the desktop stays responsive.
if command -v nice >/dev/null 2>&1; then
    nice="nice -n 10"
else
    nice=
fi

# try_fetch URL SHA256: downloads into the cache once and checks the
# checksum; on failure, says why in fetch_why.
try_fetch() {
    dest="$cache_root/downloads/${1##*/}"
    if [ -f "$dest" ] && [ "$(sha256_of "$dest")" = "$2" ]; then
        return 0
    fi
    say "fetching $1"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 2 -o "$dest.part" "$1" ||
            { rm -f "$dest.part"; fetch_why="download failed: $1"; return 1; }
    else
        wget -q --tries=3 -O "$dest.part" "$1" ||
            { rm -f "$dest.part"; fetch_why="download failed: $1"; return 1; }
    fi
    got=$(sha256_of "$dest.part")
    if [ "$got" != "$2" ]; then
        rm -f "$dest.part"
        fetch_why="${1##*/} has SHA-256 $got, expected $2"
        return 1
    fi
    mv "$dest.part" "$dest"
}
# fetch URL SHA256: the same, stopping the build on failure.
fetch() {
    try_fetch "$1" "$2" || fail "$fetch_why; nothing was built"
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
            # shellcheck disable=SC2086 # empty when make joins a job server
            $nice make $make_jobs nasm >"$work/nasm-make.log" 2>&1 ||
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

# Shows the end of the log $1 and keeps it as av1-failed.log, for the
# reason a failed AV1 attempt gives.
keep_log() {
    tail -n 20 "$1" >&2
    cp "$1" "$av1_log" 2>/dev/null || true
}

# Gets meson and ninja when the system has none, then builds dav1d and
# installs it into the staged prefix, where FFmpeg finds it through its
# pkg-config file. Returns 1 on any failure, with the reason in av1_why;
# then FFmpeg is built without AV1.
build_dav1d() {
    mkdir -p "$work/bin" || { av1_why="cannot write $work"; return 1; }
    if [ -z "$meson_cmd" ]; then
        meson_dir="$cache_root/meson-$MESON_VERSION"
        if [ ! -f "$meson_dir/meson.py" ]; then
            try_fetch "$MESON_URL" "$MESON_SHA256" || { av1_why="getting meson failed: $fetch_why"; return 1; }
            if ! { tar -xzf "$cache_root/downloads/meson-$MESON_VERSION.tar.gz" -C "$work" &&
                [ -f "$work/meson-$MESON_VERSION/meson.py" ]; }; then
                av1_why="cannot unpack meson-$MESON_VERSION.tar.gz"
                return 1
            fi
            if ! { rm -rf "$meson_dir" && mv "$work/meson-$MESON_VERSION" "$meson_dir"; }; then
                av1_why="cannot write $meson_dir"
                return 1
            fi
        fi
        # dav1d's build calls it as meson, from this script only.
        if ! {
            printf '#!/bin/sh\n' &&
                printf 'exec "%s" "%s" "$@"\n' "$python3_cmd" "$meson_dir/meson.py"
        } >"$work/bin/meson" || ! chmod +x "$work/bin/meson"; then
            av1_why="cannot write $work/bin/meson"
            return 1
        fi
        meson_cmd="$work/bin/meson"
        "$meson_cmd" --version >/dev/null 2>&1 ||
            { av1_why="meson $MESON_VERSION does not run with python3 $python3_version"; return 1; }
        say "building dav1d with meson $MESON_VERSION from the cache"
    fi
    if [ -z "$ninja_cmd" ]; then
        ninja_dir="$cache_root/ninja-$NINJA_VERSION"
        if [ ! -x "$ninja_dir/bin/ninja" ]; then
            try_fetch "$NINJA_URL" "$NINJA_SHA256" || { av1_why="getting ninja failed: $fetch_why"; return 1; }
            say "building ninja $NINJA_VERSION"
            tar -xzf "$cache_root/downloads/ninja-$NINJA_VERSION.tar.gz" -C "$work" ||
                { av1_why="cannot unpack ninja-$NINJA_VERSION.tar.gz"; return 1; }
            (
                cd "$work/ninja-$NINJA_VERSION" || exit 1
                CXX="${CXX:-$cxx_cmd}"
                export CXX
                $nice "$python3_cmd" configure.py --bootstrap >"$work/ninja-build.log" 2>&1
            ) || { keep_log "$work/ninja-build.log"; av1_why="building ninja $NINJA_VERSION failed (log in $av1_log)"; return 1; }
            { mkdir -p "$ninja_dir/bin" &&
                cp "$work/ninja-$NINJA_VERSION/ninja" "$ninja_dir/bin/ninja.part" &&
                mv "$ninja_dir/bin/ninja.part" "$ninja_dir/bin/ninja"; } ||
                { av1_why="cannot write $ninja_dir"; return 1; }
        fi
        ninja_cmd="$ninja_dir/bin/ninja"
    fi
    # meson looks for ninja by name on PATH (or in NINJA), so the chosen one,
    # which may be the cache's or a system ninja-build, goes first on PATH
    # under the name ninja, for this build only.
    ln -s "$ninja_cmd" "$work/bin/ninja" || { av1_why="cannot write $work/bin/ninja"; return 1; }
    try_fetch "$DAV1D_URL" "$DAV1D_SHA256" || { av1_why="getting dav1d failed: $fetch_why"; return 1; }
    say "building dav1d $DAV1D_VERSION"
    tar -xJf "$cache_root/downloads/dav1d-$DAV1D_VERSION.tar.xz" -C "$work" ||
        { av1_why="cannot unpack dav1d-$DAV1D_VERSION.tar.xz"; return 1; }
    dav1d_asm=true
    # dav1d's x86 assembly needs nasm, which is only built for x86 above.
    case "$(uname -m)" in
        x86_64 | amd64 | i?86) [ "$use_nasm" = yes ] || dav1d_asm=false ;;
    esac
    (
        PATH="$work/bin:$PATH"
        NINJA="$work/bin/ninja"
        export PATH NINJA
        cd "$work/dav1d-$DAV1D_VERSION" || exit 1
        $nice "$meson_cmd" setup "$work/dav1d-build" . --prefix="$prefix" --libdir=lib \
            --buildtype=release --default-library=static --wrap-mode=nodownload \
            -Denable_tools=false -Denable_tests=false -Denable_examples=false \
            -Denable_docs=false -Denable_asm="$dav1d_asm" >"$work/dav1d.log" 2>&1 || exit 1
        $nice "$meson_cmd" compile -C "$work/dav1d-build" -j "$ninja_jobs" >>"$work/dav1d.log" 2>&1 || exit 1
        DESTDIR="$stage" "$meson_cmd" install -C "$work/dav1d-build" >>"$work/dav1d.log" 2>&1 || exit 1
    ) || { keep_log "$work/dav1d.log"; av1_why="building dav1d $DAV1D_VERSION failed (log in $av1_log)"; return 1; }
    [ -f "$stage$prefix/lib/libdav1d.a" ] ||
        { keep_log "$work/dav1d.log"; av1_why="the dav1d build made no libdav1d.a (log in $av1_log)"; return 1; }
}

# AV1 is an extra: when an attempt fails, says why, keeps the reason $1 in
# av1-failed.txt (later builds skip the attempt, see the top) and goes on
# without AV1, in the prefix that build has; when that build is there
# already, it is the result.
without_av1() {
    if printf 'key %s\n%s\n' "$attempt_key" "$1" >"$av1_record.part" 2>/dev/null; then
        mv "$av1_record.part" "$av1_record" || true
    fi
    av1=no
    av1_missing="$1; make ffmpeg tries again"
    say "building without AV1: $av1_missing"
    rm -rf "$stage"
    choose_prefix
    if up_to_date; then
        rm -rf "$work"
        signal
        say "FFmpeg $FFMPEG_VERSION is already built in $prefix, with $(extras)"
        exit 0
    fi
    if [ -e "$prefix" ]; then
        fail "$prefix exists but holds another build; remove it or set FFMPEG_PREFIX elsewhere"
    fi
}

if [ "$av1" = yes ]; then
    av1_why=
    # Before build_dav1d, which sets meson_cmd and ninja_cmd to what it got.
    attempt_key=$(av1_key)
    if build_dav1d; then
        # FFmpeg looks for dav1d only in the staged prefix, never in the system.
        PKG_CONFIG_LIBDIR="$stage$prefix/lib/pkgconfig"
        PKG_CONFIG_SYSROOT_DIR="$stage"
        export PKG_CONFIG_LIBDIR PKG_CONFIG_SYSROOT_DIR
    else
        without_av1 "$av1_why"
    fi
else
    say "building without AV1: $av1_missing"
fi

src="$work/ffmpeg-$FFMPEG_VERSION"

# Run in build_ffmpeg's subshell: writes why the FFmpeg build failed ($1)
# to ffmpeg-why in the build tree for build_ffmpeg, keeps the log $2 there
# as ffmpeg-failed.log and shows the end of it.
ffmpeg_failed() {
    printf '%s\n' "$1" >"$work/ffmpeg-why"
    cp "$2" "$work/ffmpeg-failed.log" 2>/dev/null || true
    tail -n 20 "$2" >&2
}

# Configures, builds and installs FFmpeg into the stage with the current
# choices, from a fresh copy of its source. Returns 1 on failure, with the
# reason in ffmpeg_why.
build_ffmpeg() {
    flags=$(configure_flags)
    say "building FFmpeg $FFMPEG_VERSION with $jobs jobs into $prefix"
    rm -f "$work/ffmpeg-why"
    if ! { rm -rf "$src" && tar -xJf "$cache_root/downloads/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$work"; }; then
        ffmpeg_why="cannot unpack ffmpeg-$FFMPEG_VERSION.tar.xz"
        return 1
    fi
    if (
        cd "$src" || exit 1
        # shellcheck disable=SC2086 # the flags are words without spaces
        if ! $nice ./configure --prefix="$prefix" $flags "$nasm_flag" >"$work/configure.log" 2>&1; then
            # config.log has the details, when configure got as far.
            log=ffbuild/config.log
            [ -f "$log" ] || log="$work/configure.log"
            ffmpeg_failed "FFmpeg's configure failed" "$log"
            exit 1
        fi
        # configure skips a misspelt component quietly (and leaves out one
        # whose library it cannot use); every listed one must be on.
        for flag in $flags; do
            case "$flag" in
                --enable-decoder=* | --enable-demuxer=* | --enable-parser=* | --enable-protocol=*)
                    kind=${flag#--enable-}
                    kind=${kind%%=*}
                    for name in $(printf '%s' "${flag#*=}" | tr ',' ' '); do
                        macro=$(printf 'CONFIG_%s_%s 1' "$name" "$kind" | tr '[:lower:]' '[:upper:]')
                        grep -q "^#define $macro\$" config_components.h ||
                            { ffmpeg_failed "FFmpeg's configure left out the $kind '$name'" ffbuild/config.log; exit 1; }
                    done
                    ;;
            esac
        done
        # shellcheck disable=SC2086 # empty when make joins a job server
        $nice make $make_jobs >"$work/make.log" 2>&1 ||
            { ffmpeg_failed "building FFmpeg failed" "$work/make.log"; exit 1; }
        make install DESTDIR="$stage" >"$work/install.log" 2>&1 ||
            { ffmpeg_failed "installing FFmpeg failed" "$work/install.log"; exit 1; }
    ); then
        return 0
    fi
    ffmpeg_why=$(cat "$work/ffmpeg-why" 2>/dev/null || echo "the FFmpeg build failed")
    return 1
}

fetch "$FFMPEG_URL" "$FFMPEG_SHA256"
if ! build_ffmpeg; then
    [ "$av1" = yes ] || fail "$ffmpeg_why (logs in $work)"
    # With AV1 the failure may be dav1d's (its pkg-config file, or FFmpeg's
    # libdav1d decoder), so FFmpeg is built once more without it. When that
    # fails too, the failure was not AV1's, and no record of it is kept.
    cp "$work/ffmpeg-failed.log" "$av1_log" 2>/dev/null || true
    without_av1 "$ffmpeg_why with AV1 (log in $av1_log)"
    build_ffmpeg || {
        rm -f "$av1_record" "$av1_log"
        fail "$ffmpeg_why, with AV1 and without (logs in $work)"
    }
fi

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
if [ "$replace" = yes ] && [ -e "$prefix" ]; then
    # The build without AV1 in FFMPEG_PREFIX goes only now that this one is
    # ready.
    mv "$prefix" "$work/replaced" || fail "cannot move the build in $prefix aside"
    # Moving across file systems copies, which can fail partway (a full
    # disk): the build without AV1 then goes back.
    if ! mv "$stage$prefix" "$prefix"; then
        rm -rf "$prefix"
        mv "$work/replaced" "$prefix" || fail "cannot move the new build into $prefix, nor the build without AV1 back (it is in $work/replaced)"
        fail "cannot move the new build into $prefix; the build without AV1 is back"
    fi
else
    mv "$stage$prefix" "$prefix"
fi
if [ "$av1" = yes ]; then rm -f "$av1_record" "$av1_log"; fi
rm -rf "$work"
signal rebuilt
say "FFmpeg $FFMPEG_VERSION is built in $prefix, with $(extras)"
