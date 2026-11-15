"""Fits a move-ordering model on the CSVs of `snek rootmoves` and scores it on other CSVs.

    python tools/rootfit.py --train A.csv [B.csv ...] --test C.csv [D.csv ...]
                            [--features lead,size,...] [--search] [--objective total|log]

An ordering is scored by what it pays at a node: the nodes of every move tried up to and including
its first win, summed over the positions that have a win. The model is a linear score with integer
weights (scaled by 64), highest first, ties by the solver's static score.
Start: a logistic fit of "the move wins" minus a least-squares fit of log(nodes), which is the rule
"highest win probability per unit of cost first". --search then improves the integer weights by
coordinate search on the cost itself (on the training files only).
"""
import csv
import math
import sys
from collections import defaultdict

# the order the solver takes its weights in
SOLVER = ["lead", "size", "grows", "joined", "front", "touch", "my_ends", "their_ends", "their_best", "my_next", "their_up", "my_up"]
SCALE = 64


def load(paths):
    by_pos = defaultdict(list)
    for path in paths:
        for r in csv.DictReader(open(path)):
            if r["win"] == "?":
                r["win"] = "0"
            r = {k: int(v) for k, v in r.items()}
            by_pos[(path, r["pos"])].append(r)
    return [ms for ms in by_pos.values() if any(m["win"] for m in ms)]


def cost(won, key):
    total, logs, first, ranks = 0, 0.0, 0, 0
    for ms in won:
        c = 0
        for i, m in enumerate(sorted(ms, key=key)):
            c += m["nodes"]
            if m["win"]:
                first += i == 0
                ranks += i
                break
        total += c
        logs += math.log(c)
    return total, logs / len(won), first / len(won), ranks / len(won)


def standardise(data, feats):
    mean = {f: sum(m[f] for m in data) / len(data) for f in feats}
    sd = {f: (sum((m[f] - mean[f]) ** 2 for m in data) / len(data)) ** 0.5 or 1.0 for f in feats}
    return mean, sd


def fit(won, feats, target, logistic, epochs=300, lr=0.3):
    """Gradient descent on standardised features; returns raw weights."""
    data = [m for ms in won for m in ms]
    mean, sd = standardise(data, feats)
    x = [[(m[f] - mean[f]) / sd[f] for f in feats] for m in data]
    y = [target(m) for m in data]
    w, b = [0.0] * len(feats), sum(y) / len(y) if not logistic else 0.0
    n = len(x)
    for _ in range(epochs):
        gw, gb = [0.0] * len(feats), 0.0
        for xi, yi in zip(x, y):
            z = b + sum(wj * xj for wj, xj in zip(w, xi))
            e = (1 / (1 + math.exp(-max(-30, min(30, z)))) if logistic else z) - yi
            gb += e
            for j, xj in enumerate(xi):
                gw[j] += e * xj
        b -= lr * gb / n
        w = [wj - lr * g / n for wj, g in zip(w, gw)]
    return {f: wj / sd[f] for f, wj in zip(feats, w)}


def main():
    args = sys.argv[1:]
    train = args[args.index("--train") + 1:args.index("--test")]
    test = [a for a in args[args.index("--test") + 1:] if a.endswith(".csv")]
    feats = SOLVER
    if "--features" in args:
        feats = args[args.index("--features") + 1].split(",")
    objective = args[args.index("--objective") + 1] if "--objective" in args else "total"
    tr, te = load(train), load(test)
    wp = fit(tr, feats, lambda m: m["win"], True)
    wc = fit(tr, feats, lambda m: math.log(m["nodes"] + 1), False)
    start = {f: wp[f] - wc[f] for f in feats}
    ints = {f: round(start[f] * SCALE) for f in feats}

    def key(w):
        return lambda m: (-sum(w[f] * m[f] for f in feats), -m["static"])

    def show(name, won, w):
        print(f"{name}: {len(won)} won positions, {sum(len(ms) for ms in won)} moves")
        for label, k in (
            ("cheapest win first (ideal)", lambda m: (not m["win"], m["nodes"])),
            ("any win first", lambda m: (not m["win"],)),
            ("static score (the solver's)", lambda m: -m["static"]),
            ("win model alone", key(wp)),
            ("model", key(w)),
        ):
            total, logs, first, rank = cost(won, k)
            print(f"  {label:30s} total {total:>12,}  geo-mean {math.exp(logs):>9,.0f}  first move wins {first:5.0%}  mean rank {rank:5.1f}")

    if "--search" in args:
        idx = 0 if objective == "total" else 1
        best = cost(tr, key(ints))[idx]
        improved = True
        while improved:
            improved = False
            for f in feats:
                for step in (16, -16, 4, -4, 1, -1):
                    trial = dict(ints)
                    trial[f] += step
                    c = cost(tr, key(trial))[idx]
                    if c < best:
                        best, ints, improved = c, trial, True
    if "--win-only" in args:
        ints = {f: round(wp[f] * SCALE) for f in feats}
    print("weights (x64): " + "  ".join(f"{f} {ints[f]:+d}" for f in feats))
    print("for the solver: --model " + ",".join(str(ints.get(f, 0)) for f in SOLVER))
    show("train", tr, ints)
    show("test", te, ints)


if __name__ == "__main__":
    main()
