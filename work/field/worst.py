import math
M = "<session-folder>/snekagon"
S = "<scratchpad>/fs"
names = ["blue_wins", "blue_wins_holdout", "random_units", "other_openings", "ublock_refutations"]
def load(d, n):
    try:
        return [int(l.split()[4]) for l in open(f"{d}/{n}.txt") if l.strip()]
    except FileNotFoundError:
        return None
rows = []
for n in names:
    base = load(S + "/base", n); orc = load(S + "/oracle", n); re7 = load(S + "/re7", n)
    th = [int(l.split()[3]) for l in open(f"{M}/work/ref/theirs_{n}_r1.txt") if l.split() and l.split()[0].isdigit()]
    lines = [l.strip() for l in open(f"{M}/tests/set1/bench_ply5_{n}.txt") if l.strip()]
    for i in range(len(base)):
        rows.append((base[i] / th[i], n, i, base[i], th[i], orc[i] if orc else None, re7[i] if re7 else None, lines[i]))
rows.sort(reverse=True)
print("ratio file[i] ours theirs oracle re7 | moves")
for r in rows[:10]:
    print(f"{r[0]:.2f} {r[1]}[{r[2]}] {r[3]} {r[4]} {r[5]} {r[6]} | {r[7]}")
# geomeans of ratios vs theirs
for col, nm in [(3, "base"), (5, "oracle"), (6, "re7")]:
    for n in names:
        v = [math.log(r[col] / r[4]) for r in rows if r[1] == n and r[col]]
        if v:
            print(nm, n, f"geomean ours/theirs {math.exp(sum(v)/len(v)):.3f}", "worse", sum(1 for x in v if x > 0), "/", len(v))
