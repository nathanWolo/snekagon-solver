# phase 3: red nodes with 9 pieces left (reach threats on), roots 24-35 of kids_all, then label them
E=work/target/release/snek.exe
D=work/order/p3
mkdir -p $D
for k in 0 1 2 3 4 5 6 7 8 9 10 11; do
  a=$((24+k)); b=$((a+1))
  $E bench work/order/kids_all.txt --from $a --to $b --window 0,1 --reach-threat 0 --dump $D/dump9_$k.txt --dump-levels 9 > $D/rootbench9_$k.out &
done
wait
py tools/order/select.py work/order/sel_on_9.txt 2000 9 9 $(ls $D/dump9_*.txt)
py tools/order/labeljob.py work/p3b.sh 420 12 5000000 "work/order/sel_on_9.txt:work/order/p3/on9:--reach-threat 0"
sh work/p3b.sh
date
