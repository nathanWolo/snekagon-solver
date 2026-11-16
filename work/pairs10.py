"""Pairs of the first N roots of two replay summaries (replay_sum.py tables), for hc.py report --ratio.

    python work/pairs10.py CAND_SUMMARY BASE_SUMMARY N wall|thread > pairs.jsonl
"""
import json
import sys


def rows(path):
    out = {}
    for l in open(path):
        f = l.split()
        if f and f[0].isdigit():
            out[int(f[0])] = dict(tries=int(f[2]), failed=int(f[3]), wall=float(f[4]), thread=float(f[6]), stopped=float(f[7]), best=f[8], ok=f[9])
    return out


c, b = rows(sys.argv[1]), rows(sys.argv[2])
n, metric = int(sys.argv[3]), sys.argv[4]
for i in range(n):
    print(json.dumps({"cand": c[i][metric], "base": b[i][metric]}))
