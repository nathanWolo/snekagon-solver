# phase 1: dumps (RT on, off) on roots 0-23 of kids_all, and red-node counters on usable bench 0-87
E=work/target/release/snek.exe
D=work/order/p1
mkdir -p $D
for rt in on off; do
  if [ $rt = on ]; then X="--reach-threat 0"; else X=""; fi
  for k in 0 1 2 3 4 5 6 7 8 9 10 11; do
    a=$((k*2)); b=$((a+2))
    $E bench work/order/kids_all.txt --from $a --to $b --window 0,1 $X --dump $D/dump_${rt}_$k.txt --dump-every 10 > $D/rootbench_${rt}_$k.out &
  done
  wait
  for k in 0 1 2 3 4 5 6 7 8 9 10 11; do
    a=$((k*5)); b=$((a+5))
    $E bench work/order/bench_usable.txt --from $a --to $b --window 0,1 $X > $D/bench_${rt}_$k.out &
  done
  wait
done
$E bench work/order/kids_all.txt --from 0 --to 2 --window 0,1 > $D/ident_off_0.out
date
