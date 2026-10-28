import { describe, expect, it } from 'vitest';
import { fit, onFloor, project, toCanvas, wheel, zoomAt } from './view';

const turn = (deltaY: number, mods: Partial<{ ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }> = {}, deltaX = 0) =>
  wheel({ deltaX, deltaY, ctrlKey: false, metaKey: false, shiftKey: false, ...mods });

describe('wheel', () => {
  it('zooms, in when turned away', () => {
    const a = turn(-100);
    expect(a.kind).toBe('zoom');
    expect(a.kind === 'zoom' && a.factor).toBeGreaterThan(1);
    const b = turn(100);
    expect(b.kind === 'zoom' && b.factor).toBeLessThan(1);
  });
  it.each([
    [{ ctrlKey: true }, 0, 100],
    [{ metaKey: true }, 0, 100],
    [{ shiftKey: true }, 100, 0],
  ])('with %o pans', (mods, dx, dy) => {
    expect(turn(-100, mods)).toEqual({ kind: 'pan', dx, dy });
  });
  it('pans east and west with Shift on a sideways wheel', () => {
    expect(turn(0, { shiftKey: true }, 40)).toEqual({ kind: 'pan', dx: -40, dy: 0 });
  });
});

describe('zoomAt', () => {
  it('keeps the point under the pointer still', () => {
    const c = { k: 2, x: 30, y: -10 };
    const at: [number, number] = [120, 80];
    const before = toCanvas(c, at);
    const after = toCanvas(zoomAt(c, 1.7, at), at);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);
  });
  it('stays within its limits', () => {
    expect(zoomAt({ k: 1, x: 0, y: 0 }, 1000, [0, 0]).k).toBe(64);
    expect(zoomAt({ k: 1, x: 0, y: 0 }, 0.001, [0, 0]).k).toBe(0.25);
  });
});

describe('fit', () => {
  it('centers the box, filling two thirds of the tighter side', () => {
    const c = fit([-20, -10, 40, 20], 800, 600);
    expect(c.k).toBeCloseTo((800 * 0.66) / 40);
    const mid = toCanvas(c, [400, 300]);
    expect(mid[0]).toBeCloseTo(0);
    expect(mid[1]).toBeCloseTo(0);
  });
});

describe('projection', () => {
  it('finds the floor point it drew', () => {
    for (const [x, y] of [
      [0, 0],
      [1.5, -2],
      [3, 0.25],
    ] as const) {
      const [a, b] = onFloor(project(x, y, 0));
      expect(a).toBeCloseTo(x);
      expect(b).toBeCloseTo(y);
    }
  });
  it('finds a raised point at its height', () => {
    const [a, b] = onFloor(project(2, 1, 1.5), 1.5);
    expect(a).toBeCloseTo(2);
    expect(b).toBeCloseTo(1);
  });
});
