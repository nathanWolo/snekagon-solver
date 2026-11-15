"""Sums the red-node counters (`#r` lines of `snek bench`) over bench outputs (climb/order).

    python tools/order/pot.py OUT1 [OUT2 ...]

Per pieces left: red nodes that reach the move loop, how many cut, how many cut on the first
move, nodes under failed moves at the cut nodes (the pot at that level; nested levels overlap:
a level-5 pot inside a failed level-7 move is counted at both), and the share of all nodes.
"""
import sys

tot = 0
pos = 0
secs = 0.0
r = {}
for path in sys.argv[1:]:
    for line in open(path):
        if line.startswith("# positions"):
            f = line.split()
            pos += int(f[2])
            tot += int(f[4])
            secs += float(f[6])
        elif line.startswith("#r"):
            f = line.split()
            L = int(f[1])
            v = [int(x) for x in f[2:]]
            r[L] = [a + b for a, b in zip(r.get(L, [0] * 5), v)]
print(f"positions {pos} nodes {tot} seconds {secs:.1f} ({tot / max(secs, 1e-9) / 1e6:.2f} M nodes/s)")
print("left  red-loop-nodes  per-position   cut   cut-first   waste-at-cut  share-of-all  win-subtrees")
for L in sorted(r):
    n, c, c1, w, ws = r[L]
    if n == 0:
        continue
    print(f"{L:4d} {n:14d} {n / pos:12.0f} {c / n:6.1%} {c1 / max(c, 1):9.1%} {w:14d} {w / tot:12.1%} {ws / tot:12.1%}")
