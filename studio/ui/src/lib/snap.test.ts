// Advanced snapping (#118): each kind of target, what it pulls a point to,
// and that switching it off stops it; alignment guides along the plane's
// axes; and a shape's own points lining up as it moves. On a flat plane
// at scale 1, where a target pulls from 8 units.

import { describe, expect, it } from 'vitest';
import type { Map3, NodeAt, Origin } from './backend';
import { apply, flatTargets, frameTargets, joined, moving, onFloor, targets, type Point, type Solid, type Targets } from './geometry';
import { ALL_ON, TARGETS, snapPoint, switches, type Target } from './snap';

const FLAT: Map3 = [1, 0, 0, 1, 0, 0, 0, 0];
const ISO: Map3 = [34.64101615137754, 20, -34.64101615137754, 20, 0, -40, 0, 0];
let at = 0;
const solid = (kind: string, values: number[], extra: Partial<Solid> = {}): Solid => ({
  kind,
  node: { file: 'a.ilx', start: at++, len: 1 } as NodeAt,
  values,
  map: FLAT,
  shapes: [0],
  select: null,
  copy: false,
  origin: { definition: 'icon' } as Origin,
  ...extra,
});

const same = (p: Point) => p;
/** `p` snapped on the flat plane among `t`, at step 1, with `off` off. */
const snap = (p: Point, t: Targets, off: Target[] = [], step = 1, offsets?: Point[]) =>
  snapPoint(p, step, t, same, same, 1, { on: { ...ALL_ON, ...Object.fromEntries(off.map((k) => [k, false])) }, with: offsets });

describe('the switches', () => {
  it('lists each kind of target, as the Advanced list names them', () => {
    expect(TARGETS.map(([, label]) => label)).toEqual([
      'Grid lines and intersections',
      'Corners',
      'Edges',
      'Edge midpoints',
      'Face centers',
      'Anchors',
      'Path nodes',
      'The frame',
      'Alignment guides',
    ]);
  });

  it('reads them as stored, any missing or unreadable taken as on', () => {
    expect(switches(null)).toEqual(ALL_ON);
    expect(switches('{"grid":false,"zebra":false}')).toEqual({ ...ALL_ON, grid: false });
    expect(switches('not json')).toEqual(ALL_ON);
  });
});

describe('each target, and switching it off', () => {
  const rect = flatTargets([solid('rect', [10, 10, 40, 20])], null);

  it('the grid: intersections at the step, or nothing', () => {
    expect(snap([70.4, 80.6], rect).point).toEqual([70, 81]);
    expect(snap([70.4, 80.6], rect, ['grid']).point).toEqual([70.4, 80.6]);
  });

  it('corners', () => {
    expect(snap([52, 33], rect)).toMatchObject({ point: [50, 30], what: 'corner of rect' });
    expect(snap([52, 33], rect, ['corners']).what).not.toBe('corner of rect');
  });

  it('edges', () => {
    expect(snap([20.4, 12], rect)).toMatchObject({ point: [20.4, 10], what: 'edge of rect' });
    expect(snap([20.4, 12], rect, ['edges']).point).toEqual([20, 12]);
  });

  it('edge midpoints', () => {
    expect(snap([28, 12], rect)).toMatchObject({ point: [30, 10], what: 'midpoint of edge of rect' });
    expect(snap([28, 12], rect, ['midpoints']).what).toBe('edge of rect');
    const line = flatTargets([solid('line', [0, 0, 20, 0])], null);
    expect(snap([11, 3], line, ['guides'])).toMatchObject({ point: [10, 0], what: 'midpoint of line' });
  });

  it('face centers: a rectangle’s, a circle’s, a box’s faces', () => {
    expect(snap([27, 19], rect)).toMatchObject({ point: [30, 20], what: 'center of rect' });
    expect(snap([27, 19], rect, ['centers', 'guides']).point).toEqual([27, 19]);
    const circle = flatTargets([solid('circle', [60, 60, 5])], null);
    expect(snap([62, 58], circle)).toMatchObject({ point: [60, 60], what: 'center of circle' });
    const box = targets([solid('box', [0, 0, 0, 2, 2, 1], { map: ISO })], null, 1);
    expect(box.points.find((p) => p.kind === 'centers')!.at).toEqual(apply(ISO, [1, 1, 1]));
  });

  it('anchors: the origin of a plane flat shapes are drawn on', () => {
    const on = flatTargets([solid('circle', [5, 5, 2], { map: [1, 0, 0, 1, 0, 0, 100, 40] }), solid('circle', [10, 10, 1], { map: [1, 0, 0, 1, 0, 0, 100, 40] })], null);
    // one anchor, however many shapes are drawn on it
    expect(on.points.filter((p) => p.kind === 'anchors')).toEqual([{ at: [100, 40], what: 'anchor', kind: 'anchors' }]);
    expect(snap([97, 38], on)).toMatchObject({ point: [100, 40], what: 'anchor' });
    expect(snap([97, 38], on, ['anchors', 'guides']).point).toEqual([97, 38]);
  });

  it('path nodes: a polygon’s points, a path’s nodes, a line’s ends', () => {
    const t = flatTargets([solid('poly', [0, 0, 30, 0, 15, 20]), solid('path', [], { d: 'M60 0 C70 0 80 10 80 20' }), solid('line', [0, 50, 20, 50])], null);
    expect(snap([16, 22], t)).toMatchObject({ point: [15, 20], what: 'point of poly' });
    expect(snap([78, 21], t)).toMatchObject({ point: [80, 20], what: 'node of path' });
    expect(snap([21, 52], t)).toMatchObject({ point: [20, 50], what: 'end of line' });
    expect(snap([16, 22], t, ['nodes', 'guides']).point).toEqual([16, 22]);
  });

  it('nodes of the glyphs of words set in a font are not offered', () => {
    const glyph = solid('path', [], { d: 'M0 0 L10 0', origin: { definition: 'plain' } as Origin });
    expect(flatTargets([glyph], null, new Set(['plain'])).points).toEqual([]);
    expect(flatTargets([glyph], null).points).toHaveLength(2);
  });

  it('the frame: a canvas, or a footprint at its floor and its top', () => {
    const canvas = frameTargets({ canvas: [48, 24] });
    expect(snap([46, 22], canvas)).toMatchObject({ point: [48, 24], what: 'corner of the canvas' });
    expect(snap([8, 1], canvas)).toMatchObject({ point: [8, 0], what: 'edge of the canvas' });
    expect(snap([46, 22], canvas, ['frame']).point).toEqual([46, 22]);
    expect(canvas.points.every((p) => p.kind === 'frame') && canvas.edges.every((e) => e.kind === 'frame')).toBe(true);
    const footprint = frameTargets({ footprint: [2, 1, 0.5] });
    expect(footprint.points.filter((p) => p.what === 'corner of the footprint')).toHaveLength(8);
    expect(frameTargets({ footprint: [2, 1, 0.5] }, 0.5).points[0]!.at).toEqual(apply(ISO, [0, 0, 0.5]));
    expect(frameTargets({}).points).toEqual([]);
  });
});

describe('alignment guides', () => {
  const corner = flatTargets([solid('rect', [100, 100, 10, 10])], null);

  it('lines a point up with another’s x, the other at the grid, and draws the guide', () => {
    const s = snap([102.4, 20.3], corner, ['grid']);
    expect(s.point).toEqual([100, 20.3]);
    expect(s.what).toBe('aligned with corner of rect');
    expect(s.guides).toEqual([{ a: [100, 100], b: [100, 20.3] }]);
    // with the grid at 5, x at the guide, nearer than the grid's line at
    // 105, and y at the grid's
    const center = flatTargets([solid('circle', [102, 0, 1])], null);
    expect(snap([103, 21], center, [], 5).point).toEqual([102, 20]);
  });

  it('lines up on both axes at once, where two guides cross', () => {
    const t = joined(flatTargets([solid('circle', [40, 0, 1])], null), flatTargets([solid('circle', [0, 60, 1])], null));
    const s = snap([41.5, 58.5], t, ['grid']);
    expect(s.point).toEqual([40, 60]);
    expect(s.guides).toEqual([
      { a: [40, 0], b: [40, 60] },
      { a: [0, 60], b: [40, 60] },
    ]);
  });

  it('gives way to a grid line nearer than the guide', () => {
    // 0.3 from the grid's line at 102, 2.6 from the guide at 100
    expect(snap([102.3, 20], corner).point).toEqual([102, 20]);
  });

  it('draws nothing switched off', () => {
    const s = snap([102.4, 20.3], corner, ['grid', 'guides']);
    expect(s).toEqual({ point: [102.4, 20.3], what: null, at: null, guides: [] });
  });

  it('follows a 3-D floor’s axes, not the screen’s', () => {
    const box = targets([solid('box', [4, 4, 0, 1, 1, 1], { map: ISO })], null, 0);
    const toView = (p: Point) => apply(ISO, [p[0], p[1], 0]);
    const fromView = (v: Point) => onFloor(ISO, v, 0);
    const s = snapPoint([4.05, 0.3], 0, box, toView, fromView, 1, { on: { ...ALL_ON, corners: true } });
    expect(s.point[0]).toBeCloseTo(4, 6);
    expect(s.point[1]).toBeCloseTo(0.3, 6);
    expect(s.guides).toHaveLength(1);
  });
});

describe('a shape’s own points as it moves', () => {
  it('offers a box’s, a rectangle’s, a line’s and a polygon’s', () => {
    expect(moving(solid('box', [0, 0, 0, 2, 1, 1]))).toHaveLength(9);
    expect(moving(solid('rect', [5, 5, 4, 2]))).toContainEqual([2, 1]);
    expect(moving(solid('line', [1, 1, 5, 3]))).toEqual([
      [0, 0],
      [4, 2],
      [2, 1],
    ]);
    expect(moving(solid('poly', [1, 1, 3, 1, 2, 4]))).toEqual([
      [0, 0],
      [2, 0],
      [1, 3],
    ]);
    expect(moving(solid('circle', [0, 0, 1]))).toEqual([[0, 0]]);
  });

  it('snaps the one nearest a target, the shape moving with it', () => {
    const other = flatTargets([solid('rect', [50, 0, 10, 10])], null);
    // a 10 × 10 rect at (38.5, 1): its right edge's corner (48.5, 1) is
    // near the other's corner (50, 0)
    const s = snap([38.5, 1], other, ['grid'], 1, moving(solid('rect', [0, 0, 10, 10])));
    expect(s).toMatchObject({ point: [40, 0], what: 'corner of rect', at: [50, 0] });
  });

  it('lines up its middle with another’s center', () => {
    const other = flatTargets([solid('circle', [100, 0, 2])], null);
    const s = snap([93.5, 50], other, ['grid'], 1, moving(solid('rect', [0, 0, 10, 10])));
    // its middle, 5 along, lines up with the circle's center at x 100
    expect(s.point).toEqual([95, 50]);
    expect(s.guides).toEqual([{ a: [100, 0], b: [100, 50] }]);
  });
});
