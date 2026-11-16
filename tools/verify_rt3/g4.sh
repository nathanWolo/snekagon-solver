#!/bin/sh
W=<scratchpad>/wt/y
S=$W/work/sh2.exe
O=$W/work/g4
cd $W
D=$W/work/g1/dump_s.txt
N=$(wc -l < $D)
i=0
while [ $i -lt $N ]; do
  j=$((i + 500))
  SHADOW_LAYERS=0 sh tools/shadow_rt3.sh $S $O rtcheck-$i rtcheck $D --from $i --to $j
  i=$j
done
echo "== group done $(date +%T)" >> $O/progress.txt
