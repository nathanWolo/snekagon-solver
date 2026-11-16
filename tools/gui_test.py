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
import os
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
PROCS = 16
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


def start_server(a, solver, prover, proofs, env=None):
    srv = subprocess.Popen([sys.executable, "-u", str(Path(__file__).resolve().parent / "gui.py"), "--port", str(a.port),
                            "--solver", solver.path, "--prover", str(prover), "--proofs", str(proofs)],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env)
    srv.stdout.readline()
    return srv


def test_dir(prefix):
    """A fresh folder for test stores beside the real ones (work/gui/), so that the paths the server writes are as
    long as in a real run (Windows stops at 260 characters)."""
    base = gui.ROOT / "work" / "gui"
    base.mkdir(parents=True, exist_ok=True)
    return tempfile.mkdtemp(prefix=prefix, dir=str(base))


def proof(position):
    return call("/api/proof?position=" + position)[1].get("proof")


def prove(position, lock, cancel_when=None, timeout=3600):
    """A prove run through the server; cancel_when(state) cancels it. Returns (last run snapshot, pids seen)."""
    code, j = call("/api/run", {"mode": "prove", "position": position, "procs": PROCS, "lock": lock})
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
    proofs = Path(test_dir("proofs-"))
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
        code, j = call("/api/run", {"mode": "prove", "position": WIN3, "procs": PROCS, "lock": False})
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


# Two replies of the opening that the replay proved (roots of work/replay/base4/results.txt): the first with a
# failed try and a winning one, the second with one try.
FAKE_ROOTS = ["00000000000000001000000000000000_000000000000000000000000300a0000_254_126_0_2_0",
              "00000000000000001000000000000000_00000000000000000000000060100400_254_222_0_2_0"]


def expected_tries(record, root, solver):
    """The tries of a root as an independent count from a results file: try cells -> [state, reply, lost replies]
    (a job counts when its position is a child of the try, by the solver's own move list)."""
    r = gui.parse_text(root)
    tries = {int(c.split("_")[0], 16): c for mv, c in solver.children([root])[0]}
    ro, rm = r["cells"][0], r["cells"][1]
    out, jobs = {}, {}
    for l in open(record):
        f = l.split()
        t = f[0].split("_")
        a, b, ply = int(t[0], 16), int(t[1], 16), int(t[5])
        if a & ro != ro or a == ro:
            continue
        if ply == 3 and b == rm:
            e = out.setdefault(a, ["work", None, 0])
            if int(f[2]) <= -1:
                e[0] = "won"
            elif int(f[1]) >= 0:
                e[0], e[1] = "failed", f[5]
        elif ply == 4 and b & rm == rm and b != rm:
            out.setdefault(a, ["work", None, 0])
            jobs.setdefault(f[0], [a, False])
            if int(f[1]) >= 1:
                jobs[f[0]][1] = True
    keys = sorted(out)
    kids = dict(zip(keys, ({c for mv, c in k} for k in solver.children([tries[a] for a in keys]))))
    for j, (a, lost) in jobs.items():
        out[a][2] += lost and j in kids[a]
    return out


def fake_tests(a, solver):
    """The tries of the reply in work, with a stand-in driver (tools/fake_prover.py) that plays back the replay's
    records of two replies of the opening."""
    global SRV
    record = from_main(Path("work") / "replay" / "base4" / "results.txt", solver)
    proofs = Path(test_dir("fake-"))
    try:
        # the tries of a root read from a whole file, against an independent count
        path = proofs / "record.txt"
        shutil.copy(record, path)
        want = expected_tries(path, FAKE_ROOTS[0], solver)
        tr = gui.Tries(solver, FAKE_ROOTS[0], path)
        tr.summary()
        red = gui.parse_text(FAKE_ROOTS[0])["cells"][0]
        got = {red | sum(1 << c for c in t["cells"]): t for t in tr.tries.values()}
        ok = set(got) == set(want) and all(got[k]["state"] == w[0] and got[k]["reply"] == w[1] and got[k]["done"] == w[2]
                                           and got[k]["total"] and got[k]["total"] >= w[2] for k, w in want.items())
        check(ok, "tries of a root from its records: %s" % "; ".join("%s %s %d/%s %s" % (t["move"], t["state"], t["done"], t["total"], t["reply"] or "")
                                                                   for t in tr.tries.values()))
        # read on from a byte offset: the same file written in two halves, the cut inside a line
        half = proofs / "half.txt"
        data = path.read_bytes()
        half.write_bytes(data[: len(data) // 2])
        tr2 = gui.Tries(solver, FAKE_ROOTS[0], half)
        with open(half, "ab") as f:
            f.write(data[len(data) // 2:])
        tr2.read()
        check(tr2.summary() == tr.summary(), "read on from an offset gives the same tries")

        # a store of the two replies; the server's driver is the stand-in
        store = proofs / "stores" / gui.store_key(gui.OPENING)
        store.mkdir(parents=True)
        p = gui.parse_text(gui.OPENING)
        roots = [[gui.move_between(p, gui.parse_text(c)), c] for c in FAKE_ROOTS]
        (store / "roots.txt").write_text("".join("? %s\n" % c for m, c in roots))
        (store / "results.txt").write_text("")
        (store / "meta.json").write_text(json.dumps({"position": gui.OPENING, "roots": roots, "seeded": None, "created": "-",
                                                     "seconds": [0.0, 0.0], "finished": [], "runs": 0}))
        SRV.terminate()
        SRV.wait()
        env = dict(os.environ, FAKE_RECORD=str(record), FAKE_DELAY="0.004")
        SRV = start_server(a, solver, Path(__file__).resolve().parent / "fake_prover.py", proofs / "stores", env)

        states = []
        stop = threading.Event()

        def listen():
            try:
                with urllib.request.urlopen(BASE + "/api/events?client=fake", timeout=60) as s:
                    ev = None
                    while not stop.is_set():
                        line = s.readline().decode()
                        if line.startswith("event: "):
                            ev = line[7:].strip()
                        elif line.startswith("data: ") and ev == "state":
                            states.append((time.time(), json.loads(line[6:])))
            except OSError:
                pass

        th = threading.Thread(target=listen, daemon=True)
        th.start()
        time.sleep(0.5)
        code, j = call("/api/run", {"mode": "prove", "position": gui.OPENING, "procs": 2, "lock": False, "client": "fake"})
        seen = []
        t0 = time.time()
        while time.time() - t0 < 60:
            r = call("/api/status")[1]["run"]
            t = r["prove"].get("tries")
            if t:
                seen.append(t)
            if t and sum(x["done"] for x in t["list"]) >= 150:
                break
            time.sleep(0.2)
        call("/api/cancel", {})
        mid = seen[-1] if seen else None
        r = wait_run(30)
        check(r["status"] == "cancelled" and mid and mid["root"] == 0 and mid["work"] >= 1 and mid["best"] is not None
              and all(x["total"] and x["total"] >= x["done"] for x in mid["list"]),
              "tries while the first reply is in work: %s" % (mid and "; ".join("%s %s %d/%s" % (x["move"], x["state"], x["done"], x["total"]) for x in mid["list"])))
        grew = [sum(x["done"] for x in s["list"]) for s in seen]
        check(grew == sorted(grew) and len(set(grew)) > 1, "the counts grow: %s" % grew[:12])

        # prove again: the tries are built from the file first
        code, j = call("/api/run", {"mode": "prove", "position": gui.OPENING, "procs": 2, "lock": False, "client": "fake"})
        first = None
        t0 = time.time()
        while time.time() - t0 < 30 and first is None:
            t = call("/api/status")[1]["run"]["prove"].get("tries")
            if t and t["root"] == 0:
                first = t
            time.sleep(0.1)
        done_mid = sum(x["done"] for x in mid["list"]) if mid else 0
        check(first and sum(x["done"] for x in first["list"]) >= done_mid,
              "proved again: the first tries come from the file (%s lost replies, %s before the cancel)" % (
                  first and sum(x["done"] for x in first["list"]), done_mid))
        r = wait_run(60)
        p = r["prove"]
        check(r["status"] == "done" and p["refuted"] == 2 and r["value"] == -1, "both replies refuted: %s, %s of %s" % (r["status"], p["refuted"], p["n"]))
        time.sleep(1.5)
        stop.set()
        gaps = [b[0] - a_[0] for a_, b in zip(states, states[1:])]
        check(len(states) >= 3 and min(gaps) >= 0.9, "state events at most once a second: %d events, smallest gap %.2f s" % (len(states), min(gaps) if gaps else -1))
        with_tries = [s for _, s in states if s.get("prove") and s["prove"].get("tries")]
        check(with_tries and {s["prove"]["tries"]["root"] for s in with_tries} == {0, 1},
              "the events carry the tries of each reply in work (%d events with tries)" % len(with_tries))
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
    ap.add_argument("--only-fake", action="store_true", help="only the tests with the stand-in driver")
    ap.add_argument("--procs", type=int, default=16, help="processes of the prove tests' driver")
    a = ap.parse_args()
    global PROCS
    PROCS = a.procs
    a.lock = not a.no_lock_tests
    sp = Path(a.solver)
    solver = gui.Solver(sp if sp.is_absolute() else gui.ROOT / sp)
    BASE = "http://127.0.0.1:%d" % a.port
    log_before = gui.LOG_PATH.read_text().count("\n") if gui.LOG_PATH.exists() else 0
    first_store = Path(test_dir("proofs-"))
    SRV = start_server(a, solver, from_main(a.prover, solver), first_store)
    try:
        if a.only_prove or a.only_fake:
            fake_tests(a, solver)
            if a.only_prove:
                prove_tests(a, solver)
            SRV.terminate()
            SRV.wait()
            shutil.rmtree(first_store, ignore_errors=True)
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
            fake_tests(a, solver)
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
        shutil.rmtree(first_store, ignore_errors=True)
    finish()


def finish():
    print("failures: %d" % len(FAILS))
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
