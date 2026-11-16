"""The analysis board: a local web page that sets up Snekagon positions and runs the solver on them.

    python tools/gui.py [--port 8765] [--solver frozen/climb-base.exe] [--prover frozen/prove-replay.exe]

Open http://localhost:8765. The page (gui/) knows the rules itself (gui/rules.js, checked against the
solver by tools/gui_check.py); this server runs the solver. Every solve is one `solve <position>` process
per position, several at once, each killed when it reaches the time limit or the run is cancelled.

Prove (mode prove) shows that the side to move at P loses: the proof driver (--prover, a build of branch
climb/replay) refutes P's replies one after another, one reply for each class under P's symmetries, as the
replies of one proof (`prove ROOTS --replay`, the measured top-of-proof configuration, PROVE_ARGS). Its store
is work/gui/proofs/<key of P>/ (roots.txt, results.txt, meta.json); results.txt is the driver's results file and
the proof's only state, so a cancelled or yielded run goes on from it. The position after 1:60 starts from a
copy of the paused proof's work/long/ply1_results.txt (read, never written). A reply that is not refuted
stops the run. GET /api/proof?position=P reads a store without a run.

A run waits for the machine's run lock (the protocol of tools/lockfair.sh, done here in Python: a ticket
in .git/run.queue, the lock directory .git/run.lock), one lock for the whole run, unless the page says
"run now". Its ticket is a priority ticket ("0-<time>-<pid>", which sorts before every ordinary
ticket), so it waits only for the command that holds the lock now, never for the whole queue. A run that holds the lock for more than 15 minutes while others wait stops starting new
solves, lets the running ones finish, gives the lock back and queues again. Every run is logged to
work/gui/runs.log. Solver processes live in a Windows job object, so they die with this server.
"""
import argparse
import hashlib
import json
import os
import queue
import re
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GUI_DIR = ROOT / "gui"
LOG_PATH = ROOT / "work" / "gui" / "runs.log"
YIELD_AFTER = float(os.environ.get("GUI_YIELD_AFTER", 15 * 60))  # seconds under the lock before giving it back to waiters
OWNER_GONE = 4.0  # seconds without the page's event stream before its run is cancelled

# Measured on this PC, 2026-11-15, frozen/climb-base.exe: 16 positions per ply from random play, one
# `solve` process each, 16 at once (as the page runs them), 90 s limit. Ply 2 and earlier: the project's
# own runs (the position after two moves took hours on 16 threads; the start is unsolved).
# Ply 3: 0 of 16 solved. Ply 4: 15 of 16, median 8.6 s, max 33 s. Ply 5: median 0.6 s, max 19 s.
# Plies 6, 7, 8, 10, 12: medians 0.10 to 0.15 s, max 0.9 s.
# ply of the position solved -> what the page shows.
TYPICAL = {0: "unsolved", 1: "unsolved", 2: "hours on 16 threads", 3: "0 of 16 solved in 90 s",
           4: "median 8.6 s, max 33 s", 5: "median 0.6 s, max 19 s"}
TYPICAL.update({k: "under 1 s" for k in range(6, 40)})

# ---------------------------------------------------------------- rules (for checks and move names)
R, W, GRID = 5, 11, 121
PIECE_NAME = ["1", "2", "3I", "3V", "4I", "4A", "4B", "4C"]
PIECE_SIZE = [1, 2, 3, 3, 4, 4, 4, 4]


def _on(q, r):
    return abs(q) <= R and abs(r) <= R and abs(q + r) <= R


ON = [_on(i % W - R, i // W - R) for i in range(GRID)]
CELLS = [i for i in range(GRID) if ON[i]]
NB = [[] for _ in range(GRID)]
for _i in CELLS:
    for _dq, _dr in ((1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)):
        _q, _r = _i % W - R + _dq, _i // W - R + _dr
        if _on(_q, _r):
            NB[_i].append((_r + R) * W + _q + R)


class BadPosition(ValueError):
    pass


def parse_text(s):
    """A checked position text -> dict; raises BadPosition with a message."""
    f = s.strip().split("_")
    if len(f) != 7:
        raise BadPosition("a position text has 7 fields separated by _")
    cells = []
    for side in range(2):
        if not re.fullmatch(r"[0-9a-fA-F]{1,32}", f[side]):
            raise BadPosition("cells must be up to 32 hex digits")
        cells.append(int(f[side], 16))
    for side in range(2):
        m = cells[side]
        for i in range(128):
            if m >> i & 1 and (i >= GRID or not ON[i]):
                raise BadPosition("cell %d is not on the board" % i)
    if cells[0] & cells[1]:
        raise BadPosition("a cell is both red and blue")

    def num(x, lo, hi, what):
        if not re.fullmatch(r"\d+", x) or not lo <= int(x) <= hi:
            raise BadPosition("%s must be a number from %d to %d" % (what, lo, hi))
        return int(x)

    left = [num(f[2], 0, 255, "red pieces"), num(f[3], 0, 254, "blue pieces")]
    if left[1] & 1:
        raise BadPosition("blue has no one-cell piece")
    stm, ply, passes = num(f[4], 0, 1, "side"), num(f[5], 0, 255, "ply"), num(f[6], 0, 2, "passes")
    for side in range(2):
        full = 0xFF if side == 0 else 0xFE
        used = sum(PIECE_SIZE[k] for k in range(8) if full >> k & 1 and not left[side] >> k & 1)
        have = bin(cells[side]).count("1")
        who = "red" if side == 0 else "blue"
        if have != used:
            raise BadPosition("%s has %d cells but has placed pieces of %d cells" % (who, have, used))
        for c in CELLS:
            if not cells[side] >> c & 1:
                continue
            a = [n for n in NB[c] if cells[side] >> n & 1]
            if len(a) > 2 or (len(a) == 2 and a[1] in NB[a[0]]):
                raise BadPosition("%s's cells are not snakes (at cell %d)" % (who, c))
    return {"cells": cells, "left": left, "stm": stm, "ply": ply, "passes": passes}


def to_text(p):
    return "%032x_%032x_%d_%d_%d_%d_%d" % (p["cells"][0], p["cells"][1], p["left"][0], p["left"][1], p["stm"], p["ply"], p["passes"])


def is_over(p):
    return p["passes"] >= 2 or (p["left"][0] | p["left"][1]) == 0


def pass_child(p):
    return to_text(dict(p, stm=p["stm"] ^ 1, ply=p["ply"] + 1, passes=p["passes"] + 1))


def move_between(p, c):
    """The move that leads from position p to its child c (both parsed)."""
    me = p["stm"]
    cells = c["cells"][me] & ~p["cells"][me]
    if not cells:
        return "pass"
    piece = (p["left"][me] & ~c["left"][me]).bit_length() - 1
    idx = []
    while cells:
        low = cells & -cells
        idx.append(str(low.bit_length() - 1))
        cells ^= low
    return "%s:%s" % (PIECE_NAME[piece], ",".join(idx))


def order_key(p, move):
    """The solver's static order: bigger pieces, growing an own snake, touching the opponent."""
    if move == "pass":
        return 0
    cells = [int(x) for x in move.split(":")[1].split(",")]
    me = p["stm"]
    border = {n for c in cells for n in NB[c]} - set(cells)
    grow = any(p["cells"][me] >> n & 1 for n in border)
    touch = any(p["cells"][me ^ 1] >> n & 1 for n in border)
    return -(len(cells) * 64 + 256 * grow + 32 * touch)


# ---------------------------------------------------------------- processes
NO_WINDOW = 0x08000000 if os.name == "nt" else 0


class KillOnClose:
    """A Windows job object: every process assigned to it dies when this server's process ends."""

    def __init__(self):
        self.handle = None
        if os.name != "nt":
            return
        try:
            import ctypes
            from ctypes import wintypes

            class Basic(ctypes.Structure):
                _fields_ = [("a", ctypes.c_int64), ("b", ctypes.c_int64), ("LimitFlags", wintypes.DWORD),
                            ("c", ctypes.c_size_t), ("d", ctypes.c_size_t), ("e", wintypes.DWORD),
                            ("f", ctypes.c_size_t), ("g", wintypes.DWORD), ("h", wintypes.DWORD)]

            class Extended(ctypes.Structure):
                _fields_ = [("basic", Basic), ("io", ctypes.c_uint64 * 6), ("m1", ctypes.c_size_t),
                            ("m2", ctypes.c_size_t), ("m3", ctypes.c_size_t), ("m4", ctypes.c_size_t)]

            k32 = ctypes.WinDLL("kernel32", use_last_error=True)
            k32.CreateJobObjectW.restype = wintypes.HANDLE
            k32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
            k32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
            h = k32.CreateJobObjectW(None, None)
            info = Extended()
            info.basic.LimitFlags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            if h and k32.SetInformationJobObject(h, 9, ctypes.byref(info), ctypes.sizeof(info)):
                self.handle, self.k32 = h, k32
        except Exception as e:  # without it, processes are still killed on cancel and on a clean exit
            print("no job object:", e, file=sys.stderr)

    def add(self, proc):
        if self.handle:
            self.k32.AssignProcessToJobObject(self.handle, int(proc._handle))


JOB = KillOnClose()
LIVE = set()  # every solver process running now
LIVE_LOCK = threading.Lock()


def start_proc(args, merge=False, cwd=None):
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT if merge else subprocess.PIPE,
                         stdin=subprocess.DEVNULL, creationflags=NO_WINDOW, cwd=str(cwd or ROOT))
    JOB.add(p)
    with LIVE_LOCK:
        LIVE.add(p)
    return p


def end_proc(p):
    if p.poll() is None:
        p.kill()
    try:
        out, err = p.communicate(timeout=10)
    except subprocess.TimeoutExpired:
        out, err = b"", b""
    with LIVE_LOCK:
        LIVE.discard(p)
    return out.decode(errors="replace"), err.decode(errors="replace")


def kill_all():
    with LIVE_LOCK:
        procs = list(LIVE)
    for p in procs:
        try:
            p.kill()
        except OSError:
            pass


class Solver:
    def __init__(self, path):
        self.path = str(path)

    def exists(self):
        return Path(self.path).is_file()

    def quick(self, args, timeout=60):
        p = start_proc([self.path] + args)
        t0 = time.time()
        while p.poll() is None and time.time() - t0 < timeout:
            time.sleep(0.005)
        out, err = end_proc(p)
        return p.returncode, out, err

    def children(self, texts):
        """For each position text: the list of (move, child text), as the solver generates them."""
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            for k, t in enumerate(texts):
                f.write("%d %s\n" % (k, t))
            name = f.name
        try:
            p = subprocess.run([self.path, "children", name], capture_output=True, text=True,
                               creationflags=NO_WINDOW, cwd=str(ROOT), timeout=600)
        finally:
            os.unlink(name)
        if p.returncode != 0:
            raise RuntimeError("the solver's children command failed: " + p.stderr.strip()[-300:])
        out = [[] for _ in texts]
        parsed = [parse_text(t) for t in texts]
        for line in p.stdout.splitlines():
            k, child = line.split()
            k = int(k)
            f = child.split("_")  # the solver's own output: read, not checked
            c = {"cells": [int(f[0], 16), int(f[1], 16)], "left": [int(f[2]), int(f[3])]}
            out[k].append((move_between(parsed[k], c), child))
        for k, pp in enumerate(parsed):
            if not out[k] and not is_over(pp):
                out[k].append(("pass", pass_child(pp)))
        return out


# ---------------------------------------------------------------- the run lock (tools/lockfair.sh)
class RunLock:
    def __init__(self):
        self.common = None
        try:
            g = subprocess.run(["git", "rev-parse", "--git-common-dir"], capture_output=True, text=True,
                               cwd=str(ROOT), creationflags=NO_WINDOW, timeout=20)
            if g.returncode == 0 and g.stdout.strip():
                d = Path(g.stdout.strip())
                self.common = d if d.is_absolute() else (ROOT / d).resolve()
        except (OSError, subprocess.SubprocessError):
            pass
        if self.common is None and (ROOT / ".git").is_dir():
            self.common = ROOT / ".git"
        if os.environ.get("RUN_LOCK_DIR"):
            self.common = Path(os.environ["RUN_LOCK_DIR"])
        self.ticket = None
        self.held = False
        self.since = 0.0

    def available(self):
        return self.common is not None

    @property
    def lock(self):
        return self.common / "run.lock"

    @property
    def queue(self):
        return self.common / "run.queue"

    def others_waiting(self):
        try:
            return any(n != (self.ticket.name if self.ticket else None) for n in os.listdir(self.queue))
        except OSError:
            return False

    def acquire(self, give_up, on_wait):
        """Waits for the lock. Returns True when held, False when give_up() said to stop waiting."""
        self.queue.mkdir(parents=True, exist_ok=True)
        # A priority ticket: "0-" sorts before the timestamps of ordinary tickets, so the owner's run
        # goes next, after the command that holds the lock now (tools/lockfair.sh: the first name wins).
        self.ticket = self.queue / ("0-%d-%d" % (time.time_ns(), os.getpid()))
        self.ticket.touch()
        try:
            while True:
                self.ticket.touch()
                now = time.time()
                names = []
                for n in os.listdir(self.queue):
                    f = self.queue / n
                    try:
                        if now - f.stat().st_mtime > 180 and f != self.ticket:
                            f.unlink()
                            continue
                    except OSError:
                        continue
                    names.append(n)
                names.sort()
                if names and names[0] == self.ticket.name:
                    try:
                        os.mkdir(self.lock)
                        self.held, self.since = True, time.time()
                        return True
                    except FileExistsError:
                        try:
                            if now - self.lock.stat().st_mtime > 20 * 60:
                                os.rmdir(self.lock)
                                continue
                        except OSError:
                            pass
                    except OSError:
                        pass
                ahead = names.index(self.ticket.name) if self.ticket.name in names else 0
                on_wait(ahead, self.lock.exists())
                for _ in range(20):  # a 2 second wait, checked every 0.1 s
                    if give_up():
                        return False
                    time.sleep(0.1)
        finally:
            try:
                self.ticket.unlink()
            except OSError:
                pass
            self.ticket = None

    def refresh(self):
        if self.held:
            try:
                os.utime(self.lock)
            except OSError:
                pass

    def release(self):
        if self.held:
            self.held = False
            try:
                os.rmdir(self.lock)
            except OSError:
                pass


# ---------------------------------------------------------------- runs
def now_iso():
    return datetime.now().isoformat(timespec="seconds")


def log_line(fields):
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(LOG_PATH, "a", encoding="utf-8") as f:
        f.write(" ".join("%s=%s" % kv for kv in dict(fields, server=os.getpid()).items()) + "\n")


class Run:
    def __init__(self, rid, solver, mode, pos_text, seconds, procs, use_lock, owner):
        self.id, self.solver, self.mode, self.pos_text = rid, solver, mode, pos_text
        self.pos = parse_text(pos_text)
        self.seconds, self.procs, self.use_lock, self.owner = seconds, procs, use_lock, owner
        self.status = "starting"
        self.message = ""
        self.cv = threading.Condition()
        self.events = []  # results, in the order they arrive
        self.jobs = []  # {"move", "child"}; the root job has move None
        self.next = 0
        self.running = 0
        self.stopping = False
        self.cancelled = False
        self.run_now = False
        self.yielding = False
        self.root_value = None
        self.best = None
        self.nodes = 0
        self.done = 0
        self.ahead = 0
        self.lock = RunLock() if use_lock else None
        self.held_lock = False
        self.created = time.time()
        self.began = None
        self.ended = None
        self.max_running = 0

    # --- state for the page
    def snapshot(self, with_results=False):
        with self.cv:
            s = {"id": self.id, "mode": self.mode, "position": self.pos_text, "status": self.status,
                 "message": self.message, "total": len(self.jobs) - (1 if self.mode == "solve" else 0),
                 "done": self.done, "running": self.running, "nodes": self.nodes, "ahead": self.ahead,
                 "held_lock": self.held_lock, "value": self.root_value, "best": self.best, "procs": self.procs,
                 "seconds": self.seconds, "elapsed": round((self.ended or time.time()) - (self.began or time.time()), 1),
                 "waited": round((self.began or self.ended or time.time()) - self.created, 1)}
            if with_results:
                s["results"] = list(self.events)
            return s

    def cancel(self, why="cancelled"):
        with self.cv:
            if self.status in ("done", "cancelled", "error"):
                return
            self.cancelled = True
            self.stopping = True
            self.message = why
            self.cv.notify_all()

    # --- the work
    def take_job(self):
        with self.cv:
            while True:
                if self.stopping:
                    return None
                if self.yielding:
                    self.cv.wait(0.2)
                    continue
                if (self.lock and self.lock.held and time.time() - self.lock.since > YIELD_AFTER
                        and self.lock.others_waiting()):
                    self.yielding = True
                    self.cv.notify_all()
                    continue
                if self.next >= len(self.jobs):
                    return None
                j = self.next
                self.next += 1
                self.running += 1
                self.max_running = max(self.max_running, self.running)
                return j

    def worker(self):
        while True:
            j = self.take_job()
            if j is None:
                return
            job = self.jobs[j]
            res = self.solve_one(job["child"] if job["move"] is not None else self.pos_text)
            with self.cv:
                self.running -= 1
                if res["status"] == "cancelled":
                    self.cv.notify_all()
                    continue
                v = res.get("value")
                if job["move"] is not None and v is not None:
                    v = -v
                ev = {"move": job["move"], "child": job["child"], "value": v, "status": res["status"],
                      "nodes": res.get("nodes"), "seconds": res.get("seconds"), "error": res.get("error")}
                self.events.append(ev)
                self.nodes += res.get("nodes") or 0
                if job["move"] is not None:
                    self.done += 1
                self.after_result(job, v)
                self.cv.notify_all()

    def after_result(self, job, v):
        if job["move"] is None:
            self.root_value = v
            # not a win (no move to look for) or not solved in time: the answer is in
            if self.mode == "solve" and v != 1:
                self.stopping = True
            return
        if v == 1 and self.best is None:
            self.best = job["move"]
            self.root_value = 1
            if self.mode == "solve":
                self.stopping = True

    def solve_one(self, text):
        t0 = time.time()
        try:
            p = start_proc([self.solver.path, "solve", text])
        except OSError as e:
            return {"status": "error", "error": str(e)}
        limit_hit = False
        while p.poll() is None:
            if self.stopping:
                end_proc(p)
                return {"status": "cancelled"}
            if self.seconds and time.time() - t0 > self.seconds:
                limit_hit = True
                break
            time.sleep(0.05)
        out, err = end_proc(p)
        if limit_hit:
            return {"status": "limit", "seconds": round(time.time() - t0, 2)}
        m = re.search(r"value (-?\d+) nodes (\d+) seconds ([0-9.]+)", out)
        if p.returncode != 0 or not m:
            return {"status": "error", "error": (err.strip() or out.strip())[-300:]}
        v = int(m.group(1))
        res = {"nodes": int(m.group(2)), "seconds": float(m.group(3))}
        if v == -9:
            return dict(res, status="limit")
        return dict(res, status="solved", value=v)

    def set_status(self, s):
        with self.cv:
            self.status = s
            self.cv.notify_all()

    def wait_lock(self):
        """Returns False if the run was cancelled while it waited."""
        if not self.lock:
            return True
        if not self.lock.available():
            with self.cv:
                self.message = "no run lock found; running without it"
            return True
        self.set_status("waiting")

        def on_wait(ahead, busy):
            with self.cv:
                self.ahead = ahead
                self.cv.notify_all()

        got = self.lock.acquire(lambda: self.cancelled or self.run_now, on_wait)
        if self.cancelled:
            self.lock.release()
            return False
        if got:
            self.held_lock = True
        else:
            with self.cv:
                self.message = "running without the lock"
        return True

    def main(self):
        try:
            if self.mode == "all" or self.mode == "solve":
                kids = self.solver.children([self.pos_text])[0]
                kids.sort(key=lambda mc: order_key(self.pos, mc[0]))
                if self.mode == "solve":
                    self.jobs = [{"move": None, "child": None}]
                self.jobs += [{"move": m, "child": c} for m, c in kids]
            if not self.wait_lock():
                self.finish("cancelled")
                return
            self.began = time.time()
            self.set_status("running")
            log_line({"time": now_iso(), "event": "start", "run": self.id, "mode": self.mode,
                      "lock": "held" if self.held_lock else "none", "procs": self.procs,
                      "limit": self.seconds, "positions": len(self.jobs), "position": self.pos_text})
            threads = [threading.Thread(target=self.worker, daemon=True) for _ in range(min(self.procs, max(1, len(self.jobs))))]
            for t in threads:
                t.start()
            last_refresh = time.time()
            while any(t.is_alive() for t in threads):
                with self.cv:
                    self.cv.wait(0.5)
                    yielding_now = self.yielding and self.running == 0
                if time.time() - last_refresh > 30 and self.lock:
                    self.lock.refresh()
                    last_refresh = time.time()
                if yielding_now:
                    self.lock.release()
                    log_line({"time": now_iso(), "event": "lock-yield", "run": self.id})
                    self.wait_lock()
                    self.set_status("running")
                    with self.cv:
                        self.yielding = False
                        self.cv.notify_all()
            self.finish("cancelled" if self.cancelled else "done")
        except Exception as e:  # a failed run reports and logs, the server goes on
            with self.cv:
                self.message = str(e)
            self.finish("error")
        finally:
            if self.lock:
                self.lock.release()
            log_line({"time": now_iso(), "event": "end", "run": self.id, "status": self.status,
                      "lock": "held" if self.held_lock else "none", "procs": self.procs,
                      "max_running": self.max_running, "solved": self.done, "nodes": self.nodes,
                      "waited": round((self.began or self.ended or time.time()) - self.created, 1),
                      "began": datetime.fromtimestamp(self.began).isoformat(timespec="seconds") if self.began else "-",
                      "seconds": round(self.ended - self.began, 1) if self.began and self.ended else 0})

    def finish(self, status):
        with self.cv:
            self.stopping = True
            self.cv.notify_all()
        deadline = time.time() + 15
        while time.time() < deadline:
            with self.cv:
                if self.running == 0:
                    break
            time.sleep(0.05)
        with self.cv:
            if self.root_value is None:
                vals = [e["value"] for e in self.events if e["move"] is not None]
                kids = len(self.jobs) - (1 if self.mode == "solve" else 0)
                if vals and len(vals) == kids and all(v is not None for v in vals):
                    self.root_value = max(vals)
            if self.mode == "all" and self.best is None and self.root_value is not None:
                for e in self.events:
                    if e["value"] == self.root_value:
                        self.best = e["move"]
                        break
            self.ended = time.time()
            self.status = status
            self.cv.notify_all()


# ---------------------------------------------------------------- prove: the replies of a position, one by one
# The twelve symmetries of the board, as in tools/certificate.py: a reflection (q, r) -> (r, q) for t >= 6,
# then t % 6 turns (q, r) -> (-r, q + r).
def _sym(t, q, r):
    if t >= 6:
        q, r = r, q
    for _ in range(t % 6):
        q, r = -r, q + r
    return q, r


SYM = []
for _t in range(12):
    _perm = [0] * GRID
    for _c in CELLS:
        _q, _r = _sym(_t, _c % W - R, _c // W - R)
        _perm[_c] = (_r + R) * W + _q + R
    SYM.append(_perm)


def sym_mask(t, m):
    perm, out = SYM[t], 0
    while m:
        low = m & -m
        out |= 1 << perm[low.bit_length() - 1]
        m ^= low
    return out


# The driver as the measured top-of-proof configuration runs it (LOG, "The top of the proof, fourth report").
PROVE_ARGS = ["--job-threads", "1", "--depth", "2", "--tt", "27", "--upper-first", "--replay", "--win-turns"]
# The position after the centre opening 1:60, and the results file of its paused proof (read, never written).
OPENING = to_text({"cells": [1 << 60, 0], "left": [254, 254], "stm": 1, "ply": 1, "passes": 0})
SEEDS = {OPENING: ROOT / "work" / "long" / "ply1_results.txt"}


def read_results(path):
    """A results file of `snek prove` -> (known, first): known maps a position text to [lo, hi, move that
    proved lo >= 1, or None]; first maps it to the line number where its value was first decided."""
    known, first = {}, {}
    try:
        text = Path(path).read_text(errors="replace")
    except OSError:
        return known, first
    for n, line in enumerate(text.splitlines()):
        f = line.split()
        if len(f) < 6 or f[0].count("_") != 6:
            continue
        try:
            lo, hi = int(f[1]), int(f[2])
        except ValueError:
            continue
        if not -1 <= lo <= hi <= 1:
            continue
        k = known.setdefault(f[0], [-1, 1, None])
        k[0], k[1] = max(k[0], lo), min(k[1], hi)
        if lo >= 1 and k[2] is None:
            k[2] = f[5]
        if (k[0] >= 1 or k[1] <= 0) and f[0] not in first:
            first[f[0]] = n
    return known, first


def plan_roots(solver, text, seed=None):
    """P's children, one for each class under the symmetries that P has, in the order they are proved: those
    the seed results file has decided, in the order it decided them; then the others in the solver's static
    order (the driver prints no move list, so its own order of a node's moves is not known here)."""
    p = parse_text(text)
    kids = solver.children([text])[0]
    stab = [t for t in range(12) if sym_mask(t, p["cells"][0]) == p["cells"][0] and sym_mask(t, p["cells"][1]) == p["cells"][1]]
    known, first = read_results(seed) if seed else ({}, {})
    classes = {}
    for g, (m, c) in enumerate(kids):
        f = c.split("_")
        a, b = int(f[0], 16), int(f[1], 16)
        key = (min((sym_mask(t, a), sym_mask(t, b)) for t in stab), f[2], f[3])
        classes.setdefault(key, []).append((g, m, c))
    reps = []
    for members in classes.values():
        decided = [x for x in members if x[2] in first]
        if decided:
            reps.append((0, first[decided[0][2]], 0, decided[0]))
        else:
            g, m, c = min(members, key=lambda x: (order_key(p, x[1]), x[0]))
            reps.append((1, 0, order_key(p, m), (g, m, c)))
    reps.sort(key=lambda x: (x[0], x[1], x[2], x[3][0]))
    return [(m, c) for _, _, _, (g, m, c) in reps]


def store_key(text):
    """The folder name of P's store: short, because the checkout's own path can be long and Windows stops at
    260 characters (the position text itself is in meta.json)."""
    return hashlib.sha1(text.encode()).hexdigest()[:16]


class ProofStore:
    """The proof that every reply of P loses: work/gui/proofs/<key of P>/ holds roots.txt (one representative a line,
    as the driver reads it), results.txt (the driver's results file, the only state of the proof) and
    meta.json (the moves, the seconds spent on each root, the order they were finished in)."""

    def __init__(self, base, solver, text):
        self.text = text
        self.pos = parse_text(text)
        self.dir = Path(base) / store_key(text)
        self.roots_path, self.results_path, self.meta_path = self.dir / "roots.txt", self.dir / "results.txt", self.dir / "meta.json"
        self.seed = SEEDS.get(text)
        if self.seed is not None and not self.seed.is_file():
            self.seed = None
        if self.meta_path.is_file():
            self.meta = json.loads(self.meta_path.read_text())
            self.roots = [tuple(x) for x in self.meta["roots"]]
            self.exists = True
        else:
            self.exists = False
            self.roots = plan_roots(solver, text, self.seed)
            self.meta = {"position": text, "roots": [list(x) for x in self.roots], "seeded": str(self.seed) if self.seed else None,
                         "created": now_iso(), "seconds": [0.0] * len(self.roots), "finished": [], "runs": 0}
            if self.seed is not None:
                self._seed_seconds()

    def _seed_seconds(self):
        """For roots decided in the seed: the seconds of their recorded tries (as tools/certificate.py counts them)."""
        me = self.pos["stm"]
        idx = {int(c.split("_")[me], 16): i for i, (m, c) in enumerate(self.roots)}
        ply = self.pos["ply"] + 2
        for line in self.seed.read_text(errors="replace").splitlines():
            f = line.split()
            t = f[0].split("_") if f else []
            if len(f) >= 6 and len(t) == 7 and t[5] == str(ply):
                i = idx.get(int(t[me], 16))
                if i is not None and f[4].isdigit():
                    self.meta["seconds"][i] += int(f[4]) / 1000
        known, first = read_results(self.seed)
        done = [i for i, (m, c) in enumerate(self.roots) if c in first]
        self.meta["finished"] = sorted(done, key=lambda i: first[self.roots[i][1]])
        self.meta["seconds"] = [round(s, 1) for s in self.meta["seconds"]]

    def materialize(self):
        if self.exists:
            return
        self.dir.mkdir(parents=True, exist_ok=True)
        if self.seed is not None and not self.results_path.exists():
            self.results_path.write_bytes(self.seed.read_bytes())
        self.results_path.touch()
        self.roots_path.write_text("".join("? %s\n" % c for m, c in self.roots))
        self.save()
        self.exists = True

    def save(self):
        tmp = self.meta_path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.meta))
        os.replace(tmp, self.meta_path)

    def fix_tail(self):
        """A driver stopped mid-line leaves a line without its end; the next run would glue onto it."""
        try:
            with open(self.results_path, "rb+") as f:
                f.seek(0, 2)
                if f.tell() > 0:
                    f.seek(-1, 2)
                    if f.read(1) != b"\n":
                        f.write(b"\n")
        except OSError:
            pass

    def rows(self):
        """One row a root: its state (open, refuted, held), the answer that refutes it, the tries known."""
        path = self.results_path if self.exists else self.seed
        known, first = read_results(path) if path else ({}, {})
        me = self.pos["stm"]
        idx = {int(c.split("_")[me], 16): i for i, (m, c) in enumerate(self.roots)}
        tries = [[0, 0] for _ in self.roots]
        ply = str(self.pos["ply"] + 2)
        for c, (lo, hi, mv) in known.items():
            t = c.split("_")
            if t[5] == ply:
                i = idx.get(int(t[me], 16))
                if i is not None:
                    tries[i][0] += 1
                    tries[i][1] += lo >= 0  # the try did not win: the side of P to move at least draws after it
        finished = list(self.meta["finished"])
        rows = []
        for i, (m, c) in enumerate(self.roots):
            k = known.get(c)
            state, answer = "open", None
            if k and k[0] >= 1:
                state, answer = "refuted", k[2]
            elif k and k[1] <= 0:
                state = "held"
            if state != "open" and i not in finished:
                finished.append(i)
            rows.append({"i": i, "move": m, "child": c, "state": state, "answer": answer, "tries": tries[i][0],
                         "failed": tries[i][1], "jobs": None, "seconds": round(self.meta["seconds"][i], 1)})
        for s, i in enumerate(finished):
            rows[i]["seq"] = s
        return rows


def proof_summary(rows):
    n = len(rows)
    refuted = sum(r["state"] == "refuted" for r in rows)
    held = [r["i"] for r in rows if r["state"] == "held"]
    secs = [r["seconds"] for r in rows if r["state"] == "refuted" and r["seconds"]]
    return {"n": n, "refuted": refuted, "held": held, "open": n - refuted - len(held),
            "avg": round(sum(secs) / len(secs), 1) if secs else None}


def class_counts(solver, texts):
    """For each position text: (the number of its moves, one for each class under the symmetries it has, and
    the set of its children's texts)."""
    out = []
    for text, kids in zip(texts, solver.children(texts)):
        p = parse_text(text)
        stab = [t for t in range(12) if sym_mask(t, p["cells"][0]) == p["cells"][0] and sym_mask(t, p["cells"][1]) == p["cells"][1]]
        keys = set()
        for m, c in kids:
            f = c.split("_")
            a, b = int(f[0], 16), int(f[1], 16)
            keys.add((min((sym_mask(t, a), sym_mask(t, b)) for t in stab), f[2], f[3]))
        out.append((len(keys), {c for m, c in kids}))
    return out


class Tries:
    """The tries of the root in work (the root's mover's moves), read from the results file: a decided try has
    a line at the root's ply + 1, a finished job (a reply to a try) a line at the root's ply + 2. Built once from
    the lines the file holds, then the file is read on from a byte offset."""

    def __init__(self, solver, root_text, path):
        self.solver, self.path = solver, Path(path)
        r = parse_text(root_text)
        self.ply, self.opp = r["ply"], r["stm"]  # opp: the side that tries; me: the side that replies
        self.me = self.opp ^ 1
        self.mine = r["cells"][self.me]
        self.kids = {}  # the trying side's cells after a try -> (move, try position text)
        for m, c in solver.children([root_text])[0]:
            self.kids[int(c.split("_")[self.opp], 16)] = (m, c)
        self.tries = {}  # the trying side's cells -> try
        self.jobs = {}  # job position text -> (try key, lo, hi)
        self.off, self.rest = 0, b""
        self.new = []  # tries without their total yet
        self.changed = True
        self.read()

    def read(self):
        try:
            size = self.path.stat().st_size
        except OSError:
            return
        if size < self.off:  # the file was replaced: start again
            self.off, self.rest, self.tries, self.jobs = 0, b"", {}, {}
        if size == self.off:
            return
        with open(self.path, "rb") as f:
            f.seek(self.off)
            data = f.read(size - self.off)
        self.off += len(data)
        lines = (self.rest + data).split(b"\n")
        self.rest = lines.pop()
        for raw in lines:
            self.line(raw.decode(errors="replace"))
        if self.new:
            news, self.new = self.new, []
            try:
                # The replies of a try, one a class: an upper bound of the jobs the try needs (the driver drops
                # some replies without a job, and does not record a job of fewer than MIN_RECORD nodes).
                for t, (n, kids) in zip(news, class_counts(self.solver, [t["text"] for t in news])):
                    t["total"], t["kids"] = n, kids
            except (RuntimeError, OSError, subprocess.SubprocessError):
                pass
            self.changed = True

    def try_for(self, key):
        t = self.tries.get(key)
        if t is None:
            m, c = self.kids[key]
            cells = [] if m == "pass" else [int(x) for x in m.split(":")[1].split(",")]
            t = self.tries[key] = {"move": m, "cells": cells, "text": c, "state": "work", "done": 0, "total": None, "reply": None,
                                   "kids": None, "lost": set()}
            self.new.append(t)
        return t

    def line(self, line):
        f = line.split()
        if len(f) < 6:
            return
        t = f[0].split("_")
        if len(t) != 7 or not t[5].isdigit():
            return
        ply = int(t[5])
        if ply != self.ply + 1 and ply != self.ply + 2:
            return
        try:
            a, b, lo, hi = int(t[self.opp], 16), int(t[self.me], 16), int(f[1]), int(f[2])
        except ValueError:
            return
        if a not in self.kids:
            return
        if ply == self.ply + 1:
            if b != self.mine:
                return
            tr = self.try_for(a)  # values for the side that replies
            if hi <= -1:
                tr["state"] = "won"
            elif lo >= 0:
                tr["state"], tr["reply"] = "failed", f[5]
            self.changed = True
        elif b & self.mine == self.mine and b != self.mine:
            # a job under a try; it counts once the try's children are known (a position reached in another
            # order, from another root, has the same cells and need not be a reply of this try)
            tr = self.try_for(a)
            self.jobs[f[0]] = a
            if lo >= 1 and f[0] not in tr["lost"]:  # the reply is lost for the side that replies
                tr["lost"].add(f[0])
                self.changed = True

    def summary(self):
        ts = list(self.tries.values())
        for t in ts:
            t["done"] = len(t["lost"] & t["kids"]) if t["kids"] is not None else 0
        work = [t for t in ts if t["state"] == "work"]
        frac = [t["done"] / t["total"] for t in work if t["total"]]
        return {"list": [{k: v for k, v in t.items() if k not in ("text", "kids", "lost")} for t in ts],
                "work": len(work), "failed": sum(t["state"] == "failed" for t in ts), "won": sum(t["state"] == "won" for t in ts),
                "best": round(max(frac), 3) if frac else None,
                "jobs": sum(1 for j, a in self.jobs.items() if self.tries[a]["kids"] is not None and j in self.tries[a]["kids"])}


RE_ROOT = re.compile(r"^(\d+) (\d+) (\S+) (-?\d+) (\d+) (\d+) (\S+) (\d+) (\d+) (\d+) ([0-9.]+) (\S+)")
RE_TRY = re.compile(r"root move (\d+) of (\d+)\s+(\S+)\s+value (-?\d+)")
RE_GAVE = re.compile(r"window \((-?\d+),(-?\d+)\) gave (-?\d+): the value lies in \[(-?\d+), (-?\d+)\]")
RE_JOBS = re.compile(r"window \((-?\d+),(-?\d+)\)\s+jobs (\d+)\s+stopped (\d+)")


class ProveRun:
    """Mode prove: the proof driver proves P's replies (the roots of its store) one after another, as the
    replies of one proof (`prove --replay`), until every one is refuted, one is not, or the run is cancelled.
    The results file is the only state: a later run goes on from it."""

    def __init__(self, rid, app, pos_text, procs, use_lock, owner):
        self.id, self.mode, self.pos_text = rid, "prove", pos_text
        self.app, self.prover, self.solver = app, app.prover, app.solver
        self.pos = parse_text(pos_text)
        self.procs, self.use_lock, self.owner = procs, use_lock, owner
        self.seconds = 0
        self.status, self.message = "starting", ""
        self.cv = threading.Condition()
        self.events = []  # the page reads the rows from the state events
        self.stopping = self.cancelled = self.run_now = False
        self.lock = RunLock() if use_lock else None
        self.held_lock = False
        self.created, self.began, self.ended = time.time(), None, None
        self.store = None
        self.rows = []
        self.drv = 0  # the root the driver is on
        self.cur_t0 = None  # when it began on it
        self.piece_secs = []  # seconds of each root before this piece of the driver
        self.proc = None
        self.worked = 0  # roots finished by search in this run
        self.worked_secs = 0.0
        self.nodes = 0
        self.pieces = 0
        self.last_save = 0.0
        self.tracker, self.tracker_root = None, None
        self.tries_pub, self.tries_pub_t, self.tries_read = None, 0.0, 0.0

    def cur(self):
        if self.proc is not None and self.drv < len(self.rows) and self.rows[self.drv]["state"] in ("open", "work"):
            return self.drv
        return None

    def snapshot(self, with_results=False):
        with self.cv:
            s = proof_summary(self.rows)
            now = time.time()
            c = self.cur()
            rate = round(self.worked_secs / self.worked, 1) if self.worked else s["avg"]
            tries = self.tries_pub if c is not None and self.tries_pub and self.tries_pub["root"] == c else None
            s.update({"rows": [dict(r) for r in self.rows], "cur": c, "rate": rate, "tries": tries,
                      "cur_seconds": round(self.piece_secs[c] + now - self.cur_t0, 1) if c is not None and self.cur_t0 else None})
            return {"id": self.id, "mode": "prove", "position": self.pos_text, "status": self.status, "message": self.message,
                    "total": s["n"], "done": s["refuted"], "running": 1 if self.proc is not None else 0, "nodes": self.nodes,
                    "ahead": getattr(self, "ahead", 0), "held_lock": self.held_lock,
                    "value": -1 if s["refuted"] == s["n"] and s["n"] else None, "best": None, "procs": self.procs, "seconds": 0,
                    "elapsed": round((self.ended or now) - (self.began or now), 1),
                    "waited": round((self.began or self.ended or now) - self.created, 1), "prove": s}

    def cancel(self, why="cancelled"):
        with self.cv:
            if self.status in ("done", "cancelled", "error"):
                return
            self.cancelled = self.stopping = True
            self.message = why
            self.cv.notify_all()
        p = self.proc
        if p is not None:
            try:
                p.kill()
            except OSError:
                pass

    def set_status(self, s):
        with self.cv:
            self.status = s
            self.cv.notify_all()

    wait_lock = Run.wait_lock

    def drive(self):
        """One piece of the driver. Returns "end", "held", "cancel", "yield" or "error"."""
        st = self.store
        exe = [sys.executable, "-u", self.prover] if self.prover.endswith(".py") else [self.prover]  # a .py: a stand-in for tests
        args = exe + ["prove", st.roots_path.name, "--threads", str(self.procs)] + PROVE_ARGS + ["--out", st.results_path.name]
        lines = queue.Queue()
        with self.cv:
            self.piece_secs = [r["seconds"] for r in self.rows]
            self.drv, self.cur_t0 = 0, time.time()
            self.pieces += 1
            for r in self.rows:
                if r["state"] == "work":
                    r["state"] = "open"
            self.proc = start_proc(args, merge=True, cwd=st.dir)
            self.mark_work()
            self.cv.notify_all()
        p = self.proc

        def read():
            for raw in p.stdout:
                lines.put(raw.decode(errors="replace").rstrip("\r\n"))
            lines.put(None)

        reader = threading.Thread(target=read, daemon=True)
        reader.start()
        outcome, tail, last_refresh = None, [], time.time()
        try:
            while outcome is None:
                try:
                    line = lines.get(timeout=0.5)
                except queue.Empty:
                    line = ""
                if line is None:
                    p.wait()
                    outcome = "cancel" if self.stopping else "end" if p.returncode == 0 else "error"
                    if outcome == "error":
                        with self.cv:
                            self.message = "the driver stopped (exit %s): %s" % (p.returncode, " | ".join(tail[-3:]))
                    break
                if line:
                    tail = (tail + [line])[-5:]
                    outcome = self.on_line(line)
                if self.stopping:
                    outcome = "cancel"
                if time.time() - last_refresh > 30 and self.lock:
                    self.lock.refresh()
                    last_refresh = time.time()
                if (outcome is None and self.lock and self.lock.held and time.time() - self.lock.since > YIELD_AFTER
                        and self.lock.others_waiting()):
                    outcome = "yield"
                if outcome is None and time.time() - self.tries_read > 0.5:
                    self.tick_tries()
        finally:
            if p.poll() is None:
                p.kill()
            try:
                p.wait(10)
            except subprocess.TimeoutExpired:
                pass
            reader.join(5)
            with LIVE_LOCK:
                LIVE.discard(p)
            with self.cv:
                c = self.cur()
                if c is not None:  # the root cut off: its seconds so far count
                    self.rows[c]["seconds"] = round(self.piece_secs[c] + time.time() - self.cur_t0, 1)
                    self.rows[c]["state"] = "open"
                self.proc = None
                self.save_meta()
                self.cv.notify_all()
            st.fix_tail()
        return outcome

    def tick_tries(self):
        """The tries of the root in work: built from the results file when the driver takes a root up, then read
        on from where the file was; published to the page at most once a second, when something changed."""
        self.tries_read = time.time()
        c = self.cur()
        if c is None:
            if self.tracker is not None:
                self.tracker = None
            return
        try:
            if self.tracker is None or self.tracker_root != c:
                self.tracker, self.tracker_root = Tries(self.solver, self.rows[c]["child"], self.store.results_path), c
            else:
                self.tracker.read()
        except (RuntimeError, OSError, subprocess.SubprocessError, ValueError) as e:
            with self.cv:
                self.message = "tries: %s" % e
            return
        t = self.tracker
        if t.changed and time.time() - self.tries_pub_t >= 1:
            s = dict(t.summary(), root=c)
            with self.cv:
                self.tries_pub = s
                self.tries_pub_t = time.time()
                self.cv.notify_all()
            t.changed = False

    def mark_work(self):
        c = self.cur()
        if c is not None:
            r = self.rows[c]
            r["state"], r["tries"], r["failed"], r["jobs"] = "work", 0, 0, 0

    def save_meta(self):
        self.last_save = time.time()
        m = self.store.meta
        m["seconds"] = [r["seconds"] for r in self.rows]
        fin = list(m["finished"])
        for r in sorted((r for r in self.rows if r["state"] in ("refuted", "held") and "seq" in r), key=lambda r: r["seq"]):
            if r["i"] not in fin:
                fin.append(r["i"])
        m["finished"] = fin
        self.store.save()

    def on_line(self, line):
        with self.cv:
            try:
                return self._on_line(line)
            finally:
                self.cv.notify_all()

    def _on_line(self, line):
        m = RE_ROOT.match(line)
        if m:
            i, v, nodes, us, jobs, stopped = int(m.group(1)), int(m.group(4)), int(m.group(5)), int(m.group(6)), int(m.group(8)), int(m.group(9))
            if i >= len(self.rows):
                return None
            r = self.rows[i]
            self.nodes += nodes
            searched = jobs + stopped > 0
            if searched:
                r["seconds"] = round(self.piece_secs[i] + us / 1e6, 1)
                self.worked += 1
                self.worked_secs += us / 1e6
                r["jobs"] = jobs
            was = r["state"]
            if v == 1:
                r["state"], r["answer"] = "refuted", m.group(12)
            else:
                r["state"] = "held"
            if was in ("open", "work"):
                r["seq"] = max([x.get("seq", -1) for x in self.rows] + [-1]) + 1
            self.drv, self.cur_t0 = i + 1, time.time()
            self.mark_work()
            if searched and time.time() - self.last_save > 2:  # the piece's end saves the rest
                self.save_meta()
            return "held" if v != 1 else None
        c = self.cur()
        m = RE_TRY.search(line)
        if m:
            if c is not None:
                r = self.rows[c]
                r["tries"] = int(m.group(1))
                r["failed"] += int(m.group(4)) < 1
                r["last"] = m.group(3)
            return None
        m = RE_GAVE.search(line)
        if m:
            if int(m.group(5)) <= 0 and self.drv < len(self.rows):  # not a win for the root's mover: stop here
                r = self.rows[self.drv]
                r["state"] = "held"
                if c is not None:
                    r["seconds"] = round(self.piece_secs[c] + time.time() - self.cur_t0, 1)
                r["seq"] = max([x.get("seq", -1) for x in self.rows] + [-1]) + 1
                self.save_meta()
                return "held"
            return None
        m = RE_JOBS.search(line)
        if m and c is not None:
            self.rows[c]["jobs"] = int(m.group(3))
        return None

    def held_message(self):
        h = [r for r in self.rows if r["state"] == "held"]
        return "not refuted: %s" % h[0]["move"] if h else ""

    def main(self):
        outcome = None
        try:
            self.store = ProofStore(self.app.proofs, self.solver, self.pos_text)
            self.store.materialize()
            with self.cv:
                self.rows = self.store.rows()
            s = proof_summary(self.rows)
            if s["held"]:
                with self.cv:
                    self.message = self.held_message()
                self.finish("done")
                return
            if s["open"] == 0:
                self.finish("done")
                return
            if not self.wait_lock():
                self.finish("cancelled")
                return
            self.began = time.time()
            self.set_status("running")
            log_line({"time": now_iso(), "event": "start", "run": self.id, "mode": "prove",
                      "lock": "held" if self.held_lock else "none", "procs": self.procs, "limit": 0,
                      "positions": s["n"], "refuted": s["refuted"], "position": self.pos_text})
            while True:
                outcome = self.drive()
                if outcome == "yield":
                    self.lock.release()
                    log_line({"time": now_iso(), "event": "lock-yield", "run": self.id})
                    if not self.wait_lock():
                        outcome = "cancel"
                        break
                    self.set_status("running")
                    continue
                break
            if outcome == "held":
                with self.cv:
                    self.message = self.held_message()
            self.finish("cancelled" if self.cancelled else "error" if outcome == "error" else "done")
        except Exception as e:  # a failed run reports and logs, the server goes on
            with self.cv:
                self.message = str(e)
            self.finish("error")
        finally:
            if self.proc is not None:
                try:
                    self.proc.kill()
                except OSError:
                    pass
            if self.lock:
                self.lock.release()
            s = proof_summary(self.rows)
            log_line({"time": now_iso(), "event": "end", "run": self.id, "mode": "prove", "status": self.status,
                      "lock": "held" if self.held_lock else "none", "procs": self.procs, "pieces": self.pieces,
                      "refuted": s["refuted"], "held": len(s["held"]), "open": s["open"], "searched": self.worked,
                      "nodes": self.nodes, "waited": round((self.began or self.ended or time.time()) - self.created, 1),
                      "began": datetime.fromtimestamp(self.began).isoformat(timespec="seconds") if self.began else "-",
                      "seconds": round(self.ended - self.began, 1) if self.began and self.ended else 0})

    def finish(self, status):
        with self.cv:
            self.stopping = True
            self.ended = time.time()
            self.status = status
            self.cv.notify_all()


class App:
    def __init__(self, solver, prover=None, proofs=None):
        self.solver = solver
        self.prover = str(prover) if prover else ""
        self.proofs = Path(proofs) if proofs else ROOT / "work" / "gui" / "proofs"
        self.stores = {}  # position text -> a ProofStore planned but not written yet
        self.run = None
        self.ids = 0
        self.mu = threading.Lock()
        self.clients = {}  # client id -> time its event stream was last alive

    def prover_found(self):
        return bool(self.prover) and Path(self.prover).is_file()

    def proof_state(self, text):
        """The state of P's proof store without a run: None when there is none (and no seed for it)."""
        r = self.run
        if r is not None and r.mode == "prove" and r.pos_text == text and r.rows:
            s = r.snapshot()["prove"]
            return dict(s, exists=True, live=self.active())
        if (self.proofs / store_key(text) / "meta.json").is_file():
            st = ProofStore(self.proofs, self.solver, text)
        elif text in SEEDS:  # planned from its seed, written only when a run starts
            st = self.stores.get(text) or ProofStore(self.proofs, self.solver, text)
            self.stores[text] = st
        else:
            return None
        if not st.exists and st.seed is None:
            return None
        rows = st.rows()
        return dict(proof_summary(rows), rows=rows, cur=None, rate=None, cur_seconds=None, exists=st.exists, live=False)

    def active(self):
        return self.run is not None and self.run.status not in ("done", "cancelled", "error")

    def start(self, mode, text, seconds, procs, use_lock, owner):
        with self.mu:
            r = self.run
            if r is not None and self.active() and r.cancelled:
                t0 = time.time()  # a cancelled run that is still stopping: wait for it
                while self.active() and time.time() - t0 < 15:
                    time.sleep(0.05)
            if self.active():
                raise Conflict("a run is in progress (in this tab or another)")
            self.ids += 1
            if mode == "prove":
                run = ProveRun(self.ids, self, text, procs, use_lock, owner)
            else:
                run = Run(self.ids, self.solver, mode, text, seconds, procs, use_lock, owner)
            self.run = run
        threading.Thread(target=run.main, daemon=True).start()
        return run

    def watchdog(self):
        while True:
            time.sleep(0.5)
            r = self.run
            if r and r.owner and self.active():
                seen = self.clients.get(r.owner)
                if seen is None:
                    seen = r.created
                if time.time() - seen > OWNER_GONE and time.time() - r.created > OWNER_GONE:
                    r.cancel("the page went away")


class Conflict(Exception):
    pass


def load_input(solver, s):
    """A position text, a move list from the start, or a position text followed by moves."""
    toks = s.split()
    if not toks:
        raise BadPosition("nothing to load")
    if "_" in toks[0]:
        pos = to_text(parse_text(toks.pop(0)))
    else:
        pos = to_text({"cells": [0, 0], "left": [255, 254], "stm": 0, "ply": 0, "passes": 0})
    for k, tok in enumerate(toks):
        p = parse_text(pos)
        if is_over(p):
            raise BadPosition("move %d (%s): the game is over" % (k + 1, tok))
        kids = solver.children([pos])[0]
        want = tok
        if ":" in tok:
            name, _, cells = tok.partition(":")
            try:
                want = "%s:%s" % (name.upper(), ",".join(str(c) for c in sorted(int(x) for x in cells.split(","))))
            except ValueError:
                raise BadPosition("move %d (%s) is not a move" % (k + 1, tok))
        match = [c for m, c in kids if m == want]
        if not match:
            raise BadPosition("move %d (%s) is not legal here" % (k + 1, tok))
        pos = match[0]
    if toks and "_" not in s.split()[0]:
        code, out, err = solver.quick(["text"] + toks)
        if code != 0 or out.strip() != pos:
            raise BadPosition("the solver reads this move list differently: " + (out.strip() or err.strip()[-200:]))
    return pos


# ---------------------------------------------------------------- HTTP
STATIC = {"/": ("index.html", "text/html"), "/index.html": ("index.html", "text/html"),
          "/app.js": ("app.js", "text/javascript"), "/rules.js": ("rules.js", "text/javascript"),
          "/style.css": ("style.css", "text/css")}


def make_handler(app):
    class H(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):
            pass

        def send_json(self, obj, code=200):
            b = json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(b)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(b)

        def body(self):
            n = int(self.headers.get("Content-Length") or 0)
            if n > 1_000_000:
                raise BadPosition("request too large")
            try:
                return json.loads(self.rfile.read(n) or b"{}")
            except json.JSONDecodeError:
                raise BadPosition("not JSON")

        def do_GET(self):
            u = urllib.parse.urlparse(self.path)
            if u.path in STATIC:
                name, ctype = STATIC[u.path]
                try:
                    b = (GUI_DIR / name).read_bytes()
                except OSError:
                    self.send_error(404)
                    return
                self.send_response(200)
                self.send_header("Content-Type", ctype + "; charset=utf-8")
                self.send_header("Content-Length", str(len(b)))
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(b)
            elif u.path == "/api/info":
                self.send_json({"solver": app.solver.path, "solver_found": app.solver.exists(),
                                "prover": app.prover, "prover_found": app.prover_found(),
                                "cpus": os.cpu_count() or 1, "typical": TYPICAL,
                                "lock": RunLock().available()})
            elif u.path == "/api/status":
                r = app.run
                self.send_json({"run": r.snapshot(True) if r else None,
                                "solver_pids": sorted(p.pid for p in list(LIVE))})
            elif u.path == "/api/proof":
                text = urllib.parse.parse_qs(u.query).get("position", [""])[0]
                try:
                    text = to_text(parse_text(text))
                    self.send_json({"ok": True, "position": text, "proof": app.proof_state(text)})
                except BadPosition as e:
                    self.send_json({"ok": False, "error": str(e)}, 400)
                except (RuntimeError, OSError, ValueError) as e:
                    self.send_json({"ok": False, "error": str(e)}, 500)
            elif u.path == "/api/events":
                self.events(urllib.parse.parse_qs(u.query).get("client", [""])[0])
            else:
                self.send_error(404)

        def events(self, client):
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            sent_run, sent_n, last_state, last_beat, last_sent = None, 0, None, 0.0, 0.0
            try:
                while True:
                    if client:
                        app.clients[client] = time.time()
                    r = app.run
                    chunks = []
                    if r is not None and r.owner == client:
                        if r is not sent_run:
                            sent_run, sent_n, last_state = r, 0, None
                        with r.cv:
                            new = r.events[sent_n:]
                            sent_n += len(new)
                        for e in new:
                            chunks.append("event: result\ndata: %s\n\n" % json.dumps(dict(e, run=r.id, position=r.pos_text)))
                        st = r.snapshot()
                        kv = {k: v for k, v in st.items() if k not in ("elapsed", "waited")}
                        if "prove" in kv:
                            kv["prove"] = {k: v for k, v in kv["prove"].items() if k != "cur_seconds"}
                        key = json.dumps(kv)
                        gap, beat = (1.0, 5.0) if r.mode == "prove" else (0.0, 0.5)
                        if (key != last_state and time.time() - last_sent >= gap) or time.time() - last_sent > beat:
                            chunks.append("event: state\ndata: %s\n\n" % json.dumps(st))
                            last_state = key
                            last_beat = last_sent = time.time()
                    if not chunks and time.time() - last_beat > 1:
                        chunks.append(": ping\n\n")
                        last_beat = time.time()
                    if chunks:
                        self.wfile.write("".join(chunks).encode())
                        self.wfile.flush()
                    if r is not None and r.owner == client:
                        with r.cv:
                            r.cv.wait(0.25)
                    else:
                        time.sleep(0.25)
            except (OSError, ValueError):
                return

        def do_POST(self):
            u = urllib.parse.urlparse(self.path)
            try:
                b = self.body()
                if u.path == "/api/load":
                    self.send_json({"ok": True, "position": load_input(app.solver, str(b.get("text", "")))})
                elif u.path == "/api/moves":
                    text = to_text(parse_text(str(b.get("position", ""))))
                    kids = app.solver.children([text])[0]
                    self.send_json({"ok": True, "moves": [{"move": m, "child": c} for m, c in kids]})
                elif u.path == "/api/run":
                    if not app.solver.exists():
                        raise BadPosition("no solver at %s (start with --solver PATH)" % app.solver.path)
                    mode = b.get("mode")
                    if mode not in ("solve", "all", "prove"):
                        raise BadPosition("mode is solve, all or prove")
                    if mode == "prove" and not app.prover_found():
                        raise BadPosition("no proof driver at %s (start with --prover PATH)" % app.prover)
                    text = to_text(parse_text(str(b.get("position", ""))))
                    if is_over(parse_text(text)):
                        raise BadPosition("the game is over")
                    seconds = float(b.get("seconds", 10))
                    procs = int(b.get("procs", 1))
                    if not (0 <= seconds <= 7 * 86400) or not (1 <= procs <= 64):
                        raise BadPosition("the limit is 0 (no limit) to a week of seconds and 1 to 64 processes")
                    r = app.start(mode, text, seconds, procs, bool(b.get("lock", True)), str(b.get("client") or ""))
                    self.send_json({"ok": True, "run": r.id})
                elif u.path == "/api/cancel":
                    if app.run:
                        app.run.cancel()
                    self.send_json({"ok": True})
                elif u.path == "/api/runnow":
                    if app.run:
                        app.run.run_now = True
                    self.send_json({"ok": True})
                else:
                    self.send_error(404)
            except BadPosition as e:
                self.send_json({"ok": False, "error": str(e)}, 400)
            except Conflict as e:
                self.send_json({"ok": False, "error": str(e)}, 409)
            except (RuntimeError, ValueError, TypeError, subprocess.SubprocessError) as e:
                self.send_json({"ok": False, "error": str(e)}, 500)

    return H


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--solver", default=str(Path("frozen") / "climb-base.exe"),
                    help="the solver executable, relative to the repository root or absolute")
    ap.add_argument("--prover", default=str(Path("frozen") / "prove-replay.exe"),
                    help="the proof driver for Prove (a build of branch climb/replay), relative to the root or absolute")
    ap.add_argument("--proofs", default=str(Path("work") / "gui" / "proofs"),
                    help="where the proof stores live, relative to the root or absolute")
    a = ap.parse_args()
    sp = Path(a.solver)
    solver = Solver(sp if sp.is_absolute() else ROOT / sp)
    if not solver.exists():
        print("warning: no solver at", solver.path, file=sys.stderr)
    pp, pd = Path(a.prover), Path(a.proofs)
    app = App(solver, pp if pp.is_absolute() else ROOT / pp, pd if pd.is_absolute() else ROOT / pd)
    if not app.prover_found():
        print("warning: no proof driver at", app.prover, file=sys.stderr)
    threading.Thread(target=app.watchdog, daemon=True).start()
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), make_handler(app))
    srv.daemon_threads = True
    print("http://localhost:%d  (solver %s)" % (a.port, solver.path), flush=True)
    try:
        srv.serve_forever(poll_interval=0.25)
    except KeyboardInterrupt:
        pass
    finally:
        if app.run:
            app.run.cancel("the server stopped")
        kill_all()
        t0 = time.time()
        while app.active() and time.time() - t0 < 5:
            time.sleep(0.05)
        if app.run and app.run.lock:
            app.run.lock.release()
        srv.server_close()


if __name__ == "__main__":
    main()
