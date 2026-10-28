// Selection (docs/studio-interface.md, Parts): which node a shape on the
// canvas, a caret in the source or a row in the Parts panel stands for;
// which shapes on the canvas a selected node draws; and how a row moves
// in painter order. Pure, so the three places agree.

import type { Graph, NodeAt, Origin, Outline, Part, Row } from './backend';

export const same = (a: NodeAt, b: NodeAt): boolean => a.file === b.file && a.start === b.start && a.len === b.len;

/** `a` is `b` or written inside it. */
export const within = (a: NodeAt, b: NodeAt): boolean =>
  a.file === b.file && a.start >= b.start && a.start + a.len <= b.start + b.len;

/** What a click on a shape selects while the definition at `editing` is
    open: the innermost of its node and the nodes it was drawn through
    that is written there; none for a shape drawn from elsewhere. */
export function pick(o: Origin, editing: NodeAt | null): NodeAt | null {
  if (!editing) return null;
  for (const n of [o.node, ...[...o.sites].reverse()]) if (within(n, editing)) return n;
  return null;
}

/** The definition a shape drawn from elsewhere comes from: the one its
    outermost site, or its node, is written in. */
export function source(o: Origin, graph: Graph | null): string {
  const outer = o.sites[0] ?? o.node;
  const def = graph?.definitions.find((d) => within(outer, { file: d.file, start: d.span.start, len: d.span.len }));
  return def?.name ?? o.definition;
}

/** The shapes, by their index in the drawing, that the node at `selected`
    draws while `editing` is open. */
export function marks(sources: Origin[], editing: NodeAt | null, selected: NodeAt | null): Set<number> {
  const out = new Set<number>();
  if (!selected) return out;
  sources.forEach((o, i) => {
    const n = pick(o, editing);
    if (n && within(n, selected)) out.add(i);
  });
  return out;
}

/** A row of the outline, with where it is. */
export interface Found {
  part: Part;
  /** None for the part itself. */
  row: Row | null;
  /** The rows it is among, in source order, and its index there. */
  siblings: Row[];
  index: number;
  /** "2" for the third part, "2/0/1" for a row under it. */
  path: string;
}

function* rows(outline: Outline): Generator<Found> {
  function* under(part: Part, list: Row[], path: string): Generator<Found> {
    for (const [index, row] of list.entries()) {
      const here = `${path}/${index}`;
      yield { part, row, siblings: list, index, path: here };
      yield* under(part, row.children, here);
    }
  }
  for (const [i, part] of outline.parts.entries()) {
    yield { part, row: null, siblings: [], index: i, path: `${i}` };
    yield* under(part, part.shapes, `${i}`);
  }
}

/** The part or row at `node`. */
export function find(outline: Outline | null, node: NodeAt | null): Found | null {
  if (!outline || !node) return null;
  for (const f of rows(outline)) if (same(f.row?.node ?? f.part.node, node)) return f;
  return null;
}

/** The part or row at `path`, as `find` gives it. */
export function atPath(outline: Outline | null, path: string): Found | null {
  if (!outline) return null;
  for (const f of rows(outline)) if (f.path === path) return f;
  return null;
}

/** The innermost row written around byte `at` of `file`: what a caret
    there selects. */
export function rowAt(outline: Outline | null, file: string, at: number): Found | null {
  if (!outline) return null;
  let best: Found | null = null;
  for (const f of rows(outline)) {
    const n = f.row?.node;
    if (n && n.file === file && n.start <= at && at < n.start + n.len) best = f;
  }
  return best;
}

export type Move = 'raise' | 'lower' | 'top' | 'bottom';

/** The ask that moves `siblings[from]` so it is `to`th in source order:
    the sibling it then goes before, or none for last. None at all if it
    stays where it is. */
export function reorder(siblings: Row[], from: number, to: number): { node: NodeAt; before: NodeAt | null } | null {
  const n = siblings.length;
  const t = Math.max(0, Math.min(n - 1, to));
  if (from < 0 || from >= n || t === from) return null;
  const rest = siblings.filter((_, i) => i !== from);
  return { node: siblings[from]!.node, before: rest[t]?.node ?? null };
}

/** A move in painter order: raise and top draw it later, over the others. */
export function move(siblings: Row[], index: number, how: Move) {
  const to = { raise: index + 1, lower: index - 1, top: siblings.length - 1, bottom: 0 }[how];
  return reorder(siblings, index, to);
}

/** The ask for dropping `siblings[from]` just above `siblings[onto]` in
    the panel, which lists the topmost first: just after it in source. */
export function dropAbove(siblings: Row[], from: number, onto: number) {
  if (from === onto) return null;
  return reorder(siblings, from, from < onto ? onto : onto + 1);
}

/** The line, from 1, that byte `at` of `text` is on. */
export function lineOf(text: string, at: number): number {
  const before = new TextDecoder().decode(new TextEncoder().encode(text).slice(0, at));
  return before.split('\n').length;
}

/** The UTF-8 byte offset of character offset `at` of `text`. */
export const byteOf = (text: string, at: number): number => new TextEncoder().encode(text.slice(0, at)).length;
