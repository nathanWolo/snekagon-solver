#!/bin/sh
# Builds the shadow-check build (--features shadow) the way tools/build.sh builds a plain one: the MSVC
# toolchain in a short target directory (the linker fails on long paths). Run from the checkout's root.
#
#     sh tools/build_shadow.sh OUT.exe
set -e
TC=stable-x86_64-pc-windows-msvc
OUT=$1
[ -n "$OUT" ] || { echo "usage: sh tools/build_shadow.sh OUT.exe" >&2; exit 2; }
HERE="$(pwd)"
B="${SNEK_BUILD_DIR:-$(cygpath -m "${TEMP:-/tmp}")/snekb/$(printf %s "$HERE" | cksum | cut -d' ' -f1)}"
mkdir -p "$B"
rm -f "$B/shadow/release/snek.exe"
RUSTFLAGS="-Ctarget-cpu=native" cargo +$TC build --release --features shadow --target-dir "$B/shadow" 2>&1 | grep -E "^(error|warning)|LNK|Finished|-->" | cut -c1-300
[ -f "$B/shadow/release/snek.exe" ] || { echo "the build failed" >&2; exit 1; }
cp "$B/shadow/release/snek.exe" "$OUT"
sha256sum "$OUT" | cut -c1-12
