// Paths as the pen draws them and the node tool shapes them (#83,
// docs/studio-interface.md, Flat and Selecting and shaping): points, each
// with the handles of the curves on either side, and whether the path is
// closed. Read from path data and written back as `M`, `L`, `C` and `Z`.
// Pure.

import type { Point } from './geometry';
import { clean } from './snap';

/** One point of a path: where it is, its handles (none for a straight
    side), and whether they turn together. */
export interface Knot {
  p: Point;
  /** The handle of the curve coming in, and of the one going out. */
  in: Point | null;
  out: Point | null;
  smooth: boolean;
}

export interface Path {
  knots: Knot[];
  closed: boolean;
}

const f = (v: number) => String(clean(v));
const pt = (p: Point) => `${f(p[0])},${f(p[1])}`;

/** A path's data: straight sides as `L`, curved ones as `C`. */
export function toD(path: Path): string {
  const k = path.knots;
  if (!k.length) return '';
  const parts = [`M${pt(k[0]!.p)}`];
  const side = (a: Knot, b: Knot) => (a.out || b.in ? `C${pt(a.out ?? a.p)} ${pt(b.in ?? b.p)} ${pt(b.p)}` : `L${pt(b.p)}`);
  for (let i = 1; i < k.length; i++) parts.push(side(k[i - 1]!, k[i]!));
  if (path.closed) {
    // a closing curve is written out; a straight one is the Z
    const last = k.at(-1)!;
    if (last.out || k[0]!.in) parts.push(side(last, k[0]!));
    parts.push('Z');
  }
  return parts.join(' ');
}

/** The path data's one path, if it is one subpath the node tool shapes. */
export function parse(d: string): Path | null {
  const all = parseAll(d);
  return all?.length === 1 ? all[0]! : null;
}

/** Data written from paths, a subpath each. */
export const toDAll = (paths: Path[]): string => paths.map(toD).join(' ');

/** The path data's subpaths, if each is one the node tool can shape:
    moves, lines and curves, a quadratic one as the cubic it is (a glyph's
    outline is often written so); none for arcs and shorthand curves. */
export function parseAll(d: string): Path[] | null {
  const tokens = d.match(/[a-zA-Z]|-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/g);
  if (!tokens) return null;
  const paths: Path[] = [];
  let knots: Knot[] = [];
  let cur: Point = [0, 0];
  let start: Point = [0, 0];
  let cmd = '';
  let i = 0;
  const num = () => {
    const t = tokens[i++];
    if (t === undefined || /[a-zA-Z]/.test(t)) throw new Error('number expected');
    return Number(t);
  };
  const end = (closed: boolean) => {
    if (knots.length) paths.push(tidy({ knots, closed }));
    knots = [];
  };
  try {
    while (i < tokens.length) {
      if (/[a-zA-Z]/.test(tokens[i]!)) cmd = tokens[i++]!;
      const rel = cmd === cmd.toLowerCase();
      const at = (x: number, y: number): Point => (rel ? [cur[0] + x, cur[1] + y] : [x, y]);
      const to = (p: Point) => {
        // a side drawn after a close starts again where it closed
        if (!knots.length) knots.push({ p: cur, in: null, out: null, smooth: false });
        cur = p;
        knots.push({ p, in: null, out: null, smooth: false });
      };
      switch (cmd.toUpperCase()) {
        case 'M':
          end(false);
          cur = at(num(), num());
          start = cur;
          knots.push({ p: cur, in: null, out: null, smooth: false });
          cmd = rel ? 'l' : 'L';
          break;
        case 'L':
          to(at(num(), num()));
          break;
        case 'H':
          to([rel ? cur[0] + num() : num(), cur[1]]);
          break;
        case 'V':
          to([cur[0], rel ? cur[1] + num() : num()]);
          break;
        case 'C': {
          const c1 = at(num(), num());
          const c2 = at(num(), num());
          to(at(num(), num()));
          knots.at(-2)!.out = c1;
          knots.at(-1)!.in = c2;
          break;
        }
        case 'Q': {
          // exactly: the cubic's handles two thirds of the way to the control
          const q = at(num(), num());
          const from = cur;
          const p = at(num(), num());
          to(p);
          knots.at(-2)!.out = [from[0] + ((q[0] - from[0]) * 2) / 3, from[1] + ((q[1] - from[1]) * 2) / 3];
          knots.at(-1)!.in = [p[0] + ((q[0] - p[0]) * 2) / 3, p[1] + ((q[1] - p[1]) * 2) / 3];
          break;
        }
        case 'Z':
          end(true);
          cur = start;
          break;
        default:
          return null;
      }
    }
  } catch {
    return null;
  }
  end(false);
  return paths.length ? paths : null;
}

/** A subpath as the node tool keeps it: a closing curve back to the first
    point is the first point's, a handle on its point is none, and a point
    whose handles are in line is smooth. */
function tidy(path: Path): Path {
  const knots = path.knots;
  if (path.closed && knots.length > 1) {
    const last = knots.at(-1)!;
    const first = knots[0]!;
    if (same(last.p, first.p)) {
      first.in = last.in;
      knots.pop();
    }
  }
  for (const k of knots) {
    if (k.in && same(k.in, k.p)) k.in = null;
    if (k.out && same(k.out, k.p)) k.out = null;
    k.smooth = turnsTogether(k);
  }
  return path;
}

const same = (a: Point, b: Point) => Math.abs(a[0] - b[0]) < 1e-6 && Math.abs(a[1] - b[1]) < 1e-6;

/** Handles in line on either side of their point. */
function turnsTogether(k: Knot): boolean {
  if (!k.in || !k.out) return false;
  const a = [k.in[0] - k.p[0], k.in[1] - k.p[1]];
  const b = [k.out[0] - k.p[0], k.out[1] - k.p[1]];
  const la = Math.hypot(a[0]!, a[1]!);
  const lb = Math.hypot(b[0]!, b[1]!);
  if (la < 1e-9 || lb < 1e-9) return false;
  return (a[0]! * b[0]! + a[1]! * b[1]!) / (la * lb) < -0.999;
}

const copy = (path: Path): Path => ({ closed: path.closed, knots: path.knots.map((k) => ({ ...k })) });
const add = (a: Point, b: Point): Point => [a[0] + b[0], a[1] + b[1]];
const sub = (a: Point, b: Point): Point => [a[0] - b[0], a[1] - b[1]];

/** Point `i` moved to `to`, its handles with it. */
export function movePoint(path: Path, i: number, to: Point): Path {
  const out = copy(path);
  const k = out.knots[i]!;
  const d = sub(to, k.p);
  out.knots[i] = { ...k, p: to, in: k.in && add(k.in, d), out: k.out && add(k.out, d) };
  return out;
}

/** Handle `which` of point `i` moved to `to`; a smooth point's other
    handle turns with it, keeping its length, unless `alt` breaks it. */
export function moveHandle(path: Path, i: number, which: 'in' | 'out', to: Point, alt: boolean): Path {
  const out = copy(path);
  const k = out.knots[i]!;
  const next: Knot = { ...k, [which]: to };
  const other = which === 'in' ? 'out' : 'in';
  if (k.smooth && !alt && k[other]) {
    const len = Math.hypot(k[other]![0] - k.p[0], k[other]![1] - k.p[1]);
    const dir = sub(k.p, to);
    const l = Math.hypot(dir[0], dir[1]) || 1;
    next[other] = [k.p[0] + (dir[0] / l) * len, k.p[1] + (dir[1] / l) * len];
  }
  if (alt) next.smooth = false;
  out.knots[i] = next;
  return out;
}

/** Point `i` switched between corner and smooth: a corner loses its
    handles; a smooth point gets handles along the line between its
    neighbors, a third of the way to each. */
export function toggle(path: Path, i: number): Path {
  const out = copy(path);
  const k = out.knots[i]!;
  if (k.smooth) {
    out.knots[i] = { ...k, in: null, out: null, smooth: false };
    return out;
  }
  const n = out.knots.length;
  const prev = out.knots[(i - 1 + n) % n]!.p;
  const next = out.knots[(i + 1) % n]!.p;
  const first = i === 0 && !path.closed;
  const last = i === n - 1 && !path.closed;
  const dir = sub(first ? k.p : next, last ? k.p : prev);
  const l = Math.hypot(dir[0], dir[1]) || 1;
  const u: Point = [dir[0] / l, dir[1] / l];
  const lin = Math.hypot(k.p[0] - prev[0], k.p[1] - prev[1]) / 3;
  const lout = Math.hypot(next[0] - k.p[0], next[1] - k.p[1]) / 3;
  out.knots[i] = { ...k, in: first ? null : [k.p[0] - u[0] * lin, k.p[1] - u[1] * lin], out: last ? null : [k.p[0] + u[0] * lout, k.p[1] + u[1] * lout], smooth: !first && !last };
  return out;
}

/** The fewest points a path keeps. */
export const fewest = (path: Path): number => (path.closed ? 3 : 2);

/** Point `i` removed, if the path keeps enough. */
export function removePoint(path: Path, i: number): Path | null {
  if (path.knots.length <= fewest(path)) return null;
  const out = copy(path);
  out.knots.splice(i, 1);
  return out;
}

/** The sides of a path: each from a point to the next (and, closed, from
    the last round to the first), as cubic curves. */
export function sides(path: Path): [Point, Point, Point, Point][] {
  const k = path.knots;
  const out: [Point, Point, Point, Point][] = [];
  const count = path.closed ? k.length : k.length - 1;
  for (let i = 0; i < count; i++) {
    const a = k[i]!;
    const b = k[(i + 1) % k.length]!;
    out.push([a.p, a.out ?? a.p, b.in ?? b.p, b.p]);
  }
  return out;
}

/** The point at `t` along a cubic curve. */
export function at(c: [Point, Point, Point, Point], t: number): Point {
  const u = 1 - t;
  const w = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
  return [w.reduce((s, wi, j) => s + wi * c[j]![0], 0), w.reduce((s, wi, j) => s + wi * c[j]![1], 0)];
}

/** The side nearest `p`, how far along it, and how far from `p`. */
export function nearestSide(path: Path, p: Point): { side: number; t: number; d: number } | null {
  let best: { side: number; t: number; d: number } | null = null;
  sides(path).forEach((c, side) => {
    for (let s = 0; s <= 64; s++) {
      const q = at(c, s / 64);
      const d = Math.hypot(q[0] - p[0], q[1] - p[1]);
      if (!best || d < best.d) best = { side, t: s / 64, d };
    }
  });
  return best;
}

/** A point added on side `side` at `t`, the curve split so it keeps its
    shape. */
export function insertPoint(path: Path, side: number, t: number): Path {
  const out = copy(path);
  const n = out.knots.length;
  const a = out.knots[side]!;
  const b = out.knots[(side + 1) % n]!;
  const lerp = (p: Point, q: Point): Point => [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
  if (!a.out && !b.in) {
    out.knots.splice(side + 1, 0, { p: lerp(a.p, b.p), in: null, out: null, smooth: false });
    return out;
  }
  // de Casteljau
  const [p0, p1, p2, p3] = [a.p, a.out ?? a.p, b.in ?? b.p, b.p];
  const q0 = lerp(p0, p1);
  const q1 = lerp(p1, p2);
  const q2 = lerp(p2, p3);
  const r0 = lerp(q0, q1);
  const r1 = lerp(q1, q2);
  const s = lerp(r0, r1);
  out.knots[side] = { ...a, out: q0 };
  out.knots[(side + 1) % n] = { ...b, in: q2 };
  out.knots.splice(side + 1, 0, { p: s, in: r0, out: r1, smooth: true });
  return out;
}

/** The whole path moved by `d`. */
export function translate(path: Path, d: Point): Path {
  return { closed: path.closed, knots: path.knots.map((k) => ({ ...k, p: add(k.p, d), in: k.in && add(k.in, d), out: k.out && add(k.out, d) })) };
}

/** The path drawn as lines, for drawing it while it is drawn. */
export function polyline(path: Path): Point[] {
  const out: Point[] = path.knots.length ? [path.knots[0]!.p] : [];
  for (const c of sides(path)) {
    const curved = c[1] !== c[0] || c[2] !== c[3];
    if (!curved) out.push(c[3]);
    else for (let s = 1; s <= 24; s++) out.push(at(c, s / 24));
  }
  return out;
}

/** How many numbers each command takes, and which of them are x (1) and
    y (2), absolute ones moving with the path. */
const SHAPE: Record<string, number[]> = {
  M: [1, 2],
  L: [1, 2],
  T: [1, 2],
  H: [1],
  V: [2],
  C: [1, 2, 1, 2, 1, 2],
  S: [1, 2, 1, 2],
  Q: [1, 2, 1, 2],
  A: [0, 0, 0, 0, 0, 1, 2],
  Z: [],
};

/** Path data moved by `d`, every command kept as it was written: the
    absolute coordinates move, relative ones stay, but a first `m`'s. */
export function shiftData(data: string, d: Point): string {
  const tokens = data.match(/[a-zA-Z]|-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/g) ?? [];
  const out: string[] = [];
  let cmd = '';
  let k = 0;
  let firstMove = true;
  for (const t of tokens) {
    if (/[a-zA-Z]/.test(t)) {
      cmd = t;
      k = 0;
      out.push(t);
      continue;
    }
    const shape = SHAPE[cmd.toUpperCase()] ?? [];
    const role = shape.length ? shape[k % shape.length]! : 0;
    const abs = cmd === cmd.toUpperCase() || (cmd === 'm' && firstMove && k < 2);
    const v = Number(t);
    out.push(f(abs && role ? v + d[role - 1]! : v));
    k++;
    if (cmd.toUpperCase() === 'M' && k >= 2) firstMove = false;
  }
  // commands and their numbers, as SVG writes them
  return out.join(' ').replace(/([a-zA-Z]) /g, '$1');
}
