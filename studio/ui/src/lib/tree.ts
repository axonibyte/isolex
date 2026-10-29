// How a row of the Tree tab relates to what is open (#71, docs/studio-
// interface.md, Tree: "hovering an icon says how it relates to what is
// open"), from the library's graph: extends, a variant of, badges, sets
// text in. Pure.

import type { Graph, TreeRow } from './backend';

/** The definitions `name` extends, nearest first, itself first; a cycle,
    which the resolver reports, ends it. */
export function chain(graph: Graph, name: string): string[] {
  const out = [name];
  for (;;) {
    const up = graph.edges.find((e) => e.kind === 'extends' && e.from === out.at(-1))?.to;
    if (!up || out.includes(up)) return out;
    out.push(up);
  }
}

/** The definition an emitted icon is written in. */
const writer = (graph: Graph, icon: string): string | null => graph.definitions.find((d) => d.emits.includes(icon))?.name ?? null;

/** What `row` is to the definition being edited, `editing`, drawn as
    `icon`: a sentence for the hint line; none with nothing open. */
export function relation(graph: Graph, row: TreeRow, editing: string | null, icon: string | null): string {
  if (!editing) return '';
  if (row.definition === editing && (!row.icon || row.icon === icon || row.kind !== 'variant')) return 'Being edited';
  if (row.kind === 'variant') {
    const of = row.icon ? writer(graph, row.icon) : null;
    const what = `Variant ${row.variant ?? row.name} of axis ${row.axis ?? '?'}`;
    if (!of) return what;
    const rest = of === editing ? `of ${editing}, which is being edited` : `of ${of}: ${lower(between(graph, of, editing))}`;
    return `${what}, ${rest}`;
  }
  if (row.kind === 'font') {
    return graph.edges.some((e) => e.kind === 'font' && e.to === row.definition && chain(graph, editing).includes(e.from)) ? `${editing} sets text in it` : `No relation to ${editing}`;
  }
  return between(graph, row.definition, editing);
}

/** How the definition `d` relates to `editing`. */
function between(graph: Graph, d: string, editing: string): string {
  const mine = chain(graph, d);
  const theirs = chain(graph, editing);
  const at = mine.indexOf(editing);
  if (at === 1) return `Extends ${editing}`;
  if (at > 1) return `Extends ${editing}, through ${mine.slice(1, at).join(', ')}`;
  if (theirs.includes(d)) return `${editing} extends it`;
  // a badge, either way: from an icon, or from an axis it opts into
  const badges = (from: string[], to: string) =>
    graph.edges.some((e) => e.kind === 'badge' && e.to === to && (from.includes(e.from) || from.some((f) => graph.edges.some((a) => a.kind === 'axis' && a.from === f && a.to === e.from))));
  if (badges(theirs, d)) return `${editing} badges it`;
  if (badges(mine, editing)) return `Badges ${editing}`;
  const shared = mine.slice(1).find((m) => theirs.includes(m));
  if (shared) return `Shares ${shared} with ${editing}`;
  return `No relation to ${editing}`;
}

const lower = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);
