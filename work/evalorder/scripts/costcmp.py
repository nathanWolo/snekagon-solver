"""Compare the #C cost lines of bench --cost-stats outputs: python costcmp.py BASE.txt OTHER.txt [...]
Per side and level: cut nodes, nodes under the winning move, nodes under failed moves before the cut,
and the share of cuts on the first try. Nodes are nested (a level's counts include the levels below)."""
import ast, sys

def load(p):
    d = {}
    tot = None
    for l in open(p):
        if l.startswith("# positions"):
            f = l.split(); tot = int(f[f.index("nodes") + 1])
        if not l.startswith("#C ") or "|" not in l or l.startswith("#C side"):
            continue
        parts = l[3:].split("|")
        side, lev = map(int, parts[0].split())
        cuts, win, fail = map(int, parts[1].split())
        idx = ast.literal_eval(parts[3].strip())
        d[(side, lev)] = (cuts, win, fail, idx[0][0])
    return d, tot

runs = [(p, *load(p)) for p in sys.argv[1:]]
print("total nodes:", "  ".join(f"{p.split('/')[-1]} {t:,}" for p, _, t in runs))
keys = sorted(set(k for _, d, _ in runs for k in d), key=lambda k: (k[0], -k[1]))
print(f"{'side lev':9s}" + "".join(f" | {p.split('/')[-1][:14]:>14s} cuts first% win fail" for p, _, _ in runs))
for k in keys:
    if k[1] < 6:
        continue
    row = f"{k[0]:4d} {k[1]:4d}"
    for _, d, t in runs:
        c, w, f, f0 = d.get(k, (0, 0, 0, 0))
        row += f" | {c:9d} {100*f0/max(c,1):5.1f}% {w/1e6:8.2f}M {f/1e6:8.2f}M"
    print(row)
