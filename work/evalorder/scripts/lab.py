"""Label roots with `evalmoves` (every move solved alone), LANES processes at a time inside one lock hold.

python lab.py EXE ROOTS.txt OUTDIR FROM TO LANES SECONDS CAP

Each lock hold runs up to LANES roots in parallel under `timeout SECONDS`; a root's CSV is kept only when
its process finished (marker line '#end'). Roots already done are skipped.
"""
import os, subprocess, sys

MAIN = "<session-folder>/snekagon"
exe, roots, out, a, b, lanes, secs, cap = sys.argv[1], sys.argv[2], sys.argv[3], *map(int, sys.argv[4:9])
os.makedirs(out, exist_ok=True)
def done(i):
    p = f"{out}/r{i}.csv"
    return os.path.exists(p) and open(p).read().rstrip().endswith("#end")
todo = [i for i in range(a, b) if not done(i)]
tried = set()
while todo:
    batch = [i for i in todo if i not in tried][:lanes]
    if not batch:
        break
    tried.update(batch)
    sh = f"{out}/batch.sh"
    with open(sh, "w", newline="\n") as fh:
        for i in batch:
            fh.write(f'(timeout {secs} "{exe}" evalmoves "{roots}" --from {i} --to {i+1} --nodes {cap} > "{out}/r{i}.csv" && echo "#end" >> "{out}/r{i}.csv") &\n')
        fh.write("wait\n")
    subprocess.run(["sh", "tools/locked.sh", "sh", sh], cwd=MAIN)
    print("batch", batch, "finished", [i for i in batch if done(i)], flush=True)
    todo = [i for i in todo if not done(i)]
