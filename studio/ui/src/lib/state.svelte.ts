// What the window shows, from what the studio's core tells it: `reduce`
// folds each notice into a plain state, and `Studio` keeps that state
// reactive and talks to the backend.

import type { IrIcon } from './isolex/ir';
import type { Backend, Diag, Drawing, Graph, GraphDef, NodeAt, Notice, Origin, Outline, Row, Start, Tree, TreeRow } from './backend';
import { atPath, find, same } from './select';
import { advanceOf, glyphView, gridSource, metricsOf, type GlyphView } from './font';

export interface LibraryState {
  root: string;
  revision: number;
  modified: boolean;
  files: string[];
  /** The steps that can be undone, oldest first; those that can be
      redone, the next first. */
  history: string[];
  future: string[];
  canUndo: boolean;
  canRedo: boolean;
}

export interface View {
  library: LibraryState | null;
  graph: Graph | null;
  /** The Tree tab's rows. */
  tree: Tree | null;
  /** The newest compile: which revision, its problems, its icons. */
  compiled: { generation: number; revision: number; errors: Diag[]; warnings: Diag[]; icons: string[] } | null;
  palette: Record<string, { light: string; dark: string }> | null;
  /** The tokens and stroke widths the lexicon declares, which paint is
      chosen from; empty when it declares none. */
  lexicon: { tokens: string[]; widths: number[] };
  /** Each icon's last good drawing. */
  icons: Record<string, IrIcon>;
  /** Where each shape of that drawing is written. */
  sources: Record<string, Origin[]>;
  /** Icons that do not compile now, with why. */
  broken: Record<string, Diag[]>;
  /** How many times each icon has been drawn differently since the
      library opened: its first drawing is 0, so one an edit redrew is
      above it. */
  redrawn: Record<string, number>;
  /** Files changed on disk while there are unsaved edits. */
  outOfSync: string[];
}

export const empty = (): View => ({
  library: null,
  graph: null,
  tree: null,
  compiled: null,
  palette: null,
  lexicon: { tokens: [], widths: [] },
  icons: {},
  sources: {},
  broken: {},
  redrawn: {},
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
          future: n.future,
          canUndo: n.can_undo,
          canRedo: n.can_redo,
        },
        // nothing unsaved: whatever was out of sync no longer is
        outOfSync: n.modified ? v.outOfSync : [],
      };
    case 'graph':
      return { ...v, graph: n.graph, tree: n.tree };
    case 'compiled': {
      if (v.compiled && n.generation < v.compiled.generation) return v;
      return {
        ...v,
        compiled: { generation: n.generation, revision: n.revision, errors: n.errors, warnings: n.warnings, icons: n.icons },
        palette: n.palette,
        lexicon: { tokens: n.tokens, widths: n.stroke_widths },
      };
    }
    case 'icon': {
      const broken = { ...v.broken };
      delete broken[n.name];
      const had = v.icons[n.name];
      // only moved in its file: the drawing it had stands, not drawn again
      const ir = had && !n.redrawn ? had : n.ir;
      const redrawn = had && n.redrawn ? { ...v.redrawn, [n.name]: (v.redrawn[n.name] ?? 0) + 1 } : v.redrawn;
      return { ...v, icons: { ...v.icons, [n.name]: ir }, sources: { ...v.sources, [n.name]: n.sources }, broken, redrawn };
    }
    case 'broken':
      return {
        ...v,
        icons: n.last ? { ...v.icons, [n.name]: n.last.ir } : v.icons,
        sources: n.last ? { ...v.sources, [n.name]: n.last.sources } : v.sources,
        broken: { ...v.broken, [n.name]: n.diagnostics },
      };
    case 'gone': {
      const icons = { ...v.icons };
      const sources = { ...v.sources };
      const broken = { ...v.broken };
      delete icons[n.name];
      delete sources[n.name];
      delete broken[n.name];
      return { ...v, icons, sources, broken };
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
  /** The icon drawn on the canvas. */
  icon = $state<string | null>(null);
  /** The definition being edited: the icon's own, or one it is drawn
      from, opened from the Parts panel. */
  editing = $state<string | null>(null);
  /** The edited definition's parts, as of the library's revision. */
  outline = $state<Outline | null>(null);
  /** The node selected, and its place in the outline, by which it is
      found again once an edit moves it in the source. */
  selected = $state<NodeAt | null>(null);
  private selectedPath: string | null = null;
  /** The nodes chosen before it with Shift, in the order chosen, each
      with its place in the outline. */
  private others = $state<{ node: NodeAt; path: string }[]>([]);
  /** The part new shapes go into, by name. */
  target = $state<string | null>(null);
  /** Parts hidden or locked in the editor, by name; never written to the
      source. */
  hidden = $state<Set<string>>(new Set());
  locked = $state<Set<string>>(new Set());
  /** A message for the hint line, such as why an edit was refused. */
  message = $state<string>('');
  /** A place in the source to show, such as a problem's. */
  /** A place opened in the Source tab: a problem's, or what Find found
      (which has no severity). */
  place = $state<{ file: string; line: number; col: number; message: string; severity: 'error' | 'warning' | null } | null>(null);
  /** The styles a font's glyphs are set out in, on the canvas. */
  fontStyle = $state<string[]>([]);
  /** In a font being edited, the glyph open on the canvas, by its name
      ("notdef" for the notdef); none for the whole font. */
  glyph = $state<string | null>(null);
  /** What the canvas draws for a font: its glyphs set out, or the glyph
      open, as the core compiles it (`sketch`), with how a glyph is laid
      out for its guides. */
  sketch = $state<{ drawing: Drawing; glyph: GlyphView | null } | null>(null);
  /** The open icon as previewed with other params, the library as it is;
      gone once the library changes. */
  previewing = $state<{ params: Record<string, number>; drawing: Drawing } | null>(null);

  constructor(readonly backend: Backend) {}

  /** Starts hearing the core. */
  async start(): Promise<() => void> {
    return this.backend.listen((n) => this.hear(n));
  }

  hear(n: Notice) {
    const revision = this.view.library?.revision;
    this.view = reduce(this.view, n);
    // the first icon compiled opens, when none is open
    if (!this.icon && n.kind === 'icon') this.openIcon(n.name);
    if (this.icon && n.kind === 'gone' && n.name === this.icon) this.icon = null;
    if (n.kind === 'library' && n.revision !== revision) {
      this.previewing = null;
      void this.refresh();
    }
  }

  /** The open icon drawn with `params` set otherwise, nothing written;
      none clears the preview. */
  async preview(params: Record<string, number> | null) {
    const icon = this.icon;
    if (!params || !icon) {
      this.previewing = null;
      return;
    }
    const drawing = await this.backend.preview(icon, params);
    if (icon === this.icon) this.previewing = drawing ? { params, drawing } : null;
    if (!drawing) this.message = `${icon} does not compile with those values`;
  }

  /** Back or forward through the undo history by `steps`: back for fewer
      than none. */
  async travel(steps: number) {
    for (let k = 0; k < Math.abs(steps); k++) {
      const label = await (steps < 0 ? this.backend.undo() : this.backend.redo());
      if (!label) break;
    }
    this.message = steps < 0 ? `Went back ${-steps} step${steps === -1 ? '' : 's'}` : `Went forward ${steps} step${steps === 1 ? '' : 's'}`;
  }

  /** Opens a problem's place: the Source tab, at its line. */
  reveal(d: Diag, severity: 'error' | 'warning') {
    this.place = { file: d.file, line: d.line, col: d.col, message: d.message, severity };
  }

  /** The definition being edited, as the graph has it. */
  get definition(): GraphDef | null {
    const name = this.editing;
    return this.view.graph?.definitions.find((d) => d.name === name) ?? null;
  }

  /** Where the edited definition is written. */
  get editingNode(): NodeAt | null {
    const d = this.definition;
    return d ? { file: d.file, start: d.span.start, len: d.span.len } : null;
  }

  /** A font is being edited: the canvas shows it, not an icon. */
  get font(): boolean {
    return this.outline?.kind === 'font';
  }

  /** What the canvas draws, and where each of its shapes is written: the
      font's sketch while a font is edited, else the icon open. */
  get drawing(): { ir: IrIcon; sources: Origin[] } | null {
    if (this.font) return this.sketch?.drawing ?? null;
    const ir = this.icon ? this.view.icons[this.icon] : undefined;
    return ir ? { ir, sources: this.view.sources[this.icon!] ?? [] } : null;
  }

  /** Opens a glyph of the font being edited, or the whole font. */
  openGlyph(name: string | null) {
    if (name === this.glyph) return;
    this.glyph = name;
    this.selected = null;
    this.selectedPath = null;
    this.others = [];
    this.target = name;
    void this.redrawSketch();
  }

  /** A style of the font switched on or off, its glyphs set in it. */
  toggleStyle(style: string) {
    const next = new Set(this.fontStyle);
    if (!next.delete(style)) next.add(style);
    this.fontStyle = [...next];
    void this.redrawSketch();
  }

  /** The font's sketch again: its glyphs, or the one open. */
  async redrawSketch() {
    const o = this.outline;
    if (!o || o.kind !== 'font') {
      this.sketch = null;
      return;
    }
    const m = metricsOf(o);
    const part = this.glyph ? o.parts.find((p) => p.name === this.glyph) : null;
    let source: string;
    let glyph: GlyphView | null = null;
    if (part) {
      const src = await this.backend.source(part.node.file);
      const text = src ? new TextDecoder().decode(new TextEncoder().encode(src.text).slice(part.node.start, part.node.start + part.node.len)) : '';
      glyph = glyphView(o.definition, part.name, part.op, advanceOf(text, m), m);
      source = glyph.source;
    } else source = gridSource(o.definition, o, this.fontStyle).source;
    const drawing = await this.backend.sketch(source);
    if (o !== this.outline) return;
    if (drawing) this.sketch = { drawing, glyph };
    else this.message = `${o.definition} cannot be set as text now: see its problems`;
  }

  /** A row of the Tree tab opened: its icon drawn, its definition (a
      variant's axis) edited. */
  openRow(row: TreeRow) {
    if (row.icon) this.icon = row.icon;
    this.edit(row.definition);
  }

  /** Draws `name` and edits its own definition. */
  openIcon(name: string) {
    this.icon = name;
    this.edit(definitionOf(this.view.graph, name)?.name ?? null);
  }

  /** Edits the definition `name`, the icon drawn staying as it is unless
      the definition emits icons, when its first is drawn. */
  edit(name: string | null) {
    const emits = this.view.graph?.definitions.find((d) => d.name === name)?.emits ?? [];
    if (emits.length && (!this.icon || !emits.includes(this.icon))) this.icon = emits[0]!;
    if (name !== this.editing) {
      this.editing = name;
      this.glyph = null;
      this.fontStyle = [];
      this.sketch = null;
      this.previewing = null;
      this.place = null;
      this.outline = null;
      this.selected = null;
      this.selectedPath = null;
      this.others = [];
      this.target = null;
      this.hidden = new Set();
      this.locked = new Set();
    }
    void this.backend.view(name, this.icon, []);
    void this.refresh();
  }

  /** The outline again, the selection and target found in it again. */
  async refresh() {
    const name = this.editing;
    const outline = name ? await this.backend.outline(name) : null;
    if (name !== this.editing) return;
    this.outline = outline;
    const again = this.selectedPath ? atPath(outline, this.selectedPath) : null;
    this.selected = again ? (again.row?.node ?? again.part.node) : null;
    if (!again) this.selectedPath = null;
    this.others = this.others.flatMap((o) => {
      const f = atPath(outline, o.path);
      return f ? [{ node: f.row?.node ?? f.part.node, path: o.path }] : [];
    });
    const own = outline?.parts.filter((p) => p.op !== 'remove') ?? [];
    if (!own.some((p) => p.name === this.target)) this.target = own.at(-1)?.name ?? null;
    // in a font, what is drawn goes into the glyph open
    if (outline?.kind === 'font') {
      if (this.glyph && !own.some((p) => p.name === this.glyph)) this.glyph = null;
      this.target = this.glyph;
      await this.redrawSketch();
    }
  }

  /** Selects the node at `node`, a part or a row of the outline, or
      nothing. */
  select(node: NodeAt | null) {
    const f = find(this.outline, node);
    this.selected = f ? node : null;
    this.selectedPath = f?.path ?? null;
    this.others = [];
  }

  /** Everything selected, in the order chosen: the selection last. */
  get chosen(): NodeAt[] {
    return this.selected ? [...this.others.map((o) => o.node), this.selected] : [];
  }

  /** Adds the node at `node` to what is selected, as Shift+click does,
      or takes it out if it is in. It becomes the selection; the one
      before stays chosen. */
  add(node: NodeAt) {
    const f = find(this.outline, node);
    if (!f) return;
    if (this.selected && same(this.selected, node)) {
      const last = this.others.at(-1);
      this.others = this.others.slice(0, -1);
      this.selected = last?.node ?? null;
      this.selectedPath = last?.path ?? null;
      return;
    }
    const kept = this.others.filter((o) => !same(o.node, node));
    if (kept.length < this.others.length) {
      this.others = kept;
      return;
    }
    if (this.selected && this.selectedPath) this.others = [...this.others, { node: this.selected, path: this.selectedPath }];
    this.selected = node;
    this.selectedPath = f.path;
  }

  /** Keeps the selection on the row at `path` of the outline to come,
      where an edit is about to move it. */
  follow(path: string) {
    this.selectedPath = path;
  }

  /** A new library at `path`, made and opened. */
  async create(path: string, start: Start) {
    this.view = empty();
    this.icon = null;
    this.editing = null;
    this.outline = null;
    this.selected = null;
    this.selectedPath = null;
    this.others = [];
    await this.backend.newLibrary(path, start);
  }

  async open(path: string) {
    this.view = empty();
    this.icon = null;
    this.editing = null;
    this.outline = null;
    this.selected = null;
    this.selectedPath = null;
    this.others = [];
    await this.backend.open(path);
  }

  /** Asks the core, telling the person why if it refuses. */
  async ask(ask: Parameters<Backend['ask']>[0], label: string) {
    const answer = await this.backend.ask(ask, label);
    if (answer.kind === 'refused') this.message = answer.reason;
    return answer;
  }

  /** Sends a reorder of `siblings[index]` (at `path`), the selection
      following the row it moved. */
  async reorder(ask: { node: NodeAt; before: NodeAt | null } | null, siblings: Row[], index: number, path: string) {
    const outline = this.outline;
    if (!ask || !outline) return;
    const moved = siblings[index]!;
    const to = ask.before ? siblings.filter((_, i) => i !== index).findIndex((r) => same(r.node, ask.before!)) : siblings.length - 1;
    const parent = path.slice(0, path.lastIndexOf('/'));
    const was = this.selected && same(this.selected, moved.node);
    // the rows after it move: only the one moved stays chosen
    this.others = [];
    if (was) this.follow(`${parent}/${to}`);
    const answer = await this.ask({ kind: 'move-before', node: ask.node, before: ask.before }, `Move ${moved.kind} in ${outline.definition}`);
    if (answer.kind !== 'applied' && was) this.follow(path);
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
