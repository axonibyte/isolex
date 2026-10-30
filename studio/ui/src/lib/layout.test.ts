// The panels docked or floating (#119): where a floating panel goes, kept
// within reach in the window, and the layout as stored read back.

import { describe, expect, it } from 'vitest';
import { FLOAT_SIZE, dock, docked, fitted, float, floating, moved, readLayout, resized, within } from './layout';

const WINDOW = { w: 1400, h: 900 };

describe('floating', () => {
  it('takes a panel out of the tabs, where it is put, and docks it back', () => {
    const l = float({ floating: {} }, 'design', { x: 300, y: 80 }, WINDOW);
    expect(l.floating.design).toEqual({ x: 300, y: 80, ...FLOAT_SIZE });
    expect(docked(l)).toEqual(['arrange', 'source', 'tree', 'preview']);
    expect(floating(l)).toEqual(['design']);
    expect(docked(dock(l, 'design'))).toEqual(['design', 'arrange', 'source', 'tree', 'preview']);
  });

  it('moves and resizes, never smaller than a panel can be', () => {
    let l = float({ floating: {} }, 'tree', { x: 100, y: 100 }, WINDOW);
    l = moved(l, 'tree', { x: 50, y: -20 }, WINDOW);
    expect(l.floating.tree).toMatchObject({ x: 150, y: 80 });
    l = resized(l, 'tree', { w: 500, h: 40 }, WINDOW);
    expect(l.floating.tree).toMatchObject({ w: 500, h: 160 });
    // one not floating is left as it is
    expect(moved(l, 'source', { x: 1, y: 1 }, WINDOW)).toBe(l);
  });

  it('keeps a title bar in reach, however far it is dragged', () => {
    expect(within({ x: 5000, y: 5000, w: 360, h: 460 }, WINDOW)).toEqual({ x: 1280, y: 868, w: 360, h: 460 });
    expect(within({ x: -5000, y: -40, w: 360, h: 460 }, WINDOW)).toEqual({ x: -240, y: 0, w: 360, h: 460 });
    // and in a window smaller than the one it was left in
    const l = fitted({ floating: { preview: { x: 1300, y: 850, w: 300, h: 300 } } }, { w: 900, h: 600 });
    expect(l.floating.preview).toMatchObject({ x: 780, y: 568 });
  });
});

describe('the layout stored', () => {
  it('is read back as kept', () => {
    const l = float({ floating: {} }, 'source', { x: 10, y: 20 }, WINDOW);
    expect(readLayout(JSON.stringify(l))).toEqual(l);
  });

  it('leaves out what cannot be read', () => {
    expect(readLayout(null)).toEqual({ floating: {} });
    expect(readLayout('{')).toEqual({ floating: {} });
    expect(readLayout(JSON.stringify({ floating: { gone: { x: 0, y: 0, w: 1, h: 1 }, tree: { x: 'a', y: 0, w: 1, h: 1 } } }))).toEqual({ floating: {} });
  });
});
