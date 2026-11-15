"""Lists what a results file of `snek prove` says about the centre opening, and what is missing.

    python tools/certificate.py work/long/ply1_results.txt [--open]

For every position after `1:60` and one blue reply that the file knows to be a red win: blue's
reply (its cells) and red's winning move. Each line is a claim anyone can check with any solver:
after 1:60, the blue cells and the red move, blue to move loses.

The replies and their classes under the board's twelve symmetries are counted here from the
piece shapes alone, not taken from the solver, so the last lines are a check of the driver's
symmetry rule as well: the centre opening is a red win when every class has a reply that is
known to lose. `--open` lists one reply of every class that is still open.

A reply counts as lost here only when the file also holds the job behind it: the position after
red's winning move, blue to move, searched to a loss. So the claim rests on the search results
in the file and on this script, not on the bookkeeping of the driver's coordinator.

Cell numbers are (r + 5) * 11 + (q + 5), as in the move texts.
"""
import sys

SHAPES = {
    "2": [(0, 0), (1, 0)],
    "3I": [(0, 0), (0, 1), (0, 2)],
    "3V": [(0, 0), (1, 0), (1, 1)],
    "4I": [(0, 0), (-1, 1), (-2, 2), (-3, 3)],
    "4A": [(0, 0), (1, 0), (1, 1), (1, 2)],
    "4B": [(0, 0), (0, 1), (1, 1), (1, 2)],
    "4C": [(0, 0), (-1, 1), (-1, 2), (0, 2)],
}
# a first piece starts a new snake and may touch no tile: not the centre cell, not its neighbours
NEAR = {(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)}


def symmetry(t, c):
    q, r = (c[1], c[0]) if t >= 6 else c
    for _ in range(t % 6):
        q, r = -r, q + r
    return (q, r)


def on_board(c):
    q, r = c
    return abs(q) <= 5 and abs(r) <= 5 and abs(q + r) <= 5


def index(c):
    q, r = c
    return (r + 5) * 11 + (q + 5)


def reply_classes():
    """Every blue reply to 1:60 as a tuple of cell numbers, grouped into symmetry classes."""
    board = [(q, r) for q in range(-5, 6) for r in range(-5, 6) if on_board((q, r))]
    replies = set()
    for shape in SHAPES.values():
        for t in range(12):
            turned = [symmetry(t, c) for c in shape]
            for dq, dr in board:
                placed = frozenset((q + dq, r + dr) for q, r in turned)
                if all(on_board(c) for c in placed) and not (placed & NEAR):
                    replies.add(placed)
    classes = []
    seen = set()
    for reply in sorted(replies, key=lambda p: sorted(index(c) for c in p)):
        if reply in seen:
            continue
        orbit = {frozenset(symmetry(t, c) for c in reply) for t in range(12)}
        seen |= orbit
        classes.append([tuple(sorted(index(c) for c in member)) for member in orbit])
    return len(replies), classes


def cells(hex_text):
    v = int(hex_text, 16)
    return [i for i in range(121) if v >> i & 1]


def main():
    path = sys.argv[1]
    show_open = "--open" in sys.argv[2:]
    wins = {}
    holds = {}
    tries = {}
    lost_jobs = set()  # (red cells, blue cells) of positions with blue to move that are lost
    for line in open(path):
        f = line.split()
        if len(f) < 6:
            continue
        pos = f[0].split("_")
        if len(pos) != 7:
            continue
        red, blue, _lr, _lb, stm, ply, _passes = pos
        lo, hi = int(f[1]), int(f[2])
        if ply == "2" and stm == "0" and cells(red) == [60]:
            key = tuple(cells(blue))
            if lo >= 1:
                wins[key] = f[5]
            elif hi <= 0:
                holds[key] = f[5]
        if ply == "3" and stm == "1":
            # a red try after a blue reply: blue cells are the reply; count tries by reply
            key = tuple(cells(blue))
            t = tries.setdefault(key, [0, 0, 0.0])
            t[0] += 1
            if hi <= -1:
                t[1] += 1
            t[2] += int(f[4]) / 1000
            if hi <= -1:
                lost_jobs.add((tuple(cells(red)), key))
    print(f"{len(wins)} blue replies to 1:60 are known to lose; {len(holds)} are known to hold")
    unbacked = []
    for key in sorted(wins):
        t = tries.get(key, [0, 0, 0.0])
        red_after = tuple(sorted([60] + [int(c) for c in wins[key].split(":")[1].split(",")]))
        note = ""
        if (red_after, key) not in lost_jobs:
            unbacked.append(key)
            note = "  NO JOB RECORD"
        print(f"  blue {','.join(map(str, key)):16s} loses to red {wins[key]:18s} ({t[0]} red moves tried, {t[2]:.0f} s){note}")
    for key in unbacked:
        del wins[key]
    if unbacked:
        print(f"  WARNING: {len(unbacked)} of these have no searched position behind them in the file and are not counted below")
    for key in sorted(holds):
        print(f"  blue {','.join(map(str, key)):16s} HOLDS (red cannot win)")

    total, classes = reply_classes()
    lost = [c for c in classes if any(m in wins for m in c)]
    held = [c for c in classes if any(m in holds for m in c)]
    still = [c for c in classes if not any(m in wins or m in holds for m in c)]
    stray = [k for k in list(wins) + list(holds) if not any(k in c for c in classes)]
    print(f"{total} replies in {len(classes)} classes under the twelve symmetries: "
          f"{len(lost)} classes lose, {len(held)} hold, {len(still)} open")
    if stray:
        print(f"  WARNING: {len(stray)} replies in the file are not legal replies by this count: {stray[:3]}")
    if not still and not held:
        print("  every class has a reply that loses: by this file, 1:60 is a win for red")
    if show_open:
        for c in still:
            print(f"  open: blue {','.join(map(str, min(c)))}")


if __name__ == "__main__":
    main()
