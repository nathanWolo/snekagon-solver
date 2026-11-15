#!/bin/sh
# Correctness of reach threats on the labelled sets (every value checked by bench).
E=work/study.exe
S=<session-folder>/snekagon/tests/set1
for f in ply11 ply10 ply09 ply08; do
  echo "== $f all, reach threats 0"
  $E bench $S/$f.txt --quiet --reach-threat 0
done
echo "== ply07 0-589, off then on"
$E bench $S/ply07.txt --from 0 --to 590 --quiet
$E bench $S/ply07.txt --from 0 --to 590 --quiet --reach-threat 0
echo "== ply06 0-199, off then on"
$E bench $S/ply06.txt --from 0 --to 200 --quiet
$E bench $S/ply06.txt --from 0 --to 200 --quiet --reach-threat 0
