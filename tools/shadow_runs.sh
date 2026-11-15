#!/bin/sh
# Shadow-check runs, each under the run lock, each at most about 9.5 minutes.
WT="<scratchpad>/wt/v"
MAIN="<session-folder>/snekagon"
OUT="<scratchpad>/logs"
mkdir -p "$OUT"
cd "$WT" || exit 1
S=./work/target/release/snek.exe
run() {
  name="$1"; shift
  echo "== $name start $(date +%T)" >> "$OUT/progress.txt"
  sh tools/locked.sh timeout 570 "$S" "$@" > "$OUT/$name.txt" 2>&1
  echo "== $name exit $? $(date +%T)" >> "$OUT/progress.txt"
}
run verify200 verify 200
run proof0 bench tests/proof4/positions.txt --from 1 --to 4 --window 0,1 --quiet
run proof1 bench tests/proof4/positions.txt --from 4 --to 8 --window 0,1 --quiet
run proof2 bench tests/proof4/positions.txt --from 8 --to 12 --window 0,1 --quiet
run ply06 bench "$MAIN/tests/set1/ply06.txt" --to 200 --quiet
run ply07a bench "$MAIN/tests/set1/ply07.txt" --to 300 --quiet
run ply07b bench "$MAIN/tests/set1/ply07.txt" --from 300 --to 590 --quiet
run other bench "$MAIN/tests/set1/bench_ply5_other_openings.txt" --quiet
run ply08 bench "$MAIN/tests/set1/ply08.txt" --quiet
run par8 bench "$MAIN/tests/set1/ply06.txt" --to 200 --threads 8 --quiet
echo "== all done $(date +%T)" >> "$OUT/progress.txt"
