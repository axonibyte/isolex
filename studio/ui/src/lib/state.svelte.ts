// What the window shows, from what the studio's core tells it: `reduce`
// folds each notice into a plain state, and `Studio` keeps that state
// reactive and talks to the backend.

import type { IrIcon } from './isolex/ir';
import type { Backend, Diag, Graph, GraphDef, Notice } from './backend';

export interface LibraryState {
  root: string;
  revision: number;
  modified: boolean;
  files: string[];
  history: string[];
  canUndo: boolean;
  canRedo: boolean;
}

export interface View {
  library: LibraryState | null;
  graph: Graph | null;
  /** The newest compile: which revision, its problems, its icons. */
  compiled: { generation: number; revision: number; errors: Diag[]; warnings: Diag[]; icons: string[] } | null;
  palette: Record<string, { light: string; dark: string }> | null;
  /** Each icon's last good drawing. */
  icons: Record<string, IrIcon>;
  /** Icons that do not compile now, with why. */
  broken: Record<string, Diag[]>;
  /** Files changed on disk while there are unsaved edits. */
  outOfSync: string[];
}

export const empty = (): View => ({
  library: null,
  graph: null,
  compiled: null,
  palette: null,
  icons: {},
  broken: {},
  outOfSync: [],
});

/** The view after `n`. */
export function reduce(v: View, n: Notice): View {
  switch (n.kind) {
    case 'library':
      return {
        ...v,
        library: {
          root: n.root,
          revision: n.revision,
          modified: n.modified,
          files: n.files,
          history: n.history,
          canUndo: n.can_undo,
          canRedo: n.can_redo,
        },
        // nothing unsaved: whatever was out of sync no longer is
        outOfSync: n.modified ? v.outOfSync : [],
      };
    case 'graph':
      return { ...v, graph: n.graph };
    case 'compiled': {
      if (v.compiled && n.generation < v.compiled.generation) return v;
      return {
        ...v,
        compiled: { generation: n.generation, revision: n.revision, errors: n.errors, warnings: n.warnings, icons: n.icons },
        palette: n.palette,
      };
    }
    case 'icon': {
      const broken = { ...v.broken };
      delete broken[n.name];
      return { ...v, icons: { ...v.icons, [n.name]: n.ir }, broken };
    }
    case 'broken':
      return {
        ...v,
        icons: n.last ? { ...v.icons, [n.name]: n.last } : v.icons,
        broken: { ...v.broken, [n.name]: n.diagnostics },
      };
    case 'gone': {
      const icons = { ...v.icons };
      const broken = { ...v.broken };
      delete icons[n.name];
      delete broken[n.name];
      return { ...v, icons, broken };
    }
    case 'reloaded':
      return { ...v, outOfSync: [] };
    case 'out-of-sync':
      return { ...v, outOfSync: [...new Set([...v.outOfSync, ...n.files])] };
  }
}

/** The definition that emits `icon`, or that is named so. */
export function definitionOf(graph: Graph | null, icon: string | null): GraphDef | null {
  if (!graph || !icon) return null;
  return graph.definitions.find((d) => d.name === icon) ?? graph.definitions.find((d) => d.emits.includes(icon)) ?? null;
}

/** The studio, as the window sees it. */
export class Studio {
  view = $state<View>(empty());
  /** The icon open on the canvas. */
  icon = $state<string | null>(null);
  /** A message for the hint line, such as why an edit was refused. */
  message = $state<string>('');

  constructor(readonly backend: Backend) {}

  /** Starts hearing the core. */
  async start(): Promise<() => void> {
    return this.backend.listen((n) => this.hear(n));
  }

  hear(n: Notice) {
    this.view = reduce(this.view, n);
    // the first icon compiled opens, when none is open
    if (!this.icon && n.kind === 'icon') this.openIcon(n.name);
    if (this.icon && n.kind === 'gone' && n.name === this.icon) this.icon = null;
  }

  get definition(): GraphDef | null {
    return definitionOf(this.view.graph, this.icon);
  }

  openIcon(name: string) {
    this.icon = name;
    const def = definitionOf(this.view.graph, name);
    void this.backend.view(def?.name ?? null, name, []);
  }

  async open(path: string) {
    this.view = empty();
    this.icon = null;
    await this.backend.open(path);
  }

  async undo() {
    const label = await this.backend.undo();
    this.message = label ? `Undid ${label}` : 'Nothing to undo';
  }

  async redo() {
    const label = await this.backend.redo();
    this.message = label ? `Redid ${label}` : 'Nothing to redo';
  }

  async save() {
    await this.backend.save();
    this.message = 'Saved';
  }
}
