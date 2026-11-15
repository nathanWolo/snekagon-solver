"""Where does the centre-opening run spend its time: on red tries that win, or on tries that fail?

    python tools/jobsplit.py work/long/ply1_results.txt

Reads the jobs of the run (positions after 1:60, a blue reply and a red try, blue to move).
A job with hi <= -1 is a red try that wins; one with lo >= 0 is a try that failed (blue holds).
"""
import statistics
import sys

SEED = (16, 27, 28, 39)


def cells(h):
    v = int(h, 16)
    return tuple(i for i in range(121) if v >> i & 1)


def q(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p * len(xs)))]


won, failed, other = {}, {}, 0
seen = {}
for line in open(sys.argv[1]):
    f = line.split()
    if len(f) < 6:
        continue
    pos = f[0].split("_")
    if len(pos) != 7:
        continue
    red, blue, _, _, stm, ply, _ = pos
    if ply != "3" or stm != "1" or 60 not in cells(red):
        continue
    seen[f[0]] = seen.get(f[0], 0) + 1
    lo, hi, nodes, ms = int(f[1]), int(f[2]), int(f[3]), int(f[4])
    key = cells(blue)
    if hi <= -1:
        won.setdefault(key, []).append((ms / 1000, nodes))
    elif lo >= 0:
        failed.setdefault(key, []).append((ms / 1000, nodes))
    else:
        other += 1

dups = sum(1 for n in seen.values() if n > 1)
print(f"jobs: {sum(seen.values())} records, {len(seen)} positions, {dups} positions recorded twice, {other} with open bounds")

for name, keys in (("this run (seeded reply left out)", [k for k in won if k != SEED]), ("seeded reply", [k for k in won if k == SEED])):
    w = [t for k in keys for t, n in won[k]]
    fl = [t for k in keys for t, n in failed.get(k, [])]
    if not w:
        continue
    tot = sum(w) + sum(fl)
    print(f"\n{name}: {len(keys)} replies refuted")
    print(f"  winning tries: {len(w):4d} jobs, {sum(w):8.0f} s ({100 * sum(w) / tot:.0f}%), median {statistics.median(w):.0f} s, mean {statistics.mean(w):.0f} s, "
          f"quartiles {q(w, .25):.0f}/{q(w, .75):.0f} s, max {max(w):.0f} s")
    if fl:
        print(f"  failed tries:  {len(fl):4d} jobs, {sum(fl):8.0f} s ({100 * sum(fl) / tot:.0f}%), median {statistics.median(fl):.0f} s, mean {statistics.mean(fl):.0f} s, "
              f"quartiles {q(fl, .25):.0f}/{q(fl, .75):.0f} s, max {max(fl):.0f} s")
        for lim in (10, 60, 300):
            n = sum(1 for t in fl if t > lim)
            print(f"    failed tries over {lim:3d} s: {n:3d} jobs, {sum(t for t in fl if t > lim):7.0f} s")
    clean = [k for k in keys if not failed.get(k)]
    print(f"  replies refuted by the first try: {len(clean)} of {len(keys)}")
    worst = sorted(keys, key=lambda k: -sum(t for t, n in failed.get(k, [])))[:6]
    for k in worst:
        fs = [t for t, n in failed.get(k, [])]
        ws = [t for t, n in won[k]]
        print(f"    blue {','.join(map(str, k)):14s} failed tries {len(fs):3d}, {sum(fs):6.0f} s (largest {max(fs) if fs else 0:5.0f} s); winning try {sum(ws):5.0f} s")

open_keys = [k for k in failed if k not in won]
for k in open_keys:
    fs = [t for t, n in failed[k]]
    print(f"\nopen reply blue {','.join(map(str, k))}: {len(fs)} failed tries so far, {sum(fs):.0f} s")
