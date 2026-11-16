#!/bin/sh
# E34 checks for --hold (run one step under the lock each): sh work/checks.sh EXE OUTDIR STEP
# STEP: ply07 | d1 | d2 | d3 | resume
EXE=$1; OUT=$2; STEP=$3
M="$(cd "$(git rev-parse --git-common-dir)/.." && pwd)"
mkdir -p "$OUT"
case $STEP in
ply07)
  "$EXE" bench "$M/tests/set1/ply07.txt" --from 0 --to 589 --quiet | tail -1 ;;
d1|d2|d3)
  D=${STEP#d}
  for f in blue_wins blue_wins_holdout other_openings random_units ublock_refutations; do
    "$EXE" prove "$M/tests/set1/bench_ply5_$f.txt" --threads 16 --depth $D --hold 0.001 --quiet > "$OUT/$STEP-$f.txt" 2> "$OUT/$STEP-$f.err"
    echo "$STEP $f exit $? $(grep '^# positions' "$OUT/$STEP-$f.txt")"
  done ;;
resume)
  F="$M/tests/set1/bench_ply5_random_units.txt"
  R="$OUT/resume_results.txt"
  rm -f "$R"
  timeout 20 "$EXE" prove "$F" --threads 16 --depth 2 --hold 0.001 --out "$R" > "$OUT/resume-a.txt" 2> "$OUT/resume-a.err"
  echo "cut run exit $? ($(grep -c . "$R") lines in the results file, $(grep -c '^[0-9]' "$OUT/resume-a.txt") positions done)"
  "$EXE" prove "$F" --threads 16 --depth 2 --hold 0.001 --out "$R" > "$OUT/resume-b.txt" 2> "$OUT/resume-b.err"
  echo "resumed run exit $? $(grep '^# positions' "$OUT/resume-b.txt") $(grep 'positions read' "$OUT/resume-b.err")" ;;
esac
