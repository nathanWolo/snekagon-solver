#!/bin/sh
# Like tools/locked.sh, but never takes over a lock, however old (the owner's install holds it).
COMMON="$(cd "$(git rev-parse --git-common-dir)" && pwd)"
LOCK="$COMMON/run.lock"
while ! mkdir "$LOCK" 2>/dev/null; do
  sleep 5
done
trap 'rmdir "$LOCK" 2>/dev/null' EXIT INT TERM
"$@"
