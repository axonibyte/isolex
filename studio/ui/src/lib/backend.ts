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
      /** The steps that can be undone, oldest first; those that can be
          redone, the next first. */
      history: string[];
      future: string[];
      can_undo: boolean;
      can_redo: boolean;
    }
  | { kind: 'graph'; graph: Graph; tree: Tree }
  | {
      kind: 'compiled';
      generation: number;
      revision: number;
      errors: Diag[];
      warnings: Diag[];
      palette: Record<string, { light: string; dark: string }> | null;
      /** What the lexicon block declares; empty when it declares none. */
      tokens: string[];
      stroke_widths: number[];
      icons: string[];
    }
  /** `redrawn` unless only where its shapes are written moved. */
  | { kind: 'icon'; name: string; ir: IrIcon; sources: Origin[]; redrawn: boolean }
  | { kind: 'broken'; name: string; diagnostics: Diag[]; last: { ir: IrIcon; sources: Origin[] } | null }
  | { kind: 'gone'; name: string }
  | { kind: 'reloaded'; files: string[] }
  | { kind: 'out-of-sync'; files: string[] };

/** A node of the source, by file and bytes. */
/** The Tree tab's rows (studio-core's `tree`). */
export interface Tree {
  icons: TreeRow[];
  fonts: TreeRow[];
}

export interface TreeRow {
  kind: 'base' | 'icon' | 'variant' | 'font' | 'style';
  name: string;
  /** What a click opens for editing: its own, or a variant's axis. */
  definition: string;
  /** The icon drawn for it: its thumbnail, and what a click draws. */
  icon: string | null;
  axis: string | null;
  variant: string | null;
  children: TreeRow[];
}

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
  /** The node's name: "box", "path"… */
  kind: string;
  definition: string;
  part: string | null;
  sites: NodeAt[];
  /** From the node's own coordinates to the viewBox: (x, y, z) goes to
      (m0·x + m2·y + m4·z + m6, m1·x + m3·y + m5·z + m7). */
  map: Map3;
  /** The numbers a 3-D node came to in this copy, as written: box x y z
      w d h; tile x y z w d; cylinder cx cy z r h; facerect a b z extent
      h; shadow cx cy rx ry; seg its two points. None for others. */
  values: number[];
}

export type Map3 = [number, number, number, number, number, number, number, number];

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
  /** A font's glyphs are its parts: "glyph", and "notdef". */
  op: 'part' | 'replace' | 'remove' | 'glyph' | 'notdef';
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
  /** Its footprint or canvas, its own or the nearest it extends. */
  frame: { kind: 'footprint' | 'canvas'; args: string[]; from: string | null; node: NodeAt } | null;
  /** A font's metrics and kerning pairs, as written. */
  font: {
    metrics: { name: string; value: string; node: NodeAt }[];
    kerns: { left: string; right: string; by: string; value: number | null; node: NodeAt }[];
  } | null;
  /** Its params, own first, then inherited, nearest first. */
  params: { name: string; default: string; value: number | null; from: string | null; node: NodeAt }[];
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

/** An icon as drawn, and where each of its shapes is written. */
export interface Drawing {
  ir: IrIcon;
  sources: Origin[];
}

/** What the Library dialog asks of the library. */
export type LibraryOp =
  | { op: 'rename-definition'; from: string; to: string }
  | { op: 'new-file'; path: string }
  | { op: 'move-file'; from: string; to: string }
  | { op: 'new-definition'; file: string; kind: string; name: string }
  | { op: 'move-definition'; name: string; to: string };

/** One node of the lexicon or palette block, its values unquoted. */
export interface Setting {
  name: string;
  args: string[];
  props: Record<string, string>;
  node: NodeAt;
}

/** The lexicon and palette blocks, each where it is written, and the file
    one not yet written goes in. */
export interface LexiconRows {
  lexicon: { node: NodeAt; rows: Setting[] } | null;
  palette: { node: NodeAt; rows: Setting[] } | null;
  file: string;
}

/** A font to import, as the dialog asks for it. */
export interface FontImport {
  path: string;
  name: string | null;
  /** "ascii", "latin1", "all", or the characters. */
  chars: string;
  em: number | null;
  /** Each further style: "axis.variant", and a font file or "wght:700". */
  styles: [string, string][];
}

export interface Source {
  revision: number;
  text: string;
}

/** Everything the interface asks of the studio. */
export interface Backend {
  /** The studio's and the compiler's versions, or none outside the app. */
  about(): Promise<{ studio: string; isolex: string } | null>;
  /** The library folder the app was started with (`isolex-studio
      <folder>`), which opens first; none outside the app. */
  startup(): Promise<string | null>;
  /** Asks the person for a library folder; none if they cancel. */
  pickFolder(): Promise<string | null>;
  open(path: string): Promise<void>;
  view(editing: string | null, icon: string | null, visible: string[]): Promise<void>;
  source(file: string): Promise<Source | null>;
  outline(definition: string): Promise<Outline | null>;
  /** `icon` drawn with `params` set otherwise, the library unchanged;
      none if it does not compile so. */
  preview(icon: string, params: Record<string, number>): Promise<Drawing | null>;
  /** An icon `studio-sketch` the interface writes, compiled with the
      library, nothing changed: how fonts are shown. None if it does not
      compile. */
  sketch(source: string): Promise<Drawing | null>;
  /** A font file read into the library, one undo step; refused, saying
      why, outside the app or when its license forbids it. */
  importFont(font: FontImport): Promise<Answer>;
  /** A font written in `styles` as TrueType, or a UFO, at `out`: where,
      or an error saying why. */
  exportFont(font: string, styles: string[], out: string, ufo: boolean): Promise<string>;
  /** A file or definition made, renamed or moved: one undo step. */
  libraryOp(op: LibraryOp): Promise<Answer>;
  /** The `lexicon` and `palette` blocks as written. */
  lexicon(): Promise<LexiconRows>;
  /** A new library at `path`, empty or a copy of another, opened. */
  newLibrary(path: string, start: { from: 'empty' } | { from: 'copy'; of: string }): Promise<void>;
  /** Files changed "on disk" (a text, or none for removed): only the core
      outside the app, which keeps its disk in memory, does this. */
  outside?(changes: [string, string | null][]): Promise<void>;
  /** Asks the person for a file to read, or a place to write; none if
      they cancel, or outside the app. */
  pickFile(title: string, extensions: string[]): Promise<string | null>;
  pickSave(title: string, name: string, extensions: string[]): Promise<string | null>;
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
    startup: () => invoke('startup'),
    pickFolder: async () => {
      const picked = await dialog.open({ directory: true, multiple: false, title: 'Open a library folder' });
      return typeof picked === 'string' ? picked : null;
    },
    open: (path) => invoke('open', { path }),
    view: (editing, icon, visible) => invoke('view', { editing, icon, visible }),
    source: (file) => invoke('source', { file }),
    outline: (definition) => invoke('outline', { definition }),
    preview: (icon, params) => invoke('preview', { icon, params }),
    sketch: (source) => invoke('sketch', { source }),
    importFont: (font) => invoke('import_font', { font }),
    libraryOp: (op) => invoke('library_op', { op }),
    lexicon: () => invoke('lexicon'),
    newLibrary: (path, start) => invoke('new_library', { path, start }),
    exportFont: (font, styles, out, ufo) => invoke('export_font', { font, styles, out, ufo }),
    pickFile: async (title, extensions) => {
      const picked = await dialog.open({ title, multiple: false, filters: [{ name: 'Fonts', extensions }] });
      return typeof picked === 'string' ? picked : null;
    },
    pickSave: async (title, name, extensions) => (await dialog.save({ title, defaultPath: name, filters: extensions.length ? [{ name: 'Font', extensions }] : [] })) ?? null,
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
