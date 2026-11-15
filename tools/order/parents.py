"""Splits the 74 parents of the proof benchmark for the learned-order study (climb/order).

    python tools/order/parents.py RESULTS_FILE SNEK_EXE

Reads the parents as tools/mkproof.py does (sorted), keeps the last 24 out of everything, splits
the first 50 by parent into train (38) and test (12) with a pinned seed, and writes:
  work/parents.txt        the 50 usable parents (with split) and the 24 that must not be looked at
  work/order/kids_train.txt, kids_test.txt   "? <position>" children of usable parents that are
                          NOT in tests/proof4/positions.txt, shuffled (pinned seed)
  work/order/bench_map.txt  benchmark index (0-87 only) -> parent index and split, for usable parents
The children of the 24 held-out parents are listed by `snek children` only to recognise and
drop them; nothing about them is written except their parent strings in parents.txt.
"""
import os
import random
import subprocess
import sys

results, exe = sys.argv[1], sys.argv[2]
parents = []
for line in open(results):
    f = line.split()
    if len(f) < 6:
        continue
    pos = f[0].split("_")
    if len(pos) == 7 and pos[5] == "3" and pos[4] == "1" and int(f[2]) <= -1:
        parents.append(f[0])
parents = sorted(set(parents))
assert len(parents) == 74, len(parents)
usable, held = parents[:50], parents[50:]
rng = random.Random(20261115)
idx = list(range(50))
rng.shuffle(idx)
test = set(idx[:12])
os.makedirs("work/order", exist_ok=True)
with open("work/parents.txt", "w", newline="\n") as fh:
    fh.write("# learned-order study (climb/order): parents of tests/proof4 in tools/mkproof.py order\n")
    fh.write("# usable: index split parent; held out (never looked at): index HELDOUT parent\n")
    for i, p in enumerate(usable):
        fh.write(f"{i} {'test' if i in test else 'train'} {p}\n")
    for i, p in enumerate(held):
        fh.write(f"{50 + i} HELDOUT {p}\n")
tmp = "work/order/usable_parents.txt"
with open(tmp, "w", newline="\n") as fh:
    for p in usable:
        fh.write(f"-1 {p}\n")
listing = subprocess.run([exe, "children", tmp], capture_output=True, text=True, check=True).stdout
kids = {}
for line in listing.splitlines():
    i, text = line.split()
    kids.setdefault(text, set()).add(int(i))
bench = [l.split()[1] for l in open("tests/proof4/positions.txt") if l.strip()]
inbench = set(bench)
with open("work/order/bench_map.txt", "w", newline="\n") as fh:
    for j, b in enumerate(bench[:88]):
        if b in kids:
            ps = sorted(kids[b])
            splits = {("test" if p in test else "train") for p in ps}
            fh.write(f"{j} {ps[0]} {'/'.join(sorted(splits))}\n")
for split in ("train", "test"):
    out = [k for k, ps in kids.items() if k not in inbench and all((p in test) == (split == "test") for p in ps)]
    out.sort()
    random.Random(7).shuffle(out)
    with open(f"work/order/kids_{split}.txt", "w", newline="\n") as fh, open(f"work/order/kids_{split}_parent.txt", "w", newline="\n") as fp:
        for k in out:
            fh.write(f"? {k}\n")
            fp.write(f"{min(kids[k])}\n")
    print(split, len(out))
print("bench 0-87 usable:", sum(1 for _ in open("work/order/bench_map.txt")))
