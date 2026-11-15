#!/usr/bin/env python3
"""hc.py: decide from noisy paired measurements. Standard library only.

  report   the estimate with a 95% interval
  sprt     a sequential test that stops as soon as the data decide
  plan     how many units a test needs, given the noise

A unit is one independent paired comparison of the candidate against the base on the same case: the
same opening played from both sides, the same benchmark input, the same seed. Pairing removes the
variance the cases share, which is usually most of it.

Two modes:
  game    each unit has a score in [0, 1], the candidate's share of the points (1 win, 0.5 draw, 0 loss;
          for a pair of games, their mean). Results print as Elo.
  metric  each unit has a difference, candidate minus base (with --ratio, the log of candidate over
          base). Results print in the metric's own units. With --sign each unit counts only as won,
          tied or lost, which holds up when a few cases would otherwise dominate the mean.

Input is a file with one unit per line, oldest first ("-" reads standard input). A line is a number, or
a JSON object with "score" (game), "diff" (metric), or "cand" and "base" (metric). Games can also be
given as counts with --wdl or --penta; counts carry no order, so sprt then judges once, at the end.

Exit status of sprt: 0 when H1 is accepted, 1 when H0 is accepted, 3 when undecided, 2 on a usage error.
"""
from __future__ import annotations

import argparse
import json
import math
import statistics
import sys

ND = statistics.NormalDist()
Z95 = ND.inv_cdf(0.975)


def die(msg: str) -> None:
    print(f"hc.py: {msg}", file=sys.stderr)
    sys.exit(2)


# ---------------------------------------------------------------------------------------------- Elo

def score_of(elo: float, null: float = 0.5) -> float:
    """The score that is `elo` above the null score (0.5 for two sides, 1/N for a win rate among N)."""
    k = 10 ** (elo / 400) * null / (1 - null)
    return k / (1 + k)


def elo_of(score: float, null: float = 0.5) -> float:
    if score <= 0:
        return -math.inf
    if score >= 1:
        return math.inf
    return 400 * math.log10(score / (1 - score) * (1 - null) / null)


def elo_slope(score: float) -> float:
    """Elo per unit of score at `score`: the factor that turns an interval of score into one of Elo."""
    score = min(max(score, 1e-9), 1 - 1e-9)
    return 400 / (math.log(10) * score * (1 - score))


# -------------------------------------------------------------------------------------------- input

def read_units(args) -> tuple[list[float], bool, str | None]:
    """Returns the unit values, whether they are in time order, and their kind ('score', 'diff', None)."""
    if getattr(args, "wdl", None):
        w, d, l = args.wdl
        return [1.0] * w + [0.5] * d + [0.0] * l, False, "score"
    if getattr(args, "penta", None):
        values: list[float] = []
        for count, score in zip(args.penta, (0.0, 0.25, 0.5, 0.75, 1.0)):
            values += [score] * count
        return values, False, "score"
    if not args.file:
        die("give a results file, or counts with --wdl or --penta")
    values, kinds = [], set()
    fh = sys.stdin if args.file == "-" else open(args.file, encoding="utf-8")
    with fh:
        for lineno, line in enumerate(fh, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            try:
                values.append(float(line))
                kinds.add(None)
                continue
            except ValueError:
                pass
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                die(f"{args.file}:{lineno}: neither a number nor a JSON object")
            if "score" in rec:
                values.append(float(rec["score"]))
                kinds.add("score")
            elif "diff" in rec:
                values.append(float(rec["diff"]))
                kinds.add("diff")
            elif "cand" in rec and "base" in rec:
                cand, base = float(rec["cand"]), float(rec["base"])
                if args.ratio:
                    if cand <= 0 or base <= 0:
                        die(f"{args.file}:{lineno}: --ratio needs positive values")
                    values.append(math.log(cand / base))
                else:
                    values.append(cand - base)
                kinds.add("diff")
            else:
                die(f'{args.file}:{lineno}: no "score", "diff" or "cand" and "base"')
    if len(kinds) > 1:
        die(f"{args.file}: lines of different kinds; use one kind per file")
    return values, True, (kinds.pop() if kinds else None)


def resolve_mode(args, kind: str | None, values: list[float]) -> str:
    wants_game = any(getattr(args, a, None) is not None for a in ("elo0", "elo1"))
    wants_metric = any(getattr(args, a, None) is not None for a in ("h0", "h1", "p0", "p1")) or getattr(args, "sign", False)
    if wants_game and wants_metric:
        die("give Elo hypotheses (game mode) or metric hypotheses (--h0/--h1, or --sign with --p0/--p1), not both")
    mode = args.mode or {"score": "game", "diff": "metric"}.get(kind) or ("game" if wants_game else "metric" if wants_metric else None)
    if mode is None:
        die("the input is bare numbers: say --mode game or --mode metric")
    if (mode == "game" and (kind == "diff" or wants_metric)) or (mode == "metric" and (kind == "score" or wants_game)):
        die(f"{mode} mode does not fit this input or these hypotheses")
    if mode == "game" and any(v < 0 or v > 1 for v in values):
        die("game mode needs every score in [0, 1]")
    return mode


def null_score(args) -> float:
    if args.players is not None and args.null_score is not None:
        die("give --players or --null-score, not both")
    if args.players is not None:
        if args.players < 2:
            die("--players needs at least 2")
        return 1 / args.players
    if args.null_score is not None:
        if not 0 < args.null_score < 1:
            die("--null-score must lie strictly between 0 and 1")
        return args.null_score
    return 0.5


def load(args) -> tuple[list[float], bool, str, float]:
    values, ordered, kind = read_units(args)
    mode = resolve_mode(args, kind, values)
    if mode == "metric" and args.lower_is_better:
        values = [-v for v in values]
    if getattr(args, "sign", False):
        values = [1.0 if v > 0 else 0.0 if v < 0 else 0.5 for v in values]
        mode = "sign"
    cap = getattr(args, "max_units", None)
    if cap:
        values = values[:cap]
    return values, ordered, mode, (null_score(args) if mode == "game" else 0.5 if mode == "sign" else 0.0)


# ------------------------------------------------------------------------------------------- stats

def summarize(values: list[float]) -> dict:
    n = len(values)
    if n < 2:
        die(f"{n} unit(s): at least 2 are needed")
    mean = math.fsum(values) / n
    var = math.fsum((v - mean) ** 2 for v in values) / (n - 1)
    sd = math.sqrt(var)
    return {"n": n, "mean": mean, "sd": sd, "se": sd / math.sqrt(n)}


def estimate(values: list[float], mode: str, null: float, ratio: bool = False) -> dict:
    s = summarize(values)
    half = Z95 * s["se"]
    centre = null if mode in ("game", "sign") else 0.0
    s["half"] = half
    if mode != "game" or null == 0.5:
        s["decisive"] = sum(v != centre for v in values)
    s["p_no_better"] = 1 - ND.cdf((s["mean"] - centre) / s["se"]) if s["se"] > 0 else (0.0 if s["mean"] > centre else 1.0 if s["mean"] < centre else 0.5)
    if mode == "game":
        s["elo"] = elo_of(s["mean"], null)
        s["elo_half"] = half * elo_slope(s["mean"])
        s["elo_low"] = elo_of(s["mean"] - half, null)
        s["elo_high"] = elo_of(s["mean"] + half, null)
    elif mode == "sign":
        s["won"] = sum(v == 1.0 for v in values)
        s["tied"] = sum(v == 0.5 for v in values)
        s["lost"] = sum(v == 0.0 for v in values)
    elif ratio:
        s["pct"] = (math.exp(s["mean"]) - 1) * 100
        s["pct_low"] = (math.exp(s["mean"] - half) - 1) * 100
        s["pct_high"] = (math.exp(s["mean"] + half) - 1) * 100
    return s


def fmt_estimate(s: dict, mode: str, ratio: bool = False) -> list[str]:
    lines = [f"units        {s['n']}" + (f"   ({s['decisive']} decisive, the rest tied)" if "decisive" in s else "")]
    if mode == "game":
        lines.append(f"score        {s['mean']:.4f} +/- {s['half']:.4f} (95%)   sd per unit {s['sd']:.4f}")
        lines.append(f"Elo          {s['elo']:+.1f} +/- {s['elo_half']:.1f} (95%)   interval [{s['elo_low']:+.1f}, {s['elo_high']:+.1f}]")
    elif mode == "sign":
        lines.append(f"share won    {s['mean']:.4f} +/- {s['half']:.4f} (95%)   won {s['won']}, tied {s['tied']}, lost {s['lost']}")
    else:
        name = "log ratio   " if ratio else "difference  "
        lines.append(f"{name} {s['mean']:+.6g} +/- {s['half']:.6g} (95%)   sd per unit {s['sd']:.6g}")
        if ratio:
            lines.append(f"ratio        {s['pct']:+.2f}%   interval [{s['pct_low']:+.2f}%, {s['pct_high']:+.2f}%]")
    lines.append(f"one-sided p  {s['p_no_better']:.4f} that the candidate is no better")
    return lines


def hypotheses(args, mode: str, null: float) -> tuple[float, float, str]:
    """The two means the test separates, and a label for them."""
    if mode == "game":
        if args.elo1 is None:
            die("game mode needs --elo1 (and --elo0, 0 by default)")
        elo0 = args.elo0 or 0.0
        if args.elo1 <= elo0:
            die("--elo1 must be above --elo0")
        return score_of(elo0, null), score_of(args.elo1, null), f"SPRT[{elo0:g}, {args.elo1:g}] Elo"
    if mode == "sign":
        if args.p1 is None:
            die("--sign needs --p1, the share of cases won under H1 (and --p0, 0.5 by default)")
        p0 = 0.5 if args.p0 is None else args.p0
        if not 0 < p0 < args.p1 < 1:
            die("--p0 and --p1 must satisfy 0 < p0 < p1 < 1")
        return p0, args.p1, f"SPRT[{p0:g}, {args.p1:g}] share won"
    if args.h1 is None:
        die("metric mode needs --h1 (and --h0, 0 by default), or --sign with --p1")
    h0 = args.h0 or 0.0
    if args.h1 <= h0:
        die("--h1 must be above --h0 (use --lower-is-better when a smaller metric is the goal)")
    return h0, args.h1, f"SPRT[{h0:g}, {args.h1:g}]"


def bounds(alpha: float, beta: float) -> tuple[float, float]:
    return math.log(beta / (1 - alpha)), math.log((1 - beta) / alpha)


def llr(n: int, mean: float, var: float, mu0: float, mu1: float) -> float:
    """Log likelihood ratio of mean mu1 against mean mu0 with the variance estimated from the data."""
    return n * (mu1 - mu0) * (2 * mean - mu0 - mu1) / (2 * var)


def run_sprt(values: list[float], ordered: bool, mu0: float, mu1: float, alpha: float, beta: float, min_units: int) -> dict:
    """Walks the units in order and stops at the first bound crossed, so later units cannot change a decision."""
    lower, upper = bounds(alpha, beta)
    s1 = s2 = 0.0
    last = 0.0
    first = max(2, min_units)
    for k, x in enumerate(values, 1):
        s1 += x
        s2 += x * x
        if k < first or (not ordered and k < len(values)):
            continue
        mean = s1 / k
        var = (s2 - k * mean * mean) / (k - 1)
        if var <= 1e-15:
            continue
        last = llr(k, mean, var, mu0, mu1)
        if last >= upper:
            return {"decision": "H1", "at": k, "llr": last, "lower": lower, "upper": upper}
        if last <= lower:
            return {"decision": "H0", "at": k, "llr": last, "lower": lower, "upper": upper}
    return {"decision": None, "at": len(values), "llr": last, "lower": lower, "upper": upper}


# ---------------------------------------------------------------------------------------- commands

def cmd_report(args) -> int:
    values, _, mode, null = load(args)
    s = estimate(values, mode, null, args.ratio)
    centre = null if mode in ("game", "sign") else 0.0
    if args.aa:
        s["aa_ok"] = abs(s["mean"] - centre) <= s["half"]
    if args.json:
        print(json.dumps({"mode": mode, **s}))
        return 0
    print("\n".join(fmt_estimate(s, mode, args.ratio)))
    if args.aa:
        if s["aa_ok"]:
            print(f"A/A          consistent with no difference; use sd {s['sd']:.6g} per unit with `plan`")
        else:
            print("A/A          BIASED: the base against itself does not read zero. Fix the harness before testing anything.")
    return 0


def cmd_sprt(args) -> int:
    values, ordered, mode, null = load(args)
    mu0, mu1, label = hypotheses(args, mode, null)
    r = run_sprt(values, ordered, mu0, mu1, args.alpha, args.beta, args.min_units)
    s = estimate(values[: r["at"]], mode, null, args.ratio)
    capped = bool(args.max_units) and r["decision"] is None and len(values) >= args.max_units
    code = {"H1": 0, "H0": 1}.get(r["decision"], 3)
    if args.json:
        print(json.dumps({"mode": mode, "test": label, "units_given": len(values), "capped": capped, **r, **s}))
        return code
    print("\n".join(fmt_estimate(s, mode, args.ratio)))
    print(f"{label}   alpha {args.alpha:g} beta {args.beta:g}   LLR {r['llr']:+.2f}   bounds {r['lower']:+.2f} / {r['upper']:+.2f}")
    if r["decision"] == "H1":
        print(f"decision     H1 accepted at unit {r['at']} of {len(values)}: PASS")
    elif r["decision"] == "H0":
        print(f"decision     H0 accepted at unit {r['at']} of {len(values)}: FAIL")
    elif capped:
        print(f"decision     none: stopped at the cap of {args.max_units} units without crossing a bound")
    else:
        print("decision     none yet: add units and run again")
    if r["decision"] and s.get("decisive", 10) < 10:
        print(f"note         only {s['decisive']} decisive unit(s): a verdict reached mostly on ties is not evidence")
    if r["decision"] and r["at"] < len(values):
        print(f"note         {len(values) - r['at']} later unit(s) ignored: the test had already stopped")
    if r["decision"]:
        print("note         an estimate read where an SPRT stops leans toward the bound it crossed; measure the size in a separate run of fixed length if the size matters")
    if not ordered:
        print("note         counts carry no order, so this judged once at the end; give the units in order to stop early")
    return code


def cmd_plan(args) -> int:
    mode = "sign" if args.sign else args.mode or ("game" if args.elo1 is not None else "metric" if args.h1 is not None else None)
    if mode is None:
        die("give --elo1 (game), --h1 (metric) or --sign with --p1")
    null = null_score(args) if mode == "game" else 0.5 if mode == "sign" else 0.0
    source = ""
    if args.sd is not None:
        sd = args.sd
    elif mode == "sign" and not args.file:
        sd, source = 0.5, " (the most a share can have)"
    elif args.file:
        values, _, file_mode, _ = load(args)
        if file_mode != mode:
            die(f"the file holds {file_mode} units but the hypotheses are for {mode} mode")
        s = summarize(values)
        sd, source = s["sd"], f" (from {args.file}, {s['n']} units)"
    else:
        die("give the noise: --sd, or a pilot or A/A file to estimate it from")
    if sd <= 0:
        die("the noise must be positive")
    mu0, mu1, label = hypotheses(args, mode, null)
    delta = mu1 - mu0
    lower, upper = bounds(args.alpha, args.beta)
    drift = delta * delta / (2 * sd * sd)
    n_h1 = ((1 - args.beta) * upper + args.beta * lower) / drift
    n_h0 = ((1 - args.alpha) * lower + args.alpha * upper) / -drift
    n_mid = -lower * upper / (delta * delta / (sd * sd))
    z = ND.inv_cdf(1 - args.alpha) + ND.inv_cdf(1 - args.beta)
    n_fixed = (z * sd / delta) ** 2
    n_ci = (Z95 * sd / delta) ** 2
    out = {"mode": mode, "sd": sd, "delta": delta, "sprt_h0": n_h0, "sprt_h1": n_h1, "sprt_midway": n_mid, "fixed": n_fixed, "interval": n_ci}
    if args.budget:
        small = z * sd / math.sqrt(args.budget)
        out["resolvable"] = (elo_of(mu0 + small, null) - elo_of(mu0, null)) if mode == "game" else small
    if args.json:
        print(json.dumps(out))
        return 0
    print(f"noise        sd {sd:.6g} per unit{source}")
    print(f"hypotheses   {label}: {delta:.6g} apart in the unit's scale")
    print(f"SPRT         about {math.ceil(n_h0)} units if the truth is H0, {math.ceil(n_h1)} if it is H1, {math.ceil(n_mid)} if it lies halfway (the slowest case)")
    print(f"fixed size   {math.ceil(n_fixed)} units for the same error rates; {math.ceil(n_ci)} for a 95% interval as narrow as the gap")
    if args.rate:
        print(f"time         at {args.rate:g} units/hour: SPRT {n_h1 / args.rate:.2f} to {n_mid / args.rate:.2f} h, fixed size {n_fixed / args.rate:.2f} h")
    if args.budget:
        unit = " Elo" if mode == "game" else ""
        print(f"budget       {args.budget} units resolve an effect of about {out['resolvable']:.3g}{unit} at these error rates; smaller ideas need a cheaper test or more units")
    return 0


# ------------------------------------------------------------------------------------------- parser

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="hc.py", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)

    def common(q, counts=True):
        q.add_argument("file", nargs="?", help='results, one unit per line, oldest first ("-" for standard input)')
        q.add_argument("--mode", choices=["game", "metric"], help="needed only when the input is bare numbers and no hypothesis says which")
        q.add_argument("--ratio", action="store_true", help='metric mode with "cand" and "base": use the log of their ratio')
        q.add_argument("--lower-is-better", action="store_true", help="metric mode: a smaller value is an improvement")
        q.add_argument("--sign", action="store_true", help="metric mode: count each unit as won, tied or lost and test the share won")
        q.add_argument("--players", type=int, help="game mode: the score is a win rate among this many players (null 1/N)")
        q.add_argument("--null-score", type=float, help="game mode: the score of an equal candidate (default 0.5)")
        q.add_argument("--json", action="store_true", help="print one JSON object")
        if counts:
            q.add_argument("--wdl", type=int, nargs=3, metavar=("W", "D", "L"), help="game mode: unpaired games as counts")
            q.add_argument("--penta", type=int, nargs=5, metavar=("LL", "LD", "DD_WL", "WD", "WW"), help="game mode: pairs of games as counts by pair score 0, 0.5, 1, 1.5, 2")

    def tests(q):
        q.add_argument("--elo0", type=float, help="game mode: H0 (default 0)")
        q.add_argument("--elo1", type=float, help="game mode: H1")
        q.add_argument("--h0", type=float, help="metric mode: H0, in the units of the difference (default 0)")
        q.add_argument("--h1", type=float, help="metric mode: H1")
        q.add_argument("--p0", type=float, help="with --sign: H0, the share of cases won (default 0.5)")
        q.add_argument("--p1", type=float, help="with --sign: H1")
        q.add_argument("--alpha", type=float, default=0.05, help="chance of accepting H1 when H0 is true (default 0.05)")
        q.add_argument("--beta", type=float, default=0.05, help="chance of accepting H0 when H1 is true (default 0.05)")

    q = sub.add_parser("report", help="the estimate with a 95%% interval")
    common(q)
    q.add_argument("--aa", action="store_true", help="the input is the base against itself: check that it reads zero and print the noise")
    q.set_defaults(func=cmd_report)

    q = sub.add_parser("sprt", help="sequential test: stop as soon as the data decide")
    common(q)
    tests(q)
    q.add_argument("--min-units", type=int, default=30, help="no decision before this many units, while the variance estimate is poor (default 30)")
    q.add_argument("--max-units", type=int, help="use at most this many units")
    q.set_defaults(func=cmd_sprt)

    q = sub.add_parser("plan", help="how many units a test needs, given the noise")
    common(q, counts=False)
    tests(q)
    q.add_argument("--sd", type=float, help="noise per unit; otherwise estimated from the file (a pilot or an A/A run)")
    q.add_argument("--rate", type=float, help="units per hour, to print durations")
    q.add_argument("--budget", type=int, help="units you can afford: print the smallest effect they resolve")
    q.set_defaults(func=cmd_plan)
    return p


def main(argv: list[str] | None = None) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    args = build_parser().parse_args(argv)
    for name in ("alpha", "beta"):
        v = getattr(args, name, None)
        if v is not None and not 0 < v < 0.5:
            die(f"--{name} must lie strictly between 0 and 0.5")
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
