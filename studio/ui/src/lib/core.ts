// The studio's core outside the app (in a plain browser, in vitest, in
// Playwright): studio-core built as a WebAssembly module
// (studio-core/wasm), over the compiler's sample library kept in memory.
// It edits, undoes and compiles as the app does; it has no disk, so
// saving keeps the library as it is, and nothing changes outside it.
// `npm run core` builds the module into this folder.

import moduleUrl from './studio-core.wasm?url';
import sampleSource from '../../../../tests/fixtures/sample.ilx?raw';
import type { Answer, Backend, Drawing, Notice, Outline, Source } from './backend';

interface Exports {
  memory: WebAssembly.Memory;
  studio_alloc(len: number): number;
  studio_free(ptr: number, len: number): void;
  studio_call(ptr: number, len: number): bigint;
}

/** The module's bytes: fetched in a browser, read from disk in node. */
async function bytes(): Promise<BufferSource> {
  if (typeof process !== 'undefined' && process.versions?.node) {
    const { readFile } = await import('node:fs/promises');
    const { resolve } = await import('node:path');
    return readFile(resolve(process.cwd(), 'ui/src/lib/studio-core.wasm'));
  }
  return (await fetch(moduleUrl)).arrayBuffer();
}

/** The core, with a library to open: the sample, unless given another
    as files. */
export async function core(files: [string, string][] = [['sample.ilx', sampleSource]]): Promise<Backend> {
  const { instance } = await WebAssembly.instantiate(await bytes(), {});
  const x = instance.exports as unknown as Exports;
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const ears = new Set<(n: Notice) => void>();

  /** One call: its result, its notices told to whoever listens. */
  function call<T>(args: Record<string, unknown>): T {
    const input = encoder.encode(JSON.stringify(args));
    const ptr = x.studio_alloc(input.length);
    new Uint8Array(x.memory.buffer, ptr, input.length).set(input);
    const packed = x.studio_call(ptr, input.length);
    const out = Number(packed >> 32n);
    const len = Number(packed & 0xffffffffn);
    const reply = JSON.parse(decoder.decode(new Uint8Array(x.memory.buffer, out, len)));
    x.studio_free(out, len);
    if ('error' in reply) throw new Error(`studio core: ${reply.error}`);
    for (const n of reply.notices as Notice[]) ears.forEach((hear) => hear(n));
    return reply.result as T;
  }
  const later = async <T>(args: Record<string, unknown>) => call<T>(args);

  return {
    about: async () => null,
    pickFolder: async () => 'sample',
    open: async () => call({ call: 'open', files }),
    view: (editing, icon, visible) => later({ call: 'view', editing, icon, visible }),
    source: (file) => later<Source | null>({ call: 'source', file }),
    outline: (definition) => later<Outline | null>({ call: 'outline', definition }),
    preview: (icon, params) => later<Drawing | null>({ call: 'preview', icon, params }),
    sketch: (source) => later<Drawing | null>({ call: 'sketch', source }),
    // the module has no font converter, and no disk
    importFont: async () => ({ kind: 'refused', reason: 'Fonts are imported in the studio’s app' }),
    exportFont: async () => {
      throw new Error('Fonts are exported in the studio’s app');
    },
    pickFile: async () => null,
    pickSave: async () => null,
    ask: (ask, label) => later<Answer>({ call: 'ask', ask, label }),
    choose: (id, option) => later<Answer>({ call: 'choose', id, option }),
    beginGesture: () => later({ call: 'begin_gesture' }),
    endGesture: () => later({ call: 'end_gesture' }),
    undo: () => later<string | null>({ call: 'undo' }),
    redo: () => later<string | null>({ call: 'redo' }),
    save: () => later({ call: 'save' }),
    // there is no disk to reload from
    reload: async () => {},
    listen: async (hear) => {
      ears.add(hear);
      return () => ears.delete(hear);
    },
  };
}
