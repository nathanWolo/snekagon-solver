"""Writes a shell script that labels selected nodes with `snek ordermoves` in parallel chunks.

    python tools/order/labeljob.py SCRIPT SECONDS PROCS CAP JOB...    JOB = SELFILE:OUTPREFIX:EXTRA_ARGS

Each job's nodes are split into PROCS*... chunks dealt round-robin over all processes; every process
runs its chunks one after another under `timeout SECONDS` in total and appends `#end` to a chunk's
CSV when that chunk finished (tools/order/sim.py drops the last node of a chunk without it).
"""
import sys

script, secs, procs, cap = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
chunks = []
for job in sys.argv[5:]:
    sel, prefix, extra = job.split(":")
    n = sum(1 for _ in open(sel))
    size = max(1, n // (procs * 4) + 1)
    for k, a in enumerate(range(0, n, size)):
        chunks.append(f"R=$((END-$(date +%s))); [ $R -gt 0 ] && timeout $R $E ordermoves {sel} --from {a} --to {a + size} --nodes {cap} {extra} > {prefix}_{k}.csv && echo '#end' >> {prefix}_{k}.csv")
lanes = [[] for _ in range(procs)]
for i, c in enumerate(chunks):
    lanes[i % procs].append(c)
with open(script, "w", newline="\n") as fh:
    fh.write("E=work/target/release/snek.exe\nexport END=$(( $(date +%s) + " + str(secs) + " ))\n")
    for j, lane in enumerate(lanes):
        if not lane:
            continue
        lf = f"work/order/lane{j}.sh"
        with open(lf, "w", newline="\n") as fl:
            fl.write("E=work/target/release/snek.exe\n" + "\n".join(lane) + "\n")
        fh.write(f"sh {lf} &\n")
    fh.write("wait\ndate\n")
print(len(chunks), "chunks")
