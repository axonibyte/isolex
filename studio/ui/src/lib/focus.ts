// The Tree's focus view (#113, docs/studio-interface.md, Tree), as
// Unreal's Reference Viewer lays out an asset: the definition being
// edited in the middle, what it depends on in columns to its left, what
// depends on it in columns to its right, each step a column, up to a
// depth. Every line runs from a definition to one it refers to, which
// is always the one on the left. Pure but for where the view is kept.

import type { Graph, GraphDef, GraphEdge } from './backend';

export const DEPTHS = [1, 2, 3] as const;

export interface FocusNode {
  name: string;
  /** Its definition's kind; none for a name nothing defines. */
  kind: GraphDef['kind'] | null;
  /** Its column: 0 in the middle, negative for what the middle depends
      on, positive for what depends on it, each a step further out. */
  column: number;
  row: number;
  /** The definition it was reached from, a column nearer the middle, and
      by which reference; none for the middle. */
  via: string | null;
  edge: GraphEdge['kind'] | null;
  variant?: string;
}

export interface FocusLink {
  /** The node referred to, as `${column}:${name}`, and the node that
      refers to it, a column to its right. */
  left: string;
  right: string;
  kind: GraphEdge['kind'];
  variant?: string;
}

export interface Focus {
  center: string;
  depth: number;
  nodes: FocusNode[];
  links: FocusLink[];
}

export const key = (n: Pick<FocusNode, 'column' | 'name'>) => `${n.column}:${n.name}`;

/** The focus view of `center`, `depth` steps each way. Each side lists a
    definition once, at its nearest step, in the order the graph's edges
    reach it; a cycle, which the resolver reports, ends there. */
export function focus(graph: Graph, center: string, depth: number): Focus {
  const kinds = new Map(graph.definitions.map((d) => [d.name, d.kind]));
  const kind = (name: string) => kinds.get(name) ?? null;
  const nodes: FocusNode[] = [{ name: center, kind: kind(center), column: 0, row: 0, via: null, edge: null }];
  for (const side of [-1, 1]) {
    const seen = new Set([center]);
    let frontier = [center];
    for (let step = 1; step <= depth && frontier.length; step++) {
      const next: string[] = [];
      for (const at of frontier) {
        for (const e of graph.edges) {
          const [from, to] = side < 0 ? [e.from, e.to] : [e.to, e.from];
          if (from !== at || seen.has(to)) continue;
          seen.add(to);
          nodes.push({ name: to, kind: kind(to), column: side * step, row: next.length, via: at, edge: e.kind, ...(e.variant ? { variant: e.variant } : {}) });
          next.push(to);
        }
      }
      frontier = next;
    }
  }
  const at = new Set(nodes.map(key));
  const links: FocusLink[] = [];
  const linked = new Set<string>();
  for (let column = -depth; column < depth; column++) {
    for (const e of graph.edges) {
      const left = key({ column, name: e.to });
      const right = key({ column: column + 1, name: e.from });
      const id = `${left} ${right} ${e.kind} ${e.variant ?? ''}`;
      if (!at.has(left) || !at.has(right) || linked.has(id)) continue;
      linked.add(id);
      links.push({ left, right, kind: e.kind, ...(e.variant ? { variant: e.variant } : {}) });
    }
  }
  return { center, depth, nodes, links };
}

const VERBS: Record<GraphEdge['kind'], string> = { extends: 'extends', insert: 'inserts', badge: 'badges', axis: 'opts into', font: 'sets text in' };

/** How a reference reads: "inserts". */
export const verb = (kind: GraphEdge['kind']) => VERBS[kind];

/** What `n` is to the middle, for the hint line: "server inserts it",
    "Extends chassis". */
export function said(n: FocusNode): string {
  if (!n.via || !n.edge) return 'Being edited';
  const v = verb(n.edge);
  const how = (n.column < 0 ? `${n.via} ${v} it` : `${v.charAt(0).toUpperCase()}${v.slice(1)} ${n.via}`) + (n.variant ? `, in variant ${n.variant}` : '');
  return n.kind ? how : `${how}; nothing defines it`;
}

/** A depth asked for, as one the view has: whole, and within its range. */
export const clampDepth = (d: number, fallback: number) => (Number.isFinite(d) ? Math.min(DEPTHS.at(-1)!, Math.max(DEPTHS[0], Math.round(d))) : fallback);

/** The Tree's view and depth, remembered per person as the themes are. */
export type TreeView = 'library' | 'focus';

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // remembered or not, it applies now
  }
}

export const treeView = (): TreeView => (read('studio.tree.view') === 'focus' ? 'focus' : 'library');
export const setTreeView = (v: TreeView) => write('studio.tree.view', v);
export function focusDepth(): number {
  const d = Number(read('studio.tree.depth'));
  return (DEPTHS as readonly number[]).includes(d) ? d : 2;
}
export const setFocusDepth = (d: number) => write('studio.tree.depth', String(d));
