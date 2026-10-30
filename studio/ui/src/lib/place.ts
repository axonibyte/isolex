// Insert and Badge (#116, docs/studio-interface.md, Insert and Badge): a
// fragment written as an `insert` where it is dropped, or an icon as a
// `badge` at the corner of the frame nearest the drop; refused when it
// would make a cycle, naming the definitions it would join. Pure.

import type { Graph, Row } from './backend';
import { FLAT, type Point } from './geometry';
import { clean } from './snap';

const n = (v: number) => String(clean(Math.round(v * 100) / 100));

/** A fragment draws flat when what it holds is flat: its insert then goes
    on an anchor, as flat shapes do. */
export function drawsFlat(rows: Row[]): boolean {
  const walk = (rs: Row[]): boolean => rs.some((r) => FLAT.has(r.kind) || r.kind === 'text' || walk(r.children));
  return walk(rows);
}

/** The insert of `fragment` at `at`: in tiles on the floor for a 3-D
    fragment, or in output units for a flat one; set as the fragment's own
    params when it has a pair for it (cx and cy, or x0 and y0: x and y are
    the language's own, inside `at`), else moved there on the screen by a
    transform round it (`screen`, where `at` is drawn). */
export function insertText(fragment: string, params: string[], at: Point, screen: Point): string {
  const has = (...names: string[]) => names.every((p) => params.includes(p));
  const name = JSON.stringify(fragment);
  for (const [px, py] of [
    ['cx', 'cy'],
    ['x0', 'y0'],
  ] as const) {
    if (has(px, py)) return `insert ${name} ${px}=${n(at[0])} ${py}=${n(at[1])}`;
  }
  if (!clean(screen[0]) && !clean(screen[1])) return `insert ${name}`;
  return `transform translate="${n(screen[0])} ${n(screen[1])}" {\n  insert ${name}\n}`;
}

export type Corner = 'nw' | 'ne' | 'sw' | 'se';

/** The corner of the frame, a viewBox, nearest `p`. */
export function nearestCorner(viewbox: [number, number, number, number], p: Point): Corner {
  const [x, y, w, h] = viewbox;
  const east = p[0] > x + w / 2;
  const south = p[1] > y + h / 2;
  return `${south ? 's' : 'n'}${east ? 'e' : 'w'}` as Corner;
}

/** The badge of `icon` at `corner`; the language's corner, `ne`, unsaid. */
export const badgeText = (icon: string, corner: Corner): string => `badge ${JSON.stringify(icon)}${corner === 'ne' ? '' : ` corner="${corner}"`}`;

/** What `from` refers to, by every kind of reference but a font's. */
const next = (graph: Graph, from: string) => graph.edges.filter((e) => e.from === from && e.kind !== 'font').map((e) => e.to);

/** The definitions a new reference from `into` to `target` would join in
    a cycle, `into` first and last; none if it would make none. */
export function cycle(graph: Graph, into: string, target: string): string[] | null {
  if (into === target) return [into, into];
  const back = new Map<string, string>([[target, '']]);
  const queue = [target];
  while (queue.length) {
    const at = queue.shift()!;
    for (const to of next(graph, at)) {
      if (back.has(to)) continue;
      back.set(to, at);
      if (to === into) {
        const path = [into];
        for (let k = at; k; k = back.get(k)!) path.unshift(k);
        return [into, ...path];
      }
      queue.push(to);
    }
  }
  return null;
}

/** The badge `icon` draws, if it or anything it extends or opts into
    places one: a badge's icon cannot (SPEC §3). */
export function badgeOf(graph: Graph, icon: string): string | null {
  const seen = new Set<string>();
  const queue = [icon];
  while (queue.length) {
    const at = queue.shift()!;
    if (seen.has(at)) continue;
    seen.add(at);
    for (const e of graph.edges.filter((e) => e.from === at)) {
      if (e.kind === 'badge') return e.to;
      if (e.kind === 'extends' || e.kind === 'axis') queue.push(e.to);
    }
  }
  return null;
}

/** Why `target` cannot be placed in `into`, or none. */
export function refusal(graph: Graph, into: string, target: string, kind: 'insert' | 'badge'): string | null {
  const def = graph.definitions.find((d) => d.name === target);
  if (!def) return `There is no ${target}`;
  if (kind === 'insert' && def.kind !== 'Fragment') return `${target} is not a fragment: only a fragment is inserted`;
  if (kind === 'badge' && def.kind !== 'Icon') return `${target} is not an icon: only an icon is a badge`;
  const loop = cycle(graph, into, target);
  if (loop) return `That would make a cycle: ${loop.join(' → ')}`;
  const own = kind === 'badge' ? badgeOf(graph, target) : null;
  if (own) return `${target} has a badge of its own (${own}): a badge’s icon cannot`;
  return null;
}
