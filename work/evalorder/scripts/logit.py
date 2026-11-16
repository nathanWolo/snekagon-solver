"""Logistic fit of "the move wins for the mover" on the evaluation terms (IRLS, pure python).

python logit.py TRAIN.csv TEST.csv FEATS MAXROWS SCALE

Prints the weights (raw and as integers x SCALE), log loss on train and test, and the share of nodes
where the highest-probability move wins (test)."""
import csv, math, random, sys

tr, te, feats, maxrows, scale = sys.argv[1], sys.argv[2], sys.argv[3].split(","), int(sys.argv[4]), float(sys.argv[5])
def load(p, cap):
    rows = [r for r in csv.DictReader(open(p)) if r["win"] in ("0", "1")]
    random.Random(1).shuffle(rows)
    rows = rows[:cap]
    X = [[1.0] + [float(r[f]) for f in feats] for r in rows]
    y = [float(r["win"]) for r in rows]
    return X, y, rows
X, y, _ = load(tr, maxrows)
k = len(X[0])
w = [0.0] * k
def solve(A, b):
    n = len(b)
    M = [A[i][:] + [b[i]] for i in range(n)]
    for c in range(n):
        p = max(range(c, n), key=lambda r: abs(M[r][c]))
        M[c], M[p] = M[p], M[c]
        if abs(M[c][c]) < 1e-12:
            continue
        for r in range(n):
            if r != c:
                f = M[r][c] / M[c][c]
                for j in range(c, n + 1):
                    M[r][j] -= f * M[c][j]
    return [M[i][n] / M[i][i] if abs(M[i][i]) > 1e-12 else 0.0 for i in range(n)]
for it in range(8):
    H = [[1e-6 * (i == j) for j in range(k)] for i in range(k)]
    g = [0.0] * k
    for xi, yi in zip(X, y):
        z = sum(a * b for a, b in zip(w, xi))
        p = 1 / (1 + math.exp(-max(-30, min(30, z))))
        s = p * (1 - p)
        for i in range(k):
            g[i] += (yi - p) * xi[i]
            si = s * xi[i]
            for j in range(i, k):
                H[i][j] += si * xi[j]
    for i in range(k):
        for j in range(i):
            H[i][j] = H[j][i]
    d = solve(H, g)
    w = [a + b for a, b in zip(w, d)]
def loss(X, y):
    t = 0.0
    for xi, yi in zip(X, y):
        z = sum(a * b for a, b in zip(w, xi)); p = 1 / (1 + math.exp(-max(-30, min(30, z))))
        t -= math.log(max(1e-9, p if yi else 1 - p))
    return t / len(y)
Xt, yt, rows = load(te, 10 ** 9)
print("features", feats)
print("raw", " ".join(f"{f} {v:+.4f}" for f, v in zip(["bias"] + feats, w)))
print("int x%g:" % scale, ",".join(str(round(v * scale)) for v in w[1:]))
print("log loss train %.4f test %.4f (base rate test %.3f)" % (loss(X, y), loss(Xt, yt), sum(yt) / len(yt)))
by = {}
for xi, yi, r in zip(Xt, yt, rows):
    z = sum(a * b for a, b in zip(w, xi))
    by.setdefault(r["pos"], []).append((z, yi))
print("test nodes", len(by), "top move wins %.3f" % (sum(max(v)[1] for v in by.values()) / len(by)))
