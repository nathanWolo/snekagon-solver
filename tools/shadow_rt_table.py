"""Summarises the shadow-check runs of tools/shadow_rt_runs.sh: python tools/shadow_rt_table.py OUTDIR"""
import re
import sys
from pathlib import Path

out = Path(sys.argv[1])
keys = {
    "fired": "nodes where a reach threat ruled out a move",
    "pruned": "moves ruled out with a reach bound <= alpha",
    "checked": "ruled-out moves checked (exact solve)",
    "skipped": "ruled-out moves skipped by the sampling",
    "nodechk": "node checks (value returned, bounds stored)",
    "noreach": "no-reach answers checked (unlimited work)",
    "cache": "of them from the cache",
}
print("| run | positions | nodes | wrong | nodes w/ reach prune | moves ruled out | checked | only-by-reach checked | node checks | no-reach checks (cache) | mismatches | exit |")
print("|---|---|---|---|---|---|---|---|---|---|---|---|")
prog = (out / "progress.txt").read_text() if (out / "progress.txt").exists() else ""
for f in sorted(out.glob("*.txt")):
    if f.name == "progress.txt":
        continue
    s = f.read_text(errors="replace")
    m = re.search(r"# positions (\d+) nodes (\d+) seconds [\d.]+ wrong (\d+)", s)
    vals = {}
    for k, name in keys.items():
        mm = re.search(r"#\s+" + re.escape(name) + r"\s+(\d+)", s)
        vals[k] = mm.group(1) if mm else "?"
    mm = re.search(r"#\s+of them not ruled out without reach threats\s+\d+\n#\s+ruled-out moves checked \(exact solve\)\s+\d+\n#\s+of them not ruled out without reach threats\s+(\d+)", s)
    only = mm.group(1) if mm else "?"
    mis = s.count("SHADOW MISMATCH")
    e = re.search(r"== " + re.escape(f.stem) + r" exit (\d+)", prog)
    print(f"| {f.stem} | {m.group(1) if m else '-'} | {m.group(2) if m else '-'} | {m.group(3) if m else '-'} | {vals['fired']} | {vals['pruned']} | {vals['checked']} | {only} | {vals['nodechk']} | {vals['noreach']} ({vals['cache']}) | {mis} | {e.group(1) if e else '?'} |")
