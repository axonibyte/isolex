// The flat tools (#83, docs/studio-interface.md, Flat): lines,
// rectangles, ellipses, arcs and polygons drawn in output units on an
// `at` anchor, what Shift does to each, and the node each is written as.
// Pure.

import type { Point } from './geometry';
import { clean, snapValue } from './snap';

/** The flat step for a tile step: four units a tile, never finer than
    half a unit; 0 is off. */
export const flatStep = (step: number): number => (step ? Math.max(0.5, step * 4) : 0);

/** A point at the flat step. */
export const snapFlat = (p: Point, step: number): Point => [snapValue(p[0], flatStep(step)), snapValue(p[1], flatStep(step))];

/** `b` turned about `a` to the nearest 15°, as far from it, at `step`. */
export function angled(a: Point, b: Point, step = 0): Point {
  const len = snapValue(Math.hypot(b[0] - a[0], b[1] - a[1]), step);
  const turn = Math.PI / 12;
  const t = Math.round(Math.atan2(b[1] - a[1], b[0] - a[0]) / turn) * turn;
  return [clean(a[0] + len * Math.cos(t)), clean(a[1] + len * Math.sin(t))];
}

/** `b` moved so the box from `a` to it is square, as large as its
    longer side. */
export function squared(a: Point, b: Point): Point {
  const s = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
  return [a[0] + Math.sign(b[0] - a[0] || 1) * s, a[1] + Math.sign(b[1] - a[1] || 1) * s];
}

const n = (v: number) => String(clean(v));

export const lineText = (a: Point, b: Point): string => `line ${n(a[0])} ${n(a[1])} ${n(b[0])} ${n(b[1])}`;

export function rectText(a: Point, b: Point): string {
  const [x, y] = [Math.min(a[0], b[0]), Math.min(a[1], b[1])];
  return `rect ${n(x)} ${n(y)} ${n(Math.abs(b[0] - a[0]))} ${n(Math.abs(b[1] - a[1]))}`;
}

/** The ellipse in the box from `a` to `b`: a circle when it is square. */
export function ellipseText(a: Point, b: Point): string {
  const [cx, cy] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
  const [rx, ry] = [Math.abs(b[0] - a[0]) / 2, Math.abs(b[1] - a[1]) / 2];
  return clean(rx) === clean(ry) ? `circle ${n(cx)} ${n(cy)} ${n(rx)}` : `ellipse ${n(cx)} ${n(cy)} ${n(rx)} ${n(ry)}`;
}

/** The angle of `p` about `c`, as an arc measures it: degrees,
    counter-clockwise from +x on a screen whose y runs down, 0 to 360. */
export function angleOf(c: Point, p: Point): number {
  const a = (Math.atan2(c[1] - p[1], p[0] - c[0]) * 180) / Math.PI;
  return clean(a < 0 ? a + 360 : a);
}

/** The point at angle `a` (as `angleOf`) and radius `r` about `c`. */
export const atAngle = (c: Point, r: number, a: number): Point => [c[0] + r * Math.cos((a * Math.PI) / 180), c[1] - r * Math.sin((a * Math.PI) / 180)];

/** An arc from `a0` round to `a1`, counter-clockwise, at whole degrees
    (to 15° with Shift). */
export function arcText(c: Point, r: number, a0: number, a1: number): string {
  return `arc ${n(c[0])} ${n(c[1])} ${n(r)} ${n(a0)} ${n(a1)}`;
}

/** An angle at whole degrees, or 15° steps. */
export const snapAngle = (a: number, shift: boolean): number => {
  const s = snapValue(a, shift ? 15 : 1);
  return s >= 360 ? s - 360 : s;
};

/** A polygon's node, a point a line. */
export const polyText = (pts: Point[]): string => `poly {\n${pts.map((p) => `  pt ${n(p[0])} ${n(p[1])}`).join('\n')}\n}`;

/** The points of an arc, for drawing it while it is drawn. */
export function arcPoints(c: Point, r: number, a0: number, a1: number): Point[] {
  const sweep = (((a1 - a0) % 360) + 360) % 360 || 360;
  const k = Math.max(2, Math.ceil(sweep / 6));
  return Array.from({ length: k + 1 }, (_, i) => atAngle(c, r, a0 + (sweep * i) / k));
}

/** An ellipse's outline, for drawing it while it is drawn. */
export const ellipsePoints = (c: Point, rx: number, ry: number): Point[] =>
  Array.from({ length: 49 }, (_, i) => [c[0] + rx * Math.cos((i * Math.PI) / 24), c[1] + ry * Math.sin((i * Math.PI) / 24)] as Point);

/** A flat shape's numbers moved by `d`: every x by d's x, every y by its
    y. */
export function moveFlat(kind: string, values: number[], d: Point): number[] {
  const xy = (i: number) => i % 2;
  return values.map((v, i) => {
    if (kind === 'line' || kind === 'poly') return clean(v + d[xy(i)]!);
    // a rectangle's or a round shape's first two are where it is
    return i < 2 ? clean(v + d[i]!) : v;
  });
}

/** A rectangle's corner `k` (0 is x y, then round) dragged to `to`, the
    opposite corner staying. */
export function resizeRect(values: number[], k: number, to: Point): number[] {
  const [x, y, w, h] = values as [number, number, number, number];
  const corners: Point[] = [
    [x, y],
    [x + w, y],
    [x + w, y + h],
    [x, y + h],
  ];
  const fixed = corners[(k + 2) % 4]!;
  const next = values.slice();
  next[0] = clean(Math.min(fixed[0], to[0]));
  next[1] = clean(Math.min(fixed[1], to[1]));
  next[2] = clean(Math.abs(to[0] - fixed[0]));
  next[3] = clean(Math.abs(to[1] - fixed[1]));
  return next;
}

/** A round shape's radius handle dragged to `to`: a circle's or an arc's
    r, an ellipse's rx (across) or ry (down). */
export function resizeRound(kind: string, values: number[], handle: string, to: Point): number[] {
  const next = values.slice();
  const [cx, cy] = values as [number, number];
  if (kind === 'ellipse') {
    if (handle === 'rx') next[2] = clean(Math.abs(to[0] - cx));
    else next[3] = clean(Math.abs(to[1] - cy));
  } else next[2] = clean(Math.hypot(to[0] - cx, to[1] - cy));
  return next;
}

/** A polygon's node with its points as `pts`, each line indented as its
    first point was; none if any point is not two plain numbers (an
    expression, or a repeat), which only the source can change. */
export function withPoints(text: string, pts: Point[]): string | null {
  const open = text.indexOf('{');
  const close = text.lastIndexOf('}');
  if (open < 0 || close < open) return null;
  const body = text.slice(open + 1, close);
  const lines = body
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  const plain = /^pt\s+-?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?\s+-?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/;
  if (!lines.length || !lines.every((l) => plain.test(l))) return null;
  const nl = text.includes('\r\n') ? '\r\n' : '\n';
  const indent = body.match(/\n([ \t]*)pt/)?.[1] ?? '  ';
  const last = body.match(/\n([ \t]*)$/)?.[1] ?? '';
  return `${text.slice(0, open + 1)}${nl}${pts.map((p) => `${indent}pt ${n(p[0])} ${n(p[1])}`).join(nl)}${nl}${last}${text.slice(close)}`;
}

/** A path's data is written as plain text: no `{…}` worked out as it is
    drawn, which only the source can change. */
export const plainData = (text: string): boolean => !/\bd="[^"]*\{/.test(text);

/** Where a point of the viewBox is in output units, the flat plane. */
export const PLANE: [number, number, number, number, number, number, number, number] = [1, 0, 0, 1, 0, 0, 0, 0];
