"""What would spreading effort over the best few moves be worth, against committing to the first?

    python tools/spreadsim.py work/rootmoves_ply06.csv [more files ...]

Input: the output of `snek rootmoves` (every root move of a position solved on its own: win or
not, and its nodes). Positions with no winning move are left out. Moves are ordered by the model.

  commit      : search the moves in order, each to the end, stop at the first win (alpha-beta).
  spread k    : keep the k best unsolved moves alive and give them effort at the same rate; a
                move that is refuted leaves and the next in order joins; stop when one is proved
                (an idealised proof-number search with a window of k and no knowledge of costs).
  spread k /r : the same, but each place down the window gets 1/r of the effort of the one above.
  first win   : only the nodes of the first winning move in the order (a perfect win/loss guess).
  cheapest win: only the nodes of the cheapest winning move (a perfect guess of cost as well).
"""
import csv
import sys
from collections import defaultdict

W = [21, 17, 21, -1, 1, -3, 9, -7]


def model(r):
    f = [int(r["lead"]), int(r["size"]), int(r["grows"]), int(r["joined"]), int(r["front"]), int(r["touch"]), int(r["my_ends"]), int(r["their_ends"])]
    return sum(w * x for w, x in zip(W, f))


def commit(order):
    total = 0
    for nodes, win in order:
        total += nodes
        if win:
            return total
    return total


def spread(order, k, ratio=1.0):
    live = []
    nxt = 0
    spent = 0.0
    while True:
        while len(live) < k and nxt < len(order):
            live.append([float(order[nxt][0]), order[nxt][1]])
            nxt += 1
        if not live:
            return spent
        rates = [ratio ** -i for i in range(len(live))]
        t = min(c[0] / w for c, w in zip(live, rates))
        for c, w in zip(live, rates):
            spent += t * w
            c[0] -= t * w
        if any(c[1] and c[0] <= 1e-6 for c in live):
            return spent
        live = [c for c in live if c[0] > 1e-6]


def main():
    for path in sys.argv[1:]:
        by_pos = defaultdict(list)
        unknown = 0
        for r in csv.DictReader(open(path)):
            if r["win"] == "?":
                unknown += 1
            by_pos[r["pos"]].append(r)
        tot = defaultdict(float)
        npos = 0
        first_rank = []
        for rows in by_pos.values():
            rows.sort(key=lambda r: -model(r))
            order = [(int(r["nodes"]), r["win"] == "1") for r in rows]
            if not any(w for n, w in order):
                continue
            npos += 1
            first_rank.append(next(i for i, (n, w) in enumerate(order) if w))
            tot["commit"] += commit(order)
            tot["first win"] += next(n for n, w in order if w)
            tot["cheapest win"] += min(n for n, w in order if w)
            for k in (2, 3, 5, 10):
                tot[f"spread {k}"] += spread(order, k)
            for k, r in ((3, 2.0), (3, 4.0), (5, 2.0), (5, 4.0)):
                tot[f"spread {k} /{r:g}"] += spread(order, k, r)
        first_rank.sort()
        at0 = sum(1 for x in first_rank if x == 0)
        print(f"\n{path}: {npos} won positions, {unknown} moves of unknown value; the model's first move wins in {at0} ({100 * at0 / npos:.0f}%)")
        base = tot["commit"]
        for name in ["commit", "spread 2", "spread 3", "spread 5", "spread 10", "spread 3 /2", "spread 3 /4", "spread 5 /2", "spread 5 /4", "first win", "cheapest win"]:
            print(f"  {name:13s} {tot[name] / 1e6:9.2f} M nodes   {tot[name] / base:5.2f}x of commit")


main()
