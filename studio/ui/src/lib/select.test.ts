import { describe, expect, it } from 'vitest';
import sampleSource from '../../../../tests/fixtures/sample.ilx?raw';
import sampleStudio from '../../../../tests/fixtures/sample.studio.golden.json';
import type { Graph, NodeAt, Origin, Outline, Row } from './backend';
import { atPath, byteOf, dropAbove, find, lineOf, marks, move, pick, reorder, rowAt, same, source, within } from './select';

// what the core says of the sample (studio-core/tests/sample_for_ui.rs)
const studio = sampleStudio as unknown as { graph: Graph; outlines: Record<string, Outline>; sources: Record<string, Origin[]> };
const text = (n: NodeAt) => new TextDecoder().decode(new TextEncoder().encode(sampleSource).slice(n.start, n.start + n.len));
const nodeOf = (name: string): NodeAt => {
  const d = studio.graph.definitions.find((d) => d.name === name)!;
  return { file: d.file, start: d.span.start, len: d.span.len };
};
const at = (file: string, start: number, len: number): NodeAt => ({ file, start, len });

describe('within', () => {
  it('is inside, in the same file', () => {
    expect(within(at('a', 5, 2), at('a', 5, 2))).toBe(true);
    expect(within(at('a', 5, 2), at('a', 4, 4))).toBe(true);
    expect(within(at('a', 5, 4), at('a', 4, 4))).toBe(false);
    expect(within(at('a', 5, 2), at('b', 0, 99))).toBe(false);
  });
});

describe('pick', () => {
  const l3 = studio.sources['switch-l3']!;

  it('selects a shape written in the definition being edited', () => {
    const path = l3.find((o) => text(o.node).startsWith('path'))!;
    expect(text(pick(path, nodeOf('switch-l3'))!)).toMatch(/^path d=/);
  });

  it('selects nothing drawn from elsewhere, and says where from', () => {
    const box = l3.find((o) => text(o.node).startsWith('box'))!;
    expect(pick(box, nodeOf('switch-l3'))).toBeNull();
    expect(source(box, studio.graph)).toBe('rack-unit');
    // opened in its own definition, it is selectable
    expect(text(pick(box, nodeOf('rack-unit'))!)).toMatch(/^box 0 0 0 2 1/);
  });

  it('selects the shape in a repeat, not the repeat', () => {
    const port = l3.find((o) => text(o.node).startsWith('facerect'))!;
    expect(text(pick(port, nodeOf('switch'))!)).toMatch(/^facerect "front"/);
  });

  it('selects what an insert drew as the insert', () => {
    const poe = studio.sources['switch-poe']!;
    const disk = poe.find((o) => o.definition === 'disk-stack')!;
    expect(text(pick(disk, nodeOf('switch-poe'))!)).toMatch(/^insert "disk-stack"/);
    expect(source(disk, studio.graph)).toBe('switch-poe');
  });

  it('selects nothing with nothing open', () => {
    expect(pick(studio.sources['switch-l3']![0]!, null)).toBeNull();
  });
});

describe('marks', () => {
  it('marks every copy a repeat draws, by selecting the shape or the repeat', () => {
    const poe = studio.sources['switch-poe']!;
    const editing = nodeOf('switch-poe');
    const o = studio.outlines['switch-poe']!;
    const repeat = o.parts[0]!.shapes[0]!;
    const copies = marks(poe, editing, repeat.node);
    // `when="i < 7"` leaves one of the eight out
    expect(copies.size).toBe(7);
    expect(marks(poe, editing, repeat.children[0]!.node)).toEqual(copies);
  });

  it('marks a part’s shapes by selecting the part', () => {
    const poe = studio.sources['switch-poe']!;
    const part = studio.outlines['switch-poe']!.parts[2]!;
    const all = marks(poe, nodeOf('switch-poe'), part.node);
    expect(all.size).toBeGreaterThan(8);
    for (const i of all) expect(within(pick(poe[i]!, nodeOf('switch-poe'))!, part.node)).toBe(true);
  });

  it('marks nothing with nothing selected', () => {
    expect(marks(studio.sources['switch-poe']!, nodeOf('switch-poe'), null).size).toBe(0);
  });
});

describe('the outline', () => {
  const o = studio.outlines['switch-poe']!;

  it('finds a part and a row, and finds them again by path', () => {
    const at = o.parts[2]!.shapes[5]!.children[1]!;
    const f = find(o, at.node)!;
    expect(f.path).toBe('2/5/1');
    expect(f.row?.kind).toBe('circle');
    expect(f.part.name).toBe('everything-else');
    expect(same(atPath(o, '2/5/1')!.row!.node, at.node)).toBe(true);
    expect(find(o, o.parts[1]!.node)).toMatchObject({ path: '1', row: null });
    expect(find(o, { file: 'sample.ilx', start: 0, len: 1 })).toBeNull();
    expect(atPath(o, '9')).toBeNull();
  });

  it('finds the innermost row around a caret', () => {
    const circle = o.parts[2]!.shapes[5]!.children[1]!;
    expect(rowAt(o, 'sample.ilx', circle.node.start + 3)!.row!.kind).toBe('circle');
    const at = o.parts[2]!.shapes[5]!;
    // between the at's children: the at
    const gap = at.children[0]!.node.start - 1;
    expect(rowAt(o, 'sample.ilx', gap)!.row!.kind).toBe('at');
    expect(rowAt(o, 'sample.ilx', o.node.start)).toBeNull();
    expect(rowAt(o, 'other.ilx', circle.node.start)).toBeNull();
  });
});

describe('reordering', () => {
  const rows = ['a', 'b', 'c', 'd'].map((k, i): Row => ({ kind: k, note: '', node: at('f', i * 10, 5), children: [] }));
  const name = (n: NodeAt | null) => (n ? rows.find((r) => same(r.node, n))!.kind : null);
  const asked = (a: ReturnType<typeof reorder>) => (a ? [name(a.node), name(a.before)] : null);

  it.each([
    ['raise', 1, ['b', 'd']],
    ['raise', 2, ['c', null]],
    ['raise', 3, null],
    ['lower', 2, ['c', 'b']],
    ['lower', 0, null],
    ['top', 0, ['a', null]],
    ['top', 3, null],
    ['bottom', 3, ['d', 'a']],
    ['bottom', 0, null],
  ] as const)('%s from %i', (how, i, want) => {
    expect(asked(move(rows, i, how))).toEqual(want);
  });

  // the panel lists d c b a, topmost first
  it.each([
    [0, 2, ['a', 'd']], // a dropped above c: b c a d
    [3, 1, ['d', 'c']], // d dropped above b: a b d c
    [1, 0, null], // b above a, where it is
    [2, 2, null], // c onto itself
  ])('dropping %i above %i', (from, onto, want) => {
    expect(asked(dropAbove(rows, from, onto))).toEqual(want);
  });
});

describe('offsets', () => {
  it('counts lines by bytes, and bytes by characters', () => {
    // a, newline, two bytes of e-acute, newline, b: byte 4 is the second newline
    expect(lineOf('a\né\nb', 5)).toBe(3);
    expect(lineOf('a\né\nb', 4)).toBe(2);
    expect(byteOf('éa', 1)).toBe(2);
  });
});
