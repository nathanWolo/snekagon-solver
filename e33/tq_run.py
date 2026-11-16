"""E33 runner: each job is one locked bench run; output saved to work/tightq/OUT.

usage: python tq_run.py JOBFILE
JOBFILE lines: OUT|EXE|ARGS...  (ARGS after 'bench', split on spaces; FILE paths relative to main)
"""
import subprocess, sys, os, time
MAIN = r"<session-folder>/snekagon"
OUTD = MAIN + "/work/tightq"
os.makedirs(OUTD, exist_ok=True)
for line in open(sys.argv[1]):
    line = line.strip()
    if not line or line.startswith('#'):
        continue
    out, exe, args = line.split('|')
    if os.path.exists(OUTD + '/' + out):
        continue
    cmd = ['sh', 'tools/locked.sh', exe, 'bench'] + args.split()
    t0 = time.time()
    r = subprocess.run(cmd, cwd=MAIN, capture_output=True, text=True)
    open(OUTD + '/' + out + '.tmp', 'w').write(r.stdout + ('\n#ERR ' + r.stderr if r.stderr.strip() else ''))
    os.replace(OUTD + '/' + out + '.tmp', OUTD + '/' + out)
    tail = [l for l in r.stdout.splitlines() if l.startswith('# positions')]
    print(out, r.returncode, round(time.time() - t0, 1), tail[-1] if tail else r.stderr[-300:], flush=True)
