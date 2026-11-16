#!/bin/sh
# Shadow-check runs of the reach-threat rule (build: sh tools/build_shadow.sh), each under the run
# lock and at most 9 minutes. Usage: sh tools/shadow_rt_runs.sh SHADOW.exe OUTDIR [GROUP]
S="$1"
OUT="$2"
GROUP="${3:-all}"
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
NG="--no-reach-threat-grow"
setting() { # setting NAME OPTS...
  n="$1"; shift
  run "$n-ply06" bench "$SET/ply06.txt" --to 200 --quiet "$@"
  run "$n-ply07" bench "$SET/ply07.txt" --to 590 --quiet "$@"
  run "$n-ply08a" bench "$SET/ply08.txt" --to 2000 --quiet "$@"
  run "$n-ply08b" bench "$SET/ply08.txt" --from 2000 --quiet "$@"
  run "$n-other" bench "$SET/bench_ply5_other_openings.txt" --quiet "$@"
  run "$n-proofA" bench $P --to 8 --window 0,1 --quiet "$@"
  run "$n-proofB" bench $P --from 8 --to 16 --window 0,1 --quiet "$@"
  run "$n-proofC" bench $P --from 16 --to 24 --window 0,1 --quiet "$@"
  run "$n-prooffull" bench $P --to 3 --quiet "$@"
}
case "$GROUP" in
  default) setting default ;;
  all) setting all $ALL ;;
  nogrow) setting nogrow $NG ;;
  allnogrow) setting allnogrow $ALL $NG ;;
  cache) setting cache $ALL --reach-threat-cache --reach-threat-after 0 ;;
esac
echo "== $GROUP done $(date +%T)" >> "$OUT/progress.txt"
