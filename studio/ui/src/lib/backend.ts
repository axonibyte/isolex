// What the interface says to the studio's core, and hears from it: the
// shell's commands and the `isolex` event's notices (studio-core's
// session), as types; and the backend that carries them, in the app or a
// stand-in elsewhere (lib/core.ts).

import type { IrIcon } from './isolex/ir';

/** One problem, as the compiler located it. */
export interface Diag {
  file: string;
  line: number;
  col: number;
  message: string;
}

export interface GraphDef {
  name: string;
  kind: 'Base' | 'Icon' | 'Fragment' | 'Axis' | 'Font';
  file: string;
  span: { file: number; start: number; len: number };
  emits: string[];
}

export interface GraphEdge {
  kind: 'extends' | 'insert' | 'badge' | 'axis' | 'font';
  from: string;
  to: string;
  at: { file: number; start: number; len: number };
  variant?: string;
}

export interface Graph {
  definitions: GraphDef[];
  edges: GraphEdge[];
}

export type Notice =
  | {
      kind: 'library';
      root: string;
      revision: number;
      modified: boolean;
      files: string[];
      history: string[];
      can_undo: boolean;
      can_redo: boolean;
    }
  | { kind: 'graph'; graph: Graph }
  | {
      kind: 'compiled';
      generation: number;
      revision: number;
      errors: Diag[];
      warnings: Diag[];
      palette: Record<string, { light: string; dark: string }> | null;
      icons: string[];
    }
  | { kind: 'icon'; name: string; ir: IrIcon; sources: Origin[] }
  | { kind: 'broken'; name: string; diagnostics: Diag[]; last: { ir: IrIcon; sources: Origin[] } | null }
  | { kind: 'gone'; name: string }
  | { kind: 'reloaded'; files: string[] }
  | { kind: 'out-of-sync'; files: string[] };

/** A node of the source, by file and bytes. */
export interface NodeAt {
  file: string;
  start: number;
  len: number;
}

/** Where one shape of a drawing is written: the node that draws it, the
    definition and part it is in, and the insert, badge, repeat and text
    nodes it was drawn through, outermost first. One per shape, in the
    order the IR lists them, a group before its own. */
export interface Origin {
  node: NodeAt;
  definition: string;
  part: string | null;
  sites: NodeAt[];
}

/** A shape in a part, or a node holding shapes (at, repeat, transform,
    turn) with its own under it. */
export interface Row {
  kind: string;
  note: string;
  node: NodeAt;
  children: Row[];
}

export interface Part {
  name: string;
  op: 'part' | 'replace' | 'remove';
  /** The axis variant it is written in. */
  variant: string | null;
  node: NodeAt;
  /** In source order: the last is drawn on top. */
  shapes: Row[];
}

/** A definition's parts, for the Parts panel. */
export interface Outline {
  definition: string;
  kind: 'base' | 'icon' | 'fragment' | 'axis' | 'font';
  extends: string | null;
  node: NodeAt;
  parts: Part[];
  /** Drawn from the definitions it extends, each naming which. */
  inherited: { name: string; from: string; shapes: number }[];
}

export type JsonValue = boolean | number | string | null;

export type Ask =
  | { kind: 'set-arg'; node: NodeAt; index: number; value: JsonValue }
  | { kind: 'set-prop'; node: NodeAt; key: string; value: JsonValue }
  | { kind: 'remove-prop'; node: NodeAt; key: string }
  | { kind: 'insert'; parent: NodeAt; text: string; before: NodeAt | null }
  | { kind: 'delete'; node: NodeAt }
  | { kind: 'move-before'; node: NodeAt; before: NodeAt | null }
  | { kind: 'type'; file: string; at: number; len: number; text: string };

export type Answer =
  | { kind: 'applied'; revision: number }
  | { kind: 'choose'; id: number; options: { label: string; detail: string }[] }
  | { kind: 'refused'; reason: string };

export interface Source {
  revision: number;
  text: string;
}

/** Everything the interface asks of the studio. */
export interface Backend {
  /** The studio's and the compiler's versions, or none outside the app. */
  about(): Promise<{ studio: string; isolex: string } | null>;
  /** Asks the person for a library folder; none if they cancel. */
  pickFolder(): Promise<string | null>;
  open(path: string): Promise<void>;
  view(editing: string | null, icon: string | null, visible: string[]): Promise<void>;
  source(file: string): Promise<Source | null>;
  outline(definition: string): Promise<Outline | null>;
  ask(ask: Ask, label: string): Promise<Answer>;
  choose(id: number, option: number): Promise<Answer>;
  beginGesture(): Promise<void>;
  endGesture(): Promise<void>;
  undo(): Promise<string | null>;
  redo(): Promise<string | null>;
  save(): Promise<void>;
  reload(): Promise<void>;
  /** Calls `hear` with every notice from now on; returns how to stop. */
  listen(hear: (n: Notice) => void): Promise<() => void>;
}

/** Whether the interface is in the studio's window, not a plain browser. */
export const inShell = (): boolean => '__TAURI_INTERNALS__' in globalThis;

/** The backend in the studio's window: the shell's commands and events. */
export async function shell(): Promise<Backend> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const dialog = await import('@tauri-apps/plugin-dialog');
  return {
    about: () => invoke('about'),
    pickFolder: async () => {
      const picked = await dialog.open({ directory: true, multiple: false, title: 'Open a library folder' });
      return typeof picked === 'string' ? picked : null;
    },
    open: (path) => invoke('open', { path }),
    view: (editing, icon, visible) => invoke('view', { editing, icon, visible }),
    source: (file) => invoke('source', { file }),
    outline: (definition) => invoke('outline', { definition }),
    ask: (ask, label) => invoke('ask', { ask, label }),
    choose: (id, option) => invoke('choose', { id, option }),
    beginGesture: () => invoke('begin_gesture'),
    endGesture: () => invoke('end_gesture'),
    undo: () => invoke('undo'),
    redo: () => invoke('redo'),
    save: () => invoke('save'),
    reload: () => invoke('reload'),
    listen: async (hear) => listen<Notice>('isolex', (e) => hear(e.payload)),
  };
}
