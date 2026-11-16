"""Coordinate search of integer weights on the cost to the first win (the objective of tools/rootfit.py,
computed without a sort), for the evaluation terms of evalmoves CSVs.

python fit.py --train A.csv ... --test B.csv ... [--features f1,f2] [--start w1,w2,...] [--steps 16,4,1] [--max-pos N]

Cost of a node: the nodes of every move ordered ahead of the first winning move, plus that move's; order
= weighted score, highest first, ties by the static score (as the solver's buffer, which breaks ties by
move number; close enough). Reported: total and geometric mean over nodes, and the share where the first
move wins, for: the static score, the model weights given, the fitted weights, and the cheapest win.
"""
import csv, math, sys
from collections import defaultdict

args = sys.argv[1:]
def opt(name, default=None):
    return args[args.index(name) + 1] if name in args else default
def files(flag):
    if flag not in args:
        return []
    out = []
    for a in args[args.index(flag) + 1:]:
        if a.startswith("--"):
            break
        out.append(a)
    return out
feats = opt("--features", "est,ub,long,terr,hand,ends,short,fresh").split(",")
maxpos = int(opt("--max-pos", "1000000"))

def load(paths):
    by = defaultdict(list)
    for p in paths:
        for r in csv.DictReader(open(p)):
            if r["win"] not in ("0", "1"):
                continue
            by[(p, r["pos"])].append((tuple(float(r[f]) for f in feats), int(r["static"]), r["win"] == "1", int(r["nodes"])))
    nodes = [ms for ms in by.values() if any(m[2] for m in ms)]
    return nodes[:maxpos]

def cost_node(ms, w):
    best = None
    sc = []
    for f, st, win, n in ms:
        s = sum(a * b for a, b in zip(w, f))
        k = (s, st)
        sc.append(k)
        if win and (best is None or k > best[0]):
            best = (k, n)
    c = best[1]
    first = True
    for (k, (f, st, win, n)) in zip(sc, ms):
        if k > best[0]:
            c += n
            first = False
    return c, first

def cost(nodes, w):
    tot, lg, fw = 0, 0.0, 0
    for ms in nodes:
        c, f = cost_node(ms, w)
        tot += c; lg += math.log(c + 1); fw += f
    return tot, math.exp(lg / len(nodes)), fw / len(nodes)

def ideal(nodes):
    tot = sum(min(m[3] for m in ms if m[2]) for ms in nodes)
    return tot

tr, te = load(files("--train")), load(files("--test"))
w = [int(x) for x in opt("--start", ",".join(["64"] + ["0"] * (len(feats) - 1))).split(",")]
steps = [int(x) for x in opt("--steps", "32,8,2").split(",")]
best = cost(tr, w)[0]
improved = True
rounds = 0
while improved and rounds < int(opt("--rounds", "6")):
    improved = False
    rounds += 1
    for i in range(len(feats)):
        for st in steps:
            for sgn in (1, -1):
                t = list(w); t[i] += sgn * st
                c = cost(tr, t)[0]
                if c < best:
                    best, w, improved = c, t, True
    print("round", rounds, "train cost", best, "weights", ",".join(map(str, w)), flush=True)
zero = [0] * len(feats)
for name, nodes in (("train", tr), ("test", te)):
    if not nodes:
        continue
    print(f"{name}: {len(nodes)} nodes, {sum(len(m) for m in nodes)} moves; cheapest win total {ideal(nodes):,}")
    for label, ww in (("static", zero), ("start", [int(x) for x in opt("--start", ",".join(["64"] + ["0"] * (len(feats) - 1))).split(",")]), ("fitted", w)):
        t, g, f = cost(nodes, ww)
        print(f"  {label:8s} total {t:>14,} geo {g:>10,.0f} first wins {f:5.1%}")
print("features", ",".join(feats))
print("weights", ",".join(map(str, w)))
