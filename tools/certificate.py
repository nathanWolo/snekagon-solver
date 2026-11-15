"""Lists what a results file of `snek prove` says about the centre opening.

    python tools/certificate.py work/long/ply1_results.txt

For every position after `1:60` and one blue reply that the file knows to be a red win: blue's
reply (its cells) and red's winning move. Each line is a claim anyone can check with any solver:
after 1:60, the blue cells and the red move, blue to move loses.

Cell numbers are (r + 5) * 11 + (q + 5), as in the move texts.
"""
import sys


def cells(hex_text):
    v = int(hex_text, 16)
    return [i for i in range(121) if v >> i & 1]


def main():
    path = sys.argv[1]
    wins = {}
    holds = {}
    tries = {}
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
    print(f"{len(wins)} blue replies to 1:60 are known to lose; {len(holds)} are known to hold")
    for key in sorted(wins):
        t = tries.get(key, [0, 0, 0.0])
        print(f"  blue {','.join(map(str, key)):16s} loses to red {wins[key]:18s} ({t[0]} red moves tried, {t[2]:.0f} s)")
    for key in sorted(holds):
        print(f"  blue {','.join(map(str, key)):16s} HOLDS (red cannot win)")


if __name__ == "__main__":
    main()
