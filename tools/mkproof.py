"""Builds the proof benchmark: positions four moves in that the proof of the centre opening needs.

    python tools/mkproof.py work/long/ply1_results.txt tests/proof4 [--per-parent 8] [--seed 1]

The results file of `snek prove` holds positions after `1:60`, a blue reply and red's winning
answer that were searched to a loss for blue (blue to move). Every blue move from such a
position leaves a win for red, and a proof has to show each of them. This samples a fixed
number of those children from every parent, in a pinned shuffled order, labelled 1 (the side
to move, red, wins): the label follows from the parent's value, and every solve checks it.

Writes OUT/positions.txt ("1 <position>") and OUT/MANIFEST.txt.
"""
import hashlib
import os
import random
import subprocess
import sys


def main():
    results, out = sys.argv[1], sys.argv[2]
    per_parent = int(sys.argv[sys.argv.index("--per-parent") + 1]) if "--per-parent" in sys.argv else 8
    seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
    parents = []
    for line in open(results):
        f = line.split()
        if len(f) < 6:
            continue
        pos = f[0].split("_")
        if len(pos) == 7 and pos[5] == "3" and pos[4] == "1" and int(f[2]) <= -1:
            parents.append(f[0])
    parents = sorted(set(parents))
    os.makedirs(out, exist_ok=True)
    tmp = os.path.join(out, "parents.txt")
    with open(tmp, "w", newline="\n") as fh:
        for p in parents:
            fh.write(f"-1 {p}\n")
    exe = os.path.join("target", "release", "snek.exe")
    listing = subprocess.run([exe, "children", tmp], capture_output=True, text=True, check=True).stdout
    kids = {}
    for line in listing.splitlines():
        i, text = line.split()
        kids.setdefault(int(i), []).append(text)
    rng = random.Random(seed)
    chosen = []
    for i in range(len(parents)):
        ks = sorted(kids.get(i, []))
        chosen += [(i, k) for k in rng.sample(ks, min(per_parent, len(ks)))]
    rng.shuffle(chosen)
    body = "".join(f"1 {k}\n" for i, k in chosen)
    with open(os.path.join(out, "positions.txt"), "w", newline="\n") as fh:
        fh.write(body)
    digest = hashlib.sha256(body.encode()).hexdigest()
    with open(os.path.join(out, "MANIFEST.txt"), "w", newline="\n") as fh:
        fh.write(f"positions.txt {len(chosen)} positions sha256 {digest}\n")
        fh.write(f"  from {len(parents)} parents (lost for blue, three moves in) of {results}; "
                 f"{sum(len(v) for v in kids.values())} children in all; {per_parent} a parent, seed {seed}\n")
        fh.write("  label 1: red to move wins (each is a child of a position searched to a loss for blue)\n")
    print(open(os.path.join(out, "MANIFEST.txt")).read())


main()
