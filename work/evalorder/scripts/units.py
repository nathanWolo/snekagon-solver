"""Per-unit ratios from a yard.py / tree.py JSONL. python units.py FILE BASE CAND [UNIT_SIZE] [--keep POSLIST] [--write PREFIX]
Prints totals and, through tools/hc.py report --ratio --lower-is-better, the per-unit log ratio for nodes
and (when present) seconds. --keep FILE: only positions listed (one index a line)."""
import json, math, os, subprocess, sys
MAIN = "<session-folder>/snekagon"
f, base, cand = sys.argv[1:4]
us = int(sys.argv[4]) if len(sys.argv) > 4 and sys.argv[4].isdigit() else 2
keep = None
if "--keep" in sys.argv:
    keep = {int(x) for x in open(sys.argv[sys.argv.index("--keep") + 1]).read().split()}
d = {}
for l in open(f):
    r = json.loads(l)
    d.setdefault(r.get("arm", r.get("name")), {})[r["pos"]] = r
B, C = d[base], d[cand]
pos = sorted(p for p in B if p in C and (keep is None or p in keep))
units = {}
for p in pos:
    units.setdefault(p // us, []).append(p)
units = {u: ps for u, ps in units.items() if len(ps) == us or keep is not None}
wrong = sum(B[p].get("wrong", 0) + C[p].get("wrong", 0) + B[p].get("unknown", 0) + C[p].get("unknown", 0) for p in pos)
print(f"{cand} vs {base}: {len(units)} units, {sum(len(v) for v in units.values())} positions, wrong {wrong}")
for key in ("nodes", "s"):
    if key not in B[pos[0]]:
        continue
    tb = sum(B[p][key] for ps in units.values() for p in ps)
    tc = sum(C[p][key] for ps in units.values() for p in ps)
    tmp = f"{os.path.dirname(os.path.abspath(f))}/_u_{cand}_{key}.jsonl"
    with open(tmp, "w", newline="\n") as fo:
        for u, ps in sorted(units.items()):
            fo.write(json.dumps({"cand": sum(C[p][key] for p in ps), "base": sum(B[p][key] for p in ps)}) + "\n")
    print(f"  {key}: total {tc:,.2f} / {tb:,.2f} = {tc / tb:.3f}")
    out = subprocess.run(["python", f"{MAIN}/tools/hc.py", "report", tmp, "--ratio", "--lower-is-better"], capture_output=True, text=True).stdout
    print("   ", out.strip().replace("\n", "\n    "))
