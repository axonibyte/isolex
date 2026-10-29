// What each gesture of the Select, Box and Tile tools comes to (#69): a
// solid's numbers after a move, a lift, a resize or a nudge, and the
// arguments that changed; the node a drawn box or tile is written as.
// Pure: the tools turn these into asks.

import type { Point } from './geometry';
import { SLOTS, type Solid } from './geometry';
import { clean, snapValue } from './snap';

/** One argument to set. */
export interface Change {
  index: number;
  value: number;
  /** What the language calls it: "x", "h"… */
  name: string;
}

/** The arguments `next` changes of `s`'s numbers. */
export function changes(s: Solid, next: number[]): Change[] {
  const slots = SLOTS[s.kind];
  if (!slots) return [];
  const out: Change[] = [];
  next.forEach((v, i) => {
    const value = clean(v);
    if (value !== clean(s.values[i]!)) out.push({ index: slots.first + i, value, name: slots.names[i]! });
  });
  return out;
}

const set = (values: number[], at: Record<number, number>) => values.map((v, i) => (i in at ? at[i]! : v));

/** Moved on its floor so its first corner (or center) is at `to`; a face
    rect along its wall, `to` being (along, z). */
export function moveTo(s: Solid, to: Point): number[] {
  if (s.kind === 'facerect') {
    const along = faceOf(s) === 'right' ? 1 : 0;
    return set(s.values, { [along]: to[0], 2: to[1] });
  }
  return set(s.values, { 0: to[0], 1: to[1] });
}

/** Lifted by `dz`, at the step; never below the floor. */
export function lift(s: Solid, dz: number, step: number): number[] {
  if (s.kind === 'shadow') return s.values;
  return set(s.values, { 2: Math.max(0, snapValue(s.values[2]! + dz, step)) });
}

/** A box's or tile's footprint from the corner opposite corner `k` (0 is
    x y, then round) to `to`. */
export function resizeCorner(s: Solid, k: number, to: Point): number[] {
  const [x, y, , w, d] = s.values as [number, number, number, number, number];
  const corners: Point[] = [
    [x, y],
    [x + w, y],
    [x + w, y + d],
    [x, y + d],
  ];
  const fixed = corners[(k + 2) % 4]!;
  return set(s.values, {
    0: Math.min(fixed[0], to[0]),
    1: Math.min(fixed[1], to[1]),
    3: Math.abs(to[0] - fixed[0]),
    4: Math.abs(to[1] - fixed[1]),
  });
}

/** A box's or cylinder's height raised by `dz`, at the step. */
export function height(s: Solid, dz: number, step: number): number[] {
  const i = s.kind === 'box' ? 5 : 4;
  return set(s.values, { [i]: Math.max(0, snapValue(s.values[i]! + dz, step)) });
}

/** A cylinder's radius, reaching `to` on its floor. */
export function radius(s: Solid, to: Point, step: number): number[] {
  const r = Math.hypot(to[0] - s.values[0]!, to[1] - s.values[1]!);
  return set(s.values, { 3: Math.max(0, snapValue(r, step)) });
}

/** A face rect's corner `k` (0 is a z, then round: along, then up) at
    `to`, as (along, z) on its wall. */
export function resizeFace(s: Solid, k: number, to: Point): number[] {
  const [a, b, z, extent, h] = s.values as [number, number, number, number, number];
  const start = faceOf(s) === 'right' ? b : a;
  const corners: Point[] = [
    [start, z],
    [start + extent, z],
    [start + extent, z + h],
    [start, z + h],
  ];
  const fixed = corners[(k + 2) % 4]!;
  const along = faceOf(s) === 'right' ? 1 : 0;
  return set(s.values, {
    [along]: Math.min(fixed[0], to[0]),
    2: Math.min(fixed[1], to[1]),
    3: Math.abs(to[0] - fixed[0]),
    4: Math.abs(to[1] - fixed[1]),
  });
}

/** Which wall a face rect is on. */
export const faceOf = (s: Solid): 'front' | 'right' => s.face ?? 'front';

/** A nudge by an arrow key, a step at a time: Right and Left along x
    (a face rect along its wall), Down and Up along y (a face rect up and
    down). PageUp and PageDown raise and lower. */
export function nudge(s: Solid, key: string, by: number): number[] | null {
  const v = s.values;
  // only what the key moves, by exactly the step: a shape off the grid
  // stays as far off it
  const by2 = (i: number, d: number) => (d ? { [i]: clean(v[i]! + d) } : {});
  if (s.kind === 'facerect') {
    const along = faceOf(s) === 'right' ? 1 : 0;
    const d = { ArrowRight: [by, 0], ArrowLeft: [-by, 0], ArrowUp: [0, by], ArrowDown: [0, -by], PageUp: [0, by], PageDown: [0, -by] }[key];
    if (!d) return null;
    const next = set(v, { ...by2(along, d[0]!), ...by2(2, d[1]!) });
    next[2] = Math.max(0, next[2]!);
    return next;
  }
  if (key === 'PageUp' || key === 'PageDown') {
    if (s.kind === 'shadow') return null;
    return set(v, { 2: Math.max(0, clean(v[2]! + (key === 'PageUp' ? by : -by))) });
  }
  const d = { ArrowRight: [by, 0], ArrowLeft: [-by, 0], ArrowDown: [0, by], ArrowUp: [0, -by] }[key];
  if (!d) return null;
  return set(v, { ...by2(0, d[0]!), ...by2(1, d[1]!) });
}

const n = (v: number) => String(clean(v));

/** The node a box drawn from corner `a` to corner `b` at height `z`, `h`
    tall, is written as. */
export function boxText(a: Point, b: Point, z: number, h: number): string {
  const [x, y, w, d] = rect(a, b);
  return `box ${n(x)} ${n(y)} ${n(z)} ${n(w)} ${n(d)} ${n(h)}`;
}

/** The node a tile drawn from `a` to `b` at height `z` is written as. */
export function tileText(a: Point, b: Point, z: number): string {
  const [x, y, w, d] = rect(a, b);
  return `tile ${n(x)} ${n(y)} ${n(z)} ${n(w)} ${n(d)}`;
}

/** The rectangle between two corners: x y w d. */
export function rect(a: Point, b: Point): [number, number, number, number] {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])];
}

/** Numbers as the feedback beside the pointer writes them. */
export const size = (...v: number[]): string => v.map(n).join(' × ');
