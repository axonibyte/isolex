// The shapes on the canvas as solids the tools work on (#69, #83): each
// node drawn, 3-D or flat, with the numbers it came to, the map from its
// own coordinates to the viewBox, and which argument each number is
// written as. Pure.

import type { Map3, NodeAt, Origin } from './backend';
import type { IrShape } from './isolex/ir';
import { pick } from './select';
import { parseAll } from './path';
import type { Target } from './snap';

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

/** What else is there to snap to, on the viewBox, each of a kind the
    Advanced list switches (#118): corners and edges of boxes' and tiles'
    footprints, at their floor and their top, their edges' midpoints and
    their centers. */
export interface Targets {
  points: { at: Point; what: string; kind: Target }[];
  edges: { a: Point; b: Point; what: string; kind: Target }[];
}

const mid = (a: Point, b: Point): Point => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
const IDENTITY: Map3 = [1, 0, 0, 1, 0, 0, 0, 0];

/** A closed outline's corners, edges, edges' midpoints and center, named
    for `what`. */
function outline(t: Targets, at: Point[], what: string, corners = true) {
  at.forEach((p, i) => {
    const q = at[(i + 1) % at.length]!;
    if (corners) t.points.push({ at: p, what: `corner of ${what}`, kind: 'corners' });
    t.points.push({ at: mid(p, q), what: `midpoint of edge of ${what}`, kind: 'midpoints' });
    t.edges.push({ a: p, b: q, what: `edge of ${what}`, kind: 'edges' });
  });
  t.points.push({ at: mid(at[0]!, at[2]!), what: `center of ${what}`, kind: 'centers' });
}

/** Corners, edges, midpoints and centers of flat rectangles, centers of
    circles, ellipses and arcs, ends and midpoints of lines, points of
    polygons and nodes of paths, and the anchors they are drawn on, on
    the viewBox; none of the glyphs of words set in `fonts`. */
export function flatTargets(all: Solid[], except: NodeAt | null, fonts: Set<string> = new Set()): Targets {
  const t: Targets = { points: [], edges: [] };
  const anchors = new Set<string>();
  for (const s of all) {
    if (except && s.node.start === except.start && s.node.file === except.file) continue;
    if (!FLAT.has(s.kind) || fonts.has(s.origin.definition)) continue;
    const v = s.values;
    const on = (x: number, y: number) => apply(s.map, [x, y, 0]);
    if (!same(s.map, IDENTITY)) {
      const a = on(0, 0);
      const key = a.map((n) => n.toFixed(6)).join();
      if (!anchors.has(key)) {
        anchors.add(key);
        t.points.push({ at: a, what: 'anchor', kind: 'anchors' });
      }
    }
    if (s.kind === 'rect') outline(t, [on(v[0]!, v[1]!), on(v[0]! + v[2]!, v[1]!), on(v[0]! + v[2]!, v[1]! + v[3]!), on(v[0]!, v[1]! + v[3]!)], 'rect');
    else if (s.kind === 'line') {
      const [a, b] = [on(v[0]!, v[1]!), on(v[2]!, v[3]!)];
      t.points.push({ at: a, what: 'end of line', kind: 'nodes' }, { at: b, what: 'end of line', kind: 'nodes' }, { at: mid(a, b), what: 'midpoint of line', kind: 'midpoints' });
    } else if (s.kind === 'circle' || s.kind === 'ellipse' || s.kind === 'arc') t.points.push({ at: on(v[0]!, v[1]!), what: `center of ${s.kind}`, kind: 'centers' });
    else if (s.kind === 'poly') for (let i = 0; i + 1 < v.length; i += 2) t.points.push({ at: on(v[i]!, v[i + 1]!), what: 'point of poly', kind: 'nodes' });
    else if (s.kind === 'path' && s.d) for (const p of parseAll(s.d) ?? []) for (const k of p.knots) t.points.push({ at: on(k.p[0], k.p[1]), what: 'node of path', kind: 'nodes' });
  }
  return t;
}

/** Those on the plane at height `at`, when given: a point is snapped on
    its own plane, and a corner below or above it only looks near. A
    cylinder offers its centers. */
export function targets(all: Solid[], except: NodeAt | null, at?: number): Targets {
  const t: Targets = { points: [], edges: [] };
  const level = (l: number) => at === undefined || Math.abs(l - at) < 1e-6;
  for (const s of all) {
    if (except && s.node.start === except.start && s.node.file === except.file) continue;
    if (s.kind === 'cylinder') {
      const v = s.values;
      for (const z of [v[2]!, top(s)].filter(level)) t.points.push({ at: apply(s.map, [v[0]!, v[1]!, z]), what: 'center of cylinder', kind: 'centers' });
      continue;
    }
    if (s.kind !== 'box' && s.kind !== 'tile') continue;
    const levels = s.kind === 'box' ? [s.values[2]!, top(s)] : [s.values[2]!];
    for (const z of levels.filter(level)) outline(t, footprint(s, z).map((p) => apply(s.map, p)), s.kind);
  }
  return t;
}

/** The icon's frame: a flat icon's canvas, or a 3-D icon's footprint at
    its floor and its top (on the plane at height `at`, when given), its
    corners, edges, their midpoints and its center all of the frame's
    kind. */
export function frameTargets(frame: { canvas?: [number, number]; footprint?: [number, number, number] }, at?: number): Targets {
  const t: Targets = { points: [], edges: [] };
  if (frame.canvas) {
    const [w, h] = frame.canvas;
    outline(t, [[0, 0], [w, 0], [w, h], [0, h]], 'the canvas');
  } else if (frame.footprint) {
    const [w, d, h] = frame.footprint;
    for (const z of [0, h].filter((l) => at === undefined || Math.abs(l - at) < 1e-6)) {
      const corners: Point3[] = [[0, 0, z], [w, 0, z], [w, d, z], [0, d, z]];
      outline(t, corners.map((p) => apply(PROJECTION, p)), 'the footprint');
    }
  }
  for (const p of t.points) p.kind = 'frame';
  for (const e of t.edges) e.kind = 'frame';
  return t;
}

/** The points of a shape that line up as it is moved, from the point it
    is moved by (its first): a box's or tile's footprint corners, edge
    midpoints and center; a flat rectangle's likewise; a line's ends and
    midpoint; a polygon's points. Others move by that point alone. */
export function moving(s: Solid): Point[] {
  const v = s.values;
  const box = (w: number, d: number): Point[] => [
    [0, 0],
    [w, 0],
    [w, d],
    [0, d],
    [w / 2, 0],
    [w, d / 2],
    [w / 2, d],
    [0, d / 2],
    [w / 2, d / 2],
  ];
  if (s.kind === 'box' || s.kind === 'tile') return box(v[3]!, v[4]!);
  if (s.kind === 'rect') return box(v[2]!, v[3]!);
  if (s.kind === 'line') return [[0, 0], [v[2]! - v[0]!, v[3]! - v[1]!], [(v[2]! - v[0]!) / 2, (v[3]! - v[1]!) / 2]];
  if (s.kind === 'poly') {
    const out: Point[] = [];
    for (let i = 0; i + 1 < v.length; i += 2) out.push([v[i]! - v[0]!, v[i + 1]! - v[1]!]);
    return out;
  }
  return [[0, 0]];
}

/** Targets together. */
export const joined = (...all: Targets[]): Targets => ({ points: all.flatMap((t) => t.points), edges: all.flatMap((t) => t.edges) });
