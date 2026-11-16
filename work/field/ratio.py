import sys, math, os
M = "<session-folder>/snekagon"
d = sys.argv[1]
names = ["blue_wins", "blue_wins_holdout", "random_units", "other_openings", "ublock_refutations"]
rows = []
for n in names:
    ours = [int(l.split()[4]) for l in open(f"{d}/{n}.txt") if l.strip()]
    th = []
    for l in open(f"{M}/work/ref/theirs_{n}_r1.txt"):
        p = l.split()
        if len(p) >= 4 and p[0].isdigit() and p[1] == "value":
            th.append((int(p[2]), int(p[3])))
    lines = [l.strip() for l in open(f"{M}/tests/set1/bench_ply5_{n}.txt") if l.strip()]
    assert len(ours) == len(th) == len(lines), (n, len(ours), len(th), len(lines))
    so, st = sum(ours), sum(t[1] for t in th)
    lr = [math.log(o / t[1]) for o, t in zip(ours, th)]
    print(f"{n:20s} ours {so:10d} theirs {st:10d} ratio {so/st:.3f} geomean {math.exp(sum(lr)/len(lr)):.3f} worse {sum(1 for x in lr if x>0)}/{len(lr)}")
    for i, (o, t, l) in enumerate(zip(ours, th, lines)):
        rows.append((o / t[1], n, i, o, t[1], t[0], l))
rows.sort(reverse=True)
print("\nworst 15")
for r in rows[:15]:
    print(f"{r[0]:6.2f} {r[1]}[{r[2]}] ours {r[3]} theirs {r[4]} v={r[5]} | {r[6]}")
print("\nbest 8")
for r in rows[-8:]:
    print(f"{r[0]:6.2f} {r[1]}[{r[2]}] ours {r[3]} theirs {r[4]} v={r[5]} | {r[6]}")
# where the excess nodes are
exc = sorted(rows, key=lambda r: -(r[3] - r[4]))
print("\nlargest absolute excess")
for r in exc[:10]:
    print(f"{r[3]-r[4]:9d} {r[0]:6.2f} {r[1]}[{r[2]}]")
allo = sum(r[3] for r in rows); allt = sum(r[4] for r in rows)
print("all", allo, allt)
