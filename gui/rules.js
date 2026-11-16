// Snekagon rules for the analysis board, written to agree with src/pos.rs and src/pieces.rs.
// tools/gui_check.py compares this file with the solver (perft and children of random positions).
"use strict";
const SN = (() => {
  const R = 5, W = 11, GRID = 121;
  const PIECE_NAME = ["1", "2", "3I", "3V", "4I", "4A", "4B", "4C"];
  const PIECE_SIZE = [1, 2, 3, 3, 4, 4, 4, 4];
  const SHAPE = [
    [[0, 0]],
    [[0, 0], [1, 0]],
    [[0, 0], [0, 1], [0, 2]],
    [[0, 0], [1, 0], [1, 1]],
    [[0, 0], [-1, 1], [-2, 2], [-3, 3]],
    [[0, 0], [1, 0], [1, 1], [1, 2]],
    [[0, 0], [0, 1], [1, 1], [1, 2]],
    [[0, 0], [-1, 1], [-1, 2], [0, 2]],
  ];
  const DQ = [1, 1, 0, -1, -1, 0], DR = [0, -1, -1, 0, 1, 1];
  const cq = i => (i % W) - R;
  const cr = i => Math.floor(i / W) - R;
  const onBoard = (q, r) => Math.abs(q) <= R && Math.abs(r) <= R && Math.abs(q + r) <= R;
  const idx = (q, r) => (r + R) * W + (q + R);

  const CELLS = [];
  const ON = new Uint8Array(GRID);
  for (let i = 0; i < GRID; i++) if (onBoard(cq(i), cr(i))) { CELLS.push(i); ON[i] = 1; }
  const NB = [];
  for (let i = 0; i < GRID; i++) {
    const n = [];
    if (ON[i]) for (let d = 0; d < 6; d++) {
      const q = cq(i) + DQ[d], r = cr(i) + DR[d];
      if (onBoard(q, r)) n.push(idx(q, r));
    }
    NB.push(n);
  }
  const ADJ = new Uint8Array(GRID * GRID);
  for (const i of CELLS) for (const j of NB[i]) ADJ[i * GRID + j] = 1;
  const adjacent = (a, b) => ADJ[a * GRID + b] === 1;

  function sym(t, q, r) {
    if (t >= 6) [q, r] = [r, q];
    for (let k = 0; k < t % 6; k++) [q, r] = [-r, q + r];
    return [q, r];
  }
  // every placement, numbered as the solver numbers them: piece, orientation, anchor ascending
  const PLACEMENTS = [];
  const BY_KEY = new Map();
  for (let p = 0; p < 8; p++) {
    const orients = [];
    for (let t = 0; t < 12; t++) {
      const c = SHAPE[p].map(([q, r]) => sym(t, q, r));
      c.sort((a, b) => a[1] - b[1] || a[0] - b[0]);
      const [aq, ar] = c[0];
      const n = c.map(([q, r]) => [q - aq, r - ar]);
      const key = JSON.stringify(n);
      if (!orients.some(o => o.key === key)) orients.push({ key, cells: n });
    }
    for (const o of orients) {
      for (const a of CELLS) {
        const cells = [];
        let fits = true;
        for (const [dq, dr] of o.cells) {
          const q = cq(a) + dq, r = cr(a) + dr;
          if (!onBoard(q, r)) { fits = false; break; }
          cells.push(idx(q, r));
        }
        if (!fits) continue;
        cells.sort((x, y) => x - y);
        const pl = { id: PLACEMENTS.length, piece: p, cells };
        pl.name = PIECE_NAME[p] + ":" + cells.join(",");
        PLACEMENTS.push(pl);
        BY_KEY.set(pl.name, pl);
      }
    }
  }
  // the cells set of a placement as a key independent of the piece (the shape fixes the piece)
  const BY_CELLS = new Map();
  for (const pl of PLACEMENTS) {
    const k = pl.cells.join(",");
    if (!BY_CELLS.has(k)) BY_CELLS.set(k, []);
    BY_CELLS.get(k).push(pl);
  }

  function initial() {
    return { board: new Int8Array(GRID).fill(-1), left: [0xFF, 0xFE], stm: 0, ply: 0, passes: 0 };
  }
  function copy(p) {
    return { board: p.board.slice(), left: p.left.slice(), stm: p.stm, ply: p.ply, passes: p.passes };
  }
  const isOver = p => p.passes >= 2 || (p.left[0] | p.left[1]) === 0;
  const piecesNow = p => (p.ply === 0 ? p.left[p.stm] & 1 : p.left[p.stm]);

  // src/pos.rs legal_placement, for one placement
  function legal(p, pl, player) {
    const b = p.board;
    for (const c of pl.cells) if (b[c] !== -1) return false;
    const inPiece = c => pl.cells.includes(c);
    let touchOwn = false, touchAny = false;
    for (const c of pl.cells) for (const n of NB[c]) {
      if (inPiece(n)) continue;
      if (b[n] === player) touchOwn = true;
      if (b[n] !== -1) touchAny = true;
    }
    if (!touchOwn) return !touchAny;
    const mine = c => b[c] === player || inPiece(c);
    for (const c of pl.cells) {
      const around = NB[c].filter(mine);
      if (around.length > 2) return false;
      if (around.length === 2 && adjacent(around[0], around[1])) return false;
    }
    for (const c of pl.cells) for (const n of NB[c]) {
      if (b[n] === player && NB[n].filter(mine).length > 2) return false;
    }
    return true;
  }
  function moves(p) {
    if (isOver(p)) return [];
    const pieces = piecesNow(p);
    const out = [];
    for (const pl of PLACEMENTS) if ((pieces >> pl.piece) & 1 && legal(p, pl, p.stm)) out.push(pl);
    return out;
  }
  function play(p, pl) {
    const n = copy(p);
    if (pl === null) n.passes += 1;
    else {
      n.passes = 0;
      for (const c of pl.cells) n.board[c] = p.stm;
      n.left[p.stm] &= ~(1 << pl.piece);
    }
    n.stm ^= 1;
    n.ply += 1;
    return n;
  }
  function snakes(p, player) {
    const seen = new Uint8Array(GRID), out = [];
    for (const c of CELLS) {
      if (p.board[c] !== player || seen[c]) continue;
      const comp = [c];
      seen[c] = 1;
      for (let k = 0; k < comp.length; k++) for (const n of NB[comp[k]]) {
        if (p.board[n] === player && !seen[n]) { seen[n] = 1; comp.push(n); }
      }
      out.push(comp);
    }
    return out;
  }
  const longest = (p, player) => Math.max(0, ...snakes(p, player).map(s => s.length));
  function mask(p, player) {
    let m = 0n;
    for (const c of CELLS) if (p.board[c] === player) m |= 1n << BigInt(c);
    return m.toString(16).padStart(32, "0");
  }
  const text = p => [mask(p, 0), mask(p, 1), p.left[0], p.left[1], p.stm, p.ply, p.passes].join("_");

  // A position text, checked as tools/gui.py checks it; throws an Error with a message.
  function fromText(s) {
    const f = s.trim().split("_");
    if (f.length !== 7) throw new Error("a position text has 7 fields separated by _");
    const p = initial();
    for (let side = 0; side < 2; side++) {
      if (!/^[0-9a-fA-F]{1,32}$/.test(f[side])) throw new Error("cells must be up to 32 hex digits");
      const m = BigInt("0x" + f[side]);
      for (let i = 0; i < 128; i++) {
        if (((m >> BigInt(i)) & 1n) === 0n) continue;
        if (i >= GRID || !ON[i]) throw new Error("cell " + i + " is not on the board");
        if (p.board[i] !== -1) throw new Error("cell " + i + " is both red and blue");
        p.board[i] = side;
      }
    }
    const num = (x, lo, hi, what) => {
      if (!/^\d+$/.test(x) || +x < lo || +x > hi) throw new Error(what + " must be a number from " + lo + " to " + hi);
      return +x;
    };
    p.left = [num(f[2], 0, 255, "red pieces"), num(f[3], 0, 254, "blue pieces")];
    if (p.left[1] & 1) throw new Error("blue has no one-cell piece");
    p.stm = num(f[4], 0, 1, "side");
    p.ply = num(f[5], 0, 255, "ply");
    p.passes = num(f[6], 0, 2, "passes");
    for (let side = 0; side < 2; side++) {
      const full = side === 0 ? 0xFF : 0xFE;
      let used = 0;
      for (let k = 0; k < 8; k++) if ((full >> k) & 1 && !((p.left[side] >> k) & 1)) used += PIECE_SIZE[k];
      const have = CELLS.filter(c => p.board[c] === side).length;
      const who = side === 0 ? "red" : "blue";
      if (have !== used) throw new Error(who + " has " + have + " cells but has placed pieces of " + used + " cells");
      for (const c of CELLS) {
        if (p.board[c] !== side) continue;
        const a = NB[c].filter(n => p.board[n] === side);
        if (a.length > 2 || (a.length === 2 && adjacent(a[0], a[1])))
          throw new Error(who + "'s cells are not snakes (at cell " + c + ")");
      }
    }
    return p;
  }
  // A move list from a position; returns {pos, moves:[placement|null]} or throws.
  function replay(base, tokens) {
    let p = base;
    const line = [];
    tokens.forEach((tok, k) => {
      if (isOver(p)) throw new Error("move " + (k + 1) + " (" + tok + "): the game is over");
      const ms = moves(p);
      if (tok === "pass") {
        if (ms.length) throw new Error("move " + (k + 1) + ": pass, but a placement exists");
        p = play(p, null);
        line.push(null);
        return;
      }
      const m = /^([0-9A-Za-z]+):([\d,\s]+)$/.exec(tok);
      if (!m) throw new Error("move " + (k + 1) + " (" + tok + ") is not a move");
      const cells = m[2].split(",").map(x => +x).sort((a, b) => a - b);
      const pl = BY_KEY.get(m[1].toUpperCase() + ":" + cells.join(","));
      if (!pl) throw new Error("move " + (k + 1) + " (" + tok + ") is not a piece on those cells");
      if (!ms.includes(pl)) throw new Error("move " + (k + 1) + " (" + tok + ") is not legal here");
      p = play(p, pl);
      line.push(pl);
    });
    return { pos: p, moves: line };
  }
  // Text input: a position text, a move list from the start, or a position text followed by moves.
  function parseInput(s) {
    const toks = s.trim().split(/\s+/).filter(x => x);
    if (!toks.length) throw new Error("nothing to load");
    let base = initial(), baseText = null;
    if (toks[0].includes("_")) {
      base = fromText(toks[0]);
      baseText = text(base);
      toks.shift();
    }
    const r = replay(base, toks);
    return { base, baseText, moves: r.moves, pos: r.pos };
  }
  const moveName = pl => (pl === null ? "pass" : pl.name);
  function perft(p, d) {
    if (isOver(p)) return 1;
    const ms = moves(p);
    if (d === 1) return Math.max(1, ms.length);
    if (!ms.length) return perft(play(p, null), d - 1);
    let n = 0;
    for (const m of ms) n += perft(play(p, m), d - 1);
    return n;
  }

  return {
    R, W, GRID, CELLS, NB, ON, cq, cr, adjacent, PIECE_NAME, PIECE_SIZE, SHAPE, PLACEMENTS, BY_KEY, BY_CELLS,
    initial, copy, isOver, piecesNow, legal, moves, play, snakes, longest, text, fromText, replay, parseInput,
    moveName, perft,
  };
})();
if (typeof module !== "undefined") module.exports = SN;
