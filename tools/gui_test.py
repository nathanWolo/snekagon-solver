"""Exercises the analysis board's server (tools/gui.py) through its HTTP endpoints.

    python tools/gui_test.py [--solver frozen/climb-base.exe] [--prover frozen/prove-replay.exe] [--port 8799]
                             [--no-lock-tests] [--no-prove-tests] [--only-prove]

Starts its own server, then: load (good and bad input), the moves of a position, play and undo (by
loading a line one move longer and shorter), all moves of a late position, a solve of a proof position
four moves in (through the run lock, 16 processes), a time limit that is hit, a cancelled long solve,
a run whose page goes away, and two runs at once. Checks that no solver process outlives a run and that
every run is in work/gui/runs.log. The locked solve waits for the machine's run lock.

Prove (on a fresh store directory): the position after 1:60 read from the paused proof without a search;
a ply-3 red try that wins, cancelled a third of the way and proved again to the end (checked against the
replay's results, three answers checked by the solver); a ply-3 try that failed, which stops at the reply
that is not refuted (checked by the solver), and stops at once the second time; the stores after a server
restart; a killed server takes its driver with it. A few minutes under the run lock.
"""
import argparse
import json
import shutil
import subprocess
import sys
import tempfile
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


def from_main(rel, solver):
    """A file that only the main checkout has (frozen/, work/): here, or beside the solver's checkout."""
    p = gui.ROOT / rel
    return p if p.exists() else Path(solver.path).resolve().parent.parent / rel


def start_server(a, solver, prover, proofs):
    srv = subprocess.Popen([sys.executable, "-u", str(Path(__file__).resolve().parent / "gui.py"), "--port", str(a.port),
                            "--solver", solver.path, "--prover", str(prover), "--proofs", str(proofs)],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    srv.stdout.readline()
    return srv


def proof(position):
    return call("/api/proof?position=" + position)[1].get("proof")


def prove(position, lock, cancel_when=None, timeout=3600):
    """A prove run through the server; cancel_when(state) cancels it. Returns (last run snapshot, pids seen)."""
    code, j = call("/api/run", {"mode": "prove", "position": position, "procs": 16, "lock": lock})
    if code != 200:
        return {"status": "refused", "message": j.get("error"), "prove": None}, set()
    t0, seen, cancelled = time.time(), set(), False
    while time.time() - t0 < timeout:
        st = call("/api/status")[1]
        seen.update(st["solver_pids"])
        r = st["run"]
        if r["status"] in ("done", "cancelled", "error"):
            break
        if cancel_when and not cancelled and r["status"] == "running" and cancel_when(r):
            call("/api/cancel", {})
            cancelled = True
        time.sleep(0.3)
    time.sleep(0.5)
    return call("/api/status")[1]["run"], seen


# Ply-3 positions of the replay's results (work/replay/base4/results.txt), blue to move, among the cheapest there:
# a red try that wins (-1 -1: every blue reply loses) and one that failed (0 1: some reply is not a red win).
WIN3 = "00000080100400801000000000000000_00000000000000000000000060100400_190_222_1_3_0"
FAIL3 = "00000000000000001002018040000000_00000000000000000018040100000000_190_222_1_3_0"


def prove_tests(a, solver):
    """Mode prove, on a fresh store directory, with a server restart in the middle."""
    global SRV
    prover = Path(a.prover) if Path(a.prover).is_absolute() else from_main(a.prover, solver)
    base4 = from_main(Path("work") / "replay" / "base4" / "results.txt", solver)
    known, _ = gui.read_results(base4)
    check(known.get(WIN3, [0, 0])[1] == -1 and known.get(FAIL3, [0, 0])[0] == 0, "the test positions are in %s" % base4)
    proofs = Path(tempfile.mkdtemp(prefix="gui-proofs-"))
    try:
        info = call("/api/info")[1]
        SRV.terminate()
        SRV.wait()
        SRV = start_server(a, solver, prover, proofs)
        info = call("/api/info")[1]
        check(info["prover_found"], "info: proof driver found at %s" % info["prover"])
        check(proof(WIN3) is None, "no store yet: no proof state")
        seed = gui.SEEDS[gui.OPENING]
        if seed.is_file():
            t0 = time.time()
            s = proof(gui.OPENING)
            check(s and s["n"] == 172 and s["refuted"] == 74 and s["open"] == 98 and not s["held"] and not s["exists"]
                  and not call("/api/status")[1]["solver_pids"],
                  "the position after 1:60 from %s: refuted %s of %s, nothing searched, %.1f s" % (seed, s and s["refuted"], s and s["n"], time.time() - t0))
        print("     prove tests: waiting for the run lock if it is held ...", flush=True)

        # a winning try, cancelled when a third of its replies are refuted, then proved again
        r, seen = prove(WIN3, a.lock, cancel_when=lambda r: r["prove"]["refuted"] >= r["prove"]["n"] // 3)
        p = r["prove"]
        part = p["refuted"] if p else 0
        check(r["status"] == "cancelled" and 0 < part < p["n"] and not alive(seen),
              "prove a winning red try, cancelled: %s, %d of %d refuted, %d driver processes left" % (r["status"], part, p["n"] if p else 0, len(alive(seen))))
        mid = proof(WIN3)
        check(mid and mid["exists"] and mid["refuted"] >= part and mid["open"] == mid["n"] - mid["refuted"],
              "its store read without a run: refuted %s of %s" % (mid and mid["refuted"], mid and mid["n"]))
        r, seen = prove(WIN3, a.lock)
        p = r["prove"]
        ok = p and p["refuted"] == p["n"] and not p["held"] and all(x["state"] == "refuted" and x["answer"] for x in p["rows"])
        check(r["status"] == "done" and ok and r["value"] == -1 and not alive(seen),
              "proved again, it goes on: %s, refuted %d of %d, value %s, %.1f s, %d driver processes left" % (
                  r["status"], p["refuted"], p["n"], r["value"], r["elapsed"], len(alive(seen))))
        agree = [x for x in p["rows"] if x["child"] in known]
        check(all(known[x["child"]][0] >= 1 for x in agree), "every reply agrees with base4 where it knows it (%d)" % len(agree))
        for x in p["rows"][:: max(1, len(p["rows"]) // 3)][:3]:
            after = [c for mv, c in solver.children([x["child"]])[0] if mv == x["answer"]][0]
            v = solver.quick(["solve", after], timeout=600)[1]
            check(v.startswith("value -1 "), "reply %s, answer %s: the position after it is lost for the mover (%s)" % (x["move"], x["answer"], v.strip()))

        # a failed try: the run stops at the reply that is not refuted
        r, seen = prove(FAIL3, a.lock)
        p = r["prove"]
        held = [x for x in p["rows"] if x["state"] == "held"]
        before = [x for x in p["rows"] if x["state"] == "refuted"]
        check(r["status"] == "done" and len(held) == 1 and p["open"] == p["n"] - len(before) - 1 and held[0]["move"] in r["message"]
              and not alive(seen), "prove a failed red try: %s, stopped at %s (%s), %d refuted before, %.1f s" % (
                  r["status"], held and held[0]["move"], r["message"], len(before), r["elapsed"]))
        if held:
            k = known.get(held[0]["child"])
            v = solver.quick(["solve", held[0]["child"]], timeout=900)[1]
            check(v.startswith("value 0 ") or v.startswith("value -1 "),
                  "the reply that is not refuted is not a win for its mover: %s (base4: %s)" % (v.strip(), k))
        r2, seen = prove(FAIL3, a.lock)
        check(r2["status"] == "done" and held and held[0]["move"] in r2["message"] and not seen,
              "proved again: stops at once at the same reply, no driver started (%s)" % r2["message"])

        # the stores outlive the server; a driver dies with it
        SRV.terminate()
        SRV.wait()
        SRV = start_server(a, solver, prover, proofs)
        s = proof(WIN3)
        check(s and s["exists"] and s["refuted"] == s["n"] and not s["live"], "after a server restart: refuted %s of %s" % (s and s["refuted"], s and s["n"]))
        s = proof(FAIL3)
        check(s and len(s["held"]) == 1, "after a server restart: the failed try still holds at %s" % (s and s["held"]))
        shutil.rmtree(proofs / gui.store_key(WIN3))
        code, j = call("/api/run", {"mode": "prove", "position": WIN3, "procs": 16, "lock": False})
        t0, pids = time.time(), set()
        while time.time() - t0 < 30 and not pids:
            pids = set(call("/api/status")[1]["solver_pids"])
            time.sleep(0.2)
        time.sleep(1)
        SRV.kill()  # no clean exit: the job object ends the driver
        SRV.wait()
        time.sleep(1)
        check(pids and not alive(pids), "a server that is killed takes its driver with it: %d before, %d after" % (len(pids), len(alive(pids))))
        SRV = start_server(a, solver, prover, proofs)
    finally:
        shutil.rmtree(proofs, ignore_errors=True)


def main():
    global BASE, SRV
    ap = argparse.ArgumentParser()
    ap.add_argument("--solver", default=str(Path("frozen") / "climb-base.exe"))
    ap.add_argument("--prover", default=str(Path("frozen") / "prove-replay.exe"))
    ap.add_argument("--port", type=int, default=8799)
    ap.add_argument("--no-lock-tests", action="store_true")
    ap.add_argument("--no-prove-tests", action="store_true")
    ap.add_argument("--only-prove", action="store_true", help="only the prove tests")
    a = ap.parse_args()
    a.lock = not a.no_lock_tests
    sp = Path(a.solver)
    solver = gui.Solver(sp if sp.is_absolute() else gui.ROOT / sp)
    BASE = "http://127.0.0.1:%d" % a.port
    log_before = gui.LOG_PATH.read_text().count("\n") if gui.LOG_PATH.exists() else 0
    SRV = start_server(a, solver, from_main(a.prover, solver), Path(tempfile.mkdtemp(prefix="gui-proofs-")))
    try:
        if a.only_prove:
            prove_tests(a, solver)
            return finish()

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

        if not a.no_prove_tests:
            prove_tests(a, solver)

        lines = gui.LOG_PATH.read_text().splitlines()[log_before:]
        ends = [l for l in lines if "event=end" in l]
        check(len(ends) >= (4 if a.no_lock_tests else 5), "runs.log has %d new end lines; last: %s" % (len(ends), ends[-1] if ends else ""))
        if not a.no_prove_tests:
            pr = [l for l in lines if "event=end" in l and "mode=prove" in l]
            check(len(pr) >= 4, "runs.log has %d new prove end lines" % len(pr))
    finally:
        SRV.terminate()
        SRV.wait()
    finish()


def finish():
    print("failures: %d" % len(FAILS))
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
