#!/bin/sh
# The parallel-search shadow runs and the value check below the root (rtcheck), each under the run
# lock and at most 9 minutes. Usage: sh tools/shadow_rt_more.sh SHADOW.exe OUTDIR DUMP
S="$1"
OUT="$2"
DUMP="$3"
MAIN="<session-folder>/snekagon"
SET="$MAIN/tests/set1"
P=tests/proof4/positions.txt
mkdir -p "$OUT"
run() {
  name="$1"; shift
  echo "== $name start $(date +%T)" >> "$OUT/progress.txt"
  sh tools/locked.sh timeout 540 "$S" "$@" > "$OUT/$name.txt" 2>&1
  echo "== $name exit $? $(date +%T)" >> "$OUT/progress.txt"
}
ALL="--reach-threat-max 14 --reach-threat-start 14"
run par-default-ply06 bench "$SET/ply06.txt" --to 200 --threads 8 --quiet
run par-all-ply06 bench "$SET/ply06.txt" --to 200 --threads 8 --quiet $ALL
run par-default-proof bench $P --to 8 --threads 8 --quiet
run par-all-proof bench $P --to 4 --threads 8 --quiet $ALL
N=$(wc -l < "$DUMP")
i=0
while [ $i -lt $N ]; do
  j=$((i + 400))
  run "rtcheck-$i" rtcheck "$DUMP" --from $i --to $j
  i=$j
done
echo "== more done $(date +%T)" >> "$OUT/progress.txt"
