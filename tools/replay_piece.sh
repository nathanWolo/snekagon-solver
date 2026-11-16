#!/bin/sh
# One piece of a replay (called by tools/replay.sh under the run lock): sh tools/replay_piece.
[ -e "$3/STOP" ] && exit 0
EXE=$1; FILE=$2; DIR=$3; N=$4; shift 4
T0=$(date +%s)
timeout 900 "$EXE" prove "$FILE" --replay --deadline 600 --threads 16 --job-threads 1 --depth 2 --tt 27 --upper-first --out "$DIR/results.txt" "$@" 2> "$DIR/piece-$N.err" |
  while IFS= read -r l; do
    echo "$l @$(( $(date +%s) - T0 ))"
  done > "$DIR/piece-$N.out"
echo "piece $N ended after $(( $(date +%s) - T0 )) s" >> "$DIR/pieces.txt"
