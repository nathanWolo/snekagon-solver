Learned-order sizing study (climb/order), 2026-11-15. Big dumps and label CSVs stay in work/order (not committed).
parents.txt: the 50 usable parents (train/test split) and the 24 held out (never dumped, labelled or fitted).
pot_*.txt: tools/order/pot.py on bench outputs (red nodes reaching the move loop, by pieces left; waste = nodes
  under failed moves at red cut nodes, inclusive of nested levels).
fit_rt{on,off}_{5,7,9}.txt: tools/order/fit.py; nodes from kids of usable parents not in tests/proof4,
  dumped every 10th red node at that level (roots 0-23 of work/order/kids_all.txt; level 9: roots 24-35, every node),
  cut nodes only, kept with probability min(1, total/T) and weighted back (T = 1000 rt on, 3000 rt off, 2000 at 9).
Labels: snek ordermoves, every move solved alone with a fresh table, window "does it win", 5M node cap (never hit).
Phase scripts: p1.sh (dumps, counters), labeljob.py output (p2), p3.sh, fits.sh.
