"""Join the evaluation terms of the level-9 nodes (f9.csv, evalmoves --nodes 1 on sel_on_9.txt) with the
order study's labels (p3/on9_*.csv: win, nodes per move). Root 27 of kids_all (a proof4b gate position)
is dropped. Writes train/test CSVs split by root: roots 24-31 train, 32-35 test."""
import csv, glob, sys
S = "<scratchpad>"
root = [int(l.split()[0]) for l in open(f"{S}/wt/o/work/order/sel_on_9.txt.meta")]
lab = {}
for p in glob.glob(f"{S}/wt/o/work/order/p3/on9_*.csv"):
    rows = [l for l in open(p) if not l.startswith("#")]
    for r in csv.DictReader(rows):
        if r["win"] in ("0", "1"):
            lab[(int(r["pos"]), int(r["move"]))] = (r["win"], r["nodes"])
f = list(csv.DictReader(open(f"{S}/eo/fit/f9.csv")))
cols = list(f[0].keys())
have = {}
for r in f:
    k = (int(r["pos"]), int(r["move"]))
    have.setdefault(k[0], []).append(r)
outs = {"train": open(f"{S}/eo/fit/n9_train.csv", "w", newline=""), "test": open(f"{S}/eo/fit/n9_test.csv", "w", newline="")}
w = {k: csv.DictWriter(v, fieldnames=cols) for k, v in outs.items()}
for x in w.values():
    x.writeheader()
n = {"train": 0, "test": 0}
for pos, rows in have.items():
    if root[pos] == 27:
        continue
    if not all((pos, int(r["move"])) in lab for r in rows):
        continue  # every move of the node must be labelled
    split = "train" if root[pos] <= 31 else "test"
    n[split] += 1
    for r in rows:
        r = dict(r)
        r["win"], r["nodes"] = lab[(pos, int(r["move"]))]
        w[split].writerow(r)
print(n)
