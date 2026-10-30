// The shapes on the canvas as solids the tools work on (#69, #83): each
// node drawn, 3-D or flat, with the numbers it came to, the map from its
// own coordinates to the viewBox, and which argument each number is
// written as. Pure.

import type { Map3, NodeAt, Origin } from './backend';
import type { IrShape } from './isolex/ir';
import { pick } from './select';

export type Point = [number, number];
export type Point3 = [number, number, number];

/** The projection a node written in the edited definition is drawn
    with, unturned (SPEC §7). */
export const PROJECTION: Map3 = [34.64101615137754, 20, -34.64101615137754, 20, 0, -40, 0, 0];

/** A node's point on the viewBox. */
export const apply = (m: Map3, [x, y, z]: Point3): Point => [m[0] * x + m[2] * y + m[4] * z + m[6], m[1] * x + m[3] * y + m[5] * z + m[7]];

/** The point at height `z` of a node's own coordinates that `at` on the
    viewBox shows: the floor under the pointer. None if the map is flat. */
export function onFloor(m: Map3, [X, Y]: Point, z: number): Point | null {
  const [a, b, c, d] = [m[0], m[2], m[1], m[3]];
  const det = a * d - b * c;
  if (Math.abs(det) < 1e-12) return null;
  const u = X - m[4] * z - m[6];
  const v = Y - m[5] * z - m[7];
  return [(d * u - b * v) / det, (a * v - c * u) / det];
}

/** The point on a wall that `at` shows: on the plane y = `wall` (front)
    or x = `wall` (right), as (along, z). */
export function onWall(m: Map3, [X, Y]: Point, face: 'front' | 'right', wall: number): Point | null {
  // front: x and z free, y fixed; right: y and z free, x fixed
  const [a, c] = face === 'front' ? [m[0], m[1]] : [m[2], m[3]];
  const [b, d] = [m[4], m[5]];
  const fixed = face === 'front' ? [m[2] * wall, m[3] * wall] : [m[0] * wall, m[1] * wall];
  const det = a * d - b * c;
  if (Math.abs(det) < 1e-12) return null;
  const u = X - fixed[0]! - m[6];
  const v = Y - fixed[1]! - m[7];
  return [(d * u - b * v) / det, (a * v - c * u) / det];
}

/** How far up the viewBox one unit of a node's z goes. */
export const zScale = (m: Map3): number => -m[5] || 1;

/** What each number of a kind is called, and which argument it is. */
export const SLOTS: Record<string, { names: string[]; first: number }> = {
  box: { names: ['x', 'y', 'z', 'w', 'd', 'h'], first: 0 },
  tile: { names: ['x', 'y', 'z', 'w', 'd'], first: 0 },
  cylinder: { names: ['cx', 'cy', 'z', 'r', 'h'], first: 0 },
  // after the face's name
  facerect: { names: ['a', 'b', 'z', 'extent', 'h'], first: 1 },
  shadow: { names: ['cx', 'cy', 'rx', 'ry'], first: 0 },
  // flat, in output units; a rectangle's corner radius is a property, a
  // polygon's points are its own nodes, and a path's are in its data
  line: { names: ['x1', 'y1', 'x2', 'y2'], first: 0 },
  circle: { names: ['cx', 'cy', 'r'], first: 0 },
  ellipse: { names: ['cx', 'cy', 'rx', 'ry'], first: 0 },
  rect: { names: ['x', 'y', 'w', 'h'], first: 0 },
  arc: { names: ['cx', 'cy', 'r', 'a0', 'a1'], first: 0 },
  poly: { names: [], first: 0 },
  path: { names: [], first: 0 },
  // where words are set from; what they say, and in what, are the Design
  // tab's
  text: { names: ['x', 'y'], first: 0 },
};

/** The kinds drawn flat, on an anchor. */
export const FLAT = new Set(['line', 'circle', 'ellipse', 'rect', 'arc', 'poly', 'path']);

/** The IR's shapes in the order they are counted: a group, then its own. */
export function counted(shapes: IrShape[]): IrShape[] {
  const out: IrShape[] = [];
  const walk = (ss: IrShape[]) => {
    for (const s of ss) {
      out.push(s);
      if (s.kind === 'group') walk(s.shapes ?? []);
    }
  };
  walk(shapes);
  return out;
}

/** One 3-D node as drawn: one copy, in a repeat. */
export interface Solid {
  kind: string;
  node: NodeAt;
  values: number[];
  map: Map3;
  /** Its shapes on the canvas, by index. */
  shapes: number[];
  /** What selecting it selects in the edited definition; none if it is
      drawn from elsewhere. */
  select: NodeAt | null;
  /** It is one copy of a repeat. */
  copy: boolean;
  /** A face rect's wall, as its source names it, when known. */
  face?: 'front' | 'right';
  /** A path's data, as drawn. */
  d?: string;
  origin: Origin;
}

/** The solids drawn, in the order drawn: consecutive shapes from one node
    and one copy are one solid. `shapes`, counted, give a path its data. */
export function solids(sources: Origin[], editing: NodeAt | null, shapes: IrShape[] = []): Solid[] {
  const out: Solid[] = [];
  sources.forEach((o, i) => {
    if (!(o.kind in SLOTS)) return;
    const last = out.at(-1);
    // a path is one shape a copy, whose numbers are in its data
    if (last && o.kind !== 'path' && last.origin.node.start === o.node.start && last.origin.node.file === o.node.file && same(last.values, o.values) && last.shapes.at(-1) === i - 1) {
      last.shapes.push(i);
      return;
    }
    out.push({
      kind: o.kind,
      node: o.node,
      values: o.values,
      map: o.map,
      shapes: [i],
      select: pick(o, editing),
      copy: o.sites.length > 0,
      ...(o.kind === 'path' && shapes[i]?.kind === 'path' ? { d: shapes[i]!.d } : {}),
      origin: o,
    });
  });
  return out;
}

const same = (a: number[], b: number[]) => a.length === b.length && a.every((v, i) => v === b[i]);

/** A box's or tile's footprint at height `z`, corner by corner: x y,
    then round. */
export function footprint(s: Solid, z: number): Point3[] {
  const [x, y, , w, d] = s.values as [number, number, number, number, number];
  return [
    [x, y, z],
    [x + w, y, z],
    [x + w, y + d, z],
    [x, y + d, z],
  ];
}

/** A solid's top: the height its top face is at, for a box (or a
    cylinder) or a tile. */
export const top = (s: Solid): number => {
  const v = s.values;
  if (s.kind === 'box') return v[2]! + v[5]!;
  if (s.kind === 'cylinder') return v[2]! + v[4]!;
  return v[2] ?? 0;
};

/** A solid that has a top face to stand on, and which of its shapes it
    is: a box's third face, a tile, a cylinder's top. */
export function topFace(s: Solid): number | null {
  if (s.kind === 'box') return s.shapes[2] ?? null;
  if (s.kind === 'tile') return s.shapes[0] ?? null;
  if (s.kind === 'cylinder') return s.shapes.at(-1) ?? null;
  return null;
}

/** What else is there to snap to, on the viewBox: corners and edges of
    boxes' and tiles' footprints, at their floor and their top. */
export interface Targets {
  points: { at: Point; what: string }[];
  edges: { a: Point; b: Point; what: string }[];
}

/** Corners of flat rectangles, ends of lines and points of polygons, on
    the viewBox. */
export function flatTargets(all: Solid[], except: NodeAt | null): Targets {
  const t: Targets = { points: [], edges: [] };
  for (const s of all) {
    if (except && s.node.start === except.start && s.node.file === except.file) continue;
    const v = s.values;
    const pts: Point[] = [];
    if (s.kind === 'rect') pts.push([v[0]!, v[1]!], [v[0]! + v[2]!, v[1]!], [v[0]! + v[2]!, v[1]! + v[3]!], [v[0]!, v[1]! + v[3]!]);
    else if (s.kind === 'line') pts.push([v[0]!, v[1]!], [v[2]!, v[3]!]);
    else if (s.kind === 'poly') for (let i = 0; i + 1 < v.length; i += 2) pts.push([v[i]!, v[i + 1]!]);
    else continue;
    const at = pts.map((p) => apply(s.map, [p[0], p[1], 0]));
    at.forEach((p) => t.points.push({ at: p, what: s.kind === 'line' ? 'end of line' : s.kind === 'rect' ? 'corner of rect' : 'point of poly' }));
  }
  return t;
}

/** Those on the plane at height `at`, when given: a point is snapped on
    its own plane, and a corner below or above it only looks near. */
export function targets(all: Solid[], except: NodeAt | null, at?: number): Targets {
  const t: Targets = { points: [], edges: [] };
  for (const s of all) {
    if (except && s.node.start === except.start && s.node.file === except.file) continue;
    if (s.kind !== 'box' && s.kind !== 'tile') continue;
    const levels = s.kind === 'box' ? [s.values[2]!, top(s)] : [s.values[2]!];
    for (const z of levels.filter((l) => at === undefined || Math.abs(l - at) < 1e-6)) {
      const at = footprint(s, z).map((p) => apply(s.map, p));
      at.forEach((p, i) => {
        t.points.push({ at: p, what: `corner of ${s.kind}` });
        t.edges.push({ a: p, b: at[(i + 1) % 4]!, what: `edge of ${s.kind}` });
      });
    }
  }
  return t;
}
