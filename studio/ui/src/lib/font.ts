// Fonts in the editor (#84, docs/studio-interface.md, Fonts): how a font
// and a glyph are shown, as sketches the core compiles with the library
// (`sketch`), so each glyph is drawn as text draws it and its shapes still
// say where in the font they are written. Pure: the sketches' sources, and
// the geometry the canvas draws guides and handles by.

import type { Map3, Outline } from './backend';
import { propsOf } from './kdl';

/** The icon a sketch draws, and the file it is compiled in. */
export const SKETCH = 'studio-sketch';

/** A font's metrics as numbers: those written as numbers, else what
    shows a font sensibly until they are. */
export interface Metrics {
  em: number;
  ascent: number;
  descent: number;
  slant: number;
  kind: string;
}

export function metricsOf(o: Outline | null): Metrics {
  const m: Record<string, string> = {};
  for (const r of o?.font?.metrics ?? []) m[r.name] = r.value;
  const num = (v: string | undefined, d: number) => (v !== undefined && Number.isFinite(Number(v)) ? Number(v) : d);
  const em = num(m.em, 10);
  return { em, ascent: num(m.ascent, em * 0.7), descent: num(m.descent, em * 0.3), slant: num(m.slant, 0), kind: m.kind ?? 'outline' };
}

/** A string as KDL writes one: quotes and backslashes escaped. */
export const kdlString = (s: string): string => `"${s.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;

/** Text as a `text` node sets it: braces doubled, so they are their own. */
export const textValue = (s: string): string => kdlString(s.replace(/\{/g, '{{').replace(/\}/g, '}}'));

/** A character no font draws, so a text of it is set in the notdef. */
export const NOTDEF_CHAR = '';

/** What a glyph is set as: its name, or the notdef's character. */
const setAs = (name: string, op: string) => (op === 'notdef' ? NOTDEF_CHAR : name);

const wrap = (w: number, h: number, body: string) =>
  `icon "${SKETCH}" {\n  category "studio"\n  canvas ${w} ${h}\n  part "sketch" {\n    at 0 0 0 {\n${body}\n    }\n  }\n}\n`;

/** A grid cell's size, in output units, and the em size glyphs are set at. */
export const CELL = { w: 40, h: 44, size: 24, columns: 8 };

/** The font's glyphs set out in a grid, each in a cell of its own: as
    text draws them, ligatures and notdef among them. */
export function gridSource(font: string, o: Outline, style: string[] = []): { source: string; cells: { name: string; x: number; y: number }[] } {
  const glyphs = o.parts.filter((p) => p.op === 'glyph' || p.op === 'notdef');
  const rows = Math.max(1, Math.ceil(glyphs.length / CELL.columns));
  const cells = glyphs.map((g, i) => ({ name: g.name, op: g.op, x: (i % CELL.columns) * CELL.w, y: Math.floor(i / CELL.columns) * CELL.h }));
  const styled = style.length ? ` style="${style.join(' ')}"` : '';
  const body = cells
    .map((c) => `      text ${c.x + CELL.w / 2} ${c.y + CELL.h * 0.62} ${textValue(setAs(c.name, c.op))} font="${font}" size=${CELL.size} anchor="middle"${styled}`)
    .join('\n');
  return { source: wrap(CELL.w * Math.min(CELL.columns, Math.max(1, glyphs.length)), CELL.h * rows, body), cells };
}

/** How a glyph is shown: set large on its em grid, with room about it. */
export interface GlyphView {
  source: string;
  /** From the em grid (x right, y up) to the canvas. */
  map: Map3;
  width: number;
  height: number;
  /** Where its guides are, in the em grid. */
  advance: number;
  metrics: Metrics;
}

/** The em drawn this many output units tall. */
export const GLYPH_SIZE = 200;

export function glyphView(font: string, name: string, op: string, advance: number, m: Metrics): GlyphView {
  const k = GLYPH_SIZE / m.em;
  const pad = m.em * 0.25;
  const x0 = pad * k;
  const y0 = (m.ascent + pad) * k;
  const width = Math.max(advance, m.em * 0.5) * k + 2 * pad * k;
  const height = (m.ascent + m.descent + 2 * pad) * k;
  // as text draws it: T(x0, y0) · skewX(slant) · S(k, −k)
  const t = Math.tan((m.slant * Math.PI) / 180);
  const map: Map3 = [k, 0, -k * t, -k, 0, 0, x0, y0];
  const body = `      text ${x0} ${y0} ${textValue(setAs(name, op))} font="${font}" size=${GLYPH_SIZE}`;
  return { source: wrap(width, height, body), map, width, height, advance, metrics: m };
}

/** A glyph's advance, as its node writes it; the em when it is an
    expression. */
export function advanceOf(glyphText: string, m: Metrics): number {
  const a = Number(propsOf(glyphText).advance);
  return Number.isFinite(a) ? a : m.em;
}

/** A line of a font's glyphs, in a style: the Tree's sample. */
export function sampleSource(font: string, o: Outline, style: string[] = []): string {
  const names = o.parts.filter((p) => p.op === 'glyph' && p.name.trim() && [...p.name].length === 1).map((p) => p.name);
  const line = names.slice(0, 12).join('') || NOTDEF_CHAR;
  const styled = style.length ? ` style="${style.join(' ')}"` : '';
  return wrap(120, 16, `      text 2 12 ${textValue(line)} font="${font}" size=11 max-width=116${styled}`);
}

/** A kerning pair as its preview sets it: each glyph a sketch of its own
    on the same baseline, so the right-hand one is placed (and dragged) at
    the left one's advance and the space between them; `k` output units
    to one of the em grid. */
export function pairSources(font: string, left: string, right: string, m: Metrics): { left: string; right: string; k: number } {
  const size = 64;
  const k = size / m.em;
  const w = Math.round(size * 1.4);
  const h = Math.round((m.ascent + m.descent) * k + 16);
  const at = (g: string) => wrap(w, h, `      text 0 ${Math.round(m.ascent * k + 8)} ${textValue(g)} font="${font}" size=${size}`);
  return { left: at(left), right: at(right), k };
}

/** The fonts of the library, by the tree. */
export const fontsOf = (fonts: { name: string }[]): string[] => fonts.map((f) => f.name);
