// Measure (#117): the distance, the angle and how far along each axis,
// in tiles or in units, and what it says.

import { describe, expect, it } from 'vitest';
import { measure, said } from './measure';

describe('measuring', () => {
  it('on a floor, in tiles, the angle from +x round toward +y', () => {
    const m = measure([0, 0, 0], [1, 1, 0]);
    expect([m.distance, m.angle, m.delta]).toEqual([1.414214, 45, [1, 1, 0]]);
    expect(said(m)).toBe('1.41 tiles · 45° · Δx 1 Δy 1');
    expect(measure([1, 1, 0], [1, 0, 0]).angle).toBe(270);
    expect(said(measure([0, 0, 0.5], [0, 2, 0.5]))).toBe('2 tiles · 90° · Δx 0 Δy 2');
    // up a face, the height too
    expect(said(measure([0, 0, 0], [1, 0, 0.5]))).toBe('1.12 tiles · 0° · Δx 1 Δy 0 Δz 0.5');
    expect(said(measure([2, 2, 0], [3, 2, 0]))).toBe('1 tile · 0° · Δx 1 Δy 0');
  });

  it('flat, in units, counter-clockwise on a screen whose y runs down', () => {
    expect(measure([0, 0], [10, -10]).angle).toBe(45);
    expect(measure([0, 0], [0, 10]).angle).toBe(270);
    expect(said(measure([2, 2], [5, 6]))).toBe('5 units · 306.9° · Δx 3 Δy 4');
    expect(said(measure([2, 2], [2, 2]))).toBe('0 units · 0° · Δx 0 Δy 0');
  });
});
