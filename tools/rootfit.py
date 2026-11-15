"""Fits a move-ordering model on the CSVs of `snek rootmoves` and scores it on other CSVs.

    python tools/rootfit.py --train A.csv [B.csv ...] --test C.csv [D.csv ...] --features lead,their_ends,...

The model is logistic: P(move wins). An ordering is scored by what it pays at a node: the nodes of
every move tried up to and including its first win, summed over the positions that have a win.
Prints integer weights (scaled by 16) ready for the solver.
"""
import csv
import math
import sys
from collections import defaultdict

ALL = ["size", "grows", "joined", "gain", "lead", "front", "touch", "my_up", "their_up", "d_their_up", "their_moves", "my_moves", "replies", "my_ends", "their_ends"]


def load(paths):
    by_pos = defaultdict(list)
    for path in paths:
        for r in csv.DictReader(open(path)):
            capped = r["win"] == "?"
            if capped:
                r["win"] = "0"
            r = {k: int(v) for k, v in r.items()}
            r["d_their_up"] = r["their_up_before"] - r["their_up"]
            by_pos[(path, r["pos"])].append(r)
    return {k: ms for k, ms in by_pos.items() if any(m["win"] for m in ms)}


def cost(won, key):
    total, first, ranks = 0, 0, 0
    for ms in won.values():
        for i, m in enumerate(sorted(ms, key=key)):
            total += m["nodes"]
            if m["win"]:
                first += i == 0
                ranks += i
                break
    return total, first / len(won), ranks / len(won)


def fit(won, feats, epochs=400, lr=0.5):
    data = [m for ms in won.values() for m in ms]
    mean = {f: sum(m[f] for m in data) / len(data) for f in feats}
    sd = {f: (sum((m[f] - mean[f]) ** 2 for m in data) / len(data)) ** 0.5 or 1.0 for f in feats}
    x = [[(m[f] - mean[f]) / sd[f] for f in feats] for m in data]
    y = [m["win"] for m in data]
    w, b = [0.0] * len(feats), 0.0
    n = len(x)
    for _ in range(epochs):
        gw, gb = [0.0] * len(feats), 0.0
        for xi, yi in zip(x, y):
            z = b + sum(wj * xj for wj, xj in zip(w, xi))
            e = 1 / (1 + math.exp(-max(-30, min(30, z)))) - yi
            gb += e
            for j, xj in enumerate(xi):
                gw[j] += e * xj
        b -= lr * gb / n
        w = [wj - lr * (g / n + 1e-4 * wj) for wj, g in zip(w, gw)]
    return {f: wj / sd[f] for f, wj in zip(feats, w)}  # raw weights; the constant does not matter for an order


def main():
    args = sys.argv[1:]
    train = args[args.index("--train") + 1:args.index("--test")]
    rest = args[args.index("--test") + 1:]
    test = [a for a in rest if not a.startswith("--") and a.endswith(".csv")]
    feats = ALL
    if "--features" in args:
        feats = args[args.index("--features") + 1].split(",")
    tr, te = load(train), load(test)
    w = fit(tr, feats)
    scale = 16
    ints = {f: round(v * scale) for f, v in w.items()}
    print("weights (raw, and as integers scaled by 16):")
    for f in feats:
        print(f"  {f:12s} {w[f]:+.4f}  {ints[f]:+d}")
    for name, won in (("train", tr), ("test", te)):
        print(f"{name}: {len(won)} won positions, {sum(len(ms) for ms in won.values())} moves")
        for label, key in (
            ("cheapest win first (ideal)", lambda m: (not m["win"], m["nodes"])),
            ("any win first", lambda m: (not m["win"],)),
            ("static score (the solver's)", lambda m: -m["static"]),
            ("model", lambda m: -sum(w[f] * m[f] for f in feats)),
            ("model, integer weights", lambda m: -sum(ints[f] * m[f] for f in feats)),
        ):
            total, first, rank = cost(won, key)
            print(f"  {label:30s} total {total:>12,}  first move wins {first:5.0%}  mean rank of first win {rank:5.1f}")


if __name__ == "__main__":
    main()
