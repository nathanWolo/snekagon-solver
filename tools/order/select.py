"""Picks dumped red nodes for labelling (climb/order).

    python tools/order/select.py OUT.txt T SEED LEVELS DUMP1.txt [DUMP2.txt ...]

Keeps nodes that cut in the tree (red found a win there), at the given levels (e.g. 5,7). A node
whose subtree in the tree had `total` nodes is kept with probability min(1, total / T) and gets the
weight 1 / that probability, so weighted sums estimate the sums over all dumped cut nodes (the
pot is in node counts, which a few big nodes dominate). Writes OUT.txt ("? position") and
OUT.txt.meta (the dump's meta line plus the weight).
"""
import random
import sys

out, T, seed, levels = sys.argv[1], float(sys.argv[2]), int(sys.argv[3]), {int(x) for x in sys.argv[4].split(",")}
rng = random.Random(seed)
kept = 0
seen = 0
with open(out, "w", newline="\n") as fo, open(out + ".meta", "w", newline="\n") as fm:
    for d in sys.argv[5:]:
        for p, m in zip(open(d), open(d + ".meta")):
            f = m.split()
            level, cut, total = int(f[1]), int(f[7]), int(f[9])
            if level not in levels or cut != 1:
                continue
            seen += 1
            q = min(1.0, total / T)
            if rng.random() < q:
                kept += 1
                fo.write(p)
                fm.write(m.strip() + f" {1 / q:.6f}\n")
print(f"cut nodes {seen}, kept {kept}")
