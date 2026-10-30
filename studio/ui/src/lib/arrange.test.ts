// Align and Distribute (#114): each shape's bounds, what they are
// aligned to, how far each moves, and its numbers after.

import { describe, expect, it } from 'vitest';
import { align, distribute, extentOf, frameOf, reference, shifted, type Extent } from './arrange';
import { PROJECTION, type Solid } from './geometry';
import type { Map3 } from './backend';

const PLANE: Map3 = [1, 0, 0, 1, 0, 0, 0, 0];
const node = { file: 'a.ilx', start: 0, len: 1 };
const solid = (kind: string, values: number[], more: Partial<Solid> = {}): Solid =>
  ({ kind, node, values, map: PROJECTION, shapes: [0], select: node, copy: false, origin: null, ...more }) as unknown as Solid;
const flat = (kind: string, values: number[], more: Partial<Solid> = {}) => solid(kind, values, { map: PLANE, ...more });
const box = (x: number, y: number, z: number, w = 1, d = 1, h = 1) => extentOf(solid('box', [x, y, z, w, d, h]))!;

describe('a shape’s bounds', () => {
  it('are its 3-D extent, and the axes it can move along', () => {
    expect(extentOf(solid('box', [1, 2, 0, 2, 1, 0.5]))).toEqual({ lo: [1, 2, 0], hi: [3, 3, 0.5], moves: [true, true, true] });
    expect(extentOf(solid('tile', [0, 0, 0.25, 2, 1]))).toEqual({ lo: [0, 0, 0.25], hi: [2, 1, 0.25], moves: [true, true, true] });
    expect(extentOf(solid('cylinder', [0.5, 0.5, 0, 0.4, 0.12]))!.lo.map((v) => +v.toFixed(6))).toEqual([0.1, 0.1, 0]);
    // a shadow stays on the floor, a face rect on its wall
    expect(extentOf(solid('shadow', [0.5, 0.5, 0.48, 0.2]))!.moves).toEqual([true, true, false]);
    expect(extentOf(solid('facerect', [0.25, 1, 0.125, 0.25, 0.5]))).toEqual({ lo: [0.25, 1, 0.125], hi: [0.5, 1, 0.625], moves: [true, false, true] });
    expect(extentOf(solid('facerect', [2, 0.1, 0, 0.5, 0.2], { face: 'right' }))!.moves).toEqual([false, true, true]);
  });

  it('are a flat shape’s box in its plane', () => {
    expect(extentOf(flat('line', [-3, 0, 3, 2]))).toEqual({ lo: [-3, 0], hi: [3, 2], moves: [true, true] });
    expect(extentOf(flat('rect', [-9, -8, 18, 12]))!.hi).toEqual([9, 4]);
    expect(extentOf(flat('circle', [0, 0, 8]))!.lo).toEqual([-8, -8]);
    expect(extentOf(flat('ellipse', [1, 1, 4.5, 8]))!.hi).toEqual([5.5, 9]);
    expect(extentOf(flat('poly', [0, 0, 4, 1, 2, 5]))!.hi).toEqual([4, 5]);
    expect(extentOf(flat('path', [], { d: 'M1,2 L5,-1 L3,4 Z' }))).toEqual({ lo: [1, -1], hi: [5, 4], moves: [true, true] });
    expect(extentOf(flat('text', [0, 0]))).toBeNull();
  });

  it('are an arc’s ends and the quarters it passes', () => {
    // from 300° round through 0° to 60°: its right side, x to the radius
    const e = extentOf(flat('arc', [0, 0, 10, 300, 60]))!;
    expect(e.hi[0]).toBeCloseTo(10);
    expect(e.lo[0]).toBeCloseTo(5);
    expect([e.lo[1], e.hi[1]].map((v) => +v.toFixed(6))).toEqual([-8.660254, 8.660254]);
    // from 0° to 180°: the top half, on a screen whose y runs down
    const top = extentOf(flat('arc', [0, 0, 10, 0, 180]))!;
    expect([top.lo[1], top.hi[1]].map((v) => +v.toFixed(6))).toEqual([-10, 0]);
  });
});

describe('aligning', () => {
  const three = [box(0, 0, 0), box(2, 1, 0.5, 2), box(1, 3, 0, 1, 1, 2)];

  it('is to the first or last chosen, the selection’s bounds, or the frame', () => {
    expect(reference(three, 'first', null)).toBe(three[0]);
    expect(reference(three, 'last', null)).toBe(three[2]);
    expect(reference(three, 'selection', null)).toEqual({ lo: [0, 0, 0], hi: [4, 4, 2], moves: [true, true, true] });
    const frame: Extent = { lo: [0, 0, 0], hi: [5, 5, 5], moves: [true, true, true] };
    expect(reference(three, 'frame', frame)).toBe(frame);
    expect(reference([], 'first', null)).toBeNull();
  });

  it('moves each shape by what meets its edge or middle to the reference’s', () => {
    const ref = reference(three, 'selection', null)!;
    expect(align(three, 0, 'min', ref)).toEqual([0, -2, -1]);
    expect(align(three, 0, 'max', ref)).toEqual([3, 0, 2]);
    expect(align(three, 1, 'center', ref)).toEqual([1.5, 0.5, -1.5]);
    // the first stays where it is when it is the reference
    expect(align(three, 2, 'max', three[0]!)).toEqual([0, -0.5, -1]);
  });

  it('leaves a shape that cannot move that way', () => {
    const wall = extentOf(solid('facerect', [0.15, 1, 0.09, 0.14, 0.2]))!;
    expect(align([three[0]!, wall], 1, 'min', three[1]!)).toEqual([1, 0]);
  });

  it('to the frame: a 3-D icon’s footprint, or a flat icon’s canvas in the shape’s plane', () => {
    expect(frameOf({ footprint: [2, 1, 0.5] }, solid('box', [0, 0, 0, 1, 1, 1]))).toEqual({ lo: [0, 0, 0], hi: [2, 1, 0.5], moves: [true, true, true] });
    // drawn elsewhere than the icon's own coordinates: no footprint there
    expect(frameOf({ footprint: [2, 1, 0.5] }, solid('box', [0, 0, 0, 1, 1, 1], { map: [...PROJECTION.slice(0, 6), 10, 0] as Map3 }))).toBeNull();
    // an anchor at 4 2 on a 24 by 24 canvas
    expect(frameOf({ canvas: [24, 24] }, flat('circle', [0, 0, 1], { map: [1, 0, 0, 1, 0, 0, 4, 2] }))).toEqual({ lo: [-4, -2], hi: [20, 22], moves: [true, true] });
    expect(frameOf({ footprint: [2, 1, 0.5] }, flat('circle', [0, 0, 1]))).toBeNull();
  });
});

describe('distributing', () => {
  it('spaces middles evenly, the outermost two staying', () => {
    const e = [box(0, 0, 0), box(7, 0, 0), box(1, 0, 0), box(3, 0, 0, 2)];
    // middles 0.5, 7.5, 1.5, 4: in order 0.5, 1.5, 4, 7.5, so 0.5, 2.833…, 5.166…, 7.5
    expect(distribute(e, 0, 'centers').map((v) => +v.toFixed(4))).toEqual([0, 0, 1.3333, 1.1667]);
  });

  it('or makes the gaps between them equal', () => {
    const e = [box(0, 0, 0), box(9, 0, 0), box(1.5, 0, 0, 2)];
    // 10 long, 4 of it shapes: two gaps of 3
    expect(distribute(e, 0, 'gaps')).toEqual([0, 0, 2.5]);
  });

  it('needs three that can move that way', () => {
    expect(distribute([box(0, 0, 0), box(5, 0, 0)], 0, 'centers')).toEqual([0, 0]);
    const wall = extentOf(solid('facerect', [0, 1, 0, 1, 1]))!;
    expect(distribute([box(0, 0, 0), wall, box(0, 5, 0)], 1, 'gaps')).toEqual([0, 0, 0]);
  });
});

describe('a 3-D shape moved', () => {
  it('changes only where it is', () => {
    expect(shifted(solid('box', [0, 0, 0, 2, 1, 0.5]), [1, -0.5, 0.25])).toEqual([1, -0.5, 0.25, 2, 1, 0.5]);
    expect(shifted(solid('shadow', [0.5, 0.5, 0.48, 0.48]), [1, 1, 1])).toEqual([1.5, 1.5, 0.48, 0.48]);
    expect(shifted(solid('facerect', [0.15, 1, 0.09, 0.14, 0.14]), [0.1, 5, 0.01])).toEqual([0.25, 1, 0.1, 0.14, 0.14]);
    expect(shifted(solid('facerect', [2, 0.1, 0, 0.5, 0.2], { face: 'right' }), [5, 0.2, 0])).toEqual([2, 0.3, 0, 0.5, 0.2]);
  });
});
