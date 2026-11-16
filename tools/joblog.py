"""Where the seconds of red's tries go: a reading of `prove --job-log` (E34).

    python tools/joblog.py JOBLOG [--roots A-B] [--tries]

The log has one line a job, `J root pass start end path alpha,beta result seconds nodes dead`, one
a decided coordinator node, `N root pass time path value finished given`, and `P` where a new
process (a piece of a replay) began. A path is `move@place/move@place` from the root; at depth 2
the first move is red's try, the second blue's reply (place = its place in blue's order).

Per root: tries, failed, wall, thread-seconds; per failed try: when its refuting job started after
the try's first job, the reply's place, the job's seconds, how many tries had jobs running while
it ran, and the try's thread-seconds before and while; the winning try's own seconds.
"""
import sys
from collections import defaultdict


def load(path):
    pieces = []
    cur = None
    for l in open(path):
        f = l.split()
        if not f:
            continue
        if f[0] == 'P':
            cur = {'J': [], 'N': []}
            pieces.append(cur)
            continue
        if cur is None:
            cur = {'J': [], 'N': []}
            pieces.append(cur)
        if f[0] == 'J':
            root, pw, t0, t1, path, ab, res, secs, nodes, dead = f[1:11]
            cur['J'].append(dict(root=int(root), pw=pw, t0=float(t0), t1=float(t1), path=path.split('/'),
                                 ab=ab, res=res, secs=float(secs), nodes=int(nodes), dead=int(dead)))
        elif f[0] == 'N':
            root, pw, t, path, v, fin, given = f[1:8]
            cur['N'].append(dict(root=int(root), pw=pw, t=float(t), path=path.split('/') if path != '-' else [],
                                 v=int(v), fin=int(fin), given=int(given)))
    return pieces


def mv(p):
    return p.rsplit('@', 1)[0]


def place(p):
    return int(p.rsplit('@', 1)[1])


def overlap(a0, a1, b0, b1):
    return max(0.0, min(a1, b1) - max(a0, b0))


def main():
    pieces = load(sys.argv[1])
    lo, hi = 0, 10 ** 9
    if '--roots' in sys.argv:
        a, b = sys.argv[sys.argv.index('--roots') + 1].split('-')
        lo, hi = int(a), int(b)
    show_tries = '--tries' in sys.argv
    # one record a root and piece (a root cut off appears in two pieces: both are counted)
    roots = defaultdict(lambda: {'J': [], 'N': []})
    for k, pc in enumerate(pieces):
        for j in pc['J']:
            roots[(j['root'], k)]['J'].append(j)
        for n in pc['N']:
            roots[(n['root'], k)]['N'].append(n)
    tot = defaultdict(float)
    ft_rows = []
    print('root piece tries failed  wall_s  thread_s  stopped_s  win_try_s(wall,thread)  failed: thread_s')
    for (r, k) in sorted(roots):
        if not lo <= r <= hi:
            continue
        J, N = roots[(r, k)]['J'], roots[(r, k)]['N']
        if not J:
            continue
        rootn = [n for n in N if not n['path']]
        wall = max([j['t1'] for j in J] + [n['t'] for n in rootn])
        tries = defaultdict(list)
        for j in J:
            tries[j['path'][0]].append(j)
        tryend = {}
        for n in N:
            if len(n['path']) == 1:
                tryend[n['path'][0]] = n
        ts = sum(j['secs'] for j in J)
        stopped = sum(j['secs'] for j in J if j['res'] == 'S')
        failed = [t for t, n in tryend.items() if n['v'] >= 0]  # blue's value at the try: blue holds
        won = [t for t, n in tryend.items() if n['v'] < 0]
        # running intervals of each try's jobs, for concurrency
        win_s = (0, 0)
        for t in won:
            js = tries.get(t, [])
            if js:
                win_s = (tryend[t]['t'] - min(j['t0'] for j in js), sum(j['secs'] for j in js))
        fts = 0.0
        for t in failed:
            js = tries.get(t, [])
            if not js:
                continue
            t_first = min(j['t0'] for j in js)
            end = tryend[t]['t']
            ref = [j for j in js if j['res'] == '0' or j['res'] == '-1']
            ref = [j for j in ref if abs(j['t1'] - end) < 0.05] or ref
            rj = max(ref, key=lambda j: j['t1']) if ref else None
            thr = sum(j['secs'] for j in js)
            fts += thr
            row = dict(root=r, piece=k, try_=mv(t), tplace=place(t), t_first=t_first, end=end, wall=end - t_first, thr=thr,
                       njobs=len(js), nstop=sum(1 for j in js if j['res'] == 'S'))
            if rj:
                a, b = rj['t0'], rj['t1']
                row.update(rplace=place(rj['path'][1]), rstart=a - t_first, rsecs=rj['secs'], rnodes=rj['nodes'],
                           before=sum(j['secs'] for j in js if j is not rj and j['t1'] <= a + 1e-9),
                           during_same=sum(overlap(a, b, j['t0'], j['t1']) for j in js if j is not rj),
                           after_same=sum(max(0.0, j['t1'] - max(a, j['t0'])) - overlap(a, b, j['t0'], j['t1']) for j in js if j is not rj),
                           replies_before=sum(1 for j in js if j is not rj and j['t0'] <= a),
                           jobs_after_start=sum(1 for j in js if j['t0'] > a + 1e-9))
                # what the 16 workers did while the refuting job ran, by the try their job belongs to
                by = defaultdict(float)
                for j in J:
                    if j is rj:
                        continue
                    o = overlap(a, b, j['t0'], j['t1'])
                    if o <= 0:
                        continue
                    t2 = j['path'][0]
                    if t2 == t:
                        by['same try'] += o
                    elif t2 in won:
                        by['winning try'] += o
                    elif t2 in failed:
                        by['other failed try'] += o
                    else:
                        by['try stopped' if j['res'] == 'S' else 'try never decided'] += o
                row['by'] = dict(by)
                row['busy'] = sum(by.values()) / max(1e-9, b - a)
                live = set(j['path'][0] for j in J if overlap(a, b, j['t0'], j['t1']) > 0)
                row['tries_live'] = len(live)
            ft_rows.append(row)
        print(f"{r:4} {k:5} {len(tryend):5} {len(failed):6} {wall:7.1f} {ts:9.0f} {stopped:10.0f}  {win_s[0]:8.1f} {win_s[1]:9.0f}        {fts:8.0f}")
        tot['wall'] += wall
        tot['ts'] += ts
        tot['stopped'] += stopped
        tot['fts'] += fts
        tot['tries'] += len(tryend)
        tot['failed'] += len(failed)
        tot['winw'] += win_s[0]
        tot['wint'] += win_s[1]
    print(f"total      {tot['tries']:5.0f} {tot['failed']:6.0f} {tot['wall']:7.1f} {tot['ts']:9.0f} {tot['stopped']:10.0f}  {tot['winw']:8.1f} {tot['wint']:9.0f}        {tot['fts']:8.0f}")
    if show_tries:
        print()
        print('root try_place try              try_wall try_thr jobs stop | ref_place ref_start ref_secs ref_Mn replies_before jobs_after | thr_before same_during same_after | tries_live busy | workers during ref job (thread-s)')
        for x in ft_rows:
            by = ' '.join(f"{k}={v:.0f}" for k, v in sorted(x.get('by', {}).items()))
            print(f"{x['root']:4} {x['tplace']:9} {x['try_']:16s} {x['wall']:8.1f} {x['thr']:7.0f} {x['njobs']:4} {x['nstop']:4} | {x.get('rplace', -1):9} {x.get('rstart', 0):9.1f} {x.get('rsecs', 0):8.1f} {x.get('rnodes', 0)/1e6:6.1f} {x.get('replies_before', 0):14} {x.get('jobs_after_start', 0):10} | {x.get('before', 0):10.0f} {x.get('during_same', 0):11.0f} {x.get('after_same', 0):10.0f} | {x.get('tries_live', 0):10} {x.get('busy', 0):4.1f} | {by}")
        # sums over failed tries
        n = len(ft_rows)
        if n:
            s = lambda key: sum(x.get(key, 0) for x in ft_rows)
            print(f"# {n} failed tries: try wall {s('wall'):.0f} s, thread {s('thr'):.0f} s; refuting jobs {s('rsecs'):.0f} s; "
                  f"same-try thread-s before the refuting job started {s('before'):.0f}, while it ran {s('during_same'):.0f}, after {s('after_same'):.0f}")
            agg = defaultdict(float)
            for x in ft_rows:
                for k, v in x.get('by', {}).items():
                    agg[k] += v
            print('# workers while refuting jobs ran (thread-s): ' + ', '.join(f"{k} {v:.0f}" for k, v in sorted(agg.items())))
            pl = sorted(x.get('rplace', -1) for x in ft_rows)
            print('# refuting reply places in blue order: ' + ' '.join(map(str, pl)))
            st = sorted(round(x.get('rstart', 0), 1) for x in ft_rows)
            print('# refuting job start after the try\'s first job (s): ' + ' '.join(map(str, st)))


main()
