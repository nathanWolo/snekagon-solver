#!/bin/sh
# perpos.sh EXE OUTDIR NAME... : one process per position (fresh table), each ply-5 bench file named
# Extra options after "--" are passed to bench.
EXE=$1; OUT=$2; shift 2
MAIN="<session-folder>/snekagon"
NAMES=""
while [ $# -gt 0 ] && [ "$1" != "--" ]; do NAMES="$NAMES $1"; shift; done
[ "$1" = "--" ] && shift
mkdir -p "$OUT"
for n in $NAMES; do
  F="$MAIN/tests/set1/bench_ply5_$n.txt"
  [ -f "$F" ] || F="$n"
  N=$(grep -cv '^\s*$' "$F")
  B=$(basename "$n" .txt)
  : > "$OUT/$B.txt"
  i=0
  while [ $i -lt $N ]; do
    "$EXE" bench "$F" --from $i --to $((i+1)) "$@" | grep -v '^#' >> "$OUT/$B.txt"
    i=$((i+1))
  done
done
