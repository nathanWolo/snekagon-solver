"""A fixed-length read of several versions on the same unused confirm units.

    python tools/ladder.py KEY --set ply07 --units 30 NAME=EXE[:ARGS] [NAME=EXE[:ARGS] ...]

Every version solves the same positions twice, in alternating order; the table gives seconds, nodes
and, against the last version listed, the per-unit ratio with its 95% interval (hc.py). The units are
taken from the confirm range and written to the ledger, so no later read can use them again.
"""
import argparse
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("key")
    ap.add_argument("--set", default="ply07")
    ap.add_argument("--units", type=int, default=30)
    ap.add_argument("versions", nargs="+")
    a = ap.parse_args()
    out_path = os.path.join(run.RUNS, a.key + ".json")
    if os.path.exists(out_path):
        sys.exit(f"key {a.key} was already used")
    u0, limit = run.next_unused(a.set, "confirm")
    u1 = min(u0 + a.units, limit)
    if u1 <= u0:
        sys.exit("no unused confirm units left")
    versions = []
    for v in a.versions:
        name, rest = v.split("=", 1)
        exe, _, args = rest.partition(":")
        versions.append((name, os.path.abspath(exe), args.split()))
    totals = {name: {} for name, _, _ in versions}
    for rep in range(2):
        order = versions if rep == 0 else versions[::-1]
        for name, exe, args in order:
            units, ok, tail = run.solve_units(exe, args, a.set, u0, u1)
            if not ok:
                sys.exit(f"BLOCKED: {name} gave a wrong or missing value: {tail!r}")
            for u, (s, n) in units.items():
                ps, _ = totals[name].get(u, (0.0, 0))
                totals[name][u] = (ps + s / 2, n)
    run.append(run.LEDGER, {"time": run.now(), "key": a.key, "set": a.set, "kind": "confirm", "from": u0, "to": u1})
    last = versions[-1][0]
    rec = {"time": run.now(), "key": a.key, "kind": "ladder", "set": a.set, "units": [u0, u1], "versions": {}}
    print(f"{a.key}: {a.set} confirm units [{u0}, {u1}), {(u1 - u0) * run.GROUP} positions, mean of two passes")
    print(f"{'version':10s} {'seconds':>9s} {'nodes':>13s} {'time vs ' + last:>14s}   per-unit ratio against {last} (95%)")
    for name, exe, args in versions:
        secs = sum(s for s, _ in totals[name].values())
        nodes = sum(n for _, n in totals[name].values())
        ref = sum(s for s, _ in totals[last].values())
        tmp = os.path.join(run.RUNS, f"{a.key}-{name}.jsonl")
        with open(tmp, "w") as f:
            for u in range(u0, u1):
                # "cand" is the last version, "base" this one: the ratio says how much faster the last is
                f.write(json.dumps({"unit": u, "base": totals[name][u][0], "cand": totals[last][u][0]}) + "\n")
        _, report = run.hc("report", tmp, "--mode", "metric", "--ratio", "--lower-is-better")
        line = next((l for l in report.splitlines() if l.startswith("ratio")), "")
        print(f"{name:10s} {secs:9.3f} {nodes:13,d} {secs / ref:13.2f}x   {line}")
        rec["versions"][name] = {"exe": run.sha(exe), "args": " ".join(args), "seconds": round(secs, 4), "nodes": nodes, "report": line}
    with open(out_path, "w") as f:
        json.dump(rec, f, indent=1)
    run.append(run.RUNLOG, rec)


if __name__ == "__main__":
    main()
