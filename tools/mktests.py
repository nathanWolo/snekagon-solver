"""Builds the frozen test set from the reference repository's published data.

Usage: mktests.py REF_REPO OUT_DIR

Reads the labelled positions (data/labels_*.csv: position text plus the exact value for the side to
move) and the benchmark move lists (bench/ply5_*.txt). Writes, under OUT_DIR:

  labels.txt       every labelled position once, as "<value> <position>", in a pinned shuffled order
  plyNN.txt        the positions of labels.txt after NN moves, in the same order
  bench_<name>.txt the reference's ply-5 benchmark units, as "<value or ?> <moves>"
  MANIFEST.txt     counts and SHA-256 of each file

A position that appears with two different values is dropped and reported. The reference data is not
copied into this repository; this script rebuilds the set from a checkout of
github.com/BoltKey/snekagon-lab at commit 61eb3a4.
"""
import csv
import hashlib
import os
import random
import sys

ref, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)

LABEL_FILES = ["labels_good_v4.csv", "labels_noisy_v4.csv", "labels_good_v3.csv", "labels_good_ply5.csv",
               "labels_good_v4_head1.csv", "labels_noisy_v4_head1.csv"]
seen = {}
conflicts = set()
per_file = {}
for name in LABEL_FILES:
    path = os.path.join(ref, "data", name)
    n = 0
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            if "pos" not in row or not row["pos"]:
                continue
            pos, value = row["pos"], int(row["value"])
            if pos in seen and seen[pos][0] != value:
                conflicts.add(pos)
            if pos not in seen:
                seen[pos] = (value, name)
                n += 1
    per_file[name] = n

rows = [(v, pos, src) for pos, (v, src) in seen.items() if pos not in conflicts]
rows.sort(key=lambda r: r[1])
random.Random(20261114).shuffle(rows)


def write(name, lines):
    path = os.path.join(out, name)
    with open(path, "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


manifest = []
h = write("labels.txt", [f"{v} {pos}" for v, pos, _ in rows])
by_ply = {}
for v, pos, _ in rows:
    ply = int(pos.split("_")[5])
    by_ply.setdefault(ply, [0, 0, 0])[v + 1] += 1
manifest.append(f"labels.txt {len(rows)} positions sha256 {h}")
for ply in sorted(by_ply):
    l, d, w = by_ply[ply]
    manifest.append(f"  ply {ply}: {l + d + w} positions ({w} wins, {d} draws, {l} losses for the side to move)")
for ply in sorted(by_ply):
    sub = [f"{v} {pos}" for v, pos, _ in rows if int(pos.split("_")[5]) == ply]
    h = write(f"ply{ply:02d}.txt", sub)
    manifest.append(f"ply{ply:02d}.txt {len(sub)} positions sha256 {h}")
manifest.append(f"  new positions per source file: {per_file}")
manifest.append(f"  dropped for conflicting values: {len(conflicts)}")

BENCH = {"ply5_blue_wins": "1", "ply5_blue_wins_holdout": "1", "ply5_random_units": "1", "ply5_other_openings": "1",
         "ply5_ublock_refutations": "?"}
for name, value in BENCH.items():
    lines = [l.strip() for l in open(os.path.join(ref, "bench", name + ".txt")) if l.strip() and not l.startswith("#")]
    h = write(f"bench_{name}.txt", [f"{value} {l}" for l in lines])
    manifest.append(f"bench_{name}.txt {len(lines)} positions sha256 {h}")

with open(os.path.join(out, "MANIFEST.txt"), "w", newline="\n") as f:
    f.write("\n".join(manifest) + "\n")
print("\n".join(manifest))
