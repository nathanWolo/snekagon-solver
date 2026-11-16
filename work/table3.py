"""Per-root table of several replay summaries side by side (E34).

    python work/table3.py N NAME=SUMMARY [NAME=SUMMARY ...]
"""
import sys

n = int(sys.argv[1])
arms = []
for a in sys.argv[2:]:
    name, path = a.split('=', 1)
    rows = {}
    for l in open(path):
        f = l.split()
        if f and f[0].isdigit():
            rows[int(f[0])] = dict(tries=int(f[2]), failed=int(f[3]), wall=float(f[4]), thread=float(f[6]), ok=f[9])
    arms.append((name, rows))
head = 'root ' + ' | '.join(f'{name:>22}' for name, _ in arms) + ' | ' + ' '.join(f'{name}/{arms[0][0]}' for name, _ in arms[1:])
print(head)
tot = [[0, 0, 0.0, 0.0] for _ in arms]
for i in range(n):
    cells = []
    for k, (name, rows) in enumerate(arms):
        r = rows[i]
        cells.append(f"{r['tries']:3}/{r['failed']:3} {r['wall']:7.1f} {r['ok']:>3}")
        tot[k][0] += r['tries']; tot[k][1] += r['failed']; tot[k][2] += r['wall']; tot[k][3] += r['thread']
    b = arms[0][1][i]['wall']
    ratios = ' '.join(f"{rows[i]['wall'] / b:5.2f}" for _, rows in arms[1:])
    print(f"{i:4} " + ' | '.join(f'{c:>22}' for c in cells) + ' | ' + ratios)
print('tot  ' + ' | '.join(f'{t[0]:3}/{t[1]:3} {t[2]:7.1f}    ' for t in tot) + ' | ' +
      ' '.join(f'{t[2] / tot[0][2]:5.3f}' for t in tot[1:]))
print('thread-s: ' + ', '.join(f'{name} {t[3]:.0f}' for (name, _), t in zip(arms, tot)) + ' | ratios ' +
      ' '.join(f'{t[3] / tot[0][3]:5.3f}' for t in tot[1:]))
