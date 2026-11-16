#!/bin/sh
# Shadow runs of the new defaults (reach threats at every level, start3), each command under the
# run lock and at most 14 minutes. Run from the worktree root.
#     sh tools/shadow_rt3.sh SHADOW.exe OUTDIR NAME [bench args...]
# Environment: SHADOW_* (see src/shadow.rs).
S="$1"; OUT="$2"; NAME="$3"; shift 3
MAIN="$(cd "$(git rev-parse --git-common-dir)/.." && pwd)"
mkdir -p "$OUT"
echo "== $NAME start $(date +%T) $*" >> "$OUT/progress.txt"
sh "$MAIN/tools/locked.sh" timeout 840 "$S" "$@" > "$OUT/$NAME.txt" 2>&1
echo "== $NAME exit $? $(date +%T)" >> "$OUT/progress.txt"
