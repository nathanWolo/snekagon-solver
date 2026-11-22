"""Makes the public copy of this repository.

    python tools/publish.py OUT_DIR [--skip GLOB ...] [--drop PATH ...] [--scrub-paths]
                                    [--keep-trailers] [--force]

The commit messages of the working repository carry "Co-Authored-By: Claude ..." lines. The public
copy is the same history without them: every commit keeps its tree, its parents, its author, its
dates and the rest of its message, and so gets a new hash. Nothing in the working repository is
changed. OUT_DIR becomes a bare repository holding

    every local branch            except those matching a --skip pattern (fnmatch, e.g. "climb/x*")
    commit-map                    a branch with one file, commit-map.tsv: the hash of every commit
                                  here and in the public copy, since the docs cite the hashes here

--drop PATH       leaves a file (its exact path) out of every commit, contents and all: for a file
                  that must not be published while the rest of its branch may.
--scrub-paths     replaces, in the contents of every text file, the folders of this machine that
                  scripts and logs name (the user's home, the app's session folder, its scratch
                  folder) by <user>, <session-folder> and <scratchpad>.

The copy is checked before the script says "ok". Every commit must have the same people and dates
as its original, parents that are the copies of the original's parents, and a message that differs
by the removed lines only. Its files must be the original's files: the same tree, or, where an
option changed something, the same paths and modes except the dropped files, each changed file
equal to its original after the replacement. A dropped file's contents must be gone from the copy,
and with --scrub-paths no file of the copy may still name the user's home. With --keep-trailers
and no other option nothing is changed and every hash must come out the same, which tests the
copying itself.

The same input gives the same output, so a later run gives the old commits the same hashes and a
push of the copy is an ordinary fast-forward (the branch commit-map alone is replaced each time):

    git -C OUT_DIR push URL 'refs/heads/*:refs/heads/*' '+refs/heads/commit-map:refs/heads/commit-map'

Pushing is the owner's call: this script only builds the copy. The options the public copy is made
with, and why, are in CLAUDE.md ("The public copy").
"""
import argparse
import fnmatch
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ATTRIBUTION = re.compile(rb"^\s*(co-authored-by:\s*claude\b.*|.*generated with \[claude code\].*)$", re.I)
FIELDS = "%H%x00%T%x00%P%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%B%x01"


def git(*args, cwd=ROOT, data=None):
    return subprocess.run(["git", *args], cwd=cwd, input=data, capture_output=True, check=True).stdout


def clean(message):
    """The message without the attribution lines and without the blank lines they leave at the end."""
    lines = message.split(b"\n")
    kept = [l for l in lines if not ATTRIBUTION.match(l)]
    if len(kept) == len(lines):
        return message
    while kept and not kept[-1].strip():
        kept.pop()
    return b"\n".join(kept) + b"\n"


def path_rules():
    """The folders of this machine, as scripts and logs write them: C:\\Users\\NAME\\..., C:/Users/NAME/...,
    /c/Users/NAME/... The name is this machine's, read here and written nowhere."""
    user = re.escape(Path.home().name.encode())
    sep = rb"[\\/]+"
    home = rb"(?:[A-Za-z]:|/[a-z]|/mnt/[a-z])" + sep + rb"Users" + sep + user
    local = home + sep + rb"AppData" + sep + rb"Local" + sep
    part = rb"[\w.-]+"
    return [
        (re.compile(local + rb"Claude-Data" + sep + part + sep + rb"scratch-workspaces" + sep + part + sep + part + sep + rb"scratch-[\w-]+"),
         b"<session-folder>"),
        (re.compile(local + rb"Temp" + sep + rb"claude" + sep + part + sep + part + sep + rb"scratchpad"), b"<scratchpad>"),
        (re.compile(rb"(Users" + sep + rb")" + user + rb"(?![\w])"), rb"\1<user>"),
    ], re.compile(rb"Users" + sep + user + rb"(?![\w])")


def scrubbed(data, rules):
    if b"\0" in data:
        return data
    for pattern, text in rules:
        data = pattern.sub(text, data)
    return data


def read_exactly(stream, n):
    parts = []
    while n:
        part = stream.read(n)
        if not part:
            raise SystemExit("the export ended inside a data block")
        parts.append(part)
        n -= len(part)
    return b"".join(parts)


def copy(refs, out, keep, drops, rules):
    """fast-export | (messages cleaned, files dropped or scrubbed) | fast-import. Returns {original commit: its copy}."""
    drops = {d.encode() for d in drops}
    marks_file = out / "publish-marks.txt"
    src = subprocess.Popen(["git", "fast-export", "--show-original-ids", "--reencode=no", "--signed-tags=strip", *refs],
                           cwd=ROOT, stdout=subprocess.PIPE)
    dst = subprocess.Popen(["git", "fast-import", "--quiet", "--export-marks=" + str(marks_file)], cwd=out, stdin=subprocess.PIPE)
    mark_of = {}
    kind = None
    message_next = False
    mark = None
    for line in iter(src.stdout.readline, b""):
        if line.startswith(b"data "):
            body = read_exactly(src.stdout, int(line[5:]))
            if message_next:
                message_next = False
                if not keep:
                    body = clean(body)
            elif kind == "blob" and rules:
                body = scrubbed(body, rules)
            dst.stdin.write(b"data %d\n" % len(body))
            dst.stdin.write(body)
            continue
        if line.startswith(b"commit "):
            kind, mark = "commit", None
        elif line == b"blob\n":
            kind = "blob"
        elif line.startswith((b"reset ", b"tag ")):
            kind = None
        elif kind == "commit" and line.startswith(b"mark :"):
            mark = line[5:].strip().decode()
        elif kind == "commit" and line.startswith(b"original-oid "):
            mark_of[line.split()[1].decode()] = mark
        elif kind == "commit" and line.startswith(b"committer "):
            message_next = True
        elif kind == "commit" and drops and line.startswith((b"M ", b"D ")):
            # "M <mode> <blob> <path>" or "D <path>": a dropped file never enters the copy's trees
            path = line.rstrip(b"\n").split(b" ", 3 if line.startswith(b"M ") else 1)[-1]
            if path in drops:
                continue
        dst.stdin.write(line)
    dst.stdin.close()
    if src.wait() or dst.wait():
        raise SystemExit("git fast-export or fast-import failed")
    new_of_mark = dict(l.split() for l in marks_file.read_text().splitlines())
    marks_file.unlink()
    if drops:
        # the dropped files' contents came through the stream as blobs nothing points to: take them out
        git("reflog", "expire", "--expire=now", "--all", cwd=out)
        git("gc", "--quiet", "--prune=now", cwd=out)
    return {old: new_of_mark[m] for old, m in mark_of.items()}


def records(cwd, refs):
    out = {}
    for rec in git("log", "--format=" + FIELDS, "--date=raw", *refs, cwd=cwd).split(b"\x01"):
        rec = rec.lstrip(b"\n")
        if rec:
            f = rec.split(b"\x00")
            out[f[0].decode()] = f[1:]
    return out


def entries(cwd, commit, drops=()):
    out = {}
    for l in git("ls-tree", "-r", commit, cwd=cwd).decode().splitlines():
        meta, path = l.split("\t", 1)
        if path not in drops:
            out[path] = tuple(meta.split())
    return out


def check(refs, out, new_of, keep, drops, rules, leak):
    old, new = records(ROOT, refs), records(out, ["--all"])
    if len(old) != len(new) or set(old) != set(new_of):
        raise SystemExit(f"commit counts differ: {len(old)} here, {len(new)} in the copy, {len(new_of)} mapped")
    changed = 0
    trees, blobs = {}, {}

    def same_file(a, b):
        if (a, b) not in blobs:
            blobs[(a, b)] = bool(rules) and scrubbed(git("cat-file", "blob", a), rules) == git("cat-file", "blob", b, cwd=out)
        return blobs[(a, b)]

    def same_files(h, ta, tb):
        if (ta, tb) not in trees:
            a, b = entries(ROOT, h, drops), entries(out, new_of[h])
            trees[(ta, tb)] = a.keys() == b.keys() and all(
                a[p] == b[p] or (a[p][:2] == b[p][:2] and same_file(a[p][2], b[p][2])) for p in a)
        return trees[(ta, tb)]

    for h, f in old.items():
        g = new.get(new_of[h])
        if g is None:
            raise SystemExit(f"{h}: its copy {new_of[h]} is not in the copy's history")
        if f[2:8] != g[2:8]:
            raise SystemExit(f"{h}: people or dates differ in the copy")
        if f[0] != g[0] and not ((drops or rules) and same_files(h, f[0], g[0])):
            raise SystemExit(f"{h}: the copy's files differ by more than the options allow")
        if [new_of[p] for p in f[1].decode().split()] != g[1].decode().split():
            raise SystemExit(f"{h}: the copy's parents are not the copies of its parents")
        if (f[8] if keep else clean(f[8])) != g[8]:
            raise SystemExit(f"{h}: the message differs by more than the removed lines")
        if keep and not drops and not rules and new_of[h] != h:
            raise SystemExit(f"{h}: copied unchanged, but its hash came out as {new_of[h]}")
        changed += f[8] != g[8]
        if not keep and any(ATTRIBUTION.match(l) for l in g[8].split(b"\n")):
            raise SystemExit(f"{h}: an attribution line is left in the copy")
    for ref in refs:
        tip = git("rev-parse", ref).decode().strip()
        if git("rev-parse", ref, cwd=out).decode().strip() != new_of[tip]:
            raise SystemExit(f"{ref}: the copy's tip is not the copy of the tip here")
    if drops:
        if git("log", "--all", "--format=%H", "--", *drops, cwd=out).strip():
            raise SystemExit("a dropped file is still in the copy's history")
        gone = {l.split()[0] for l in git("rev-list", "--objects", *refs).decode().splitlines()
                if " " in l and l.split(" ", 1)[1] in drops}
        for b in gone:
            if subprocess.run(["git", "cat-file", "-e", b], cwd=out, capture_output=True).returncode == 0:
                raise SystemExit(f"the contents of a dropped file ({b[:10]}) are still in the copy")
    if rules:
        # every file of every commit of the copy, once each: none may still name the user's home
        ids = sorted({l.split()[0] for l in git("rev-list", "--objects", "--all", cwd=out).decode().splitlines() if " " in l})
        dump = git("cat-file", "--batch", cwd=out, data=("\n".join(ids) + "\n").encode())
        at = 0
        while at < len(dump):
            end = dump.index(b"\n", at)
            oid, kind, size = dump[at:end].decode().split()
            body = dump[end + 1:end + 1 + int(size)]
            at = end + 1 + int(size) + 1
            if kind == "blob" and leak.search(body):
                raise SystemExit(f"a file of the copy ({oid[:10]}) still names the user's home")
    return len(old), changed, len([1 for v in trees.values() if v]), len([1 for v in blobs.values() if v])


def add_map(out, new_of):
    """An orphan branch commit-map with commit-map.tsv, dated like the newest commit of the copy."""
    subject = {}
    for l in git("log", "--all", "--format=%H%x09%s", cwd=out).decode("utf-8", "replace").splitlines():
        h, _, s = l.partition("\t")
        subject[h] = s
    rows = sorted((new, old) for old, new in new_of.items())
    text = "public_commit\tworking_commit\tsubject\n" + "".join(f"{n}\t{o}\t{subject.get(n, '')}\n" for n, o in rows)
    blob = git("hash-object", "-w", "--stdin", cwd=out, data=text.encode("utf-8")).decode().strip()
    tree = git("mktree", cwd=out, data=f"100644 blob {blob}\tcommit-map.tsv\n".encode()).decode().strip()
    who = git("log", "--all", "-1", "--date-order", "--format=%an%x00%ae%x00%cd", "--date=raw", cwd=out).decode().strip().split("\x00")
    env = {"GIT_AUTHOR_NAME": who[0], "GIT_AUTHOR_EMAIL": who[1], "GIT_AUTHOR_DATE": who[2],
           "GIT_COMMITTER_NAME": who[0], "GIT_COMMITTER_EMAIL": who[1], "GIT_COMMITTER_DATE": who[2]}
    commit = subprocess.run(["git", "commit-tree", tree, "-m", "The hashes of the commits here and in the working repository"],
                            cwd=out, env={**os.environ, **env}, capture_output=True, check=True).stdout.decode().strip()
    git("update-ref", "refs/heads/commit-map", commit, cwd=out)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("out")
    ap.add_argument("--skip", action="append", default=[], help="a branch name pattern to leave out (may be given several times)")
    ap.add_argument("--drop", action="append", default=[], help="a file to leave out of every commit (its exact path; may be given several times)")
    ap.add_argument("--scrub-paths", action="store_true", help="replace this machine's folders in file contents by placeholders")
    ap.add_argument("--keep-trailers", action="store_true", help="remove no line from the messages")
    ap.add_argument("--force", action="store_true", help="replace OUT_DIR if it exists")
    a = ap.parse_args()
    out = Path(a.out).resolve()
    if out.exists():
        if not a.force:
            raise SystemExit(f"{out} exists (pass --force to replace it)")
        shutil.rmtree(out, onerror=lambda f, p, e: (Path(p).chmod(0o700), f(p)))
    names = git("for-each-ref", "--format=%(refname:short)", "refs/heads").decode().split()
    if "commit-map" in names:
        raise SystemExit("a local branch is called commit-map: that name is taken by the copy")
    skipped = [n for n in names if any(fnmatch.fnmatch(n, p) for p in a.skip)]
    refs = ["refs/heads/" + n for n in names if n not in skipped]
    rules, leak = path_rules() if a.scrub_paths else ([], None)
    git("init", "--quiet", "--bare", str(out))
    new_of = copy(refs, out, a.keep_trailers, a.drop, rules)
    commits, changed, thinner, files = check(refs, out, new_of, a.keep_trailers, a.drop, rules, leak)
    if not a.keep_trailers or a.drop or rules:
        add_map(out, new_of)
    here = git("symbolic-ref", "--short", "HEAD").decode().strip()
    if here not in skipped:
        git("symbolic-ref", "HEAD", "refs/heads/" + here, cwd=out)
    print(f"ok: {len(refs)} branches, {commits} commits, {changed} messages changed")
    print(f"branches left out: {', '.join(skipped) or 'none'}")
    print(f"files dropped: {', '.join(a.drop) or 'none'}; folders replaced in {files} versions of files; {thinner} different trees checked file by file")
    print(f"the copy is in {out}")


if __name__ == "__main__":
    sys.exit(main())
