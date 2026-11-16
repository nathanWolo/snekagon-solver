F="py tools/order/fit.py"
P=work/order/p2
$F work/order/sel_on_7.txt.meta "$(ls $P/on7_*.csv | tr '\n' ' ')" --mlp --board > work/order/fit_on7_mlp.txt 2>&1 &
$F work/order/sel_on_5.txt.meta "$(ls $P/on5_*.csv | tr '\n' ' ')" --mlp --board > work/order/fit_on5.txt 2>&1 &
$F work/order/sel_off_7.txt.meta "$(ls $P/off7_*.csv | tr '\n' ' ')" --mlp > work/order/fit_off7.txt 2>&1 &
$F work/order/sel_off_5.txt.meta "$(ls $P/off5_*.csv | tr '\n' ' ')" --mlp > work/order/fit_off5.txt 2>&1 &
wait
date
