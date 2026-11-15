"""How much of a proof's work is the same position reached twice, up to the board's symmetries?

    python tools/sharepot.py work/long/ply1_results_paused.txt

Takes the positions three moves in that the results file holds as lost for blue (after 1:60, a
blue reply and red's winning answer), lists every blue move from each (`snek children`), and
counts the positions four moves in: all of them, the distinct ones, and the distinct ones when a
position and its eleven mirror images count as one. Each of these positions is a search of
about 4 thread-seconds in a proof, so the share that repeats is work a table keyed by the
canonical position would save.
"""
import os
import subprocess
import sys


def symmetry(t, c):
    q, r = (c[1], c[0]) if t >= 6 else c
    for _ in range(t % 6):
        q, r = -r, q + r
    return (q, r)


CELLS = [(q, r) for r in range(-5, 6) for q in range(-5, 6)]
INDEX = {c: (c[1] + 5) * 11 + (c[0] + 5) for c in CELLS}
MAPS = []
for t in range(12):
    m = {}
    for c in CELLS:
        if abs(c[0]) <= 5 and abs(c[1]) <= 5 and abs(c[0] + c[1]) <= 5:
            m[INDEX[c]] = INDEX[symmetry(t, c)]
    MAPS.append(m)


def transform(bits, m):
    out = 0
    while bits:
        low = bits & -bits
        out |= 1 << m[low.bit_length() - 1]
        bits ^= low
    return out


def canonical(text):
    f = text.split("_")
    red, blue = int(f[0], 16), int(f[1], 16)
    best = min((transform(red, m), transform(blue, m)) for m in MAPS)
    return (best, tuple(f[2:]))


def main():
    parents = []
    for line in open(sys.argv[1]):
        f = line.split()
        if len(f) < 6:
            continue
        pos = f[0].split("_")
        if len(pos) == 7 and pos[5] == "3" and pos[4] == "1" and int(f[2]) <= -1:
            parents.append(f[0])
    parents = sorted(set(parents))
    tmp = os.path.join("work", "sharepot_parents.txt")
    with open(tmp, "w", newline="\n") as fh:
        for p in parents:
            fh.write(f"-1 {p}\n")
    out = subprocess.run([os.path.join("target", "release", "snek.exe"), "children", tmp], capture_output=True, text=True, check=True).stdout
    kids = [line.split() for line in out.splitlines()]
    total = len(kids)
    exact = len({k[1] for k in kids})
    canon_all = len({canonical(k[1]) for k in kids})
    within = 0
    by_parent = {}
    for i, text in kids:
        by_parent.setdefault(i, set()).add(canonical(text))
    within = sum(len(v) for v in by_parent.values())
    print(f"{len(parents)} parents, {total} positions four moves in")
    print(f"  distinct as they stand:                      {exact:6d}  ({100 * (1 - exact / total):.1f}% repeat)")
    print(f"  distinct up to symmetry, within each parent: {within:6d}  ({100 * (1 - within / total):.1f}% repeat)")
    print(f"  distinct up to symmetry, over all parents:   {canon_all:6d}  ({100 * (1 - canon_all / total):.1f}% repeat)")


main()
