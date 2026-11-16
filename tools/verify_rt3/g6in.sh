#!/bin/sh
# verify 100 with the shadow build (every reach test in start3's domain also goes through the
# differential), without and with planted faults; and verify 100 of the candidate itself.
W=<scratchpad>/wt/y
C=<scratchpad>/wt/y3c/work/cand.exe
S=$W/work/sh2.exe
O=$W/work/g6
mkdir -p $O
r() {
  n=$1; t=$2; shift 2
  echo "== $n start $(date +%T)" >> $O/progress.txt
  timeout $t "$@" > $O/$n.txt 2>&1
  echo "== $n exit $? $(date +%T)" >> $O/progress.txt
}
r cand-verify 240 $C verify 100
r sh-verify 240 $S verify 100
for f in 1 2 3; do
  SHADOW_FAULT=$f SHADOW_LAYERS=0 r f$f-verify 120 $S verify 100
done
echo "== g6 done $(date +%T)" >> $O/progress.txt
