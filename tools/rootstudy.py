"""Reads the CSV of `snek rootmoves` and reports what predicts a cheap winning move.

    python tools/rootstudy.py work/rootmoves_ply06.csv [--fit]

For each position: how many moves win, what the cheapest win costs, and what an ordering pays (the
nodes of every move tried up to and including its first win). Orderings are compared by that cost.
--fit also fits a logistic model of "wins" by gradient descent and scores the ordering it gives.
"""
import csv
import math
import sys
from collections import defaultdict

FEATURES = ["size", "grows", "joined", "gain", "lead", "front", "touch", "my_up", "their_up", "d_their_up", "their_moves", "my_moves", "replies", "my_ends", "their_ends"]


def load(path):
    rows = []
    for r in csv.DictReader(open(path)):
        if r["win"] == "?":
            r["win"] = "0"  # undecided within the cap: treat as not a (cheap) win
            r["capped"] = 1
        else:
            r["capped"] = 0
        for k in list(r):
            if k != "capped":
                r[k] = int(r[k])
        r["d_their_up"] = r["their_up_before"] - r["their_up"]
        rows.append(r)
    return rows


def cost_of_order(moves, key):
    """Nodes an ordering pays at this node: every move up to and including the first win."""
    total = 0
    for m in sorted(moves, key=key):
        total += m["nodes"]
        if m["win"]:
            return total
    return total


def main():
    rows = load(sys.argv[1])
    by_pos = defaultdict(list)
    for r in rows:
        by_pos[r["pos"]].append(r)
    won = {p: ms for p, ms in by_pos.items() if any(m["win"] for m in ms)}
    print(f"positions {len(by_pos)}, with a winning move {len(won)}, moves {len(rows)}, capped {sum(r['capped'] for r in rows)}")
    wins = [m for ms in won.values() for m in ms if m["win"]]
    print(f"winning moves per won position: mean {len(wins) / len(won):.1f} of {sum(len(ms) for ms in won.values()) / len(won):.0f}")

    def report(name, key):
        costs = [cost_of_order(ms, key) for ms in won.values()]
        ranks = []
        for ms in won.values():
            order = sorted(ms, key=key)
            ranks.append(next(i for i, m in enumerate(order) if m["win"]))
        first = sum(1 for r in ranks if r == 0) / len(ranks)
        print(f"  {name:34s} total {sum(costs):>12,}  first move wins {first:5.0%}  mean rank of first win {sum(ranks) / len(ranks):5.1f}")
        return sum(costs)

    print("cost of an ordering (nodes up to and including the first win, summed over won positions):")
    report("cheapest win first (ideal)", lambda m: (not m["win"], m["nodes"]))
    report("any win first, as generated", lambda m: (not m["win"],))
    base = report("static score (the solver's)", lambda m: -m["static"])
    for f in FEATURES:
        for sign, label in ((1, "low"), (-1, "high")):
            report(f"{f} {label} first, then static", lambda m, f=f, sign=sign: (sign * m[f], -m["static"]))
    print("win rate and median cost of a win, by feature value:")
    for f in FEATURES:
        buckets = defaultdict(list)
        for ms in won.values():
            for m in ms:
                buckets[m[f]].append(m)
        keys = sorted(buckets)
        if len(keys) > 12:
            # quantile buckets
            vals = sorted(m[f] for ms in won.values() for m in ms)
            cuts = sorted(set(vals[int(len(vals) * q / 8)] for q in range(1, 8)))
            merged = defaultdict(list)
            for k in keys:
                b = sum(1 for c in cuts if k >= c)
                merged[(cuts[b - 1] if b else keys[0])].extend(buckets[k])
            buckets, keys = merged, sorted(merged)
        line = []
        for k in keys:
            ms = buckets[k]
            w = sorted(m["nodes"] for m in ms if m["win"])
            med = w[len(w) // 2] if w else 0
            line.append(f"{k}: {len(w) / len(ms):.0%}/{med // 1000}k/n{len(ms)}")
        print(f"  {f:12s} " + "  ".join(line))

    if "--fit" in sys.argv:
        fit(won, base, report)


def fit(won, base, report):
    feats = FEATURES
    data = [m for ms in won.values() for m in ms]
    mean = {f: sum(m[f] for m in data) / len(data) for f in feats}
    sd = {f: (sum((m[f] - mean[f]) ** 2 for m in data) / len(data)) ** 0.5 or 1.0 for f in feats}
    x = [[(m[f] - mean[f]) / sd[f] for f in feats] for m in data]
    y = [m["win"] for m in data]
    w = [0.0] * len(feats)
    b = 0.0
    lr = 0.5
    for epoch in range(300):
        gw = [0.0] * len(feats)
        gb = 0.0
        for xi, yi in zip(x, y):
            z = b + sum(wj * xj for wj, xj in zip(w, xi))
            p = 1 / (1 + math.exp(-max(-30, min(30, z))))
            e = p - yi
            gb += e
            for j, xj in enumerate(xi):
                gw[j] += e * xj
        n = len(x)
        b -= lr * gb / n
        w = [wj - lr * (g / n + 1e-4 * wj) for wj, g in zip(w, gw)]
    print("logistic fit (standardised features):")
    for f, wj in sorted(zip(feats, w), key=lambda t: -abs(t[1])):
        print(f"  {f:12s} {wj:+.3f}   (raw weight {wj / sd[f]:+.4f} per unit)")

    def score(m):
        return -(b + sum(wj * (m[f] - mean[f]) / sd[f] for wj, f in zip(w, feats)))

    report("logistic model (fit on these)", score)


if __name__ == "__main__":
    main()
