"""Fits move orders on the train parents' nodes and scores them on the test parents' nodes.

    python tools/order/fit.py DUMP.txt.meta "CSV1 CSV2 ..." [--level 7] [--mlp]

Split by parent: work/parents.txt. Root -> parent: work/order/kids_ROOTS_parent.txt (--roots train|test|all
names the kids file the dump was made from; default all = kids_all_parent.txt).
Prints, per level, for train and test: nodes, parents, and the nodes paid by each order (sum over
nodes), as a ratio to today's order and to the oracles; failed-move nodes and win nodes apart.
"""
import math
import sys

import numpy as np

sys.path.insert(0, "tools/order")
import sim  # noqa: E402

args = sys.argv[1:]
meta = args[0]
csvs = args[1].split()
level_only = int(args[args.index("--level") + 1]) if "--level" in args else None
roots_name = args[args.index("--roots") + 1] if "--roots" in args else "all"
do_mlp = "--mlp" in args

split = {}
for line in open("work/parents.txt"):
    if line.startswith("#"):
        continue
    i, s, _ = line.split()
    split[int(i)] = s
root_parent = [int(l) for l in open(f"work/order/kids_{roots_name}_parent.txt")]
nodes = [n for n in sim.load(csvs, meta, root_parent) if sim.usable(n)]
if "--debug-split" in args:
    for n in nodes:
        n["parent"] = n["idx"] % 4
        split = {0: "test", 1: "train", 2: "train", 3: "train"}
for n in nodes:
    assert split[n["parent"]] in ("train", "test"), "a held-out parent in the data"
allnodes_count = len(sim.load(csvs, meta, root_parent))

BASE = ["lead", "n", "grows", "joined", "front", "touch", "my_ends8", "their_ends8"]
LOOK = ["my_up", "their_up", "their_moves", "my_moves", "their_best", "my_next"]


def popcount(x):
    return bin(x).count("1")


def feats(node, extended):
    """One row a move. extended: look-ahead features, piece one-hot, threat-cover counts, and every
    feature again relative to the node's best (x - max over the node's moves)."""
    ms = node["moves"]
    rows = []
    for mv in ms:
        r = [mv[k] for k in BASE]
        if extended:
            r += [mv[k] for k in LOOK]
            r += [mv["my_up"] - mv["their_up"], mv["my_next"] - mv["their_best"], math.log1p(mv["their_moves"])]
            zs = node["scan"]
            cov = sum(1 for z, v in zs if mv["cells"] & z)
            r += [cov, int(cov == len(zs)) if zs else 1, len(zs)]
            r += [int(mv["piece"] == p) for p in range(12)]
            r += [int(mv["m"] in (node["tt"], node["k1"], node["k2"]))]
        rows.append(r)
    X = np.array(rows, dtype=float)
    if extended:
        X = np.hstack([X, X[:, :17] - X[:, :17].max(axis=0, keepdims=True), X[:, :17] - X[:, :17].mean(axis=0, keepdims=True)])
    if extended == "board":
        bits = np.array(BOARD_BITS, dtype=object)
        def enc(x):
            return np.array([(x >> int(b)) & 1 for b in bits], dtype=float)
        own, th = enc(node["own"]), enc(node["theirs"])
        mvb = np.array([enc(mv["cells"]) for mv in ms])
        bdb = np.array([enc(mv["border"]) for mv in ms])
        X = np.hstack([X, mvb, bdb, bdb * own, bdb * th, mvb * 0 + own]).astype(np.float32)
    return X


def stack(ns, extended):
    Xs, ys, cs, gs = [], [], [], []
    for g, n in enumerate(ns):
        X = feats(n, extended)
        Xs.append(X)
        ys.append([1.0 if mv["win"] == "1" else 0.0 for mv in n["moves"]])
        cs.append([math.log(mv["nodes"] + 1) for mv in n["moves"]])
        gs.append([g] * len(n["moves"]))
    return np.vstack(Xs), np.concatenate(ys), np.concatenate(cs), np.concatenate(gs)


def logistic(X, y, l2=1e-2, iters=300):
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Z = (X - mu) / sd
    Z = np.hstack([Z, np.ones((len(Z), 1))])
    w = np.zeros(Z.shape[1])
    for _ in range(iters):  # Newton
        p = 1 / (1 + np.exp(-Z @ w))
        g = Z.T @ (p - y) + l2 * len(y) * np.r_[w[:-1], 0]
        H = (Z * (p * (1 - p))[:, None]).T @ Z + l2 * len(y) * np.diag(np.r_[np.ones(len(w) - 1), 0])
        step = np.linalg.solve(H, g)
        w -= step
        if np.abs(step).max() < 1e-7:
            break
    return lambda Xn: 1 / (1 + np.exp(-(np.hstack([(Xn - mu) / sd, np.ones((len(Xn), 1))]) @ w)))


def ridge(X, y, l2=1.0):
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Z = np.hstack([(X - mu) / sd, np.ones((len(X), 1))])
    A = Z.T @ Z + l2 * np.diag(np.r_[np.ones(Z.shape[1] - 1), 0])
    w = np.linalg.solve(A, Z.T @ y)
    return lambda Xn: np.hstack([(Xn - mu) / sd, np.ones((len(Xn), 1))]) @ w


def mlp(X, y, c, hidden=32, epochs=60, lr=3e-3, seed=0):
    """Two heads on one hidden layer: logit of win, and log nodes. Adam, full batch in chunks."""
    rng = np.random.default_rng(seed)
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Z = (X - mu) / sd
    cm, cs = c.mean(), c.std() + 1e-9
    ct = (c - cm) / cs
    d = Z.shape[1]
    P = {"W1": rng.normal(0, 1 / math.sqrt(d), (d, hidden)), "b1": np.zeros(hidden),
         "W2": rng.normal(0, 1 / math.sqrt(hidden), (hidden, 2)), "b2": np.zeros(2)}
    M = {k: np.zeros_like(v) for k, v in P.items()}
    V = {k: np.zeros_like(v) for k, v in P.items()}
    t = 0
    n = len(Z)
    for ep in range(epochs):
        perm = rng.permutation(n)
        for s in range(0, n, 512):
            idx = perm[s:s + 512]
            x, yy, cc = Z[idx], y[idx], ct[idx]
            h = np.maximum(x @ P["W1"] + P["b1"], 0)
            o = h @ P["W2"] + P["b2"]
            p = 1 / (1 + np.exp(-o[:, 0]))
            go = np.stack([(p - yy), (o[:, 1] - cc)], 1) / len(idx)
            G = {"W2": h.T @ go + 1e-4 * P["W2"], "b2": go.sum(0)}
            gh = (go @ P["W2"].T) * (h > 0)
            G["W1"] = x.T @ gh + 1e-4 * P["W1"]
            G["b1"] = gh.sum(0)
            t += 1
            for k in P:
                M[k] = 0.9 * M[k] + 0.1 * G[k]
                V[k] = 0.999 * V[k] + 0.001 * G[k] ** 2
                P[k] -= lr * (M[k] / (1 - 0.9 ** t)) / (np.sqrt(V[k] / (1 - 0.999 ** t)) + 1e-8)

    def f(Xn):
        h = np.maximum(((Xn - mu) / sd) @ P["W1"] + P["b1"], 0)
        o = h @ P["W2"] + P["b2"]
        return 1 / (1 + np.exp(-o[:, 0])), o[:, 1] * cs + cm
    return f


def score(ns, keyfn, **kw):
    tot = fails = wins = 0
    for n in ns:
        c, _, f, w = sim.replay(n, keyfn(n), **kw)
        tot += n["w"] * c
        fails += n["w"] * f
        wins += n["w"] * w
    return int(tot), int(fails), int(wins)


def report(name, ns, res, base, ch, an):
    tot, fails, wins = res
    print(f"  {name:34s} {tot:12d}  x{tot / base:5.3f} today  x{tot / ch:6.2f} cheapest  x{tot / an:5.2f} any   fails {fails:11d} wins {wins:11d}")


used = 0
for n in nodes:
    used |= n["own"] | n["theirs"]
    for mv in n["moves"]:
        used |= mv["cells"] | mv["border"]
BOARD_BITS = [b for b in range(128) if used >> b & 1]
levels = sorted({n["level"] for n in nodes}) if level_only is None else [level_only]
print(f"# nodes labelled {allnodes_count}, usable (a win, no capped move) {len(nodes)}")
for L in levels:
    tr = [n for n in nodes if n["level"] == L and split[n["parent"]] == "train"]
    te = [n for n in nodes if n["level"] == L and split[n["parent"]] == "test"]
    if not tr or not te:
        print(f"level {L}: train {len(tr)} test {len(te)}: not enough")
        continue
    models = {}
    X, y, c, g = stack(tr, False)
    f8 = logistic(X, y)
    models["8-feature logistic (refit)"] = lambda n: f8(feats(n, False))
    X, y, c, g = stack(tr, True)
    fx = logistic(X, y)
    rc = ridge(X, c)
    models["extended logistic p(win)"] = lambda n: fx(feats(n, True))
    models["extended p(win)/exp(logcost) Smith"] = lambda n: (lambda F: fx(F) / np.exp(rc(F)))(feats(n, True))
    models["extended p/exp(logcost) by win-cost"] = None
    # cost of a winning move alone: fit log nodes on the wins only
    rw = ridge(X[y == 1], c[y == 1])
    rl = ridge(X[y == 0], c[y == 0])
    models["extended p/E[cost] (win & fail costs)"] = lambda n: (lambda F: (lambda p: p / (p * np.exp(rw(F)) + (1 - p) * np.exp(rl(F))))(fx(F)))(feats(n, True))
    del models["extended p/exp(logcost) by win-cost"]
    if "--board" in args:
        import random as _r
        sub = _r.Random(3).sample(tr, min(1500, len(tr)))
        Xb, yb, cb, gb = stack(sub, "board")
        fb = mlp(Xb, yb, cb, hidden=64, epochs=40)
        models["MLP 64 board+ext p/exp(logcost)"] = lambda n: (lambda pc: pc[0] / np.exp(pc[1]))(fb(feats(n, "board")))
        models["MLP 64 board+ext p(win)"] = lambda n: fb(feats(n, "board"))[0]
    if do_mlp:
        fm = mlp(X, y, c)
        models["MLP 32 p/exp(logcost)"] = lambda n: (lambda pc: pc[0] / np.exp(pc[1]))(fm(feats(n, True)))
        models["MLP 32 p(win)"] = lambda n: fm(feats(n, True))[0]
    for name, ns in (("train", tr), ("test", te)):
        base = score(ns, lambda n: [sim.today_key(n, mv) for mv in n["moves"]])
        ch = sum(n["w"] * sim.oracle_cheapest(n) for n in ns)
        an = sum(n["w"] * sim.oracle_any(n) for n in ns)
        me = sum(n["w"] * sim.oracle_mean(n) for n in ns)
        parents = len({n["parent"] for n in ns})
        roots = len({n["root"] for n in ns})
        tree = int(sum(n["w"] * n["tree_total"] for n in ns))
        treew = int(sum(n["w"] * (n["tree_total"] - n["tree_winsub"]) for n in ns if n["tree_cut"]))
        first = sum(1 for n in ns if sim.replay(n, [sim.today_key(n, mv) for mv in n["moves"]])[1] == 1)
        print(f"level {L} {name}: nodes {len(ns)} roots {roots} parents {parents}; today's first move wins {first / len(ns):.1%}; "
              f"oracles: cheapest {ch:.0f} any {an:.0f} mean-win {me:.0f}; in the tree at these nodes {tree} nodes, {treew} under failed moves")
        report("today (replayed)", ns, base, base[0], ch, an)
        report("today, no table move/killers", ns, score(ns, lambda n: [sim.today_key(n, mv) for mv in n["moves"]], ttk=False), base[0], ch, an)
        report("model order at this level (8 feat)", ns, score(ns, lambda n: [sim.model_key(mv) for mv in n["moves"]]), base[0], ch, an)
        for mname, fn in models.items():
            report(mname, ns, score(ns, fn), base[0], ch, an)
        report("ext. Smith, no table move/killers", ns, score(ns, models["extended p(win)/exp(logcost) Smith"], ttk=False), base[0], ch, an)
        for K in (3, 10):
            def topk(n, K=K):
                keys = [sim.today_key(n, mv) for mv in n["moves"]]
                order = sorted(range(len(keys)), key=lambda j: (keys[j], n["moves"][j]["m"]), reverse=True)[:K]
                w = [n["moves"][j]["nodes"] for j in order if n["moves"][j]["win"] == "1"]
                return min(w) if w else sim.replay(n, keys)[0]
            v = sum(n["w"] * topk(n) for n in ns)
            print(f"  {'oracle: cheapest win in first ' + str(K):34s} {v:12.0f}  x{v / base[0]:5.3f} today")
        # how well the cost head ranks winning moves: weighted share of nodes where the Smith key's
        # best winning move is the cheapest winning move
        hit = 0.0
        for n in ns:
            k = models["extended p(win)/exp(logcost) Smith"](n)
            wins = [j for j, mv in enumerate(n["moves"]) if mv["win"] == "1"]
            if len(wins) > 1:
                bj = max(wins, key=lambda j: k[j])
                hit += n["w"] * (n["moves"][bj]["nodes"] == min(n["moves"][j]["nodes"] for j in wins))
        multi = sum(n["w"] for n in ns if sum(mv["win"] == "1" for mv in n["moves"]) > 1)
        print(f"  Smith key picks the cheapest win (nodes with 2+ wins, weighted): {hit / max(multi, 1e-9):.1%}")
        # perfect win label, today's order among wins: what a perfect classifier would get
        report("perfect p(win), today's key in ties", ns, score(ns, lambda n: [(1e9 if mv["win"] == "1" else 0) + sim.today_key(n, mv) for mv in n["moves"]]), base[0], ch, an)
