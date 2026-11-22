"""Fill the blog post's generated parts in place. Run from the repo root.

    python tools/mkblog.py PLAY_DIR [--preview OUT_DIR]

PLAY_DIR is the play page's folder (wasm/play on branch `wasm`, with the built snek.wasm and
book.json beside play.html and worker.js). Two parts of docs/blog/solving-snekagon.html are
written between their markers:

  CERT  one string for each of blue's 172 replies, in the order they were refuted:
        reply|red's answer|red moves tried|seconds, from proofs/centre-opening.
  PLAY  the play page's script, unchanged but for its first and last line: it becomes a function
        that the post calls when the board comes near the screen.

With --preview the page is also written to OUT_DIR/index.html as a whole document, with the
worker, the solver and the book beside it, to look at through a local server.
It prints the figures the post quotes, so the text can be checked against them.
"""
import json
import re
import shutil
import statistics
import sys
from collections import Counter
from pathlib import Path

POST = Path("docs/blog/solving-snekagon.html")
PROOF = Path("proofs/centre-opening")


def cells(move):
    return [int(c) for c in move.split(":")[1].split(",")]


def touches(a, b):
    dq, dr = b % 11 - a % 11, b // 11 - a // 11
    return abs(dq) <= 1 and abs(dr) <= 1 and dq != dr


def cert_lines():
    meta = json.loads((PROOF / "meta.json").read_text(encoding="utf-8"))
    found = {}
    pat = re.compile(r"^\s+blue (\S+)\s+loses to red (\S+)\s+\((\d+) red moves tried")
    for line in (PROOF / "certificate.txt").read_text(encoding="utf-8").splitlines():
        m = pat.match(line)
        if m:
            found[m.group(1)] = (m.group(2), int(m.group(3)))
    assert len(found) == 172, len(found)
    assert len(meta["roots"]) == 172 and len(meta["seconds"]) == 172
    rows = []
    for (reply, _), secs in zip(meta["roots"], meta["seconds"]):
        answer, tries = found[reply.split(":")[1]]
        rows.append((reply, answer, tries, secs))
    return rows


def report(rows):
    secs = [r[3] for r in rows]
    tries = [r[2] for r in rows]
    answers = Counter(r[1] for r in rows)
    slow = sorted(rows, key=lambda r: -r[3])[:3]
    print("replies", len(rows), " hours", round(sum(secs) / 3600, 1),
          " first 74:", round(sum(secs[:74]) / 3600, 1), " rest:", round(sum(secs[74:]) / 3600, 1))
    print("median s", statistics.median(secs), " max", max(secs), " slowest", [(r[0], r[3]) for r in slow])
    print("tries: one", sum(t == 1 for t in tries), " up to three", sum(t <= 3 for t in tries),
          " twenty or more", sum(t >= 20 for t in tries), " max", max(tries))
    print("answers: distinct", len(answers), " by piece", dict(Counter(a.split(":")[0] for _, a, _, _ in rows)))
    print("most used", answers.most_common(4))
    print("answer touches the centre", sum(any(touches(60, c) for c in cells(a)) for _, a, _, _ in rows))
    print("reply sizes", dict(Counter(len(cells(r[0])) for r in rows)))


def between(text, name, body):
    a, b = "/*%s-BEGIN*/" % name, "/*%s-END*/" % name
    assert text.count(a) == 1 and text.count(b) == 1, name
    i, j = text.index(a) + len(a), text.index(b)
    return text[:i] + body + text[j:]


def play_script(play_dir):
    page = (play_dir / "play.html").read_text(encoding="utf-8")
    script = page[page.index("<script>") + len("<script>"):page.rindex("</script>")].strip("\n")
    head, tail = "(function () {", "})();"
    assert script.startswith(head) and script.endswith(tail)
    return "\nwindow.__startPlay = function () {" + script[len(head):-len(tail)] + "};\n"


def main():
    args = sys.argv[1:]
    preview = None
    if "--preview" in args:
        i = args.index("--preview")
        preview = Path(args[i + 1])
        del args[i:i + 2]
    play_dir = Path(args[0])
    rows = cert_lines()
    report(rows)
    data = json.dumps(["%s|%s|%d|%d" % (r[0], r[1], r[2], round(r[3])) for r in rows], separators=(",", ""))
    text = POST.read_text(encoding="utf-8")
    text = between(text, "CERT", data)
    text = between(text, "PLAY", play_script(play_dir))
    POST.write_text(text, encoding="utf-8", newline="\n")
    print("wrote", POST, len(text.encode("utf-8")), "bytes")
    if preview:
        preview.mkdir(parents=True, exist_ok=True)
        doc = '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n' \
              '<meta name="viewport" content="width=device-width, initial-scale=1">\n</head>\n<body>\n' + text + "</body>\n</html>\n"
        (preview / "index.html").write_text(doc, encoding="utf-8", newline="\n")
        for name in ("worker.js", "snek.wasm", "book.json"):
            shutil.copyfile(play_dir / name, preview / name)
        print("preview in", preview)


main()
