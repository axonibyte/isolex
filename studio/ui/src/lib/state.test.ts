import { describe, expect, it } from 'vitest';
import { definitionOf, empty, reduce } from './state.svelte';
import type { Diag, Graph, Notice, Origin } from './backend';
import type { IrIcon } from './isolex/ir';

const icon = (name: string): IrIcon => ({ name, viewbox: [0, 0, 1, 1], shapes: [] }) as unknown as IrIcon;
const diag: Diag = { file: 'a.ilx', line: 1, col: 1, message: 'bad' };
const origin = (start: number): Origin => ({
  node: { file: 'a.ilx', start, len: 3 },
  kind: 'box',
  definition: 'a',
  part: null,
  sites: [],
  map: [1, 0, 0, 1, 0, 0, 0, 0],
  values: [],
});
const drawn = (name: string, start = 0): Notice => ({ kind: 'icon', name, ir: icon(name), sources: [origin(start)] });
const compiled = (generation: number): Notice => ({
  kind: 'compiled',
  generation,
  revision: generation,
  errors: [],
  warnings: [],
  palette: null,
  tokens: ['ink', 'outline'],
  stroke_widths: [1.2],
  icons: [`g${generation}`],
});
const library = (modified: boolean): Notice => ({
  kind: 'library',
  root: 'r',
  revision: 1,
  modified,
  files: ['a.ilx'],
  history: [],
  future: [],
  can_undo: false,
  can_redo: false,
});

describe('reduce', () => {
  it('keeps the newest generation', () => {
    const v = [compiled(2), compiled(1)].reduce(reduce, empty());
    expect(v.compiled?.generation).toBe(2);
    expect(reduce(v, compiled(3)).compiled?.icons).toEqual(['g3']);
    // and what paint is chosen from
    expect(v.lexicon).toEqual({ tokens: ['ink', 'outline'], widths: [1.2] });
  });

  it('keeps a broken icon’s last drawing, and mends it', () => {
    let v = reduce(empty(), drawn('a'));
    v = reduce(v, { kind: 'broken', name: 'a', diagnostics: [diag], last: null });
    expect(v.icons.a?.name).toBe('a');
    expect(v.broken.a).toEqual([diag]);
    v = reduce(v, drawn('a'));
    expect(v.broken.a).toBeUndefined();
  });

  it('keeps each drawing’s sources with it', () => {
    let v = reduce(empty(), drawn('a', 5));
    expect(v.sources.a).toEqual([origin(5)]);
    v = reduce(v, { kind: 'broken', name: 'a', diagnostics: [diag], last: { ir: icon('a'), sources: [origin(9)] } });
    expect(v.sources.a).toEqual([origin(9)]);
    v = reduce(v, { kind: 'broken', name: 'a', diagnostics: [diag], last: null });
    expect(v.sources.a).toEqual([origin(9)]);
  });

  it('forgets an icon that is gone', () => {
    let v = reduce(empty(), drawn('a'));
    v = reduce(v, { kind: 'broken', name: 'a', diagnostics: [diag], last: null });
    v = reduce(v, { kind: 'gone', name: 'a' });
    expect(v.icons).toEqual({});
    expect(v.sources).toEqual({});
    expect(v.broken).toEqual({});
  });

  it('is out of sync until there is nothing unsaved, or a reload', () => {
    let v = reduce(empty(), library(true));
    v = reduce(v, { kind: 'out-of-sync', files: ['a.ilx'] });
    v = reduce(v, { kind: 'out-of-sync', files: ['a.ilx', 'b.ilx'] });
    expect(v.outOfSync).toEqual(['a.ilx', 'b.ilx']);
    expect(reduce(v, library(true)).outOfSync).toHaveLength(2);
    expect(reduce(v, library(false)).outOfSync).toEqual([]);
    expect(reduce(v, { kind: 'reloaded', files: ['a.ilx'] }).outOfSync).toEqual([]);
  });
});

describe('definitionOf', () => {
  const graph = {
    definitions: [
      { name: 'rack', kind: 'Icon', file: 'a.ilx', span: { file: 0, start: 0, len: 1 }, emits: ['rack', 'rack-down'] },
      { name: 'rack-down', kind: 'Icon', file: 'b.ilx', span: { file: 1, start: 0, len: 1 }, emits: ['rack-down'] },
    ],
    edges: [],
  } as unknown as Graph;

  it('prefers the definition named as the icon', () => {
    expect(definitionOf(graph, 'rack-down')?.file).toBe('b.ilx');
  });
  it('falls back to the one that emits it', () => {
    const g = { ...graph, definitions: graph.definitions.slice(0, 1) };
    expect(definitionOf(g, 'rack-down')?.name).toBe('rack');
  });
  it('is none for nothing', () => {
    expect(definitionOf(null, 'rack')).toBeNull();
    expect(definitionOf(graph, null)).toBeNull();
    expect(definitionOf(graph, 'switch')).toBeNull();
  });
});
