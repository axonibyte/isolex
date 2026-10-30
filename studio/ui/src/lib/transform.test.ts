// Transform (#115): each step as the language writes it, wrapped round
// what it draws, added to a transform of its kind, about the middle of
// what is selected.

import { describe, expect, it } from 'vitest';
import { composed, head, idle, middle, pivotHandles, pivotStep, runs, stepped, wrapped } from './transform';
import { PROJECTION, type Solid } from './geometry';

const node = { file: 'a.ilx', start: 0, len: 1 };
const solid = (kind: string, values: number[], map = PROJECTION): Solid =>
  ({ kind, node, values, map, shapes: [0], select: node, copy: false, origin: null }) as unknown as Solid;

describe('a step', () => {
  it('is written as the language writes it, about its origin', () => {
    const o: [number, number] = [0, 60.004];
    expect(head({ kind: 'rotate', deg: -15 }, o)).toBe('transform rotate=-15 origin="0 60"');
    expect(head({ kind: 'scale', sx: 0.5, sy: 0.5 }, o)).toBe('transform scale=0.5 origin="0 60"');
    expect(head({ kind: 'scale', sx: 2, sy: 1 }, o)).toBe('transform scale="2 1" origin="0 60"');
    // skew-y before skew-x, the order the language applies them in
    expect(head({ kind: 'skew', x: 10, y: -5 }, o)).toBe('transform skew-y=-5 skew-x=10 origin="0 60"');
    expect(head({ kind: 'skew', x: 10, y: 0 }, o)).toBe('transform skew-x=10 origin="0 60"');
    expect(head({ kind: 'mirror', how: 'down' }, o)).toBe('transform mirror="down" origin="0 60"');
    // in tiles: a quarter-turn the language takes
    expect(head({ kind: 'turn', deg: -90 }, [2, 2])).toBe('turn 270 origin="2 2"');
    expect(head({ kind: 'mirror-diagonal' }, [1.5, 0])).toBe('mirror-diagonal origin="1.5 0"');
  });

  it('that changes nothing is none', () => {
    expect([idle({ kind: 'rotate', deg: 360 }), idle({ kind: 'scale', sx: 1, sy: 1 }), idle({ kind: 'skew', x: 0, y: 0 }), idle({ kind: 'rotate', deg: 5 })]).toEqual([true, true, true, false]);
  });
});

describe('wrapping', () => {
  it('indents what it holds a level in, its comments and blank lines kept', () => {
    const body = 'box 0 0 0 1 1 1\n    // the lid\n\n    tile 0 0 1 1 1';
    expect(wrapped(body, '    ', 'transform rotate=15 origin="0 0"')).toBe(
      'transform rotate=15 origin="0 0" {\n      box 0 0 0 1 1 1\n      // the lid\n\n      tile 0 0 1 1 1\n    }',
    );
  });

  it('takes neighbors together, in source order, and others alone', () => {
    const a = [1, 2, 3, 4];
    const b = [9];
    const found = [
      { siblings: a, index: 3, at: 40 },
      { siblings: a, index: 0, at: 10 },
      { siblings: b, index: 0, at: 5 },
      { siblings: a, index: 1, at: 20 },
    ];
    expect(runs(found).map((r) => r.map((f) => f.at))).toEqual([[5], [10, 20], [40]]);
  });
});

describe('a step added to the transform selected', () => {
  it('turns it further, or scales it further, written as one number', () => {
    expect(composed('transform', 'transform rotate=350 origin="0 60" {\n  box 0 0 0 1 1 1\n}', { kind: 'rotate', deg: 15 })).toEqual({ key: 'rotate', value: 5 });
    expect(composed('transform', 'transform scale=0.5 origin="0 0" strokes="fixed" {}', { kind: 'scale', sx: 0.5, sy: 1 })).toEqual({ key: 'scale', value: '0.25 0.5' });
    expect(composed('turn', 'turn 270 origin="2 2" {}', { kind: 'turn', deg: 180 })).toEqual({ index: 0, value: 90 });
  });

  it('is a new transform round it when that would change what it says', () => {
    // another kind of step, an expression, a block of steps, a rotate in a box with more
    expect(composed('transform', 'transform rotate=15 {}', { kind: 'scale', sx: 2, sy: 2 })).toBeNull();
    expect(composed('transform', 'transform rotate="a * 2" {}', { kind: 'rotate', deg: 15 })).toBeNull();
    expect(composed('transform', 'transform origin="0 0" {\n  steps { rotate 15 }\n}', { kind: 'rotate', deg: 15 })).toBeNull();
    expect(composed('transform', 'transform rotate=15 skew-x=5 {}', { kind: 'rotate', deg: 15 })).toBeNull();
    expect(composed('turn', 'turn "q * 90" {}', { kind: 'turn', deg: 90 })).toBeNull();
    expect(composed('box', 'box 0 0 0 1 1 1', { kind: 'rotate', deg: 15 })).toBeNull();
  });
});

describe('the middle', () => {
  it('of 3-D shapes is on the screen round every corner, or in tiles their footprint', () => {
    const box = solid('box', [1.5, 1.5, 0, 1, 1, 1]);
    expect(middle([box], false)).toEqual([0, 60]);
    expect(middle([box, solid('tile', [3, 3, 0, 1, 1])], true)).toEqual([2.75, 2.75]);
  });

  it('of flat shapes is on the screen, through their plane', () => {
    expect(middle([solid('circle', [12, 12, 4], [1, 0, 0, 1, 0, 0, 0, 0])], false)).toEqual([12, 12]);
    expect(middle([solid('rect', [0, 0, 4, 2], [1, 0, 0, 1, 0, 0, 10, 5])], false)).toEqual([12, 6]);
    // nothing flat turns in tiles
    expect(middle([solid('rect', [0, 0, 4, 2], [1, 0, 0, 1, 0, 0, 0, 0])], true)).toBeNull();
  });
});

describe('the turn handles', () => {
  const box = { lo: [0, 0] as [number, number], hi: [40, 20] as [number, number] };

  it('rotate at the corners and scale at the middles of the sides', () => {
    const hs = pivotHandles(box);
    expect(hs.filter((h) => h.kind === 'rotate').map((h) => h.at)).toEqual([
      [0, 0],
      [40, 0],
      [40, 20],
      [0, 20],
    ]);
    expect(hs.filter((h) => h.kind === 'scale').map((h) => [h.id, h.at])).toEqual([
      ['scale-x0', [0, 10]],
      ['scale-x1', [40, 10]],
      ['scale-y0', [20, 0]],
      ['scale-y1', [20, 20]],
    ]);
  });

  it('come, dragged, to a rotation clockwise to 15°, or to 1° with Shift', () => {
    const c: [number, number] = [20, 10];
    // a quarter round, clockwise on a screen whose y runs down
    expect(pivotStep('rot-1', c, [40, 10], [20, 30], false)).toEqual({ kind: 'rotate', deg: 90 });
    expect(pivotStep('rot-1', c, [40, 10], [20, -10], false)).toEqual({ kind: 'rotate', deg: -90 });
    // 22° rounds to 15°, or stays 22° with Shift
    const to: [number, number] = [20 + 20 * Math.cos((22 * Math.PI) / 180), 10 + 20 * Math.sin((22 * Math.PI) / 180)];
    expect(pivotStep('rot-1', c, [40, 10], to, false)).toEqual({ kind: 'rotate', deg: 15 });
    expect(pivotStep('rot-1', c, [40, 10], to, true)).toEqual({ kind: 'rotate', deg: 22 });
    expect(pivotStep('rot-1', c, [40, 10], [0, 10], false)).toEqual({ kind: 'rotate', deg: 180 });
  });

  it('come to a scale across or down, to 5%, both ways with Shift', () => {
    const c: [number, number] = [20, 10];
    expect(pivotStep('scale-x1', c, [40, 10], [61, 12], false)).toEqual({ kind: 'scale', sx: 2.05, sy: 1 });
    expect(pivotStep('scale-y0', c, [20, 0], [20, 5], false)).toEqual({ kind: 'scale', sx: 1, sy: 0.5 });
    expect(pivotStep('scale-x0', c, [0, 10], [10, 10], true)).toEqual({ kind: 'scale', sx: 0.5, sy: 0.5 });
    // never to nothing, nor through to the other side
    expect(pivotStep('scale-x1', c, [40, 10], [0, 10], false)).toEqual({ kind: 'scale', sx: 0.05, sy: 1 });
  });

  it('preview the box drawn through the step', () => {
    const r = stepped(box, [20, 10], { kind: 'rotate', deg: 90 }).map(([x, y]) => [+x.toFixed(6), +y.toFixed(6)]);
    expect(r).toEqual([
      [30, -10],
      [30, 30],
      [10, 30],
      [10, -10],
      [30, -10],
    ]);
    expect(stepped(box, [20, 10], { kind: 'scale', sx: 2, sy: 1 })[2]).toEqual([60, 20]);
  });
});
