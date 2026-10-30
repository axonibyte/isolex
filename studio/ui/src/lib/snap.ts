// Snapping (docs/studio-interface.md, The canvas): a point drawn or
// dragged goes to another shape's corner or edge when the pointer is near
// one on screen, else to the grid at the chosen step. As Inkscape 1.2
// does (#118), it also lines up with other shapes' points along the
// plane's axes, drawing the guide it lines up on, and each kind of target
// can be switched off. Pure.

import type { Point, Targets } from './geometry';

/** The kinds of target, as the Advanced list names them. */
export const TARGETS = [
  ['grid', 'Grid lines and intersections'],
  ['corners', 'Corners'],
  ['edges', 'Edges'],
  ['midpoints', 'Edge midpoints'],
  ['centers', 'Face centers'],
  ['anchors', 'Anchors'],
  ['nodes', 'Path nodes'],
  ['frame', 'The frame'],
  ['guides', 'Alignment guides'],
] as const;
export type Target = (typeof TARGETS)[number][0];
/** Which kinds are on. */
export type Switches = Record<Target, boolean>;

export const ALL_ON: Switches = Object.fromEntries(TARGETS.map(([t]) => [t, true])) as Switches;

/** Switches as stored, any missing or unknown taken as on. */
export function switches(raw: string | null): Switches {
  let read: Record<string, unknown> = {};
  try {
    read = raw ? JSON.parse(raw) : {};
  } catch {
    // stored badly: all on
  }
  return Object.fromEntries(TARGETS.map(([t]) => [t, read[t] !== false])) as Switches;
}

/** The steps offered: a whole tile down to an eighth, or off (0). */
export const STEPS = [1, 0.5, 0.25, 0.125, 0] as const;
export type Step = (typeof STEPS)[number];

export const stepLabel = (s: Step): string => ({ 1: '1', 0.5: '½', 0.25: '¼', 0.125: '⅛', 0: 'off' })[s];

/** How near, in screen pixels, a corner or an edge pulls. */
export const PULL = 8;

/** `v` at the step, and without float dust (0.1 + 0.2 is 0.3 here). */
export function snapValue(v: number, step: number): number {
  const s = step ? Math.round(v / step) * step : v;
  return clean(s);
}

/** A number as it is written: six places at most. */
export const clean = (v: number): number => {
  const r = Math.round(v * 1e6) / 1e6;
  return Object.is(r, -0) ? 0 : r;
};

/** Where a point went, and what it snapped to, for the indicator: none
    for the grid or nothing. */
export interface Snapped {
  point: Point;
  what: string | null;
  /** Where on the viewBox the indicator goes. */
  at: Point | null;
  /** The alignment guides lined up on, on the viewBox: from the point
      lined up with to the point that lines up. */
  guides: { a: Point; b: Point }[];
}

export interface SnapOptions {
  /** The kinds of target on; all, if not given. */
  on?: Switches;
  /** Points that move with `p`, from it, each snapping as it would: a
      shape's corners, edge midpoints and center as it is moved. `p`
      itself is [0, 0], and is taken first if not given. */
  with?: Point[];
}

const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const plus = (a: Point, b: Point): Point => [a[0] + b[0], a[1] + b[1]];

/** Snaps `p`, a point on a plane in tile units, which `toView` shows on
    the viewBox and `fromView` finds again; `scale` is the viewBox's
    pixels per unit, so `PULL` is measured on screen. A point nearby comes
    first, then an edge; then, along each axis, an alignment with another
    point if it is nearer than the grid's line, else the grid. A kind
    switched off is passed over. */
export function snapPoint(
  p: Point,
  step: number,
  targets: Targets,
  toView: (p: Point) => Point,
  fromView: (v: Point) => Point | null,
  scale: number,
  options: SnapOptions = {},
): Snapped {
  const on = options.on ?? ALL_ON;
  const offsets = options.with?.length ? options.with : [[0, 0] as Point];
  const reach = PULL / scale;
  const grid = on.grid ? step : 0;
  const points = targets.points.filter((t) => on[t.kind]);
  const moving = offsets.map((o) => ({ o, at: toView(plus(p, o)) }));
  let best: { d: number; v: Point; o: Point; what: string } | null = null;
  for (const m of moving)
    for (const t of points) {
      const d = dist(t.at, m.at);
      if (d <= reach && (!best || d < best.d)) best = { d, v: t.at, o: m.o, what: t.what };
    }
  if (!best) {
    for (const m of moving)
      for (const e of targets.edges.filter((e) => on[e.kind])) {
        const v = nearestOn(e.a, e.b, m.at);
        const d = dist(v, m.at);
        if (d <= reach && (!best || d < best.d)) best = { d, v, o: m.o, what: e.what };
      }
  }
  if (best) {
    const q = fromView(best.v);
    if (q) return { point: [clean(q[0] - best.o[0]), clean(q[1] - best.o[1])], what: best.what, at: best.v, guides: [] };
  }
  const gridded: Point = [snapValue(p[0], grid), snapValue(p[1], grid)];
  if (on.guides) {
    // how far each axis's grid line is, on screen; none with no grid
    const toGrid = [0, 1].map((k) => (grid ? dist(toView(k === 0 ? [gridded[0], p[1]] : [p[0], gridded[1]]), toView(p)) : Infinity));
    const lined = align(p, offsets, points, toView, fromView, reach).map((a, k) => (a && a.d < toGrid[k]! ? a : null));
    const used = lined.filter((a) => a !== null);
    if (used.length) {
      const point = [0, 1].map((k) => clean(lined[k] ? lined[k]!.to : gridded[k]!)) as Point;
      const guides = used.map((a) => ({ a: a.from, b: toView(plus(point, a.o)) }));
      return { point, what: used.map((a) => `aligned with ${a.what}`).join(', '), at: guides[0]!.b, guides };
    }
  }
  return { point: gridded, what: null, at: null, guides: [] };
}

type Lined = { to: number; d: number; o: Point; from: Point; what: string };

/** The nearest alignment along each of the plane's axes: a moving point
    that, at the same x (or y) as a target, is within `reach` on screen
    of where it is. */
function align(
  p: Point,
  offsets: Point[],
  points: Targets['points'],
  toView: (p: Point) => Point,
  fromView: (v: Point) => Point | null,
  reach: number,
): [Lined | null, Lined | null] {
  const axes: [Lined | null, Lined | null] = [null, null];
  const sources = points.map((t) => ({ t, q: fromView(t.at) })).filter((s) => s.q);
  for (const o of offsets) {
    const m = plus(p, o);
    const at = toView(m);
    for (const { t, q } of sources) {
      for (const k of [0, 1] as const) {
        const there: Point = k === 0 ? [q![0], m[1]] : [m[0], q![1]];
        const d = dist(toView(there), at);
        // a guide along nothing, the point itself, is a point snap's
        if (d > reach || dist(toView(there), t.at) < 1e-9) continue;
        if (!axes[k] || d < axes[k]!.d) axes[k] = { to: q![k] - o[k], d, o, from: t.at, what: t.what };
      }
    }
  }
  return axes;
}

/** The point of the segment from `a` to `b` nearest `p`. */
export function nearestOn(a: Point, b: Point, p: Point): Point {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = dx * dx + dy * dy;
  const t = len ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len)) : 0;
  return [a[0] + t * dx, a[1] + t * dy];
}
