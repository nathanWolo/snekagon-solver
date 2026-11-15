#!/bin/sh
# Reach threats asked only after a failed move (study2), proof 0-11; identity first.
E=work/study2.exe
P=tests/proof4/positions.txt
for args in "" "--reach-threat 0" "--reach-threat 0 --reach-threat-work 200" "--reach-threat 5" "--reach-threat 6"; do
  echo "== $args"
  $E bench $P --from 0 --to 12 --window 0,1 --quiet $args
done
