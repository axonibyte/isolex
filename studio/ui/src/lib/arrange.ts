// Align and Distribute (#114, docs/studio-interface.md, Arrange), as
// Inkscape's dialog: shapes lined up by an edge or their middles along an
// axis, relative to the first or last chosen, the selection's bounds or
// the icon's footprint (a flat icon's canvas); or spread evenly. 3-D
// shapes are measured along the isometric x, y and z; flat shapes along
// their plane's x and y. Pure: the tools write the moves.

import type { Map3 } from './backend';
import { FLAT, PROJECTION, onFloor, type Point, type Solid } from './geometry';
import { faceOf } from './gesture';
import { atAngle } from './flat';
import { parseAll } from './path';
import { clean } from './snap';

/** A shape's bounds, axis by axis, and the axes it can move along: a face
    rect stays on its wall, a shadow on the floor. */
export interface Extent {
  lo: number[];
  hi: number[];
  moves: boolean[];
}

export type Edge = 'min' | 'center' | 'max';
export type Relative = 'first' | 'last' | 'selection' | 'frame';
export type Spread = 'centers' | 'gaps';

const ALL3 = [true, true, true];
const ALL2 = [true, true];

/** Bounds of points, as an extent every axis of which moves. */
function around(pts: Point[]): Extent | null {
  if (!pts.length) return null;
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return { lo: [Math.min(...xs), Math.min(...ys)], hi: [Math.max(...xs), Math.max(...ys)], moves: ALL2 };
}

const pairs = (v: number[]): Point[] => Array.from({ length: Math.floor(v.length / 2) }, (_, i) => [v[2 * i]!, v[2 * i + 1]!]);

/** The arc from `a0` round to `a1`, counter-clockwise, as its ends and the
    quarters it passes. */
function arcPoints(c: Point, r: number, a0: number, a1: number): Point[] {
  const sweep = (((a1 - a0) % 360) + 360) % 360 || 360;
  const pts = [atAngle(c, r, a0), atAngle(c, r, a0 + sweep)];
  for (const q of [0, 90, 180, 270, 360, 450, 540, 630]) if (q > a0 && q < a0 + sweep) pts.push(atAngle(c, r, q));
  return pts;
}

/** What a solid occupies, in its own coordinates: 3-D, or flat in its
    plane; none for what has no bounds to align by (text, whose glyphs
    the font sets). */
export function extentOf(s: Solid): Extent | null {
  const v = s.values;
  switch (s.kind) {
    case 'box':
      return { lo: [v[0]!, v[1]!, v[2]!], hi: [v[0]! + v[3]!, v[1]! + v[4]!, v[2]! + v[5]!], moves: ALL3 };
    case 'tile':
      return { lo: [v[0]!, v[1]!, v[2]!], hi: [v[0]! + v[3]!, v[1]! + v[4]!, v[2]!], moves: ALL3 };
    case 'cylinder':
      return { lo: [v[0]! - v[3]!, v[1]! - v[3]!, v[2]!], hi: [v[0]! + v[3]!, v[1]! + v[3]!, v[2]! + v[4]!], moves: ALL3 };
    case 'shadow':
      return { lo: [v[0]! - v[2]!, v[1]! - v[3]!, 0], hi: [v[0]! + v[2]!, v[1]! + v[3]!, 0], moves: [true, true, false] };
    case 'facerect': {
      const [a, b, z, extent, h] = v as [number, number, number, number, number];
      return faceOf(s) === 'right'
        ? { lo: [a, b, z], hi: [a, b + extent, z + h], moves: [false, true, true] }
        : { lo: [a, b, z], hi: [a + extent, b, z + h], moves: [true, false, true] };
    }
    case 'line':
    case 'poly':
      return around(pairs(v));
    case 'rect':
      return around([
        [v[0]!, v[1]!],
        [v[0]! + v[2]!, v[1]! + v[3]!],
      ]);
    case 'circle':
      return around([
        [v[0]! - v[2]!, v[1]! - v[2]!],
        [v[0]! + v[2]!, v[1]! + v[2]!],
      ]);
    case 'ellipse':
      return around([
        [v[0]! - v[2]!, v[1]! - v[3]!],
        [v[0]! + v[2]!, v[1]! + v[3]!],
      ]);
    case 'arc':
      return around(arcPoints([v[0]!, v[1]!], v[2]!, v[3]!, v[4]!));
    case 'path':
      // by its points: a curve bulging past them is not measured
      return s.d ? around((parseAll(s.d) ?? []).flatMap((p) => p.knots.map((k) => k.p))) : null;
    default:
      return null;
  }
}

/** The icon's frame, as an extent in the coordinates `s` is drawn in: a
    3-D icon's footprint, from the origin, for a shape drawn in the
    icon's own coordinates; a flat icon's canvas, in the plane a flat
    shape is drawn on. None for a flat shape on a 3-D icon's face. */
export function frameOf(icon: { footprint?: [number, number, number]; canvas?: [number, number] }, s: Solid): Extent | null {
  if (FLAT.has(s.kind)) {
    const a = icon.canvas ? onFloor(s.map, [0, 0], 0) : null;
    const b = icon.canvas ? onFloor(s.map, icon.canvas, 0) : null;
    return a && b ? around([a, b]) : null;
  }
  return icon.footprint && sameFrame(s.map, PROJECTION) ? { lo: [0, 0, 0], hi: [...icon.footprint], moves: ALL3 } : null;
}


const edgeOf = (e: Extent, axis: number, edge: Edge) => (edge === 'min' ? e.lo[axis]! : edge === 'max' ? e.hi[axis]! : (e.lo[axis]! + e.hi[axis]!) / 2);

/** What the shapes are aligned to: the first or last chosen, all of
    them, or the frame. */
export function reference(extents: Extent[], relative: Relative, frame: Extent | null): Extent | null {
  if (!extents.length) return null;
  if (relative === 'first') return extents[0]!;
  if (relative === 'last') return extents.at(-1)!;
  if (relative === 'frame') return frame;
  const dims = extents[0]!.lo.length;
  const lo = Array.from({ length: dims }, (_, i) => Math.min(...extents.map((e) => e.lo[i]!)));
  const hi = Array.from({ length: dims }, (_, i) => Math.max(...extents.map((e) => e.hi[i]!)));
  return { lo, hi, moves: extents[0]!.moves };
}

/** How far each shape moves along `axis` for its `edge` to meet the
    reference's; none for a shape that cannot move that way. */
export function align(extents: Extent[], axis: number, edge: Edge, ref: Extent): number[] {
  const to = edgeOf(ref, axis, edge);
  return extents.map((e) => (e.moves[axis] ? clean(to - edgeOf(e, axis, edge)) : 0));
}

/** How far each shape moves along `axis` to spread them evenly, in the
    order their middles come: the outermost two stay, and the rest go so
    their middles are evenly apart, or so the gaps between them are.
    Fewer than three that can move that way stay as they are. */
export function distribute(extents: Extent[], axis: number, spread: Spread): number[] {
  const out = extents.map(() => 0);
  const order = extents
    .map((e, i) => ({ e, i, c: edgeOf(e, axis, 'center') }))
    .filter((x) => x.e.moves[axis])
    .sort((a, b) => a.c - b.c || a.i - b.i);
  const n = order.length;
  if (n < 3) return out;
  const first = order[0]!;
  const last = order[n - 1]!;
  if (spread === 'centers') {
    const step = (last.c - first.c) / (n - 1);
    order.forEach((x, k) => (out[x.i] = clean(first.c + k * step - x.c)));
    return out;
  }
  const size = (e: Extent) => e.hi[axis]! - e.lo[axis]!;
  const gap = (last.e.hi[axis]! - first.e.lo[axis]! - order.reduce((sum, x) => sum + size(x.e), 0)) / (n - 1);
  let at = first.e.lo[axis]!;
  for (const x of order) {
    out[x.i] = clean(at - x.e.lo[axis]!);
    at += size(x.e) + gap;
  }
  return out;
}

/** A 3-D solid's numbers moved by `by` along x, y and z. */
export function shifted(s: Solid, by: number[]): number[] {
  const v = s.values.slice();
  const add = (i: number, d: number) => (v[i] = clean(v[i]! + d));
  if (s.kind === 'facerect') {
    if (faceOf(s) === 'right') add(1, by[1]!);
    else add(0, by[0]!);
    add(2, by[2]!);
    return v;
  }
  add(0, by[0]!);
  add(1, by[1]!);
  if (s.kind !== 'shadow') add(2, by[2]!);
  return v;
}

/** Two shapes drawn in the same coordinates: aligned together. */
export const sameFrame = (a: Map3, b: Map3) => a.every((x, i) => Math.abs(x - b[i]!) < 1e-9);
