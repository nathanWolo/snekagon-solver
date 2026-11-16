"""In-tree coordinate search of the evaluation weights on node counts (exact), fit roots only.

python tune.py EXE ROOTS.txt FROM TO OUT.jsonl LEVEL START STEPS ROUNDS [PER_HOLD] [--probe V;V;...]

Objective: mean over the roots of log(nodes with the weights / nodes with the option off). The centre is
the best vector evaluated so far; each round evaluates every single +-step move around it and the
vector that takes every single move that improved in the round before. PER_HOLD vectors a lock hold.
"""
import json, math, sys
sys.path.insert(0, __file__.rsplit("/", 1)[0] if "/" in __file__ else ".")
import tree

args = sys.argv[1:]
probe = []
if "--probe" in args:
    k = args.index("--probe"); probe = [[int(x) for x in v.split(",")] for v in args[k + 1].split(";")]; del args[k:k + 2]
exe, roots, a, b, out, level = args[0], args[1], int(args[2]), int(args[3]), args[4], int(args[5])
w = [int(x) for x in args[6].split(",")]
steps = [int(x) for x in args[7].split(",")]
rounds = int(args[8])
per = int(args[9]) if len(args) > 9 else 12
name = lambda v: f"L{level}w" + "_".join(map(str, v))
opts = lambda v: f"--eval-order {level} --eval-w {','.join(map(str, v))}"
seen = {}

def evaluate(vs):
    vs = [list(v) for v in dict.fromkeys(tuple(v) for v in vs) if tuple(v) not in seen]
    for k in range(0, len(vs), per):
        tree.run(exe, roots, a, b, 16, out, [("off", "")] + [(name(v), opts(v)) for v in vs[k:k + per]], tag="tune")
    t = tree.totals(out)
    off = t["off"]
    for v in vs:
        r = t.get(name(v), {})
        if len(r) < b - a:
            seen[tuple(v)] = (float("inf"), 0); continue
        assert sum(x["wrong"] for x in r.values()) == 0, ("WRONG", v)
        lg = sum(math.log(r[i]["nodes"] / off[i]["nodes"]) for i in range(a, b)) / (b - a)
        tot = sum(r[i]["nodes"] for i in range(a, b)) / sum(off[i]["nodes"] for i in range(a, b))
        seen[tuple(v)] = (lg, tot)

evaluate([w] + probe)
for v in [w] + probe:
    print("probe", v, "mean log %.4f total %.4f" % seen[tuple(v)], flush=True)
combo = None
for st in steps:
    for rnd in range(rounds):
        c = min(seen, key=lambda v: seen[v][0])
        cands = []
        for i in range(len(c)):
            for sg in (1, -1):
                v = list(c); v[i] += sg * st
                cands.append(v)
        if combo:
            cands.append(combo)
        before = seen[c][0]
        evaluate(cands)
        imp = [v for v in cands if seen[tuple(v)][0] < before - 0.002]
        combo = None
        if len(imp) > 1:
            combo = list(c)
            for v in imp:
                for i in range(len(c)):
                    if v[i] != c[i] and combo[i] == c[i]:
                        combo[i] = v[i]
        nb = min(seen, key=lambda v: seen[v][0])
        print(f"step {st} round {rnd}: best {list(nb)} mean log {seen[nb][0]:.4f} total {seen[nb][1]:.4f}; improving moves {len(imp)}", flush=True)
        if not imp and not combo:
            break
nb = min(seen, key=lambda v: seen[v][0])
print("final", ",".join(map(str, nb)), "mean log %.4f total %.4f" % seen[nb])
