"""Checks the analysis board's rules (gui/rules.js) against the solver.

    python tools/gui_check.py [--games 300] [--seed 1] [--solver frozen/climb-base.exe] [--browser EXE]

Plays random games with the solver's own move generator (`children`, a pass when a side has no
placement) and keeps every position on the way, from the start to the end of each game. Then the page's
rules run in a headless Edge or Chrome on all of them: the set of child positions (as position texts)
must be the solver's, the text must read back to itself, perft 1 and 2 must be 91 and 194,388, each
game's move list must load to the game's last position, and malformed input must be refused. Also
checks the server's own reading of positions and moves (tools/gui.py) on the same games.
Prints the counts and exits non-zero on any disagreement. A few seconds of one processor.
"""
import argparse
import json
import os
import random
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gui  # noqa: E402

BROWSERS = [r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"]


def fnv(s):
    h = 0x811C9DC5
    for ch in s.encode():
        h = ((h ^ ch) * 0x01000193) & 0xFFFFFFFF
    return "%08x" % h


def play_games(solver, games, rng):
    """Random games; returns (positions {text: children texts}, [(move list, final text)])."""
    start = gui.to_text({"cells": [0, 0], "left": [255, 254], "stm": 0, "ply": 0, "passes": 0})
    kids_of = {}
    live = [(k, start, []) for k in range(games)]
    finished = []
    while live:
        todo = sorted({t for _, t, _ in live if t not in kids_of})
        for t, kids in zip(todo, solver.children(todo) if todo else []):
            kids_of[t] = kids
        nxt = []
        for k, t, line in live:
            kids = kids_of[t]
            if not kids:  # the game is over
                finished.append((line, t))
                continue
            m, c = rng.choice(kids)
            nxt.append((k, c, line + [m]))
        live = nxt
    return kids_of, finished


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--games", type=int, default=300)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--solver", default=str(Path("frozen") / "climb-base.exe"))
    ap.add_argument("--browser", default=None)
    a = ap.parse_args()
    sp = Path(a.solver)
    solver = gui.Solver(sp if sp.is_absolute() else gui.ROOT / sp)
    if not solver.exists():
        sys.exit("no solver at " + solver.path)
    browser = a.browser or next((b for b in BROWSERS if os.path.isfile(b)), None) or shutil.which("msedge") or shutil.which("chrome")
    if not browser:
        sys.exit("no Edge or Chrome found; pass --browser")

    t0 = time.time()
    rng = random.Random(a.seed)
    kids_of, games = play_games(solver, a.games, rng)
    texts = sorted(kids_of)
    print("random games: %d, positions: %d (%.1f s)" % (len(games), len(texts), time.time() - t0))
    fails = []

    # the server's own reading: move names and move lists
    for line, final in games:
        code, out, err = solver.quick(["text"] + line)
        if line and out.strip() != final:
            fails.append("solver text of a derived move list differs: %s" % " ".join(line))
    for t in texts:
        if gui.to_text(gui.parse_text(t)) != t:
            fails.append("tools/gui.py reads %s differently" % t)
    for line, final in games[:20]:
        try:
            if gui.load_input(solver, " ".join(line)) != final:
                fails.append("tools/gui.py load of a move list differs")
        except gui.BadPosition as e:
            fails.append("tools/gui.py refuses a game: %s" % e)

    bad = ["", "xyz", "1:61 2:1,2", "1:60 4B:16,27,28,39 pass", "2:60,61", "1:60 1:61",
           "00000000000000001000000000000000_00000000000000000000000000000000_255_254_1_1_0",
           "00000000000000001000000000000000_00000000000000000000000000000060_254_254_0_2_0",
           "0000000000000000000000000000000f_00000000000000000000000000000000_253_254_1_1_0",
           "ffffffffffffffffffffffffffffffff_00000000000000000000000000000000_0_254_1_1_0",
           "1_2_3"]
    for s in bad:
        try:
            gui.load_input(solver, s)
            fails.append("tools/gui.py accepts the malformed %r" % s)
        except gui.BadPosition:
            pass

    data = {"positions": texts, "games": [" ".join(line) for line, _ in games], "bad": bad, "full": []}
    result = {}
    stages = []
    done = threading.Event()

    class H(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            if self.path.startswith("/check/stage?"):
                stages.append(urllib.parse.unquote(self.path.split("?", 1)[1]))
                self.send_response(204)
                self.end_headers()
                return
            if self.path == "/check/data":
                b = json.dumps(data).encode()
                ctype = "application/json"
            elif self.path in ("/check.html", "/rules.js"):
                b = (gui.GUI_DIR / self.path[1:]).read_bytes()
                ctype = "text/html" if self.path.endswith("html") else "text/javascript"
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(b)))
            self.end_headers()
            self.wfile.write(b)

        def do_POST(self):
            n = int(self.headers.get("Content-Length") or 0)
            got = json.loads(self.rfile.read(n))
            self.send_response(200)
            self.send_header("Content-Length", "2")
            self.end_headers()
            self.wfile.write(b"ok")
            self.wfile.flush()
            if not done.is_set():
                result.update(got)
                done.set()

    srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    url = "http://127.0.0.1:%d/check.html" % srv.server_address[1]
    profile = tempfile.mkdtemp(prefix="gui-check-")
    proc = None
    try:
        for attempt in range(2):  # a headless browser that does not get going is started once more
            proc = subprocess.Popen([browser, "--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check",
                                     "--user-data-dir=" + profile, url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if done.wait(120):
                break
            print("the page did not answer within 2 minutes (stages: %s)" % (", ".join(stages) or "none"), flush=True)
            proc.kill()
            proc.wait()
            proc = None
        if not done.is_set():
            sys.exit("giving up")
    finally:
        if proc:
            proc.kill()
            proc.wait()
        srv.shutdown()
        time.sleep(0.5)
        shutil.rmtree(profile, ignore_errors=True)
    if "fatal" in result:
        sys.exit("the page failed: " + result["fatal"])

    print("page: placements %d, perft 1 = %d, perft 2 = %d (%d ms for everything)" % (
        result["placements"], result["perft1"], result["perft2"], result["ms"]))
    if (result["placements"], result["perft1"], result["perft2"]) != (2602, 91, 194388):
        fails.append("placements or perft differ from 2602, 91, 194388")
    code, out, _ = solver.quick(["perft", "2"])
    print("solver:", " ".join(l.split("(")[0].strip() for l in out.strip().splitlines()))

    by_ply, kinds = Counter(), Counter()
    total_children = 0
    for r in result["positions"]:
        t = r["text"]
        p = gui.parse_text(t)
        sol = [c for m, c in kids_of[t] if m != "pass"]
        total_children += len(sol)
        by_ply[p["ply"]] += 1
        if p["passes"]:
            kinds["after a pass"] += 1
        if gui.is_over(p):
            kinds["game over"] += 1
        elif not sol:
            kinds["side to move must pass"] += 1
        if "error" in r:
            fails.append("page refuses %s: %s" % (t, r["error"]))
            continue
        if not r["roundtrip"]:
            fails.append("page reads %s back differently" % t)
        if r["n"] != len(sol) or r["h"] != fnv("\n".join(sorted(sol))):
            fails.append("children differ at %s: page %d, solver %d" % (t, r["n"], len(sol)))
        if r["over"] != gui.is_over(p):
            fails.append("game over differs at %s" % t)
    for (line, final), got in zip(games, result["games"]):
        if got != final:
            fails.append("page loads a game differently: %s -> %s" % (" ".join(line), got))
    for s, msg in zip(bad, result["bad"]):
        if msg is None:
            fails.append("page accepts the malformed %r" % s)

    print("positions compared: %d, with %d children in all" % (len(result["positions"]), total_children))
    print("by ply: " + " ".join("%d:%d" % kv for kv in sorted(by_ply.items())))
    print("special: " + ", ".join("%s %d" % kv for kv in sorted(kinds.items())))
    print("games: %d move lists loaded by the page and by tools/gui.py; %d malformed inputs refused by both" % (len(games), len(bad)))
    for f in fails[:20]:
        print("FAIL", f)
    print("disagreements: %d" % len(fails))
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
