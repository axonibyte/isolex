// How a row of the Tree tab relates to what is open (#71), over the
// sample's graph and tree as the core gives them.

import { describe, expect, it } from 'vitest';
import sampleStudio from '../../../../tests/fixtures/sample.studio.golden.json';
import type { Graph, Tree, TreeRow } from './backend';
import { chain, relation } from './tree';

const { graph, tree } = sampleStudio as unknown as { graph: Graph; tree: Tree };

function find(rows: TreeRow[], name: string): TreeRow | null {
  for (const r of rows) {
    if (r.name === name) return r;
    const f = find(r.children, name);
    if (f) return f;
  }
  return null;
}
const row = (name: string) => find(tree.icons, name)!;

describe('relation', () => {
  it('follows extends both ways, and finds what two share', () => {
    expect(chain(graph, 'switch-l3')).toEqual(['switch-l3', 'switch-l2', 'switch', 'rack-unit']);
    expect(relation(graph, row('switch-l3'), 'switch-l2', 'switch-l2')).toBe('Extends switch-l2');
    expect(relation(graph, row('switch-l3'), 'rack-unit', null)).toBe('Extends rack-unit, through switch-l2, switch');
    expect(relation(graph, row('rack-unit'), 'switch-l3', 'switch-l3')).toBe('switch-l3 extends it');
    expect(relation(graph, row('switch-poe'), 'switch-l3', 'switch-l3')).toBe('Shares switch-l2 with switch-l3');
  });

  it('names a variant’s axis, and finds badges, from an icon or its axis', () => {
    expect(relation(graph, row('switch-poe-down'), 'switch-l2', 'switch-l2')).toBe('Variant down of axis state, of switch-poe: extends switch-l2');
    expect(relation(graph, row('state-down'), 'switch-poe', 'switch-poe')).toBe('switch-poe badges it');
    expect(relation(graph, row('switch-poe'), 'state-down', 'state-down')).toBe('Badges state-down');
    // state-planned is badged only by the axis switch-poe opts into
    expect(relation(graph, row('state-planned'), 'switch-poe', 'switch-poe')).toBe('switch-poe badges it');
    expect(relation(graph, row('state-planned'), 'switch-l3', 'switch-l3')).toBe('No relation to switch-l3');
  });

  it('says which row is being edited, and nothing with nothing open', () => {
    expect(relation(graph, row('switch-poe'), 'switch-poe', 'switch-poe')).toBe('Being edited');
    expect(relation(graph, row('switch-poe-down'), 'state', 'switch-poe-down')).toBe('Being edited');
    expect(relation(graph, row('switch-poe'), null, null)).toBe('');
  });
});
