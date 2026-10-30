// The pure parts of the Select, Box and Tile tools (#69): geometry over
// the sample as the core draws it, snapping, and what each gesture comes
// to.

import { describe, expect, it } from 'vitest';
import sampleStudio from '../../../../tests/fixtures/sample.studio.golden.json';
import type { Graph, Map3, NodeAt, Origin } from './backend';
import { apply, footprint, onFloor, onWall, solids, targets, top, topFace, zScale, type Point, type Solid } from './geometry';
import { STEPS, clean, nearestOn, snapPoint, snapValue, stepLabel } from './snap';
import { boxText, changes, height, lift, moveTo, nudge, radius, resizeCorner, resizeFace, size, tileText } from './gesture';

const studio = sampleStudio as unknown as { graph: Graph; sources: Record<string, Origin[]> };
const nodeOf = (name: string): NodeAt => {
  const d = studio.graph.definitions.find((d) => d.name === name)!;
  return { file: d.file, start: d.span.start, len: d.span.len };
};
const near = (a: Point, b: Point) => {
  expect(a[0]).toBeCloseTo(b[0], 6);
  expect(a[1]).toBeCloseTo(b[1], 6);
};

/** The isolex projection, unturned: a node's map with nothing between. */
const ISO: Map3 = [34.64101615137754, 20, -34.64101615137754, 20, 0, -40, 0, 0];
const solid = (kind: string, values: number[], extra: Partial<Solid> = {}): Solid => ({
  kind,
  node: { file: 'a.ilx', start: 0, len: 1 },
  values,
  map: ISO,
  shapes: [0],
  select: null,
  copy: false,
  origin: {} as Origin,
  ...extra,
});

describe('geometry', () => {
  const l3 = solids(studio.sources['switch-l3']!, nodeOf('switch-l3'));

  it('finds the solids drawn: one box of three faces, twelve ports, a badge', () => {
    expect(l3.map((s) => s.kind)).toEqual(['box', ...Array(12).fill('facerect'), 'path']);
    expect(l3[0]!.shapes).toEqual([0, 1, 2]);
    expect(l3[0]!.values).toEqual([0, 0, 0, 2, 1, 0.28]);
    // drawn from elsewhere: nothing to select while switch-l3 is edited
    expect(l3[0]!.select).toBeNull();
    expect(l3[1]!.copy).toBe(true);
    expect(new Set(l3.slice(1, 13).map((s) => s.values[0])).size).toBe(12);
  });

  it('maps a node’s point to the viewBox and the floor back', () => {
    const box = l3[0]!;
    for (const p of footprint(box, top(box))) {
      const v = apply(box.map, p);
      near(onFloor(box.map, v, p[2])!, [p[0], p[1]]);
    }
    expect(top(box)).toBeCloseTo(0.28);
    expect(topFace(box)).toBe(2);
    expect(zScale(box.map)).toBe(40);
  });

  it('finds a point on a wall', () => {
    const v = apply(ISO, [1.25, 1, 0.5]);
    near(onWall(ISO, v, 'front', 1)!, [1.25, 0.5]);
    const r = apply(ISO, [2, 0.75, 0.25]);
    near(onWall(ISO, r, 'right', 2)!, [0.75, 0.25]);
  });

  it('offers a box’s corners and edges, at its floor and its top, as targets', () => {
    const t = targets(l3, null);
    const corners = t.points.filter((p) => p.kind === 'corners');
    expect(corners).toHaveLength(8);
    expect(t.edges).toHaveLength(8);
    // and each face's edge midpoints and center
    expect(t.points.filter((p) => p.kind === 'midpoints')).toHaveLength(8);
    expect(t.points.filter((p) => p.kind === 'centers')).toHaveLength(2);
    expect(targets(l3, l3[0]!.node).points).toHaveLength(0);
    // on a plane, only what is on it
    expect(targets(l3, null, 0.28).points.filter((p) => p.kind === 'corners')).toHaveLength(4);
    expect(targets(l3, null, 0.5).points).toHaveLength(0);
  });
});

describe('snapping', () => {
  const toView = (p: Point) => apply(ISO, [p[0], p[1], 0]);
  const fromView = (v: Point) => onFloor(ISO, v, 0);
  const box = solid('box', [0, 0, 0, 2, 1, 0.5]);
  const t = targets([box], null);

  it('offers the steps', () => {
    expect(STEPS.map(stepLabel)).toEqual(['1', '½', '¼', '⅛', 'off']);
  });

  it.each([
    [1.3, 0.25, 1.25],
    [1.3, 0.5, 1.5],
    [1.3, 0, 1.3],
    [0.1 + 0.2, 0.1, 0.3],
    [-0.0001, 0.25, 0],
  ])('%f at %f is %f', (v, step, want) => {
    expect(snapValue(v, step)).toBe(want);
  });

  it('goes to the grid away from other shapes', () => {
    const s = snapPoint([3.4, 3.1], 0.5, t, toView, fromView, 1);
    expect(s).toEqual({ point: [3.5, 3], what: null, at: null, guides: [] });
  });

  it('goes to a corner within reach on screen, before the grid', () => {
    // 0.1 tile from the box's far corner is ~4 viewBox units at scale 1
    const s = snapPoint([2.1, 0.95], 1, t, toView, fromView, 1);
    expect(s.what).toBe('corner of box');
    near(s.point, [2, 1]);
    // zoomed out, the same distance is out of reach... in tiles, not on screen
    expect(snapPoint([2.1, 0.95], 0.25, t, toView, fromView, 0.1).what).toBe('corner of box');
    expect(snapPoint([2.6, 1.6], 0.25, t, toView, fromView, 4).what).toBeNull();
  });

  it('goes to an edge when no corner is near', () => {
    // a quarter along the far edge: its midpoint, at (1, 1), out of reach
    const s = snapPoint([0.53, 1.04], 0.25, t, toView, fromView, 1);
    expect(s.what).toBe('edge of box');
    expect(s.point[1]).toBeCloseTo(1, 1);
  });

  it('finds the nearest point on a segment, ends included', () => {
    expect(nearestOn([0, 0], [10, 0], [4, 3])).toEqual([4, 0]);
    expect(nearestOn([0, 0], [10, 0], [-4, 3])).toEqual([0, 0]);
    expect(nearestOn([1, 1], [1, 1], [4, 3])).toEqual([1, 1]);
    expect(clean(-0)).toBe(0);
  });
});

describe('gestures', () => {
  const box = solid('box', [0, 0, 0, 2, 1, 0.5]);
  const named = (s: Solid, next: number[]) => changes(s, next).map((c) => `${c.name}=${c.value}@${c.index}`);

  it('moves on the floor, and sets only what changed', () => {
    expect(named(box, moveTo(box, [0.5, 0]))).toEqual(['x=0.5@0']);
    expect(named(box, moveTo(box, [0.5, 0.25]))).toEqual(['x=0.5@0', 'y=0.25@1']);
  });

  it('lifts at the step, never below the floor', () => {
    expect(named(box, lift(box, 0.3, 0.25))).toEqual(['z=0.25@2']);
    expect(named(box, lift(box, -1, 0.25))).toEqual([]);
  });

  it('resizes a footprint from the opposite corner, either way over', () => {
    // the far corner (2) dragged to 3 2
    expect(resizeCorner(box, 2, [3, 2])).toEqual([0, 0, 0, 3, 2, 0.5]);
    // the near corner (0) dragged past the far one
    expect(resizeCorner(box, 0, [3, 2])).toEqual([2, 1, 0, 1, 1, 0.5]);
  });

  it('raises a height, and sets a radius', () => {
    expect(named(box, height(box, 0.26, 0.25))).toEqual(['h=0.75@5']);
    const cyl = solid('cylinder', [1, 1, 0, 0.5, 1]);
    expect(named(cyl, radius(cyl, [1, 1.8], 0.25))).toEqual(['r=0.75@3']);
    expect(named(cyl, height(cyl, -2, 0.25))).toEqual(['h=0@4']);
  });

  it('moves and resizes a face rect on its wall, the face name first', () => {
    const front = solid('facerect', [0.25, 1, 0.1, 0.5, 0.25], { face: 'front' });
    expect(named(front, moveTo(front, [0.5, 0.2]))).toEqual(['a=0.5@1', 'z=0.2@3']);
    expect(resizeFace(front, 2, [1, 0.5])).toEqual([0.25, 1, 0.1, 0.75, 0.4]);
    const right = solid('facerect', [2, 0.25, 0, 0.5, 0.25], { face: 'right' });
    expect(named(right, moveTo(right, [0.5, 0]))).toEqual(['b=0.5@2']);
  });

  it.each([
    ['ArrowRight', 0.25, ['x=0.25@0']],
    ['ArrowLeft', 1, ['x=-1@0']],
    ['ArrowDown', 0.5, ['y=0.5@1']],
    ['ArrowUp', 0.25, ['y=-0.25@1']],
    ['PageUp', 0.25, ['z=0.25@2']],
    ['PageDown', 0.25, []],
  ])('nudges with %s by %f', (key, by, want) => {
    expect(named(box, nudge(box, key, by)!)).toEqual(want);
  });

  it('nudges by the step, off the grid as far as before', () => {
    const off = solid('box', [0.3, 0.1, 0, 1, 1, 1]);
    expect(named(off, nudge(off, 'ArrowDown', 0.25)!)).toEqual(['y=0.35@1']);
  });

  it('nudges a face rect along its wall and up', () => {
    const front = solid('facerect', [0.25, 1, 0.1, 0.5, 0.25], { face: 'front' });
    expect(named(front, nudge(front, 'ArrowUp', 0.1)!)).toEqual(['z=0.2@3']);
    expect(nudge(box, 'Enter', 1)).toBeNull();
  });

  it('writes what it draws, whichever way it was dragged', () => {
    expect(boxText([2, 1], [0.5, 0], 0.28, 0.5)).toBe('box 0.5 0 0.28 1.5 1 0.5');
    expect(tileText([0, 0], [1, 1], 0)).toBe('tile 0 0 0 1 1');
    expect(size(2, 1, 0.30000000000000004)).toBe('2 × 1 × 0.3');
  });
});
