"use strict";
(() => {
  const $ = id => document.getElementById(id);
  const svgNS = "http://www.w3.org/2000/svg";
  const SIDE = ["Red", "Blue"];
  const S3 = Math.sqrt(3);
  const cx = c => S3 * (SN.cq(c) + SN.cr(c) / 2);
  const cy = c => 1.5 * SN.cr(c);
  const client = Math.random().toString(36).slice(2) + Date.now().toString(36);

  // ---------------------------------------------------------------- the line of play
  const st = { base: SN.initial(), baseText: null, line: [], cur: 0, positions: [SN.initial()] };
  let legal = [];       // legal placements in the shown position
  let playable = new Set();
  let drag = null;      // {cells: []}
  let hint = null;      // cells to outline (hovered result)
  let info = { cpus: 16, typical: {} };
  let run = null;       // the last run snapshot from the server
  let cancelling = false; // Cancel was pressed and the run has not ended yet
  const cache = new Map(); // position text -> {mode, results: Map(move -> result), value, best, status, seconds}
  const proofs = new Map(); // position text -> its proof store's state (rows of replies), null when it has none
  let lastFollow = null;  // the reply the board last followed

  const pos = () => st.positions[st.cur];
  const posText = () => SN.text(pos());

  function setLine(base, baseText, line, cur) {
    st.base = base; st.baseText = baseText; st.line = line;
    st.positions = [base];
    for (const m of line) st.positions.push(SN.play(st.positions[st.positions.length - 1], m));
    st.cur = cur === undefined ? line.length : cur;
    changed();
  }
  function playMove(pl) {
    const next = st.line[st.cur];
    if (st.cur < st.line.length && (next === pl || (next && pl && next.id === pl.id))) { st.cur++; changed(); return; }
    st.line = st.line.slice(0, st.cur).concat([pl]);
    st.positions = st.positions.slice(0, st.cur + 1);
    st.positions.push(SN.play(pos(), pl));
    st.cur++;
    changed();
  }
  function go(k) { st.cur = Math.max(0, Math.min(st.line.length, k)); changed(); }

  function changed() {
    legal = SN.moves(pos());
    playable = new Set();
    for (const pl of legal) for (const c of pl.cells) playable.add(c);
    drag = null; hint = null;
    $("msg").textContent = "";
    render();
    fetchProof(posText());
  }
  // the state of a position's proof store, read once per position (a run's events keep it current)
  async function fetchProof(text) {
    if (proofs.has(text) || SN.isOver(pos())) return;
    proofs.set(text, null);
    try {
      const j = await (await fetch("/api/proof?position=" + encodeURIComponent(text))).json();
      if (j.ok && j.proof && !(run && run.mode === "prove" && run.position === text && proofs.get(text))) {
        proofs.set(text, j.proof);
        renderVerdict();
      }
    } catch (e) { proofs.delete(text); }
  }

  // ---------------------------------------------------------------- board
  const board = $("board");
  let cellEls = [];
  let HEX = "";
  function buildBoard() {
    const xs = SN.CELLS.map(cx), ys = SN.CELLS.map(cy);
    const pad = 1.1;
    const x0 = Math.min(...xs) - pad, y0 = Math.min(...ys) - pad;
    board.setAttribute("viewBox", [x0, y0, Math.max(...xs) - x0 + pad, Math.max(...ys) - y0 + pad].join(" "));
    const hex = [];
    for (let k = 0; k < 6; k++) {
      const a = Math.PI / 180 * (60 * k - 30);
      hex.push((0.97 * Math.cos(a)).toFixed(3) + "," + (0.97 * Math.sin(a)).toFixed(3));
    }
    HEX = hex.join(" ");
    const cells = document.createElementNS(svgNS, "g");
    cellEls = [];
    for (const c of SN.CELLS) {
      const g = document.createElementNS(svgNS, "g");
      g.setAttribute("transform", `translate(${cx(c).toFixed(3)},${cy(c).toFixed(3)})`);
      g.dataset.c = c;
      const p = document.createElementNS(svgNS, "polygon");
      p.setAttribute("points", hex.join(" "));
      const t = document.createElementNS(svgNS, "text");
      t.textContent = c;
      g.append(p, t);
      cells.append(g);
      cellEls[c] = g;
    }
    const over = document.createElementNS(svgNS, "g");
    over.id = "overlay";
    board.replaceChildren(cells, over);
  }

  function resultsHere() {
    const r = cache.get(posText());
    return r && r.results ? r : null;
  }
  const VAL = { 1: "win", 0: "draw", "-1": "loss" };
  const vclass = r => (r.status === "solved" ? "v-" + VAL[r.value] : "v-unk");

  function renderBoard() {
    const p = pos();
    const sel = drag ? drag.cells : [];
    const selKey = sel.slice().sort((a, b) => a - b).join(",");
    const complete = drag && legal.some(pl => pl.cells.join(",") === selKey);
    const nexts = new Set();
    if (drag && sel.length) {
      for (const pl of legal) {
        if (!sel.every(c => pl.cells.includes(c))) continue;
        for (const c of pl.cells) if (!sel.includes(c) && sel.some(s => SN.adjacent(s, c))) nexts.add(c);
      }
    }
    const here = resultsHere();
    const best = here && here.best && here.value >= 0 ? SN.BY_KEY.get(here.best) : null;
    const lastPl = st.cur > 0 ? st.line[st.cur - 1] : null;
    for (const c of SN.CELLS) {
      const g = cellEls[c];
      const v = p.board[c];
      const cls = ["cell"];
      if (v >= 0) cls.push("c" + v);
      else {
        if (!playable.has(c)) cls.push("dead");
        if (sel.includes(c)) { cls.push("sel"); if (complete) cls.push("ok", "s" + p.stm); }
        else if (nexts.has(c)) cls.push("next");
        if (hint && hint.includes(c)) cls.push("hint");
        if (best && best.cells.includes(c) && !drag) cls.push("best");
      }
      g.setAttribute("class", cls.join(" "));
    }
    // snakes, the last move, result marks
    const over = $("overlay");
    const parts = [];
    for (let side = 0; side < 2; side++) {
      const snakes = SN.snakes(p, side);
      const top = Math.max(0, ...snakes.map(s => s.length));
      for (const s of snakes) {
        const long = s.length === top && top > 1 ? " long" : "";
        for (const a of s) for (const b of SN.NB[a]) {
          if (b > a && p.board[b] === side)
            parts.push(`<line class="link l${side}${long}" x1="${cx(a)}" y1="${cy(a)}" x2="${cx(b)}" y2="${cy(b)}"/>`);
        }
      }
    }
    if (lastPl) for (const c of lastPl.cells) parts.push(`<circle class="lastmark" cx="${cx(c)}" cy="${cy(c)}" r="0.62"/>`);
    if (here && !drag) {
      const rank = { "v-win": 3, "v-draw": 2, "v-unk": 1, "v-loss": 0 };
      const bestAt = new Map();
      for (const [mv, r] of here.results) {
        const pl = SN.BY_KEY.get(mv);
        if (!pl) continue;
        const k = vclass(r);
        for (const c of pl.cells) if (!bestAt.has(c) || rank[k] > rank[bestAt.get(c)]) bestAt.set(c, k);
      }
      for (const [c, k] of bestAt) parts.push(`<circle class="mark ${k}" cx="${cx(c)}" cy="${cy(c) + 0.5}" r="0.13"/>`);
    }
    // the tries in work at the reply in work: the more complete, the stronger the outline
    const tw = triesHere();
    if (tw && st.cur === tw.ctx.k + 1 && !drag) {
      const at = new Map();
      for (const t of tw.tries.list) {
        if (t.s !== "w") continue;
        const fr = t.t ? t.d / t.t : 0;
        for (const c of tryCells(t)) if (!at.has(c) || fr > at.get(c)) at.set(c, fr);
      }
      for (const [c, fr] of at)
        parts.push(`<polygon class="trymark" transform="translate(${cx(c).toFixed(3)},${cy(c).toFixed(3)}) scale(0.86)" points="${HEX}" style="stroke-opacity:${(0.25 + 0.75 * fr).toFixed(2)}"/>`);
    }
    over.innerHTML = parts.join("");
  }

  function cellAt(e) {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    const g = el && el.closest && el.closest("[data-c]");
    return g ? +g.dataset.c : null;
  }
  function nope() { board.classList.remove("nope"); void board.getBoundingClientRect(); board.classList.add("nope"); }
  board.addEventListener("pointerdown", e => {
    if (e.button !== 0) return;
    const c = cellAt(e);
    if (c === null || pos().board[c] !== -1) return;
    if (!playable.has(c)) { nope(); return; }
    drag = { cells: [c] };
    board.setPointerCapture(e.pointerId);
    renderBoard(); renderTray();
    e.preventDefault();
  });
  board.addEventListener("pointermove", e => {
    const c = cellAt(e);
    if (!drag) {
      for (const g of cellEls) if (g) g.classList.remove("hover");
      if (c !== null && playable.has(c)) cellEls[c].classList.add("hover");
      return;
    }
    if (c === null) return;
    const sel = drag.cells;
    if (sel.length >= 2 && c === sel[sel.length - 2]) { sel.pop(); renderBoard(); renderTray(); return; }
    if (sel.includes(c) || pos().board[c] !== -1 || sel.length >= 4) return;
    if (!sel.some(s => SN.adjacent(s, c))) return;
    const want = sel.concat([c]);
    if (!legal.some(pl => want.every(x => pl.cells.includes(x)))) return;
    sel.push(c);
    renderBoard(); renderTray();
  });
  function endDrag(commit) {
    if (!drag) return;
    const key = drag.cells.slice().sort((a, b) => a - b).join(",");
    drag = null;
    const pl = commit ? legal.find(x => x.cells.join(",") === key) : null;
    if (pl) playMove(pl);
    else { if (commit) nope(); renderBoard(); renderTray(); }
  }
  board.addEventListener("pointerup", () => endDrag(true));
  board.addEventListener("pointercancel", () => endDrag(false));
  board.addEventListener("pointerleave", () => { for (const g of cellEls) if (g) g.classList.remove("hover"); });
  board.addEventListener("contextmenu", e => { if (drag) { e.preventDefault(); endDrag(false); } });

  // ---------------------------------------------------------------- side panel
  function pieceSvg(k, color, cls) {
    const cells = SN.SHAPE[k].map(([q, r]) => [S3 * (q + r / 2), 1.5 * r]);
    const xs = cells.map(c => c[0]), ys = cells.map(c => c[1]);
    const x0 = Math.min(...xs) - 1, y0 = Math.min(...ys) - 1;
    const w = Math.max(...xs) - x0 + 1, h = Math.max(...ys) - y0 + 1;
    const hex = [];
    for (let i = 0; i < 6; i++) {
      const a = Math.PI / 180 * (60 * i - 30);
      hex.push((0.95 * Math.cos(a)).toFixed(2) + "," + (0.95 * Math.sin(a)).toFixed(2));
    }
    const polys = cells.map(([x, y]) => `<polygon transform="translate(${x.toFixed(2)},${y.toFixed(2)})" points="${hex.join(" ")}" fill="${color}"/>`).join("");
    const scale = 4.6;
    return `<svg class="${cls}" viewBox="${x0} ${y0} ${w} ${h}" width="${(w * scale).toFixed(1)}" height="${(h * scale).toFixed(1)}"><title>${SN.PIECE_NAME[k]}</title>${polys}</svg>`;
  }
  function renderTray() {
    const p = pos();
    let matchPiece = -1;
    if (drag) {
      const key = drag.cells.slice().sort((a, b) => a - b).join(",");
      const pl = legal.find(x => x.cells.join(",") === key);
      if (pl) matchPiece = pl.piece;
    }
    for (let side = 0; side < 2; side++) {
      const row = $("p" + side);
      row.classList.toggle("tomove", p.stm === side && !SN.isOver(p));
      row.querySelector(".len").textContent = SN.longest(p, side);
      const color = side === 0 ? "var(--red)" : "var(--blue)";
      let html = "";
      for (let k = side === 0 ? 0 : 1; k < 8; k++) {
        const has = (p.left[side] >> k) & 1;
        html += pieceSvg(k, color, (has ? "" : "used") + (side === p.stm && k === matchPiece ? " match" : ""));
      }
      row.querySelector(".tray").innerHTML = html;
    }
  }
  function renderStatus() {
    const p = pos();
    const s = $("status");
    if (SN.isOver(p)) {
      const a = SN.longest(p, 0), b = SN.longest(p, 1);
      s.innerHTML = `Game over: ${a === b ? "draw" : (a > b ? "Red" : "Blue") + " wins"} <span class="ply">${a} : ${b} · ply ${p.ply}</span>`;
    } else {
      const mustPass = legal.length === 0;
      s.innerHTML = `<span class="dot ${p.stm ? "blue" : "red"}"></span>${SIDE[p.stm]} to ${mustPass ? "pass" : "move"} <span class="ply">ply ${p.ply}${p.passes ? " · passes " + p.passes : ""} · ${legal.length} moves</span>`;
    }
    const mustPass = !SN.isOver(p) && legal.length === 0;
    $("bPass").disabled = !mustPass;
    $("bPass").classList.toggle("hot", mustPass);
    $("bUndo").disabled = $("bStart").disabled = st.cur === 0;
    $("bRedo").disabled = $("bEnd").disabled = st.cur === st.line.length;
    $("postext").textContent = posText();
  }
  function renderMoves() {
    const ol = $("moves");
    const items = [`<li data-k="0" class="${st.cur === 0 ? "cur" : ""}">${st.baseText ? "loaded" : "start"}</li>`];
    st.line.forEach((m, i) => {
      const mover = st.positions[i].stm;
      const cls = ["m" + mover];
      if (i + 1 === st.cur) cls.push("cur");
      if (i + 1 > st.cur) cls.push("future");
      items.push(`<li data-k="${i + 1}" class="${cls.join(" ")}"><span class="n">${st.positions[i].ply + 1}</span>${SN.moveName(m)}</li>`);
    });
    ol.innerHTML = items.join("");
    const cur = ol.querySelector(".cur");
    if (cur) cur.scrollIntoView({ block: "nearest" });
  }
  $("moves").addEventListener("click", e => {
    const li = e.target.closest("li");
    if (li) go(+li.dataset.k);
  });

  const p2 = x => String(Math.floor(x)).padStart(2, "0");
  const fmtT = s => (s >= 3600 ? Math.floor(s / 3600) + ":" + p2(s % 3600 / 60) + ":" + p2(s % 60) :
    s >= 60 ? Math.floor(s / 60) + ":" + p2(s % 60) : s.toFixed(s < 10 ? 1 : 0) + " s");
  const fmtN = n => (n >= 1e9 ? (n / 1e9).toFixed(2) + "G" : n >= 1e6 ? (n / 1e6).toFixed(1) + "M" : n >= 1e3 ? (n / 1e3).toFixed(0) + "k" : String(n));

  function renderRun() {
    const busy = run && (run.status === "waiting" || run.status === "running" || run.status === "starting");
    const over = SN.isOver(pos());
    if (run && !busy) cancelling = false;
    $("bSolve").disabled = $("bAll").disabled = busy || cancelling || over || !info.solver_found;
    $("bProve").disabled = busy || cancelling || over || !info.solver_found;
    const waiting = busy && run.status !== "running";
    let rh = "";
    if (waiting) rh = `<span class="wait">${run.ahead ? "waiting for the run lock" : "waiting for the run in progress"}</span> ${run.ahead ? run.ahead + " ahead · " : ""}${fmtT(run.waited)}`;
    else if (busy && run.mode === "prove") rh = `prove: ${fmtT(run.elapsed)}${run.held_lock ? "" : " · no lock"}`;
    else if (busy) rh = `${run.mode === "solve" ? "solve" : "all moves"}: ${run.done}/${run.total} moves · ${fmtN(run.nodes)} nodes · ${fmtT(run.elapsed)} · ${run.running} running${run.held_lock ? "" : " · no lock"}`;
    setHtml($("runtext"), rh);
    $("runstate").hidden = !busy;
    $("bNow").hidden = !waiting;
    renderVerdict();
  }
  function renderVerdict() {
    const v = $("verdict");
    const p = pos();
    const here = cache.get(posText());
    const live = run && run.position === posText() && (run.status === "running" || run.status === "waiting");
    let html = "";
    if (SN.isOver(p)) html = "";
    else if (here && here.value !== null && here.value !== undefined) {
      const who = here.value === 1 ? SIDE[p.stm] + " wins" : here.value === 0 ? "Draw" : SIDE[p.stm ^ 1] + " wins";
      html = `<span class="v-${VAL[here.value]}">${who}</span>`;
      if (here.best && here.value >= 0) html += `<span>with</span><button id="bBest">${here.best}</button>`;
      else if (here.value === 1 && !live) html += `<span class="v-unk">winning move not found within the limit</span>`;
    } else if (here && !live && here.status) {
      html = here.status === "cancelled" ? `<span class="v-unk">cancelled</span>` :
        here.status === "error" ? `<span class="v-loss">error: ${esc(here.message || "")}</span>` :
        `<span class="v-unk">not solved${here.seconds ? ` within ${here.seconds} s` : ""}</span>`;
    }
    setHtml(v, html);
    if ($("bBest")) {
      $("bBest").onclick = () => { const pl = SN.BY_KEY.get(here.best) || (here.best === "pass" ? null : undefined); if (pl !== undefined) playMove(pl); };
      $("bBest").onmouseenter = () => { const pl = SN.BY_KEY.get(here.best); hint = pl ? pl.cells : null; renderBoard(); };
      $("bBest").onmouseleave = () => { hint = null; renderBoard(); };
    }
    renderResults();
  }
  // ---------------------------------------------------------------- prove: the replies of a position, refuted one by one
  // The proof shown: the nearest position at or before the current one on the line that has a proof store,
  // unless the current position has results of its own (a solve or all moves).
  function proveView() {
    const own = cache.get(posText());
    if (own && own.mode !== "proof") return null;
    for (let k = st.cur; k >= 0; k--) {
      const t = SN.text(st.positions[k]);
      const pr = proofs.get(t);
      if (pr) return { k, text: t, pr };
    }
    return null;
  }
  const PSTATE = { refuted: ["refuted", "v-win"], work: ["in work", "v-run"], open: ["open", "v-unk"], held: ["holds", "v-held"] };
  function proveLive(text) { return run && run.mode === "prove" && run.position === text && run.status === "running"; }
  // seconds that went by since the last state event (they come at most once a second, a beat every 5 s)
  const since = () => (run && run._at ? (Date.now() - run._at) / 1000 : 0);
  function curSeconds(ctx) { return proveLive(ctx.text) && ctx.pr.cur_seconds != null ? ctx.pr.cur_seconds + since() : null; }
  function renderProveHead(ctx) {
    const pr = ctx.pr, live = proveLive(ctx.text);
    const bits = [`refuted ${pr.refuted} of ${pr.n}`];
    if (live) bits.push(fmtT(run.elapsed + since()));
    if (pr.rate && pr.open) {
      bits.push(fmtT(pr.rate) + " a reply");
      const cur = curSeconds(ctx) || 0;
      bits.push(fmtT(Math.max(pr.rate * (pr.open - 1), pr.rate * pr.open - cur)) + " left");
    }
    let html = esc(bits.join(" · "));
    const p = st.positions[ctx.k];
    if (pr.held && pr.held.length) html += ` · <span class="held">holds: ${esc(pr.rows[pr.held[0]].move)}</span>`;
    else if (pr.n && pr.refuted === pr.n) html += ` · <span class="v-win">${SIDE[p.stm ^ 1]} wins</span>`;
    setHtml($("provetext"), html);
  }
  function renderProve(ctx) {
    const pr = ctx.pr, ol = $("results");
    const rank = r => (r.state === "work" ? 0 : r.state === "open" ? 2 : 1);
    const rows = pr.rows.slice().sort((a, b) => rank(a) - rank(b) ||
      (rank(a) === 1 ? (b.seq ?? -1) - (a.seq ?? -1) : a.i - b.i));
    const shown = st.cur > ctx.k ? SN.text(st.positions[ctx.k + 1]) : null;
    const best = pr.tries && pr.tries.root === pr.cur ? pr.tries.best : null;
    // the first row in view keeps its place when the list is rebuilt (a reply that finishes moves rows down)
    let anchor = null;
    if (ol.scrollTop > 0) {
      for (const li of ol.children) if (li.offsetTop + li.offsetHeight > ol.scrollTop) { anchor = [li.dataset.i, li.offsetTop - ol.scrollTop]; break; }
    }
    const changed = setHtml(ol, rows.map(r => {
      const [label, vc] = PSTATE[r.state] || [r.state, "v-unk"];
      const cls = ["pr", r.state === "work" ? "work" : "", r.child === shown ? "here" : ""].join(" ");
      const bar = r.state === "work" && best != null ? `<span class="bar"><i style="width:${(100 * best).toFixed(1)}%"></i></span>` : "";
      return `<li class="${cls}" data-i="${r.i}" data-mv="${esc(r.move)}"><span class="v ${vc}">${label}</span>` +
        `<span class="mv">${esc(r.move)}</span><span class="ans">${esc(r.answer || "")}</span>` +
        `<span class="n">${r.tries || r.state !== "open" ? r.tries + "/" + r.failed : ""}</span>` +
        `<span class="t">${r.seconds ? fmtT(r.seconds) : ""}</span>${bar}</li>`;
    }).join(""));
    if (changed && anchor) {
      const li = ol.querySelector(`li[data-i="${anchor[0]}"]`);
      if (li) ol.scrollTop = li.offsetTop - anchor[1];
    }
    // the reply in work: its seconds tick in place, so that the list is not rebuilt (and a click lost) each second
    const w = ol.querySelector("li.work .t"), cs = curSeconds(ctx);
    if (w && cs) w.textContent = fmtT(cs);
    renderTries(ctx);
  }
  // the tries of the reply in work, when that reply is on the board
  function triesHere() {
    const ctx = proveView();
    if (!ctx || !ctx.pr.tries || ctx.pr.cur == null || ctx.pr.tries.root !== ctx.pr.cur || st.cur < ctx.k + 1) return null;
    const row = ctx.pr.rows[ctx.pr.cur];
    if (!row || SN.text(st.positions[ctx.k + 1]) !== row.child) return null;
    return { ctx, row, tries: ctx.pr.tries };
  }
  const CAP = 12;
  // a try as the server sends it: m move, s state (w in work, f failed, x won), d replies shown lost, t replies, r refuting reply
  function tryCells(t) { const pl = SN.BY_KEY.get(t.m); return pl ? pl.cells : []; }
  function renderTries() {
    const el = $("tries"), tw = triesHere();
    if (!tw) { el.hidden = true; setHtml(el, ""); return; }
    const fr = t => (t.t ? t.d / t.t : 0);
    const work = tw.tries.list.filter(t => t.s === "w").sort((a, b) => fr(b) - fr(a) || b.d - a.d);
    const failed = tw.tries.list.filter(t => t.s === "f").reverse();
    const won = tw.tries.list.filter(t => t.s === "x");
    const all = won.concat(work, failed);
    const lines = all.slice(0, CAP).map(t => {
      if (t.s === "f")
        return `<li class="failed" data-m="${esc(t.m)}"><span class="mv">${esc(t.m)}</span><span class="rep">${esc(t.r || "")}</span></li>`;
      return `<li class="${t.s === "x" ? "won" : "work"}" data-m="${esc(t.m)}"><span class="mv">${esc(t.m)}</span>` +
        `<span class="bar"><i style="width:${(100 * fr(t)).toFixed(1)}%"></i></span><span class="n">${t.d}/${t.t ?? "?"}</span></li>`;
    });
    const total = tw.tries.work + tw.tries.failed + tw.tries.won;
    if (total > CAP) lines.push(`<li class="more">+${total - CAP}</li>`);
    el.hidden = false;
    setHtml(el, lines.join(""));
  }
  function tryAt(li) {
    const tw = triesHere();
    return tw && li && li.dataset.m ? { tw, t: tw.tries.list.find(t => t.m === li.dataset.m) } : null;
  }
  $("tries").addEventListener("mouseover", e => {
    const x = tryAt(e.target.closest("li"));
    const cells = x && x.t ? tryCells(x.t) : null;
    if (String(cells) !== String(hint)) { hint = cells; renderBoard(); }
  });
  $("tries").addEventListener("mouseleave", () => { hint = null; renderBoard(); });
  // a try: the reply and the try as moves (a failed one also with the reply that refuted it)
  $("tries").addEventListener("click", e => {
    const x = tryAt(e.target.closest("li"));
    if (!x || !x.t) return;
    const get = m => (m === "pass" ? null : SN.BY_KEY.get(m));
    const moves = [x.tw.row.move, x.t.m].concat(x.t.s === "f" && x.t.r ? [x.t.r] : []).map(get);
    if (moves.some(m => m === undefined)) return;
    setLine(st.base, st.baseText, st.line.slice(0, x.tw.ctx.k).concat(moves));
  });
  setInterval(() => {
    const ctx = proveView();
    if (ctx && proveLive(ctx.text)) {
      renderProveHead(ctx);
      const w = $("results").querySelector("li.work .t"), cs = curSeconds(ctx);
      if (w && cs) w.textContent = fmtT(cs);
    }
  }, 1000);
  // the reply on the board (after P), with its answer as the next move when it is known
  function showReply(ctx, r, withAnswer) {
    const pl = r.move === "pass" ? null : SN.BY_KEY.get(r.move);
    if (pl === undefined) return;
    const line = st.line.slice(0, ctx.k).concat([pl]);
    if (withAnswer && r.answer && r.state === "refuted") {
      const a = r.answer === "pass" ? null : SN.BY_KEY.get(r.answer);
      if (a !== undefined) {
        line.push(a);
        // proved: the reply's mover-to-be wins with the answer
        if (!cache.has(r.child)) cache.set(r.child, { mode: "proof", results: new Map(), value: 1, best: r.answer, status: "done" });
      }
    }
    setLine(st.base, st.baseText, line, ctx.k + 1);
  }
  function follow() {
    if (!$("iFollow").checked || !run || run.mode !== "prove" || !proveLive(run.position)) return;
    const ctx = proveView();
    if (!ctx || ctx.text !== run.position) return;
    const c = ctx.pr.cur;
    if (c === null || c === undefined || c === lastFollow) return;
    lastFollow = c;
    showReply(ctx, ctx.pr.rows[c], false);
  }
  $("iFollow").addEventListener("change", () => { lastFollow = null; follow(); });

  function renderResults() {
    const ctx = proveView();
    $("provehead").hidden = !ctx;
    if (ctx) { renderProveHead(ctx); renderProve(ctx); return; }
    renderTries();
    const here = resultsHere();
    const ol = $("results");
    if (!here) { setHtml(ol, ""); return; }
    const rows = [...here.results.entries()].map(([mv, r]) => ({ mv, r }));
    const score = r => (r.status === "solved" ? r.value : -0.5);
    rows.sort((a, b) => score(b.r) - score(a.r) || (a.r.seconds || 0) - (b.r.seconds || 0));
    setHtml(ol, rows.map(({ mv, r }) => {
      const label = r.status === "solved" ? VAL[r.value] : r.status === "limit" ? "not solved" : r.status;
      const t = r.seconds !== null && r.seconds !== undefined ? r.seconds.toFixed(2) + " s" : "";
      return `<li data-mv="${mv}"><span class="v ${vclass(r)}">${label}</span><span class="mv">${mv}</span><span class="t">${t}</span></li>`;
    }).join(""));
  }
  $("results").addEventListener("click", e => {
    const li = e.target.closest("li");
    if (!li) return;
    if (li.dataset.i !== undefined) {
      const ctx = proveView();
      if (ctx && ctx.pr.rows[+li.dataset.i]) showReply(ctx, ctx.pr.rows[+li.dataset.i], true);
      return;
    }
    const mv = li.dataset.mv;
    if (mv === "pass") playMove(null);
    else if (SN.BY_KEY.get(mv)) playMove(SN.BY_KEY.get(mv));
  });
  $("results").addEventListener("mouseover", e => {
    const li = e.target.closest("li");
    const pl = li && SN.BY_KEY.get(li.dataset.mv);
    const cells = pl ? pl.cells : null;
    if (String(cells) !== String(hint)) { hint = cells; renderBoard(); }
  });
  $("results").addEventListener("mouseleave", () => { hint = null; renderBoard(); });

  function renderTypical() {
    const t = info.typical || {};
    const p = pos();
    const a = t[p.ply], b = t[p.ply + 1];
    const bits = [];
    if (a) bits.push(`solve here: ${a}`);
    if (b) bits.push(`each move: ${b}`);
    $("typical").textContent = SN.isOver(p) ? "" : bits.join(" · ");
  }
  // replaces an element's content only when it changed, so a click in progress is not lost
  function setHtml(el, html) { if (el._html !== html) { el.innerHTML = html; el._html = html; return true; } return false; }
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  function render() {
    renderBoard(); renderTray(); renderStatus(); renderMoves(); renderRun(); renderTypical();
  }

  // ---------------------------------------------------------------- controls
  $("bStart").onclick = () => go(0);
  $("bUndo").onclick = () => go(st.cur - 1);
  $("bRedo").onclick = () => go(st.cur + 1);
  $("bEnd").onclick = () => go(st.line.length);
  $("bPass").onclick = () => { if (!SN.isOver(pos()) && legal.length === 0) playMove(null); };
  $("bReset").onclick = () => setLine(SN.initial(), null, [], 0);
  document.addEventListener("keydown", e => {
    if (e.target.closest && e.target.closest("textarea, input")) return;
    if (e.key === "ArrowLeft") go(st.cur - 1);
    else if (e.key === "ArrowRight") go(st.cur + 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(st.line.length);
    else if (e.key === "Escape") endDrag(false);
    else return;
    e.preventDefault();
  });

  function say(text, ok) { $("msg").textContent = text; $("msg").className = ok ? "ok" : ""; }
  async function post(url, body) {
    const r = await fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
    let j = {};
    try { j = await r.json(); } catch (e) { j = { ok: false, error: "HTTP " + r.status }; }
    return j;
  }
  $("bLoad").onclick = async () => {
    const text = $("io").value.trim();
    let got;
    try { got = SN.parseInput(text); }
    catch (e) { say(e.message); return; }
    setLine(got.base, got.baseText, got.moves);
    try {
      const j = await post("/api/load", { text });
      if (!j.ok) say("the solver refuses it: " + j.error);
      else if (j.position !== posText()) say("the page and the solver read this differently: " + j.position);
    } catch (e) { say("no server"); }
  };
  async function copy(text, what) {
    try { await navigator.clipboard.writeText(text); say(what + " copied", true); }
    catch (e) { $("io").value = text; $("io").select(); say("copy blocked; it is in the box", true); }
  }
  $("bCopyPos").onclick = () => copy(posText(), "position");
  $("bCopyMoves").onclick = () => {
    const moves = st.line.slice(0, st.cur).map(SN.moveName).join(" ");
    copy(st.baseText ? (st.baseText + " " + moves).trim() : moves, "moves");
  };

  async function startRun(mode) {
    const rawSec = $("iSec").value.trim();
    const seconds = rawSec === "" || isNaN(+rawSec) ? 30 : Math.max(0, +rawSec); // 0 = no limit
    const procs = Math.max(1, Math.min(64, Math.floor(+$("iProcs").value || 1)));
    const text = posText();
    const j = await post("/api/run", { mode, position: text, seconds, procs, lock: $("iLock").checked, client });
    if (!j.ok) { say(j.error); return; }
    if (mode === "prove") { if (cache.has(text)) cache.delete(text); lastFollow = null; }
    else entryFor(j.run, text, mode, seconds);
    render();
  }
  $("bSolve").onclick = () => startRun("solve");
  $("bProve").onclick = () => startRun("prove");
  $("bNow").onclick = () => run && post("/api/runnow", { run: run.id });
  $("bCancel").onclick = () => { if (!run) return; cancelling = true; renderRun(); post("/api/cancel", { run: run.id }); };
  $("bAll").onclick = () => startRun("all");

  // ---------------------------------------------------------------- server events
  function entryFor(id, text, mode, seconds) {
    const c = cache.get(text);
    if (c && c.run === id) return c;
    const e = { mode, results: new Map(), value: null, best: null, status: "running", seconds, run: id };
    cache.set(text, e);
    return e;
  }
  let jumped = false;
  function onState(r) {
      // a state without its rows: they have not changed since the ones the page has (rows_v)
      if (r.prove && r.prove.rows == null) {
        const old = proofs.get(r.position);
        if (old && old.rows) r.prove.rows = old.rows;
        else { r.prove = null; } // no rows to keep: the next poll or event brings them
      }
      run = r;
      run._at = Date.now();
      if (run.mode === "prove") {
        if (run.prove && run.prove.n) proofs.set(run.position, Object.assign({ exists: true }, run.prove));
        // a page opened while a proof runs starts on the proof's position
        if (!jumped && run.status === "running" && st.line.length === 0 && !st.baseText) {
          try { const got = SN.parseInput(run.position); setLine(got.base, got.baseText, got.moves); } catch (e) {}
        }
        jumped = true;
        if (run.message && run.status === "error") say(run.message);
        follow();
        renderRun();
        renderBoard();
        return;
      }
      const c = entryFor(run.id, run.position, run.mode, run.seconds);
      {
        if (run.value !== null) c.value = run.value;
        if (run.best) c.best = run.best;
        if (["done", "cancelled", "error"].includes(run.status)) { c.status = run.status; c.message = run.message; }
      }
      if (run.message && ["error"].includes(run.status)) say(run.message);
      renderRun();
      if (c && run.position === posText()) renderBoard();
  }
  function onResult(r) {
      const c = entryFor(r.run, r.position, null, null);
      if (r.move !== null) c.results.set(r.move, r);
      if (r.position === posText()) { renderBoard(); renderVerdict(); }
  }
  let lastStream = 0;
  function connect() {
    const es = new EventSource("/api/events?client=" + client);
    es.addEventListener("state", e => { lastStream = Date.now(); onState(JSON.parse(e.data)); });
    es.addEventListener("result", e => { lastStream = Date.now(); onResult(JSON.parse(e.data)); });
    // A tunnel can hold the event stream back. While the stream is silent, ask for the state instead.
    setInterval(async () => {
      if (document.hidden || Date.now() - lastStream < 10000) return;
      try {
        const pr = run && run.mode === "prove" ? proofs.get(run.position) : null;
        const j = await (await fetch("/api/status" + (pr && pr.rows_v ? "?rows=" + pr.rows_v : ""))).json();
        if (!j.run || (run && run.id === j.run.id && JSON.stringify(run) === JSON.stringify(j.run))) return;
        for (const r of j.run.results || []) onResult(Object.assign({ run: j.run.id, position: j.run.position }, r));
        onState(j.run);
      } catch (e) {}
    }, 5000);
  }

  $("iProcs").addEventListener("input", () => { $("iProcs").dataset.touched = "1"; });
  async function init() {
    buildBoard();
    setLine(SN.initial(), null, [], 0);
    connect();
    try {
      info = await (await fetch("/api/info")).json();
      if (!$("iProcs").dataset.touched) $("iProcs").value = info.cpus;
      if (!info.solver_found) say("no solver at " + info.solver);
      else if (!info.prover_found) say("no proof driver at " + info.prover);
      if (!info.lock) $("iLock").checked = false;
    } catch (e) { say("no server"); }
    renderRun(); renderTypical();
  }
  init();
})();
