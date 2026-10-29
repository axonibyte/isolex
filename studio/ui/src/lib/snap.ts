// Snapping (docs/studio-interface.md, The canvas): a point drawn or
// dragged goes to another shape's corner or edge when the pointer is near
// one on screen, else to the grid at the chosen step. Pure.

import type { Point, Targets } from './geometry';

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
}

/** Snaps `p`, a point on a plane in tile units, which `toView` shows on
    the viewBox and `fromView` finds again; `scale` is the viewBox's
    pixels per unit, so `PULL` is measured on screen. */
export function snapPoint(
  p: Point,
  step: number,
  targets: Targets,
  toView: (p: Point) => Point,
  fromView: (v: Point) => Point | null,
  scale: number,
): Snapped {
  const at = toView(p);
  const reach = PULL / scale;
  let best: { d: number; v: Point; what: string } | null = null;
  for (const t of targets.points) {
    const d = Math.hypot(t.at[0] - at[0], t.at[1] - at[1]);
    if (d <= reach && (!best || d < best.d)) best = { d, v: t.at, what: t.what };
  }
  if (!best) {
    for (const e of targets.edges) {
      const v = nearestOn(e.a, e.b, at);
      const d = Math.hypot(v[0] - at[0], v[1] - at[1]);
      if (d <= reach && (!best || d < best.d)) best = { d, v, what: e.what };
    }
  }
  if (best) {
    const q = fromView(best.v);
    if (q) return { point: [clean(q[0]), clean(q[1])], what: best.what, at: best.v };
  }
  return { point: [snapValue(p[0], step), snapValue(p[1], step)], what: null, at: null };
}

/** The point of the segment from `a` to `b` nearest `p`. */
export function nearestOn(a: Point, b: Point, p: Point): Point {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = dx * dx + dy * dy;
  const t = len ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len)) : 0;
  return [a[0] + t * dx, a[1] + t * dy];
}
