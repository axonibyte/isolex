// The pure parts of the drawing tools (#83): the keymap, the flat shapes,
// paths as the pen and the node tool see them, the 3-D writers, and the
// node text the Design tab writes.

import { describe, expect, it } from 'vitest';
import { BOUND, TOOLS, fits, keyName, toolFor } from './keymap';
import { angleOf, angled, arcText, ellipseText, flatStep, lineText, moveFlat, plainData, polyText, rectText, resizeRect, resizeRound, snapAngle, squared, withPoints } from './flat';
import { fewest, insertPoint, moveHandle, movePoint, nearestSide, parse, removePoint, shiftData, toD, toggle, type Path } from './path';
import { cylinderText, faceRectText, poly3dText, segText, shadowText } from './gesture';
import { argSpans, argText, counter, inRepeat, propsOf, withArg } from './kdl';

describe('the keymap', () => {
  it('binds no key twice', () => {
    const keys = [...TOOLS.filter((t) => t).map((t) => t!.key), ...BOUND];
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('finds a tool by its key, with Shift or without', () => {
    expect(toolFor('b', false)?.id).toBe('box');
    expect(toolFor('S', true)?.id).toBe('shadow');
    expect(toolFor('a', false)?.id).toBe('node');
    expect(toolFor('A', true)?.id).toBe('arc');
    expect(toolFor('q', false)).toBeNull();
    expect(keyName('g', true)).toBe('Shift+G');
  });

  it('keeps 3-D tools out of a flat icon', () => {
    const box = TOOLS.find((t) => t?.id === 'box')!;
    const line = TOOLS.find((t) => t?.id === 'line')!;
    expect(fits(box, true)).toBe(false);
    expect(fits(box, false)).toBe(true);
    expect(fits(line, true)).toBe(true);
  });
});

describe('flat shapes', () => {
  it('snap at four units a tile, never finer than a half', () => {
    expect([1, 0.25, 0.125, 0].map(flatStep)).toEqual([4, 1, 0.5, 0]);
  });

  it('keep 15° steps and squares with Shift', () => {
    const b = angled([0, 0], [10, 1]);
    expect(b).toEqual([clean(Math.hypot(10, 1)), 0]);
    expect(angled([0, 0], [10, 1], 1)).toEqual([10, 0]);
    const d = angled([0, 0], [10, 9]);
    expect(d[0]).toBeCloseTo(d[1], 6);
    expect(squared([0, 0], [4, -2])).toEqual([4, -4]);
    expect(squared([0, 0], [-1, 3])).toEqual([-3, 3]);
  });

  it('are written whichever way they were dragged', () => {
    expect(lineText([1, 2], [3, 4])).toBe('line 1 2 3 4');
    expect(rectText([4, 6], [1, 2])).toBe('rect 1 2 3 4');
    expect(ellipseText([0, 0], [8, 4])).toBe('ellipse 4 2 4 2');
    expect(ellipseText([0, 0], [-6, 6])).toBe('circle -3 3 3');
    expect(polyText([
      [0, 0],
      [4, 0],
      [2, 3],
    ])).toBe('poly {\n  pt 0 0\n  pt 4 0\n  pt 2 3\n}');
  });

  it('measure an arc’s angles counter-clockwise, y down', () => {
    expect(angleOf([0, 0], [5, 0])).toBe(0);
    expect(angleOf([0, 0], [0, -5])).toBe(90);
    expect(angleOf([0, 0], [-5, 0])).toBe(180);
    expect(angleOf([0, 0], [0, 5])).toBe(270);
    expect(snapAngle(37.4, false)).toBe(37);
    expect(snapAngle(37.4, true)).toBe(30);
    expect(snapAngle(359.6, false)).toBe(0);
    expect(arcText([0, 0], 11, 300, 420)).toBe('arc 0 0 11 300 420');
  });

  it('move, every x and y or only where it is', () => {
    expect(moveFlat('line', [0, 0, 4, 2], [1, -1])).toEqual([1, -1, 5, 1]);
    expect(moveFlat('rect', [0, 0, 4, 2, 1], [1, 1])).toEqual([1, 1, 4, 2, 1]);
    expect(moveFlat('poly', [0, 0, 4, 0, 2, 3], [1, 2])).toEqual([1, 2, 5, 2, 3, 5]);
  });

  it('resize a rectangle by a corner and a round shape by its radius', () => {
    expect(resizeRect([0, 0, 4, 2, 0], 2, [6, 3])).toEqual([0, 0, 6, 3, 0]);
    expect(resizeRect([0, 0, 4, 2, 0], 0, [6, 3])).toEqual([4, 2, 2, 1, 0]);
    expect(resizeRound('circle', [0, 0, 2], 'round-r', [3, 4])).toEqual([0, 0, 5]);
    expect(resizeRound('ellipse', [0, 0, 2, 1], 'rx', [-5, 9])).toEqual([0, 0, 5, 1]);
    expect(resizeRound('ellipse', [0, 0, 2, 1], 'ry', [-5, 9])).toEqual([0, 0, 2, 9]);
  });

  it('write a polygon’s points anew, only when each is plain', () => {
    const text = 'poly fill="accent" {\n        pt 7 2\n        pt 17 2\n        pt 22 7\n      }';
    expect(withPoints(text, [
      [1, 1],
      [2, 2],
      [3, 3],
      [4, 4],
    ])).toBe('poly fill="accent" {\n        pt 1 1\n        pt 2 2\n        pt 3 3\n        pt 4 4\n      }');
    expect(withPoints('poly {\n  repeat 3 "k" {\n    pt k 0\n  }\n}', [[0, 0]])).toBeNull();
    expect(withPoints('poly {\n  pt "a" 0\n}', [[0, 0]])).toBeNull();
  });

  it('tells path data worked out from expressions', () => {
    expect(plainData('path d="M8,8 L16,16"')).toBe(true);
    expect(plainData('path d="M{x-10},{y} h20"')).toBe(false);
  });
});

describe('paths', () => {
  const teardrop: Path = {
    closed: true,
    knots: [
      { p: [0, -10], in: null, out: null, smooth: false },
      { p: [6, 2], in: [6, -4], out: [6, 6], smooth: true },
      { p: [-6, 2], in: [-6, 6], out: [-6, -4], smooth: true },
    ],
  };

  it('are written as lines and curves, and read back', () => {
    const d = toD(teardrop);
    expect(d).toBe('M0,-10 C0,-10 6,-4 6,2 C6,6 -6,6 -6,2 C-6,-4 0,-10 0,-10 Z');
    expect(parse(d)).toEqual(teardrop);
    expect(toD({ closed: false, knots: [teardrop.knots[0]!, { p: [4, 0], in: null, out: null, smooth: false }] })).toBe('M0,-10 L4,0');
  });

  it('read lines, H and V, relative or not, as corners', () => {
    expect(parse('M8,8 L16,16')?.knots.map((k) => k.p)).toEqual([
      [8, 8],
      [16, 16],
    ]);
    expect(parse('m1 1 h4 v4 H0 z')).toEqual({
      closed: true,
      knots: [
        { p: [1, 1], in: null, out: null, smooth: false },
        { p: [5, 1], in: null, out: null, smooth: false },
        { p: [5, 5], in: null, out: null, smooth: false },
        { p: [0, 5], in: null, out: null, smooth: false },
      ],
    });
    // two subpaths, or an arc, are not one path the node tool shapes
    expect(parse('M8,8 L16,16 M16,8 L8,16')).toBeNull();
    expect(parse('M0,0 A5 5 0 0 0 10,0')).toBeNull();
  });

  it('move a point with its handles', () => {
    const moved = movePoint(teardrop, 1, [8, 2]);
    expect(moved.knots[1]).toEqual({ p: [8, 2], in: [8, -4], out: [8, 6], smooth: true });
    expect(teardrop.knots[1]!.p).toEqual([6, 2]);
  });

  it('turn a smooth point’s handles together, keeping each length, unless Alt', () => {
    const turned = moveHandle(teardrop, 1, 'out', [10, 2], false).knots[1]!;
    expect(turned.out).toEqual([10, 2]);
    expect(turned.in![0]).toBeCloseTo(0, 9);
    expect(turned.in![1]).toBeCloseTo(2, 9);
    expect(turned.smooth).toBe(true);
    const broken = moveHandle(teardrop, 1, 'out', [10, 2], true).knots[1]!;
    expect(broken.in).toEqual([6, -4]);
    expect(broken.smooth).toBe(false);
  });

  it('switch a point between corner and smooth', () => {
    const corner = toggle(teardrop, 1);
    expect(corner.knots[1]).toEqual({ p: [6, 2], in: null, out: null, smooth: false });
    const smooth = toggle(corner, 1).knots[1]!;
    expect(smooth.smooth).toBe(true);
    // in line through the point, from the one before to the one after
    const a = [smooth.in![0] - 6, smooth.in![1] - 2];
    const b = [smooth.out![0] - 6, smooth.out![1] - 2];
    expect(a[0]! * b[1]! - a[1]! * b[0]!).toBeCloseTo(0, 9);
  });

  it('remove a point while enough are kept', () => {
    expect(fewest(teardrop)).toBe(3);
    expect(removePoint(teardrop, 0)).toBeNull();
    const open: Path = { closed: false, knots: teardrop.knots };
    expect(removePoint(open, 0)?.knots).toHaveLength(2);
  });

  it('add a point on a side without changing its shape', () => {
    const near = nearestSide(teardrop, [0, 6])!;
    expect(near.side).toBe(1);
    expect(near.t).toBeCloseTo(0.5, 1);
    const split = insertPoint(teardrop, near.side, 0.5);
    expect(split.knots).toHaveLength(4);
    expect(split.knots[2]!.p[0]).toBeCloseTo(0, 9);
    expect(split.knots[2]!.p[1]).toBeCloseTo(5, 9);
    expect(split.knots[2]!.smooth).toBe(true);
    const square = parse('M0,0 L4,0 L4,4 Z')!;
    expect(insertPoint(square, 0, 0.5).knots[1]!.p).toEqual([2, 0]);
  });

  it('move their data as written, absolute numbers only', () => {
    expect(shiftData('M8,8 L16,16 M16,8 L8,16', [1, 2])).toBe('M9 10 L17 18 M17 10 L9 18');
    expect(shiftData('m1 1 h4 v4 z', [1, 2])).toBe('m2 3 h4 v4 z');
    expect(shiftData('M0 0 A5 5 0 0 0 10 0 H3 V4', [1, 2])).toBe('M1 2 A5 5 0 0 0 11 2 H4 V6');
  });
});

describe('3-D writers', () => {
  it('write each node as the language does', () => {
    expect(cylinderText([0.5, 0.5], 0.25, 0.4, 0.5)).toBe('cylinder 0.5 0.5 0.25 0.4 0.5');
    expect(shadowText([1, 1], 0.5, 0.25)).toBe('shadow 1 1 0.5 0.25');
    expect(segText([0, 0, 0], [2, 1, 0.5])).toBe('seg 0 0 0 2 1 0.5');
    expect(poly3dText([
      [0, 0, 0],
      [1, 0, 0],
      [1, 1, 0.5],
    ])).toBe('poly3d {\n  pt 0 0 0\n  pt 1 0 0\n  pt 1 1 0.5\n}');
    // on the front wall (y 1) from x 0.5 to 1, z 0.1 to 0.3
    expect(faceRectText('front', 1, [1, 0.3], [0.5, 0.1])).toBe('facerect "front" 0.5 1 0.1 0.5 0.2');
    expect(faceRectText('right', 2, [0.25, 0], [0.75, 0.25])).toBe('facerect "right" 2 0.25 0 0.5 0.25');
  });
});

describe('node text', () => {
  it('finds the arguments, quoted or not, before the properties', () => {
    const t = 'facerect "front" "0.1 + i * 2" 1 \\\n  0.09 0.11 0.11 fill="ink" opacity=0.55';
    expect(argSpans(t).map((a) => t.slice(a.start, a.end))).toEqual(['"front"', '"0.1 + i * 2"', '1', '0.09', '0.11', '0.11']);
    expect(argText(t, 1)).toBe('0.1 + i * 2');
    expect(withArg('box 0 0 0 2 1 0.5', 3, '4')).toBe('box 0 0 0 4 1 0.5');
    expect(argSpans('poly fill="ink" {\n  pt 0 0\n}')).toEqual([]);
  });

  it('reads the properties written before the children', () => {
    expect(propsOf('rect 3 3 18 18 rx=3 stroke="ink-muted" width=1.5 dash="3 2.5"')).toEqual({ rx: '3', stroke: 'ink-muted', width: '1.5', dash: '3 2.5' });
    expect(propsOf('poly fill="ink" {\n  pt 0 0 fill="no"\n}')).toEqual({ fill: 'ink' });
  });

  it('wraps a node in a repeat, an argument stepping', () => {
    expect(inRepeat('tile 0 0 0 1 1 fill="body-top"', '    ', 0, 0, 4, 1.25)).toBe('repeat 4 "i" {\n      tile "0 + i * 1.25" 0 0 1 1 fill="body-top"\n    }');
    expect(counter('line i 0 j 0')).toBe('k');
    // a node of more lines keeps its own, one step in
    expect(inRepeat('box 0 0 0 1 1 1 \\\n      fills="virt"', '    ', 1, 0, 2, 2)).toBe('repeat 2 "i" {\n      box 0 "0 + i * 2" 0 1 1 1 \\\n        fills="virt"\n    }');
  });
});

const clean = (v: number) => Math.round(v * 1e6) / 1e6;
