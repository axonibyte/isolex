// Paint (docs/studio-interface.md, Design; #117's Dropper): what each kind
// is painted with when its source says nothing, and a paint written into
// a node's text. Pure.

/** What each kind is painted with when its source says nothing; a kind
    not here is painted otherwise (a box's faces, a cylinder's top and
    side, a shadow's token). */
export const DEFAULT_PAINT: Record<string, { fill: string; stroke: string; width: number | null }> = {
  tile: { fill: 'body-top', stroke: 'outline', width: 1.2 },
  facerect: { fill: 'ink', stroke: 'none', width: null },
  poly3d: { fill: 'body-top', stroke: 'outline', width: 1.2 },
  seg: { fill: 'none', stroke: 'ink', width: 1.4 },
  line: { fill: 'none', stroke: 'ink', width: 1.4 },
  circle: { fill: 'none', stroke: 'outline', width: 1.2 },
  ellipse: { fill: 'none', stroke: 'outline', width: 1.2 },
  rect: { fill: 'none', stroke: 'none', width: 1.4 },
  arc: { fill: 'none', stroke: 'ink', width: 1.4 },
  path: { fill: 'none', stroke: 'ink', width: 1.4 },
  poly: { fill: 'ink', stroke: 'none', width: null },
};

/** A kind whose fill and stroke are its own properties. */
export const painted = (kind: string): boolean => kind in DEFAULT_PAINT;

/** `text`, a node about to be written, with `key="value"` on its first
    line, before its body if it has one. */
export function withPaint(text: string, key: string, value: string): string {
  const end = text.indexOf('\n');
  const first = end < 0 ? text : text.slice(0, end);
  const rest = end < 0 ? '' : text.slice(end);
  const prop = ` ${key}="${value}"`;
  const head = first.trimEnd().endsWith('{') ? `${first.trimEnd().slice(0, -1).trimEnd()}${prop} {` : `${first}${prop}`;
  return head + rest;
}
