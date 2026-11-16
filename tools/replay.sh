#!/bin/sh
# The proof-order replay: proves the roots of FILE in order, as the replies of one proof (`prove --replay`),
# under the run lock in pieces of at most about 15 minutes, resuming from the results file.
#
#     sh tools/replay.sh EXE FILE DIR [extra prove options]
#
# DIR (fresh for a new replay, the same to resume one) gets results.txt (the results file), piece-N.out
# (the per-root lines, each ending in the seconds since the piece began), pieces.txt and summary.txt
# (one line a root, with the certificate check). A piece starts no new root after 600 s (--deadline)
# and its solver is stopped after 900 s; a root cut off that way is proved again in the next piece
# from what the results file kept, and the seconds it lost are added to it (its lost nodes are not).
# A resumed piece starts without the earlier pieces' table and workers' history; the winners come
# back from the results file (a root decided by a known result adds its winning move to them).
# Defaults: 16 single-thread workers, the coordinator searching two plies (jobs at ply 4, as in E24),
# --upper-first (a red ply-2 node asks only "can red win", as in the real proof), table 2^27.
set -e
EXE=$1; FILE=$2; DIR=$3; shift 3
mkdir -p "$DIR"
N=$(grep -cv '^#' "$FILE")
piece=$(ls "$DIR"/piece-*.out 2>/dev/null | wc -l)
while :; do
  done_n=$(cat "$DIR"/piece-*.out 2>/dev/null | awk '$1 ~ /^[0-9]+$/ {print $1}' | sort -un | wc -l)
  [ "$done_n" -lt "$N" ] || break
  piece=$((piece + 1))
  [ $piece -le 40 ] || { echo "too many pieces" >&2; exit 1; }
  sh tools/locked.sh sh tools/replay_piece.sh "$EXE" "$FILE" "$DIR" $piece "$@"
done
python tools/replay_sum.py "$FILE" "$DIR" | tee "$DIR/summary.txt"
