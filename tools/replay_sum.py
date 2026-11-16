"""Summary of a proof-order replay (tools/replay.sh): one line a root, totals, and the certificate check.

    python tools/replay_sum.py FILE DIR
    python tools/replay_sum.py FILE --pairs CAND_DIR BASE_DIR [--metric wall|thread]

Per root: red's tries (positions after a red move, blue to move, found in the results file), the
failed ones (blue holds at least a draw), wall-clock seconds (summed over pieces, with the seconds a
root lost when a piece was cut off), nodes, thread-seconds, and job seconds lost on stopped jobs.
Check, as tools/certificate.py does: the root is a red win, and the position after the winning move
(blue to move) is in the results file searched to a loss.

--pairs prints one JSON line a root, {"cand": x, "base": y} (wall seconds by default, or
thread-seconds), for a paired comparison of two replays of the same file:
    python tools/replay_sum.py FILE --pairs DIR_A DIR_B > pairs.jsonl
    python tools/hc.py report pairs.jsonl --ratio --lower-is-better
"""
import json
import sys


def cells(h):
    v = int(h, 16)
    return [i for i in range(121) if v >> i & 1]


def collect(FILE, DIR):
    roots = [l.split()[1] for l in open(FILE) if l.strip() and not l.startswith('#')]
    res = {}
    for l in open(f'{DIR}/results.txt'):
        f = l.split()
        if len(f) >= 6:
            res[f[0]] = (int(f[1]), int(f[2]))
    pieces = [l.split() for l in open(f'{DIR}/pieces.txt')]
    acc = {i: [0.0, 0, 0.0, 0.0, None, 0] for i in range(len(roots))}  # wall, nodes, thread-s, stopped-s, best, value
    carry = 0.0
    completed = set()
    cut = {}  # root: pieces in which it was cut off
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
            last = float(t)
            if i in completed:
                continue  # a piece first decides the roots done before from the results file
            completed.add(i)
            a = acc[i]
            a[0] += int(f[5]) / 1e6 + carry
            carry = 0.0
            a[1] += int(f[4])
            a[2] += float(f[10])
            a[3] += float(f[12])
            a[4] = f[11]
            a[5] = int(f[3])
        if not finished:
            carry += dur - last  # a root cut off: its seconds go to it when it is done
            nxt = min(set(range(len(roots))) - completed, default=None)
            if nxt is not None:
                cut[nxt] = cut.get(nxt, 0) + 1
                # the cut-off root's nodes and thread-seconds, from the driver's last progress line
                # of the piece (written about once a minute: up to a minute of work is missed)
                try:
                    err = open(f'{DIR}/piece-{k}.err').readlines()
                except OSError:
                    err = []
                ends = [j for j, l in enumerate(err) if ') gave ' in l]
                start = ends[-1] + 1 if ends else 0  # after the last root this piece finished
                prog = [l for l in err[start:] if 'window (' in l and ' jobs ' in l and ' nodes ' in l]
                if prog:
                    w = prog[-1].split()
                    acc[nxt][1] += int(float(w[w.index('nodes') + 1]) * 1e6)
                    acc[nxt][2] += float(w[w.index('thread-s') + 1])
    rows = []
    for i, r in enumerate(roots):
        f0 = r.split('_')
        red, blue, stm, ply = f0[0], f0[1], int(f0[4]), int(f0[5])
        mover, other = (red, blue) if stm == 0 else (blue, red)
        # the mover's tries: positions one ply down whose other side is as at the root
        def side(p, k):
            return p.split('_')[k]
        ok_k = 1 if stm == 0 else 0
        tr = [(p, v) for p, v in res.items() if int(side(p, 5)) == ply + 1 and int(side(p, 4)) == 1 - stm and side(p, ok_k) == other]
        failed = sum(1 for p, (lo, hi) in tr if lo >= 0)
        wall, nodes, ts, ss, best, value = acc[i]
        ok = 'no value'
        if best is not None:
            mc = [int(c) for c in best.split(':')[1].split(',')] if ':' in best else []
            want = sorted(cells(mover) + mc)
            mk = 0 if stm == 0 else 1
            hit = [p for p, (lo, hi) in tr if cells(p.split('_')[mk]) == want and hi <= -1]
            ok = 'ok' if value == 1 and hit else 'FAILED'
        rows.append(dict(i=i, blue=cells(other), tries=len(tr), failed=failed, wall=wall, nodes=nodes, thread=ts, stopped=ss, best=best, ok=ok, cut=cut.get(i, 0)))
    return rows


def main():
    FILE = sys.argv[1]
    if '--pairs' in sys.argv:
        k = sys.argv.index('--pairs')
        cand, base = collect(FILE, sys.argv[k + 1]), collect(FILE, sys.argv[k + 2])
        metric = sys.argv[sys.argv.index('--metric') + 1] if '--metric' in sys.argv else 'wall'
        for a, b in zip(cand, base):
            print(json.dumps({"cand": round(a[metric], 3), "base": round(b[metric], 3)}))
        return
    rows = collect(FILE, sys.argv[2])
    tot = [0.0, 0, 0.0, 0.0, 0, 0]
    bad = 0
    print('root  blue              tries failed  wall_s     Gnodes  thread_s  stopped_s  winning move     check  cut')
    for r in rows:
        if r['ok'] != 'ok':
            bad += 1
        print(f"{r['i']:4}  {','.join(map(str, r['blue'])):16s} {r['tries']:5} {r['failed']:6} {r['wall']:7.1f} {r['nodes']/1e9:10.2f} {r['thread']:9.0f} {r['stopped']:10.0f}  {r['best'] or '-':16s} {r['ok']:6s} {r['cut'] or ''}")
        for j, x in enumerate([r['wall'], r['nodes'], r['thread'], r['stopped'], r['tries'], r['failed']]):
            tot[j] += x
    print(f"total {'':16s} {tot[4]:5} {tot[5]:6} {tot[0]:7.1f} {tot[1]/1e9:10.2f} {tot[2]:9.0f} {tot[3]:10.0f}  {'':16s} {'all ok' if bad == 0 else str(bad) + ' NOT OK'}")
    print(f"# sum over roots: wall {tot[0]:.1f} s, thread {tot[2]:.0f} s")


main()
