"""A paired read of the proof driver: candidate against base on whole proof jobs, by wall-clock.

    python tools/provegate.py KEY --base EXE --cand EXE --base-args "..." --cand-args "..."
                              --jobs g h i j [--dir work/e24] [--tt 27] --hypothesis "..."

A job is a file DIR/<name>.pos with one position ("? <position>"). Each job is solved from nothing by
the base and by the candidate (the order alternates from job to job), every solve under the machine's
run lock, each into a fresh results file. A unit is one job: log(base seconds / candidate seconds),
the seconds being the driver's own ("# positions .. seconds ..").

A solve that reports a wrong value, or two solves of a job that disagree on the value, block the run.
Units go to runs/KEY.jsonl as they finish, one record to runs.jsonl at the end. The read is of fixed
length: the jobs are named before it starts and all of them are run.
"""
import argparse
import datetime
import hashlib
import json
import math
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = os.path.join(ROOT, "runs")
RUNLOG = os.path.join(ROOT, "runs.jsonl")
HC = os.path.join(ROOT, "tools", "hc.py")
LOCKED = os.path.join(ROOT, "tools", "locked.sh")


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()[:12]


def solve(exe, args, job_file, tt, res):
    if os.path.exists(res):
        os.remove(res)
    cmd = ["sh", LOCKED, exe, "prove", job_file] + args + ["--tt", str(tt), "--out", res]
    out = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    m = re.search(r"# positions (\d+) nodes (\d+) seconds ([0-9.]+) wrong (\d+) unknown (\d+)", out.stdout)
    if out.returncode != 0 or not m:
        sys.exit("the solve failed: %s\n%s\n%s" % (" ".join(cmd), out.stdout[-2000:], out.stderr[-2000:]))
    if int(m.group(4)) != 0:
        sys.exit("WRONG VALUE: %s\n%s" % (" ".join(cmd), out.stdout))
    first = out.stdout.split("\n")[0].split()
    value = first[3] if len(first) > 3 else "?"
    t = re.search(r"thread-seconds ([0-9.]+)", out.stdout)
    return {"seconds": float(m.group(3)), "nodes": int(m.group(2)), "value": value,
            "thread_seconds": float(t.group(1)) if t else None}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("key")
    ap.add_argument("--base", required=True)
    ap.add_argument("--cand", required=True)
    ap.add_argument("--base-args", default="")
    ap.add_argument("--cand-args", default="")
    ap.add_argument("--jobs", nargs="+", required=True)
    ap.add_argument("--dir", default=os.path.join(ROOT, "work", "e24"))
    ap.add_argument("--tt", type=int, default=27)
    ap.add_argument("--hypothesis", required=True)
    a = ap.parse_args()
    os.makedirs(RUNS, exist_ok=True)
    out_path = os.path.join(RUNS, a.key + ".jsonl")
    if os.path.exists(out_path):
        sys.exit("the key %s has been used: %s exists" % (a.key, out_path))
    scratch = os.path.join(ROOT, "work", "provegate", a.key)
    os.makedirs(scratch, exist_ok=True)
    arms = {"base": (a.base, a.base_args.split()), "cand": (a.cand, a.cand_args.split())}
    units = []
    for i, job in enumerate(a.jobs):
        job_file = os.path.join(a.dir, job + ".pos")
        got = {}
        for arm in (("base", "cand") if i % 2 == 0 else ("cand", "base")):
            exe, args = arms[arm]
            got[arm] = solve(exe, args, job_file, a.tt, os.path.join(scratch, "%s-%s.res" % (job, arm)))
        if got["base"]["value"] != got["cand"]["value"]:
            sys.exit("the two solves of job %s disagree: %s" % (job, got))
        rec = {"job": job, "base": got["base"]["seconds"], "cand": got["cand"]["seconds"],
               "base_nodes": got["base"]["nodes"], "cand_nodes": got["cand"]["nodes"],
               "base_thread_seconds": got["base"]["thread_seconds"], "cand_thread_seconds": got["cand"]["thread_seconds"],
               "value": got["base"]["value"]}
        units.append(rec)
        with open(out_path, "a", newline="\n") as f:
            f.write(json.dumps(rec) + "\n")
        print("%s  base %.1f s  cand %.1f s  ratio %.3f" % (job, rec["base"], rec["cand"], rec["cand"] / rec["base"]), flush=True)
    tb = sum(u["base"] for u in units)
    tc = sum(u["cand"] for u in units)
    logs = [math.log(u["cand"] / u["base"]) for u in units]
    n = len(logs)
    mean = sum(logs) / n
    sd = math.sqrt(sum((x - mean) ** 2 for x in logs) / (n - 1)) if n > 1 else float("nan")
    print("total: base %.1f s, cand %.1f s, ratio of totals %.3f; mean log ratio %+.4f, sd %.4f over %d jobs" % (tb, tc, tc / tb, mean, sd, n))
    rep = subprocess.run([sys.executable, HC, "report", out_path, "--ratio", "--lower-is-better"], capture_output=True, text=True)
    print(rep.stdout.strip() or rep.stderr.strip())
    rec = {"key": a.key, "kind": "provegate", "when": datetime.datetime.now().isoformat(timespec="seconds"),
           "hypothesis": a.hypothesis, "base": a.base, "base_sha": sha(a.base), "base_args": a.base_args,
           "cand": a.cand, "cand_sha": sha(a.cand), "cand_args": a.cand_args, "jobs": a.jobs,
           "base_seconds": tb, "cand_seconds": tc, "ratio_of_totals": tc / tb, "mean_log_ratio": mean, "sd": sd,
           "report": rep.stdout.strip()}
    with open(RUNLOG, "a", newline="\n") as f:
        f.write(json.dumps(rec) + "\n")


if __name__ == "__main__":
    main()
