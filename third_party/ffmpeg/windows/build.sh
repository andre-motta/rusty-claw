#!/bin/sh
# Builds the pinned FFmpeg for Windows with Microsoft's compiler (MSVC), as
# static .lib files that cargo links into one rusty-claw.exe.
#
# The version, the checksums and the list of decoders, demuxers and parsers
# are read from ../build.sh, the Linux build, so both build the same FFmpeg.
# Only the platform parts of the configure line change: MSVC instead of gcc,
# Windows threads instead of POSIX threads, and the DLL C runtime (-MD) that
# Rust's MSVC builds use.
#
# Run it from an MSYS2 shell opened inside the "x64 Native Tools Command
# Prompt for VS" (see ../README.md, "Windows 11"), so cl.exe and link.exe are
# on PATH:
#
#   sh third_party/ffmpeg/windows/build.sh            build, or reuse a finished build
#   sh third_party/ffmpeg/windows/build.sh --prefix   print the prefix as a Windows path
#   sh third_party/ffmpeg/windows/build.sh --configure  print the configure line
#
# The prefix is %LOCALAPPDATA%\rusty-claw\ffmpeg\<version>-<hash>-msvc, where
# the hash covers the pins and the configure line. FFMPEG_PREFIX (a Windows
# or MSYS2 path without spaces) overrides it. FFMPEG_JOBS sets the parallel
# compile jobs (default: the CPU count, at most 8).
set -eu

say() { printf 'ffmpeg: %s\n' "$*"; }
fail() { printf 'ffmpeg: %s\n' "$*" >&2; exit 1; }

here=$(cd "$(dirname "$0")" && pwd)
main="$here/../build.sh"
[ -f "$main" ] || fail "cannot find $main"

# The pins: the lines of ../build.sh from FFMPEG_VERSION= to the end of the
# CONFIGURE_FLAGS string. They are plain assignments and comments; anything
# else stops the build rather than run.
pins=$(awk '
    /^FFMPEG_VERSION=/ { on = 1 }
    on { print }
    on && /^CONFIGURE_FLAGS="/ { flags = 1 }
    flags && /"$/ { exit }
' "$main")
unexpected=$(printf '%s\n' "$pins" | grep -v -E '^(#.*|[A-Z0-9_]+=.*|--.*|)$' || true)
unexpected="$unexpected$(printf '%s\n' "$pins" | grep -v '^#' | grep -E '`|\$\(|[;&|<>]' || true)"
[ -z "$unexpected" ] || fail "unexpected lines among the pins in $main: $unexpected"
FFMPEG_VERSION='' FFMPEG_URL='' FFMPEG_SHA256='' NASM_VERSION='' NASM_URL='' NASM_SHA256='' CONFIGURE_FLAGS=''
eval "$pins"
for v in FFMPEG_VERSION FFMPEG_URL FFMPEG_SHA256 NASM_VERSION NASM_URL NASM_SHA256 CONFIGURE_FLAGS; do
    eval "[ -n \"\${$v}\" ]" || fail "$v is not set in $main"
done
case "$CONFIGURE_FLAGS" in
    *--disable-gpl*--disable-nonfree*--enable-decoder=*) ;;
    *) fail "the configure line in $main is not the expected LGPL allowlist" ;;
esac

# Windows threads replace POSIX threads; position-independent code is a
# Linux notion. Everything else, the allowlist included, stays as it is.
WINDOWS_FLAGS="--toolchain=msvc --target-os=win64 --arch=x86_64 --extra-cflags=-MD"
flags=
for flag in $CONFIGURE_FLAGS; do
    case "$flag" in
        --enable-pthreads) flags="$flags --enable-w32threads" ;;
        --enable-pic) ;;
        *) flags="$flags $flag" ;;
    esac
done
CONFIGURE_FLAGS_MSVC="${flags# } $WINDOWS_FLAGS"

sha256_of() { sha256sum "$1" | cut -d' ' -f1; }

stamp_text() {
    printf 'FFmpeg %s\n' "$FFMPEG_VERSION"
    printf 'source %s\n' "$FFMPEG_URL"
    printf 'sha256 %s\n' "$FFMPEG_SHA256"
    printf 'nasm %s sha256 %s\n' "$NASM_VERSION" "$NASM_SHA256"
    printf 'licence LGPL-2.1-or-later\n'
    printf 'configure %s\n' "$CONFIGURE_FLAGS_MSVC"
}

if [ -n "${LOCALAPPDATA:-}" ] && command -v cygpath >/dev/null 2>&1; then
    cache_root="$(cygpath -u "$LOCALAPPDATA")/rusty-claw/ffmpeg"
else
    cache_root="${XDG_CACHE_HOME:-$HOME/.cache}/rusty-claw/ffmpeg"
fi
config_hash=$(stamp_text | sha256sum | cut -c1-12)
if [ -n "${FFMPEG_PREFIX:-}" ]; then
    case "$FFMPEG_PREFIX" in
        /* | [A-Za-z]:[/\\]*) ;;
        *) fail "FFMPEG_PREFIX must be an absolute path, not '$FFMPEG_PREFIX'" ;;
    esac
    prefix=$FFMPEG_PREFIX
    if command -v cygpath >/dev/null 2>&1; then prefix=$(cygpath -u "$prefix"); fi
else
    prefix="$cache_root/$FFMPEG_VERSION-$config_hash-msvc"
fi
stamp="$prefix/share/rusty-claw/ffmpeg-build.txt"

windows_path() {
    if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s\n' "$1"; fi
}

case "${1:-}" in
    --prefix) windows_path "$prefix"; exit 0 ;;
    --configure) printf '%s\n' "$CONFIGURE_FLAGS_MSVC"; exit 0 ;;
    "") ;;
    *) fail "unknown option '$1' (expected --prefix, --configure or nothing)" ;;
esac

case "$prefix" in
    *" "*) fail "the prefix '$prefix' has a space, which FFmpeg's build cannot take; set FFMPEG_PREFIX to a folder without one, for example C:/rusty-claw/ffmpeg" ;;
esac

print_next() {
    win=$(windows_path "$prefix")
    say "FFmpeg $FFMPEG_VERSION for MSVC is in $win"
    say "point cargo at it, in the Developer prompt you build from:"
    say "  cmd:        set FFMPEG_PREFIX=$win"
    say "  PowerShell: \$env:FFMPEG_PREFIX = '$win'"
    say "  for good:   setx FFMPEG_PREFIX \"$win\"   (then open a new prompt)"
}

up_to_date() { [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$(stamp_text)" ]; }

if up_to_date; then
    print_next
    exit 0
fi
[ ! -e "$prefix" ] || fail "$prefix exists but holds another build; remove it or set FFMPEG_PREFIX elsewhere"

# The tools: MSVC from the Developer prompt, the rest from MSYS2.
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) ;;
    *) fail "run this in an MSYS2 shell on Windows (on Linux, use ../build.sh)" ;;
esac
command -v cl.exe >/dev/null 2>&1 ||
    fail "cl.exe is not on PATH: open MSYS2 from the x64 Native Tools Command Prompt with -use-full-path (see the README)"
[ -x "$(dirname "$(command -v cl.exe)")/link.exe" ] ||
    fail "MSVC's link.exe is not next to cl.exe; repair the Visual Studio Build Tools"
cl.exe 2>&1 | grep -q 'x64' ||
    fail "cl.exe does not build for x64: use the x64 Native Tools Command Prompt, not the x86 one"
for tool in make gcc curl tar xz sha256sum cmp; do
    command -v "$tool" >/dev/null 2>&1 ||
        fail "$tool is missing: in MSYS2, run pacman -S --needed make gcc diffutils tar xz curl"
done

mkdir -p "$cache_root/downloads" "$cache_root/build"
jobs="${FFMPEG_JOBS:-$(nproc 2>/dev/null || echo 4)}"
if [ -z "${FFMPEG_JOBS:-}" ] && [ "$jobs" -gt 8 ]; then jobs=8; fi

# fetch URL SHA256: downloads into the cache once and checks the checksum.
fetch() {
    dest="$cache_root/downloads/${1##*/}"
    if [ -f "$dest" ] && [ "$(sha256_of "$dest")" = "$2" ]; then
        return 0
    fi
    say "fetching $1"
    curl -fsSL --retry 2 -o "$dest.part" "$1" || { rm -f "$dest.part"; fail "download failed: $1"; }
    got=$(sha256_of "$dest.part")
    if [ "$got" != "$2" ]; then
        rm -f "$dest.part"
        fail "${1##*/} has SHA-256 $got, expected $2; nothing was built"
    fi
    mv "$dest.part" "$dest"
}

work="$cache_root/build/$FFMPEG_VERSION-$config_hash-msvc"
rm -rf "$work"
mkdir -p "$work"

# nasm, the assembler for FFmpeg's fast x86 paths, built from the pinned
# source with MSYS2's gcc (it runs inside the MSYS2 shell only).
nasm_dir="$cache_root/nasm-$NASM_VERSION"
if [ ! -x "$nasm_dir/bin/nasm.exe" ] && [ ! -x "$nasm_dir/bin/nasm" ]; then
    fetch "$NASM_URL" "$NASM_SHA256"
    say "building nasm $NASM_VERSION"
    tar -xJf "$cache_root/downloads/nasm-$NASM_VERSION.tar.xz" -C "$work"
    (
        cd "$work/nasm-$NASM_VERSION"
        CC=gcc ./configure >"$work/nasm-configure.log" 2>&1 ||
            { tail -n 20 "$work/nasm-configure.log" >&2; exit 1; }
        make -j"$jobs" nasm >"$work/nasm-make.log" 2>&1 ||
            { tail -n 20 "$work/nasm-make.log" >&2; exit 1; }
        mkdir -p "$nasm_dir/bin"
        if [ -f nasm.exe ]; then cp nasm.exe "$nasm_dir/bin/"; else cp nasm "$nasm_dir/bin/"; fi
    ) || fail "nasm build failed (logs in $work)"
fi
PATH="$nasm_dir/bin:$PATH"
export PATH

fetch "$FFMPEG_URL" "$FFMPEG_SHA256"
say "building FFmpeg $FFMPEG_VERSION for MSVC with $jobs jobs into $prefix"
tar -xJf "$cache_root/downloads/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$work"
src="$work/ffmpeg-$FFMPEG_VERSION"
stage="$work/stage"
(
    cd "$src"
    # shellcheck disable=SC2086 # the flags are words without spaces
    ./configure --prefix="$prefix" $CONFIGURE_FLAGS_MSVC --enable-x86asm >"$work/configure.log" 2>&1 ||
        { tail -n 20 ffbuild/config.log >&2; exit 1; }
    # configure skips a misspelt component quietly; every listed one must be on.
    for flag in $CONFIGURE_FLAGS_MSVC; do
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
    make -j"$jobs" >"$work/make.log" 2>&1 || { tail -n 30 "$work/make.log" >&2; exit 1; }
    make install DESTDIR="$stage" >"$work/install.log" 2>&1 || { tail -n 20 "$work/install.log" >&2; exit 1; }
) || fail "FFmpeg build failed (logs in $work)"

staged="$stage$prefix"
[ -f "$staged/lib/avcodec.lib" ] || fail "the build made no avcodec.lib (logs in $work)"

# MSVC names system libraries `bcrypt.lib`; the game's link step reads them
# from the pkg-config files as `-lbcrypt`, as on Linux, and would take an
# MSVC `-libpath:` for a library name, so those go.
for pc in "$staged"/lib/pkgconfig/*.pc; do
    sed -E -e '/^Libs(\.private)?:/ s/(^|[[:space:]])-libpath:[^[:space:]]*//g' \
        -e ':again' \
        -e '/^Libs(\.private)?:/ s/(^|[[:space:]])([A-Za-z0-9_]+)\.lib([[:space:]]|$)/\1-l\2\3/' \
        -e 't again' "$pc" >"$pc.new"
    mv "$pc.new" "$pc"
done

mkdir -p "$staged/share/licenses/ffmpeg" "$staged/share/rusty-claw"
cp "$src/LICENSE.md" "$src/COPYING.LGPLv2.1" "$staged/share/licenses/ffmpeg/"
stamp_text >"$staged/share/rusty-claw/ffmpeg-build.txt"

mkdir -p "$(dirname "$prefix")"
mv "$staged" "$prefix"
rm -rf "$work"
print_next
