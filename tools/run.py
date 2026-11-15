"""The experiment runner: candidate against the frozen base on the same positions.

    python tools/run.py KEY KIND --set ply07 [--cand EXE] [--cand-args "..."] [--base-args "..."]
                        [--units N] [--h1 0.05] [--cap 100] [--start U] --hypothesis "..."

KIND is one of
    aa       base against itself on the tune range
    known    a planted change on the tune range (give --cand-args or --base-args)
    tune     a screen on the tune range; decides nothing; may be repeated freely
    gate     the one deciding test: a sequential test on the next unused units of the gate range
    confirm  a fixed-length read on the next unused units of the confirm range
    heldout  a fixed-length read on the held-out range (rare; logged)
    freeze   make the candidate the new base (after a passed gate)

A unit is GROUP consecutive positions of the set (10 by default), each solved from an empty table by
base and by candidate; its value is log(base seconds / candidate seconds) over the unit's positions.
Every position's value is checked against its label first: a wrong value blocks the run.
Results go to runs/KEY.jsonl (one unit per line) and one record to runs.jsonl.
"""
import argparse
import datetime
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SETS = os.path.join(ROOT, "tests", "set1")
LEDGER = os.path.join(ROOT, "tests", "ledger.jsonl")
RUNS = os.path.join(ROOT, "runs")
RUNLOG = os.path.join(ROOT, "runs.jsonl")
FROZEN = os.path.join(ROOT, "frozen")
BASE = os.path.join(FROZEN, "base.exe")
CAND = os.path.join(ROOT, "target", "release", "snek.exe")
HC = os.path.join(ROOT, "tools", "hc.py")
GROUP = 10
# shares of each set, in units: tune, gate, confirm, held-out
SHARES = (0.15, 0.60, 0.15, 0.10)


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()[:12]


def set_path(name):
    return os.path.join(SETS, name + ".txt")


def ranges(name):
    """Unit ranges [a, b) of each kind for a set."""
    n = sum(1 for l in open(set_path(name)) if l.strip() and not l.startswith("#")) // GROUP
    cuts = [0]
    for s in SHARES[:-1]:
        cuts.append(cuts[-1] + int(n * s))
    cuts.append(n)
    return {"tune": (cuts[0], cuts[1]), "gate": (cuts[1], cuts[2]), "confirm": (cuts[2], cuts[3]), "heldout": (cuts[3], cuts[4])}


def ledger():
    if not os.path.exists(LEDGER):
        return []
    return [json.loads(l) for l in open(LEDGER) if l.strip()]


def next_unused(name, kind):
    a, b = ranges(name)[kind]
    used = [e["to"] for e in ledger() if e["set"] == name and e["kind"] == kind]
    return max([a] + used), b


def solve_units(exe, args, name, u0, u1):
    """Runs one arm on units [u0, u1); returns per-unit (seconds, nodes) and whether every value was right."""
    cmd = [exe, "bench", set_path(name), "--from", str(u0 * GROUP), "--to", str(u1 * GROUP)] + args
    out = subprocess.run(cmd, capture_output=True, text=True)
    units = {}
    ok = out.returncode == 0
    for line in out.stdout.splitlines():
        if line.startswith("#") or not line.strip():
            continue
        i, ply, expected, value, nodes, us, verdict = line.split()
        if verdict != "ok":
            ok = False
        u = int(i) // GROUP
        s, n = units.get(u, (0.0, 0))
        units[u] = (s + max(int(us), 1) / 1e6, n + int(nodes))
    if len(units) != u1 - u0:
        ok = False
    return units, ok, out.stdout[-400:] + out.stderr[-400:]


def hc(*args):
    out = subprocess.run([sys.executable, "-I", HC] + list(args), capture_output=True, text=True)
    return out.returncode, out.stdout.strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("key")
    ap.add_argument("kind", choices=["aa", "known", "tune", "gate", "confirm", "heldout", "freeze"])
    ap.add_argument("--set", default="ply07")
    ap.add_argument("--cand", default=CAND)
    ap.add_argument("--cand-args", default="")
    ap.add_argument("--base-args", default="")
    ap.add_argument("--units", type=int, default=30)
    ap.add_argument("--start", type=int, default=0, help="first unit within the tune range")
    ap.add_argument("--h1", type=float, default=0.05)
    ap.add_argument("--h0", type=float, default=0.0)
    ap.add_argument("--cap", type=int, default=100)
    ap.add_argument("--chunk", type=int, default=10, help="units per interleaved chunk")
    ap.add_argument("--hypothesis", default="")
    a = ap.parse_args()
    os.makedirs(RUNS, exist_ok=True)
    os.makedirs(FROZEN, exist_ok=True)

    if a.kind == "freeze":
        shutil.copyfile(a.cand, BASE)
        git = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
        dirty = subprocess.run(["git", "-C", ROOT, "status", "--porcelain", "--", "src", "Cargo.toml"], capture_output=True, text=True).stdout.strip()
        rec = {"time": now(), "key": a.key, "kind": "freeze", "base": sha(BASE), "commit": git + ("+dirty" if dirty else ""), "note": a.hypothesis}
        shutil.copyfile(BASE, os.path.join(FROZEN, f"{a.key}-{rec['base']}.exe"))
        with open(os.path.join(FROZEN, "BASE.txt"), "w") as f:
            f.write(json.dumps(rec) + "\n")
        append(RUNLOG, rec)
        print("frozen", rec)
        return

    out_path = os.path.join(RUNS, a.key + ".jsonl")
    if os.path.exists(out_path):
        sys.exit(f"key {a.key} was already used; a run is not repeated under its key")
    if a.kind in ("gate", "confirm", "heldout") and not a.hypothesis:
        sys.exit("a deciding read needs --hypothesis")
    base_exe = BASE
    cand_exe = BASE if a.kind == "aa" else a.cand
    base_args, cand_args = a.base_args.split(), a.cand_args.split()

    if a.kind in ("aa", "known", "tune"):
        lo, hi = ranges(a.set)["tune"]
        u0, limit = lo + a.start, hi
        total = min(a.units, limit - u0)
    else:
        u0, limit = next_unused(a.set, a.kind)
        total = min(a.cap if a.kind == "gate" else a.units, limit - u0)
    if total <= 0:
        sys.exit(f"no unused {a.kind} units left in {a.set}")

    rows, blocked, verdict, sprt_text = [], None, None, ""
    done = 0
    flip = False
    while done < total:
        n = min(a.chunk, total - done)
        c0, c1 = u0 + done, u0 + done + n
        arms = [("base", base_exe, base_args), ("cand", cand_exe, cand_args)]
        if flip:
            arms.reverse()
        flip = not flip
        res = {}
        for label, exe, args in arms:
            units, ok, tail = solve_units(exe, args, a.set, c0, c1)
            if not ok:
                blocked = f"{label} gave a wrong or missing value in units [{c0}, {c1}): {tail!r}"
            res[label] = units
        if blocked:
            break
        with open(out_path, "a") as f:
            for u in range(c0, c1):
                row = {"unit": u, "base": res["base"][u][0], "cand": res["cand"][u][0], "base_nodes": res["base"][u][1], "cand_nodes": res["cand"][u][1]}
                rows.append(row)
                f.write(json.dumps(row) + "\n")
        done += n
        if a.kind == "gate":
            code, sprt_text = hc("sprt", out_path, "--mode", "metric", "--ratio", "--lower-is-better", "--h0", str(a.h0), "--h1", str(a.h1), "--max-units", str(total))
            if code in (0, 1):
                verdict = "pass" if code == 0 else "fail"
                break
    used_to = u0 + done

    rec = {"time": now(), "key": a.key, "kind": a.kind, "set": a.set, "units": [u0, used_to], "group": GROUP, "hypothesis": a.hypothesis,
           "base": sha(base_exe), "cand": sha(cand_exe), "base_args": a.base_args, "cand_args": a.cand_args}
    if a.kind in ("gate", "confirm", "heldout"):
        append(LEDGER, {"time": now(), "key": a.key, "set": a.set, "kind": a.kind, "from": u0, "to": used_to})
    if blocked:
        rec["verdict"] = "blocked"
        rec["reason"] = blocked
        append(RUNLOG, rec)
        print("BLOCKED:", blocked)
        sys.exit(2)

    tb, tc = sum(r["base"] for r in rows), sum(r["cand"] for r in rows)
    nb, nc = sum(r["base_nodes"] for r in rows), sum(r["cand_nodes"] for r in rows)
    _, report = hc("report", out_path, "--mode", "metric", "--ratio", "--lower-is-better")
    rec.update({"n": len(rows), "base_seconds": round(tb, 4), "cand_seconds": round(tc, 4), "total_time_ratio": round(tc / tb, 4),
                "base_nodes": nb, "cand_nodes": nc, "node_ratio": round(nc / nb, 4) if nb else None, "report": report})
    if a.kind == "gate":
        rec["test"] = f"SPRT[{a.h0}, {a.h1}] on log(base/cand) seconds per unit, alpha = beta = 0.05, cap {total} units"
        rec["sprt"] = sprt_text
        if verdict is None:
            verdict = "undecided"
        # the guard: a pass must not cost total time
        if verdict == "pass" and tc > tb:
            verdict = "fail"
            rec["guard"] = "passed per unit but total time is higher"
        rec["verdict"] = verdict
    elif a.kind == "aa":
        ratio = math.log(tb / tc)
        rec["verdict"] = "pass" if nb == nc and abs(ratio) < 0.03 else "fail"
    append(RUNLOG, rec)
    print(f"{a.key} {a.kind} {a.set} units [{u0}, {used_to})  base {sha(base_exe)}  cand {sha(cand_exe)} {a.cand_args}")
    print(report)
    print(f"total time   base {tb:.3f} s  cand {tc:.3f} s  ratio {tc / tb:.4f}   nodes base {nb}  cand {nc}  ratio {nc / nb if nb else 0:.4f}")
    if a.kind == "gate":
        print(sprt_text)
    if "verdict" in rec:
        print("VERDICT:", rec["verdict"], rec.get("guard", ""))


def now():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")


def append(path, rec):
    with open(path, "a") as f:
        f.write(json.dumps(rec) + "\n")


if __name__ == "__main__":
    main()
