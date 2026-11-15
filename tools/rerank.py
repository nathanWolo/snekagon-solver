"""What would a second look at the best few moves be worth?

    python tools/rerank.py work/rm2_ply06a.csv [more files ...]

For each position in the output of `snek rootmoves`: order the moves by the solver's model score,
then re-order the first K by a second key that needs a look ahead (fewest replies after threat zones,
and so on). The cost of an order is the nodes of every move up to and including the first winning
one. Positions with no winning move are left out (every move is searched whatever the order).
"""
import csv
import math
import sys
from collections import defaultdict

W = [21, 17, 21, -1, 1, -3, 9, -7]


def model(r):
    f = [int(r["lead"]), int(r["size"]), int(r["grows"]), int(r["joined"]), int(r["front"]), int(r["touch"]), int(r["my_ends"]), int(r["their_ends"])]
    return sum(w * x for w, x in zip(W, f))


def cost(rows):
    total = 0
    for r in rows:
        total += int(r["nodes"])
        if r["win"] == "1":
            return total
    return total


def main():
    for path in sys.argv[1:]:
        by_pos = defaultdict(list)
        for r in csv.DictReader(open(path)):
            by_pos[r["pos"]].append(r)
        keys = {
            "fewest replies": lambda r: (int(r["replies"]), -model(r)),
            "replies - 4*my_next": lambda r: (int(r["replies"]) - 4 * int(r["my_next"]), -model(r)),
            "their_best low": lambda r: (int(r["their_best"]), -model(r)),
            "their_reach=0 first": lambda r: (int(r["their_reach"]), -model(r)),
            "their_up low": lambda r: (int(r["their_up"]), -model(r)),
            "lead - their_best": lambda r: (-(int(r["lead"]) + int(r["size"]) * 0 - int(r["their_best"])), -model(r)),
        }
        base = oracle_any = oracle_cheap = 0
        npos = 0
        totals = defaultdict(int)
        for pos, rows in by_pos.items():
            wins = [r for r in rows if r["win"] == "1"]
            if not wins:
                continue
            npos += 1
            order = sorted(rows, key=lambda r: -model(r))
            base += cost(order)
            oracle_any += int(wins[0]["nodes"]) if False else min(int(r["nodes"]) for r in wins[:1]) if False else int(sorted(wins, key=lambda r: -model(r))[0]["nodes"])
            oracle_cheap += min(int(r["nodes"]) for r in wins)
            for k in (3, 5, 10, 20, 40):
                for name, key in keys.items():
                    head = sorted(order[:k], key=key)
                    totals[(name, k)] += cost(head + order[k:])
        print(f"{path}: {npos} positions with a win")
        print(f"  model order            {base:12,d}")
        print(f"  first win in model order, alone {oracle_any:12,d}")
        print(f"  cheapest win, alone    {oracle_cheap:12,d}")
        for name in keys:
            print(f"  {name:22s} " + "  ".join(f"K={k}: {totals[(name, k)] / base:5.2f}x" for k in (3, 5, 10, 20, 40)))


if __name__ == "__main__":
    main()
