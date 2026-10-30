// The Tree's focus view (#113): its layout over the graph fixture with
// every kind of edge, kept as a golden; the depth limiting what it shows;
// and what a hovered node says.

import { describe, expect, it } from 'vitest';
import graphGolden from '../../../../tests/fixtures/graph.golden.json';
import type { Graph } from './backend';
import { clampDepth, focus, key, said } from './focus';

const graph = graphGolden as unknown as Graph;

/** A graph from `from kind to` lines, every name defined but `gone`. */
function made(...lines: string[]): Graph {
  const edges = lines.map((l) => {
    const [from, kind, to] = l.split(' ');
    return { from: from!, kind: kind as Graph['edges'][number]['kind'], to: to!, at: { file: 0, start: 0, len: 0 } };
  });
  const names = [...new Set(edges.flatMap((e) => [e.from, e.to]))].filter((n) => n !== 'gone');
  return { definitions: names.map((name) => ({ name, kind: 'Icon', file: 'a.ilx', span: { file: 0, start: 0, len: 0 }, emits: [name] })), edges };
}

/** Each column's names, in order. */
const columns = (f: ReturnType<typeof focus>) => {
  const out: Record<number, string[]> = {};
  for (const n of f.nodes) (out[n.column] ??= [])[n.row] = n.name;
  return out;
};

describe('the focus view', () => {
  it('lays out every kind of reference, as its golden has it', async () => {
    const views: [string, number][] = [
      ['server', 2],
      ['switch', 3],
      ['down', 3],
      ['led', 3],
      ['mono', 1],
    ];
    const laid = Object.fromEntries(views.map(([name, depth]) => [`${name}, depth ${depth}`, focus(graph, name, depth)]));
    await expect(`${JSON.stringify(laid, null, 2)}\n`).toMatchFileSnapshot('../../../../tests/fixtures/graph.focus.golden.json');
  });

  it('shows what each side reaches within the depth, and no further', () => {
    expect(columns(focus(graph, 'server', 1))).toEqual({ [-1]: ['chassis', 'state', 'led-row'], 0: ['server'] });
    expect(columns(focus(graph, 'server', 2))).toEqual({ [-2]: ['down', 'led'], [-1]: ['chassis', 'state', 'led-row'], 0: ['server'] });
    // down is badged by an axis, which server opts into
    expect(columns(focus(graph, 'down', 1))).toEqual({ 0: ['down'], 1: ['state'] });
    expect(columns(focus(graph, 'down', 2))).toEqual({ 0: ['down'], 1: ['state'], 2: ['server'] });
    expect(columns(focus(graph, 'down', 3))).toEqual(columns(focus(graph, 'down', 2)));
    // a depth asked for is one the view has
    expect([0, 1, 2.4, 3, 7, NaN].map((d) => clampDepth(d, 2))).toEqual([1, 1, 2, 3, 3, 2]);
  });

  it('lists a definition once, at its nearest step, and stops at a cycle', () => {
    // b is a step away, and two through c; b and c refer back to the middle
    const f = focus(made('a insert b', 'a extends c', 'c insert b', 'b extends a'), 'a', 3);
    expect(columns(f)).toEqual({ [-1]: ['b', 'c'], 0: ['a'], 1: ['b'], 2: ['c'] });
    // every line runs between neighboring columns, from the one referred to
    expect(f.links.map((l) => `${l.left} <-${l.kind}- ${l.right}`)).toEqual(['-1:b <-insert- 0:a', '-1:c <-extends- 0:a', '0:a <-extends- 1:b', '1:b <-insert- 2:c']);
  });

  it('says how each node relates to the middle', () => {
    const f = focus(graph, 'server', 2);
    const node = (column: number, name: string) => f.nodes.find((n) => key(n) === `${column}:${name}`)!;
    expect(said(node(0, 'server'))).toBe('Being edited');
    expect(said(node(-1, 'led-row'))).toBe('server inserts it');
    expect(said(node(-1, 'state'))).toBe('server opts into it');
    expect(said(node(-2, 'down'))).toBe('state badges it, in variant down');
    expect(said(focus(graph, 'chassis', 1).nodes.find((n) => n.name === 'switch')!)).toBe('Extends chassis');
    expect(said(focus(graph, 'down', 1).nodes[1]!)).toBe('Badges down, in variant down');
    expect(said(focus(made('a insert gone'), 'a', 1).nodes[1]!)).toBe('a inserts it; nothing defines it');
  });
});
