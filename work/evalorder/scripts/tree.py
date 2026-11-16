"""Node counts in the tree for several option sets, many processes inside one lock hold (node counts are
exact; times are not read).

python tree.py EXE FILE FROM TO LANES OUT.jsonl NAME=OPTS [NAME=OPTS ...]

Every position is solved in its own process (fresh table) with --window 0,1. All (name, position) tasks
are dealt over LANES parallel lanes in one lock hold. Records {"name", "pos", "nodes", "wrong"} are
appended to OUT.jsonl; tasks already there are skipped.
"""
import json, os, subprocess, sys

MAIN = "<session-folder>/snekagon"

def run(exe, file, a, b, lanes, out, arms, tag="t"):
    tag = f"{tag}{os.getpid()}"
    done = set()
    if os.path.exists(out):
        for l in open(out):
            r = json.loads(l)
            done.add((r["name"], r["pos"]))
    tasks = [(n, o, i) for n, o in arms for i in range(a, b) if (n, i) not in done]
    if not tasks:
        return
    d = os.path.dirname(os.path.abspath(out))
    lanefiles = []
    for k in range(lanes):
        mine = tasks[k::lanes]
        if not mine:
            continue
        lf = f"{d}/{tag}_lane{k}.sh"
        with open(lf, "w", newline="\n") as fh:
            for n, o, i in mine:
                e = exe
                if o.startswith("@"):
                    e, _, o = o[1:].partition("::")
                fh.write(f'echo "@@ {n} {i} $("{e}" bench "{file}" --from {i} --to {i+1} --window 0,1 {o} --quiet | tail -1)"\n')
        lanefiles.append(lf)
    main = f"{d}/{tag}_all.sh"
    with open(main, "w", newline="\n") as fh:
        for k, lf in enumerate(lanefiles):
            fh.write(f'sh "{lf}" > "{lf}.out" &\n')
        fh.write("wait\n")
    subprocess.run(["sh", "tools/locked.sh", "sh", main], cwd=MAIN)
    with open(out, "a", newline="\n") as fo:
        for lf in lanefiles:
            for l in open(lf + ".out"):
                if not l.startswith("@@ "):
                    continue
                f = l.split()
                w = f[3:]
                try:
                    r = {"name": f[1], "pos": int(f[2]), "nodes": int(w[w.index("nodes") + 1]), "wrong": int(w[w.index("wrong") + 1]) + int(w[w.index("unknown") + 1])}
                except Exception:
                    continue
                fo.write(json.dumps(r) + "\n")

def totals(out):
    t = {}
    for l in open(out):
        r = json.loads(l)
        t.setdefault(r["name"], {})[r["pos"]] = r
    return t

if __name__ == "__main__":
    exe, file, a, b, lanes, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]), sys.argv[6]
    arms = [tuple(s.split("=", 1)) for s in sys.argv[7:]]
    run(exe, file, a, b, lanes, out, arms)
    t = totals(out)
    for n, _ in arms:
        rs = t.get(n, {})
        print(n, len(rs), sum(r["nodes"] for r in rs.values()), "wrong", sum(r["wrong"] for r in rs.values()))
