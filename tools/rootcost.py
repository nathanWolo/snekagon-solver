"""Within a position, what goes with a cheap win and with a win at all?

    python tools/rootcost.py FILE.csv [FILE.csv ...]

For each feature: its correlation, within positions, with log(nodes) among the winning moves, with
log(nodes) among the losing moves, and with winning. Position effects are removed by demeaning.
"""
import csv, math, sys
from collections import defaultdict

SKIP = {"pos", "move", "win", "nodes", "static", "their_up_before"}

def main():
    by_pos = defaultdict(list)
    for path in sys.argv[1:]:
        for r in csv.DictReader(open(path)):
            if r["win"] == "?":
                r["win"] = "0"
            by_pos[(path, r["pos"])].append({k: int(v) for k, v in r.items()})
    feats = [k for k in next(iter(by_pos.values()))[0] if k not in SKIP] + ["static"]
    def corr(rows_by_pos, f, target):
        xs, ys = [], []
        for ms in rows_by_pos:
            if len(ms) < 3:
                continue
            mx = sum(m[f] for m in ms) / len(ms)
            my = sum(target(m) for m in ms) / len(ms)
            xs += [m[f] - mx for m in ms]
            ys += [target(m) - my for m in ms]
        sx = math.sqrt(sum(x * x for x in xs)) or 1
        sy = math.sqrt(sum(y * y for y in ys)) or 1
        return sum(x * y for x, y in zip(xs, ys)) / (sx * sy)
    won = [ms for ms in by_pos.values() if any(m["win"] for m in ms)]
    wins = [[m for m in ms if m["win"]] for ms in won]
    losses = [[m for m in ms if not m["win"]] for ms in won]
    lognodes = lambda m: math.log(m["nodes"] + 1)
    print(f"{len(won)} won positions; wins {sum(map(len, wins))}, losses {sum(map(len, losses))}")
    print(f"{'feature':16s} {'cost|win':>9s} {'cost|loss':>9s} {'win':>7s}")
    for f in feats:
        print(f"{f:16s} {corr(wins, f, lognodes):+9.3f} {corr(losses, f, lognodes):+9.3f} {corr(won, f, lambda m: m['win']):+7.3f}")
    # spread of win costs within a position
    ratios = []
    for ws in wins:
        c = sorted(m["nodes"] for m in ws)
        if len(c) >= 4:
            ratios.append((c[len(c) // 2] + 1) / (c[0] + 1))
    ratios.sort()
    print(f"median win cost / cheapest win cost within a position: median {ratios[len(ratios) // 2]:.1f}, quartiles {ratios[len(ratios) // 4]:.1f} to {ratios[3 * len(ratios) // 4]:.1f}")

main()
