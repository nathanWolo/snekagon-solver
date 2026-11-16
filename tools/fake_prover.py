"""A stand-in for the proof driver in the board's tests (tools/gui_test.py): `fake_prover.py prove ROOTS ... --out RESULTS`.

It plays back what the real driver recorded for each root in a results file (FAKE_RECORD, by default the
replay's work/replay/base4/results.txt): the root's tries (lines at the root's ply + 1) and jobs (ply + 2),
appended to RESULTS one line every FAKE_DELAY seconds, with the driver's progress lines, then the root's own
line and its result line. A root the results file already decides is decided at once, a line RESULTS already
holds is not written again (as the driver goes on from its file). A root without a record ends it with exit 2.
"""
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gui  # noqa: E402


def main():
    a = sys.argv[1:]
    roots_path, out = a[1], a[a.index("--out") + 1]
    rec_path = os.environ.get("FAKE_RECORD") or str(gui.ROOT / "work" / "replay" / "base4" / "results.txt")
    delay = float(os.environ.get("FAKE_DELAY", "0.003"))
    roots = [l.split()[1] for l in open(roots_path) if l.strip() and not l.startswith("#")]
    known, _ = gui.read_results(out)
    present = set(l.rstrip("\n") for l in open(out)) if os.path.exists(out) else set()
    rec = [l.rstrip("\n") for l in open(rec_path)]
    t0 = time.time()
    print("# %d positions read from %s" % (len(known), out), flush=True)
    f = open(out, "a")
    for i, root in enumerate(roots):
        k = known.get(root)
        if k and (k[0] >= 1 or k[1] <= 0):
            v = 1 if k[0] >= 1 else 0
            print("# %8.1f s  window (0,1) gave %d: the value lies in [%d, %d]" % (time.time() - t0, v, k[0], k[1]), flush=True)
            print("%d 2 ? %d 0 100 ok 0 0 1 0.0 %s 0.0" % (i, v, k[2] or "pass"), flush=True)
            continue
        r = gui.parse_text(root)
        opp, me = r["stm"], r["stm"] ^ 1
        ro, rm = r["cells"][opp], r["cells"][me]
        seg, last = [], None
        for l in rec:
            t = l.split()[0].split("_")
            ply = int(t[5])
            a_, b_ = int(t[opp], 16), int(t[me], 16)
            if a_ & ro != ro or a_ == ro:
                continue
            if (ply == r["ply"] + 1 and b_ == rm) or (ply == r["ply"] + 2 and b_ & rm == rm and b_ != rm):
                seg.append(l)
        last = [l for l in rec if l.split()[0] == root and int(l.split()[1]) >= 1]
        if not last:
            sys.exit(2)
        tries = jobs = 0
        for l in seg:
            fs = l.split()
            if l not in present:
                f.write(l + "\n")
                f.flush()
                time.sleep(delay)
            if int(fs[0].split("_")[5]) == r["ply"] + 1:
                tries += 1
                v = 1 if int(fs[2]) <= -1 else 0
                mv = gui.move_between(r, gui.parse_text(fs[0]))
                print("# %8.1f s  root move %d of 100  %s  value %d  best so far %d" % (time.time() - t0, tries, mv, v, v), flush=True)
            else:
                jobs += 1
        if last[0] not in present:
            f.write(last[0] + "\n")
            f.flush()
        print("# %8.1f s  window (0,1) gave 1: the value lies in [1, 1]" % (time.time() - t0), flush=True)
        print("%d 2 ? 1 1000 %d ok %d 0 0 1.0 %s 0.0" % (i, int((time.time() - t0) * 1e6), jobs, last[0].split()[5]), flush=True)
    print("# positions %d nodes 0 seconds 0.0 wrong 0 unknown 0" % len(roots), flush=True)


if __name__ == "__main__":
    main()
