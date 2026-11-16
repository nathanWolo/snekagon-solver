"""Summary of a proof-order replay (tools/replay.sh): one line a root, totals, and the certificate check.

    python tools/replay_sum.py FILE DIR

Per root: red's tries (positions after a red move, blue to move, found in the results file), the
failed ones (blue holds at least a draw), wall-clock seconds (summed over pieces, with the seconds a
root lost when a piece was cut off), nodes, thread-seconds, and job seconds lost on stopped jobs.
Check, as tools/certificate.py does: the root is a red win, and the position after the winning move
(blue to move) is in the results file searched to a loss.
"""
import sys


def cells(h):
    v = int(h, 16)
    return [i for i in range(121) if v >> i & 1]


FILE, DIR = sys.argv[1], sys.argv[2]
roots = [l.split()[1] for l in open(FILE) if l.strip() and not l.startswith('#')]
res = {}
for l in open(f'{DIR}/results.txt'):
    f = l.split()
    if len(f) >= 6:
        res[f[0]] = (int(f[1]), int(f[2]))
pieces = [l.split() for l in open(f'{DIR}/pieces.txt')]
acc = {i: [0.0, 0, 0.0, 0.0, None, 0] for i in range(len(roots))}  # wall, nodes, thread-s, stopped-s, best, value
carry = 0.0
for k in range(1, len(pieces) + 1):
    lines = [l.rstrip('\n') for l in open(f'{DIR}/piece-{k}.out')]
    dur = float(pieces[k - 1][4])
    last = 0.0
    finished = any(l.startswith('# positions') or l.startswith('# deadline') for l in lines)
    for l in lines:
        if l.startswith('#'):
            continue
        body, t = l.rsplit(' @', 1)
        f = body.split()
        i = int(f[0])
        a = acc[i]
        a[0] += int(f[5]) / 1e6 + carry
        carry = 0.0
        a[1] += int(f[4])
        a[2] += float(f[10])
        a[3] += float(f[12])
        a[4] = f[11]
        a[5] = int(f[3])
        last = float(t)
    if not finished:
        carry += dur - last  # a root cut off: its seconds go to it when it is done
tot = [0.0, 0, 0.0, 0.0, 0, 0]
bad = 0
print('root  blue              tries failed  wall_s     Gnodes  thread_s  stopped_s  winning move     check')
for i, r in enumerate(roots):
    red, blue = r.split('_')[:2]
    tr = [(p, v) for p, v in res.items() if p.split('_')[1] == blue and p.split('_')[5] == '3' and p.split('_')[4] == '1']
    failed = sum(1 for p, (lo, hi) in tr if lo >= 0)
    wall, nodes, ts, ss, best, value = acc[i]
    ok = 'no value'
    if best is not None:
        mc = [int(c) for c in best.split(':')[1].split(',')]
        want = sorted(cells(red) + mc)
        hit = [p for p, (lo, hi) in tr if cells(p.split('_')[0]) == want and hi <= -1]
        ok = 'ok' if value == 1 and hit else 'FAILED'
    if ok != 'ok':
        bad += 1
    print(f"{i:4}  {','.join(map(str, cells(blue))):16s} {len(tr):5} {failed:6} {wall:7.1f} {nodes/1e9:10.2f} {ts:9.0f} {ss:10.0f}  {best or '-':16s} {ok}")
    for j, x in enumerate([wall, nodes, ts, ss, len(tr), failed]):
        tot[j] += x
print(f"total {'':16s} {tot[4]:5} {tot[5]:6} {tot[0]:7.1f} {tot[1]/1e9:10.2f} {tot[2]:9.0f} {tot[3]:10.0f}  {'' :16s} {'all ok' if bad == 0 else str(bad) + ' NOT OK'}")
