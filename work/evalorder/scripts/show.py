import json, math, sys
d = {}
for l in open(sys.argv[1]):
    r = json.loads(l)
    d.setdefault(r["name"], {})[r["pos"]] = r["nodes"]
off = d["off"]
rows = []
for n, r in d.items():
    ks = [k for k in r if k in off]
    if not ks:
        continue
    lg = sum(math.log(r[k] / off[k]) for k in ks) / len(ks)
    tot = sum(r[k] for k in ks) / sum(off[k] for k in ks)
    rows.append((lg, tot, len(ks), n))
for lg, tot, k, n in sorted(rows):
    print(f"{lg:+.4f} {tot:.4f} {k:3d} {n}")
