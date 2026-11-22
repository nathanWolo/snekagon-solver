"""A page to watch a run of `tools/crosscheck.py` (another proof's answers, checked by our driver).

    python tools/crosscheck_watch.py [--port 8772] [DIR]        # DIR: work/k0check by default

Listens on 127.0.0.1 only and answers two addresses: `/` (the page) and `/api/status` (what the
page shows, as JSON). It reads DIR/lines.txt, map.tsv, run.log, run.err and run.times, and starts,
stops and writes nothing. To look at it from outside this PC, put `tools/gui_gate.py --board
127.0.0.1:8772` in front of it and a tunnel in front of the gate, as for the analysis board.
"""
import argparse
import json
import re
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROGRESS = re.compile(r"^#\s+([\d.]+) s\s+root move (\d+) of (\d+)")
CACHE = {"at": 0.0, "body": b""}


def read_text(path):
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def tail(path, size=6000):
    try:
        with open(path, "rb") as f:
            f.seek(0, 2)
            end = f.tell()
            f.seek(max(0, end - size))
            return f.read().decode("utf-8", errors="replace")
    except OSError:
        return ""


def mtime(path):
    try:
        return path.stat().st_mtime
    except OSError:
        return None


def stamp(text):
    try:
        return time.mktime(time.strptime(text, "%Y-%m-%d %H:%M:%S"))
    except ValueError:
        return None


def status(d):
    lines = [l.split() for l in read_text(d / "lines.txt").splitlines() if l.strip()]
    same = {}
    for l in read_text(d / "map.tsv").splitlines()[1:]:
        f = l.split("\t")
        if len(f) >= 6:
            same[(f[0], f[1])] = f[5] == "same"
    done = {}
    for l in read_text(d / "run.log").splitlines():
        f = l.split()
        if len(f) >= 11 and f[0].isdigit() and f[6] in ("ok", "WRONG"):
            done[int(f[0])] = f
    rows = []
    for i, l in enumerate(lines):
        y, z = l[2], l[3]
        f = done.get(i)
        if f:
            good = f[6] == "ok" and f[3] == "-1"
            rows.append([y, z, "ok" if good else "wrong", round(int(f[5]) / 1e6), int(f[3]), same.get((y, z), False)])
        else:
            rows.append([y, z, "", 0, None, same.get((y, z), False)])
    started = ended = code = None
    for l in read_text(d / "run.times").splitlines():
        m = re.match(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d) (start|end)(?:, exit (\d+))?", l)
        if m and m.group(2) == "start":
            started = stamp(m.group(1))
        elif m:
            ended, code = stamp(m.group(1)), int(m.group(3) or 0)
    now = time.time()
    spent = sum(r[3] for r in rows if r[2])
    n_done = len(done)
    work = None
    left = None
    idle = 0
    if ended is None and n_done < len(lines):
        k = n = secs = 0
        for l in tail(d / "run.err").splitlines():
            m = PROGRESS.match(l)
            if m:
                secs, k, n = float(m.group(1)), int(m.group(2)), int(m.group(3))
        # the driver writes a line to run.log when a root ends, so the file's time is the start
        # of the root in work
        began = mtime(d / "run.log") if n_done else started
        in_root = max(0, round(now - began)) if began else 0
        # the tail can still belong to the root that has just ended: then nothing is known yet
        fresh = in_root + 5 >= secs
        work = {"index": n_done, "k": k if fresh else 0, "n": n if fresh else 0, "seconds": in_root}
        if n_done:
            mean = spent / n_done
            left = round(max(0, mean - in_root) + mean * (len(lines) - n_done - 1))
        last = mtime(d / "run.err")
        idle = max(0, round(now - last)) if last else 0
    return {
        "total": len(lines), "done": n_done,
        "ok": sum(r[2] == "ok" for r in rows), "wrong": sum(r[2] == "wrong" for r in rows),
        "elapsed": round((ended or now) - started) if started else None,
        "left": left, "ended": ended is not None, "exit": code, "idle": idle, "work": work, "rows": rows,
    }


PAGE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Cross-check</title>
<style>
:root { color-scheme: light dark; --bg: #f5f6f8; --surface: #fff; --ink: #18202b; --muted: #586372; --rule: #d7dce4;
  --red: #c4382d; --blue: #2a5fc0; --ok: #0c6b67; --cell: #e8ecf1; --cell-line: #bfc7d2; --now: #cfe8e5; }
@media (prefers-color-scheme: dark) { :root { --bg: #11151b; --surface: #181e26; --ink: #e3e7ed; --muted: #97a2b0; --rule: #2a323d;
  --red: #ec6b5f; --blue: #6d9ef2; --ok: #57c6bd; --cell: #202832; --cell-line: #3a4553; --now: #173f3c; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink); font: 16px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; }
.wrap { max-width: 880px; margin: 0 auto; padding: 18px max(16px, env(safe-area-inset-left)) 40px; display: grid; gap: 16px; }
.top { display: flex; align-items: baseline; gap: 14px; flex-wrap: wrap; }
.count { font: 600 2.3rem/1 ui-monospace, "SF Mono", Consolas, monospace; font-variant-numeric: tabular-nums; }
.count small { font-size: 1.1rem; font-weight: 500; color: var(--muted); }
.times { color: var(--muted); font: 0.9rem ui-monospace, Consolas, monospace; font-variant-numeric: tabular-nums; }
.bad { color: var(--red); font-weight: 600; }
.bar { height: 10px; background: var(--cell); border-radius: 5px; overflow: hidden; }
.bar i { display: block; height: 100%; width: 0; background: var(--ok); }
.bar.thin { height: 4px; } .bar.thin i { background: var(--blue); }
.main { display: grid; gap: 16px; grid-template-columns: minmax(0, 1fr); }
@media (min-width: 760px) { .main { grid-template-columns: minmax(0, 380px) minmax(0, 1fr); align-items: start; } }
.card { background: var(--surface); border: 1px solid var(--rule); border-radius: 10px; padding: 12px; }
svg { display: block; width: 100%; height: auto; }
.hx { fill: var(--cell); stroke: var(--cell-line); stroke-width: .06; }
.hx.r { fill: var(--red); stroke: var(--red); } .hx.b { fill: var(--blue); stroke: var(--blue); }
.line { margin: 10px 0 6px; font: 0.86rem ui-monospace, Consolas, monospace; display: flex; justify-content: space-between; gap: 8px; color: var(--muted); }
.line b { color: var(--ink); font-weight: 500; }
.list { max-height: 70vh; overflow-y: auto; padding: 4px; position: relative; display: grid; align-content: start;
  grid-template-columns: 1.3rem auto auto minmax(2.6rem, 1fr); column-gap: 10px; }
.row { display: grid; grid-template-columns: subgrid; grid-column: 1 / -1; padding: 5px 6px; border-radius: 6px; cursor: pointer;
  font: 0.82rem ui-monospace, Consolas, monospace; font-variant-numeric: tabular-nums; color: var(--muted); white-space: nowrap; }
@media (max-width: 480px) { .list { column-gap: 6px; } .row { font-size: 0.68rem; } }
.row.ok, .row.wrong, .row.now { color: var(--ink); }
.row.now { background: var(--now); } .row.sel { outline: 2px solid var(--blue); outline-offset: -2px; }
.row .m { text-align: center; } .row.ok .m { color: var(--ok); } .row.wrong .m, .row.wrong { color: var(--red); }
.row .s { text-align: right; color: var(--muted); }
.yb { color: var(--blue); } .zr { color: var(--red); }
</style></head><body>
<div class="wrap">
  <div class="top"><div class="count"><span id="done">0</span><small> / <span id="total">172</span></small></div>
    <div class="times" id="times"></div><div class="bad" id="bad"></div></div>
  <div class="bar"><i id="bar"></i></div>
  <div class="main">
    <div class="card"><svg id="board" viewBox="-9.75 -8.7 19.5 17.4" role="img" aria-label="The board"></svg>
      <div class="line"><span><b class="yb" id="y"></b> <b class="zr" id="z"></b></span><span id="kn"></span></div>
      <div class="bar thin"><i id="bar2"></i></div></div>
    <div class="card list" id="list"></div>
  </div>
</div>
<script>
(function () {
  "use strict";
  var NS = "http://www.w3.org/2000/svg", S3 = Math.sqrt(3), cells = {}, S = null, pinned = null, built = false;
  var board = document.getElementById("board");
  for (var c = 0; c < 121; c++) {
    var q = c % 11 - 5, r = Math.floor(c / 11) - 5;
    if (Math.abs(q) > 5 || Math.abs(r) > 5 || Math.abs(q + r) > 5) continue;
    var x = S3 * (q + r / 2), y = 1.5 * r, p = [];
    for (var k = 0; k < 6; k++) { var a = Math.PI / 180 * (60 * k - 30); p.push((x + 0.93 * Math.cos(a)).toFixed(3) + "," + (y + 0.93 * Math.sin(a)).toFixed(3)); }
    var e = document.createElementNS(NS, "polygon");
    e.setAttribute("points", p.join(" ")); e.setAttribute("class", "hx");
    board.appendChild(e); cells[c] = e;
  }
  function of(move) { return move.slice(move.indexOf(":") + 1).split(",").map(Number); }
  function clock(s) { if (s == null) return ""; var h = Math.floor(s / 3600), m = Math.floor(s % 3600 / 60); return h + ":" + (m < 10 ? "0" : "") + m; }
  function id(x) { return document.getElementById(x); }
  function show(i) {
    var row = S.rows[i]; if (!row) return;
    for (var c in cells) cells[c].setAttribute("class", "hx");
    cells[60].setAttribute("class", "hx r");
    of(row[0]).forEach(function (c) { cells[c].setAttribute("class", "hx b"); });
    of(row[1]).forEach(function (c) { cells[c].setAttribute("class", "hx r"); });
    id("y").textContent = row[0]; id("z").textContent = row[1];
    var w = S.work, live = w && w.index === i;
    id("kn").textContent = live && w.n ? w.k + " / " + w.n : row[2] ? row[3] + " s" : "";
    id("bar2").style.width = (live && w.n ? 100 * w.k / w.n : row[2] ? 100 : 0) + "%";
    Array.prototype.forEach.call(document.querySelectorAll(".row.sel"), function (e) { e.classList.remove("sel"); });
    if (pinned != null) id("r" + i).classList.add("sel");
  }
  function build() {
    var list = id("list");
    S.rows.forEach(function (row, i) {
      var d = document.createElement("div"); d.id = "r" + i; d.className = "row";
      ["m", "yb", "zr", "s"].forEach(function (cl) { var s = document.createElement("span"); s.className = cl; d.appendChild(s); });
      d.children[1].textContent = row[0]; d.children[2].textContent = row[1];
      d.addEventListener("click", function () { pinned = pinned === i ? null : i; render(); });
      list.appendChild(d);
    });
    built = true;
  }
  function render() {
    if (!built) build();
    id("done").textContent = S.done; id("total").textContent = S.total;
    id("bar").style.width = (S.total ? 100 * S.done / S.total : 0) + "%";
    id("times").textContent = S.elapsed == null ? "" : clock(S.elapsed) + (S.ended ? "" : S.left != null ? "  ·  ~" + clock(S.left) : "");
    id("bad").textContent = S.wrong ? S.wrong + " ✗" : S.ended ? (S.done === S.total ? "✓" : "■") : S.idle > 600 ? "⏸" : "";
    var now = S.work ? S.work.index : -1;
    S.rows.forEach(function (row, i) {
      var d = id("r" + i);
      d.className = "row " + (i === now ? "now" : row[2]);
      d.children[0].textContent = row[2] === "ok" ? "✓" : row[2] === "wrong" ? "✗" : i === now ? "…" : "";
      d.children[3].textContent = row[2] ? row[3] + " s" : "";
    });
    var at = pinned != null ? pinned : now >= 0 ? now : Math.max(0, S.done - 1);
    show(at);
    // keep the line in work in view, by moving the list only and never the page
    if (pinned == null && now >= 0 && render.at !== now) {
      var list = id("list"), d = id("r" + now);
      list.scrollTop = d.offsetTop - (list.clientHeight - d.offsetHeight) / 2;
      render.at = now;
    }
  }
  function poll() {
    fetch("api/status", { cache: "no-store" }).then(function (r) { return r.json(); }).then(function (s) { S = s; render(); })
      .catch(function () {}).then(function () { setTimeout(poll, S && S.ended ? 30000 : 4000); });
  }
  poll();
})();
</script></body></html>
"""


def make_handler(d):
    class Watch(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):
            pass

        def send(self, code, kind, body):
            self.send_response(code)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(body)

        def do_GET(self):
            path = self.path.split("?", 1)[0]
            if path == "/":
                return self.send(200, "text/html; charset=utf-8", PAGE.encode("utf-8"))
            if path == "/api/status":
                if time.time() - CACHE["at"] > 1.0:
                    CACHE["body"] = json.dumps(status(d), separators=(",", ":")).encode("utf-8")
                    CACHE["at"] = time.time()
                return self.send(200, "application/json", CACHE["body"])
            return self.send(404, "text/plain", b"not here")

        do_HEAD = do_GET

    return Watch


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir", nargs="?", default=str(ROOT / "work" / "k0check"))
    ap.add_argument("--port", type=int, default=8772)
    a = ap.parse_args()
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), make_handler(Path(a.dir)))
    print("watching %s on http://127.0.0.1:%d" % (a.dir, a.port), flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
