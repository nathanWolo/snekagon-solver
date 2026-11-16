"""Per-position paired runs under the run lock.

python yard.py OUT.jsonl FILE FROM_POS TO_POS CHUNK ARM=EXE[::opts] ...

Each position is solved in its own process (fresh table) by every arm; positions are taken CHUNK at
a time, one lock hold per chunk, with the order of the arms reversed every other chunk. Records:
{"arm", "pos", "nodes", "s", "wrong", "unknown"} one a line. Resumes: positions already in OUT for
every arm are skipped.
"""
import json, os, subprocess, sys

MAIN = "<session-folder>/snekagon"
out, file, a, b, chunk = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
arms = []
for s in sys.argv[6:]:
    name, rest = s.split("=", 1)
    exe, _, opts = rest.partition("::")
    arms.append((name, exe, opts))
done = set()
if os.path.exists(out):
    for l in open(out):
        r = json.loads(l)
        done.add((r["arm"], r["pos"]))
tmpdir = os.path.dirname(os.path.abspath(out))
k = 0
for c0 in range(a, b, chunk):
    c1 = min(b, c0 + chunk)
    order = arms if k % 2 == 0 else arms[::-1]
    k += 1
    lines = []
    for name, exe, opts in order:
        for i in range(c0, c1):
            if (name, i) in done:
                continue
            lines.append(f'echo "@@ {name} {i} $("{exe}" bench "{file}" --from {i} --to {i+1} {opts} --quiet | tail -1)"')
    if not lines:
        continue
    sh = os.path.join(tmpdir, f"chunk_{os.getpid()}.sh")
    with open(sh, "w", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    res = subprocess.run(["sh", "tools/locked.sh", "sh", sh], cwd=MAIN, capture_output=True, text=True)
    with open(out, "a", newline="\n") as fo:
        for l in res.stdout.splitlines():
            if not l.startswith("@@ "):
                continue
            f = l.split()
            name, i = f[1], int(f[2])
            w = f[3:]
            try:
                r = {"arm": name, "pos": i, "nodes": int(w[w.index("nodes") + 1]), "s": float(w[w.index("seconds") + 1]),
                     "wrong": int(w[w.index("wrong") + 1]), "unknown": int(w[w.index("unknown") + 1])}
            except Exception:
                print("bad line", l, file=sys.stderr)
                continue
            fo.write(json.dumps(r) + "\n")
            if r["wrong"] or r["unknown"]:
                print("WRONG", r, file=sys.stderr)
    print(f"chunk {c0}-{c1} done", flush=True)
