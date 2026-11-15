"""Learned move order, sizing study (climb/order): loads labelled red nodes and replays orders.

Library used by tools/order/fit.py. A node is one dumped red position (from `snek bench --dump`),
labelled by `snek ordermoves` (every move: features, does it win, nodes with a fresh table, and for
a failed move the refuting reply and the threat the solver would make of it).

The replay of an order follows the solver's loop at a node with window (0, 1):
  - the table move, then the killers, first (those of the node in the tree, from the dump's meta);
  - then the moves by the order's key, highest first (ties: higher move number first);
  - after a failed move with reply R: moves that cover a cell of R get the answer bonus
    (cumulative over distinct replies), the threat of R (if any) is added, and after the first
    failure the threats of `scan_threats` are added too (at most 6 threats, as the solver);
  - a move whose cells miss the zone of a threat of value <= 0 is skipped at no cost;
  - the cost is the nodes of every move searched up to and including the first win.
What it leaves out: the threats the solver tries before the loop from the killers one ply down,
and the table shared between moves of the node (each move's nodes are its own fresh solve).
"""
import csv
import math

MODEL = [21, 17, 21, -1, 1, -3, 9, -7]


def load(csv_paths, meta_path, root_parent):
    """Returns a list of nodes: dict(level, tt, k1, k2, root, parent, tree_total, moves=[...], scan=[...])."""
    metas = [l.split() for l in open(meta_path)]
    texts = [l.split()[1] for l in open(meta_path[:-5])]
    nodes = {}
    scans = {}
    for path in csv_paths:
        with open(path) as fh:
            header = None
            last = None
            ended = False
            for line in fh:
                if line.startswith("#end"):
                    ended = True
                    continue
                if line.startswith("#scan"):
                    f = line.split()
                    scans[int(f[1])] = [(int(z.split(":")[0], 16), int(z.split(":")[1])) for z in f[2:]]
                    continue
                if line.startswith("pos,"):
                    header = line.strip().split(",")
                    continue
                if not line.strip() or header is None:
                    continue
                r = dict(zip(header, line.strip().split(",")))
                i = int(r["pos"])
                mv = {
                    "m": int(r["move"]), "piece": int(r["piece"]), "n": int(r["n"]),
                    "cells": int(r["cells"], 16), "border": int(r["border"], 16),
                    "win": r["win"], "nodes": int(r["nodes"]), "reply": int(r["reply"]),
                    "rcells": int(r["rcells"], 16),
                    "rzone": None if r["rzone"] == "-" else int(r["rzone"], 16),
                    "rvalue": None if r["rvalue"] == "-" else int(r["rvalue"]),
                }
                for k in ("lead", "grows", "joined", "front", "touch", "my_ends8", "their_ends8", "my_up",
                          "their_up", "their_moves", "my_moves", "their_best", "my_next"):
                    mv[k] = int(r[k])
                nodes.setdefault(i, []).append(mv)
                last = i
            if not ended and last is not None:
                del nodes[last]  # cut off by a time limit: its move list may be incomplete
    out = []
    for i in sorted(nodes):
        m = metas[i]
        root, level, tt, k1, k2, a, b, cut, searched, total, winsub = (int(x) for x in m[:11])
        w = float(m[11]) if len(m) > 11 else 1.0
        f = texts[i].split("_")
        stm = int(f[4])
        cells = (int(f[0], 16), int(f[1], 16))
        out.append(dict(idx=i, w=w, own=cells[stm], theirs=cells[stm ^ 1], root=root, parent=root_parent[root], level=level, tt=tt, k1=k1, k2=k2,
                        alpha=a, beta=b, tree_cut=cut, tree_searched=searched, tree_total=total,
                        tree_winsub=winsub, moves=nodes[i], scan=scans.get(i, [])))
    return out


def model_key(mv):
    x = [mv["lead"], mv["n"], mv["grows"], mv["joined"], mv["front"], mv["touch"], mv["my_ends8"], mv["their_ends8"]]
    s = sum(w * v for w, v in zip(MODEL, x))
    return max(s + (1 << 20), 0)


def static_key(mv):
    s = mv["n"] * 64
    if mv["grows"]:
        s += 256
    if mv["front"]:
        s += 128
    elif mv["touch"]:
        s += 32
    return s


def today_key(node, mv):
    return model_key(mv) if node["level"] >= 7 else static_key(mv)


def replay(node, keys, ttk=True, answer=True, threats=True):
    """Nodes paid by an order (keys: one float per move, higher first). Returns (cost, n_searched, fails_cost, win_cost)."""
    moves = node["moves"]
    n = len(moves)
    BONUS = 1e12
    TOP = 1e18
    k = [float(x) for x in keys]
    if ttk:
        byid = {mv["m"]: j for j, mv in enumerate(moves)}
        for rank, c in enumerate((node["tt"], node["k1"], node["k2"])):
            if c in byid:
                j = byid[c]
                k[j] = max(k[j], TOP * (3 - rank))
    order = sorted(range(n), key=lambda j: (k[j], moves[j]["m"]), reverse=True)
    th = []
    answered = set()
    scanned = False
    cost = fails = 0
    searched = 0
    i = 0
    while i < len(order):
        j = order[i]
        i += 1
        mv = moves[j]
        if threats and th and any(mv["cells"] & z == 0 and v <= 0 for z, v in th):
            continue
        searched += 1
        cost += mv["nodes"]
        if mv["win"] == "1":
            return cost, searched, fails, mv["nodes"]
        fails += mv["nodes"]
        r = mv["reply"]
        if answer and r != 0xFFFF and r not in answered and mv["rcells"]:
            answered.add(r)
            rest = order[i:]
            for jj in rest:
                if moves[jj]["cells"] & mv["rcells"]:
                    k[jj] += BONUS
            order[i:] = sorted(rest, key=lambda jj: (k[jj], moves[jj]["m"]), reverse=True)
        if threats:
            if mv["rzone"] is not None and len(th) < 6 and (mv["rzone"], mv["rvalue"]) not in th:
                th.append((mv["rzone"], mv["rvalue"]))
            if not scanned:
                scanned = True
                for z in node["scan"]:
                    if len(th) < 6:
                        th.append(z)
    return cost, searched, fails, 0


def oracle_cheapest(node):
    w = [mv["nodes"] for mv in node["moves"] if mv["win"] == "1"]
    return min(w)


def oracle_any(node):
    """Any winning move first: the first win in today's order, searched first."""
    keys = [today_key(node, mv) for mv in node["moves"]]
    wins = [j for j, mv in enumerate(node["moves"]) if mv["win"] == "1"]
    best = max(wins, key=lambda j: (keys[j], node["moves"][j]["m"]))
    return node["moves"][best]["nodes"]


def oracle_mean(node):
    w = [mv["nodes"] for mv in node["moves"] if mv["win"] == "1"]
    return sum(w) / len(w)


def usable(node):
    """A node counts when some move wins and no move hit the node cap."""
    ms = node["moves"]
    return any(mv["win"] == "1" for mv in ms) and not any(mv["win"] == "?" for mv in ms)
