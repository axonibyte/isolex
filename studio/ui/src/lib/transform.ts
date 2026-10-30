// Transform (#115, docs/studio-interface.md, Arrange), as the language
// writes it (#43, SPEC §4): shapes rotated, scaled, skewed or mirrored on
// the screen by a `transform` around them, about the selection's middle;
// 3-D shapes turned by a quarter, or mirrored across the diagonal, in
// tile space by a `turn` or `mirror-diagonal`. Never by rewriting their
// numbers or path data. Pure: the tools write the text.

import { apply, FLAT, type Point, type Solid } from './geometry';
import { extentOf } from './arrange';
import { clean } from './snap';

export type Step =
  | { kind: 'rotate'; deg: number }
  | { kind: 'scale'; sx: number; sy: number }
  | { kind: 'skew'; x: number; y: number }
  | { kind: 'mirror'; how: 'across' | 'down' }
  | { kind: 'turn'; deg: number }
  | { kind: 'mirror-diagonal' };

/** A step done in tile space, before projection: only around 3-D nodes. */
export const inTiles = (s: Step) => s.kind === 'turn' || s.kind === 'mirror-diagonal';

/** A step that changes nothing. */
export function idle(s: Step): boolean {
  if (s.kind === 'rotate' || s.kind === 'turn') return clean(s.deg % 360) === 0;
  if (s.kind === 'scale') return s.sx === 1 && s.sy === 1;
  if (s.kind === 'skew') return !s.x && !s.y;
  return false;
}

const n = (v: number) => String(clean(Math.round(v * 100) / 100));
const pair = (p: Point) => `"${n(p[0])} ${n(p[1])}"`;
/** A quarter-turn as the language takes it: 0, 90, 180 or 270. */
const quarter = (deg: number) => (((Math.round(deg / 90) * 90) % 360) + 360) % 360;

/** The node a step opens, before its children: about `origin`, on the
    screen in output units or, for a turn, in tiles. */
export function head(s: Step, origin: Point): string {
  const about = ` origin=${pair(origin)}`;
  switch (s.kind) {
    case 'rotate':
      return `transform rotate=${n(s.deg)}${about}`;
    case 'scale':
      return `transform scale=${s.sx === s.sy ? n(s.sx) : `"${n(s.sx)} ${n(s.sy)}"`}${about}`;
    case 'skew':
      return `transform${s.y ? ` skew-y=${n(s.y)}` : ''}${s.x ? ` skew-x=${n(s.x)}` : ''}${about}`;
    case 'mirror':
      return `transform mirror="${s.how}"${about}`;
    case 'turn':
      return `turn ${quarter(s.deg)}${about}`;
    case 'mirror-diagonal':
      return `mirror-diagonal${about}`;
  }
}

/** `body`, the text of nodes from the first's start to the last's end,
    inside a node opened by `head`, at the indent the first stood at. */
export function wrapped(body: string, indent: string, opened: string, unit = '  '): string {
  const [first, ...rest] = body.split('\n');
  const inner = [`${indent}${unit}${first}`, ...rest.map((l) => (l.trim() ? `${unit}${l}` : l))];
  return `${opened} {\n${inner.join('\n')}\n${indent}}`;
}

/** Its properties, before its body: `rotate=30` as rotate → "30". */
function props(text: string): Map<string, string> {
  const headText = text.slice(0, text.indexOf('{') < 0 ? undefined : text.indexOf('{'));
  const out = new Map<string, string>();
  for (const m of headText.matchAll(/([a-z-]+)=("[^"]*"|\S+)/g)) out.set(m[1]!, m[2]!.replace(/^"|"$/g, ''));
  return out;
}

const number = (v: string | undefined) => (v !== undefined && /^-?\d+(\.\d+)?$/.test(v) ? Number(v) : null);
/** What else a transform may say beside its one step and stay that step. */
const BESIDE = new Set(['origin', 'bounds', 'strokes', 'when']);

/** A step added to the transform or turn already selected, `text` its
    source, when it is the same kind of step, written as a plain number:
    what to set, by property or argument. None means wrap it instead. */
export function composed(kind: string, text: string, s: Step): { key: string; value: number | string } | { index: number; value: number } | null {
  if (kind === 'turn' && s.kind === 'turn') {
    const at = number(text.match(/^turn\s+(\S+)/)?.[1]);
    return at === null ? null : { index: 0, value: quarter(at + s.deg) };
  }
  if (kind !== 'transform' || /\bsteps\s*\{/.test(text)) return null;
  const p = props(text);
  const only = (key: string) => [...p.keys()].every((k) => k === key || BESIDE.has(k));
  if (s.kind === 'rotate' && p.has('rotate') && only('rotate')) {
    const at = number(p.get('rotate'));
    return at === null ? null : { key: 'rotate', value: clean((((at + s.deg) % 360) + 360) % 360) };
  }
  if (s.kind === 'scale' && p.has('scale') && only('scale')) {
    const parts = p.get('scale')!.split(/\s+/).map((v) => number(v));
    if (parts.some((v) => v === null)) return null;
    const [sx, sy] = [parts[0]!, parts[1] ?? parts[0]!];
    const [x, y] = [clean(sx * s.sx), clean(sy * s.sy)];
    return { key: 'scale', value: x === y ? x : `${n(x)} ${n(y)}` };
  }
  return null;
}

/** The box round what `solids` draw: on the screen, in output units,
    round every corner drawn; or in tiles, for a turn, round their
    footprint. None if none has bounds. */
export function around(solids: Solid[], tiles: boolean): { lo: Point; hi: Point } | null {
  const pts: Point[] = [];
  for (const s of solids) {
    const e = extentOf(s);
    if (!e) continue;
    if (tiles) {
      if (FLAT.has(s.kind)) continue;
      pts.push([e.lo[0]!, e.lo[1]!], [e.hi[0]!, e.hi[1]!]);
    } else if (FLAT.has(s.kind)) {
      for (const x of [e.lo[0]!, e.hi[0]!]) for (const y of [e.lo[1]!, e.hi[1]!]) pts.push(apply(s.map, [x, y, 0]));
    } else {
      for (const x of [e.lo[0]!, e.hi[0]!]) for (const y of [e.lo[1]!, e.hi[1]!]) for (const z of [e.lo[2]!, e.hi[2]!]) pts.push(apply(s.map, [x, y, z]));
    }
  }
  if (!pts.length) return null;
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return { lo: [Math.min(...xs), Math.min(...ys)], hi: [Math.max(...xs), Math.max(...ys)] };
}

/** The middle of what `solids` draw, as `around` measures it. */
export function middle(solids: Solid[], tiles: boolean): Point | null {
  const b = around(solids, tiles);
  return b ? [clean((b.lo[0] + b.hi[0]) / 2), clean((b.lo[1] + b.hi[1]) / 2)] : null;
}

/** The turn handles round a box on the screen (#115), as Inkscape shows
    them on a second click: its corners rotate, the middles of its sides
    scale across or down. */
export function pivotHandles(b: { lo: Point; hi: Point }): { id: string; at: Point; kind: 'rotate' | 'scale'; label: string }[] {
  const [x0, y0] = b.lo;
  const [x1, y1] = b.hi;
  const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
  const corners: Point[] = [
    [x0, y0],
    [x1, y0],
    [x1, y1],
    [x0, y1],
  ];
  return [
    ...corners.map((at, k) => ({ id: `rot-${k}`, at, kind: 'rotate' as const, label: 'Rotate: drag round the middle (Shift for single degrees)' })),
    { id: 'scale-x0', at: [x0, my] as Point, kind: 'scale' as const, label: 'Scale across (Shift keeps proportions)' },
    { id: 'scale-x1', at: [x1, my] as Point, kind: 'scale' as const, label: 'Scale across (Shift keeps proportions)' },
    { id: 'scale-y0', at: [mx, y0] as Point, kind: 'scale' as const, label: 'Scale down (Shift keeps proportions)' },
    { id: 'scale-y1', at: [mx, y1] as Point, kind: 'scale' as const, label: 'Scale down (Shift keeps proportions)' },
  ];
}

/** The step a turn handle dragged from `from` to `to` about `c` comes to:
    a rotation, clockwise on the screen, to 15° (to 1° with `fine`); or a
    scale across or down, to 5%, both ways with `keep`. */
export function pivotStep(handle: string, c: Point, from: Point, to: Point, fine: boolean): Step | null {
  if (handle.startsWith('rot-')) {
    const deg = ((Math.atan2(to[1] - c[1], to[0] - c[0]) - Math.atan2(from[1] - c[1], from[0] - c[0])) * 180) / Math.PI;
    const step = fine ? 1 : 15;
    const snapped = clean(Math.round((((deg % 360) + 540) % 360) / step) * step - 180);
    return { kind: 'rotate', deg: snapped === -180 ? 180 : snapped };
  }
  const across = handle.startsWith('scale-x');
  const i = across ? 0 : 1;
  const was = from[i] - c[i];
  if (Math.abs(was) < 1e-9) return null;
  const f = Math.max(0.05, clean(Math.round(((to[i] - c[i]) / was) * 20) / 20));
  if (fine) return { kind: 'scale', sx: f, sy: f };
  return across ? { kind: 'scale', sx: f, sy: 1 } : { kind: 'scale', sx: 1, sy: f };
}

/** A box on the screen drawn through a step about `c`: its outline, to
    preview a drag. */
export function stepped(b: { lo: Point; hi: Point }, c: Point, s: Step): Point[] {
  const corners: Point[] = [b.lo, [b.hi[0], b.lo[1]], b.hi, [b.lo[0], b.hi[1]], b.lo];
  return corners.map(([x, y]) => {
    const [dx, dy] = [x - c[0], y - c[1]];
    if (s.kind === 'rotate') {
      const a = (s.deg * Math.PI) / 180;
      return [c[0] + dx * Math.cos(a) - dy * Math.sin(a), c[1] + dx * Math.sin(a) + dy * Math.cos(a)];
    }
    if (s.kind === 'scale') return [c[0] + dx * s.sx, c[1] + dy * s.sy];
    return [x, y];
  });
}

/** Runs of neighbors: the rows given, by their siblings list and index,
    grouped where they stand next to each other, in source order (`at`,
    where each starts). */
export function runs<T extends { siblings: unknown[]; index: number; at: number }>(found: T[]): T[][] {
  const sorted = [...found].sort((a, b) => a.at - b.at);
  const out: T[][] = [];
  for (const f of sorted) {
    const last = out.at(-1)?.at(-1);
    if (last && last.siblings === f.siblings && f.index === last.index + 1) out.at(-1)!.push(f);
    else out.push([f]);
  }
  return out;
}
