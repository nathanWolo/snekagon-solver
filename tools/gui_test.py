"""Exercises the analysis board's server (tools/gui.py) through its HTTP endpoints.

    python tools/gui_test.py [--solver frozen/climb-base.exe] [--port 8799] [--no-lock-tests]

Starts its own server, then: load (good and bad input), the moves of a position, play and undo (by
loading a line one move longer and shorter), all moves of a late position, a solve of a proof position
four moves in (through the run lock, 16 processes), a time limit that is hit, a cancelled long solve,
a run whose page goes away, and two runs at once. Checks that no solver process outlives a run and that
every run is in work/gui/runs.log. The locked solve waits for the machine's run lock.
"""
import argparse
import json
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gui  # noqa: E402

BASE = ""
FAILS = []


def call(path, body=None):
    req = urllib.request.Request(BASE + path, data=None if body is None else json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        FAILS.append(what)


def alive(pids):
    out = subprocess.run(["tasklist", "/FO", "CSV", "/NH"], capture_output=True, text=True).stdout
    running = {int(l.split('","')[1]) for l in out.splitlines() if l.count('","') >= 2}
    return [p for p in pids if p in running]


def wait_run(timeout, seen=None):
    t0 = time.time()
    while time.time() - t0 < timeout:
        st = call("/api/status")[1]
        if seen is not None:
            seen.update(st["solver_pids"])
        r = st["run"]
        if r and r["status"] in ("done", "cancelled", "error"):
            return r
        time.sleep(0.2)
    return call("/api/status")[1]["run"]


def run(mode, position, seconds, procs, lock, client="", timeout=900):
    code, j = call("/api/run", {"mode": mode, "position": position, "seconds": seconds, "procs": procs,
                                "lock": lock, "client": client})
    if code != 200:
        return code, j
    seen = set()
    r = wait_run(timeout, seen)
    time.sleep(0.3)
    return r, seen


def main():
    global BASE
    ap = argparse.ArgumentParser()
    ap.add_argument("--solver", default=str(Path("frozen") / "climb-base.exe"))
    ap.add_argument("--port", type=int, default=8799)
    ap.add_argument("--no-lock-tests", action="store_true")
    a = ap.parse_args()
    sp = Path(a.solver)
    solver = gui.Solver(sp if sp.is_absolute() else gui.ROOT / sp)
    BASE = "http://127.0.0.1:%d" % a.port
    log_before = gui.LOG_PATH.read_text().count("\n") if gui.LOG_PATH.exists() else 0
    srv = subprocess.Popen([sys.executable, "-u", str(Path(__file__).resolve().parent / "gui.py"), "--port", str(a.port),
                            "--solver", solver.path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    srv.stdout.readline()
    try:
        info = call("/api/info")[1]
        check(info["solver_found"] and info["lock"], "info: solver found, run lock found")

        # load
        two = "1:60 4B:16,27,28,39"
        code, j = call("/api/load", {"text": two})
        want = solver.quick(["text"] + two.split())[1].strip()
        check(code == 200 and j["position"] == want, "load a move list: %s" % j.get("position"))
        code, j2 = call("/api/load", {"text": want})
        check(code == 200 and j2["position"] == want, "load its position text")
        for bad in ["1:60 4B:16,27,28,40", "hello", "1:60 pass", "zz_00_255_254_0_0_0", want.replace("_254_", "_255_")]:
            code, j = call("/api/load", {"text": bad})
            check(code == 400 and j.get("error"), "refuse %r: %s" % (bad, j.get("error")))

        # moves, play, undo
        code, j = call("/api/moves", {"position": want})
        moves = j["moves"]
        check(code == 200 and len(moves) == len(solver.children([want])[0]), "moves after two moves: %d" % len(moves))
        m = moves[7]
        code, j = call("/api/load", {"text": two + " " + m["move"]})
        check(j.get("position") == m["child"], "play %s gives its child" % m["move"])
        code, j = call("/api/load", {"text": two})
        check(j.get("position") == want, "undo gives the position back")

        # a late position: ten random moves of a fixed line, then all moves (4 processes, a second or less)
        line, pos = [], want
        import random
        rng = random.Random(5)
        while len(line) < 9:
            kids = solver.children([pos])[0]
            mv, pos = rng.choice(kids)
            line.append(mv)
        r, seen = run("all", pos, 20, 4, False)
        res = call("/api/status")[1]["run"]["results"]
        vals = [e["value"] for e in res if e["move"] is not None]
        check(r["status"] == "done" and len(vals) == r["total"] and None not in vals,
              "all moves at ply %d: %d moves, all solved, %.1f s" % (gui.parse_text(pos)["ply"], len(vals), r["elapsed"]))
        check(r["value"] == max(vals), "its value %s is the best move's" % r["value"])
        check(not alive(seen), "no solver process left (%d were started)" % len(seen))

        # two runs at once
        code, j = call("/api/run", {"mode": "solve", "position": want, "seconds": 30, "procs": 2, "lock": False})
        code2, j2 = call("/api/run", {"mode": "all", "position": want, "seconds": 30, "procs": 2, "lock": False})
        check(code == 200 and code2 == 409, "a second run is refused while one runs: %s" % j2.get("error"))
        time.sleep(1.5)
        # cancel a long solve (the position after two moves takes hours)
        pids = set(call("/api/status")[1]["solver_pids"])
        call("/api/cancel", {})
        r = wait_run(30)
        time.sleep(0.5)
        check(r["status"] == "cancelled" and len(pids) == 2 and not alive(pids),
              "cancel a long solve: %s, %d processes before, %d after" % (r["status"], len(pids), len(alive(pids))))

        # a limit that is hit
        r, seen = run("solve", want, 3, 2, False)
        res = call("/api/status")[1]["run"]["results"]
        check(r["status"] == "done" and r["value"] is None and all(e["status"] == "limit" for e in res),
              "a 3 s limit at ply 2: %s, value %s, %d positions not solved" % (r["status"], r["value"], len(res)))
        check(not alive(seen), "no solver process left (%d were started)" % len(seen))

        # the page goes away: an event stream that closes
        stop = threading.Event()

        def listen():
            with urllib.request.urlopen(BASE + "/api/events?client=tab1", timeout=60) as s:
                while not stop.is_set():
                    s.readline()

        th = threading.Thread(target=listen, daemon=True)
        th.start()
        time.sleep(0.5)
        code, j = call("/api/run", {"mode": "solve", "position": want, "seconds": 600, "procs": 2, "lock": False, "client": "tab1"})
        time.sleep(2)
        pids = set(call("/api/status")[1]["solver_pids"])
        busy = call("/api/status")[1]["run"]["status"]
        stop.set()  # the reader stops; its socket closes when the thread ends
        th.join(5)
        r = wait_run(30)
        time.sleep(0.5)
        check(busy == "running" and r["status"] == "cancelled" and "went away" in r["message"] and not alive(pids),
              "a closed page cancels its run: %s (%s), %d processes left" % (r["status"], r["message"], len(alive(pids))))

        if not a.no_lock_tests:
            # a proof position four moves in: a win for the side to move, under the run lock
            p4 = Path(gui.ROOT / "tests" / "proof4" / "positions.txt")
            if not p4.exists():
                p4 = Path(solver.path).resolve().parent.parent / "tests" / "proof4" / "positions.txt"
            first = p4.read_text().splitlines()[0].split()[1]
            print("     waiting for the run lock ...", flush=True)
            r, seen = run("solve", first, 120, 16, True, timeout=3600)
            check(r["status"] == "done" and r["value"] == 1 and r["best"] and r["held_lock"],
                  "solve a ply-4 proof position: value %s, best %s, held the lock %s, %.1f s (waited %.0f s)" % (
                      r["value"], r["best"], r["held_lock"], r["elapsed"], r["waited"]))
            check(not alive(seen), "no solver process left (%d were started)" % len(seen))
            if r["best"]:
                child = [c for mv, c in solver.children([first])[0] if mv == r["best"]][0]
                v = solver.quick(["solve", child], timeout=600)[1]
                check(" -1 " in " " + v.split("value")[1][:4] + " ", "the winning move's child is a loss for the opponent: %s" % v.strip())

        lines = gui.LOG_PATH.read_text().splitlines()[log_before:]
        ends = [l for l in lines if "event=end" in l]
        check(len(ends) >= (4 if a.no_lock_tests else 5), "runs.log has %d new end lines; last: %s" % (len(ends), ends[-1] if ends else ""))
    finally:
        srv.terminate()
        srv.wait()
    print("failures: %d" % len(FAILS))
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
