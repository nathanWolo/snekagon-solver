#!/bin/sh
# Reach threats on a band of levels, proof 0-11, then the first study half.
E=work/study4.exe
P=tests/proof4/positions.txt
for args in "--reach-threat 0 --reach-threat-touch" "--reach-threat 0 --reach-threat-max 4" "--reach-threat 0 --reach-threat-max 5" "--reach-threat 3 --reach-threat-max 4 --reach-threat-work 100"; do
  echo "== $args"
  $E bench $P --from 0 --to 12 --window 0,1 --quiet --profile $args | grep -E "^# pos|reach-threat"
done
