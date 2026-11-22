"""Checks another proof's answers with our solver.

A proof that red wins after `1:60` is, at its top, a list: for every class of blue's reply Y one red
answer Z, with the claim that after `1:60 Y Z` blue, to move, loses. This script takes such a list
from someone else and prepares it for `snek prove`, which then decides every claim on its own.

    python tools/crosscheck.py prepare INDEX.txt DIR     # lines "Y Z [anything]", '#' starts a comment
    python tools/crosscheck.py report DIR                # after the run: what the driver said

`prepare` checks the list against this repository's own count of the replies (`certificate.py`:
1,932 replies in 172 classes, from the piece shapes alone): every line's Y must be a legal reply,
and the lines must cover every class exactly once. It writes

    DIR/lines.txt   one root for the driver per claim: "-1 1:60 Y Z" (expected value -1: the side
                    to move, blue, loses)
    DIR/map.tsv     for every claim: Y, Z, the reply of that class in our own proof, our answer to
                    it carried onto their Y by the symmetry that maps the replies, and whether the
                    two answers are the same move

The run itself (see docs/LOG.md, "The first proof's answers, checked"):

    sh tools/locked.sh frozen/prove-replay.exe prove DIR/lines.txt --threads 16 --job-threads 1 \
        --depth 1 --tt 27 --out DIR/results.txt > DIR/run.log 2> DIR/run.err

The driver prints one line per root (index, ply, expected, value, nodes, microseconds, ok or WRONG,
...) and exits 1 if any value differs from the expected one. Nothing of the other proof is run or
read here except the list.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from certificate import SHAPES, index, on_board, reply_classes, symmetry  # noqa: E402

MOVE = re.compile(r"^(2|3I|3V|4I|4A|4B|4C):(\d+(?:,\d+)*)$")
ROOT = Path(__file__).resolve().parent.parent


def cells_of(move):
    m = MOVE.match(move)
    if not m:
        raise SystemExit(f"not a move: {move!r}")
    cs = [int(c) for c in m.group(2).split(",")]
    if cs != sorted(set(cs)):
        raise SystemExit(f"cells not ascending: {move!r}")
    return m.group(1), tuple(cs)


def coord(c):
    return (c % 11 - 5, c // 11 - 5)


def is_piece(name, cs):
    """The cells are a placement of that piece: some turn of its shape, moved onto the board."""
    want = frozenset(coord(c) for c in cs)
    if not all(on_board(c) for c in want):
        return False
    for t in range(12):
        turned = [symmetry(t, c) for c in SHAPES[name]]
        for anchor in want:
            dq, dr = anchor[0] - turned[0][0], anchor[1] - turned[0][1]
            if frozenset((q + dq, r + dr) for q, r in turned) == want:
                return True
    return False


def turn(t, cs):
    return tuple(sorted(index(symmetry(t, coord(c))) for c in cs))


def our_proof():
    """reply cells -> our winning answer (move text), from the committed certificate."""
    out = {}
    pat = re.compile(r"^\s+blue (\S+)\s+loses to red (\S+)")
    for line in open(ROOT / "proofs" / "centre-opening" / "certificate.txt", encoding="utf-8"):
        m = pat.match(line)
        if m:
            out[tuple(int(c) for c in m.group(1).split(","))] = m.group(2)
    return out


def prepare(index_path, out_dir):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    claims = []
    for n, line in enumerate(open(index_path, encoding="utf-8"), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        f = line.split()
        if len(f) < 2:
            raise SystemExit(f"line {n}: needs a reply and an answer")
        (yp, yc), (zp, zc) = cells_of(f[0]), cells_of(f[1])
        if not is_piece(yp, yc) or not is_piece(zp, zc):
            raise SystemExit(f"line {n}: the cells are not a placement of the named piece: {line}")
        if set(yc) & set(zc) or 60 in yc or 60 in zc:
            raise SystemExit(f"line {n}: the moves overlap each other or the centre: {line}")
        if len(f) > 2 and f[2] != f[0].replace(":", "_").replace(",", "_") + ".lvb":
            raise SystemExit(f"line {n}: the file name does not match the reply: {line}")
        claims.append((f[0], yc, f[1], zc, zp))

    total, classes = reply_classes()
    class_of = {member: k for k, members in enumerate(classes) for member in members}
    ours = our_proof()
    our_rep = {}
    for cs, answer in ours.items():
        our_rep[class_of[cs]] = (cs, answer)
    if len(our_rep) != len(classes):
        raise SystemExit(f"our own certificate covers {len(our_rep)} of {len(classes)} classes")

    seen = {}
    rows = []
    for y, yc, z, zc, zp in claims:
        if yc not in class_of:
            raise SystemExit(f"{y} is not a legal reply to 1:60 by our count")
        k = class_of[yc]
        if k in seen:
            raise SystemExit(f"{y} and {seen[k]} are the same reply up to symmetry")
        seen[k] = y
        ocs, oanswer = our_rep[k]
        opiece, oz = cells_of(oanswer)
        # every symmetry that carries our reply onto theirs carries our answer onto a move that
        # wins against theirs; the two proofs agree on this class if one of those is their answer
        carried = sorted({turn(t, oz) for t in range(12) if turn(t, ocs) == yc})
        if not carried:
            raise SystemExit(f"no symmetry maps our reply {ocs} onto {y}")
        same = zc in carried and zp == opiece
        rows.append((y, z, ",".join(map(str, ocs)), oanswer,
                     opiece + ":" + ",".join(map(str, carried[0])), "same" if same else "different"))
    missing = [k for k in range(len(classes)) if k not in seen]
    if missing:
        raise SystemExit(f"{len(missing)} classes of replies have no line, e.g. {classes[missing[0]][0]}")

    with open(out_dir / "lines.txt", "w", encoding="utf-8", newline="\n") as f:
        for y, z, *_ in rows:
            f.write(f"-1 1:60 {y} {z}\n")
    with open(out_dir / "map.tsv", "w", encoding="utf-8", newline="\n") as f:
        f.write("their_reply\ttheir_answer\tour_reply_cells\tour_answer\tour_answer_on_their_reply\tanswers\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    n_same = sum(r[5] == "same" for r in rows)
    print(f"{len(rows)} claims; {total} replies in {len(classes)} classes by our count, every class covered once")
    print(f"answers: {n_same} the same move as in our proof, {len(rows) - n_same} different")
    print(f"wrote {out_dir / 'lines.txt'} and {out_dir / 'map.tsv'}")


def report(out_dir):
    out_dir = Path(out_dir)
    lines = [l.split() for l in open(out_dir / "lines.txt", encoding="utf-8") if l.strip()]
    same = {}
    for l in list(open(out_dir / "map.tsv", encoding="utf-8"))[1:]:
        f = l.rstrip("\n").split("\t")
        same[(f[0], f[1])] = f[5]
    done = {}
    for l in open(out_dir / "run.log", encoding="utf-8"):
        f = l.split()
        if len(f) >= 11 and f[0].isdigit() and f[6] in ("ok", "WRONG"):
            done[int(f[0])] = f
    ok = [i for i, f in done.items() if f[6] == "ok" and f[3] == "-1"]
    wrong = [i for i in done if i not in ok]
    secs = sorted(((int(done[i][5]) / 1e6, i) for i in done), reverse=True)
    nodes = sum(int(f[4]) for f in done.values())
    jobs = sum(int(f[7]) for f in done.values())
    thread_s = sum(float(f[10]) for f in done.values())
    print(f"{len(done)} of {len(lines)} claims decided: {len(ok)} confirmed (blue loses), {len(wrong)} not")
    for i in sorted(wrong):
        print(f"  NOT CONFIRMED: {' '.join(lines[i][1:])}: value {done[i][3]} for blue, best {done[i][-1]}")
    total = sum(s for s, _ in secs)
    print(f"time {total / 3600:.2f} h, {thread_s / 3600:.1f} thread-hours, {nodes / 1e9:.1f} G nodes, {jobs} jobs")
    if secs:
        med = secs[len(secs) // 2][0]
        print(f"per claim: median {med:.0f} s; slowest " + ", ".join(f"{lines[i][2]} -> {lines[i][3]} {s:.0f} s" for s, i in secs[:3]))
    n_same = sum(same.get((lines[i][2], lines[i][3])) == "same" for i in done)
    print(f"of the decided claims, {n_same} use the same answer as our proof and {len(done) - n_same} a different one")
    with open(out_dir / "summary.tsv", "w", encoding="utf-8", newline="\n") as f:
        f.write("their_reply\ttheir_answer\tvalue_for_blue\tverdict\tseconds\tnodes\tjobs\tanswers\n")
        for i in sorted(done):
            d = done[i]
            f.write("\t".join([lines[i][2], lines[i][3], d[3], d[6], f"{int(d[5]) / 1e6:.1f}", d[4], d[7],
                               same.get((lines[i][2], lines[i][3]), "?")]) + "\n")
    print(f"wrote {out_dir / 'summary.tsv'}")
    return 0 if not wrong and len(done) == len(lines) else 1


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "prepare":
        prepare(sys.argv[2], sys.argv[3])
    elif len(sys.argv) >= 3 and sys.argv[1] == "report":
        sys.exit(report(sys.argv[2]))
    else:
        sys.exit(__doc__)
