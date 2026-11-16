"""A gate in front of the analysis board, for looking at it from outside this PC (through a tunnel).

    python tools/gui_gate.py [--port 8781] [--board 127.0.0.1:8770] [--token-file work/gui/gate.token] [--control]

The board itself (tools/gui.py) has no login and starts solver processes for whoever asks, so it must not
be put on the internet as it is. This gate listens on 127.0.0.1 and passes requests on to the board when
they carry the token: the first time as ?k=TOKEN in the address (answered with a cookie and a redirect to
the same address without it), after that as the cookie. Anything else gets 403.

Unless --control is given, the requests that start or stop work on this PC (/api/run, /api/cancel,
/api/runnow) are refused: a visitor can watch a run and move around positions, not use the machine or stop
a proof. The event stream is passed through as it comes.

The token is made on the first start and kept in the token file (work/ is not in git). The gate prints
nothing that contains it.
"""
import argparse
import hmac
import http.client
import json
import secrets
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COOKIE = "sgate"
CONTROL = {"/api/run", "/api/cancel", "/api/runnow"}
PASS_REQUEST = ("content-type", "accept", "cache-control", "last-event-id")
SKIP_RESPONSE = {"connection", "keep-alive", "transfer-encoding", "content-length", "server", "date"}


def make_handler(board, token, control):
    host, _, port = board.partition(":")

    class Gate(BaseHTTPRequestHandler):
        def log_message(self, fmt, *args):  # the address can hold the token: log nothing
            pass

        def cookie_token(self):
            for part in (self.headers.get("Cookie") or "").split(";"):
                name, _, value = part.strip().partition("=")
                if name == COOKIE:
                    return value
            return ""

        def refuse(self, code, text):
            body = json.dumps({"ok": False, "error": text}).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def handle_any(self):
            u = urllib.parse.urlsplit(self.path)
            query = urllib.parse.parse_qsl(u.query, keep_blank_values=True)
            given = [v for k, v in query if k == "k"]
            rest = urllib.parse.urlencode([(k, v) for k, v in query if k != "k"])
            path = u.path + ("?" + rest if rest else "")
            if given:
                if not hmac.compare_digest(given[0].encode(), token.encode()):
                    return self.refuse(403, "no access")
                secure = "; Secure" if (self.headers.get("X-Forwarded-Proto") or "").lower() == "https" else ""
                self.send_response(302)
                self.send_header("Location", path)
                self.send_header("Set-Cookie", "%s=%s; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800%s" % (COOKIE, token, secure))
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            if not hmac.compare_digest(self.cookie_token().encode(), token.encode()):
                return self.refuse(403, "no access")
            if not control and u.path in CONTROL:
                return self.refuse(403, "watch only: runs are started and stopped at the PC")
            n = int(self.headers.get("Content-Length") or 0)
            if n > 1 << 20:
                return self.refuse(413, "too large")
            body = self.rfile.read(n) if n else None
            headers = {k: v for k, v in self.headers.items() if k.lower() in PASS_REQUEST}
            try:
                conn = http.client.HTTPConnection(host, int(port or 80), timeout=3600)
                conn.request(self.command, path, body=body, headers=headers)
                resp = conn.getresponse()
            except OSError:
                return self.refuse(502, "the board is not running")
            try:
                stream = (resp.getheader("Content-Type") or "").startswith("text/event-stream")
                data = None if stream else resp.read()
                self.send_response(resp.status)
                for k, v in resp.getheaders():
                    if k.lower() not in SKIP_RESPONSE:
                        self.send_header(k, v)
                if stream:
                    self.send_header("X-Accel-Buffering", "no")
                    self.end_headers()
                    while True:
                        chunk = resp.read1(65536)
                        if not chunk:
                            break
                        self.wfile.write(chunk)
                        self.wfile.flush()
                else:
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
            except OSError:
                pass
            finally:
                conn.close()

        do_GET = do_POST = do_HEAD = handle_any

    return Gate


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8781)
    ap.add_argument("--board", default="127.0.0.1:8770")
    ap.add_argument("--token-file", default=str(ROOT / "work" / "gui" / "gate.token"))
    ap.add_argument("--control", action="store_true", help="also pass the requests that start and stop runs")
    a = ap.parse_args()
    tf = Path(a.token_file)
    if tf.is_file() and tf.read_text().strip():
        token = tf.read_text().strip()
    else:
        tf.parent.mkdir(parents=True, exist_ok=True)
        token = secrets.token_urlsafe(18)
        tf.write_text(token + "\n")
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), make_handler(a.board, token, a.control))
    srv.daemon_threads = True
    print("gate on http://127.0.0.1:%d -> %s (%s); the token is in %s" % (a.port, a.board, "control" if a.control else "watch only", tf), flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    sys.exit(main())
