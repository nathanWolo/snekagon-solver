#!/bin/sh
# E34 checks, second batch: sh work/checks2.sh EXE OUTDIR
EXE=$1; OUT=$2
M="$(cd "$(git rev-parse --git-common-dir)/.." && pwd)"
echo "base plain, ply07 --to 589: $("$M/frozen/climb-base-plain.exe" bench "$M/tests/set1/ply07.txt" --from 0 --to 589 --quiet | tail -1)"
echo "base plain, ply07 --to 590: $("$M/frozen/climb-base-plain.exe" bench "$M/tests/set1/ply07.txt" --from 0 --to 590 --quiet | tail -1)"
echo "hold exe,   ply07 --to 590: $("$EXE" bench "$M/tests/set1/ply07.txt" --from 0 --to 590 --quiet | tail -1)"
F="$M/tests/set1/bench_ply5_ublock_refutations.txt"
"$EXE" prove "$F" --threads 16 --depth 1 --hold 0.001 > "$OUT/d1v.txt" 2> "$OUT/d1v.err"
echo "depth 1 hold 0.001: exit $? $(grep '^# positions' "$OUT/d1v.txt"); held-out jobs: $(grep '^# hold' "$OUT/d1v.err" | awk '{s+=$4} END {print s}')"
"$EXE" prove "$F" --threads 16 --depth 1 > "$OUT/d1off.txt" 2> /dev/null
echo "depth 1 hold off:   exit $? $(grep '^# positions' "$OUT/d1off.txt")"
"$EXE" prove "$F" --threads 16 --depth 2 --hold 0.001 > "$OUT/d2v.txt" 2> "$OUT/d2v.err"
echo "depth 2 hold 0.001: exit $? $(grep '^# positions' "$OUT/d2v.txt"); held-out jobs: $(grep '^# hold' "$OUT/d2v.err" | awk '{s+=$4} END {print s}')"
"$EXE" prove "$F" --threads 16 --depth 2 > "$OUT/d2off.txt" 2> /dev/null
echo "depth 2 hold off:   exit $? $(grep '^# positions' "$OUT/d2off.txt")"
R="$OUT/resume2_results.txt"
rm -f "$R"
timeout 3 "$EXE" prove "$F" --threads 16 --depth 2 --hold 0.001 --out "$R" > "$OUT/resume2-a.txt" 2> "$OUT/resume2-a.err"
echo "cut run exit $? ($(grep -c . "$R") lines in the results file, $(grep -c '^[0-9]' "$OUT/resume2-a.txt") positions done)"
"$EXE" prove "$F" --threads 16 --depth 2 --hold 0.001 --out "$R" > "$OUT/resume2-b.txt" 2> "$OUT/resume2-b.err"
echo "resumed run exit $? $(grep '^# positions' "$OUT/resume2-b.txt") $(grep 'positions read' "$OUT/resume2-b.err" | cut -c1-30)"
