"""The analysis board: a local web page that sets up Snekagon positions and runs the solver on them.

    python tools/gui.py [--port 8765] [--solver frozen/climb-base.exe]

Open http://localhost:8765. The page (gui/) knows the rules itself (gui/rules.js, checked against the
solver by tools/gui_check.py); this server runs the solver. Every solve is one `solve <position>` process
per position, several at once, each killed when it reaches the time limit or the run is cancelled.

A run waits for the machine's run lock (the protocol of tools/lockfair.sh, done here in Python: a ticket
in .git/run.queue, the lock directory .git/run.lock), one lock for the whole run, unless the page says
"run now". A run that holds the lock for more than 15 minutes while others wait stops starting new
solves, lets the running ones finish, gives the lock back and queues again. Every run is logged to
work/gui/runs.log. Solver processes live in a Windows job object, so they die with this server.
"""
import argparse
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
YIELD_AFTER = 15 * 60  # seconds under the lock before giving it back to waiters
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


def start_proc(args):
    p = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, stdin=subprocess.DEVNULL,
                         creationflags=NO_WINDOW, cwd=str(ROOT))
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
        self.ticket = self.queue / ("%d-%d" % (time.time_ns(), os.getpid()))
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
                self.ahead = ahead + (1 if busy else 0)
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


class App:
    def __init__(self, solver):
        self.solver = solver
        self.run = None
        self.ids = 0
        self.mu = threading.Lock()
        self.clients = {}  # client id -> time its event stream was last alive

    def active(self):
        return self.run is not None and self.run.status not in ("done", "cancelled", "error")

    def start(self, mode, text, seconds, procs, use_lock, owner):
        with self.mu:
            if self.active():
                raise Conflict("a run is in progress (in this tab or another)")
            self.ids += 1
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
                                "cpus": os.cpu_count() or 1, "typical": TYPICAL,
                                "lock": RunLock().available()})
            elif u.path == "/api/status":
                r = app.run
                self.send_json({"run": r.snapshot(True) if r else None,
                                "solver_pids": sorted(p.pid for p in list(LIVE))})
            elif u.path == "/api/events":
                self.events(urllib.parse.parse_qs(u.query).get("client", [""])[0])
            else:
                self.send_error(404)

        def events(self, client):
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            sent_run, sent_n, last_state, last_beat = None, 0, None, 0.0
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
                        key = json.dumps({k: v for k, v in st.items() if k not in ("elapsed", "waited")})
                        if key != last_state or time.time() - last_beat > 0.5:
                            chunks.append("event: state\ndata: %s\n\n" % json.dumps(st))
                            last_state = key
                            last_beat = time.time()
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
                    if mode not in ("solve", "all"):
                        raise BadPosition("mode is solve or all")
                    text = to_text(parse_text(str(b.get("position", ""))))
                    if is_over(parse_text(text)):
                        raise BadPosition("the game is over")
                    seconds = float(b.get("seconds", 10))
                    procs = int(b.get("procs", 1))
                    if not (0 < seconds <= 7 * 86400) or not (1 <= procs <= 64):
                        raise BadPosition("the limit is 0 to a week of seconds and 1 to 64 processes")
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
    a = ap.parse_args()
    sp = Path(a.solver)
    solver = Solver(sp if sp.is_absolute() else ROOT / sp)
    if not solver.exists():
        print("warning: no solver at", solver.path, file=sys.stderr)
    app = App(solver)
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
