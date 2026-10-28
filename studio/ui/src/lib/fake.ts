// A stand-in for the studio's core, outside the app (in a plain browser,
// in vitest, in Playwright): the compiler's own sample library, already
// compiled, which takes typing in its source and nothing else.

import sampleSource from '../../../../tests/fixtures/sample.ilx?raw';
import sampleIr from '../../../../tests/fixtures/sample.ir.golden.json';
import type { IrDocument } from './isolex/ir';
import type { Answer, Backend, Graph, Notice } from './backend';

const FILE = 'sample.ilx';
const ir = sampleIr as unknown as IrDocument;

/** Each icon, under the definition that emits it: a variant's (`-down`)
    under its icon's. */
function graph(): Graph {
  const names = ir.icons.map((i) => i.name);
  const own = names.filter((n) => !names.some((m) => m !== n && n.startsWith(m + '-')));
  return {
    definitions: own.map((name) => ({
      name,
      kind: 'Icon' as const,
      file: FILE,
      span: { file: 0, start: Math.max(0, sampleSource.indexOf(`icon "${name}"`)), len: 1 },
      emits: names.filter((n) => n === name || n.startsWith(name + '-')),
    })),
    edges: [],
  };
}

export function fake(): Backend {
  let text = sampleSource;
  let revision = 0;
  let saved = text;
  const undo: string[] = [];
  const redo: string[] = [];
  const ears = new Set<(n: Notice) => void>();
  const tell = (n: Notice) => ears.forEach((e) => e(n));
  const state = () =>
    tell({
      kind: 'library',
      root: 'sample',
      revision,
      modified: text !== saved,
      files: [FILE],
      history: undo.map(() => 'Type in sample.ilx'),
      can_undo: undo.length > 0,
      can_redo: redo.length > 0,
    });
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  return {
    about: async () => null,
    pickFolder: async () => 'sample',
    open: async () => {
      state();
      tell({ kind: 'graph', graph: graph() });
      tell({ kind: 'compiled', generation: 1, revision, errors: [], warnings: [], palette: null, icons: ir.icons.map((i) => i.name) });
      for (const icon of ir.icons) tell({ kind: 'icon', name: icon.name, ir: icon });
    },
    view: async () => {},
    source: async (file) => (file === FILE ? { revision, text } : null),
    ask: async (ask): Promise<Answer> => {
      if (ask.kind !== 'type' || ask.file !== FILE) {
        return { kind: 'refused', reason: 'The sample library outside the app takes typing only.' };
      }
      const b = encoder.encode(text);
      if (ask.at + ask.len > b.length) return { kind: 'refused', reason: 'there is no such text to replace' };
      undo.push(text);
      redo.length = 0;
      text = decoder.decode(b.slice(0, ask.at)) + ask.text + decoder.decode(b.slice(ask.at + ask.len));
      revision++;
      state();
      return { kind: 'applied', revision };
    },
    choose: async () => ({ kind: 'refused', reason: 'nothing to choose' }),
    beginGesture: async () => {},
    endGesture: async () => {},
    undo: async () => {
      const t = undo.pop();
      if (t === undefined) return null;
      redo.push(text);
      text = t;
      revision++;
      state();
      return 'Type in sample.ilx';
    },
    redo: async () => {
      const t = redo.pop();
      if (t === undefined) return null;
      undo.push(text);
      text = t;
      revision++;
      state();
      return 'Type in sample.ilx';
    },
    save: async () => {
      saved = text;
      state();
    },
    reload: async () => {},
    listen: async (hear) => {
      ears.add(hear);
      return () => ears.delete(hear);
    },
  };
}
